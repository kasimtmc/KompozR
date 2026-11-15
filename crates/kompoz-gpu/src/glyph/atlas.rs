//! The glyph atlas (the C++ Kompoz's glyph_atlas.hpp): rasterized glyphs packed into pages of pixels kept on the CPU,
//! which a renderer uploads to its own textures as they change. Coverage pages (a byte a texel: a glyph that takes
//! the text's colour, sampled as white with its coverage for alpha) and colour pages (straight-alpha RGBA: an emoji,
//! drawn as it is).
//!
//! Glyphs go on a page in shelves, a texel apart. A page that fills up leaves room to the next one; with more pages
//! than the atlas keeps whatever happens, those not drawn from for a while are cleared for reuse between frames,
//! never during one (a frame's glyphs stay where they were put until it has been drawn), their generation moving on
//! so that whoever cached a glyph there knows to rasterize it again.

/// What a page's texels are: coverage (a byte each) or straight-alpha RGBA (four bytes each).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PageFormat {
    Coverage,
    Rgba,
}

impl PageFormat {
    /// Bytes a texel.
    pub fn bytes_per_texel(self) -> usize {
        match self {
            PageFormat::Coverage => 1,
            PageFormat::Rgba => 4,
        }
    }

    fn index(self) -> usize {
        match self {
            PageFormat::Coverage => 0,
            PageFormat::Rgba => 1,
        }
    }
}

/// A rectangle of texels (x0 ≤ x < x1, y0 ≤ y < y1) that changed since the renderer last uploaded the page.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct DirtyRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl DirtyRect {
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    /// Grown to hold (x0, y0)–(x1, y1).
    pub fn add(&mut self, x0: u32, y0: u32, x1: u32, y1: u32) {
        if self.is_empty() {
            *self = DirtyRect { x0, y0, x1, y1 };
        } else {
            self.x0 = self.x0.min(x0);
            self.y0 = self.y0.min(y0);
            self.x1 = self.x1.max(x1);
            self.y1 = self.y1.max(y1);
        }
    }

    pub fn width(&self) -> u32 {
        self.x1.saturating_sub(self.x0)
    }

    pub fn height(&self) -> u32 {
        self.y1.saturating_sub(self.y0)
    }
}

/// A page of the atlas: PAGE_SIZE² texels of its format, rows top down.
pub struct AtlasPage {
    format: PageFormat,
    pixels: Vec<u8>,
    dirty: DirtyRect,
    generation: u64,
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
    /// The frame it was last drawn from.
    last_used: u64,
}

impl AtlasPage {
    /// Texels each way.
    pub const SIZE: u32 = 1024;

    fn new(format: PageFormat) -> AtlasPage {
        let size = AtlasPage::SIZE as usize;
        AtlasPage {
            format,
            pixels: vec![0; size * size * format.bytes_per_texel()],
            dirty: DirtyRect { x0: 0, y0: 0, x1: AtlasPage::SIZE, y1: AtlasPage::SIZE },
            generation: 0,
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
            last_used: 0,
        }
    }

    pub fn format(&self) -> PageFormat {
        self.format
    }

    /// Its texels, rows top down, SIZE · bytes-per-texel bytes a row.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Bytes a row.
    pub fn stride(&self) -> usize {
        AtlasPage::SIZE as usize * self.format.bytes_per_texel()
    }

    /// The bytes of `rect`'s rows from its first texel (each row `stride()` on from the one before; the last row
    /// ends at the rectangle's right edge): what a renderer uploads for it.
    pub fn rect_bytes(&self, rect: DirtyRect) -> &[u8] {
        if rect.is_empty() {
            return &[];
        }
        let bpt = self.format.bytes_per_texel();
        let start = rect.y0 as usize * self.stride() + rect.x0 as usize * bpt;
        let end = (rect.y1 as usize - 1) * self.stride() + rect.x1 as usize * bpt;
        &self.pixels[start..end]
    }

    /// What changed since the last call (the whole page at first, and after it was cleared).
    pub fn take_dirty(&mut self) -> DirtyRect {
        std::mem::take(&mut self.dirty)
    }

    /// All of it is to be uploaded again (a renderer's texture for it was new, or lost).
    pub fn mark_all_dirty(&mut self) {
        self.dirty = DirtyRect { x0: 0, y0: 0, x1: AtlasPage::SIZE, y1: AtlasPage::SIZE };
    }

    /// Moves on each time it is cleared for reuse: what was cached on it is gone.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether a `w` × `h` glyph fits where the next one would go (on this shelf, or the next).
    fn fits(&self, w: u32, h: u32) -> bool {
        let y = if self.shelf_x + w + 1 > AtlasPage::SIZE { self.shelf_y + self.shelf_height } else { self.shelf_y };
        y + h < AtlasPage::SIZE
    }

    fn is_fresh(&self) -> bool {
        self.shelf_x == 0 && self.shelf_y == 0
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
        self.mark_all_dirty();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;
        self.generation += 1;
    }
}

/// Where a glyph was put: its page, its top left texel there.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AtlasSlot {
    pub page: usize,
    pub u: u32,
    pub v: u32,
}

/// The pages, the one each format adds to now, the frame count.
pub struct GlyphAtlas {
    pages: Vec<AtlasPage>,
    current: [Option<usize>; 2],
    frame: u64,
}

impl Default for GlyphAtlas {
    fn default() -> GlyphAtlas {
        GlyphAtlas::new()
    }
}

impl GlyphAtlas {
    /// Pages kept whatever happens (4 MB of colour each, 1 MB of coverage).
    pub const KEEP: usize = 4;

    pub fn new() -> GlyphAtlas {
        GlyphAtlas { pages: Vec::new(), current: [None, None], frame: 1 }
    }

    /// A spot for a `w` × `h` glyph (`pixels`: w · h texels of `format`, rows top down) on the format's current
    /// page or a fresh one; None if it is too big for a page.
    pub fn add(&mut self, format: PageFormat, pixels: &[u8], w: u32, h: u32) -> Option<AtlasSlot> {
        let size = AtlasPage::SIZE;
        if w + 1 > size || h + 1 > size {
            return None;
        }
        let bpt = format.bytes_per_texel();
        debug_assert!(pixels.len() >= (w * h) as usize * bpt);
        let current = match self.current[format.index()] {
            Some(c) if self.pages[c].fits(w, h) => c,
            _ => self.fresh_page(format),
        };
        self.current[format.index()] = Some(current);
        let frame = self.frame;
        let p = &mut self.pages[current];
        if p.shelf_x + w + 1 > size {
            p.shelf_x = 0;
            p.shelf_y += p.shelf_height;
            p.shelf_height = 0;
        }
        let (u, v) = (p.shelf_x, p.shelf_y);
        p.shelf_x += w + 1;
        p.shelf_height = p.shelf_height.max(h + 1);
        let row_bytes = w as usize * bpt;
        let stride = p.stride();
        for row in 0..h as usize {
            let from = row * row_bytes;
            let to = (v as usize + row) * stride + u as usize * bpt;
            p.pixels[to..to + row_bytes].copy_from_slice(&pixels[from..from + row_bytes]);
        }
        p.dirty.add(u, v, u + w, v + h);
        p.last_used = frame;
        Some(AtlasSlot { page: current, u, v })
    }

    /// A glyph on `page` is drawn this frame: the page stays until the frame is over.
    pub fn use_page(&mut self, page: usize) {
        if let Some(p) = self.pages.get_mut(page) {
            p.last_used = self.frame;
        }
    }

    /// Whether what was put on `page` at `generation` is still there.
    pub fn valid(&self, page: usize, generation: u64) -> bool {
        self.pages.get(page).is_some_and(|p| p.generation == generation)
    }

    pub fn generation(&self, page: usize) -> u64 {
        self.pages[page].generation
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page(&self, i: usize) -> &AtlasPage {
        &self.pages[i]
    }

    pub fn page_mut(&mut self, i: usize) -> &mut AtlasPage {
        &mut self.pages[i]
    }

    /// The frame being drawn (counted from 1).
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Between frames: with more pages than KEEP, those unused longest (more than two frames ago, and not a
    /// format's current page) are cleared for reuse, their glyphs rasterized again when drawn. Returns the pages
    /// cleared.
    pub fn end_frame(&mut self) -> Vec<usize> {
        self.frame += 1;
        let mut cleared = Vec::new();
        if self.pages.len() <= GlyphAtlas::KEEP {
            return cleared;
        }
        let mut order: Vec<usize> = (0..self.pages.len()).collect();
        order.sort_by_key(|&i| self.pages[i].last_used);
        for &i in &order[..order.len() - GlyphAtlas::KEEP] {
            if !self.current.contains(&Some(i)) && self.frame - self.pages[i].last_used > 2 {
                self.pages[i].clear();
                cleared.push(i);
            }
        }
        cleared
    }

    /// A page of `format` with room: one cleared earlier, else a new one.
    fn fresh_page(&mut self, format: PageFormat) -> usize {
        let current = self.current[format.index()];
        if let Some(i) = (0..self.pages.len())
            .find(|&i| Some(i) != current && self.pages[i].format == format && self.pages[i].is_fresh())
        {
            return i;
        }
        self.pages.push(AtlasPage::new(format));
        self.pages.len() - 1
    }
}
