//! Kate's Kompoz Rust (`kate/kompoz-rust.xml`) and `kompoz-lsp --kate`.
//!
//! The definition, as KSyntaxHighlighting reads it (its command line highlighter, `ksyntaxhighlighter6`: the
//! check is skipped without it), on `tests/data/kate_sample.rs`: its Rust lines highlighted as Kate's Rust
//! highlights them alone, and the Kotlin's tokens in the styles below (read back from a theme that gives each
//! style a color of its own). `kompoz-lsp --kate`: the definition written where Kate reads it, and the server
//! added to Kate's settings, their other servers and keys kept.

use kompoz_lsp::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFINITION: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/kate/kompoz-rust.xml");
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/kate_sample.rs");

/// The sample's Kotlin lines (1-based): the kompoz! { … } block's inside, and the two lines whose kompoz!( … )
/// and kompoz! [ … ] are Kotlin in a Rust line.
fn is_kotlin(line: usize) -> bool {
    (15..=39).contains(&line) || line == 64 || line == 66
}

/// Tokens of the Kotlin (and of the Rust around it in its lines): (line, text, style); a piece of the line
/// starting with the text, its first one.
const EXPECTED: &[(usize, &str, &str)] = &[
    (15, "data", "Keyword"),
    (15, "class", "Keyword"),
    (15, "Item", "DataType"),
    (15, "val", "Keyword"),
    (15, "id", "Variable"),
    (15, "Int", "DataType"),
    (15, "false", "Constant"),
    (17, "@Composable", "Attribute"),
    (18, "fun", "Keyword"),
    (18, "ItemRow", "Function"),
    (18, "Boolean", "DataType"),
    (18, "->", "Operator"),
    (19, "Row", "Function"),
    (19, "fillMaxWidth", "Function"),
    (19, "!", "Operator"),
    (19, "{", "Operator"),
    (20, "Checkbox", "Function"),
    (21, "// a comment in the Kotlin", "Comment"),
    (22, "\"", "String"),
    (22, "${", "SpecialChar"),
    (22, "$item", "SpecialChar"),
    (22, " #", "String"),
    (22, "MaterialTheme", "DataType"),
    (25, "/* a block comment", "Comment"),
    (26, "private", "Keyword"),
    (26, "Spacing", "Variable"),
    (26, "8", "DecVal"),
    (28, "object", "Keyword"),
    (28, "Routes", "DataType"),
    (29, "const", "Keyword"),
    (29, "Home", "Variable"),
    (29, "\"home\"", "String"),
    (32, "enum", "Keyword"),
    (32, "Mode", "DataType"),
    (32, "Light", "Function"),
    (34, "sealed", "Keyword"),
    (34, "Screen", "DataType"),
    (34, "route", "Variable"),
    (36, "Braces", "Function"),
    (36, "\"a } in a string\"", "String"),
    (36, "'}'", "Char"),
    (37, "${", "SpecialChar"),
    (37, "if", "ControlFlow"),
    (37, "\"}\"", "String"),
    (37, "else", "ControlFlow"),
    (38, "\"\"\"a raw } string", "String"),
    (38, "${", "SpecialChar"),
    (39, "}", "Operator"),
    (64, "kompoz!", "Preprocessor"),
    (64, "Text", "Function"),
    (64, "\"a ) in a string\"", "String"),
    // Rust again after the closing parenthesis (Kotlin would have it a function).
    (64, "Some", "Constant"),
    (66, "[", "Operator"),
    (66, "Text", "Function"),
    (66, "\"one\"", "String"),
];

/// KSyntaxHighlighting's default styles, in the order the theme gives them their colors (#000001, #000002, …).
const STYLES: &[&str] = &[
    "Normal",
    "Keyword",
    "Function",
    "Variable",
    "ControlFlow",
    "Operator",
    "BuiltIn",
    "Extension",
    "Preprocessor",
    "Attribute",
    "Char",
    "SpecialChar",
    "String",
    "VerbatimString",
    "SpecialString",
    "Import",
    "DataType",
    "DecVal",
    "BaseN",
    "Float",
    "Constant",
    "Comment",
    "Documentation",
    "Annotation",
    "CommentVar",
    "RegionMarker",
    "Information",
    "Warning",
    "Alert",
    "Others",
    "Error",
];

/// A directory of this run's own under Cargo's temporary directory for tests, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `ksyntaxhighlighter6`, when it's on `PATH`.
fn highlighter() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join("ksyntaxhighlighter6")).find(|tool| tool.is_file())
}

/// HTML's text: its entities decoded (those the highlighter writes).
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else { break };
        let entity = &rest[1..end];
        let decoded = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A line highlighted: its pieces as (style, text).
type Pieces = Vec<(String, String)>;

/// `text` highlighted as the definition `syntax` (by name) with the test theme from `data_home`: each line's
/// pieces.
fn highlighted(tool: &Path, syntax: &str, text: &str, data_home: &Path) -> Vec<Pieces> {
    let file = data_home.join(format!("{}.rs", syntax.replace(' ', "-")));
    std::fs::write(&file, text).unwrap();
    let out = Command::new(tool)
        .args(["-s", syntax, "-t", "Kompoz Test", "-f", "html"])
        .arg(&file)
        .env("XDG_DATA_HOME", data_home)
        .env("XDG_CONFIG_HOME", data_home.join("config"))
        .env("XDG_CACHE_HOME", data_home.join("cache"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let html = String::from_utf8(out.stdout).unwrap();
    let start = html.find("<pre>").expect("no <pre>") + "<pre>".len();
    let end = html.rfind("</pre>").expect("no </pre>");
    let pre = html[start..end].strip_prefix('\n').unwrap_or(&html[start..end]);
    let style_of = |style: &str| {
        let color = style.split(';').find_map(|p| p.trim().strip_prefix("color:#"));
        match color.and_then(|c| usize::from_str_radix(c, 16).ok()) {
            Some(n) => STYLES.get(n.wrapping_sub(1)).copied().unwrap_or("?").to_string(),
            None => "Normal".to_string(),
        }
    };
    pre.split('\n')
        .map(|line| {
            let mut pieces = Pieces::new();
            let mut rest = line;
            while !rest.is_empty() {
                let at = rest.find("<span style=\"").unwrap_or(rest.len());
                if at > 0 {
                    pieces.push(("Normal".to_string(), unescape(&rest[..at])));
                }
                rest = &rest[at..];
                if rest.is_empty() {
                    break;
                }
                let open = rest.find("\">").unwrap();
                let style = &rest["<span style=\"".len()..open];
                let close = rest.find("</span>").unwrap();
                pieces.push((style_of(style), unescape(&rest[open + 2..close])));
                rest = &rest[close + "</span>".len()..];
            }
            pieces
        })
        .collect()
}

#[test]
fn highlighting() {
    let Some(tool) = highlighter() else {
        println!("skipped: no ksyntaxhighlighter6");
        return;
    };
    let data_home = TempDir::new("kate-highlighting");
    let base = data_home.0.join("org.kde.syntax-highlighting");
    std::fs::create_dir_all(base.join("syntax")).unwrap();
    std::fs::create_dir_all(base.join("themes")).unwrap();
    std::fs::copy(DEFINITION, base.join("syntax").join("kompoz-rust.xml")).unwrap();
    let mut styles = Json::new_object();
    for (i, style) in STYLES.iter().enumerate() {
        styles.set(style, Json::object([("text-color", Json::from(format!("#{:06x}", i + 1)))]));
    }
    let theme = Json::object([
        ("metadata", Json::object([("name", Json::from("Kompoz Test")), ("revision", Json::from(1))])),
        ("text-styles", styles),
        ("editor-colors", Json::object([("BackgroundColor", Json::from("#ffffff"))])),
    ]);
    std::fs::write(base.join("themes").join("kompoz-test.theme"), theme.pretty(2)).unwrap();

    let sample = std::fs::read_to_string(SAMPLE).unwrap();
    let lines: Vec<&str> = sample.split('\n').collect();
    let blanked: Vec<&str> = lines.iter().enumerate().map(|(i, l)| if is_kotlin(i + 1) { "" } else { l }).collect();
    let mine = highlighted(&tool, "Kompoz Rust", &sample, &data_home.0);
    let as_rust = highlighted(&tool, "Rust", &blanked.join("\n"), &data_home.0);
    assert_eq!(mine.len(), lines.len(), "a line each");

    let mut failures = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !is_kotlin(i + 1) && mine[i] != as_rust[i] {
            failures.push(format!(
                "line {}, Rust: {line}\n  got      {:?}\n  expected {:?}",
                i + 1,
                mine[i],
                as_rust[i]
            ));
        }
    }
    for &(line, text, style) in EXPECTED {
        let got = mine[line - 1].iter().find(|(_, t)| t.starts_with(text)).map(|(s, _)| s.as_str());
        if got != Some(style) {
            failures.push(format!("line {line}, Kotlin: {text:?} is {got:?}, not {style}  ({:?})", mine[line - 1]));
        }
    }
    assert!(failures.is_empty(), "{} lines, {} Kotlin tokens:\n{}", lines.len(), EXPECTED.len(), failures.join("\n"));
}

#[test]
fn install() {
    let home = TempDir::new("kate-install");
    let data = home.0.join("data");
    let config = home.0.join("config");
    let settings = config.join("kate").join("lspclient").join("settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let mine = Json::object([
        ("command", Json::from(vec![Json::from("my-lsp")])),
        ("highlightingModeRegex", Json::from("^Mine$")),
    ]);
    let kompoz = Json::object([
        ("command", Json::from(vec![Json::from("/usr/bin/kompozc"), Json::from("--lsp")])),
        ("highlightingModeRegex", Json::from("^Kompoz C\\+\\+$")),
    ]);
    let before = Json::object([
        ("servers", Json::object([("mine", mine.clone()), ("kompoz", kompoz.clone())])),
        ("other", Json::from(1)),
    ]);
    std::fs::write(&settings, before.pretty(2)).unwrap();

    let run = || {
        Command::new(env!("CARGO_BIN_EXE_kompoz-lsp"))
            .arg("--kate")
            .env("HOME", &home.0)
            .env("XDG_DATA_HOME", &data)
            .env("XDG_CONFIG_HOME", &config)
            .output()
            .unwrap()
    };
    let done = run();
    assert!(done.status.success(), "kompoz-lsp --kate: {}", String::from_utf8_lossy(&done.stderr));
    assert!(String::from_utf8_lossy(&done.stdout).contains("Restart Kate"));

    let written = data.join("org.kde.syntax-highlighting").join("syntax").join("kompoz-rust.xml");
    assert_eq!(
        std::fs::read(&written).unwrap(),
        std::fs::read(DEFINITION).unwrap(),
        "the definition, as it is in kate/"
    );

    let text = std::fs::read_to_string(&settings).unwrap();
    assert!(text.ends_with("}\n"));
    let after = Json::parse(&text).unwrap();
    assert_eq!(after["servers"]["mine"], mine, "the user's server kept");
    assert_eq!(after["servers"]["kompoz"], kompoz, "the C++ Kompoz's server kept");
    assert_eq!(after["other"], Json::from(1), "the user's settings kept");
    let server = &after["servers"]["kompozr"];
    let exe = std::fs::canonicalize(env!("CARGO_BIN_EXE_kompoz-lsp")).unwrap();
    assert_eq!(server["command"][0].str(), exe.to_str().unwrap(), "by its path");
    assert_eq!(server["command"][1].str(), "--lsp");
    assert_eq!(server["command"].array().len(), 2);
    assert_eq!(server["highlightingModeRegex"].str(), "^Kompoz Rust$|^Kotlin$");
    assert_eq!(server["documentLanguageId"].as_bool(), Some(false), "no language id");

    std::fs::write(&settings, "{ not JSON").unwrap();
    let broken = run();
    assert!(!broken.status.success(), "settings that aren't JSON: an error");
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), "{ not JSON", "and left as they are");
    assert!(String::from_utf8_lossy(&broken.stderr).starts_with("kompoz-lsp: "));
}
