//! The web's files: the page has no file system, so what the app reads by path is kept in memory — the files
//! dropped on the canvas or pasted (kept under `/drop/<n>/<name>` and `/paste/<n>/<name>`, as C++ wrote them into
//! its MEMFS), and the assets the app embeds in its module ([`embed_assets`]: `include_bytes!`, as the web has no
//! directory beside the program) — and read through the platform's asset reader ([`set_asset_reader`]), which
//! every reading of the app's files tries ([`read_file`](crate::system::read_file): `load_image`, `read_asset`).
//!
//! [`set_asset_reader`]: crate::system::set_asset_reader

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

fn files() -> &'static Mutex<HashMap<String, Arc<[u8]>>> {
    static FILES: OnceLock<Mutex<HashMap<String, Arc<[u8]>>>> = OnceLock::new();
    FILES.get_or_init(Mutex::default)
}

/// The app's assets, embedded in its module: each `(path, bytes)` read as a file of that path (relative, as an
/// asset next to the program is named) from now on. Before the app runs (`main`'s first lines):
///
/// ```text
/// kompoz_platform::web::embed_assets(&[("images/logo.png", include_bytes!("../images/logo.png"))]);
/// ```
pub fn embed_assets(assets: &[(&str, &'static [u8])]) {
    let mut files = files().lock().unwrap_or_else(|e| e.into_inner());
    for (path, bytes) in assets {
        files.insert(path.trim_start_matches("./").to_string(), Arc::from(*bytes));
    }
}

/// A file the page handed over (`kind`: "drop", "paste"), kept under a path of its own: that path.
pub fn keep(kind: &str, name: &str, bytes: Vec<u8>) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    // (A name as the page gave it, without what would make it another directory.)
    let name: String = name.chars().map(|c| if c == '/' || c == '\\' { '_' } else { c }).collect();
    let name = if name.is_empty() || name == "." || name == ".." { "file".to_string() } else { name };
    let path = format!("/{kind}/{n}/{name}");
    files().lock().unwrap_or_else(|e| e.into_inner()).insert(path.clone(), Arc::from(bytes));
    path
}

/// A kept file's bytes (an asset's: its path as embedded, with or without a leading `./`).
pub fn read(path: &str) -> Option<Vec<u8>> {
    let files = files().lock().unwrap_or_else(|e| e.into_inner());
    files.get(path).or_else(|| files.get(path.trim_start_matches("./"))).map(|b| b.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_and_embedded_files_are_read_by_path() {
        let path = keep("drop", "a/b.txt", b"merhaba".to_vec());
        assert!(path.starts_with("/drop/") && path.ends_with("/a_b.txt"), "{path}");
        assert_eq!(read(&path).as_deref(), Some(&b"merhaba"[..]));
        embed_assets(&[("images/x.bin", b"\x01\x02")]);
        assert_eq!(read("images/x.bin"), Some(vec![1, 2]));
        assert_eq!(read("./images/x.bin"), Some(vec![1, 2]));
        assert_eq!(read("images/y.bin"), None);
    }
}
