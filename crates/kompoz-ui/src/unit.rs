//! Constraints, the sizes a layout may take, and Velocity (Compose's `ui-unit` Constraints and Velocity).

use kompoz_graphics::IntSize;
use kompoz_runtime::named;
use std::fmt;

/// The range of sizes a layout may measure itself to, in whole pixels (Compose's Constraints): a width from
/// `min_width` to `max_width` and a height from `min_height` to `max_height`; a max of [`Constraints::Infinity`]
/// leaves that side unbounded. Compose packs the four into a `Long` (which caps them at 2^18 px); here they are
/// plain `i32`s, so any size goes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Constraints {
    pub min_width: i32,
    pub max_width: i32,
    pub min_height: i32,
    pub max_height: i32,
}

impl Default for Constraints {
    /// Anything: `0..Infinity` both ways.
    fn default() -> Self {
        Constraints { min_width: 0, max_width: Constraints::Infinity, min_height: 0, max_height: Constraints::Infinity }
    }
}

impl Constraints {
    /// A max that leaves its side unbounded.
    pub const Infinity: i32 = i32::MAX;

    /// Compose's `Constraints(minWidth, maxWidth, minHeight, maxHeight)`: the mins at least 0, each max at least
    /// its min (panics otherwise, as Compose's does).
    #[track_caller]
    pub fn new(min_width: i32, max_width: i32, min_height: i32, max_height: i32) -> Constraints {
        assert!(
            max_width >= min_width && max_height >= min_height && min_width >= 0 && min_height >= 0,
            "kompoz: maxWidth must be >= than minWidth, maxHeight must be >= than minHeight, minWidth and \
             minHeight must be >= 0 (got {min_width}..{max_width} x {min_height}..{max_height})"
        );
        Constraints { min_width, max_width, min_height, max_height }
    }

    /// Exactly `width` by `height`.
    #[track_caller]
    pub fn fixed(width: i32, height: i32) -> Constraints {
        assert!(width >= 0 && height >= 0, "kompoz: width({width}) and height({height}) must be >= 0");
        Constraints::new(width, width, height, height)
    }

    /// Exactly `width` wide, any height.
    #[track_caller]
    pub fn fixed_width(width: i32) -> Constraints {
        assert!(width >= 0, "kompoz: width({width}) must be >= 0");
        Constraints::new(width, width, 0, Constraints::Infinity)
    }

    /// Exactly `height` high, any width.
    #[track_caller]
    pub fn fixed_height(height: i32) -> Constraints {
        assert!(height >= 0, "kompoz: height({height}) must be >= 0");
        Constraints::new(0, Constraints::Infinity, height, height)
    }

    /// Compose's `Constraints.fitPrioritizingWidth`: the range given, the width kept as it is and the height made to
    /// fit what can be represented — everything can be here, so it is the range given (mins clamped to their maxes).
    pub fn fit_prioritizing_width(min_width: i32, max_width: i32, min_height: i32, max_height: i32) -> Constraints {
        Constraints::new(min_width.min(max_width), max_width, min_height.min(max_height), max_height)
    }

    /// Compose's `Constraints.fitPrioritizingHeight` (as [`fit_prioritizing_width`](Self::fit_prioritizing_width)).
    pub fn fit_prioritizing_height(min_width: i32, max_width: i32, min_height: i32, max_height: i32) -> Constraints {
        Constraints::fit_prioritizing_width(min_width, max_width, min_height, max_height)
    }

    /// Compose's `Constraints.restrictConstraints`: the range given, the width or the height first as
    /// `prioritize_width` says (both fit here).
    pub fn restrict_constraints(
        min_width: i32,
        max_width: i32,
        min_height: i32,
        max_height: i32,
        prioritize_width: bool,
    ) -> Constraints {
        if prioritize_width {
            Constraints::fit_prioritizing_width(min_width, max_width, min_height, max_height)
        } else {
            Constraints::fit_prioritizing_height(min_width, max_width, min_height, max_height)
        }
    }

    /// Whether the width has a bound (its max isn't Infinity).
    pub fn has_bounded_width(&self) -> bool {
        self.max_width != Constraints::Infinity
    }
    pub fn has_bounded_height(&self) -> bool {
        self.max_height != Constraints::Infinity
    }
    /// Whether only one width fits.
    pub fn has_fixed_width(&self) -> bool {
        self.max_width == self.min_width
    }
    pub fn has_fixed_height(&self) -> bool {
        self.max_height == self.min_height
    }
    /// Whether nothing bigger than nothing fits one way or the other.
    pub fn is_zero(&self) -> bool {
        self.max_width == 0 || self.max_height == 0
    }

    /// `other` brought within these (each of its bounds clamped into this range).
    pub fn constrain(&self, other: Constraints) -> Constraints {
        Constraints::new(
            other.min_width.clamp(self.min_width, self.max_width),
            other.max_width.clamp(self.min_width, self.max_width),
            other.min_height.clamp(self.min_height, self.max_height),
            other.max_height.clamp(self.min_height, self.max_height),
        )
    }

    /// The nearest size to `size` these allow.
    pub fn constrain_size(&self, size: IntSize) -> IntSize {
        IntSize::new(self.constrain_width(size.width), self.constrain_height(size.height))
    }
    pub fn constrain_width(&self, width: i32) -> i32 {
        width.clamp(self.min_width, self.max_width)
    }
    pub fn constrain_height(&self, height: i32) -> i32 {
        height.clamp(self.min_height, self.max_height)
    }

    /// Whether `size` is within these.
    pub fn is_satisfied_by(&self, size: IntSize) -> bool {
        (self.min_width..=self.max_width).contains(&size.width)
            && (self.min_height..=self.max_height).contains(&size.height)
    }

    /// The same without the mins (Compose's `copyMaxDimensions`).
    pub fn copy_max_dimensions(&self) -> Constraints {
        Constraints::new(0, self.max_width, 0, self.max_height)
    }
}

#[named]
impl Constraints {
    /// Grown (or shrunk, for negative values) by `horizontal` and `vertical` pixels: the mins at least 0, an
    /// unbounded max left so (Compose's `Constraints.offset`; both given here, as plain Rust has no defaults).
    pub fn offset(&self, horizontal: i32, vertical: i32) -> Constraints {
        let add_max = |max: i32, value: i32| if max == Constraints::Infinity { max } else { (max + value).max(0) };
        Constraints::new(
            (self.min_width + horizontal).max(0),
            add_max(self.max_width, horizontal),
            (self.min_height + vertical).max(0),
            add_max(self.max_height, vertical),
        )
    }

    /// A copy with some bounds replaced (Compose's `copy`).
    #[track_caller]
    pub fn copy(
        &self,
        #[default(self.min_width)] min_width: i32,
        #[default(self.max_width)] max_width: i32,
        #[default(self.min_height)] min_height: i32,
        #[default(self.max_height)] max_height: i32,
    ) -> Constraints {
        Constraints::new(min_width, max_width, min_height, max_height)
    }
}

/// Compose's `Constraints(minWidth = 0, maxWidth = Infinity, minHeight = 0, maxHeight = Infinity)`, with named
/// arguments in a kompozable body (`Constraints(max_width = 100)`); plain code calls [`Constraints::new`].
#[named]
pub fn Constraints(
    #[default(0)] min_width: i32,
    #[default(Constraints::Infinity)] max_width: i32,
    #[default(0)] min_height: i32,
    #[default(Constraints::Infinity)] max_height: i32,
) -> Constraints {
    Constraints::new(min_width, max_width, min_height, max_height)
}

impl fmt::Debug for Constraints {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let max = |v: i32| if v == Constraints::Infinity { "Infinity".to_string() } else { v.to_string() };
        write!(
            f,
            "Constraints(minWidth = {}, maxWidth = {}, minHeight = {}, maxHeight = {})",
            self.min_width,
            max(self.max_width),
            self.min_height,
            max(self.max_height)
        )
    }
}

/// How fast something moves, in px per second along each axis (Compose's `ui-unit` Velocity): what a velocity
/// tracker measures, and what flings and nested scrolling hand on.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Velocity {
    pub const Zero: Velocity = Velocity { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Velocity {
        Velocity { x, y }
    }
    /// A copy with an axis replaced (Compose's `copy`).
    pub fn copy(self, x: Option<f32>, y: Option<f32>) -> Velocity {
        Velocity { x: x.unwrap_or(self.x), y: y.unwrap_or(self.y) }
    }
}

impl std::ops::Add for Velocity {
    type Output = Velocity;
    fn add(self, o: Velocity) -> Velocity {
        Velocity::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Velocity {
    type Output = Velocity;
    fn sub(self, o: Velocity) -> Velocity {
        Velocity::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Neg for Velocity {
    type Output = Velocity;
    fn neg(self) -> Velocity {
        Velocity::new(-self.x, -self.y)
    }
}

impl std::ops::Mul<f32> for Velocity {
    type Output = Velocity;
    fn mul(self, k: f32) -> Velocity {
        Velocity::new(self.x * k, self.y * k)
    }
}

impl std::ops::Div<f32> for Velocity {
    type Output = Velocity;
    fn div(self, k: f32) -> Velocity {
        Velocity::new(self.x / k, self.y / k)
    }
}

impl fmt::Debug for Velocity {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:.2}, {:.2}) px/sec", self.x, self.y)
    }
}
