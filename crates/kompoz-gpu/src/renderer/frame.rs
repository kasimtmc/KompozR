//! Frames (C++'s drawFrame, move, blit, fillOpaque): a display list's frame drawn into a kept frame, only what
//! changed (its moves made through a texture, its areas drawn again each clipped to itself, white where nothing is
//! drawn unless an opaque rect covers the area); the kept frame copied onto the window's image, turned as the
//! display is; a rect set to a color as it is.
//!
//! Drawn by stage 7's frame package: until then a frame draws nothing (and says it drew all of it), a move can't be
//! made, and a blit and an opaque fill draw nothing.

use super::Renderer;
use crate::device::{IRect, TextureId};
use kompoz_graphics::Color;
use kompoz_ui::DisplayList;

impl Renderer {
    /// Sets a px rect of the target to `color` as it is (no blending). Drawn by stage 7's frame package: until then
    /// nothing.
    pub fn fill_opaque(&mut self, rect: IRect, color: Color) {
        let _ = (rect, color);
    }

    /// Copies `texture` into the target as it is, texel for pixel, turned by `quarter_turns` (clockwise, about the
    /// target: Device::swapchain_turns), upside down after that if `flipped`: all of it, or with `regions` (its
    /// texels') only those, each in a pass area of its own. Drawn by stage 7's frame package: until then nothing.
    pub fn blit(&mut self, texture: TextureId, quarter_turns: i32, flipped: bool, regions: Option<&[IRect]>) {
        let _ = (texture, quarter_turns, flipped, regions);
    }

    /// Moves the target's pixels in `from` by (dx, dy) (as it is: the pixels it leaves keep theirs); false if it
    /// can't (no texture to copy them through). Made by stage 7's frame package: until then it can't.
    pub fn move_pixels(&mut self, from: IRect, dx: i32, dy: i32) -> bool {
        let _ = (from, dx, dy);
        false
    }

    /// Draws the frame `list` recorded last (DisplayList::end) into `target`: all of it, or with `changes` only that,
    /// into the frame before it. True if only the changes were drawn. Drawn by stage 7's frame package: until then
    /// nothing is drawn, and it says it drew all of it.
    pub fn draw_frame(&mut self, target: TextureId, list: &DisplayList, changes: bool) -> bool {
        let _ = (target, list, changes);
        false
    }
}
