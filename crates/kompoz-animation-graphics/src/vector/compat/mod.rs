//! Android's XML resources read as Compose's androidMain `vector.compat` reads them: the pull parser over a
//! compiled-like document, its resources and TypedArray, and the animated-vector and animator parsers.

pub(crate) mod animated_vector_parser;
pub(crate) mod animator_parser;
pub(crate) mod resources;
pub(crate) mod xml;
