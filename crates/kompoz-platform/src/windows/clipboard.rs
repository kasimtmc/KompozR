//! The Windows clipboard (C++'s readClipEntry/writeClipboard and clip formats, W:65-242, made lazy both ways as
//! Linux's selections are): what other apps put there read only as the app asks for each type (the clipboard's
//! sequence number telling whether it is still what was offered), and what the app puts there promised format by
//! format (delayed rendering: `SetClipboardData(format, NULL)`, the bytes made when another app asks, WM_RENDERFORMAT;
//! all of them as the app quits, WM_RENDERALLFORMATS), from the types [`Payload`] offers.
//!
//! The formats: text as `CF_UNICODETEXT` (Windows makes `CF_TEXT` of it for older apps); HTML as "HTML Format"
//! (CF_HTML: a header saying where the fragment is, in UTF-8); an image as "PNG" (browsers, Office, the Snipping
//! Tool) and a device-independent bitmap (`CF_DIB`, 32 bits with alpha: Windows makes `CF_DIBV5` and `CF_BITMAP` of
//! it); files as `CF_HDROP` (a DROPFILES); a URL alone as "UniformResourceLocatorW"; every other type the app gives
//! as a format registered by its MIME type's name (what another KompozR app reads it back by). A sensitive entry (a
//! password) is offered with "ExcludeClipboardContentFromMonitorProcessing" and with clipboard history and the cloud
//! clipboard told to leave it out ("CanIncludeInClipboardHistory", "CanUploadToCloudClipboard": 0).

use super::sys::{self, Hwnd, OpenClipboardGuard};
use crate::transfer::Payload;
use kompoz_graphics::{decode_to_image_bitmap, encode_png};
use kompoz_ui::clipboard::{
    ClipDataReader, ClipEntry, ClipItem, ClipMetadata, MIMETYPE_IMAGE_PNG, MIMETYPE_TEXT_HTML, MIMETYPE_TEXT_PLAIN,
    MIMETYPE_TEXT_URILIST, PASSWORD_MANAGER_HINT, file_path_of_uri, file_uri, uris_in_uri_list,
};
use std::cell::RefCell;
use std::rc::Rc;

// The predefined formats (winuser.h).
pub const CF_UNICODETEXT: u32 = 13;
pub const CF_DIB: u32 = 8;
pub const CF_DIBV5: u32 = 17;
pub const CF_HDROP: u32 = 15;

/// Text, as the app's payload names it here (what `Payload::offered` lists its text under).
const TEXT_TYPES: &[&str] = &[MIMETYPE_TEXT_PLAIN];

/// The registered formats the clipboard speaks with (their numbers are the session's).
#[derive(Clone, Copy, Debug)]
pub struct Formats {
    pub html: u32,
    pub png: u32,
    pub url: u32,
    exclude_monitor: u32,
    history: u32,
    cloud: u32,
}

impl Formats {
    pub fn register() -> Formats {
        Formats {
            html: sys::register_clipboard_format("HTML Format"),
            png: sys::register_clipboard_format("PNG"),
            url: sys::register_clipboard_format("UniformResourceLocatorW"),
            exclude_monitor: sys::register_clipboard_format("ExcludeClipboardContentFromMonitorProcessing"),
            history: sys::register_clipboard_format("CanIncludeInClipboardHistory"),
            cloud: sys::register_clipboard_format("CanUploadToCloudClipboard"),
        }
    }

    /// The MIME types the clipboard's `formats` are read as (each once, in the order they come), and whether they
    /// are sensitive (the password managers' hint among them, as ClipMetadata takes it).
    pub fn mime_types(&self, formats: &[u32]) -> Vec<String> {
        let mut types: Vec<String> = Vec::new();
        let mut add = |t: &str| {
            if !types.iter().any(|x| x == t) {
                types.push(t.to_string());
            }
        };
        for &f in formats {
            match f {
                CF_HDROP => add(MIMETYPE_TEXT_URILIST),
                CF_UNICODETEXT => add(MIMETYPE_TEXT_PLAIN),
                CF_DIB | CF_DIBV5 => add(MIMETYPE_IMAGE_PNG),
                _ if f == self.png => add(MIMETYPE_IMAGE_PNG),
                _ if f == self.html => add(MIMETYPE_TEXT_HTML),
                _ if f == self.url => add(MIMETYPE_TEXT_URILIST),
                _ if f == self.exclude_monitor => add(PASSWORD_MANAGER_HINT),
                _ => {
                    // (A type registered by its MIME name: another KompozR app's own.)
                    if let Some(name) = sys::clipboard_format_name(f).filter(|n| is_mime_type(n)) {
                        add(&name);
                    }
                }
            }
        }
        types
    }

    /// The formats `mime_type` is read from, the best first.
    fn sources(&self, mime_type: &str) -> Vec<u32> {
        match mime_type {
            MIMETYPE_TEXT_PLAIN => vec![CF_UNICODETEXT],
            MIMETYPE_TEXT_HTML => vec![self.html],
            MIMETYPE_IMAGE_PNG => vec![self.png, CF_DIBV5, CF_DIB],
            MIMETYPE_TEXT_URILIST => vec![CF_HDROP, self.url],
            other => vec![sys::register_clipboard_format(other)],
        }
    }

    /// `mime_type` from the clipboard (open), converted from the format it is there as; none if it isn't.
    fn read(&self, clipboard: &OpenClipboardGuard, mime_type: &str) -> Option<Vec<u8>> {
        for format in self.sources(mime_type) {
            if format == 0 || !sys::clipboard_has(format) {
                continue;
            }
            let Some(bytes) = clipboard.get(format) else { continue };
            let converted = match format {
                CF_UNICODETEXT => Some(utf16_text(&bytes).into_bytes()),
                CF_DIB | CF_DIBV5 => png_of_dib(&bytes),
                CF_HDROP => {
                    Some(uri_list(&paths_of_drop_files(&bytes).iter().map(|p| file_uri(p)).collect::<Vec<_>>()))
                }
                f if f == self.html => Some(html_of_cf_html(&nul_terminated(&bytes)).into_bytes()),
                f if f == self.url => Some(uri_list(&[utf16_text(&bytes)])),
                _ => Some(bytes),
            };
            if converted.is_some() {
                return converted;
            }
        }
        None
    }
}

/// Whether a registered format's name is a MIME type's (`type/subtype`, no spaces).
fn is_mime_type(name: &str) -> bool {
    let mut parts = name.splitn(2, '/');
    let (Some(a), Some(b)) = (parts.next(), parts.next()) else { return false };
    !a.is_empty() && !b.is_empty() && !name.contains(char::is_whitespace)
}

/// What the clipboard holds (all of it, by its formats), read as each type is asked for while it holds the same
/// (its sequence number); none if it holds nothing the app can read. `owner`: the window opening it.
pub fn clip_entry(formats: Formats, owner: Hwnd) -> Option<ClipEntry> {
    let sequence = sys::clipboard_sequence();
    let types = {
        let clipboard = OpenClipboardGuard::open(owner)?;
        formats.mime_types(&clipboard.formats())
    };
    let metadata = ClipMetadata::of_offered(&types);
    let data: Vec<String> = metadata.mime_types().to_vec();
    if data.is_empty() {
        return None;
    }
    let reader = ClipboardReader { formats, owner, sequence };
    Some(ClipEntry::with_metadata(vec![ClipItem::from_reader(data, Rc::new(reader))], metadata))
}

/// Reads a type of what the clipboard held when the entry was made (none once it holds something else).
struct ClipboardReader {
    formats: Formats,
    owner: Hwnd,
    sequence: u32,
}

impl ClipDataReader for ClipboardReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        if sys::clipboard_sequence() != self.sequence {
            return None;
        }
        let clipboard = OpenClipboardGuard::open(self.owner)?;
        self.formats.read(&clipboard, mime_type)
    }
}

/// What the app put on the clipboard, promised format by format, rendered as other apps ask (the helper window's:
/// it owns the clipboard).
pub struct Owned {
    payload: Payload,
    /// Each promised format and the type it is made from.
    promised: Vec<(u32, String)>,
    /// The clipboard's sequence number once it held this.
    sequence: u32,
}

/// The clipboard's side the app keeps: its formats, what it put there (while it owns it).
pub struct Clipboard {
    pub formats: Formats,
    owned: RefCell<Option<Owned>>,
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard { formats: Formats::register(), owned: RefCell::new(None) }
    }

    /// What it holds: the app's own entry where it put it (it owns it still), else another app's (read lazily).
    pub fn entry(&self, owner: Hwnd) -> Option<ClipEntry> {
        if sys::clipboard_owner() == owner
            && let Some(own) = self.owned.borrow().as_ref()
        {
            return Some(own.payload.entry().clone());
        }
        clip_entry(self.formats, owner)
    }

    /// `entry` put on it by `owner` (the helper window), its formats promised (the sensitive markers given at once:
    /// they are small); none: emptied.
    pub fn set(&self, owner: Hwnd, entry: Option<ClipEntry>) {
        let promised = {
            let Some(clipboard) = OpenClipboardGuard::open(owner) else {
                crate::system::log("kompoz: the clipboard is held by another app");
                return;
            };
            // (Emptying makes WM_DESTROYCLIPBOARD come to the owner before: what it held goes.)
            clipboard.empty();
            *self.owned.borrow_mut() = None;
            let Some(entry) = entry else { return };
            let payload = Payload::of_entry(&entry);
            let promised = self.promise(&clipboard, &entry, &payload);
            (payload, promised)
        };
        let (payload, promised) = promised;
        // (Its sequence number once closed: a WM_DESTROYCLIPBOARD of the emptying that comes after, as wine sends it
        // from its clipboard's own thread, is told from another app's taking it.)
        *self.owned.borrow_mut() = Some(Owned { payload, promised, sequence: sys::clipboard_sequence() });
    }

    /// The formats `payload` (of `entry`) is offered as, promised on the open clipboard (the sensitive markers given
    /// at once: they are small); each with the type it is made from.
    fn promise(&self, clipboard: &OpenClipboardGuard, entry: &ClipEntry, payload: &Payload) -> Vec<(u32, String)> {
        let f = self.formats;
        let mut promised: Vec<(u32, String)> = Vec::new();
        for t in payload.offered(TEXT_TYPES) {
            let formats: Vec<u32> = match t.as_str() {
                MIMETYPE_TEXT_PLAIN => vec![CF_UNICODETEXT],
                MIMETYPE_TEXT_HTML => vec![f.html],
                MIMETYPE_IMAGE_PNG => vec![f.png, CF_DIB],
                MIMETYPE_TEXT_URILIST => {
                    // Files as a DROPFILES; a single other URL as a URL; and the list itself by its name (for
                    // KompozR apps: every URI).
                    let uris = entry.uris();
                    let mut v = Vec::new();
                    if !entry.files().is_empty() {
                        v.push(CF_HDROP);
                    } else if uris.len() == 1 {
                        v.push(f.url);
                    }
                    v.push(sys::register_clipboard_format(MIMETYPE_TEXT_URILIST));
                    v
                }
                PASSWORD_MANAGER_HINT => {
                    clipboard.set(f.exclude_monitor, &[0]);
                    clipboard.set(f.history, &0u32.to_le_bytes());
                    clipboard.set(f.cloud, &0u32.to_le_bytes());
                    continue;
                }
                other => vec![sys::register_clipboard_format(other)],
            };
            for format in formats {
                if format != 0 && !promised.iter().any(|(p, _)| *p == format) && clipboard.promise(format) {
                    promised.push((format, t.clone()));
                }
            }
        }
        promised
    }

    /// WM_RENDERFORMAT: `format`'s bytes made from what the app put there, given to the clipboard (open by the app
    /// asking for it).
    pub fn render(&self, format: u32) {
        let bytes = self.owned.borrow().as_ref().and_then(|o| self.bytes(o, format));
        if let Some(bytes) = bytes {
            sys::set_clipboard_data(format, Some(&bytes));
        }
    }

    /// WM_RENDERALLFORMATS (the owner going: the app quits): every promised format made, if the app still owns the
    /// clipboard.
    pub fn render_all(&self, owner: Hwnd) {
        if self.owned.borrow().is_none() {
            return;
        }
        let Some(clipboard) = OpenClipboardGuard::open(owner) else { return };
        if sys::clipboard_owner() != owner {
            return;
        }
        let owned = self.owned.borrow();
        let Some(o) = owned.as_ref() else { return };
        for (format, _) in &o.promised {
            if let Some(bytes) = self.bytes(o, *format) {
                clipboard.set(*format, &bytes);
            }
        }
    }

    /// WM_DESTROYCLIPBOARD: another app (or the app, emptying it) took the clipboard: what it held goes — unless
    /// `owner` still owns what it put there (the message of an emptying before, come late).
    pub fn lost(&self, owner: Hwnd) {
        let mut owned = self.owned.borrow_mut();
        let still =
            owned.as_ref().is_some_and(|o| sys::clipboard_owner() == owner && sys::clipboard_sequence() == o.sequence);
        if !still {
            *owned = None;
        }
    }

    fn bytes(&self, owned: &Owned, format: u32) -> Option<Vec<u8>> {
        let (_, mime_type) = owned.promised.iter().find(|(f, _)| *f == format)?;
        let data = owned.payload.bytes(mime_type, TEXT_TYPES)?;
        let f = self.formats;
        Some(match format {
            CF_UNICODETEXT => utf16_bytes(&String::from_utf8_lossy(&data)),
            CF_DIB => dib_of_png(&data)?,
            CF_HDROP => {
                let list = String::from_utf8_lossy(&data);
                let paths: Vec<String> = uris_in_uri_list(&list).iter().filter_map(|u| file_path_of_uri(u)).collect();
                drop_files_of(&paths)
            }
            _ if format == f.html => {
                let mut cf = cf_html_of(&String::from_utf8_lossy(&data)).into_bytes();
                cf.push(0);
                cf
            }
            _ if format == f.url => {
                let list = String::from_utf8_lossy(&data);
                utf16_bytes(uris_in_uri_list(&list).first()?)
            }
            _ => data.to_vec(),
        })
    }
}

// ---- The formats' conversions -----------------------------------------------------------------------------------

/// `CF_UNICODETEXT`'s bytes as text (UTF-16LE up to its 0).
fn utf16_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&p| u16::from_le_bytes(p)).collect();
    sys::narrow(&units)
}

/// Text as `CF_UNICODETEXT`'s bytes (UTF-16LE with its 0; line ends as they are).
fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

/// Bytes up to their first 0, as UTF-8.
fn nul_terminated(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// URIs as a `text/uri-list` (each ended by CRLF: RFC 2483).
fn uri_list(uris: &[String]) -> Vec<u8> {
    uris.iter().map(|u| format!("{u}\r\n")).collect::<String>().into_bytes()
}

/// The fragment a CF_HTML holds (C++'s `htmlOfCfHtml`): between its StartFragment and EndFragment offsets, else its
/// StartHTML and EndHTML, else all of it.
pub fn html_of_cf_html(cf: &str) -> String {
    let number = |key: &str| -> Option<usize> {
        let at = cf.find(key)? + key.len();
        let digits: String = cf[at..].chars().skip_while(|c| *c == ' ').take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    };
    let span = |a: &str, b: &str| -> Option<String> {
        let (from, to) = (number(a)?, number(b)?);
        (from <= to && to <= cf.len()).then(|| cf.get(from..to).map(str::to_string)).flatten()
    };
    span("StartFragment:", "EndFragment:").or_else(|| span("StartHTML:", "EndHTML:")).unwrap_or_else(|| cf.to_string())
}

/// HTML as CF_HTML (C++'s `cfHtmlOf`): the header's offsets, ten digits each, to where the markup and the fragment
/// are (byte offsets in UTF-8).
pub fn cf_html_of(html: &str) -> String {
    let header = |start: usize, end: usize, fragment: usize, fragment_end: usize| {
        format!(
            "Version:0.9\r\nStartHTML:{start:010}\r\nEndHTML:{end:010}\r\nStartFragment:{fragment:010}\r\nEndFragment:{fragment_end:010}\r\n"
        )
    };
    let (before, after) = ("<html><body>\r\n<!--StartFragment-->", "<!--EndFragment-->\r\n</body></html>");
    let start = header(0, 0, 0, 0).len();
    let fragment = start + before.len();
    let fragment_end = fragment + html.len();
    header(start, fragment_end + after.len(), fragment, fragment_end) + before + html + after
}

/// A CF_DIB (or CF_DIBV5) as PNG (C++'s `pngOfDib`): given the file header a .bmp has, decoded, written as PNG.
pub fn png_of_dib(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let u32_at = |at: usize| u32::from_le_bytes([dib[at], dib[at + 1], dib[at + 2], dib[at + 3]]);
    let header = u32_at(0);
    let bpp = u16::from_le_bytes([dib[14], dib[15]]) as u32;
    let compression = u32_at(16);
    let colors = u32_at(32);
    let palette = if bpp <= 8 { (if colors != 0 { colors } else { 1 << bpp }) * 4 } else { 0 };
    // (BI_BITFIELDS' three masks follow a 40-byte header; the larger headers hold them inside.)
    let masks = if header == 40 && compression == 3 { 12 } else { 0 };
    let offset = 14 + header + masks + palette;
    let size = 14 + dib.len() as u32;
    let mut bmp = Vec::with_capacity(size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    let image = decode_to_image_bitmap(&bmp)?;
    encode_png(image.width(), image.height(), &image.pixels())
}

/// A PNG as a CF_DIB (C++'s `dibOfPng`): 32 bits a pixel, BI_RGB, the bottom row first, alpha in the fourth byte.
pub fn dib_of_png(png: &[u8]) -> Option<Vec<u8>> {
    let image = decode_to_image_bitmap(png)?;
    let (w, h) = (image.width(), image.height());
    let pixels = image.pixels();
    let mut dib = Vec::with_capacity(40 + (w * h * 4) as usize);
    for v in [40u32, w as u32, h as u32] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&1u16.to_le_bytes()); // planes
    dib.extend_from_slice(&32u16.to_le_bytes()); // bits a pixel
    for v in [0u32, (w * h * 4) as u32, 0, 0, 0, 0] {
        // (BI_RGB, the image's size, no resolution, no palette)
        dib.extend_from_slice(&v.to_le_bytes());
    }
    for y in (0..h as usize).rev() {
        for x in 0..w as usize {
            let p = &pixels[(y * w as usize + x) * 4..][..4];
            dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    Some(dib)
}

/// The paths a CF_HDROP (a DROPFILES) holds (C++'s `pathsOfDropFiles`): wide paths, or (an old app's) ANSI ones,
/// each ended by 0, a 0 after the last.
pub fn paths_of_drop_files(bytes: &[u8]) -> Vec<String> {
    let mut paths = Vec::new();
    if bytes.len() < 20 {
        return paths;
    }
    let files = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let wide = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) != 0;
    let mut at = files;
    while at < bytes.len() {
        if wide {
            let mut units = Vec::new();
            while at + 1 < bytes.len() {
                let c = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
                at += 2;
                if c == 0 {
                    break;
                }
                units.push(c);
            }
            if units.is_empty() {
                break;
            }
            paths.push(String::from_utf16_lossy(&units));
        } else {
            let end = bytes[at..].iter().position(|&b| b == 0).map_or(bytes.len(), |n| at + n);
            if end == at {
                break;
            }
            // (The ANSI code page's: Latin-1 for what is ASCII, which is what such paths almost always are.)
            paths.push(bytes[at..end].iter().map(|&b| char::from(b)).collect());
            at = end + 1;
        }
    }
    paths
}

/// Paths as a CF_HDROP (C++'s `dropFilesOf`): a DROPFILES (its files at 20, wide), then each path in UTF-16 with its
/// 0, and a 0 after the last.
pub fn drop_files_of(paths: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&20u32.to_le_bytes()); // pFiles
    bytes.extend_from_slice(&[0; 12]); // pt, fNC
    bytes.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for path in paths {
        let path = path.replace('/', "\\");
        bytes.extend(path.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes));
    }
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cf_html_round_trips() {
        let html = "<b>kalın</b> ve <i>eğik</i>";
        let cf = cf_html_of(html);
        assert!(cf.starts_with("Version:0.9\r\nStartHTML:0000000"));
        assert_eq!(html_of_cf_html(&cf), html);
        // Without a fragment, the markup; without a header, all of it.
        // (The header is 55 bytes long: the markup after it.)
        let cf = "Version:0.9\r\nStartHTML:0000000055\r\nEndHTML:0000000063\r\n<p>a</p>";
        assert_eq!(html_of_cf_html(cf), "<p>a</p>");
        assert_eq!(html_of_cf_html("<p>yalnız</p>"), "<p>yalnız</p>");
    }

    #[test]
    fn images_go_both_ways_as_dibs() {
        let rgba = [255u8, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 10, 20, 30, 0];
        let png = encode_png(2, 2, &rgba).unwrap();
        let dib = dib_of_png(&png).unwrap();
        assert_eq!(dib.len(), 40 + 16);
        // (The bottom row first: its first pixel blue, BGRA.)
        assert_eq!(&dib[40..44], &[255, 0, 0, 255]);
        let back = decode_to_image_bitmap(&png_of_dib(&dib).unwrap()).unwrap();
        assert_eq!(back.pixels().as_slice(), rgba);
    }

    #[test]
    fn drop_files_hold_paths() {
        let paths = vec!["C:\\Users\\kasim\\Görüntü 1.png".to_string(), "D:/b.txt".into()];
        let bytes = drop_files_of(&paths);
        assert_eq!(paths_of_drop_files(&bytes), ["C:\\Users\\kasim\\Görüntü 1.png", "D:\\b.txt"]);
        // An ANSI one.
        let mut ansi = vec![20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        ansi.extend_from_slice(b"C:\\a.txt\0C:\\b c.txt\0\0");
        assert_eq!(paths_of_drop_files(&ansi), ["C:\\a.txt", "C:\\b c.txt"]);
        assert!(paths_of_drop_files(&[1, 2]).is_empty());
    }

    #[test]
    fn text_is_utf16_with_its_end() {
        let bytes = utf16_bytes("Merhaba ğ 😀");
        assert_eq!(&bytes[bytes.len() - 2..], &[0, 0]);
        assert_eq!(utf16_text(&bytes), "Merhaba ğ 😀");
        assert!(is_mime_type("application/x-kompoz-fruit"));
        assert!(!is_mime_type("HTML Format"));
        assert!(!is_mime_type("Rich Text Format"));
    }
}
