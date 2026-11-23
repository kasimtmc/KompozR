//! Compose's `androidx.compose.animation.graphics.res`: rememberAnimatedVectorPainter, and the animator and
//! interpolator resources the XML parsers load (with Android's interpolators as easings, its PathInterpolator).

pub(crate) mod animated_vector_painter;
pub(crate) mod animated_vector_resources;
pub(crate) mod animator_resources;
pub(crate) mod path_interpolator;

pub use animated_vector_painter::remember_animated_vector_painter;
pub use animated_vector_resources::{animated_vector_resource, load_animated_vector_resource};
