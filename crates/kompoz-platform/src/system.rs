//! What every platform needs of the system without a platform library (C++'s common/system): the log, the app's
//! files (as they are, the platform's assets, or next to the program), the program's directory, the app's cache
//! directory, the language the environment asks for. A platform sets what is its own (its log sink, its assets,
//! its cache directory).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

/// Where log lines go (stderr until a platform says).
pub type LogSink = fn(&str);

static SINK: RwLock<Option<LogSink>> = RwLock::new(None);

/// A line into the platform's log (stderr until it says where: [`set_log_sink`]). Any thread.
pub fn log(message: &str) {
    match *SINK.read().unwrap_or_else(|e| e.into_inner()) {
        Some(sink) => sink(message),
        None => eprintln!("{message}"),
    }
}

/// Where log lines go from now on (none: stderr).
pub fn set_log_sink(sink: Option<LogSink>) {
    *SINK.write().unwrap_or_else(|e| e.into_inner()) = sink;
}

/// The directory the program's binary is in (C++'s `programDirectory`); none where it can't be told.
pub fn program_directory() -> Option<&'static Path> {
    static DIRECTORY: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            let exe = std::env::current_exe().ok()?;
            let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
            exe.parent().map(Path::to_path_buf)
        })
        .as_deref()
}

/// What reads the platform's own assets (Android's APK), if it has them.
pub type AssetReader = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

static ASSETS: RwLock<Option<AssetReader>> = RwLock::new(None);
static CACHE: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

/// The platform's assets from now on (C++'s `setAssetReader`).
pub fn set_asset_reader(reader: Option<AssetReader>) {
    *ASSETS.write().unwrap_or_else(|e| e.into_inner()) = reader;
}

/// A file's bytes (C++'s `readFile`): the path as it is, or the platform's assets', or (a relative one) next to the
/// program; none if none of them has it (or it is empty).
pub fn read_file(path: &str) -> Option<Vec<u8>> {
    if path.is_empty() {
        return None;
    }
    let plain = |p: &Path| std::fs::read(p).ok().filter(|b| !b.is_empty());
    if let Some(bytes) = plain(Path::new(path)) {
        return Some(bytes);
    }
    if let Some(reader) = ASSETS.read().unwrap_or_else(|e| e.into_inner()).as_ref()
        && let Some(bytes) = reader(path).filter(|b| !b.is_empty())
    {
        return Some(bytes);
    }
    let absolute = path.starts_with('/') || path.starts_with('\\') || path.as_bytes().get(1) == Some(&b':');
    if absolute {
        return None;
    }
    program_directory().and_then(|dir| plain(&dir.join(path)))
}

/// A name that makes a directory of `app_name`: its letters, digits, '-', '_', '.' and anything beyond ASCII,
/// the rest '_'; never empty, never hidden.
fn directory_name(app_name: &str) -> String {
    let mut name: String = app_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') || !c.is_ascii() { c } else { '_' })
        .collect();
    if name.is_empty() || name.starts_with('.') {
        name.insert_str(0, "app");
    }
    name
}

/// The app's own cache directory (C++'s `appCacheDirectory`): the platform's (set with [`set_cache_directory`]),
/// else `$XDG_CACHE_HOME/kompoz/<app>` (`~/.cache/kompoz/<app>` without it) on Linux, the system's elsewhere; none
/// where there is none. Not made: what writes there makes it.
pub fn app_cache_directory(app_name: &str) -> Option<PathBuf> {
    if let Some(set) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return set.clone();
    }
    let name = directory_name(app_name);
    let var = |v: &str| kompoz_runtime::environment::var_os(v).filter(|s| !s.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return var("HOME").map(|home| home.join("Library").join("Caches").join(name));
    }
    if cfg!(windows) {
        return var("LOCALAPPDATA").map(|local| local.join("Kompoz").join(name));
    }
    if let Some(xdg) = var("XDG_CACHE_HOME") {
        return Some(xdg.join("kompoz").join(name));
    }
    var("HOME").map(|home| home.join(".cache").join("kompoz").join(name))
}

/// The platform's own cache directory (Android's), or none where there is none (C++'s `setCacheDirectory`).
pub fn set_cache_directory(directory: Option<PathBuf>) {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some(directory);
}

/// The language the environment asks for (C++'s `preferredLanguage`: `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`;
/// "tr_TR.UTF-8" is "tr"), else the one the system says the user prefers first
/// ([`system_languages`](kompoz_runtime::environment::system_languages): Windows' `GetUserDefaultLocaleName`, what
/// the Android, web and macOS platforms say: "tr-TR" is "tr"); none if neither says.
pub fn preferred_language() -> Option<String> {
    let language_of =
        |tag: &str| -> String { tag.chars().take_while(|c| !matches!(c, '_' | '.' | '@' | '-')).collect() };
    for name in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = kompoz_runtime::environment::var(name) else { continue };
        let language = language_of(value.split(':').next().unwrap_or(""));
        if !language.is_empty() && language != "C" && language != "POSIX" {
            return Some(language);
        }
    }
    let system = kompoz_runtime::environment::system_languages();
    system.first().map(|tag| language_of(tag)).filter(|l| !l.is_empty() && l != "und")
}

/// A dropped file's path as a `file:` URI (C++'s `fileUri`, a `text/uri-list`'s line): kompoz-ui's.
pub use kompoz_ui::clipboard::file_uri;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_names_make_directories() {
        assert_eq!(directory_name("Kompoz Sayaç"), "Kompoz_Sayaç");
        assert_eq!(directory_name(".hidden"), "app.hidden");
        assert_eq!(directory_name(""), "app");
        assert_eq!(directory_name("a/b:c"), "a_b_c");
    }

    #[test]
    fn dropped_paths_are_file_uris() {
        if cfg!(not(windows)) {
            // (A rooted path without a drive is the current drive's on Windows.)
            assert_eq!(file_uri("/home/kasim/Görüntü 1.png"), "file:///home/kasim/G%C3%B6r%C3%BCnt%C3%BC%201.png");
        }
        assert_eq!(file_uri("C:\\Users\\a.txt"), "file:///C:/Users/a.txt");
    }
}
