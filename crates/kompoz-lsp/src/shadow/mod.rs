//! The shadow-project bridge (the design's U1, second half; what Pas's language server does for its own syntax):
//! rustc's errors in the Rust the Kotlin becomes, on the Kotlin's lines, and rust-analyzer's hover of that Rust
//! inside the Kotlin.
//!
//! Each package with Kotlin the editor has open (one using KompozR: depending on `kompoz` or `kompoz-kotlin-macro`)
//! gets a shadow project ([`project`]): a copy of it whose `kompoz!` blocks and `kompoz_file!` calls are the
//! generator's Rust, printed with a source map from each Rust token back to its Kotlin place ([`map`]). One
//! rust-analyzer ([`analyzer`]), started with the first shadow, has every shadow as a linked project: the shadows'
//! sources with Kotlin are open in it (their latest text), and a `didSave` a moment after the shadow changes runs
//! its `cargo check`. Its errors in the generated Rust go back to the Kotlin (a block the generator failed on keeps
//! the generator's own diagnostics alone); a hover in the Kotlin the server has no answer of goes to the Rust made
//! of the token there.
//!
//! The work runs on a thread of its own; the server is told of new diagnostics with [`Event::Call`].

pub mod analyzer;
pub mod map;
pub mod project;

use crate::bridge::{Bridge, Reply};
use crate::doc::{Doc, Lang};
use crate::json::Json;
use crate::rpc;
use crate::server::Event;
use crate::workspace;
use analyzer::{Analyzer, Incoming};
use project::{Mirror, Package, ShadowFile};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long after the shadow last changed its `cargo check` runs.
const CHECK_DELAY: Duration = Duration::from_millis(800);

/// An open document, as the shadow reads it.
struct Snapshot {
    path: PathBuf,
    manifest: PathBuf,
    text: String,
}

enum Msg {
    Update(Vec<Snapshot>, Sender<Event>),
    Hover { path: PathBuf, offset: usize, reply: Reply },
    Analyzer(Incoming),
    Shutdown,
}

/// The bridge (see the module's doc).
pub struct ShadowBridge {
    analyzer: PathBuf,
    worker: Option<(Sender<Msg>, std::thread::JoinHandle<()>)>,
    diagnostics: Arc<Mutex<HashMap<String, Vec<Json>>>>,
    /// The documents (URI, version) last sent.
    sent: Vec<(String, i64)>,
}

impl ShadowBridge {
    /// A bridge running `analyzer` (rust-analyzer's command) when a shadow is first made.
    pub fn new(analyzer: PathBuf) -> ShadowBridge {
        ShadowBridge { analyzer, worker: None, diagnostics: Default::default(), sent: Vec::new() }
    }

    fn worker(&mut self) -> &Sender<Msg> {
        if self.worker.is_none() {
            let (tx, rx) = mpsc::channel();
            let (command, me, shared) = (self.analyzer.clone(), tx.clone(), self.diagnostics.clone());
            // (made on its thread: the packages' paths are tokens, not Send)
            let handle = std::thread::Builder::new()
                .name("kompoz-shadow".into())
                .spawn(move || Worker::new(command, me, shared).run(rx))
                .expect("a thread for the shadow");
            self.worker = Some((tx, handle));
        }
        &self.worker.as_ref().expect("started").0
    }
}

impl Bridge for ShadowBridge {
    fn translated(&mut self, docs: &HashMap<String, Doc>, events: &Sender<Event>) {
        let mut sent: Vec<(String, i64)> = docs.values().map(|d| (d.uri.clone(), d.version)).collect();
        sent.sort();
        if sent == self.sent {
            return;
        }
        self.sent = sent;
        let snaps: Vec<Snapshot> = docs
            .values()
            .filter(|d| d.lang != Lang::Other)
            .filter_map(|d| {
                Some(Snapshot {
                    path: workspace::key(d.path.as_deref()?),
                    manifest: d.manifest.clone()?,
                    text: d.text.clone(),
                })
            })
            .collect();
        if snaps.is_empty() && self.worker.is_none() {
            return;
        }
        let _ = self.worker().send(Msg::Update(snaps, events.clone()));
    }

    fn diagnostics(&self, uri: &str) -> Vec<Json> {
        self.diagnostics.lock().map(|d| d.get(uri).cloned().unwrap_or_default()).unwrap_or_default()
    }

    fn hover(&mut self, doc: &Doc, offset: usize, reply: Reply) -> Result<(), Reply> {
        let (Some(path), Some((tx, _))) = (doc.path.as_deref(), self.worker.as_ref()) else { return Err(reply) };
        let msg = Msg::Hover { path: workspace::key(path), offset: doc.norm.to_normalized(offset), reply };
        tx.send(msg).map_err(|e| match e.0 {
            Msg::Hover { reply, .. } => reply,
            _ => unreachable!(),
        })
    }

    fn shutdown(&mut self) {
        if let Some((tx, handle)) = self.worker.take() {
            let _ = tx.send(Msg::Shutdown);
            let _ = handle.join();
        }
    }
}

impl Drop for ShadowBridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// What a request to the analyzer was for.
enum Pending {
    /// `rest`: the other Rust tokens made of the Kotlin one, asked in turn while the answers are empty.
    Hover { file: Arc<ShadowFile>, origin: PathBuf, reply: Reply, rest: Vec<usize> },
}

struct Worker {
    command: PathBuf,
    me: Sender<Msg>,
    shared: Arc<Mutex<HashMap<String, Vec<Json>>>>,
    events: Option<Sender<Event>>,
    /// The packages by directory (none: not one using KompozR, or unreadable).
    packages: HashMap<PathBuf, Option<Mirror>>,
    /// The shadow sources with Kotlin, by URI; the version each was last given the analyzer with.
    files: HashMap<String, (Arc<ShadowFile>, i64)>,
    /// The analyzer's diagnostics, by shadow URI.
    raw: HashMap<String, Vec<Json>>,
    analyzer: Option<Analyzer>,
    /// The analyzer couldn't start or exited: not started again.
    analyzer_gone: bool,
    next_id: i64,
    pending: HashMap<i64, Pending>,
    /// When the shadow's `cargo check` runs (a `didSave`).
    check_due: Option<Instant>,
    /// The analyzer has loaded the shadows (quiescent once): a check before that waits for it.
    loaded: bool,
    check_waiting: bool,
}

impl Worker {
    fn new(command: PathBuf, me: Sender<Msg>, shared: Arc<Mutex<HashMap<String, Vec<Json>>>>) -> Worker {
        Worker {
            command,
            me,
            shared,
            events: None,
            packages: HashMap::new(),
            files: HashMap::new(),
            raw: HashMap::new(),
            analyzer: None,
            analyzer_gone: false,
            next_id: 1,
            pending: HashMap::new(),
            check_due: None,
            loaded: false,
            check_waiting: false,
        }
    }

    fn run(&mut self, rx: Receiver<Msg>) {
        loop {
            let msg = match self.check_due {
                Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                    Ok(m) => m,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        self.check();
                        continue;
                    }
                    Err(_) => break,
                },
                None => match rx.recv() {
                    Ok(m) => m,
                    Err(_) => break,
                },
            };
            match msg {
                Msg::Update(snaps, events) => {
                    self.events = Some(events);
                    self.update(snaps);
                }
                Msg::Hover { path, offset, reply } => self.hover(path, offset, reply),
                Msg::Analyzer(Incoming::Message(m)) => self.on_analyzer(m),
                Msg::Analyzer(Incoming::Closed) => {
                    if self.analyzer.take().is_some() {
                        eprintln!("kompoz-lsp: the shadow's rust-analyzer exited");
                    }
                    self.analyzer_gone = true;
                    for (_, p) in self.pending.drain() {
                        let Pending::Hover { reply, .. } = p;
                        reply.send(Json::Null);
                    }
                }
                Msg::Shutdown => break,
            }
        }
        if let Some(a) = self.analyzer.take() {
            a.stop();
        }
    }

    fn update(&mut self, snaps: Vec<Snapshot>) {
        let mut by_package: HashMap<PathBuf, HashMap<PathBuf, String>> = HashMap::new();
        for s in snaps {
            by_package.entry(workspace::key(&s.manifest)).or_default().insert(s.path, s.text);
        }
        let mut current: HashMap<String, Arc<ShadowFile>> = HashMap::new();
        let mut written = false;
        for (dir, open) in &by_package {
            // (a package with Kotlin open only)
            let any_kotlin = open.iter().any(|(p, t)| {
                p.extension().is_some_and(|e| e == "kt") || {
                    let k = kompoz_kotlin::module::kotlin_in(&crate::text::Normalized::new(t).text);
                    !k.blocks.is_empty() || !k.kt_files.is_empty()
                }
            });
            if !any_kotlin && !self.packages.contains_key(dir) {
                continue;
            }
            let mirror = self.packages.entry(dir.clone()).or_insert_with(|| match Package::load(dir) {
                Ok(p) if p.uses_kompoz => Some(Mirror::new(p)),
                Ok(_) => None,
                Err(e) => {
                    eprintln!("kompoz-lsp: no shadow of {}: {e}", dir.display());
                    None
                }
            });
            let Some(mirror) = mirror else { continue };
            let up = mirror.update(open);
            written |= !up.written.is_empty();
            for f in up.files {
                current.insert(f.uri.clone(), f);
            }
        }
        // (the shadow files of packages not updated now: kept as they were)
        let updated: Vec<PathBuf> =
            by_package.keys().filter_map(|d| self.packages.get(d)?.as_ref().map(|m| m.pkg.shadow.clone())).collect();
        for (uri, (f, _)) in &self.files {
            if !current.contains_key(uri) && !updated.iter().any(|s| f.path.starts_with(s)) {
                current.insert(uri.clone(), f.clone());
            }
        }
        if current.is_empty() && self.analyzer.is_none() {
            return;
        }
        self.ensure_analyzer();
        // (the analyzer's documents: the new ones opened, the changed ones' text, the gone ones closed)
        let gone: Vec<String> = self.files.keys().filter(|u| !current.contains_key(*u)).cloned().collect();
        for uri in gone {
            self.files.remove(&uri);
            self.raw.remove(&uri);
            self.send(rpc::notification(
                "textDocument/didClose",
                Json::object([("textDocument", Json::object([("uri", uri.into())]))]),
            ));
        }
        for (uri, f) in current {
            let message = match self.files.get(&uri) {
                Some((old, _)) if old.text == f.text => {
                    // (the same text: its new map, the analyzer's diagnostics still of it)
                    let v = self.files[&uri].1;
                    self.files.insert(uri, (f, v));
                    continue;
                }
                Some((_, v)) => {
                    let v = v + 1;
                    let m = rpc::notification(
                        "textDocument/didChange",
                        Json::object([
                            ("textDocument", Json::object([("uri", uri.clone().into()), ("version", v.into())])),
                            ("contentChanges", Json::Array(vec![Json::object([("text", f.text.clone().into())])])),
                        ]),
                    );
                    self.files.insert(uri, (f, v));
                    m
                }
                None => {
                    let m = rpc::notification(
                        "textDocument/didOpen",
                        Json::object([(
                            "textDocument",
                            Json::object([
                                ("uri", uri.clone().into()),
                                ("languageId", "rust".into()),
                                ("version", 1.into()),
                                ("text", f.text.clone().into()),
                            ]),
                        )]),
                    );
                    self.files.insert(uri, (f, 1));
                    m
                }
            };
            self.send(message);
        }
        if written {
            self.check_due = Some(Instant::now() + CHECK_DELAY);
        }
        self.republish();
    }

    fn send(&mut self, m: Json) {
        if let Some(a) = self.analyzer.as_mut() {
            a.send(m);
        }
    }

    fn projects(&self) -> Vec<PathBuf> {
        let mut p: Vec<PathBuf> = self.packages.values().flatten().map(|m| m.pkg.manifest_path()).collect();
        p.sort();
        p
    }

    fn ensure_analyzer(&mut self) {
        let projects = self.projects();
        if projects.is_empty() || self.analyzer_gone {
            return;
        }
        if let Some(a) = self.analyzer.as_mut() {
            if a.projects != projects {
                a.set_projects(projects);
                self.loaded = false;
            }
            return;
        }
        let target = self.packages.values().flatten().map(|m| m.pkg.target_dir.clone()).next().unwrap_or_default();
        let me = self.me.clone();
        match Analyzer::start(&self.command, projects, &target, move |m| me.send(Msg::Analyzer(m)).is_ok()) {
            Ok(a) => self.analyzer = Some(a),
            Err(e) => {
                eprintln!("kompoz-lsp: no rust-analyzer for the shadow ({e}): no rustc errors on the Kotlin's lines");
                self.analyzer_gone = true;
            }
        }
    }

    /// The shadow's `cargo check`: a `didSave` of one of its sources.
    fn check(&mut self) {
        self.check_due = None;
        if !self.loaded {
            self.check_waiting = true;
            return;
        }
        let Some(uri) = self.files.keys().min().cloned() else { return };
        self.send(rpc::notification(
            "textDocument/didSave",
            Json::object([("textDocument", Json::object([("uri", uri.into())]))]),
        ));
    }

    fn on_analyzer(&mut self, m: Json) {
        let Some(a) = self.analyzer.as_mut() else { return };
        let method = m["method"].str().to_string();
        if method.is_empty() {
            let Some(id) = m["id"].as_i64() else { return };
            if id == 0 && !a.ready {
                if m.has("error") {
                    eprintln!("kompoz-lsp: the shadow's rust-analyzer refused to initialize");
                }
                a.initialized();
                return;
            }
            match self.pending.remove(&id) {
                Some(Pending::Hover { file, origin, reply, rest }) => {
                    let mut result = m["result"].clone();
                    if !result.is_object() {
                        self.ask_hover(file, origin, reply, rest);
                        return;
                    }
                    match result.get("range").and_then(|r| file.to_origin(r)) {
                        Some((o, r)) if o.path == origin => result.set("range", r),
                        _ => {
                            result.remove("range");
                        }
                    }
                    reply.send(result);
                }
                None => {}
            }
            return;
        }
        if m.has("id") {
            a.answer(&m);
            return;
        }
        if method == "experimental/serverStatus" {
            // (loaded: the check that waited for it)
            if m["params"]["quiescent"].as_bool() == Some(true) && !self.loaded {
                self.loaded = true;
                if self.check_waiting {
                    self.check_waiting = false;
                    self.check_due = Some(Instant::now());
                }
            }
            return;
        }
        if method == "textDocument/publishDiagnostics" {
            let uri = m["params"]["uri"].str().to_string();
            if self.files.contains_key(&uri) {
                self.raw.insert(uri, m["params"]["diagnostics"].array().to_vec());
                self.republish();
            }
        }
    }

    /// The analyzer's errors in the generated Rust, by Kotlin document; the server told of the ones that changed.
    fn republish(&mut self) {
        let mut by_origin: HashMap<String, Vec<Json>> = HashMap::new();
        for (f, _) in self.files.values() {
            for o in &f.origins {
                by_origin.entry(o.uri.clone()).or_default();
            }
        }
        for (uri, diags) in &self.raw {
            let Some((f, _)) = self.files.get(uri) else { continue };
            for d in diags {
                if d["severity"].as_i64().unwrap_or(1) != 1 {
                    continue;
                }
                let code = d["code"].as_str().unwrap_or("");
                let rustc = d["source"].str() == "rustc"
                    || (code.len() == 5 && code.starts_with('E') && code[1..].bytes().all(|b| b.is_ascii_digit()));
                if !rustc {
                    continue;
                }
                let Some((origin, range)) = f.to_origin(&d["range"]) else { continue };
                let mut x = Json::object([
                    ("range", range),
                    ("severity", 1.into()),
                    ("source", "rustc".into()),
                    ("message", d["message"].clone()),
                ]);
                if !code.is_empty() {
                    x.set("code", code);
                }
                let list = by_origin.entry(origin.uri.clone()).or_default();
                if !list.iter().any(|y| y["range"] == x["range"] && y["message"] == x["message"]) {
                    list.push(x);
                }
            }
        }
        let changed: Vec<String> = {
            let Ok(mut shared) = self.shared.lock() else { return };
            let mut changed = Vec::new();
            for (uri, list) in &by_origin {
                if shared.get(uri).map_or(!list.is_empty(), |old| old != list) {
                    changed.push(uri.clone());
                }
            }
            for uri in shared.keys() {
                if !by_origin.contains_key(uri) {
                    changed.push(uri.clone());
                }
            }
            *shared = by_origin.into_iter().filter(|(_, l)| !l.is_empty()).collect();
            changed
        };
        if let Some(events) = &self.events {
            for uri in changed {
                let _ = events.send(Event::Call(Box::new(move |s| s.publish(&uri))));
            }
        }
    }

    fn hover(&mut self, path: PathBuf, offset: usize, reply: Reply) {
        let found = self.files.values().find_map(|(f, _)| {
            let i = f.origins.iter().position(|o| workspace::key(&o.path) == path)?;
            let mut at = f.map.shadow_candidates(i, offset, &f.origins[i].norm.text);
            // (a few: a literal has no hover of its own, the conversion made of it has)
            at.truncate(4);
            at.reverse();
            (!at.is_empty()).then(|| (f.clone(), at))
        });
        match (found, self.analyzer.is_some()) {
            (Some((file, rest)), true) => self.ask_hover(file, path, reply, rest),
            _ => reply.send(Json::Null),
        }
    }

    /// Asks the analyzer for the hover at the last of `rest` (the others asked after it, while it has none).
    fn ask_hover(&mut self, file: Arc<ShadowFile>, origin: PathBuf, reply: Reply, mut rest: Vec<usize>) {
        let Some(at) = rest.pop() else {
            reply.send(Json::Null);
            return;
        };
        let id = self.next_id;
        self.next_id += 1;
        let position = file.lines.json(&file.text, at);
        self.send(rpc::request(
            id,
            "textDocument/hover",
            Json::object([("textDocument", Json::object([("uri", file.uri.clone().into())])), ("position", position)]),
        ));
        self.pending.insert(id, Pending::Hover { file, origin, reply, rest });
    }
}
