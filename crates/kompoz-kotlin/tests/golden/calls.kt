// Calls: named arguments, defaults (the builder's and Kotlin's from the index), trailing lambdas, modifier chains,
// overloads by their arguments, nullable parameters, varargs, the kt shims' members by the receiver's type.

@Kompozable
fun Calls(items: List<String>) {
    Column(modifier = Modifier.padding(16.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text("a", fontSize = 20.sp)
        Text(text = "b", modifier = Modifier.testTag("b"))
        Box(Modifier.padding(horizontal = 8.dp).size(20.dp).background(Color(0xFF6200EE))) { }
        Spacer(Modifier.height(4.dp))
        Text(items.filter { it.startsWith("a") }.map { it.uppercase() }.joinToString("+"))
        Text(items.joinToString(separator = ", ", prefix = "[", postfix = "]"))
        items.forEachIndexed { i, s -> Text("$i: $s") }
        Text("${items.size} ${items.first()} ${items.getOrNull(9) ?: "-"}")
    }
}

fun colors(): List<Color> {
    val a = Color(0.5f, 0.25f, 1f)
    val b = Color(255, 0, 0)
    val c = Color.Red.copy(alpha = 0.5f)
    return listOf(a, b, c)
}

fun collections(): Int {
    val xs = mutableListOf(3, 1, 2)
    xs.add(4)
    val ys = xs.sorted().reversed()
    val m = mapOf("a" to 1, "b" to 2)
    val total = ys.sumOf { it * 2 } + (m["a"] ?: 0)
    repeat(3) { println(it) }
    require(total > 0) { "pozitif olmalı" }
    return total + xs[0]
}

fun text(s: String): String {
    val parts = s.split(",")
    val n = s.length + parts.size
    val trimmed = s.trim().lowercase().replace("a", "b")
    return "$n ${trimmed.substring(1)} ${s.toInt() + 1} ${n.toString()}"
}
