use kompoz_runtime::*;

#[kompozable]
fn Show(value: impl std::fmt::Display) {
    let _ = value.to_string();
}

fn main() {}
