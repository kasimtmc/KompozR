//! What the generator knows of names: the types it infers (enough to write Rust Kotlin's way: what is copied, what
//! a `+` of strings is, which of the kt shims' traits a member is of), the scopes of locals and parameters, the
//! implicit receivers (`this` of an extension function, a receiver lambda's scope), the lambdas being written and
//! what they capture, the file's own declarations, and the API index's items as callees.
//!
//! Kotlin's resolution order is followed: a local (innermost first), then the implicit receivers' members
//! (innermost first), then the file's top-level declarations, then the API index (Kotlin's imports: what the Rust
//! side has in scope).

use crate::index::{self, Item, Kind, LambdaKind};
use std::collections::HashMap;

// ---- Types -----------------------------------------------------------------------------------------------------

/// A type as far as the generator can tell.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Unknown,
    Unit,
    Int,
    Long,
    Float,
    Double,
    Bool,
    Char,
    String,
    /// Another of Rust's numbers (`usize`, `u8`, `i16`, …): copied, an integer.
    OtherInt,
    Nullable(Box<Ty>),
    List(Box<Ty>),
    Set(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Range(Box<Ty>),
    Pair(Box<Ty>, Box<Ty>),
    Fun(Box<FunTy>),
    /// Any other type, by its Rust name (`Dp`, `Modifier`, `MutableState`) and arguments.
    Named(String, Vec<Ty>),
}

/// A function type: `(A, B) -> R`, `Scope.() -> R`, `suspend () -> R`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FunTy {
    pub params: Vec<Ty>,
    pub ret: Ty,
    /// Its receiver's Rust type name, when the Rust side passes it (`DrawScope`).
    pub receiver: Option<String>,
}

impl Ty {
    pub fn named(name: &str) -> Ty {
        Ty::Named(name.to_string(), Vec::new())
    }

    /// Whether a value of it is copied by Rust as it is (no `.clone()` needed to use it again).
    pub fn is_copy(&self) -> bool {
        match self {
            Ty::Unit | Ty::Int | Ty::Long | Ty::Float | Ty::Double | Ty::Bool | Ty::Char | Ty::OtherInt => true,
            Ty::Nullable(t) => t.is_copy(),
            Ty::Named(n, _) => {
                matches!(
                    n.as_str(),
                    "Dp" | "TextUnit" | "Offset" | "Size" | "IntSize" | "IntOffset" | "Constraints" | "Color"
                )
            }
            _ => false,
        }
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Ty::Int | Ty::Long | Ty::Float | Ty::Double | Ty::OtherInt)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Ty::Float | Ty::Double)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Ty::String)
    }

    /// Whether it leaves a part of itself unknown (`Vec<_>`).
    pub fn is_vague(&self) -> bool {
        match self {
            Ty::Unknown => true,
            Ty::List(e) | Ty::Set(e) | Ty::Range(e) | Ty::Nullable(e) => e.is_vague(),
            Ty::Map(k, v) | Ty::Pair(k, v) => k.is_vague() || v.is_vague(),
            Ty::Named(_, args) => args.iter().any(Ty::is_vague),
            Ty::Fun(_) => true,
            _ => false,
        }
    }

    pub fn is_nullable(&self) -> bool {
        matches!(self, Ty::Nullable(_))
    }

    /// What it holds when nullable; itself otherwise.
    pub fn non_null(&self) -> Ty {
        match self {
            Ty::Nullable(t) => (**t).clone(),
            t => t.clone(),
        }
    }

    pub fn nullable(self) -> Ty {
        match self {
            Ty::Nullable(_) | Ty::Unknown => self,
            t => Ty::Nullable(Box::new(t)),
        }
    }

    /// An element of it, when it is a collection or a range.
    pub fn element(&self) -> Ty {
        match self {
            Ty::List(t) | Ty::Set(t) | Ty::Range(t) => (**t).clone(),
            Ty::Named(n, a) if n == "SnapshotStateList" => a.first().cloned().unwrap_or(Ty::Unknown),
            Ty::Map(k, v) => Ty::Pair(k.clone(), v.clone()),
            Ty::String => Ty::Char,
            _ => Ty::Unknown,
        }
    }

    /// Its Rust type's name as the API index names owners (`Size`, `Modifier`); none for what has none there.
    pub fn owner(&self) -> Option<&str> {
        match self {
            Ty::Named(n, _) => Some(n),
            Ty::String => Some("String"),
            _ => None,
        }
    }

    /// The kt shims' extension traits its Kotlin members may be of, in the order they are tried.
    pub fn kt_traits(&self) -> &'static [&'static str] {
        match self {
            Ty::List(_) | Ty::Set(_) | Ty::Range(_) => {
                &["KtList", "KtMutableList", "KtListOfLists", "KtListOfNullable", "KtScope"]
            }
            // (a state list: Kotlin's MutableList functions are its own, taking `&self`; its reads are a list's)
            Ty::Named(n, _) if n == "SnapshotStateList" => &["KtList", "KtListOfLists", "KtListOfNullable", "KtScope"],
            Ty::String => &["KtStr", "KtScope"],
            Ty::Nullable(t) if t.is_string() => &["KtNullableStr", "KtScope"],
            Ty::Map(_, _) => &["KtMapExt", "KtScope"],
            Ty::Char => &["KtChar", "KtScope"],
            Ty::Float | Ty::Double => &["KtFloat", "KtNumber", "KtScope"],
            Ty::Int | Ty::Long | Ty::OtherInt => &["KtNumber", "KtScope"],
            Ty::Pair(_, _) => &["KtPair", "KtComponents", "KtComponent3", "KtTriple", "KtScope"],
            _ => &["KtScope"],
        }
    }

    /// A Rust type written as text (the API index's): what the generator needs to know of it.
    pub fn from_rust(text: &str) -> Ty {
        let mut p = TypeText { s: text.trim(), at: 0 };
        p.ty()
    }
}

/// A small reader of Rust types as the index writes them.
struct TypeText<'a> {
    s: &'a str,
    at: usize,
}

impl TypeText<'_> {
    fn rest(&self) -> &str {
        &self.s[self.at..]
    }

    fn skip_ws(&mut self) {
        while self.rest().starts_with(' ') {
            self.at += 1;
        }
    }

    fn eat(&mut self, p: &str) -> bool {
        self.skip_ws();
        if self.rest().starts_with(p) {
            self.at += p.len();
            true
        } else {
            false
        }
    }

    fn word(&mut self) -> String {
        self.skip_ws();
        let n = self.rest().find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':')).unwrap_or(self.rest().len());
        let w = self.rest()[..n].to_string();
        self.at += n;
        w
    }

    /// Skips what is left of a type (bounds, lifetimes) up to a `,`, `>`, `)` or `]` at its level.
    fn skip_to_end(&mut self) {
        let mut depth = 0i32;
        while let Some(c) = self.rest().chars().next() {
            match c {
                '<' | '(' | '[' => depth += 1,
                '>' | ')' | ']' if depth == 0 => return,
                ',' if depth == 0 => return,
                '>' | ')' | ']' => depth -= 1,
                _ => {}
            }
            self.at += c.len_utf8();
        }
    }

    fn args(&mut self) -> Vec<Ty> {
        let mut out = Vec::new();
        if !self.eat("<") {
            return out;
        }
        loop {
            self.skip_ws();
            if self.eat(">") || self.rest().is_empty() {
                return out;
            }
            let before = self.at;
            if self.rest().starts_with('\'') {
                self.skip_to_end();
            } else {
                out.push(self.ty());
            }
            self.eat(",");
            self.progress(before);
        }
    }

    /// Makes sure a list's reading moves on: past one character when nothing was read.
    fn progress(&mut self, before: usize) {
        if self.at == before
            && let Some(c) = self.rest().chars().next()
        {
            self.at += c.len_utf8();
        }
    }

    fn ty(&mut self) -> Ty {
        self.skip_ws();
        if self.eat("&") {
            if self.rest().starts_with('\'') {
                self.word_lifetime();
            }
            self.eat("mut ");
            return self.ty();
        }
        if self.eat("[") {
            let t = self.ty();
            self.skip_to_end();
            self.eat("]");
            return Ty::List(Box::new(t));
        }
        if self.eat("(") {
            let mut items = Vec::new();
            while !self.eat(")") && !self.rest().is_empty() {
                let before = self.at;
                items.push(self.ty());
                self.eat(",");
                self.progress(before);
            }
            return match items.len() {
                0 => Ty::Unit,
                2 => Ty::Pair(Box::new(items[0].clone()), Box::new(items[1].clone())),
                _ => Ty::Unknown,
            };
        }
        if self.eat("dyn ") || self.eat("impl ") {
            let w = self.word();
            let w = w.rsplit("::").next().unwrap_or("").to_string();
            let t = if matches!(w.as_str(), "Fn" | "FnMut" | "FnOnce") {
                let mut params = Vec::new();
                if self.eat("(") {
                    while !self.eat(")") && !self.rest().is_empty() {
                        let before = self.at;
                        params.push(self.ty());
                        self.eat(",");
                        self.progress(before);
                    }
                }
                let ret = if self.eat("->") { self.ty() } else { Ty::Unit };
                Ty::Fun(Box::new(FunTy { params, ret, receiver: None }))
            } else if w == "Into" {
                let a = self.args();
                a.into_iter().next().unwrap_or(Ty::Unknown)
            } else {
                self.args();
                Ty::named(&w)
            };
            self.skip_to_end();
            return t;
        }
        let w = self.word();
        let name = w.rsplit("::").next().unwrap_or("").to_string();
        let args = self.args();
        // (an associated type of a type parameter or of `Self`: `I::Item`, `Self::K`)
        if let Some((first, _)) = w.split_once("::")
            && (first == "Self" || first.len() == 1)
        {
            return Ty::Unknown;
        }
        let first = || args.first().cloned().unwrap_or(Ty::Unknown);
        match name.as_str() {
            "i32" => Ty::Int,
            "i64" => Ty::Long,
            "f32" => Ty::Float,
            "f64" => Ty::Double,
            "bool" => Ty::Bool,
            "char" => Ty::Char,
            "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "isize" | "i128" | "u128" => Ty::OtherInt,
            "String" | "str" => Ty::String,
            "Arc" | "Rc" | "Box" if matches!(args.first(), Some(Ty::String)) => Ty::String,
            "Option" => Ty::Nullable(Box::new(first())),
            "Vec" | "VecDeque" | "List" | "MutableList" => Ty::List(Box::new(first())),
            "KtSet" | "HashSet" | "BTreeSet" => Ty::Set(Box::new(first())),
            "KtMap" | "HashMap" | "BTreeMap" => {
                Ty::Map(Box::new(first()), Box::new(args.get(1).cloned().unwrap_or(Ty::Unknown)))
            }
            "KtProgression" => Ty::Range(Box::new(first())),
            "" | "Self" | "T" | "R" | "K" | "V" | "F" | "U" => Ty::Unknown,
            _ if name.len() == 1 => Ty::Unknown,
            _ => Ty::Named(name, args),
        }
    }

    fn word_lifetime(&mut self) {
        self.at += 1;
        self.word();
        self.skip_ws();
    }
}

// ---- Callees ---------------------------------------------------------------------------------------------------

/// How a nullable parameter takes a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OptionForm {
    /// It isn't nullable.
    No,
    /// `Option<T>`: a value given is `Some(v)` (except through a builder, whose setter takes `impl Into<Option<T>>`).
    Option,
    /// `impl Into<Option<T>>` (the file's own functions): a value as it is.
    IntoOption,
}

/// What a parameter's default is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PDefault {
    None,
    /// The callee's builder has it.
    Builder,
    /// A Kotlin expression (an index doc line's) the call gives when it leaves the parameter out.
    Kotlin(String),
}

/// A lambda parameter: what a lambda given to it becomes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LInfo {
    pub kind: LambdaKind,
    /// Its Rust parameters' types (a receiver's first).
    pub inputs: Vec<String>,
    pub output: String,
    /// The Rust type of its receiver (its first input), Kotlin's `T.() -> R`.
    pub receiver: Option<String>,
    pub suspend: bool,
    /// It is kept by the callee (`+ 'static`): it captures clones and moves them in, as Kotlin's lambdas share
    /// what they capture; otherwise it runs before the call returns (Kotlin's inline functions) and borrows.
    pub stored: bool,
    /// `impl MeasurePolicy`: given through `measure_policy(…)`, which tells Rust its parameters' types.
    pub measure_policy: bool,
    /// Made a shared or boxed trait object (`Rc<dyn Fn(&TextLayoutResult)>`, `Callback`): its parameters are written
    /// with their types, which a closure can't take from where it goes then.
    pub typed: bool,
}

impl LInfo {
    /// Its output is nothing (a lambda's last expression is then a statement).
    pub fn unit_output(&self) -> bool {
        matches!(self.output.trim(), "()" | "")
    }

    /// How many parameters Kotlin's lambda has (the receiver isn't one).
    pub fn arity(&self) -> usize {
        self.inputs.len() - usize::from(self.receiver.is_some())
    }
}

/// A callee's parameter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PInfo {
    pub kotlin: String,
    pub rust: String,
    /// Its Rust type, as text.
    pub ty: String,
    pub default: PDefault,
    pub vararg: bool,
    pub lambda: Option<LInfo>,
    pub option: OptionForm,
}

impl PInfo {
    /// What its type tells of a string literal given to it: `true` when a `&'static str` does (`impl Into<String>`,
    /// `&str`, `impl AsRef<str>`, `impl KtToString`); else a literal is made a `String`, Kotlin's only string type.
    pub fn takes_str(&self) -> bool {
        let t = plain_type_text(&self.ty);
        t.starts_with("implInto<String>")
            || t == "&str"
            || t == "&'staticstr"
            || t.contains("AsRef<str>")
            || t.contains("KtToString")
            || t.contains("KtNeedle")
            || t.starts_with("implInto<Arc<str>>")
            || t.starts_with("implInto<AnnotatedString>")
            || t.starts_with("implInto<BasicTextValue>")
    }

    pub fn ty(&self) -> Ty {
        Ty::from_rust(&plain_type_text(&self.ty))
    }
}

/// A Rust type's text without spaces and the standard paths the generator writes (`impl ::core::convert::Into<…>`
/// → `implInto<…>`).
pub(crate) fn plain_type_text(t: &str) -> String {
    let mut t = t.replace(' ', "");
    for p in [
        "::core::convert::",
        "::std::string::",
        "::core::option::",
        "::core::ops::",
        "::std::vec::",
        "::std::rc::",
        "::std::sync::",
        "::std::boxed::",
    ] {
        t = t.replace(p, "");
    }
    t
}

/// How a Kotlin value of an interface the Rust API has as a trait (`Shape`, `Painter`) is held: the smart pointer
/// (`Arc`, `Rc`) the API's own parameters hold that trait's objects in (`Rc` if none says), none for what isn't a
/// trait of the API (a trait is what the index has methods of through it, of no type).
pub(crate) fn trait_object(name: &str) -> Option<&'static str> {
    let is_trait = index::ITEMS.iter().any(|i| i.via == name && i.owner.is_empty() && i.kind == Kind::Method);
    if !is_trait {
        return None;
    }
    let (arc, rc) = (format!("Arc<dyn {name}"), format!("Rc<dyn {name}"));
    let held = |p: &str| index::ITEMS.iter().flat_map(|i| i.params).filter(|x| x.ty.contains(p)).count();
    Some(if held(&arc) > held(&rc) { "Arc" } else { "Rc" })
}

/// What a call calls: its Rust path or method name, its parameters, and how it is called.
#[derive(Clone, Debug)]
pub(crate) struct Callee {
    /// Its Rust name (a method's, or a function's last segment).
    pub rust: String,
    pub params: Vec<PInfo>,
    /// It has a builder (`#[named]`, `#[kompozable]` with defaults): a call leaving defaults out or naming
    /// arguments goes through it.
    pub builder: bool,
    pub suspend: bool,
    pub ret: Ty,
    pub kompozable: bool,
    /// Where it is: an index item's crate, module, owner and trait (a file's own function: none).
    pub item: Option<&'static Item>,
}

impl Callee {
    pub fn from_item(item: &'static Item) -> Callee {
        let params = item.params.iter().map(param_of_item).collect();
        Callee {
            rust: item.rust.to_string(),
            params,
            builder: item.has(index::flags::NAMED) || item.has(index::flags::KOMPOZABLE),
            suspend: item.has(index::flags::SUSPEND),
            ret: Ty::from_rust(item.ret),
            kompozable: item.has(index::flags::KOMPOZABLE),
            item: Some(item),
        }
    }
}

fn param_of_item(p: &'static index::Param) -> PInfo {
    let ty = p.ty.trim();
    let measure_policy = ty.replace(' ', "") == "implMeasurePolicy";
    let lambda = if measure_policy {
        Some(LInfo {
            kind: LambdaKind::Fn,
            inputs: vec!["&MeasureScope".into(), "&[&dyn Measurable]".into(), "Constraints".into()],
            output: "MeasureResult".into(),
            receiver: Some("MeasureScope".into()),
            suspend: false,
            stored: true,
            measure_policy: true,
            typed: false,
        })
    } else {
        p.lambda.map(|l| LInfo {
            kind: l.kind,
            inputs: l.inputs.iter().map(|s| s.to_string()).collect(),
            output: l.output.to_string(),
            receiver: (!l.receiver.is_empty()).then(|| l.receiver.to_string()),
            suspend: l.suspend,
            // (a generic parameter's bounds are elsewhere: what a callee takes that way it keeps; so does a
            // coroutine, and a shared or boxed one: an owned trait object is `'static`)
            stored: ty.contains("'static")
                || ty.len() <= 2
                || l.suspend
                || ["Rc<dyn", "Arc<dyn", "Box<dyn"].iter().any(|o| ty.contains(o)),
            measure_policy: false,
            typed: ["Rc<dyn", "Arc<dyn", "Box<dyn"].iter().any(|o| ty.contains(o)),
        })
    };
    PInfo {
        kotlin: p.kotlin.to_string(),
        rust: p.rust.to_string(),
        ty: ty.to_string(),
        default: match p.default {
            index::Default::None => PDefault::None,
            index::Default::Rust(_) => PDefault::Builder,
            index::Default::Kotlin(k) => PDefault::Kotlin(k.to_string()),
        },
        vararg: p.vararg,
        lambda,
        option: if ty.starts_with("Option<") { OptionForm::Option } else { OptionForm::No },
    }
}

/// The index's items of a Kotlin name, `Composition…` (`compositionLocalOf` too) also found as KompozR's
/// `Kompozition…` (pasted Compose code), and the C++ Kompoz's `Subkompoz…` (`SubkompozLayout`, `subkompoz`) as
/// Compose's `Subcompose…`.
pub(crate) fn lookup(kotlin: &str) -> &'static [Item] {
    let found = index::lookup(kotlin);
    if found.is_empty() && kotlin.contains("Composition") {
        return index::lookup(&kotlin.replace("Composition", "Kompozition"));
    }
    if found.is_empty() && kotlin.contains("ubkompoz") {
        return index::lookup(&kotlin.replace("Subkompoz", "Subcompose").replace("subkompoz", "subcompose"));
    }
    if found.is_empty()
        && let Some(rest) = kotlin.strip_prefix("composition")
    {
        return index::lookup(&format!("kompozition{rest}"));
    }
    found
}

/// A free function of the index (no owner, not through a trait).
pub(crate) fn free_functions(kotlin: &str) -> impl Iterator<Item = &'static Item> {
    lookup(kotlin).iter().filter(|i| i.kind == Kind::Function && i.owner.is_empty() && i.via.is_empty())
}

/// The index's members (methods, properties) of a Kotlin name that a type of Rust name `owner` has: its own, and
/// those of the extension traits named after it (`PointerInputScopeGestures` for `PointerInputScope`: Compose's
/// extensions of it); then those of what it derefs to (a `MeasureScope`'s `Density`: Compose's supertypes), nearest
/// first.
pub(crate) fn members_of(owner: &str, kotlin: &str) -> impl Iterator<Item = &'static Item> + use<> {
    let items = lookup(kotlin);
    let mut found = Vec::new();
    if owner.is_empty() {
        return found.into_iter();
    }
    for o in index::deref_chain(owner) {
        found.extend(items.iter().filter(|i| {
            matches!(i.kind, Kind::Method | Kind::Property)
                && (i.owner == o || (i.owner.is_empty() && i.via.starts_with(o)))
        }));
    }
    found.into_iter()
}

/// The kt shims' extension-trait methods of a Kotlin name, in the traits `traits` (any of them if empty).
pub(crate) fn kt_members(kotlin: &str, traits: &[&str]) -> Vec<&'static Item> {
    lookup(kotlin)
        .iter()
        .filter(|i| {
            i.krate == "kompoz_runtime"
                && i.module.starts_with("kt")
                && !i.via.is_empty()
                && i.kind == Kind::Method
                && (traits.is_empty() || traits.contains(&i.via))
        })
        .collect()
}

/// Whether a PascalCase name is a value object of the index (`Modifier`, `LocalDensity`): `.` after it, not `::`.
pub(crate) fn is_value_object(name: &str) -> bool {
    lookup(name)
        .iter()
        .any(|i| i.kind == Kind::Value && i.owner.is_empty() && !i.module.contains(|c: char| c.is_uppercase()))
}

/// The index's items owned by a type of Rust name `owner` (its associated functions, constants, entries).
pub(crate) fn associated(owner: &str, kotlin: &str) -> Vec<&'static Item> {
    lookup(kotlin)
        .iter()
        .filter(|i| {
            (i.owner == owner && matches!(i.kind, Kind::Function | Kind::Constant | Kind::Variant))
                || (i.kind == Kind::Value && i.module.rsplit("::").next() == Some(owner))
        })
        .collect()
}

// ---- Scopes ----------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LocalKind {
    Val,
    Var,
    /// `val`/`var x by delegate`: the delegate is held, reads and writes go through it.
    Delegated,
    Param,
    /// The implicit `it` of a lambda, or one of its parameters.
    LambdaParam,
}

#[derive(Clone, Debug)]
pub(crate) struct Local {
    pub rust: String,
    pub kind: LocalKind,
    pub ty: Ty,
    /// The scope depth it is declared at (what lambdas inside capture it by).
    pub depth: usize,
    /// A kept closure parameter of a restartable composable (`Rc`-held: cloned when used as a value).
    pub kept_closure: bool,
    /// A lambda's parameter given a `&mut` (`state.update { it.add(…) }`: the value to change in place): what takes
    /// it by reference takes it reborrowed, a value of it is a clone of what it points to.
    pub mut_ref: bool,
}

/// An implicit receiver: an extension function's `self`, a receiver lambda's scope parameter.
#[derive(Clone, Debug)]
pub(crate) struct ImplicitReceiver {
    /// Its Rust name (`self`, `this`, `this1`, …).
    pub rust: String,
    /// Its Rust type's name (`Modifier`, `DrawScope`).
    pub ty: String,
    /// Its Kotlin label: the extension function's name, the function the lambda is given to, or the lambda's own.
    pub labels: Vec<String>,
    pub depth: usize,
    /// A receiver lambda's scope given by shared reference (`&SubcomposeMeasureScope`): a lambda kept inside it
    /// captures a clone of the scope itself (Kotlin's lambda holds the scope object), not the reference.
    pub by_ref: bool,
}

/// A lambda being written: the outer locals its body uses (what it captures), in the order first used.
#[derive(Clone, Debug, Default)]
pub(crate) struct CaptureFrame {
    /// The scope depth its own scopes start at: a local declared below it is outside.
    pub boundary: usize,
    pub used: Vec<String>,
}

/// A file's own function, as its calls see it.
#[derive(Clone, Debug)]
pub(crate) struct FileFn {
    pub rust: String,
    pub params: Vec<PInfo>,
    pub kompozable: bool,
    pub suspend: bool,
    pub ret: Ty,
    /// An extension function's receiver type (its Rust name).
    pub receiver: Option<String>,
    pub builder: bool,
}

impl FileFn {
    pub fn callee(&self) -> Callee {
        Callee {
            rust: self.rust.clone(),
            params: self.params.clone(),
            builder: self.builder,
            suspend: self.suspend,
            ret: self.ret.clone(),
            kompozable: self.kompozable,
            item: None,
        }
    }
}

/// How a file's top-level `val` is held.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ValForm {
    /// A `const` (numbers, booleans, characters).
    Const,
    /// A `const` string (`&str`), read as a `String`.
    StrConst,
    /// A `static` made by a `const fn` (a composition local: known by its address, it can't be a thread-local's
    /// copy), read in place.
    Static,
    /// A thread-local value made at its first read (Kotlin's initialization of a top-level property).
    ThreadLocal,
}

#[derive(Clone, Debug)]
pub(crate) struct FileVal {
    pub rust: String,
    pub ty: Ty,
    pub form: ValForm,
}

/// The file's top-level declarations, by Kotlin name.
#[derive(Default)]
pub(crate) struct FileTable {
    pub functions: HashMap<String, Vec<FileFn>>,
    pub vals: HashMap<String, FileVal>,
}

impl FileTable {
    /// A top-level (not extension) function of the name.
    pub fn function(&self, kotlin: &str) -> Option<&FileFn> {
        self.functions.get(kotlin)?.iter().find(|f| f.receiver.is_none())
    }

    /// An extension function of the name for a receiver of Rust type `ty` (any receiver when `None`).
    pub fn extension(&self, kotlin: &str, ty: Option<&str>) -> Option<&FileFn> {
        self.functions.get(kotlin)?.iter().find(|f| match (&f.receiver, ty) {
            (Some(r), Some(t)) => r == t,
            (Some(_), None) => true,
            (None, _) => false,
        })
    }
}

/// A Kotlin type's Rust name for the index's owners (`Modifier.Companion` → `Modifier`).
pub(crate) fn owner_name(kotlin: &str) -> String {
    match kotlin {
        "Int" => "i32".into(),
        "Float" => "f32".into(),
        "String" => "String".into(),
        _ => kotlin.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_types_read() {
        assert_eq!(Ty::from_rust("f32"), Ty::Float);
        assert_eq!(Ty::from_rust("Option<String>"), Ty::Nullable(Box::new(Ty::String)));
        assert_eq!(Ty::from_rust("&[&dyn Measurable]"), Ty::List(Box::new(Ty::named("Measurable"))));
        assert_eq!(Ty::from_rust("Vec<Placeable>"), Ty::List(Box::new(Ty::named("Placeable"))));
        assert_eq!(Ty::from_rust("impl Into<String>"), Ty::String);
        assert_eq!(Ty::from_rust("MutableState<T>"), Ty::Named("MutableState".into(), vec![Ty::Unknown]));
        assert_eq!(Ty::from_rust("()"), Ty::Unit);
        assert!(matches!(Ty::from_rust("impl Fn(i32) -> bool + 'static"), Ty::Fun(_)));
        assert_eq!(Ty::from_rust("KtProgression<i32>"), Ty::Range(Box::new(Ty::Int)));
    }
}
