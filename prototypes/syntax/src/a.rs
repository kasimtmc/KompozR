//! Form A: functions and methods with named, defaulted parameters (`#[kompozable]`, `#[named]`), called as in
//! Kotlin from `#[kompozable]` functions. These stand-ins print what they were called with.

use crate::common::*;
use kompoz_syntax_proto_macros::{kompozable, named};

#[named]
impl Modifier {
    pub fn padding(self, padding: impl Into<PaddingValues>) -> Modifier {
        let p = padding.into();
        self.then(format!("padding({}, {}, {}, {})", p.start.0, p.top.0, p.end.0, p.bottom.0))
    }
    pub fn fill_max_width(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then(format!("fillMaxWidth({fraction})"))
    }
    pub fn fill_max_height(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then(format!("fillMaxHeight({fraction})"))
    }
    pub fn background(self, brush: impl Into<Brush>, #[default(RectangleShape)] shape: Shape) -> Modifier {
        self.then(format!("background({:?}, {shape:?})", brush.into()))
    }
}

#[named]
impl Color {
    pub fn copy(
        self,
        #[default(self.alpha)] alpha: f32,
        #[default(self.red)] red: f32,
        #[default(self.green)] green: f32,
        #[default(self.blue)] blue: f32,
    ) -> Color {
        Color { red, green, blue, alpha }
    }
}

#[named]
pub fn RoundedCornerShape(size: Dp) -> Shape {
    Shape::RoundedCorner(size)
}

#[kompozable]
pub fn MaterialTheme(#[default(light_color_scheme())] color_scheme: ColorScheme, content: impl Fn() + 'static) {
    let _ = color_scheme;
    node("MaterialTheme".into(), content);
}
/// `MaterialTheme.colorScheme`, `MaterialTheme.typography`.
impl __kz_MaterialTheme::Marker {
    pub fn color_scheme(&self) -> ColorScheme {
        light_color_scheme()
    }
    pub fn typography(&self) -> Typography {
        typography()
    }
}

#[kompozable]
pub fn Text(
    text: impl Into<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(TextAlign::Start)] text_align: TextAlign,
    #[default(typography().body_large)] style: TextStyle,
) {
    node(format!("Text({:?}, {modifier:?}, {text_align:?}, {style:?})", text.into()), || {});
}

#[kompozable]
pub fn Column(
    #[default(Modifier)] modifier: Modifier,
    #[default(Arrangement::Top)] vertical_arrangement: Arrangement,
    #[default(Alignment::Start)] horizontal_alignment: Alignment,
    content: impl Fn() + 'static,
) {
    node(format!("Column({modifier:?}, {vertical_arrangement:?}, {horizontal_alignment:?})"), content);
}

#[kompozable]
pub fn Box(#[default(Modifier)] modifier: Modifier, content: impl Fn() + 'static) {
    node(format!("Box({modifier:?})"), content);
}

#[kompozable]
pub fn Button(on_click: impl Fn() + 'static, #[default(Modifier)] modifier: Modifier, content: impl Fn() + 'static) {
    clickable("Button", on_click);
    node(format!("Button({modifier:?})"), content);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopAppBarColors {
    pub container_color: Color,
    pub title_content_color: Color,
}
#[named]
pub fn top_app_bar_colors(
    #[default(light_color_scheme().primary_container)] container_color: Color,
    #[default(light_color_scheme().on_background)] title_content_color: Color,
) -> TopAppBarColors {
    TopAppBarColors { container_color, title_content_color }
}

#[kompozable]
pub fn Scaffold(
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] top_bar: impl Fn() + 'static,
    #[default(|| {})] bottom_bar: impl Fn() + 'static,
    #[default(|| {})] floating_action_button: impl Fn() + 'static,
    content: impl Fn(PaddingValues) + 'static,
) {
    node(format!("Scaffold({modifier:?})"), || {
        node("topBar".into(), &top_bar);
        node("bottomBar".into(), &bottom_bar);
        node("floatingActionButton".into(), &floating_action_button);
        let inner = PaddingValues { start: Dp(0.0), top: Dp(64.0), end: Dp(0.0), bottom: Dp(80.0) };
        node("content".into(), || content(inner));
    });
}

#[kompozable]
pub fn TopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(top_app_bar_colors())] colors: TopAppBarColors,
) {
    node(format!("TopAppBar({modifier:?}, {colors:?})"), title);
}

#[kompozable]
pub fn BottomAppBar(
    #[default(Modifier)] modifier: Modifier,
    #[default(light_color_scheme().primary_container)] container_color: Color,
    #[default(light_color_scheme().on_background)] content_color: Color,
    content: impl Fn() + 'static,
) {
    node(format!("BottomAppBar({modifier:?}, {container_color:?}, {content_color:?})"), content);
}

#[kompozable]
pub fn FloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    clickable("FloatingActionButton", on_click);
    node(format!("FloatingActionButton({modifier:?})"), content);
}

#[kompozable]
pub fn Icon(painter: Painter, content_description: impl Into<String>, #[default(Modifier)] modifier: Modifier) {
    node(format!("Icon({:?}, {:?}, {modifier:?})", painter.0, content_description.into()), || {});
}
