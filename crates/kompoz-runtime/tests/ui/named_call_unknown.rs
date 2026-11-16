use kompoz_runtime::*;

/// A PascalCase #[named] function the macros don't know of (not in the API index, not defined in the body): a
/// call of it with named arguments can't find its builder.
#[named]
fn Corner(#[default(1)] radius: i32) -> i32 {
    radius
}

#[kompozable]
fn Screen() {
    let _ = Corner(radius = 2);
}

fn main() {}
