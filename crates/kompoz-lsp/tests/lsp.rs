//! The language server driven as an editor drives it, over stdio (the C++ Kompoz's tests/kompozc_lsp_test.py,
//! ported): a crate of its own (written under the target's temporary directory: a `kompoz!` block in its `lib.rs`, a
//! `.kt` file it reads, its own `[workspace]`, and stand-ins for the macros so rust-analyzer needs no dependency).
//!
//! The Kotlin's answers are checked with and without rust-analyzer (`--no-rust-analyzer`: the server's own
//! answers alone). With the toolchain's rust-analyzer (skipped, saying so, when `rust-analyzer --version` doesn't
//! run, unless `KOMPOZ_RUST_ANALYZER_REQUIRED=1`: CI): the file clean as it is, an error in the Rust around the Kotlin where it is, rust-analyzer's hover in the
//! Rust and none of it inside the Kotlin, its semantic tokens merged with the Kotlin's, rustfmt's edits kept off the
//! Kotlin, a Kotlin call of the crate's Rust function going to it through rust-analyzer's symbols. One server at a
//! time (rust-analyzer is big).

use kompoz_lsp::json::Json;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const MANIFEST: &str = "[package]\nname = \"lsp_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n";

const LIB: &str = r##"//! A crate for the language server's test.

macro_rules! kompoz {
    ($($t:tt)*) => {};
}
macro_rules! kompoz_file {
    ($($t:tt)*) => {};
}

/// Adds one.
pub fn helper(n: i32) -> i32 {
    n + 1
}

kompoz! {
    data class Sticker(val label: String, val count: Int = 0)

    val StickerGreeting = "selam " + Shout

    @Kompozable
    fun StickerRow(sticker: Sticker, prefix: String = "#") {
        Column {
            Text("$prefix ${sticker.label}")
            Text(StickerGreeting + helper(sticker.count))
        }
    }
}

kompoz_file!("src/ui.kt");

pub fn twice(n: i32) -> i32 {
    helper(helper( n ))
}
"##;

const KT: &str = "val Shout = \"!\"\n\n@Composable\nfun Greeting(name: String) {\n    val shown = name + Shout\n    Text(shown)\n}\n";

/// The fixture crate, removed after.
struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Fixture {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("lsp-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), MANIFEST).unwrap();
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

/// The server, and what it said.
struct Client {
    child: Child,
    stdin: ChildStdin,
    inbox: Receiver<Json>,
    next: i64,
    /// The latest diagnostics, by URI.
    diagnostics: std::collections::HashMap<String, Json>,
    /// The checks that failed.
    failures: Vec<String>,
}

impl Client {
    fn start(args: &[&str], root: &Path) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_kompoz-lsp"))
            .args(args)
            .current_dir(root)
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
        Client { child, stdin, inbox, next: 100, diagnostics: Default::default(), failures: Vec::new() }
    }

    fn send(&mut self, m: Json) {
        self.stdin.write_all(&kompoz_lsp::rpc::frame(&m)).unwrap();
        self.stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str, params: Json) {
        self.send(kompoz_lsp::rpc::notification(method, params));
    }

    /// A message from the server: diagnostics kept, its requests answered (with nothing).
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
            let Ok(m) = self.inbox.recv_timeout(Duration::from_millis(500)) else { continue };
            if let Some(m) = self.take(m)
                && m["id"].as_i64() == Some(id)
            {
                return m["result"].clone();
            }
        }
        panic!("no answer to {method}");
    }

    fn check(&mut self, what: &str, ok: bool) {
        println!("{} {what}", if ok { "[ OK ]" } else { "[FAIL]" });
        if !ok {
            self.failures.push(what.to_string());
        }
    }

    /// Waits until `uri`'s latest diagnostics satisfy `condition` (and stay so for a moment).
    fn wait_for(&mut self, what: &str, uri: &str, timeout: u64, condition: impl Fn(&[Json]) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(timeout);
        let mut since: Option<Instant> = None;
        while Instant::now() < deadline {
            if let Ok(m) = self.inbox.recv_timeout(Duration::from_millis(200)) {
                self.take(m);
            }
            let ok = self.diagnostics.get(uri).is_some_and(|d| condition(d.array()));
            if ok {
                let at = *since.get_or_insert_with(Instant::now);
                if at.elapsed() > Duration::from_millis(1500) {
                    self.check(what, true);
                    return;
                }
            } else {
                since = None;
            }
        }
        println!("  diagnostics: {}", self.diagnostics.get(uri).map(|d| d.dump()).unwrap_or_default());
        self.check(what, false);
    }

    fn stop(mut self) -> Vec<String> {
        self.request("shutdown", Json::Null);
        self.notify("exit", Json::Null);
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut code = None;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                code = status.code();
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if code.is_none() {
            let _ = self.child.kill();
        }
        self.check("exits after exit, with 0", code == Some(0));
        std::mem::take(&mut self.failures)
    }
}

/// An open document's text, its lines, and the changes made to it (as editors send them: incremental).
struct Text {
    uri: String,
    lines: Vec<String>,
    version: i64,
}

impl Text {
    fn line_of(&self, needle: &str) -> usize {
        self.lines.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no {needle}"))
    }

    /// The LSP position of the `n`th character of `needle`'s first occurrence (on its line).
    fn at(&self, needle: &str, n: usize) -> Json {
        let line = self.line_of(needle);
        let col = self.lines[line].find(needle).unwrap() + n;
        let character = self.lines[line][..col].encode_utf16().count();
        Json::object([("line", line.into()), ("character", character.into())])
    }

    fn doc(&self) -> Json {
        Json::object([("uri", self.uri.clone().into())])
    }

    fn params(&self, position: Json) -> Json {
        Json::object([("textDocument", self.doc()), ("position", position)])
    }

    fn replace(&mut self, client: &mut Client, needle: &str, new: &str) {
        let line = self.line_of(needle);
        let col = self.lines[line].find(needle).unwrap();
        let start = self.lines[line][..col].encode_utf16().count();
        let end = start + needle.encode_utf16().count();
        self.version += 1;
        let range = Json::object([
            ("start", Json::object([("line", line.into()), ("character", start.into())])),
            ("end", Json::object([("line", line.into()), ("character", end.into())])),
        ]);
        client.notify(
            "textDocument/didChange",
            Json::object([
                ("textDocument", Json::object([("uri", self.uri.clone().into()), ("version", self.version.into())])),
                ("contentChanges", Json::Array(vec![Json::object([("range", range), ("text", new.into())])])),
            ]),
        );
        self.lines[line] = self.lines[line].replacen(needle, new, 1);
    }
}

fn open(client: &mut Client, uri: &str, text: &str, language: &str) -> Text {
    client.notify(
        "textDocument/didOpen",
        Json::object([(
            "textDocument",
            Json::object([
                ("uri", uri.into()),
                ("languageId", language.into()),
                ("version", 1.into()),
                ("text", text.into()),
            ]),
        )]),
    );
    Text { uri: uri.to_string(), lines: text.split('\n').map(str::to_string).collect(), version: 1 }
}

fn initialize(client: &mut Client, root: &Path, options: Json) -> Json {
    let caps = Json::parse(
        r#"{"textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true},
            "hover": {"contentFormat": ["markdown", "plaintext"]},
            "semanticTokens": {"requests": {"full": true}, "tokenTypes": [], "tokenModifiers": [], "formats": ["relative"]}}}"#,
    )
    .unwrap();
    let r = client.request(
        "initialize",
        Json::object([
            ("processId", Json::Null),
            ("rootUri", kompoz_lsp::doc::path_to_uri(root).into()),
            ("capabilities", caps),
            ("initializationOptions", options),
        ]),
    );
    client.notify("initialized", Json::new_object());
    r
}

/// The semantic tokens of a document: (1-based line, text) → (type, modifiers).
fn tokens(
    client: &mut Client,
    text: &Text,
    legend: &Json,
) -> std::collections::HashMap<(usize, String), (String, Vec<String>)> {
    let data = client.request("textDocument/semanticTokens/full", Json::object([("textDocument", text.doc())]));
    let data = data["data"].array();
    let mut out = std::collections::HashMap::new();
    let (mut line, mut start) = (0usize, 0usize);
    for c in data.as_chunks::<5>().0 {
        let dl = c[0].int() as usize;
        line += dl;
        start = if dl > 0 { c[1].int() as usize } else { start + c[1].int() as usize };
        let units: Vec<u16> = text.lines[line].encode_utf16().collect();
        let word = String::from_utf16_lossy(&units[start..(start + c[2].int() as usize).min(units.len())]);
        let ty = legend["tokenTypes"][c[3].int() as usize].str().to_string();
        let mods: Vec<String> = (0..32)
            .filter(|b| c[4].int() >> b & 1 == 1)
            .map(|b| legend["tokenModifiers"][b].str().to_string())
            .collect();
        out.insert((line + 1, word), (ty, mods));
    }
    out
}

fn on_line(line: usize, source: Option<&str>, message: Option<&str>) -> impl Fn(&[Json]) -> bool {
    let (source, message) = (source.map(str::to_string), message.map(str::to_string));
    move |d: &[Json]| {
        d.iter().any(|x| {
            x["range"]["start"]["line"].int() as usize == line
                && source.as_deref().is_none_or(|s| x["source"].str() == s)
                && message.as_deref().is_none_or(|m| x["message"].str().contains(m))
        })
    }
}

/// What both runs check of the Kotlin.
fn kotlin_checks(client: &mut Client, f: &Fixture, legend: &Json, with_analyzer: bool) {
    let mut rs = open(client, &f.uri("src/lib.rs"), LIB, "rust");
    let kt = open(client, &f.uri("src/ui.kt"), KT, "kotlin");
    let clean = |d: &[Json]| !d.iter().any(|x| x["source"].str() == "kompoz");
    client.wait_for("the Kotlin as it is: no kompoz diagnostics", &rs.uri.clone(), 120, clean);
    client.wait_for("the .kt file as it is: none either", &kt.uri.clone(), 30, |d| d.is_empty());

    // Semantic tokens: the Kotlin's own (keywords, declarations, calls, strings, annotations).
    let seen = tokens(client, &rs, legend);
    let fun_line = rs.line_of("fun StickerRow(") + 1;
    let text_line = rs.line_of("Text(\"$prefix") + 1;
    let greeting_line = rs.line_of("val StickerGreeting") + 1;
    for ((line, word), (kind, declaration)) in [
        ((fun_line - 1, "@Kompozable"), ("decorator", false)),
        ((fun_line, "fun"), ("keyword", false)),
        ((fun_line, "StickerRow"), ("function", true)),
        ((fun_line, "sticker"), ("parameter", true)),
        ((text_line, "Text"), ("function", false)),
        ((text_line, "label"), ("property", false)),
        ((greeting_line, "\"selam \""), ("string", false)),
        ((greeting_line, "Shout"), ("variable", false)),
        ((greeting_line - 2, "data"), ("keyword", false)),
        ((greeting_line - 2, "Sticker"), ("class", true)),
    ] {
        let got = seen.get(&(line, word.to_string()));
        let ok = got.is_some_and(|(t, m)| t == kind && (!declaration || m.iter().any(|m| m == "declaration")));
        client.check(&format!("the Kotlin's {word} on line {line}: {kind} (got {got:?})"), ok);
    }
    if with_analyzer {
        let helper_line = rs.line_of("pub fn helper") + 1;
        let got = seen.get(&(helper_line, "helper".to_string()));
        client.check(
            &format!("the Rust's tokens are rust-analyzer's (helper: a function, got {got:?})"),
            got.is_some_and(|(t, _)| t == "function"),
        );
    }

    // Hover: a parameter, a property of the crate's data class, an API composable, a declaration of the .kt file.
    let hover = |client: &mut Client, text: &Text, needle: &str, n: usize| {
        let h = client.request("textDocument/hover", text.params(text.at(needle, n)));
        h["contents"]["value"].str().to_string()
    };
    let h = hover(client, &rs, "sticker.label", 1);
    client.check(&format!("hover on a parameter: its type ({h:?})"), h.contains("sticker: Sticker"));
    let h = hover(client, &rs, "sticker.label", 9);
    client.check(&format!("hover on a property: its declaration ({h:?})"), h.contains("val label: String"));
    let h = hover(client, &rs, "Text(\"$prefix", 1);
    client.check(
        &format!("hover on an API composable: its signature and Rust ({h:?})"),
        h.contains("fun Text(") && h.contains("kompoz_material3"),
    );
    let h = hover(client, &rs, "+ Shout", 3);
    client.check(
        &format!("hover on the .kt file's val: its header ({h:?})"),
        h.contains("val Shout") && h.contains("ui.kt"),
    );
    let h = hover(client, &kt, "val shown", 5);
    client.check(&format!("hover in the .kt file: a local ({h:?})"), h.contains("val shown: String"));

    // Completion: members after a dot, names in scope (locals, the crate's Kotlin, the API's).
    let labels = |r: &Json| r["items"].array().iter().map(|i| i["label"].str().to_string()).collect::<Vec<_>>();
    rs.replace(client, "${sticker.label}", "${sticker.la}");
    let r = client.request("textDocument/completion", rs.params(rs.at("${sticker.la}", 12)));
    let got = labels(&r);
    client.check(&format!("completion after `sticker.`: label ({got:?})"), got.contains(&"label".to_string()));
    rs.replace(client, "${sticker.la}", "${sticker.}");
    let r = client.request("textDocument/completion", rs.params(rs.at("${sticker.}", 10)));
    let got = labels(&r);
    client.check(
        &format!("completion after `sticker.`, nothing typed: its properties ({got:?})"),
        got.contains(&"label".to_string()) && got.contains(&"count".to_string()),
    );
    rs.replace(client, "${sticker.}", "${sticker.label}");
    rs.replace(client, "Text(StickerGreeting", "Text(Sti");
    let r = client.request("textDocument/completion", rs.params(rs.at("Text(Sti", 8)));
    let got = labels(&r);
    client.check(
        &format!("completion of a name: the block's val and class ({got:?})"),
        got.contains(&"StickerGreeting".to_string()) && got.contains(&"Sticker".to_string()),
    );
    rs.replace(client, "Text(Sti", "Text(StickerGreeting");
    rs.replace(client, "Text(StickerGreeting", "Text(pre");
    let r = client.request("textDocument/completion", rs.params(rs.at("Text(pre", 8)));
    let got = labels(&r);
    client.check(&format!("completion of a name: a parameter ({got:?})"), got.contains(&"prefix".to_string()));
    rs.replace(client, "Text(pre", "Text(StickerGreeting");
    rs.replace(client, "\"selam \" + Shout", "\"selam \" + Sh");
    let r = client.request("textDocument/completion", rs.params(rs.at("\"selam \" + Sh", 13)));
    let got = labels(&r);
    client.check(&format!("completion of a name: the .kt file's val ({got:?})"), got.contains(&"Shout".to_string()));
    rs.replace(client, "\"selam \" + Sh", "\"selam \" + Shout");
    rs.replace(client, "Column {", "Colu {");
    let r = client.request("textDocument/completion", rs.params(rs.at("Colu {", 4)));
    let got = labels(&r);
    client
        .check(&format!("completion of a name: the API's composables ({got:?})"), got.contains(&"Column".to_string()));
    rs.replace(client, "Colu {", "Column {");

    // Definition: a local, the block's declaration, the .kt file's.
    let def = |client: &mut Client, text: &Text, needle: &str, n: usize| {
        client.request("textDocument/definition", text.params(text.at(needle, n)))
    };
    let d = def(client, &rs, "sticker.label", 1);
    client.check(
        &format!("definition of a parameter: its declaration ({})", d.dump()),
        d["uri"].str() == rs.uri && d["range"]["start"]["line"].int() as usize == fun_line - 1,
    );
    let d = def(client, &rs, "Text(StickerGreeting", 6);
    client.check(
        &format!("definition of the block's val ({})", d.dump()),
        d["range"]["start"]["line"].int() as usize == greeting_line - 1,
    );
    let d = def(client, &rs, "+ Shout", 3);
    client.check(
        &format!("definition of the .kt file's val: in it ({})", d.dump()),
        d["uri"].str() == kt.uri
            && d["range"]["start"]["line"].int() == 0
            && d["range"]["start"]["character"].int() == 4,
    );
    if with_analyzer {
        let d = def(client, &rs, "helper(sticker.count)", 1);
        let helper_line = rs.line_of("pub fn helper");
        client.check(
            &format!("definition of the crate's Rust function called from the Kotlin: rust-analyzer's ({})", d.dump()),
            d["uri"].str() == rs.uri && d["range"]["start"]["line"].int() as usize == helper_line,
        );
    }

    // Highlights and document symbols.
    let h = client.request("textDocument/documentHighlight", rs.params(rs.at("$prefix", 1)));
    client
        .check(&format!("highlights of a parameter: its declaration and its use ({})", h.dump()), h.array().len() == 2);
    let s = client.request("textDocument/documentSymbol", Json::object([("textDocument", rs.doc())]));
    let names: Vec<&str> = s.array().iter().map(|x| x["name"].str()).collect();
    client.check(
        &format!("document symbols: the Kotlin's declarations ({names:?})"),
        ["Sticker", "StickerGreeting", "StickerRow"].iter().all(|n| names.contains(n)),
    );
    let sticker = s.array().iter().find(|x| x["name"].str() == "Sticker");
    let members: Vec<&str> =
        sticker.map(|x| x["children"].array().iter().map(|c| c["name"].str()).collect()).unwrap_or_default();
    client.check(&format!("a data class's properties under it ({members:?})"), members == ["label", "count"]);
    if with_analyzer {
        client.check("and the Rust's, rust-analyzer's (helper)", names.contains(&"helper"));
    }
    let s = client.request("textDocument/documentSymbol", Json::object([("textDocument", kt.doc())]));
    let names: Vec<&str> = s.array().iter().map(|x| x["name"].str()).collect();
    client.check(&format!("the .kt file's symbols ({names:?})"), names == ["Shout", "Greeting"]);

    // Formatting: never of the Kotlin.
    let options = Json::object([("tabSize", 4.into()), ("insertSpaces", true.into())]);
    let r = client
        .request("textDocument/formatting", Json::object([("textDocument", kt.doc()), ("options", options.clone())]));
    client.check("no formatting of a .kt file", r.is_null());
    let r = client.request(
        "textDocument/rangeFormatting",
        Json::object([
            ("textDocument", rs.doc()),
            ("range", Json::object([("start", rs.at("Column", 0)), ("end", rs.at("Column", 3))])),
            ("options", options.clone()),
        ]),
    );
    client.check("no formatting of a range in the Kotlin", r.is_null());
    let r = client.request("textDocument/formatting", Json::object([("textDocument", rs.doc()), ("options", options)]));
    let block = (rs.line_of("kompoz! {"), rs.line_of("kompoz_file!") - 1);
    let touches = r.array().iter().any(|e| {
        let (a, b) = (e["range"]["start"]["line"].int() as usize, e["range"]["end"]["line"].int() as usize);
        a <= block.1 && block.0 < b
    });
    client.check(&format!("rustfmt's edits kept off the Kotlin ({})", r.dump()), !touches);

    // The generator's error in the Kotlin: where it is; fixed: gone.
    rs.replace(client, "StickerRow(", "StickerRow((");
    client.wait_for(
        "the generator's error in the Kotlin: where it is",
        &rs.uri.clone(),
        30,
        on_line(fun_line - 1, Some("kompoz"), None),
    );
    rs.replace(client, "StickerRow((", "StickerRow(");
    client.wait_for("fixed: gone", &rs.uri.clone(), 30, clean);

    // An edit of the .kt file (unsaved) is what the block's Kotlin sees.
    let mut kt = kt;
    kt.replace(client, "fun Greeting(name: String)", "fun Greeting(name: String, times: Int)");
    let r = client.request("textDocument/hover", rs.params(rs.at("+ Shout", 3)));
    client.check(
        "hover in the block still reads the .kt file as edited",
        r["contents"]["value"].str().contains("val Shout"),
    );
    let r = client.request("textDocument/documentSymbol", Json::object([("textDocument", kt.doc())]));
    let detail = r.array().iter().find(|x| x["name"].str() == "Greeting").map(|x| x["detail"].str().to_string());
    client.check(
        &format!("the .kt file's symbols as edited ({detail:?})"),
        detail.is_some_and(|d| d.contains("times: Int")),
    );
}

fn finish(failures: Vec<String>) {
    assert!(failures.is_empty(), "failed checks:\n  {}", failures.join("\n  "));
}

/// The Kotlin's answers, the server's own (no rust-analyzer).
#[test]
fn kotlin_without_rust_analyzer() {
    let f = Fixture::new();
    let mut client = Client::start(&["--no-rust-analyzer", "--no-shadow"], &f.0);
    let init = initialize(&mut client, &f.0, Json::Null);
    let legend = init["capabilities"]["semanticTokensProvider"]["legend"].clone();
    client.check("a semantic tokens legend", !legend["tokenTypes"].array().is_empty());
    kotlin_checks(&mut client, &f, &legend, false);
    let rs = Text { uri: f.uri("src/lib.rs"), lines: LIB.split('\n').map(str::to_string).collect(), version: 1 };
    let h = client.request("textDocument/hover", rs.params(rs.at("pub fn helper", 8)));
    client.check("no hover in the Rust without rust-analyzer", h.is_null());
    finish(client.stop());
}

fn rust_analyzer_runs() -> bool {
    Command::new("rust-analyzer")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// With rust-analyzer behind the server.
#[test]
fn with_rust_analyzer() {
    if !rust_analyzer_runs() {
        assert!(
            std::env::var("KOMPOZ_RUST_ANALYZER_REQUIRED").as_deref() != Ok("1"),
            "no rust-analyzer (rust-analyzer --version doesn't run) and KOMPOZ_RUST_ANALYZER_REQUIRED=1"
        );
        println!("skipped: no rust-analyzer (rust-analyzer --version doesn't run)");
        return;
    }
    let _one = ANALYZER.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let mut client = Client::start(&["--no-shadow"], &f.0);
    // (no cargo check, no build scripts or proc macros: the fixture has no dependency)
    let options = Json::parse(
        r#"{"checkOnSave": false, "cargo": {"buildScripts": {"enable": false}}, "procMacro": {"enable": false},
            "cachePriming": {"enable": false}}"#,
    )
    .unwrap();
    let init = initialize(&mut client, &f.0, options);
    let legend = init["capabilities"]["semanticTokensProvider"]["legend"].clone();
    let mut rs = open(&mut client, &f.uri("src/lib.rs"), LIB, "rust");
    client.wait_for("the file as it is: no diagnostics", &rs.uri.clone(), 180, |d| d.is_empty());
    // An error in the Rust around the Kotlin: where it is (rust-analyzer's).
    rs.replace(&mut client, "pub fn twice", "mod nothere_kz; pub fn twice");
    let module_line = rs.line_of("mod nothere_kz");
    client.wait_for(
        "an error in the Rust around the Kotlin: where it is",
        &rs.uri.clone(),
        120,
        on_line(module_line, Some("rust-analyzer"), None),
    );
    rs.replace(&mut client, "mod nothere_kz; pub fn twice", "pub fn twice");
    client.wait_for("fixed: no diagnostics", &rs.uri.clone(), 120, |d| d.is_empty());
    let h = client.request("textDocument/hover", rs.params(rs.at("pub fn helper", 8)));
    client.check(&format!("rust-analyzer's hover in the Rust ({})", h.dump()), h["contents"].dump().contains("helper"));
    let c = client.request("textDocument/completion", rs.params(rs.at("Text(\"$prefix", 2)));
    let ours = c["items"].array().iter().all(|i| i["data"]["kompoz"].as_bool() == Some(true));
    client.check("completion inside the Kotlin is the server's", ours && !c["items"].array().is_empty());
    client.notify("textDocument/didClose", Json::object([("textDocument", rs.doc())]));
    kotlin_checks(&mut client, &f, &legend, true);
    finish(client.stop());
}

/// rust-analyzer runs one at a time.
static ANALYZER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The workspace's own Kotlin (it compiles: the generator finds nothing wrong in it), read by the server as an
/// editor opens it — each file with its crate's module table, as the macros read it.
#[test]
fn the_workspaces_kotlin_has_no_errors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let files = [
        "examples/counter-kotlin/src/counter.kt",
        "examples/showcase/src/main.kt",
        "examples/showcase/src/common.kt",
        "examples/showcase/src/material/controls.kt",
        "examples/showcase/src/places/places.kt",
        "crates/kompoz/tests/kotlin_classes.rs",
        "crates/kompoz/tests/kotlin_lazy.rs",
        "crates/kompoz/tests/kt/ui.kt",
    ];
    let mut client = Client::start(&["--no-rust-analyzer", "--no-shadow"], &root);
    initialize(&mut client, &root, Json::Null);
    for f in files {
        let path = root.join(f);
        let text = std::fs::read_to_string(&path).unwrap();
        let uri = kompoz_lsp::doc::path_to_uri(&path);
        let language = if f.ends_with(".kt") { "kotlin" } else { "rust" };
        let doc = open(&mut client, &uri, &text, language);
        let errors = |d: &[Json]| !d.iter().any(|x| x["source"].str() == "kompoz" && x["severity"].int() == 1);
        client.wait_for(&format!("{f}: no errors in its Kotlin"), &uri, 60, errors);
        let s = client.request("textDocument/documentSymbol", Json::object([("textDocument", doc.doc())]));
        client.check(&format!("{f}: its declarations as symbols"), !s.array().is_empty());
        client.notify("textDocument/didClose", Json::object([("textDocument", doc.doc())]));
    }
    finish(client.stop());
}
