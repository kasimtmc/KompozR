//! The Kotlin Counter's Windows resources (the Rust Counter's icon, its manifest: per-monitor DPI, UTF-8; its
//! version); nothing elsewhere.

use kompoz_platform::build::{WindowsResources, windows_resources};

fn main() {
    windows_resources(&WindowsResources {
        name: "Sayaç (Kotlin)",
        icon: &[
            "../counter/windows/icon-16.png",
            "../counter/windows/icon-32.png",
            "../counter/windows/icon-48.png",
            "../counter/windows/icon-256.png",
        ],
    });
}
