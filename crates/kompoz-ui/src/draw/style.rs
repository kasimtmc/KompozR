//! How a DrawScope paints a shape (Compose's DrawStyle: `Fill`, `Stroke`) and what its drawPoints draws (Compose's
//! PointMode).

use kompoz_graphics::{BlendMode, PathEffect, PathStroke, StrokeCap, StrokeJoin};
use kompoz_runtime::named;

/// A stroke (Compose's Stroke): `width` px wide (0: a hairline, a pixel whatever the transform), its ends `cap`, its
/// corners `join` (a miter beveled past `miter` times half the width), its path through `path_effect` first.
#[derive(Clone, PartialEq, Debug)]
pub struct Stroke {
    pub width: f32,
    pub miter: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub path_effect: Option<PathEffect>,
}

impl Stroke {
    /// Compose's Stroke.HairlineWidth.
    pub const HairlineWidth: f32 = 0.0;
    /// Compose's Stroke.DefaultMiter.
    pub const DefaultMiter: f32 = 4.0;
    /// Compose's Stroke.DefaultCap.
    pub const DefaultCap: StrokeCap = StrokeCap::Butt;
    /// Compose's Stroke.DefaultJoin.
    pub const DefaultJoin: StrokeJoin = StrokeJoin::Miter;

    /// Every parameter given (for plain code: [`Stroke`] takes them by name in a kompozable body).
    pub fn new(width: f32, miter: f32, cap: StrokeCap, join: StrokeJoin, path_effect: Option<PathEffect>) -> Stroke {
        Stroke { width, miter, cap, join, path_effect }
    }

    /// What a backend strokes with: a width of 0 a hairline.
    pub fn path_stroke(&self) -> PathStroke {
        PathStroke {
            width: self.width.max(0.0),
            cap: self.cap,
            join: self.join,
            miter: self.miter,
            hairline: self.width <= 0.0,
            path_effect: self.path_effect.clone(),
        }
    }
}

/// A hairline, its defaults Compose's.
impl Default for Stroke {
    fn default() -> Stroke {
        Stroke::new(Stroke::HairlineWidth, Stroke::DefaultMiter, Stroke::DefaultCap, Stroke::DefaultJoin, None)
    }
}

/// Compose's `Stroke(width = …, miter = …, cap = …, join = …, pathEffect = …)`, each defaulted: a hairline.
#[named]
pub fn Stroke(
    #[default(Stroke::HairlineWidth)] width: f32,
    #[default(Stroke::DefaultMiter)] miter: f32,
    #[default(Stroke::DefaultCap)] cap: StrokeCap,
    #[default(Stroke::DefaultJoin)] join: StrokeJoin,
    #[default(None)] path_effect: Option<PathEffect>,
) -> Stroke {
    Stroke::new(width, miter, cap, join, path_effect)
}

/// How a shape is painted (Compose's DrawStyle): filled, or stroked.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum DrawStyle {
    #[default]
    Fill,
    Stroke(Stroke),
}

/// Filled (Compose's Fill).
pub const Fill: DrawStyle = DrawStyle::Fill;

impl DrawStyle {
    /// Whether it strokes.
    pub fn is_stroke(&self) -> bool {
        matches!(self, DrawStyle::Stroke(_))
    }

    /// What a backend draws with: a fill, or the stroke's.
    pub fn path_stroke(&self) -> PathStroke {
        match self {
            DrawStyle::Fill => PathStroke::default(),
            DrawStyle::Stroke(s) => s.path_stroke(),
        }
    }
}

impl From<Stroke> for DrawStyle {
    fn from(stroke: Stroke) -> DrawStyle {
        DrawStyle::Stroke(stroke)
    }
}

/// What drawPoints draws (Compose's PointMode): each point a dot (round with a round cap, else square), every two a
/// line, or all of them one line.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointMode {
    #[default]
    Points,
    Lines,
    Polygon,
}

/// Compose's DrawScope.DefaultBlendMode: SrcOver.
pub const DefaultBlendMode: BlendMode = BlendMode::SrcOver;

/// Compose's DefaultAlpha.
pub const DefaultAlpha: f32 = 1.0;
