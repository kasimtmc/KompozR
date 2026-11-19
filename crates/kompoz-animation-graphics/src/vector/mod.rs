//! Compose's `androidx.compose.animation.graphics.vector`: [`AnimatedImageVector`], its animators and the specs
//! they play with; `compat`: Android's XML read (animated vectors, animators, interpolators) and its resources.

pub(crate) mod animated_image_vector;
pub(crate) mod animator;
pub(crate) mod compat;
pub(crate) mod specs;

pub use animated_image_vector::AnimatedImageVector;
pub use compat::resources::{NoXmlResources, ResourceReference, XmlResourceError, XmlResources};
