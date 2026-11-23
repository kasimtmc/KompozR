//! DeepLinkUri.kt and nonAndroidMain's DeepLinkUri.nonAndroid.kt (`ActualUri`, `DeepLinkUriUtils`), InternalUri.kt and
//! UriCodec.kt: a URI as Navigation 3 reads it (Android's `android.net.Uri` in part, as KMP has it).

use kompoz_runtime::kt::{KtSet, Regex, panic_kt, require};
use std::cell::OnceCell;
use std::fmt;
use std::sync::LazyLock;

static QUERY_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new("^[^?#]+\\?([^#]*).*"));
static FRAGMENT_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new("#(.+)"));

/// A URI (Compose's `DeepLinkUri`, see Android's `android.net.Uri`): its scheme, authority, path segments, query and
/// fragment, read from its encoded text when first asked for.
///
/// Kotlin's is an abstract class whose one implementation off Android is the private `ActualUri` (made by
/// [`DeepLinkUri()`](fn@DeepLinkUri)): this is that implementation. Indices in it are its text's bytes (every
/// delimiter is ASCII), where Kotlin counts UTF-16 units: the same places.
#[derive(Clone)]
pub struct DeepLinkUri {
    uri_string: String,
    query: OnceCell<Option<String>>,
    fragment: OnceCell<Option<String>>,
    scheme_separator_index: OnceCell<Option<usize>>,
    path_segments: OnceCell<Vec<String>>,
}

/// A [`DeepLinkUri`] parsing `uri_string` (an RFC 2396-compliant, encoded URI).
/// Kotlin: `DeepLinkUri(uriString: String): DeepLinkUri`
#[allow(non_snake_case)]
pub fn DeepLinkUri(uri_string: &str) -> DeepLinkUri {
    deep_link_uri_utils::parse(uri_string)
}

impl DeepLinkUri {
    /// Kotlin: `DeepLinkUri(uriString: String): DeepLinkUri`
    pub fn new(uri_string: &str) -> DeepLinkUri {
        DeepLinkUri {
            uri_string: uri_string.to_string(),
            query: OnceCell::new(),
            fragment: OnceCell::new(),
            scheme_separator_index: OnceCell::new(),
            path_segments: OnceCell::new(),
        }
    }

    fn ssi(&self) -> Option<usize> {
        *self.scheme_separator_index.get_or_init(|| self.uri_string.find(':'))
    }

    /// The decoded fragment, everything after the `#`.
    /// Kotlin: `getFragment(): String?`
    pub fn get_fragment(&self) -> Option<&str> {
        self.fragment
            .get_or_init(|| FRAGMENT_PATTERN.find(&self.uri_string, 0).map(|m| m.group_values()[1].clone()))
            .as_deref()
    }

    /// The query: after the `?`, before the `#` (`"q=android"` of `"http://www.google.com/search?q=android"`).
    /// Kotlin: `getQuery(): String?`
    pub fn get_query(&self) -> Option<&str> {
        self.query
            .get_or_init(|| {
                QUERY_PATTERN.find(&self.uri_string, 0).and_then(|m| m.groups()[1].as_ref().map(|g| g.value.clone()))
            })
            .as_deref()
    }

    /// The decoded path segments.
    /// Kotlin: `getPathSegments(): List<String>`
    pub fn get_path_segments(&self) -> &[String] {
        self.path_segments.get_or_init(|| {
            let s = &self.uri_string;
            let ssi = self.ssi();
            if let Some(ssi) = ssi {
                if ssi + 1 == s.len() {
                    return Vec::new();
                }
                if s.as_bytes().get(ssi + 1) != Some(&b'/') {
                    return Vec::new();
                }
            }
            internal_uri::parse_path(s, ssi).split('/').map(internal_uri::decode).collect()
        })
    }

    /// Whether it is hierarchical: relative, or its scheme-specific part starting with a `/`.
    /// Kotlin: `isHierarchical(): Boolean`
    pub fn is_hierarchical(&self) -> bool {
        let Some(ssi) = self.ssi() else { return true };
        if self.uri_string.len() == ssi + 1 {
            return false;
        }
        self.uri_string.as_bytes()[ssi + 1] == b'/'
    }

    /// The decoded values of the query's parameters named `key`.
    /// Kotlin: `getQueryParameters(key: String): List<String>`
    ///
    /// # Panics
    /// If it isn't hierarchical (Kotlin's `require`).
    #[track_caller]
    pub fn get_query_parameters(&self, key: &str) -> Vec<String> {
        require(self.is_hierarchical(), None);
        let Some(query) = self.get_query() else { return Vec::new() };
        let encoded_key = internal_uri::encode(key, None);
        query
            .split('&')
            .filter_map(|part| match part.find('=') {
                None => (part == encoded_key).then(String::new),
                Some(i) if part[..i] == encoded_key => Some(internal_uri::decode(&part[i + 1..])),
                Some(_) => None,
            })
            .collect()
    }

    /// The names of the query's parameters, each once, in the order they first come (decoded, but for a parameter
    /// without a `=`, which is as it is).
    /// Kotlin: `getQueryParameterNames(): Set<String>`
    ///
    /// # Panics
    /// If it isn't hierarchical (Kotlin's `require`).
    #[track_caller]
    pub fn get_query_parameter_names(&self) -> KtSet<String> {
        require(self.is_hierarchical(), None);
        let mut names = KtSet::new();
        if let Some(query) = self.get_query() {
            for part in query.split('&') {
                names.add(match part.find('=') {
                    None => part.to_string(),
                    Some(i) => internal_uri::decode(&part[..i]),
                });
            }
        }
        names
    }

    /// The scheme (`"http"`).
    /// Kotlin: `getScheme(): String?`
    pub fn get_scheme(&self) -> Option<&str> {
        self.ssi().map(|ssi| &self.uri_string[..ssi])
    }

    /// The authority: `[ userinfo '@' ] host [ ':' port ]` (`"google.com"`, `"bob@google.com:80"`).
    /// Kotlin: `getAuthority(): String?`
    pub fn get_authority(&self) -> Option<&str> {
        let s = self.uri_string.as_bytes();
        // (Kotlin's index arithmetic, -1 for no scheme)
        let ssi = self.ssi().map_or(-1, |i| i as isize);
        let at = |i: isize| s[i as usize];
        if s.len() as isize > ssi + 2 && at(ssi + 1) == b'/' && at(ssi + 2) == b'/' {
            let start = (ssi + 3) as usize;
            let end = s[start..].iter().position(|c| matches!(c, b'/' | b'\\' | b'?' | b'#')).map(|i| start + i);
            Some(&self.uri_string[start..end.unwrap_or(s.len())])
        } else {
            None
        }
    }
}

/// Its encoded text (`"http://google.com/"`): Kotlin's `toString()`.
impl fmt::Display for DeepLinkUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uri_string)
    }
}

impl fmt::Debug for DeepLinkUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeepLinkUri({:?})", self.uri_string)
    }
}

impl kompoz_runtime::kt::KtToString for DeepLinkUri {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.uri_string);
    }
}

/// Kotlin's internal `DeepLinkUriUtils` (a copy of navigation-common's `NavUriUtils`).
pub(crate) mod deep_link_uri_utils {
    use super::{DeepLinkUri, internal_uri};

    /// `%`-escapes `s`'s characters as UTF-8 octets, but for letters, digits, `_-!.~'()*` and `allow`'s.
    #[allow(dead_code)] // (Kotlin's internal DeepLinkUriUtils.encode, kept as Compose has it)
    pub(crate) fn encode(s: &str, allow: Option<&str>) -> String {
        internal_uri::encode(s, allow)
    }

    /// `s` with its `%`-escaped UTF-8 octets decoded (invalid ones U+FFFD).
    pub(crate) fn decode(s: &str) -> String {
        internal_uri::decode(s)
    }

    /// A [`DeepLinkUri`] of the encoded `uri_string`.
    pub(crate) fn parse(uri_string: &str) -> DeepLinkUri {
        DeepLinkUri::new(uri_string)
    }
}

/// Kotlin's internal `InternalUri` (Android's `android.net.Uri`, partly).
pub(crate) mod internal_uri {
    use super::uri_codec;

    const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

    fn is_allowed(c: char, allow: Option<&str>) -> bool {
        c.is_ascii_alphanumeric() || "_-!.~'()*".contains(c) || allow.is_some_and(|a| a.contains(c))
    }

    /// `%`-escapes `s`'s characters as UTF-8 octets, but for letters, digits, the unreserved `_-!.~'()*` and
    /// `allow`'s.
    pub(crate) fn encode(s: &str, allow: Option<&str>) -> String {
        let mut out = String::with_capacity(s.len());
        let mut buf = [0u8; 4];
        for c in s.chars() {
            if is_allowed(c, allow) {
                out.push(c);
            } else {
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push('%');
                    out.push(HEX_DIGITS[(b >> 4) as usize] as char);
                    out.push(HEX_DIGITS[(b & 0xf) as usize] as char);
                }
            }
        }
        out
    }

    /// `s`'s `%`-escaped octets decoded as UTF-8, an invalid one U+FFFD (`+` kept).
    pub(crate) fn decode(s: &str) -> String {
        uri_codec::decode(s, false, false)
    }

    /// The path of `uri_string`, `ssi` its scheme separator's index (none: a relative URI).
    pub(crate) fn parse_path(uri_string: &str, ssi: Option<usize>) -> &str {
        let s = uri_string.as_bytes();
        let length = s.len() as isize;
        let ssi = ssi.map_or(-1, |i| i as isize);
        let at = |i: isize| s[i as usize];
        let mut path_start;
        if length > ssi + 2 && at(ssi + 1) == b'/' && at(ssi + 2) == b'/' {
            // Past the authority to the path.
            path_start = ssi + 3;
            while path_start < length {
                match at(path_start) {
                    b'?' | b'#' => return "",
                    // (per the URL standard's host state, a `\` in a host is a `/`)
                    b'/' | b'\\' => break,
                    _ => {}
                }
                path_start += 1;
            }
        } else {
            path_start = ssi + 1;
        }
        let mut path_end = path_start;
        while path_end < length && !matches!(at(path_end), b'?' | b'#') {
            path_end += 1;
        }
        &uri_string[path_start as usize..path_end as usize]
    }
}

/// Kotlin's internal `UriCodec` (Android's `android.net.UriCodec`): `application/x-www-form-urlencoded` decoding.
pub(crate) mod uri_codec {
    use super::panic_kt;

    const INVALID_INPUT_CHARACTER: char = '\u{FFFD}';

    /// `s` decoded: `%`-escaped octets as UTF-8; `+` a space if `convert_plus`; an invalid input U+FFFD, or an
    /// IllegalArgumentException if `throw_on_failure`.
    ///
    /// As Android's: after a `%` with a bad digit the U+FFFD is followed by the octet read so far (the digit itself
    /// consumed).
    #[track_caller]
    pub(crate) fn decode(s: &str, convert_plus: bool, throw_on_failure: bool) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(s.len());
        let mut bytes: Vec<u8> = Vec::new();
        let flush = |out: &mut String, bytes: &mut Vec<u8>| {
            if bytes.is_empty() {
                return;
            }
            if throw_on_failure {
                match std::str::from_utf8(bytes) {
                    Ok(text) => out.push_str(text),
                    Err(e) => panic_kt(
                        "MalformedInputException",
                        &format!("Input length = {}", e.error_len().unwrap_or(bytes.len() - e.valid_up_to())),
                    ),
                }
            } else {
                out.push_str(&String::from_utf8_lossy(bytes));
            }
            bytes.clear();
        };
        let syntax = |reason: String, index: usize| -> ! {
            panic_kt(
                "IllegalArgumentException",
                &format!("androidx.navigation3.runtime.deeplink.URISyntaxException: {reason} at index {index}: {s}"),
            )
        };
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            i += 1;
            match c {
                '+' => {
                    flush(&mut out, &mut bytes);
                    out.push(if convert_plus { ' ' } else { '+' });
                }
                '%' => {
                    let mut hex: u8 = 0;
                    for _ in 0..2 {
                        let Some(&d) = chars.get(i) else {
                            if throw_on_failure {
                                syntax("Unexpected end of string".to_string(), i);
                            }
                            flush(&mut out, &mut bytes);
                            out.push(INVALID_INPUT_CHARACTER);
                            return out;
                        };
                        i += 1;
                        let Some(digit) = d.to_digit(16) else {
                            if throw_on_failure {
                                syntax(format!("Unexpected character: {d}"), i - 1);
                            }
                            flush(&mut out, &mut bytes);
                            out.push(INVALID_INPUT_CHARACTER);
                            break;
                        };
                        hex = hex.wrapping_mul(16).wrapping_add(digit as u8);
                    }
                    bytes.push(hex);
                }
                c => {
                    flush(&mut out, &mut bytes);
                    out.push(c);
                }
            }
        }
        flush(&mut out, &mut bytes);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_is_androids() {
        assert_eq!(uri_codec::decode("a%20b+c", false, false), "a b+c");
        assert_eq!(uri_codec::decode("a+c", true, false), "a c");
        assert_eq!(uri_codec::decode("%E2%82%AC", false, false), "€");
        assert_eq!(uri_codec::decode("%", false, false), "\u{FFFD}");
        assert_eq!(uri_codec::decode("a%2", false, false), "a\u{FFFD}");
        assert_eq!(uri_codec::decode("%zz", false, false), "\u{FFFD}\u{0}z");
        assert_eq!(uri_codec::decode("%C3", false, false), "\u{FFFD}");
        assert_eq!(internal_uri::encode("a b/€", None), "a%20b%2F%E2%82%AC");
        assert_eq!(internal_uri::encode("a b/", Some("/")), "a%20b/");
        assert_eq!(internal_uri::encode("_-!.~'()*", None), "_-!.~'()*");
        assert_eq!(deep_link_uri_utils::encode("{x}", None), "%7Bx%7D");
        assert_eq!(deep_link_uri_utils::decode("%7Bx%7D"), "{x}");
        let e = std::panic::catch_unwind(|| uri_codec::decode("a%g0", false, true)).expect_err("a failure");
        assert_eq!(
            e.downcast_ref::<String>().unwrap(),
            "IllegalArgumentException: androidx.navigation3.runtime.deeplink.URISyntaxException: Unexpected \
             character: g at index 2: a%g0"
        );
    }

    #[test]
    fn paths_are_parsed_as_androids() {
        assert_eq!(internal_uri::parse_path("http://a.com/b/c?d#e", Some(4)), "/b/c");
        assert_eq!(internal_uri::parse_path("http://a.com?d", Some(4)), "");
        assert_eq!(internal_uri::parse_path("//a.com/b", None), "/b");
        assert_eq!(internal_uri::parse_path("a/b#c", None), "a/b");
        assert_eq!(internal_uri::parse_path("http://a.com\\b", Some(4)), "\\b");
    }
}
