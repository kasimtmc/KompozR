//! Fonts: weights matched as CSS does and synthesis as Compose and minikin decide it (the C++ Kompoz's
//! font_weights_match_the_nearest_file_as_css_does, its Material part aside), variable fonts' settings and families
//! told apart by them (font_variations_pick_a_variable_fonts_instance), families equal by what they are, OpenType
//! feature settings read as HarfBuzz reads them, the font registry, the resolver over the fake backend.

use kompoz_graphics::*;
use kompoz_runtime::kompozable_block;
use kompoz_text::font::{FontData, FontMatcher, parse_feature};
use kompoz_text::*;
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

fn panics(f: impl FnOnce()) -> bool {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = catch_unwind(AssertUnwindSafe(f)).is_err();
    std::panic::set_hook(previous);
    r
}

/// Fonts of these weights (upright), each its own file.
fn fonts(weights: &[FontWeight]) -> Vec<Font> {
    weights
        .iter()
        .enumerate()
        .map(|(i, w)| {
            Font::new_with(FontSource::File(format!("{i}.ttf").into()), *w, FontStyle::Normal, None, Default::default())
        })
        .collect()
}

/// The index of the font CSS matching picks for `wanted` (C++'s FontWeight::closest), None of none.
fn closest(weights: &[FontWeight], wanted: FontWeight) -> Option<usize> {
    let fonts = fonts(weights);
    let picked = FontMatcher.match_font(&fonts, wanted, FontStyle::Normal).into_iter().next()?;
    fonts.iter().position(|f| *f == picked)
}

#[test]
fn font_weights_match_the_nearest_file_as_css_does() {
    use FontWeight as W;
    let regular_and_bold = [W::Normal, W::Bold];
    assert!(closest(&regular_and_bold, W::Medium) == Some(0) && closest(&regular_and_bold, W::SemiBold) == Some(1));
    // None lighter / heavier.
    assert!(closest(&regular_and_bold, W::Light) == Some(0) && closest(&regular_and_bold, W::Black) == Some(1));
    let four = [W::Light, W::Normal, W::Medium, W::Bold];
    // Between 400 and 500: up to 500 first.
    assert!(closest(&four, W::Normal) == Some(1) && closest(&four, W::new(450)) == Some(2));
    // Lighter go lighter, heavier heavier.
    assert!(closest(&four, W::new(350)) == Some(0) && closest(&four, W::new(550)) == Some(3));
    let ends = [W::Thin, W::Black];
    // After 400..500, lighter ones.
    assert!(closest(&ends, W::Normal) == Some(0) && closest(&ends, W::Medium) == Some(0));
    assert_eq!(closest(&[], W::Normal), None);
    // Emboldened: a weight of 600 and up asked of a face lighter than 600 (Compose's rule) and 200 or more heavier
    // than it (minikin's). (C++ emboldened a bold face for black too: Compose doesn't, the face being 600 or more.)
    let all = FontSynthesis::All;
    assert!(all.synthesizes_weight(W::Bold, W::Normal) && all.synthesizes_weight(W::Bold, W::Medium));
    assert!(!all.synthesizes_weight(W::Black, W::Bold));
    assert!(!all.synthesizes_weight(W::SemiBold, W::Medium) && !all.synthesizes_weight(W::Medium, W::Light));
    assert!(!all.synthesizes_weight(W::Bold, W::Bold) && !FontSynthesis::Style.synthesizes_weight(W::Bold, W::Normal));
    assert!(
        all.synthesizes_style(FontStyle::Italic, FontStyle::Normal)
            && !all.synthesizes_style(FontStyle::Normal, FontStyle::Italic)
    );
    assert!(!FontSynthesis::Weight.synthesizes_style(FontStyle::Italic, FontStyle::Normal));
    assert!(W::new(600) == W::SemiBold && W::W500 == W::Medium && W::Bold > W::Medium && W::default().weight == 400);
    assert!(panics(|| {
        W::new(0);
    }));
    assert_eq!(lerp_font_weight(W::Normal, W::Bold, 0.5), W::new(550));
    assert_eq!(format!("{:?}", W::Bold), "FontWeight(weight=700)");
}

#[test]
fn the_style_asked_for_wins_before_the_weight() {
    let upright = Font::file("r.ttf");
    let italic = Font::file("i.ttf").with_weight_and_style(FontWeight::Bold, FontStyle::Italic);
    let list = [upright.clone(), italic.clone()];
    // Italic asked: the italic face, whatever its weight.
    assert_eq!(FontMatcher.match_font(&list, FontWeight::Light, FontStyle::Italic), vec![italic.clone()]);
    // An exact match first.
    assert_eq!(FontMatcher.match_font(&list, FontWeight::Normal, FontStyle::Normal), vec![upright.clone()]);
    // No face of the style: among all.
    assert_eq!(
        FontMatcher.match_font(std::slice::from_ref(&upright), FontWeight::Normal, FontStyle::Italic),
        vec![upright.clone()]
    );
    let family = FontFamily::new(list);
    assert_eq!(FontMatcher.match_font_family(&family, FontWeight::Bold, FontStyle::Italic), vec![italic]);
    assert!(panics(|| {
        FontMatcher.match_font_family(&FontFamily::Serif, FontWeight::Bold, FontStyle::Normal);
    }));
}

#[test]
fn font_variations_pick_a_variable_fonts_instance() {
    use FontVariation as V;
    let w = V::weight(650);
    assert!(w.axis_name() == "wght" && w.to_variation_value(None) == 650.0 && !w.needs_density());
    assert_eq!((V::width(80.0).axis_name(), V::width(80.0).to_variation_value(None)), ("wdth", 80.0));
    assert_eq!((V::slant(-10.0).axis_name(), V::slant(-10.0).to_variation_value(None)), ("slnt", -10.0));
    assert_eq!((V::italic(1.0).axis_name(), V::grade(50).axis_name()), ("ital", "GRAD"));
    let opsz = V::optical_sizing(14.sp());
    assert!(opsz.axis_name() == "opsz" && opsz.needs_density());
    assert_eq!(opsz.to_variation_value(Some(Density::new(1.0, 2.0))), 28.0);
    assert_eq!(V::setting("XHGT", 0.5).to_variation_value(None), 0.5);
    // A whole number is not the same setting as a float (as Compose's).
    assert_ne!(V::weight(650), V::setting("wght", 650.0));
    let s = V::settings(FontWeight::SemiBold, FontStyle::Italic, [V::width(75.0)]);
    assert!(s.settings().len() == 3 && s.settings()[0] == V::weight(600) && s.settings()[1] == V::italic(1.0));
    assert_eq!(s.settings()[2], V::width(75.0));
    assert_eq!(V::settings(FontWeight::Normal, FontStyle::Normal, []).settings()[1], V::italic(0.0));
    assert_eq!(s.values(None)[0], (*b"wght", 600.0));
    assert!(panics(|| {
        FontVariationSettings::new([V::weight(300), V::weight(400)]);
    }));
    assert!(
        panics(|| {
            V::italic(2.0);
        }) && panics(|| {
            V::setting("long", 1.0);
            V::setting("toolong", 1.0);
        })
    );
    // The same file at two weights: two families' worth of files, told apart by their settings.
    let flex = |weight: i32| {
        FontFamily::new([Font::new_with(
            FontSource::File("fonts/Flex.ttf".into()),
            FontWeight::Light,
            FontStyle::Normal,
            Some(FontVariationSettings::new([V::weight(weight)])),
            FontLoadingStrategy::Blocking,
        )])
    };
    let (light, heavy, again) = (flex(300), flex(800), flex(300));
    assert!(light != heavy && light == again);
    let set: HashSet<FontFamily> = [light, heavy.clone(), again].into_iter().collect();
    assert_eq!(set.len(), 2);
    assert_eq!(heavy.fonts().unwrap()[0].variation_settings, FontVariationSettings::new([V::weight(800)]));
    // A font's settings default to its weight and style's, a device family's to none.
    let bold = kompozable_block!(Font(FontSource::Asset("b.ttf".into()), weight = FontWeight::Bold));
    assert_eq!(bold.variation_settings, V::settings(FontWeight::Bold, FontStyle::Normal, []));
    assert!(Font::device(DeviceFontFamilyName::new("serif-condensed")).variation_settings.is_empty());
}

#[test]
fn families_are_equal_by_what_they_are() {
    assert_eq!(FontFamily::Serif, FontFamily::generic(GenericFontFamily::Serif));
    assert_eq!(FontFamily::Monospace.as_generic(), Some(GenericFontFamily::Monospace));
    assert_eq!(GenericFontFamily::SansSerif.name(), Some("sans-serif"));
    assert_eq!(GenericFontFamily::from_name("cursive"), Some(GenericFontFamily::Cursive));
    assert_eq!(GenericFontFamily::Default.name(), None);
    // Bytes are told apart by their identity and index.
    let data: Arc<[u8]> = Arc::from(vec![1u8, 2, 3]);
    assert_eq!(Font::bytes("a", data.clone()), Font::bytes("a", Arc::<[u8]>::from(vec![9u8])));
    assert_ne!(Font::bytes("a", data.clone()), Font::bytes("b", data));
    assert_ne!(FontFamily::from(Font::file("x.ttf")), FontFamily::from(Font::asset("x.ttf")));
    assert!(panics(|| {
        FontFamily::new([]);
    }));
    assert!(panics(|| {
        DeviceFontFamilyName::new("");
    }));
    assert_eq!(format!("{:?}", FontFamily::Cursive), "FontFamily.Cursive");
    assert_eq!(FontFamily::default(), FontFamily::Default);
    // A loaded typeface's family is the typeface's.
    let typeface = FakeFontFamilyResolver::typeface(None, FontWeight::Bold, FontStyle::Normal, FontSynthesis::All);
    assert_eq!(FontFamily::from_typeface(typeface.clone()), FontFamily::Loaded(typeface));
}

#[test]
fn font_feature_settings_read_as_harfbuzz_reads_them() {
    let f = |s: &str| parse_feature(s).map(|f| (f.tag_str().to_string(), f.value, f.start, f.end));
    let all = FontFeature::GLOBAL_END;
    assert_eq!(f("kern"), Some(("kern".into(), 1, 0, all)));
    assert_eq!(f("+dlig"), Some(("dlig".into(), 1, 0, all)));
    assert_eq!(f("-liga"), Some(("liga".into(), 0, 0, all)));
    assert_eq!(f("aalt=2"), Some(("aalt".into(), 2, 0, all)));
    assert_eq!(f("\"liga\" 0"), Some(("liga".into(), 0, 0, all)));
    assert_eq!(f("'ss01' on"), Some(("ss01".into(), 1, 0, all)));
    assert_eq!(f("'ss01' off"), Some(("ss01".into(), 0, 0, all)));
    assert_eq!(f("kern[3:5]=0"), Some(("kern".into(), 0, 3, 5)));
    assert_eq!(f("kern[3]"), Some(("kern".into(), 1, 3, 4)));
    assert_eq!(f("kern[:5]"), Some(("kern".into(), 1, 0, 5)));
    assert_eq!(f("kern[5:]"), Some(("kern".into(), 1, 5, all)));
    assert_eq!(f("cv1"), Some(("cv1 ".into(), 1, 0, all)));
    assert_eq!(f("kern="), None);
    assert_eq!(f("toolong"), None);
    assert_eq!(f("\"abc\" 1"), None);
    assert_eq!(f("kern[3"), None);
    assert_eq!(f("kern 1 x"), None);
    let list = font::parse_font_feature_settings("tnum, smcp,, \"frac\" 1, nope=");
    assert_eq!(list.iter().map(|f| f.tag_str()).collect::<Vec<_>>(), vec!["tnum", "smcp", "frac"]);
    assert!(font::parse_font_feature_settings("").is_empty());
    assert_eq!(format!("{:?}", parse_feature("-kern").unwrap()), "-kern");
    assert_eq!(format!("{:?}", parse_feature("aalt[3:5]=2").unwrap()), "aalt[3:5]=2");
    assert_eq!(format!("{:?}", parse_feature("kern[3]").unwrap()), "kern[3]");
    assert_eq!(FontFeature::new(*b"smcp", 1), parse_feature("smcp").unwrap());
}

#[test]
fn fonts_are_registered_once() {
    let blob: Arc<[u8]> = Arc::from(vec![0u8; 16]);
    let a = FontRegistry::register(blob.clone(), 0);
    assert_eq!(FontRegistry::register(blob.clone(), 0), a);
    let b = FontRegistry::register(blob.clone(), 1);
    assert_ne!(a, b);
    let FontData { blob: back, index } = FontRegistry::get(b).expect("registered");
    assert!(Arc::ptr_eq(&back, &blob) && index == 1);
    // The same bytes in another allocation are another font.
    assert_ne!(FontRegistry::register(Arc::from(vec![0u8; 16]), 0), a);
    assert!(FontRegistry::get(FontId(u64::MAX)).is_none());
}

#[test]
fn the_fake_resolver_matches_and_fakes_what_a_family_lacks() {
    let resolver = FakeFontFamilyResolver::new();
    let regular = FontFamily::new([Font::file("regular.ttf")]);
    let t = resolver.resolve(Some(&regular), FontWeight::Bold, FontStyle::Italic, FontSynthesis::All).get();
    assert!(t.synthesis.embolden && t.synthesis.skew == GlyphSynthesis::ITALIC_SKEW);
    assert_eq!((t.weight, t.style, t.fallback), (FontWeight::Bold, FontStyle::Italic, GenericFontFamily::Default));
    assert_eq!(t.variation_settings, FontVariation::settings(FontWeight::Normal, FontStyle::Normal, []));
    let none = resolver.resolve(Some(&regular), FontWeight::Bold, FontStyle::Italic, FontSynthesis::None).get();
    assert_eq!(none.synthesis, GlyphSynthesis::None);
    // The system's families have every weight and style.
    let serif =
        resolver.resolve(Some(&FontFamily::Serif), FontWeight::Black, FontStyle::Italic, FontSynthesis::All).get();
    assert!(serif.synthesis == GlyphSynthesis::None && serif.fallback == GenericFontFamily::Serif);
    assert_eq!(serif.font, FakeBackend::font_id());
    assert_eq!(
        resolver.resolve(None, FontWeight::Normal, FontStyle::Normal, FontSynthesis::All).get().fallback,
        GenericFontFamily::Default
    );
    // A loaded typeface is itself.
    let loaded = FontFamily::from_typeface(t.clone());
    assert_eq!(resolver.resolve(Some(&loaded), FontWeight::Thin, FontStyle::Normal, FontSynthesis::All).get(), t);
    // Preloading needs nothing.
    let done = std::pin::pin!(resolver.preload(&regular));
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    assert!(matches!(done.poll(&mut cx), std::task::Poll::Ready(Ok(()))));
}
