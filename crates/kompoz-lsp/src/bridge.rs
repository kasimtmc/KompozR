//! The seam for a second source of facts about the Kotlin: the shadow-project bridge (stage 14's WPE, the design's
//! U1 second half) — a rust-analyzer workspace of its own over the generated Rust, rustc's type errors mapped back
//! onto `.kt` and `kompoz!` lines, type-accurate hovers. The server works without one; with one installed
//! ([`crate::server::Server::set_bridge`]) it is told of every translation and asked where the server's own
//! knowledge ends:
//!
//! - after the documents' Kotlin is translated, [`Bridge::translated`] (the documents as they are now, and a
//!   sender of [`Event`]s: a bridge working on threads of its own comes back to the server with [`Event::Call`],
//!   e.g. to publish a document's diagnostics again once it has its own: [`crate::server::Server::publish`]);
//! - when the server publishes a document's diagnostics, [`Bridge::diagnostics`] adds the bridge's;
//! - a hover inside the Kotlin the generator knows nothing of goes to [`Bridge::hover`], which may answer later
//!   through its [`Reply`];
//! - [`Bridge::shutdown`] when the editor shuts the server down.

use crate::doc::Doc;
use crate::json::Json;
use crate::rpc;
use crate::server::Event;
use std::collections::HashMap;
use std::sync::mpsc::Sender;

/// A second source of facts about the Kotlin (see the module's doc).
pub trait Bridge {
    /// The documents' Kotlin was translated again (`docs`: every open document, by URI).
    fn translated(&mut self, docs: &HashMap<String, Doc>, events: &Sender<Event>);

    /// Its diagnostics of a document (LSP's), published with the generator's and rust-analyzer's.
    fn diagnostics(&self, uri: &str) -> Vec<Json> {
        let _ = uri;
        Vec::new()
    }

    /// A hover at `offset` in `doc`'s Kotlin the server has none of: answer through `reply` (now or later), or give it
    /// back (`Err`) to answer nothing.
    fn hover(&mut self, doc: &Doc, offset: usize, reply: Reply) -> Result<(), Reply> {
        let _ = (doc, offset);
        Err(reply)
    }

    /// The editor shuts the server down.
    fn shutdown(&mut self) {}
}

/// The answer to an editor's request, given once (from any thread).
pub struct Reply {
    pub(crate) id: Json,
    pub(crate) editor: Sender<Json>,
}

impl Reply {
    pub fn send(self, result: Json) {
        let _ = self.editor.send(rpc::response(self.id, result));
    }
}
