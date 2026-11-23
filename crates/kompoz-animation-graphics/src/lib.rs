//! KompozR's animated vector graphics, as Compose's `androidx.compose.animation.graphics`: an
//! [`AnimatedImageVector`] (an Android `<animated-vector>`: a vector and its targets' animators — object animators,
//! sets of them together or one after the other, property values holders of floats, colors and path data,
//! keyframes, Android's interpolators) drawn by [`remember_animated_vector_painter`] at its start or its end, a
//! change animated by a `Transition` of `at_end` (each property's animators as Compose plays them: keyframes specs
//! combined by start time, reversed going back; path data morphed node by node).
//!
//! ```no_run
//! # use kompoz_animation_graphics::*;
//! # use kompoz_runtime::*;
//! # const XML: &str = "";
//! #[kompozable]
//! fn Hourglass(at_end: bool) -> kompoz_ui::draw::VectorPainter {
//!     let image = remember(|| AnimatedImageVector::from_animated_vector_xml(XML).expect("an animated vector"));
//!     remember_animated_vector_painter(image, at_end)
//! }
//! ```
//!
//! Without Android's resource system, an animated vector is read from its XML text
//! ([`AnimatedImageVector::from_animated_vector_xml`]) as Android's resource compiler and Compose's parsers read
//! the file: inline `<aapt:attr>` resources, the framework's interpolators (`@android:interpolator/…`,
//! `@android:anim/…`), attribute values compiled for their attribute's format; what it refers to otherwise
//! (`@drawable/…`, `@animator/…`, `@interpolator/…`, `@color/…`, values, theme attributes) the caller's
//! [`XmlResources`] give ([`AnimatedImageVector::from_animated_vector_xml_with`]). Compose's
//! `animatedVectorResource` reads one of KompozR's resources ([`animated_vector_resource`]: a `DrawableResource`
//! of a generated `Res`, its `@drawable/…` references that `Res`'s).

#![allow(non_upper_case_globals, non_snake_case)]

pub mod res;
pub mod vector;

#[cfg(test)]
mod tests;

pub use res::{animated_vector_resource, load_animated_vector_resource, remember_animated_vector_painter};
pub use vector::{AnimatedImageVector, NoXmlResources, ResourceReference, XmlResourceError, XmlResources};
