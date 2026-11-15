//! The values text styles hold (Compose's `androidx.compose.ui.text.style`): alignment, direction, indents, line
//! heights and how they sit, line breaking, hyphenation, overflow, decorations, baseline shifts, geometric
//! transforms and how text moves.

pub mod line_break;
pub mod line_height_style;

pub use line_break::LineBreak;
pub use line_height_style::LineHeightStyle;

use crate::lerp_discrete;
use kompoz_graphics::{LayoutDirection, TextUnit, TextUnitType, lerp};
use std::fmt;

/// Where a paragraph's lines go across its width (Compose's TextAlign): `Start` and `End` follow the paragraph's
/// direction (the start of right-to-left text is its right), `Left` and `Right` don't; `Justify` stretches each
/// wrapped line to the width at its spaces (not a paragraph's last line). Unspecified takes the style around it,
/// and in the end `Start`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextAlign {
    #[default]
    Unspecified,
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

impl TextAlign {
    /// All but Unspecified (Compose's values()).
    pub const VALUES: [TextAlign; 6] =
        [TextAlign::Left, TextAlign::Right, TextAlign::Center, TextAlign::Justify, TextAlign::Start, TextAlign::End];

    pub fn is_specified(self) -> bool {
        self != TextAlign::Unspecified
    }
    /// This, or `block`'s where unspecified (Compose's takeOrElse).
    pub fn take_or_else(self, block: impl FnOnce() -> TextAlign) -> TextAlign {
        if self.is_specified() { self } else { block() }
    }
}

/// Which way a paragraph goes (Compose's TextDirection): `Ltr` or `Rtl` whatever it holds; `Content` by its first
/// strong character (the layout's direction if none: `ContentOrLtr`, `ContentOrRtl` where the layout goes so);
/// `ContentOrLtr` / `ContentOrRtl` by its first strong character, else left to right / right to left. Unspecified
/// takes the style around it, and in the end the layout's direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextDirection {
    #[default]
    Unspecified,
    Ltr,
    Rtl,
    Content,
    ContentOrLtr,
    ContentOrRtl,
}

impl TextDirection {
    pub fn is_specified(self) -> bool {
        self != TextDirection::Unspecified
    }
    /// This, or `block`'s where unspecified (Compose's takeOrElse).
    pub fn take_or_else(self, block: impl FnOnce() -> TextDirection) -> TextDirection {
        if self.is_specified() { self } else { block() }
    }
}

/// The text direction a style's resolves to in a layout's direction (Compose's resolveTextDirection): `Content`
/// becomes `ContentOrLtr` or `ContentOrRtl`, Unspecified `Ltr` or `Rtl`, by the layout's direction.
pub fn resolve_text_direction(layout_direction: LayoutDirection, text_direction: TextDirection) -> TextDirection {
    match (text_direction, layout_direction) {
        (TextDirection::Content, LayoutDirection::Ltr) => TextDirection::ContentOrLtr,
        (TextDirection::Content, LayoutDirection::Rtl) => TextDirection::ContentOrRtl,
        (TextDirection::Unspecified, LayoutDirection::Ltr) => TextDirection::Ltr,
        (TextDirection::Unspecified, LayoutDirection::Rtl) => TextDirection::Rtl,
        (d, _) => d,
    }
}

/// Which way a stretch of laid out text goes (Compose's ResolvedTextDirection).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ResolvedTextDirection {
    Ltr,
    Rtl,
}

/// How far in a paragraph's first line and its other lines begin, from its start (Compose's TextIndent); in sp,
/// or em of the paragraph's font size.
#[derive(Clone, Copy, PartialEq, Hash, Debug)]
pub struct TextIndent {
    pub first_line: TextUnit,
    pub rest_line: TextUnit,
}

impl TextIndent {
    /// No indent (Compose's TextIndent.None).
    pub const None: TextIndent = TextIndent::new(ZERO_SP, ZERO_SP);

    pub const fn new(first_line: TextUnit, rest_line: TextUnit) -> TextIndent {
        TextIndent { first_line, rest_line }
    }
}

const ZERO_SP: TextUnit = TextUnit::new(0.0, TextUnitType::Sp);

/// No indent.
impl Default for TextIndent {
    fn default() -> TextIndent {
        TextIndent::None
    }
}

/// Compose's `TextIndent(firstLine = 0.sp, restLine = 0.sp)`.
#[kompoz_runtime::named]
pub fn TextIndent(
    #[default(TextUnit::new(0.0, TextUnitType::Sp))] first_line: TextUnit,
    #[default(TextUnit::new(0.0, TextUnitType::Sp))] rest_line: TextUnit,
) -> TextIndent {
    TextIndent::new(first_line, rest_line)
}

/// Between two indents (Compose's lerp(TextIndent, TextIndent, fraction)): each line's along when both are of one
/// unit, else the nearer one's.
pub fn lerp_text_indent(start: &TextIndent, stop: &TextIndent, fraction: f32) -> TextIndent {
    TextIndent::new(
        lerp_text_unit_inheritable(start.first_line, stop.first_line, fraction),
        lerp_text_unit_inheritable(start.rest_line, stop.rest_line, fraction),
    )
}

/// Between two text lengths: along when both are given in one unit, else the nearer one (Compose's
/// lerpTextUnitInheritable).
pub(crate) fn lerp_text_unit_inheritable(a: TextUnit, b: TextUnit, t: f32) -> TextUnit {
    if a.is_unspecified() || b.is_unspecified() || a.unit_type() != b.unit_type() {
        return lerp_discrete(a, b, t);
    }
    kompoz_graphics::lerp_text_unit(a, b, t)
}

/// Whether a paragraph's words may be hyphenated where they break (Compose's Hyphens): never, or where the
/// language's rules allow (carried, but KompozR breaks no word with a hyphen yet). Unspecified takes the style
/// around it, and in the end `None`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Hyphens {
    #[default]
    Unspecified,
    None,
    Auto,
}

impl Hyphens {
    pub fn is_specified(self) -> bool {
        self != Hyphens::Unspecified
    }
    /// This, or `block`'s where unspecified (Compose's takeOrElse).
    pub fn take_or_else(self, block: impl FnOnce() -> Hyphens) -> Hyphens {
        if self.is_specified() { self } else { block() }
    }
}

/// What text that doesn't fit does (Compose's TextOverflow): cut at its bounds, its end (start, middle) replaced by
/// an ellipsis, or drawn past its bounds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
    Visible,
    StartEllipsis,
    MiddleEllipsis,
}

/// Lines drawn with text (Compose's TextDecoration): under it, through it, or both.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextDecoration {
    pub mask: i32,
}

impl TextDecoration {
    pub const None: TextDecoration = TextDecoration { mask: 0 };
    pub const Underline: TextDecoration = TextDecoration { mask: 0x1 };
    pub const LineThrough: TextDecoration = TextDecoration { mask: 0x2 };

    /// All of them (Compose's combine).
    pub fn combine(decorations: &[TextDecoration]) -> TextDecoration {
        TextDecoration { mask: decorations.iter().fold(0, |m, d| m | d.mask) }
    }
    /// From its mask (Compose's valueOf): only the underline and line-through bits.
    pub fn value_of(mask: i32) -> TextDecoration {
        assert!(mask | 0b11 == 0b11, "kompoz: a text decoration's mask has only its two lowest bits, not {mask}");
        TextDecoration { mask }
    }
    /// Whether it draws all of `other`'s lines.
    pub fn contains(self, other: TextDecoration) -> bool {
        self.mask | other.mask == self.mask
    }
}

/// Both's lines (Compose's plus: the masks combined).
#[allow(clippy::suspicious_arithmetic_impl)]
impl std::ops::Add for TextDecoration {
    type Output = TextDecoration;
    fn add(self, o: TextDecoration) -> TextDecoration {
        TextDecoration { mask: self.mask | o.mask }
    }
}

/// As Compose writes it: `TextDecoration.Underline`, `TextDecoration[Underline, LineThrough]`.
impl fmt::Debug for TextDecoration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut names = Vec::new();
        if self.mask & 1 != 0 {
            names.push("Underline");
        }
        if self.mask & 2 != 0 {
            names.push("LineThrough");
        }
        match names.len() {
            0 => f.write_str("TextDecoration.None"),
            1 => write!(f, "TextDecoration.{}", names[0]),
            _ => write!(f, "TextDecoration[{}]", names.join(", ")),
        }
    }
}

/// How far text is moved up from the baseline, as a share of its ascent (Compose's BaselineShift): superscript
/// half up, subscript half down. NaN is unspecified.
#[derive(Clone, Copy, Debug)]
pub struct BaselineShift(pub f32);

impl BaselineShift {
    pub const Superscript: BaselineShift = BaselineShift(0.5);
    pub const Subscript: BaselineShift = BaselineShift(-0.5);
    pub const None: BaselineShift = BaselineShift(0.0);
    pub const Unspecified: BaselineShift = BaselineShift(f32::NAN);

    /// Its share of the ascent (Compose's multiplier).
    pub fn multiplier(self) -> f32 {
        self.0
    }
    pub fn is_specified(self) -> bool {
        !self.0.is_nan()
    }
    /// This, or `block`'s where unspecified (Compose's takeOrElse).
    pub fn take_or_else(self, block: impl FnOnce() -> BaselineShift) -> BaselineShift {
        if self.is_specified() { self } else { block() }
    }
}

/// The same share (an unspecified one equal to another).
impl PartialEq for BaselineShift {
    fn eq(&self, o: &BaselineShift) -> bool {
        self.0 == o.0 || self.0.is_nan() && o.0.is_nan()
    }
}

/// Between two shifts (Compose's lerp(BaselineShift, BaselineShift, fraction)).
pub fn lerp_baseline_shift(start: BaselineShift, stop: BaselineShift, fraction: f32) -> BaselineShift {
    BaselineShift(lerp(start.0, stop.0, fraction))
}

/// Glyphs stretched across (their advances too) and slanted (Compose's TextGeometricTransform): a point `y` above
/// the baseline moves `y * skew_x` to the right (-0.25 leans as a synthetic italic does).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextGeometricTransform {
    pub scale_x: f32,
    pub skew_x: f32,
}

impl TextGeometricTransform {
    /// Neither stretched nor slanted.
    pub const None: TextGeometricTransform = TextGeometricTransform { scale_x: 1.0, skew_x: 0.0 };

    pub const fn new(scale_x: f32, skew_x: f32) -> TextGeometricTransform {
        TextGeometricTransform { scale_x, skew_x }
    }
}

/// Neither stretched nor slanted.
impl Default for TextGeometricTransform {
    fn default() -> TextGeometricTransform {
        TextGeometricTransform::None
    }
}

/// Compose's `TextGeometricTransform(scaleX = 1f, skewX = 0f)`.
#[kompoz_runtime::named]
pub fn TextGeometricTransform(#[default(1.0)] scale_x: f32, #[default(0.0)] skew_x: f32) -> TextGeometricTransform {
    TextGeometricTransform::new(scale_x, skew_x)
}

/// Between two transforms (Compose's lerp(TextGeometricTransform, …)).
pub fn lerp_text_geometric_transform(
    start: &TextGeometricTransform,
    stop: &TextGeometricTransform,
    fraction: f32,
) -> TextGeometricTransform {
    TextGeometricTransform::new(lerp(start.scale_x, stop.scale_x, fraction), lerp(start.skew_x, stop.skew_x, fraction))
}

/// How glyphs' positions are rounded (Compose's TextMotion.Linearity, Android's).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Linearity {
    /// Positions and sizes as they are, not snapped to pixels (text that moves or scales stays smooth).
    Linear,
    /// Hinted to the pixel grid (sharp still text).
    FontHinting,
    /// Neither.
    None,
}

/// How text is rendered for how it moves (Compose's TextMotion): `Static` hinted and on whole pixels (sharp),
/// `Animated` linear and at subpixel positions (smooth when it moves or scales). Stage 7's renderer reads it from
/// each glyph run.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TextMotion {
    pub linearity: Linearity,
    pub subpixel_text_positioning: bool,
}

impl TextMotion {
    pub const Static: TextMotion = TextMotion { linearity: Linearity::FontHinting, subpixel_text_positioning: false };
    pub const Animated: TextMotion = TextMotion { linearity: Linearity::Linear, subpixel_text_positioning: true };
}

/// Static.
impl Default for TextMotion {
    fn default() -> TextMotion {
        TextMotion::Static
    }
}
