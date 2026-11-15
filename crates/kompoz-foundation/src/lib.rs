//! KompozR's foundation, as Jetpack Compose's `androidx.compose.foundation` and `foundation.layout`: the layouts
//! (`Box`, `Row`, `Column`, `Spacer`), arrangements, the layout modifiers (padding, size, offset, aspect ratio,
//! intrinsic sizes, alignment line padding), window insets, the gesture detectors, `background` and `border`, `Canvas`
//! and `Image`. Its modifiers extend `Modifier` through traits: `use kompoz_foundation::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

mod background;
mod border;
mod canvas;
pub mod gestures;
mod image;
pub mod layout;

pub use background::BackgroundModifiers;
pub use border::{BorderModifierNode, BorderModifiers};
pub use canvas::Canvas;
pub use image::{BitmapImage, Image, VectorImage};

/// What an app using foundation imports: the layouts, the composables, the modifier extension traits and the gesture
/// detectors.
pub mod prelude {
    pub use crate::gestures::*;
    pub use crate::layout::*;
    pub use crate::{BackgroundModifiers, BitmapImage, BorderModifiers, Canvas, Image, VectorImage};
}
