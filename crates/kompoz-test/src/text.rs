//! Text before the text stage: C++'s `FakeTextMeasurer` and a one-line test `Text` measured by it.

use kompoz_graphics::{Color, Density, IntSize, LayoutDirection, Offset, Size, TextUnit, UnitExt};
use kompoz_runtime::kompozable;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::graphics_context::TextDraw;
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
};
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Constraints, FirstBaseline, LastBaseline, Layout, Modifier, ModifierNode, ModifierNodeElement};
use kompoz_ui::{measure_policy, node_capabilities};

/// Deterministic text metrics (C++'s FakeTextMeasurer): every code point half an em wide (stretched by its scale,
/// plus its letter spacing); the ascent 0.8 em, the descent 0.2, a line 1.25.
#[derive(Clone, Copy, Debug, Default)]
pub struct FakeTextMeasurer;

/// A font's metrics in px (C++'s FontMetrics).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}

impl FakeTextMeasurer {
    /// How wide `text` is at `font_size` px.
    pub fn measure_width(&self, text: &str, font_size: f32, scale_x: f32, letter_spacing: f32) -> f32 {
        text.chars().count() as f32 * (font_size * 0.5 * scale_x + letter_spacing)
    }

    pub fn metrics(&self, font_size: f32) -> FontMetrics {
        FontMetrics { ascent: font_size * 0.8, descent: font_size * 0.2, line_height: font_size * 1.25 }
    }
}

/// One line of text measured by the [`FakeTextMeasurer`] (the design's test `Text`): as wide as its text (within
/// the constraints), a line high, its baselines set, its `Text` semantics set, drawn as one text command. Its min
/// intrinsic width is its widest word's, its max its whole width.
#[kompozable]
pub fn Text(text: impl Into<String>, #[default(14.sp())] font_size: TextUnit, #[default(Modifier)] modifier: Modifier) {
    let element = TextElement { text, font_size };
    Layout(
        modifier = modifier.then_element(element),
        measure_policy = measure_policy(|scope, _, c| scope.layout(c.min_width, c.min_height, |_| {})),
    );
}

#[derive(PartialEq)]
struct TextElement {
    text: String,
    font_size: TextUnit,
}

impl ModifierNodeElement for TextElement {
    type Node = TextNode;
    fn create(&self) -> TextNode {
        TextNode { text: self.text.clone(), font_size: self.font_size, size: IntSize::Zero, px: 0.0 }
    }
    fn update(&self, node: &mut TextNode) {
        node.text = self.text.clone();
        node.font_size = self.font_size;
    }
    fn name(&self) -> &'static str {
        "text"
    }
}

/// The test Text's node: it measures, draws and describes the text (as Compose's TextStringSimpleNode).
struct TextNode {
    text: String,
    font_size: TextUnit,
    size: IntSize,
    px: f32,
}

impl TextNode {
    fn font_px(&self, density: &Density) -> f32 {
        if self.font_size.is_sp() { density.text_to_px(self.font_size) } else { density.to_px(14.dp()) }
    }

    fn width(&self, density: &Density) -> f32 {
        FakeTextMeasurer.measure_width(&self.text, self.font_px(density), 1.0, 0.0)
    }

    fn widest_word(&self, density: &Density) -> f32 {
        let px = self.font_px(density);
        self.text.split_whitespace().map(|w| FakeTextMeasurer.measure_width(w, px, 1.0, 0.0)).fold(0.0, f32::max)
    }

    fn line_height(&self, density: &Density) -> f32 {
        FakeTextMeasurer.metrics(self.font_px(density)).line_height
    }
}

impl ModifierNode for TextNode {
    node_capabilities!(layout, draw, semantics);
}

impl LayoutModifierNode for TextNode {
    fn measure(&mut self, scope: &MeasureScope, measurable: &dyn Measurable, c: Constraints) -> MeasureResult {
        let density = scope.density();
        let px = self.font_px(&density);
        let width = c.constrain_width(self.width(&density).ceil() as i32);
        let height = c.constrain_height(self.line_height(&density).ceil() as i32);
        self.size = IntSize::new(width, height);
        self.px = px;
        let placeable = measurable.measure(Constraints::fixed(width, height));
        let baseline = (FakeTextMeasurer.metrics(px).ascent).round() as i32;
        MeasureResult::new(
            width,
            height,
            vec![(FirstBaseline.into(), baseline), (LastBaseline.into(), baseline)],
            move |p| p.place(&placeable, 0, 0),
        )
    }

    fn min_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        self.widest_word(&scope.density()).ceil() as i32
    }
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        self.width(&scope.density()).ceil() as i32
    }
    fn min_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        self.line_height(&scope.density()).ceil() as i32
    }
    fn max_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        self.line_height(&scope.density()).ceil() as i32
    }
}

impl DrawModifierNode for TextNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let metrics = FakeTextMeasurer.metrics(self.px);
        let rtl = scope.layout_direction() == LayoutDirection::Rtl;
        let mut text = TextDraw::new(
            self.text.as_str(),
            Offset::new(0.0, metrics.ascent),
            self.px,
            Size::new(self.width(&scope.density()), metrics.line_height),
        );
        text.rtl = rtl;
        scope.canvas().draw_text(&text, Color::Black);
        scope.draw_content();
    }
}

impl SemanticsModifierNode for TextNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_text(self.text.clone());
    }
}
