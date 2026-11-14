use kompoz_runtime::*;

#[kompozable]
fn Label(text: String, #[default(16)] size: i32) {
    let _ = (text, size);
}

#[kompozable]
fn Screen() {
    Label("hi".into(), colour = 3);
}

fn main() {}
