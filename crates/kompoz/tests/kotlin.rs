//! Kotlin's syntax end to end: `kompoz! { … }` translated, compiled through `#[kompozable]` (with only the umbrella
//! crate's paths), composed by the test rule.

#![allow(non_snake_case)]

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

kompoz! {
    /** A composable that emits nothing. */
    @Kompozable
    fun Empty() {}

    // (pasted Compose code: @Composable is @Kompozable)
    @Composable private fun AlsoEmpty() {
    }
}

// Kotlin written by another macro: its tokens' positions don't add up (the name comes from the call, the rest from
// the definition), so the text is rebuilt from the tokens.
macro_rules! empty_named {
    ($name:ident) => {
        kompoz! { @Kompozable fun $name() {} }
    };
}
empty_named!(EmptyToo);

kompoz_file!("tests/kt/empty.kt");

fn empty_composes() {
    let rule = KompozTestRule::new(Empty);
    rule.wait_for_idle();
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
    rule.set_content(AlsoEmpty);
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
    rule.set_content(EmptyToo);
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
}

/// A `.kt` file's declarations: a composable, and a plain function named by a Rust keyword (`r#in`).
fn a_file_composes() {
    let rule = KompozTestRule::new(FromFile);
    rule.wait_for_idle();
    assert_eq!(rule.all_nodes(true).len(), 1, "the root alone");
    r#in();
}

/// The text is the source's: a group's comments kept, the gaps between top-level tokens rebuilt from their lines
/// and columns (a comment there is spacing), a doc comment once; it starts at its first token's line.
fn text_from_the_source() {
    let here = line!();
    let (text, line, rebuilt) = kompoz_kotlin_macro::__kompoz_text! {
        @Kompozable fun A() { /* kept */ "ğü" ; x }
        /** doc */ /* gone */
        val y = 1 // gone
    };
    assert!(!rebuilt);
    assert_eq!(line, here + 2);
    assert_eq!(text, "        @Kompozable fun A() { /* kept */ \"ğü\" ; x }\n        /** doc */\n        val y = 1");
}

/// Tokens whose positions don't add up (here: repeated by a macro): the text rebuilt from the tokens, newlines
/// where Kotlin needs them, doc comments left out.
fn text_rebuilt_from_tokens() {
    macro_rules! twice {
        ($($t:tt)*) => { kompoz_kotlin_macro::__kompoz_text!($($t)* $($t)*) };
    }
    let (text, line, rebuilt) = twice! {
        /// doc
        @Kompozable fun A(x: Int?) { val y = x!! ; Text("a") }
    };
    assert!(rebuilt);
    assert_eq!(line, 1);
    let one = "@Kompozable fun A(x: Int?) {val y = x!!; Text(\"a\")}";
    assert_eq!(text, format!("{one}\n{one}"));
}

/// Runs the tests given one after the other on this thread, made the UI thread; the arguments name the ones to
/// run (all if none).
fn run(tests: &[(&str, fn())]) {
    set_ui_thread();
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        ran += 1;
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{ran} run, {} failed", failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

macro_rules! tests {
    ($($name:ident),* $(,)?) => {
        fn main() {
            run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
}

tests![empty_composes, a_file_composes, text_from_the_source, text_rebuilt_from_tokens];
