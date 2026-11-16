//! Kotlin's `String` and `Char` functions ([`KtStr`], [`KtChar`]) with Kotlin's meanings: lengths and indices in
//! UTF-16 units (`"😀".length` is 2), whitespace and digits as Java's `Character` classes them, numbers read as
//! `toInt()`/`toDouble()` read them.
//!
//! An index inside a surrogate pair (half of a character outside the BMP, which a Rust `char` can't be) cuts it:
//! the half becomes U+FFFD, so lengths stay Kotlin's.

use super::panic_kt;
use super::range::{KtContains, KtProgression};
use super::text::{FormatArg, KtToString};

/// What a string is searched for: a string (`&str`, `String`) or a `Char`.
pub trait KtNeedle {
    fn kt_needle(&self) -> std::borrow::Cow<'_, str>;
}
impl KtNeedle for str {
    fn kt_needle(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(self)
    }
}
impl KtNeedle for String {
    fn kt_needle(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(self)
    }
}
impl KtNeedle for char {
    fn kt_needle(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Owned(self.to_string())
    }
}
impl<N: KtNeedle + ?Sized> KtNeedle for &N {
    fn kt_needle(&self) -> std::borrow::Cow<'_, str> {
        (**self).kt_needle()
    }
}

/// What `split` splits at: one delimiter or several (Kotlin's `vararg delimiters`), strings or chars.
pub trait KtDelimiters {
    fn kt_delimiters(&self) -> Vec<String>;
}
impl<N: KtNeedle> KtDelimiters for N {
    fn kt_delimiters(&self) -> Vec<String> {
        vec![self.kt_needle().into_owned()]
    }
}
impl<N: KtNeedle> KtDelimiters for [N] {
    fn kt_delimiters(&self) -> Vec<String> {
        self.iter().map(|n| n.kt_needle().into_owned()).collect()
    }
}
impl<N: KtNeedle, const K: usize> KtDelimiters for [N; K] {
    fn kt_delimiters(&self) -> Vec<String> {
        self.iter().map(|n| n.kt_needle().into_owned()).collect()
    }
}
impl<N: KtNeedle> KtDelimiters for &[N] {
    fn kt_delimiters(&self) -> Vec<String> {
        self.iter().map(|n| n.kt_needle().into_owned()).collect()
    }
}
impl<N: KtNeedle> KtDelimiters for Vec<N> {
    fn kt_delimiters(&self) -> Vec<String> {
        self.iter().map(|n| n.kt_needle().into_owned()).collect()
    }
}

// ---- UTF-16 positions ----

/// The UTF-16 length of `s` (Kotlin's `length`).
pub(crate) fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// `s` from UTF-16 unit `from` to `to` (both within it), a cut surrogate pair's half as U+FFFD.
fn utf16_slice(s: &str, from: usize, to: usize) -> String {
    let mut out = String::new();
    let mut at = 0;
    for c in s.chars() {
        let n = c.len_utf16();
        let (start, end) = (at, at + n);
        at = end;
        if end <= from {
            continue;
        }
        if start >= to {
            break;
        }
        if start >= from && end <= to {
            out.push(c);
        } else {
            out.push('\u{FFFD}');
        }
    }
    out
}

/// The UTF-16 index of byte offset `byte` in `s`.
fn utf16_index(s: &str, byte: usize) -> i32 {
    utf16_len(&s[..byte]) as i32
}

/// The byte offset of the first character at or after UTF-16 index `index` (past a pair `index` is inside).
fn byte_of(s: &str, index: usize) -> usize {
    let mut at = 0;
    for (b, c) in s.char_indices() {
        if at >= index {
            return b;
        }
        at += c.len_utf16();
    }
    s.len()
}

/// Lower-cases a string for comparing without case, as Kotlin's `ignoreCase` (char by char).
fn fold(s: &str) -> String {
    s.chars().flat_map(|c| c.to_lowercase()).collect()
}

/// The byte offsets where `needle` starts in `hay` from byte `from`, `ignore_case` as Kotlin's (both folded the same
/// way, positions mapped back by characters).
fn find_from(hay: &str, needle: &str, from: usize, ignore_case: bool, last: bool) -> Option<usize> {
    if !ignore_case {
        return if last {
            hay.rfind(needle).filter(|&i| i >= from)
        } else {
            hay[from..].find(needle).map(|i| i + from)
        };
    }
    let needle: Vec<char> = fold(needle).chars().collect();
    let chars: Vec<(usize, char)> = hay.char_indices().collect();
    let lower: Vec<char> = chars.iter().map(|(_, c)| c.to_lowercase().next().unwrap_or(*c)).collect();
    let mut found = None;
    for i in 0..=chars.len() {
        let b = chars.get(i).map_or(hay.len(), |c| c.0);
        if b < from {
            continue;
        }
        if i + needle.len() <= lower.len() && lower[i..i + needle.len()] == needle[..] {
            found = Some(b);
            if !last {
                break;
            }
        }
    }
    found
}

#[track_caller]
fn check_range(start: i32, end: i32, length: usize) {
    if start < 0 || end > length as i32 || start > end {
        panic_kt("StringIndexOutOfBoundsException", &format!("begin {start}, end {end}, length {length}"));
    }
}

// ---- Java's Character classes ----

/// The zero of each run of ten decimal digits (Unicode's Nd: what Java's `Character.isDigit` says yes to).
const DIGIT_ZEROS: &[u32] = &[
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66, 0x0DE6,
    0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40,
    0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10, 0x104A0, 0x10D30, 0x11066, 0x110F0,
    0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50, 0x11D50,
    0x11DA0, 0x11F50, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0,
    0x1E4F0, 0x1E950, 0x1FBF0,
];

/// The decimal digit `c` is (Java's `Character.digit(c, 10)` of an Nd character), or none.
fn decimal_digit(c: char) -> Option<u32> {
    let v = c as u32;
    if c.is_ascii_digit() {
        return Some(v - 0x30);
    }
    DIGIT_ZEROS.iter().find(|&&z| v >= z && v < z + 10).map(|z| v - z)
}

/// The digit `c` is in `radix` (2..=36): a decimal digit, or a Latin letter (ASCII or fullwidth) past 9.
fn digit_in(c: char, radix: u32) -> Option<u32> {
    let d = decimal_digit(c).or_else(|| {
        let v = c as u32;
        match v {
            0x41..=0x5A => Some(v - 0x41 + 10),
            0x61..=0x7A => Some(v - 0x61 + 10),
            0xFF21..=0xFF3A => Some(v - 0xFF21 + 10),
            0xFF41..=0xFF5A => Some(v - 0xFF41 + 10),
            _ => None,
        }
    })?;
    (d < radix).then_some(d)
}

/// Java's `Character.isWhitespace(c) || Character.isSpaceChar(c)` (Kotlin's `isWhitespace`): the space
/// separators, the line and paragraph separators, and `\t \n \u{B} \f \r \u{1C}`–`\u{1F}`.
fn is_kt_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | '\u{1C}'..='\u{1F}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// Kotlin's `Char` functions. `char`'s own `is_digit(radix)`, `is_whitespace()`, `is_uppercase()`… come first in
/// a method call: Kotlin's are `KtChar::is_digit(c)`.
pub trait KtChar: Copy {
    /// Kotlin: `isDigit(): Boolean`
    fn is_digit(self) -> bool;
    /// Unicode's letters (Rust's Alphabetic less what is numeric: Java's isLetter also leaves out the combining
    /// marks Alphabetic takes in).
    /// Kotlin: `isLetter(): Boolean`
    fn is_letter(self) -> bool;
    /// Kotlin: `isLetterOrDigit(): Boolean`
    fn is_letter_or_digit(self) -> bool;
    /// Kotlin: `isWhitespace(): Boolean`
    fn is_whitespace(self) -> bool;
    /// Kotlin: `isUpperCase(): Boolean`
    fn is_upper_case(self) -> bool;
    /// Kotlin: `isLowerCase(): Boolean`
    fn is_lower_case(self) -> bool;
    /// Kotlin: `uppercase(): String`
    fn uppercase(self) -> String;
    /// Kotlin: `lowercase(): String`
    fn lowercase(self) -> String;
    /// Kotlin: `uppercaseChar(): Char`
    fn uppercase_char(self) -> char;
    /// Kotlin: `lowercaseChar(): Char`
    fn lowercase_char(self) -> char;
    /// Kotlin: `code: Int`
    fn code(self) -> i32;
    /// Kotlin: `digitToInt(): Int`
    fn digit_to_int(self) -> i32;
    /// Kotlin: `digitToIntOrNull(): Int?`
    fn digit_to_int_or_null(self) -> Option<i32>;
}

impl KtChar for char {
    fn is_digit(self) -> bool {
        decimal_digit(self).is_some()
    }
    fn is_letter(self) -> bool {
        self.is_alphabetic() && !self.is_numeric()
    }
    fn is_letter_or_digit(self) -> bool {
        KtChar::is_letter(self) || KtChar::is_digit(self)
    }
    fn is_whitespace(self) -> bool {
        is_kt_whitespace(self)
    }
    fn is_upper_case(self) -> bool {
        self.is_uppercase()
    }
    fn is_lower_case(self) -> bool {
        self.is_lowercase()
    }
    fn uppercase(self) -> String {
        self.to_uppercase().collect()
    }
    fn lowercase(self) -> String {
        self.to_lowercase().collect()
    }
    fn uppercase_char(self) -> char {
        let mut up = self.to_uppercase();
        match (up.next(), up.next()) {
            (Some(c), None) => c,
            _ => self,
        }
    }
    fn lowercase_char(self) -> char {
        let mut low = self.to_lowercase();
        match (low.next(), low.next()) {
            (Some(c), None) => c,
            _ => self,
        }
    }
    fn code(self) -> i32 {
        self as i32
    }
    #[track_caller]
    fn digit_to_int(self) -> i32 {
        self.digit_to_int_or_null()
            .unwrap_or_else(|| panic_kt("IllegalArgumentException", &format!("Char {self} is not a decimal digit")))
    }
    fn digit_to_int_or_null(self) -> Option<i32> {
        decimal_digit(self).map(|d| d as i32)
    }
}

// ---- Numbers from text ----

/// Kotlin's `toIntOrNull(radix)` (and Long's…): an optional sign, then digits of the radix, all of it, within `min`
/// and `max`.
fn parse_integer(s: &str, radix: u32, min: i128, max: i128) -> Option<i128> {
    let mut chars = s.chars().peekable();
    let negative = match chars.peek() {
        Some('-') => {
            chars.next();
            true
        }
        Some('+') => {
            chars.next();
            false
        }
        _ => false,
    };
    let mut value: i128 = 0;
    let mut any = false;
    for c in chars {
        let d = digit_in(c, radix)? as i128;
        value = value * radix as i128 + d;
        if value > max + 1 {
            return None;
        }
        any = true;
    }
    let value = if negative { -value } else { value };
    (any && value >= min && value <= max).then_some(value)
}

#[track_caller]
fn check_radix(radix: i32) -> u32 {
    if !(2..=36).contains(&radix) {
        panic_kt("IllegalArgumentException", &format!("radix {radix} was not in valid range 2..36"));
    }
    radix as u32
}

/// Java's `Double.parseDouble` (and so Kotlin's `toDouble()`): what is at most a control or a space around it is let
/// be; `NaN`, `Infinity` with a sign, or a decimal number (digits, a point, an exponent) with an optional `f`, `F`,
/// `d` or `D` after it. (Hexadecimal floats, which Java also reads, are not read.)
fn parse_java_float(s: &str) -> Option<f64> {
    let t = s.trim_matches(|c: char| c <= ' ');
    let (sign, body) = match t.as_bytes().first() {
        Some(b'-') => (-1.0, &t[1..]),
        Some(b'+') => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    match body {
        "NaN" => return Some(f64::NAN),
        "Infinity" => return Some(sign * f64::INFINITY),
        _ => {}
    }
    let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
    let bytes = body.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let from = *i;
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - from
    };
    let whole = digits(&mut i);
    let mut fraction = 0;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        fraction = digits(&mut i);
    }
    if whole + fraction == 0 {
        return None;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return None;
        }
    }
    if i != bytes.len() {
        return None;
    }
    body.parse::<f64>().ok().map(|v| sign * v)
}

#[track_caller]
fn number_format(s: &str) -> ! {
    panic_kt("NumberFormatException", &format!("For input string: \"{s}\""))
}

// ---- String ----

/// Kotlin's `String` (and `CharSequence`) functions over anything that is text (`&str`, `String`, …): indices and
/// lengths in UTF-16 units, parameters in Kotlin's order. `str`'s own methods of the same names (`trim`, `split`,
/// `contains`, `replace`, `repeat`, `starts_with`, `lines`…) come first in a method call on a `&str`: Kotlin's are
/// `KtStr::split(&s, …)`.
pub trait KtStr: AsRef<str> {
    /// Kotlin: `length: Int`
    fn length(&self) -> i32 {
        utf16_len(self.as_ref()) as i32
    }
    /// Kotlin: `isEmpty(): Boolean`
    fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }
    /// Kotlin: `isNotEmpty(): Boolean`
    fn is_not_empty(&self) -> bool {
        !self.as_ref().is_empty()
    }
    /// Kotlin: `isBlank(): Boolean`
    fn is_blank(&self) -> bool {
        self.as_ref().chars().all(is_kt_whitespace)
    }
    /// Kotlin: `isNotBlank(): Boolean`
    fn is_not_blank(&self) -> bool {
        !self.is_blank()
    }
    /// Kotlin: `ifEmpty(defaultValue: () -> String): String`
    fn if_empty(&self, default_value: impl FnOnce() -> String) -> String {
        if self.as_ref().is_empty() { default_value() } else { self.as_ref().to_string() }
    }
    /// Kotlin: `ifBlank(defaultValue: () -> String): String`
    fn if_blank(&self, default_value: impl FnOnce() -> String) -> String {
        if self.is_blank() { default_value() } else { self.as_ref().to_string() }
    }
    /// Kotlin's `s[index]`: the character starting at that UTF-16 index (the whole of one outside the BMP; U+FFFD
    /// at a pair's second half).
    /// Kotlin: `get(index: Int): Char`
    #[track_caller]
    fn get(&self, index: i32) -> char {
        let s = self.as_ref();
        let n = utf16_len(s);
        if index < 0 || index as usize >= n {
            panic_kt("StringIndexOutOfBoundsException", &format!("index: {index}, length: {n}"));
        }
        let mut at = 0;
        for c in s.chars() {
            if at == index as usize {
                return c;
            }
            at += c.len_utf16();
            if at > index as usize {
                break;
            }
        }
        '\u{FFFD}'
    }
    /// Kotlin: `first(): Char`
    #[track_caller]
    fn first(&self) -> char {
        self.as_ref().chars().next().unwrap_or_else(|| panic_kt("NoSuchElementException", "Char sequence is empty."))
    }
    /// Kotlin: `last(): Char`
    #[track_caller]
    fn last(&self) -> char {
        self.as_ref().chars().last().unwrap_or_else(|| panic_kt("NoSuchElementException", "Char sequence is empty."))
    }
    /// Kotlin: `firstOrNull(): Char?`
    fn first_or_null(&self) -> Option<char> {
        self.as_ref().chars().next()
    }
    /// Kotlin: `lastOrNull(): Char?`
    fn last_or_null(&self) -> Option<char> {
        self.as_ref().chars().last()
    }
    /// Kotlin: `indices: IntRange`
    fn indices(&self) -> KtProgression<i32> {
        super::range::until(0, self.length())
    }
    /// Kotlin: `lastIndex: Int`
    fn last_index(&self) -> i32 {
        self.length() - 1
    }

    /// Kotlin: `uppercase(): String`
    fn uppercase(&self) -> String {
        self.as_ref().to_uppercase()
    }
    /// Kotlin: `lowercase(): String`
    fn lowercase(&self) -> String {
        self.as_ref().to_lowercase()
    }
    /// Kotlin: `trim(): String`
    fn trim(&self) -> String {
        self.as_ref().trim_matches(is_kt_whitespace).to_string()
    }
    /// Kotlin: `trimStart(): String`
    fn trim_start(&self) -> String {
        self.as_ref().trim_start_matches(is_kt_whitespace).to_string()
    }
    /// Kotlin: `trimEnd(): String`
    fn trim_end(&self) -> String {
        self.as_ref().trim_end_matches(is_kt_whitespace).to_string()
    }
    /// The first and last lines dropped if blank, the indent every non-blank line has removed, blank lines
    /// emptied.
    /// Kotlin: `trimIndent(): String`
    fn trim_indent(&self) -> String {
        let lines = lines_of(self.as_ref());
        let indent = lines
            .iter()
            .filter(|l| !l.chars().all(is_kt_whitespace))
            .map(|l| l.chars().take_while(|c| is_kt_whitespace(*c)).count())
            .min()
            .unwrap_or(0);
        reindent(&lines, |l| {
            if l.chars().all(is_kt_whitespace) { Some(String::new()) } else { Some(l.chars().skip(indent).collect()) }
        })
    }
    /// Each line's blanks and `marginPrefix` before its text removed (a line without the prefix kept), the first
    /// and last lines dropped if blank. Panics, as Kotlin throws, for a blank prefix.
    /// Kotlin: `trimMargin(marginPrefix: String = "|"): String`
    #[track_caller]
    fn trim_margin(&self, margin_prefix: impl AsRef<str>) -> String {
        let prefix = margin_prefix.as_ref();
        if prefix.chars().all(is_kt_whitespace) {
            panic_kt("IllegalArgumentException", "marginPrefix must be non-blank string.");
        }
        let lines = lines_of(self.as_ref());
        reindent(&lines, |l| {
            let rest = l.trim_start_matches(is_kt_whitespace);
            Some(match rest.strip_prefix(prefix) {
                Some(after) => after.to_string(),
                None => l.to_string(),
            })
        })
    }
    /// Kotlin: `lines(): List<String>`
    fn lines(&self) -> Vec<String> {
        lines_of(self.as_ref()).into_iter().map(str::to_string).collect()
    }

    /// Kotlin: `split(vararg delimiters: String, ignoreCase: Boolean = false, limit: Int = 0): List<String>`
    #[track_caller]
    fn split(&self, delimiters: impl KtDelimiters, ignore_case: bool, limit: i32) -> Vec<String> {
        if limit < 0 {
            panic_kt("IllegalArgumentException", &format!("Limit must be non-negative, but was {limit}"));
        }
        let s = self.as_ref();
        let delims = delimiters.kt_delimiters();
        let mut out = Vec::new();
        let mut from = 0;
        let mut search = 0;
        loop {
            if limit > 0 && out.len() as i32 == limit - 1 {
                break;
            }
            // (the earliest delimiter; of two at one place, the first given)
            let mut best: Option<(usize, usize)> = None;
            for d in &delims {
                if let Some(at) = find_from(s, d, search, ignore_case, false)
                    && best.is_none_or(|(b, _)| at < b)
                {
                    best = Some((at, d.len()));
                }
            }
            let Some((at, len)) = best else { break };
            out.push(s[from..at].to_string());
            from = at + len;
            search = if len == 0 { next_char(s, at) } else { from };
            if len == 0 && at >= s.len() {
                break;
            }
        }
        out.push(s[from..].to_string());
        out
    }
    /// Kotlin: `startsWith(prefix: String, ignoreCase: Boolean = false): Boolean`
    fn starts_with(&self, prefix: impl KtNeedle, ignore_case: bool) -> bool {
        let (s, p) = (self.as_ref(), prefix.kt_needle());
        if ignore_case { str::starts_with(&fold(s), &*fold(&p)) } else { s.starts_with(&*p) }
    }
    /// Kotlin: `endsWith(suffix: String, ignoreCase: Boolean = false): Boolean`
    fn ends_with(&self, suffix: impl KtNeedle, ignore_case: bool) -> bool {
        let (s, p) = (self.as_ref(), suffix.kt_needle());
        if ignore_case { str::ends_with(&fold(s), &*fold(&p)) } else { s.ends_with(&*p) }
    }
    /// Kotlin: `contains(other: CharSequence, ignoreCase: Boolean = false): Boolean`
    fn contains(&self, other: impl KtNeedle, ignore_case: bool) -> bool {
        find_from(self.as_ref(), &other.kt_needle(), 0, ignore_case, false).is_some()
    }
    /// Kotlin: `indexOf(string: String, startIndex: Int = 0, ignoreCase: Boolean = false): Int`
    fn index_of(&self, string: impl KtNeedle, start_index: i32, ignore_case: bool) -> i32 {
        let s = self.as_ref();
        let from = byte_of(s, start_index.max(0) as usize);
        find_from(s, &string.kt_needle(), from, ignore_case, false).map_or(-1, |b| utf16_index(s, b))
    }
    /// The last place at or before `start_index` (by default anywhere).
    /// Kotlin: `lastIndexOf(string: String, startIndex: Int? = null, ignoreCase: Boolean = false): Int`
    fn last_index_of(&self, string: impl KtNeedle, start_index: Option<i32>, ignore_case: bool) -> i32 {
        let s = self.as_ref();
        let needle = string.kt_needle();
        let limit = start_index.unwrap_or(i32::MAX);
        let mut best = -1;
        let mut from = 0;
        while from <= s.len() {
            let Some(b) = find_from(s, &needle, from, ignore_case, false) else { break };
            let i = utf16_index(s, b);
            if i > limit {
                break;
            }
            best = i;
            from = next_char(s, b);
        }
        best
    }
    /// Kotlin: `replace(oldValue: String, newValue: String, ignoreCase: Boolean = false): String`
    fn replace(&self, old_value: impl KtNeedle, new_value: impl KtNeedle, ignore_case: bool) -> String {
        let s = self.as_ref();
        let (old, new) = (old_value.kt_needle(), new_value.kt_needle());
        if !ignore_case {
            if old.is_empty() {
                // (Kotlin's: the new value between every character and at both ends)
                let mut out = String::from(&*new);
                for c in s.chars() {
                    out.push(c);
                    out.push_str(&new);
                }
                return out;
            }
            return s.replace(&*old, &new);
        }
        let mut out = String::new();
        let mut from = 0;
        while let Some(at) = find_from(s, &old, from, true, false) {
            out.push_str(&s[from..at]);
            out.push_str(&new);
            let matched: usize = s[at..].chars().take(old.chars().count()).map(char::len_utf8).sum();
            from = at + matched;
            if matched == 0 {
                match s[from..].chars().next() {
                    Some(c) => {
                        out.push(c);
                        from += c.len_utf8();
                    }
                    None => break,
                }
            }
        }
        out.push_str(&s[from..]);
        out
    }
    /// Kotlin: `substring(startIndex: Int, endIndex: Int? = null): String`
    #[track_caller]
    fn substring(&self, start_index: i32, end_index: Option<i32>) -> String {
        let s = self.as_ref();
        let n = utf16_len(s);
        let end = end_index.unwrap_or(n as i32);
        check_range(start_index, end, n);
        utf16_slice(s, start_index as usize, end as usize)
    }
    /// Kotlin: `substringBefore(delimiter: String, missingDelimiterValue: String? = null): String`
    fn substring_before(&self, delimiter: impl KtNeedle, missing_delimiter_value: Option<&str>) -> String {
        let s = self.as_ref();
        let d = delimiter.kt_needle();
        match s.find(&*d) {
            Some(at) => s[..at].to_string(),
            None => missing_delimiter_value.unwrap_or(s).to_string(),
        }
    }
    /// Kotlin: `substringAfter(delimiter: String, missingDelimiterValue: String? = null): String`
    fn substring_after(&self, delimiter: impl KtNeedle, missing_delimiter_value: Option<&str>) -> String {
        let s = self.as_ref();
        let d = delimiter.kt_needle();
        match s.find(&*d) {
            Some(at) => s[at + d.len()..].to_string(),
            None => missing_delimiter_value.unwrap_or(s).to_string(),
        }
    }
    /// Kotlin: `substringBeforeLast(delimiter: String, missingDelimiterValue: String? = null): String`
    fn substring_before_last(&self, delimiter: impl KtNeedle, missing_delimiter_value: Option<&str>) -> String {
        let s = self.as_ref();
        let d = delimiter.kt_needle();
        match s.rfind(&*d) {
            Some(at) => s[..at].to_string(),
            None => missing_delimiter_value.unwrap_or(s).to_string(),
        }
    }
    /// Kotlin: `substringAfterLast(delimiter: String, missingDelimiterValue: String? = null): String`
    fn substring_after_last(&self, delimiter: impl KtNeedle, missing_delimiter_value: Option<&str>) -> String {
        let s = self.as_ref();
        let d = delimiter.kt_needle();
        match s.rfind(&*d) {
            Some(at) => s[at + d.len()..].to_string(),
            None => missing_delimiter_value.unwrap_or(s).to_string(),
        }
    }
    /// Kotlin: `removePrefix(prefix: CharSequence): String`
    fn remove_prefix(&self, prefix: impl KtNeedle) -> String {
        let s = self.as_ref();
        s.strip_prefix(&*prefix.kt_needle()).unwrap_or(s).to_string()
    }
    /// Kotlin: `removeSuffix(suffix: CharSequence): String`
    fn remove_suffix(&self, suffix: impl KtNeedle) -> String {
        let s = self.as_ref();
        s.strip_suffix(&*suffix.kt_needle()).unwrap_or(s).to_string()
    }
    /// Kotlin: `removeSurrounding(delimiter: CharSequence): String`
    fn remove_surrounding(&self, delimiter: impl KtNeedle) -> String {
        let s = self.as_ref();
        let d = delimiter.kt_needle();
        if s.len() >= 2 * d.len() && s.starts_with(&*d) && s.ends_with(&*d) {
            s[d.len()..s.len() - d.len()].to_string()
        } else {
            s.to_string()
        }
    }
    /// Kotlin: `padStart(length: Int, padChar: Char = ' '): String`
    #[track_caller]
    fn pad_start(&self, length: i32, pad_char: char) -> String {
        let s = self.as_ref();
        if length < 0 {
            panic_kt("IllegalArgumentException", &format!("Desired length {length} is less than zero."));
        }
        let missing = (length as usize).saturating_sub(utf16_len(s));
        let mut out: String = std::iter::repeat_n(pad_char, missing).collect();
        out.push_str(s);
        out
    }
    /// Kotlin: `padEnd(length: Int, padChar: Char = ' '): String`
    #[track_caller]
    fn pad_end(&self, length: i32, pad_char: char) -> String {
        let s = self.as_ref();
        if length < 0 {
            panic_kt("IllegalArgumentException", &format!("Desired length {length} is less than zero."));
        }
        let missing = (length as usize).saturating_sub(utf16_len(s));
        let mut out = s.to_string();
        out.extend(std::iter::repeat_n(pad_char, missing));
        out
    }
    /// Kotlin: `repeat(n: Int): String`
    #[track_caller]
    fn repeat(&self, n: i32) -> String {
        if n < 0 {
            panic_kt("IllegalArgumentException", &format!("Count 'n' must be non-negative, but was {n}."));
        }
        self.as_ref().repeat(n as usize)
    }
    /// Kotlin: `reversed(): String`
    fn reversed(&self) -> String {
        self.as_ref().chars().rev().collect()
    }
    /// Kotlin: `take(n: Int): String`
    #[track_caller]
    fn take(&self, n: i32) -> String {
        count_check(n);
        let s = self.as_ref();
        utf16_slice(s, 0, (n as usize).min(utf16_len(s)))
    }
    /// Kotlin: `drop(n: Int): String`
    #[track_caller]
    fn drop(&self, n: i32) -> String {
        count_check(n);
        let s = self.as_ref();
        let len = utf16_len(s);
        utf16_slice(s, (n as usize).min(len), len)
    }
    /// Kotlin: `takeLast(n: Int): String`
    #[track_caller]
    fn take_last(&self, n: i32) -> String {
        count_check(n);
        let s = self.as_ref();
        let len = utf16_len(s);
        utf16_slice(s, len.saturating_sub(n as usize), len)
    }
    /// Kotlin: `dropLast(n: Int): String`
    #[track_caller]
    fn drop_last(&self, n: i32) -> String {
        count_check(n);
        let s = self.as_ref();
        let len = utf16_len(s);
        utf16_slice(s, 0, len.saturating_sub(n as usize))
    }

    /// Kotlin: `filter(predicate: (Char) -> Boolean): String`
    fn filter(&self, mut predicate: impl FnMut(char) -> bool) -> String {
        self.as_ref().chars().filter(|c| predicate(*c)).collect()
    }
    /// Kotlin: `filterNot(predicate: (Char) -> Boolean): String`
    fn filter_not(&self, mut predicate: impl FnMut(char) -> bool) -> String {
        self.as_ref().chars().filter(|c| !predicate(*c)).collect()
    }
    /// Kotlin: `map(transform: (Char) -> R): List<R>`
    fn map<R>(&self, transform: impl FnMut(char) -> R) -> Vec<R> {
        self.as_ref().chars().map(transform).collect()
    }
    /// Kotlin: `forEach(action: (Char) -> Unit)`
    fn for_each(&self, action: impl FnMut(char)) {
        self.as_ref().chars().for_each(action);
    }
    /// Kotlin: `forEachIndexed(action: (Int, Char) -> Unit)` (the index in UTF-16 units)
    fn for_each_indexed(&self, mut action: impl FnMut(i32, char)) {
        let mut i = 0;
        for c in self.as_ref().chars() {
            action(i, c);
            i += c.len_utf16() as i32;
        }
    }
    /// Kotlin: `any(predicate: (Char) -> Boolean): Boolean`
    fn any_that(&self, predicate: impl FnMut(char) -> bool) -> bool {
        self.as_ref().chars().any(predicate)
    }
    /// Kotlin: `all(predicate: (Char) -> Boolean): Boolean`
    fn all(&self, predicate: impl FnMut(char) -> bool) -> bool {
        self.as_ref().chars().all(predicate)
    }
    /// Kotlin: `none(predicate: (Char) -> Boolean): Boolean`
    fn none_that(&self, predicate: impl FnMut(char) -> bool) -> bool {
        !self.as_ref().chars().any(predicate)
    }
    /// Kotlin: `count(predicate: (Char) -> Boolean): Int`
    fn count_that(&self, mut predicate: impl FnMut(char) -> bool) -> i32 {
        self.as_ref().chars().filter(|c| predicate(*c)).count() as i32
    }
    /// Kotlin: `toList(): List<Char>`
    fn to_list(&self) -> Vec<char> {
        self.as_ref().chars().collect()
    }
    /// Kotlin: `toCharArray(): CharArray`
    fn to_char_array(&self) -> Vec<char> {
        self.as_ref().chars().collect()
    }

    /// Kotlin: `toInt(): Int`
    #[track_caller]
    fn to_int(&self) -> i32 {
        self.to_int_or_null().unwrap_or_else(|| number_format(self.as_ref()))
    }
    /// Kotlin: `toInt(radix: Int): Int`
    #[track_caller]
    fn to_int_radix(&self, radix: i32) -> i32 {
        let r = check_radix(radix);
        parse_integer(self.as_ref(), r, i32::MIN as i128, i32::MAX as i128)
            .map_or_else(|| number_format(self.as_ref()), |v| v as i32)
    }
    /// Kotlin: `toIntOrNull(): Int?`
    fn to_int_or_null(&self) -> Option<i32> {
        parse_integer(self.as_ref(), 10, i32::MIN as i128, i32::MAX as i128).map(|v| v as i32)
    }
    /// Kotlin: `toLong(): Long`
    #[track_caller]
    fn to_long(&self) -> i64 {
        self.to_long_or_null().unwrap_or_else(|| number_format(self.as_ref()))
    }
    /// Kotlin: `toLongOrNull(): Long?`
    fn to_long_or_null(&self) -> Option<i64> {
        parse_integer(self.as_ref(), 10, i64::MIN as i128, i64::MAX as i128).map(|v| v as i64)
    }
    /// Kotlin: `toFloat(): Float`
    #[track_caller]
    fn to_float(&self) -> f32 {
        self.to_float_or_null().unwrap_or_else(|| number_format(self.as_ref()))
    }
    /// Kotlin: `toFloatOrNull(): Float?`
    fn to_float_or_null(&self) -> Option<f32> {
        // (read as a float itself: one rounding, as Java's parseFloat)
        parse_java_float(self.as_ref())?;
        let t = self.as_ref().trim_matches(|c: char| c <= ' ');
        let t = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
        match t.trim_start_matches(['+', '-']) {
            "NaN" => Some(f32::NAN),
            "Infinity" => Some(if t.starts_with('-') { f32::NEG_INFINITY } else { f32::INFINITY }),
            _ => t.parse::<f32>().ok(),
        }
    }
    /// Kotlin: `toDouble(): Double`
    #[track_caller]
    fn to_double(&self) -> f64 {
        self.to_double_or_null().unwrap_or_else(|| number_format(self.as_ref()))
    }
    /// Kotlin: `toDoubleOrNull(): Double?`
    fn to_double_or_null(&self) -> Option<f64> {
        parse_java_float(self.as_ref())
    }
    /// Kotlin: `toBoolean(): Boolean`
    fn to_boolean(&self) -> bool {
        self.as_ref().eq_ignore_ascii_case("true")
    }
    /// Kotlin's `"…".format(args)`: Java's `String.format`.
    /// Kotlin: `format(vararg args: Any?): String`
    #[track_caller]
    fn format(&self, args: &[&dyn KtToString]) -> String {
        super::format::format(self.as_ref(), args)
    }
}

impl<S: AsRef<str> + ?Sized> KtStr for S {}

#[track_caller]
fn count_check(n: i32) {
    if n < 0 {
        panic_kt("IllegalArgumentException", &format!("Requested character count {n} is less than zero."));
    }
}

/// The start of the character after byte `at` (one past the end at the end).
fn next_char(s: &str, at: usize) -> usize {
    s[at..].chars().next().map_or(at + 1, |c| at + c.len_utf8())
}

/// Kotlin's `lines()`: split at `\r\n`, `\n` and `\r`.
fn lines_of(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                out.push(&s[from..i]);
                from = i + 1;
            }
            b'\r' => {
                out.push(&s[from..i]);
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                from = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&s[from..]);
    out
}

/// The lines changed by `change`, the first and last dropped if blank, joined by `\n` (Kotlin's reindent).
fn reindent(lines: &[&str], change: impl Fn(&str) -> Option<String>) -> String {
    let last = lines.len().saturating_sub(1);
    let mut out: Vec<String> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if (i == 0 || i == last) && l.chars().all(is_kt_whitespace) {
            continue;
        }
        out.push(change(l).unwrap_or_else(|| l.to_string()));
    }
    out.join("\n")
}

/// What a string is in: `"ab" in s`, `'a' in s`.
impl<N: KtNeedle + ?Sized> KtContains<N> for str {
    fn kt_contains(&self, value: &N) -> bool {
        self.contains(&*value.kt_needle())
    }
}
impl<N: KtNeedle + ?Sized> KtContains<N> for String {
    fn kt_contains(&self, value: &N) -> bool {
        self.as_str().contains(&*value.kt_needle())
    }
}

/// Kotlin's functions of a nullable string.
pub trait KtNullableStr {
    /// Kotlin: `isNullOrEmpty(): Boolean`
    fn is_null_or_empty(&self) -> bool;
    /// Kotlin: `isNullOrBlank(): Boolean`
    fn is_null_or_blank(&self) -> bool;
    /// Kotlin: `orEmpty(): String`
    fn or_empty(&self) -> String;
}
impl<S: AsRef<str>> KtNullableStr for Option<S> {
    fn is_null_or_empty(&self) -> bool {
        self.as_ref().is_none_or(|s| s.as_ref().is_empty())
    }
    fn is_null_or_blank(&self) -> bool {
        self.as_ref().is_none_or(|s| s.is_blank())
    }
    fn or_empty(&self) -> String {
        self.as_ref().map_or_else(String::new, |s| s.as_ref().to_string())
    }
}

/// A char as `String.format` takes it from an integer (`%c` of a code point).
pub(crate) fn char_of(arg: FormatArg) -> Option<char> {
    match arg {
        FormatArg::Char(c) => Some(c),
        FormatArg::Int(v, _) => u32::try_from(v).ok().and_then(char::from_u32),
        _ => None,
    }
}
