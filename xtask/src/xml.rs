//! A small XML reader, enough for the Wayland protocols' files (std only, as every task here): elements with their
//! attributes, text, comments, CDATA, the declaration, the five predefined entities and character references. No
//! DTDs, namespaces or encodings other than UTF-8.

/// An element: its name, its attributes in order, its children.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Node>,
}

/// What an element holds.
#[derive(Clone, PartialEq, Debug)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    /// The attribute `name`'s value, if it has one.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// The child elements, in order.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// The child elements named `name`.
    pub fn elements_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.elements().filter(move |e| e.name == name)
    }

    /// The first child element named `name`.
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }

    /// Its text (its text children's, joined).
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) => Some(t.as_str()),
                Node::Element(_) => None,
            })
            .collect()
    }
}

/// The document's root element; why not, with the line it went wrong on.
pub fn parse(text: &str) -> Result<Element, String> {
    let mut p = Parser { s: text.as_bytes(), at: 0, text };
    p.misc()?;
    if p.peek() != Some(b'<') {
        return Err(p.error("no root element"));
    }
    let root = p.element()?;
    p.misc()?;
    if p.at < p.s.len() {
        return Err(p.error("content after the root element"));
    }
    Ok(root)
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    text: &'a str,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        let line = self.s[..self.at.min(self.s.len())].iter().filter(|&&b| b == b'\n').count() + 1;
        format!("line {line}: {what}")
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn starts(&self, prefix: &str) -> bool {
        self.s[self.at..].starts_with(prefix.as_bytes())
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    /// Skips past `end` (after what it ends: a comment's, a declaration's).
    fn skip_past(&mut self, end: &str) -> Result<(), String> {
        match self.text[self.at..].find(end) {
            Some(i) => {
                self.at += i + end.len();
                Ok(())
            }
            None => Err(self.error(&format!("no {end}"))),
        }
    }

    /// Whitespace, comments, processing instructions and a doctype, between elements at the top.
    fn misc(&mut self) -> Result<(), String> {
        loop {
            self.skip_space();
            if self.starts("<?") {
                self.skip_past("?>")?;
            } else if self.starts("<!--") {
                self.skip_past("-->")?;
            } else if self.starts("<!DOCTYPE") {
                self.skip_past(">")?;
            } else {
                return Ok(());
            }
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':')) {
            self.at += 1;
        }
        if self.at == start {
            return Err(self.error("a name expected"));
        }
        Ok(self.text[start..self.at].to_string())
    }

    fn element(&mut self) -> Result<Element, String> {
        self.at += 1; // '<'
        let mut element = Element { name: self.name()?, ..Element::default() };
        loop {
            self.skip_space();
            match self.peek() {
                Some(b'/') => {
                    if !self.starts("/>") {
                        return Err(self.error("/> expected"));
                    }
                    self.at += 2;
                    return Ok(element);
                }
                Some(b'>') => {
                    self.at += 1;
                    break;
                }
                Some(_) => {
                    let name = self.name()?;
                    self.skip_space();
                    if self.peek() != Some(b'=') {
                        return Err(self.error("= expected"));
                    }
                    self.at += 1;
                    self.skip_space();
                    let quote =
                        self.peek().filter(|&q| q == b'"' || q == b'\'').ok_or_else(|| self.error("a quote"))?;
                    self.at += 1;
                    let start = self.at;
                    while self.peek().is_some_and(|b| b != quote) {
                        self.at += 1;
                    }
                    if self.peek().is_none() {
                        return Err(self.error("an unended attribute"));
                    }
                    let value = unescape(&self.text[start..self.at]).map_err(|e| self.error(&e))?;
                    self.at += 1;
                    element.attributes.push((name, value));
                }
                None => return Err(self.error("an unended tag")),
            }
        }
        // Content, until the end tag.
        let mut text = String::new();
        loop {
            if self.at >= self.s.len() {
                return Err(self.error(&format!("<{}> isn't ended", element.name)));
            }
            if self.starts("</") {
                self.at += 2;
                let name = self.name()?;
                if name != element.name {
                    return Err(self.error(&format!("</{name}> ends <{}>", element.name)));
                }
                self.skip_space();
                if self.peek() != Some(b'>') {
                    return Err(self.error("> expected"));
                }
                self.at += 1;
                if !text.is_empty() {
                    element.children.push(Node::Text(text));
                }
                return Ok(element);
            }
            if self.starts("<!--") {
                self.skip_past("-->")?;
            } else if self.starts("<![CDATA[") {
                let start = self.at + 9;
                self.skip_past("]]>")?;
                text.push_str(&self.text[start..self.at - 3]);
            } else if self.starts("<?") {
                self.skip_past("?>")?;
            } else if self.peek() == Some(b'<') {
                if !text.is_empty() {
                    element.children.push(Node::Text(std::mem::take(&mut text)));
                }
                let child = self.element()?;
                element.children.push(Node::Element(child));
            } else {
                let start = self.at;
                while self.peek().is_some_and(|b| b != b'<') {
                    self.at += 1;
                }
                text.push_str(&unescape(&self.text[start..self.at]).map_err(|e| self.error(&e))?);
            }
        }
    }
}

/// Text with its entities and character references replaced.
fn unescape(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let semi = after.find(';').ok_or("an unended entity")?;
        let entity = &after[..semi];
        let c = match entity {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse().ok()
                } else {
                    None
                };
                code.and_then(char::from_u32).ok_or_else(|| format!("an unknown entity &{entity};"))?
            }
        };
        out.push(c);
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_attributes_and_text_are_read() {
        let doc = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- a comment -->\n<protocol name=\"p\">\n  \
                   <copyright>a &amp; b</copyright>\n  <interface name='i' version=\"3\">\
                   <description summary=\"&lt;x&gt; &#x41;&#66;\">some <![CDATA[<raw>]]> text</description>\
                   <request name=\"r\"/><!-- between --><event name=\"e\" since=\"2\"/></interface>\n</protocol>\n";
        let root = parse(doc).unwrap();
        assert_eq!(root.name, "protocol");
        assert_eq!(root.attribute("name"), Some("p"));
        assert_eq!(root.element("copyright").unwrap().text(), "a & b");
        let interface = root.element("interface").unwrap();
        assert_eq!(interface.attribute("version"), Some("3"));
        let description = interface.element("description").unwrap();
        assert_eq!(description.attribute("summary"), Some("<x> AB"));
        assert_eq!(description.text(), "some <raw> text");
        let names: Vec<&str> = interface.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["description", "request", "event"]);
        assert_eq!(interface.element("event").unwrap().attribute("since"), Some("2"));
    }

    #[test]
    fn broken_documents_say_where() {
        assert!(parse("<a><b></a>").unwrap_err().contains("</a> ends <b>"));
        assert!(parse("<a x=\"1></a>").is_err());
        assert!(parse("<a>&nope;</a>").unwrap_err().contains("&nope;"));
        assert!(parse("<a></a><b/>").unwrap_err().contains("after the root"));
        assert!(parse("\n\n<a>").unwrap_err().starts_with("line 3"));
    }
}
