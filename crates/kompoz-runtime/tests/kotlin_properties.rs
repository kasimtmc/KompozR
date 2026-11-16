//! `#[derive(KotlinProperties)]`: a struct's public fields read and written as the Kotlin syntax reads and
//! writes properties (`a.b` → `a.b()`, `a.b = v` → `a.set_b(v)`).

use kompoz_runtime::KotlinProperties;

#[derive(Debug, PartialEq, KotlinProperties)]
pub struct Ticket {
    pub id: i32,
    pub title: String,
    #[kotlin(set)]
    pub seats: u8,
    #[kotlin(ref)]
    pub notes: NotClone,
    #[kotlin(skip)]
    pub price: f32,
    secret: bool,
    pub(crate) font_scale: f32,
    pub r#type: char,
}

#[derive(Debug, PartialEq)]
pub struct NotClone(i32);

impl Ticket {
    /// A method of a skipped field's name stays the type's own.
    pub fn price(&self) -> String {
        format!("{} TL", self.price)
    }
    pub fn secret(&self) -> bool {
        self.secret
    }
}

/// Every field written: `#[kotlin(set)]` on the struct.
#[derive(Default, KotlinProperties)]
#[kotlin(set)]
pub struct Layer<T> {
    pub alpha: f32,
    pub tag: T,
}

#[test]
fn getters_of_the_public_fields() {
    let t = Ticket {
        id: 7,
        title: "Konser".to_string(),
        seats: 2,
        notes: NotClone(1),
        price: 9.5,
        secret: true,
        font_scale: 1.5,
        r#type: 'k',
    };
    assert_eq!(t.id(), 7);
    assert_eq!(t.title(), "Konser");
    assert_eq!(t.seats(), 2);
    assert_eq!(t.notes(), &NotClone(1));
    assert_eq!(t.price(), "9.5 TL");
    assert!(t.secret());
    assert_eq!(t.font_scale(), 1.5);
    assert_eq!(t.r#type(), 'k');
}

#[test]
fn setters_where_kotlin_writes() {
    let mut t = Ticket {
        id: 1,
        title: String::new(),
        seats: 0,
        notes: NotClone(0),
        price: 0.0,
        secret: false,
        font_scale: 1.0,
        r#type: 'a',
    };
    t.set_seats(4);
    assert_eq!(t.seats(), 4);
    let mut layer = Layer { alpha: 1.0, tag: vec![1] };
    layer.set_alpha(0.5);
    layer.set_tag(vec![2, 3]);
    assert_eq!((layer.alpha(), layer.tag()), (0.5, vec![2, 3]));
}
