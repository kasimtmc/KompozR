//! The shadow project: a copy of a package (`<target dir>/kompoz-shadow/<package>/`) whose `kompoz!` blocks and
//! `kompoz_file!` calls are the Rust the generator makes of them, so a rust-analyzer of its own (and its
//! `cargo check`) reads the Kotlin's Rust as rustc does, each generated token remembering its Kotlin place
//! ([`super::map`]).
//!
//! - Its manifest is written from `cargo metadata` of the package (inherited workspace keys resolved, path
//!   dependencies absolute, its targets at their shadow paths), a workspace of its own, the workspace's `Cargo.lock`
//!   and `[patch]` sections copied: the same dependency versions, offline.
//! - Its sources mirror the package's: `.rs` files written (the open documents as the editor has them), anything
//!   else a symbolic link to the original (`include_str!` reads the same file); `target/` and hidden entries left out.

use super::map::{self, Block, Markers, Printed, ShadowMap};
use crate::doc::path_to_uri;
use crate::json::Json;
use crate::text::{Lines, Normalized};
use crate::workspace;
use kompoz_kotlin::module::{self, ModuleTable};
use kompoz_kotlin::{Options, Paths, Pos, TextSource};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A package of the editor's, as its shadow is made.
pub struct Package {
    /// Its directory (canonical).
    pub dir: PathBuf,
    pub name: String,
    /// Its workspace's target directory (the shadow's `cargo check` builds there: the dependencies already built).
    pub target_dir: PathBuf,
    /// The shadow's directory.
    pub shadow: PathBuf,
    /// How its Kotlin's Rust names KompozR's crates (what the macro finds out from its manifest).
    pub paths: Paths,
    /// It uses the Kotlin syntax (depends on `kompoz` or `kompoz-kotlin-macro`, or is one of KompozR's crates).
    pub uses_kompoz: bool,
    manifest: String,
    workspace_root: PathBuf,
}

impl Package {
    /// The package in `dir` (`cargo metadata` of it; `cargo`: the command, `$CARGO` or `cargo`).
    pub fn load(dir: &Path) -> Result<Package, String> {
        let dir = workspace::key(dir);
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let out = std::process::Command::new(cargo)
            .args(["metadata", "--no-deps", "--format-version", "1", "--offline", "--manifest-path"])
            .arg(dir.join("Cargo.toml"))
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("cargo metadata: {e}"))?;
        if !out.status.success() {
            return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        let meta = Json::parse(&String::from_utf8_lossy(&out.stdout))?;
        let manifest_path = dir.join("Cargo.toml");
        let pkg = meta["packages"]
            .array()
            .iter()
            .find(|p| workspace::key(Path::new(p["manifest_path"].str())) == manifest_path)
            .ok_or_else(|| format!("cargo metadata: no package at {}", dir.display()))?
            .clone();
        let target_dir = PathBuf::from(meta["target_directory"].str());
        let workspace_root = PathBuf::from(meta["workspace_root"].str());
        let name = pkg["name"].str().to_string();
        let shadow = target_dir.join("kompoz-shadow").join(&name);
        let deps = pkg["dependencies"].array();
        let uses_kompoz = name.starts_with("kompoz")
            || deps.iter().any(|d| ["kompoz", "kompoz-kotlin-macro"].contains(&d["name"].str()));
        let paths = paths_of(&name, deps);
        let manifest = manifest(&pkg, &dir, &shadow, &workspace_root);
        Ok(Package { dir, name, target_dir, shadow, paths, uses_kompoz, manifest, workspace_root })
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.shadow.join("Cargo.toml")
    }

    /// The shadow path of a file of the package.
    pub fn shadow_of(&self, file: &Path) -> Option<PathBuf> {
        Some(self.shadow.join(file.strip_prefix(&self.dir).ok()?))
    }
}

/// The calling crate's paths to KompozR's crates, as kompoz-kotlin-macro's `paths::resolve` finds them.
fn paths_of(package: &str, deps: &[Json]) -> Paths {
    let named = |p: &str| -> Option<String> {
        deps.iter()
            .find(|d| d["name"].str() == p)
            .map(|d| d["rename"].as_str().unwrap_or(d["name"].str()).replace('-', "_"))
    };
    let ident = |n: &str| Ident::new(n, Span::call_site());
    if package == "kompoz" {
        return Paths::umbrella(&ident("kompoz"));
    }
    if let Some(n) = named("kompoz") {
        return Paths::umbrella(&ident(&n));
    }
    let path = |p: &str| -> TokenStream {
        if package == p {
            return quote!(crate);
        }
        let i = ident(&named(p).unwrap_or_else(|| p.replace('-', "_")));
        quote!(::#i)
    };
    Paths {
        runtime: path("kompoz-runtime"),
        coroutines: path("kompoz-coroutines"),
        graphics: path("kompoz-graphics"),
        text: path("kompoz-text"),
        ui: path("kompoz-ui"),
        foundation: path("kompoz-foundation"),
    }
}

/// A TOML string.
fn toml_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn toml_list(items: &[Json]) -> String {
    let v: Vec<String> = items.iter().map(|i| toml_str(i.str())).collect();
    format!("[{}]", v.join(", "))
}

/// The shadow's manifest, from the package's metadata.
fn manifest(pkg: &Json, dir: &Path, shadow: &Path, workspace_root: &Path) -> String {
    let at = |p: &str| -> String {
        let p = Path::new(p);
        let moved = p.strip_prefix(dir).map(|r| shadow.join(r)).unwrap_or_else(|_| p.to_path_buf());
        toml_str(&moved.to_string_lossy())
    };
    let edition = pkg["edition"].str();
    let mut m =
        String::from("# Written by kompoz-lsp: the shadow of a package (its Kotlin as Rust), for its rust-analyzer.\n");
    m += "[package]\n";
    m += &format!(
        "name = {}\nversion = {}\nedition = {}\n",
        toml_str(pkg["name"].str()),
        toml_str(pkg["version"].str()),
        toml_str(edition)
    );
    m += "publish = false\nautolib = false\nautobins = false\nautoexamples = false\nautotests = false\nautobenches = false\n";
    if let Some(l) = pkg["links"].as_str() {
        m += &format!("links = {}\n", toml_str(l));
    }
    if let Some(r) = pkg["rust_version"].as_str() {
        m += &format!("rust-version = {}\n", toml_str(r));
    }
    let targets = pkg["targets"].array();
    match targets.iter().find(|t| t["kind"].array().iter().any(|k| k.str() == "custom-build")) {
        Some(b) => m += &format!("build = {}\n", at(b["src_path"].str())),
        None => m += "build = false\n",
    }
    let mut sections = String::new();
    for t in targets {
        let kinds: Vec<&str> = t["kind"].array().iter().map(Json::str).collect();
        let table = match kinds.first().copied() {
            Some("custom-build") | None => continue,
            Some("bin") => "[[bin]]",
            Some("example") => "[[example]]",
            Some("test") => "[[test]]",
            Some("bench") => "[[bench]]",
            Some(_) => "[lib]",
        };
        sections += &format!("\n{table}\n");
        let name = if table == "[lib]" { t["name"].str().replace('-', "_") } else { t["name"].str().to_string() };
        sections += &format!("name = {}\npath = {}\n", toml_str(&name), at(t["src_path"].str()));
        if table == "[lib]" {
            if kinds.contains(&"proc-macro") {
                sections += "proc-macro = true\n";
            } else {
                sections += &format!("crate-type = {}\n", toml_list(t["crate_types"].array()));
            }
        }
        if t["edition"].str() != edition && !t["edition"].str().is_empty() {
            sections += &format!("edition = {}\n", toml_str(t["edition"].str()));
        }
        if t["test"].as_bool() == Some(false) {
            sections += "test = false\n";
        }
        if t["doctest"].as_bool() == Some(false) && table == "[lib]" {
            sections += "doctest = false\n";
        }
        if !t["required-features"].array().is_empty() {
            sections += &format!("required-features = {}\n", toml_list(t["required-features"].array()));
        }
    }
    m += &sections;
    // (dependencies, by section, in order)
    let mut by_section: Vec<(String, Vec<String>)> = Vec::new();
    for d in pkg["dependencies"].array() {
        let kind = match d["kind"].as_str() {
            Some("dev") => "dev-dependencies",
            Some("build") => "build-dependencies",
            _ => "dependencies",
        };
        let section = match d["target"].as_str() {
            Some(t) => format!("target.{}.{kind}", toml_str(t)),
            None => kind.to_string(),
        };
        let name = d["name"].str();
        let key = d["rename"].as_str().unwrap_or(name);
        let mut fields = Vec::new();
        if d["rename"].as_str().is_some() {
            fields.push(format!("package = {}", toml_str(name)));
        }
        let source = d["source"].as_str().unwrap_or("");
        if let Some(git) = source.strip_prefix("git+") {
            let (url, rev) = git.split_once('#').unwrap_or((git, ""));
            let url = url.split('?').next().unwrap_or(url);
            fields.push(format!("git = {}", toml_str(url)));
            if !rev.is_empty() {
                fields.push(format!("rev = {}", toml_str(rev)));
            }
        } else if let Some(p) = d["path"].as_str() {
            fields.push(format!("path = {}", toml_str(p)));
        }
        let req = d["req"].str();
        if !req.is_empty() && req != "*" {
            fields.push(format!("version = {}", toml_str(req)));
        }
        if let Some(r) = d["registry"].as_str() {
            fields.push(format!("registry-index = {}", toml_str(r)));
        }
        if !d["features"].array().is_empty() {
            fields.push(format!("features = {}", toml_list(d["features"].array())));
        }
        if d["uses_default_features"].as_bool() == Some(false) {
            fields.push("default-features = false".into());
        }
        if d["optional"].as_bool() == Some(true) {
            fields.push("optional = true".into());
        }
        let line = format!("{} = {{ {} }}", toml_str(key), fields.join(", "));
        match by_section.iter_mut().find(|(s, _)| *s == section) {
            Some((_, lines)) => lines.push(line),
            None => by_section.push((section, vec![line])),
        }
    }
    for (section, lines) in by_section {
        m += &format!("\n[{section}]\n{}\n", lines.join("\n"));
    }
    if !pkg["features"].members().is_empty() {
        m += "\n[features]\n";
        for (k, v) in pkg["features"].members() {
            m += &format!("{} = {}\n", toml_str(k), toml_list(v.array()));
        }
    }
    m += "\n[workspace]\n";
    m += &patches(workspace_root);
    m
}

/// The workspace manifest's `[patch…]` sections, their relative paths made absolute.
fn patches(workspace_root: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(workspace_root.join("Cargo.toml")) else { return String::new() };
    let mut out = String::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('[') {
            inside = t.starts_with("[patch");
            if inside {
                out.push('\n');
            }
        }
        if !inside {
            continue;
        }
        let mut line = line.to_string();
        if let Some(i) = line.find("path = \"") {
            let from = i + "path = \"".len();
            if let Some(len) = line[from..].find('"') {
                let p = &line[from..from + len];
                if Path::new(p).is_relative() {
                    let abs = workspace_root.join(p).to_string_lossy().replace('\\', "/");
                    line.replace_range(from..from + len, &abs);
                }
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// A Kotlin origin: a `.rs` file with `kompoz!` blocks, or a `.kt` file, as its shadow was made of it.
#[derive(Debug)]
pub struct Origin {
    pub path: PathBuf,
    pub uri: String,
    pub original: String,
    pub norm: Normalized,
    pub lines: Lines,
}

impl Origin {
    pub fn new(path: &Path, original: String) -> Origin {
        let norm = Normalized::new(&original);
        let lines = Lines::new(&original);
        Origin { uri: path_to_uri(path), path: path.to_path_buf(), original, norm, lines }
    }

    /// An LSP range of a normalized byte range.
    pub fn range(&self, r: std::ops::Range<usize>) -> Json {
        self.lines.range(&self.original, self.norm.to_original(r.start), self.norm.to_original(r.end))
    }
}

/// A shadow source with Kotlin made Rust.
#[derive(Debug)]
pub struct ShadowFile {
    pub path: PathBuf,
    pub uri: String,
    pub text: String,
    pub lines: Lines,
    pub map: ShadowMap,
    /// Its origins (the `.rs` file first, then the `.kt` files it reads), by [`map::Block::origin`].
    pub origins: Vec<Arc<Origin>>,
}

impl ShadowFile {
    /// The origin and its LSP range of a shadow LSP range.
    pub fn to_origin(&self, range: &Json) -> Option<(&Arc<Origin>, Json)> {
        let from = self.lines.offset_of(&self.text, &range["start"]);
        let to = self.lines.offset_of(&self.text, &range["end"]).max(from);
        let (o, r) = self.map.to_origin(from..to, |i| self.origins[i].norm.text.as_str())?;
        let origin = &self.origins[o];
        Some((origin, origin.range(r)))
    }
}

/// What a Rust source's Kotlin is replaced by.
enum Kotlin {
    /// A `kompoz!` block: its text's normalized range.
    Block(std::ops::Range<usize>),
    /// A `kompoz_file!` call: the file's path.
    File(String),
}

fn skip_ws_back(src: &str, mut i: usize) -> usize {
    while i > 0 && src.as_bytes()[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i
}

fn skip_ws(src: &str, mut i: usize) -> usize {
    while i < src.len() && src.as_bytes()[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Where the invocation `[::path::]name ! <delimiter at open>` starts.
fn invocation_start(src: &str, open: usize, name: &str) -> Option<usize> {
    let b = src.as_bytes();
    let mut i = skip_ws_back(src, open);
    if i == 0 || b[i - 1] != b'!' {
        return None;
    }
    i = skip_ws_back(src, i - 1);
    if !src[..i].ends_with(name) {
        return None;
    }
    i -= name.len();
    loop {
        let j = skip_ws_back(src, i);
        if !src[..j].ends_with("::") {
            return Some(i);
        }
        let k = skip_ws_back(src, j - 2);
        let mut s = k;
        while s > 0 && is_ident_byte(b[s - 1]) {
            s -= 1;
        }
        if s == k {
            // (a leading `::`)
            return Some(j - 2);
        }
        i = s;
    }
}

/// Where the invocation closed at `close` ends (its `;` with it).
fn invocation_end(src: &str, close: usize) -> usize {
    let e = close + 1;
    let j = skip_ws(src, e);
    if src[j..].starts_with(';') { j + 1 } else { e }
}

/// The Rust of a Kotlin text, printed with its marks; whether the generator found errors.
fn generate(text: &str, base: Pos, opts: &Options) -> (Printed, bool) {
    let markers = Markers::new(text.len());
    let t = kompoz_kotlin::translate(text, base, &markers, opts);
    let failed = t.diagnostics.iter().any(|d| d.is_error());
    (map::print(t.tokens), failed)
}

/// A `.rs` source with its Kotlin made Rust (`kt`: a `.kt` file's text by path, the editor's or the disk's). None:
/// no Kotlin in it.
pub fn shadow_source(
    pkg: &Package,
    rs: Arc<Origin>,
    table: Option<Arc<ModuleTable>>,
    kt: &dyn Fn(&Path) -> Option<String>,
) -> Option<ShadowFile> {
    let src = rs.norm.text.as_str();
    let found = module::kotlin_in(src);
    if found.blocks.is_empty() && found.kt_files.is_empty() {
        return None;
    }
    let mut parts: Vec<(std::ops::Range<usize>, Kotlin)> = Vec::new();
    for b in &found.blocks {
        let Some(start) = invocation_start(src, b.start - 1, "kompoz") else { continue };
        parts.push((start..invocation_end(src, b.end), Kotlin::Block(b.clone())));
    }
    for (name, lit) in &found.kt_files {
        let open = skip_ws_back(src, lit.start);
        let close = skip_ws(src, lit.end);
        if open == 0 || !src[..open].ends_with('(') || !src[close..].starts_with(')') {
            continue;
        }
        let Some(start) = invocation_start(src, open - 1, "kompoz_file") else { continue };
        parts.push((start..invocation_end(src, close), Kotlin::File(name.clone())));
    }
    parts.sort_by_key(|(r, _)| r.start);
    let base = Options { paths: pkg.paths.clone(), module: table, ..Default::default() };
    let mut text = String::with_capacity(src.len() * 2);
    let mut blocks = Vec::new();
    let mut origins = vec![rs.clone()];
    let mut at = 0;
    for (range, kotlin) in parts {
        if range.start < at {
            continue;
        }
        text.push_str(&src[at..range.start]);
        let (printed, failed, origin, kotlin_start) = match kotlin {
            Kotlin::Block(b) => {
                let opts = Options { text: TextSource::Exact, keyed_calls: false, ..base.clone() };
                let (p, f) = generate(&src[b.clone()], module::pos_in(src, b.start), &opts);
                (p, f, 0, b.start)
            }
            Kotlin::File(name) => {
                let path = pkg.dir.join(&name);
                let Some(original) = kt(&path) else {
                    // (no such file: the call as it is, rustc's error the macro's)
                    text.push_str(&src[range.clone()]);
                    at = range.end;
                    continue;
                };
                let o = Arc::new(Origin::new(&path, original));
                let opts = Options { text: TextSource::Exact, keyed_calls: true, ..base.clone() };
                let (p, f) = generate(&o.norm.text, Pos::start(1, 1), &opts);
                origins.push(o);
                (p, f, origins.len() - 1, 0)
            }
        };
        let start = text.len();
        text.push_str(&printed.text);
        blocks.push(Block {
            shadow: start..text.len(),
            origin,
            kotlin_start,
            marks: printed.marks.iter().map(|(r, k)| (r.start + start..r.end + start, *k)).collect(),
            failed,
        });
        at = range.end;
    }
    text.push_str(&src[at..]);
    // (proc-macro2's spans of this thread: the markers' text, no longer needed)
    proc_macro2::extra::invalidate_current_thread_spans();
    let path = pkg.shadow_of(&rs.path)?;
    Some(ShadowFile {
        uri: path_to_uri(&path),
        lines: Lines::new(&text),
        path,
        text,
        map: ShadowMap { blocks },
        origins,
    })
}

/// Writes `content` to `path` when it differs; whether it did.
fn write_if_changed(path: &Path, content: &[u8]) -> bool {
    if std::fs::read(path).is_ok_and(|old| old == content) {
        return false;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, content).is_ok()
}

/// What [`Mirror::update`] did.
#[derive(Default)]
pub struct Updated {
    /// The shadow sources with Kotlin, as now.
    pub files: Vec<Arc<ShadowFile>>,
    /// Shadow files written (changed on disk).
    pub written: Vec<PathBuf>,
}

/// A package's shadow kept up to date.
pub struct Mirror {
    pub pkg: Package,
    /// The package's `.rs` files with Kotlin (as of the last full walk, and since).
    with_kotlin: HashSet<PathBuf>,
    last_walk: Option<std::time::Instant>,
}

impl Mirror {
    pub fn new(pkg: Package) -> Mirror {
        Mirror { pkg, with_kotlin: HashSet::new(), last_walk: None }
    }

    /// Brings the shadow up to date with the package (`open`: the editor's documents of it, original text by their
    /// canonical paths): every file on a full walk (the first, then at most every few seconds), else the open ones
    /// and the ones with Kotlin.
    pub fn update(&mut self, open: &HashMap<PathBuf, String>) -> Updated {
        let mut out = Updated::default();
        let pkg = &self.pkg;
        let _ = std::fs::create_dir_all(&pkg.shadow);
        if write_if_changed(&pkg.manifest_path(), pkg.manifest.as_bytes()) {
            out.written.push(pkg.manifest_path());
        }
        if let Ok(lock) =
            std::fs::read(pkg.workspace_root.join("Cargo.lock")).or_else(|_| std::fs::read(pkg.dir.join("Cargo.lock")))
        {
            // (cargo keeps it as it needs it: written once, not over its own edits)
            let shadow_lock = pkg.shadow.join("Cargo.lock");
            if !shadow_lock.exists() && write_if_changed(&shadow_lock, &lock) {
                out.written.push(shadow_lock);
            }
        }
        let full = self.last_walk.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(3));
        let mut rs_files: Vec<PathBuf> = Vec::new();
        let mut others: Vec<PathBuf> = Vec::new();
        if full {
            walk(&pkg.dir, &pkg.dir, &pkg.target_dir, &mut rs_files, &mut others);
            self.last_walk = Some(std::time::Instant::now());
        } else {
            let mut set: HashSet<PathBuf> = self.with_kotlin.clone();
            set.extend(
                open.keys().filter(|p| p.extension().is_some_and(|e| e == "rs") && p.starts_with(&pkg.dir)).cloned(),
            );
            rs_files.extend(set);
        }
        // (the module table with the open documents' Kotlin, as the server translates with)
        let mut open_sources = HashMap::new();
        for (p, text) in open {
            if !p.starts_with(&pkg.dir) {
                continue;
            }
            let norm = Normalized::new(text).text;
            let kt_file = p.extension().is_some_and(|e| e == "kt");
            let blocks = if kt_file {
                vec![(norm, Pos::start(1, 1))]
            } else {
                module::kotlin_in(&norm)
                    .blocks
                    .into_iter()
                    .map(|b| (norm[b.clone()].to_string(), module::pos_in(&norm, b.start)))
                    .collect()
            };
            open_sources.insert(p.clone(), workspace::OpenSource { kt_file, blocks });
        }
        let kt = |p: &Path| -> Option<String> {
            let k = workspace::key(p);
            open.get(&k).cloned().or_else(|| std::fs::read_to_string(&k).ok())
        };
        let mut produced: HashSet<PathBuf> = HashSet::new();
        for path in &rs_files {
            let Some(target) = pkg.shadow_of(path) else { continue };
            let Some(text) = open.get(path).cloned().or_else(|| std::fs::read_to_string(path).ok()) else { continue };
            let origin = Arc::new(Origin::new(path, text));
            let has_kotlin = {
                let k = module::kotlin_in(&origin.norm.text);
                !k.blocks.is_empty() || !k.kt_files.is_empty()
            };
            let content = if has_kotlin {
                self.with_kotlin.insert(path.clone());
                let table = workspace::table_for(&pkg.dir, path);
                let table = Arc::new(workspace::with_open(&table, &open_sources));
                match shadow_source(pkg, origin.clone(), Some(table), &kt) {
                    Some(f) => {
                        let text = f.text.clone();
                        out.files.push(Arc::new(f));
                        text
                    }
                    None => origin.original.clone(),
                }
            } else {
                self.with_kotlin.remove(path);
                origin.original.clone()
            };
            if write_if_changed(&target, content.as_bytes()) {
                out.written.push(target.clone());
            }
            produced.insert(target);
        }
        if full {
            for path in &others {
                let Some(target) = pkg.shadow_of(path) else { continue };
                if std::fs::symlink_metadata(&target).is_err() {
                    if let Some(parent) = target.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    link(path, &target);
                }
                produced.insert(target);
            }
            // (what the package no longer has)
            let mut shadow_rs = Vec::new();
            let mut shadow_others = Vec::new();
            walk(&pkg.shadow, &pkg.shadow, &pkg.shadow.join("target"), &mut shadow_rs, &mut shadow_others);
            for p in shadow_rs.into_iter().chain(shadow_others) {
                let top = p
                    .strip_prefix(&pkg.shadow)
                    .ok()
                    .and_then(|r| r.components().next())
                    .map(|c| c.as_os_str().to_owned());
                let kept = top.is_some_and(|t| t == "Cargo.toml" || t == "Cargo.lock");
                if !kept && !produced.contains(&p) {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        out
    }
}

#[cfg(unix)]
fn link(from: &Path, to: &Path) {
    let _ = std::os::unix::fs::symlink(from, to);
}

#[cfg(not(unix))]
fn link(from: &Path, to: &Path) {
    let _ = std::fs::copy(from, to);
}

/// The package's files under `dir`: `.rs` sources and the others (`target/`, the target directory, hidden entries,
/// the manifest and lock file at the top left out).
fn walk(top: &Path, dir: &Path, target_dir: &Path, rs: &mut Vec<PathBuf>, others: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let p = e.path();
        let Ok(ft) = std::fs::symlink_metadata(&p).map(|m| m.file_type()) else { continue };
        if ft.is_dir() {
            if (dir == top && name == "target") || p == target_dir || p.starts_with(target_dir) {
                continue;
            }
            walk(top, &p, target_dir, rs, others);
        } else if dir == top && (name == "Cargo.toml" || name == "Cargo.lock") {
            continue;
        } else if name.ends_with(".rs") {
            rs.push(p);
        } else {
            others.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invocations_whole() {
        let src = "x;\n::kompoz :: kompoz ! {\n fun A() {}\n};\nkompoz_file!(\"a.kt\") ;\ny";
        let k = module::kotlin_in(src);
        let b = &k.blocks[0];
        let s = invocation_start(src, b.start - 1, "kompoz").unwrap();
        assert_eq!(&src[s..invocation_end(src, b.end)], "::kompoz :: kompoz ! {\n fun A() {}\n};");
        let (_, lit) = &k.kt_files[0];
        let open = skip_ws_back(src, lit.start) - 1;
        let s = invocation_start(src, open, "kompoz_file").unwrap();
        assert_eq!(&src[s..invocation_end(src, lit.end)], "kompoz_file!(\"a.kt\") ;");
    }

    #[test]
    fn paths_as_the_macro_finds_them() {
        let deps = Json::parse(r#"[{"name": "kompoz", "rename": "kz"}]"#).unwrap();
        assert_eq!(paths_of("app", deps.array()).runtime.to_string(), ":: kz :: runtime");
        let deps = Json::parse(r#"[{"name": "kompoz-runtime", "rename": null}]"#).unwrap();
        let p = paths_of("kompoz-ui", deps.array());
        assert_eq!(p.runtime.to_string(), ":: kompoz_runtime");
        assert_eq!(p.ui.to_string(), "crate");
    }

    #[test]
    fn a_manifest_of_its_own() {
        let pkg = Json::parse(
            r#"{"name": "app", "version": "0.1.0", "edition": "2024", "features": {"x": ["dep:serde"]},
                "targets": [{"kind": ["lib"], "crate_types": ["lib"], "name": "app", "src_path": "/p/src/lib.rs", "edition": "2024", "test": true, "doctest": true},
                            {"kind": ["test"], "crate_types": ["bin"], "name": "t", "src_path": "/p/tests/t.rs", "edition": "2024", "test": true}],
                "dependencies": [{"name": "kompoz", "source": null, "req": "*", "kind": null, "rename": null, "optional": false,
                                  "uses_default_features": true, "features": [], "target": null, "registry": null, "path": "/k/crates/kompoz"},
                                 {"name": "serde", "source": "registry+https://github.com/rust-lang/crates.io-index", "req": "^1", "kind": "dev",
                                  "rename": "sd", "optional": true, "uses_default_features": false, "features": ["derive"], "target": "cfg(unix)", "registry": null}]}"#,
        )
        .unwrap();
        let m = manifest(&pkg, Path::new("/p"), Path::new("/t/shadow/app"), Path::new("/nowhere"));
        assert!(
            m.contains("[lib]\nname = \"app\"\npath = \"/t/shadow/app/src/lib.rs\"\ncrate-type = [\"lib\"]\n"),
            "{m}"
        );
        assert!(m.contains("[[test]]\nname = \"t\"\npath = \"/t/shadow/app/tests/t.rs\"\n"), "{m}");
        assert!(m.contains("[dependencies]\n\"kompoz\" = { path = \"/k/crates/kompoz\" }"), "{m}");
        assert!(
            m.contains("[target.\"cfg(unix)\".dev-dependencies]\n\"sd\" = { package = \"serde\", version = \"^1\", features = [\"derive\"], default-features = false, optional = true }"),
            "{m}"
        );
        assert!(m.contains("[features]\n\"x\" = [\"dep:serde\"]\n"), "{m}");
        assert!(m.contains("build = false\n") && m.contains("\n[workspace]\n"), "{m}");
    }
}
