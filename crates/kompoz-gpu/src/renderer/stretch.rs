//! Stretched layers: Android's stretch overscroll as hwui composites a RenderNode's layer that has a StretchEffect
//! (RenderNodeDrawable::drawContent, its default StretchEffectBehavior::ShaderHWUI). The layer is drawn offscreen,
//! cut to its bounds as hwui's layer is its RenderNode's size; its pixels go through its render effect first, if it
//! has one (hwui's snapshot through its image filter); then a pass of shaders/stretch.slang (hwui's stretch shader)
//! makes a texture of where that shows, each texel the content the warp brings there (its uniforms as
//! StretchEffect::getShader sets them for the layer's width and height, the content clamped to its edges and filtered
//! linearly, as getShader's image shader samples it), which is composited as an effect's image is: faded, blended,
//! filtered, clipped and transformed as the layer is. The warp is in the layer's own px, before its transform (as
//! hwui's shader is in the RenderNode's coordinates), whatever its resolution.
//!
//! What a canvas keeps: the layer's content is kept from frame to frame as any layer's (key_layer: a stretch that
//! changes while the content stays isn't drawn again, as hwui keeps the RenderNode's layer and warps it anew each
//! frame), and so is what its render effect makes of it; the warp is done each time it is composited.
//!
//! hwui's hole punches and its StretchMask (a SurfaceView's hole through the window, stretched with the content in an
//! alpha mask drawn DstOut) have nothing to do here: nothing is drawn under a frame from another surface, so nothing
//! punches holes.

use super::effect::{EffectImage, effect_reach, push_effect_layer, visible_before_transform};
use super::layer::{Layer, push_layer};
use super::{EffectDraw, Pass, Renderer};
use crate::device::{Format, IRect, TextureId};
use crate::shaders::StretchPassUniforms;
use kompoz_graphics::{Offset, Rect, RenderEffect, StretchEffect};
use kompoz_ui::graphics_context::{CornerRadii, LayerTransform};

/// A layer whose content goes through `effect`, if any, and is then stretched over `bounds` as it is composited.
/// None stretched: an effect layer, or a plain one. Too big for a texture: the part around what can show, where the
/// warp takes its content from what shows (and as far as the effect brings what is there into that).
pub(super) fn push_stretch_layer(
    r: &mut Renderer,
    bounds: Rect,
    stretch: &StretchEffect,
    effect: Option<&RenderEffect>,
    alpha: f32,
) {
    if stretch.is_empty() {
        return match effect {
            Some(effect) => push_effect_layer(r, bounds, effect, alpha),
            None => push_layer(r, bounds, CornerRadii::Zero, alpha, LayerTransform::Identity),
        };
    }
    let s = r.scale;
    let (lx, ly) = (bounds.left * s - r.ox, bounds.top * s - r.oy); // (its px' top left in the target's)
    let within = visible_before_transform(r).and_then(|v| {
        let u = stretch.uniforms(bounds.width() * s, bounds.height() * s)?;
        // (The warp keeps the content in order: what shows from one corner to the other comes from between where
        // the warp takes the two.)
        let a = u.content_position(Offset::new(v.left - lx, v.top - ly));
        let b = u.content_position(Offset::new(v.right - lx, v.bottom - ly));
        let (reach_x, reach_y) = effect.map_or((1.0, 1.0), |e| effect_reach(r, bounds, e));
        Some(Rect::new(lx + a.x - reach_x, ly + a.y - reach_y, lx + b.x + reach_x, ly + b.y + reach_y))
    });
    r.push_layer_within(bounds, CornerRadii::Zero, alpha, LayerTransform::Identity, within);
    if let Some(layer) = r.layers.stack.last_mut()
        && layer.texture.is_some()
    {
        layer.effect = effect.cloned();
        layer.stretch = Some(*stretch);
    }
}

impl Renderer {
    /// A stretch's pass: `rect` of `target` made from `source` by stretch.slang with `uniforms`. It runs before the
    /// pass that composites what it makes.
    fn stretch_pass(&mut self, target: TextureId, source: TextureId, rect: IRect, uniforms: StretchPassUniforms) {
        let mut pass = Pass::new(target, None);
        let uniforms = uniforms.floats();
        pass.effect =
            Some(EffectDraw { source, rect, uniforms, linear: true, table: false, mask: None, stretch: true });
        self.order.push(self.passes.len());
        self.passes.push(pass);
    }

    /// `image` (what the layer's effect made of its pixels, or its pixels) stretched by `stretch` over the layer's
    /// bounds: a texture of the layer's texels where the image shows (hwui's dstBounds), each the content the warp
    /// brings there.
    pub(super) fn stretch_image(&mut self, layer: &Layer, image: EffectImage, stretch: &StretchEffect) -> EffectImage {
        let res = layer.resolution;
        let b = layer.shape; // (its bounds in its texels)
        let Some(u) = stretch.uniforms(b.width() / res, b.height() / res) else { return image };
        let x0 = (image.shown.x as f32 + image.offset_x).floor() as i32;
        let y0 = (image.shown.y as f32 + image.offset_y).floor() as i32;
        let x1 = (image.shown.x as f32 + image.offset_x + image.shown.width as f32).ceil() as i32;
        let y1 = (image.shown.y as f32 + image.offset_y + image.shown.height as f32).ceil() as i32;
        let out = IRect::new(x0, y0, x1 - x0, y1 - y0);
        if out.width <= 0 || out.height <= 0 {
            return image;
        }
        let Some(target) = self.acquire(out.width, out.height, Format::Rgba8) else { return image };
        // The layer's px p is its texel p res + its bounds' top left; that is the image's texel (it - its origin -
        // its offset) / its step.
        let t = image.texture;
        let (kx, ky) = (image.step_x as f32 * t.width() as f32, image.step_y as f32 * t.height() as f32);
        let (ox, oy) = (image.origin_x as f32 + image.offset_x, image.origin_y as f32 + image.offset_y);
        // The image's own texels (where it shows), their middles at its edges (the middle of all of them where they
        // are fewer than one).
        let edge = |from: i32, to: i32, origin: i32, step: i32, size: f32| {
            let (low, high) = ((from - origin) as f32 / step as f32 + 0.5, (to - origin) as f32 / step as f32 - 0.5);
            if low <= high { (low / size, high / size) } else { ((low + high) / 2.0 / size, (low + high) / 2.0 / size) }
        };
        let s = &image.shown;
        let (ex0, ex1) = edge(s.x, s.x + s.width, image.origin_x, image.step_x, t.width() as f32);
        let (ey0, ey1) = edge(s.y, s.y + s.height, image.origin_y, image.step_y, t.height() as f32);
        let uniforms = StretchPassUniforms {
            to_clip: [0.0; 4], // (the pass's target's)
            local: [1.0 / res, 1.0 / res, (x0 as f32 - b.left) / res, (y0 as f32 - b.top) / res],
            source: [res / kx, res / ky, (b.left - ox) / kx, (b.top - oy) / ky],
            edges: [ex0, ey0, ex1, ey1],
            stretch: u,
        };
        self.stretch_pass(target, t, IRect::new(0, 0, out.width, out.height), uniforms);
        EffectImage {
            texture: target,
            origin_x: out.x,
            origin_y: out.y,
            step_x: 1,
            step_y: 1,
            texels_x: out.width,
            texels_y: out.height,
            shown: out,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}
