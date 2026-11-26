#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn still_translated(x: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ x + 1 })
}
// error 4:1: an annotation class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 6:1: `value` on a class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 8:21: a generic sealed class (its cases' `Nothing`, `out T`) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 14:5: `try` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 21:21: `throw` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 23:5: write `references`'s return type: its expression's type isn't one KompozR can tell (`fun references(…): Type = …`)
// error 23:20: a callable reference (`::name`) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 26:5: a local function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 30:1: an `infix` function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 34:5: write `inferred`'s return type: its expression's type isn't one KompozR can tell (`fun inferred(…): Type = …`)
// error 36:1: a top-level `var` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 39:30: a `return` from inside a lambda (Kotlin's non-local return) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
