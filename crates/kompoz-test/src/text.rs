//! A one-line test `Text` measured and drawn by kompoz-text's [`FakeBackend`] (C++'s FakeTextMeasurer's numbers),
//! until foundation's BasicText takes its place.

use kompoz_graphics::{Color, Density, IntSize, LayoutDirection, Offset, TextUnit, UnitExt};
use kompoz_runtime::kompozable;
use kompoz_text::{FakeBackend, FontBackend, TextRunStyle};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
};
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Constraints, FirstBaseline, LastBaseline, Layout, Modifier, ModifierNode, ModifierNodeElement};
use kompoz_ui::{measure_policy, node_capabilities};
use std::rc::Rc;

/// One line of text measured by the [`FakeBackend`] (the design's test `Text`): as wide as its text (within the
/// constraints), a line high, its baselines set, its `Text` semantics set, drawn as one text run. Its min intrinsic
/// width is its widest word's, its max its whole width.
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
        FakeBackend::measure_width(&self.text, self.font_px(density), 1.0, 0.0)
    }

    fn widest_word(&self, density: &Density) -> f32 {
        let px = self.font_px(density);
        self.text.split_whitespace().map(|w| FakeBackend::measure_width(w, px, 1.0, 0.0)).fold(0.0, f32::max)
    }

    fn line_height(&self, density: &Density) -> f32 {
        FakeBackend.metrics(&FakeBackend::instance(self.font_px(density))).line_height()
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
        let baseline = FakeBackend.metrics(&FakeBackend::instance(px)).ascent.round() as i32;
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
        let ascent = FakeBackend.metrics(&FakeBackend::instance(self.px)).ascent;
        let rtl = scope.layout_direction() == LayoutDirection::Rtl;
        let style = Rc::new(TextRunStyle::new(self.px));
        let text = FakeBackend.text_draw(&self.text, Offset::new(0.0, ascent), style, rtl);
        scope.draw_text_run(&text, Color::Black);
        scope.draw_content();
    }
}

impl SemanticsModifierNode for TextNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_text(self.text.as_str());
    }
}
