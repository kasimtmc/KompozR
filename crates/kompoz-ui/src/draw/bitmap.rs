//! Drawing into bitmaps (C++'s BitmapRenderer): a canvas over a bitmap ([`Canvas::from_image`]) records what it
//! draws into a [`Picture`] and hands it, as the canvas goes, to the platform's renderer, which draws it into the
//! bitmap; while there is none, the pictures wait on the bitmap for one to come.
//!
//! [`Canvas::from_image`]: super::Canvas::from_image

use super::Picture;
use kompoz_graphics::ImageBitmap;
use std::cell::RefCell;
use std::rc::Rc;

/// What draws into bitmaps: the platform's renderer, set on the UI thread while there is one.
pub trait BitmapRenderer {
    /// `bitmap` drawn into by `draws` (over what it holds, a unit a pixel), now: its new pixels stored
    /// (`ImageBitmap::store`) or kept by the renderer (`ImageBitmap::drawn_on_renderer`).
    fn draw(&self, bitmap: &ImageBitmap, draws: &Picture);
}

thread_local! {
    static RENDERER: RefCell<Option<Rc<dyn BitmapRenderer>>> = const { RefCell::new(None) };
    /// What was drawn into bitmaps while there was no renderer, in order (the bitmaps kept with it).
    static PENDING: RefCell<Vec<(ImageBitmap, Rc<Picture>)>> = const { RefCell::new(Vec::new()) };
}

/// Sets the renderer (none: there is none); what was drawn into bitmaps while there was none is drawn now, in the
/// order it was drawn.
pub fn set_bitmap_renderer(renderer: Option<Rc<dyn BitmapRenderer>>) {
    RENDERER.with(|r| *r.borrow_mut() = renderer.clone());
    let Some(renderer) = renderer else { return };
    let waiting = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
    for (bitmap, draws) in waiting {
        renderer.draw(&bitmap, &draws);
    }
}

/// `draws` drawn into `bitmap`: by the renderer now, or kept for one to come.
pub fn draw_into_bitmap(bitmap: &ImageBitmap, draws: Rc<Picture>) {
    let renderer = RENDERER.with(|r| r.borrow().clone());
    match renderer {
        Some(r) => r.draw(bitmap, &draws),
        None => PENDING.with(|p| p.borrow_mut().push((bitmap.clone(), draws))),
    }
}

/// What waits to be drawn into `bitmap` for a renderer to come, in order (what tests read of a bitmap a painter
/// drew into).
pub fn pending_draws(bitmap: &ImageBitmap) -> Vec<Rc<Picture>> {
    PENDING.with(|p| p.borrow().iter().filter(|(b, _)| b == bitmap).map(|(_, d)| d.clone()).collect())
}
