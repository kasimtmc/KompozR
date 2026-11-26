// (stage 14 FINAL) Jumps out of the lambdas of any inline function: a `return` out of `let`'s inside
// `buildAnnotatedString`'s (ExposedDropdownMenuSamples' underlineSubsequence), `break`/`continue` of a loop out of
// `let`'s, `return@outer` of a lambda out of `map`'s, out of the file's own `inline fun` (its `try`/`finally`), and in a
// coroutine out of a suspending inline lambda (`let`'s twin): `kt::leave` in the lambda, `kt::leaving` around the call.

fun underlineSubsequence(needle: CharSequence, haystack: String): AnnotatedString? {
    return buildAnnotatedString {
        var i = 0
        for (char in needle) {
            val start = i
            haystack.indexOf(char, startIndex = i, ignoreCase = true).let {
                if (it < 0) return null else i = it
            }
            append(haystack.substring(start, i))
            withStyle(SpanStyle(textDecoration = TextDecoration.Underline)) {
                append(haystack[i])
            }
            i += 1
        }
        append(haystack.substring(i, haystack.length))
    }
}

fun loops(xs: List<Int>): Int {
    var n = 0
    for (x in xs) {
        x.let { if (it == 2) continue; if (it == 5) break }
        n += x
    }
    return n
}

fun outerLabel(xs: List<Int>): Int = run outer@{
    xs.map { if (it == 2) return@outer it * 10; it }
    0
}

inline fun guarded(log: SnapshotStateList<String>, block: () -> Unit) {
    try {
        block()
        log.add("sonra")
    } catch (e: Throwable) {
        log.add("yakalandı")
    } finally {
        log.add("finally")
    }
}

fun throughOwn(log: SnapshotStateList<String>, stop: Boolean): String {
    guarded(log) {
        if (stop) return "erken"
    }
    return "sonuna"
}

suspend fun waitsThenLeaves(xs: List<Int>): Int {
    xs.let {
        delay(1)
        if (it.size > 1) return it.first() + 100
    }
    return -1
}
