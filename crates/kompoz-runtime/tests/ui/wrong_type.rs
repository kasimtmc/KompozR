use kompoz_runtime::*;

#[kompozable]
fn Label(text: String, #[default(16)] size: i32) {
    let _ = (text, size);
}

#[kompozable]
fn Screen() {
    Label("hi".into(), size = "big");
}

fn main() {}
