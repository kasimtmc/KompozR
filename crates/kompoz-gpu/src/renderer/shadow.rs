//! Shadows of paths and elevation shadows (C++'s drawShadowPath, drawElevationShadow): a path's shadow drawn white
//! once into a texture kept while it stays the same, then tinted where it goes (as Compose's DropShadowRenderer keeps
//! its blurred bitmap); an elevation shadow as Android draws one (Skia's SkShadowUtils: analytic ambient and spot
//! shadows of a flat rounded rect, else blurred), the light mapped into the target's pixels from the window's. A
//! rounded rect's shadow is a primitive ([`prims`](super::prims)'s).

use super::effect::push_effect_layer;
use super::layer::pop_layer;
use super::paint::{premultiplied, with_alpha};
use super::path::{command_bytes, draw_path};
use super::{Inputs, MAX_LAYER, Renderer};
use crate::device::{ClearColor, FNV1A_BASIS, Filter, Format, Sampler, Texture, fnv1a_from};
use crate::shaders::{ELEVATION, GLYPH, Prim};
use kompoz_graphics::{
    BlurEffect, Brush, Color, CornerRadius, Matrix, Offset, Outline, Path, PathDirection, PathFillType, PathStroke,
    Projective, Rect, Shadow, TileMode,
};
use kompoz_runtime::clock::Instant;
use kompoz_ui::graphics_context::{ElevationShadow, ShadowLight, default_draw_shadow_path, shadow_path_reach};
use std::collections::HashMap;
use std::time::Duration;

/// A path's shadow's mask (as Compose's DropShadowRenderer and InnerShadowRenderer keep their blurred bitmaps), by
/// what it is drawn from (draw_shadow_path): white where the shadow is, its texture's top left `width` x `height`.
#[derive(Debug)]
pub(super) struct KeptShadow {
    pub(super) texture: Option<Texture>,
    pub(super) width: i32,
    pub(super) height: i32,
    /// path_frame when it was last drawn.
    pub(super) frame: u64,
    pub(super) used: Instant,
}

/// The paths' shadows' masks kept.
#[derive(Debug, Default)]
pub(super) struct ShadowState {
    pub(super) kept: HashMap<u64, KeptShadow>,
}

impl Renderer {
    /// Paths' shadows' masks undrawn for 10 s go back to the pool.
    pub(super) fn end_frame_shadows(&mut self) {
        let now = Instant::now();
        let frame = self.path_frame;
        let mut gone = Vec::new();
        for (&key, k) in &mut self.shadows.kept {
            if k.frame == frame {
                k.used = now;
            }
            if now.duration_since(k.used) > Duration::from_secs(10) {
                gone.push(key);
            }
        }
        for key in gone {
            if let Some(k) = self.shadows.kept.remove(&key) {
                self.give_to_pool(k.texture);
            }
        }
    }
}

/// A shadow of any outline, as Compose keeps a dropShadow's or innerShadow's blurred bitmap (DropShadowRenderer,
/// InnerShadowRenderer) while its shape, size and shadow stay the same: drawn white once into a texture of its own
/// (GraphicsContext's), then that texture where it goes, tinted with the shadow's color, for as long as it is drawn
/// from the same path, at the same px a unit, with the same blur and spread (and an inner one's offset), as far into
/// a pixel (to a 64th: it moves by whole pixels). Turned or scaled, it is drawn as it is.
pub(super) fn draw_shadow_path(r: &mut Renderer, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
    if shadow.color.alpha() <= 0.0 || outline.is_empty() || r.targets.is_empty() {
        return;
    }
    if !r.transform.is_identity() {
        return default_draw_shadow_path(r, outline, origin, shadow, inner);
    }
    let b = outline.get_bounds().translate(origin);
    let reach = shadow_path_reach(b, shadow, inner);
    if super::quick_reject(r, reach) {
        return;
    }
    let s = r.scale;
    let (x0, y0) = (reach.left * s - r.ox, reach.top * s - r.oy);
    let (px, py) = (x0.floor() as i32, y0.floor() as i32);
    let w = (reach.right * s - r.ox).ceil() as i32 - px;
    let h = (reach.bottom * s - r.oy).ceil() as i32 - py;
    if w <= 0 || h <= 0 || w > MAX_LAYER || h > MAX_LAYER {
        return default_draw_shadow_path(r, outline, origin, shadow, inner);
    }
    r.path_shadows += 1;
    let what: Vec<u8> = [s, shadow.blur_radius, shadow.spread]
        .into_iter()
        .chain(if inner { [shadow.offset.x, shadow.offset.y] } else { [0.0, 0.0] })
        .flat_map(f32::to_ne_bytes)
        .chain(
            [
                inner as i32,
                (outline.fill_type == PathFillType::EvenOdd) as i32,
                ((x0 - px as f32) * 64.0).round() as i32,
                ((y0 - py as f32) * 64.0).round() as i32,
            ]
            .into_iter()
            .flat_map(i32::to_ne_bytes),
        )
        .collect();
    let key = fnv1a_from(fnv1a_from(FNV1A_BASIS, &command_bytes(outline)), &what);
    let fits = r.shadows.kept.get(&key).is_some_and(|k| k.width == w && k.height == h && k.texture.is_some());
    if fits {
        r.kept_shadows += 1;
    } else {
        // Made: drawn white, the target's px from its corner (as a layer is), into a texture kept out of the pool.
        let Some(t) = r.acquire(w, h, Format::Rgba8) else {
            return default_draw_shadow_path(r, outline, origin, shadow, inner);
        };
        let texture = r.take_from_pool(t);
        r.begin_target(t, Some(ClearColor::TRANSPARENT));
        let parent = r.targets.last().expect("begun above");
        (r.ox, r.oy) = (parent.ox + px as f32, parent.oy + py as f32);
        let white = Shadow { color: Color::White, ..shadow.clone() };
        default_draw_shadow_path(r, outline, origin, &white, inner);
        r.end_target();
        let before = r
            .shadows
            .kept
            .insert(key, KeptShadow { texture, width: w, height: h, frame: r.path_frame, used: Instant::now() });
        if let Some(before) = before {
            r.give_to_pool(before.texture); // (what was drawn with it this frame may read it yet)
        }
    }
    let kept = r.shadows.kept.get_mut(&key).expect("kept above");
    kept.frame = r.path_frame;
    let Some(texture) = kept.texture.as_ref().map(Texture::id) else { return };
    let (tw, th) = (texture.width() as f32, texture.height() as f32);
    let mut p = r.painted(&Brush::from(shadow.color), reach, 1.0, 1.0);
    let (fx, fy) = (px as f32, py as f32);
    p.bounds = [fx, fy, fx + w as f32, fy + h as f32];
    p.shape = p.bounds;
    p.uv = [0.0, 0.0, w as f32 / tw, h as f32 / th];
    p.params = [0.0, 0.0, 0.5 / tw, 0.5 / th];
    let nearest = Sampler { filter: Filter::Nearest, ..Sampler::default() }; // (texel for pixel)
    r.add(GLYPH, &p, Inputs::texture(texture, nearest));
}

/// Skia's SkDrawShadowMetrics::GetSpotParams: a caster `z` above the plane under the light (x, y, lz) of `radius`
/// casts its spot shadow scaled by `scale` about the light's foot and moved by `offset`, blurred by `blur` (all in
/// the same px).
struct SpotParams {
    blur: f32,
    scale: f32,
    offset: Offset,
}

fn spot_params(z: f32, lx: f32, ly: f32, lz: f32, radius: f32) -> SpotParams {
    let ratio = if lz - z > 0.0 { (z / (lz - z)).clamp(0.0, 0.95) } else { 0.95 };
    let scale = if lz - z > 0.0 { (lz / (lz - z)).clamp(1.0, 1.95) } else { 1.95 };
    SpotParams { blur: radius * ratio, scale, offset: Offset::new(-ratio * lx, -ratio * ly) }
}

type Point3 = [f64; 3];

fn cross(a: Point3, b: Point3) -> Point3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Skia's SkDrawShadowMetrics::GetSpotShadowTransform: the map from the caster's coordinates to the target's px of
/// its spot shadow (cast from the light through its corners onto the plane under it), and its blur; none where the
/// light isn't above it.
fn spot_shadow_transform(
    light: Offset,
    lz: f32,
    radius: f32,
    ctm: &Projective,
    plane: [f32; 3],
    bounds: Rect,
) -> Option<(Projective, f32)> {
    let height = |x: f32, y: f32| plane[0] * x + plane[1] * y + plane[2];
    let c = bounds.center();
    let z = height(c.x, c.y);
    if ctm.affine() {
        let p = spot_params(z, light.x, light.y, lz, radius);
        let cast = Projective { m: [p.scale, 0.0, p.offset.x, 0.0, p.scale, p.offset.y, 0.0, 0.0, 1.0] };
        return Some((cast * *ctm, p.blur));
    }
    if bounds.width() < 1e-6 || bounds.height() < 1e-6 {
        return None;
    }
    // The bounds' corners in 3D, cast from the light onto z = 0.
    let corners = [
        Offset::new(bounds.left, bounds.top),
        Offset::new(bounds.right, bounds.top),
        Offset::new(bounds.right, bounds.bottom),
        Offset::new(bounds.left, bounds.bottom),
    ];
    let mut q = [[0.0f64; 3]; 4];
    for (i, corner) in corners.iter().enumerate() {
        let d = ctm.apply(corner.x, corner.y);
        let h = height(corner.x, corner.y) as f64;
        let dz = lz as f64 - h;
        if dz <= 1e-6 {
            return None;
        }
        let ratio = h / dz;
        let (dx, dy) = (d.x as f64, d.y as f64);
        q[i] = [dx - (light.x as f64 - dx) * ratio, dy - (light.y as f64 - dy) * ratio, 1.0];
    }
    // The square [-1, 1]^2 onto the cast quad: its edges' crossings the new axes, its diagonals' the origin.
    let mut h0 = cross(cross(q[1], q[0]), cross(q[2], q[3]));
    let mut h1 = cross(cross(q[0], q[3]), cross(q[1], q[2]));
    let h2 = cross(cross(q[0], q[2]), cross(q[1], q[3]));
    if h2[2].abs() < 1e-12 {
        return None;
    }
    // Which way round: (-1, -1) to the first corner, as Skia means it to (its own check, which takes the crossing
    // points undivided by their w, leaves the shadow mirrored in some perspectives: a triangle's, say). Turning an
    // axis round takes it to another of the square's.
    let off = |x: f64, y: f64| {
        let w = h0[2] * x + h1[2] * y + h2[2];
        ((h0[0] * x + h1[0] * y + h2[0]) / w - q[0][0]).hypot((h0[1] * x + h1[1] * y + h2[1]) / w - q[0][1])
    };
    let (mut best, mut signs) = (f64::MAX, 0);
    for s in 0..4 {
        let e = off(if s & 1 != 0 { 1.0 } else { -1.0 }, if s & 2 != 0 { 1.0 } else { -1.0 });
        if e < best {
            (best, signs) = (e, s);
        }
    }
    if signs & 1 != 0 {
        h0 = h0.map(|v| -v);
    }
    if signs & 2 != 0 {
        h1 = h1.map(|v| -v);
    }
    let d = h2[2];
    let to_quad = Projective {
        m: [
            (h0[0] / d) as f32,
            (h1[0] / d) as f32,
            (h2[0] / d) as f32,
            (h0[1] / d) as f32,
            (h1[1] / d) as f32,
            (h2[1] / d) as f32,
            (h0[2] / d) as f32,
            (h1[2] / d) as f32,
            1.0,
        ],
    };
    let (sx, sy) = (2.0 / bounds.width(), 2.0 / bounds.height());
    let to_square =
        Projective { m: [sx, 0.0, -sx * bounds.left - 1.0, 0.0, sy, -sy * bounds.top - 1.0, 0.0, 0.0, 1.0] };
    let ratio = if lz - z > 0.0 { (z / (lz - z)).clamp(0.0, 0.95) } else { 0.95 };
    Some((to_quad * to_square, radius * ratio))
}

/// A Matrix of a Projective (its plane part).
fn matrix_of(p: &Projective) -> Matrix {
    let mut m = Matrix::new();
    let v = &mut m.values;
    v[Matrix::ScaleX] = p.m[0];
    v[Matrix::SkewX] = p.m[1];
    v[Matrix::TranslateX] = p.m[2];
    v[Matrix::SkewY] = p.m[3];
    v[Matrix::ScaleY] = p.m[4];
    v[Matrix::TranslateY] = p.m[5];
    v[Matrix::Perspective0] = p.m[6];
    v[Matrix::Perspective1] = p.m[7];
    v[Matrix::Perspective2] = p.m[8];
    m
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

/// The one radius of a rect (0), or of a rounded rect whose corners are quarter circles of one radius (as drawn:
/// fitted), within half its shorter side; none for any other outline.
fn one_radius(outline: &Outline, b: Rect) -> Option<f32> {
    match outline {
        Outline::Rectangle(_) => Some(0.0),
        Outline::Rounded(rr) => {
            let positive = |c: CornerRadius| CornerRadius::new(c.x.max(0.0), c.y.max(0.0));
            let f = kompoz_graphics::RoundRect {
                top_left_corner_radius: positive(rr.top_left_corner_radius),
                top_right_corner_radius: positive(rr.top_right_corner_radius),
                bottom_right_corner_radius: positive(rr.bottom_right_corner_radius),
                bottom_left_corner_radius: positive(rr.bottom_left_corner_radius),
                ..*rr
            }
            .scaled_radii();
            let corners = [
                f.top_left_corner_radius,
                f.top_right_corner_radius,
                f.bottom_right_corner_radius,
                f.bottom_left_corner_radius,
            ];
            let circular = corners.iter().all(|c| c.x == c.y);
            let tl = corners[0].x;
            (circular && corners.iter().all(|c| (c.x - tl).abs() < 1e-3))
                .then(|| tl.min(b.width().min(b.height()) / 2.0))
        }
        Outline::Generic(_) => None,
    }
}

/// An ELEVATION primitive: a rounded rect's shadow falling off over `blur` px in from `at`'s edge, left out
/// from `hole` px in (none: below 0), in `color`.
fn elevation_prim(r: &mut Renderer, at: Rect, corner: f32, blur: f32, hole: f32, color: Color) {
    let p = Prim {
        shape: [at.left, at.top, at.right, at.bottom],
        bounds: [at.left - 1.0, at.top - 1.0, at.right + 1.0, at.bottom + 1.0],
        radii: [corner; 4],
        params: [blur, hole, 0.0, 0.0],
        color: premultiplied(color),
        ..Prim::default()
    };
    r.add(ELEVATION, &p, Inputs::default());
}

/// An elevation shadow as Android draws it (HWUI, then Skia's SkShadowUtils on Ganesh), in this target's px (a
/// layer's resolution makes them more than the window's: its lengths, the light's too, taken up with them), the
/// light mapped into them from the window's. A rounded rect of one radius (or a rect, a circle), flat, drawn
/// through a similarity: analytically, as drawFastShadow and ShadowRRectOp do it (an ambient shadow the outline
/// spread by r = z / 2 px, up to 150, its falloff b = r (1 + z / 128) in from there; a spot one the outline cast
/// from the light, spread by its blur and falling off over twice that; each left out under the caster where it is
/// opaque). Otherwise as Skia blurs when it can do nothing better, in the target's px: the ambient shadow its
/// outline filled and stroked, the spot one cast through the light (in perspective, its corners each from its own
/// height).
pub(super) fn draw_elevation_shadow(r: &mut Renderer, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
    let light = r.light;
    let alpha = shadow.alpha.clamp(0.0, 1.0);
    let ambient = with_alpha(shadow.ambient_color, shadow.ambient_color.alpha() * light.ambient_alpha * alpha);
    let spot = with_alpha(shadow.spot_color, shadow.spot_color.alpha() * light.spot_alpha * alpha);
    if r.targets.is_empty() || (ambient.alpha() <= 0.0 && spot.alpha() <= 0.0) {
        return;
    }
    let transparent = alpha < 1.0;
    let u = r.scale; // this target's px a window's px
    let (light_px, w) = r.window_from_target.inverse().apply_w(light.center.x, light.center.y);
    let (lz, lr) = (light.z * u, light.radius * u);
    let b = outline.bounds().translate(origin);
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return;
    }
    let flat = shadow.plane[0].abs() < 1e-6 && shadow.plane[1].abs() < 1e-6;
    let radius = one_radius(outline, b);
    let t = r.transform;
    let (a, c, bb, d) = (t.m[0], t.m[1], t.m[3], t.m[4]);
    let (len0, len1) = (a.hypot(bb), c.hypot(d));
    let similar = t.affine()
        && (t.m[8] - 1.0).abs() < 1e-6
        && len0 > 0.0
        && (len0 - len1).abs() < 1e-4 * len0
        && (a * c + bb * d).abs() < 1e-4 * len0 * len0;
    if let Some(radius) = radius
        && flat
        && similar
        && w > 0.0
    {
        let to_local = 1.0 / len0; // the target's px in the prim's
        let s = r.scale;
        let rect = Rect::from_offset_size(
            Offset::new(b.left * s - r.ox, b.top * s - r.oy),
            kompoz_graphics::Size::new(b.width() * s, b.height() * s),
        );
        let rr = radius * s;
        let z = shadow.plane[2];
        if ambient.alpha() > 0.0 && z > 0.0 {
            let inset = (z / 2.0).min(150.0) * u;
            let blur = inset * (1.0 + z / 128.0);
            let out = inset * to_local;
            let at = rect.inflate(out);
            elevation_prim(r, at, rr + out, blur * to_local, if transparent { -1.0 } else { out }, ambient);
        }
        if spot.alpha() > 0.0 && z > 0.0 {
            let sp = spot_params(z * u, light_px.x, light_px.y, lz, lr);
            let off = Offset::new(sp.offset.x + sp.scale * t.m[2], sp.offset.y + sp.scale * t.m[5]);
            let local = t.inverse().apply(off.x, off.y); // (an affine map's inverse, as a point)
            let cast = Rect::from_offset_size(
                Offset::new(rect.left * sp.scale + local.x, rect.top * sp.scale + local.y),
                kompoz_graphics::Size::new(rect.width() * sp.scale, rect.height() * sp.scale),
            );
            let (cast_radius, blur) = (rr * sp.scale, sp.blur * to_local);
            let max_offset = if rr <= 0.0 {
                (cast.left - rect.left)
                    .abs()
                    .max((cast.top - rect.top).abs())
                    .max((cast.right - rect.right).abs())
                    .max((cast.bottom - rect.bottom).abs())
            } else {
                let dr = cast_radius - rr;
                let upper_left = Offset::new(cast.left - rect.left + dr, cast.top - rect.top + dr);
                let lower_right = Offset::new(cast.right - rect.right - dr, cast.bottom - rect.bottom - dr);
                (upper_left.x * upper_left.x + upper_left.y * upper_left.y)
                    .max(lower_right.x * lower_right.x + lower_right.y * lower_right.y)
                    .sqrt()
                    + dr
            };
            let inset = blur + blur.max(max_offset);
            let at = cast.inflate(blur);
            elevation_prim(
                r,
                at,
                cast_radius + blur,
                2.0 * sp.blur * to_local,
                if transparent { -1.0 } else { inset },
                spot,
            );
        }
        return;
    }
    // Blurred, in the target's px: the outline there, and the map from what is drawn to there.
    let path = outline_path(outline);
    let device = r.polygons_of(&path, origin, &PathStroke::default());
    let ctm = t * Projective { m: [r.scale, 0.0, -r.ox, 0.0, r.scale, -r.oy, 0.0, 0.0, 1.0] };
    let saved = (r.scale, r.ox, r.oy, r.transform);
    (r.scale, r.ox, r.oy, r.transform) = (1.0, 0.0, 0.0, Projective::Identity);
    let mut flat_outline = Path::new(); // (its px, through the transform: what Skia strokes)
    let (mut l, mut top, mut rgt, mut bot) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
    for contour in &device {
        for (i, q) in contour.points.iter().enumerate() {
            if i == 0 {
                flat_outline.move_to(q.x, q.y);
            } else {
                flat_outline.line_to(q.x, q.y);
            }
            (l, top, rgt, bot) = (l.min(q.x), top.min(q.y), rgt.max(q.x), bot.max(q.y));
        }
        flat_outline.close();
    }
    // (At the outline's origin, as Skia's.)
    let z = shadow.plane[0] * origin.x + shadow.plane[1] * origin.y + shadow.plane[2];
    if ambient.alpha() > 0.0 && z > 0.0 && l <= rgt {
        let outset = (z / 2.0).min(150.0) * u;
        let blur = 0.5 * outset * (1.0 + z / 128.0);
        let stroke = 0.5 * (outset - blur);
        let grow = stroke.max(0.0) * 2.0; // (mitered: a little more)
        let at = Rect::new(l - grow, top - grow, rgt + grow, bot + grow);
        let opaque = Brush::from(with_alpha(ambient, 1.0));
        push_effect_layer(r, at, &BlurEffect(blur, blur, TileMode::Decal), ambient.alpha());
        draw_path(r, &flat_outline, Offset::Zero, &opaque, at, &PathStroke::default());
        if stroke > 0.0 {
            draw_path(r, &flat_outline, Offset::Zero, &opaque, at, &PathStroke::new(stroke));
        }
        pop_layer(r);
    }
    if spot.alpha() > 0.0 {
        let plane = shadow.plane.map(|v| v * u); // (z in the target's px)
        let pb = path.get_bounds().translate(origin);
        if let Some((m, blur)) = spot_shadow_transform(light_px, lz, lr, &ctm, plane, pb)
            && w > 0.0
        {
            let (mut cl, mut ct, mut cr, mut cb) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
            let mut seen = true;
            for i in 0..4 {
                if !seen {
                    break;
                }
                let x = if i & 1 != 0 { pb.right } else { pb.left };
                let y = if i & 2 != 0 { pb.bottom } else { pb.top };
                let (q, cw) = m.apply_w(x, y);
                seen = cw > 0.0;
                (cl, ct, cr, cb) = (cl.min(q.x), ct.min(q.y), cr.max(q.x), cb.max(q.y));
            }
            if seen {
                let at = Rect::new(cl - 1.0, ct - 1.0, cr + 1.0, cb + 1.0);
                push_effect_layer(r, at, &BlurEffect(blur, blur, TileMode::Decal), spot.alpha());
                super::push_transform(r, &matrix_of(&m));
                draw_path(r, &path, origin, &Brush::from(with_alpha(spot, 1.0)), pb, &PathStroke::default());
                super::pop_translation(r);
                pop_layer(r);
            }
        }
    }
    (r.scale, r.ox, r.oy, r.transform) = saved;
}

pub(super) fn light(r: &Renderer) -> ShadowLight {
    r.light
}

/// The light's center in the px drawn in now: from the window's px to the current target's (its layers'
/// placements), then back through the transform and the translations.
pub(super) fn light_center(r: &Renderer) -> Offset {
    let c = r.light.center;
    let target = r.window_from_target.inverse().apply(c.x, c.y);
    let local = r.transform.inverse().apply(target.x, target.y);
    Offset::new((local.x + r.ox) / r.scale, (local.y + r.oy) / r.scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_shadows_are_cast_as_skias() {
        // 50 px up under a light 500 px up: scaled by 500 / 450 about the light's foot, blurred by its radius / 9.
        let p = spot_params(50.0, 100.0, 0.0, 500.0, 800.0);
        assert!((p.scale - 500.0 / 450.0).abs() < 1e-6 && (p.blur - 800.0 / 9.0).abs() < 1e-3);
        assert!((p.offset.x + 100.0 / 9.0).abs() < 1e-4 && p.offset.y == 0.0);
        // In perspective: the square's corners onto the cast quad's, the first to the first.
        let ctm = Projective { m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.002, 0.0, 1.0] };
        let b = Rect::new(10.0, 20.0, 40.0, 50.0);
        let (m, _) = spot_shadow_transform(Offset::new(30.0, 0.0), 500.0, 800.0, &ctm, [0.0, 0.0, 20.0], b).unwrap();
        let cast = |x: f32, y: f32| {
            let d = ctm.apply(x, y);
            let ratio = 20.0 / 480.0;
            Offset::new(d.x - (30.0 - d.x) * ratio, d.y - (0.0 - d.y) * ratio)
        };
        for (x, y) in [(10.0, 20.0), (40.0, 20.0), (40.0, 50.0), (10.0, 50.0)] {
            let (got, want) = (m.apply(x, y), cast(x, y));
            assert!((got.x - want.x).abs() < 1e-3 && (got.y - want.y).abs() < 1e-3, "{got:?} {want:?}");
        }
        // Back as a Matrix, the same map.
        let back = Projective::of(&matrix_of(&m));
        assert!(back.m.iter().zip(m.m).all(|(a, b)| (a - b).abs() < 1e-6));
    }

    #[test]
    fn rounded_rects_of_one_radius_are_drawn_analytically() {
        let b = Rect::new(0.0, 0.0, 40.0, 20.0);
        let rr = |r: f32| Outline::Rounded(kompoz_graphics::RoundRect::from_rect_radius(b, CornerRadius::circular(r)));
        assert_eq!(one_radius(&Outline::Rectangle(b), b), Some(0.0));
        assert_eq!(one_radius(&rr(6.0), b), Some(6.0));
        assert_eq!(one_radius(&rr(30.0), b), Some(10.0)); // (fitted)
        let mut odd = kompoz_graphics::RoundRect::from_rect_radius(b, CornerRadius::circular(4.0));
        odd.top_left_corner_radius = CornerRadius::new(4.0, 2.0);
        assert_eq!(one_radius(&Outline::Rounded(odd), b), None);
        assert_eq!(one_radius(&Outline::Generic(Path::new()), b), None);
    }
}
