//! The adaptive-layout samples (Compose's ThreePaneScaffoldSample.kt, adaptive-layout 1.2.0) in Rust, as written,
//! their behaviour checked: the list and detail panes side by side or one at a time, the extra pane with its
//! expansion anchors and drag handle, the extra pane levitated as a dialog over its scrim, the supporting pane, the
//! preferred sizes, the reflow strategy and the navigable scaffold's back behaviors. (Kotlin's `NavItemData`, a
//! Parcelable data class, is a Rust struct saved as its two fields.)

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{
    AnimatedContent, AnimatedContentScope, AnimatedContentTransitionScope, AnimatedVisibility, AnimatedVisibilityScope,
    expand_horizontally, fade_in, fade_out, shrink_horizontally, slide_in_horizontally, slide_out_horizontally,
};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::back::BackHandler;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::semantics::{Role, SemanticsActions};
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    list_detail_pane_scaffold_sample,
    list_detail_pane_scaffold_sample_with_extra_pane,
    list_detail_pane_scaffold_sample_with_extra_pane_levitated_as_dialog,
    supporting_pane_scaffold_sample,
    preferred_size_modifier_samples,
    pane_expansion_drag_handle_sample,
    reflow_adapt_strategy,
    navigable_list_detail_pane_scaffold_sample,
);

const HANDLE: &str = "Pane expansion drag handle";

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn has_text(rule: &KompozTestRule, text: &str) -> bool {
    named_args!(rule.on_node_with_text(text)).exists()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text)).get_unclipped_bounds_in_root()
}

fn click_text(rule: &KompozTestRule, text: &str) {
    named_args!(rule.on_node_with_text(text)).perform_click();
    rule.wait_for_idle();
}

/// The list's card of `text` (the detail's top bar shows the same text).
fn card<'a>(rule: &'a KompozTestRule, text: &str) -> kompoz_test::SemanticsNodeInteraction<'a> {
    rule.on_node_with(kompoz_test::has_text(text, false, false).and(kompoz_test::is_selectable()), false)
}

fn click_back(rule: &KompozTestRule) {
    rule.on_node_with_content_description("Back").perform_click();
    rule.wait_for_idle();
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

/// Escape pressed: the platform's back.
fn escape(rule: &KompozTestRule) -> bool {
    let handled = rule.host().key_down(Key::Escape, KeyModifiers::default());
    rule.wait_for_idle();
    handled
}

// ---- The samples, as Compose writes them ------------------------------------------------------------------------

/// Kotlin: `fun ListDetailPaneScaffoldSample()`
#[kompozable]
fn ListDetailPaneScaffoldSample() {
    let coroutine_scope = remember_coroutine_scope();
    let scaffold_navigator: Navigator = remember_list_detail_pane_scaffold_navigator();
    let items = vec!["Item 1", "Item 2", "Item 3"];
    let selected_item = scaffold_navigator.current_destination().and_then(|d| d.content_key);

    let (list_navigator, list_scope, list_items) = (scaffold_navigator.clone(), coroutine_scope.clone(), items.clone());
    let (detail_navigator, detail_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    StateListDetailPaneScaffold(
        scaffold_navigator.scaffold_directive(),
        scaffold_navigator.scaffold_state(),
        list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, items) = (list_navigator.clone(), list_scope.clone(), list_items.clone());
            pane.AnimatedPane(
                modifier = pane.preferred_width(Modifier, 200.dp()),
                content = move |_: &AnimatedPaneScope| {
                    ListPaneContent(
                        items = items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
        detail_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, items) = (detail_navigator.clone(), detail_scope.clone(), items.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    DetailPaneContent(
                        items = items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
    );
}

/// Kotlin: `fun ListDetailPaneScaffoldSampleWithExtraPane()`
#[kompozable]
fn ListDetailPaneScaffoldSampleWithExtraPane() {
    let coroutine_scope = remember_coroutine_scope();
    let scaffold_navigator: Navigator = remember_list_detail_pane_scaffold_navigator();
    let items = vec!["Item 1", "Item 2", "Item 3"];
    let extra_items = vec!["Extra 1", "Extra 2", "Extra 3"];
    let selected_item = scaffold_navigator.current_destination().and_then(|d| d.content_key);

    let (list_navigator, list_scope, list_items) = (scaffold_navigator.clone(), coroutine_scope.clone(), items.clone());
    let (detail_navigator, detail_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    let (extra_navigator, extra_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    StateListDetailPaneScaffold(
        scaffold_navigator.scaffold_directive(),
        scaffold_navigator.scaffold_state(),
        list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, items) = (list_navigator.clone(), list_scope.clone(), list_items.clone());
            pane.AnimatedPane(
                modifier = pane.preferred_width(Modifier, 200.dp()),
                content = move |_: &AnimatedPaneScope| {
                    ListPaneContent(
                        items = items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
        detail_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, items) = (detail_navigator.clone(), detail_scope.clone(), items.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    DetailPaneContent(
                        items = items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        has_extra_pane = true,
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
        extra_pane = Some(Rc::new(move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, extra_items) = (extra_navigator.clone(), extra_scope.clone(), extra_items.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    ExtraPaneContent(
                        extra_items = extra_items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        }) as ThreePanePane),
        pane_expansion_state = Some(remember_pane_expansion_state_of_key_provider(
            &scaffold_navigator.scaffold_value(),
            anchors = PaneExpansionAnchors(),
            initial_anchored_index = 1,
        )),
        pane_expansion_drag_handle = Some(Rc::new(|scope: &ThreePaneScaffoldScope, state: PaneExpansionState| {
            PaneExpansionDragHandleSample(scope.clone(), state = state);
        }) as ThreePaneDragHandle),
    );
}

/// Kotlin: `fun ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog()`
#[kompozable]
fn ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog() {
    let coroutine_scope = remember_coroutine_scope();
    let scaffold_navigator: Navigator = levitate_as_dialog_sample();
    let items = vec!["Item 1", "Item 2", "Item 3"];
    let extra_items = vec!["Extra 1", "Extra 2", "Extra 3"];
    let selected_item = scaffold_navigator.current_destination().and_then(|d| d.content_key);

    let (list_navigator, list_scope, list_items) = (scaffold_navigator.clone(), coroutine_scope.clone(), items.clone());
    let (detail_navigator, detail_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    let (extra_navigator, extra_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    StateListDetailPaneScaffold(
        scaffold_navigator.scaffold_directive(),
        scaffold_navigator.scaffold_state(),
        list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, items) = (list_navigator.clone(), list_scope.clone(), list_items.clone());
            pane.AnimatedPane(
                modifier = pane.preferred_width(Modifier, 200.dp()),
                content = move |_: &AnimatedPaneScope| {
                    ListPaneContent(
                        items = items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
        detail_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, items) = (detail_navigator.clone(), detail_scope.clone(), items.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    DetailPaneContent(
                        items = items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        has_extra_pane = true,
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
        extra_pane = Some(Rc::new(move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, extra_items) = (extra_navigator.clone(), extra_scope.clone(), extra_items.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    ExtraPaneContent(
                        extra_items = extra_items.clone(),
                        selected_item = selected_item,
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        }) as ThreePanePane),
        pane_expansion_state = Some(remember_pane_expansion_state_of_key_provider(
            &scaffold_navigator.scaffold_value(),
            anchors = PaneExpansionAnchors(),
            initial_anchored_index = 1,
        )),
        pane_expansion_drag_handle = Some(Rc::new(|scope: &ThreePaneScaffoldScope, state: PaneExpansionState| {
            PaneExpansionDragHandleSample(scope.clone(), state = state);
        }) as ThreePaneDragHandle),
    );
}

/// Kotlin: `fun SupportingPaneScaffoldSample()`
#[kompozable]
fn SupportingPaneScaffoldSample() {
    let coroutine_scope = remember_coroutine_scope();
    let scaffold_navigator: Navigator = remember_supporting_pane_scaffold_navigator();
    let extra_items = vec!["Extra content"];
    let selected_item = NavItemData(0, true);

    let (main_navigator, main_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    let (extra_navigator, extra_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    StateSupportingPaneScaffold(
        scaffold_navigator.scaffold_directive(),
        scaffold_navigator.scaffold_state(),
        main_pane = move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope) = (main_navigator.clone(), main_scope.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    MainPaneContent(
                        scaffold_navigator = n.clone(),
                        has_extra_pane = true,
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        },
        supporting_pane = |pane: &ThreePaneScaffoldPaneScope| {
            pane.AnimatedPane(
                modifier = pane.preferred_width(Modifier, 200.dp()),
                content = |_: &AnimatedPaneScope| SupportingPaneContent(),
            );
        },
        extra_pane = Some(Rc::new(move |pane: &ThreePaneScaffoldPaneScope| {
            let (n, scope, extra_items) = (extra_navigator.clone(), extra_scope.clone(), extra_items.clone());
            pane.AnimatedPane(
                content = move |_: &AnimatedPaneScope| {
                    ExtraPaneContent(
                        extra_items = extra_items.clone(),
                        selected_item = Some(selected_item),
                        scaffold_navigator = n.clone(),
                        coroutine_scope = scope.clone(),
                    );
                },
            );
        }) as ThreePanePane),
        pane_expansion_state = Some(remember_pane_expansion_state_of_key_provider(
            &scaffold_navigator.scaffold_value(),
            anchors = PaneExpansionAnchors(),
        )),
        pane_expansion_drag_handle = Some(Rc::new(|scope: &ThreePaneScaffoldScope, state: PaneExpansionState| {
            PaneExpansionDragHandleSample(scope.clone(), state = state);
        }) as ThreePaneDragHandle),
    );
}

/// Kotlin: `fun ThreePaneScaffoldPaneScope.PreferredSizeModifierInDpSample(modifier: Modifier = Modifier, preferredWidth: Dp, preferredHeight: Dp, content: @Composable () -> Unit)`
/// (Kotlin's receiver a handle passed by value: a test's composable taking defaults is a builder, its arguments
/// owned.)
#[kompozable]
fn PreferredSizeModifierInDpSample(
    pane: ThreePaneScaffoldPaneScope,
    #[default(Modifier)] modifier: Modifier,
    preferred_width: Dp,
    preferred_height: Dp,
    content: impl Fn() + 'static,
) {
    pane.AnimatedPane(
        modifier =
            pane.preferred_height(pane.preferred_width(Modifier, preferred_width), preferred_height).then(modifier),
        content = move |_: &AnimatedPaneScope| content(),
    );
}

/// Kotlin: `fun ThreePaneScaffoldPaneScope.PreferredSizeModifierInProportionSample(modifier: Modifier = Modifier, preferredWidthInProportion: Float, preferredHeightInProportion: Float, content: @Composable () -> Unit)`
/// (Kotlin's receiver a handle passed by value: a test's composable taking defaults is a builder, its arguments
/// owned.)
#[kompozable]
fn PreferredSizeModifierInProportionSample(
    pane: ThreePaneScaffoldPaneScope,
    #[default(Modifier)] modifier: Modifier,
    preferred_width_in_proportion: f32,
    preferred_height_in_proportion: f32,
    content: impl Fn() + 'static,
) {
    pane.AnimatedPane(
        modifier = pane
            .preferred_height_proportion(
                pane.preferred_width_proportion(Modifier, preferred_width_in_proportion),
                preferred_height_in_proportion,
            )
            .then(modifier),
        content = move |_: &AnimatedPaneScope| content(),
    );
}

/// Kotlin: `fun ThreePaneScaffoldScope.PaneExpansionDragHandleSample(state: PaneExpansionState = rememberPaneExpansionState())`
/// (Kotlin's receiver a handle passed by value: a test's composable taking defaults is a builder, its arguments
/// owned.)
#[kompozable]
fn PaneExpansionDragHandleSample(
    scope: ThreePaneScaffoldScope,
    #[default(remember_pane_expansion_state())] state: PaneExpansionState,
) {
    let interaction_source = remember(MutableInteractionSource::new);
    VerticalDragHandle(
        modifier = scope.pane_expansion_draggable(
            Modifier,
            state,
            LocalMinimumInteractiveComponentSize.current(),
            interaction_source.clone(),
            None,
        ),
        interaction_source = Some(interaction_source),
    );
}

/// Kotlin: `fun <T> reflowAdaptStrategySample(): ThreePaneScaffoldNavigator<T>`
#[kompozable]
fn reflow_adapt_strategy_sample<T: NavigatorContentKey>() -> Rc<dyn ThreePaneScaffoldNavigator<T>> {
    remember_list_detail_pane_scaffold_navigator(
        adapt_strategies =
            named_args!(ListDetailPaneScaffoldDefaults.adapt_strategies(
                extra_pane_adapt_strategy = AdaptStrategy::reflow(ListDetailPaneScaffoldRole::Detail)
            )),
    )
}

/// Kotlin: `fun <T> levitateAsDialogSample(): ThreePaneScaffoldNavigator<T>`
#[kompozable]
fn levitate_as_dialog_sample<T: NavigatorContentKey>() -> Rc<dyn ThreePaneScaffoldNavigator<T>> {
    let coroutine_scope = remember_coroutine_scope();
    let scaffold_directive = calculate_pane_scaffold_directive(&current_window_adaptive_info());
    // (Kotlin's `var navigator` captured by `onClick` before it is set: a cell set after the navigator is
    // remembered.)
    let navigator: Rc<RefCell<Option<Rc<dyn ThreePaneScaffoldNavigator<T>>>>> =
        remember(|| Rc::new(RefCell::new(None)));
    let on_click: Rc<dyn Fn()> = {
        let navigator = navigator.clone();
        Rc::new(move || {
            let navigator = navigator.borrow().clone();
            named_args!(coroutine_scope.launch(async move {
                if let Some(navigator) = navigator {
                    navigator.navigate_back_with(BackNavigationBehavior::PopUntilScaffoldValueChange).await;
                }
            }));
        })
    };
    // (Compose's compiler remembers the scrim's lambda, its captures stable: remembered here, the levitated value
    // stays equal across compositions.)
    let scrim: Rc<dyn Fn()> = remember(move || {
        Rc::new(move || {
            let (semantics_click, scrim_click) = (on_click.clone(), on_click.clone());
            LevitatedPaneScrim(
                Modifier.semantics(move |c| {
                    c.set_content_description("Scrim".to_string());
                    let semantics_click = semantics_click.clone();
                    named_args!(c.on_click(
                        label = Some("Dismiss the extra pane".to_string()),
                        action = move || {
                            semantics_click();
                            true
                        }
                    ));
                }),
                on_click = scrim_click,
            );
        }) as Rc<dyn Fn()>
    });
    let n = remember_list_detail_pane_scaffold_navigator(
        scaffold_directive = scaffold_directive.clone(),
        adapt_strategies = named_args!(SupportingPaneScaffoldDefaults.adapt_strategies(
            extra_pane_adapt_strategy =
                AdaptStrategy::levitate(Alignment::Center, Some(scrim)).only_if_single_pane(&scaffold_directive)
        )),
    );
    *navigator.borrow_mut() = Some(n.clone());
    n
}

/// Kotlin: `fun NavigableListDetailPaneScaffoldSample()`
///
/// (androidx.navigation's NavHost and its controller are KompozR's stage 13: the two routes here are a saveable
/// route state shown by AnimatedContent with the NavHost's slides, each route's state kept by a saveable state
/// holder as the back stack's entries keep theirs, and the platform's back from the list-detail route pops to the
/// welcome one as the NavHost's back handler does, registered before the scaffold's so the scaffold's goes first.)
#[kompozable]
fn NavigableListDetailPaneScaffoldSample() {
    let welcome_route = "welcome";
    let list_detail_route = "listdetail";

    let coroutine_scope = remember_coroutine_scope();

    // `navController` handles navigation outside the ListDetailPaneScaffold, and `scaffoldNavigator` handles
    // navigation within it. The "content" of the scaffold uses a custom type which tracks the index of the selected
    // item, which is passed as a type argument to `rememberListDetailPaneScaffoldNavigator`.
    let nav_controller = remember_saveable(|| mutable_state_of(welcome_route.to_string()));
    let scaffold_navigator: Navigator = remember_list_detail_pane_scaffold_navigator();

    // Back behavior can be customized based on the needs of the app.
    let back_behavior_index = remember_saveable(|| mutable_state_of(0usize));
    let back_behaviors = [
        BackNavigationBehavior::PopUntilScaffoldValueChange,
        BackNavigationBehavior::PopUntilCurrentDestinationChange,
        BackNavigationBehavior::PopUntilContentChange,
        BackNavigationBehavior::PopLatest,
    ];
    let back_behavior = back_behaviors[back_behavior_index.get()];

    let items = vec!["Item 1", "Item 2", "Item 3"];
    let extra_items = vec!["Extra 1", "Extra 2", "Extra 3"];

    // (the NavHost: its back handler, its entries' saved states, its transitions)
    let pop = nav_controller.clone();
    BackHandler(enabled = nav_controller.get() != welcome_route, on_back = move || pop.set(welcome_route.to_string()));
    let saveable_state_holder = remember_saveable_state_holder();
    AnimatedContent(
        nav_controller.get(),
        transition_spec = move |scope: &AnimatedContentTransitionScope<String>| {
            if scope.target_state() == list_detail_route {
                slide_in_horizontally(initial_offset_x = |it| it)
                    .together_with(slide_out_horizontally(target_offset_x = |it| -it))
            } else {
                slide_in_horizontally(initial_offset_x = |it| -it)
                    .together_with(slide_out_horizontally(target_offset_x = |it| it))
            }
        },
        content = move |_: &AnimatedContentScope, route: String| {
            let (index, nav_controller) = (back_behavior_index.clone(), nav_controller.clone());
            let (scaffold_navigator, coroutine_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
            let (items, extra_items) = (items.clone(), extra_items.clone());
            saveable_state_holder.SaveableStateProvider(&route, || {
                if route == welcome_route {
                    Scaffold(
                        modifier = Modifier.fill_max_size(),
                        content = move |padding_values: PaddingValues| {
                            let (index, nav_controller) = (index.clone(), nav_controller.clone());
                            Column(
                                modifier = Modifier
                                    .vertical_scroll(&remember_scroll_state())
                                    .padding_values(padding_values)
                                    .padding(24.dp())
                                    .fill_max_size(),
                                vertical_arrangement = Arrangement::spaced_by(16.dp()),
                                content = move || {
                                    Text(
                                        text = "How should the scaffold handle back navigation?",
                                        style = MaterialTheme::typography().headline_medium,
                                    );
                                    let i = index.clone();
                                    RadioButtonRow(
                                        selected = index.get() == 0,
                                        on_click = move || i.set(0),
                                        text = "PopUntilScaffoldValueChange - Back navigation forces a change in "
                                            .to_string()
                                            + "which pane(s) is/are shown.",
                                    );
                                    let i = index.clone();
                                    RadioButtonRow(
                                        selected = index.get() == 1,
                                        on_click = move || i.set(1),
                                        text = "PopUntilCurrentDestinationChange - Back navigation forces a "
                                            .to_string()
                                            + "change in which pane is currently considered \"active\".",
                                    );
                                    let i = index.clone();
                                    RadioButtonRow(
                                        selected = index.get() == 2,
                                        on_click = move || i.set(2),
                                        text = "PopUntilContentChange - Back navigation forces a change in the "
                                            .to_string()
                                            + "content of any pane or which pane(s) is/are shown.\nNote: this "
                                            + "may result in unintuitive behavior if the device size changes "
                                            + "in the middle of the navigation.",
                                    );
                                    let i = index.clone();
                                    RadioButtonRow(
                                        selected = index.get() == 3,
                                        on_click = move || i.set(3),
                                        text = "PopLatest - No special back handling.\nNote: this may result in "
                                            .to_string()
                                            + "unintuitive behavior if the device size changes in the middle "
                                            + "of the navigation.",
                                    );
                                    let nav_controller = nav_controller.clone();
                                    Button(
                                        on_click = move || nav_controller.set(list_detail_route.to_string()),
                                        content = || Text("Next"),
                                    );
                                },
                            );
                        },
                    );
                } else {
                    let selected_item = scaffold_navigator.current_destination().and_then(|d| d.content_key);
                    let (list_navigator, list_scope, list_items) =
                        (scaffold_navigator.clone(), coroutine_scope.clone(), items.clone());
                    let (detail_navigator, detail_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
                    let (extra_navigator, extra_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
                    NavigableListDetailPaneScaffold(
                        scaffold_navigator.clone(),
                        default_back_behavior = back_behavior,
                        list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
                            let (n, scope, items) = (list_navigator.clone(), list_scope.clone(), list_items.clone());
                            pane.AnimatedPane(
                                modifier = pane.preferred_width(Modifier, 200.dp()),
                                content = move |_: &AnimatedPaneScope| {
                                    ListPaneContent(
                                        items = items.clone(),
                                        selected_item = selected_item,
                                        scaffold_navigator = n.clone(),
                                        coroutine_scope = scope.clone(),
                                    );
                                },
                            );
                        },
                        detail_pane = move |pane: &ThreePaneScaffoldPaneScope| {
                            let (n, scope, items) = (detail_navigator.clone(), detail_scope.clone(), items.clone());
                            pane.AnimatedPane(
                                content = move |_: &AnimatedPaneScope| {
                                    DetailPaneContent(
                                        items = items.clone(),
                                        selected_item = selected_item,
                                        scaffold_navigator = n.clone(),
                                        has_extra_pane = true,
                                        back_behavior = back_behavior,
                                        coroutine_scope = scope.clone(),
                                    );
                                },
                            );
                        },
                        extra_pane = Some(Rc::new(move |pane: &ThreePaneScaffoldPaneScope| {
                            let (n, scope, extra_items) =
                                (extra_navigator.clone(), extra_scope.clone(), extra_items.clone());
                            pane.AnimatedPane(
                                content = move |_: &AnimatedPaneScope| {
                                    ExtraPaneContent(
                                        extra_items = extra_items.clone(),
                                        selected_item = selected_item,
                                        scaffold_navigator = n.clone(),
                                        back_behavior = back_behavior,
                                        coroutine_scope = scope.clone(),
                                    );
                                },
                            );
                        }) as ThreePanePane),
                    );
                }
            });
        },
    );
}

// ---- What the samples share, private to Compose's file ----------------------------------------------------------

type Navigator = Rc<dyn ThreePaneScaffoldNavigator<NavItemData>>;

/// Kotlin: `private fun ThreePaneScaffoldNavigator<*>.isExpanded(role: ThreePaneScaffoldRole)`
fn is_expanded(navigator: &Navigator, role: ThreePaneScaffoldRole) -> bool {
    *navigator.scaffold_value().get_ref(role) == PaneAdaptedValue::Expanded
}

/// Kotlin: `private data class NavItemData(val index: Int, val showExtra: Boolean = false) : Parcelable` (its
/// parcel's two fields its saved value).
#[derive(Clone, Copy, PartialEq, Debug)]
struct NavItemData {
    index: i32,
    show_extra: bool,
}

/// Kotlin: `NavItemData(index: Int, showExtra: Boolean = false)`
fn NavItemData(index: i32, show_extra: bool) -> NavItemData {
    NavItemData { index, show_extra }
}

impl Saveable for NavItemData {
    fn save(&self) -> SavedValue {
        SavedValue::List(std::sync::Arc::new(vec![self.index.save(), self.show_extra.save()]))
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let SavedValue::List(fields) = saved else { return None };
        Some(NavItemData { index: i32::restore(fields.first()?)?, show_extra: bool::restore(fields.get(1)?)? })
    }
}

/// Kotlin: `private fun ListCard(title: String, isSelected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
fn ListCard(title: String, is_selected: bool, on_click: impl Fn() + 'static, #[default(Modifier)] modifier: Modifier) {
    let on_click: Rc<dyn Fn()> = Rc::new(on_click);
    let select = on_click.clone();
    ClickableOutlinedCard(
        move || on_click(),
        colors = named_args!(CardDefaults.outlined_card_colors(Some(if is_selected {
            MaterialTheme::color_scheme().primary_container
        } else {
            MaterialTheme::color_scheme().surface
        }))),
        modifier = modifier.height(80.dp()).fill_max_width().selectable(is_selected, on_click = move || select()),
        content = move || {
            let title = title.clone();
            Row(
                modifier = Modifier.padding(12.dp()),
                horizontal_arrangement = Arrangement::spaced_by(12.dp()),
                vertical_alignment = Alignment::CenterVertically,
                content = move || {
                    VectorIcon(
                        if is_selected { Icons::Filled::CheckCircle() } else { Icons::Outlined::CheckCircle() },
                        None,
                        modifier = Modifier.fill_max_height(),
                    );
                    Text(
                        title.clone(),
                        modifier = Modifier.fill_max_size().wrap_content_height(),
                        style = MaterialTheme::typography().headline_small,
                    );
                },
            );
        },
    );
}

/// Kotlin: `private fun ListPaneContent(items: List<String>, selectedItem: NavItemData?, scaffoldNavigator: ThreePaneScaffoldNavigator<NavItemData>, modifier: Modifier = Modifier, coroutineScope: CoroutineScope)`
#[kompozable]
fn ListPaneContent(
    items: Vec<&'static str>,
    selected_item: Option<NavItemData>,
    scaffold_navigator: Navigator,
    #[default(Modifier)] modifier: Modifier,
    coroutine_scope: CoroutineScope,
) {
    Column(
        modifier = modifier.selectable_group(),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = move || {
            for (index, item) in items.iter().enumerate() {
                let index = index as i32;
                let (n, scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
                ListCard(
                    item.to_string(),
                    Some(index) == selected_item.map(|s| s.index)
                        && is_expanded(&scaffold_navigator, ListDetailPaneScaffoldRole::Detail),
                    on_click = move || {
                        named_args!(scope.launch(
                            n.navigate_to(ListDetailPaneScaffoldRole::Detail, Some(NavItemData(index, false)))
                        ));
                    },
                );
            }
        },
    );
}

/// Kotlin: `private fun DetailPaneContent(items: List<String>, selectedItem: NavItemData?, scaffoldNavigator: ThreePaneScaffoldNavigator<NavItemData>, modifier: Modifier = Modifier, hasExtraPane: Boolean = false, backBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange, coroutineScope: CoroutineScope)`
#[kompozable]
fn DetailPaneContent(
    items: Vec<&'static str>,
    selected_item: Option<NavItemData>,
    scaffold_navigator: Navigator,
    #[default(Modifier)] modifier: Modifier,
    #[default(false)] has_extra_pane: bool,
    #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)] back_behavior: BackNavigationBehavior,
    coroutine_scope: CoroutineScope,
) {
    let (title, description) = match selected_item {
        None => ("No item selected".to_string(), "Select an item from the list.".to_string()),
        Some(selected_item) => {
            let item = items[selected_item.index as usize];
            (item.to_string(), format!("This is a description about {item}."))
        }
    };

    let (back_navigator, back_scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
    BasicScreen(
        modifier = modifier,
        title = title,
        container_color = MaterialTheme::color_scheme().surface_container,
        back_button = move || {
            let (n, scope) = (back_navigator.clone(), back_scope.clone());
            BackButton(
                !is_expanded(&back_navigator, ListDetailPaneScaffoldRole::List),
                on_click = move || {
                    let back = n.navigate_back_with(back_behavior);
                    named_args!(scope.launch(async move {
                        back.await;
                    }));
                },
            );
        },
        content = move || {
            Text(description.clone());
            if let Some(selected_item) = selected_item
                && has_extra_pane
            {
                let (n, scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
                Button(
                    on_click = move || {
                        named_args!(scope.launch(n.navigate_to(
                            ListDetailPaneScaffoldRole::Extra,
                            Some(NavItemData(selected_item.index, true))
                        )));
                    },
                    content = || Text("Show extra"),
                );
            }
        },
    );
}

/// Kotlin: `private fun MainPaneContent(scaffoldNavigator: ThreePaneScaffoldNavigator<NavItemData>, modifier: Modifier = Modifier, hasExtraPane: Boolean = false, coroutineScope: CoroutineScope)`
#[kompozable]
fn MainPaneContent(
    scaffold_navigator: Navigator,
    #[default(Modifier)] modifier: Modifier,
    #[default(false)] has_extra_pane: bool,
    coroutine_scope: CoroutineScope,
) {
    let title = "My content".to_string();
    let description = "lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum".to_string()
        + " lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum ";

    BasicScreen(
        modifier = modifier,
        title = title,
        container_color = MaterialTheme::color_scheme().surface_container,
        back_button = || {},
        content = move || {
            Text(description.clone());
            if has_extra_pane {
                let (n, scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
                Button(
                    on_click = move || {
                        named_args!(
                            scope.launch(n.navigate_to(SupportingPaneScaffoldRole::Extra, Some(NavItemData(0, true))))
                        );
                    },
                    content = || Text("Show extra"),
                );
            }
        },
    );
}

/// Kotlin: `private fun SupportingPaneContent(modifier: Modifier = Modifier)`
#[kompozable]
fn SupportingPaneContent(#[default(Modifier)] modifier: Modifier) {
    let items = ["Item 1", "Item 2", "Item 3"];
    let selected_index = remember_saveable(|| mutable_int_state_of(-1));
    Column(
        modifier = modifier.selectable_group(),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = move || {
            for (index, item) in items.iter().enumerate() {
                let (index, selected) = (index as i32, selected_index.clone());
                ListCard(item.to_string(), index == selected_index.get(), on_click = move || selected.set(index));
            }
        },
    );
}

/// Kotlin: `private fun ExtraPaneContent(extraItems: List<String>, selectedItem: NavItemData?, scaffoldNavigator: ThreePaneScaffoldNavigator<NavItemData>, modifier: Modifier = Modifier, backBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange, coroutineScope: CoroutineScope)`
#[kompozable]
fn ExtraPaneContent(
    extra_items: Vec<&'static str>,
    selected_item: Option<NavItemData>,
    scaffold_navigator: Navigator,
    #[default(Modifier)] modifier: Modifier,
    #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)] back_behavior: BackNavigationBehavior,
    coroutine_scope: CoroutineScope,
) {
    let item = match selected_item {
        Some(selected_item) if selected_item.show_extra => extra_items[selected_item.index as usize],
        _ => "",
    };

    BasicScreen(
        modifier = modifier,
        title = item.to_string(),
        container_color = MaterialTheme::color_scheme().surface_container_highest,
        back_button = move || {
            let (n, scope) = (scaffold_navigator.clone(), coroutine_scope.clone());
            BackButton(
                scaffold_navigator.can_navigate_back_with(back_behavior),
                on_click = move || {
                    let back = n.navigate_back_with(back_behavior);
                    named_args!(scope.launch(async move {
                        back.await;
                    }));
                },
            );
        },
        content = move || Text(format!("This is extra content about {item}.")),
    );
}

/// Kotlin: `private fun BasicScreen(title: String, backButton: @Composable () -> Unit, containerColor: Color, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
fn BasicScreen(
    title: String,
    back_button: impl Fn() + 'static,
    container_color: Color,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    let back_button: Rc<dyn Fn()> = Rc::new(back_button);
    let content: Rc<dyn Fn()> = Rc::new(content);
    Scaffold(
        modifier = modifier,
        top_bar = move || {
            let (title, back_button) = (title.clone(), back_button.clone());
            TopAppBar(move || Text(title.clone()), navigation_icon = move || back_button());
        },
        content = move |padding_values: PaddingValues| {
            let content = content.clone();
            Card(
                colors = named_args!(CardDefaults.card_colors(Some(container_color))),
                modifier = Modifier.padding_values(padding_values).fill_max_size(),
                content = move || {
                    let content = content.clone();
                    Column(
                        modifier = Modifier.padding(16.dp()),
                        vertical_arrangement = Arrangement::spaced_by(12.dp()),
                        content = move || content(),
                    );
                },
            );
        },
    );
}

/// Kotlin: `private fun BackButton(visible: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
fn BackButton(visible: bool, on_click: impl Fn() + 'static, #[default(Modifier)] modifier: Modifier) {
    let on_click: Rc<dyn Fn()> = Rc::new(on_click);
    AnimatedVisibility(
        visible,
        modifier = modifier,
        enter = fade_in() + expand_horizontally(),
        exit = shrink_horizontally() + fade_out(),
        content = move |_: &AnimatedVisibilityScope| {
            let on_click = on_click.clone();
            IconButton(
                move || on_click(),
                content = || VectorIcon(Icons::AutoMirrored::Filled::ArrowBack(), Some("Back".to_string())),
            );
        },
    );
}

/// Kotlin: `private fun RadioButtonRow(selected: Boolean, onClick: () -> Unit, text: String, modifier: Modifier = Modifier)`
#[kompozable]
fn RadioButtonRow(
    selected: bool,
    on_click: impl Fn() + 'static,
    text: String,
    #[default(Modifier)] modifier: Modifier,
) {
    let on_click: Rc<dyn Fn()> = Rc::new(on_click);
    let row_click = on_click.clone();
    Row(
        modifier = modifier.selectable(selected, on_click = move || row_click(), role = Some(Role::RadioButton)),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            RadioButton(selected, Some(on_click.clone()));
            Text(text.clone());
        },
    );
}

/// Kotlin: `private val PaneExpansionAnchors`
fn PaneExpansionAnchors() -> Vec<PaneExpansionAnchor> {
    vec![
        PaneExpansionAnchor::Proportion(0.0),
        PaneExpansionAnchor::Offset(PaneExpansionAnchorOffset::from_start(240.dp())),
        PaneExpansionAnchor::Proportion(0.5),
        PaneExpansionAnchor::Offset(PaneExpansionAnchorOffset::from_end(240.dp())),
        PaneExpansionAnchor::Proportion(1.0),
    ]
}

// ---- Their behaviour ----------------------------------------------------------------------------------------------

fn list_detail_pane_scaffold_sample() {
    // Expanded: the list (200 dp preferred) and the detail side by side; a card picks the detail, selected while
    // the detail is shown; no back button with the list shown.
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSample, IntSize::new(1000, 800));
    assert!(has_text(&rule, "Item 1") && has_text(&rule, "No item selected"));
    assert!(has_text(&rule, "Select an item from the list."));
    assert!(text_bounds(&rule, "Item 1").right <= 200.0);
    click_text(&rule, "Item 2");
    assert!(has_text(&rule, "This is a description about Item 2.") && has_text(&rule, "Item 1"));
    card(&rule, "Item 2").assert(kompoz_test::is_selected());
    card(&rule, "Item 1").assert(kompoz_test::is_not_selected());
    rule.on_node_with_content_description("Back").assert_does_not_exist();
    assert!(!has_text(&rule, "Show extra"), "no extra pane in this sample");
    drop(rule);

    // Compact: one pane at a time; the detail with its back button to the list.
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSample, IntSize::new(400, 800));
    assert!(has_text(&rule, "Item 3") && !has_text(&rule, "No item selected"));
    click_text(&rule, "Item 3");
    assert!(has_text(&rule, "This is a description about Item 3.") && !has_text(&rule, "Item 1"));
    click_back(&rule);
    assert!(has_text(&rule, "Item 1") && !has_text(&rule, "This is a description about Item 3."));
    // (the list's card not selected: the detail is no longer shown)
    card(&rule, "Item 3").assert(kompoz_test::is_not_selected());
}

fn list_detail_pane_scaffold_sample_with_extra_pane() {
    // Expanded: the list and the detail with the drag handle between them, the split at the second anchor (240 dp
    // from the start: the list 240 − 12, half the spacer, wide — its 200 dp preferred width overridden).
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSampleWithExtraPane, IntSize::new(1000, 800));
    rule.on_node_with_content_description(HANDLE).assert_exists();
    assert!(near(text_bounds(&rule, "Item 1").right, 228.0), "{:?}", text_bounds(&rule, "Item 1"));
    click_text(&rule, "Item 1");
    // The extra pane: in place of the list (hidden by the default strategies); the detail's back button shown now
    // (the list hidden) and the extra pane's (back possible), the extra's back to the list.
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra 1."));
    assert!(has_text(&rule, "This is a description about Item 1.") && !has_text(&rule, "Item 2"));
    assert!(has_text(&rule, "Extra 1"), "the title");
    let backs = rule.on_all_nodes_with_content_description("Back");
    assert!(backs.get(0).get_unclipped_bounds_in_root().left < 500.0);
    assert!(backs.get(1).get_unclipped_bounds_in_root().left > 500.0);
    backs.get(1).perform_click();
    rule.wait_for_idle();
    assert!(has_text(&rule, "Item 2") && !has_text(&rule, "This is extra content about Extra 1."));
    drop(rule);

    // Compact: the handle not shown (one pane); the list, the detail, the extra pane, back by back.
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSampleWithExtraPane, IntSize::new(400, 800));
    // (Compose composes the handle always and places it only between two expanded panes: there, not displayed)
    rule.on_node_with_content_description(HANDLE).assert_is_not_displayed();
    click_text(&rule, "Item 2");
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra 2.") && !has_text(&rule, "Show extra"));
    click_back(&rule);
    assert!(has_text(&rule, "This is a description about Item 2."));
    click_back(&rule);
    assert!(has_text(&rule, "Item 1"));
}

fn list_detail_pane_scaffold_sample_with_extra_pane_levitated_as_dialog() {
    // Compact (a single pane): the extra pane levitates in the middle over the detail, behind its scrim; the
    // scrim's click goes back.
    let rule = KompozTestRule::with_viewport(
        ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog,
        IntSize::new(400, 800),
    );
    click_text(&rule, "Item 2");
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra 2."));
    // (the detail under it, its semantics cleared: no pane but the levitated one takes interaction behind a scrim)
    assert!(!has_text(&rule, "This is a description about Item 2."));
    let scrim = rule.on_node_with_content_description("Scrim");
    assert_eq!(scrim.get_unclipped_bounds_in_root(), Rect::new(0.0, 0.0, 400.0, 800.0));
    let action = scrim.fetch_semantics_node().config().get(SemanticsActions::OnClick).expect("the scrim's click");
    assert_eq!(action.label.as_deref(), Some("Dismiss the extra pane"));
    // A tap on the scrim beside the pane goes back.
    tap(&rule, Offset::new(10.0, 10.0));
    assert!(!has_text(&rule, "This is extra content about Extra 2."));
    rule.on_node_with_content_description("Scrim").assert_does_not_exist();
    assert!(has_text(&rule, "This is a description about Item 2."));
    // Again, dismissed by the scrim's semantics' click.
    click_text(&rule, "Show extra");
    rule.on_node_with_content_description("Scrim").perform_semantics_action(SemanticsActions::OnClick);
    rule.wait_for_idle();
    assert!(!has_text(&rule, "This is extra content about Extra 2."));
    assert!(has_text(&rule, "This is a description about Item 2."));
    drop(rule);

    // Expanded (two panes): not levitated (only if single pane): the extra pane beside the detail, no scrim.
    let rule = KompozTestRule::with_viewport(
        ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog,
        IntSize::new(1000, 800),
    );
    click_text(&rule, "Item 2");
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra 2."));
    rule.on_node_with_content_description("Scrim").assert_does_not_exist();
    assert!(text_bounds(&rule, "This is extra content about Extra 2.").left > 500.0);
}

fn supporting_pane_scaffold_sample() {
    // Expanded: the main pane and the supporting pane at the end, its cards selectable (one at a time), the handle
    // between them.
    let rule = KompozTestRule::with_viewport(SupportingPaneScaffoldSample, IntSize::new(1000, 800));
    assert!(has_text(&rule, "My content") && has_text(&rule, "Item 1"));
    assert!(text_bounds(&rule, "Item 1").left > text_bounds(&rule, "My content").right);
    rule.on_node_with_content_description(HANDLE).assert_exists();
    click_text(&rule, "Item 2");
    card(&rule, "Item 2").assert(kompoz_test::is_selected());
    click_text(&rule, "Item 3");
    card(&rule, "Item 2").assert(kompoz_test::is_not_selected());
    // The extra pane: in place of the supporting pane, its back button back to it.
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra content.") && !has_text(&rule, "Item 1"));
    click_back(&rule);
    assert!(has_text(&rule, "Item 1"));
    card(&rule, "Item 3").assert(kompoz_test::is_selected());
    drop(rule);

    // Compact: the main pane alone (the supporting pane reflows only with two vertical partitions).
    let rule = KompozTestRule::with_viewport(SupportingPaneScaffoldSample, IntSize::new(400, 800));
    assert!(has_text(&rule, "My content") && !has_text(&rule, "Item 1"));
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra content.") && !has_text(&rule, "My content"));
    click_back(&rule);
    assert!(has_text(&rule, "My content"));
}

fn preferred_size_modifier_samples() {
    // The list pane by the samples: 300 dp wide, then a quarter of the scaffold; the detail the rest after the
    // 24 dp spacer.
    let proportion = mutable_state_of(false);
    let p = proportion.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let n = remember_list_detail_pane_scaffold_navigator::<i32>();
            let p = p.clone();
            NavigableListDetailPaneScaffold(
                n,
                list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
                    if p.get() {
                        PreferredSizeModifierInProportionSample(
                            pane.clone(),
                            modifier = Modifier.test_tag("liste"),
                            preferred_width_in_proportion = 0.25,
                            preferred_height_in_proportion = 0.5,
                            content = || {},
                        );
                    } else {
                        PreferredSizeModifierInDpSample(
                            pane.clone(),
                            modifier = Modifier.test_tag("liste"),
                            preferred_width = 300.dp(),
                            preferred_height = 300.dp(),
                            content = || {},
                        );
                    }
                },
                detail_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("ayrıntı"), content = |_: &AnimatedPaneScope| {});
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    // (the preferred heights size a pane only when it is reflowed or levitated: the list is the scaffold's height)
    assert_eq!(bounds(&rule, "liste"), Rect::new(0.0, 0.0, 300.0, 800.0));
    assert_eq!(bounds(&rule, "ayrıntı"), Rect::new(324.0, 0.0, 1000.0, 800.0));
    proportion.set(true);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "liste"), Rect::new(0.0, 0.0, 250.0, 800.0));
    assert_eq!(bounds(&rule, "ayrıntı"), Rect::new(274.0, 0.0, 1000.0, 800.0));
}

fn pane_expansion_drag_handle_sample() {
    // The sample's handle in a scaffold with its own state (the default): a 48 dp touch target between the panes,
    // its semantics the expansion's.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let n = remember_list_detail_pane_scaffold_navigator::<i32>();
            NavigableListDetailPaneScaffold(
                n,
                list_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("liste"), content = |_: &AnimatedPaneScope| {});
                },
                detail_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("ayrıntı"), content = |_: &AnimatedPaneScope| {});
                },
                pane_expansion_drag_handle =
                    Some(Rc::new(kompozable_lambda!(|scope: &ThreePaneScaffoldScope, _state: PaneExpansionState| {
                        PaneExpansionDragHandleSample(scope.clone());
                    })) as ThreePaneDragHandle),
            );
        }),
        IntSize::new(1000, 800),
    );
    let handle = rule.on_node_with_content_description(HANDLE);
    handle.assert_exists();
    let b = handle.get_unclipped_bounds_in_root();
    assert!(near(b.width(), 48.0) && near(b.height(), 48.0), "{b:?}");
    // (between the panes: the list 360 dp, the spacer 24)
    assert!(near(b.center().x, 372.0), "{b:?}");
    assert!(near(bounds(&rule, "liste").right, 360.0));
}

fn reflow_adapt_strategy() {
    // A tall medium window (one horizontal partition, two vertical): the extra pane reflows under the detail
    // pane, both shown, the detail above.
    let navigator: Rc<RefCell<Option<Rc<dyn ThreePaneScaffoldNavigator<i32>>>>> = Rc::default();
    let scope: Rc<RefCell<Option<CoroutineScope>>> = Rc::default();
    let (n, s) = (navigator.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *s.borrow_mut() = Some(remember_coroutine_scope());
            let nv: Rc<dyn ThreePaneScaffoldNavigator<i32>> = reflow_adapt_strategy_sample();
            *n.borrow_mut() = Some(nv.clone());
            NavigableListDetailPaneScaffold(
                nv,
                list_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("liste"), content = |_: &AnimatedPaneScope| {});
                },
                detail_pane = |pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("ayrıntı"), content = |_: &AnimatedPaneScope| {});
                },
                extra_pane = Some(Rc::new(|pane: &ThreePaneScaffoldPaneScope| {
                    pane.AnimatedPane(modifier = Modifier.test_tag("ek"), content = |_: &AnimatedPaneScope| {});
                }) as ThreePanePane),
            );
        }),
        IntSize::new(700, 1000),
    );
    let navigator = navigator.borrow().clone().unwrap();
    let directive = navigator.scaffold_directive();
    assert_eq!((directive.max_horizontal_partitions, directive.max_vertical_partitions), (1, 2));
    let go = navigator.navigate_to_with(ListDetailPaneScaffoldRole::Extra, None);
    let coroutine_scope = scope.borrow().clone().unwrap();
    named_args!(coroutine_scope.launch(async move {
        go.await;
    }));
    rule.wait_for_idle();
    assert_eq!(navigator.scaffold_value().tertiary, PaneAdaptedValue::reflowed(ListDetailPaneScaffoldRole::Detail));
    let (detail, extra) = (bounds(&rule, "ayrıntı"), bounds(&rule, "ek"));
    assert!(near(detail.top, 0.0) && extra.top > detail.bottom, "{detail:?} {extra:?}");
    assert!(near(extra.left, 0.0) && near(extra.right, 700.0), "{extra:?}");
    rule.on_node_with_tag("liste").assert_does_not_exist();
}

fn navigable_list_detail_pane_scaffold_sample() {
    // The welcome screen: the back behaviors, the first selected; PopLatest picked, Next to the scaffold.
    let rule = KompozTestRule::with_viewport(NavigableListDetailPaneScaffoldSample, IntSize::new(400, 800));
    assert!(has_text(&rule, "How should the scaffold handle back navigation?"));
    let behavior = |name: &str| named_args!(rule.on_node_with_text(name, substring = true));
    behavior("PopUntilScaffoldValueChange").assert(kompoz_test::is_selected());
    behavior("PopLatest").perform_click();
    rule.wait_for_idle();
    behavior("PopLatest").assert(kompoz_test::is_selected());
    behavior("PopUntilScaffoldValueChange").assert(kompoz_test::is_not_selected());
    click_text(&rule, "Next");
    assert!(has_text(&rule, "Item 1") && !has_text(&rule, "How should the scaffold handle back navigation?"));
    // PopLatest: from the extra pane back to the detail, then to the list.
    click_text(&rule, "Item 1");
    click_text(&rule, "Show extra");
    assert!(has_text(&rule, "This is extra content about Extra 1."));
    click_back(&rule);
    assert!(has_text(&rule, "This is a description about Item 1."));
    click_back(&rule);
    assert!(has_text(&rule, "Item 2"));
    // The platform's back: the scaffold has nothing to go back to — the route pops to the welcome screen, its
    // choice kept.
    assert!(escape(&rule));
    assert!(has_text(&rule, "How should the scaffold handle back navigation?"));
    behavior("PopLatest").assert(kompoz_test::is_selected());
}
