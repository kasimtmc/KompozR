//! A shadow (Compose's `androidx.compose.ui.graphics.Shadow`): what text casts, and what dropShadow and
//! innerShadow draw.

use crate::brush::{BlendMode, Brush};
use crate::color::{Color, lerp_color};
use crate::geometry::{Offset, lerp, lerp_offset};

/// A shadow (Compose's Shadow, for text and, with spread, for dropShadow/innerShadow): its color, how far it is
/// moved and how much it is blurred (px), and how much bigger (a drop shadow) or smaller (an inner one's hole) the
/// shape is.
#[derive(Clone, PartialEq, Debug)]
pub struct Shadow {
    pub color: Color,
    pub offset: Offset,
    pub blur_radius: f32,
    /// Not for text.
    pub spread: f32,
    /// Not for text either (Compose 1.9's dropShadow and innerShadow): a brush painting it instead of the color
    /// (spread over what it covers).
    pub brush: Option<Brush>,
    /// Its alpha.
    pub alpha: f32,
    /// How it goes onto what is under it.
    pub blend_mode: BlendMode,
}

impl Shadow {
    /// Black, where it is, sharp (Compose's Shadow.None: no shadow drawn, as nothing is moved or blurred).
    pub const None: Shadow = Shadow {
        color: DefaultShadowColor,
        offset: Offset::Zero,
        blur_radius: 0.0,
        spread: 0.0,
        brush: None,
        alpha: 1.0,
        blend_mode: BlendMode::SrcOver,
    };

    /// One of `color`, moved, blurred and spread, painted by no brush, opaque, over what is under it.
    pub fn new(color: Color, offset: Offset, blur_radius: f32, spread: f32) -> Shadow {
        Shadow { color, offset, blur_radius, spread, ..Shadow::default() }
    }
}

/// Black, where it is, sharp.
impl Default for Shadow {
    fn default() -> Shadow {
        Shadow::None
    }
}

/// Compose's default shadow color.
pub const DefaultShadowColor: Color = Color::Black;

/// A text's shadow: of `color`, moved by `offset`, blurred by `blur_radius` (px).
///
/// Kotlin: `Shadow(color: Color = Color(0xFF000000), offset: Offset = Offset.Zero, blurRadius: Float = 0.0f)`
#[kompoz_runtime::named(type)]
pub fn Shadow(
    #[default(DefaultShadowColor)] color: Color,
    #[default(Offset::Zero)] offset: Offset,
    #[default(0.0)] blur_radius: f32,
) -> Shadow {
    Shadow::new(color, offset, blur_radius, 0.0)
}

/// Between two shadows (Compose's lerp(Shadow, Shadow, fraction)): the color, offset, blur, spread and alpha
/// along, the brush and blend mode the nearer one's.
pub fn lerp_shadow(start: &Shadow, stop: &Shadow, fraction: f32) -> Shadow {
    let near = if fraction < 0.5 { start } else { stop };
    Shadow {
        color: lerp_color(start.color, stop.color, fraction),
        offset: lerp_offset(start.offset, stop.offset, fraction),
        blur_radius: lerp(start.blur_radius, stop.blur_radius, fraction),
        spread: lerp(start.spread, stop.spread, fraction),
        brush: near.brush.clone(),
        alpha: lerp(start.alpha, stop.alpha, fraction),
        blend_mode: near.blend_mode,
    }
}
