//! (WP8 helper) Unit tests of the pane expansion's, drag-to-resize's and pane margins' crate internals, as Compose's
//! behaviour: a bare layout measuring a drag handle as a scaffold does (`on_measured`, `on_expansion_offset_measured`,
//! the split at the dragged offset), the savers, the remember helpers, the drag handle's semantics in English and
//! Turkish. The UI cases run in one test, on one thread (the UI's).

use super::drag_to_resize_state::{DockedEdge, DragToResizeState, DragToResizeValue, click_to_resize};
use super::internal::remember_utils::{remember_persistently_with_key, remember_updated_ref};
use super::pane_expansion_draggable_modifier::pane_expansion_draggable_internal;
use super::pane_expansion_state::{
    PaneExpansionAnchor, PaneExpansionAnchorOffset, PaneExpansionState, PaneExpansionStateDataSaver,
    PaneExpansionStateKey, PaneExpansionStateKeySaver, remember_pane_expansion_state,
};
use super::pane_margins::{PaneMarginsImpl, WindowInsetsRulers, pane_margins_with_fixed_margins};
use super::pane_scaffold::{SemanticsProperties, min_touch_target_size, pane_scaffold_parent_data};
use super::pane_scaffold_role::ThreePaneScaffoldRole;
use super::three_pane_scaffold_value::TwoPaneExpansionStateKeyImpl;
use kompoz_foundation::gestures::DraggableState;
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::layout::PaddingValues;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Constraints, Density, Dp, IntSize, LayoutDirection, Offset, Rect, UnitExt};
use kompoz_runtime::{
    KompozitionLocalProvider, ProvideSaveableStateRegistry, SaveableStateRegistry, Saver, kompozable_lambda,
    mutable_state_of, named_args,
};
use kompoz_test::KompozTestRule;
use kompoz_ui::Modifier;
use kompoz_ui::layout::{Layout, LookaheadScope, measure_policy};
use kompoz_ui::semantics::{SemanticsActions, SemanticsConfiguration, SemanticsProperties as Props};
use kompoz_ui::strings::{LocalStrings, Strings as UiStrings};
use std::cell::RefCell;
use std::rc::Rc;

type Slot<T> = Rc<RefCell<Option<T>>>;

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A bare two-pane split as a scaffold measures it: the room `on_measured`, the handle 24 px wide centered at the
/// dragged offset (else the middle), the offset told back as measured.
fn split(
    state: Slot<PaneExpansionState>,
    anchors: Vec<PaneExpansionAnchor>,
    consume_drag_delta: Option<Rc<dyn Fn(f32) -> f32>>,
    semantics: Option<SemanticsProperties>,
    turkish: bool,
) -> impl kompoz_runtime::Content {
    kompozable_lambda!(move || {
        let strings = if turkish { UiStrings::turkish() } else { UiStrings::english() };
        let (state, anchors, consume_drag_delta, semantics) =
            (state.clone(), anchors.clone(), consume_drag_delta.clone(), semantics.clone());
        KompozitionLocalProvider(&[LocalStrings.provides(strings)], move || {
            let s = match consume_drag_delta.clone() {
                Some(consume) => {
                    named_args!(remember_pane_expansion_state(anchors = anchors.clone(), consume_drag_delta = consume))
                }
                None => named_args!(remember_pane_expansion_state(anchors = anchors.clone())),
            };
            *state.borrow_mut() = Some(s.clone());
            let semantics = semantics.clone();
            LookaheadScope(move |scope| {
                let handle = pane_expansion_draggable_internal(
                    named_args!(Modifier.size_wh(24.dp(), 400.dp())).test_tag("tutamaç"),
                    s.clone(),
                    48.dp(),
                    MutableInteractionSource::new(),
                    semantics.clone(),
                    Rc::new(|| 1.0),
                    scope.clone(),
                );
                let measured = s.clone();
                Layout(
                    content = move || Box(modifier = handle.clone()),
                    measure_policy = measure_policy(move |ms, measurables, c| {
                        let handle = measurables[0].measure(named_args!(c.copy(min_width = 0, min_height = 0)));
                        let touch = min_touch_target_size(measurables[0]);
                        assert_eq!(touch, Dp(48.0));
                        measured.on_measured(c.max_width, &ms.density());
                        let dragged = measured.current_dragging_offset();
                        let offset = if dragged != PaneExpansionState::Unspecified { dragged } else { c.max_width / 2 };
                        measured.on_expansion_offset_measured(offset);
                        let x = offset - handle.width() / 2;
                        ms.layout(c.max_width, c.max_height, move |p| p.place(&handle, x, 0))
                    }),
                );
            });
        });
    })
}

fn handle_x(rule: &KompozTestRule) -> f32 {
    bounds(rule, "tutamaç").center().x
}

fn anchors() -> Vec<PaneExpansionAnchor> {
    vec![
        PaneExpansionAnchor::Proportion(0.25),
        PaneExpansionAnchor::Proportion(0.5),
        PaneExpansionAnchorOffset::from_end(100.dp()).into(),
    ]
}

/// Dragging the handle moves the split and settles it on the nearest anchor (the next one its way after a fast
/// fling), animated; its default semantics tell the anchors in the UI's language and click to the next one.
fn a_dragged_handle_settles_on_its_anchors() {
    let state: Slot<PaneExpansionState> = Rc::default();
    let rule =
        KompozTestRule::with_viewport(split(state.clone(), anchors(), None, None, false), IntSize::new(1000, 400));
    let s = get(&state);
    assert_eq!(s.max_expansion_width(), 1000);
    assert!(s.is_unspecified() && s.current_anchor().is_none());
    assert!((handle_x(&rule) - 500.0).abs() < 0.5);
    // (what the scaffold measured: the split's offset, the anchors' positions by position; the drag consumer taking
    // nothing by default)
    assert_eq!(s.current_measured_dragging_offset(), 500);
    assert_eq!(s.measured_anchor_positions().iter().collect::<Vec<_>>(), [(0, 250), (1, 500), (2, 900)]);
    assert_eq!((s.consume_drag_delta())(7.0), 7.0);
    let semantics = rule.on_node_with_tag("tutamaç").fetch_semantics_node();
    let config = semantics.config();
    assert_eq!(config.get(Props::ContentDescription), Some(vec!["Pane expansion drag handle".to_string()]));
    assert_eq!(config.get(Props::StateDescription), None);
    // The next anchor after the middle: 100 dp from the end.
    let click = config.get(SemanticsActions::OnClick).expect("a click to the next anchor");
    assert_eq!(click.label.as_deref(), Some("Change pane split to 100 DPs from end"));
    // A slow drag of 120 px right: settles back on the middle (the nearest).
    rule.on_node_with_tag("tutamaç").perform_touch_input(|t| {
        t.down(Offset::new(12.0, 200.0));
        for _ in 0..12 {
            t.move_by(Offset::new(10.0, 0.0));
            t.r#move(100);
        }
        t.up();
    });
    rule.wait_for_idle();
    assert!((handle_x(&rule) - 500.0).abs() < 0.5, "{}", handle_x(&rule));
    assert_eq!(s.current_anchor(), Some(PaneExpansionAnchor::Proportion(0.5)));
    assert!(!s.is_dragging_or_settling() && !s.is_unspecified());
    let config = rule.on_node_with_tag("tutamaç").fetch_semantics_node().config();
    assert_eq!(config.get(Props::StateDescription), Some("Current pane split, 50 percent".to_string()));
    // A fast fling right: the next anchor its way, 100 dp from the end.
    rule.on_node_with_tag("tutamaç")
        .perform_touch_input(|t| t.swipe_with(Offset::new(12.0, 200.0), Offset::new(112.0, 200.0), 50));
    rule.wait_for_idle();
    assert!((handle_x(&rule) - 900.0).abs() < 0.5, "{}", handle_x(&rule));
    assert_eq!(s.current_anchor(), Some(PaneExpansionAnchorOffset::from_end(100.dp()).into()));
    // The click action: to the next anchor, wrapping to the first.
    rule.on_node_with_tag("tutamaç").perform_semantics_action(SemanticsActions::OnClick);
    rule.wait_for_idle();
    assert!((handle_x(&rule) - 250.0).abs() < 0.5, "{}", handle_x(&rule));
    assert_eq!(s.current_anchor(), Some(PaneExpansionAnchor::Proportion(0.25)));
    // A new room: the split follows its anchor (not dragging).
    rule.host().set_viewport_size(IntSize::new(800, 400));
    rule.wait_for_idle();
    assert!((handle_x(&rule) - 200.0).abs() < 0.5);
    // Set from code: a width, then cleared.
    s.set_first_pane_width(300);
    assert_eq!((s.first_pane_width(), s.current_anchor()), (300, None));
    assert!(s.first_pane_proportion().is_nan() && s.current_dragging_offset() == PaneExpansionState::Unspecified);
    s.set_first_pane_width(5000);
    assert_eq!(s.first_pane_width(), 800); // (within the room)
    s.set_first_pane_proportion(0.4);
    assert_eq!((s.first_pane_proportion(), s.first_pane_width()), (0.4, PaneExpansionState::Unspecified));
    s.clear();
    assert!(s.is_unspecified());
}

/// The handle's texts in Turkish (the l10n's), the drag's delta taken first by `consume_drag_delta`, a given
/// semantics block's click making the handle clickable.
fn the_handle_speaks_the_uis_language_and_takes_its_options() {
    let state: Slot<PaneExpansionState> = Rc::default();
    let consumed = Rc::new(RefCell::new(Vec::<f32>::new()));
    let c = consumed.clone();
    let half: Rc<dyn Fn(f32) -> f32> = Rc::new(move |delta| {
        c.borrow_mut().push(delta);
        delta / 2.0
    });
    let rule =
        KompozTestRule::with_viewport(split(state.clone(), anchors(), Some(half), None, true), IntSize::new(1000, 400));
    let config = rule.on_node_with_tag("tutamaç").fetch_semantics_node().config();
    assert_eq!(
        config.get(Props::ContentDescription),
        Some(vec!["Bölmeyi genişletmek için sürükleme tutamacı".to_string()])
    );
    let click = config.get(SemanticsActions::OnClick).expect("a click");
    assert_eq!(click.label.as_deref(), Some("Bölme oranını Sondan 100 DP olarak değiştirin"));
    // Half of each delta moves the split (the anchors aside: dispatched straight).
    let s = get(&state);
    s.draggable_state().dispatch_raw_delta(100.0);
    assert_eq!(consumed.borrow().as_slice(), &[100.0]);
    assert_eq!(s.current_dragging_offset(), 550);
    rule.wait_for_idle();
    assert!((handle_x(&rule) - 550.0).abs() < 0.5);
    s.snap_to_anchor(PaneExpansionAnchor::Proportion(0.25));
    assert_eq!(s.current_dragging_offset(), 250);
    drop(rule);

    // Semantics of one's own: its click clicks the handle too.
    let clicks = Rc::new(RefCell::new(0));
    let k = clicks.clone();
    let semantics: SemanticsProperties = Rc::new(move |c: &mut SemanticsConfiguration| {
        c.set_content_description("kendi tutamacım");
        let k = k.clone();
        named_args!(c.on_click(
            action = move || {
                *k.borrow_mut() += 1;
                true
            }
        ));
    });
    let rule = KompozTestRule::with_viewport(
        split(state.clone(), anchors(), None, Some(semantics), false),
        IntSize::new(1000, 400),
    );
    rule.on_node_with_tag("tutamaç").assert(kompoz_test::has_content_description("kendi tutamacım", false, false));
    rule.on_node_with_tag("tutamaç").perform_click();
    rule.wait_for_idle();
    assert_eq!(*clicks.borrow(), 1);
}

/// The state is remembered per key (two keys, two data; back to a key, its data again) and saved with them.
fn states_are_remembered_and_saved_per_key() {
    let key = mutable_state_of(PaneExpansionStateKey::Default);
    let two = PaneExpansionStateKey::TwoPane(TwoPaneExpansionStateKeyImpl {
        first_expanded_pane: ThreePaneScaffoldRole::Primary,
        second_expanded_pane: ThreePaneScaffoldRole::Secondary,
    });
    let state: Slot<PaneExpansionState> = Rc::default();
    fn saved(
        registry: &SaveableStateRegistry,
        key: kompoz_runtime::MutableState<PaneExpansionStateKey>,
        state: &Slot<PaneExpansionState>,
    ) -> KompozTestRule {
        let (r, s) = (registry.clone(), state.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let (s, key) = (s.clone(), key.clone());
            ProvideSaveableStateRegistry(&r, move || {
                *s.borrow_mut() = Some(named_args!(remember_pane_expansion_state(key = key.get())));
            });
        }))
    }
    let registry = SaveableStateRegistry::new(None);
    let rule = saved(&registry, key.clone(), &state);
    let s = get(&state);
    s.set_first_pane_width(120);
    key.set(two);
    rule.wait_for_idle();
    let s2 = get(&state);
    assert!(s == s2); // (the same state, the other key's data once restored)
    assert_eq!(s.first_pane_width(), PaneExpansionState::Unspecified);
    s.set_first_pane_proportion(0.3);
    key.set(PaneExpansionStateKey::Default);
    rule.wait_for_idle();
    assert_eq!(get(&state).first_pane_width(), PaneExpansionState::Unspecified); // (not measured: no room yet)
    let saved_values = registry.perform_save();
    drop(rule);
    // Restored: each key's data back.
    let rule = saved(&SaveableStateRegistry::new(Some(&saved_values)), key.clone(), &state);
    assert!(get(&state).first_pane_proportion().is_nan());
    key.set(two);
    rule.wait_for_idle();
    assert_eq!(get(&state).first_pane_proportion(), 0.3);
}

/// The savers of the keys and the data, round trips (Compose's PaneExpansionStateKeySaver /
/// PaneExpansionStateDataSaver).
fn the_savers_round_trip() {
    let keys: Saver<PaneExpansionStateKey> = PaneExpansionStateKeySaver();
    let two = PaneExpansionStateKey::TwoPane(TwoPaneExpansionStateKeyImpl {
        first_expanded_pane: ThreePaneScaffoldRole::Secondary,
        second_expanded_pane: ThreePaneScaffoldRole::Tertiary,
    });
    for k in [PaneExpansionStateKey::Default, two] {
        assert_eq!(keys.restore(&keys.save(&k)), Some(k));
    }
    let datas = PaneExpansionStateDataSaver();
    for anchor in [
        None,
        Some(PaneExpansionAnchor::Proportion(0.25)),
        Some(PaneExpansionAnchorOffset::from_start(12.dp()).into()),
        Some(PaneExpansionAnchorOffset::from_end(34.dp()).into()),
    ] {
        let data = named_args!(super::pane_expansion_state::PaneExpansionStateData(
            first_pane_width = 7,
            first_pane_proportion = 0.5,
            current_dragging_offset = 9,
            current_anchor = anchor
        ));
        let back = datas.restore(&datas.save(&data)).expect("restored");
        assert!(back == data, "{anchor:?}");
    }
}

fn int_saver() -> Saver<i32> {
    use kompoz_runtime::Saveable;
    Saver::new(|v: &i32| v.save(), i32::restore)
}

/// `remember_persistently_with_key` makes a value once per key; `remember_updated_ref` keeps the latest.
fn remember_helpers_keep_their_values() {
    let key = mutable_state_of(1);
    let made = Rc::new(RefCell::new(Vec::new()));
    let seen: Slot<(i32, i32)> = Rc::default();
    let (k, m, s) = (key.clone(), made.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let key = k.get();
        let m = m.clone();
        let value = remember_persistently_with_key(key, int_saver(), int_saver(), move || {
            m.borrow_mut().push(key);
            key * 10
        });
        let latest = remember_updated_ref(key);
        *s.borrow_mut() = Some((value, latest.value()));
    }));
    assert_eq!(get(&seen), (10, 1));
    key.set(2);
    rule.wait_for_idle();
    key.set(1);
    rule.wait_for_idle();
    assert_eq!(get(&seen), (10, 1));
    assert_eq!(made.borrow().as_slice(), &[1, 2]);
}

/// A drag-to-resize state: dragged within its range (collapsed and expanded at its ends), clicked to the next
/// state with its texts, saved and restored.
fn drag_to_resize_states_resize_within_their_range() {
    let state: Slot<DragToResizeState> = Rc::default();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let st = super::drag_to_resize_state::remember_drag_to_resize_state(
                DockedEdge::Start,
                100.dp(),
                Dp::Unspecified,
            );
            *s.borrow_mut() = Some(st.clone());
            let measured = st.clone();
            Layout(
                content = move || Box(modifier = click_to_resize(Modifier.test_tag("kenar").size(10.dp()), st.clone())),
                measure_policy = measure_policy(move |ms, measurables, c| {
                    let width = measured.get_dragged_width(300, 50, c.max_width);
                    let p = measurables[0].measure(Constraints::fixed(width, 10));
                    ms.layout(c.max_width, c.max_height, move |pl| pl.place(&p, 0, 0))
                }),
            );
        }),
        IntSize::new(1000, 400),
    );
    let s = get(&state);
    assert_eq!(s.size(), 300.0);
    assert_eq!(s.state(), DragToResizeValue::Default);
    assert_eq!(s.size_range(), 100.0..=1000.0);
    s.dispatch_raw_delta(50.0);
    assert_eq!((s.size(), s.state()), (350.0, DragToResizeValue::Dragged));
    s.dispatch_raw_delta(-1000.0);
    assert_eq!((s.size(), s.state()), (100.0, DragToResizeValue::Collapsed));
    rule.wait_for_idle();
    let config = rule.on_node_with_tag("kenar").fetch_semantics_node().config();
    assert_eq!(config.get(Props::StateDescription), Some("collapsed".to_string()));
    assert_eq!(config.get(SemanticsActions::OnClick).and_then(|a| a.label), Some("partially expand".to_string()));
    // Dragged again, then clicked: expanded, collapsed, then the size dragged last (animated). Compose's quirk kept:
    // the animation's first value is the range's start, which collapses it again, so it ends at the default size
    // still told collapsed.
    s.dispatch_raw_delta(250.0);
    assert_eq!((s.size(), s.state()), (350.0, DragToResizeValue::Dragged));
    rule.wait_for_idle();
    rule.on_node_with_tag("kenar").perform_click();
    rule.wait_for_idle();
    assert_eq!((s.size(), s.state()), (1000.0, DragToResizeValue::Expanded));
    rule.on_node_with_tag("kenar").perform_click();
    rule.wait_for_idle();
    assert_eq!((s.size(), s.state()), (100.0, DragToResizeValue::Collapsed));
    rule.on_node_with_tag("kenar").perform_click();
    rule.wait_for_idle();
    assert_eq!((s.size(), s.state()), (350.0, DragToResizeValue::Collapsed));
    s.animate_to_state(DragToResizeValue::Expanded);
    rule.wait_for_idle();
    assert_eq!((s.size(), s.state()), (1000.0, DragToResizeValue::Expanded));
    // Right to left, the start is the right: a drag right shrinks it.
    let rtl = DragToResizeState::of_edge(DockedEdge::Start, LayoutDirection::Rtl);
    assert_eq!(rtl.kind(), super::drag_to_resize_state::DragToResizeKind::Right);
    let saver = DragToResizeState::Saver(DockedEdge::Start, LayoutDirection::Ltr);
    let back = saver.restore(&saver.save(&s)).expect("restored");
    // (Compose's restore sets the size before the ranges, still 0..0, so the size comes back coerced to 0 — the
    // next measurement, with the remembered sizes set again, coerces it into its range)
    assert_eq!((back.size(), back.state(), back.size_range()), (0.0, DragToResizeValue::Expanded, 100.0..=1000.0));
}

/// A pane's margins keep its edges inside the fixed margins and the rulers' insets.
fn pane_margins_keep_a_pane_inside() {
    let rulers = WindowInsetsRulers::new();
    let r = rulers.clone();
    let seen: Slot<(i32, i32, i32, i32)> = Rc::default();
    let s = seen.clone();
    let _rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (r, s) = (r.clone(), s.clone());
            let inner = r.clone();
            Layout(
                content = move || {
                    let modifier = pane_margins_with_fixed_margins(
                        Modifier.test_tag("bölme"),
                        PaddingValues::new(10.dp(), 20.dp(), 0.dp(), 0.dp()),
                        std::slice::from_ref(&inner),
                    );
                    Box(modifier = modifier);
                },
                measure_policy = measure_policy(move |ms, measurables, c| {
                    let data = pane_scaffold_parent_data(measurables[0]).expect("parent data");
                    let p = measurables[0].measure(c);
                    let (r, s) = (r.clone(), s.clone());
                    ms.layout_with_rulers(
                        c.max_width,
                        c.max_height,
                        move |rs| {
                            rs.provides(r.left.clone(), 30.0);
                            rs.provides(r.right.clone(), 900.0);
                        },
                        move |pl| {
                            let m = &data.pane_margins;
                            *s.borrow_mut() = Some((
                                m.get_pane_left(pl, 5),
                                m.get_pane_top(pl, 0),
                                m.get_pane_right(pl, 1000, 1000),
                                m.get_pane_bottom(pl, 400, 400),
                            ));
                            pl.place(&p, 0, 0);
                        },
                    )
                }),
            );
        }),
        IntSize::new(1000, 400),
    );
    assert_eq!(get(&seen), (30, 20, 900, 400));
    let plain =
        PaneMarginsImpl::new(PaddingValues::all(5.dp()), Vec::new(), &Density((2.0, 1.0)), LayoutDirection::Ltr);
    let _ = plain;
}

#[test]
fn pane_expansion_internals() {
    kompoz_runtime::set_ui_thread();
    a_dragged_handle_settles_on_its_anchors();
    the_handle_speaks_the_uis_language_and_takes_its_options();
    states_are_remembered_and_saved_per_key();
    the_savers_round_trip();
    remember_helpers_keep_their_values();
    drag_to_resize_states_resize_within_their_range();
    pane_margins_keep_a_pane_inside();
}
