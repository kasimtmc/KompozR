//! A namespace-aware XML DOM (CMP's `vector.xmldom`: `Element`, `Node`, `NodeList`, `MalformedXMLException` over
//! the JVM's `DocumentBuilder`): a document read whole into elements and text, each element's namespace URI and
//! local name resolved from its prefix (`android:width` in `http://schemas.android.com/apk/res/android`), its
//! attributes by qualified name or by namespace and local name, its text content (its descendants' text, as the
//! DOM's `textContent`). Entities (`&lt;` … `&#x41;`), CDATA sections, comments, processing instructions and a
//! DOCTYPE are read as a DOM parser reads them (the DOCTYPE's internal subset skipped, not applied).
//!
//! What the build reads too: the `compose_resources!` macro shares this file (its `values*/*.xml`).

use std::fmt;

/// The namespace `xml:` names (bound without a declaration).
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
/// The namespace `xmlns:` names.
const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";

/// An XML text that isn't well formed (CMP's MalformedXMLException; the JVM's SAXParseException): what is wrong and
/// where (1-based line and column).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MalformedXMLException {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for MalformedXMLException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (line {}, column {})", self.message, self.line, self.column)
    }
}

impl std::error::Error for MalformedXMLException {}

/// An attribute: its qualified name as written (`android:width`), its namespace (empty: none), its local name, its
/// value with its entities replaced.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Attribute {
    pub name: String,
    pub namespace_uri: String,
    pub local_name: String,
    pub value: String,
}

/// A node of the tree (CMP's Node): an element or a piece of text (character data and CDATA alike).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Node {
    /// Its text content (the DOM's `textContent`): a text's text, an element's descendants' text joined.
    pub fn text_content(&self) -> String {
        match self {
            Node::Element(e) => e.text_content(),
            Node::Text(t) => t.clone(),
        }
    }
    /// Its node name (the DOM's `nodeName`): an element's qualified name, `#text` for text.
    pub fn node_name(&self) -> &str {
        match self {
            Node::Element(e) => &e.node_name,
            Node::Text(_) => "#text",
        }
    }
    pub fn as_element(&self) -> Option<&Element> {
        match self {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        }
    }
}

/// An element (CMP's Element): its qualified name, namespace and local name, the namespaces declared in scope
/// where it is (prefix → URI, the innermost last), its attributes and its children.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Element {
    pub node_name: String,
    pub namespace_uri: String,
    pub local_name: String,
    pub attributes: Vec<Attribute>,
    pub child_nodes: Vec<Node>,
    /// The namespace declarations in scope (prefix, URI; the default namespace's prefix empty), outermost first.
    pub namespaces: Vec<(String, String)>,
}

impl Element {
    /// Attribute `local_name` of namespace `namespace_uri`, empty if it has none (the DOM's `getAttributeNS`).
    pub fn get_attribute_ns(&self, namespace_uri: &str, local_name: &str) -> &str {
        self.attributes
            .iter()
            .find(|a| a.namespace_uri == namespace_uri && a.local_name == local_name)
            .map_or("", |a| a.value.as_str())
    }
    /// Attribute `name` as written (its qualified name), empty if it has none (the DOM's `getAttribute`).
    pub fn get_attribute(&self, name: &str) -> &str {
        self.attributes.iter().find(|a| a.name == name).map_or("", |a| a.value.as_str())
    }
    /// Whether it has attribute `name` (qualified).
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.iter().any(|a| a.name == name)
    }
    /// The prefix bound to `namespace_uri` where it is (the DOM's `lookupPrefix`), the innermost binding; none if
    /// none is (or only the default namespace is).
    pub fn lookup_prefix(&self, namespace_uri: &str) -> Option<&str> {
        self.namespaces.iter().rev().find(|(p, u)| u == namespace_uri && !p.is_empty()).map(|(p, _)| p.as_str())
    }
    /// Its child elements, in order.
    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.child_nodes.iter().filter_map(Node::as_element)
    }
    /// Its text content: its descendants' text, in order (the DOM's `textContent`).
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        fn add(e: &Element, out: &mut String) {
            for n in &e.child_nodes {
                match n {
                    Node::Element(c) => add(c, out),
                    Node::Text(t) => out.push_str(t),
                }
            }
        }
        add(self, &mut out);
        out
    }
}

/// Reads `xml` into its document element (the DOM's `parse(…).documentElement`). Text outside it (other than
/// blanks), a second root, a tag left open or closed by another name, an attribute given twice, an undeclared
/// prefix or an unknown entity make it malformed.
pub fn parse_xml(xml: &str) -> Result<Element, MalformedXMLException> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let mut p = Parser { s: xml, i: 0 };
    p.document()
}

/// The same, of bytes (UTF-8, a BOM skipped).
pub fn parse_xml_bytes(bytes: &[u8]) -> Result<Element, MalformedXMLException> {
    match std::str::from_utf8(bytes) {
        Ok(s) => parse_xml(s),
        Err(e) => {
            let before = &bytes[..e.valid_up_to()];
            let line = before.iter().filter(|b| **b == b'\n').count() + 1;
            Err(MalformedXMLException {
                message: "Invalid byte 1 of a UTF-8 sequence".into(),
                line,
                column: before.len() - before.iter().rposition(|b| *b == b'\n').map_or(0, |p| p + 1) + 1,
            })
        }
    }
}

/// A tag read, before its names are resolved.
struct RawElement {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<RawNode>,
}

enum RawNode {
    Element(RawElement),
    Text(String),
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == ':' || (!c.is_ascii() && !c.is_whitespace())
}

fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-' || c == '.' || c == '\u{b7}'
}

impl<'a> Parser<'a> {
    fn error<T>(&self, message: impl Into<String>) -> Result<T, MalformedXMLException> {
        self.error_at(self.i, message)
    }

    fn error_at<T>(&self, at: usize, message: impl Into<String>) -> Result<T, MalformedXMLException> {
        let before = &self.s[..at.min(self.s.len())];
        let line = before.matches('\n').count() + 1;
        let column = before.chars().rev().take_while(|c| *c != '\n').count() + 1;
        Err(MalformedXMLException { message: message.into(), line, column })
    }

    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn skip_blanks(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }

    /// Up to (and past) `end`; what was before it.
    fn until(&mut self, end: &str, what: &str) -> Result<&'a str, MalformedXMLException> {
        match self.rest().find(end) {
            Some(at) => {
                let text = &self.s[self.i..self.i + at];
                self.i += at + end.len();
                Ok(text)
            }
            None => self.error(format!("XML document structures must start and end within the same entity ({what})")),
        }
    }

    /// Comments, processing instructions, a declaration, a DOCTYPE and blanks, before or after the root.
    fn misc(&mut self, allow_doctype: bool) -> Result<(), MalformedXMLException> {
        loop {
            self.skip_blanks();
            let rest = self.rest();
            if rest.starts_with("<!--") {
                self.i += 4;
                self.until("-->", "comment")?;
            } else if rest.starts_with("<?") {
                self.i += 2;
                self.until("?>", "processing instruction")?;
            } else if allow_doctype && rest.starts_with("<!DOCTYPE") {
                self.doctype()?;
            } else {
                return Ok(());
            }
        }
    }

    /// A DOCTYPE skipped, its internal subset with it.
    fn doctype(&mut self) -> Result<(), MalformedXMLException> {
        let mut depth = 0;
        let mut quote: Option<char> = None;
        while let Some(c) = self.peek() {
            self.i += c.len_utf8();
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, '[') => depth += 1,
                (None, ']') => depth -= 1,
                (None, '>') if depth == 0 => return Ok(()),
                _ => {}
            }
        }
        self.error("XML document structures must start and end within the same entity (DOCTYPE)")
    }

    fn document(&mut self) -> Result<Element, MalformedXMLException> {
        self.misc(true)?;
        if self.peek() != Some('<') {
            return if self.i >= self.s.len() {
                self.error("Premature end of file.")
            } else {
                self.error("Content is not allowed in prolog.")
            };
        }
        let root = self.element()?;
        self.misc(false)?;
        if self.i < self.s.len() {
            return self.error("Content is not allowed in trailing section.");
        }
        let mut scope = vec![("xml".to_string(), XML_NS.to_string()), ("xmlns".to_string(), XMLNS_NS.to_string())];
        resolve(root, &mut scope).map_err(|m| MalformedXMLException { message: m, line: 1, column: 1 })
    }

    fn name(&mut self) -> Result<&'a str, MalformedXMLException> {
        let start = self.i;
        match self.peek() {
            Some(c) if is_name_start(c) => self.i += c.len_utf8(),
            _ => return self.error("A name was expected"),
        }
        while let Some(c) = self.peek() {
            if is_name_char(c) {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        Ok(&self.s[start..self.i])
    }

    /// An element from its `<`.
    fn element(&mut self) -> Result<RawElement, MalformedXMLException> {
        self.i += 1; // '<'
        let name = self.name()?.to_string();
        let mut attributes: Vec<(String, String)> = Vec::new();
        loop {
            let before = self.i;
            self.skip_blanks();
            match self.peek() {
                Some('>') => {
                    self.i += 1;
                    break;
                }
                Some('/') if self.rest().starts_with("/>") => {
                    self.i += 2;
                    return Ok(RawElement { name, attributes, children: Vec::new() });
                }
                Some(_) => {
                    if self.i == before {
                        return self.error(format!("Element type \"{name}\" must be followed by either attribute specifications, \">\" or \"/>\"."));
                    }
                    let at = self.i;
                    let key = self.name()?.to_string();
                    self.skip_blanks();
                    if self.peek() != Some('=') {
                        return self.error(format!("Attribute name \"{key}\" associated with an element type \"{name}\" must be followed by the ' = ' character."));
                    }
                    self.i += 1;
                    self.skip_blanks();
                    let quote = match self.peek() {
                        Some(q @ ('"' | '\'')) => q,
                        _ => return self.error(format!("Open quote is expected for attribute \"{key}\" associated with an  element type  \"{name}\".")),
                    };
                    self.i += 1;
                    let raw = self.until(&quote.to_string(), "attribute value")?;
                    if raw.contains('<') {
                        return self.error(format!("The value of attribute \"{key}\" associated with an element type \"{name}\" must not contain the '<' character."));
                    }
                    let value = self.unescape(raw, true, at)?;
                    if attributes.iter().any(|(k, _)| *k == key) {
                        return self.error_at(
                            at,
                            format!("Attribute \"{key}\" was already specified for element \"{name}\"."),
                        );
                    }
                    attributes.push((key, value));
                }
                None => return self.error("XML document structures must start and end within the same entity."),
            }
        }
        let mut children = Vec::new();
        loop {
            let rest = self.rest();
            if rest.is_empty() {
                return self.error(format!(
                    "The element type \"{name}\" must be terminated by the matching end-tag \"</{name}>\"."
                ));
            }
            if rest.starts_with("</") {
                self.i += 2;
                let at = self.i;
                let end = self.name()?;
                if end != name {
                    return self.error_at(
                        at,
                        format!(
                            "The element type \"{name}\" must be terminated by the matching end-tag \"</{name}>\"."
                        ),
                    );
                }
                self.skip_blanks();
                if self.peek() != Some('>') {
                    return self
                        .error(format!("The end-tag for element type \"{name}\" must end with a '>' delimiter."));
                }
                self.i += 1;
                return Ok(RawElement { name, attributes, children });
            }
            if rest.starts_with("<!--") {
                self.i += 4;
                self.until("-->", "comment")?;
            } else if rest.starts_with("<![CDATA[") {
                self.i += 9;
                let text = self.until("]]>", "CDATA section")?.to_string();
                push_text(&mut children, &text);
            } else if rest.starts_with("<?") {
                self.i += 2;
                self.until("?>", "processing instruction")?;
            } else if rest.starts_with('<') {
                children.push(RawNode::Element(self.element()?));
            } else {
                let at = self.i;
                let end = rest.find('<').unwrap_or(rest.len());
                let raw = &rest[..end];
                self.i += end;
                let text = self.unescape(raw, false, at)?;
                push_text(&mut children, &text);
            }
        }
    }

    /// `raw` with its entity and character references replaced; in an attribute, blanks (tab, CR, LF) are spaces,
    /// as attribute-value normalization makes them.
    fn unescape(&self, raw: &str, attribute: bool, at: usize) -> Result<String, MalformedXMLException> {
        let mut out = String::with_capacity(raw.len());
        let mut rest = raw;
        while let Some(amp) = rest.find('&') {
            push_raw(&mut out, &rest[..amp], attribute);
            let after = &rest[amp + 1..];
            let Some(semi) = after.find(';') else {
                return self.error_at(at, "The entity name must immediately follow the '&' in the entity reference.");
            };
            let entity = &after[..semi];
            let c = match entity {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if entity.starts_with("#x") => u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32),
                _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            match c {
                Some(c) => out.push(c),
                None => {
                    return self.error_at(at, format!("The entity \"{entity}\" was referenced, but not declared."));
                }
            }
            rest = &after[semi + 1..];
        }
        push_raw(&mut out, rest, attribute);
        Ok(out)
    }
}

/// Text as it stands (a CR LF or a lone CR is a LF, as XML's end-of-line handling makes it; an attribute's blanks
/// spaces).
fn push_raw(out: &mut String, raw: &str, attribute: bool) {
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(if attribute { ' ' } else { '\n' });
            }
            '\n' | '\t' if attribute => out.push(' '),
            c => out.push(c),
        }
    }
}

/// Text appended to the last text node, else a new one.
fn push_text(children: &mut Vec<RawNode>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(RawNode::Text(t)) = children.last_mut() {
        t.push_str(text);
    } else {
        children.push(RawNode::Text(text.to_string()));
    }
}

/// `(prefix, local)` of a qualified name.
fn split_name(name: &str) -> (&str, &str) {
    match name.split_once(':') {
        Some((p, l)) => (p, l),
        None => ("", name),
    }
}

/// An element's names resolved in `scope` (its declarations added for itself and its children).
fn resolve(raw: RawElement, scope: &mut Vec<(String, String)>) -> Result<Element, String> {
    let outer = scope.len();
    for (k, v) in &raw.attributes {
        if k == "xmlns" {
            scope.push((String::new(), v.clone()));
        } else if let Some(p) = k.strip_prefix("xmlns:") {
            scope.push((p.to_string(), v.clone()));
        }
    }
    let lookup = |prefix: &str, scope: &[(String, String)]| -> Option<String> {
        scope.iter().rev().find(|(p, _)| p == prefix).map(|(_, u)| u.clone())
    };
    let (prefix, local) = split_name(&raw.name);
    let namespace_uri = match lookup(prefix, scope) {
        Some(u) => u,
        None if prefix.is_empty() => String::new(),
        None => return Err(format!("The prefix \"{prefix}\" for element \"{}\" is not bound.", raw.name)),
    };
    let mut attributes = Vec::with_capacity(raw.attributes.len());
    for (k, v) in raw.attributes {
        let (p, l) = split_name(&k);
        let ns = if k == "xmlns" {
            XMLNS_NS.to_string()
        } else if p.is_empty() {
            String::new() // (unprefixed attributes are in no namespace)
        } else {
            match lookup(p, scope) {
                Some(u) => u,
                None => {
                    return Err(format!(
                        "The prefix \"{p}\" for attribute \"{k}\" associated with an element type \"{}\" is not bound.",
                        raw.name
                    ));
                }
            }
        };
        let local_name = if p.is_empty() { k.clone() } else { l.to_string() };
        attributes.push(Attribute { name: k.clone(), namespace_uri: ns, local_name, value: v });
    }
    let namespaces: Vec<(String, String)> = scope.iter().skip(2).cloned().collect();
    let mut child_nodes = Vec::with_capacity(raw.children.len());
    for c in raw.children {
        child_nodes.push(match c {
            RawNode::Element(e) => Node::Element(resolve(e, scope)?),
            RawNode::Text(t) => Node::Text(t),
        });
    }
    scope.truncate(outer);
    Ok(Element {
        node_name: raw.name.clone(),
        namespace_uri,
        local_name: local.to_string(),
        attributes,
        child_nodes,
        namespaces,
    })
}
