// Kotlin's `withStyle(ParagraphStyle)` takes its lambda `crossinline`: a suspending call in it is an error, as in
// Kotlin (rustc's, through the bound of the suspending twin `withStyle(SpanStyle)`'s lambda goes to), on the style.

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
