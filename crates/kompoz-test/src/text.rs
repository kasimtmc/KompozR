//! A test `Text` laid out by kompoz-text's paragraph engine over the host's font family resolver (the test rule's
//! is the [`FakeBackend`](kompoz_text::FakeBackend)'s: C++'s FakeTextMeasurer's numbers), until foundation's
//! BasicText takes its place.

use kompoz_graphics::{Constraints, IntSize, TextUnit, UnitExt};
use kompoz_runtime::kompozable;
use kompoz_text::{
    AnnotatedString, FontFamilyResolver, MultiParagraph, MultiParagraphIntrinsics, TextLayoutResult, TextMeasurer,
    TextOverflow, TextStyle,
};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
};
use kompoz_ui::platform::LocalFontFamilyResolver;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{FirstBaseline, LastBaseline, Layout, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use kompoz_ui::{measure_policy, node_capabilities};
use std::cell::RefCell;
use std::rc::Rc;

/// Text laid out by the paragraph engine (the design's test `Text`): `font_size` the only style it takes, wrapping
/// in the width it is given (as wide as its widest line, within the constraints), its baselines set (rounded), its
/// `Text` semantics and its layout (`GetTextLayoutResult`) given, drawn as its layout's runs. Its intrinsic widths
/// are Compose's (the widest of its pieces between line breaks, its unwrapped width), its intrinsic heights its
/// height laid out in that width.
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
        TextNode {
            text: self.text.clone(),
            font_size: self.font_size,
            handle: None,
            measurer: None,
            intrinsics: None,
            layout: Rc::new(RefCell::new(None)),
        }
    }
    fn update(&self, node: &mut TextNode) {
        if node.text != self.text || node.font_size != self.font_size {
            node.text = self.text.clone();
            node.font_size = self.font_size;
            node.intrinsics = None;
        }
    }
    fn name(&self) -> &'static str {
        "text"
    }
}

/// The test Text's node: it lays out, draws and describes the text (as Compose's TextStringSimpleNode), keeping a
/// measurer (its last layout cached, so its draw list is the same frame to frame) and its intrinsics.
struct TextNode {
    text: String,
    font_size: TextUnit,
    handle: Option<NodeHandle>,
    measurer: Option<(Rc<dyn FontFamilyResolver>, TextMeasurer)>,
    intrinsics: Option<(Rc<dyn FontFamilyResolver>, kompoz_graphics::Density, Rc<MultiParagraphIntrinsics>)>,
    layout: Rc<RefCell<Option<TextLayoutResult>>>,
}

impl TextNode {
    fn style(&self) -> TextStyle {
        TextStyle { font_size: self.font_size, ..TextStyle::Default }
    }

    fn resolver(&self) -> Rc<dyn FontFamilyResolver> {
        match &self.handle {
            Some(h) => h.current_value_of(&LocalFontFamilyResolver),
            None => LocalFontFamilyResolver.default_value(),
        }
    }

    /// Its text laid out in `constraints`.
    fn lay_out(&mut self, scope: &MeasureScope, c: Constraints) -> TextLayoutResult {
        let resolver = self.resolver();
        let same = self.measurer.as_ref().is_some_and(|(r, _)| Rc::ptr_eq(r, &resolver));
        if !same {
            let m = TextMeasurer::new(resolver.clone(), scope.density(), scope.layout_direction(), 1);
            self.measurer = Some((resolver.clone(), m));
        }
        let (_, measurer) = self.measurer.as_ref().expect("a measurer");
        measurer.measure_with(
            AnnotatedString::new(self.text.as_str()),
            self.style(),
            TextOverflow::Clip,
            true,
            usize::MAX,
            Vec::new(),
            c,
            scope.layout_direction(),
            scope.density(),
            resolver,
            false,
        )
    }

    /// Its text measured (kept while the resolver and density stay).
    fn intrinsics(&mut self, scope: &IntrinsicMeasureScope) -> Rc<MultiParagraphIntrinsics> {
        let resolver = self.resolver();
        let density = scope.density();
        if let Some((r, d, i)) = &self.intrinsics
            && Rc::ptr_eq(r, &resolver)
            && *d == density
        {
            return i.clone();
        }
        let style = self.style().resolve_defaults(scope.layout_direction());
        let i = Rc::new(MultiParagraphIntrinsics::new(
            &AnnotatedString::new(self.text.as_str()),
            &style,
            &[],
            density,
            &resolver,
            true,
        ));
        self.intrinsics = Some((resolver, density, i.clone()));
        i
    }

    /// How tall it is laid out `width` px wide (as wide as it takes within that).
    fn height_in(&mut self, scope: &IntrinsicMeasureScope, width: i32) -> i32 {
        let intrinsics = self.intrinsics(scope);
        let w = (intrinsics.max_intrinsic_width().ceil() as i32).clamp(0, width.max(0));
        let laid = MultiParagraph::new(
            intrinsics,
            Constraints::new(0, w, 0, Constraints::Infinity),
            usize::MAX,
            TextOverflow::Clip,
        );
        laid.height().ceil() as i32
    }
}

impl ModifierNode for TextNode {
    node_capabilities!(layout, draw, semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.handle = None;
    }
}

impl LayoutModifierNode for TextNode {
    fn measure(&mut self, scope: &MeasureScope, measurable: &dyn Measurable, c: Constraints) -> MeasureResult {
        let result = self.lay_out(scope, c);
        let IntSize { width, height } = result.size;
        let (first, last) = (result.first_baseline().round() as i32, result.last_baseline().round() as i32);
        *self.layout.borrow_mut() = Some(result);
        let placeable = measurable.measure(Constraints::fixed(width, height));
        MeasureResult::new(width, height, vec![(FirstBaseline.into(), first), (LastBaseline.into(), last)], move |p| {
            p.place(&placeable, 0, 0)
        })
    }

    fn min_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        self.intrinsics(scope).min_intrinsic_width().ceil() as i32
    }
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        self.intrinsics(scope).max_intrinsic_width().ceil() as i32
    }
    fn min_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, w: i32) -> i32 {
        self.height_in(scope, w)
    }
    fn max_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, w: i32) -> i32 {
        self.height_in(scope, w)
    }
}

impl DrawModifierNode for TextNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let layout = self.layout.borrow().clone();
        if let Some(result) = layout {
            scope.draw_text(&result);
        }
        scope.draw_content();
    }
}

impl SemanticsModifierNode for TextNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_text(self.text.as_str());
        let layout = self.layout.clone();
        config.get_text_layout_result(move |out: &mut Vec<TextLayoutResult>| match layout.borrow().clone() {
            Some(r) => {
                out.push(r);
                true
            }
            None => false,
        });
    }
}
