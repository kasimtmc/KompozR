use kompoz_runtime::*;

/// A method named as one the API index lists with defaults (`copy(alpha = …, …)`), without any: a positional call
/// past what the indexed one requires goes to a builder this type doesn't have. `ruler.r#copy(2)` (or
/// `#[kompozable(plain(copy))]`) keeps it as written.
#[derive(Clone, Copy, PartialEq)]
struct Ruler(i32);

impl Ruler {
    fn copy(&self, length: i32) -> Ruler {
        Ruler(self.0 + length)
    }
}

#[kompozable]
fn Measure(ruler: Ruler) {
    let _ = ruler.copy(2);
}

fn main() {}
