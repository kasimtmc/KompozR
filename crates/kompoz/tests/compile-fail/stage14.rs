// Kotlin outside stage 9's: an error saying it comes with stage 14, on the Kotlin construct.

use kompoz::prelude::*;

kompoz! {
    class Point(val x: Int, val y: Int)

    fun sum(a: Any): Int = if (a is Int) a else 0
}

fn main() {}
