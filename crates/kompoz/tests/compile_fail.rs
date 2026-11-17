//! What the compiler says when the Kotlin is wrong, in `kompoz! { … }` and in a `.kt` file (`kompoz_file!`): our
//! errors (a parse error, what comes with stage 14) at the Kotlin token, a `.kt` file's with its `file:line:column`
//! in the message; rustc's errors in the Rust the Kotlin becomes on the Kotlin token (a `.kt` file's: at the call);
//! what Rust's lexer rejects in `kompoz!`, rustc's error and ours saying what to write (or to use a `.kt` file).

use std::path::Path;

#[test]
fn compile_fail() {
    // trybuild compiles the cases as a crate of its own, in the target directory's tests/trybuild/kompoz, and
    // `kompoz_file!` reads relative to the crate's directory: the cases' `.kt` files go there too.
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).parent().expect("the target directory");
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/compile-fail");
    let copies = target.join("tests/trybuild/kompoz/tests/compile-fail");
    std::fs::create_dir_all(&copies).expect("trybuild's crate's directory");
    for entry in std::fs::read_dir(&cases).expect("tests/compile-fail") {
        let path = entry.expect("a case").path();
        if path.extension().is_some_and(|e| e == "kt") {
            std::fs::copy(&path, copies.join(path.file_name().unwrap())).expect("a .kt case copied");
        }
    }
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}
