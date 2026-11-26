//! (stage 14 FINAL) Declared types the Rust side needs narrower than Kotlin writes them, settled on the syntax tree
//! before it is translated.
//!
//! A property declared `MutableList<T>` (or `MutableMap<K, V>`) whose value is a snapshot state list (map) —
//! `mutableStateListOf()`, `….toMutableStateList()`, or a call whose lambda gives one (`handle.saveable(…) {
//! mutableStateListOf() }`, `remember { mutableStateMapOf() }`) — is that state list in Kotlin (Compose's
//! `SnapshotStateList<T>` is a `MutableList<T>`: the property holds the one object, which its users change); Rust
//! has no subtype to hold it as, so its declared type becomes `SnapshotStateList<T>` (`SnapshotStateMap<K, V>`), the
//! type arguments as written.

use crate::ast::*;

/// The file's declarations' types settled (see the module's doc).
pub(crate) fn state_collections(file: &mut File) {
    for d in &mut file.decls {
        decl(d);
    }
}

fn decl(d: &mut Decl) {
    match d {
        Decl::Function(f) => {
            list_views(f);
            default_reads(f.params.iter_mut());
            if let Some(b) = &mut f.body {
                body(b);
            }
        }
        Decl::Property(p) => property(p),
        Decl::Class(c) => {
            if let Some(pc) = &mut c.primary_constructor {
                default_reads(pc.params.iter_mut().map(|cp| &mut cp.param));
            }
            property_views(c);
            members(&mut c.body.members)
        }
        Decl::Object(o) => members(&mut o.body.members),
        Decl::TypeAlias(_) | Decl::Failed { .. } => {}
    }
}

fn members(ms: &mut [Member]) {
    for m in ms {
        match m {
            Member::Decl(d) => decl(d),
            Member::Init(b) => block(b),
            Member::Constructor(c) => default_reads(c.params.iter_mut()),
        }
    }
}

fn body(b: &mut FunctionBody) {
    if let FunctionBody::Block(b) = b {
        block(b);
    }
}

fn block(b: &mut Block) {
    for s in &mut b.stmts {
        if let StmtKind::Decl(d) = &mut s.kind {
            decl(d);
        }
    }
}

fn property(p: &mut Property) {
    let (PropertyInit::Value(e) | PropertyInit::Delegate(e)) = &p.init else { return };
    let Some(state) = state_collection(e) else { return };
    let Some(Type { kind: TypeKind::Named(segs), nullable: false, .. }) = &mut p.ty else { return };
    let [seg] = segs.as_mut_slice() else { return };
    let to = match (seg.name.name.as_str(), state) {
        ("MutableList", "SnapshotStateList") | ("MutableMap", "SnapshotStateMap") => state,
        _ => return,
    };
    seg.name.name = to.to_string();
}

/// (stage 14 LISTS) The parameters a later one's default reads, marked ([`Param::read_by_default`]): Kotlin's `fun f(a:
/// Int, b: Int = a * 2)` — the default sees the parameters before it, by their names.
fn default_reads<'p>(params: impl Iterator<Item = &'p mut Param>) {
    let mut params: Vec<&mut Param> = params.collect();
    for i in 0..params.len() {
        let Some(d) = params[i].default.clone() else { continue };
        let mut names = Vec::new();
        crate::codegen::walk_expr(&d, &mut |e: &Expr| {
            if let ExprKind::Name(n) = &e.kind {
                names.push(n.name.clone());
            }
        });
        for p in params[..i].iter_mut() {
            if names.contains(&p.name.name) {
                p.read_by_default = true;
            }
        }
    }
}

/// (stage 14 LISTS) A parameter declared `List<T>` (`Collection<T>`, `Iterable<T>`) whose function asks whether it is
/// a `MutableList` — `x is MutableList`, `x as? MutableList<T>`, `when (x) { is MutableList -> … }`, in its body or
/// a default (`onBack: () -> Unit = { if (backstack is MutableList) backstack.removeAt(…) }`) — is the caller's list
/// itself, which may be one: KompozR's `List<T>` is a value (a `Vec`), so its declared type becomes `ListView<T>`
/// (`kt::ListView`: the caller's read-only list or mutable one), the type argument as written.
fn list_views(f: &mut Function) {
    let mut asked: Vec<String> = Vec::new();
    {
        let mut note = |e: &Expr| {
            let (inner, ty) = match &e.kind {
                ExprKind::Is { expr, ty, .. } | ExprKind::As { expr, ty, .. } => (expr, ty),
                ExprKind::When(w) => {
                    if let Some(s) = &w.subject
                        && let ExprKind::Name(n) = &s.expr.kind
                        && w.branches.iter().any(|b| {
                            b.conditions
                                .iter()
                                .any(|c| matches!(c, WhenCondition::Is { ty, .. } if is_mutable_list(ty)))
                        })
                    {
                        asked.push(n.name.clone());
                    }
                    return;
                }
                _ => return,
            };
            if let ExprKind::Name(n) = &inner.kind
                && is_mutable_list(ty)
            {
                asked.push(n.name.clone());
            }
        };
        crate::codegen::walk_body(f.body.as_ref(), &mut note);
        for p in &f.params {
            if let Some(d) = &p.default {
                crate::codegen::walk_expr(d, &mut note);
            }
        }
    }
    for p in &mut f.params {
        if !asked.contains(&p.name.name) || p.is_vararg() {
            continue;
        }
        if let Some(Type { kind: TypeKind::Named(segs), .. }) = &mut p.ty
            && let [seg] = segs.as_mut_slice()
            && matches!(seg.name.name.as_str(), "List" | "Collection" | "Iterable")
        {
            seg.name.name = "ListView".to_string();
        }
    }
}

/// (stage 14 LISTS) A class's property declared `List<T>` (`Collection<T>`, `Iterable<T>`) holding a mutable list —
/// `val items: List<T> = _items` of a `private val _items = mutableListOf<T>()`, a constructor's `MutableList`
/// parameter, a `mutableListOf(…)`/`mutableStateListOf(…)`/`toMutableList()` — is that one object in Kotlin, its
/// readers seeing it change: its declared type becomes `ListView<T>` (`kt::ListView`, the list itself), the type
/// argument as written. (A local's is settled where its value's type is known: the generator's `list_view_type`.)
fn property_views(c: &mut Class) {
    let mut mutable: Vec<String> = Vec::new();
    if let Some(pc) = &c.primary_constructor {
        for cp in &pc.params {
            if cp.param.ty.as_ref().is_some_and(|t| !t.nullable && mutable_list_type(t)) {
                mutable.push(cp.param.name.name.clone());
            }
        }
    }
    // (a property's own: declared one, or made by its value; in order, a later one reading an earlier)
    for m in &c.body.members {
        if let Member::Decl(Decl::Property(p)) = m
            && let Binding::Name(n) = &p.binding
        {
            let declared = p.ty.as_ref().is_some_and(|t| !t.nullable && mutable_list_type(t));
            let made = p.ty.is_none() && matches!(&p.init, PropertyInit::Value(e) if mutable_list_value(e, &mutable));
            if declared || made {
                mutable.push(n.name.clone());
            }
        }
    }
    for m in &mut c.body.members {
        if let Member::Decl(Decl::Property(p)) = m
            && p.getter.is_none()
            && let PropertyInit::Value(e) = &p.init
            && mutable_list_value(e, &mutable)
            && let Some(Type { kind: TypeKind::Named(segs), nullable: false, .. }) = &mut p.ty
            && let [seg] = segs.as_mut_slice()
            && matches!(seg.name.name.as_str(), "List" | "Collection" | "Iterable")
        {
            seg.name.name = "ListView".to_string();
        }
    }
}

/// Whether a declared type is a mutable list's (`MutableList`, `ArrayList`, a state list).
fn mutable_list_type(t: &Type) -> bool {
    matches!(&t.kind, TypeKind::Named(segs) if segs.len() == 1
        && matches!(segs[0].name.name.as_str(), "MutableList" | "ArrayList" | "SnapshotStateList"))
}

/// Whether an expression's value is a mutable list, as written: one of `mutable` (the class's), or a call making one
/// (a lambda's last expression for `remember { … }`).
fn mutable_list_value(e: &Expr, mutable: &[String]) -> bool {
    match &e.kind {
        ExprKind::Name(n) => mutable.contains(&n.name),
        ExprKind::Paren(inner) => mutable_list_value(inner, mutable),
        ExprKind::Call { callee, trailing_lambda, .. } => {
            let name = match &callee.kind {
                ExprKind::Name(n) => n.name.as_str(),
                ExprKind::Member { name, .. } => name.name.as_str(),
                _ => return false,
            };
            match name {
                "mutableListOf" | "arrayListOf" | "ArrayList" | "MutableList" | "mutableStateListOf"
                | "toMutableList" | "toMutableStateList" => true,
                _ => match trailing_lambda.as_deref().and_then(|l| l.body.stmts.last()) {
                    Some(Stmt { kind: StmtKind::Expr(last), .. }) => mutable_list_value(last, mutable),
                    _ => false,
                },
            }
        }
        _ => false,
    }
}

/// Whether a type is Kotlin's `MutableList` (`ArrayList`), its arguments as written or none.
fn is_mutable_list(t: &Type) -> bool {
    matches!(&t.kind, TypeKind::Named(segs) if segs.len() == 1
        && matches!(segs[0].name.name.as_str(), "MutableList" | "ArrayList"))
}

/// The state collection an expression's value is (its class's name), as written.
fn state_collection(e: &Expr) -> Option<&'static str> {
    let ExprKind::Call { callee, trailing_lambda, .. } = &e.kind else { return None };
    let name = match &callee.kind {
        ExprKind::Name(n) => n.name.as_str(),
        ExprKind::Member { name, .. } => name.name.as_str(),
        _ => return None,
    };
    match name {
        "mutableStateListOf" | "toMutableStateList" => Some("SnapshotStateList"),
        "mutableStateMapOf" | "toMutableStateMap" => Some("SnapshotStateMap"),
        // (what a lambda the call runs gives: `remember { … }`, `handle.saveable(…) { … }`)
        _ => match trailing_lambda.as_deref()?.body.stmts.last()? {
            Stmt { kind: StmtKind::Expr(last), .. } => state_collection(last),
            _ => None,
        },
    }
}
