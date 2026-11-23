//! foundation's styles (Compose 1.12's experimental `foundation.style`), from Compose's semantics: a style's sizes,
//! paddings, border width and position laid out by `styleable`'s outer and inner nodes; its shadows, background,
//! content, foreground and border drawn in that order, in its shape; its layer properties; the state's blocks
//! following a state's interactions (pressed, hovered, focused) and its component's parameters (enabled, a custom
//! key); a state or a local read in a style changing only what it affects, nothing composed again; animated
//! properties interpolated as their animations run, back when the state ends, from where they are when
//! interrupted; text properties inherited by the text inside (ComposeFoundationFlags.isInheritedTextStyleEnabled).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::{AnimationSpec, LinearEasing, TweenSpec, tween};
use kompoz_foundation::interaction::{FocusInteraction, HoverInteraction, Interaction, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::TextStyle;
use kompoz_ui::state::ToggleableState;
use kompoz_ui::*;
use std::cell::Cell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn same_rect(r: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
    near(r.left, x) && near(r.top, y) && near(r.width(), w) && near(r.height(), h)
}

fn spec(millis: i32) -> Arc<dyn AnimationSpec<f32>> {
    let tween: TweenSpec<f32> = named_args!(tween(duration_millis = millis, easing = Arc::new(LinearEasing)));
    Arc::new(tween)
}

// ---- Layout ---------------------------------------------------------------------------------------------------

fn sizes_paddings_and_borders_lay_out() {
    // The outer node: the external padding around the style's size; the inner node: the border's width and the
    // content padding inside it.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.size(100.dp());
                s.external_padding(5.dp());
                s.border_width(2.dp());
                s.content_padding(10.dp());
            }),
            content = || Box(modifier = Modifier.fill_max_size().test_tag("içerik")),
        );
    }));
    assert!(same_rect(bounds(&rule, "içerik"), 17.0, 17.0, 76.0, 76.0), "{:?}", bounds(&rule, "içerik"));
}

fn fractions_bounds_and_positions() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            modifier = Modifier.size(200.dp()),
            content = || {
                // (half the room's width, the content's height)
                Box(modifier = Modifier
                    .styleable_with(None, |s: &dyn StyleScope| {
                        s.width_fraction(0.5);
                        s.height(10.dp());
                    })
                    .test_tag("yarım"));
                // (at least its minimum, at most its maximum)
                Box(
                    modifier = Modifier
                        .styleable_with(None, |s: &dyn StyleScope| {
                            s.min_size(DpSize::new(30.dp(), 20.dp()));
                            s.max_width(40.dp());
                        })
                        .test_tag("en az"),
                    content = || Box(modifier = Modifier.size(5.dp())),
                );
                Box(
                    modifier = Modifier
                        .styleable_with(None, |s: &dyn StyleScope| {
                            s.max_width(40.dp());
                        })
                        .test_tag("en çok"),
                    content = || Box(modifier = Modifier.size_wh(80.dp(), 10.dp())),
                );
                // (from the right: the room's width less its own and the distance)
                Box(
                    modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                        s.size(20.dp());
                        s.right(10.dp());
                    }),
                    content = || Box(modifier = Modifier.fill_max_size().test_tag("sağda")),
                );
            },
        );
    }));
    assert!(same_rect(bounds(&rule, "yarım"), 0.0, 0.0, 100.0, 10.0), "{:?}", bounds(&rule, "yarım"));
    assert!(same_rect(bounds(&rule, "en az"), 0.0, 10.0, 30.0, 20.0), "{:?}", bounds(&rule, "en az"));
    assert!(near(bounds(&rule, "en çok").width(), 40.0));
    assert!(same_rect(bounds(&rule, "sağda"), 170.0, 40.0, 20.0, 20.0), "{:?}", bounds(&rule, "sağda"));
}

// ---- Drawing --------------------------------------------------------------------------------------------------

fn drawn_in_order_in_the_shape() {
    // Drop shadows, the background, the content, the foreground, the border, inner shadows.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.size(40.dp());
                s.drop_shadow(Shadow::new(Color::Black, Offset::Zero, 4.0, 0.0));
                s.background(Color::Red);
                s.foreground(named_args!(Color::Blue.copy(alpha = 0.5)));
                s.border(2.dp(), Color::Green);
                s.inner_shadow(Shadow::new(Color::Black, Offset::Zero, 2.0, 0.0));
            }),
            content = || Box(modifier = Modifier.size(10.dp()).background(Color::Yellow)),
        );
    }));
    let c = rule.draw();
    let reds = c.rect_colors.iter().position(|&x| x == Color::Red).expect("the background");
    let yellow = c.rect_colors.iter().position(|&x| x == Color::Yellow).expect("the content");
    let blue = c.rect_colors.iter().position(|&x| x.alpha() < 0.6 && x.blue() > 0.9).expect("the foreground");
    assert!(reds < yellow && yellow < blue, "{:?}", c.rect_colors);
    // (the border: a rect stroked along its inside, 1 px in, which the canvas records as the bounds it covers)
    assert!(c.strokes.len() == 1 && same_rect(c.strokes[0], 0.0, 0.0, 40.0, 40.0), "{:?}", c.strokes);
    assert_eq!(c.stroke_colors, vec![Color::Green]);
    // (the drop shadow under the background, the inner one over the content)
    assert_eq!(c.shadows.len(), 2);
    assert!(!c.shadows[0].inner && c.shadows[1].inner);
    assert!(c.order.starts_with('s') && c.order.ends_with('s'), "{}", c.order);
}

fn a_shape_and_a_brush() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
            s.size(40.dp());
            s.shape(Arc::new(RoundedCornerShape(8.0f32)));
            s.background_brush(Brush::vertical_gradient(
                &[Color::Red, Color::Blue],
                0.0,
                f32::INFINITY,
                TileMode::Clamp,
            ));
        }));
    }));
    let c = rule.draw();
    assert_eq!(c.gradients, 1);
}

fn borders_of_every_outline() {
    // (Compose's BorderLogic: a rounded rect of one radius stroked; one of several radii a ring path; a generic
    // outline stroked twice the width in an offscreen layer, less what is outside it)
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.size(20.dp());
                s.shape(Arc::new(RoundedCornerShape(6.0f32)));
                s.border(2.dp(), Color::Red);
            }));
            Box(modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.size(20.dp());
                s.shape(Arc::new(RoundedCornerShape::new(2.0f32, 4.0f32, 6.0f32, 8.0f32)));
                s.border(2.dp(), Color::Green);
            }));
            Box(modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.size(20.dp());
                s.shape(Arc::new(GenericShape(|path: &mut Path, size: Size, _| {
                    path.move_to(size.width / 2.0, 0.0);
                    path.line_to(size.width, size.height);
                    path.line_to(0.0, size.height);
                    path.close();
                })));
                s.border(2.dp(), Color::Blue);
            }));
        });
    }));
    let c = rule.draw();
    assert_eq!(c.stroke_colors, vec![Color::Red]);
    assert!(c.path_colors.contains(&Color::Green), "{:?}", c.path_colors);
    // (the triangle's border recorded into a layer drawn offscreen)
    assert!(c.path_colors.contains(&Color::Blue), "{:?}", c.path_colors);
    assert!(c.layers > 0);
}

fn layer_properties() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
            s.size(40.dp());
            s.alpha(0.5);
            s.background(Color::Red);
        }));
        Box(modifier = Modifier
            .styleable_with(None, |s: &dyn StyleScope| {
                s.size(20.dp());
                s.scale(2.0);
            })
            .test_tag("büyük"));
    }));
    let c = rule.draw();
    assert!(c.layer_alphas.iter().any(|&a| near(a, 0.5)), "{:?}", c.layer_alphas);
    // (the background in the layer too: a layout node draws in the space of what it wraps, placed with the layer)
    let red = c.rect_colors.iter().position(|&x| x == Color::Red).expect("the background");
    assert!(c.rect_depths[red] >= 1, "{:?}", c.rect_depths);
    // (scaled about its middle: twice its size around it)
    assert!(same_rect(bounds(&rule, "büyük"), -10.0, -10.0, 40.0, 40.0), "{:?}", bounds(&rule, "büyük"));
}

fn the_empty_style_adds_nothing() {
    assert!(Modifier.styleable_with(None, Style) == Modifier);
}

// ---- States ---------------------------------------------------------------------------------------------------

fn interactions_drive_the_states_blocks() {
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_updated_style_state(Some(s.as_interaction_source()));
        Box(modifier = Modifier.styleable_with(Some(state), |st: &dyn StyleScope| {
            st.size(10.dp());
            st.background(Color::White);
            st.hovered(|| st.background(Color::Yellow));
            st.focused(|| st.border(1.dp(), Color::Blue));
            st.pressed(|| st.background(Color::Red));
        }));
    }));
    let background = |rule: &KompozTestRule| rule.draw().rect_colors[0];
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::White);
    let hover = HoverInteraction::Enter::new();
    source.try_emit(hover);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Yellow);
    // (two presses: pressed until both end)
    let (p1, p2) = (PressInteraction::Press::new(Offset::Zero), PressInteraction::Press::new(Offset::Zero));
    source.try_emit(p1);
    source.try_emit(p2);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Red);
    source.try_emit(PressInteraction::Release::new(p1));
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Red);
    source.try_emit(PressInteraction::Cancel::new(p2));
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Yellow);
    source.try_emit(HoverInteraction::Exit::new(hover));
    let focus = FocusInteraction::Focus::new();
    source.try_emit(focus);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.rect_colors[0] == Color::White && c.strokes.len() == 1);
    source.try_emit(FocusInteraction::Unfocus::new(focus));
    rule.wait_for_idle();
    assert!(rule.draw().strokes.is_empty());
}

fn the_components_parameters_set_its_state() {
    // (rememberUpdatedStyleState's block: enabled, selected and the toggle from the component's parameters)
    let (enabled, selected, toggle) =
        (mutable_state_of(true), mutable_state_of(false), mutable_state_of(ToggleableState::Off));
    let (e, sel, t) = (enabled.clone(), selected.clone(), toggle.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (enabled, selected, toggle) = (e.get(), sel.get(), t.get());
        let state = named_args!(remember_updated_style_state(
            None,
            block = move |s: &MutableStyleState| {
                s.set_is_enabled(enabled);
                s.set_is_selected(selected);
                s.set_tri_state_toggle(toggle);
            }
        ));
        Box(modifier = Modifier.styleable_with(Some(state), |st: &dyn StyleScope| {
            st.size(10.dp());
            st.background(Color::White);
            st.selected(|| st.background(Color::Blue));
            st.checked(|| st.background(Color::Green));
            st.tri_state_toggle_indeterminate(|| st.background(Color::Cyan));
            st.disabled(|| st.background(Color::Gray));
        }));
    }));
    let background = |rule: &KompozTestRule| rule.draw().rect_colors[0];
    assert_eq!(background(&rule), Color::White);
    selected.set(true);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Blue);
    toggle.set(ToggleableState::On);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Green);
    toggle.set(ToggleableState::Indeterminate);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Cyan);
    enabled.set(false);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Gray);
}

/// An interaction of the app's own (a custom key's processor follows it).
#[derive(Debug)]
struct Playing(bool);
impl Interaction for Playing {}

fn custom_keys_and_their_interactions() {
    // (Compose's StyleStateKeySample: a state of the component's own, set from its parameters; and one whose value
    // an interaction implies, Compose's `processInteraction` override)
    let playing = StyleStateKey::new(false);
    let loud = StyleStateKey::with_interaction_processor(false, |interaction, state: MutableStyleState| {
        let on = interaction.downcast_ref::<Playing>().map(|p| p.0);
        Box::pin(async move {
            if let Some(on) = on {
                state.set(&LOUD.with(|l| l.borrow().clone()), on);
            }
        }) as Pin<Box<dyn std::future::Future<Output = ()>>>
    });
    LOUD.with(|l| *l.borrow_mut() = loud.clone());
    let source = MutableInteractionSource::new();
    let on = mutable_state_of(false);
    let (o, pk, lk, src) = (on.clone(), playing.clone(), loud.clone(), source.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (playing_now, pk, lk) = (o.get(), pk.clone(), lk.clone());
        let k = pk.clone();
        let l = lk.clone();
        let state = named_args!(remember_updated_style_state(
            Some(src.as_interaction_source()),
            block = move |s: &MutableStyleState| {
                s.set(&k, playing_now);
                // (the processed key is in the state: its interactions are followed)
                if !s.get(&l) {
                    s.set(&l, false);
                }
            }
        ));
        Box(modifier = Modifier.styleable_with(Some(state), move |st: &dyn StyleScope| {
            st.size(10.dp());
            st.background(Color::Gray);
            st.state_key(&pk, || st.background(Color::Green));
            st.state_key(&lk, || st.alpha(0.5));
        }));
    }));
    assert_eq!(rule.draw().rect_colors[0], Color::Gray);
    on.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Green);
    source.try_emit(Playing(true));
    rule.wait_for_idle();
    assert!(rule.draw().layer_alphas.iter().any(|&a| near(a, 0.5)));
}

thread_local! {
    static LOUD: std::cell::RefCell<StyleStateKey<bool>> = std::cell::RefCell::new(StyleStateKey::new(false));
}

// ---- What a style reads ---------------------------------------------------------------------------------------

fn a_state_read_changes_what_it_affects_without_composing() {
    let color = mutable_state_of(Color::Red);
    let padding = mutable_state_of(0.0f32);
    let compositions = Rc::new(Cell::new(0));
    let (c, p, n) = (color.clone(), padding.clone(), compositions.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        n.set(n.get() + 1);
        let (c, p) = (c.clone(), p.clone());
        Box(
            modifier = Modifier.styleable_with(None, move |s: &dyn StyleScope| {
                s.size(40.dp());
                s.background(c.get());
                s.content_padding(Dp(p.get()));
            }),
            content = || Box(modifier = Modifier.fill_max_size().test_tag("iç")),
        );
    }));
    let composed = compositions.get();
    assert_eq!(rule.draw().rect_colors[0], Color::Red);
    color.set(Color::Blue);
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Blue);
    padding.set(5.0);
    rule.wait_for_idle();
    assert!(same_rect(bounds(&rule, "iç"), 5.0, 5.0, 30.0, 30.0), "{:?}", bounds(&rule, "iç"));
    assert_eq!(compositions.get(), composed);
}

static LocalTint: KompozitionLocal<Color> = KompozitionLocal::new(|| Color::Black);

fn locals_read_where_the_style_is_applied() {
    let tint = mutable_state_of(Color::Red);
    let t = tint.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(&[LocalTint.provides(t.get())], || {
            Box(modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.size(10.dp());
                s.background(s.current_value(&LocalTint));
            }));
        });
    }));
    assert_eq!(rule.draw().rect_colors[0], Color::Red);
    tint.set(Color::Green);
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Green);
}

// ---- Animations -----------------------------------------------------------------------------------------------

fn animated_properties_follow_their_animations() {
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_updated_style_state(Some(s.as_interaction_source()));
        Box(
            modifier = Modifier.styleable_with(Some(state), |st: &dyn StyleScope| {
                st.height(10.dp());
                st.width(100.dp());
                st.background(Color::Blue);
                st.pressed(|| {
                    st.animate_spec(spec(100), || {
                        st.background(Color::Red);
                        st.width(200.dp());
                    });
                });
            }),
            content = || Box(modifier = Modifier.fill_max_size().test_tag("geniş")),
        );
    }));
    rule.wait_for_idle();
    rule.main_clock().set_auto_advance(false);
    let press = PressInteraction::Press::new(Offset::Zero);
    source.try_emit(press);
    rule.advance_frame();
    rule.advance_time_by(50);
    // (half way: a linear tween of 100 ms)
    let w = bounds(&rule, "geniş").width();
    assert!(w > 120.0 && w < 180.0, "{w}");
    let c = rule.draw().rect_colors[0];
    assert!(c.red() > 0.2 && c.red() < 0.8, "{c:?}");
    rule.advance_time_by(100);
    assert!(near(bounds(&rule, "geniş").width(), 200.0));
    assert_eq!(rule.draw().rect_colors[0], Color::Red);
    // (released: back, by the default spring, as the properties stay animated)
    source.try_emit(PressInteraction::Release::new(press));
    rule.advance_frame();
    rule.advance_time_by(32);
    let w = bounds(&rule, "geniş").width();
    assert!(w > 100.0 && w < 200.0, "{w}");
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "geniş").width(), 100.0));
    assert_eq!(rule.draw().rect_colors[0], Color::Blue);
}

fn an_interrupted_animation_goes_on_from_where_it_is() {
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_updated_style_state(Some(s.as_interaction_source()));
        Box(
            modifier = Modifier.styleable_with(Some(state), |st: &dyn StyleScope| {
                st.height(10.dp());
                st.animate_spec(spec(100), || st.width(100.dp()));
                st.pressed(|| st.animate_spec(spec(100), || st.width(200.dp())));
            }),
            content = || Box(modifier = Modifier.fill_max_size().test_tag("yarıda")),
        );
    }));
    rule.wait_for_idle();
    rule.main_clock().set_auto_advance(false);
    let press = PressInteraction::Press::new(Offset::Zero);
    source.try_emit(press);
    rule.advance_frame();
    rule.advance_time_by(50);
    let mid = bounds(&rule, "yarıda").width();
    assert!(mid > 120.0 && mid < 180.0, "{mid}");
    // (released half way: from where it is back to 100, not from 200)
    source.try_emit(PressInteraction::Release::new(press));
    rule.advance_frame();
    let now = bounds(&rule, "yarıda").width();
    assert!(now <= mid + 20.0 && now > 100.0, "{mid} → {now}");
    // (frame by frame down to 100, by the tween back)
    let mut last = now;
    for _ in 0..8 {
        rule.advance_time_by(16);
        let w = bounds(&rule, "yarıda").width();
        assert!(w <= last, "{last} → {w}");
        last = w;
    }
    assert!(near(last, 100.0), "{last}");
}

// ---- Text -----------------------------------------------------------------------------------------------------

fn text_inherits_a_styles_text_properties() {
    ComposeFoundationFlags::set_is_inherited_text_style_enabled(true);
    let big = mutable_state_of(false);
    let b = big.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let big = b.clone();
        Column(|| {
            Box(
                modifier = Modifier.styleable_with(None, move |s: &dyn StyleScope| {
                    s.font_size(if big.get() { 40.sp() } else { 20.sp() });
                    s.content_color(Color::Red);
                }),
                content = || {
                    BasicText("miras", modifier = Modifier.test_tag("miras"));
                    // (the text's own style wins)
                    BasicText(
                        "kendi",
                        modifier = Modifier.test_tag("kendi"),
                        style = TextStyle { color: Color::Blue, ..TextStyle::Default },
                    );
                },
            );
            BasicText("dışarıda", modifier = Modifier.test_tag("dışarıda"));
        });
    }));
    let small = bounds(&rule, "miras").height();
    let outside = bounds(&rule, "dışarıda").height();
    assert!(small > outside, "{small} {outside}");
    let c = rule.draw();
    let color_of = |text: &str| c.texts.iter().position(|t| t == text).map(|i| c.colors[i]);
    assert_eq!(color_of("miras"), Some(Color::Red));
    assert_eq!(color_of("kendi"), Some(Color::Blue));
    assert_eq!(color_of("dışarıda"), Some(Color::Black));
    big.set(true);
    rule.wait_for_idle();
    let large = bounds(&rule, "miras").height();
    ComposeFoundationFlags::set_is_inherited_text_style_enabled(false);
    assert!(large > small * 1.5, "{small} → {large}");
}

fn nested_styles_add_up_for_the_text() {
    ComposeFoundationFlags::set_is_inherited_text_style_enabled(true);
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| {
                s.font_size(30.sp());
                s.content_color(Color::Red);
            }),
            content = || {
                Box(
                    modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| s.content_color(Color::Green)),
                    content = || BasicText("iç içe", modifier = Modifier.test_tag("iç içe")),
                );
            },
        );
        BasicText("düz", modifier = Modifier.test_tag("düz"));
    }));
    let c = rule.draw();
    let at = c.texts.iter().position(|t| t == "iç içe").expect("drawn");
    // (the nearer style's color, the farther one's size)
    assert_eq!(c.colors[at], Color::Green);
    assert!(bounds(&rule, "iç içe").height() > bounds(&rule, "düz").height());
    // (off after the finders: they run what is due, the keep check drawing again)
    ComposeFoundationFlags::set_is_inherited_text_style_enabled(false);
}

fn without_the_flag_text_inherits_nothing() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.styleable_with(None, |s: &dyn StyleScope| s.content_color(Color::Red)),
            content = || BasicText("düz"),
        );
    }));
    assert_eq!(rule.draw().colors[0], Color::Black);
}

tests![
    sizes_paddings_and_borders_lay_out,
    fractions_bounds_and_positions,
    drawn_in_order_in_the_shape,
    a_shape_and_a_brush,
    borders_of_every_outline,
    layer_properties,
    the_empty_style_adds_nothing,
    interactions_drive_the_states_blocks,
    the_components_parameters_set_its_state,
    custom_keys_and_their_interactions,
    a_state_read_changes_what_it_affects_without_composing,
    locals_read_where_the_style_is_applied,
    animated_properties_follow_their_animations,
    an_interrupted_animation_goes_on_from_where_it_is,
    text_inherits_a_styles_text_properties,
    nested_styles_add_up_for_the_text,
    without_the_flag_text_inherits_nothing,
];
