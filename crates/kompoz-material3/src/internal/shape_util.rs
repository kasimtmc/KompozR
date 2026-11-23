//! Paths of material shapes, as Compose's internal ShapeUtil.kt: a [`RoundedPolygon`] through a matrix, and a
//! polygon or a [`Morph`] (at a progress) as a path turned to start at an angle, twice round if asked, closed or
//! not — what the shaped progress indicators and loading indicators draw.
//!
//! kompoz-graphics' material shapes carry this file's code (`pathFromCubics` and its callers, the
//! `RoundedPolygon.transformed(matrix)` point mapping): these are Compose's internal functions, with their defaults,
//! over them.

// (Morph.toPath's seven parameters, and the morph: the trait `#[named]` makes of it)
#![allow(clippy::too_many_arguments)]

use kompoz_graphics::material_shapes::{Morph, RoundedPolygon};
use kompoz_graphics::{Matrix, Path};
use kompoz_runtime::named;

/// The internal paths of polygons (Compose's internal `RoundedPolygon` extensions).
#[named(pub(crate) trait RoundedPolygonPaths)]
impl RoundedPolygon {
    /// The polygon with each point through `matrix` (Compose's internal `RoundedPolygon.transformed(matrix)`).
    fn transformed_by_matrix(&self, matrix: &Matrix) -> RoundedPolygon {
        self.transformed_by(matrix)
    }

    /// Its outline into `path` (emptied first; a new one by default), turned about its center so that it starts
    /// `start_angle` degrees round (12 o'clock by default), twice round if `repeat_path`, closed if `close_path`
    /// (Compose's internal `RoundedPolygon.toPath`).
    fn to_path_with(
        &self,
        #[default(Path::new())] path: Path,
        #[default(270)] start_angle: i32,
        #[default(false)] repeat_path: bool,
        #[default(true)] close_path: bool,
    ) -> Path {
        let mut path = path;
        self.to_path_into(&mut path, start_angle, repeat_path, close_path);
        path
    }
}

/// The internal paths of morphs (Compose's internal `Morph.toPath`).
#[named(pub(crate) trait MorphPaths)]
impl Morph {
    /// The shape `progress` of the way into `path` (emptied first; a new one by default), turned about the
    /// rotation pivot (the origin by default: a morph of polygons whose centers are there) so that it starts
    /// `start_angle` degrees round (12 o'clock by default), twice round if `repeat_path`, closed if `close_path`
    /// (Compose's internal `Morph.toPath`).
    fn to_path_with(
        &self,
        progress: f32,
        #[default(Path::new())] path: Path,
        #[default(270)] start_angle: i32,
        #[default(false)] repeat_path: bool,
        #[default(true)] close_path: bool,
        #[default(0.0)] rotation_pivot_x: f32,
        #[default(0.0)] rotation_pivot_y: f32,
    ) -> Path {
        let mut path = path;
        self.to_path_into(
            progress,
            &mut path,
            start_angle,
            repeat_path,
            close_path,
            rotation_pivot_x,
            rotation_pivot_y,
        );
        path
    }
}

/// A shared shape as a shape (what `Modifier.clip` takes by value), equal as the shape is (surfaces, icon buttons,
/// list items, the navigation items' indicator ripples); it prints as the shape (a list item's semantics name it).
#[derive(Debug)]
pub(crate) struct ClipShape(pub(crate) std::sync::Arc<dyn kompoz_graphics::Shape>);

impl PartialEq for ClipShape {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl kompoz_graphics::Shape for ClipShape {
    fn create_outline(
        &self,
        size: kompoz_graphics::Size,
        layout_direction: kompoz_graphics::LayoutDirection,
        density: &kompoz_graphics::Density,
    ) -> kompoz_graphics::Outline {
        self.0.create_outline(size, layout_direction, density)
    }
}

impl std::fmt::Display for ClipShape {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
