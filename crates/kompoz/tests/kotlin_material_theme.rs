//! Material's theme in Kotlin's syntax, as a Compose user writes it: the C++ Kompoz's `theming_as_compose_writes_it`
//! (kompozable_tests.cpp 2299-2343: an enum class in a `when`, a color scheme with its colors by name, Typography
//! and TextStyle copies, `MaterialTheme(typography = …)`, `MaterialTheme.colorScheme.primary`, a value-returning
//! composable remembering on its own however often it is called) through `kompoz!`, and material3's samples for the
//! theme, colors, type and Surface from a `.kt` file (kompoz_file!). (The C++ test's AppTheme and Inbox are in
//! kotlin_ported.rs, their themes now Material's.)

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::Cell;

// ---- theming_as_compose_writes_it (kompozable_tests.cpp 2299-2343) -------------------------------------------------

kompoz! {
    enum class ThemeChoice { System, Light, Dark }

    fun schemeFor(choice: ThemeChoice): ColorScheme = when (choice) {
        ThemeChoice.Dark -> darkColorScheme(primary = Color(0xFF9ECAFF))
        else -> lightColorScheme(primary = Color(0xFF0061A4), onPrimary = Color.White)
    }

    var nextId = 0
    var medium: CornerBasedShape? = null

    @Composable
    fun counted(): Int = remember { ++nextId }

    @Composable
    fun Themed(choice: ThemeChoice) {
        val base = MaterialTheme.typography
        val type = base.copy(headlineSmall = base.headlineSmall.copy(fontFamily = FontFamily.Serif, fontSize = 30.sp))
        var showFirst by remember { mutableStateOf(true) }
        MaterialTheme(colorScheme = schemeFor(choice), typography = type, shapes = Shapes(medium = RoundedCornerShape(20.dp))) {
            Column {
                medium = MaterialTheme.shapes.medium
                Text("${MaterialTheme.typography.headlineSmall.fontSize.value.toInt()}", modifier = Modifier.testTag("size"), color = MaterialTheme.colorScheme.primary)
                if (showFirst) Text("a ${counted()}")
                Text("b ${counted()}")
                Button(onClick = { showFirst = false }) { Text("hide") }
            }
        }
    }
}

fn theming_as_compose_writes_it() {
    // (Color(Long): AARRGGBB)
    assert_eq!(scheme_for(ThemeChoice::Dark).primary, Color(0xFF9ECAFF_i64));
    assert_eq!(scheme_for(ThemeChoice::Light).on_primary, Color::White);
    let rule = KompozTestRule::new(kompozable_lambda!(|| Themed(ThemeChoice::System)));
    rule.on_node_with_tag("size").assert_text_equals(&["30"]);
    let canvas = rule.draw();
    let size = canvas.texts.iter().position(|t| t == "30").unwrap();
    assert_eq!(canvas.colors[size], Color(0xFF0061A4_i64));
    assert_eq!(medium.with(|m| m.get()), Some(CornerBasedShape::from(RoundedCornerShape(20.dp()))));
    rule.on_node_with_text("a 1").assert_exists();
    rule.on_node_with_text("b 2").assert_exists();
    rule.on_node_with_text("hide").perform_click();
    // The first call gone, the second keeps what it remembered: each call a group of its own, not a slot by order
    // from the same site (which would hand b a's).
    rule.on_node_with_text("a 1").assert_does_not_exist();
    rule.on_node_with_text("b 2").assert_exists();
}

// ---- material3's samples (kt/material_theme.kt) ------------------------------------------------------------------

kompoz_file!("tests/kt/material_theme.kt");

fn material_theme_sample() {
    // Desktop: the baseline schemes of the sample's primaries until the host has the system's palette, then those.
    let rule = KompozTestRule::new(MaterialThemeSample);
    rule.on_node_with_text("Text style and color from light theme").assert_exists();
    rule.on_node_with_content_description("Localized Description").assert_exists();
    let canvas = rule.draw();
    let label = canvas.texts.iter().position(|t| t.starts_with("Text style")).unwrap();
    // (the typography given; the icon in the content color around, black: a theme gives none of its own)
    assert_eq!(canvas.text_styles[label].font_weight, FontWeight::W600);
    assert_eq!(canvas.vector_colors.first(), Some(&Color::Black));
    rule.host().set_system_dark_theme(true);
    rule.wait_for_idle();
    rule.on_node_with_text("Text style and color from dark theme").assert_exists();
    let palette = kompoz::graphics::material_color::palettes::DynamicTonalPalette::from_seed(Color::hex(0x2E7D32));
    rule.host().set_dynamic_tonal_palette(Some(palette));
    rule.wait_for_idle();
    rule.on_node_with_text("Text style and color from dark theme").assert_exists();
}

fn material_expressive_theme_sample() {
    let rule = KompozTestRule::new(MaterialExpressiveThemeSample);
    rule.on_node_with_text("increased true").assert_exists();
    rule.on_node_with_text("expressive true").assert_exists();
    let seen = std::rc::Rc::new(Cell::new(Color::Unspecified));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MyMaterialTheme(move || s.set(MaterialTheme::color_scheme().on_primary_container));
    }));
    assert_eq!(seen.get(), expressive_light_color_scheme().on_primary_container);
}

fn theme_color_and_text_style_samples() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    ThemeColorSample();
                    ThemeTextStyleSample();
                });
            },
        );
    }));
    assert!(rule.draw().rect_colors.contains(&light_color_scheme().primary));
    // (headline small: 24 sp in lines of 32)
    assert_eq!(rule.on_node_with_tag("headline").get_unclipped_bounds_in_root().height(), 32.0);
}

fn surface_samples() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    SurfaceSample();
                    ClickableSurfaceSample();
                    SelectableSurfaceSample();
                    ToggleableSurfaceSample();
                });
            },
        );
    }));
    rule.on_node_with_text("Text on Surface").assert_exists();
    rule.on_node_with_text("Clickable Surface. Count: 0").perform_click();
    rule.on_node_with_text("Clickable Surface. Count: 1").assert_exists();
    rule.on_node_with_text("Not Selected").perform_click();
    rule.on_node_with_text("Selected").assert_exists();
    rule.on_node_with_text("OFF").perform_click();
    rule.on_node_with_text("ON").assert_exists();
    // (the toggled surface in the surface variant)
    assert!(rule.draw().rect_colors.contains(&light_color_scheme().surface_variant));
}

// ---- (WP4b: icons) the extended icons through the generator's `Icons.<theme>.<name>` rule --------------------------

kompoz! {
    @Composable
    fun ExtendedIcons() {
        Column {
            Icon(Icons.Outlined.StarBorder, contentDescription = "yıldız")
            Icon(Icons.AutoMirrored.Filled.MenuOpen, contentDescription = "menü")
            Icon(imageVector = Icons.Default.Bookmarks, contentDescription = "imler")
            Icon(Icons.Filled.Add, contentDescription = "ekle")
            Icon(Icons.Rounded._10k, contentDescription = null)
        }
    }
}

fn extended_icons_come_through_the_icons_rule() {
    // (Kotlin's one `Icons.Filled` names core's icons and the extended set's alike: the umbrella's merged modules)
    let rule = KompozTestRule::new(ExtendedIcons);
    rule.on_node_with_content_description("yıldız").assert_exists();
    rule.on_node_with_content_description("menü").assert_exists();
    let ids = [
        Icons::Outlined::StarBorder().id(),
        Icons::AutoMirrored::Filled::MenuOpen().id(),
        Icons::Filled::Bookmarks().id(),
        Icons::Filled::Add().id(),
        Icons::Rounded::_10k().id(),
    ];
    assert_eq!(rule.draw().vectors, ids);
}

fn main() {
    common::run(&[
        // (WP4b: icons)
        ("extended_icons_come_through_the_icons_rule", extended_icons_come_through_the_icons_rule),
        ("theming_as_compose_writes_it", theming_as_compose_writes_it),
        ("material_theme_sample", material_theme_sample),
        ("material_expressive_theme_sample", material_expressive_theme_sample),
        ("theme_color_and_text_style_samples", theme_color_and_text_style_samples),
        ("surface_samples", surface_samples),
    ]);
}
