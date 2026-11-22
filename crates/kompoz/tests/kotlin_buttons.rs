//! Material's buttons and containers in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's
//! `required_arguments_by_name` (kompozable_tests.cpp 201-226: all of its NamedHeads, the Checkbox, TextButton,
//! TextField, Icon and texts) and `containers_as_compose_writes_them` (2799-2848: its buttons, lazy list, badges,
//! segmented list items, extended FAB and bottom app bar; its LoadingIndicator in kotlin_progress.rs) through
//! `kompoz!`, and
//! material3's samples of buttons, icon buttons, FABs, cards and chips from a `.kt` file (kompoz_file!).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
// (what the generated code types the animated FAB samples' tooltip position providers with)
use kompoz::ui::window::PopupPositionProvider;

// ---- required_arguments_by_name (kompozable_tests.cpp 201-226) -----------------------------------------------------

kompoz! {
    // The arguments a composable needs (onClick, value, checked, ...) by their names too.
    @Kompozable
    fun NamedHeads() {
        var on by remember { mutableStateOf(false) }
        var text by remember { mutableStateOf("") }
        Column {
            Checkbox(checked = on, onCheckedChange = { on = it }, modifier = Modifier.testTag("kutu"))
            TextButton(onClick = { text = "basıldı" }) { Text(text = "düğme") }
            TextField(value = text, onValueChange = { text = it }, modifier = Modifier.testTag("alan"))
            Icon(imageVector = Icons.Filled.Star, contentDescription = "yıldız")
            Text(text = if (on) "açık" else "kapalı")
            Text("değer: $text")
        }
    }
}

fn required_arguments_by_name() {
    let rule = KompozTestRule::new(NamedHeads);
    rule.on_node_with_text("kapalı").assert_exists();
    rule.on_node_with_text("değer: ").assert_exists();
    rule.on_node_with_content_description("yıldız").assert_exists();
    rule.on_node_with_text("düğme").perform_click();
    rule.on_node_with_text("değer: basıldı").assert_exists();
    // Material's TextField of a String value: what is typed goes in at its cursor. The value set from outside (the
    // button's) keeps the field's selection, at 0 (Compose's legacy BasicTextField: `textFieldValueState.copy(text =
    // value)`), and the test rule's text input inserts at the cursor: "!basıldı" (the C++ appended: "basıldı!").
    rule.on_node_with_tag("alan").perform_text_input("!");
    rule.on_node_with_text("değer: !basıldı").assert_exists();
    // The Checkbox: a click checks it.
    rule.on_node_with_tag("kutu").assert_is_off().perform_click();
    rule.on_node_with_tag("kutu").assert_is_on();
    rule.on_node_with_text("açık").assert_exists();
}

// ---- containers_as_compose_writes_them (kompozable_tests.cpp 2799-2848) --------------------------------------------

kompoz! {
    @Kompozable
    fun Containers() {
        val tasks = remember { mutableStateListOf(1, 2, 3) }
        var plan by remember { mutableStateOf(0) }
        var open by remember { mutableStateOf(true) }
        Column {
            Row {
                FilledTonalButton(onClick = { tasks.shuffle(Random(7)) }) { Text("Karıştır") }
                FilledTonalButton(onClick = { tasks.clear(); tasks.addAll(listOf(4, 5)) }) { Text("Doldur") }
            }
            LazyColumn(modifier = Modifier.height(200.dp)) {
                items(tasks, key = { it }) { Text("görev $it") }
            }
            Text("sıra: ${tasks.joinToString(",")}")
            BadgedBox(badge = { Badge { Text("8") } }) { Icon(Icons.Filled.Email, contentDescription = "Posta") }
            BadgedBox(badge = { Badge(modifier = Modifier.testTag("nokta")) }) { Icon(Icons.Filled.Notifications, contentDescription = "Bildirim") }
            for (i in 0 until 2) {
                // (the C++ gave its headline as `headlineContent = { Text("plan $i") }`: M15's SegmentedListItem takes
                // it as its `content`, the trailing block)
                SegmentedListItem(selected = plan == i, onClick = { plan = i }, shapes = ListItemDefaults.segmentedShapes(i, 2),
                                  supportingContent = { Text(if (plan == i) "seçili" else "") }) { Text("plan $i") }
            }
            SmallExtendedFloatingActionButton(onClick = { open = !open }, expanded = open, icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                                              text = { Text("Yeni") }, modifier = Modifier.testTag("fab"))
            BottomAppBar(actions = { IconButton(onClick = { }) { Icon(Icons.Filled.Check, contentDescription = "Tamam") } },
                         windowInsets = WindowInsets(0.dp), modifier = Modifier.testTag("çubuk"))
        }
    }
}

fn containers_as_compose_writes_them() {
    let rule = KompozTestRule::new(Containers);
    rule.on_node_with_text("sıra: 1,2,3").assert_exists();
    rule.on_node_with_text("görev 2").assert_exists();
    rule.on_node_with_text("Karıştır").perform_click();
    let node = named_args!(rule.on_node_with_text("sıra: ", substring = true)).fetch_semantics_node();
    let texts = node.config().get(kompoz::ui::semantics::SemanticsProperties::Text).unwrap_or_default();
    let order: String = texts.iter().map(|t| t.text().to_string()).collect();
    assert!(order != "sıra: 1,2,3" && order.len() == "sıra: 1,2,3".len(), "{order}");
    rule.on_node_with_text("Doldur").perform_click();
    rule.on_node_with_text("sıra: 4,5").assert_exists();
    rule.on_node_with_text("görev 5").assert_exists();
    rule.on_node_with_text("görev 1").assert_does_not_exist();
    rule.on_node_with_text("8").assert_exists();
    rule.on_node_with_tag("nokta").assert_exists();
    rule.on_node_with_text("plan 1").perform_click();
    rule.on_node_with_text("seçili").assert_exists();
    let wide = rule.on_node_with_tag("fab").get_unclipped_bounds_in_root();
    rule.on_node_with_tag("fab").perform_click();
    rule.main_clock().advance_time_by(2000);
    assert!(rule.on_node_with_tag("fab").get_unclipped_bounds_in_root().width() < wide.width());
    // (collapsed, its label left with its exit animation, as Compose's AnimatedVisibility takes it out; the C++ kept
    // it composed)
    rule.on_node_with_text("Yeni").assert_does_not_exist();
    // The bottom app bar: no inset padding (its window insets none), 80 dp; its action 4 dp in.
    let bar = rule.on_node_with_tag("çubuk").get_unclipped_bounds_in_root();
    assert!(bar.height() <= 80.5 && (bar.height() - 80.0).abs() <= 0.5, "{bar:?}");
    let action = rule.on_node_with_content_description("Tamam").get_unclipped_bounds_in_root();
    assert!((action.center().x - (bar.left + 4.0 + 24.0)).abs() <= 0.5, "{action:?}");
}

// ---- material3's samples (kt/buttons.kt) ----------------------------------------------------------------------------

kompoz_file!("tests/kt/buttons.kt");

/// The samples in a MaterialTheme, in a column that scrolls.
fn themed(content: impl Fn() + 'static) -> KompozTestRule {
    let content = std::rc::Rc::new(content);
    KompozTestRule::new(kompozable_lambda!(move || {
        let content = content.clone();
        MaterialTheme(
            content = move || {
                let content = content.clone();
                Column(modifier = Modifier.vertical_scroll(&remember_scroll_state()), move || content());
            },
        );
    }))
}

fn button_samples() {
    let rule = themed(kompozable_lambda!(|| {
        ButtonSample();
        ButtonWithAnimatedShapeSample();
        SquareButtonSample();
        SmallButtonSample();
        ElevatedButtonSample();
        ElevatedButtonWithAnimatedShapeSample();
        FilledTonalButtonSample();
        FilledTonalButtonWithAnimatedShapeSample();
        OutlinedButtonSample();
        OutlinedButtonWithAnimatedShapeSample();
        TextButtonSample();
        TextButtonWithAnimatedShapeSample();
    }));
    let density = rule.density().density;
    // (each a button of the minimum height, 40 dp: its node the clickable container, in 48 dp of room)
    for b in rule.on_all_nodes_with(kompoz::test::has_click_action(), false).fetch_semantics_nodes() {
        assert!((b.unclipped_bounds_in_root().height() - 40.0 * density).abs() < 0.5, "{b:?}");
    }
    assert_eq!(rule.on_all_nodes_with(kompoz::test::has_click_action(), false).fetch_semantics_nodes().len(), 12);
    // (each kind twice: with its shape and with the animated shapes)
    for label in ["Elevated Button", "Filled Tonal Button", "Outlined Button"] {
        rule.on_all_nodes_with_text(label).assert_count_equals(2);
    }
    let canvas = rule.draw();
    let scheme = light_color_scheme();
    assert!(canvas.rect_colors.contains(&scheme.primary) || canvas.path_colors.contains(&scheme.primary));
}

fn buttons_with_icons_come_in_sizes() {
    let rule = themed(kompozable_lambda!(|| {
        ButtonWithIconSample();
        XSmallButtonWithIconSample();
        MediumButtonWithIconSample();
        LargeButtonWithIconSample();
        XLargeButtonWithIconSample();
    }));
    let density = rule.density().density;
    // (the sizes' heights: 40 (min), 32 (extra small), 56, 96, 136 dp: the containers, the smaller two in 48 dp of
    // room)
    let heights: Vec<f32> = rule
        .on_all_nodes_with(kompoz::test::has_click_action(), false)
        .fetch_semantics_nodes()
        .iter()
        .map(|n| (n.unclipped_bounds_in_root().height() / density).round())
        .collect();
    assert_eq!(heights, [40.0, 32.0, 56.0, 96.0, 136.0]);
    assert_eq!(rule.on_all_nodes_with_content_description("Localized description").fetch_semantics_nodes().len(), 5);
}

fn icon_button_samples() {
    let rule = themed(kompozable_lambda!(|| {
        IconButtonSample();
        IconButtonWithAnimatedShapeSample();
        ExtraSmallNarrowSquareIconButtonsSample();
        MediumRoundWideIconButtonSample();
        LargeRoundUniformOutlinedIconButtonSample();
        TintedIconButtonSample();
        IconToggleButtonSample();
        IconToggleButtonWithAnimatedShapeSample();
        FilledIconButtonSample();
        FilledIconButtonWithAnimatedShapeSample();
        FilledIconToggleButtonSample();
        FilledIconToggleButtonWithAnimatedShapeSample();
        FilledTonalIconButtonSample();
        FilledTonalIconButtonWithAnimatedShapeSample();
        FilledTonalIconToggleButtonSample();
        FilledTonalIconToggleButtonWithAnimatedShapeSample();
        OutlinedIconButtonSample();
        OutlinedIconButtonWithAnimatedShapeSample();
        OutlinedIconToggleButtonSample();
        OutlinedIconToggleButtonWithAnimatedShapeSample();
    }));
    let toggles = rule.on_all_nodes_with(kompoz::test::is_toggleable(), false);
    assert_eq!(toggles.fetch_semantics_nodes().len(), 8);
    // Each toggle turns on as it is clicked (its icon the filled lock then).
    for i in 0..8 {
        let toggle = rule.on_all_nodes_with(kompoz::test::is_toggleable(), false).get(i);
        toggle.perform_scroll_to();
        let toggle = rule.on_all_nodes_with(kompoz::test::is_toggleable(), false).get(i);
        toggle.assert_is_off();
        toggle.perform_click();
        rule.on_all_nodes_with(kompoz::test::is_toggleable(), false).get(i).assert_is_on();
    }
    // The tinted one's icon red.
    assert!(rule.draw().vector_colors.contains(&Color::Red));
    // (the medium wide one: Compose's medium container of the wide option; the large uniform one the widest)
    let density = rule.density().density;
    let all = rule.on_all_nodes_with(kompoz::test::has_click_action(), false).fetch_semantics_nodes();
    let widths: Vec<f32> = all.iter().map(|n| n.unclipped_bounds_in_root().width() / density).collect();
    let wide =
        named_args!(IconButtonDefaults.medium_container_size(width_option = IconButtonWidthOption::Wide)).width.0;
    let large = IconButtonDefaults.large_container_size().width.0;
    assert!(widths.iter().any(|w| (w - wide).abs() < 0.5), "{widths:?} {wide}");
    assert!(widths.iter().all(|w| *w <= large + 0.5), "{widths:?} {large}");
}

fn fab_samples() {
    let rule = themed(kompozable_lambda!(|| {
        FloatingActionButtonSample();
        SmallFloatingActionButtonSample();
        MediumFloatingActionButtonSample();
        LargeFloatingActionButtonSample();
        ExtendedFloatingActionButtonTextSample();
        SmallExtendedFloatingActionButtonTextSample();
        MediumExtendedFloatingActionButtonTextSample();
        LargeExtendedFloatingActionButtonTextSample();
        ExtendedFloatingActionButtonSample();
        SmallExtendedFloatingActionButtonSample();
        MediumExtendedFloatingActionButtonSample();
        LargeExtendedFloatingActionButtonSample();
    }));
    let density = rule.density().density;
    let sizes: Vec<(f32, f32)> = rule
        .on_all_nodes_with(kompoz::test::has_click_action(), false)
        .fetch_semantics_nodes()
        .iter()
        .map(|n| {
            let b = n.unclipped_bounds_in_root();
            ((b.width() / density).round(), (b.height() / density).round())
        })
        .collect();
    // FAB 56, small 40 (in 48 dp of room), medium 80, large 96 dp squares.
    assert_eq!(&sizes[..4], &[(56.0, 56.0), (40.0, 40.0), (80.0, 80.0), (96.0, 96.0)]);
    // The extended ones as tall as their sizes: 56, 56 (small), 80, 96 — twice.
    let heights: Vec<f32> = sizes[4..].iter().map(|s| s.1).collect();
    assert_eq!(heights, [56.0, 56.0, 80.0, 96.0, 56.0, 56.0, 80.0, 96.0]);
    rule.on_node_with_text("Large Extended FAB").assert_exists();
}

fn card_samples() {
    let rule = themed(kompozable_lambda!(|| {
        CardSample();
        ClickableCardSample();
        ElevatedCardSample();
        ClickableElevatedCardSample();
        OutlinedCardSample();
        ClickableOutlinedCardSample();
    }));
    let density = rule.density().density;
    // (the three clickable ones, 180 by 100 dp, their text merged in)
    let clickable = rule.on_all_nodes_with(kompoz::test::has_click_action(), false).fetch_semantics_nodes();
    assert_eq!(clickable.len(), 3);
    for n in &clickable {
        let b = n.unclipped_bounds_in_root();
        assert!((b.width() - 180.0 * density).abs() < 0.5 && (b.height() - 100.0 * density).abs() < 0.5, "{b:?}");
    }
    assert_eq!(rule.on_all_nodes_with_text("Card content").fetch_semantics_nodes().len(), 3);
    let scheme = light_color_scheme();
    let canvas = rule.draw();
    // (the filled card's surface container highest; the outlined one's outline variant border)
    assert!(
        canvas.rect_colors.contains(&scheme.surface_container_highest)
            || canvas.path_colors.contains(&scheme.surface_container_highest)
    );
}

fn chip_samples() {
    let rule = themed(kompozable_lambda!(|| {
        AssistChipSample();
        ElevatedAssistChipSample();
        FilterChipSample();
        ElevatedFilterChipSample();
        FilterChipWithLeadingIconSample();
        FilterChipWithTrailingIconSample();
        FilterChipWithCustomSpacingSample();
        InputChipSample();
        InputChipWithAvatarSample();
        SuggestionChipSample();
        ElevatedSuggestionChipSample();
    }));
    let selectable = || rule.on_all_nodes_with(kompoz::test::is_selectable(), false);
    // The filter and input chips: 7 selectable, none selected.
    assert_eq!(selectable().fetch_semantics_nodes().len(), 7);
    let done = || rule.on_all_nodes_with_content_description("Localized Description").fetch_semantics_nodes().len();
    // (the trailing icon only, until a chip with a leading `Done` is selected)
    let before = done();
    selectable().get(0).perform_click();
    selectable().get(0).assert_is_selected();
    assert_eq!(done(), before + 1);
    selectable().get(0).perform_click();
    selectable().get(0).assert_is_not_selected();
    assert_eq!(done(), before);
    let density = rule.density().density;
    // (32 dp chips, in 48 dp of room)
    for n in selectable().fetch_semantics_nodes() {
        assert!((n.unclipped_bounds_in_root().height() / density - 32.0).abs() < 0.5);
    }
}

/// ChipGroupSingleLineSample: one line of chips scrolled sideways (the last past the screen's edge); "Show All"
/// opens the DropdownMenu of all the chips under the row, each with its arrow; Escape closes it.
fn chip_group_single_line_sample() {
    let rule = themed(kompozable_lambda!(|| ChipGroupSingleLineSample()));
    let row = rule.on_node_with_tag("ChipGroupSingleLineSample").get_unclipped_bounds_in_root();
    let last = rule.on_node_with_text("Chip 8").get_unclipped_bounds_in_root();
    assert!(last.left > row.right, "{last:?} {row:?}");
    rule.on_all_nodes_with_text("Chip 0").assert_count_equals(1);
    rule.on_node_with_text("Show All").perform_click();
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 1);
    for i in 0..9 {
        rule.on_all_nodes_with_text(&format!("Chip {i}")).assert_count_equals(2);
    }
    let zeros = rule.on_all_nodes_with_text("Chip 0").fetch_semantics_nodes();
    let in_menu = zeros.iter().map(|n| n.unclipped_bounds_in_root()).find(|b| b.top >= row.bottom - 0.5);
    assert!(in_menu.is_some_and(|b| b.left < row.right), "{in_menu:?} {row:?}");
    rule.host().key_down(kompoz::ui::input::Key::Escape, kompoz::ui::input::KeyModifiers::None);
    rule.host().key_up(kompoz::ui::input::Key::Escape, kompoz::ui::input::KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
    rule.on_all_nodes_with_text("Chip 0").assert_count_equals(1);
}

// (WP4b: icons) ChipGroupReflowSample: its Icons.Filled.Tune the extended set's; its chips through ui-util's
// `fastForEachIndexed`.
fn chip_group_reflow_sample() {
    // (not in `themed`'s scrolling column: the sample scrolls vertically once selected)
    let rule = KompozTestRule::new(kompozable_lambda!(|| MaterialTheme(content = || ChipGroupReflowSample())));
    let tune = Icons::Filled::Tune().id();
    assert_eq!(rule.draw().vectors.iter().filter(|v| **v == tune).count(), 1);
    // (one line, scrolled sideways: the chips past the edge are there, at its height)
    let first = rule.on_node_with_text("Blue 0").get_unclipped_bounds_in_root();
    let last = rule.on_node_with_text("Transparent 9").get_unclipped_bounds_in_root();
    assert!((first.top - last.top).abs() < 0.5, "{first:?} {last:?}");
    // (Show All selected: every chip, the lines below each other)
    rule.on_node_with_text("Show All").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Show All").assert_is_selected();
    let last = rule.on_node_with_text("Transparent 9").get_unclipped_bounds_in_root();
    assert!(last.top > first.bottom, "{first:?} {last:?}");
}

// ---- FloatingActionButtonSamples' animated FABs (kt/buttons.kt: each a Scaffold's FAB over a LazyColumn) ---------

/// The list dragged `dy` (up: negative; above the FABs) over 20 frames, let go, settled with the FAB's animations.
fn drag_list(rule: &KompozTestRule, dy: f32) {
    let h = rule.host();
    let from = Offset::new(200.0, if dy < 0.0 { 550.0 } else { 100.0 });
    h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(from + Offset::new(0.0, dy * i as f32 / 20.0), -1, 0);
    }
    rule.advance_time_by(100);
    h.pointer_up(from + Offset::new(0.0, dy), -1, 0);
    rule.advance_time_by(2000);
}

fn animated_fab_samples() {
    // AnimatedFloatingActionButtonSample: the medium FAB (80 dp) at the scaffold's bottom end, 16 dp in; hidden (scaled
    // to nothing, taking no room) once the list's first item scrolls away, back with it.
    let rule = KompozTestRule::with_viewport(AnimatedFloatingActionButtonSample, IntSize::new(400, 800));
    let fab = || rule.on_node_with_content_description("Localized description").get_unclipped_bounds_in_root();
    let shown = fab();
    assert_eq!((shown.width(), shown.right, shown.bottom), (80.0, 400.0 - 16.0, 800.0 - 16.0));
    drag_list(&rule, -450.0);
    rule.on_node_with_text("List item - 0").assert_does_not_exist();
    // (its layout's size: what the scaffold has to place)
    assert_eq!(rule.on_node_with_content_description("Localized description").layout_node().size(), IntSize::new(0, 0));
    drag_list(&rule, 500.0);
    rule.on_node_with_text("List item - 0").assert_exists();
    assert_eq!(fab(), shown);

    // The four AnimatedExtendedFloatingActionButtonSamples: expanded (wider than tall) at the bottom end; collapsed to
    // a square of their height once the list scrolls, expanded again back at its start.
    let samples: [(fn() -> KompozTestRule, f32); 4] = [
        (|| KompozTestRule::with_viewport(AnimatedExtendedFloatingActionButtonSample, IntSize::new(400, 800)), 56.0),
        (
            || KompozTestRule::with_viewport(SmallAnimatedExtendedFloatingActionButtonSample, IntSize::new(400, 800)),
            56.0,
        ),
        (
            || KompozTestRule::with_viewport(MediumAnimatedExtendedFloatingActionButtonSample, IntSize::new(400, 800)),
            80.0,
        ),
        (
            || KompozTestRule::with_viewport(LargeAnimatedExtendedFloatingActionButtonSample, IntSize::new(400, 800)),
            96.0,
        ),
    ];
    for (make, height) in samples {
        let rule = make();
        let fab = || rule.on_node_with_content_description("Localized Description").get_unclipped_bounds_in_root();
        let expanded = fab();
        assert!(expanded.width() > height && expanded.height() == height, "{expanded:?}");
        assert_eq!((expanded.right, expanded.bottom), (400.0 - 16.0, 800.0 - 16.0));
        drag_list(&rule, -450.0);
        let collapsed = fab();
        assert!((collapsed.width() - height).abs() < 0.5 && collapsed.right == 400.0 - 16.0, "{collapsed:?}");
        drag_list(&rule, 500.0);
        assert!((fab().width() - expanded.width()).abs() < 0.5);
    }
}

fn main() {
    common::run(&[
        // (WP4b: icons)
        ("chip_group_reflow_sample", chip_group_reflow_sample),
        ("required_arguments_by_name", required_arguments_by_name),
        ("containers_as_compose_writes_them", containers_as_compose_writes_them),
        ("button_samples", button_samples),
        ("buttons_with_icons_come_in_sizes", buttons_with_icons_come_in_sizes),
        ("icon_button_samples", icon_button_samples),
        ("fab_samples", fab_samples),
        ("animated_fab_samples", animated_fab_samples),
        ("card_samples", card_samples),
        ("chip_samples", chip_samples),
        ("chip_group_single_line_sample", chip_group_single_line_sample),
    ]);
}
