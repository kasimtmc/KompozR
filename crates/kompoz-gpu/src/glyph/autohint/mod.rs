//! FreeType's automatic hinter (its `autofit` module) as FreeType 2.15 hints, for the glyphs the C++ Kompoz drew with
//! FreeType 2.15.0 (FT_LOAD_TARGET_LIGHT): the font's glyphs given styles by script (the character map, and the
//! layout tables as FreeType with HarfBuzz reads them), each style's standard stem widths and blue zones measured
//! from the font, and an outline's segments and edges found and fitted to the pixel grid.
//!
//! This is skrifa 0.50's port of FreeType's hinter (`skrifa::outline::autohint`, Copyright 2019 the fontations
//! developers, under the MIT or Apache 2.0 license; see the NOTICE file), brought into this crate as it is (its
//! modules, types and functions keep their names), reading fonts through skrifa's and read-fonts' public APIs and
//! handing its outlines over as FreeType holds them ([`FixedOutline`](super::raster::FixedOutline)). Left out:
//! skrifa's ahead-of-time hinting plans (ttfautohint's quirks), its path output and its `FontRef` context.
//!
//! Added from FreeType 2.14 (2.14.3's `src/autofit`), which skrifa's port predates:
//! - the diacritic adjustment database and each style's reverse character map, extended through the script's
//!   single and alternate substitutions as FreeType does with HarfBuzz (`afadjust.c`, `afgsub.c`, see
//!   [`adjust`]);
//! - in the hinter, in FreeType's order (`aflatin.c`'s `af_latin_hints_apply`): a tilde above or below its base
//!   stretched to stay a tilde and aligned to the pixel grid, it and what lies beyond it kept out of the edges;
//!   blue zones ignored above or below the base of a glyph whose diacritic is attached to it (a cedilla, an
//!   ogonek, a hook); and, after the edges are fitted, a diacritic moved up or down until a pixel separates it from
//!   its base;
//! - the blue zones' capital and small bottom properties (`afblue.dat`), Unicode 17's script ranges (`afranges.c`),
//!   and `af_latin_hint_edges`'s check that a serif is a real one.
//!
//! And from FreeType 2.15.0, whose sources weren't at hand: what it does differently was found from the system
//! library (Arch Linux's freetype2 2.15.0, which made the goldens): it calls HarfBuzz's
//! `hb_ot_layout_lookups_substitute_closure`, which 2.14 doesn't, and a 2.14.3 changed to take a default style's
//! coverage by that closure (of the glyphs the style has so far, under the script's lookups; see `shape.rs`,
//! `closure.rs`) gives 2.15.0's style map and glyph bitmaps exactly for every test font; its script ranges differ
//! from 2.14.3's in nine places (`generate_styles.py`), read from its style map of fonts mapping every code point.
//!
//! FreeType is under the FreeType License (or the GPL), HarfBuzz under the "Old MIT" license; this follows their
//! code.

#![deny(unsafe_code)]
// (skrifa's code, kept as it is written there: it indexes its loops)
#![allow(clippy::needless_range_loop)]

pub mod adjust;
mod closure;
mod context;
mod gsub;
mod hint;
mod instance;
mod metrics;
mod outline;
mod shape;
mod style;
mod topo;
mod unscaled;

#[cfg(test)]
mod tests;

pub use instance::{Autohinter, HintedGlyph, Target};
pub use metrics::{
    BlueZones, Scale, ScaleFlags, ScaledAxisMetrics, ScaledBlue, ScaledStyleMetrics, ScaledWidth, UnscaledAxisMetrics,
    UnscaledBlue, UnscaledStyleMetrics, WidthMetrics,
};
pub use outline::Direction;
pub use style::{GlyphStyle, SCRIPT_CLASSES, STYLE_CLASSES, ScriptClass, ScriptGroup, StyleClass};
pub use topo::{Axis, BlueProvenance, Dimension, Edge, Segment, TopoFlags};

/// All constants are defined based on a UPEM of 2048.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L34>
fn derived_constant(units_per_em: i32, value: i32) -> i32 {
    value * units_per_em / 2048
}
