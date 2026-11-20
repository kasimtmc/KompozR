// Kotlin's `withStyle(ParagraphStyle)` takes its lambda `crossinline`: a suspending call in it is an error, as in
// Kotlin, the generator's (it knows the overload by the argument's type, and that the suspending twin takes only
// `withStyle(SpanStyle)`'s lambda: the index's `TWIN_BOUNDS`), at the suspending call.

use kompoz::prelude::*;

kompoz! {
    @Composable
    fun Styled() {
        LaunchedEffect(Unit) {
            val s = buildAnnotatedString {
                withStyle(ParagraphStyle(textAlign = TextAlign.Center)) {
                    delay(1)
                    append("a")
                }
            }
        }
    }
}

fn main() {}
