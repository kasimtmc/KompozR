//! The module table: what the other sources of the crate being compiled declare, so that a `kompoz!` block or a
//! `.kt` file sees them as Kotlin sees a package's declarations (another file's classes, objects, functions,
//! properties and type aliases), and the Rust items of the crate (its `pub` functions, structs, enums, constants,
//! `thread_local!`s, type aliases) as it sees the API index's.
//!
//! [`scan`] reads a crate's sources the way rustc does: from its root file (found by the crate's name among its
//! targets' files, or by the file a macro is called from) through its `mod` declarations, the `kompoz! { … }` blocks
//! (their Kotlin text as written, at its file's lines and columns) and the `.kt` files `kompoz_file!("…")` reads in
//! them, and a `syn` scan of the `pub` Rust items. [`cached`] keeps one table per crate root for the life of the
//! process (a rustc, rust-analyzer's proc-macro server, a language server), made again when one of the files it read
//! changes (their modification times and lengths are checked on each use).
//!
//! The generator reads the table's Kotlin sources as declarations only (headers: classes and their members,
//! objects, functions with their parameters, properties with their types, type aliases); a declaration of the
//! translation's own text wins over one of the table, and a `private` one of another file isn't seen (Kotlin's
//! file-private).

mod rust_items;
mod scan;

pub use rust_items::rust_items;
pub use scan::{cached, scan};

use crate::pos::Pos;
use std::path::PathBuf;

/// A crate's other sources, as the generator sees them.
#[derive(Clone, Debug, Default)]
pub struct ModuleTable {
    /// The Kotlin sources: each `kompoz!` block's text, each `.kt` file `kompoz_file!` reads.
    pub sources: Vec<ModuleSource>,
    /// The crate's own `pub` Rust items.
    pub rust: Vec<RustItem>,
    /// The files read (a dependency of whatever uses the table): `.kt` files and the crate's `.rs` files.
    pub files: Vec<PathBuf>,
}

/// One Kotlin source of the crate.
#[derive(Clone, Debug)]
pub struct ModuleSource {
    /// The file it is in (a `.rs` file holding a `kompoz!` block, or a `.kt` file).
    pub file: PathBuf,
    /// The Kotlin text: a `kompoz!` block's from just after its opening delimiter to just before its closing one, a
    /// `.kt` file whole (no byte order mark, LF line ends).
    pub text: String,
    /// Where the text starts in its file (a `.kt` file: 1:1).
    pub base: Pos,
    /// It is a `.kt` file (read by `kompoz_file!`), not a `kompoz!` block.
    pub kt_file: bool,
}

/// A `pub` Rust item of the crate, as the API index has its items (see [`crate::index::Item`]; owned here, the
/// generator interns them).
///
/// Besides the index's kinds:
/// - a type (a `pub` struct or enum, or the type a `#[named(type)]` function makes) is an item of kind
///   [`crate::index::Kind::Value`] whose [`owner`](RustItem::owner), [`rust`](RustItem::rust) and
///   [`kotlin`](RustItem::kotlin) are all its name ([`ret`](RustItem::ret) too), with no flags;
/// - a type alias, `pub type Name = Target;`, is an item of kind [`crate::index::Kind::Value`], no owner, with
///   [`alias`](RustItem::alias) its target;
/// - a `thread_local! { pub static X: T = …; }` is a [`crate::index::Kind::Value`] with
///   [`thread_local`](RustItem::thread_local) set.
///
/// "`pub`" is any visibility but the inherited one and `pub(self)` (`pub(crate)`, `pub(super)` count).
#[derive(Clone, Debug, PartialEq)]
pub struct RustItem {
    /// Kotlin's name: the Rust one in camelCase for functions, methods and fields (`set_title` → `setTitle`),
    /// PascalCase and constants as they are, or what a doc line ``Kotlin: `name(…)` `` says.
    pub kotlin: String,
    pub rust: String,
    pub kind: crate::index::Kind,
    /// The module it is in, `::`-separated from the crate's root (`screens::list`; empty at the root).
    pub module: String,
    /// A method's, associated function's or constant's, field property's, enum variant's type; empty for a free item.
    pub owner: String,
    /// A `#[named(trait Name)]` impl's trait.
    pub via: String,
    pub receiver: crate::index::Receiver,
    pub params: Vec<RustParam>,
    /// Its type or result, Rust's text (as `quote` prints it, spaces normalized: `Vec<String>`, `Option<Rc<dyn Fn()>>`).
    pub ret: String,
    /// [`crate::index::flags`].
    pub flags: u16,
    /// A `thread_local!` static (read with `.with(…)`); a `static`/`const` is read in place.
    pub thread_local: bool,
    /// A `type Name = Target;`'s target (the item's kind is then [`crate::index::Kind::Value`], its owner empty).
    pub alias: Option<String>,
}

/// A parameter of a Rust function or method.
#[derive(Clone, Debug, PartialEq)]
pub struct RustParam {
    pub rust: String,
    /// Kotlin's name (camelCase of the Rust one).
    pub kotlin: String,
    pub ty: String,
    /// A `#[default(expr)]`'s expression (Rust's text).
    pub default: Option<String>,
}
