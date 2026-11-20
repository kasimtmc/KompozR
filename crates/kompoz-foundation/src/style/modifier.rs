//! `Modifier.styleable` (Compose's StyleModifier.kt): a [`Style`] applied to a layout — an outer node laying out
//! the external padding, size and position, drawing the shadows, background, foreground and border, placing what
//! it wraps with the layer properties; an inner node laying out the content padding and the border's width; the
//! text properties inherited by the text inside. The style is resolved again when it changes, its state changes,
//! or a state it read changes, and only the phases whose properties changed are invalidated; animated properties
//! are interpolated where they are read (measure, placement, layer, draw).
//!
//! Compose's outer node is found by its inner node and by the text inside through a traversal; the outer node's
//! part is shared here and found through a modifier local, so a node below finds it while the outer node is busy
//! (measuring or drawing what it wraps).

use super::properties::*;
use super::resolved::{BuildContext, ResolvedStyle};
use super::scope::StyleLocals;
use super::state::{MutableStyleState, StyleState};
use super::styles::{Style, combined_styles, same_style};
use crate::border_logic::BorderLogic;
use crate::interaction::InteractionSource;
use kompoz_coroutines::{CoroutineScope, Job};
use kompoz_graphics::{
    Brush, Color, Constraints, IntOffset, LayoutDirection, Offset, Outline, Rect, RectangleShape, Shape, Size,
    SolidColor,
};
use kompoz_runtime::named;
use kompoz_text::TextStyle;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, GraphicsLayer};
use kompoz_ui::layout::{
    GraphicsLayerScope, LayerBlock, LayoutModifierNode, Measurable, MeasureResult, MeasureScope, TransformOrigin,
};
use kompoz_ui::modifier::ObserverModifierNode;
use kompoz_ui::modifier::{ModifierLocal, ModifierLocalMap, ModifierLocalModifierNode};
use kompoz_ui::platform::LocalPlatform;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The styleable modifiers (Compose's `Modifier.styleable` overloads).
#[named(trait StyleModifiers)]
impl Modifier {
    /// `style` applied to the layout, in `style_state` (Compose's `styleable(styleState, style)`): its sizes,
    /// paddings and position laid out, its background, border, shadows and foreground drawn, its layer properties
    /// applied, its text properties inherited by the text inside; the state's pressed, hovered, focused… blocks
    /// applying as the state says. No state: a state of its own, which no interaction updates. The empty style adds
    /// nothing.
    ///
    /// Kotlin: `styleable(styleState: StyleState? = null, style: Style)`
    fn styleable(self, #[default(None)] style_state: Option<StyleState>, style: impl Style + 'static) -> Modifier {
        self.styleable_with(style_state, style)
    }

    /// [`styleable`](StyleModifiers::styleable) with every parameter, for plain code.
    fn styleable_with(self, style_state: Option<StyleState>, style: impl Style + 'static) -> Modifier {
        let style = style.into_shared();
        if style.is_empty_style() {
            return self;
        }
        self.then_element(StyleElement { style_state, style }).then_element(StyleInnerElement)
    }

    /// `styles` combined, each overriding the ones before it, applied as one (Compose's
    /// `styleable(styleState, vararg styles)`).
    ///
    /// Kotlin: `styleable(styleState: StyleState?, vararg styles: Style)`
    fn styleable_styles(self, style_state: Option<StyleState>, styles: &[Rc<dyn Style>]) -> Modifier {
        self.styleable_with(style_state, combined_styles(styles))
    }
}

// ---- The elements ---------------------------------------------------------------------------------------------

/// Compose's StyleElement: equal when the style and the state are.
struct StyleElement {
    style_state: Option<StyleState>,
    style: Rc<dyn Style>,
}

impl PartialEq for StyleElement {
    fn eq(&self, other: &Self) -> bool {
        same_style(&self.style, &other.style) && self.style_state == other.style_state
    }
}

impl ModifierNodeElement for StyleElement {
    type Node = StyleOuterNode;
    fn create(&self) -> StyleOuterNode {
        StyleOuterNode { outer: StyleOuter::new(self.style_state.clone(), self.style.clone(), None) }
    }
    fn update(&self, node: &mut StyleOuterNode) {
        node.outer.set_style(self.style.clone());
        node.outer.set_state(self.style_state.clone().unwrap_or_else(|| MutableStyleState::new(None).into()));
    }
    fn name(&self) -> &'static str {
        "style"
    }
}

/// Compose's StyleInnerElement: one, always equal to itself.
#[derive(PartialEq)]
struct StyleInnerElement;

impl ModifierNodeElement for StyleInnerElement {
    type Node = StyleInnerNode;
    fn create(&self) -> StyleInnerNode {
        StyleInnerNode { outer: None, handle: None }
    }
    fn update(&self, _: &mut StyleInnerNode) {}
    fn name(&self) -> &'static str {
        "styleInner"
    }
}

/// How the outer node's part is found by the nodes below it: its inner node, the text inside, and the outer nodes
/// inside (for the inherited text properties).
pub(crate) static LocalStyleOuter: ModifierLocal<Option<Weak<StyleOuter>>> = ModifierLocal::new(|| None);

// ---- The outer node -------------------------------------------------------------------------------------------

/// The outer node of `styleable` (Compose's StyleOuterNode), its part shared with the nodes below.
pub(crate) struct StyleOuterNode {
    outer: Rc<StyleOuter>,
}

/// What the outer node is (Compose's StyleOuterNode's state and logic): kept apart from the node, as the inner node
/// and the text inside call it while the node is busy.
pub(crate) struct StyleOuter {
    this: Weak<StyleOuter>,
    handle: RefCell<Option<NodeHandle>>,
    inner_handle: RefCell<Option<NodeHandle>>,
    provided: ModifierLocalMap,
    style: RefCell<Rc<dyn Style>>,
    state: RefCell<StyleState>,
    /// Where its animations run instead of its node's scope (Compose's `overrideScope`, tests').
    override_scope: Option<CoroutineScope>,
    builder: ResolvedStyle,
    /// The properties resolved (Compose's `_resolved`), and the other buffer (`_bufferOrNull`).
    resolved: RefCell<Rc<StyleProperties>>,
    buffer: RefCell<Option<Rc<StyleProperties>>>,
    border_layer: RefCell<Option<GraphicsLayer>>,
    border_logic: RefCell<BorderLogic>,
    current_interaction_source: RefCell<Option<InteractionSource>>,
    source_job: RefCell<Option<Job>>,
    /// The layer block (Compose's `layerBlock`): a new one when the layer properties change, so the layer is
    /// updated (Compose's `updateLayerBlock`).
    layer_block: RefCell<Option<LayerBlock>>,
    /// The outline drawn last, for its size, direction and shape (Compose's outline cache).
    outline_cache: RefCell<Option<(Size, LayoutDirection, Arc<dyn Shape>, Outline)>>,
    /// The inherited text properties worked out (Compose's `cachedInheritedStyle`), and whether they are stale.
    cached_inherited_style: RefCell<Option<StyleProperties>>,
    inherited_style_dirty: Cell<bool>,
}

impl StyleOuter {
    fn new(state: Option<StyleState>, style: Rc<dyn Style>, override_scope: Option<CoroutineScope>) -> Rc<StyleOuter> {
        Rc::new_cyclic(|this: &Weak<StyleOuter>| StyleOuter {
            this: this.clone(),
            handle: RefCell::new(None),
            inner_handle: RefCell::new(None),
            provided: ModifierLocalMap::of(&LocalStyleOuter, Some(this.clone())),
            style: RefCell::new(style),
            state: RefCell::new(state.unwrap_or_else(|| MutableStyleState::new(None).into())),
            override_scope,
            builder: ResolvedStyle::default(),
            resolved: RefCell::new(Rc::new(StyleProperties::default())),
            buffer: RefCell::new(None),
            border_layer: RefCell::new(None),
            border_logic: RefCell::new(BorderLogic::default()),
            current_interaction_source: RefCell::new(None),
            source_job: RefCell::new(None),
            layer_block: RefCell::new(None),
            outline_cache: RefCell::new(None),
            cached_inherited_style: RefCell::new(None),
            inherited_style_dirty: Cell::new(false),
        })
    }

    fn rc(&self) -> Rc<StyleOuter> {
        self.this.upgrade().expect("kompoz: a style node's part dropped")
    }

    fn cx(&self) -> Option<NodeCx> {
        self.handle.borrow().as_ref().and_then(NodeHandle::cx)
    }

    /// A new style: resolved again (Compose's `style` setter; set only when unequal).
    fn set_style(&self, style: Rc<dyn Style>) {
        *self.style.borrow_mut() = style;
        self.resolve_style_and_invalidate(false);
    }

    /// Another state: resolved again, the layer drawn again (Compose's `state` setter).
    fn set_state(&self, state: StyleState) {
        if *self.state.borrow() != state {
            *self.state.borrow_mut() = state;
            self.resolve_style_and_invalidate(false);
            self.invalidate_inner_layer();
        }
    }

    fn invalidate_inner_layer(&self) {
        let inner = self.inner_handle.borrow().clone();
        if let Some(h) = inner {
            h.invalidate_draw();
        }
    }

    /// The properties of `flags`' phases with their animations as they are now, if any animates; else the
    /// resolved ones (Compose's `resolveAnimatedStyleFor`).
    pub(crate) fn resolve_animated_style_for(&self, flags: u32) -> Rc<StyleProperties> {
        if self.builder.animating_flags() & flags != 0 {
            let mut properties = StyleProperties::default();
            self.builder.resolve_into(flags, &mut properties);
            Rc::new(properties)
        } else {
            self.resolved.borrow().clone()
        }
    }

    /// The same into `target`, only if any animates (Compose's `resolveAnimatedStyleForInto`).
    fn resolve_animated_style_for_into(&self, flags: u32, target: &mut StyleProperties) {
        if self.builder.animating_flags() & flags != 0 {
            self.builder.resolve_into(flags, target);
        }
    }

    fn current_layer_style(&self) -> Rc<StyleProperties> {
        self.resolve_animated_style_for(LayerFlag)
    }

    fn current_layout_style(&self) -> Rc<StyleProperties> {
        self.resolve_animated_style_for(OuterLayoutFlag | LayerFlag)
    }

    /// The layer block, made if there is none (Compose's `layerBlockNonNull`).
    fn layer_block(&self) -> LayerBlock {
        if let Some(b) = self.layer_block.borrow().as_ref() {
            return b.clone();
        }
        let block = self.new_layer_block();
        *self.layer_block.borrow_mut() = Some(block.clone());
        block
    }

    fn new_layer_block(&self) -> LayerBlock {
        let weak = self.this.clone();
        Rc::new(move |scope: &mut GraphicsLayerScope| {
            if let Some(outer) = weak.upgrade() {
                outer.update_layer(scope);
            }
        })
    }

    /// The layer's properties (Compose's `updateLayer`).
    fn update_layer(&self, scope: &mut GraphicsLayerScope) {
        let r = self.current_layer_style();
        scope.alpha = if r.has(AlphaId) { r.alpha } else { 1.0 };
        scope.scale_x = if r.has(ScaleXId) { r.scale_x } else { 1.0 };
        scope.scale_y = if r.has(ScaleYId) { r.scale_y } else { 1.0 };
        scope.translation_x = if r.has(TranslationXId) { r.translation_x } else { 0.0 };
        scope.translation_y = if r.has(TranslationYId) { r.translation_y } else { 0.0 };
        scope.rotation_x = if r.has(RotationXId) { r.rotation_x } else { 0.0 };
        scope.rotation_y = if r.has(RotationYId) { r.rotation_y } else { 0.0 };
        scope.rotation_z = if r.has(RotationZId) { r.rotation_z } else { 0.0 };
        scope.color_filter = if r.has_obj(ColorFilterId) { r.color_filter } else { None };
        let center = TransformOrigin::Center;
        scope.transform_origin = if r.has(TransformOriginXId) || r.has(TransformOriginYId) {
            TransformOrigin::new(
                if r.has(TransformOriginXId) { r.transform_origin_x } else { center.pivot_fraction_x },
                if r.has(TransformOriginYId) { r.transform_origin_y } else { center.pivot_fraction_y },
            )
        } else {
            center
        };
        scope.clip = r.has(ClipId) && r.clip;
        scope.shape = if r.has_obj(ShapeId) { r.shape.clone() } else { Arc::new(RectangleShape) };
    }

    /// Resolves the style again, in its own observation, and invalidates the phases whose properties changed
    /// (Compose's `resolveStyleAndInvalidate`); `initial`: the first resolution, at attach, which invalidates
    /// nothing.
    pub(crate) fn resolve_style_and_invalidate(&self, initial: bool) {
        let Some(cx) = self.cx().filter(NodeCx::is_attached) else { return };
        let prev: Option<Rc<StyleProperties>> = if initial { None } else { Some(self.resolved.borrow().clone()) };
        let density = cx.density();
        let scope = self.override_scope.clone().or_else(|| Some(cx.coroutine_scope()));
        let build = BuildContext {
            state: self.state.borrow().clone(),
            locals: StyleLocals::of(Some(cx.handle())),
            density,
            animation_scope: scope,
        };
        self.builder.prepare_build();
        let style = self.style.borrow().clone();
        let anim_changes = cx.observe_reads(|| {
            self.builder.build(&*style, &build);
            // (flags 0: no animation read)
            let mut next =
                if initial { None } else { self.buffer.borrow_mut().take().and_then(|b| Rc::try_unwrap(b).ok()) }
                    .unwrap_or_default();
            self.builder.resolve_into(0, &mut next);
            let old = std::mem::replace(&mut *self.resolved.borrow_mut(), Rc::new(next));
            if !initial {
                *self.buffer.borrow_mut() = Some(old);
            }
            self.builder.animating_flags()
        });
        let next = self.resolved.borrow().clone();
        let changes = anim_changes
            | match &prev {
                Some(prev) => prev.diff(&next, PhaseFlagMask),
                None => next.phase_flags(),
            };
        let source = self.state.borrow().interaction_source();
        if source != *self.current_interaction_source.borrow() {
            self.update_interaction_sources(&cx);
        }
        if initial {
            return;
        }
        if changes & InnerLayoutFlag != 0 {
            let inner = self.inner_handle.borrow().clone();
            if let Some(h) = inner {
                h.invalidate_measurement();
            }
        }
        if changes & OuterLayoutFlag != 0 {
            cx.invalidate_measurement();
        }
        if changes & DrawFlag != 0 {
            cx.invalidate_draw();
            self.invalidate_inner_layer();
        }
        if changes & LayerFlag != 0 {
            // (Compose's `updateLayerBlock`: the layer's properties worked out again)
            *self.layer_block.borrow_mut() = Some(self.new_layer_block());
            cx.invalidate_placement();
        }
        if changes & TextLayoutFlag != 0 {
            self.inherited_style_dirty.set(true);
            if let Some(n) = cx.layout_node() {
                n.invalidate_measurement_for_subtree();
            }
        }
        if changes & TextDrawFlag != 0 {
            self.inherited_style_dirty.set(true);
            if let Some(n) = cx.layout_node() {
                n.invalidate_draw_for_subtree();
            }
        }
    }

    /// Follows the state's interaction source (Compose's `updateInteractionSources`): its interactions processed
    /// in the node's scope.
    fn update_interaction_sources(&self, cx: &NodeCx) {
        let old = self.source_job.borrow_mut().take();
        if let Some(job) = old {
            job.cancel();
        }
        let state = self.state.borrow().clone();
        let source = state.interaction_source();
        *self.current_interaction_source.borrow_mut() = source.clone();
        if let Some(source) = source {
            let mutable = MutableStyleState::of(state);
            let job = cx.coroutine_scope().launch(mutable.process_interactions(source));
            *self.source_job.borrow_mut() = Some(job);
        }
    }

    // ---- Measuring ----

    fn measure(&self, scope: &MeasureScope, measurable: &dyn Measurable, constraints: Constraints) -> MeasureResult {
        let resolved = self.current_layout_style();
        let or_zero = |id: PrimitiveId, v: f32| if resolved.has(id) { v } else { 0.0 };
        let start = add_if_specified(
            or_zero(ExternalPaddingStartId, resolved.external_padding_start),
            or_zero(LeftId, resolved.left),
        );
        let end = add_if_specified(
            or_zero(ExternalPaddingEndId, resolved.external_padding_end),
            or_zero(RightId, resolved.right),
        );
        let top = add_if_specified(
            or_zero(ExternalPaddingTopId, resolved.external_padding_top),
            or_zero(TopId, resolved.top),
        );
        let bottom = add_if_specified(
            or_zero(ExternalPaddingBottomId, resolved.external_padding_bottom),
            or_zero(BottomId, resolved.bottom),
        );
        let horizontal = round(start + end);
        let vertical = round(top + bottom);

        let mut min_width = (constraints.min_width - horizontal).max(0);
        let mut max_width = add_max_with_minimum(constraints.max_width, horizontal);
        let mut min_height = (constraints.min_height - vertical).max(0);
        let mut max_height = add_max_with_minimum(constraints.max_height, vertical);

        // (the style's width constraints, then the incoming ones adjusted by them)
        let mut style_min_width = 0;
        let mut style_max_width = Constraints::Infinity;
        if resolved.has(MaxWidthId) {
            style_max_width = round(resolved.max_width).max(0);
        }
        if resolved.has(MinWidthId) {
            style_min_width = coerce_in(round(resolved.min_width), 0, style_max_width);
        }
        if resolved.has(WidthId) {
            style_min_width = coerce_in(round(resolved.width), style_min_width, style_max_width);
            style_max_width = style_min_width;
        }
        min_width = if style_min_width == 0 { min_width } else { coerce_in(style_min_width, min_width, max_width) };
        max_width = if style_max_width == Constraints::Infinity {
            max_width
        } else {
            coerce_in(style_max_width, min_width, max_width)
        };
        if !resolved.has(WidthId) {
            if resolved.has(WidthFractionId) && constraints.has_bounded_width() {
                let width = coerce_in(round(max_width as f32 * resolved.width_fraction), min_width, max_width);
                min_width = width;
                max_width = width;
            } else if resolved.has(LeftId) && resolved.has(RightId) {
                min_width = max_width;
            }
        }

        let mut style_min_height = 0;
        let mut style_max_height = Constraints::Infinity;
        if resolved.has(MaxHeightId) {
            style_max_height = round(resolved.max_height).max(0);
        }
        if resolved.has(MinHeightId) {
            style_min_height = coerce_in(round(resolved.min_height), 0, style_max_height);
        }
        if resolved.has(HeightId) {
            style_min_height = coerce_in(round(resolved.height), style_min_height, style_max_height);
            style_max_height = style_min_height;
        }
        min_height =
            if style_min_height == 0 { min_height } else { coerce_in(style_min_height, min_height, max_height) };
        max_height = if style_max_height == Constraints::Infinity {
            max_height
        } else {
            coerce_in(style_max_height, min_height, max_height)
        };
        if !resolved.has(HeightId) {
            if resolved.has(HeightFractionId) && constraints.has_bounded_height() {
                let height = coerce_in(round(max_height as f32 * resolved.height_fraction), min_height, max_height);
                min_height = height;
                max_height = height;
            } else if resolved.has(TopId) && resolved.has(BottomId) {
                min_height = max_height;
            }
        }

        let placeable = measurable.measure(Constraints::new(min_width, max_width, min_height, max_height));
        let (pw, ph) = (placeable.width(), placeable.height());
        let weak = self.this.clone();
        scope.layout(pw + horizontal, ph + vertical, move |p| {
            let Some(outer) = weak.upgrade() else { return };
            let style = outer.current_layout_style();
            let x = if !style.has(LeftId) && style.has(RightId) {
                constraints.max_width - pw - round(end)
            } else {
                round(start)
            };
            let y = if style.has(BottomId) && !style.has(TopId) {
                constraints.max_height - ph - round(bottom)
            } else {
                round(top)
            };
            // (Compose leaves the z index to do)
            if style.phase_flags() & LayerFlag != 0 {
                p.place_with_layer_block(&placeable, IntOffset::new(x, y), outer.layer_block(), 0.0);
            } else {
                p.place(&placeable, x, y);
            }
        })
    }

    // ---- Drawing ----

    fn outline(&self, scope: &ContentDrawScope, size: Size, shape: &Arc<dyn Shape>) -> Outline {
        let direction = scope.layout_direction();
        let mut cache = self.outline_cache.borrow_mut();
        if let Some((s, d, sh, o)) = cache.as_ref()
            && *s == size
            && *d == direction
            && **sh == **shape
        {
            return o.clone();
        }
        let outline = shape.create_outline(size, direction, &scope.density());
        *cache = Some((size, direction, shape.clone(), outline.clone()));
        outline
    }

    fn draw(&self, scope: &mut ContentDrawScope) {
        let r = self.resolve_animated_style_for(DrawFlag);
        let bg_color = if r.has(BackgroundColorId) { r.background_color } else { Color::Unspecified };
        let bg_brush = if r.has_obj(BackgroundBrushId) { r.background_brush.clone() } else { None };
        let fg_color = if r.has(ForegroundColorId) { r.foreground_color } else { Color::Unspecified };
        let fg_brush = if r.has_obj(ForegroundBrushId) { r.foreground_brush.clone() } else { None };
        let border_color = if r.has(BorderColorId) { r.border_color } else { Color::Black };
        let border_brush = if r.has_obj(BorderBrushId) { r.border_brush.clone() } else { None };
        let border_width = if r.has(BorderWidthId) { r.border_width } else { 0.0 };
        let shape = r.shape.clone();
        let has_border = border_width / 2.0 > 0.0;
        let has_background = bg_color.is_specified() || bg_brush.is_some();
        let has_foreground = fg_color.is_specified() || fg_brush.is_some();

        draw_shadows(scope, &r, r.drop_shadow.as_ref(), DropShadowId, false);
        let outline = self.outline(scope, scope.size(), &shape);
        if has_background {
            match bg_brush {
                Some(brush) => scope.draw_outline(&outline, brush),
                None => scope.draw_outline(&outline, bg_color),
            }
        }
        scope.draw_content();
        if has_foreground {
            match fg_brush {
                Some(brush) => scope.draw_outline(&outline, brush),
                None => scope.draw_outline(&outline, fg_color),
            }
        }
        if has_border {
            let brush: Brush = border_brush.unwrap_or_else(|| SolidColor(border_color));
            let platform = self.cx().map(|c| c.current_value_of(&LocalPlatform));
            let mut provider = || {
                let mut layer = self.border_layer.borrow_mut();
                layer.get_or_insert_with(|| GraphicsLayer::new(platform.clone())).clone()
            };
            self.border_logic.borrow_mut().draw_border(
                scope,
                border_width,
                &brush,
                &mut provider,
                &outline,
                Offset::Zero,
            );
        }
        draw_shadows(scope, &r, r.inner_shadow.as_ref(), InnerShadowId, true);
    }

    // ---- Inherited text properties ----

    /// The text properties inherited here (Compose's `resolveInheritedStyle`): those of the outer nodes above and
    /// this one, the nearer overriding the farther, their caches brought up to date; then those animating as they
    /// are now. None where none sets one.
    pub(crate) fn resolve_inherited_style(&self, flags: u32) -> Option<StyleProperties> {
        let this = self.rc();
        let mut nodes: Vec<Rc<StyleOuter>> = Vec::new();
        if this.resolved.borrow().phase_flags() & InheritedFlags != 0
            || this.builder.animating_flags() & InheritedFlags != 0
        {
            nodes.push(this.clone());
        }
        let mut up = this.ancestor();
        while let Some(node) = up {
            if node.resolved.borrow().phase_flags() & InheritedFlags != 0
                || node.builder.animating_flags() & InheritedFlags != 0
            {
                nodes.push(node.clone());
            }
            up = node.ancestor();
        }

        let mut start_style = this.cached_inherited_style();
        let mut start_index: isize = if start_style.is_some() { -1 } else { -2 };
        let mut has_animations = this.builder.animating_flags() & InheritedFlags != 0;
        let mut has_inherited_styles = this.resolved.borrow().phase_flags() & InheritedFlags;
        for (index, node) in nodes.iter().enumerate() {
            let cached = node.cached_inherited_style();
            has_animations = has_animations || node.builder.animating_flags() & InheritedFlags != 0;
            has_inherited_styles |= node.resolved.borrow().phase_flags() & InheritedFlags;
            match cached {
                None => {
                    start_style = None;
                    start_index = -2;
                }
                Some(c) if start_style.is_none() => {
                    start_style = Some(c);
                    start_index = index as isize;
                }
                Some(_) => {}
            }
        }
        if has_inherited_styles == 0 {
            return None;
        }
        if start_style.is_some() && start_index < 0 && !has_animations {
            return start_style;
        }

        // (the caches of the inherited properties, the farthest first)
        let mut ancestor_style = start_style;
        let start_index = if start_index < -1 { nodes.len() as isize - 1 } else { start_index };
        let mut index = start_index;
        while index >= -1 {
            let node = if index < 0 { &this } else { &nodes[index as usize] };
            let mut cached = node.cached_inherited_style.borrow_mut().take().unwrap_or_default();
            if let Some(a) = &ancestor_style {
                a.copy_into_flags(&mut cached, InheritedFlags);
            }
            node.resolved.borrow().copy_into_flags(&mut cached, InheritedFlags);
            node.save_inherited_styles(cached.clone());
            ancestor_style = Some(cached);
            index -= 1;
        }

        if has_animations {
            let mut animating = StyleProperties::default();
            if let Some(a) = &ancestor_style {
                a.copy_into_flags(&mut animating, InheritedFlags);
            }
            let mut index = nodes.len() as isize - 1;
            while index >= -1 {
                let node = if index < 0 { &this } else { &nodes[index as usize] };
                node.resolve_animated_style_for_into(flags, &mut animating);
                index -= 1;
            }
            ancestor_style = Some(animating);
        }
        ancestor_style
    }

    /// The nearest outer node above this one (Compose's traversal of the outer nodes' key).
    fn ancestor(&self) -> Option<Rc<StyleOuter>> {
        self.cx().and_then(|cx| cx.modifier_local_current(&LocalStyleOuter)).and_then(|w| w.upgrade())
    }

    fn cached_inherited_style(&self) -> Option<StyleProperties> {
        if self.inherited_style_dirty.get() { None } else { self.cached_inherited_style.borrow().clone() }
    }

    fn save_inherited_styles(&self, style: StyleProperties) {
        self.inherited_style_dirty.set(false);
        *self.cached_inherited_style.borrow_mut() = Some(style);
    }

    /// The text style inherited here for `phase`, `fallback`'s specified properties overriding it (Compose's
    /// `computeInheritedTextStyle`).
    pub(crate) fn compute_inherited_text_style(&self, flags: u32, fallback: &TextStyle) -> TextStyle {
        match self.resolve_inherited_style(flags) {
            Some(style) => to_text_style(&style, fallback),
            None => fallback.clone(),
        }
    }
}

/// Draws the drop (`inner` false) or inner shadows in the shape (Compose's `drawDropShadow`/`drawInnerShadow`).
fn draw_shadows(
    scope: &mut ContentDrawScope,
    r: &StyleProperties,
    value: Option<&ShadowValue>,
    id: ObjectId,
    inner: bool,
) {
    if !r.has_obj(id) {
        return;
    }
    let Some(value) = value else { return };
    let shape: Arc<dyn Shape> = if r.has_obj(ShapeId) { r.shape.clone() } else { Arc::new(RectangleShape) };
    let rect = Rect::from_offset_size(scope.origin(), scope.size());
    let (direction, density) = (scope.layout_direction(), scope.density());
    match value {
        ShadowValue::One(shadow) => {
            scope.context().draw_shape_shadow(rect, &*shape, direction, &density, shadow, inner);
        }
        ShadowValue::Many(shadows) => {
            for shadow in shadows.iter().flatten() {
                scope.context().draw_shape_shadow(rect, &*shape, direction, &density, shadow, inner);
            }
        }
    }
}

impl ModifierNode for StyleOuterNode {
    node_capabilities!(layout, draw, observer, modifier_local);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.outer.handle.borrow_mut() = Some(cx.handle());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        let layer = self.outer.border_layer.borrow_mut().take();
        if let Some(layer) = layer {
            layer.release();
        }
        let job = self.outer.source_job.borrow_mut().take();
        if let Some(job) = job {
            job.cancel();
        }
        *self.outer.current_interaction_source.borrow_mut() = None;
    }

    /// Compose's `shouldAutoInvalidate = false`: a resolution invalidates what changed.
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl ModifierLocalModifierNode for StyleOuterNode {
    fn provided_values(&self) -> &ModifierLocalMap {
        &self.outer.provided
    }
}

impl LayoutModifierNode for StyleOuterNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let outer = self.outer.clone();
        outer.measure(scope, measurable, constraints)
    }
}

impl DrawModifierNode for StyleOuterNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let outer = self.outer.clone();
        outer.draw(scope);
    }
}

impl ObserverModifierNode for StyleOuterNode {
    /// A state the style read changed: resolved again (Compose's `onObservedReadsChanged`).
    fn on_observed_reads_changed(&mut self) {
        let outer = self.outer.clone();
        outer.resolve_style_and_invalidate(false);
    }
}

// ---- The inner node -------------------------------------------------------------------------------------------

/// The inner node of `styleable` (Compose's StyleInnerNode): the content padding and the border's width.
pub(crate) struct StyleInnerNode {
    outer: Option<Rc<StyleOuter>>,
    handle: Option<NodeHandle>,
}

impl ModifierNode for StyleInnerNode {
    node_capabilities!(layout);

    /// Finds its outer node (attached before it, above it in the chain), and the style is first resolved
    /// (Compose's `onAttach`).
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        let outer = cx
            .modifier_local_current(&LocalStyleOuter)
            .and_then(|w| w.upgrade())
            .expect("kompoz: a styleable's inner node without its outer node");
        *outer.inner_handle.borrow_mut() = Some(cx.handle());
        self.outer = Some(outer.clone());
        outer.resolve_style_and_invalidate(true);
    }

    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutModifierNode for StyleInnerNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let outer = self.outer.clone().expect("kompoz: a styleable's inner node measured before it was attached");
        let r = outer.resolve_animated_style_for(InnerLayoutFlag);
        let or_zero = |id: PrimitiveId, v: f32| if r.has(id) { v } else { 0.0 };
        let border_width = or_zero(BorderWidthId, r.border_width);
        let start = or_zero(ContentPaddingStartId, r.content_padding_start) + border_width;
        let end = or_zero(ContentPaddingEndId, r.content_padding_end) + border_width;
        let top = or_zero(ContentPaddingTopId, r.content_padding_top) + border_width;
        let bottom = or_zero(ContentPaddingBottomId, r.content_padding_bottom) + border_width;
        let horizontal = round(start + end);
        let vertical = round(top + bottom);
        let placeable = measurable.measure(constraints.offset(-horizontal, -vertical));
        let width = constraints.constrain_width(placeable.width() + horizontal);
        let height = constraints.constrain_height(placeable.height() + vertical);
        let (x, y) = (round(start), round(top));
        scope.layout(width, height, move |p| p.place(&placeable, x, y))
    }
}

// ---- Helpers --------------------------------------------------------------------------------------------------

/// Kotlin's `fastRoundToInt`: the nearest, a half up.
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// Kotlin's `fastCoerceIn` (no check of `min <= max`: below `min` is `min`).
fn coerce_in(v: i32, min: i32, max: i32) -> i32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Compose's `addIfSpecified`: a NaN left out.
fn add_if_specified(v: f32, abs: f32) -> f32 {
    if abs.is_nan() { v } else { v + abs }
}

/// Compose's `addMaxWithMinimum`.
fn add_max_with_minimum(max: i32, value: i32) -> i32 {
    if max == Constraints::Infinity { max } else { (max + value).max(0) }
}

/// The text style of `p` under `supplied`'s specified properties (Compose's `toTextStyle`): what the text
/// supplies wins, the style's fill in; a supplied color, else a supplied brush, else the style's brush paints it.
pub(crate) fn to_text_style(p: &StyleProperties, supplied: &TextStyle) -> TextStyle {
    fn or<T: Clone>(supplied: &Option<T>, set: bool, value: impl FnOnce() -> Option<T>) -> Option<T> {
        match supplied {
            Some(v) => Some(v.clone()),
            None if set => value(),
            None => None,
        }
    }
    fn unit(
        supplied: kompoz_graphics::TextUnit,
        set: bool,
        value: kompoz_graphics::TextUnit,
    ) -> kompoz_graphics::TextUnit {
        if supplied.is_specified() || !set { supplied } else { value }
    }
    let mut style = TextStyle {
        color: if supplied.color.is_specified() || !p.has(ContentColorId) { supplied.color } else { p.content_color },
        font_size: unit(supplied.font_size, p.has(FontSizeId), p.font_size),
        font_weight: or(&supplied.font_weight, p.has(FontWeightId), || Some(p.font_weight())),
        font_style: or(&supplied.font_style, p.has(FontStyleId), || Some(p.font_style())),
        font_synthesis: or(&supplied.font_synthesis, p.has(FontSynthesisId), || Some(p.font_synthesis())),
        font_family: or(&supplied.font_family, p.has_obj(FontFamilyId), || p.font_family.clone()),
        letter_spacing: unit(supplied.letter_spacing, p.has(LetterSpacingId), p.letter_spacing),
        baseline_shift: or(&supplied.baseline_shift, p.has(BaselineShiftId), || Some(p.baseline_shift)),
        text_decoration: or(&supplied.text_decoration, p.has(TextDecorationId), || Some(p.text_decoration())),
        text_align: if supplied.text_align.is_specified() || !p.has(TextAlignId) {
            supplied.text_align
        } else {
            p.text_align()
        },
        text_direction: if supplied.text_direction.is_specified() || !p.has(TextDirectionId) {
            supplied.text_direction
        } else {
            p.text_direction()
        },
        line_height: unit(supplied.line_height, p.has(LineHeightId), p.line_height),
        text_indent: or(&supplied.text_indent, p.has_obj(TextIndentId), || p.text_indent),
        line_break: if supplied.line_break.is_specified() || !p.has(LineBreakId) {
            supplied.line_break
        } else {
            p.line_break
        },
        hyphens: if supplied.hyphens.is_specified() || !p.has(HyphensId) { supplied.hyphens } else { p.hyphens() },
        text_motion: or(&supplied.text_motion, p.has_obj(TextMotionId), || Some(p.text_motion)),
        ..supplied.clone()
    };
    if supplied.color.is_specified() {
        style.brush = None;
    } else if supplied.brush.is_some() {
        style.brush = supplied.brush.clone();
        style.color = Color::Unspecified;
    } else if p.has_obj(ContentBrushId) {
        style.brush = p.content_brush.clone();
        style.color = Color::Unspecified;
    } else {
        style.brush = None;
    }
    style
}
