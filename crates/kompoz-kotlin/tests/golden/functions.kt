// Plain functions (block and expression bodies, written and inferred return types, defaults), suspend
// functions, extension functions, top-level vals; a type named as the macros' support trait is (`Key`: the struct);
// an interface's companion function (`TextAutoSize.StepBased`: the trait object's own); a result of an interface
// type given a value of its own type (`DraggableAnchors { … }`: an `Arc` of it, as the API hands the anchors out);
// kotlin.math's `round` over an API function of that name, `PI`'s member, an Int literal's member; an object's lent
// value (`MaterialShapes.Heart`: a clone) given to a constructor (`Morph(…)`); `Path()` before the vector `Path(…)`,
// a `val` of a type changed in place (`mut`).

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

fun arrows(): List<Key> = listOf(Key.DirectionLeft, Key.MoveEnd)

fun stepSizes(): TextAutoSize = TextAutoSize.StepBased(8.sp, 40.sp, 1.sp)

fun twoAnchors(): DraggableAnchors<Int> = DraggableAnchors {
    1 at 0f
    2 at 120f
}

fun mathForms(): Boolean = round(2.5f) == 2f && (-3).toFloat() == -3f && PI.toFloat() > 3f

fun heartToPill(): Morph = Morph(MaterialShapes.Heart, MaterialShapes.Pill)

fun tenLong(): Float {
    val path = Path()
    path.lineTo(10f, 0f)
    val measure = PathMeasure()
    measure.setPath(path, false)
    return measure.length
}
