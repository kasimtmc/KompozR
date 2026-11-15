//! OpenType feature settings (a text style's `font_feature_settings`): CSS's font-feature-settings syntax, as
//! Android's Paint takes it (comma separated, each as HarfBuzz's hb_feature_from_string reads one).

use std::fmt;

/// An OpenType feature turned on, off or to an alternate (HarfBuzz's hb_feature_t): over the run's clusters
/// `start..end` (all of them unless the setting gave a range).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFeature {
    pub tag: [u8; 4],
    pub value: u32,
    pub start: u32,
    pub end: u32,
}

impl FontFeature {
    /// Over everything.
    pub const GLOBAL_END: u32 = u32::MAX;

    /// `tag` set to `value` everywhere.
    pub fn new(tag: [u8; 4], value: u32) -> FontFeature {
        FontFeature { tag, value, start: 0, end: FontFeature::GLOBAL_END }
    }
    /// The tag as text.
    pub fn tag_str(&self) -> &str {
        std::str::from_utf8(&self.tag).unwrap_or("????")
    }
}

/// HarfBuzz's way: `kern`, `-liga`, `aalt=2`, `kern[3:5]=0`.
impl fmt::Debug for FontFeature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let tag = self.tag_str().trim_end();
        if self.value == 0 {
            f.write_str("-")?;
        }
        f.write_str(tag)?;
        if self.start != 0 || self.end != FontFeature::GLOBAL_END {
            f.write_str("[")?;
            if self.start != 0 {
                write!(f, "{}", self.start)?;
            }
            if self.end != self.start.wrapping_add(1) {
                f.write_str(":")?;
                if self.end != FontFeature::GLOBAL_END {
                    write!(f, "{}", self.end)?;
                }
            }
            f.write_str("]")?;
        }
        if self.value > 1 {
            write!(f, "={}", self.value)?;
        }
        Ok(())
    }
}

/// The features a font-feature-settings string turns on and off, in its order: comma separated settings, each
/// `tag`, `"tag"`, `+tag`, `-tag`, `tag=N`, `"tag" N`, `"tag" on|off`, optionally with a range `tag[a:b]`
/// (HarfBuzz's syntax, CSS's within it). Settings that don't parse are left out, as Android leaves them.
pub fn parse_font_feature_settings(settings: &str) -> Vec<FontFeature> {
    settings.split(',').filter_map(|one| parse_feature(one.trim())).collect()
}

/// One setting (HarfBuzz's hb_feature_from_string), or none if it doesn't parse.
pub fn parse_feature(text: &str) -> Option<FontFeature> {
    let mut p = Parser { s: text.as_bytes(), at: 0 };
    p.space();
    // A sign before: on or off.
    let mut value = 1;
    if p.eat(b'-') {
        value = 0;
    } else {
        p.eat(b'+');
    }
    p.space();
    let tag = p.tag()?;
    p.space();
    let (mut start, mut end) = (0, FontFeature::GLOBAL_END);
    if p.eat(b'[') {
        p.space();
        let from = p.uint();
        if let Some(n) = from {
            start = n;
        }
        p.space();
        if p.eat(b':') {
            p.space();
            if let Some(n) = p.uint() {
                end = n;
            }
        } else if from.is_some() {
            end = start.wrapping_add(1);
        }
        p.space();
        if !p.eat(b']') {
            return None;
        }
    }
    p.space();
    // A value after (CSS without the '=', which then needs one).
    let had_equal = p.eat(b'=');
    p.space();
    let had_value = match p.uint() {
        Some(n) => {
            value = n;
            true
        }
        None => match p.boolean() {
            Some(b) => {
                value = b as u32;
                true
            }
            None => false,
        },
    };
    if had_equal && !had_value {
        return None;
    }
    p.space();
    p.done().then_some(FontFeature { tag, value, start, end })
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        let yes = self.peek() == Some(c);
        if yes {
            self.at += 1;
        }
        yes
    }
    fn space(&mut self) {
        while self.peek().is_some_and(|c| c == b' ' || c == b'\t') {
            self.at += 1;
        }
    }
    fn done(&self) -> bool {
        self.at == self.s.len()
    }
    /// A tag of up to four letters, digits or `_`, quoted or not (a short one padded with spaces).
    fn tag(&mut self) -> Option<[u8; 4]> {
        let quote = self.peek().filter(|&c| c == b'"' || c == b'\'');
        if quote.is_some() {
            self.at += 1;
        }
        let from = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_') {
            self.at += 1;
        }
        let name = &self.s[from..self.at];
        if name.is_empty() || name.len() > 4 {
            return None;
        }
        if let Some(q) = quote
            && !self.eat(q)
        {
            return None;
        }
        // A quoted tag is exactly four characters (as HarfBuzz checks).
        if quote.is_some() && name.len() != 4 {
            return None;
        }
        let mut tag = [b' '; 4];
        tag[..name.len()].copy_from_slice(name);
        Some(tag)
    }
    fn uint(&mut self) -> Option<u32> {
        let from = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        if self.at == from {
            return None;
        }
        std::str::from_utf8(&self.s[from..self.at]).ok()?.parse().ok()
    }
    fn boolean(&mut self) -> Option<bool> {
        let rest = &self.s[self.at..];
        let word = |w: &[u8]| rest.len() >= w.len() && rest[..w.len()].eq_ignore_ascii_case(w);
        if word(b"on") {
            self.at += 2;
            Some(true)
        } else if word(b"off") {
            self.at += 3;
            Some(false)
        } else {
            None
        }
    }
}
