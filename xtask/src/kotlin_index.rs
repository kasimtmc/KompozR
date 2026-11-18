//! `cargo xtask kotlin-index [--check]`: the API index, from the workspace's sources (read with syn, nothing
//! compiled), into two files that are committed:
//!
//! - `crates/kompoz-macros/api-index.txt`, what the macros read (its format and its writer:
//!   `crates/kompoz-macros/src/api_index.rs`, shared here by `#[path]`): every `#[named]` and `#[kompozable]`
//!   function and every `#[named]` impl's method with a defaulted parameter, of every target of every package, with
//!   which of their parameters have defaults;
//! - `crates/kompoz-kotlin/src/index/generated.rs`, what the Kotlin syntax's generator reads (its types:
//!   `crates/kompoz-kotlin/src/index/mod.rs`): the public API of the crates Kotlin code calls — functions,
//!   methods, extension traits' methods, associated functions, properties (`#[derive(KotlinProperties)]`), value
//!   objects (`Modifier`, `LocalDensity`), constants and enum entries (`Color::Red`, `TextAlign::Center`) — each
//!   with its Kotlin name (Rust's in camelCase, or what a doc line ``Kotlin: `name(…)` `` says), its parameters in
//!   order (types as text, defaults: Rust's `#[default(…)]` or Kotlin's from the doc line), lambdas' arities,
//!   receivers (`impl Fn(&mut DrawScope)`) and whether they suspend, and whether it suspends (`async fn`, a
//!   future returned), is `#[kompozable]` or `#[named]` or a `const fn`, or takes Kotlin's overloads as a tuple
//!   (`Color(args: impl ColorArgs)`, each overload an `impl ColorArgs for (…)` with its own doc line); what types
//!   deref to (`DEREFS`), the types Kotlin names otherwise (`TYPES`: a struct's doc line ``Kotlin:
//!   `Arrangement.Absolute` ``); closures' aliases written out in the parameters of their types (`Callback` →
//!   `Rc<dyn Fn()>`); the items a file's own `macro_rules!` makes of a type's name (`transforms!(DrawScope)`),
//!   expanded; the fun interfaces (a trait implemented for the closures of one signature), with the lambda their SAM
//!   constructor takes.
//!
//! `--check` builds both and fails if what is committed differs (CI).

use crate::api_index::{self, Entry, Kind as EntryKind, Scope};
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::Visit;
use syn::{
    Attribute, Expr, Fields, FnArg, GenericParam, Generics, ImplItem, Item, ItemImpl, ReturnType, Signature, TraitItem,
    Type, TypeParamBound, Visibility, WherePredicate,
};

const API_INDEX: &str = "crates/kompoz-macros/api-index.txt";
const KOTLIN_INDEX: &str = "crates/kompoz-kotlin/src/index/generated.rs";

/// The crates Kotlin code calls into: their library's public API goes into the Kotlin index.
const API_CRATES: &[&str] = &[
    "kompoz-runtime",
    "kompoz-coroutines",
    "kompoz-graphics",
    "kompoz-text",
    "kompoz-ui",
    "kompoz-animation",
    "kompoz-foundation",
    "kompoz-material-ripple",
    "kompoz-test",
];

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let packages = packages(root)?;
    let api = api_index::render(&api_entries(root, &packages)?);
    let kotlin = kotlin_index(&packages)?;
    let files = [(API_INDEX, api), (KOTLIN_INDEX, kotlin)];
    if check {
        let stale: Vec<&str> = files
            .iter()
            .filter(|(path, text)| fs::read_to_string(root.join(path)).ok().as_deref() != Some(text.as_str()))
            .map(|(path, _)| *path)
            .collect();
        if !stale.is_empty() {
            for s in &stale {
                eprintln!("out of date: {s}");
            }
            return Err("run `cargo xtask kotlin-index` and commit what it writes".to_string());
        }
        println!("kotlin-index: up to date");
        return Ok(());
    }
    for (path, text) in &files {
        let full = root.join(path);
        if fs::read_to_string(&full).ok().as_deref() != Some(text.as_str()) {
            if let Some(dir) = full.parent() {
                fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            fs::write(&full, text).map_err(|e| format!("{}: {e}", full.display()))?;
            println!("wrote {path}");
        }
    }
    Ok(())
}

// ---- The workspace: packages, their targets, the files of each target ----

struct Package {
    name: String,
    targets: Vec<Target>,
}

struct Target {
    /// The library (its items count everywhere), or another target.
    lib: bool,
    /// Cargo's `CARGO_CRATE_NAME`: the target's name, `-` made `_`.
    krate: String,
    root: PathBuf,
}

/// A source file of a target, parsed, with its module path in the crate and whether every module on the way is
/// public.
struct Source {
    path: PathBuf,
    module: Vec<String>,
    public: bool,
    file: syn::File,
}

/// The workspace's packages (its `members`, `dir/*` patterns expanded), in path order.
fn packages(root: &Path) -> Result<Vec<Package>, String> {
    let manifest = read(&root.join("Cargo.toml"))?;
    let mut dirs = Vec::new();
    let members = manifest.split_once("members").map(|(_, rest)| rest).unwrap_or("");
    let list = members.split_once('[').and_then(|(_, r)| r.split_once(']')).map(|(l, _)| l).unwrap_or("");
    for m in list.split(',').map(|m| m.trim().trim_matches('"')).filter(|m| !m.is_empty()) {
        match m.strip_suffix("/*") {
            Some(parent) => {
                let mut found: Vec<PathBuf> = fs::read_dir(root.join(parent))
                    .map_err(|e| format!("{parent}: {e}"))?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.join("Cargo.toml").is_file())
                    .collect();
                found.sort();
                dirs.extend(found);
            }
            None => dirs.push(root.join(m)),
        }
    }
    dirs.sort();
    dirs.iter().map(|d| package(d)).collect()
}

/// A package's name and targets (Cargo's: what its manifest declares, and what is found where Cargo looks).
fn package(dir: &Path) -> Result<Package, String> {
    let manifest = read(&dir.join("Cargo.toml"))?;
    let mut name = String::new();
    let mut lib_name: Option<String> = None;
    let mut lib_path: Option<String> = None;
    // ([[test]], [[bin]]…: their kind, name and path)
    let mut declared: Vec<(String, String, Option<String>)> = Vec::new();
    let mut table = String::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            table = line.to_string();
            if let Some(kind) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
                declared.push((kind.to_string(), String::new(), None));
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim().trim_matches('"').to_string());
        match (table.as_str(), key) {
            ("[package]", "name") => name = value,
            ("[lib]", "name") => lib_name = Some(value),
            ("[lib]", "path") => lib_path = Some(value),
            (t, "name") if t.starts_with("[[") => {
                if let Some(d) = declared.last_mut() {
                    d.1 = value;
                }
            }
            (t, "path") if t.starts_with("[[") => {
                if let Some(d) = declared.last_mut() {
                    d.2 = Some(value);
                }
            }
            _ => {}
        }
    }
    let krate = |n: &str| n.replace('-', "_");
    let mut targets = Vec::new();
    let lib = dir.join(lib_path.as_deref().unwrap_or("src/lib.rs"));
    if lib.is_file() {
        targets.push(Target { lib: true, krate: krate(lib_name.as_deref().unwrap_or(&name)), root: lib });
    }
    let mut others: BTreeMap<(String, String), PathBuf> = BTreeMap::new();
    if dir.join("src/main.rs").is_file() {
        others.insert(("bin".into(), name.clone()), dir.join("src/main.rs"));
    }
    for (kind, sub) in [("bin", "src/bin"), ("test", "tests"), ("example", "examples"), ("bench", "benches")] {
        let Ok(entries) = fs::read_dir(dir.join(sub)) else { continue };
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if p.extension().is_some_and(|x| x == "rs") {
                others.insert((kind.into(), stem), p);
            } else if p.join("main.rs").is_file() {
                others.insert((kind.into(), stem), p.join("main.rs"));
            }
        }
    }
    for (kind, n, path) in declared {
        let default = match kind.as_str() {
            "bin" => format!("src/bin/{n}.rs"),
            "test" => format!("tests/{n}.rs"),
            "example" => format!("examples/{n}.rs"),
            _ => format!("benches/{n}.rs"),
        };
        others.insert((kind, n), dir.join(path.unwrap_or(default)));
    }
    for ((_, n), root) in others {
        if root.is_file() {
            targets.push(Target { lib: false, krate: krate(&n), root });
        }
    }
    Ok(Package { name, targets })
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The files of a target: its root and every module file it declares (`mod x;`, `#[path]` honoured), in order.
fn sources(target: &Target) -> Result<Vec<Source>, String> {
    let mut out = Vec::new();
    let dir = target.root.parent().unwrap_or(Path::new("")).to_path_buf();
    load(&target.root, &dir, Vec::new(), true, &mut out)?;
    Ok(out)
}

fn load(path: &Path, children: &Path, module: Vec<String>, public: bool, out: &mut Vec<Source>) -> Result<(), String> {
    let text = read(path)?;
    let file = syn::parse_file(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut pending = Vec::new();
    declared_modules(&file.items, path, children, &module, public, &mut pending);
    out.push(Source { path: path.to_path_buf(), module, public, file });
    for (p, c, m, public) in pending {
        load(&p, &c, m, public, out)?;
    }
    Ok(())
}

/// The file modules `items` declare (in inline modules too): (file, its children's directory, module path, public).
fn declared_modules(
    items: &[Item],
    file: &Path,
    children: &Path,
    module: &[String],
    public: bool,
    out: &mut Vec<(PathBuf, PathBuf, Vec<String>, bool)>,
) {
    for item in items {
        let Item::Mod(m) = item else { continue };
        let name = m.ident.to_string();
        let name = name.trim_start_matches("r#").to_string();
        let mut path = module.to_vec();
        path.push(name.clone());
        let is_pub = public && matches!(m.vis, Visibility::Public(_));
        match &m.content {
            Some((_, inner)) => declared_modules(inner, file, &children.join(&name), &path, is_pub, out),
            None => {
                let explicit = m.attrs.iter().find_map(|a| {
                    if !a.path().is_ident("path") {
                        return None;
                    }
                    match &a.meta {
                        syn::Meta::NameValue(nv) => match &nv.value {
                            Expr::Lit(l) => match &l.lit {
                                syn::Lit::Str(s) => Some(s.value()),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    }
                });
                let found = match explicit {
                    Some(p) => {
                        let f = file.parent().unwrap_or(Path::new("")).join(p);
                        let c = f.parent().unwrap_or(Path::new("")).to_path_buf();
                        Some((f, c))
                    }
                    None => {
                        let flat = children.join(format!("{name}.rs"));
                        let nested = children.join(&name).join("mod.rs");
                        if flat.is_file() {
                            Some((flat, children.join(&name)))
                        } else if nested.is_file() {
                            Some((nested, children.join(&name)))
                        } else {
                            // (a module of another platform's, or one a build script writes: not here)
                            None
                        }
                    }
                };
                if let Some((f, c)) = found {
                    out.push((f, c, path, is_pub));
                }
            }
        }
    }
}

/// `path` relative to the workspace, with `/` separators, `..` resolved (`tests/../../x` is `x`).
fn shown(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut parts: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::CurDir => {}
            other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
        }
    }
    parts.join("/")
}

// ---- What attributes say ----

/// The last segment of an attribute's path (`#[kompoz_runtime::kompozable]` is `kompozable`).
fn attr_name(a: &Attribute) -> String {
    a.path().segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| attr_name(a) == name)
}

/// `#[named(trait Name)]`'s trait (`None` for a plain `#[named]`).
fn named_trait(attrs: &[Attribute]) -> Option<Option<String>> {
    let a = attrs.iter().find(|a| attr_name(a) == "named")?;
    let syn::Meta::List(list) = &a.meta else { return Some(None) };
    let mut tokens = list.tokens.clone().into_iter().skip_while(|t| !matches!(t, TokenTree::Ident(i) if i == "trait"));
    tokens.next();
    Some(tokens.next().map(|t| t.to_string()))
}

/// Whether a parameter has a `#[default(…)]`, and its expression as text.
fn param_default(attrs: &[Attribute]) -> Option<String> {
    let a = attrs.iter().find(|a| a.path().is_ident("default"))?;
    match &a.meta {
        syn::Meta::List(l) => Some(text(l.tokens.clone())),
        _ => Some(String::new()),
    }
}

/// Whether an item is `#[doc(hidden)]`.
fn hidden(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("doc") && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string().contains("hidden"))
    })
}

/// The doc line ``Kotlin: `…` `` of an item, its text between the backticks.
fn kotlin_doc(attrs: &[Attribute]) -> Option<String> {
    for a in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let syn::Meta::NameValue(nv) = &a.meta else { continue };
        let Expr::Lit(l) = &nv.value else { continue };
        let syn::Lit::Str(s) = &l.lit else { continue };
        let line = s.value();
        let Some(rest) = line.trim().strip_prefix("Kotlin: `") else { continue };
        if let Some(sig) = rest.strip_suffix('`') {
            return Some(sig.to_string());
        }
    }
    None
}

// ---- api-index.txt ----

fn api_entries(root: &Path, packages: &[Package]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for p in packages {
        for t in &p.targets {
            let scope =
                if t.lib { Scope::All } else { Scope::Target { package: p.name.clone(), krate: t.krate.clone() } };
            for s in sources(t)? {
                let mut v = ApiVisitor { scope: &scope, file: shown(root, &s.path), out: &mut entries };
                v.visit_file(&s.file);
            }
        }
    }
    Ok(entries)
}

struct ApiVisitor<'a> {
    scope: &'a Scope,
    file: String,
    out: &'a mut Vec<Entry>,
}

fn defaults_of(sig: &Signature) -> Vec<bool> {
    sig.inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) => Some(param_default(&pt.attrs).is_some()),
            FnArg::Receiver(_) => None,
        })
        .collect()
}

impl<'ast> Visit<'ast> for ApiVisitor<'_> {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.function(&f.attrs, &f.sig);
        syn::visit::visit_item_fn(self, f);
    }

    fn visit_item_impl(&mut self, imp: &'ast ItemImpl) {
        if let Some(ext) = named_trait(&imp.attrs) {
            let ty = type_name(&imp.self_ty);
            let owner = match ext {
                Some(t) => format!("{} {ty} as {t}", self.file),
                None => format!("{} {ty}", self.file),
            };
            for item in &imp.items {
                let ImplItem::Fn(m) = item else { continue };
                let defaults = defaults_of(&m.sig);
                if defaults.iter().any(|d| *d) {
                    self.out.push(Entry {
                        scope: self.scope.clone(),
                        kind: EntryKind::Method,
                        name: unraw(&m.sig.ident),
                        defaults,
                        owner: owner.clone(),
                    });
                }
            }
        }
        syn::visit::visit_item_impl(self, imp);
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.function(&f.attrs, &f.sig);
        syn::visit::visit_impl_item_fn(self, f);
    }

    // (what a macro invocation holds isn't read: neither are `macro_rules!` bodies nor `quote!`'s)
    fn visit_macro(&mut self, _: &'ast syn::Macro) {}
}

impl ApiVisitor<'_> {
    fn function(&mut self, attrs: &[Attribute], sig: &Signature) {
        let kind = if has_attr(attrs, "kompozable") {
            EntryKind::Kompozable
        } else if has_attr(attrs, "named") {
            EntryKind::Fn
        } else {
            return;
        };
        self.out.push(Entry {
            scope: self.scope.clone(),
            kind,
            name: unraw(&sig.ident),
            defaults: defaults_of(sig),
            owner: self.file.clone(),
        });
    }
}

fn unraw(i: &syn::Ident) -> String {
    i.to_string().trim_start_matches("r#").to_string()
}

/// A type's name as an owner: its path's last segment, generics left out.
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
        Type::Reference(r) => type_name(&r.elem),
        other => text(other.to_token_stream()),
    }
}

// ---- Rust as text ----

/// Tokens as Rust is written: spaces between words, after commas, around `+`, `->`, `=`, `=>`, none elsewhere
/// (`impl Fn(&mut DrawScope<'_>) + 'static`, `Option<Vec<T>>`).
fn text(ts: TokenStream) -> String {
    let mut out = String::new();
    write_tokens(ts, &mut out);
    out.trim().to_string()
}

fn write_tokens(ts: TokenStream, out: &mut String) {
    // (what the last token was: a word wants a space before the next word)
    let mut last_word = false;
    let mut joint_quote = false;
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Ident(id) => {
                if last_word && !joint_quote {
                    out.push(' ');
                }
                out.push_str(&id.to_string());
                last_word = true;
                joint_quote = false;
            }
            TokenTree::Literal(l) => {
                if last_word {
                    out.push(' ');
                }
                out.push_str(&l.to_string());
                last_word = true;
                joint_quote = false;
            }
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::Brace => ("{ ", " }"),
                    Delimiter::None => ("", ""),
                };
                // (a keyword before a bracket is a word of its own: `mut [u8]`, `dyn (Fn())`; not a call's `f(`)
                let keyword = last_word
                    && ["mut", "dyn", "impl", "as", "in", "move", "ref", "const", "static", "where", "for"].iter().any(
                        |k| {
                            out.ends_with(k)
                                && !out[..out.len() - k.len()].ends_with(|c: char| c.is_alphanumeric() || c == '_')
                        },
                    );
                if (g.delimiter() == Delimiter::Brace && !out.is_empty() && !out.ends_with(' ')) || keyword {
                    out.push(' ');
                }
                out.push_str(open);
                write_tokens(g.stream(), out);
                out.push_str(close);
                last_word = g.delimiter() == Delimiter::None;
                joint_quote = false;
            }
            TokenTree::Punct(p) => {
                // (a multi-character operator: its puncts joined)
                let mut op = p.as_char().to_string();
                let mut j = i;
                while matches!(&tokens[j], TokenTree::Punct(q) if q.spacing() == Spacing::Joint)
                    && j + 1 < tokens.len()
                    && matches!(&tokens[j + 1], TokenTree::Punct(_))
                {
                    j += 1;
                    if let TokenTree::Punct(q) = &tokens[j] {
                        op.push(q.as_char());
                    }
                }
                i = j;
                match op.as_str() {
                    "'" => {
                        if last_word {
                            out.push(' ');
                        }
                        out.push('\'');
                        joint_quote = true;
                        last_word = false;
                        i += 1;
                        continue;
                    }
                    "," | ";" => {
                        out.push_str(&op);
                        out.push(' ');
                    }
                    "+" | "->" | "=" | "=>" | "|" | "+=" | "-=" | "==" | "!=" | "&&" | "||" | "<=" | ">=" => {
                        out.push(' ');
                        out.push_str(&op);
                        out.push(' ');
                    }
                    ":" => out.push_str(": "),
                    _ => out.push_str(&op),
                }
                last_word = false;
                joint_quote = false;
            }
        }
        i += 1;
    }
    // (a trailing comma's space, or a doubled one)
    while out.ends_with(' ') && !out.ends_with("{ ") {
        out.pop();
    }
    let fixed = out.replace(", )", ")").replace(", >", ">").replace(", ]", "]").replace("  ", " ");
    *out = fixed;
}

// ---- The Kotlin index ----

/// What the generator gets of an item (the types of crates/kompoz-kotlin/src/index/mod.rs, as text).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct KItem {
    kotlin: String,
    krate: String,
    module: String,
    owner: String,
    rust: String,
    kind: &'static str,
    via: String,
    receiver: &'static str,
    params: Vec<KParam>,
    ret: String,
    flags: BTreeSet<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct KParam {
    rust: String,
    kotlin: String,
    ty: String,
    default: Option<(bool, String)>,
    vararg: bool,
    lambda: Option<KLambda>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct KLambda {
    kind: &'static str,
    inputs: Vec<String>,
    output: String,
    receiver: String,
    suspend: bool,
    /// Its future is boxed: `Pin<Box<dyn Future<…>>>`, or an alias of one.
    boxed: bool,
}

/// A Kotlin signature from a doc line: `name(p: T = d, vararg q: U): R`.
struct KotlinSig {
    name: String,
    params: Vec<(String, String, Option<String>, bool)>,
}

fn kotlin_sig(sig: &str) -> Option<KotlinSig> {
    let open = sig.find('(')?;
    let name = sig[..open].trim().to_string();
    let mut depth = 0;
    let mut close = None;
    for (i, c) in sig.char_indices().skip(open) {
        match c {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' | '>' | ']' | '}' => {
                // (`->` in a function type isn't a bracket)
                if c == '>' && sig[..i].ends_with('-') {
                    continue;
                }
                depth -= 1;
                if depth == 0 && c == ')' {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let inner = &sig[open + 1..close?];
    let mut params = Vec::new();
    for part in split_top(inner, ',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (vararg, part) = match part.strip_prefix("vararg ") {
            Some(p) => (true, p.trim()),
            None => (false, part),
        };
        let (decl, default) = match split_top(part, '=').as_slice() {
            [d, rest @ ..] if !rest.is_empty() => (d.trim().to_string(), Some(rest.join("=").trim().to_string())),
            _ => (part.to_string(), None),
        };
        let (pname, ty) = decl.split_once(':').map(|(n, t)| (n.trim(), t.trim())).unwrap_or((decl.as_str(), ""));
        params.push((pname.to_string(), ty.to_string(), default, vararg));
    }
    Some(KotlinSig { name, params })
}

/// `s` split at `sep` outside brackets and quotes (`=` in `->` or `==` doesn't split).
fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if let Some(q) = quote {
            cur.push(c);
            if c == q && chars.get(i.wrapping_sub(1)) != Some(&'\\') {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' | '<' | '[' | '{' => depth += 1,
            '>' if i > 0 && chars[i - 1] == '-' => {}
            ')' | '>' | ']' | '}' => depth -= 1,
            _ => {}
        }
        let lone_eq = sep != '=' || (chars.get(i + 1) != Some(&'=') && i > 0 && !"=!<>".contains(chars[i - 1]));
        if c == sep && depth == 0 && lone_eq {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// `snake_case` as Kotlin's `camelCase` (`join_to_string` → `joinToString`); a PascalCase or upper-case name as it
/// is.
fn camel(name: &str) -> String {
    let name = name.trim_start_matches("r#");
    if !name.contains('_') || name.chars().next().is_some_and(|c| c.is_uppercase()) {
        return name.to_string();
    }
    let mut out = String::new();
    let mut up = false;
    for c in name.chars() {
        if c == '_' {
            up = !out.is_empty();
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// What the scan of the API crates collects.
#[derive(Default)]
struct Scan {
    items: Vec<KItem>,
    /// The types that are futures (`impl Future for Delay`): a function giving one suspends.
    futures: BTreeSet<String>,
    /// `impl XArgs for (…)`'s, by trait: the overloads of the function taking `impl XArgs`.
    overloads: BTreeMap<String, Vec<(Type, Option<String>)>>,
    /// `impl Deref for X { type Target = Y; }`'s: (X, Y), what X's values have of Y's members.
    derefs: BTreeSet<(String, String)>,
    /// The type aliases of closures (`type Callback = Rc<dyn Fn()>`), by name: a parameter of one takes a lambda
    /// (one name aliasing two types isn't).
    aliases: BTreeMap<String, Option<Type>>,
    /// The aliases of boxed futures (`type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>`), by name:
    /// their type parameters and their future's output.
    boxed_futures: BTreeMap<String, (Vec<String>, Type)>,
    /// Kotlin's names of types that Rust names otherwise (a doc line ``Kotlin: `Arrangement.Absolute` `` on
    /// `AbsoluteArrangement`): (Kotlin, Rust).
    types: BTreeSet<(String, String)>,
}

fn kotlin_index(packages: &[Package]) -> Result<String, String> {
    let mut scan = Scan::default();
    let mut files = Vec::new();
    for p in packages.iter().filter(|p| API_CRATES.contains(&p.name.as_str())) {
        if let Some(lib) = p.targets.iter().find(|t| t.lib) {
            for mut s in sources(lib)? {
                expand_local_macros(&mut s.file.items);
                files.push((lib.krate.clone(), s));
            }
        }
    }
    // (first what every file says of futures and overloads, which the items then read)
    for (_, s) in &files {
        prescan(&s.file.items, &mut scan);
    }
    for (krate, s) in &files {
        let module = s.module.join("::");
        let cx = Cx { krate, module: &module, public: s.public };
        collect(&s.file.items, &cx, &mut scan);
    }
    let mut items = scan.items;
    items.sort();
    items.dedup();
    Ok(render_kotlin(&items, &scan.derefs, &scan.types))
}

/// Expands, in place, the invocations of a file's own `macro_rules!` that make items from a type's name: a macro of
/// one arm taking `$name:ident` (`transforms!(DrawScope);`, the transforms of each drawing scope). What such a
/// macro makes is the API as much as what is written out; other macros' items aren't read.
fn expand_local_macros(items: &mut Vec<Item>) {
    let mut macros: BTreeMap<String, (String, TokenStream)> = BTreeMap::new();
    for item in items.iter() {
        if let Item::Macro(m) = item
            && m.mac.path.is_ident("macro_rules")
            && let Some(name) = &m.ident
            && let Some(arm) = ident_arm(m.mac.tokens.clone())
        {
            macros.insert(name.to_string(), arm);
        }
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        match item {
            Item::Macro(m) if m.ident.is_none() => {
                let name = m.mac.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                let arg: Vec<TokenTree> = m.mac.tokens.clone().into_iter().collect();
                let expanded = match (macros.get(&name), arg.as_slice()) {
                    (Some((param, body)), [TokenTree::Ident(ty)]) => {
                        syn::parse2::<syn::File>(substitute(body.clone(), param, ty)).ok()
                    }
                    _ => None,
                };
                match expanded {
                    Some(file) => out.extend(file.items),
                    None => out.push(Item::Macro(m)),
                }
            }
            Item::Mod(mut m) => {
                if let Some((_, inner)) = &mut m.content {
                    expand_local_macros(inner);
                }
                out.push(Item::Mod(m));
            }
            other => out.push(other),
        }
    }
    *items = out;
}

/// A `macro_rules!` body of one arm `($name:ident) => { … }`: the parameter's name and the arm's body.
fn ident_arm(tokens: TokenStream) -> Option<(String, TokenStream)> {
    let tts: Vec<TokenTree> = tokens.into_iter().collect();
    let (matcher, body) = match tts.as_slice() {
        [TokenTree::Group(m), TokenTree::Punct(eq), TokenTree::Punct(gt), TokenTree::Group(b), rest @ ..]
            if eq.as_char() == '='
                && gt.as_char() == '>'
                && rest.iter().all(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ';')) =>
        {
            (m.stream(), b.stream())
        }
        _ => return None,
    };
    let m: Vec<TokenTree> = matcher.into_iter().collect();
    match m.as_slice() {
        [TokenTree::Punct(d), TokenTree::Ident(name), TokenTree::Punct(c), TokenTree::Ident(kind)]
            if d.as_char() == '$' && c.as_char() == ':' && kind == "ident" =>
        {
            Some((name.to_string(), body))
        }
        _ => None,
    }
}

/// `body` with `$param` made `ty`.
fn substitute(body: TokenStream, param: &str, ty: &proc_macro2::Ident) -> TokenStream {
    let mut out = Vec::new();
    let mut tts = body.into_iter().peekable();
    while let Some(t) = tts.next() {
        match t {
            TokenTree::Punct(p)
                if p.as_char() == '$' && matches!(tts.peek(), Some(TokenTree::Ident(i)) if i == param) =>
            {
                tts.next();
                out.push(TokenTree::Ident(ty.clone()));
            }
            TokenTree::Group(g) => {
                let mut ng = proc_macro2::Group::new(g.delimiter(), substitute(g.stream(), param, ty));
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
            }
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}

thread_local! {
    /// The fun interfaces found (traits implemented for the closures of a signature): the closures' bounds.
    static FUN_INTERFACES: std::cell::RefCell<BTreeMap<String, Vec<TypeParamBound>>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
}

fn prescan(items: &[Item], scan: &mut Scan) {
    for item in items {
        match item {
            Item::Impl(imp) => {
                if let Some((path, _)) = &imp.trait_ {
                    let trait_name = path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                    if trait_name == "Future" {
                        scan.futures.insert(type_name(&imp.self_ty));
                    }
                    if trait_name == "Deref"
                        && let Some(target) = imp.items.iter().find_map(|i| match i {
                            syn::ImplItem::Type(t) if t.ident == "Target" => Some(type_name(&t.ty)),
                            _ => None,
                        })
                    {
                        scan.derefs.insert((type_name(&imp.self_ty), target));
                    }
                    if trait_name.ends_with("Args") {
                        scan.overloads
                            .entry(trait_name.clone())
                            .or_default()
                            .push(((*imp.self_ty).clone(), kotlin_doc(&imp.attrs)));
                    }
                    // A Kotlin fun interface (`fun interface FlexBoxConfig`): a trait implemented for the closures of
                    // one signature (`impl<F: Fn(&mut FlexBoxConfigScope)> FlexBoxConfig for F`) takes a lambda.
                    if let Type::Path(self_ty) = &*imp.self_ty
                        && self_ty.path.segments.len() == 1
                        && let Some(bounds) = imp.generics.params.iter().find_map(|p| match p {
                            syn::GenericParam::Type(tp) if self_ty.path.segments[0].ident == tp.ident => {
                                Some(tp.bounds.iter().cloned().collect::<Vec<_>>())
                            }
                            _ => None,
                        })
                        && bounds.iter().any(|b| matches!(b, TypeParamBound::Trait(t) if t.path.segments.last().is_some_and(|s| s.ident == "Fn")))
                    {
                        FUN_INTERFACES.with(|f| f.borrow_mut().insert(trait_name, bounds));
                    }
                }
            }
            Item::Type(t) if is_pub(&t.vis) && boxed_future_output(&t.ty).is_some() => {
                let params = t
                    .generics
                    .params
                    .iter()
                    .filter_map(|p| match p {
                        syn::GenericParam::Type(tp) => Some(tp.ident.to_string()),
                        _ => None,
                    })
                    .collect();
                let output = boxed_future_output(&t.ty).expect("a boxed future");
                scan.boxed_futures.insert(t.ident.to_string(), (params, output));
            }
            Item::Type(t) if t.generics.params.is_empty() && is_pub(&t.vis) => {
                let mut f = FindFn(false);
                f.visit_type(&t.ty);
                if f.0 {
                    let entry = scan.aliases.entry(t.ident.to_string()).or_insert_with(|| Some((*t.ty).clone()));
                    if entry
                        .as_ref()
                        .is_some_and(|known| known.to_token_stream().to_string() != t.ty.to_token_stream().to_string())
                    {
                        *entry = None;
                    }
                }
            }
            Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    prescan(inner, scan);
                }
            }
            _ => {}
        }
    }
}

/// Finds a closure trait (`Fn`, `FnMut`, `FnOnce`) in a type.
struct FindFn(bool);

impl<'ast> Visit<'ast> for FindFn {
    fn visit_path_segment(&mut self, s: &'ast syn::PathSegment) {
        if matches!(s.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce")
            && matches!(s.arguments, syn::PathArguments::Parenthesized(_))
        {
            self.0 = true;
        }
        syn::visit::visit_path_segment(self, s);
    }
}

/// `ty` with the closures' aliases it names written out (`Option<Callback>` → `Option<Rc<dyn Fn()>>`): what the
/// generator reads of a lambda parameter.
fn expand_aliases(ty: &Type, aliases: &BTreeMap<String, Option<Type>>) -> Type {
    struct Expand<'a>(&'a BTreeMap<String, Option<Type>>, u8);
    impl syn::visit_mut::VisitMut for Expand<'_> {
        fn visit_type_mut(&mut self, t: &mut Type) {
            if let Type::Path(p) = t
                && p.qself.is_none()
                && let Some(last) = p.path.segments.last()
                && last.arguments.is_empty()
                && let Some(Some(target)) = self.0.get(&last.ident.to_string())
                && self.1 < 4
            {
                *t = target.clone();
                self.1 += 1;
                self.visit_type_mut(t);
                self.1 -= 1;
                return;
            }
            syn::visit_mut::visit_type_mut(self, t);
        }
    }
    let mut out = ty.clone();
    syn::visit_mut::VisitMut::visit_type_mut(&mut Expand(aliases, 0), &mut out);
    out
}

struct Cx<'a> {
    krate: &'a str,
    module: &'a str,
    public: bool,
}

fn is_pub(v: &Visibility) -> bool {
    matches!(v, Visibility::Public(_))
}

fn collect(items: &[Item], cx: &Cx, scan: &mut Scan) {
    for item in items {
        match item {
            Item::Fn(f) if is_pub(&f.vis) && !hidden(&f.attrs) => {
                let it = function(cx, &f.attrs, &f.sig, "", "", scan);
                scan.items.extend(it);
            }
            Item::Const(c) if is_pub(&c.vis) && !hidden(&c.attrs) => {
                scan.items.push(value(cx, &c.ident, &c.ty, "Value", ""));
            }
            Item::Static(s) if is_pub(&s.vis) && !hidden(&s.attrs) => {
                scan.items.push(value(cx, &s.ident, &s.ty, "Value", ""));
            }
            Item::Enum(e) if is_pub(&e.vis) && !hidden(&e.attrs) => {
                let owner = e.ident.to_string();
                for v in &e.variants {
                    let params = match &v.fields {
                        Fields::Unit => Vec::new(),
                        Fields::Unnamed(u) => {
                            u.unnamed.iter().enumerate().map(|(i, f)| plain_param(&i.to_string(), &f.ty)).collect()
                        }
                        Fields::Named(n) => n
                            .named
                            .iter()
                            .map(|f| plain_param(&unraw(f.ident.as_ref().expect("named")), &f.ty))
                            .collect(),
                    };
                    let mut it = base(cx, &v.ident.to_string(), &v.ident.to_string(), "Variant", &owner);
                    it.params = params;
                    it.ret = owner.clone();
                    scan.items.push(it);
                }
            }
            Item::Struct(s) if is_pub(&s.vis) && !hidden(&s.attrs) => {
                if let Some(kotlin) = kotlin_doc(&s.attrs).filter(|d| !d.contains('(')) {
                    scan.types.insert((kotlin, s.ident.to_string()));
                }
                properties(cx, s, scan)
            }
            Item::Trait(t) if is_pub(&t.vis) && !hidden(&t.attrs) => {
                let via = t.ident.to_string();
                // (a fun interface: its SAM constructor takes the lambda its closures are; one of `Fn()` closures,
                // `Content`, stands for Kotlin's `() -> Unit`, a function type, which has none)
                if FUN_INTERFACES.with(|f| f.borrow().contains_key(&via)) {
                    let ty: Type = syn::parse_str(&format!("impl {via}")).expect("a trait's `impl` type");
                    let mut block = plain_param("block", &ty);
                    block.lambda = lambda(&ty, &BTreeMap::new(), &scan.boxed_futures);
                    if block.lambda.as_ref().is_some_and(|l| !l.inputs.is_empty() || l.output != "()" || l.suspend) {
                        let mut it = base(cx, &via, &via, "FunInterface", "");
                        it.params = vec![block];
                        it.ret = via.clone();
                        scan.items.push(it);
                    }
                }
                for ti in &t.items {
                    if let TraitItem::Fn(f) = ti
                        && !hidden(&f.attrs)
                    {
                        let it = function(cx, &f.attrs, &f.sig, "", &via, scan);
                        scan.items.extend(it);
                    }
                }
            }
            Item::Impl(imp) if imp.trait_.is_none() => impl_block(cx, imp, scan),
            Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let name = unraw(&m.ident);
                    let module = if cx.module.is_empty() { name } else { format!("{}::{name}", cx.module) };
                    let inner_cx = Cx { krate: cx.krate, module: &module, public: cx.public && is_pub(&m.vis) };
                    collect(inner, &inner_cx, scan);
                }
            }
            _ => {}
        }
    }
}

fn base(cx: &Cx, kotlin: &str, rust: &str, kind: &'static str, owner: &str) -> KItem {
    let mut flags = BTreeSet::new();
    if cx.public {
        flags.insert("REACHABLE");
    }
    KItem {
        kotlin: kotlin.to_string(),
        krate: cx.krate.to_string(),
        module: cx.module.to_string(),
        owner: owner.to_string(),
        rust: rust.to_string(),
        kind,
        via: String::new(),
        receiver: "None",
        params: Vec::new(),
        ret: String::new(),
        flags,
    }
}

fn value(cx: &Cx, ident: &syn::Ident, ty: &Type, kind: &'static str, owner: &str) -> KItem {
    let name = unraw(ident);
    let mut it = base(cx, &name, &name, kind, owner);
    it.ret = text(ty.to_token_stream());
    it
}

fn plain_param(name: &str, ty: &Type) -> KParam {
    KParam {
        rust: name.to_string(),
        kotlin: camel(name),
        ty: text(ty.to_token_stream()),
        default: None,
        vararg: false,
        lambda: None,
    }
}

/// `#[derive(KotlinProperties)]`'s getters and setters, as the derive makes them.
fn properties(cx: &Cx, s: &syn::ItemStruct, scan: &mut Scan) {
    let derives = s.attrs.iter().any(|a| {
        a.path().is_ident("derive")
            && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string().contains("KotlinProperties"))
    });
    let Fields::Named(fields) = &s.fields else { return };
    if !derives {
        return;
    }
    let kotlin_opt = |attrs: &[Attribute], what: &str| {
        attrs.iter().any(|a| {
            a.path().is_ident("kotlin") && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string().contains(what))
        })
    };
    let all_set = kotlin_opt(&s.attrs, "set");
    let owner = s.ident.to_string();
    for f in &fields.named {
        if !is_pub(&f.vis) || kotlin_opt(&f.attrs, "skip") {
            continue;
        }
        let name = unraw(f.ident.as_ref().expect("named"));
        let mut it = base(cx, &camel(&name), &name, "Property", &owner);
        it.receiver = "Ref";
        it.ret = text(f.ty.to_token_stream());
        if kotlin_opt(&f.attrs, "ref") {
            it.ret = format!("&{}", it.ret);
        }
        if all_set || kotlin_opt(&f.attrs, "set") {
            it.flags.insert("SETTER");
        }
        scan.items.push(it);
    }
}

fn impl_block(cx: &Cx, imp: &ItemImpl, scan: &mut Scan) {
    let named = named_trait(&imp.attrs);
    let via = named.clone().flatten().unwrap_or_default();
    // (a blanket impl, `impl<S: AsRef<str>> S`: of any type)
    let blanket = matches!(&*imp.self_ty, Type::Path(p) if p.path.segments.len() == 1
        && imp.generics.type_params().any(|g| g.ident == p.path.segments[0].ident));
    let owner = if blanket { String::new() } else { type_name(&imp.self_ty) };
    for item in &imp.items {
        match item {
            ImplItem::Fn(f) => {
                // (an extension trait's methods have no visibility of their own: the trait's)
                let visible = if named.as_ref().is_some_and(|n| n.is_some()) { true } else { is_pub(&f.vis) };
                if !visible || hidden(&f.attrs) || f.sig.ident.to_string().starts_with("__") {
                    continue;
                }
                let mut items = function(cx, &f.attrs, &f.sig, &owner, &via, scan);
                if named.is_some() {
                    for it in &mut items {
                        it.flags.insert("NAMED");
                    }
                }
                // (the impl's generics bound what its methods' lambdas take)
                scan.items.extend(items);
            }
            ImplItem::Const(c) if is_pub(&c.vis) && !hidden(&c.attrs) => {
                scan.items.push(value(cx, &c.ident, &c.ty, "Constant", &owner));
            }
            _ => {}
        }
    }
}

/// The bounds a function's generics put on each of its type parameters.
fn bounds_of(generics: &Generics) -> BTreeMap<String, Vec<TypeParamBound>> {
    let mut out: BTreeMap<String, Vec<TypeParamBound>> = BTreeMap::new();
    for p in &generics.params {
        if let GenericParam::Type(t) = p {
            out.entry(t.ident.to_string()).or_default().extend(t.bounds.iter().cloned());
        }
    }
    if let Some(w) = &generics.where_clause {
        for pred in &w.predicates {
            if let WherePredicate::Type(pt) = pred
                && let Type::Path(p) = &pt.bounded_ty
                && p.path.segments.len() == 1
            {
                out.entry(p.path.segments[0].ident.to_string()).or_default().extend(pt.bounds.iter().cloned());
            }
        }
    }
    out
}

/// A function or method as items: itself, and, for one taking Kotlin's overloads (`args: impl XArgs`), one item per
/// overload.
fn function(cx: &Cx, attrs: &[Attribute], sig: &Signature, owner: &str, via: &str, scan: &Scan) -> Vec<KItem> {
    let rust = unraw(&sig.ident);
    if rust.starts_with("__") {
        return Vec::new();
    }
    let doc = kotlin_doc(attrs).and_then(|d| kotlin_sig(&d));
    let kotlin = doc.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| camel(&rust));
    let has_self = matches!(sig.inputs.first(), Some(FnArg::Receiver(_)));
    let kind = if has_self { "Method" } else { "Function" };
    let mut it = base(cx, &kotlin, &rust, kind, owner);
    it.via = via.to_string();
    it.receiver = match sig.inputs.first() {
        Some(FnArg::Receiver(r)) => match &r.kind {
            syn::ReceiverKind::Reference(_, _, Some(_)) => "RefMut",
            syn::ReceiverKind::Reference(_, _, None) => "Ref",
            _ => "Value",
        },
        _ => "None",
    };
    let bounds = bounds_of(&sig.generics);
    let typed: Vec<&syn::PatType> = sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) => Some(pt),
            FnArg::Receiver(_) => None,
        })
        .collect();
    for (i, pt) in typed.iter().enumerate() {
        let name = match &*pt.pat {
            syn::Pat::Ident(pi) => unraw(&pi.ident),
            other => text(other.to_token_stream()),
        };
        let ty = expand_aliases(&pt.ty, &scan.aliases);
        let mut p = plain_param(&name, &ty);
        if let Some(d) = param_default(&pt.attrs) {
            p.default = Some((false, d));
        }
        p.lambda = lambda(&ty, &bounds, &scan.boxed_futures);
        if let Some(doc) = &doc {
            // (by name where the doc names it, else by place)
            let found = doc
                .params
                .iter()
                .find(|k| k.0 == p.kotlin)
                .or_else(|| (doc.params.len() == typed.len()).then(|| &doc.params[i]));
            if let Some((kname, _, kdefault, vararg)) = found {
                p.kotlin = kname.clone();
                p.vararg = *vararg;
                if p.default.is_none() {
                    p.default = kdefault.clone().map(|d| (true, d));
                }
            }
        }
        it.params.push(p);
    }
    it.ret = match &sig.output {
        ReturnType::Default => "()".to_string(),
        ReturnType::Type(_, t) => text(t.to_token_stream()),
    };
    if sig.asyncness.is_some() || returns_future(&sig.output, &bounds, &scan.futures) {
        it.flags.insert("SUSPEND");
    }
    if sig.constness.is_some() {
        it.flags.insert("CONST");
    }
    if has_attr(attrs, "kompozable") {
        it.flags.insert("KOMPOZABLE");
        let inline = attrs.iter().any(|a| {
            attr_name(a) == "kompozable" && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string() == "inline")
        });
        if inline {
            it.flags.insert("INLINE");
        }
    }
    if has_attr(attrs, "named") || it.params.iter().any(|p| p.default.as_ref().is_some_and(|d| !d.0)) {
        it.flags.insert("NAMED");
    }
    let mut out = Vec::new();
    // (Kotlin's overloads, as a tuple: `fn Color(args: impl ColorArgs)`)
    if let [only] = typed.as_slice()
        && let Some(trait_name) = impl_trait_name(&only.ty).or_else(|| generic_trait_name(&only.ty, &bounds))
        && trait_name.ends_with("Args")
        && let Some(overloads) = scan.overloads.get(&trait_name)
    {
        it.flags.insert("OVERLOADED");
        // (Kotlin's are the ones a doc line names: the others are Rust's conveniences)
        for (ty, doc) in overloads.iter().filter(|(_, d)| d.is_some()) {
            let elems: Vec<&Type> = match ty {
                Type::Tuple(t) => t.elems.iter().collect(),
                other => vec![other],
            };
            let sig = doc.as_deref().and_then(kotlin_sig);
            let mut o = base(cx, sig.as_ref().map_or(kotlin.as_str(), |s| s.name.as_str()), &rust, "Overload", owner);
            o.via = trait_name.clone();
            o.ret = it.ret.clone();
            for (i, t) in elems.iter().enumerate() {
                let kname = sig.as_ref().and_then(|s| s.params.get(i));
                let mut p = plain_param(&format!("{i}"), t);
                if let Some((n, _, d, vararg)) = kname {
                    p.kotlin = n.clone();
                    p.default = d.clone().map(|d| (true, d));
                    p.vararg = *vararg;
                }
                p.lambda = lambda(t, &BTreeMap::new(), &scan.boxed_futures);
                o.params.push(p);
            }
            out.push(o);
        }
    }
    out.insert(0, it);
    out
}

/// A generic parameter's trait: `A` of `fn f<A: AnimatableArgs>(args: A)` → `AnimatableArgs` (what an overloaded
/// function whose return type depends on its arguments takes).
fn generic_trait_name(ty: &Type, bounds: &BTreeMap<String, Vec<TypeParamBound>>) -> Option<String> {
    let Type::Path(p) = ty else { return None };
    if p.path.segments.len() != 1 {
        return None;
    }
    bounds.get(&p.path.segments[0].ident.to_string())?.iter().find_map(|b| match b {
        TypeParamBound::Trait(t) => t.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    })
}

/// `impl Name` → `Name`.
fn impl_trait_name(ty: &Type) -> Option<String> {
    let Type::ImplTrait(it) = ty else { return None };
    it.bounds.iter().find_map(|b| match b {
        TypeParamBound::Trait(t) => t.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    })
}

/// Whether a function gives a future: `impl Future<…>`, a generic one, or a type that is one.
fn returns_future(
    output: &ReturnType,
    bounds: &BTreeMap<String, Vec<TypeParamBound>>,
    futures: &BTreeSet<String>,
) -> bool {
    let ReturnType::Type(_, t) = output else { return false };
    match &**t {
        Type::ImplTrait(it) => future_output(&it.bounds.iter().cloned().collect::<Vec<_>>()).is_some(),
        Type::Path(p) => {
            let last = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            futures.contains(&last)
                || (p.path.segments.len() == 1 && bounds.get(&last).is_some_and(|b| future_output(b).is_some()))
        }
        _ => false,
    }
}

/// The `Output` a `Future` bound names (`()` when it names none).
fn future_output(bounds: &[TypeParamBound]) -> Option<String> {
    bounds.iter().find_map(|b| {
        let TypeParamBound::Trait(t) = b else { return None };
        let seg = t.path.segments.last()?;
        if seg.ident != "Future" {
            return None;
        }
        if let syn::PathArguments::AngleBracketed(a) = &seg.arguments {
            for arg in &a.args {
                if let syn::GenericArgument::AssocType(at) = arg
                    && at.ident == "Output"
                {
                    return Some(text(at.ty.to_token_stream()));
                }
            }
        }
        Some("()".to_string())
    })
}

/// What a parameter's type says of a lambda: `impl Fn(A) -> R`, a generic `F: FnMut(…)`, `&dyn Fn…`,
/// `Box`/`Rc`/`Arc<dyn Fn…>`, `Option<…>` of one; a future (`impl Future<Output = ()>`: a suspending block).
fn lambda(ty: &Type, bounds: &BTreeMap<String, Vec<TypeParamBound>>, boxed: &BoxedFutures) -> Option<KLambda> {
    match ty {
        Type::ImplTrait(it) => from_bounds(&it.bounds.iter().cloned().collect::<Vec<_>>(), bounds, boxed),
        Type::TraitObject(o) => from_bounds(&o.bounds.iter().cloned().collect::<Vec<_>>(), bounds, boxed),
        Type::Reference(r) => lambda(&r.elem, bounds, boxed),
        Type::Paren(p) => lambda(&p.elem, bounds, boxed),
        Type::Path(p) => {
            let seg = p.path.segments.last()?;
            let name = seg.ident.to_string();
            if p.path.segments.len() == 1
                && let Some(b) = bounds.get(&name)
            {
                return from_bounds(b, bounds, boxed);
            }
            if ["Option", "Box", "Rc", "Arc"].contains(&name.as_str())
                && let syn::PathArguments::AngleBracketed(a) = &seg.arguments
                && let Some(syn::GenericArgument::Type(inner)) = a.args.first()
            {
                return lambda(inner, bounds, boxed);
            }
            None
        }
        _ => None,
    }
}

/// The aliases of boxed futures, by name: their type parameters and their future's output.
type BoxedFutures = BTreeMap<String, (Vec<String>, Type)>;

/// The output of a boxed future's type: `Pin<Box<dyn Future<Output = T> + 'a>>`'s `T`.
fn boxed_future_output(ty: &Type) -> Option<Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Pin" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(a) = &seg.arguments else { return None };
    let Some(syn::GenericArgument::Type(Type::Path(b))) = a.args.first() else { return None };
    let bseg = b.path.segments.last()?;
    if bseg.ident != "Box" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(ba) = &bseg.arguments else { return None };
    let Some(syn::GenericArgument::Type(Type::TraitObject(o))) = ba.args.first() else { return None };
    let output = future_output(&o.bounds.iter().cloned().collect::<Vec<_>>())?;
    syn::parse_str::<Type>(&output).ok()
}

/// The output of a boxed future a lambda gives (`Pin<Box<dyn Future<Output = T>>>`, or an alias of one with its
/// type arguments put in), as text.
fn boxed_output(ty: &Type, boxed: &BoxedFutures) -> Option<String> {
    if let Some(o) = boxed_future_output(ty) {
        return Some(text(o.to_token_stream()));
    }
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    let (params, output) = boxed.get(&seg.ident.to_string())?;
    let args: Vec<&Type> = match &seg.arguments {
        syn::PathArguments::AngleBracketed(a) => a
            .args
            .iter()
            .filter_map(|g| match g {
                syn::GenericArgument::Type(t) => Some(t),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    // (the output a type parameter: the argument in its place)
    let out = text(output.to_token_stream());
    Some(match params.iter().position(|p| *p == out) {
        Some(i) => args.get(i).map_or(out, |a| text(a.to_token_stream())),
        None => out,
    })
}

fn from_bounds(
    bs: &[TypeParamBound],
    generic: &BTreeMap<String, Vec<TypeParamBound>>,
    boxed_futures: &BoxedFutures,
) -> Option<KLambda> {
    if let Some(output) = future_output(bs) {
        return Some(KLambda {
            kind: "Future",
            inputs: Vec::new(),
            output,
            receiver: String::new(),
            suspend: true,
            boxed: false,
        });
    }
    bs.iter().find_map(|b| {
        let TypeParamBound::Trait(t) = b else { return None };
        let seg = t.path.segments.last()?;
        let kind = match seg.ident.to_string().as_str() {
            "Fn" => "Fn",
            "FnMut" => "FnMut",
            "FnOnce" => "FnOnce",
            // (a fun interface: the lambda its closures are)
            name => {
                let fun = FUN_INTERFACES.with(|f| f.borrow().get(name).cloned())?;
                return from_bounds(&fun, generic, boxed_futures);
            }
        };
        let syn::PathArguments::Parenthesized(args) = &seg.arguments else { return None };
        let inputs: Vec<String> = args.inputs.iter().map(|a| text(a.ty.to_token_stream())).collect();
        let (mut output, mut suspend, mut boxed) = match &args.output {
            ReturnType::Default => ("()".to_string(), false, false),
            ReturnType::Type(_, t) => (text(t.to_token_stream()), false, false),
        };
        // (a lambda giving a future suspends: `impl Fn(PointerInputScope) -> impl Future<Output = ()>`, `Fut`)
        if let ReturnType::Type(_, t) = &args.output {
            let fut = match &**t {
                Type::ImplTrait(it) => future_output(&it.bounds.iter().cloned().collect::<Vec<_>>()),
                Type::Path(p) if p.path.segments.len() == 1 => {
                    generic.get(&p.path.segments[0].ident.to_string()).and_then(|b| future_output(b))
                }
                _ => None,
            };
            if let Some(o) = fut {
                output = o;
                suspend = true;
            } else if let Some(o) = boxed_output(t, boxed_futures) {
                // (a boxed one: `-> Pin<Box<dyn Future<Output = ()>>>`, `-> LocalFuture<'static, ()>`)
                output = o;
                suspend = true;
                boxed = true;
            }
        }
        let receiver = args.inputs.first().map(|a| receiver_of(&a.ty)).unwrap_or_default();
        Some(KLambda { kind, inputs, output, receiver, suspend, boxed })
    })
}

/// A lambda's receiver: its first parameter's type when that is a scope (`&mut DrawScope`, `PointerInputScope`,
/// `&mut SemanticsPropertyReceiver`, a builder, `FocusProperties`, a DSL's config (`&mut KeyframesSpecConfig<T>`), an
/// `&Animatable` (its `animateTo` block); `&mut SemanticsConfiguration`, which is Compose's
/// SemanticsPropertyReceiver; `&mut TextFieldBuffer`, a text field state's `edit` and its transformations' block),
/// as Kotlin's `T.() -> R` is.
fn receiver_of(first: &Type) -> String {
    let inner = match first {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    let Type::Path(p) = inner else { return String::new() };
    let name = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
    let scope = ["Scope", "Receiver", "Builder", "Properties", "Transform", "Config"].iter().any(|s| name.ends_with(s))
        || matches!(name.as_str(), "Path" | "Placeable" | "SemanticsConfiguration" | "Animatable" | "TextFieldBuffer");
    if scope { name } else { String::new() }
}

// ---- generated.rs ----

fn render_kotlin(items: &[KItem], derefs: &BTreeSet<(String, String)>, types: &BTreeSet<(String, String)>) -> String {
    let mut out = String::from(
        "// The API index the Kotlin syntax's generator reads: written by `cargo xtask kotlin-index` from the\n\
         // workspace's sources (don't edit by hand; `--check` in CI). Its types: `super` (index/mod.rs).\n\
         \n\
         use super::*;\n\
         \n\
         /// Every item, sorted by Kotlin name (then crate, module, owner, Rust name).\n\
         pub static ITEMS: &[Item] = &[\n",
    );
    for it in items {
        let flags = if it.flags.is_empty() {
            "0".to_string()
        } else {
            it.flags.iter().map(|f| format!("flags::{f}")).collect::<Vec<_>>().join(" | ")
        };
        let params: Vec<String> = it.params.iter().map(render_param).collect();
        let _ = writeln!(
            out,
            "    Item {{ kotlin: {:?}, rust: {:?}, kind: Kind::{}, krate: {:?}, module: {:?}, owner: {:?}, via: {:?}, \
             receiver: Receiver::{}, params: &[{}], ret: {:?}, flags: {} }},",
            it.kotlin,
            it.rust,
            it.kind,
            it.krate,
            it.module,
            it.owner,
            it.via,
            it.receiver,
            params.join(", "),
            it.ret,
            flags
        );
    }
    out.push_str(
        "];\n\n/// What a type's values have of another's members (`impl Deref for X { type Target = Y; }`): (X, Y).\n",
    );
    out.push_str("pub static DEREFS: &[(&str, &str)] = &[\n");
    for (from, to) in derefs {
        let _ = writeln!(out, "    ({from:?}, {to:?}),");
    }
    out.push_str("];\n\n/// Kotlin's names of types Rust names otherwise (`Arrangement.Absolute` → `AbsoluteArrangement`): (Kotlin, Rust).\n");
    out.push_str("pub static TYPES: &[(&str, &str)] = &[\n");
    for (kotlin, rust) in types {
        let _ = writeln!(out, "    ({kotlin:?}, {rust:?}),");
    }
    out.push_str("];\n");
    out
}

fn render_param(p: &KParam) -> String {
    let default = match &p.default {
        None => "Default::None".to_string(),
        Some((false, d)) => format!("Default::Rust({d:?})"),
        Some((true, d)) => format!("Default::Kotlin({d:?})"),
    };
    let lambda = match &p.lambda {
        None => "None".to_string(),
        Some(l) => format!(
            "Some(Lambda {{ kind: LambdaKind::{}, inputs: &[{}], output: {:?}, receiver: {:?}, suspend: {}, boxed: {} }})",
            l.kind,
            l.inputs.iter().map(|i| format!("{i:?}")).collect::<Vec<_>>().join(", "),
            l.output,
            l.receiver,
            l.suspend,
            l.boxed
        ),
    };
    format!(
        "Param {{ rust: {:?}, kotlin: {:?}, ty: {:?}, default: {default}, vararg: {}, lambda: {lambda} }}",
        p.rust, p.kotlin, p.ty, p.vararg
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_written_as_text() {
        let ty: Type = syn::parse_str("impl Fn(&mut DrawScope<'_>, i32) -> Option<Vec<T>> + 'static").unwrap();
        assert_eq!(text(ty.to_token_stream()), "impl Fn(&mut DrawScope<'_>, i32) -> Option<Vec<T>> + 'static");
        let ty: Type = syn::parse_str("&'a mut [u8; 4]").unwrap();
        assert_eq!(text(ty.to_token_stream()), "&'a mut [u8; 4]");
        let e: Expr = syn::parse_str("Arc::new(RectangleShape)").unwrap();
        assert_eq!(text(e.to_token_stream()), "Arc::new(RectangleShape)");
        let e: Expr = syn::parse_str("|t: &T| t.to_string()").unwrap();
        assert_eq!(text(e.to_token_stream()), "| t: &T | t.to_string()");
    }

    #[test]
    fn kotlin_signatures_from_doc_lines() {
        let s = kotlin_sig(
            "joinToString(separator: CharSequence = \", \", limit: Int = -1, transform: ((T) -> CharSequence)? = null): String",
        )
        .unwrap();
        assert_eq!(s.name, "joinToString");
        let names: Vec<_> = s.params.iter().map(|p| (p.0.as_str(), p.2.as_deref())).collect();
        assert_eq!(names, [("separator", Some("\", \"")), ("limit", Some("-1")), ("transform", Some("null"))]);
        let s = kotlin_sig("listOf(vararg elements: T): List<T>").unwrap();
        assert!(s.params[0].3);
        let s = kotlin_sig("PaddingValues.Absolute(left: Dp = 0.dp)").unwrap();
        assert_eq!(s.name, "PaddingValues.Absolute");
        assert_eq!(s.params[0].2.as_deref(), Some("0.dp"));
        assert_eq!(camel("join_to_string"), "joinToString");
        assert_eq!(camel("r#type"), "type");
        assert_eq!(camel("LaunchedEffect"), "LaunchedEffect");
    }

    #[test]
    fn a_fixture_scanned() {
        let file: syn::File = syn::parse_str(
            r#"
            #[kompozable]
            pub fn Canvas(modifier: Modifier, #[default(None)] content_description: Option<String>,
                          on_draw: impl Fn(&mut DrawScope) + 'static) {}
            #[named(trait FoundationLayoutModifiers)]
            impl Modifier {
                fn padding(self, #[default(0.dp())] start: Dp) -> Modifier { self }
                fn plain(self) -> Modifier { self }
            }
            pub fn pointer_input<K, F, Fut>(keys: K, block: F) where F: Fn(PointerInputScope) -> Fut, Fut: Future<Output = ()> {}
            pub async fn await_frame() -> i64 { 0 }
            pub fn delay(ms: i64) -> Delay { todo() }
            impl Future for Delay { type Output = (); }
            pub fn LaunchedEffect<K>(keys: K, effect: impl Future<Output = ()> + 'static) {}
            pub const Modifier: Modifier = Modifier { elements: Vec::new() };
            pub enum TextAlign { Left, Center }
            #[derive(Clone, KotlinProperties)]
            pub struct Size { pub width: f32, #[kotlin(set)] pub height: f32, private: i32 }
            /// Kotlin's.
            /// Kotlin: `Color(color: Int)`
            impl ColorArgs for i32 {}
            /// Kotlin: `Color(red: Float, green: Float, blue: Float, alpha: Float = 1f)`
            impl ColorArgs for (f32, f32, f32, f32) {}
            pub fn Color(args: impl ColorArgs) -> Color {}
            pub trait KtStr {
                /// Kotlin: `padStart(length: Int, padChar: Char = ' '): String`
                fn pad_start(&self, length: i32, pad_char: char) -> String;
            }
            impl TextFieldState {
                pub fn edit(&self, block: impl FnOnce(&mut TextFieldBuffer)) {}
            }
            "#,
        )
        .unwrap();
        let mut scan = Scan::default();
        prescan(&file.items, &mut scan);
        collect(&file.items, &Cx { krate: "k", module: "m", public: true }, &mut scan);
        let find = |k: &str| scan.items.iter().find(|i| i.kotlin == k).unwrap_or_else(|| panic!("no {k}"));
        let canvas = find("Canvas");
        assert!(canvas.flags.contains("KOMPOZABLE"));
        assert_eq!(canvas.params[1].default, Some((false, "None".to_string())));
        let on_draw = canvas.params[2].lambda.as_ref().unwrap();
        assert_eq!((on_draw.receiver.as_str(), on_draw.inputs.len()), ("DrawScope", 1));
        let padding = find("padding");
        assert_eq!((padding.owner.as_str(), padding.via.as_str()), ("Modifier", "FoundationLayoutModifiers"));
        assert!(padding.flags.contains("NAMED"));
        let pi = find("pointerInput");
        let block = pi.params[1].lambda.as_ref().unwrap();
        assert!(block.suspend && block.receiver == "PointerInputScope");
        // A text field state's edit block: `TextFieldBuffer.() -> Unit`.
        let edit = find("edit").params[0].lambda.as_ref().unwrap();
        assert_eq!(edit.receiver, "TextFieldBuffer");
        assert!(find("awaitFrame").flags.contains("SUSPEND"));
        assert!(find("delay").flags.contains("SUSPEND"));
        let effect = find("LaunchedEffect").params[1].lambda.as_ref().unwrap();
        assert!(effect.suspend && effect.kind == "Future");
        assert_eq!(find("Modifier").kind, "Value");
        assert_eq!(find("Center").owner, "TextAlign");
        assert!(find("height").flags.contains("SETTER"));
        assert!(scan.items.iter().all(|i| i.kotlin != "private"));
        let overloads: Vec<_> = scan.items.iter().filter(|i| i.kind == "Overload").collect();
        assert_eq!(overloads.len(), 2);
        assert_eq!(overloads[1].params[3].default, Some((true, "1f".to_string())));
        let pad = find("padStart");
        assert_eq!((pad.via.as_str(), pad.params[1].default.clone()), ("KtStr", Some((true, "' '".to_string()))));
    }

    #[test]
    fn derefs_nested_objects_aliases_and_local_macros() {
        let mut file: syn::File = syn::parse_str(
            r#"
            pub struct MeasureScope;
            impl std::ops::Deref for MeasureScope { type Target = Density; fn deref(&self) -> &Density { todo!() } }
            /// Kotlin: `Arrangement.Absolute`
            pub struct AbsoluteArrangement;
            pub type Callback = Rc<dyn Fn()>;
            pub type Shared = Rc<Thing>;
            pub fn clickable(on_long_click: Option<Callback>, shared: Shared) {}
            pub const fn local_of(default: fn() -> i32) -> Local { todo!() }
            macro_rules! transforms {
                ($ty:ident) => {
                    impl $ty { pub fn rotate(&mut self, degrees: f32, block: impl FnOnce(&mut $ty)) {} }
                };
            }
            transforms!(DrawScope);
            "#,
        )
        .unwrap();
        expand_local_macros(&mut file.items);
        let mut scan = Scan::default();
        prescan(&file.items, &mut scan);
        collect(&file.items, &Cx { krate: "k", module: "m", public: true }, &mut scan);
        assert!(scan.derefs.contains(&("MeasureScope".to_string(), "Density".to_string())));
        assert!(scan.types.contains(&("Arrangement.Absolute".to_string(), "AbsoluteArrangement".to_string())));
        let find = |k: &str| scan.items.iter().find(|i| i.kotlin == k).unwrap_or_else(|| panic!("no {k}"));
        let clickable = find("clickable");
        // (a closure's alias written out, its lambda found; another alias kept)
        assert_eq!(clickable.params[0].ty, "Option<Rc<dyn Fn()>>");
        assert!(clickable.params[0].lambda.is_some());
        assert_eq!(clickable.params[1].ty, "Shared");
        assert!(find("localOf").flags.contains("CONST"));
        let rotate = find("rotate");
        assert_eq!(
            (rotate.owner.as_str(), rotate.params[1].lambda.as_ref().unwrap().receiver.as_str()),
            ("DrawScope", "DrawScope")
        );
    }
}
