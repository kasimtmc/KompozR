//! The theme, as Compose's material3 has it: color schemes (Material's baseline, the system's dynamic colors and
//! the wallpaper's, content colors, tonal elevation), MaterialTheme and its pieces (type scale, shapes, motion), the
//! Expressive theme, the scrollbars' style, the selection colors, Material's texts by language. The C++ Kompoz's
//! area T by name (kompoz_tests.cpp), its numbers M15's where Compose 1.5 differs, and Compose's behaviour.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{Animatable, Spring, SpringSpec};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::prelude::*;
use kompoz_foundation::scrollbar::{LocalScrollbarStyle, ScrollbarStyle};
use kompoz_foundation::text::selection::{LocalTextSelectionColors, TextSelectionColors};
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_text::{FontFamily, FontWeight, TextStyle};
use kompoz_ui::input::PointerType;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

tests!(
    color_schemes_have_compose_1_4s_colors_and_content_colors,
    scrims_take_the_schemes_scrim_color,
    dynamic_color_schemes_map_the_system_tones,
    dynamic_color_comes_from_the_host,
    wallpaper_colors_are_a_source_of_their_own,
    material_theme_gives_scrollbars_its_on_surface,
    motion_schemes_set_how_material_components_move,
    material_type_styles_carry_line_heights_and_letter_spacing,
    text_selection_colors_come_from_the_theme_or_the_local,
    surfaces_are_tinted_by_their_elevation,
    expressive_themes_nest,
    typography_of_a_font_family,
    the_theme_is_one_local,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The grey of a CIELAB tone (the C++ test's `grayTone`).
fn gray_tone(tone: f32) -> Color {
    let fy = (tone + 16.0) / 116.0;
    let y = if fy > 6.0 / 29.0 { fy * fy * fy } else { 3.0 * (6.0 / 29.0) * (6.0 / 29.0) * (fy - 4.0 / 29.0) };
    let v = if y <= 0.0031308 { y * 12.92 } else { 1.055 * y.powf(1.0 / 2.4) - 0.055 };
    Color::new(v, v, v, 1.0)
}

/// The C++ test's palette: distinct accents (so the mapping shows), grey neutrals of their tones.
fn test_palette() -> DynamicTonalPalette {
    let tone = |i: usize| DynamicTonalPalette::TONES[i] as f32;
    DynamicTonalPalette {
        accent1: std::array::from_fn(|i| Color::new(tone(i) / 100.0, 0.2, 0.4, 1.0)),
        accent2: std::array::from_fn(|i| Color::new(0.3, tone(i) / 100.0, 0.1, 1.0)),
        accent3: std::array::from_fn(|i| Color::new(0.5, 0.1, tone(i) / 100.0, 1.0)),
        neutral1: std::array::from_fn(|i| gray_tone(tone(i))),
        neutral2: std::array::from_fn(|i| gray_tone(tone(i))),
    }
}

fn color_schemes_have_compose_1_4s_colors_and_content_colors() {
    let (light, dark) = (light_color_scheme(), dark_color_scheme());
    // The baseline's (Material's tokens, the same in 1.4 and 1.5).
    assert!(light.error_container == Color::hex(0xF9DEDC) && light.surface_dim == Color::hex(0xDED8E1));
    assert_eq!(light.scrim, Color::hex(0x000000));
    assert!(dark.surface_bright == Color::hex(0x3B383E) && dark.surface_container_lowest == Color::hex(0x0F0D13));
    assert_eq!(dark.surface_tint, dark.primary);
    assert!(light.primary_fixed == dark.primary_fixed && light.on_tertiary_fixed_variant == Color::hex(0x633B48));
    // From the system's palette: the fixed colors 90 / 80 / 10 / 30, light or dark.
    let p = test_palette();
    let (dl, dd) = (dynamic_light_color_scheme_of(&p), dynamic_dark_color_scheme_of(&p));
    assert!(dl.primary_fixed == p.accent1[3] && dd.primary_fixed_dim == p.accent1[4]);
    assert!(dd.on_secondary_fixed == p.accent2[11] && dl.on_tertiary_fixed_variant == p.accent3[9]);
    // (Compose 1.5's surfaces are the neutral variant's: the C++'s grey neutrals are the same here)
    assert!(dl.surface_variant == p.neutral2[3] && dd.surface_variant == p.neutral2[9]);
    assert!(dl.surface_container_lowest == p.neutral1[0] && dl.surface_tint == dl.primary);
    assert_eq!(dl.error_container, light.error_container);
    // contentColorFor: the matching content color, none for others.
    assert_eq!(light.content_color_for(light.primary), light.on_primary);
    assert_eq!(light.content_color_for(light.error_container), light.on_error_container);
    assert_eq!(light.content_color_for(light.surface_container_lowest), light.on_surface);
    assert_eq!(light.content_color_for(light.tertiary_fixed_dim), light.on_tertiary_fixed);
    // (the same as dark tertiary: that one first, as Compose's `when`)
    assert_eq!(dark.content_color_for(dark.tertiary_fixed_dim), dark.on_tertiary);
    assert!(light.content_color_for(Color::hex(0x123456)).is_unspecified());
    let seen = Rc::new(RefCell::new((Color::Unspecified, Color::Unspecified, Color::Unspecified)));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(&[LocalContentColor.provides(Color::hex(0x00FF00))], move || {
            let around = LocalContentColor.current();
            let on_primary = content_color_for(MaterialTheme::color_scheme().primary);
            // (the content color around it)
            let other = content_color_for(Color::hex(0x123456));
            *s.borrow_mut() = (around, on_primary, other);
        });
    }));
    let (around, on_primary, other) = *seen.borrow();
    assert!(on_primary == light_color_scheme().on_primary && other == around);
}

fn scrims_take_the_schemes_scrim_color() {
    // The C++'s scrim was a bottom sheet's (Material's sheets come later): its Scrim, in the scheme's scrim color at
    // 32%, across the window.
    let mut scheme = light_color_scheme();
    scheme.scrim = Color::hex(0xFF0000);
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            MaterialTheme(color_scheme = scheme, content = || Scrim(None));
        }),
        IntSize::new(400, 800),
    );
    let canvas = rule.draw();
    let red = canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| {
        near(r.width(), 400.0, 0.5)
            && c.red() == 1.0
            && c.green() == 0.0
            && c.blue() == 0.0
            && near(c.alpha(), 0.32, 0.01)
    });
    assert!(red, "{:?} {:?}", canvas.rects, canvas.rect_colors);
    assert_eq!(ScrimDefaults::color(), named_args!(Color::Black.copy(alpha = 0.32)));
}

fn dynamic_color_schemes_map_the_system_tones() {
    let p = test_palette();
    let (light, dark) = (dynamic_light_color_scheme_of(&p), dynamic_dark_color_scheme_of(&p));
    // Tones 40 / 100 / 90 / 10 of accent1 in light, 80 / 20 / 30 / 90 in dark (Compose's mapping).
    assert!(light.primary == p.accent1[8] && light.on_primary == p.accent1[0]);
    assert!(light.primary_container == p.accent1[3] && light.on_primary_container == p.accent1[11]);
    assert!(dark.primary == p.accent1[4] && dark.on_primary == p.accent1[10] && dark.primary_container == p.accent1[9]);
    assert!(
        light.secondary_container == p.accent2[3] && light.tertiary == p.accent3[8] && dark.tertiary == p.accent3[4]
    );
    // (not from the palette)
    assert!(light.error == light_color_scheme().error && dark.on_error == dark_color_scheme().on_error);
    // Tones the system doesn't give are made at that lightness (Compose's setLuminance): surface 98 between 100
    // and 95, the dark surface (6) between 10 and 0, still grey.
    let between = |c: Color, lighter: Color, darker: Color| c.red() < lighter.red() && c.red() > darker.red();
    assert!(between(light.surface, p.neutral2[0], p.neutral2[2]));
    // (CAM16's lightness: a grey's hue and its little chroma kept, the green — most of luminance — that tone's)
    assert!(near(light.surface.green() * 255.0, gray_tone(98.0).green() * 255.0, 1.0));
    assert!(between(dark.surface, p.neutral2[11], p.neutral2[12]));
    assert!(near(dark.surface.green() * 255.0, dark.surface.blue() * 255.0, 1.0));
    // (94)
    assert!(between(light.surface_container, p.neutral2[2], p.neutral2[3]));
    assert!(near(dark.surface_container.red() * 255.0, gray_tone(12.0).red() * 255.0, 1.0));
}

fn dynamic_color_comes_from_the_host() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = if is_dynamic_color_available() { "dinamik" } else { "varsayılan" };
        Text(state, modifier = Modifier.test_tag("state"));
        let scheme = if dynamic_light_color_scheme() == light_color_scheme() { "baseline" } else { "palette" };
        l.borrow_mut().push(scheme.to_string());
    }));
    rule.on_node_with_tag("state").assert_text_equals(&["varsayılan"]);
    assert_eq!(log.borrow().last().map(String::as_str), Some("baseline"));
    rule.host().set_dynamic_tonal_palette(Some(test_palette()));
    rule.wait_for_idle();
    rule.on_node_with_tag("state").assert_text_equals(&["dinamik"]);
    assert_eq!(log.borrow().last().map(String::as_str), Some("palette"));
    let runs = log.borrow().len();
    // (the same palette: nothing to do)
    rule.host().set_dynamic_tonal_palette(Some(test_palette()));
    rule.wait_for_idle();
    assert_eq!(log.borrow().len(), runs);
    rule.host().set_dynamic_tonal_palette(None);
    rule.wait_for_idle();
    rule.on_node_with_tag("state").assert_text_equals(&["varsayılan"]);
}

fn wallpaper_colors_are_a_source_of_their_own() {
    // Where the host has a wallpaper palette, the wallpaper schemes come from it, apart from the system's (accent)
    // palette and the app's own scheme; without one they are the baseline.
    let wall = DynamicTonalPalette::from_seed(Color::hex(0x2E7D32));
    let accent = DynamicTonalPalette::from_seed(Color::hex(0xC2185B));
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *s.borrow_mut() = Some((
            is_wallpaper_color_available(),
            wallpaper_light_color_scheme(),
            wallpaper_dark_color_scheme(),
            dynamic_light_color_scheme(),
        ));
    }));
    let now = || seen.borrow().unwrap();
    let (available, light, dark, _) = now();
    assert!(!available && light == light_color_scheme() && dark == dark_color_scheme());
    rule.host().set_wallpaper_tonal_palette(Some(wall));
    rule.host().set_dynamic_tonal_palette(Some(accent));
    rule.wait_for_idle();
    let (available, light, dark, system) = now();
    assert!(available && light == dynamic_light_color_scheme_of(&wall) && dark == dynamic_dark_color_scheme_of(&wall));
    assert!(system == dynamic_light_color_scheme_of(&accent) && light != system);
    rule.host().set_wallpaper_tonal_palette(None);
    rule.wait_for_idle();
    let (available, light, _, system) = now();
    assert!(!available && light == light_color_scheme() && system == dynamic_light_color_scheme_of(&accent));
}

fn material_theme_gives_scrollbars_its_on_surface() {
    let seen = Rc::new(RefCell::new(Vec::<ScrollbarStyle>::new()));
    let s = seen.clone();
    let dark_scheme = dark_color_scheme();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s1, s2) = (s.clone(), s.clone());
        MaterialTheme(
            color_scheme = light_color_scheme(),
            content = move || s1.borrow_mut().push(LocalScrollbarStyle.current()),
        );
        MaterialTheme(
            color_scheme = dark_scheme,
            content = move || s2.borrow_mut().push(LocalScrollbarStyle.current()),
        );
    }));
    let seen = seen.borrow();
    let (light, dark) = (&seen[0], &seen[1]);
    assert!(dark.hover_color == named_args!(dark_scheme.on_surface.copy(alpha = 0.5)));
    assert!(dark.unhover_color == named_args!(dark_scheme.on_surface.copy(alpha = 0.12)));
    assert!(light.hover_color == named_args!(light_color_scheme().on_surface.copy(alpha = 0.5)));
    assert!(near(light.thickness.0, 8.0, 0.001));
}

fn motion_schemes_set_how_material_components_move() {
    // The C++ morphed a shape's corner with the scheme's fast spatial spec; here a value animates by it, from 0 to
    // 20, in the standard theme and in the Expressive one, frame by frame.
    let pressed = mutable_state_of(false);
    let schemes = Rc::new(RefCell::new(Vec::<MotionScheme>::new()));
    let values: Rc<RefCell<(Vec<f32>, Vec<f32>)>> = Rc::default();
    let (p, s, v) = (pressed.clone(), schemes.clone(), values.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        s.borrow_mut().push(MaterialTheme::motion_scheme());
        let (p1, s1, v1, p2, s2, v2) = (p.clone(), s.clone(), v.clone(), p.clone(), s.clone(), v.clone());
        Column(move || {
            let (p1, s1, v1, p2, s2, v2) = (p1.clone(), s1.clone(), v1.clone(), p2.clone(), s2.clone(), v2.clone());
            MaterialTheme(
                color_scheme = dark_color_scheme(),
                content = move || {
                    s1.borrow_mut().push(MaterialTheme::motion_scheme());
                    let value = animated_by_fast_spatial(if p1.get() { 20.0 } else { 0.0 });
                    v1.borrow_mut().0.push(value);
                },
            );
            MaterialExpressiveTheme(
                content = move || {
                    s2.borrow_mut().push(MaterialTheme::motion_scheme());
                    let value = animated_by_fast_spatial(if p2.get() { 20.0 } else { 0.0 });
                    v2.borrow_mut().1.push(value);
                    // (keeps the one around)
                    let s3 = s2.clone();
                    MaterialTheme(
                        color_scheme = light_color_scheme(),
                        content = move || s3.borrow_mut().push(MaterialTheme::motion_scheme()),
                    );
                },
            );
        });
    }));
    {
        let schemes = schemes.borrow();
        assert!(schemes[0] == MotionScheme::standard() && schemes[1] == MotionScheme::standard());
        assert!(schemes[2] == MotionScheme::expressive() && schemes[3] == MotionScheme::expressive());
    }
    // (Compose 1.5's springs: standard 0.9 / 1400, Expressive 0.6 / 800)
    let fast = |scheme: &MotionScheme| scheme.fast_spatial_spec::<f32>();
    let spring_of = |d: f32, s: f32| -> Arc<dyn kompoz_animation::FiniteAnimationSpec<f32>> {
        Arc::new(AnyTypeSpec::<f32>::new(Arc::new(SpringSpec::<()>::new(d, s, None))))
    };
    assert!(*fast(&MotionScheme::expressive()) == *spring_of(0.6, 800.0));
    assert!(*fast(&MotionScheme::standard()) == *spring_of(0.9, 1400.0));
    assert!(*MotionScheme::standard().default_effects_spec::<f32>() == *spring_of(1.0, 1600.0));
    values.borrow_mut().0.clear();
    values.borrow_mut().1.clear();
    pressed.set(true);
    for _ in 0..90 {
        rule.advance_frame();
    }
    let (standard, expressive) = values.borrow().clone();
    let most = standard.iter().cloned().fold(f32::MIN, f32::max);
    let most_expressive = expressive.iter().cloned().fold(f32::MIN, f32::max);
    // The Expressive one overshoots (its fast spatial spring bounces); the standard one hardly.
    assert!(most < 20.2 && most_expressive > 21.0, "{most} {most_expressive}");
    assert!(near(*standard.last().unwrap(), 20.0, 0.05) && near(*expressive.last().unwrap(), 20.0, 0.05));
    let _ = Spring::StiffnessMedium;
}

/// A value animated to `target` by the theme's fast spatial spec, its value read in composition.
#[kompozable]
fn animated_by_fast_spatial(target: f32) -> f32 {
    let spec = MaterialTheme::motion_scheme().fast_spatial_spec::<f32>();
    let animatable = remember(|| Animatable(0.0));
    let a = animatable.clone();
    LaunchedEffect(target, async move {
        let _ = named_args!(a.animate_to(target, animation_spec = spec)).await;
    });
    animatable.value()
}

fn material_type_styles_carry_line_heights_and_letter_spacing() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            color_scheme = light_color_scheme(),
            content = || {
                Column(|| {
                    // bodyLarge: 16 sp, 24 sp lines, 0.5 sp apart
                    Text("bir", modifier = Modifier.test_tag("gövde"));
                    Text("iki\nsatır", modifier = Modifier.test_tag("iki"));
                    // 32 sp lines; 11 sp in 16
                    Text(
                        "başlık",
                        modifier = Modifier.test_tag("başlık"),
                        style = MaterialTheme::typography().headline_small,
                    );
                    Text(
                        "etiket",
                        modifier = Modifier.test_tag("etiket"),
                        style = MaterialTheme::typography().label_small,
                    );
                    // (foundation's text in the same style, and without its letter spacing)
                    let body = MaterialTheme::typography().body_large;
                    BasicText("bir", modifier = Modifier.test_tag("temel"), style = body.clone());
                    BasicText(
                        "bir",
                        modifier = Modifier.test_tag("sıkı"),
                        style = TextStyle { letter_spacing: 0.sp(), ..body },
                    );
                });
            },
        );
    }));
    let bounds = |tag: &str| rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    // Each line centred in its 24 (the fake metrics at 16 sp: ascent 12.8, line 20), nothing trimmed.
    assert_eq!((bounds("gövde").height(), bounds("iki").height()), (24.0, 48.0));
    assert_eq!((bounds("başlık").height(), bounds("etiket").height()), (32.0, 16.0));
    // (3 letters of 8, the letter spacing between them: as the style's own text is, wider than without it)
    assert_eq!(bounds("gövde").width(), bounds("temel").width());
    assert_eq!(bounds("sıkı").width(), 24.0);
    assert!(bounds("gövde").width() > 24.0);
    let canvas = rule.draw();
    let at = |t: &str| canvas.baselines[canvas.texts.iter().position(|x| x == t).unwrap()].y;
    assert!(near(at("bir") - bounds("gövde").top, 14.8, 0.01), "{}", at("bir"));
}

fn text_selection_colors_come_from_the_theme_or_the_local() {
    // C++ 11568's MaterialTheme part (its local part: kompoz-foundation's selection tests): the highlight takes the
    // theme's primary at 40%, the light scheme's or a dark theme's, or what LocalTextSelectionColors provides.
    let own = TextSelectionColors::new(Color::new(1.0, 0.0, 0.0, 1.0), Color::new(0.0, 1.0, 0.0, 0.5));
    let pick = mutable_state_of(0);
    let pk = pick.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let inside = || {
            SelectionContainer(content = || Text("seçilecek metin", modifier = Modifier.test_tag("metin")));
        };
        match pk.get() {
            0 => MaterialTheme(content = inside),
            1 => MaterialTheme(color_scheme = dark_color_scheme(), content = inside),
            _ => KompozitionLocalProvider(&[LocalTextSelectionColors.provides(own)], inside),
        }
    }));
    let highlighted = |color: Color| {
        let t = rule.on_node_with_tag("metin").fetch_semantics_node().bounds_in_root();
        let p = Offset::new(t.left + 2.0, t.center().y);
        rule.host().pointer_down(p, -1, 0, PointerType::Touch);
        rule.advance_time_by(500);
        rule.host().pointer_up(p, -1, 0);
        rule.wait_for_idle();
        let canvas = rule.draw();
        canvas.path_colors.contains(&color) || canvas.rect_colors.contains(&color)
    };
    assert!(highlighted(named_args!(light_color_scheme().primary.copy(alpha = 0.4))));
    pick.set(1);
    rule.wait_for_idle();
    assert!(highlighted(named_args!(dark_color_scheme().primary.copy(alpha = 0.4))));
    pick.set(2);
    rule.wait_for_idle();
    assert!(highlighted(own.background_color));
}

// (C++ theme_pieces_take_compose_named_arguments: in both syntaxes in crates/kompoz/tests/named_forms.rs)

fn surfaces_are_tinted_by_their_elevation() {
    // Compose's surfaceColorAtElevation: none at 0, (4.5 ln(e + 1) + 2)% of the tint over the surface; a Surface's
    // background its scheme's surface at the elevations around it added, unless tonal elevation is off.
    let scheme = light_color_scheme();
    assert_eq!(scheme.surface_color_at_elevation(Dp(0.0)), scheme.surface);
    let alpha = (4.5 * (3.0f32 + 1.0).ln() + 2.0) / 100.0;
    let tinted = named_args!(scheme.surface_tint.copy(alpha = alpha)).composite_over(scheme.surface);
    assert_eq!(scheme.surface_color_at_elevation(Dp(3.0)), tinted);
    let seen = Rc::new(RefCell::new(Vec::<Dp>::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                let s = s.clone();
                Column(move || {
                    let s = s.clone();
                    Surface(
                        modifier = Modifier.size(20.dp()),
                        tonal_elevation = Dp(1.0),
                        content = move || {
                            let s = s.clone();
                            Surface(
                                modifier = Modifier.size(10.dp()),
                                tonal_elevation = Dp(2.0),
                                content = move || {
                                    s.borrow_mut().push(LocalAbsoluteTonalElevation.current());
                                },
                            );
                        },
                    );
                    KompozitionLocalProvider(&[LocalTonalElevationEnabled.provides(false)], || {
                        Surface(modifier = Modifier.size(5.dp()), tonal_elevation = Dp(3.0), content = || {});
                    });
                });
            },
        );
    }));
    assert_eq!(seen.borrow().last(), Some(&Dp(3.0)));
    let canvas = rule.draw();
    let colors = &canvas.rect_colors;
    assert!(colors.contains(&scheme.surface_color_at_elevation(Dp(1.0))), "{colors:?}");
    assert!(colors.contains(&scheme.surface_color_at_elevation(Dp(3.0))));
    // (tonal elevation off: the surface's own color)
    assert!(colors.contains(&scheme.surface));
}

fn expressive_themes_nest() {
    // Compose's MaterialExpressiveTheme: the first gives Expressive's light scheme and motion and says it is one; one
    // inside it keeps the scheme around unless given.
    let seen = Rc::new(RefCell::new(Vec::<(ColorScheme, MotionScheme, bool)>::new()));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialExpressiveTheme(
            content = move || {
                s.borrow_mut().push((
                    MaterialTheme::color_scheme(),
                    MaterialTheme::motion_scheme(),
                    LocalUsingExpressiveTheme.current(),
                ));
                let s = s.clone();
                MaterialExpressiveTheme(
                    color_scheme = Some(dark_color_scheme()),
                    content = move || {
                        s.borrow_mut().push((
                            MaterialTheme::color_scheme(),
                            MaterialTheme::motion_scheme(),
                            LocalUsingExpressiveTheme.current(),
                        ));
                    },
                );
            },
        );
    }));
    let seen = seen.borrow();
    assert!(seen[0].0 == expressive_light_color_scheme() && seen[0].1 == MotionScheme::expressive() && seen[0].2);
    assert_eq!(expressive_light_color_scheme().on_primary_container, Color::hex(0x4F378B));
    assert!(seen[1].0 == dark_color_scheme() && seen[1].1 == MotionScheme::expressive() && seen[1].2);
    assert!(!LocalUsingExpressiveTheme.current());
}

fn typography_of_a_font_family() {
    // Compose's Typography(fontFamily = …): every style in it, those given as they are; Material's type scale's
    // numbers (Compose 1.5's tokens).
    let given = TextStyle { font_size: 40.sp(), ..TextStyle::Default };
    let ty = named_args!(typography_with_font_family(FontFamily::Monospace, title_large = Some(given.clone())));
    assert_eq!(ty.body_large.font_family, Some(FontFamily::Monospace));
    assert_eq!(ty.display_small_emphasized.font_family, Some(FontFamily::Monospace));
    assert_eq!(ty.title_large, given);
    let m = Typography();
    assert_eq!(
        (m.body_large.font_size, m.body_large.line_height, m.body_large.letter_spacing),
        (16.sp(), 24.sp(), 0.5.sp())
    );
    assert_eq!((m.display_large.font_size, m.display_large.letter_spacing), (57.sp(), (-0.2).sp()));
    assert_eq!(
        (m.label_large.font_weight, m.label_large.font_family.clone()),
        (Some(FontWeight::Medium), Some(FontFamily::SansSerif))
    );
    assert_eq!(m.title_large_emphasized.font_weight, Some(FontWeight::Medium));
    assert!(format!("{m}").starts_with("Typography(displayLarge="));
}

fn the_theme_is_one_local() {
    // Compose 1.5's MaterialTheme.LocalMaterialTheme: the theme's values, equal by their pieces.
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            color_scheme = dark_color_scheme(),
            content = move || {
                *s.borrow_mut() = Some(MaterialTheme::LocalMaterialTheme().current());
            },
        );
    }));
    let values = seen.borrow().clone().unwrap();
    assert_eq!(values, MaterialThemeValues::new(dark_color_scheme(), Typography(), Shapes(), MotionScheme::standard()));
    assert_eq!(MaterialTheme::LocalMaterialTheme().current(), MaterialThemeValues::default());
}
