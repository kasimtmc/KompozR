//! The formats Windows moves data between apps in, as both of its ways speak them — the clipboard (`clipboard`) and
//! OLE's drag and drop (`ole`) — (C++'s clip formats, W:65-242, completed): what MIME types the formats another app
//! offers are read as and from which format each type is read, the best first; what formats the app's own
//! [`Payload`] is offered as and the bytes each is made of when asked for (delayed rendering, both ways); and the
//! actions of a drag as OLE's drop effects.
//!
//! The formats: text as `CF_UNICODETEXT` (and `CF_TEXT` in the ANSI code page for a drag: the clipboard makes that
//! one itself); HTML as "HTML Format" (CF_HTML: a header saying where the fragment is, in UTF-8); an image as "PNG"
//! (browsers, Office, the Snipping Tool) and device-independent bitmaps (32 bits with alpha: `CF_DIBV5`, its alpha
//! said by a mask, for a drag — the clipboard makes it of `CF_DIB` — and `CF_DIB`); files as `CF_HDROP` (a
//! DROPFILES), and from another app also as virtual files ("FileGroupDescriptorW" with their names, "FileContents"
//! with each one's bytes: an e-mail's attachments, a zip's entries — written out where the app can read them); a URL
//! alone as "UniformResourceLocatorW"; the uri-list itself by its MIME name (every URI, for KompozR apps); every other
//! type the app gives as a format registered by its MIME type's name (what another KompozR app reads it back by). A
//! sensitive entry (a password) goes with "ExcludeClipboardContentFromMonitorProcessing" and with clipboard history
//! and the cloud clipboard told to leave it out ("CanIncludeInClipboardHistory", "CanUploadToCloudClipboard": 0).
//!
//! Plain Rust, tested on every host: what is the system's — registering a format's name, a registered format's name,
//! the ANSI code page — comes through [`System`].

use crate::transfer::Payload;
use kompoz_graphics::{decode_to_image_bitmap, encode_png};
use kompoz_ui::clipboard::{
    MIMETYPE_IMAGE_PNG, MIMETYPE_TEXT_HTML, MIMETYPE_TEXT_PLAIN, MIMETYPE_TEXT_URILIST, PASSWORD_MANAGER_HINT,
    file_path_of_uri, file_uri, uris_in_uri_list,
};
use kompoz_ui::draganddrop::DragAndDropTransferAction;

// The predefined formats (winuser.h).
pub const CF_TEXT: u32 = 1;
pub const CF_DIB: u32 = 8;
pub const CF_UNICODETEXT: u32 = 13;
pub const CF_HDROP: u32 = 15;
pub const CF_DIBV5: u32 = 17;

/// Text, as the app's payload names it here (what `Payload::offered` lists its text under).
pub const TEXT_TYPES: &[&str] = &[MIMETYPE_TEXT_PLAIN];

/// What of the system the formats need.
pub trait System {
    /// A format registered by name (the same number in every process of the session); 0 if it can't be.
    fn register(&self, name: &str) -> u32;
    /// A registered format's name (none for the predefined ones).
    fn name(&self, format: u32) -> Option<String>;
    /// Text in the ANSI code page (what can't be said in it as the code page's default character), without its 0.
    fn ansi(&self, text: &str) -> Vec<u8>;
    /// Text in the ANSI code page, as Unicode.
    fn text_of_ansi(&self, bytes: &[u8]) -> String;
}

/// Where the formats go: the clipboard (which makes the older formats of the newer itself: `CF_TEXT`, `CF_DIBV5`), or
/// a drag (OLE makes none).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Medium {
    Clipboard,
    Drag,
}

/// The registered formats (their numbers are the session's).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Formats {
    pub html: u32,
    pub png: u32,
    pub url: u32,
    /// The uri-list by its MIME name (KompozR's).
    pub uri_list: u32,
    pub file_group: u32,
    pub file_contents: u32,
    /// What a target that moved files itself says it did ("Performed DropEffect": Explorer's optimized move).
    pub performed_effect: u32,
    pub exclude_monitor: u32,
    pub history: u32,
    pub cloud: u32,
}

/// What a payload is offered as: the formats promised (each with the type it is made from) and those given at once
/// (the sensitive markers: they are small).
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Offered {
    pub promised: Vec<(u32, String)>,
    pub given: Vec<(u32, Vec<u8>)>,
}

/// Another app's data, by format (its clipboard, a drag's data object).
pub trait Source {
    /// Whether it has `format` (as global memory).
    fn has(&self, format: u32) -> bool;
    /// `format`'s bytes.
    fn get(&self, format: u32) -> Option<Vec<u8>>;
    /// Its virtual files (FileGroupDescriptorW and FileContents) written out where the app can read them: their
    /// paths (the top ones: a folder's, not what it holds). None where it has none (the clipboard's are left).
    fn virtual_files(&self) -> Option<Vec<String>> {
        None
    }
}

impl Formats {
    pub fn register(system: &dyn System) -> Formats {
        Formats {
            html: system.register("HTML Format"),
            png: system.register("PNG"),
            url: system.register("UniformResourceLocatorW"),
            uri_list: system.register(MIMETYPE_TEXT_URILIST),
            file_group: system.register("FileGroupDescriptorW"),
            file_contents: system.register("FileContents"),
            performed_effect: system.register("Performed DropEffect"),
            exclude_monitor: system.register("ExcludeClipboardContentFromMonitorProcessing"),
            history: system.register("CanIncludeInClipboardHistory"),
            cloud: system.register("CanUploadToCloudClipboard"),
        }
    }

    /// The MIME types `formats` (another app's, in the order it offers them) are read as, each once, the password
    /// managers' hint among them for a sensitive entry (as ClipMetadata takes it).
    pub fn mime_types(&self, formats: &[u32], system: &dyn System) -> Vec<String> {
        let mut types: Vec<String> = Vec::new();
        let mut add = |t: &str| {
            if !types.iter().any(|x| x == t) {
                types.push(t.to_string());
            }
        };
        for &f in formats {
            match f {
                CF_HDROP => add(MIMETYPE_TEXT_URILIST),
                CF_UNICODETEXT | CF_TEXT => add(MIMETYPE_TEXT_PLAIN),
                CF_DIB | CF_DIBV5 => add(MIMETYPE_IMAGE_PNG),
                0 => {}
                _ if f == self.png => add(MIMETYPE_IMAGE_PNG),
                _ if f == self.html => add(MIMETYPE_TEXT_HTML),
                _ if f == self.url || f == self.file_group => add(MIMETYPE_TEXT_URILIST),
                _ if f == self.exclude_monitor => add(PASSWORD_MANAGER_HINT),
                _ => {
                    // (A type registered by its MIME name: another KompozR app's own, its uri-list.)
                    if let Some(name) = system.name(f).filter(|n| is_mime_type(n)) {
                        add(&name);
                    }
                }
            }
        }
        types
    }

    /// The formats `mime_type` is read from, the best first.
    pub fn sources(&self, mime_type: &str, system: &dyn System) -> Vec<u32> {
        match mime_type {
            MIMETYPE_TEXT_PLAIN => vec![CF_UNICODETEXT, CF_TEXT],
            MIMETYPE_TEXT_HTML => vec![self.html],
            MIMETYPE_IMAGE_PNG => vec![self.png, CF_DIBV5, CF_DIB],
            // (KompozR's list holds every URI; files alone as a DROPFILES or as virtual files; a URL alone.)
            MIMETYPE_TEXT_URILIST => vec![self.uri_list, CF_HDROP, self.file_group, self.url],
            other => vec![system.register(other)],
        }
    }

    /// `mime_type` read from `source` (another app's), converted from the first format it has it as that converts;
    /// none if it hasn't.
    pub fn read(&self, source: &dyn Source, mime_type: &str, system: &dyn System) -> Option<Vec<u8>> {
        for format in self.sources(mime_type, system) {
            if format == 0 || !source.has(format) {
                continue;
            }
            let converted = if format == self.file_group {
                source.virtual_files().map(|paths| uri_list(&paths.iter().map(|p| file_uri(p)).collect::<Vec<_>>()))
            } else {
                source.get(format).and_then(|bytes| self.convert(format, &bytes, system))
            };
            if converted.is_some() {
                return converted;
            }
        }
        None
    }

    /// A format's bytes (another app's) as the MIME type it is read as.
    fn convert(&self, format: u32, bytes: &[u8], system: &dyn System) -> Option<Vec<u8>> {
        match format {
            CF_UNICODETEXT => Some(utf16_text(bytes).into_bytes()),
            CF_TEXT => {
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                Some(system.text_of_ansi(&bytes[..end]).into_bytes())
            }
            CF_DIB | CF_DIBV5 => png_of_dib(bytes),
            CF_HDROP => Some(uri_list(&paths_of_drop_files(bytes).iter().map(|p| file_uri(p)).collect::<Vec<_>>())),
            f if f == self.html => Some(html_of_cf_html(&nul_terminated(bytes)).into_bytes()),
            f if f == self.url => Some(uri_list(&[utf16_text(bytes)])),
            f if f == self.uri_list => Some(nul_terminated(bytes).into_bytes()),
            _ => Some(bytes.to_vec()),
        }
    }

    /// The formats `payload` is offered as on `medium`, the best first (each with the type it is made from), and
    /// the sensitive markers given at once.
    pub fn offered(&self, payload: &Payload, medium: Medium, system: &dyn System) -> Offered {
        let entry = payload.entry();
        let mut offered = Offered::default();
        for t in payload.offered(TEXT_TYPES) {
            let formats: Vec<u32> = match t.as_str() {
                MIMETYPE_TEXT_PLAIN => match medium {
                    Medium::Clipboard => vec![CF_UNICODETEXT],
                    Medium::Drag => vec![CF_UNICODETEXT, CF_TEXT],
                },
                MIMETYPE_TEXT_HTML => vec![self.html],
                MIMETYPE_IMAGE_PNG => match medium {
                    Medium::Clipboard => vec![self.png, CF_DIB],
                    Medium::Drag => vec![self.png, CF_DIBV5, CF_DIB],
                },
                MIMETYPE_TEXT_URILIST => {
                    // Files as a DROPFILES; a single other URL as a URL; and the list itself by its name (for
                    // KompozR apps: every URI).
                    let mut v = Vec::new();
                    if !entry.files().is_empty() {
                        v.push(CF_HDROP);
                    } else if entry.uris().len() == 1 {
                        v.push(self.url);
                    }
                    v.push(self.uri_list);
                    v
                }
                PASSWORD_MANAGER_HINT => {
                    offered.given.push((self.exclude_monitor, vec![0]));
                    offered.given.push((self.history, 0u32.to_le_bytes().to_vec()));
                    offered.given.push((self.cloud, 0u32.to_le_bytes().to_vec()));
                    continue;
                }
                other => vec![system.register(other)],
            };
            for format in formats {
                if format != 0 && !offered.promised.iter().any(|(p, _)| *p == format) {
                    offered.promised.push((format, t.clone()));
                }
            }
        }
        offered
    }

    /// `format`'s bytes made from `payload`'s `mime_type` (a format it was offered as: `offered`).
    pub fn render(&self, payload: &Payload, format: u32, mime_type: &str, system: &dyn System) -> Option<Vec<u8>> {
        let data = payload.bytes(mime_type, TEXT_TYPES)?;
        Some(match format {
            CF_UNICODETEXT => utf16_bytes(&String::from_utf8_lossy(&data)),
            CF_TEXT => {
                let mut ansi = system.ansi(&String::from_utf8_lossy(&data));
                ansi.push(0);
                ansi
            }
            CF_DIB => dib_of_png(&data)?,
            CF_DIBV5 => dibv5_of_png(&data)?,
            CF_HDROP => {
                let list = String::from_utf8_lossy(&data);
                let paths: Vec<String> = uris_in_uri_list(&list).iter().filter_map(|u| file_path_of_uri(u)).collect();
                drop_files_of(&paths)
            }
            _ if format == self.html => {
                let mut cf = cf_html_of(&String::from_utf8_lossy(&data)).into_bytes();
                cf.push(0);
                cf
            }
            _ if format == self.url => {
                let list = String::from_utf8_lossy(&data);
                utf16_bytes(uris_in_uri_list(&list).first()?)
            }
            _ => data.to_vec(),
        })
    }
}

/// Whether a registered format's name is a MIME type's (`type/subtype`, no spaces).
pub fn is_mime_type(name: &str) -> bool {
    let mut parts = name.splitn(2, '/');
    let (Some(a), Some(b)) = (parts.next(), parts.next()) else { return false };
    !a.is_empty() && !b.is_empty() && !name.contains(char::is_whitespace)
}

// ---- The formats' conversions -----------------------------------------------------------------------------------

/// `CF_UNICODETEXT`'s bytes as text (UTF-16LE up to its 0).
pub fn utf16_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&p| u16::from_le_bytes(p)).collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// Text as `CF_UNICODETEXT`'s bytes (UTF-16LE with its 0; line ends as they are).
pub fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

/// Bytes up to their first 0, as UTF-8.
pub fn nul_terminated(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// URIs as a `text/uri-list` (each ended by CRLF: RFC 2483).
pub fn uri_list(uris: &[String]) -> Vec<u8> {
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

/// An image's rows from the bottom, each pixel as BGRA (what a 32-bit DIB holds).
fn bottom_up_bgra(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let image = decode_to_image_bitmap(png)?;
    let (w, h) = (image.width().max(0) as usize, image.height().max(0) as usize);
    let pixels = image.pixels();
    let mut out = Vec::with_capacity(w * h * 4);
    for y in (0..h).rev() {
        for x in 0..w {
            let p = &pixels[(y * w + x) * 4..][..4];
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    Some((w as u32, h as u32, out))
}

/// A PNG as a CF_DIB (C++'s `dibOfPng`): 32 bits a pixel, BI_RGB, the bottom row first, alpha in the fourth byte.
pub fn dib_of_png(png: &[u8]) -> Option<Vec<u8>> {
    let (w, h, pixels) = bottom_up_bgra(png)?;
    let mut dib = Vec::with_capacity(40 + pixels.len());
    for v in [40u32, w, h] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&1u16.to_le_bytes()); // planes
    dib.extend_from_slice(&32u16.to_le_bytes()); // bits a pixel
    for v in [0u32, pixels.len() as u32, 0, 0, 0, 0] {
        // (BI_RGB, the image's size, no resolution, no palette)
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&pixels);
    Some(dib)
}

/// A PNG as a CF_DIBV5: 32 bits a pixel with their masks (BI_BITFIELDS: alpha in the fourth byte, said so — what
/// keeps an image's transparency where a target reads it, as browsers and Office do), sRGB, the bottom row first.
pub fn dibv5_of_png(png: &[u8]) -> Option<Vec<u8>> {
    const BI_BITFIELDS: u32 = 3;
    const LCS_SRGB: u32 = 0x7352_4742; // 'sRGB'
    const LCS_GM_IMAGES: u32 = 4;
    let (w, h, pixels) = bottom_up_bgra(png)?;
    let mut dib = Vec::with_capacity(124 + pixels.len());
    for v in [124u32, w, h] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&1u16.to_le_bytes()); // planes
    dib.extend_from_slice(&32u16.to_le_bytes()); // bits a pixel
    for v in [BI_BITFIELDS, pixels.len() as u32, 0, 0, 0, 0] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    for mask in [0x00ff_0000u32, 0x0000_ff00, 0x0000_00ff, 0xff00_0000, LCS_SRGB] {
        // (red, green, blue, alpha; the color space)
        dib.extend_from_slice(&mask.to_le_bytes());
    }
    dib.extend_from_slice(&[0; 36 + 12]); // (endpoints, gamma: unused for sRGB)
    for v in [LCS_GM_IMAGES, 0, 0, 0] {
        // (the intent; no profile; reserved)
        dib.extend_from_slice(&v.to_le_bytes());
    }
    debug_assert_eq!(dib.len(), 124);
    dib.extend_from_slice(&pixels);
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

// ---- Virtual files ----------------------------------------------------------------------------------------------

/// A FILEDESCRIPTORW's size (shlobj_core.h: its fields, then 260 wide characters of its name).
const FILE_DESCRIPTOR_SIZE: usize = 592;
const FD_ATTRIBUTES: u32 = 0x0004;
const FD_FILESIZE: u32 = 0x0040;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0010;

/// A virtual file a FileGroupDescriptorW describes: its name (a path under the drop's: "folder\file.txt"), whether
/// it is a folder, its size where it says.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VirtualFile {
    pub name: String,
    pub directory: bool,
    pub size: Option<u64>,
}

/// The virtual files a FileGroupDescriptorW (a FILEGROUPDESCRIPTORW) describes, in its order (FileContents' indices).
pub fn virtual_files_of(bytes: &[u8]) -> Vec<VirtualFile> {
    let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let Some(count) = u32_at(0) else { return Vec::new() };
    let mut files = Vec::new();
    for i in 0..count as usize {
        let at = 4 + i * FILE_DESCRIPTOR_SIZE;
        let Some(d) = bytes.get(at..at + FILE_DESCRIPTOR_SIZE) else { break };
        let field = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        let flags = field(0);
        let directory = flags & FD_ATTRIBUTES != 0 && field(36) & FILE_ATTRIBUTE_DIRECTORY != 0;
        let size = (flags & FD_FILESIZE != 0).then(|| (u64::from(field(64)) << 32) | u64::from(field(68)));
        files.push(VirtualFile { name: utf16_text(&d[72..]), directory, size });
    }
    files
}

/// Virtual files as a FileGroupDescriptorW (what a source of them offers: tests).
#[cfg(test)]
pub fn file_group_of(files: &[VirtualFile]) -> Vec<u8> {
    let mut bytes = (files.len() as u32).to_le_bytes().to_vec();
    for f in files {
        let mut d = vec![0u8; FILE_DESCRIPTOR_SIZE];
        let mut flags = FD_ATTRIBUTES;
        if let Some(size) = f.size {
            flags |= FD_FILESIZE;
            d[64..68].copy_from_slice(&((size >> 32) as u32).to_le_bytes());
            d[68..72].copy_from_slice(&(size as u32).to_le_bytes());
        }
        d[0..4].copy_from_slice(&flags.to_le_bytes());
        let attributes: u32 = if f.directory { FILE_ATTRIBUTE_DIRECTORY } else { 0x80 }; // (FILE_ATTRIBUTE_NORMAL)
        d[36..40].copy_from_slice(&attributes.to_le_bytes());
        for (i, u) in f.name.encode_utf16().take(259).enumerate() {
            d[72 + i * 2..74 + i * 2].copy_from_slice(&u.to_le_bytes());
        }
        bytes.extend_from_slice(&d);
    }
    bytes
}

/// A virtual file's name as the parts of a path under the folder it is written to — none where it would leave it
/// (another app's names: "..", a drive, a root, a device's name) or names nothing.
pub fn safe_parts(name: &str) -> Option<Vec<String>> {
    const DEVICES: &[&str] = &["CON", "PRN", "AUX", "NUL"];
    let mut parts = Vec::new();
    for part in name.split(['\\', '/']) {
        if part.is_empty() || part == "." {
            continue;
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let device = DEVICES.contains(&stem.as_str())
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit());
        let forbidden = part.chars().any(|c| matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') || c < ' ');
        if part == ".." || device || forbidden || part.ends_with(['.', ' ']) {
            return None;
        }
        parts.push(part.to_string());
    }
    (!parts.is_empty()).then_some(parts)
}

// ---- Drop effects -----------------------------------------------------------------------------------------------

pub const DROPEFFECT_NONE: u32 = 0;
pub const DROPEFFECT_COPY: u32 = 1;
pub const DROPEFFECT_MOVE: u32 = 2;
pub const DROPEFFECT_LINK: u32 = 4;

// The keys a drag's key state holds (oleidl.h's MK_ bits).
pub const MK_LBUTTON: u32 = 0x01;
pub const MK_RBUTTON: u32 = 0x02;
pub const MK_SHIFT: u32 = 0x04;
pub const MK_CONTROL: u32 = 0x08;
pub const MK_MBUTTON: u32 = 0x10;
pub const MK_ALT: u32 = 0x20;
/// The mouse's buttons among them.
pub const MK_BUTTONS: u32 = MK_LBUTTON | MK_RBUTTON | MK_MBUTTON;

/// The effects a drag allows, of the actions its source allows.
pub fn effects_of(actions: &[DragAndDropTransferAction]) -> u32 {
    actions.iter().fold(DROPEFFECT_NONE, |e, &a| {
        e | if a == DragAndDropTransferAction::Copy {
            DROPEFFECT_COPY
        } else if a == DragAndDropTransferAction::Move {
            DROPEFFECT_MOVE
        } else if a == DragAndDropTransferAction::Link {
            DROPEFFECT_LINK
        } else {
            DROPEFFECT_NONE
        }
    })
}

/// The action a drop took, of its effect (one; none: not taken).
pub fn action_of(effect: u32) -> Option<DragAndDropTransferAction> {
    if effect & DROPEFFECT_MOVE != 0 {
        Some(DragAndDropTransferAction::Move)
    } else if effect & DROPEFFECT_COPY != 0 {
        Some(DragAndDropTransferAction::Copy)
    } else if effect & DROPEFFECT_LINK != 0 {
        Some(DragAndDropTransferAction::Link)
    } else {
        None
    }
}

/// The effect a drop over the app takes of those its source `allowed`, with the keys of `keys` held (Windows'
/// own: Ctrl copies, Shift moves, Ctrl+Shift or Alt links; with none, a copy of another app's, as Linux takes it —
/// a move of the app's own drag back over its window, `own`, as within the app); none if it allows none.
pub fn drop_effect(allowed: u32, keys: u32, own: bool) -> u32 {
    let (ctrl, shift, alt) = (keys & MK_CONTROL != 0, keys & MK_SHIFT != 0, keys & MK_ALT != 0);
    let wanted = match (ctrl, shift) {
        _ if alt => DROPEFFECT_LINK,
        (true, true) => DROPEFFECT_LINK,
        (true, false) => DROPEFFECT_COPY,
        (false, true) => DROPEFFECT_MOVE,
        (false, false) if own => DROPEFFECT_MOVE,
        (false, false) => DROPEFFECT_COPY,
    };
    if allowed & wanted != 0 {
        return wanted;
    }
    let order = if own {
        [DROPEFFECT_MOVE, DROPEFFECT_COPY, DROPEFFECT_LINK]
    } else {
        [DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_LINK]
    };
    order.into_iter().find(|e| allowed & e != 0).unwrap_or(DROPEFFECT_NONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::ImageBitmap;
    use kompoz_ui::clipboard::{ClipEntry, ClipItem, ClipMetadata, ClipValue};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    /// The session's formats as a table here: names registered from 0xC000 on, as Windows numbers them; Latin-1 as
    /// the ANSI code page ('?' past it).
    #[derive(Default)]
    struct Table(RefCell<Vec<String>>);

    impl System for Table {
        fn register(&self, name: &str) -> u32 {
            let mut names = self.0.borrow_mut();
            let at = names.iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or_else(|| {
                names.push(name.to_string());
                names.len() - 1
            });
            0xC000 + at as u32
        }
        fn name(&self, format: u32) -> Option<String> {
            format.checked_sub(0xC000).and_then(|i| self.0.borrow().get(i as usize).cloned())
        }
        fn ansi(&self, text: &str) -> Vec<u8> {
            text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
        }
        fn text_of_ansi(&self, bytes: &[u8]) -> String {
            bytes.iter().map(|&b| char::from(b)).collect()
        }
    }

    /// Another app's data: bytes by format.
    #[derive(Default)]
    struct Data(HashMap<u32, Vec<u8>>, Option<Vec<String>>);

    impl Source for Data {
        fn has(&self, format: u32) -> bool {
            self.0.contains_key(&format)
        }
        fn get(&self, format: u32) -> Option<Vec<u8>> {
            self.0.get(&format).cloned()
        }
        fn virtual_files(&self) -> Option<Vec<String>> {
            self.1.clone()
        }
    }

    fn image() -> ImageBitmap {
        ImageBitmap::from_pixels(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 10, 20, 30, 0]).unwrap()
    }

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
        let rgba = image().pixels().to_vec();
        let png = encode_png(2, 2, &rgba).unwrap();
        let dib = dib_of_png(&png).unwrap();
        assert_eq!(dib.len(), 40 + 16);
        // (The bottom row first: its first pixel blue, BGRA.)
        assert_eq!(&dib[40..44], &[255, 0, 0, 255]);
        let back = decode_to_image_bitmap(&png_of_dib(&dib).unwrap()).unwrap();
        assert_eq!(back.pixels().as_slice(), rgba);
        // A V5 one: its header's masks say where alpha is; read back the same.
        let v5 = dibv5_of_png(&png).unwrap();
        assert_eq!(v5.len(), 124 + 16);
        assert_eq!(u32::from_le_bytes(v5[16..20].try_into().unwrap()), 3); // (BI_BITFIELDS)
        assert_eq!(u32::from_le_bytes(v5[52..56].try_into().unwrap()), 0xff00_0000); // (the alpha mask)
        assert_eq!(&v5[124..128], &[255, 0, 0, 255]);
        let back = decode_to_image_bitmap(&png_of_dib(&v5).unwrap()).unwrap();
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
        // (Up to its 0: global memory is rounded up past it.)
        let mut padded = utf16_bytes("ab");
        padded.extend_from_slice(&[b'x', 0, 0, 0]);
        assert_eq!(utf16_text(&padded), "ab");
        assert!(is_mime_type("application/x-kompoz-fruit"));
        assert!(!is_mime_type("HTML Format"));
        assert!(!is_mime_type("Rich Text Format"));
    }

    #[test]
    fn another_apps_formats_are_read_as_mime_types() {
        let t = Table::default();
        let f = Formats::register(&t);
        let own = t.register("application/x-kompoz-meyve");
        let rtf = t.register("Rich Text Format");
        let formats = [CF_UNICODETEXT, CF_TEXT, f.html, rtf, CF_DIBV5, f.png, CF_HDROP, f.file_group, own, 0];
        assert_eq!(
            f.mime_types(&formats, &t),
            ["text/plain", "text/html", "image/png", "text/uri-list", "application/x-kompoz-meyve"]
        );
        assert_eq!(f.mime_types(&[f.exclude_monitor, CF_UNICODETEXT], &t), [PASSWORD_MANAGER_HINT, "text/plain"]);
        // Each read from its best format; what isn't there from the next.
        let mut data = Data::default();
        data.0.insert(CF_TEXT, b"eski \xfc\0".to_vec());
        data.0.insert(f.html, format!("{}\0", cf_html_of("<b>kalın</b>")).into_bytes());
        let png = encode_png(2, 2, &image().pixels()).unwrap();
        data.0.insert(CF_DIBV5, dibv5_of_png(&png).unwrap());
        data.0.insert(f.url, utf16_bytes("https://kompoz.dev/"));
        data.0.insert(own, vec![1, 2, 3]);
        assert_eq!(f.read(&data, "text/plain", &t).unwrap(), "eski ü".as_bytes());
        assert_eq!(f.read(&data, "text/html", &t).unwrap(), "<b>kalın</b>".as_bytes());
        let read = decode_to_image_bitmap(&f.read(&data, "image/png", &t).unwrap()).unwrap();
        assert_eq!(read.pixels().as_slice(), image().pixels().as_slice());
        assert_eq!(f.read(&data, "text/uri-list", &t).unwrap(), b"https://kompoz.dev/\r\n");
        assert_eq!(f.read(&data, "application/x-kompoz-meyve", &t).unwrap(), [1, 2, 3]);
        assert_eq!(f.read(&data, "image/jpeg", &t), None);
        data.0.insert(CF_UNICODETEXT, utf16_bytes("yeni ğ"));
        assert_eq!(f.read(&data, "text/plain", &t).unwrap(), "yeni ğ".as_bytes());
        // Files: a DROPFILES before the URL; virtual files where there are no others (written out by the source).
        let path = if cfg!(windows) { "C:\\tmp\\a.txt" } else { "/tmp/a.txt" };
        data.0.insert(f.file_group, file_group_of(&[]));
        data.1 = Some(vec![path.into()]);
        let virtual_list = uri_list(&[file_uri(path)]);
        assert_eq!(f.read(&data, "text/uri-list", &t).unwrap(), virtual_list);
        data.0.insert(CF_HDROP, drop_files_of(&[path.into()]));
        if cfg!(windows) {
            // (the same file, from the DROPFILES: a Windows path)
            assert_eq!(f.read(&data, "text/uri-list", &t).unwrap(), virtual_list);
        }
        // KompozR's own list first: it holds every URI.
        data.0.insert(f.uri_list, b"https://a/\r\nhttps://b/\r\n\0".to_vec());
        assert_eq!(f.read(&data, "text/uri-list", &t).unwrap(), b"https://a/\r\nhttps://b/\r\n");
    }

    #[test]
    fn the_apps_payload_is_offered_as_formats_and_rendered() {
        let t = Table::default();
        let f = Formats::register(&t);
        let entry = ClipEntry::new(vec![
            ClipItem::with_html_text("kalın yazı", "<b>kalın</b> yazı"),
            ClipItem::with_image(image()),
            ClipItem::of(vec![("application/x-kompoz-meyve".into(), ClipValue::Bytes(Rc::from(&[1u8, 2, 3][..])))]),
        ]);
        let payload = Payload::of_entry(&entry);
        let own = t.register("application/x-kompoz-meyve");
        let formats = |medium| f.offered(&payload, medium, &t).promised.into_iter().map(|p| p.0).collect::<Vec<_>>();
        // (The clipboard makes CF_TEXT and CF_DIBV5 itself; a drag offers them.)
        assert_eq!(formats(Medium::Clipboard), [CF_UNICODETEXT, f.html, f.png, CF_DIB, own]);
        assert_eq!(formats(Medium::Drag), [CF_UNICODETEXT, CF_TEXT, f.html, f.png, CF_DIBV5, CF_DIB, own]);
        let offered = f.offered(&payload, Medium::Drag, &t);
        let render = |format: u32| {
            let (_, mime) = offered.promised.iter().find(|(p, _)| *p == format).unwrap();
            f.render(&payload, format, mime, &t).unwrap()
        };
        assert_eq!(utf16_text(&render(CF_UNICODETEXT)), "kalın yazı");
        assert_eq!(render(CF_TEXT), b"kal?n yaz?\0");
        assert_eq!(html_of_cf_html(&nul_terminated(&render(f.html))), "<b>kalın</b> yazı");
        for format in [CF_DIBV5, CF_DIB] {
            let back = decode_to_image_bitmap(&png_of_dib(&render(format)).unwrap()).unwrap();
            assert_eq!(back.pixels().as_slice(), image().pixels().as_slice());
        }
        assert_eq!(render(own), [1, 2, 3]);
        assert!(offered.given.is_empty());
        // Files: a DROPFILES and the list; a URL alone: the URL and the list; a sensitive entry: its markers given.
        let path = if cfg!(windows) { "C:\\tmp\\ç.txt" } else { "/tmp/ç.txt" };
        let payload = Payload::of_entry(&ClipEntry::with_files(vec![path.into()]));
        let offered = f.offered(&payload, Medium::Drag, &t);
        assert_eq!(offered.promised, [(CF_HDROP, "text/uri-list".into()), (f.uri_list, "text/uri-list".into())]);
        let drop_files = f.render(&payload, CF_HDROP, "text/uri-list", &t).unwrap();
        assert_eq!(paths_of_drop_files(&drop_files), [path.replace('/', "\\")]);
        let payload = Payload::of_entry(&ClipEntry::with_uris(vec!["https://kompoz.dev/".into()]));
        let offered = f.offered(&payload, Medium::Clipboard, &t);
        assert_eq!(offered.promised[..2], [(f.url, "text/uri-list".into()), (f.uri_list, "text/uri-list".into())]);
        assert_eq!(utf16_text(&f.render(&payload, f.url, "text/uri-list", &t).unwrap()), "https://kompoz.dev/");
        let secret = ClipEntry::with_metadata(
            vec![ClipItem::with_text("parola")],
            ClipMetadata::new(vec!["text/plain".into()]).with_sensitive(true),
        );
        let offered = f.offered(&Payload::of_entry(&secret), Medium::Clipboard, &t);
        assert_eq!(offered.promised, [(CF_UNICODETEXT, "text/plain".into())]);
        let given: Vec<u32> = offered.given.iter().map(|g| g.0).collect();
        assert_eq!(given, [f.exclude_monitor, f.history, f.cloud]);
    }

    #[test]
    fn virtual_files_are_described_and_kept_under_their_folder() {
        let files = vec![
            VirtualFile { name: "Ekler".into(), directory: true, size: None },
            VirtualFile { name: "Ekler\\rapor ğ.pdf".into(), directory: false, size: Some(5_000_000_000) },
            VirtualFile { name: "not.txt".into(), directory: false, size: None },
        ];
        let bytes = file_group_of(&files);
        assert_eq!(bytes.len(), 4 + 3 * 592);
        assert_eq!(virtual_files_of(&bytes), files);
        assert!(virtual_files_of(&bytes[..100]).len() < 3); // (cut short: what is whole)
        assert_eq!(safe_parts("Ekler\\rapor ğ.pdf").unwrap(), ["Ekler", "rapor ğ.pdf"]);
        assert_eq!(safe_parts("./a/b").unwrap(), ["a", "b"]);
        for bad in ["..\\..\\Windows\\x.dll", "C:\\x", "a\\..\\..\\b", "CON", "lpt1.txt", "a:b", "", "\\", "x."] {
            assert_eq!(safe_parts(bad), None, "{bad}");
        }
        assert!(safe_parts("\\\\server\\share\\a.txt").is_some()); // (a UNC name: its parts, under the folder)
    }

    #[test]
    fn drop_effects_follow_the_keys_among_those_allowed() {
        use DragAndDropTransferAction as A;
        let all = effects_of(&[A::Copy, A::Move, A::Link]);
        assert_eq!(all, 7);
        assert_eq!(drop_effect(all, MK_LBUTTON, false), DROPEFFECT_COPY);
        assert_eq!(drop_effect(all, MK_LBUTTON, true), DROPEFFECT_MOVE);
        assert_eq!(drop_effect(all, MK_SHIFT, false), DROPEFFECT_MOVE);
        assert_eq!(drop_effect(all, MK_CONTROL, true), DROPEFFECT_COPY);
        assert_eq!(drop_effect(all, MK_CONTROL | MK_SHIFT, false), DROPEFFECT_LINK);
        assert_eq!(drop_effect(all, MK_ALT, false), DROPEFFECT_LINK);
        // What isn't allowed: the first that is (another app's: copy first; the app's own: move first).
        assert_eq!(drop_effect(DROPEFFECT_MOVE | DROPEFFECT_LINK, 0, false), DROPEFFECT_MOVE);
        assert_eq!(drop_effect(DROPEFFECT_COPY | DROPEFFECT_LINK, MK_SHIFT, true), DROPEFFECT_COPY);
        assert_eq!(drop_effect(DROPEFFECT_NONE, 0, false), DROPEFFECT_NONE);
        assert_eq!(action_of(DROPEFFECT_MOVE), Some(A::Move));
        assert_eq!(action_of(DROPEFFECT_COPY), Some(A::Copy));
        assert_eq!(action_of(DROPEFFECT_LINK), Some(A::Link));
        assert_eq!(action_of(DROPEFFECT_NONE), None);
        assert_eq!(effects_of(&[A::Move]), DROPEFFECT_MOVE);
    }
}
