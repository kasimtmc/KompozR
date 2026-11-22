//! Material's lists, tabs, dialogs, sheets, swipe to dismiss and pull to refresh in Kotlin's syntax, as a Compose
//! user writes them: the C++ Kompoz's Kotlin sources of those components (kompozable_tests.cpp) through `kompoz!`, and
//! material3's samples of them from `.kt` files (kompoz_file!).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
// (shared by the sections: the window types the Kotlin sources name — Compose's androidx.compose.ui.window imports —
// and the generated code types with, as a tooltip box's position provider)
use kompoz::ui::window::{Dialog, DialogProperties, PopupPositionProvider};
// (sheets: what the generated code types nestedScroll's connection with — BottomSheetScaffoldNestedScrollSample's)
use kompoz::ui::input::NestedScrollConnection;

// ==== lists ==========================================================================================================
// The C++ Kompoz's `list_items_by_name` (kompozable_tests.cpp 651-670, Settings) and the ListItem line of its
// `kotlin_helpers_through_kompozc` (577-615, KotlinHelpers: the rest of it is Kotlin's String.format and collection
// functions, not lists'), and material3's ListSamples with the rest of the list API from kt/lists.kt.

kompoz! {
    // ListItem as Compose writes it: its slots by name; an interactive one's headline is its block.
    @Kompozable
    fun Settings() {
        var wifi by remember { mutableStateOf(false) }
        var opened by remember { mutableStateOf(0) }
        Column {
            ListItem(headlineContent = { Text("Wi-Fi") }, supportingContent = { Text(if (wifi) "Açık" else "Kapalı") })
            ListItem(checked = wifi, onCheckedChange = { wifi = it }) { Text("Wi-Fi'yi aç") }
            ListItem(onClick = { opened++ }, supportingContent = { Text("$opened kez") }) { Text("Hakkında") }
        }
    }

    // (KotlinHelpers' ListItemDefaults.colors: its other lines are String.format's and collections')
    @Kompozable
    fun KotlinHelpersListItem() {
        Column {
            ListItem(headlineContent = { Text("Saydam satır") }, colors = ListItemDefaults.colors(containerColor = Color.Transparent))
        }
    }
}

fn list_items_by_name() {
    let rule = KompozTestRule::new(Settings);
    rule.on_node_with_text("Kapalı").assert_exists();
    rule.on_node_with_text("0 kez").assert_exists();
    rule.on_node_with_text("Wi-Fi'yi aç").perform_click();
    rule.on_node_with_text("Hakkında").perform_click();
    rule.on_node_with_text("Açık").assert_exists();
    rule.on_node_with_text("1 kez").assert_exists();
    // (the checked item a checkbox to accessibility, on now)
    rule.on_node_with_text("Wi-Fi'yi aç").assert_is_on();
}

fn kotlin_helpers_list_item() {
    let rule = KompozTestRule::new(KotlinHelpersListItem);
    rule.on_node_with_text("Saydam satır").assert_exists();
    // (no container drawn: transparent; the headline on surface outside a theme too)
    let canvas = rule.draw();
    assert!(!canvas.rect_colors.contains(&light_color_scheme().surface));
    assert!(canvas.colors.contains(&light_color_scheme().on_surface));
}

kompoz_file!("tests/kt/lists.kt");

/// The samples in a MaterialTheme, in a column that scrolls, in a tall window.
fn themed_tall(content: impl Fn() + 'static) -> KompozTestRule {
    let content = std::rc::Rc::new(content);
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let content = content.clone();
            MaterialTheme(
                content = move || {
                    let content = content.clone();
                    Column(modifier = Modifier.vertical_scroll(&remember_scroll_state()), move || content());
                },
            );
        }),
        // (wide: the test text backend's glyphs are wider than a real font's)
        IntSize::new(800, 900),
    )
}

fn list_item_samples() {
    let rule = themed_tall(kompozable_lambda!(|| {
        OneLineListItem();
        TwoLineListItem();
        ThreeLineListItemWithOverlineAndSupporting();
        ThreeLineListItemWithExtendedSupporting();
    }));
    // (each item one node, its height by its lines: 56, 72, 88, 88)
    let heights: Vec<f32> = rule
        .on_all_nodes(kompoz::test::has_text("list item", true, false))
        .fetch_semantics_nodes()
        .iter()
        .map(|n| n.unclipped_bounds_in_root().height())
        .collect();
    assert_eq!(heights, [56.0, 72.0, 88.0, 88.0]);
    named_args!(rule.on_all_nodes_with_content_description("Localized description", use_unmerged_tree = true))
        .assert_count_equals(4);
}

fn clickable_list_item_samples() {
    let rule = themed_tall(kompozable_lambda!(|| {
        ClickableListItemSample();
        ClickableListItemWithClickableChildSample();
    }));
    rule.on_all_nodes_with_text("Item 2").on_first().perform_click();
    rule.on_all_nodes_with_text("Item 2").on_first().perform_click();
    rule.on_all_nodes_with_text("Item 3").on_first().perform_click();
    // (each item counts its own clicks: 0, 2, 1)
    for count in ["0", "1", "2"] {
        named_args!(rule.on_all_nodes_with_text(count, use_unmerged_tree = true)).assert_count_equals(1);
    }
    // (six items take clicks; the three icon buttons in them nodes of their own)
    let items = kompoz::test::has_click_action().and(kompoz::test::has_text("Item", true, false));
    rule.on_all_nodes(items).assert_count_equals(6);
    rule.on_all_nodes(kompoz::test::has_click_action()).assert_count_equals(9);
}

// (WP4b: icons) SegmentedListItemWithExpansionSample: its Icons.Default.ExpandLess/ExpandMore the extended set's.
fn segmented_list_item_with_expansion_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MaterialTheme(content = || SegmentedListItemWithExpansionSample())),
        IntSize::new(400, 900),
    );
    let header = || rule.on_node_with_text("Click to expand/collapse");
    let state =
        || header().fetch_semantics_node().config().get(kompoz::ui::semantics::SemanticsProperties::StateDescription);
    let drawn = |icon: ImageVector| rule.draw().vectors.contains(&icon.id());
    // Collapsed: ExpandMore; under the 100 dp spacer and the 2 dp gap.
    assert_eq!(state().as_deref(), Some("Collapsed"));
    assert!(drawn(Icons::Default::ExpandMore()) && !drawn(Icons::Default::ExpandLess()));
    assert!((header().get_unclipped_bounds_in_root().top - 102.0).abs() < 0.5);
    rule.on_node_with_text("Child 1").assert_does_not_exist();
    // Expanded: three children 2 dp apart, ExpandLess; each child checks on its own (the saved state list).
    header().perform_click();
    rule.advance_time_by(1000);
    assert_eq!(state().as_deref(), Some("Expanded"));
    assert!(drawn(Icons::Default::ExpandLess()) && !drawn(Icons::Default::ExpandMore()));
    let (one, two) = (
        rule.on_node_with_text("Child 1").get_unclipped_bounds_in_root(),
        rule.on_node_with_text("Child 2").get_unclipped_bounds_in_root(),
    );
    assert!((two.top - one.bottom - 2.0).abs() < 0.5, "{one:?} {two:?}");
    assert_eq!(controls_on(&rule), (0, 0));
    rule.on_node_with_text("Child 2").perform_click();
    rule.advance_time_by(1000);
    rule.on_node_with_text("Child 2").assert_is_on();
    rule.on_node_with_text("Child 3").assert_is_off();
    // (its trailing Checkbox ticked with it)
    assert_eq!(controls_on(&rule), (1, 0));
    // Collapsed: the children gone once they have shrunk; expanded again, the check kept.
    header().perform_click();
    rule.advance_time_by(1000);
    rule.on_node_with_text("Child 2").assert_does_not_exist();
    header().perform_click();
    rule.advance_time_by(1000);
    rule.on_node_with_text("Child 2").assert_is_on();
}

/// How many checked checkboxes (a 20 dp box filled in the primary color) and selected radio buttons (a 10 dp dot in
/// it) are drawn.
fn controls_on(rule: &KompozTestRule) -> (usize, usize) {
    let canvas = rule.draw();
    let primary = light_color_scheme().primary;
    let count = |side: f32| {
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .filter(|(r, c)| **c == primary && (r.width() - side).abs() < 0.01 && (r.height() - side).abs() < 0.01)
            .count()
    };
    (count(20.0), count(10.0))
}

/// A click on the `Item n` list item, its controls' animations done.
fn click_item(rule: &KompozTestRule, n: i32) {
    rule.on_node_with_text(&format!("Item {n}")).perform_click();
    rule.advance_time_by(1000);
}

/// A long click on the `Item n` list item, settled.
fn long_click_item(rule: &KompozTestRule, n: i32) {
    rule.on_node_with_text(&format!("Item {n}")).perform_touch_input(|t| named_args!(t.long_click()));
    rule.advance_time_by(1000);
}

fn selection_list_item_samples() {
    use kompoz::test::{has_role, is_selected};
    use kompoz::ui::semantics::Role;
    // SingleSelectionListItemSample, SingleSelectionSegmentedListItemSample (a selectable group): one item selected at
    // a time, its RadioButton with it; a second click unselects it.
    for (rule, count) in [
        (themed_tall(kompozable_lambda!(|| SingleSelectionListItemSample())), 3),
        (themed_tall(kompozable_lambda!(|| SingleSelectionSegmentedListItemSample())), 4),
    ] {
        rule.on_all_nodes(has_role(Role::RadioButton)).assert_count_equals(count);
        rule.on_all_nodes(is_selected()).assert_count_equals(0);
        click_item(&rule, 2);
        rule.on_node_with_text("Item 2").assert_is_selected();
        assert_eq!(controls_on(&rule), (0, 1));
        click_item(&rule, 3);
        rule.on_node_with_text("Item 3").assert_is_selected();
        rule.on_node_with_text("Item 2").assert_is_not_selected();
        rule.on_all_nodes(is_selected()).assert_count_equals(1);
        assert_eq!(controls_on(&rule), (0, 1));
        click_item(&rule, 3);
        rule.on_all_nodes(is_selected()).assert_count_equals(0);
        assert_eq!(controls_on(&rule), (0, 0));
    }
    // MultiSelectionListItemSample, MultiSelectionSegmentedListItemSample: each item checks on its own, its Checkbox
    // with it.
    for (rule, count) in [
        (themed_tall(kompozable_lambda!(|| MultiSelectionListItemSample())), 3),
        (themed_tall(kompozable_lambda!(|| MultiSelectionSegmentedListItemSample())), 4),
    ] {
        rule.on_all_nodes(has_role(Role::Checkbox)).assert_count_equals(count);
        click_item(&rule, 1);
        click_item(&rule, 3);
        rule.on_node_with_text("Item 1").assert_is_on();
        rule.on_node_with_text("Item 2").assert_is_off();
        rule.on_node_with_text("Item 3").assert_is_on();
        assert_eq!(controls_on(&rule), (2, 0));
        click_item(&rule, 1);
        rule.on_node_with_text("Item 1").assert_is_off();
        assert_eq!(controls_on(&rule), (1, 0));
    }
}

fn list_item_with_mode_change_on_long_click_sample() {
    use kompoz::test::has_role;
    use kompoz::ui::semantics::Role;
    let rule = themed_tall(kompozable_lambda!(|| ListItemWithModeChangeOnLongClickSample()));
    // Click mode: each item counts its clicks (`counts[idx]++`); no checkboxes.
    click_item(&rule, 1);
    click_item(&rule, 1);
    named_args!(rule.on_all_nodes_with_text("2", use_unmerged_tree = true)).assert_count_equals(1);
    rule.on_all_nodes(has_role(Role::Checkbox)).assert_count_equals(0);
    // A long click: checkboxes, the long-clicked item checked.
    long_click_item(&rule, 2);
    rule.on_all_nodes(has_role(Role::Checkbox)).assert_count_equals(3);
    rule.on_node_with_text("Item 2").assert_is_on();
    rule.on_node_with_text("Item 1").assert_is_off();
    assert_eq!(controls_on(&rule), (1, 0));
    click_item(&rule, 3);
    assert_eq!(controls_on(&rule), (2, 0));
    // A long click again: back to counting, the checks cleared (`checked.clear(); checked.addAll(…)`), the counts kept.
    long_click_item(&rule, 1);
    rule.on_all_nodes(has_role(Role::Checkbox)).assert_count_equals(0);
    assert_eq!(controls_on(&rule), (0, 0));
    named_args!(rule.on_all_nodes_with_text("2", use_unmerged_tree = true)).assert_count_equals(1);
    long_click_item(&rule, 3);
    rule.on_node_with_text("Item 3").assert_is_on();
    rule.on_node_with_text("Item 2").assert_is_off();
}

fn list_item_forms() {
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let l = log.clone();
    let rule = themed_tall(kompozable_lambda!(move || {
        let l = l.clone();
        ListItemForms(move |s: String| l.borrow_mut().push(s));
    }));
    // (ListItemDefaults.colors by the older names' overload and by the new ones', a state's color picked)
    rule.on_node_with_text("saydam: true · başlık: true").assert_exists();
    rule.on_node_with_text("seçili: true").assert_exists();
    rule.on_node_with_text("sürüklenen: true").assert_exists();
    rule.on_node_with_text("boşluk: true · yükselti: true").assert_exists();
    // (ListItemColors by its older names, a copy; ListItemElevation, a copy of ListItemShapes; the defaults' colors)
    rule.on_node_with_text("eski: true").assert_exists();
    rule.on_node_with_text("kopya: true").assert_exists();
    rule.on_node_with_text("yükselti: true · biçim: true").assert_exists();
    rule.on_node_with_text("öntanımlı: true").assert_exists();
    // (its long click and click; its shape 8 dp at rest; its headline red)
    rule.on_node_with_tag("biçimli").perform_touch_input(|t| named_args!(t.long_click()));
    rule.on_node_with_tag("biçimli").perform_click();
    rule.on_node_with_text("Karanlık: false").perform_click();
    rule.on_node_with_text("Karanlık: true").assert_is_on();
    rule.on_node_with_text("Plan 2").perform_click();
    rule.on_node_with_tag("plan 2").assert_is_selected();
    rule.on_node_with_tag("plan 0").assert_is_not_selected();
    rule.on_node_with_text("Tek").perform_click();
    assert_eq!(*log.borrow(), ["uzun", "tık", "tek"]);
    let canvas = rule.draw();
    assert!(canvas.colors.contains(&Color::Red));
    // (the segmented items 2 dp apart)
    let (a, b) = (
        rule.on_node_with_tag("plan 0").get_unclipped_bounds_in_root(),
        rule.on_node_with_tag("plan 1").get_unclipped_bounds_in_root(),
    );
    assert!((b.top - a.bottom - 2.0).abs() < 0.01, "{a:?} {b:?}");
}

// ==== tabs and dialogs ===============================================================================================
// The C++ Kompoz's `tabs_and_fab_menu_as_compose_writes_them` (kompozable_tests.cpp 2849-2899, Navigation: its tabs;
// the whole source, with its FAB menu and floating toolbar, is in kotlin_expressive_buttons.rs) and
// `the_users_add_text_dialog` (249-388, AddTextDialog/DialogHost) through `kompoz!`, and material3's TabSamples and
// AlertDialogSamples from kt/tabs_dialogs.kt.

kompoz! {
    // Tabs as Compose writes them (Navigation's tabs: the text and icon tab, the leading icon tab, a scrolling row
    // without its edge padding).
    @Kompozable
    fun NavigationTabs() {
        var tab by remember { mutableStateOf(0) }
        Column {
            PrimaryTabRow(selectedTabIndex = tab) {
                for (i in 0 until 2) Tab(selected = tab == i, onClick = { tab = i }, text = { Text("sekme $i") }, icon = { Icon(Icons.Filled.Star, contentDescription = null) })
            }
            PrimaryScrollableTabRow(selectedTabIndex = tab, edgePadding = 0.dp) {
                for (i in 0 until 2) LeadingIconTab(selected = tab == i, onClick = { tab = i }, text = { Text("kayan $i") }, icon = { Icon(Icons.Filled.Home, contentDescription = null) })
            }
            Text("seçili $tab")
        }
    }

    // A user's own indicator: a composable extension of the tab rows' TabIndicatorScope (as M15's
    // FancyAnimatedIndicatorWithModifier is), called with the scope implicit.
    @Kompozable
    fun TabIndicatorScope.UnderlineIndicator(index: Int, thick: Boolean = false) {
        Box(Modifier.tabIndicatorOffset(index).height(if (thick) 6.dp else 2.dp).background(Color.Red))
    }

    @Kompozable
    fun IndicatorExtension() {
        var tab by remember { mutableStateOf(0) }
        SecondaryTabRow(selectedTabIndex = tab, indicator = { UnderlineIndicator(tab, thick = true) }) {
            for (i in 0 until 2) Tab(selected = tab == i, onClick = { tab = i }, text = { Text("alt $i") })
        }
    }

    // The user's own Compose code, as written for Android (textSize() is theirs too).
    @Kompozable
    fun textSize(): TextUnit = 18.sp

    @Kompozable
    fun AddTextDialog(
        colors: ColorScheme,
        title: String,
        backTitle: String,
        addTitle: String,
        onDismissRequest: () -> Unit,
        onCancel: () -> Unit,
        onSave: (String) -> Unit
    ) {
        Dialog(
            onDismissRequest = { onDismissRequest() },
            properties = DialogProperties(dismissOnBackPress = true, dismissOnClickOutside = true)
        ) {
            // (unused in the C++'s source too: Kotlin's compiler would warn as Rust's does)
            @Suppress("UNUSED_VARIABLE")
            var isBlank by remember { mutableStateOf(false) }
            Card(
                modifier = Modifier
                    .background(
                        brush = Brush.linearGradient(
                            listOf(
                                colors.primaryContainer.copy(alpha = 0.6f),
                                colors.tertiaryContainer.copy(alpha = 0.6f)
                            )
                        ),
                        shape = RoundedCornerShape(28.dp)
                    )
                    .fillMaxWidth()
                    .fillMaxHeight(0.3f)
                    .padding(4.dp),
                colors = CardDefaults.cardColors(containerColor = Color.Transparent),
                shape = RoundedCornerShape(28.dp)
            ) {
                var content by remember { mutableStateOf("") }
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(6.dp),
                    verticalArrangement = Arrangement.SpaceEvenly,
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    TextField (
                        modifier = Modifier
                            .fillMaxWidth()
                            .fillMaxHeight(0.8f)
                            .border(width = 1.dp, color = colors.onBackground.copy(alpha = 0.4f), shape = RoundedCornerShape(24.dp)),
                        value = content,
                        onValueChange = {
                            content= it
                        },
                        textStyle = TextStyle(fontSize = textSize(), color = colors.onBackground),
                        shape = RoundedCornerShape(24.dp),
                        colors = TextFieldDefaults.colors(
                            focusedContainerColor = Color.Transparent,
                            unfocusedContainerColor = Color.Transparent,
                            disabledContainerColor = Color.Transparent,
                            errorContainerColor = Color.Transparent,
                            focusedPlaceholderColor = Color.Transparent,
                            unfocusedPlaceholderColor = Color.Transparent,
                            unfocusedIndicatorColor = Color.Transparent,
                            focusedIndicatorColor = Color.Transparent
                        )
                    )
                    Row {
                        TextButton(
                            onClick = { onCancel() },
                            colors = ButtonDefaults.textButtonColors(
                                containerColor = colors.primaryContainer.copy(alpha = 0.2f),
                                contentColor = colors.onPrimaryContainer
                            ),
                            modifier = Modifier.Companion
                                .width(140.dp)
                                .padding(8.dp),
                        ) {
                            Text(text = backTitle, fontSize = textSize())
                        }
                        TextButton(
                            onClick =
                                {
                                    onSave(content)
                                },
                            colors = ButtonDefaults.textButtonColors(
                                containerColor = colors.primaryContainer.copy(alpha = 0.2f),
                                contentColor = colors.onPrimaryContainer
                            ),
                            modifier = Modifier
                                .width(140.dp)
                                .padding(8.dp),
                        ) {
                            Text(text = addTitle, fontSize = textSize())
                        }
                    }
                }
            }
        }
    }

    @Kompozable
    fun DialogHost(saved: (String) -> Unit) {
        var open by remember { mutableStateOf(true) }
        if (open) {
            AddTextDialog(
                colors = MaterialTheme.colorScheme,
                title = "",
                backTitle = "Vazgeç",
                addTitle = "Ekle",
                onDismissRequest = { open = false },
                onCancel = { open = false },
                onSave = {
                    saved(it)
                    open = false
                }
            )
        }
    }
}

fn tabs_as_compose_writes_them() {
    let rule = KompozTestRule::new(NavigationTabs);
    rule.on_node_with_text("seçili 0").assert_exists();
    rule.on_node_with_text("sekme 1").assert_exists();
    rule.on_node_with_text("kayan 1").assert_exists();
    rule.on_node_with_text("kayan 1").perform_click();
    rule.on_node_with_text("seçili 1").assert_exists();
    rule.on_node_with_text("sekme 0").perform_click();
    rule.on_node_with_text("seçili 0").assert_exists();
}

fn indicators_as_composable_extensions() {
    let rule = KompozTestRule::with_viewport(IndicatorExtension, IntSize::new(400, 800));
    let red = |rule: &KompozTestRule| {
        let canvas = rule.draw();
        canvas.rects.iter().zip(&canvas.rect_colors).find(|(_, c)| **c == Color::Red).map(|(r, _)| *r).expect("red")
    };
    let first = red(&rule);
    assert!(first.left.abs() < 0.5 && (first.width() - 200.0).abs() < 0.5 && (first.height() - 6.0).abs() < 0.5);
    rule.on_node_with_text("alt 1").perform_click();
    rule.wait_for_idle();
    assert!((red(&rule).left - 200.0).abs() < 0.5, "{:?}", red(&rule));
}

fn the_users_add_text_dialog() {
    let saved = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let s = saved.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        DialogHost(move |text: String| *s.borrow_mut() = text);
    }));
    rule.on_node_with_text("Vazgeç").assert_exists();
    rule.on_node_with_text("Ekle").assert_exists();
    *saved.borrow_mut() = "-".into();
    rule.on_node_with_text("Ekle").perform_click();
    assert!(saved.borrow().is_empty()); // (nothing typed)
    rule.on_node_with_text("Ekle").assert_does_not_exist();
    let s = saved.clone();
    let cancel = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        DialogHost(move |text: String| *s.borrow_mut() = text);
    }));
    *saved.borrow_mut() = "-".into();
    cancel.on_node_with_text("Vazgeç").perform_click();
    assert_eq!(*saved.borrow(), "-");
    cancel.on_node_with_text("Vazgeç").assert_does_not_exist();
    // (beyond the C++'s: the field fills the card's column, wider than tall — its own 24 dp corners, no container
    // color drawn —; typed into, Ekle saves the text)
    let s = saved.clone();
    let typed = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        DialogHost(move |text: String| *s.borrow_mut() = text);
    }));
    let field = typed.on_node(kompoz::test::has_set_text_action()).get_unclipped_bounds_in_root();
    assert!(field.width() > field.height(), "{field:?}");
    typed.on_node(kompoz::test::has_set_text_action()).perform_text_input("not");
    typed.on_node_with_text("Ekle").perform_click();
    assert_eq!(*saved.borrow(), "not");
}

kompoz_file!("tests/kt/tabs_dialogs.kt");

/// Composes a tab sample, clicks the tab `click`, and checks `expect` is then shown.
fn tab_sample(content: impl Content + 'static, click: &str, before: &str, expect: &str) {
    let rule = KompozTestRule::with_viewport(content, IntSize::new(400, 800));
    rule.on_node_with_text(before).assert_exists();
    rule.on_node_with_text(click).perform_click();
    rule.on_node_with_text(expect).assert_exists();
}

fn tab_samples() {
    tab_sample(PrimaryTextTabs, "Tab 2", "Primary tab 1 selected", "Primary tab 2 selected");
    tab_sample(SecondaryTextTabs, "Tab 2", "Secondary tab 1 selected", "Secondary tab 2 selected");
    tab_sample(TextTabs, "Tab 2", "Text tab 1 selected", "Text tab 2 selected");
    tab_sample(TextAndIconTabs, "Tab 2", "Text and icon tab 1 selected", "Text and icon tab 2 selected");
    tab_sample(LeadingIconTabs, "Tab & icon", "Leading icon tab 1 selected", "Leading icon tab 2 selected");
    tab_sample(
        ScrollingPrimaryTextTabs,
        "Tab 2",
        "Scrolling primary tab 1 selected",
        "Scrolling primary tab 2 selected",
    );
    tab_sample(
        ScrollingSecondaryTextTabs,
        "Tab 2",
        "Scrolling secondary tab 1 selected",
        "Scrolling secondary tab 2 selected",
    );
    tab_sample(ScrollingTextTabs, "Tab 2", "Scrolling text tab 1 selected", "Scrolling text tab 2 selected");
    tab_sample(FancyTabs, "Tab 2", "Fancy tab 1 selected", "Fancy tab 2 selected");
    tab_sample(FancyIndicatorTabs, "Tab 2", "Fancy indicator tab 1 selected", "Fancy indicator tab 2 selected");
    // (FancyIndicatorContainerTabs and ScrollingFancyIndicatorContainerTabs: see kt/tabs_dialogs.kt — their
    // indicator's states hold Animatables, which KompozR's states can't)
    // The icon samples: three Favorite icon tabs, each in a tooltip box.
    let icon_samples: [fn() -> KompozTestRule; 2] = [
        || KompozTestRule::with_viewport(PrimaryIconTabs, IntSize::new(400, 800)),
        || KompozTestRule::with_viewport(SecondaryIconTabs, IntSize::new(400, 800)),
    ];
    for make in icon_samples {
        let rule = make();
        rule.on_node_with_text("Icon tab 1 selected").assert_exists();
        let icons = rule.on_all_nodes_with_content_description("Favorite").fetch_semantics_nodes();
        assert_eq!(icons.len(), 3);
        let third = icons[2].bounds_in_root().center();
        rule.host().pointer_down(third, -1, 0, kompoz::ui::input::PointerType::Touch);
        rule.host().pointer_up(third, -1, 0);
        rule.wait_for_idle();
        rule.on_node_with_text("Icon tab 3 selected").assert_exists();
    }
}

fn alert_dialog_samples() {
    let samples: [(fn() -> KompozTestRule, &str); 3] = [
        (|| KompozTestRule::with_viewport(AlertDialogSample, IntSize::new(400, 800)), "Confirm"),
        (|| KompozTestRule::with_viewport(AlertDialogWithIconSample, IntSize::new(400, 800)), "Dismiss"),
        (|| KompozTestRule::with_viewport(BasicAlertDialogSample, IntSize::new(400, 800)), "Confirm"),
    ];
    for (make, closer) in samples {
        let rule = make();
        assert_eq!(rule.host().overlay_count(), 1);
        rule.on_node_with_text(closer).perform_click();
        rule.wait_for_idle();
        assert_eq!(rule.host().overlay_count(), 0);
        rule.on_node_with_text("Open dialog").perform_click();
        rule.wait_for_idle();
        assert_eq!(rule.host().overlay_count(), 1);
    }
}

// ==== sheets =========================================================================================================
// No kompozable_tests.cpp source composes a sheet: the sheets' API as Compose writes it (a modal sheet of a state that
// skips half way, its properties, drag handle and scrim color, hidden from code; a scaffold of a standard state that may
// hide, without a drag handle, a top bar, its state's sheet and snackbar host), and material3's BottomSheetSamples and
// DragHandleSamples from kt/sheets.kt.

kompoz! {
    @OptIn(ExperimentalMaterial3Api::class)
    @Kompozable
    fun SheetForms() {
        var open by remember { mutableStateOf(true) }
        var closed by remember { mutableStateOf(0) }
        val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
        val scope = rememberCoroutineScope()
        Column {
            Text("kapandı: $closed")
            Button(onClick = { open = true }) { Text("Aç") }
        }
        if (open) {
            ModalBottomSheet(
                onDismissRequest = { open = false; closed++ },
                sheetState = sheetState,
                scrimColor = BottomSheetDefaults.ScrimColor,
                properties = ModalBottomSheetProperties(shouldDismissOnClickOutside = false),
                dragHandle = { BottomSheetDefaults.DragHandle(color = Color.Red) },
            ) {
                Text("Sayfa: ${sheetState.currentValue}")
                Button(onClick = { scope.launch { sheetState.hide(); open = false } }) { Text("Gizle") }
            }
        }
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Kompozable
    fun ScaffoldForms() {
        val scaffoldState = rememberBottomSheetScaffoldState(bottomSheetState = rememberStandardBottomSheetState(skipHiddenState = false))
        val scope = rememberCoroutineScope()
        BottomSheetScaffold(
            sheetContent = { Box(Modifier.fillMaxWidth().height(200.dp)) { Text("Ayrıntılar") } },
            scaffoldState = scaffoldState,
            sheetPeekHeight = 80.dp,
            sheetDragHandle = null,
            topBar = { Text("Başlık") },
        ) { padding ->
            Column(Modifier.padding(padding)) {
                Text("Değer: ${scaffoldState.bottomSheetState.currentValue}")
                Button(onClick = { scope.launch { scaffoldState.bottomSheetState.expand() } }) { Text("Genişlet") }
                Button(onClick = { scope.launch { scaffoldState.snackbarHostState.showSnackbar("Merhaba") } }) { Text("Bildir") }
            }
        }
    }
}

fn sheets_as_compose_writes_them() {
    let rule = KompozTestRule::new(SheetForms);
    rule.wait_for_idle();
    // Skipping half way: expanded at once; its drag handle in the color given.
    rule.on_node_with_text("Sayfa: Expanded").assert_exists();
    let canvas = rule.draw();
    let red = kompoz::graphics::Color::Red;
    assert!(
        canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| r.width() == 32.0 && r.height() == 4.0 && *c == red)
    );
    // A tap on the scrim doesn't dismiss it (shouldDismissOnClickOutside = false: the scrim takes no clicks).
    rule.on_node_with_content_description("Close sheet").assert_does_not_exist();
    rule.host().pointer_down(kompoz::graphics::Offset::new(200.0, 100.0), -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.host().pointer_up(kompoz::graphics::Offset::new(200.0, 100.0), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("Sayfa: Expanded").assert_exists();
    // Hidden from code, then taken away: not dismissed.
    rule.on_node_with_text("Gizle").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Gizle").assert_does_not_exist();
    rule.on_node_with_text("kapandı: 0").assert_exists();
    rule.on_node_with_text("Aç").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Sayfa: Expanded").assert_exists();

    let rule = KompozTestRule::new(ScaffoldForms);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_text("Başlık").get_unclipped_bounds_in_root().top, 0.0);
    rule.on_node_with_text("Değer: PartiallyExpanded").assert_exists();
    // No drag handle: the sheet's content right at its top, 80 peeking out.
    let details = rule.on_node_with_text("Ayrıntılar").get_unclipped_bounds_in_root();
    assert!((details.top - 720.0).abs() < 0.5, "{details:?}");
    rule.on_node_with_text("Genişlet").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Değer: Expanded").assert_exists();
    rule.on_node_with_text("Bildir").perform_click();
    rule.wait_for_idle_within(1000);
    rule.on_node_with_text("Merhaba").assert_exists();
}

kompoz_file!("tests/kt/sheets.kt");

fn bottom_sheet_samples() {
    let rule = KompozTestRule::new(ManualModalBottomSheetSample);
    rule.on_node_with_text("Show Manual Sheet").perform_click();
    rule.wait_for_idle();
    let item = rule.on_node_with_text("Item 0").get_unclipped_bounds_in_root();
    assert!(item.top > 400.0 && item.top < 800.0, "{item:?}");
    rule.on_node_with_content_description("scrim")
        .perform_semantics_action(kompoz::ui::semantics::SemanticsActions::OnClick);
    rule.wait_for_idle();
    rule.on_node_with_text("Item 0").assert_does_not_exist();

    // ModalBottomSheetSample: shown half way up (25 items fill the window), a text field typed into, hidden by its
    // button (the hide awaited, then the flag cleared: `launch { … }.invokeOnCompletion { … }`); the row's Checkbox
    // checked, it skips the half way state, shown all the way up.
    let rule = KompozTestRule::new(ModalBottomSheetSample);
    rule.on_node_with_text("Show Bottom Sheet").perform_click();
    rule.wait_for_idle();
    let hide = rule.on_node_with_text("Hide Bottom Sheet").get_unclipped_bounds_in_root();
    assert!(hide.top > 300.0 && hide.top < 500.0, "{hide:?}");
    rule.on_node_with_text("Text field").perform_text_input("merhaba");
    rule.wait_for_idle();
    rule.on_node_with_text("merhaba").assert_exists();
    rule.on_node_with_text("Hide Bottom Sheet").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Hide Bottom Sheet").assert_does_not_exist();
    rule.on_node_with_text("Item 0").assert_does_not_exist();
    // (the toggleable row, its texts merged into it)
    rule.on_node_with_text("Skip partially expanded State").assert_is_off().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Skip partially expanded State").assert_is_on();
    rule.on_node_with_text("Show Bottom Sheet").perform_click();
    rule.wait_for_idle();
    let hide = rule.on_node_with_text("Hide Bottom Sheet").get_unclipped_bounds_in_root();
    assert!(hide.top < 100.0, "{hide:?}");

    let rule = KompozTestRule::new(SimpleBottomSheetScaffoldSample);
    rule.wait_for_idle();
    let content = rule.on_node_with_text("Scaffold Content").get_unclipped_bounds_in_root();
    assert!((content.center().y - (800.0 - 128.0) / 2.0).abs() < 0.5, "{content:?}");
    let swipe_box = rule.on_node_with_text("Swipe up to expand sheet").get_unclipped_bounds_in_root();
    assert!(swipe_box.center().y > 800.0 - 80.0, "{swipe_box:?}");

    // BottomSheetScaffoldNestedScrollSample: the pinned bar takes its scrolled color as the body's list scrolls
    // under it (its nested scroll reaching the scaffold's connection); the sheet peeks 128 dp.
    let rule = KompozTestRule::with_viewport(BottomSheetScaffoldNestedScrollSample, IntSize::new(400, 800));
    rule.wait_for_idle();
    let item = rule.on_node_with_text("Item 0").get_unclipped_bounds_in_root();
    assert!(item.top > 800.0 - 128.0 && item.top < 800.0, "{item:?}");
    let scrolled = light_color_scheme().surface_container;
    assert!(!rule.draw().rect_colors.contains(&scrolled));
    swipe_pull_swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 200.0), 10);
    assert!(rule.draw().rect_colors.contains(&scrolled));
}

fn drag_handle_samples() {
    let rule = KompozTestRule::new(VerticalDragHandleSample);
    rule.wait_for_idle();
    let outline = light_color_scheme().outline;
    let canvas = rule.draw();
    // The handle at the end of a pane half the window wide, in its 48 dp room.
    let handle = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .find(|(r, c)| r.width() == 4.0 && r.height() == 48.0 && **c == outline);
    assert!((handle.expect("drawn").0.left - (200.0 - 48.0 + 22.0)).abs() < 0.5);
    assert!(!rule.host().system_gesture_exclusion().is_empty());
}

// ==== snackbars and tooltips =========================================================================================

// ---- material_as_compose_writes_it (kompozable_tests.cpp `MaterialForms`): its chip, its toggleable row with a
// Checkbox, its snackbar awaited with an action label and its progress bar (what the C++ asserted of them; the
// progress line's 40% is kotlin_progress.rs' `material_forms_progress`); its NavigationSuiteScaffold (WP8's) comes
// with that package.

kompoz! {
    @Kompozable
    fun MaterialFormsSnackbar(undo: () -> Unit) {
        var unread by remember { mutableStateOf(false) }
        var muted by remember { mutableStateOf(false) }
        val snackbars = remember { SnackbarHostState() }
        val scope = rememberCoroutineScope()
        Column {
            FilterChip(selected = unread, onClick = { unread = !unread }, label = { Text("Unread") }, modifier = Modifier.testTag("chip"))
            Row(modifier = Modifier.toggleable(value = muted, onValueChange = { muted = it }, role = Role.Checkbox).testTag("row")) {
                Checkbox(checked = muted, onCheckedChange = null)
                Text("Mute")
            }
            Button(onClick = {
                scope.launch {
                    if (snackbars.showSnackbar("Deleted", actionLabel = "Undo") == SnackbarResult.ActionPerformed) undo()
                }
            }) { Text("Delete") }
            SnackbarHost(hostState = snackbars)
            LinearProgressIndicator(progress = { 0.4f }, modifier = Modifier.testTag("bar"))
        }
    }
}

fn material_forms_snackbar() {
    let undone = std::rc::Rc::new(std::cell::Cell::new(0));
    let u = undone.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let u = u.clone();
        MaterialFormsSnackbar(move || u.set(u.get() + 1))
    }));
    rule.on_node_with_tag("chip").assert_is_not_selected().perform_click();
    rule.on_node_with_tag("chip").assert_is_selected();
    // (the row toggles its Checkbox, which takes no clicks of its own)
    rule.on_node_with_tag("row").assert_is_off().perform_click();
    rule.on_node_with_tag("row").assert_is_on();
    let checked = light_color_scheme().primary; // (the checked box's fill)
    assert!(rule.draw().path_colors.contains(&checked) || rule.draw().rect_colors.contains(&checked));
    rule.on_node_with_tag("bar").assert_exists();
    rule.on_node_with_text("Delete").perform_click();
    rule.on_node_with_text("Deleted").assert_exists();
    rule.on_node_with_text("Undo").perform_click();
    assert_eq!(undone.get(), 1);
    rule.on_node_with_text("Deleted").assert_does_not_exist();
}

// ---- sections_forms_take_compose_named_arguments (kompozable_tests.cpp `SectionsForms`): its TooltipBox; its search
// bar and RichTooltip are in kotlin_search_carousel.rs, its date picker dialog, swipe to dismiss box and focus rows
// come with their packages. (The C++ gave
// its PlainTooltip `caretSize = TooltipDefaults.caretSize`: M15 takes `caretShape = TooltipDefaults.caretShape()`; its
// `rememberTooltipPositionProvider()` is deprecated in M15 for the anchor-position form it names, written so here.)

kompoz! {
    @Kompozable
    fun SectionsFormsTooltip() {
        var open by remember { mutableStateOf(false) }
        Column(modifier = Modifier.padding(top = 300.dp)) {
            TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above), tooltip = { PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("İpucu") } },
                       state = rememberTooltipState()) {
                IconButton(onClick = { open = true }, modifier = Modifier.testTag("aç")) { Icon(Icons.Filled.Info, contentDescription = "Bilgi") }
            }
            Text(if (open) "açık" else "kapalı")
        }
    }
}

fn sections_forms_tooltip() {
    let rule = KompozTestRule::new(SectionsFormsTooltip);
    let anchor = rule.on_node_with_tag("aç").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(anchor, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(anchor, -1, 0);
    rule.advance_frame();
    rule.on_node_with_text("İpucu").assert_exists();
    rule.on_node_with_text("kapalı").assert_exists(); // (the long press didn't click)
    rule.wait_for_idle();
    rule.on_node_with_tag("aç").perform_click();
    rule.on_node_with_text("açık").assert_exists();
}

kompoz_file!("tests/kt/snackbars_tooltips.kt");

fn snackbar_samples() {
    // (ScaffoldWithSimpleSnackbar, …Indefinite…, …Multiline…, each shown by its button; a snapshot flow's collector)
    let rule = KompozTestRule::new(ScaffoldWithSimpleSnackbar);
    rule.on_node_with_text("Body content").assert_exists();
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    // (the scaffold's: the button at the bottom end, 16 dp in; the snackbar above it)
    let (d, root) = (rule.density().density, rule.on_root().get_unclipped_bounds_in_root());
    let fab = rule.on_node_with_text("Show snackbar").get_unclipped_bounds_in_root();
    assert!((fab.right - (root.right - 16.0 * d)).abs() < 0.5 && (fab.bottom - (root.bottom - 16.0 * d)).abs() < 0.5);
    let snackbar = rule.on_node_with_text("Snackbar # 1").get_unclipped_bounds_in_root();
    assert!(snackbar.bottom < fab.top, "{snackbar:?} {fab:?}");
    rule.advance_time_by(4500);
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_does_not_exist();

    let rule = KompozTestRule::new(ScaffoldWithIndefiniteSnackbar);
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.advance_time_by(15000);
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    rule.on_node_with_text("Action").assert_exists();
    rule.on_node_with_content_description("Dismiss").perform_click();
    rule.on_node_with_text("Snackbar # 1").assert_does_not_exist();

    // (a flow collected with a suspending collector: each count shown in turn; its action resets the count)
    let rule = KompozTestRule::new(SnackbarForEachCount);
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    rule.on_node_with_text("Action on 1").perform_click();
    rule.on_node_with_text("Snackbar # 1").assert_does_not_exist();
    rule.on_node_with_text("Snackbar demo: 0").assert_exists();

    let rule = KompozTestRule::new(ScaffoldWithMultilineSnackbar);
    rule.on_node_with_text("Show snackbar").perform_click();
    // (two lines at most: 68 dp)
    let height = rule.on_node_with_tag("multiline").get_unclipped_bounds_in_root().height();
    assert!((height - 68.0 * rule.density().density).abs() < 0.5, "{height}");
}

fn plain_tooltip_samples() {
    // (each shown by a long press on its anchor; the manual one by its button)
    let samples: [fn() -> KompozTestRule; 8] = [
        || KompozTestRule::new(PlainTooltipSample),
        || KompozTestRule::new(PlainTooltipWithCaret),
        || KompozTestRule::new(PlainTooltipWithCaretBelowAnchor),
        || KompozTestRule::new(PlainTooltipWithCaretLeftOfAnchor),
        || KompozTestRule::new(PlainTooltipWithCaretRightOfAnchor),
        || KompozTestRule::new(PlainTooltipWithCaretStartOfAnchor),
        || KompozTestRule::new(PlainTooltipWithCaretEndOfAnchor),
        || KompozTestRule::new(PlainTooltipWithCustomCaret),
    ];
    for make in samples {
        let rule = make();
        let anchor = rule.on_node_with_content_description("Localized Description").get_unclipped_bounds_in_root();
        rule.host().pointer_down(anchor.center(), -1, 0, kompoz::ui::input::PointerType::Touch);
        rule.advance_time_by(600);
        rule.host().pointer_up(anchor.center(), -1, 0);
        rule.advance_frame();
        rule.on_node_with_text("Add to favorites").assert_exists();
        assert_eq!(rule.host().overlay_count(), 1);
        rule.advance_time_by(1600);
        rule.wait_for_idle();
        assert_eq!(rule.host().overlay_count(), 0);
    }
    let rule = KompozTestRule::new(PlainTooltipWithManualInvocationSample);
    // (a click of the pointer: `perform_click` waits until idle, the tooltip's 1.5 s gone with it)
    let button = rule.on_node_with_text("Display tooltip").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(button, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.host().pointer_up(button, -1, 0);
    rule.advance_time_by(100);
    rule.on_node_with_text("Add to list").assert_exists();
}

// ==== swipe to dismiss and pull to refresh ===========================================================================
// The C++ Kompoz's `sections_forms_take_compose_named_arguments` (kompozable_tests.cpp `SectionsForms`): its swipe to
// dismiss boxes (`rememberSwipeToDismissBoxState(confirmValueChange =)`: M15's deprecated overload); material3's
// SwipeToDismissSamples and the rest of the swipe-to-dismiss and pull-to-refresh API from kt/swipe_pull.kt. (The C++
// ScrollForms' pull to refresh is in named_forms.rs.)

kompoz! {
    @Kompozable
    fun SectionsFormsSwipe() {
        val tasks = remember { mutableStateListOf(1, 2) }
        Column {
            for (id in tasks) {
                key(id) {
                    val state = rememberSwipeToDismissBoxState(confirmValueChange = { tasks.remove(id); true })
                    SwipeToDismissBox(state = state, backgroundContent = { Box(modifier = Modifier.fillMaxSize().background(Color.Red)) }) {
                        Text("görev $id", modifier = Modifier.fillMaxWidth().testTag("görev$id"))
                    }
                }
            }
            Text("görevler ${tasks.size}")
        }
    }
}

/// A move each frame from `from` to `to` over `frames` frames, then up (a fling at the speed of the moves).
fn swipe_pull_swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.wait_for_idle();
}

fn sections_forms_swipe() {
    let rule = KompozTestRule::with_viewport(SectionsFormsSwipe, IntSize::new(400, 800));
    rule.on_node_with_tag("görev1").assert_exists();
    rule.on_node_with_text("görevler 2").assert_exists();
    // Swiped away: asked to confirm, the task removed (its box with it).
    let y = rule.on_node_with_tag("görev1").get_unclipped_bounds_in_root().center().y;
    swipe_pull_swipe(&rule, Offset::new(350.0, y), Offset::new(50.0, y), 20);
    rule.on_node_with_tag("görev1").assert_does_not_exist();
    rule.on_node_with_tag("görev2").assert_exists();
    rule.on_node_with_text("görevler 1").assert_exists();
}

kompoz_file!("tests/kt/swipe_pull.kt");

fn swipe_to_dismiss_samples() {
    let rule = KompozTestRule::with_viewport(SwipeToDismissListItems, IntSize::new(400, 800));
    let y = rule.on_node_with_text("Cupcake").get_unclipped_bounds_in_root().center().y;
    // Swiped right: reset back. Swiped left: gone.
    swipe_pull_swipe(&rule, Offset::new(50.0, y), Offset::new(350.0, y), 20);
    assert!(rule.on_node_with_text("Cupcake").get_unclipped_bounds_in_root().left < 1.0);
    swipe_pull_swipe(&rule, Offset::new(350.0, y), Offset::new(50.0, y), 20);
    rule.on_node_with_text("Cupcake").assert_does_not_exist();
}

fn swipe_forms() {
    let rule = KompozTestRule::with_viewport(SwipeForms, IntSize::new(400, 800));
    // The second state starts dismissed (its constructor's initial value); the first in place.
    rule.on_node_with_text("günlük: · ikinci gitti · 100").assert_exists();
    rule.on_node_with_text("eşik 56").assert_exists(); // (the defaults' 56 dp, whatever the distance)
    assert!(rule.on_node_with_tag("ikinci").get_unclipped_bounds_in_root().right <= 1.0);
    // Not from the start to the end: dragged right, the first stays.
    swipe_pull_swipe(&rule, Offset::new(50.0, 30.0), Offset::new(350.0, 30.0), 20);
    rule.on_node_with_text("günlük: · ikinci gitti · 100").assert_exists();
    // Dragged left slowly 150 (142 past the slop): short of its threshold, half the 400 between its anchors.
    let h = rule.host();
    h.pointer_down(Offset::new(350.0, 30.0), -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=60 {
        rule.advance_frame();
        h.pointer_move(Offset::new(350.0 - 2.5 * i as f32, 30.0), -1, 0);
    }
    rule.advance_frame();
    rule.on_node_with_text("sola").assert_exists(); // (the direction it is being dismissed in, behind it)
    rule.advance_time_by(200);
    h.pointer_up(Offset::new(200.0, 30.0), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("günlük: · ikinci gitti · 100").assert_exists();
    // On to 250: dismissed to the start, told.
    swipe_pull_swipe(&rule, Offset::new(350.0, 30.0), Offset::new(50.0, 30.0), 20);
    rule.on_node_with_text("günlük: sola · ikinci gitti · 100").assert_exists();
    // From code: the second back, the first put back in place (without an animation).
    rule.on_node_with_text("geri getir").perform_click();
    rule.on_node_with_text("yerine koy").perform_click();
    rule.on_node_with_text("günlük: sola · ikinci yerinde · 100").assert_exists();
    assert!(rule.on_node_with_tag("ikinci").get_unclipped_bounds_in_root().left.abs() < 1.0);
}

fn pull_forms() {
    let rule = KompozTestRule::with_viewport(PullForms, IntSize::new(400, 800));
    rule.on_node_with_text("yenileme 0 · 0 · 0 · false").assert_exists();
    // Pulled 300 down slowly (292 past the slop: 146, past the 80 dp threshold): refreshing, the indicator at the
    // threshold.
    let h = rule.host();
    h.pointer_down(Offset::new(200.0, 50.0), -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=30 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 50.0 + 10.0 * i as f32), -1, 0);
    }
    rule.advance_frame();
    // (the indicator's content out with it, under the box's top edge it came from)
    assert!(rule.on_node_with_tag("gösterge").get_unclipped_bounds_in_root().top > 0.0);
    rule.advance_time_by(200);
    h.pointer_up(Offset::new(200.0, 350.0), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("yenileme 1 · 100 · 0 · false").assert_exists();
    // (the default Indicator spinning: an endless progress)
    assert!(endless_progress_shown(&rule));
    // Done: hidden again.
    rule.on_node_with_text("bitti").perform_click();
    rule.on_node_with_text("yenileme 1 · 0 · 0 · false").assert_exists();
    assert!(!endless_progress_shown(&rule));
    // The other state, moved from code.
    rule.on_node_with_text("öbürü").perform_click();
    rule.on_node_with_text("yenileme 1 · 0 · 100 · false").assert_exists();
    // The PullToRefreshBox: 160 down on it (152 past the slop: 76, past its 60 dp threshold) refreshes it; its
    // LoadingIndicator (48 dp, at the top center) spins the endless one.
    rule.on_node_with_text("kutu: false").assert_exists();
    let top = rule.on_node_with_tag("kutu").get_unclipped_bounds_in_root().top_center() + Offset::new(0.0, 10.0);
    h.pointer_down(top, -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(top + Offset::new(0.0, 8.0 * i as f32), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(top + Offset::new(0.0, 160.0), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("kutu: true").assert_exists();
    let indicator = rule.on_node_with_tag("yükleme").get_unclipped_bounds_in_root();
    let place = rule.on_node_with_tag("kutu").get_unclipped_bounds_in_root();
    assert!(
        (indicator.width() - 48.0).abs() < 0.5 && (indicator.center().x - place.center().x).abs() < 0.5,
        "{indicator:?}"
    );
    assert!(endless_progress_shown(&rule));
}

/// The pull-to-refresh samples' first item shown, if any (under the 64 dp bar).
fn pull_sample_first_item(rule: &KompozTestRule) -> Option<String> {
    use kompoz::ui::semantics::SemanticsProperties;
    let mut shown: Vec<(f32, String)> = rule
        .all_nodes(true)
        .iter()
        .filter_map(|n| {
            let text = n.config().get(SemanticsProperties::Text)?.first()?.text().to_string();
            let b = n.bounds_in_root();
            (text.starts_with("Item ") && b.height() > 0.0).then_some((b.top, text))
        })
        .collect();
    shown.sort_by(|a, b| a.0.total_cmp(&b.0));
    shown.into_iter().next().map(|(top, text)| {
        assert!(top >= 64.0 * rule.density().density, "{text} at {top}");
        text
    })
}

/// A sample refreshed by a pull past the threshold (5 more items after `ms`, an endless indicator meanwhile) and by
/// its bar's action. `hides_list`: the list is gone while refreshing.
fn pull_sample_refreshes(rule: &KompozTestRule, ms: i64, hides_list: bool) {
    assert_eq!(pull_sample_first_item(rule).as_deref(), Some("Item 15"));
    let h = rule.host();
    let (from, to) = (Offset::new(200.0, 150.0), Offset::new(200.0, 450.0));
    h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / 20.0), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(to, -1, 0);
    rule.advance_time_by(500);
    assert!(endless_progress_shown(rule));
    assert_eq!(pull_sample_first_item(rule).is_none(), hides_list);
    rule.advance_time_by(ms);
    rule.wait_for_idle();
    assert_eq!(pull_sample_first_item(rule).as_deref(), Some("Item 20"));
    assert!(!endless_progress_shown(rule));
    // (a click of the pointer: `perform_click` waits until idle, the indicator's animations taking the fetch's time)
    let action = rule.on_node_with_content_description("Trigger Refresh").get_unclipped_bounds_in_root().center();
    h.pointer_down(action, -1, 0, kompoz::ui::input::PointerType::Touch);
    h.pointer_up(action, -1, 0);
    rule.advance_time_by(500);
    assert!(endless_progress_shown(rule));
    rule.advance_time_by(ms);
    rule.wait_for_idle();
    assert_eq!(pull_sample_first_item(rule).as_deref(), Some("Item 25"));
}

fn pull_to_refresh_samples() {
    let window = IntSize::new(400, 800);
    pull_sample_refreshes(&KompozTestRule::with_viewport(PullToRefreshSample, window), 5000, false);
    pull_sample_refreshes(&KompozTestRule::with_viewport(PullToRefreshWithLoadingIndicatorSample, window), 5000, false);
    pull_sample_refreshes(&KompozTestRule::with_viewport(PullToRefreshScalingSample, window), 5000, true);
    pull_sample_refreshes(
        &KompozTestRule::with_viewport(PullToRefreshLinearProgressIndicatorSample, window),
        5000,
        true,
    );
    pull_sample_refreshes(
        &KompozTestRule::with_viewport(PullToRefreshCustomIndicatorWithDefaultTransform, window),
        1500,
        false,
    );
    pull_sample_refreshes(&KompozTestRule::with_viewport(LoadingIndicatorPullToRefreshSample, window), 5000, true);
}

/// Whether an endless progress is shown to accessibility (a refreshing indicator's).
fn endless_progress_shown(rule: &KompozTestRule) -> bool {
    use kompoz::ui::semantics::{ProgressBarRangeInfo, SemanticsProperties};
    rule.all_nodes(true)
        .iter()
        .any(|n| n.config().get(SemanticsProperties::ProgressBarRangeInfo) == Some(ProgressBarRangeInfo::Indeterminate))
}

fn main() {
    common::run(&[
        // (lists)
        ("list_items_by_name", list_items_by_name),
        ("kotlin_helpers_list_item", kotlin_helpers_list_item),
        ("list_item_samples", list_item_samples),
        ("clickable_list_item_samples", clickable_list_item_samples),
        // (WP4b: icons)
        ("segmented_list_item_with_expansion_sample", segmented_list_item_with_expansion_sample),
        ("selection_list_item_samples", selection_list_item_samples),
        ("list_item_with_mode_change_on_long_click_sample", list_item_with_mode_change_on_long_click_sample),
        ("list_item_forms", list_item_forms),
        // (tabs and dialogs)
        ("tabs_as_compose_writes_them", tabs_as_compose_writes_them),
        ("indicators_as_composable_extensions", indicators_as_composable_extensions),
        ("the_users_add_text_dialog", the_users_add_text_dialog),
        ("tab_samples", tab_samples),
        ("alert_dialog_samples", alert_dialog_samples),
        // (sheets)
        ("sheets_as_compose_writes_them", sheets_as_compose_writes_them),
        ("bottom_sheet_samples", bottom_sheet_samples),
        ("drag_handle_samples", drag_handle_samples),
        // (snackbars and tooltips)
        ("material_forms_snackbar", material_forms_snackbar),
        ("sections_forms_tooltip", sections_forms_tooltip),
        ("snackbar_samples", snackbar_samples),
        ("plain_tooltip_samples", plain_tooltip_samples),
        // (swipe to dismiss and pull to refresh)
        ("sections_forms_swipe", sections_forms_swipe),
        ("swipe_to_dismiss_samples", swipe_to_dismiss_samples),
        ("swipe_forms", swipe_forms),
        ("pull_forms", pull_forms),
        ("pull_to_refresh_samples", pull_to_refresh_samples),
    ]);
}
