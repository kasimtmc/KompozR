// What the Kotlin syntax doesn't translate yet: each an error at its Kotlin position saying it comes with stage 14; the
// declarations without one are translated.

annotation class Marker

value class Meters(val value: Int)

sealed class Loaded<out T> {
    data class Value<T>(val value: T) : Loaded<T>()
    object Nothing : Loaded<Nothing>()
}

fun tries(): Int {
    try {
        return 1
    } catch (e: Exception) {
        return 2
    }
}

fun throws(): Int = throw IllegalStateException("x")

fun references() = ::tries

fun locals() {
    fun inner() = 1
    println(inner())
}

infix fun Int.plus2(other: Int) = this + other

fun stillTranslated(x: Int) = x + 1

fun inferred(x: Int) = unknownFunction(x)

var counter: Int = 0

fun nonLocal(xs: List<Int>) {
    xs.forEach { if (it > 2) return }
}
