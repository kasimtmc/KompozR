// What a generic callee gives, as Kotlin infers it, so a local bound to it is typed: a scope function's `R` from
// what its lambda gives (`let`, `run`, `with`; `also`/`apply` its value, `takeIf` its value or null), a kt shim's
// from its lambdas and arguments (`map`, `mapNotNull`, `flatMap`, `fold`, `associateWith`, `maxOf`, `List(n)`), a
// generic type's members from its type arguments (`Animatable<Float, …>`'s `value`, `lowerBound`), an overloaded
// constructor's from the overload its arguments take (`Animatable(0f)`: an `Animatable<Float, AnimationVector1D>`).
// Each local's `toInt()` is the number's, which asked for the local's type before.
@Composable
fun Scoped(anim: Animatable<Float, AnimationVector1D>, log: (Int) -> Unit) {
    LaunchedEffect(Unit) {
        val got = anim.let {
            it.animateTo(5f)
            it.value
        }
        log(got.toInt())
        val ran = anim.run { targetValue }
        log(ran.toInt())
        val with = with(anim) { value * 2 }
        log(with.toInt())
        val same = anim.also { it.snapTo(1f) }.value
        log(same.toInt())
        val kept = anim.value.takeIf { it > 0f }
        log(kept!!.toInt())
        val bound = anim.lowerBound
        log(bound!!.toInt())
    }
}

@Composable
fun Remembered(log: (Int) -> Unit) {
    val anim = remember { Animatable(0f) }
    val color = remember { Animatable(Color.Red) }
    LaunchedEffect(Unit) {
        val got = anim.let {
            it.animateTo(5f)
            it.value
        }
        log(got.toInt())
        val red = color.value.red
        log(red.toInt())
    }
}

@Composable
fun Collections(log: (Int) -> Unit) {
    val floats = listOf(1, 2).map { it.toFloat() }
    log(floats[0].toInt())
    val some = listOf(1, 2).mapNotNull { if (it > 1) it.toFloat() else null }
    log(some.first().toInt())
    val flat = listOf(1, 2).flatMap { listOf(it.toFloat()) }
    log(flat.last().toInt())
    val sum = listOf(1, 2).fold(0f) { acc, x -> acc + x }
    log(sum.toInt())
    val byKey = listOf(1, 2).associateWith { it.toDouble() }
    log(byKey[1]!!.toInt())
    val most = listOf(1, 2).maxOf { it.toFloat() }
    log(most.toInt())
    val made = List(2) { it.toFloat() }
    log(made[1].toInt())
    val first = listOf(1.5f).let { it.first() }
    log(first.toInt())
}
