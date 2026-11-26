//! Which crate a document is in, and the module table its Kotlin is translated with: the one the macros see
//! ([`kompoz_kotlin::module::cached`], found from the package's manifest and the crate the file belongs to), with
//! the documents the editor has open read as the editor has them (unsaved text included) instead of from disk.

use kompoz_kotlin::module::{self, ModuleSource, ModuleTable};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The package a file is in: the nearest directory above it with a `Cargo.toml` holding a `[package]`.
pub fn manifest_dir(file: &Path) -> Option<PathBuf> {
    let mut dir = file.parent();
    while let Some(d) = dir {
        let manifest = d.join("Cargo.toml");
        if let Ok(text) = std::fs::read_to_string(&manifest)
            && text.lines().any(|l| l.trim() == "[package]")
        {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// The crate a `.rs` file of the package is a source of, as cargo names its targets (`tests/x.rs`,
/// `tests/x/…`, `examples/x.rs`, `benches/x.rs`, `src/bin/x.rs` → `x`; `None`: the library or the main binary,
/// which the module table tells apart by the file).
pub fn crate_name(manifest_dir: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(manifest_dir).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let target = match parts.as_slice() {
        [dir, name] if ["tests", "examples", "benches"].contains(&dir.as_str()) => name.strip_suffix(".rs")?,
        [dir, name, ..] if ["tests", "examples", "benches"].contains(&dir.as_str()) => name.as_str(),
        [src, bin, name] if src == "src" && bin == "bin" => name.strip_suffix(".rs")?,
        [src, bin, name, ..] if src == "src" && bin == "bin" => name.as_str(),
        _ => return None,
    };
    Some(target.replace('-', "_"))
}

/// A path as the tables are compared by (canonical when it exists).
pub fn key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The module table the Kotlin of `file` (a `.rs` source or a `.kt` file) is translated with, as the macros' scan
/// finds it. A `.kt` file is in the table of the crate whose sources read it (`kompoz_file!`): the library's or main
/// binary's, else one of the package's other targets.
pub fn table_for(manifest_dir: &Path, file: &Path) -> Arc<ModuleTable> {
    let is_kt = file.extension().is_some_and(|e| e == "kt");
    if !is_kt {
        let name = crate_name(manifest_dir, file);
        return module::cached(manifest_dir, name.as_deref(), Some(file));
    }
    let wanted = key(file);
    let has = |t: &ModuleTable| t.sources.iter().any(|s| s.kt_file && key(&s.file) == wanted);
    let first = module::cached(manifest_dir, None, None);
    if has(&first) {
        return first;
    }
    for dir in ["tests", "examples", "benches", "src/bin"] {
        let Ok(entries) = std::fs::read_dir(manifest_dir.join(dir)) else { continue };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.is_dir() && p.join("main.rs").is_file() {
                    p.file_name().map(|n| n.to_string_lossy().into_owned())
                } else if p.extension().is_some_and(|x| x == "rs") {
                    p.file_stem().map(|n| n.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        for n in names {
            let t = module::cached(manifest_dir, Some(&n.replace('-', "_")), None);
            if has(&t) {
                return t;
            }
        }
    }
    first
}

/// What an open document holds for the table: a `.rs` file's `kompoz!` blocks (text and base), a `.kt` file's text.
pub struct OpenSource {
    pub kt_file: bool,
    pub blocks: Vec<(String, kompoz_kotlin::Pos)>,
}

/// `table` with the open documents' Kotlin (by their [`key`]) instead of what their files on disk hold: a document of
/// the table's files only (another crate's open file isn't this crate's source).
pub fn with_open(table: &ModuleTable, open: &HashMap<PathBuf, OpenSource>) -> ModuleTable {
    if open.is_empty() {
        return table.clone();
    }
    let files: Vec<PathBuf> = table.files.iter().map(|f| key(f)).collect();
    let mut out = ModuleTable { sources: Vec::new(), rust: table.rust.clone(), files: table.files.clone() };
    let mut replaced: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for s in &table.sources {
        let k = key(&s.file);
        match open.get(&k) {
            Some(o) => {
                if replaced.insert(k) {
                    for (text, base) in &o.blocks {
                        out.sources.push(ModuleSource {
                            file: s.file.clone(),
                            text: text.clone(),
                            base: *base,
                            kt_file: o.kt_file,
                        });
                    }
                }
            }
            None => out.sources.push(s.clone()),
        }
    }
    // (an open file of the table that had no Kotlin on disk, and has now)
    for (path, o) in open {
        if files.contains(path) && !replaced.contains(path) && !o.kt_file {
            for (text, base) in &o.blocks {
                out.sources.push(ModuleSource { file: path.clone(), text: text.clone(), base: *base, kt_file: false });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crates_by_their_targets() {
        let m = Path::new("/p");
        assert_eq!(crate_name(m, Path::new("/p/tests/kotlin-ui.rs")).as_deref(), Some("kotlin_ui"));
        assert_eq!(crate_name(m, Path::new("/p/tests/big/screens.rs")).as_deref(), Some("big"));
        assert_eq!(crate_name(m, Path::new("/p/examples/demo.rs")).as_deref(), Some("demo"));
        assert_eq!(crate_name(m, Path::new("/p/src/bin/tool/main.rs")).as_deref(), Some("tool"));
        assert_eq!(crate_name(m, Path::new("/p/src/lib.rs")), None);
        assert_eq!(crate_name(m, Path::new("/p/src/ui/mod.rs")), None);
    }
}
