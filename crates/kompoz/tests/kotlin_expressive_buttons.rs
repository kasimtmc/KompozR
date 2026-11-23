//! Material's Expressive buttons in Kotlin's syntax, as a Compose user writes them: toggle buttons, split buttons,
//! button groups, the FAB menu and segmented buttons — the C++ Kompoz's kompozable_tests.cpp sources (Expressive,
//! Mixed's segments, Navigation's FAB menu) through `kompoz!`, its other tests of these components written in Kotlin,
//! and material3's samples of them from `.kt` files (kompoz_file!).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

// ==== toggle and split buttons =======================================================================================
// (WP1b: toggle_split, in a module of its own: its imports and helpers apart from the other sections')
mod toggle_split {
    use super::*;
    use kompoz::test::{RecordingCanvas, has_content_description, has_role, has_state_description, has_text, is_on};
    use kompoz::ui::input::PointerType;
    use kompoz::ui::semantics::{Role, SemanticsProperties};
    use std::rc::Rc;

    // (WP1b: toggle_split) The C++ Kompoz's toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked and
    // split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow (kompoz_tests.cpp; kompozable_tests.cpp has no
    // toggle or split button source) written in Kotlin, checked as crates/kompoz-material3/tests/{toggle_buttons,
    // split_buttons}.rs check them (M15's values; the differences from the C++'s said there), and material3's
    // ToggleButtonSamples (10) and SplitButtonSamples (12) from kt/toggle_split_buttons.kt.

    kompoz! {
        @Composable
        fun ToggleMorph() {
            var bold by remember { mutableStateOf(false) }
            var italic by remember { mutableStateOf(true) }
            var clicks by remember { mutableStateOf(0) }
            Column {
                ToggleButton(checked = bold, onCheckedChange = { bold = it }, modifier = Modifier.testTag("kalın")) {
                    Text("Kalın")
                }
                OutlinedToggleButton(italic, { italic = it }, Modifier.testTag("italik")) { Text("İtalik") }
                Button(onClick = { clicks++ }, shapes = ButtonDefaults.shapes(), modifier = Modifier.testTag("düğme")) {
                    Text("Bas")
                }
                TonalToggleButton(true, { clicks++ }, Modifier.testTag("ton")) { Text("Ton") }
                ToggleButton(true, { clicks++ }, Modifier.testTag("kapalı"), enabled = false) { Text("Kapalı") }
                ElevatedToggleButton(true, {}, Modifier.testTag("kabarık")) { Text("Kabarık") }
                Text("kalın=$bold italik=$italic tık=$clicks")
            }
        }

        @Composable
        fun SplitMenu() {
            var open by remember { mutableStateOf(false) }
            var rtl by remember { mutableStateOf(false) }
            var saves by remember { mutableStateOf(0) }
            CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
                Column {
                    SplitButtonLayout(
                        leadingButton = {
                            SplitButtonDefaults.LeadingButton(onClick = { saves++ }, modifier = Modifier.testTag("kaydet")) {
                                Text("Kaydet")
                            }
                        },
                        trailingButton = {
                            SplitButtonDefaults.TrailingButton(
                                checked = open,
                                onCheckedChange = { open = it },
                                modifier = Modifier.testTag("menü"),
                            ) {
                                val rotation: Float by animateFloatAsState(targetValue = if (open) 180f else 0f)
                                Icon(
                                    Icons.Filled.KeyboardArrowDown,
                                    contentDescription = "Seçenekler",
                                    modifier = Modifier.size(SplitButtonDefaults.TrailingIconSize).graphicsLayer {
                                        rotationZ = rotation
                                    },
                                )
                            }
                        },
                    )
                    SplitButtonLayout(
                        leadingButton = {
                            SplitButtonDefaults.TonalLeadingButton(onClick = {}, modifier = Modifier.testTag("ton")) {
                                Text("Ton")
                            }
                        },
                        trailingButton = { SplitButtonDefaults.TonalTrailingButton(checked = false, onCheckedChange = {}) {} },
                    )
                    Text("açık=$open kayıt=$saves")
                    TextButton(onClick = { rtl = true }, modifier = Modifier.testTag("rtl")) { Text("Sağdan sola") }
                }
            }
        }
    }

    fn kt_near(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    fn kt_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
        rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
    }

    /// Long enough for the theme's springs to settle, drawn in between.
    fn kt_settle(rule: &KompozTestRule) {
        rule.advance_time_by(1000);
        rule.draw();
        rule.advance_time_by(100);
    }

    /// The fills drawn as `b` (rects and paths: corners of two sizes make a path), their colors.
    fn kt_fills(canvas: &RecordingCanvas, b: Rect) -> Vec<Color> {
        let at =
            |r: &Rect| kt_near(r.left, b.left, 0.1) && kt_near(r.top, b.top, 0.1) && kt_near(r.right, b.right, 0.1);
        let rects = canvas.rects.iter().zip(&canvas.rect_colors).filter(|(r, _)| at(r)).map(|(_, c)| *c);
        let paths = (canvas.paths.iter().zip(&canvas.path_colors).zip(&canvas.path_strokes))
            .filter(|((r, _), w)| at(r) && **w <= 0.0)
            .map(|((_, c), _)| *c);
        rects.chain(paths).collect()
    }

    /// The top start radius of the rounded fill centered on `b`.
    fn kt_radius(canvas: &RecordingCanvas, b: Rect) -> Option<f32> {
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_radii)
            .find(|(r, _)| kt_near(r.center().x, b.center().x, 0.1) && kt_near(r.center().y, b.center().y, 0.1))
            .map(|(_, radius)| *radius)
    }

    /// The corners (top left, top right, bottom right, bottom left) of the button tagged `tag` now: its shape (the
    /// animated one it tells accessibility) made an outline of its size.
    fn kt_corners(rule: &KompozTestRule, tag: &str, direction: LayoutDirection) -> [f32; 4] {
        let node = rule.on_node_with_tag_unmerged(tag).fetch_semantics_node();
        let shape = node.config().get(SemanticsProperties::Shape).expect("a shape");
        let b = kt_bounds(rule, tag);
        match shape.create_outline(Size::new(b.width(), b.height()), direction, &Density::new(1.0, 1.0)) {
            Outline::Rounded(r) => [
                r.top_left_corner_radius.x,
                r.top_right_corner_radius.x,
                r.bottom_right_corner_radius.x,
                r.bottom_left_corner_radius.x,
            ],
            _ => [-1.0; 4],
        }
    }

    pub fn toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked() {
        let rule = KompozTestRule::new(ToggleMorph);
        let cs = light_color_scheme();
        let at = kt_bounds(&rule, "kalın");
        let canvas = rule.draw();
        assert_eq!(kt_radius(&canvas, at), Some(20.0));
        assert_eq!(kt_fills(&canvas, at), vec![cs.surface_container]);
        rule.on_node_with_tag("kalın").assert_is_off();
        rule.host().pointer_down(at.center(), -1, 0, PointerType::Touch);
        rule.advance_frame();
        rule.draw();
        rule.advance_frame();
        rule.advance_frame();
        let morphing = kt_radius(&rule.draw(), at).expect("the container");
        assert!(morphing < 19.5 && morphing > 6.5, "on its way: {morphing}");
        kt_settle(&rule);
        assert_eq!(kt_radius(&rule.draw(), at), Some(6.0)); // (M15's pressed toggle shape; the C++ 8)
        rule.host().pointer_up(at.center(), -1, 0);
        kt_settle(&rule);
        let canvas = rule.draw();
        assert!(
            kt_radius(&canvas, at) == Some(12.0) && kt_fills(&canvas, at) == vec![cs.primary],
            "square once checked"
        );
        rule.on_node_with_tag("kalın").assert_is_on();
        rule.on_node_with_text("kalın=true italik=true tık=0").assert_exists();
        rule.on_node_with_tag("kalın").perform_click();
        kt_settle(&rule);
        assert_eq!(kt_radius(&rule.draw(), at), Some(20.0));
        // The outlined one: checked, the inverse surface without a line; unchecked, outlined.
        let it = kt_bounds(&rule, "italik");
        assert_eq!(kt_fills(&rule.draw(), it), vec![cs.inverse_surface]);
        rule.on_node_with_tag("italik").perform_click();
        let canvas = rule.draw();
        let line = canvas
            .strokes
            .iter()
            .zip(&canvas.stroke_colors)
            .find(|(s, _)| kt_near(s.center().y, it.center().y, 0.5))
            .map(|(_, c)| *c);
        assert_eq!(line, Some(cs.outline_variant));
        // A plain button with shapes morphs while held (the theme's small corners).
        let d = kt_bounds(&rule, "düğme");
        rule.host().pointer_down(d.center(), -1, 0, PointerType::Touch);
        kt_settle(&rule);
        assert_eq!(kt_radius(&rule.draw(), d), Some(8.0));
        rule.host().pointer_up(d.center(), -1, 0);
        kt_settle(&rule);
        assert_eq!(kt_radius(&rule.draw(), d), Some(20.0));
        // Tonal checked: secondary, square; disabled: the surface's text at 10%, no clicks.
        let canvas = rule.draw();
        let ton = kt_bounds(&rule, "ton");
        assert!(kt_fills(&canvas, ton) == vec![cs.secondary] && kt_radius(&canvas, ton) == Some(12.0));
        assert_eq!(kt_fills(&canvas, kt_bounds(&rule, "kapalı")), vec![named_args!(cs.on_surface.copy(alpha = 0.1))]);
        rule.on_node_with_tag("kapalı").perform_click().assert_is_not_enabled().assert_is_on();
        rule.on_node_with_text("kalın=false italik=false tık=1").assert_exists();
        // A checkbox to accessibility, checked, named by its label.
        let told = rule
            .on_all_nodes(has_role(Role::Checkbox).and(is_on()).and(has_text("Kabarık", false, false)))
            .fetch_semantics_nodes();
        assert_eq!(told.len(), 1);
    }

    pub fn split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow() {
        let rule = KompozTestRule::new(SplitMenu);
        let cs = light_color_scheme();
        let (lead, trail) = (kt_bounds(&rule, "kaydet"), kt_bounds(&rule, "menü"));
        assert!(
            kt_near(trail.left - lead.right, 2.0, 0.01) && kt_near(trail.width(), 48.0, 0.01),
            "{lead:?} {trail:?}"
        );
        let canvas = rule.draw();
        assert_eq!(kt_fills(&canvas, lead), vec![cs.primary]);
        assert_eq!(kt_fills(&canvas, trail), vec![cs.primary]);
        assert_eq!(kt_fills(&canvas, kt_bounds(&rule, "ton")), vec![cs.secondary_container]);
        assert_eq!(kt_corners(&rule, "kaydet", LayoutDirection::Ltr), [20.0, 4.0, 4.0, 20.0]);
        assert_eq!(kt_corners(&rule, "menü", LayoutDirection::Ltr), [4.0, 20.0, 20.0, 4.0]);
        rule.on_node_with_tag("menü").assert_is_off();
        rule.on_node_with_tag("menü").perform_click();
        kt_settle(&rule);
        let canvas = rule.draw();
        assert_eq!(kt_corners(&rule, "menü", LayoutDirection::Ltr), [20.0, 20.0, 20.0, 20.0]);
        assert_eq!(kt_fills(&canvas, trail)[0], cs.primary);
        // (the arrow turned over: its layer drawn through a half turn)
        assert!(canvas.transforms.iter().any(|m| kt_near(m.get(0, 0), -1.0, 0.001)));
        rule.on_node_with_tag("menü").assert_is_on();
        rule.on_node_with_text("açık=true kayıt=0").assert_exists();
        // Held: rounder inside (M15: 12 dp; the C++ 8).
        rule.host().pointer_down(lead.center(), -1, 0, PointerType::Touch);
        kt_settle(&rule);
        assert_eq!(kt_corners(&rule, "kaydet", LayoutDirection::Ltr), [20.0, 12.0, 12.0, 20.0]);
        rule.host().pointer_up(lead.center(), -1, 0);
        kt_settle(&rule);
        assert_eq!(kt_corners(&rule, "kaydet", LayoutDirection::Ltr), [20.0, 4.0, 4.0, 20.0]);
        rule.on_node_with_tag("menü").perform_click();
        rule.on_node_with_tag("rtl").perform_click();
        kt_settle(&rule);
        rule.on_node_with_text("açık=false kayıt=1").assert_exists();
        let (lead, trail) = (kt_bounds(&rule, "kaydet"), kt_bounds(&rule, "menü"));
        assert!(trail.right < lead.left && kt_corners(&rule, "kaydet", LayoutDirection::Rtl) == [4.0, 20.0, 20.0, 4.0]);
    }

    kompoz_file!("tests/kt/toggle_split_buttons.kt");

    /// The samples in a MaterialTheme, in a column that scrolls.
    fn toggle_split_themed(content: impl Fn() + 'static) -> KompozTestRule {
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
            IntSize::new(600, 2400),
        )
    }

    pub fn toggle_button_samples() {
        // Each sample toggles with a click (a checkbox to accessibility, on after it); the sized ones as tall as their
        // size; the square one turns round once checked.
        let samples: [(&str, Rc<dyn Fn()>); 10] = [
            ("Button", Rc::new(kompozable_lambda!(|| ToggleButtonSample())) as Rc<dyn Fn()>),
            ("Button", Rc::new(kompozable_lambda!(|| SquareToggleButtonSample())) as Rc<dyn Fn()>),
            ("Elevated Button", Rc::new(kompozable_lambda!(|| ElevatedToggleButtonSample())) as Rc<dyn Fn()>),
            ("Tonal Button", Rc::new(kompozable_lambda!(|| TonalToggleButtonSample())) as Rc<dyn Fn()>),
            ("Outlined Button", Rc::new(kompozable_lambda!(|| OutlinedToggleButtonSample())) as Rc<dyn Fn()>),
            ("Edit", Rc::new(kompozable_lambda!(|| ToggleButtonWithIconSample())) as Rc<dyn Fn()>),
            ("Label", Rc::new(kompozable_lambda!(|| XSmallToggleButtonWithIconSample())) as Rc<dyn Fn()>),
            ("Label", Rc::new(kompozable_lambda!(|| MediumToggleButtonWithIconSample())) as Rc<dyn Fn()>),
            ("Label", Rc::new(kompozable_lambda!(|| LargeToggleButtonWithIconSample())) as Rc<dyn Fn()>),
            ("Label", Rc::new(kompozable_lambda!(|| XLargeToggleButtonWithIconSample())) as Rc<dyn Fn()>),
        ];
        let heights = [40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 32.0, 56.0, 96.0, 136.0];
        for ((text, sample), height) in samples.into_iter().zip(heights) {
            let rule = toggle_split_themed(move || sample());
            let node = rule.on_node(has_role(Role::Checkbox));
            node.assert_is_off();
            assert!(kt_near(node.get_unclipped_bounds_in_root().height(), height, 0.01), "{text} {height}");
            rule.on_node_with_text(text).assert_exists();
            node.perform_click();
            rule.on_node(has_role(Role::Checkbox)).assert_is_on();
            if height == 40.0 && text == "Button" {
                kt_settle(&rule);
            }
        }
        // SquareToggleButtonSample: square at rest, round once checked.
        let rule = toggle_split_themed(kompozable_lambda!(|| SquareToggleButtonSample()));
        let b = rule.on_node(has_role(Role::Checkbox)).get_unclipped_bounds_in_root();
        assert_eq!(kt_radius(&rule.draw(), b), Some(12.0));
        rule.on_node(has_role(Role::Checkbox)).perform_click();
        kt_settle(&rule);
        assert_eq!(kt_radius(&rule.draw(), b), Some(20.0));
    }

    pub fn split_button_samples() {
        // Each sample's trailing button toggles (Expanded/Collapsed, its arrow turned over) or clicks; the menu sample's
        // menu opens with it; the sized ones as tall as their size.
        let checkable: [(Rc<dyn Fn()>, f32); 11] = [
            (Rc::new(kompozable_lambda!(|| FilledSplitButtonSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| SplitButtonWithDropdownMenuSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| TonalSplitButtonSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| ElevatedSplitButtonSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| OutlinedSplitButtonSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| SplitButtonWithTextSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| SplitButtonWithIconSample())) as Rc<dyn Fn()>, 40.0),
            (Rc::new(kompozable_lambda!(|| XSmallFilledSplitButtonSample())) as Rc<dyn Fn()>, 32.0),
            (Rc::new(kompozable_lambda!(|| MediumFilledSplitButtonSample())) as Rc<dyn Fn()>, 56.0),
            (Rc::new(kompozable_lambda!(|| LargeFilledSplitButtonSample())) as Rc<dyn Fn()>, 96.0),
            (Rc::new(kompozable_lambda!(|| ExtraLargeFilledSplitButtonSample())) as Rc<dyn Fn()>, 136.0),
        ];
        for (i, (sample, height)) in checkable.into_iter().enumerate() {
            let rule = toggle_split_themed(move || sample());
            let trailing = rule.on_node(has_state_description("Collapsed").and(has_content_description(
                "Toggle Button",
                false,
                false,
            )));
            trailing.assert_is_off();
            assert!(kt_near(trailing.get_unclipped_bounds_in_root().height(), height, 0.01), "sample {i}");
            trailing.perform_click();
            kt_settle(&rule);
            rule.on_node(has_state_description("Expanded")).assert_is_on();
            let canvas = rule.draw();
            assert!(
                canvas.transforms.iter().any(|m| kt_near(m.get(0, 0), -1.0, 0.001)),
                "sample {i}: the arrow turned"
            );
            if i == 1 {
                for item in ["Edit", "Settings", "Send Feedback", "F11"] {
                    rule.on_node_with_text(item).assert_exists();
                }
            }
        }
        // SplitButtonWithUnCheckableTrailingButtonSample: a button, no toggle state.
        let rule = toggle_split_themed(kompozable_lambda!(|| SplitButtonWithUnCheckableTrailingButtonSample()));
        let node = rule.on_node(has_content_description("Toggle Button", false, false)).fetch_semantics_node();
        assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button));
        assert!(node.config().get(SemanticsProperties::ToggleableState).is_none());
        rule.on_node_with_text("My Button").assert_exists();
    }
}

// ==== button groups ==================================================================================================
// (WP1b: group, in a module of its own: its imports and helpers apart from the other sections')
#[allow(deprecated, unused_variables)]
mod group {
    use super::*;
    use kompoz::test::{has_role, has_text, is_on};
    use kompoz::ui::input::PointerType;
    use kompoz::ui::semantics::{Role, SemanticsActions, SemanticsProperties};

    // (WP1b: group) kompozable_tests.cpp's `Expressive` as written, M15's where the C++ called what M15 hasn't (said at
    // each line), with its test expressive_buttons_and_sliders_as_compose_writes_them; the C++ Kompoz's ButtonGroup
    // tests (kompoz_tests.cpp: button_group_grows_the_held_button_and_squeezes_its_neighbours,
    // button_group_puts_what_does_not_fit_into_a_menu, connected_button_group_rounds_the_outer_ends_and_the_checked_one)
    // written in Kotlin, checked as crates/kompoz-material3/tests/button_groups.rs checks them (M15's values; the
    // differences from the C++'s said there); material3's ButtonGroupSamples (6) from kt/button_groups.kt.

    kompoz! {
        @Composable
        fun Expressive() {
            var last by remember { mutableStateOf("") }
            var bold by remember { mutableStateOf(false) }
            var open by remember { mutableStateOf(false) }
            var volume by remember { mutableStateOf(40f) }
            var price by remember { mutableStateOf(200f..800f) }
            val source = remember { MutableInteractionSource() }
            val stars = rememberSliderState(value = 3f, steps = 3, valueRange = 1f..5f)
            val hours = rememberRangeSliderState(activeRangeStart = 9f, activeRangeEnd = 17f, steps = 23, valueRange = 0f..24f)
            Column {
                ButtonGroup {
                    Button(onClick = { last = "oynat" }, modifier = Modifier.animateWidth(source).testTag("oynat"), interactionSource = source) { Text("Oynat") }
                }
                ButtonGroup(
                    overflowIndicator = { menuState ->
                        // (C++: `menuState.isExpanded`, hidden in M15: its `isShowing`)
                        IconButton(onClick = { if (menuState.isShowing) menuState.dismiss() else menuState.show() }, modifier = Modifier.testTag("daha")) {
                            Icon(Icons.Filled.MoreVert, contentDescription = "Daha")
                        }
                    },
                    modifier = Modifier.width(120.dp),
                ) {
                    clickableItem(onClick = { last = "kopyala" }, label = "Kopyala")
                    toggleableItem(checked = bold, onCheckedChange = { bold = it }, label = "Kalın")
                    clickableItem(onClick = { last = "sil" }, label = "Sil", icon = { Icon(Icons.Filled.Delete, contentDescription = null) })
                }
                ToggleButton(checked = bold, onCheckedChange = { bold = it }, shapes = ButtonGroupDefaults.connectedLeadingButtonShapes()) { Text("Koyu") }
                SplitButtonLayout(
                    leadingButton = { SplitButtonDefaults.LeadingButton(onClick = { last = "kaydet" }) { Text("Kaydet") } },
                    trailingButton = {
                        // (C++: `SplitButtonDefaults.TrailingButtonIcon(open)`, which M15 hasn't: its samples' arrow)
                        SplitButtonDefaults.TrailingButton(checked = open, onCheckedChange = { open = it }) {
                            Icon(
                                Icons.Filled.KeyboardArrowDown,
                                modifier = Modifier.size(SplitButtonDefaults.TrailingIconSize).graphicsLayer { rotationZ = if (open) 180f else 0f },
                                contentDescription = "Localized description",
                            )
                        }
                    },
                )
                // (C++: `SliderDefaults.Thumb(s)` and `track = if (bold) { { s -> SliderDefaults.Track(s) } } else null`: M15's
                // thumb takes its interaction source and state, its track isn't nullable — its stop indicator is, the lambda
                // or null going there, as WP2's ExpressiveSliders)
                Slider(value = volume, onValueChange = { volume = it }, valueRange = 0f..100f, interactionSource = source,
                       thumb = { s -> Label(label = { PlainTooltip { Text("%.0f".format(volume)) } }, interactionSource = source) { SliderDefaults.Thumb(interactionSource = source, sliderState = s) } },
                       track = { s -> SliderDefaults.Track(s, drawStopIndicator = if (bold) { { drawCircle(Color.Red, radius = 3.dp.toPx(), center = it) } } else null) })
                RangeSlider(value = price, onValueChange = { price = it }, valueRange = 0f..1000f, steps = 19)
                val names = listOf<String>()
                Text("son: ${last.ifEmpty { "yok" }} · ${price.start.roundToInt()}–${price.endInclusive.roundToInt()} · ${names.ifEmpty { listOf("kimse") }.first()} · " +
                         "${stars.value.roundToInt()} · ${hours.activeRangeEnd.roundToInt()} · ${if (bold) "kalın" else "ince"} · ${if (open) "açık" else "kapalı"}",
                     fontWeight = if (bold) FontWeight.Bold else null)
            }
        }

        // C++ button_group_grows_the_held_button_and_squeezes_its_neighbours, in Kotlin (the group composing its content
        // as written: M15's deprecated overload).
        @Composable
        fun HeldGroup(first: MutableInteractionSource, second: MutableInteractionSource, third: MutableInteractionSource) {
            var rtl by remember { mutableStateOf(false) }
            CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
                Column {
                    ButtonGroup {
                        Button(onClick = {}, modifier = Modifier.animateWidth(first).testTag("a"), interactionSource = first) { Text("Önceki") }
                        Button(onClick = {}, modifier = Modifier.animateWidth(second).testTag("b"), interactionSource = second) { Text("Oynat") }
                        Button(onClick = {}, modifier = Modifier.animateWidth(third).testTag("c"), interactionSource = third) { Text("Sonraki") }
                    }
                    ButtonGroup(expandedRatio = 0.6f, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        val big = remember { MutableInteractionSource() }
                        Button(onClick = {}, modifier = Modifier.animateWidth(big).testTag("büyük"), interactionSource = big) { Text("Önceki") }
                        Box(Modifier.size(64.dp).testTag("kutu"))
                    }
                    Text(if (rtl) "sağdan" else "soldan", Modifier.clickable { rtl = !rtl }.testTag("yön"))
                }
            }
        }

        // C++ button_group_puts_what_does_not_fit_into_a_menu, in Kotlin.
        @Composable
        fun OverflowingGroup() {
            var log by remember { mutableStateOf("") }
            var cut by remember { mutableStateOf(false) }
            var wide by remember { mutableStateOf(false) }
            Column {
                Box(Modifier.width(if (wide) 600.dp else 400.dp)) {
                    ButtonGroup(overflowIndicator = { ButtonGroupDefaults.OverflowIndicator(it) }) {
                        clickableItem(onClick = { log += "kopyala " }, label = "Kopyala")
                        clickableItem(onClick = { log += "yapıştır " }, label = "Yapıştır")
                        toggleableItem(checked = cut, label = "Kes", onCheckedChange = { cut = it })
                        clickableItem(onClick = { log += "sil " }, label = "Sil")
                        clickableItem(onClick = { log += "paylaş " }, label = "Paylaş")
                    }
                }
                Text("günlük: $log kes=$cut", Modifier.padding(top = 80.dp))
                Text("genişlet", Modifier.clickable { wide = !wide })
            }
        }

        // C++ connected_button_group_rounds_the_outer_ends_and_the_checked_one, in Kotlin.
        @Composable
        fun ConnectedGroup() {
            var picked by remember { mutableStateOf(-1) }
            val labels = listOf("Gün", "Hafta", "Ay")
            Row(Modifier.width(320.dp), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                labels.forEachIndexed { i, label ->
                    ToggleButton(
                        checked = picked == i,
                        onCheckedChange = { picked = i },
                        modifier = Modifier.weight(1f).testTag(label),
                        shapes = when (i) {
                            0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                            labels.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                            else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                        },
                    ) { Text(label) }
                }
            }
        }
    }

    kompoz_file!("tests/kt/button_groups.kt");

    fn kt_near(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    fn kt_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
        rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
    }

    fn kt_text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
        rule.on_node_with(has_text(text, false, false), true).get_unclipped_bounds_in_root()
    }

    /// The bounds of the shown node of `text` (placed, of a size: the menu's over the row's).
    fn kt_shown(rule: &KompozTestRule, text: &str) -> Option<Rect> {
        let all = rule.on_all_nodes_with(has_text(text, false, false), true).fetch_semantics_nodes();
        all.iter().map(|n| n.bounds_in_root()).rfind(|b| b.width() > 0.0)
    }

    fn kt_drawn(rule: &KompozTestRule, text: &str) -> bool {
        rule.drawn_texts().iter().any(|t| t == text)
    }

    fn kt_tap(rule: &KompozTestRule, at: Offset) {
        rule.host().pointer_down(at, -1, 0, PointerType::Touch);
        rule.host().pointer_up(at, -1, 0);
        rule.wait_for_idle();
    }

    /// The corners of the shape a node tells, left to right.
    fn kt_corners(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
        rule.draw();
        let node = rule.on_node_with_tag_unmerged(tag).fetch_semantics_node();
        let shape = node.config().get(SemanticsProperties::Shape).expect("a shape");
        let b = kt_bounds(rule, tag);
        match shape.create_outline(Size::new(b.width(), b.height()), LayoutDirection::Ltr, &Density::new(1.0, 1.0)) {
            Outline::Rounded(r) => [
                r.top_left_corner_radius.x,
                r.top_right_corner_radius.x,
                r.bottom_right_corner_radius.x,
                r.bottom_left_corner_radius.x,
            ],
            other => panic!("{other:?}"),
        }
    }

    fn kt_settle(rule: &KompozTestRule) {
        rule.advance_time_by(1000);
        rule.draw();
        rule.advance_time_by(100);
    }

    pub fn expressive_buttons_and_sliders_as_compose_writes_them() {
        let rule = KompozTestRule::new(Expressive);
        rule.on_node_with_text("son: yok · 200–800 · kimse · 3 · 17 · ince · kapalı").assert_exists();
        rule.on_node_with_tag("oynat").perform_click();
        rule.on_node_with_text("Kaydet").perform_click();
        rule.on_node_with_text("Koyu").perform_click();
        rule.on_node_with_text("son: kaydet · 200–800 · kimse · 3 · 17 · kalın · kapalı").assert_exists();
        // (M15's group is as tall as the items it shows — none of 120 dp —: the indicator reaches over the toggle button
        // under it, which takes a touch there; its click action, then)
        rule.on_node_with_tag("daha").perform_semantics_action(SemanticsActions::OnClick);
        rule.wait_for_idle();
        // the menu's (the row's are composed, not placed)
        kt_tap(&rule, kt_shown(&rule, "Sil").expect("the menu's").center());
        rule.on_node_with_text("son: sil · 200–800 · kimse · 3 · 17 · kalın · kapalı").assert_exists();
        // the trailing button turns the arrow (its rotation as drawn)
        rule.on_node_with_text("Kaydet").assert_exists();
    }

    pub fn button_group_grows_the_held_button_and_squeezes_its_neighbours() {
        let (first, second, third) =
            (MutableInteractionSource::new(), MutableInteractionSource::new(), MutableInteractionSource::new());
        let (f, s, t) = (first.clone(), second.clone(), third.clone());
        let rule = KompozTestRule::new(kompozable_lambda!(move || HeldGroup(f.clone(), s.clone(), t.clone())));
        let (a0, b0, c0) = (kt_bounds(&rule, "a"), kt_bounds(&rule, "b"), kt_bounds(&rule, "c"));
        assert!(kt_near(b0.left - a0.right, 12.0, 0.01) && kt_near(c0.left - b0.right, 12.0, 0.01));
        assert!(b0.width() < a0.width() && a0.width() < c0.width());
        rule.host().pointer_down(b0.center(), -1, 0, PointerType::Touch);
        rule.advance_time_by(500);
        let extra = b0.width() * 0.15;
        assert!(kt_near(kt_bounds(&rule, "b").width(), b0.width() + extra, 1.0));
        assert!(kt_near(kt_bounds(&rule, "a").width(), a0.width() - extra / 2.0, 1.0));
        assert!(kt_near(kt_bounds(&rule, "c").right, c0.right, 1.01));
        rule.host().pointer_up(b0.center(), -1, 0);
        rule.advance_time_by(500);
        assert_eq!(kt_bounds(&rule, "b"), b0);
        // The first takes it all from the second.
        rule.host().pointer_down(a0.center(), -1, 0, PointerType::Touch);
        rule.advance_time_by(500);
        let extra = a0.width() * 0.15;
        assert!(kt_near(kt_bounds(&rule, "b").width(), b0.width() - extra, 1.0) && kt_bounds(&rule, "c") == c0);
        rule.host().pointer_up(a0.center(), -1, 0);
        rule.advance_time_by(500);
        // The second group: 4 dp apart, at the top (M15's deprecated group; the container 4 dp into its 48 of room).
        let (big, kutu) = (kt_bounds(&rule, "büyük"), kt_bounds(&rule, "kutu"));
        assert!(kt_near(kutu.left - big.right, 4.0, 0.01) && kt_near(big.top, kutu.top + 4.0, 0.01));
        // Right to left: from the right; the last one, on the left now, takes from the middle.
        rule.on_node_with_tag("yön").perform_click();
        rule.wait_for_idle();
        assert!(
            kt_near(kt_bounds(&rule, "a").right, c0.right, 0.01)
                && kt_bounds(&rule, "c").left < kt_bounds(&rule, "b").left
        );
        let c = kt_bounds(&rule, "c");
        rule.host().pointer_down(c.center(), -1, 0, PointerType::Touch);
        rule.advance_time_by(500);
        let extra = c0.width() * 0.15;
        assert!(
            kt_near(kt_bounds(&rule, "c").width(), c0.width() + extra, 1.0)
                && kt_near(kt_bounds(&rule, "c").left, 0.0, 0.01)
        );
    }

    pub fn button_group_puts_what_does_not_fit_into_a_menu() {
        let rule = KompozTestRule::with_viewport(OverflowingGroup, IntSize::new(600, 800));
        assert!(
            kt_drawn(&rule, "Kopyala")
                && kt_drawn(&rule, "Kes")
                && !kt_drawn(&rule, "Sil")
                && !kt_drawn(&rule, "Paylaş")
        );
        kt_tap(&rule, kt_shown(&rule, "Kopyala").unwrap().center());
        kt_tap(&rule, rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root().center());
        assert!(kt_drawn(&rule, "Sil") && kt_drawn(&rule, "Paylaş"));
        kt_tap(&rule, kt_shown(&rule, "Paylaş").unwrap().center());
        rule.on_node_with_text("günlük: kopyala paylaş  kes=false").assert_exists();
        assert!(!kt_drawn(&rule, "Sil")); // the menu closed
        rule.on_node_with_text("genişlet").perform_click();
        rule.wait_for_idle();
        assert!(kt_drawn(&rule, "Sil") && kt_drawn(&rule, "Paylaş"));
        rule.on_node_with_content_description("More options").assert_is_not_displayed();
        rule.on_node_with_text("genişlet").perform_click();
        rule.wait_for_idle();
        // Picked in the menu, the toggle item toggles (M15's menu item shows no check mark).
        kt_tap(&rule, rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root().center());
        kt_tap(&rule, kt_shown(&rule, "Sil").unwrap().center());
        rule.on_node_with_text("günlük: kopyala paylaş sil  kes=false").assert_exists();
    }

    pub fn connected_button_group_rounds_the_outer_ends_and_the_checked_one() {
        let rule = KompozTestRule::new(ConnectedGroup);
        let (day, week) = (kt_bounds(&rule, "Gün"), kt_bounds(&rule, "Hafta"));
        assert!(kt_near(week.left - day.right, 2.0, 0.01));
        assert_eq!(kt_corners(&rule, "Gün"), [20.0, 8.0, 8.0, 20.0]);
        assert_eq!(kt_corners(&rule, "Hafta"), [8.0; 4]);
        assert_eq!(kt_corners(&rule, "Ay"), [8.0, 20.0, 20.0, 8.0]);
        rule.host().pointer_down(week.center(), -1, 0, PointerType::Touch);
        kt_settle(&rule);
        assert_eq!(kt_corners(&rule, "Hafta"), [4.0; 4]); // squarer while held
        rule.host().pointer_up(week.center(), -1, 0);
        kt_settle(&rule);
        assert_eq!(kt_corners(&rule, "Hafta"), [20.0; 4]); // checked: fully round
        kt_tap(&rule, day.center());
        kt_settle(&rule);
        assert_eq!(kt_corners(&rule, "Gün"), [20.0; 4]);
        assert_eq!(kt_corners(&rule, "Hafta"), [8.0; 4]);
    }

    pub fn button_group_samples() {
        let rule = KompozTestRule::new(kompozable_lambda!(|| ButtonGroupSample()));
        assert!(kt_drawn(&rule, "0") && !kt_drawn(&rule, "9"));
        kt_tap(&rule, rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root().center());
        assert!(kt_drawn(&rule, "9"));

        let rule = KompozTestRule::new(kompozable_lambda!(|| SingleSelectConnectedButtonGroupSample()));
        let radios = rule.on_all_nodes(has_role(Role::RadioButton)).fetch_semantics_nodes();
        let widths: Vec<f32> = radios.iter().map(|n| n.bounds_in_root().width()).collect();
        assert!(widths.len() == 3 && kt_near(widths[1], widths[0] * 1.5, 1.0), "{widths:?}");
        rule.on_node_with_text("Coffee").perform_click();
        rule.wait_for_idle();
        rule.on_all_nodes(is_on().and(has_text("Coffee", false, false))).assert_count_equals(1);
        rule.on_all_nodes(is_on()).assert_count_equals(1);

        let rule = KompozTestRule::new(kompozable_lambda!(|| SingleSelectConnectedButtonGroupWithFlowLayoutSample()));
        assert!(kt_text_bounds(&rule, "Home").top > kt_text_bounds(&rule, "Work").top + 40.0);
        rule.on_node_with_text("Home").perform_click();
        rule.wait_for_idle();
        rule.on_all_nodes(is_on().and(has_text("Home", false, false))).assert_count_equals(1);

        let rule = KompozTestRule::new(kompozable_lambda!(|| MultiSelectConnectedButtonGroupSample()));
        rule.on_node_with_text("Work").perform_click();
        rule.on_node_with_text("Coffee").perform_click();
        rule.wait_for_idle();
        rule.on_all_nodes(is_on()).assert_count_equals(2);

        let rule = KompozTestRule::new(kompozable_lambda!(|| MultiSelectConnectedButtonGroupWithFlowLayoutSample()));
        rule.on_node_with_text("Search").perform_click();
        rule.wait_for_idle();
        rule.on_all_nodes(is_on().and(has_text("Search", false, false))).assert_count_equals(1);

        let rule = KompozTestRule::new(kompozable_lambda!(|| VerticalButtonGroupSample()));
        rule.on_node_with_text("Button 3").perform_click();
        rule.wait_for_idle();
        rule.on_all_nodes(is_on().and(has_text("Button 3", false, false))).assert_count_equals(1);
        assert!(kt_text_bounds(&rule, "Button 2").top > kt_text_bounds(&rule, "Button 1").top);
    }
}

// ==== FAB menu and segmented buttons =================================================================================
// (WP1b: fab_segmented, in a module of its own: its imports and helpers apart from the other sections')
mod fab_segmented {
    use super::*;
    use kompoz::test::RecordingCanvas;
    use kompoz::ui::input::PointerType;
    use kompoz::ui::semantics::{Role, SemanticsActions, SemanticsProperties, ToggleableState};
    use std::rc::Rc;

    // (WP1b: fab_segmented) kompozable_tests.cpp's `Mixed` (whole as written: its ViewModel and SideLine — stage 13
    // WP1 —, segments and staggered grid) with view_models_maps_and_segments_as_kotlin_writes_them's checks
    // (kompoz_tests.cpp's
    // segmented_buttons_take_compose_named_arguments, much the same content, is in both syntaxes in named_forms.rs, as
    // are its tabs_and_fab_menu_… and expressive_buttons_…); `Navigation` (whole as written: its tabs, FAB
    // menu and floating toolbar — the one copy of it; kotlin_lists_sheets.rs's NavigationTabs and
    // kotlin_floating_toolbar.rs's NavigationToolbar check its parts on their own) with
    // tabs_and_fab_menu_as_compose_writes_them; the C++ Kompoz's
    // other FAB menu and segmented button tests (kompoz_tests.cpp: fab_menu_opens_from_the_bottom_and_its_button_turns_round,
    // segmented_buttons_as_compose_1_4, segmented_buttons_keep_48_dp_of_room_and_draw_40,
    // multi_choice_segmented_buttons_toggle_each_and_tell_their_roles, the segments of
    // chips_radio_buttons_and_segments_select) written in Kotlin, checked as crates/kompoz-material3/tests/{fab_menu,
    // segmented_buttons}.rs check them (M15's values; the differences from the C++'s said there); material3's
    // SegmentedButtonSamples (2) and FloatingActionButtonMenuSample from kt/fab_menu_segmented.kt.

    // ---- Rust twins of Mixed's Kotlin classes (stage 14) ----

    /// kompozable_tests.cpp's `class TodoViewModel : ViewModel() { val items = MutableStateFlow(listOf(Todo(1, "Yaz"),
    /// Todo(2, "Gönder"))) … }` (its items' count is what SideLine shows: their titles here).
    pub type TodoViewModel = Rc<TodoViewModelClass>;

    /// TodoViewModel's object.
    pub struct TodoViewModelClass {
        base: ViewModelImpl,
        items: MutableStateFlow<Vec<String>>,
    }

    impl TodoViewModelClass {
        pub fn items(&self) -> MutableStateFlow<Vec<String>> {
            self.items.clone()
        }
    }

    impl ViewModel for TodoViewModelClass {
        fn view_model_impl(&self) -> &ViewModelImpl {
            &self.base
        }
        fn new_instance(_extras: &CreationExtras) -> Option<Self> {
            Some(TodoViewModelClass {
                base: ViewModelImpl::new(),
                items: MutableStateFlow::new(vec!["Yaz".into(), "Gönder".into()]),
            })
        }
    }

    /// kompozable_tests.cpp's `enum class Side { Left, Right }`.
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum Side {
        #[allow(dead_code)] // (the C++ test uses the other one)
        Left,
        Right,
    }

    impl Side {
        pub fn ordinal(self) -> i32 {
            self as i32
        }
    }

    /// kompozable_tests.cpp's `namespace routes { struct Item { int n = 0; }; }` (Kotlin's `object routes` with its
    /// `class Item(val n: Int)`).
    #[derive(Clone, Copy)]
    pub struct Routes;

    #[allow(non_upper_case_globals)]
    pub const routes: Routes = Routes;

    /// `routes.Item`.
    #[derive(Clone, Copy)]
    pub struct RoutesItem {
        n: i32,
    }

    impl Routes {
        pub fn Item(&self, n: i32) -> RoutesItem {
            RoutesItem { n }
        }
    }

    impl RoutesItem {
        pub fn n(&self) -> i32 {
            self.n
        }
    }

    kompoz! {
        @Composable
        fun SideLine(vm: TodoViewModel, side: Side) {
            val items by vm.items.collectAsState()
            val counts = mapOf(1 to 3)
            val n = counts[1] ?: 0
            val m = counts[2] ?: 0
            Text("öğe ${items.size} · ${side.ordinal} · $n $m · ${counts.keys.sum()} · ${counts.values.sum()} · ${routes.Item(7).n}")
        }

        @Composable
        fun Mixed() {
            val vm = viewModel<TodoViewModel>()
            var pick by remember { mutableStateOf(0) }
            var bold by remember { mutableStateOf(false) }
            Column {
                SideLine(vm, Side.Right)
                SingleChoiceSegmentedButtonRow(modifier = Modifier.width(300.dp)) {
                    val labels = listOf("Bir", "İki")
                    for (i in 0 until 2) {
                        SegmentedButton(selected = pick == i, onClick = { pick = i }, shape = SegmentedButtonDefaults.itemShape(i, 2)) { Text(labels[i]) }
                    }
                }
                // (M15's SegmentedButton takes its shape, which the C++'s line left out: the one segment's, the base shape)
                MultiChoiceSegmentedButtonRow { SegmentedButton(checked = bold, onCheckedChange = { bold = it }, shape = SegmentedButtonDefaults.itemShape(0, 1)) { Text("Kalın") } }
                Text("seçim $pick ${if (bold) "kalın" else "ince"}")
                LazyVerticalStaggeredGrid(columns = StaggeredGridCells.Fixed(2), modifier = Modifier.size(200.dp)) {
                    item(span = StaggeredGridItemSpan.FullLine) { Text("Başlık", modifier = Modifier.testTag("başlık")) }
                    items(listOf("a", "b"), span = { StaggeredGridItemSpan.SingleLane }) { Text(it, modifier = Modifier.height(40.dp).testTag("kart$it")) }
                }
            }
        }

        @Composable
        fun Navigation() {
            var tab by remember { mutableStateOf(0) }
            var open by remember { mutableStateOf(false) }
            var picked by remember { mutableStateOf("yok") }
            var more by remember { mutableStateOf(false) }
            Column {
                PrimaryTabRow(selectedTabIndex = tab) {
                    for (i in 0 until 2) Tab(selected = tab == i, onClick = { tab = i }, text = { Text("sekme $i") }, icon = { Icon(Icons.Filled.Star, contentDescription = null) })
                }
                PrimaryScrollableTabRow(selectedTabIndex = tab, edgePadding = 0.dp) {
                    for (i in 0 until 2) LeadingIconTab(selected = tab == i, onClick = { tab = i }, text = { Text("kayan $i") }, icon = { Icon(Icons.Filled.Home, contentDescription = null) })
                }
                Text("seçili $tab · $picked · ${if (more) "fazla" else "az"}")
                Box(modifier = Modifier.size(300.dp), contentAlignment = Alignment.BottomEnd) {
                    FloatingActionButtonMenu(
                        expanded = open,
                        button = {
                            ToggleFloatingActionButton(checked = open, onCheckedChange = { open = it }, modifier = Modifier.testTag("aç")) {
                                Icon(Icons.Filled.Add, contentDescription = "Menü", modifier = Modifier.testTag("artı").graphicsLayer { rotationZ = 45 * checkedProgress })
                            }
                        },
                    ) {
                        for (label in listOf("Not", "Konum")) {
                            FloatingActionButtonMenuItem(onClick = { picked = label; open = false }, text = { Text(label) }, icon = { Icon(Icons.Filled.Edit, contentDescription = null) })
                        }
                    }
                }
                HorizontalFloatingToolbar(expanded = more, colors = FloatingToolbarDefaults.vibrantFloatingToolbarColors(),
                                          leadingContent = { IconButton(onClick = { }) { Icon(Icons.Filled.Favorite, contentDescription = "Beğen") } }) {
                    IconButton(onClick = { more = !more }, modifier = Modifier.testTag("daha")) { Icon(Icons.Filled.MoreVert, contentDescription = "Daha") }
                }
            }
        }

        // C++ fab_menu_opens_from_the_bottom_and_its_button_turns_round, in Kotlin (the progress and the pick shown).
        @Composable
        fun OpeningMenu() {
            var open by remember { mutableStateOf(false) }
            var picked by remember { mutableStateOf("") }
            var progress by remember { mutableStateOf(-1f) }
            Column {
                Box(Modifier.size(400.dp, 600.dp), contentAlignment = Alignment.BottomEnd) {
                    FloatingActionButtonMenu(
                        open,
                        button = {
                            ToggleFloatingActionButton(open, { open = it }, Modifier.testTag("fab")) {
                                progress = checkedProgress
                                Icon(Icons.Filled.Add, contentDescription = "Menü")
                            }
                        },
                    ) {
                        listOf("Belge", "Fotoğraf", "Kişi").forEach { label ->
                            FloatingActionButtonMenuItem(onClick = { picked = label }, text = { Text(label) }, icon = { Icon(Icons.Filled.Add, contentDescription = "") }, modifier = Modifier.testTag(label))
                        }
                    }
                }
                Text("seçilen $picked · ${if (progress == 0f) "kapalı" else if (progress >= 0.99f) "açık" else "arada"}")
            }
        }

        // The C++ test's medium one: 80 dp, 56 once checked, shrinking towards its top end.
        @Composable
        fun MediumToggle() {
            var checked by remember { mutableStateOf(false) }
            Box {
                ToggleFloatingActionButton(checked = checked, onCheckedChange = { checked = it }, modifier = Modifier.testTag("orta"), containerSize = ToggleFloatingActionButtonDefaults.containerSize(80.dp)) {}
            }
        }

        // C++ segmented_buttons_as_compose_1_4, in Kotlin.
        @Composable
        fun TwoSegments() {
            var picked by remember { mutableStateOf(0) }
            SingleChoiceSegmentedButtonRow(Modifier.width(300.dp)) {
                for (i in 0 until 2) {
                    SegmentedButton(picked == i, { picked = i }, SegmentedButtonDefaults.itemShape(i, 2)) { Text(if (i == 0) "Bir" else "Iki") }
                }
            }
        }

        // C++ segmented_buttons_keep_48_dp_of_room_and_draw_40, in Kotlin; also the segments of
        // chips_radio_buttons_and_segments_select (the picked one shown).
        @Composable
        fun RoomySegments() {
            var picked by remember { mutableStateOf(0) }
            var room by remember { mutableStateOf(48) }
            Column {
                CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides room.dp) {
                    SingleChoiceSegmentedButtonRow(Modifier.width(300.dp).testTag("satır")) {
                        for (i in 0 until 3) {
                            SegmentedButton(picked == i, { picked = i }, SegmentedButtonDefaults.itemShape(i, 3), Modifier.testTag("seg$i")) { Text("S$i") }
                        }
                    }
                }
                Text("seçim: $picked", Modifier.padding(top = 8.dp).clickable { room = 0 })
            }
        }

        // C++ multi_choice_segmented_buttons_toggle_each_and_tell_their_roles, in Kotlin.
        @Composable
        fun MultiChoice() {
            var walk by remember { mutableStateOf(true) }
            var bike by remember { mutableStateOf(false) }
            var period by remember { mutableStateOf(0) }
            var carTaps by remember { mutableStateOf(0) }
            Column {
                MultiChoiceSegmentedButtonRow(Modifier.width(300.dp)) {
                    SegmentedButton(checked = walk, onCheckedChange = { walk = it }, shape = SegmentedButtonDefaults.itemShape(0, 3)) { Text("Yürüyüş") }
                    SegmentedButton(checked = bike, onCheckedChange = { bike = it }, shape = SegmentedButtonDefaults.itemShape(1, 3)) { Text("Bisiklet") }
                    SegmentedButton(checked = false, onCheckedChange = { carTaps++ }, shape = SegmentedButtonDefaults.itemShape(2, 3), enabled = false) { Text("Araba") }
                }
                SingleChoiceSegmentedButtonRow(Modifier.width(300.dp).testTag("tekli")) {
                    val names = listOf("Gün", "Hafta", "Ay")
                    for (i in 0 until 3) {
                        if (i == 2) {
                            SegmentedButton(selected = period == i, onClick = { period = i }, shape = SegmentedButtonDefaults.itemShape(i, 3), icon = { Icon(Icons.Filled.Star, contentDescription = "Yıldız") }) { Text(names[i]) }
                        } else {
                            SegmentedButton(selected = period == i, onClick = { period = i }, shape = SegmentedButtonDefaults.itemShape(i, 3)) { Text(names[i]) }
                        }
                    }
                }
                Text("yürüyüş $walk · bisiklet $bike · araba $carTaps · dönem $period")
            }
        }
    }

    kompoz_file!("tests/kt/fab_menu_segmented.kt");

    fn kt_near(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    fn kt_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
        rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
    }

    fn kt_text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
        named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
    }

    fn kt_tap(rule: &KompozTestRule, at: Offset) {
        rule.host().pointer_down(at, -1, 0, PointerType::Touch);
        rule.host().pointer_up(at, -1, 0);
    }

    /// Whether a text shows (a FAB menu item hidden tells nothing).
    fn kt_shows(rule: &KompozTestRule, text: &str) -> bool {
        rule.on_node_with_text(text).exists()
    }

    /// The rounded rect filling `b`: its radius and color.
    fn kt_fill(canvas: &RecordingCanvas, b: Rect) -> Option<(f32, Color)> {
        canvas
            .rects
            .iter()
            .zip(canvas.rect_radii.iter().zip(&canvas.rect_colors))
            .find(|(r, _)| {
                kt_near(r.left, b.left, 0.01) && kt_near(r.top, b.top, 0.01) && kt_near(r.width(), b.width(), 0.01)
            })
            .map(|(_, (radius, color))| (*radius, *color))
    }

    pub fn view_models_maps_and_segments_as_kotlin_writes_them() {
        // The ViewModel's and the maps' line, then the segment checks.
        let rule = KompozTestRule::with_viewport(kompozable_lambda!(|| Mixed()), IntSize::new(400, 800));
        rule.on_node_with_text("öğe 2 · 1 · 3 0 · 1 · 3 · 7").assert_exists();
        rule.on_node_with_text("İki").perform_click();
        rule.on_node_with_text("Kalın").perform_click();
        rule.on_node_with_text("seçim 1 kalın").assert_exists();
        let (header, a, b) = (kt_bounds(&rule, "başlık"), kt_bounds(&rule, "karta"), kt_bounds(&rule, "kartb"));
        // (the header across both lanes)
        assert!(a.top >= header.top + header.height() - 1.0 && (a.top - b.top).abs() < 1.0 && a.left < b.left);
    }

    pub fn tabs_and_fab_menu_as_compose_writes_them() {
        // The C++ test's checks, with WP4a's (kotlin_floating_toolbar.rs): the collapsed toolbar hides its leading
        // "Beğen".
        let rule = KompozTestRule::with_viewport(kompozable_lambda!(|| Navigation()), IntSize::new(400, 800));
        rule.on_node_with_text("seçili 0 · yok · az").assert_exists();
        rule.on_node_with_text("sekme 1").assert_exists();
        rule.on_node_with_text("kayan 1").assert_exists();
        rule.on_node_with_content_description("Beğen").assert_does_not_exist();
        rule.on_node_with_text("kayan 1").perform_click();
        rule.on_node_with_tag("daha").perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("seçili 1 · yok · fazla").assert_exists();
        rule.on_node_with_content_description("Beğen").assert_exists();
        rule.on_node_with_tag("aç").perform_click();
        rule.main_clock().advance_time_by(1500);
        rule.on_node_with_text("Konum").assert_exists();
        // The icon's layer block, kept past the content, read the scope it was in: turned 45° once checked.
        assert!(
            rule.draw()
                .transforms
                .iter()
                .any(|m| kt_near(m.values[0], 0.70710677, 0.001) && kt_near(m.values[1], 0.70710677, 0.001))
        );
        rule.on_node_with_text("Konum").perform_click();
        rule.main_clock().advance_time_by(1500);
        rule.on_node_with_text("seçili 1 · Konum · fazla").assert_exists();
        assert!(!kt_shows(&rule, "Konum"));
        assert!(
            !rule
                .draw()
                .transforms
                .iter()
                .any(|m| kt_near(m.values[0], 0.70710677, 0.001) && kt_near(m.values[1], 0.70710677, 0.001))
        );
    }

    pub fn fab_menu_opens_from_the_bottom_and_its_button_turns_round() {
        let rule = KompozTestRule::new(kompozable_lambda!(|| OpeningMenu()));
        let cs = light_color_scheme();
        let fab = kt_bounds(&rule, "fab");
        assert!(
            kt_near(fab.right, 384.0, 0.01) && kt_near(fab.bottom, 584.0, 0.01) && kt_near(fab.width(), 56.0, 0.01)
        );
        let button = |rule: &KompozTestRule| kt_fill(&rule.draw(), kt_bounds(rule, "fab")).expect("drawn");
        assert_eq!(button(&rule), (16.0, cs.primary_container));
        rule.on_node_with_text("seçilen  · kapalı").assert_exists();
        assert!(!kt_shows(&rule, "Belge"));
        // (the clock by hand: the checks frame by frame see the menu opening)
        rule.main_clock().set_auto_advance(false);
        kt_tap(&rule, fab.center());
        let mut frames = 0;
        while !kt_shows(&rule, "Kişi") && frames < 30 {
            rule.advance_frame();
            frames += 1;
        }
        // The one next to the button first.
        assert!(kt_shows(&rule, "Kişi") && !kt_shows(&rule, "Belge"));
        rule.on_node_with_text("seçilen  · arada").assert_exists();
        crate::common::settle(&rule);
        let (radius, color) = button(&rule);
        assert!(kt_near(radius, 28.0, 0.01) && color == cs.primary);
        rule.on_node_with_text("seçilen  · açık").assert_exists();
        let (top, middle, bottom) = (kt_bounds(&rule, "Belge"), kt_bounds(&rule, "Fotoğraf"), kt_bounds(&rule, "Kişi"));
        assert!(kt_near(top.height(), 56.0, 0.01) && kt_near(middle.top - top.bottom, 4.0, 0.01));
        assert!(kt_near(fab.top - bottom.bottom, 8.0, 0.01) && kt_near(top.right, fab.right, 0.01));
        rule.on_node_with_text("Fotoğraf").perform_click();
        rule.on_node_with_text("seçilen Fotoğraf · açık").assert_exists();
        rule.on_node_with_tag("fab").perform_click();
        crate::common::settle(&rule);
        assert!(!kt_shows(&rule, "Kişi") && button(&rule).0 == 16.0);
        let fab_node = rule.on_node_with_content_description("Menü").fetch_semantics_node();
        assert_eq!(fab_node.config().get(SemanticsProperties::ToggleableState), Some(ToggleableState::Off));

        let sized = KompozTestRule::new(kompozable_lambda!(|| MediumToggle()));
        let b = kt_bounds(&sized, "orta");
        assert!(kt_near(b.left, 0.0, 0.01) && kt_near(b.width(), 80.0, 0.01));
        sized.on_node_with_tag("orta").perform_click();
        sized.wait_for_idle();
        let b = kt_bounds(&sized, "orta");
        assert!(kt_near(b.left, 24.0, 0.01) && kt_near(b.top, 0.0, 0.01) && kt_near(b.width(), 56.0, 0.01), "{b:?}");
    }

    pub fn segmented_buttons_as_compose_1_4() {
        let rule = KompozTestRule::new(kompozable_lambda!(|| TwoSegments()));
        let one = kt_text_bounds(&rule, "Bir");
        // (the clock by hand: the checks on the way see the animation running)
        rule.main_clock().set_auto_advance(false);
        assert!(kt_near(kt_text_bounds(&rule, "Iki").center().x, 225.0, 1.0));
        kt_tap(&rule, kt_text_bounds(&rule, "Iki").center());
        rule.advance_frame();
        rule.advance_frame();
        rule.advance_frame();
        let sliding = kt_text_bounds(&rule, "Iki").center().x;
        assert!(sliding > 225.5 && sliding < 237.5, "{sliding}");
        crate::common::settle(&rule);
        assert!(kt_near(kt_text_bounds(&rule, "Iki").center().x, 238.0, 1.0));
        assert!(kt_near(kt_text_bounds(&rule, "Bir").center().x, one.center().x, 13.5));
    }

    pub fn segmented_buttons_keep_48_dp_of_room_and_draw_40() {
        let rule = KompozTestRule::new(kompozable_lambda!(|| RoomySegments()));
        let (row, seg) = (kt_bounds(&rule, "satır"), kt_bounds(&rule, "seg1"));
        assert!(
            kt_near(row.height(), 48.0, 0.01)
                && kt_near(seg.height(), 40.0, 0.01)
                && kt_near(seg.top, row.top + 4.0, 0.01)
        );
        kt_tap(&rule, Offset::new(seg.center().x, row.top + 2.0));
        rule.wait_for_idle();
        rule.on_node_with_text("seçim: 1").assert_exists();
        rule.on_node_with_text("seçim: 1").perform_click();
        rule.wait_for_idle();
        assert!(kt_near(kt_bounds(&rule, "satır").height(), 40.0, 0.01));
    }

    pub fn multi_choice_segmented_buttons_toggle_each_and_tell_their_roles() {
        let rule = KompozTestRule::new(kompozable_lambda!(|| MultiChoice()));
        rule.on_node_with_text("Bisiklet").perform_click();
        rule.on_node_with_text("Yürüyüş").perform_click();
        rule.on_node_with_text("Araba").perform_click();
        rule.on_node_with_text("Ay").perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("yürüyüş false · bisiklet true · araba 0 · dönem 2").assert_exists();
        let node = |word: &str| rule.on_node_with_text(word).fetch_semantics_node();
        assert_eq!(node("Bisiklet").config().get(SemanticsProperties::ToggleableState), Some(ToggleableState::On));
        assert!(node("Araba").config().contains(SemanticsProperties::Disabled));
        assert_eq!(node("Ay").config().get(SemanticsProperties::Role), Some(Role::RadioButton));
        let group = rule.on_node_with_tag_unmerged("tekli").fetch_semantics_node();
        assert!(group.config().contains(SemanticsProperties::SelectableGroup));
        let canvas = rule.draw();
        let star = Icons::Filled::Star().id();
        assert_eq!(canvas.vectors.iter().filter(|v| **v == star).count(), 1);
        // The check of "Bisiklet" only.
        assert_eq!(canvas.vectors.iter().filter(|v| **v != star).count(), 1);
    }

    pub fn chips_radio_buttons_and_segments_select() {
        // Its segments (in RoomySegments): S2 picked, its check drawn, the three sharing the row.
        let rule = KompozTestRule::new(kompozable_lambda!(|| RoomySegments()));
        rule.on_node_with_text("S2").perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("seçim: 2").assert_exists();
        assert_eq!(rule.draw().vectors.len(), 1);
        assert!(
            kt_near(kt_bounds(&rule, "seg0").width(), 100.0, 1.0) && kt_near(kt_bounds(&rule, "seg2").left, 200.0, 1.0)
        );
    }

    pub fn fab_menu_and_segmented_button_samples() {
        let rule = KompozTestRule::new(kompozable_lambda!(|| SegmentedButtonSingleSelectSample()));
        let selected = |rule: &KompozTestRule, t: &str| {
            rule.on_node_with_text(t).fetch_semantics_node().config().get(SemanticsProperties::Selected)
        };
        assert_eq!(selected(&rule, "Day"), Some(true));
        rule.on_node_with_text("Week").perform_click();
        rule.wait_for_idle();
        assert_eq!((selected(&rule, "Day"), selected(&rule, "Week")), (Some(false), Some(true)));

        let rule = KompozTestRule::new(kompozable_lambda!(|| SegmentedButtonMultiSelectSample()));
        let state = |rule: &KompozTestRule, t: &str| {
            rule.on_node_with_text(t).fetch_semantics_node().config().get(SemanticsProperties::ToggleableState)
        };
        let star = Icons::Filled::StarBorder().id();
        assert!(rule.draw().vectors.contains(&star));
        rule.on_node_with_text("Favorites").perform_click();
        rule.on_node_with_text("Saved").perform_click();
        rule.wait_for_idle();
        assert_eq!(state(&rule, "Favorites"), Some(ToggleableState::On));
        assert_eq!(state(&rule, "Trending"), Some(ToggleableState::Off));
        assert!(!rule.draw().vectors.contains(&star));

        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(|| FloatingActionButtonMenuSample()),
            IntSize::new(400, 800),
        );
        let toggle = || rule.on_node_with_content_description("Toggle menu");
        let described = |rule: &KompozTestRule| {
            rule.on_node_with_content_description("Toggle menu")
                .fetch_semantics_node()
                .config()
                .get(SemanticsProperties::StateDescription)
        };
        assert_eq!(described(&rule), Some("Collapsed".into()));
        toggle().perform_click();
        rule.wait_for_idle();
        assert_eq!(described(&rule), Some("Expanded".into()));
        for label in ["Reply", "Reply all", "Forward", "Snooze", "Archive", "Label"] {
            assert!(kt_shows(&rule, label), "{label}");
        }
        let last = rule.on_node_with_text("Label").fetch_semantics_node();
        let actions = last.config().get(SemanticsActions::CustomActions).expect("the last item's action");
        assert_eq!(actions[0].label, "Close menu");
        rule.run_on_idle(|| (actions[0].action)());
        rule.wait_for_idle();
        assert!(!kt_shows(&rule, "Reply"));
        toggle().perform_click();
        rule.wait_for_idle();
        rule.host().back_invoked();
        rule.wait_for_idle();
        assert!(!kt_shows(&rule, "Archive"));
        toggle().perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("Forward").perform_click();
        rule.wait_for_idle();
        assert!(!kt_shows(&rule, "Forward"));
    }
}

fn main() {
    common::run(&[
        // (toggle and split buttons)
        (
            "toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked",
            toggle_split::toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked,
        ),
        (
            "split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow",
            toggle_split::split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow,
        ),
        ("toggle_button_samples", toggle_split::toggle_button_samples),
        ("split_button_samples", toggle_split::split_button_samples),
        // (button groups)
        (
            "expressive_buttons_and_sliders_as_compose_writes_them",
            group::expressive_buttons_and_sliders_as_compose_writes_them,
        ),
        (
            "button_group_grows_the_held_button_and_squeezes_its_neighbours",
            group::button_group_grows_the_held_button_and_squeezes_its_neighbours,
        ),
        ("button_group_puts_what_does_not_fit_into_a_menu", group::button_group_puts_what_does_not_fit_into_a_menu),
        (
            "connected_button_group_rounds_the_outer_ends_and_the_checked_one",
            group::connected_button_group_rounds_the_outer_ends_and_the_checked_one,
        ),
        ("button_group_samples", group::button_group_samples),
        // (FAB menu and segmented buttons)
        (
            "view_models_maps_and_segments_as_kotlin_writes_them",
            fab_segmented::view_models_maps_and_segments_as_kotlin_writes_them,
        ),
        ("tabs_and_fab_menu_as_compose_writes_them", fab_segmented::tabs_and_fab_menu_as_compose_writes_them),
        (
            "fab_menu_opens_from_the_bottom_and_its_button_turns_round",
            fab_segmented::fab_menu_opens_from_the_bottom_and_its_button_turns_round,
        ),
        ("segmented_buttons_as_compose_1_4", fab_segmented::segmented_buttons_as_compose_1_4),
        (
            "segmented_buttons_keep_48_dp_of_room_and_draw_40",
            fab_segmented::segmented_buttons_keep_48_dp_of_room_and_draw_40,
        ),
        (
            "multi_choice_segmented_buttons_toggle_each_and_tell_their_roles",
            fab_segmented::multi_choice_segmented_buttons_toggle_each_and_tell_their_roles,
        ),
        ("chips_radio_buttons_and_segments_select", fab_segmented::chips_radio_buttons_and_segments_select),
        ("fab_menu_and_segmented_button_samples", fab_segmented::fab_menu_and_segmented_button_samples),
    ]);
}
