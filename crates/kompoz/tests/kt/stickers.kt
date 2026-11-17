// Another Kotlin file of the module (the C++ Kompoz's kompozable_tests_other.cpp): its composable and vals are used
// from kotlin_ported.rs's Kotlin as Kotlin calls them from another file (named arguments, defaults, a val read by
// its name, a local read and provided), and from its Rust. `Sticker` is a Rust struct there (Kotlin's data classes come with stage 14).
package com.example.stickers

@Composable
fun StickerRow(sticker: Sticker, prefix: String = "", suffix: String = "") {
    Text("$prefix${sticker.label}:${sticker.count}$suffix")
}

// (its type is its value's; it reads a val declared after it)
val StickerGreeting = "selam " + Shout

private val Shout = "!"

// (a composition local other files read and provide)
val LocalStickerTone = compositionLocalOf { "sakin" }
