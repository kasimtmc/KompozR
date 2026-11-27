//! Which font each grapheme cluster of a text is shaped in (font itemisation): the typeface's own face where it
//! has the cluster, else a fallback face from the collection — the C++ Kompoz's `faceFor` rules (minikin's) over
//! fontique's families. A cluster is never split between fonts, so an emoji sequence (a ZWJ family, a flag, a
//! keycap, a skin tone, a tag sequence) and a letter with its marks stay in one.
//!
//! For each cluster, its first character decides:
//! - Emoji presentation (a sequence, U+FE0F, or an Emoji_Presentation character, unless U+FE0E asks for text)
//!   wants a color font: the emoji families first, then the other candidates; a font without color only if none
//!   with color has it (and the typeface's own then, if it has it).
//! - Text presentation wants an outline font: the typeface's own if it has the character; else the first
//!   candidate without color that has it, else one with color.
//! - A character neutral as to direction or a European digit (spaces, punctuation, digits, marks) stays in the
//!   fallback font the cluster before it took, if that has it (Arabic words keep their font's spaces), unless
//!   that was a color font.
//! - Control characters stay in the typeface's own face.
//!
//! - What no font has, the platform draws where it has a text engine of its own (the web's browser: a [platform
//!   font](super::platform), in colour for emoji); else the typeface's own face (its missing glyph).
//!
//! The candidates, in order: the fallback families for the cluster's script in each of the text's locales that
//! writes it its own way (Persian's Arabic, Japanese Han), then in none (fontconfig's choice for the system's
//! fonts), the typeface's generic family (all its fonts, the
//! system's sorted by coverage), the emoji families, then every family registered with the collection. A
//! fallback face takes the weight and style asked for (its axes set for them if it has some) and is faked as
//! minikin fakes: emboldened when 600 or more is asked of a face 200 or more lighter, slanted when italics are
//! asked of an upright face.

use super::collection::{CollectionFace, FontCollection, FontFamilyId};
use super::family::GenericFontFamily;
use super::resolver::Typeface;
use super::synthesis::FontStyle;
use super::weight::FontWeight;
use crate::draw::GlyphSynthesis;
use crate::intl::LocaleList;
use crate::segment::{self, Presentation};
use crate::shape::face::{Face, Faces};
use crate::shape::{FontInstance, FontRun, FontRunRequest};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, PartialEq, Eq, Hash)]
struct ChoiceKey {
    primary: crate::font::FontId,
    weight: FontWeight,
    style: FontStyle,
    generic: GenericFontFamily,
    locales: LocaleList,
    first: char,
    presentation: Presentation,
}

/// Picks the faces clusters are shaped in, remembering what it picked for a character in a typeface.
#[derive(Default)]
pub(crate) struct FontFallback {
    choices: RefCell<HashMap<ChoiceKey, Option<CollectionFace>>>,
    faces: RefCell<HashMap<(FontFamilyId, FontWeight, FontStyle), Option<CollectionFace>>>,
}

impl FontFallback {
    /// The runs of `request.text[request.range]` in one face each, in order.
    pub(crate) fn font_runs(
        &self,
        collection: &FontCollection,
        faces: &Faces,
        request: &FontRunRequest<'_>,
    ) -> Vec<FontRun> {
        let typeface = request.typeface;
        let primary_face = faces.get(typeface.font);
        let settings = typeface.variation_settings.values(Some(request.density));
        let primary = FontInstance {
            font: typeface.font,
            coords: primary_face.as_ref().map_or_else(|| Rc::from([]), |f| f.coords(&settings)),
            size: request.size,
            synthesis: typeface.synthesis,
        };
        let text = &request.text[request.range.clone()];
        let mut runs: Vec<FontRun> = Vec::new();
        // The fallback face the cluster before took (not the typeface's own, not a color one).
        let mut previous: Option<(FontInstance, Rc<Face>)> = None;
        for cluster in segment::graphemes(text) {
            let range = request.range.start + cluster.start..request.range.start + cluster.end;
            let cluster = &text[cluster];
            let (instance, face) =
                self.choose(collection, faces, request, &primary, primary_face.as_ref(), cluster, &previous);
            previous = match face {
                Some(face) if instance.font != primary.font && !face.is_color() => Some((instance.clone(), face)),
                _ => None,
            };
            match runs.last_mut() {
                Some(last) if last.font == instance && last.range.end == range.start => last.range.end = range.end,
                _ => runs.push(FontRun { range, font: instance }),
            }
        }
        if runs.is_empty() {
            runs.push(FontRun { range: request.range.clone(), font: primary });
        }
        runs
    }

    #[allow(clippy::too_many_arguments)]
    fn choose(
        &self,
        collection: &FontCollection,
        faces: &Faces,
        request: &FontRunRequest<'_>,
        primary: &FontInstance,
        primary_face: Option<&Rc<Face>>,
        cluster: &str,
        previous: &Option<(FontInstance, Rc<Face>)>,
    ) -> (FontInstance, Option<Rc<Face>>) {
        let typeface = request.typeface;
        let Some(first) = cluster.chars().next() else {
            return (primary.clone(), primary_face.cloned());
        };
        let presentation = segment::presentation(cluster);
        let emoji = presentation == Presentation::Emoji;
        if let Some((instance, face)) = previous
            && !emoji
            && segment::is_direction_neutral(first)
            && face.has_char(first)
        {
            return (instance.clone(), Some(face.clone()));
        }
        let has = primary_face.is_some_and(|f| f.has_char(first));
        let primary_color = primary_face.is_some_and(|f| f.is_color());
        if first < ' ' || (has && !emoji) || (has && emoji && primary_color) {
            return (primary.clone(), primary_face.cloned());
        }
        let key = ChoiceKey {
            primary: typeface.font,
            weight: typeface.weight,
            style: typeface.style,
            generic: typeface.fallback,
            locales: request.locales.clone(),
            first,
            presentation,
        };
        let cached = self.choices.borrow().get(&key).cloned();
        let choice = match cached {
            Some(c) => c,
            None => {
                let c = self.search(collection, faces, typeface, request.locales, cluster, first, emoji, has);
                self.choices.borrow_mut().insert(key, c.clone());
                c
            }
        };
        match choice {
            Some(face) => {
                let read = faces.get(face.font);
                let coords = read.as_ref().map_or_else(|| Rc::from([]), |f| f.coords(&face.variations));
                let synthesis = GlyphSynthesis {
                    embolden: typeface.weight.weight >= 600
                        && typeface.weight.weight as f32 - face.effective_weight() >= 200.0
                        && read.as_ref().is_some_and(|f| !f.is_color()),
                    skew: if typeface.style == FontStyle::Italic && !face.effective_italic() {
                        GlyphSynthesis::ITALIC_SKEW
                    } else {
                        0.0
                    },
                };
                (FontInstance { font: face.font, coords, size: request.size, synthesis }, read)
            }
            // No font has it: the platform's text engine's, where it has one.
            None if !has && super::platform::platform_glyphs().is_some() => {
                let style = super::platform::PlatformStyle {
                    weight: typeface.weight.weight.clamp(1, 1000) as u16,
                    italic: typeface.style == FontStyle::Italic,
                    color: emoji,
                };
                let font = super::platform::platform_font(style);
                (FontInstance { font, coords: Rc::from([]), size: request.size, synthesis: GlyphSynthesis::None }, None)
            }
            None => (primary.clone(), primary_face.cloned()),
        }
    }

    /// The fallback face for a cluster, None for the typeface's own.
    #[allow(clippy::too_many_arguments)]
    fn search(
        &self,
        collection: &FontCollection,
        faces: &Faces,
        typeface: &Typeface,
        locales: &LocaleList,
        cluster: &str,
        first: char,
        emoji: bool,
        primary_has: bool,
    ) -> Option<CollectionFace> {
        let script = segment::script_of(cluster);
        let mut families: Vec<FontFamilyId> = Vec::new();
        let mut add = |list: Vec<FontFamilyId>| {
            for f in list {
                if !families.contains(&f) {
                    families.push(f);
                }
            }
        };
        if emoji {
            add(collection.emoji_families());
        }
        for locale in locales.iter() {
            add(collection.fallback_families(script, Some(locale)));
        }
        add(collection.fallback_families(script, None));
        add(collection.generic_families(typeface.fallback));
        // (Characters of every script — digits, punctuation, symbols — where the system says no family of their own
        // for them and its generic family has none of its fonts (wine's prefix without Arial, DirectWrite naming no
        // family for a sample of no script): the system's Latin family's, which has them.)
        if script == *b"Zyyy" || script == *b"Zinh" {
            add(collection.fallback_families(*b"Latn", None));
        }
        add(collection.emoji_families());
        add(collection.registered_families());
        let selector = cluster
            .chars()
            .find(|&c| c == segment::EMOJI_PRESENTATION_SELECTOR || c == segment::TEXT_PRESENTATION_SELECTOR);
        let mut other: Option<CollectionFace> = None;
        for family in families {
            let Some(face) = self.face(collection, family, typeface.weight, typeface.style) else {
                continue;
            };
            if face.font == typeface.font {
                continue;
            }
            let Some(read) = faces.get(face.font) else {
                continue;
            };
            let covers = match selector {
                Some(s) => read.has_variant(first, s),
                None => read.has_char(first),
            };
            if !covers {
                continue;
            }
            if read.is_color() == emoji {
                return Some(face);
            }
            if other.is_none() {
                other = Some(face);
            }
        }
        // Emoji no color font has: the typeface's own if it has them, else any font that has.
        if emoji && primary_has {
            return None;
        }
        other
    }

    fn face(
        &self,
        collection: &FontCollection,
        family: FontFamilyId,
        weight: FontWeight,
        style: FontStyle,
    ) -> Option<CollectionFace> {
        let key = (family, weight, style);
        if let Some(f) = self.faces.borrow().get(&key) {
            return f.clone();
        }
        let face = collection.face(family, weight, style);
        self.faces.borrow_mut().insert(key, face.clone());
        face
    }
}
