//! Kotlin's syntax end to end: `kompoz! { … }` translated, compiled through `#[kompozable]` (with only the umbrella
//! crate's paths), composed by the test rule.

#![allow(non_snake_case)]

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, Text};

// The Counter in Kotlin's syntax, its button a Kotlin-defined clickable Box (Material's Button is stage 12).
kompoz! {
    @Kompozable
    fun Counter(start: Int = 0) {
        var count by remember { mutableStateOf(start) }
        Column {
            Text("Count: $count")
            Button({ count++ }) { Text("+") }
        }
    }

    @Composable
    fun Button(onClick: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
        Box(modifier.clickable(onClick = onClick).testTag("button")) {
            content()
        }
    }
}

fn a_kotlin_counter_counts() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| Counter(3)));
    rule.on_node_with_text("Count: 3").assert_exists();
    rule.on_node_with_tag("button").perform_click();
    rule.on_node_with_text("Count: 4").assert_exists();
    rule.on_node_with_tag("button").perform_click();
    rule.on_node_with_text("Count: 5").assert_exists();
}

// Named arguments in any order, defaults left out, a trailing lambda for the last parameter; loops, ifs and whens;
// templates writing numbers as Kotlin does; Kotlin's standard functions.
kompoz! {
    @Kompozable
    fun Labeled(label: String, value: Int = 7, suffix: String = "", content: @Kompozable () -> Unit) {
        Row {
            Text("$label=$value$suffix")
            content()
        }
    }

    @Kompozable
    fun NamedArguments() {
        Column {
            Labeled(suffix = "!", label = "a") { Text("içerik") }
            Labeled("b", 3) { }
            Labeled("c", content = { Text("son") }, suffix = "?")
        }
    }

    @Kompozable
    fun Branches(n: Int) {
        Column {
            for (i in 0 until n) {
                Text("satır $i")
            }
            for (i in 10 downTo 8 step 2) Text("geri $i")
            if (n > 2) Text("çok") else Text("az")
            val word = when (n) {
                0 -> "sıfır"
                1, 2 -> "biraz"
                in 3..5 -> "orta"
                else -> "bol"
            }
            Text(word)
            Text(if (n % 2 == 0) "çift" else "tek")
            var k = 0
            while (true) {
                k++
                if (k < 3) continue
                break
            }
            do { k += 10 } while (k < 25)
            Text("k=$k", modifier = Modifier.testTag("k"))
        }
    }

    @Kompozable
    fun Templates() {
        val ratio = 2.5f
        val whole = 3f
        val name = "Kompoz"
        val big = 3000000000
        val half = whole / 2
        Text("${name.length} harf, $ratio, $whole, ${ratio * 2}, $big, $half", modifier = Modifier.testTag("t"))
    }

    @Kompozable
    fun Collections() {
        val names = listOf("ayla", "ali", "veli")
        Column {
            names.filter { it.startsWith("a") }.forEach { Text(it.uppercase()) }
            Text(names.map { it.length }.joinToString("+"), modifier = Modifier.Companion.testTag("lengths"))
            val text = """
                bir
                  iki
            """.trimIndent()
            Text(text, modifier = Modifier.testTag("indent"))
            val maybe: String? = null
            val some: String? = "dört"
            Text(maybe?.length?.toString() ?: "yok")
            Text("var: ${some?.length ?: 0}")
            if (some != null) Text("akıllı: ${some.uppercase()}")
            for ((i, name) in names.withIndex()) {
                if (i == 2) Text("son: $name")
            }
            Text("${names}", modifier = Modifier.testTag("list"))
        }
    }

    @Kompozable
    fun Ticker() {
        var ticks by remember { mutableStateOf(0) }
        LaunchedEffect(Unit) {
            while (ticks < 3) {
                delay(100)
                ticks += 1
            }
        }
        Text("tik $ticks")
    }
}

fn named_arguments_go_in_place() {
    let rule = KompozTestRule::new(NamedArguments);
    rule.on_node_with_text("a=7!").assert_exists();
    rule.on_node_with_text("içerik").assert_exists();
    rule.on_node_with_text("b=3").assert_exists();
    rule.on_node_with_text("c=7?").assert_exists();
    rule.on_node_with_text("son").assert_exists();
}

fn loops_ifs_and_whens() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| Branches(4)));
    rule.on_node_with_text("satır 0").assert_exists();
    rule.on_node_with_text("satır 3").assert_exists();
    rule.on_node_with_text("satır 4").assert_does_not_exist();
    rule.on_node_with_text("geri 10").assert_exists();
    rule.on_node_with_text("geri 8").assert_exists();
    rule.on_node_with_text("geri 9").assert_does_not_exist();
    rule.on_node_with_text("çok").assert_exists();
    rule.on_node_with_text("orta").assert_exists();
    rule.on_node_with_text("çift").assert_exists();
    rule.on_node_with_tag("k").assert_text_equals(&["k=33"]);
    let none = KompozTestRule::new(kompozable_lambda!(|| Branches(0)));
    none.on_node_with_text("satır 0").assert_does_not_exist();
    none.on_node_with_text("sıfır").assert_exists();
    none.on_node_with_text("az").assert_exists();
}

fn string_templates_write_numbers_as_kotlin_does() {
    let rule = KompozTestRule::new(Templates);
    rule.on_node_with_tag("t").assert_text_equals(&["6 harf, 2.5, 3.0, 5.0, 3000000000, 1.5"]);
}

fn kotlin_standard_functions() {
    let rule = KompozTestRule::new(Collections);
    rule.on_node_with_text("AYLA").assert_exists();
    rule.on_node_with_text("ALI").assert_exists();
    rule.on_node_with_text("VELI").assert_does_not_exist();
    rule.on_node_with_tag("lengths").assert_text_equals(&["4+3+4"]);
    rule.on_node_with_tag("indent").assert_text_equals(&["bir\n  iki"]);
    rule.on_node_with_text("yok").assert_exists();
    rule.on_node_with_text("var: 4").assert_exists();
    rule.on_node_with_text("akıllı: DÖRT").assert_exists();
    rule.on_node_with_text("son: veli").assert_exists();
    rule.on_node_with_tag("list").assert_text_equals(&["[ayla, ali, veli]"]);
}

fn launched_effect_is_a_coroutine() {
    let rule = KompozTestRule::new(Ticker);
    rule.advance_time_by(150);
    rule.on_node_with_text("tik 1").assert_exists();
    rule.advance_time_by(500);
    rule.on_node_with_text("tik 3").assert_exists();
}

// Receiver lambdas: a Canvas's DrawScope, a graphics layer's scope, semantics, pointer input; a Layout's measure
// policy and placement; an extension function of Modifier; a Rust composable and function called from Kotlin.
kompoz! {
    @Kompozable
    fun Drawing() {
        var taps by remember { mutableStateOf(0) }
        Column {
            Canvas(Modifier.size(40.dp).testTag("canvas")) {
                drawRect(Color.Red, topLeft = Offset(10f, 10f), size = Size(size.width / 4, 20f))
                drawCircle(Color.Blue, radius = 5f)
            }
            Box(Modifier.size(10.dp).graphicsLayer { alpha = 0.5f; scaleX = 2f }.semantics { contentDescription = "kutu" })
            Box(Modifier.size(30.dp).testTag("tap").pointerInput(Unit) {
                detectTapGestures(onTap = { taps++ })
            })
            Text("dokunuş $taps")
        }
    }

    @Kompozable
    fun Stacked(modifier: Modifier = Modifier, content: @Kompozable () -> Unit) {
        Layout(content = content, modifier = modifier) { measurables, constraints ->
            val placeables = measurables.map { it.measure(constraints) }
            val width = placeables.maxOf { it.width }
            val height = placeables.sumOf { it.height }
            layout(width, height) {
                var y = 0
                placeables.forEach {
                    it.place(0, y)
                    y += it.height
                }
            }
        }
    }

    fun Modifier.framed(width: Dp = 2.dp): Modifier = this.padding(width).testTag("framed")

    fun Modifier.boxed(): Modifier = framed(4.dp).size(20.dp)

    @Kompozable
    fun Custom() {
        Stacked(Modifier.testTag("stack")) {
            Box(Modifier.size(10.dp, 15.dp).testTag("first"))
            Box(Modifier.size(20.dp).testTag("second"))
            Box(Modifier.boxed())
        }
        RustBadge(label = "rust", count = shout("x").length)
    }

    fun shout(text: String): String = text.uppercase() + "!"

    fun total(xs: List<Int>): Int {
        var sum = 0
        for (x in xs) sum += x
        return sum
    }
}

/// A Rust composable Kotlin calls (with named arguments and a default; restartable, it keeps `label` as a `String`).
#[kompozable]
fn RustBadge(label: impl Into<String>, #[default(1)] count: i32) {
    Text(format!("[{label}:{count}]"));
}

fn receiver_lambdas_draw_and_set() {
    let rule = KompozTestRule::new(Drawing);
    let drawn = rule.draw();
    assert!(
        drawn
            .rects
            .iter()
            .zip(&drawn.rect_colors)
            .any(|(r, c)| *c == Color::Red && (r.left, r.top, r.right, r.bottom) == (10.0, 10.0, 20.0, 30.0)),
        "the canvas's rect: {:?}",
        drawn.rects
    );
    assert!(drawn.layer_alphas.contains(&0.5), "the graphics layer's alpha: {:?}", drawn.layer_alphas);
    rule.on_node_with_content_description("kutu").assert_exists();
    rule.on_node_with_tag("tap").perform_click();
    rule.on_node_with_tag("tap").perform_click();
    rule.on_node_with_text("dokunuş 2").assert_exists();
}

fn a_layout_lambda_measures_and_places() {
    let rule = KompozTestRule::new(Custom);
    let stack = rule.on_node_with_tag("stack").fetch_semantics_node();
    // (the widest child is the boxed one: 20 + 2 × 4 padding)
    assert_eq!((stack.size().width, stack.size().height), (28, 15 + 20 + 28));
    let first = rule.on_node_with_tag("first").fetch_semantics_node().position_in_root();
    let second = rule.on_node_with_tag("second").fetch_semantics_node().position_in_root();
    assert_eq!((first.x, first.y), (0.0, 0.0));
    assert_eq!((second.x, second.y), (0.0, 15.0));
    // (Modifier.boxed(): framed(4.dp) — padding 4, the tag after it, on what is inside — then 20 × 20)
    let framed = rule.on_node_with_tag("framed").fetch_semantics_node();
    assert_eq!((framed.size().width, framed.position_in_root().y), (20, 35.0 + 4.0));
    rule.on_node_with_text("[rust:2]").assert_exists();
    // (Rust calling Kotlin's plain functions and extensions)
    assert_eq!(shout("ok"), "OK!");
    assert_eq!(total(vec![1, 2, 3]), 6);
    let _ = Modifier.framed();
}

kompoz! {
    /** A composable that emits nothing. */
    @Kompozable
    fun Empty() {}

    // (pasted Compose code: @Composable is @Kompozable)
    @Composable private fun AlsoEmpty() {
    }
}

// Kotlin written by another macro: its tokens' positions don't add up (the name comes from the call, the rest from
// the definition), so the text is rebuilt from the tokens.
macro_rules! empty_named {
    ($name:ident) => {
        kompoz! { @Kompozable fun $name() {} }
    };
}
empty_named!(EmptyToo);

kompoz_file!("tests/kt/empty.kt");

kompoz_file!("tests/kt/ui.kt");

thread_local! {
    static LOG: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A Rust function Kotlin calls: what it is given, logged.
fn record(what: &str) -> String {
    LOG.with(|l| l.borrow_mut().push(what.to_string()));
    what.to_string()
}

fn taken_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// A `.kt` file's composables, vals, extension, effects and locals.
fn a_kt_file_composes() {
    taken_log();
    let done = std::rc::Rc::new(std::cell::Cell::new(0));
    let d = done.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        FileScreen("dünya", on_done = move || d.set(d.get() + 1))
    }));
    rule.on_node_with_text("Merhaba, DÜNYA").assert_exists();
    rule.on_node_with_tag("tag-dir").assert_text_equals(&["yön: Rtl"]);
    rule.on_node_with_text("adım: 6").assert_exists();
    assert_eq!(taken_log(), ["başla"]);
    rule.on_node_with_tag("tag-hide").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("görünür: false").assert_exists();
    assert_eq!(taken_log(), ["bitti"]);
    assert_eq!(done.get(), 1);
    assert_eq!(Accent.with(|c| *c), Color::Red);
    // (a backticked Kotlin name that is a Rust keyword: a raw identifier)
    assert_eq!(r#match(3), 9);
    assert_eq!(Greeting, "Merhaba");
}

/// Named arguments are evaluated in the order written.
fn named_arguments_evaluate_in_written_order() {
    taken_log();
    assert_eq!(ordered(), "ab");
    assert_eq!(taken_log(), ["b", "a"]);
}

fn empty_composes() {
    let rule = KompozTestRule::new(Empty);
    rule.wait_for_idle();
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
    rule.set_content(AlsoEmpty);
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
    rule.set_content(EmptyToo);
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
}

/// A `.kt` file's declarations: a composable, and a plain function named by a Rust keyword (`r#in`).
fn a_file_composes() {
    let rule = KompozTestRule::new(FromFile);
    rule.wait_for_idle();
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
    r#in();
}

/// The text is the source's: a group's comments kept, the gaps between top-level tokens rebuilt from their lines
/// and columns (a comment there is spacing), a doc comment once; it starts at its first token's line.
fn text_from_the_source() {
    let here = line!();
    let (text, line, rebuilt) = kompoz_kotlin_macro::__kompoz_text! {
        @Kompozable fun A() { /* kept */ "ğü" ; x }
        /** doc */ /* gone */
        val y = 1 // gone
    };
    assert!(!rebuilt);
    assert_eq!(line, here + 2);
    assert_eq!(text, "        @Kompozable fun A() { /* kept */ \"ğü\" ; x }\n        /** doc */\n        val y = 1");
}

/// Tokens whose positions don't add up (here: repeated by a macro): the text rebuilt from the tokens, newlines
/// where Kotlin needs them, doc comments left out.
fn text_rebuilt_from_tokens() {
    macro_rules! twice {
        ($($t:tt)*) => { kompoz_kotlin_macro::__kompoz_text!($($t)* $($t)*) };
    }
    let (text, line, rebuilt) = twice! {
        /// doc
        @Kompozable fun A(x: Int?) { val y = x!! ; Text("a") }
    };
    assert!(rebuilt);
    assert_eq!(line, 1);
    let one = "@Kompozable fun A(x: Int?) {val y = x!!; Text(\"a\")}";
    assert_eq!(text, format!("{one}\n{one}"));
    // (a postfix `++` ends a statement, a prefix one doesn't; a trailing lambda's label goes on its call's line)
    let (text, _, rebuilt) = twice! {
        fun f(xs: List<Int>) { var i = 0
            i++
            ++i
            xs.forEach lit@{ return@lit } }
    };
    assert!(rebuilt);
    let one = "fun f(xs: List < Int >) {var i = 0\ni++\n++ i\nxs.forEach lit@{return@lit}}";
    assert_eq!(text, format!("{one}\n{one}"));
}

// Kotlin written by another macro (its text rebuilt from its tokens) whose statements end in a postfix `++` and
// with a labeled trailing lambda.
macro_rules! counting_fn {
    ($name:ident) => {
        kompoz! {
            fun $name(xs: List<Int>): Int {
                var i = 0
                i++
                xs.forEach lit@{
                    if (it > 1) return@lit
                    i += it
                }
                return i
            }
        }
    };
}
counting_fn!(counted);

fn rebuilt_text_translates() {
    assert_eq!(counted(vec![1, 2, 1]), 3);
}

/// Runs the tests given one after the other on this thread, made the UI thread; the arguments name the ones to
/// run (all if none).
fn run(tests: &[(&str, fn())]) {
    set_ui_thread();
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        ran += 1;
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{ran} run, {} failed", failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

macro_rules! tests {
    ($($name:ident),* $(,)?) => {
        fn main() {
            run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
}

tests![
    empty_composes,
    a_file_composes,
    text_from_the_source,
    text_rebuilt_from_tokens,
    a_kotlin_counter_counts,
    named_arguments_go_in_place,
    loops_ifs_and_whens,
    string_templates_write_numbers_as_kotlin_does,
    kotlin_standard_functions,
    launched_effect_is_a_coroutine,
    receiver_lambdas_draw_and_set,
    a_layout_lambda_measures_and_places,
    a_kt_file_composes,
    named_arguments_evaluate_in_written_order,
    rebuilt_text_translates,
];
