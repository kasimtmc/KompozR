//! Unit tests of the style resolution, from Compose's semantics: what a style sets (each write overriding the ones
//! before, styles combined in order, blocks of a state only in it), in px of the density; which phases a change
//! affects; how colors and brushes, NaN ends, switching properties and shadows interpolate; states and their keys;
//! the text style a style gives a text.

use super::animations::{StyleAnimations, default_spring_spec, same_spec};
use super::modifier::to_text_style;
use super::properties::*;
use super::resolved::{BuildContext, ResolvedStyle};
use super::scope::*;
use super::state::{MutableStyleState, StyleState, StyleStateKey};
use super::styles::{EmptyStyle, Style, combined_style, combined_style3, combined_styles, same_style};
use kompoz_graphics::{
    Brush, Color, ColorFilter, Density, Dp, Offset, RectangleShape, RoundedCornerShape, Shadow, SolidColor, UnitExt,
};
use kompoz_text::{FontStyle, FontWeight, TextAlign, TextDecoration, TextMotion, TextStyle};
use kompoz_ui::state::ToggleableState;
use std::rc::Rc;
use std::sync::Arc;

fn resolve_in(style: &dyn Style, state: StyleState, density: f32) -> StyleProperties {
    let resolved = ResolvedStyle::default();
    let cx = BuildContext {
        state,
        locals: StyleLocals::default(),
        density: Density::new(density, 1.0),
        animation_scope: None,
    };
    resolved.build(style, &cx);
    let mut p = StyleProperties::default();
    resolved.resolve_into(0, &mut p);
    p
}

fn resolve(style: &dyn Style) -> StyleProperties {
    resolve_in(style, MutableStyleState::new(None).into(), 1.0)
}

fn shared(f: impl Fn(&dyn StyleScope) + 'static) -> Rc<dyn Style> {
    Rc::new(f)
}

#[test]
fn later_writes_override_earlier_ones() {
    let p = resolve(&|s: &dyn StyleScope| {
        s.background(Color::Red);
        s.size(10.dp());
        s.background(Color::Blue);
        s.width(20.dp());
    });
    assert_eq!(p.background_color, Color::Blue);
    assert_eq!((p.width, p.height), (20.0, 10.0));
    // (nothing else set)
    assert!(!p.has(AlphaId) && !p.has_obj(ShapeId) && !p.has(ContentPaddingStartId));
}

#[test]
fn lengths_are_px_of_the_density() {
    let p = resolve_in(
        &|s: &dyn StyleScope| {
            s.content_padding(3.3.dp());
            s.external_padding_start(1.2.dp());
            s.width(10.dp());
            s.min_height(4.dp());
            s.left(2.5.dp());
            s.border_width(1.2.dp());
        },
        MutableStyleState::new(None).into(),
        2.0,
    );
    // (paddings rounded to px, other lengths as they are, a border's width up to a whole px)
    assert_eq!(p.content_padding_start, 7.0);
    assert_eq!(p.content_padding_bottom, 7.0);
    assert_eq!(p.external_padding_start, 2.0);
    assert_eq!((p.width, p.min_height, p.left), (20.0, 8.0, 5.0));
    assert_eq!(p.border_width, 3.0);
    // (a hairline is 1 px, an unspecified width 0)
    let p = resolve_in(&|s: &dyn StyleScope| s.border_width(Dp::Hairline), MutableStyleState::new(None).into(), 3.0);
    assert_eq!(p.border_width, 1.0);
    let p = resolve(&|s: &dyn StyleScope| s.border_width(Dp::Unspecified));
    assert_eq!(p.border_width, 0.0);
}

#[test]
fn a_size_and_its_fraction_exclude_each_other() {
    let p = resolve(&|s: &dyn StyleScope| {
        s.width(10.dp());
        s.fill_width();
    });
    assert!(p.has(WidthFractionId) && !p.has(WidthId) && p.width.is_nan());
    let p = resolve(&|s: &dyn StyleScope| {
        s.fill_size();
        s.height(5.dp());
    });
    assert!(p.has(HeightId) && !p.has(HeightFractionId) && p.has(WidthFractionId));
}

#[test]
fn a_color_and_a_brush_exclude_each_other() {
    let gradient = Brush::horizontal_gradient(&[Color::Red, Color::Blue], 0.0, 10.0, kompoz_graphics::TileMode::Clamp);
    let g = gradient.clone();
    let p = resolve(&move |s: &dyn StyleScope| {
        s.background(Color::Red);
        s.background_brush(g.clone());
        s.border_color(Color::Green);
    });
    assert!(!p.has(BackgroundColorId) && p.has_obj(BackgroundBrushId));
    assert_eq!(p.background_brush, Some(gradient.clone()));
    assert_eq!(p.background_color, Color::Unspecified);
    assert!(p.has(BorderColorId) && !p.has_obj(BorderBrushId));
    let g = gradient.clone();
    let p = resolve(&move |s: &dyn StyleScope| {
        s.content_brush(g.clone());
        s.content_color(Color::Red);
    });
    assert!(p.has(ContentColorId) && !p.has_obj(ContentBrushId) && p.content_brush.is_none());
}

#[test]
fn styles_combine_in_order() {
    let a = shared(|s| {
        s.background(Color::Red);
        s.alpha(0.5);
    });
    let b = shared(|s| s.background(Color::Blue));
    let p = resolve(&*combined_style(a.clone(), b.clone()));
    assert_eq!((p.background_color, p.alpha), (Color::Blue, 0.5));
    let p = resolve(&*a.clone().then(b.clone()));
    assert_eq!(p.background_color, Color::Blue);
    let p = resolve(&*b.clone().then(a.clone()));
    assert_eq!(p.background_color, Color::Red);
    // (three, and any number: the last wins)
    let c = shared(|s| s.background(Color::Green));
    let p = resolve(&*combined_style3(a.clone(), b.clone(), c.clone()));
    assert_eq!(p.background_color, Color::Green);
    let p = resolve(&*combined_styles(&[c.clone(), b.clone(), a.clone()]));
    assert_eq!(p.background_color, Color::Red);
}

#[test]
fn the_empty_style_is_left_out_of_combinations() {
    let a = shared(|s| s.background(Color::Red));
    let empty: Rc<dyn Style> = Rc::new(Style);
    // (Compose's `Style(style1, Style) === style1`)
    assert!(Rc::ptr_eq(&combined_style(a.clone(), Style), &a));
    assert!(Rc::ptr_eq(&combined_style(Style, a.clone()), &a));
    assert!(Rc::ptr_eq(&a.clone().then(EmptyStyle), &a));
    assert!(combined_style(Style, Style).is_empty_style());
    assert!(combined_styles(&[empty.clone(), empty.clone()]).is_empty_style());
    assert!(Rc::ptr_eq(&combined_styles(&[empty.clone(), a.clone(), empty]), &a));
    assert!(Rc::ptr_eq(&combined_style3(Style, Style, a.clone()), &a));
}

#[test]
fn combinations_are_flat_and_equal_by_their_parts() {
    let (a, b, c) = (shared(|s| s.alpha(0.1)), shared(|s| s.alpha(0.2)), shared(|s| s.alpha(0.3)));
    let ab = combined_style(a.clone(), b.clone());
    let abc = combined_style(ab.clone(), c.clone());
    assert_eq!(abc.combined_styles().map(<[_]>::len), Some(3));
    let abc2 = combined_style3(a.clone(), b.clone(), c.clone());
    assert!(same_style(&abc, &abc2));
    assert!(!same_style(&abc, &combined_style3(a.clone(), c.clone(), b.clone())));
    // (a closure is equal only to itself; the empty style to the empty style)
    assert!(same_style(&a, &a.clone()) && !same_style(&a, &shared(|s| s.alpha(0.1))));
    assert!(same_style(&(Rc::new(Style) as Rc<dyn Style>), &(Rc::new(EmptyStyle) as Rc<dyn Style>)));
    // (the vararg form keeps a combined one as one part)
    assert_eq!(combined_styles(&[ab, c]).combined_styles().map(<[_]>::len), Some(2));
}

#[test]
fn a_states_blocks_apply_only_in_it() {
    let style = |s: &dyn StyleScope| {
        s.background(Color::White);
        s.pressed(|| s.background(Color::Red));
        s.hovered(|| s.alpha(0.5));
        s.focused(|| s.border_width(2.dp()));
        s.selected(|| s.scale(2.0));
        s.disabled(|| s.background(Color::Gray));
        s.checked(|| s.content_color(Color::Green));
        s.tri_state_toggle_indeterminate(|| s.content_color(Color::Yellow));
        s.tri_state_toggle_off(|| s.font_size(10.sp()));
    };
    let state = MutableStyleState::new(None);
    let p = resolve_in(&style, state.clone().into(), 1.0);
    assert_eq!(p.background_color, Color::White);
    assert!(!p.has(AlphaId) && !p.has(BorderWidthId) && !p.has(ScaleXId) && !p.has(ContentColorId));
    assert!(p.has(FontSizeId)); // (a toggle is off by default)
    state.set_is_pressed(true);
    state.set_is_hovered(true);
    state.set_is_focused(true);
    state.set_is_selected(true);
    state.set_is_checked(true);
    let p = resolve_in(&style, state.clone().into(), 1.0);
    assert_eq!((p.background_color, p.alpha, p.border_width, p.scale_x, p.scale_y), (Color::Red, 0.5, 2.0, 2.0, 2.0));
    assert_eq!(p.content_color, Color::Green);
    assert!(!p.has(FontSizeId));
    state.set_tri_state_toggle(ToggleableState::Indeterminate);
    state.set_is_enabled(false);
    let p = resolve_in(&style, state.clone().into(), 1.0);
    assert_eq!((p.background_color, p.content_color), (Color::Gray, Color::Yellow));
}

#[test]
fn custom_state_keys() {
    let playing = StyleStateKey::new(false);
    let volume = StyleStateKey::new(3);
    let (pk, vk) = (playing.clone(), volume.clone());
    let style = move |s: &dyn StyleScope| {
        s.border_color(Color::Gray);
        s.state_key(&pk, || s.border_color(Color::Green));
        s.state_of(&vk, || s.alpha(1.0), |key, state| state.get(key) > 5);
    };
    let state = MutableStyleState::new(None);
    assert!(!state.get(&playing) && state.get(&volume) == 3);
    let p = resolve_in(&style, state.clone().into(), 1.0);
    assert_eq!(p.border_color, Color::Gray);
    assert!(!p.has(AlphaId));
    state.set(&playing, true);
    state.set(&volume, 7);
    let p = resolve_in(&style, state.clone().into(), 1.0);
    assert_eq!((p.border_color, p.alpha), (Color::Green, 1.0));
    // (removed: its default again)
    state.remove(&volume);
    assert_eq!(state.get(&volume), 3);
    // (keys are known by their identity; the predefined ones by what they are)
    assert_eq!(playing, playing.clone());
    assert_ne!(playing, StyleStateKey::new(false));
    assert_eq!(StyleStateKey::Pressed, StyleStateKey::Pressed);
    assert_ne!(StyleStateKey::Pressed, StyleStateKey::Hovered);
}

#[test]
#[should_panic(expected = "Cannot remove an internal StyleStateKey")]
fn a_predefined_key_cant_be_removed() {
    MutableStyleState::new(None).remove(&StyleStateKey::Pressed);
}

#[test]
fn predefined_states_through_their_keys() {
    let state = MutableStyleState::new(None);
    assert!(state.is_enabled() && state.get(&StyleStateKey::Enabled));
    assert!(StyleStateKey::Enabled.default_value());
    state.set(&StyleStateKey::Pressed, true);
    assert!(state.is_pressed() && state.get(&StyleStateKey::Pressed));
    state.set(&StyleStateKey::Toggle, ToggleableState::On);
    assert!(state.is_checked() && state.get(&StyleStateKey::Toggle) == ToggleableState::On);
    state.set_is_checked(false);
    assert_eq!(state.tri_state_toggle(), ToggleableState::Off);
    // (states are equal when they are the same one)
    let other = MutableStyleState::new(None);
    assert!(*state == *state.clone() && *state != *other);
}

#[test]
fn which_phases_a_change_affects() {
    let diff = |a: &dyn Style, b: &dyn Style| resolve(a).diff(&resolve(b), PhaseFlagMask);
    let none = |_: &dyn StyleScope| {};
    assert_eq!(
        diff(&|s: &dyn StyleScope| s.background(Color::Red), &|s: &dyn StyleScope| s.background(Color::Blue)),
        DrawFlag
    );
    assert_eq!(diff(&|s: &dyn StyleScope| s.background(Color::Red), &|s: &dyn StyleScope| s.background(Color::Red)), 0);
    assert_eq!(diff(&|s: &dyn StyleScope| s.content_padding(1.dp()), &none), InnerLayoutFlag);
    assert_eq!(diff(&|s: &dyn StyleScope| s.external_padding(1.dp()), &none), OuterLayoutFlag);
    assert_eq!(diff(&|s: &dyn StyleScope| s.width(1.dp()), &|s: &dyn StyleScope| s.width(2.dp())), OuterLayoutFlag);
    assert_eq!(diff(&|s: &dyn StyleScope| s.alpha(0.5), &none), LayerFlag);
    assert_eq!(diff(&|s: &dyn StyleScope| s.border_width(1.dp()), &none), DrawFlag | InnerLayoutFlag);
    assert_eq!(diff(&|s: &dyn StyleScope| s.content_color(Color::Red), &none), TextDrawFlag);
    assert_eq!(diff(&|s: &dyn StyleScope| s.font_size(12.sp()), &none), TextLayoutFlag | TextDrawFlag);
    // (a shape is drawn; with layer properties it is the layer's outline too)
    let round = Arc::new(RoundedCornerShape(4.0f32));
    let r = round.clone();
    assert_eq!(diff(&move |s: &dyn StyleScope| s.shape(r.clone()), &none), DrawFlag);
    let r = round.clone();
    assert_eq!(
        diff(
            &move |s: &dyn StyleScope| {
                s.shape(r.clone());
                s.clip(true);
            },
            &|s: &dyn StyleScope| s.clip(true)
        ),
        DrawFlag | LayerFlag
    );
    // (an equal shape is no change)
    let (a, b) = (round.clone(), Arc::new(RoundedCornerShape(4.0f32)));
    assert_eq!(diff(&move |s: &dyn StyleScope| s.shape(a.clone()), &move |s: &dyn StyleScope| s.shape(b.clone())), 0);
}

#[test]
fn text_properties_of_a_text_style() {
    let style = TextStyle {
        color: Color::Red,
        font_size: 20.sp(),
        font_weight: Some(FontWeight::Bold),
        text_align: TextAlign::Center,
        text_motion: Some(TextMotion::Animated),
        ..TextStyle::Default
    };
    let s2 = style.clone();
    let p = resolve(&move |s: &dyn StyleScope| s.text_style(s2.clone()));
    assert_eq!(p.content_color, Color::Red);
    assert_eq!(p.font_size, 20.sp());
    assert_eq!(p.font_weight(), FontWeight::Bold);
    assert_eq!(p.text_align(), TextAlign::Center);
    assert_eq!(p.text_motion, TextMotion::Animated);
    // (what it doesn't specify isn't set)
    assert!(!p.has(LineHeightId) && !p.has(FontStyleId) && !p.has(TextDecorationId) && !p.has_obj(FontFamilyId));
    // (unset "enums" read as their defaults)
    assert_eq!((p.font_style(), p.text_decoration()), (FontStyle::Normal, TextDecoration::None));
}

#[test]
fn the_text_style_a_style_gives_a_text() {
    let p = resolve(&|s: &dyn StyleScope| {
        s.content_color(Color::Red);
        s.font_size(30.sp());
        s.font_style(FontStyle::Italic);
    });
    // (what the text specifies wins; the style fills in the rest)
    let supplied = TextStyle { font_size: 12.sp(), ..TextStyle::Default };
    let t = to_text_style(&p, &supplied);
    assert_eq!((t.color, t.font_size, t.font_style), (Color::Red, 12.sp(), Some(FontStyle::Italic)));
    assert!(t.font_weight.is_none());
    // (a supplied brush paints it rather than the style's color)
    let brush = SolidColor(Color::Blue);
    let t = to_text_style(&p, &TextStyle { brush: Some(brush.clone()), ..TextStyle::Default });
    assert_eq!((t.brush, t.color), (Some(brush), Color::Unspecified));
}

#[test]
fn interpolation_of_lengths_colors_and_switches() {
    let anim = StyleAnimations::default();
    // (no animation of a property: its time is 0, the start)
    assert_eq!(anim.time_of(AlphaId as u32), 0.0);
    let a = resolve(&|s: &dyn StyleScope| {
        s.alpha(0.2);
        s.background(Color::Red);
        s.font_size(10.sp());
    });
    let b = resolve(&|s: &dyn StyleScope| {
        s.alpha(0.8);
        s.background(Color::Blue);
        s.width(10.dp());
        s.font_size(20.sp());
    });
    let mut r = StyleProperties::default();
    lerp_properties(&a, &b, &anim, u64::MAX, u32::MAX, &mut r);
    // (at 0: the start's values; the width only one end set taken from it, a NaN end)
    assert_eq!((r.alpha, r.background_color, r.width), (0.2, Color::Red, 10.0));
    assert_eq!(r.font_size, 10.sp());
}

#[test]
fn shadows_interpolate_to_and_from_none() {
    let one =
        Shadow { color: Color::Black, blur_radius: 8.0, spread: 2.0, offset: Offset::new(2.0, 2.0), ..Shadow::None };
    let o = one.clone();
    let a = resolve(&move |s: &dyn StyleScope| s.drop_shadow(o.clone()));
    let b = resolve(&|s: &dyn StyleScope| s.drop_shadows(&[Shadow::None, Shadow::None]));
    // (one shadow and an array: an array, place by place)
    let mut r = StyleProperties::default();
    lerp_properties(&a, &b, &StyleAnimations::default(), 0, u32::MAX, &mut r);
    match &r.drop_shadow {
        Some(ShadowValue::Many(m)) => {
            assert_eq!(m.len(), 2);
            assert_eq!(m[0].as_ref().map(|s| s.blur_radius), Some(8.0));
            // (its second place from none: the other end made transparent)
            assert_eq!(m[1].as_ref().map(|s| s.color), Some(Color::Transparent));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn color_filters_interpolate_within_their_kind() {
    let tint = |c: Color| ColorFilter::tint(c, kompoz_graphics::BlendMode::SrcIn);
    let (a, b) = (tint(Color::Red), tint(Color::Blue));
    let p = resolve(&move |s: &dyn StyleScope| s.color_filter(Some(a)));
    let q = resolve(&move |s: &dyn StyleScope| s.color_filter(Some(b)));
    let mut r = StyleProperties::default();
    lerp_properties(&p, &q, &StyleAnimations::default(), 0, u32::MAX, &mut r);
    assert_eq!(r.color_filter, Some(tint(Color::Red)));
}

#[test]
fn what_animates_when_it_changes() {
    // (Compose's semantics: a property written in `animate` animates when its value changes; once animated, it
    // animates back when it is written outside of `animate`; one never animated jumps)
    let resolved = ResolvedStyle::default();
    let state = MutableStyleState::new(None);
    let cx = BuildContext {
        state: state.clone().into(),
        locals: StyleLocals::default(),
        density: Density::new(1.0, 1.0),
        animation_scope: None,
    };
    let style = |s: &dyn StyleScope| {
        s.background(Color::Blue);
        s.alpha(1.0);
        s.pressed(|| {
            s.animate(|| s.background(Color::Red));
            s.alpha(0.5);
        });
    };
    resolved.build(&style, &cx);
    // (the first resolution starts no animation: nothing to animate from)
    assert_eq!(resolved.animating_flags(), 0);
    state.set_is_pressed(true);
    resolved.build(&style, &cx);
    assert_eq!(resolved.animating_flags(), DrawFlag);
    // (the alpha jumps; the background's animation, run in a node's scope, is what tests/style.rs follows)
    let mut p = StyleProperties::default();
    resolved.resolve_into(DrawFlag | LayerFlag, &mut p);
    assert_eq!(p.alpha, 0.5);
    state.set_is_pressed(false);
    resolved.build(&style, &cx);
    assert_eq!(resolved.animating_flags(), DrawFlag);
    // (a change of a property not animated starts nothing)
    let other = |s: &dyn StyleScope| {
        s.background(Color::Blue);
        s.alpha(0.2);
    };
    let fresh = ResolvedStyle::default();
    fresh.build(&style, &cx);
    fresh.build(&other, &cx);
    assert_eq!(fresh.animating_flags(), 0);
    // (specs: the default spring is the one unspecified, and a tween is another)
    let tween: Arc<dyn kompoz_animation::AnimationSpec<f32>> = Arc::new(kompoz_animation::tween::<f32>());
    assert!(same_spec(&default_spring_spec(), &default_spring_spec()) && !same_spec(&tween, &default_spring_spec()));
    resolved.close();
    assert_eq!(resolved.animating_flags(), 0);
}

#[test]
fn the_rectangle_is_the_default_shape() {
    let p = resolve(&|_: &dyn StyleScope| {});
    assert!(p.shape.shape_as_any().is::<RectangleShape>());
    assert_eq!(p.phase_flags(), 0);
}

#[test]
fn a_custom_scope_by_the_style_scope() {
    // (Compose's StyleStateKeySample: a scope of its own made of the style scope, `object : StyleScope by this, …`)
    trait MediaPlayerStyleScope: StyleScope {
        fn player_playing(&self, key: &StyleStateKey<bool>, block: impl FnOnce())
        where
            Self: Sized,
        {
            self.state_key(key, block);
        }
    }
    impl MediaPlayerStyleScope for StyleScopeRef<'_> {}
    let playing = StyleStateKey::new(false);
    let pk = playing.clone();
    let media_style = move |s: &StyleScopeRef<'_>| {
        s.border_color(Color::Gray);
        s.player_playing(&pk, || s.border_color(Color::Green));
    };
    // (a custom style: a closure of the custom scope, applied on it)
    let to_style = move |this: &dyn StyleScope| {
        let scope = StyleScopeRef(this);
        scope.apply_custom_style(&media_style);
    };
    let state = MutableStyleState::new(None);
    state.set(&playing, true);
    let p = resolve_in(&to_style, state.into(), 1.0);
    assert_eq!(p.border_color, Color::Green);
    // (a style applied on a scope: Compose's `apply`)
    let inner: Rc<dyn Style> = Rc::new(|s: &dyn StyleScope| s.alpha(0.25));
    let i = inner.clone();
    let p = resolve(&move |s: &dyn StyleScope| s.apply_custom_style(&*i));
    assert_eq!(p.alpha, 0.25);
}

#[test]
fn locals_outside_of_a_node_are_their_defaults() {
    let p = resolve(&|s: &dyn StyleScope| {
        let direction = s.current_value(&kompoz_ui::platform::LocalLayoutDirection);
        s.alpha(if direction == kompoz_graphics::LayoutDirection::Ltr { 1.0 } else { 0.0 });
        // (a density's members: Kotlin's `CustomStyleScope : Density`)
        s.width(Dp(s.density));
    });
    assert_eq!((p.alpha, p.width), (1.0, 1.0));
}

#[test]
fn brushes_compare_by_value_shadow_arrays_by_identity() {
    let p = resolve(&|s: &dyn StyleScope| s.background_brush(SolidColor(Color::Red)));
    let q = resolve(&|s: &dyn StyleScope| s.background_brush(SolidColor(Color::Red)));
    assert_eq!(p.diff(&q, PhaseFlagMask), 0);
    // (Compose compares the shadow property with `==`: an array by its identity)
    let p = resolve(&|s: &dyn StyleScope| s.drop_shadow(Shadow::None));
    let q = resolve(&|s: &dyn StyleScope| s.drop_shadow(Shadow::None));
    assert_eq!(p.diff(&q, PhaseFlagMask), 0);
    let p = resolve(&|s: &dyn StyleScope| s.drop_shadows(&[Shadow::None]));
    let q = resolve(&|s: &dyn StyleScope| s.drop_shadows(&[Shadow::None]));
    assert_eq!(p.diff(&q, PhaseFlagMask), DrawFlag);
}
