//! KompozR's foundation, as Jetpack Compose's `androidx.compose.foundation` and `foundation.layout`: the layouts
//! (`Box`, `Row`, `Column`, `Spacer`), arrangements, and the padding and size modifiers. Its modifiers extend
//! `Modifier` through traits: `use kompoz_foundation::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

pub mod layout;

/// What an app using foundation imports: the layouts and the modifier extension traits.
pub mod prelude {
    pub use crate::layout::*;
}
