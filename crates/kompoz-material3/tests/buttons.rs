//! Material's buttons and floating action buttons as Compose's material3 (1.5.0-alpha17) has them: the C++
//! Kompoz's tests by name (kompoz_tests.cpp, with M15's values where the C++ followed 1.4 or its own layout),
//! Compose's behaviour (colors per state, elevations animating with interactions, the Expressive sizes and shapes,
//! the shape morphing on press, the extended FABs' expanding and collapsing, showing and hiding a FAB), and the M15
//! samples of ButtonSamples.kt and FloatingActionButtonSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{AnimationSpec, LinearEasing, TweenSpec};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_test::RecordingCanvas;
use kompoz_text::TextStyle;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    buttons_as_compose_1_4,
    disabled_button_ignores_clicks,
    fabs_as_compose_1_4,
    expressive_fabs_come_in_sizes_and_extended_ones_narrow_to_their_icon,
    button_colors_follow_the_scheme_and_the_state,
    button_colors_copy_keeps_what_is_unspecified,
    buttons_are_buttons_to_accessibility_in_label_large,
    button_defaults_follow_the_tokens,
    expressive_button_sizes_pick_their_paddings_icons_and_styles,
    button_shapes_morph_as_they_are_pressed,
    button_elevation_follows_focus_and_snaps_when_disabled,
    fab_sizes_shapes_and_colors,
    fab_elevations_follow_interactions_and_their_defaults,
    extended_fabs_of_content_take_their_sizes,
    extended_fab_of_text_and_icon_collapses_to_its_icon,
    animate_floating_action_button_shows_and_hides,
    button_samples,
    floating_action_button_samples,
);

/// A shared shape equal to `shape`.
fn same(a: &Arc<dyn Shape>, shape: impl Shape) -> bool {
    **a == *(&shape as &dyn Shape)
}

/// A 100 ms linear tween.
fn tween100() -> Option<Arc<dyn AnimationSpec<f32>>> {
    Some(Arc::new(TweenSpec::<f32>::new(100, 0, LinearEasing)))
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// A node's bounds (its own, not its minimum interactive size's), in the unmerged tree (a button's content too).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

/// The opaque fill (or else the line) centered on `b`: a button's container in its (larger) touch area.
fn container(canvas: &RecordingCanvas, b: Rect) -> Option<Rect> {
    let centered = |r: &Rect| near(r.center().y, b.center().y, 0.1) && near(r.center().x, b.center().x, 0.1);
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .find(|(r, c)| c.alpha() > 0.5 && centered(r))
        .map(|(r, _)| *r)
        .or_else(|| canvas.strokes.iter().find(|r| centered(r)).copied())
}

/// The highest elevation shadow drawn around `r`'s center.
fn shadow_at(canvas: &RecordingCanvas, r: Rect, tolerance_y: f32) -> f32 {
    canvas
        .elevation_shadows
        .iter()
        .filter(|e| {
            near(e.bounds.center().x, r.center().x, 2.0) && near(e.bounds.center().y, r.center().y, tolerance_y)
        })
        .map(|e| e.elevation())
        .fold(0.0, f32::max)
}

fn buttons_as_compose_1_4() {
    let rtl = mutable_state_of(false);
    let r = rtl.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let r = r.clone();
        Column(move || {
            // 8 dp above and below (46 for a 30 dp label), at least 58 wide (a TextButton too).
            Button(
                || {},
                modifier = Modifier.test_tag("dolu"),
                content = || {
                    Box(modifier = Modifier.size_wh(4.dp(), 30.dp()));
                },
            );
            TextButton(
                || {},
                modifier = Modifier.test_tag("metin"),
                content = || {
                    Box(modifier = Modifier.size_wh(4.dp(), 20.dp()));
                },
            );
            // An icon's padding: 16 dp before, 24 after; the start mirrored right to left.
            let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], || {
                Button(
                    || {},
                    modifier = Modifier.test_tag("simgeli"),
                    content_padding = ButtonDefaults::ButtonWithIconContentPadding(),
                    content = || {
                        Box(modifier = Modifier.size(20.dp()).test_tag("simge"));
                    },
                );
                // Squeezed: M15's padding keeps its sides, the content takes what is left (the C++ gave way by the
                // sides' shares, 16:24).
                Button(
                    || {},
                    modifier = Modifier.width(50.dp()).test_tag("sıkışık"),
                    content_padding = ButtonDefaults::ButtonWithIconContentPadding(),
                    content = || {
                        Box(modifier = Modifier
                            .width_in(0.dp(), 40.dp())
                            .fill_max_width()
                            .height(20.dp())
                            .test_tag("sıkışık-içi"));
                    },
                );
            });
            OutlinedButton(
                || {},
                modifier = Modifier.test_tag("çizgili"),
                content = || {
                    Box(modifier = Modifier.size_wh(4.dp(), 20.dp()));
                },
            );
            OutlinedButton(
                || {},
                modifier = Modifier.test_tag("kapalı"),
                enabled = false,
                content = || {
                    Box(modifier = Modifier.size_wh(4.dp(), 20.dp()));
                },
            );
            ElevatedButton(
                || {},
                modifier = Modifier.test_tag("yüksek"),
                content = || {
                    Box(modifier = Modifier.size_wh(80.dp(), 20.dp()));
                },
            );
        });
    }));
    let cs = light_color_scheme();
    let mut canvas = rule.draw();
    let filled = container(&canvas, bounds(&rule, "dolu")).expect("the filled container");
    assert!(near(filled.height(), 46.0, 0.01) && near(filled.width(), 58.0, 0.01), "{filled:?}");
    assert!(near(bounds(&rule, "metin").width(), 58.0, 0.01));
    let iconed = container(&canvas, bounds(&rule, "simgeli")).expect("the icon button's container");
    let icon = bounds(&rule, "simge");
    assert!(near(iconed.width(), 60.0, 0.01) && near(icon.left, iconed.left + 16.0, 0.01), "{iconed:?} {icon:?}");
    let squeezed = container(&canvas, bounds(&rule, "sıkışık")).expect("the squeezed container");
    let inside = bounds(&rule, "sıkışık-içi");
    assert!(near(inside.width(), 10.0, 0.01) && near(inside.left, squeezed.left + 16.0, 0.01), "{inside:?}");
    rtl.set(true);
    rule.wait_for_idle();
    canvas = rule.draw();
    let iconed = container(&canvas, bounds(&rule, "simgeli")).expect("the icon button's container");
    let icon = bounds(&rule, "simge");
    assert!(near(icon.right, iconed.right - 16.0, 0.01), "{iconed:?} {icon:?}");
    // Outlined: outlineVariant, a tenth of it disabled.
    let stroke_of = |canvas: &RecordingCanvas, tag: &str| {
        let b = bounds(&rule, tag);
        canvas
            .strokes
            .iter()
            .zip(&canvas.stroke_colors)
            .find(|(s, _)| near(s.center().y, b.center().y, 0.1))
            .map(|(_, c)| *c)
    };
    assert_eq!(stroke_of(&canvas, "çizgili"), Some(cs.outline_variant));
    assert_eq!(stroke_of(&canvas, "kapalı"), Some(named_args!(cs.outline_variant.copy(alpha = 0.1))));
    // Elevations: 1 dp for the elevated one, none for the filled one; 3 and 1 hovered.
    let spread = |canvas: &RecordingCanvas, tag: &str| {
        let r = container(canvas, bounds(&rule, tag)).expect("a container");
        shadow_at(canvas, r, 2.0)
    };
    assert!(near(spread(&canvas, "yüksek"), 1.0, 0.01) && near(spread(&canvas, "dolu"), 0.0, 0.01));
    rule.host().pointer_move(bounds(&rule, "yüksek").center(), -1, 0);
    rule.advance_time_by(300);
    canvas = rule.draw();
    assert!(near(spread(&canvas, "yüksek"), 3.0, 0.01), "{}", spread(&canvas, "yüksek"));
    rule.host().pointer_move(bounds(&rule, "dolu").center(), -1, 0);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    canvas = rule.draw();
    let easing = spread(&canvas, "yüksek");
    assert!(easing > 1.05 && easing < 2.95, "on its way back down: {easing}");
    rule.advance_time_by(300);
    canvas = rule.draw();
    assert!(near(spread(&canvas, "yüksek"), 1.0, 0.01) && near(spread(&canvas, "dolu"), 1.0, 0.01));
    // Pressed: back down, though hovered.
    let at = bounds(&rule, "dolu").center();
    rule.host().pointer_down(at, -1, 0, PointerType::Mouse);
    rule.advance_time_by(300);
    canvas = rule.draw();
    assert!(near(spread(&canvas, "dolu"), 0.0, 0.01));
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

fn disabled_button_ignores_clicks() {
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Button(move || c.set(c.get() + 1), enabled = false, content = || Text("Go"));
    }));
    rule.on_node_with_text("Go").perform_click();
    assert_eq!(clicks.get(), 0);
}

fn fabs_as_compose_1_4() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            FloatingActionButton(
                || {},
                modifier = Modifier.test_tag("fab"),
                content = || {
                    VectorIcon(Icons::Filled::Add(), Some("Ekle".to_string()));
                },
            );
            TextIconExtendedFloatingActionButton(
                || Text("Yeni"),
                || {
                    VectorIcon(
                        Icons::Filled::Add(),
                        Some("Yeni".to_string()),
                        modifier = Modifier.test_tag("geniş-simge"),
                    )
                },
                on_click = || {},
                modifier = Modifier.test_tag("geniş"),
            );
            TextIconExtendedFloatingActionButton(
                || Text(""),
                || VectorIcon(Icons::Filled::Add(), None),
                on_click = || {},
                modifier = Modifier.test_tag("kısa"),
            );
            Text("Yeni", modifier = Modifier.test_tag("ölçü"), style = MaterialTheme::typography().label_large);
        });
    }));
    // 16 dp before the icon, 12 between, 20 after the label; at least 80 wide.
    let wide = bounds(&rule, "geniş");
    let icon = bounds(&rule, "geniş-simge");
    let label = bounds(&rule, "ölçü").width();
    assert!(near(icon.left, wide.left + 16.0, 0.01), "{icon:?} {wide:?}");
    assert!(near(wide.width(), (16.0 + 24.0 + 12.0 + label + 20.0).ceil(), 1.0), "{wide:?} {label}");
    assert!(near(bounds(&rule, "kısa").width(), 80.0, 0.01));
    // 6 dp up, 8 hovered, 6 again pressed.
    let spread = || {
        let canvas = rule.draw();
        shadow_at(&canvas, bounds(&rule, "fab"), 4.0)
    };
    assert!(near(spread(), 6.0, 0.01), "{}", spread());
    let at = bounds(&rule, "fab").center();
    rule.host().pointer_move(at, -1, 0);
    rule.advance_time_by(300);
    assert!(near(spread(), 8.0, 0.01), "{}", spread());
    rule.host().pointer_down(at, -1, 0, PointerType::Mouse);
    rule.advance_time_by(300);
    assert!(near(spread(), 6.0, 0.01), "{}", spread());
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

fn expressive_fabs_come_in_sizes_and_extended_ones_narrow_to_their_icon() {
    let expanded = mutable_state_of(true);
    let clicks = Rc::new(Cell::new(0));
    let (e, c) = (expanded.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, c) = (e.clone(), c.clone());
        Column(move || {
            let c1 = c.clone();
            MediumFloatingActionButton(
                move || c1.set(c1.get() + 1),
                modifier = Modifier.test_tag("orta"),
                content = || {
                    Box(modifier = Modifier.size(28.dp()));
                },
            );
            let c2 = c.clone();
            TextIconSmallExtendedFloatingActionButton(
                || Text("Yeni"),
                || VectorIcon(Icons::Filled::Add(), Some("Yeni".to_string())),
                on_click = move || c2.set(c2.get() + 1),
                modifier = Modifier.test_tag("küçük"),
                expanded = e.get(),
            );
            TextIconMediumExtendedFloatingActionButton(
                || Text("Yaz"),
                || {
                    VectorIcon(
                        Icons::Filled::Edit(),
                        None,
                        modifier = Modifier.size(FloatingActionButtonDefaults::MediumIconSize),
                    )
                },
                on_click = || {},
                modifier = Modifier.test_tag("orta-uzun"),
                expanded = e.get(),
            );
            TextIconLargeExtendedFloatingActionButton(
                || Text("Yaz"),
                || {
                    VectorIcon(
                        Icons::Filled::Edit(),
                        None,
                        modifier = Modifier.size(FloatingActionButtonDefaults::LargeIconSize).test_tag("büyük-simge"),
                    )
                },
                on_click = || {},
                modifier = Modifier.test_tag("büyük"),
                expanded = e.get(),
            );
            Text("Yeni", modifier = Modifier.test_tag("yeni-ölçü"), style = MaterialTheme::typography().title_medium);
            Text("Yaz", modifier = Modifier.test_tag("yaz-ölçü"), style = MaterialTheme::typography().title_large);
            Text("Yaz", modifier = Modifier.test_tag("büyük-ölçü"), style = MaterialTheme::typography().headline_small);
        });
    }));
    let cs = light_color_scheme();
    let fill = |b: Rect| {
        let canvas = rule.draw();
        canvas
            .rects
            .iter()
            .zip(canvas.rect_radii.iter().zip(&canvas.rect_colors))
            .find(|(r, _)| near(r.left, b.left, 0.5) && near(r.top, b.top, 0.5) && near(r.width(), b.width(), 0.5))
            .map(|(_, (radius, color))| (*radius, *color))
    };
    let orta = bounds(&rule, "orta");
    assert!(orta.left == 0.0 && orta.top == 0.0 && orta.width() == 80.0 && orta.height() == 80.0, "{orta:?}");
    assert_eq!(fill(orta), Some((20.0, cs.primary_container)));
    // Expanded: padding, icon, gap, label, padding (labels titleMedium / titleLarge / headlineSmall).
    let width_of = |tag: &str| bounds(&rule, tag).width().ceil();
    let small = bounds(&rule, "küçük");
    assert!(near(small.top, 80.0, 0.01) && near(small.height(), 56.0, 0.01), "{small:?}");
    assert!(near(small.width(), 16.0 + 24.0 + 8.0 + width_of("yeni-ölçü") + 16.0, 1.0), "{small:?}");
    assert_eq!(fill(small).map(|f| f.0), Some(16.0));
    let medium = bounds(&rule, "orta-uzun");
    assert!(near(medium.top, 136.0, 0.01) && near(medium.height(), 80.0, 0.01), "{medium:?}");
    assert!(near(medium.width(), 26.0 + 28.0 + 12.0 + width_of("yaz-ölçü") + 26.0, 1.0), "{medium:?}");
    assert_eq!(fill(medium).map(|f| f.0), Some(20.0));
    let large = bounds(&rule, "büyük");
    assert!(near(large.top, 216.0, 0.01) && near(large.height(), 96.0, 0.01), "{large:?}");
    assert!(near(large.width(), 28.0 + 36.0 + 16.0 + width_of("büyük-ölçü") + 28.0, 1.0), "{large:?}");
    assert_eq!(fill(large).map(|f| f.0), Some(28.0));
    // (the measuring Text draws "Yeni" too: one more in the FAB)
    assert_eq!(rule.drawn_texts().iter().filter(|t| *t == "Yeni").count(), 2);
    rule.on_node_with_content_description("Yeni").assert_has_click_action();

    expanded.set(false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let narrowing = bounds(&rule, "küçük").width();
    assert!(narrowing < small.width() - 1.0 && narrowing > 57.0, "on its way: {narrowing}");
    rule.wait_for_idle();
    rule.advance_time_by(2000);
    // Collapsed: square FABs around their icons, the labels gone.
    let (small, medium, large) = (bounds(&rule, "küçük"), bounds(&rule, "orta-uzun"), bounds(&rule, "büyük"));
    assert!(small.width() == 56.0 && small.height() == 56.0, "{small:?}");
    assert!(medium.width() == 80.0 && medium.height() == 80.0 && medium.top == 136.0, "{medium:?}");
    assert!(large.width() == 96.0 && large.height() == 96.0 && large.top == 216.0, "{large:?}");
    // (M15's row starts its icon after the start padding — the C++ centered it, 30 dp in)
    let icon = bounds(&rule, "büyük-simge");
    assert!(near(icon.left, 28.0, 0.01) && near(icon.top, 216.0 + 30.0, 0.01), "{icon:?}");
    assert_eq!(rule.drawn_texts().iter().filter(|t| *t == "Yeni").count(), 1);
    rule.on_node_with_tag("küçük").perform_click();
    rule.on_node_with_tag("orta").perform_click();
    assert_eq!(clicks.get(), 2);
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// What each kind of button gives its content: its color, and the colors of the defaults.
fn button_colors_follow_the_scheme_and_the_state() {
    let seen: Rc<RefCell<Vec<(String, Color)>>> = Rc::default();
    let colors: Rc<RefCell<Vec<ButtonColors>>> = Rc::default();
    let (s, c) = (seen.clone(), colors.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, c) = (s.clone(), c.clone());
        MaterialTheme(
            content = move || {
                *c.borrow_mut() = vec![
                    ButtonDefaults.button_colors(),
                    ButtonDefaults.elevated_button_colors(),
                    ButtonDefaults.filled_tonal_button_colors(),
                    ButtonDefaults.outlined_button_colors(),
                    ButtonDefaults.text_button_colors(),
                ];
                let record = {
                    let s = s.clone();
                    move |name: &'static str| {
                        let s = s.clone();
                        move || s.borrow_mut().push((name.to_string(), LocalContentColor.current()))
                    }
                };
                Column(move || {
                    let r = record.clone();
                    Button(|| {}, modifier = Modifier.test_tag("dolu"), content = r("dolu"));
                    Button(
                        || {},
                        enabled = false,
                        modifier = Modifier.test_tag("dolu-kapalı"),
                        content = r("dolu-kapalı"),
                    );
                    ElevatedButton(|| {}, content = r("yüksek"));
                    FilledTonalButton(|| {}, modifier = Modifier.test_tag("tonlu"), content = r("tonlu"));
                    OutlinedButton(|| {}, content = r("çizgili"));
                    OutlinedButton(|| {}, enabled = false, content = r("çizgili-kapalı"));
                    TextButton(|| {}, content = r("metin"));
                    Button(
                        || {},
                        colors = named_args!(ButtonDefaults.button_colors(container_color = Color::Red)),
                        modifier = Modifier.test_tag("kırmızı"),
                        content = r("kırmızı"),
                    );
                });
            },
        );
    }));
    let cs = light_color_scheme();
    let colors = colors.borrow().clone();
    let disabled = |c: Color, alpha: f32| named_args!(c.copy(alpha = alpha));
    assert_eq!(
        colors[0],
        ButtonColors::new(
            cs.primary,
            cs.on_primary,
            disabled(cs.on_surface, 0.1),
            disabled(cs.on_surface_variant, 0.38)
        )
    );
    assert_eq!((colors[1].container_color, colors[1].content_color), (cs.surface_container_low, cs.primary));
    assert_eq!(
        (colors[2].container_color, colors[2].content_color),
        (cs.secondary_container, cs.on_secondary_container)
    );
    assert_eq!(
        (colors[3].container_color, colors[3].disabled_container_color),
        (Color::Transparent, Color::Transparent)
    );
    assert_eq!((colors[4].container_color, colors[4].content_color), (Color::Transparent, cs.primary));
    let seen = seen.borrow().clone();
    let of = |name: &str| seen.iter().rev().find(|(n, _)| n == name).map(|(_, c)| *c);
    assert_eq!(of("dolu"), Some(cs.on_primary));
    assert_eq!(of("dolu-kapalı"), Some(colors[0].disabled_content_color));
    assert_eq!(of("yüksek"), Some(cs.primary));
    assert_eq!(of("tonlu"), Some(cs.on_secondary_container));
    assert_eq!(of("çizgili"), Some(colors[3].content_color));
    assert_eq!(of("çizgili-kapalı"), Some(colors[3].disabled_content_color));
    assert_eq!(of("metin"), Some(cs.primary));
    assert_eq!(of("kırmızı"), Some(cs.on_primary));
    // The containers drawn: primary, the disabled one, the tonal one, the red one.
    let canvas = rule.draw();
    let fill_of = |tag: &str| {
        let b = bounds(&rule, tag);
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .find(|(r, _)| near(r.center().y, b.center().y, 0.1) && near(r.center().x, b.center().x, 0.1))
            .map(|(_, c)| *c)
    };
    assert_eq!(fill_of("dolu"), Some(cs.primary));
    assert_eq!(fill_of("dolu-kapalı"), Some(colors[0].disabled_container_color));
    assert_eq!(fill_of("tonlu"), Some(cs.secondary_container));
    assert_eq!(fill_of("kırmızı"), Some(Color::Red));
}

fn button_colors_copy_keeps_what_is_unspecified() {
    let colors = ButtonColors::new(Color::Red, Color::Green, Color::Blue, Color::Black);
    assert_eq!(named_args!(colors.copy(content_color = Color::Unspecified)), colors);
    assert_eq!(named_args!(colors.copy(content_color = Color::White)).content_color, Color::White);
    assert_eq!(named_args!(colors.copy(container_color = Color::White)).disabled_content_color, Color::Black);
    // (Kotlin's properties)
    assert_eq!(colors.container_color(), Color::Red);
    let shapes = ButtonShapes::new(Arc::new(CircleShape), Arc::new(RoundedCornerShape(8.dp())));
    assert_eq!(shapes, ButtonShapes::new(Arc::new(CircleShape), Arc::new(RoundedCornerShape(8.dp()))));
    assert_eq!(named_args!(shapes.copy(pressed_shape = None)), shapes);
    let square = named_args!(shapes.copy(shape = Some(Arc::new(RectangleShape))));
    assert!(square != shapes && same(&square.shape, RectangleShape) && *square.pressed_shape == *shapes.pressed_shape);
}

fn buttons_are_buttons_to_accessibility_in_label_large() {
    let style = Rc::new(RefCell::new(None));
    let st = style.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let st = st.clone();
        MaterialTheme(
            content = move || {
                let st = st.clone();
                Button(
                    || {},
                    modifier = Modifier.test_tag("düğme"),
                    content = move || {
                        *st.borrow_mut() = Some(LocalTextStyle.current());
                        Text("Kaydet");
                    },
                );
            },
        );
    }));
    let node = rule.on_node_with_tag("düğme").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button));
    rule.on_node_with_tag("düğme").assert_text_equals(&["Kaydet"]).assert_has_click_action();
    let style: TextStyle = style.borrow().clone().expect("the label's style");
    assert_eq!(style.font_size, Typography::default().label_large.font_size);
    // (the container's bounds, 40 dp; its touch area the minimum interactive size)
    assert!(near(bounds(&rule, "düğme").height(), 40.0, 0.01));
}

fn button_defaults_follow_the_tokens() {
    assert_eq!(ButtonDefaults::MinWidth, Dp(58.0));
    assert_eq!(ButtonDefaults::MinHeight(), Dp(40.0));
    assert_eq!(
        [
            ButtonDefaults::ExtraSmallContainerHeight,
            ButtonDefaults::MediumContainerHeight(),
            ButtonDefaults::LargeContainerHeight(),
            ButtonDefaults::ExtraLargeContainerHeight
        ],
        [Dp(32.0), Dp(56.0), Dp(96.0), Dp(136.0)]
    );
    assert_eq!(ButtonDefaults::IconSize, Dp(18.0));
    assert_eq!(
        [
            ButtonDefaults::ExtraSmallIconSize,
            ButtonDefaults::SmallIconSize,
            ButtonDefaults::MediumIconSize,
            ButtonDefaults::LargeIconSize(),
            ButtonDefaults::ExtraLargeIconSize
        ],
        [Dp(20.0), Dp(20.0), Dp(24.0), Dp(32.0), Dp(40.0)]
    );
    assert_eq!(
        [
            ButtonDefaults::ExtraSmallIconSpacing,
            ButtonDefaults::IconSpacing,
            ButtonDefaults::MediumIconSpacing,
            ButtonDefaults::LargeIconSpacing(),
            ButtonDefaults::ExtraLargeIconSpacing
        ],
        [Dp(4.0), Dp(8.0), Dp(8.0), Dp(12.0), Dp(16.0)]
    );
    let sides = |p: PaddingValues| {
        [
            p.calculate_left_padding(LayoutDirection::Ltr),
            p.calculate_top_padding(),
            p.calculate_right_padding(LayoutDirection::Ltr),
            p.calculate_bottom_padding(),
        ]
    };
    assert_eq!(sides(ButtonDefaults::ContentPadding()), [Dp(24.0), Dp(8.0), Dp(24.0), Dp(8.0)]);
    assert_eq!(sides(ButtonDefaults::ButtonWithIconContentPadding()), [Dp(16.0), Dp(8.0), Dp(24.0), Dp(8.0)]);
    assert_eq!(sides(ButtonDefaults::TextButtonContentPadding()), [Dp(12.0), Dp(8.0), Dp(12.0), Dp(8.0)]);
    assert_eq!(sides(ButtonDefaults::TextButtonWithIconContentPadding()), [Dp(12.0), Dp(8.0), Dp(16.0), Dp(8.0)]);
    assert_eq!(sides(ButtonDefaults::ExtraSmallContentPadding()), [Dp(12.0), Dp(6.0), Dp(12.0), Dp(6.0)]);
    assert_eq!(sides(ButtonDefaults::SmallContentPadding()), [Dp(16.0), Dp(10.0), Dp(16.0), Dp(10.0)]);
    assert_eq!(sides(ButtonDefaults::MediumContentPadding()), [Dp(24.0), Dp(16.0), Dp(24.0), Dp(16.0)]);
    assert_eq!(sides(ButtonDefaults::LargeContentPadding()), [Dp(48.0), Dp(32.0), Dp(48.0), Dp(32.0)]);
    assert_eq!(sides(ButtonDefaults::ExtraLargeContentPadding()), [Dp(64.0), Dp(48.0), Dp(64.0), Dp(48.0)]);
    #[allow(deprecated)]
    {
        assert_eq!(sides(ButtonDefaults::SmallButtonContentPadding()), [Dp(16.0), Dp(8.0), Dp(16.0), Dp(8.0)]);
        assert_eq!(
            sides(ButtonDefaults.content_padding_for_height(Dp(56.0))),
            sides(ButtonDefaults::MediumContentPadding())
        );
    }
    // By the height: the size it is at least.
    let d = ButtonDefaults;
    assert_eq!(sides(d.content_padding_for(Dp(30.0))), sides(ButtonDefaults::ExtraSmallContentPadding()));
    assert_eq!(sides(d.content_padding_for(Dp(40.0))), sides(ButtonDefaults::SmallContentPadding()));
    assert_eq!(
        sides(named_args!(d.content_padding_for(Dp(56.0), has_start_icon = true))),
        [Dp(24.0), Dp(16.0), Dp(24.0), Dp(16.0)]
    );
    assert_eq!(sides(d.content_padding_for(Dp(100.0))), sides(ButtonDefaults::LargeContentPadding()));
    assert_eq!(sides(d.content_padding_for(Dp(136.0))), sides(ButtonDefaults::ExtraLargeContentPadding()));
    assert_eq!(
        [d.icon_size_for(Dp(32.0)), d.icon_size_for(Dp(56.0)), d.icon_size_for(Dp(96.0)), d.icon_size_for(Dp(136.0))],
        [Dp(20.0), Dp(24.0), Dp(32.0), Dp(40.0)]
    );
    assert_eq!([d.icon_spacing_for(Dp(32.0)), d.icon_spacing_for(Dp(96.0))], [Dp(4.0), Dp(12.0)]);
    // What reads the theme: the shapes, the styles, the border, the elevations.
    let checked = Rc::new(Cell::new(false));
    let ch = checked.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let ch = ch.clone();
        MaterialTheme(
            content = move || {
                let shapes = MaterialTheme::shapes();
                let typography = MaterialTheme::typography();
                let cs = MaterialTheme::color_scheme();
                assert!(same(&ButtonDefaults::shape(), CircleShape));
                for shape in [
                    ButtonDefaults::elevated_shape(),
                    ButtonDefaults::filled_tonal_shape(),
                    ButtonDefaults::outlined_shape(),
                    ButtonDefaults::text_shape(),
                ] {
                    assert!(same(&shape, CircleShape));
                }
                assert!(same(&ButtonDefaults::square_shape(), shapes.medium.clone()));
                assert!(same(&ButtonDefaults::pressed_shape(), shapes.small.clone()));
                assert!(same(&ButtonDefaults::extra_small_pressed_shape(), shapes.small.clone()));
                assert!(same(&ButtonDefaults::medium_pressed_shape(), shapes.medium.clone()));
                assert!(same(&ButtonDefaults::large_pressed_shape(), shapes.large.clone()));
                assert!(same(&ButtonDefaults::extra_large_pressed_shape(), shapes.large.clone()));
                assert_eq!(
                    ButtonDefaults.shapes(),
                    ButtonShapes::new(Arc::new(CircleShape), Arc::new(shapes.small.clone()))
                );
                assert_eq!(ButtonDefaults.shapes_for(Dp(40.0)), ButtonDefaults.shapes());
                assert!(same(&ButtonDefaults.shapes_for(Dp(56.0)).pressed_shape, shapes.medium.clone()));
                assert!(same(&ButtonDefaults.shapes_for(Dp(136.0)).pressed_shape, shapes.large.clone()));
                let square = named_args!(ButtonDefaults.shapes(shape = Some(Arc::new(RectangleShape))));
                assert!(same(&square.shape, RectangleShape));
                assert_eq!(ButtonDefaults.text_style_for(Dp(40.0)), typography.label_large);
                assert_eq!(ButtonDefaults.text_style_for(Dp(56.0)), typography.title_medium);
                assert_eq!(ButtonDefaults.text_style_for(Dp(96.0)), typography.headline_small);
                assert_eq!(ButtonDefaults.text_style_for(Dp(136.0)), typography.headline_large);
                assert_eq!(ButtonDefaults.outlined_button_border(), BorderStroke(1.dp(), cs.outline_variant));
                assert_eq!(
                    named_args!(ButtonDefaults.outlined_button_border(enabled = false)),
                    BorderStroke(1.dp(), named_args!(cs.outline_variant.copy(alpha = 0.1)))
                );
                #[allow(deprecated)]
                {
                    assert_eq!(
                        ButtonDefaults::outlined_button_border_property(),
                        BorderStroke(1.dp(), cs.outline_variant)
                    );
                }
                assert_eq!(
                    named_args!(ButtonDefaults.button_elevation(hovered_elevation = 5.dp())),
                    named_args!(ButtonDefaults.button_elevation(hovered_elevation = 5.dp()))
                );
                assert!(ButtonDefaults.button_elevation() != ButtonDefaults.elevated_button_elevation());
                assert_eq!(ButtonDefaults.filled_tonal_button_elevation(), ButtonDefaults.button_elevation());
                ch.set(true);
            },
        );
    }));
    assert!(checked.get());
}

/// M15's samples of the sized buttons (X/Medium/Large/XLargeButtonWithIconSample): each `heightIn` its size, its
/// padding, icon, spacing and label style by it.
fn expressive_button_sizes_pick_their_paddings_icons_and_styles() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    for (i, size) in [
                        ButtonDefaults::ExtraSmallContainerHeight,
                        ButtonDefaults::MediumContainerHeight(),
                        ButtonDefaults::LargeContainerHeight(),
                        ButtonDefaults::ExtraLargeContainerHeight,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        key(&i, move || {
                            Button(
                                || {},
                                modifier = Modifier.height_in(size, Dp::Unspecified).test_tag(format!("düğme{i}")),
                                content_padding =
                                    named_args!(ButtonDefaults.content_padding_for(size, has_start_icon = true)),
                                content = move || {
                                    VectorIcon(
                                        Icons::Filled::Edit(),
                                        Some("Localized description".to_string()),
                                        modifier = Modifier
                                            .size(ButtonDefaults.icon_size_for(size))
                                            .test_tag(format!("simge{i}")),
                                    );
                                    Spacer(Modifier.size(ButtonDefaults.icon_spacing_for(size)));
                                    // (the extra small sample's label in the button's own style)
                                    if i == 0 {
                                        Text("Label", modifier = Modifier.test_tag(format!("yazı{i}")));
                                    } else {
                                        Text(
                                            "Label",
                                            style = ButtonDefaults.text_style_for(size),
                                            modifier = Modifier.test_tag(format!("yazı{i}")),
                                        );
                                    }
                                },
                            );
                        });
                    }
                });
            },
        );
    }));
    let canvas = rule.draw();
    let expected =
        [(32.0, 12.0, 20.0, 4.0), (56.0, 24.0, 24.0, 8.0), (96.0, 48.0, 32.0, 12.0), (136.0, 64.0, 40.0, 16.0)];
    for (i, (height, start, icon_size, spacing)) in expected.into_iter().enumerate() {
        let c = container(&canvas, bounds(&rule, &format!("düğme{i}"))).expect("a container");
        let icon = rule.on_node_with_tag_unmerged(&format!("simge{i}")).get_unclipped_bounds_in_root();
        let label = rule.on_node_with_tag_unmerged(&format!("yazı{i}")).get_unclipped_bounds_in_root();
        assert!(near(c.height(), height, 0.01), "{i}: {c:?}");
        assert!(near(icon.left - c.left, start, 0.01) && near(icon.width(), icon_size, 0.01), "{i}: {icon:?}");
        assert!(near(label.left - icon.right, spacing, 0.01), "{i}: {label:?}");
    }
    // The label sizes: labelLarge, titleMedium, headlineSmall, headlineLarge.
    let typography = Typography::default();
    let sizes: Vec<f32> =
        canvas.text_styles.iter().zip(&canvas.texts).filter(|(_, t)| *t == "Label").map(|(s, _)| s.font_size).collect();
    let expected: Vec<f32> =
        [typography.label_large, typography.title_medium, typography.headline_small, typography.headline_large]
            .iter()
            .map(|s| s.font_size.value())
            .collect();
    assert_eq!(sizes, expected);
}

/// The radius of the container's top start corner drawn under `b`.
fn radius_at(canvas: &RecordingCanvas, b: Rect) -> Option<f32> {
    canvas
        .rects
        .iter()
        .zip(canvas.rect_radii.iter().zip(&canvas.rect_colors))
        .find(|(r, (_, c))| {
            c.alpha() > 0.5 && near(r.center().y, b.center().y, 0.1) && near(r.center().x, b.center().x, 0.1)
        })
        .map(|(_, (radius, _))| *radius)
}

fn button_shapes_morph_as_they_are_pressed() {
    // Compose's Button(onClick, shapes): round (20 dp corners of a 40 dp button) at rest, the theme's small
    // corners (8 dp) pressed, corner by corner along the default effects spec; back as it is let go. Shapes that
    // aren't corner based switch at once.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    ButtonWithShapes(
                        || {},
                        ButtonDefaults.shapes(),
                        modifier = Modifier.test_tag("biçim"),
                        content = || {
                            Text("Bas");
                        },
                    );
                    ButtonWithShapes(
                        || {},
                        ButtonShapes::new(
                            Arc::new(RectangleShape),
                            Arc::new(GenericShape(|path: &mut Path, size: Size, _: LayoutDirection| {
                                path.add_oval(size.to_rect(), PathDirection::CounterClockwise)
                            })),
                        ),
                        modifier = Modifier.test_tag("köşesiz"),
                        content = || Text("Bas"),
                    );
                    OutlinedButtonWithShapes(
                        || {},
                        ButtonDefaults.shapes(),
                        modifier = Modifier.test_tag("çizgili"),
                        content = || Text("Bas"),
                    );
                    ElevatedButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Bas"));
                    FilledTonalButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Bas"));
                    TextButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Bas"));
                });
            },
        );
    }));
    let b = bounds(&rule, "biçim");
    assert_eq!(radius_at(&rule.draw(), b), Some(20.0));
    rule.host().pointer_down(b.center(), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.draw();
    rule.advance_frame();
    rule.advance_frame();
    let morphing = radius_at(&rule.draw(), b).expect("a container");
    assert!(morphing < 19.9 && morphing > 8.1, "on its way: {morphing}");
    rule.advance_time_by(1000);
    rule.draw();
    rule.advance_time_by(100);
    assert_eq!(radius_at(&rule.draw(), b), Some(8.0));
    rule.host().pointer_up(b.center(), -1, 0);
    rule.advance_time_by(1000);
    rule.draw();
    rule.advance_time_by(100);
    assert_eq!(radius_at(&rule.draw(), b), Some(20.0));
    // (a rectangle at rest; pressed, the oval's path at once)
    let k = bounds(&rule, "köşesiz");
    assert_eq!(radius_at(&rule.draw(), k), Some(0.0));
    rule.host().pointer_down(k.center(), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    let canvas = rule.draw();
    assert!(radius_at(&canvas, k).is_none() && !canvas.paths.is_empty());
    rule.host().pointer_up(k.center(), -1, 0);
    rule.wait_for_idle();
    // The outlined one's line takes the shape too.
    let o = bounds(&rule, "çizgili");
    let canvas = rule.draw();
    assert!(canvas.strokes.iter().any(|s| near(s.center().y, o.center().y, 0.1)));
}

fn button_elevation_follows_focus_and_snaps_when_disabled() {
    let source = MutableInteractionSource::new();
    let enabled = mutable_state_of(true);
    let (s, e) = (source.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, e) = (s.clone(), e.clone());
        MaterialTheme(
            content = move || {
                Button(
                    || {},
                    enabled = e.get(),
                    elevation = Some(named_args!(ButtonDefaults.button_elevation(
                        default_elevation = 2.dp(),
                        focused_elevation = 5.dp(),
                        disabled_elevation = 1.dp(),
                    ))),
                    interaction_source = Some(s.clone()),
                    modifier = Modifier.test_tag("odak"),
                    content = || Text("Odak"),
                );
            },
        );
    }));
    let spread = || shadow_at(&rule.draw(), bounds(&rule, "odak"), 2.0);
    assert!(near(spread(), 2.0, 0.01));
    let focus = FocusInteraction::Focus::new();
    assert!(source.try_emit(focus));
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let rising = spread();
    assert!(rising > 2.01 && rising < 4.99, "on its way: {rising}");
    rule.advance_time_by(500);
    assert!(near(spread(), 5.0, 0.01));
    source.try_emit(FocusInteraction::Unfocus::new(focus));
    rule.advance_time_by(500);
    assert!(near(spread(), 2.0, 0.01));
    // Disabled: at once.
    enabled.set(false);
    rule.advance_frame();
    rule.advance_frame();
    assert!(near(spread(), 1.0, 0.01), "{}", spread());
    // No elevation: no shadow.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Button(|| {}, elevation = None, modifier = Modifier.test_tag("düz"), content = || Text("Düz"));
    }));
    let at = bounds(&rule, "düz").center();
    rule.host().pointer_move(at, -1, 0);
    rule.advance_time_by(500);
    assert!(rule.draw().elevation_shadows.is_empty());
}

fn fab_sizes_shapes_and_colors() {
    let seen: Rc<RefCell<Vec<Color>>> = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                let s = s.clone();
                Column(move || {
                    let r = s.clone();
                    FloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("fab"),
                        content = move || {
                            r.borrow_mut().push(LocalContentColor.current());
                            VectorIcon(Icons::Filled::Add(), Some("Ekle".to_string()));
                        },
                    );
                    SmallFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("küçük"),
                        content = || {
                            VectorIcon(Icons::Filled::Add(), None);
                        },
                    );
                    LargeFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("büyük"),
                        content = || {
                            VectorIcon(
                                Icons::Filled::Add(),
                                None,
                                modifier = Modifier.size(FloatingActionButtonDefaults::LargeIconSize),
                            );
                        },
                    );
                    let r = s.clone();
                    FloatingActionButton(
                        || {},
                        container_color = MaterialTheme::color_scheme().tertiary,
                        modifier = Modifier.test_tag("üçüncül"),
                        content = move || r.borrow_mut().push(LocalContentColor.current()),
                    );
                });
            },
        );
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    let fill = |tag: &str| {
        let b = bounds(&rule, tag);
        canvas
            .rects
            .iter()
            .zip(canvas.rect_radii.iter().zip(&canvas.rect_colors))
            .find(|(r, _)| **r == b)
            .map(|(_, (radius, color))| (*radius, *color))
    };
    assert_eq!(bounds(&rule, "fab").size(), Size::new(56.0, 56.0));
    assert_eq!(fill("fab"), Some((16.0, cs.primary_container)));
    // (the small one is 40 dp; its touch area 48)
    let small = bounds(&rule, "küçük");
    assert!(near(small.height(), 40.0, 0.01) && near(small.top, 56.0 + 4.0, 0.01), "{small:?}");
    let small_fill = canvas
        .rects
        .iter()
        .zip(&canvas.rect_radii)
        .find(|(r, _)| near(r.center().y, small.center().y, 0.1) && near(r.width(), 40.0, 0.01))
        .map(|(_, radius)| *radius);
    assert_eq!(small_fill, Some(12.0));
    assert_eq!(bounds(&rule, "büyük").size(), Size::new(96.0, 96.0));
    assert_eq!(fill("büyük"), Some((28.0, cs.primary_container)));
    assert_eq!(fill("üçüncül").map(|f| f.1), Some(cs.tertiary));
    assert_eq!(seen.borrow().first().copied(), Some(cs.on_primary_container));
    assert_eq!(seen.borrow().last().copied(), Some(cs.on_tertiary));
    let node = rule.on_node_with_tag("fab").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button));
    rule.on_node_with_content_description("Ekle").assert_exists();
    let checked = Rc::new(Cell::new(false));
    let ch = checked.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let ch = ch.clone();
        MaterialTheme(
            content = move || {
                let shapes = MaterialTheme::shapes();
                assert!(same(&FloatingActionButtonDefaults::shape(), shapes.large.clone()));
                assert!(same(&FloatingActionButtonDefaults::small_shape(), shapes.medium.clone()));
                assert!(same(&FloatingActionButtonDefaults::medium_shape(), ShapeDefaults::LargeIncreased()));
                assert!(same(&FloatingActionButtonDefaults::large_shape(), shapes.extra_large.clone()));
                assert!(same(&FloatingActionButtonDefaults::extended_fab_shape(), shapes.large.clone()));
                assert!(same(&FloatingActionButtonDefaults::small_extended_fab_shape(), shapes.large.clone()));
                assert!(same(
                    &FloatingActionButtonDefaults::medium_extended_fab_shape(),
                    ShapeDefaults::LargeIncreased()
                ));
                assert!(same(&FloatingActionButtonDefaults::large_extended_fab_shape(), shapes.extra_large.clone()));
                assert_eq!(
                    FloatingActionButtonDefaults::container_color(),
                    MaterialTheme::color_scheme().primary_container
                );
                assert_eq!(
                    (FloatingActionButtonDefaults::MediumIconSize, FloatingActionButtonDefaults::LargeIconSize),
                    (Dp(28.0), Dp(36.0))
                );
                ch.set(true);
            },
        );
    }));
    assert!(checked.get());
}

fn fab_elevations_follow_interactions_and_their_defaults() {
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let checked = Rc::new(Cell::new(false));
    let ch = checked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, ch) = (s.clone(), ch.clone());
        MaterialTheme(
            content = move || {
                assert_eq!(
                    FloatingActionButtonDefaults.elevation(),
                    named_args!(
                        FloatingActionButtonDefaults.elevation(default_elevation = 6.dp(), hovered_elevation = 8.dp())
                    )
                );
                assert_eq!(
                    FloatingActionButtonDefaults.lowered_elevation(),
                    named_args!(FloatingActionButtonDefaults.elevation(
                        default_elevation = 1.dp(),
                        pressed_elevation = 1.dp(),
                        focused_elevation = 1.dp(),
                        hovered_elevation = 3.dp(),
                    ))
                );
                assert_eq!(
                    FloatingActionButtonDefaults.bottom_app_bar_fab_elevation(),
                    named_args!(FloatingActionButtonDefaults.lowered_elevation(
                        default_elevation = 0.dp(),
                        pressed_elevation = 0.dp(),
                        focused_elevation = 0.dp(),
                        hovered_elevation = 0.dp(),
                    ))
                );
                ch.set(true);
                let s = s.clone();
                Column(move || {
                    FloatingActionButton(
                        || {},
                        elevation = FloatingActionButtonDefaults.lowered_elevation(),
                        interaction_source = Some(s.clone()),
                        modifier = Modifier.test_tag("alçak"),
                        content = || {},
                    );
                    FloatingActionButton(
                        || {},
                        elevation = FloatingActionButtonDefaults.bottom_app_bar_fab_elevation(),
                        modifier = Modifier.test_tag("çubukta"),
                        content = || {},
                    );
                });
            },
        );
    }));
    assert!(checked.get());
    let spread = |tag: &str| shadow_at(&rule.draw(), bounds(&rule, tag), 4.0);
    assert!(near(spread("alçak"), 1.0, 0.01) && near(spread("çubukta"), 0.0, 0.01));
    let hover = HoverInteraction::Enter::new();
    source.try_emit(hover);
    rule.advance_time_by(500);
    assert!(near(spread("alçak"), 3.0, 0.01), "{}", spread("alçak"));
    // A focus on top of the hover: the last one wins (1 dp); its end goes back to the hover's.
    let focus = FocusInteraction::Focus::new();
    source.try_emit(focus);
    rule.advance_time_by(500);
    assert!(near(spread("alçak"), 1.0, 0.01));
    source.try_emit(FocusInteraction::Unfocus::new(focus));
    rule.advance_time_by(500);
    assert!(near(spread("alçak"), 3.0, 0.01));
    source.try_emit(HoverInteraction::Exit::new(hover));
    rule.advance_time_by(500);
    assert!(near(spread("alçak"), 1.0, 0.01));
}

fn extended_fabs_of_content_take_their_sizes() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    ExtendedFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("geniş"),
                        content = || {
                            Box(modifier = Modifier.size_wh(10.dp(), 10.dp()).test_tag("geniş-içi"));
                        },
                    );
                    ExtendedFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("uzun"),
                        content = || {
                            Box(modifier = Modifier.size_wh(100.dp(), 10.dp()).test_tag("uzun-içi"));
                        },
                    );
                    SmallExtendedFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("küçük"),
                        content = || {
                            Box(modifier = Modifier.size_wh(10.dp(), 10.dp()));
                        },
                    );
                    MediumExtendedFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("orta"),
                        content = || {
                            Box(modifier = Modifier.size_wh(100.dp(), 10.dp()).test_tag("orta-içi"));
                        },
                    );
                    LargeExtendedFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("büyük"),
                        content = || {
                            Box(modifier = Modifier.size_wh(100.dp(), 10.dp()));
                        },
                    );
                });
            },
        );
    }));
    // At least 80 wide (its content centered), 20 dp in from each side.
    let wide = bounds(&rule, "geniş");
    assert!(near(wide.width(), 80.0, 0.01) && near(wide.height(), 56.0, 0.01), "{wide:?}");
    assert!(near(bounds(&rule, "geniş-içi").center().x, wide.center().x, 0.5));
    let long = bounds(&rule, "uzun");
    assert!(near(long.width(), 140.0, 0.01) && near(bounds(&rule, "uzun-içi").left, long.left + 20.0, 0.01));
    // The Expressive sizes: squares of their height at least, their paddings.
    assert_eq!(bounds(&rule, "küçük").size(), Size::new(56.0, 56.0));
    let medium = bounds(&rule, "orta");
    assert!(near(medium.width(), 26.0 + 100.0 + 26.0, 0.01) && near(medium.height(), 80.0, 0.01), "{medium:?}");
    assert!(near(bounds(&rule, "orta-içi").left, medium.left + 26.0, 0.01));
    let large = bounds(&rule, "büyük");
    assert!(near(large.width(), 28.0 + 100.0 + 28.0, 0.01) && near(large.height(), 96.0, 0.01), "{large:?}");
}

fn extended_fab_of_text_and_icon_collapses_to_its_icon() {
    // Compose's ExtendedFloatingActionButton(text, icon, onClick, expanded): collapsed, a 56 dp square, the icon
    // centered, the label (no part of its semantics) gone after fading out and shrinking towards the start.
    let expanded = mutable_state_of(true);
    let e = expanded.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let e = e.clone();
        MaterialTheme(
            content = move || {
                TextIconExtendedFloatingActionButton(
                    || Text("Yeni ileti", modifier = Modifier.test_tag("yazı")),
                    || {
                        VectorIcon(
                            Icons::Filled::Edit(),
                            Some("Yaz".to_string()),
                            modifier = Modifier.test_tag("simge"),
                        )
                    },
                    on_click = || {},
                    expanded = e.get(),
                    modifier = Modifier.test_tag("fab"),
                );
            },
        );
    }));
    let wide = bounds(&rule, "fab");
    let label = rule.on_node_with_tag_unmerged("yazı").get_unclipped_bounds_in_root();
    assert!(near(wide.width(), (16.0 + 24.0 + 12.0 + label.width() + 20.0).max(80.0), 1.0), "{wide:?} {label:?}");
    assert!(near(rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root().left, 16.0, 0.01));
    // (the label isn't in the FAB's semantics)
    assert_eq!(rule.on_all_nodes_with_text("Yeni ileti").fetch_semantics_nodes().len(), 0);
    rule.on_node_with_tag("fab").assert_has_click_action();
    expanded.set(false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let narrowing = bounds(&rule, "fab").width();
    assert!(narrowing < wide.width() - 1.0 && narrowing > 56.5, "on its way: {narrowing}");
    rule.advance_time_by(2000);
    let collapsed = bounds(&rule, "fab");
    assert_eq!(collapsed.size(), Size::new(56.0, 56.0));
    let icon = rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root();
    assert!(near(icon.center().x, collapsed.center().x, 0.01), "{icon:?}");
    assert!(!rule.drawn_texts().iter().any(|t| t == "Yeni ileti"));
    expanded.set(true);
    rule.advance_time_by(2000);
    assert!(near(bounds(&rule, "fab").width(), wide.width(), 0.01));
    assert!(rule.drawn_texts().iter().any(|t| t == "Yeni ileti"));
}

fn animate_floating_action_button_shows_and_hides() {
    // Compose's Modifier.animateFloatingActionButton: hidden, scaled to 20% towards the alignment and faded out
    // (then taking no room); shown, back. Drawn through a layer 16 dp larger each way.
    let visible = mutable_state_of(true);
    let v = visible.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        MaterialTheme(
            content = move || {
                let v = v.clone();
                Row(
                    modifier = Modifier.size(200.dp()),
                    content = move || {
                        FloatingActionButton(
                            || {},
                            // (the tag outside: the room it takes)
                            modifier = Modifier.test_tag("fab").animate_floating_action_button_with(
                                v.get(),
                                Alignment::BottomEnd,
                                0.2,
                                tween100(),
                                tween100(),
                            ),
                            content = || VectorIcon(Icons::Filled::Add(), Some("Ekle".to_string())),
                        );
                        Box(modifier = Modifier.size(10.dp()).test_tag("yan"));
                    },
                );
            },
        );
    }));
    assert_eq!(bounds(&rule, "fab").size(), Size::new(56.0, 56.0));
    assert_eq!(bounds(&rule, "yan").left, 56.0);
    visible.set(false);
    rule.advance_frame();
    rule.advance_time_by(48);
    let canvas = rule.draw();
    assert!(canvas.layer_alphas.iter().any(|a| *a > 0.05 && *a < 0.95), "{:?}", canvas.layer_alphas);
    // (the layer: 16 dp beyond the FAB each way, scaled towards its bottom end)
    assert!(
        canvas.layer_bounds.iter().any(|b| near(b.width(), 56.0 + 32.0, 0.5) && near(b.left, -16.0, 0.5)),
        "{:?} {:?}",
        canvas.layer_bounds,
        canvas.layer_transforms
    );
    rule.advance_time_by(500);
    // Hidden: no room.
    assert_eq!(bounds(&rule, "yan").left, 0.0);
    visible.set(true);
    rule.advance_time_by(500);
    assert_eq!(bounds(&rule, "yan").left, 56.0);
    let canvas = rule.draw();
    assert!(!canvas.layer_alphas.iter().any(|a| *a > 0.01 && *a < 0.99), "{:?}", canvas.layer_alphas);
    // The defaults: the theme's specs, 20%.
    let visible = mutable_state_of(true);
    let v = visible.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        MaterialTheme(
            content = move || {
                let v = v.clone();
                Row(move || {
                    FloatingActionButton(
                        || {},
                        modifier = named_args!(Modifier.animate_floating_action_button(v.get(), Alignment::Center)),
                        content = || {},
                    );
                    Box(modifier = Modifier.size(10.dp()).test_tag("yan"));
                });
            },
        );
    }));
    assert_eq!(bounds(&rule, "yan").left, 56.0);
    visible.set(false);
    rule.advance_time_by(2000);
    assert_eq!(bounds(&rule, "yan").left, 0.0);
}

// ---- M15's samples ---------------------------------------------------------------------------------------------

/// ButtonSamples.kt: each sample composed as written, its label shown and clickable (the sized ones:
/// `expressive_button_sizes_pick_their_paddings_icons_and_styles`).
fn button_samples() {
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            MaterialTheme(
                content = move || {
                    let c = c.clone();
                    Column(move || {
                        let c = c.clone();
                        // ButtonSample
                        Button(move || c.set(c.get() + 1), content = || Text("Button"));
                        // ButtonWithAnimatedShapeSample
                        ButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Shaped"));
                        // SquareButtonSample
                        Button(
                            || {},
                            shape = ButtonDefaults::square_shape(),
                            modifier = Modifier.test_tag("kare"),
                            content = || Text("Square"),
                        );
                        // SmallButtonSample
                        Button(
                            || {},
                            content_padding = ButtonDefaults::SmallContentPadding(),
                            content = || Text("Small"),
                        );
                        // ElevatedButtonSample, ElevatedButtonWithAnimatedShapeSample
                        ElevatedButton(|| {}, content = || Text("Elevated Button"));
                        ElevatedButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Elevated Shaped"));
                        // FilledTonalButtonSample, FilledTonalButtonWithAnimatedShapeSample
                        FilledTonalButton(|| {}, content = || Text("Filled Tonal Button"));
                        FilledTonalButtonWithShapes(
                            || {},
                            ButtonDefaults.shapes(),
                            content = || Text("Filled Tonal Shaped"),
                        );
                        // OutlinedButtonSample, OutlinedButtonWithAnimatedShapeSample
                        OutlinedButton(|| {}, content = || Text("Outlined Button"));
                        OutlinedButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Outlined Shaped"));
                        // TextButtonSample, TextButtonWithAnimatedShapeSample
                        TextButton(|| {}, content = || Text("Text Button"));
                        TextButtonWithShapes(|| {}, ButtonDefaults.shapes(), content = || Text("Text Shaped"));
                        // ButtonWithIconSample
                        Button(
                            || {},
                            modifier = Modifier.test_tag("simgeli"),
                            content_padding = named_args!(
                                ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight(), has_start_icon = true)
                            ),
                            content = || {
                                VectorIcon(
                                    Icons::Filled::Favorite(),
                                    Some("Localized description".to_string()),
                                    modifier = Modifier
                                        .size(ButtonDefaults.icon_size_for(ButtonDefaults::MinHeight()))
                                        .test_tag("kalp"),
                                );
                                Spacer(Modifier.size(ButtonDefaults.icon_spacing_for(ButtonDefaults::MinHeight())));
                                Text("Like");
                            },
                        );
                    });
                },
            );
        }),
        IntSize::new(400, 1200),
    );
    for label in [
        "Button",
        "Shaped",
        "Square",
        "Small",
        "Elevated Button",
        "Elevated Shaped",
        "Filled Tonal Button",
        "Filled Tonal Shaped",
        "Outlined Button",
        "Outlined Shaped",
        "Text Button",
        "Text Shaped",
        "Like",
    ] {
        rule.on_node_with_text(label).assert_has_click_action();
    }
    rule.on_node_with_text("Button").perform_click();
    assert_eq!(clicks.get(), 1);
    let canvas = rule.draw();
    // (the square one in the theme's medium corners)
    assert_eq!(radius_at(&canvas, bounds(&rule, "kare")), Some(12.0));
    let icon = rule.on_node_with_tag_unmerged("kalp").get_unclipped_bounds_in_root();
    let button = container(&canvas, bounds(&rule, "simgeli")).expect("a container");
    assert!(near(icon.left - button.left, 16.0, 0.01) && near(icon.width(), 20.0, 0.01), "{icon:?} {button:?}");
}

/// The samples' `TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(
/// TooltipAnchorPosition.Above), tooltip = { PlainTooltip { Text("Localized description") } }, state =
/// rememberTooltipState()) { … }` around a FAB ("A FAB should have a tooltip associated with it").
#[kompozable]
fn FabTip(content: impl Fn() + 'static) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = |s: &dyn TooltipScope| s.PlainTooltip(content = || Text("Localized description")),
        state = remember_tooltip_state(),
        content = content,
    );
}

/// FloatingActionButtonSamples.kt (the FAB menu's sample is in tests/fab_menu.rs): the FABs as the samples compose them, each in its
/// TooltipBox (a long press shows it); the animated ones, over a Scaffold, are in tests/scaffold.rs.
fn floating_action_button_samples() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            MaterialTheme(
                content = || {
                    Column(|| {
                        // FloatingActionButtonSample
                        FabTip(|| {
                            FloatingActionButton(
                                || {},
                                content = || {
                                    VectorIcon(
                                        Icons::Filled::Add(),
                                        Some("fab".to_string()),
                                        modifier = Modifier.test_tag("fab"),
                                    )
                                },
                            );
                        });
                        // SmallFloatingActionButtonSample
                        FabTip(|| {
                            SmallFloatingActionButton(
                                || {},
                                content = || {
                                    VectorIcon(
                                        Icons::Filled::Add(),
                                        Some("small".to_string()),
                                        modifier = Modifier.test_tag("small"),
                                    )
                                },
                            );
                        });
                        // MediumFloatingActionButtonSample
                        FabTip(|| {
                            MediumFloatingActionButton(
                                || {},
                                content = || {
                                    VectorIcon(
                                        Icons::Filled::Add(),
                                        Some("medium".to_string()),
                                        modifier = Modifier
                                            .size(FloatingActionButtonDefaults::MediumIconSize)
                                            .test_tag("medium"),
                                    )
                                },
                            );
                        });
                        // LargeFloatingActionButtonSample
                        FabTip(|| {
                            LargeFloatingActionButton(
                                || {},
                                content = || {
                                    VectorIcon(
                                        Icons::Filled::Add(),
                                        Some("large".to_string()),
                                        modifier = Modifier
                                            .size(FloatingActionButtonDefaults::LargeIconSize)
                                            .test_tag("large"),
                                    )
                                },
                            );
                        });
                        // ExtendedFloatingActionButtonTextSample and its sizes
                        ExtendedFloatingActionButton(|| {}, content = || Text("Extended FAB"));
                        SmallExtendedFloatingActionButton(|| {}, content = || Text("Small Extended FAB"));
                        MediumExtendedFloatingActionButton(|| {}, content = || Text("Medium Extended FAB"));
                        LargeExtendedFloatingActionButton(|| {}, content = || Text("Large Extended FAB"));
                        // ExtendedFloatingActionButtonSample and its sizes
                        TextIconExtendedFloatingActionButton(
                            || Text("Extended FAB icon"),
                            || VectorIcon(Icons::Filled::Add(), Some("Localized description".to_string())),
                            on_click = || {},
                        );
                        TextIconSmallExtendedFloatingActionButton(
                            || Text("Small Extended FAB icon"),
                            || VectorIcon(Icons::Filled::Add(), Some("Localized description".to_string())),
                            on_click = || {},
                        );
                        TextIconMediumExtendedFloatingActionButton(
                            || Text("Medium Extended FAB icon"),
                            || {
                                VectorIcon(
                                    Icons::Filled::Add(),
                                    Some("Localized description".to_string()),
                                    modifier = Modifier.size(FloatingActionButtonDefaults::MediumIconSize),
                                )
                            },
                            on_click = || {},
                        );
                        TextIconLargeExtendedFloatingActionButton(
                            || Text("Large Extended FAB icon"),
                            || {
                                VectorIcon(
                                    Icons::Filled::Add(),
                                    Some("Localized description".to_string()),
                                    modifier = Modifier
                                        .size(FloatingActionButtonDefaults::LargeIconSize)
                                        .test_tag("large-ext"),
                                )
                            },
                            on_click = || {},
                        );
                    });
                },
            );
        }),
        IntSize::new(600, 1400),
    );
    for (tag, size) in [("fab", 24.0), ("small", 24.0), ("medium", 28.0), ("large", 36.0), ("large-ext", 36.0)] {
        let icon = rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root();
        assert!(near(icon.width(), size, 0.01), "{tag}: {icon:?}");
    }
    // (a long press on the FAB shows its tooltip, for a while)
    let fab = rule.on_node_with_tag_unmerged("fab").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(fab, -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(fab, -1, 0);
    rule.advance_frame();
    rule.on_node_with_text("Localized description").assert_exists();
    rule.advance_time_by(1600);
    rule.wait_for_idle();
    rule.on_node_with_text("Localized description").assert_does_not_exist();
    let texts = rule.drawn_texts();
    for label in [
        "Extended FAB",
        "Small Extended FAB",
        "Medium Extended FAB",
        "Large Extended FAB",
        "Extended FAB icon",
        "Small Extended FAB icon",
        "Medium Extended FAB icon",
        "Large Extended FAB icon",
    ] {
        assert!(texts.iter().any(|t| t == label), "{label}");
    }
    // (AnimatedFloatingActionButtonSample and the four AnimatedExtendedFloatingActionButtonSamples compose a Scaffold
    // over a LazyColumn and a TooltipBox: written as M15 has them in tests/scaffold.rs)
}
