//! KompozR's graphics foundations, as Compose's `ui-graphics`, `ui-geometry` and `ui-unit`: units and geometry,
//! colors and color spaces, HCT and dynamic color (Material Color Utilities), paths and their effects, brushes,
//! draw styles (fill, stroke) and shadows, shapes, vector images, bitmaps and decoding. Lengths are pixels; what an app writes is in dp and sp ([`Dp`],
//! [`TextUnit`]), turned into pixels by a [`Density`].

#![allow(non_upper_case_globals, non_snake_case)]

pub mod brush;
pub mod color;
pub mod colorspace;
pub mod constraints;
pub mod draw_style;
pub mod geometry;
pub mod image;
mod kotlin;
pub mod material_color;
pub mod material_shapes;
pub mod matrix;
pub mod path;
pub mod projective;
pub mod shadow;
pub mod shape;
pub mod unit;
pub mod vector;

pub use brush::*;
pub use color::*;
pub use constraints::*;
pub use draw_style::*;
pub use geometry::*;
pub use image::*;
pub use material_shapes::*;
pub use matrix::*;
pub use path::*;
pub use projective::*;
pub use shadow::*;
pub use shape::*;
pub use unit::*;
pub use vector::*;
