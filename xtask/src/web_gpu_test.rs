//! `cargo xtask web-gpu-test [--firefox] [--gpu] [--release] [names…]`: kompoz-gpu's GPU tests (tests/gpu) in a
//! browser, on WebGPU. The test binary is built for the web (wasm32-unknown-unknown on the web's pinned nightly,
//! std rebuilt with unwinding, as `cargo check-web`), served with its page (tests/gpu/web: index.html, runner.js) and
//! the glue's JavaScript (kompoz-web's kompoz.js, kompoz-gpu's webgpu.js) from a server of our own on 127.0.0.1, and
//! opened in a headless browser: Chrome on SwiftShader's WebGPU (the CPU's, as lavapipe is natively), or with
//! `--gpu` on the machine's GPU; Firefox with `--firefox`. The page sends back what the run prints (printed here as
//! it comes), the pictures it writes (saved into target/web-gpu-test/: `<name>.actual.png`, `<name>.diff.png`, or
//! the goldens with KOMPOZ_GOLDEN_UPDATE=1) and its result: the task fails if a check failed, or if nothing came
//! back in time.
//!
//! The tests' variables go in the page's query: KOMPOZ_GPU_REQUIRED=1 always (no device is a failure), and
//! KOMPOZ_GPU_VALIDATION, KOMPOZ_GOLDEN_UPDATE, KOMPOZ_BLUR_WORST as set here; names choose the tests
//! (KOMPOZ_GPU_TESTS). The environment:
//!
//!   KOMPOZ_CHROME=<binary>         Chrome (else google-chrome-stable, google-chrome, chromium on the PATH)
//!   KOMPOZ_FIREFOX=<binary>        Firefox (else firefox on the PATH)
//!   KOMPOZ_WEB_TOOLCHAIN=<name>    the toolchain built with (else nightly-2026-11-13 if installed, else nightly)
//!   KOMPOZ_WEB_TIMEOUT=<seconds>   how long the run may take (1800)
//!
//! Nothing is downloaded: the browsers are the machine's.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The web's pinned nightly (CI's web jobs and `cargo check-web` pin the same).
const WEB_NIGHTLY: &str = "nightly-2026-11-13";

/// What the page sends.
enum Message {
    Line(String),
    Done { checks: u64, failures: u64 },
}

pub fn run(workspace: &Path, args: &[&str]) -> Result<(), String> {
    let firefox = args.contains(&"--firefox");
    let gpu = args.contains(&"--gpu");
    let release = args.contains(&"--release");
    let names: Vec<&str> = args.iter().copied().filter(|a| !a.starts_with("--")).collect();
    if let Some(unknown) = args.iter().find(|a| a.starts_with("--") && !["--firefox", "--gpu", "--release"].contains(a))
    {
        return Err(format!("web-gpu-test: unknown option {unknown}"));
    }
    let wasm = build(workspace, release)?;
    let out = workspace.join("target").join("web-gpu-test");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("the test server: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let files = Files {
        index: workspace.join("crates/kompoz-gpu/tests/gpu/web/index.html"),
        runner: workspace.join("crates/kompoz-gpu/tests/gpu/web/runner.js"),
        glue: workspace.join("crates/kompoz-web/js/kompoz.js"),
        webgpu: workspace.join("crates/kompoz-gpu/src/device/webgpu/webgpu.js"),
        wasm,
        out: out.clone(),
    };
    let (sender, messages) = mpsc::channel();
    std::thread::spawn(move || serve(listener, files, sender));

    let mut query = vec![("KOMPOZ_GPU_REQUIRED".to_string(), "1".to_string())];
    for name in ["KOMPOZ_GPU_VALIDATION", "KOMPOZ_GOLDEN_UPDATE", "KOMPOZ_BLUR_WORST"] {
        if let Ok(value) = std::env::var(name) {
            query.push((name.to_string(), value));
        }
    }
    if !names.is_empty() {
        query.push(("KOMPOZ_GPU_TESTS".to_string(), names.join(",")));
    }
    let query: Vec<String> = query.iter().map(|(n, v)| format!("{n}={}", percent_encoded(v))).collect();
    let url = format!("http://127.0.0.1:{port}/?{}", query.join("&"));
    let mut browser = if firefox { open_firefox(&out, &url)? } else { open_chrome(&out, &url, gpu)? };
    println!("web-gpu-test: {url}");

    let timeout = std::env::var("KOMPOZ_WEB_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(1800u64);
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let result = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break Err(format!("web-gpu-test: no result in {timeout} s"));
        }
        match messages.recv_timeout(left.min(Duration::from_secs(1))) {
            Ok(Message::Line(line)) => println!("{line}"),
            Ok(Message::Done { checks, failures }) => {
                break if failures == 0 && checks > 0 {
                    Ok(())
                } else {
                    Err(format!("web-gpu-test: {failures} of {checks} checks failed"))
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(Some(status)) = browser.try_wait() {
                    break Err(format!(
                        "web-gpu-test: the browser exited ({status}) before the run finished (its output: {})",
                        out.join("browser.log").display()
                    ));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err("web-gpu-test: the test server stopped".into()),
        }
    };
    let _ = browser.kill();
    let _ = browser.wait();
    result
}

/// Builds the test binary for the web; its path.
fn build(workspace: &Path, release: bool) -> Result<PathBuf, String> {
    let toolchain = std::env::var("KOMPOZ_WEB_TOOLCHAIN").unwrap_or_else(|_| {
        let installed = Command::new("rustup")
            .args(["toolchain", "list"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.starts_with(WEB_NIGHTLY)))
            .unwrap_or(false);
        if installed { WEB_NIGHTLY.to_string() } else { "nightly".to_string() }
    });
    // (rustup's cargo for the toolchain: not the one running this task, which $CARGO names.)
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(workspace)
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("CARGO")
        .arg(format!("+{toolchain}"))
        .args(["test", "-p", "kompoz-gpu", "--test", "gpu", "--no-run", "--message-format=json-render-diagnostics"])
        .args(["--target", "wasm32-unknown-unknown", "-Zbuild-std=std,panic_unwind", "--config"])
        .arg("target.wasm32-unknown-unknown.rustflags = ['-Cpanic=unwind', '-Cllvm-args=-wasm-use-legacy-eh=false']")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if release {
        cargo.arg("--release");
    }
    println!("web-gpu-test: building the GPU tests for the web ({toolchain})");
    let mut child = cargo.spawn().map_err(|e| format!("cargo: {e}"))?;
    let stdout = child.stdout.take().ok_or("cargo's output")?;
    let mut wasm = None;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if line.contains("\"reason\":\"compiler-artifact\"")
            && let Some(path) = json_string(&line, "executable")
        {
            wasm = Some(PathBuf::from(path));
        }
    }
    let status = child.wait().map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err("web-gpu-test: the build failed".into());
    }
    wasm.ok_or_else(|| "web-gpu-test: cargo named no test binary".to_string())
}

/// A string field of a line of cargo's JSON (paths have no escapes but backslashes, Windows' only).
fn json_string(line: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\":\"");
    let start = line.find(&key)? + key.len();
    let end = start + line[start..].find('"')?;
    Some(line[start..end].replace("\\\\", "\\"))
}

fn percent_encoded(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn percent_decoded(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The first of these on the PATH.
fn on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    names.iter().find_map(|name| std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| p.is_file()))
}

fn browser_log(out: &Path) -> Result<Stdio, String> {
    let log = std::fs::File::create(out.join("browser.log")).map_err(|e| format!("browser.log: {e}"))?;
    Ok(Stdio::from(log))
}

/// Headless Chrome on WebGPU: SwiftShader's (Vulkan on the CPU, shipped with Chrome), or with `gpu` the machine's
/// GPU (Vulkan), as the C++ Kompoz's web tests ran it.
fn open_chrome(out: &Path, url: &str, gpu: bool) -> Result<Child, String> {
    let binary = std::env::var_os("KOMPOZ_CHROME")
        .map(PathBuf::from)
        .or_else(|| on_path(&["google-chrome-stable", "google-chrome", "chromium", "chromium-browser"]))
        .ok_or("web-gpu-test: no Chrome (KOMPOZ_CHROME names one)")?;
    let profile = out.join("chrome-profile");
    let _ = std::fs::remove_dir_all(&profile);
    let mut chrome = Command::new(binary);
    chrome
        .arg("--headless=new")
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(["--no-first-run", "--no-default-browser-check", "--disable-extensions", "--no-sandbox"])
        .args(["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--disable-gpu-watchdog"]);
    if gpu {
        // (Headless Chrome keeps to software unless told the GPU is fine: its blocklist, ANGLE on Vulkan.)
        chrome.args(["--ignore-gpu-blocklist", "--enable-gpu"]);
    } else {
        chrome.arg("--use-webgpu-adapter=swiftshader");
    }
    let log = browser_log(out)?;
    chrome.arg(url).stdout(Stdio::null()).stderr(log).spawn().map_err(|e| format!("Chrome: {e}"))
}

/// Headless Firefox with WebGPU on (its own pref, and past the blocklist: a CI machine's GPU is a software one),
/// JSPI and wasm's exception references, in a profile of its own.
fn open_firefox(out: &Path, url: &str) -> Result<Child, String> {
    let binary = std::env::var_os("KOMPOZ_FIREFOX")
        .map(PathBuf::from)
        .or_else(|| on_path(&["firefox", "firefox-esr"]))
        .ok_or("web-gpu-test: no Firefox (KOMPOZ_FIREFOX names one)")?;
    let profile = out.join("firefox-profile");
    let _ = std::fs::remove_dir_all(&profile);
    std::fs::create_dir_all(&profile).map_err(|e| format!("{}: {e}", profile.display()))?;
    let prefs = [
        ("dom.webgpu.enabled", "true"),
        ("gfx.webgpu.ignore-blocklist", "true"),
        ("gfx.webgpu.force-enabled", "true"),
        ("javascript.options.wasm_js_promise_integration", "true"),
        ("javascript.options.wasm_exnref", "true"),
        ("browser.shell.checkDefaultBrowser", "false"),
        ("browser.aboutwelcome.enabled", "false"),
        ("datareporting.policy.dataSubmissionEnabled", "false"),
        ("toolkit.telemetry.reportingpolicy.firstRun", "false"),
    ];
    let user: String = prefs.iter().map(|(name, value)| format!("user_pref(\"{name}\", {value});\n")).collect();
    std::fs::write(profile.join("user.js"), user).map_err(|e| format!("user.js: {e}"))?;
    let log = browser_log(out)?;
    Command::new(binary)
        .args(["--headless", "--no-remote", "--profile"])
        .arg(&profile)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("Firefox: {e}"))
}

/// What the server serves, and where it saves the pictures.
struct Files {
    index: PathBuf,
    runner: PathBuf,
    glue: PathBuf,
    webgpu: PathBuf,
    wasm: PathBuf,
    out: PathBuf,
}

/// Serves the page and takes what it sends, a connection at a time (the page sends one request after another).
fn serve(listener: TcpListener, files: Files, messages: mpsc::Sender<Message>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        // (A browser may close a connection it no longer wants the answer of: not the run's failure.)
        if let Err(why) = answer(stream, &files, &messages)
            && !matches!(why.kind(), std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset)
        {
            let _ = messages.send(Message::Line(format!("web-gpu-test: a request failed: {why}")));
        }
    }
}

fn answer(mut stream: TcpStream, files: &Files, messages: &mpsc::Sender<Message>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let mut parts = request.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let respond = |stream: &mut TcpStream, status: &str, kind: &str, content: &[u8]| -> std::io::Result<()> {
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
             Connection: close\r\n\r\n",
            content.len()
        )?;
        stream.write_all(content)
    };
    match (method, path) {
        ("GET", "/" | "/index.html") => respond(&mut stream, "200 OK", "text/html", &std::fs::read(&files.index)?),
        ("GET", "/runner.js") => respond(&mut stream, "200 OK", "text/javascript", &std::fs::read(&files.runner)?),
        ("GET", "/kompoz.js") => respond(&mut stream, "200 OK", "text/javascript", &std::fs::read(&files.glue)?),
        ("GET", "/webgpu.js") => respond(&mut stream, "200 OK", "text/javascript", &std::fs::read(&files.webgpu)?),
        ("GET", "/gpu.wasm") => respond(&mut stream, "200 OK", "application/wasm", &std::fs::read(&files.wasm)?),
        ("POST", "/log") => {
            let _ = messages.send(Message::Line(String::from_utf8_lossy(&body).into_owned()));
            respond(&mut stream, "204 No Content", "text/plain", b"")
        }
        ("POST", "/file") => {
            // (Saved by its name alone, in the output directory: the page names it by the run's own path.)
            let named = query.split('&').find_map(|p| p.strip_prefix("path=")).map(percent_decoded);
            let name = named.as_deref().and_then(|p| Path::new(p).file_name()).map(|n| n.to_owned());
            if let Some(name) = name {
                let saved = files.out.join(name);
                std::fs::write(&saved, &body)?;
                let _ = messages.send(Message::Line(format!("  (saved {})", saved.display())));
            }
            respond(&mut stream, "204 No Content", "text/plain", b"")
        }
        ("POST", "/done") => {
            let text = String::from_utf8_lossy(&body);
            let number = |field: &str| {
                let key = format!("\"{field}\":");
                let start = text.find(&key)? + key.len();
                text[start..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
            };
            let (checks, failures) = (number("checks").unwrap_or(0), number("failures").unwrap_or(1));
            respond(&mut stream, "204 No Content", "text/plain", b"")?;
            let _ = messages.send(Message::Done { checks, failures });
            Ok(())
        }
        ("GET", "/favicon.ico") => respond(&mut stream, "204 No Content", "text/plain", b""),
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not here"),
    }
}
