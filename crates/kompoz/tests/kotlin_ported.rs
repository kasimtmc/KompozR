//! The C++ Kompoz's Kotlin-syntax tests (tests/kompozable_tests.cpp) that stage 9's Kotlin takes, ported: the same
//! Kotlin (as Compose writes it where the C++ Kompoz's was looser) through `kompoz!` and `kompoz_file!`, the same
//! behaviour asserted. What they use of later stages (data classes and enums, Material's components, object
//! expressions) is left out or written as Rust types Kotlin calls, as tools/stage9-followups.md lists; Material's
//! theme (AppTheme, the inbox's dark surface) is material3's.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, is_heading};
use kompoz::ui::semantics::SemanticsProperties;
use std::cell::RefCell;
use std::rc::Rc;

// ---- structs_by_name ---------------------------------------------------------------------------------------------

/// A Rust struct Kotlin makes with its arguments by name, in any order (the C++ test's: a C++ struct).
#[derive(Clone, Debug, PartialEq, KotlinProperties)]
pub struct Size2 {
    pub width: i32,
    pub height: i32,
}

/// `Size2(width = 0, height = 0)`.
#[named(type)]
pub fn Size2(#[default(0)] width: i32, #[default(0)] height: i32) -> Size2 {
    Size2 { width, height }
}

kompoz! {
    @Kompozable
    fun StructsByName() {
        val s = Size2(height = 3, width = 2)
        Column {
            Text("${s.width}x${s.height}", modifier = Modifier.testTag("boyut"))
            BasicText("stil", style = TextStyle(fontSize = 30.sp), maxLines = 1, modifier = Modifier.testTag("stil"))
        }
    }
}

fn structs_by_name() {
    let rule = KompozTestRule::new(StructsByName);
    rule.on_node_with_tag("boyut").assert_text_equals(&["2x3"]);
    let style = rule.on_node_with_tag("stil").fetch_semantics_node();
    assert!(style.size().height > 30, "30 sp of text and its line's room: {:?}", style.size());
}

// ---- another_files_composables, cpp_calls_the_modules_kotlin -----------------------------------------------------

/// What `StickerRow` (tests/kt/stickers.kt) shows: Kotlin's `data class Sticker(val label: String, val count: Int =
/// 0)`, a Rust struct here (data classes come with stage 14).
#[derive(Clone, Debug, PartialEq, KotlinProperties)]
pub struct Sticker {
    pub label: String,
    pub count: i32,
}

/// `Sticker(label, count = 0)`.
#[named(type)]
pub fn Sticker(label: impl Into<String>, #[default(0)] count: i32) -> Sticker {
    Sticker { label: label.into(), count }
}

kompoz_file!("tests/kt/stickers.kt");

kompoz! {
    // (another file's composable, by name and with its defaults; its local read and provided)
    @Kompozable
    fun OtherFile() {
        Column {
            StickerRow(suffix = "!", sticker = Sticker(count = 3, label = "yeni"))
            StickerRow(Sticker("eski"))
            Text("ton: ${LocalStickerTone.current}")
            CompositionLocalProvider(LocalStickerTone provides "neşeli") {
                Text("ton: ${LocalStickerTone.current}")
            }
        }
    }
}

fn another_files_composables() {
    let rule = KompozTestRule::new(OtherFile);
    rule.on_node_with_text("yeni:3!").assert_exists();
    rule.on_node_with_text("eski:0").assert_exists();
    rule.on_node_with_text("ton: sakin").assert_exists();
    rule.on_node_with_text("ton: neşeli").assert_exists();
}

/// Rust calls the module's Kotlin (another file's composable, its parameters in place).
fn rust_calls_the_modules_kotlin() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| StickerRow(Sticker("çıkartma", 3), "".into(), "!".into())));
    rule.on_node_with_text("çıkartma:3!").assert_exists();
    let rule = KompozTestRule::new(kompozable_lambda!(|| StickerRow(Sticker("çıkartma", 3), prefix = "> ".into())));
    rule.on_node_with_text("> çıkartma:3").assert_exists();
}

// ---- lazy_items_effects_and_locals ------------------------------------------------------------------------------

/// A mail of the inbox: no `==`, so a list of it is never compared and its composable isn't skipped.
#[derive(Clone, KotlinProperties)]
pub struct Mail {
    pub from: String,
}

kompoz! {
    @Kompozable
    fun Inbox(mails: List<Mail>, log: (String) -> Unit) {
        DisposableEffect(mails.size) {
            log("açıldı ${mails.size}")
            onDispose { log("kapandı") }
        }
        MaterialTheme(colorScheme = darkColorScheme()) {
            Surface(shape = RoundedCornerShape(8.dp), color = MaterialTheme.colorScheme.surface) {
                CompositionLocalProvider(LocalContentColor provides Color.Red) {
                    LazyColumn(modifier = Modifier.height(300.dp)) {
                        item { Text("Gelen kutusu") }
                        itemsIndexed(mails) { index, mail -> Text("$index: ${mail.from}", color = LocalContentColor.current) }
                    }
                }
            }
        }
    }
}

fn effects_and_locals() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let show = mutable_state_of(true);
    let (l, s) = (log.clone(), show.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if s.value() {
            let l = l.clone();
            Inbox(vec![Mail { from: "ayşe".into() }, Mail { from: "ali".into() }], move |e: String| {
                l.borrow_mut().push(e)
            });
        }
    }));
    rule.on_node_with_text("Gelen kutusu").assert_exists();
    rule.on_node_with_text("1: ali").assert_exists();
    // (the mails in the content color provided over the dark surface's)
    let canvas = rule.draw();
    let mail = canvas.texts.iter().position(|t| t == "1: ali").unwrap();
    assert_eq!(canvas.colors[mail], Color::Red);
    assert!(
        canvas.path_colors.contains(&dark_color_scheme().surface)
            || canvas.rect_colors.contains(&dark_color_scheme().surface)
    );
    show.set_value(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["açıldı 2", "kapandı"]);
}

// ---- state_forms_as_compose_writes_them -------------------------------------------------------------------------

/// A town: saved by its name.
#[derive(Clone, Debug, PartialEq, KotlinProperties)]
pub struct Town {
    pub name: String,
}

/// `Town(name)`.
pub fn Town(name: impl Into<String>) -> Town {
    Town { name: name.into() }
}

thread_local! {
    /// Compose's `mapSaver` of a town (the C++ test's `townSaver`, Rust's here): Kotlin reads it by its name.
    static town_saver: Saver<Town> = map_saver(
        |t: &Town| vec![("name".to_string(), SavedValue::from(t.name.clone()))],
        |m| m.get("name").and_then(|n| n.as_str()).map(Town),
    );
}

kompoz! {
    @Kompozable
    fun StateForms() {
        val list = rememberLazyListState()
        val todos = remember { mutableStateOf(mutableListOf("bir")) }
        val tags = remember { mutableStateListOf("a", "b") }
        var runs by remember { mutableStateOf(0, neverEqualPolicy()) }
        val town by rememberSaveable(stateSaver = townSaver) { mutableStateOf(Town("İzmir")) }
        Column {
            Text("ilk görünen ${list.firstVisibleItemIndex}")
            Text("yapılacak ${todos.value.size}")
            Text("etiket ${tags.size}")
            Text("${town.name} $runs")
            Button(onClick = {
                todos.update { it.add("iki") }
                tags.add("c")
                runs = runs
            }) { Text("ekle") }
        }
    }
}

fn state_forms_as_compose_writes_them() {
    let rule = KompozTestRule::new(StateForms);
    rule.on_node_with_text("ilk görünen 0").assert_exists();
    rule.on_node_with_text("yapılacak 1").assert_exists();
    rule.on_node_with_text("etiket 2").assert_exists();
    rule.on_node_with_text("İzmir 0").assert_exists();
    rule.on_node_with_text("ekle").perform_click();
    rule.on_node_with_text("yapılacak 2").assert_exists();
    rule.on_node_with_text("etiket 3").assert_exists();
}

// ---- lambdas_with_receivers --------------------------------------------------------------------------------------

kompoz! {
    // (what a receiver lambda's body doesn't declare is the receiver's, a local of the same name first; `this` is the
    // receiver)
    @Kompozable
    fun Receivers(tint: Color) {
        val alpha = 0.5f
        Column {
            Box(modifier = Modifier.size(20.dp).graphicsLayer { this.alpha = alpha; rotationZ += 30f }) { Text("katman") }
            Canvas(modifier = Modifier.size(40.dp)) {
                drawRect(tint, Offset(0f, 0f), Size(size.width / 2, size.height))
                rotate(45f) { drawCircle(tint, center.x / 4) }
            }
        }
    }
}

fn lambdas_with_receivers() {
    let tint = Color(0xFF123456_i64);
    let rule = KompozTestRule::new(kompozable_lambda!(move || Receivers(tint)));
    let drawn = rule.draw();
    assert_eq!(drawn.layer_alphas.iter().filter(|a| **a == 0.5).count(), 1, "{:?}", drawn.layer_alphas);
    let half = 40.0 * rule.density().density / 2.0;
    let rect = drawn.rect_colors.iter().position(|c| *c == tint).map(|i| drawn.rects[i]);
    assert!(rect.is_some_and(|r| r.width() == half), "the tinted rect: {rect:?}");
    assert!(!drawn.transforms.is_empty() || !drawn.layer_transforms.is_empty(), "the rotations");
}

// ---- top_level_vals_and_content_by_name -------------------------------------------------------------------------

/// Kotlin's `data class Spacing(val small: Dp = 4.dp, val large: Dp = 16.dp)`, Rust's here.
#[derive(Clone, Debug, PartialEq, KotlinProperties)]
pub struct Spacing {
    pub small: Dp,
    pub large: Dp,
}

/// `Spacing(small = 4.dp, large = 16.dp)`.
#[named(type)]
pub fn Spacing(#[default(4.dp())] small: Dp, #[default(16.dp())] large: Dp) -> Spacing {
    Spacing { small, large }
}

/// Kotlin's `data class Account(val name: String)`, Rust's here.
#[derive(Clone, Debug, PartialEq, KotlinProperties)]
pub struct Account {
    pub name: String,
}

/// `Account(name)`.
pub fn Account(name: impl Into<String>) -> Account {
    Account { name: name.into() }
}

kompoz! {
    // (top-level vals as Compose's Color.kt and locals are written: their types their values', defined after what
    // reads them, or in another file of the module — StickerGreeting; content = … by name)
    @Kompozable
    fun SpacedTitle(title: String) {
        Text("$title ${LocalSpacing.current.large} $StickerGreeting")
    }

    val LocalSpacing = staticCompositionLocalOf { Spacing() }
    val LocalAccount = compositionLocalOf<Account> { error("LocalAccount was not provided") }
    private val Purple = Color(0xFF6650A4)

    @Kompozable
    fun AppTheme(content: @Kompozable () -> Unit) {
        MaterialTheme(colorScheme = lightColorScheme(), content = content)
    }

    @Kompozable
    fun Locals() {
        AppTheme {
            Column {
                SpacedTitle("geniş")
                CompositionLocalProvider(LocalSpacing provides Spacing(2.dp, 8.dp), content = { SpacedTitle("dar") })
                CompositionLocalProvider(LocalAccount provides Account("Ada")) {
                    Box(modifier = Modifier.background(Purple), content = { Text(LocalAccount.current.name) })
                }
            }
        }
    }
}

fn top_level_vals_and_content_by_name() {
    let rule = KompozTestRule::new(Locals);
    rule.on_node_with_text("geniş 16.0.dp selam !").assert_exists();
    rule.on_node_with_text("dar 8.0.dp selam !").assert_exists();
    rule.on_node_with_text("Ada").assert_exists();
    // (AppTheme's body large: 16 sp)
    let canvas = rule.draw();
    let ada = canvas.texts.iter().position(|t| t == "Ada").unwrap();
    assert_eq!(canvas.text_styles[ada].font_size, 16.0);
    assert_eq!(Purple.with(|c| *c), Color(0xFF6650A4_i64));
    // (no provider, outside a composition: its default, which throws as Kotlin's error() does)
    let message = common::thrown(|| LocalAccount.current()).unwrap_or_default();
    assert_eq!(message, "IllegalStateException: LocalAccount was not provided");
}

// ---- plain_and_extension_functions -------------------------------------------------------------------------------

kompoz! {
    // (x.framed(c) is framed(x, c): an extension's receiver is reached by its members' names, as a lambda's is; a
    // function's type is its body's)
    fun Modifier.framed(color: Color): Modifier =
        border(1.dp, color).padding(4.dp)

    fun Modifier.tallerBy(extra: Dp) = layout { measurable, constraints ->
        val placeable = measurable.measure(constraints)
        val more = extra.roundToPx()
        layout(placeable.width, placeable.height + more) {
            placeable.placeRelative(0, more)
        }
    }

    private fun label(count: Int): String {
        if (count == 1) return "one item"
        return "$count items"
    }

    fun shout(text: String) = text.uppercase() + "!"

    @Kompozable
    fun Extensions() {
        Column {
            Box(modifier = Modifier.testTag("framed").framed(Color.Red).tallerBy(8.dp)) { Text(label(1)) }
            Text(shout(label(3)), modifier = Modifier.semantics { heading() })
        }
    }
}

fn plain_and_extension_functions() {
    let rule = KompozTestRule::new(Extensions);
    rule.on_node_with_text("one item").assert_exists();
    rule.on_node_with_text("3 ITEMS!").assert(is_heading());
    let density = rule.density().density;
    let boxed = rule.on_node_with_tag("framed").fetch_semantics_node().bounds_in_root();
    let text = rule.on_node_with_text("one item").fetch_semantics_node().bounds_in_root();
    // (a border takes no room: 4 dp of padding each side, 8 more above)
    assert_eq!(boxed.height(), text.height() + 16.0 * density);
    assert_eq!(text.top - boxed.top, 12.0 * density);
}

// ---- custom_layouts_as_compose_writes_them (Layout's measure policy; SubcomposeLayout is stage 10's) -------------

kompoz! {
    // (layout(w, h) { … }; a var changed in forEach { } — an inline function's lambda reaches it by reference)
    @Kompozable
    fun Staircase(indent: Dp, content: @Kompozable () -> Unit) {
        Layout(content = content, modifier = Modifier.testTag("stairs")) { measurables, constraints ->
            val placeables = measurables.map { it.measure(constraints.copy(minWidth = 0, minHeight = 0)) }
            val step = indent.roundToPx()
            var width = 0
            var height = 0
            placeables.forEachIndexed { i, p ->
                width = maxOf(width, i * step + p.width)
                height += p.height
            }
            layout(width, height) {
                var x = 0
                var y = 0
                placeables.forEach { placeable ->
                    placeable.placeRelative(x = x, y = y)
                    x += step
                    y += placeable.height
                }
            }
        }
    }

    @Kompozable
    fun CustomLayouts() {
        Column {
            Staircase(24.dp) {
                Box(modifier = Modifier.size(20.dp).testTag("step0"))
                Box(modifier = Modifier.size(20.dp).testTag("step1"))
                Box(modifier = Modifier.size(20.dp).testTag("step2"))
            }
        }
    }
}

fn custom_layouts_as_compose_writes_them() {
    let rule = KompozTestRule::new(CustomLayouts);
    let d = rule.density().density;
    let stairs = rule.on_node_with_tag("stairs").fetch_semantics_node().bounds_in_root();
    let last = rule.on_node_with_tag("step2").fetch_semantics_node().bounds_in_root();
    assert_eq!((last.left - stairs.left, last.top - stairs.top), (48.0 * d, 40.0 * d));
    assert_eq!((stairs.width(), stairs.height()), (68.0 * d, 60.0 * d));
}

// ---- graphics_as_compose_writes_it -------------------------------------------------------------------------------

kompoz! {
    // (a Canvas drawing by name, a shape from a path, a cached drawing, a blur set in its scope, an ImageVector built
    // with groups and paths, a vector painter's content)
    val Ticket = GenericShape { size, _ ->
        moveTo(0f, 0f)
        lineTo(size.width, 0f)
        lineTo(size.width, size.height)
        lineTo(0f, size.height)
        close()
    }

    val BadgeIcon = ImageVector.Builder("Badge", 24.dp, 24.dp, 24f, 24f)
        .addPath(pathData = addPathNodes("M12,2a10,10 0 1,0 0.01,0z"), fill = SolidColor(Color(0xFF1E88E5)))
        .group(rotate = -10f, pivotX = 12f, pivotY = 12f) {
            path(stroke = SolidColor(Color.White), strokeLineWidth = 2f, strokeLineCap = StrokeCap.Round) {
                moveTo(7f, 12f)
                lineTo(10.5f, 15.5f)
                lineTo(17f, 9f)
            }
        }
        .build()

    @Kompozable
    fun Drawings() {
        val ink = Color.White.copy(alpha = 0.5f)
        Column {
            Canvas(modifier = Modifier.size(100.dp, 50.dp).testTag("tuval")) {
                drawCircle(ink, radius = 9f, center = Offset(20f, 25f), style = Stroke(width = 2f))
                rotate(30f, pivot = Offset(50f, 25f)) { drawRect(ink, topLeft = Offset(50f, 10f), size = Size(20f, 20f)) }
                drawLine(ink, start = Offset(0f, 0f), end = Offset(10f, 10f), strokeWidth = 3f, cap = StrokeCap.Round,
                         pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 6f)))
            }
            Box(modifier = Modifier.size(64.dp).clip(Ticket).drawWithCache {
                val half = size.width / 2
                onDrawBehind { drawRect(ink, size = Size(half, size.height)) }
            })
            Box(modifier = Modifier.size(40.dp).blur { blurRadiusSpec = BlurRadiusSpec.radialGradient(0.dp, 12.dp) }.background(ink))
            Image(rememberVectorPainter(BadgeIcon), contentDescription = "Badge")
            val tick = rememberVectorPainter(24.dp, 24.dp, 24f, 24f) { _, _ ->
                Path(pathData = addPathNodes("M5,12l5,5l9,-10"), stroke = SolidColor(Color.Black), strokeLineWidth = 2f)
            }
            Image(tick, contentDescription = "Done")
        }
    }
}

fn graphics_as_compose_writes_it() {
    let rule = KompozTestRule::new(Drawings);
    rule.on_node_with_tag("tuval").assert_exists();
    rule.on_node_with_content_description("Badge").assert_exists();
    rule.on_node_with_content_description("Done").assert_exists();
    let drawn = rule.draw();
    // (the circle's stroke, the rects; the line, the ticket and the vectors' paths)
    assert!(drawn.rects.len() + drawn.strokes.len() >= 3, "{:?} {:?}", drawn.rects, drawn.strokes);
    assert!(!drawn.paths.is_empty() || !drawn.lines.is_empty(), "the paths and the line");
    assert_eq!(BadgeIcon.with(|b| b.root().size()), 2);
    let outline = Ticket.with(|t| t.create_outline(Size(10.0, 10.0), LayoutDirection::Ltr, &rule.density()));
    assert_eq!(outline.bounds().width(), 10.0);
}

// ---- gestures_as_compose_writes_them (its draggable, anchored draggable and drop target — a target given from Rust:
// an object expression is stage 14's — are kotlin_gestures.rs') -----------------------------------------------------

kompoz! {
    // (pointerInput { detectTapGestures(onTap = { }) }, a drag's points in a state list)
    @Kompozable
    fun Gestures(onEvent: (String) -> Unit) {
        val points = remember { mutableStateListOf<Offset>() }
        Column {
            Box(modifier = Modifier.size(100.dp).testTag("dokun").pointerInput(Unit) {
                detectTapGestures(onTap = { onEvent("tap") }, onLongPress = { onEvent("long") })
            })
            Box(modifier = Modifier.size(100.dp).testTag("çiz").pointerInput(Unit) {
                detectDragGestures(onDragStart = { at -> points.clear(); points.add(at) }, onDrag = { change, _ -> points.add(change.position) })
            })
            Text("noktalar ${points.size}", modifier = Modifier.testTag("noktalar"))
        }
    }
}

fn gestures_as_compose_writes_them() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Gestures(move |e: String| l.borrow_mut().push(e));
    }));
    rule.on_node_with_text("noktalar 0").assert_exists();
    rule.on_node_with_tag("dokun").perform_touch_input(|t| t.click());
    rule.wait_for_idle();
    rule.advance_time_by(400);
    assert_eq!(*log.borrow(), ["tap"]);
    rule.on_node_with_tag("dokun").perform_touch_input(|t| t.long_click());
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["tap", "long"]);
    rule.on_node_with_tag("çiz").perform_touch_input(|t| t.swipe(Offset(10.0, 50.0), Offset(90.0, 50.0)));
    rule.wait_for_idle();
    let texts = rule.on_node_with_tag("noktalar").fetch_semantics_node().config().get(SemanticsProperties::Text);
    let shown = texts.and_then(|t| t.first().map(|t| t.text().to_string())).unwrap_or_default();
    let n: i32 = shown.trim_start_matches("noktalar ").parse().unwrap_or(0);
    assert!(n >= 2, "the drag's start and moves: {shown}");
}

tests![
    structs_by_name,
    another_files_composables,
    rust_calls_the_modules_kotlin,
    effects_and_locals,
    state_forms_as_compose_writes_them,
    lambdas_with_receivers,
    top_level_vals_and_content_by_name,
    plain_and_extension_functions,
    custom_layouts_as_compose_writes_them,
    graphics_as_compose_writes_it,
    gestures_as_compose_writes_them,
];
