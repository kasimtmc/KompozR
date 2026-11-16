//! What KompozR's platforms take from a Linux desktop alike (C++'s desktop_services' plain part): links opened with
//! `xdg-open`, and the files a dropped `text/uri-list` names.

use crate::system::log;

/// Opens `uri` with the desktop's browser, or the app for its scheme (`xdg-open`), its process waited for on a
/// thread of its own (C++'s `openUri`).
pub fn open_uri(uri: &str) {
    match std::process::Command::new("xdg-open").arg(uri).spawn() {
        Ok(mut child) => {
            let spawned = std::thread::Builder::new().name("kompoz-xdg-open".into()).spawn(move || {
                let _ = child.wait();
            });
            if spawned.is_err() {
                log("kompoz: can't wait for xdg-open");
            }
        }
        Err(_) => log(&format!("kompoz: can't open {uri}")),
    }
}

/// The local files a `text/uri-list` names (C++'s `filesInUriList`): its `file:` URIs, a host before the path
/// dropped, percent-decoded; comments and other schemes left out.
pub fn files_in_uri_list(list: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in list.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let Some(rest) = line.strip_prefix("file://") else { continue };
        let path = match rest.find('/') {
            Some(slash) => &rest[slash..],
            None => continue,
        };
        let bytes = path.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if bytes[i] == b'%'
                && i + 2 < bytes.len()
                && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
            {
                out.push((h * 16 + l) as u8);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        let path = String::from_utf8_lossy(&out).into_owned();
        if !path.is_empty() {
            files.push(path);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_lists_name_their_local_files() {
        let list = "# a comment\r\nfile:///home/kasim/G%C3%B6r%C3%BCnt%C3%BC.png\r\nhttps://example.org/x\r\n\
                    file://host/tmp/a%20b.txt\nfile:///bad%2\r\n";
        assert_eq!(files_in_uri_list(list), ["/home/kasim/Görüntü.png", "/tmp/a b.txt", "/bad%2"]);
        assert!(files_in_uri_list("").is_empty());
    }
}
