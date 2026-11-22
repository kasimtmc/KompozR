//! A graphics layer's properties (Compose's GraphicsLayerScope) and the matrix it is drawn and hit through, as
//! Android's RenderNode makes it (C++'s `LayerGeometry`/`layerMatrix`).

use kompoz_graphics::{
    BlendMode, Color, ColorFilter, Density, Dp, LayoutDirection, Matrix, Outline, RectangleShape, RenderEffect, Shape,
    Size,
};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// Where a layer is scaled and turned about, as fractions of its size (Compose's TransformOrigin).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TransformOrigin {
    pub pivot_fraction_x: f32,
    pub pivot_fraction_y: f32,
}

impl TransformOrigin {
    pub const Center: TransformOrigin = TransformOrigin { pivot_fraction_x: 0.5, pivot_fraction_y: 0.5 };
    pub const fn new(pivot_fraction_x: f32, pivot_fraction_y: f32) -> TransformOrigin {
        TransformOrigin { pivot_fraction_x, pivot_fraction_y }
    }
}

/// Compose's constructor `TransformOrigin(pivotFractionX, pivotFractionY)`.
pub const fn TransformOrigin(pivot_fraction_x: f32, pivot_fraction_y: f32) -> TransformOrigin {
    TransformOrigin::new(pivot_fraction_x, pivot_fraction_y)
}

impl Default for TransformOrigin {
    fn default() -> Self {
        TransformOrigin::Center
    }
}

/// Whether a layer draws offscreen first (Compose's CompositingStrategy): when it has to (Auto), always
/// (Offscreen), or never, its alpha applied to each draw (ModulateAlpha).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CompositingStrategy {
    #[default]
    Auto,
    Offscreen,
    ModulateAlpha,
}

/// How far a layer drawn offscreen reaches past its bounds, each side (Compose's LayerOutsets): room for what an
/// effect or a drawing past the bounds puts there.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct LayerOutsets {
    pub left: Dp,
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
}

impl LayerOutsets {
    /// None on any side.
    pub const Zero: LayerOutsets = LayerOutsets::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0));

    /// Each side its own. Panics, as Compose throws, for a negative one.
    #[track_caller]
    pub const fn new(left: Dp, top: Dp, right: Dp, bottom: Dp) -> LayerOutsets {
        assert!(
            left.0 >= 0.0 && top.0 >= 0.0 && right.0 >= 0.0 && bottom.0 >= 0.0,
            "Layer outsets must be non-negative"
        );
        LayerOutsets { left, top, right, bottom }
    }
}

/// Compose's DefaultCameraDistance: 8 (inches of 72 px, as Android takes it).
pub const DefaultCameraDistance: f32 = 8.0;

/// What a graphics layer block sets (Compose's GraphicsLayerScope): how the layer is scaled, turned, moved, faded,
/// shadowed and clipped, and through what it is composited. `size`, `density` and `layout_direction` are the
/// layer's, to set the others by. Its properties are Kotlin's (`scaleX = 2f` in a `graphicsLayer { }` block).
#[derive(Clone, kompoz_runtime::KotlinProperties)]
#[kotlin(set)]
pub struct GraphicsLayerScope {
    pub scale_x: f32,
    pub scale_y: f32,
    pub alpha: f32,
    /// Moved by this many px, after scaling and turning.
    pub translation_x: f32,
    pub translation_y: f32,
    /// How high it is above what is under it, in px (its elevation shadow).
    pub shadow_elevation: f32,
    pub ambient_shadow_color: Color,
    pub spot_shadow_color: Color,
    /// Degrees about the x axis (its top coming towards the viewer as it grows), the y axis, and the z axis
    /// (clockwise on the screen).
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    /// How far the camera is for the 3D turns, in Android's units (inches of 72 px).
    pub camera_distance: f32,
    pub transform_origin: TransformOrigin,
    /// Its outline (for its clip and its shadow).
    pub shape: Arc<dyn Shape>,
    /// Whether what it draws is cut to its shape.
    pub clip: bool,
    pub render_effect: Option<RenderEffect>,
    pub compositing_strategy: CompositingStrategy,
    pub blend_mode: BlendMode,
    pub color_filter: Option<ColorFilter>,
    /// How far it reaches past its bounds when it is drawn offscreen.
    pub outsets: LayerOutsets,
    #[kotlin(skip)]
    pub(crate) size: Size,
    #[kotlin(skip)]
    pub(crate) density: Density,
    #[kotlin(skip)]
    pub(crate) layout_direction: LayoutDirection,
    /// The graphics layer it was placed with (Compose's `placeWithLayer(offset, layer)`): what is drawn through it
    /// is recorded into that layer too, which can draw it again (a lazy list's item fading out after it left).
    #[kotlin(skip)]
    pub(crate) record_into: Option<crate::draw::GraphicsLayer>,
}

impl Default for GraphicsLayerScope {
    fn default() -> Self {
        GraphicsLayerScope {
            scale_x: 1.0,
            scale_y: 1.0,
            alpha: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            shadow_elevation: 0.0,
            ambient_shadow_color: Color::Black,
            spot_shadow_color: Color::Black,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            camera_distance: DefaultCameraDistance,
            transform_origin: TransformOrigin::Center,
            shape: Arc::new(RectangleShape),
            clip: false,
            render_effect: None,
            compositing_strategy: CompositingStrategy::Auto,
            blend_mode: BlendMode::SrcOver,
            color_filter: None,
            outsets: LayerOutsets::Zero,
            size: Size::Zero,
            density: Density::default(),
            layout_direction: LayoutDirection::Ltr,
            record_into: None,
        }
    }
}

/// A layer's scope is the density it is worked out in (Compose's `GraphicsLayerScope : Density`): `to_px(8.dp())`
/// in a layer block, Kotlin's `8.dp.toPx()`.
impl std::ops::Deref for GraphicsLayerScope {
    type Target = Density;

    fn deref(&self) -> &Density {
        &self.density
    }
}

impl fmt::Debug for GraphicsLayerScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GraphicsLayerScope")
            .field("scale", &(self.scale_x, self.scale_y))
            .field("alpha", &self.alpha)
            .field("translation", &(self.translation_x, self.translation_y))
            .field("rotation", &(self.rotation_x, self.rotation_y, self.rotation_z))
            .field("shadow_elevation", &self.shadow_elevation)
            .field("clip", &self.clip)
            .field("shape", &self.shape)
            .finish()
    }
}

impl GraphicsLayerScope {
    /// A scope for a layer of `size`, its properties at their defaults.
    pub fn new(size: Size, density: Density, layout_direction: LayoutDirection) -> GraphicsLayerScope {
        GraphicsLayerScope { size, density, layout_direction, ..Default::default() }
    }
    /// The layer's size in px.
    pub fn size(&self) -> Size {
        self.size
    }
    pub fn density(&self) -> Density {
        self.density
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }

    /// The matrix the layer is drawn and hit through: scaled about its pivot, turned (in 3D seen from the camera, as
    /// Android's RenderNode), then moved.
    pub fn matrix(&self) -> Matrix {
        let pivot_x = self.transform_origin.pivot_fraction_x * self.size.width;
        let pivot_y = self.transform_origin.pivot_fraction_y * self.size.height;
        layer_matrix(&LayerGeometry {
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            translation_x: self.translation_x,
            translation_y: self.translation_y,
            rotation_x: self.rotation_x,
            rotation_y: self.rotation_y,
            rotation_z: self.rotation_z,
            camera_distance: self.camera_distance,
            pivot_x,
            pivot_y,
        })
    }

    /// Its outline (its shape in its size).
    pub fn outline(&self) -> Outline {
        self.shape.create_outline(self.size, self.layout_direction, &self.density)
    }

    /// Whether its matrix moves anything: the positional properties aren't all at their defaults.
    pub(crate) fn has_positional_properties(&self) -> bool {
        self.scale_x != 1.0
            || self.scale_y != 1.0
            || self.translation_x != 0.0
            || self.translation_y != 0.0
            || self.rotation_x != 0.0
            || self.rotation_y != 0.0
            || self.rotation_z != 0.0
    }

    /// Whether the positional properties (what moves points: the matrix and the clip) equal `other`'s (Compose's
    /// LayerPositionalProperties).
    pub(crate) fn same_positional_properties(&self, other: &GraphicsLayerScope) -> bool {
        self.scale_x == other.scale_x
            && self.scale_y == other.scale_y
            && self.translation_x == other.translation_x
            && self.translation_y == other.translation_y
            && self.rotation_x == other.rotation_x
            && self.rotation_y == other.rotation_y
            && self.rotation_z == other.rotation_z
            && self.camera_distance == other.camera_distance
            && self.transform_origin == other.transform_origin
    }

    /// Whether everything that shows equals `other`'s.
    pub(crate) fn same_as(&self, other: &GraphicsLayerScope) -> bool {
        self.same_positional_properties(other)
            && self.alpha == other.alpha
            && self.shadow_elevation == other.shadow_elevation
            && self.ambient_shadow_color == other.ambient_shadow_color
            && self.spot_shadow_color == other.spot_shadow_color
            && *self.shape == *other.shape
            && self.clip == other.clip
            && self.render_effect == other.render_effect
            && self.compositing_strategy == other.compositing_strategy
            && self.blend_mode == other.blend_mode
            && self.color_filter == other.color_filter
            && self.outsets == other.outsets
            && self.size == other.size
            && self.record_into == other.record_into
    }
}

/// A graphics layer block, as placement takes it.
pub type LayerBlock = Rc<dyn Fn(&mut GraphicsLayerScope)>;

/// What a layer's matrix is made from (C++'s LayerGeometry), in px; the pivot where it is in what the layer is
/// drawn in.
#[derive(Clone, Copy, PartialEq, Debug)]
#[doc(hidden)]
pub struct LayerGeometry {
    pub scale_x: f32,
    pub scale_y: f32,
    pub translation_x: f32,
    pub translation_y: f32,
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    pub camera_distance: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
}

impl Default for LayerGeometry {
    fn default() -> Self {
        LayerGeometry {
            scale_x: 1.0,
            scale_y: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            camera_distance: DefaultCameraDistance,
            pivot_x: 0.0,
            pivot_y: 0.0,
        }
    }
}

/// The 3x3 of a matrix's plane part, row by row.
pub(crate) type Plane = [f32; 9];

/// `b`, then `a`.
pub(crate) fn mul(a: &Plane, b: &Plane) -> Plane {
    let mut out = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            out[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
        }
    }
    out
}

fn moving(x: f32, y: f32) -> Plane {
    [1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0]
}

fn matrix_of(p: &Plane) -> Matrix {
    let mut m = Matrix::new();
    m.values[Matrix::ScaleX] = p[0];
    m.values[Matrix::SkewX] = p[1];
    m.values[Matrix::TranslateX] = p[2];
    m.values[Matrix::SkewY] = p[3];
    m.values[Matrix::ScaleY] = p[4];
    m.values[Matrix::TranslateY] = p[5];
    m.values[Matrix::Perspective0] = p[6];
    m.values[Matrix::Perspective1] = p[7];
    m.values[Matrix::Perspective2] = p[8];
    m
}

/// A rotation about a unit axis (Skia's SkM44::Rotate), 3x3.
pub(crate) fn rotation(x: f32, y: f32, z: f32, degrees: f32) -> Plane {
    let r = (degrees as f64).to_radians();
    let (c, s) = (r.cos() as f32, r.sin() as f32);
    let t = 1.0 - c;
    [
        t * x * x + c,
        t * x * y - s * z,
        t * x * z + s * y,
        t * x * y + s * z,
        t * y * y + c,
        t * y * z - s * x,
        t * x * z - s * y,
        t * y * z + s * x,
        t * z * z + c,
    ]
}

/// The matrix a layer is drawn through, as Android's RenderNode makes it (C++'s `layerMatrix`): scaled about the
/// pivot; turned about z in the plane, then moved; or, turned in 3D, seen through Sk3DView's camera at 72 ×
/// camera distance px.
pub(crate) fn layer_matrix(g: &LayerGeometry) -> Matrix {
    let (px, py) = (g.pivot_x, g.pivot_y);
    let scale =
        mul(&moving(px, py), &mul(&[g.scale_x, 0.0, 0.0, 0.0, g.scale_y, 0.0, 0.0, 0.0, 1.0], &moving(-px, -py)));
    if g.rotation_x == 0.0 && g.rotation_y == 0.0 {
        let r = (g.rotation_z as f64).to_radians();
        let (c, s) = (r.cos() as f32, r.sin() as f32);
        let turn = mul(&moving(px, py), &mul(&[c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0], &moving(-px, -py)));
        return matrix_of(&mul(&moving(g.translation_x, g.translation_y), &mul(&turn, &scale)));
    }
    // Sk3DView: turned about x, then (0, -1, 0), then z the other way (applied z first), the drawing's plane (x
    // across, y up: its patch) seen from (0, 0, -72 * cameraDistance) px.
    let m = mul(
        &mul(&rotation(1.0, 0.0, 0.0, g.rotation_x), &rotation(0.0, -1.0, 0.0, g.rotation_y)),
        &rotation(0.0, 0.0, 1.0, -g.rotation_z),
    );
    let u = [m[0], m[3], m[6]];
    let v = [-m[1], -m[4], -m[7]];
    let k = 1.0 / (72.0 * g.camera_distance.max(1e-3));
    let camera = [u[0], v[0], 0.0, -u[1], -v[1], 0.0, u[2] * k, v[2] * k, 1.0];
    let placed = mul(&moving(px + g.translation_x, py + g.translation_y), &mul(&camera, &moving(-px, -py)));
    matrix_of(&mul(&placed, &scale))
}
