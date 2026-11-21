//! What paints and what it goes through (Compose's `ui-graphics`): tile modes, blend modes and [`blend`], shaders
//! (gradients, images, composites), brushes (a color, gradients, shaders of one's own, composites) and their lerp,
//! border strokes, vertices, color matrices and color filters (on colors, brushes and bitmaps), render effects
//! (blurs, offsets, Compose 1.13's progressive blurs) and filter qualities. Lengths are pixels.

use crate::color::{Color, lerp_color};
use crate::geometry::{Offset, Rect, Size, lerp, lerp_offset};
use crate::image::{ImageBitmap, WeakImageBitmap};
use crate::matrix::Matrix;
use crate::unit::{Density, Dp};
use kompoz_runtime::named;
use std::fmt;
use std::sync::{Arc, Mutex};

/// The color with another alpha, in its color space (Compose's `copy(alpha = …)`).
fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::new_in(c.red(), c.green(), c.blue(), alpha, c.color_space())
}

// ---- Tile modes, filter qualities, vertex modes ------------------------------------------------------------------

/// How a gradient (or an image's shader) goes on past its ends (Compose's TileMode): its end colors (Clamp), over
/// again (Repeated), back and forth (Mirror), or not at all (Decal: transparent there).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TileMode {
    #[default]
    Clamp,
    Repeated,
    Mirror,
    Decal,
}

impl TileMode {
    /// Whether it can be drawn here (Compose's isSupported): every mode can.
    pub fn is_supported(self) -> bool {
        true
    }
}

/// How an image is sampled where it is drawn bigger or smaller than its pixels (Compose's FilterQuality): the
/// nearest pixel (None), between the four nearest (Low, bilinear), from copies halved and halved again as small as
/// it is drawn (Medium: mipmaps), or a cubic of the sixteen nearest (High: Mitchell's).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FilterQuality {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

impl FilterQuality {
    /// Compose's value.
    pub fn value(self) -> i32 {
        self as i32
    }
}

/// What images are drawn with unless told otherwise (Compose's DrawScope.DefaultFilterQuality).
pub const DefaultFilterQuality: FilterQuality = FilterQuality::Low;

/// How [`Vertices`] make triangles (Compose's VertexMode): each three in turn, each three running along a strip
/// (each one with the two before it), or a fan round the first.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VertexMode {
    Triangles,
    TriangleStrip,
    TriangleFan,
}

// ---- Blend modes -------------------------------------------------------------------------------------------------

/// How a drawing (the source) goes onto what is there (the destination), Compose's BlendMode in its order: the
/// Porter-Duff ones, Plus and Modulate, the separable ones and the non-separable ones (hue, saturation, color,
/// luminosity), as W3C's compositing spec and Skia define them. Also how a tint's color goes onto the pixels it
/// filters ([`ColorFilter::tint`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum BlendMode {
    Clear,
    Src,
    Dst,
    #[default]
    SrcOver,
    DstOver,
    SrcIn,
    DstIn,
    SrcOut,
    DstOut,
    SrcAtop,
    DstAtop,
    Xor,
    Plus,
    Modulate,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    Hardlight,
    Softlight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    /// Whether it can be drawn here (Compose's isSupported): every mode can.
    pub fn is_supported(self) -> bool {
        true
    }
}

/// W3C's luminosity of a straight color.
fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// W3C's ClipColor: back within 0..1, the luminosity kept.
fn clip_color(mut c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    if n < 0.0 && l - n > 0.0 {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 && x - l > 0.0 {
        c = c.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    c
}

/// W3C's SetLum.
fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

/// W3C's Sat.
fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// W3C's SetSat.
fn set_sat(mut c: [f32; 3], s: f32) -> [f32; 3] {
    let mut order = [0usize, 1, 2]; // min, mid, max
    order.sort_by(|&x, &y| c[x].total_cmp(&c[y]));
    let [lo, mid, hi] = order;
    if c[hi] > c[lo] {
        c[mid] = (c[mid] - c[lo]) * s / (c[hi] - c[lo]);
        c[hi] = s;
    } else {
        c[mid] = 0.0;
        c[hi] = 0.0;
    }
    c[lo] = 0.0;
    c
}

/// A separable mode's B(cs, cb), on straight channels.
fn separable(mode: BlendMode, cs: f32, cb: f32) -> f32 {
    let hard_light = |s: f32, b: f32| if s <= 0.5 { b * 2.0 * s } else { b + (2.0 * s - 1.0) - b * (2.0 * s - 1.0) };
    match mode {
        BlendMode::Multiply => cs * cb,
        BlendMode::Screen => cs + cb - cs * cb,
        BlendMode::Overlay => hard_light(cb, cs),
        BlendMode::Darken => cs.min(cb),
        BlendMode::Lighten => cs.max(cb),
        BlendMode::ColorDodge => {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        BlendMode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        BlendMode::Hardlight => hard_light(cs, cb),
        BlendMode::Softlight => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 { ((16.0 * cb - 12.0) * cb + 4.0) * cb } else { cb.sqrt() };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        BlendMode::Difference => (cs - cb).abs(),
        BlendMode::Exclusion => cs + cb - 2.0 * cs * cb,
        _ => cs,
    }
}

/// `source` drawn onto `destination` with `mode`: both straight colors (taken in sRGB), as is the result. The
/// Porter-Duff modes as their coefficients say, the others as W3C's compositing spec (and Skia) blend them.
pub fn blend(source: Color, destination: Color, mode: BlendMode) -> Color {
    let (source, destination) = (source.in_srgb(), destination.in_srgb());
    let (sa, da) = (source.alpha(), destination.alpha());
    let cs = [source.red(), source.green(), source.blue()];
    let cb = [destination.red(), destination.green(), destination.blue()];
    let s = cs.map(|c| c * sa);
    let d = cb.map(|c| c * da);
    // (the separable and non-separable modes' alpha)
    let mut a = sa + da - sa * da;
    let mut out = [0.0f32; 3];
    // s fs + d fd
    let mut porter_duff = |fs: f32, fd: f32| {
        for i in 0..3 {
            out[i] = s[i] * fs + d[i] * fd;
        }
        a = sa * fs + da * fd;
    };
    match mode {
        BlendMode::Clear => porter_duff(0.0, 0.0),
        BlendMode::Src => porter_duff(1.0, 0.0),
        BlendMode::Dst => porter_duff(0.0, 1.0),
        BlendMode::SrcOver => porter_duff(1.0, 1.0 - sa),
        BlendMode::DstOver => porter_duff(1.0 - da, 1.0),
        BlendMode::SrcIn => porter_duff(da, 0.0),
        BlendMode::DstIn => porter_duff(0.0, sa),
        BlendMode::SrcOut => porter_duff(1.0 - da, 0.0),
        BlendMode::DstOut => porter_duff(0.0, 1.0 - sa),
        BlendMode::SrcAtop => porter_duff(da, 1.0 - sa),
        BlendMode::DstAtop => porter_duff(1.0 - da, sa),
        BlendMode::Xor => porter_duff(1.0 - da, 1.0 - sa),
        BlendMode::Plus => {
            for i in 0..3 {
                out[i] = (s[i] + d[i]).min(1.0);
            }
            a = (sa + da).min(1.0);
        }
        BlendMode::Modulate => {
            for i in 0..3 {
                out[i] = s[i] * d[i];
            }
            a = sa * da;
        }
        BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => {
            let b = match mode {
                BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
                BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
                BlendMode::Color => set_lum(cs, lum(cb)),
                _ => set_lum(cb, lum(cs)),
            };
            for i in 0..3 {
                out[i] = s[i] * (1.0 - da) + d[i] * (1.0 - sa) + sa * da * b[i];
            }
        }
        _ => {
            // separable: s (1 - da) + d (1 - sa) + sa da B(cs, cb)
            for i in 0..3 {
                out[i] = s[i] * (1.0 - da) + d[i] * (1.0 - sa) + sa * da * separable(mode, cs[i], cb[i]);
            }
        }
    }
    if a <= 0.0 {
        return Color::new(0.0, 0.0, 0.0, 0.0);
    }
    Color::new((out[0] / a).clamp(0.0, 1.0), (out[1] / a).clamp(0.0, 1.0), (out[2] / a).clamp(0.0, 1.0), a.min(1.0))
}

// ---- Gradients' colors -------------------------------------------------------------------------------------------

/// A color at a place along a gradient, 0..1 (Compose's color stops, `offset to color`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorStop {
    pub offset: f32,
    pub color: Color,
}

impl ColorStop {
    pub const fn new(offset: f32, color: Color) -> ColorStop {
        ColorStop { offset, color }
    }
}

impl From<(f32, Color)> for ColorStop {
    fn from((offset, color): (f32, Color)) -> ColorStop {
        ColorStop { offset, color }
    }
}

/// `colors` at `stops`, or spread evenly where there are none.
fn stops_of(colors: &[Color], stops: Option<&[f32]>) -> Vec<ColorStop> {
    let n = colors.len();
    colors
        .iter()
        .enumerate()
        .map(|(i, &color)| {
            let offset = match stops {
                Some(s) if s.len() == n => s[i],
                _ if n > 1 => i as f32 / (n - 1) as f32,
                _ => 0.0,
            };
            ColorStop { offset, color }
        })
        .collect()
}

/// As Compose (Android) checks a gradient's colors: at least two without stops, as many stops as colors with them.
fn validate_color_stops(colors: &[Color], color_stops: Option<&[f32]>) {
    match color_stops {
        None if colors.len() < 2 => panic!("colors must have length of at least 2 if colorStops is omitted."),
        Some(s) if s.len() != colors.len() => panic!("colors and colorStops arguments must have equal length."),
        _ => {}
    }
}

/// `t` along `stops`, past their ends as `tile` says (Decal: nothing); each component along in sRGB, not
/// premultiplied (as Skia interpolates).
fn color_along(stops: &[ColorStop], tile: TileMode, mut t: f32) -> Color {
    match stops {
        [] => return Color::Transparent,
        [one] => return one.color,
        _ => {}
    }
    match tile {
        TileMode::Clamp => t = t.clamp(0.0, 1.0),
        TileMode::Repeated => t -= t.floor(),
        TileMode::Mirror => {
            t = t.abs() % 2.0;
            if t > 1.0 {
                t = 2.0 - t;
            }
        }
        TileMode::Decal => {
            if !(0.0..=1.0).contains(&t) {
                return Color::Transparent;
            }
        }
    }
    if t <= stops[0].offset {
        return stops[0].color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.offset {
            let span = b.offset - a.offset;
            let (x, y) = (a.color.in_srgb(), b.color.in_srgb());
            let u = if span > 0.0 { (t - a.offset) / span } else { 1.0 };
            return Color::new(
                lerp(x.red(), y.red(), u),
                lerp(x.green(), y.green(), u),
                lerp(x.blue(), y.blue(), u),
                lerp(x.alpha(), y.alpha(), u),
            );
        }
    }
    stops[stops.len() - 1].color
}

/// Where `p` is along a sweep round `center`, 0..1 clockwise from 3 o'clock.
fn sweep_parameter(p: Offset, center: Offset) -> f32 {
    let tau = 2.0 * std::f32::consts::PI;
    let mut a = (p.y - center.y).atan2(p.x - center.x);
    if a < 0.0 {
        a += tau;
    }
    a / tau
}

// ---- Shaders ---------------------------------------------------------------------------------------------------

/// What kind of shader it is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ShaderKind {
    Linear,
    Radial,
    Sweep,
    Image,
    Composite,
}

#[derive(Clone, PartialEq)]
struct ShaderData {
    kind: ShaderKind,
    stops: Vec<ColorStop>,
    // (an image's across, and down)
    tile_mode: TileMode,
    tile_mode_y: TileMode,
    image: Option<ImageBitmap>,
    start: Offset,
    end: Offset,
    radius: f32,
    dst: Option<Shader>,
    src: Option<Shader>,
    blend_mode: BlendMode,
    // its transform and its inverse
    transform: Option<Matrix>,
    back: Option<Matrix>,
}

/// What paints a shape pixel by pixel (Compose's Shader): a gradient, an image (tiled), or two shaders composited,
/// through a transform if it has one; an image's pixel a pixel. Made by [`LinearGradientShader`],
/// [`RadialGradientShader`], [`SweepGradientShader`], [`ImageShader`] and [`CompositeShader`]; painted by a
/// [`Brush`] ([`ShaderBrush`]). Copies share what they are; equal by what they are (an image by being the same
/// bitmap).
#[derive(Clone)]
pub struct Shader {
    data: Arc<ShaderData>,
}

impl PartialEq for Shader {
    fn eq(&self, o: &Shader) -> bool {
        Arc::ptr_eq(&self.data, &o.data) || *self.data == *o.data
    }
}

impl fmt::Debug for Shader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let d = &*self.data;
        let mut s = f.debug_struct("Shader");
        s.field("kind", &d.kind);
        match d.kind {
            ShaderKind::Composite => s.field("dst", &d.dst).field("src", &d.src).field("blend_mode", &d.blend_mode),
            ShaderKind::Linear => s.field("stops", &d.stops).field("from", &d.start).field("to", &d.end),
            ShaderKind::Radial => s.field("stops", &d.stops).field("center", &d.start).field("radius", &d.radius),
            ShaderKind::Sweep => s.field("stops", &d.stops).field("center", &d.start),
            ShaderKind::Image => s.field("image", &d.image),
        };
        if d.kind != ShaderKind::Composite {
            s.field("tile_mode", &d.tile_mode);
        }
        if d.kind == ShaderKind::Image {
            s.field("tile_mode_y", &d.tile_mode_y);
        }
        s.field("transform", &d.transform).finish()
    }
}

fn gradient_shader(
    kind: ShaderKind,
    stops: Vec<ColorStop>,
    start: Offset,
    end: Offset,
    radius: f32,
    tile: TileMode,
) -> Shader {
    Shader {
        data: Arc::new(ShaderData {
            kind,
            stops,
            tile_mode: tile,
            tile_mode_y: TileMode::Clamp,
            image: None,
            start,
            end,
            radius,
            dst: None,
            src: None,
            blend_mode: BlendMode::SrcOver,
            transform: None,
            back: None,
        }),
    }
}

/// A linear gradient from `from` to `to` (Compose's LinearGradientShader): `colors` spread evenly, or at
/// `color_stops` (each 0..1) where given, going on past the ends as `tile_mode` says. Panics, as Compose throws,
/// for fewer than two colors without stops, or stops not as many as the colors.
pub fn LinearGradientShader(
    from: Offset,
    to: Offset,
    colors: &[Color],
    color_stops: Option<&[f32]>,
    tile_mode: TileMode,
) -> Shader {
    validate_color_stops(colors, color_stops);
    gradient_shader(ShaderKind::Linear, stops_of(colors, color_stops), from, to, 0.0, tile_mode)
}

/// A radial gradient round `center` out to `radius` (Compose's RadialGradientShader); as
/// [`LinearGradientShader`] otherwise.
pub fn RadialGradientShader(
    center: Offset,
    radius: f32,
    colors: &[Color],
    color_stops: Option<&[f32]>,
    tile_mode: TileMode,
) -> Shader {
    validate_color_stops(colors, color_stops);
    gradient_shader(ShaderKind::Radial, stops_of(colors, color_stops), center, Offset::Zero, radius, tile_mode)
}

/// A gradient sweeping round `center`, clockwise from 3 o'clock (Compose's SweepGradientShader); as
/// [`LinearGradientShader`] otherwise (its ends clamped).
pub fn SweepGradientShader(center: Offset, colors: &[Color], color_stops: Option<&[f32]>) -> Shader {
    validate_color_stops(colors, color_stops);
    gradient_shader(ShaderKind::Sweep, stops_of(colors, color_stops), center, Offset::Zero, 0.0, TileMode::Clamp)
}

/// `image` from its top left, a pixel a pixel, going on past its edges as `tile_mode_x` says across and
/// `tile_mode_y` down (Compose's ImageShader; Compose's defaults: `TileMode::Clamp` both ways).
pub fn ImageShader(image: ImageBitmap, tile_mode_x: TileMode, tile_mode_y: TileMode) -> Shader {
    Shader {
        data: Arc::new(ShaderData {
            kind: ShaderKind::Image,
            stops: Vec::new(),
            tile_mode: tile_mode_x,
            tile_mode_y,
            image: Some(image),
            start: Offset::Zero,
            end: Offset::Zero,
            radius: 0.0,
            dst: None,
            src: None,
            blend_mode: BlendMode::SrcOver,
            transform: None,
            back: None,
        }),
    }
}

/// `src` composited onto `dst` by `blend_mode`, pixel by pixel (Compose's CompositeShader).
pub fn CompositeShader(dst: Shader, src: Shader, blend_mode: BlendMode) -> Shader {
    Shader {
        data: Arc::new(ShaderData {
            kind: ShaderKind::Composite,
            stops: Vec::new(),
            tile_mode: TileMode::Clamp,
            tile_mode_y: TileMode::Clamp,
            image: None,
            start: Offset::Zero,
            end: Offset::Zero,
            radius: 0.0,
            dst: Some(dst),
            src: Some(src),
            blend_mode,
            transform: None,
            back: None,
        }),
    }
}

impl Shader {
    pub fn kind(&self) -> ShaderKind {
        self.data.kind
    }
    /// A gradient's stops (its colors spread evenly where it was given none).
    pub fn stops(&self) -> &[ColorStop] {
        &self.data.stops
    }
    /// How a gradient goes on past its ends; an image's across.
    pub fn tile_mode(&self) -> TileMode {
        self.data.tile_mode
    }
    /// How an image goes on past its edges down.
    pub fn tile_mode_y(&self) -> TileMode {
        self.data.tile_mode_y
    }
    /// An image shader's image.
    pub fn image(&self) -> Option<&ImageBitmap> {
        self.data.image.as_ref()
    }
    /// Where it lies: a linear gradient's start, a radial one's or a sweep's center.
    pub fn start(&self) -> Offset {
        self.data.start
    }
    /// A linear gradient's end.
    pub fn end(&self) -> Offset {
        self.data.end
    }
    /// A radial gradient's radius.
    pub fn radius(&self) -> f32 {
        self.data.radius
    }
    /// A composite's destination (Compose's dst).
    pub fn dst(&self) -> Option<&Shader> {
        self.data.dst.as_ref()
    }
    /// A composite's source (Compose's src).
    pub fn src(&self) -> Option<&Shader> {
        self.data.src.as_ref()
    }
    /// How a composite's source goes onto its destination.
    pub fn blend_mode(&self) -> BlendMode {
        self.data.blend_mode
    }
    pub fn transform(&self) -> Option<&Matrix> {
        self.data.transform.as_ref()
    }

    /// This through `matrix` (what it painted at p it paints at matrix(p)), after any transform it has (Android's
    /// local matrix).
    pub fn transformed(&self, matrix: &Matrix) -> Shader {
        if matrix.is_identity() {
            return self.clone();
        }
        let mut d = (*self.data).clone();
        let mut m = d.transform.unwrap_or(Matrix::IDENTITY);
        m.times_assign(matrix); // (its own first, then this one)
        let mut back = m;
        back.invert();
        d.transform = Some(m);
        d.back = Some(back);
        Shader { data: Arc::new(d) }
    }

    /// Its color at `p` (straight): an image's bilinearly from its pixels, as Compose's default sampling does.
    pub fn color_at(&self, p: Offset) -> Color {
        let d = &*self.data;
        let p = d.back.map_or(p, |b| b.map(p));
        match d.kind {
            ShaderKind::Linear => {
                let (dx, dy) = (d.end.x - d.start.x, d.end.y - d.start.y);
                let len2 = dx * dx + dy * dy;
                let t = if len2 > 0.0 { ((p.x - d.start.x) * dx + (p.y - d.start.y) * dy) / len2 } else { 0.0 };
                color_along(&d.stops, d.tile_mode, t)
            }
            ShaderKind::Radial => {
                let t = if d.radius > 0.0 { (p.x - d.start.x).hypot(p.y - d.start.y) / d.radius } else { 0.0 };
                color_along(&d.stops, d.tile_mode, t)
            }
            ShaderKind::Sweep => color_along(&d.stops, TileMode::Clamp, sweep_parameter(p, d.start)),
            ShaderKind::Image => match &d.image {
                Some(image) => image_color_at(image, d.tile_mode, d.tile_mode_y, p),
                None => Color::Transparent,
            },
            ShaderKind::Composite => match (&d.src, &d.dst) {
                (Some(src), Some(dst)) => blend(src.color_at(p), dst.color_at(p), d.blend_mode),
                _ => Color::Transparent,
            },
        }
    }
}

/// Where index `i` of `n` falls past an image's edge as `tile` says; none for Decal.
fn tiled(i: i64, n: i64, tile: TileMode) -> Option<i64> {
    if (0..n).contains(&i) {
        return Some(i);
    }
    match tile {
        TileMode::Clamp => Some(i.clamp(0, n - 1)),
        TileMode::Repeated => Some(i.rem_euclid(n)),
        TileMode::Mirror => {
            let m = i.rem_euclid(2 * n);
            Some(if m < n { m } else { 2 * n - 1 - m })
        }
        TileMode::Decal => None,
    }
}

/// `image` at `p` bilinearly, from the four pixels round it (premultiplied: a clear pixel adds no color), each
/// tiled.
fn image_color_at(image: &ImageBitmap, tile_x: TileMode, tile_y: TileMode, p: Offset) -> Color {
    let (w, h) = (image.width() as i64, image.height() as i64);
    let pixels = image.pixels();
    let (u, v) = (p.x - 0.5, p.y - 0.5);
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (u - x0, v - y0);
    // (Rust's `as`: NaN 0, saturated)
    let (x0, y0) = (x0 as i64, y0 as i64);
    let mut sum = [0.0f32; 4];
    for j in 0..2 {
        for i in 0..2 {
            let (Some(x), Some(y)) = (tiled(x0.saturating_add(i), w, tile_x), tiled(y0.saturating_add(j), h, tile_y))
            else {
                continue;
            };
            let at = (y * w + x) as usize * 4;
            let q = &pixels[at..at + 4];
            let weight = (if i == 1 { fx } else { 1.0 - fx }) * (if j == 1 { fy } else { 1.0 - fy });
            let a = q[3] as f32 / 255.0;
            for c in 0..3 {
                sum[c] += q[c] as f32 / 255.0 * a * weight;
            }
            sum[3] += a * weight;
        }
    }
    if sum[3] > 0.0 {
        Color::new(sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3], sum[3])
    } else {
        Color::Transparent
    }
}

// ---- Brushes ---------------------------------------------------------------------------------------------------

/// Offsets equal as Compose's are (packed: bit for bit, an unspecified one equal to itself).
fn same_offset(a: Offset, b: Offset) -> bool {
    a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits()
}

/// A linear gradient's brush (Compose's LinearGradient): `colors` at `stops` (none: evenly) from `start` to `end`
/// in what it paints, an infinite coordinate the far edge.
#[derive(Clone, Debug)]
pub struct LinearGradient {
    pub colors: Vec<Color>,
    pub stops: Option<Vec<f32>>,
    pub start: Offset,
    pub end: Offset,
    pub tile_mode: TileMode,
}

/// A radial gradient's brush (Compose's RadialGradient): round `center` (unspecified: the middle; an infinite
/// coordinate the far edge) out to `radius` (infinite: half the shorter side).
#[derive(Clone, Debug)]
pub struct RadialGradient {
    pub colors: Vec<Color>,
    pub stops: Option<Vec<f32>>,
    pub center: Offset,
    pub radius: f32,
    pub tile_mode: TileMode,
}

/// A sweep gradient's brush (Compose's SweepGradient): round `center` (unspecified: the middle), clockwise from 3
/// o'clock.
#[derive(Clone, Debug)]
pub struct SweepGradient {
    pub center: Offset,
    pub colors: Vec<Color>,
    pub stops: Option<Vec<f32>>,
}

/// By what they are, as Compose's equals.
impl PartialEq for LinearGradient {
    fn eq(&self, o: &LinearGradient) -> bool {
        self.colors == o.colors
            && self.stops == o.stops
            && same_offset(self.start, o.start)
            && same_offset(self.end, o.end)
            && self.tile_mode == o.tile_mode
    }
}
impl PartialEq for RadialGradient {
    fn eq(&self, o: &RadialGradient) -> bool {
        self.colors == o.colors
            && self.stops == o.stops
            && same_offset(self.center, o.center)
            && self.radius == o.radius
            && self.tile_mode == o.tile_mode
    }
}
impl PartialEq for SweepGradient {
    fn eq(&self, o: &SweepGradient) -> bool {
        same_offset(self.center, o.center) && self.colors == o.colors && self.stops == o.stops
    }
}

/// Two brushes composited (Compose's CompositeShaderBrush, made by [`Brush::composite`]): each made a shader for
/// the size painted, `src_brush`'s onto `dst_brush`'s by `blend_mode`. Equal by what it composites.
#[derive(Clone, PartialEq, Debug)]
pub struct CompositeShaderBrush {
    pub dst_brush: Arc<Brush>,
    pub src_brush: Arc<Brush>,
    pub blend_mode: BlendMode,
}

type MakeShader = dyn Fn(Size) -> Shader + Send + Sync;

/// A brush of one's own (Compose's ShaderBrush implemented): the shader its maker makes for the size of what it
/// paints (made again only for another size, as Compose's), or one shader whatever the size. Copies share it; one
/// shader whatever the size is equal by it, a maker only to itself.
#[derive(Clone)]
pub struct ShaderBrushSource {
    make: Arc<MakeShader>,
    fixed: Option<Shader>,
    intrinsic_size: Size,
    made: Arc<Mutex<Option<(Size, Shader)>>>,
}

impl ShaderBrushSource {
    /// The shader for `size`, made only if it wasn't made for that size last.
    pub fn create_shader(&self, size: Size) -> Shader {
        let mut made = self.made.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((for_size, shader)) = &*made
            && *for_size == size
        {
            return shader.clone();
        }
        let shader = (self.make)(size);
        *made = Some((size, shader.clone()));
        shader
    }
    /// The one shader it paints whatever the size, if it is one.
    pub fn fixed_shader(&self) -> Option<&Shader> {
        self.fixed.as_ref()
    }
}

impl PartialEq for ShaderBrushSource {
    fn eq(&self, o: &ShaderBrushSource) -> bool {
        match (&self.fixed, &o.fixed) {
            (None, None) => Arc::ptr_eq(&self.make, &o.make),
            (a, b) => a == b,
        }
    }
}

impl fmt::Debug for ShaderBrushSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.fixed {
            Some(shader) => write!(f, "ShaderBrush({shader:?})"),
            None => write!(f, "ShaderBrush({:p})", Arc::as_ptr(&self.make)),
        }
    }
}

/// What a brush is.
#[derive(Clone, PartialEq, Debug)]
pub enum BrushKind {
    SolidColor(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    SweepGradient(SweepGradient),
    Composite(CompositeShaderBrush),
    Shader(ShaderBrushSource),
}

/// What paints (Compose's Brush): a color ([`SolidColor`]), a gradient, a shader of one's own ([`ShaderBrush`],
/// [`Brush::shader_brush`]) or two composited ([`Brush::composite`]); all but a color through a transform if they
/// have one (Compose's ShaderBrush.transform). Gradient coordinates are in what it paints, from its top left; an
/// infinite one is that side's far edge.
#[derive(Clone)]
pub struct Brush {
    kind: BrushKind,
    transform: Option<Matrix>,
    shader_alpha: f32,
}

impl PartialEq for Brush {
    fn eq(&self, o: &Brush) -> bool {
        self.kind == o.kind && self.transform == o.transform && self.shader_alpha == o.shader_alpha
    }
}

/// A list as Kotlin writes it.
struct KList<'a, T>(&'a [T]);

impl<T: fmt::Debug> fmt::Debug for KList<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("[")?;
        for (i, v) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{v:?}")?;
        }
        f.write_str("]")
    }
}

fn stops_text(stops: &Option<Vec<f32>>) -> String {
    stops.as_ref().map_or("null".into(), |s| format!("{:?}", KList(s)))
}

/// Compose's toString of each.
impl fmt::Debug for Brush {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            BrushKind::SolidColor(c) => write!(f, "SolidColor(value={c:?})"),
            BrushKind::LinearGradient(g) => {
                write!(f, "LinearGradient(colors={:?}, stops={}, ", KList(&g.colors), stops_text(&g.stops))?;
                if g.start.is_finite() {
                    write!(f, "start={:?}, ", g.start)?;
                }
                if g.end.is_finite() {
                    write!(f, "end={:?}, ", g.end)?;
                }
                write!(f, "tileMode={:?})", g.tile_mode)
            }
            BrushKind::RadialGradient(g) => {
                write!(f, "RadialGradient(colors={:?}, stops={}, ", KList(&g.colors), stops_text(&g.stops))?;
                if g.center.is_specified() {
                    write!(f, "center={:?}, ", g.center)?;
                }
                if g.radius.is_finite() {
                    write!(f, "radius={:?}, ", g.radius)?;
                }
                write!(f, "tileMode={:?})", g.tile_mode)
            }
            BrushKind::SweepGradient(g) => {
                f.write_str("SweepGradient(")?;
                if g.center.is_specified() {
                    write!(f, "center={:?}, ", g.center)?;
                }
                write!(f, "colors={:?}, stops={})", KList(&g.colors), stops_text(&g.stops))
            }
            BrushKind::Composite(c) => write!(
                f,
                "CompositeShaderBrush(dstBrush={:?}, srcBrush={:?}, blendMode={:?})",
                c.dst_brush, c.src_brush, c.blend_mode
            ),
            BrushKind::Shader(s) => write!(f, "{s:?}"),
        }?;
        if let Some(m) = &self.transform {
            write!(f, ".transform({m:?})")?;
        }
        if self.shader_alpha != 1.0 {
            write!(f, ".faded({})", self.shader_alpha)?;
        }
        Ok(())
    }
}

/// A brush of one color (Compose's SolidColor).
pub fn SolidColor(value: Color) -> Brush {
    Brush::of(BrushKind::SolidColor(value))
}

/// A brush painting `shader` whatever the size (Compose's ShaderBrush(shader)); equal by it.
pub fn ShaderBrush(shader: Shader) -> Brush {
    let fixed = shader.clone();
    Brush::of(BrushKind::Shader(ShaderBrushSource {
        make: Arc::new(move |_| fixed.clone()),
        fixed: Some(shader),
        intrinsic_size: Size::Unspecified,
        made: Arc::new(Mutex::new(None)),
    }))
}

impl From<Color> for Brush {
    /// [`SolidColor`].
    fn from(color: Color) -> Brush {
        SolidColor(color)
    }
}

/// `offset to color` pairs as colors and stops.
fn split_stops(color_stops: &[(f32, Color)]) -> (Vec<Color>, Option<Vec<f32>>) {
    (color_stops.iter().map(|s| s.1).collect(), Some(color_stops.iter().map(|s| s.0).collect()))
}

/// Where a gradient lies when it paints an area, the far edges and the default centers resolved (what a backend
/// draws it from): a linear one from `start` to `end`; a radial one round `start` out to `radius`; a sweep round
/// `start`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct BrushGeometry {
    pub start: Offset,
    pub end: Offset,
    pub radius: f32,
}

/// A coordinate, the far edge where it is (positive) infinite.
fn or_far(v: f32, size: f32) -> f32 {
    if v == f32::INFINITY { size } else { v }
}

impl Brush {
    fn of(kind: BrushKind) -> Brush {
        Brush { kind, transform: None, shader_alpha: 1.0 }
    }

    /// A linear gradient of `colors` spread evenly from `start` to `end` (Compose's Brush.linearGradient; Compose's
    /// defaults: from `Offset::Zero` to `Offset::Infinite`, the far corner, `TileMode::Clamp`).
    ///
    /// Kotlin: `linearGradient(colors: List<Color>, start: Offset = Offset.Zero, end: Offset = Offset.Infinite, tileMode: TileMode = TileMode.Clamp)`
    pub fn linear_gradient(colors: &[Color], start: Offset, end: Offset, tile_mode: TileMode) -> Brush {
        Brush::of(BrushKind::LinearGradient(LinearGradient {
            colors: colors.to_vec(),
            stops: None,
            start,
            end,
            tile_mode,
        }))
    }
    /// The same through `offset to color` stops.
    pub fn linear_gradient_stops(
        color_stops: &[(f32, Color)],
        start: Offset,
        end: Offset,
        tile_mode: TileMode,
    ) -> Brush {
        let (colors, stops) = split_stops(color_stops);
        Brush::of(BrushKind::LinearGradient(LinearGradient { colors, stops, start, end, tile_mode }))
    }
    /// Left to right from `start_x` to `end_x` (Compose's Brush.horizontalGradient; Compose's defaults: 0 to
    /// infinity, the right edge).
    pub fn horizontal_gradient(colors: &[Color], start_x: f32, end_x: f32, tile_mode: TileMode) -> Brush {
        Brush::linear_gradient(colors, Offset::new(start_x, 0.0), Offset::new(end_x, 0.0), tile_mode)
    }
    pub fn horizontal_gradient_stops(
        color_stops: &[(f32, Color)],
        start_x: f32,
        end_x: f32,
        tile_mode: TileMode,
    ) -> Brush {
        Brush::linear_gradient_stops(color_stops, Offset::new(start_x, 0.0), Offset::new(end_x, 0.0), tile_mode)
    }
    /// Top to bottom from `start_y` to `end_y` (Compose's Brush.verticalGradient; Compose's defaults: 0 to
    /// infinity, the bottom edge).
    pub fn vertical_gradient(colors: &[Color], start_y: f32, end_y: f32, tile_mode: TileMode) -> Brush {
        Brush::linear_gradient(colors, Offset::new(0.0, start_y), Offset::new(0.0, end_y), tile_mode)
    }
    pub fn vertical_gradient_stops(
        color_stops: &[(f32, Color)],
        start_y: f32,
        end_y: f32,
        tile_mode: TileMode,
    ) -> Brush {
        Brush::linear_gradient_stops(color_stops, Offset::new(0.0, start_y), Offset::new(0.0, end_y), tile_mode)
    }
    /// Round `center` out to `radius` (Compose's Brush.radialGradient; Compose's defaults: `Offset::Unspecified`,
    /// the middle, and infinity, half the shorter side).
    pub fn radial_gradient(colors: &[Color], center: Offset, radius: f32, tile_mode: TileMode) -> Brush {
        Brush::of(BrushKind::RadialGradient(RadialGradient {
            colors: colors.to_vec(),
            stops: None,
            center,
            radius,
            tile_mode,
        }))
    }
    pub fn radial_gradient_stops(
        color_stops: &[(f32, Color)],
        center: Offset,
        radius: f32,
        tile_mode: TileMode,
    ) -> Brush {
        let (colors, stops) = split_stops(color_stops);
        Brush::of(BrushKind::RadialGradient(RadialGradient { colors, stops, center, radius, tile_mode }))
    }
    /// Round `center` (unspecified: the middle), clockwise from 3 o'clock (Compose's Brush.sweepGradient).
    pub fn sweep_gradient(colors: &[Color], center: Offset) -> Brush {
        Brush::of(BrushKind::SweepGradient(SweepGradient { center, colors: colors.to_vec(), stops: None }))
    }
    pub fn sweep_gradient_stops(color_stops: &[(f32, Color)], center: Offset) -> Brush {
        let (colors, stops) = split_stops(color_stops);
        Brush::of(BrushKind::SweepGradient(SweepGradient { center, colors, stops }))
    }
    /// `src_brush` composited onto `dst_brush` by `blend_mode`, pixel by pixel (Compose's Brush.composite: each
    /// made a shader brush, a color as a vertical gradient of it).
    pub fn composite(dst_brush: &Brush, src_brush: &Brush, blend_mode: BlendMode) -> Brush {
        Brush::of(BrushKind::Composite(CompositeShaderBrush {
            dst_brush: Arc::new(dst_brush.to_shader_brush()),
            src_brush: Arc::new(src_brush.to_shader_brush()),
            blend_mode,
        }))
    }
    /// A brush of one's own (Compose's ShaderBrush implemented): `create_shader` makes its shader for the size of
    /// what it paints, once for each size in a row; its intrinsic size (unspecified: none). Equal only to itself
    /// and its copies.
    pub fn shader_brush(create_shader: impl Fn(Size) -> Shader + Send + Sync + 'static, intrinsic_size: Size) -> Brush {
        Brush::of(BrushKind::Shader(ShaderBrushSource {
            make: Arc::new(create_shader),
            fixed: None,
            intrinsic_size,
            made: Arc::new(Mutex::new(None)),
        }))
    }

    /// A color as a shader brush (Compose's toShaderBrush: a vertical gradient of the one color).
    fn to_shader_brush(&self) -> Brush {
        match &self.kind {
            BrushKind::SolidColor(c) => {
                let mut b = Brush::vertical_gradient(&[*c, *c], 0.0, f32::INFINITY, TileMode::Clamp);
                b.shader_alpha = self.shader_alpha;
                b
            }
            _ => self.clone(),
        }
    }

    pub fn kind(&self) -> &BrushKind {
        &self.kind
    }
    pub fn is_solid(&self) -> bool {
        matches!(self.kind, BrushKind::SolidColor(_))
    }
    /// A color's color.
    pub fn solid_color(&self) -> Option<Color> {
        match self.kind {
            BrushKind::SolidColor(c) => Some(c),
            _ => None,
        }
    }

    /// A gradient's colors at their places (spread evenly where it was given none); a color's, the one.
    pub fn color_stops(&self) -> Vec<ColorStop> {
        match &self.kind {
            BrushKind::SolidColor(c) => vec![ColorStop::new(0.0, *c)],
            BrushKind::LinearGradient(g) => stops_of(&g.colors, g.stops.as_deref()),
            BrushKind::RadialGradient(g) => stops_of(&g.colors, g.stops.as_deref()),
            BrushKind::SweepGradient(g) => stops_of(&g.colors, g.stops.as_deref()),
            BrushKind::Composite(_) | BrushKind::Shader(_) => Vec::new(),
        }
    }

    /// How a gradient goes on past its ends (a sweep's and the others' clamped).
    pub fn tile_mode(&self) -> TileMode {
        match &self.kind {
            BrushKind::LinearGradient(g) => g.tile_mode,
            BrushKind::RadialGradient(g) => g.tile_mode,
            _ => TileMode::Clamp,
        }
    }

    /// Its size where it has one (Compose's intrinsicSize): a linear gradient's from its start to its end where
    /// those are finite (NaN across or down where not), a radial one's diameter, a shader brush's own; unspecified
    /// for the others.
    pub fn intrinsic_size(&self) -> Size {
        match &self.kind {
            BrushKind::LinearGradient(g) => Size::new(
                if g.start.x.is_finite() && g.end.x.is_finite() { (g.start.x - g.end.x).abs() } else { f32::NAN },
                if g.start.y.is_finite() && g.end.y.is_finite() { (g.start.y - g.end.y).abs() } else { f32::NAN },
            ),
            BrushKind::RadialGradient(g) if g.radius.is_finite() => Size::new(g.radius * 2.0, g.radius * 2.0),
            BrushKind::Shader(s) => s.intrinsic_size,
            _ => Size::Unspecified,
        }
    }

    /// Its transform (Compose's ShaderBrush.transform).
    pub fn transform(&self) -> Option<&Matrix> {
        self.transform.as_ref()
    }
    /// This brush with its shader through `transform` (Compose's ShaderBrush.transform: what it paints at p it
    /// paints at transform(p), in what it paints from its top left). A color has none; the identity is none.
    pub fn with_transform(&self, transform: Option<Matrix>) -> Brush {
        let mut b = self.clone();
        b.transform = transform.filter(|m| !m.is_identity() && !self.is_solid());
        b
    }

    /// How opaque what a shader brush paints is (a paint's alpha it was faded by).
    pub fn shader_alpha(&self) -> f32 {
        self.shader_alpha
    }
    /// Each of its colors `alpha` times as opaque (as a paint's alpha fades what it paints): a color's and a
    /// gradient's colors, a shader brush's [`shader_alpha`](Brush::shader_alpha).
    pub fn faded(&self, alpha: f32) -> Brush {
        let mut b = self.clone();
        if alpha >= 1.0 {
            return b;
        }
        let k = alpha.max(0.0);
        let fade = |colors: &mut Vec<Color>| {
            for c in colors.iter_mut() {
                *c = with_alpha(*c, c.alpha() * k);
            }
        };
        match &mut b.kind {
            BrushKind::SolidColor(c) => *c = with_alpha(*c, c.alpha() * k),
            BrushKind::LinearGradient(g) => fade(&mut g.colors),
            BrushKind::RadialGradient(g) => fade(&mut g.colors),
            BrushKind::SweepGradient(g) => fade(&mut g.colors),
            BrushKind::Composite(_) | BrushKind::Shader(_) => b.shader_alpha *= k,
        }
        b
    }

    /// The shader it paints `size` with (Compose's ShaderBrush.createShader), through its transform: a gradient's
    /// where it lies on that size; a color's a vertical gradient of it; a composite's two composited. Panics, as
    /// Compose's gradient shaders throw, for a gradient of fewer than two colors.
    pub fn create_shader(&self, size: Size) -> Shader {
        let shader = match &self.kind {
            BrushKind::SolidColor(c) => {
                LinearGradientShader(Offset::Zero, Offset::new(0.0, size.height), &[*c, *c], None, TileMode::Clamp)
            }
            BrushKind::LinearGradient(g) => {
                let start = Offset::new(or_far(g.start.x, size.width), or_far(g.start.y, size.height));
                let end = Offset::new(or_far(g.end.x, size.width), or_far(g.end.y, size.height));
                LinearGradientShader(start, end, &g.colors, g.stops.as_deref(), g.tile_mode)
            }
            BrushKind::RadialGradient(g) => {
                let center = if g.center.is_unspecified() {
                    size.center()
                } else {
                    Offset::new(or_far(g.center.x, size.width), or_far(g.center.y, size.height))
                };
                let radius = if g.radius == f32::INFINITY { size.min_dimension() / 2.0 } else { g.radius };
                RadialGradientShader(center, radius, &g.colors, g.stops.as_deref(), g.tile_mode)
            }
            BrushKind::SweepGradient(g) => {
                let center = if g.center.is_unspecified() {
                    size.center()
                } else {
                    Offset::new(or_far(g.center.x, size.width), or_far(g.center.y, size.height))
                };
                SweepGradientShader(center, &g.colors, g.stops.as_deref())
            }
            BrushKind::Composite(c) => {
                CompositeShader(c.dst_brush.create_shader(size), c.src_brush.create_shader(size), c.blend_mode)
            }
            BrushKind::Shader(s) => s.create_shader(size),
        };
        match &self.transform {
            Some(m) => shader.transformed(m),
            None => shader,
        }
    }

    /// Where it lies when painting `area` (absolute), the far edges and the default centers resolved, in absolute
    /// points (a color's and a shader brush's: nothing).
    pub fn geometry(&self, area: Rect) -> BrushGeometry {
        let (size, corner) = (area.size(), area.top_left());
        let centered = |c: Offset| {
            if c.is_unspecified() {
                corner + size.center()
            } else {
                corner + Offset::new(or_far(c.x, size.width), or_far(c.y, size.height))
            }
        };
        match &self.kind {
            BrushKind::LinearGradient(g) => BrushGeometry {
                start: corner + Offset::new(or_far(g.start.x, size.width), or_far(g.start.y, size.height)),
                end: corner + Offset::new(or_far(g.end.x, size.width), or_far(g.end.y, size.height)),
                radius: 0.0,
            },
            BrushKind::RadialGradient(g) => BrushGeometry {
                start: centered(g.center),
                end: Offset::Zero,
                radius: if g.radius == f32::INFINITY { size.min_dimension() / 2.0 } else { g.radius },
            },
            BrushKind::SweepGradient(g) => BrushGeometry { start: centered(g.center), ..BrushGeometry::default() },
            _ => BrushGeometry::default(),
        }
    }

    /// Where absolute point `p` is along the gradient painting `area` (absolute), before its tile mode, through its
    /// transform: 0 at its start, 1 at its end (a sweep's 0..1 round its center).
    pub fn parameter_at(&self, mut p: Offset, area: Rect) -> f32 {
        if let Some(m) = &self.transform {
            // where the transform takes p from, in what it paints
            let mut back = *m;
            back.invert();
            p = area.top_left() + back.map(p - area.top_left());
        }
        let g = self.geometry(area);
        match &self.kind {
            BrushKind::LinearGradient(_) => {
                let (dx, dy) = (g.end.x - g.start.x, g.end.y - g.start.y);
                let len2 = dx * dx + dy * dy;
                if len2 > 0.0 { ((p.x - g.start.x) * dx + (p.y - g.start.y) * dy) / len2 } else { 0.0 }
            }
            BrushKind::RadialGradient(_) => {
                if g.radius > 0.0 {
                    (p.x - g.start.x).hypot(p.y - g.start.y) / g.radius
                } else {
                    0.0
                }
            }
            BrushKind::SweepGradient(_) => sweep_parameter(p, g.start),
            _ => 0.0,
        }
    }

    /// The color that far along it, its tile mode applied (a color's: the color).
    pub fn color_at_parameter(&self, t: f32) -> Color {
        match &self.kind {
            BrushKind::SolidColor(c) => *c,
            _ => color_along(&self.color_stops(), self.tile_mode(), t),
        }
    }

    /// The color it paints at absolute point `p` when painting `area` (absolute), through its transform.
    pub fn color_at(&self, p: Offset, area: Rect) -> Color {
        let shaded = matches!(self.kind, BrushKind::Composite(_) | BrushKind::Shader(_));
        if shaded || (self.transform.is_some() && !self.is_solid()) {
            let c = self.create_shader(area.size()).color_at(p - area.top_left());
            return with_alpha(c, c.alpha() * self.shader_alpha);
        }
        self.color_at_parameter(self.parameter_at(p, area))
    }

    /// This `fraction` of the way to `other` (Compose's Interpolatable.lerp of each brush): a color to a color, a
    /// gradient to one of its kind (colors and stops index by index, the shorter's last again; ends, centers and
    /// radii mixed; the tile mode switching half way), a gradient to a color as to itself in that color; none
    /// (`other`) as to transparent. `None` where it can't be.
    pub fn lerp(&self, other: Option<&Brush>, fraction: f32) -> Option<Brush> {
        let t = fraction;
        let transparent = SolidColor(Color::Transparent);
        let other = other.unwrap_or(&transparent);
        // (a color to a gradient: as the gradient in that color)
        let solid = other.solid_color();
        let mixed = |a: &[Color], b: &[Color]| lerp_color_list(a, b, t);
        let kind = match (&self.kind, &other.kind) {
            (BrushKind::SolidColor(a), BrushKind::SolidColor(b)) => BrushKind::SolidColor(lerp_color(*a, *b, t)),
            (BrushKind::SolidColor(_), _) => return None,
            (BrushKind::LinearGradient(g), _) => {
                let o = match &other.kind {
                    BrushKind::LinearGradient(o) => o.clone(),
                    _ => LinearGradient { colors: vec![solid?; g.colors.len()], ..g.clone() },
                };
                BrushKind::LinearGradient(LinearGradient {
                    colors: mixed(&g.colors, &o.colors),
                    stops: lerp_nullable_float_list(&g.stops, &o.stops, t),
                    start: lerp_safe(g.start, o.start, t),
                    end: lerp_safe(g.end, o.end, t),
                    tile_mode: if t < 0.5 { g.tile_mode } else { o.tile_mode },
                })
            }
            (BrushKind::RadialGradient(g), _) => {
                let o = match &other.kind {
                    BrushKind::RadialGradient(o) => o.clone(),
                    _ => RadialGradient { colors: vec![solid?; g.colors.len()], ..g.clone() },
                };
                BrushKind::RadialGradient(RadialGradient {
                    colors: mixed(&g.colors, &o.colors),
                    stops: lerp_nullable_float_list(&g.stops, &o.stops, t),
                    center: lerp_offset(g.center, o.center, t),
                    radius: lerp(g.radius, o.radius, t),
                    tile_mode: if t < 0.5 { g.tile_mode } else { o.tile_mode },
                })
            }
            (BrushKind::SweepGradient(g), _) => {
                let o = match &other.kind {
                    BrushKind::SweepGradient(o) => o.clone(),
                    _ => SweepGradient { colors: vec![solid?; g.colors.len()], ..g.clone() },
                };
                BrushKind::SweepGradient(SweepGradient {
                    center: lerp_offset(g.center, o.center, t),
                    colors: mixed(&g.colors, &o.colors),
                    stops: lerp_nullable_float_list(&g.stops, &o.stops, t),
                })
            }
            (BrushKind::Composite(_) | BrushKind::Shader(_), _) => return None,
        };
        // (Compose makes a new brush: no transform)
        Some(Brush::of(kind))
    }
}

/// Two lists as long as the longer, index by index, the shorter's last again past its end (Compose's
/// lerpColorList).
fn lerp_color_list(left: &[Color], right: &[Color], t: f32) -> Vec<Color> {
    (0..left.len().max(right.len()))
        .map(|i| lerp_color(left[i.min(left.len() - 1)], right[i.min(right.len() - 1)], t))
        .collect()
}

/// Compose's lerpNullableFloatList: none unless both have stops.
fn lerp_nullable_float_list(left: &Option<Vec<f32>>, right: &Option<Vec<f32>>, t: f32) -> Option<Vec<f32>> {
    let (l, r) = (left.as_ref()?, right.as_ref()?);
    Some((0..l.len().max(r.len())).map(|i| lerp(l[i.min(l.len() - 1)], r[i.min(r.len() - 1)], t)).collect())
}

/// Compose's lerpSafe: mixed where both are finite, else switching half way.
fn lerp_safe(left: Offset, right: Offset, t: f32) -> Offset {
    if left.is_finite() && right.is_finite() {
        lerp_offset(left, right, t)
    } else if t < 0.5 {
        left
    } else {
        right
    }
}

/// Between two brushes (Compose's Interpolatable.lerp): equal ones switch half way; else `start`'s lerp to `stop`,
/// or `stop`'s back to `start`, or (neither can) a switch half way.
pub fn lerp_brush(start: &Brush, stop: &Brush, fraction: f32) -> Brush {
    if start == stop {
        return if fraction < 0.5 { start.clone() } else { stop.clone() };
    }
    start
        .lerp(Some(stop), fraction)
        .or_else(|| stop.lerp(Some(start), 1.0 - fraction))
        .unwrap_or_else(|| if fraction < 0.5 { start.clone() } else { stop.clone() })
}

// (WP5: Kotlin's Brush.linearGradient(colors) with its defaults, the doc line's)
impl Brush {
    /// [`Brush::linear_gradient`] of a list of colors (Compose's `Brush.linearGradient(colors: List<Color>, …)`,
    /// what the Kotlin syntax calls: Kotlin's defaults, from the top left to the far corner, clamped, come from the
    /// doc line).
    ///
    /// Kotlin: `linearGradient(colors: List<Color>, start: Offset = Offset.Zero, end: Offset = Offset.Infinite, tileMode: TileMode = TileMode.Clamp)`
    pub fn linear_gradient_of(colors: Vec<Color>, start: Offset, end: Offset, tile_mode: TileMode) -> Brush {
        Brush::linear_gradient(&colors, start, end, tile_mode)
    }
}

// ---- Border strokes --------------------------------------------------------------------------------------------

/// A border's width and paint (Compose's BorderStroke), for a border round a shape.
#[derive(Clone, PartialEq, Debug)]
pub struct BorderStroke {
    pub width: Dp,
    pub brush: Brush,
}

/// `BorderStroke(1.dp(), Color::Red)` or `BorderStroke(2.dp(), brush)` (Compose's two constructors).
pub fn BorderStroke(width: Dp, brush: impl Into<Brush>) -> BorderStroke {
    BorderStroke { width, brush: brush.into() }
}

#[named]
impl BorderStroke {
    /// Another width or brush, the rest its own (Compose's copy).
    pub fn copy(&self, #[default(self.width)] width: Dp, #[default(self.brush.clone())] brush: Brush) -> BorderStroke {
        BorderStroke { width, brush }
    }
}

// ---- Vertices --------------------------------------------------------------------------------------------------

/// A triangle mesh for a canvas's drawVertices (Compose's Vertices): where each vertex is, where it samples the
/// paint's shader (its texture coordinate) and its color, in the order `indices` give (none: as they come), kept as
/// Compose keeps them (x, y pairs, ARGB colors, 16 bit indices).
#[derive(Clone, PartialEq, Debug)]
pub struct Vertices {
    vertex_mode: VertexMode,
    positions: Vec<f32>,
    texture_coordinates: Vec<f32>,
    colors: Vec<u32>,
    indices: Vec<i16>,
}

fn encode_points(points: &[Offset]) -> Vec<f32> {
    points.iter().flat_map(|p| [p.x, p.y]).collect()
}

impl Vertices {
    /// Panics, as Compose throws, unless there are as many texture coordinates and colors as positions and every
    /// index is one of a position.
    pub fn new(
        vertex_mode: VertexMode,
        positions: &[Offset],
        texture_coordinates: &[Offset],
        colors: &[Color],
        indices: &[i32],
    ) -> Vertices {
        if texture_coordinates.len() != positions.len() {
            panic!("positions and textureCoordinates lengths must match.");
        }
        if colors.len() != positions.len() {
            panic!("positions and colors lengths must match.");
        }
        if indices.iter().any(|&i| i < 0 || i as usize >= positions.len()) {
            panic!("indices values must be valid indices in the positions list.");
        }
        Vertices {
            vertex_mode,
            positions: encode_points(positions),
            texture_coordinates: encode_points(texture_coordinates),
            colors: colors.iter().map(|c| c.to_argb()).collect(),
            indices: indices.iter().map(|&i| i as i16).collect(),
        }
    }

    /// From arrays: x, y pairs (an odd one left over is left out) and ARGB colors; checked as [`Vertices::new`].
    pub fn from_arrays(
        vertex_mode: VertexMode,
        positions: &[f32],
        texture_coordinates: &[f32],
        colors: &[u32],
        indices: &[i16],
    ) -> Vertices {
        let points = |v: &[f32]| v.as_chunks::<2>().0.iter().map(|&[x, y]| Offset::new(x, y)).collect::<Vec<_>>();
        let colors: Vec<Color> = colors.iter().map(|&c| Color::from_argb(c)).collect();
        let indices: Vec<i32> = indices.iter().map(|&i| i as i32).collect();
        Vertices::new(vertex_mode, &points(positions), &points(texture_coordinates), &colors, &indices)
    }

    pub fn vertex_mode(&self) -> VertexMode {
        self.vertex_mode
    }
    /// x, y pairs.
    pub fn positions(&self) -> &[f32] {
        &self.positions
    }
    /// x, y pairs.
    pub fn texture_coordinates(&self) -> &[f32] {
        &self.texture_coordinates
    }
    /// ARGB.
    pub fn colors(&self) -> &[u32] {
        &self.colors
    }
    pub fn indices(&self) -> &[i16] {
        &self.indices
    }
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 2
    }
    pub fn position(&self, i: usize) -> Offset {
        Offset::new(self.positions[2 * i], self.positions[2 * i + 1])
    }
    pub fn texture_coordinate(&self, i: usize) -> Offset {
        Offset::new(self.texture_coordinates[2 * i], self.texture_coordinates[2 * i + 1])
    }

    /// Its triangles, each three indices into its positions, in the order they are drawn.
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        let n = if self.indices.is_empty() { self.vertex_count() } else { self.indices.len() };
        let at = |i: usize| if self.indices.is_empty() { i } else { self.indices[i] as usize };
        let tri = |a: usize, b: usize, c: usize| [at(a), at(b), at(c)];
        match self.vertex_mode {
            VertexMode::Triangles => (0..n / 3).map(|k| tri(3 * k, 3 * k + 1, 3 * k + 2)).collect(),
            VertexMode::TriangleStrip => (0..n.saturating_sub(2)).map(|i| tri(i, i + 1, i + 2)).collect(),
            VertexMode::TriangleFan => (1..n.saturating_sub(1)).map(|i| tri(0, i, i + 1)).collect(),
        }
    }

    /// Its positions' bounds.
    pub fn bounds(&self) -> Rect {
        let mut points = (0..self.vertex_count()).map(|i| self.position(i));
        let Some(first) = points.next() else { return Rect::Zero };
        points.fold(Rect::new(first.x, first.y, first.x, first.y), |r, p| {
            Rect::new(r.left.min(p.x), r.top.min(p.y), r.right.max(p.x), r.bottom.max(p.y))
        })
    }
}

// ---- Color matrices and color filters --------------------------------------------------------------------------

/// A 4x5 matrix over a color's red, green, blue and alpha (Compose's ColorMatrix, row by row): each row's first
/// four times the color's channels in 0..255, plus its fifth (so the fifth column is in 0..255, as Android's).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorMatrix {
    pub values: [f32; 20],
}

impl Default for ColorMatrix {
    fn default() -> ColorMatrix {
        ColorMatrix::IDENTITY
    }
}

impl ColorMatrix {
    pub const IDENTITY: ColorMatrix = ColorMatrix {
        values: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    };

    /// The identity.
    pub fn new() -> ColorMatrix {
        ColorMatrix::IDENTITY
    }
    /// Its 20 values, four rows of five (Compose's ColorMatrix(floatArrayOf(…))).
    pub const fn from_values(values: [f32; 20]) -> ColorMatrix {
        ColorMatrix { values }
    }
    /// Up to 20 values, the rest the identity's.
    pub fn from_slice(values: &[f32]) -> ColorMatrix {
        let mut m = ColorMatrix::IDENTITY;
        let n = values.len().min(20);
        m.values[..n].copy_from_slice(&values[..n]);
        m
    }

    pub fn get(&self, row: usize, column: usize) -> f32 {
        self.values[row * 5 + column]
    }
    pub fn set(&mut self, row: usize, column: usize, v: f32) {
        self.values[row * 5 + column] = v;
    }
    /// The identity again.
    pub fn reset(&mut self) {
        *self = ColorMatrix::IDENTITY;
    }
    /// `src`'s values (Compose's set(src)).
    pub fn set_from(&mut self, src: &ColorMatrix) {
        self.values = src.values;
    }

    /// The color's channels (0..255) through the matrix, each kept to 0..255, as Android applies it: taken in
    /// sRGB, straight.
    pub fn apply(&self, color: Color) -> Color {
        let c = color.in_srgb();
        let input = [c.red(), c.green(), c.blue(), c.alpha()];
        let mut out = [0.0f32; 4];
        for (row, o) in out.iter_mut().enumerate() {
            let mut v = self.get(row, 4) / 255.0;
            for (k, &x) in input.iter().enumerate() {
                v += self.get(row, k) * x;
            }
            *o = v.clamp(0.0, 1.0);
        }
        Color::new(out[0], out[1], out[2], out[3])
    }

    /// This, after `other`'s change (Compose's timesAssign: this times `other`).
    pub fn times_assign(&mut self, other: &ColorMatrix) {
        let (a, b) = (*self, other);
        let dot = |row: usize, column: usize| {
            a.get(row, 0) * b.get(0, column)
                + a.get(row, 1) * b.get(1, column)
                + a.get(row, 2) * b.get(2, column)
                + a.get(row, 3) * b.get(3, column)
        };
        for row in 0..4 {
            for column in 0..4 {
                self.set(row, column, dot(row, column));
            }
            let v4 = a.get(row, 0) * b.get(0, 4)
                + a.get(row, 1) * b.get(1, 4)
                + a.get(row, 2) * b.get(2, 4)
                + a.get(row, 3) * b.get(3, 4)
                + a.get(row, 4);
            self.set(row, 4, v4);
        }
    }

    /// Saturation `sat`: 0 gray (Rec. 709's luminance, as Android's), 1 as it is.
    pub fn set_to_saturation(&mut self, sat: f32) {
        self.reset();
        let inv_sat = 1.0 - sat;
        let (r, g, b) = (0.213 * inv_sat, 0.715 * inv_sat, 0.072 * inv_sat);
        self.set(0, 0, r + sat);
        self.set(0, 1, g);
        self.set(0, 2, b);
        self.set(1, 0, r);
        self.set(1, 1, g + sat);
        self.set(1, 2, b);
        self.set(2, 0, r);
        self.set(2, 1, g);
        self.set(2, 2, b + sat);
    }

    /// Each channel scaled.
    pub fn set_to_scale(&mut self, red_scale: f32, green_scale: f32, blue_scale: f32, alpha_scale: f32) {
        self.reset();
        self.set(0, 0, red_scale);
        self.set(1, 1, green_scale);
        self.set(2, 2, blue_scale);
        self.set(3, 3, alpha_scale);
    }

    /// The identity, then two channels turned by `degrees` (Compose's rotateInternal, its sine and cosine
    /// approximated as Compose's normalizedAngleSin).
    fn rotate(&mut self, degrees: f32, set: impl FnOnce(&mut ColorMatrix, f32, f32)) {
        self.reset();
        let normalized = degrees * (1.0 / 360.0);
        set(self, normalized_angle_cos(normalized), normalized_angle_sin(normalized));
    }
    /// Turned `degrees` about the red axis.
    pub fn set_to_rotate_red(&mut self, degrees: f32) {
        self.rotate(degrees, |m, cosine, sine| {
            m.set(1, 1, cosine);
            m.set(1, 2, sine);
            m.set(2, 1, -sine);
            m.set(2, 2, cosine);
        });
    }
    /// Turned `degrees` about the green axis.
    pub fn set_to_rotate_green(&mut self, degrees: f32) {
        self.rotate(degrees, |m, cosine, sine| {
            m.set(0, 0, cosine);
            m.set(0, 2, -sine);
            m.set(2, 0, sine);
            m.set(2, 2, cosine);
        });
    }
    /// Turned `degrees` about the blue axis.
    pub fn set_to_rotate_blue(&mut self, degrees: f32) {
        self.rotate(degrees, |m, cosine, sine| {
            m.set(0, 0, cosine);
            m.set(0, 1, sine);
            m.set(1, 0, -sine);
            m.set(1, 1, cosine);
        });
    }

    /// RGB to YUV (libjpeg's coefficients).
    pub fn convert_rgb_to_yuv(&mut self) {
        self.reset();
        self.set(0, 0, 0.299);
        self.set(0, 1, 0.587);
        self.set(0, 2, 0.114);
        self.set(1, 0, -0.16874);
        self.set(1, 1, -0.33126);
        self.set(1, 2, 0.5);
        self.set(2, 0, 0.5);
        self.set(2, 1, -0.41869);
        self.set(2, 2, -0.08131);
    }

    /// YUV to RGB (libjpeg's coefficients).
    pub fn convert_yuv_to_rgb(&mut self) {
        self.reset();
        self.set(0, 2, 1.402);
        self.set(1, 0, 1.0);
        self.set(1, 1, -0.34414);
        self.set(1, 2, -0.71414);
        self.set(2, 0, 1.0);
        self.set(2, 1, 1.772);
        self.set(2, 2, 0.0);
    }
}

impl std::ops::MulAssign<&ColorMatrix> for ColorMatrix {
    fn mul_assign(&mut self, other: &ColorMatrix) {
        self.times_assign(other);
    }
}

/// Compose's normalizedAngleSin: the sine of a turn's fraction, by Bhaskara's approximation.
fn normalized_angle_sin(normalized_degrees: f32) -> f32 {
    let degrees = normalized_degrees - (normalized_degrees + 0.5).floor();
    let x = 2.0 * degrees.abs();
    let a = 1.0 - x;
    8.0 * degrees * a / (1.25 - x * a)
}

fn normalized_angle_cos(normalized_degrees: f32) -> f32 {
    normalized_angle_sin(normalized_degrees + 0.25)
}

/// What the pixels drawn turn into (Compose's ColorFilter): a color blended onto them ([`tint`](ColorFilter::tint):
/// Compose's BlendModeColorFilter), a color matrix (ColorMatrixColorFilter), or a light that multiplies and adds
/// (LightingColorFilter).
#[derive(Clone, Copy, PartialEq)]
pub enum ColorFilter {
    BlendMode { color: Color, blend_mode: BlendMode },
    ColorMatrix(ColorMatrix),
    Lighting { multiply: Color, add: Color },
}

/// Compose's toString of each.
impl fmt::Debug for ColorFilter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ColorFilter::BlendMode { color, blend_mode } => {
                write!(f, "BlendModeColorFilter(color={color:?}, blendMode={blend_mode:?})")
            }
            ColorFilter::ColorMatrix(m) => write!(f, "ColorMatrixColorFilter(colorMatrix={:?})", KList(&m.values)),
            ColorFilter::Lighting { multiply, add } => {
                write!(f, "LightingColorFilter(multiply={multiply:?}, add={add:?})")
            }
        }
    }
}

impl ColorFilter {
    /// `color` blended onto what is drawn by `blend_mode` (Compose's default: `BlendMode::SrcIn`, its shape in the
    /// color).
    pub fn tint(color: Color, blend_mode: BlendMode) -> ColorFilter {
        ColorFilter::BlendMode { color, blend_mode }
    }
    pub fn color_matrix(color_matrix: ColorMatrix) -> ColorFilter {
        ColorFilter::ColorMatrix(color_matrix)
    }
    /// Each channel times `multiply`'s, plus `add`'s (alpha left as it is).
    pub fn lighting(multiply: Color, add: Color) -> ColorFilter {
        ColorFilter::Lighting { multiply, add }
    }

    /// A pixel (straight, taken in sRGB) through it.
    pub fn apply(&self, pixel: Color) -> Color {
        match *self {
            ColorFilter::ColorMatrix(m) => m.apply(pixel),
            ColorFilter::Lighting { multiply, add } => {
                let (d, m, a) = (pixel.in_srgb(), multiply.in_srgb(), add.in_srgb());
                Color::new(
                    (d.red() * m.red() + a.red()).clamp(0.0, 1.0),
                    (d.green() * m.green() + a.green()).clamp(0.0, 1.0),
                    (d.blue() * m.blue() + a.blue()).clamp(0.0, 1.0),
                    d.alpha(),
                )
            }
            ColorFilter::BlendMode { color, blend_mode } => {
                let d = pixel.in_srgb();
                if blend_mode == BlendMode::SrcIn && color.alpha() * d.alpha() > 0.0 {
                    // (exactly: the tint, as opaque as both)
                    let c = color.in_srgb();
                    return with_alpha(c, c.alpha() * d.alpha());
                }
                // the tint (the source) onto the pixel
                blend(color, d, blend_mode)
            }
        }
    }

    /// A filtered copy of `image` (an Argb8888 bitmap of its own, in sRGB): each of its pixels through it. Made once
    /// for each bitmap (as it is: one drawn into since is filtered again) and filter, and kept while the bitmap
    /// lives (the last 32 made).
    pub fn apply_image(&self, image: &ImageBitmap) -> ImageBitmap {
        struct Entry {
            image: WeakImageBitmap,
            id: u64,
            version: u64,
            filter: ColorFilter,
            filtered: ImageBitmap,
        }
        // (the latest last)
        static CACHE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
        const KEPT: usize = 32;
        let (id, version) = (image.id(), image.version());
        {
            let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
            cache.retain(|e| e.image.is_alive());
            if let Some(i) = cache.iter().position(|e| e.id == id && e.version == version && e.filter == *self) {
                let entry = cache.remove(i);
                let filtered = entry.filtered.clone();
                cache.push(entry);
                return filtered;
            }
        }
        let input = image.pixels();
        let mut out = vec![0u8; input.len()];
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        for (q, p) in out.as_chunks_mut::<4>().0.iter_mut().zip(input.as_chunks::<4>().0) {
            let c = self.apply(Color::new(
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
                p[3] as f32 / 255.0,
            ));
            q.copy_from_slice(&[byte(c.red()), byte(c.green()), byte(c.blue()), byte(c.alpha())]);
        }
        let Some(filtered) = ImageBitmap::from_pixels(image.width(), image.height(), out) else {
            return image.clone();
        };
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache.push(Entry { image: image.downgrade(), id, version, filter: *self, filtered: filtered.clone() });
        if cache.len() > KEPT {
            cache.remove(0);
        }
        filtered
    }

    /// A brush filtered: a color's and a gradient's colors through it, a composite's two brushes; a shader brush of
    /// one's own as it is.
    pub fn apply_brush(&self, brush: &Brush) -> Brush {
        let mut b = brush.clone();
        let filter = |colors: &mut Vec<Color>| {
            for c in colors.iter_mut() {
                *c = self.apply(*c);
            }
        };
        match &mut b.kind {
            BrushKind::SolidColor(c) => *c = self.apply(*c),
            BrushKind::LinearGradient(g) => filter(&mut g.colors),
            BrushKind::RadialGradient(g) => filter(&mut g.colors),
            BrushKind::SweepGradient(g) => filter(&mut g.colors),
            BrushKind::Composite(c) => {
                c.dst_brush = Arc::new(self.apply_brush(&c.dst_brush));
                c.src_brush = Arc::new(self.apply_brush(&c.src_brush));
            }
            BrushKind::Shader(_) => {}
        }
        b
    }
}

// ---- Render effects --------------------------------------------------------------------------------------------

/// A progressive blur's: how far it blurs each pixel ([`BlurRadiusSpec::create_render_effect`]). Along a line from
/// `start` to `end`, or (`radial`) out from `start` to `radius`, in px from the layer's top left; the radius there
/// (px) between `stops` (fraction along it, radius). Or by a mask (`shader`): each pixel's radius the widest
/// stop's times the shader's alpha there, kept to 0..1.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ProgressiveBlur {
    pub radial: bool,
    pub start: Offset,
    pub end: Offset,
    pub radius: f32,
    pub stops: Vec<(f32, f32)>,
    pub shader: Option<Shader>,
}

/// One step of a [`RenderEffect`].
#[derive(Clone, PartialEq, Debug)]
pub enum RenderEffectStep {
    /// A Gaussian blur, its radii across and down in px as Android's (a sigma of 0.57735 r + 0.5 px), the pixels
    /// past the edges of what it blurs taken as `edge_treatment` says.
    Blur { radius_x: f32, radius_y: f32, edge_treatment: TileMode },
    /// Moved.
    Offset(Offset),
    /// A progressive blur (Compose 1.13's), the pixels past the edges taken as `edge_treatment` says (Decal: none;
    /// any other: the edges').
    ProgressiveBlur { edge_treatment: TileMode, progressive: ProgressiveBlur },
}

/// What a layer's pixels go through as it is composited (Compose's RenderEffect, set on a graphics layer): steps
/// done one after another, each on what the one before it made, the innermost (the input) first. Made by
/// [`BlurEffect`], [`OffsetEffect`], [`RenderEffect::blur_effect`], [`RenderEffect::offset_effect`] and
/// [`BlurRadiusSpec::create_render_effect`].
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RenderEffect {
    steps: Vec<RenderEffectStep>,
}

/// The pixels of the layer blurred by `radius_x` across and `radius_y` down (Compose's BlurEffect(radiusX,
/// radiusY, edgeTreatment), Compose's default edge treatment: `TileMode::Clamp`). A radius of 0 (or below) leaves
/// that direction as it is.
pub fn BlurEffect(radius_x: f32, radius_y: f32, edge_treatment: TileMode) -> RenderEffect {
    RenderEffect::blur_effect(None, radius_x, radius_y, edge_treatment)
}

/// The pixels of the layer moved (Compose's OffsetEffect(offsetX, offsetY)).
pub fn OffsetEffect(offset_x: f32, offset_y: f32) -> RenderEffect {
    RenderEffect::offset_effect(None, Offset::new(offset_x, offset_y))
}

impl RenderEffect {
    /// `render_effect` (none: the layer's pixels) blurred (Compose's BlurEffect class).
    pub fn blur_effect(
        render_effect: Option<&RenderEffect>,
        radius_x: f32,
        radius_y: f32,
        edge_treatment: TileMode,
    ) -> RenderEffect {
        let mut e = render_effect.cloned().unwrap_or_default();
        e.steps.push(RenderEffectStep::Blur {
            radius_x: radius_x.max(0.0),
            radius_y: radius_y.max(0.0),
            edge_treatment,
        });
        e
    }
    /// `render_effect` (none: the layer's pixels) moved by `offset` (Compose's OffsetEffect class).
    pub fn offset_effect(render_effect: Option<&RenderEffect>, offset: Offset) -> RenderEffect {
        let mut e = render_effect.cloned().unwrap_or_default();
        e.steps.push(RenderEffectStep::Offset(offset));
        e
    }

    /// Whether it can be drawn here (Compose's isSupported): every effect can.
    pub fn is_supported(&self) -> bool {
        true
    }

    pub fn steps(&self) -> &[RenderEffectStep] {
        &self.steps
    }

    /// Where what it makes of content within `bounds` may reach: a blur three sigmas past each edge, a progressive
    /// one its widest radius and a pixel, moved where it moves.
    pub fn output_bounds(&self, bounds: Rect) -> Rect {
        let mut r = bounds;
        for s in &self.steps {
            match s {
                RenderEffectStep::Offset(o) => r = r.translate(*o),
                RenderEffectStep::ProgressiveBlur { progressive, .. } => {
                    let most = progressive.stops.iter().fold(0.0f32, |m, s| m.max(s.1));
                    if most > 0.0 {
                        r = r.inflate(most + 1.0);
                    }
                }
                RenderEffectStep::Blur { radius_x, radius_y, .. } => {
                    let reach = |radius: f32| if radius > 0.0 { 3.0 * (0.57735 * radius + 0.5) } else { 0.0 };
                    let (x, y) = (reach(*radius_x), reach(*radius_y));
                    r = Rect::new(r.left - x, r.top - y, r.right + x, r.bottom + y);
                }
            }
        }
        r
    }
}

/// A radius along a progressive blur (Compose's BlurStop): `fraction` of the way along it (0..1), `radius`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BlurStop {
    pub fraction: f32,
    pub radius: Dp,
}

impl BlurStop {
    pub const fn new(fraction: f32, radius: Dp) -> BlurStop {
        BlurStop { fraction, radius }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum BlurSpecKind {
    Uniform,
    Vertical,
    Horizontal,
    Linear,
    Radial,
    Shader,
}

type BlurShaderBlock = dyn Fn(&Density, Size) -> Shader + Send + Sync;

/// How much a blur blurs where (Compose 1.13's BlurRadiusSpec): the same everywhere (uniform: a [`BlurEffect`]), or
/// progressively from one radius to another (or through stops, sorted by their fractions) down, across, along a
/// line, or out from a center; or by a shader's alpha. Radii are dp, positions px from the layer's top left. Radii
/// below 0 are taken as 0, fractions kept to 0..1, a fall-off radius that isn't positive and finite as unset.
#[derive(Clone)]
pub struct BlurRadiusSpec {
    kind: BlurSpecKind,
    // (uniform: its radius, the one; a shader's: its widest, twice)
    stops: Vec<BlurStop>,
    start: Offset,
    end: Offset,
    center: Offset,
    fall_off: Option<Dp>,
    shader_block: Option<Arc<BlurShaderBlock>>,
}

impl PartialEq for BlurRadiusSpec {
    /// By what it is; a shader's only to itself (and its copies).
    fn eq(&self, o: &BlurRadiusSpec) -> bool {
        let same_block = match (&self.shader_block, &o.shader_block) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        self.kind == o.kind
            && self.stops == o.stops
            && same_offset(self.start, o.start)
            && same_offset(self.end, o.end)
            && same_offset(self.center, o.center)
            && self.fall_off == o.fall_off
            && same_block
    }
}

impl fmt::Debug for BlurRadiusSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlurRadiusSpec")
            .field("kind", &self.kind)
            .field("stops", &self.stops)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("center", &self.center)
            .field("fall_off", &self.fall_off)
            .field("shader", &self.shader_block.as_ref().map(Arc::as_ptr))
            .finish()
    }
}

impl BlurRadiusSpec {
    fn with_stops(kind: BlurSpecKind, stops: &[BlurStop]) -> BlurRadiusSpec {
        let mut stops: Vec<BlurStop> = stops
            .iter()
            .map(|s| BlurStop { fraction: s.fraction.clamp(0.0, 1.0), radius: Dp(s.radius.0.max(0.0)) })
            .collect();
        stops.sort_by(|a, b| a.fraction.total_cmp(&b.fraction)); // (stable)
        BlurRadiusSpec {
            kind,
            stops,
            start: Offset::Zero,
            end: Offset::Zero,
            center: Offset::Unspecified,
            fall_off: None,
            shader_block: None,
        }
    }
    fn two(start_radius: Dp, end_radius: Dp) -> [BlurStop; 2] {
        [BlurStop::new(0.0, start_radius), BlurStop::new(1.0, end_radius)]
    }

    /// The same everywhere: a [`BlurEffect`].
    pub fn uniform(radius: Dp) -> BlurRadiusSpec {
        BlurRadiusSpec::with_stops(BlurSpecKind::Uniform, &[BlurStop::new(0.0, radius)])
    }
    /// From the top to the bottom.
    pub fn vertical_gradient(start_radius: Dp, end_radius: Dp) -> BlurRadiusSpec {
        BlurRadiusSpec::with_stops(BlurSpecKind::Vertical, &BlurRadiusSpec::two(start_radius, end_radius))
    }
    pub fn vertical_gradient_stops(stops: &[BlurStop]) -> BlurRadiusSpec {
        BlurRadiusSpec::with_stops(BlurSpecKind::Vertical, stops)
    }
    /// From the left to the right.
    pub fn horizontal_gradient(start_radius: Dp, end_radius: Dp) -> BlurRadiusSpec {
        BlurRadiusSpec::with_stops(BlurSpecKind::Horizontal, &BlurRadiusSpec::two(start_radius, end_radius))
    }
    pub fn horizontal_gradient_stops(stops: &[BlurStop]) -> BlurRadiusSpec {
        BlurRadiusSpec::with_stops(BlurSpecKind::Horizontal, stops)
    }
    /// Along the line from `start` to `end`.
    pub fn linear_gradient(start: Offset, end: Offset, start_radius: Dp, end_radius: Dp) -> BlurRadiusSpec {
        BlurRadiusSpec::linear_gradient_stops(start, end, &BlurRadiusSpec::two(start_radius, end_radius))
    }
    pub fn linear_gradient_stops(start: Offset, end: Offset, stops: &[BlurStop]) -> BlurRadiusSpec {
        let mut s = BlurRadiusSpec::with_stops(BlurSpecKind::Linear, stops);
        s.start = start;
        s.end = end;
        s
    }
    /// Out from `center` (unspecified: the middle) to `fall_off_radius` (unspecified: half the shorter side).
    ///
    /// Kotlin: `radialGradient(startRadius: Dp, endRadius: Dp, center: Offset = Offset.Unspecified, falloffRadius: Dp = Dp.Unspecified)`
    pub fn radial_gradient(start_radius: Dp, end_radius: Dp, center: Offset, fall_off_radius: Dp) -> BlurRadiusSpec {
        BlurRadiusSpec::radial_gradient_stops(&BlurRadiusSpec::two(start_radius, end_radius), center, fall_off_radius)
    }
    pub fn radial_gradient_stops(stops: &[BlurStop], center: Offset, fall_off_radius: Dp) -> BlurRadiusSpec {
        let mut s = BlurRadiusSpec::with_stops(BlurSpecKind::Radial, stops);
        s.center = center;
        s.fall_off = (fall_off_radius.0 > 0.0 && fall_off_radius.0.is_finite()).then_some(fall_off_radius);
        s
    }
    /// Each pixel by `max_radius` times the alpha of the shader `block` makes for the layer (the density, and its
    /// size), in its px from its top left (Compose's BlurRadiusSpec.shader; the shader made each time the effect
    /// is). Equal only to itself.
    pub fn shader(max_radius: Dp, block: impl Fn(&Density, Size) -> Shader + Send + Sync + 'static) -> BlurRadiusSpec {
        let mut s = BlurRadiusSpec::with_stops(BlurSpecKind::Shader, &BlurRadiusSpec::two(max_radius, max_radius));
        s.shader_block = Some(Arc::new(block));
        s
    }

    /// The effect for a layer of `size`, the pixels past its edges as `edge_treatment` says (Decal: none; any other:
    /// the edges', as Compose's takes them), its radii in px by `density`. One that blurs nothing changes nothing
    /// (an offset of none).
    pub fn create_render_effect(&self, size: Size, edge_treatment: TileMode, density: &Density) -> RenderEffect {
        let edges = if edge_treatment == TileMode::Decal { TileMode::Decal } else { TileMode::Clamp };
        let px = |s: &BlurStop| density.to_px(s.radius);
        if self.kind == BlurSpecKind::Uniform {
            let r = px(&self.stops[0]);
            return BlurEffect(r, r, edges);
        }
        let most = self.stops.iter().fold(0.0f32, |m, s| m.max(px(s)));
        if self.stops.is_empty() || most <= 0.0 {
            return OffsetEffect(0.0, 0.0);
        }
        let mut p =
            ProgressiveBlur { stops: self.stops.iter().map(|s| (s.fraction, px(s))).collect(), ..Default::default() };
        match self.kind {
            BlurSpecKind::Vertical => p.end = Offset::new(0.0, size.height),
            BlurSpecKind::Horizontal => p.end = Offset::new(size.width, 0.0),
            BlurSpecKind::Linear => {
                p.start = self.start;
                p.end = self.end;
            }
            BlurSpecKind::Shader => p.shader = self.shader_block.as_ref().map(|block| block(density, size)),
            _ => {
                p.radial = true;
                p.start = self.center.take_or_else(|| size.center());
                p.radius = self.fall_off.map_or(size.min_dimension() / 2.0, |r| density.to_px(r));
            }
        }
        RenderEffect { steps: vec![RenderEffectStep::ProgressiveBlur { edge_treatment: edges, progressive: p }] }
    }
}
