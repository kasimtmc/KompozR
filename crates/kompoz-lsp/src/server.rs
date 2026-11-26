//! The server: the editor on stdin/stdout, rust-analyzer (the toolchain's) as a child behind it.
//!
//! A `.rs` file goes to rust-analyzer as it is — it reads the Rust around the Kotlin itself, and expands `kompoz!`
//! with the macro as rustc does —, a `.kt` file never does (it isn't Rust). Inside the Kotlin (a `kompoz!` block, a
//! `.kt` file) the server answers from the generator's translation of it ([`crate::kotlin`]): its diagnostics,
//! semantic tokens, completion, hover, definition, highlights, document symbols; formatting there is refused (there
//! is no Kotlin formatter: rustfmt's edits touching the Kotlin are dropped, a range or a keystroke in it gets none).
//! Everywhere else rust-analyzer answers, and what it answers about a document with Kotlin is kept off the Kotlin
//! (its tokens, hints, symbols, edits and diagnostics there are of the macro's expansion).
//!
//! rust-analyzer is started when it is first needed (a Rust document opened, a workspace symbol asked for), once: it
//! is a big process. The server answers `initialize` itself, with rust-analyzer 1.99's capabilities (as the editor's
//! capabilities make them) and one semantic tokens legend for both. Positions are UTF-16 (LSP's default) on both
//! sides. Without rust-analyzer (not installed, `--no-rust-analyzer`, or gone) the Kotlin's answers remain.
//!
//! Translations run a moment after the last change (or at once when a request needs them); a document's change
//! makes the open documents of its crate due too (its declarations are their module table's).

use crate::bridge::{Bridge, Reply};
use crate::doc::{Doc, Lang};
use crate::json::Json;
use crate::{kotlin, rpc, tokens, workspace};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

/// How the server runs.
#[derive(Clone, Debug)]
pub struct Config {
    /// rust-analyzer's command (`None`: none, the Kotlin's answers alone).
    pub rust_analyzer: Option<PathBuf>,
    /// Its arguments.
    pub analyzer_args: Vec<String>,
    /// How long after the last change the Kotlin is translated.
    pub delay: Duration,
    /// The shadow-project bridge's rust-analyzer ([`crate::shadow`]; `None`: no bridge).
    pub shadow: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            rust_analyzer: Some("rust-analyzer".into()),
            analyzer_args: Vec::new(),
            delay: Duration::from_millis(200),
            shadow: None,
        }
    }
}

/// What the server's loop handles.
pub enum Event {
    /// A message from the editor.
    Editor(Json),
    /// The editor's stream ended.
    EditorClosed,
    /// A message from rust-analyzer.
    Analyzer(Json),
    /// rust-analyzer's stream ended (it exited).
    AnalyzerClosed,
    /// Work for the server's thread (a [`Bridge`]'s, coming back).
    Call(Box<dyn FnOnce(&mut Server) + Send>),
}

/// What became of a request the server sent rust-analyzer.
enum Pending {
    /// The editor's, forwarded: its id, method, document.
    Editor { id: Json, method: String, uri: String },
    /// The server's own: what to do with the result (`None` on an error).
    Own(Box<dyn FnOnce(&mut Server, Option<Json>)>),
}

enum Analyzer {
    /// Not started yet.
    Idle,
    /// Started, its `initialize` not answered yet: what is to be sent after it.
    Starting {
        child: Child,
        tx: Sender<Json>,
        queue: Vec<Json>,
    },
    Ready {
        child: Child,
        tx: Sender<Json>,
    },
    /// None (not configured, failed to start, or exited).
    Gone,
}

pub struct Server {
    config: Config,
    events: Sender<Event>,
    editor: Sender<Json>,
    docs: HashMap<String, Doc>,
    /// The editor's `initialize` parameters (rust-analyzer gets them too).
    init: Json,
    analyzer: Analyzer,
    /// rust-analyzer's semantic tokens legend.
    analyzer_legend: Json,
    next_id: i64,
    pending: HashMap<i64, Pending>,
    /// rust-analyzer's requests to the editor, by the id the editor sees: their own id.
    analyzer_requests: HashMap<String, Json>,
    shut_down: bool,
    /// When the due translations run.
    due: Option<Instant>,
    bridge: Option<Box<dyn Bridge>>,
}

/// Runs the server on stdin/stdout until the editor's `exit`; the process's exit code.
pub fn run(config: Config) -> i32 {
    let (tx, rx) = mpsc::channel();
    let reader = tx.clone();
    std::thread::spawn(move || {
        let mut input = BufReader::new(std::io::stdin().lock());
        while let Ok(Some(m)) = rpc::read(&mut input) {
            if reader.send(Event::Editor(m)).is_err() {
                return;
            }
        }
        let _ = reader.send(Event::EditorClosed);
    });
    let editor = rpc::writer(std::io::stdout());
    let shadow = config.shadow.clone();
    let mut server = Server::new(config, tx, editor);
    if let Some(analyzer) = shadow {
        server.set_bridge(Box::new(crate::shadow::ShadowBridge::new(analyzer)));
    }
    server.run(rx)
}

impl Server {
    pub fn new(config: Config, events: Sender<Event>, editor: Sender<Json>) -> Server {
        let analyzer = if config.rust_analyzer.is_some() { Analyzer::Idle } else { Analyzer::Gone };
        Server {
            config,
            events,
            editor,
            docs: HashMap::new(),
            init: Json::Null,
            analyzer,
            analyzer_legend: Json::Null,
            next_id: 1,
            pending: HashMap::new(),
            analyzer_requests: HashMap::new(),
            shut_down: false,
            due: None,
            bridge: None,
        }
    }

    /// Installs the second source of facts about the Kotlin (see [`crate::bridge`]).
    pub fn set_bridge(&mut self, bridge: Box<dyn Bridge>) {
        self.bridge = Some(bridge);
    }

    /// The open documents, by URI.
    pub fn docs(&self) -> &HashMap<String, Doc> {
        &self.docs
    }

    /// Handles events until the editor's `exit` (or its stream's end).
    pub fn run(&mut self, rx: Receiver<Event>) -> i32 {
        loop {
            let event = match self.due {
                Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                    Ok(e) => e,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        self.translate_due();
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return 1,
                },
                None => match rx.recv() {
                    Ok(e) => e,
                    Err(_) => return 1,
                },
            };
            match event {
                Event::Editor(m) => {
                    if let Some(code) = self.on_editor(m) {
                        return code;
                    }
                }
                Event::EditorClosed => {
                    self.stop_analyzer();
                    return 1;
                }
                Event::Analyzer(m) => self.on_analyzer(m),
                Event::AnalyzerClosed => self.analyzer_gone(),
                Event::Call(f) => f(self),
            }
        }
    }

    fn to_editor(&self, m: Json) {
        let _ = self.editor.send(m);
    }

    fn respond(&self, id: &Json, result: Json) {
        self.to_editor(rpc::response(id.clone(), result));
    }

    // ---- rust-analyzer ---------------------------------------------------------------------------------------

    /// Whether there is (or will be) a rust-analyzer: started now when it wasn't yet.
    fn analyzer_available(&mut self) -> bool {
        if matches!(self.analyzer, Analyzer::Idle) {
            self.start_analyzer();
        }
        matches!(self.analyzer, Analyzer::Starting { .. } | Analyzer::Ready { .. })
    }

    fn start_analyzer(&mut self) {
        let Some(cmd) = self.config.rust_analyzer.clone() else {
            self.analyzer = Analyzer::Gone;
            return;
        };
        let child = Command::new(&cmd)
            .args(&self.config.analyzer_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                eprintln!("kompoz-lsp: no rust-analyzer ({}: {e}): the Kotlin's answers only", cmd.display());
                self.analyzer = Analyzer::Gone;
                return;
            }
        };
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            self.analyzer = Analyzer::Gone;
            return;
        };
        let tx = rpc::writer(stdin);
        let events = self.events.clone();
        std::thread::spawn(move || {
            let mut input = BufReader::new(stdout);
            while let Ok(Some(m)) = rpc::read(&mut input) {
                if events.send(Event::Analyzer(m)).is_err() {
                    return;
                }
            }
            let _ = events.send(Event::AnalyzerClosed);
        });
        // (the editor's parameters, positions in UTF-16, diagnostics pushed: the server merges them with its own)
        let mut params = self.init.clone();
        if params.is_null() {
            params = Json::object([("processId", Json::Null), ("capabilities", Json::new_object())]);
        }
        let caps = params.entry("capabilities");
        caps.entry("general").remove("positionEncodings");
        caps.remove("offsetEncoding");
        caps.entry("textDocument").remove("diagnostic");
        let id = self.take_id();
        let _ = tx.send(rpc::request(id, "initialize", params));
        self.pending.insert(
            id,
            Pending::Own(Box::new(|s, result| {
                let Some(result) = result else {
                    eprintln!("kompoz-lsp: rust-analyzer refused to initialize");
                    s.stop_analyzer();
                    return;
                };
                s.analyzer_legend = result["capabilities"]["semanticTokensProvider"]["legend"].clone();
                if let Analyzer::Starting { child, tx, queue } = std::mem::replace(&mut s.analyzer, Analyzer::Gone) {
                    let _ = tx.send(rpc::notification("initialized", Json::new_object()));
                    for m in queue {
                        let _ = tx.send(m);
                    }
                    s.analyzer = Analyzer::Ready { child, tx };
                }
            })),
        );
        self.analyzer = Analyzer::Starting { child, tx, queue: Vec::new() };
    }

    fn take_id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }

    /// Sends rust-analyzer a message (queued while it initializes); false when there is none.
    fn send_analyzer(&mut self, m: Json) -> bool {
        if !self.analyzer_available() {
            return false;
        }
        match &mut self.analyzer {
            Analyzer::Starting { queue, .. } => queue.push(m),
            Analyzer::Ready { tx, .. } => {
                let _ = tx.send(m);
            }
            _ => return false,
        }
        true
    }

    /// Sends rust-analyzer a request of the server's own; `then` gets its result (`None`: an error, or no
    /// rust-analyzer).
    fn analyzer_request(&mut self, method: &str, params: Json, then: impl FnOnce(&mut Server, Option<Json>) + 'static) {
        let id = self.take_id();
        if self.send_analyzer(rpc::request(id, method, params)) {
            self.pending.insert(id, Pending::Own(Box::new(then)));
        } else {
            then(self, None);
        }
    }

    /// Forwards the editor's request to rust-analyzer (its answer comes back through [`Server::answered`]), or
    /// answers it with nothing when there is none.
    fn forward_request(&mut self, mut m: Json, uri: &str) {
        let id = m["id"].clone();
        let method = m["method"].str().to_string();
        let own = self.take_id();
        m.set("id", own);
        if self.send_analyzer(m) {
            self.pending.insert(own, Pending::Editor { id, method, uri: uri.to_string() });
        } else {
            // (the Kotlin's part of the answer alone)
            let r = self.off_kotlin(&method, uri, empty_result(&method));
            self.respond(&id, r);
        }
    }

    fn stop_analyzer(&mut self) {
        match std::mem::replace(&mut self.analyzer, Analyzer::Gone) {
            Analyzer::Starting { mut child, tx, .. } | Analyzer::Ready { mut child, tx } => {
                let _ = tx.send(rpc::notification("exit", Json::Null));
                drop(tx);
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    if let Ok(Some(_)) = child.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = child.kill();
                let _ = child.wait();
            }
            _ => {}
        }
    }

    /// rust-analyzer exited: its pending requests answered with nothing, its diagnostics gone.
    fn analyzer_gone(&mut self) {
        if let Analyzer::Starting { mut child, .. } | Analyzer::Ready { mut child, .. } =
            std::mem::replace(&mut self.analyzer, Analyzer::Gone)
        {
            let _ = child.wait();
            if !self.shut_down {
                eprintln!("kompoz-lsp: rust-analyzer exited: the Kotlin's answers only");
            }
        }
        let pending: Vec<Pending> = self.pending.drain().map(|(_, p)| p).collect();
        for p in pending {
            match p {
                Pending::Editor { id, method, .. } => self.respond(&id, empty_result(&method)),
                Pending::Own(f) => f(self, None),
            }
        }
        let uris: Vec<String> = self.docs.keys().cloned().collect();
        for u in uris {
            if let Some(d) = self.docs.get_mut(&u) {
                d.analyzer_diagnostics = Json::Array(Vec::new());
                d.in_analyzer = false;
            }
            self.publish(&u);
        }
    }

    fn on_analyzer(&mut self, m: Json) {
        let method = m["method"].str().to_string();
        if method.is_empty() {
            // (an answer)
            let Some(id) = m["id"].as_i64() else { return };
            let Some(p) = self.pending.remove(&id) else { return };
            let result = if m.has("error") { None } else { Some(m["result"].clone()) };
            match p {
                Pending::Editor { id, method, uri } => match result {
                    Some(r) => {
                        let r = self.off_kotlin(&method, &uri, r);
                        self.respond(&id, r);
                    }
                    None => {
                        let mut m = m;
                        m.set("id", id);
                        self.to_editor(m);
                    }
                },
                Pending::Own(f) => f(self, result),
            }
            return;
        }
        if m.has("id") {
            // (rust-analyzer's request: the editor's to answer, under an id of the server's)
            let own = format!("kompoz-ra-{}", self.take_id());
            self.analyzer_requests.insert(Json::from(own.as_str()).dump(), m["id"].clone());
            let mut m = m;
            m.set("id", own);
            self.to_editor(m);
            return;
        }
        if method == "textDocument/publishDiagnostics" {
            let uri = m["params"]["uri"].str().to_string();
            if let Some(d) = self.docs.get_mut(&uri)
                && d.lang != Lang::Kotlin
            {
                d.analyzer_diagnostics = m["params"]["diagnostics"].clone();
                self.publish(&uri);
                return;
            }
        }
        self.to_editor(m);
    }

    // ---- The editor ------------------------------------------------------------------------------------------

    /// Handles the editor's message; `Some(code)` to exit with.
    fn on_editor(&mut self, m: Json) -> Option<i32> {
        let method = m["method"].str().to_string();
        if method.is_empty() {
            // (an answer to rust-analyzer's request)
            if let Some(id) = self.analyzer_requests.remove(&m["id"].dump()) {
                let mut m = m;
                m.set("id", id);
                self.send_analyzer(m);
            }
            return None;
        }
        let is_request = m.has("id");
        match method.as_str() {
            "initialize" => {
                self.init = m["params"].clone();
                let caps = capabilities(&self.init);
                self.respond(
                    &m["id"],
                    Json::object([
                        ("capabilities", caps),
                        (
                            "serverInfo",
                            Json::object([
                                ("name", "kompoz-lsp".into()),
                                ("version", env!("CARGO_PKG_VERSION").into()),
                            ]),
                        ),
                    ]),
                );
            }
            "initialized" => {}
            "shutdown" => {
                self.shut_down = true;
                if let Some(b) = self.bridge.as_mut() {
                    b.shutdown();
                }
                let id = m["id"].clone();
                if matches!(self.analyzer, Analyzer::Starting { .. } | Analyzer::Ready { .. }) {
                    self.analyzer_request("shutdown", Json::Null, move |s, _| s.respond(&id, Json::Null));
                } else {
                    self.respond(&id, Json::Null);
                }
            }
            "exit" => {
                self.stop_analyzer();
                return Some(if self.shut_down { 0 } else { 1 });
            }
            "$/cancelRequest" => {
                let target = m["params"]["id"].clone();
                let own = self.pending.iter().find_map(|(own, p)| match p {
                    Pending::Editor { id, .. } if *id == target => Some(*own),
                    _ => None,
                });
                if let Some(own) = own {
                    self.send_analyzer(rpc::notification("$/cancelRequest", Json::object([("id", own.into())])));
                }
            }
            "textDocument/didOpen" => self.did_open(m),
            "textDocument/didChange" => self.did_change(m),
            "textDocument/didClose" => self.did_close(m),
            "textDocument/didSave" => {
                let uri = m["params"]["textDocument"]["uri"].str().to_string();
                if self.docs.get(&uri).is_some_and(|d| d.in_analyzer) {
                    self.send_analyzer(m);
                }
                // (another source of the crate saved: the open documents' Kotlin again)
                for d in self.docs.values_mut() {
                    if d.has_kotlin() {
                        d.dirty = true;
                    }
                }
                self.schedule();
            }
            "workspace/symbol" => self.workspace_symbol(m),
            _ if method.starts_with("textDocument/") || method == "completionItem/resolve" => {
                self.document_request(m, &method, is_request)
            }
            _ => {
                if is_request {
                    let uri = String::new();
                    self.forward_request(m, &uri);
                } else {
                    self.send_analyzer(m);
                }
            }
        }
        None
    }

    fn did_open(&mut self, m: Json) {
        let td = &m["params"]["textDocument"];
        let uri = td["uri"].str().to_string();
        let mut doc = Doc::new(&uri, td["text"].str().to_string(), td["version"].int());
        if doc.lang != Lang::Kotlin {
            let mut m = m.clone();
            if doc.lang == Lang::Rust {
                m.entry("params").entry("textDocument").set("languageId", "rust");
            }
            doc.in_analyzer = self.send_analyzer(m);
        }
        let kotlin = doc.has_kotlin();
        let manifest = doc.manifest.clone();
        self.docs.insert(uri.clone(), doc);
        if kotlin {
            self.crate_due(manifest.as_deref());
        }
    }

    fn did_change(&mut self, m: Json) {
        let uri = m["params"]["textDocument"]["uri"].str().to_string();
        let Some(doc) = self.docs.get_mut(&uri) else { return };
        let had = doc.has_kotlin();
        for change in m["params"]["contentChanges"].array() {
            crate::text::apply_change(&mut doc.text, change);
        }
        doc.version = m["params"]["textDocument"]["version"].as_i64().unwrap_or(doc.version + 1);
        doc.refresh();
        let (kotlin, manifest, forward) = (doc.has_kotlin(), doc.manifest.clone(), doc.in_analyzer);
        if forward {
            self.send_analyzer(m);
        }
        if had || kotlin {
            self.crate_due(manifest.as_deref());
        }
    }

    fn did_close(&mut self, m: Json) {
        let uri = m["params"]["textDocument"]["uri"].str().to_string();
        let Some(doc) = self.docs.remove(&uri) else { return };
        if doc.in_analyzer {
            self.send_analyzer(m);
        }
        if doc.has_kotlin() || doc.lang == Lang::Kotlin {
            self.to_editor(rpc::notification(
                "textDocument/publishDiagnostics",
                Json::object([("uri", uri.into()), ("diagnostics", Json::Array(Vec::new()))]),
            ));
            self.crate_due(doc.manifest.as_deref());
        }
    }

    /// The open documents with Kotlin of a crate (`manifest`'s package) due for translation.
    fn crate_due(&mut self, manifest: Option<&Path>) {
        for d in self.docs.values_mut() {
            if d.has_kotlin() && d.manifest.as_deref() == manifest {
                d.dirty = true;
            }
        }
        self.schedule();
    }

    fn schedule(&mut self) {
        if self.docs.values().any(|d| d.dirty) {
            self.due = Some(Instant::now() + self.config.delay);
        }
    }

    /// The module table a document's Kotlin is translated with: its crate's, with the open documents' Kotlin.
    fn table_for(&self, uri: &str) -> Option<Arc<kompoz_kotlin::module::ModuleTable>> {
        let doc = self.docs.get(uri)?;
        let (manifest, path) = (doc.manifest.as_deref()?, doc.path.as_deref()?);
        let table = workspace::table_for(manifest, path);
        let mut open = HashMap::new();
        for d in self.docs.values() {
            let (Some(p), Some(m)) = (d.path.as_deref(), d.manifest.as_deref()) else { continue };
            if m != manifest || d.lang == Lang::Other {
                continue;
            }
            let blocks = (0..d.regions.len()).map(|i| (d.kotlin(i).to_string(), d.regions[i].base)).collect();
            open.insert(workspace::key(p), workspace::OpenSource { kt_file: d.lang == Lang::Kotlin, blocks });
        }
        Some(Arc::new(workspace::with_open(&table, &open)))
    }

    /// Translates a document's Kotlin now (when it is due) and publishes its diagnostics.
    fn ensure_translated(&mut self, uri: &str) {
        let Some(d) = self.docs.get_mut(uri) else { return };
        if !d.dirty {
            return;
        }
        if !d.has_kotlin() && d.translated.is_empty() {
            // (a Rust document without Kotlin, then and now: nothing to translate or publish)
            d.dirty = false;
            return;
        }
        let table = self.table_for(uri);
        if let Some(d) = self.docs.get_mut(uri) {
            d.translate(table);
        }
        self.publish(uri);
    }

    fn translate_due(&mut self) {
        self.due = None;
        let uris: Vec<String> = self.docs.iter().filter(|(_, d)| d.dirty).map(|(u, _)| u.clone()).collect();
        for u in &uris {
            self.ensure_translated(u);
        }
        if let Some(mut b) = self.bridge.take() {
            b.translated(&self.docs, &self.events);
            self.bridge = Some(b);
        }
    }

    /// Publishes a document's diagnostics: rust-analyzer's off the Kotlin, the generator's, the bridge's.
    pub fn publish(&mut self, uri: &str) {
        let Some(d) = self.docs.get(uri) else { return };
        let mut all: Vec<Json> = Vec::new();
        for x in d.analyzer_diagnostics.array() {
            if !d.has_kotlin() || !d.position_in_kotlin(&x["range"]["start"]) {
                all.push(x.clone());
            }
        }
        all.extend(d.kotlin_diagnostics());
        if let Some(b) = &self.bridge {
            all.extend(b.diagnostics(uri));
        }
        let mut seen = std::collections::HashSet::new();
        all.retain(|x| seen.insert(x["range"].dump() + x["message"].str()));
        self.to_editor(rpc::notification(
            "textDocument/publishDiagnostics",
            Json::object([("uri", uri.into()), ("version", d.version.into()), ("diagnostics", Json::Array(all))]),
        ));
    }

    // ---- Requests about a document ---------------------------------------------------------------------------

    fn document_request(&mut self, m: Json, method: &str, is_request: bool) {
        let uri = m["params"]["textDocument"]["uri"].str().to_string();
        let Some(doc) = self.docs.get(&uri) else {
            // (not open: rust-analyzer's, as it is)
            if is_request {
                self.forward_request(m, &uri);
            } else {
                self.send_analyzer(m);
            }
            return;
        };
        if !is_request {
            if doc.in_analyzer {
                self.send_analyzer(m);
            }
            return;
        }
        let id = m["id"].clone();
        let (lang, kotlin) = (doc.lang, doc.has_kotlin());
        if lang == Lang::Other || method == "completionItem/resolve" {
            if method == "completionItem/resolve" && m["params"]["data"]["kompoz"].as_bool() == Some(true) {
                self.respond(&id, m["params"].clone());
                return;
            }
            self.forward_request(m, &uri);
            return;
        }
        if kotlin {
            self.ensure_translated(&uri);
        }
        let Some(doc) = self.docs.get(&uri) else { return };
        let position = &m["params"]["position"];
        let at = (!position.is_null()).then(|| doc.lines.offset_of(&doc.text, position));
        let inside = at.is_some_and(|o| doc.in_kotlin(o));
        let range_inside = {
            let r = &m["params"]["range"];
            !r.is_null() && (doc.position_in_kotlin(&r["start"]) || doc.position_in_kotlin(&r["end"]))
        };
        // (a `.kt` file: the server's throughout)
        let ours = lang == Lang::Kotlin || inside;
        match method {
            "textDocument/formatting"
            | "textDocument/rangeFormatting"
            | "textDocument/onTypeFormatting"
            | "textDocument/rangesFormatting"
                if lang == Lang::Kotlin || inside || range_inside =>
            {
                // (no Kotlin formatter)
                self.respond(&id, Json::Null);
            }
            "textDocument/rangesFormatting" if kotlin => {
                let any = m["params"]["ranges"]
                    .array()
                    .iter()
                    .any(|r| doc.position_in_kotlin(&r["start"]) || doc.position_in_kotlin(&r["end"]));
                if any {
                    self.respond(&id, Json::Null);
                } else {
                    self.forward_request(m, &uri);
                }
            }
            "textDocument/semanticTokens/full/delta" | "textDocument/semanticTokens/range" => {
                // (not offered: rust-analyzer's can't be merged with the Kotlin's)
                self.respond(&id, Json::Null);
            }
            "textDocument/hover" if ours => {
                let offset = at.unwrap_or(0);
                match kotlin::hover(doc, offset) {
                    Some(h) => self.respond(&id, h),
                    None => {
                        let reply = Reply { id: id.clone(), editor: self.editor.clone() };
                        let refused = match (self.bridge.as_mut(), self.docs.get(&uri)) {
                            (Some(b), Some(doc)) => b.hover(doc, offset, reply),
                            _ => Err(reply),
                        };
                        if let Err(reply) = refused {
                            reply.send(Json::Null);
                        }
                    }
                }
            }
            "textDocument/completion" if ours => {
                let table = self.table_for(&uri);
                let Some(doc) = self.docs.get(&uri) else { return };
                let list = kotlin::completion(doc, at.unwrap_or(0), table);
                self.respond(&id, list);
            }
            "textDocument/definition" | "textDocument/declaration" if ours => {
                let table = self.table_for(&uri);
                let rust_items = table.as_ref().map_or(&[][..], |t| &t.rust[..]);
                let docs = &self.docs;
                let Some(doc) = docs.get(&uri) else { return };
                let text_of = |p: &Path| -> Option<String> {
                    let key = workspace::key(p);
                    docs.values()
                        .find(|d| d.path.as_deref().map(workspace::key).as_ref() == Some(&key))
                        .map(|d| d.text.clone())
                        .or_else(|| std::fs::read_to_string(p).ok())
                };
                match kotlin::definition(doc, at.unwrap_or(0), rust_items, &text_of) {
                    kotlin::Definition::Location(l) => self.respond(&id, l),
                    kotlin::Definition::None => self.respond(&id, Json::Null),
                    kotlin::Definition::Api { rust, place, module, owner } => {
                        // (rust-analyzer's symbols: the workspace's, then the dependencies' too, `*`)
                        let query = Json::object([("query", rust.clone().into())]);
                        self.analyzer_request("workspace/symbol", query, move |s, result| {
                            let found = result.and_then(|r| kotlin::pick_symbol(&r, &rust, &place, &module, &owner));
                            if let Some(l) = found {
                                s.respond(&id, l);
                                return;
                            }
                            let query = Json::object([("query", format!("{rust}*").into())]);
                            s.analyzer_request("workspace/symbol", query, move |s, result| {
                                let found =
                                    result.and_then(|r| kotlin::pick_symbol(&r, &rust, &place, &module, &owner));
                                s.respond(&id, found.unwrap_or(Json::Null));
                            });
                        });
                    }
                }
            }
            "textDocument/documentHighlight" if ours => {
                let h = kotlin::highlights(doc, at.unwrap_or(0));
                self.respond(&id, h);
            }
            "textDocument/semanticTokens/full" if lang == Lang::Kotlin => {
                let data = tokens::encode(kotlin::semantic_tokens(doc));
                self.respond(&id, Json::object([("data", data)]));
            }
            "textDocument/documentSymbol" if lang == Lang::Kotlin => {
                let s = kotlin::symbols(doc, self.hierarchical_symbols());
                self.respond(&id, Json::Array(s));
            }
            _ if ours => {
                // (nothing of rust-analyzer's inside the Kotlin: its view there is the macro's expansion)
                self.respond(&id, empty_result(method));
            }
            _ if kotlin || doc.in_analyzer => self.forward_request(m, &uri),
            _ => {
                // (a Rust document rust-analyzer doesn't have: none running)
                self.forward_request(m, &uri);
            }
        }
    }

    fn hierarchical_symbols(&self) -> bool {
        self.init["capabilities"]["textDocument"]["documentSymbol"]["hierarchicalDocumentSymbolSupport"].as_bool()
            == Some(true)
    }

    /// rust-analyzer's answer about a document with Kotlin, kept off the Kotlin (and the Kotlin's own added).
    fn off_kotlin(&mut self, method: &str, uri: &str, mut result: Json) -> Json {
        if method == "workspace/symbol" {
            return result;
        }
        let doc = self.docs.get(uri).filter(|d| d.has_kotlin());
        let Some(doc) = doc else {
            if method == "textDocument/semanticTokens/full" && !result.is_null() {
                // (rust-analyzer's tokens in the server's legend)
                let data = tokens::decode(&result["data"], &self.analyzer_legend);
                return Json::object([("data", tokens::encode(data))]);
            }
            return result;
        };
        let in_kotlin = |p: &Json| doc.position_in_kotlin(p);
        match method {
            "textDocument/semanticTokens/full" => {
                let mut all: Vec<tokens::SemanticToken> = tokens::decode(&result["data"], &self.analyzer_legend)
                    .into_iter()
                    .filter(|t| {
                        let p = Json::object([("line", t.line.into()), ("character", t.start.into())]);
                        !in_kotlin(&p)
                    })
                    .collect();
                all.extend(kotlin::semantic_tokens(doc));
                Json::object([("data", tokens::encode(all))])
            }
            "textDocument/documentSymbol" => {
                let mut kept: Vec<Json> = result
                    .array()
                    .iter()
                    .filter(|s| {
                        let r = if s.has("location") { &s["location"]["range"] } else { &s["range"] };
                        !in_kotlin(&r["start"])
                    })
                    .cloned()
                    .collect();
                let flat = kept.first().is_some_and(|s| s.has("location"));
                let hierarchical = if kept.is_empty() { self.hierarchical_symbols() } else { !flat };
                kept.extend(kotlin::symbols(doc, hierarchical));
                Json::Array(kept)
            }
            "textDocument/inlayHint" | "textDocument/codeLens" | "textDocument/foldingRange" => {
                if let Some(a) = result.array_mut() {
                    a.retain(|h| {
                        let p = if h.has("position") {
                            h["position"].clone()
                        } else if h.has("range") {
                            h["range"]["start"].clone()
                        } else {
                            Json::object([("line", h["startLine"].clone()), ("character", 0.into())])
                        };
                        p.is_null() || !in_kotlin(&p) || method == "textDocument/foldingRange"
                    });
                }
                result
            }
            "textDocument/formatting" => {
                if let Some(a) = result.array_mut() {
                    a.retain(|e| !touches_kotlin(doc, &e["range"]));
                }
                result
            }
            _ => {
                // (edits anywhere in the answer — a rename's, a code action's: none inside the Kotlin)
                drop_edits(doc, &mut result);
                result
            }
        }
    }

    fn workspace_symbol(&mut self, m: Json) {
        let query = m["params"]["query"].str().to_lowercase();
        let mut ours: Vec<Json> = Vec::new();
        let uris: Vec<String> = self.docs.keys().cloned().collect();
        for u in &uris {
            self.ensure_translated(u);
        }
        for d in self.docs.values() {
            for s in kotlin::symbols(d, false) {
                if s["name"].str().to_lowercase().contains(query.trim_end_matches(['*', '#'])) {
                    ours.push(s);
                }
            }
        }
        let id = m["id"].clone();
        let params = m["params"].clone();
        self.analyzer_request("workspace/symbol", params, move |s, result| {
            let mut all: Vec<Json> = result.map(|r| r.array().to_vec()).unwrap_or_default();
            all.extend(ours);
            s.respond(&id, Json::Array(all));
        });
    }
}

/// Whether an LSP range touches a document's Kotlin.
fn touches_kotlin(doc: &Doc, range: &Json) -> bool {
    let from = doc.lines.offset_of(&doc.text, &range["start"]);
    let to = doc.lines.offset_of(&doc.text, &range["end"]);
    doc.regions.iter().any(|r| {
        let (a, b) = (doc.norm.to_original(r.start), doc.norm.to_original(r.end));
        from <= b && a <= to
    })
}

/// Drops the edits of a document's Kotlin from a workspace edit, a code action, … (wherever they are in `v`).
fn drop_edits(doc: &Doc, v: &mut Json) {
    match v {
        Json::Object(_) => {
            if let Some(changes) = v.get_mut("changes")
                && let Some(edits) = changes.get_mut(&doc.uri).and_then(Json::array_mut)
            {
                edits.retain(|e| !touches_kotlin(doc, &e["range"]));
            }
            if v["textDocument"]["uri"].str() == doc.uri
                && let Some(edits) = v.get_mut("edits").and_then(Json::array_mut)
            {
                edits.retain(|e| !touches_kotlin(doc, &e["range"]));
            }
            if let Some(m) = v.members_mut() {
                for (_, child) in m.iter_mut() {
                    drop_edits(doc, child);
                }
            }
        }
        Json::Array(a) => {
            for child in a.iter_mut() {
                drop_edits(doc, child);
            }
        }
        _ => {}
    }
}

/// What a request gets when there is nothing to say.
fn empty_result(method: &str) -> Json {
    match method {
        "textDocument/completion" => Json::object([("isIncomplete", false.into()), ("items", Json::Array(Vec::new()))]),
        "textDocument/references"
        | "textDocument/documentHighlight"
        | "textDocument/documentSymbol"
        | "textDocument/inlayHint"
        | "textDocument/codeAction"
        | "textDocument/codeLens"
        | "textDocument/foldingRange"
        | "workspace/symbol" => Json::Array(Vec::new()),
        "textDocument/semanticTokens/full" => Json::object([("data", Json::Array(Vec::new()))]),
        _ => Json::Null,
    }
}

/// The server's capabilities: rust-analyzer 1.99's (as it gives them to an editor of these capabilities), with
/// one legend for the Kotlin's semantic tokens and rust-analyzer's, whole (a delta or a range's can't be merged),
/// and the diagnostics pushed (the server merges them).
pub fn capabilities(init: &Json) -> Json {
    let td = &init["capabilities"]["textDocument"];
    let resolve = |cap: &str| !td[cap]["resolveSupport"].is_null();
    let mut caps = Json::parse(include_str!("capabilities.json")).unwrap_or_default();
    caps.entry("completionProvider")
        .set("resolveProvider", !td["completion"]["completionItem"]["resolveSupport"].is_null());
    if resolve("codeAction") || !td["codeAction"]["codeActionLiteralSupport"].is_null() {
        caps.set(
            "codeActionProvider",
            Json::object([
                (
                    "codeActionKinds",
                    Json::Array(
                        ["", "quickfix", "refactor", "refactor.extract", "refactor.inline", "refactor.rewrite"]
                            .iter()
                            .map(|k| Json::from(*k))
                            .collect(),
                    ),
                ),
                ("resolveProvider", resolve("codeAction").into()),
            ]),
        );
    }
    caps.entry("inlayHintProvider").set("resolveProvider", resolve("inlayHint"));
    caps.set(
        "semanticTokensProvider",
        Json::object([("legend", tokens::legend()), ("full", true.into()), ("range", false.into())]),
    );
    caps
}
