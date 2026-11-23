//! SVG attribute values read as Skia's SVG module reads them (SkSVGAttributeParser): numbers, lengths, colors,
//! paints, IRIs, transform lists, view boxes, aspect ratios, point lists and dash arrays. A value that doesn't
//! read is `None` (the attribute ignored, as Skia leaves it unset).

use kompoz_graphics::{Matrix, Offset, Rect};

// ---- Scanning ---------------------------------------------------------------------------------------------------

/// A cursor over an attribute's text.
pub(super) struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    pub(super) fn new(s: &'a str) -> Scanner<'a> {
        Scanner { s: s.as_bytes(), i: 0 }
    }
    pub(super) fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    pub(super) fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')) {
            self.i += 1;
        }
    }
    /// Blanks, at most one comma, blanks (SVG's comma-wsp).
    pub(super) fn skip_comma_ws(&mut self) {
        self.skip_ws();
        if self.peek() == Some(b',') {
            self.i += 1;
            self.skip_ws();
        }
    }
    /// `c` next (it is taken), else false.
    pub(super) fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    /// `word` next, ASCII case ignored (it is taken), else false.
    pub(super) fn eat_word(&mut self, word: &str) -> bool {
        let w = word.as_bytes();
        if self.s.len() - self.i >= w.len() && self.s[self.i..self.i + w.len()].eq_ignore_ascii_case(w) {
            self.i += w.len();
            true
        } else {
            false
        }
    }
    /// A number (SVG's: a sign, digits, a fraction, an exponent; an `e` not followed by digits isn't one, so `2em`
    /// is 2 then `em`).
    pub(super) fn number(&mut self) -> Option<f32> {
        let start = self.i;
        let mut i = self.i;
        let s = self.s;
        if matches!(s.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let digits_from = i;
        while matches!(s.get(i), Some(b'0'..=b'9')) {
            i += 1;
        }
        let mut any = i > digits_from;
        if s.get(i) == Some(&b'.') {
            let f = i + 1;
            let mut j = f;
            while matches!(s.get(j), Some(b'0'..=b'9')) {
                j += 1;
            }
            if j > f {
                any = true;
                i = j;
            } else if any {
                i = f; // ("1." is 1)
            }
        }
        if !any {
            return None;
        }
        if matches!(s.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(s.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            let d = j;
            while matches!(s.get(j), Some(b'0'..=b'9')) {
                j += 1;
            }
            if j > d {
                i = j;
            }
        }
        let text = std::str::from_utf8(&s[start..i]).ok()?;
        let v: f32 = text.parse().ok()?;
        if !v.is_finite() {
            return None;
        }
        self.i = i;
        Some(v)
    }
    /// What is left.
    pub(super) fn rest(&self) -> &'a str {
        std::str::from_utf8(&self.s[self.i.min(self.s.len())..]).unwrap_or("")
    }
}

/// A whole number attribute (blanks around it allowed).
pub(super) fn parse_number(s: &str) -> Option<f32> {
    let mut sc = Scanner::new(s);
    sc.skip_ws();
    let v = sc.number()?;
    sc.skip_ws();
    sc.at_end().then_some(v)
}

/// A number or a percentage (`50%` is 0.5), as opacities and stop offsets take them.
pub(super) fn parse_number_or_percent(s: &str) -> Option<f32> {
    let mut sc = Scanner::new(s);
    sc.skip_ws();
    let mut v = sc.number()?;
    if sc.eat(b'%') {
        v /= 100.0;
    }
    sc.skip_ws();
    sc.at_end().then_some(v)
}

/// A number list (blanks and commas between).
pub(super) fn parse_numbers(s: &str) -> Option<Vec<f32>> {
    let mut sc = Scanner::new(s);
    let mut out = Vec::new();
    sc.skip_ws();
    while !sc.at_end() {
        out.push(sc.number()?);
        sc.skip_comma_ws();
    }
    Some(out)
}

// ---- Lengths ----------------------------------------------------------------------------------------------------

/// A length's unit (SkSVGLength::Unit).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Unit {
    Number,
    Percentage,
    Px,
    Em,
    Ex,
    In,
    Cm,
    Mm,
    Pt,
    Pc,
}

/// A length (SkSVGLength): a number and its unit.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Length {
    pub value: f32,
    pub unit: Unit,
}

impl Length {
    pub(crate) const fn new(value: f32, unit: Unit) -> Length {
        Length { value, unit }
    }
    pub(crate) const fn number(value: f32) -> Length {
        Length { value, unit: Unit::Number }
    }
    pub(crate) const fn percent(value: f32) -> Length {
        Length { value, unit: Unit::Percentage }
    }
}

/// Skia's SkSVGLengthContext DPI: an inch is 90 px.
pub(crate) const DPI: f32 = 90.0;
/// The font size em and ex lengths are taken against (Skia has none: 16 px, the CSS default; an ex half of it).
pub(crate) const EM_PX: f32 = 16.0;

/// A length in px where it doesn't depend on a viewport (none for a percentage), as SkSVGLengthContext resolves
/// one (its DPI 90, a point 1/72.272 in as Skia has it).
pub(crate) fn absolute_px(l: Length) -> Option<f32> {
    let v = l.value;
    Some(match l.unit {
        Unit::Number | Unit::Px => v,
        Unit::Percentage => return None,
        Unit::Em => v * EM_PX,
        Unit::Ex => v * EM_PX / 2.0,
        Unit::In => v * DPI,
        Unit::Cm => v * DPI / 2.54,
        Unit::Mm => v * DPI / 25.4,
        Unit::Pt => v * DPI / 72.272,
        Unit::Pc => v * DPI * 12.0 / 72.272,
    })
}

/// A length (SVG's `<length>` or `<percentage>`): a number and a unit, blanks around it allowed.
pub(super) fn parse_length(s: &str) -> Option<Length> {
    let mut sc = Scanner::new(s);
    sc.skip_ws();
    let l = scan_length(&mut sc)?;
    sc.skip_ws();
    sc.at_end().then_some(l)
}

fn scan_length(sc: &mut Scanner) -> Option<Length> {
    let v = sc.number()?;
    let unit = if sc.eat(b'%') {
        Unit::Percentage
    } else {
        [
            ("px", Unit::Px),
            ("em", Unit::Em),
            ("ex", Unit::Ex),
            ("in", Unit::In),
            ("cm", Unit::Cm),
            ("mm", Unit::Mm),
            ("pt", Unit::Pt),
            ("pc", Unit::Pc),
        ]
        .into_iter()
        .find(|(w, _)| sc.eat_word(w))
        .map_or(Unit::Number, |(_, u)| u)
    };
    Some(Length::new(v, unit))
}

/// A list of lengths (stroke-dasharray's), blanks and commas between.
pub(super) fn parse_length_list(s: &str) -> Option<Vec<Length>> {
    let mut sc = Scanner::new(s);
    let mut out = Vec::new();
    sc.skip_ws();
    while !sc.at_end() {
        out.push(scan_length(&mut sc)?);
        sc.skip_comma_ws();
    }
    Some(out)
}

// ---- Colors and paints ------------------------------------------------------------------------------------------

/// A color value (SkSVGColor): one, or `currentColor` (the `color` property where it is used).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SvgColor {
    Argb(u32),
    Current,
}

/// A paint (SkSVGPaint): none, a color, or a paint server's IRI with what to use where it can't paint (its
/// fallback: none given is black, as Skia's parser leaves it; `none` paints nothing).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum SvgPaint {
    None,
    Color(SvgColor),
    Iri(String, Option<SvgColor>),
}

/// A color: `currentColor`, `#rgb`, `#rrggbb`, `rgb(…)`/`rgba(…)` of numbers or percentages, or a CSS named
/// color (Skia takes neither `#rgba` nor `#rrggbbaa`).
pub(super) fn parse_color(s: &str) -> Option<SvgColor> {
    let mut sc = Scanner::new(s);
    sc.skip_ws();
    let c = scan_color(&mut sc)?;
    sc.skip_ws();
    sc.at_end().then_some(c)
}

fn scan_color(sc: &mut Scanner) -> Option<SvgColor> {
    if sc.eat_word("currentColor") {
        return Some(SvgColor::Current);
    }
    if sc.eat(b'#') {
        let rest = sc.rest().as_bytes();
        let n = rest.iter().take_while(|b| b.is_ascii_hexdigit()).count();
        let hex = std::str::from_utf8(&rest[..n]).ok()?;
        let v = u32::from_str_radix(hex, 16).ok();
        let argb = match (n, v) {
            (6, Some(v)) => 0xFF00_0000 | v,
            (3, Some(v)) => {
                let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
                0xFF00_0000 | (r * 17) << 16 | (g * 17) << 8 | (b * 17)
            }
            _ => return None,
        };
        sc.i += n;
        return Some(SvgColor::Argb(argb));
    }
    if sc.eat_word("rgba(") || sc.eat_word("rgb(") {
        return scan_rgb(sc).map(SvgColor::Argb);
    }
    let rest = sc.rest();
    let n = rest.bytes().take_while(|b| b.is_ascii_alphabetic()).count();
    let argb = named_color(&rest[..n])?;
    sc.i += n;
    Some(SvgColor::Argb(argb))
}

/// `rgb(`'s components after it, through its `)`: three numbers (0..255) or percentages, an alpha after them
/// (a number 0..1 or a percentage), commas, blanks or a slash between.
fn scan_rgb(sc: &mut Scanner) -> Option<u32> {
    let mut channels = [0u32; 3];
    for (k, ch) in channels.iter_mut().enumerate() {
        sc.skip_ws();
        let v = sc.number()?;
        let v = if sc.eat(b'%') { v * 2.55 } else { v };
        *ch = v.round().clamp(0.0, 255.0) as u32;
        sc.skip_ws();
        if k < 2 {
            sc.eat(b',');
        }
    }
    sc.skip_ws();
    let mut alpha = 255u32;
    if sc.eat(b',') || sc.eat(b'/') {
        sc.skip_ws();
        let a = sc.number()?;
        let a = if sc.eat(b'%') { a / 100.0 } else { a };
        alpha = (a.clamp(0.0, 1.0) * 255.0).round() as u32;
        sc.skip_ws();
    }
    if !sc.eat(b')') {
        return None;
    }
    Some(alpha << 24 | channels[0] << 16 | channels[1] << 8 | channels[2])
}

/// A paint: `none`, a color, or `url(#id)` and a fallback (`none` or a color).
pub(super) fn parse_paint(s: &str) -> Option<SvgPaint> {
    let t = s.trim();
    if t == "none" {
        return Some(SvgPaint::None);
    }
    let mut sc = Scanner::new(t);
    if let Some(id) = scan_func_iri(&mut sc) {
        sc.skip_ws();
        if sc.at_end() {
            return Some(SvgPaint::Iri(id, Some(SvgColor::Argb(0xFF00_0000))));
        }
        if sc.rest().trim() == "none" {
            return Some(SvgPaint::Iri(id, None));
        }
        let c = scan_color(&mut sc)?;
        sc.skip_ws();
        return sc.at_end().then_some(SvgPaint::Iri(id, Some(c)));
    }
    parse_color(t).map(SvgPaint::Color)
}

/// `url(#id)` (its IRI quoted or not), giving the id.
fn scan_func_iri(sc: &mut Scanner) -> Option<String> {
    if !sc.eat_word("url(") {
        return None;
    }
    sc.skip_ws();
    let quote = if sc.eat(b'"') {
        Some('"')
    } else if sc.eat(b'\'') {
        Some('\'')
    } else {
        None
    };
    if !sc.eat(b'#') {
        return None;
    }
    let rest = sc.rest();
    let end = rest.find(|c: char| Some(c) == quote || c == ')' || (quote.is_none() && c.is_whitespace()))?;
    let id = rest[..end].to_string();
    sc.i += end;
    if let Some(q) = quote
        && !sc.eat(q as u8)
    {
        return None;
    }
    sc.skip_ws();
    if !sc.eat(b')') || id.is_empty() {
        return None;
    }
    Some(id)
}

/// A whole `url(#id)` value (clip-path's), giving the id.
pub(super) fn parse_func_iri(s: &str) -> Option<String> {
    let mut sc = Scanner::new(s.trim());
    let id = scan_func_iri(&mut sc)?;
    sc.skip_ws();
    sc.at_end().then_some(id)
}

/// An href (`#id`), giving the id.
pub(super) fn parse_iri(s: &str) -> Option<String> {
    let id = s.trim().strip_prefix('#')?;
    (!id.is_empty()).then(|| id.to_string())
}

/// The CSS named colors (SVG 1.1's 147), sorted.
static NAMED_COLORS: [(&str, u32); 147] = [
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

/// A named color, opaque (its name's case ignored).
pub(super) fn named_color(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    NAMED_COLORS.binary_search_by(|(n, _)| n.cmp(&lower.as_str())).ok().map(|i| 0xFF00_0000 | NAMED_COLORS[i].1)
}

#[cfg(test)]
pub(super) fn named_color_count() -> (usize, bool) {
    (NAMED_COLORS.len(), NAMED_COLORS.windows(2).all(|w| w[0].0 < w[1].0))
}

// ---- Transforms -------------------------------------------------------------------------------------------------

/// A 2D affine transform (SkMatrix's affine part, SVG's `matrix(a b c d e f)`): x' = a x + c y + e,
/// y' = b x + d y + f.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Affine {
    pub(crate) const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub(crate) fn translate(tx: f32, ty: f32) -> Affine {
        Affine { e: tx, f: ty, ..Affine::IDENTITY }
    }
    pub(crate) fn scale(sx: f32, sy: f32) -> Affine {
        Affine { a: sx, d: sy, ..Affine::IDENTITY }
    }
    /// Degrees, clockwise on screen (y down).
    pub(crate) fn rotate(degrees: f32) -> Affine {
        let r = (degrees as f64).to_radians();
        let (s, c) = (r.sin() as f32, r.cos() as f32);
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    pub(crate) fn skew_x(degrees: f32) -> Affine {
        Affine { c: ((degrees as f64).to_radians().tan()) as f32, ..Affine::IDENTITY }
    }
    pub(crate) fn skew_y(degrees: f32) -> Affine {
        Affine { b: ((degrees as f64).to_radians().tan()) as f32, ..Affine::IDENTITY }
    }

    /// This times `o` (SkMatrix's `this * o`): `o` first, then this.
    pub(crate) fn pre(self, o: Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }

    pub(crate) fn is_identity(&self) -> bool {
        *self == Affine::IDENTITY
    }

    pub(crate) fn map(&self, p: Offset) -> Offset {
        Offset::new(self.a * p.x + self.c * p.y + self.e, self.b * p.x + self.d * p.y + self.f)
    }

    /// The bounds of where the rect's corners go.
    pub(crate) fn map_rect(&self, r: Rect) -> Rect {
        let pts = [
            self.map(Offset::new(r.left, r.top)),
            self.map(Offset::new(r.right, r.top)),
            self.map(Offset::new(r.left, r.bottom)),
            self.map(Offset::new(r.right, r.bottom)),
        ];
        let (mut l, mut t, mut rr, mut b) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in pts {
            l = l.min(p.x);
            t = t.min(p.y);
            rr = rr.max(p.x);
            b = b.max(p.y);
        }
        Rect::new(l, t, rr, b)
    }

    /// Its inverse; none if it has none.
    pub(crate) fn invert(&self) -> Option<Affine> {
        let det = self.a as f64 * self.d as f64 - self.b as f64 * self.c as f64;
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (a, b, c, d, e, f) =
            (self.a as f64, self.b as f64, self.c as f64, self.d as f64, self.e as f64, self.f as f64);
        let inv = Affine {
            a: (d / det) as f32,
            b: (-b / det) as f32,
            c: (-c / det) as f32,
            d: (a / det) as f32,
            e: ((c * f - d * e) / det) as f32,
            f: ((b * e - a * f) / det) as f32,
        };
        [inv.a, inv.b, inv.c, inv.d, inv.e, inv.f].iter().all(|v| v.is_finite()).then_some(inv)
    }

    /// As a [`Matrix`] (its plane part).
    pub(crate) fn to_matrix(self) -> Matrix {
        let mut m = Matrix::IDENTITY;
        m.values[Matrix::ScaleX] = self.a;
        m.values[Matrix::SkewY] = self.b;
        m.values[Matrix::SkewX] = self.c;
        m.values[Matrix::ScaleY] = self.d;
        m.values[Matrix::TranslateX] = self.e;
        m.values[Matrix::TranslateY] = self.f;
        m
    }
}

/// A transform list (SVG's `transform`, `gradientTransform`): `matrix`, `translate`, `scale`, `rotate` (about a
/// point), `skewX`, `skewY`, the first outermost; commas or blanks between. One that doesn't read: none.
pub(super) fn parse_transform(s: &str) -> Option<Affine> {
    let mut sc = Scanner::new(s);
    let mut m = Affine::IDENTITY;
    sc.skip_ws();
    while !sc.at_end() {
        let name_len = sc.rest().bytes().take_while(|b| b.is_ascii_alphabetic()).count();
        let name = &sc.rest()[..name_len];
        sc.i += name_len;
        sc.skip_ws();
        if !sc.eat(b'(') {
            return None;
        }
        let mut args = Vec::new();
        sc.skip_ws();
        while !sc.eat(b')') {
            args.push(sc.number()?);
            sc.skip_comma_ws();
        }
        let t = match (name, args.as_slice()) {
            ("matrix", &[a, b, c, d, e, f]) => Affine { a, b, c, d, e, f },
            ("translate", &[tx]) => Affine::translate(tx, 0.0),
            ("translate", &[tx, ty]) => Affine::translate(tx, ty),
            ("scale", &[s]) => Affine::scale(s, s),
            ("scale", &[sx, sy]) => Affine::scale(sx, sy),
            ("rotate", &[a]) => Affine::rotate(a),
            ("rotate", &[a, cx, cy]) => {
                Affine::translate(cx, cy).pre(Affine::rotate(a)).pre(Affine::translate(-cx, -cy))
            }
            ("skewX", &[a]) => Affine::skew_x(a),
            ("skewY", &[a]) => Affine::skew_y(a),
            _ => return None,
        };
        m = m.pre(t);
        sc.skip_comma_ws();
    }
    Some(m)
}

// ---- Viewports --------------------------------------------------------------------------------------------------

/// A `viewBox` (min-x min-y width height); a negative size doesn't read (an empty one does: it disables
/// rendering).
pub(super) fn parse_view_box(s: &str) -> Option<Rect> {
    match parse_numbers(s)?.as_slice() {
        &[x, y, w, h] if w >= 0.0 && h >= 0.0 => Some(Rect::new(x, y, x + w, y + h)),
        _ => None,
    }
}

/// How a viewBox goes into a viewport (SkSVGPreserveAspectRatio): stretched (`none`), or kept, aligned
/// (0 min, 1 mid, 2 max each way) and fitted in (meet) or covering it (slice).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct AspectRatio {
    /// None: `none`; else the x and y alignments.
    pub align: Option<(u8, u8)>,
    pub slice: bool,
}

impl AspectRatio {
    /// `xMidYMid meet`, SVG's default.
    pub(crate) const DEFAULT: AspectRatio = AspectRatio { align: Some((1, 1)), slice: false };
    pub(crate) const NONE: AspectRatio = AspectRatio { align: None, slice: false };
}

/// `[defer] <align> [meet | slice]`.
pub(super) fn parse_aspect_ratio(s: &str) -> Option<AspectRatio> {
    let mut words = s.split_ascii_whitespace().peekable();
    if words.peek() == Some(&"defer") {
        words.next();
    }
    let align = match words.next()? {
        "none" => None,
        a => {
            let pos = |w: &str| match w {
                "Min" => Some(0u8),
                "Mid" => Some(1),
                "Max" => Some(2),
                _ => None,
            };
            let rest = a.strip_prefix('x')?;
            if rest.len() != 7 || !rest.is_char_boundary(3) {
                return None;
            }
            let (x, y) = rest.split_at(3);
            Some((pos(x)?, pos(y.strip_prefix('Y')?)?))
        }
    };
    let slice = match words.next() {
        None | Some("meet") => false,
        Some("slice") => true,
        _ => return None,
    };
    words.next().is_none().then_some(AspectRatio { align, slice })
}

/// The transform taking `view_box` onto `view_port` as `par` says (SkSVGSVG's ComputeViewboxMatrix): scale both
/// ways (`none`) or by the smaller (meet) or bigger (slice) of the two, then aligned; an empty box or port: all
/// to a point.
pub(crate) fn view_box_transform(view_box: Rect, view_port: Rect, par: AspectRatio) -> Affine {
    let (vbw, vbh) = (view_box.right - view_box.left, view_box.bottom - view_box.top);
    let (vpw, vph) = (view_port.right - view_port.left, view_port.bottom - view_port.top);
    if vbw <= 0.0 || vbh <= 0.0 || vpw <= 0.0 || vph <= 0.0 {
        return Affine::scale(0.0, 0.0);
    }
    let (sx, sy) = (vpw / vbw, vph / vbh);
    let (sx, sy, (ax, ay)) = match par.align {
        None => (sx, sy, (0, 0)),
        Some(al) => {
            let s = if par.slice { sx.max(sy) } else { sx.min(sy) };
            (s, s, al)
        }
    };
    let coeff = |a: u8| a as f32 * 0.5;
    let tx = -view_box.left * sx + (vpw - vbw * sx) * coeff(ax);
    let ty = -view_box.top * sy + (vph - vbh * sy) * coeff(ay);
    Affine::translate(tx, ty).pre(Affine::scale(sx, sy))
}

// ---- Points -----------------------------------------------------------------------------------------------------

/// A points list (polyline's, polygon's): pairs, an odd number left over dropped; reading stops at what isn't a
/// number (what was read before kept, as SVG's error handling has it).
pub(super) fn parse_points(s: &str) -> Vec<Offset> {
    let mut sc = Scanner::new(s);
    let mut nums = Vec::new();
    sc.skip_ws();
    while !sc.at_end() {
        let Some(v) = sc.number() else { break };
        nums.push(v);
        sc.skip_comma_ws();
    }
    nums.as_chunks::<2>().0.iter().map(|&[x, y]| Offset::new(x, y)).collect()
}
