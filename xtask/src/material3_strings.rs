//! `cargo xtask material3-strings [--check]`: material3's texts in every language Compose for desktop translates
//! them to (CMP material3-desktop 1.11.0-alpha07's skikoMain: `internal/Strings.skiko.kt`'s ids and
//! `l10n/*.kt`'s translations, `l10n/Translations.kt`'s locale tags), converted to Rust data into
//! `crates/kompoz-material3/src/internal/l10n/` (committed): `mod.rs` with the ids (`Strings::NavigationMenu`…) and
//! `translation_for(tag)`, a file per Kotlin file, each translation an array of the texts by id.
//!
//! It reads the sources jar from Gradle's cache; `--check` converts and compares where the jar is, and says it
//! skipped where it isn't (CI).

use crate::compose_l10n::{Spec, convert};
use crate::jar::Jar;
use std::path::Path;

const OUT: &str = "crates/kompoz-material3/src/internal/l10n";
const GROUP: &str = "org.jetbrains.compose.material3";
const ARTIFACT: &str = "material3-desktop";
const VERSION: &str = "1.11.0-alpha07";

/// material3's texts (the converter is shared with `adaptive-strings`: crate::compose_l10n).
const SPEC: Spec = Spec {
    task: "material3-strings",
    strings: "skikoMain/androidx/compose/material3/internal/Strings.skiko.kt",
    l10n: "skikoMain/androidx/compose/material3/l10n/",
    what: "material3's texts",
    module_source: "CMP material3-desktop 1.11.0-alpha07's skikoMain (internal/Strings.skiko.kt, l10n/*.kt)",
    file_source: "Compose for desktop's material3 1.11.0-alpha07",
};

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let Some(jar) = Jar::find(GROUP, ARTIFACT, VERSION, "-sources.jar") else {
        if check {
            println!("material3-strings: skipped (no {GROUP}:{ARTIFACT}:{VERSION} sources jar in Gradle's cache)");
            return Ok(());
        }
        return Err(format!("no {GROUP}:{ARTIFACT}:{VERSION} sources jar in Gradle's cache"));
    };
    let jar = Jar::open(&jar)?;
    let files = convert(&jar, &SPEC)?;
    crate::write_generated(root, OUT, &files, check, "material3-strings")
}
