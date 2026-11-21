// Expressions: literals with Kotlin's typing, templates, operators, ranges, null handling and smart casts,
// string concatenation, qualified names.

fun literals() {
    val i = 16
    val big = 3000000000
    val l = 10L
    val f = 1.5f
    val g = 2f
    val d = 2.5
    val e = 1e3
    val hex = 0xFF00FF00
    val bin = 0b1010
    val u = 7u
    val c = 'ç'
    val b = true && !false
    val s = "tab\t\"quoted\" ğüş"
    val raw = """
        bir
          iki
    """.trimIndent()
    println("$i $big $l $f $g $d $e $hex $bin $u $c $b $s $raw")
}

fun arithmetic(width: Float, n: Int): Float {
    val half = width / 2
    val third = 3 * width
    val mixed = n * 2 + 1
    val neg = -n % 3
    return half + third + mixed + neg - 1
}

fun templates(name: String, ratio: Float, items: List<String>): String {
    val a = "Ad: $name, uzunluk: ${name.length}, oran: $ratio, iki kat: ${ratio * 2}"
    val b = "liste: $items, boş mu: ${items.isEmpty()}, ${if (ratio > 1f) "büyük" else "küçük"}"
    val c = "dolar: ${'$'}5, " + name + 1 + 'c'
    return a + b + c
}

fun nulls(maybe: String?, other: String?): Int {
    val length = maybe?.length ?: 0
    val text = maybe?.uppercase()?.trim() ?: "yok"
    val forced = other!!.length
    if (maybe != null && maybe.length > 2) {
        println(maybe)
    }
    if (other == null) {
        println("null")
    } else {
        println(other.length)
    }
    val n: Int? = null
    val some: String? = "dört"
    val first = maybe ?: return -1
    return length + forced + text.length + first.length + (n ?: 0) + (some?.length ?: 0)
}

fun ranges(x: Int): Boolean {
    val r = 1..10
    val open = 0 until 5
    val down = 10 downTo 1 step 3
    val half = 0..<4
    return x in r && x !in open && x in down || x in half
}

fun qualified(): String {
    val red = Color.Red
    val top = Alignment.TopStart
    val max = Int.MAX_VALUE
    val pi = PI
    val spaced = Arrangement.spacedBy(8.dp)
    val m = Modifier.Companion.fillMaxSize()
    return "$red $top $max $pi $spaced $m"
}

fun comparisons(a: String, b: String, n: Long): Boolean {
    return a == b || a != "x" || n >= 3 || n < 2L || (n > 1 && a < b)
}

fun bits(x: Int): Int = (x and 0xFF) or (x shl 2) xor (x shr 1)

fun unusedLocals(): Int {
    @Suppress("UNUSED_VARIABLE")
    val kept = 1
    @Suppress("unused") var counted = 2
    return 3
}
