//! Vector images, as Compose's `androidx.compose.ui.graphics.vector`: path data ([`PathNode`]s, SVG path data's
//! commands), read from SVG path data by a [`PathParser`] ([`add_path_nodes`]), made by calls ([`PathBuilder`],
//! [`PathData`]), turned into a [`Path`](crate::Path) ([`to_path`]) and morphed from one list into another
//! ([`lerp_path_nodes`]); vectors of paths and groups ([`ImageVector`], built by an [`ImageVectorBuilder`]), read
//! from Android vector drawables ([`ImageVector::from_vector_xml`]). Animated ones are animation-graphics'
//! (kompoz-animation-graphics' AnimatedImageVector), as Compose's.

mod builder;
mod image_vector;
mod node;
mod parser;
mod to_path;
mod vector_xml;
mod xml;

pub use builder::{PathBuilder, PathData, PathData_async};
pub use image_vector::{
    DefaultFillType, DefaultGroupName, DefaultPathName, DefaultPivotX, DefaultPivotY, DefaultRotation, DefaultScaleX,
    DefaultScaleY, DefaultStrokeLineCap, DefaultStrokeLineJoin, DefaultStrokeLineMiter, DefaultStrokeLineWidth,
    DefaultTintBlendMode, DefaultTintColor, DefaultTranslationX, DefaultTranslationY, DefaultTrimPathEnd,
    DefaultTrimPathOffset, DefaultTrimPathStart, ImageVector, ImageVectorBuilder, VectorGroup, VectorNode, VectorPath,
    tintable_with_alpha_mask,
};
pub use node::{EmptyPath, PathNode, can_morph, lerp_path_nodes};
pub use parser::{PathParser, add_path_nodes};
pub use to_path::{to_path, to_path_into};
