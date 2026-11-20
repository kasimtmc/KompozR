//! Reading the Kotlin sources of a local Maven artifact (a `-sources.jar`, a zip) for the generators that convert
//! them (`material3-tokens`, `material3-strings`, `material-icons`): the jar found in Gradle's cache, its entries
//! listed from the zip's central directory, stored or deflated ones read (miniz_oxide inflates).
//!
//! The jars are on the machine that generates (Gradle's cache, `~/.gradle/caches/modules-2/files-2.1`), not on CI:
//! a generator's `--check` checks only where its jar is.

use std::fs;
use std::path::{Path, PathBuf};

/// A zip's entries, read from its bytes.
pub struct Jar {
    path: PathBuf,
    bytes: Vec<u8>,
    entries: Vec<Entry>,
}

struct Entry {
    name: String,
    method: u16,
    compressed_size: usize,
    size: usize,
    local_header: usize,
}

impl Jar {
    /// The jar of `group:artifact:version` named `<artifact>-<version><suffix>` (`-sources.jar`), in Gradle's cache;
    /// none where it isn't there.
    pub fn find(group: &str, artifact: &str, version: &str, suffix: &str) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        let dir = Path::new(&home).join(".gradle/caches/modules-2/files-2.1").join(group).join(artifact).join(version);
        let file = format!("{artifact}-{version}{suffix}");
        let mut found: Vec<PathBuf> = fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter_map(|hash| fs::read_dir(hash.path()).ok())
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some(file.as_str()))
            .collect();
        found.sort();
        found.into_iter().next()
    }

    pub fn open(path: &Path) -> Result<Jar, String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let entries = central_directory(&bytes).ok_or_else(|| format!("{}: not a zip file", path.display()))?;
        Ok(Jar { path: path.to_path_buf(), bytes, entries })
    }

    /// The names of its entries under `prefix` ending in `suffix`, sorted.
    pub fn names(&self, prefix: &str, suffix: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.name.starts_with(prefix) && e.name.ends_with(suffix))
            .map(|e| e.name.clone())
            .collect();
        names.sort();
        names
    }

    /// An entry's text (UTF-8).
    pub fn text(&self, name: &str) -> Result<String, String> {
        let e = self
            .entries
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| format!("{}: no entry {name}", self.path.display()))?;
        let bad = || format!("{}: entry {name} can't be read", self.path.display());
        let h = e.local_header;
        if u32_at(&self.bytes, h)? != 0x0403_4b50 {
            return Err(bad());
        }
        let name_len = u16_at(&self.bytes, h + 26).ok_or_else(bad)? as usize;
        let extra_len = u16_at(&self.bytes, h + 28).ok_or_else(bad)? as usize;
        let start = h + 30 + name_len + extra_len;
        let data = self.bytes.get(start..start + e.compressed_size).ok_or_else(bad)?;
        let raw = match e.method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(data).map_err(|_| bad())?,
            m => return Err(format!("{}: entry {name} is compressed by method {m}", self.path.display())),
        };
        if raw.len() != e.size {
            return Err(bad());
        }
        String::from_utf8(raw).map_err(|_| format!("{}: entry {name} isn't UTF-8", self.path.display()))
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "a zip cut short".to_string())
}

/// The entries the end of central directory record points at.
fn central_directory(b: &[u8]) -> Option<Vec<Entry>> {
    // (the record is 22 bytes and a comment of up to 64 KiB)
    let lowest = b.len().saturating_sub(22 + 0xFFFF);
    let eocd = (lowest..=b.len().checked_sub(22)?).rev().find(|&i| u32_at(b, i).ok() == Some(0x0605_4b50))?;
    let count = u16_at(b, eocd + 10)? as usize;
    let mut at = u32_at(b, eocd + 16).ok()? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(b, at).ok()? != 0x0201_4b50 {
            return None;
        }
        let method = u16_at(b, at + 10)?;
        let compressed_size = u32_at(b, at + 20).ok()? as usize;
        let size = u32_at(b, at + 24).ok()? as usize;
        let name_len = u16_at(b, at + 28)? as usize;
        let extra_len = u16_at(b, at + 30)? as usize;
        let comment_len = u16_at(b, at + 32)? as usize;
        let local_header = u32_at(b, at + 42).ok()? as usize;
        let name = String::from_utf8_lossy(b.get(at + 46..at + 46 + name_len)?).into_owned();
        entries.push(Entry { name, method, compressed_size, size, local_header });
        at += 46 + name_len + extra_len + comment_len;
    }
    Some(entries)
}
