//! Autohinting state for a font instance (skrifa's `instance.rs`): the glyphs' styles and each style's metrics,
//! measured once for a font at a location, and the drawing of a glyph hinted for a target (FreeType's
//! `af_loader_load_glyph`).

use super::adjust::ReverseCharmap;
use super::context::{AutohintContext, FontContext};
use super::metrics::{Scale, ScaleFlags, UnscaledStyleMetrics, compute_unscaled_style_metrics, fixed_mul, pix_round};
use super::outline::Outline;
use super::shape::{Shaper, ShaperMode};
use super::style::{GlyphStyle, GlyphStyleMap};
use crate::glyph::raster::FixedOutline;
use skrifa::Font;
use skrifa::attribute::Style;
use skrifa::raw::types::GlyphId;
use std::sync::{Arc, OnceLock};

/// Layout tables extend style coverage beyond the character map as FreeType does with HarfBuzz (the FreeType the
/// C++ Kompoz's goldens were made with is built with it).
pub(crate) const SHAPER_MODE: ShaperMode = ShaperMode::BestEffort;

/// What an outline is hinted for: FreeType's load targets.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Target {
    /// FT_LOAD_TARGET_MONO: strong hinting, for aliased rasterization.
    Mono,
    /// FT_LOAD_TARGET_NORMAL (FT_LOAD_DEFAULT): anti-aliased rasterization.
    Normal,
    /// FT_LOAD_TARGET_LIGHT: anti-aliased, fitted only vertically.
    Light,
    /// FT_LOAD_TARGET_LCD: horizontal subpixels.
    Lcd,
    /// FT_LOAD_TARGET_LCD_V: vertical subpixels.
    VerticalLcd,
}

impl Target {
    pub(crate) fn is_light(self) -> bool {
        self == Target::Light
    }

    pub(crate) fn is_lcd(self) -> bool {
        self == Target::Lcd
    }

    pub(crate) fn is_vertical_lcd(self) -> bool {
        self == Target::VerticalLcd
    }
}

/// A glyph hinted: its outline as FreeType's glyph slot holds it, and its hinted advance (26.6).
#[derive(Clone, PartialEq, Debug)]
pub struct HintedGlyph {
    pub outline: FixedOutline,
    pub advance: i32,
}

/// The data the automatic hinter derives from a font at one location (FreeType's face globals and their style
/// metrics): the glyphs' styles, each style's metrics (measured when first drawn, with the style's reverse
/// character map), whether the font is monospaced, and its style. It doesn't depend on the size or the target.
pub struct Autohinter {
    styles: GlyphStyleMap,
    metrics: Vec<OnceLock<Arc<StyleState>>>,
    is_fixed_width: bool,
    style: Style,
}

/// A style's unscaled metrics and the reverse character map its diacritic adjustments look glyphs up in.
pub(crate) struct StyleState {
    pub metrics: UnscaledStyleMetrics,
    pub reverse_charmap: ReverseCharmap,
}

impl Autohinter {
    /// The hinter's state for `font` at its location.
    pub fn new(font: &Font) -> Autohinter {
        let context = FontContext::new(font);
        let shaper = Shaper::new(&context, SHAPER_MODE);
        let styles = GlyphStyleMap::new(context.glyph_count(), &shaper);
        let metrics = (0..styles.metrics_count()).map(|_| OnceLock::new()).collect();
        Autohinter { styles, metrics, is_fixed_width: context.is_fixed_pitch(), style: context.style() }
    }

    /// The style the hinter gives `glyph`, if any.
    pub fn glyph_style(&self, glyph: u32) -> Option<GlyphStyle> {
        self.styles.style(GlyphId::new(glyph))
    }

    /// The state of `glyph`'s style, measured from `context` when first asked for.
    fn style_state(&self, context: &dyn AutohintContext, glyph: GlyphId) -> Option<Arc<StyleState>> {
        let style = self.styles.style(glyph)?;
        let index = self.styles.metrics_index(style)?;
        let style_class = style.style_class()?;
        let state = self.metrics.get(index)?.get_or_init(|| {
            let shaper = Shaper::new(context, SHAPER_MODE);
            let metrics = compute_unscaled_style_metrics(&shaper, style_class);
            let reverse_charmap = ReverseCharmap::new(context, style_class);
            Arc::new(StyleState { metrics, reverse_charmap })
        });
        Some(state.clone())
    }

    /// Glyph `glyph` of `font` (the font this state was made for) at `ppem` px, hinted for `target` as FreeType's
    /// automatic hinter hints it: None when the font has no outline for it, or the hinter no style.
    pub fn hint(&self, font: &Font, glyph: u32, ppem: u32, target: Target) -> Option<HintedGlyph> {
        let context = FontContext::new(font);
        let glyph_id = GlyphId::new(glyph);
        let style = self.styles.style(glyph_id)?;
        let state = self.style_state(&context, glyph_id)?;
        let metrics = &state.metrics;
        let mut outline = Outline::default();
        let has_overlaps = context.fill_outline(glyph_id, &mut outline)?;
        let units_per_em = outline.units_per_em;
        let scale = Scale::new(ppem as f32, units_per_em, self.style, target, metrics.style_class().script.group);
        let hinted_metrics =
            super::hint::hint_outline(&mut outline, metrics, &scale, Some(style), glyph, &state.reverse_charmap);
        let h_advance = outline.advance;
        let mut pp1x = 0;
        let mut pp2x = fixed_mul(h_advance, hinted_metrics.x_scale);
        let is_light = target.is_light();
        // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afloader.c#L422>
        if !is_light {
            if let (true, Some(edge_metrics)) =
                (!scale.flags.contains(ScaleFlags::NO_ADVANCE), hinted_metrics.edge_metrics)
            {
                let old_rsb = pp2x - edge_metrics.right_opos;
                let old_lsb = edge_metrics.left_opos;
                let new_lsb = edge_metrics.left_pos;
                let mut pp1x_uh = new_lsb - old_lsb;
                let mut pp2x_uh = edge_metrics.right_pos + old_rsb;
                if old_lsb < 24 {
                    pp1x_uh -= 8;
                }
                if old_rsb < 24 {
                    pp2x_uh += 8;
                }
                pp1x = pix_round(pp1x_uh);
                pp2x = pix_round(pp2x_uh);
                if pp1x >= new_lsb && old_lsb > 0 {
                    pp1x -= 64;
                }
                if pp2x <= edge_metrics.right_pos && old_rsb > 0 {
                    pp2x += 64;
                }
            } else {
                pp1x = pix_round(pp1x);
                pp2x = pix_round(pp2x);
            }
        } else {
            pp1x = pix_round(pp1x);
            pp2x = pix_round(pp2x);
        }
        if pp1x != 0 {
            for point in &mut outline.points {
                point.x -= pp1x;
            }
        }
        let advance = if !is_light && (self.is_fixed_width || (metrics.digits_have_same_width && style.is_digit())) {
            fixed_mul(h_advance, scale.x_scale)
        } else if h_advance != 0 {
            pp2x - pp1x
        } else {
            0
        };
        Some(HintedGlyph { outline: outline.to_fixed(has_overlaps), advance: pix_round(advance) })
    }
}
