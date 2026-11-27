//! What an app's build script adds to its program for the platforms (decision D9/§2: our own helper, no build crate):
//! on Windows its resources ([`windows_resources`]) — the program's icon (Explorer's, the taskbar's before a window
//! sets its own, the window class's: resource id 1), its manifest (per-monitor DPI awareness v2 before the program
//! starts, UTF-8 as the process's code page, Windows 10 and 11 as the systems it is made for, long paths, run as the
//! user who starts it) and its version information — compiled by the toolchain's resource compiler (MinGW's
//! `windres` or LLVM's for the GNU target; the Windows SDK's `rc.exe` or `llvm-rc` for MSVC's) and linked into the
//! program's binaries. Nothing for the other targets.
//!
//! ```text
//! // build.rs, with `kompoz-platform = { path = "…", default-features = false }` among the build-dependencies
//! // (the crate without its platforms: built in seconds)
//! fn main() {
//!     kompoz_platform::build::windows_resources(&WindowsResources {
//!         name: "Sayaç",
//!         icon: &["icon.png"],
//!     });
//! }
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

/// What a Windows program is told about itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsResources<'a> {
    /// Its name (its version information's product name and description: what Explorer and the Task Manager show).
    pub name: &'a str,
    /// Its icon's pictures: PNG files (a size each, 256 × 256 at most; Windows picks the one it needs), relative to
    /// the package's directory; or one `.ico` file. Empty: Windows' application icon.
    pub icon: &'a [&'a str],
}

/// The program's resources (see the module), when the target is Windows: written into the build's `OUT_DIR`,
/// compiled, linked into the package's binaries. Panics (failing the build, with what to install) where it can't.
pub fn windows_resources(resources: &WindowsResources) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR (a build script's)"));
    let package = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let mut lines = vec!["#pragma code_page(65001)".to_string()];
    // The icon.
    if !resources.icon.is_empty() {
        let paths: Vec<PathBuf> = resources.icon.iter().map(|p| package.join(p)).collect();
        for p in &paths {
            println!("cargo:rerun-if-changed={}", p.display());
        }
        let ico = if paths.len() == 1 && paths[0].extension().is_some_and(|e| e.eq_ignore_ascii_case("ico")) {
            paths[0].clone()
        } else {
            let pngs: Vec<Vec<u8>> = paths
                .iter()
                .map(|p| std::fs::read(p).unwrap_or_else(|e| panic!("the icon {}: {e}", p.display())))
                .collect();
            let ico = out.join("icon.ico");
            let bytes = ico_of_pngs(&pngs).unwrap_or_else(|why| panic!("the icon: {why}"));
            std::fs::write(&ico, bytes).expect("icon.ico written");
            ico
        };
        lines.push(format!("1 ICON \"{}\"", rc_path(&ico)));
    }
    // The manifest.
    let manifest = out.join("app.manifest");
    std::fs::write(&manifest, MANIFEST).expect("app.manifest written");
    lines.push(format!("1 24 \"{}\"", rc_path(&manifest)));
    // The version information.
    lines.push(version_info(resources.name, &version));
    let rc = out.join("resources.rc");
    std::fs::write(&rc, lines.join("\n") + "\n").expect("resources.rc written");
    let compiled = if msvc { compile_msvc(&rc, &out) } else { compile_gnu(&rc, &out) };
    println!("cargo:rustc-link-arg-bins={}", compiled.display());
    if msvc {
        // (The manifest is the resource's: the linker makes none of its own beside it.)
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
    println!("cargo:rerun-if-env-changed=RC");
    println!("cargo:rerun-if-env-changed=WINDRES");
}

/// The manifest: per-monitor DPI awareness v2 (Windows 10 1703; per-monitor before that, system-aware before 8.1),
/// UTF-8 as the active code page (1903), long paths (1607), Windows 10 and 11 as the supported systems, run as the
/// invoker.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;

/// A path as a resource script's string (forward slashes: both compilers take them; quotes can't be in a path).
fn rc_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// A string as a resource script's (its quotes doubled).
fn rc_string(s: &str) -> String {
    s.replace('"', "\"\"")
}

/// The version information (VS_VERSIONINFO) of program `name`, version `version` ("1.2.3": its numbers, 0 for
/// those missing).
fn version_info(name: &str, version: &str) -> String {
    let mut n: Vec<u16> = version
        .split(['.', '-', '+'])
        .take(3)
        .map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0))
        .collect();
    n.resize(4, 0);
    let numbers = format!("{},{},{},{}", n[0], n[1], n[2], n[3]);
    let name = rc_string(name);
    let version = rc_string(version);
    format!(
        "1 VERSIONINFO\nFILEVERSION {numbers}\nPRODUCTVERSION {numbers}\nFILEOS 0x40004\nFILETYPE 0x1\n\
         BEGIN\n  BLOCK \"StringFileInfo\"\n  BEGIN\n    BLOCK \"040904B0\"\n    BEGIN\n\
         \x20     VALUE \"FileDescription\", \"{name}\"\n      VALUE \"ProductName\", \"{name}\"\n\
         \x20     VALUE \"FileVersion\", \"{version}\"\n      VALUE \"ProductVersion\", \"{version}\"\n\
         \x20   END\n  END\n  BLOCK \"VarFileInfo\"\n  BEGIN\n    VALUE \"Translation\", 0x409, 1200\n  END\nEND"
    )
}

/// PNG pictures as an icon file (Windows Vista's: each entry a PNG as it is), the smallest first.
pub fn ico_of_pngs(pngs: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut images: Vec<(u32, u32, &[u8])> = Vec::new();
    for png in pngs {
        if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
            return Err("not a PNG".into());
        }
        let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        if w == 0 || h == 0 || w > 256 || h > 256 {
            return Err(format!("a {w} × {h} picture (256 × 256 at most)"));
        }
        images.push((w, h, png));
    }
    images.sort_by_key(|(w, _, _)| *w);
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]); // (reserved, an icon)
    ico.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (w, h, png) in &images {
        // (256 is written as 0)
        ico.extend_from_slice(&[(*w % 256) as u8, (*h % 256) as u8, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits a pixel
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, _, png) in &images {
        ico.extend_from_slice(png);
    }
    Ok(ico)
}

/// Runs `tool` with `args`; whether it ran and succeeded (its failure's output on the build's stderr).
fn run(tool: &Path, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let output = Command::new(tool).args(args).output().map_err(|e| format!("{}: {e}", tool.display()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{} failed ({}):\n{}{}",
        tool.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// The resources as a COFF object for the GNU target: `WINDRES`, else MinGW's windres for the target
/// (`x86_64-w64-mingw32-windres`), `windres`, LLVM's `llvm-windres`.
fn compile_gnu(rc: &Path, out: &Path) -> PathBuf {
    let object = out.join("resources.o");
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".into());
    let mut tools: Vec<PathBuf> = std::env::var_os("WINDRES").map(PathBuf::from).into_iter().collect();
    tools.extend([format!("{arch}-w64-mingw32-windres"), "windres".into(), "llvm-windres".into()].map(PathBuf::from));
    let mut failures = Vec::new();
    for tool in &tools {
        let target = if arch == "x86_64" { "pe-x86-64" } else { "pe-i386" };
        let args = [
            "--input-format=rc".as_ref(),
            "--output-format=coff".as_ref(),
            "--target".as_ref(),
            target.as_ref(),
            "-c".as_ref(),
            "65001".as_ref(),
            rc.as_os_str(),
            object.as_os_str(),
        ];
        match run(tool, &args) {
            Ok(()) => return object,
            Err(why) => failures.push(why),
        }
    }
    panic!(
        "the program's Windows resources couldn't be compiled: no resource compiler worked (MinGW-w64's binutils' \
         windres, or LLVM's llvm-windres; WINDRES names another):\n{}",
        failures.join("\n")
    );
}

/// The resources as a .res for MSVC's linker: `RC`, else `rc.exe` (on the PATH, or the newest Windows SDK's),
/// LLVM's `llvm-rc`.
fn compile_msvc(rc: &Path, out: &Path) -> PathBuf {
    let res = out.join("resources.res");
    let mut tools: Vec<PathBuf> = std::env::var_os("RC").map(PathBuf::from).into_iter().collect();
    tools.push("rc.exe".into());
    tools.extend(windows_sdk_rc());
    tools.push("llvm-rc".into());
    let mut failures = Vec::new();
    for tool in &tools {
        let fo = format!("/fo{}", res.display());
        let args = ["/nologo".as_ref(), fo.as_ref(), rc.as_os_str()];
        match run(tool, &args) {
            Ok(()) => return res,
            Err(why) => failures.push(why),
        }
    }
    panic!(
        "the program's Windows resources couldn't be compiled: no resource compiler worked (the Windows SDK's \
         rc.exe, or LLVM's llvm-rc; RC names another):\n{}",
        failures.join("\n")
    );
}

/// The Windows SDK's rc.exe for the host's architecture, the newest SDK's first (Windows Kits 10's bin\<version>).
fn windows_sdk_rc() -> Vec<PathBuf> {
    let roots = [std::env::var_os("ProgramFiles(x86)"), std::env::var_os("ProgramFiles")];
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    let mut found: Vec<PathBuf> = Vec::new();
    for root in roots.into_iter().flatten() {
        let bin = Path::new(&root).join("Windows Kits").join("10").join("bin");
        let Ok(entries) = std::fs::read_dir(&bin) else { continue };
        let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        versions.sort();
        found.extend(versions.iter().rev().map(|v| v.join(arch).join("rc.exe")).filter(|p| p.is_file()));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut p = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        p.extend_from_slice(&w.to_be_bytes());
        p.extend_from_slice(&h.to_be_bytes());
        p.extend_from_slice(&[8, 6, 0, 0, 0]);
        p
    }

    #[test]
    fn icons_hold_their_pngs_smallest_first() {
        let (big, small) = (png(256, 256), png(16, 16));
        let ico = ico_of_pngs(&[big.clone(), small.clone()]).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        // The first entry: 16 × 16, its PNG after the directory.
        assert_eq!(&ico[6..8], &[16, 16]);
        let offset = u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize;
        assert_eq!(offset, 6 + 32);
        assert_eq!(&ico[offset..offset + small.len()], small.as_slice());
        // The second: 256 written as 0.
        assert_eq!(&ico[22..24], &[0, 0]);
        assert!(ico_of_pngs(&[png(512, 512)]).is_err());
        assert!(ico_of_pngs(&[b"GIF89a".to_vec()]).is_err());
    }

    #[test]
    fn version_information_has_the_name_and_numbers() {
        let v = version_info("Kompoz \"Showcase\"", "0.1.0");
        assert!(v.contains("FILEVERSION 0,1,0,0"));
        assert!(v.contains("VALUE \"ProductName\", \"Kompoz \"\"Showcase\"\"\""));
        assert!(version_info("a", "2.10.3-beta").contains("PRODUCTVERSION 2,10,3,0"));
    }
}
