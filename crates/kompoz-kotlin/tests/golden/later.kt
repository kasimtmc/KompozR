// What the Kotlin syntax doesn't translate yet: each an error at its Kotlin position saying it comes with stage 15; the
// declarations without one are translated.

sealed class Loaded<out T>(val first: T) {
    data class Value<T>(val value: T) : Loaded<T>(value)
}

fun stillTranslated(x: Int) = x + 1

fun inferred(x: Int) = unknownFunction(x)

sealed class Problem(val code: Int) : IllegalStateException("kod $code")

@Composable
fun Rows(xs: List<String>) {
    Column {
        if (xs.isEmpty()) return
        Text(xs.first())
    }
}
