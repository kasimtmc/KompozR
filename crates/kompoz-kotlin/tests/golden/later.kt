// What the Kotlin syntax doesn't translate yet: each an error at its Kotlin position saying it comes with stage 14; the
// declarations without one are translated.

annotation class Marker

value class Meters(val value: Int)

sealed class Loaded<out T>(val first: T) {
    data class Value<T>(val value: T) : Loaded<T>(value)
}

fun stillTranslated(x: Int) = x + 1

fun inferred(x: Int) = unknownFunction(x)

sealed class Problem(val code: Int) : IllegalStateException("kod $code")
