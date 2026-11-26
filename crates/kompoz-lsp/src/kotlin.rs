//! What the server answers inside the Kotlin, from what the generator knows of it (the regions' translations: their
//! [`Analysis`], their declarations, the module table's): hover, completion, definition, highlights, document
//! symbols, semantic tokens. rust-analyzer answers none of these there: its view of a `kompoz!` block is the macro's
//! expansion, not the Kotlin.

use crate::doc::{Doc, ModuleDecl};
use crate::json::Json;
use crate::tokens::{self, Names, SemanticToken};
use kompoz_kotlin::analysis::{Analysis, DeclKind, Declaration, LocalDecl, LocalKind, Target};
use kompoz_kotlin::index::{self, Item, Kind};
use kompoz_kotlin::module::ModuleTable;
use kompoz_kotlin::{Loc, OneSpan};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// The kinds of every declaration a document's Kotlin can name: its regions' and the module table's.
pub fn declaration_kinds(doc: &Doc) -> HashMap<String, DeclKind> {
    let mut out = HashMap::new();
    for m in doc.module_decls.iter() {
        out.insert(m.decl.kotlin.clone(), m.decl.kind);
    }
    for t in &doc.translated {
        for d in &t.declarations {
            out.insert(d.kotlin.clone(), d.kind);
        }
    }
    out
}

/// The Kotlin's semantic tokens, in the document's positions (a token over lines split, one a line).
pub fn semantic_tokens(doc: &Doc) -> Vec<SemanticToken> {
    let kinds = declaration_kinds(doc);
    let mut out = Vec::new();
    for (i, _) in doc.regions.iter().enumerate() {
        let names = Names { analysis: doc.translated.get(i).map(|t| &t.analysis), declarations: &kinds };
        let text = doc.kotlin(i);
        for t in tokens::kotlin_tokens(text, &names) {
            let (ty, modifiers) = tokens::legend_of(&t);
            let mut from = t.from;
            while from < t.to {
                let end = text[from..t.to].find('\n').map_or(t.to, |n| from + n);
                if end > from {
                    let a = doc.doc_offset(i, from);
                    let mut b = doc.doc_offset(i, end);
                    if b > a && doc.text.as_bytes().get(b - 1) == Some(&b'\r') {
                        b -= 1;
                    }
                    let (line, start) = doc.lines.position(&doc.text, a);
                    let (_, stop) = doc.lines.position(&doc.text, b);
                    if stop > start {
                        out.push(SemanticToken {
                            line: line as u32,
                            start: start as u32,
                            length: (stop - start) as u32,
                            ty,
                            modifiers,
                        });
                    }
                }
                from = end + 1;
            }
        }
    }
    out
}

// ---- Describing what a name is ---------------------------------------------------------------------------------

fn fenced(code: &str) -> String {
    format!("```kotlin\n{code}\n```")
}

/// A local's line: `val shown: String`.
fn local_line(l: &LocalDecl) -> String {
    let keyword = match l.kind {
        LocalKind::Var => "var ",
        LocalKind::Val => "val ",
        LocalKind::Param | LocalKind::LambdaParam => "",
    };
    if l.ty.is_empty() { format!("{keyword}{}", l.name) } else { format!("{keyword}{}: {}", l.name, l.ty) }
}

/// An API item as Kotlin calls it (its types Rust's, as the API writes them).
pub fn item_signature(i: &Item) -> String {
    let owner = if i.owner.is_empty() { String::new() } else { format!("{}.", index::kotlin_type(i.owner)) };
    let ret = if i.ret.is_empty() || i.ret == "()" { String::new() } else { format!(": {}", i.ret) };
    let params = || {
        i.params
            .iter()
            .map(|p| {
                let vararg = if p.vararg { "vararg " } else { "" };
                let default = match p.default {
                    index::Default::None => String::new(),
                    index::Default::Rust(d) | index::Default::Kotlin(d) => format!(" = {d}"),
                };
                format!("{vararg}{}: {}{default}", p.kotlin, p.ty)
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let composable = if i.has(index::flags::KOMPOZABLE) { "@Composable " } else { "" };
    let suspend = if i.has(index::flags::SUSPEND) { "suspend " } else { "" };
    match i.kind {
        Kind::Function | Kind::Method | Kind::Overload => {
            format!("{composable}{suspend}fun {owner}{}({}){ret}", i.kotlin, params())
        }
        Kind::FunInterface => format!("fun interface {}", i.kotlin),
        Kind::Property => format!("val {owner}{}{ret}", i.kotlin),
        Kind::Value if !i.owner.is_empty() && i.owner == i.rust => format!("class {}", i.kotlin),
        Kind::Value => format!("val {}{ret}", i.kotlin),
        Kind::Constant | Kind::Variant => {
            let args = if i.params.is_empty() { String::new() } else { format!("({})", params()) };
            format!("{owner}{}{args}{ret}", i.kotlin)
        }
    }
}

/// Where an API item is in Rust: `kompoz_foundation::layout::padding::Modifier::padding`.
pub fn item_path(krate: &str, module: &str, owner: &str, rust: &str) -> String {
    let mut parts: Vec<&str> = vec![krate];
    if !module.is_empty() {
        parts.push(module);
    }
    if !owner.is_empty() && owner != rust {
        parts.push(owner);
    }
    parts.push(rust);
    parts.join("::")
}

/// The API item a reference names.
pub fn api_item(kotlin: &str, rust: &str, krate: &str, module: &str, owner: &str) -> Option<&'static Item> {
    index::lookup(kotlin)
        .iter()
        .find(|i| i.rust == rust && i.krate == krate && i.module == module && i.owner == owner)
        .or_else(|| index::by_rust(rust).find(|i| i.krate == krate && i.module == module && i.owner == owner))
}

/// A declaration of the document or of the module table, by Kotlin's qualified name: (the region it's in, or the
/// module table's file), the declaration.
pub enum Found<'a> {
    Here(usize, &'a Declaration),
    Module(&'a ModuleDecl),
}

pub fn find_declaration<'a>(doc: &'a Doc, kotlin: &str) -> Option<Found<'a>> {
    for (i, t) in doc.translated.iter().enumerate() {
        if let Some(d) = t.declarations.iter().find(|d| d.kotlin == kotlin) {
            return Some(Found::Here(i, d));
        }
    }
    let here = doc.path.as_deref().map(crate::workspace::key);
    doc.module_decls
        .iter()
        .filter(|m| here.as_ref() != Some(&crate::workspace::key(&m.file)))
        .find(|m| m.decl.kotlin == kotlin)
        .map(Found::Module)
}

fn declaration_markdown(d: &Declaration, file: Option<&Path>) -> String {
    let mut s = fenced(&d.detail);
    s += &format!("\n\nRust: `{}`", d.rust);
    if let Some(f) = file.and_then(|f| f.file_name()) {
        s += &format!(" · {}", f.to_string_lossy());
    }
    s
}

/// The hover at a document offset in the Kotlin (none where the generator knows nothing there).
pub fn hover(doc: &Doc, offset: usize) -> Option<Json> {
    let (region, k) = doc.region_at(offset)?;
    let t = doc.translated.get(region)?;
    let a = &t.analysis;
    let ty = a.type_at(k);
    let (text, loc): (String, Loc) = if let Some(r) = a.reference_at(k) {
        let text = match &r.target {
            Target::Local(at) => {
                let l = a.locals.iter().find(|l| l.loc == *at)?;
                fenced(&local_line(l))
            }
            Target::Declaration(kotlin) => match find_declaration(doc, kotlin) {
                Some(Found::Here(_, d)) => declaration_markdown(d, None),
                Some(Found::Module(m)) => declaration_markdown(&m.decl, Some(&m.file)),
                None => fenced(kotlin),
            },
            Target::Api { kotlin, rust, krate, module, owner } => {
                let path = item_path(krate, module, owner, rust);
                match api_item(kotlin, rust, krate, module, owner) {
                    Some(i) => format!("{}\n\nRust: `{path}`", fenced(&item_signature(i))),
                    None => format!("{}\n\nRust: `{path}`", fenced(kotlin)),
                }
            }
        };
        let text = match ty {
            Some(t)
                if t.loc == r.loc
                    || t.loc.start.offset <= r.loc.start.offset && r.loc.end.offset <= t.loc.end.offset =>
            {
                if matches!(r.target, Target::Local(_)) { text } else { format!("{text}\n\nType: `{}`", t.ty) }
            }
            _ => text,
        };
        (text, r.loc)
    } else if let Some(local) = a.locals.iter().find(|l| l.loc.start.offset <= k && k <= l.loc.end.offset) {
        (fenced(&local_line(local)), local.loc)
    } else if let Some(d) = t.declarations.iter().find(|d| d.name.start.offset <= k && k <= d.name.end.offset) {
        (declaration_markdown(d, None), d.name)
    } else if let Some((found, loc)) = member_declaration(doc, region, k) {
        let text = match found {
            Found::Here(_, d) => declaration_markdown(d, None),
            Found::Module(m) => declaration_markdown(&m.decl, Some(&m.file)),
        };
        (text, loc)
    } else {
        let ty = ty?;
        (fenced(&ty.ty), ty.loc)
    };
    Some(Json::object([
        ("contents", Json::object([("kind", "markdown".into()), ("value", text.into())])),
        ("range", doc.range(region, loc)),
    ]))
}

/// The crate's declaration a member name at `k` (`receiver.name`) is of, by the receiver's type, and the name's
/// stretch: what a member read of the crate's own classes is (the generator records no reference there).
fn member_declaration(doc: &Doc, region: usize, k: usize) -> Option<(Found<'_>, Loc)> {
    let text = doc.kotlin(region);
    let k = crate::text::floor_char(text, k.min(text.len()));
    let is_part = |c: char| kompoz_kotlin::lexer::ident_part(c);
    let start = text[..k].char_indices().rev().take_while(|(_, c)| is_part(*c)).last().map_or(k, |(i, _)| i);
    let end = k + text[k..].char_indices().take_while(|(_, c)| is_part(*c)).last().map_or(0, |(i, c)| i + c.len_utf8());
    if start == end {
        return None;
    }
    let before = &text[..start];
    let receiver_end = before.strip_suffix("?.").or_else(|| before.strip_suffix('.'))?.trim_end().len();
    let a = &doc.translated.get(region)?.analysis;
    let ty = receiver_type(a, None, text, receiver_end)?;
    let found = find_declaration(doc, &format!("{}.{}", type_base(&ty), &text[start..end]))?;
    let at = |offset| kompoz_kotlin::Pos { offset, line: 0, column: 0 };
    Some((found, Loc::new(at(start), at(end))))
}

// ---- Definition ------------------------------------------------------------------------------------------------

/// Where a name in the Kotlin is declared.
pub enum Definition {
    /// A place: an LSP `Location`.
    Location(Json),
    /// A Rust item (an API item, or one of the crate's): found by rust-analyzer's workspace symbols, of its name,
    /// its URI holding `place` (its crate's directory: `/kompoz-ui/`; the package's URI).
    Api {
        rust: String,
        place: String,
        module: String,
        owner: String,
    },
    None,
}

/// An LSP location of a Kotlin stretch in a file the module table read (its text: an open document's, else the
/// file's): the stretch's lines and columns are the file's.
pub fn file_location(path: &Path, loc: Loc, text: &str) -> Json {
    let lines = crate::text::Lines::new(text);
    let offset = |p: kompoz_kotlin::Pos| {
        let start = lines.start((p.line as usize).saturating_sub(1), text);
        let mut o = start;
        for (n, (i, _)) in text[start..].char_indices().enumerate() {
            o = start + i;
            if n + 1 >= p.column as usize {
                return o;
            }
        }
        o.max(text.len())
    };
    let (from, to) = (offset(loc.start), offset(loc.end));
    Json::object([("uri", crate::doc::path_to_uri(path).into()), ("range", lines.range(text, from, to.max(from)))])
}

pub fn definition(
    doc: &Doc,
    offset: usize,
    rust_items: &[kompoz_kotlin::module::RustItem],
    text_of: &dyn Fn(&Path) -> Option<String>,
) -> Definition {
    let Some((region, k)) = doc.region_at(offset) else { return Definition::None };
    let Some(t) = doc.translated.get(region) else { return Definition::None };
    let here = |region: usize, loc: Loc| {
        Definition::Location(Json::object([("uri", doc.uri.clone().into()), ("range", doc.range(region, loc))]))
    };
    let Some(r) = t.analysis.reference_at(k) else {
        if let Some((found, _)) = member_declaration(doc, region, k) {
            return match found {
                Found::Here(i, d) => here(i, d.name),
                Found::Module(m) => match text_of(&m.file) {
                    Some(text) => Definition::Location(file_location(&m.file, m.decl.name, &text)),
                    None => Definition::None,
                },
            };
        }
        // (a declaration's own name: itself)
        return match t.declarations.iter().find(|d| d.name.start.offset <= k && k <= d.name.end.offset) {
            Some(d) => here(region, d.name),
            None => Definition::None,
        };
    };
    match &r.target {
        Target::Local(at) => here(region, *at),
        Target::Declaration(kotlin) => match find_declaration(doc, kotlin) {
            Some(Found::Here(i, d)) => here(i, d.name),
            Some(Found::Module(m)) => match text_of(&m.file) {
                Some(text) => Definition::Location(file_location(&m.file, m.decl.name, &text)),
                None => Definition::None,
            },
            None => {
                // (the crate's own Rust item, as the module table has it)
                let (owner, name) = match kotlin.rsplit_once('.') {
                    Some((o, n)) => (o, n),
                    None => ("", kotlin.as_str()),
                };
                match rust_items.iter().find(|r| r.kotlin == name && (owner.is_empty() || r.owner == owner)) {
                    Some(r) => Definition::Api {
                        rust: r.rust.clone(),
                        place: doc.manifest.as_deref().map(|m| crate::doc::path_to_uri(m) + "/").unwrap_or_default(),
                        module: r.module.clone(),
                        owner: r.owner.clone(),
                    },
                    None => Definition::None,
                }
            }
        },
        Target::Api { rust, krate, module, owner, .. } => Definition::Api {
            rust: rust.clone(),
            place: format!("/{}/", krate.replace('_', "-")),
            module: module.clone(),
            owner: owner.clone(),
        },
    }
}

/// Of rust-analyzer's workspace symbols, a Rust item's: its name, its URI holding `place` (see
/// [`Definition::Api`]), its owner the container when it has one, the module's file preferred.
pub fn pick_symbol(symbols: &Json, rust: &str, place: &str, module: &str, owner: &str) -> Option<Json> {
    let last_module = module.rsplit("::").next().unwrap_or("");
    let mut best: Option<(i32, Json)> = None;
    for s in symbols.array() {
        if s["name"].str() != rust {
            continue;
        }
        let uri = s["location"]["uri"].str();
        let mut score = 0;
        if uri.contains(place) {
            score += 4;
        }
        if !owner.is_empty() && s["containerName"].str() == owner {
            score += 2;
        }
        if !last_module.is_empty()
            && (uri.contains(&format!("/{last_module}.rs")) || uri.contains(&format!("/{last_module}/")))
        {
            score += 1;
        }
        if best.as_ref().is_none_or(|(b, _)| score > *b) {
            best = Some((score, s["location"].clone()));
        }
    }
    best.filter(|(s, _)| *s >= 4).map(|(_, l)| l)
}

// ---- Highlights ------------------------------------------------------------------------------------------------

/// The uses of what the name at `offset` refers to, in its region (and its declaration there).
pub fn highlights(doc: &Doc, offset: usize) -> Json {
    let Some((region, k)) = doc.region_at(offset) else { return Json::Array(Vec::new()) };
    let Some(t) = doc.translated.get(region) else { return Json::Array(Vec::new()) };
    let a = &t.analysis;
    let target = match a.reference_at(k) {
        Some(r) => r.target.clone(),
        None => match a.locals.iter().find(|l| l.loc.start.offset <= k && k <= l.loc.end.offset) {
            Some(l) => Target::Local(l.loc),
            None => match t.declarations.iter().find(|d| d.name.start.offset <= k && k <= d.name.end.offset) {
                Some(d) => Target::Declaration(d.kotlin.clone()),
                None => return Json::Array(Vec::new()),
            },
        },
    };
    let mut locs: Vec<Loc> = a.references.iter().filter(|r| r.target == target).map(|r| r.loc).collect();
    match &target {
        Target::Local(at) => locs.push(*at),
        Target::Declaration(kotlin) => {
            locs.extend(t.declarations.iter().filter(|d| d.kotlin == *kotlin).map(|d| d.name));
        }
        Target::Api { .. } => {}
    }
    locs.sort_by_key(|l| l.start.offset);
    locs.dedup();
    Json::Array(locs.into_iter().map(|l| Json::object([("range", doc.range(region, l)), ("kind", 1.into())])).collect())
}

// ---- Document symbols ------------------------------------------------------------------------------------------

fn symbol_kind(k: DeclKind, member: bool) -> i64 {
    match k {
        DeclKind::Function | DeclKind::Composable if member => 6,
        DeclKind::Function | DeclKind::Composable => 12,
        DeclKind::Property if member => 7,
        DeclKind::Property => 13,
        DeclKind::Class | DeclKind::DataClass => 5,
        DeclKind::Object | DeclKind::Companion => 19,
        DeclKind::Enum => 10,
        DeclKind::EnumEntry => 22,
        DeclKind::Interface => 11,
        DeclKind::TypeAlias => 26,
        DeclKind::Constructor => 9,
    }
}

/// The Kotlin's declarations as LSP's document symbols: nested (`DocumentSymbol`s), or flat with their containers
/// (`SymbolInformation`s).
pub fn symbols(doc: &Doc, hierarchical: bool) -> Vec<Json> {
    let mut out = Vec::new();
    for (i, t) in doc.translated.iter().enumerate() {
        // (the declarations by their place: a class's constructor properties among its members)
        let mut decls: Vec<&Declaration> = t.declarations.iter().collect();
        decls.sort_by_key(|d| (d.range.start.offset, std::cmp::Reverse(d.range.end.offset)));
        if hierarchical {
            let mut roots: Vec<(String, Json)> = Vec::new();
            for d in decls {
                let (parent, name) = match d.kotlin.rsplit_once('.') {
                    Some((p, n)) => (Some(p), n),
                    None => (None, d.kotlin.as_str()),
                };
                let sym = Json::object([
                    ("name", name.into()),
                    ("detail", d.detail.clone().into()),
                    ("kind", symbol_kind(d.kind, parent.is_some()).into()),
                    ("range", doc.range(i, d.range)),
                    ("selectionRange", doc.range(i, d.name)),
                    ("children", Json::Array(Vec::new())),
                ]);
                match parent.and_then(|p| find_symbol(&mut roots, p)) {
                    Some(p) => p.entry("children").push(sym),
                    None => roots.push((d.kotlin.clone(), sym)),
                }
            }
            out.extend(roots.into_iter().map(|(_, s)| s));
        } else {
            for d in decls {
                let (parent, name) = match d.kotlin.rsplit_once('.') {
                    Some((p, n)) => (Some(p), n),
                    None => (None, d.kotlin.as_str()),
                };
                let mut sym = Json::object([
                    ("name", name.into()),
                    ("kind", symbol_kind(d.kind, parent.is_some()).into()),
                    ("location", Json::object([("uri", doc.uri.clone().into()), ("range", doc.range(i, d.range))])),
                ]);
                if let Some(p) = parent {
                    sym.set("containerName", p);
                }
                out.push(sym);
            }
        }
    }
    out
}

/// The symbol of qualified name `kotlin` among `roots` and their children (whose names are their last segment).
fn find_symbol<'a>(roots: &'a mut [(String, Json)], kotlin: &str) -> Option<&'a mut Json> {
    let (root, rest) = match kotlin.split_once('.') {
        Some((r, rest)) => (r, Some(rest)),
        None => (kotlin, None),
    };
    let sym = &mut roots.iter_mut().rev().find(|(k, _)| k == root)?.1;
    let Some(rest) = rest else { return Some(sym) };
    let mut at = sym;
    for part in rest.split('.') {
        let children = at.get_mut("children")?.array_mut()?;
        at = children.iter_mut().rev().find(|c| c["name"].str() == part)?;
    }
    Some(at)
}

// ---- Completion ------------------------------------------------------------------------------------------------

/// LSP's completion item kinds.
mod item_kind {
    pub const METHOD: i64 = 2;
    pub const FUNCTION: i64 = 3;
    pub const CONSTRUCTOR: i64 = 4;
    pub const VARIABLE: i64 = 6;
    pub const CLASS: i64 = 7;
    pub const INTERFACE: i64 = 8;
    pub const PROPERTY: i64 = 10;
    pub const VALUE: i64 = 12;
    pub const ENUM: i64 = 13;
    pub const KEYWORD: i64 = 14;
    pub const ENUM_MEMBER: i64 = 20;
    pub const CONSTANT: i64 = 21;
}

fn decl_item_kind(k: DeclKind, member: bool) -> i64 {
    match k {
        DeclKind::Function | DeclKind::Composable if member => item_kind::METHOD,
        DeclKind::Function | DeclKind::Composable => item_kind::FUNCTION,
        DeclKind::Property if member => item_kind::PROPERTY,
        DeclKind::Property => item_kind::VARIABLE,
        DeclKind::Class | DeclKind::DataClass | DeclKind::Object | DeclKind::Companion | DeclKind::TypeAlias => {
            item_kind::CLASS
        }
        DeclKind::Enum => item_kind::ENUM,
        DeclKind::EnumEntry => item_kind::ENUM_MEMBER,
        DeclKind::Interface => item_kind::INTERFACE,
        DeclKind::Constructor => item_kind::CONSTRUCTOR,
    }
}

fn api_item_kind(i: &Item) -> i64 {
    match i.kind {
        Kind::Function | Kind::Overload => item_kind::FUNCTION,
        Kind::Method => item_kind::METHOD,
        Kind::Property => item_kind::PROPERTY,
        Kind::Value if !i.owner.is_empty() && i.owner == i.rust => item_kind::CLASS,
        Kind::Value => item_kind::VALUE,
        Kind::Constant => item_kind::CONSTANT,
        Kind::Variant => item_kind::ENUM_MEMBER,
        Kind::FunInterface => item_kind::INTERFACE,
    }
}

const KEYWORDS: &[&str] = &[
    "fun",
    "val",
    "var",
    "if",
    "else",
    "when",
    "for",
    "while",
    "do",
    "return",
    "break",
    "continue",
    "in",
    "is",
    "as",
    "null",
    "true",
    "false",
    "this",
    "object",
    "class",
    "data",
    "enum",
    "sealed",
    "interface",
    "private",
    "internal",
    "override",
    "open",
    "abstract",
    "companion",
    "typealias",
    "try",
    "catch",
    "finally",
    "throw",
    "by",
    "suspend",
    "inline",
    "const",
    "lateinit",
];

/// The most items a list gives (it says it is incomplete past them: the editor asks again as more is typed).
const MOST: usize = 400;

struct Items {
    list: Vec<Json>,
    seen: std::collections::HashSet<String>,
    prefix: String,
    incomplete: bool,
}

impl Items {
    fn matches(&self, label: &str) -> bool {
        label.to_lowercase().starts_with(&self.prefix.to_lowercase())
    }

    fn add(&mut self, label: &str, kind: i64, detail: &str, doc: Option<String>, sort: &str) {
        if label.is_empty() || !self.matches(label) || !self.seen.insert(format!("{label}\u{0}{detail}")) {
            return;
        }
        if self.list.len() >= MOST {
            self.incomplete = true;
            return;
        }
        let mut item = Json::object([
            ("label", label.into()),
            ("kind", kind.into()),
            ("detail", detail.into()),
            ("sortText", format!("{sort}{label}").into()),
            ("data", Json::object([("kompoz", true.into())])),
        ]);
        if let Some(d) = doc {
            item.set("documentation", Json::object([("kind", "markdown".into()), ("value", d.into())]));
        }
        self.list.push(item);
    }
}

/// What the completion at a place is of: the name being written, and the receiver's end before a `.` (a member).
struct Spot {
    /// Where the name being written starts (the Kotlin offset), and it.
    start: usize,
    prefix: String,
    /// A member after `receiver.` (or `receiver?.`): the receiver's end.
    receiver_end: Option<usize>,
}

fn spot(text: &str, k: usize) -> Spot {
    let k = crate::text::floor_char(text, k.min(text.len()));
    let start = text[..k]
        .char_indices()
        .rev()
        .take_while(|(_, c)| kompoz_kotlin::lexer::ident_part(*c))
        .last()
        .map_or(k, |(i, _)| i);
    let before = &text[..start];
    let receiver_end = if let Some(b) = before.strip_suffix("?.") {
        Some(b.trim_end().len())
    } else if let Some(b) = before.strip_suffix('.') {
        // (`1..` isn't a member)
        (!b.ends_with('.')).then(|| b.trim_end().len())
    } else {
        None
    };
    Spot { start, prefix: text[start..k].to_string(), receiver_end }
}

/// The analysis of a region's text as it would read with the completion's spot settled: the name being written
/// taken out (a member's `.` too, the receiver left), so that the text parses as it did before the typing began.
fn settled(doc: &Doc, region: usize, spot: &Spot, k: usize, table: Option<Arc<ModuleTable>>) -> Analysis {
    let text = doc.kotlin(region);
    let k = crate::text::floor_char(text, k.min(text.len()));
    let (from, filler) = match spot.receiver_end {
        Some(end) => (end, ""),
        None => (spot.start, "__kompoz_completion"),
    };
    let mut changed = String::with_capacity(text.len() + filler.len());
    changed.push_str(&text[..from]);
    changed.push_str(filler);
    changed.push_str(&text[k..]);
    let t = kompoz_kotlin::translate(&changed, doc.regions[region].base, &OneSpan::default(), &doc.options(table));
    t.analysis.unwrap_or_default()
}

/// The Kotlin name of a type's base (`List<Int>?` → `List`).
fn type_base(ty: &str) -> &str {
    let t = ty.trim().trim_end_matches('?').trim_start_matches('(');
    t.split(['<', '?', ' ']).next().unwrap_or(t)
}

/// The extension traits of `kompoz_runtime::kt` whose methods are a Kotlin type's members.
fn kt_traits(base: &str) -> &'static [&'static str] {
    match base {
        "String" | "CharSequence" => &["KtStr", "KtNullableStr", "KtToRegex", "KtToString", "KtScope"],
        "List" | "MutableList" | "Iterable" | "Collection" | "Array" | "Sequence" | "Set" | "MutableSet" => &[
            "KtList",
            "KtMutableList",
            "KtIterable",
            "KtSum",
            "KtShuffle",
            "KtListOfLists",
            "KtListOfNullable",
            "KtToString",
            "KtScope",
        ],
        "Map" | "MutableMap" => &["KtMapExt", "KtMapLike", "KtToString", "KtScope"],
        "Int" | "Long" | "Short" | "Byte" => &["KtNumber", "KtAbs", "KtInc", "KtRangeBound", "KtToString", "KtScope"],
        "Float" | "Double" => &["KtNumber", "KtFloat", "KtAbs", "KtRangeBound", "KtToString", "KtScope"],
        "Char" => &["KtChar", "KtToString", "KtScope"],
        "Pair" => &["KtPair", "KtToString", "KtScope"],
        "Triple" => &["KtTriple", "KtToString", "KtScope"],
        "IntRange" | "ClosedRange" | "IntProgression" => &["KtClosedRange", "KtStep", "KtList", "KtScope"],
        _ => &["KtScope", "KtToString"],
    }
}

/// The completion list at a document offset in the Kotlin.
pub fn completion(doc: &Doc, offset: usize, table: Option<Arc<ModuleTable>>) -> Json {
    let empty = Json::object([("isIncomplete", false.into()), ("items", Json::Array(Vec::new()))]);
    let Some((region, k)) = doc.region_at(offset) else { return empty };
    let text = doc.kotlin(region);
    let s = spot(text, k);
    let a = settled(doc, region, &s, k, table.clone());
    let current = doc.translated.get(region).map(|t| &t.analysis);
    let mut items = Items { list: Vec::new(), seen: Default::default(), prefix: s.prefix.clone(), incomplete: false };
    let kinds = declaration_kinds(doc);
    match s.receiver_end {
        Some(end) => members(doc, &a, current, text, end, &kinds, &mut items),
        None => names(doc, &a, current, s.start, table.as_deref(), &mut items),
    }
    Json::object([("isIncomplete", items.incomplete.into()), ("items", Json::Array(items.list))])
}

/// The receiver's type at its end: the widest expression ending there the generator typed, else what the name just
/// before is (a local's type, a class or object named).
fn receiver_type(a: &Analysis, current: Option<&Analysis>, text: &str, end: usize) -> Option<String> {
    for an in [Some(a), current].into_iter().flatten() {
        if let Some(t) =
            an.types.iter().filter(|t| t.loc.end.offset == end).max_by_key(|t| t.loc.end.offset - t.loc.start.offset)
        {
            return Some(t.ty.clone());
        }
    }
    let name_start = text[..end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| kompoz_kotlin::lexer::ident_part(*c))
        .last()
        .map(|(i, _)| i)?;
    let name = &text[name_start..end];
    for an in [Some(a), current].into_iter().flatten() {
        if let Some(l) = an.locals_at(name_start).into_iter().find(|l| l.name == name)
            && !l.ty.is_empty()
        {
            return Some(l.ty.clone());
        }
    }
    name.chars().next().filter(|c| c.is_uppercase()).map(|_| name.to_string())
}

fn members(
    doc: &Doc,
    a: &Analysis,
    current: Option<&Analysis>,
    text: &str,
    end: usize,
    kinds: &HashMap<String, DeclKind>,
    items: &mut Items,
) {
    let Some(ty) = receiver_type(a, current, text, end) else { return };
    let base = type_base(&ty).to_string();
    // (the crate's own: a class's members, an object's, an enum's entries)
    let prefix = format!("{base}.");
    let mut own: Vec<(&Declaration, Option<&Path>)> = Vec::new();
    for t in &doc.translated {
        own.extend(t.declarations.iter().map(|d| (d, None)));
    }
    own.extend(doc.module_decls.iter().map(|m| (&m.decl, Some(m.file.as_path()))));
    for (d, _) in &own {
        if let Some(name) = d.kotlin.strip_prefix(&prefix)
            && !name.contains('.')
            && d.kind != DeclKind::Constructor
        {
            let kind = kinds.get(&d.kotlin).copied().unwrap_or(d.kind);
            items.add(name, decl_item_kind(kind, true), &d.detail, None, "0");
        }
    }
    // (the API's: the type's members and what it derefs to, the kt shims' extension members)
    let owner = index::rust_type(&base).unwrap_or(&base).to_string();
    let chain = index::deref_chain(&owner);
    let traits = kt_traits(&base);
    for i in index::ITEMS {
        let member = (chain.contains(&i.owner) && !i.owner.is_empty())
            || (i.owner.is_empty() && i.kind == Kind::Method && traits.contains(&i.via));
        if !member || i.kind == Kind::Overload {
            continue;
        }
        let label = i.kotlin.rsplit('.').next().unwrap_or(i.kotlin);
        let path = item_path(i.krate, i.module, i.owner, i.rust);
        items.add(label, api_item_kind(i), &item_signature(i), Some(format!("Rust: `{path}`")), "1");
    }
}

fn names(
    doc: &Doc,
    a: &Analysis,
    current: Option<&Analysis>,
    at: usize,
    table: Option<&ModuleTable>,
    items: &mut Items,
) {
    // (the locals that can be named here, innermost first)
    let mut locals = a.locals_at(at);
    if locals.is_empty()
        && let Some(c) = current
    {
        locals = c.locals_at(at);
    }
    for l in locals {
        items.add(&l.name, item_kind::VARIABLE, &local_line(l), None, "0");
    }
    // (the text's declarations and the module table's: top-level ones, and the members of the class the place is in)
    for t in &doc.translated {
        for d in &t.declarations {
            if !d.kotlin.contains('.') && d.kind != DeclKind::Constructor {
                items.add(&d.kotlin, decl_item_kind(d.kind, false), &d.detail, None, "1");
            }
        }
        for d in &t.declarations {
            let inside = d.range.start.offset <= at && at <= d.range.end.offset;
            if inside
                && matches!(d.kind, DeclKind::Class | DeclKind::DataClass | DeclKind::Object | DeclKind::Companion)
            {
                let prefix = format!("{}.", d.kotlin);
                for m in &t.declarations {
                    if let Some(name) = m.kotlin.strip_prefix(&prefix)
                        && !name.contains('.')
                        && m.kind != DeclKind::Constructor
                    {
                        items.add(name, decl_item_kind(m.kind, true), &m.detail, None, "0");
                    }
                }
            }
        }
    }
    for m in doc.module_decls.iter() {
        let private =
            m.decl.detail.split_whitespace().take_while(|w| *w != "fun" && *w != "val").any(|w| w == "private");
        if !m.decl.kotlin.contains('.') && !private && m.decl.kind != DeclKind::Constructor {
            let file = m.file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            items.add(&m.decl.kotlin, decl_item_kind(m.decl.kind, false), &m.decl.detail, Some(file), "1");
        }
    }
    // (the crate's Rust items, as the API index's)
    if let Some(t) = table {
        for r in &t.rust {
            if r.owner.is_empty() || r.owner == r.rust {
                let kind = match r.kind {
                    Kind::Function => item_kind::FUNCTION,
                    Kind::Value if r.owner == r.rust => item_kind::CLASS,
                    _ => item_kind::VALUE,
                };
                let path = if r.module.is_empty() {
                    format!("crate::{}", r.rust)
                } else {
                    format!("crate::{}::{}", r.module, r.rust)
                };
                items.add(&r.kotlin, kind, &path, None, "2");
            }
        }
    }
    // (the API's free functions, values and types)
    for i in index::ITEMS {
        let free = i.owner.is_empty() && matches!(i.kind, Kind::Function | Kind::Value | Kind::FunInterface);
        let ty = i.kind == Kind::Value && !i.owner.is_empty() && i.owner == i.rust;
        if !(free || ty) || i.kotlin.contains('.') {
            continue;
        }
        let path = item_path(i.krate, i.module, i.owner, i.rust);
        items.add(i.kotlin, api_item_kind(i), &item_signature(i), Some(format!("Rust: `{path}`")), "3");
    }
    for k in KEYWORDS {
        items.add(k, item_kind::KEYWORD, "", None, "4");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots() {
        let s = spot("val x = note.ti", 15);
        assert_eq!((s.start, s.prefix.as_str(), s.receiver_end), (13, "ti", Some(12)));
        let s = spot("a?.", 3);
        assert_eq!((s.prefix.as_str(), s.receiver_end), ("", Some(1)));
        let s = spot("foo(Te", 6);
        assert_eq!((s.start, s.prefix.as_str(), s.receiver_end), (4, "Te", None));
        assert_eq!(spot("1..", 3).receiver_end, None);
        assert_eq!(type_base("List<Note>?"), "List");
    }

    #[test]
    fn an_api_items_symbol() {
        let at = |uri: &str, container: &str| {
            Json::object([
                ("name", "padding".into()),
                ("containerName", container.into()),
                ("location", Json::object([("uri", uri.into())])),
            ])
        };
        let symbols = Json::Array(vec![
            at("file:///w/crates/kompoz-ui/src/x.rs", "Modifier"),
            at("file:///w/crates/kompoz-foundation/src/other.rs", "Other"),
            at("file:///w/crates/kompoz-foundation/src/layout/padding.rs", "Modifier"),
        ]);
        let got = pick_symbol(&symbols, "padding", "/kompoz-foundation/", "layout::padding", "Modifier").unwrap();
        assert_eq!(got["uri"].str(), "file:///w/crates/kompoz-foundation/src/layout/padding.rs");
        assert!(pick_symbol(&symbols, "padding", "/kompoz-text/", "", "").is_none());
    }
}
