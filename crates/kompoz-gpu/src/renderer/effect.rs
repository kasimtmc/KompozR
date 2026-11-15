//! Render effects (C++'s pushEffectLayer, applyEffect, progressiveBlur, effectPass, drawBackdrop): a layer's pixels
//! through its effect's steps in passes of shaders/effects.slang (blurs scaled down by powers of two, then Gaussian
//! across and down; progressive blurs; offsets), composited where they show; a backdrop the same on what is drawn
//! under it.
//!
//! Each blur: its first pass makes the pixels past what it blurs as its tile mode says, within what the blur reads,
//! in a texture scaled down by powers of two until the sigma left is at most 4 texels (as Skia's does for a wide
//! one: no more than 25 taps); then a Gaussian across and down. The variance the scaling adds (the mean of each
//! block, and the bilinear filtering that scales the last one back up) is taken off the sigma. Offsets are the
//! composite's.

use super::layer::Layer;
use super::paint::tile_index;
use super::{EffectDraw, Inputs, Pass, PassCopy, Renderer, STOPS_WIDTH, intersect};
use crate::device::{ClearColor, Format, IRect, TextureId};
use crate::shaders::{FILL, FLAG_MIXED, GAUSS, LAYER, PREPARE, PROGRESSIVE, Prim};
use kompoz_graphics::{BlendMode, Matrix, ProgressiveBlur, Rect, RenderEffect, RenderEffectStep, TileMode};
use kompoz_ui::graphics_context::{CornerRadii, LayerTransform};

/// What an effect layer's steps make of its texture: `texels` of `texture`, whose texel (0, 0) is at `origin` in the
/// layer's pixels, each texel `step` of them across and down; it shows `shown` (whole pixels), moved by `offset`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct EffectImage {
    pub(super) texture: TextureId,
    pub(super) origin_x: i32,
    pub(super) origin_y: i32,
    pub(super) step_x: i32,
    pub(super) step_y: i32,
    pub(super) texels_x: i32,
    pub(super) texels_y: i32,
    pub(super) shown: IRect,
    pub(super) offset_x: f32,
    pub(super) offset_y: f32,
}

impl EffectImage {
    /// The layer's own texels, as they are.
    pub(super) fn of(layer: &Layer, texture: TextureId) -> EffectImage {
        let r = layer.rect;
        EffectImage {
            texture,
            origin_x: r.x,
            origin_y: r.y,
            step_x: 1,
            step_y: 1,
            texels_x: r.width,
            texels_y: r.height,
            shown: r,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// The same moved by whole pixels (kept from the layer's rect's corner, and back).
    pub(super) fn moved(self, dx: i32, dy: i32) -> EffectImage {
        EffectImage {
            origin_x: self.origin_x + dx,
            origin_y: self.origin_y + dy,
            shown: IRect::new(self.shown.x + dx, self.shown.y + dy, self.shown.width, self.shown.height),
            ..self
        }
    }

    /// Composited as a LAYER primitive: its texels where they show (moved), at `alpha`, the layer's texels `res`
    /// times the parent's pixels.
    pub(super) fn prim(&self, alpha: f32, res: f32) -> Prim {
        let t = self.texture;
        let x0 = self.shown.x as f32 + self.offset_x;
        let y0 = self.shown.y as f32 + self.offset_y;
        let (x1, y1) = (x0 + self.shown.width as f32, y0 + self.shown.height as f32);
        let tx = self.origin_x as f32 + self.offset_x;
        let ty = self.origin_y as f32 + self.offset_y;
        Prim {
            bounds: [x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0],
            shape: [x0, y0, x1, y1],
            params: [tx, ty, tx + (self.texels_x * self.step_x) as f32, ty + (self.texels_y * self.step_y) as f32],
            uv: [0.0, 0.0, 1.0 / (t.width() * self.step_x) as f32, 1.0 / (t.height() * self.step_y) as f32],
            color: [0.0, 0.0, 0.0, alpha],
            xform: [1.0 / res, 0.0, 0.0, 1.0 / res],
            ..Prim::default()
        }
    }
}

/// A blur of an effect's steps: its sigmas in the layer's texels, the level it is worked out at (texels a texel of
/// it, each way), the pixels past what it blurs as `tile` says.
struct Blur {
    sigma_x: f32,
    sigma_y: f32,
    step_x: i32,
    step_y: i32,
    tile: TileMode,
}

/// The level a sigma is worked out at, from `at` on: scaled down by twos until it is at most 4 texels there.
fn level(sigma: f32, mut at: i32) -> i32 {
    while sigma / at as f32 > 4.0 {
        at *= 2;
    }
    at
}

/// Android's sigma for a blur radius in px (no more than Skia's 532).
fn sigma_of(radius_px: f32) -> f32 {
    (0.57735 * radius_px + 0.5).min(532.0)
}

/// What is left of each blur's sigmas at its level, across and down (in its level's texels): its variance less what
/// scaling down to it adds, and the last ones less what scaling back up adds (as much of that as each has, the last
/// first).
fn sigmas_left(blurs: &[Blur]) -> Vec<[f32; 2]> {
    let across = left_along(blurs.iter().map(|b| (b.sigma_x, b.step_x)).collect());
    let down = left_along(blurs.iter().map(|b| (b.sigma_y, b.step_y)).collect());
    across.into_iter().zip(down).map(|(x, y)| [x, y]).collect()
}

/// sigmas_left along one axis, from each blur's sigma and level on it.
fn left_along(blurs: Vec<(f32, i32)>) -> Vec<f32> {
    let last = blurs.last().map_or(1, |b| b.1);
    let mut up = (last * last - 1) as f32 / 6.0;
    let mut left = vec![0.0f32; blurs.len()];
    for k in (0..blurs.len()).rev() {
        let (sigma, step) = blurs[k];
        let before = if k > 0 { blurs[k - 1].1 } else { 1 };
        let variance = (sigma * sigma - (step * step - before * before) as f32 / 12.0).max(0.0);
        let taken = if sigma > 0.0 { variance.min(up) } else { 0.0 };
        up -= taken;
        left[k] = if sigma > 0.0 { (variance - taken).sqrt() / step as f32 } else { 0.0 };
    }
    left
}

impl Renderer {
    /// A pass of an effect: `rect` of `target` made from `source` (filtered linearly, or not), with this frame's
    /// table if it reads stops, or `mask` in the table's place. It runs before the pass that composites what it makes.
    #[allow(clippy::too_many_arguments)]
    fn effect_pass(
        &mut self,
        target: TextureId,
        source: TextureId,
        rect: IRect,
        uniforms: [f32; 32],
        linear: bool,
        table: bool,
        mask: Option<TextureId>,
    ) {
        let mut pass = Pass::new(target, None);
        pass.effect = Some(EffectDraw { source, rect, uniforms, linear, table, mask });
        self.order.push(self.passes.len());
        self.passes.push(pass);
    }

    /// A row of the table: each stop (fraction, radius over the widest) at a texel of its own.
    fn table_row(&mut self, stops: &[(f32, f32)]) -> i32 {
        let row = self.table_rows();
        let at = self.stops.len();
        self.stops.resize(at + STOPS_WIDTH * 4, 0.0);
        for (i, &(fraction, radius)) in stops.iter().take(STOPS_WIDTH).enumerate() {
            self.stops[at + i * 4] = fraction;
            self.stops[at + i * 4 + 1] = radius;
        }
        row
    }

    /// A progressive blur, as Compose's: the radius at each pixel the widest one (no more than 150 px) times where it
    /// lies between the stops (their radii over the widest); the layer's pixels cut to its bounds first, as what lies
    /// past them is none of it; the bounds (its pixels, the layer's own) are where its taps count. What it makes
    /// reaches as far as its widest radius past them. A shader's (a mask) is drawn first into a texture of what the
    /// blur makes, in half floats (8 bits would move where a radius reaches a whole pixel), each texel its alpha where
    /// it lies (its px from the layer's top left): the radius there the widest times that.
    fn progressive_blur(
        &mut self,
        layer: &Layer,
        texture: TextureId,
        edges: TileMode,
        p: &ProgressiveBlur,
    ) -> EffectImage {
        let res = layer.resolution;
        let most = p.stops.iter().fold(0.0f32, |m, s| m.max(s.1));
        let widest = (most * self.scale).min(150.0); // px
        let image = EffectImage::of(layer, texture);
        if widest.is_nan() || widest <= 0.0 {
            return image;
        }
        let ext = (widest.floor() * res) as i32 + 2;
        let crop = layer.rect;
        let region = IRect::new(crop.x - ext, crop.y - ext, crop.width + 2 * ext, crop.height + 2 * ext);
        let Some(cut) = self.acquire(region.width, region.height, Format::Rgba8) else { return image };
        let mut c = [0.0f32; 32];
        c[4..12].copy_from_slice(&[
            1.0,
            1.0,
            (region.x - crop.x) as f32,
            (region.y - crop.y) as f32,
            0.0,
            0.0,
            crop.width as f32,
            crop.height as f32,
        ]);
        c[12] = PREPARE as f32;
        c[13] = tile_index(TileMode::Decal) as f32;
        c[18] = 1.0 / texture.width() as f32;
        c[19] = 1.0 / texture.height() as f32;
        self.effect_pass(cut, texture, IRect::new(0, 0, region.width, region.height), c, false, false, None);
        let stops: Vec<(f32, f32)> = p.stops.iter().map(|&(at, radius)| (at, radius / most)).collect();
        let row = self.table_row(&stops);
        let k = self.scale * res; // texels a px
        let (ox, oy) = (layer.shape.left - region.x as f32, layer.shape.top - region.y as f32);
        let mut mask = None;
        if let Some(shader) = &p.shader {
            let Some(m) = self.acquire(region.width, region.height, Format::Rgba16f) else { return image };
            mask = Some(m);
            let s0 = self.scale;
            self.begin_target(m, Some(ClearColor::TRANSPARENT));
            // (The region in the layer's px, its top left where the shader is taken from: a composite's bake covers
            // all of it.)
            self.scale = k;
            self.ox = -ox;
            self.oy = -oy;
            let mut shift = Matrix::new();
            shift.translate(ox / k, oy / k, 0.0);
            let area = Rect::new(-ox / k, -oy / k, (region.width as f32 - ox) / k, (region.height as f32 - oy) / k);
            let mut q = self.painted_shader(&shader.transformed(&shift), area, 1.0, 1.0, None);
            q.bounds = [-1.0, -1.0, region.width as f32 + 1.0, region.height as f32 + 1.0];
            q.shape = [0.0, 0.0, region.width as f32, region.height as f32];
            self.add(FILL, &q, Inputs::default());
            self.end_target();
            self.scale = s0;
        }
        let mut from = cut;
        for axis in 0..2 {
            let Some(to) = self.acquire(region.width, region.height, Format::Rgba8) else { return image };
            let kind = if mask.is_some() {
                3.0
            } else if p.radial {
                2.0
            } else {
                1.0
            };
            let u = [
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                1.0,
                0.0,
                0.0,
                (crop.x - region.x) as f32,
                (crop.y - region.y) as f32,
                (crop.x + crop.width - region.x) as f32,
                (crop.y + crop.height - region.y) as f32,
                PROGRESSIVE as f32,
                tile_index(edges) as f32,
                axis as f32,
                widest,
                res,
                0.0,
                1.0 / from.width() as f32,
                1.0 / from.height() as f32,
                kind,
                row as f32,
                stops.len().min(STOPS_WIDTH) as f32,
                0.0,
                p.start.x * k,
                p.start.y * k,
                if p.radial { p.radius * k } else { p.end.x * k },
                if p.radial { 0.0 } else { p.end.y * k },
                ox,
                oy,
                0.0,
                0.0,
            ];
            self.effect_pass(to, from, IRect::new(0, 0, region.width, region.height), u, true, true, mask);
            from = to;
        }
        EffectImage {
            texture: from,
            origin_x: region.x,
            origin_y: region.y,
            step_x: 1,
            step_y: 1,
            texels_x: region.width,
            texels_y: region.height,
            shown: region,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// An effect layer's steps on its texture (in passes that run before the one compositing it), and the image they
    /// make.
    pub(super) fn apply_effect(&mut self, layer: &Layer) -> EffectImage {
        let texture = layer.texture.unwrap_or(self.white.id());
        let mut image = EffectImage::of(layer, texture);
        let Some(effect) = &layer.effect else { return image };
        let res = layer.resolution;
        let steps = effect.steps();
        let mut first = 0;
        if let Some(RenderEffectStep::ProgressiveBlur { edge_treatment, progressive }) = steps.first() {
            image = self.progressive_blur(layer, texture, *edge_treatment, progressive);
            first = 1;
        }
        let mut blurs = Vec::new();
        let (mut step_x, mut step_y) = (1, 1);
        for s in &steps[first..] {
            match *s {
                RenderEffectStep::ProgressiveBlur { .. } => {} // (only ever the first)
                RenderEffectStep::Offset(o) => {
                    image.offset_x += o.x * self.scale * res;
                    image.offset_y += o.y * self.scale * res;
                }
                RenderEffectStep::Blur { radius_x, radius_y, edge_treatment } => {
                    // Android's sigma for the radius in the parent's pixels, in the layer's.
                    let sx = if radius_x > 0.0 { sigma_of(radius_x * self.scale) * res } else { 0.0 };
                    let sy = if radius_y > 0.0 { sigma_of(radius_y * self.scale) * res } else { 0.0 };
                    if sx <= 0.0 && sy <= 0.0 {
                        continue;
                    }
                    step_x = level(sx, step_x);
                    step_y = level(sy, step_y);
                    blurs.push(Blur { sigma_x: sx, sigma_y: sy, step_x, step_y, tile: edge_treatment });
                }
            }
        }
        let left = sigmas_left(&blurs);
        for (blur, [gx, gy]) in blurs.iter().zip(left) {
            // What it shows: three sigmas past what it blurs (whole pixels, as Skia's).
            let reach = |sigma: f32| if sigma > 0.0 { (3.0 * sigma).ceil() as i32 } else { 0 };
            let (ex, ey) = (reach(blur.sigma_x), reach(blur.sigma_y));
            let shown = IRect::new(
                image.shown.x - ex,
                image.shown.y - ey,
                image.shown.width + 2 * ex,
                image.shown.height + 2 * ey,
            );
            // What it works out: the texels the filtering scaling it back up reads (one past what shows) and as far
            // past them as the kernel reaches, on the image's texels.
            let mx = ((3.0 * gx).ceil() as i32 + 1) * blur.step_x;
            let my = ((3.0 * gy).ceil() as i32 + 1) * blur.step_y;
            let ox = image.origin_x
                + ((shown.x - mx - image.origin_x) as f32 / image.step_x as f32).floor() as i32 * image.step_x;
            let oy = image.origin_y
                + ((shown.y - my - image.origin_y) as f32 / image.step_y as f32).floor() as i32 * image.step_y;
            let nx = (shown.x + shown.width + mx - ox + blur.step_x - 1) / blur.step_x;
            let ny = (shown.y + shown.height + my - oy + blur.step_y - 1) / blur.step_y;
            // The image's own texels (whole ones of it).
            let mut vx0 = ((image.shown.x - image.origin_x) as f32 / image.step_x as f32).floor();
            let mut vy0 = ((image.shown.y - image.origin_y) as f32 / image.step_y as f32).floor();
            let mut vx1 = ((image.shown.x + image.shown.width - image.origin_x) as f32 / image.step_x as f32).ceil();
            let mut vy1 = ((image.shown.y + image.shown.height - image.origin_y) as f32 / image.step_y as f32).ceil();
            // Scaled down at most 8 times each way in a pass, the first one tiling.
            let mut from = image.texture;
            let (mut fx, mut fy) = (blur.step_x / image.step_x, blur.step_y / image.step_y);
            let (mut at_x, mut at_y) = (image.step_x, image.step_y);
            let mut map_x = ((ox - image.origin_x) / image.step_x) as f32;
            let mut map_y = ((oy - image.origin_y) / image.step_y) as f32;
            let mut tile = tile_index(blur.tile) as f32;
            let mut first_pass = true;
            while first_pass || fx > 1 || fy > 1 {
                let (px, py) = (fx.min(8), fy.min(8));
                fx /= px;
                fy /= py;
                at_x *= px;
                at_y *= py;
                let w = (shown.x + shown.width + mx - ox + at_x - 1) / at_x;
                let h = (shown.y + shown.height + my - oy + at_y - 1) / at_y;
                let Some(to) = self.acquire(w, h, Format::Rgba8) else { return image };
                let mut u = [0.0f32; 32];
                u[4..16].copy_from_slice(&[
                    px as f32,
                    py as f32,
                    map_x,
                    map_y,
                    vx0,
                    vy0,
                    vx1,
                    vy1,
                    PREPARE as f32,
                    tile,
                    0.0,
                    0.0,
                ]);
                u[18] = 1.0 / from.width() as f32;
                u[19] = 1.0 / from.height() as f32;
                self.effect_pass(to, from, IRect::new(0, 0, w, h), u, false, false, None);
                // The next pass reads all of this one's, nothing past it.
                from = to;
                (map_x, map_y, vx0, vy0, vx1, vy1) = (0.0, 0.0, 0.0, 0.0, w as f32, h as f32);
                tile = tile_index(TileMode::Decal) as f32;
                first_pass = false;
            }
            for (axis, g) in [gx, gy].into_iter().enumerate() {
                if g <= 0.0 {
                    continue;
                }
                let Some(to) = self.acquire(nx, ny, Format::Rgba8) else { return image };
                let mut u = [0.0f32; 32];
                u[4..20].copy_from_slice(&[
                    1.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    nx as f32,
                    ny as f32,
                    GAUSS as f32,
                    tile_index(TileMode::Decal) as f32,
                    axis as f32,
                    g,
                    (3.0 * g).ceil(),
                    0.0,
                    1.0 / from.width() as f32,
                    1.0 / from.height() as f32,
                ]);
                self.effect_pass(to, from, IRect::new(0, 0, nx, ny), u, false, false, None);
                from = to;
            }
            image = EffectImage {
                texture: from,
                origin_x: ox,
                origin_y: oy,
                step_x: blur.step_x,
                step_y: blur.step_y,
                texels_x: nx,
                texels_y: ny,
                shown,
                offset_x: image.offset_x,
                offset_y: image.offset_y,
            };
        }
        image
    }
}

/// A layer whose content goes through `effect` as it is composited. Too big for a texture: the part around what
/// can show, as far as the effect brings what is there into it (its resolution is 1 then).
pub(super) fn push_effect_layer(r: &mut Renderer, bounds: Rect, effect: &RenderEffect, alpha: f32) {
    let visible = r.scissor();
    let back = r.transform.inverse();
    let (mut l, mut t, mut rr, mut b) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
    let mut seen = true;
    for c in 0..4 {
        if !seen {
            break;
        }
        let x = if c & 1 != 0 { visible.x + visible.width } else { visible.x } as f32;
        let y = if c & 2 != 0 { visible.y + visible.height } else { visible.y } as f32;
        let (q, w) = back.apply_w(x, y);
        seen = w > 0.0;
        (l, t, rr, b) = (l.min(q.x), t.min(q.y), rr.max(q.x), b.max(q.y));
    }
    let within = seen.then(|| {
        let out = effect.output_bounds(bounds); // (how far it takes what is drawn, either way)
        let reach_x = (bounds.left - out.left).max(out.right - bounds.right).max(0.0) * r.scale + 1.0;
        let reach_y = (bounds.top - out.top).max(out.bottom - bounds.bottom).max(0.0) * r.scale + 1.0;
        Rect::new(l - reach_x, t - reach_y, rr + reach_x, b + reach_y)
    });
    r.push_layer_within(bounds, CornerRadii::Zero, alpha, LayerTransform::Identity, within);
    if let Some(layer) = r.layers.stack.last_mut()
        && layer.texture.is_some()
    {
        layer.effect = Some(effect.clone());
    }
}

/// What is drawn so far under `bounds` through `effect`, drawn back in place of it. The pass drawn into the target
/// so far runs first (ended here: what follows goes into a pass of its own after), then what it reads is copied out,
/// goes through the effect as a layer's texture would (in the target's pixels: at its px a unit, whatever the
/// transform), and is drawn back where the effect takes it in place of what is there (Src, mixed by the alpha), in
/// the clips (which cut it to its shape).
pub(super) fn draw_backdrop(r: &mut Renderer, bounds: Rect, effect: &RenderEffect, alpha: f32) {
    let Some(pass) = r.pass_index() else { return };
    if alpha <= 0.0 {
        return;
    }
    let reads = effect.output_bounds(bounds);
    let target = r.passes[pass].target;
    let all = target.bounds();
    let s = r.scale;
    // The px it covers through the transform (all of them if behind the eye).
    let pixels = |rect: Rect| {
        r.mapped(rect.left * s - r.ox, rect.top * s - r.oy, rect.right * s - r.ox, rect.bottom * s - r.oy)
            .unwrap_or(Rect::new(0.0, 0.0, all.width as f32, all.height as f32))
    };
    let (on, shape) = (pixels(reads), pixels(bounds));
    let (x0, y0) = (on.left.floor() as i32, on.top.floor() as i32);
    let region = intersect(IRect::new(x0, y0, on.right.ceil() as i32 - x0, on.bottom.ceil() as i32 - y0), all);
    if region.is_empty() {
        return;
    }
    let Some(copy) = r.acquire(region.width, region.height, target.format()) else { return };
    r.order.push(pass);
    let mut copying = Pass::new(copy, None);
    copying.copy = Some(PassCopy { from: target, region, x: 0, y: 0 });
    r.order.push(r.passes.len());
    r.passes.push(copying);
    let next = r.passes.len();
    if let Some(t) = r.targets.last_mut() {
        t.pass = next;
    }
    r.passes.push(Pass::new(target, None));
    let layer = Layer {
        texture: Some(copy),
        rect: region,
        shape,
        radii: CornerRadii::Zero,
        alpha,
        transform: LayerTransform::Identity,
        resolution: 1.0,
        parent_scale: r.scale,
        outline: None,
        effect: Some(effect.clone()),
        key: 0,
        kept: false,
    };
    let image = r.apply_effect(&layer);
    let mut p = image.prim(alpha, 1.0);
    p.extra[3] = FLAG_MIXED as f32; // (in place of what is there, mixed with it by the alpha)
    r.blends.push(BlendMode::Src);
    r.add(LAYER, &p, Inputs { texture: Some(image.texture), in_target_pixels: true, ..Inputs::default() });
    r.blends.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_blurs_are_worked_out_scaled_down() {
        // At most 4 texels of sigma at the level; a second blur from the first's level on.
        assert_eq!([level(3.0, 1), level(4.0, 1), level(4.5, 1), level(17.0, 1), level(5.0, 8)], [1, 1, 2, 8, 8]);
        assert!((sigma_of(10.0) - 6.2735).abs() < 1e-4 && sigma_of(2000.0) == 532.0);
    }

    #[test]
    fn scaling_takes_its_variance_off_the_sigma() {
        let blur =
            |s: f32, step: i32| Blur { sigma_x: s, sigma_y: 0.0, step_x: step, step_y: 1, tile: TileMode::Decal };
        // Not scaled: as it is.
        assert_eq!(sigmas_left(&[blur(3.0, 1)]), [[3.0, 0.0]]);
        // Scaled down 8 times: its variance less the blocks' (63 / 12) and the bilinear filtering's back up
        // (63 / 6), in the level's texels.
        let s = 24.0f32;
        let want = (s * s - 63.0 / 12.0 - 63.0 / 6.0).sqrt() / 8.0;
        assert!((sigmas_left(&[blur(s, 8)])[0][0] - want).abs() < 1e-5);
        // Two: the second at the first's level takes all of the scaling back up it can (here all of its variance),
        // the first the rest.
        let left = sigmas_left(&[blur(17.82, 8), blur(2.23, 8)]);
        assert_eq!(left[1][0], 0.0);
        let rest = 63.0 / 6.0 - 2.23f32 * 2.23;
        assert!((left[0][0] - (17.82f32 * 17.82 - 63.0 / 12.0 - rest).sqrt() / 8.0).abs() < 1e-4);
    }
}
