//! Golden tests of the generator: each `tests/golden/*.kt` translated (through the umbrella crate's paths, as an
//! app's `kompoz!` is; a `*.file.kt` as `kompoz_file!` translates a file, its calls keyed by their positions) and
//! compared with the `*.rs` next to it — the Rust, formatted by the installed `rustfmt` when there is one (else as
//! the tokens print), then the diagnostics as comments (`// error 3:5: …`).
//!
//! `UPDATE_GOLDEN=1 cargo test -p kompoz-kotlin --test golden` writes the expected files from what is generated (read
//! the diff before committing it).

use kompoz_kotlin::{OneSpan, Options, Paths, Pos, Severity, translate};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The Rust `text` becomes, as the golden files hold it.
fn generated(text: &str, file: bool) -> String {
    let opts = Options {
        paths: Paths::umbrella(&proc_macro2::Ident::new("kompoz", proc_macro2::Span::call_site())),
        keyed_calls: file,
        ..Default::default()
    };
    let t = translate(text, Pos::start(1, 1), &OneSpan::default(), &opts);
    let mut out = format(&t.tokens.to_string());
    for d in &t.diagnostics {
        let kind = if d.severity == Severity::Error { "error" } else { "warning" };
        out += &format!("// {kind} {}: {}\n", d.loc.start, d.message.replace('\n', "\n//   "));
    }
    out
}

/// `rust` formatted by rustfmt (edition 2024, the workspace's width); as it is when rustfmt isn't there or can't.
fn format(rust: &str) -> String {
    let child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout", "--config", "max_width=120,use_small_heuristics=Max"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return format!("{rust}\n") };
    child.stdin.take().expect("its input").write_all(rust.as_bytes()).expect("rustfmt reads");
    match child.wait_with_output() {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => format!("{rust}\n"),
    }
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

#[test]
fn goldens() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut files: Vec<PathBuf> = std::fs::read_dir(golden_dir())
        .expect("tests/golden")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "kt"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no golden files");
    let mut failed = Vec::new();
    for kt in &files {
        let text = std::fs::read_to_string(kt).expect("a .kt file");
        let file = kt.to_string_lossy().ends_with(".file.kt");
        let got = generated(&text, file);
        let rs = kt.with_extension("rs");
        if update {
            std::fs::write(&rs, &got).expect("written");
            continue;
        }
        let expected = std::fs::read_to_string(&rs).unwrap_or_default();
        if expected != got {
            let name = kt.file_name().unwrap().to_string_lossy().into_owned();
            eprintln!("--- {name}: generated differs from {}:\n{}", rs.display(), diff(&expected, &got));
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "golden files differ: {failed:?} (UPDATE_GOLDEN=1 writes them)");
}

/// The lines that differ, `-` expected, `+` generated (the first few).
fn diff(expected: &str, got: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), got.lines().collect());
    let mut out = String::new();
    let mut shown = 0;
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i), b.get(i));
        if x != y {
            if let Some(x) = x {
                out += &format!("{:4} - {x}\n", i + 1);
            }
            if let Some(y) = y {
                out += &format!("{:4} + {y}\n", i + 1);
            }
            shown += 1;
            if shown == 12 {
                out += "     …\n";
                break;
            }
        }
    }
    out
}
