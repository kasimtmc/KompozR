//! KompozR's core Material icons, as Compose's `androidx.compose.material:material-icons-core` (1.7.8): the icons
//! most apps use, in each of Material's themes — `Icons::Filled::Add()`, `Icons::Outlined::Home()`,
//! `Icons::AutoMirrored::Filled::ArrowBack()` (mirrored right to left) — each an [`ImageVector`] built once, when
//! first asked for, and shared after. [`Icons::Default`] is the filled theme. Draw one with material3's `Icon`
//! (it tints it with the content color) or foundation's `Image`.
//!
//! A cargo feature a theme (`filled`, `outlined`, `rounded`, `sharp`, `two-tone`, `auto-mirrored`; all by default)
//! leaves the others out. The icons are data (`data/<theme>.txt`: each icon's paths as SVG path data) converted
//! from Compose's Kotlin by `cargo xtask material-icons`; [`material_icon`] and
//! [`material_path`](MaterialPathBuilder::material_path) build one's own icon the way they are built.
//!
//! In Kotlin's syntax, `Icons.Filled.Add` is `Icons::Filled::Add()` (the generator's rule for `Icons.<theme>.<name>`).

#![allow(non_snake_case, non_upper_case_globals)]

use kompoz_graphics::{
    Color, Dp, ImageVector, ImageVectorBuilder, PathBuilder, PathData, PathFillType, SolidColor, StrokeCap, StrokeJoin,
    VectorPath, add_path_nodes,
};
use kompoz_runtime::named;

/// The size of Material's icons, dp, and of their viewport (Compose's `MaterialIconDimension`).
pub(crate) const MaterialIconDimension: f32 = 24.0;

/// An icon of Material's size, 24 by 24 dp in a 24 by 24 viewport (Compose's `materialIcon`): `block` adds its
/// paths (with [`material_path`](MaterialPathBuilder::material_path)); `auto_mirror`: drawn mirrored right to left.
#[named]
pub fn material_icon(
    name: &str,
    #[default(false)] auto_mirror: bool,
    block: impl FnOnce(&mut ImageVectorBuilder),
) -> ImageVector {
    let mut builder = ImageVector::Builder(
        name,
        Dp(MaterialIconDimension),
        Dp(MaterialIconDimension),
        MaterialIconDimension,
        MaterialIconDimension,
        Color::Unspecified,
        kompoz_graphics::DefaultTintBlendMode,
        auto_mirror,
    );
    block(&mut builder);
    builder.build()
}

/// The path of a Material icon (Compose's `ImageVector.Builder.materialPath`).
#[named(trait MaterialPathBuilder)]
impl ImageVectorBuilder {
    /// A path of what `path_builder` makes, filled black at `fill_alpha` (its stroke at `stroke_alpha`: none drawn,
    /// 1 px, butt caps, bevel joins), filled by `path_fill_type` (Compose's `materialPath`).
    fn material_path(
        &mut self,
        #[default(1.0)] fill_alpha: f32,
        #[default(1.0)] stroke_alpha: f32,
        #[default(kompoz_graphics::DefaultFillType)] path_fill_type: PathFillType,
        path_builder: impl FnOnce(&mut PathBuilder),
    ) -> &mut ImageVectorBuilder {
        self.add_path_of(material_vector_path(PathData(path_builder), fill_alpha, stroke_alpha, path_fill_type))
    }
}

/// Compose's `materialPath`'s paints: filled black, no stroke (its width 1, butt, bevel, miter 1).
fn material_vector_path(
    path_data: Vec<kompoz_graphics::PathNode>,
    fill_alpha: f32,
    stroke_alpha: f32,
    path_fill_type: PathFillType,
) -> VectorPath {
    VectorPath {
        path_data,
        fill: Some(SolidColor(Color::Black)),
        fill_alpha,
        stroke: None,
        stroke_alpha,
        stroke_line_width: 1.0,
        stroke_line_cap: StrokeCap::Butt,
        stroke_line_join: StrokeJoin::Bevel,
        stroke_line_miter: 1.0,
        path_fill_type,
        ..VectorPath::default()
    }
}

/// An icon of its data line (`Name|fillAlpha,strokeAlpha,N|E:pathData|…`): as Compose's Kotlin builds it.
#[doc(hidden)]
pub fn build_icon(name: &str, line: &str) -> ImageVector {
    let auto_mirror = name.starts_with("AutoMirrored.");
    let block = |b: &mut ImageVectorBuilder| {
        for path in line.split('|').skip(1) {
            let (paints, data) = path.split_once(':').expect("an icon path's paints");
            let mut paints = paints.split(',');
            let mut number = || paints.next().and_then(|p| p.parse::<f32>().ok()).expect("an icon path's alpha");
            let (fill_alpha, stroke_alpha) = (number(), number());
            let fill_type = match paints.next() {
                Some("E") => PathFillType::EvenOdd,
                _ => PathFillType::NonZero,
            };
            b.add_path_of(material_vector_path(add_path_nodes(data), fill_alpha, stroke_alpha, fill_type));
        }
    };
    kompoz_runtime::named_args!(material_icon(name, auto_mirror = auto_mirror, block = block))
}

/// The icons of a theme: a function an icon, building it from its line of the theme's data once.
macro_rules! icons {
    ($theme:literal, $data:literal; $( $(#[$attr:meta])* $name:ident $start:literal $end:literal ),* $(,)?) => {
        const DATA: &str = include_str!($data);
        $(
            #[doc = concat!("Compose's `Icons.", $theme, ".", stringify!($name), "`.")]
            $(#[$attr])*
            pub fn $name() -> ::kompoz_graphics::ImageVector {
                static ICON: ::std::sync::OnceLock<::kompoz_graphics::ImageVector> = ::std::sync::OnceLock::new();
                ICON.get_or_init(|| $crate::build_icon(concat!($theme, ".", stringify!($name)), &DATA[$start..$end]))
                    .clone()
            }
        )*
    };
}
pub(crate) use icons;

/// Material's icons (Compose's `object Icons`): a module a theme, a function an icon.
#[path = "icons.rs"]
pub mod Icons;

/// What an app using the icons imports.
pub mod prelude {
    pub use crate::{Icons, MaterialPathBuilder, material_icon};
}
