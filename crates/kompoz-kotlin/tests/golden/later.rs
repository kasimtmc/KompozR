#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn still_translated(x: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ x + 1 })
}
// error 4:1: an annotation class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 6:1: `value` on a class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 8:28: a generic sealed class holding its type parameter's values in its own properties isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 14:5: write `inferred`'s return type: its expression's type isn't one KompozR can tell (`fun inferred(…): Type = …`)
// error 16:14: a sealed class deriving from an exception class isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
