//! Material's icon buttons and icon toggle buttons as Compose's material3 has them: the C++ Kompoz's tests by name
//! (kompoz_tests.cpp, with M15's values where the C++ targeted Compose 1.4), Compose's behaviour (sizes, colors per
//! state, borders, shapes and their morphing, the ripple, semantics) and M15's IconButtonSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, has_click_action, has_content_description, has_role, is_on};
use kompoz_ui::draw::{Painter, remember_vector_painter};
use kompoz_ui::input::PointerType;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    icon_button_clicks,
    icon_toggle_buttons_flip_and_take_their_checked_colors,
    icon_toggle_buttons_disabled_and_restyled,
    outlined_icon_button_has_an_outline_and_ripples_in_its_circle,
    filled_and_tonal_icon_buttons_as_compose_1_4,
    icon_buttons_take_the_content_color_around_them,
    icon_buttons_are_40_dp_in_48_dp_and_buttons_to_accessibility,
    container_and_icon_sizes_per_size_and_width_option,
    default_colors_are_the_tokens,
    colors_given_take_kotlins_defaults_for_the_rest,
    colors_copy_keeps_what_is_unspecified,
    icon_toggle_buttons_draw_the_colors_of_their_state,
    outlined_borders_follow_enabled_and_checked,
    shapes_defaults_constructors_and_copy,
    icon_buttons_with_shapes_morph_as_they_are_pressed,
    icon_toggle_buttons_with_shapes_take_their_checked_shape,
    every_kind_with_shapes_clicks_and_toggles,
    expressive_shapes_are_the_tokens,
    the_ripple_is_bounded_by_the_shape_in_the_content_color,
    deprecated_overloads_are_the_standard_shapes,
    icon_button_samples,
    icon_toggle_button_samples,
    sized_icon_button_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

#[kompozable]
fn icon(vector: ImageVector, label: &'static str) {
    VectorIcon(vector, Some(label.to_string()));
}

fn faded(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

fn icon_button_clicks() {
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Row(move || {
            let (c1, c2) = (c.clone(), c.clone());
            IconButton(move || c1.set(c1.get() + 1), content = || icon(Icons::Filled::Favorite(), "Beğen"));
            IconButton(
                move || c2.set(c2.get() + 1),
                enabled = false,
                content = || icon(Icons::Filled::Delete(), "Sil"),
            );
        });
    }));
    rule.on_node_with_content_description("Beğen").perform_click();
    rule.on_node_with_content_description("Sil").perform_click();
    assert_eq!(clicks.get(), 1);
}

fn icon_toggle_buttons_flip_and_take_their_checked_colors() {
    let cs = light_color_scheme();
    let on: Vec<MutableState<bool>> = (0..4).map(|_| mutable_state_of(false)).collect();
    let heard: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (o, h) = (on.clone(), heard.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, h) = (o.clone(), h.clone());
        Row(move || {
            let set = |i: usize| {
                let (o, h) = (o.clone(), h.clone());
                move |v: bool| {
                    h.borrow_mut().push(v);
                    o[i].set(v);
                }
            };
            let star = || icon(Icons::Filled::Star(), "Yıldız");
            IconToggleButton(o[0].get(), set(0), modifier = Modifier.test_tag("düz"), content = star);
            FilledIconToggleButton(o[1].get(), set(1), modifier = Modifier.test_tag("dolu"), content = star);
            FilledTonalIconToggleButton(o[2].get(), set(2), modifier = Modifier.test_tag("tonal"), content = star);
            let o3 = o[3].get();
            let set3 = set(3);
            // (the room it takes: the Box around it — its own node's bounds are its toggleable's, the merging
            // semantics' inside the minimum size, as Compose's `findCoordinatorToGetBounds`; the C++ gave the
            // node the 48 dp)
            Box(
                modifier = Modifier.test_tag("çerçeveli kutu"),
                content = move || {
                    OutlinedIconToggleButton(o3, set3, modifier = Modifier.test_tag("çerçeveli"), content = star);
                },
            );
        });
    }));
    let off = rule.draw(); // one container and one icon each, in order
    // (M15's, as Compose 1.4's: off, filled on surface container, tonal on secondary container)
    assert_eq!(
        off.rect_colors,
        vec![Color::Transparent, cs.surface_container, cs.secondary_container, Color::Transparent]
    );
    // (the plain and outlined ones in the content color around them: black here, outside a theme)
    assert_eq!(off.vector_colors, vec![Color::Black, cs.on_surface_variant, cs.on_secondary_container, Color::Black]);
    // (only the outlined one, around its circle, in that color)
    assert_eq!(off.stroke_colors, vec![Color::Black]);
    // (a 40 dp circle in the middle of 48 dp of room)
    let outlined = bounds(&rule, "çerçeveli kutu");
    assert!(near(outlined.width(), 48.0, 0.01) && near(outlined.height(), 48.0, 0.01));
    assert!(near(bounds(&rule, "çerçeveli").width(), 40.0, 0.01));
    assert!(
        off.strokes.len() == 1
            && near(off.strokes[0].width(), 40.0, 0.5)
            && near(off.strokes[0].left, outlined.left + 4.0, 0.5)
    );
    for tag in ["düz", "dolu", "tonal", "çerçeveli"] {
        rule.on_node_with_tag(tag).assert_is_off();
        rule.on_node_with_tag(tag).perform_click();
        rule.on_node_with_tag(tag).assert_is_on();
    }
    assert_eq!(*heard.borrow(), vec![true, true, true, true]);
    // (accessibility services read each as a checked checkbox named by its icon)
    let boxes = rule
        .on_all_nodes(
            has_role(Role::Checkbox)
                .and(is_on())
                .and(has_content_description("Yıldız", false, false))
                .and(has_click_action()),
        )
        .fetch_semantics_nodes();
    assert_eq!(boxes.len(), 4);
    let checked = rule.draw();
    assert_eq!(checked.rect_colors, vec![Color::Transparent, cs.primary, cs.secondary, cs.inverse_surface]);
    assert_eq!(checked.vector_colors, vec![cs.primary, cs.on_primary, cs.on_secondary, cs.inverse_on_surface]);
    // (on, the outlined one is its dark container alone)
    assert!(checked.strokes.is_empty());
    rule.on_node_with_tag("dolu").perform_click(); // and back
    assert!(!heard.borrow().last().unwrap() && !on[1].get());
    rule.on_node_with_tag("dolu").assert_is_off();
}

fn icon_toggle_buttons_disabled_and_restyled() {
    let cs = light_color_scheme();
    let liked = mutable_state_of(true);
    let changes = Rc::new(Cell::new(0));
    let (l, ch) = (liked.clone(), changes.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, ch) = (l.clone(), ch.clone());
        Row(move || {
            let heart = || icon(Icons::Filled::Favorite(), "Beğen");
            let count = |ch: &Rc<Cell<i32>>| {
                let ch = ch.clone();
                move |_: bool| ch.set(ch.get() + 1)
            };
            FilledIconToggleButton(
                l.get(),
                count(&ch),
                modifier = Modifier.test_tag("kapalı"),
                enabled = false,
                content = heart,
            );
            OutlinedIconToggleButton(
                l.get(),
                count(&ch),
                modifier = Modifier.test_tag("kapalı çerçeveli"),
                enabled = false,
                content = heart,
            );
            OutlinedIconToggleButton(!l.get(), count(&ch), enabled = false, content = heart);
            let l2 = l.clone();
            IconToggleButton(
                l.get(),
                move |v| l2.set(v),
                modifier = Modifier.test_tag("renkli"),
                colors = IconButtonDefaults.icon_toggle_button_colors(
                    container_color = Some(Color::White),
                    content_color = Some(Color::hex(0x808080)),
                    checked_container_color = Some(Color::Black),
                    checked_content_color = Some(Color::hex(0xFFEB3B)),
                ),
                content = heart,
            );
        });
    }));
    rule.on_node_with_tag("kapalı").assert_is_not_enabled().perform_click();
    rule.on_node_with_tag("kapalı çerçeveli").perform_click();
    assert_eq!(changes.get(), 0);
    let canvas = rule.draw();
    let (dim, faint) = (faded(cs.on_surface, 0.1), faded(cs.on_surface, 0.38));
    // (disabled, M15's as Compose 1.4's: a filled one on the surface's text at 10%; an outlined one clear, checked or
    // not, its icon — and an unchecked one's outline — in the color around it faded)
    assert_eq!(canvas.rect_colors, vec![dim, Color::Transparent, Color::Transparent, Color::Black]);
    assert_eq!(
        canvas.vector_colors,
        vec![faint, faded(Color::Black, 0.38), faded(Color::Black, 0.38), Color::hex(0xFFEB3B)]
    );
    assert_eq!(canvas.stroke_colors, vec![faded(Color::Black, 0.38)]);
    rule.on_node_with_tag("renkli").perform_click();
    assert!(!liked.get());
    let after = rule.draw();
    assert!(after.rect_colors.len() == 4 && *after.rect_colors.last().unwrap() == Color::White);
    assert_eq!(*after.vector_colors.last().unwrap(), Color::hex(0x808080));
}

fn outlined_icon_button_has_an_outline_and_ripples_in_its_circle() {
    let cs = light_color_scheme();
    let clicks = Rc::new(Cell::new(0));
    let on = mutable_state_of(true);
    let (c, o) = (clicks.clone(), on.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, o) = (c.clone(), o.clone());
        Row(move || {
            let (c1, c2) = (c.clone(), c.clone());
            OutlinedIconButton(
                move || c1.set(c1.get() + 1),
                modifier = Modifier.test_tag("çerçeve"),
                content = || icon(Icons::Filled::Add(), "Ekle"),
            );
            OutlinedIconButton(
                move || c2.set(c2.get() + 1),
                enabled = false,
                content = || icon(Icons::Filled::Add(), "Ekleme"),
            );
            let o2 = o.clone();
            FilledIconToggleButton(
                o.get(),
                move |v| o2.set(v),
                modifier = Modifier.test_tag("aç"),
                content = || icon(Icons::Filled::Star(), "Yıldız"),
            );
        });
    }));
    rule.on_node_with_content_description("Ekle").perform_click();
    rule.on_node_with_content_description("Ekleme").perform_click();
    assert_eq!(clicks.get(), 1);
    {
        let canvas = rule.draw();
        // (the color around them)
        assert_eq!(canvas.stroke_colors, vec![Color::Black, faded(Color::Black, 0.38)]);
        assert_eq!(canvas.rect_colors, vec![Color::Transparent, Color::Transparent, cs.primary]);
        assert_eq!(canvas.vector_colors, vec![Color::Black, faded(Color::Black, 0.38), cs.on_primary]);
    }
    // A press on the (checked) toggle: its ripple is clipped to the circle (a rounded clip) and drawn in its content
    // color. (The toggle's node is its 40 dp toggleable, as Compose's merging semantics; inside the circle the bounded
    // ripple clips to its bounds too, as Compose's `RippleAnimation.draw`'s `clipRect` — the C++ had no rect clip.)
    let toggle = bounds(&rule, "aç");
    assert!(near(toggle.width(), 40.0, 0.01));
    let mid = toggle.center();
    rule.host().pointer_down(mid, -1, 0, PointerType::Touch);
    for _ in 0..3 {
        rule.main_clock().advance_time_by_frame();
    }
    let pressed = rule.draw();
    let circles: Vec<_> = pressed.clip_shapes.iter().filter(|c| near(c.bounds.width(), 40.0, 0.5)).collect();
    assert!(pressed.clips == vec![toggle] && circles.len() == 3, "{:?} {:?}", pressed.clips, pressed.clip_shapes);
    let clip = circles[2];
    assert!(near(clip.bounds.left, toggle.left, 0.5) && near(clip.radii.top_left, 20.0, 0.5));
    let ripple =
        pressed.rect_colors.iter().chain(&pressed.path_colors).copied().find(|c| c.alpha() > 0.0 && c.alpha() < 1.0);
    let ripple = ripple.expect("a ripple");
    assert!(
        ripple.red() == cs.on_primary.red()
            && ripple.green() == cs.on_primary.green()
            && ripple.blue() == cs.on_primary.blue(),
        "{ripple:?}"
    );
    rule.host().pointer_up(mid, -1, 0);
    rule.wait_for_idle();
    assert!(!on.get());
}

fn filled_and_tonal_icon_buttons_as_compose_1_4() {
    let cs = light_color_scheme();
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Row(|| {
            FilledTonalIconButton(|| {}, content = || icon(Icons::Filled::Add(), "Tonal"));
            FilledIconButton(|| {}, enabled = false, content = || icon(Icons::Filled::Add(), "Dolu"));
            FilledTonalIconButton(|| {}, enabled = false, content = || icon(Icons::Filled::Add(), "Kapalı"));
        });
    }));
    let canvas = rule.draw();
    // (tonal on secondary container; disabled ones on the surface's text at 10%: M15's as 1.4's)
    assert_eq!(canvas.rect_colors, vec![cs.secondary_container, faded(cs.on_surface, 0.1), faded(cs.on_surface, 0.1)]);
}

fn icon_buttons_take_the_content_color_around_them() {
    // (the C++ test's Snackbar with a dismiss action — its close button an IconButton in inverse on surface — waits
    // for WP4a's Snackbar)
    let red = Color::new(0.8, 0.1, 0.1, 1.0);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        MaterialTheme(
            color_scheme = light_color_scheme(),
            content = move || {
                Column(move || {
                    KompozitionLocalProvider(&[LocalContentColor.provides(red)], || {
                        IconButton(|| {}, content = || icon(Icons::Filled::Add(), "Ekle"));
                        IconButton(|| {}, enabled = false, content = || icon(Icons::Filled::Add(), "Ekleme"));
                        // (its own)
                        FilledTonalIconButton(|| {}, content = || icon(Icons::Filled::Add(), "Tonal"));
                    });
                });
            },
        );
    }));
    let canvas = rule.draw();
    let cs = light_color_scheme();
    assert_eq!(canvas.vector_colors, vec![red, faded(red, 0.38), cs.on_secondary_container]);
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

fn icon_buttons_are_40_dp_in_48_dp_and_buttons_to_accessibility() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    // (the room each takes: the Box around it; its node the clickable inside, 40 dp)
                    Box(
                        modifier = Modifier.test_tag("düz kutu"),
                        content = || {
                            IconButton(
                                || {},
                                modifier = Modifier.test_tag("düz"),
                                content = || icon(Icons::Filled::Add(), "a"),
                            );
                        },
                    );
                    Box(
                        modifier = Modifier.test_tag("dolu kutu"),
                        content = || {
                            FilledIconButton(
                                || {},
                                modifier = Modifier.test_tag("dolu"),
                                content = || icon(Icons::Filled::Add(), "b"),
                            );
                        },
                    );
                    KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(0.0))], || {
                        IconButton(
                            || {},
                            modifier = Modifier.test_tag("sıkı"),
                            content = || icon(Icons::Filled::Add(), "c"),
                        );
                        OutlinedIconButton(
                            || {},
                            modifier = Modifier.test_tag("sıkı çerçeve"),
                            content = || icon(Icons::Filled::Add(), "d"),
                        );
                    });
                    Box(
                        modifier = Modifier.test_tag("aç kutu"),
                        content = || {
                            IconToggleButton(
                                false,
                                |_| {},
                                modifier = Modifier.test_tag("aç"),
                                content = || icon(Icons::Filled::Add(), "e"),
                            );
                        },
                    );
                });
            },
        );
    }));
    for tag in ["düz", "dolu", "aç"] {
        let (room, node) = (bounds(&rule, &format!("{tag} kutu")), bounds(&rule, tag));
        assert_eq!((room.width(), room.height()), (48.0, 48.0), "{tag}");
        assert_eq!((node.width(), node.height(), node.left - room.left), (40.0, 40.0, 4.0), "{tag}");
    }
    // (without the minimum interactive size, the small container: 40 dp)
    for tag in ["sıkı", "sıkı çerçeve"] {
        let b = bounds(&rule, tag);
        assert_eq!((b.width(), b.height()), (40.0, 40.0), "{tag}");
    }
    // (the icon, 24 dp, in the middle)
    let icon_bounds =
        rule.on_node_with(has_content_description("a", false, false), true).get_unclipped_bounds_in_root();
    let outer = bounds(&rule, "düz");
    assert_eq!((icon_bounds.width(), icon_bounds.left - outer.left, icon_bounds.top - outer.top), (24.0, 8.0, 8.0));
    // (buttons to accessibility, their icon's description theirs; a toggle one a checkbox)
    for tag in ["düz", "dolu", "sıkı çerçeve"] {
        let node = rule.on_node_with_tag(tag).fetch_semantics_node();
        assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button), "{tag}");
    }
    rule.on_node_with_tag("düz").assert_content_description_equals(&["a"]);
    let toggle = rule.on_node_with_tag("aç").fetch_semantics_node();
    assert_eq!(toggle.config().get(SemanticsProperties::Role), Some(Role::Checkbox));
}

fn container_and_icon_sizes_per_size_and_width_option() {
    use IconButtonWidthOption as W;
    let d = |w: f32, h: f32| DpSize::new(Dp(w), Dp(h));
    let size = |f: &dyn Fn(W) -> DpSize| [f(W::Narrow), f(W::Uniform), f(W::Wide)];
    let defaults = IconButtonDefaults;
    assert_eq!(
        size(&|w| named_args!(defaults.extra_small_container_size(width_option = w))),
        [d(28.0, 32.0), d(32.0, 32.0), d(40.0, 32.0)]
    );
    assert_eq!(
        size(&|w| named_args!(defaults.small_container_size(width_option = w))),
        [d(32.0, 40.0), d(40.0, 40.0), d(52.0, 40.0)]
    );
    assert_eq!(
        size(&|w| named_args!(defaults.medium_container_size(width_option = w))),
        [d(48.0, 56.0), d(56.0, 56.0), d(72.0, 56.0)]
    );
    assert_eq!(
        size(&|w| named_args!(defaults.large_container_size(width_option = w))),
        [d(64.0, 96.0), d(96.0, 96.0), d(128.0, 96.0)]
    );
    assert_eq!(
        size(&|w| named_args!(defaults.extra_large_container_size(width_option = w))),
        [d(104.0, 136.0), d(136.0, 136.0), d(184.0, 136.0)]
    );
    // (uniform by default)
    assert_eq!(IconButtonDefaults.medium_container_size(), d(56.0, 56.0));
    assert_eq!(
        [
            IconButtonDefaults::ExtraSmallIconSize,
            IconButtonDefaults::SmallIconSize,
            IconButtonDefaults::MediumIconSize,
            IconButtonDefaults::LargeIconSize,
            IconButtonDefaults::ExtraLargeIconSize,
        ],
        [Dp(20.0), Dp(24.0), Dp(24.0), Dp(32.0), Dp(40.0)]
    );
    // (Compose's toString)
    assert_eq!(format!("{} {} {}", W::Narrow, W::Uniform, W::Wide), "Narrow Uniform Wide");
}

fn default_colors_are_the_tokens() {
    // Each kind's default colors in a theme (a dark one: the tokens' roles, not 1.4's light values), and those made
    // of the content color around.
    let cs = dark_color_scheme();
    let seen: Rc<RefCell<Vec<(IconButtonColors, IconToggleButtonColors)>>> = Rc::default();
    let s = seen.clone();
    let around = Color::hex(0x336699);
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            color_scheme = dark_color_scheme(),
            content = move || {
                let s = s.clone();
                KompozitionLocalProvider(&[LocalContentColor.provides(around)], move || {
                    let mut s = s.borrow_mut();
                    s.clear();
                    let d = IconButtonDefaults;
                    s.push((d.icon_button_colors(), d.icon_toggle_button_colors()));
                    s.push((d.icon_button_vibrant_colors(), d.icon_toggle_button_vibrant_colors()));
                    s.push((d.filled_icon_button_colors(), d.filled_icon_toggle_button_colors()));
                    s.push((d.filled_tonal_icon_button_colors(), d.filled_tonal_icon_toggle_button_colors()));
                    s.push((d.outlined_icon_button_colors(), d.outlined_icon_toggle_button_colors()));
                    s.push((d.outlined_icon_button_vibrant_colors(), d.outlined_icon_toggle_button_vibrant_colors()));
                });
            },
        );
    }));
    let seen = seen.borrow();
    let t = Color::Transparent;
    let off = |c: Color| faded(c, 0.38);
    let ten = |c: Color| faded(c, 0.1);
    let plain = IconButtonColors::new(t, around, t, off(around));
    let vibrant = IconButtonColors::new(t, cs.on_surface_variant, t, off(cs.on_surface));
    assert_eq!(seen[0], (plain, IconToggleButtonColors::new(t, around, t, off(around), t, cs.primary)));
    assert_eq!(
        seen[1],
        (vibrant, IconToggleButtonColors::new(t, cs.on_surface_variant, t, off(cs.on_surface), t, cs.primary))
    );
    assert_eq!(
        seen[2],
        (
            IconButtonColors::new(cs.primary, cs.on_primary, ten(cs.on_surface), off(cs.on_surface)),
            IconToggleButtonColors::new(
                cs.surface_container,
                cs.on_surface_variant,
                ten(cs.on_surface),
                off(cs.on_surface),
                cs.primary,
                cs.on_primary
            )
        )
    );
    assert_eq!(
        seen[3],
        (
            IconButtonColors::new(
                cs.secondary_container,
                cs.on_secondary_container,
                ten(cs.on_surface),
                off(cs.on_surface)
            ),
            IconToggleButtonColors::new(
                cs.secondary_container,
                cs.on_secondary_container,
                ten(cs.on_surface),
                off(cs.on_surface),
                cs.secondary,
                cs.on_secondary
            )
        )
    );
    assert_eq!(
        seen[4],
        (plain, IconToggleButtonColors::new(t, around, t, off(around), cs.inverse_surface, cs.inverse_on_surface))
    );
    assert_eq!(
        seen[5],
        (
            vibrant,
            IconToggleButtonColors::new(
                t,
                cs.on_surface_variant,
                t,
                off(cs.on_surface),
                cs.inverse_surface,
                cs.inverse_on_surface
            )
        )
    );
}

fn colors_given_take_kotlins_defaults_for_the_rest() {
    // The overloads with parameters: what isn't given is Kotlin's default, not the theme's (contentColorFor of the
    // container, the content color at 38%).
    let cs = light_color_scheme();
    let around = Color::hex(0x112233);
    let seen: Rc<RefCell<Vec<IconButtonColors>>> = Rc::default();
    let toggles: Rc<RefCell<Vec<IconToggleButtonColors>>> = Rc::default();
    let (s, ts) = (seen.clone(), toggles.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, ts) = (s.clone(), ts.clone());
        MaterialTheme(
            content = move || {
                let (s, ts) = (s.clone(), ts.clone());
                KompozitionLocalProvider(&[LocalContentColor.provides(around)], move || {
                    let d = IconButtonDefaults;
                    let mut s = s.borrow_mut();
                    s.clear();
                    // (a scheme color's content color)
                    s.push(d.filled_icon_button_colors(container_color = Some(cs.tertiary)));
                    // (an unspecified container: the default one, the content color around its content)
                    s.push(d.filled_icon_button_colors(disabled_container_color = Some(Color::Red)));
                    s.push(d.icon_button_colors(content_color = Some(Color::Blue)));
                    s.push(d.icon_button_vibrant_colors(container_color = Some(Color::Green)));
                    s.push(d.outlined_icon_button_colors(container_color = Some(Color::Green)));
                    s.push(d.filled_tonal_icon_button_colors(container_color = Some(Color::Yellow)));
                    s.push(d.outlined_icon_button_vibrant_colors(content_color = Some(Color::Red)));
                    let mut ts = ts.borrow_mut();
                    ts.clear();
                    ts.push(d.filled_icon_toggle_button_colors(checked_container_color = Some(cs.secondary)));
                    ts.push(d.filled_tonal_icon_toggle_button_colors(container_color = Some(cs.primary)));
                    ts.push(d.outlined_icon_toggle_button_colors(checked_container_color = Some(cs.error)));
                    ts.push(d.icon_toggle_button_vibrant_colors(checked_content_color = Some(Color::Red)));
                    ts.push(d.outlined_icon_toggle_button_vibrant_colors(checked_container_color = Some(Color::Blue)));
                    ts.push(d.icon_toggle_button_colors(disabled_content_color = Some(Color::Red)));
                });
            },
        );
    }));
    let seen = seen.borrow();
    let t = Color::Transparent;
    assert_eq!(
        seen[0],
        IconButtonColors::new(cs.tertiary, cs.on_tertiary, faded(cs.on_surface, 0.1), faded(cs.on_surface, 0.38))
    );
    assert_eq!(seen[1], IconButtonColors::new(cs.primary, around, Color::Red, faded(cs.on_surface, 0.38)));
    assert_eq!(seen[2], IconButtonColors::new(t, Color::Blue, t, faded(Color::Blue, 0.38)));
    // (an unspecified content color at 38% is a color, as Compose's `Color.Unspecified.copy(alpha = 0.38f)`)
    let unspecified_faded = faded(Color::Unspecified, 0.38);
    assert!(unspecified_faded.is_specified());
    assert_eq!(seen[3], IconButtonColors::new(Color::Green, cs.on_surface_variant, t, unspecified_faded));
    assert_eq!(seen[4], IconButtonColors::new(Color::Green, around, t, faded(around, 0.38)));
    // (a color of none of the scheme's: the content color around)
    assert_eq!(seen[5].content_color, around);
    assert_eq!(seen[6], IconButtonColors::new(t, Color::Red, t, faded(Color::Red, 0.38)));
    let toggles = toggles.borrow();
    assert_eq!((toggles[0].checked_container_color, toggles[0].checked_content_color), (cs.secondary, cs.on_secondary));
    assert_eq!((toggles[0].container_color, toggles[0].content_color), (cs.surface_container, cs.on_surface_variant));
    assert_eq!((toggles[1].container_color, toggles[1].content_color), (cs.primary, cs.on_primary));
    // (the checked container's content color: none given is the content color around)
    assert_eq!((toggles[1].checked_container_color, toggles[1].checked_content_color), (cs.secondary, around));
    assert_eq!((toggles[2].checked_container_color, toggles[2].checked_content_color), (cs.error, cs.on_error));
    assert_eq!(toggles[3].checked_content_color, Color::Red);
    assert_eq!(toggles[3].disabled_content_color, faded(Color::Unspecified, 0.38));
    assert_eq!((toggles[4].checked_container_color, toggles[4].checked_content_color), (Color::Blue, around));
    assert_eq!((toggles[5].content_color, toggles[5].disabled_content_color), (around, Color::Red));
}

fn colors_copy_keeps_what_is_unspecified() {
    let colors = IconButtonColors::new(Color::Red, Color::Green, Color::Blue, Color::Black);
    assert_eq!(colors, IconButtonColors::new(Color::Red, Color::Green, Color::Blue, Color::Black));
    let copied = named_args!(colors.copy(content_color = Color::White, disabled_container_color = Color::Unspecified));
    assert_eq!(copied, IconButtonColors::new(Color::Red, Color::White, Color::Blue, Color::Black));
    assert_eq!(colors.copy(), colors);
    let toggles =
        IconToggleButtonColors::new(Color::Red, Color::Green, Color::Blue, Color::Black, Color::White, Color::Yellow);
    let copied = named_args!(toggles.copy(checked_content_color = Color::Cyan, container_color = Color::Unspecified));
    assert_eq!(
        copied,
        IconToggleButtonColors::new(Color::Red, Color::Green, Color::Blue, Color::Black, Color::White, Color::Cyan)
    );
    assert_ne!(copied, toggles);
    // (Kotlin's properties)
    assert_eq!((copied.checked_container_color(), colors.disabled_content_color()), (Color::White, Color::Black));
}

fn icon_toggle_buttons_draw_the_colors_of_their_state() {
    // Each state's colors: disabled wins over checked; unchecked and checked as given.
    let colors = IconToggleButtonColors::new(
        Color::hex(0x010101),
        Color::hex(0x020202),
        Color::hex(0x030303),
        Color::hex(0x040404),
        Color::hex(0x050505),
        Color::hex(0x060606),
    );
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Row(move || {
            for (checked, enabled) in [(false, true), (true, true), (false, false), (true, false)] {
                IconToggleButton(
                    checked,
                    |_| {},
                    enabled = enabled,
                    colors = colors,
                    content = || icon(Icons::Filled::Star(), "y"),
                );
                FilledIconToggleButton(
                    checked,
                    |_| {},
                    enabled = enabled,
                    colors = colors,
                    content = || icon(Icons::Filled::Star(), "y"),
                );
            }
        });
    }));
    let canvas = rule.draw();
    let c = Color::hex;
    assert_eq!(
        canvas.rect_colors,
        vec![c(0x010101), c(0x010101), c(0x050505), c(0x050505), c(0x030303), c(0x030303), c(0x030303), c(0x030303)]
    );
    assert_eq!(
        canvas.vector_colors,
        vec![c(0x020202), c(0x020202), c(0x060606), c(0x060606), c(0x040404), c(0x040404), c(0x040404), c(0x040404)]
    );
}

fn outlined_borders_follow_enabled_and_checked() {
    let cs = dark_color_scheme();
    let around = Color::hex(0x445566);
    let seen: Rc<RefCell<Vec<Option<BorderStroke>>>> = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            color_scheme = dark_color_scheme(),
            content = move || {
                let s = s.clone();
                KompozitionLocalProvider(&[LocalContentColor.provides(around)], move || {
                    let mut s = s.borrow_mut();
                    s.clear();
                    s.push(Some(IconButtonDefaults::outlined_icon_button_border(true)));
                    s.push(Some(IconButtonDefaults::outlined_icon_button_border(false)));
                    s.push(Some(IconButtonDefaults::outlined_icon_button_vibrant_border(true)));
                    s.push(Some(IconButtonDefaults::outlined_icon_button_vibrant_border(false)));
                    s.push(IconButtonDefaults::outlined_icon_toggle_button_border(true, false));
                    s.push(IconButtonDefaults::outlined_icon_toggle_button_border(true, true));
                    s.push(IconButtonDefaults::outlined_icon_toggle_button_vibrant_border(false, false));
                    s.push(IconButtonDefaults::outlined_icon_toggle_button_vibrant_border(true, true));
                    drop(s);
                    Row(|| {
                        // (the vibrant border by default with shapes; none where none given)
                        OutlinedIconToggleButtonWithShapes(
                            false,
                            |_| {},
                            IconButtonDefaults.toggleable_shapes(),
                            content = || icon(Icons::Filled::Star(), "titreşimli"),
                        );
                        OutlinedIconButton(|| {}, border = Some(None), content = || icon(Icons::Filled::Star(), "yok"));
                        OutlinedIconButtonWithShapes(
                            || {},
                            IconButtonDefaults.shapes(),
                            border = Some(Some(BorderStroke(Dp(3.0), Color::Red))),
                            content = || icon(Icons::Filled::Star(), "kırmızı"),
                        );
                    });
                });
            },
        );
    }));
    let seen = seen.borrow();
    let b = |c: Color| Some(BorderStroke(Dp(1.0), c));
    assert_eq!(
        *seen,
        vec![
            b(around),
            b(faded(around, 0.38)),
            b(cs.outline_variant),
            b(faded(cs.outline_variant, 0.38)),
            b(around),
            None,
            b(faded(cs.outline_variant, 0.38)),
            None,
        ]
    );
    let canvas = rule.draw();
    assert_eq!(canvas.stroke_colors, vec![cs.outline_variant, Color::Red]);
    // (the vibrant outlined toggle one's content in on surface variant)
    assert_eq!(canvas.vector_colors[0], cs.on_surface_variant);
}

fn shapes_defaults_constructors_and_copy() {
    let seen: Rc<RefCell<Option<(IconButtonShapes, IconToggleButtonShapes, IconButtonShapes)>>> = Rc::default();
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                let d = IconButtonDefaults;
                let cut: Arc<dyn Shape> = Arc::new(CutCornerShape(4.dp()));
                *s.borrow_mut() =
                    Some((d.shapes(), d.toggleable_shapes(), named_args!(d.shapes(pressed_shape = Some(cut)))));
            },
        );
    }));
    let (shapes, toggleable, given) = seen.borrow().clone().unwrap();
    let theme = Shapes::default();
    let circle: Arc<dyn Shape> = Arc::new(CircleShape);
    let small: Arc<dyn Shape> = Arc::new(theme.small.clone());
    let medium: Arc<dyn Shape> = Arc::new(theme.medium.clone());
    assert_eq!(shapes, IconButtonShapes::new(circle.clone(), Some(small.clone())));
    assert_eq!(toggleable, IconToggleButtonShapes::new(circle.clone(), Some(small.clone()), Some(medium.clone())));
    assert_eq!(*given.shape, *circle);
    let cut: Arc<dyn Shape> = Arc::new(CutCornerShape(4.dp()));
    assert_eq!(*given.pressed_shape, *cut);
    // (Kotlin's constructor: the pressed and checked shapes the shape by default)
    let only = IconButtonShapes(circle.clone());
    assert!(Arc::ptr_eq(&only.shape, &only.pressed_shape));
    let same = IconToggleButtonShapes(circle.clone());
    assert!(Arc::ptr_eq(&same.shape, &same.pressed_shape) && Arc::ptr_eq(&same.shape, &same.checked_shape));
    // (its named form, `IconToggleButtonShapes(shape, checked_shape = …)`, reached through the API index once it
    // is regenerated: the same as this)
    let toggle_only = IconToggleButtonShapes::new(circle.clone(), None, Some(medium.clone()));
    assert!(Arc::ptr_eq(&toggle_only.shape, &toggle_only.pressed_shape));
    assert_eq!(*toggle_only.checked_shape, *medium);
    // (copy: None keeps; equality by what the shapes are)
    let copied = named_args!(shapes.copy(shape = Some(medium.clone())));
    assert_eq!(copied, IconButtonShapes::new(medium.clone(), Some(small.clone())));
    assert_eq!(shapes.copy(), shapes);
    assert_ne!(copied, shapes);
    let toggle_copy = named_args!(toggleable.copy(checked_shape = Some(small.clone())));
    assert_eq!(*toggle_copy.checked_shape, *small);
    assert_eq!(*toggle_copy.pressed_shape, *small);
    // (Kotlin's properties)
    assert_eq!(*toggleable.checked_shape(), *medium);
}

/// The corner radius (top left) of the last 40 dp wide clip drawn.
fn last_corner(rule: &KompozTestRule) -> f32 {
    let canvas = rule.draw();
    let clip = canvas.clip_shapes.iter().rev().find(|c| near(c.bounds.width(), 40.0, 0.5)).expect("a 40 dp clip");
    clip.radii.top_left
}

fn icon_buttons_with_shapes_morph_as_they_are_pressed() {
    // Compose's shapes overload: round at rest, the theme's small corners (8 dp) pressed, the corners animating
    // between (the default effects spring: no bounce), and back.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                IconButtonWithShapes(
                    || {},
                    IconButtonDefaults.shapes(),
                    modifier = Modifier.test_tag("biçim"),
                    content = || icon(Icons::Filled::Lock(), "kilit"),
                );
            },
        );
    }));
    assert!(near(last_corner(&rule), 20.0, 0.01));
    let at = bounds(&rule, "biçim").center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    // (frame by frame: the corners pass between, no frame past either end)
    let mut corners = Vec::new();
    for _ in 0..30 {
        rule.main_clock().advance_time_by_frame();
        corners.push(last_corner(&rule));
    }
    assert!(corners.iter().any(|c| *c < 19.9 && *c > 8.1), "{corners:?}");
    assert!(corners.iter().all(|c| *c <= 20.01 && *c >= 7.99), "{corners:?}");
    rule.advance_time_by(1000);
    let pressed = last_corner(&rule);
    assert!(near(pressed, 8.0, 0.05), "{pressed}");
    rule.host().pointer_up(at, -1, 0);
    rule.advance_time_by(1000);
    assert!(near(last_corner(&rule), 20.0, 0.05));
}

fn icon_toggle_buttons_with_shapes_take_their_checked_shape() {
    let checked = mutable_state_of(false);
    let c = checked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        MaterialTheme(
            content = move || {
                let c2 = c.clone();
                IconToggleButtonWithShapes(
                    c.get(),
                    move |v| c2.set(v),
                    IconButtonDefaults.toggleable_shapes(),
                    modifier = Modifier.test_tag("biçim"),
                    content = || icon(Icons::Filled::Lock(), "kilit"),
                );
            },
        );
    }));
    assert!(near(last_corner(&rule), 20.0, 0.01));
    rule.on_node_with_tag("biçim").perform_click();
    rule.advance_time_by(1000);
    assert!(checked.get());
    // (checked: the theme's medium corners, 12 dp)
    assert!(near(last_corner(&rule), 12.0, 0.05));
    // (the vibrant colors by default: checked in primary, no container)
    let canvas = rule.draw();
    assert_eq!(canvas.vector_colors, vec![light_color_scheme().primary]);
    // (static shapes don't animate: one shape)
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let square: Arc<dyn Shape> = Arc::new(RoundedCornerShape(4.dp()));
        IconToggleButtonWithShapes(
            true,
            |_| {},
            IconToggleButtonShapes(square),
            content = || icon(Icons::Filled::Lock(), "kilit"),
        );
    }));
    assert!(near(last_corner(&rule), 4.0, 0.01));
}

fn every_kind_with_shapes_clicks_and_toggles() {
    let clicks = Rc::new(Cell::new(0));
    let toggles: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (c, t) = (clicks.clone(), toggles.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, t) = (c.clone(), t.clone());
        MaterialTheme(
            content = move || {
                let (c, t) = (c.clone(), t.clone());
                Column(move || {
                    let click = || {
                        let c = c.clone();
                        move || c.set(c.get() + 1)
                    };
                    let toggle = || {
                        let t = t.clone();
                        move |v: bool| t.borrow_mut().push(v)
                    };
                    let d = IconButtonDefaults;
                    FilledIconButtonWithShapes(click(), d.shapes(), content = || icon(Icons::Filled::Add(), "1"));
                    FilledTonalIconButtonWithShapes(click(), d.shapes(), content = || icon(Icons::Filled::Add(), "2"));
                    OutlinedIconButtonWithShapes(click(), d.shapes(), content = || icon(Icons::Filled::Add(), "3"));
                    IconButtonWithShapes(
                        click(),
                        d.shapes(),
                        enabled = false,
                        content = || icon(Icons::Filled::Add(), "4"),
                    );
                    FilledIconToggleButtonWithShapes(
                        false,
                        toggle(),
                        d.toggleable_shapes(),
                        content = || icon(Icons::Filled::Add(), "5"),
                    );
                    FilledTonalIconToggleButtonWithShapes(
                        true,
                        toggle(),
                        d.toggleable_shapes(),
                        content = || icon(Icons::Filled::Add(), "6"),
                    );
                    OutlinedIconToggleButtonWithShapes(
                        false,
                        toggle(),
                        d.toggleable_shapes(),
                        content = || icon(Icons::Filled::Add(), "7"),
                    );
                });
            },
        );
    }));
    for d in ["1", "2", "3", "4", "5", "6", "7"] {
        rule.on_node_with_content_description(d).perform_click();
    }
    assert_eq!(clicks.get(), 3);
    assert_eq!(*toggles.borrow(), vec![true, false, true]);
    let node = rule.on_node_with_content_description("6").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Checkbox));
    let node = rule.on_node_with_content_description("2").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button));
}

fn expressive_shapes_are_the_tokens() {
    let seen: Rc<RefCell<Vec<Arc<dyn Shape>>>> = Rc::default();
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                type D = IconButtonDefaults;
                #[allow(deprecated)]
                let all = vec![
                    D::standard_shape(),
                    D::filled_shape(),
                    D::outlined_shape(),
                    D::extra_small_round_shape(),
                    D::extra_small_square_shape(),
                    D::extra_small_pressed_shape(),
                    D::extra_small_selected_round_shape(),
                    D::extra_small_selected_square_shape(),
                    D::small_round_shape(),
                    D::small_square_shape(),
                    D::small_pressed_shape(),
                    D::small_selected_round_shape(),
                    D::small_selected_square_shape(),
                    D::SmallSelectedSquareShape(),
                    D::medium_round_shape(),
                    D::medium_square_shape(),
                    D::medium_pressed_shape(),
                    D::medium_selected_round_shape(),
                    D::medium_selected_square_shape(),
                    D::large_round_shape(),
                    D::large_square_shape(),
                    D::large_pressed_shape(),
                    D::large_selected_round_shape(),
                    D::large_selected_square_shape(),
                    D::extra_large_round_shape(),
                    D::extra_large_square_shape(),
                    D::extra_large_pressed_shape(),
                    D::extra_large_selected_round_shape(),
                    D::extra_large_selected_square_shape(),
                ];
                *s.borrow_mut() = all;
            },
        );
    }));
    let t = Shapes::default();
    let circle: Arc<dyn Shape> = Arc::new(CircleShape);
    let [small, medium, large, extra_large]: [Arc<dyn Shape>; 4] = [
        Arc::new(t.small.clone()),
        Arc::new(t.medium.clone()),
        Arc::new(t.large.clone()),
        Arc::new(t.extra_large.clone()),
    ];
    let c = || circle.clone();
    let expected = vec![
        c(),
        c(),
        c(),
        // extra small: round, square, pressed, selected round, selected square
        c(),
        medium.clone(),
        small.clone(),
        medium.clone(),
        c(),
        // small (and the deprecated name)
        c(),
        medium.clone(),
        small.clone(),
        medium.clone(),
        c(),
        c(),
        // medium
        c(),
        large.clone(),
        medium.clone(),
        large.clone(),
        c(),
        // large
        c(),
        extra_large.clone(),
        large.clone(),
        extra_large.clone(),
        c(),
        // extra large
        c(),
        extra_large.clone(),
        large.clone(),
        extra_large.clone(),
        c(),
    ];
    let seen = seen.borrow();
    assert_eq!(seen.len(), expected.len());
    for (k, (a, b)) in seen.iter().zip(&expected).enumerate() {
        assert!(**a == **b, "{k}: {a:?} != {b:?}");
    }
}

fn the_ripple_is_bounded_by_the_shape_in_the_content_color() {
    // The standard icon button's ripple: bounded, cut to its shape (a 40 dp circle, not its 48 dp of room), in the
    // content color around.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                KompozitionLocalProvider(&[LocalContentColor.provides(Color::Blue)], || {
                    IconButton(
                        || {},
                        modifier = Modifier.test_tag("dalga"),
                        content = || icon(Icons::Filled::Add(), "a"),
                    );
                });
            },
        );
    }));
    let at = bounds(&rule, "dalga");
    rule.host().pointer_down(at.center(), -1, 0, PointerType::Touch);
    rule.advance_time_by(100);
    let canvas = rule.draw();
    let clip = canvas.clip_shapes.iter().find(|c| near(c.bounds.width(), 40.0, 0.5)).expect("the circle's clip");
    assert!(near(clip.bounds.left, at.left, 0.5) && near(clip.radii.top_left, 20.0, 0.5));
    let ripple = canvas
        .rect_colors
        .iter()
        .chain(&canvas.path_colors)
        .copied()
        .find(|c| c.alpha() > 0.0 && c.alpha() < 1.0)
        .expect("a ripple");
    assert!(ripple.blue() == 1.0 && ripple.red() == 0.0 && near(ripple.alpha(), 0.1, 0.06), "{ripple:?}");
    rule.host().pointer_up(at.center(), -1, 0);
    rule.wait_for_idle();
}

#[allow(deprecated)]
fn deprecated_overloads_are_the_standard_shapes() {
    let toggled = mutable_state_of(false);
    let t = toggled.clone();
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (t, c) = (t.clone(), c.clone());
        Row(move || {
            let c = c.clone();
            IconButtonWithoutShape(
                move || c.set(c.get() + 1),
                modifier = Modifier.test_tag("eski"),
                content = || icon(Icons::Filled::Add(), "a"),
            );
            let t2 = t.clone();
            IconToggleButtonWithoutShape(
                t.get(),
                move |v| t2.set(v),
                modifier = Modifier.test_tag("eski aç"),
                content = || icon(Icons::Filled::Add(), "b"),
            );
        });
    }));
    rule.on_node_with_tag("eski").perform_click();
    rule.on_node_with_tag("eski aç").perform_click();
    assert!(clicks.get() == 1 && toggled.get());
    let canvas = rule.draw();
    let circles = canvas.clip_shapes.iter().filter(|c| near(c.radii.top_left, 20.0, 0.5)).count();
    assert_eq!(circles, 2);
}

// ---- M15's samples (IconButtonSamples.kt) ----------------------------------------------------------------------
//
// Each sample wraps its button in a `TooltipBox` with a `PlainTooltip` (for accessibility): the tooltips are WP7's,
// so the buttons are composed here without them.

const LOCALIZED_DESCRIPTION: &str = "Localized description";

fn icon_button_samples() {
    // IconButtonSample, IconButtonWithAnimatedShapeSample, FilledIconButtonSample(+WithAnimatedShape),
    // FilledTonalIconButtonSample(+WithAnimatedShape), OutlinedIconButtonSample(+WithAnimatedShape),
    // TintedIconButtonSample.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    let lock = || VectorIcon(Icons::Filled::Lock(), Some(LOCALIZED_DESCRIPTION.into()));
                    IconButton(|| { /* doSomething() */ }, content = lock);
                    IconButtonWithShapes(|| {}, IconButtonDefaults.shapes(), content = lock);
                    FilledIconButton(|| {}, content = lock);
                    FilledIconButtonWithShapes(|| {}, IconButtonDefaults.shapes(), content = lock);
                    FilledTonalIconButton(|| {}, content = lock);
                    FilledTonalIconButtonWithShapes(|| {}, IconButtonDefaults.shapes(), content = lock);
                    OutlinedIconButton(|| {}, content = lock);
                    OutlinedIconButtonWithShapes(|| {}, IconButtonDefaults.shapes(), content = lock);
                    IconButton(
                        || {},
                        content = || {
                            let painter: Rc<dyn Painter> = Rc::new(remember_vector_painter(Icons::Filled::Lock()));
                            Icon(painter, Some(LOCALIZED_DESCRIPTION.into()), tint = Color::Red);
                        },
                    );
                });
            },
        );
    }));
    let buttons =
        rule.on_all_nodes(has_content_description(LOCALIZED_DESCRIPTION, false, false)).fetch_semantics_nodes();
    assert_eq!(buttons.len(), 9);
    let cs = light_color_scheme();
    let canvas = rule.draw();
    // (MaterialTheme gives no content color: the standard and outlined ones' content in the default one, black)
    assert_eq!(
        canvas.vector_colors,
        vec![
            Color::Black,
            Color::Black,
            cs.on_primary,
            cs.on_primary,
            cs.on_secondary_container,
            cs.on_secondary_container,
            Color::Black,
            Color::Black,
            Color::Red,
        ]
    );
    assert_eq!(canvas.stroke_colors, vec![Color::Black, Color::Black]);
}

fn icon_toggle_button_samples() {
    // IconToggleButtonSample(+WithAnimatedShape), FilledIconToggleButtonSample(+WithAnimatedShape),
    // FilledTonalIconToggleButtonSample(+WithAnimatedShape), OutlinedIconToggleButtonSample(+WithAnimatedShape):
    // a click checks each, its icon the filled lock then (the outlined one before).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    for kind in 0..8 {
                        let checked = remember(|| mutable_state_of(false));
                        let c = checked.clone();
                        let on_checked_change = move |v: bool| c.set(v);
                        let is_checked = checked.get();
                        let content = move || {
                            if is_checked {
                                VectorIcon(Icons::Filled::Lock(), Some(LOCALIZED_DESCRIPTION.into()));
                            } else {
                                VectorIcon(Icons::Outlined::Lock(), Some(LOCALIZED_DESCRIPTION.into()));
                            }
                        };
                        let modifier = Modifier.test_tag(kind.to_string());
                        let shapes = IconButtonDefaults.toggleable_shapes();
                        match kind {
                            0 => {
                                IconToggleButton(is_checked, on_checked_change, modifier = modifier, content = content)
                            }
                            1 => IconToggleButtonWithShapes(
                                is_checked,
                                on_checked_change,
                                shapes,
                                modifier = modifier,
                                content = content,
                            ),
                            2 => FilledIconToggleButton(
                                is_checked,
                                on_checked_change,
                                modifier = modifier,
                                content = content,
                            ),
                            3 => FilledIconToggleButtonWithShapes(
                                is_checked,
                                on_checked_change,
                                shapes,
                                modifier = modifier,
                                content = content,
                            ),
                            4 => FilledTonalIconToggleButton(
                                is_checked,
                                on_checked_change,
                                modifier = modifier,
                                content = content,
                            ),
                            5 => FilledTonalIconToggleButtonWithShapes(
                                is_checked,
                                on_checked_change,
                                shapes,
                                modifier = modifier,
                                content = content,
                            ),
                            6 => OutlinedIconToggleButton(
                                is_checked,
                                on_checked_change,
                                modifier = modifier,
                                content = content,
                            ),
                            _ => OutlinedIconToggleButtonWithShapes(
                                is_checked,
                                on_checked_change,
                                shapes,
                                modifier = modifier,
                                content = content,
                            ),
                        }
                    }
                });
            },
        );
    }));
    let outlined_lock = Icons::Outlined::Lock().id();
    let filled_lock = Icons::Filled::Lock().id();
    assert!(rule.draw().vectors.iter().all(|v| *v == outlined_lock));
    for kind in 0..8 {
        let tag = kind.to_string();
        rule.on_node_with_tag(&tag).assert_is_off().perform_click();
        rule.on_node_with_tag(&tag).assert_is_on();
    }
    rule.wait_for_idle();
    assert!(rule.draw().vectors.iter().all(|v| *v == filled_lock));
}

fn sized_icon_button_samples() {
    // ExtraSmallNarrowSquareIconButtonsSample, MediumRoundWideIconButtonSample,
    // LargeRoundUniformOutlinedIconButtonSample: Expressive's sizes through the container sizes, shapes and icon sizes.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    // (small narrow round icon button, Compose's comment says: an extra small narrow square one)
                    FilledIconButton(
                        || {},
                        modifier = Modifier
                            .minimum_interactive_component_size()
                            .size_dp(
                                IconButtonDefaults
                                    .extra_small_container_size(width_option = IconButtonWidthOption::Narrow),
                            )
                            .test_tag("xs"),
                        shape = IconButtonDefaults::extra_small_square_shape(),
                        content = || {
                            VectorIcon(
                                Icons::Filled::Lock(),
                                Some(LOCALIZED_DESCRIPTION.into()),
                                modifier = Modifier.size(IconButtonDefaults::ExtraSmallIconSize).test_tag("xs icon"),
                            );
                        },
                    );
                    IconButton(
                        || {},
                        modifier = Modifier
                            .size_dp(
                                IconButtonDefaults.medium_container_size(width_option = IconButtonWidthOption::Wide),
                            )
                            .test_tag("m"),
                        shape = IconButtonDefaults::medium_round_shape(),
                        content = || {
                            VectorIcon(
                                Icons::Filled::Lock(),
                                Some(LOCALIZED_DESCRIPTION.into()),
                                modifier = Modifier.size(IconButtonDefaults::MediumIconSize).test_tag("m icon"),
                            );
                        },
                    );
                    OutlinedIconButton(
                        || {},
                        modifier = Modifier.size_dp(IconButtonDefaults.large_container_size()).test_tag("l"),
                        shape = IconButtonDefaults::large_round_shape(),
                        content = || {
                            VectorIcon(
                                Icons::Filled::Lock(),
                                Some(LOCALIZED_DESCRIPTION.into()),
                                modifier = Modifier.size(IconButtonDefaults::LargeIconSize).test_tag("l icon"),
                            );
                        },
                    );
                });
            },
        );
    }));
    let size = |tag: &str| {
        let b = rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root();
        (b.width(), b.height())
    };
    // (the extra small narrow one: 28 by 32, its tag after its size; the button in 48 dp of room)
    assert_eq!(size("xs"), (28.0, 32.0));
    assert_eq!(size("xs icon"), (20.0, 20.0));
    // (medium wide: 72 by 56; large uniform: 96 square, its outline around it)
    assert_eq!(size("m"), (72.0, 56.0));
    assert_eq!(size("m icon"), (24.0, 24.0));
    assert_eq!(size("l"), (96.0, 96.0));
    assert_eq!(size("l icon"), (32.0, 32.0));
    let canvas = rule.draw();
    let xs_container = canvas
        .rect_corners
        .iter()
        .zip(&canvas.rects)
        .find(|(_, r)| near(r.width(), 28.0, 0.5) && near(r.height(), 32.0, 0.5));
    // (the extra small square shape: the theme's medium corners, 12 dp)
    assert!(xs_container.is_some_and(|(c, _)| near(c.top_left, 12.0, 0.5)), "{:?}", canvas.rects);
    assert!(canvas.strokes.iter().any(|s| near(s.width(), 96.0, 1.0)));
}
