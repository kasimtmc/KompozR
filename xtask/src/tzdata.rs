//! `cargo xtask tzdata [--check]`: the web's time zone database (the web has no zoneinfo files; kompoz-web embeds
//! this one): every zone of the system's /usr/share/zoneinfo (`TZDIR` instead), its TZif file as it is, in one bundle
//! of Android's tzdata format (a header with the database's version and where the index and the data are; the
//! index's 52-byte entries, a zone's name and where its bytes are) — the zones that are links of another sharing its
//! bytes —, written to crates/kompoz-web/tzdata/tzdata. With `--check`: built and compared with what is committed
//! (skipped where there is no zoneinfo, or another version of it than the committed one's).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where the bundle is.
const BUNDLE: &str = "crates/kompoz-web/tzdata/tzdata";

pub fn run(workspace: &Path, check: bool) -> Result<(), String> {
    let dir = std::env::var_os("TZDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/share/zoneinfo"));
    let path = workspace.join(BUNDLE);
    if check && !dir.join("tzdata.zi").is_file() {
        println!("tzdata: no zoneinfo at {} (skipped)", dir.display());
        return Ok(());
    }
    let bundle = build(&dir)?;
    if check {
        let committed = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if committed.get(..12) != bundle.get(..12) {
            println!(
                "tzdata: the system's zoneinfo is {}, the committed bundle's {} (skipped)",
                version_of(&bundle),
                version_of(&committed)
            );
            return Ok(());
        }
        if committed != bundle {
            return Err(format!("tzdata: {BUNDLE} isn't what `cargo xtask tzdata` makes of {}", dir.display()));
        }
        println!("tzdata: {BUNDLE} up to date ({})", version_of(&bundle));
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&path, &bundle).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("tzdata: wrote {BUNDLE} ({}, {} bytes)", version_of(&bundle), bundle.len());
    Ok(())
}

fn version_of(bundle: &[u8]) -> String {
    let v = &bundle[..12.min(bundle.len())];
    String::from_utf8_lossy(&v[..v.iter().position(|&b| b == 0).unwrap_or(v.len())]).into_owned()
}

/// Every TZif file under `dir` (not its `posix` and `right` copies), by its zone's name, in order.
fn zones(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.strip_prefix(dir).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            if name == "posix" || name == "right" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            if bytes.starts_with(b"TZif") && name.len() < 40 && name != "posixrules" && name != "Factory" {
                out.push((name, bytes));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The bundle of `dir`'s zones.
fn build(dir: &Path) -> Result<Vec<u8>, String> {
    let zi = std::fs::read_to_string(dir.join("tzdata.zi")).map_err(|e| format!("tzdata.zi: {e}"))?;
    let version = zi
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("# version "))
        .map(str::trim)
        .ok_or("tzdata.zi names no version")?;
    let zones = zones(dir)?;
    // The data: each file's bytes once.
    let mut data = Vec::new();
    let mut at: HashMap<&[u8], (u32, u32)> = HashMap::new();
    let mut index = Vec::new();
    for (name, bytes) in &zones {
        let (start, length) = *at.entry(bytes.as_slice()).or_insert_with(|| {
            let start = data.len() as u32;
            data.extend_from_slice(bytes);
            (start, bytes.len() as u32)
        });
        let mut entry = [0u8; 52];
        entry[..name.len()].copy_from_slice(name.as_bytes());
        entry[40..44].copy_from_slice(&start.to_be_bytes());
        entry[44..48].copy_from_slice(&length.to_be_bytes());
        index.extend_from_slice(&entry);
    }
    let mut header = [0u8; 24];
    let tag = format!("tzdata{version}");
    let tag = &tag.as_bytes()[..tag.len().min(11)];
    header[..tag.len()].copy_from_slice(tag);
    let index_at = header.len() as u32;
    let data_at = index_at + index.len() as u32;
    header[12..16].copy_from_slice(&index_at.to_be_bytes());
    header[16..20].copy_from_slice(&data_at.to_be_bytes());
    header[20..24].copy_from_slice(&(data_at + data.len() as u32).to_be_bytes());
    let mut bundle = header.to_vec();
    bundle.extend_from_slice(&index);
    bundle.extend_from_slice(&data);
    Ok(bundle)
}
