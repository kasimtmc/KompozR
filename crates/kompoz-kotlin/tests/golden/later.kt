// What the Kotlin syntax doesn't translate yet: each an error at its Kotlin position saying it comes with stage 14; the
// declarations without one are translated.

sealed interface Shape

open class Base(val id: Int)

enum class Mode {
    A {
        override fun toString() = "a"
    },
    B
}

fun <T> identity(x: T): T = x

fun tries(): Int {
    try {
        return 1
    } catch (e: Exception) {
        return 2
    }
}

fun checks(x: Any): Boolean = x is String

fun throws(): Int = throw IllegalStateException("x")

fun references() = ::tries

fun locals() {
    fun inner() = 1
    println(inner())
}

infix fun Int.plus2(other: Int) = this + other

fun stillTranslated(x: Int) = x + 1

fun inferred(x: Int) = unknownFunction(x)

var counter: Int = 0

fun nonLocal(xs: List<Int>) {
    xs.forEach { if (it > 2) return }
}
