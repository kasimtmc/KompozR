//! KompozR's development tasks, run as `cargo xtask <task>` (the alias in `.cargo/config.toml`):
//!
//! ```text
//! cargo xtask shaders            compile crates/kompoz-gpu/shaders/*.slang and write what they compile to
//! cargo xtask shaders --check    compile them and fail if what is committed differs (CI)
//! cargo xtask wayland            generate the Wayland protocols' Rust from their XML
//! cargo xtask wayland --check    generate it and fail if what is committed differs (CI)
//! ```
//!
//! Only std: building the workspace needs none of the tools a task runs, only running it does.

mod sha256;
mod shaders;
mod wayland;
mod xml;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask shaders [--check] | cargo xtask wayland [--check]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["shaders"] => shaders::run(&workspace(), false),
        ["shaders", "--check"] => shaders::run(&workspace(), true),
        ["wayland"] => wayland::run(&workspace(), false),
        ["wayland", "--check"] => wayland::run(&workspace(), true),
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

/// The workspace's root (this package is its `xtask/`).
fn workspace() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(PathBuf::from).unwrap_or(manifest)
}
