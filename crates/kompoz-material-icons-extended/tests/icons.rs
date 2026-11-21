//! The extended icons as Compose's Kotlin builds them: every icon of every theme built from its data, a few icons'
//! paths checked against their Kotlin builder's calls, their paints Compose's `materialPath`'s (the two-tone ones'
//! translucent paths, the even-odd ones), the auto-mirrored ones mirrored and the deprecated unmirrored ones not, each
//! built once and shared.

#![allow(non_snake_case)]

use kompoz_graphics::{Color, Dp, ImageVector, PathData, PathFillType, SolidColor, StrokeCap, StrokeJoin, VectorNode};
use kompoz_material_icons_extended::Icons;

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
fn star_border_is_the_paths_the_kotlin_builds() {
    let star = Icons::Outlined::StarBorder();
    assert_eq!(star.name(), "Outlined.StarBorder");
    assert_eq!((star.default_width(), star.default_height()), (Dp(24.0), Dp(24.0)));
    assert_eq!((star.viewport_width(), star.viewport_height()), (24.0, 24.0));
    assert!(!star.auto_mirror());
    let p = paths(&star);
    assert_eq!(p.len(), 1);
    // (Icons.Outlined.StarBorder's Kotlin: materialPath { moveTo(22.0f, 9.24f); lineToRelative(-7.19f, -0.62f); … })
    let kotlin = PathData(|b| {
        b.move_to(22.0, 9.24)
            .line_to_relative(-7.19, -0.62)
            .line_to(12.0, 2.0)
            .line_to(9.19, 8.63)
            .line_to(2.0, 9.24)
            .line_to_relative(5.46, 4.73)
            .line_to(5.82, 21.0)
            .line_to(12.0, 17.27)
            .line_to(18.18, 21.0)
            .line_to_relative(-1.63, -7.03)
            .line_to(22.0, 9.24)
            .close()
            .move_to(12.0, 15.4)
            .line_to_relative(-3.76, 2.27)
            .line_to_relative(1.0, -4.28)
            .line_to_relative(-3.32, -2.88)
            .line_to_relative(4.38, -0.38)
            .line_to(12.0, 6.1)
            .line_to_relative(1.71, 4.04)
            .line_to_relative(4.38, 0.38)
            .line_to_relative(-3.32, 2.88)
            .line_to_relative(1.0, 4.28)
            .line_to(12.0, 15.4)
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
fn auto_mirrored_icons_mirror_and_their_deprecated_unmirrored_ones_do_not() {
    // (Icons.AutoMirrored.Filled.MenuOpen: materialIcon(…, autoMirror = true); its first contour moveTo(3f, 18f),
    // horizontalLineToRelative(13f), verticalLineToRelative(-2f), lineTo(3f, 16f), verticalLineToRelative(2f), close())
    let menu_open = Icons::AutoMirrored::Filled::MenuOpen();
    assert!(menu_open.auto_mirror());
    assert_eq!(menu_open.name(), "AutoMirrored.Filled.MenuOpen");
    let first = PathData(|b| {
        b.move_to(3.0, 18.0)
            .horizontal_line_to_relative(13.0)
            .vertical_line_to_relative(-2.0)
            .line_to(3.0, 16.0)
            .vertical_line_to_relative(2.0)
            .close();
    });
    assert_eq!(paths(&menu_open)[0].path_data[..first.len()], first[..]);
    assert_eq!(Icons::AutoMirrored::Default::MenuOpen(), menu_open);
    // (Icons.Filled.MenuOpen: @Deprecated("Use the AutoMirrored version at Icons.AutoMirrored.Filled.MenuOpen"), not
    // mirrored)
    #[allow(deprecated)]
    let old = Icons::Filled::MenuOpen();
    assert!(!old.auto_mirror());
    assert_eq!(old.name(), "Filled.MenuOpen");
    assert_eq!(Icons::Default::Tune(), Icons::Filled::Tune());
}

#[test]
fn even_odd_and_translucent_paths_keep_their_paints() {
    // (Icons.Sharp.PushPin: materialPath(pathFillType = EvenOdd), EvenOdd imported from PathFillType.Companion)
    let pin = Icons::Sharp::PushPin();
    let p = paths(&pin);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].path_fill_type, PathFillType::EvenOdd);
    let start = PathData(|b| {
        b.move_to(16.0, 9.0).vertical_line_to(4.0).line_to_relative(2.0, 0.0).vertical_line_to(2.0);
    });
    assert_eq!(p[0].path_data[..start.len()], start[..]);
    // (Icons.TwoTone.AlternateEmail: its one materialPath(fillAlpha = 0.9f), its stroke alpha the default 1)
    let email = Icons::TwoTone::AlternateEmail();
    let p = paths(&email);
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].fill_alpha, p[0].stroke_alpha), (0.9, 1.0));
    // (Icons.TwoTone.LunchDining: two materialPath(fillAlpha = 0.3f) under three plain ones)
    let lunch = Icons::TwoTone::LunchDining();
    let alphas: Vec<f32> = paths(&lunch).iter().map(|p| p.fill_alpha).collect();
    assert_eq!(alphas, [0.3, 0.3, 1.0, 1.0, 1.0]);
}

#[test]
fn names_of_their_own_come_through() {
    // (Kotlin's `Icons.Filled._10k`: a name starting with a digit, written with an underscore; and
    // `materialIcon(name =\n "TwoTone.SignalWifiConnectedNoInternet4")`, its name on a line of its own)
    assert_eq!(Icons::Filled::_10k().name(), "Filled._10k");
    assert_eq!(Icons::TwoTone::SignalWifiConnectedNoInternet4().name(), "TwoTone.SignalWifiConnectedNoInternet4");
    assert_eq!(Icons::Rounded::Bookmarks().name(), "Rounded.Bookmarks");
    assert_eq!(Icons::AutoMirrored::TwoTone::WrapText().name(), "AutoMirrored.TwoTone.WrapText");
}

#[test]
fn a_themes_module_has_the_core_sets_icons_too() {
    // (Kotlin's one `Icons.Filled` with the extended icons a dependency: core's Add and the extended set's Tune; the
    // same vectors as the core crate's, built once)
    assert_eq!(Icons::Filled::Add().id(), kompoz_material_icons_core::Icons::Filled::Add().id());
    assert_eq!(Icons::Default::Home(), kompoz_material_icons_core::Icons::Filled::Home());
    assert_eq!(Icons::Filled::Tune().name(), "Filled.Tune");
    let back = Icons::AutoMirrored::Outlined::ArrowBack();
    assert!(back.auto_mirror());
    assert_eq!(back.id(), kompoz_material_icons_core::Icons::AutoMirrored::Outlined::ArrowBack().id());
}

#[test]
fn each_icon_is_built_once() {
    // (Compose's cached `_starBorder`: the same vector, its id the same, each time)
    assert_eq!(Icons::Outlined::StarBorder().id(), Icons::Outlined::StarBorder().id());
    assert_ne!(Icons::Outlined::StarBorder().id(), Icons::Filled::StarBorder().id());
}

#[test]
fn every_icon_of_every_theme_builds() {
    // (each data line as its icon function builds it: every path's data parses into nodes, 11 105 icons)
    let themes: [(&str, &str); 10] = [
        ("Filled", include_str!("../data/filled.txt")),
        ("Outlined", include_str!("../data/outlined.txt")),
        ("Rounded", include_str!("../data/rounded.txt")),
        ("Sharp", include_str!("../data/sharp.txt")),
        ("TwoTone", include_str!("../data/two_tone.txt")),
        ("AutoMirrored.Filled", include_str!("../data/auto_mirrored_filled.txt")),
        ("AutoMirrored.Outlined", include_str!("../data/auto_mirrored_outlined.txt")),
        ("AutoMirrored.Rounded", include_str!("../data/auto_mirrored_rounded.txt")),
        ("AutoMirrored.Sharp", include_str!("../data/auto_mirrored_sharp.txt")),
        ("AutoMirrored.TwoTone", include_str!("../data/auto_mirrored_two_tone.txt")),
    ];
    let mut count = 0;
    for (theme, data) in themes {
        for line in data.lines() {
            let name = format!("{theme}.{}", line.split('|').next().unwrap());
            let icon = kompoz_material_icons_core::build_icon(&name, line);
            assert_eq!(icon.auto_mirror(), theme.starts_with("AutoMirrored."), "{name}");
            let p = paths(&icon);
            assert!(!p.is_empty() && p.iter().all(|p| !p.path_data.is_empty()), "{name}");
            count += 1;
        }
    }
    assert_eq!(count, 2083 * 5 + 138 * 5);
}
