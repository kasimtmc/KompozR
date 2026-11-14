//! Just enough XML for vector drawables (the C++ Kompoz's reader): elements, their attributes (a prefix like
//! `android:` dropped), nesting; comments, declarations and text skipped; and their numbers and colors as Android
//! writes them.

use crate::color::Color;
use std::collections::BTreeMap;

/// One tag: its name (without a prefix), its attributes, and whether it closes an element or is one on its own.
struct XmlTag {
    name: String,
    attributes: BTreeMap<String, String>,
    closing: bool,
    self_closing: bool,
}

/// The tags of a text, one after another.
struct XmlReader<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> XmlReader<'a> {
    fn new(s: &'a str) -> XmlReader<'a> {
        XmlReader { s, i: 0 }
    }

    fn bytes(&self) -> &'a [u8] {
        self.s.as_bytes()
    }

    /// The next tag; none at the end, or where it can't be read.
    fn next_tag(&mut self) -> Option<XmlTag> {
        loop {
            let lt = self.i + self.s[self.i..].find('<')?;
            self.i = lt;
            let rest = &self.s[self.i..];
            if rest.starts_with("<!--") {
                self.i += rest.find("-->")? + 3;
                continue;
            }
            if rest.starts_with("<?") || rest.starts_with("<!") {
                self.i += rest.find('>')? + 1;
                continue;
            }
            break;
        }
        let b = self.bytes();
        self.i += 1;
        let closing = self.i < b.len() && b[self.i] == b'/';
        if closing {
            self.i += 1;
        }
        let mut tag = XmlTag {
            name: strip_prefix(self.name()).into(),
            attributes: BTreeMap::new(),
            closing,
            self_closing: false,
        };
        loop {
            self.skip_space();
            if self.i >= b.len() {
                return None;
            }
            if b[self.i] == b'>' {
                self.i += 1;
                return Some(tag);
            }
            if self.s[self.i..].starts_with("/>") {
                self.i += 2;
                tag.self_closing = true;
                return Some(tag);
            }
            let key = strip_prefix(self.name()).to_string();
            if key.is_empty() {
                return None;
            }
            self.skip_space();
            if self.i >= b.len() || b[self.i] != b'=' {
                return None;
            }
            self.i += 1;
            self.skip_space();
            if self.i >= b.len() || (b[self.i] != b'"' && b[self.i] != b'\'') {
                return None;
            }
            let quote = b[self.i] as char;
            self.i += 1;
            let end = self.i + self.s[self.i..].find(quote)?;
            // (entities: none in what is read)
            tag.attributes.insert(key, self.s[self.i..end].to_string());
            self.i = end + 1;
        }
    }

    fn skip_space(&mut self) {
        let b = self.bytes();
        while self.i < b.len() && b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    /// A name: up to a blank, `=`, `>` or `/`.
    fn name(&mut self) -> &'a str {
        let b = self.bytes();
        let start = self.i;
        while self.i < b.len() && !b[self.i].is_ascii_whitespace() && !matches!(b[self.i], b'=' | b'>' | b'/') {
            self.i += 1;
        }
        &self.s[start..self.i]
    }
}

/// A name without its prefix (`android:width`: `width`).
fn strip_prefix(n: &str) -> &str {
    n.find(':').map_or(n, |colon| &n[colon + 1..])
}

/// An XML element with its attributes and children (a vector drawable, an animated one).
#[derive(Clone, Debug, Default)]
pub(crate) struct XmlNode {
    pub(crate) name: String,
    pub(crate) attributes: BTreeMap<String, String>,
    pub(crate) children: Vec<XmlNode>,
}

impl XmlNode {
    /// Its attribute `key`, empty if it has none.
    pub(crate) fn attribute(&self, key: &str) -> &str {
        self.attributes.get(key).map_or("", String::as_str)
    }

    /// The first `element` in it (it too), depth first.
    pub(crate) fn find(&self, element: &str) -> Option<&XmlNode> {
        if self.name == element {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(element))
    }

    /// The first element in it (it too) whose `android:name` is `name`.
    pub(crate) fn named(&self, name: &str) -> Option<&XmlNode> {
        if self.attribute("name") == name {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.named(name))
    }
}

/// The first element of `xml` read whole, its children in it; none if there is none.
pub(crate) fn read_xml_tree(xml: &str) -> Option<XmlNode> {
    let mut reader = XmlReader::new(xml);
    let mut open: Vec<XmlNode> = Vec::new();
    // An element done: the root if nothing is open, else its parent's child.
    let done = |open: &mut Vec<XmlNode>, node: XmlNode| match open.last_mut() {
        None => Some(node),
        Some(parent) => {
            parent.children.push(node);
            None
        }
    };
    while let Some(tag) = reader.next_tag() {
        let root = if tag.closing {
            let Some(node) = open.pop() else { continue };
            done(&mut open, node)
        } else if tag.self_closing {
            done(&mut open, XmlNode { name: tag.name, attributes: tag.attributes, children: Vec::new() })
        } else {
            open.push(XmlNode { name: tag.name, attributes: tag.attributes, children: Vec::new() });
            None
        };
        if root.is_some() {
            return root;
        }
    }
    None
}

/// The number `s` starts with, as C's strtof reads a decimal one (leading blanks skipped; a sign, digits with a
/// point among or before them, an exponent; `inf`, `infinity`, `nan`): "24dp" is 24. None where it starts with none.
pub(crate) fn leading_float(s: &str) -> Option<f32> {
    let t = s.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let lower = t[i..].to_ascii_lowercase();
    for word in ["infinity", "inf", "nan"] {
        if lower.starts_with(word) {
            let negative = b.first() == Some(&b'-');
            let v = if word == "nan" { f32::NAN } else { f32::INFINITY };
            return Some(if negative { -v } else { v });
        }
    }
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] | 0x20) == b'e' {
        let mut e = i + 1;
        if e < b.len() && (b[e] == b'+' || b[e] == b'-') {
            e += 1;
        }
        if e < b.len() && b[e].is_ascii_digit() {
            while e < b.len() && b[e].is_ascii_digit() {
                e += 1;
            }
            i = e;
        }
    }
    t[..i].parse::<f32>().ok()
}

/// Attribute `key` of `a` as a number, `fallback` if it has none or it isn't one.
pub(crate) fn xml_number(a: &BTreeMap<String, String>, key: &str, fallback: f32) -> f32 {
    a.get(key).and_then(|v| leading_float(v)).unwrap_or(fallback)
}

/// A color as Android writes one: `#RGB`, `#ARGB`, `#RRGGBB` or `#AARRGGBB`; a reference (`@color/…`, `?attr/…`)
/// black, as it isn't resolved here; none if it is empty or isn't one.
pub(crate) fn xml_color(v: &str) -> Option<Color> {
    let hex = v.strip_prefix('#');
    if v.is_empty() {
        return None;
    }
    let Some(hex) = hex else { return Some(Color::Black) };
    if !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut hex: String = if hex.len() == 3 || hex.len() == 4 {
        // (each digit twice)
        hex.chars().flat_map(|c| [c, c]).collect()
    } else {
        hex.to_string()
    };
    if hex.len() == 6 {
        hex.insert_str(0, "ff");
    }
    if hex.len() != 8 {
        return None;
    }
    u32::from_str_radix(&hex, 16).ok().map(Color::from_argb)
}

/// Attribute `key` of `a` as a color.
pub(crate) fn xml_color_of(a: &BTreeMap<String, String>, key: &str) -> Option<Color> {
    a.get(key).and_then(|v| xml_color(v))
}
