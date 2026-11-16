//! Geometry (Compose's `androidx.compose.ui.geometry` and the Int types of `ui.unit`): offsets, sizes, rectangles
//! and rounded rectangles, in pixels.

use kompoz_runtime::KotlinProperties;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

/// Between two values.
pub fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

/// A point or a displacement.
#[derive(Clone, Copy, PartialEq, Debug, Default, KotlinProperties)]
pub struct Offset {
    pub x: f32,
    pub y: f32,
}

/// Compose's `Offset(x, y)`.
pub const fn Offset(x: f32, y: f32) -> Offset {
    Offset { x, y }
}

impl Offset {
    pub const Zero: Offset = Offset { x: 0.0, y: 0.0 };
    pub const Infinite: Offset = Offset { x: f32::INFINITY, y: f32::INFINITY };
    /// Not given (NaN both ways).
    pub const Unspecified: Offset = Offset { x: f32::NAN, y: f32::NAN };

    pub const fn new(x: f32, y: f32) -> Offset {
        Offset { x, y }
    }
    /// Its length.
    pub fn get_distance(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn get_distance_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
    /// Neither component NaN.
    pub fn is_valid(self) -> bool {
        !self.x.is_nan() && !self.y.is_nan()
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    pub fn is_specified(self) -> bool {
        !self.is_unspecified()
    }
    pub fn is_unspecified(self) -> bool {
        self.x.is_nan() || self.y.is_nan()
    }
    pub fn take_or_else(self, other: impl FnOnce() -> Offset) -> Offset {
        if self.is_specified() { self } else { other() }
    }
    pub fn copy(self, x: Option<f32>, y: Option<f32>) -> Offset {
        Offset { x: x.unwrap_or(self.x), y: y.unwrap_or(self.y) }
    }
    /// Rounded to whole pixels (Compose's round()).
    pub fn round(self) -> IntOffset {
        IntOffset::new(self.x.round() as i32, self.y.round() as i32)
    }
}

impl Add for Offset {
    type Output = Offset;
    fn add(self, o: Offset) -> Offset {
        Offset::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Offset {
    type Output = Offset;
    fn sub(self, o: Offset) -> Offset {
        Offset::new(self.x - o.x, self.y - o.y)
    }
}
impl Neg for Offset {
    type Output = Offset;
    fn neg(self) -> Offset {
        Offset::new(-self.x, -self.y)
    }
}
impl Mul<f32> for Offset {
    type Output = Offset;
    fn mul(self, k: f32) -> Offset {
        Offset::new(self.x * k, self.y * k)
    }
}
impl Div<f32> for Offset {
    type Output = Offset;
    fn div(self, k: f32) -> Offset {
        Offset::new(self.x / k, self.y / k)
    }
}
impl Rem<f32> for Offset {
    type Output = Offset;
    fn rem(self, k: f32) -> Offset {
        Offset::new(self.x % k, self.y % k)
    }
}

pub fn lerp_offset(start: Offset, stop: Offset, fraction: f32) -> Offset {
    Offset::new(lerp(start.x, stop.x, fraction), lerp(start.y, stop.y, fraction))
}

/// A width and a height.
#[derive(Clone, Copy, PartialEq, Debug, Default, KotlinProperties)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Compose's `Size(width, height)`.
pub const fn Size(width: f32, height: f32) -> Size {
    Size { width, height }
}

impl Size {
    pub const Zero: Size = Size { width: 0.0, height: 0.0 };
    /// Not given (NaN both ways).
    pub const Unspecified: Size = Size { width: f32::NAN, height: f32::NAN };

    pub const fn new(width: f32, height: f32) -> Size {
        Size { width, height }
    }
    pub fn min_dimension(self) -> f32 {
        self.width.abs().min(self.height.abs())
    }
    pub fn max_dimension(self) -> f32 {
        self.width.abs().max(self.height.abs())
    }
    /// No area (or a negative side).
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
    pub fn is_specified(self) -> bool {
        !self.width.is_nan() && !self.height.is_nan()
    }
    pub fn is_unspecified(self) -> bool {
        !self.is_specified()
    }
    pub fn center(self) -> Offset {
        Offset::new(self.width / 2.0, self.height / 2.0)
    }
    pub fn to_rect(self) -> Rect {
        Rect::from_offset_size(Offset::Zero, self)
    }
    pub fn take_or_else(self, other: impl FnOnce() -> Size) -> Size {
        if self.is_specified() { self } else { other() }
    }
    pub fn copy(self, width: Option<f32>, height: Option<f32>) -> Size {
        Size { width: width.unwrap_or(self.width), height: height.unwrap_or(self.height) }
    }
}

impl Mul<f32> for Size {
    type Output = Size;
    fn mul(self, k: f32) -> Size {
        Size::new(self.width * k, self.height * k)
    }
}
impl Div<f32> for Size {
    type Output = Size;
    fn div(self, k: f32) -> Size {
        Size::new(self.width / k, self.height / k)
    }
}

pub fn lerp_size(start: Size, stop: Size, fraction: f32) -> Size {
    Size::new(lerp(start.width, stop.width, fraction), lerp(start.height, stop.height, fraction))
}

/// A rectangle by its edges.
#[derive(Clone, Copy, PartialEq, Debug, Default, KotlinProperties)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Compose's `Rect(…)`: `(left, top, right, bottom)`, `(offset, size)`, `(top_left, bottom_right)` or
/// `(center, radius)`.
pub fn Rect(args: impl RectArgs) -> Rect {
    args.rect()
}

/// What [`Rect()`](fn@Rect) is given: Kotlin's arguments as a tuple, each overload by its types.
pub trait RectArgs {
    fn rect(self) -> Rect;
}
/// Kotlin: `Rect(left: Float, top: Float, right: Float, bottom: Float)`
impl RectArgs for (f32, f32, f32, f32) {
    fn rect(self) -> Rect {
        Rect { left: self.0, top: self.1, right: self.2, bottom: self.3 }
    }
}
/// Kotlin: `Rect(offset: Offset, size: Size)`
impl RectArgs for (Offset, Size) {
    fn rect(self) -> Rect {
        Rect::from_offset_size(self.0, self.1)
    }
}
/// Kotlin: `Rect(topLeft: Offset, bottomRight: Offset)`
impl RectArgs for (Offset, Offset) {
    fn rect(self) -> Rect {
        Rect::from_corners(self.0, self.1)
    }
}
/// Kotlin: `Rect(center: Offset, radius: Float)`
impl RectArgs for (Offset, f32) {
    fn rect(self) -> Rect {
        Rect::from_center(self.0, self.1)
    }
}

impl Rect {
    pub const Zero: Rect = Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect { left, top, right, bottom }
    }
    /// `Rect(offset, size)`.
    pub fn from_offset_size(offset: Offset, size: Size) -> Rect {
        Rect::new(offset.x, offset.y, offset.x + size.width, offset.y + size.height)
    }
    /// `Rect(topLeft, bottomRight)`.
    pub fn from_corners(top_left: Offset, bottom_right: Offset) -> Rect {
        Rect::new(top_left.x, top_left.y, bottom_right.x, bottom_right.y)
    }
    /// `Rect(center, radius)`: the square around a circle.
    pub fn from_center(center: Offset, radius: f32) -> Rect {
        Rect::new(center.x - radius, center.y - radius, center.x + radius, center.y + radius)
    }

    pub fn width(self) -> f32 {
        self.right - self.left
    }
    pub fn height(self) -> f32 {
        self.bottom - self.top
    }
    pub fn size(self) -> Size {
        Size::new(self.width(), self.height())
    }
    pub fn is_infinite(self) -> bool {
        self.left.is_infinite() || self.top.is_infinite() || self.right.is_infinite() || self.bottom.is_infinite()
    }
    pub fn is_finite(self) -> bool {
        self.left.is_finite() && self.top.is_finite() && self.right.is_finite() && self.bottom.is_finite()
    }
    /// No area (or edges the wrong way round).
    pub fn is_empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }
    pub fn translate(self, offset: Offset) -> Rect {
        Rect::new(self.left + offset.x, self.top + offset.y, self.right + offset.x, self.bottom + offset.y)
    }
    pub fn translate_by(self, dx: f32, dy: f32) -> Rect {
        self.translate(Offset::new(dx, dy))
    }
    /// Grown by `delta` on every side.
    pub fn inflate(self, delta: f32) -> Rect {
        Rect::new(self.left - delta, self.top - delta, self.right + delta, self.bottom + delta)
    }
    pub fn deflate(self, delta: f32) -> Rect {
        self.inflate(-delta)
    }
    /// The overlap (edges the wrong way round where there is none, as Compose's).
    pub fn intersect(self, o: Rect) -> Rect {
        Rect::new(self.left.max(o.left), self.top.max(o.top), self.right.min(o.right), self.bottom.min(o.bottom))
    }
    pub fn overlaps(self, o: Rect) -> bool {
        !(self.right <= o.left || o.right <= self.left || self.bottom <= o.top || o.bottom <= self.top)
    }
    pub fn min_dimension(self) -> f32 {
        self.width().abs().min(self.height().abs())
    }
    pub fn max_dimension(self) -> f32 {
        self.width().abs().max(self.height().abs())
    }
    pub fn top_left(self) -> Offset {
        Offset::new(self.left, self.top)
    }
    pub fn top_center(self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.top)
    }
    pub fn top_right(self) -> Offset {
        Offset::new(self.right, self.top)
    }
    pub fn center_left(self) -> Offset {
        Offset::new(self.left, self.top + self.height() / 2.0)
    }
    pub fn center(self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.top + self.height() / 2.0)
    }
    pub fn center_right(self) -> Offset {
        Offset::new(self.right, self.top + self.height() / 2.0)
    }
    pub fn bottom_left(self) -> Offset {
        Offset::new(self.left, self.bottom)
    }
    pub fn bottom_center(self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.bottom)
    }
    pub fn bottom_right(self) -> Offset {
        Offset::new(self.right, self.bottom)
    }
    /// Whether `point` is in it (its left and top edges in, its right and bottom ones out).
    pub fn contains(self, point: Offset) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }
    /// The smallest rectangle holding both.
    pub fn union(self, o: Rect) -> Rect {
        Rect::new(self.left.min(o.left), self.top.min(o.top), self.right.max(o.right), self.bottom.max(o.bottom))
    }
    /// Its edges rounded outwards to whole pixels.
    pub fn round_out(self) -> IntRect {
        IntRect::new(
            self.left.floor() as i32,
            self.top.floor() as i32,
            self.right.ceil() as i32,
            self.bottom.ceil() as i32,
        )
    }
    /// Its edges rounded to the nearest whole pixels (Compose's roundToIntRect).
    pub fn round_to_int_rect(self) -> IntRect {
        IntRect::new(
            self.left.round() as i32,
            self.top.round() as i32,
            self.right.round() as i32,
            self.bottom.round() as i32,
        )
    }
}

pub fn lerp_rect(start: Rect, stop: Rect, fraction: f32) -> Rect {
    Rect::new(
        lerp(start.left, stop.left, fraction),
        lerp(start.top, stop.top, fraction),
        lerp(start.right, stop.right, fraction),
        lerp(start.bottom, stop.bottom, fraction),
    )
}

/// The radii of a corner's ellipse.
#[derive(Clone, Copy, PartialEq, Debug, Default, KotlinProperties)]
pub struct CornerRadius {
    pub x: f32,
    pub y: f32,
}

/// Compose's `CornerRadius(x, y = x)`: one radius both ways, or `(x, y)`.
pub fn CornerRadius(args: impl CornerRadiusArgs) -> CornerRadius {
    args.corner_radius()
}

/// What [`CornerRadius()`](fn@CornerRadius) is given: Kotlin's arguments, one as itself, two as a pair.
pub trait CornerRadiusArgs {
    fn corner_radius(self) -> CornerRadius;
}
/// Kotlin: `CornerRadius(x: Float)`
impl CornerRadiusArgs for f32 {
    fn corner_radius(self) -> CornerRadius {
        CornerRadius { x: self, y: self }
    }
}
/// Kotlin: `CornerRadius(x: Float, y: Float)`
impl CornerRadiusArgs for (f32, f32) {
    fn corner_radius(self) -> CornerRadius {
        CornerRadius { x: self.0, y: self.1 }
    }
}

impl CornerRadius {
    pub const Zero: CornerRadius = CornerRadius { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> CornerRadius {
        CornerRadius { x, y }
    }
    /// The same both ways.
    pub const fn circular(r: f32) -> CornerRadius {
        CornerRadius { x: r, y: r }
    }
}

impl Mul<f32> for CornerRadius {
    type Output = CornerRadius;
    fn mul(self, k: f32) -> CornerRadius {
        CornerRadius::new(self.x * k, self.y * k)
    }
}

pub fn lerp_corner_radius(start: CornerRadius, stop: CornerRadius, fraction: f32) -> CornerRadius {
    CornerRadius::new(lerp(start.x, stop.x, fraction), lerp(start.y, stop.y, fraction))
}

/// A rectangle with rounded corners (Compose's RoundRect).
#[derive(Clone, Copy, PartialEq, Debug, Default, KotlinProperties)]
pub struct RoundRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub top_left_corner_radius: CornerRadius,
    pub top_right_corner_radius: CornerRadius,
    pub bottom_right_corner_radius: CornerRadius,
    pub bottom_left_corner_radius: CornerRadius,
}

impl RoundRect {
    pub const Zero: RoundRect = RoundRect {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        top_left_corner_radius: CornerRadius::Zero,
        top_right_corner_radius: CornerRadius::Zero,
        bottom_right_corner_radius: CornerRadius::Zero,
        bottom_left_corner_radius: CornerRadius::Zero,
    };

    /// `RoundRect(rect, topLeft, topRight, bottomRight, bottomLeft)`.
    pub fn from_rect(
        rect: Rect,
        top_left: CornerRadius,
        top_right: CornerRadius,
        bottom_right: CornerRadius,
        bottom_left: CornerRadius,
    ) -> RoundRect {
        RoundRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            top_left_corner_radius: top_left,
            top_right_corner_radius: top_right,
            bottom_right_corner_radius: bottom_right,
            bottom_left_corner_radius: bottom_left,
        }
    }
    /// Every corner the same.
    pub fn from_rect_radius(rect: Rect, radius: CornerRadius) -> RoundRect {
        RoundRect::from_rect(rect, radius, radius, radius, radius)
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
    pub fn bounding_rect(&self) -> Rect {
        Rect::new(self.left, self.top, self.right, self.bottom)
    }
    pub fn is_empty(&self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }
    /// No corner rounded.
    pub fn is_rect(&self) -> bool {
        let zero = |c: CornerRadius| c.x == 0.0 || c.y == 0.0;
        zero(self.top_left_corner_radius)
            && zero(self.top_right_corner_radius)
            && zero(self.bottom_right_corner_radius)
            && zero(self.bottom_left_corner_radius)
    }
    /// Every corner the same, and as round one way as the other.
    pub fn is_simple(&self) -> bool {
        let c = self.top_left_corner_radius;
        c.x == c.y
            && c == self.top_right_corner_radius
            && c == self.bottom_right_corner_radius
            && c == self.bottom_left_corner_radius
    }
    pub fn translate(&self, offset: Offset) -> RoundRect {
        RoundRect {
            left: self.left + offset.x,
            top: self.top + offset.y,
            right: self.right + offset.x,
            bottom: self.bottom + offset.y,
            ..*self
        }
    }

    /// Its radii scaled down so that neighbouring corners don't overlap (what Compose draws and tests with).
    pub fn scaled_radii(&self) -> RoundRect {
        let (w, h) = (self.width().abs(), self.height().abs());
        let (tl, tr, br, bl) = (
            self.top_left_corner_radius,
            self.top_right_corner_radius,
            self.bottom_right_corner_radius,
            self.bottom_left_corner_radius,
        );
        let mut scale = 1.0f32;
        let mut fit = |sum: f32, side: f32| {
            if sum > side && sum > 0.0 {
                scale = scale.min(side / sum);
            }
        };
        fit(tl.y + bl.y, h);
        fit(tl.x + tr.x, w);
        fit(tr.y + br.y, h);
        fit(bl.x + br.x, w);
        if scale >= 1.0 {
            return *self;
        }
        RoundRect {
            top_left_corner_radius: tl * scale,
            top_right_corner_radius: tr * scale,
            bottom_right_corner_radius: br * scale,
            bottom_left_corner_radius: bl * scale,
            ..*self
        }
    }

    /// Whether `point` is inside it, the corners' curves counted.
    pub fn contains(&self, point: Offset) -> bool {
        if point.x < self.left || point.x >= self.right || point.y < self.top || point.y >= self.bottom {
            return false;
        }
        let r = self.scaled_radii();
        // Whether the point is outside a corner's ellipse, in that corner's square.
        let outside = |cx: f32, cy: f32, radius: CornerRadius| {
            if radius.x <= 0.0 || radius.y <= 0.0 {
                return false;
            }
            let (dx, dy) = ((point.x - cx) / radius.x, (point.y - cy) / radius.y);
            dx * dx + dy * dy > 1.0
        };
        let (tl, tr, br, bl) = (
            r.top_left_corner_radius,
            r.top_right_corner_radius,
            r.bottom_right_corner_radius,
            r.bottom_left_corner_radius,
        );
        if point.x < self.left + tl.x && point.y < self.top + tl.y {
            return !outside(self.left + tl.x, self.top + tl.y, tl);
        }
        if point.x >= self.right - tr.x && point.y < self.top + tr.y {
            return !outside(self.right - tr.x, self.top + tr.y, tr);
        }
        if point.x >= self.right - br.x && point.y >= self.bottom - br.y {
            return !outside(self.right - br.x, self.bottom - br.y, br);
        }
        if point.x < self.left + bl.x && point.y >= self.bottom - bl.y {
            return !outside(self.left + bl.x, self.bottom - bl.y, bl);
        }
        true
    }
}

/// A position in whole pixels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, KotlinProperties)]
pub struct IntOffset {
    pub x: i32,
    pub y: i32,
}

/// Compose's `IntOffset(x, y)`.
pub const fn IntOffset(x: i32, y: i32) -> IntOffset {
    IntOffset { x, y }
}

impl IntOffset {
    pub const Zero: IntOffset = IntOffset { x: 0, y: 0 };
    pub const fn new(x: i32, y: i32) -> IntOffset {
        IntOffset { x, y }
    }
    pub fn to_offset(self) -> Offset {
        Offset::new(self.x as f32, self.y as f32)
    }
}

impl Add for IntOffset {
    type Output = IntOffset;
    fn add(self, o: IntOffset) -> IntOffset {
        IntOffset::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for IntOffset {
    type Output = IntOffset;
    fn sub(self, o: IntOffset) -> IntOffset {
        IntOffset::new(self.x - o.x, self.y - o.y)
    }
}
impl Neg for IntOffset {
    type Output = IntOffset;
    fn neg(self) -> IntOffset {
        IntOffset::new(-self.x, -self.y)
    }
}

/// A size in whole pixels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, KotlinProperties)]
pub struct IntSize {
    pub width: i32,
    pub height: i32,
}

/// Compose's `IntSize(width, height)`.
pub const fn IntSize(width: i32, height: i32) -> IntSize {
    IntSize { width, height }
}

impl IntSize {
    pub const Zero: IntSize = IntSize { width: 0, height: 0 };
    pub const fn new(width: i32, height: i32) -> IntSize {
        IntSize { width, height }
    }
    pub fn to_size(self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }
    pub fn center(self) -> IntOffset {
        IntOffset::new(self.width / 2, self.height / 2)
    }
}

/// A rectangle in whole pixels, by its edges.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, KotlinProperties)]
pub struct IntRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Compose's `IntRect(…)`: `(left, top, right, bottom)`, `(offset, size)`, `(top_left, bottom_right)` or
/// `(center, radius)`.
pub fn IntRect(args: impl IntRectArgs) -> IntRect {
    args.int_rect()
}

/// What [`IntRect()`](fn@IntRect) is given: Kotlin's arguments as a tuple, each overload by its types.
pub trait IntRectArgs {
    fn int_rect(self) -> IntRect;
}
/// Kotlin: `IntRect(left: Int, top: Int, right: Int, bottom: Int)`
impl IntRectArgs for (i32, i32, i32, i32) {
    fn int_rect(self) -> IntRect {
        IntRect { left: self.0, top: self.1, right: self.2, bottom: self.3 }
    }
}
/// Kotlin: `IntRect(offset: IntOffset, size: IntSize)`
impl IntRectArgs for (IntOffset, IntSize) {
    fn int_rect(self) -> IntRect {
        IntRect::from_offset_size(self.0, self.1)
    }
}
/// Kotlin: `IntRect(topLeft: IntOffset, bottomRight: IntOffset)`
impl IntRectArgs for (IntOffset, IntOffset) {
    fn int_rect(self) -> IntRect {
        IntRect { left: self.0.x, top: self.0.y, right: self.1.x, bottom: self.1.y }
    }
}
/// Kotlin: `IntRect(center: IntOffset, radius: Int)`
impl IntRectArgs for (IntOffset, i32) {
    fn int_rect(self) -> IntRect {
        let (c, r) = self;
        IntRect { left: c.x - r, top: c.y - r, right: c.x + r, bottom: c.y + r }
    }
}

impl IntRect {
    pub const Zero: IntRect = IntRect { left: 0, top: 0, right: 0, bottom: 0 };
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> IntRect {
        IntRect { left, top, right, bottom }
    }
    pub fn from_offset_size(offset: IntOffset, size: IntSize) -> IntRect {
        IntRect::new(offset.x, offset.y, offset.x + size.width, offset.y + size.height)
    }
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
    pub fn size(self) -> IntSize {
        IntSize::new(self.width(), self.height())
    }
    pub fn top_left(self) -> IntOffset {
        IntOffset::new(self.left, self.top)
    }
    pub fn center(self) -> IntOffset {
        IntOffset::new((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
    pub fn is_empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }
    pub fn translate(self, offset: IntOffset) -> IntRect {
        IntRect::new(self.left + offset.x, self.top + offset.y, self.right + offset.x, self.bottom + offset.y)
    }
    pub fn intersect(self, o: IntRect) -> IntRect {
        IntRect::new(self.left.max(o.left), self.top.max(o.top), self.right.min(o.right), self.bottom.min(o.bottom))
    }
    pub fn contains(self, point: IntOffset) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }
    pub fn to_rect(self) -> Rect {
        Rect::new(self.left as f32, self.top as f32, self.right as f32, self.bottom as f32)
    }
}
