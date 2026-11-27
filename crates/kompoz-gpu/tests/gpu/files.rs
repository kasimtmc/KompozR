//! The files the tests read and write: the system's natively. On the web, which has none, a memory of the run's
//! own: tests/golden's pictures and tests/fonts' fonts built in (so the same comparisons run), what the tests write
//! kept there for them to read back, and the pictures written beside the goldens (`.actual.png`, `.diff.png`, or the
//! goldens themselves with KOMPOZ_GOLDEN_UPDATE=1) also sent to the page's runner, which saves them.

use std::path::{Path, PathBuf};

/// A file of the run's own for `name` (in the system's temporary directory, named for this process).
pub fn temp_path(name: &str) -> PathBuf {
    #[cfg(not(target_family = "wasm"))]
    {
        std::env::temp_dir().join(format!("{name}_{}", std::process::id()))
    }
    #[cfg(target_family = "wasm")]
    {
        Path::new("/tmp").join(name)
    }
}

/// Where the crate's own files are (tests/golden, tests/fonts): its directory natively; on the web the same path,
/// which names the files built in.
pub fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(not(target_family = "wasm"))]
pub fn read(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[cfg(not(target_family = "wasm"))]
pub fn write(path: &Path, bytes: &[u8]) -> bool {
    std::fs::write(path, bytes).is_ok()
}

#[cfg(not(target_family = "wasm"))]
pub fn create_dir(path: &Path) {
    let _ = std::fs::create_dir_all(path);
}

#[cfg(not(target_family = "wasm"))]
pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(not(target_family = "wasm"))]
pub fn remove_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[cfg(target_family = "wasm")]
pub use web::{create_dir, read, remove, remove_dir, write};

#[cfg(target_family = "wasm")]
mod web {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    /// tests/golden's pictures and tests/fonts' fonts, by their names there.
    macro_rules! built_in {
        ($($dir:literal / $name:literal),* $(,)?) => {
            &[$(
                (concat!($dir, "/", $name), include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/", $dir, "/", $name))),
            )*]
        };
    }

    static BUILT_IN: &[(&str, &[u8])] = built_in![
        "golden" / "blend-modes.png",
        "golden" / "density.png",
        "golden" / "effects.png",
        "golden" / "gradients.png",
        "golden" / "graphics-layers.png",
        "golden" / "images.png",
        "golden" / "layers.png",
        "golden" / "lines.png",
        "golden" / "outline-layers.png",
        "golden" / "paths.png",
        "golden" / "rects.png",
        "golden" / "shadows.png",
        "golden" / "stretch.png",
        "golden" / "text.png",
        "golden" / "transforms.png",
        "fonts" / "Regular.ttf",
        "fonts" / "Medium.ttf",
        "fonts" / "Bold.ttf",
    ];

    thread_local! {
        /// What the run wrote (none: removed, a built-in file too).
        static WRITTEN: RefCell<HashMap<PathBuf, Option<Vec<u8>>>> = RefCell::new(HashMap::new());
    }

    /// A built-in file's name (`golden/rects.png`) if `path` is one's.
    fn built_in_name(path: &Path) -> Option<String> {
        let tests = super::crate_dir().join("tests");
        let name = path.strip_prefix(&tests).ok()?.to_str()?.to_string();
        BUILT_IN.iter().any(|(n, _)| *n == name).then_some(name)
    }

    pub fn read(path: &Path) -> Option<Vec<u8>> {
        if let Some(written) = WRITTEN.with(|w| w.borrow().get(path).cloned()) {
            return written;
        }
        let name = built_in_name(path)?;
        BUILT_IN.iter().find(|(n, _)| *n == name).map(|(_, bytes)| bytes.to_vec())
    }

    /// Kept for the run; a picture beside the goldens (in tests/golden, or wherever KOMPOZ_GOLDEN_DIR says) sent
    /// to the runner too.
    pub fn write(path: &Path, bytes: &[u8]) -> bool {
        WRITTEN.with(|w| w.borrow_mut().insert(path.to_path_buf(), Some(bytes.to_vec())));
        if !path.starts_with("/tmp") {
            crate::web::save_file(&path.to_string_lossy(), bytes);
        }
        true
    }

    pub fn create_dir(_path: &Path) {}

    pub fn remove(path: &Path) {
        WRITTEN.with(|w| w.borrow_mut().insert(path.to_path_buf(), None));
    }

    pub fn remove_dir(path: &Path) {
        WRITTEN.with(|w| {
            for (p, bytes) in w.borrow_mut().iter_mut() {
                if p.starts_with(path) {
                    *bytes = None;
                }
            }
        });
    }
}
