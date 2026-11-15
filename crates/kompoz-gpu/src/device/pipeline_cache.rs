//! The pipeline cache's file, as the C++ Kompoz's: `KPZPIPE2`, the data's size and hash (8 bytes each,
//! little-endian), the data; read whole or not at all, written whole or not at all (a file beside it, renamed).

use super::log;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"KPZPIPE2";
const HEADER: usize = MAGIC.len() + 16;

/// FNV-1a's offset basis: where a hash starts.
pub const FNV1A_BASIS: u64 = 0xcbf29ce484222325;

/// FNV-1a, 64 bits: what the caches check and key what they keep by.
pub fn fnv1a(data: &[u8]) -> u64 {
    fnv1a_from(FNV1A_BASIS, data)
}

/// FNV-1a going on from `hash` (a hash of what came before) over `data`.
pub fn fnv1a_from(hash: u64, data: &[u8]) -> u64 {
    data.iter().fold(hash, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// FNV-1a over the data's 8-byte words (little-endian), then its last bytes: a cache file is megabytes, which byte
/// by byte took a Redmi Note 9 Pro 16 ms to check at every start.
fn cache_hash(data: &[u8]) -> u64 {
    let (words, rest) = data.as_chunks::<8>();
    let hash = words.iter().fold(FNV1A_BASIS, |h, w| (h ^ u64::from_le_bytes(*w)).wrapping_mul(0x100000001b3));
    fnv1a_from(hash, rest)
}

/// What a cache file's bytes hold; none if they aren't a whole one (a write cut short, another program's file).
pub fn pipeline_cache_contents(file: &[u8]) -> Option<&[u8]> {
    if file.len() < HEADER || &file[..MAGIC.len()] != MAGIC {
        return None;
    }
    let u64_at = |at: usize| u64::from_le_bytes(file[at..at + 8].try_into().unwrap());
    let data = &file[HEADER..];
    (u64_at(MAGIC.len()) == data.len() as u64 && u64_at(MAGIC.len() + 8) == cache_hash(data)).then_some(data)
}

/// A cache file's bytes holding `data`.
pub fn pipeline_cache_file(data: &[u8]) -> Vec<u8> {
    let mut file = Vec::with_capacity(HEADER + data.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&(data.len() as u64).to_le_bytes());
    file.extend_from_slice(&cache_hash(data).to_le_bytes());
    file.extend_from_slice(data);
    file
}

/// A pipeline cache's file: what was written into it; nothing if there is none or it isn't whole. (In one read: a
/// character at a time, 6.5 MB took a Redmi Note 9 Pro 70 ms at every start.)
pub fn read_pipeline_cache(path: &Path) -> Vec<u8> {
    std::fs::read(path).ok().and_then(|file| pipeline_cache_contents(&file).map(<[u8]>::to_vec)).unwrap_or_default()
}

/// Writes it whole or not at all (a file beside it, renamed), making its directory.
pub fn write_pipeline_cache(path: &Path, data: &[u8]) {
    // (Another run of the app writing it too writes a file of its own.)
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let mut part = path.as_os_str().to_owned();
    part.push(format!(".{}.{}.tmp", std::process::id(), stamp % 1_000_000_007));
    let part = PathBuf::from(part);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&part, pipeline_cache_file(data)).is_err() {
        let _ = std::fs::remove_file(&part);
        log(&format!("the pipeline cache couldn't be written: {}", path.display()));
        return;
    }
    let mut renamed = std::fs::rename(&part, path);
    if renamed.is_err() {
        // (Where renaming doesn't replace.)
        let _ = std::fs::remove_file(path);
        renamed = std::fs::rename(&part, path);
    }
    if renamed.is_err() {
        let _ = std::fs::remove_file(&part);
        log(&format!("the pipeline cache couldn't be written: {}", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_is_fnv1a() {
        assert_eq!(fnv1a(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a(b""), FNV1A_BASIS);
        assert_eq!(fnv1a(b"foobar"), 0x85944171f73967e8);
        assert_eq!(fnv1a_from(fnv1a(b"foo"), b"bar"), fnv1a(b"foobar"));
    }

    #[test]
    fn the_hash_takes_words_then_bytes() {
        // Eight bytes at a time as a little-endian word, then the rest byte by byte.
        let data: Vec<u8> = (0..19u8).map(|i| i.wrapping_mul(37)).collect();
        let mut hash = FNV1A_BASIS;
        for w in data[..16].chunks(8) {
            hash = (hash ^ u64::from_le_bytes(w.try_into().unwrap())).wrapping_mul(0x100000001b3);
        }
        assert_eq!(cache_hash(&data), fnv1a_from(hash, &data[16..]));
        assert_eq!(cache_hash(&[]), FNV1A_BASIS);
    }

    #[test]
    fn files_hold_what_was_written() {
        for n in [0usize, 1, 7, 8, 9, 100] {
            let data: Vec<u8> = (0..n).map(|i| (i * 31 + 5) as u8).collect();
            let file = pipeline_cache_file(&data);
            assert_eq!(&file[..8], b"KPZPIPE2");
            assert_eq!(file.len(), 24 + n);
            assert_eq!(pipeline_cache_contents(&file), Some(&data[..]));
        }
    }

    #[test]
    fn files_not_whole_hold_nothing() {
        let data: Vec<u8> = (0..64u8).collect();
        let file = pipeline_cache_file(&data);
        // Cut short anywhere, longer, or any byte changed: none.
        for n in 0..file.len() {
            assert_eq!(pipeline_cache_contents(&file[..n]), None, "cut to {n}");
        }
        let mut longer = file.clone();
        longer.push(0);
        assert_eq!(pipeline_cache_contents(&longer), None);
        for at in 0..file.len() {
            let mut changed = file.clone();
            changed[at] ^= 0x10;
            assert_eq!(pipeline_cache_contents(&changed), None, "byte {at} changed");
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)] // (files)
    fn files_are_written_whole_and_read_back() {
        let dir = std::env::temp_dir().join(format!("kompoz_gpu_cache_test_{}", std::process::id()));
        let path = dir.join("nested").join("pipelines.bin");
        assert!(read_pipeline_cache(&path).is_empty());
        write_pipeline_cache(&path, b"some driver's data");
        assert_eq!(read_pipeline_cache(&path), b"some driver's data");
        write_pipeline_cache(&path, b"more");
        assert_eq!(read_pipeline_cache(&path), b"more");
        // Nothing left beside it.
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        std::fs::write(&path, b"KPZPIPE2 cut").unwrap();
        assert!(read_pipeline_cache(&path).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
