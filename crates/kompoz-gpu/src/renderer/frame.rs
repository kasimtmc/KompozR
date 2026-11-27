//! Frames (C++'s drawFrame, move, blit, fillOpaque): a display list's frame drawn into a kept frame, only what
//! changed (its moves made through a texture, its areas drawn again each clipped to itself, white where nothing is
//! drawn unless an opaque rect covers the area); the kept frame copied onto the window's image, turned as the
//! display is; a rect set to a color as it is.

use super::{Inputs, Pass, PassCopy, Renderer, clip, intersect};
use crate::device::{Filter, IRect, Sampler, TextureId};
use crate::shaders::{FILL, IMAGE, Prim};
use kompoz_graphics::{Color, Rect};
use kompoz_ui::DisplayList;
use kompoz_ui::graphics_context::GraphicsContext;

impl Renderer {
    /// Sets a px rect of the target to `color` as it is (no blending).
    pub fn fill_opaque(&mut self, rect: IRect, color: Color) {
        let bounds = [rect.x as f32, rect.y as f32, (rect.x + rect.width) as f32, (rect.y + rect.height) as f32];
        let p = Prim { bounds, shape: bounds, color: color.srgb_components(), ..Prim::default() }; // (as it is)
        self.add(FILL, &p, Inputs { opaque: true, ..Inputs::default() });
    }

    /// Copies `texture` into the target as it is, texel for pixel, turned by `quarter_turns` (clockwise, about the
    /// target: Device::swapchain_turns), upside down after that if `flipped`: all of it, or with `regions` (its
    /// texels') only those, each where it goes in the target.
    pub fn blit(&mut self, texture: TextureId, quarter_turns: i32, flipped: bool, regions: Option<&[IRect]>) {
        let (w, h) = (texture.width() as f32, texture.height() as f32);
        let mut p = Prim {
            bounds: [0.0, 0.0, w, h],
            shape: [0.0, 0.0, w, h], // (an image's texels go across its shape)
            uv: [0.0, 0.0, 1.0, 1.0],
            params: [1.0, 0.0, 0.0, 0.0],
            ..Prim::default()
        };
        // Its own transform, turning it into the target (xform's columns, then the move).
        match quarter_turns & 3 {
            1 => (p.xform, p.extra[0]) = ([0.0, 1.0, -1.0, 0.0], h),
            2 => (p.xform, p.extra[0], p.extra[1]) = ([-1.0, 0.0, 0.0, -1.0], w, h),
            3 => (p.xform, p.extra[1]) = ([0.0, -1.0, 1.0, 0.0], w),
            _ => {}
        }
        if flipped {
            // Upside down after that, in a target as high as it is turned to be.
            p.xform[1] = -p.xform[1];
            p.xform[3] = -p.xform[3];
            p.extra[1] = (if quarter_turns & 1 != 0 { w } else { h }) - p.extra[1];
        }
        // (Texel for pixel, on any GPU.)
        let inputs = Inputs {
            texture: Some(texture),
            sampler: Sampler { filter: Filter::Nearest, ..Sampler::default() },
            opaque: true,
            ..Inputs::default()
        };
        let Some(regions) = regions else {
            return self.add(IMAGE, &p, inputs);
        };
        let to = |x: f32, y: f32| {
            (p.xform[0] * x + p.xform[2] * y + p.extra[0], p.xform[1] * x + p.xform[3] * y + p.extra[1])
        };
        let s = self.scale;
        for r in regions {
            // Each where it goes in the target: its corners turned.
            let (a, b) = (to(r.x as f32, r.y as f32), to((r.x + r.width) as f32, (r.y + r.height) as f32));
            let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
            clip::push_clip(
                self,
                Rect::new(x0 / s, y0 / s, (x0 + (b.0 - a.0).abs()) / s, (y0 + (b.1 - a.1).abs()) / s),
            );
            self.add(IMAGE, &p, inputs);
            clip::pop_clip(self);
        }
    }

    /// Moves the target's pixels in `from` by (dx, dy) (as it is: the pixels it leaves keep theirs); false if it
    /// can't (no target, or no texture to copy them through).
    pub fn move_pixels(&mut self, from: IRect, dx: i32, dy: i32) -> bool {
        let Some(t) = self.targets.last() else { return false };
        let pass = t.pass;
        let target = self.passes[pass].target;
        let all = target.bounds();
        let region = intersect(intersect(from, all), IRect::new(-dx, -dy, all.width, all.height));
        if region.is_empty() || (dx == 0 && dy == 0) {
            return true;
        }
        // Through a texture of its own (a texture's copy into itself where they overlap isn't one): after what the
        // target's pass drew so far, which what follows goes on from.
        let Some(copy) = self.acquire(region.width, region.height, target.format()) else { return false };
        self.order.push(pass);
        let mut out = Pass::new(copy, None);
        out.copy = Some(PassCopy { from: target, region, x: 0, y: 0 });
        let mut back = Pass::new(target, None);
        back.copy = Some(PassCopy {
            from: copy,
            region: IRect::new(0, 0, region.width, region.height),
            x: region.x + dx,
            y: region.y + dy,
        });
        for p in [out, back] {
            self.order.push(self.passes.len());
            self.passes.push(p);
        }
        let next = self.passes.len();
        self.passes.push(Pass::new(target, None));
        if let Some(t) = self.targets.last_mut() {
            t.pass = next;
        }
        true
    }

    /// Draws the frame `list` recorded last (DisplayList::end) into `target`: all of it, or with `changes` only that,
    /// into the frame before it (its moves made first, then each of its areas drawn again clipped to itself). True
    /// if only the changes were drawn.
    pub fn draw_frame(&mut self, target: TextureId, list: &DisplayList, changes: bool) -> bool {
        self.begin_target(target, None);
        let mut changes = changes;
        let s = self.scale;
        let px = |v: f32| (v * s).round() as i32; // (whole pixels, as the list finds them)
        for m in list.moves() {
            if !changes {
                break;
            }
            let from = IRect::new(px(m.from.left), px(m.from.top), px(m.from.width()), px(m.from.height()));
            changes = self.move_pixels(from, px(m.by.x), px(m.by.y));
        }
        let areas: Vec<Option<Rect>> =
            if changes { list.areas().iter().map(|&a| Some(a)).collect() } else { vec![None] };
        let all = Rect::new(0.0, 0.0, target.width() as f32 / s, target.height() as f32 / s);
        for area in areas {
            if let Some(a) = area {
                self.push_clip(a);
            }
            let cover = list.opaque_cover(area.unwrap_or(all));
            if cover.is_none() {
                self.fill_opaque(target.bounds(), self.frame_background);
            }
            list.replay_from(self, area, cover.unwrap_or(0));
            if area.is_some() {
                self.pop_clip();
            }
        }
        self.end_target();
        changes
    }
}
