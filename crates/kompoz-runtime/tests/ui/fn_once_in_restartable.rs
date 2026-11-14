use kompoz_runtime::*;

#[kompozable]
fn Once(content: impl FnOnce()) {
    content();
}

fn main() {}
