use kompoz_runtime::KotlinProperties;

// A field whose name a method of the type has already: rustc's duplicate definition at the field (the fix is
// `#[kotlin(skip)]` on it).
#[derive(KotlinProperties)]
pub struct Meters {
    pub value: f32,
}

impl Meters {
    pub fn value(&self) -> f32 {
        self.value
    }
}

// What the derive can't make properties of.
#[derive(KotlinProperties)]
pub struct Pair(pub i32, pub i32);

#[derive(KotlinProperties)]
pub struct Wrong {
    #[kotlin(get)]
    pub a: i32,
}

fn main() {}
