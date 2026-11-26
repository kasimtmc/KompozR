// What the Kotlin syntax doesn't translate yet: each an error at its Kotlin position saying it comes with stage 15; the
// declarations without one are translated.

sealed class Loaded<out T>(val first: T) {
    data class Value<T>(val value: T) : Loaded<T>(value)
}

fun stillTranslated(x: Int) = x + 1

fun inferred(x: Int) = unknownFunction(x)

sealed class Problem(val code: Int) : IllegalStateException("kod $code")

fun firstLong(words: List<String>): String? {
    val found = buildString {
        for (w in words) {
            if (w.length > 3) return w
            append(w)
        }
    }
    return found.ifEmpty { null }
}
