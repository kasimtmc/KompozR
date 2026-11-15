//! KompozR's GPU renderer (C++ Kompoz's platform/gpu): the shaders every backend draws with ([`shaders`]: compiled
//! from Slang by `cargo xtask shaders`, and what they read laid out as they read it) and a path's geometry in the
//! target's pixels ([`path`]: curves cut into lines, strokes into outlines, projected, as edges for the coverage
//! mask).

pub mod path;
pub mod shaders;
