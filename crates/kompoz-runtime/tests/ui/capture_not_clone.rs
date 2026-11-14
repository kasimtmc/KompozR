use kompoz_runtime::*;

struct Handle;

#[kompozable]
fn Boxed(content: impl Fn()) {
    content();
}

#[kompozable]
fn Screen() {
    let handle = Handle;
    Boxed(|| {
        let _ = &handle;
    });
}

fn main() {}
