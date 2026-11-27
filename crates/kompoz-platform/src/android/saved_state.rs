//! The app's saved state across process death (Compose's `rememberSaveable` through the activity's saved instance
//! state): the host's state goes into the activity's Bundle when it is small, and into a file of the app's own when
//! it isn't, the Bundle then holding only which file. A Bundle travels through a binder transaction, whose buffer
//! (about 1 MB) is the process's, shared with everything else in flight: a state much larger fails the save with
//! `TransactionTooLargeException` (C++ Kompoz had no guard, Compose has none: the app crashes); Google's advice is to
//! keep it to tens of kilobytes. Pure file work: tested on the host.

use std::path::{Path, PathBuf};

/// The most of the state kept in the Bundle itself (a quarter of the binder's buffer, half of what Android starts
/// warning of): what is larger goes to a file.
pub const BUNDLE_LIMIT: usize = 128 * 1024;

/// What a Bundle holds in place of a state kept in a file: this, then the file's token.
const MARKER: &[u8] = b"\0kompoz-saved-state-file\0";

/// The files' names (then the token).
const PREFIX: &str = "kompoz-saved-state-";

fn file(dir: &Path, token: &str) -> PathBuf {
    dir.join(format!("{PREFIX}{token}"))
}

/// The files of earlier saves in `dir` but `keep`'s, removed.
fn remove_others(dir: &Path, keep: Option<&str>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.strip_prefix(PREFIX).is_some_and(|token| Some(token) != keep) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// What goes into the Bundle for `state`: the state itself when it is small enough; else the marker and `token`,
/// the state written to a file of that token in `dir` (written whole, then renamed into place: a process ended
/// while writing leaves the last good one), earlier ones removed. None if it is too large and can't be written
/// (the app then starts afresh after a process death, as with no saved state).
pub fn save(state: &[u8], dir: &Path, token: &str) -> Option<Vec<u8>> {
    if state.len() <= BUNDLE_LIMIT && !state.starts_with(MARKER) {
        remove_others(dir, None);
        return Some(state.to_vec());
    }
    let path = file(dir, token);
    let partial = dir.join(format!("{PREFIX}{token}.partial"));
    std::fs::create_dir_all(dir).ok()?;
    std::fs::write(&partial, state).ok()?;
    std::fs::rename(&partial, &path).ok()?;
    remove_others(dir, Some(token));
    let mut out = MARKER.to_vec();
    out.extend_from_slice(token.as_bytes());
    Some(out)
}

/// The state a Bundle saved (`saved`, its bytes; none: the app starts afresh, its old files removed): the state
/// itself, or the file the marker names, read (and kept: a process ended again before the next save restores from
/// it again).
pub fn restore(saved: Option<&[u8]>, dir: &Path) -> Option<Vec<u8>> {
    let Some(saved) = saved else {
        remove_others(dir, None);
        return None;
    };
    match saved.strip_prefix(MARKER) {
        Some(token) => {
            let token = std::str::from_utf8(token).ok().filter(|t| !t.is_empty() && !t.contains(['/', '\\']))?;
            std::fs::read(file(dir, token)).ok()
        }
        None => Some(saved.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kompoz-saved-state-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_small_state_is_the_bundles() {
        let d = dir("small");
        let saved = save(b"state", &d, "1").unwrap();
        assert_eq!(saved, b"state");
        assert_eq!(restore(Some(&saved), &d).unwrap(), b"state");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_large_state_is_a_files() {
        let d = dir("large");
        let big = vec![7u8; BUNDLE_LIMIT + 1];
        let first = save(&big, &d, "a").unwrap();
        assert!(first.len() < 64);
        assert_eq!(restore(Some(&first), &d).unwrap(), big);
        // (Restored again after another process death: still there.)
        assert_eq!(restore(Some(&first), &d).unwrap(), big);
        // A later save replaces it; a small one removes it.
        let bigger = vec![8u8; BUNDLE_LIMIT + 2];
        let second = save(&bigger, &d, "b").unwrap();
        assert!(restore(Some(&first), &d).is_none());
        assert_eq!(restore(Some(&second), &d).unwrap(), bigger);
        save(b"x", &d, "c").unwrap();
        assert!(restore(Some(&second), &d).is_none());
        // A fresh start leaves no file behind.
        let third = save(&big, &d, "d").unwrap();
        assert!(restore(None, &d).is_none());
        assert!(restore(Some(&third), &d).is_none());
        // (A token that would leave the directory: nothing read.)
        let mut evil = MARKER.to_vec();
        evil.extend_from_slice(b"../x");
        assert!(restore(Some(&evil), &d).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
