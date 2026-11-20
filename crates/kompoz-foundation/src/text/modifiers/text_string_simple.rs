//! A plain string's text node (Compose's TextStringSimpleElement and TextStringSimpleNode): the fast path of
//! `BasicText(String)` without a layout callback, an auto size or a selection — a [`Paragraph`] laid out and kept,
//! drawn, its baselines given, described to accessibility (its layout made only when asked for).
//!
//! [`Paragraph`]: kompoz_text::Paragraph

use super::layout_utils::round_to_int;
use super::min_lines_constrainer::same_resolver;
use super::paragraph_layout_cache::ParagraphLayoutCache;
use super::text_style_provider::{StylePhase, inherited_text_style};
use super::{invalidate_for_translate, text_paint_style, with_color};
use crate::ComposeFoundationFlags;
use kompoz_graphics::{ColorProducer, Constraints, Density};
use kompoz_text::{AnnotatedString, FontFamilyResolver, TextLayoutResult, TextOverflow, TextStyle};
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

/// The element of a plain string's text (Compose's TextStringSimpleElement): equal when every argument is (the
/// resolver the same one, the color producer the same one).
pub(crate) struct TextStringSimpleElement {
    pub(crate) text: String,
    pub(crate) style: TextStyle,
    pub(crate) font_family_resolver: Rc<dyn FontFamilyResolver>,
    pub(crate) overflow: TextOverflow,
    pub(crate) soft_wrap: bool,
    pub(crate) max_lines: usize,
    pub(crate) min_lines: usize,
    pub(crate) color: Option<ColorProducer>,
}

impl PartialEq for TextStringSimpleElement {
    fn eq(&self, o: &TextStringSimpleElement) -> bool {
        self.color == o.color
            && self.text == o.text
            && self.style == o.style
            && same_resolver(&self.font_family_resolver, &o.font_family_resolver)
            && self.overflow == o.overflow
            && self.soft_wrap == o.soft_wrap
            && self.max_lines == o.max_lines
            && self.min_lines == o.min_lines
    }
}

impl ModifierNodeElement for TextStringSimpleElement {
    type Node = TextStringSimpleNode;

    fn create(&self) -> TextStringSimpleNode {
        TextStringSimpleNode {
            text: self.text.clone(),
            style: self.style.clone(),
            font_family_resolver: self.font_family_resolver.clone(),
            overflow: self.overflow,
            soft_wrap: self.soft_wrap,
            max_lines: self.max_lines,
            min_lines: self.min_lines,
            override_color: self.color.clone(),
            layout_cache: None,
            baseline_cache: None,
            text_substitution: None,
            semantics_applied: false,
            resolved_inherited_style: None,
            handle: None,
        }
    }

    fn update(&self, node: &mut TextStringSimpleNode) {
        let draw_changed = node.update_draw(self.color.clone(), &self.style);
        let text_changed = node.update_text(&self.text);
        let layout_changed = node.update_layout_related_args(
            self.style.clone(),
            self.min_lines,
            self.max_lines,
            self.soft_wrap,
            self.font_family_resolver.clone(),
            self.overflow,
        );
        node.do_invalidations(draw_changed, text_changed, layout_changed);
    }

    fn name(&self) -> &'static str {
        "textStringSimple"
    }
}

/// A text substitution of a plain string's (Compose's TextStringSimpleNode.TextSubstitutionValue): what it shows
/// instead of the text, whether it shows it, and its own layout.
pub(crate) struct TextSubstitutionValue {
    pub(crate) substitution: String,
    pub(crate) is_showing_substitution: bool,
    pub(crate) layout_cache: ParagraphLayoutCache,
}

/// The node of a plain string's text (Compose's TextStringSimpleNode): lays the text out (kept in a
/// [`ParagraphLayoutCache`]), draws it (clipped where it overflows), gives its baselines, says it to
/// accessibility with its substitution actions; it invalidates only what an update changes (layout, drawing or
/// semantics), as Compose's `doInvalidations`.
pub(crate) struct TextStringSimpleNode {
    text: String,
    style: TextStyle,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    overflow: TextOverflow,
    soft_wrap: bool,
    max_lines: usize,
    min_lines: usize,
    override_color: Option<ColorProducer>,
    layout_cache: Option<ParagraphLayoutCache>,
    /// Its first and last baselines, from the last layout that changed.
    baseline_cache: Option<(i32, i32)>,
    text_substitution: Option<TextSubstitutionValue>,
    /// Whether its semantics were asked for (Compose's semanticsTextLayoutResult set): a draw change then
    /// changes what they hand out.
    semantics_applied: bool,
    /// The text style the styles above give it, its own specified properties over them (Compose's
    /// `resolvedInheritedStyle`, with `ComposeFoundationFlags.isInheritedTextStyleEnabled`).
    resolved_inherited_style: Option<TextStyle>,
    handle: Option<NodeHandle>,
}

impl TextStringSimpleNode {
    /// Its layout cache, made when first needed (in the inherited style, where it is inherited).
    fn layout_cache(&mut self) -> &mut ParagraphLayoutCache {
        let style = match &self.resolved_inherited_style {
            Some(inherited) if ComposeFoundationFlags::is_inherited_text_style_enabled() => inherited,
            _ => &self.style,
        };
        self.layout_cache.get_or_insert_with(|| {
            ParagraphLayoutCache::new(
                self.text.clone(),
                style.clone(),
                self.font_family_resolver.clone(),
                self.overflow,
                self.soft_wrap,
                self.max_lines,
                self.min_lines,
            )
        })
    }

    /// The cache of what it shows (its substitution's while it shows one), laying out in `density` (Compose's
    /// getLayoutCacheForMeasure).
    fn layout_cache_for_measure(&mut self, density: Density) -> &mut ParagraphLayoutCache {
        if ComposeFoundationFlags::is_inherited_text_style_enabled() && self.resolve_inherited_style(StylePhase::Layout)
        {
            let style = self.resolved_inherited_style.clone().unwrap_or_else(|| self.style.clone());
            let (text, resolver) = (self.text.clone(), self.font_family_resolver.clone());
            let (overflow, soft_wrap, max_lines, min_lines) =
                (self.overflow, self.soft_wrap, self.max_lines, self.min_lines);
            self.layout_cache().update(text, style, resolver, overflow, soft_wrap, max_lines, min_lines);
        }
        self.layout_cache();
        let cache = self.active_layout_cache();
        cache.set_density(Some(density));
        cache
    }

    /// The inherited text style for `phase` worked out again (Compose's `resolveInheritedStyle`): whether it
    /// changed (not the first time).
    fn resolve_inherited_style(&mut self, phase: StylePhase) -> bool {
        let Some(cx) = self.handle.as_ref().and_then(NodeHandle::cx) else { return false };
        let previous = self.resolved_inherited_style.take();
        let new = inherited_text_style(&cx, phase, &self.style);
        let changed = previous.as_ref().is_some_and(|p| *p != new);
        self.resolved_inherited_style = Some(new);
        changed
    }

    /// The cache of what it shows: its substitution's while it shows one, else its own (Compose's
    /// getLayoutCache).
    fn active_layout_cache(&mut self) -> &mut ParagraphLayoutCache {
        self.layout_cache();
        let showing = self.text_substitution.as_ref().is_some_and(|s| s.is_showing_substitution);
        match &mut self.text_substitution {
            Some(s) if showing => &mut s.layout_cache,
            _ => self.layout_cache.as_mut().expect("kompoz: a layout cache"),
        }
    }

    /// Takes a new color and style; whether it draws otherwise (Compose's updateDraw).
    fn update_draw(&mut self, color: Option<ColorProducer>, style: &TextStyle) -> bool {
        let changed = color != self.override_color;
        self.override_color = color;
        changed || !style.has_same_draw_affecting_attributes(&self.style)
    }

    /// Takes a new text; whether it changed (Compose's updateText): a new one drops the substitution.
    fn update_text(&mut self, text: &str) -> bool {
        if self.text == text {
            return false;
        }
        self.text = text.to_string();
        self.text_substitution = None;
        true
    }

    /// Takes the arguments its layout depends on; whether any lays it out otherwise (Compose's
    /// updateLayoutRelatedArgs).
    fn update_layout_related_args(
        &mut self,
        style: TextStyle,
        min_lines: usize,
        max_lines: usize,
        soft_wrap: bool,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        overflow: TextOverflow,
    ) -> bool {
        let mut changed = !self.style.has_same_layout_affecting_attributes(&style);
        self.style = style;
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
        changed
    }

    /// Invalidates what the update changed (Compose's doInvalidations): the cache brought up to date (attached or
    /// not); a text change its semantics, measurement and drawing; a layout change its measurement and drawing; a
    /// draw change its drawing (and its semantics, which hand out its colors, once asked for).
    fn do_invalidations(&mut self, draw_changed: bool, text_changed: bool, layout_changed: bool) {
        if draw_changed || text_changed || layout_changed {
            self.resolved_inherited_style = None;
        }
        if text_changed || layout_changed {
            let (text, style, resolver) = (self.text.clone(), self.style.clone(), self.font_family_resolver.clone());
            let (overflow, soft_wrap, max_lines, min_lines) =
                (self.overflow, self.soft_wrap, self.max_lines, self.min_lines);
            self.layout_cache().update(text, style, resolver, overflow, soft_wrap, max_lines, min_lines);
        }
        let Some(handle) = self.handle.clone().filter(|h| h.is_attached()) else {
            return;
        };
        if text_changed || (draw_changed && self.semantics_applied) {
            handle.invalidate_semantics();
        }
        if text_changed || layout_changed {
            handle.invalidate_measurement();
            handle.invalidate_draw();
        }
        if draw_changed {
            handle.invalidate_draw();
        }
    }

    /// Shows `updated` instead of its text (Compose's setSubstitution); whether that changed anything.
    fn set_substitution(&mut self, updated: &str) -> bool {
        let (style, resolver) = (self.style.clone(), self.font_family_resolver.clone());
        let (overflow, soft_wrap, max_lines, min_lines) =
            (self.overflow, self.soft_wrap, self.max_lines, self.min_lines);
        if let Some(current) = &mut self.text_substitution {
            if updated == current.substitution {
                return false;
            }
            current.substitution = updated.to_string();
            current.layout_cache.update(
                updated.to_string(),
                style,
                resolver,
                overflow,
                soft_wrap,
                max_lines,
                min_lines,
            );
        } else {
            let density = self.layout_cache().density();
            let mut cache = ParagraphLayoutCache::new(
                updated.to_string(),
                style,
                resolver,
                overflow,
                soft_wrap,
                max_lines,
                min_lines,
            );
            cache.set_density(density);
            self.text_substitution = Some(TextSubstitutionValue {
                substitution: updated.to_string(),
                is_showing_substitution: false,
                layout_cache: cache,
            });
        }
        true
    }
}

impl ModifierNode for TextStringSimpleNode {
    node_capabilities!(layout, draw, semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.handle = None;
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutModifierNode for TextStringSimpleNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let direction = scope.layout_direction();
        let cache = self.layout_cache_for_measure(scope.density());
        let did_change_layout = cache.layout_with_constraints(constraints, direction);
        // Measured again when an async font comes.
        cache.observe_font_changes();
        let paragraph = cache.paragraph.clone().expect("kompoz: a paragraph laid out");
        let size = cache.layout_size;
        if did_change_layout {
            if let Some(h) = &self.handle {
                h.invalidate_draw();
            }
            self.baseline_cache =
                Some((round_to_int(paragraph.first_baseline()), round_to_int(paragraph.last_baseline())));
        }
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
        self.layout_cache_for_measure(scope.density()).min_intrinsic_width(direction)
    }
    fn min_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        let direction = scope.layout_direction();
        self.layout_cache_for_measure(scope.density()).intrinsic_height(width, direction)
    }
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        let direction = scope.layout_direction();
        self.layout_cache_for_measure(scope.density()).max_intrinsic_width(direction)
    }
    fn max_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        let direction = scope.layout_direction();
        self.layout_cache_for_measure(scope.density()).intrinsic_height(width, direction)
    }
}

impl DrawModifierNode for TextStringSimpleNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        if self.handle.is_none() {
            return;
        }
        let style = if ComposeFoundationFlags::is_inherited_text_style_enabled() {
            self.resolve_inherited_style(StylePhase::Draw);
            self.resolved_inherited_style.clone().unwrap_or_else(|| self.style.clone())
        } else {
            self.style.clone()
        };
        let paint = text_paint_style(&style, self.override_color.as_ref());
        let cache = self.active_layout_cache();
        let paragraph = cache.paragraph.clone().expect(
            "kompoz: Internal Error: ParagraphLayoutCache could not provide a Paragraph during the draw phase.",
        );
        let (will_clip, size) = (cache.did_overflow, cache.layout_size);
        if will_clip {
            scope.clip_rect_with(0.0, 0.0, size.width as f32, size.height as f32, ClipOp::Intersect, |s| {
                s.paint_paragraph(&paragraph, &paint, DefaultBlendMode)
            });
        } else {
            scope.paint_paragraph(&paragraph, &paint, DefaultBlendMode);
        }
    }
}

impl SemanticsModifierNode for TextStringSimpleNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        self.semantics_applied = true;
        config.set_text(AnnotatedString::new(self.text.as_str()));
        if let Some(s) = &self.text_substitution {
            config.set_is_showing_text_substitution(s.is_showing_substitution);
            config.set_text_substitution(AnnotatedString::new(s.substitution.as_str()));
        }
        let handle = self.handle.clone();
        let h = handle.clone();
        config.set_text_substitution_action(move |updated: AnnotatedString| {
            let Some(cx) = h.as_ref().and_then(NodeHandle::cx) else { return false };
            cx.with(|n: &mut TextStringSimpleNode| n.set_substitution(updated.text()));
            invalidate_for_translate(&cx);
            true
        });
        let h = handle.clone();
        config.show_text_substitution(move |show: bool| {
            let Some(cx) = h.as_ref().and_then(NodeHandle::cx) else { return false };
            let shown = cx.with(|n: &mut TextStringSimpleNode| match &mut n.text_substitution {
                Some(s) => {
                    s.is_showing_substitution = show;
                    true
                }
                None => false,
            });
            if shown != Some(true) {
                return false;
            }
            invalidate_for_translate(&cx);
            true
        });
        let h = handle.clone();
        config.clear_text_substitution(move || {
            let Some(cx) = h.as_ref().and_then(NodeHandle::cx) else { return false };
            cx.with(|n: &mut TextStringSimpleNode| n.text_substitution = None);
            invalidate_for_translate(&cx);
            true
        });
        config.get_text_layout_result(move |out: &mut Vec<TextLayoutResult>| {
            let Some(cx) = handle.as_ref().and_then(NodeHandle::cx) else { return false };
            let layout = cx
                .with(|n: &mut TextStringSimpleNode| {
                    let color = n.override_color.as_ref().map_or(kompoz_graphics::Color::Unspecified, |c| c.get());
                    let style = with_color(&n.style, color);
                    n.layout_cache().slow_create_text_layout_result_or_none(style)
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
