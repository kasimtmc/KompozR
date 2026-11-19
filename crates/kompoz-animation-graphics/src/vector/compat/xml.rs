//! Android's XML resources as its resource compiler leaves them and its `XmlPullParser` reads them: a document's
//! elements (namespaces resolved; comments, declarations and text dropped; entities decoded), each `<aapt:attr>`
//! taken out of its parent and kept as an inline resource of the attribute it names (as aapt2 extracts it into a
//! file of its own), and a pull parser over the elements' start and end tags (Compose's XmlPullParserUtils on it:
//! `seek_to_start_tag`, `for_each_child_of`, `is_at_end`).

use super::resources::XmlResourceError;
use std::ops::Range;
use std::rc::Rc;

/// Android's namespace (`xmlns:android`).
pub(crate) const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
/// aapt's namespace (`xmlns:aapt`), of `<aapt:attr>`.
pub(crate) const AAPT_NS: &str = "http://schemas.android.com/aapt";
/// The XML namespace (`xml:` attributes).
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// An attribute: its namespace (empty: none), local name and value (entities decoded).
#[derive(Clone, Debug)]
pub(crate) struct XmlAttribute {
    pub(crate) namespace: String,
    pub(crate) name: String,
    pub(crate) value: String,
}

/// An element as the resource compiler leaves it: its local name (what Compose's parsers match), its attributes,
/// the inline resources of `<aapt:attr>` children (by the attribute's namespace and name), its other child
/// elements, and where its text is in the document's (an inline resource's text is read again by what reads it
/// whole).
#[derive(Debug)]
pub(crate) struct XmlElement {
    pub(crate) name: String,
    pub(crate) attributes: Vec<XmlAttribute>,
    pub(crate) inline: Vec<(String, String, Rc<XmlElement>)>,
    pub(crate) children: Vec<Rc<XmlElement>>,
    pub(crate) source: Rc<str>,
    pub(crate) span: Range<usize>,
}

impl XmlElement {
    /// Its attribute `name` in `namespace`.
    pub(crate) fn attribute(&self, namespace: &str, name: &str) -> Option<&str> {
        self.attributes.iter().find(|a| a.namespace == namespace && a.name == name).map(|a| a.value.as_str())
    }

    /// The inline resource of its attribute `name` in `namespace` (an `<aapt:attr>`'s element).
    pub(crate) fn inline(&self, namespace: &str, name: &str) -> Option<&Rc<XmlElement>> {
        self.inline.iter().find(|(ns, n, _)| ns == namespace && n == name).map(|(_, _, e)| e)
    }

    /// Its text in its document.
    pub(crate) fn text(&self) -> &str {
        &self.source[self.span.clone()]
    }
}

fn malformed(message: impl Into<String>) -> XmlResourceError {
    XmlResourceError::new("XmlPullParserException", message)
}

/// An element being read: its namespace declarations in scope, its prefixed name, its attributes as written.
struct Open {
    prefixed: String,
    scope: Vec<(String, String)>,
    attributes: Vec<(String, String)>,
    inline: Vec<(String, String, Rc<XmlElement>)>,
    children: Vec<Rc<XmlElement>>,
    start: usize,
}

/// Reads a document: its root element. Malformed XML (an unclosed or mismatched tag, an unclosed comment, an
/// attribute without a quoted value, a prefix bound to no namespace but `android:`/`aapt:`, an `<aapt:attr>` without
/// one element in it), or none: an error, as Android's resource compiler refuses it.
pub(crate) fn read_document(text: &str) -> Result<Rc<XmlElement>, XmlResourceError> {
    let source: Rc<str> = Rc::from(text);
    let b = text.as_bytes();
    let mut i = 0;
    // (a byte order mark)
    if text.starts_with('\u{feff}') {
        i = 3;
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut root: Option<Rc<XmlElement>> = None;
    while i < b.len() {
        let Some(lt) = text[i..].find('<').map(|p| p + i) else {
            if !stack.is_empty() {
                return Err(malformed("unexpected end of document: an element isn't closed"));
            }
            break;
        };
        // (text between tags: dropped; outside the root only blanks may be)
        if stack.is_empty() && !text[i..lt].trim().is_empty() {
            return Err(malformed("text outside the root element"));
        }
        i = lt;
        let rest = &text[i..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            let end = comment.find("-->").ok_or_else(|| malformed("unclosed comment"))?;
            i += 4 + end + 3;
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            if stack.is_empty() {
                return Err(malformed("text outside the root element"));
            }
            let end = rest.find("]]>").ok_or_else(|| malformed("unclosed CDATA section"))?;
            i += end + 3;
            continue;
        }
        if rest.starts_with("<?") {
            let end = rest.find("?>").ok_or_else(|| malformed("unclosed processing instruction"))?;
            i += end + 2;
            continue;
        }
        if rest.starts_with("<!") {
            let end = rest.find('>').ok_or_else(|| malformed("unclosed declaration"))?;
            i += end + 1;
            continue;
        }
        if rest.starts_with("</") {
            let mut j = i + 2;
            let name = read_name(text, &mut j);
            skip_blanks(b, &mut j);
            if j >= b.len() || b[j] != b'>' {
                return Err(malformed(format!("malformed end tag </{name}")));
            }
            let open = stack.pop().ok_or_else(|| malformed(format!("end tag </{name}> without a start tag")))?;
            if open.prefixed != name {
                return Err(malformed(format!("end tag </{name}> does not match <{}>", open.prefixed)));
            }
            j += 1;
            i = j;
            if let Some(done) = close(open, &source, j, &mut stack)? {
                if root.is_some() {
                    return Err(malformed("more than one root element"));
                }
                root = Some(done);
            }
            continue;
        }
        // A start tag.
        let start = i;
        let mut j = i + 1;
        let prefixed = read_name(text, &mut j);
        if prefixed.is_empty() {
            return Err(malformed("a tag without a name"));
        }
        if stack.is_empty() && root.is_some() {
            return Err(malformed("more than one root element"));
        }
        let mut attributes: Vec<(String, String)> = Vec::new();
        let self_closing = loop {
            skip_blanks(b, &mut j);
            if j >= b.len() {
                return Err(malformed(format!("unclosed tag <{prefixed}")));
            }
            if b[j] == b'>' {
                j += 1;
                break false;
            }
            if text[j..].starts_with("/>") {
                j += 2;
                break true;
            }
            let key = read_name(text, &mut j);
            if key.is_empty() {
                return Err(malformed(format!("malformed attribute in <{prefixed}>")));
            }
            skip_blanks(b, &mut j);
            if j >= b.len() || b[j] != b'=' {
                return Err(malformed(format!("attribute {key} without a value")));
            }
            j += 1;
            skip_blanks(b, &mut j);
            if j >= b.len() || (b[j] != b'"' && b[j] != b'\'') {
                return Err(malformed(format!("attribute {key}'s value isn't quoted")));
            }
            let quote = b[j] as char;
            j += 1;
            let end = text[j..].find(quote).map(|p| p + j).ok_or_else(|| malformed("unclosed attribute value"))?;
            let value = decode_entities(&text[j..end])?;
            if attributes.iter().any(|(k, _)| *k == key) {
                return Err(malformed(format!("duplicate attribute {key}")));
            }
            attributes.push((key, value));
            j = end + 1;
        };
        let mut scope = stack.last().map(|o| o.scope.clone()).unwrap_or_default();
        for (k, v) in &attributes {
            if k == "xmlns" {
                scope.push((String::new(), v.clone()));
            } else if let Some(p) = k.strip_prefix("xmlns:") {
                scope.push((p.to_string(), v.clone()));
            }
        }
        let open = Open { prefixed, scope, attributes, inline: Vec::new(), children: Vec::new(), start };
        i = j;
        if self_closing {
            if let Some(done) = close(open, &source, j, &mut stack)? {
                if root.is_some() {
                    return Err(malformed("more than one root element"));
                }
                root = Some(done);
            }
        } else {
            stack.push(open);
        }
    }
    if !stack.is_empty() {
        return Err(malformed("unexpected end of document: an element isn't closed"));
    }
    root.ok_or_else(|| malformed("No start tag found"))
}

/// An element read to its end: made, then given to its parent (an `<aapt:attr>`: its element an inline resource of
/// the parent's attribute it names); the root if it has no parent.
fn close(
    open: Open,
    source: &Rc<str>,
    end: usize,
    stack: &mut [Open],
) -> Result<Option<Rc<XmlElement>>, XmlResourceError> {
    let (namespace, name) = resolve(&open.prefixed, &open.scope, true)?;
    let mut attributes = Vec::new();
    for (k, v) in &open.attributes {
        if k == "xmlns" || k.starts_with("xmlns:") {
            continue;
        }
        let (ns, n) = resolve(k, &open.scope, false)?;
        attributes.push(XmlAttribute { namespace: ns, name: n, value: v.clone() });
    }
    if namespace == AAPT_NS && name == "attr" {
        let Some(parent) = stack.last_mut() else {
            return Err(malformed("<aapt:attr> as the root element"));
        };
        let target = attributes
            .iter()
            .find(|a| a.namespace.is_empty() && a.name == "name")
            .ok_or_else(|| malformed("<aapt:attr> without a name"))?;
        let (ns, n) = resolve(&target.value, &open.scope, false)?;
        let [element] = &open.children[..] else {
            return Err(malformed(format!("<aapt:attr name=\"{}\"> must hold one element", target.value)));
        };
        if parent.inline.iter().any(|(pns, pn, _)| *pns == ns && *pn == n) {
            return Err(malformed(format!("duplicate attribute {}", target.value)));
        }
        parent.inline.push((ns, n, element.clone()));
        return Ok(None);
    }
    let element = Rc::new(XmlElement {
        name,
        attributes,
        inline: open.inline,
        children: open.children,
        source: source.clone(),
        span: open.start..end,
    });
    // (an attribute both written and inline: as aapt2, an error)
    for (ns, n, _) in &element.inline {
        if element.attribute(ns, n).is_some() {
            return Err(malformed(format!("duplicate attribute {n}")));
        }
    }
    match stack.last_mut() {
        Some(parent) => {
            parent.children.push(element);
            Ok(None)
        }
        None => Ok(Some(element)),
    }
}

/// A prefixed name's namespace and local name in `scope` (an element without a prefix: the default namespace's;
/// an attribute's: none). `android:` and `aapt:` undeclared are Android's and aapt's.
fn resolve(prefixed: &str, scope: &[(String, String)], element: bool) -> Result<(String, String), XmlResourceError> {
    let Some((prefix, local)) = prefixed.split_once(':') else {
        let ns = if element {
            scope.iter().rev().find(|(p, _)| p.is_empty()).map(|(_, u)| u.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        return Ok((ns, prefixed.to_string()));
    };
    if let Some((_, uri)) = scope.iter().rev().find(|(p, _)| p == prefix) {
        return Ok((uri.clone(), local.to_string()));
    }
    let uri = match prefix {
        "android" => ANDROID_NS,
        "aapt" => AAPT_NS,
        "xml" => XML_NS,
        _ => return Err(malformed(format!("unbound prefix {prefix} in {prefixed}"))),
    };
    Ok((uri.to_string(), local.to_string()))
}

fn skip_blanks(b: &[u8], j: &mut usize) {
    while *j < b.len() && b[*j].is_ascii_whitespace() {
        *j += 1;
    }
}

/// A name: up to a blank, `=`, `>` or `/`.
fn read_name(text: &str, j: &mut usize) -> String {
    let b = text.as_bytes();
    let start = *j;
    while *j < b.len() && !b[*j].is_ascii_whitespace() && !matches!(b[*j], b'=' | b'>' | b'/' | b'<') {
        *j += 1;
    }
    text[start..*j].to_string()
}

/// XML's entities decoded: the five named ones and character references.
fn decode_entities(s: &str) -> Result<String, XmlResourceError> {
    if !s.contains('&') {
        return Ok(s.to_string());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let semi = after.find(';').ok_or_else(|| malformed(format!("unterminated entity in \"{s}\"")))?;
        let entity = &after[..semi];
        let c = match entity {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                code.and_then(char::from_u32).ok_or_else(|| malformed(format!("unknown entity &{entity};")))?
            }
        };
        out.push(c);
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

// ---- The pull parser ------------------------------------------------------------------------------------------

/// What a pull parser is at (XmlPullParser's event types, those a compiled resource has for elements).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum XmlEvent {
    StartDocument,
    StartTag,
    EndTag,
    EndDocument,
}

/// A pull parser over an element and what is in it (Android's XmlResourceParser of a compiled resource, as far as
/// Compose's parsers use it): at the document's start first; `next` goes to each element's start tag and end tag
/// in order, then to the document's end.
pub(crate) struct XmlPullParser {
    events: Rc<[(XmlEvent, Option<Rc<XmlElement>>, usize)]>,
    at: usize,
}

impl XmlPullParser {
    /// A parser over `root` (a resource's element) at the document's start.
    pub(crate) fn new(root: Rc<XmlElement>) -> XmlPullParser {
        fn walk(e: &Rc<XmlElement>, depth: usize, out: &mut Vec<(XmlEvent, Option<Rc<XmlElement>>, usize)>) {
            out.push((XmlEvent::StartTag, Some(e.clone()), depth));
            for c in &e.children {
                walk(c, depth + 1, out);
            }
            out.push((XmlEvent::EndTag, Some(e.clone()), depth));
        }
        let mut events = vec![(XmlEvent::StartDocument, None, 0)];
        walk(&root, 1, &mut events);
        events.push((XmlEvent::EndDocument, None, 0));
        XmlPullParser { events: events.into(), at: 0 }
    }

    /// The next event (the document's end stays where it is).
    pub(crate) fn next(&mut self) -> XmlEvent {
        if self.at + 1 < self.events.len() {
            self.at += 1;
        }
        self.event_type()
    }

    pub(crate) fn event_type(&self) -> XmlEvent {
        self.events[self.at].0
    }

    /// The tag's local name (none at the document's start or end).
    pub(crate) fn name(&self) -> &str {
        self.events[self.at].1.as_ref().map_or("", |e| e.name.as_str())
    }

    /// The tag's depth (the root's 1; 0 at the document's start and end).
    pub(crate) fn depth(&self) -> usize {
        self.events[self.at].2
    }

    /// The element whose tag it is at (Compose's AttributeSet of the parser: its attributes).
    pub(crate) fn element(&self) -> &Rc<XmlElement> {
        self.events[self.at].1.as_ref().expect("a parser at a tag")
    }

    /// Compose's `isAtEnd`: at the document's end, or at an end tag above the root.
    pub(crate) fn is_at_end(&self) -> bool {
        self.event_type() == XmlEvent::EndDocument || (self.depth() < 1 && self.event_type() == XmlEvent::EndTag)
    }

    /// Compose's `seekToStartTag`: on to the first start tag; none: "No start tag found".
    pub(crate) fn seek_to_start_tag(mut self) -> Result<XmlPullParser, XmlResourceError> {
        let mut t = self.next();
        while t != XmlEvent::StartTag && t != XmlEvent::EndDocument {
            t = self.next();
        }
        if t != XmlEvent::StartTag {
            return Err(malformed("No start tag found"));
        }
        Ok(self)
    }

    /// Compose's `forEachChildOf(tag, action)`: at the start of `tag`, `action` at each event (of every depth: what
    /// `action` doesn't read itself) until `tag`'s end tag.
    pub(crate) fn for_each_child_of(
        &mut self,
        tag: &str,
        mut action: impl FnMut(&mut XmlPullParser) -> Result<(), XmlResourceError>,
    ) -> Result<(), XmlResourceError> {
        self.next();
        while !self.is_at_end() {
            if self.event_type() == XmlEvent::EndTag && self.name() == tag {
                break;
            }
            action(self)?;
            self.next();
        }
        Ok(())
    }
}
