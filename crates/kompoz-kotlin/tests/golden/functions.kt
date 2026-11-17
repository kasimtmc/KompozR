// Plain functions (block and expression bodies, written and inferred return types, defaults), suspend
// functions, extension functions, top-level vals.

val Gap: Dp = 8.dp
const val Limit: Int = 3
val Title: String = "Başlık"
private val Ratio: Float = 1.5f * 2

fun double(x: Int) = x * 2

fun greet(name: String, punctuation: String = "!"): String {
    return "Merhaba, $name$punctuation"
}

internal fun sum(values: List<Int>): Int {
    var total = 0
    for (v in values) {
        total += v
    }
    return total
}

fun label(n: Int): String = if (n > Limit) "çok" else "az"

fun log(message: String) = println(message)

suspend fun pause(ms: Long) {
    delay(ms)
}

fun Modifier.framed(width: Dp = 2.dp): Modifier = this.padding(width).border(width, Color.Black)

fun Modifier.card(): Modifier = framed().padding(Gap)

private fun String.shout() = uppercase() + "!"

fun twice(f: () -> Unit) {
    f()
    f()
}

fun apply2(x: Int, f: (Int) -> Int): Int = f(f(x))
