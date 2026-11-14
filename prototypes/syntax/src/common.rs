//! What both forms share: values (Dp, Color, Modifier…), a stand-in for state, and a tree printer in place of a
//! real composition, so that a sample can be run and its output read.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

// --- A stand-in for the runtime ------------------------------------------------------------------------------

type Clicks = Vec<(String, Rc<dyn Fn()>)>;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static CLICKS: RefCell<Clicks> = const { RefCell::new(Vec::new()) };
    static SLOTS: RefCell<(usize, Vec<Rc<dyn std::any::Any>>)> = const { RefCell::new((0, Vec::new())) };
}

/// Prints `label` at the current depth and runs `content` one level deeper.
pub fn node(label: String, content: impl FnOnce()) {
    let d = DEPTH.with(|d| d.get());
    println!("{}{}", "  ".repeat(d), label);
    DEPTH.with(|x| x.set(d + 1));
    content();
    DEPTH.with(|x| x.set(d));
}

/// Keeps a click handler, so that `click(label)` can run it after the composition.
pub fn clickable(label: &str, on_click: impl Fn() + 'static) {
    CLICKS.with(|c| c.borrow_mut().push((label.to_string(), Rc::new(on_click))));
}

/// Composes `root` as if it ran again after a change (remembered values come back in call order).
pub fn compose(root: impl Fn()) {
    CLICKS.with(|c| c.borrow_mut().clear());
    SLOTS.with(|s| s.borrow_mut().0 = 0);
    root();
    println!();
}

pub fn click(label: &str) {
    let f = CLICKS.with(|c| c.borrow().iter().find(|(l, _)| l == label).map(|(_, f)| f.clone()));
    println!("> click {label:?}");
    f.expect("no such clickable")();
}

/// `remember { … }`: the value made in the first composition, the same one afterwards.
pub fn remember<T: Clone + 'static>(init: impl FnOnce() -> T) -> T {
    SLOTS.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.0;
        s.0 += 1;
        if i == s.1.len() {
            s.1.push(Rc::new(init()));
        }
        s.1[i].downcast_ref::<T>().unwrap().clone()
    })
}

/// A copyable handle to a value (in KompozR, a record in the snapshot system).
pub struct MutableState<T: 'static>(&'static RefCell<T>);
impl<T> Clone for MutableState<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for MutableState<T> {}
impl<T: Clone> MutableState<T> {
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
    pub fn set(&self, v: T) {
        *self.0.borrow_mut() = v;
    }
    pub fn update(&self, f: impl FnOnce(T) -> T) {
        let v = f(self.get());
        self.set(v);
    }
}
impl<T: fmt::Display> fmt::Display for MutableState<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.borrow().fmt(f)
    }
}
pub fn mutable_state_of<T>(v: T) -> MutableState<T> {
    MutableState(Box::leak(Box::new(RefCell::new(v))))
}
pub fn mutable_int_state_of(v: i32) -> MutableState<i32> {
    mutable_state_of(v)
}

// --- Values -----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dp(pub f32);
pub trait DpExt {
    fn dp(self) -> Dp;
}
impl DpExt for i32 {
    fn dp(self) -> Dp {
        Dp(self as f32)
    }
}
impl DpExt for f64 {
    fn dp(self) -> Dp {
        Dp(self as f32)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}
impl Color {
    pub const Transparent: Color = Color::rgb(0, 0, 0, 0.0);
    pub const fn rgb(r: u8, g: u8, b: u8, alpha: f32) -> Color {
        Color { red: r as f32 / 255.0, green: g as f32 / 255.0, blue: b as f32 / 255.0, alpha }
    }
}
impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = |v: f32| (v * 255.0).round() as u8;
        write!(f, "#{:02X}{:02X}{:02X}{:02X}", c(self.alpha), c(self.red), c(self.green), c(self.blue))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorScheme {
    pub primary: Color,
    pub on_primary_container: Color,
    pub primary_container: Color,
    pub tertiary_container: Color,
    pub on_background: Color,
}
pub fn light_color_scheme() -> ColorScheme {
    ColorScheme {
        primary: Color::rgb(0x65, 0x55, 0x8F, 1.0),
        on_primary_container: Color::rgb(0x21, 0x0F, 0x48, 1.0),
        primary_container: Color::rgb(0xE9, 0xDD, 0xFF, 1.0),
        tertiary_container: Color::rgb(0xFF, 0xD8, 0xE4, 1.0),
        on_background: Color::rgb(0x1D, 0x1B, 0x20, 1.0),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font_size: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub title_large: TextStyle,
    pub body_large: TextStyle,
}
pub fn typography() -> Typography {
    Typography { title_large: TextStyle { font_size: 22.0 }, body_large: TextStyle { font_size: 16.0 } }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arrangement {
    Top,
    SpaceEvenly,
    SpacedBy(Dp),
}
impl Arrangement {
    pub fn spaced_by(space: Dp) -> Arrangement {
        Arrangement::SpacedBy(space)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Alignment {
    Start,
    CenterHorizontally,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Start,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaddingValues {
    pub start: Dp,
    pub top: Dp,
    pub end: Dp,
    pub bottom: Dp,
}
impl From<Dp> for PaddingValues {
    fn from(d: Dp) -> Self {
        PaddingValues { start: d, top: d, end: d, bottom: d }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rectangle,
    RoundedCorner(Dp),
}
pub const RectangleShape: Shape = Shape::Rectangle;

#[derive(Clone, Debug, PartialEq)]
pub enum Brush {
    Solid(Color),
    LinearGradient(Vec<Color>),
}
impl Brush {
    pub fn linear_gradient(colors: Vec<Color>) -> Brush {
        Brush::LinearGradient(colors)
    }
}
impl From<Color> for Brush {
    fn from(c: Color) -> Self {
        Brush::Solid(c)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageVector(pub &'static str);
#[allow(non_snake_case)]
pub mod Icons {
    pub mod Default {
        #[allow(non_upper_case_globals)]
        pub const Add: super::super::ImageVector = super::super::ImageVector("Add");
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Painter(pub ImageVector);
pub fn remember_vector_painter(image: ImageVector) -> Painter {
    Painter(image)
}

/// Compose's `Modifier`: the type, and (as a constant of the same name) the empty chain to start one from.
#[derive(Clone, PartialEq)]
pub struct Modifier {
    pub elements: Vec<String>,
}
#[allow(non_upper_case_globals)]
pub const Modifier: Modifier = Modifier { elements: Vec::new() };
impl Modifier {
    pub fn then(mut self, element: String) -> Modifier {
        self.elements.push(element);
        self
    }
}
impl fmt::Debug for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Modifier")?;
        for e in &self.elements {
            write!(f, ".{e}")?;
        }
        Ok(())
    }
}
