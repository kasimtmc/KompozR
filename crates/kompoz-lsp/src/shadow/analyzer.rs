//! The shadow's rust-analyzer: one process for every shadow project (its linked projects), started when the first
//! shadow is made, its `cargo check` (flycheck: rustc's own errors) building into the package's target directory.

use crate::json::Json;
use crate::rpc;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// What the analyzer's reader thread passes on.
pub enum Incoming {
    Message(Json),
    Closed,
}

pub struct Analyzer {
    child: Child,
    tx: Sender<Json>,
    /// Its `initialize` answered.
    pub ready: bool,
    /// What is sent once it is.
    queue: Vec<Json>,
    pub projects: Vec<PathBuf>,
}

/// rust-analyzer's settings for the shadows.
pub fn settings(projects: &[PathBuf]) -> Json {
    let linked: Vec<Json> = projects.iter().map(|p| Json::from(p.to_string_lossy().into_owned())).collect();
    let mut s = Json::parse(
        r#"{"checkOnSave": true, "check": {"command": "check", "allTargets": true},
            "cargo": {"buildScripts": {"enable": true}}, "procMacro": {"enable": true},
            "cachePriming": {"enable": false}, "files": {"watcher": "server"},
            "diagnostics": {"enable": true, "experimental": {"enable": false}},
            "lens": {"enable": false}, "inlayHints": {"typeHints": {"enable": false}}}"#,
    )
    .unwrap_or_else(|_| Json::new_object());
    s.set("linkedProjects", Json::Array(linked));
    s
}

impl Analyzer {
    /// Starts `cmd` for the shadow `projects` (manifests), cargo building into `target_dir`; its messages to `out`.
    pub fn start(
        cmd: &Path,
        projects: Vec<PathBuf>,
        target_dir: &Path,
        out: impl Fn(Incoming) -> bool + Send + 'static,
    ) -> Result<Analyzer, String> {
        let root = projects.first().and_then(|p| p.parent()).map(Path::to_path_buf).unwrap_or_default();
        let mut child = Command::new(cmd)
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", target_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{}: {e}", cmd.display()))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err("no pipes".into());
        };
        std::thread::spawn(move || {
            let mut input = BufReader::new(stdout);
            while let Ok(Some(m)) = rpc::read(&mut input) {
                if !out(Incoming::Message(m)) {
                    return;
                }
            }
            out(Incoming::Closed);
        });
        let tx = rpc::writer(stdin);
        let caps = Json::parse(
            r#"{"textDocument": {"hover": {"contentFormat": ["markdown", "plaintext"]},
                                 "publishDiagnostics": {"relatedInformation": false},
                                 "synchronization": {"didSave": true}},
                "workspace": {"configuration": true, "didChangeConfiguration": {"dynamicRegistration": false}},
                "experimental": {"serverStatusNotification": true}}"#,
        )
        .unwrap_or_else(|_| Json::new_object());
        let params = Json::object([
            ("processId", (std::process::id() as i64).into()),
            ("rootUri", crate::doc::path_to_uri(&root).into()),
            ("capabilities", caps),
            ("initializationOptions", settings(&projects)),
        ]);
        let _ = tx.send(rpc::request(0, "initialize", params));
        Ok(Analyzer { child, tx, ready: false, queue: Vec::new(), projects })
    }

    /// Its `initialize` answered: `initialized`, then what waited.
    pub fn initialized(&mut self) {
        self.ready = true;
        let _ = self.tx.send(rpc::notification("initialized", Json::new_object()));
        for m in std::mem::take(&mut self.queue) {
            let _ = self.tx.send(m);
        }
    }

    pub fn send(&mut self, m: Json) {
        if self.ready {
            let _ = self.tx.send(m);
        } else {
            self.queue.push(m);
        }
    }

    /// The linked projects changed: told (it asks for its settings again).
    pub fn set_projects(&mut self, projects: Vec<PathBuf>) {
        self.projects = projects;
        let settings = settings(&self.projects);
        self.send(rpc::notification("workspace/didChangeConfiguration", Json::object([("settings", settings)])));
    }

    /// Answers its request: its settings, else nothing.
    pub fn answer(&mut self, m: &Json) {
        let result = if m["method"].str() == "workspace/configuration" {
            Json::Array(m["params"]["items"].array().iter().map(|_| settings(&self.projects)).collect())
        } else {
            Json::Null
        };
        let _ = self.tx.send(rpc::response(m["id"].clone(), result));
    }

    /// Shuts it down (killed when it doesn't exit in a few seconds).
    pub fn stop(mut self) {
        let _ = self.tx.send(rpc::request(-1, "shutdown", Json::Null));
        let _ = self.tx.send(rpc::notification("exit", Json::Null));
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
