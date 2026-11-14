//! How an image (or a painter) is scaled into a space of another shape (Compose's ContentScale and ScaleFactor).

use crate::geometry::{Size, lerp};
use std::fmt;
use std::ops::{Div, Mul};
use std::sync::Arc;

/// How much something is scaled horizontally and vertically (Compose's ScaleFactor).
#[derive(Clone, Copy, PartialEq)]
pub struct ScaleFactor {
    pub scale_x: f32,
    pub scale_y: f32,
}

impl ScaleFactor {
    /// Neither given (a sentinel: NaN both ways).
    pub const Unspecified: ScaleFactor = ScaleFactor { scale_x: f32::NAN, scale_y: f32::NAN };

    pub const fn new(scale_x: f32, scale_y: f32) -> ScaleFactor {
        ScaleFactor { scale_x, scale_y }
    }

    /// Not [`Unspecified`](ScaleFactor::Unspecified) (its bits, as Compose's packed value compares).
    pub fn is_specified(self) -> bool {
        !self.is_unspecified()
    }
    pub fn is_unspecified(self) -> bool {
        self.scale_x.to_bits() == f32::NAN.to_bits() && self.scale_y.to_bits() == f32::NAN.to_bits()
    }
    pub fn take_or_else(self, block: impl FnOnce() -> ScaleFactor) -> ScaleFactor {
        if self.is_specified() { self } else { block() }
    }

    /// Another x or y scale, each its own unless given.
    pub fn copy(self, scale_x: Option<f32>, scale_y: Option<f32>) -> ScaleFactor {
        ScaleFactor::new(scale_x.unwrap_or(self.scale_x), scale_y.unwrap_or(self.scale_y))
    }
}

/// Compose's toString: "ScaleFactor(x, y)".
impl fmt::Debug for ScaleFactor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScaleFactor({:?}, {:?})", self.scale_x, self.scale_y)
    }
}

impl Mul<f32> for ScaleFactor {
    type Output = ScaleFactor;
    fn mul(self, operand: f32) -> ScaleFactor {
        ScaleFactor::new(self.scale_x * operand, self.scale_y * operand)
    }
}
impl Div<f32> for ScaleFactor {
    type Output = ScaleFactor;
    fn div(self, operand: f32) -> ScaleFactor {
        ScaleFactor::new(self.scale_x / operand, self.scale_y / operand)
    }
}
/// The size scaled each way.
impl Mul<ScaleFactor> for Size {
    type Output = Size;
    fn mul(self, scale_factor: ScaleFactor) -> Size {
        Size::new(self.width * scale_factor.scale_x, self.height * scale_factor.scale_y)
    }
}
impl Mul<Size> for ScaleFactor {
    type Output = Size;
    fn mul(self, size: Size) -> Size {
        size * self
    }
}
impl Div<ScaleFactor> for Size {
    type Output = Size;
    fn div(self, scale_factor: ScaleFactor) -> Size {
        Size::new(self.width / scale_factor.scale_x, self.height / scale_factor.scale_y)
    }
}

/// Between two scale factors (Compose's lerp).
pub fn lerp_scale_factor(start: ScaleFactor, stop: ScaleFactor, fraction: f32) -> ScaleFactor {
    ScaleFactor::new(lerp(start.scale_x, stop.scale_x, fraction), lerp(start.scale_y, stop.scale_y, fraction))
}

/// A content scale of one's own (Compose's ContentScale interface, implemented).
pub trait ComputeScaleFactor: Send + Sync {
    fn compute_scale_factor(&self, src_size: Size, dst_size: Size) -> ScaleFactor;
}

/// How an image (or a painter) of one size fills a space of another (Compose's ContentScale): all of it inside,
/// as big as fits (Fit), covering it, cut at the sides (Crop), stretched (FillBounds), as it is unless too big
/// (Inside), the width or the height filled, scaled by a fixed amount (FixedScale; None: as it is), or one's own
/// (Custom). Two are equal when they are the same scale (a custom one: the very same).
#[derive(Clone)]
pub enum ContentScale {
    Crop,
    Fit,
    FillHeight,
    FillWidth,
    Inside,
    FillBounds,
    /// Compose's FixedScale(value).
    FixedScale(f32),
    Custom(Arc<dyn ComputeScaleFactor>),
}

impl ContentScale {
    /// As it is (Compose's ContentScale.None: FixedScale(1)).
    pub const None: ContentScale = ContentScale::FixedScale(1.0);

    /// How much `src_size` is scaled into `dst_size` (Compose's computeScaleFactor).
    pub fn compute_scale_factor(&self, src_size: Size, dst_size: Size) -> ScaleFactor {
        let fill_width = dst_size.width / src_size.width;
        let fill_height = dst_size.height / src_size.height;
        let uniform = |s: f32| ScaleFactor::new(s, s);
        match self {
            ContentScale::Crop => uniform(fill_width.max(fill_height)),
            ContentScale::Fit => uniform(fill_width.min(fill_height)),
            ContentScale::FillHeight => uniform(fill_height),
            ContentScale::FillWidth => uniform(fill_width),
            ContentScale::Inside => {
                if src_size.width <= dst_size.width && src_size.height <= dst_size.height {
                    uniform(1.0)
                } else {
                    uniform(fill_width.min(fill_height))
                }
            }
            ContentScale::FillBounds => ScaleFactor::new(fill_width, fill_height),
            ContentScale::FixedScale(value) => uniform(*value),
            ContentScale::Custom(scale) => scale.compute_scale_factor(src_size, dst_size),
        }
    }
}

impl PartialEq for ContentScale {
    fn eq(&self, o: &ContentScale) -> bool {
        match (self, o) {
            (ContentScale::Crop, ContentScale::Crop)
            | (ContentScale::Fit, ContentScale::Fit)
            | (ContentScale::FillHeight, ContentScale::FillHeight)
            | (ContentScale::FillWidth, ContentScale::FillWidth)
            | (ContentScale::Inside, ContentScale::Inside)
            | (ContentScale::FillBounds, ContentScale::FillBounds) => true,
            (ContentScale::FixedScale(a), ContentScale::FixedScale(b)) => a == b,
            (ContentScale::Custom(a), ContentScale::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for ContentScale {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ContentScale::Crop => f.write_str("Crop"),
            ContentScale::Fit => f.write_str("Fit"),
            ContentScale::FillHeight => f.write_str("FillHeight"),
            ContentScale::FillWidth => f.write_str("FillWidth"),
            ContentScale::Inside => f.write_str("Inside"),
            ContentScale::FillBounds => f.write_str("FillBounds"),
            ContentScale::FixedScale(value) => write!(f, "FixedScale(value={value:?})"),
            ContentScale::Custom(_) => f.write_str("Custom"),
        }
    }
}
