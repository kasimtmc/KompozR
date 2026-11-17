//! KompozR's ripple, as Compose's `androidx.compose.material.ripple` (material-ripple 1.12, the common path that
//! the desktop and the web take; Android's RippleDrawable is the Android platform's): the ripple node a design
//! system makes its ripple indication of ([`create_ripple_modifier_node`]) — a ripple spreading from each press (225
//! ms, FastOutSlowIn), fading in (75 ms) and, once let go, out (150 ms) — and its state layer for hovers, focuses and
//! drags; the alphas a design system gives them ([`RippleAlpha`]).
//!
//! The library's deprecated-as-errors API (`rememberRipple`, `RippleTheme`, `LocalRippleTheme`: Kotlin can't call
//! them) isn't here; material's and material3's `ripple()` and `LocalRippleConfiguration` come with those libraries.
//! A ripple indication of one's own:
//!
//! ```no_run
//! # use kompoz_foundation::prelude::*;
//! # use kompoz_graphics::{Color, ColorProducer, Dp};
//! # use kompoz_material_ripple::*;
//! # use kompoz_ui::ModifierNode;
//! #[derive(PartialEq)]
//! struct MyRipple;
//!
//! impl Indication for MyRipple {
//!     fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
//!         Some(self)
//!     }
//! }
//!
//! impl IndicationNodeFactory for MyRipple {
//!     fn create(&self, interaction_source: InteractionSource) -> Box<dyn ModifierNode> {
//!         let color = ColorProducer::new(|| Color::Black);
//!         create_ripple_modifier_node(interaction_source, true, Dp::Unspecified, color, || {
//!             RippleAlpha::new(0.16, 0.1, 0.08, 0.1)
//!         })
//!     }
//! }
//! ```

#![allow(non_upper_case_globals, non_snake_case)]

mod ripple;
mod ripple_animation;

pub use ripple::{RippleAlpha, create_ripple_modifier_node};

/// What an app using the ripple imports.
pub mod prelude {
    pub use crate::{RippleAlpha, create_ripple_modifier_node};
}
