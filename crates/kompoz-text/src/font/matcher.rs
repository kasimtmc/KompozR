//! Which of a family's fonts a weight and style take (Compose's FontMatcher: CSS Fonts Level 3's font matching).

use super::family::{FontFamily, FontListFontFamily};
use super::font::Font;
use super::synthesis::FontStyle;
use super::weight::FontWeight;

/// Matches a weight and style to a family's fonts as CSS does (Compose's FontMatcher): the fonts of exactly that
/// weight and style if any; else, of those of the style (all fonts if none is), the nearest weight: below 400 the
/// nearest lighter first, above 500 the nearest heavier first, and 400 to 500 the nearest up to 500 first (400
/// tries 500 before lighter ones, 500 tries 400), then heavier ones.
#[derive(Clone, Copy, Debug, Default)]
pub struct FontMatcher;

impl FontMatcher {
    /// The fonts of `fonts` the weight and style take, best first (all of the weight chosen; empty for no fonts).
    pub fn match_font(&self, fonts: &[Font], weight: FontWeight, style: FontStyle) -> Vec<Font> {
        let exact: Vec<Font> = fonts.iter().filter(|f| f.weight == weight && f.style == style).cloned().collect();
        if !exact.is_empty() {
            return exact;
        }
        let of_style: Vec<Font> = fonts.iter().filter(|f| f.style == style).cloned().collect();
        let search = if of_style.is_empty() { fonts.to_vec() } else { of_style };
        if weight < FontWeight::W400 {
            closest_weight(&search, weight, true, None, None)
        } else if weight > FontWeight::W500 {
            closest_weight(&search, weight, false, None, None)
        } else {
            let up_to_500 = closest_weight(&search, weight, false, None, Some(FontWeight::W500));
            if up_to_500.is_empty() {
                closest_weight(&search, weight, false, Some(FontWeight::W500), None)
            } else {
                up_to_500
            }
        }
    }

    /// The same for a family of one's own.
    pub fn match_family(&self, family: &FontListFontFamily, weight: FontWeight, style: FontStyle) -> Vec<Font> {
        self.match_font(family.fonts(), weight, style)
    }

    /// The same for any family: one not of one's own fonts panics (as Compose's).
    pub fn match_font_family(&self, family: &FontFamily, weight: FontWeight, style: FontStyle) -> Vec<Font> {
        match family {
            FontFamily::List(l) => self.match_family(l, weight, style),
            other => {
                panic!("kompoz: Only FontFamily instances that presents a list of Fonts can be used, not {other:?}")
            }
        }
    }
}

/// The fonts of the weight nearest to `weight` within `min..=max` (Compose's filterByClosestWeight): the nearest
/// lighter or heavier, as `prefer_below` says, else the other.
fn closest_weight(
    fonts: &[Font],
    weight: FontWeight,
    prefer_below: bool,
    min: Option<FontWeight>,
    max: Option<FontWeight>,
) -> Vec<Font> {
    let mut above: Option<FontWeight> = None;
    let mut below: Option<FontWeight> = None;
    for f in fonts {
        let w = f.weight;
        if min.is_some_and(|m| w < m) || max.is_some_and(|m| w > m) {
            continue;
        }
        if w < weight {
            if below.is_none_or(|b| w > b) {
                below = Some(w);
            }
        } else if w > weight {
            if above.is_none_or(|a| w < a) {
                above = Some(w);
            }
        } else {
            above = Some(w);
            below = Some(w);
            break;
        }
    }
    let best = if prefer_below { below.or(above) } else { above.or(below) };
    fonts.iter().filter(|f| Some(f.weight) == best).cloned().collect()
}
