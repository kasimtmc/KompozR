//! What a translation knows of its Kotlin, for a language server (stage 14): the declarations (the text's, with
//! their Kotlin signatures as written and their Rust names), the locals and parameters (where each is visible, its
//! type), what each name used refers to (a local, a declaration of the text or of the crate's module table, an item
//! of the API index), and the types the generator gave the names, member reads and calls. Asked for with
//! [`Options::analyze`](crate::Options::analyze); [`Translation::analysis`](crate::Translation::analysis) holds it.
//!
//! It is what the generator itself saw — its scopes, its overload choices, its inference —, so what an editor shows
//! is what the Rust was made from. Where the generator writes an expression more than once (a speculative pass whose
//! output it drops), only the pass kept is recorded.

use crate::ast::*;
use crate::codegen::names;
use crate::pos::Loc;

/// What a translation knows of its Kotlin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    /// The text's declarations, in order (nested ones after their outer one).
    pub declarations: Vec<Declaration>,
    /// Its locals and parameters, in the order declared.
    pub locals: Vec<LocalDecl>,
    /// What each name it uses refers to, in the order written.
    pub references: Vec<Reference>,
    /// The types of its names, member reads and calls, in the order written (none where the generator couldn't
    /// tell one).
    pub types: Vec<TypeAt>,
}

/// A kind of declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKind {
    Function,
    /// A `@Composable` (`@Kompozable`) function.
    Composable,
    Property,
    Class,
    DataClass,
    Object,
    Companion,
    Enum,
    EnumEntry,
    Interface,
    TypeAlias,
    /// A secondary constructor.
    Constructor,
}

/// A declaration of the text.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    /// Its Kotlin name, qualified by what it is declared in (`Catalog.Entry`, `Tally.add`, `Ledger.Companion.create`).
    pub kotlin: String,
    /// The Rust item it becomes (`CatalogEntry`, `add`, `LedgerCompanion`).
    pub rust: String,
    pub kind: DeclKind,
    /// Its name's place.
    pub name: Loc,
    /// The whole declaration's.
    pub range: Loc,
    /// Its Kotlin header as written, its whitespace one space (`fun add(n: Int = Step): Int`).
    pub detail: String,
}

/// What kind of local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Param,
    Val,
    Var,
    /// A lambda's parameter (its `it` too).
    LambdaParam,
}

/// A local or parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalDecl {
    pub name: String,
    pub kind: LocalKind,
    /// Its name's place.
    pub loc: Loc,
    /// Where it can be named: from its declaration to the end of this (its block, lambda or function).
    pub scope: Loc,
    /// Its type, as Kotlin writes it (empty when the generator couldn't tell).
    pub ty: String,
}

/// What a name refers to.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A local or parameter, by its name's place.
    Local(Loc),
    /// A declaration of the text or of the crate's module table, by its Kotlin name (qualified as
    /// [`Declaration::kotlin`]).
    Declaration(String),
    /// An item of the API index.
    Api { kotlin: String, rust: String, krate: String, module: String, owner: String },
}

/// A name used, and what it refers to.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    /// The name's place.
    pub loc: Loc,
    pub target: Target,
}

/// The type of an expression.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeAt {
    pub loc: Loc,
    /// As Kotlin writes it (`List<String>`, `Note?`, `(Int) -> String`).
    pub ty: String,
}

impl Analysis {
    /// What it has at `loc` (by its start): sorted, one of each, as [`crate::translate`] gives it.
    pub(crate) fn settle(&mut self) {
        let key = |l: &Loc| (l.start.offset, l.end.offset);
        self.locals.sort_by_key(|l| key(&l.loc));
        self.locals.dedup_by(|a, b| key(&a.loc) == key(&b.loc));
        self.references.sort_by_key(|r| key(&r.loc));
        self.references.dedup_by(|a, b| key(&a.loc) == key(&b.loc));
        self.types.sort_by_key(|t| key(&t.loc));
        self.types.dedup_by(|a, b| key(&a.loc) == key(&b.loc));
    }

    /// The innermost type recorded at `offset` (a byte offset in the text).
    pub fn type_at(&self, offset: usize) -> Option<&TypeAt> {
        self.types
            .iter()
            .filter(|t| t.loc.start.offset <= offset && offset <= t.loc.end.offset)
            .min_by_key(|t| t.loc.end.offset - t.loc.start.offset)
    }

    /// What the name at `offset` refers to.
    pub fn reference_at(&self, offset: usize) -> Option<&Reference> {
        self.references.iter().find(|r| r.loc.start.offset <= offset && offset <= r.loc.end.offset)
    }

    /// The locals that can be named at `offset`, innermost (latest declared) first; one of each name.
    pub fn locals_at(&self, offset: usize) -> Vec<&LocalDecl> {
        let mut out: Vec<&LocalDecl> = self
            .locals
            .iter()
            .filter(|l| l.loc.end.offset <= offset && offset <= l.scope.end.offset && l.scope.start.offset <= offset)
            .collect();
        out.reverse();
        let mut seen = std::collections::HashSet::new();
        out.retain(|l| seen.insert(l.name.clone()));
        out
    }
}

/// The declarations of a Kotlin text (its offset 0 at `base`) read without translating it: what a language server
/// shows of a text (its outline, a member's header in a hover, where to go to it) — [`Analysis::declarations`]'s,
/// and the properties classes declare in their primary constructors (`Note.title` of
/// `data class Note(val title: String)`, kind [`DeclKind::Property`], after their class's other members).
pub fn declarations_of(text: &str, base: crate::pos::Pos) -> Vec<Declaration> {
    let mut diags = crate::diag::Diagnostics::default();
    let tokens = crate::lexer::lex(text, base, &mut diags);
    let file = crate::parser::parse(&tokens, &mut diags);
    let mut out = declarations(&file, text);
    let mut properties = Vec::new();
    for d in &file.decls {
        constructor_properties(d, "", text, &mut properties);
    }
    out.extend(properties);
    out
}

/// The primary constructors' properties of `d` and the classes nested in it (`outer`: the Kotlin name it is in).
fn constructor_properties(d: &Decl, outer: &str, text: &str, out: &mut Vec<Declaration>) {
    let qualified = |name: &str| if outer.is_empty() { name.to_string() } else { format!("{outer}.{name}") };
    let body = match d {
        Decl::Class(c) => {
            let class = qualified(&c.name.name);
            for p in c.primary_constructor.iter().flat_map(|k| &k.params) {
                let Some(m) = p.property else { continue };
                let written = header(text, p.param.loc.start.offset, p.param.loc.end.offset);
                let keyword = if m == Mutability::Var { "var" } else { "val" };
                let detail = if written.split_whitespace().any(|w| w == keyword) {
                    written
                } else {
                    format!("{keyword} {written}")
                };
                out.push(Declaration {
                    kotlin: format!("{class}.{}", p.param.name.name),
                    rust: names::snake(&p.param.name.name),
                    kind: DeclKind::Property,
                    name: p.param.name.loc,
                    range: p.param.loc,
                    detail,
                });
            }
            (&c.body, class)
        }
        Decl::Object(o) => {
            let name = o.name.as_ref().map_or("Companion", |n| n.name.as_str());
            (&o.body, qualified(name))
        }
        _ => return,
    };
    for m in &body.0.members {
        if let Member::Decl(inner) = m {
            constructor_properties(inner, &body.1, text, out);
        }
    }
}

/// The declarations of `file` (whose text is `text`), with the Rust names the generator gives them.
pub(crate) fn declarations(file: &File, text: &str) -> Vec<Declaration> {
    let mut out = Vec::new();
    for d in &file.decls {
        declaration(d, None, text, &mut out);
    }
    out
}

/// `text` from `from` to `to` (offsets), its whitespace runs one space, without a last `=` or `{`.
fn header(text: &str, from: usize, to: usize) -> String {
    let slice = text.get(from..to.min(text.len())).unwrap_or("");
    let squashed = slice.split_whitespace().collect::<Vec<_>>().join(" ");
    squashed.trim_end_matches(['=', '{', ' ']).to_string()
}

fn declaration(d: &Decl, outer: Option<(&str, &str)>, text: &str, out: &mut Vec<Declaration>) {
    let qualify = |name: &str, rust_own: String| match outer {
        Some((k, r)) => (format!("{k}.{name}"), if rust_own.is_empty() { format!("{r}{name}") } else { rust_own }),
        None => (name.to_string(), if rust_own.is_empty() { name.to_string() } else { rust_own }),
    };
    match d {
        Decl::Function(f) => {
            let end = match &f.body {
                Some(FunctionBody::Block(b)) => b.loc.start.offset,
                Some(FunctionBody::Expr(e)) => e.loc.start.offset,
                None => f.loc.end.offset,
            };
            let rust = if f.name.name == "toString" && f.params.is_empty() && outer.is_some() {
                "to_string_kt".to_string()
            } else if outer.is_none() && names::is_pascal(&f.name.name) {
                f.name.name.clone()
            } else {
                names::snake(&f.name.name)
            };
            let kotlin = match outer {
                Some((k, _)) => format!("{k}.{}", f.name.name),
                None => f.name.name.clone(),
            };
            out.push(Declaration {
                kotlin,
                rust,
                kind: if f.is_kompozable() { DeclKind::Composable } else { DeclKind::Function },
                name: f.name.loc,
                range: f.loc,
                detail: header(text, f.loc.start.offset, end),
            });
        }
        Decl::Property(p) => {
            let Binding::Name(n) = &p.binding else { return };
            let end = match &p.init {
                PropertyInit::Value(e) | PropertyInit::Delegate(e) => e.loc.start.offset,
                _ => p.loc.end.offset,
            };
            let kotlin = match outer {
                Some((k, _)) => format!("{k}.{}", n.name),
                None => n.name.clone(),
            };
            let rust = if outer.is_some() { names::snake(&n.name) } else { names::value_name(&n.name) };
            out.push(Declaration {
                kotlin,
                rust,
                kind: DeclKind::Property,
                name: n.loc,
                range: p.loc,
                detail: header(text, p.loc.start.offset, end),
            });
        }
        Decl::Class(c) => {
            let (kotlin, rust) = qualify(&c.name.name, String::new());
            let kind = match c.kind {
                ClassKind::Enum => DeclKind::Enum,
                ClassKind::Interface | ClassKind::FunInterface => DeclKind::Interface,
                ClassKind::Class if c.is_data() => DeclKind::DataClass,
                _ => DeclKind::Class,
            };
            let end = c.body.loc.map_or(c.loc.end.offset, |b| b.start.offset);
            out.push(Declaration {
                kotlin: kotlin.clone(),
                rust: rust.clone(),
                kind,
                name: c.name.loc,
                range: c.loc,
                detail: header(text, c.loc.start.offset, end),
            });
            for e in &c.enum_entries {
                out.push(Declaration {
                    kotlin: format!("{kotlin}.{}", e.name.name),
                    rust: format!("{rust}::{}", e.name.name),
                    kind: DeclKind::EnumEntry,
                    name: e.name.loc,
                    range: e.loc,
                    detail: header(text, e.loc.start.offset, e.loc.end.offset),
                });
            }
            members(&c.body, &kotlin, &rust, text, out);
        }
        Decl::Object(o) => {
            let (kotlin, rust, kind) = if o.is_companion() {
                let (ok, or) = outer.unwrap_or(("", ""));
                let name = o.name.as_ref().map_or("Companion", |n| n.name.as_str());
                (format!("{ok}.{name}"), format!("{or}{name}"), DeclKind::Companion)
            } else {
                let Some(n) = &o.name else { return };
                let (k, r) = qualify(&n.name, String::new());
                (k, r, DeclKind::Object)
            };
            let end = o.body.loc.map_or(o.loc.end.offset, |b| b.start.offset);
            out.push(Declaration {
                kotlin: kotlin.clone(),
                rust: rust.clone(),
                kind,
                name: o.name.as_ref().map_or(o.loc, |n| n.loc),
                range: o.loc,
                detail: header(text, o.loc.start.offset, end),
            });
            members(&o.body, &kotlin, &rust, text, out);
        }
        Decl::TypeAlias(a) => {
            let (kotlin, rust) = qualify(&a.name.name, String::new());
            out.push(Declaration {
                kotlin,
                rust,
                kind: DeclKind::TypeAlias,
                name: a.name.loc,
                range: a.loc,
                detail: header(text, a.loc.start.offset, a.loc.end.offset),
            });
        }
        Decl::Failed { .. } => {}
    }
}

fn members(body: &ClassBody, kotlin: &str, rust: &str, text: &str, out: &mut Vec<Declaration>) {
    for m in &body.members {
        match m {
            Member::Decl(d) => declaration(d, Some((kotlin, rust)), text, out),
            Member::Constructor(s) => {
                let end = s.body.as_ref().map_or(s.loc.end.offset, |b| b.loc.start.offset);
                out.push(Declaration {
                    kotlin: format!("{kotlin}.constructor"),
                    rust: rust.to_string(),
                    kind: DeclKind::Constructor,
                    name: s.loc,
                    range: s.loc,
                    detail: header(text, s.loc.start.offset, end),
                });
            }
            Member::Init(_) => {}
        }
    }
}
