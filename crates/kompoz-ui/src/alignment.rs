//! Alignments, as Compose's `Alignment`: where a box of one size goes in a space of another, by biases from -1 (the
//! start or top) to 1 (the end or bottom), mirrored right to left (or not, the absolute ones).

use kompoz_graphics::{IntOffset, IntSize, LayoutDirection};
use std::fmt;
use std::ops::Add;
use std::rc::Rc;

/// Kotlin's `roundToInt` on a float (Math.round: a half up).
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// Where a box goes in a space, both ways (Compose's Alignment): by biases ([`BiasAlignment`], mirrored right to
/// left), by absolute biases ([`BiasAbsoluteAlignment`], not mirrored), or as a function says.
#[derive(Clone)]
pub enum Alignment {
    Bias(BiasAlignment),
    Absolute(BiasAbsoluteAlignment),
    Custom(Rc<dyn Fn(IntSize, IntSize, LayoutDirection) -> IntOffset>),
}

impl Alignment {
    pub const TopStart: Alignment = Alignment::Bias(BiasAlignment::new(-1.0, -1.0));
    pub const TopCenter: Alignment = Alignment::Bias(BiasAlignment::new(0.0, -1.0));
    pub const TopEnd: Alignment = Alignment::Bias(BiasAlignment::new(1.0, -1.0));
    pub const CenterStart: Alignment = Alignment::Bias(BiasAlignment::new(-1.0, 0.0));
    pub const Center: Alignment = Alignment::Bias(BiasAlignment::new(0.0, 0.0));
    pub const CenterEnd: Alignment = Alignment::Bias(BiasAlignment::new(1.0, 0.0));
    pub const BottomStart: Alignment = Alignment::Bias(BiasAlignment::new(-1.0, 1.0));
    pub const BottomCenter: Alignment = Alignment::Bias(BiasAlignment::new(0.0, 1.0));
    pub const BottomEnd: Alignment = Alignment::Bias(BiasAlignment::new(1.0, 1.0));

    pub const Top: VerticalAlignment = VerticalAlignment::Bias(-1.0);
    pub const CenterVertically: VerticalAlignment = VerticalAlignment::Bias(0.0);
    pub const Bottom: VerticalAlignment = VerticalAlignment::Bias(1.0);

    pub const Start: HorizontalAlignment = HorizontalAlignment::Bias(-1.0);
    pub const CenterHorizontally: HorizontalAlignment = HorizontalAlignment::Bias(0.0);
    pub const End: HorizontalAlignment = HorizontalAlignment::Bias(1.0);

    /// An alignment a function works out (Compose's `Alignment { size, space, layoutDirection -> … }`).
    pub fn custom(align: impl Fn(IntSize, IntSize, LayoutDirection) -> IntOffset + 'static) -> Alignment {
        Alignment::Custom(Rc::new(align))
    }

    /// Where a box of `size` goes in `space` (from its top left).
    pub fn align(&self, size: IntSize, space: IntSize, layout_direction: LayoutDirection) -> IntOffset {
        match self {
            Alignment::Bias(b) => b.align(size, space, layout_direction),
            Alignment::Absolute(b) => b.align(size, space, layout_direction),
            Alignment::Custom(f) => f(size, space, layout_direction),
        }
    }
}

impl PartialEq for Alignment {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Alignment::Bias(a), Alignment::Bias(b)) => a == b,
            (Alignment::Absolute(a), Alignment::Absolute(b)) => a == b,
            (Alignment::Custom(a), Alignment::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Alignment::Bias(b) => write!(f, "{b:?}"),
            Alignment::Absolute(b) => write!(f, "{b:?}"),
            Alignment::Custom(_) => write!(f, "Alignment(custom)"),
        }
    }
}

/// Where a box goes across (Compose's Alignment.Horizontal).
#[derive(Clone)]
pub enum HorizontalAlignment {
    /// From -1 (the start) to 1 (the end), mirrored right to left.
    Bias(f32),
    /// From -1 (the left) to 1 (the right), whatever the direction.
    Absolute(f32),
    Custom(Rc<dyn Fn(i32, i32, LayoutDirection) -> i32>),
}

impl HorizontalAlignment {
    pub fn custom(align: impl Fn(i32, i32, LayoutDirection) -> i32 + 'static) -> HorizontalAlignment {
        HorizontalAlignment::Custom(Rc::new(align))
    }

    /// Where a line of `size` goes in `space`.
    pub fn align(&self, size: i32, space: i32, layout_direction: LayoutDirection) -> i32 {
        let center = (space - size) as f32 / 2.0;
        match self {
            HorizontalAlignment::Bias(bias) => {
                let resolved = if layout_direction == LayoutDirection::Ltr { *bias } else { -bias };
                round(center * (1.0 + resolved))
            }
            HorizontalAlignment::Absolute(bias) => round(center * (1.0 + bias)),
            HorizontalAlignment::Custom(f) => f(size, space, layout_direction),
        }
    }
}

impl PartialEq for HorizontalAlignment {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HorizontalAlignment::Bias(a), HorizontalAlignment::Bias(b)) => a == b,
            (HorizontalAlignment::Absolute(a), HorizontalAlignment::Absolute(b)) => a == b,
            (HorizontalAlignment::Custom(a), HorizontalAlignment::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for HorizontalAlignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HorizontalAlignment::Bias(b) => write!(f, "Horizontal(bias={b})"),
            HorizontalAlignment::Absolute(b) => write!(f, "AbsoluteHorizontal(bias={b})"),
            HorizontalAlignment::Custom(_) => write!(f, "Horizontal(custom)"),
        }
    }
}

/// Where a box goes down (Compose's Alignment.Vertical).
#[derive(Clone)]
pub enum VerticalAlignment {
    /// From -1 (the top) to 1 (the bottom).
    Bias(f32),
    Custom(Rc<dyn Fn(i32, i32) -> i32>),
}

impl VerticalAlignment {
    pub fn custom(align: impl Fn(i32, i32) -> i32 + 'static) -> VerticalAlignment {
        VerticalAlignment::Custom(Rc::new(align))
    }

    pub fn align(&self, size: i32, space: i32) -> i32 {
        match self {
            VerticalAlignment::Bias(bias) => round((space - size) as f32 / 2.0 * (1.0 + bias)),
            VerticalAlignment::Custom(f) => f(size, space),
        }
    }
}

impl PartialEq for VerticalAlignment {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VerticalAlignment::Bias(a), VerticalAlignment::Bias(b)) => a == b,
            (VerticalAlignment::Custom(a), VerticalAlignment::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for VerticalAlignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VerticalAlignment::Bias(b) => write!(f, "Vertical(bias={b})"),
            VerticalAlignment::Custom(_) => write!(f, "Vertical(custom)"),
        }
    }
}

/// `horizontal + vertical`: the two together (Compose's `Alignment.Horizontal.plus`).
impl Add<VerticalAlignment> for HorizontalAlignment {
    type Output = Alignment;
    fn add(self, vertical: VerticalAlignment) -> Alignment {
        match (&self, &vertical) {
            (HorizontalAlignment::Bias(h), VerticalAlignment::Bias(v)) => Alignment::Bias(BiasAlignment::new(*h, *v)),
            (HorizontalAlignment::Absolute(h), VerticalAlignment::Bias(v)) => {
                Alignment::Absolute(BiasAbsoluteAlignment::new(*h, *v))
            }
            _ => Alignment::custom(move |size, space, dir| {
                IntOffset::new(self.align(size.width, space.width, dir), vertical.align(size.height, space.height))
            }),
        }
    }
}

/// `vertical + horizontal` (Compose's `Alignment.Vertical.plus`).
impl Add<HorizontalAlignment> for VerticalAlignment {
    type Output = Alignment;
    fn add(self, horizontal: HorizontalAlignment) -> Alignment {
        horizontal + self
    }
}

/// An alignment by biases, the horizontal one mirrored right to left (Compose's BiasAlignment).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BiasAlignment {
    pub horizontal_bias: f32,
    pub vertical_bias: f32,
}

impl BiasAlignment {
    pub const fn new(horizontal_bias: f32, vertical_bias: f32) -> BiasAlignment {
        BiasAlignment { horizontal_bias, vertical_bias }
    }

    pub fn align(&self, size: IntSize, space: IntSize, layout_direction: LayoutDirection) -> IntOffset {
        let center_x = (space.width - size.width) as f32 / 2.0;
        let center_y = (space.height - size.height) as f32 / 2.0;
        let resolved =
            if layout_direction == LayoutDirection::Ltr { self.horizontal_bias } else { -self.horizontal_bias };
        IntOffset::new(round(center_x * (1.0 + resolved)), round(center_y * (1.0 + self.vertical_bias)))
    }

    /// Compose's `BiasAlignment.Horizontal(bias)`.
    pub const fn horizontal(bias: f32) -> HorizontalAlignment {
        HorizontalAlignment::Bias(bias)
    }

    /// Compose's `BiasAlignment.Vertical(bias)`.
    pub const fn vertical(bias: f32) -> VerticalAlignment {
        VerticalAlignment::Bias(bias)
    }
}

impl From<BiasAlignment> for Alignment {
    fn from(b: BiasAlignment) -> Alignment {
        Alignment::Bias(b)
    }
}

/// An alignment by biases from the left, whatever the layout direction (Compose's BiasAbsoluteAlignment).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BiasAbsoluteAlignment {
    pub horizontal_bias: f32,
    pub vertical_bias: f32,
}

impl BiasAbsoluteAlignment {
    pub const fn new(horizontal_bias: f32, vertical_bias: f32) -> BiasAbsoluteAlignment {
        BiasAbsoluteAlignment { horizontal_bias, vertical_bias }
    }

    pub fn align(&self, size: IntSize, space: IntSize, _layout_direction: LayoutDirection) -> IntOffset {
        let remaining_x = (space.width - size.width) as f32;
        let remaining_y = (space.height - size.height) as f32;
        let x = round(remaining_x / 2.0 * (1.0 + self.horizontal_bias));
        let y = round(remaining_y / 2.0 * (1.0 + self.vertical_bias));
        IntOffset::new(x, y)
    }

    /// Compose's `BiasAbsoluteAlignment.Horizontal(bias)`.
    pub const fn horizontal(bias: f32) -> HorizontalAlignment {
        HorizontalAlignment::Absolute(bias)
    }
}

impl From<BiasAbsoluteAlignment> for Alignment {
    fn from(b: BiasAbsoluteAlignment) -> Alignment {
        Alignment::Absolute(b)
    }
}

/// Alignments that don't mirror right to left (Compose's AbsoluteAlignment).
pub struct AbsoluteAlignment;

impl AbsoluteAlignment {
    pub const TopLeft: Alignment = Alignment::Absolute(BiasAbsoluteAlignment::new(-1.0, -1.0));
    pub const TopRight: Alignment = Alignment::Absolute(BiasAbsoluteAlignment::new(1.0, -1.0));
    pub const CenterLeft: Alignment = Alignment::Absolute(BiasAbsoluteAlignment::new(-1.0, 0.0));
    pub const CenterRight: Alignment = Alignment::Absolute(BiasAbsoluteAlignment::new(1.0, 0.0));
    pub const BottomLeft: Alignment = Alignment::Absolute(BiasAbsoluteAlignment::new(-1.0, 1.0));
    pub const BottomRight: Alignment = Alignment::Absolute(BiasAbsoluteAlignment::new(1.0, 1.0));
    pub const Left: HorizontalAlignment = HorizontalAlignment::Absolute(-1.0);
    pub const Right: HorizontalAlignment = HorizontalAlignment::Absolute(1.0);
}
