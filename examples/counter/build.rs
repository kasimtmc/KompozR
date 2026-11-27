//! The Counter's Windows resources (its icon, its manifest: per-monitor DPI, UTF-8; its version); nothing elsewhere.

use kompoz_platform::build::{WindowsResources, windows_resources};

fn main() {
    windows_resources(&WindowsResources {
        name: "Sayaç",
        icon: &["windows/icon-16.png", "windows/icon-32.png", "windows/icon-48.png", "windows/icon-256.png"],
    });
}
