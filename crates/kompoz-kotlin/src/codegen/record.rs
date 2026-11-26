//! The generator's half of [`crate::analysis`]: what it records as it writes (when the translation asks for it).
//! Nothing is recorded in a speculative pass ([`Gen::quiet`]): its output is dropped, so are its facts.

use super::Gen;
use super::resolve::{LocalKind, Ty};
use crate::analysis::{self, Analysis, LocalDecl, Reference, Target, TypeAt};
use crate::ast::Ident;
use crate::index;
use crate::pos::Loc;

impl Gen<'_, '_> {
    /// Whether facts are recorded now.
    fn recording(&self) -> bool {
        self.analysis.is_some() && self.quiet_depth == 0
    }

    fn analysis_mut(&mut self) -> Option<&mut Analysis> {
        if self.quiet_depth == 0 { self.analysis.as_mut() } else { None }
    }

    /// Declares a local named by `n` (see [`Gen::declare`]), recording it and where it can be named.
    pub(crate) fn declare_at(&mut self, n: &Ident, rust: String, kind: LocalKind, ty: Ty) -> String {
        let shown = if self.recording() { kotlin_ty(&ty) } else { String::new() };
        let recorded = match kind {
            LocalKind::Param => analysis::LocalKind::Param,
            LocalKind::Var => analysis::LocalKind::Var,
            LocalKind::LambdaParam => analysis::LocalKind::LambdaParam,
            _ => analysis::LocalKind::Val,
        };
        let rust = self.declare(&n.name, rust, kind, ty);
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&n.name)) {
            l.at = Some(n.loc);
        }
        let scope = self.ranges.last().copied().or(self.decl_range).unwrap_or(n.loc);
        if let Some(a) = self.analysis_mut() {
            a.locals.push(LocalDecl { name: n.name.clone(), kind: recorded, loc: n.loc, scope, ty: shown });
        }
        rust
    }

    /// The local just recorded is a `var` (a delegated one: `var x by remember { … }`) when `mutable`.
    pub(crate) fn record_mutable(&mut self, mutable: bool) {
        if mutable && let Some(l) = self.analysis_mut().and_then(|a| a.locals.last_mut()) {
            l.kind = analysis::LocalKind::Var;
        }
    }

    /// The local `name` (of the innermost scope) declared at `at`.
    pub(crate) fn set_local_at(&mut self, name: &str, at: Option<Loc>) {
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(name)) {
            l.at = at;
        }
    }

    /// `loc`'s type, `ty` (none recorded for one the generator couldn't tell).
    pub(crate) fn record_type(&mut self, loc: Loc, ty: &Ty) {
        if !self.recording() || *ty == Ty::Unknown {
            return;
        }
        let ty = kotlin_ty(ty);
        if let Some(a) = self.analysis_mut() {
            a.types.push(TypeAt { loc, ty });
        }
    }

    /// The name at `loc` refers to `target`.
    pub(crate) fn record_ref(&mut self, loc: Loc, target: Target) {
        if let Some(a) = self.analysis_mut() {
            a.references.push(Reference { loc, target });
        }
    }

    /// The name at `loc` is the API index's `item`.
    pub(crate) fn record_item(&mut self, loc: Loc, item: &index::Item) {
        if !self.recording() {
            return;
        }
        // (the overlay's: a declaration of the text's or of the module table's — `Tally.add`, `Note`)
        if item.krate.is_empty() {
            let kotlin = if item.owner.is_empty() || item.kind == index::Kind::Function && item.owner == item.rust {
                item.kotlin.to_string()
            } else {
                format!("{}.{}", index::kotlin_type(item.owner), item.kotlin)
            };
            return self.record_ref(loc, Target::Declaration(kotlin));
        }
        self.record_ref(
            loc,
            Target::Api {
                kotlin: item.kotlin.to_string(),
                rust: item.rust.to_string(),
                krate: item.krate.to_string(),
                module: item.module.to_string(),
                owner: item.owner.to_string(),
            },
        );
    }

    /// Runs `f` with `loc` the range the locals it declares can be named in (a block, a lambda).
    pub(crate) fn in_range<T>(&mut self, loc: Loc, f: impl FnOnce(&mut Self) -> T) -> T {
        self.ranges.push(loc);
        let out = f(self);
        self.ranges.pop();
        out
    }
}

/// A type as Kotlin writes it.
pub(crate) fn kotlin_ty(t: &Ty) -> String {
    match t {
        Ty::Unknown => String::new(),
        Ty::Unit => "Unit".into(),
        Ty::Int | Ty::OtherInt => "Int".into(),
        Ty::Long => "Long".into(),
        Ty::Float => "Float".into(),
        Ty::Double => "Double".into(),
        Ty::Bool => "Boolean".into(),
        Ty::Char => "Char".into(),
        Ty::String => "String".into(),
        Ty::Nullable(t) => match &**t {
            Ty::Fun(_) => format!("({})?", kotlin_ty(t)),
            t => format!("{}?", kotlin_ty(t)),
        },
        Ty::List(t) => format!("List<{}>", arg(t)),
        Ty::Set(t) => format!("Set<{}>", arg(t)),
        Ty::Map(k, v) => format!("Map<{}, {}>", arg(k), arg(v)),
        Ty::Range(_) => "IntRange".into(),
        Ty::ClosedRange(t) => format!("ClosedRange<{}>", arg(t)),
        Ty::Pair(a, b) => format!("Pair<{}, {}>", arg(a), arg(b)),
        Ty::Fun(f) => {
            let params: Vec<String> = f.params.iter().map(arg).collect();
            let receiver = f.receiver.as_deref().map_or(String::new(), |r| format!("{}.", index::kotlin_type(r)));
            let suspend = if f.suspend { "suspend " } else { "" };
            format!("{suspend}{receiver}({}) -> {}", params.join(", "), arg(&f.ret))
        }
        // ((stage 14 LISTS) a `List` that may be mutable: Kotlin's `List`)
        Ty::Named(n, args) if n == "ListView" && args.len() == 1 => format!("List<{}>", arg(&args[0])),
        Ty::Named(n, args) => {
            let name = index::kotlin_type(n);
            if args.is_empty() {
                name.to_string()
            } else {
                format!("{name}<{}>", args.iter().map(arg).collect::<Vec<_>>().join(", "))
            }
        }
    }
}

/// A type argument (one the generator couldn't tell: Kotlin's `*`).
fn arg(t: &Ty) -> String {
    let s = kotlin_ty(t);
    if s.is_empty() { "*".into() } else { s }
}
