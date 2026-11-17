//! Colors animated in Oklab (Compose animation's ColorVectorConverter.kt and its `Animatable(Color)`).

use crate::animatable::{Animatable, AnimatableArgs};
use crate::converter::TwoWayConverter;
use crate::easing::coerce_in;
use crate::vector::AnimationVector4D;
use kompoz_graphics::Color;
use kompoz_graphics::colorspace::{ColorSpace, ColorSpaces};

/// The converter of colors (Compose's `Color.VectorConverter`: `Color::VectorConverter(color_space)` here, with
/// this trait in scope): a color as (alpha, L, a, b) in Oklab, back into `color_space`, each kept within Oklab's
/// range (L and alpha within 0..1, a and b within -0.5..0.5) so a spring's overshoot stays a color.
pub trait ColorVectorConverter {
    /// Kotlin: `Color.VectorConverter(colorSpace: ColorSpace)`
    fn VectorConverter(color_space: &'static dyn ColorSpace) -> TwoWayConverter<Color, AnimationVector4D>;
}

impl ColorVectorConverter for Color {
    fn VectorConverter(color_space: &'static dyn ColorSpace) -> TwoWayConverter<Color, AnimationVector4D> {
        TwoWayConverter(
            |color: Color| {
                let c = color.convert(&ColorSpaces::Oklab);
                AnimationVector4D(c.alpha(), c.red(), c.green(), c.blue())
            },
            move |vector: AnimationVector4D| {
                Color::new_in(
                    coerce_in(vector.v2, 0.0, 1.0),
                    coerce_in(vector.v3, -0.5, 0.5),
                    coerce_in(vector.v4, -0.5, 0.5),
                    coerce_in(vector.v1, 0.0, 1.0),
                    &ColorSpaces::Oklab,
                )
                .convert(color_space)
            },
        )
    }
}

/// Kotlin: `Animatable(initialValue: Color)`
impl AnimatableArgs for Color {
    type Animatable = Animatable<Color, AnimationVector4D>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self, Color::VectorConverter(self.color_space()), None, "Animatable")
    }
}
