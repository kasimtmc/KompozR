//! `cargo xtask adaptive-strings [--check]`: material3-adaptive's layout texts (the panes' titles, the pane
//! expansion drag handle's and drag-to-resize descriptions) in every language Compose for desktop translates them to
//! (CMP adaptive-layout-desktop 1.2.0's skikoMain: `layout/internal/Strings.skiko.kt`'s ids, `l10n/*.kt`'s
//! translations and `l10n/Translations.kt`'s locale tags), converted to Rust data into
//! `crates/kompoz-material3-adaptive/src/layout/internal/l10n/` (committed) by the converter `material3-strings`
//! uses (crate::compose_l10n).
//!
//! Its sources jar isn't in Gradle's cache: it is read from `~/.cache/compose-src/cmp-adaptive-1.2.0/`; `--check`
//! converts and compares where the jar is, and says it skipped where it isn't (CI).

use crate::compose_l10n::{Spec, convert};
use crate::jar::Jar;
use std::path::{Path, PathBuf};

const OUT: &str = "crates/kompoz-material3-adaptive/src/layout/internal/l10n";
/// The sources jar, under the home directory.
const JAR: &str = ".cache/compose-src/cmp-adaptive-1.2.0/adaptive-layout-desktop-1.2.0-sources.jar";

const SPEC: Spec = Spec {
    task: "adaptive-strings",
    strings: "skikoMain/androidx/compose/material3/adaptive/layout/internal/Strings.skiko.kt",
    l10n: "skikoMain/androidx/compose/material3/adaptive/l10n/",
    what: "material3-adaptive's layout texts",
    module_source: "CMP adaptive-layout-desktop 1.2.0's skikoMain (layout/internal/Strings.skiko.kt, l10n/*.kt)",
    file_source: "Compose for desktop's adaptive-layout 1.2.0",
};

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let jar = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(JAR)).filter(|p| p.is_file());
    let Some(jar) = jar else {
        if check {
            println!("adaptive-strings: skipped (no ~/{JAR})");
            return Ok(());
        }
        return Err(format!("no ~/{JAR}"));
    };
    let jar = Jar::open(&jar)?;
    let files = convert(&jar, &SPEC)?;
    crate::write_generated(root, OUT, &files, check, "adaptive-strings")
}
