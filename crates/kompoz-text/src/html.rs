//! AnnotatedString from HTML (Compose's AnnotatedString.fromHtml): the tags Android's Html knows in its compact
//! mode, turned into styles, paragraphs, links and annotations as Compose turns Android's spans.

use crate::annotated_string::{AnnotatedString, Annotation, Range};
use crate::font::{DeviceFontFamilyName, Font, FontFamily, FontStyle, FontWeight, GenericFontFamily};
use crate::link::{LinkAnnotation, LinkInteractionListener, TextLinkStyles};
use crate::paragraph_style::ParagraphStyle;
use crate::span_style::SpanStyle;
use crate::style::{BaselineShift, TextAlign, TextDecoration};
use kompoz_graphics::{Color, TextUnit, TextUnitType};

/// Html's heading sizes, h1 to h6 (em).
const HEADING_SIZES: [f32; 6] = [1.5, 1.4, 1.3, 1.2, 1.1, 1.0];

fn em(v: f32) -> TextUnit {
    TextUnit::new(v, TextUnitType::Em)
}

/// "#rgb", "#rrggbb" or a color's name (the ones Android's Html knows).
fn parse_color(value: &str) -> Option<Color> {
    let v = value.trim().to_ascii_lowercase();
    if let Some(hex) = v.strip_prefix('#') {
        let hex: String = if hex.len() == 3 { hex.chars().flat_map(|c| [c, c]).collect() } else { hex.to_string() };
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        return u32::from_str_radix(&hex, 16).ok().map(|rgb| Color::from_argb(0xFF00_0000 | rgb));
    }
    let rgb = match v.as_str() {
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "red" => 0xFF0000,
        "green" => 0x008000,
        "blue" => 0x0000FF,
        "yellow" => 0xFFFF00,
        "gray" | "grey" => 0x808080,
        "silver" => 0xC0C0C0,
        "maroon" => 0x800000,
        "purple" => 0x800080,
        "fuchsia" | "magenta" => 0xFF00FF,
        "lime" => 0x00FF00,
        "olive" => 0x808000,
        "navy" => 0x000080,
        "teal" => 0x008080,
        "aqua" | "cyan" => 0x00FFFF,
        _ => return None,
    };
    Some(Color::from_argb(0xFF00_0000 | rgb))
}

/// A tag's attributes, in order (`name="value"`, `name='value'`, `name=value`; names lower-cased).
fn attributes(tag: &str) -> Vec<(String, String)> {
    let b = tag.as_bytes();
    let mut out = Vec::new();
    // Past the tag's name.
    let mut i = b.iter().position(|c| !c.is_ascii_alphanumeric()).unwrap_or(b.len());
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let from = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        if from == i {
            break;
        }
        let name = tag[from..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            out.push((name, String::new()));
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
            let quote = b[i];
            let start = i + 1;
            let end = b[start..].iter().position(|&c| c == quote).map_or(b.len(), |p| start + p);
            i = (end + 1).min(b.len());
            &tag[start..end]
        } else {
            let start = i;
            while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
                i += 1;
            }
            &tag[start..i]
        };
        out.push((name, value.to_string()));
    }
    out
}

fn attribute(attributes: &[(String, String)], name: &str) -> String {
    attributes.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()).unwrap_or_default()
}

/// A style attribute's color, background, font weight and style and text decoration (Android's Html's and more),
/// and its text-align (a block's).
fn css_style(css: &str) -> (Option<SpanStyle>, Option<TextAlign>) {
    let mut style = SpanStyle::Default;
    let mut given = false;
    let mut align = None;
    for declaration in css.split(';') {
        let Some((property, value)) = declaration.split_once(':') else { continue };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        match property.as_str() {
            "color" => {
                if let Some(c) = parse_color(&value) {
                    style.color = c;
                    given = true;
                }
            }
            "background-color" | "background" => {
                if let Some(c) = parse_color(&value) {
                    style.background = c;
                    given = true;
                }
            }
            "font-weight" if value == "bold" || value == "bolder" || value.parse::<i32>().is_ok_and(|w| w >= 600) => {
                style.font_weight = Some(FontWeight::Bold);
                given = true;
            }
            "font-style" if value == "italic" => {
                style.font_style = Some(FontStyle::Italic);
                given = true;
            }
            "text-decoration" if value.contains("line-through") => {
                style.text_decoration = Some(TextDecoration::LineThrough);
                given = true;
            }
            "text-decoration" if value.contains("underline") => {
                style.text_decoration = Some(TextDecoration::Underline);
                given = true;
            }
            "text-align" => {
                align = match value.as_str() {
                    "start" => Some(TextAlign::Start),
                    "center" => Some(TextAlign::Center),
                    "end" => Some(TextAlign::End),
                    _ => align,
                }
            }
            _ => {}
        }
    }
    (given.then_some(style), align)
}

/// "&amp;", "&#252;", "&#xE7;"…: the character and how much of `s` (from the '&') it took.
fn entity(s: &str) -> Option<(char, usize)> {
    let semi = s.find(';').filter(|&p| p <= 10)?;
    let name = &s[1..semi];
    if let Some(number) = name.strip_prefix('#') {
        let c = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse::<u32>().ok()?,
        };
        return char::from_u32(c).filter(|&c| c != '\0').map(|c| (c, semi + 1));
    }
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{A0}',
        "copy" => '\u{A9}',
        "reg" => '\u{AE}',
        "hellip" => '\u{2026}',
        "mdash" => '\u{2014}',
        "ndash" => '\u{2013}',
        "laquo" => '\u{AB}',
        "raquo" => '\u{BB}',
        "euro" => '\u{20AC}',
        "trade" => '\u{2122}',
        "deg" => '\u{B0}',
        "middot" => '\u{B7}',
        "bull" => '\u{2022}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        _ => return None,
    };
    Some((c, semi + 1))
}

fn is_block(tag: &str) -> bool {
    matches!(tag, "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "ul" | "ol" | "blockquote" | "pre")
}

/// An element open: its annotations' indices (their ends set when it closes).
struct Open {
    tag: String,
    items: Vec<usize>,
}

/// An annotation being made.
struct Item {
    annotation: Annotation,
    start: usize,
    end: Option<usize>,
    tag: String,
}

impl AnnotatedString {
    /// Styled text from HTML (Compose's AnnotatedString.fromHtml, the tags Android's Html knows in its compact
    /// mode): `<b>`/`<strong>`, `<i>`/`<em>`/`<cite>`/`<dfn>`, `<u>`, `<s>`/`<del>`, `<sup>`/`<sub>`,
    /// `<big>`/`<small>`, `<tt>`/`<code>` (monospace), `<h1>`–`<h6>`, `<font color face>`, `style="color: …;
    /// background-color: …; text-decoration: …; font-weight: …; font-style: …; text-align: …"` (the alignment a
    /// block's paragraph style), `<a href>` (a [`LinkAnnotation::Url`] with `link_styles` and the listener, no look
    /// of its own), `<annotation key="value">` (string annotations), `<br>`, and `<p>`, `<div>`, `<ul>`/`<li>` (a
    /// "• " bullet), `<blockquote>`, `<pre>` each on lines of their own; entities; white space collapsed as HTML
    /// does (but in `<pre>`). Other tags are left out, their text kept; comments go.
    pub fn from_html_with(
        html: &str,
        link_styles: Option<TextLinkStyles>,
        link_interaction_listener: Option<LinkInteractionListener>,
    ) -> AnnotatedString {
        let mut text = String::with_capacity(html.len());
        let mut items: Vec<Item> = Vec::new();
        let mut open: Vec<Open> = Vec::new();
        let mut preformatted = 0;
        // A paragraph's edge: a line of its own (one line break, as Html's compact mode).
        let new_line = |text: &mut String| {
            while text.ends_with(' ') {
                text.pop();
            }
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
        };
        let close = |o: Open, text: &mut String, items: &mut Vec<Item>, preformatted: &mut i32| {
            for i in o.items {
                items[i].end = Some(text.len());
            }
            if o.tag == "pre" {
                *preformatted -= 1;
            }
            if is_block(&o.tag) {
                new_line(text);
            }
        };
        let mut i = 0;
        while i < html.len() {
            let rest = &html[i..];
            if rest.starts_with('<') {
                if rest.starts_with("<!--") {
                    i = rest.find("-->").map_or(html.len(), |e| i + e + 3);
                    continue;
                }
                let Some(end) = rest.find('>') else {
                    text.push('<');
                    i += 1;
                    continue;
                };
                let mut tag = &rest[1..end];
                i += end + 1;
                let closing = tag.starts_with('/');
                if closing {
                    tag = &tag[1..];
                }
                let name_len = tag.bytes().position(|c| !c.is_ascii_alphanumeric()).unwrap_or(tag.len());
                let name = tag[..name_len].to_ascii_lowercase();
                if name.is_empty() {
                    continue;
                }
                if closing {
                    // Closes it and whatever was left open inside it (none open: ignored).
                    if let Some(at) = open.iter().rposition(|o| o.tag == name) {
                        while open.len() > at {
                            let o = open.pop().expect("open");
                            close(o, &mut text, &mut items, &mut preformatted);
                        }
                    }
                    continue;
                }
                if name == "br" {
                    text.push('\n');
                    continue;
                }
                if is_block(&name) {
                    new_line(&mut text);
                }
                let attrs = attributes(tag);
                let start = text.len();
                let mut o = Open { tag: name.clone(), items: Vec::new() };
                let mut add = |annotation: Annotation, tag: String, items: &mut Vec<Item>| {
                    items.push(Item { annotation, start, end: None, tag });
                    o.items.push(items.len() - 1);
                };
                let span = |f: fn(&mut SpanStyle)| {
                    let mut s = SpanStyle::Default;
                    f(&mut s);
                    Annotation::SpanStyle(s)
                };
                match name.as_str() {
                    "b" | "strong" => add(span(|s| s.font_weight = Some(FontWeight::Bold)), String::new(), &mut items),
                    "i" | "em" | "cite" | "dfn" | "var" => {
                        add(span(|s| s.font_style = Some(FontStyle::Italic)), String::new(), &mut items)
                    }
                    "u" | "ins" => {
                        add(span(|s| s.text_decoration = Some(TextDecoration::Underline)), String::new(), &mut items)
                    }
                    "s" | "strike" | "del" => {
                        add(span(|s| s.text_decoration = Some(TextDecoration::LineThrough)), String::new(), &mut items)
                    }
                    "sup" => {
                        add(span(|s| s.baseline_shift = Some(BaselineShift::Superscript)), String::new(), &mut items)
                    }
                    "sub" => {
                        add(span(|s| s.baseline_shift = Some(BaselineShift::Subscript)), String::new(), &mut items)
                    }
                    "big" => add(span(|s| s.font_size = em(1.25)), String::new(), &mut items),
                    "small" => add(span(|s| s.font_size = em(0.8)), String::new(), &mut items),
                    "tt" | "code" | "kbd" | "samp" | "pre" => {
                        add(span(|s| s.font_family = Some(FontFamily::Monospace)), String::new(), &mut items)
                    }
                    "font" => {
                        let mut s = SpanStyle::Default;
                        let mut given = false;
                        if let Some(c) = parse_color(&attribute(&attrs, "color")) {
                            s.color = c;
                            given = true;
                        }
                        let face = attribute(&attrs, "face");
                        if !face.trim().is_empty() {
                            let face = face.trim();
                            s.font_family = Some(match GenericFontFamily::from_name(face) {
                                Some(g) => FontFamily::generic(g),
                                None => FontFamily::new([Font::device(DeviceFontFamilyName::new(face))]),
                            });
                            given = true;
                        }
                        if given {
                            add(Annotation::SpanStyle(s), String::new(), &mut items);
                        }
                    }
                    "a" => {
                        let href = attribute(&attrs, "href");
                        if !href.is_empty() {
                            let link =
                                LinkAnnotation::url_with(href, link_styles.clone(), link_interaction_listener.clone());
                            add(Annotation::Link(Box::new(link)), String::new(), &mut items);
                        }
                    }
                    "annotation" => {
                        for (key, value) in &attrs {
                            if !key.is_empty() && !value.is_empty() {
                                add(Annotation::String(value.clone()), key.clone(), &mut items);
                            }
                        }
                    }
                    _ => {}
                }
                if name.len() == 2 && name.starts_with('h') && (b'1'..=b'6').contains(&name.as_bytes()[1]) {
                    let size = HEADING_SIZES[(name.as_bytes()[1] - b'1') as usize];
                    let s =
                        SpanStyle { font_size: em(size), font_weight: Some(FontWeight::Bold), ..SpanStyle::Default };
                    add(Annotation::SpanStyle(s), String::new(), &mut items);
                }
                if name == "span" || is_block(&name) {
                    let (style, align) = css_style(&attribute(&attrs, "style"));
                    if let Some(s) = style {
                        add(Annotation::SpanStyle(s), String::new(), &mut items);
                    }
                    if let Some(a) = align.filter(|_| is_block(&name)) {
                        let p = ParagraphStyle { text_align: a, ..ParagraphStyle::Default };
                        add(Annotation::ParagraphStyle(p), String::new(), &mut items);
                    }
                }
                if name == "li" {
                    // A bullet, as Html's compact mode.
                    text.push_str("\u{2022} ");
                }
                if name == "pre" {
                    preformatted += 1;
                }
                if tag.ends_with('/') {
                    close(o, &mut text, &mut items, &mut preformatted);
                } else {
                    open.push(o);
                }
                continue;
            }
            if rest.starts_with('&')
                && let Some((c, used)) = entity(rest)
            {
                text.push(c);
                i += used;
                continue;
            }
            let c = rest.chars().next().expect("a character");
            if c.is_ascii_whitespace() && preformatted == 0 {
                // A run of white space is one space, none at a line's start.
                if !text.is_empty() && !text.ends_with(' ') && !text.ends_with('\n') {
                    text.push(' ');
                }
                i += 1;
                continue;
            }
            text.push(c);
            i += c.len_utf8();
        }
        // Still open at the end: closed there.
        while let Some(o) = open.pop() {
            close(o, &mut text, &mut items, &mut preformatted);
        }
        // No line breaks or spaces left at the end.
        while text.ends_with('\n') || text.ends_with(' ') {
            text.pop();
        }
        let length = text.len();
        let annotations = items
            .into_iter()
            .filter_map(|it| {
                let end = it.end.unwrap_or(length).min(length);
                (it.start < end).then(|| Range::tagged(it.annotation, it.start, end, it.tag))
            })
            .collect();
        AnnotatedString::from_annotations(text, annotations)
    }
}

impl AnnotatedString {
    /// Styled text from HTML, its links with no styles or listener (Compose's `fromHtml(htmlString)`): see
    /// [`from_html_with`](AnnotatedString::from_html_with).
    pub fn from_html(html: &str) -> AnnotatedString {
        AnnotatedString::from_html_with(html, None, None)
    }
}
