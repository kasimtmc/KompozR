// (stage 14 WPB) Functions and control flow: exceptions (`throw`, `try`/`catch`/`finally` as a statement and a value,
// a class deriving from a standard exception), overloads (mangled names, chosen by the arguments), operator and infix
// functions, extension properties, varargs and spreads, local functions (one calling itself), a `var` a kept lambda
// assigns (shared), callable references, anonymous functions, jumps out of inline lambdas (written as loops), an
// inline function's suspending twin, nullable and suspending function types, top-level `var`s, delegated properties and
// ones with accessors, a class's delegated (one keyed by its name), accessor and `lateinit` properties, `++` of a
// property as a value.

class Refused(val code: Int) : IllegalStateException("kod $code")

fun parsedOr(text: String, fallback: Int): Int = try {
    text.toInt()
} catch (e: NumberFormatException) {
    fallback
} finally {
    println("parsed")
}

fun firstLabel(items: List<String>): String {
    for (item in items) {
        try {
            if (item.isEmpty()) continue
            if (item == "stop") break
            return item
        } catch (e: Refused) {
            return "kod ${e.code}"
        }
    }
    throw Refused(1)
}

fun describe(n: Int): String = "sayı $n"
fun describe(s: String, times: Int = 2): String = s.repeat(times)

data class Vec2(val x: Int, val y: Int) {
    operator fun plus(o: Vec2) = Vec2(x + o.x, y + o.y)
    operator fun get(i: Int) = if (i == 0) x else y
    infix fun dot(o: Vec2) = x * o.x + y * o.y
}

operator fun Vec2.times(k: Int) = Vec2(x * k, y * k)

val Vec2.length2: Int get() = x * x + y * y

fun total(vararg xs: Int): Int = xs.sum()

var visits = 0
val greeting by lazy { "merhaba" }
val now: Int get() = visits + 1
var level: Int
    get() = visits
    set(value) {
        visits = value
    }

class Counter {
    var count by mutableStateOf(0)
        private set
    var step = 1
        set(value) { field = if (value > 0) value else 1 }
    lateinit var name: String

    fun bump() {
        count += step
    }
}

class Profile(val handle: SavedStateHandle) {
    val user by handle.saved { "ada" }
}

inline fun twice(block: () -> Unit) {
    block()
    block()
}

fun forms(more: List<Int>, onDone: (() -> Unit)?, load: suspend () -> Int): Int {
    var clicks = 0
    val click = { clicks++ }
    click()
    fun fact(n: Int): Int = if (n <= 1) 1 else n * fact(n - 1)
    val double = fun(x: Int): Int { return x * 2 }
    val ref = ::parsedOr
    val a = Vec2(1, 2) + Vec2(3, 4) * 2
    more.forEach {
        if (it < 0) return -1
    }
    onDone?.invoke()
    visits++
    level += now
    val c = Counter()
    c.step = -3
    val before = c.step++
    return fact(4) + double(clicks) + ref("1", 0) + a[0] + (a dot a) + a.length2 + total(1, *more, 3) +
        describe("x").length + before + visits + greeting.length
}

@Composable
fun Coroutines() {
    LaunchedEffect(Unit) {
        twice { delay(1) }
        try {
            delay(1)
        } catch (e: CancellationException) {
            println("cancelled")
        }
    }
}

// Local functions of every kind, a top-level property with a backing field, `equals`/`hashCode` overridden, `::f` of
// an overload its function type picks.
var stored: Int = 1
    get() = field * 10
    set(value) {
        field = value + 1
    }

class Cents(val cents: Int) {
    override fun equals(other: Any?): Boolean = (other as? Cents)?.cents == cents
    override fun hashCode(): Int = cents
}

fun pickOne(n: Int) = "i$n"
fun pickOne(s: String) = "s$s"

fun moreLocals(base: Int): String {
    fun Int.plusBase() = this + base
    fun greet(name: String, punct: String = "!") = name + punct
    fun <T> twiceOf(x: T): List<T> = listOf(x, x)
    val f: (String) -> String = ::pickOne
    stored = 5
    return greet("a") + greet(punct = "?", name = "b") + 3.plusBase() + twiceOf(1) + f("x") + stored + (Cents(1) == Cents(1))
}

suspend fun waits(): Int {
    suspend fun later(n: Int): Int {
        delay(1)
        return n * 2
    }
    return later(2) + later(3)
}
