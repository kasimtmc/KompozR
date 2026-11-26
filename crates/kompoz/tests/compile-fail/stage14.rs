// Kotlin the syntax doesn't translate yet: an error saying it comes with stage 14, on the Kotlin construct.

use kompoz::prelude::*;

kompoz! {
    annotation class Marker

    value class Meters(val value: Int)
}

fn main() {}
