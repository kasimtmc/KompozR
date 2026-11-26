// (stage 14 FINAL-T) The closing pass's classes and types: user exception hierarchies (open, sealed), `==` with
// `Any?`, data classes hashed, a class of a Rust standard type's name, `out` variance, generic sealed classes holding
// their parameter's values, object expressions' own members, handler objects, generic overrides, `equals` of `Any?`.

// Exceptions of the user's open and sealed classes: `e is IllegalStateException` of an open exception class's
// value (its shared object a Kotlin exception), a sealed one's cases thrown and caught as the class or a case.
open class AppFault(message: String) : IllegalStateException(message)
class DiskFault : AppFault("disk")

fun faultKind(e: AppFault): String = if (e is IllegalStateException) "ise " + e.message else "başka"
fun diskKind(): String = faultKind(DiskFault())

sealed class Trouble(val code: Int) : IllegalStateException("kod $code") {
    class NotFound : Trouble(404)
    class Snapped(val why: String) : Trouble(500)
}

// (one of no cases)
sealed class Mishap(val code: Int) : IllegalStateException("kod $code")

fun risky(n: Int): Int {
    if (n == 1) throw Trouble.NotFound()
    if (n == 2) throw Trouble.Snapped("bozuk")
    return n
}

fun tryRisky(n: Int): String = try {
    "ok " + risky(n)
} catch (e: Trouble.Snapped) {
    "broken " + e.why + " " + e.code + " " + e.message
} catch (e: Trouble) {
    "trouble " + e.code + " " + (e is IllegalStateException) + " " + e
}

// A data class compared with a value of `Any?`; hashed as Kotlin's `hashCode()` (a `Float` by its bits:
// `NaN == NaN`, `0f != -0f`), in sets; a class named as a Rust standard type is one of the crate's.
data class Coin(val amount: Int)
fun sameCoin(m: Any?): Boolean = Coin(5) == m
fun otherCoin(m: Any): Boolean = m != Coin(5)

data class Speckle(val x: Int, val y: Float)
fun speckles(): Int = setOf(Speckle(1, 2f), Speckle(1, 2f), Speckle(1, -0f), Speckle(1, 0f)).size
fun speckleHashes(): Boolean = Speckle(1, 2f).hashCode() == Speckle(1, 2f).hashCode() && "ab".hashCode() == 3105 &&
    listOf(1, 2).hashCode() == 994

class Vec(val x: Int)
fun vec() = Vec(1)
fun vecs() = listOf(Vec(2), Vec(3))

// `out` variance of the user's generic classes: a `Lookup<Hound>` a `Lookup<Creature>`, given where one is
// taken (a result, an argument, a case of `Nothing`), a data class's too.
open class Creature(val name: String)
class Hound(name: String) : Creature(name)

sealed class Lookup<out T> {
    data class Found<out T>(val value: T, val all: List<T>) : Lookup<T>()
    data class Pairs<out T>(val p: Pair<T, Int>?) : Lookup<T>()
    object Missing : Lookup<Nothing>()
}
data class Satchel<out T>(val item: T, val more: List<T>?)

fun hounds(): Lookup<Hound> = Lookup.Found(Hound("karabaş"), listOf(Hound("a")))
fun creatures(): Lookup<Creature> = hounds()
fun satchel(): Satchel<Creature> = Satchel(Hound("x"), null)
fun describeLookup(o: Lookup<Creature>): String = when (o) {
    is Lookup.Found -> "bulundu " + o.value.name + " " + o.all.size
    is Lookup.Pairs -> "çift"
    Lookup.Missing -> "yok"
}
fun lookups(): String = describeLookup(hounds()) + " " + describeLookup(Lookup.Missing) + " " + satchel().item.name

// A generic sealed class holding its type parameter's values (its cases giving their own), a sealed class's
// secondary constructor with a body its data and plain cases' super calls go through, a sealed class of a type
// parameter no case takes (its hidden variant).
sealed class Keeping<out T>(val first: T) {
    data class Value<T>(val value: T) : Keeping<T>(value)
    class Both<T>(val a: T, val b: T) : Keeping<T>(a)
    fun firstOr(): T = first
}
fun keepings(): String {
    val k: Keeping<Int> = Keeping.Value(3)
    val p: Keeping<String> = Keeping.Both("a", "b")
    return "" + k.first + " " + k.firstOr() + " " + p.first + " " + k + " " + (k == Keeping.Value(3))
}

sealed class Polygon(val name: String) {
    var note = "birincil"
    constructor(sides: Int) : this("kenar $sides") {
        note = "çokgen $sides"
    }
    data class Regular(val n: Int) : Polygon(n)
    data class Named(val label: String) : Polygon(label)
    class Plain(val k: Int) : Polygon(k)
}
fun polygons(): String =
    Polygon.Regular(3).note + " " + Polygon.Regular(4).name + " " + Polygon.Named("x").note + " " + Polygon.Plain(5).note

sealed class Quiet<out T> {
    object Off : Quiet<Nothing>()
    data class Note(val text: String) : Quiet<Nothing>()
}
fun quiets(q: Quiet<Int>): String = when (q) {
    Quiet.Off -> "kapalı"
    is Quiet.Note -> "not " + q.text
}
fun quietLine(): String {
    val all: List<Quiet<Int>> = listOf(Quiet.Off, Quiet.Note("a"))
    return all.map { quiets(it) }.toString() + " " + (all[1] == Quiet.Note("a")) + " " + all[1]
}

// Object expressions: members of their own (a `var`, a `lateinit var`, a function) its overrides share, an
// override another calls; an object declaration of a base whose callbacks are given its struct (Kotlin's one
// object); generic functions of an open or abstract class overridden (by a class, an object expression).
abstract class Tocker {
    abstract fun next(): Int
    fun twice(): String = "${next()} ${next()}"
}
fun tocker(start: Int): Tocker = object : Tocker() {
    var n = start
    fun bump() = n++
    override fun next(): Int = bump()
}

interface Drip {
    fun next(): Int
    fun both(): String
}
fun drip(start: Int): Drip = object : Drip {
    lateinit var last: String
    var n = start
    override fun next(): Int { n++; last = "n$n"; return n }
    override fun both(): String = "${next()} $last"
}
fun objectsLine(): String = tocker(5).twice() + " " + drip(1).both() + " " + drip(7).both()

data class Spot2(val x: Int, val y: Int)
fun spotType(sep: String): NavType<Spot2> = object : NavType<Spot2>(isNullableAllowed = false) {
    override fun put(bundle: SavedState, key: String, value: Spot2) {
        bundle.write { putString(key, serializeAsValue(value)) }
    }
    override fun get(bundle: SavedState, key: String): Spot2? = parseValue(bundle.read { getString(key) })
    override fun parseValue(value: String): Spot2 =
        Spot2(value.substringBefore(sep).toInt(), value.substringAfter(sep).toInt())
    override fun serializeAsValue(value: Spot2): String = "${value.x}$sep${value.y}"
}

object BackLog : NavigationEventHandler<NavigationEventInfo.None>(NavigationEventInfo.None, isBackEnabled = true) {
    var log = ""
    override fun onBackStarted(event: NavigationEvent) {
        log += "başla "
    }
    override fun onBackCompleted() {
        log += "bitti " + isBackEnabled + " " + (transitionState == NavigationEventTransitionState.Idle)
        isBackEnabled = false
    }
}
fun backLog(): NavigationEventHandler<NavigationEventInfo.None> = BackLog
fun backLogText(): String = BackLog.log
fun sameHandler(): Boolean = backLog() == backLog()

open class Wrapper {
    open fun <T> id(x: T): T = x
    open fun <T> wrap(x: T): List<T> = listOf(x)
    fun both(): String = "" + id(1) + wrap("a")
}
class TwinWrapper : Wrapper() {
    override fun <T> wrap(x: T): List<T> = listOf(x, x)
}
fun wrapper(): Wrapper = object : Wrapper() {
    override fun <T> wrap(x: T): List<T> = listOf(x, x, x)
}
abstract class Doubler {
    abstract fun <T> wrap(x: T): List<T>
    fun twice(): Int = wrap(2).size
}
fun doubler(): Doubler = object : Doubler() {
    override fun <T> wrap(x: T): List<T> = listOf(x, x)
}
fun genericsLine(): String =
    Wrapper().both() + " " + TwinWrapper().both() + " " + wrapper().both() + " " + doubler().twice() + " " + wrapper().wrap(true)

// A plain class's `equals` of `Any?` (`this === other`, an early `!is` return: the smart cast after it) and its
// `hashCode()` (Kotlin's wrapping arithmetic).
class Brick(val side: Int) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Brick) return false
        return side == other.side
    }
    override fun hashCode(): Int = side * 1000003 * 1000003 + listOf(side).hashCode()
}
fun squares(): String = "" + (Brick(2) == Brick(2)) + " " + (Brick(2) == Brick(3)) + " " +
    (Brick(7).hashCode() == Brick(7).hashCode())
