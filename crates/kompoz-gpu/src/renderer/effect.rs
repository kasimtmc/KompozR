//! Render effects (C++'s pushEffectLayer, applyEffect, progressiveBlur, effectPass, drawBackdrop): a layer's pixels
//! through its effect's steps in passes of shaders/effects.slang (blurs scaled down by powers of two, then Gaussian
//! across and down; progressive blurs; offsets), composited where they show; a backdrop the same on what is drawn
//! under it.
//!
//! Drawn by stage 7's layers-and-effects package: until then an effect layer is a plain layer (what GraphicsContext
//! does by default), a backdrop draws nothing (its default), and an effect leaves a layer's pixels as they are.

use super::Renderer;
use super::layer::Layer;
use crate::device::{IRect, TextureId};
use kompoz_graphics::{Rect, RenderEffect};
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
}

impl Renderer {
    /// An effect layer's steps on its texture (in passes that run before the one compositing it), and the image they
    /// make. Drawn by stage 7's layers-and-effects package: until then, the layer's pixels as they are.
    pub(super) fn apply_effect(&mut self, layer: &Layer) -> EffectImage {
        EffectImage::of(layer, layer.texture.unwrap_or(self.white.id()))
    }
}

/// A layer whose content goes through `effect` as it is composited. Drawn by stage 7's layers-and-effects package:
/// until then GraphicsContext's default, the content composited as it is.
pub(super) fn push_effect_layer(r: &mut Renderer, bounds: Rect, effect: &RenderEffect, alpha: f32) {
    let _ = effect;
    r.push_layer_within(bounds, CornerRadii::Zero, alpha, LayerTransform::Identity, None);
}

/// What is drawn so far under `bounds` through `effect`, drawn back in place of it. Drawn by stage 7's
/// layers-and-effects package: until then GraphicsContext's default, nothing.
pub(super) fn draw_backdrop(r: &mut Renderer, bounds: Rect, effect: &RenderEffect, alpha: f32) {
    let _ = (r, bounds, effect, alpha);
}
