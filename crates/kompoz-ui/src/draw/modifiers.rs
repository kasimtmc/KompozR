//! ui's draw modifiers, as Compose's: `draw_behind`, `draw_with_content`, `draw_with_cache`; `graphics_layer` (its
//! properties, or a block read as the layer is worked out: what it reads changes the layer alone), and what is made
//! of it: `clip`, `clip_to_bounds`, `alpha`, `rotate`, `scale`, `shadow`, `blur`; `drop_shadow` and `inner_shadow`
//! (Compose 1.9's shadows of shapes).
//!
//! `blur` also takes a [`BlurRadiusSpec`] (a progressive blur, Compose 1.13's), a block read while drawing, and a
//! [`BlurType`] (the C++ Kompoz's: what is blurred — the node and its children as Compose's, only what the node
//! draws itself, or what is behind it).

use super::layer_draw::{DRAW_PART, DrawPart, LayerDraw, StateOnly, draw_layer, push_outline_clip};
use super::{ContentDrawScope, DrawModifierNode, DrawScope, GraphicsLayer};
use crate::graphics_context::{DefaultShadowColor, Shadow};
use crate::layout::{
    CompositingStrategy, DefaultCameraDistance, GraphicsLayerScope, LayerBlock, LayerOutsets, LayoutModifierNode,
    Measurable, MeasureResult, MeasureScope, TransformOrigin,
};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, ObserverModifierNode};
use crate::node_capabilities;
use crate::platform::LocalPlatform;
use crate::unit::Constraints;
use kompoz_graphics::{
    BlendMode, BlurEffect, BlurRadiusSpec, Brush, Color, ColorFilter, Density, Dp, IntOffset, IntSize, LayoutDirection,
    Offset, Outline, Rect, RectangleShape, RenderEffect, Shape, Size, TileMode,
};
use kompoz_runtime::named;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- draw_behind, draw_with_content ------------------------------------------------------------------------------

impl Modifier {
    /// Draws `on_draw` behind the content (Compose's `Modifier.drawBehind`).
    pub fn draw_behind(self, on_draw: impl Fn(&mut DrawScope<'_>) + 'static) -> Modifier {
        self.then_element(DrawBehindElement { on_draw: Rc::new(on_draw) })
    }

    /// Draws `on_draw`, which decides where the content goes with `draw_content` (Compose's
    /// `Modifier.drawWithContent`).
    pub fn draw_with_content(self, on_draw: impl Fn(&mut ContentDrawScope<'_>) + 'static) -> Modifier {
        self.then_element(DrawWithContentElement { on_draw: Rc::new(on_draw) })
    }

    /// Builds paths, brushes, layers and the like once (`on_build_draw_cache`, given a [`CacheDrawScope`]) and draws
    /// with them every frame what the [`DrawResult`] it returns says; built again for another size, density or
    /// layout direction, when a state it read changes, or for another block (Compose's `Modifier.drawWithCache`).
    pub fn draw_with_cache(
        self,
        on_build_draw_cache: impl Fn(&mut CacheDrawScope) -> DrawResult + 'static,
    ) -> Modifier {
        self.then_element(DrawWithCacheElement { build: Rc::new(on_build_draw_cache) })
    }
}

type DrawFn = Rc<dyn Fn(&mut DrawScope<'_>)>;
type ContentFn = Rc<dyn Fn(&mut ContentDrawScope<'_>)>;

struct DrawBehindElement {
    on_draw: DrawFn,
}

impl PartialEq for DrawBehindElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_draw, &other.on_draw)
    }
}

impl ModifierNodeElement for DrawBehindElement {
    type Node = DrawBackgroundModifier;
    fn create(&self) -> DrawBackgroundModifier {
        DrawBackgroundModifier { on_draw: self.on_draw.clone() }
    }
    fn update(&self, node: &mut DrawBackgroundModifier) {
        node.on_draw = self.on_draw.clone();
    }
    fn name(&self) -> &'static str {
        "drawBehind"
    }
}

/// The node of `draw_behind` (Compose's DrawBackgroundModifier).
pub struct DrawBackgroundModifier {
    on_draw: DrawFn,
}

impl ModifierNode for DrawBackgroundModifier {
    node_capabilities!(draw);
}

impl DrawModifierNode for DrawBackgroundModifier {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        (self.on_draw)(scope);
        scope.draw_content();
    }
}

struct DrawWithContentElement {
    on_draw: ContentFn,
}

impl PartialEq for DrawWithContentElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_draw, &other.on_draw)
    }
}

impl ModifierNodeElement for DrawWithContentElement {
    type Node = DrawWithContentModifier;
    fn create(&self) -> DrawWithContentModifier {
        DrawWithContentModifier { on_draw: self.on_draw.clone() }
    }
    fn update(&self, node: &mut DrawWithContentModifier) {
        node.on_draw = self.on_draw.clone();
    }
    fn name(&self) -> &'static str {
        "drawWithContent"
    }
}

/// The node of `draw_with_content` (Compose's DrawWithContentModifier).
pub struct DrawWithContentModifier {
    on_draw: ContentFn,
}

impl ModifierNode for DrawWithContentModifier {
    node_capabilities!(draw);
}

impl DrawModifierNode for DrawWithContentModifier {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        (self.on_draw)(scope);
    }
}

// ---- draw_with_cache ----------------------------------------------------------------------------------------------

/// What a cached drawing draws each frame (Compose's DrawResult): made by [`CacheDrawScope::on_draw_behind`] or
/// [`CacheDrawScope::on_draw_with_content`].
#[derive(Clone)]
pub struct DrawResult {
    block: ContentFn,
}

impl fmt::Debug for DrawResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("DrawResult")
    }
}

/// Where a cached drawing is built (Compose's CacheDrawScope): the size, density (it derefs to it) and layout
/// direction it is built for; what it draws each frame (`on_draw_behind`, `on_draw_with_content`); layers it keeps
/// (`obtain_graphics_layer`), recorded with the content (`record`) while it is built.
pub struct CacheDrawScope<'c, 'a> {
    content: &'c mut ContentDrawScope<'a>,
    layers: &'c mut Vec<GraphicsLayer>,
    platform: Option<crate::platform::PlatformHandle>,
}

impl CacheDrawScope<'_, '_> {
    /// The size it is built for, px.
    pub fn size(&self) -> Size {
        self.content.size
    }
    pub fn density(&self) -> Density {
        self.content.density
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.content.layout_direction
    }

    /// Draws `block` behind the content each frame.
    pub fn on_draw_behind(&self, block: impl Fn(&mut DrawScope<'_>) + 'static) -> DrawResult {
        DrawResult {
            block: Rc::new(move |c: &mut ContentDrawScope<'_>| {
                block(c);
                c.draw_content();
            }),
        }
    }

    /// Draws `block` each frame, the content where it says (`draw_content`).
    pub fn on_draw_with_content(&self, block: impl Fn(&mut ContentDrawScope<'_>) + 'static) -> DrawResult {
        DrawResult { block: Rc::new(block) }
    }

    /// A layer kept with what is built (Compose's obtainGraphicsLayer): let go of when it is built again.
    pub fn obtain_graphics_layer(&mut self) -> GraphicsLayer {
        let layer = GraphicsLayer::new(self.platform.clone());
        self.layers.push(layer.clone());
        layer
    }

    /// Records `block` (with the content: `draw_content`) into `layer` now (Compose's GraphicsLayer.record in a
    /// CacheDrawScope), of the size it is built for.
    pub fn record(&mut self, layer: &GraphicsLayer, block: impl FnOnce(&mut ContentDrawScope<'_>)) {
        let s = self.content.size;
        let size = IntSize::new(s.width.round() as i32, s.height.round() as i32);
        self.content.record_with(layer, size, block);
    }

    /// The same of `size`.
    pub fn record_sized(
        &mut self,
        layer: &GraphicsLayer,
        size: IntSize,
        block: impl FnOnce(&mut ContentDrawScope<'_>),
    ) {
        self.content.record_with(layer, size, block);
    }
}

impl std::ops::Deref for CacheDrawScope<'_, '_> {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.content.density
    }
}

type BuildFn = Rc<dyn Fn(&mut CacheDrawScope) -> DrawResult>;

struct DrawWithCacheElement {
    build: BuildFn,
}

impl PartialEq for DrawWithCacheElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.build, &other.build)
    }
}

impl ModifierNodeElement for DrawWithCacheElement {
    type Node = CacheDrawModifierNode;
    fn create(&self) -> CacheDrawModifierNode {
        CacheDrawModifierNode::new(self.build.clone())
    }
    fn update(&self, node: &mut CacheDrawModifierNode) {
        node.build = self.build.clone();
        node.invalidate_draw_cache();
    }
    fn name(&self) -> &'static str {
        "drawWithCache"
    }
}

/// The node of `draw_with_cache` (Compose's CacheDrawModifierNode): its cache built when it is drawn and isn't
/// valid, what the build reads observed.
pub struct CacheDrawModifierNode {
    build: BuildFn,
    result: Option<DrawResult>,
    built_for: Option<(Size, Density, LayoutDirection)>,
    layers: Vec<GraphicsLayer>,
    handle: Option<NodeHandle>,
}

impl CacheDrawModifierNode {
    pub(crate) fn new(build: BuildFn) -> CacheDrawModifierNode {
        CacheDrawModifierNode { build, result: None, built_for: None, layers: Vec::new(), handle: None }
    }

    /// Built again the next time it is drawn, which is now (Compose's invalidateDrawCache).
    pub fn invalidate_draw_cache(&mut self) {
        self.result = None;
        if let Some(h) = &self.handle {
            h.invalidate_draw();
        }
    }
}

impl ModifierNode for CacheDrawModifierNode {
    node_capabilities!(draw, observer);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn on_detach(&mut self, _: &NodeCx) {
        self.result = None;
        for l in self.layers.drain(..) {
            l.release();
        }
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl ObserverModifierNode for CacheDrawModifierNode {
    fn on_observed_reads_changed(&mut self) {
        self.invalidate_draw_cache();
    }
}

impl DrawModifierNode for CacheDrawModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let key = (scope.size, scope.density, scope.layout_direction);
        if self.result.is_none() || self.built_for != Some(key) {
            for l in self.layers.drain(..) {
                l.release();
            }
            let platform = self.handle.as_ref().map(|h| h.current_value_of(&LocalPlatform));
            let build = self.build.clone();
            let mut layers = Vec::new();
            let run = || build(&mut CacheDrawScope { content: &mut *scope, layers: &mut layers, platform });
            let result = match &self.handle {
                Some(h) => h.observe_reads(run),
                None => run(),
            };
            self.layers = layers;
            self.result = Some(result);
            self.built_for = Some(key);
        }
        if let Some(result) = &self.result {
            let block = result.block.clone();
            block(scope);
        }
    }

    fn on_measure_result_changed(&mut self) {
        self.result = None;
    }
}

// ---- graphics_layer ----------------------------------------------------------------------------------------------

#[named]
impl Modifier {
    /// Draws the content through a layer of these properties (Compose's `Modifier.graphicsLayer` with them, every
    /// one defaulted): scaled, turned, moved, faded, shadowed, clipped to its shape, composited through an effect, a
    /// blend mode, a color filter; offscreen only where it has to be.
    #[allow(clippy::too_many_arguments)]
    pub fn graphics_layer(
        self,
        #[default(1.0)] scale_x: f32,
        #[default(1.0)] scale_y: f32,
        #[default(1.0)] alpha: f32,
        #[default(0.0)] translation_x: f32,
        #[default(0.0)] translation_y: f32,
        #[default(0.0)] shadow_elevation: f32,
        #[default(0.0)] rotation_x: f32,
        #[default(0.0)] rotation_y: f32,
        #[default(0.0)] rotation_z: f32,
        #[default(DefaultCameraDistance)] camera_distance: f32,
        #[default(TransformOrigin::Center)] transform_origin: TransformOrigin,
        #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
        #[default(false)] clip: bool,
        #[default(None)] render_effect: Option<RenderEffect>,
        #[default(DefaultShadowColor)] ambient_shadow_color: Color,
        #[default(DefaultShadowColor)] spot_shadow_color: Color,
        #[default(CompositingStrategy::Auto)] compositing_strategy: CompositingStrategy,
        #[default(BlendMode::SrcOver)] blend_mode: BlendMode,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(LayerOutsets::Zero)] outsets: LayerOutsets,
    ) -> Modifier {
        self.graphics_layer_with(GraphicsLayerScope {
            scale_x,
            scale_y,
            alpha,
            translation_x,
            translation_y,
            shadow_elevation,
            ambient_shadow_color,
            spot_shadow_color,
            rotation_x,
            rotation_y,
            rotation_z,
            camera_distance,
            transform_origin,
            shape,
            clip,
            render_effect,
            compositing_strategy,
            blend_mode,
            color_filter,
            outsets,
            ..GraphicsLayerScope::default()
        })
    }

    /// The shadow of an elevation (Compose's `Modifier.shadow`): a graphics layer `elevation` high of `shape`, in
    /// its colors, clipped to it (by default when the elevation is above 0: `clip` none); nothing at all without
    /// either.
    pub fn shadow(
        self,
        elevation: Dp,
        #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
        #[default(None)] clip: Option<bool>,
        #[default(DefaultShadowColor)] ambient_color: Color,
        #[default(DefaultShadowColor)] spot_color: Color,
    ) -> Modifier {
        self.shadow_element(elevation, shape, clip.unwrap_or(elevation.0 > 0.0), ambient_color, spot_color)
    }

    /// What is drawn blurred by `radius` both ways (Compose's `Modifier.blur(radius)`); see
    /// [`blur_xy`](Modifier::blur_xy).
    pub fn blur(
        self,
        radius: Dp,
        #[default(BlurredEdgeTreatment::Rectangle)] edge_treatment: BlurredEdgeTreatment,
        #[default(BlurType::Family)] blur_type: BlurType,
    ) -> Modifier {
        self.blur_xy_with(radius, radius, edge_treatment, blur_type)
    }

    /// What is drawn blurred by `radius_x` across and `radius_y` down (Compose's `Modifier.blur(radiusX, radiusY)`):
    /// a graphics layer with a blur effect, clipped to the edge treatment's shape if it has one (its edges' pixels
    /// clamped then, else fading out). Unless it is clipped, a radius of 0 either way draws it as it is. Another
    /// `blur_type` blurs only what the node draws itself, or what is behind it.
    pub fn blur_xy(
        self,
        radius_x: Dp,
        radius_y: Dp,
        #[default(BlurredEdgeTreatment::Rectangle)] edge_treatment: BlurredEdgeTreatment,
        #[default(BlurType::Family)] blur_type: BlurType,
    ) -> Modifier {
        self.blur_xy_with(radius_x, radius_y, edge_treatment, blur_type)
    }

    /// Blurred as much as `blur_radius_spec` says where (a progressive blur), clipped to the edge treatment's shape
    /// if it has one, faded by `alpha`: always a layer, which a uniform radius of 0 leaves without an effect.
    pub fn blur_spec(
        self,
        blur_radius_spec: BlurRadiusSpec,
        #[default(BlurredEdgeTreatment::Rectangle)] edge_treatment: BlurredEdgeTreatment,
        #[default(1.0)] alpha: f32,
        #[default(BlurType::Family)] blur_type: BlurType,
    ) -> Modifier {
        self.blur_spec_with(blur_radius_spec, edge_treatment, alpha, blur_type)
    }
}

impl Modifier {
    /// The graphics layer of `layer`'s properties (its size, density and direction unused), for plain code.
    pub fn graphics_layer_with(self, layer: GraphicsLayerScope) -> Modifier {
        self.then_element(GraphicsLayerElement { layer })
    }

    /// Draws the content through a layer `block` sets up as the layer is worked out (Compose's graphicsLayer {}):
    /// what it reads changes the layer alone, without measuring, placing or drawing anything again.
    pub fn graphics_layer_block(self, block: impl Fn(&mut GraphicsLayerScope) + 'static) -> Modifier {
        self.then_element(BlockGraphicsLayerElement { block: Rc::new(block), name: "graphicsLayer" })
    }

    /// Cut to `shape` (Compose's `Modifier.clip`): a graphics layer clipped to it.
    pub fn clip(self, shape: impl Shape + 'static) -> Modifier {
        self.graphics_layer_with(GraphicsLayerScope { shape: Arc::new(shape), clip: true, ..Default::default() })
    }

    /// Cut to its bounds (Compose's `Modifier.clipToBounds`).
    pub fn clip_to_bounds(self) -> Modifier {
        self.graphics_layer_with(GraphicsLayerScope { clip: true, ..Default::default() })
    }

    /// Faded to `alpha` (Compose's `Modifier.alpha`: a graphics layer of it, clipped); nothing at 1.
    pub fn alpha(self, alpha: f32) -> Modifier {
        if alpha == 1.0 {
            return self;
        }
        self.graphics_layer_with(GraphicsLayerScope { alpha, clip: true, ..Default::default() })
    }

    /// Turned `degrees` clockwise about its middle (Compose's `Modifier.rotate`); nothing for 0.
    pub fn rotate(self, degrees: f32) -> Modifier {
        if degrees == 0.0 {
            return self;
        }
        self.graphics_layer_with(GraphicsLayerScope { rotation_z: degrees, ..Default::default() })
    }

    /// Scaled about its middle (Compose's `Modifier.scale(scaleX, scaleY)`); nothing for 1 both ways.
    pub fn scale_xy(self, scale_x: f32, scale_y: f32) -> Modifier {
        if scale_x == 1.0 && scale_y == 1.0 {
            return self;
        }
        self.graphics_layer_with(GraphicsLayerScope { scale_x, scale_y, ..Default::default() })
    }

    /// The same both ways (Compose's `Modifier.scale(scale)`).
    pub fn scale(self, scale: f32) -> Modifier {
        self.scale_xy(scale, scale)
    }

    /// [`shadow`](Modifier::shadow) with every parameter.
    pub fn shadow_with(
        self,
        elevation: Dp,
        shape: impl Shape + 'static,
        clip: bool,
        ambient_color: Color,
        spot_color: Color,
    ) -> Modifier {
        self.shadow_element(elevation, Arc::new(shape), clip, ambient_color, spot_color)
    }

    fn shadow_element(
        self,
        elevation: Dp,
        shape: Arc<dyn Shape>,
        clip: bool,
        ambient_color: Color,
        spot_color: Color,
    ) -> Modifier {
        if elevation.0 <= 0.0 && !clip {
            return self;
        }
        self.then_element(ShadowGraphicsLayerElement { elevation, shape, clip, ambient_color, spot_color })
    }

    /// [`blur_xy`](Modifier::blur_xy) with every parameter.
    pub fn blur_xy_with(
        self,
        radius_x: Dp,
        radius_y: Dp,
        edge_treatment: BlurredEdgeTreatment,
        blur_type: BlurType,
    ) -> Modifier {
        let clip = edge_treatment.shape().is_some();
        if !((radius_x.0 > 0.0 && radius_y.0 > 0.0) || clip) {
            return self;
        }
        if blur_type != BlurType::Family {
            let mut scope = BlurScope { edge_treatment, blur_type, ..BlurScope::default() };
            scope.radii = Some((radius_x, radius_y));
            return self.then_element(BlurElement { scope: Some(scope), block: None });
        }
        // Compose's: a graphicsLayer, blurred only when both radii are.
        let tile_mode = if clip { TileMode::Clamp } else { TileMode::Decal };
        let shape = edge_treatment.shape().unwrap_or_else(|| Arc::new(RectangleShape));
        self.then_element(BlurLayerElement { radius_x, radius_y, tile_mode, shape, clip })
    }

    /// [`blur_spec`](Modifier::blur_spec) with every parameter.
    pub fn blur_spec_with(
        self,
        blur_radius_spec: BlurRadiusSpec,
        edge_treatment: BlurredEdgeTreatment,
        alpha: f32,
        blur_type: BlurType,
    ) -> Modifier {
        let scope = BlurScope { blur_radius_spec, edge_treatment, alpha, blur_type, ..BlurScope::default() };
        self.then_element(BlurElement { scope: Some(scope), block: None })
    }

    /// The same, set by `block` as it is drawn (from the defaults, the size given): what it reads (an animation)
    /// draws it again, never composes it.
    pub fn blur_block(self, block: impl Fn(&mut BlurScope) + 'static) -> Modifier {
        self.then_element(BlurElement { scope: None, block: Some(Rc::new(block)) })
    }

    /// The shadow of `shape` under it (Compose's `Modifier.dropShadow`): its outline spread, moved and blurred, in
    /// its color or brush, at its alpha, by its blend mode; before what comes after.
    pub fn drop_shadow(self, shape: impl Shape + 'static, shadow: Shadow) -> Modifier {
        self.then_element(ShapeShadowElement {
            shape: Arc::new(shape),
            shadow: Some(shadow),
            block: None,
            inner: false,
        })
    }

    /// The same, set by `block` as it is drawn (Compose's dropShadow(shape) { }).
    pub fn drop_shadow_block(
        self,
        shape: impl Shape + 'static,
        block: impl Fn(&mut ShadowScope) + 'static,
    ) -> Modifier {
        self.then_element(ShapeShadowElement {
            shape: Arc::new(shape),
            shadow: None,
            block: Some(Rc::new(block)),
            inner: false,
        })
    }

    /// The shadow of `shape` inside it (Compose's `Modifier.innerShadow`): its color where the outline worn away by
    /// the spread and moved isn't, blurred, cut to the outline; over what comes before, under what comes after.
    pub fn inner_shadow(self, shape: impl Shape + 'static, shadow: Shadow) -> Modifier {
        self.then_element(ShapeShadowElement { shape: Arc::new(shape), shadow: Some(shadow), block: None, inner: true })
    }

    /// The same, set by `block` as it is drawn (Compose's innerShadow(shape) { }).
    pub fn inner_shadow_block(
        self,
        shape: impl Shape + 'static,
        block: impl Fn(&mut ShadowScope) + 'static,
    ) -> Modifier {
        self.then_element(ShapeShadowElement {
            shape: Arc::new(shape),
            shadow: None,
            block: Some(Rc::new(block)),
            inner: true,
        })
    }
}

/// A graphics layer's properties as an element compares them.
fn same_layer(a: &GraphicsLayerScope, b: &GraphicsLayerScope) -> bool {
    a.same_as(b)
}

struct GraphicsLayerElement {
    layer: GraphicsLayerScope,
}

impl PartialEq for GraphicsLayerElement {
    fn eq(&self, other: &Self) -> bool {
        same_layer(&self.layer, &other.layer)
    }
}

/// A block setting `layer`'s properties (all but the scope's size, density and direction).
fn block_of(layer: &GraphicsLayerScope) -> LayerBlock {
    let layer = layer.clone();
    Rc::new(move |scope: &mut GraphicsLayerScope| {
        let (size, density, direction) = (scope.size, scope.density, scope.layout_direction);
        *scope = GraphicsLayerScope { size, density, layout_direction: direction, ..layer.clone() };
    })
}

impl ModifierNodeElement for GraphicsLayerElement {
    type Node = LayerBlockNode;
    fn create(&self) -> LayerBlockNode {
        LayerBlockNode::new(block_of(&self.layer))
    }
    fn update(&self, node: &mut LayerBlockNode) {
        node.set_block(block_of(&self.layer));
    }
    fn name(&self) -> &'static str {
        "graphicsLayer"
    }
}

struct BlockGraphicsLayerElement {
    block: LayerBlock,
    name: &'static str,
}

impl PartialEq for BlockGraphicsLayerElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.block, &other.block)
    }
}

impl ModifierNodeElement for BlockGraphicsLayerElement {
    type Node = LayerBlockNode;
    fn create(&self) -> LayerBlockNode {
        LayerBlockNode::new(self.block.clone())
    }
    fn update(&self, node: &mut LayerBlockNode) {
        node.set_block(self.block.clone());
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

/// Compose's ShadowGraphicsLayerElement: a layer block of the elevation (px at the layer's density), the shape, the
/// clip and the colors.
struct ShadowGraphicsLayerElement {
    elevation: Dp,
    shape: Arc<dyn Shape>,
    clip: bool,
    ambient_color: Color,
    spot_color: Color,
}

impl PartialEq for ShadowGraphicsLayerElement {
    fn eq(&self, o: &Self) -> bool {
        self.elevation == o.elevation
            && *self.shape == *o.shape
            && self.clip == o.clip
            && self.ambient_color == o.ambient_color
            && self.spot_color == o.spot_color
    }
}

impl ShadowGraphicsLayerElement {
    fn block(&self) -> LayerBlock {
        let (elevation, shape, clip) = (self.elevation, self.shape.clone(), self.clip);
        let (ambient, spot) = (self.ambient_color, self.spot_color);
        Rc::new(move |l: &mut GraphicsLayerScope| {
            l.shadow_elevation = l.density.to_px(elevation);
            l.shape = shape.clone();
            l.clip = clip;
            l.ambient_shadow_color = ambient;
            l.spot_shadow_color = spot;
        })
    }
}

impl ModifierNodeElement for ShadowGraphicsLayerElement {
    type Node = LayerBlockNode;
    fn create(&self) -> LayerBlockNode {
        LayerBlockNode::new(self.block())
    }
    fn update(&self, node: &mut LayerBlockNode) {
        node.set_block(self.block());
    }
    fn name(&self) -> &'static str {
        "shadow"
    }
}

/// Compose's blur(radiusX, radiusY): a layer block of a blur effect (the radii px at the layer's density, blurred
/// only when both are), the edge treatment's shape and clip.
struct BlurLayerElement {
    radius_x: Dp,
    radius_y: Dp,
    tile_mode: TileMode,
    shape: Arc<dyn Shape>,
    clip: bool,
}

impl PartialEq for BlurLayerElement {
    fn eq(&self, o: &Self) -> bool {
        self.radius_x == o.radius_x
            && self.radius_y == o.radius_y
            && self.tile_mode == o.tile_mode
            && *self.shape == *o.shape
            && self.clip == o.clip
    }
}

impl BlurLayerElement {
    fn block(&self) -> LayerBlock {
        let (rx, ry, tile_mode, shape, clip) =
            (self.radius_x, self.radius_y, self.tile_mode, self.shape.clone(), self.clip);
        Rc::new(move |l: &mut GraphicsLayerScope| {
            let (x, y) = (l.density.to_px(rx), l.density.to_px(ry));
            l.render_effect = (x > 0.0 && y > 0.0).then(|| BlurEffect(x, y, tile_mode));
            l.shape = shape.clone();
            l.clip = clip;
        })
    }
}

impl ModifierNodeElement for BlurLayerElement {
    type Node = LayerBlockNode;
    fn create(&self) -> LayerBlockNode {
        LayerBlockNode::new(self.block())
    }
    fn update(&self, node: &mut LayerBlockNode) {
        node.set_block(self.block());
    }
    fn name(&self) -> &'static str {
        "blur"
    }
}

/// The node of the graphics layer modifiers (Compose's SimpleGraphicsLayerModifier and BlockGraphicsLayerModifier):
/// what it wraps placed through a layer of its block; a new block places it again (not measured).
pub struct LayerBlockNode {
    block: Rc<RefCell<LayerBlock>>,
    handle: Option<NodeHandle>,
}

impl LayerBlockNode {
    fn new(block: LayerBlock) -> LayerBlockNode {
        LayerBlockNode { block: Rc::new(RefCell::new(block)), handle: None }
    }

    /// Compose's invalidateLayerBlock: placed again with it.
    fn set_block(&mut self, block: LayerBlock) {
        *self.block.borrow_mut() = block;
        if let Some(h) = &self.handle {
            h.invalidate_placement();
        }
    }
}

impl ModifierNode for LayerBlockNode {
    node_capabilities!(layout);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutModifierNode for LayerBlockNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let block = self.block.clone();
        scope.layout(placeable.width(), placeable.height(), move |p| {
            let b = block.borrow().clone();
            p.place_with_layer_block(&placeable, IntOffset::Zero, b, 0.0)
        })
    }
}

// ---- drop_shadow, inner_shadow -----------------------------------------------------------------------------------

/// What a shadow block sets (Compose's ShadowScope), px; the density (it derefs to it) to set them by.
#[derive(Clone, Debug)]
pub struct ShadowScope {
    pub radius: f32,
    pub spread: f32,
    pub color: Color,
    pub brush: Option<Brush>,
    pub alpha: f32,
    pub blend_mode: BlendMode,
    pub offset: Offset,
    density: Density,
}

impl ShadowScope {
    fn new(density: Density) -> ShadowScope {
        ShadowScope {
            radius: 0.0,
            spread: 0.0,
            color: Color::Black,
            brush: None,
            alpha: 1.0,
            blend_mode: BlendMode::SrcOver,
            offset: Offset::Zero,
            density,
        }
    }

    fn shadow(&self) -> Shadow {
        Shadow {
            color: self.color,
            offset: self.offset,
            blur_radius: self.radius,
            spread: self.spread,
            brush: self.brush.clone(),
            alpha: self.alpha,
            blend_mode: self.blend_mode,
        }
    }
}

impl std::ops::Deref for ShadowScope {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

type ShadowBlock = Rc<dyn Fn(&mut ShadowScope)>;

struct ShapeShadowElement {
    shape: Arc<dyn Shape>,
    shadow: Option<Shadow>,
    block: Option<ShadowBlock>,
    inner: bool,
}

impl PartialEq for ShapeShadowElement {
    fn eq(&self, o: &Self) -> bool {
        let same_block = match (&self.block, &o.block) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        *self.shape == *o.shape && self.shadow == o.shadow && same_block && self.inner == o.inner
    }
}

impl ModifierNodeElement for ShapeShadowElement {
    type Node = ShapeShadowNode;
    fn create(&self) -> ShapeShadowNode {
        ShapeShadowNode {
            shape: self.shape.clone(),
            shadow: self.shadow.clone(),
            block: self.block.clone(),
            inner: self.inner,
        }
    }
    fn update(&self, node: &mut ShapeShadowNode) {
        node.shape = self.shape.clone();
        node.shadow = self.shadow.clone();
        node.block = self.block.clone();
        node.inner = self.inner;
    }
    fn name(&self) -> &'static str {
        if self.inner { "innerShadow" } else { "dropShadow" }
    }
}

/// The node of `drop_shadow` and `inner_shadow` (Compose's SimpleDropShadowNode, BlockInnerShadowNode, …): the
/// shape's shadow drawn through the context's `draw_shape_shadow`, then the content.
pub struct ShapeShadowNode {
    shape: Arc<dyn Shape>,
    shadow: Option<Shadow>,
    block: Option<ShadowBlock>,
    inner: bool,
}

impl ModifierNode for ShapeShadowNode {
    node_capabilities!(draw);
}

impl DrawModifierNode for ShapeShadowNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let shadow = match (&self.shadow, &self.block) {
            (Some(s), _) => s.clone(),
            (None, Some(block)) => {
                let mut s = ShadowScope::new(scope.density);
                block(&mut s);
                s.shadow()
            }
            (None, None) => Shadow::default(),
        };
        let rect = scope.area();
        let (direction, density) = (scope.layout_direction, scope.density);
        scope.canvas.draw_shape_shadow(rect, &*self.shape, direction, &density, &shadow, self.inner);
        scope.draw_content();
    }
}

// ---- blur ---------------------------------------------------------------------------------------------------------

/// How a blur's edges are treated (Compose's BlurredEdgeTreatment): cut to a shape (the bounds: `Rectangle`), the
/// edges' pixels taken past it (Clamp); or spreading out past the bounds, fading (`Unbounded`: Decal).
#[derive(Clone)]
pub enum BlurredEdgeTreatment {
    Rectangle,
    Unbounded,
    Shape(Arc<dyn Shape>),
}

impl BlurredEdgeTreatment {
    /// Cut to `shape`.
    pub fn of(shape: impl Shape + 'static) -> BlurredEdgeTreatment {
        BlurredEdgeTreatment::Shape(Arc::new(shape))
    }

    /// The shape it cuts to (none: unbounded).
    pub fn shape(&self) -> Option<Arc<dyn Shape>> {
        match self {
            BlurredEdgeTreatment::Rectangle => Some(Arc::new(RectangleShape)),
            BlurredEdgeTreatment::Unbounded => None,
            BlurredEdgeTreatment::Shape(s) => Some(s.clone()),
        }
    }
}

impl PartialEq for BlurredEdgeTreatment {
    fn eq(&self, o: &Self) -> bool {
        match (self.shape(), o.shape()) {
            (None, None) => true,
            (Some(a), Some(b)) => *a == *b,
            _ => false,
        }
    }
}

impl fmt::Debug for BlurredEdgeTreatment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BlurredEdgeTreatment(shape={:?})", self.shape())
    }
}

/// What a blur blurs (the C++ Kompoz's): the node and its children (Family, Compose's); what is drawn behind the
/// node, within its shape, the node and its children sharp over it (Morph: frosted glass); or only what the node
/// draws itself (its background, border, image, draw_behind…), its children sharp over it (SelfOnly). The alpha
/// goes with what is blurred.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum BlurType {
    #[default]
    Family,
    Morph,
    SelfOnly,
}

/// What a blur block sets as it is drawn (Compose 1.13's BlurScope), each time from these: how much it blurs where,
/// its edges, its alpha, what it blurs; the node's size (px) and density (it derefs to it) to read.
#[derive(Clone)]
pub struct BlurScope {
    pub blur_radius_spec: BlurRadiusSpec,
    pub edge_treatment: BlurredEdgeTreatment,
    pub alpha: f32,
    pub blur_type: BlurType,
    size: Size,
    density: Density,
    /// The radii of `blur_xy` (a radius form of another type), in place of the spec.
    radii: Option<(Dp, Dp)>,
}

impl BlurScope {
    pub fn size(&self) -> Size {
        self.size
    }
}

impl Default for BlurScope {
    fn default() -> Self {
        BlurScope {
            blur_radius_spec: BlurRadiusSpec::uniform(Dp(0.0)),
            edge_treatment: BlurredEdgeTreatment::Rectangle,
            alpha: 1.0,
            blur_type: BlurType::Family,
            size: Size::Zero,
            density: Density::default(),
            radii: None,
        }
    }
}

impl PartialEq for BlurScope {
    fn eq(&self, o: &Self) -> bool {
        self.blur_radius_spec == o.blur_radius_spec
            && self.edge_treatment == o.edge_treatment
            && self.alpha == o.alpha
            && self.blur_type == o.blur_type
            && self.radii == o.radii
    }
}

impl std::ops::Deref for BlurScope {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

type BlurBlock = Rc<dyn Fn(&mut BlurScope)>;

struct BlurElement {
    scope: Option<BlurScope>,
    block: Option<BlurBlock>,
}

impl PartialEq for BlurElement {
    fn eq(&self, o: &Self) -> bool {
        match (&self.block, &o.block) {
            (None, None) => self.scope == o.scope,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl ModifierNodeElement for BlurElement {
    type Node = BlurNode;
    fn create(&self) -> BlurNode {
        BlurNode { scope: self.scope.clone(), block: self.block.clone() }
    }
    fn update(&self, node: &mut BlurNode) {
        node.scope = self.scope.clone();
        node.block = self.block.clone();
    }
    fn name(&self) -> &'static str {
        "blur"
    }
}

/// The node of the blurs that aren't Compose's graphics layer: its effect (a blur of the radii, blurred only when
/// both are, or the spec's for the node's size), Clamp at the edges when it is cut to a shape, Decal when it spreads
/// out; drawn as its type says.
pub struct BlurNode {
    scope: Option<BlurScope>,
    block: Option<BlurBlock>,
}

impl ModifierNode for BlurNode {
    node_capabilities!(draw);
}

impl DrawModifierNode for BlurNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let mut b = match &self.block {
            Some(_) => BlurScope::default(),
            None => self.scope.clone().unwrap_or_default(),
        };
        b.size = scope.size;
        b.density = scope.density;
        if let Some(block) = &self.block {
            block(&mut b);
        }
        let shape = b.edge_treatment.shape();
        let clip = shape.is_some();
        let edges = if clip { TileMode::Clamp } else { TileMode::Decal };
        let bounds = scope.area();
        let mut d = LayerDraw::plain(bounds);
        d.render_effect = match b.radii {
            Some((rx, ry)) => {
                let (x, y) = (b.density.to_px(rx), b.density.to_px(ry));
                (x > 0.0 && y > 0.0).then(|| BlurEffect(x, y, edges))
            }
            None if b.blur_radius_spec != BlurRadiusSpec::uniform(Dp(0.0)) => {
                Some(b.blur_radius_spec.create_render_effect(b.size, edges, &b.density))
            }
            None => None,
        };
        d.alpha = b.alpha;
        d.clip = clip;
        d.outline = match &shape {
            Some(s) => s.create_outline(b.size, scope.layout_direction, &scope.density),
            None => Outline::Rectangle(Rect::from_offset_size(Offset::Zero, b.size)),
        };
        let Some(node) = scope.next.coordinator.layout_node() else { return };
        match b.blur_type {
            BlurType::Family => {
                let next = scope.next.clone();
                draw_layer(&mut *scope.scope.canvas, &d, &mut |g| draw_content_into(next.clone(), g, bounds));
            }
            BlurType::SelfOnly => {
                // What the node draws itself through the layer; then its children over it, sharp, placed as the
                // nodes in between place them.
                let pass = Rc::new(Cell::new(false));
                let own = DrawPart { node: node.id, own: true, pass: pass.clone() };
                let outer = DRAW_PART.with(|p| p.replace(Some(own)));
                let next = scope.next.clone();
                draw_layer(&mut *scope.scope.canvas, &d, &mut |g| draw_content_into(next.clone(), g, bounds));
                DRAW_PART.with(|p| *p.borrow_mut() = Some(DrawPart { node: node.id, own: false, pass: pass.clone() }));
                let mut placing = StateOnly { to: &mut *scope.scope.canvas, pass };
                draw_content_into(next, &mut placing, bounds);
                DRAW_PART.with(|p| *p.borrow_mut() = outer);
            }
            BlurType::Morph => {
                // What is behind, blurred, within its shape; the node over it, sharp.
                let alpha = b.alpha.clamp(0.0, 1.0);
                if let Some(effect) = &d.render_effect
                    && alpha > 0.0
                    && !scope.canvas.quick_reject(effect.output_bounds(bounds))
                {
                    if clip {
                        push_outline_clip(&mut *scope.scope.canvas, &d.outline, bounds);
                    }
                    scope.canvas.draw_backdrop(bounds, effect, alpha);
                    if clip {
                        scope.canvas.pop_clip();
                    }
                }
                scope.draw_content();
            }
        }
    }
}

/// What is below a node drawn onto `g` (a content scope's `draw_content` over another context), the scope at
/// `bounds`'s top left.
fn draw_content_into(next: super::ContentNext, g: &mut dyn crate::graphics_context::GraphicsContext, bounds: Rect) {
    let mut inner = ContentDrawScope {
        scope: DrawScope::at(g, bounds.top_left(), bounds.size(), LayoutDirection::Ltr, Density::default()),
        next,
    };
    inner.draw_content();
}
