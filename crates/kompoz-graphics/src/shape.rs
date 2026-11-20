//! Shapes (Compose's `Shape` and foundation's shapes): what backgrounds, borders, clips and shadows take the form
//! of, each making an [`Outline`] in a size, a layout direction and a density. Rectangles, circles, rounded and cut
//! corners (each corner a [`CornerSize`](struct@CornerSize): dp, px or a share of the shorter side), shapes of a
//! path or an outline of one's own, and their lerp.

use crate::geometry::{CornerRadius, RoundRect, Size, lerp};
use crate::path::{Outline, Path};
use crate::unit::{Density, Dp, LayoutDirection};
use kompoz_runtime::named;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

// ---- Shape -----------------------------------------------------------------------------------------------------

/// Equality across shapes of any type, for [`Shape`] (any `PartialEq` type has it).
pub trait ShapeEq: Any {
    /// The shape itself (call it on a `&dyn Shape`, not on what holds one).
    fn shape_as_any(&self) -> &dyn Any;
    /// Whether `other` is a shape of the same type, equal to this.
    fn shape_eq(&self, other: &dyn Any) -> bool;
}

impl<T: PartialEq + Any> ShapeEq for T {
    fn shape_as_any(&self) -> &dyn Any {
        self
    }
    fn shape_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
}

/// What backgrounds, borders, clips and shadows take the form of (Compose's Shape): in a size, a layout direction
/// and a density it makes an [`Outline`], from the top left of what it is for. Equal by what it is (its type's
/// `PartialEq`); interpolated by [`lerp_shape`] where its type says how (Compose's Interpolatable).
pub trait Shape: ShapeEq + fmt::Debug + Send + Sync {
    /// Its outline in `size` (px), mirrored right to left where it has a start and an end (Compose's
    /// createOutline).
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline;

    /// This `fraction` of the way to `other` (none: to nothing, as a shape takes it), or `None` where it can't be
    /// (Compose's Interpolatable.lerp; by default it can't).
    fn lerp(&self, other: Option<&dyn Shape>, fraction: f32) -> Option<Arc<dyn Shape>> {
        let _ = (other, fraction);
        None
    }
}

impl PartialEq for dyn Shape {
    fn eq(&self, other: &dyn Shape) -> bool {
        self.shape_eq(other.shape_as_any())
    }
}

/// Between two shapes (Compose's Interpolatable.lerp): equal ones switch half way; else `start`'s lerp to `stop`,
/// or `stop`'s back to `start`, or (neither can) a switch half way.
pub fn lerp_shape(start: &Arc<dyn Shape>, stop: &Arc<dyn Shape>, fraction: f32) -> Arc<dyn Shape> {
    let (a, b): (&dyn Shape, &dyn Shape) = (&**start, &**stop);
    if a == b {
        return if fraction < 0.5 { start.clone() } else { stop.clone() };
    }
    a.lerp(Some(b), fraction)
        .or_else(|| b.lerp(Some(a), 1.0 - fraction))
        .unwrap_or_else(|| if fraction < 0.5 { start.clone() } else { stop.clone() })
}

// ---- Rectangle -------------------------------------------------------------------------------------------------

/// The type of [`RectangleShape`](const@RectangleShape).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RectangleShape {}

/// A rectangle (Compose's RectangleShape).
pub const RectangleShape: RectangleShape = RectangleShape {};

impl fmt::Debug for RectangleShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("RectangleShape")
    }
}

impl Shape for RectangleShape {
    fn create_outline(&self, size: Size, _: LayoutDirection, _: &Density) -> Outline {
        Outline::Rectangle(size.to_rect())
    }
}

// ---- Corner sizes ----------------------------------------------------------------------------------------------

#[derive(Clone)]
enum CornerKind {
    Zero,
    Px(f32),
    Dp(Dp),
    Percent(f32),
    Lerp(Arc<(CornerSize, CornerSize, f32)>),
}

/// A corner's size (Compose's CornerSize): dp, px, a percentage of the shape's shorter side, none
/// ([`ZeroCornerSize`]), or one between two others ([`lerp_corner_size`]). `CornerSize(16.dp())`,
/// `CornerSize(4.0)` (px), `CornerSize(50)` (percent).
#[derive(Clone)]
pub struct CornerSize {
    kind: CornerKind,
}

/// A corner of no size (Compose's ZeroCornerSize).
pub const ZeroCornerSize: CornerSize = CornerSize::Zero;

/// `CornerSize(16.dp())`, `CornerSize(4.0)` (px), `CornerSize(50)` (percent): Compose's three constructors.
pub fn CornerSize(size: impl Into<CornerSize>) -> CornerSize {
    size.into()
}

impl CornerSize {
    /// A corner of no size (Compose's ZeroCornerSize).
    pub const Zero: CornerSize = CornerSize { kind: CornerKind::Zero };

    pub const fn dp(size: Dp) -> CornerSize {
        CornerSize { kind: CornerKind::Dp(size) }
    }
    pub const fn px(size: f32) -> CornerSize {
        CornerSize { kind: CornerKind::Px(size) }
    }
    /// `percent` of the shape's shorter side. Panics outside 0..=100, as Compose throws.
    pub const fn percent(percent: i32) -> CornerSize {
        if percent < 0 || percent > 100 {
            panic!("The percent should be in the range of [0, 100]");
        }
        CornerSize { kind: CornerKind::Percent(percent as f32) }
    }

    /// Its size in px for a shape of `shape_size` (Compose's toPx).
    pub fn to_px(&self, shape_size: Size, density: &Density) -> f32 {
        match &self.kind {
            CornerKind::Zero => 0.0,
            CornerKind::Px(size) => *size,
            CornerKind::Dp(size) => density.to_px(*size),
            CornerKind::Percent(percent) => shape_size.min_dimension() * (percent / 100.0),
            CornerKind::Lerp(l) => {
                let (a, b, t) = &**l;
                lerp(a.to_px(shape_size, density), b.to_px(shape_size, density), *t)
            }
        }
    }
}

/// By what it is (Compose's data classes); one between two others only to itself and its copies (Compose's
/// object).
impl PartialEq for CornerSize {
    fn eq(&self, o: &CornerSize) -> bool {
        match (&self.kind, &o.kind) {
            (CornerKind::Zero, CornerKind::Zero) => true,
            (CornerKind::Px(a), CornerKind::Px(b)) => a == b,
            (CornerKind::Dp(a), CornerKind::Dp(b)) => a == b,
            (CornerKind::Percent(a), CornerKind::Percent(b)) => a == b,
            (CornerKind::Lerp(a), CornerKind::Lerp(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// Compose's toString.
impl fmt::Debug for CornerSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            CornerKind::Zero => f.write_str("ZeroCornerSize"),
            CornerKind::Px(size) => write!(f, "CornerSize(size = {size:?}.px)"),
            CornerKind::Dp(size) => write!(f, "CornerSize(size = {:?}.dp)", size.0),
            CornerKind::Percent(percent) => write!(f, "CornerSize(size = {percent:?}%)"),
            CornerKind::Lerp(l) => write!(f, "CornerSize(lerp({:?}, {:?}, {:?}))", l.0, l.1, l.2),
        }
    }
}

impl From<Dp> for CornerSize {
    fn from(size: Dp) -> CornerSize {
        CornerSize::dp(size)
    }
}
/// Px.
impl From<f32> for CornerSize {
    fn from(size: f32) -> CornerSize {
        CornerSize::px(size)
    }
}
/// Px (as f32).
impl From<f64> for CornerSize {
    fn from(size: f64) -> CornerSize {
        CornerSize::px(size as f32)
    }
}
/// A percentage of the shorter side (panics outside 0..=100).
impl From<i32> for CornerSize {
    fn from(percent: i32) -> CornerSize {
        CornerSize::percent(percent)
    }
}

/// A corner `fraction` of the way from `start` to `stop`: their sizes in px mixed when it is resolved (Compose's
/// lerp of CornerSize).
pub fn lerp_corner_size(start: &CornerSize, stop: &CornerSize, fraction: f32) -> CornerSize {
    CornerSize { kind: CornerKind::Lerp(Arc::new((start.clone(), stop.clone(), fraction))) }
}

// ---- Corner-based shapes ---------------------------------------------------------------------------------------

/// Four corners in px for `size`: a side's two together within the shorter side (both shrunk if they wouldn't
/// fit), as Compose's CornerBasedShape resolves them. Panics, as Compose throws, for a negative one.
fn resolve_corners(corners: [&CornerSize; 4], size: Size, density: &Density) -> [f32; 4] {
    let [mut top_start, mut top_end, mut bottom_end, mut bottom_start] = corners.map(|c| c.to_px(size, density));
    let min_dimension = size.min_dimension();
    if top_start + bottom_start > min_dimension {
        let scale = min_dimension / (top_start + bottom_start);
        top_start *= scale;
        bottom_start *= scale;
    }
    if top_end + bottom_end > min_dimension {
        let scale = min_dimension / (top_end + bottom_end);
        top_end *= scale;
        bottom_end *= scale;
    }
    if !(top_start >= 0.0 && top_end >= 0.0 && bottom_end >= 0.0 && bottom_start >= 0.0) {
        panic!(
            "Corner size in Px can't be negative(topStart = {top_start}, topEnd = {top_end}, bottomEnd = \
             {bottom_end}, bottomStart = {bottom_start})!"
        );
    }
    [top_start, top_end, bottom_end, bottom_start]
}

/// A rounded rect of these corners (left, right as drawn), or a rect where they are all none.
fn rounded_outline(size: Size, [tl, tr, br, bl]: [f32; 4]) -> Outline {
    if tl + tr + br + bl == 0.0 {
        return Outline::Rectangle(size.to_rect());
    }
    Outline::Rounded(RoundRect::from_rect(
        size.to_rect(),
        CornerRadius::circular(tl),
        CornerRadius::circular(tr),
        CornerRadius::circular(br),
        CornerRadius::circular(bl),
    ))
}

/// The rect with these corners (left, right as drawn) cut straight, as Compose's CutCornerShape draws it, or a
/// rect where they are all none.
fn cut_outline(size: Size, [tl, tr, br, bl]: [f32; 4]) -> Outline {
    if tl + tr + bl + br == 0.0 {
        return Outline::Rectangle(size.to_rect());
    }
    let (w, h) = (size.width, size.height);
    let mut path = Path::new();
    path.move_to(0.0, tl);
    path.line_to(tl, 0.0);
    path.line_to(w - tr, 0.0);
    path.line_to(w, tr);
    path.line_to(w, h - br);
    path.line_to(w - br, h);
    path.line_to(bl, h);
    path.line_to(0.0, h - bl);
    path.close();
    Outline::Generic(path)
}

/// Start and end corners as drawn: right to left, the start ones on the right.
fn mirrored([ts, te, be, bs]: [f32; 4], layout_direction: LayoutDirection) -> [f32; 4] {
    match layout_direction {
        LayoutDirection::Ltr => [ts, te, be, bs],
        LayoutDirection::Rtl => [te, ts, bs, be],
    }
}

macro_rules! corner_based_shape {
    ($(#[$doc:meta])* $name:ident, [$ts:literal, $te:literal, $be:literal, $bs:literal],
     $variant:ident, |$size:ident, $corners:ident, $direction:ident| $outline:expr) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq)]
        pub struct $name {
            pub top_start: CornerSize,
            pub top_end: CornerSize,
            pub bottom_end: CornerSize,
            pub bottom_start: CornerSize,
        }

        #[doc = concat!("A [`", stringify!($name), "`](struct@", stringify!($name),
            ") of the same size at all four corners: `", stringify!($name),
            "(16.dp())`, `", stringify!($name), "(4.0)` (px), `", stringify!($name),
            "(50)` (percent) or a [`CornerSize`](struct@CornerSize) (Compose's one-size constructors).")]
        pub fn $name(corner: impl Into<CornerSize>) -> $name {
            let c = corner.into();
            $name { top_start: c.clone(), top_end: c.clone(), bottom_end: c.clone(), bottom_start: c }
        }

        impl $name {
            #[doc = concat!("Each corner its own (", $ts, ", ", $te, ", ", $be, ", ", $bs,
                "): dp, px, a percentage or a [`CornerSize`](struct@CornerSize) each (Compose's four-corner \
                constructors).")]
            pub fn new(
                top_start: impl Into<CornerSize>,
                top_end: impl Into<CornerSize>,
                bottom_end: impl Into<CornerSize>,
                bottom_start: impl Into<CornerSize>,
            ) -> $name {
                $name {
                    top_start: top_start.into(),
                    top_end: top_end.into(),
                    bottom_end: bottom_end.into(),
                    bottom_start: bottom_start.into(),
                }
            }

            /// The same size at all four corners (Compose's copy(all)).
            pub fn copy_all(&self, all: CornerSize) -> $name {
                $name { top_start: all.clone(), top_end: all.clone(), bottom_end: all.clone(), bottom_start: all }
            }

            fn corners(&self) -> [&CornerSize; 4] {
                [&self.top_start, &self.top_end, &self.bottom_end, &self.bottom_start]
            }
        }

        #[named]
        impl $name {
            /// Other corners, the rest its own (Compose's copy).
            pub fn copy(
                &self,
                #[default(self.top_start.clone())] top_start: CornerSize,
                #[default(self.top_end.clone())] top_end: CornerSize,
                #[default(self.bottom_end.clone())] bottom_end: CornerSize,
                #[default(self.bottom_start.clone())] bottom_start: CornerSize,
            ) -> $name {
                $name { top_start, top_end, bottom_end, bottom_start }
            }
        }

        /// Compose's toString.
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    f,
                    concat!(stringify!($name), "(", $ts, " = {:?}, ", $te, " = {:?}, ", $be, " = {:?}, ", $bs,
                        " = {:?})"),
                    self.top_start, self.top_end, self.bottom_end, self.bottom_start
                )
            }
        }

        impl Shape for $name {
            fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline {
                let $size = size;
                let $corners = resolve_corners(self.corners(), size, density);
                let $direction = layout_direction;
                $outline
            }

            /// To one of its type, corner by corner; to a rectangle or nothing as to itself with no corners
            /// (Compose's).
            fn lerp(&self, other: Option<&dyn Shape>, fraction: f32) -> Option<Arc<dyn Shape>> {
                let zero = $name(CornerSize::px(0.0));
                let other = match other {
                    None => &zero,
                    Some(o) if o.shape_as_any().is::<RectangleShape>() => &zero,
                    Some(o) => match o.shape_as_any().downcast_ref::<$name>() {
                        Some(same) => same,
                        None => match o.shape_as_any().downcast_ref::<CornerBasedShape>() {
                            Some(CornerBasedShape::$variant(same)) => same,
                            _ => return None,
                        },
                    },
                };
                let l = |a: &CornerSize, b: &CornerSize| lerp_corner_size(a, b, fraction);
                Some(Arc::new($name {
                    top_start: l(&self.top_start, &other.top_start),
                    top_end: l(&self.top_end, &other.top_end),
                    bottom_end: l(&self.bottom_end, &other.bottom_end),
                    bottom_start: l(&self.bottom_start, &other.bottom_start),
                }))
            }
        }

        impl From<$name> for CornerBasedShape {
            fn from(shape: $name) -> CornerBasedShape {
                CornerBasedShape::$variant(shape)
            }
        }

        /// A corner-based shape is equal to the shape of its kind it holds (Kotlin's subclass equality: a theme's
        /// `shapes.medium == RoundedCornerShape(12.dp)`).
        impl PartialEq<$name> for CornerBasedShape {
            fn eq(&self, other: &$name) -> bool {
                matches!(self, CornerBasedShape::$variant(s) if s == other)
            }
        }

        impl PartialEq<CornerBasedShape> for $name {
            fn eq(&self, other: &CornerBasedShape) -> bool {
                *other == *self
            }
        }
    };
}

corner_based_shape!(
    /// A rect with rounded corners (Compose's RoundedCornerShape): each corner's radius its
    /// [`CornerSize`](struct@CornerSize), a side's two within the shorter side; mirrored right to left (the start on
    /// the right).
    RoundedCornerShape,
    ["topStart", "topEnd", "bottomEnd", "bottomStart"],
    Rounded,
    |size, corners, direction| rounded_outline(size, mirrored(corners, direction))
);

corner_based_shape!(
    /// A rect with its corners cut straight across, as far along each side as its [`CornerSize`](struct@CornerSize)
    /// (Compose's CutCornerShape); mirrored right to left (the start on the right).
    CutCornerShape,
    ["topStart", "topEnd", "bottomEnd", "bottomStart"],
    Cut,
    |size, corners, direction| cut_outline(size, mirrored(corners, direction))
);

corner_based_shape!(
    /// A rect with rounded corners named left and right on the screen, the same right to left (Compose's
    /// AbsoluteRoundedCornerShape): `top_start` is its top left, `top_end` its top right, `bottom_end` its bottom
    /// right, `bottom_start` its bottom left (as Compose's CornerBasedShape names them).
    AbsoluteRoundedCornerShape,
    ["topLeft", "topRight", "bottomRight", "bottomLeft"],
    AbsoluteRounded,
    |size, corners, direction| {
        let _ = direction;
        rounded_outline(size, corners)
    }
);

corner_based_shape!(
    /// A rect with corners cut straight across, named left and right on the screen, the same right to left
    /// (Compose's AbsoluteCutCornerShape); its corners named as
    /// [`AbsoluteRoundedCornerShape`](struct@AbsoluteRoundedCornerShape)'s.
    AbsoluteCutCornerShape,
    ["topLeft", "topRight", "bottomRight", "bottomLeft"],
    AbsoluteCut,
    |size, corners, direction| {
        let _ = direction;
        cut_outline(size, corners)
    }
);

/// A circle, or the pill or round-ended shape of a rect that isn't square (Compose's CircleShape: corners of 50
/// percent).
pub const CircleShape: RoundedCornerShape = RoundedCornerShape {
    top_start: CornerSize { kind: CornerKind::Percent(50.0) },
    top_end: CornerSize { kind: CornerKind::Percent(50.0) },
    bottom_end: CornerSize { kind: CornerKind::Percent(50.0) },
    bottom_start: CornerSize { kind: CornerKind::Percent(50.0) },
};

/// Any of the shapes made of four [`CornerSize`](struct@CornerSize)s (Compose's CornerBasedShape, as a theme's
/// shapes hold them).
#[derive(Clone, PartialEq, Debug)]
pub enum CornerBasedShape {
    Rounded(RoundedCornerShape),
    Cut(CutCornerShape),
    AbsoluteRounded(AbsoluteRoundedCornerShape),
    AbsoluteCut(AbsoluteCutCornerShape),
}

macro_rules! each_corner_shape {
    ($self:expr, $s:ident => $e:expr) => {
        match $self {
            CornerBasedShape::Rounded($s) => $e,
            CornerBasedShape::Cut($s) => $e,
            CornerBasedShape::AbsoluteRounded($s) => $e,
            CornerBasedShape::AbsoluteCut($s) => $e,
        }
    };
}

impl CornerBasedShape {
    pub fn top_start(&self) -> &CornerSize {
        each_corner_shape!(self, s => &s.top_start)
    }
    pub fn top_end(&self) -> &CornerSize {
        each_corner_shape!(self, s => &s.top_end)
    }
    pub fn bottom_end(&self) -> &CornerSize {
        each_corner_shape!(self, s => &s.bottom_end)
    }
    pub fn bottom_start(&self) -> &CornerSize {
        each_corner_shape!(self, s => &s.bottom_start)
    }
    /// The same size at all four corners (Compose's copy(all)).
    pub fn copy_all(&self, all: CornerSize) -> CornerBasedShape {
        each_corner_shape!(self, s => s.copy_all(all).into())
    }
}

#[named]
impl CornerBasedShape {
    /// Other corners, the rest its own, of the same kind (Compose's copy).
    pub fn copy(
        &self,
        #[default(self.top_start().clone())] top_start: CornerSize,
        #[default(self.top_end().clone())] top_end: CornerSize,
        #[default(self.bottom_end().clone())] bottom_end: CornerSize,
        #[default(self.bottom_start().clone())] bottom_start: CornerSize,
    ) -> CornerBasedShape {
        let corners = (top_start, top_end, bottom_end, bottom_start);
        match self {
            CornerBasedShape::Rounded(_) => RoundedCornerShape::new(corners.0, corners.1, corners.2, corners.3).into(),
            CornerBasedShape::Cut(_) => CutCornerShape::new(corners.0, corners.1, corners.2, corners.3).into(),
            CornerBasedShape::AbsoluteRounded(_) => {
                AbsoluteRoundedCornerShape::new(corners.0, corners.1, corners.2, corners.3).into()
            }
            CornerBasedShape::AbsoluteCut(_) => {
                AbsoluteCutCornerShape::new(corners.0, corners.1, corners.2, corners.3).into()
            }
        }
    }
}

impl Shape for CornerBasedShape {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline {
        each_corner_shape!(self, s => s.create_outline(size, layout_direction, density))
    }
    fn lerp(&self, other: Option<&dyn Shape>, fraction: f32) -> Option<Arc<dyn Shape>> {
        each_corner_shape!(self, s => s.lerp(other, fraction))
    }
}

// ---- Shapes of one's own ---------------------------------------------------------------------------------------

type BuildPath = dyn Fn(&mut Path, Size, LayoutDirection) + Send + Sync;

/// A shape made by a path built in its size and layout direction, then closed (Compose's GenericShape):
/// `GenericShape(|path, size, _| { path.move_to(size.width / 2.0, 0.0); … })`. Equal only to itself and its
/// copies.
#[derive(Clone)]
pub struct GenericShape {
    builder: Arc<BuildPath>,
}

pub fn GenericShape(builder: impl Fn(&mut Path, Size, LayoutDirection) + Send + Sync + 'static) -> GenericShape {
    GenericShape { builder: Arc::new(builder) }
}

impl PartialEq for GenericShape {
    fn eq(&self, o: &GenericShape) -> bool {
        Arc::ptr_eq(&self.builder, &o.builder)
    }
}

impl fmt::Debug for GenericShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "GenericShape({:p})", Arc::as_ptr(&self.builder))
    }
}

impl Shape for GenericShape {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, _: &Density) -> Outline {
        let mut path = Path::new();
        (self.builder)(&mut path, size, layout_direction);
        path.close();
        Outline::Generic(path)
    }
}

type MakeOutline = dyn Fn(Size, LayoutDirection, &Density) -> Outline + Send + Sync;

/// A shape whose outline `create_outline` makes (what Compose writes as an `object : Shape`). Equal only to itself
/// and its copies.
#[derive(Clone)]
pub struct OutlineShape {
    create: Arc<MakeOutline>,
}

pub fn OutlineShape(
    create_outline: impl Fn(Size, LayoutDirection, &Density) -> Outline + Send + Sync + 'static,
) -> OutlineShape {
    OutlineShape { create: Arc::new(create_outline) }
}

impl PartialEq for OutlineShape {
    fn eq(&self, o: &OutlineShape) -> bool {
        Arc::ptr_eq(&self.create, &o.create)
    }
}

impl fmt::Debug for OutlineShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OutlineShape({:p})", Arc::as_ptr(&self.create))
    }
}

impl Shape for OutlineShape {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline {
        (self.create)(size, layout_direction, density)
    }
}
