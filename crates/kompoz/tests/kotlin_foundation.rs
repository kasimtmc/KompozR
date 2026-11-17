//! Foundation's components in Kotlin's syntax, as a Compose user writes them: Box, Row, Column and Spacer with their
//! arrangements, alignments and the size, padding and scope modifiers, right to left; BasicText's wrapping, overflow
//! and layout; clicks, toggles, selections and combined clicks; backgrounds, borders, a Canvas and Images. The same
//! layouts, semantics and drawings the Rust syntax's foundation tests check (crates/kompoz-foundation/tests).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::semantics::SemanticsProperties;
use std::cell::RefCell;
use std::rc::Rc;

/// A node's bounds in the root: x, y, width, height (px; the test rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let b = rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

// ---- Box, Row, Column, Spacer --------------------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Weights() {
        Row(Modifier.width(300.dp)) {
            Box(Modifier.testTag("a").weight(1f).height(10.dp))
            Box(Modifier.testTag("b").weight(2f).height(10.dp))
        }
    }

    @Composable
    fun Arranged() {
        Column(
            Modifier.size(200.dp, 200.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(Modifier.testTag("first").size(50.dp))
            Box(Modifier.testTag("second").size(100.dp, 20.dp))
        }
    }

    @Composable
    fun Aligned() {
        Box(Modifier.size(100.dp), contentAlignment = Alignment.Center) {
            Box(Modifier.testTag("centered").size(20.dp))
            Box(Modifier.testTag("corner").size(10.dp).align(Alignment.BottomEnd))
        }
    }

    @Composable
    fun Mirrored() {
        Column {
            Row(Modifier.width(100.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                Box(Modifier.testTag("a").size(10.dp))
                Box(Modifier.testTag("b").size(20.dp))
                Box(Modifier.testTag("c").size(30.dp))
            }
            CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
                Row(Modifier.testTag("rtl").width(100.dp)) {
                    Box(Modifier.testTag("x").size(10.dp))
                    Box(Modifier.testTag("y").size(20.dp))
                }
                Row(Modifier.testTag("absolute").width(100.dp), horizontalArrangement = Arrangement.Absolute.Left) {
                    Box(Modifier.testTag("p").size(10.dp))
                    Box(Modifier.testTag("q").size(20.dp))
                }
            }
        }
    }

    @Composable
    fun Spaced() {
        Column {
            Row {
                Spacer(Modifier.width(30.dp))
                Box(Modifier.testTag("after").size(10.dp))
            }
            Row(Modifier.width(100.dp)) {
                Spacer(Modifier.weight(1f))
                Box(Modifier.testTag("end").size(10.dp))
            }
            Spacer(Modifier.testTag("spacer").height(7.dp))
            Box(Modifier.testTag("padded").padding(10.dp)) {
                Box(Modifier.testTag("inner").size(50.dp))
            }
            Box(Modifier.width(200.dp)) {
                Box(Modifier.padding(start = 4.dp, top = 6.dp).fillMaxWidth(0.5f).height(5.dp).testTag("half"))
            }
        }
    }
}

fn rows_share_by_weight() {
    let rule = KompozTestRule::new(Weights);
    assert_eq!(bounds(&rule, "a"), [0.0, 0.0, 100.0, 10.0]);
    assert_eq!(bounds(&rule, "b"), [100.0, 0.0, 200.0, 10.0]);
}

fn columns_arrange_and_align() {
    let rule = KompozTestRule::new(Arranged);
    assert_eq!(bounds(&rule, "first")[..2], [75.0, 0.0]);
    assert_eq!(bounds(&rule, "second")[..2], [50.0, 60.0]);
}

fn boxes_align() {
    let rule = KompozTestRule::new(Aligned);
    assert_eq!(bounds(&rule, "centered")[..2], [40.0, 40.0]);
    assert_eq!(bounds(&rule, "corner")[..2], [90.0, 90.0]);
}

fn rows_arrange_and_mirror_right_to_left() {
    let rule = KompozTestRule::new(Mirrored);
    assert_eq!([bounds(&rule, "a")[0], bounds(&rule, "b")[0], bounds(&rule, "c")[0]], [0.0, 30.0, 70.0]);
    let rtl = bounds(&rule, "rtl")[0];
    assert_eq!([bounds(&rule, "x")[0] - rtl, bounds(&rule, "y")[0] - rtl], [90.0, 70.0]);
    let absolute = bounds(&rule, "absolute")[0];
    assert_eq!([bounds(&rule, "p")[0] - absolute, bounds(&rule, "q")[0] - absolute], [0.0, 10.0]);
}

fn spacers_padding_and_fractions() {
    let rule = KompozTestRule::new(Spaced);
    assert_eq!(bounds(&rule, "after")[0], 30.0);
    assert_eq!(bounds(&rule, "end")[0], 90.0);
    assert_eq!(rule.on_node_with_tag("spacer").layout_node().size(), IntSize::new(0, 7));
    let (padded, inner) = (bounds(&rule, "padded"), bounds(&rule, "inner"));
    assert_eq!([inner[0] - padded[0], inner[1] - padded[1], inner[2], padded[2]], [10.0, 10.0, 50.0, 70.0]);
    // (the padding before the fraction: half of what is left of 200 after it)
    let half = bounds(&rule, "half");
    assert_eq!([half[2], half[3]], [98.0, 5.0]);
}

// ---- BasicText ---------------------------------------------------------------------------------------------------

kompoz! {
    // (C++'s FakeTextMeasurer's numbers: 80 px hold 10 characters of 16 px, a line is 20 px)
    @Composable
    fun Texts(onLines: (Int) -> Unit) {
        val style = TextStyle(fontSize = 16.sp)
        Column {
            BasicText("bir iki üç dört beş", Modifier.width(80.dp).testTag("wrap"), style = style)
            BasicText(
                "bir iki üç dört beş",
                modifier = Modifier.width(80.dp).testTag("ellipsis"),
                style = style,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            BasicText("satır\n\nboş satır", modifier = Modifier.testTag("breaks"), style = style)
            BasicText(
                "bir iki üç dört beş altı",
                modifier = Modifier.width(80.dp),
                style = style,
                onTextLayout = { onLines(it.lineCount) },
            )
        }
    }
}

fn basic_text_wraps_cuts_and_tells_its_layout() {
    let lines = Rc::new(RefCell::new(Vec::new()));
    let l = lines.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Texts(move |n: i32| l.borrow_mut().push(n));
    }));
    assert_eq!(bounds(&rule, "wrap")[3], 40.0); // "bir iki üç" / "dört beş"
    assert_eq!(bounds(&rule, "ellipsis")[3], 20.0);
    assert_eq!(bounds(&rule, "breaks")[3], 60.0); // the empty line counts
    let texts = rule.drawn_texts();
    assert!(texts.iter().any(|t| t == "bir iki ü…"), "{texts:?}");
    assert_eq!(*lines.borrow(), [3]);
}

// ---- Clicks, toggles, selections -----------------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Counter() {
        var count by remember { mutableStateOf(0) }
        Column {
            BasicText("Count: $count")
            BasicText("+", modifier = Modifier.clickable(role = Role.Button) { count++ })
        }
    }

    @Composable
    fun Choices() {
        var checked by remember { mutableStateOf(false) }
        var selected by remember { mutableStateOf("a") }
        Column {
            Row(Modifier.testTag("check").toggleable(value = checked, onValueChange = { checked = it }, role = Role.Checkbox)) {
                BasicText(if (checked) "açık" else "kapalı")
            }
            for (option in listOf("a", "b")) {
                Row(
                    Modifier
                        .testTag("option $option")
                        .selectable(selected = selected == option, onClick = { selected = option }, role = Role.RadioButton)
                ) {
                    BasicText(option)
                }
            }
            BasicText("seçili: $selected")
        }
    }

    @Composable
    fun Combined(log: (String) -> Unit) {
        Box(
            Modifier
                .size(100.dp)
                .testTag("combined")
                .combinedClickable(onLongClick = { log("long") }, onDoubleClick = { log("double") }) { log("click") }
        )
    }
}

fn clicks_count() {
    let rule = KompozTestRule::new(Counter);
    rule.on_node_with_text("Count: 0").assert_exists();
    rule.on_node_with_text("+").assert_has_click_action().perform_click();
    rule.on_node_with_text("+").perform_click();
    rule.on_node_with_text("Count: 2").assert_exists();
    let role = rule.on_node_with_text("+").fetch_semantics_node().config().get(SemanticsProperties::Role);
    assert_eq!(role, Some(Role::Button));
}

fn toggles_and_selections() {
    let rule = KompozTestRule::new(Choices);
    rule.on_node_with_tag("check").assert_is_toggleable().assert_is_off();
    rule.on_node_with_text("kapalı").assert_exists();
    rule.on_node_with_tag("check").perform_click();
    rule.on_node_with_tag("check").assert_is_on();
    rule.on_node_with_text("açık").assert_exists();
    rule.on_node_with_tag("option a").assert_is_selectable().assert_is_selected();
    rule.on_node_with_tag("option b").assert_is_not_selected().perform_click();
    rule.on_node_with_tag("option b").assert_is_selected();
    rule.on_node_with_tag("option a").assert_is_not_selected();
    rule.on_node_with_text("seçili: b").assert_exists();
}

fn combined_clicks() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Combined(move |e: String| l.borrow_mut().push(e));
    }));
    let target = rule.on_node_with_tag("combined");
    target.perform_touch_input(|t| t.click());
    rule.wait_for_idle();
    assert_eq!(std::mem::take(&mut *log.borrow_mut()), ["click"]);
    target.perform_touch_input(|t| t.double_click());
    rule.wait_for_idle();
    assert_eq!(std::mem::take(&mut *log.borrow_mut()), ["double"]);
    target.perform_touch_input(|t| t.long_click());
    rule.wait_for_idle();
    assert_eq!(std::mem::take(&mut *log.borrow_mut()), ["long"]);
}

// ---- Backgrounds, borders, Canvas, Image ---------------------------------------------------------------------------

kompoz! {
    val Red = Color(0xFFFF0000)
    val Blue = Color(0xFF0000FF)

    val Dot = ImageVector.Builder(defaultWidth = 10.dp, defaultHeight = 10.dp, viewportWidth = 10f, viewportHeight = 10f)
        .addPath(pathData = addPathNodes("M0,0h10v10h-10z"), fill = SolidColor(Blue))
        .build()

    @Composable
    fun Painted(onDraw: () -> Unit) {
        Column {
            Box(modifier = Modifier.size(20.dp).background(Red))
            Box(modifier = Modifier.size(20.dp).background(Red, shape = RoundedCornerShape(4.dp)))
            Box(modifier = Modifier.size(20.dp).border(2.dp, Blue, CircleShape).testTag("bordered"))
            Canvas(Modifier.size(10.dp).semantics { contentDescription = "çizim" }) {
                onDraw()
                drawRect(Blue, size = Size(5f, 5f))
            }
            Image(rememberVectorPainter(Dot), contentDescription = "nokta", modifier = Modifier.size(10.dp))
            Image(ColorPainter(Red), contentDescription = null, modifier = Modifier.size(10.dp).testTag("süs"))
        }
    }
}

fn backgrounds_borders_canvases_and_images() {
    let drawn = Rc::new(RefCell::new(0));
    let d = drawn.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        Painted(move || *d.borrow_mut() += 1);
    }));
    let red = Color(0xFFFF0000_i64);
    let c = rule.draw();
    // (a rectangle as a rect, a rounded one as a rounded rect, both in the background's color; the canvas's rect)
    let reds: Vec<(Rect, f32)> = c
        .rects
        .iter()
        .zip(&c.rect_colors)
        .zip(&c.rect_radii)
        .filter(|((_, k), _)| **k == red)
        .map(|((r, _), q)| (*r, *q))
        .collect();
    assert_eq!(reds[0], (Rect::new(0.0, 0.0, 20.0, 20.0), 0.0), "{reds:?}");
    assert_eq!((reds[1].0, reds[1].1), (Rect::new(0.0, 20.0, 20.0, 40.0), 4.0));
    assert!(!c.strokes.is_empty() || !c.paths.is_empty(), "the border");
    assert!(*drawn.borrow() > 0, "the canvas drew");
    rule.on_node_with_content_description("çizim").assert_exists();
    rule.on_node_with_content_description("nokta").assert_exists();
    let decorative = rule.on_node_with_tag("süs").fetch_semantics_node();
    assert!(decorative.config().get(SemanticsProperties::ContentDescription).is_none());
}

kompoz! {
    // (a component of one's own taking an interface, `Shape`, passed on to what takes one)
    @Composable
    fun Tag(text: String, modifier: Modifier = Modifier, shape: Shape = RoundedCornerShape(4.dp)) {
        Box(modifier.background(Red, shape).border(1.dp, Blue, shape).padding(4.dp)) {
            BasicText(text, style = TextStyle(fontSize = 16.sp))
        }
    }

    @Composable
    fun Tags() {
        Column {
            Tag("etiket", Modifier.testTag("rounded"))
            Tag("daire", Modifier.testTag("circle").size(30.dp), shape = CircleShape)
        }
    }
}

fn a_component_takes_a_shape() {
    let rule = KompozTestRule::new(Tags);
    let red = Color(0xFFFF0000_i64);
    let c = rule.draw();
    let reds: Vec<(Rect, f32)> = c
        .rects
        .iter()
        .zip(&c.rect_colors)
        .zip(&c.rect_radii)
        .filter(|((_, k), _)| **k == red)
        .map(|((r, _), q)| (*r, *q))
        .collect();
    let (rounded, circle) = (bounds(&rule, "rounded"), bounds(&rule, "circle"));
    assert_eq!(reds.len(), 2, "{reds:?}");
    assert_eq!((reds[0].0.width(), reds[0].0.height(), reds[0].1), (rounded[2], rounded[3], 4.0));
    assert_eq!((reds[1].0.top, reds[1].1), (circle[1], 15.0));
}

tests![
    rows_share_by_weight,
    columns_arrange_and_align,
    boxes_align,
    rows_arrange_and_mirror_right_to_left,
    spacers_padding_and_fractions,
    basic_text_wraps_cuts_and_tells_its_layout,
    clicks_count,
    toggles_and_selections,
    combined_clicks,
    backgrounds_borders_canvases_and_images,
    a_component_takes_a_shape,
];
