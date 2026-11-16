use kompoz_runtime::*;

/// A PascalCase #[named] function named as the type it makes: its builder needs `#[named(type)]` to go on that
/// type, else the hidden type of its name clashes with it.
struct Insets {
    left: i32,
}

#[named]
fn Insets(#[default(0)] left: i32) -> Insets {
    Insets { left }
}

fn main() {
    let _ = Insets().left;
}
