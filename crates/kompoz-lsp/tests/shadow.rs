//! The shadow-project bridge end to end, over stdio: a package using KompozR (a `kompoz!` block in its `lib.rs`, a
//! `.kt` file it reads with `kompoz_file!`), each with a type error the generator can't see (a `String` where an
//! `Int` is declared); rustc's errors (the shadow's `cargo check`) land on the Kotlin's lines, and a hover inside
//! the Kotlin is rust-analyzer's of the Rust made of it.
//!
//! Skipped (saying so) when `rust-analyzer --version` doesn't run, unless `KOMPOZ_RUST_ANALYZER_REQUIRED=1` (CI). The server runs without its proxied
//! rust-analyzer (`--no-rust-analyzer`): the shadow's is the only one. The shadow builds into a target directory kept
//! under the test's temporary directory (the dependencies built once), offline, two jobs.

use kompoz_lsp::json::Json;
use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const LIB: &str = r#"//! A package using KompozR's Kotlin syntax.

use kompoz_kotlin_macro::{kompoz, kompoz_file};

kompoz! {
    fun answer(): Int = "forty-two"

    fun fine(n: Int): Int = n + 1
}

kompoz_file!("src/ui.kt");
"#;

const KT: &str = "fun half(n: Int): Int {\n    return n / 2\n}\n\nfun wrong(n: Int): Int = \"no\" + n\n";

fn rust_analyzer_runs() -> bool {
    Command::new("rust-analyzer")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The fixture package, removed after.
struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Fixture {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize().unwrap();
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("shadow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let manifest = format!(
            "[package]\nname = \"shadow_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n\
             kompoz-runtime = {{ path = {:?} }}\nkompoz-kotlin-macro = {{ path = {:?} }}\n\n[workspace]\n",
            crates.join("kompoz-runtime"),
            crates.join("kompoz-kotlin-macro"),
        );
        std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
        // (the workspace's lock: the same versions, offline)
        let _ = std::fs::copy(crates.join("../Cargo.lock"), dir.join("Cargo.lock"));
        std::fs::write(dir.join("src/lib.rs"), LIB).unwrap();
        std::fs::write(dir.join("src/ui.kt"), KT).unwrap();
        Fixture(dir.canonicalize().unwrap())
    }

    fn uri(&self, rel: &str) -> String {
        kompoz_lsp::doc::path_to_uri(&self.0.join(rel))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Client {
    child: Child,
    stdin: ChildStdin,
    inbox: Receiver<Json>,
    next: i64,
    diagnostics: HashMap<String, Json>,
}

impl Client {
    fn start(root: &Path, target: &Path) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_kompoz-lsp"))
            .arg("--no-rust-analyzer")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", target)
            .env("CARGO_BUILD_JOBS", "2")
            .env("CARGO_NET_OFFLINE", "true")
            .env("CARGO_INCREMENTAL", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("kompoz-lsp runs");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, inbox) = mpsc::channel();
        std::thread::spawn(move || {
            let mut input = BufReader::new(stdout);
            while let Ok(Some(m)) = kompoz_lsp::rpc::read(&mut input) {
                if tx.send(m).is_err() {
                    return;
                }
            }
        });
        Client { child, stdin, inbox, next: 1, diagnostics: HashMap::new() }
    }

    fn send(&mut self, m: Json) {
        self.stdin.write_all(&kompoz_lsp::rpc::frame(&m)).unwrap();
        self.stdin.flush().unwrap();
    }

    fn take(&mut self, m: Json) -> Option<Json> {
        if m["method"].str() == "textDocument/publishDiagnostics" {
            self.diagnostics.insert(m["params"]["uri"].str().to_string(), m["params"]["diagnostics"].clone());
            return None;
        }
        if m.has("method") && m.has("id") {
            self.send(kompoz_lsp::rpc::response(m["id"].clone(), Json::Null));
            return None;
        }
        Some(m)
    }

    fn request(&mut self, method: &str, params: Json) -> Json {
        self.next += 1;
        let id = self.next;
        self.send(kompoz_lsp::rpc::request(id, method, params));
        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline {
            let Ok(m) = self.inbox.recv_timeout(Duration::from_secs(1)) else { continue };
            if let Some(m) = self.take(m)
                && m["id"].as_i64() == Some(id)
            {
                return m["result"].clone();
            }
        }
        panic!("no answer to {method}");
    }

    /// Waits until the document's diagnostics satisfy `ok` (its latest ones on timeout, for the message).
    fn wait_for(&mut self, uri: &str, timeout: u64, ok: impl Fn(&[Json]) -> bool) -> Result<Vec<Json>, Json> {
        let deadline = Instant::now() + Duration::from_secs(timeout);
        loop {
            if let Some(d) = self.diagnostics.get(uri)
                && ok(d.array())
            {
                return Ok(d.array().to_vec());
            }
            if Instant::now() > deadline {
                return Err(self.diagnostics.get(uri).cloned().unwrap_or(Json::Null));
            }
            if let Ok(m) = self.inbox.recv_timeout(Duration::from_secs(1)) {
                self.take(m);
            }
        }
    }

    fn stop(mut self) {
        let _ = self.request("shutdown", Json::Null);
        self.send(kompoz_lsp::rpc::notification("exit", Json::Null));
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        panic!("the server didn't exit");
    }
}

/// A rustc error on the 0-based line.
fn rustc_on(line: usize) -> impl Fn(&[Json]) -> bool {
    move |d| d.iter().any(|x| x["source"].str() == "rustc" && x["range"]["start"]["line"].int() == line as i64)
}

fn line_col(text: &str, needle: &str, plus: usize) -> Json {
    let at = text.find(needle).unwrap() + plus;
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    Json::object([("line", line.into()), ("character", col.into())])
}

#[test]
fn rustc_errors_on_the_kotlins_lines() {
    if !rust_analyzer_runs() {
        assert!(
            std::env::var("KOMPOZ_RUST_ANALYZER_REQUIRED").as_deref() != Ok("1"),
            "no rust-analyzer (rust-analyzer --version doesn't run) and KOMPOZ_RUST_ANALYZER_REQUIRED=1"
        );
        println!("skipped: no rust-analyzer (rust-analyzer --version doesn't run)");
        return;
    }
    let f = Fixture::new();
    let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shadow-target");
    let mut client = Client::start(&f.0, &target);
    let init = client.request(
        "initialize",
        Json::object([("processId", Json::Null), ("rootUri", f.uri("").into()), ("capabilities", Json::new_object())]),
    );
    assert!(init["capabilities"].is_object());
    client.send(kompoz_lsp::rpc::notification("initialized", Json::new_object()));
    let (rs, kt) = (f.uri("src/lib.rs"), f.uri("src/ui.kt"));
    for (uri, text, language) in [(&rs, LIB, "rust"), (&kt, KT, "kotlin")] {
        let doc = Json::object([
            ("uri", uri.clone().into()),
            ("languageId", language.into()),
            ("version", 1.into()),
            ("text", text.into()),
        ]);
        client.send(kompoz_lsp::rpc::notification("textDocument/didOpen", Json::object([("textDocument", doc)])));
    }
    // (the first run builds the dependencies: minutes)
    let answer_line = LIB.lines().position(|l| l.contains("forty-two")).unwrap();
    let in_rs = client.wait_for(&rs, 900, rustc_on(answer_line));
    let wrong_line = KT.lines().position(|l| l.contains("fun wrong")).unwrap();
    let in_kt = client.wait_for(&kt, 300, rustc_on(wrong_line));
    // (the rest of the Kotlin is clean: no rustc error on `fine`'s or `half`'s lines)
    let fine_line = LIB.lines().position(|l| l.contains("fun fine")).unwrap();
    let rs_clean = client.diagnostics.get(&rs).is_some_and(|d| !rustc_on(fine_line)(d.array()));
    let kt_clean = client.diagnostics.get(&kt).is_some_and(|d| !rustc_on(1)(d.array()));
    // Hovers inside the Kotlin the generator has none of: rust-analyzer's, of the Rust made of the Kotlin there —
    // `Int` is `i32`; a string literal (no hover of its own in rust-analyzer) the conversion made of it (asked again
    // while rust-analyzer still loads).
    let hover_at = |client: &mut Client, needle: &str, plus: usize| {
        let params = Json::object([
            ("textDocument", Json::object([("uri", rs.clone().into())])),
            ("position", line_col(LIB, needle, plus)),
        ]);
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut hover = client.request("textDocument/hover", params.clone());
        while hover.is_null() && Instant::now() < deadline {
            let _ = client.wait_for("none", 2, |_| false);
            hover = client.request("textDocument/hover", params.clone());
        }
        hover
    };
    let int = hover_at(&mut client, "Int = \"forty", 1);
    let literal = hover_at(&mut client, "\"forty-two\"", 2);
    client.stop();
    let in_rs =
        in_rs.unwrap_or_else(|d| panic!("no rustc error on the kompoz! block's line {answer_line}: {}", d.dump()));
    let e = in_rs.iter().find(|x| x["source"].str() == "rustc").unwrap();
    assert!(e["message"].str().contains("mismatched types"), "{}", e.dump());
    // (the error on the literal itself)
    assert_eq!(e["range"]["start"], line_col(LIB, "\"forty-two\"", 0), "{}", e.dump());
    let in_kt = in_kt.unwrap_or_else(|d| panic!("no rustc error on the .kt file's line {wrong_line}: {}", d.dump()));
    assert!(in_kt.iter().any(|x| x["source"].str() == "rustc"));
    assert!(rs_clean && kt_clean, "rustc errors elsewhere in the Kotlin");
    assert!(int["contents"].dump().contains("i32"), "rust-analyzer's hover of `Int`: {}", int.dump());
    // (its range the Kotlin token's)
    assert_eq!(int["range"]["start"], line_col(LIB, "Int = \"forty", 0), "{}", int.dump());
    assert!(literal["contents"].dump().contains("String"), "rust-analyzer's hover of the literal: {}", literal.dump());
}
