//! What the compiler says when `#[kompozable]` code is wrong: the messages point at the call and name the problem.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
