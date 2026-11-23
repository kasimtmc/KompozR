//! The pane expansion as Compose's adaptive-layout 1.2.0 has it, through its public API: the C++ Kompoz's pane
//! expansion tests by name (written as a Compose user writes them: a navigator, the navigable scaffold, material3's
//! `VerticalDragHandle` made draggable by the scope's `pane_expansion_draggable`; Compose's values asserted, where
//! they differ from the C++'s said), and Compose's behaviour of the state's public members. (The crate internals —
//! the savers, the anchors' math, drag to resize, pane margins, the internal draggable — are unit tests in
//! src/layout/expansion_tests.rs.)

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    pane_expansion_handle_drags_the_split_and_settles_on_an_anchor,
    pane_expansion_handle_alone_brings_its_own_state,
    pane_expansion_without_a_handle_and_on_a_supporting_pane_scaffold,
    material_states_come_back_after_the_process_ended,
    pane_expansion_states_take_widths_proportions_and_anchors,
);

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// A drag handle as Compose's samples make it: material3's VerticalDragHandle, draggable by the scope's
/// `pane_expansion_draggable` with the handle's own interaction source and the default semantics.
fn drag_handle(source: Option<MutableInteractionSource>) -> ThreePaneDragHandle {
    Rc::new(kompozable_lambda!(move |scope: &ThreePaneScaffoldScope, state: PaneExpansionState| {
        let source = source.clone().unwrap_or_else(|| remember(MutableInteractionSource::new));
        VerticalDragHandle(
            modifier = scope
                .pane_expansion_draggable(
                    Modifier,
                    state,
                    LocalMinimumInteractiveComponentSize.current(),
                    source.clone(),
                    None,
                )
                .test_tag("tutamaç"),
            interaction_source = Some(source),
        );
    }))
}

/// A navigator of two panes side by side (the C++'s `maxHorizontalPartitions = 2`).
#[track_caller]
fn two_pane_navigator() -> Rc<dyn ThreePaneScaffoldNavigator<i32>> {
    kompozable_block!({
        let directive = named_args!(
            calculate_pane_scaffold_directive(&current_window_adaptive_info()).copy(max_horizontal_partitions = 2)
        );
        let navigator: Rc<dyn ThreePaneScaffoldNavigator<i32>> =
            remember_list_detail_pane_scaffold_navigator(scaffold_directive = directive);
        navigator
    })
}

fn pane(tag: &'static str) -> impl Fn(&ThreePaneScaffoldPaneScope) + 'static {
    kompozable_lambda!(move |_: &ThreePaneScaffoldPaneScope| Box(modifier = Modifier.fill_max_size().test_tag(tag)))
}

fn drag(rule: &KompozTestRule, from: Offset, steps: &[f32]) {
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    rule.advance_frame();
    for d in steps {
        rule.host().pointer_move(Offset::new(from.x + d, from.y), -1, 0);
        rule.advance_frame();
    }
}

fn fling(rule: &KompozTestRule, dx: f32) {
    let from = bounds(rule, "tutamaç").center();
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    for k in 1..=3 {
        rule.advance_frame();
        rule.host().pointer_move(Offset::new(from.x + dx * k as f32 / 3.0, from.y), -1, 0);
    }
    rule.advance_frame();
    rule.host().pointer_up(Offset::new(from.x + dx, from.y), -1, 0);
    rule.wait_for_idle();
}

/// The handle's pill as drawn: its rect and color (the drawn rect in the handle narrower than 20).
fn pill(rule: &KompozTestRule) -> Option<(Rect, Color)> {
    let handle = bounds(rule, "tutamaç");
    let canvas: RecordingCanvas = rule.draw();
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .find(|(r, _)| handle.contains(r.center()) && r.width() < 20.0)
        .map(|(r, c)| (*r, *c))
}

fn pane_expansion_handle_drags_the_split_and_settles_on_an_anchor() {
    let rtl = mutable_state_of(false);
    let expansion: Slot<PaneExpansionState> = slot();
    let (r, e) = (rtl.clone(), expansion.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let e = e.clone();
            let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                let n = two_pane_navigator();
                let state = named_args!(remember_pane_expansion_state(
                    anchors = vec![
                        PaneExpansionAnchor::Proportion(0.0),
                        PaneExpansionAnchorOffset::from_start(360.dp()).into(),
                        PaneExpansionAnchor::Proportion(0.5),
                        PaneExpansionAnchorOffset::from_end(100.dp()).into(),
                    ]
                ));
                *e.borrow_mut() = Some(state.clone());
                let source = remember(MutableInteractionSource::new);
                NavigableListDetailPaneScaffold(
                    n,
                    list_pane = pane("liste"),
                    detail_pane = pane("ayrıntı"),
                    pane_expansion_state = Some(state),
                    pane_expansion_drag_handle = Some(drag_handle(Some(source))),
                );
            });
        }),
        IntSize::new(1000, 600),
    );
    let list = || bounds(&rule, "liste");
    let details = || bounds(&rule, "ayrıntı");
    let handle = || bounds(&rule, "tutamaç");
    let cs = light_color_scheme();
    let e = get(&expansion);
    // As the scaffold lays it out until dragged: the list 360 wide, the handle in the 24 dp between.
    assert!(
        near(list().width(), 360.0) && near(details().left, 384.0) && near(details().width(), 616.0),
        "{:?}",
        list()
    );
    assert!(e.is_unspecified());
    let h = handle();
    // (M15 vs C++: material3's VerticalDragHandle keeps the minimum interactive size, 48 × 48, its center between
    // the panes; the C++'s handle was the 24 dp spacer wide)
    assert!(
        near(h.width(), 48.0) && near(h.height(), 48.0) && near(h.center().x, 372.0) && near(h.center().y, 300.0),
        "{h:?}"
    );
    let (p, color) = pill(&rule).expect("the pill");
    assert!(
        near(p.width(), 4.0) && near(p.height(), 48.0) && near(p.center().x, 372.0) && color == cs.outline,
        "{p:?}"
    );
    // Dragged, it follows the finger (past the 8 dp touch slop) and grows. M15 vs C++: Compose's hit path skips a
    // move whose position in the handle didn't change (ComposeUiFlags.isTriggerMoveEventsWhenLocationHasNotChanged-
    // Enabled is off): once the handle has moved 40 px with the finger, the next 40 px move is no move to it — the
    // split 52 px over, not the C++'s 92.
    let c = handle().center();
    drag(&rule, c, &[20.0, 60.0, 100.0]);
    rule.advance_time_by(600);
    assert!(near(list().width(), 412.0) && near(details().left, 436.0) && !e.is_unspecified(), "{:?}", list());
    let (p, color) = pill(&rule).expect("the pill");
    assert!(near(p.width(), 12.0) && near(p.height(), 52.0) && color == cs.on_surface, "{p:?} {color:?}");
    // Held still and let go, the split settles on the nearest anchor: 360 dp from the start (the C++'s, from 92 px
    // over, half the room).
    rule.host().pointer_up(Offset::new(c.x + 100.0, c.y), -1, 0);
    rule.wait_for_idle();
    assert!(near(list().width(), 348.0) && near(details().width(), 1000.0 - 372.0), "{:?}", list());
    assert_eq!(e.current_anchor(), Some(PaneExpansionAnchorOffset::from_start(360.dp()).into()));
    let (p, color) = pill(&rule).expect("the pill");
    assert!(near(p.width(), 4.0) && color == cs.outline);
    // Flung: on to the next anchor that way, an offset from the end the last one.
    // (Compose's anchors are in the scaffold's whole 1000 px — its `onMeasured(outerBounds.width)` —, the list
    // ending 12 px, half the spacer, before the split: half way 488, 100 dp from the end 888 where the C++ had 876 of
    // its 976 dp; the start, 0, hides the list)
    fling(&rule, -60.0);
    assert_eq!(e.current_anchor(), Some(PaneExpansionAnchor::Proportion(0.0)));
    assert!(near(list().width(), 0.0), "{:?}", list());
    fling(&rule, 60.0);
    assert!(near(list().width(), 348.0), "{:?}", list());
    fling(&rule, 60.0);
    assert!(near(list().width(), 488.0), "{:?}", list());
    fling(&rule, 60.0);
    assert!(near(list().width(), 888.0), "{:?}", list());
    fling(&rule, 60.0);
    assert!(near(list().width(), 888.0), "{:?}", list());
    // Set in code (up to the room), then back to the scaffold's widths.
    e.set_first_pane_width(5000);
    rule.wait_for_idle();
    // (the whole width: the room is the scaffold's; the C++'s 976)
    assert!(near(list().width(), 1000.0) && near(details().width(), 0.0), "{:?}", list());
    e.set_first_pane_proportion(0.25);
    rule.wait_for_idle();
    assert!(near(list().width(), 244.0), "{:?}", list());
    e.clear();
    rule.wait_for_idle();
    assert!(near(list().width(), 360.0) && e.is_unspecified());
    // Dragged past the start it stops there, and coming back moves it at once.
    let c = handle().center();
    drag(&rule, c, &[-100.0, -300.0, -600.0]);
    assert!(near(list().width(), 0.0), "{:?}", list());
    rule.host().pointer_move(Offset::new(c.x - 550.0, c.y), -1, 0);
    rule.advance_time_by(300);
    // (50 px back: the list 12 px short of the split, 38)
    assert!(near(list().width(), 38.0), "{:?}", list());
    rule.host().pointer_up(Offset::new(c.x - 550.0, c.y), -1, 0);
    rule.wait_for_idle(); // (settled on the start: the drag's settling is a coroutine, cleared after it)
    e.clear();
    rule.wait_for_idle();
    // Right to left: the list on the right, dragging left widens it.
    rtl.set(true);
    rule.wait_for_idle();
    assert!(near(list().left, 640.0) && near(handle().center().x, 628.0), "{:?}", list());
    let c = handle().center();
    drag(&rule, c, &[-20.0, -60.0, -100.0]);
    rule.advance_time_by(600);
    // (the move lost as left to right: 52 px)
    assert!(near(list().width(), 412.0) && near(list().left, 588.0), "{:?}", list());
    rule.host().pointer_up(Offset::new(c.x - 100.0, c.y), -1, 0);
    rule.wait_for_idle();
    // (settled on half way; the C++'s too)
    assert!(near(list().width(), 488.0) && near(list().left, 512.0), "{:?}", list());
    // Flung left: wider. (Compose's anchor positions are from the left whatever the direction, the split's offset
    // too: flung left, it goes to the anchor left of it, 360 dp from the left — the list 628 wide, where the C++
    // mirrored its anchors and reached the one 100 dp from the end)
    fling(&rule, -60.0);
    assert!(near(list().width(), 628.0) && near(list().left, 372.0), "{:?}", list());
}

fn pane_expansion_handle_alone_brings_its_own_state() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let n = two_pane_navigator();
            NavigableListDetailPaneScaffold(
                n,
                list_pane = pane("liste"),
                detail_pane = pane("ayrıntı"),
                pane_expansion_drag_handle = Some(drag_handle(None)),
            );
        }),
        IntSize::new(1000, 600),
    );
    let c = bounds(&rule, "tutamaç").center();
    drag(&rule, c, &[20.0, 60.0, 108.0]);
    rule.advance_time_by(300);
    rule.host().pointer_up(Offset::new(c.x + 108.0, c.y), -1, 0);
    rule.wait_for_idle();
    // no anchors: where it was let go
    assert!(near(bounds(&rule, "liste").width(), 460.0), "{:?}", bounds(&rule, "liste"));
}

fn pane_expansion_without_a_handle_and_on_a_supporting_pane_scaffold() {
    let expansion: Slot<PaneExpansionState> = slot();
    let panes = mutable_state_of(2);
    let (e, p) = (expansion.clone(), panes.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            // (a scaffold of its directive and value as the window gives them: a navigator's value follows its
            // directive only as it navigates, Compose's; the C++'s scaffold took the partitions at once)
            let directive = named_args!(
                calculate_pane_scaffold_directive(&current_window_adaptive_info())
                    .copy(max_horizontal_partitions = p.get())
            );
            let main: ThreePaneScaffoldDestinationItem<i32> =
                ThreePaneScaffoldDestinationItem { pane: SupportingPaneScaffoldRole::Main, content_key: None };
            let value = named_args!(calculate_three_pane_scaffold_value_of_destination(
                directive.max_horizontal_partitions,
                &SupportingPaneScaffoldDefaults.adapt_strategies(),
                Some(&main),
            ));
            let state = remember_pane_expansion_state();
            *e.borrow_mut() = Some(state.clone());
            SupportingPaneScaffold(
                directive,
                value,
                main_pane = pane("ana"),
                supporting_pane = pane("destek"),
                pane_expansion_state = Some(state),
            );
        }),
        IntSize::new(1000, 600),
    );
    // The supporting pane keeps its 360 dp until the main pane's width is set; no handle.
    assert!(near(bounds(&rule, "ana").width(), 616.0) && near(bounds(&rule, "destek").width(), 360.0));
    rule.on_node_with_tag("tutamaç").assert_does_not_exist();
    get(&expansion).set_first_pane_width(700);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ana").width(), 700.0) && near(bounds(&rule, "destek").width(), 276.0));
    // One pane at a time: the main pane fills the window whatever the state says.
    panes.set(1);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ana").width(), 1000.0), "{:?}", bounds(&rule, "ana"));
    // (the hidden pane stays composed, Compose's hidden panes kept for their exit motion, but isn't shown)
    rule.on_node_with_tag("destek").assert_is_not_displayed();
}

/// The pane expansion part of the C++ test (its other states are material3's, ported there): a width set is saved
/// and comes back with the UI's state, the list as wide again.
fn material_states_come_back_after_the_process_ended() {
    fn app(registry: &SaveableStateRegistry, out: &Slot<PaneExpansionState>) -> KompozTestRule {
        let (r, out) = (registry.clone(), out.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let out = out.clone();
                ProvideSaveableStateRegistry(&r, move || {
                    let n = two_pane_navigator();
                    let state = remember_pane_expansion_state();
                    *out.borrow_mut() = Some(state.clone());
                    NavigableListDetailPaneScaffold(
                        n,
                        list_pane = pane("liste"),
                        detail_pane = pane("ayrıntı"),
                        pane_expansion_state = Some(state),
                    );
                });
            }),
            IntSize::new(1000, 600),
        )
    }
    let registry = SaveableStateRegistry::new(None);
    let pane_state: Slot<PaneExpansionState> = slot();
    let rule = app(&registry, &pane_state);
    get(&pane_state).set_first_pane_width(300);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "liste").width(), 300.0));
    let saved = registry.perform_save();
    drop(rule);
    let rule = app(&SaveableStateRegistry::new(Some(&saved)), &pane_state);
    assert!(!get(&pane_state).is_unspecified());
    assert!(near(bounds(&rule, "liste").width(), 300.0));
}

/// The state's public members: a width or a proportion set clears the other and the anchor; `animate_to` an
/// anchor not among the state's panics (Kotlin's `require`); a proportion outside 0..1 too.
fn pane_expansion_states_take_widths_proportions_and_anchors() {
    let expansion: Slot<PaneExpansionState> = slot();
    let scope: Slot<kompoz_coroutines::CoroutineScope> = slot();
    let (e, sc) = (expansion.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let e = e.clone();
            *sc.borrow_mut() = Some(kompoz_coroutines::remember_coroutine_scope());
            let n = two_pane_navigator();
            let state = named_args!(remember_pane_expansion_state(
                anchors = vec![PaneExpansionAnchor::Proportion(0.25), PaneExpansionAnchor::Proportion(0.75)],
                initial_anchored_index = 1
            ));
            *e.borrow_mut() = Some(state.clone());
            NavigableListDetailPaneScaffold(
                n,
                list_pane = pane("liste"),
                detail_pane = pane("ayrıntı"),
                pane_expansion_state = Some(state),
            );
        }),
        IntSize::new(1000, 600),
    );
    let e = get(&expansion);
    // At its initial anchor: three quarters of the room.
    assert_eq!(e.current_anchor(), Some(PaneExpansionAnchor::Proportion(0.75)));
    // (the anchors in the scaffold's whole width, the list 12 px, half the spacer, short of the split)
    assert!(near(bounds(&rule, "liste").width(), 738.0), "{:?}", bounds(&rule, "liste"));
    // Animated to the other anchor.
    let future = named_args!(e.animate_to(PaneExpansionAnchor::Proportion(0.25)));
    named_args!(get(&scope).launch(future));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "liste").width(), 238.0), "{:?}", bounds(&rule, "liste"));
    assert_eq!(e.current_anchor(), Some(PaneExpansionAnchor::Proportion(0.25)));
    e.set_first_pane_proportion(0.5);
    assert!(e.current_anchor().is_none() && !e.is_unspecified());
    let wrong = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| e.set_first_pane_proportion(1.5)));
    assert!(wrong.is_err());
}
