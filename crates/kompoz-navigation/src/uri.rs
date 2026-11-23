//! URIs as navigation reads them (Compose's `NavUri`, Android's `Uri` on Android — whose parsing the other
//! platforms' `NavUri` follows): a string kept as it is, its parts found when asked — the scheme up to the first
//! `:`, the fragment after the first `#`, the query between `?` and `#`, the path's segments decoded — and
//! `NavUriUtils`' encoding (`%XX` for all but `A–Z a–z 0–9 _-!.~'()*`) and decoding (UTF-8, a malformed escape read
//! as U+FFFD).

use std::fmt;

/// A URI (Compose's `NavUri`): made from its text with [`NavUri()`](fn@NavUri) (or `NavUri::new`), read in parts.
/// Two are equal when their texts are.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NavUri {
    text: String,
}

/// The URI `uri_string` (Compose's `NavUri(uriString)`).
#[allow(non_snake_case)]
pub fn NavUri(uri_string: impl Into<String>) -> NavUri {
    NavUri::new(uri_string)
}

impl fmt::Display for NavUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for NavUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavUri({:?})", self.text)
    }
}

impl From<&str> for NavUri {
    fn from(s: &str) -> NavUri {
        NavUri::new(s)
    }
}

impl From<String> for NavUri {
    fn from(s: String) -> NavUri {
        NavUri::new(s)
    }
}

impl NavUri {
    /// The URI `uri_string` (Android's `Uri.parse`: nothing is checked, its parts are found when read).
    pub fn new(uri_string: impl Into<String>) -> NavUri {
        NavUri { text: uri_string.into() }
    }

    /// Its text, as it was given (Kotlin's `toString()`).
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Where its scheme ends: the first `:` (Android's), if any.
    fn scheme_separator(&self) -> Option<usize> {
        self.text.find(':')
    }

    fn fragment_separator(&self) -> Option<usize> {
        let from = self.scheme_separator().unwrap_or(0);
        self.text[from..].find('#').map(|i| i + from)
    }

    /// Its scheme (`kompoz` of `kompoz://item/1`), if it has one.
    pub fn get_scheme(&self) -> Option<String> {
        self.scheme_separator().map(|i| self.text[..i].to_string())
    }

    /// Whether its scheme-specific part is a path (`scheme:/…` or no scheme), not opaque (`mailto:a@b`).
    pub fn is_hierarchical(&self) -> bool {
        match self.scheme_separator() {
            None => true,
            Some(ssi) => self.text[ssi + 1..].starts_with('/'),
        }
    }

    /// Its path as written (still encoded), if it is hierarchical.
    pub fn get_encoded_path(&self) -> Option<String> {
        let ssi = self.scheme_separator();
        let t = self.text.as_bytes();
        let after = ssi.map_or(0, |s| s + 1);
        if let Some(s) = ssi
            && (s + 1 == t.len() || t[s + 1] != b'/')
        {
            return None;
        }
        let mut start = after;
        if t.len() > after + 1 && t[after] == b'/' && t[after + 1] == b'/' {
            // (an authority first: up to the path's '/', or nothing before a query or fragment)
            start = after + 2;
            loop {
                match t.get(start) {
                    None => break,
                    Some(b'?' | b'#') => return Some(String::new()),
                    Some(b'/' | b'\\') => break,
                    _ => start += 1,
                }
            }
        }
        let end = t[start..].iter().position(|c| *c == b'?' || *c == b'#').map_or(t.len(), |e| e + start);
        Some(self.text[start..end].to_string())
    }

    /// Its path decoded.
    pub fn get_path(&self) -> Option<String> {
        self.get_encoded_path().map(|p| decode(&p))
    }

    /// Its path's segments, decoded (empty ones left out: `/a//b/` is `a`, `b`).
    pub fn get_path_segments(&self) -> Vec<String> {
        let Some(path) = self.get_encoded_path() else { return Vec::new() };
        path.split('/').filter(|s| !s.is_empty()).map(decode).collect()
    }

    /// Its query as written (between the `?` and the `#`), if it has one.
    pub fn get_encoded_query(&self) -> Option<String> {
        let from = self.scheme_separator().unwrap_or(0);
        let qsi = self.text[from..].find('?')? + from;
        match self.fragment_separator() {
            None => Some(self.text[qsi + 1..].to_string()),
            Some(fsi) if fsi < qsi => None,
            Some(fsi) => Some(self.text[qsi + 1..fsi].to_string()),
        }
    }

    /// Its query decoded.
    pub fn get_query(&self) -> Option<String> {
        self.get_encoded_query().map(|q| decode(&q))
    }

    /// Its fragment decoded (after the `#`), if it has one.
    pub fn get_fragment(&self) -> Option<String> {
        self.fragment_separator().map(|f| decode(&self.text[f + 1..]))
    }

    /// The query's `name=value` pairs as written.
    fn query_pairs(&self) -> Vec<(String, Option<String>)> {
        if !self.is_hierarchical() {
            return Vec::new();
        }
        let Some(query) = self.get_encoded_query() else { return Vec::new() };
        query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (pair.to_string(), None),
            })
            .collect()
    }

    /// The values of the query's parameter `key`, decoded (a `+` a space), in order (empty: none; an opaque URI
    /// has none — Android's throws).
    pub fn get_query_parameters(&self, key: &str) -> Vec<String> {
        let encoded_key = encode(key, None);
        self.query_pairs()
            .into_iter()
            .filter(|(name, _)| *name == encoded_key)
            .map(|(_, value)| value.map(|v| decode_plus(&v)).unwrap_or_default())
            .collect()
    }

    /// The first value of `key`, decoded.
    pub fn get_query_parameter(&self, key: &str) -> Option<String> {
        self.get_query_parameters(key).into_iter().next()
    }

    /// The query's parameter names, decoded, each once, in order.
    pub fn get_query_parameter_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for (name, _) in self.query_pairs() {
            let name = decode(&name);
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }
}

/// What [`encode`] leaves as it is.
fn unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_-!.~'()*".contains(c)
}

/// `s` with what a URI can't hold as it is written `%XX` (Android's `Uri.encode(s, allow)`: all but letters,
/// digits, `_-!.~'()*` and the characters of `allow`; UTF-8 bytes, upper-case hex).
pub fn encode(s: &str, allow: Option<&str>) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if unreserved(c) || allow.is_some_and(|a| a.contains(c)) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// `s` with its `%XX` escapes read as UTF-8 (Android's `Uri.decode`; a malformed one is U+FFFD).
pub fn decode(s: &str) -> String {
    decode_with(s, false)
}

/// The same, a `+` read as a space (a query parameter's value).
fn decode_plus(s: &str) -> String {
    decode_with(s, true)
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

fn decode_with(s: &str, plus_as_space: bool) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut pending: Vec<u8> = Vec::new();
    let flush = |pending: &mut Vec<u8>, out: &mut String| {
        if !pending.is_empty() {
            out.push_str(&String::from_utf8_lossy(pending));
            pending.clear();
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            match (bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
                (Some(h), Some(l)) => {
                    pending.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                _ => {
                    flush(&mut pending, &mut out);
                    out.push('\u{FFFD}');
                    // (Android's skips what it couldn't read of the escape)
                    i += 1 + bytes[i + 1..].iter().take(2).take_while(|b| hex(**b).is_some()).count();
                    continue;
                }
            }
        }
        flush(&mut pending, &mut out);
        let c = s[i..].chars().next().expect("a character");
        out.push(if plus_as_space && c == '+' { ' ' } else { c });
        i += c.len_utf8();
    }
    flush(&mut pending, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_are_androids() {
        let u = NavUri("kompoz://item/4%202/x?q=a%20b+c&q=d&e#top%21");
        assert_eq!(u.get_scheme().as_deref(), Some("kompoz"));
        assert_eq!(u.get_path_segments(), ["4 2", "x"]);
        assert_eq!(u.get_query_parameters("q"), ["a b c", "d"]);
        assert_eq!(u.get_query_parameters("e"), [""]);
        assert_eq!(u.get_query_parameter_names(), ["q", "e"]);
        assert_eq!(u.get_query().as_deref(), Some("q=a b+c&q=d&e"));
        assert_eq!(u.get_fragment().as_deref(), Some("top!"));
        let relative = NavUri("item/42?tab=1");
        assert_eq!(relative.get_path_segments(), ["item", "42"]);
        assert_eq!(relative.get_query_parameter("tab").as_deref(), Some("1"));
        let opaque = NavUri("mailto:a@b?x=1");
        assert!(!opaque.is_hierarchical() && opaque.get_path_segments().is_empty());
        assert!(opaque.get_query_parameters("x").is_empty());
        assert_eq!(NavUri("https://example.com?q=1").get_encoded_path().as_deref(), Some(""));
    }

    #[test]
    fn encoding_is_androids() {
        assert_eq!(encode("a b/c?ğ", None), "a%20b%2Fc%3F%C4%9F");
        assert_eq!(encode("a/b", Some("/")), "a/b");
        assert_eq!(encode("x_-!.~'()*", None), "x_-!.~'()*");
        assert_eq!(decode("a%20b%2Fc%C4%9F+"), "a b/cğ+");
        assert_eq!(decode("%zz%4"), "\u{FFFD}zz\u{FFFD}");
    }
}
