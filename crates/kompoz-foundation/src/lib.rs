//! KompozR's foundation, as Jetpack Compose's `androidx.compose.foundation` and `foundation.layout`: the layouts
//! (`Box`, `Row`, `Column`, `Spacer`), arrangements, the layout modifiers (padding, size, offset, aspect ratio,
//! intrinsic sizes, alignment line padding), window insets and the gesture detectors. Its modifiers extend `Modifier`
//! through traits: `use kompoz_foundation::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

pub mod gestures;
pub mod layout;

/// What an app using foundation imports: the layouts, the modifier extension traits and the gesture detectors.
pub mod prelude {
    pub use crate::gestures::*;
    pub use crate::layout::*;
}
