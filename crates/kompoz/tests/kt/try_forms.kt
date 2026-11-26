// (stage 14 WPB) The C++ Kompoz's `try_as_a_value_and_throw` (tests/kompozable_tests.cpp), as it is written there: a
// .kt file, as Rust's lexer reads `"iki söz"` inside `kompoz! { … }` as a literal of a reserved prefix.

// try as a value: the body's last expression, or the catching block's (finally runs, giving nothing); throw,
// as a statement and as a value (?: throw); Kotlin's exceptions as they derive from one another (a
// NumberFormatException is an IllegalArgumentException), their message, rethrown as they are.
fun parsedOr(text: String, fallback: Int): Int = try {
    if (text.isEmpty()) throw IllegalArgumentException("boş")
    text.toInt()
} catch (e: IllegalArgumentException) {
    fallback
}

fun firstWord(line: String?): String = line?.split(" ")?.firstOrNull() ?: throw IllegalStateException("satır yok")

class Refused(val code: Int) : IllegalStateException("kod $code")

fun rethrown(): String {
    try {
        try {
            check(false) { "iç" }
        } catch (e: IllegalStateException) {
            throw e
        }
    } catch (e: RuntimeException) {
        return "yine " + e.message
    }
    return "?"
}

@Kompozable
fun TryForms() {
    var cleaned = 0
    val a = parsedOr("41", 0)
    val b = parsedOr("", -1)
    val c = parsedOr("12abc", -2)
    val d: String? = try { error("yok") } catch (e: IllegalStateException) { null } finally { cleaned++ }
    val f = listOf("7", "x").map { s -> try { s.toInt() * 2 } catch (e: NumberFormatException) { 0 } }
    val g = try { firstWord(null) } catch (e: Exception) { e.message ?: "?" }
    val h = try { listOf<Int>().first().toString() } catch (e: NoSuchElementException) { "$e" }
    val r = try { throw Refused(7) } catch (e: Refused) { "${e.code} ${e.message}" }
    val r2 = try { throw Refused(2) } catch (e: IllegalStateException) { "$e" }
    Text("dene $a $b $c ${d ?: "boş"} $cleaned ${f.joinToString(",")} $g ${firstWord("iki söz")} ${rethrown()} $h | $r | $r2")
}
