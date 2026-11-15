//! The atlas: shelf packing, pages by format, dirty rectangles, pages cleared for reuse.

use crate::glyph::atlas::{AtlasPage, AtlasSlot, DirtyRect, GlyphAtlas, PageFormat};

const SIZE: u32 = AtlasPage::SIZE;

#[test]
fn glyphs_go_on_shelves_a_texel_apart() {
    let mut atlas = GlyphAtlas::new();
    let a = atlas.add(PageFormat::Coverage, &[1; 6], 3, 2).unwrap();
    let b = atlas.add(PageFormat::Coverage, &[2; 20], 4, 5).unwrap();
    assert_eq!(a, AtlasSlot { page: 0, u: 0, v: 0 });
    assert_eq!(b, AtlasSlot { page: 0, u: 4, v: 0 });
    // Its texels are where it was put, rows down the page.
    let page = atlas.page(0);
    assert_eq!(page.format(), PageFormat::Coverage);
    assert_eq!(page.stride(), SIZE as usize);
    assert_eq!(&page.pixels()[..8], &[1, 1, 1, 0, 2, 2, 2, 2]);
    assert_eq!(page.pixels()[SIZE as usize + 4], 2);
    assert_eq!(page.pixels()[2 * SIZE as usize], 0);
    // A glyph past the shelf's end starts the next shelf, under the tallest one on it (and its gap).
    let wide = vec![3u8; (SIZE - 5) as usize];
    let c = atlas.add(PageFormat::Coverage, &wide, SIZE - 5, 1).unwrap();
    assert_eq!(c, AtlasSlot { page: 0, u: 0, v: 6 });
    // Too big for a page: none.
    assert!(atlas.add(PageFormat::Coverage, &[], SIZE, 1).is_none());
    assert!(atlas.add(PageFormat::Coverage, &[], 1, SIZE).is_none());
}

#[test]
fn colour_glyphs_have_pages_of_their_own() {
    let mut atlas = GlyphAtlas::new();
    atlas.add(PageFormat::Coverage, &[9; 4], 2, 2).unwrap();
    let c = atlas.add(PageFormat::Rgba, &[1, 2, 3, 4, 5, 6, 7, 8], 2, 1).unwrap();
    assert_eq!(c, AtlasSlot { page: 1, u: 0, v: 0 });
    assert_eq!(atlas.page(1).format(), PageFormat::Rgba);
    assert_eq!(atlas.page(1).stride(), SIZE as usize * 4);
    assert_eq!(&atlas.page(1).pixels()[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    // Coverage still goes on its own page.
    assert_eq!(atlas.add(PageFormat::Coverage, &[9], 1, 1).unwrap().page, 0);
}

#[test]
fn dirty_rectangles_hold_what_changed() {
    let mut atlas = GlyphAtlas::new();
    atlas.add(PageFormat::Rgba, &[0; 16], 2, 2).unwrap();
    // A new page is dirty all over (its texture has nothing yet).
    let d = atlas.page_mut(0).take_dirty();
    assert_eq!(d, DirtyRect { x0: 0, y0: 0, x1: SIZE, y1: SIZE });
    assert!(atlas.page_mut(0).take_dirty().is_empty());
    let a = atlas.add(PageFormat::Rgba, &[7; 12], 3, 1).unwrap();
    let b = atlas.add(PageFormat::Rgba, &[8; 16], 1, 4).unwrap();
    let d = atlas.page_mut(0).take_dirty();
    assert_eq!(d, DirtyRect { x0: a.u, y0: 0, x1: b.u + 1, y1: 4 });
    assert_eq!((d.width(), d.height()), (b.u + 1 - a.u, 4));
    // The bytes to upload: from its first texel, rows a stride apart, ending at its right edge.
    let bytes = atlas.page(0).rect_bytes(d);
    assert_eq!(bytes.len(), 3 * SIZE as usize * 4 + (d.width() as usize * 4));
    assert_eq!(&bytes[..12], &[7; 12]);
    atlas.page_mut(0).mark_all_dirty();
    assert_eq!(atlas.page_mut(0).take_dirty(), DirtyRect { x0: 0, y0: 0, x1: SIZE, y1: SIZE });
    assert!(atlas.page(0).rect_bytes(DirtyRect::default()).is_empty());
}

#[test]
fn full_pages_make_room_on_new_ones() {
    let mut atlas = GlyphAtlas::new();
    let big = 600u32;
    let glyph = vec![1u8; (big * big) as usize];
    let first = atlas.add(PageFormat::Coverage, &glyph, big, big).unwrap();
    let second = atlas.add(PageFormat::Coverage, &glyph, big, big).unwrap();
    assert_eq!((first.page, second.page), (0, 1));
    assert_eq!(atlas.page_count(), 2);
}

#[test]
fn pages_unused_longest_are_cleared_between_frames() {
    // Six pages of coverage in the first frame, then only the newest drawn from.
    let mut atlas = GlyphAtlas::new();
    let big = 600u32;
    let glyph = vec![5u8; (big * big) as usize];
    for i in 0..6 {
        assert_eq!(atlas.add(PageFormat::Coverage, &glyph, big, big).unwrap().page, i);
    }
    for page in 0..6 {
        atlas.page_mut(page).take_dirty();
    }
    // Pages unused for two frames stay (a frame's glyphs stay where they were put until it has been drawn)...
    atlas.use_page(5);
    assert!(atlas.end_frame().is_empty());
    atlas.use_page(5);
    assert!(atlas.end_frame().is_empty());
    // ...then, with more than four pages, those unused longest go, down to four (never the current one).
    atlas.use_page(5);
    assert_eq!(atlas.end_frame(), vec![0, 1]);
    for i in [0, 1] {
        assert!(!atlas.valid(i, 0));
        assert_eq!(atlas.generation(i), 1);
        assert!(atlas.page(i).pixels().iter().all(|&t| t == 0));
        assert_eq!(atlas.page_mut(i).take_dirty(), DirtyRect { x0: 0, y0: 0, x1: SIZE, y1: SIZE });
    }
    assert!((2..6).all(|i| atlas.valid(i, 0)));
    // A cleared page is reused before a new one is made.
    let reused = atlas.add(PageFormat::Coverage, &glyph, big, big).unwrap();
    assert_eq!(reused, AtlasSlot { page: 0, u: 0, v: 0 });
    assert_eq!(atlas.page_count(), 6);
}

#[test]
fn few_pages_are_kept_whatever_happens() {
    let mut atlas = GlyphAtlas::new();
    for _ in 0..GlyphAtlas::KEEP {
        atlas.add(PageFormat::Coverage, &vec![1u8; 600 * 600], 600, 600).unwrap();
    }
    for _ in 0..10 {
        assert!(atlas.end_frame().is_empty());
    }
    assert!((0..GlyphAtlas::KEEP).all(|i| atlas.valid(i, 0)));
    assert_eq!(atlas.frame(), 11);
}
