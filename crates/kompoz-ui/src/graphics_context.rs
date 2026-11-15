//! The drawing surface a platform backend provides (C++'s GraphicsContext): rounded rects, lines, arcs, text,
//! images, paths and meshes; layers, clips, translations, transforms, blend modes and color filters; shadows of
//! rounded rects, of paths and of elevated outlines; and the hooks a recording canvas lines frames up by. Shapes are
//! drawn through it by their outlines. Lengths are pixels.

use kompoz_graphics::{
    BlendMode, BlurEffect, Brush, Color, ColorFilter, CornerRadius, Density, FilterQuality, ImageBitmap,
    LayoutDirection, Matrix, Offset, Outline, Path, PathCommand, PathDirection, PathStroke, Rect, RenderEffect,
    RoundRect, Shape, Size, SolidColor, StrokeJoin, TileMode, Vertices,
};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

// ---- Values ----------------------------------------------------------------------------------------------------

/// Each corner's radius as drawn (left and right on the screen, px): a rounded rect whose corners are circles of
/// their own. One radius converts to four equal ones.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    /// No corner rounded.
    pub const Zero: CornerRadii = CornerRadii::all(0.0);

    /// Each corner its own.
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> CornerRadii {
        CornerRadii { top_left, top_right, bottom_right, bottom_left }
    }
    /// One radius for all four.
    pub const fn all(radius: f32) -> CornerRadii {
        CornerRadii::new(radius, radius, radius, radius)
    }
    /// All four the same.
    pub fn uniform(self) -> bool {
        self.top_left == self.top_right && self.top_right == self.bottom_right && self.bottom_right == self.bottom_left
    }
    /// The largest.
    pub fn max(self) -> f32 {
        self.top_left.max(self.top_right).max(self.bottom_right.max(self.bottom_left))
    }
    /// Each `k` times as big.
    pub fn scaled(self, k: f32) -> CornerRadii {
        CornerRadii::new(self.top_left * k, self.top_right * k, self.bottom_right * k, self.bottom_left * k)
    }
    /// Each `d` bigger.
    pub fn grown(self, d: f32) -> CornerRadii {
        CornerRadii::new(self.top_left + d, self.top_right + d, self.bottom_right + d, self.bottom_left + d)
    }
}

/// [`CornerRadii::all`].
impl From<f32> for CornerRadii {
    fn from(radius: f32) -> CornerRadii {
        CornerRadii::all(radius)
    }
}

/// How a clip clips (Compose's ClipOp): to what is inside its shape, or to what is outside it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ClipOp {
    #[default]
    Intersect,
    Difference,
}

/// How a layer is composited (graphicsLayer): scaled and rotated around `origin` (a fraction of its size), then
/// moved.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LayerTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    /// Degrees, clockwise.
    pub rotation_z: f32,
    /// Px.
    pub translation_x: f32,
    pub translation_y: f32,
    pub origin: Offset,
}

impl LayerTransform {
    /// Nothing done, about the middle.
    pub const Identity: LayerTransform = LayerTransform {
        scale_x: 1.0,
        scale_y: 1.0,
        rotation_z: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        origin: Offset::new(0.5, 0.5),
    };

    /// Whether it leaves the layer where it is (whatever its origin).
    pub fn is_identity(&self) -> bool {
        self.scale_x == 1.0
            && self.scale_y == 1.0
            && self.rotation_z == 0.0
            && self.translation_x == 0.0
            && self.translation_y == 0.0
    }
}

/// [`LayerTransform::Identity`].
impl Default for LayerTransform {
    fn default() -> LayerTransform {
        LayerTransform::Identity
    }
}

/// A shadow (Compose's Shadow, for text and, with spread, for dropShadow/innerShadow): its color, how far it is
/// moved and how much it is blurred (px), and how much bigger (a drop shadow) or smaller (an inner one's hole) the
/// shape is.
#[derive(Clone, PartialEq, Debug)]
pub struct Shadow {
    pub color: Color,
    pub offset: Offset,
    pub blur_radius: f32,
    /// Not for text.
    pub spread: f32,
    /// Not for text either (Compose 1.9's dropShadow and innerShadow): a brush painting it instead of the color
    /// (spread over what it covers).
    pub brush: Option<Brush>,
    /// Its alpha.
    pub alpha: f32,
    /// How it goes onto what is under it.
    pub blend_mode: BlendMode,
}

impl Shadow {
    /// One of `color`, moved, blurred and spread, painted by no brush, opaque, over what is under it.
    pub fn new(color: Color, offset: Offset, blur_radius: f32, spread: f32) -> Shadow {
        Shadow { color, offset, blur_radius, spread, ..Shadow::default() }
    }
}

/// Black, where it is, sharp.
impl Default for Shadow {
    fn default() -> Shadow {
        Shadow {
            color: DefaultShadowColor,
            offset: Offset::Zero,
            blur_radius: 0.0,
            spread: 0.0,
            brush: None,
            alpha: 1.0,
            blend_mode: BlendMode::SrcOver,
        }
    }
}

/// Where elevation shadows' light is, as Android's theme has it: above the window's top middle (`center`, the
/// window's px), `z` px up, `radius` px across, the ambient and spot shadows at those alphas. The platform sets the
/// window's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ShadowLight {
    pub center: Offset,
    pub z: f32,
    pub radius: f32,
    pub ambient_alpha: f32,
    pub spot_alpha: f32,
}

/// Android's theme's, over the origin.
impl Default for ShadowLight {
    fn default() -> ShadowLight {
        ShadowLight { center: Offset::Zero, z: 500.0, radius: 800.0, ambient_alpha: 0.039, spot_alpha: 0.19 }
    }
}

/// An elevation shadow (graphicsLayer's shadowElevation): how high its caster is at each point (z = plane[0] x +
/// plane[1] y + plane[2], px, x and y the drawing's px: flat but for a layer turned in 3D), its two colors (at their
/// alphas times the light's), and the caster's alpha (below 1 it is seen through, so its shadow shows under it
/// too).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ElevationShadow {
    pub plane: [f32; 3],
    pub ambient_color: Color,
    pub spot_color: Color,
    pub alpha: f32,
}

/// Nowhere up, black, opaque.
impl Default for ElevationShadow {
    fn default() -> ElevationShadow {
        ElevationShadow {
            plane: [0.0; 3],
            ambient_color: DefaultShadowColor,
            spot_color: DefaultShadowColor,
            alpha: 1.0,
        }
    }
}

impl ElevationShadow {
    /// A caster `elevation` px up everywhere, black, opaque.
    pub fn flat(elevation: f32) -> ElevationShadow {
        ElevationShadow { plane: [0.0, 0.0, elevation], ..ElevationShadow::default() }
    }

    /// Where it may reach for a caster over `bounds` (px), the light's center there `light_center` (as Skia's
    /// SkDrawShadowMetrics::GetLocalBounds works it out, before any transform).
    pub fn reach(&self, bounds: Rect, light: &ShadowLight, light_center: Offset) -> Rect {
        let at = |x: f32, y: f32| self.plane[0] * x + self.plane[1] * y + self.plane[2];
        let z = at(bounds.left, bounds.top)
            .max(at(bounds.right, bounds.top))
            .max(at(bounds.left, bounds.bottom))
            .max(at(bounds.right, bounds.bottom));
        let lz = light.z;
        let ambient = (z / 2.0).min(150.0);
        let mut out = bounds.inflate(ambient);
        if lz > z && z > 0.0 {
            let ratio = (z / (lz - z)).clamp(0.0, 0.95);
            let scale = (lz / (lz - z)).clamp(1.0, 1.95);
            let blur = light.radius * ratio;
            let spot = Rect::new(
                bounds.left * scale - ratio * light_center.x - blur,
                bounds.top * scale - ratio * light_center.y - blur,
                bounds.right * scale - ratio * light_center.x + blur,
                bounds.bottom * scale - ratio * light_center.y + blur,
            );
            out = out.union(spot);
        }
        out.inflate(1.0)
    }
}

/// Compose's default shadow color.
pub const DefaultShadowColor: Color = Color::Black;

/// What a path's shadow ([`GraphicsContext::draw_shadow_path`]) can draw on, the path's bounds at `bounds` (where
/// it is drawn): a drop shadow's blur around the path spread and moved, an inner one's within the path (2 px past
/// its bounds).
pub fn shadow_path_reach(bounds: Rect, shadow: &Shadow, inner: bool) -> Rect {
    if inner {
        return bounds.inflate(2.0);
    }
    let blur = shadow.blur_radius.max(0.0);
    let grow = shadow.spread.max(0.0) * PathStroke::DefaultMiter;
    let at = bounds.translate(shadow.offset).inflate(grow);
    BlurEffect(blur, blur, TileMode::Decal).output_bounds(at)
}

/// What a canvas that keeps what nodes drew from one frame to the next does with a node about to be drawn
/// ([`GraphicsContext::record_node`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum NodeRecording {
    /// It is drawn, but not into what is kept (any other canvas, or a node drawn a second time in a frame).
    #[default]
    Other,
    /// It is drawn, into the frame kept.
    Fresh,
    /// The canvas drew again what it drew last frame; it draws nothing.
    Kept,
}

/// What is drawn into a layer just pushed ([`GraphicsContext::layer_content`]), where the canvas keeps layers'
/// pixels from one frame to the next.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum LayerContent {
    /// As usual.
    #[default]
    Draw,
    /// All of it, even what is outside the area replayed (the canvas keeps it).
    DrawAll,
    /// Nothing: the canvas has the pixels already.
    Kept,
}

/// A run of text to draw (C++'s utf8 and Font): its text, where its baseline starts, its font size (px), its
/// advance width and line height, its font's ascent and descent (C++'s FontMetrics, which a display list measures
/// what it touches by), whether its paragraph goes right to left (which orders its stretches of either direction;
/// either way the text starts at `baseline.x` and goes right), and what the text stage shaped it into (opaque here).
#[derive(Clone)]
pub struct TextDraw {
    pub text: Rc<str>,
    pub baseline: Offset,
    /// Px.
    pub font_size: f32,
    /// The run's advance width and line height.
    pub size: Size,
    /// How far its glyphs reach above and below the baseline (px; accents, italics and the like aside).
    pub ascent: f32,
    pub descent: f32,
    pub rtl: bool,
    /// An opaque shaped payload the text stage fills in.
    pub shaped: Option<Rc<dyn Any>>,
}

impl TextDraw {
    /// Left to right, not shaped yet; its metrics as far as fonts' glyphs mostly reach (an em above the baseline,
    /// 0.3 em below) until told ([`with_metrics`](TextDraw::with_metrics)).
    pub fn new(text: impl Into<Rc<str>>, baseline: Offset, font_size: f32, size: Size) -> TextDraw {
        TextDraw {
            text: text.into(),
            baseline,
            font_size,
            size,
            ascent: font_size,
            descent: 0.3 * font_size,
            rtl: false,
            shaped: None,
        }
    }

    /// The same, its font's ascent and descent these.
    pub fn with_metrics(self, ascent: f32, descent: f32) -> TextDraw {
        TextDraw { ascent, descent, ..self }
    }
}

/// The same text, placed, sized and measured the same, the same way round, shaped into the same payload (or neither
/// shaped).
impl PartialEq for TextDraw {
    fn eq(&self, o: &TextDraw) -> bool {
        let same_shaped = match (&self.shaped, &o.shaped) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        self.text == o.text
            && self.baseline == o.baseline
            && self.font_size == o.font_size
            && self.size == o.size
            && self.ascent == o.ascent
            && self.descent == o.descent
            && self.rtl == o.rtl
            && same_shaped
    }
}

/// Its text, baseline and size.
impl fmt::Debug for TextDraw {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TextDraw")
            .field("text", &self.text)
            .field("baseline", &self.baseline)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

// ---- The drawing surface ---------------------------------------------------------------------------------------

/// The minimal drawing surface a platform backend has to provide (app code draws with a DrawScope). Lengths are px;
/// what it provides itself it draws through what the backend does (as C++'s defaults).
pub trait GraphicsContext {
    /// A rounded rect filled, each corner a circle of its own radius.
    fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color);
    /// A rounded rect's outline, `width` px wide.
    fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, width: f32, color: Color);
    /// A line `width` px wide, its ends square (half its width past them).
    fn draw_line(&mut self, from: Offset, to: Offset, width: f32, color: Color);
    /// A run of text from its baseline, in `color`.
    fn draw_text(&mut self, text: &TextDraw, color: Color);
    /// Text painted with `brush` spread over `brush_area` (a gradient across a paragraph). The default paints it in
    /// the color at the area's center.
    fn draw_text_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        self.draw_text(text, brush.color_at(brush_area.center(), brush_area));
    }
    /// Text's shadow: its glyphs (not color emoji) in `color`, blurred by `blur_radius` px. The default draws them
    /// sharp.
    fn draw_text_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        let _ = blur_radius;
        self.draw_text(text, color);
    }
    /// Strokes part of the circle inscribed in `oval`. Angles in degrees, 0 = 3 o'clock, clockwise.
    fn draw_arc(
        &mut self,
        oval: Rect,
        start_degrees: f32,
        sweep_degrees: f32,
        stroke_width: f32,
        color: Color,
        round_cap: bool,
    );
    /// Draws the `src` pixel rect of `image` into `dst`, sampled as `quality` says (DefaultFilterQuality unless
    /// told otherwise).
    fn draw_image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality);
    /// An image as a mask (an Alpha8 bitmap, as Android draws one): `brush` (spread over `brush_area`) where the
    /// `src` pixels of `image` go in `dst`, as opaque as they are, faded by `alpha`. The default draws the brush in a
    /// layer and the image over it by DstIn.
    #[allow(clippy::too_many_arguments)]
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
        default_draw_image_mask(self, image, src, dst, brush, brush_area, alpha, quality);
    }
    /// Everything drawn until pop_layer() goes to an offscreen layer that is then composited into `bounds`, clipped
    /// to a rounded rect of `radii`, faded by `alpha` and transformed by `transform` (LayerTransform::Identity
    /// unless told otherwise).
    fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform);
    /// Ends the layer last pushed (push_layer, push_effect_layer, push_outline_layer, push_shape_layer).
    fn pop_layer(&mut self);
    /// Draws until pop_clip() only inside `rect`, within the clips already set (Compose's Canvas.clipRect).
    fn push_clip(&mut self, rect: Rect);
    /// Draws until pop_clip() only inside a rounded rect, or with ClipOp::Difference only outside it; within the
    /// clips already set, their edges anti-aliased (Compose's Canvas.clipRect and clipRoundRect).
    fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp);
    /// The same inside (or outside) a path placed at `origin`, by its fill type (Compose's Canvas.clipPath).
    fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp);
    /// Ends the clip last pushed.
    fn pop_clip(&mut self);
    /// Everything drawn until pop_translation() moves by `by` px, without a layer: clips set before stay where they
    /// are, those set inside move along.
    fn push_translation(&mut self, by: Offset);
    /// Ends the translation last pushed.
    fn pop_translation(&mut self);
    /// Everything drawn until pop_transform() goes through `matrix` (px to px, from the drawing's own coordinates to
    /// those outside it), as through Compose's Canvas.concat: clips set before stay where they are, those set inside
    /// are transformed with what they clip. Nothing is clipped to where the drawing was.
    fn push_transform(&mut self, matrix: &Matrix);
    /// Ends the transform last pushed.
    fn pop_transform(&mut self);
    /// A layer (pop_layer ends it) whose content, drawn within `bounds` and cut to them, goes through `effect` as it
    /// is composited (blurred, moved, as far past the bounds as that takes it), faded by `alpha`; clips set around
    /// it cut what shows. The default composites the content as it is.
    fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        let _ = effect;
        self.push_layer(bounds, CornerRadii::Zero, alpha, LayerTransform::Identity);
    }
    /// What is drawn so far under `bounds` (into the same target: in an offscreen layer, into the layer), and around
    /// them as far as `effect` reaches (past that as its edge treatment says), through the effect, drawn back in
    /// place of it, mixed with it by `alpha`: a backdrop blur (frosted glass). Clips set around it cut what shows.
    /// The default draws nothing.
    fn draw_backdrop(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        let _ = (bounds, effect, alpha);
    }
    /// Draws in between go on with `mode` instead of SrcOver (where the backend can: see DrawScope's blendMode).
    fn push_blend_mode(&mut self, mode: BlendMode) {
        let _ = mode;
    }
    /// Ends the blend mode last pushed.
    fn pop_blend_mode(&mut self) {}
    /// Draws in between not anti-aliased (`on` false: a Paint's isAntiAlias): each pixel all in or all out, as Skia
    /// decides it: its middle inside the shape (a path's winding there), a hairline a pixel each step along it
    /// (where the backend can; the default draws as it is).
    fn push_anti_alias(&mut self, on: bool) {
        let _ = on;
    }
    /// Ends the anti-aliasing last pushed.
    fn pop_anti_alias(&mut self) {}
    /// Draws in between have their colors (their paint's, an image's pixels) through `filter` before they go on, an
    /// inner filter's first (Compose's ColorFilter on a Paint, an Image, an Icon). Backends apply it; one that draws
    /// nothing itself records it.
    fn push_color_filter(&mut self, filter: &ColorFilter);
    /// Ends the color filter last pushed.
    fn pop_color_filter(&mut self);
    /// True if nothing drawn inside `rect` could show: it lies wholly outside the current clip (or layer). Content
    /// that clips itself to such a rect isn't drawn at all.
    fn quick_reject(&self, rect: Rect) -> bool {
        let _ = rect;
        false
    }
    /// Fills a rounded rect with `brush`, its gradient spread over `brush_area`. The default paints the color at the
    /// rect's center; backends interpolate the gradient.
    fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
        self.fill_round_rect(rect, radii, brush.color_at(rect.center(), brush_area));
    }
    /// A triangle mesh (Compose's Canvas.drawVertices, Skia's): its triangles placed at `origin`, filled without
    /// anti-aliasing, as Skia fills them; each pixel the brush (spread over `brush_area`, as everywhere) where its
    /// texture coordinate is (from `origin` too; none: where the pixel is), or the brush's color made opaque; that
    /// through the vertex colors (worked out between them, premultiplied) by `blend_mode`, the brush the source and
    /// the colors the destination (no colors: the brush as it is); then faded by `alpha`. Triangles that overlap
    /// each go on over the ones before.
    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        origin: Offset,
        blend_mode: BlendMode,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
    );
    /// Fills (a stroke that doesn't stroke) or strokes `path` placed at `origin`, with `brush` spread over
    /// `brush_area`.
    fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke);
    /// A rounded rect's shadow: under it (spread out, moved, blurred), or inside it (`inner`: the color where a hole
    /// of the rect shrunk by spread and moved isn't, blurred, inside the rect). The default draws a drop shadow as
    /// rings fading out, and no inner shadow.
    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
        let alpha = shadow.color.alpha();
        if inner || alpha <= 0.0 {
            return;
        }
        for i in (1..=6).rev() {
            let t = i as f32 / 6.0;
            let grow = shadow.spread + shadow.blur_radius * t;
            let r = rect.translate(shadow.offset).inflate(grow);
            if r.width() > 0.0 && r.height() > 0.0 {
                self.fill_round_rect(
                    r,
                    radii.grown(grow),
                    with_alpha(shadow.color, alpha * (1.0 - t + 1.0 / 6.0) / 3.5),
                );
            }
        }
    }
    /// The same for a shape of any outline (`outline` from `origin`), as Compose draws one: under it, the outline
    /// filled and stroked 2 * spread wide (mitered), each blurred, the two over each other, moved; inside it, the
    /// color where a hole (the outline worn away by the spread, moved) blurred isn't. The default draws it through
    /// effect layers (push_effect_layer), as Compose's DropShadowPainter and InnerShadowPainter draw a path's shadow
    /// (blurred masks: Android's BlurMaskFilter, Android's sigma for the radius): the masks drawn opaque in layers
    /// that blur them (Decal), in one faded by the color's alpha.
    fn draw_shadow_path(&mut self, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
        if shadow.color.alpha() <= 0.0 || outline.is_empty() {
            return;
        }
        let b = outline.get_bounds().translate(origin);
        let opaque = SolidColor(with_alpha(shadow.color, 1.0));
        let blur_radius = shadow.blur_radius.max(0.0);
        let blur = BlurEffect(blur_radius, blur_radius, TileMode::Decal);
        let spread = shadow.spread.max(0.0);
        let band = PathStroke::new(2.0 * spread); // (mitered, as a Paint's stroke is by default)
        let fill = PathStroke::default();
        let moved = origin + shadow.offset;
        if !inner {
            let grow = spread * band.miter; // (a miter reaches out that far at most)
            let at = b.translate(shadow.offset).inflate(grow);
            let reach = shadow_path_reach(b, shadow, false);
            if self.quick_reject(reach) {
                return;
            }
            self.push_layer(reach, CornerRadii::Zero, shadow.color.alpha(), LayerTransform::Identity);
            self.push_effect_layer(at, &blur, 1.0);
            self.draw_path(outline, moved, &opaque, at, &fill);
            self.pop_layer();
            if spread > 0.0 {
                self.push_effect_layer(at, &blur, 1.0);
                self.draw_path(outline, moved, &opaque, at, &band);
                self.pop_layer();
            }
            self.pop_layer();
            return;
        }
        if self.quick_reject(b) {
            return;
        }
        // The color 2 px past the bounds (a pixel and a half at the least): what cuts the edge's pixels is the
        // outline (the clip), not the fill's own edge.
        let at = shadow_path_reach(b, shadow, true);
        self.push_clip_path(outline, origin, ClipOp::Intersect);
        self.push_layer(at, CornerRadii::Zero, shadow.color.alpha(), LayerTransform::Identity);
        self.fill_round_rect(at, CornerRadii::Zero, with_alpha(shadow.color, 1.0));
        // Less the hole blurred.
        let hole = at.translate(shadow.offset);
        self.push_blend_mode(BlendMode::DstOut);
        self.push_effect_layer(hole, &blur, 1.0);
        self.draw_path(outline, moved, &opaque, hole, &fill);
        if spread > 0.0 {
            self.push_blend_mode(BlendMode::Clear);
            self.draw_path(outline, moved, &opaque, hole, &band);
            self.pop_blend_mode();
        }
        self.pop_layer();
        self.pop_blend_mode();
        self.pop_layer();
        self.pop_clip();
    }
    /// The light elevation shadows have (set by the platform). By default Android's theme's, over the origin.
    fn light(&self) -> ShadowLight {
        ShadowLight::default()
    }
    /// The light's center in the px drawn in now: as given (the window's) less what has moved the drawing (a backend
    /// that knows its translations and layers says so; the default takes the drawing's px for the window's).
    fn light_center(&self) -> Offset {
        self.light().center
    }
    /// An elevation shadow of `outline` (from `origin`) as Android draws one (a RenderNode's, by Skia's
    /// SkShadowUtils): an ambient shadow all round and a spot shadow cast from the light. The default draws them as
    /// Skia does when it can do nothing better (SkDevice::drawShadow's last resort: blurred), through draw_shadow and
    /// draw_shadow_path (a stroke and fill's spread as a shadow's), in the px drawn in (light_center()): the ambient
    /// shadow the outline stroked and filled (stroked 0.5 (r - b) wide, r = z / 2 up to 150, b = 0.5 r (1 + z /
    /// 128)) and blurred by b; the spot shadow the outline cast from the light onto the plane under it (scaled by
    /// Lz / (Lz - z) about the light's foot) and blurred by its radius z / (Lz - z) times the light's. Both blurred
    /// as Android blurs (0.57735 r + 0.5 px).
    fn draw_elevation_shadow(&mut self, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
        let light = self.light();
        let alpha = shadow.alpha.clamp(0.0, 1.0);
        let b = outline.bounds().translate(origin);
        let (rounded, radii) = rounded_radii(outline);
        let cast = |g: &mut Self, s: Shadow, scale: f32, by: Offset| {
            if rounded {
                let r = Rect::new(
                    b.left * scale + by.x,
                    b.top * scale + by.y,
                    b.right * scale + by.x,
                    b.bottom * scale + by.y,
                );
                g.draw_shadow(r, radii.scaled(scale), &s, false);
            } else {
                let mut p = outline_path(outline);
                p.translate(origin);
                g.draw_shadow_path(&scaled_path(&p, scale, by), Offset::Zero, &s, false);
            }
        };
        let flat = shadow.plane[0] * origin.x + shadow.plane[1] * origin.y + shadow.plane[2]; // (at the outline's origin, as Skia's)
        let c = shadow.ambient_color;
        if c.alpha() * light.ambient_alpha * alpha > 0.0 && flat > 0.0 {
            let z = flat;
            let r = (z / 2.0).min(150.0);
            let one_over_a = 1.0 + z / 128.0;
            let blur = 0.5 * r * one_over_a;
            let stroke = (0.5 * (r - blur)).max(0.0); // (a negative width strokes nothing)
            let s =
                Shadow::new(with_alpha(c, c.alpha() * light.ambient_alpha * alpha), Offset::Zero, blur, stroke / 2.0);
            cast(self, s, 1.0, Offset::Zero);
        }
        let center = b.center();
        let z = shadow.plane[0] * center.x + shadow.plane[1] * center.y + shadow.plane[2];
        let lz = light.z;
        let c = shadow.spot_color;
        if c.alpha() * light.spot_alpha * alpha > 0.0 && z > 0.0 && lz > z {
            let ratio = (z / (lz - z)).clamp(0.0, 0.95);
            let scale = (lz / (lz - z)).clamp(1.0, 1.95);
            let l = self.light_center();
            let s = Shadow::new(
                with_alpha(c, c.alpha() * light.spot_alpha * alpha),
                Offset::Zero,
                light.radius * ratio,
                0.0,
            );
            cast(self, s, scale, Offset::new(-ratio * l.x, -ratio * l.y));
        }
    }
    /// A layer clipped to `outline` (from the bounds' top left, any path by its fill type) rather than a rounded
    /// rect. The default clips to the bounds.
    fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
        let _ = outline;
        self.push_layer(bounds, CornerRadii::Zero, alpha, transform);
    }

    // Any Shape over a rect, by its outline there: a rect or a rounded rect whose corners are circles drawn as such,
    // any other as a path. Strokes lie inside the bounds; a clip (push_clip_shape) is popped by pop_clip().

    /// `shape` over `rect` filled with `color`.
    fn fill_shape(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        color: Color,
    ) {
        let p = place(rect, shape, layout_direction, density);
        if p.rounded {
            self.fill_round_rect(rect, p.radii, color);
        } else {
            self.draw_path(
                &outline_path(&p.outline),
                rect.top_left(),
                &SolidColor(color),
                rect,
                &PathStroke::default(),
            );
        }
    }
    /// `shape` over `rect` filled with `brush` spread over `brush_area`.
    fn fill_shape_brush(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        brush: &Brush,
        brush_area: Rect,
    ) {
        let p = place(rect, shape, layout_direction, density);
        if p.rounded {
            self.fill_gradient(rect, p.radii, brush, brush_area);
        } else {
            self.draw_path(&outline_path(&p.outline), rect.top_left(), brush, brush_area, &PathStroke::default());
        }
    }
    /// `shape` over `rect` stroked `width` px wide in `color`, inside it.
    fn stroke_shape(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        width: f32,
        color: Color,
    ) {
        let p = place(rect, shape, layout_direction, density);
        if p.rounded {
            return self.stroke_round_rect(rect, p.radii, width, color);
        }
        self.stroke_shape_brush(rect, shape, layout_direction, density, width, &SolidColor(color), rect);
    }
    /// `shape` over `rect` stroked `width` px wide with `brush` spread over `brush_area`, inside it: along the shape
    /// made half the width smaller (or its rounded rect with the corners that much smaller), its joins round.
    #[allow(clippy::too_many_arguments)]
    fn stroke_shape_brush(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        width: f32,
        brush: &Brush,
        brush_area: Rect,
    ) {
        let path = stroked_path(rect, shape, layout_direction, density, width);
        let stroke = PathStroke { width, join: StrokeJoin::Round, ..PathStroke::default() };
        self.draw_path(&path, rect.top_left(), brush, brush_area, &stroke);
    }
    /// A layer (pop_layer ends it) clipped to `shape` over `bounds`.
    fn push_shape_layer(
        &mut self,
        bounds: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        alpha: f32,
        transform: LayerTransform,
    ) {
        let p = place(bounds, shape, layout_direction, density);
        if p.rounded {
            self.push_layer(bounds, p.radii, alpha, transform);
        } else {
            self.push_outline_layer(bounds, &outline_path(&p.outline), alpha, transform);
        }
    }
    /// Draws until pop_clip() only inside `shape` over `rect` (ClipOp::Difference: only outside it).
    fn push_clip_shape(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        op: ClipOp,
    ) {
        let p = place(rect, shape, layout_direction, density);
        if p.rounded && op == ClipOp::Intersect && p.radii.max() <= 0.0 {
            self.push_clip(rect);
        } else if p.rounded {
            self.push_clip_round_rect(rect, p.radii, op);
        } else {
            self.push_clip_path(&outline_path(&p.outline), rect.top_left(), op);
        }
    }
    /// A shape's shadow as Compose's dropShadow and innerShadow draw it, through draw_shadow or draw_shadow_path:
    /// under it, from a bitmap as big as the shape out to its radius and spread, moved (what is past that cut off);
    /// painted with its color, or its brush over what it covers (in a layer, the brush SrcIn the shadow's mask); at
    /// its alpha, by its blend mode. A backend may draw a brushed one its own way.
    fn draw_shape_shadow(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        shadow: &Shadow,
        inner: bool,
    ) {
        default_draw_shape_shadow(self, rect, shape, layout_direction, density, shadow, inner);
    }

    /// Before what a layout node (known by its id) draws: a recording canvas lines frames up by these; others
    /// ignore them.
    fn begin_node(&mut self, node: u64) {
        let _ = node;
    }
    /// After what the node last begun drew.
    fn end_node(&mut self) {}
    /// Whether anything the node last ended (end_node) drew, with those in it, can show: false only where the
    /// canvas knows all of it was clipped away (a recording canvas: scrolled out of view).
    fn ended_node_shows(&self) -> bool {
        true
    }
    /// Asked as a node (known by its id) is about to be drawn (before begin_node), where the canvas keeps what nodes
    /// drew from one frame to the next (a display list's frames): Kept, it drew again what the node drew last
    /// frame, moved from where it was to `origin` (where it is in what it is drawn in: its position in its parent,
    /// the translation it is drawn through), as the node would (`unchanged`: nothing that it and those in it draw
    /// from changed), and the node draws nothing; Fresh, the node is drawn, into the frame kept; Other: it is drawn,
    /// but not into what is kept (any other canvas, or a node drawn a second time in a frame).
    fn record_node(&mut self, node: u64, origin: Offset, unchanged: bool) -> NodeRecording {
        let _ = (node, origin, unchanged);
        NodeRecording::Other
    }
    /// The next layer pushed is the one known as `key`; `same`: what is drawn in it is what was drawn in it last
    /// frame (but for where it is), so the pixels it had then do.
    fn key_layer(&mut self, key: u64, same: bool) {
        let _ = (key, same);
    }
    /// How to draw into the layer just pushed, where the canvas keeps layers' pixels from one frame to the next (a
    /// recording replayed tells it which layer comes next).
    fn layer_content(&self) -> LayerContent {
        LayerContent::Draw
    }
}

// ---- The provided methods' drawing, for a backend that overrides one and still draws it the default way ----------

/// What [`GraphicsContext::draw_image_mask`] draws by default: the brush in a layer, the image over it by DstIn.
#[allow(clippy::too_many_arguments)]
pub fn default_draw_image_mask<G: GraphicsContext + ?Sized>(
    g: &mut G,
    image: &ImageBitmap,
    src: Rect,
    dst: Rect,
    brush: &Brush,
    brush_area: Rect,
    alpha: f32,
    quality: FilterQuality,
) {
    g.push_layer(dst, CornerRadii::Zero, alpha, LayerTransform::Identity); // the brush, kept where the image is
    match brush.solid_color() {
        Some(color) => g.fill_round_rect(dst, CornerRadii::Zero, color),
        None => g.fill_gradient(dst, CornerRadii::Zero, brush, brush_area),
    }
    g.push_blend_mode(BlendMode::DstIn);
    g.draw_image(image, src, dst, 1.0, quality);
    g.pop_blend_mode();
    g.pop_layer();
}

/// What [`GraphicsContext::draw_shape_shadow`] draws by default (see there).
pub fn default_draw_shape_shadow<G: GraphicsContext + ?Sized>(
    g: &mut G,
    rect: Rect,
    shape: &dyn Shape,
    layout_direction: LayoutDirection,
    density: &Density,
    shadow: &Shadow,
    inner: bool,
) {
    let p = place(rect, shape, layout_direction, density);
    let mut s = Shadow { brush: None, alpha: 1.0, blend_mode: BlendMode::SrcOver, ..shadow.clone() };
    let alpha = shadow.alpha.clamp(0.0, 1.0);
    let mut brush = shadow.brush.clone();
    if let Some(color) = brush.as_ref().and_then(Brush::solid_color) {
        s.color = color;
        brush = None;
    }
    let draw = |g: &mut G, s: &Shadow| {
        if p.rounded {
            g.draw_shadow(rect, p.radii, s, inner);
        } else {
            g.draw_shadow_path(&outline_path(&p.outline), rect.top_left(), s, inner);
        }
    };
    let out = shadow.blur_radius.max(0.0) + shadow.spread.max(0.0);
    let bitmap = rect.translate(shadow.offset).inflate(out);
    if !inner {
        g.push_clip(bitmap);
    }
    if shadow.blend_mode != BlendMode::SrcOver {
        g.push_blend_mode(shadow.blend_mode);
    }
    if let Some(brush) = brush {
        let area = if inner { rect } else { bitmap };
        g.push_layer(area, CornerRadii::Zero, alpha, LayerTransform::Identity);
        s.color = Color::White;
        draw(g, &s);
        g.push_blend_mode(BlendMode::SrcIn);
        g.fill_gradient(area, CornerRadii::Zero, &brush, area);
        g.pop_blend_mode();
        g.pop_layer();
    } else {
        s.color = with_alpha(s.color, s.color.alpha() * alpha);
        draw(g, &s);
    }
    if shadow.blend_mode != BlendMode::SrcOver {
        g.pop_blend_mode();
    }
    if !inner {
        g.pop_clip();
    }
}

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// The color with another alpha, in its color space (Compose's `copy(alpha = …)`).
fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::new_in(c.red(), c.green(), c.blue(), alpha, c.color_space())
}

/// A rounded rect's corners shrunk to fit, as Android draws a RoundRect (C++'s RoundRect::fitted): negative ones
/// none, then all four by the largest factor that fits every side's two corners in it.
fn fitted(rr: &RoundRect) -> RoundRect {
    let positive = |c: CornerRadius| CornerRadius::new(c.x.max(0.0), c.y.max(0.0));
    let mut f = RoundRect {
        top_left_corner_radius: positive(rr.top_left_corner_radius),
        top_right_corner_radius: positive(rr.top_right_corner_radius),
        bottom_right_corner_radius: positive(rr.bottom_right_corner_radius),
        bottom_left_corner_radius: positive(rr.bottom_left_corner_radius),
        ..*rr
    };
    let mut k = 1.0f32;
    let mut fit = |side: f32, a: f32, b: f32| {
        if a + b > side && a + b > 0.0 {
            k = k.min(side.max(0.0) / (a + b));
        }
    };
    let (w, h) = (rr.width(), rr.height());
    fit(w, f.top_left_corner_radius.x, f.top_right_corner_radius.x);
    fit(w, f.bottom_left_corner_radius.x, f.bottom_right_corner_radius.x);
    fit(h, f.top_left_corner_radius.y, f.bottom_left_corner_radius.y);
    fit(h, f.top_right_corner_radius.y, f.bottom_right_corner_radius.y);
    if k < 1.0 {
        f.top_left_corner_radius = f.top_left_corner_radius * k;
        f.top_right_corner_radius = f.top_right_corner_radius * k;
        f.bottom_right_corner_radius = f.bottom_right_corner_radius * k;
        f.bottom_left_corner_radius = f.bottom_left_corner_radius * k;
    }
    f
}

/// Whether an outline is drawn as a rounded rect (a rect, or a rounded rect whose fitted corners are quarter
/// circles), and a rounded rect's fitted radii (none for the others).
fn rounded_radii(outline: &Outline) -> (bool, CornerRadii) {
    match outline {
        Outline::Rectangle(_) => (true, CornerRadii::Zero),
        Outline::Rounded(rr) => {
            let f = fitted(rr);
            let (tl, tr, br, bl) = (
                f.top_left_corner_radius,
                f.top_right_corner_radius,
                f.bottom_right_corner_radius,
                f.bottom_left_corner_radius,
            );
            let circular = tl.x == tl.y && tr.x == tr.y && br.x == br.y && bl.x == bl.y;
            (circular, CornerRadii::new(tl.x, tr.x, br.x, bl.x))
        }
        Outline::Generic(_) => (false, CornerRadii::Zero),
    }
}

/// An outline as a path (C++'s Outline::asPath): a rect's or rounded rect's outline (counterclockwise), or the path.
fn outline_path(outline: &Outline) -> Path {
    match outline {
        Outline::Generic(path) => path.clone(),
        Outline::Rectangle(rect) => {
            let mut p = Path::new();
            p.add_rect(*rect, PathDirection::CounterClockwise);
            p
        }
        Outline::Rounded(rr) => {
            let mut p = Path::new();
            p.add_round_rect(rr, PathDirection::CounterClockwise);
            p
        }
    }
}

/// A shape's outline over a rect, and whether it is drawn as a rounded rect (its corners circles) of those radii.
struct Placed {
    outline: Outline,
    rounded: bool,
    radii: CornerRadii,
}

fn place(rect: Rect, shape: &dyn Shape, layout_direction: LayoutDirection, density: &Density) -> Placed {
    let outline = shape.create_outline(rect.size(), layout_direction, density);
    let (rounded, radii) = rounded_radii(&outline);
    Placed { outline, rounded, radii }
}

/// The path a stroke `width` wide goes along so that it lies inside the shape over `rect` (from the rect's top
/// left): the shape made half the width smaller, or its rounded rect with the corners that much smaller.
fn stroked_path(
    rect: Rect,
    shape: &dyn Shape,
    layout_direction: LayoutDirection,
    density: &Density,
    width: f32,
) -> Path {
    let h = width / 2.0;
    let inner = Size::new((rect.width() - width).max(0.0), (rect.height() - width).max(0.0));
    let full = shape.create_outline(rect.size(), layout_direction, density);
    let f = match &full {
        Outline::Generic(_) => {
            let mut path = outline_path(&shape.create_outline(inner, layout_direction, density));
            path.translate(Offset::new(h, h));
            return path;
        }
        Outline::Rectangle(r) => fitted(&RoundRect::from_rect_radius(*r, CornerRadius::Zero)),
        Outline::Rounded(rr) => fitted(rr),
    };
    let less = |c: CornerRadius| CornerRadius::new((c.x - h).max(0.0), (c.y - h).max(0.0));
    let mut path = Path::new();
    path.add_round_rect(
        &RoundRect::from_rect(
            Rect::from_offset_size(Offset::new(h, h), inner),
            less(f.top_left_corner_radius),
            less(f.top_right_corner_radius),
            less(f.bottom_right_corner_radius),
            less(f.bottom_left_corner_radius),
        ),
        PathDirection::CounterClockwise,
    );
    path
}

/// `path` scaled by `scale` about the origin and moved by `by`.
fn scaled_path(path: &Path, scale: f32, by: Offset) -> Path {
    let p = |o: Offset| Offset::new(o.x * scale + by.x, o.y * scale + by.y);
    let commands = path
        .commands()
        .iter()
        .map(|c| match *c {
            PathCommand::Move(to) => PathCommand::Move(p(to)),
            PathCommand::Line(to) => PathCommand::Line(p(to)),
            PathCommand::Quadratic(c1, to) => PathCommand::Quadratic(p(c1), p(to)),
            PathCommand::Conic(c1, to, w) => PathCommand::Conic(p(c1), p(to), w),
            PathCommand::Cubic(c1, c2, to) => PathCommand::Cubic(p(c1), p(c2), p(to)),
            PathCommand::Close => PathCommand::Close,
        })
        .collect();
    Path::from_commands(commands, path.fill_type)
}
