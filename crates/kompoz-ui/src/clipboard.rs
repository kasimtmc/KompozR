//! What the clipboard and drags carry (Compose's `androidx.compose.ui.platform` ClipEntry and ClipMetadata, as
//! Android's ClipData and ClipDescription are under them): a [`ClipEntry`] is items ([`ClipItem`], Android's
//! `ClipData.Item`), each of one thing in one or more representations named by MIME types — text (plain, styled,
//! HTML), an image, URIs (files) or any bytes of the app's own type —, and the [`ClipMetadata`] describing them
//! (the MIME types, matched with wildcards as Android's `ClipDescription.hasMimeType`).
//!
//! What another app offers (the platform's clipboard, a drop) is read only when asked for: its item is made of the
//! types offered and a [`ClipDataReader`] the platform reads one of them through, once (a pipe on Wayland, a
//! selection conversion with INCR on X11). What the app puts there holds its values; the platform serves each type
//! as it is asked for (text in the encodings clipboards name, an image as PNG, URIs as a `text/uri-list`).
//!
//! The clipboard itself ([`Clipboard`](crate::platform::Clipboard), Compose's `Clipboard` with its suspending
//! `getClipEntry`/`setClipEntry`) and the text one Compose deprecated
//! ([`ClipboardManager`](crate::platform::ClipboardManager)) are the platform module's.

use kompoz_graphics::{ImageBitmap, decode_to_image_bitmap, encode_png};
use kompoz_text::AnnotatedString;
use std::cell::RefCell;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

/// Plain text (Android's `ClipDescription.MIMETYPE_TEXT_PLAIN`).
pub const MIMETYPE_TEXT_PLAIN: &str = "text/plain";
/// HTML (Android's `MIMETYPE_TEXT_HTML`).
pub const MIMETYPE_TEXT_HTML: &str = "text/html";
/// A list of URIs (RFC 2483: files as `file://` URIs; Android's `MIMETYPE_TEXT_URILIST`).
pub const MIMETYPE_TEXT_URILIST: &str = "text/uri-list";
/// A PNG image: what an image of the app's is offered as.
pub const MIMETYPE_IMAGE_PNG: &str = "image/png";

/// The names text goes by on Linux's clipboards besides MIME types (X11's ICCCM targets): UTF-8, and Latin-1 as
/// `STRING`.
const X11_TEXT_NAMES: &[&str] = &["UTF8_STRING", "TEXT", "STRING", "COMPOUND_TEXT"];

/// What X11 lists as targets that aren't data (ICCCM's and the clipboard managers' own).
const X11_NON_DATA_TARGETS: &[&str] =
    &["TARGETS", "MULTIPLE", "TIMESTAMP", "SAVE_TARGETS", "DELETE", "INSERT_SELECTION", "INSERT_PROPERTY"];

/// The image types an image is decoded from, the best first.
const IMAGE_TYPES: &[&str] = &["image/png", "image/webp", "image/jpeg", "image/jpg", "image/bmp", "image/x-bmp"];

/// The type and subtype of a MIME type, its parameters left out, in lower case.
fn essence(mime_type: &str) -> String {
    mime_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

/// A parameter of a MIME type (`charset` of `text/plain;charset=utf-8`), in lower case.
fn parameter(mime_type: &str, name: &str) -> Option<String> {
    mime_type.split(';').skip(1).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim().trim_matches('"').to_ascii_lowercase())
    })
}

/// Whether `concrete` is of `desired` (Android's `ClipDescription.compareMimeTypes`): `*/*` takes all, `type/*`
/// every subtype of the type, else the same type and subtype. Parameters (`;charset=utf-8`) aren't compared: Linux's
/// clipboards give them, Android's types have none.
pub fn compare_mime_types(concrete: &str, desired: &str) -> bool {
    let (concrete, desired) = (essence(concrete), essence(desired));
    if desired == "*/*" {
        return true;
    }
    match desired.split_once('/') {
        Some((kind, "*")) if !kind.is_empty() => concrete.split_once('/').is_some_and(|(k, _)| k == kind),
        Some((kind, _)) if !kind.is_empty() => concrete == desired,
        _ => false,
    }
}

/// Whether `mime_type` names text a clipboard holds as plain text: `text/plain` with any charset, or one of X11's
/// names for text.
pub fn is_plain_text_type(mime_type: &str) -> bool {
    essence(mime_type) == MIMETYPE_TEXT_PLAIN || X11_TEXT_NAMES.contains(&mime_type)
}

/// Bytes of `mime_type` as text: by its charset (UTF-8 if it names none; Latin-1 for X11's `STRING`, ISO-8859-1 and
/// ASCII; UTF-16 by its byte order mark, little-endian without one), a byte order mark taken off, a C string's
/// ending NULs too. UTF-8 that isn't valid is replaced where it isn't.
pub fn decode_text(bytes: &[u8], mime_type: &str) -> String {
    let charset = parameter(mime_type, "charset");
    let latin1 = mime_type == "STRING"
        || charset.as_deref().is_some_and(|c| matches!(c, "iso-8859-1" | "latin1" | "latin-1" | "us-ascii" | "ascii"));
    let text = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        utf16(rest, false)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        utf16(rest, true)
    } else if charset.as_deref().is_some_and(|c| c.starts_with("utf-16")) {
        utf16(bytes, charset.as_deref() == Some("utf-16be"))
    } else if latin1 {
        bytes.iter().map(|&b| char::from(b)).collect()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.trim_end_matches('\0').to_string()
}

fn utf16(bytes: &[u8], big_endian: bool) -> String {
    let (pairs, _) = bytes.as_chunks::<2>();
    let units: Vec<u16> =
        pairs.iter().map(|&p| if big_endian { u16::from_be_bytes(p) } else { u16::from_le_bytes(p) }).collect();
    String::from_utf16_lossy(&units)
}

/// `text` as bytes of `mime_type`: Latin-1 for X11's `STRING` and a Latin-1 charset (what it can't hold as '?'),
/// UTF-16 for a UTF-16 charset (with a byte order mark for plain `utf-16`), else UTF-8.
pub fn encode_text(text: &str, mime_type: &str) -> Vec<u8> {
    let charset = parameter(mime_type, "charset");
    match charset.as_deref() {
        _ if mime_type == "STRING" => latin1(text),
        Some("iso-8859-1" | "latin1" | "latin-1") => latin1(text),
        Some("us-ascii" | "ascii") => text.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }).collect(),
        Some("utf-16") => [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect(),
        Some("utf-16le") => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        Some("utf-16be") => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        _ => text.as_bytes().to_vec(),
    }
}

fn latin1(text: &str) -> Vec<u8> {
    text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect()
}

/// A local path as a `file://` URI (RFC 8089), percent-encoded: what isn't unreserved (or `/`, `:`) as `%XX` of its
/// UTF-8 bytes; a relative path taken from the app's working directory; a Windows drive's path with a slash before.
pub fn file_uri(path: &str) -> String {
    let absolute = match std::env::current_dir() {
        Ok(cwd) if !Path::new(path).is_absolute() && path.as_bytes().get(1) != Some(&b':') => {
            cwd.join(path).to_string_lossy().into_owned()
        }
        _ => path.to_string(),
    };
    let mut path = absolute.replace('\\', "/");
    if path.as_bytes().get(1) == Some(&b':') {
        path.insert(0, '/');
    }
    let mut out = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The URIs of a `text/uri-list` (RFC 2483): its lines, comments (`#`) and blank ones left out, each line's ending
/// (CRLF or LF) taken off.
pub fn uris_in_uri_list(list: &str) -> Vec<String> {
    list.split('\n')
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The local path a `file:` URI names: percent-decoded, a host before the path dropped (on Windows a share's: its
/// `\\host\share\…` path), as the system writes paths (on Windows a drive's with backslashes: `C:\a b.txt` of
/// `file:///C:/a%20b.txt`); none for another scheme.
pub fn file_path_of_uri(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://").or_else(|| uri.strip_prefix("file:"))?;
    let (host, path) = if rest.starts_with('/') { ("", rest) } else { rest.split_at(rest.find('/')?) };
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
    (!path.is_empty()).then(|| native_path(host, path))
}

/// A URI's path (and host) as the system writes paths: on Windows a drive's without the slash before it and with
/// backslashes, a share's after its host (`\\host\share`); elsewhere as it is (the host dropped).
fn native_path(host: &str, path: String) -> String {
    if cfg!(not(windows)) {
        let _ = host;
        return path;
    }
    let b = path.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        return path[1..].replace('/', "\\");
    }
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return format!("\\\\{host}{}", path.replace('/', "\\"));
    }
    path.replace('/', "\\")
}

/// The local files a `text/uri-list` names (C++'s `filesInUriList`): its `file:` URIs as paths; other schemes left
/// out.
pub fn files_in_uri_list(list: &str) -> Vec<String> {
    uris_in_uri_list(list).iter().filter_map(|u| file_path_of_uri(u)).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// Items

/// What reads another app's clip (the platform's): one of the MIME types it offers, read when asked for. A read
/// that can't be done (the clip went, the app didn't answer) gives none.
pub trait ClipDataReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>>;
}

/// A value an item holds as one of its types: text (plain or styled: the text as its type says it is encoded), any
/// bytes, or an image (as `image/png`: PNG-encoded when asked for).
#[derive(Clone)]
pub enum ClipValue {
    Text(String),
    AnnotatedText(AnnotatedString),
    Bytes(Rc<[u8]>),
    Image(ImageBitmap),
}

impl fmt::Debug for ClipValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClipValue::Text(t) => write!(f, "Text({t:?})"),
            ClipValue::AnnotatedText(t) => write!(f, "AnnotatedText({:?})", t.text()),
            ClipValue::Bytes(b) => write!(f, "Bytes({} bytes)", b.len()),
            ClipValue::Image(i) => write!(f, "Image({}x{})", i.width(), i.height()),
        }
    }
}

struct Representation {
    mime_type: String,
    /// None: read through the item's reader.
    value: Option<ClipValue>,
}

struct ItemData {
    representations: Vec<Representation>,
    reader: Option<Rc<dyn ClipDataReader>>,
    /// What was read through the reader (none: it couldn't be), each type once.
    read: RefCell<Vec<(String, Option<Rc<[u8]>>)>>,
}

/// One thing a clip holds (Android's `ClipData.Item`), in one or more representations, each named by its MIME type:
/// its text (`text/plain`; X11's `UTF8_STRING`, `STRING`), its HTML, its URIs (`text/uri-list`), an image, any other
/// type's bytes. Another app's is read as asked for. Cheap to clone; equal when the same.
#[derive(Clone)]
pub struct ClipItem(Rc<ItemData>);

impl PartialEq for ClipItem {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ClipItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut l = f.debug_list();
        for r in &self.0.representations {
            match &r.value {
                Some(v) => l.entry(&format_args!("{}: {v:?}", r.mime_type)),
                None => l.entry(&format_args!("{}: (read when asked)", r.mime_type)),
            };
        }
        l.finish()
    }
}

impl ClipItem {
    /// An item of these representations, as they are (each a MIME type and its value).
    pub fn of(representations: Vec<(String, ClipValue)>) -> ClipItem {
        let representations =
            representations.into_iter().map(|(mime_type, v)| Representation { mime_type, value: Some(v) }).collect();
        ClipItem(Rc::new(ItemData { representations, reader: None, read: RefCell::default() }))
    }

    /// Another app's item: of the types it offers, each read through `reader` when asked for (the platform's).
    pub fn from_reader(mime_types: Vec<String>, reader: Rc<dyn ClipDataReader>) -> ClipItem {
        let representations =
            mime_types.into_iter().map(|mime_type| Representation { mime_type, value: None }).collect();
        ClipItem(Rc::new(ItemData { representations, reader: Some(reader), read: RefCell::default() }))
    }

    /// Text (Android's `ClipData.Item(text)`): `text/plain`.
    pub fn with_text(text: impl Into<String>) -> ClipItem {
        ClipItem::of(vec![(MIMETYPE_TEXT_PLAIN.into(), ClipValue::Text(text.into()))])
    }

    /// Styled text (`text/plain`, its styles kept within the app: Compose desktop's AnnotatedString flavor).
    pub fn with_annotated_string(text: AnnotatedString) -> ClipItem {
        ClipItem::of(vec![(MIMETYPE_TEXT_PLAIN.into(), ClipValue::AnnotatedText(text))])
    }

    /// HTML and its text (Android's `ClipData.Item(text, htmlText)`): `text/html` and `text/plain`.
    pub fn with_html_text(text: impl Into<String>, html_text: impl Into<String>) -> ClipItem {
        ClipItem::of(vec![
            (MIMETYPE_TEXT_HTML.into(), ClipValue::Text(html_text.into())),
            (MIMETYPE_TEXT_PLAIN.into(), ClipValue::Text(text.into())),
        ])
    }

    /// A URI (Android's `ClipData.Item(uri)`): a `text/uri-list` of it.
    pub fn with_uri(uri: impl Into<String>) -> ClipItem {
        ClipItem::of(vec![(MIMETYPE_TEXT_URILIST.into(), ClipValue::Text(uri.into()))])
    }

    /// A local file: its `file://` URI ([`file_uri`]: a relative path taken from the working directory).
    pub fn with_file(path: &str) -> ClipItem {
        ClipItem::with_uri(file_uri(path))
    }

    /// An image (`image/png`).
    pub fn with_image(image: ImageBitmap) -> ClipItem {
        ClipItem::of(vec![(MIMETYPE_IMAGE_PNG.into(), ClipValue::Image(image))])
    }

    /// Its types, as it has them (the best first).
    pub fn mime_types(&self) -> Vec<String> {
        self.0.representations.iter().map(|r| r.mime_type.clone()).collect()
    }

    /// Whether it has a type of `mime_type` (wildcards as [`compare_mime_types`]).
    pub fn has_mime_type(&self, mime_type: &str) -> bool {
        self.0.representations.iter().any(|r| compare_mime_types(&r.mime_type, mime_type))
    }

    /// The value it holds as `mime_type` (its own; none if it is read), the type as it has it.
    pub fn value(&self, mime_type: &str) -> Option<&ClipValue> {
        self.0.representations.iter().find(|r| r.mime_type == mime_type).and_then(|r| r.value.as_ref())
    }

    /// Whether it is another app's (read as asked for).
    pub fn is_read_on_request(&self) -> bool {
        self.0.reader.is_some()
    }

    /// Its bytes as `mime_type` (one of its types, as it names it): text encoded as the type says, an image as PNG
    /// (for `image/png`), another app's read through the platform (once); none if it hasn't the type or it can't
    /// be read.
    pub fn bytes(&self, mime_type: &str) -> Option<Rc<[u8]>> {
        let r = self.0.representations.iter().find(|r| r.mime_type == mime_type)?;
        match &r.value {
            Some(ClipValue::Text(t)) => Some(encode_text(t, mime_type).into()),
            Some(ClipValue::AnnotatedText(t)) => Some(encode_text(t.text(), mime_type).into()),
            Some(ClipValue::Bytes(b)) => Some(b.clone()),
            Some(ClipValue::Image(i)) => {
                if essence(mime_type) != MIMETYPE_IMAGE_PNG {
                    return None;
                }
                encode_png(i.width(), i.height(), &i.pixels()).map(Rc::from)
            }
            None => {
                if let Some((_, read)) = self.0.read.borrow().iter().find(|(m, _)| m == mime_type) {
                    return read.clone();
                }
                let read: Option<Rc<[u8]>> = self.0.reader.as_ref()?.read(mime_type).map(Rc::from);
                self.0.read.borrow_mut().push((mime_type.to_string(), read.clone()));
                read
            }
        }
    }

    /// The types its text can be read as, the best first: UTF-8, then plain text of another charset, then X11's
    /// names (Latin-1's last).
    fn text_types(&self) -> Vec<&str> {
        let rank = |m: &str| {
            if essence(m) == MIMETYPE_TEXT_PLAIN {
                match parameter(m, "charset").as_deref() {
                    Some("utf-8" | "utf8") => Some(0),
                    None => Some(1),
                    Some(_) => Some(3),
                }
            } else {
                match m {
                    "UTF8_STRING" => Some(2),
                    "TEXT" => Some(4),
                    "STRING" => Some(5),
                    _ => None,
                }
            }
        };
        let mut types: Vec<(i32, &str)> =
            self.0.representations.iter().filter_map(|r| Some((rank(&r.mime_type)?, r.mime_type.as_str()))).collect();
        types.sort_by_key(|(k, _)| *k);
        types.into_iter().map(|(_, m)| m).collect()
    }

    /// Its text (Android's `Item.getText`): its plain text, of the best type that can be read, decoded as the type
    /// says; none if it has none.
    pub fn text(&self) -> Option<String> {
        self.text_types().into_iter().find_map(|mime| match self.value(mime) {
            Some(ClipValue::Text(t)) => Some(t.clone()),
            Some(ClipValue::AnnotatedText(t)) => Some(t.text().to_string()),
            _ => self.bytes(mime).map(|b| decode_text(&b, mime)),
        })
    }

    /// Its text with its styles where the app put styled text there (Compose desktop's AnnotatedString flavor),
    /// else its plain text unstyled.
    pub fn annotated_string(&self) -> Option<AnnotatedString> {
        for mime in self.text_types() {
            if let Some(ClipValue::AnnotatedText(t)) = self.value(mime) {
                return Some(t.clone());
            }
        }
        self.text().map(AnnotatedString::new)
    }

    /// Its HTML (Android's `Item.getHtmlText`): `text/html`, decoded by its charset or byte order mark (browsers give
    /// it as UTF-8 or UTF-16).
    pub fn html_text(&self) -> Option<String> {
        let mime =
            self.0.representations.iter().find(|r| essence(&r.mime_type) == MIMETYPE_TEXT_HTML)?.mime_type.as_str();
        match self.value(mime) {
            Some(ClipValue::Text(t)) => Some(t.clone()),
            Some(ClipValue::AnnotatedText(t)) => Some(t.text().to_string()),
            _ => self.bytes(mime).map(|b| decode_text(&b, mime)),
        }
    }

    /// Its URIs (a `text/uri-list`'s), in order.
    pub fn uris(&self) -> Vec<String> {
        let Some(r) = self.0.representations.iter().find(|r| essence(&r.mime_type) == MIMETYPE_TEXT_URILIST) else {
            return Vec::new();
        };
        let list = match &r.value {
            Some(ClipValue::Text(t)) => t.clone(),
            Some(ClipValue::AnnotatedText(t)) => t.text().to_string(),
            _ => match self.bytes(&r.mime_type) {
                Some(b) => decode_text(&b, &r.mime_type),
                None => return Vec::new(),
            },
        };
        uris_in_uri_list(&list)
    }

    /// Its URI (Android's `Item.getUri`): the first of its URIs.
    pub fn uri(&self) -> Option<String> {
        self.uris().into_iter().next()
    }

    /// The local files its URIs name (`file:` ones, as paths).
    pub fn files(&self) -> Vec<String> {
        self.uris().iter().filter_map(|u| file_path_of_uri(u)).collect()
    }

    /// Its image: one it holds, else its bytes of an image type decoded (PNG, WebP, JPEG, BMP), else the image file
    /// its first `file:` URI names, decoded; none if it has none that decodes.
    pub fn image(&self) -> Option<ImageBitmap> {
        for r in &self.0.representations {
            if let Some(ClipValue::Image(i)) = &r.value {
                return Some(i.clone());
            }
        }
        for wanted in IMAGE_TYPES {
            if let Some(r) = self.0.representations.iter().find(|r| essence(&r.mime_type) == *wanted)
                && let Some(image) = self.bytes(&r.mime_type).and_then(|b| decode_to_image_bitmap(&b))
            {
                return Some(image);
            }
        }
        let path = self.files().into_iter().next()?;
        decode_to_image_bitmap(&std::fs::read(path).ok()?)
    }

    /// What it is as text (Android's `Item.coerceToText`): its text, else its URIs a line each, else nothing.
    pub fn coerce_to_text(&self) -> String {
        if let Some(text) = self.text() {
            return text;
        }
        self.uris().join("\n")
    }

    /// The same item with every representation read now (another app's: what it offers kept, as it may go).
    pub fn materialized(&self) -> ClipItem {
        if self.0.reader.is_none() {
            return self.clone();
        }
        let representations = self
            .0
            .representations
            .iter()
            .filter_map(|r| Some((r.mime_type.clone(), ClipValue::Bytes(self.bytes(&r.mime_type)?))))
            .collect();
        ClipItem::of(representations)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Metadata and entries

/// The type a Linux password manager (KeePassXC, Klipper's, …) offers beside a password it copies, asking that it
/// be kept out of clipboard histories and syncs ("secret").
pub const PASSWORD_MANAGER_HINT: &str = "x-kde-passwordManagerHint";

/// What a clip holds, described without reading it (Compose's ClipMetadata over Android's ClipDescription): its
/// MIME types, and whether it is sensitive (a password: Android's `EXTRA_IS_SENSITIVE`, a Linux password manager's
/// [`PASSWORD_MANAGER_HINT`]; C++'s `isSensitive`, b7ebe27).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ClipMetadata {
    mime_types: Vec<String>,
    is_sensitive: bool,
}

impl ClipMetadata {
    pub fn new(mime_types: Vec<String>) -> ClipMetadata {
        ClipMetadata { mime_types, is_sensitive: false }
    }

    /// The same, sensitive or not: what the app copies so is offered with the password managers' hint (other apps'
    /// histories leave it out).
    pub fn with_sensitive(mut self, sensitive: bool) -> ClipMetadata {
        self.is_sensitive = sensitive;
        self
    }

    /// Whether it is a password or the like, its source asking for it not to be kept or shown (C++'s and Compose's
    /// `isSensitive`).
    pub fn is_sensitive(&self) -> bool {
        self.is_sensitive
    }

    /// What another app offers, as the platform lists it: its types as they are, X11's targets that aren't data
    /// (TARGETS, TIMESTAMP, …) left out, and `text/plain` added where text is offered only by X11's names
    /// (`UTF8_STRING`, `STRING`) — as Android describes text.
    pub fn of_offered(types: &[String]) -> ClipMetadata {
        // (A password manager's hint: the clip sensitive, the hint itself no data.)
        let is_sensitive = types.iter().any(|t| t == PASSWORD_MANAGER_HINT);
        let mut mime_types: Vec<String> = types
            .iter()
            .filter(|t| !X11_NON_DATA_TARGETS.contains(&t.as_str()) && *t != PASSWORD_MANAGER_HINT)
            .cloned()
            .collect();
        if mime_types.iter().any(|t| X11_TEXT_NAMES.contains(&t.as_str()))
            && !mime_types.iter().any(|t| essence(t) == MIMETYPE_TEXT_PLAIN)
        {
            mime_types.push(MIMETYPE_TEXT_PLAIN.into());
        }
        ClipMetadata { mime_types, is_sensitive }
    }

    /// Its MIME types.
    pub fn mime_types(&self) -> &[String] {
        &self.mime_types
    }

    /// How many types it has (Android's `getMimeTypeCount`).
    pub fn mime_type_count(&self) -> usize {
        self.mime_types.len()
    }

    /// Its type at `index` (Android's `getMimeType`).
    pub fn get_mime_type(&self, index: usize) -> Option<&str> {
        self.mime_types.get(index).map(String::as_str)
    }

    /// Whether it has a type of `mime_type`, wildcards matched (Android's `hasMimeType`: `text/*`, `*/*`).
    pub fn has_mime_type(&self, mime_type: &str) -> bool {
        self.mime_types.iter().any(|t| compare_mime_types(t, mime_type))
    }

    /// Its types of `mime_type` (Android's `filterMimeTypes`).
    pub fn filter_mime_types(&self, mime_type: &str) -> Vec<String> {
        self.mime_types.iter().filter(|t| compare_mime_types(t, mime_type)).cloned().collect()
    }
}

struct EntryData {
    items: Vec<ClipItem>,
    metadata: ClipMetadata,
}

/// What the clipboard (or a drag) holds (Compose's ClipEntry, Android's ClipData under it): its items and the
/// metadata describing them ([`clip_metadata`](ClipEntry::clip_metadata), readable without reading the items).
/// Cheap to clone; equal when the same (as Compose's).
#[derive(Clone)]
pub struct ClipEntry(Rc<EntryData>);

impl PartialEq for ClipEntry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ClipEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ClipEntry")
            .field("mime_types", &self.0.metadata.mime_types)
            .field("items", &self.0.items)
            .finish()
    }
}

impl Default for ClipEntry {
    /// An entry holding nothing.
    fn default() -> Self {
        ClipEntry::new(Vec::new())
    }
}

impl ClipEntry {
    /// An entry of `items`, described by their types (each once, in order: as Android's ClipData made of items).
    pub fn new(items: Vec<ClipItem>) -> ClipEntry {
        let mut mime_types: Vec<String> = Vec::new();
        for item in &items {
            for r in &item.0.representations {
                let described = if is_plain_text_type(&r.mime_type) { MIMETYPE_TEXT_PLAIN } else { &r.mime_type };
                if !mime_types.iter().any(|m| m == described) {
                    mime_types.push(described.to_string());
                }
            }
        }
        ClipEntry::with_metadata(items, ClipMetadata::new(mime_types))
    }

    /// An entry of `items` described by `metadata` (another app's, as it lists its types).
    pub fn with_metadata(items: Vec<ClipItem>, metadata: ClipMetadata) -> ClipEntry {
        ClipEntry(Rc::new(EntryData { items, metadata }))
    }

    /// Plain text (Compose's `ClipEntry.withPlainText`, Android's `ClipData.newPlainText`).
    pub fn with_plain_text(text: impl Into<String>) -> ClipEntry {
        ClipEntry::new(vec![ClipItem::with_text(text)])
    }

    /// Styled text: plain text to other apps, its styles within the app (Compose's `AnnotatedString.toClipEntry`).
    pub fn with_annotated_string(text: AnnotatedString) -> ClipEntry {
        ClipEntry::new(vec![ClipItem::with_annotated_string(text)])
    }

    /// HTML and its text (Android's `ClipData.newHtmlText`).
    pub fn with_html_text(text: impl Into<String>, html_text: impl Into<String>) -> ClipEntry {
        ClipEntry::new(vec![ClipItem::with_html_text(text, html_text)])
    }

    /// An image (as `image/png`).
    pub fn with_image(image: ImageBitmap) -> ClipEntry {
        ClipEntry::new(vec![ClipItem::with_image(image)])
    }

    /// URIs, an item each (Android's `ClipData.newRawUri` and `addItem`).
    pub fn with_uris(uris: Vec<String>) -> ClipEntry {
        ClipEntry::new(uris.into_iter().map(ClipItem::with_uri).collect())
    }

    /// Local files, an item each (their `file://` URIs).
    pub fn with_files(paths: Vec<String>) -> ClipEntry {
        ClipEntry::new(paths.iter().map(|p| ClipItem::with_file(p)).collect())
    }

    /// What describes it (Compose's `clipMetadata`).
    pub fn clip_metadata(&self) -> &ClipMetadata {
        &self.0.metadata
    }

    /// Its items.
    pub fn items(&self) -> &[ClipItem] {
        &self.0.items
    }

    /// How many items it has (Android's `getItemCount`).
    pub fn item_count(&self) -> usize {
        self.0.items.len()
    }

    /// Its item at `index` (Android's `getItemAt`).
    pub fn get_item_at(&self, index: usize) -> Option<&ClipItem> {
        self.0.items.get(index)
    }

    /// Whether it holds text (Compose's `ClipEntry.hasText`: described as `text/*`).
    pub fn has_text(&self) -> bool {
        self.0.metadata.has_mime_type("text/*")
    }

    /// Its first item's text (Compose's `ClipEntry.readText`).
    pub fn read_text(&self) -> Option<String> {
        self.0.items.first()?.text()
    }

    /// Its first item's text with the styles the app gave it (Compose's `ClipEntry.readAnnotatedString`).
    pub fn read_annotated_string(&self) -> Option<AnnotatedString> {
        self.0.items.first()?.annotated_string()
    }

    /// The text of its items, a line between each (Compose's `ClipEntry.readPlainText`): none if none has text.
    pub fn read_plain_text(&self) -> Option<String> {
        let texts: Vec<String> = self.0.items.iter().filter_map(ClipItem::text).collect();
        (!texts.is_empty()).then(|| texts.join("\n"))
    }

    /// Its first URI (Compose's `ClipEntry.firstUriOrNull`).
    pub fn first_uri_or_null(&self) -> Option<String> {
        self.0.items.iter().find_map(ClipItem::uri)
    }

    /// Its items' URIs.
    pub fn uris(&self) -> Vec<String> {
        self.0.items.iter().flat_map(ClipItem::uris).collect()
    }

    /// The local files its items' URIs name.
    pub fn files(&self) -> Vec<String> {
        self.0.items.iter().flat_map(ClipItem::files).collect()
    }

    /// Its first image (an item's, as [`ClipItem::image`] finds it).
    pub fn image(&self) -> Option<ImageBitmap> {
        self.0.items.iter().find_map(ClipItem::image)
    }

    /// What it is as text: its items' ([`ClipItem::coerce_to_text`]), a line between each.
    pub fn coerce_to_text(&self) -> String {
        let texts: Vec<String> = self.0.items.iter().map(ClipItem::coerce_to_text).filter(|t| !t.is_empty()).collect();
        texts.join("\n")
    }

    /// The same entry with its items read now (what another app offered kept: it may go once the app takes the
    /// clipboard or the drop ends); itself if nothing is read on request.
    pub fn materialized(&self) -> ClipEntry {
        if !self.0.items.iter().any(ClipItem::is_read_on_request) {
            return self.clone();
        }
        ClipEntry::with_metadata(self.0.items.iter().map(ClipItem::materialized).collect(), self.0.metadata.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_password_managers_clip_is_sensitive() {
        let offered =
            |types: &[&str]| ClipMetadata::of_offered(&types.iter().map(|t| t.to_string()).collect::<Vec<_>>());
        let types = ["text/plain;charset=utf-8", "text/html"];
        assert!(!offered(&types).is_sensitive());
        let secret = offered(&["text/plain;charset=utf-8", PASSWORD_MANAGER_HINT]);
        assert!(secret.is_sensitive(), "the hint marks it");
        assert_eq!(secret.mime_types(), ["text/plain;charset=utf-8"], "the hint is no data");
        assert!(ClipMetadata::new(vec!["text/plain".into()]).with_sensitive(true).is_sensitive());
    }

    #[test]
    fn mime_types_match_as_androids_with_parameters_left_out() {
        assert!(compare_mime_types("text/plain", "text/*"));
        assert!(compare_mime_types("text/html", "*/*"));
        assert!(compare_mime_types("image/png", "image/*"));
        assert!(!compare_mime_types("image/png", "text/*"));
        assert!(compare_mime_types("text/plain;charset=utf-8", "text/plain"));
        assert!(compare_mime_types("TEXT/Plain", "text/plain"));
        assert!(!compare_mime_types("text/plain", "text/html"));
        assert!(!compare_mime_types("UTF8_STRING", "text/*"));
        assert!(!compare_mime_types("text/plain", "/*"));
    }

    #[test]
    fn offered_types_are_described_as_android_would() {
        let m = ClipMetadata::of_offered(&["TARGETS".into(), "UTF8_STRING".into(), "image/png".into()]);
        assert_eq!(m.mime_types(), ["UTF8_STRING", "image/png", "text/plain"]);
        assert!(m.has_mime_type("text/*") && m.has_mime_type("image/*"));
        assert_eq!(m.filter_mime_types("image/*"), ["image/png"]);
        let m = ClipMetadata::of_offered(&["text/plain;charset=utf-8".into(), "STRING".into()]);
        assert_eq!(m.mime_types(), ["text/plain;charset=utf-8", "STRING"]);
    }

    #[test]
    fn text_is_read_from_the_best_type_and_charset() {
        assert_eq!(decode_text(b"Saya\xe7", "STRING"), "Sayaç");
        assert_eq!(decode_text("ğ\0".as_bytes(), "text/plain"), "ğ");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'a', 0, 0x1F, 0x01], "text/html"), "ağ");
        assert_eq!(decode_text(&[0xFE, 0xFF, 0, b'a'], "text/html"), "a");
        assert_eq!(decode_text(b"\xef\xbb\xbfx", "text/html"), "x");
        assert_eq!(decode_text(b"\xe7", "text/plain;charset=ISO-8859-1"), "ç");
        assert_eq!(encode_text("aç€", "STRING"), b"a\xe7?");
        assert_eq!(encode_text("a", "text/plain;charset=utf-16"), [0xFF, 0xFE, b'a', 0]);
        assert_eq!(encode_text("ç", "text/plain"), "ç".as_bytes());

        struct Reader(Cell<i32>);
        impl ClipDataReader for Reader {
            fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
                self.0.set(self.0.get() + 1);
                match mime_type {
                    "STRING" => Some(b"latin \xe7".to_vec()),
                    "text/plain;charset=utf-8" => Some("utf-8 ç".as_bytes().to_vec()),
                    _ => None,
                }
            }
        }
        let reader = Rc::new(Reader(Cell::new(0)));
        let item = ClipItem::from_reader(
            vec!["STRING".into(), "text/plain;charset=utf-8".into(), "image/png".into()],
            reader.clone(),
        );
        assert_eq!(reader.0.get(), 0); // (nothing read until asked)
        assert_eq!(item.text().as_deref(), Some("utf-8 ç"));
        assert_eq!(item.text().as_deref(), Some("utf-8 ç"));
        assert_eq!(reader.0.get(), 1); // (read once)
        assert_eq!(item.image(), None); // (it couldn't be read)
        assert_eq!(item.image(), None);
        assert_eq!(reader.0.get(), 2);
        let kept = item.materialized();
        assert_eq!(reader.0.get(), 3); // (STRING read; the others were)
        assert!(!kept.is_read_on_request());
        assert_eq!(kept.mime_types(), ["STRING", "text/plain;charset=utf-8"]);
    }

    #[test]
    fn entries_describe_their_items_and_read_as_composes() {
        let image = ImageBitmap::from_pixels(1, 1, vec![255, 0, 0, 255]).unwrap();
        let e = ClipEntry::new(vec![
            ClipItem::with_html_text("kalın", "<b>kalın</b>"),
            ClipItem::with_image(image.clone()),
            ClipItem::with_file(FILE),
            ClipItem::with_text("iki"),
        ]);
        assert_eq!(e.clip_metadata().mime_types(), ["text/html", "text/plain", "image/png", "text/uri-list"]);
        assert!(e.has_text());
        assert_eq!(e.read_text().as_deref(), Some("kalın"));
        assert_eq!(e.read_plain_text().as_deref(), Some("kalın\niki"));
        assert_eq!(e.items()[0].html_text().as_deref(), Some("<b>kalın</b>"));
        assert_eq!(e.first_uri_or_null().as_deref(), Some(FILE_URI));
        assert_eq!(e.files(), [FILE]);
        assert_eq!(e.image(), Some(image.clone()));
        let png = e.items()[1].bytes("image/png").unwrap();
        assert_eq!(decode_to_image_bitmap(&png).unwrap().pixels().as_slice(), [255, 0, 0, 255]);
        assert_eq!(e.coerce_to_text(), format!("kalın\n{FILE_URI}\niki"));
        assert_eq!(ClipEntry::with_uris(vec!["https://kompoz.dev".into()]).read_plain_text(), None);
        assert_eq!(e.materialized(), e); // (nothing to read)
        assert_ne!(ClipEntry::with_plain_text("a"), ClipEntry::with_plain_text("a")); // (by identity)
    }

    /// A file, as the system writes its paths, and its URI.
    #[cfg(not(windows))]
    const FILE: &str = "/tmp/a b.txt";
    #[cfg(not(windows))]
    const FILE_URI: &str = "file:///tmp/a%20b.txt";
    #[cfg(windows)]
    const FILE: &str = "C:\\tmp\\a b.txt";
    #[cfg(windows)]
    const FILE_URI: &str = "file:///C:/tmp/a%20b.txt";

    #[test]
    fn uri_lists_and_file_uris() {
        if cfg!(not(windows)) {
            assert_eq!(file_uri("/home/kasim/Görüntü 1.png"), "file:///home/kasim/G%C3%B6r%C3%BCnt%C3%BC%201.png");
        }
        assert_eq!(file_uri("C:\\a b"), "file:///C:/a%20b");
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(file_path_of_uri(&file_uri("notes.txt")).unwrap(), cwd.join("notes.txt").to_string_lossy());
        let list = "# a comment\r\nfile:///home/kasim/G%C3%B6r%C3%BCnt%C3%BC.png\r\nhttps://example.org/x\r\n\
                    file://host/tmp/a%20b.txt\nfile:///bad%2\r\n";
        assert_eq!(uris_in_uri_list(list).len(), 4);
        if cfg!(windows) {
            // (Windows' paths: rooted on the current drive, a share's, a drive's.)
            assert_eq!(files_in_uri_list(list), ["\\home\\kasim\\Görüntü.png", "\\\\host\\tmp\\a b.txt", "\\bad%2"]);
            assert_eq!(file_path_of_uri("file:///C:/Users/a%20b.txt").unwrap(), "C:\\Users\\a b.txt");
        } else {
            assert_eq!(files_in_uri_list(list), ["/home/kasim/Görüntü.png", "/tmp/a b.txt", "/bad%2"]);
        }
        assert_eq!(file_path_of_uri("https://example.org/x"), None);
    }
}
