//! The real font backend: faces from a [`FontCollection`] (the system's through fontique, or fonts registered by
//! bytes), picked per grapheme cluster ([`FontFallback`](crate::font::fallback)'s rules), shaped by harfrust
//! (HarfBuzz's shaping: ligatures, kerning, Arabic joining, Indic reordering, emoji sequences), measured and read
//! by skrifa.

use super::cache::{PlanKey, RawGlyph, RawRun, RunKey, ShapeCaches};
use super::face::Faces;
use super::{
    FontBackend, FontInstance, FontMetrics, FontRun, FontRunRequest, ShapeCacheStats, ShapeRequest, ShapedGlyph,
    ShapedRun, letter_spacing_sides,
};
use crate::draw::GlyphColorKind;
use crate::font::FontCollection;
use crate::font::FontId;
use crate::font::fallback::FontFallback;
use harfrust::{Buffer, BufferFlags, Direction, Feature, Language, Script, ShapeOptions, ShapePlan, ShaperFont, Tag};
use kompoz_graphics::Rect;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Scripts whose letters join or hang from a line, which letter spacing would tear apart (minikin's
/// isScriptOkForLetterspacing, and Syriac with them as the C++ Kompoz had it).
const NOT_LETTER_SPACED: [&[u8; 4]; 15] = [
    b"Arab", b"Syrc", b"Nkoo", b"Mand", b"Mong", b"Phag", b"Phlp", b"Deva", b"Beng", b"Guru", b"Modi", b"Shrd",
    b"Sylo", b"Tirh", b"Ogam",
];

/// How much context each side of a run is shaped with (HarfBuzz's).
const CONTEXT: usize = 5;

/// The font backend over harfrust and skrifa, its faces from a [`FontCollection`]. Shaped runs and shape plans are
/// cached ([`ShapeCacheStats`]).
pub struct HarfrustBackend {
    collection: Rc<FontCollection>,
    faces: Faces,
    fallback: FontFallback,
    caches: RefCell<ShapeCaches>,
    buffer: RefCell<Option<Buffer>>,
}

impl HarfrustBackend {
    pub fn new(collection: Rc<FontCollection>) -> HarfrustBackend {
        HarfrustBackend {
            collection,
            faces: Faces::default(),
            fallback: FontFallback::default(),
            caches: RefCell::new(ShapeCaches::new()),
            buffer: RefCell::new(None),
        }
    }

    /// The collection its faces come from.
    pub fn collection(&self) -> &Rc<FontCollection> {
        &self.collection
    }

    /// How its caches have answered so far.
    pub fn cache_stats(&self) -> ShapeCacheStats {
        self.caches.borrow().stats()
    }

    /// The normalised coordinates (F2Dot14 bits) a font's axes take at these settings (user units; none for a
    /// font without axes or one that can't be read).
    pub fn coords(&self, font: FontId, settings: &[([u8; 4], f32)]) -> Rc<[i16]> {
        self.faces.get(font).map_or_else(|| Rc::from([]), |f| f.coords(settings))
    }

    /// Whether a font has a glyph for `c`.
    pub fn has_char(&self, font: FontId, c: char) -> bool {
        self.faces.get(font).is_some_and(|f| f.has_char(c))
    }

    /// The weight and italics a font is designed in (at its default location).
    pub fn design(&self, font: FontId) -> Option<(f32, bool)> {
        self.faces.get(font).map(|f| (f.weight, f.italic))
    }

    /// Shapes `key.text` in font units (cached).
    fn shape_raw(&self, key: RunKey) -> Rc<RawRun> {
        if let Some(run) = self.caches.borrow_mut().runs.get(&key) {
            return run;
        }
        let run = Rc::new(self.shape_uncached(&key));
        self.caches.borrow_mut().runs.insert(key, run.clone());
        run
    }

    fn shape_uncached(&self, key: &RunKey) -> RawRun {
        let Some(face) = self.faces.get(key.font) else {
            // A font that can't be read: every character its missing glyph, taking no room.
            let mut glyphs: Vec<RawGlyph> = key
                .text
                .char_indices()
                .map(|(i, _)| RawGlyph {
                    id: 0,
                    cluster: i as u32,
                    x_advance: 0,
                    y_advance: 0,
                    x_offset: 0,
                    y_offset: 0,
                    unsafe_to_break: false,
                })
                .collect();
            if key.rtl {
                glyphs.reverse();
            }
            return RawRun { glyphs, script: key.script.unwrap_or(*b"Zyyy") };
        };
        let font = face.instance(&key.coords);
        let shaper = ShaperFont::new(&font);
        let mut buffer = self.buffer.borrow_mut().take().unwrap_or_default();
        buffer.clear();
        for (i, c) in key.text.char_indices() {
            buffer.push(c as u32, i as u32);
        }
        buffer.set_pre_context(&key.before);
        buffer.set_post_context(&key.after);
        let mut flags = BufferFlags::empty();
        if key.begins_text {
            flags |= BufferFlags::BEGINNING_OF_TEXT;
        }
        if key.ends_text {
            flags |= BufferFlags::END_OF_TEXT;
        }
        buffer.set_flags(flags);
        buffer.set_direction(if key.rtl { Direction::RightToLeft } else { Direction::LeftToRight });
        if let Some(script) = key.script {
            buffer.set_script(Script::from_iso15924_tag(Tag::new(&script)));
        }
        let language = key.language.as_deref().and_then(Language::new);
        buffer.set_language(language.clone());
        buffer.guess_segment_properties();
        let script = buffer.script().map_or(*b"Zyyy", |s| s.tag().to_be_bytes());
        let features: Vec<Feature> = key
            .features
            .iter()
            .map(|f| Feature { tag: Tag::new(&f.tag), value: f.value, start: f.start, end: f.end })
            .collect();
        let plan = self.plan(key, &font, buffer.script(), language.as_ref(), &features);
        let result = harfrust::shape(&shaper, &mut buffer, ShapeOptions::new().plan(Some(&plan)).features(&features));
        let glyphs = match result {
            Ok(()) => buffer
                .glyph_infos()
                .iter()
                .zip(buffer.glyph_positions())
                .map(|(info, pos)| RawGlyph {
                    id: info.glyph_id,
                    cluster: info.cluster,
                    x_advance: pos.x_advance,
                    y_advance: pos.y_advance,
                    x_offset: pos.x_offset,
                    y_offset: pos.y_offset,
                    unsafe_to_break: info.unsafe_to_break(),
                })
                .collect(),
            Err(_) => Vec::new(),
        };
        *self.buffer.borrow_mut() = Some(buffer);
        RawRun { glyphs, script }
    }

    /// The shape plan for a run's font and settings (cached).
    fn plan(
        &self,
        key: &RunKey,
        font: &harfrust::Font,
        script: Option<Script>,
        language: Option<&Language>,
        features: &[Feature],
    ) -> Rc<ShapePlan> {
        let plan_key = PlanKey {
            font: key.font,
            coords: key.coords.clone(),
            rtl: key.rtl,
            script: script.map(|s| s.tag().to_be_bytes()),
            language: key.language.clone(),
            features: key.features.clone(),
        };
        if let Some(plan) = self.caches.borrow_mut().plans.get(&plan_key) {
            return plan;
        }
        let direction = if key.rtl { Direction::RightToLeft } else { Direction::LeftToRight };
        let plan = Rc::new(ShapePlan::new(font, direction, script, language, features));
        self.caches.borrow_mut().plans.insert(plan_key, plan.clone());
        plan
    }
}

/// At most `n` characters of the end of `s`.
fn last_chars(s: &str, n: usize) -> &str {
    let start = s.char_indices().rev().nth(n.saturating_sub(1)).map_or(0, |(i, _)| i);
    &s[start..]
}

/// At most `n` characters of the start of `s`.
fn first_chars(s: &str, n: usize) -> &str {
    let end = s.char_indices().nth(n).map_or(s.len(), |(i, _)| i);
    &s[..end]
}

impl FontBackend for HarfrustBackend {
    fn font_runs(&self, request: &FontRunRequest<'_>) -> Vec<FontRun> {
        self.fallback.font_runs(&self.collection, &self.faces, request)
    }

    fn shape(&self, request: &ShapeRequest<'_>) -> ShapedRun {
        let range = request.range.clone();
        let key = RunKey {
            text: request.text[range.clone()].into(),
            before: last_chars(&request.text[..range.start], CONTEXT).into(),
            after: first_chars(&request.text[range.end..], CONTEXT).into(),
            begins_text: range.start == 0,
            ends_text: range.end == request.text.len(),
            font: request.font.font,
            coords: request.font.coords.clone(),
            rtl: request.rtl,
            script: request.script,
            language: request.language.map(|l| l.to_language_tag().into_boxed_str()),
            features: request.features.into(),
        };
        let raw = self.shape_raw(key);
        let upem = self.faces.get(request.font.font).map_or(1000.0, |f| f.upem);
        let scale = request.font.size / upem;
        let sx = scale * request.scale_x;
        let (before, after) = if NOT_LETTER_SPACED.contains(&&raw.script) {
            (0.0, 0.0)
        } else {
            letter_spacing_sides(request.letter_spacing, request.scale_x, request.linear)
        };
        let mut glyphs: Vec<ShapedGlyph> = raw
            .glyphs
            .iter()
            .map(|g| ShapedGlyph {
                id: g.id,
                cluster: range.start + g.cluster as usize,
                x_advance: g.x_advance as f32 * sx,
                y_advance: g.y_advance as f32 * scale,
                x_offset: g.x_offset as f32 * sx,
                y_offset: g.y_offset as f32 * scale,
                unsafe_to_break: g.unsafe_to_break,
            })
            .collect();
        if before != 0.0 || after != 0.0 {
            // Some before each cluster, the rest after (minikin's): its glyphs (a base and its marks alike) move in
            // by what is before, its last one carries all.
            let mut i = 0;
            while i < glyphs.len() {
                let cluster = glyphs[i].cluster;
                let mut j = i;
                while j < glyphs.len() && glyphs[j].cluster == cluster {
                    glyphs[j].x_offset += before;
                    j += 1;
                }
                glyphs[j - 1].x_advance += before + after;
                i = j;
            }
        }
        let advance = glyphs.iter().map(|g| g.x_advance).sum();
        ShapedRun { glyphs, advance }
    }

    fn metrics(&self, font: &FontInstance) -> FontMetrics {
        self.faces.get(font.font).map_or_else(FontMetrics::default, |f| f.metrics(&font.coords, font.size))
    }

    fn glyph_bounds(&self, font: &FontInstance, glyph: u32) -> Rect {
        self.faces.get(font.font).map_or(Rect::Zero, |f| f.glyph_bounds(&font.coords, font.size, glyph, font.synthesis))
    }

    fn color_kind(&self, font: FontId) -> GlyphColorKind {
        self.faces.get(font).map_or(GlyphColorKind::Outline, |f| f.color)
    }
}

/// Its collection.
impl fmt::Debug for HarfrustBackend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HarfrustBackend({:?})", self.collection)
    }
}
