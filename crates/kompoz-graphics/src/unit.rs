//! Units (Compose's `androidx.compose.ui.unit`): `Dp`, `TextUnit` (sp, em), their offsets and sizes, and `Density`,
//! which turns them into pixels. Layout and drawing work in pixels; what an app writes is in dp and sp.

use crate::geometry::{Offset, Rect, Size};
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A density-independent length: 1 dp is 1 px at 160 dpi.
#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Dp(pub f32);

impl Dp {
    /// No length.
    pub const Hairline: Dp = Dp(0.0);
    pub const Infinity: Dp = Dp(f32::INFINITY);
    /// Not given (NaN).
    pub const Unspecified: Dp = Dp(f32::NAN);

    pub const fn value(self) -> f32 {
        self.0
    }
    pub fn is_specified(self) -> bool {
        !self.0.is_nan()
    }
    pub fn is_unspecified(self) -> bool {
        self.0.is_nan()
    }
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }
    /// This, or `other` where this is unspecified (Compose's takeOrElse).
    pub fn take_or_else(self, other: impl FnOnce() -> Dp) -> Dp {
        if self.is_specified() { self } else { other() }
    }
    pub fn coerce_in(self, min: Dp, max: Dp) -> Dp {
        Dp(self.0.max(min.0).min(max.0))
    }
    pub fn coerce_at_least(self, min: Dp) -> Dp {
        Dp(self.0.max(min.0))
    }
    pub fn coerce_at_most(self, max: Dp) -> Dp {
        Dp(self.0.min(max.0))
    }
}

impl std::fmt::Debug for Dp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.is_unspecified() { f.write_str("Dp.Unspecified") } else { write!(f, "{}.dp", self.0) }
    }
}

impl Add for Dp {
    type Output = Dp;
    fn add(self, o: Dp) -> Dp {
        Dp(self.0 + o.0)
    }
}
impl Sub for Dp {
    type Output = Dp;
    fn sub(self, o: Dp) -> Dp {
        Dp(self.0 - o.0)
    }
}
impl Neg for Dp {
    type Output = Dp;
    fn neg(self) -> Dp {
        Dp(-self.0)
    }
}
impl Mul<f32> for Dp {
    type Output = Dp;
    fn mul(self, k: f32) -> Dp {
        Dp(self.0 * k)
    }
}
impl Mul<Dp> for f32 {
    type Output = Dp;
    fn mul(self, d: Dp) -> Dp {
        Dp(self * d.0)
    }
}
impl Div<f32> for Dp {
    type Output = Dp;
    fn div(self, k: f32) -> Dp {
        Dp(self.0 / k)
    }
}
/// How many times `o` fits in this.
impl Div<Dp> for Dp {
    type Output = f32;
    fn div(self, o: Dp) -> f32 {
        self.0 / o.0
    }
}

pub fn min(a: Dp, b: Dp) -> Dp {
    Dp(a.0.min(b.0))
}
pub fn max(a: Dp, b: Dp) -> Dp {
    Dp(a.0.max(b.0))
}

/// What a scale and a text size are measured in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextUnitType {
    Unspecified,
    /// Scaled pixels: dp times the font scale.
    Sp,
    /// Of the font size (a line height of 1.5.em).
    Em,
}

/// A text length in sp or em (Compose's TextUnit). Equal by unit and value, an unspecified one to another (as
/// Compose's packed values are).
#[derive(Clone, Copy)]
pub struct TextUnit {
    value: f32,
    unit: TextUnitType,
}

impl PartialEq for TextUnit {
    fn eq(&self, o: &TextUnit) -> bool {
        self.unit == o.unit && (self.value == o.value || self.value.is_nan() && o.value.is_nan())
    }
}

impl std::hash::Hash for TextUnit {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.unit.hash(state);
        // (0 and -0 alike, every NaN alike: as they compare)
        let v = if self.value.is_nan() { f32::NAN } else { self.value + 0.0 };
        v.to_bits().hash(state);
    }
}

impl TextUnit {
    pub const Unspecified: TextUnit = TextUnit { value: f32::NAN, unit: TextUnitType::Unspecified };

    pub const fn new(value: f32, unit: TextUnitType) -> TextUnit {
        TextUnit { value, unit }
    }
    pub const fn value(self) -> f32 {
        self.value
    }
    pub const fn unit_type(self) -> TextUnitType {
        self.unit
    }
    pub fn is_sp(self) -> bool {
        self.unit == TextUnitType::Sp
    }
    pub fn is_em(self) -> bool {
        self.unit == TextUnitType::Em
    }
    pub fn is_specified(self) -> bool {
        self.unit != TextUnitType::Unspecified
    }
    pub fn is_unspecified(self) -> bool {
        self.unit == TextUnitType::Unspecified
    }
    pub fn take_or_else(self, other: impl FnOnce() -> TextUnit) -> TextUnit {
        if self.is_specified() { self } else { other() }
    }
}

impl Default for TextUnit {
    fn default() -> Self {
        TextUnit::Unspecified
    }
}

impl std::fmt::Debug for TextUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.unit {
            TextUnitType::Unspecified => f.write_str("Unspecified"),
            TextUnitType::Sp => write!(f, "{}.sp", self.value),
            TextUnitType::Em => write!(f, "{}.em", self.value),
        }
    }
}

impl Mul<f32> for TextUnit {
    type Output = TextUnit;
    fn mul(self, k: f32) -> TextUnit {
        TextUnit { value: self.value * k, unit: self.unit }
    }
}
impl Div<f32> for TextUnit {
    type Output = TextUnit;
    fn div(self, k: f32) -> TextUnit {
        TextUnit { value: self.value / k, unit: self.unit }
    }
}
impl Neg for TextUnit {
    type Output = TextUnit;
    fn neg(self) -> TextUnit {
        TextUnit { value: -self.value, unit: self.unit }
    }
}

/// Between two text lengths of one unit (Compose's lerp(TextUnit, TextUnit, fraction)); of two units, or an
/// unspecified one, panics.
pub fn lerp_text_unit(start: TextUnit, stop: TextUnit, fraction: f32) -> TextUnit {
    assert!(
        start.unit == stop.unit && start.is_specified(),
        "kompoz: only text lengths of one unit lerp, not {start:?} and {stop:?}"
    );
    TextUnit::new(crate::geometry::lerp(start.value, stop.value, fraction), start.unit)
}

/// `16.dp()`, `14.sp()`, `1.5.em()`: Compose's `16.dp`, `14.sp`, `1.5.em`.
pub trait UnitExt {
    fn dp(self) -> Dp;
    fn sp(self) -> TextUnit;
    fn em(self) -> TextUnit;
}

macro_rules! unit_ext {
    ($($t:ty)*) => {$(
        impl UnitExt for $t {
            fn dp(self) -> Dp {
                Dp(self as f32)
            }
            fn sp(self) -> TextUnit {
                TextUnit::new(self as f32, TextUnitType::Sp)
            }
            fn em(self) -> TextUnit {
                TextUnit::new(self as f32, TextUnitType::Em)
            }
        }
    )*};
}
unit_ext!(i32 i64 u32 usize f32 f64);

/// A position in dp (Compose's DpOffset).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct DpOffset {
    pub x: Dp,
    pub y: Dp,
}

impl DpOffset {
    pub const Zero: DpOffset = DpOffset { x: Dp(0.0), y: Dp(0.0) };
    pub const Unspecified: DpOffset = DpOffset { x: Dp::Unspecified, y: Dp::Unspecified };
    pub const fn new(x: Dp, y: Dp) -> DpOffset {
        DpOffset { x, y }
    }
    pub fn is_specified(self) -> bool {
        self.x.is_specified() && self.y.is_specified()
    }
}

impl Add for DpOffset {
    type Output = DpOffset;
    fn add(self, o: DpOffset) -> DpOffset {
        DpOffset { x: self.x + o.x, y: self.y + o.y }
    }
}
impl Sub for DpOffset {
    type Output = DpOffset;
    fn sub(self, o: DpOffset) -> DpOffset {
        DpOffset { x: self.x - o.x, y: self.y - o.y }
    }
}

/// A size in dp (Compose's DpSize).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct DpSize {
    pub width: Dp,
    pub height: Dp,
}

impl DpSize {
    pub const Zero: DpSize = DpSize { width: Dp(0.0), height: Dp(0.0) };
    pub const Unspecified: DpSize = DpSize { width: Dp::Unspecified, height: Dp::Unspecified };
    pub const fn new(width: Dp, height: Dp) -> DpSize {
        DpSize { width, height }
    }
    pub fn is_specified(self) -> bool {
        self.width.is_specified() && self.height.is_specified()
    }
    pub fn center(self) -> DpOffset {
        DpOffset { x: self.width / 2.0, y: self.height / 2.0 }
    }
}

impl Mul<f32> for DpSize {
    type Output = DpSize;
    fn mul(self, k: f32) -> DpSize {
        DpSize { width: self.width * k, height: self.height * k }
    }
}

/// A rectangle in dp (Compose's DpRect).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct DpRect {
    pub left: Dp,
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
}

impl DpRect {
    pub fn width(self) -> Dp {
        self.right - self.left
    }
    pub fn height(self) -> Dp {
        self.bottom - self.top
    }
    pub fn size(self) -> DpSize {
        DpSize { width: self.width(), height: self.height() }
    }
}

/// Which way layouts go (Compose's LayoutDirection): left to right, or right to left (where a start is on the
/// right).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LayoutDirection {
    Ltr,
    Rtl,
}

/// Between two lengths.
pub fn lerp_dp(start: Dp, stop: Dp, fraction: f32) -> Dp {
    Dp(crate::geometry::lerp(start.0, stop.0, fraction))
}

/// How many pixels a dp is and how much bigger sp are than dp (Compose's Density): what turns dp and sp into
/// pixels and back.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Density {
    pub density: f32,
    pub font_scale: f32,
}

impl Default for Density {
    fn default() -> Self {
        Density { density: 1.0, font_scale: 1.0 }
    }
}

impl Density {
    pub const fn new(density: f32, font_scale: f32) -> Density {
        Density { density, font_scale }
    }

    /// `dp.toPx()`.
    pub fn to_px(&self, dp: Dp) -> f32 {
        dp.0 * self.density
    }
    /// `dp.roundToPx()`: the nearest whole pixel (a half up); an infinite length as `i32::MAX`
    /// (Compose's Constraints.Infinity).
    pub fn round_to_px(&self, dp: Dp) -> i32 {
        let px = self.to_px(dp);
        if px.is_infinite() { i32::MAX } else { (px + 0.5).floor() as i32 }
    }
    /// `px.toDp()`.
    pub fn to_dp(&self, px: f32) -> Dp {
        Dp(px / self.density)
    }
    /// An sp or em length in pixels (an em one isn't one on its own: panics).
    pub fn text_to_px(&self, unit: TextUnit) -> f32 {
        self.to_px(self.text_to_dp(unit))
    }
    /// `sp.toDp()`: an sp length in dp (the font scale).
    pub fn text_to_dp(&self, unit: TextUnit) -> Dp {
        assert!(unit.is_sp(), "kompoz: only sp lengths turn into dp, not {unit:?}");
        Dp(unit.value * self.font_scale)
    }
    /// `px.toSp()`.
    pub fn to_sp(&self, px: f32) -> TextUnit {
        TextUnit::new(px / self.density / self.font_scale, TextUnitType::Sp)
    }
    /// `dpSize.toSize()`.
    pub fn size_to_px(&self, size: DpSize) -> Size {
        Size::new(self.to_px(size.width), self.to_px(size.height))
    }
    pub fn size_to_dp(&self, size: Size) -> DpSize {
        DpSize::new(self.to_dp(size.width), self.to_dp(size.height))
    }
    pub fn offset_to_px(&self, offset: DpOffset) -> Offset {
        Offset::new(self.to_px(offset.x), self.to_px(offset.y))
    }
    pub fn rect_to_px(&self, rect: DpRect) -> Rect {
        Rect::new(self.to_px(rect.left), self.to_px(rect.top), self.to_px(rect.right), self.to_px(rect.bottom))
    }
}
