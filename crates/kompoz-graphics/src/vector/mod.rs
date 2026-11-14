//! Vector path data, as Compose's `androidx.compose.ui.graphics.vector`: [`PathNode`]s (SVG path data's commands),
//! read from SVG path data by a [`PathParser`] ([`add_path_nodes`]), made by calls ([`PathBuilder`], [`PathData`]),
//! turned into a [`Path`](crate::Path) ([`to_path`]), and morphed from one list into another ([`lerp_path_nodes`]).

mod builder;
mod node;
mod parser;
mod to_path;

pub use builder::{PathBuilder, PathData};
pub use node::{EmptyPath, PathNode, can_morph, lerp_path_nodes};
pub use parser::{PathParser, add_path_nodes};
pub use to_path::{to_path, to_path_into};
