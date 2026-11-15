//! Shadows of paths and elevation shadows (C++'s drawShadowPath, drawElevationShadow): a path's shadow drawn white
//! once into a texture kept while it stays the same, then tinted where it goes (as Compose's DropShadowRenderer keeps
//! its blurred bitmap); an elevation shadow as Android draws one (Skia's SkShadowUtils: analytic ambient and spot
//! shadows of a flat rounded rect, else blurred), the light mapped into the target's pixels from the window's.
//!
//! Drawn by stage 7's layers-and-effects package: until then each is GraphicsContext's default (blurred effect
//! layers), as C++'s falls back to where it can't do better. A rounded rect's shadow is a primitive
//! ([`prims`](super::prims)'s).

use super::Renderer;
use crate::device::Texture;
use kompoz_graphics::{Offset, Outline, Path, Shadow};
use kompoz_ui::graphics_context::{
    ElevationShadow, ShadowLight, default_draw_elevation_shadow, default_draw_shadow_path,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A path's shadow's mask (as Compose's DropShadowRenderer and InnerShadowRenderer keep their blurred bitmaps), by
/// what it is drawn from (draw_shadow_path): white where the shadow is, its texture's top left `width` x `height`.
#[derive(Debug)]
pub(super) struct KeptShadow {
    pub(super) texture: Option<Texture>,
    #[allow(dead_code)] // (read by stage 7's layers-and-effects package: draw_shadow_path)
    pub(super) width: i32,
    #[allow(dead_code)] // (the same)
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

/// A shadow of any outline. Drawn by stage 7's layers-and-effects package (its mask kept): until then
/// GraphicsContext's default.
pub(super) fn draw_shadow_path(r: &mut Renderer, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
    default_draw_shadow_path(r, outline, origin, shadow, inner);
}

/// An elevation shadow. Drawn by stage 7's layers-and-effects package (Android's, analytically where it can): until
/// then GraphicsContext's default.
pub(super) fn draw_elevation_shadow(r: &mut Renderer, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
    default_draw_elevation_shadow(r, outline, origin, shadow);
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
