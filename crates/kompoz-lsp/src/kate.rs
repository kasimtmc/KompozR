//! `kompoz-lsp --kate`: KDE's Kate has no extension store; a language comes to it as a highlighting definition
//! (KSyntaxHighlighting's XML, in its data directory) and a language server in its LSP client's settings
//! (`kate/lspclient/settings.json` in its configuration directory). This writes both: `kate/kompoz-rust.xml`,
//! built into the binary, and a `"kompozr"` server, this `kompoz-lsp --lsp`, for the files Kate highlights as
//! Kompoz Rust (`.rs` files) or Kotlin (`.kt` files); the settings' other servers and keys are kept (the C++
//! Kompoz's `"kompoz"` among them).

use crate::json::Json;
use std::path::{Path, PathBuf};

/// The highlighting definition: Kate's Rust, with Kotlin inside `kompoz! { … }`.
pub const SYNTAX: &str = include_str!("../kate/kompoz-rust.xml");

/// The server's key in Kate's settings. Not the C++ Kompoz's `"kompoz"` (both may be installed), and sorting
/// before Kate's default `"kotlin"` server, so that Kate, going through its servers by key, gives `.kt` files to
/// this one.
const SERVER: &str = "kompozr";

/// Where Kate (Qt's standard paths) looks: its highlighting definitions under the data directory, its settings
/// under the configuration one.
struct KatePaths {
    data: PathBuf,
    config: PathBuf,
}

/// An environment variable as a path, when set and not empty.
fn environment(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn kate_paths() -> Option<KatePaths> {
    if cfg!(windows) {
        let local = environment("LOCALAPPDATA")?;
        Some(KatePaths { data: local.clone(), config: local })
    } else if cfg!(target_os = "macos") {
        let home = environment("HOME")?;
        Some(KatePaths { data: home.join("Library/Application Support"), config: home.join("Library/Preferences") })
    } else {
        let home = environment("HOME");
        let data = environment("XDG_DATA_HOME").or_else(|| Some(home.as_ref()?.join(".local/share")))?;
        let config = environment("XDG_CONFIG_HOME").or_else(|| Some(home.as_ref()?.join(".config")))?;
        Some(KatePaths { data, config })
    }
}

/// Writes `text` to `file`, its directories made first.
fn write(file: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    }
    std::fs::write(file, text).map_err(|e| format!("can't write {}: {e}", file.display()))
}

/// Kate's LSP settings (`text`, the file's contents: `None` when there is none) with the `"kompozr"` server, for
/// the binary `exe`, set in its `"servers"`: everything else as it was, in its order. Settings that aren't a JSON
/// object (or whose `"servers"` isn't one) are an error, to be left as they are; an empty file is `{}`.
pub(crate) fn merge(text: Option<&str>, exe: &str) -> Result<String, String> {
    let mut config = match text {
        Some(text) if !text.trim().is_empty() => Json::parse(text).map_err(|e| format!("isn't JSON ({e})"))?,
        _ => Json::new_object(),
    };
    if !config.is_object() {
        return Err("isn't a JSON object".to_string());
    }
    if config.get("servers").is_some_and(|s| !s.is_object() && !s.is_null()) {
        return Err("has \"servers\" that isn't a JSON object".to_string());
    }
    let server = Json::object([
        ("command", Json::from(vec![Json::from(exe), Json::from("--lsp")])),
        // The .rs files Kate highlights with kate/kompoz-rust.xml, and .kt files in Kate's own Kotlin.
        ("highlightingModeRegex", Json::from("^Kompoz Rust$|^Kotlin$")),
        ("rootIndicationFileNames", Json::from(vec![Json::from("Cargo.lock"), Json::from("Cargo.toml")])),
        // The server tells a document's language by its URI; Kate's id would be its highlighting's name.
        ("documentLanguageId", Json::from(false)),
        ("url", Json::from("https://github.com/kasimtmc/KompozR")),
    ]);
    config.entry("servers").set(SERVER, server);
    Ok(config.pretty(2) + "\n")
}

/// Installs Kate's support: the highlighting definition where Kate reads it and the `"kompozr"` server, `exe`
/// (this binary) `--lsp`, in its LSP client's settings. Gives what to print, or what went wrong (with nothing
/// written when the settings can't be read as an object).
pub fn install(exe: &Path) -> Result<String, String> {
    let paths = kate_paths().ok_or("no home directory to find Kate's in")?;
    let syntax = paths.data.join("org.kde.syntax-highlighting").join("syntax").join("kompoz-rust.xml");
    let settings = paths.config.join("kate").join("lspclient").join("settings.json");

    // The settings as they are (the user's own servers in them).
    let text = match std::fs::read_to_string(&settings) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("can't read {}: {e}", settings.display())),
    };
    let exe = std::path::absolute(exe).map_err(|e| format!("can't find {}: {e}", exe.display()))?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let exe = exe.to_string_lossy();
    let merged = merge(text.as_deref(), &exe).map_err(|e| format!("{} {e}; left as it is", settings.display()))?;

    write(&syntax, SYNTAX)?;
    write(&settings, &merged)?;
    Ok(format!(
        "Kate's highlighting for Kompoz Rust (.rs files): {}\n\
         KompozR's language server, {exe} --lsp, for them and .kt files: {}\n\
         Restart Kate; it asks once whether to start the server.\n",
        syntax.display(),
        settings.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_settings_or_empty_ones_are_an_object() {
        for text in [None, Some(""), Some(" \n\t")] {
            let config = Json::parse(&merge(text, "/bin/kompoz-lsp").unwrap()).unwrap();
            let server = &config["servers"][SERVER];
            assert_eq!(server["command"][0].str(), "/bin/kompoz-lsp");
            assert_eq!(server["command"][1].str(), "--lsp");
            assert_eq!(server["highlightingModeRegex"].str(), "^Kompoz Rust$|^Kotlin$");
            assert_eq!(server["documentLanguageId"].as_bool(), Some(false));
            assert_eq!(server["rootIndicationFileNames"][1].str(), "Cargo.toml");
        }
    }

    #[test]
    fn the_rest_kept_in_its_order() {
        let text =
            r#"{"a": 1, "servers": {"mine": {"command": ["x"]}, "kompoz": {"command": ["kompozc"]}}, "z": [true]}"#;
        let merged = merge(Some(text), "/bin/kompoz-lsp").unwrap();
        assert!(merged.ends_with("}\n"));
        assert!(merged.contains("\n  \"a\": 1,\n"), "2-space indentation: {merged}");
        let config = Json::parse(&merged).unwrap();
        let keys: Vec<&str> = config.members().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["a", "servers", "z"]);
        let servers: Vec<&str> = config["servers"].members().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(servers, ["mine", "kompoz", SERVER]);
        assert_eq!(config["servers"]["kompoz"]["command"][0].str(), "kompozc");
        // Again: the server replaced in its place.
        let again = Json::parse(&merge(Some(&merged), "/other").unwrap()).unwrap();
        let servers: Vec<&str> = again["servers"].members().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(servers, ["mine", "kompoz", SERVER]);
        assert_eq!(again["servers"][SERVER]["command"][0].str(), "/other");
    }

    #[test]
    fn not_an_object_is_an_error() {
        for text in ["{ not JSON", "[1, 2]", "\"text\"", r#"{"servers": [1]}"#] {
            assert!(merge(Some(text), "/bin/kompoz-lsp").is_err(), "{text}");
        }
    }
}
