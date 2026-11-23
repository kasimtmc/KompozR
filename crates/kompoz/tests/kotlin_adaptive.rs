//! Material's adaptive layouts and navigation suite in Kotlin's syntax, as a Compose user writes them: the C++
//! Kompoz's sources (kompozable_tests.cpp's LayoutForms pane scaffolds and MaterialForms' NavigationSuiteScaffold,
//! kompoz_tests.cpp's named-form pane scaffolds) and its adaptive tests written in Kotlin through `kompoz!`, and the
//! adaptive-layout and navigation-suite samples from `.kt` files (kompoz_file!).

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
// (the samples': a NavHost's back handler as the Kotlin file imports it — androidx.activity.compose.BackHandler —,
// a scrim's semantics' click, the platform's back key)
use kompoz::ui::back::BackHandler;
use kompoz::ui::input::{Key, KeyModifiers};
use kompoz::ui::semantics::SemanticsActions;
use std::cell::RefCell;
use std::rc::Rc;

// ==== window size classes and pane scaffolds =========================================================================
// kompozable_tests.cpp's LayoutForms pane scaffold (its list-detail scaffold of two cities with a pane expansion
// state of a proportion anchor and a drag handle) and kompoz_tests.cpp's layout_forms_take_compose_named_arguments
// pane scaffolds (both scaffolds by name), written as Compose writes them: the C++'s `ListDetailPaneScaffold(
// navigator =, listPane =, …, listPaneWidth =)` is Compose's NavigableListDetailPaneScaffold of a navigator, its panes
// AnimatedPanes, the list's width `Modifier.preferredWidth(…)` (Compose has no `listPaneWidth`).

kompoz! {
    @Kompozable
    fun LayoutFormsPanes() {
        val navigator = rememberListDetailPaneScaffoldNavigator<Int>()
        val expansion = rememberPaneExpansionState(anchors = listOf(PaneExpansionAnchor.Proportion(0.5f)))
        val cities = listOf("Ankara", "İzmir")
        val scope = rememberCoroutineScope()
        Box(modifier = Modifier.size(400.dp, 200.dp)) {
            NavigableListDetailPaneScaffold(
                navigator = navigator,
                listPane = {
                    AnimatedPane(modifier = Modifier.preferredWidth(160.dp)) {
                        Column {
                            for (i in 0 until 2) {
                                Text(cities[i], modifier = Modifier.clickable {
                                    scope.launch { navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, i) }
                                })
                            }
                        }
                    }
                },
                detailPane = {
                    AnimatedPane {
                        val city = navigator.currentDestination?.contentKey
                        if (city == null) Text("seçin") else Text("şehir: ${cities[city]}")
                    }
                },
                paneExpansionState = expansion,
                paneExpansionDragHandle = { state ->
                    val interactionSource = remember { MutableInteractionSource() }
                    VerticalDragHandle(
                        modifier = Modifier.paneExpansionDraggable(
                            state,
                            LocalMinimumInteractiveComponentSize.current,
                            interactionSource,
                            state.defaultDragHandleSemantics(),
                        ),
                        interactionSource = interactionSource,
                    )
                },
            )
        }
    }

    @Kompozable
    fun NamedPaneScaffolds() {
        val navigator = rememberListDetailPaneScaffoldNavigator<Int>()
        val support = rememberSupportingPaneScaffoldNavigator<Int>()
        val scope = rememberCoroutineScope()
        Column {
            Box(modifier = Modifier.size(400.dp, 200.dp)) {
                NavigableListDetailPaneScaffold(
                    navigator = navigator,
                    listPane = {
                        AnimatedPane(modifier = Modifier.preferredWidth(160.dp)) {
                            Text("liste", modifier = Modifier.clickable {
                                scope.launch { navigator.navigateTo(pane = ListDetailPaneScaffoldRole.Detail, contentKey = 1) }
                            })
                        }
                    },
                    detailPane = {
                        AnimatedPane {
                            val key = navigator.currentDestination?.contentKey
                            Text(if (key != null) "ayrıntı $key" else "seçin")
                        }
                    },
                )
            }
            Box(modifier = Modifier.size(400.dp, 100.dp)) {
                NavigableSupportingPaneScaffold(
                    navigator = support,
                    mainPane = { AnimatedPane { Text("ana") } },
                    supportingPane = { AnimatedPane(modifier = Modifier.preferredWidth(120.dp)) { Text("destek") } },
                )
            }
        }
    }
}

fn layout_forms_as_compose_writes_them_panes() {
    let rule = KompozTestRule::with_viewport(LayoutFormsPanes, IntSize::new(400, 800));
    rule.wait_for_idle();
    assert!(named_args!(rule.on_node_with_text("Ankara")).exists());
    named_args!(rule.on_node_with_text("İzmir")).perform_click();
    rule.wait_for_idle();
    assert!(named_args!(rule.on_node_with_text("şehir: İzmir")).exists());
}

fn layout_forms_take_compose_named_arguments_panes() {
    let rule = KompozTestRule::with_viewport(NamedPaneScaffolds, IntSize::new(400, 800));
    rule.wait_for_idle();
    assert!(named_args!(rule.on_node_with_text("liste")).exists());
    assert!(named_args!(rule.on_node_with_text("ana")).exists());
    named_args!(rule.on_node_with_text("liste")).perform_click();
    rule.wait_for_idle();
    assert!(named_args!(rule.on_node_with_text("ayrıntı 1")).exists());
}

// ---- samples ------------------------------------------------------------------------------------------------------
// adaptive-layout 1.2.0's ThreePaneScaffoldSample.kt from tests/kt/adaptive_layout.kt, their behaviour checked as
// the Rust syntax's twins in kompoz-material3-adaptive's tests/samples.rs check it. What the Kotlin syntax doesn't
// take yet stands here in Rust, as Compose writes it: the Parcelable data class `NavItemData` (classes) and the
// generic `reflowAdaptStrategySample<T>()` and `levitateAsDialogSample<T>()` (generic functions; Rust's names
// theirs, as the Kotlin syntax calls them) come with stage 14.

/// Kotlin: `private data class NavItemData(val index: Int, val showExtra: Boolean = false) : Parcelable` (its
/// parcel's two fields its saved value; its properties read as Kotlin's syntax reads them).
#[derive(Clone, Copy, PartialEq, Debug)]
struct NavItemData {
    index: i32,
    show_extra: bool,
}

/// Kotlin: `NavItemData(index: Int, showExtra: Boolean = false)`
#[named(type)]
fn NavItemData(index: i32, #[default(false)] show_extra: bool) -> NavItemData {
    NavItemData { index, show_extra }
}

impl NavItemData {
    /// Kotlin: `val index: Int`
    fn index(&self) -> i32 {
        self.index
    }

    /// Kotlin: `val showExtra: Boolean`
    fn show_extra(&self) -> bool {
        self.show_extra
    }
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

kompoz_file!("tests/kt/adaptive_layout.kt");

const KT_HANDLE: &str = "Pane expansion drag handle";

fn kt_samples_has_text(rule: &KompozTestRule, text: &str) -> bool {
    named_args!(rule.on_node_with_text(text)).exists()
}

fn kt_samples_text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text)).get_unclipped_bounds_in_root()
}

fn kt_samples_click(rule: &KompozTestRule, text: &str) {
    named_args!(rule.on_node_with_text(text)).perform_click();
    rule.wait_for_idle();
}

fn kt_samples_back(rule: &KompozTestRule) {
    rule.on_node_with_content_description("Back").perform_click();
    rule.wait_for_idle();
}

/// The list's card of `text` (the detail's top bar shows the same text).
fn kt_samples_card<'a>(rule: &'a KompozTestRule, text: &str) -> kompoz::test::SemanticsNodeInteraction<'a> {
    rule.on_node_with(kompoz::test::has_text(text, false, false).and(kompoz::test::is_selectable()), false)
}

fn kt_samples_near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn list_detail_pane_scaffold_samples_in_kotlin() {
    // ListDetailPaneScaffoldSample. Expanded: the list (200 dp preferred) and the detail side by side; a card picks
    // the detail, selected while it is shown. Compact: one pane at a time, the detail's back button to the list.
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSample, IntSize::new(1000, 800));
    assert!(kt_samples_has_text(&rule, "Item 1") && kt_samples_has_text(&rule, "No item selected"));
    assert!(kt_samples_text_bounds(&rule, "Item 1").right <= 200.0);
    kt_samples_click(&rule, "Item 2");
    assert!(kt_samples_has_text(&rule, "This is a description about Item 2."));
    kt_samples_card(&rule, "Item 2").assert(kompoz::test::is_selected());
    kt_samples_card(&rule, "Item 1").assert(kompoz::test::is_not_selected());
    rule.on_node_with_content_description("Back").assert_does_not_exist();
    drop(rule);
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSample, IntSize::new(400, 800));
    kt_samples_click(&rule, "Item 3");
    assert!(kt_samples_has_text(&rule, "This is a description about Item 3.") && !kt_samples_has_text(&rule, "Item 1"));
    kt_samples_back(&rule);
    assert!(kt_samples_has_text(&rule, "Item 1"));
    drop(rule);

    // ListDetailPaneScaffoldSampleWithExtraPane: the split at the second anchor (240 dp: the list 228 wide), the
    // handle between; the extra pane in place of the list, the extra's back to it.
    let rule = KompozTestRule::with_viewport(ListDetailPaneScaffoldSampleWithExtraPane, IntSize::new(1000, 800));
    rule.on_node_with_content_description(KT_HANDLE).assert_exists();
    assert!(kt_samples_near(kt_samples_text_bounds(&rule, "Item 1").right, 228.0));
    kt_samples_click(&rule, "Item 1");
    kt_samples_click(&rule, "Show extra");
    assert!(
        kt_samples_has_text(&rule, "This is extra content about Extra 1.") && !kt_samples_has_text(&rule, "Item 2")
    );
    let backs = rule.on_all_nodes_with_content_description("Back");
    backs.get(1).perform_click();
    rule.wait_for_idle();
    assert!(
        kt_samples_has_text(&rule, "Item 2") && !kt_samples_has_text(&rule, "This is extra content about Extra 1.")
    );
    drop(rule);

    // ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog. Compact: levitated over the detail (its semantics
    // cleared) behind the scrim, dismissed by the scrim's semantics' click. Expanded: beside the detail, no scrim.
    let rule = KompozTestRule::with_viewport(
        ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog,
        IntSize::new(400, 800),
    );
    kt_samples_click(&rule, "Item 2");
    kt_samples_click(&rule, "Show extra");
    assert!(kt_samples_has_text(&rule, "This is extra content about Extra 2."));
    assert!(!kt_samples_has_text(&rule, "This is a description about Item 2."));
    let scrim = rule.on_node_with_content_description("Scrim");
    assert_eq!(scrim.get_unclipped_bounds_in_root(), Rect::new(0.0, 0.0, 400.0, 800.0));
    scrim.perform_semantics_action(SemanticsActions::OnClick);
    rule.wait_for_idle();
    assert!(!kt_samples_has_text(&rule, "This is extra content about Extra 2."));
    assert!(kt_samples_has_text(&rule, "This is a description about Item 2."));
    drop(rule);
    let rule = KompozTestRule::with_viewport(
        ListDetailPaneScaffoldSampleWithExtraPaneLevitatedAsDialog,
        IntSize::new(1000, 800),
    );
    kt_samples_click(&rule, "Item 2");
    kt_samples_click(&rule, "Show extra");
    assert!(kt_samples_has_text(&rule, "This is extra content about Extra 2."));
    rule.on_node_with_content_description("Scrim").assert_does_not_exist();
}

fn supporting_pane_scaffold_sample_in_kotlin() {
    // Expanded: the main pane, the supporting pane at the end (its cards one selected at a time), the handle; the
    // extra pane in place of the supporting one. Compact: the main pane alone.
    let rule = KompozTestRule::with_viewport(SupportingPaneScaffoldSample, IntSize::new(1000, 800));
    assert!(kt_samples_text_bounds(&rule, "Item 1").left > kt_samples_text_bounds(&rule, "My content").right);
    rule.on_node_with_content_description(KT_HANDLE).assert_exists();
    kt_samples_click(&rule, "Item 2");
    kt_samples_card(&rule, "Item 2").assert(kompoz::test::is_selected());
    kt_samples_click(&rule, "Item 3");
    kt_samples_card(&rule, "Item 2").assert(kompoz::test::is_not_selected());
    kt_samples_click(&rule, "Show extra");
    assert!(kt_samples_has_text(&rule, "This is extra content about Extra content."));
    kt_samples_back(&rule);
    kt_samples_card(&rule, "Item 3").assert(kompoz::test::is_selected());
    drop(rule);
    let rule = KompozTestRule::with_viewport(SupportingPaneScaffoldSample, IntSize::new(400, 800));
    assert!(kt_samples_has_text(&rule, "My content") && !kt_samples_has_text(&rule, "Item 1"));
}

/// The preferred size samples (the list pane's, its content tagged) and the drag handle sample, its own state, in a
/// scaffold. (The file's extension composables, called by their implicit receiver from Kotlin elsewhere, would be
/// imported by that file: Kotlin's imports come with stage 14, so they are called here as Rust calls them.)
fn preferred_sizes(proportion: bool) -> impl Fn() + 'static {
    kompozable_lambda!(move || {
        let navigator = remember_list_detail_pane_scaffold_navigator::<i32>();
        NavigableListDetailPaneScaffold(
            navigator,
            list_pane = move |pane: &ThreePaneScaffoldPaneScope| {
                let tagged = || Box(modifier = Modifier.fill_max_size().test_tag("liste"));
                if proportion {
                    pane.PreferredSizeModifierInProportionSample(0.25, 0.5, tagged);
                } else {
                    pane.PreferredSizeModifierInDpSample(300.dp(), 300.dp(), tagged);
                }
            },
            detail_pane = |pane: &ThreePaneScaffoldPaneScope| {
                pane.AnimatedPane(modifier = Modifier.test_tag("ayrıntı"), content = |_: &AnimatedPaneScope| {});
            },
            pane_expansion_drag_handle = Some(Rc::new(kompozable_lambda!(
                |scope: &ThreePaneScaffoldScope, _state: PaneExpansionState| scope.PaneExpansionDragHandleSample()
            )) as ThreePaneDragHandle),
        );
    })
}

/// The reflow sample's navigator in a scaffold, its extra pane shown.
fn reflowed() -> impl Fn() + 'static {
    kompozable_lambda!(|| {
        let navigator: Rc<dyn ThreePaneScaffoldNavigator<i32>> = reflow_adapt_strategy_sample();
        let n = navigator.clone();
        LaunchedEffect((), async move {
            n.navigate_to_with(ListDetailPaneScaffoldRole::Extra, None).await;
        });
        NavigableListDetailPaneScaffold(
            navigator,
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
    })
}

fn preferred_size_drag_handle_and_reflow_samples_in_kotlin() {
    // The list 300 dp, then a quarter of the scaffold; the default drag handle sample's 48 dp touch target.
    let rule = KompozTestRule::with_viewport(preferred_sizes(false), IntSize::new(1000, 800));
    assert_eq!(rule.on_node_with_tag("liste").get_unclipped_bounds_in_root(), Rect::new(0.0, 0.0, 300.0, 800.0));
    let handle = rule.on_node_with_content_description(KT_HANDLE).get_unclipped_bounds_in_root();
    assert!(kt_samples_near(handle.width(), 48.0) && kt_samples_near(handle.center().x, 312.0), "{handle:?}");
    drop(rule);
    let rule = KompozTestRule::with_viewport(preferred_sizes(true), IntSize::new(1000, 800));
    assert_eq!(rule.on_node_with_tag("liste").get_unclipped_bounds_in_root(), Rect::new(0.0, 0.0, 250.0, 800.0));
    drop(rule);
    // reflowAdaptStrategySample: a tall medium window, the extra pane reflowed under the detail.
    let rule = KompozTestRule::with_viewport(reflowed(), IntSize::new(700, 1000));
    rule.wait_for_idle();
    let (detail, extra) = (
        rule.on_node_with_tag("ayrıntı").get_unclipped_bounds_in_root(),
        rule.on_node_with_tag("ek").get_unclipped_bounds_in_root(),
    );
    assert!(kt_samples_near(detail.top, 0.0) && extra.top > detail.bottom, "{detail:?} {extra:?}");
    rule.on_node_with_tag("liste").assert_does_not_exist();
}

fn navigable_list_detail_pane_scaffold_sample_in_kotlin() {
    // The welcome screen's back behaviors, PopLatest picked; Next; from the extra pane back to the detail, to the
    // list; the platform's back to the welcome screen, its choice kept.
    let rule = KompozTestRule::with_viewport(NavigableListDetailPaneScaffoldSample, IntSize::new(400, 800));
    let behavior = |name: &str| named_args!(rule.on_node_with_text(name, substring = true));
    behavior("PopUntilScaffoldValueChange").assert(kompoz::test::is_selected());
    behavior("PopLatest").perform_click();
    rule.wait_for_idle();
    behavior("PopLatest").assert(kompoz::test::is_selected());
    kt_samples_click(&rule, "Next");
    kt_samples_click(&rule, "Item 1");
    kt_samples_click(&rule, "Show extra");
    assert!(kt_samples_has_text(&rule, "This is extra content about Extra 1."));
    kt_samples_back(&rule);
    assert!(kt_samples_has_text(&rule, "This is a description about Item 1."));
    kt_samples_back(&rule);
    assert!(kt_samples_has_text(&rule, "Item 2"));
    assert!(rule.host().key_down(Key::Escape, KeyModifiers::default()));
    rule.wait_for_idle();
    assert!(kt_samples_has_text(&rule, "How should the scaffold handle back navigation?"));
    behavior("PopLatest").assert(kompoz::test::is_selected());
}

// ==== navigation suite ===============================================================================================
// kompozable_tests.cpp's MaterialForms NavigationSuiteScaffold (its "Settings"/"Tab 1" checks of
// material_as_compose_writes_it), the C++ Kompoz's navigation-suite tests (kompoz_tests.cpp:
// window_size_classes_and_the_navigation_they_suit, navigation_suite_scaffold_moves_its_items_and_keeps_the_screen,
// navigation_suite_types_as_compose_1_4, navigation_suite_scaffold_with_its_own_items_a_primary_action_and_hiding,
// navigation_suite_items_suit_each_type, the NavigationSuiteScaffoldLayout cases of
// scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose) written in Kotlin, checked as
// crates/kompoz-material3-adaptive-navigation-suite/tests/navigation_suite.rs checks them (M15's values; the
// differences from the C++'s said there), and NavigationSuiteScaffoldSamples from kt/navigation_suite.kt.

kompoz! {
    // (MaterialForms' NavigationSuiteScaffold part: its items given to the NavigationSuiteScope)
    @Kompozable
    fun MaterialFormsNavigationSuite() {
        var tab by rememberSaveable { mutableStateOf(0) }
        Column {
            NavigationSuiteScaffold(
                navigationSuiteItems = {
                    item(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Home") })
                    item(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Settings") })
                },
                modifier = Modifier.height(300.dp),
                layoutType = NavigationSuiteType.NavigationBar,
            ) {
                Text("Tab $tab")
            }
        }
    }

    @Composable
    fun SuitedTypes(log: (String) -> Unit) {
        val phone = WindowSizeClass.compute(400f, 800f)
        log("${phone.windowWidthSizeClass} ${phone.windowHeightSizeClass}")
        log("${phone.isHeightAtLeastBreakpoint(WindowSizeClass.HEIGHT_DP_MEDIUM_LOWER_BOUND)} ${phone.isWidthAtLeastBreakpoint(WindowSizeClass.WIDTH_DP_MEDIUM_LOWER_BOUND)}")
        log("${WindowSizeClass.compute(840f, 900f).minHeightDp}")
        for (size in listOf(400f to 800f, 700f to 1000f, 1280f to 800f, 900f to 400f)) {
            val info = WindowAdaptiveInfo(WindowSizeClass.compute(size.first, size.second), Posture())
            log("${NavigationSuiteScaffoldDefaults.calculateFromAdaptiveInfo(info)} ${NavigationSuiteScaffoldDefaults.navigationSuiteType(info)}")
        }
        val tabletop = WindowAdaptiveInfo(WindowSizeClass.compute(700f, 1000f), Posture(isTabletop = true))
        log("${NavigationSuiteScaffoldDefaults.navigationSuiteType(tabletop)} ${NavigationSuiteScaffoldDefaults.calculateFromAdaptiveInfo(tabletop)}")
        log("${currentWindowAdaptiveInfo().windowPosture.isTabletop}")
    }

    @Composable
    fun MovingItems(forced: NavigationSuiteType?, onPadding: (PaddingValues) -> Unit) {
        var tab by remember { mutableStateOf(0) }
        NavigationSuiteScaffold(
            navigationSuiteItems = {
                item(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Ev") })
                item(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Ayarlar") })
            },
            modifier = Modifier.fillMaxSize(),
            layoutType = forced ?: NavigationSuiteScaffoldDefaults.calculateFromAdaptiveInfo(currentWindowAdaptiveInfo()),
        ) {
            Scaffold(Modifier.fillMaxSize()) { padding ->
                onPadding(padding)
                var count by remember { mutableStateOf(0) }
                Box(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding).imePadding()) {
                    Box(Modifier.fillMaxSize().testTag("ekran"))
                    TextButton(onClick = { count++ }, modifier = Modifier.testTag("sayac")) { Text("sayaç $count") }
                }
            }
        }
    }

    @Composable
    fun OwnItemsAndAction(tab: Int, onTab: (Int) -> Unit, state: NavigationSuiteScaffoldState) {
        NavigationSuiteScaffold(
            navigationItems = {
                NavigationSuiteItem(tab == 0, { onTab(0) }, { Icon(Icons.Filled.Home, "") }, { Text("Ev") })
                NavigationSuiteItem(tab == 1, { onTab(1) }, { Icon(Icons.Filled.Settings, "") }, { Text("Ayarlar") },
                                    badge = { Badge { Text("3") } })
            },
            modifier = Modifier.fillMaxSize(),
            state = state,
            primaryActionContent = { Box(Modifier.size(56.dp).testTag("eylem")) },
        ) {
            Box(Modifier.fillMaxSize().testTag("ekran"))
        }
    }

    @Composable
    fun HidingScaffold() {
        var tab by remember { mutableStateOf(0) }
        val state = rememberNavigationSuiteScaffoldState()
        val scope = rememberCoroutineScope()
        Column {
            Row {
                Button(onClick = { scope.launch { state.hide() } }, modifier = Modifier.testTag("gizle")) { Text("gizle") }
                Button(onClick = { scope.launch { state.show() } }, modifier = Modifier.testTag("göster")) { Text("göster") }
                Button(onClick = { scope.launch { state.toggle() } }, modifier = Modifier.testTag("çevir")) { Text("çevir") }
                Text("durum ${state.currentValue} ${state.targetValue}")
            }
            OwnItemsAndAction(tab, { tab = it }, state)
        }
    }

    @Composable
    fun ItemsOfEachType(type: NavigationSuiteType, onClick: () -> Unit) {
        NavigationSuite(type, Modifier.testTag("suite"), verticalArrangement = Arrangement.Bottom,
                        primaryActionContent = { Box(Modifier.size(40.dp).testTag("başlık")) }) {
            NavigationSuiteItem(true, {}, { Icon(Icons.Filled.Home, "") }, { Text("Ev") }, navigationSuiteType = type)
            NavigationSuiteItem(false, onClick, { Icon(Icons.Filled.Settings, "") }, { Text("Kapalı") },
                                navigationSuiteType = type, enabled = false)
        }
    }

    @Composable
    fun ScaffoldBody(which: Int) {
        val body = @Composable { Box(Modifier.size(60.dp, 40.dp).testTag("icerik")) }
        val bar = @Composable { Box(Modifier.size(80.dp, 80.dp)) }
        when (which) {
            2 -> NavigationSuiteScaffoldLayout(navigationSuite = bar, navigationSuiteType = NavigationSuiteType.NavigationBar, content = body)
            3 -> NavigationSuiteScaffoldLayout(navigationSuite = bar, navigationSuiteType = NavigationSuiteType.NavigationRail, content = body)
            else -> NavigationSuiteScaffoldLayout(navigationSuite = bar, layoutType = NavigationSuiteType.None, content = body)
        }
    }
}

fn kt_suite_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A text's own bounds (an item merges its label).
fn kt_suite_text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with(kompoz::test::has_text(text, false, false), true).get_unclipped_bounds_in_root()
}

/// The bounds of the one placed node tagged `tag` (others may be composed but not placed).
fn kt_suite_placed_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let nodes: Vec<_> =
        rule.on_all_nodes_with_tag(tag).fetch_semantics_nodes().into_iter().filter(|n| n.is_placed()).collect();
    assert_eq!(nodes.len(), 1, "{tag}");
    nodes[0].unclipped_bounds_in_root()
}

fn kt_suite_near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn material_forms_navigation_suite() {
    let rule = KompozTestRule::with_viewport(MaterialFormsNavigationSuite, IntSize::new(400, 800));
    rule.on_node_with_text("Tab 0").assert_exists();
    rule.on_node_with_text("Settings").perform_click();
    rule.on_node_with_text("Tab 1").assert_exists();
}

fn navigation_suite_types_in_kotlin() {
    // window_size_classes_and_the_navigation_they_suit + navigation_suite_types_as_compose_1_4 (Compose for
    // desktop's posture: always the default one, whatever the host says).
    let log: std::rc::Rc<std::cell::RefCell<Vec<String>>> = Default::default();
    let l = log.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let l = l.clone();
            SuitedTypes(move |s: String| l.borrow_mut().push(s))
        }),
        IntSize::new(700, 1000),
    );
    assert_eq!(
        log.borrow().clone(),
        [
            "WindowWidthSizeClass: COMPACT WindowHeightSizeClass: MEDIUM",
            "true false",
            "900",
            "NavigationBar ShortNavigationBarCompact",
            "NavigationRail WideNavigationRailCollapsed",
            "NavigationRail WideNavigationRailCollapsed",
            "NavigationBar ShortNavigationBarMedium",
            "ShortNavigationBarMedium NavigationBar",
            "false",
        ]
    );
    rule.host().set_media_info(kompoz::ui::media_query::MediaInfo {
        window_posture: kompoz::ui::media_query::Posture::Tabletop,
        ..Default::default()
    });
    rule.wait_for_idle();
    assert_eq!(log.borrow().last().map(String::as_str), Some("false"));
}

fn navigation_suite_scaffold_moves_its_items_in_kotlin() {
    let forced: MutableState<Option<NavigationSuiteType>> = mutable_state_of(None);
    let seen: std::rc::Rc<std::cell::Cell<(Dp, Dp)>> = std::rc::Rc::new(std::cell::Cell::new((Dp(-1.0), Dp(-1.0))));
    let (f, s) = (forced.clone(), seen.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            MovingItems(f.get(), move |p: PaddingValues| {
                s.set((p.calculate_top_padding(), p.calculate_bottom_padding()))
            })
        }),
        IntSize::new(400, 800),
    );
    let insets = |ime: i32| kompoz::ui::platform::SystemInsets {
        status_bars: kompoz::ui::platform::Insets::new(0, 24, 0, 0),
        navigation_bars: kompoz::ui::platform::Insets::new(0, 0, 0, 48),
        ime: kompoz::ui::platform::Insets::new(0, 0, 0, ime),
        ..Default::default()
    };
    rule.host().set_system_insets(insets(0));
    rule.wait_for_idle();
    // A bar at the bottom (80 dp over the navigation bar's 48); the screen's Scaffold pads by the status bar only.
    assert!(kt_suite_text_bounds(&rule, "Ev").top > 650.0);
    assert_eq!(seen.get(), (Dp(24.0), Dp(0.0)));
    let screen = kt_suite_bounds(&rule, "ekran");
    assert!(
        kt_suite_near(screen.top, 24.0, 0.5) && kt_suite_near(screen.bottom, 800.0 - 80.0 - 48.0, 0.5),
        "{screen:?}"
    );
    rule.on_node_with_text("Ayarlar").perform_click();
    rule.on_node_with_text("Ayarlar").assert(kompoz::test::is_selected());
    rule.on_node_with_tag("sayac").perform_click();
    rule.on_node_with_text("sayaç 1").assert_exists();
    // The keyboard up: M15's ime padding is the keyboard less the bar's consumed 48, from the screen's own bottom
    // (the C++ ended the screen at the keyboard's top, 500, and a keyboard lower than the bar did nothing).
    rule.host().set_system_insets(insets(300));
    rule.wait_for_idle();
    assert!(kt_suite_near(kt_suite_bounds(&rule, "ekran").bottom, 800.0 - 80.0 - 48.0 - (300.0 - 48.0), 0.5));
    rule.host().set_system_insets(insets(100));
    rule.wait_for_idle();
    assert!(kt_suite_near(kt_suite_bounds(&rule, "ekran").bottom, 800.0 - 80.0 - 48.0 - (100.0 - 48.0), 0.5));
    rule.host().set_system_insets(insets(0));
    // Wider: a rail at the start (not keeping clear of the bottom: the Scaffold pads by it); a drawer asked for; none.
    rule.host().set_viewport_size(IntSize::new(1000, 800));
    rule.wait_for_idle();
    let home = kt_suite_text_bounds(&rule, "Ev");
    assert!(home.left < 80.0 && home.top < 300.0 && kt_suite_bounds(&rule, "sayac").left >= 80.0);
    assert_eq!(seen.get().1, Dp(48.0));
    rule.on_node_with_text("sayaç 1").assert_exists();
    forced.set(Some(NavigationSuiteType::NavigationDrawer));
    rule.wait_for_idle();
    assert!(kt_suite_bounds(&rule, "sayac").left > 250.0);
    rule.on_node_with_text("sayaç 1").assert_exists();
    forced.set(Some(NavigationSuiteType::None));
    rule.wait_for_idle();
    rule.on_node_with_text("Ayarlar").assert_does_not_exist();
    assert!(kt_suite_near(kt_suite_bounds(&rule, "sayac").left, 0.0, 0.5));
    rule.on_node_with_text("sayaç 1").assert_exists();
}

fn navigation_suite_scaffold_with_its_own_items_in_kotlin() {
    let rule = KompozTestRule::with_viewport(HidingScaffold, IntSize::new(400, 800));
    // A short bar (64 dp) at the bottom, the action 16 dp above its end.
    let column_top = kt_suite_bounds(&rule, "ekran").top;
    let (screen, action) = (kt_suite_bounds(&rule, "ekran"), kt_suite_placed_bounds(&rule, "eylem"));
    assert!(kt_suite_near(screen.bottom, 800.0 - 64.0, 0.5) && kt_suite_text_bounds(&rule, "Ev").top > 736.0);
    rule.on_node_with_text("3").assert_exists();
    assert!(
        kt_suite_near(action.left, 400.0 - 16.0 - 56.0, 0.5)
            && kt_suite_near(action.top, 800.0 - 64.0 - 16.0 - 56.0, 0.5)
    );
    rule.on_node_with_text("Ayarlar").perform_click();
    rule.on_node_with_text("Ayarlar").assert(kompoz::test::is_selected());
    // Hidden: the screen takes the bar's room, the action comes down with it.
    rule.on_node_with_tag("gizle").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("durum Hidden Hidden").assert_exists();
    assert!(kt_suite_near(kt_suite_bounds(&rule, "ekran").bottom, 800.0, 0.5) && column_top < 800.0);
    assert!(kt_suite_near(kt_suite_placed_bounds(&rule, "eylem").top, 800.0 - 16.0 - 56.0, 0.5));
    rule.on_node_with_tag("göster").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("durum Visible Visible").assert_exists();
    // Wider: a collapsed wide rail (96 dp), the action its header; toggled, out to the start.
    rule.host().set_viewport_size(IntSize::new(1000, 800));
    rule.wait_for_idle();
    let (screen, home, action) =
        (kt_suite_bounds(&rule, "ekran"), kt_suite_text_bounds(&rule, "Ev"), kt_suite_placed_bounds(&rule, "eylem"));
    assert!(kt_suite_near(screen.left, 96.0, 0.5) && home.left < 96.0 && action.left < 96.0 && action.top < home.top);
    rule.on_node_with_tag("çevir").perform_click();
    rule.wait_for_idle();
    assert!(
        kt_suite_near(kt_suite_bounds(&rule, "ekran").left, 0.0, 0.5) && kt_suite_text_bounds(&rule, "Ev").right <= 0.5
    );
}

fn navigation_suite_items_suit_each_type_in_kotlin() {
    let ty = mutable_state_of(NavigationSuiteType::NavigationRail);
    let clicked = std::rc::Rc::new(std::cell::Cell::new(false));
    let (t, c) = (ty.clone(), clicked.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            ItemsOfEachType(t.get(), move || c.set(true))
        }),
        IntSize::new(1000, 800),
    );
    assert!(kt_suite_bounds(&rule, "başlık").top < 100.0 && kt_suite_text_bounds(&rule, "Kapalı").top > 650.0);
    ty.set(NavigationSuiteType::NavigationDrawer);
    rule.wait_for_idle();
    assert!(kt_suite_bounds(&rule, "suite").width() > 200.0 && kt_suite_text_bounds(&rule, "Kapalı").top > 650.0);
    ty.set(NavigationSuiteType::ShortNavigationBarMedium);
    rule.wait_for_idle();
    assert!(kt_suite_bounds(&rule, "suite").height() < 100.0 && kt_suite_text_bounds(&rule, "Ev").height() < 30.0);
    rule.on_node_with_tag("başlık").assert_does_not_exist();
    rule.on_node_with_text("Kapalı").perform_click();
    assert!(!clicked.get());
    ty.set(NavigationSuiteType::WideNavigationRailExpanded);
    rule.wait_for_idle();
    assert!(kt_suite_bounds(&rule, "suite").width() > 150.0 && kt_suite_text_bounds(&rule, "Kapalı").top > 650.0);
}

fn navigation_suite_scaffold_bodies_in_kotlin() {
    let which = mutable_state_of(2);
    let w = which.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || ScaffoldBody(w.get())), IntSize::new(400, 800));
    for i in 2..=4 {
        which.set(i);
        rule.wait_for_idle();
        let b = kt_suite_bounds(&rule, "icerik");
        assert!(kt_suite_near(b.width(), 60.0, 0.01) && kt_suite_near(b.height(), 40.0, 0.01), "{i}: {b:?}");
    }
}

kompoz_file!("tests/kt/navigation_suite.kt");

fn navigation_suite_scaffold_samples() {
    // NavigationSuiteScaffoldSample: the window's type (V2) told, the items picked, the component hidden and shown.
    let rule = KompozTestRule::with_viewport(NavigationSuiteScaffoldSample, IntSize::new(400, 800));
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarCompact\nVisibility: Visible")
        .assert_exists();
    rule.on_node_with_text("Artists").perform_click();
    rule.on_node_with_text("Artists").assert(kompoz::test::is_selected());
    rule.on_node_with_text("Hide/show navigation component").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarCompact\nVisibility: Hidden")
        .assert_exists();
    assert!(kt_suite_text_bounds(&rule, "Artists").top >= 800.0);
    rule.host().set_viewport_size(IntSize::new(1300, 900));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: WideNavigationRailCollapsed\nVisibility: Hidden")
        .assert_exists();
    // NavigationSuiteScaffoldCustomConfigSample: a collapsed wide rail when compact or medium, expanded when
    // expanded, a short bar when short.
    let rule = KompozTestRule::with_viewport(NavigationSuiteScaffoldCustomConfigSample, IntSize::new(400, 800));
    rule.on_node_with_text("Current NavigationSuiteType: WideNavigationRailCollapsed\nVisibility: Visible")
        .assert_exists();
    let songs = kt_suite_text_bounds(&rule, "Songs");
    assert!(songs.left < 96.0 && songs.top > 250.0 && songs.top < 450.0, "{songs:?}");
    rule.on_node_with_text("Playlists").perform_click();
    rule.on_node_with_text("Playlists").assert(kompoz::test::is_selected());
    rule.host().set_viewport_size(IntSize::new(1000, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: WideNavigationRailExpanded\nVisibility: Visible")
        .assert_exists();
    rule.host().set_viewport_size(IntSize::new(1000, 400));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarMedium\nVisibility: Visible")
        .assert_exists();
}

fn main() {
    common::run(&[
        // (window size classes and pane scaffolds)
        ("layout_forms_as_compose_writes_them_panes", layout_forms_as_compose_writes_them_panes),
        ("layout_forms_take_compose_named_arguments_panes", layout_forms_take_compose_named_arguments_panes),
        ("list_detail_pane_scaffold_samples_in_kotlin", list_detail_pane_scaffold_samples_in_kotlin),
        ("supporting_pane_scaffold_sample_in_kotlin", supporting_pane_scaffold_sample_in_kotlin),
        (
            "preferred_size_drag_handle_and_reflow_samples_in_kotlin",
            preferred_size_drag_handle_and_reflow_samples_in_kotlin,
        ),
        ("navigable_list_detail_pane_scaffold_sample_in_kotlin", navigable_list_detail_pane_scaffold_sample_in_kotlin),
        // (navigation suite)
        ("material_forms_navigation_suite", material_forms_navigation_suite),
        ("navigation_suite_types_in_kotlin", navigation_suite_types_in_kotlin),
        ("navigation_suite_scaffold_moves_its_items_in_kotlin", navigation_suite_scaffold_moves_its_items_in_kotlin),
        (
            "navigation_suite_scaffold_with_its_own_items_in_kotlin",
            navigation_suite_scaffold_with_its_own_items_in_kotlin,
        ),
        ("navigation_suite_items_suit_each_type_in_kotlin", navigation_suite_items_suit_each_type_in_kotlin),
        ("navigation_suite_scaffold_bodies_in_kotlin", navigation_suite_scaffold_bodies_in_kotlin),
        ("navigation_suite_scaffold_samples", navigation_suite_scaffold_samples),
    ]);
}
