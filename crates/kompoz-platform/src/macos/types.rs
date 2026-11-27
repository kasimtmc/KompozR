//! The pasteboard's types (uniform type identifiers) as KompozR's MIME types and back: what the clipboard and drags
//! offer and read on macOS. Text is UTF-8 (`public.utf8-plain-text`), HTML `public.html`, an image PNG (`public.png`;
//! TIFF, what most of AppKit's apps put there, read as PNG and offered besides it), files each a `public.file-url` of
//! an item of its own (one URL alone, not a file, as `public.url`), a sensitive entry marked with
//! nspasteboard.org's `org.nspasteboard.ConcealedType` (clipboard managers leave it out); every other type the app
//! gives under its MIME type's own name (what another KompozR app reads it back by).
//!
//! Plain Rust: tested on every host.

use kompoz_ui::clipboard::{
    MIMETYPE_IMAGE_PNG, MIMETYPE_TEXT_HTML, MIMETYPE_TEXT_PLAIN, MIMETYPE_TEXT_URILIST, PASSWORD_MANAGER_HINT,
};

pub const STRING: &str = "public.utf8-plain-text";
/// Text as older apps name it (NSStringPboardType).
pub const LEGACY_STRING: &str = "NSStringPboardType";
pub const HTML: &str = "public.html";
pub const PNG: &str = "public.png";
pub const TIFF: &str = "public.tiff";
pub const FILE_URL: &str = "public.file-url";
pub const URL: &str = "public.url";
pub const CONCEALED: &str = "org.nspasteboard.ConcealedType";

/// Text, as the app's payload names it on the pasteboard (`Payload::offered`'s text types).
pub const TEXT_TYPES: &[&str] = &[STRING];

/// The MIME types the pasteboard's `types` are read as (each once, in the order they come; the password managers'
/// hint for a concealed entry, as ClipMetadata takes it); types the app can't read left out.
pub fn mime_types(types: &[String]) -> Vec<String> {
    let mut mime: Vec<String> = Vec::new();
    for t in types {
        let read = match t.as_str() {
            STRING | LEGACY_STRING => MIMETYPE_TEXT_PLAIN,
            HTML => MIMETYPE_TEXT_HTML,
            PNG | TIFF => MIMETYPE_IMAGE_PNG,
            FILE_URL | URL | MIMETYPE_TEXT_URILIST => MIMETYPE_TEXT_URILIST,
            CONCEALED => PASSWORD_MANAGER_HINT,
            other if is_mime_type(other) => other,
            _ => continue,
        };
        if !mime.iter().any(|m| m == read) {
            mime.push(read.to_string());
        }
    }
    mime
}

/// The pasteboard types `mime_type` is read from, the best first (a URI list from the items' file URLs, else a URL,
/// else KompozR's own list: `ClipboardReader` gathers the URLs).
pub fn sources(mime_type: &str) -> Vec<&str> {
    match mime_type {
        MIMETYPE_TEXT_PLAIN => vec![STRING, LEGACY_STRING],
        MIMETYPE_TEXT_HTML => vec![HTML],
        MIMETYPE_IMAGE_PNG => vec![PNG, TIFF],
        MIMETYPE_TEXT_URILIST => vec![FILE_URL, URL, MIMETYPE_TEXT_URILIST],
        PASSWORD_MANAGER_HINT => vec![CONCEALED],
        other => vec![other],
    }
}

/// The pasteboard types the app's offered type `offered` (as `Payload::offered` names it with [`TEXT_TYPES`]) is put
/// there as: an image as PNG and TIFF, the URI list by its own name (the files and a URL go as items of their own);
/// the password managers' hint as the concealed marker.
pub fn offered_as(offered: &str) -> Vec<&str> {
    match offered {
        MIMETYPE_IMAGE_PNG => vec![PNG, TIFF],
        MIMETYPE_TEXT_HTML => vec![HTML],
        PASSWORD_MANAGER_HINT => vec![CONCEALED],
        other => vec![other],
    }
}

/// What the app's offered type is, from the pasteboard type another app asks for (TIFF: the PNG's, converted).
pub fn offered_for(pasteboard_type: &str) -> &str {
    match pasteboard_type {
        PNG | TIFF => MIMETYPE_IMAGE_PNG,
        HTML => MIMETYPE_TEXT_HTML,
        CONCEALED => PASSWORD_MANAGER_HINT,
        other => other,
    }
}

/// Whether a type's name is a MIME type's (`type/subtype`, no spaces): another KompozR app's own.
fn is_mime_type(name: &str) -> bool {
    let mut parts = name.splitn(2, '/');
    let (Some(a), Some(b)) = (parts.next(), parts.next()) else { return false };
    !a.is_empty() && !b.is_empty() && !name.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pasteboards_types_are_read_as_mime_types() {
        let types: Vec<String> = [FILE_URL, STRING, "public.rtf", TIFF, PNG, HTML, CONCEALED, "application/x-kompoz"]
            .iter()
            .map(|t| t.to_string())
            .collect();
        assert_eq!(
            mime_types(&types),
            ["text/uri-list", "text/plain", "image/png", "text/html", PASSWORD_MANAGER_HINT, "application/x-kompoz"]
        );
        assert_eq!(sources("text/plain"), [STRING, LEGACY_STRING]);
        assert_eq!(sources("image/png"), [PNG, TIFF]);
        assert_eq!(sources("application/x-kompoz"), ["application/x-kompoz"]);
    }

    #[test]
    fn the_apps_types_are_offered_as_the_pasteboards() {
        assert_eq!(offered_as("image/png"), [PNG, TIFF]);
        assert_eq!(offered_as(STRING), [STRING]);
        assert_eq!(offered_as(PASSWORD_MANAGER_HINT), [CONCEALED]);
        assert_eq!(offered_as("application/x-kompoz"), ["application/x-kompoz"]);
        assert_eq!(offered_for(TIFF), "image/png");
        assert_eq!(offered_for(STRING), STRING);
    }
}
