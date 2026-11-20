//! The shapes, as Compose's Shapes.kt: a theme's corner shapes by size ([`Shapes`], Material's
//! [`ShapeDefaults`]), the shapes the tokens name ([`ShapeKeyTokens`]' value), and a shape's halves (its top, bottom,
//! start or end corners only, the rest given).

use crate::material_theme::MaterialTheme;
use crate::tokens::{ShapeKeyTokens, ShapeTokens};
use kompoz_graphics::{CircleShape, CornerBasedShape, CornerSize, RectangleShape, Shape};
use kompoz_runtime::{KotlinProperties, named, named_args};
use std::fmt;
use std::sync::Arc;

/// A theme's shapes by size (Compose's Shapes): what Material's components are cut to (a chip `small`, a card
/// `medium`, a dialog `extra_large`), and Material Expressive's sizes between. Equal by its shapes.
#[derive(Clone, PartialEq, KotlinProperties)]
pub struct Shapes {
    pub extra_small: CornerBasedShape,
    pub small: CornerBasedShape,
    pub medium: CornerBasedShape,
    pub large: CornerBasedShape,
    pub extra_large: CornerBasedShape,
    pub large_increased: CornerBasedShape,
    pub extra_large_increased: CornerBasedShape,
    pub extra_extra_large: CornerBasedShape,
}

/// The shapes, each Material's unless given (Compose's `Shapes(…)`, its Expressive sizes too).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn Shapes(
    #[default(ShapeDefaults::ExtraSmall())] extra_small: CornerBasedShape,
    #[default(ShapeDefaults::Small())] small: CornerBasedShape,
    #[default(ShapeDefaults::Medium())] medium: CornerBasedShape,
    #[default(ShapeDefaults::Large())] large: CornerBasedShape,
    #[default(ShapeDefaults::ExtraLarge())] extra_large: CornerBasedShape,
    #[default(ShapeDefaults::LargeIncreased())] large_increased: CornerBasedShape,
    #[default(ShapeDefaults::ExtraLargeIncreased())] extra_large_increased: CornerBasedShape,
    #[default(ShapeDefaults::ExtraExtraLarge())] extra_extra_large: CornerBasedShape,
) -> Shapes {
    Shapes { extra_small, small, medium, large, extra_large, large_increased, extra_large_increased, extra_extra_large }
}

impl Default for Shapes {
    /// Material's.
    fn default() -> Shapes {
        Shapes()
    }
}

#[named]
impl Shapes {
    /// The same with what is given replaced (Compose's copy, its Expressive sizes too).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.extra_small.clone())] extra_small: CornerBasedShape,
        #[default(self.small.clone())] small: CornerBasedShape,
        #[default(self.medium.clone())] medium: CornerBasedShape,
        #[default(self.large.clone())] large: CornerBasedShape,
        #[default(self.extra_large.clone())] extra_large: CornerBasedShape,
        #[default(self.large_increased.clone())] large_increased: CornerBasedShape,
        #[default(self.extra_large_increased.clone())] extra_large_increased: CornerBasedShape,
        #[default(self.extra_extra_large.clone())] extra_extra_large: CornerBasedShape,
    ) -> Shapes {
        Shapes {
            extra_small,
            small,
            medium,
            large,
            extra_large,
            large_increased,
            extra_large_increased,
            extra_extra_large,
        }
    }
}

impl Shapes {
    /// A token's shape in these (Compose's internal `Shapes.fromToken`): a size's, a size's top or a side's half,
    /// a circle or a rectangle.
    #[allow(dead_code, clippy::wrong_self_convention)] // (Compose's internal `fromToken`; the components', as they come)
    pub(crate) fn from_token(&self, value: ShapeKeyTokens) -> Arc<dyn Shape> {
        match value {
            ShapeKeyTokens::CornerExtraLarge => Arc::new(self.extra_large.clone()),
            ShapeKeyTokens::CornerExtraLargeIncreased => Arc::new(self.extra_large_increased.clone()),
            ShapeKeyTokens::CornerExtraExtraLarge => Arc::new(self.extra_extra_large.clone()),
            ShapeKeyTokens::CornerExtraLargeTop => Arc::new(top(&self.extra_large, ShapeDefaults::CornerNone)),
            ShapeKeyTokens::CornerExtraSmall => Arc::new(self.extra_small.clone()),
            ShapeKeyTokens::CornerExtraSmallTop => Arc::new(top(&self.extra_small, ShapeDefaults::CornerNone)),
            ShapeKeyTokens::CornerFull => Arc::new(CircleShape),
            ShapeKeyTokens::CornerLarge => Arc::new(self.large.clone()),
            ShapeKeyTokens::CornerLargeIncreased => Arc::new(self.large_increased.clone()),
            ShapeKeyTokens::CornerLargeEnd => Arc::new(end(&self.large, ShapeDefaults::CornerNone)),
            ShapeKeyTokens::CornerLargeTop => Arc::new(top(&self.large, ShapeDefaults::CornerNone)),
            ShapeKeyTokens::CornerMedium => Arc::new(self.medium.clone()),
            ShapeKeyTokens::CornerNone => Arc::new(RectangleShape),
            ShapeKeyTokens::CornerSmall => Arc::new(self.small.clone()),
            ShapeKeyTokens::CornerLargeStart => Arc::new(start(&self.large, ShapeDefaults::CornerNone)),
        }
    }
}

impl ShapeKeyTokens {
    /// The token's shape in the theme's shapes (Compose's internal `ShapeKeyTokens.value`).
    #[allow(dead_code)] // (the components', used as they come)
    pub(crate) fn value(self) -> Arc<dyn Shape> {
        MaterialTheme::shapes().from_token(self)
    }
}

/// Compose's toString (its order: `largeIncreased` after `large`, `extralargeIncreased` so spelled).
impl fmt::Display for Shapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Shapes(extraSmall={:?}, small={:?}, medium={:?}, large={:?}, largeIncreased={:?}, extraLarge={:?}, \
             extralargeIncreased={:?}, extraExtraLarge={:?})",
            self.extra_small,
            self.small,
            self.medium,
            self.large,
            self.large_increased,
            self.extra_large,
            self.extra_large_increased,
            self.extra_extra_large
        )
    }
}

impl fmt::Debug for Shapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Material's shapes by size (Compose's ShapeDefaults): rounded corners of 4, 8, 12, 16, 20, 28, 32 and 48 dp.
pub struct ShapeDefaults;

#[allow(non_snake_case)]
#[allow(dead_code)] // (its crate's corner sizes: the components', used as they come)
impl ShapeDefaults {
    /// 4 dp corners.
    pub fn ExtraSmall() -> CornerBasedShape {
        ShapeTokens::CornerExtraSmall.into()
    }
    /// 8 dp corners.
    pub fn Small() -> CornerBasedShape {
        ShapeTokens::CornerSmall.into()
    }
    /// 12 dp corners.
    pub fn Medium() -> CornerBasedShape {
        ShapeTokens::CornerMedium.into()
    }
    /// 16 dp corners.
    pub fn Large() -> CornerBasedShape {
        ShapeTokens::CornerLarge.into()
    }
    /// 20 dp corners (Material Expressive's).
    pub fn LargeIncreased() -> CornerBasedShape {
        ShapeTokens::CornerLargeIncreased.into()
    }
    /// 28 dp corners.
    pub fn ExtraLarge() -> CornerBasedShape {
        ShapeTokens::CornerExtraLarge.into()
    }
    /// 32 dp corners (Material Expressive's).
    pub fn ExtraLargeIncreased() -> CornerBasedShape {
        ShapeTokens::CornerExtraLargeIncreased.into()
    }
    /// 48 dp corners (Material Expressive's).
    pub fn ExtraExtraLarge() -> CornerBasedShape {
        ShapeTokens::CornerExtraExtraLarge.into()
    }

    pub(crate) const CornerNone: CornerSize = ShapeTokens::CornerValueNone;
    pub(crate) const CornerExtraSmall: CornerSize = ShapeTokens::CornerValueExtraSmall;
    pub(crate) const CornerSmall: CornerSize = ShapeTokens::CornerValueSmall;
    pub(crate) const CornerMedium: CornerSize = ShapeTokens::CornerValueMedium;
    pub(crate) const CornerLarge: CornerSize = ShapeTokens::CornerValueLarge;
    pub(crate) const CornerLargeIncreased: CornerSize = ShapeTokens::CornerValueLargeIncreased;
    pub(crate) const CornerExtraLarge: CornerSize = ShapeTokens::CornerValueExtraLarge;
    pub(crate) const CornerExtraLargeIncreased: CornerSize = ShapeTokens::CornerValueExtraLargeIncreased;
    pub(crate) const CornerExtraExtraLarge: CornerSize = ShapeTokens::CornerValueExtraExtraLarge;
    pub(crate) const CornerFull: CornerSize = CornerSize::percent(100);
}

/// `shape`'s top corners, its bottom ones `bottom_size` (Compose's internal `CornerBasedShape.top`).
#[allow(dead_code)] // (the components', used as they come)
pub(crate) fn top(shape: &CornerBasedShape, bottom_size: CornerSize) -> CornerBasedShape {
    with_corners(shape, None, None, Some(bottom_size.clone()), Some(bottom_size))
}

/// `shape`'s bottom corners, its top ones `top_size` (Compose's internal `CornerBasedShape.bottom`).
#[allow(dead_code)] // (the components', used as they come)
pub(crate) fn bottom(shape: &CornerBasedShape, top_size: CornerSize) -> CornerBasedShape {
    with_corners(shape, Some(top_size.clone()), Some(top_size), None, None)
}

/// `shape`'s start corners, its end ones `end_size` (Compose's internal `CornerBasedShape.start`).
#[allow(dead_code)] // (the components', used as they come)
pub(crate) fn start(shape: &CornerBasedShape, end_size: CornerSize) -> CornerBasedShape {
    with_corners(shape, None, Some(end_size.clone()), Some(end_size), None)
}

/// `shape`'s end corners, its start ones `start_size` (Compose's internal `CornerBasedShape.end`).
#[allow(dead_code)] // (the components', used as they come)
pub(crate) fn end(shape: &CornerBasedShape, start_size: CornerSize) -> CornerBasedShape {
    with_corners(shape, Some(start_size.clone()), None, None, Some(start_size))
}

/// A copy of `shape` of the same kind, the corners given replaced (Compose's `CornerBasedShape.copy`).
#[allow(dead_code)] // (the components', used as they come)
fn with_corners(
    shape: &CornerBasedShape,
    top_start: Option<CornerSize>,
    top_end: Option<CornerSize>,
    bottom_end: Option<CornerSize>,
    bottom_start: Option<CornerSize>,
) -> CornerBasedShape {
    let top_start = top_start.unwrap_or_else(|| shape.top_start().clone());
    let top_end = top_end.unwrap_or_else(|| shape.top_end().clone());
    let bottom_end = bottom_end.unwrap_or_else(|| shape.bottom_end().clone());
    let bottom_start = bottom_start.unwrap_or_else(|| shape.bottom_start().clone());
    named_args!(shape.copy(
        top_start = top_start,
        top_end = top_end,
        bottom_end = bottom_end,
        bottom_start = bottom_start
    ))
}
