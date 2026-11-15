//! COLR colour glyphs painted on the CPU through skrifa's paint callbacks.
//!
//! A COLR version 0 glyph is painted as FreeType painted it for the C++ Kompoz (FT_LOAD_COLOR): each layer's
//! outline hinted as FT_LOAD_DEFAULT hints it and rendered to coverage in its own box, the layers blended in that
//! order into premultiplied 8-bit pixels with FreeType's integer arithmetic (tt_face_colr_blend_layer), the box
//! growing to hold each; then made straight as C++ made it. A version 1 glyph's paint graph is painted into
//! premultiplied floating point layers over its clip box (or the box of what it paints): transforms, clips to glyph
//! outlines and boxes (coverage from the same rasterizer), solid fills and linear, radial and sweep gradients with
//! their extend modes, layers composited by each of COLR's composite modes.

use super::outline::{OutlineBuilder, glyf_tags, load};
use super::raster::{Coverage, FixedOutline, FixedPoint};
use skrifa::font::glyph::color::{Brush, Color as PaletteColor, ColorStop, ColorVersion, CompositeMode, Extend};
use skrifa::font::glyph::color::{ColorGlyph, PaintSink};
use skrifa::raw::types::{BoundingBox, Matrix};
use skrifa::{DrawMemory, Font, FontRef, GlyphId, PathSink, ScalerFont};

/// The most pixels a colour glyph is painted with each way (an atlas page's side).
const MAX_SIDE: f32 = 1024.0;

/// A colour glyph's pixels, straight-alpha RGBA (sRGB, rows top down), placed from the glyph's origin as a
/// [`Coverage`] is: `left` px to the right, its top `top` px above.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct ColorImage {
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl ColorImage {
    /// The pixel at (x, y), [r, g, b, a] (transparent outside).
    pub fn at(&self, x: i32, y: i32) -> [u8; 4] {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return [0; 4];
        }
        let i = (y as usize * self.width + x as usize) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

/// A colour glyph painted: its pixels (None when it paints nothing), and whether it used the foreground colour (its
/// palette index 0xFFFF), so that it differs by the text's colour.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PaintedGlyph {
    pub image: Option<ColorImage>,
    pub uses_foreground: bool,
}

/// `glyph`'s COLR colour glyph in `font` at `px` px (None if the font has none for it, or its paint graph is
/// malformed: a renderer then draws its outline). `layers` draws version 0 layers' outlines at `px` hinted as
/// FT_LOAD_DEFAULT (`glyf`, the font's table directory where it has one, gives their points' tags); `units` draws
/// outlines in font units, unhinted (version 1's clips). `foreground` is the colour of palette index 0xFFFF,
/// straight sRGB.
pub fn paint_colr(
    font: &Font,
    glyph: u32,
    px: u32,
    foreground: [u8; 4],
    layers: &ScalerFont,
    glyf: Option<&FontRef<'_>>,
    units: &ScalerFont,
) -> Option<PaintedGlyph> {
    let color_glyph = font.color_glyphs().get(GlyphId::new(glyph))?;
    let palette: &[PaletteColor] = font.palettes().get(0).map(|p| p.colors()).unwrap_or(&[]);
    match color_glyph.version() {
        ColorVersion::V0 => {
            let mut painter = LayerPainter {
                scaler: layers,
                glyf,
                palette,
                foreground,
                uses_foreground: false,
                failed: false,
                canvas: Canvas8::default(),
            };
            color_glyph.paint(&mut painter).ok()?;
            if painter.failed {
                return None;
            }
            Some(PaintedGlyph { image: painter.canvas.straight(), uses_foreground: painter.uses_foreground })
        }
        ColorVersion::V1 => {
            let scale = px as f32 / font.units_per_em().max(1) as f32;
            let bounds = v1_bounds(&color_glyph)?;
            let left = (bounds.x_min * scale).floor();
            let right = (bounds.x_max * scale).ceil();
            let bottom = (bounds.y_min * scale).floor();
            let top = (bounds.y_max * scale).ceil();
            if !(right - left).is_finite() || !(top - bottom).is_finite() {
                return None;
            }
            let (width, height) = ((right - left).max(0.0), (top - bottom).max(0.0));
            if width > MAX_SIDE || height > MAX_SIDE {
                return Some(PaintedGlyph::default());
            }
            let (width, height) = (width as usize, height as usize);
            if width == 0 || height == 0 {
                return Some(PaintedGlyph::default());
            }
            let root = Affine([scale, 0.0, 0.0, scale, -left, -bottom]);
            let mut painter = Painter::new(units, palette, foreground, width, height, root);
            color_glyph.paint(&mut painter).ok()?;
            let rgba = painter.straight();
            Some(PaintedGlyph {
                image: Some(ColorImage { left: left as i32, top: top as i32, width, height, rgba }),
                uses_foreground: painter.uses_foreground,
            })
        }
    }
}

/// What a version 1 glyph covers, in font units (y up): its clip box, else the box of what it paints.
fn v1_bounds(glyph: &ColorGlyph<'_>) -> Option<BoundingBox<f32>> {
    if let Some(b) = glyph.clip_box() {
        return Some(b);
    }
    let e = glyph.extents().ok()?;
    Some(BoundingBox {
        x_min: e.x_bearing,
        y_min: e.y_bearing - e.height,
        x_max: e.x_bearing + e.width,
        y_max: e.y_bearing,
    })
}

/// Palette entry `index` (0xFFFF: the foreground) times `alpha`, straight sRGB 0..1.
fn palette_color(palette: &[PaletteColor], foreground: [u8; 4], index: u16, alpha: f32) -> [f32; 4] {
    let [r, g, b, a] = if index == 0xFFFF {
        foreground
    } else {
        match palette.get(index as usize) {
            Some(c) => [c.red, c.green, c.blue, c.alpha],
            None => [0, 0, 0, 0],
        }
    };
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0 * alpha.clamp(0.0, 1.0)]
}

// ---- Version 0: FreeType's layers ---------------------------------------------------------------------------------

/// Premultiplied 8-bit RGBA placed from the glyph's origin (FreeType's BGRA glyph slot, in RGBA order).
#[derive(Default)]
struct Canvas8 {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas8 {
    /// `layer`'s coverage blended in `color` (straight 8-bit RGBA) as FreeType blends a COLR layer: the canvas made
    /// the layer's box at first (while it has no pixels), else grown to hold it.
    fn blend(&mut self, layer: &Coverage, color: [u8; 4]) {
        if self.width == 0 || self.height == 0 {
            *self = Canvas8 {
                left: layer.left,
                top: layer.top,
                width: layer.width,
                height: layer.height,
                pixels: vec![0; layer.width * layer.height * 4],
            };
        } else {
            let x_min = self.left.min(layer.left);
            let x_max = (self.left + self.width as i32).max(layer.left + layer.width as i32);
            let y_min = (self.top - self.height as i32).min(layer.top - layer.height as i32);
            let y_max = self.top.max(layer.top);
            if x_min != self.left
                || x_max != self.left + self.width as i32
                || y_min != self.top - self.height as i32
                || y_max != self.top
            {
                let (width, height) = ((x_max - x_min) as usize, (y_max - y_min) as usize);
                let mut pixels = vec![0u8; width * height * 4];
                let (dx, dy) = ((self.left - x_min) as usize, (y_max - self.top) as usize);
                for row in 0..self.height {
                    let from = row * self.width * 4;
                    let to = ((row + dy) * width + dx) * 4;
                    pixels[to..to + self.width * 4].copy_from_slice(&self.pixels[from..from + self.width * 4]);
                }
                *self = Canvas8 { left: x_min, top: y_max, width, height, pixels };
            }
        }
        let [r, g, b, alpha] = color.map(|c| c as i32);
        let (dx, dy) = ((layer.left - self.left) as usize, (self.top - layer.top) as usize);
        for y in 0..layer.height {
            for x in 0..layer.width {
                let aa = layer.alpha[y * layer.width + x] as i32;
                let fa = alpha * aa / 255;
                let (fr, fg, fb) = (r * fa / 255, g * fa / 255, b * fa / 255);
                let ba2 = 255 - fa;
                let i = ((y + dy) * self.width + x + dx) * 4;
                let d = &mut self.pixels[i..i + 4];
                d[0] = (d[0] as i32 * ba2 / 255 + fr) as u8;
                d[1] = (d[1] as i32 * ba2 / 255 + fg) as u8;
                d[2] = (d[2] as i32 * ba2 / 255 + fb) as u8;
                d[3] = (d[3] as i32 * ba2 / 255 + fa) as u8;
            }
        }
    }

    /// Its pixels made straight as C++ made FreeType's (each colour over the alpha as a fraction, cut down to a
    /// byte); None when it has none.
    fn straight(&self) -> Option<ColorImage> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let mut rgba = self.pixels.clone();
        for p in rgba.as_chunks_mut::<4>().0 {
            let a = p[3] as f32 / 255.0;
            for c in &mut p[..3] {
                *c = if a > 0.0 { (*c as f32 / a).min(255.0) as u8 } else { 0 };
            }
        }
        Some(ColorImage { left: self.left, top: self.top, width: self.width, height: self.height, rgba })
    }
}

/// Paints a version 0 glyph: each layer a solid fill of a glyph's outline.
struct LayerPainter<'a> {
    scaler: &'a ScalerFont,
    glyf: Option<&'a FontRef<'a>>,
    palette: &'a [PaletteColor],
    foreground: [u8; 4],
    uses_foreground: bool,
    failed: bool,
    canvas: Canvas8,
}

impl PaintSink for LayerPainter<'_> {
    fn push_transform(&mut self, _: Matrix<f32>) {
        self.failed = true;
    }
    fn pop_transform(&mut self) {}
    fn push_clip_glyph(&mut self, _: GlyphId) {
        self.failed = true;
    }
    fn push_clip_box(&mut self, _: BoundingBox<f32>) {
        self.failed = true;
    }
    fn pop_clip(&mut self) {}
    fn fill(&mut self, _: Brush<'_>) {
        self.failed = true;
    }
    fn push_layer(&mut self, _: CompositeMode) {
        self.failed = true;
    }

    fn fill_glyph(&mut self, glyph: GlyphId, brush_transform: Option<Matrix<f32>>, brush: Brush<'_>) {
        let Brush::Solid { palette_index, alpha } = brush else {
            self.failed = true;
            return;
        };
        if brush_transform.is_some() {
            self.failed = true;
            return;
        }
        let tags = self.glyf.and_then(|f| glyf_tags(f, glyph.to_u32()));
        let Some(outline) = load(self.scaler, glyph.to_u32(), tags) else {
            self.failed = true;
            return;
        };
        self.uses_foreground |= palette_index == 0xFFFF;
        let c = palette_color(self.palette, self.foreground, palette_index, alpha);
        let color = c.map(|v| (v * 255.0).round() as u8);
        self.canvas.blend(&outline.render(), color);
    }
}

// ---- Version 1: the paint graph -----------------------------------------------------------------------------------

/// An affine transform [xx, yx, xy, yy, dx, dy]: (x, y) to (xx·x + xy·y + dx, yx·x + yy·y + dy).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Affine([f32; 6]);

impl Affine {
    fn of(m: &Matrix<f32>) -> Affine {
        Affine([m.xx, m.yx, m.xy, m.yy, m.dx, m.dy])
    }

    /// `self` after `t`: t applied first.
    fn then_first(&self, t: &Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [ta, tb, tc, td, te, tf] = t.0;
        Affine([
            a * ta + c * tb,
            b * ta + d * tb,
            a * tc + c * td,
            b * tc + d * td,
            a * te + c * tf + e,
            b * te + d * tf + f,
        ])
    }

    fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }

    fn invert(&self) -> Option<Affine> {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        if det.abs() < 1e-12 || !det.is_finite() {
            return None;
        }
        let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
        Some(Affine([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)]))
    }
}

/// A path sink putting each point through a transform before building a FreeType outline (px, y up).
struct TransformedBuilder<'a> {
    transform: Affine,
    builder: &'a mut OutlineBuilder,
}

impl PathSink for TransformedBuilder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.transform.apply(x, y);
        self.builder.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.transform.apply(x, y);
        self.builder.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let ((cx, cy), (x, y)) = (self.transform.apply(cx0, cy0), self.transform.apply(x, y));
        self.builder.quad_to(cx, cy, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c0, c1, p) = (self.transform.apply(cx0, cy0), self.transform.apply(cx1, cy1), self.transform.apply(x, y));
        self.builder.curve_to(c0.0, c0.1, c1.0, c1.1, p.0, p.1);
    }
    fn close(&mut self) {
        self.builder.close();
    }
}

/// Paints a version 1 glyph into a `width` × `height` canvas (rows top down) whose bottom left corner is the origin
/// of the space `root` maps font units into (px, y up): the current transform, a stack of clip coverages (0..1 per
/// pixel) and a stack of premultiplied layers.
struct Painter<'a> {
    units: &'a ScalerFont,
    palette: &'a [PaletteColor],
    foreground: [u8; 4],
    uses_foreground: bool,
    width: usize,
    height: usize,
    transforms: Vec<Affine>,
    clips: Vec<Vec<f32>>,
    layers: Vec<Vec<[f32; 4]>>,
}

impl<'a> Painter<'a> {
    fn new(
        units: &'a ScalerFont,
        palette: &'a [PaletteColor],
        foreground: [u8; 4],
        width: usize,
        height: usize,
        root: Affine,
    ) -> Painter<'a> {
        let n = width * height;
        Painter {
            units,
            palette,
            foreground,
            uses_foreground: false,
            width,
            height,
            transforms: vec![root],
            clips: vec![vec![1.0; n]],
            layers: vec![vec![[0.0; 4]; n]],
        }
    }

    fn transform(&self) -> Affine {
        *self.transforms.last().expect("the root transform")
    }

    /// The canvas point (px, y up) at the middle of pixel (x, y) (rows counted down).
    fn center(&self, x: usize, y: usize) -> (f32, f32) {
        (x as f32 + 0.5, (self.height - y) as f32 - 0.5)
    }

    /// The current clip narrowed to `outline` (canvas px, y up).
    fn push_clip_outline(&mut self, outline: &FixedOutline) {
        let coverage = outline.rasterize(self.width, self.height);
        let top = self.clips.last().expect("the root clip");
        let clip: Vec<f32> = if coverage.len() == top.len() {
            top.iter().zip(&coverage).map(|(c, &a)| c * a as f32 / 255.0).collect()
        } else {
            vec![0.0; top.len()]
        };
        self.clips.push(clip);
    }

    /// The premultiplied colour at `t` along `stops` (normalized, sorted), interpolated premultiplied.
    fn along(&mut self, stops: &[ColorStop], t: f32) -> [f32; 4] {
        let color = |p: &mut Painter, s: &ColorStop| {
            p.uses_foreground |= s.palette_index == 0xFFFF;
            premultiplied(palette_color(p.palette, p.foreground, s.palette_index, s.alpha))
        };
        let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
            return [0.0; 4];
        };
        if t <= first.offset {
            return color(self, first);
        }
        if t >= last.offset {
            return color(self, last);
        }
        for pair in stops.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if t >= a.offset && t <= b.offset {
                let span = b.offset - a.offset;
                let f = if span > 0.0 { (t - a.offset) / span } else { 1.0 };
                let (ca, cb) = (color(self, a), color(self, b));
                return std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * f);
            }
        }
        color(self, last)
    }

    /// The colour a degenerate gradient paints (Skia's): its last stop's when padded, else the average along it.
    fn degenerate(&mut self, stops: &[ColorStop], extend: Extend) -> [f32; 4] {
        if !matches!(extend, Extend::Repeat | Extend::Reflect) {
            return stops.last().map_or([0.0; 4], |s| self.along(stops, s.offset));
        }
        let mut sum = [0.0f32; 4];
        let mut previous = (0.0f32, self.along(stops, 0.0));
        for s in stops.iter().map(|s| s.offset.clamp(0.0, 1.0)).chain(std::iter::once(1.0)) {
            let c = self.along(stops, s);
            for i in 0..4 {
                sum[i] += (previous.1[i] + c[i]) * 0.5 * (s - previous.0);
            }
            previous = (s, c);
        }
        sum
    }

    /// `brush`'s premultiplied colour at `p` (in the brush's space).
    fn brush_at(&mut self, brush: &Brush<'_>, p: (f32, f32)) -> [f32; 4] {
        match brush {
            Brush::Solid { palette_index, alpha } => {
                self.uses_foreground |= *palette_index == 0xFFFF;
                premultiplied(palette_color(self.palette, self.foreground, *palette_index, *alpha))
            }
            Brush::LinearGradient { p0, p1, color_stops, extend } => {
                let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
                let d2 = dx * dx + dy * dy;
                if d2 <= f32::EPSILON {
                    return self.degenerate(color_stops, *extend);
                }
                let t = ((p.0 - p0.x) * dx + (p.1 - p0.y) * dy) / d2;
                self.along(color_stops, extended(t, *extend))
            }
            Brush::RadialGradient { c0, r0, c1, r1, color_stops, extend } => {
                match conical_t(p, (c0.x, c0.y), *r0, (c1.x, c1.y), *r1) {
                    Some(t) => self.along(color_stops, extended(t, *extend)),
                    None => [0.0; 4],
                }
            }
            Brush::SweepGradient { c0, start_angle, end_angle, color_stops, extend } => {
                // Clockwise degrees from +x in the brush's space (y up), 0..360.
                let ccw = (p.1 - c0.y).atan2(p.0 - c0.x).to_degrees();
                let angle = (360.0 - ccw).rem_euclid(360.0);
                let sector = end_angle - start_angle;
                let t = if sector.abs() <= f32::EPSILON {
                    if angle < *start_angle { 0.0 } else { 1.0 }
                } else {
                    (angle - start_angle) / sector
                };
                self.along(color_stops, extended(t, *extend))
            }
        }
    }

    /// Its pixels made straight 8-bit RGBA.
    fn straight(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for p in &self.layers[0] {
            let a = p[3].clamp(0.0, 1.0);
            for c in &p[..3] {
                let v = if a > 0.0 { (c / a).clamp(0.0, 1.0) } else { 0.0 };
                out.push((v * 255.0).round() as u8);
            }
            out.push((a * 255.0).round() as u8);
        }
        out
    }
}

impl PaintSink for Painter<'_> {
    fn push_transform(&mut self, transform: Matrix<f32>) {
        let t = self.transform().then_first(&Affine::of(&transform));
        self.transforms.push(t);
    }

    fn pop_transform(&mut self) {
        if self.transforms.len() > 1 {
            self.transforms.pop();
        }
    }

    fn push_clip_glyph(&mut self, glyph: GlyphId) {
        let mut builder = OutlineBuilder::default();
        if let Some(outline) = self.units.outline(glyph) {
            let mut sink = TransformedBuilder { transform: self.transform(), builder: &mut builder };
            if outline.draw(DrawMemory::Scratch, &mut sink).is_err() {
                builder = OutlineBuilder::default();
            }
        }
        let outline = builder.finish();
        self.push_clip_outline(&outline);
    }

    fn push_clip_box(&mut self, b: BoundingBox<f32>) {
        let t = self.transform();
        let corners = [(b.x_min, b.y_min), (b.x_max, b.y_min), (b.x_max, b.y_max), (b.x_min, b.y_max)];
        let mut outline = FixedOutline::new();
        outline.add_polygon(corners.iter().map(|&(x, y)| {
            let (x, y) = t.apply(x, y);
            FixedPoint::from_px(x, y)
        }));
        self.push_clip_outline(&outline);
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    fn fill(&mut self, brush: Brush<'_>) {
        let solid = matches!(brush, Brush::Solid { .. });
        let inverse = if solid { Some(Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])) } else { self.transform().invert() };
        let Some(inverse) = inverse else {
            return;
        };
        let clip = std::mem::take(self.clips.last_mut().expect("the root clip"));
        let mut layer = std::mem::take(self.layers.last_mut().expect("the root layer"));
        for y in 0..self.height {
            for x in 0..self.width {
                let i = y * self.width + x;
                let c = clip[i];
                if c <= 0.0 {
                    continue;
                }
                let (cx, cy) = self.center(x, y);
                let s = self.brush_at(&brush, inverse.apply(cx, cy)).map(|v| v * c);
                let d = &mut layer[i];
                for k in 0..4 {
                    d[k] = s[k] + d[k] * (1.0 - s[3]);
                }
            }
        }
        *self.clips.last_mut().expect("the root clip") = clip;
        *self.layers.last_mut().expect("the root layer") = layer;
    }

    fn push_layer(&mut self, _: CompositeMode) {
        self.layers.push(vec![[0.0; 4]; self.width * self.height]);
    }

    fn pop_layer_with_mode(&mut self, mode: CompositeMode) {
        if self.layers.len() < 2 {
            return;
        }
        let top = self.layers.pop().expect("a layer");
        let clip = self.clips.last().expect("the root clip");
        let below = self.layers.last_mut().expect("the root layer");
        for ((d, s), &c) in below.iter_mut().zip(&top).zip(clip) {
            if c <= 0.0 {
                continue;
            }
            let blended = composite(*s, *d, mode);
            for k in 0..4 {
                d[k] += (blended[k] - d[k]) * c;
            }
        }
    }
}

fn premultiplied(c: [f32; 4]) -> [f32; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

/// `t` through a colour line's extend mode: held at the ends, repeated, or reflected.
fn extended(t: f32, extend: Extend) -> f32 {
    match extend {
        Extend::Repeat => t - t.floor(),
        Extend::Reflect => {
            let u = t.rem_euclid(2.0);
            if u > 1.0 { 2.0 - u } else { u }
        }
        _ => t.clamp(0.0, 1.0),
    }
}

/// Where `p` is along a two-point conical gradient from circle (c0, r0) to (c1, r1): the largest t whose circle
/// (centre and radius interpolated, the radius not below zero) passes through p. None where no circle does.
fn conical_t(p: (f32, f32), c0: (f32, f32), r0: f32, c1: (f32, f32), r1: f32) -> Option<f32> {
    let (cdx, cdy) = (c1.0 - c0.0, c1.1 - c0.1);
    let (pdx, pdy) = (p.0 - c0.0, p.1 - c0.1);
    let dr = r1 - r0;
    let a = cdx * cdx + cdy * cdy - dr * dr;
    let b = pdx * cdx + pdy * cdy + r0 * dr;
    let c = pdx * pdx + pdy * pdy - r0 * r0;
    let radius_ok = |t: f32| r0 + t * dr >= 0.0;
    if a.abs() < 1e-6 * (b.abs() + c.abs()).max(1e-12) {
        if b.abs() < 1e-12 {
            return None;
        }
        let t = c / (2.0 * b);
        return radius_ok(t).then_some(t);
    }
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let (t1, t2) = ((b + root) / a, (b - root) / a);
    let (hi, lo) = if t1 >= t2 { (t1, t2) } else { (t2, t1) };
    if radius_ok(hi) {
        Some(hi)
    } else if radius_ok(lo) {
        Some(lo)
    } else {
        None
    }
}

/// `s` composited onto `d` (both premultiplied) by `mode`: Porter-Duff's coefficients, Plus clamped, the separable
/// and non-separable blend modes as the W3C compositing spec defines them.
pub(crate) fn composite(s: [f32; 4], d: [f32; 4], mode: CompositeMode) -> [f32; 4] {
    let (sa, da) = (s[3], d[3]);
    let porter_duff = |fs: f32, fd: f32| -> [f32; 4] { std::array::from_fn(|i| s[i] * fs + d[i] * fd) };
    match mode {
        CompositeMode::Clear => [0.0; 4],
        CompositeMode::Src => s,
        CompositeMode::Dest => d,
        CompositeMode::SrcOver => porter_duff(1.0, 1.0 - sa),
        CompositeMode::DestOver => porter_duff(1.0 - da, 1.0),
        CompositeMode::SrcIn => porter_duff(da, 0.0),
        CompositeMode::DestIn => porter_duff(0.0, sa),
        CompositeMode::SrcOut => porter_duff(1.0 - da, 0.0),
        CompositeMode::DestOut => porter_duff(0.0, 1.0 - sa),
        CompositeMode::SrcAtop => porter_duff(da, 1.0 - sa),
        CompositeMode::DestAtop => porter_duff(1.0 - da, sa),
        CompositeMode::Xor => porter_duff(1.0 - da, 1.0 - sa),
        CompositeMode::Plus => std::array::from_fn(|i| (s[i] + d[i]).min(1.0)),
        _ => {
            let unpremultiply = |c: [f32; 4]| -> [f32; 3] {
                if c[3] > 0.0 { [c[0] / c[3], c[1] / c[3], c[2] / c[3]] } else { [0.0; 3] }
            };
            let (cs, cb) = (unpremultiply(s), unpremultiply(d));
            let b: [f32; 3] = match mode {
                CompositeMode::HslHue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
                CompositeMode::HslSaturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
                CompositeMode::HslColor => set_lum(cs, lum(cb)),
                CompositeMode::HslLuminosity => set_lum(cb, lum(cs)),
                _ => std::array::from_fn(|i| separable(mode, cs[i], cb[i])),
            };
            let mut out: [f32; 4] =
                std::array::from_fn(
                    |i| {
                        if i < 3 { s[i] * (1.0 - da) + d[i] * (1.0 - sa) + sa * da * b[i] } else { 0.0 }
                    },
                );
            out[3] = sa + da - sa * da;
            out
        }
    }
}

/// A separable blend mode's B(cs, cb) (straight colours).
fn separable(mode: CompositeMode, cs: f32, cb: f32) -> f32 {
    let screen = |a: f32, b: f32| a + b - a * b;
    let hard_light = |cs: f32, cb: f32| if cs <= 0.5 { cb * 2.0 * cs } else { screen(cb, 2.0 * cs - 1.0) };
    match mode {
        CompositeMode::Screen => screen(cs, cb),
        CompositeMode::Overlay => hard_light(cb, cs),
        CompositeMode::Darken => cs.min(cb),
        CompositeMode::Lighten => cs.max(cb),
        CompositeMode::ColorDodge => {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        CompositeMode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        CompositeMode::HardLight => hard_light(cs, cb),
        CompositeMode::SoftLight => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 { ((16.0 * cb - 12.0) * cb + 4.0) * cb } else { cb.sqrt() };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        CompositeMode::Difference => (cs - cb).abs(),
        CompositeMode::Exclusion => cs + cb - 2.0 * cs * cb,
        CompositeMode::Multiply => cs * cb,
        _ => cs,
    }
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 && l - n > 0.0 {
        out = out.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 && x - l > 0.0 {
        out = out.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    out
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let (max, min) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    if max <= min {
        return [0.0; 3];
    }
    c.map(|v| (v - min) * s / (max - min))
}
