//! Label and the tooltips' base, as Compose's material3 has them: a label shown above its component while the
//! component is pressed, dragged or hovered (its interactions, through material3's tooltip box), all the time when
//! persistent; the tooltip position providers at each of the six positionings, flipped at the window's edges, start
//! and end by the layout direction; the tooltip scope; the anchor positions' names.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::interaction::{DragInteraction, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::window::PopupPositionProvider;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    a_label_shows_while_its_component_is_hovered_pressed_or_dragged,
    a_persistent_label_always_shows,
    the_label_scope_gives_the_anchor_and_the_position_provider,
    tooltip_position_providers_place_at_each_positioning,
    tooltip_position_providers_flip_at_the_window_edges,
    the_deprecated_tooltip_position_provider_is_above,
    tooltip_anchor_positions_are_named,
);

#[kompozable]
fn Tagged(tag: String, width: i32, height: i32) {
    Box(Modifier.size_wh(width.dp(), height.dp()).test_tag(tag));
}

/// (the label is in a popup; the anchor's tags are in the unmerged tree)
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// A 40 dp component 100 dp down, hoverable through `source`, labelled by a 60 by 20 dp "etiket".
fn labelled(holder: Rc<RefCell<Option<MutableInteractionSource>>>, is_persistent: bool) -> impl Content + 'static {
    kompozable_lambda!(move || {
        let source = remember(MutableInteractionSource::new);
        *holder.borrow_mut() = Some(source.clone());
        Column(
            modifier = Modifier.padding_sides(top = 100.dp()),
            content = move || {
                let hover = source.clone();
                Label(
                    label = |_scope: &dyn TooltipScope| Tagged("etiket".into(), 60, 20),
                    interaction_source = Some(source.clone()),
                    is_persistent = is_persistent,
                    content = move || {
                        Box(Modifier.size_wh(40.dp(), 40.dp()).hoverable(hover.clone()).test_tag("çapa"));
                    },
                );
            },
        );
    })
}

fn a_label_shows_while_its_component_is_hovered_pressed_or_dragged() {
    // Compose's Label: not persistent, it follows the interactions (HandleInteractions), shown with UserInput's
    // priority — until they end, not for a while —, above the anchor as a plain tooltip is (4 dp from it, centered,
    // start aligned where centering leaves the window).
    let holder = Rc::new(RefCell::new(None));
    let rule = KompozTestRule::with_viewport(labelled(holder.clone(), false), IntSize::new(400, 800));
    let source = || holder.borrow().clone().unwrap();
    let density = rule.density();
    rule.on_node_with_tag("etiket").assert_does_not_exist();

    // The mouse over the component shows it; leaving it dismisses it.
    let anchor = bounds(&rule, "çapa");
    rule.on_node_with_tag("çapa").perform_mouse_input(|m| m.move_to(Offset::new(20.0, 20.0)));
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_exists();
    let label = bounds(&rule, "etiket");
    let spacing = density.round_to_px(4.dp()) as f32;
    assert!(near(label.left, anchor.left) && near(label.bottom, anchor.top - spacing), "{label:?} {anchor:?}");
    assert!(near(label.width(), density.to_px(60.dp())) && near(label.height(), density.to_px(20.dp())));
    rule.on_node_with_tag("çapa").perform_mouse_input(|m| m.move_to(Offset::new(300.0, 500.0)));
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_does_not_exist();

    // A press shows it as long as it lasts (no timeout), its release dismisses it.
    let press = PressInteraction::Press::new(Offset::new(10.0, 10.0));
    source().try_emit(press);
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_exists();
    rule.advance_time_by(3000);
    rule.on_node_with_tag("etiket").assert_exists();
    source().try_emit(PressInteraction::Release::new(press));
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_does_not_exist();

    // A drag the same.
    let start = DragInteraction::Start::new();
    source().try_emit(start);
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_exists();
    source().try_emit(DragInteraction::Stop::new(start));
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_does_not_exist();

    // Each interaction takes over from the one before (collectLatest): a press cancelled, which isn't one of the
    // ends it dismisses on, still ends the press's show — its mutation cancelled, the label hidden.
    let press = PressInteraction::Press::new(Offset::Zero);
    source().try_emit(press);
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_exists();
    source().try_emit(PressInteraction::Cancel::new(press));
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_does_not_exist();
}

fn a_persistent_label_always_shows() {
    // Compose's LabelStateImpl: visible, persistent, its show and dismiss doing nothing; the interactions aren't
    // followed.
    let holder = Rc::new(RefCell::new(None));
    let rule = KompozTestRule::with_viewport(labelled(holder.clone(), true), IntSize::new(400, 800));
    let source = || holder.borrow().clone().unwrap();
    rule.on_node_with_tag("etiket").assert_exists();
    let press = PressInteraction::Press::new(Offset::Zero);
    source().try_emit(press);
    rule.advance_frame();
    source().try_emit(PressInteraction::Release::new(press));
    rule.advance_frame();
    rule.on_node_with_tag("etiket").assert_exists();
    rule.on_node_with_tag("çapa").perform_mouse_input(|m| m.move_to(Offset::new(20.0, 20.0)));
    rule.on_node_with_tag("çapa").perform_mouse_input(|m| m.move_to(Offset::new(300.0, 500.0)));
    rule.advance_time_by(3000);
    rule.on_node_with_tag("etiket").assert_exists();
}

fn the_label_scope_gives_the_anchor_and_the_position_provider() {
    // Compose's TooltipScopeImpl: the anchor's coordinates (the content's Box, as it was last placed) and the
    // position provider the label is placed by.
    type Seen = Option<(Option<IntSize>, Rc<dyn PopupPositionProvider>)>;
    let seen: Rc<RefCell<Seen>> = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Label(
                label = move |scope: &dyn TooltipScope| {
                    let measure_scope = MeasureScope::new(Density::default(), LayoutDirection::Ltr);
                    let anchor = scope.obtain_anchor_bounds(&measure_scope).map(|c| c.size());
                    *s.borrow_mut() = Some((anchor, scope.obtain_position_provider()));
                    Tagged("etiket".into(), 30, 10);
                },
                is_persistent = true,
                content = || Tagged("içerik".into(), 50, 24),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let (anchor, provider) = seen.borrow().clone().expect("the label composed");
    let density = rule.density();
    let expected = IntSize::new(density.round_to_px(50.dp()), density.round_to_px(24.dp()));
    // (the label composed before the anchor was placed had none; the coordinates are a state: it composes again)
    assert_eq!(anchor, Some(expected));
    // The provider is Above's: centered over the anchor, 4 dp up.
    let spacing = density.round_to_px(4.dp());
    let at = provider.calculate_position(
        IntRect::new(100, 300, 150, 324),
        IntSize::new(400, 800),
        LayoutDirection::Ltr,
        IntSize::new(30, 10),
    );
    assert_eq!(at, IntOffset::new(110, 300 - 10 - spacing));
}

/// The six providers, made in a window of 400 by 800 px; the spacing in px.
fn providers() -> (KompozTestRule, Vec<Rc<dyn PopupPositionProvider>>, i32) {
    let made: Rc<RefCell<Vec<Rc<dyn PopupPositionProvider>>>> = Rc::new(RefCell::new(Vec::new()));
    let m = made.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let spacing = 4.dp();
            *m.borrow_mut() = vec![
                TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::Above, spacing),
                TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::Below, spacing),
                TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::Left, spacing),
                TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::Right, spacing),
                TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::Start, spacing),
                TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::End, spacing),
            ];
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let spacing = rule.density().round_to_px(4.dp());
    let providers = made.borrow().clone();
    (rule, providers, spacing)
}

const Ltr: LayoutDirection = LayoutDirection::Ltr;
const Rtl: LayoutDirection = LayoutDirection::Rtl;

fn tooltip_position_providers_place_at_each_positioning() {
    // Compose's TooltipPositionProviderImpl, a 100 by 30 popup at an anchor of 100 by 40 in the middle of the window.
    let (_rule, p, s) = providers();
    let (above, below, left, right, start, end) = (&p[0], &p[1], &p[2], &p[3], &p[4], &p[5]);
    let anchor = IntRect::new(150, 300, 250, 340);
    let popup = IntSize::new(100, 30);
    let window = IntSize::new(400, 800);
    let at = |provider: &Rc<dyn PopupPositionProvider>, direction| {
        provider.calculate_position(anchor, window, direction, popup)
    };
    let middle = (300 + 340 - 30) / 2;
    assert_eq!(at(above, Ltr), IntOffset::new(150, 300 - 30 - s));
    assert_eq!(at(below, Ltr), IntOffset::new(150, 340 + s));
    assert_eq!(at(left, Ltr), IntOffset::new(150 - (100 + s), middle));
    assert_eq!(at(right, Ltr), IntOffset::new(250 + s, middle));
    // Start and end by the layout direction; left and right the same both ways.
    assert_eq!(at(start, Ltr), at(left, Ltr));
    assert_eq!(at(start, Rtl), at(right, Ltr));
    assert_eq!(at(end, Ltr), at(right, Ltr));
    assert_eq!(at(end, Rtl), at(left, Ltr));
    assert_eq!(at(left, Rtl), at(left, Ltr));
    assert_eq!(at(above, Rtl), at(above, Ltr));
    // The window kept within is the container's (LocalWindowInfo's) the provider was made in, not the one given.
    let tiny = IntSize::new(10, 10);
    assert_eq!(right.calculate_position(anchor, tiny, Ltr, popup), at(right, Ltr));
    assert_eq!(below.calculate_position(anchor, tiny, Ltr, popup), at(below, Ltr));
}

fn tooltip_position_providers_flip_at_the_window_edges() {
    let (_rule, p, s) = providers();
    let (above, below, left, right, start, end) = (&p[0], &p[1], &p[2], &p[3], &p[4], &p[5]);
    let popup = IntSize::new(100, 30);
    let window = IntSize::new(400, 800);
    let at = |provider: &Rc<dyn PopupPositionProvider>, anchor: IntRect, direction| {
        provider.calculate_position(anchor, window, direction, popup)
    };
    // Above at the window's top: below instead.
    let top = IntRect::new(150, 10, 250, 50);
    assert_eq!(at(above, top, Ltr), IntOffset::new(150, 50 + s));
    // Below at the window's bottom: above instead.
    let bottom = IntRect::new(150, 760, 250, 790);
    assert_eq!(at(below, bottom, Ltr), IntOffset::new(150, 760 - 30 - s));
    // Centering past the window's left: start aligned; past its right: end aligned.
    let at_left = IntRect::new(0, 300, 40, 340);
    assert_eq!(at(above, at_left, Ltr).x, 0);
    assert_eq!(at(below, at_left, Ltr).x, 0);
    let at_right = IntRect::new(360, 300, 400, 340);
    assert_eq!(at(above, at_right, Ltr).x, 400 - 100);
    assert_eq!(at(below, at_right, Ltr).x, 400 - 100);
    // Left at the window's left: on the right, pulled back into the window where it would leave it.
    let middle = (300 + 340 - 30) / 2;
    let near_left = IntRect::new(20, 300, 120, 340);
    assert_eq!(at(left, near_left, Ltr), IntOffset::new(120 + s, middle));
    let wide = IntRect::new(20, 300, 380, 340);
    assert_eq!(at(left, wide, Ltr), IntOffset::new(380 + s - (380 + s + 100 - 400), middle));
    // Right at the window's right: on the left, kept from going past the window's left.
    let near_right = IntRect::new(300, 300, 380, 340);
    assert_eq!(at(right, near_right, Ltr), IntOffset::new(300 - (100 + s), middle));
    let wide_right = IntRect::new(50, 300, 380, 340);
    assert_eq!(at(right, wide_right, Ltr), IntOffset::new(0, middle));
    // Start right to left is right's: flipped at the window's right as right is; end right to left is left's.
    assert_eq!(at(start, near_right, Rtl), at(right, near_right, Ltr));
    assert_eq!(at(end, near_left, Rtl), at(left, near_left, Ltr));
    assert_eq!(at(start, near_left, Ltr), at(left, near_left, Ltr));
    assert_eq!(at(end, near_right, Ltr), at(right, near_right, Ltr));
}

fn the_deprecated_tooltip_position_provider_is_above() {
    // Compose's deprecated rememberTooltipPositionProvider(spacing): above, centered; start aligned at the window's
    // left (the anchor's left as it is), end aligned at its right (the window it is given); below where above leaves
    // the window.
    let made: Rc<RefCell<Option<Rc<dyn PopupPositionProvider>>>> = Rc::new(RefCell::new(None));
    let m = made.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        #[allow(deprecated)]
        let provider = TooltipDefaults::remember_tooltip_position_provider_with_spacing(8.dp());
        *m.borrow_mut() = Some(provider);
    }));
    rule.wait_for_idle();
    let s = rule.density().round_to_px(8.dp());
    let provider = made.borrow().clone().unwrap();
    let popup = IntSize::new(100, 30);
    let window = IntSize::new(400, 800);
    let at = |anchor: IntRect| provider.calculate_position(anchor, window, Ltr, popup);
    assert_eq!(at(IntRect::new(150, 300, 250, 340)), IntOffset::new(150, 300 - 30 - s));
    assert_eq!(at(IntRect::new(10, 300, 50, 340)), IntOffset::new(10, 300 - 30 - s));
    assert_eq!(at(IntRect::new(360, 300, 400, 340)), IntOffset::new(400 - 100, 300 - 30 - s));
    assert_eq!(at(IntRect::new(150, 10, 250, 50)), IntOffset::new(150, 50 + s));
    // (the window it is given: a narrower one end aligns sooner)
    let anchor = IntRect::new(100, 300, 160, 340);
    assert_eq!(at(anchor).x, 100 + (60 - 100) / 2);
    let narrow = provider.calculate_position(anchor, IntSize::new(150, 800), Ltr, popup);
    assert_eq!(narrow.x, 160 - 100);
}

fn tooltip_anchor_positions_are_named() {
    // Compose's TooltipAnchorPosition.toString.
    let names: Vec<String> = [
        TooltipAnchorPosition::Above,
        TooltipAnchorPosition::Below,
        TooltipAnchorPosition::Left,
        TooltipAnchorPosition::Right,
        TooltipAnchorPosition::Start,
        TooltipAnchorPosition::End,
    ]
    .iter()
    .map(|p| p.to_string())
    .collect();
    assert_eq!(names, ["Above", "Below", "Left", "Right", "Start", "End"]);
    assert_eq!(format!("{:?}", TooltipAnchorPosition::Start), "Start");
    assert_ne!(TooltipAnchorPosition::Start, TooltipAnchorPosition::Left);
}
