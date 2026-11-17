//! A drawing kept to draw again (Compose's `androidx.compose.ui.graphics.layer.GraphicsLayer`): recorded in a draw
//! scope, then drawn — faded, scaled, turned, moved, shadowed, clipped — as often as wanted, or turned into pixels
//! by the platform.
//!
//! ```ignore
//! let layer = remember_graphics_layer();
//! Box(modifier = Modifier.draw_with_content(move |d| {
//!     d.record(&layer, |d| d.draw_content());
//!     d.draw_layer(&layer);
//! }));
//! let shot = layer.to_image_bitmap(); // later, a "share" button
//! ```

use super::layer_draw::{LayerDraw, draw_layer};
use super::{ContentDrawScope, DrawScope, KEPT_PASS, Picture};
use crate::graphics_context::DefaultShadowColor;
use crate::layout::{
    CompositingStrategy, DefaultCameraDistance, GraphicsLayerScope, LayerBlock, LayerGeometry, TransformOrigin,
};
use crate::platform::{LocalPlatform, PlatformHandle};
use kompoz_graphics::{
    BlendMode, Color, ColorFilter, CornerRadius, Density, ImageBitmap, IntOffset, IntSize, LayoutDirection, Offset,
    Outline, Path, Rect, RenderEffect, RoundRect, Size, StretchEffect,
};
use kompoz_runtime::{kompozable, named, remember};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// A layer's outline as the shape of the layout placed with it.
#[derive(Debug, PartialEq)]
struct OutlineShape(Outline);

impl kompoz_graphics::Shape for OutlineShape {
    fn create_outline(&self, _: Size, _: LayoutDirection, _: &Density) -> Outline {
        self.0.clone()
    }
}

/// What a layer is drawn with (Compose's GraphicsLayer properties), as a graphics layer draws its content: where
/// it goes in the scope it is drawn in, how faded, scaled, turned (about its pivot: its middle unless given) and
/// moved, how high (its outline's shadow), whether it is cut to its outline (its bounds unless given), through what
/// it is composited; and how it is stretched (Android's RenderNode's, which Compose's GraphicsLayer is on Android:
/// see [`GraphicsLayer::stretch`]).
#[derive(Clone, PartialEq, Debug)]
pub struct GraphicsLayerProperties {
    pub top_left: IntOffset,
    pub alpha: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub translation_x: f32,
    pub translation_y: f32,
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    pub camera_distance: f32,
    /// What it scales and turns about, px from its top left (unspecified: its middle).
    pub pivot_offset: Offset,
    pub shadow_elevation: f32,
    pub ambient_shadow_color: Color,
    pub spot_shadow_color: Color,
    pub clip: bool,
    /// Its outline, from its top left (none: its bounds).
    pub outline: Option<Outline>,
    pub render_effect: Option<RenderEffect>,
    pub blend_mode: BlendMode,
    pub color_filter: Option<ColorFilter>,
    pub compositing_strategy: CompositingStrategy,
    /// Its stretch overscroll (none unless set).
    pub stretch: StretchEffect,
}

/// Compose's defaults: where it was recorded, as it was.
impl Default for GraphicsLayerProperties {
    fn default() -> Self {
        GraphicsLayerProperties {
            top_left: IntOffset::Zero,
            alpha: 1.0,
            scale_x: 1.0,
            scale_y: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            camera_distance: DefaultCameraDistance,
            pivot_offset: Offset::Unspecified,
            shadow_elevation: 0.0,
            ambient_shadow_color: DefaultShadowColor,
            spot_shadow_color: DefaultShadowColor,
            clip: false,
            outline: None,
            render_effect: None,
            blend_mode: BlendMode::SrcOver,
            color_filter: None,
            compositing_strategy: CompositingStrategy::Auto,
            stretch: StretchEffect::Empty,
        }
    }
}

struct LayerData {
    picture: Option<Rc<Picture>>,
    size: IntSize,
    properties: GraphicsLayerProperties,
    platform: Option<PlatformHandle>,
    released: bool,
    /// What placing a layout with it does: copies its properties into the layout's layer (made once, kept so
    /// placing again with it isn't taken as a new layer).
    placement_block: Option<LayerBlock>,
    /// Told when a property changes, by the layout placed with it (its layer's properties taken again).
    on_property_changed: Option<Rc<dyn Fn()>>,
}

/// A recording and what it is drawn with (Compose's GraphicsLayer). Clones are the same layer; two are equal when
/// they are. Its properties can change any time: the next `draw_layer` uses them.
#[derive(Clone)]
pub struct GraphicsLayer {
    data: Rc<RefCell<LayerData>>,
}

impl PartialEq for GraphicsLayer {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

impl fmt::Debug for GraphicsLayer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let d = self.data.borrow();
        f.debug_struct("GraphicsLayer").field("size", &d.size).field("properties", &d.properties).finish()
    }
}

macro_rules! properties {
    ($($(#[$doc:meta])* $get:ident, $set:ident: $ty:ty;)*) => {
        $(
            $(#[$doc])*
            pub fn $get(&self) -> $ty {
                self.data.borrow().properties.$get.clone()
            }
            pub fn $set(&self, value: $ty) {
                let changed = {
                    let mut d = self.data.borrow_mut();
                    let changed = d.properties.$get != value;
                    d.properties.$get = value;
                    changed
                };
                if changed {
                    self.property_changed();
                }
            }
        )*
    };
}

impl GraphicsLayer {
    /// A layer with nothing recorded, turned into pixels by `platform`'s renderer (none: it can't be).
    pub fn new(platform: Option<PlatformHandle>) -> GraphicsLayer {
        GraphicsLayer {
            data: Rc::new(RefCell::new(LayerData {
                picture: None,
                size: IntSize::Zero,
                properties: GraphicsLayerProperties::default(),
                platform,
                released: false,
                placement_block: None,
                on_property_changed: None,
            })),
        }
    }

    properties! {
        /// Where `draw_layer` puts it in its scope.
        top_left, set_top_left: IntOffset;
        alpha, set_alpha: f32;
        scale_x, set_scale_x: f32;
        scale_y, set_scale_y: f32;
        translation_x, set_translation_x: f32;
        translation_y, set_translation_y: f32;
        rotation_x, set_rotation_x: f32;
        rotation_y, set_rotation_y: f32;
        rotation_z, set_rotation_z: f32;
        camera_distance, set_camera_distance: f32;
        /// What it scales and turns about, px from its top left (unspecified: its middle).
        pivot_offset, set_pivot_offset: Offset;
        shadow_elevation, set_shadow_elevation: f32;
        ambient_shadow_color, set_ambient_shadow_color: Color;
        spot_shadow_color, set_spot_shadow_color: Color;
        /// Whether it is cut to its outline.
        clip, set_clip: bool;
        render_effect, set_render_effect: Option<RenderEffect>;
        blend_mode, set_blend_mode: BlendMode;
        color_filter, set_color_filter: Option<ColorFilter>;
        compositing_strategy, set_compositing_strategy: CompositingStrategy;
    }

    /// All its properties.
    pub fn properties(&self) -> GraphicsLayerProperties {
        self.data.borrow().properties.clone()
    }

    pub fn set_properties(&self, properties: GraphicsLayerProperties) {
        self.data.borrow_mut().properties = properties;
        self.property_changed();
    }

    /// Tells the layout placed with it that a property changed.
    fn property_changed(&self) {
        let listener = self.data.borrow().on_property_changed.clone();
        if let Some(listener) = listener {
            listener();
        }
    }

    /// The layer block that places a layout through this layer (what `PlacementScope::place_with_graphics_layer`
    /// gives): its properties are the layout layer's, kept up to date as they change, and what the layout draws is
    /// recorded into it.
    pub(crate) fn placement_block(&self) -> LayerBlock {
        if let Some(block) = self.data.borrow().placement_block.clone() {
            return block;
        }
        let weak = Rc::downgrade(&self.data);
        let block: LayerBlock = Rc::new(move |scope: &mut GraphicsLayerScope| {
            if let Some(data) = weak.upgrade() {
                GraphicsLayer { data }.apply_to(scope);
            }
        });
        self.data.borrow_mut().placement_block = Some(block.clone());
        block
    }

    /// Its properties as a layout's layer has them, and itself as what that layer records into.
    fn apply_to(&self, scope: &mut GraphicsLayerScope) {
        let p = self.properties();
        scope.alpha = p.alpha;
        scope.scale_x = p.scale_x;
        scope.scale_y = p.scale_y;
        scope.translation_x = p.translation_x;
        scope.translation_y = p.translation_y;
        scope.rotation_x = p.rotation_x;
        scope.rotation_y = p.rotation_y;
        scope.rotation_z = p.rotation_z;
        scope.camera_distance = p.camera_distance;
        if p.pivot_offset.is_specified() && scope.size.width > 0.0 && scope.size.height > 0.0 {
            scope.transform_origin =
                TransformOrigin::new(p.pivot_offset.x / scope.size.width, p.pivot_offset.y / scope.size.height);
        }
        scope.shadow_elevation = p.shadow_elevation;
        scope.ambient_shadow_color = p.ambient_shadow_color;
        scope.spot_shadow_color = p.spot_shadow_color;
        scope.clip = p.clip;
        if let Some(outline) = p.outline {
            scope.shape = Arc::new(OutlineShape(outline));
        }
        scope.render_effect = p.render_effect;
        scope.blend_mode = p.blend_mode;
        scope.color_filter = p.color_filter;
        scope.compositing_strategy = p.compositing_strategy;
        scope.record_into = Some(self.clone());
    }

    /// Who hears its properties change: the layout placed with it.
    pub(crate) fn set_on_property_changed(&self, listener: Option<Rc<dyn Fn()>>) {
        self.data.borrow_mut().on_property_changed = listener;
    }

    /// Keeps `picture` as its recording, of `size` (a layout placed with it drew).
    pub(crate) fn keep_recording(&self, picture: Picture, size: IntSize) {
        self.keep(picture, size);
    }

    /// Its stretch (none unless [`stretch`](GraphicsLayer::stretch) set one).
    pub fn stretch_effect(&self) -> StretchEffect {
        self.data.borrow().properties.stretch
    }

    /// Stretches it as Android's EdgeEffect stretches the RenderNode it draws into (RenderNode.stretch, @hide on
    /// Android, which foundation's stretch overscroll reaches through EdgeEffect.draw): `vec_x` and `vec_y` (each a
    /// fraction of its size, -1 to 1: positive pulled from the left or top, negative from the right or bottom) added
    /// to its stretch so far ([`StretchEffect::merge_with`]: the stretches of a frame's edges add up), at most
    /// `max_stretch_amount_x` and `_y` px each way. The renderer warps it as hwui does when it is drawn
    /// ([`GraphicsContext::push_stretch_layer`](crate::graphics_context::GraphicsContext::push_stretch_layer)). True,
    /// as RenderNode's.
    ///
    /// ```ignore
    /// // Each frame, in the overscroll's draw (Compose's StretchOverscrollNode, EdgeEffect.draw's numbers):
    /// layer.clear_stretch();
    /// layer.stretch(0.0, top_stretch, width, height); // pulled down from the top
    /// d.record(&layer, |d| d.draw_content());
    /// d.draw_layer(&layer);
    /// ```
    ///
    /// Panics, as RenderNode throws IllegalArgumentException, for a vector that isn't finite or a most that isn't
    /// more than 0.
    #[track_caller]
    pub fn stretch(&self, vec_x: f32, vec_y: f32, max_stretch_amount_x: f32, max_stretch_amount_y: f32) -> bool {
        if !vec_x.is_finite() {
            panic!("vecX must be a finite, non-NaN value {vec_x}");
        }
        if !vec_y.is_finite() {
            panic!("vecY must be a finite, non-NaN value {vec_y}");
        }
        if max_stretch_amount_x <= 0.0 {
            panic!("The max horizontal stretch amount must be >0, got {max_stretch_amount_x}");
        }
        if max_stretch_amount_y <= 0.0 {
            panic!("The max vertical stretch amount must be >0, got {max_stretch_amount_y}");
        }
        let effect = StretchEffect::new(Offset::new(vec_x, vec_y), max_stretch_amount_x, max_stretch_amount_y);
        self.data.borrow_mut().properties.stretch.merge_with(&effect);
        true
    }

    /// Takes its stretch away (RenderNode.clearStretch, @hide on Android, as Views call it each frame before
    /// EdgeEffect stretches again): false if it had none.
    pub fn clear_stretch(&self) -> bool {
        let stretch = &mut self.data.borrow_mut().properties.stretch;
        if stretch.is_empty() {
            return false;
        }
        stretch.set_empty();
        true
    }

    /// Its outline (none: its bounds).
    pub fn outline(&self) -> Option<Outline> {
        self.data.borrow().properties.outline.clone()
    }

    /// Its outline a rect, from its top left (Compose's setRectOutline).
    pub fn set_rect_outline(&self, top_left: Offset, size: Size) {
        self.data.borrow_mut().properties.outline = Some(Outline::Rectangle(Rect::from_offset_size(top_left, size)));
    }

    /// Its outline a rounded rect (Compose's setRoundRectOutline).
    pub fn set_round_rect_outline(&self, top_left: Offset, size: Size, corner_radius: f32) {
        let rr =
            RoundRect::from_rect_radius(Rect::from_offset_size(top_left, size), CornerRadius::circular(corner_radius));
        self.data.borrow_mut().properties.outline = Some(Outline::Rounded(rr));
    }

    /// Its outline any path (Compose's setPathOutline).
    pub fn set_path_outline(&self, path: Path) {
        self.data.borrow_mut().properties.outline = Some(Outline::Generic(path));
    }

    /// The last recording's size.
    pub fn size(&self) -> IntSize {
        self.data.borrow().size
    }

    /// Nothing recorded yet.
    pub fn is_empty(&self) -> bool {
        self.data.borrow().picture.is_none()
    }

    /// Whether it was let go of (Compose's isReleased): it draws nothing more.
    pub fn is_released(&self) -> bool {
        self.data.borrow().released
    }

    /// Lets go of its recording (Compose's releaseGraphicsLayer).
    pub fn release(&self) {
        let mut d = self.data.borrow_mut();
        d.released = true;
        d.picture = None;
    }

    /// What it recorded last.
    pub fn picture(&self) -> Option<Rc<Picture>> {
        self.data.borrow().picture.clone()
    }

    /// Records what `block` draws (Compose's GraphicsLayer.record): a draw scope of `size` at `density`, laid out
    /// `layout_direction`, from its top left.
    pub fn record(
        &self,
        density: Density,
        layout_direction: LayoutDirection,
        size: IntSize,
        block: impl FnOnce(&mut DrawScope<'_>),
    ) {
        let mut picture = Picture::new();
        {
            let mut scope = DrawScope::at(&mut picture, Offset::Zero, size.to_size(), layout_direction, density);
            let outer = KEPT_PASS.replace(false); // (what is drawn into it isn't what a display list keeps)
            block(&mut scope);
            KEPT_PASS.set(outer);
        }
        self.keep(picture, size);
    }

    fn keep(&self, picture: Picture, size: IntSize) {
        let mut d = self.data.borrow_mut();
        d.picture = Some(Rc::new(picture));
        d.size = size;
        d.released = false;
    }

    /// The recording as pixels, drawn offscreen by the platform from its top left; none where it can't be (nothing
    /// recorded, no platform renderer: tests). Compose's toImageBitmap, here right away.
    pub fn to_image_bitmap(&self) -> Option<ImageBitmap> {
        let (picture, size, platform) = {
            let d = self.data.borrow();
            (d.picture.clone()?, d.size, d.platform.clone()?)
        };
        if size.width <= 0 || size.height <= 0 {
            return None;
        }
        platform.get().render_image(size.to_size(), &|g| picture.replay(g))
    }

    /// What it is drawn through in a scope at `origin`.
    fn layer_draw(&self, origin: Offset) -> Option<(LayerDraw, Rc<Picture>)> {
        let d = self.data.borrow();
        let picture = d.picture.clone()?;
        if d.released {
            return None;
        }
        let p = &d.properties;
        let size = d.size.to_size();
        let bounds = Rect::from_offset_size(origin + p.top_left.to_offset(), size);
        let pivot = if p.pivot_offset.is_specified() { p.pivot_offset } else { size.center() };
        let mut l = LayerDraw::plain(bounds);
        l.geometry = LayerGeometry {
            scale_x: p.scale_x,
            scale_y: p.scale_y,
            translation_x: p.translation_x,
            translation_y: p.translation_y,
            rotation_x: p.rotation_x,
            rotation_y: p.rotation_y,
            rotation_z: p.rotation_z,
            camera_distance: p.camera_distance,
            pivot_x: bounds.left + pivot.x,
            pivot_y: bounds.top + pivot.y,
        };
        l.alpha = p.alpha;
        l.shadow_elevation = p.shadow_elevation;
        l.ambient_shadow_color = p.ambient_shadow_color;
        l.spot_shadow_color = p.spot_shadow_color;
        l.clip = p.clip;
        l.outline = p.outline.clone().unwrap_or(Outline::Rectangle(Rect::from_offset_size(Offset::Zero, size)));
        l.render_effect = p.render_effect.clone();
        l.stretch = p.stretch;
        l.blend_mode = p.blend_mode;
        l.color_filter = p.color_filter;
        l.strategy = p.compositing_strategy;
        Some((l, picture))
    }
}

/// A layer remembered where it is called, turned into pixels by the host's platform (Compose's
/// rememberGraphicsLayer).
#[kompozable]
pub fn remember_graphics_layer() -> GraphicsLayer {
    let platform = LocalPlatform.current();
    remember(move || GraphicsLayer::new(Some(platform)))
}

#[named]
impl<'a> DrawScope<'a> {
    /// Records what `block` draws into `layer` instead of drawing it here (Compose's DrawScope.record): from this
    /// scope's top left, in a size of its own (this scope's by default).
    pub fn record(
        &mut self,
        layer: &GraphicsLayer,
        #[default(IntSize::new(self.size.width.round() as i32, self.size.height.round() as i32))] size: IntSize,
        block: impl FnOnce(&mut DrawScope<'_>),
    ) {
        layer.record(self.density, self.layout_direction, size, block);
    }

    pub fn record_with(&mut self, layer: &GraphicsLayer, size: IntSize, block: impl FnOnce(&mut DrawScope<'_>)) {
        layer.record(self.density, self.layout_direction, size, block);
    }
}

impl<'a> DrawScope<'a> {
    /// Draws what `layer` recorded at its top left, with its properties (Compose's drawLayer); nothing where it
    /// recorded nothing.
    pub fn draw_layer(&mut self, layer: &GraphicsLayer) {
        let Some((l, picture)) = layer.layer_draw(self.origin) else { return };
        let at = l.bounds.top_left();
        draw_layer(&mut *self.canvas, &l, &mut |g| {
            g.push_translation(at);
            picture.replay(g);
            g.pop_translation();
        });
    }
}

#[named]
impl<'a> ContentDrawScope<'a> {
    /// Records what `block` draws into `layer` (Compose's record in a content draw scope): its content too, with
    /// `draw_content`.
    pub fn record(
        &mut self,
        layer: &GraphicsLayer,
        #[default(IntSize::new(self.size().width.round() as i32, self.size().height.round() as i32))] size: IntSize,
        block: impl FnOnce(&mut ContentDrawScope<'_>),
    ) {
        self.record_content(layer, size, block);
    }

    pub fn record_with(&mut self, layer: &GraphicsLayer, size: IntSize, block: impl FnOnce(&mut ContentDrawScope<'_>)) {
        self.record_content(layer, size, block);
    }
}

impl<'a> ContentDrawScope<'a> {
    fn record_content(&mut self, layer: &GraphicsLayer, size: IntSize, block: impl FnOnce(&mut ContentDrawScope<'_>)) {
        let mut picture = Picture::new();
        {
            let (density, direction) = (self.scope.density, self.scope.layout_direction);
            let mut inner = ContentDrawScope {
                scope: DrawScope::at(&mut picture, Offset::Zero, size.to_size(), direction, density),
                next: self.next.clone(),
            };
            let outer = KEPT_PASS.replace(false);
            block(&mut inner);
            KEPT_PASS.set(outer);
        }
        layer.keep(picture, size);
    }
}
