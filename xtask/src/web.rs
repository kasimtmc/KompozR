//! `cargo xtask web build|serve|test`: KompozR's apps on the web.
//!
//! - `build <app> [--release]`: the app (`counter`, `showcase`, or `<package>[:<binary>]`) built for the web
//!   (wasm32-unknown-unknown on the web's pinned nightly, std rebuilt with unwinding, as `cargo check-web`) and put
//!   with its page into target/web/<app>/: index.html and app.js (kompoz-platform's web/page), the glue's
//!   JavaScript (kompoz-web's kompoz.js, kompoz-gpu's webgpu.js, kompoz-platform's web.js as platform.js), app.wasm.
//! - `serve <app> [--release] [--port <n>]`: built, then served from 127.0.0.1 (8765 unless told) until stopped:
//!   open the address it prints (the page's query is the app's environment: `?KOMPOZ_SCRIPT=…&LANG=tr_TR`).
//! - `test [--firefox] [--gpu] [--release] [apps…]`: the Counter and the showcase (or those named) run by scripts
//!   in a headless browser — Chrome on SwiftShader's WebGPU (with `--gpu` on the machine's GPU), Firefox with
//!   `--firefox` — and headless natively (lavapipe), with the same scripts, fonts and environment: each shot the
//!   page sends back is the native run's within the GPU goldens' three a channel (as Linux's display tests compare
//!   a window's shots with the headless run's; kompoz-gpu's golden tests hold WebGPU to lavapipe the same way).
//!   What differs is kept in target/web-test/<app>/ (`<shot>.web.png`, `<shot>.diff.png`), with the browser's log.
//!   The screen readers' mirror is checked too: dumped by the page as a screen reader reads it (its elements' roles,
//!   names, places and states: target/web-test/<app>/web/*.aria.txt) where the script says, each holding what it
//!   must, and the Counter's button activated through it as a screen reader does (its shot as natively the second
//!   click's).
//!
//! The environment: KOMPOZ_CHROME, KOMPOZ_FIREFOX, KOMPOZ_WEB_TOOLCHAIN, KOMPOZ_WEB_TIMEOUT as `web-gpu-test` reads
//! them. Nothing is downloaded: the browsers are the machine's.

use crate::png::{self, Picture};
use crate::web_gpu_test::{json_string, open_chrome, open_firefox, percent_decoded, percent_encoded, web_toolchain};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How far a channel may be off (kompoz-gpu's goldens').
const TOLERANCE: i32 = 3;

/// An app of the workspace: its package and its binary.
struct WebApp {
    name: String,
    package: String,
    binary: String,
}

fn app_of(name: &str) -> WebApp {
    let (package, binary) = name.split_once(':').unwrap_or((name, name));
    WebApp { name: package.to_string(), package: package.to_string(), binary: binary.to_string() }
}

pub fn run(workspace: &Path, args: &[&str]) -> Result<(), String> {
    let release = args.contains(&"--release");
    let plain: Vec<&str> = args.iter().copied().filter(|a| !a.starts_with("--")).collect();
    match plain.first().copied() {
        Some("build") => {
            let app = app_of(plain.get(1).ok_or("web build: which app (counter, showcase)?")?);
            let dist = build_dist(workspace, &app, release)?;
            println!("web: {}", dist.display());
            Ok(())
        }
        Some("serve") => {
            let app = app_of(plain.get(1).ok_or("web serve: which app (counter, showcase)?")?);
            let port = args
                .iter()
                .position(|a| *a == "--port")
                .and_then(|i| args.get(i + 1))
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(8765);
            let dist = build_dist(workspace, &app, release)?;
            let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("port {port}: {e}"))?;
            println!("web: http://127.0.0.1:{port}/ (stop with Ctrl+C)");
            let (sender, messages) = mpsc::channel();
            let out = dist.clone();
            std::thread::spawn(move || serve(listener, dist, out, sender));
            for message in messages {
                if let Message::Line(line) = message {
                    println!("{line}");
                }
            }
            Ok(())
        }
        Some("test") => test(workspace, args, &plain[1..], release),
        _ => Err("web: build <app> | serve <app> [--port <n>] | test [--firefox] [--gpu] [--release] [apps…]".into()),
    }
}

/// The app built for the web; its module's path.
fn build_wasm(workspace: &Path, app: &WebApp, release: bool) -> Result<PathBuf, String> {
    let toolchain = web_toolchain();
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(workspace)
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("CARGO")
        // (No debug info: a module's size is limited, the showcase's with it is over a gigabyte.)
        .env("CARGO_PROFILE_DEV_DEBUG", "false")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "false")
        .arg(format!("+{toolchain}"))
        .args(["build", "-p", &app.package, "--bin", &app.binary, "--message-format=json-render-diagnostics"])
        .args(["--target", "wasm32-unknown-unknown", "-Zbuild-std=std,panic_unwind", "--config"])
        .arg("target.wasm32-unknown-unknown.rustflags = ['-Cpanic=unwind', '-Cllvm-args=-wasm-use-legacy-eh=false']")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if release {
        cargo.arg("--release");
    }
    println!("web: building {} for the web ({toolchain})", app.name);
    artifact(cargo, "web: the web build")
}

/// The app built natively; its program's path.
fn build_native(workspace: &Path, app: &WebApp, release: bool) -> Result<PathBuf, String> {
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(workspace)
        .args(["build", "-p", &app.package, "--bin", &app.binary, "--message-format=json-render-diagnostics"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if release {
        cargo.arg("--release");
    }
    println!("web: building {} natively", app.name);
    artifact(cargo, "web: the native build")
}

/// What cargo built (its JSON's last executable).
fn artifact(mut cargo: Command, what: &str) -> Result<PathBuf, String> {
    let mut child = cargo.spawn().map_err(|e| format!("cargo: {e}"))?;
    let stdout = child.stdout.take().ok_or("cargo's output")?;
    let mut built = None;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if line.contains("\"reason\":\"compiler-artifact\"")
            && let Some(path) = json_string(&line, "executable")
        {
            built = Some(PathBuf::from(path));
        }
    }
    let status = child.wait().map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err(format!("{what} failed"));
    }
    built.ok_or_else(|| format!("{what}: cargo named no program"))
}

/// The page's files, with the module: target/web/<app>/.
fn build_dist(workspace: &Path, app: &WebApp, release: bool) -> Result<PathBuf, String> {
    let wasm = build_wasm(workspace, app, release)?;
    let dist = workspace.join("target").join("web").join(&app.name);
    std::fs::create_dir_all(&dist).map_err(|e| format!("{}: {e}", dist.display()))?;
    let page = workspace.join("crates/kompoz-platform/src/web/page");
    for (from, to) in [
        (page.join("index.html"), "index.html"),
        (page.join("app.js"), "app.js"),
        (workspace.join("crates/kompoz-web/js/kompoz.js"), "kompoz.js"),
        (workspace.join("crates/kompoz-gpu/src/device/webgpu/webgpu.js"), "webgpu.js"),
        (workspace.join("crates/kompoz-platform/src/web/web.js"), "platform.js"),
        (wasm, "app.wasm"),
    ] {
        std::fs::copy(&from, dist.join(to)).map_err(|e| format!("{}: {e}", from.display()))?;
    }
    Ok(dist)
}

/// What the page sends.
enum Message {
    Line(String),
    Done(i64),
}

/// Serves `dist`'s files and takes what the page sends (its log, its files into `out`, its end).
fn serve(listener: TcpListener, dist: PathBuf, out: PathBuf, messages: mpsc::Sender<Message>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if let Err(why) = answer(stream, &dist, &out, &messages)
            && !matches!(why.kind(), std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset)
        {
            let _ = messages.send(Message::Line(format!("web: a request failed: {why}")));
        }
    }
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn answer(mut stream: TcpStream, dist: &Path, out: &Path, messages: &mpsc::Sender<Message>) -> std::io::Result<()> {
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
    match method {
        "GET" => {
            let name = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
            // (Only the page's own files, by name.)
            if name.contains('/') || name.contains("..") {
                return respond(&mut stream, "404 Not Found", "text/plain", b"not here");
            }
            match std::fs::read(dist.join(name)) {
                Ok(content) => respond(&mut stream, "200 OK", content_type(name), &content),
                Err(_) => respond(&mut stream, "404 Not Found", "text/plain", b"not here"),
            }
        }
        "POST" => {
            match path {
                "/log" => {
                    let _ = messages.send(Message::Line(String::from_utf8_lossy(&body).into_owned()));
                }
                "/file" => {
                    // (Saved by its name alone, in the output directory.)
                    let named = query.split('&').find_map(|p| p.strip_prefix("path=")).map(percent_decoded);
                    if let Some(name) = named.as_deref().and_then(|p| Path::new(p).file_name()) {
                        std::fs::write(out.join(name), &body)?;
                    }
                }
                "/done" => {
                    let text = String::from_utf8_lossy(&body);
                    let code = text
                        .split_once("\"code\":")
                        .and_then(|(_, rest)| {
                            rest.trim()
                                .chars()
                                .take_while(|c| c.is_ascii_digit() || *c == '-')
                                .collect::<String>()
                                .parse()
                                .ok()
                        })
                        .unwrap_or(1);
                    respond(&mut stream, "204 No Content", "text/plain", b"")?;
                    let _ = messages.send(Message::Done(code));
                    return Ok(());
                }
                _ => return respond(&mut stream, "404 Not Found", "text/plain", b"not here"),
            }
            respond(&mut stream, "204 No Content", "text/plain", b"")
        }
        _ => respond(&mut stream, "405 Method Not Allowed", "text/plain", b""),
    }
}

/// A test run of an app: its script, its fonts, its shots.
struct Run {
    app: WebApp,
    script: String,
    /// The page's own script where its actions are the page's (`KOMPOZ_WEB_DRIVE`: what a user does, through the
    /// page's events) rather than the app script's: its script, and the actions by frame.
    web: Option<(String, String)>,
    /// The fonts natively (fontconfig's only ones): those the app is set in on the web (the bundled ones, or the
    /// showcase's own comparison fonts, which it registers there).
    fonts: PathBuf,
    shots: Vec<&'static str>,
    /// The screen readers' mirror as the page dumped it (`aria:<file>` among the page's actions): each file's lines
    /// that must be there (a line holding each: its pieces between ` … `, in that order).
    aria: Vec<(&'static str, Vec<&'static str>)>,
}

fn runs(workspace: &Path, names: &[&str]) -> Result<Vec<Run>, String> {
    let names = if names.is_empty() { vec!["counter", "showcase"] } else { names.to_vec() };
    names
        .into_iter()
        .map(|name| match name {
            // Linux's display tests' script: a shot, a click (the count changed), a resize (the new size); then a
            // second click. On the web the first click is the page's: a pointer's events on the canvas, before the
            // frame the script clicks in (its 33rd: the resize and the wait, the shot, then the click); the second
            // a screen reader's, on the button's mirrored element (frame 80, a few after shot c), the mirror dumped
            // before it and after.
            "counter" => Ok(Run {
                app: app_of("counter"),
                script: "resize 360 480;wait 30;shot a.png;click 70 70;wait 30;shot b.png;resize 500 400;wait 10;\
                         shot c.png;click 70 70;wait 30;shot d.png;quit"
                    .into(),
                web: Some((
                    "resize 360 480;wait 30;shot a.png;wait 30;shot b.png;resize 500 400;wait 10;shot c.png;wait 30;\
                     shot d.png;quit"
                        .into(),
                    "33:click:70,70;74:aria:c.aria.txt;80:ariaclick:button:+1;104:aria:d.aria.txt".into(),
                )),
                fonts: workspace.join("crates/kompoz-platform/src/web/fonts"),
                shots: vec!["a.png", "b.png", "c.png", "d.png"],
                aria: vec![
                    ("c.aria.txt", vec!["button \"+1\" @24,24 120x80", "\"Sayaç: 1\""]),
                    ("d.aria.txt", vec!["button \"+1\" @24,24 120x80", "\"Sayaç: 2\""]),
                ],
            }),
            // The Windows test's: the Material tab's first page, the next as the page is dragged up, then the
            // theme switched (the app bar's moon) and the page again.
            "showcase" => {
                let mut s = String::from("resize 440 860; wait 10; click 385 830; wait 60; shot 00.png; ");
                s.push_str("down 10 700; wait 1; ");
                for k in 1..=12 {
                    s.push_str(&format!("move 10 {}; wait 1; ", 700 - 50 * k));
                }
                s.push_str("wait 6; up 10 100; wait 60; shot 01.png; click 316 32; wait 60; shot 02.png; quit");
                // On the web the mirror is dumped before each shot's frame (73, 167, 229); then, after the last shot
                // (the web's script waits before it quits), a screen reader activates the e-mail field and types into
                // it (the text area linked to it), then the password field (a password input linked), then a message
                // row's custom action (its button beside the row: the count of unread messages, a live region, said).
                let web = s.replace("shot 02.png; quit", "shot 02.png; wait 90; quit");
                let drive = "72:aria:00.aria.txt;166:aria:01.aria.txt;228:aria:02.aria.txt;\
                             232:ariaclick:textbox:E-posta E-posta;240:type:ali@ornek.com;250:aria:03.aria.txt;\
                             252:ariaclick:textbox:Şifre Şifre;260:type:gizli;275:aria:04.aria.txt;\
                             280:ariaclick:button:Okundu say;300:aria:05.aria.txt"
                    .to_string();
                Ok(Run {
                    app: app_of("showcase"),
                    script: s,
                    web: Some((web, drive)),
                    fonts: workspace.join("examples/showcase/fonts"),
                    shots: vec!["00.png", "01.png", "02.png"],
                    // (Roles, names, places and states; the places after the page was dragged up as it moved.)
                    aria: vec![
                        (
                            "00.aria.txt",
                            vec![
                                "button \"Koyu tema\" @296,12 40x40",
                                "textbox \"Arama Ara Şehir ara\" @36,124 368x56",
                                "checkbox \"Üst\" @36,476 56x32 aria-checked=true",
                                "combobox \"Meyve\" @36,692 368x57",
                                "tab \"Material\" @330,795 110x65 aria-posinset=4 aria-selected=true aria-setsize=4",
                                "radio \"Öğle\" … aria-checked=true aria-posinset=2 aria-setsize=3",
                                "switch \"Hatırlatmalar\" … aria-checked=true",
                                "slider \"Aralık başlangıcı\" … aria-valuemax=800 aria-valuemin=0 aria-valuenow=200",
                                "checkbox \"\" … aria-checked=mixed",
                                "textbox \"Kilitli Müşteri no (enabled = false) … aria-disabled=true",
                                "listitem \"Satır 1\" … aria-setsize=40",
                                "img \"Tutamak\"",
                                "button \"Arşivle\" (custom action)",
                                "region \"Gezinme menüsü\"",
                            ],
                        ),
                        (
                            "01.aria.txt",
                            vec![
                                "combobox \"Meyve\" @36,128 368x57",
                                "button \"Tarih seç\" @36,349 126x40",
                                "checkbox \"12 saat\" @170,517 81x32 aria-checked=false",
                                "tab \"Material\" @330,795 110x65 aria-posinset=4 aria-selected=true",
                            ],
                        ),
                        (
                            "02.aria.txt",
                            vec!["button \"Tarih seç\" @36,349 126x40", "tab \"Material\" @330,795 110x65"],
                        ),
                        (
                            "03.aria.txt",
                            vec![
                                "none \"\" @",
                                "  textbox \"E-posta E-posta … value=\"ali@ornek.com\" (text area) focused",
                            ],
                        ),
                        (
                            "04.aria.txt",
                            vec![
                                "  textbox \"Şifre Şifre … value=\"•••••\" (password input) focused",
                                "textbox \"E-posta E-posta",
                            ],
                        ),
                        (
                            "05.aria.txt",
                            vec!["\"Ayşe: Toplantı yarın 10'da Son eylem: okundu\"", "polite: 2 okunmamış ileti"],
                        ),
                    ],
                })
            }
            other => Err(format!("web test: no script for {other} (counter, showcase)")),
        })
        .collect()
}

/// What both runs share: the script, the scale, the language and direction, the time zone, no desktop colors,
/// validation on.
fn environment(script: &str) -> Vec<(String, String)> {
    [
        ("KOMPOZ_SCRIPT", script),
        ("KOMPOZ_SCALE", "1"),
        ("KOMPOZ_LANGUAGE", "tr"),
        ("KOMPOZ_LAYOUT_DIRECTION", "ltr"),
        ("LANG", "tr_TR.UTF-8"),
        ("TZ", "Europe/Istanbul"),
        ("KOMPOZ_NO_DESKTOP_ACCENT", "1"),
        ("KOMPOZ_NO_WALLPAPER_COLORS", "1"),
        ("KOMPOZ_GPU_VALIDATION", "1"),
    ]
    .iter()
    .map(|(n, v)| (n.to_string(), v.to_string()))
    .collect()
}

/// A fontconfig configuration of only `fonts` (sans-serif Noto Sans, monospace Noto Sans Mono where it is there).
fn write_fontconfig(dir: &Path, fonts: &Path) -> Result<PathBuf, String> {
    let path = dir.join("fonts.conf");
    let alias = |generic: &str, family: &str| {
        format!(
            "  <alias binding=\"strong\"><family>{generic}</family><prefer><family>{family}</family></prefer></alias>\n"
        )
    };
    let conf = format!(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\">\n<fontconfig>\n  <dir>{}</dir>\n  \
         <cachedir>{}</cachedir>\n{}{}{}</fontconfig>\n",
        fonts.display(),
        dir.join("fontconfig-cache").display(),
        alias("sans-serif", "Noto Sans"),
        alias("serif", "Noto Sans"),
        alias("monospace", "Noto Sans Mono"),
    );
    std::fs::write(&path, conf).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Mesa's lavapipe's ICD file.
fn lavapipe() -> Option<PathBuf> {
    let dir = Path::new("/usr/share/vulkan/icd.d");
    ["lvp_icd.json".to_string(), format!("lvp_icd.{}.json", std::env::consts::ARCH)]
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
}

/// The app run headless natively by `run`'s script in `dir`.
fn run_native(binary: &Path, run: &Run, dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let fontconfig = write_fontconfig(dir, &run.fonts)?;
    let mut command = Command::new(binary);
    if std::env::var_os("VK_DRIVER_FILES").is_none() && std::env::var_os("VK_ICD_FILENAMES").is_none() {
        command.env("VK_DRIVER_FILES", lavapipe().ok_or("no lavapipe (/usr/share/vulkan/icd.d/lvp_icd*.json)")?);
    }
    let output = command
        .current_dir(dir)
        .envs(environment(&run.script))
        .env("KOMPOZ_HEADLESS", "1")
        .env("FONTCONFIG_FILE", fontconfig)
        .env_remove("KOMPOZ_STATE_FILE")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let log = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() || log.contains("GPU validation") {
        return Err(format!("web test: the native run of {} failed ({}):\n{log}", run.app.name, output.status));
    }
    Ok(())
}

/// The app run in the browser by `run`'s script; its shots land in `dir`.
fn run_web(dist: &Path, run: &Run, dir: &Path, firefox: bool, gpu: bool) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("the server: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (sender, messages) = mpsc::channel();
    let (dist, out) = (dist.to_path_buf(), dir.to_path_buf());
    std::thread::spawn(move || serve(listener, dist, out, sender));
    let (script, drive) = run.web.clone().unwrap_or_else(|| (run.script.clone(), String::new()));
    let mut query: Vec<String> =
        environment(&script).iter().map(|(n, v)| format!("{n}={}", percent_encoded(v))).collect();
    query.push("KOMPOZ_WEB_REPORT=1".into());
    if !drive.is_empty() {
        query.push(format!("KOMPOZ_WEB_DRIVE={}", percent_encoded(&drive)));
    }
    let url = format!("http://127.0.0.1:{port}/?{}", query.join("&"));
    let mut browser = if firefox { open_firefox(dir, &url)? } else { open_chrome(dir, &url, gpu)? };
    println!("web: {} in {}: {url}", run.app.name, if firefox { "Firefox" } else { "Chrome" });
    let timeout = std::env::var("KOMPOZ_WEB_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(900u64);
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut validation = false;
    let result = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break Err(format!("web test: {} didn't end in {timeout} s", run.app.name));
        }
        match messages.recv_timeout(left.min(Duration::from_secs(1))) {
            Ok(Message::Line(line)) => {
                validation |= line.contains("GPU validation") || line.contains("WebGPU:");
                println!("  {line}");
            }
            Ok(Message::Done(0)) => break Ok(()),
            Ok(Message::Done(code)) => break Err(format!("web test: {} ended with {code}", run.app.name)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(Some(status)) = browser.try_wait() {
                    break Err(format!(
                        "web test: the browser exited ({status}) (its output: {})",
                        dir.join("browser.log").display()
                    ));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err("web test: the server stopped".into()),
        }
    };
    let _ = browser.kill();
    let _ = browser.wait();
    result?;
    if validation {
        return Err(format!("web test: {}: WebGPU reported errors (above)", run.app.name));
    }
    Ok(())
}

/// The screen readers' mirror as the page dumped it: each file there, holding what it must.
fn check_aria(run: &Run, web: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    for (file, wanted) in &run.aria {
        let dump = match std::fs::read_to_string(web.join(file)) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: the mirror's {file}: {e}", run.app.name));
                continue;
            }
        };
        // (A line holding the pieces between ` … `, in order.)
        let holds = |line: &str, wanted: &str| {
            let mut rest = line;
            wanted.split(" … ").all(|piece| match rest.find(piece) {
                Some(at) => {
                    rest = &rest[at + piece.len()..];
                    true
                }
                None => false,
            })
        };
        let missing: Vec<&str> = wanted.iter().copied().filter(|w| !dump.lines().any(|l| holds(l, w))).collect();
        if missing.is_empty() {
            println!("web: {}: the mirror's {file} as it must be", run.app.name);
        } else {
            failures.push(format!(
                "{}: the mirror's {file} has no line with {} (it is in {})",
                run.app.name,
                missing.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", "),
                web.join(file).display()
            ));
        }
    }
    failures
}

/// The web run's shots against the native run's: the same size, every channel within [`TOLERANCE`]. What differs
/// kept beside the native ones.
fn compare(run: &Run, native: &Path, web: &Path, out: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    for name in &run.shots {
        let read = |dir: &Path| -> Result<Picture, String> {
            let bytes = std::fs::read(dir.join(name)).map_err(|e| format!("{}: {e}", dir.join(name).display()))?;
            png::decode(&bytes).map_err(|e| format!("{name}: {e}"))
        };
        let (n, w) = match (read(native), read(web)) {
            (Ok(n), Ok(w)) => (n, w),
            (Err(e), _) | (_, Err(e)) => {
                failures.push(format!("{}: {e}", run.app.name));
                continue;
            }
        };
        if (n.width, n.height) != (w.width, w.height) {
            failures.push(format!(
                "{}: {name}: {}x{} on the web, {}x{} natively",
                run.app.name, w.width, w.height, n.width, n.height
            ));
            continue;
        }
        let (mut count, mut worst) = (0usize, 0i32);
        let mut diff = n.rgba.clone();
        for (i, (p, q)) in n.rgba.chunks(4).zip(w.rgba.chunks(4)).enumerate() {
            let m = p.iter().zip(q).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0);
            worst = worst.max(m);
            let d = &mut diff[i * 4..i * 4 + 4];
            if m > TOLERANCE {
                count += 1;
                d.copy_from_slice(&[255, 0, 80, 255]);
            } else {
                for c in d.iter_mut().take(3) {
                    *c = 255 - (255 - *c) / 4;
                }
            }
        }
        if count > 0 {
            let stem = name.trim_end_matches(".png");
            let _ = std::fs::write(out.join(format!("{stem}.native.png")), png::encode(&n));
            let _ = std::fs::write(out.join(format!("{stem}.web.png")), png::encode(&w));
            let _ = std::fs::write(
                out.join(format!("{stem}.diff.png")),
                png::encode(&Picture { width: n.width, height: n.height, rgba: diff }),
            );
            failures
                .push(format!("{}: {name}: {count} pixels off by more than {TOLERANCE} (worst {worst})", run.app.name));
        } else {
            println!("web: {}: {name} as natively (worst {worst})", run.app.name);
        }
    }
    failures
}

fn test(workspace: &Path, args: &[&str], names: &[&str], release: bool) -> Result<(), String> {
    let firefox = args.contains(&"--firefox");
    let gpu = args.contains(&"--gpu");
    if let Some(unknown) = args.iter().find(|a| a.starts_with("--") && !["--firefox", "--gpu", "--release"].contains(a))
    {
        return Err(format!("web test: unknown option {unknown}"));
    }
    let mut failures = Vec::new();
    for run in runs(workspace, names)? {
        let dist = build_dist(workspace, &run.app, release)?;
        let native_binary = build_native(workspace, &run.app, release)?;
        let out = workspace.join("target").join("web-test").join(&run.app.name);
        let (native, web) = (out.join("native"), out.join("web"));
        run_native(&native_binary, &run, &native)?;
        if let Err(why) = run_web(&dist, &run, &web, firefox, gpu) {
            failures.push(why);
            continue;
        }
        failures.extend(compare(&run, &native, &web, &out));
        failures.extend(check_aria(&run, &web));
    }
    if failures.is_empty() {
        println!("web test: every shot as natively");
        Ok(())
    } else {
        Err(format!("web test:\n  {}", failures.join("\n  ")))
    }
}
