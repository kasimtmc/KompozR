//! JSON, the server's own (the everything-ours rule: no serde_json, no lsp-types): a value tree that keeps an
//! object's members in the order they came (a settings file written back reads as its author wrote it), a parser of
//! RFC 8259's grammar, and writers (compact for the wire, indented for files).
//!
//! Reading is forgiving the way LSP code wants it: indexing a missing member or a non-object gives `null`
//! (`msg["params"]["textDocument"]["uri"].str()` is `""` when any step is missing), the accessors give a default.

use std::fmt::Write as _;

/// A JSON value. Numbers are integers when written without a fraction or exponent and they fit an `i64` (LSP's
/// are: positions, ids, token data), floats otherwise.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Json {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    /// Its members in order (a key at most once: [`Json::set`] replaces).
    Object(Vec<(String, Json)>),
}

static NULL: Json = Json::Null;

impl Json {
    /// An object of these members, in this order.
    pub fn object<const N: usize>(members: [(&str, Json); N]) -> Json {
        Json::Object(members.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// An empty object.
    pub fn new_object() -> Json {
        Json::Object(Vec::new())
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Json::Object(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Json::Array(_))
    }

    /// A member of an object (none of anything else).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Whether an object has the member `key` (`null` or not).
    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Object(m) => m.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The member `key`, made (`null`) when missing; a `null` value becomes an object first. Anything else but an
    /// object is replaced by one.
    pub fn entry(&mut self, key: &str) -> &mut Json {
        if !self.is_object() {
            *self = Json::new_object();
        }
        let Json::Object(m) = self else { unreachable!() };
        let i = match m.iter().position(|(k, _)| k == key) {
            Some(i) => i,
            None => {
                m.push((key.to_string(), Json::Null));
                m.len() - 1
            }
        };
        &mut m[i].1
    }

    /// Sets the member `key` (replacing it in its place, or added last).
    pub fn set(&mut self, key: &str, value: impl Into<Json>) {
        *self.entry(key) = value.into();
    }

    /// Removes the member `key`.
    pub fn remove(&mut self, key: &str) -> Option<Json> {
        match self {
            Json::Object(m) => m.iter().position(|(k, _)| k == key).map(|i| m.remove(i).1),
            _ => None,
        }
    }

    /// Adds an element (a `null` becomes an array first).
    pub fn push(&mut self, value: impl Into<Json>) {
        if self.is_null() {
            *self = Json::Array(Vec::new());
        }
        if let Json::Array(a) = self {
            a.push(value.into());
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The string, `""` for anything else.
    pub fn str(&self) -> &str {
        self.as_str().unwrap_or("")
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(i) => Some(*i),
            Json::Float(f) if f.fract() == 0.0 && f.is_finite() => Some(*f as i64),
            _ => None,
        }
    }

    /// The integer, `0` for anything else.
    pub fn int(&self) -> i64 {
        self.as_i64().unwrap_or(0)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The elements of an array, none of anything else.
    pub fn array(&self) -> &[Json] {
        match self {
            Json::Array(a) => a,
            _ => &[],
        }
    }

    pub fn array_mut(&mut self) -> Option<&mut Vec<Json>> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The members of an object, none of anything else.
    pub fn members(&self) -> &[(String, Json)] {
        match self {
            Json::Object(m) => m,
            _ => &[],
        }
    }

    pub fn members_mut(&mut self) -> Option<&mut Vec<(String, Json)>> {
        match self {
            Json::Object(m) => Some(m),
            _ => None,
        }
    }

    // ---- Writing ---------------------------------------------------------------------------------------------

    /// The value as compact JSON (the wire's).
    pub fn dump(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None, 0);
        out
    }

    /// The value as JSON indented by `indent` spaces a level (a file's).
    pub fn pretty(&self, indent: usize) -> String {
        let mut out = String::new();
        self.write(&mut out, Some(indent), 0);
        out
    }

    fn write(&self, out: &mut String, indent: Option<usize>, level: usize) {
        let newline = |out: &mut String, level: usize| {
            if let Some(n) = indent {
                out.push('\n');
                out.extend(std::iter::repeat_n(' ', n * level));
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Json::Float(f) => {
                if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 {
                    // (a float stays one when read back)
                    let _ = write!(out, "{f:.1}");
                } else if f.is_finite() {
                    let _ = write!(out, "{f}");
                } else {
                    out.push_str("null");
                }
            }
            Json::Str(s) => write_string(out, s),
            Json::Array(a) => {
                if a.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, level + 1);
                    v.write(out, indent, level + 1);
                }
                newline(out, level);
                out.push(']');
            }
            Json::Object(m) => {
                if m.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, level + 1);
                    write_string(out, k);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    v.write(out, indent, level + 1);
                }
                newline(out, level);
                out.push('}');
            }
        }
    }

    // ---- Reading ---------------------------------------------------------------------------------------------

    /// The value `text` holds (one value, whitespace around it).
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { s: text.as_bytes(), i: 0, depth: 0 };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(p.error("text after the value"));
        }
        Ok(v)
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl std::ops::Index<&str> for Json {
    type Output = Json;
    /// The member `key`, `null` when missing (or not an object).
    fn index(&self, key: &str) -> &Json {
        self.get(key).unwrap_or(&NULL)
    }
}

impl std::ops::Index<usize> for Json {
    type Output = Json;
    /// The element `i`, `null` when missing (or not an array).
    fn index(&self, i: usize) -> &Json {
        self.array().get(i).unwrap_or(&NULL)
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Json {
        Json::Str(s.to_string())
    }
}
impl From<String> for Json {
    fn from(s: String) -> Json {
        Json::Str(s)
    }
}
impl From<&String> for Json {
    fn from(s: &String) -> Json {
        Json::Str(s.clone())
    }
}
impl From<bool> for Json {
    fn from(b: bool) -> Json {
        Json::Bool(b)
    }
}
impl From<i64> for Json {
    fn from(i: i64) -> Json {
        Json::Int(i)
    }
}
impl From<i32> for Json {
    fn from(i: i32) -> Json {
        Json::Int(i64::from(i))
    }
}
impl From<u32> for Json {
    fn from(i: u32) -> Json {
        Json::Int(i64::from(i))
    }
}
impl From<usize> for Json {
    fn from(i: usize) -> Json {
        Json::Int(i as i64)
    }
}
impl From<f64> for Json {
    fn from(f: f64) -> Json {
        Json::Float(f)
    }
}
impl From<Vec<Json>> for Json {
    fn from(a: Vec<Json>) -> Json {
        Json::Array(a)
    }
}
impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(o: Option<T>) -> Json {
        o.map_or(Json::Null, Into::into)
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        format!("{what} at byte {}", self.i)
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.i) {
            self.i += 1;
        }
    }

    fn eat(&mut self, word: &str) -> bool {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.s.get(self.i) {
            None => Err(self.error("a value expected")),
            Some(b'n') if self.eat("null") => Ok(Json::Null),
            Some(b't') if self.eat("true") => Ok(Json::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.nest()?;
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Json::Array(a));
                }
                loop {
                    self.ws();
                    a.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            self.depth -= 1;
                            return Ok(Json::Array(a));
                        }
                        _ => return Err(self.error("`,` or `]` expected")),
                    }
                }
            }
            Some(b'{') => {
                self.nest()?;
                self.i += 1;
                let mut m: Vec<(String, Json)> = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Json::Object(m));
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&b'"') {
                        return Err(self.error("a member's name expected"));
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return Err(self.error("`:` expected"));
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value()?;
                    match m.iter_mut().find(|(key, _)| *key == k) {
                        Some(slot) => slot.1 = v,
                        None => m.push((k, v)),
                    }
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            self.depth -= 1;
                            return Ok(Json::Object(m));
                        }
                        _ => return Err(self.error("`,` or `}` expected")),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("a value expected")),
        }
    }

    /// One level deeper (a bound: a hostile message can't overflow the stack).
    fn nest(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > 512 { Err(self.error("nested too deeply")) } else { Ok(()) }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        let digits = |p: &mut Self| {
            let from = p.i;
            while p.s.get(p.i).is_some_and(u8::is_ascii_digit) {
                p.i += 1;
            }
            p.i > from
        };
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        if !digits(self) {
            return Err(self.error("a digit expected"));
        }
        let mut float = false;
        if self.s.get(self.i) == Some(&b'.') {
            self.i += 1;
            float = true;
            if !digits(self) {
                return Err(self.error("a digit expected"));
            }
        }
        if let Some(b'e' | b'E') = self.s.get(self.i) {
            self.i += 1;
            float = true;
            if let Some(b'+' | b'-') = self.s.get(self.i) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.error("a digit expected"));
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.error("a number expected"))?;
        if !float && let Ok(i) = text.parse::<i64>() {
            return Ok(Json::Int(i));
        }
        text.parse::<f64>().map(Json::Float).map_err(|_| self.error("a number expected"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.s.get(self.i..self.i + 4).ok_or_else(|| self.error("four hex digits expected"))?;
        let h = std::str::from_utf8(h).map_err(|_| self.error("four hex digits expected"))?;
        let v = u32::from_str_radix(h, 16).map_err(|_| self.error("four hex digits expected"))?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(&b) = self.s.get(self.i) else { return Err(self.error("an unterminated string")) };
            self.i += 1;
            match b {
                b'"' => return String::from_utf8(out).map_err(|_| self.error("a string that isn't UTF-8")),
                b'\\' => {
                    let Some(&e) = self.s.get(self.i) else { return Err(self.error("an unterminated string")) };
                    self.i += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) && self.eat("\\u") {
                                let lo = self.hex4()?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    0xFFFD
                                }
                            } else {
                                hi
                            };
                            char::from_u32(code).unwrap_or('\u{FFFD}')
                        }
                        _ => return Err(self.error("an unknown escape")),
                    };
                    let mut buf = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                b if b < 0x20 => return Err(self.error("a control character in a string")),
                b => out.push(b),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let text = r#"{"a":1,"b":[true,false,null,-2.5,1e3],"c":{"d":"x\"y\\z\n\u00e7\ud83d\ude00"},"e":[],"f":{}}"#;
        let v = Json::parse(text).unwrap();
        assert_eq!(v["a"].int(), 1);
        assert_eq!(v["b"][3], Json::Float(-2.5));
        assert_eq!(v["b"][4], Json::Float(1000.0));
        assert_eq!(v["c"]["d"].str(), "x\"y\\z\nç😀");
        assert!(v["missing"]["deeper"].is_null());
        assert_eq!(Json::parse(&v.dump()).unwrap(), v);
        assert_eq!(Json::parse(&v.pretty(2)).unwrap(), v);
        assert_eq!(Json::object([("k", 1.into())]).pretty(2), "{\n  \"k\": 1\n}");
    }

    #[test]
    fn members_keep_their_order() {
        let mut v = Json::parse(r#"{"z":1,"a":2}"#).unwrap();
        v.set("m", "x");
        v.set("z", 3);
        assert_eq!(v.dump(), r#"{"z":3,"a":2,"m":"x"}"#);
        v.entry("n").set("deep", true);
        assert_eq!(v["n"]["deep"], Json::Bool(true));
        assert_eq!(v.remove("a"), Some(Json::Int(2)));
    }

    #[test]
    fn mistakes() {
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "\"\\x\"", "01x", "[1] 2", "{not}", "\"a\u{1}\""] {
            assert!(Json::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Json::parse(&"[".repeat(10_000)).is_err());
    }
}
