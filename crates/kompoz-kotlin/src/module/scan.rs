//! Finding a crate's sources and reading the module table from them (see the module's doc).
//!
//! The crate's root file is found as cargo names its targets' files: by the crate's name (`CARGO_CRATE_NAME`, the
//! target's name with `-` written `_`) among `tests/`, `examples/`, `benches/`, `src/bin/` and the `path`s the
//! manifest gives its targets, then `src/lib.rs` and `src/main.rs`; of several, the one whose module tree holds the
//! file the macro is called from (a package's library and binary of the same name). From the root the `mod`
//! declarations are followed as rustc follows them (`name.rs` or `name/mod.rs` beside a root or `mod.rs` file, in
//! the directory of the file's name beside another, an inline `mod x { … }` a directory deeper, `#[path = "…"]`);
//! `#[cfg]`s are not evaluated (every module is read). Without a root every `.rs` file under `src/` is read, each a
//! module of its own.
//!
//! The files are read with a small Rust lexer (comments, string, raw string, byte and C string literals, character
//! literals and lifetimes, raw identifiers, balanced delimiters) — not `syn`, which would lose the text as written
//! and fail on a file it can't parse — finding the `mod` declarations, the `kompoz!` invocations (their group's
//! text verbatim, at its line and column) and the `kompoz_file!("…")` ones; each file's `pub` items are read by
//! [`rust_items`]. A file is read as rustc reads it: no byte order mark, LF line ends.

use super::{ModuleSource, ModuleTable, rust_items};
use crate::pos::Pos;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

/// The module table of the crate whose manifest is in `manifest_dir`, named `crate_name` (cargo's
/// `CARGO_CRATE_NAME`), a macro of it called from `call_file` (rustc's path of the file, relative to the manifest's
/// directory or to the working directory, or absolute) when known.
pub fn scan(manifest_dir: &Path, crate_name: Option<&str>, call_file: Option<&Path>) -> ModuleTable {
    let call = call_file.and_then(|f| resolve_call_file(manifest_dir, f));
    scan_crate(manifest_dir, crate_name, call.as_deref()).table
}

/// [`scan`]'s table, kept for the process and made again when a file it read (or looked for) changed: one per
/// crate root, the one of the crate whose tree holds `call_file`. Each call checks the files' modification times
/// and lengths (a file now missing, or now there, is a change too).
pub fn cached(manifest_dir: &Path, crate_name: Option<&str>, call_file: Option<&Path>) -> Arc<ModuleTable> {
    let call = call_file.and_then(|f| resolve_call_file(manifest_dir, f));
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    // (an entry of this crate whose tree holds the call's file, or whose scan saw that call: any entry without one)
    let hit = cache
        .iter()
        .find(|((dir, name, _), e)| {
            dir == manifest_dir
                && name.as_deref() == crate_name
                && call.as_ref().is_none_or(|c| e.files.contains(c) || e.calls.contains(c))
        })
        .map(|(key, _)| key.clone());
    if let Some(key) = &hit {
        let entry = &cache[key];
        if entry.stamps.iter().all(|(path, s)| stamp(path) == *s) {
            return entry.table.clone();
        }
        cache.remove(key);
    }
    let scanned = scan_crate(manifest_dir, crate_name, call.as_deref());
    let table = Arc::new(scanned.table);
    let key = (manifest_dir.to_path_buf(), crate_name.map(str::to_string), scanned.root);
    let entry = Entry {
        stamps: scanned.stamps,
        files: scanned.canonical,
        calls: call.into_iter().collect(),
        table: table.clone(),
    };
    cache.insert(key, entry);
    table
}

// ---- The cache -------------------------------------------------------------------------------------------------

/// A file's state when read: its modification time and length, `None` when it wasn't there.
type Stamp = Option<(Option<SystemTime>, u64)>;

fn stamp(path: &Path) -> Stamp {
    std::fs::metadata(path).ok().map(|m| (m.modified().ok(), m.len()))
}

/// (manifest directory, crate name, root file — `src/` when there was none)
type Key = (PathBuf, Option<String>, PathBuf);

struct Entry {
    /// Every file read or looked for (and, without a root, the directories listed), as it was.
    stamps: Vec<(PathBuf, Stamp)>,
    /// The `.rs` files of the tree, canonical: where a call's file is looked for.
    files: HashSet<PathBuf>,
    /// The calls' files that made this entry though not in its tree (a file `include!`d from elsewhere): found here
    /// again without a scan.
    calls: HashSet<PathBuf>,
    table: Arc<ModuleTable>,
}

static CACHE: LazyLock<Mutex<HashMap<Key, Entry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A call's file as a canonical path: rustc names it relative to its working directory (cargo's: the workspace's
/// root) or absolute; relative to the manifest's directory is tried too.
fn resolve_call_file(manifest_dir: &Path, file: &Path) -> Option<PathBuf> {
    let candidates = if file.is_absolute() {
        vec![file.to_path_buf()]
    } else {
        let cwd = std::env::current_dir().ok();
        cwd.map(|d| d.join(file)).into_iter().chain([manifest_dir.join(file)]).collect()
    };
    candidates.into_iter().find_map(|c| c.canonicalize().ok())
}

// ---- The crate's root ------------------------------------------------------------------------------------------

/// A scan's outcome: the table, the root it started from, what it read.
struct Scanned {
    table: ModuleTable,
    root: PathBuf,
    stamps: Vec<(PathBuf, Stamp)>,
    canonical: HashSet<PathBuf>,
}

fn scan_crate(manifest_dir: &Path, crate_name: Option<&str>, call: Option<&Path>) -> Scanned {
    let manifest = manifest_dir.join("Cargo.toml");
    let manifest_text = std::fs::read_to_string(&manifest).unwrap_or_default();
    let candidates = root_candidates(manifest_dir, &manifest_text, crate_name);
    let mut probes: Vec<(PathBuf, Stamp)> = vec![(manifest.clone(), stamp(&manifest))];
    probes.extend(candidates.iter().map(|c| (c.clone(), stamp(c))));
    let mut first: Option<Scanned> = None;
    for root in candidates.iter().filter(|c| c.is_file()) {
        let mut w = Walker::new(manifest_dir);
        w.root(root);
        let scanned = w.finish(root.clone(), &probes);
        if call.is_none_or(|c| scanned.canonical.contains(c)) {
            return scanned;
        }
        first.get_or_insert(scanned);
    }
    if let Some(s) = first {
        return s;
    }
    // (no root: every file under src/, each a module)
    let src = manifest_dir.join("src");
    let mut w = Walker::new(manifest_dir);
    let mut files = Vec::new();
    rs_files(&src, &mut files, &mut w.stamps);
    files.sort();
    for f in files {
        let module = fallback_module(&src, &f);
        w.file(&f, &module, true, false);
    }
    w.finish(src, &probes)
}

/// The `.rs` files under `dir` (its directories stamped: a file added changes their modification time).
fn rs_files(dir: &Path, out: &mut Vec<PathBuf>, stamps: &mut Vec<(PathBuf, Stamp)>) {
    stamps.push((dir.to_path_buf(), stamp(dir)));
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            rs_files(&path, out, stamps);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// A file's module when every file under `src/` is one: its path's (`a/b.rs` → `a::b`, `a/mod.rs` → `a`,
/// `lib.rs`/`main.rs` the root).
fn fallback_module(src: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(src).unwrap_or(file).with_extension("");
    let mut parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.last().is_some_and(|l| l == "mod") || parts.len() == 1 && (parts[0] == "lib" || parts[0] == "main") {
        parts.pop();
    }
    parts.join("::")
}

/// The files that can be the root of the crate named `crate_name`, in the order they are tried: the manifest's
/// targets' `path`s of that name, cargo's conventional places, `src/lib.rs`, `src/main.rs`.
fn root_candidates(manifest_dir: &Path, manifest: &str, crate_name: Option<&str>) -> Vec<PathBuf> {
    let norm = |s: &str| s.replace('-', "_");
    let wanted = crate_name.map(norm);
    let (package, targets) = cargo_targets(manifest);
    let mut out: Vec<PathBuf> = Vec::new();
    for t in &targets {
        let Some(path) = &t.path else { continue };
        // (a library is named after its package unless it says otherwise)
        let name = t.name.clone().or_else(|| (t.table == "lib").then(|| package.clone()).flatten());
        let matches = match (&wanted, &name) {
            (Some(w), Some(n)) => norm(n) == *w,
            (None, _) => t.table == "lib",
            (Some(_), None) => false,
        };
        if matches {
            out.push(manifest_dir.join(path));
        }
    }
    if let Some(n) = &wanted {
        let mut names = vec![n.clone()];
        if n.contains('_') {
            names.push(n.replace('_', "-"));
        }
        for name in &names {
            for place in [
                format!("tests/{name}.rs"),
                format!("tests/{name}/main.rs"),
                format!("examples/{name}.rs"),
                format!("examples/{name}/main.rs"),
                format!("benches/{name}.rs"),
                format!("src/bin/{name}.rs"),
                format!("src/bin/{name}/main.rs"),
            ] {
                out.push(manifest_dir.join(place));
            }
        }
    }
    out.push(manifest_dir.join("src/lib.rs"));
    out.push(manifest_dir.join("src/main.rs"));
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// A target table of the manifest: `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]`, `[[bench]]`.
struct Target {
    table: String,
    name: Option<String>,
    path: Option<String>,
}

/// The manifest's package name and target tables, read line by line (`key = "value"` lines of the tables; enough
/// for what cargo's manifests say of their targets).
fn cargo_targets(manifest: &str) -> (Option<String>, Vec<Target>) {
    let mut package = None;
    let mut targets: Vec<Target> = Vec::new();
    let mut table = String::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            let name = line.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            if ["lib", "bin", "test", "example", "bench"].contains(&name.as_str()) {
                targets.push(Target { table: name.clone(), name: None, path: None });
            }
            table = name;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        let Some(value) = value.strip_prefix('"').and_then(|v| v.split('"').next()) else { continue };
        let key = key.trim();
        if table == "package" && key == "name" {
            package = Some(value.to_string());
        } else if let Some(t) = targets.last_mut().filter(|t| t.table == table) {
            match key {
                "name" => t.name = Some(value.to_string()),
                "path" => t.path = Some(value.to_string()),
                _ => {}
            }
        }
    }
    (package, targets)
}

// ---- Walking the module tree -----------------------------------------------------------------------------------

/// Where a module's `mod` declarations look for their files.
#[derive(Clone)]
struct Ctx {
    /// The directory of the file the declaration is in (a `#[path]` outside an inline module is relative to it).
    file_dir: PathBuf,
    /// Where `mod name;` finds `name.rs`/`name/mod.rs` (an inline module's directory included).
    child_dir: PathBuf,
    /// Inside an inline `mod x { … }` (a `#[path]` is then relative to `child_dir`).
    inline: bool,
    module: String,
}

struct Walker<'a> {
    manifest_dir: &'a Path,
    table: ModuleTable,
    stamps: Vec<(PathBuf, Stamp)>,
    canonical: HashSet<PathBuf>,
    visited: HashSet<PathBuf>,
}

impl<'a> Walker<'a> {
    fn new(manifest_dir: &'a Path) -> Walker<'a> {
        Walker {
            manifest_dir,
            table: ModuleTable::default(),
            stamps: Vec::new(),
            canonical: HashSet::new(),
            visited: HashSet::new(),
        }
    }

    fn finish(mut self, root: PathBuf, probes: &[(PathBuf, Stamp)]) -> Scanned {
        self.stamps.extend(probes.iter().cloned());
        Scanned { table: self.table, root, stamps: self.stamps, canonical: self.canonical }
    }

    fn root(&mut self, root: &Path) {
        self.file(root, "", true, true);
    }

    /// Reads a module's file: its items, its Kotlin sources, and (`follow`) its `mod` declarations' files. A
    /// `mod_rs` file (a root, a `mod.rs`, one a `#[path]` names) has its submodules' files beside it; another
    /// (`name.rs`), in the directory `name/` beside it.
    fn file(&mut self, path: &Path, module: &str, mod_rs: bool, follow: bool) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if !self.visited.insert(canonical.clone()) {
            return;
        }
        self.stamps.push((path.to_path_buf(), stamp(path)));
        let Some(src) = read_source(path) else { return };
        self.table.files.push(path.to_path_buf());
        self.canonical.insert(canonical);
        self.table.rust.extend(rust_items(&src, module));
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let child_dir = if mod_rs {
            dir.clone()
        } else {
            dir.join(path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
        };
        let ctx = Ctx { file_dir: dir, child_dir, inline: false, module: module.to_string() };
        let toks = lex(&src);
        let close = matching(&toks);
        self.walk(path, &src, &toks, &close, 0, toks.len(), &ctx, follow);
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        path: &Path,
        src: &str,
        toks: &[Tok],
        close: &[usize],
        from: usize,
        to: usize,
        ctx: &Ctx,
        follow: bool,
    ) {
        let kind = |i: usize| toks.get(i).filter(|_| i < to).map(|t| &t.kind);
        let ident = |i: usize| match kind(i) {
            Some(TokKind::Ident) => Some(&src[toks[i].start..toks[i].end]),
            _ => None,
        };
        let mut path_attr: Option<String> = None;
        let mut i = from;
        while i < to {
            match &toks[i].kind {
                // (an attribute: `#[path = "…"]` kept for the `mod` it is on, the others passed over)
                TokKind::Punct('#') => {
                    let j = if kind(i + 1) == Some(&TokKind::Punct('!')) { i + 2 } else { i + 1 };
                    if kind(j) == Some(&TokKind::Open('[')) {
                        let c = close[j];
                        if ident(j + 1) == Some("path")
                            && kind(j + 2) == Some(&TokKind::Punct('='))
                            && j + 4 == c
                            && let Some(TokKind::Str(p)) = kind(j + 3)
                        {
                            path_attr = Some(p.clone());
                        }
                        i = c.saturating_add(1);
                        continue;
                    }
                }
                TokKind::Ident => match ident(i) {
                    Some("pub") => {
                        i += 1;
                        if kind(i) == Some(&TokKind::Open('(')) {
                            i = close[i].saturating_add(1);
                        }
                        continue;
                    }
                    Some("mod") if ident(i + 1).is_some() => {
                        let name = ident(i + 1).unwrap_or_default().trim_start_matches("r#").to_string();
                        let module =
                            if ctx.module.is_empty() { name.clone() } else { format!("{}::{name}", ctx.module) };
                        match kind(i + 2) {
                            Some(TokKind::Punct(';')) => {
                                if follow {
                                    self.module_file(ctx, &name, &module, path_attr.take());
                                }
                                i += 3;
                            }
                            Some(TokKind::Open('{')) => {
                                let c = close[i + 2];
                                let dir = path_attr.take().unwrap_or_else(|| name.clone());
                                let inner = Ctx {
                                    file_dir: ctx.file_dir.clone(),
                                    child_dir: ctx.child_dir.join(dir),
                                    inline: true,
                                    module,
                                };
                                self.walk(path, src, toks, close, i + 3, c.min(to), &inner, follow);
                                i = c.saturating_add(1);
                            }
                            _ => i += 2,
                        }
                        path_attr = None;
                        continue;
                    }
                    // (a macro's definition: what it expands to isn't an invocation here)
                    Some("macro_rules")
                        if kind(i + 1) == Some(&TokKind::Punct('!'))
                            && ident(i + 2).is_some()
                            && matches!(kind(i + 3), Some(TokKind::Open(_))) =>
                    {
                        i = close[i + 3].saturating_add(1);
                        path_attr = None;
                        continue;
                    }
                    Some("kompoz")
                        if kind(i + 1) == Some(&TokKind::Punct('!'))
                            && matches!(kind(i + 2), Some(TokKind::Open(_))) =>
                    {
                        let c = close[i + 2];
                        if let Some(end) = toks.get(c).filter(|_| c < to) {
                            let start = toks[i + 2].end;
                            self.table.sources.push(ModuleSource {
                                file: path.to_path_buf(),
                                text: src[start..end.start].to_string(),
                                base: pos_at(src, start),
                                kt_file: false,
                            });
                        }
                        i = c.saturating_add(1);
                        path_attr = None;
                        continue;
                    }
                    Some("kompoz_file")
                        if kind(i + 1) == Some(&TokKind::Punct('!'))
                            && matches!(kind(i + 2), Some(TokKind::Open(_))) =>
                    {
                        let c = close[i + 2];
                        if c == i + 4
                            && let Some(TokKind::Str(name)) = kind(i + 3)
                        {
                            self.kt_file(name);
                        }
                        i = c.saturating_add(1);
                        path_attr = None;
                        continue;
                    }
                    _ => {}
                },
                _ => {}
            }
            path_attr = None;
            i += 1;
        }
    }

    /// A `mod name;`'s file: a `#[path]`'s (relative to the file's directory, or inside an inline module to its
    /// directory), else `name.rs` or `name/mod.rs` where the module's submodules are.
    fn module_file(&mut self, ctx: &Ctx, name: &str, module: &str, path_attr: Option<String>) {
        if let Some(p) = path_attr {
            let base = if ctx.inline { &ctx.child_dir } else { &ctx.file_dir };
            let file = base.join(p);
            self.file(&file, module, true, true);
            return;
        }
        let flat = ctx.child_dir.join(format!("{name}.rs"));
        let nested = ctx.child_dir.join(name).join("mod.rs");
        if flat.is_file() {
            self.stamps.push((nested.clone(), stamp(&nested)));
            self.file(&flat, module, false, true);
        } else {
            self.stamps.push((flat.clone(), stamp(&flat)));
            self.file(&nested, module, true, true);
        }
    }

    /// A `kompoz_file!("…")`'s `.kt` file, relative to the manifest's directory.
    fn kt_file(&mut self, name: &str) {
        let path = self.manifest_dir.join(name);
        self.stamps.push((path.clone(), stamp(&path)));
        let Some(text) = read_source(&path) else { return };
        if !self.table.files.contains(&path) {
            self.table.files.push(path.clone());
        }
        self.table.sources.push(ModuleSource { file: path, text, base: Pos::start(1, 1), kt_file: true });
    }
}

/// A source file's text as rustc reads it: no byte order mark, LF line ends.
fn read_source(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    Some(content.strip_prefix('\u{feff}').unwrap_or(&content).replace("\r\n", "\n"))
}

/// The line and column (characters, 1-based) of a byte offset of `src`.
fn pos_at(src: &str, offset: usize) -> Pos {
    let before = &src[..offset];
    let line_start = before.rfind('\n').map_or(0, |n| n + 1);
    let line = before.matches('\n').count() as u32 + 1;
    let column = before[line_start..].chars().count() as u32 + 1;
    Pos::start(line, column)
}

// ---- One file's Kotlin, for an editor ----------------------------------------------------------------------------

/// The Kotlin of one Rust source, found as the scan finds it (see [`kotlin_in`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RustFileKotlin {
    /// Each `kompoz!` invocation's text: byte offsets from just after its opening delimiter to just before its
    /// closing one (what the macro reads), in order.
    pub blocks: Vec<std::ops::Range<usize>>,
    /// Each `kompoz_file!("…")`: the `.kt` file's path as written (relative to the crate's directory), and the
    /// byte range of its string literal.
    pub kt_files: Vec<(String, std::ops::Range<usize>)>,
}

/// The `kompoz!` blocks and `kompoz_file!` calls of a Rust source's text (as rustc reads it: no byte order mark, LF
/// line ends) — the scan's own reading, so a language server sees exactly the Kotlin the macros and the module table
/// see: comments, strings and `macro_rules!` definitions don't hold invocations, an unclosed block isn't one. The
/// line and column of an offset are [`pos_in`]'s.
pub fn kotlin_in(src: &str) -> RustFileKotlin {
    let toks = lex(src);
    let close = matching(&toks);
    let kind = |i: usize| toks.get(i).map(|t| &t.kind);
    let ident = |i: usize| match kind(i) {
        Some(TokKind::Ident) => Some(&src[toks[i].start..toks[i].end]),
        _ => None,
    };
    let mut out = RustFileKotlin::default();
    let mut i = 0;
    while i < toks.len() {
        let bang_group = kind(i + 1) == Some(&TokKind::Punct('!')) && matches!(kind(i + 2), Some(TokKind::Open(_)));
        match ident(i) {
            Some("macro_rules") if ident(i + 2).is_some() && kind(i + 1) == Some(&TokKind::Punct('!')) => {
                if matches!(kind(i + 3), Some(TokKind::Open(_))) {
                    i = close[i + 3].saturating_add(1);
                    continue;
                }
            }
            Some("kompoz") if bang_group => {
                // (closed by its own kind of delimiter, the others not counted: Kotlin being written — `f((x)` —
                // is still the block's, its mistake the Kotlin's to report)
                let Some(TokKind::Open(open)) = kind(i + 2) else { unreachable!() };
                let closer = match open {
                    '{' => '}',
                    '(' => ')',
                    _ => ']',
                };
                let mut depth = 0usize;
                let mut c = None;
                for (j, t) in toks.iter().enumerate().skip(i + 2) {
                    match t.kind {
                        TokKind::Open(o) if o == *open => depth += 1,
                        TokKind::Close if src[t.start..].starts_with(closer) => {
                            depth -= 1;
                            if depth == 0 {
                                c = Some(j);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Some(c) = c else { break };
                out.blocks.push(toks[i + 2].end..toks[c].start);
                i = c + 1;
                continue;
            }
            Some("kompoz_file") if bang_group => {
                let c = close[i + 2];
                if c == i + 4
                    && let Some(TokKind::Str(name)) = kind(i + 3)
                {
                    out.kt_files.push((name.clone(), toks[i + 3].start..toks[i + 3].end));
                }
                i = c.saturating_add(1);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The line and column (characters, 1-based) of a byte offset of `src` (offset 0: a translation's base, as the
/// scan gives a `kompoz!` block's).
pub fn pos_in(src: &str, offset: usize) -> Pos {
    pos_at(src, offset)
}

// ---- The lexer -------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum TokKind {
    /// A name or keyword (a raw one with its `r#`).
    Ident,
    Punct(char),
    Open(char),
    Close,
    /// A string literal (normal, raw, byte, C), with its value.
    Str(String),
    /// A number or character literal.
    Literal,
    Lifetime,
}

/// A token: what it is, its byte offsets in the text.
#[derive(Clone, Debug)]
struct Tok {
    kind: TokKind,
    start: usize,
    end: usize,
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// The tokens of a Rust text (comments, doc comments included, left out).
fn lex(src: &str) -> Vec<Tok> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let n = chars.len();
    let at = |k: usize| chars.get(k).map_or('\0', |c| c.1);
    let pos = |k: usize| chars.get(k).map_or(src.len(), |c| c.0);
    // (a string literal from its opening quote at `q`: its end and value)
    let string = |q: usize| {
        let mut k = q + 1;
        let mut value = String::new();
        while k < n && at(k) != '"' {
            if at(k) == '\\' {
                k += 1;
                match at(k) {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '0' => value.push('\0'),
                    '\n' => {
                        while k + 1 < n && at(k + 1).is_whitespace() {
                            k += 1;
                        }
                    }
                    c => value.push(c),
                }
            } else {
                value.push(at(k));
            }
            k += 1;
        }
        (k + 1, value)
    };
    // (a raw string from its first `#` or quote at `h`, when it is one: its end and value)
    let raw = |h: usize| {
        let mut k = h;
        while at(k) == '#' {
            k += 1;
        }
        if at(k) != '"' {
            return None;
        }
        let hashes = k - h;
        let open = k + 1;
        let mut j = open;
        while j < n {
            if at(j) == '"' && (1..=hashes).all(|d| at(j + d) == '#') {
                return Some((j + 1 + hashes, src[pos(open)..pos(j)].to_string()));
            }
            j += 1;
        }
        Some((n, src[pos(open)..].to_string()))
    };
    // (a character literal or a lifetime from its quote at `q`)
    let quote = |q: usize| {
        if at(q + 1) == '\\' {
            let mut k = q + 3;
            while k < n && at(k) != '\'' {
                k += 1;
            }
            (k + 1, TokKind::Literal)
        } else if at(q + 2) == '\'' && at(q + 1) != '\'' {
            (q + 3, TokKind::Literal)
        } else {
            let mut k = q + 1;
            while k < n && is_ident_continue(at(k)) {
                k += 1;
            }
            if k == q + 1 { (k, TokKind::Punct('\'')) } else { (k, TokKind::Lifetime) }
        }
    };
    let suffix = |mut k: usize| {
        while k < n && is_ident_continue(at(k)) {
            k += 1;
        }
        k
    };
    let mut out = Vec::new();
    let mut k = 0;
    while k < n {
        let c = at(k);
        let start = k;
        let (end, kind) = if c.is_whitespace() {
            k += 1;
            continue;
        } else if c == '/' && at(k + 1) == '/' {
            while k < n && at(k) != '\n' {
                k += 1;
            }
            continue;
        } else if c == '/' && at(k + 1) == '*' {
            let mut depth = 1;
            k += 2;
            while k < n && depth > 0 {
                if at(k) == '/' && at(k + 1) == '*' {
                    depth += 1;
                    k += 2;
                } else if at(k) == '*' && at(k + 1) == '/' {
                    depth -= 1;
                    k += 2;
                } else {
                    k += 1;
                }
            }
            continue;
        } else if is_ident_start(c) {
            let mut e = k;
            while e < n && is_ident_continue(at(e)) {
                e += 1;
            }
            let word = &src[pos(k)..pos(e)];
            let raw_string = if matches!(word, "r" | "br" | "cr") { raw(e) } else { None };
            if let Some((end, value)) = raw_string {
                (suffix(end), TokKind::Str(value))
            } else if matches!(word, "b" | "c") && at(e) == '"' {
                let (end, value) = string(e);
                (suffix(end), TokKind::Str(value))
            } else if word == "b" && at(e) == '\'' {
                let (end, _) = quote(e);
                (end, TokKind::Literal)
            } else if word == "r" && at(e) == '#' && is_ident_start(at(e + 1)) {
                let mut j = e + 1;
                while j < n && is_ident_continue(at(j)) {
                    j += 1;
                }
                (j, TokKind::Ident)
            } else {
                (e, TokKind::Ident)
            }
        } else if c.is_ascii_digit() {
            let mut e = k;
            while e < n && (is_ident_continue(at(e)) || at(e) == '.' && at(e + 1).is_ascii_digit()) {
                e += 1;
            }
            (e, TokKind::Literal)
        } else if c == '"' {
            let (end, value) = string(k);
            (suffix(end), TokKind::Str(value))
        } else if c == '\'' {
            quote(k)
        } else {
            let kind = match c {
                '(' | '[' | '{' => TokKind::Open(c),
                ')' | ']' | '}' => TokKind::Close,
                _ => TokKind::Punct(c),
            };
            (k + 1, kind)
        };
        let end = end.min(n);
        out.push(Tok { kind, start: pos(start), end: pos(end) });
        k = end;
    }
    out
}

/// For each opening delimiter its closing one's index (`usize::MAX` for one never closed, and for the other
/// tokens).
fn matching(toks: &[Tok]) -> Vec<usize> {
    let mut close = vec![usize::MAX; toks.len()];
    let mut stack = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t.kind {
            TokKind::Open(_) => stack.push(i),
            TokKind::Close => {
                if let Some(o) = stack.pop() {
                    close[o] = i;
                }
            }
            _ => {}
        }
    }
    close
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Kind;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A crate written under the temporary directory, removed after.
    struct TempCrate(PathBuf);

    impl TempCrate {
        fn new(files: &[(&str, &str)]) -> TempCrate {
            static N: AtomicUsize = AtomicUsize::new(0);
            let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let dir = std::env::temp_dir().join(format!(
                "kompoz-scan-{}-{}-{nanos}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let c = TempCrate(dir);
            for (path, text) in files {
                c.write(path, text);
            }
            c
        }

        fn write(&self, path: &str, text: &str) {
            let p = self.0.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    }

    impl Drop for TempCrate {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const MANIFEST: &str = "[package]\nname = \"demo-app\"\nversion = \"0.1.0\"\n";

    fn rust_names(t: &ModuleTable) -> Vec<(String, String)> {
        let mut v: Vec<_> = t.rust.iter().map(|i| (i.module.clone(), i.rust.clone())).collect();
        v.sort();
        v
    }

    #[test]
    fn test_target_root_and_its_modules() {
        let c = TempCrate::new(&[
            ("Cargo.toml", MANIFEST),
            ("src/lib.rs", "pub fn in_lib() {}"),
            (
                "tests/screens.rs",
                "mod common;\nmod a { mod b; }\n#[path = \"other/x.rs\"]\nmod renamed;\npub fn root_fn() {}",
            ),
            ("tests/common/mod.rs", "pub fn helper() {}\nmod inner;"),
            ("tests/common/inner.rs", "pub fn deep() {}"),
            ("tests/a/b.rs", "pub fn in_b() {}"),
            ("tests/other/x.rs", "pub fn in_x() {}\nmod y;"),
            ("tests/other/y.rs", "pub fn in_y() {}"),
        ]);
        let t = scan(&c.0, Some("screens"), None);
        assert_eq!(
            rust_names(&t),
            [
                ("".into(), "root_fn".into()),
                ("a::b".into(), "in_b".into()),
                ("common".into(), "helper".into()),
                ("common::inner".into(), "deep".into()),
                ("renamed".into(), "in_x".into()),
                ("renamed::y".into(), "in_y".into()),
            ]
        );
        assert_eq!(t.files.len(), 6);
        // (a lib's crate name: src/lib.rs)
        let t = scan(&c.0, Some("demo_app"), None);
        assert_eq!(rust_names(&t), [("".into(), "in_lib".into())]);
    }

    #[test]
    fn root_by_the_calling_file_and_manifest_paths() {
        let c = TempCrate::new(&[
            ("Cargo.toml", "[package]\nname = \"tool\"\n\n[[bin]]\nname = \"tool\"\npath = \"app/start.rs\"\n"),
            ("src/lib.rs", "pub fn lib_fn() {}"),
            ("app/start.rs", "mod ui;"),
            ("app/ui.rs", "pub fn ui_fn() {}"),
        ]);
        let from_bin = scan(&c.0, Some("tool"), Some(Path::new("app/ui.rs")));
        assert_eq!(rust_names(&from_bin), [("ui".into(), "ui_fn".into())]);
        let from_lib = scan(&c.0, Some("tool"), Some(&c.0.join("src/lib.rs")));
        assert_eq!(rust_names(&from_lib), [("".into(), "lib_fn".into())]);
    }

    #[test]
    fn no_root_reads_every_file() {
        let c = TempCrate::new(&[
            ("Cargo.toml", MANIFEST),
            ("src/a/mod.rs", "pub fn fa() {}"),
            ("src/b.rs", "pub fn fb() {}"),
        ]);
        let t = scan(&c.0, Some("elsewhere"), None);
        assert_eq!(rust_names(&t), [("a".into(), "fa".into()), ("b".into(), "fb".into())]);
    }

    #[test]
    fn kompoz_blocks_verbatim() {
        let rs = concat!(
            "use kompoz::kompoz;\n",
            "// kompoz! { commented out }\n",
            "/* kompoz! { /* nested */ also out } */\n",
            "macro_rules! kompoz { ($($t:tt)*) => {} }\n",
            "pub fn kompoz() {}\n",
            "const S: &str = \"kompoz! { in a string }\";\n",
            "const R: &str = r#\"kompoz! { \"raw\" }\"#;\n",
            "fn f<'a>(x: &'a str) -> char { '}' }\n",
            "kompoz! {\n",
            "    fun A() { val s = \"}\"; val c = '}' } // }\n",
            "}\n",
            "/* çğ */ ::kompoz::kompoz!(fun B() {})\n",
            "kompoz![x]\n",
        );
        let c = TempCrate::new(&[("Cargo.toml", MANIFEST), ("src/lib.rs", rs)]);
        let t = scan(&c.0, Some("demo_app"), None);
        let got: Vec<_> = t.sources.iter().map(|s| (s.text.as_str(), s.base.line, s.base.column, s.kt_file)).collect();
        assert_eq!(
            got,
            [
                ("\n    fun A() { val s = \"}\"; val c = '}' } // }\n", 9, 10, false),
                ("fun B() {}", 12, 28, false),
                ("x", 13, 9, false),
            ]
        );
        assert!(t.sources.iter().all(|s| s.base.offset == 0 && s.file == c.0.join("src/lib.rs")));
    }

    #[test]
    fn kompoz_file_reads_the_kt() {
        let c = TempCrate::new(&[
            ("Cargo.toml", MANIFEST),
            ("src/main.rs", "kompoz_file!(\"src/ui.kt\");\n// kompoz_file!(\"src/gone.kt\");\nfn main() {}"),
            ("src/ui.kt", "\u{feff}fun Ui() {\r\n}\r\n"),
        ]);
        let t = scan(&c.0, Some("demo_app"), None);
        assert_eq!(t.sources.len(), 1);
        let s = &t.sources[0];
        assert_eq!((s.text.as_str(), s.base, s.kt_file), ("fun Ui() {\n}\n", Pos::start(1, 1), true));
        assert_eq!(t.files, [c.0.join("src/main.rs"), c.0.join("src/ui.kt")]);
    }

    #[test]
    fn crlf_and_bom_in_rust_files() {
        let c =
            TempCrate::new(&[("Cargo.toml", MANIFEST), ("src/lib.rs", "\u{feff}// x\r\nkompoz! {\r\n  a\r\n}\r\n")]);
        let t = scan(&c.0, Some("demo_app"), None);
        let s = &t.sources[0];
        assert_eq!((s.text.as_str(), s.base.line, s.base.column), ("\n  a\n", 2, 10));
    }

    #[test]
    fn one_files_kotlin_for_an_editor() {
        let src = "macro_rules! m { () => { kompoz! { no } } }\n// kompoz! { no }\nconst S: &str = \"kompoz! { no }\";\n\
                   kompoz! {\n    fun A() { f((x) }\n}\nkompoz_file!(\"src/ui.kt\");\nkompoz::kompoz!(fun B())\nkompoz! { open";
        let found = kotlin_in(src);
        let texts: Vec<&str> = found.blocks.iter().map(|b| &src[b.clone()]).collect();
        assert_eq!(texts, ["\n    fun A() { f((x) }\n", "fun B()"]);
        assert_eq!(found.kt_files.len(), 1);
        assert_eq!((found.kt_files[0].0.as_str(), &src[found.kt_files[0].1.clone()]), ("src/ui.kt", "\"src/ui.kt\""));
        assert_eq!(pos_in(src, found.blocks[0].start), Pos::start(4, 10));
    }

    #[test]
    fn lexer_tokens() {
        let src = "r#mod b'x' br#\"q\"# c\"z\" 'a 'b' '\\n' 1.5f32 x..y /* /* */ */ \"a\\\"b\"";
        let kinds: Vec<_> = lex(src).into_iter().map(|t| (t.kind, &src[t.start..t.end])).collect();
        assert_eq!(
            kinds,
            [
                (TokKind::Ident, "r#mod"),
                (TokKind::Literal, "b'x'"),
                (TokKind::Str("q".into()), "br#\"q\"#"),
                (TokKind::Str("z".into()), "c\"z\""),
                (TokKind::Lifetime, "'a"),
                (TokKind::Literal, "'b'"),
                (TokKind::Literal, "'\\n'"),
                (TokKind::Literal, "1.5f32"),
                (TokKind::Ident, "x"),
                (TokKind::Punct('.'), "."),
                (TokKind::Punct('.'), "."),
                (TokKind::Ident, "y"),
                (TokKind::Str("a\"b".into()), "\"a\\\"b\""),
            ]
        );
    }

    #[test]
    fn cache_sees_changes() {
        let c = TempCrate::new(&[("Cargo.toml", MANIFEST), ("src/lib.rs", "mod m;"), ("src/m.rs", "pub fn one() {}")]);
        let first = cached(&c.0, Some("demo_app"), None);
        let again = cached(&c.0, Some("demo_app"), None);
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(first.rust[0].rust, "one");
        c.write("src/m.rs", "pub fn second() {}");
        let changed = cached(&c.0, Some("demo_app"), None);
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(changed.rust[0].rust, "second");
        // (a module's file appearing: `mod n;` found once it is there)
        c.write("src/lib.rs", "mod m;\nmod n;");
        assert_eq!(cached(&c.0, Some("demo_app"), None).rust.len(), 1);
        c.write("src/n/mod.rs", "pub const N: u8 = 1;");
        let added = cached(&c.0, Some("demo_app"), None);
        assert!(added.rust.iter().any(|i| i.rust == "N" && i.kind == Kind::Value && i.module == "n"));
    }
}
