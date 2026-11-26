// A `.kt` file's errors (`kompoz_file!`): its tokens all have the call's span, so ours name the file's
// `file:line:column` in the message (a parse error, what comes with stage 15) and rustc's (a `String` where an `Int`
// goes) are at the call.

use kompoz::prelude::*;

kompoz_file!("tests/compile-fail/parse_error.kt");

kompoz_file!("tests/compile-fail/type_error.kt");

fn main() {}
