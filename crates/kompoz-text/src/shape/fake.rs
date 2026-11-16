//! A backend that measures as the C++ Kompoz's FakeTextMeasurer did (`testing.hpp`): what tests lay text out with
//! when their numbers must not depend on fonts.

use super::{
    FontBackend, FontInstance, FontMetrics, FontRun, FontRunRequest, ShapeRequest, ShapedGlyph, ShapedRun,
    letter_spacing_sides,
};
use crate::draw::{Glyph, GlyphColorKind, GlyphRun, GlyphSynthesis, TextDraw, TextRunStyle};
use crate::font::{
    FontFamily, FontFamilyResolver, FontId, FontLoadError, FontMatcher, FontRegistry, FontStyle, FontSynthesis,
    FontVariationSettings, FontWeight, GenericFontFamily, Typeface, fixed_state,
};
use crate::style::TextMotion;
use kompoz_graphics::{Offset, Rect, Size};
use kompoz_runtime::State;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

static FAKE_FONT: LazyLock<FontId> = LazyLock::new(|| FontRegistry::register(Arc::from(Vec::<u8>::new()), 0));

/// C++'s FakeTextMeasurer as a backend: every code point one glyph (its id the code point) half an em wide,
/// stretched by the run's `scale_x`, plus the letter spacing (before and after the glyph as minikin spaces
/// clusters: [`letter_spacing_sides`]); an ascent of 0.8 em, a descent of 0.2 and a leading of 0.25 (lines 1.25 em
/// apart); every glyph a box from the ascent to the descent (so its top and bottom are its ascent and descent); the
/// underline 2 px below the baseline and the strikeout 0.3 ascents above it, `max(1, size / 14)` thick (as C++ drew
/// them). One face has every character, weight and style (its id [`FakeBackend::font_id`]);
/// sizes come in px, so the font scale reaches it through them.
#[derive(Clone, Copy, Debug, Default)]
pub struct FakeBackend;

impl FakeBackend {
    /// The one face.
    pub fn font_id() -> FontId {
        *FAKE_FONT
    }

    /// The face at `size` px.
    pub fn instance(size: f32) -> FontInstance {
        FontInstance { font: FakeBackend::font_id(), coords: Rc::from([]), size, synthesis: GlyphSynthesis::None }
    }

    /// How wide `text` is at `size` px (C++'s measureWidth), as still text (TextMotion.Static) is spaced.
    pub fn measure_width(text: &str, size: f32, scale_x: f32, letter_spacing: f32) -> f32 {
        let (before, after) = letter_spacing_sides(letter_spacing, scale_x, false);
        text.chars().count() as f32 * (size * 0.5 * scale_x + before + after)
    }

    /// One line of `text` drawn in `style` from `baseline`, shaped as one run (left to right unless `rtl`, its
    /// glyphs in visual order either way): what a test's Text draws.
    pub fn text_draw(&self, text: &str, baseline: Offset, style: Rc<TextRunStyle>, rtl: bool) -> TextDraw {
        let font = FakeBackend::instance(style.font_size);
        let shaped = self.shape(&ShapeRequest {
            text,
            range: 0..text.len(),
            font: &font,
            rtl,
            script: None,
            language: None,
            features: &[],
            letter_spacing: style.letter_spacing,
            scale_x: style.scale_x,
            linear: false,
        });
        let metrics = self.metrics(&font);
        let mut pen = 0.0;
        let mut ink: Option<Rect> = None;
        let mut glyphs = Vec::with_capacity(shaped.glyphs.len());
        for g in &shaped.glyphs {
            let x = pen + g.x_offset;
            let b = self.glyph_bounds(&font, g.id);
            let placed = Rect::new(x + b.left * style.scale_x, b.top, x + b.right * style.scale_x, b.bottom);
            ink = Some(ink.map_or(placed, |i| i.union(placed)));
            glyphs.push(Glyph { id: g.id, x, y: -g.y_offset, cluster: g.cluster as u32 });
            pen += g.x_advance;
        }
        let run = GlyphRun {
            font: font.font,
            coords: font.coords.clone(),
            size: font.size,
            synthesis: font.synthesis,
            scale_x: style.scale_x,
            skew_x: style.skew_x,
            color: GlyphColorKind::Outline,
            text_motion: TextMotion::Static,
            glyphs,
        };
        let height = style.line_height.unwrap_or(metrics.line_height());
        TextDraw::new(
            text,
            baseline,
            Size::new(shaped.advance, height),
            ink.unwrap_or(Rect::new(0.0, -metrics.ascent, 0.0, metrics.descent)),
            rtl,
            style,
            Rc::from([run]),
        )
    }
}

impl FontBackend for FakeBackend {
    fn font_runs(&self, request: &FontRunRequest<'_>) -> Vec<FontRun> {
        let font = FontInstance {
            font: FakeBackend::font_id(),
            coords: Rc::from([]),
            size: request.size,
            synthesis: request.typeface.synthesis,
        };
        vec![FontRun { range: request.range.clone(), font }]
    }

    fn shape(&self, request: &ShapeRequest<'_>) -> ShapedRun {
        let size = request.font.size;
        let (before, after) = letter_spacing_sides(request.letter_spacing, request.scale_x, request.linear);
        let advance = size * 0.5 * request.scale_x + before + after;
        let mut glyphs: Vec<ShapedGlyph> = request.text[request.range.clone()]
            .char_indices()
            .map(|(i, c)| ShapedGlyph {
                id: c as u32,
                cluster: request.range.start + i,
                x_advance: advance,
                y_advance: 0.0,
                x_offset: before,
                y_offset: 0.0,
                unsafe_to_break: false,
            })
            .collect();
        if request.rtl {
            glyphs.reverse();
        }
        let advance = glyphs.iter().map(|g| g.x_advance).sum();
        ShapedRun { glyphs, advance }
    }

    fn metrics(&self, font: &FontInstance) -> FontMetrics {
        let s = font.size;
        let thickness = (s / 14.0).max(1.0);
        FontMetrics {
            ascent: 0.8 * s,
            descent: 0.2 * s,
            leading: 0.25 * s,
            top: 0.8 * s,
            bottom: 0.2 * s,
            underline_position: 2.0,
            underline_thickness: thickness,
            strikeout_position: -0.3 * 0.8 * s,
            strikeout_thickness: thickness,
            x_height: 0.8 * s,
            cap_height: 0.8 * s,
        }
    }

    fn glyph_bounds(&self, font: &FontInstance, _glyph: u32) -> Rect {
        let s = font.size;
        Rect::new(0.0, -0.8 * s, 0.5 * s, 0.2 * s)
    }

    fn color_kind(&self, _font: FontId) -> GlyphColorKind {
        GlyphColorKind::Outline
    }
}

/// A resolver over [`FakeBackend`]: the system's families resolve to the one face as asked (nothing faked); a
/// family of one's own to the font CSS matching picks of it ([`FontMatcher`]), faking what the face lacks (as
/// [`FontSynthesis`] rules); a loaded typeface to itself. What a host and the test rule use until a real backend is
/// installed.
#[derive(Clone, Default)]
pub struct FakeFontFamilyResolver {
    backend: Rc<FakeBackend>,
}

impl FakeFontFamilyResolver {
    pub fn new() -> FakeFontFamilyResolver {
        FakeFontFamilyResolver::default()
    }

    /// The face a family, weight, style and synthesis take.
    pub fn typeface(
        family: Option<&FontFamily>,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> Typeface {
        let system = |fallback: GenericFontFamily| Typeface {
            font: FakeBackend::font_id(),
            variation_settings: FontVariationSettings::Empty,
            synthesis: GlyphSynthesis::None,
            weight,
            style,
            fallback,
        };
        match family {
            None => system(GenericFontFamily::Default),
            Some(FontFamily::Loaded(t)) => t.clone(),
            Some(FontFamily::List(list)) => {
                let font = FontMatcher.match_family(list, weight, style).into_iter().next();
                let font = font.expect("kompoz: a family of one's own always matches one of its fonts");
                Typeface {
                    font: FakeBackend::font_id(),
                    variation_settings: font.variation_settings.clone(),
                    synthesis: GlyphSynthesis {
                        embolden: synthesis.synthesizes_weight(weight, font.weight),
                        skew: if synthesis.synthesizes_style(style, font.style) {
                            GlyphSynthesis::ITALIC_SKEW
                        } else {
                            0.0
                        },
                    },
                    weight,
                    style,
                    fallback: GenericFontFamily::Default,
                }
            }
            Some(generic) => system(generic.as_generic().unwrap_or_default()),
        }
    }
}

impl FontFamilyResolver for FakeFontFamilyResolver {
    fn preload(&self, _family: &FontFamily) -> Pin<Box<dyn Future<Output = Result<(), FontLoadError>>>> {
        Box::pin(async { Ok(()) })
    }

    fn resolve(
        &self,
        family: Option<&FontFamily>,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> State<Typeface> {
        fixed_state(FakeFontFamilyResolver::typeface(family, weight, style, synthesis))
    }

    fn backend(&self) -> Rc<dyn FontBackend> {
        self.backend.clone()
    }
}
