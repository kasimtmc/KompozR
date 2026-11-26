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
            if let Some(b) = &mut f.body {
                body(b);
            }
        }
        Decl::Property(p) => property(p),
        Decl::Class(c) => members(&mut c.body.members),
        Decl::Object(o) => members(&mut o.body.members),
        Decl::TypeAlias(_) | Decl::Failed { .. } => {}
    }
}

fn members(ms: &mut [Member]) {
    for m in ms {
        match m {
            Member::Decl(d) => decl(d),
            Member::Init(b) => block(b),
            Member::Constructor(_) => {}
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
