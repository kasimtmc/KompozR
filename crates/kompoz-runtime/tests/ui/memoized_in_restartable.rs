use kompoz_runtime::*;

/// `#[memoized]` hands the body a parameter's identity; a restartable composable keeps its closures by theirs
/// already.
#[kompozable]
fn Clicker(#[memoized] on_click: impl Fn()) {
    let _ = on_click;
}

fn main() {}
