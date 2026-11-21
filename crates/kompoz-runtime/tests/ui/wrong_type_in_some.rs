use kompoz_runtime::*;
use std::rc::Rc;

#[kompozable]
fn Iconed(text: String, #[default(None)] icon: Option<Rc<dyn Fn() -> i32>>) {
    let _ = (text, icon);
}

#[kompozable]
fn Screen() {
    Iconed("hi".into(), icon = Some(Rc::new(|| "five")));
}

fn main() {}
