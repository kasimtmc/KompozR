//! The core icons as Compose's Kotlin builds them: every icon of every theme built from its data, their paths the
//! nodes the Kotlin builder's calls add, their paints Compose's `materialPath`'s, the two-tone ones' translucent
//! paths, the auto-mirrored ones mirrored, each built once and shared.

#![allow(non_snake_case)]

use kompoz_graphics::{
    Color, Dp, ImageVector, PathData, PathFillType, PathNode, SolidColor, StrokeCap, StrokeJoin, VectorNode,
};
use kompoz_material_icons_core::{Icons, MaterialPathBuilder, material_icon};
use kompoz_runtime::named_args;

fn paths(icon: &ImageVector) -> Vec<&kompoz_graphics::VectorPath> {
    icon.root()
        .children
        .iter()
        .filter_map(|n| match n {
            VectorNode::Path(p) => Some(p),
            VectorNode::Group(_) => None,
        })
        .collect()
}

#[test]
fn add_is_the_paths_the_kotlin_builds() {
    let add = Icons::Filled::Add();
    assert_eq!(add.name(), "Filled.Add");
    assert_eq!((add.default_width(), add.default_height()), (Dp(24.0), Dp(24.0)));
    assert_eq!((add.viewport_width(), add.viewport_height()), (24.0, 24.0));
    assert!(!add.auto_mirror());
    let p = paths(&add);
    assert_eq!(p.len(), 1);
    // (Icons.Filled.Add's Kotlin: materialPath { moveTo(19f, 13f); horizontalLineToRelative(-6f); … close() })
    let kotlin = PathData(|b| {
        b.move_to(19.0, 13.0)
            .horizontal_line_to_relative(-6.0)
            .vertical_line_to_relative(6.0)
            .horizontal_line_to_relative(-2.0)
            .vertical_line_to_relative(-6.0)
            .horizontal_line_to(5.0)
            .vertical_line_to_relative(-2.0)
            .horizontal_line_to_relative(6.0)
            .vertical_line_to(5.0)
            .horizontal_line_to_relative(2.0)
            .vertical_line_to_relative(6.0)
            .horizontal_line_to_relative(6.0)
            .vertical_line_to_relative(2.0)
            .close();
    });
    assert_eq!(p[0].path_data, kotlin);
    // (materialPath's paints)
    assert_eq!(p[0].fill, Some(SolidColor(Color::Black)));
    assert_eq!((p[0].fill_alpha, p[0].stroke_alpha), (1.0, 1.0));
    assert!(p[0].stroke.is_none());
    assert_eq!((p[0].stroke_line_width, p[0].stroke_line_miter), (1.0, 1.0));
    assert_eq!((p[0].stroke_line_cap, p[0].stroke_line_join), (StrokeCap::Butt, StrokeJoin::Bevel));
    assert_eq!(p[0].path_fill_type, PathFillType::NonZero);
}

#[test]
fn arcs_and_reflective_curves_come_through() {
    // (Icons.Outlined.LocationOn's arcToRelative(2.5f, 2.5f, 0f, true, true, 5f, 0f): its flags as Kotlin's booleans)
    let location = Icons::Outlined::LocationOn();
    let nodes: Vec<&PathNode> = paths(&location).iter().flat_map(|p| p.path_data.iter()).collect();
    let arc = nodes.iter().find(|n| matches!(n, PathNode::RelativeArcTo { .. })).expect("an arc");
    assert_eq!(
        **arc,
        PathNode::RelativeArcTo {
            horizontal_ellipse_radius: 2.5,
            vertical_ellipse_radius: 2.5,
            theta: 0.0,
            is_more_than_half: true,
            is_positive_arc: true,
            arc_start_dx: 5.0,
            arc_start_dy: 0.0
        }
    );
    let account = Icons::Filled::AccountCircle();
    let nodes = &paths(&account)[0].path_data;
    assert!(nodes.iter().any(|n| matches!(n, PathNode::RelativeReflectiveCurveTo { .. })), "{nodes:?}");
    let face = Icons::Filled::Face();
    let nodes: Vec<&PathNode> = paths(&face).iter().flat_map(|p| p.path_data.iter()).collect();
    assert!(nodes.iter().any(|n| matches!(n, PathNode::RelativeCurveTo { .. })));
    assert!(nodes.iter().any(|n| matches!(n, PathNode::Close)));
}

#[test]
fn two_tone_icons_have_their_translucent_paths() {
    let account_box = Icons::TwoTone::AccountBox();
    let p = paths(&account_box);
    assert_eq!(p.len(), 3);
    assert_eq!((p[0].fill_alpha, p[0].stroke_alpha), (0.3, 0.3));
    assert_eq!((p[1].fill_alpha, p[2].fill_alpha), (1.0, 1.0));
}

#[test]
fn auto_mirrored_icons_mirror_and_the_default_is_filled() {
    let back = Icons::AutoMirrored::Filled::ArrowBack();
    assert!(back.auto_mirror());
    assert_eq!(back.name(), "AutoMirrored.Filled.ArrowBack");
    assert_eq!(Icons::Default::Home(), Icons::Filled::Home());
    assert_eq!(Icons::AutoMirrored::Default::Send(), Icons::AutoMirrored::Filled::Send());
    #[allow(deprecated)]
    let old_back = Icons::Filled::ArrowBack();
    assert!(!old_back.auto_mirror());
}

#[test]
fn each_icon_is_built_once() {
    // (Compose's cached `_add`: the same vector, its id the same, each time)
    assert_eq!(Icons::Outlined::Star().id(), Icons::Outlined::Star().id());
    assert_ne!(Icons::Outlined::Star().id(), Icons::Rounded::Star().id());
}

#[test]
fn every_icon_of_every_theme_builds() {
    macro_rules! all {
        ($($theme:ident),*) => {
            $(
                for icon in [
                    Icons::$theme::AccountBox(), Icons::$theme::AccountCircle(), Icons::$theme::Add(),
                    Icons::$theme::AddCircle(), Icons::$theme::ArrowDropDown(), Icons::$theme::Build(),
                    Icons::$theme::Call(), Icons::$theme::Check(), Icons::$theme::CheckCircle(), Icons::$theme::Clear(),
                    Icons::$theme::Close(), Icons::$theme::Create(), Icons::$theme::DateRange(),
                    Icons::$theme::Delete(), Icons::$theme::Done(), Icons::$theme::Edit(), Icons::$theme::Email(),
                    Icons::$theme::Face(), Icons::$theme::Favorite(), Icons::$theme::FavoriteBorder(),
                    Icons::$theme::Home(), Icons::$theme::Info(), Icons::$theme::KeyboardArrowDown(),
                    Icons::$theme::KeyboardArrowUp(), Icons::$theme::LocationOn(), Icons::$theme::Lock(),
                    Icons::$theme::MailOutline(), Icons::$theme::Menu(), Icons::$theme::MoreVert(),
                    Icons::$theme::Notifications(), Icons::$theme::Person(), Icons::$theme::Phone(),
                    Icons::$theme::Place(), Icons::$theme::PlayArrow(), Icons::$theme::Refresh(),
                    Icons::$theme::Search(), Icons::$theme::Settings(), Icons::$theme::Share(),
                    Icons::$theme::ShoppingCart(), Icons::$theme::Star(), Icons::$theme::ThumbUp(),
                    Icons::$theme::Warning(),
                    Icons::AutoMirrored::$theme::ArrowBack(), Icons::AutoMirrored::$theme::ArrowForward(),
                    Icons::AutoMirrored::$theme::ExitToApp(), Icons::AutoMirrored::$theme::KeyboardArrowLeft(),
                    Icons::AutoMirrored::$theme::KeyboardArrowRight(), Icons::AutoMirrored::$theme::List(),
                    Icons::AutoMirrored::$theme::Send(),
                ] {
                    assert!(!paths(&icon).is_empty() && paths(&icon).iter().all(|p| !p.path_data.is_empty()), "{}", icon.name());
                    assert!(icon.name().contains(stringify!($theme)));
                }
            )*
        };
    }
    all!(Filled, Outlined, Rounded, Sharp, TwoTone);
}

#[test]
fn an_icon_of_ones_own() {
    // (Compose's `materialIcon(name = "Dot") { materialPath(fillAlpha = 0.5f) { moveTo(…) … } }`)
    let dot = named_args!(material_icon(
        "Dot",
        block = |b| {
            named_args!(b.material_path(
                fill_alpha = 0.5,
                path_builder = |p| {
                    p.move_to(12.0, 12.0).line_to(13.0, 12.0).close();
                }
            ));
        }
    ));
    assert_eq!((dot.name(), dot.default_width(), dot.auto_mirror()), ("Dot", Dp(24.0), false));
    let p = paths(&dot);
    assert_eq!((p.len(), p[0].fill_alpha, p[0].fill.clone()), (1, 0.5, Some(SolidColor(Color::Black))));
}
