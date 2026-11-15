//! An annotated string's text node (Compose's TextAnnotatedStringElement and TextAnnotatedStringNode): a
//! [`MultiParagraph`] laid out and kept (at an auto size's size), drawn (the selection under it, the links' and
//! inline content's nodes over it), its baselines and placeholders given, told to `on_text_layout` when it is laid
//! out anew, described to accessibility.
//!
//! [`MultiParagraph`]: kompoz_text::MultiParagraph

use super::layout_utils::round_to_int;
use super::min_lines_constrainer::same_resolver;
use super::multi_paragraph_layout_cache::MultiParagraphLayoutCache;
use super::selection_controller::SelectionController;
use super::{invalidate_for_translate, text_paint_style, with_color};
use crate::text::auto_size::{TextAutoSize, same_auto_size};
use kompoz_graphics::{Color, ColorProducer, Constraints, Density, Rect};
use kompoz_text::{
    AnnotatedString, FontFamilyResolver, Placeholder, Range, TextLayoutInput, TextLayoutResult, TextOverflow, TextStyle,
};
use kompoz_ui::draw::{ContentDrawScope, DefaultBlendMode, DrawModifierNode};
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
};
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{
    FirstBaseline, LastBaseline, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::rc::Rc;

/// What a text calls when it is laid out anew (Compose's `onTextLayout`).
pub type OnTextLayout = Rc<dyn Fn(&TextLayoutResult)>;

/// What a text calls with where its placeholders are after each measure (None where one was cut off).
pub(crate) type OnPlaceholderLayout = Rc<dyn Fn(&[Option<Rect>])>;

/// What a text calls when its substitution is shown or hidden (Compose's onShowTranslation).
pub(crate) type OnShowTranslation = Rc<dyn Fn(&TextSubstitutionValue)>;

/// The same callback (both none, or the same one).
pub(crate) fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => std::ptr::eq(Rc::as_ptr(a) as *const (), Rc::as_ptr(b) as *const ()),
        _ => false,
    }
}

/// The element of an annotated string's text (Compose's TextAnnotatedStringElement): equal when every argument is
/// (callbacks, the resolver and the selection controller the same ones).
#[derive(Clone)]
pub(crate) struct TextAnnotatedStringElement {
    pub(crate) text: AnnotatedString,
    pub(crate) style: TextStyle,
    pub(crate) font_family_resolver: Rc<dyn FontFamilyResolver>,
    pub(crate) on_text_layout: Option<OnTextLayout>,
    pub(crate) overflow: TextOverflow,
    pub(crate) soft_wrap: bool,
    pub(crate) max_lines: usize,
    pub(crate) min_lines: usize,
    pub(crate) placeholders: Option<Vec<Range<Placeholder>>>,
    pub(crate) on_placeholder_layout: Option<OnPlaceholderLayout>,
    pub(crate) selection_controller: Option<Rc<SelectionController>>,
    pub(crate) color: Option<ColorProducer>,
    pub(crate) auto_size: Option<Rc<dyn TextAutoSize>>,
    pub(crate) on_show_translation: Option<OnShowTranslation>,
}

impl PartialEq for TextAnnotatedStringElement {
    fn eq(&self, o: &TextAnnotatedStringElement) -> bool {
        self.color == o.color
            && self.text == o.text
            && self.style == o.style
            && self.placeholders == o.placeholders
            && same_resolver(&self.font_family_resolver, &o.font_family_resolver)
            && same_rc(&self.on_text_layout, &o.on_text_layout)
            && same_rc(&self.on_show_translation, &o.on_show_translation)
            && self.overflow == o.overflow
            && self.soft_wrap == o.soft_wrap
            && self.max_lines == o.max_lines
            && self.min_lines == o.min_lines
            && same_rc(&self.on_placeholder_layout, &o.on_placeholder_layout)
            && same_rc(&self.selection_controller, &o.selection_controller)
            && same_auto_size(&self.auto_size, &o.auto_size)
    }
}

impl TextAnnotatedStringElement {
    /// Its node (Compose's TextAnnotatedStringNode(…)), reaching itself through `access` in its semantics.
    pub(crate) fn create_node(&self, access: NodeAccess) -> TextAnnotatedStringNode {
        TextAnnotatedStringNode {
            text: self.text.clone(),
            style: self.style.clone(),
            font_family_resolver: self.font_family_resolver.clone(),
            on_text_layout: self.on_text_layout.clone(),
            overflow: self.overflow,
            soft_wrap: self.soft_wrap,
            max_lines: self.max_lines,
            min_lines: self.min_lines,
            placeholders: self.placeholders.clone(),
            on_placeholder_layout: self.on_placeholder_layout.clone(),
            selection_controller: self.selection_controller.clone(),
            override_color: self.color.clone(),
            auto_size: self.auto_size.clone(),
            on_show_translation: self.on_show_translation.clone(),
            layout_cache: None,
            baseline_cache: None,
            text_substitution: None,
            semantics_applied: false,
            handle: None,
            access,
        }
    }

    /// Updates `node` to it (Compose's update: each kind of change, then what it invalidates).
    pub(crate) fn update_node(&self, node: &mut TextAnnotatedStringNode) {
        let draw_changed = node.update_draw(self.color.clone(), &self.style);
        let text_changed = node.update_text(&self.text);
        let layout_changed = node.update_layout_related_args(
            self.style.clone(),
            self.placeholders.clone(),
            self.min_lines,
            self.max_lines,
            self.soft_wrap,
            self.font_family_resolver.clone(),
            self.overflow,
            self.auto_size.clone(),
        );
        let callbacks_changed = node.update_callbacks(
            self.on_text_layout.clone(),
            self.on_placeholder_layout.clone(),
            self.selection_controller.clone(),
            self.on_show_translation.clone(),
        );
        node.do_invalidations(draw_changed, text_changed, layout_changed, callbacks_changed);
    }
}

impl ModifierNodeElement for TextAnnotatedStringElement {
    type Node = TextAnnotatedStringNode;

    fn create(&self) -> TextAnnotatedStringNode {
        self.create_node(|cx, f| {
            cx.with(|n: &mut TextAnnotatedStringNode| f(n));
        })
    }

    fn update(&self, node: &mut TextAnnotatedStringNode) {
        self.update_node(node);
    }

    fn name(&self) -> &'static str {
        "textAnnotatedString"
    }
}

/// How a text node's semantics actions reach it later: through the node of its chain it is (itself, or the
/// selectable node holding it).
pub(crate) type NodeAccess = fn(&NodeCx, &mut dyn FnMut(&mut TextAnnotatedStringNode));

/// `f` run on the text node `access` reaches from `handle` (none once it is gone).
fn with_node<R>(
    handle: &Option<NodeHandle>,
    access: NodeAccess,
    f: impl FnOnce(&mut TextAnnotatedStringNode) -> R,
) -> Option<R> {
    let cx = handle.as_ref()?.cx()?;
    let mut f = Some(f);
    let mut out = None;
    access(&cx, &mut |n| {
        if let Some(f) = f.take() {
            out = Some(f(n));
        }
    });
    out
}

/// A text substitution of an annotated string's (Compose's TextAnnotatedStringNode.TextSubstitutionValue): the
/// text it stands for, what it shows instead, whether it shows it, and its own layout.
pub struct TextSubstitutionValue {
    pub original: AnnotatedString,
    pub substitution: AnnotatedString,
    pub is_showing_substitution: bool,
    pub(crate) layout_cache: MultiParagraphLayoutCache,
}

/// The node of an annotated string's text (Compose's TextAnnotatedStringNode): lays the text out (kept in a
/// [`MultiParagraphLayoutCache`]), tells `on_text_layout` and the selection when it is laid out anew and
/// `on_placeholder_layout` where its placeholders are, draws it (clipped where it overflows), gives its
/// baselines, says it to accessibility with its substitution actions; it invalidates only what an update
/// changes, as Compose's `doInvalidations`.
pub(crate) struct TextAnnotatedStringNode {
    text: AnnotatedString,
    style: TextStyle,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    on_text_layout: Option<OnTextLayout>,
    overflow: TextOverflow,
    soft_wrap: bool,
    max_lines: usize,
    min_lines: usize,
    placeholders: Option<Vec<Range<Placeholder>>>,
    on_placeholder_layout: Option<OnPlaceholderLayout>,
    selection_controller: Option<Rc<SelectionController>>,
    override_color: Option<ColorProducer>,
    auto_size: Option<Rc<dyn TextAutoSize>>,
    on_show_translation: Option<OnShowTranslation>,
    layout_cache: Option<MultiParagraphLayoutCache>,
    /// Its first and last baselines, from the last layout that changed.
    baseline_cache: Option<(i32, i32)>,
    pub(crate) text_substitution: Option<TextSubstitutionValue>,
    /// Whether its semantics were asked for (Compose's semanticsTextLayoutResult set).
    semantics_applied: bool,
    handle: Option<NodeHandle>,
    access: NodeAccess,
}

impl TextAnnotatedStringNode {
    /// Its own layout cache, made when first needed.
    fn layout_cache(&mut self) -> &mut MultiParagraphLayoutCache {
        self.layout_cache.get_or_insert_with(|| {
            MultiParagraphLayoutCache::new(
                self.text.clone(),
                self.style.clone(),
                self.font_family_resolver.clone(),
                self.overflow,
                self.soft_wrap,
                self.max_lines,
                self.min_lines,
                self.placeholders.clone(),
                self.auto_size.clone(),
            )
        })
    }

    /// The cache of what it shows (its substitution's while it shows one), laying out in `density` (Compose's
    /// getLayoutCache(density)).
    fn active_layout_cache(&mut self, density: Density) -> &mut MultiParagraphLayoutCache {
        self.layout_cache();
        let showing = self.text_substitution.as_ref().is_some_and(|s| s.is_showing_substitution);
        let cache = match &mut self.text_substitution {
            Some(s) if showing => &mut s.layout_cache,
            _ => self.layout_cache.as_mut().expect("kompoz: a layout cache"),
        };
        cache.set_density(Some(density));
        cache
    }

    /// Takes a new color and style; whether it draws otherwise (Compose's updateDraw).
    fn update_draw(&mut self, color: Option<ColorProducer>, style: &TextStyle) -> bool {
        let changed = color != self.override_color;
        self.override_color = color;
        changed || !style.has_same_draw_affecting_attributes(&self.style)
    }

    /// Takes a new text; whether it or its annotations changed (Compose's updateText): other characters drop the
    /// substitution.
    fn update_text(&mut self, text: &AnnotatedString) -> bool {
        let char_diff = self.text.text() != text.text();
        let annotation_diff = !self.text.has_equal_annotations(text);
        if char_diff || annotation_diff {
            self.text = text.clone();
        }
        if char_diff {
            self.text_substitution = None;
        }
        char_diff || annotation_diff
    }

    /// Takes the arguments its layout depends on; whether any lays it out otherwise (Compose's
    /// updateLayoutRelatedArgs).
    #[allow(clippy::too_many_arguments)]
    fn update_layout_related_args(
        &mut self,
        style: TextStyle,
        placeholders: Option<Vec<Range<Placeholder>>>,
        min_lines: usize,
        max_lines: usize,
        soft_wrap: bool,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        overflow: TextOverflow,
        auto_size: Option<Rc<dyn TextAutoSize>>,
    ) -> bool {
        let mut changed = !self.style.has_same_layout_affecting_attributes(&style);
        self.style = style;
        if self.placeholders != placeholders {
            self.placeholders = placeholders;
            changed = true;
        }
        if self.min_lines != min_lines {
            self.min_lines = min_lines;
            changed = true;
        }
        if self.max_lines != max_lines {
            self.max_lines = max_lines;
            changed = true;
        }
        if self.soft_wrap != soft_wrap {
            self.soft_wrap = soft_wrap;
            changed = true;
        }
        if !same_resolver(&self.font_family_resolver, &font_family_resolver) {
            self.font_family_resolver = font_family_resolver;
            changed = true;
        }
        if self.overflow != overflow {
            self.overflow = overflow;
            changed = true;
        }
        if !same_auto_size(&self.auto_size, &auto_size) {
            self.auto_size = auto_size;
            changed = true;
        }
        changed
    }

    /// Takes new callbacks; whether any is another one (Compose's updateCallbacks).
    fn update_callbacks(
        &mut self,
        on_text_layout: Option<OnTextLayout>,
        on_placeholder_layout: Option<OnPlaceholderLayout>,
        selection_controller: Option<Rc<SelectionController>>,
        on_show_translation: Option<OnShowTranslation>,
    ) -> bool {
        let mut changed = false;
        if !same_rc(&self.on_text_layout, &on_text_layout) {
            self.on_text_layout = on_text_layout;
            changed = true;
        }
        if !same_rc(&self.on_placeholder_layout, &on_placeholder_layout) {
            self.on_placeholder_layout = on_placeholder_layout;
            changed = true;
        }
        if !same_rc(&self.selection_controller, &selection_controller) {
            self.selection_controller = selection_controller;
            changed = true;
        }
        if !same_rc(&self.on_show_translation, &on_show_translation) {
            self.on_show_translation = on_show_translation;
            changed = true;
        }
        changed
    }

    /// Invalidates what the update changed (Compose's doInvalidations): the cache brought up to date (attached or
    /// not); a text change its semantics; a text, layout or callback change its measurement and drawing; a draw
    /// change its drawing (and its semantics, once asked for).
    pub(crate) fn do_invalidations(
        &mut self,
        draw_changed: bool,
        text_changed: bool,
        layout_changed: bool,
        callbacks_changed: bool,
    ) {
        if text_changed || layout_changed || callbacks_changed {
            let (text, style, resolver) = (self.text.clone(), self.style.clone(), self.font_family_resolver.clone());
            let (overflow, soft_wrap, max_lines, min_lines) =
                (self.overflow, self.soft_wrap, self.max_lines, self.min_lines);
            let (placeholders, auto_size) = (self.placeholders.clone(), self.auto_size.clone());
            self.layout_cache().update(
                text,
                style,
                resolver,
                overflow,
                soft_wrap,
                max_lines,
                min_lines,
                placeholders,
                auto_size,
            );
        }
        let Some(handle) = self.handle.clone().filter(NodeHandle::is_attached) else {
            return;
        };
        if text_changed || (draw_changed && self.semantics_applied) {
            handle.invalidate_semantics();
        }
        if text_changed || layout_changed || callbacks_changed {
            handle.invalidate_measurement();
            handle.invalidate_draw();
        }
        if draw_changed {
            handle.invalidate_draw();
        }
    }

    /// Shows `updated` instead of its text (Compose's setSubstitution: a plain text, its placeholders left out);
    /// whether that changed anything.
    fn set_substitution(&mut self, updated: AnnotatedString) -> bool {
        let (style, resolver) = (self.style.clone(), self.font_family_resolver.clone());
        let (overflow, soft_wrap, max_lines, min_lines) =
            (self.overflow, self.soft_wrap, self.max_lines, self.min_lines);
        let auto_size = self.auto_size.clone();
        if let Some(current) = &mut self.text_substitution {
            if updated == current.substitution {
                return false;
            }
            current.substitution = updated.clone();
            current.layout_cache.update(
                updated,
                style,
                resolver,
                overflow,
                soft_wrap,
                max_lines,
                min_lines,
                Some(Vec::new()),
                auto_size,
            );
        } else {
            let density = self.layout_cache().density();
            let mut cache = MultiParagraphLayoutCache::new(
                updated.clone(),
                style,
                resolver,
                overflow,
                soft_wrap,
                max_lines,
                min_lines,
                Some(Vec::new()),
                auto_size,
            );
            cache.set_density(density);
            self.text_substitution = Some(TextSubstitutionValue {
                original: self.text.clone(),
                substitution: updated,
                is_showing_substitution: false,
                layout_cache: cache,
            });
        }
        true
    }

    /// The text node of the chain is attached: it keeps its handle.
    pub(crate) fn attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    /// It is detached.
    pub(crate) fn detach(&mut self) {
        self.handle = None;
    }

    /// Whether its text has links (none while it shows a substitution, a plain text).
    fn has_links(&self) -> bool {
        if self.text_substitution.as_ref().is_some_and(|s| s.is_showing_substitution) {
            return false;
        }
        self.text.has_link_annotations(0, self.text.len())
    }
}

impl ModifierNode for TextAnnotatedStringNode {
    node_capabilities!(layout, draw, semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.attach(cx);
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.detach();
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutModifierNode for TextAnnotatedStringNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let direction = scope.layout_direction();
        let cache = self.active_layout_cache(scope.density());
        let did_change_layout = cache.layout_with_constraints(constraints, direction);
        let result = cache.text_layout_result().clone();
        // Measured again when an async font comes.
        result.multi_paragraph.intrinsics().has_stale_resolved_fonts();
        if did_change_layout {
            if let Some(h) = &self.handle {
                h.invalidate_draw();
            }
            if let Some(on_text_layout) = self.on_text_layout.clone() {
                on_text_layout(&result);
            }
            if let Some(controller) = self.selection_controller.clone() {
                controller.update_text_layout(&result);
            }
            self.baseline_cache = Some((round_to_int(result.first_baseline()), round_to_int(result.last_baseline())));
        }
        // The placeholders first, then what is inside measured in its box.
        if let Some(on_placeholder_layout) = self.on_placeholder_layout.clone() {
            on_placeholder_layout(result.placeholder_rects());
        }
        let size = result.size;
        let placeable =
            measurable.measure(Constraints::fit_prioritizing_width(size.width, size.width, size.height, size.height));
        let (first, last) = self.baseline_cache.unwrap_or_default();
        MeasureResult::new(
            size.width,
            size.height,
            vec![(FirstBaseline.into(), first), (LastBaseline.into(), last)],
            move |p| p.place(&placeable, 0, 0),
        )
    }

    fn min_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        let direction = scope.layout_direction();
        self.active_layout_cache(scope.density()).min_intrinsic_width(direction)
    }
    fn min_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        let direction = scope.layout_direction();
        self.active_layout_cache(scope.density()).intrinsic_height(width, direction)
    }
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        let direction = scope.layout_direction();
        self.active_layout_cache(scope.density()).max_intrinsic_width(direction)
    }
    fn max_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        let direction = scope.layout_direction();
        self.active_layout_cache(scope.density()).intrinsic_height(width, direction)
    }
}

impl DrawModifierNode for TextAnnotatedStringNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        if self.handle.is_none() {
            return;
        }
        if let Some(controller) = self.selection_controller.clone() {
            controller.draw(scope);
        }
        let paint = text_paint_style(&self.style, self.override_color.as_ref());
        let overflow = self.overflow;
        let result = self.active_layout_cache(scope.density()).text_layout_result().clone();
        let will_clip = result.has_visual_overflow() && overflow != TextOverflow::Visible;
        if will_clip {
            let (w, h) = (result.size.width as f32, result.size.height as f32);
            scope.clip_rect_with(0.0, 0.0, w, h, ClipOp::Intersect, |s| {
                s.paint_multi_paragraph(&result.multi_paragraph, &paint, DefaultBlendMode)
            });
        } else {
            scope.paint_multi_paragraph(&result.multi_paragraph, &paint, DefaultBlendMode);
        }
        // The inline content and the links' nodes.
        if self.has_links() || self.placeholders.as_ref().is_some_and(|p| !p.is_empty()) {
            scope.draw_content();
        }
    }
}

impl SemanticsModifierNode for TextAnnotatedStringNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        self.semantics_applied = true;
        config.set_text(self.text.clone());
        if let Some(s) = &self.text_substitution {
            config.set_text_substitution(s.substitution.clone());
            config.set_is_showing_text_substitution(s.is_showing_substitution);
        }
        let (handle, access) = (self.handle.clone(), self.access);
        let h = handle.clone();
        config.set_text_substitution_action(move |updated: AnnotatedString| {
            if with_node(&h, access, |n| n.set_substitution(updated)).is_none() {
                return false;
            }
            if let Some(cx) = h.as_ref().and_then(NodeHandle::cx) {
                invalidate_for_translate(&cx);
            }
            true
        });
        let h = handle.clone();
        config.show_text_substitution(move |show: bool| {
            let shown = with_node(&h, access, |n| {
                let Some(s) = &n.text_substitution else { return false };
                if let Some(on_show) = n.on_show_translation.clone() {
                    on_show(s);
                }
                if let Some(s) = &mut n.text_substitution {
                    s.is_showing_substitution = show;
                }
                true
            });
            if shown != Some(true) {
                return false;
            }
            if let Some(cx) = h.as_ref().and_then(NodeHandle::cx) {
                invalidate_for_translate(&cx);
            }
            true
        });
        let h = handle.clone();
        config.clear_text_substitution(move || {
            if with_node(&h, access, |n| n.text_substitution = None).is_none() {
                return false;
            }
            if let Some(cx) = h.as_ref().and_then(NodeHandle::cx) {
                invalidate_for_translate(&cx);
            }
            true
        });
        config.get_text_layout_result(move |out: &mut Vec<TextLayoutResult>| {
            let layout = with_node(&handle, access, |n| {
                let input_layout = n.layout_cache.as_ref()?.layout_or_none()?.clone();
                let color = n.override_color.as_ref().map_or(Color::Unspecified, ColorProducer::get);
                let input = &input_layout.layout_input;
                let layout_input = TextLayoutInput { style: with_color(&n.style, color), ..(**input).clone() };
                Some(input_layout.copy(Rc::new(layout_input), input_layout.size))
            })
            .flatten();
            match layout {
                Some(r) => {
                    out.push(r);
                    true
                }
                None => false,
            }
        });
    }
}
