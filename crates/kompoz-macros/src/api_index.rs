//! The API index: which functions and methods take named, defaulted parameters (`#[named]`, `#[kompozable]`), and
//! which of their parameters have a default. What the macros can't see (other crates, other items of a crate) they
//! read here: a call given more positional arguments than the parameters without a default goes through the
//! callee's builder (`Font(source, FontWeight::Bold)`, `color.copy(0.5)`), a named call of a PascalCase function
//! that isn't a composable goes through its type (`TextStyle::__kz_builder()`).
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
//!   `#[default(…)]`, `d` for one with; `-` for none.
//! - `owner`: where it is, for readers (the macros don't use it): the file, relative to the workspace root, with
//!   `/` separators; for a method then a space and the impl's type as written (its last path segment, generics
//!   left out), and ` as <Trait>` for a `#[named(trait Trait)]` impl.
//!
//! What is listed: every `#[named]` and `#[kompozable]` function, and every method of a `#[named]` impl that has a
//! defaulted parameter (the others are plain methods), of every target of every workspace package (`src/`,
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
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Fn => "fn",
            Kind::Kompozable => "kompozable",
            Kind::Method => "method",
        }
    }

    fn parse(s: &str) -> Option<Kind> {
        match s {
            "fn" => Some(Kind::Fn),
            "kompozable" => Some(Kind::Kompozable),
            "method" => Some(Kind::Method),
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
    /// Where it is, for readers (see the format).
    pub owner: String,
}

impl Entry {
    /// How many parameters have no default.
    pub fn required(&self) -> usize {
        self.defaults.iter().filter(|d| !**d).count()
    }

    /// The `parameters` field.
    pub fn pattern(&self) -> String {
        if self.defaults.is_empty() {
            return "-".into();
        }
        self.defaults.iter().map(|d| if *d { 'd' } else { 'r' }).collect()
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
# The format: crates/kompoz-macros/src/api_index.rs. Parameters: r without a default, d with one.
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
        let kind = Kind::parse(kind)
            .ok_or_else(|| format!("api index, line {n}: a kind is `fn`, `kompozable` or `method`, not `{kind}`"))?;
        let name = next("name")?.to_string();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!("api index, line {n}: `{name}` isn't a name"));
        }
        let defaults = match next("parameters")? {
            "-" => Vec::new(),
            p => p
                .chars()
                .map(|c| match c {
                    'r' => Ok(false),
                    'd' => Ok(true),
                    _ => Err(format!("api index, line {n}: parameters are `r` and `d` (or `-`), not `{p}`")),
                })
                .collect::<Result<_, _>>()?,
        };
        let owner = next("owner")?.to_string();
        entries.push(Entry { scope, kind, name, defaults, owner });
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
                owner: "crates/kompoz-runtime/tests/macros.rs".into(),
            },
            Entry {
                scope: Scope::All,
                kind: Kind::Method,
                name: "copy".into(),
                defaults: vec![true; 4],
                owner: "crates/kompoz-graphics/src/constraints.rs Constraints".into(),
            },
            Entry {
                scope: Scope::All,
                kind: Kind::Kompozable,
                name: "Counter".into(),
                defaults: Vec::new(),
                owner: "examples/counter/src/main.rs".into(),
            },
        ];
        let text = render(&entries);
        assert!(text.ends_with(
            "* kompozable Counter - examples/counter/src/main.rs\n\
             kompoz-runtime/macros fn Twice rd crates/kompoz-runtime/tests/macros.rs\n\
             * method copy dddd crates/kompoz-graphics/src/constraints.rs Constraints\n"
        ));
        let mut back = parse(&text).unwrap();
        back.sort();
        let mut want = entries;
        want.sort();
        assert_eq!(back, want);
        assert_eq!(want.iter().map(Entry::required).collect::<Vec<_>>(), [0, 0, 1]);
        assert!(parse("* fn x rx f").unwrap_err().contains("line 1"));
        assert!(parse("# c\n\nmine fn x r f").unwrap_err().contains("line 3"));
    }
}
