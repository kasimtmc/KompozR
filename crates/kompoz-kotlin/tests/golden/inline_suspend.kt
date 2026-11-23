// Kotlin's inline functions run their lambdas in place: in a coroutine such a lambda suspends with its caller. A lambda
// that suspends is an async closure given to the kt shim's suspending twin (`with_async`, `KtList::for_each_async`…),
// the call awaited; `return@label` returns from the closure; one that doesn't suspend is the plain closure it was. A
// safe call and `?:` whose right side suspend go through `safe_async`/`elvis_async`. A lambda Kotlin doesn't run in
// place (`sortedBy`'s crossinline one, an unknown callee's) can't suspend: an error, as in Kotlin.
suspend fun load(n: Int): Int {
    delay(1)
    return n * 2
}

@Composable
fun Scoped(anim: Animatable<Float, AnimationVector1D>, maybe: Animatable<Float, AnimationVector1D>?, log: (Int) -> Unit) {
    LaunchedEffect(Unit) {
        with(anim) { animateTo(1f) }
        val bound = with(anim) {
            updateBounds(upperBound = 4f)
            animateTo(2f)
            if (value > 1f) return@with 7
            3
        }
        anim.run { animateTo(3f) }
        anim.apply { snapTo(0f) }.also { it.animateTo(4f) }
        val got: Float = anim.let { it.animateTo(5f); it.value }
        val kept = anim.takeIf { it.animateTo(6f); true }
        anim.takeUnless { load(1) > 1 }
        val ran = run { delay(10); load(3) }
        with(anim) { updateBounds(upperBound = 9f) }
        log(bound + got.toInt() + ran)
        maybe?.animateTo(2f)
        maybe?.let { it.animateTo(3f) }
        val n: Int? = null
        val m = n ?: load(4)
        log(m)
    }
}

@Composable
fun Collections(anim: Animatable<Float, AnimationVector1D>, log: (Int) -> Unit) {
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) {
        var total = 0
        repeat(3) { anim.animateTo(it.toFloat()) }
        listOf(1, 2, 3).forEach {
            delay(10)
            total += it
        }
        listOf(4, 5).forEachIndexed { i, x -> log(load(i) + x) }
        listOf(1, 2, 3, 4).forEach lit@{
            if (it % 2 == 0) return@lit
            delay(1)
            log(it)
        }
        listOf(6, 7).forEach {
            if (it == 6) return@forEach
            log(load(it))
        }
        val doubled = listOf(1, 2).map { load(it) }
        val odd = listOf(1, 2, 3).filter { delay(1); it % 2 == 1 }
        val any = listOf(1, 2).any { load(it) > 3 }
        val sum = listOf(1, 2).fold(0) { acc, x -> acc + load(x) }
        val chars = "ab".map { delay(1); it.uppercaseChar() }
        mapOf("a" to 1).forEach { (k, v) -> delay(1); log(k.length + v) }
        val cache = mutableMapOf<String, Int>()
        val cached = cache.getOrPut("a") { load(5) }
        val made = List(2) { load(it) }
        require(total > 0) { "total ${load(total)}" }
        listOf(1, 2).forEach { x -> repeat(2) { delay(x.toLong()) } }
        repeat(2) {
            for (i in 0 until 5) {
                if (i == 2) break
                delay(1)
            }
        }
        listOf(1, 2).forEach { log(it) }
        listOf(1, 2).forEach { x -> scope.launch { delay(x.toLong()) } }
        made.forEachIndexed { i, _ -> delay(i.toLong()) }
        log(total + doubled.size + odd.size + sum + chars.size + cached + made.size + if (any) 1 else 0)
    }
}

suspend fun countDown(from: Int): Int {
    var left = 0
    (1..from).forEach {
        delay(1)
        left += it
    }
    return left
}

@Composable
fun NotInPlace(log: (Int) -> Unit) {
    LaunchedEffect(Unit) {
        val sorted = listOf(2, 1).sortedBy { load(it) }
        madeElsewhere { delay(1) }
        log(sorted.size)
    }
}

@Composable
fun OutOfTheLambda(log: (Int) -> Unit) {
    LaunchedEffect(Unit) {
        for (i in 0 until 3) {
            listOf(i).forEach {
                delay(1)
                if (it == 1) continue
            }
        }
        listOf(1).forEach { if (it == 1) return@LaunchedEffect; delay(1) }
    }
}

inline fun twice(block: () -> Unit) {
    block()
    block()
}

@Composable
fun OwnInline() {
    LaunchedEffect(Unit) { twice { delay(1) } }
}
