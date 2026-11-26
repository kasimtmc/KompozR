//! An open document: its text as the editor has it, where its Kotlin is (a `.rs` file's `kompoz!` blocks, a `.kt`
//! file whole: its regions), and what the generator made of each region (its diagnostics and analysis).
//!
//! Offsets: a region is a stretch of the document's [`Normalized`] text (what rustc reads, which the Kotlin front end's
//! offsets are into); [`Doc::doc_offset`] takes a region's Kotlin offset back to the editor's text, whose
//! [`Lines`] give LSP positions.

use crate::json::Json;
use crate::text::{Lines, Normalized};
use crate::workspace;
use kompoz_kotlin::analysis::{Analysis, Declaration};
use kompoz_kotlin::module::ModuleTable;
use kompoz_kotlin::{Diagnostic, OneSpan, Options, Pos};
use std::path::PathBuf;
use std::sync::Arc;

/// What kind of document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// A `.rs` file: rust-analyzer's, with Kotlin in its `kompoz!` blocks.
    Rust,
    /// A `.kt` file: Kotlin throughout, the server's alone.
    Kotlin,
    /// Anything else: rust-analyzer's (a `Cargo.toml`, …).
    Other,
}

/// A stretch of Kotlin in a document.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    /// Its byte offsets in the normalized text.
    pub start: usize,
    pub end: usize,
    /// Its first character's line and column in the file (offset 0: the Kotlin's offsets are the region's).
    pub base: Pos,
}

/// What the generator made of a region.
#[derive(Clone, Debug, Default)]
pub struct Translated {
    pub diagnostics: Vec<Diagnostic>,
    pub analysis: Analysis,
    /// Its declarations with its classes' constructor properties ([`kompoz_kotlin::analysis::declarations_of`]).
    pub declarations: Vec<Declaration>,
}

/// A declaration of the module table's other sources (or of this document's other regions), where it is.
#[derive(Clone, Debug)]
pub struct ModuleDecl {
    pub file: PathBuf,
    pub decl: Declaration,
}

pub struct Doc {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub lang: Lang,
    pub text: String,
    pub version: i64,
    pub norm: Normalized,
    pub lines: Lines,
    pub regions: Vec<Region>,
    /// The package it is in (its Kotlin is translated with its crate's module table; none: not a crate's).
    pub manifest: Option<PathBuf>,
    /// The regions' translations (one each, in order), as of the last [`Doc::translate`].
    pub translated: Vec<Translated>,
    /// The module table's declarations as of the last translation.
    pub module_decls: Arc<Vec<ModuleDecl>>,
    /// The text changed since the last translation.
    pub dirty: bool,
    /// rust-analyzer's latest diagnostics of it.
    pub analyzer_diagnostics: Json,
    /// Given to rust-analyzer (`didOpen` sent).
    pub in_analyzer: bool,
}

impl Doc {
    pub fn new(uri: &str, text: String, version: i64) -> Doc {
        let path = uri_to_path(uri);
        let lang = match path.as_ref().and_then(|p| p.extension()).and_then(|e| e.to_str()) {
            Some("rs") => Lang::Rust,
            Some("kt") => Lang::Kotlin,
            _ => Lang::Other,
        };
        let manifest = path.as_deref().and_then(workspace::manifest_dir);
        let mut d = Doc {
            uri: uri.to_string(),
            path,
            lang,
            text,
            version,
            norm: Normalized::default(),
            lines: Lines::default(),
            regions: Vec::new(),
            manifest,
            translated: Vec::new(),
            module_decls: Arc::new(Vec::new()),
            dirty: true,
            analyzer_diagnostics: Json::Array(Vec::new()),
            in_analyzer: false,
        };
        d.refresh();
        d
    }

    /// The text changed: its lines and regions again (its translations due).
    pub fn refresh(&mut self) {
        self.norm = Normalized::new(&self.text);
        self.lines = Lines::new(&self.text);
        self.regions = match self.lang {
            Lang::Kotlin => vec![Region { start: 0, end: self.norm.text.len(), base: Pos::start(1, 1) }],
            Lang::Rust => kompoz_kotlin::module::kotlin_in(&self.norm.text)
                .blocks
                .into_iter()
                .map(|b| Region {
                    start: b.start,
                    end: b.end,
                    base: kompoz_kotlin::module::pos_in(&self.norm.text, b.start),
                })
                .collect(),
            Lang::Other => Vec::new(),
        };
        self.dirty = true;
    }

    pub fn has_kotlin(&self) -> bool {
        !self.regions.is_empty()
    }

    /// The region's Kotlin text.
    pub fn kotlin(&self, region: usize) -> &str {
        let r = &self.regions[region];
        &self.norm.text[r.start..r.end]
    }

    /// The region a document offset is in (its ends included), and the Kotlin offset there.
    pub fn region_at(&self, offset: usize) -> Option<(usize, usize)> {
        let n = self.norm.to_normalized(offset);
        self.regions.iter().position(|r| r.start <= n && n <= r.end).map(|i| (i, n - self.regions[i].start))
    }

    pub fn in_kotlin(&self, offset: usize) -> bool {
        self.region_at(offset).is_some()
    }

    /// Whether an LSP position is in the Kotlin.
    pub fn position_in_kotlin(&self, position: &Json) -> bool {
        self.in_kotlin(self.lines.offset_of(&self.text, position))
    }

    /// The document offset of a region's Kotlin offset.
    pub fn doc_offset(&self, region: usize, kotlin: usize) -> usize {
        self.norm.to_original(self.regions[region].start + kotlin)
    }

    /// An LSP range of a region's Kotlin stretch.
    pub fn range(&self, region: usize, loc: kompoz_kotlin::Loc) -> Json {
        let from = self.doc_offset(region, loc.start.offset);
        let to = self.doc_offset(region, loc.end.offset);
        self.lines.range(&self.text, from, to)
    }

    /// The translation options of this document's Kotlin.
    pub fn options(&self, table: Option<Arc<ModuleTable>>) -> Options {
        Options { keyed_calls: self.lang == Lang::Kotlin, module: table, analyze: true, ..Default::default() }
    }

    /// Translates every region with the module table (none outside a crate).
    pub fn translate(&mut self, table: Option<Arc<ModuleTable>>) {
        let opts = self.options(table.clone());
        let mut out = Vec::with_capacity(self.regions.len());
        for r in &self.regions {
            let text = &self.norm.text[r.start..r.end];
            let t = kompoz_kotlin::translate(text, r.base, &OneSpan::default(), &opts);
            let declarations = kompoz_kotlin::analysis::declarations_of(text, r.base);
            out.push(Translated { diagnostics: t.diagnostics, analysis: t.analysis.unwrap_or_default(), declarations });
        }
        self.translated = out;
        self.module_decls = Arc::new(module_declarations(table.as_deref()));
        self.dirty = false;
    }

    /// The generator's diagnostics as LSP's (none for a document outside a crate: its Kotlin isn't KompozR's).
    pub fn kotlin_diagnostics(&self) -> Vec<Json> {
        if self.manifest.is_none() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (i, t) in self.translated.iter().enumerate() {
            for d in &t.diagnostics {
                let mut loc = d.loc;
                if loc.end.offset <= loc.start.offset {
                    // (a place: its character)
                    let text = self.kotlin(i);
                    let next = text[loc.start.offset.min(text.len())..].chars().next().map_or(0, char::len_utf8);
                    loc.end.offset = (loc.start.offset + next).min(text.len());
                }
                let mut j = Json::object([
                    ("range", self.range(i, loc)),
                    ("severity", Json::from(if d.is_error() { 1 } else { 2 })),
                    ("source", "kompoz".into()),
                    ("message", d.message.clone().into()),
                ]);
                let related: Vec<Json> = d
                    .notes
                    .iter()
                    .filter_map(|n| {
                        let at = n.loc?;
                        Some(Json::object([
                            (
                                "location",
                                Json::object([("uri", self.uri.clone().into()), ("range", self.range(i, at))]),
                            ),
                            ("message", n.message.clone().into()),
                        ]))
                    })
                    .collect();
                let unplaced: Vec<&str> =
                    d.notes.iter().filter(|n| n.loc.is_none()).map(|n| n.message.as_str()).collect();
                if !unplaced.is_empty() {
                    j.set("message", format!("{}\n{}", d.message, unplaced.join("\n")));
                }
                if !related.is_empty() {
                    j.set("relatedInformation", related);
                }
                out.push(j);
            }
        }
        out
    }
}

/// The declarations of a module table's Kotlin sources.
pub fn module_declarations(table: Option<&ModuleTable>) -> Vec<ModuleDecl> {
    let Some(t) = table else { return Vec::new() };
    t.sources
        .iter()
        .flat_map(|s| {
            kompoz_kotlin::analysis::declarations_of(&s.text, s.base)
                .into_iter()
                .map(|decl| ModuleDecl { file: s.file.clone(), decl })
        })
        .collect()
}

/// A `file:` URI's path (percent-decoded); none for another scheme.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("x"), 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    let mut s = String::from_utf8(out).ok()?;
    // (Windows: file:///C:/…)
    if s.len() > 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[2] == b':' {
        s.remove(0);
    }
    Some(PathBuf::from(s))
}

/// A path's `file:` URI (percent-encoded as editors write them).
pub fn path_to_uri(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else if b == b':' && out.len() <= 10 {
            out.push(':');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_and_paths() {
        let p = uri_to_path("file:///home/a%20b/x%C3%A7.rs").unwrap();
        assert_eq!(p, PathBuf::from("/home/a b/xç.rs"));
        assert_eq!(path_to_uri(&p), "file:///home/a%20b/x%C3%A7.rs");
        assert_eq!(uri_to_path("untitled:1"), None);
    }

    #[test]
    fn regions_of_a_rust_file() {
        let text = "fn a() {}\r\nkompoz! {\r\n    fun B() {}\r\n}\r\n// kompoz! { not }\r\n".to_string();
        let d = Doc::new("file:///nowhere/x.rs", text, 1);
        assert_eq!(d.regions.len(), 1);
        assert_eq!(d.kotlin(0), "\n    fun B() {}\n");
        assert_eq!(d.regions[0].base, Pos::start(2, 10));
        let b = d.text.find("B()").unwrap();
        let (r, k) = d.region_at(b).unwrap();
        assert_eq!((r, &d.kotlin(0)[k..k + 1]), (0, "B"));
        assert_eq!(d.doc_offset(0, k), b);
        assert!(!d.in_kotlin(0));
    }
}
