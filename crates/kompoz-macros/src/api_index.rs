//! The API index: which functions and methods take named, defaulted parameters (`#[named]`, `#[kompozable]`), and
//! which of their parameters have a default. What the macros can't see (other crates, other items of a crate) they
//! read here: a call given more positional arguments than the parameters without a default goes through the
//! callee's builder (`Font(source, FontWeight::Bold)`, `color.copy(0.5)`), a named call of a PascalCase function
//! that isn't a composable goes through its type (`TextStyle::__kz_builder()`), and a call of one with a
//! `#[memoized]` parameter goes through its builder whenever its arguments fill the parameters (which gives the
//! closure it is given its identity: `LocalX.provides_computed(|s| …)`).
//!
//! `cargo xtask kotlin-index` writes it (`crates/kompoz-macros/api-index.txt`, checked in; `--check` in CI) with
//! [`render`]; kompoz-macros reads it with [`parse`]. This file is shared as it is (`#[path]`): std only.
//!
//! # Format
//!
//! UTF-8 text, one item per line; lines starting with `#` and blank lines are ignored. An item's line is five
//! fields separated by single spaces, the last running to the end of the line:
//!
//! ```text
//! <scope> <kind> <name> <parameters> <owner>
//! * fn Constraints dddd crates/kompoz-graphics/src/constraints.rs
//! * fn Font rdddd crates/kompoz-text/src/font/font.rs
//! * method copy dddd crates/kompoz-graphics/src/constraints.rs Constraints
//! * method padding dddd crates/kompoz-foundation/src/layout/padding.rs Modifier as FoundationLayoutModifiers
//! * method provides_computed R crates/kompoz-runtime/src/local.rs KompozitionLocal
//! * kompozable Counter - examples/counter/src/main.rs
//! kompoz-runtime/macros fn Twice rd crates/kompoz-runtime/tests/macros.rs
//! ```
//!
//! - `scope`: `*` for an item of a library (`src/` of a package with a lib target, but not `src/main.rs` or
//!   `src/bin/`): it counts in every crate (one that can't call it never names it). `<package>/<crate>` for an item
//!   of another target, counting only there: the package's name as in its Cargo.toml and the target's crate name
//!   (`kompoz-runtime/macros` for `crates/kompoz-runtime/tests/macros.rs`, `counter/counter` for the binary of the
//!   package `counter`): cargo's `CARGO_PKG_NAME` and `CARGO_CRATE_NAME` when it compiles that target. An item of
//!   a file several targets include (`tests/common/mod.rs`) has a line for each.
//! - `kind`: `fn` (a `#[named]` function), `kompozable` (a `#[kompozable]` function) or `method` (a method of a
//!   `#[named]` impl, `#[named(trait …)]` ones included).
//! - `name`: the function's or method's name as written (no `r#`).
//! - `parameters`: one letter per parameter in order (the receiver not counted), `r` for one without a
//!   `#[default(…)]`, `d` for one with, in upper case (`R`, `D`) for a `#[memoized]` one; `-` for none.
//! - `owner`: where it is, for readers (the macros don't use it): the file, relative to the workspace root, with
//!   `/` separators; for a method then a space and the impl's type as written (its last path segment, generics
//!   left out; `{T}` for an impl of every type of its type parameter `T`, a blanket one), and ` as <Trait>` for a
//!   `#[named(trait Trait)]` impl. The macros read the type and the trait: a method call whose receiver has a type
//!   they see (a typed parameter or `let`) that is none of them, nor has one as a supertype (below), keeps its plain
//!   call.
//! - `super` lines (`* super VectorPainter - Painter`): the API's supertypes of a type as Kotlin sees them (what it
//!   derefs to, the traits of the API it implements, a trait's supertraits; the Kotlin index's `DEREFS`), `name` the
//!   type, `-` for the parameters, `owner` the supertype.
//!
//! What is listed: every `#[named]` and `#[kompozable]` function, and every method of a `#[named]` impl that has a
//! defaulted or `#[memoized]` parameter (the others are plain methods), of every target of every workspace package (`src/`,
//! `tests/` but not trybuild's `tests/ui/`, `examples/`, `benches/`; items inside function bodies too, not what
//! macro invocations hold: `macro_rules!` bodies, `quote!`), and nothing else. Lines are sorted by name, then
//! scope (`*` first, then by package, then crate), kind (`fn`, `kompozable`, `method`), parameters and owner
//! (byte order), with no duplicates; the file starts with the header [`render`] writes.

#![allow(dead_code)]

use std::fmt::Write as _;

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A `#[named]` function.
    Fn,
    /// A `#[kompozable]` function.
    Kompozable,
    /// A method of a `#[named]` impl.
    Method,
    /// What a type has the members of besides its own (Kotlin's supertypes: what it derefs to, the API's traits
    /// it implements, a trait's supertraits): `name` the type, `owner` the supertype.
    Super,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Fn => "fn",
            Kind::Kompozable => "kompozable",
            Kind::Method => "method",
            Kind::Super => "super",
        }
    }

    fn parse(s: &str) -> Option<Kind> {
        match s {
            "fn" => Some(Kind::Fn),
            "kompozable" => Some(Kind::Kompozable),
            "method" => Some(Kind::Method),
            "super" => Some(Kind::Super),
            _ => None,
        }
    }
}

/// Where an item counts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// A library's: everywhere.
    All,
    /// Another target's: only in the crate of that package and crate name.
    Target { package: String, krate: String },
}

impl Scope {
    /// Whether it counts in the crate cargo is compiling as `package`'s `krate`.
    pub fn includes(&self, package: &str, krate: &str) -> bool {
        match self {
            Scope::All => true,
            Scope::Target { package: p, krate: k } => p == package && k == krate,
        }
    }

    fn render(&self, out: &mut String) {
        match self {
            Scope::All => out.push('*'),
            Scope::Target { package, krate } => {
                let _ = write!(out, "{package}/{krate}");
            }
        }
    }
}

/// An item of the index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entry {
    pub scope: Scope,
    pub kind: Kind,
    pub name: String,
    /// Whether each parameter has a default, in order (not the receiver).
    pub defaults: Vec<bool>,
    /// Whether each parameter is `#[memoized]`, in order (as many as `defaults`).
    pub memoized: Vec<bool>,
    /// Where it is, for readers (see the format).
    pub owner: String,
}

impl Entry {
    /// How many parameters have no default.
    pub fn required(&self) -> usize {
        self.defaults.iter().filter(|d| !**d).count()
    }

    /// Whether a parameter is `#[memoized]`.
    pub fn has_memoized(&self) -> bool {
        self.memoized.contains(&true)
    }

    /// The `parameters` field.
    pub fn pattern(&self) -> String {
        if self.defaults.is_empty() {
            return "-".into();
        }
        self.defaults
            .iter()
            .zip(&self.memoized)
            .map(|(d, m)| match (*d, *m) {
                (false, false) => 'r',
                (true, false) => 'd',
                (false, true) => 'R',
                (true, true) => 'D',
            })
            .collect()
    }

    /// The order lines are written in.
    fn sort_key(&self) -> (&str, &Scope, Kind, String, &str) {
        (&self.name, &self.scope, self.kind, self.pattern(), &self.owner)
    }
}

/// The header [`render`] starts the file with.
pub const HEADER: &str = "\
# KompozR's API index: the #[named] and #[kompozable] functions and the #[named] methods with defaults.
# Written by `cargo xtask kotlin-index` (don't edit by hand); read by kompoz-macros and kompoz-kotlin.
# The format: crates/kompoz-macros/src/api_index.rs. Parameters: r without a default, d with one (R, D #[memoized]).
# <scope> <kind> <name> <parameters> <owner>
";

/// The index's text: [`HEADER`], then the entries sorted, without duplicates.
pub fn render(entries: &[Entry]) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    sorted.dedup();
    let mut out = String::from(HEADER);
    for e in sorted {
        e.scope.render(&mut out);
        let _ = writeln!(out, " {} {} {} {}", e.kind.as_str(), e.name, e.pattern(), e.owner);
    }
    out
}

/// The entries of an index's text, or what is wrong with it (with its line number).
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.splitn(5, ' ');
        let mut next = |what: &str| fields.next().ok_or_else(|| format!("api index, line {n}: no {what}"));
        let scope = match next("scope")? {
            "*" => Scope::All,
            s => match s.split_once('/') {
                Some((package, krate)) if !package.is_empty() && !krate.is_empty() && !krate.contains('/') => {
                    Scope::Target { package: package.to_string(), krate: krate.to_string() }
                }
                _ => return Err(format!("api index, line {n}: a scope is `*` or `<package>/<crate>`, not `{s}`")),
            },
        };
        let kind = next("kind")?;
        let kind = Kind::parse(kind).ok_or_else(|| {
            format!("api index, line {n}: a kind is `fn`, `kompozable`, `method` or `super`, not `{kind}`")
        })?;
        let name = next("name")?.to_string();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!("api index, line {n}: `{name}` isn't a name"));
        }
        let (defaults, memoized) = match next("parameters")? {
            "-" => (Vec::new(), Vec::new()),
            p => p
                .chars()
                .map(|c| match c {
                    'r' => Ok((false, false)),
                    'd' => Ok((true, false)),
                    'R' => Ok((false, true)),
                    'D' => Ok((true, true)),
                    _ => Err(format!("api index, line {n}: parameters are `r`, `d`, `R` and `D` (or `-`), not `{p}`")),
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .unzip(),
        };
        let owner = next("owner")?.to_string();
        entries.push(Entry { scope, kind, name, defaults, memoized, owner });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_render_writes_parse_reads() {
        let entries = vec![
            Entry {
                scope: Scope::Target { package: "kompoz-runtime".into(), krate: "macros".into() },
                kind: Kind::Fn,
                name: "Twice".into(),
                defaults: vec![false, true],
                memoized: vec![false; 2],
                owner: "crates/kompoz-runtime/tests/macros.rs".into(),
            },
            Entry {
                scope: Scope::All,
                kind: Kind::Method,
                name: "copy".into(),
                defaults: vec![true; 4],
                memoized: vec![false; 4],
                owner: "crates/kompoz-graphics/src/constraints.rs Constraints".into(),
            },
            Entry {
                scope: Scope::All,
                kind: Kind::Kompozable,
                name: "Counter".into(),
                defaults: Vec::new(),
                memoized: Vec::new(),
                owner: "examples/counter/src/main.rs".into(),
            },
            Entry {
                scope: Scope::All,
                kind: Kind::Kompozable,
                name: "Layout".into(),
                defaults: vec![true, true, false],
                memoized: vec![false, true, false],
                owner: "crates/kompoz-ui/src/layout/composables.rs".into(),
            },
        ];
        let text = render(&entries);
        assert!(text.ends_with(
            "* kompozable Counter - examples/counter/src/main.rs\n\
             * kompozable Layout dDr crates/kompoz-ui/src/layout/composables.rs\n\
             kompoz-runtime/macros fn Twice rd crates/kompoz-runtime/tests/macros.rs\n\
             * method copy dddd crates/kompoz-graphics/src/constraints.rs Constraints\n"
        ));
        let mut back = parse(&text).unwrap();
        back.sort();
        let mut want = entries;
        want.sort();
        assert_eq!(back, want);
        assert_eq!(want.iter().map(Entry::required).collect::<Vec<_>>(), [0, 1, 0, 1]);
        assert_eq!(want.iter().map(Entry::has_memoized).collect::<Vec<_>>(), [false, true, false, false]);
        assert!(parse("* fn x rx f").unwrap_err().contains("line 1"));
        assert!(parse("# c\n\nmine fn x r f").unwrap_err().contains("line 3"));
    }
}
