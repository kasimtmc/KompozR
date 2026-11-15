//! How a DrawScope paints a shape (Compose's DrawStyle: `Fill`, `Stroke`, kompoz-graphics' as Compose's are
//! ui-graphics') and what its drawPoints draws (Compose's PointMode).

use kompoz_graphics::BlendMode;
pub use kompoz_graphics::{DrawStyle, Fill, Stroke};

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
