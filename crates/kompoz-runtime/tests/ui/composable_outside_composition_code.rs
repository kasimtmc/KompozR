use kompoz_runtime::*;

#[kompozable]
fn Label(text: String) {
    let _ = text;
}

fn plain() {
    Label("not in a #[kompozable] body".into());
}

fn main() {}
