//! KompozR's development tasks, run as `cargo xtask <task>` (the alias in `.cargo/config.toml`):
//!
//! ```text
//! cargo xtask shaders            compile crates/kompoz-gpu/shaders/*.slang and write what they compile to
//! cargo xtask shaders --check    compile them and fail if what is committed differs (CI)
//! cargo xtask wayland            generate the Wayland protocols' Rust from their XML
//! cargo xtask wayland --check    generate it and fail if what is committed differs (CI)
//! cargo xtask kotlin-index         write the API index (the macros' and the Kotlin syntax's) from the sources
//! cargo xtask kotlin-index --check build it and fail if what is committed differs (CI)
//! cargo xtask material3-tokens   convert material3's design tokens (its sources jar's tokens/*.kt) to Rust
//! cargo xtask material3-strings  convert material3's texts (Compose for desktop's l10n) to Rust data
//! cargo xtask adaptive-strings   convert material3-adaptive's layout texts (Compose for desktop's l10n) likewise
//! cargo xtask material-icons     convert the material icons' Kotlin (core's) to their crate's data
//!                                (each of the four with --check: convert and compare where the jar is, else skip)
//! cargo xtask showcase-shots compare|reference …
//!                                the showcase's shots against the C++ Kompoz showcase's: builds the showcase
//!                                (release) and runs its showcase-shots with the rest (examples/showcase, shots.rs)
//! cargo xtask web-gpu-test [--firefox] [--gpu] [--release] [names…]
//!                                kompoz-gpu's GPU tests in a headless browser on WebGPU (Chrome's SwiftShader, the
//!                                GPU with --gpu, or Firefox), built for the web on its nightly (web_gpu_test.rs)
//! ```
//!
//! Std, and syn to read Rust sources (the workspace's own, already built for its macros): building the workspace
//! needs none of the tools a task runs, only running it does.

// (the API index's text format, shared with kompoz-macros, which reads it)
mod adaptive_strings;
#[path = "../../crates/kompoz-macros/src/api_index.rs"]
mod api_index;
mod compose_l10n;
mod jar;
mod kotlin_index;
mod material3_strings;
mod material3_tokens;
mod material_icons;
mod sha256;
mod shaders;
mod wayland;
mod web_gpu_test;
mod xml;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask shaders [--check] | cargo xtask wayland [--check] | cargo xtask kotlin-index \
                     [--check] | cargo xtask material3-tokens [--check] | cargo xtask material3-strings [--check] | \
                     cargo xtask adaptive-strings [--check] | cargo xtask material-icons [--check] | cargo xtask \
                     showcase-shots compare [--showcase <binary>] [--out <dir>] | cargo xtask showcase-shots reference \
                     --cpp <binary> --revision <commit> | cargo xtask web-gpu-test [--firefox] [--gpu] [--release] \
                     [names…]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["shaders"] => shaders::run(&workspace(), false),
        ["shaders", "--check"] => shaders::run(&workspace(), true),
        ["wayland"] => wayland::run(&workspace(), false),
        ["wayland", "--check"] => wayland::run(&workspace(), true),
        ["kotlin-index"] => kotlin_index::run(&workspace(), false),
        ["kotlin-index", "--check"] => kotlin_index::run(&workspace(), true),
        ["material3-tokens"] => material3_tokens::run(&workspace(), false),
        ["material3-tokens", "--check"] => material3_tokens::run(&workspace(), true),
        ["material3-strings"] => material3_strings::run(&workspace(), false),
        ["material3-strings", "--check"] => material3_strings::run(&workspace(), true),
        ["adaptive-strings"] => adaptive_strings::run(&workspace(), false),
        ["adaptive-strings", "--check"] => adaptive_strings::run(&workspace(), true),
        ["material-icons"] => material_icons::run(&workspace(), false),
        ["material-icons", "--check"] => material_icons::run(&workspace(), true),
        ["showcase-shots", rest @ ..] => showcase_shots(rest),
        ["web-gpu-test", rest @ ..] => web_gpu_test::run(&workspace(), rest),
        ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask showcase-shots …`: the showcase and its showcase-shots built (release: lavapipe draws hundreds of
/// pages) and run with `args`.
fn showcase_shots(args: &[&str]) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    // (both binaries: compare runs the showcase beside showcase-shots)
    let built = std::process::Command::new(&cargo)
        .current_dir(workspace())
        .args(["build", "--release", "-p", "showcase", "--bins"])
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if !built.success() {
        return Err(format!("building the showcase: {built}"));
    }
    let status = std::process::Command::new(cargo)
        .current_dir(workspace())
        .args(["run", "--release", "-p", "showcase", "--bin", "showcase-shots", "--"])
        .args(args)
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("showcase-shots: {status}")) }
}

/// The workspace's root (this package is its `xtask/`).
fn workspace() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(PathBuf::from).unwrap_or(manifest)
}

/// Writes a generator's files (name → text) into `dir` (relative to `root`), deleting the files there it no longer
/// makes; with `check`, only compares and fails on a difference.
fn write_generated(
    root: &std::path::Path,
    dir: &str,
    files: &std::collections::BTreeMap<String, String>,
    check: bool,
    task: &str,
) -> Result<(), String> {
    use std::fs;
    let full = root.join(dir);
    let existing: Vec<String> = fs::read_dir(&full)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    let stale: Vec<&String> = existing.iter().filter(|n| !files.contains_key(*n)).collect();
    let changed: Vec<&String> = files
        .iter()
        .filter(|(n, t)| fs::read_to_string(full.join(n)).ok().as_deref() != Some(t.as_str()))
        .map(|(n, _)| n)
        .collect();
    if check {
        if stale.is_empty() && changed.is_empty() {
            println!("{task}: up to date");
            return Ok(());
        }
        for n in changed.iter().chain(&stale) {
            eprintln!("out of date: {dir}/{n}");
        }
        return Err(format!("run `cargo xtask {task}` and commit what it writes"));
    }
    fs::create_dir_all(&full).map_err(|e| format!("{}: {e}", full.display()))?;
    for n in &stale {
        fs::remove_file(full.join(n)).map_err(|e| format!("{dir}/{n}: {e}"))?;
        println!("deleted {dir}/{n}");
    }
    for n in &changed {
        fs::write(full.join(n), &files[*n]).map_err(|e| format!("{dir}/{n}: {e}"))?;
        println!("wrote {dir}/{n}");
    }
    Ok(())
}
