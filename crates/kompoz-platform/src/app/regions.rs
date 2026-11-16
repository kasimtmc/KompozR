//! What of the kept frame a swapchain image needs copied over (C++'s drawFrame's part of it): each frame shown
//! records what it changed in the kept pixels (its areas and where its moves went, whole pixels; none: all of it);
//! an image holding the frame `age` frames ago (the device's `swapchain_age`) needs what changed since, each pixel
//! once, and all of it once that takes half the window (an image's pixels kept cost loading on a tiling GPU, as all
//! of them copied don't).

use kompoz_gpu::device::IRect;
use kompoz_graphics::Rect;
use kompoz_ui::display_list::Move;
use std::collections::VecDeque;

/// How many frames' changes are kept (an image older than that is copied whole).
pub const SHOWN_FRAMES: usize = 8;

/// What a frame changed in the kept pixels: its areas out to whole pixels, and where its moves went (as the
/// renderer moves them: rounded).
pub fn frame_changes(areas: &[Rect], moves: &[Move]) -> Vec<IRect> {
    let mut changed: Vec<IRect> = areas
        .iter()
        .map(|a| {
            let (x0, y0) = (a.left.floor() as i32, a.top.floor() as i32);
            IRect::new(x0, y0, a.right.ceil() as i32 - x0, a.bottom.ceil() as i32 - y0)
        })
        .collect();
    let px = |v: f32| v.round() as i32;
    changed.extend(moves.iter().map(|m| {
        IRect::new(px(m.from.left) + px(m.by.x), px(m.from.top) + px(m.by.y), px(m.from.width()), px(m.from.height()))
    }));
    changed
}

/// The frames shown, the last first: what each changed (none: all of it).
#[derive(Clone, Debug, Default)]
pub struct ShownChanges(VecDeque<Option<Vec<IRect>>>);

impl ShownChanges {
    /// A frame shown, having changed `changed` (none: all of it).
    pub fn shown(&mut self, changed: Option<Vec<IRect>>) {
        self.0.push_front(changed);
        self.0.truncate(SHOWN_FRAMES);
    }

    /// What a swapchain image holding the frame `age` frames ago needs copied over, in a `width` x `height` window:
    /// the changes since, clipped to the window, those overlapping as one; none: all of it (its age unknown or
    /// beyond what is kept, a frame since that changed all, or half the window or more to copy).
    pub fn copied(&self, age: i32, width: i32, height: i32) -> Option<Vec<IRect>> {
        if age <= 0 || age as usize > self.0.len() {
            return None;
        }
        let mut c = Vec::new();
        for frame in self.0.iter().take(age as usize) {
            c.extend_from_slice(frame.as_ref()?);
        }
        for r in &mut c {
            let (x0, y0) = (r.x.max(0), r.y.max(0));
            let (x1, y1) = ((r.x + r.width).min(width), (r.y + r.height).min(height));
            *r = IRect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0));
        }
        c.retain(|r| r.width > 0 && r.height > 0);
        let overlap = |a: &IRect, b: &IRect| {
            a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
        };
        'merging: loop {
            for i in 0..c.len() {
                for j in i + 1..c.len() {
                    if overlap(&c[i], &c[j]) {
                        let (a, b) = (c[i], c[j]);
                        let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
                        let x1 = (a.x + a.width).max(b.x + b.width);
                        let y1 = (a.y + a.height).max(b.y + b.height);
                        c[i] = IRect::new(x0, y0, x1 - x0, y1 - y0);
                        c.remove(j);
                        continue 'merging;
                    }
                }
            }
            break;
        }
        let area: i64 = c.iter().map(|r| r.width as i64 * r.height as i64).sum();
        if area * 2 > width as i64 * height as i64 { None } else { Some(c) }
    }
}

/// How much of the screen a frame drew again (its areas), as a fraction: 0 if nothing, a little at least if
/// anything (C++'s `drawnArea`).
pub fn drawn_area(areas: &[Rect], width: f32, height: f32) -> f32 {
    if areas.is_empty() {
        return 0.0;
    }
    let drawn: f32 = areas.iter().map(|a| a.width() * a.height()).sum();
    (drawn / (width * height).max(1.0)).max(1e-4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::Offset;

    fn r(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect::new(x, y, w, h)
    }

    #[test]
    fn a_frames_changes_are_its_areas_and_where_its_moves_went() {
        let areas = [Rect::new(1.5, 2.25, 10.2, 20.0)];
        let moves = [Move { from: Rect::new(0.0, 100.0, 50.0, 300.0), by: Offset::new(0.4, -20.6) }];
        assert_eq!(frame_changes(&areas, &moves), [r(1, 2, 10, 18), r(0, 79, 50, 200)]);
    }

    #[test]
    fn an_image_gets_what_changed_since_it_was_shown() {
        let mut shown = ShownChanges::default();
        assert_eq!(shown.copied(1, 100, 100), None); // (nothing shown yet: all of it)
        shown.shown(None); // the first frame: all of it
        shown.shown(Some(vec![r(0, 0, 10, 10)]));
        shown.shown(Some(vec![r(5, 5, 10, 10), r(50, 50, 5, 5)]));
        // One frame old: the last frame's changes.
        assert_eq!(shown.copied(1, 100, 100), Some(vec![r(5, 5, 10, 10), r(50, 50, 5, 5)]));
        // Two: both frames' (the first two overlap: merged).
        assert_eq!(shown.copied(2, 100, 100), Some(vec![r(0, 0, 15, 15), r(50, 50, 5, 5)]));
        // Three: one of them changed all of it; unknown (0) or older than kept: all of it.
        assert_eq!(shown.copied(3, 100, 100), None);
        assert_eq!(shown.copied(0, 100, 100), None);
        assert_eq!(shown.copied(9, 100, 100), None);
    }

    #[test]
    fn what_is_copied_is_clipped_and_all_of_it_past_half() {
        let mut shown = ShownChanges::default();
        shown.shown(Some(vec![r(-5, -5, 10, 10), r(95, 95, 20, 20), r(200, 0, 5, 5)]));
        assert_eq!(shown.copied(1, 100, 100), Some(vec![r(0, 0, 5, 5), r(95, 95, 5, 5)]));
        shown.shown(Some(vec![r(0, 0, 100, 51)])); // (half the window and a row: all of it)
        assert_eq!(shown.copied(1, 100, 100), None);
        shown.shown(Some(Vec::new())); // a frame shown that changed nothing
        assert_eq!(shown.copied(1, 100, 100), Some(Vec::new()));
        // Merging goes on until nothing overlaps: a chain merges into one.
        let mut chain = ShownChanges::default();
        chain.shown(Some(vec![r(0, 0, 10, 10), r(20, 0, 10, 10), r(8, 0, 14, 2)]));
        assert_eq!(chain.copied(1, 100, 100), Some(vec![r(0, 0, 30, 10)]));
        // Only the last 8 frames are kept.
        for _ in 0..8 {
            chain.shown(Some(vec![r(0, 0, 1, 1)]));
        }
        assert_eq!(chain.copied(8, 100, 100), Some(vec![r(0, 0, 1, 1)]));
        assert_eq!(chain.copied(9, 100, 100), None);
    }

    #[test]
    fn the_area_drawn_again_is_a_fraction_of_the_screen() {
        assert_eq!(drawn_area(&[], 100.0, 100.0), 0.0);
        assert_eq!(drawn_area(&[Rect::new(0.0, 0.0, 50.0, 100.0)], 100.0, 100.0), 0.5);
        assert_eq!(drawn_area(&[Rect::new(0.0, 0.0, 0.1, 0.1)], 100.0, 100.0), 1e-4);
    }
}
