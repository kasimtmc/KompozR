//! The icons material3's components draw themselves, as Compose's internal Icons.kt: copies of a few of
//! material-icons-core's (a menu's arrows, a chip's check, a date picker's calendar, a time picker's clock and
//! keyboard, …), so that material3 needn't depend on the icons. Each is an [`ImageVector`] built the first time it is
//! asked for and shared after (Compose's backing `_name` fields), as the icons crate builds its own.
//!
//! In Kotlin's syntax, `Icons.Filled.Close` is `Icons::Filled::Close()`.

use kompoz_graphics::{
    Color, DefaultFillType, DefaultTintBlendMode, Dp, ImageVector, ImageVectorBuilder, PathBuilder, PathFillType,
    SolidColor, StrokeCap, StrokeJoin, VectorPath,
};

/// Material's icons material3 draws (Compose's internal `object Icons`): a module a theme, a function an icon.
pub(crate) mod Icons {
    /// The icons mirrored right to left (Compose's internal `Icons.AutoMirrored`).
    pub(crate) mod AutoMirrored {
        /// Compose's internal `Icons.AutoMirrored.Filled`.
        pub(crate) mod Filled {
            use super::super::super::{material_icon, material_path};
            use kompoz_graphics::ImageVector;
            use std::sync::OnceLock;

            /// An arrow to the left, to the start right to left (Compose's internal
            /// `Icons.AutoMirrored.Filled.KeyboardArrowLeft`).
            pub(crate) fn KeyboardArrowLeft() -> ImageVector {
                static ICON: OnceLock<ImageVector> = OnceLock::new();
                ICON.get_or_init(|| {
                    material_icon("AutoMirrored.Filled.KeyboardArrowLeft", true, |b| {
                        material_path(b, |p| {
                            p.move_to(15.41, 16.59);
                            p.line_to(10.83, 12.0);
                            p.line_to_relative(4.58, -4.59);
                            p.line_to(14.0, 6.0);
                            p.line_to_relative(-6.0, 6.0);
                            p.line_to_relative(6.0, 6.0);
                            p.line_to_relative(1.41, -1.41);
                            p.close();
                        });
                    })
                })
                .clone()
            }

            /// An arrow to the right, to the end right to left (Compose's internal
            /// `Icons.AutoMirrored.Filled.KeyboardArrowRight`).
            pub(crate) fn KeyboardArrowRight() -> ImageVector {
                static ICON: OnceLock<ImageVector> = OnceLock::new();
                ICON.get_or_init(|| {
                    material_icon("AutoMirrored.Filled.KeyboardArrowRight", true, |b| {
                        material_path(b, |p| {
                            p.move_to(8.59, 16.59);
                            p.line_to(13.17, 12.0);
                            p.line_to(8.59, 7.41);
                            p.line_to(10.0, 6.0);
                            p.line_to_relative(6.0, 6.0);
                            p.line_to_relative(-6.0, 6.0);
                            p.line_to_relative(-1.41, -1.41);
                            p.close();
                        });
                    })
                })
                .clone()
            }
        }
    }

    /// Compose's internal `Icons.Filled`.
    pub(crate) mod Filled {
        use super::super::{material_icon, material_path};
        use kompoz_graphics::ImageVector;
        use std::sync::OnceLock;

        /// A cross (Compose's internal `Icons.Filled.Close`).
        pub(crate) fn Close() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Filled.Close", false, |b| {
                    material_path(b, |p| {
                        p.move_to(19.0, 6.41);
                        p.line_to(17.59, 5.0);
                        p.line_to(12.0, 10.59);
                        p.line_to(6.41, 5.0);
                        p.line_to(5.0, 6.41);
                        p.line_to(10.59, 12.0);
                        p.line_to(5.0, 17.59);
                        p.line_to(6.41, 19.0);
                        p.line_to(12.0, 13.41);
                        p.line_to(17.59, 19.0);
                        p.line_to(19.0, 17.59);
                        p.line_to(13.41, 12.0);
                        p.close();
                    });
                })
            })
            .clone()
        }

        /// A check mark (Compose's internal `Icons.Filled.Check`).
        pub(crate) fn Check() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Filled.Check", false, |b| {
                    material_path(b, |p| {
                        p.move_to(9.0, 16.17);
                        p.line_to(4.83, 12.0);
                        p.line_to_relative(-1.42, 1.41);
                        p.line_to(9.0, 19.0);
                        p.line_to(21.0, 7.0);
                        p.line_to_relative(-1.41, -1.41);
                        p.close();
                    });
                })
            })
            .clone()
        }

        /// A pencil (Compose's internal `Icons.Filled.Edit`).
        pub(crate) fn Edit() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Filled.Edit", false, |b| {
                    material_path(b, |p| {
                        p.move_to(3.0, 17.25);
                        p.vertical_line_to(21.0);
                        p.horizontal_line_to_relative(3.75);
                        p.line_to(17.81, 9.94);
                        p.line_to_relative(-3.75, -3.75);
                        p.line_to(3.0, 17.25);
                        p.close();
                        p.move_to(20.71, 7.04);
                        p.curve_to_relative(0.39, -0.39, 0.39, -1.02, 0.0, -1.41);
                        p.line_to_relative(-2.34, -2.34);
                        p.curve_to_relative(-0.39, -0.39, -1.02, -0.39, -1.41, 0.0);
                        p.line_to_relative(-1.83, 1.83);
                        p.line_to_relative(3.75, 3.75);
                        p.line_to_relative(1.83, -1.83);
                        p.close();
                    });
                })
            })
            .clone()
        }

        /// A calendar (Compose's internal `Icons.Filled.DateRange`).
        pub(crate) fn DateRange() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Filled.DateRange", false, |b| {
                    material_path(b, |p| {
                        p.move_to(9.0, 11.0);
                        p.line_to(7.0, 11.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(-2.0);
                        p.close();
                        p.move_to(13.0, 11.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(-2.0);
                        p.close();
                        p.move_to(17.0, 11.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(-2.0);
                        p.close();
                        p.move_to(19.0, 4.0);
                        p.horizontal_line_to_relative(-1.0);
                        p.line_to(18.0, 2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(8.0, 4.0);
                        p.line_to(8.0, 2.0);
                        p.line_to(6.0, 2.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(5.0, 4.0);
                        p.curve_to_relative(-1.11, 0.0, -1.99, 0.9, -1.99, 2.0);
                        p.line_to(3.0, 20.0);
                        p.curve_to_relative(0.0, 1.1, 0.89, 2.0, 2.0, 2.0);
                        p.horizontal_line_to_relative(14.0);
                        p.curve_to_relative(1.1, 0.0, 2.0, -0.9, 2.0, -2.0);
                        p.line_to(21.0, 6.0);
                        p.curve_to_relative(0.0, -1.1, -0.9, -2.0, -2.0, -2.0);
                        p.close();
                        p.move_to(19.0, 20.0);
                        p.line_to(5.0, 20.0);
                        p.line_to(5.0, 9.0);
                        p.horizontal_line_to_relative(14.0);
                        p.vertical_line_to_relative(11.0);
                        p.close();
                    });
                })
            })
            .clone()
        }

        /// A triangle pointing down (Compose's internal `Icons.Filled.ArrowDropDown`).
        pub(crate) fn ArrowDropDown() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Filled.ArrowDropDown", false, |b| {
                    material_path(b, |p| {
                        p.move_to(7.0, 10.0);
                        p.line_to_relative(5.0, 5.0);
                        p.line_to_relative(5.0, -5.0);
                        p.close();
                    });
                })
            })
            .clone()
        }

        /// Three dots one above the other (Compose's internal `Icons.Filled.MoreVert`).
        pub(crate) fn MoreVert() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Filled.MoreVert", false, |b| {
                    material_path(b, |p| {
                        p.move_to(12.0, 8.0);
                        p.curve_to_relative(1.1, 0.0, 2.0, -0.9, 2.0, -2.0);
                        p.reflective_curve_to_relative(-0.9, -2.0, -2.0, -2.0);
                        p.reflective_curve_to_relative(-2.0, 0.9, -2.0, 2.0);
                        p.reflective_curve_to_relative(0.9, 2.0, 2.0, 2.0);
                        p.close();
                        p.move_to(12.0, 10.0);
                        p.curve_to_relative(-1.1, 0.0, -2.0, 0.9, -2.0, 2.0);
                        p.reflective_curve_to_relative(0.9, 2.0, 2.0, 2.0);
                        p.reflective_curve_to_relative(2.0, -0.9, 2.0, -2.0);
                        p.reflective_curve_to_relative(-0.9, -2.0, -2.0, -2.0);
                        p.close();
                        p.move_to(12.0, 16.0);
                        p.curve_to_relative(-1.1, 0.0, -2.0, 0.9, -2.0, 2.0);
                        p.reflective_curve_to_relative(0.9, 2.0, 2.0, 2.0);
                        p.reflective_curve_to_relative(2.0, -0.9, 2.0, -2.0);
                        p.reflective_curve_to_relative(-0.9, -2.0, -2.0, -2.0);
                        p.close();
                    });
                })
            })
            .clone()
        }
    }

    /// Compose's internal `Icons.Outlined`.
    pub(crate) mod Outlined {
        use super::super::{material_icon, material_path};
        use kompoz_graphics::ImageVector;
        use std::sync::OnceLock;

        /// A clock (Compose's internal `Icons.Outlined.Schedule`).
        pub(crate) fn Schedule() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Outlined.Schedule", false, |b| {
                    material_path(b, |p| {
                        p.move_to(11.99, 2.0);
                        p.curve_to(6.47, 2.0, 2.0, 6.48, 2.0, 12.0);
                        p.reflective_curve_to_relative(4.47, 10.0, 9.99, 10.0);
                        p.curve_to(17.52, 22.0, 22.0, 17.52, 22.0, 12.0);
                        p.reflective_curve_to(17.52, 2.0, 11.99, 2.0);
                        p.close();
                        p.move_to(12.0, 20.0);
                        p.curve_to_relative(-4.42, 0.0, -8.0, -3.58, -8.0, -8.0);
                        p.reflective_curve_to_relative(3.58, -8.0, 8.0, -8.0);
                        p.reflective_curve_to_relative(8.0, 3.58, 8.0, 8.0);
                        p.reflective_curve_to_relative(-3.58, 8.0, -8.0, 8.0);
                        p.close();
                        p.move_to(12.5, 7.0);
                        p.line_to(11.0, 7.0);
                        p.vertical_line_to_relative(6.0);
                        p.line_to_relative(5.25, 3.15);
                        p.line_to_relative(0.75, -1.23);
                        p.line_to_relative(-4.5, -2.67);
                        p.close();
                    });
                })
            })
            .clone()
        }

        /// A keyboard (Compose's internal `Icons.Outlined.Keyboard`).
        pub(crate) fn Keyboard() -> ImageVector {
            static ICON: OnceLock<ImageVector> = OnceLock::new();
            ICON.get_or_init(|| {
                material_icon("Outlined.Keyboard", false, |b| {
                    material_path(b, |p| {
                        p.move_to(20.0, 7.0);
                        p.vertical_line_to_relative(10.0);
                        p.line_to(4.0, 17.0);
                        p.line_to(4.0, 7.0);
                        p.horizontal_line_to_relative(16.0);
                        p.move_to_relative(0.0, -2.0);
                        p.line_to(4.0, 5.0);
                        p.curve_to_relative(-1.1, 0.0, -1.99, 0.9, -1.99, 2.0);
                        p.line_to(2.0, 17.0);
                        p.curve_to_relative(0.0, 1.1, 0.9, 2.0, 2.0, 2.0);
                        p.horizontal_line_to_relative(16.0);
                        p.curve_to_relative(1.1, 0.0, 2.0, -0.9, 2.0, -2.0);
                        p.line_to(22.0, 7.0);
                        p.curve_to_relative(0.0, -1.1, -0.9, -2.0, -2.0, -2.0);
                        p.close();
                        // (the keys: two by two dp, a row of three above a row of three, the space bar below)
                        p.move_to(11.0, 8.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.close();
                        p.move_to(11.0, 11.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.close();
                        p.move_to(8.0, 8.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(8.0, 10.0);
                        p.close();
                        p.move_to(8.0, 11.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(8.0, 13.0);
                        p.close();
                        p.move_to(5.0, 11.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(5.0, 13.0);
                        p.close();
                        p.move_to(5.0, 8.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(5.0, 10.0);
                        p.close();
                        p.move_to(8.0, 14.0);
                        p.horizontal_line_to_relative(8.0);
                        p.vertical_line_to_relative(2.0);
                        p.line_to(8.0, 16.0);
                        p.close();
                        p.move_to(14.0, 11.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.close();
                        p.move_to(14.0, 8.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.close();
                        p.move_to(17.0, 11.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.close();
                        p.move_to(17.0, 8.0);
                        p.horizontal_line_to_relative(2.0);
                        p.vertical_line_to_relative(2.0);
                        p.horizontal_line_to_relative(-2.0);
                        p.close();
                    });
                })
            })
            .clone()
        }
    }
}

/// All Material icons (currently) are 24dp by 24dp, with a viewport size of 24 by 24 (Compose's private
/// `MaterialIconDimension`).
const MaterialIconDimension: f32 = 24.0;

/// An icon of Material's size, `block` adding its paths, drawn mirrored right to left if `auto_mirror` (Compose's
/// private `materialIcon`, both overloads: the one without `autoMirror` is this one's `false`).
fn material_icon(name: &str, auto_mirror: bool, block: impl FnOnce(&mut ImageVectorBuilder)) -> ImageVector {
    let mut builder = ImageVector::Builder(
        name,
        Dp(MaterialIconDimension),
        Dp(MaterialIconDimension),
        MaterialIconDimension,
        MaterialIconDimension,
        Color::Unspecified,
        DefaultTintBlendMode,
        auto_mirror,
    );
    block(&mut builder);
    builder.build()
}

/// A path of what `path_builder` makes, filled black, no stroke (1 px, butt caps, bevel joins, miter 1) (Compose's
/// private `ImageVector.Builder.materialPath`, its defaults: alphas of 1, the default fill type).
fn material_path(builder: &mut ImageVectorBuilder, path_builder: impl FnOnce(&mut PathBuilder)) {
    material_path_with(builder, 1.0, 1.0, DefaultFillType, path_builder);
}

/// [`material_path`] of these alphas and fill type (Compose's `materialPath(fillAlpha, strokeAlpha, pathFillType)`).
fn material_path_with(
    builder: &mut ImageVectorBuilder,
    fill_alpha: f32,
    stroke_alpha: f32,
    path_fill_type: PathFillType,
    path_builder: impl FnOnce(&mut PathBuilder),
) {
    let paints = VectorPath {
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
    };
    builder.path_of(paints, path_builder);
}
