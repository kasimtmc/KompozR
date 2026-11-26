// (stage 14 LISTS) Kotlin's MutableList/MutableSet/MutableMap as kt's shared cells (a parameter, a `val`, a class's
// property: one object), `+=`/`-=` changing them, `===` their identity, a `List` declared over a mutable one a view of
// it, a `List` parameter asked `is MutableList` the caller's list (`kt::ListView`), conversions where Kotlin's types
// relate; `mapOf(…)` of `Any` values; defaults reading earlier parameters (a function's, a composable's, a
// constructor's, a lambda's capturing one, one calling with named arguments).

fun dropFirst(xs: MutableList<Int>) {
    xs.removeAt(0)
}

fun tally(counts: MutableMap<String, Int>, key: String) {
    counts[key] = (counts[key] ?: 0) + 1
}

fun total(xs: List<Int>): Int = xs.sum()

fun <T> popIfMutable(stack: List<T>): Boolean {
    if (stack is MutableList) {
        stack.removeAt(stack.size - 1)
        return true
    }
    return false
}

class Basket(val items: MutableList<String> = mutableListOf()) {
    private val _log = mutableListOf<String>()
    val log: List<String> = _log
    fun put(item: String) {
        items += item
        _log.add(item)
    }
}

fun shared(): String {
    val xs = mutableListOf(1, 2, 3)
    val ys = xs
    dropFirst(xs)
    ys += 4
    ys -= 2
    xs[0] = 9
    val view: List<Int> = xs
    val copy = xs.toMutableList()
    val seen = mutableSetOf("a")
    seen += listOf("b", "c")
    val counts = hashMapOf<String, Int>()
    tally(counts, "k")
    counts += "m" to 2
    val sum = total(xs) + view.size
    return "$xs $copy $seen $counts $sum ${xs === ys} ${xs === copy} ${popIfMutable(xs)}"
}

fun meta(value: Int, onDone: () -> Unit): Map<String, Any> = mapOf("value" to value, "done" to onDone)

fun scaled(a: Int, b: Int = a * 2, label: String = "$a/$b"): String = "$a $b $label"

fun <T> popper(stack: List<T>, onBack: () -> Unit = { if (stack is MutableList) stack.removeAt(stack.size - 1) }): () -> Unit = onBack

fun faded(duration: Int, spec: ContentTransform = ContentTransform(fadeIn(animationSpec = tween(duration)), fadeOut(), sizeTransform = null)) = spec

class Frame(val width: Int, val height: Int = width / 2)

@Composable
fun Titled(title: String, subtitle: String = title.uppercase()) {
    Text("$title $subtitle")
}
