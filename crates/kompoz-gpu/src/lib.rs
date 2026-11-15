//! KompozR's GPU renderer (C++ Kompoz's platform/gpu): the device layer ([`device`]: the little of a graphics API the
//! renderer needs, on Vulkan now, with our own memory allocator), the shaders every backend draws with ([`shaders`]:
//! compiled from Slang by `cargo xtask shaders`, and what they read laid out as they read it) and a path's geometry
//! in the target's pixels ([`path`]: curves cut into lines, strokes into outlines, projected, as edges for the
//! coverage mask).
//!
//! `unsafe` code is confined to [`device::vulkan`], each block with a `// SAFETY:` comment; everything else is safe.

#![deny(unsafe_code, unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

pub mod device;
pub mod path;
pub mod shaders;
