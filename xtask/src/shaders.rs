//! `cargo xtask shaders [--check]`: the GPU renderer's shaders (crates/kompoz-gpu/shaders/*.slang) compiled for
//! every backend, as C++'s scripts/shaders.py compiled them, into files that are committed
//! (crates/kompoz-gpu/src/shaders/generated/): building KompozR needs none of the tools, only changing a shader does.
//!
//! Each entry point of each shader is compiled by Slang to SPIR-V 1.3 (Vulkan 1.1), checked by spirv-val, to MSL
//! (Metal) and to WGSL (WebGPU, rebound as the WebGPU backend binds it), as `{stem}-{entry}.{spv,metal,wgsl}`; a
//! generated `mod.rs` includes them, with each shader's version (what all its entry points compiled to, in eight
//! bytes). `--check` compiles them elsewhere and fails if what is committed differs (CI).
//!
//! Nothing is downloaded: Slang 2026.20's slangc is `KOMPOZ_SLANGC`, or where C++ Kompoz's script put it
//! (~/Projects/kompoz/third_party/.tools/slang-2026.20/bin/slangc); spirv-val is `KOMPOZ_SPIRV_VAL`, or the one on
//! the PATH (SPIRV-Tools).

use crate::sha256::Sha256;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The Slang release the shaders are compiled with (its output is what is committed: another one's differs).
const SLANG_VERSION: &str = "2026.20";

const SOURCES: &str = "crates/kompoz-gpu/shaders";
const GENERATED: &str = "crates/kompoz-gpu/src/shaders/generated";

/// What one entry point compiled to.
struct Compiled {
    stage: String,
    entry: String,
    spirv: Vec<u8>,
    msl: Vec<u8>,
    wgsl: Vec<u8>,
}

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let slangc = slangc()?;
    let spirv_val = spirv_val()?;
    let work = std::env::temp_dir().join(format!("kompoz-xtask-shaders-{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let result = generate(root, &slangc, &spirv_val, &work, check);
    let _ = fs::remove_dir_all(&work);
    result
}

/// Compiles every shader into `work`, then writes what changed into the generated directory (or, checking, lists
/// what differs there).
fn generate(root: &Path, slangc: &Path, spirv_val: &Path, work: &Path, check: bool) -> Result<(), String> {
    let sources_dir = root.join(SOURCES);
    let mut sources: Vec<PathBuf> = fs::read_dir(&sources_dir)
        .map_err(|e| format!("{}: {e}", sources_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "slang"))
        .collect();
    sources.sort();

    // Every file the generated directory should hold, and what it should hold.
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut shaders: Vec<(String, Vec<Compiled>, u64)> = Vec::new();
    for source in &sources {
        let stem = source.file_stem().and_then(|s| s.to_str()).ok_or("a shader's name isn't UTF-8")?.to_string();
        let text = fs::read_to_string(source).map_err(|e| format!("{}: {e}", source.display()))?;
        let entries = entry_points(&text);
        if entries.is_empty() {
            continue;
        }
        let mut compiled = Vec::new();
        let mut everything = Sha256::new();
        for (stage, entry) in entries {
            let c = compile(slangc, spirv_val, source, &stem, &stage, &entry, work)?;
            for data in [&c.spirv, &c.msl, &c.wgsl] {
                everything.update(data);
            }
            files.push((format!("{stem}-{entry}.spv"), c.spirv.clone()));
            files.push((format!("{stem}-{entry}.metal"), c.msl.clone()));
            files.push((format!("{stem}-{entry}.wgsl"), c.wgsl.clone()));
            compiled.push(c);
        }
        let digest = everything.digest();
        let mut first = [0u8; 8];
        first.copy_from_slice(&digest[..8]);
        shaders.push((stem, compiled, u64::from_le_bytes(first)));
    }
    files.push(("mod.rs".to_string(), module(&shaders).into_bytes()));

    let generated = root.join(GENERATED);
    let expected: BTreeSet<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    let present: Vec<String> = match fs::read_dir(&generated) {
        Ok(dir) => dir.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect(),
        Err(_) => Vec::new(),
    };
    let orphans: Vec<&String> = present.iter().filter(|name| !expected.contains(name.as_str())).collect();
    let shown = |name: &str| format!("{GENERATED}/{name}");
    if check {
        let mut stale: Vec<String> = files
            .iter()
            .filter(|(name, data)| fs::read(generated.join(name)).ok().as_ref() != Some(data))
            .map(|(name, _)| shown(name))
            .collect();
        stale.extend(orphans.iter().map(|name| shown(name)));
        if !stale.is_empty() {
            for s in &stale {
                eprintln!("out of date: {s}");
            }
            return Err("run `cargo xtask shaders` and commit what it writes".to_string());
        }
        println!("shaders: up to date");
        return Ok(());
    }
    fs::create_dir_all(&generated).map_err(|e| format!("{}: {e}", generated.display()))?;
    for (name, data) in &files {
        let path = generated.join(name);
        if fs::read(&path).ok().as_ref() != Some(data) {
            fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("wrote {}", shown(name));
        }
    }
    for name in orphans {
        let path = generated.join(name);
        fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("removed {}", shown(name));
    }
    Ok(())
}

/// Slang's compiler: `KOMPOZ_SLANGC`, else where C++ Kompoz's script put it; the pinned release.
fn slangc() -> Result<PathBuf, String> {
    let home_tool = std::env::var_os("HOME").map(|home| {
        PathBuf::from(home).join(format!("Projects/kompoz/third_party/.tools/slang-{SLANG_VERSION}/bin/slangc"))
    });
    let path = match std::env::var_os("KOMPOZ_SLANGC") {
        Some(path) => Some(PathBuf::from(path)),
        None => home_tool,
    };
    let Some(path) = path.filter(|p| p.is_file()) else {
        return Err(format!(
            "slangc not found: set KOMPOZ_SLANGC to Slang {SLANG_VERSION}'s slangc (its release's bin/slangc, from \
             https://github.com/shader-slang/slang/releases/tag/v{SLANG_VERSION}), or put it at \
             ~/Projects/kompoz/third_party/.tools/slang-{SLANG_VERSION}/bin/slangc. Nothing is downloaded."
        ));
    };
    let output = Command::new(&path).arg("-version").output().map_err(|e| format!("{}: {e}", path.display()))?;
    let version =
        String::from_utf8_lossy(&output.stdout).trim().to_string() + String::from_utf8_lossy(&output.stderr).trim();
    if version != SLANG_VERSION {
        return Err(format!(
            "{} is Slang {version}, not {SLANG_VERSION}: what is committed is {SLANG_VERSION}'s output",
            path.display()
        ));
    }
    Ok(path)
}

/// SPIRV-Tools' validator: `KOMPOZ_SPIRV_VAL`, else the one on the PATH.
fn spirv_val() -> Result<PathBuf, String> {
    let path = std::env::var_os("KOMPOZ_SPIRV_VAL").map_or_else(|| PathBuf::from("spirv-val"), PathBuf::from);
    match Command::new(&path).arg("--version").output() {
        Ok(output) if output.status.success() => Ok(path),
        _ => Err(format!(
            "spirv-val not found ({}): install SPIRV-Tools (spirv-tools) or set KOMPOZ_SPIRV_VAL. Nothing is \
             downloaded.",
            path.display()
        )),
    }
}

/// (stage, entry name) for each `[shader("...")]` function of a Slang file, in order (shaders.py's regex:
/// `\[shader\("(\w+)"\)\]\s*(?:\[[^\]]*\]\s*)*[\w<>, ]+?\s+(\w+)\s*\(`).
fn entry_points(text: &str) -> Vec<(String, String)> {
    const MARK: &str = "[shader(\"";
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        rest = &rest[at + MARK.len()..];
        let stage_len = rest.find(|c: char| !word(c)).unwrap_or(rest.len());
        let stage = &rest[..stage_len];
        let Some(after) = rest[stage_len..].strip_prefix("\")]") else { continue };
        if stage.is_empty() {
            continue;
        }
        // Other attributes, then the return type, the name, and its parameters' parenthesis.
        let mut s = after.trim_start();
        while let Some(attribute) = s.strip_prefix('[') {
            let Some(end) = attribute.find(']') else { break };
            s = attribute[end + 1..].trim_start();
        }
        let Some(paren) = s.find('(') else { continue };
        let head = s[..paren].trim_end();
        let name_start = head.rfind(|c: char| !word(c)).map_or(0, |i| i + 1);
        let (ty, name) = head.split_at(name_start);
        let typed = !ty.trim().is_empty()
            && ty.ends_with(char::is_whitespace)
            && ty.trim_end().chars().all(|c| word(c) || "<>, ".contains(c));
        if !name.is_empty() && typed {
            out.push((stage.to_string(), name.to_string()));
        }
    }
    out
}

/// One entry point compiled for every target (shaders.py's compile_shader).
fn compile(
    slangc: &Path,
    spirv_val: &Path,
    source: &Path,
    stem: &str,
    stage: &str,
    entry: &str,
    work: &Path,
) -> Result<Compiled, String> {
    let base = work.join(format!("{stem}-{entry}"));
    let run = |command: &mut Command| -> Result<(), String> {
        let output = command.output().map_err(|e| format!("{stem}.slang ({entry}): {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "{stem}.slang ({entry}): {} failed\n{}{}",
                Path::new(command.get_program()).display(),
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        Ok(())
    };
    let slang = |target: &str, suffix: &str, extra: &[&str]| -> Result<Vec<u8>, String> {
        let out = base.with_extension(suffix);
        // No #line directives: they would carry this machine's path into the sources.
        run(Command::new(slangc)
            .arg(source)
            .args(["-entry", entry, "-stage", stage, "-target", target, "-O2", "-matrix-layout-row-major"])
            .args(["-line-directive-mode", "none", "-o"])
            .arg(&out)
            .args(extra))?;
        fs::read(&out).map_err(|e| format!("{}: {e}", out.display()))
    };
    // SPIR-V 1.3: what Vulkan 1.1 takes (every Vulkan Android device); entry points named as on the other targets.
    let spirv = slang("spirv", "spv", &["-DKOMPOZ_SPIRV", "-profile", "spirv_1_3", "-fvk-use-entrypoint-name"])?;
    run(Command::new(spirv_val).args(["--target-env", "vulkan1.1"]).arg(base.with_extension("spv")))?;
    let msl = slang("metal", "metal", &[])?;
    let wgsl = rebind_wgsl(&String::from_utf8(slang("wgsl", "wgsl", &[])?).map_err(|_| "WGSL that isn't UTF-8")?)
        .map_err(|e| format!("{stem}.slang ({entry}): {e}"))?;
    Ok(Compiled { stage: stage.to_string(), entry: entry.to_string(), spirv, msl, wgsl: wgsl.into_bytes() })
}

/// WGSL bound as the WebGPU backend binds it (shaders.py's wgsl()): Slang leaves the uniforms (Vulkan's push
/// constants) without a binding and splits each combined sampler into its texture at the sampler's binding and its
/// sampler at the next, where the next texture is. The uniforms go to binding 0, texture slot i (binding i + 1 in
/// the source) to bindings 2i + 1 (the texture) and 2i + 2 (its sampler). And derivatives are taken in non-uniform
/// control flow, as every other target allows: WGSL's uniformity diagnostic for them is turned off. (`@interpolate`
/// is left out where WGSL refuses it: see below.)
fn rebind_wgsl(text: &str) -> Result<String, String> {
    const BINDING: &str = "@binding(";
    const GROUP: &str = ") @group(0) var ";
    // `@binding(N) @group(0) var IDENTIFIER` at each place it is, with what follows the identifier.
    fn declarations(text: &str) -> Vec<(usize, usize, u32, &str, &str)> {
        let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = text[from..].find(BINDING).map(|i| from + i) {
            from = at + BINDING.len();
            let digits = text[from..].find(|c: char| !c.is_ascii_digit()).map_or(text.len(), |i| from + i);
            let Ok(binding) = text[from..digits].parse::<u32>() else { continue };
            let Some(rest) = text[digits..].strip_prefix(GROUP) else { continue };
            let start = digits + GROUP.len();
            let len = rest.find(|c: char| !word(c)).unwrap_or(rest.len());
            if len == 0 {
                continue;
            }
            out.push((at, start + len, binding, &rest[..len], &rest[len..]));
        }
        out
    }

    // Uniforms: binding 0. And `@interpolate` only on what is a stage's input or output (`@location`): Slang puts it
    // on the members of a fragment shader's own copy of its varyings too, which WGSL refuses (Chrome's Tint since
    // the C++ Kompoz's web build: "'@interpolate' can only be used with '@location'"); there it says nothing.
    let mut s = String::with_capacity(text.len() + 64);
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let line = match trimmed.strip_prefix("@interpolate(").and_then(|rest| rest.split_once(')')) {
            Some((_, member)) if !member.contains("@location(") => {
                format!("{}{}", &line[..line.len() - trimmed.len()], member.trim_start())
            }
            _ => line.to_string(),
        };
        let line = line.as_str();
        match line.strip_prefix("var<uniform> ") {
            Some(rest) => {
                s.push_str("@binding(0) @group(0) var<uniform> ");
                s.push_str(rest);
            }
            None => s.push_str(line),
        }
    }
    // Each texture's slot (its binding less one), by the name before `_texture_<n>`.
    let mut slots: Vec<(String, u32)> = Vec::new();
    for (_, _, binding, ident, after) in declarations(&s) {
        if let Some(cut) = ident.rfind("_texture_")
            && cut > 0
            && ident[cut + "_texture_".len()..].chars().all(|c| c.is_ascii_digit())
            && ident.len() > cut + "_texture_".len()
            && after.starts_with(" : texture_2d<f32>;")
            && binding >= 1
        {
            slots.push((ident[..cut].to_string(), binding - 1));
        }
    }
    // Each texture and sampler rebound by its slot.
    let mut out = String::with_capacity(s.len() + 64);
    let mut copied = 0;
    for (at, end, _, ident, _) in declarations(&s) {
        let cut = ["_texture_", "_sampler_"].iter().filter_map(|k| ident.rfind(k).map(|i| (i, *k))).max();
        let Some((cut, kind)) = cut.filter(|&(i, _)| i > 0) else { continue };
        let name = &ident[..cut];
        let slot = slots
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, slot)| slot)
            .ok_or_else(|| format!("WGSL: no texture for `{ident}`"))?;
        let binding = 2 * slot + if kind == "_texture_" { 1 } else { 2 };
        out.push_str(&s[copied..at]);
        out.push_str(&format!("{BINDING}{binding}{GROUP}{ident}"));
        copied = end;
    }
    out.push_str(&s[copied..]);
    Ok(format!("diagnostic(off, derivative_uniformity);\n{out}"))
}

/// The generated `mod.rs`: each entry point's code (a [`ShaderSource`] of the files), and each shader's version.
fn module(shaders: &[(String, Vec<Compiled>, u64)]) -> String {
    let mut s = String::from(
        "// Generated by `cargo xtask shaders` from crates/kompoz-gpu/shaders/*.slang: don't edit, run it.\n\
         \n\
         use super::ShaderSource;\n",
    );
    for (stem, compiled, version) in shaders {
        for c in compiled {
            let name = format!("{}_{}", stem.to_uppercase(), screaming(&c.entry));
            let file = format!("{stem}-{}", c.entry);
            s.push_str(&format!(
                "\n/// {stem}.slang's {stage} entry point `{entry}`.\n\
                 pub const {name}: ShaderSource = ShaderSource {{\n    \
                 spirv: include_bytes!(\"{file}.spv\"),\n    \
                 msl: include_str!(\"{file}.metal\"),\n    \
                 wgsl: include_str!(\"{file}.wgsl\"),\n    \
                 entry: \"{entry}\",\n\
                 }};\n",
                stage = c.stage,
                entry = c.entry,
            ));
        }
        s.push_str(&format!(
            "\n/// What {stem}.slang's entry points all compiled to, in eight bytes (the first of the SHA-256 of their \
             SPIR-V,\n/// MSL and WGSL, little-endian): pipelines made from other code (an older build's, kept in a \
             cache) aren't these.\n\
             pub const {}_VERSION: u64 = 0x{version:016x};\n",
            stem.to_uppercase()
        ));
    }
    s
}

/// `vsMesh` as `VS_MESH`.
fn screaming(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{entry_points, rebind_wgsl, screaming};

    #[test]
    fn entry_points_are_shaders_pys() {
        let text = "[shader(\"vertex\")]\nVaryings vs(Instance instance, uint vertex : VERTEX_ID) {\n}\n\
                    [shader(\"fragment\")]\n[vk::something(1)] float4 fs(Varyings v) : SV_Target {}\n\
                    [shader(\"vertex\")] Varyings vsMesh (Instance i) {}\n\
                    // [shader(\"vertex\")] not one: (\n";
        assert_eq!(
            entry_points(text),
            [("vertex", "vs"), ("fragment", "fs"), ("vertex", "vsMesh")]
                .map(|(s, e)| (s.to_string(), e.to_string()))
                .to_vec()
        );
        assert_eq!(screaming("vsMesh"), "VS_MESH");
        assert_eq!(screaming("fs"), "FS");
    }

    #[test]
    fn wgsl_is_bound_as_webgpu_binds_it() {
        let text = "var<uniform> uniforms_0 : U;\n\
                    @binding(2) @group(0) var table_texture_0 : texture_2d<f32>;\n\
                    @binding(3) @group(0) var table_sampler_0 : sampler;\n\
                    @binding(1) @group(0) var source_texture_0 : texture_2d<f32>;\n\
                    @binding(2) @group(0) var source_sampler_0 : sampler;\n\
                    fn f() { var<uniform> x; }\n";
        assert_eq!(
            rebind_wgsl(text).unwrap(),
            "diagnostic(off, derivative_uniformity);\n\
             @binding(0) @group(0) var<uniform> uniforms_0 : U;\n\
             @binding(3) @group(0) var table_texture_0 : texture_2d<f32>;\n\
             @binding(4) @group(0) var table_sampler_0 : sampler;\n\
             @binding(1) @group(0) var source_texture_0 : texture_2d<f32>;\n\
             @binding(2) @group(0) var source_sampler_0 : sampler;\n\
             fn f() { var<uniform> x; }\n"
        );
        // `@interpolate` kept where it is an output's (or an input's), left out on a struct's own member.
        assert_eq!(
            rebind_wgsl(
                "struct V\n{\n    @interpolate(flat) @location(4) a : f32,\n    @interpolate(flat)  b : f32,\n};\n"
            )
            .unwrap(),
            "diagnostic(off, derivative_uniformity);\n\
             struct V\n{\n    @interpolate(flat) @location(4) a : f32,\n    b : f32,\n};\n"
        );
        // A sampler without its texture: an error (shaders.py's KeyError).
        assert!(rebind_wgsl("@binding(2) @group(0) var lone_sampler_0 : sampler;\n").is_err());
    }
}
