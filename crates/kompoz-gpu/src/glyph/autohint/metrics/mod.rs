//! Autohinting specific metrics.

mod blues;
mod scale;
mod widths;

use super::{
    instance::Target,
    style::{ScriptGroup, StyleClass},
    topo::Dimension,
};
use skrifa::attribute::Style;
use skrifa::raw::types::Fixed;

pub(crate) use blues::{BlueOutlineBuf, ScaledBlues, UnscaledBlues};
pub use blues::{BlueZones, ScaledBlue, UnscaledBlue};
pub(crate) use scale::{compute_unscaled_style_metrics, scale_style_metrics};

/// Maximum number of widths, same for Latin and CJK.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L65>
/// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.h#L55>
pub(crate) const MAX_WIDTHS: usize = 16;

/// Unscaled metrics for a single axis.
///
/// This is the union of the Latin and CJK axis records.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L88>
/// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.h#L73>
#[derive(Clone, Default, Debug)]
pub struct UnscaledAxisMetrics {
    /// The dimension of this axis.
    pub dim: Dimension,
    pub(crate) widths: UnscaledWidths,
    /// The unscaled width metrics for the axis.
    pub width_metrics: WidthMetrics,
    pub(crate) blues: UnscaledBlues,
}

impl UnscaledAxisMetrics {
    /// Returns the set of unscaled widths.
    pub fn widths(&self) -> &[i32] {
        self.widths.as_slice()
    }

    /// Returns the set of unscaled blues.
    pub fn blues(&self) -> &[UnscaledBlue] {
        self.blues.as_slice()
    }

    /// Returns the largest width, if any.
    pub fn max_width(&self) -> Option<i32> {
        self.widths.last().copied()
    }
}

/// Scaled metrics for a single axis.
#[derive(Clone, Default, Debug)]
pub struct ScaledAxisMetrics {
    /// The dimension of this axis.
    pub dim: Dimension,
    /// Font unit to 26.6 scale in the axis direction.
    pub scale: i32,
    /// 1/64 pixel delta in the axis direction.
    pub delta: i32,
    pub(crate) widths: ScaledWidths,
    /// The scaled width metrics.
    pub width_metrics: WidthMetrics,
    pub(crate) blues: ScaledBlues,
}

impl ScaledAxisMetrics {
    /// Returns the set of scaled widths.
    pub fn widths(&self) -> &[ScaledWidth] {
        self.widths.as_slice()
    }

    /// Returns the set of scaled blues.
    pub fn blues(&self) -> &[ScaledBlue] {
        self.blues.as_slice()
    }
}

/// Unscaled metrics for a single style and script.
//
// This is the union of the root, Latin and CJK style metrics but
// the latter two are actually identical.
//
// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aftypes.h#L413>
// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L109>
// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.h#L95>
#[derive(Clone, Default, Debug)]
pub struct UnscaledStyleMetrics {
    /// Index of style class.
    pub(crate) class_ix: u16,
    /// Monospaced digits?
    pub digits_have_same_width: bool,
    /// Per-dimension unscaled metrics.
    pub(crate) axes: [UnscaledAxisMetrics; 2],
}

impl UnscaledStyleMetrics {
    /// Returns the associated style class.
    pub fn style_class(&self) -> &'static StyleClass {
        &super::style::STYLE_CLASSES[self.class_ix as usize]
    }

    /// Returns unscaled metrics for the horizontal axis.
    pub fn horizontal_metrics(&self) -> &UnscaledAxisMetrics {
        &self.axes[0]
    }

    /// Returns unscaled metrics for the vertical axis.
    pub fn vertical_metrics(&self) -> &UnscaledAxisMetrics {
        &self.axes[1]
    }
}

/// Scaled metrics for a single style and script.
#[derive(Clone, Default, Debug)]
pub struct ScaledStyleMetrics {
    /// Multidimensional scaling factors and deltas.
    pub scale: Scale,
    /// Per-dimension scaled metrics.
    pub(crate) axes: [ScaledAxisMetrics; 2],
}

impl ScaledStyleMetrics {
    /// Returns unscaled metrics for the horizontal axis.
    pub fn horizontal_metrics(&self) -> &ScaledAxisMetrics {
        &self.axes[0]
    }

    /// Returns unscaled metrics for the vertical axis.
    pub fn vertical_metrics(&self) -> &ScaledAxisMetrics {
        &self.axes[1]
    }
}

/// Metrics for the set of stems along a single axis.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct WidthMetrics {
    /// Used for creating edges.
    pub edge_distance_threshold: i32,
    /// Default stem thickness.
    pub standard_width: i32,
    /// Is standard width very light?
    pub is_extra_light: bool,
}

pub(crate) type UnscaledWidths = Vec<i32>;

/// A scaled stem width.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ScaledWidth {
    /// Width after applying scale.
    pub scaled: i32,
    /// Grid-fitted width.
    pub fitted: i32,
}

pub(crate) type ScaledWidths = Vec<ScaledWidth>;

/// Flags that define how scaling and hinting is applied.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ScaleFlags(pub(crate) u32);

// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aftypes.h#L115>
// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L143>
impl ScaleFlags {
    /// Stem width snapping.
    pub const HORIZONTAL_SNAP: Self = Self(1 << 0);
    /// Stem height snapping.
    pub const VERTICAL_SNAP: Self = Self(1 << 1);
    /// Stem width/height adjustment.
    pub const STEM_ADJUST: Self = Self(1 << 2);
    /// Monochrome rendering.
    pub const MONO: Self = Self(1 << 3);
    /// Disable horizontal hinting.
    pub const NO_HORIZONTAL: Self = Self(1 << 4);
    /// Disable vertical hinting.
    pub const NO_VERTICAL: Self = Self(1 << 5);
    /// Disable advance hinting.
    pub const NO_ADVANCE: Self = Self(1 << 6);
}

impl ScaleFlags {
    /// Creates new flags, truncating the given bits to valid values.
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & 0b1111111)
    }

    /// Returns the underlying flag bits.
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Returns true if `self` contains all flags in `other`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns true if `self` contains any flags in `other`.
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl core::ops::Not for ScaleFlags {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl core::ops::BitOr for ScaleFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for ScaleFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for ScaleFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::BitAndAssign for ScaleFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

/// Captures scaling parameters which may be modified during metrics
/// computation.
#[derive(Copy, Clone, Default, Debug)]
pub struct Scale {
    /// Flags that determine hinting functionality.
    pub flags: ScaleFlags,
    /// Font unit to 26.6 scale in the X direction.
    pub x_scale: i32,
    /// Font unit to 26.6 scale in the Y direction.
    pub y_scale: i32,
    /// In 1/64 device pixels.
    pub x_delta: i32,
    /// In 1/64 device pixels.
    pub y_delta: i32,
    /// Font size in pixels per em.
    pub size: f32,
    /// From the source font.
    pub units_per_em: i32,
}

impl Scale {
    /// Create initial scaling parameters from metrics and hinting target.
    pub fn new(size: f32, units_per_em: i32, font_style: Style, target: Target, group: ScriptGroup) -> Self {
        let scale = (Fixed::from_bits((size * 64.0) as i32) / Fixed::from_bits(units_per_em)).to_bits();
        let mut flags = ScaleFlags::default();
        let is_italic = font_style != Style::Normal;
        let is_mono = target == Target::Mono;
        let is_light = target.is_light();
        // Snap vertical stems for monochrome and horizontal LCD rendering.
        if is_mono || target.is_lcd() {
            flags |= ScaleFlags::HORIZONTAL_SNAP;
        }
        // Snap horizontal stems for monochrome and vertical LCD rendering.
        if is_mono || target.is_vertical_lcd() {
            flags |= ScaleFlags::VERTICAL_SNAP;
        }
        // Adjust stems to full pixels unless in LCD or light modes.
        if !(target.is_lcd() || is_light) {
            flags |= ScaleFlags::STEM_ADJUST;
        }
        if is_mono {
            flags |= ScaleFlags::MONO;
        }
        if group == ScriptGroup::Default {
            // Disable horizontal hinting completely for LCD, light hinting
            // and italic fonts
            // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2674>
            if target.is_lcd() || is_light || is_italic {
                flags |= ScaleFlags::NO_HORIZONTAL;
            }
        } else {
            // CJK doesn't hint advances
            // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L1432>
            flags |= ScaleFlags::NO_ADVANCE;
        }
        // CJK doesn't hint advances
        // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L1432>
        if group != ScriptGroup::Default {
            flags |= ScaleFlags::NO_ADVANCE;
        }
        Self { x_scale: scale, y_scale: scale, x_delta: 0, y_delta: 0, size, units_per_em, flags }
    }
}

// <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L59>
pub(crate) fn sort_and_quantize_widths(widths: &mut UnscaledWidths, threshold: i32) {
    if widths.len() <= 1 {
        return;
    }
    widths.sort_unstable();
    let table = widths.as_mut_slice();
    let mut cur_ix = 0;
    let mut cur_val = table[cur_ix];
    let last_ix = table.len() - 1;
    let mut ix = 1;
    // Compute and use mean values for clusters not larger than
    // `threshold`.
    while ix < table.len() {
        if (table[ix] - cur_val) > threshold || ix == last_ix {
            let mut sum = 0;
            // Fix loop for end of array?
            if (table[ix] - cur_val <= threshold) && ix == last_ix {
                ix += 1;
            }
            for val in &mut table[cur_ix..ix] {
                sum += *val;
                *val = 0;
            }
            table[cur_ix] = sum / ix as i32;
            if ix < last_ix {
                cur_ix = ix + 1;
                cur_val = table[cur_ix];
            }
        }
        ix += 1;
    }
    cur_ix = 1;
    // Compress array to remove zero values
    for ix in 1..table.len() {
        if table[ix] != 0 {
            table[cur_ix] = table[ix];
            cur_ix += 1;
        }
    }
    widths.truncate(cur_ix);
}

// Fixed point helpers
//
// Note: lots of bit fiddling based fixed point math in the autohinter
// so we're opting out of using the strongly typed variants because they
// just add noise and reduce clarity.

pub(crate) fn fixed_mul(a: i32, b: i32) -> i32 {
    (Fixed::from_bits(a) * Fixed::from_bits(b)).to_bits()
}

pub(crate) fn fixed_div(a: i32, b: i32) -> i32 {
    (Fixed::from_bits(a) / Fixed::from_bits(b)).to_bits()
}

pub(crate) fn fixed_mul_div(a: i32, b: i32, c: i32) -> i32 {
    Fixed::from_bits(a).mul_div(Fixed::from_bits(b), Fixed::from_bits(c)).to_bits()
}

pub(crate) fn pix_round(a: i32) -> i32 {
    (a.wrapping_add(32)) & !63
}

pub(crate) fn pix_floor(a: i32) -> i32 {
    a & !63
}
