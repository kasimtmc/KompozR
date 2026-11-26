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
    /// Kotlin's `ClosedRange<T>`/`ClosedFloatingPointRange<T>` (a `Float` or `Double` `a..b`): Rust's
    /// `RangeInclusive<T>`, not a progression.
    ClosedRange(Box<Ty>),
    Pair(Box<Ty>, Box<Ty>),
    Fun(Box<FunTy>),
    /// Any other type, by its Rust name (`Dp`, `Modifier`, `MutableState`) and arguments.
    Named(String, Vec<Ty>),
}

/// A function type: `(A, B) -> R`, `Scope.() -> R`, `suspend () -> R`.
#[derive(Clone, Debug)]
pub(crate) struct FunTy {
    pub params: Vec<Ty>,
    pub ret: Ty,
    /// Its receiver's Rust type name, when the Rust side passes it (`DrawScope`).
    pub receiver: Option<String>,
    /// `suspend (A) -> R`: a call of a value of it is awaited.
    pub suspend: bool,
    /// A value of it is a shared closure on the Rust side (`Rc<dyn Fn(f32) -> f32>`, what an API gives —
    /// `SwipeToDismissBoxDefaults.positionalThreshold`, `WavyProgressIndicatorDefaults.indicatorAmplitude` — and a
    /// `val` keeps): given as it is where a shared closure is taken (not `Rc::new` of it, as a plain function value
    /// is), cloned when used again. Not part of the Kotlin type (equality ignores it).
    pub shared: bool,
}

impl PartialEq for FunTy {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params
            && self.ret == other.ret
            && self.receiver == other.receiver
            && self.suspend == other.suspend
    }
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
            Ty::List(e) | Ty::Set(e) | Ty::Range(e) | Ty::ClosedRange(e) | Ty::Nullable(e) => e.is_vague(),
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
            Ty::List(t) | Ty::Set(t) | Ty::Range(t) | Ty::ClosedRange(t) => (**t).clone(),
            Ty::Named(n, a) if n == "SnapshotStateList" => a.first().cloned().unwrap_or(Ty::Unknown),
            // (a Rust `Result` is Kotlin's `Result<T>`: what `getOrThrow()` gives)
            Ty::Named(n, a) if n == "Result" => a.first().cloned().unwrap_or(Ty::Unknown),
            Ty::Map(k, v) => Ty::Pair(k.clone(), v.clone()),
            Ty::String => Ty::Char,
            _ => Ty::Unknown,
        }
    }

    /// Its Rust type's name as the API index names owners (`Size`, `Modifier`); none for what has none there. A shared
    /// or boxed value's is what it holds (`Rc<dyn Measurable>`: a `Measurable`, Kotlin's reference to one, whose
    /// methods Rust's calls reach through it).
    pub fn owner(&self) -> Option<&str> {
        match self {
            Ty::Named(n, a) if matches!(n.as_str(), "Rc" | "Arc" | "Box") && a.len() == 1 => a[0].owner(),
            Ty::Named(n, _) => Some(n),
            Ty::String => Some("String"),
            _ => None,
        }
    }

    /// What a shared or boxed value holds (`Rc<dyn Measurable>` → `Measurable`), else itself: the type whose members
    /// its owner's are ([`Ty::owner`]).
    pub fn through_shared(&self) -> &Ty {
        match self {
            Ty::Named(n, a) if matches!(n.as_str(), "Rc" | "Arc" | "Box") && a.len() == 1 && a[0].owner().is_some() => {
                a[0].through_shared()
            }
            t => t,
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
            // (a Rust `Result`: Kotlin's `Result<T>`, its members the kt shim's)
            Ty::Named(n, _) if n == "Result" => &["KtResult", "KtScope"],
            Ty::String => &["KtStr", "KtScope"],
            Ty::Nullable(t) if t.is_string() => &["KtNullableStr", "KtScope"],
            Ty::Map(_, _) => &["KtMapExt", "KtScope"],
            Ty::Char => &["KtChar", "KtScope"],
            Ty::Float | Ty::Double => &["KtFloat", "KtNumber", "KtScope"],
            Ty::Int | Ty::Long | Ty::OtherInt => &["KtNumber", "KtScope"],
            Ty::Pair(_, _) => &["KtPair", "KtComponents", "KtComponent3", "KtTriple", "KtScope"],
            Ty::ClosedRange(_) => &["KtClosedRange", "KtScope"],
            _ => &["KtScope"],
        }
    }

    /// A Rust type written as text (the API index's): what the generator needs to know of it.
    pub fn from_rust(text: &str) -> Ty {
        // (WP7: search/tooltips) A type the generator wrote itself, its tokens printed with spaces round the paths'
        // `::` (`:: std :: string :: String`, a file function's `(String) -> Unit` parameter): read as one path.
        let joined;
        let text = if text.contains(":: ") {
            joined = text.replace(":: ", "::").replace(" ::", "::");
            joined.as_str()
        } else {
            text
        };
        let mut p = TypeText { s: text.trim(), at: 0, params: Params::Unknown };
        p.ty()
    }

    /// A Rust type's text naming type parameters (`R`, `Option<R>`, `KtMap<K, Self::Item>`, `Vec<I::Item>`) with what
    /// `binds` says they are ([`bind_type_params`]); one it doesn't say is unknown.
    pub fn from_rust_bound(text: &str, binds: &TypeBinds) -> Ty {
        let mut p = TypeText { s: text.trim(), at: 0, params: Params::Bound(binds) };
        p.ty()
    }
}

/// What a generic callee's type parameters are, by name (`R`, `Self`, `Self::Item`, `I`), as inference tells them.
pub(crate) type TypeBinds = HashMap<String, Ty>;

/// Binds the type parameters a Rust type's text names (`R`, `Option<R>`, `(K, V)`, `Vec<T>`, `Self::V`) to what
/// `actual` has in their places (Kotlin's inference of a generic call: `map { it.toFloat() }`'s `R` a `Float`); one
/// bound already keeps what it is.
pub(crate) fn bind_type_params(pattern: &str, actual: &Ty, binds: &mut TypeBinds) {
    let mut p = TypeText { s: pattern.trim(), at: 0, params: Params::Marked };
    let pattern = p.ty();
    unify(&pattern, actual, binds);
}

/// The name a type parameter marked in a pattern has ([`Params::Marked`]).
fn marked(t: &Ty) -> Option<&str> {
    match t {
        Ty::Named(n, a) if a.is_empty() => n.strip_prefix('$'),
        _ => None,
    }
}

fn unify(pattern: &Ty, actual: &Ty, binds: &mut TypeBinds) {
    if *actual == Ty::Unknown {
        return;
    }
    if let Some(name) = marked(pattern) {
        let known = binds.get(name).is_some_and(|t| !t.is_vague());
        if !known {
            binds.insert(name.to_string(), actual.clone());
        }
        return;
    }
    match (pattern, actual) {
        (Ty::Nullable(p), Ty::Nullable(a)) => unify(p, a, binds),
        // (a `T?` given a `T`: Kotlin's `mapNotNull { it.toFloat() }`, its `R` a `Float`)
        (Ty::Nullable(p), a) => unify(p, a, binds),
        (Ty::List(p), Ty::List(a))
        | (Ty::Set(p), Ty::Set(a))
        | (Ty::Range(p), Ty::Range(a))
        | (Ty::ClosedRange(p), Ty::ClosedRange(a)) => unify(p, a, binds),
        // (any collection of it: `flatMap`'s `I`, its `I::Item` its element)
        (Ty::List(p), a @ (Ty::Set(_) | Ty::Range(_))) => unify(p, &a.element(), binds),
        (Ty::Map(pk, pv), Ty::Map(ak, av)) | (Ty::Pair(pk, pv), Ty::Pair(ak, av)) => {
            unify(pk, ak, binds);
            unify(pv, av, binds);
        }
        (Ty::Named(pn, pa), Ty::Named(an, aa)) if pn == an && pa.len() == aa.len() => {
            for (p, a) in pa.iter().zip(aa) {
                unify(p, a, binds);
            }
        }
        _ => {}
    }
}

/// How a reader of Rust types takes the type parameters a text names.
#[derive(Clone, Copy)]
enum Params<'b> {
    /// Unknown (what the index's types are without a call's inference).
    Unknown,
    /// Each one marked (`$R`, `$Self::Item`): a pattern [`bind_type_params`] matches with a type.
    Marked,
    /// What a call's inference binds them to.
    Bound(&'b TypeBinds),
}

/// A small reader of Rust types as the index writes them.
struct TypeText<'a, 'b> {
    s: &'a str,
    at: usize,
    params: Params<'b>,
}

/// Whether a type's name (a path's text) is a type parameter of a generic item: `T`, `R`, `Self`, or an associated
/// type of one (`Self::Item`, `I::Item`).
fn names_param(w: &str) -> bool {
    let first = w.split("::").next().unwrap_or("");
    first == "Self" || (first.len() == 1 && first.chars().all(|c| c.is_ascii_uppercase()))
}

impl TypeText<'_, '_> {
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
                Ty::Fun(Box::new(FunTy { params, ret, receiver: None, suspend: false, shared: false }))
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
        if names_param(&w) && args.is_empty() {
            match self.params {
                Params::Unknown => {}
                Params::Marked => return Ty::Named(format!("${w}"), Vec::new()),
                Params::Bound(binds) => {
                    if let Some(t) = binds.get(&w) {
                        return t.clone();
                    }
                    // (an associated type of one bound: `I::Item` of a list, `Self::K` of a map)
                    if let Some((of, assoc)) = w.split_once("::")
                        && let Some(t) = binds.get(of)
                    {
                        return match (assoc, t) {
                            ("K", Ty::Map(k, _)) => (**k).clone(),
                            ("V", Ty::Map(_, v)) => (**v).clone(),
                            ("Item" | "Out", t) => t.element(),
                            _ => Ty::Unknown,
                        };
                    }
                    return Ty::Unknown;
                }
            }
        }
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
            // (a shared closure is a Kotlin function value, one marked shared: `val threshold =
            // SwipeToDismissBoxDefaults.positionalThreshold`, an `Rc<dyn Fn(f32) -> f32>`, given on to a lambda
            // parameter as it is)
            "Rc" | "Arc" if matches!(args.first(), Some(Ty::Fun(_))) => match first() {
                Ty::Fun(mut f) => {
                    f.shared = true;
                    Ty::Fun(f)
                }
                t => t,
            },
            "Option" => Ty::Nullable(Box::new(first())),
            "Vec" | "VecDeque" | "List" | "MutableList" => Ty::List(Box::new(first())),
            "KtSet" | "HashSet" | "BTreeSet" => Ty::Set(Box::new(first())),
            "KtMap" | "HashMap" | "BTreeMap" => {
                Ty::Map(Box::new(first()), Box::new(args.get(1).cloned().unwrap_or(Ty::Unknown)))
            }
            "KtProgression" => Ty::Range(Box::new(first())),
            "RangeInclusive" => Ty::ClosedRange(Box::new(first())),
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
    /// That receiver's type with its type arguments, where the call tells them (the value a scope function lends:
    /// `with(anim) { value }`'s `Animatable<Float, …>`).
    pub receiver_value: Option<Ty>,
    pub suspend: bool,
    /// Its future is boxed (`Pin<Box<dyn Future>>`): it gives `Box::pin(async move { … })`.
    pub boxed: bool,
    /// It is kept by the callee (`+ 'static`): it captures clones and moves them in, as Kotlin's lambdas share
    /// what they capture; otherwise it runs before the call returns (Kotlin's inline functions) and borrows.
    pub stored: bool,
    /// `impl MeasurePolicy`: given through `measure_policy(…)`, which tells Rust its parameters' types.
    pub measure_policy: bool,
    /// Made a shared or boxed trait object (`Rc<dyn Fn(&TextLayoutResult)>`, `Callback`): its parameters are written
    /// with their types, which a closure can't take from where it goes then.
    pub typed: bool,
    /// The callee runs it in place and has a suspending twin ([`index::flags::ASYNC_TWIN`]: Kotlin's inline function,
    /// a kt shim's): in a coroutine it may suspend, an async closure given to the twin.
    pub twin: bool,
    /// A lambda of the file's own `inline fun`, run in place: one that suspends needs the function's suspending form
    /// (stage 14's inline functions).
    pub file_inline: bool,
    /// A block of Compose's run with a snapshot current (`Snapshot.withMutableSnapshot`, `snapshot.enter`,
    /// `Snapshot.withoutReadObservation`): inline in Kotlin, but it must not suspend ([`snapshot_block`]).
    pub snapshot_block: bool,
    /// Given to Kotlin's overload of an inline function that takes it `crossinline` (`withStyle(ParagraphStyle)`, its
    /// argument's type one the suspending twin doesn't take: [`index::TwinBound`]): it can't suspend, an error saying
    /// so.
    pub crossinline: Option<Crossinline>,
}

/// Kotlin's overload of an inline function a call is that takes its lambda `crossinline`, and those that don't.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Crossinline {
    /// The call's overload, by the types given where the twin takes fewer (`withStyle(ParagraphStyle)`).
    pub overload: String,
    /// The overloads whose lambdas may suspend (`withStyle(SpanStyle)`).
    pub suspending: Vec<String>,
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
    /// Whether a Kotlin argument named `name` is for it (see [`answers_to`](crate::index::answers_to)).
    pub fn answers_to(&self, name: &str) -> bool {
        crate::index::answers_to(&self.kotlin, &self.ty, name)
    }

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
    // (every whitespace: a long type's text, as tokens print it, breaks lines)
    let mut t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
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
    // (the user's interfaces: held as `Rc<dyn I>`, Kotlin's references to their objects)
    if index::overlay::is_interface(name) {
        return Some("Rc");
    }
    let (arc, rc) = (format!("Arc<dyn {name}"), format!("Rc<dyn {name}"));
    // (the whole name: `Rc<dyn KeyMapping>` holds no `Key`)
    let names = |ty: &str, p: &str| {
        ty.match_indices(p)
            .any(|(at, _)| ty[at + p.len()..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_')))
    };
    let held = |p: &str| index::ITEMS.iter().flat_map(|i| i.params).filter(|x| names(x.ty, p)).count();
    let (mut arcs, mut rcs) = (held(&arc), held(&rc));
    // (none taken: as the API hands them out — `DraggableAnchors<T>`, taken as `impl DraggableAnchors<T>`, a state's
    // `anchors` an `Arc<dyn DraggableAnchors<T>>`)
    if arcs + rcs == 0 {
        let given = |p: &str| index::ITEMS.iter().filter(|i| names(i.ret, p)).count();
        (arcs, rcs) = (given(&arc), given(&rc));
    }
    // (a trait the index knows a method of, or one the API takes as a shared trait object: `AnimationSpec<T>`, whose
    // methods are hidden but which `animationSpec: Arc<dyn AnimationSpec<T>>` takes); not the macros' support traits
    // (`kompoz_runtime::private`'s `Key`, which Compose's `Key` isn't)
    let is_trait = index::ITEMS
        .iter()
        .any(|i| i.via == name && i.owner.is_empty() && i.kind == Kind::Method && i.module != "private");
    if !is_trait && arcs + rcs == 0 {
        return None;
    }
    Some(if arcs > rcs { "Arc" } else { "Rc" })
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
        let mut params: Vec<PInfo> = item.params.iter().map(param_of_item).collect();
        // (Kotlin's inline function with a suspending twin: its lambdas run in place may suspend in a coroutine)
        if item.has(index::flags::ASYNC_TWIN) {
            for l in params.iter_mut().filter_map(|p| p.lambda.as_mut()).filter(|l| !l.stored) {
                l.twin = true;
            }
        }
        if snapshot_block(item) {
            for l in params.iter_mut().filter_map(|p| p.lambda.as_mut()) {
                l.snapshot_block = true;
            }
        }
        // (WP7: carousel) A plain function's builder lambda, its receiver `&mut` (`GenericShape { size, _ ->
        // addRoundRect(…) }`'s Path): its parameters typed — a composable's body calls a plain function through the
        // macro's positional builder, which is generic over the closure, so nothing tells Rust their types.
        let builder = item.has(index::flags::NAMED) || item.has(index::flags::KOMPOZABLE);
        if !builder && item.owner.is_empty() && item.kind == index::Kind::Function {
            let mutable_receiver = |l: &LInfo| {
                l.receiver.is_some() && l.inputs.first().is_some_and(|t| t.trim_start().starts_with("&mut"))
            };
            for l in params.iter_mut().filter_map(|p| p.lambda.as_mut()).filter(|l| mutable_receiver(l)) {
                l.typed = true;
            }
        }
        // ((stage 14 WPD) a named plain function's builder of a concrete receiver — `SavedStateConfiguration {
        // serializersModule = … }`, its `from` defaulted —: typed too, the named call's closure generic as well)
        if item.has(index::flags::NAMED) && !item.has(index::flags::KOMPOZABLE) && item.owner.is_empty() {
            for l in params.iter_mut().filter_map(|p| p.lambda.as_mut()) {
                if l.receiver.is_some()
                    && l.inputs.len() == 1
                    && l.inputs[0].trim_start().starts_with("&mut")
                    && !super::lambda::names_type_parameter(&l.inputs[0])
                {
                    l.typed = true;
                }
            }
        }
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

/// Whether `item` is one of Compose's inline functions running their block with a snapshot current: Kotlin lets the
/// block suspend, Compose says it must not (`withMutableSnapshot`'s doc: the snapshot is the thread's current one
/// until the block returns, so the coroutines run while it is suspended would run in it — its changes not applied,
/// a read-only one's writes failing). No suspending twin: a suspending call in the block is an error saying so.
fn snapshot_block(item: &Item) -> bool {
    item.krate == "kompoz_runtime"
        && matches!(
            (item.module, item.rust),
            ("snapshot", "enter" | "with_mutable_snapshot" | "without_read_observation")
                | ("read", "without_read_observation")
        )
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
            receiver_value: None,
            suspend: false,
            boxed: false,
            stored: true,
            measure_policy: true,
            typed: false,
            twin: false,
            file_inline: false,
            snapshot_block: false,
            crossinline: None,
        })
    } else {
        p.lambda.map(|l| LInfo {
            kind: l.kind,
            inputs: l.inputs.iter().map(|s| s.to_string()).collect(),
            output: l.output.to_string(),
            receiver: (!l.receiver.is_empty()).then(|| l.receiver.to_string()),
            receiver_value: None,
            suspend: l.suspend,
            boxed: l.boxed,
            // (a generic parameter's bounds are elsewhere: what a callee takes that way it keeps; so does a
            // coroutine, and a shared or boxed one: an owned trait object is `'static`)
            stored: ty.contains("'static")
                || ty.len() <= 2
                || l.suspend
                || ["Rc<dyn", "Arc<dyn", "Box<dyn"].iter().any(|o| ty.contains(o)),
            measure_policy: false,
            // (a trait object's, and a fun interface's — `impl FlexBoxConfig`, its closures' parameters typed for
            // Rust to find the trait's blanket impl)
            typed: ["Rc<dyn", "Arc<dyn", "Box<dyn"].iter().any(|o| ty.contains(o))
                || (ty.starts_with("impl ") && !["Fn(", "FnMut(", "FnOnce(", "Future"].iter().any(|f| ty.contains(f))),
            twin: false,
            file_inline: false,
            snapshot_block: false,
            crossinline: None,
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
    // (kotlin.math's functions are what Kotlin's name calls where it has one: `round(2.5f)`, not an API's function of
    // that name — material-color's `round`, Compose's internal `MathUtils.round`)
    let kt_math = |i: &Item| i.krate == "kompoz_runtime" && i.module == "kt::math";
    let items = lookup(kotlin);
    let math = items.iter().any(|i| i.kind == Kind::Function && kt_math(i));
    items
        .iter()
        .filter(move |i| i.kind == Kind::Function && i.owner.is_empty() && i.via.is_empty() && (!math || kt_math(i)))
}

/// Whether the API changes a value of Rust type `owner` in place: it has a `&mut self` method (`Path`'s `line_to`,
/// `PathMeasure`'s `set_path`).
pub(crate) fn changed_in_place(owner: &str) -> bool {
    index::ITEMS.iter().any(|i| i.owner == owner && i.kind == Kind::Method && i.receiver == index::Receiver::RefMut)
}

/// Kotlin's constructors of a type that a Rust type's associated functions are (their doc line naming them
/// `Type(…)`: `PaddingValues::all` is `PaddingValues(all: Dp)`), the type's name `kotlin`.
pub(crate) fn constructor_overloads(kotlin: &str) -> impl Iterator<Item = &'static Item> {
    lookup(kotlin).iter().filter(move |i| i.kind == Kind::Function && i.owner == kotlin && i.via.is_empty())
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

/// The fun interface of a Kotlin name (`TextFieldDecorator`) whose SAM constructor a call with a lambda is, unless
/// the API has a function of that name taking a lambda, which its calls call (`OutputTransformation { … }`:
/// `output_transformation`); a function of that name taking other values is Kotlin's other function
/// (`Style(style1, style2)` beside `Style { … }`).
pub(crate) fn fun_interface(kotlin: &str) -> Option<&'static Item> {
    if free_functions(kotlin).any(|f| f.params.len() == 1 && f.params[0].lambda.is_some()) {
        return None;
    }
    fun_interface_type(kotlin)
}

/// The fun interface a Kotlin type name is (`Style`, `TextFieldDecorator`): what a value of the type is made of
/// given a lambda.
pub(crate) fn fun_interface_type(kotlin: &str) -> Option<&'static Item> {
    lookup(kotlin).iter().find(|i| i.kind == Kind::FunInterface && i.params.first().is_some_and(|p| p.lambda.is_some()))
}

/// Whether a PascalCase name is a value object of the index (`Modifier`, `LocalDensity`): `.` after it, not `::`.
pub(crate) fn is_value_object(name: &str) -> bool {
    lookup(name)
        .iter()
        .any(|i| i.kind == Kind::Value && i.owner.is_empty() && !i.module.contains(|c: char| c.is_uppercase()))
}

/// The index's items owned by a type of Rust name `owner` (its associated functions, constants, entries); a function
/// Kotlin names qualified (`TextFieldLineLimits.MultiLine(…)`, Rust's `multi_line`: a doc line's name) too.
pub(crate) fn associated(owner: &str, kotlin: &str) -> Vec<&'static Item> {
    // (an interface's companion is the trait object's own items, `impl dyn TextAutoSize`: owner `dyn TextAutoSize`,
    // its doc lines naming it `TextAutoSize.StepBased`)
    let named = owner.strip_prefix("dyn ").unwrap_or(owner);
    let qualified =
        lookup(&format!("{named}.{kotlin}")).iter().filter(|i| i.owner == owner && i.kind == Kind::Function);
    qualified
        .chain(lookup(kotlin).iter())
        .filter(|i| {
            (i.owner == owner && matches!(i.kind, Kind::Function | Kind::Constant | Kind::Variant))
                || (i.kind == Kind::Value && i.module.rsplit("::").next() == Some(owner))
        })
        .collect()
}

/// The functions `Owner.kotlin(…)` calls: the type's associated ones, or, for an interface, its companion's — the
/// trait object's own (`TextAutoSize.StepBased(…)`: `impl dyn TextAutoSize`'s `step_based`); whether they are that.
pub(crate) fn associated_functions(owner: &str, kotlin: &str) -> (Vec<&'static Item>, bool) {
    let functions = |owner: &str| -> Vec<&'static Item> {
        associated(owner, kotlin).into_iter().filter(|i| i.kind == Kind::Function).collect()
    };
    let own = functions(owner);
    if own.is_empty() && trait_object(owner).is_some() {
        return (functions(&format!("dyn {owner}")), true);
    }
    (own, false)
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
    /// (WP5: swipe to dismiss) A function value a call gave (an `Rc<dyn Fn…>`, Kotlin's function value: `val threshold =
    /// SwipeToDismissBoxDefaults.positionalThreshold`), or a state's destructured setter (`val (x, setX) = remember {
    /// … }`, a cloneable closure): cloned when used as a value, called in place.
    pub shared_closure: bool,
    /// A lambda's parameter given a `&mut` (`state.update { it.add(…) }`: the value to change in place): what takes
    /// it by reference takes it reborrowed, a value of it is a clone of what it points to.
    pub mut_ref: bool,
    /// Set when it is used ([`Gen::local`](super::Gen::local)): a lambda's implicit `it`, which the lambda takes only
    /// when its body (or a lambda inside it without an `it` of its own) uses it.
    pub used: Option<std::rc::Rc<std::cell::Cell<bool>>>,
    /// A lambda's parameter lent by shared reference (`{ menuState -> … }` given a `&ButtonGroupMenuState`): set when
    /// a lambda kept inside the lending one uses it — the lending lambda then holds a clone of it, `__kz_<name>`
    /// (Kotlin's lambda holds the object), which that lambda captures (as a receiver lent by reference is, see
    /// `ImplicitReceiver::owned_used`).
    pub lent_ref: Option<std::rc::Rc<std::cell::Cell<bool>>>,
    /// (WP8) A `val` declared without a value (`val title: String`), given one later (`title = …` in each branch):
    /// Kotlin's definite assignment, Rust's deferred initialization (`let title: String;`), which rustc checks the
    /// same way.
    pub deferred: bool,
    /// Its name's place where it is declared (for the analysis).
    pub at: Option<crate::pos::Loc>,
}

/// An implicit receiver: an extension function's `self`, a receiver lambda's scope parameter.
#[derive(Clone, Debug)]
pub(crate) struct ImplicitReceiver {
    /// Its Rust name (`self`, `this`, `this1`, …).
    pub rust: String,
    /// Its Rust type's name (`Modifier`, `DrawScope`).
    pub ty: String,
    /// Its type with its type arguments where known (`Animatable<Float, AnimationVector1D>` lent by `with`), else
    /// the type of that name: what its members' `T` is.
    pub value: Ty,
    /// Its Kotlin label: the extension function's name, the function the lambda is given to, or the lambda's own.
    pub labels: Vec<String>,
    pub depth: usize,
    /// A receiver lambda's scope given by shared reference (`&SubcomposeMeasureScope`): a lambda kept inside it
    /// captures a clone of the scope itself (Kotlin's lambda holds the scope object), not the reference.
    pub by_ref: bool,
    /// A receiver lambda's scope lent mutably (`&mut DrawScope`): `this` given on is reborrowed (`drawScope = this`
    /// → `&mut *this`), as a `&mut` local is.
    pub by_mut: bool,
    /// Set when a lambda inside the one lending it by reference uses it: the lending lambda then holds a clone of
    /// the scope, `__kz_this` (what a `#[kompozable]` body's closures capture clones of, as Kotlin's lambdas share
    /// the scope object), and the lambdas inside use that.
    pub owned_used: std::rc::Rc<std::cell::Cell<bool>>,
}

/// A lambda being written: the outer locals its body uses (what it captures), in the order first used.
#[derive(Clone, Debug, Default)]
pub(crate) struct CaptureFrame {
    /// The scope depth its own scopes start at: a local declared below it is outside.
    pub boundary: usize,
    pub used: Vec<String>,
    /// Whether the lambda is kept by what it is given to (it outlives the lambda around it).
    pub kept: bool,
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
    /// (stage 14 WPB) A thread-local delegate read (and, `mutable`, set) through it: a top-level `var` (a `kt::Ref`),
    /// a property by a delegate (`by lazy { … }`, `by mutableStateOf(…)`).
    Shared { mutable: bool },
    /// (stage 14 WPB) A property with accessors and no backing field (`val now: Int get() = …`): its getter `now()`
    /// (and setter `set_now(v)`), functions of the file.
    Accessors { setter: bool },
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
    /// The members of classes and objects the file imports by name (`import a.b.Defaults.ScreenOffset`, `import
    /// a.b.Direction.Companion.Bottom`, `import a.b.Defaults.value as v`), by the name the file calls them: the
    /// qualifier the member is of (`Defaults`, `Direction`: a companion's members are its class's) and the member.
    pub imports: HashMap<String, (Vec<crate::ast::Ident>, crate::ast::Ident)>,
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
        assert_eq!(Ty::from_rust("RangeInclusive<f32>"), Ty::ClosedRange(Box::new(Ty::Float)));
    }

    #[test]
    fn a_shared_values_members_are_what_it_holds() {
        // (Kotlin's reference to a Measurable: Rust's calls reach its methods through the `Rc`)
        let shared = Ty::from_rust("Rc<dyn Measurable>");
        assert_eq!((shared.owner(), shared.through_shared()), (Some("Measurable"), &Ty::named("Measurable")));
        let state = Ty::from_rust("Arc<State<f32>>");
        assert_eq!(state.through_shared(), &Ty::Named("State".into(), vec![Ty::Float]));
        // (a shared closure is no type's)
        assert_eq!(Ty::from_rust("Rc<dyn Fn()>").owner(), None);
    }
}
