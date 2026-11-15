//! The glyph cache (the C++ Kompoz's FontSystem::layout and its glyph maps): a [`TextDraw`]'s glyphs as quads on
//! the atlas, each glyph rasterized once per face, size and style, put on the atlas and drawn from there until the
//! page it is on is cleared (then rasterized again when drawn).
//!
//! A glyph run's font is read back from the [`FontRegistry`] by its [`FontId`] and set to the run's variation
//! coordinates; its glyphs are drawn at a whole number of pixels (`round(size · scale)`, as C++'s FreeType size), at
//! their positions rounded to whole pixels (no subpixel positioning: C++'s `round(x + glyph.x)`) and light hinted for
//! still text, at a quarter of a pixel and unhinted for text that moves (the run's TextMotion), outline glyphs in the
//! text's style, colour glyphs (COLR, bitmaps) as their fonts colour them.

use super::atlas::{AtlasSlot, GlyphAtlas, PageFormat};
use super::autohint::{Autohinter, Target};
use super::bitmap::{BitmapImage, bitmap_glyph};
use super::blur::blurred;
use super::color::{ColorImage, paint_colr};
use super::outline::{GlyphStroke, GlyphStyle, Hinting, glyf_tags, hint_engine, load, scaler, styled_coverage};
use super::raster::{Coverage, FixedOutline};
use kompoz_graphics::{DrawStyle, Offset, PathEffect, StrokeCap, StrokeJoin};
use kompoz_text::draw::{GlyphColorKind, GlyphRun, TextDraw};
use kompoz_text::font::{FontBytes, FontId, FontRegistry};
use kompoz_text::style::Linearity;
use skrifa::font::glyph::outline::OutlineSource;
use skrifa::raw::types::F2Dot14;
use skrifa::{Font, FontRef, HintEngine, ScalerFont};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Where a glyph is drawn and where it is on the atlas: its quad (px, in the space of the request's origin times
/// its scale), its page and texels there; `color`: drawn as it is (an emoji, on a colour page), else on a coverage
/// page and tinted by the text's paint.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GlyphQuad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub page: usize,
    pub u: u32,
    pub v: u32,
    pub width: u32,
    pub height: u32,
    pub color: bool,
}

impl GlyphQuad {
    /// Its texels on the page as texture coordinates, [u0, v0, u1, v1] (0..1).
    pub fn uv(&self) -> [f32; 4] {
        let page = super::atlas::AtlasPage::SIZE as f32;
        [
            self.u as f32 / page,
            self.v as f32 / page,
            (self.u + self.width) as f32 / page,
            (self.v + self.height) as f32 / page,
        ]
    }
}

/// How a [`TextDraw`]'s glyphs are asked for (C++'s TextSource::layout arguments): where its baseline origin is
/// drawn (px of the target; the text's glyphs are placed from there); how many times bigger than that it is laid
/// out (C++'s textScale under an enlarging transform: sizes, positions, stroke widths and blur are that much bigger,
/// and the quads are in that bigger space); a text shadow (only the glyphs that take a colour, blurred by this
/// radius, px where drawn); the colour of a COLR glyph's foreground (palette index 0xFFFF, straight sRGB: the text's
/// colour where it has one; FreeType's opaque black, which C++ drew with, otherwise).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GlyphRequest {
    pub origin: Offset,
    pub scale: f32,
    pub shadow: Option<f32>,
    pub foreground: [u8; 4],
}

impl GlyphRequest {
    /// The text's glyphs as they are, from `origin`.
    pub fn new(origin: Offset) -> GlyphRequest {
        GlyphRequest { origin, scale: 1.0, shadow: None, foreground: [0, 0, 0, 255] }
    }
}

/// A glyph rasterized: coverage (white, tinted by the text's paint) or colour (straight RGBA, drawn as it is).
#[derive(Clone, PartialEq, Debug)]
pub enum GlyphImage {
    Coverage(Coverage),
    Color(ColorImage),
}

/// A font at a location in its design space, with skrifa's scalers by size and hinting (a scaler keeps what its
/// hinting works out for the size: TrueType's programs run once), and the automatic hinter's state for it (its
/// glyphs' styles and their metrics: made when first needed).
pub struct Face {
    blob: FontBytes,
    index: u32,
    font: Font,
    scalers: HashMap<(u32, Hinting), ScalerFont>,
    units: Option<ScalerFont>,
    autohinter: Option<Autohinter>,
}

impl Face {
    /// Face `index` of `blob` at variation coordinates `coords` (normalized, F2Dot14 bits); None if it isn't a font.
    pub fn new(blob: FontBytes, index: u32, coords: &[i16]) -> Option<Face> {
        let font = Font::new(blob.clone(), index)?;
        let font = if coords.is_empty() {
            font
        } else {
            font.instance_builder().normalized_coords(coords.iter().map(|&c| F2Dot14::from_bits(c))).build()
        };
        Some(Face { blob, index, font, scalers: HashMap::new(), units: None, autohinter: None })
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Which hinter fits its outlines with `hinting` ([`HintEngine::Auto`]: our automatic hinter).
    pub fn hint_engine(&self, hinting: Hinting) -> Option<HintEngine> {
        hint_engine(&self.font, hinting)
    }

    /// Its skrifa scaler at `px` px with `hinting`.
    pub fn scaler(&mut self, px: u32, hinting: Hinting) -> &ScalerFont {
        let font = &self.font;
        self.scalers.entry((px, hinting)).or_insert_with(|| scaler(font, px, hinting))
    }

    /// Glyph `glyph`'s outline at `px` px with `hinting`, as FreeType hints it: by our automatic hinter (FreeType
    /// 2.14's, see [`autohint`](super::autohint)) where FreeType hints with it, else by skrifa's scaler (the font's
    /// own instructions or hints, or none).
    pub fn outline(&mut self, px: u32, hinting: Hinting, glyph: u32) -> Option<FixedOutline> {
        if self.hint_engine(hinting) == Some(HintEngine::Auto) {
            let target = if hinting == Hinting::Light { Target::Light } else { Target::Normal };
            let font = &self.font;
            let hinter = self.autohinter.get_or_insert_with(|| Autohinter::new(font));
            return hinter.hint(font, glyph, px, target).map(|hinted| hinted.outline);
        }
        let blob = self.blob.clone();
        let font_ref = FontRef::from_index((*blob).as_ref(), self.index).ok();
        let scaler = self.scaler(px, hinting);
        let tags = match (scaler.outline_source(), &font_ref) {
            (Some(OutlineSource::Glyf), Some(f)) => glyf_tags(f, glyph),
            _ => None,
        };
        load(scaler, glyph, tags)
    }

    /// Glyph `glyph` rasterized as `spec` asks: an outline glyph (`kind` Outline) at its size, light hinted or not,
    /// moved its subpixel offset across, in its style, blurred for a shadow; a colour glyph as its font colours it
    /// (palette index 0xFFFF as the foreground), on whole pixels, never blurred: a shadow with a blur has none of a
    /// colour font's glyphs, as C++'s had none. None when it draws nothing. Also whether it used the foreground.
    pub fn rasterize(&mut self, kind: GlyphColorKind, glyph: u32, spec: &GlyphSpec) -> (Option<GlyphImage>, bool) {
        let coverage = |c: Coverage| (!c.is_empty()).then_some(GlyphImage::Coverage(c));
        let px = spec.px;
        let blurred_shadow = spec.blur.is_some_and(|b| b > 0);
        match kind {
            GlyphColorKind::Outline => {
                let hinting = if spec.hinted { Hinting::Light } else { Hinting::None };
                let Some(outline) = self.outline(px, hinting, glyph) else {
                    return (None, false);
                };
                let c = styled_coverage(outline, px, &spec.style, spec.scale, spec.subpixel as i32 * 16);
                let c = match spec.blur {
                    Some(radius) if radius > 0 => blurred(&c, radius),
                    _ => c,
                };
                (coverage(c), false)
            }
            _ if blurred_shadow => (None, false),
            GlyphColorKind::ColrV0 | GlyphColorKind::ColrV1 => {
                // (Its scaler drawing in font units, kept with it, taken out while the layers borrow the face.)
                let font = self.font.clone();
                let units = self.units.take().unwrap_or_else(|| ScalerFont::unhinted(font.clone(), None));
                let mut layers = |layer: u32| self.outline(px, Hinting::Normal, layer);
                let painted = paint_colr(&font, glyph, px, spec.foreground, &mut layers, &units);
                self.units = Some(units);
                match painted {
                    Some(painted) => (painted.image.map(GlyphImage::Color), painted.uses_foreground),
                    None => (self.plain(glyph, px), false),
                }
            }
            GlyphColorKind::Bitmap => match bitmap_glyph(&self.font, glyph, px) {
                Some(BitmapImage::Color(image)) => ((image.width > 0).then_some(GlyphImage::Color(image)), false),
                Some(BitmapImage::Coverage(c)) => (coverage(c), false),
                None => (self.plain(glyph, px), false),
            },
        }
    }

    /// A colour font's glyph without colours of its own: its outline as FreeType's FT_LOAD_COLOR drew it for C++
    /// (default hinting, no style), as coverage.
    fn plain(&mut self, glyph: u32, px: u32) -> Option<GlyphImage> {
        let outline = self.outline(px, Hinting::Normal, glyph)?;
        let c = outline.render();
        (!c.is_empty()).then_some(GlyphImage::Coverage(c))
    }
}

/// How a glyph is rasterized besides its face and id: its size (whole px), whether it is hinted (text that moves is
/// drawn unhinted: Compose's TextMotion), where its origin falls within a pixel across (in quarters, 0..3: subpixel
/// positioning), its style, how many times bigger than drawn text is laid out, a shadow's blur (px), the foreground
/// colour of a COLR glyph.
#[derive(Clone, PartialEq, Debug)]
pub struct GlyphSpec {
    pub px: u32,
    pub hinted: bool,
    pub subpixel: u8,
    pub style: GlyphStyle,
    pub scale: f32,
    pub blur: Option<u32>,
    pub foreground: [u8; 4],
}

impl GlyphSpec {
    /// A glyph at `px` px, hinted, on a whole pixel, as its font draws it.
    pub fn new(px: u32) -> GlyphSpec {
        GlyphSpec {
            px,
            hinted: true,
            subpixel: 0,
            style: GlyphStyle::PLAIN,
            scale: 1.0,
            blur: None,
            foreground: [0, 0, 0, 255],
        }
    }
}

/// A path effect in a key: equal as path effects are, hashed only by whether there is one.
#[derive(Clone, Debug, Default)]
pub struct EffectKey(pub Option<PathEffect>);

impl PartialEq for EffectKey {
    fn eq(&self, o: &EffectKey) -> bool {
        self.0 == o.0
    }
}

impl Eq for EffectKey {}

impl Hash for EffectKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.is_some().hash(state);
    }
}

/// A float in a key: its bits (-0 as 0).
fn bits(v: f32) -> u32 {
    (v + 0.0).to_bits()
}

/// What a glyph is cached by: its face (an index standing for its font and variation coordinates), id, size (whole
/// px), hinting and subpixel offset, style (stretch and slant, synthesis, stroke: width, cap, join, miter, path
/// effect, and for a path effect the scale text is laid out at), a shadow's blur, and the foreground colour for a
/// COLR glyph that uses it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct GlyphKey {
    pub face: usize,
    pub glyph: u32,
    pub px: u32,
    pub hinted: bool,
    pub subpixel: u8,
    pub scale_x: u32,
    pub skew_x: u32,
    pub embolden: bool,
    pub skew: u32,
    pub stroke: Option<(u32, StrokeCap, StrokeJoin, u32, EffectKey)>,
    pub effect_scale: u32,
    pub blur: Option<u32>,
    pub foreground: Option<[u8; 4]>,
}

impl GlyphKey {
    /// The key of glyph `glyph` of `face` rasterized as `spec` asks (its foreground left out: see
    /// [`GlyphCache`]).
    pub fn new(face: usize, glyph: u32, spec: &GlyphSpec) -> GlyphKey {
        let style = &spec.style;
        let stroke =
            style.stroke.as_ref().map(|s| (s.width, s.cap, s.join, bits(s.miter), EffectKey(s.path_effect.clone())));
        let effect_scale = match &style.stroke {
            Some(s) if s.path_effect.is_some() => bits(spec.scale),
            _ => 0,
        };
        GlyphKey {
            face,
            glyph,
            px: spec.px,
            hinted: spec.hinted,
            subpixel: spec.subpixel,
            scale_x: bits(style.scale_x),
            skew_x: bits(style.skew_x),
            embolden: style.synthesis.embolden,
            skew: bits(style.synthesis.skew),
            stroke,
            effect_scale,
            blur: spec.blur,
            foreground: None,
        }
    }
}

/// A cached glyph: where it is drawn from its origin (left, top: y up) and its size, its atlas spot and the page's
/// generation when it was put there (none: nothing to draw), whether it is colour, whether it used the foreground.
#[derive(Clone, Copy, PartialEq, Debug)]
struct CachedGlyph {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    slot: Option<(AtlasSlot, u64)>,
    color: bool,
    uses_foreground: bool,
}

/// The fonts read, the glyphs rasterized, the atlas they are on. A COLR glyph that paints with the foreground colour
/// is cached by that colour too: its plain key holds a marker saying so.
pub struct GlyphCache {
    atlas: GlyphAtlas,
    faces: Vec<Option<Face>>,
    face_index: HashMap<FontId, HashMap<Box<[i16]>, usize>>,
    glyphs: HashMap<GlyphKey, CachedGlyph>,
    rasterized: usize,
}

impl Default for GlyphCache {
    fn default() -> GlyphCache {
        GlyphCache::new()
    }
}

impl GlyphCache {
    pub fn new() -> GlyphCache {
        GlyphCache {
            atlas: GlyphAtlas::new(),
            faces: Vec::new(),
            face_index: HashMap::new(),
            glyphs: HashMap::new(),
            rasterized: 0,
        }
    }

    /// The atlas the glyphs are on (whose pages' dirty rectangles a renderer uploads before drawing).
    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
    }

    pub fn atlas_mut(&mut self) -> &mut GlyphAtlas {
        &mut self.atlas
    }

    /// How many glyphs were rasterized so far (C++'s KOMPOZ_PROFILE count).
    pub fn rasterized(&self) -> usize {
        self.rasterized
    }

    /// Between frames: the atlas's pages unused longest cleared for reuse (see [`GlyphAtlas::end_frame`]), the
    /// glyphs that were on them forgotten.
    pub fn end_frame(&mut self) {
        let cleared = self.atlas.end_frame();
        if !cleared.is_empty() {
            self.glyphs.retain(|_, g| g.slot.is_none_or(|(slot, _)| !cleared.contains(&slot.page)));
        }
    }

    /// The index of font `font` at `coords` (normalized, F2Dot14 bits), read now if it wasn't (None for a font that
    /// isn't registered or can't be read).
    pub fn face(&mut self, font: FontId, coords: &[i16]) -> Option<usize> {
        if let Some(&i) = self.face_index.get(&font).and_then(|m| m.get(coords)) {
            return self.faces[i].as_ref().map(|_| i);
        }
        let face = FontRegistry::get(font).and_then(|data| Face::new(data.blob, data.index, coords));
        let i = self.faces.len();
        self.faces.push(face);
        self.face_index.entry(font).or_default().insert(coords.into(), i);
        self.faces[i].as_ref().map(|_| i)
    }

    /// Face `i` (as [`GlyphCache::face`] gave it).
    pub fn face_mut(&mut self, i: usize) -> Option<&mut Face> {
        self.faces.get_mut(i).and_then(|f| f.as_mut())
    }

    /// `text`'s glyphs as `request` asks for them, appended to `out` in its runs' order (glyphs that draw nothing
    /// left out). Each glyph's page is marked used this frame.
    ///
    /// Each run is drawn at `round(size · scale)` px; its glyphs' origins at `origin · scale + glyph · scale` (laid
    /// out bigger or smaller than drawn, `glyph · round(size · scale) / size`: at the size it is rasterized at, as
    /// C++ shaped it again there), rounded to whole pixels (C++'s, for text that is still: Compose's TextMotion.Static), or to a quarter of a
    /// pixel across with the glyph rasterized that far into its pixel and unhinted (TextMotion.Animated: subpixel
    /// positions, linear); colour glyphs on whole pixels either way. A stroke's width is whole pixels
    /// (`round(width · scale)`, at least one), a shadow's blur too (`round(radius · scale)`).
    pub fn layout(&mut self, text: &TextDraw, request: &GlyphRequest, out: &mut Vec<GlyphQuad>) {
        let k = if request.scale > 0.0 && request.scale.is_finite() { request.scale } else { 1.0 };
        let stroke = match &text.style.draw_style {
            DrawStyle::Stroke(s) => Some(GlyphStroke {
                width: ((s.width * k).round() as i64).max(1) as u32,
                cap: s.cap,
                join: s.join,
                miter: s.miter,
                path_effect: s.path_effect.clone(),
            }),
            DrawStyle::Fill => None,
        };
        // (A shadow without a blur has the glyphs as they are.)
        let blur = request.shadow.map(|radius| (radius * k).round().max(0.0) as u32).filter(|&b| b > 0);
        for run in text.glyphs.iter() {
            self.layout_run(run, stroke.as_ref(), blur, request, k, out);
        }
    }

    fn layout_run(
        &mut self,
        run: &GlyphRun,
        stroke: Option<&GlyphStroke>,
        blur: Option<u32>,
        request: &GlyphRequest,
        k: f32,
        out: &mut Vec<GlyphQuad>,
    ) {
        let Some(face) = self.face(run.font, &run.coords) else {
            return;
        };
        let outline = run.color == GlyphColorKind::Outline;
        let subpixel = outline && run.text_motion.subpixel_text_positioning;
        let mut spec = GlyphSpec {
            px: ((run.size * k).round() as i64).max(1) as u32,
            hinted: run.text_motion.linearity == Linearity::FontHinting,
            subpixel: 0,
            // Colour glyphs are drawn as they are: no style.
            style: if outline {
                GlyphStyle {
                    scale_x: run.scale_x,
                    skew_x: run.skew_x,
                    synthesis: run.synthesis,
                    stroke: stroke.cloned(),
                }
            } else {
                GlyphStyle::PLAIN
            },
            scale: k,
            blur,
            foreground: request.foreground,
        };
        // Text laid out bigger (or smaller) than drawn is laid out at the whole size its glyphs are rasterized at, as
        // C++ shaped it again at that size: its glyphs apart as that size's advances put them (glyph and advance in
        // proportion), not the drawn size's times k. Drawn as it is, as it was laid out.
        let at = if k != 1.0 && run.size > 0.0 { spec.px as f32 / run.size } else { k };
        for g in &run.glyphs {
            let x = request.origin.x * k + g.x * at;
            let (x, offset) = if subpixel {
                let quarters = (x * 4.0).round();
                let whole = (quarters / 4.0).floor();
                (whole, (quarters - whole * 4.0) as u8)
            } else {
                (x.round(), 0)
            };
            spec.subpixel = offset;
            let Some(cached) = self.glyph(face, run.color, g.id, &spec) else {
                continue;
            };
            let Some((slot, _)) = cached.slot else {
                continue;
            };
            if request.shadow.is_some() && cached.color {
                continue;
            }
            self.atlas.use_page(slot.page);
            let x0 = x + cached.left as f32;
            let y0 = (request.origin.y * k + g.y * at).round() - cached.top as f32;
            out.push(GlyphQuad {
                x0,
                y0,
                x1: x0 + cached.width as f32,
                y1: y0 + cached.height as f32,
                page: slot.page,
                u: slot.u,
                v: slot.v,
                width: cached.width,
                height: cached.height,
                color: cached.color,
            });
        }
    }

    /// Glyph `glyph` of face `face` as `spec` asks, from the cache if it is still on the atlas, else rasterized and
    /// put there (None for a face that isn't there).
    fn glyph(&mut self, face: usize, kind: GlyphColorKind, glyph: u32, spec: &GlyphSpec) -> Option<CachedGlyph> {
        let mut key = GlyphKey::new(face, glyph, spec);
        let mut found = self.glyphs.get(&key).copied();
        if found.is_some_and(|g| g.uses_foreground) {
            key.foreground = Some(spec.foreground);
            found = self.glyphs.get(&key).copied();
        }
        if let Some(g) = found
            && g.slot.is_none_or(|(slot, generation)| self.atlas.valid(slot.page, generation))
        {
            return Some(g);
        }
        let (image, uses_foreground) = self.faces.get_mut(face)?.as_mut()?.rasterize(kind, glyph, spec);
        self.rasterized += 1;
        let mut cached =
            CachedGlyph { left: 0, top: 0, width: 0, height: 0, slot: None, color: false, uses_foreground: false };
        if let Some(image) = image {
            let (format, left, top, w, h, pixels, color) = match &image {
                GlyphImage::Coverage(c) => (PageFormat::Coverage, c.left, c.top, c.width, c.height, &c.alpha, false),
                GlyphImage::Color(c) => (PageFormat::Rgba, c.left, c.top, c.width, c.height, &c.rgba, true),
            };
            if let Some(slot) = self.atlas.add(format, pixels, w as u32, h as u32) {
                cached = CachedGlyph {
                    left,
                    top,
                    width: w as u32,
                    height: h as u32,
                    slot: Some((slot, self.atlas.generation(slot.page))),
                    color,
                    uses_foreground: false,
                };
            }
        }
        if uses_foreground {
            // The glyph by its foreground; the plain key says to look it up so.
            key.foreground = None;
            self.glyphs.insert(key.clone(), CachedGlyph { uses_foreground: true, ..cached });
            key.foreground = Some(spec.foreground);
        }
        self.glyphs.insert(key, cached);
        Some(cached)
    }
}
