//! Images from http(s) on the web (C++'s): fetched by the browser (its HTTP cache is the disk cache) and decoded by
//! it (`createImageBitmap`: off the page's thread, every format the browser has, AVIF and the like), scaled down
//! for the size asked for on an OffscreenCanvas, their pixels handed back to the app. The app's own images (its
//! assets, files dropped on it) are decoded by KompozR's decoders, as everywhere: what [`image_loader`] does with a
//! path.
//!
//! [`image_loader`]: crate::image_loader

use super::sys;
use crate::image_loader::{UrlLoadDone, set_url_loader};
use kompoz_graphics::ImageBitmap;
use kompoz_ui::platform::ImageRequestSize;
use std::cell::RefCell;

thread_local! {
    /// The loads under way: the page's number of each and what gets the image.
    static LOADS: RefCell<Vec<(u32, UrlLoadDone)>> = const { RefCell::new(Vec::new()) };
}

/// The image loader's URLs through the browser from now on.
pub(super) fn start() {
    set_url_loader(Some(load));
}

/// `url` loaded by the browser for `size`; `done` called with it at a turn of the loop after it is ([`finish`]).
fn load(url: &str, size: ImageRequestSize, done: UrlLoadDone) {
    let id = sys::image_load(url, size.width.max(0) as u32, size.height.max(0) as u32, size.fill);
    LOADS.with(|l| l.borrow_mut().push((id, done)));
}

/// The loads the browser has finished handed their images (none for one that failed); the others wait.
pub(super) fn finish() {
    let finished: Vec<(UrlLoadDone, Option<ImageBitmap>)> = LOADS.with(|l| {
        let mut loads = l.borrow_mut();
        let mut finished = Vec::new();
        let mut i = 0;
        while i < loads.len() {
            match sys::image_result(loads[i].0) {
                None => i += 1,
                Some(result) => {
                    let (_, done) = loads.remove(i);
                    let image = result.and_then(|(w, h, rgba)| ImageBitmap::from_pixels(w as i32, h as i32, rgba));
                    finished.push((done, image));
                }
            }
        }
        finished
    });
    // (Called after the loads' list is let go of: what `done` does may load again.)
    for (done, image) in finished {
        done(image);
    }
}
