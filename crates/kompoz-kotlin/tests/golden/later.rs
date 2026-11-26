#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn still_translated(x: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Base(r)), { x + 1 })
}
// error 4:1: a sealed interface isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 6:1: `open` on a class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 9:7: an enum entry's body isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 15:6: a generic function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 18:5: `try` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 25:15: the type `Any` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 27:21: `throw` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 29:5: write `references`'s return type: its expression's type isn't one KompozR can tell (`fun references(…): Type = …`)
// error 29:20: a callable reference (`::name`) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 32:5: a local function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 36:1: an `infix` function isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 40:5: write `inferred`'s return type: its expression's type isn't one KompozR can tell (`fun inferred(…): Type = …`)
// error 42:1: a top-level `var` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 45:30: a `return` from inside a lambda (Kotlin's non-local return) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
