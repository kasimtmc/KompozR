//! Form B: plain Rust functions; the required parameters come first, the optional ones in an `…Args` struct
//! filled with `..default()`. Methods have one name per form (`fill_max_height_fraction`). `#[kompozable(plain)]`
//! only makes the closures passed to calls capture by value.

use crate::common::*;

pub fn default<T: Default>() -> T {
    T::default()
}

pub trait ModifierB {
    fn padding(self, padding: impl Into<PaddingValues>) -> Modifier;
    fn fill_max_width(self) -> Modifier;
    fn fill_max_height_fraction(self, fraction: f32) -> Modifier;
    fn background_brush(self, brush: impl Into<Brush>, shape: Shape) -> Modifier;
}
impl ModifierB for Modifier {
    fn padding(self, padding: impl Into<PaddingValues>) -> Modifier {
        let p = padding.into();
        self.then(format!("padding({}, {}, {}, {})", p.start.0, p.top.0, p.end.0, p.bottom.0))
    }
    fn fill_max_width(self) -> Modifier {
        self.then("fillMaxWidth(1)".into())
    }
    fn fill_max_height_fraction(self, fraction: f32) -> Modifier {
        self.then(format!("fillMaxHeight({fraction})"))
    }
    fn background_brush(self, brush: impl Into<Brush>, shape: Shape) -> Modifier {
        self.then(format!("background({:?}, {shape:?})", brush.into()))
    }
}

pub trait ColorB {
    fn with_alpha(self, alpha: f32) -> Color;
}
impl ColorB for Color {
    fn with_alpha(self, alpha: f32) -> Color {
        Color { alpha, ..self }
    }
}

pub fn RoundedCornerShape(size: Dp) -> Shape {
    Shape::RoundedCorner(size)
}

/// `MaterialTheme.colorScheme`, `MaterialTheme.typography`.
pub struct MaterialTheme {}
impl MaterialTheme {
    pub fn color_scheme() -> ColorScheme {
        light_color_scheme()
    }
    pub fn typography() -> Typography {
        typography()
    }
}
pub fn MaterialTheme(color_scheme: ColorScheme, content: impl Fn() + 'static) {
    let _ = color_scheme;
    node("MaterialTheme".into(), content);
}

#[derive(Clone, Debug)]
pub struct TextArgs {
    pub modifier: Modifier,
    pub text_align: TextAlign,
    pub style: TextStyle,
}
impl Default for TextArgs {
    fn default() -> Self {
        TextArgs { modifier: Modifier, text_align: TextAlign::Start, style: typography().body_large }
    }
}
pub fn Text(text: impl Into<String>, args: TextArgs) {
    let TextArgs { modifier, text_align, style } = args;
    node(format!("Text({:?}, {modifier:?}, {text_align:?}, {style:?})", text.into()), || {});
}

#[derive(Clone, Debug)]
pub struct ColumnArgs {
    pub modifier: Modifier,
    pub vertical_arrangement: Arrangement,
    pub horizontal_alignment: Alignment,
}
impl Default for ColumnArgs {
    fn default() -> Self {
        ColumnArgs {
            modifier: Modifier,
            vertical_arrangement: Arrangement::Top,
            horizontal_alignment: Alignment::Start,
        }
    }
}
pub fn Column(args: ColumnArgs, content: impl Fn() + 'static) {
    let ColumnArgs { modifier, vertical_arrangement, horizontal_alignment } = args;
    node(format!("Column({modifier:?}, {vertical_arrangement:?}, {horizontal_alignment:?})"), content);
}

#[derive(Clone, Debug)]
pub struct BoxArgs {
    pub modifier: Modifier,
}
impl Default for BoxArgs {
    fn default() -> Self {
        BoxArgs { modifier: Modifier }
    }
}
pub fn Box(args: BoxArgs, content: impl Fn() + 'static) {
    node(format!("Box({:?})", args.modifier), content);
}

#[derive(Clone, Debug)]
pub struct ButtonArgs {
    pub modifier: Modifier,
}
impl Default for ButtonArgs {
    fn default() -> Self {
        ButtonArgs { modifier: Modifier }
    }
}
pub fn Button(on_click: impl Fn() + 'static, args: ButtonArgs, content: impl Fn() + 'static) {
    clickable("Button", on_click);
    node(format!("Button({:?})", args.modifier), content);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopAppBarColors {
    pub container_color: Color,
    pub title_content_color: Color,
}
impl Default for TopAppBarColors {
    fn default() -> Self {
        TopAppBarColors {
            container_color: light_color_scheme().primary_container,
            title_content_color: light_color_scheme().on_background,
        }
    }
}

/// An optional slot: `slot(|| …)`, or nothing.
#[derive(Default)]
pub struct Slot(Option<std::boxed::Box<dyn Fn()>>);
pub fn slot(f: impl Fn() + 'static) -> Slot {
    Slot(Some(std::boxed::Box::new(f)))
}
impl Slot {
    fn call(&self) {
        if let Some(f) = &self.0 {
            f()
        }
    }
}

#[derive(Default)]
pub struct ScaffoldArgs {
    pub modifier: Modifier,
    pub top_bar: Slot,
    pub bottom_bar: Slot,
    pub floating_action_button: Slot,
}
impl Default for Modifier {
    fn default() -> Self {
        Modifier
    }
}
pub fn Scaffold(args: ScaffoldArgs, content: impl Fn(PaddingValues) + 'static) {
    node(format!("Scaffold({:?})", args.modifier), || {
        node("topBar".into(), || args.top_bar.call());
        node("bottomBar".into(), || args.bottom_bar.call());
        node("floatingActionButton".into(), || args.floating_action_button.call());
        let inner = PaddingValues { start: Dp(0.0), top: Dp(64.0), end: Dp(0.0), bottom: Dp(80.0) };
        node("content".into(), || content(inner));
    });
}

#[derive(Default)]
pub struct TopAppBarArgs {
    pub modifier: Modifier,
    pub colors: TopAppBarColors,
}
pub fn TopAppBar(title: impl Fn() + 'static, args: TopAppBarArgs) {
    node(format!("TopAppBar({:?}, {:?})", args.modifier, args.colors), title);
}

pub struct BottomAppBarArgs {
    pub modifier: Modifier,
    pub container_color: Color,
    pub content_color: Color,
}
impl Default for BottomAppBarArgs {
    fn default() -> Self {
        BottomAppBarArgs {
            modifier: Modifier,
            container_color: light_color_scheme().primary_container,
            content_color: light_color_scheme().on_background,
        }
    }
}
pub fn BottomAppBar(args: BottomAppBarArgs, content: impl Fn() + 'static) {
    let BottomAppBarArgs { modifier, container_color, content_color } = args;
    node(format!("BottomAppBar({modifier:?}, {container_color:?}, {content_color:?})"), content);
}

#[derive(Default)]
pub struct FloatingActionButtonArgs {
    pub modifier: Modifier,
}
pub fn FloatingActionButton(
    on_click: impl Fn() + 'static,
    args: FloatingActionButtonArgs,
    content: impl Fn() + 'static,
) {
    clickable("FloatingActionButton", on_click);
    node(format!("FloatingActionButton({:?})", args.modifier), content);
}

#[derive(Default)]
pub struct IconArgs {
    pub modifier: Modifier,
}
pub fn Icon(painter: Painter, content_description: impl Into<String>, args: IconArgs) {
    node(format!("Icon({:?}, {:?}, {:?})", painter.0, content_description.into(), args.modifier), || {});
}
