//! (stage 14 WPB) The C++ Kompoz's Kotlin-syntax tests of functions and control flow (tests/kompozable_tests.cpp
//! `class_properties_by_delegates_and_accessors`, `suspend_functions`, `kotlins_order_of_evaluation`,
//! `operator_functions`, `extension_properties`, `vararg_parameters`, `lateinit_properties`,
//! `try_as_a_value_and_throw`, `overloads_by_arguments_and_types`, `top_level_vars_and_delegated_properties`,
//! `exceptions_as_values`, `coroutines_await_what_suspends`), ported by name: the same Kotlin through `kompoz!`, the
//! same lines expected (`coroutines_await_what_suspends`' produceState written as KompozR's takes its block, a WPD
//! note saying so). Then the package's other forms as they run: local and anonymous functions, callable references,
//! a `var` lambdas share, jumps out of inline lambdas, an inline function's suspending twin, a coroutine's
//! `catch (e: CancellationException)`.
//!
//! Kotlin's exceptions are Rust's unwinding panics (`kt::Throwable`): these tests, as every program using Kotlin's
//! `try`, need `panic = "unwind"` (Rust's default).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, has_text};

/// The text of the node whose text starts with `prefix`.
fn line(rule: &KompozTestRule, prefix: &str, expected: &str) {
    rule.on_node_with(has_text(prefix, true, false), false).assert_text_equals(&[expected]);
}

// ---- class_properties_by_delegates_and_accessors -----------------------------------------------------------------

kompoz! {
    // A class's properties by a delegate (var count by mutableStateOf(0) with a private set, by derivedStateOf, by
    // lazy), with get() and set(v) (`field`), in a ViewModel, a plain state holder and an object; read from a
    // composable.
    class CounterViewModel : ViewModel() {
        var count by mutableStateOf(0)
            private set
        val doubled by derivedStateOf { count * 2 }
        val label: String get() = "sayaç $count"
        var step = 1
            set(value) { field = if (value > 0) value else 1 }

        fun increment() { count += step }
    }

    class FormState {
        var text by mutableStateOf("")
        val isValid get() = text.length >= 3
        val greeting by lazy { "merhaba" }
    }

    object Prefs {
        var dark by mutableStateOf(false)
        val mode get() = if (dark) "koyu" else "açık"
    }

    @Kompozable
    fun DelegatedMembers() {
        val vm = viewModel<CounterViewModel>()
        val form = remember { FormState() }
        Column {
            Text("${vm.count} ${vm.doubled} ${vm.label}")
            Text("${form.text}|${form.isValid}|${form.greeting} ${Prefs.mode}")
            Button(onClick = { vm.increment() }) { Text("Artır") }
            Button(onClick = {
                vm.step = -4
                vm.step = vm.step + 2
                form.text = "abc"
                Prefs.dark = !Prefs.dark
            }) { Text("Adım") }
        }
    }
}

fn class_properties_by_delegates_and_accessors() {
    let rule = KompozTestRule::new(DelegatedMembers);
    rule.wait_for_idle();
    rule.on_node_with_text("0 0 sayaç 0").assert_exists();
    rule.on_node_with_text("|false|merhaba açık").assert_exists();
    rule.on_node_with_text("Artır").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("1 2 sayaç 1").assert_exists();
    rule.on_node_with_text("Adım").perform_click();
    rule.on_node_with_text("Artır").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("4 8 sayaç 4").assert_exists();
    rule.on_node_with_text("abc|true|merhaba koyu").assert_exists();
}

// ---- suspend_functions -------------------------------------------------------------------------------------------

kompoz! {
    // suspend fun: a coroutine, awaited where it is called from one (a LaunchedEffect, another suspend fun): at the
    // top level (its result written, or its expression's), a class's, an interface's implemented by a class.
    suspend fun greet(name: String): String {
        delay(100)
        return "merhaba $name"
    }

    suspend fun twice(x: Int) = slowDouble(x) + slowDouble(x)

    suspend fun shortWait() {
        delay(50)
    }

    class Loader {
        var loads = 0
        suspend fun load(n: Int): List<Int> {
            shortWait()
            loads++
            return listOf(1, 2, 3).take(n)
        }
    }

    interface Source {
        suspend fun next(): Int
    }

    class Counting : Source {
        var n = 0
        override suspend fun next(): Int {
            delay(10)
            n++
            return n
        }
    }

    @Kompozable
    fun SuspendFunctions() {
        var text by remember { mutableStateOf("…") }
        LaunchedEffect(Unit) {
            val loader = Loader()
            val source: Source = Counting()
            val g = greet("dünya")
            val items = loader.load(2)
            source.next()
            text = g + " " + items + " " + twice(5) + " " + source.next() + " " + loader.loads
        }
        Text(text)
    }
}

fn suspend_functions() {
    let rule = KompozTestRule::new(SuspendFunctions);
    rule.on_node_with_text("…").assert_exists();
    rule.main_clock().advance_time_by(1500);
    rule.on_node_with_text("merhaba dünya [1, 2] 20 2 1").assert_exists();
}

// ---- kotlins_order_of_evaluation ---------------------------------------------------------------------------------

kompoz! {
    // Kotlin's order of evaluation, left to right: a call's arguments (named ones as written, not as declared), a
    // string template's parts, an operator's operands, a variable read before what changes it.
    class Clicker {
        var n = 0
        fun next(): Int {
            n++
            return n
        }
    }

    fun pairOf(a: Int, b: Int) = "$a,$b"

    data class Spot(val x: Int, val y: Int)

    @Kompozable
    fun ShowPair(a: Int, b: Int) {
        Text("çift $a $b")
    }

    @Kompozable
    fun OrderForms() {
        val c = remember { Clicker() }
        val list = listOf(c.next(), c.next(), c.next())
        val text = "${c.next()}-${c.next()}"
        val sum = c.next() * 10 + c.next()
        val pair = pairOf(c.next(), c.next())
        val spot = Spot(y = c.next(), x = c.next())
        var i = 0
        val read = pairOf(i, i++)
        Column {
            Text("sıra $list $text $sum $pair $spot $read")
            ShowPair(b = c.next(), a = c.next())
        }
    }
}

fn kotlins_order_of_evaluation() {
    let rule = KompozTestRule::new(OrderForms);
    rule.wait_for_idle();
    line(&rule, "sıra", "sıra [1, 2, 3] 4-5 67 8,9 Spot(x=11, y=10) 0,0");
    rule.on_node_with_text("çift 13 12").assert_exists();
}

// ---- operator_functions ------------------------------------------------------------------------------------------

kompoz! {
    // operator fun: a data class's +, -, unary -, ++, [], compareTo, contains (and += from plus), an extension's *, a
    // class's invoke and plusAssign.
    data class Vec2(val x: Int, val y: Int) {
        operator fun plus(o: Vec2) = Vec2(x + o.x, y + o.y)
        operator fun minus(o: Vec2) = Vec2(x - o.x, y - o.y)
        operator fun unaryMinus() = Vec2(-x, -y)
        operator fun inc() = Vec2(x + 1, y + 1)
        operator fun get(i: Int) = if (i == 0) x else y
        operator fun compareTo(o: Vec2) = (x + y) - (o.x + o.y)
        operator fun contains(v: Int) = v == x || v == y
    }

    operator fun Vec2.times(k: Int) = Vec2(x * k, y * k)

    class Meter {
        var total = 0
        operator fun invoke(n: Int): Int {
            total += n
            return total
        }
        operator fun plusAssign(n: Int) {
            total += n
        }
    }

    @Kompozable
    fun OperatorForms() {
        val a = Vec2(1, 2)
        val b = Vec2(3, 4)
        var c = a + b
        c += a
        c++
        val meter = Meter()
        meter += 5
        Text("işleç " + c + " " + (-a) + " " + (b - a) + " " + a * 3 + " " + a[1] + " " + (a < b) + " " + (2 in a) + " " + meter(10) + " " + meter.total)
    }
}

fn operator_functions() {
    let rule = KompozTestRule::new(OperatorForms);
    rule.wait_for_idle();
    line(&rule, "işleç", "işleç Vec2(x=6, y=9) Vec2(x=-1, y=-2) Vec2(x=2, y=2) Vec2(x=3, y=6) 2 true true 15 15");
}

// ---- extension_properties ----------------------------------------------------------------------------------------

kompoz! {
    // Extension properties: a val on String, a generic one on List<T>, a var on a Kotlin class (its setter), one on a
    // data class reading its fields; used before they are declared in the file.
    @Kompozable
    fun ExtensionPropertyForms() {
        val m = remember { Meter() }
        m.doubled = 8
        m.doubled += 2
        Text("eklenti " + "selam".shout + " " + listOf(1, 2, 3).second + " " + m.total + " " + m.doubled + " " + Vec2(3, 4).norm)
    }

    val String.shout: String get() = this + "!"

    val <T> List<T>.second: T get() = this[1]

    var Meter.doubled: Int
        get() = total * 2
        set(value) {
            total = value / 2
        }

    val Vec2.norm get() = x * x + y * y
}

fn extension_properties() {
    let rule = KompozTestRule::new(ExtensionPropertyForms);
    rule.wait_for_idle();
    line(&rule, "eklenti", "eklenti selam! 2 5 10 25");
}

// ---- vararg_parameters -------------------------------------------------------------------------------------------

kompoz! {
    // vararg: none, several, *list spread among them, after another parameter, a generic one.
    fun sumAll(vararg xs: Int): Int = xs.sum()
    fun joinAll(sep: String, vararg parts: String) = parts.joinToString(sep)
    fun <T> bag(vararg items: T): List<T> = items

    @Kompozable
    fun VarargForms() {
        val more = listOf(4, 5)
        Text("çok " + sumAll() + " " + sumAll(1, 2, 3) + " " + sumAll(1, *more, 9) + " " + joinAll("-", "a", "b") + " " + bag("x", "y").size + " " + bag(0, *more, 6))
    }
}

fn vararg_parameters() {
    let rule = KompozTestRule::new(VarargForms);
    rule.wait_for_idle();
    line(&rule, "çok", "çok 0 6 19 a-b 2 [0, 4, 5, 6]");
}

// ---- lateinit_properties -----------------------------------------------------------------------------------------

kompoz! {
    class Ledger(val id: Int) {
        fun label() = "$PREFIX$id"

        companion object {
            const val PREFIX = "hesap-"
        }
    }

    // lateinit var: set later (in init, in a function), of a type without a default (a class made with arguments);
    // read before it is set, it throws, as Kotlin's does.
    class Session {
        lateinit var user: String
        lateinit var ledger: Ledger

        fun open(name: String) {
            user = name
            ledger = Ledger(7)
        }
    }

    @Kompozable
    fun LateinitForms() {
        val session = remember { Session() }
        var early = "?"
        try {
            early = session.user
        } catch (e: Exception) {
            early = "henüz yok"
        }
        session.open("ayşe")
        Text("geç " + early + " " + session.user + " " + session.ledger.label())
    }
}

fn lateinit_properties() {
    let rule = KompozTestRule::new(LateinitForms);
    rule.wait_for_idle();
    line(&rule, "geç", "geç henüz yok ayşe hesap-7");
}

// ---- try_as_a_value_and_throw ------------------------------------------------------------------------------------

// (in a .kt file: Rust's lexer reads `"iki söz"` in `kompoz! { … }` as a literal of a reserved prefix)
kompoz_file!("tests/kt/try_forms.kt");

fn try_as_a_value_and_throw() {
    let rule = KompozTestRule::new(TryForms);
    rule.wait_for_idle();
    line(
        &rule,
        "dene",
        "dene 41 -1 -2 boş 1 14,0 satır yok iki yine iç NoSuchElementException: List is empty. | 7 kod 7 | Refused: kod 2",
    );
}

// ---- overloads_by_arguments_and_types ----------------------------------------------------------------------------

kompoz! {
    // Overloads: functions of one name taking different parameters, as Kotlin chooses between them: by the arguments'
    // number and names, by a literal's type, the one needing no default first, else by their types (C++'s choice);
    // at the top level, as extensions, composables, a class's and an interface's (overridden each).
    fun describe(n: Int): String = "sayı $n"
    fun describe(s: String): String = "yazı $s"
    fun describe(a: Int, b: Int): String = "çift ${a + b}"
    fun describe(label: String, times: Int = 2): String = label.repeat(times)
    fun describe(flag: Boolean, onYes: () -> String): String = if (flag) onYes() else "hayır"

    fun Int.tagged() = "i$this"
    fun String.tagged() = "s$this"

    @Kompozable
    fun Chip(n: Int) {
        Text("çip sayı $n")
    }

    @Kompozable
    fun Chip(text: String, suffix: String = "!") {
        Text("çip $text$suffix")
    }

    interface Sink {
        fun take(x: Int): String
        fun take(x: String): String
    }

    class Printer : Sink {
        override fun take(x: Int) = "p$x"
        override fun take(x: String) = "p'$x'"
        fun put(x: Int, times: Int = 1) = take(x * times)
        fun put(x: String) = take(x)
        fun put(x: String, times: Int = 2) = take(x.repeat(times))  // (put("b") is put(x: String): it needs no default)
        fun both() = put(2) + put("b") + put(2, times = 3) + put("c", 2)
    }

    @Kompozable
    fun OverloadForms() {
        val count = 4
        val name = "ali"
        val sink: Sink = object : Sink {
            override fun take(x: Int) = "o$x"
            override fun take(x: String) = "o'$x'"
        }
        Text("aşırı " + describe(3) + "|" + describe("a") + "|" + describe(1, 2) + "|" + describe(label = "ab") + "|" + describe("c", 3) + "|" +
             describe(true) { "evet" } + "|" + describe(1.5) + "|" + describe(count) + "|" + describe(name) + "|" + 5.tagged() + name.tagged() + "|" +
             Printer().both() + "|" + sink.take(1) + sink.take("z") + "|" + Printer().put("q") + Printer().put("r", 3))
        Chip(7)
        Chip("a")
        Chip("b", suffix = "?")
    }
}

fn overloads_by_arguments_and_types() {
    let rule = KompozTestRule::new(OverloadForms);
    rule.wait_for_idle();
    line(
        &rule,
        "aşırı",
        "aşırı sayı 3|yazı a|çift 3|abab|ccc|evet|ondalık 1.5|sayı 4|yazı ali|i5sali|p2p'b'p6p'cc'|o1o'z'|p'q'p'rrr'",
    );
    rule.on_node_with_text("çip sayı 7").assert_exists();
    rule.on_node_with_text("çip a!").assert_exists();
    rule.on_node_with_text("çip b?").assert_exists();
}

kompoz! {
    // (another file's overload: the C++ test's kompozable_tests_other.cpp)
    fun describe(d: Double): String = "ondalık $d"
}

// ---- top_level_vars_and_delegated_properties ---------------------------------------------------------------------

kompoz! {
    // Top-level vars and delegated properties: a var of the file (changed by a function), a val by lazy (made when
    // first read), a var by mutableStateOf (state: what reads it recomposes when it changes).
    var visits = 0
    private var calls = 0
    val greeting by lazy { "merhaba ${visits + 1}" }
    var sharedCount by mutableStateOf(1)

    fun visit(): Int {
        visits++
        calls += 2
        return visits * 10 + calls
    }

    @Kompozable
    fun TopLevelForms() {
        Text("üst ${visit()} $greeting $sharedCount")
        Button(onClick = { sharedCount += 1 }) { Text("Artır") }
    }
}

fn top_level_vars_and_delegated_properties() {
    let rule = KompozTestRule::new(TopLevelForms);
    rule.wait_for_idle();
    rule.on_node_with_text("üst 12 merhaba 2 1").assert_exists();
    rule.on_node_with_text("Artır").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("üst 24 merhaba 2 2").assert_exists();
}

// ---- exceptions_as_values ----------------------------------------------------------------------------------------

kompoz! {
    // An exception as a value (a class's field, a data class's nullable one, a list's element): held as what it is, as
    // Kotlin's references are: its message, its text, `is` and `as`, and thrown again as what it is.
    class Attempt(val error: Exception, val note: String = "")

    data class Report(val problem: Exception?)

    fun failureReport(): String {
        val a = try { throw Refused(9) } catch (e: Exception) { Attempt(e) }
        val r = Report(IllegalArgumentException("kötü"))
        val list = listOf<Exception>(Refused(1), NumberFormatException("sayı"))
        val again = try { throw a.error } catch (e: Refused) { "yine ${e.code}" }
        val code = if (a.error is Refused) (a.error as Refused).code else 0
        val none = Report(null)
        return "tutulan ${a.error.message} ${a.error} ${r.problem?.message} ${list.joinToString { it.toString() }} $again $code ${none.problem == null}"
    }

    @Kompozable
    fun HeldExceptionForms() {
        Text(failureReport())
    }
}

fn exceptions_as_values() {
    let rule = KompozTestRule::new(HeldExceptionForms);
    rule.wait_for_idle();
    line(
        &rule,
        "tutulan",
        "tutulan kod 9 Refused: kod 9 kötü Refused: kod 1, NumberFormatException: sayı yine 9 9 true",
    );
}

// ---- coroutines_await_what_suspends ------------------------------------------------------------------------------

kompoz! {
    // In a coroutine, a call that suspends (a suspend fun of the file's, a Deferred's await()) is awaited, any other
    // gives its value at once; produceState's block sets `value`.
    suspend fun slowDouble(x: Int): Int {
        delay(200)
        return x * 2
    }

    @Kompozable
    fun Awaits() {
        // (WPD: Compose's `val produced by produceState(0, 21) { value = slowDouble(21) }` once KompozR's produceState gives a
        // State and runs its block on a ProduceStateScope; its value and the state it sets meanwhile)
        val produced = produceState(0, 21) { it.value = slowDouble(21) }
        var both by remember { mutableStateOf("…") }
        val scope = rememberCoroutineScope()
        LaunchedEffect(Unit) {
            val a = scope.async { slowDouble(1) }
            val b = scope.async { slowDouble(2) }
            both = "${a.await() + b.await()} ${"x".uppercase()}"
        }
        Column {
            Text("üretilen $produced")
            Text("ikisi $both")
        }
    }
}

fn coroutines_await_what_suspends() {
    let rule = KompozTestRule::new(Awaits);
    rule.on_node_with_text("üretilen 0").assert_exists();
    rule.on_node_with_text("ikisi …").assert_exists();
    rule.main_clock().advance_time_by(400);
    rule.on_node_with_text("üretilen 42").assert_exists();
    rule.on_node_with_text("ikisi 6 X").assert_exists();
}

// ---- (stage 14 WPB) the rest of the package's Kotlin, as it runs --------------------------------------------------

kompoz! {
    // A `var` a kept lambda assigns (shared, Kotlin's capture by reference), local functions (one calling itself, one
    // assigning a `var` around it), a callable reference, an anonymous function, a `return` out of `forEach`'s lambda,
    // `break`/`continue` out of one in a loop, a nullable function type, a property with accessors.
    var level: Int
        get() = visits * 100
        set(value) {
            visits = value / 100
        }

    fun firstNegative(xs: List<Int>): Int {
        xs.forEach {
            if (it < 0) return it
        }
        return 0
    }

    fun skipping(rows: List<List<Int>>): Int {
        var sum = 0
        for (row in rows) {
            row.forEach {
                if (it == 0) continue
                if (it > 50) break
                sum += it
            }
        }
        return sum
    }

    fun notify(onDone: (() -> Unit)?): String {
        onDone?.invoke()
        return if (onDone == null) "yok" else "var"
    }

    fun twiceOf(x: Int) = x * 2

    @Kompozable
    fun JumpForms() {
        var clicks = 0
        var total = 0
        fun add(n: Int) {
            total += n
        }
        fun fact(n: Int): Int = if (n <= 1) 1 else n * fact(n - 1)
        val doubler = ::twiceOf
        val halve = fun(x: Int): Int { return x / 2 }
        add(fact(4))
        add(doubler(3))
        add(halve(10))
        var seen by remember { mutableStateOf("") }
        level = 300
        Column {
            Text("atla $total ${firstNegative(listOf(3, -2, -5))} ${skipping(listOf(listOf(1, 0, 2), listOf(60, 5), listOf(4)))} ${notify(null)} ${notify { clicks++ }} ${level}")
            Button(onClick = {
                clicks++
                seen = "tık $clicks"
            }) { Text("Tıkla") }
            Text(seen)
        }
    }

    suspend fun waitBoth(block: () -> Unit) {
        twiceInline { delay(10) }
        block()
    }

    inline fun twiceInline(block: () -> Unit) {
        block()
        block()
    }

    @Kompozable
    fun CancelForms(onCancel: () -> Unit) {
        LaunchedEffect(Unit) {
            try {
                delay(100000)
            } catch (e: CancellationException) {
                onCancel()
            }
        }
        var waited by remember { mutableStateOf("…") }
        LaunchedEffect(Unit) {
            waitBoth { waited = "bekledi" }
        }
        Text("iptal $waited")
    }
}

kompoz! {
    // Local functions of every kind (generic, extension, with defaults, suspending), a top-level property with a
    // backing field and accessors, `equals`/`hashCode` overridden, `::f` of an overload its function type picks, a
    // delegate's lambda using the object it is the property of (held weakly: no cycle).
    var stored: Int = 1
        get() = field * 10
        set(value) {
            field = value + 1
        }

    class Money(val cents: Int) {
        override fun equals(other: Any?): Boolean {
            val o = other as? Money ?: return false
            return o.cents == cents
        }
        override fun hashCode(): Int = cents
    }

    fun pick(n: Int) = "i$n"
    fun pick(s: String) = "s$s"

    class Tally {
        var n by mutableStateOf(2)
        val twice by derivedStateOf { n * 2 }
    }

    fun locals(base: Int): String {
        fun Int.plusBase() = this + base
        fun greet(name: String, punct: String = "!") = name + punct
        fun <T> twiceOf(x: T): List<T> = listOf(x, x)
        val f: (String) -> String = ::pick
        stored = 5
        return greet("a") + greet(punct = "?", name = "b") + " " + 3.plusBase() + " " + twiceOf(1) + " " + f("x") + " " +
            stored + " " + (Money(1) == Money(1)) + " " + (Money(1) == Money(2)) + " " + setOf(Money(4), Money(4)).size +
            " " + Tally().twice
    }

    suspend fun waits(): Int {
        suspend fun later(n: Int): Int {
            delay(1)
            return n * 2
        }
        return later(2) + later(3)
    }

    @Kompozable
    fun LocalForms() {
        var waited by remember { mutableStateOf(0) }
        LaunchedEffect(Unit) { waited = waits() }
        Text("yerel ${locals(10)} $waited")
    }
}

fn local_functions_and_accessors() {
    let rule = KompozTestRule::new(LocalForms);
    rule.main_clock().advance_time_by(100);
    rule.wait_for_idle();
    line(&rule, "yerel", "yerel a!b? 13 [1, 1] sx 60 true false 1 4 10");
}

fn functions_and_jumps() {
    let rule = KompozTestRule::new(JumpForms);
    rule.wait_for_idle();
    // (24 + 6 + 5; the first negative; 1, then `continue` goes on to the next row and `break` leaves the rows: Kotlin's
    // loop around the lambda; the lambda ran once)
    line(&rule, "atla", "atla 35 -2 1 yok var 300");
    rule.on_node_with_text("Tıkla").perform_click();
    rule.wait_for_idle();
    // (the click counted on from the one `notify`'s lambda made: the `var` is the lambdas' together)
    rule.on_node_with_text("tık 2").assert_exists();
}

fn cancellation_and_inline_twins() {
    use std::cell::Cell;
    use std::rc::Rc;
    let cancelled = Rc::new(Cell::new(0));
    let shown = mutable_state_of(true);
    let rule = {
        let (cancelled, shown) = (cancelled.clone(), shown.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            if shown.get() {
                let c = cancelled.clone();
                CancelForms(move || c.set(c.get() + 1));
            }
        }))
    };
    rule.on_node_with_text("iptal …").assert_exists();
    rule.main_clock().advance_time_by(100);
    rule.on_node_with_text("iptal bekledi").assert_exists();
    assert_eq!(cancelled.get(), 0);
    // (leaving the composition cancels the coroutine: its `catch (e: CancellationException)` runs)
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(cancelled.get(), 1);
}

fn main() {
    common::run(&[
        ("class_properties_by_delegates_and_accessors", class_properties_by_delegates_and_accessors),
        ("suspend_functions", suspend_functions),
        ("kotlins_order_of_evaluation", kotlins_order_of_evaluation),
        ("operator_functions", operator_functions),
        ("extension_properties", extension_properties),
        ("vararg_parameters", vararg_parameters),
        ("lateinit_properties", lateinit_properties),
        ("try_as_a_value_and_throw", try_as_a_value_and_throw),
        ("overloads_by_arguments_and_types", overloads_by_arguments_and_types),
        ("top_level_vars_and_delegated_properties", top_level_vars_and_delegated_properties),
        ("exceptions_as_values", exceptions_as_values),
        ("coroutines_await_what_suspends", coroutines_await_what_suspends),
        ("functions_and_jumps", functions_and_jumps),
        ("local_functions_and_accessors", local_functions_and_accessors),
        ("cancellation_and_inline_twins", cancellation_and_inline_twins),
    ]);
}
