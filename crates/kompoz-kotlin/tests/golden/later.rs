#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn still_translated(x: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ x + 1 })
}
// error 4:1: a data class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 6:1: an object declaration isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 8:1: an interface isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 10:1: an enum class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 12:1: a type alias isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 14:6: a generic function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 17:5: `try` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 24:15: the type `Any` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 24:31: an `is` check isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 26:21: `throw` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 28:5: write `references`'s return type: its expression's type isn't one KompozR can tell (`fun references(…): Type = …`)
// error 28:20: a callable reference (`::name`) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 31:5: a local function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 35:1: an `infix` function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 39:5: write `inferred`'s return type: its expression's type isn't one KompozR can tell (`fun inferred(…): Type = …`)
// error 41:1: a top-level `var` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 44:30: a `return` from inside a lambda (Kotlin's non-local return) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
