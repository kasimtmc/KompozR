//! KompozR's GPU renderer (C++ Kompoz's platform/gpu): the device layer ([`device`]: the little of a graphics API the
//! renderer needs, on Vulkan now, with our own memory allocator), the shaders every backend draws with ([`shaders`]:
//! compiled from Slang by `cargo xtask shaders`, and what they read laid out as they read it), a path's geometry in
//! the target's pixels ([`path`]: curves cut into lines, strokes into outlines, projected, as edges for the coverage
//! mask), text's glyphs ([`glyph`]: rasterized as FreeType does, colour glyphs painted, kept in an atlas), and the
//! [`renderer`] that draws kompoz-ui's GraphicsContext with them; [`golden`] tells a picture from its golden one.
//!
//! `unsafe` code is confined to the backends' modules (`device::vulkan` where Vulkan is the target's API), each block
//! with a `// SAFETY:` comment; everything else is safe.

#![deny(unsafe_code, unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

pub mod device;
pub mod glyph;
pub mod golden;
pub mod path;
pub mod renderer;
pub mod shaders;

pub use renderer::Renderer;
