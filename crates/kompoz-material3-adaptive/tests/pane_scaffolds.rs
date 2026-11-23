//! The three-pane scaffolds as Compose's adaptive-layout and adaptive-navigation 1.2.0 have them: the C++ Kompoz's
//! pane scaffold tests by name (written as a Compose user writes them: a navigator, the navigable scaffold, its
//! panes `AnimatedPane`s; where Compose's navigation differs from the C++'s, Compose's is asserted and said), and
//! Compose's behaviour of the values, directives, strategies, navigators, levitated panes and preferred sizes.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    list_detail_pane_scaffold_shows_one_pane_or_two,
    supporting_pane_scaffold_puts_its_supporting_pane_at_the_end_or_slides_it_in,
    scaffold_values_follow_the_strategies_and_the_destinations,
    directives_follow_the_window,
    navigators_go_back_by_their_behavior_and_come_back_saved,
    a_levitated_pane_floats_over_the_others_behind_its_scrim,
    preferred_widths_size_the_panes,
);

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn launch(scope: &Slot<CoroutineScope>, f: impl std::future::Future + 'static) {
    named_args!(get(scope).launch(async move {
        f.await;
    }));
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text)).get_unclipped_bounds_in_root()
}

fn has_text(rule: &KompozTestRule, text: &str) -> bool {
    named_args!(rule.on_node_with_text(text)).exists()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

/// Escape pressed: back, from the root.
fn escape(rule: &KompozTestRule) -> bool {
    rule.host().key_down(Key::Escape, KeyModifiers::default())
}

type Navigator = Rc<dyn ThreePaneScaffoldNavigator<i32>>;

/// The C++'s list-detail scaffold of five cities, as Compose writes it.
#[kompozable]
fn Cities(nav: Slot<Navigator>, scope_out: Slot<CoroutineScope>) {
    let n = remember_list_detail_pane_scaffold_navigator::<i32>();
    *nav.borrow_mut() = Some(n.clone());
    let scope = remember_coroutine_scope();
    *scope_out.borrow_mut() = Some(scope.clone());
    let detail = n.clone();
    NavigableListDetailPaneScaffold(
        n.clone(),
        list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope) = (n.clone(), scope.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    let (n, scope) = (n.clone(), scope.clone());
                    Column(
                        content = move || {
                            for i in 0..5 {
                                let (n, scope) = (n.clone(), scope.clone());
                                Text(
                                    format!("Şehir {i}"),
                                    modifier = Modifier
                                        .clickable(
                                            on_click = move || {
                                                named_args!(scope.launch(
                                                    n.navigate_to(ListDetailPaneScaffoldRole::Detail, Some(i))
                                                ));
                                            },
                                        )
                                        .test_tag(format!("liste{i}")),
                                );
                            }
                        },
                    );
                },
            );
        },
        detail_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let n = detail.clone();
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    let item = n.current_destination().and_then(|d| d.content_key);
                    Text(
                        item.map_or("Seçilmedi".to_string(), |i| format!("Ayrıntı {i}")),
                        modifier = Modifier.test_tag("ayrıntı"),
                    );
                },
            );
        },
    );
}

fn list_detail_pane_scaffold_shows_one_pane_or_two() {
    let (nav, scope): (Slot<Navigator>, Slot<CoroutineScope>) = (slot(), slot());
    let (n, s) = (nav.clone(), scope.clone());
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(move || Cities(n.clone(), s.clone())), IntSize::new(400, 800));
    // Compact: one pane at a time.
    let navigator = get(&nav);
    assert!(rule.on_node_with_tag("liste0").exists() && !rule.on_node_with_tag("ayrıntı").exists());
    assert!(!navigator.can_navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange));
    let third = bounds(&rule, "liste3");
    tap(&rule, Offset::new(third.left + 10.0, (third.top + third.bottom) / 2.0));
    rule.advance_time_by(48);
    assert!(text_bounds(&rule, "Ayrıntı 3").left > 10.0, "sliding in from the end");
    rule.wait_for_idle();
    assert!(!rule.on_node_with_tag("liste0").exists() && near(text_bounds(&rule, "Ayrıntı 3").left, 0.0));
    let current = navigator.current_destination().unwrap();
    assert!(navigator.can_navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange));
    assert_eq!((current.pane, current.content_key), (ListDetailPaneScaffoldRole::Detail, Some(3)));
    // Another detail: in place, no slide (the scaffold's value is the same).
    launch(&scope, navigator.navigate_to_with(ListDetailPaneScaffoldRole::Detail, Some(4)));
    rule.advance_frame();
    assert!(!has_text(&rule, "Ayrıntı 3") && near(text_bounds(&rule, "Ayrıntı 4").left, 0.0));
    // (Compose's default back pops until the scaffold's value changes — to the list at once; the C++'s back went to
    // the previous detail first: `PopLatest` does that.)
    assert_eq!(
        navigator.peek_previous_scaffold_value_with(BackNavigationBehavior::PopUntilScaffoldValueChange).secondary,
        PaneAdaptedValue::Expanded
    );
    launch(&scope, navigator.navigate_back_with(BackNavigationBehavior::PopLatest));
    rule.wait_for_idle();
    assert!(has_text(&rule, "Ayrıntı 3"));
    // Back: the list again, coming in from the start while the detail leaves. (Compose's leaving pane takes no
    // interaction: its semantics are cleared — the C++ found its old text there.)
    assert!(escape(&rule));
    rule.advance_frame();
    rule.advance_time_by(48);
    assert!(!has_text(&rule, "Ayrıntı 3") && !has_text(&rule, "Seçilmedi"));
    assert!(bounds(&rule, "liste0").left < -10.0);
    rule.wait_for_idle();
    assert!(rule.on_node_with_tag("liste0").exists() && !rule.on_node_with_tag("ayrıntı").exists());
    assert!(!navigator.can_navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange));
    assert!(!escape(&rule), "nothing to go back to: the platform's back");
    drop(rule);

    // Expanded: side by side, the list 360 dp, the detail after the 24 dp spacer.
    let (n, s) = (nav.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || Cities(n.clone(), s.clone())),
        IntSize::new(1000, 800),
    );
    let navigator = get(&nav);
    assert!(rule.on_node_with_tag("liste0").exists() && has_text(&rule, "Seçilmedi"));
    assert!(near(bounds(&rule, "ayrıntı").left, 360.0 + 24.0));
    rule.on_node_with_tag("liste2").perform_click();
    rule.wait_for_idle();
    assert!(has_text(&rule, "Ayrıntı 2") && rule.on_node_with_tag("liste0").exists());
    launch(&scope, navigator.navigate_to_with(ListDetailPaneScaffoldRole::Detail, Some(4)));
    rule.wait_for_idle();
    assert!(has_text(&rule, "Ayrıntı 4"));
    // (Both panes stay shown whatever the destination: back by the content — Compose's PopUntilContentChange — to
    // the previous detail, as the C++'s back; the default back has nothing to go back to and clears the history.)
    assert!(!navigator.can_navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange));
    launch(&scope, navigator.navigate_back_with(BackNavigationBehavior::PopUntilContentChange));
    rule.wait_for_idle();
    assert!(has_text(&rule, "Ayrıntı 2"));
    launch(&scope, navigator.navigate_back_with(BackNavigationBehavior::PopUntilContentChange));
    rule.wait_for_idle();
    assert!(has_text(&rule, "Seçilmedi") && !navigator.can_navigate_back_with(BackNavigationBehavior::PopLatest));
}

/// The C++'s supporting pane scaffold: an article and its related item.
#[kompozable]
fn Article(nav: Slot<Navigator>, scope_out: Slot<CoroutineScope>) {
    let n = remember_supporting_pane_scaffold_navigator::<i32>();
    *nav.borrow_mut() = Some(n.clone());
    let scope = remember_coroutine_scope();
    *scope_out.borrow_mut() = Some(scope.clone());
    let supporting = n.clone();
    NavigableSupportingPaneScaffold(
        n.clone(),
        main_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope) = (n.clone(), scope.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    let (n, scope) = (n.clone(), scope.clone());
                    Column(
                        content = move || {
                            Text("Makale", modifier = Modifier.test_tag("ana"));
                            let (n, scope) = (n.clone(), scope.clone());
                            Text(
                                "İlgili",
                                modifier = Modifier
                                    .clickable(
                                        on_click = move || {
                                            named_args!(scope.launch(
                                                n.navigate_to(SupportingPaneScaffoldRole::Supporting, Some(7))
                                            ));
                                        },
                                    )
                                    .test_tag("aç"),
                            );
                        },
                    );
                },
            );
        },
        supporting_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let n = supporting.clone();
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    let item = n.current_destination().and_then(|d| d.content_key);
                    Text(
                        item.map_or("Destek".to_string(), |i| format!("Destek {i}")),
                        modifier = Modifier.test_tag("destek"),
                    );
                },
            );
        },
    );
}

fn supporting_pane_scaffold_puts_its_supporting_pane_at_the_end_or_slides_it_in() {
    let (nav, scope): (Slot<Navigator>, Slot<CoroutineScope>) = (slot(), slot());
    let (n, s) = (nav.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || Article(n.clone(), s.clone())),
        IntSize::new(400, 800),
    );
    let navigator = get(&nav);
    // Compact (a medium height: one vertical partition, no reflow): the main pane.
    assert!(rule.on_node_with_tag("ana").exists() && !rule.on_node_with_tag("destek").exists());
    assert!(!navigator.can_navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange));
    let open = bounds(&rule, "aç");
    tap(&rule, open.center());
    rule.advance_time_by(48);
    assert!(text_bounds(&rule, "Destek 7").left > 10.0, "sliding in from the end");
    rule.wait_for_idle();
    assert!(!rule.on_node_with_tag("ana").exists() && near(text_bounds(&rule, "Destek 7").left, 0.0));
    let current = navigator.current_destination().unwrap();
    assert_eq!((current.pane, current.content_key), (SupportingPaneScaffoldRole::Supporting, Some(7)));
    assert!(escape(&rule), "back: the main pane again");
    rule.wait_for_idle();
    assert!(rule.on_node_with_tag("ana").exists() && !rule.on_node_with_tag("destek").exists());
    assert!(!navigator.can_navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange));
    drop(rule);

    // Expanded: side by side, the supporting pane at the end.
    let (n, s) = (nav.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || Article(n.clone(), s.clone())),
        IntSize::new(1000, 800),
    );
    let navigator = get(&nav);
    assert!(rule.on_node_with_tag("ana").exists() && has_text(&rule, "Destek"));
    assert!(near(bounds(&rule, "destek").left, 1000.0 - 360.0) && near(bounds(&rule, "ana").left, 0.0));
    launch(&scope, navigator.navigate_to_with(SupportingPaneScaffoldRole::Supporting, Some(3)));
    rule.wait_for_idle();
    assert!(has_text(&rule, "Destek 3") && rule.on_node_with_tag("ana").exists());
    drop(rule);

    // Two panes asked for on a medium width (Compose's calculatePaneScaffoldDirectiveWithTwoPanesOnMediumWidth: 24 dp
    // apart), the supporting pane 200 dp; right to left, the supporting one on the left.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                let directive = named_args!(calculate_pane_scaffold_directive_with_two_panes_on_medium_width(
                    &current_window_adaptive_info()
                ));
                let n: Navigator = remember_supporting_pane_scaffold_navigator(scaffold_directive = directive);
                NavigableSupportingPaneScaffold(
                    n,
                    main_pane = |pane: &ThreePaneScaffoldPaneScope| {
                        pane.AnimatedPane(
                            modifier = Modifier.test_tag("ana"),
                            content = |_: &AnimatedPaneScope| {
                                Text("Makale");
                            },
                        );
                    },
                    supporting_pane = |pane: &ThreePaneScaffoldPaneScope| {
                        let modifier = pane.preferred_width(Modifier.test_tag("destek"), 200.dp());
                        pane.AnimatedPane(modifier = modifier, content = |_: &AnimatedPaneScope| Text("Destek"));
                    },
                );
            });
        }),
        IntSize::new(600, 800),
    );
    let (supporting, main) = (bounds(&rule, "destek"), bounds(&rule, "ana"));
    assert_eq!(supporting, Rect::new(0.0, 0.0, 200.0, 800.0));
    assert_eq!(main, Rect::new(224.0, 0.0, 600.0, 800.0));
}

fn scaffold_values_follow_the_strategies_and_the_destinations() {
    let hide = || ThreePaneScaffoldAdaptStrategies(AdaptStrategy::Hide, AdaptStrategy::Hide, AdaptStrategy::Hide);
    let item = |pane| ThreePaneScaffoldDestinationItem::<i32> { pane, content_key: None };
    let (p, s, t) = (ThreePaneScaffoldRole::Primary, ThreePaneScaffoldRole::Secondary, ThreePaneScaffoldRole::Tertiary);
    let (e, h) = (PaneAdaptedValue::Expanded, PaneAdaptedValue::Hidden);
    // No destination: by priority, as many as there are partitions.
    let one = calculate_three_pane_scaffold_value(1, &hide(), &[] as &[ThreePaneScaffoldDestinationItem<i32>]);
    assert_eq!(one, ThreePaneScaffoldValue(e.clone(), h.clone(), h.clone()));
    let two = calculate_three_pane_scaffold_value(2, &hide(), &[] as &[ThreePaneScaffoldDestinationItem<i32>]);
    assert_eq!(two, ThreePaneScaffoldValue(e.clone(), e.clone(), h.clone()));
    // The destinations first, from the most recent.
    let history = [item(s), item(t)];
    let value = calculate_three_pane_scaffold_value(2, &hide(), &history);
    assert_eq!(value, ThreePaneScaffoldValue(h.clone(), e.clone(), e.clone()));
    let current = calculate_three_pane_scaffold_value_of_destination(1, &hide(), Some(&item(t)));
    assert_eq!(current, ThreePaneScaffoldValue(h.clone(), h.clone(), e.clone()));
    // Reflow: only in one horizontal partition of more vertical ones, under an expanded pane.
    let reflow = ThreePaneScaffoldAdaptStrategies(AdaptStrategy::Hide, AdaptStrategy::reflow(p), AdaptStrategy::Hide);
    let reflowed =
        named_args!(calculate_three_pane_scaffold_value(1, &reflow, &[item(s)], max_vertical_partitions = 2));
    assert_eq!(reflowed, ThreePaneScaffoldValue(e.clone(), PaneAdaptedValue::reflowed(p), h.clone()));
    assert_eq!(reflowed.secondary.to_string(), "PaneAdaptedValue[Reflowed to Primary]");
    let not_reflowed = calculate_three_pane_scaffold_value(1, &reflow, &[item(s)]);
    assert_eq!(not_reflowed, ThreePaneScaffoldValue(h.clone(), e.clone(), h.clone()));
    // Levitate: the current destination levitated, the rest as without it; elsewhere hidden.
    let scrim: Rc<dyn Fn()> = Rc::new(|| {});
    let levitate = ThreePaneScaffoldAdaptStrategies(
        AdaptStrategy::Hide,
        AdaptStrategy::Hide,
        AdaptStrategy::levitate(Alignment::BottomCenter, Some(scrim.clone())),
    );
    let levitated = calculate_three_pane_scaffold_value(2, &levitate, &[item(s), item(t)]);
    assert_eq!(levitated.tertiary, PaneAdaptedValue::levitated(Alignment::BottomCenter, Some(scrim.clone())));
    assert_ne!(levitated.tertiary, PaneAdaptedValue::levitated(Alignment::BottomCenter, Some(Rc::new(|| {}))));
    assert_eq!((levitated.primary.clone(), levitated.secondary.clone()), (e.clone(), e.clone()));
    let away = calculate_three_pane_scaffold_value(2, &levitate, &[item(t), item(s)]);
    assert_eq!(away.tertiary, h);
    // Strategies' helpers and texts.
    let only = AdaptStrategy::levitate(Alignment::Center, None);
    assert_eq!(only.only_if(false), AdaptStrategy::Hide);
    assert_eq!(only.only_if_single_pane(&PaneScaffoldDirective::Default()), only);
    assert_eq!(AdaptStrategy::Hide.to_string(), "AdaptStrategy[Hide]");
    assert_eq!(AdaptStrategy::reflow(p).to_string(), "AdaptStrategy[Reflow to Primary]");
    assert_eq!(
        two.to_string(),
        "ThreePaneScaffoldValue(primary=PaneAdaptedValue[Expanded], secondary=PaneAdaptedValue[Expanded], \
         tertiary=PaneAdaptedValue[Hidden])"
    );
    // The defaults of the two scaffolds.
    assert_eq!(*named_args!(SupportingPaneScaffoldDefaults.adapt_strategies()).get(s), AdaptStrategy::reflow(p));
    assert_eq!(*named_args!(ListDetailPaneScaffoldDefaults.adapt_strategies()).get(s), AdaptStrategy::Hide);
    assert_eq!(
        (ListDetailPaneScaffoldRole::List, ListDetailPaneScaffoldRole::Detail, SupportingPaneScaffoldRole::Supporting),
        (s, p, s)
    );
}

fn directives_follow_the_window() {
    let info = |w: f32, h: f32, posture: Posture| {
        WindowAdaptiveInfo(
            named_args!(WindowSizeClass::BREAKPOINTS_V2().compute_window_size_class_of_float(w, h)),
            posture,
        )
    };
    let compact = calculate_pane_scaffold_directive(&info(400.0, 800.0, Posture()));
    assert_eq!(compact, PaneScaffoldDirective::Default());
    let tall = calculate_pane_scaffold_directive(&info(700.0, 1000.0, Posture()));
    assert_eq!((tall.max_horizontal_partitions, tall.max_vertical_partitions), (1, 2));
    assert_eq!(tall.vertical_partition_spacer_size, 24.dp());
    let expanded = calculate_pane_scaffold_directive(&info(1000.0, 800.0, Posture()));
    assert_eq!((expanded.max_horizontal_partitions, expanded.horizontal_partition_spacer_size), (2, 24.dp()));
    let large = calculate_pane_scaffold_directive(&info(1300.0, 800.0, Posture()));
    assert_eq!((large.max_horizontal_partitions, large.default_pane_preferred_width), (3, 412.dp()));
    let medium = calculate_pane_scaffold_directive_with_two_panes_on_medium_width(&info(700.0, 1000.0, Posture()));
    assert_eq!((medium.max_horizontal_partitions, medium.max_vertical_partitions), (2, 1));
    // Hinges: separating vertical ones excluded by default; never avoided by NeverAvoid.
    let hinge = HingeInfo(Rect::new(390.0, 0.0, 410.0, 800.0), false, true, true, false);
    let tabletop = named_args!(Posture(is_tabletop = true, hinge_list = vec![hinge]));
    let folded = calculate_pane_scaffold_directive(&info(1000.0, 800.0, tabletop.clone()));
    assert_eq!((folded.excluded_bounds.clone(), folded.max_vertical_partitions), (vec![hinge.bounds], 2));
    let never = named_args!(calculate_pane_scaffold_directive(
        &info(1000.0, 800.0, tabletop),
        vertical_hinge_policy = HingePolicy::NeverAvoid
    ));
    assert!(never.excluded_bounds.is_empty());
    assert_eq!(HingePolicy::AvoidSeparating.to_string(), "HingePolicy.AvoidOccludingAndSeparating");
    let copy = named_args!(expanded.copy(max_horizontal_partitions = 1));
    assert_eq!((copy.max_horizontal_partitions, copy.horizontal_partition_spacer_size), (1, 24.dp()));
    assert_eq!(
        PaneScaffoldDirective::Default().to_string(),
        "PaneScaffoldDirective(maxHorizontalPartitions=1, horizontalPartitionSpacerSize=0.0.dp, maxVerticalPartitions=1, \
         verticalPartitionSpacerSize=0.0.dp, defaultPanePreferredWidth=360.0.dp, defaultPanePreferredHeight=420.0.dp, \
         number of excluded bounds=0)"
    );
}

fn navigators_go_back_by_their_behavior_and_come_back_saved() {
    let (nav, scope): (Slot<Navigator>, Slot<CoroutineScope>) = (slot(), slot());
    // (one composable for both compositions: the saved values are found by their place in it)
    fn saved_cities(
        registry: &SaveableStateRegistry,
        nav: &Slot<Navigator>,
        scope: &Slot<CoroutineScope>,
    ) -> KompozTestRule {
        let (n, s, r) = (nav.clone(), scope.clone(), registry.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (n, s) = (n.clone(), s.clone());
                ProvideSaveableStateRegistry(&r, move || Cities(n.clone(), s.clone()));
            }),
            IntSize::new(400, 800),
        )
    }
    let registry = SaveableStateRegistry::new(None);
    let rule = saved_cities(&registry, &nav, &scope);
    let navigator = get(&nav);
    for (pane, key) in [(ListDetailPaneScaffoldRole::Detail, 1), (ListDetailPaneScaffoldRole::Detail, 2)] {
        launch(&scope, navigator.navigate_to_with(pane, Some(key)));
        rule.wait_for_idle();
    }
    launch(&scope, navigator.navigate_to_with(ListDetailPaneScaffoldRole::Extra, Some(2)));
    rule.wait_for_idle();
    // History: List, Detail 1, Detail 2, Extra 2.
    use BackNavigationBehavior as B;
    let peek = |b| navigator.peek_previous_scaffold_value_with(b);
    assert_eq!(peek(B::PopLatest).primary, PaneAdaptedValue::Expanded);
    assert_eq!(peek(B::PopUntilCurrentDestinationChange).primary, PaneAdaptedValue::Expanded);
    assert_eq!(peek(B::PopUntilScaffoldValueChange).primary, PaneAdaptedValue::Expanded);
    assert_eq!(B::PopUntilContentChange.to_string(), "PopUntilContentChange");
    assert!(navigator.is_destination_history_aware());
    // Saved: the history comes back.
    let saved = registry.perform_save();
    drop(rule);
    let _rule = saved_cities(&SaveableStateRegistry::new(Some(&saved)), &nav, &scope);
    let back = get(&nav);
    assert!(!Rc::ptr_eq(&back, &navigator));
    let current = back.current_destination().unwrap();
    assert_eq!((current.pane, current.content_key), (ListDetailPaneScaffoldRole::Extra, Some(2)));
    launch(&scope, back.navigate_back_with(B::PopUntilCurrentDestinationChange));
    _rule.wait_for_idle();
    assert_eq!(back.current_destination().unwrap().content_key, Some(2));
    launch(&scope, back.navigate_back_with(B::PopLatest));
    _rule.wait_for_idle();
    assert_eq!(back.current_destination().unwrap().content_key, Some(1));
}

fn a_levitated_pane_floats_over_the_others_behind_its_scrim() {
    let (nav, scope): (Slot<Navigator>, Slot<CoroutineScope>) = (slot(), slot());
    let (n, s) = (nav.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let scope = remember_coroutine_scope();
            *s.borrow_mut() = Some(scope.clone());
            let (n_scrim, scrim_scope) = (n.clone(), scope.clone());
            let navigator: Slot<Navigator> = n.clone();
            let scrim: Rc<dyn Fn()> = remember(move || {
                let (n, scope) = (n_scrim.clone(), scrim_scope.clone());
                Rc::new(move || {
                    let (n, scope) = (n.clone(), scope.clone());
                    LevitatedPaneScrim(
                        modifier = Modifier.test_tag("perde"),
                        on_click = Rc::new(move || {
                            let back = get(&n).navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange);
                            named_args!(scope.launch(async move {
                                back.await;
                            }));
                        }),
                    );
                }) as Rc<dyn Fn()>
            });
            let strategies =
                named_args!(ListDetailPaneScaffoldDefaults.adapt_strategies(
                    extra_pane_adapt_strategy = AdaptStrategy::levitate(Alignment::Center, Some(scrim))
                ));
            let nv: Navigator = remember_list_detail_pane_scaffold_navigator(adapt_strategies = strategies);
            *navigator.borrow_mut() = Some(nv.clone());
            NavigableListDetailPaneScaffold(
                nv,
                list_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("liste"), content = |_: &AnimatedPaneScope| {});
                },
                detail_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("ayrıntı"), content = |_: &AnimatedPaneScope| {});
                },
                extra_pane = Some(Rc::new(|pane: &ThreePaneScaffoldPaneScope| {
                    let modifier =
                        pane.preferred_height(pane.preferred_width(Modifier.test_tag("ek"), 300.dp()), 200.dp());
                    pane.AnimatedPane(modifier = modifier, content = |_: &AnimatedPaneScope| Text("Ek"));
                }) as ThreePanePane),
            );
        }),
        IntSize::new(1000, 800),
    );
    let navigator = get(&nav);
    assert!(!rule.on_node_with_tag("ek").exists());
    launch(&scope, navigator.navigate_to_with(ListDetailPaneScaffoldRole::Extra, None));
    rule.wait_for_idle();
    // Levitated in the middle, 300 × 200, over the two expanded panes, the scrim over the whole scaffold.
    assert_eq!(bounds(&rule, "ek"), Rect::new(350.0, 300.0, 650.0, 500.0));
    assert!(rule.on_node_with_tag("liste").exists() && rule.on_node_with_tag("ayrıntı").exists());
    assert_eq!(bounds(&rule, "perde"), Rect::new(0.0, 0.0, 1000.0, 800.0));
    assert_eq!(
        navigator.scaffold_value().tertiary.to_string().split(" with").next(),
        Some("PaneAdaptedValue[Levitated")
    );
    // Clicking the scrim goes back: the pane and its scrim leave.
    rule.on_node_with_tag("perde").perform_click();
    rule.wait_for_idle();
    assert!(!rule.on_node_with_tag("ek").exists() && !rule.on_node_with_tag("perde").exists());
    assert_eq!(LevitatedPaneScrimDefaults::Color, Color::new(0.0, 0.0, 0.0, 0.32));
}

fn preferred_widths_size_the_panes() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let n = remember_list_detail_pane_scaffold_navigator::<i32>();
            NavigableListDetailPaneScaffold(
                n,
                list_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    let modifier = pane.preferred_width_proportion(Modifier.test_tag("liste"), 0.5);
                    pane.AnimatedPane(modifier = modifier, content = |_: &AnimatedPaneScope| {});
                },
                detail_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    let modifier = pane.preferred_width(Modifier.test_tag("ayrıntı"), 200.dp());
                    pane.AnimatedPane(modifier = modifier, content = |_: &AnimatedPaneScope| {});
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    // The list half of the scaffold (500), the detail 200 dp and the rest (the highest priority's): 476.
    assert!(near(bounds(&rule, "liste").width(), 500.0));
    assert!(near(bounds(&rule, "ayrıntı").left, 524.0) && near(bounds(&rule, "ayrıntı").width(), 476.0));
}
