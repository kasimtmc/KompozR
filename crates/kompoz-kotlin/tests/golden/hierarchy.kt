// (stage 14 WPA) Kotlin's class hierarchies (crates/kompoz/tests/kotlin_hierarchy.rs has them working): interfaces
// with default members, one extending another; abstract and open classes, super calls; a ViewModel; sealed classes
// and interfaces (an enum of their cases), `when` over them; generic classes, functions and extensions; enum entries
// with bodies; inner classes; fun interfaces (a suspending one too); a companion implementing an interface; object
// expressions with properties; Kotlin's `Any` and star projections.

interface Titled {
    val name: String
    fun title(): String = "[$name]"
}

interface Shelf : Titled {
    var picks: Int
    fun count(): Int
}

data class Tag(override val name: String) : Titled

abstract class Plate(val label: String) : Titled {
    abstract fun area(): Float
    open fun describe() = "$label ${area()}"
    override val name: String get() = label
}

class Round(val r: Float) : Plate("daire") {
    override fun area() = 3f * r * r
    override fun describe() = "yuvarlak " + super.describe()
}

open class Stepper {
    var total = 0
    open fun step() = 1
    fun tick() { total += step() }
}

class DoubleStepper : Stepper() {
    override fun step() = 2
}

class PageViewModel : ViewModel() {
    override fun onCleared() {}
}

sealed class Screen(val route: String) {
    data object Home : Screen("home")
    data class Detail(val id: Int) : Screen("detail/$id")
}

sealed interface Figure

data class Disc(val r: Float) : Figure

fun describe(s: Screen): String = when (s) {
    is Screen.Home -> "ana"
    is Screen.Detail -> "ayrıntı ${s.id}"
}

class Crate<T>(val value: T) {
    fun describe() = "Crate($value)"
    fun <R> convert(f: (T) -> R): Crate<R> = Crate(f(value))
}

data class Duo<A, B>(val first: A, val second: B) {
    fun swap() = Duo(second, first)
}

fun <T> firstOr(items: List<T>, fallback: T): T = if (items.isEmpty()) fallback else items.first()

fun <T> List<T>.second(): T = this[1]

enum class Calc(val symbol: String) {
    PLUS("+") {
        override fun apply(a: Int, b: Int) = a + b
    },
    TIMES("*") {
        override fun apply(x: Int, y: Int): Int = x * y
        override fun toString() = "kez"
    };

    abstract fun apply(a: Int, b: Int): Int
}

class Scoreboard(val name: String) {
    var count = 0
    inner class Counter(val step: Int) {
        fun bump() { count += step }
        fun owner() = this@Scoreboard
    }
}

fun interface Check {
    fun test(text: String): Boolean
}

fun interface Fetcher {
    suspend fun load(key: String): String
}

interface Maker<T> {
    fun make(n: Int): T
}

class Widget(val size: Int) {
    companion object : Maker<Widget> {
        override fun make(n: Int) = Widget(n * 10)
    }
}

fun forms(): String {
    val plates: List<Plate> = listOf(Round(1f))
    val stepper: Stepper = DoubleStepper()
    stepper.tick()
    val check = Check { it.isNotEmpty() }
    val anonymous = object : Titled { override val name = "adsız" }
    val all = listOf<Screen>(Screen.Home, Screen.Detail(7))
    val t: Titled = Tag("x")
    return plates[0].describe() + stepper.total + check.test("x") + anonymous.title() + all.size + (t is Tag) +
        (t as? Shelf)?.count() + firstOr(listOf(1), 0) + listOf("a", "b").second() + Calc.TIMES.apply(2, 3)
}

fun checks(x: Any): Boolean = x is String

fun count(xs: List<*>): Int = xs.size

// (WPA-rest) Generic sealed classes (cases of `Nothing`, `out T`), the receiver smart-cast in their members; API
// bases of the second kind (a struct of callbacks); an object expression's `override var` and suspending override;
// a secondary constructor's body run by a super call; the user's exception hierarchies; smart casts in chains.
sealed class Outcome<out T> {
    data class Done<out T>(val value: T) : Outcome<T>()
    data class Failed(val reason: String) : Outcome<Nothing>()
    object Loading : Outcome<Nothing>()

    fun valueOrNull(): T? = if (this is Done) value else null
}

fun parse(text: String): Outcome<Int> = if (text.isEmpty()) Outcome.Failed("boş") else Outcome.Done(text.length)

fun shown(o: Outcome<Int>): String = when (o) {
    is Outcome.Done -> "#" + o.value
    is Outcome.Failed, Outcome.Loading -> "-"
}

fun handler(): NavigationEventHandler<NavigationEventInfo.None> =
    object : NavigationEventHandler<NavigationEventInfo.None>(initialInfo = NavigationEventInfo.None, isBackEnabled = true) {
        override fun onBackCompleted() {
            println(transitionState)
        }
    }

object IntPairType : NavType<Int>(isNullableAllowed = false) {
    override fun put(bundle: SavedState, key: String, value: Int) {}
    override fun get(bundle: SavedState, key: String): Int? = null
    override fun parseValue(value: String): Int = value.toInt()
    override fun serializeAsValue(value: Int): String = "$value"
}

abstract class Fetcher2 {
    abstract var calls: Int
    abstract suspend fun fetch(key: String): String
}

fun counting(): Fetcher2 = object : Fetcher2() {
    override var calls = 0
    override suspend fun fetch(key: String): String {
        calls++
        return key
    }
}

open class Panel2(val w: Int, val h: Int) {
    var note = ""
    constructor(side: Int) : this(side, side) {
        note = "kare"
    }
}

class SquarePanel(s: Int) : Panel2(s)

open class AppException(message: String) : Exception(message)

class NetworkException(val code: Int) : AppException("ağ $code")

fun handled(): String = try {
    throw NetworkException(1)
} catch (e: AppException) {
    e.message ?: ""
}

fun describeAny(x: Any?): String = if (x is String && x.length > 2) x else if (x !is Int || x < 0) "-" else "${x + 1}"
