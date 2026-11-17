//! Drawing through a graphics layer (C++'s `drawThroughLayer`): its matrix as Android's RenderNode makes it, its
//! elevation shadow, its clip to its outline, its alpha, blend mode, color filter, render effect and stretch,
//! offscreen only where it has to be; and what a blur of the node's own drawing needs to draw a node in two parts.

use crate::graphics_context::{
    ClipOp, CornerRadii, DefaultShadowColor, ElevationShadow, GraphicsContext, LayerTransform, TextDraw,
};
use crate::layout::{CompositingStrategy, GraphicsLayerScope, LayerGeometry, layer_matrix, mul, rotation};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, ColorMatrix, FilterQuality, ImageBitmap, Matrix, Offset, Outline, Path,
    PathDirection, PathStroke, Rect, RenderEffect, Size, StretchEffect, Vertices,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a layer is drawn through, everything in px in what it is drawn in: its bounds, its geometry (the pivot
/// where it is), how faded, how high (its outline's shadow, in its colors), whether it is cut to its outline (from
/// the bounds' top left), through what it is composited (its stretch last, over its bounds), and how far past its
/// bounds an offscreen layer reaches (not a stretched one: as hwui's layer, it is its bounds).
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct LayerDraw {
    pub bounds: Rect,
    pub geometry: LayerGeometry,
    pub alpha: f32,
    pub shadow_elevation: f32,
    pub ambient_shadow_color: Color,
    pub spot_shadow_color: Color,
    pub clip: bool,
    pub outline: Outline,
    pub blend_mode: BlendMode,
    pub color_filter: Option<ColorFilter>,
    pub render_effect: Option<RenderEffect>,
    /// Android's stretch overscroll (a graphics layer's, as foundation's overscroll sets it).
    pub stretch: StretchEffect,
    pub strategy: CompositingStrategy,
    /// Left, top, right, bottom.
    pub outsets: [f32; 4],
}

impl LayerDraw {
    /// A layer over `bounds` that does nothing: drawn straight through.
    pub fn plain(bounds: Rect) -> LayerDraw {
        let center = bounds.center();
        LayerDraw {
            bounds,
            geometry: LayerGeometry { pivot_x: center.x, pivot_y: center.y, ..LayerGeometry::default() },
            alpha: 1.0,
            shadow_elevation: 0.0,
            ambient_shadow_color: DefaultShadowColor,
            spot_shadow_color: DefaultShadowColor,
            clip: false,
            outline: Outline::Rectangle(Rect::from_offset_size(Offset::Zero, bounds.size())),
            blend_mode: BlendMode::SrcOver,
            color_filter: None,
            render_effect: None,
            stretch: StretchEffect::Empty,
            strategy: CompositingStrategy::Auto,
            outsets: [0.0; 4],
        }
    }

    /// A graphics layer's, of `size` at the origin.
    pub fn of_scope(layer: &GraphicsLayerScope, size: Size) -> LayerDraw {
        let bounds = Rect::from_offset_size(Offset::Zero, size);
        let outline = if layer.clip || layer.shadow_elevation > 0.0 {
            layer.shape.create_outline(size, layer.layout_direction, &layer.density)
        } else {
            Outline::Rectangle(bounds)
        };
        let d = &layer.density;
        LayerDraw {
            bounds,
            geometry: LayerGeometry {
                scale_x: layer.scale_x,
                scale_y: layer.scale_y,
                translation_x: layer.translation_x,
                translation_y: layer.translation_y,
                rotation_x: layer.rotation_x,
                rotation_y: layer.rotation_y,
                rotation_z: layer.rotation_z,
                camera_distance: layer.camera_distance,
                pivot_x: layer.transform_origin.pivot_fraction_x * size.width,
                pivot_y: layer.transform_origin.pivot_fraction_y * size.height,
            },
            alpha: layer.alpha,
            shadow_elevation: layer.shadow_elevation,
            ambient_shadow_color: layer.ambient_shadow_color,
            spot_shadow_color: layer.spot_shadow_color,
            clip: layer.clip,
            outline,
            blend_mode: layer.blend_mode,
            color_filter: layer.color_filter,
            render_effect: layer.render_effect.clone(),
            stretch: StretchEffect::Empty,
            strategy: layer.compositing_strategy,
            outsets: [
                d.to_px(layer.outsets.left),
                d.to_px(layer.outsets.top),
                d.to_px(layer.outsets.right),
                d.to_px(layer.outsets.bottom),
            ],
        }
    }
}

/// Whether an outline is drawn as a rounded rect (a rect, or a rounded rect whose corners are circles), and its
/// radii (none for the others).
fn rounded(o: &Outline) -> (bool, CornerRadii) {
    match o {
        Outline::Rectangle(_) => (true, CornerRadii::Zero),
        Outline::Rounded(rr) => {
            let (tl, tr, br, bl) = (
                rr.top_left_corner_radius,
                rr.top_right_corner_radius,
                rr.bottom_right_corner_radius,
                rr.bottom_left_corner_radius,
            );
            let circular = tl.x == tl.y && tr.x == tr.y && br.x == br.y && bl.x == bl.y;
            (circular, CornerRadii::new(tl.x, tr.x, br.x, bl.x))
        }
        Outline::Generic(_) => (false, CornerRadii::Zero),
    }
}

/// An outline as a path (from its own origin).
pub(crate) fn outline_path(o: &Outline) -> Path {
    match o {
        Outline::Generic(p) => p.clone(),
        Outline::Rectangle(r) => {
            let mut p = Path::new();
            p.add_rect(*r, PathDirection::CounterClockwise);
            p
        }
        Outline::Rounded(rr) => {
            let mut p = Path::new();
            p.add_round_rect(rr, PathDirection::CounterClockwise);
            p
        }
    }
}

/// A clip to `o` (from the top left of `bounds`): a rect, a rounded rect whose corners are circles as such, any
/// other outline as a path.
pub(crate) fn push_outline_clip(g: &mut dyn GraphicsContext, o: &Outline, bounds: Rect) {
    let at = o.bounds().translate(bounds.top_left());
    let (is_rounded, radii) = rounded(o);
    if is_rounded && radii.max() <= 0.0 {
        g.push_clip(at);
    } else if is_rounded {
        g.push_clip_round_rect(at, radii, ClipOp::Intersect);
    } else {
        g.push_clip_path(&outline_path(o), bounds.top_left(), ClipOp::Intersect);
    }
}

/// Draws `content` (over the layer's bounds) through the layer: its matrix, its shadow, its clip to its outline,
/// its alpha, blend mode, color filter, render effect and stretch, offscreen only where it has to be (faded without
/// ModulateAlpha, blended, filtered, an effect, stretched, or Offscreen); nothing at all where nothing of it can
/// show.
#[doc(hidden)]
pub fn draw_layer(g: &mut dyn GraphicsContext, l: &LayerDraw, content: &mut dyn FnMut(&mut dyn GraphicsContext)) {
    let alpha = l.alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 || l.geometry.scale_x == 0.0 || l.geometry.scale_y == 0.0 {
        return;
    }
    let m = layer_matrix(&l.geometry);
    let moved = !m.is_identity();
    let offscreen = l.strategy == CompositingStrategy::Offscreen
        || (alpha < 1.0 && l.strategy == CompositingStrategy::Auto)
        || l.blend_mode != BlendMode::SrcOver
        || l.color_filter.is_some()
        || l.render_effect.is_some()
        || l.stretch.requires_layer();
    let modulated = !offscreen && alpha < 1.0; // (ModulateAlpha: each draw faded)
    let shadowed = l.shadow_elevation > 0.0;
    if !moved && !offscreen && !modulated && !l.clip && !shadowed {
        return content(g);
    }
    if moved {
        g.push_transform(&m);
    }
    let [ol, ot, or, ob] = l.outsets;
    let reach = Rect::new(l.bounds.left - ol, l.bounds.top - ot, l.bounds.right + or, l.bounds.bottom + ob);
    // Its shadow: how high each point is, as Android works it out (RenderNode's true 3D transform: turned about x,
    // y and z and scaled about the pivot, raised by the elevation).
    let mut shadow = ElevationShadow::default();
    let mut shows = match &l.render_effect {
        Some(e) => e.output_bounds(reach),
        None => reach,
    };
    if shadowed {
        let q = &l.geometry;
        let r = mul(
            &mul(&rotation(1.0, 0.0, 0.0, q.rotation_x), &rotation(0.0, 1.0, 0.0, q.rotation_y)),
            &rotation(0.0, 0.0, 1.0, q.rotation_z),
        );
        let (a, b) = (r[6] * q.scale_x, r[7] * q.scale_y); // (the z row, times the scale)
        shadow.plane = [a, b, l.shadow_elevation - a * q.pivot_x - b * q.pivot_y];
        shadow.ambient_color = l.ambient_shadow_color;
        shadow.spot_color = l.spot_shadow_color;
        shadow.alpha = alpha;
        let caster = l.outline.bounds().translate(l.bounds.top_left());
        shows = shows.union(shadow.reach(caster, &g.light(), g.light_center()));
    }
    // Nothing of it can show (the transform taken into account): not drawn at all.
    if !g.quick_reject(shows) {
        if shadowed {
            g.draw_elevation_shadow(&l.outline, l.bounds.top_left(), &shadow);
        }
        let o = &l.outline;
        let at = o.bounds().translate(l.bounds.top_left());
        let (is_rounded, radii) = rounded(o);
        if offscreen {
            if let Some(f) = &l.color_filter {
                g.push_color_filter(f);
            }
            if l.blend_mode != BlendMode::SrcOver {
                g.push_blend_mode(l.blend_mode);
            }
            // Clipped: with an effect or a stretch, by a clip around it (what the effect makes is cut, not what it is
            // made of: hwui clips the stretched layer by its outline as it draws it); else by the layer itself, to a
            // rounded rect of its bounds or its outline.
            let stretched = l.stretch.requires_layer();
            let clipped = l.clip && (l.render_effect.is_some() || stretched);
            if clipped {
                push_outline_clip(g, o, l.bounds);
            }
            if stretched {
                g.push_stretch_layer(l.bounds, &l.stretch, l.render_effect.as_ref(), alpha);
            } else if let Some(effect) = &l.render_effect {
                g.push_effect_layer(reach, effect, alpha);
            } else if l.clip && is_rounded && at == l.bounds && l.outsets == [0.0; 4] {
                g.push_layer(l.bounds, radii, alpha, LayerTransform::Identity);
            } else if l.clip {
                let mut p = outline_path(o); // (from the layer's top left, which the outsets move)
                p.translate(l.bounds.top_left() - reach.top_left());
                g.push_outline_layer(reach, &p, alpha, LayerTransform::Identity);
            } else {
                g.push_layer(reach, CornerRadii::Zero, alpha, LayerTransform::Identity);
            }
            content(g);
            g.pop_layer();
            if clipped {
                g.pop_clip();
            }
            if l.blend_mode != BlendMode::SrcOver {
                g.pop_blend_mode();
            }
            if l.color_filter.is_some() {
                g.pop_color_filter();
            }
        } else {
            if modulated {
                let mut fade = ColorMatrix::new();
                fade.set_to_scale(1.0, 1.0, 1.0, alpha);
                g.push_color_filter(&ColorFilter::color_matrix(fade));
            }
            if l.clip {
                push_outline_clip(g, o, l.bounds);
            }
            content(g);
            if l.clip {
                g.pop_clip();
            }
            if modulated {
                g.pop_color_filter();
            }
        }
    }
    if moved {
        g.pop_transform();
    }
}

/// Draws `content` through a graphics layer of `size` (what a coordinator with a layer draws through).
#[doc(hidden)]
pub fn draw_through_layer(
    canvas: &mut dyn GraphicsContext,
    layer: &GraphicsLayerScope,
    size: Size,
    content: &mut dyn FnMut(&mut dyn GraphicsContext),
) {
    draw_layer(canvas, &LayerDraw::of_scope(layer, size), content);
}

// ---- A node drawn in two parts ----------------------------------------------------------------------------------

/// Which part of a node is drawn now (a blur of the node's own drawing: C++'s DrawPart): only its own drawing (its
/// children left out), or only its children (its own drawing between swallowed, what places them passed on).
#[derive(Clone)]
pub(crate) struct DrawPart {
    pub(crate) node: u64,
    pub(crate) own: bool,
    /// While its children are drawn: what lets the drawing through (see [`StateOnly`]).
    pub(crate) pass: Rc<Cell<bool>>,
}

thread_local! {
    pub(crate) static DRAW_PART: RefCell<Option<DrawPart>> = const { RefCell::new(None) };
}

/// Passes on to `to` only what changes how things are drawn (clips, transforms, layers, blend modes, filters,
/// node hooks), nothing drawn — but everything while `pass` is on (the node's children drawn through it).
pub(crate) struct StateOnly<'a> {
    pub(crate) to: &'a mut dyn GraphicsContext,
    pub(crate) pass: Rc<Cell<bool>>,
}

impl GraphicsContext for StateOnly<'_> {
    fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
        if self.pass.get() {
            self.to.fill_round_rect(rect, radii, color);
        }
    }
    fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, width: f32, color: Color) {
        if self.pass.get() {
            self.to.stroke_round_rect(rect, radii, width, color);
        }
    }
    fn draw_line(&mut self, from: Offset, to: Offset, width: f32, color: Color) {
        if self.pass.get() {
            self.to.draw_line(from, to, width, color);
        }
    }
    fn draw_text(&mut self, text: &TextDraw, color: Color) {
        if self.pass.get() {
            self.to.draw_text(text, color);
        }
    }
    fn draw_text_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        if self.pass.get() {
            self.to.draw_text_brush(text, brush, brush_area);
        }
    }
    fn draw_text_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        if self.pass.get() {
            self.to.draw_text_shadow(text, color, blur_radius);
        }
    }
    fn draw_arc(&mut self, oval: Rect, start: f32, sweep: f32, width: f32, color: Color, round_cap: bool) {
        if self.pass.get() {
            self.to.draw_arc(oval, start, sweep, width, color, round_cap);
        }
    }
    fn draw_image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
        if self.pass.get() {
            self.to.draw_image(image, src, dst, alpha, quality);
        }
    }
    fn draw_image_mask(
        &mut self,
        image: &ImageBitmap,
        src: Rect,
        dst: Rect,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
        quality: FilterQuality,
    ) {
        if self.pass.get() {
            self.to.draw_image_mask(image, src, dst, brush, brush_area, alpha, quality);
        }
    }
    fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
        self.to.push_layer(bounds, radii, alpha, transform);
    }
    fn pop_layer(&mut self) {
        self.to.pop_layer();
    }
    fn push_clip(&mut self, rect: Rect) {
        self.to.push_clip(rect);
    }
    fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp) {
        self.to.push_clip_round_rect(rect, radii, op);
    }
    fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp) {
        self.to.push_clip_path(path, origin, op);
    }
    fn pop_clip(&mut self) {
        self.to.pop_clip();
    }
    fn push_translation(&mut self, by: Offset) {
        self.to.push_translation(by);
    }
    fn pop_translation(&mut self) {
        self.to.pop_translation();
    }
    fn push_transform(&mut self, matrix: &Matrix) {
        self.to.push_transform(matrix);
    }
    fn pop_transform(&mut self) {
        self.to.pop_transform();
    }
    fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        self.to.push_effect_layer(bounds, effect, alpha);
    }
    fn push_stretch_layer(&mut self, bounds: Rect, stretch: &StretchEffect, effect: Option<&RenderEffect>, alpha: f32) {
        self.to.push_stretch_layer(bounds, stretch, effect, alpha);
    }
    fn draw_backdrop(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        if self.pass.get() {
            self.to.draw_backdrop(bounds, effect, alpha);
        }
    }
    fn push_blend_mode(&mut self, mode: BlendMode) {
        self.to.push_blend_mode(mode);
    }
    fn pop_blend_mode(&mut self) {
        self.to.pop_blend_mode();
    }
    fn push_anti_alias(&mut self, on: bool) {
        self.to.push_anti_alias(on);
    }
    fn pop_anti_alias(&mut self) {
        self.to.pop_anti_alias();
    }
    fn push_color_filter(&mut self, filter: &ColorFilter) {
        self.to.push_color_filter(filter);
    }
    fn pop_color_filter(&mut self) {
        self.to.pop_color_filter();
    }
    fn quick_reject(&self, rect: Rect) -> bool {
        self.to.quick_reject(rect)
    }
    fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
        if self.pass.get() {
            self.to.fill_gradient(rect, radii, brush, brush_area);
        }
    }
    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        origin: Offset,
        blend_mode: BlendMode,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
    ) {
        if self.pass.get() {
            self.to.draw_vertices(vertices, origin, blend_mode, brush, brush_area, alpha);
        }
    }
    fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke) {
        if self.pass.get() {
            self.to.draw_path(path, origin, brush, brush_area, stroke);
        }
    }
    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &crate::graphics_context::Shadow, inner: bool) {
        if self.pass.get() {
            self.to.draw_shadow(rect, radii, shadow, inner);
        }
    }
    fn draw_shadow_path(
        &mut self,
        outline: &Path,
        origin: Offset,
        shadow: &crate::graphics_context::Shadow,
        inner: bool,
    ) {
        if self.pass.get() {
            self.to.draw_shadow_path(outline, origin, shadow, inner);
        }
    }
    fn light(&self) -> crate::graphics_context::ShadowLight {
        self.to.light()
    }
    fn light_center(&self) -> Offset {
        self.to.light_center()
    }
    fn draw_elevation_shadow(&mut self, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
        if self.pass.get() {
            self.to.draw_elevation_shadow(outline, origin, shadow);
        }
    }
    fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
        self.to.push_outline_layer(bounds, outline, alpha, transform);
    }
    fn draw_shape_shadow(
        &mut self,
        rect: Rect,
        shape: &dyn kompoz_graphics::Shape,
        layout_direction: kompoz_graphics::LayoutDirection,
        density: &kompoz_graphics::Density,
        shadow: &crate::graphics_context::Shadow,
        inner: bool,
    ) {
        if self.pass.get() {
            self.to.draw_shape_shadow(rect, shape, layout_direction, density, shadow, inner);
        }
    }
    fn begin_node(&mut self, node: u64) {
        self.to.begin_node(node);
    }
    fn end_node(&mut self) {
        self.to.end_node();
    }
    fn ended_node_shows(&self) -> bool {
        self.to.ended_node_shows()
    }
    fn record_node(&mut self, node: u64, origin: Offset, unchanged: bool) -> crate::graphics_context::NodeRecording {
        self.to.record_node(node, origin, unchanged)
    }
    fn key_layer(&mut self, key: u64, same: bool) {
        self.to.key_layer(key, same);
    }
    fn layer_content(&self) -> crate::graphics_context::LayerContent {
        self.to.layer_content()
    }
}
