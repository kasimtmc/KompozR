// Statements: locals and delegates, assignments, increments, loops with labels, jumps, if and when.

@Kompozable
fun States() {
    var count by remember { mutableStateOf(0) }
    val doubled by remember { derivedStateOf { count * 2 } }
    var name by rememberSaveable { mutableStateOf("") }
    val keyed = remember(count, name) { count + 1 }
    count = 5
    count += 2
    count--
    ++count
    name += "x"
    Text("$count $doubled $name $keyed")
}

fun counting(n: Int): Int {
    var i = 0
    var total = 0
    while (i < n) {
        i++
        if (i % 2 == 0) continue
        total += i
    }
    do {
        total -= 1
    } while (total > 100)
    outer@ for (a in 0 until n) {
        for (b in 0..a) {
            if (b == 3) break@outer
            if (b == 1) continue@outer
            total += a * b
        }
    }
    for (k in 10 downTo 0 step 2) total += k
    val x = ++i
    val y = i--
    return total + x + y
}

fun destructure(pairs: List<Pair<String, Int>>): String {
    var out = ""
    val (first, second) = "a" to 1
    for ((key, value) in pairs) {
        out += "$key=$value;"
    }
    return out + first + second
}

fun branches(n: Int): String {
    val word = when (n) {
        0 -> "sıfır"
        1, 2 -> "biraz"
        in 3..5 -> "orta"
        !in 6..9 -> "dışarıda"
        else -> "bol"
    }
    when {
        n < 0 -> println("eksi")
        n == 0 -> println("sıfır")
    }
    val sign = if (n > 0) 1 else if (n < 0) -1 else 0
    return "$word $sign"
}

fun loopy(xs: List<Int>): Int {
    var found = -1
    xs.forEach lit@{
        if (it < 0) return@lit
        found = it
    }
    var i = 0
    do {
        i++
        if (i == 2) continue
    } while (i < 5)
    return found
}

// An element's increment as a statement: `counts[i]++` of a state list, `--` of a list's (Kotlin's
// `a.set(i, a.get(i).inc())`, the index evaluated once).
@Kompozable
fun Counts(i: Int) {
    val counts = remember { mutableStateListOf(0, 0, 0) }
    val local = mutableListOf(1.5f, 2f)
    counts[i + 1]++
    --local[0]
    Text("${counts[1]} ${local[0]}")
}
