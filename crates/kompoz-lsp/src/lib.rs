//! KompozR's language server, `kompoz-lsp`: one server for a crate's Rust and its Kotlin syntax (`kompoz! { … }`
//! blocks in `.rs` files, `.kt` files read by `kompoz_file!`), what the C++ Kompoz's `kompozc --lsp` is to clangd.
//!
//! - [`server`]: the editor on stdin/stdout, the toolchain's rust-analyzer as a child (started when first needed,
//!   once) for everything outside the Kotlin; inside it, the server's own answers.
//! - [`kotlin`]: those answers, from kompoz-kotlin's translation of each Kotlin region with the analysis on
//!   ([`kompoz_kotlin::analysis`]) and the crate's module table ([`workspace`]): diagnostics, semantic tokens
//!   ([`tokens`]), completion, hover, definition (an API item's through rust-analyzer's workspace symbols),
//!   highlights, document symbols. No Kotlin formatter: formatting there is refused.
//! - [`doc`], [`text`]: open documents, their Kotlin regions and positions (LSP's UTF-16, rustc's view of the text
//!   the Kotlin's offsets are into).
//! - [`bridge`]: the seam for a second source of facts about the Kotlin (the shadow-project bridge).
//! - [`json`], [`rpc`]: JSON and LSP's framing, the server's own.
//! - [`kate`]: `kompoz-lsp --kate`, Kate's highlighting definition and its LSP client's settings.

pub mod bridge;
pub mod doc;
pub mod json;
pub mod kate;
pub mod kotlin;
pub mod rpc;
pub mod server;
pub mod text;
pub mod tokens;
pub mod workspace;
