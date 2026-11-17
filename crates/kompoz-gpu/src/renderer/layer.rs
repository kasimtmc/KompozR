//! Layers (C++'s pushLayer, popLayer, pushOutlineLayer, keyLayer, acquire and the pool): what is drawn between
//! push_layer and pop_layer goes into a texture of its own, in the parent's pixels before the parent's transform
//! times its resolution (the transform's scale, so that what it enlarges stays sharp), then is composited as a LAYER
//! primitive: cut to its rounded rect (or, an outline layer, to a path's coverage), faded, through the layer's
//! transform. Its texture comes from a pool of a few sizes; a layer the canvas keys whose content stays the same is
//! composited from the pixels it kept. A layer too big for a texture is drawn straight into its target.

use super::effect::EffectImage;
use super::{Inputs, MAX_LAYER, Mask, Renderer};
use crate::device::{ClearColor, Format, IRect, Texture, TextureId, TextureUsage};
use crate::path::{PathPlacement, flatten};
use crate::shaders::{FLAG_OUTLINE, LAYER, Prim};
use kompoz_graphics::{Offset, Path, Projective, Rect, RenderEffect, StretchEffect};
use kompoz_ui::graphics_context::{CornerRadii, LayerContent, LayerTransform};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A layer's texture is in the parent's pixels before the parent's transform, times `resolution` (the transform's
/// scale, so that what it enlarges stays sharp).
#[derive(Clone, Debug)]
pub(super) struct Layer {
    /// None: too big, drawn straight into the target.
    pub(super) texture: Option<TextureId>,
    /// In the parent's pixels times the resolution, whole ones (where the texture goes).
    pub(super) rect: IRect,
    /// The exact bounds there, which rounded corners follow.
    pub(super) shape: Rect,
    /// There.
    pub(super) radii: CornerRadii,
    pub(super) alpha: f32,
    pub(super) transform: LayerTransform,
    pub(super) resolution: f32,
    /// The target's px a unit outside it.
    pub(super) parent_scale: f32,
    /// Cut to this (px from the bounds' top left) instead of the corners.
    pub(super) outline: Option<Path>,
    /// An effect layer's (not cut, its own way of compositing).
    pub(super) effect: Option<RenderEffect>,
    /// A stretched layer's (its effect's image warped as it is composited: see stretch.rs).
    pub(super) stretch: Option<StretchEffect>,
    /// key_layer's: kept (the texture is the kept one's) or to keep; 0: none.
    pub(super) key: u64,
    pub(super) kept: bool,
}

/// A layer target in the pool (or a target's copy, in its format).
#[derive(Debug)]
pub(super) struct PooledTexture {
    pub(super) texture: Texture,
    pub(super) in_use: bool,
    /// Last.
    pub(super) used: Instant,
}

/// A layer's pixels kept from frame to frame (by its key): its texture as it was made, where its exact bounds were
/// in it (a fraction of a pixel), at what resolution and px a unit; what its effect made of it, if it has one (its
/// texture, unless that is the layer's own, and where it goes from the layer's rect's corner).
#[derive(Debug, Default)]
pub(super) struct KeptLayer {
    pub(super) texture: Option<Texture>,
    pub(super) effected: Option<Texture>,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) fraction_x: f32,
    pub(super) fraction_y: f32,
    pub(super) resolution: f32,
    pub(super) parent_scale: f32,
    pub(super) effect: Option<RenderEffect>,
    pub(super) image: Option<EffectImage>,
    pub(super) idle_frames: i32,
}

/// The layers pushed, their textures' pool and the layers kept.
#[derive(Debug, Default)]
pub(super) struct LayerState {
    /// Pushed, the innermost last.
    pub(super) stack: Vec<Layer>,
    /// Layer targets.
    pub(super) pool: Vec<PooledTexture>,
    /// Px of the biggest frame's target (the pool's budget).
    pub(super) surface_area: i64,
    pub(super) kept: HashMap<u64, KeptLayer>,
    /// key_layer's, for the next push_layer.
    pub(super) next: Option<(u64, bool)>,
    pub(super) pushed: LayerContent,
}

/// A size as Skia's approximate fit (GetApproxSize) gives it: up to a power of two to 1024 px, by half of one past
/// it (to MAX_LAYER; what is bigger as big as it is).
fn approx(v: i32) -> i32 {
    if v > MAX_LAYER {
        return v;
    }
    let up = (v.max(16) as u32).next_power_of_two() as i32;
    if v <= 1024 || v > up / 4 * 3 { up } else { up / 4 * 3 }
}

impl Renderer {
    /// A layer target (or a target's copy, in its format) at least w x h, not used yet this frame (one that is would
    /// still be read by the pass that composites it, which runs later). Of a few sizes (see approx), so that a layer
    /// that changes size from frame to frame (clipped to the screen as it scrolls in) takes one kept unused, as does a
    /// layer moving by fractions.
    pub(super) fn acquire(&mut self, width: i32, height: i32, format: Format) -> Option<TextureId> {
        let (w, h) = (approx(width), approx(height));
        if let Some(p) =
            self.layers.pool.iter_mut().find(|p| {
                !p.in_use && p.texture.format() == format && p.texture.width() == w && p.texture.height() == h
            })
        {
            p.in_use = true;
            return Some(p.texture.id());
        }
        let texture = self.device.create_texture(w, h, format, TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET)?;
        self.textures_made += 1;
        let id = texture.id();
        self.layers.pool.push(PooledTexture { texture, in_use: true, used: Instant::now() });
        Some(id)
    }

    /// A pooled texture taken out of the pool (to keep).
    pub(super) fn take_from_pool(&mut self, texture: TextureId) -> Option<Texture> {
        let i = self.layers.pool.iter().position(|p| p.texture.id() == texture)?;
        Some(self.layers.pool.remove(i).texture)
    }

    /// A texture given back to the pool (still in use this frame: what was drawn may read it).
    pub(super) fn give_to_pool(&mut self, texture: Option<Texture>) {
        if let Some(texture) = texture {
            self.layers.pool.push(PooledTexture { texture, in_use: true, used: Instant::now() });
        }
    }

    /// Layer targets unused for 10 s go (layers scroll back in), as Android's renderer keeps its unused ones
    /// (MemoryPolicy's minimumResourceRetention); and the least lately used first while they take more than 12 RGBA
    /// frames (its budget for all it keeps).
    pub(super) fn evict_pool(&mut self) {
        let now = Instant::now();
        let bytes = |p: &PooledTexture| {
            p.texture.width() as i64 * p.texture.height() as i64 * p.texture.format().bytes_per_texel() as i64
        };
        let mut kept = 0;
        for p in &mut self.layers.pool {
            if p.in_use {
                p.used = now;
            }
            p.in_use = false;
            kept += bytes(p);
        }
        self.layers.pool.retain(|p| {
            let old = now.duration_since(p.used) > Duration::from_secs(10);
            if old {
                kept -= bytes(p);
            }
            !old
        });
        let budget = self.layers.surface_area * 12 * 4;
        if kept > budget {
            self.layers.pool.sort_by_key(|p| p.used); // (stable)
            let end = self
                .layers
                .pool
                .iter()
                .position(|p| {
                    if kept <= budget || p.used == now {
                        return true; // (drawn this frame: may be read yet)
                    }
                    kept -= bytes(p);
                    false
                })
                .unwrap_or(self.layers.pool.len());
            self.layers.pool.drain(..end);
        }
    }

    /// Kept layers' pixels unused for a second go back (the layer may not be on the screen any more).
    pub(super) fn age_kept_layers(&mut self) {
        let gone: Vec<u64> = self
            .layers
            .kept
            .iter_mut()
            .filter_map(|(&key, k)| {
                k.idle_frames += 1;
                (k.idle_frames > 60).then_some(key)
            })
            .collect();
        for key in gone {
            if let Some(k) = self.layers.kept.remove(&key) {
                self.give_to_pool(k.texture);
                self.give_to_pool(k.effected);
            }
        }
    }

    /// push_layer, a layer too big for a texture cut to `within` (the parent's pixels), if any.
    pub(super) fn push_layer_within(
        &mut self,
        bounds: Rect,
        radii: CornerRadii,
        alpha: f32,
        transform: LayerTransform,
        within: Option<Rect>,
    ) {
        self.layer_count += 1;
        let s = self.scale;
        let (lx, ly, lw, lh) =
            (bounds.left * s - self.ox, bounds.top * s - self.oy, bounds.width() * s, bounds.height() * s);
        // As many texels for each of its pixels as the transform enlarges it by at most (no more than fit a texture).
        let mut r = 1.0f32;
        for c in 0..4 {
            r = r.max(self.raster_scale(if c & 1 != 0 { lx + lw } else { lx }, if c & 2 != 0 { ly + lh } else { ly }));
        }
        if r > 1.0 && lw.max(lh) * r > MAX_LAYER as f32 {
            r = (MAX_LAYER as f32 / lw.max(lh)).max(1.0);
        }
        let (mut x0, mut y0) = ((lx * r).floor() as i32, (ly * r).floor() as i32);
        let (mut x1, mut y1) = (((lx + lw) * r).ceil() as i32, ((ly + lh) * r).ceil() as i32);
        let part = within.is_some() && (x1 - x0 > MAX_LAYER || y1 - y0 > MAX_LAYER); // (what can show of it)
        if let Some(within) = within
            && part
        {
            x0 = x0.max(within.left.floor() as i32);
            y0 = y0.max(within.top.floor() as i32);
            x1 = x1.min(within.right.ceil() as i32);
            y1 = y1.min(within.bottom.ceil() as i32);
        }
        let mut layer = Layer {
            texture: None,
            rect: IRect::new(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)),
            shape: Rect::from_offset_size(Offset::new(lx * r, ly * r), kompoz_graphics::Size::new(lw * r, lh * r)),
            radii: radii.scaled(s * r),
            alpha,
            transform,
            resolution: r,
            parent_scale: s,
            outline: None,
            effect: None,
            stretch: None,
            key: 0,
            kept: false,
        };
        let fits = layer.rect.width <= MAX_LAYER && layer.rect.height <= MAX_LAYER;
        // Its pixels kept from a frame before (made as big, its bounds as far into a pixel, at the same resolution),
        // or to keep (all of it in its texture).
        self.layers.pushed = LayerContent::Draw;
        if let Some((key, same)) = self.layers.next.take() {
            let (fx, fy) = (layer.shape.left - x0 as f32, layer.shape.top - y0 as f32);
            let found = self.layers.kept.get(&key).map(|k| {
                same && fits
                    && !part
                    && k.width == layer.rect.width
                    && k.height == layer.rect.height
                    && (k.fraction_x - fx).abs() < 1e-3
                    && (k.fraction_y - fy).abs() < 1e-3
                    && k.resolution == r
                    && k.parent_scale == s
            });
            if found == Some(true) {
                let k = self.layers.kept.get_mut(&key).expect("found above");
                layer.texture = k.texture.as_ref().map(Texture::id);
                layer.key = key;
                layer.kept = true;
                k.idle_frames = 0;
                self.layers.pushed = LayerContent::Kept;
                self.kept_layers += 1;
            } else {
                // One seen for the first time is drawn whole anyway (a layer that has just come, a fade starting):
                // kept from now, it isn't drawn whole again in the next frame.
                let first = found.is_none();
                if let Some(k) = self.layers.kept.remove(&key) {
                    // (What it holds is gone.)
                    self.give_to_pool(k.texture);
                    self.give_to_pool(k.effected);
                }
                if (same || first) && fits && !part {
                    layer.key = key;
                    self.layers.pushed = LayerContent::DrawAll;
                }
            }
        }
        if fits && !layer.kept {
            layer.texture = self.acquire(layer.rect.width, layer.rect.height, Format::Rgba8);
        }
        if layer.texture.is_none() {
            layer.key = 0;
            self.layers.pushed = LayerContent::Draw;
        }
        // Too big: drawn straight into the target (no rounding, alpha or transform), in its clips.
        if let Some(texture) = layer.texture
            && !layer.kept
        {
            self.begin_target(texture, Some(ClearColor::TRANSPARENT));
            self.scale *= r;
            let parent = self.targets.last().expect("begun above");
            self.ox = parent.ox * r + x0 as f32;
            self.oy = parent.oy * r + y0 as f32;
            // Its pixels in the parent's before its transform (x / r + the rect's corner), through the layer's own
            // transform (scaled and turned about its origin, moved), then the parent's.
            let own = own_transform(&layer, layer.parent_scale);
            let from_texels =
                Projective { m: [1.0 / r, 0.0, x0 as f32 / r, 0.0, 1.0 / r, y0 as f32 / r, 0.0, 0.0, 1.0] };
            self.window_from_target = parent.window_from_target * parent.transform * own * from_texels;
        }
        self.layers.stack.push(layer);
    }
}

/// A layer's transform in its parent's pixels before the parent's transform: scaled and turned about its origin,
/// then moved (x' = M (x - o) + o + t; its translation px a unit times `scale`).
fn own_transform(layer: &Layer, scale: f32) -> Projective {
    let (b, tf, r) = (layer.shape, layer.transform, layer.resolution);
    let ox = (b.left + b.width() * tf.origin.x) / r;
    let oy = (b.top + b.height() * tf.origin.y) / r;
    let (a, bb, c, d) = turned(&tf);
    Projective {
        m: [
            a,
            c,
            ox - (a * ox + c * oy) + tf.translation_x * scale,
            bb,
            d,
            oy - (bb * ox + d * oy) + tf.translation_y * scale,
            0.0,
            0.0,
            1.0,
        ],
    }
}

/// A layer transform's linear part: x' = a x + c y, y' = b x + d y.
fn turned(tf: &LayerTransform) -> (f32, f32, f32, f32) {
    let rad = tf.rotation_z * std::f32::consts::PI / 180.0;
    let (sn, cs) = rad.sin_cos();
    (cs * tf.scale_x, sn * tf.scale_x, -sn * tf.scale_y, cs * tf.scale_y)
}

pub(super) fn push_layer(r: &mut Renderer, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
    r.push_layer_within(bounds, radii, alpha, transform, None);
}

pub(super) fn push_outline_layer(
    r: &mut Renderer,
    bounds: Rect,
    outline: &Path,
    alpha: f32,
    transform: LayerTransform,
) {
    r.push_layer_within(bounds, CornerRadii::Zero, alpha, transform, None);
    if let Some(layer) = r.layers.stack.last_mut()
        && layer.texture.is_some()
    {
        layer.outline = Some(outline.clone());
    }
}

/// A layer whose content is the same as when it was last drawn (whole) is composited from the pixels it had (and
/// what its effect made of them), not drawn again; one that stays the same is drawn whole, and kept.
pub(super) fn key_layer(r: &mut Renderer, key: u64, same: bool) {
    r.layers.next = Some((key, same));
}

pub(super) fn pop_layer(r: &mut Renderer) {
    let Some(layer) = r.layers.stack.pop() else { return };
    let Some(texture) = layer.texture else { return };
    if !layer.kept {
        r.end_target();
    }
    r.scale = layer.parent_scale;
    let res = layer.resolution;
    if layer.key != 0 && !layer.kept {
        // Drawn whole: kept.
        let taken = r.take_from_pool(texture);
        let k = r.layers.kept.remove(&layer.key).unwrap_or_default();
        r.give_to_pool(k.texture);
        r.give_to_pool(k.effected);
        r.layers.kept.insert(
            layer.key,
            KeptLayer {
                texture: taken,
                effected: None,
                width: layer.rect.width,
                height: layer.rect.height,
                fraction_x: layer.shape.left - layer.rect.x as f32,
                fraction_y: layer.shape.top - layer.rect.y as f32,
                resolution: res,
                parent_scale: r.scale,
                ..KeptLayer::default()
            },
        );
    }
    if layer.effect.is_some() || layer.stretch.is_some() {
        // What the effect made (kept, or made now), its pixels where they show, back in the parent's pixels; then
        // stretched, if it is.
        let image = match &layer.effect {
            Some(effect) => effect_image(r, &layer, texture, effect),
            None => EffectImage::of(&layer, texture),
        };
        let image = match &layer.stretch {
            Some(stretch) => r.stretch_image(&layer, image, stretch),
            None => image,
        };
        let p = image.prim(layer.alpha, res);
        r.add(LAYER, &p, Inputs { texture: Some(image.texture), ..Inputs::default() });
        return;
    }
    let b = layer.shape;
    let half = b.width().min(b.height()) / 2.0;
    let clamped = |v: f32| v.clamp(0.0, half.max(0.0));
    let radii = layer.radii;
    let corners =
        [clamped(radii.top_left), clamped(radii.top_right), clamped(radii.bottom_right), clamped(radii.bottom_left)];
    let tf = layer.transform;
    let mut square = corners.iter().fold(0.0f32, |m, &c| m.max(c)) < 0.5;
    let rx0 = layer.rect.x as f32;
    let ry0 = layer.rect.y as f32;
    let (rx1, ry1) = (rx0 + layer.rect.width as f32, ry0 + layer.rect.height as f32);
    let mut p = Prim::default();
    // Cut to an outline: its coverage, rasterized in the layer's pixels before its transform.
    let mut outline = None;
    if let Some(path) = &layer.outline {
        let contours = flatten(path, &PathPlacement::new(Offset::Zero, r.scale * res, Offset::new(-b.left, -b.top)));
        let Some(origin) = r.rasterize_in_clips(&contours, layer.rect, false) else { return };
        outline = Some(origin);
        square = true; // (its pixels, cut by the mask)
    }
    // Square: all of its pixels. Rounded: its exact bounds, edges anti-aliased.
    if square {
        p.bounds = [rx0, ry0, rx1, ry1];
        p.shape = [rx0, ry0, rx1, ry1];
        if !tf.is_identity() || res != 1.0 || !r.transform.axis_aligned() {
            p.bounds = [rx0 - 1.0, ry0 - 1.0, rx1 + 1.0, ry1 + 1.0];
        }
    } else {
        p.bounds = [b.left - 1.0, b.top - 1.0, b.right + 1.0, b.bottom + 1.0];
        p.shape = [b.left, b.top, b.right, b.bottom];
        p.radii = corners;
    }
    p.params = [rx0, ry0, rx1, ry1];
    p.uv = [0.0, 0.0, 1.0 / texture.width() as f32, 1.0 / texture.height() as f32];
    p.color = [0.0, 0.0, 0.0, layer.alpha];
    // Back to the parent's pixels (the texture's are `res` times as many), scaled and turned about the origin, then
    // moved: x' = M (x / res - o) + o + t.
    let (a, bb, c, d) = turned(&tf);
    let own = own_transform(&layer, r.scale);
    p.xform = [a / res, bb / res, c / res, d / res];
    p.extra[0] = own.m[2];
    p.extra[1] = own.m[5];
    if let Some(origin) = outline {
        p.radii[0] = origin.x;
        p.radii[1] = origin.y;
        p.extra[3] = FLAG_OUTLINE as f32;
        p.bounds = [rx0, ry0, rx1, ry1]; // (no further: the mask goes no further)
        r.add(LAYER, &p, Inputs { texture: Some(texture), mask: Mask::Texture1, ..Inputs::default() });
        return;
    }
    r.add(LAYER, &p, Inputs { texture: Some(texture), ..Inputs::default() });
}

/// What an effect layer's effect makes of its pixels: kept from when it was last made, if the layer and its effect
/// are the same; else made now (and kept, if the layer is).
fn effect_image(r: &mut Renderer, layer: &Layer, texture: TextureId, effect: &RenderEffect) -> EffectImage {
    let keeping = (layer.key != 0).then_some(layer.key);
    let kept_image = keeping.and_then(|key| {
        let k = r.layers.kept.get(&key)?;
        if k.effect.as_ref() != Some(effect) {
            return None;
        }
        let mut image = k.image?;
        image.texture = k.effected.as_ref().map_or(texture, Texture::id);
        Some(image.moved(layer.rect.x, layer.rect.y))
    });
    if let Some(image) = kept_image {
        return image;
    }
    let image = r.apply_effect(layer);
    if let Some(key) = keeping {
        let effected = if image.texture != texture { r.take_from_pool(image.texture) } else { None };
        let k = r.layers.kept.entry(key).or_default();
        let before = std::mem::replace(&mut k.effected, effected);
        k.effect = Some(effect.clone());
        k.image = Some(image.moved(-layer.rect.x, -layer.rect.y));
        r.give_to_pool(before);
    }
    image
}

pub(super) fn layer_content(r: &Renderer) -> LayerContent {
    r.layers.pushed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_textures_come_in_few_sizes() {
        // Powers of two up to 1024 (16 at least), by quarters of one past it, as big as asked past MAX_LAYER.
        assert_eq!([1, 16, 17, 100, 1024].map(approx), [16, 16, 32, 128, 1024]);
        assert_eq!([1025, 1536, 1537, 3000, 4096, 5000].map(approx), [1536, 1536, 2048, 3072, 4096, 5000]);
    }

    #[test]
    fn layers_turn_about_their_origin() {
        let layer = Layer {
            texture: None,
            rect: IRect::new(10, 20, 40, 20),
            shape: Rect::new(10.0, 20.0, 50.0, 40.0),
            radii: CornerRadii::Zero,
            alpha: 1.0,
            transform: LayerTransform { rotation_z: 90.0, translation_x: 3.0, ..LayerTransform::Identity },
            resolution: 1.0,
            parent_scale: 1.0,
            outline: None,
            effect: None,
            stretch: None,
            key: 0,
            kept: false,
        };
        let own = own_transform(&layer, 1.0);
        // Its middle stays (moved 3 across); its top left turns a quarter about it.
        let mid = own.apply(30.0, 30.0);
        assert!((mid.x - 33.0).abs() < 1e-4 && (mid.y - 30.0).abs() < 1e-4);
        let corner = own.apply(10.0, 20.0);
        assert!((corner.x - 43.0).abs() < 1e-4 && (corner.y - 10.0).abs() < 1e-4, "{corner:?}");
    }
}
