//! `cargo xtask android …`: the showcase on Android (examples/showcase/android, its Gradle project: KompozR's Kotlin
//! side and the showcase's library built by cargo-ndk), built, installed and run by a script on a device or an
//! emulator through adb:
//!
//! ```text
//! cargo xtask android build [--abi <abi>]… [--release]
//!     the APK (debug, or release: R8, unsigned), its library for each ABI (arm64-v8a and x86_64; one with --abi)
//! cargo xtask android install [--serial <device>] [--abi <abi>]… [--release]
//!     built, then installed (`adb install -r`)
//! cargo xtask android shots --out <dir> --script "<script>" [--serial <device>] [--option NAME=VALUE]…
//!                           [--headless --abi <abi>] [--timeout <seconds>]
//!     the installed app run by a script (KOMPOZ_SCRIPT and the options as files in its files: `run-as`, so a debug
//!     build), until the script quits; its shots pulled into <dir>. With --headless the showcase's own program for
//!     the device's ABI instead, pushed to /data/local/tmp and run there on the headless platform (the same GPU, the
//!     same fonts): its shots the window's, pixel for pixel, as the Windows test compares them.
//! cargo xtask android compare <dir> <dir>
//!     the shots of two runs compared (the same pictures, byte for byte: one encoder)
//! ```
//!
//! Gradle runs offline without a daemon (`--offline --no-daemon`: everything it needs is in ~/.gradle). The SDK is
//! `ANDROID_HOME`'s (else ~/Android/Sdk), adb its platform-tools' one.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The app's package and activity.
const PACKAGE: &str = "dev.kompozr.showcase";
const ACTIVITY: &str = "dev.kompozr.showcase/.MainActivity";
/// Where the headless program runs on the device.
const DEVICE_DIR: &str = "/data/local/tmp/kompozr";
/// The app's minSdk (cargo-ndk's platform).
const MIN_SDK: &str = "29";

pub fn run(workspace: &Path, args: &[&str]) -> Result<(), String> {
    let (task, rest) = args.split_first().ok_or(USAGE)?;
    let options = Options::parse(rest)?;
    match *task {
        "build" => build(workspace, &options).map(|_| ()),
        "install" => {
            let apk = build(workspace, &options)?;
            adb(&options, &["install", "-r", &apk.to_string_lossy()])
        }
        "shots" => shots(workspace, &options),
        "compare" => match options.free.as_slice() {
            [a, b] => compare(Path::new(a), Path::new(b)),
            _ => Err(USAGE.into()),
        },
        _ => Err(USAGE.into()),
    }
}

const USAGE: &str = "usage: cargo xtask android build|install [--abi <abi>]… [--release] [--serial <device>] | \
                     cargo xtask android shots --out <dir> --script <script> [--serial <device>] [--option N=V]… \
                     [--headless --abi <abi>] [--timeout <s>] | cargo xtask android compare <dir> <dir>";

#[derive(Default)]
struct Options {
    abis: Vec<String>,
    release: bool,
    serial: Option<String>,
    out: Option<PathBuf>,
    script: Option<String>,
    options: Vec<(String, String)>,
    headless: bool,
    timeout: Duration,
    free: Vec<String>,
}

impl Options {
    fn parse(args: &[&str]) -> Result<Options, String> {
        let mut o = Options { timeout: Duration::from_secs(600), ..Options::default() };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            let mut value = || it.next().map(|v| v.to_string()).ok_or(format!("{arg}: a value is missing"));
            match arg {
                "--abi" => o.abis.push(value()?),
                "--release" => o.release = true,
                "--serial" => o.serial = Some(value()?),
                "--out" => o.out = Some(PathBuf::from(value()?)),
                "--script" => o.script = Some(value()?),
                "--option" => {
                    let v = value()?;
                    let (name, value) = v.split_once('=').ok_or(format!("--option {v}: not NAME=VALUE"))?;
                    o.options.push((name.to_string(), value.to_string()));
                }
                "--headless" => o.headless = true,
                "--timeout" => {
                    let v = value()?;
                    o.timeout = Duration::from_secs(v.parse().map_err(|_| format!("--timeout {v}: not seconds"))?);
                }
                other if other.starts_with("--") => return Err(format!("android: unknown option {other}")),
                other => o.free.push(other.to_string()),
            }
        }
        Ok(o)
    }
}

/// The Android SDK.
fn sdk() -> PathBuf {
    std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Android/Sdk"))
}

/// The Gradle project.
fn project(workspace: &Path) -> PathBuf {
    workspace.join("examples/showcase/android")
}

/// The APK built (debug, or release), its path.
fn build(workspace: &Path, o: &Options) -> Result<PathBuf, String> {
    let project = project(workspace);
    let gradlew = project.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    let mut command = Command::new(&gradlew);
    command.current_dir(&project).args(["--offline", "--no-daemon"]);
    command.arg(if o.release { "assembleRelease" } else { "assembleDebug" });
    if !o.abis.is_empty() {
        command.arg(format!("-Pkompozr.abis={}", o.abis.join(",")));
    }
    let status = command.status().map_err(|e| format!("{}: {e}", gradlew.display()))?;
    if !status.success() {
        return Err(format!("gradle: {status}"));
    }
    let apk = if o.release {
        project.join("app/build/outputs/apk/release/app-release-unsigned.apk")
    } else {
        project.join("app/build/outputs/apk/debug/app-debug.apk")
    };
    println!("{}", apk.display());
    Ok(apk)
}

/// adb (the SDK's) with the device chosen.
fn adb_command(o: &Options) -> Command {
    let mut command = Command::new(sdk().join("platform-tools/adb"));
    if let Some(serial) = &o.serial {
        command.args(["-s", serial]);
    }
    command
}

/// adb with `args`, its output shown; an error if it fails.
fn adb(o: &Options, args: &[&str]) -> Result<(), String> {
    let status = adb_command(o).args(args).status().map_err(|e| format!("adb: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("adb {}: {status}", args.join(" "))) }
}

/// adb with `args` and `input` on its standard input, its output.
fn adb_output(o: &Options, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut child = adb_command(o)
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("adb: {e}"))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        use std::io::Write;
        stdin.write_all(input).map_err(|e| format!("adb: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("adb: {e}"))?;
    if out.status.success() { Ok(out.stdout) } else { Err(format!("adb {}: {}", args.join(" "), out.status)) }
}

/// A shell word quoted for `sh` ('…', a quote in it closed and escaped).
fn quoted(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The option file's name of `name` (kompoz-platform's android::configuration::option_file).
fn option_file(name: &str) -> String {
    name.chars().map(|c| if c == '_' { '-' } else { c.to_ascii_lowercase() }).collect()
}

/// `cargo xtask android shots`.
fn shots(workspace: &Path, o: &Options) -> Result<(), String> {
    let out = o.out.clone().ok_or("android shots: --out <dir> is missing")?;
    let script = o.script.clone().ok_or("android shots: --script is missing")?;
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let log = if o.headless { headless_shots(workspace, o, &script, &out)? } else { app_shots(o, &script, &out)? };
    std::fs::write(out.join("log.txt"), &log).map_err(|e| format!("{}: {e}", out.display()))?;
    let shots = pngs(&out);
    println!("{} shots in {}", shots.len(), out.display());
    if shots.is_empty() {
        return Err(format!("no shots (the run's log: {})", out.join("log.txt").display()));
    }
    Ok(())
}

/// The app run by `script`: its options written into its files, started, waited for until its script ends it;
/// its shots pulled; its log (logcat's "kompoz" lines).
fn app_shots(o: &Options, script: &str, out: &Path) -> Result<String, String> {
    let files = |command: &str| -> Result<Vec<u8>, String> {
        adb_output(o, &["shell", "run-as", PACKAGE, "sh", "-c", &quoted(command)], None)
    };
    adb(o, &["shell", "am", "force-stop", PACKAGE])?;
    files("rm -f files/*.png files/kompoz-*")?;
    let mut options = o.options.clone();
    options.push(("KOMPOZ_SCRIPT".into(), script.to_string()));
    for (name, value) in &options {
        let path = format!("files/{}", option_file(name));
        // (exec-in runs its command without a shell's quoting: the redirection one argument)
        let write = format!("cat > {path}");
        adb_output(o, &["exec-in", "run-as", PACKAGE, "sh", "-c", &write], Some(value.as_bytes()))?;
    }
    adb(o, &["logcat", "-c"])?;
    adb(o, &["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    let started = Instant::now();
    let log = loop {
        let log = String::from_utf8_lossy(&adb_output(o, &["logcat", "-d", "-s", "kompoz:*"], None)?).into_owned();
        if log.contains("kompoz: the app ended") || log.contains("kompoz: a panic in") {
            break log;
        }
        if started.elapsed() > o.timeout {
            let _ = files("rm -f files/kompoz-*");
            return Err(format!("the app didn't end in {:?}; its log:\n{log}", o.timeout));
        }
        std::thread::sleep(Duration::from_secs(2));
    };
    // (Its next start an ordinary one.)
    files("rm -f files/kompoz-*")?;
    let listed = String::from_utf8_lossy(&files("ls files")?).into_owned();
    for name in listed.split_whitespace().filter(|n| n.ends_with(".png")) {
        let png = adb_output(o, &["exec-out", "run-as", PACKAGE, "cat", &format!("files/{name}")], None)?;
        std::fs::write(out.join(name), png).map_err(|e| format!("{name}: {e}"))?;
    }
    if log.contains("kompoz: a panic in") {
        return Err(format!("the app panicked; its log:\n{log}"));
    }
    Ok(log)
}

/// The showcase's program for the device's ABI (cargo-ndk, the app's profile) run headless on the device by
/// `script` with the options as its environment; its shots pulled; its output.
fn headless_shots(workspace: &Path, o: &Options, script: &str, out: &Path) -> Result<String, String> {
    let [abi] = o.abis.as_slice() else { return Err("android shots --headless: one --abi".into()) };
    let triple = match abi.as_str() {
        "arm64-v8a" => "aarch64-linux-android",
        "x86_64" => "x86_64-linux-android",
        other => return Err(format!("android shots: no Rust target for the ABI {other}")),
    };
    let ndk = sdk().join("ndk/29.0.14206865");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(workspace)
        .env("ANDROID_NDK_HOME", &ndk)
        .args(["ndk", "-t", abi, "--platform", MIN_SDK, "build", "-p", "showcase", "--bin", "showcase"])
        .args(["--profile", "android"])
        .status()
        .map_err(|e| format!("cargo ndk: {e}"))?;
    if !status.success() {
        return Err(format!("cargo ndk: {status}"));
    }
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| workspace.join("target"));
    let program = target.join(triple).join("android/showcase");
    adb(o, &["shell", "rm", "-rf", DEVICE_DIR])?;
    adb(o, &["shell", "mkdir", "-p", DEVICE_DIR])?;
    adb(o, &["push", &program.to_string_lossy(), &format!("{DEVICE_DIR}/showcase")])?;
    let mut run = format!("cd {DEVICE_DIR} && HOME={DEVICE_DIR} KOMPOZ_HEADLESS=1 KOMPOZ_SCRIPT={} ", quoted(script));
    for (name, value) in &o.options {
        run += &format!("{name}={} ", quoted(value));
    }
    run += "./showcase 2>&1";
    let output = adb_output(o, &["shell", &run], None)?;
    let listed = String::from_utf8_lossy(&adb_output(o, &["shell", "ls", DEVICE_DIR], None)?).into_owned();
    for name in listed.split_whitespace().filter(|n| n.ends_with(".png")) {
        let png = adb_output(o, &["exec-out", "cat", &format!("{DEVICE_DIR}/{name}")], None)?;
        std::fs::write(out.join(name), png).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// The PNGs in `dir`, by name.
fn pngs(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|n| n.ends_with(".png")).collect())
        .unwrap_or_default();
    names.sort();
    names
}

/// The shots of two runs: the same names, the same pictures (one encoder: the same pixels are the same bytes).
fn compare(a: &Path, b: &Path) -> Result<(), String> {
    let (left, right) = (pngs(a), pngs(b));
    if left.is_empty() || left != right {
        return Err(format!("the runs' shots differ: {left:?} and {right:?}"));
    }
    let differ: Vec<&String> =
        left.iter().filter(|name| std::fs::read(a.join(name)).ok() != std::fs::read(b.join(name)).ok()).collect();
    if !differ.is_empty() {
        return Err(format!("these shots differ: {differ:?}"));
    }
    println!("{} shots the same: {left:?}", left.len());
    Ok(())
}
