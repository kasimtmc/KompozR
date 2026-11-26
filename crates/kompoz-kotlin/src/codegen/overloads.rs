//! Kotlin's overloads (stage 14 WPB): functions of one name taking different parameters — at the top level (the
//! text's and the module table's together), extension functions of one receiver type, a class's or an interface's
//! members, composables.
//!
//! Rust has no overloads: each of them gets a name of its own, its Kotlin name and its parameters' types
//! (`describe(n: Int)` → `describe_int`, `describe(label: String, times: Int = 2)` → `describe_string_int`,
//! `@Kompozable fun Chip(text: String)` → `Chip_string`, `fun take(x: Int)` of a class → `take_int`); a name declared
//! once stays as it is. A call is resolved as Kotlin resolves it: the overloads its arguments are applicable to (their
//! number, names and lambdas, their types as far as KompozR can tell: a literal's), then the most specific (an
//! argument of the parameter's own type before one it converts to: `describe(3)` is `describe(n: Int)`, not
//! `describe(d: Double)`), then the one needing no default (`put("b")` is `put(x: String)`, not `put(x: String,
//! times: Int = 2)`), then the first declared. Rust code calls them by their own names (each `Kotlin:` named in the
//! translation's doc).

use super::Gen;
use super::names::{is_pascal, snake};
use crate::ast::*;

/// The tag of a parameter's type in an overload's Rust name (`int`, `string`, `fn`, `list`, a class's name).
fn type_tag(t: &Type) -> String {
    let base = match &t.kind {
        TypeKind::Function(_) => "fn".to_string(),
        TypeKind::Named(segs) => {
            let last = segs.last().map(|s| s.name.name.as_str()).unwrap_or("");
            match last {
                "Int" => "int".into(),
                "Long" => "long".into(),
                "Float" => "float".into(),
                "Double" => "double".into(),
                "Boolean" => "boolean".into(),
                "Char" => "char".into(),
                "String" | "CharSequence" => "string".into(),
                "List" | "MutableList" | "Collection" | "Iterable" | "Array" => {
                    let arg = segs.last().and_then(|s| s.args.first()).and_then(|a| match a {
                        TypeArg::Type { ty, .. } => Some(type_tag(ty)),
                        TypeArg::Star(_) => None,
                    });
                    match arg {
                        Some(a) => format!("list_{a}"),
                        None => "list".into(),
                    }
                }
                other => {
                    let mut chars = other.chars();
                    let lowered: String =
                        chars.next().map(|c| c.to_lowercase().chain(chars).collect()).unwrap_or_default();
                    snake(&lowered)
                }
            }
        }
        TypeKind::Error => "x".into(),
    };
    if t.nullable { format!("{base}_or_null") } else { base }
}

/// The Rust name of one of a name's overloads: the name, then its parameters' types (`describe_string_int`; a
/// vararg's `int_vararg`; none: `describe_0`).
pub(crate) fn mangled(rust: &str, params: &[Param]) -> String {
    if params.is_empty() {
        return format!("{rust}_0");
    }
    let tags: Vec<String> = params
        .iter()
        .map(|p| {
            let t = p.ty.as_ref().map(type_tag).unwrap_or_else(|| "x".into());
            if p.is_vararg() { format!("{t}_vararg") } else { t }
        })
        .collect();
    format!("{rust}_{}", tags.join("_"))
}

/// A receiver type's text, as overloads of extension functions are told apart by it.
fn receiver_key(f: &Function) -> Option<String> {
    f.receiver.as_ref().map(|r| r.simple_name().unwrap_or_default())
}

impl Gen<'_, '_> {
    /// Notes the top-level functions declared more than once (by name and receiver: the text's and the module
    /// table's).
    pub(crate) fn collect_overloads<'d>(&mut self, decls: impl Iterator<Item = &'d Decl>) {
        let mut seen: std::collections::HashMap<(String, Option<String>), usize> = Default::default();
        for d in decls {
            if let Decl::Function(f) = d {
                *seen.entry((f.name.name.clone(), receiver_key(f))).or_default() += 1;
                if f.receiver.is_none() {
                    self.control.fn_params.entry(f.name.name.clone()).or_default().push(f.params.clone());
                }
            }
            // ((stage 14 WPD) a class's constructor beside a function of its name — NavigationSamples' `data class
            // Dashboard` and `@Composable fun Dashboard(navController, title)`, which Kotlin keeps apart: the class's
            // `Dashboard(…)` keeps its name, the function's is mangled)
            if let Decl::Class(c) = d
                && !matches!(
                    c.kind,
                    ClassKind::Interface | ClassKind::FunInterface | ClassKind::Enum | ClassKind::Annotation
                )
            {
                *seen.entry((c.name.name.clone(), None)).or_default() += 1;
            }
        }
        self.control.overloaded = seen.into_iter().filter(|(_, n)| *n > 1).map(|(k, _)| k).collect();
    }

    /// A top-level function's Rust name: a PascalCase one (a composable) as it is, others in snake_case; one of
    /// several overloads mangled ([`mangled`]).
    pub(crate) fn fn_rust_name(&self, f: &Function) -> String {
        let plain = if is_pascal(&f.name.name) { f.name.name.clone() } else { snake(&f.name.name) };
        if self.control.overloaded.contains(&(f.name.name.clone(), receiver_key(f))) {
            mangled(&plain, &f.params)
        } else {
            plain
        }
    }

    /// A member function's Rust name in a class (or interface, object) whose members are `members`: one of several
    /// of its name mangled, as is an override of an interface's overloaded function (the interface's name for it).
    pub(crate) fn member_rust_name(&self, members: &[Member], f: &Function, supers: &[String]) -> String {
        // (Any's members overridden: beside Rust's own, which they make — `Display`, `PartialEq`, `Hash`)
        match (f.name.name.as_str(), f.params.len()) {
            ("toString", 0) => return "to_string_kt".to_string(),
            ("equals", 1) => return "equals_kt".to_string(),
            ("hashCode", 0) => return "hash_code_kt".to_string(),
            _ => {}
        }
        let plain = snake(&f.name.name);
        let own = members
            .iter()
            .filter(
                |m| matches!(m, Member::Decl(Decl::Function(g)) if g.name.name == f.name.name && g.receiver.is_none()),
            )
            .count();
        let inherited = supers.iter().any(|s| {
            crate::index::overlay::current().is_some_and(|o| {
                o.items.iter().filter(|i| i.owner == s.as_str() && i.kotlin == f.name.name.as_str()).count() > 1
            })
        });
        if own > 1 || inherited { mangled(&plain, &f.params) } else { plain }
    }

    /// The overload of the file's function `name` a call's arguments are for, as inference reads it (their number,
    /// names and types: the call itself resolves it fully, `Gen::choose_file_fn`).
    pub(crate) fn peek_file_fn(
        &self,
        name: &str,
        args: &[Arg],
        trailing: Option<&Lambda>,
    ) -> Option<&super::resolve::FileFn> {
        let candidates: Vec<&super::resolve::FileFn> =
            self.file.functions.get(name)?.iter().filter(|f| f.receiver.is_none()).collect();
        if candidates.len() <= 1 {
            return candidates.first().copied();
        }
        let given = args.len() + usize::from(trailing.is_some());
        let fits = |f: &super::resolve::FileFn| -> Option<i32> {
            let vararg = f.params.iter().any(|p| p.vararg);
            let required = f.params.iter().filter(|p| p.default == super::resolve::PDefault::None && !p.vararg).count();
            if given < required || (given > f.params.len() && !vararg) {
                return None;
            }
            let mut exact = 0;
            for (k, a) in args.iter().enumerate() {
                let p = match &a.name {
                    Some(n) => f.params.iter().find(|p| p.kotlin == n.name)?,
                    None => f.params.get(k).or_else(|| f.params.last().filter(|p| p.vararg))?,
                };
                let t = self.peek_ty(&a.value);
                let t = if a.spread { t.element() } else { t };
                let want = if p.vararg { p.ty().element() } else { p.ty() };
                if t != super::resolve::Ty::Unknown && want != super::resolve::Ty::Unknown {
                    if t == want {
                        exact += 2;
                    } else if !(t.is_number() && want.is_number()) {
                        exact -= 3;
                    }
                }
            }
            Some(exact)
        };
        candidates
            .into_iter()
            .filter_map(|f| fits(f).map(|s| (s, f)))
            .fold(None, |best, (s, f)| match best {
                Some((b, _)) if b >= s => best,
                _ => Some((s, f)),
            })
            .map(|(_, f)| f)
    }

    /// ((stage 14 WPD) a call of a name that is a class's and a function's: whether its arguments are for one of the
    /// file's functions of that name (their number and names), else for the class's constructor.
    /// (`positional`: the arguments not named, a plain value's expression: one of a text, number or boolean type
    /// where the parameter's is another, or of another type where it is one of those, doesn't fit it.)
    pub(crate) fn fits_file_fn(&self, name: &str, named: &[&str], positional: &[Option<&Expr>], given: usize) -> bool {
        use super::resolve::Ty;
        let basic = |t: &Ty| t.is_number() || matches!(t, Ty::String | Ty::Bool | Ty::Char);
        self.file.functions.get(name).is_some_and(|fs| {
            fs.iter().filter(|f| f.receiver.is_none()).any(|f| {
                let vararg = f.params.iter().any(|p| p.vararg);
                let required =
                    f.params.iter().filter(|p| p.default == super::resolve::PDefault::None && !p.vararg).count();
                let typed = positional.iter().zip(&f.params).all(|(e, p)| {
                    let Some(e) = e else { return true };
                    let (t, want) = (self.peek_ty(e), p.ty().non_null());
                    t == Ty::Unknown
                        || want == Ty::Unknown
                        || t.non_null() == want
                        || (t.is_number() && want.is_number())
                        || !(basic(&t.non_null()) || basic(&want))
                });
                given >= required
                    && (given <= f.params.len() || vararg)
                    && named.iter().all(|n| f.params.iter().any(|p| p.kotlin == *n))
                    && typed
            })
        })
    }

    /// Whether `name` is a class of the user's with a constructor function of that name beside a file function.
    pub(crate) fn class_beside_fn(&self, name: &str) -> bool {
        self.file.function(name).is_some()
            && self
                .user_class(name)
                .is_some_and(|c| matches!(c.shape, super::class::Shape::Data | super::class::Shape::Plain))
    }

    /// A member function's Rust name in the user's type of Rust name `owner` ([`Gen::member_rust_name`]).
    pub(crate) fn member_fn_rust(&self, owner: &str, f: &Function) -> String {
        let decl = self.classes.decls.iter().find(|d| d.info.rust == owner);
        let (members, supers): (&[Member], Vec<String>) = match decl.map(|d| &d.ast) {
            Some(super::class::ClassAst::Class(k)) => (&k.body.members, self.super_rust_names(&k.supers)),
            Some(super::class::ClassAst::Object(o)) => (&o.body.members, self.super_rust_names(&o.supers)),
            None => (&[], Vec::new()),
        };
        self.member_rust_name(members, f, &supers)
    }

    /// Rust names of the supertypes a class or object expression names (for [`Gen::member_rust_name`]).
    pub(crate) fn super_rust_names(&self, supers: &[SuperType]) -> Vec<String> {
        supers
            .iter()
            .filter_map(|s| s.ty.simple_name())
            .map(|n| self.user_class(&n).map(|c| c.rust.clone()).unwrap_or(n))
            .collect()
    }
}
