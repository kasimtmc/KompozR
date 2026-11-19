//! Kotlin's inline functions in a coroutine, in Kotlin's syntax: their lambdas suspend with it, as Kotlin's do — scope
//! functions (`with(anim) { animateTo(…) }`, `run`, `let`, `apply`, `also`, `takeIf`, `run { }`), `repeat`, a list's
//! `forEach`/`map`/`filter`/`fold`/`any`, a string's `forEachIndexed`, `getOrPut`, `List(n) { }`, `require`'s message,
//! in a `suspend fun` too, `return@forEach`/`return@run` from such a lambda; a safe call and `?:` that suspend. The
//! lambdas really suspend: the coroutine is seen part way. And in a `.kt` file (kompoz_file!). Locals bound to what a
//! generic callee gives are typed as Kotlin infers them (`val got = anim.let { …; it.value }`, `anim.run { … }`,
//! `xs.map { … }`, an `Animatable(0f)`'s value); `it` is the nearest lambda's that has one (`xs.forEach {
//! scope.launch { delay(it.toLong()) } }`).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::RefCell;
use std::rc::Rc;

kompoz! {
    @Composable
    fun ScopeFunctions(go: Boolean, log: (String) -> Unit) {
        val anim = remember { Animatable(0f) }
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            with(anim) { animateTo(100f, tween(100, easing = LinearEasing)) }
            log("with ${anim.value}")
            val half = anim.run {
                animateTo(50f, tween(100, easing = LinearEasing))
                if (value == 50f) return@run "half"
                "other"
            }
            log(half)
            val got: Float = anim.let {
                it.animateTo(20f, tween(50, easing = LinearEasing))
                it.value
            }
            log("let $got")
            anim.apply { snapTo(5f) }.also { log("also ${it.value}") }
            val kept = anim.takeIf {
                it.animateTo(10f, tween(50, easing = LinearEasing))
                it.value == 10f
            }
            log("takeIf ${kept != null}")
            val ran = run {
                delay(30)
                3
            }
            log("run $ran")
        }
        Box(Modifier.testTag("scoped").size(anim.value.dp))
    }

    suspend fun countTo(n: Int): Int {
        var sum = 0
        (1..n).forEach {
            delay(1)
            sum += it
        }
        return sum
    }

    @Composable
    fun Lists(go: Boolean, log: (String) -> Unit) {
        val anim = remember { Animatable(0f) }
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            var total = 0
            listOf(1, 2, 3).forEach {
                delay(100)
                total += it
                log("each $it")
            }
            listOf(1, 2, 3, 4).forEach {
                if (it % 2 == 0) return@forEach
                delay(5)
                log("odd $it")
            }
            repeat(3) { anim.animateTo((it + 1) * 10f, tween(20, easing = LinearEasing)) }
            log("repeat ${anim.value}")
            val doubled = listOf(1, 2, 3).map {
                delay(1)
                it * 2
            }
            val big = doubled.filter {
                delay(1)
                it > 2
            }
            val sum = big.fold(0) { acc, x ->
                delay(1)
                acc + x
            }
            val any = listOf(1, 2).any {
                delay(1)
                it == 2
            }
            log("$total $doubled $big $sum $any")
            "ab".forEachIndexed { i, c ->
                delay(1)
                log("$i$c")
            }
            val cache = mutableMapOf<String, Int>()
            val first = cache.getOrPut("a") {
                delay(1)
                7
            }
            val again = cache.getOrPut("a") {
                delay(1)
                8
            }
            log("cache $first $again")
            val made = List(3) {
                delay(1)
                it * it
            }
            log("made $made")
            require(total == 6) {
                delay(1)
                "never"
            }
            log("sum ${countTo(4)}")
        }
    }
}

kompoz! {
    @Composable
    fun Inferred(go: Boolean, log: (String) -> Unit) {
        val anim = remember { Animatable(0f) }
        val scope = rememberCoroutineScope()
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            val got = anim.let {
                it.animateTo(20f, tween(50, easing = LinearEasing))
                it.value
            }
            val ran = anim.run { targetValue + 1f }
            val doubled = with(anim) { value * 2 }
            val floats = listOf(1, 2).map { it * 1.5f }
            val folded = floats.fold(0f) { acc, x -> acc + x }
            val most = listOf(3, 1).maxOf { it.toFloat() }
            val ints = "${got.toInt()} ${ran.toInt()} ${doubled.toInt()}"
            log("$ints ${floats[1].toInt()} ${folded.toInt()} ${most.toInt()}")
            listOf(30, 10).forEach { scope.launch { delay(it.toLong()); log("after $it") } }
            listOf(1).forEach { run { log("run $it") } }
        }
    }
}

kompoz_file!("tests/kt/inline_suspend.kt");

fn width(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().width()
}

fn scope_functions_suspend_with_the_coroutine() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let (l, g) = (log.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        ScopeFunctions(g.value(), move |s: String| l.borrow_mut().push(s));
    }));
    rule.wait_for_idle();
    go.set(true);
    rule.advance_time_by(60);
    // Part way through `with`'s animation: its lambda is suspended in the coroutine.
    let w = width(&rule, "scoped");
    assert!(w > 10.0 && w < 90.0, "{w}");
    assert!(log.borrow().is_empty());
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["with 100.0", "half", "let 20.0", "also 5.0", "takeIf true", "run 3"]);
    assert_eq!(width(&rule, "scoped"), 10.0);
}

fn collection_functions_suspend_with_the_coroutine() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let (l, g) = (log.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Lists(g.value(), move |s: String| l.borrow_mut().push(s));
    }));
    rule.wait_for_idle();
    go.set(true);
    rule.advance_time_by(150);
    // The forEach's lambda suspended between its elements.
    assert_eq!(*log.borrow(), ["each 1"]);
    rule.wait_for_idle();
    assert_eq!(
        *log.borrow(),
        [
            "each 1",
            "each 2",
            "each 3",
            "odd 1",
            "odd 3",
            "repeat 30.0",
            "6 [2, 4, 6] [4, 6] 10 true",
            "0a",
            "1b",
            "cache 7 7",
            "made [0, 1, 4]",
            "sum 10",
        ]
    );
}

fn inline_functions_suspend_in_a_kt_file() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let target: MutableState<Option<f32>> = mutable_state_of(None);
    let (l, g, t) = (log.clone(), go.clone(), target.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Stepper(g.value(), t.value(), move |s: String| l.borrow_mut().push(s));
    }));
    rule.wait_for_idle();
    go.set(true);
    rule.advance_time_by(200);
    // The first step done, the second on its way.
    assert_eq!(*log.borrow(), ["at 10.0"]);
    let w = width(&rule, "stepper");
    assert!(w > 10.0 && w < 20.0, "{w}");
    rule.wait_for_idle();
    // `missing?.animateTo` skipped, `there?.animateTo` animated, `?:` fetched (suspending).
    assert_eq!(*log.borrow(), ["at 10.0", "at 20.0", "at 30.0", "n 2 40.0"]);
    // Again with a target: `?:`'s right side not run.
    log.borrow_mut().clear();
    go.set(false);
    rule.wait_for_idle();
    target.set(Some(5.0));
    go.set(true);
    rule.wait_for_idle();
    assert_eq!(log.borrow().last().map(String::as_str), Some("n 5 40.0"));
}

fn generic_results_and_nested_its_are_kotlins() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let (l, g) = (log.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Inferred(g.value(), move |s: String| l.borrow_mut().push(s));
    }));
    rule.wait_for_idle();
    go.set(true);
    rule.wait_for_idle();
    // The values typed as Kotlin infers them (their `toInt()` the numbers'); the forEach's `it` in the coroutines it
    // launches (which take none), each delayed by its element.
    assert_eq!(*log.borrow(), ["20 21 40 3 4 3", "run 1", "after 10", "after 30"]);
}

tests![
    generic_results_and_nested_its_are_kotlins,
    scope_functions_suspend_with_the_coroutine,
    collection_functions_suspend_with_the_coroutine,
    inline_functions_suspend_in_a_kt_file,
];
