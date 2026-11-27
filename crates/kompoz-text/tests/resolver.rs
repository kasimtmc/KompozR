//! The font family resolver over a font collection: the generic families as the collection has them, families of
//! one's own matched as CSS does and loaded by their strategy (blocking, optional local, async with the
//! typeface's state changing when the font arrives), files, assets, bytes and device fonts, preloading, variable
//! fonts' settings as coordinates, synthesis, the caches; and the system's fonts (KOMPOZ_SYSTEM_FONTS=1).

mod common;

use common::*;
use kompoz_graphics::UnitExt;
use kompoz_text::font::{AsyncFontLoader, FontBytes, FontCollection, FontFamilyResolverImpl};
use kompoz_text::*;
use std::cell::RefCell;
use std::future::Future;
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

fn face_id(r: &FontFamilyResolverImpl, family: &str) -> FontId {
    r.collection().face_by_name(family, FontWeight::Normal, FontStyle::Normal).expect(family).font
}

fn poll<F: Future>(f: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    f.poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn generic_families_are_the_collections() {
    let r = resolver();
    let c = r.collection();
    c.set_generic_families(GenericFontFamily::Serif, &["Noto Sans Hebrew"]);
    c.set_generic_families(GenericFontFamily::Monospace, &["Kompoz Test"]);
    let t = |family: Option<&FontFamily>| r.typeface(family, FontWeight::Normal, FontStyle::Normal, FontSynthesis::All);
    // The default family is sans-serif's.
    assert_eq!(t(None).font, face_id(&r, "Noto Sans"));
    assert_eq!(t(Some(&FontFamily::Default)).font, face_id(&r, "Noto Sans"));
    assert_eq!(t(Some(&FontFamily::SansSerif)).font, face_id(&r, "Noto Sans"));
    assert_eq!(t(Some(&FontFamily::Serif)).font, face_id(&r, "Noto Sans Hebrew"));
    let mono = t(Some(&FontFamily::Monospace));
    assert!(mono.font == face_id(&r, "Kompoz Test") && mono.fallback == GenericFontFamily::Monospace);
    assert!(mono.synthesis == GlyphSynthesis::None && mono.variation_settings.is_empty());
    // A generic family with no fonts: its fallback's characters still come from the collection.
    let cursive = t(Some(&FontFamily::Cursive));
    assert_eq!(cursive.fallback, GenericFontFamily::Cursive);
    let runs = runs(&*r.backend(), "a", &cursive, 16.0);
    assert_eq!(family_of(&r, runs[0].font.font), "Noto Sans");
    // A typeface loaded already is itself; a request resolves once.
    let loaded = FontFamily::from_typeface(mono.clone());
    assert_eq!(t(Some(&loaded)), mono);
    let first = r.resolve(None, FontWeight::Bold, FontStyle::Normal, FontSynthesis::All);
    assert!(first == r.resolve(None, FontWeight::Bold, FontStyle::Normal, FontSynthesis::All));
}

#[test]
fn families_of_ones_own_load_files_assets_bytes_and_device_fonts() {
    let r = resolver();
    let regular = |font: Font| {
        r.typeface(Some(&FontFamily::new([font])), FontWeight::Normal, FontStyle::Normal, FontSynthesis::All)
    };
    // A file, mapped.
    let file = regular(Font::file(font_path(KOMPOZ_TEST)));
    let data = FontRegistry::get(file.font).unwrap();
    assert_eq!(data.bytes(), font_bytes(KOMPOZ_TEST).as_slice());
    assert_eq!(file.variation_settings, FontVariation::settings(FontWeight::Normal, FontStyle::Normal, []));
    assert!(file.synthesis == GlyphSynthesis::None && file.fallback == GenericFontFamily::Default);
    // The same file again: the same font.
    let again = r.typeface(
        Some(&FontFamily::new([Font::file(font_path(KOMPOZ_TEST)), Font::file("other.ttf")])),
        FontWeight::Normal,
        FontStyle::Normal,
        FontSynthesis::All,
    );
    assert_eq!(again.font, file.font);
    // Bytes.
    let bytes = regular(Font::bytes("kompoz-test", font_bytes(KOMPOZ_TEST)));
    assert_eq!(FontRegistry::get(bytes.font).unwrap().bytes(), font_bytes(KOMPOZ_TEST).as_slice());
    // An asset, through the loader.
    let asked = Rc::new(RefCell::new(Vec::new()));
    let a = asked.clone();
    r.set_asset_loader(Rc::new(move |path: &str| {
        a.borrow_mut().push(path.to_string());
        let bytes: FontBytes = Arc::new(font_bytes(path.strip_prefix("fonts/")?));
        Some(bytes)
    }));
    let asset = regular(Font::asset(format!("fonts/{KOMPOZ_TEST}")));
    assert_eq!(FontRegistry::get(asset.font).unwrap().bytes(), font_bytes(KOMPOZ_TEST).as_slice());
    assert_eq!(*asked.borrow(), vec![format!("fonts/{KOMPOZ_TEST}")]);
    // A device font: the collection's family by name.
    let device = regular(Font::device(DeviceFontFamilyName::new("Noto Sans Hebrew")));
    assert_eq!(device.font, face_id(&r, "Noto Sans Hebrew"));
    // What is shaped in it is the font's.
    let runs = runs(&*r.backend(), "Ağ", &file, 16.0);
    assert!(runs.len() == 1 && runs[0].font.font == file.font);
}

#[test]
fn fonts_load_by_their_strategy() {
    let r = resolver();
    let default = default_typeface(&r);
    let resolve = |fonts: Vec<Font>| {
        r.typeface(Some(&FontFamily::new(fonts)), FontWeight::Normal, FontStyle::Normal, FontSynthesis::All)
    };
    let missing = |strategy| {
        Font::new_with(FontSource::File("missing.ttf".into()), FontWeight::Normal, FontStyle::Normal, None, strategy)
    };
    let kompoz = Font::file(font_path(KOMPOZ_TEST));
    // A blocking font that doesn't load: the default family (Compose's createDefaultTypeface).
    assert_eq!(resolve(vec![missing(FontLoadingStrategy::Blocking), kompoz.clone()]).font, default.font);
    // An optional one is passed over.
    let picked = resolve(vec![missing(FontLoadingStrategy::OptionalLocal), kompoz.clone()]);
    assert_eq!(FontRegistry::get(picked.font).unwrap().bytes(), font_bytes(KOMPOZ_TEST).as_slice());
    // A device font that isn't there, optional: passed over too.
    let absent = Font::new_with(
        FontSource::Device(DeviceFontFamilyName::new("No Such Family")),
        FontWeight::Normal,
        FontStyle::Normal,
        None,
        FontLoadingStrategy::OptionalLocal,
    );
    assert_eq!(resolve(vec![absent, kompoz.clone()]).font, picked.font);
}

#[test]
fn async_fonts_load_while_text_shows_the_default_family() {
    let r = resolver();
    // Loads the tests hold, finished by hand.
    let loads: Rc<RefCell<Vec<(Font, Box<dyn FnOnce(Option<FontBytes>) + Send>)>>> = Rc::new(RefCell::new(Vec::new()));
    let l = loads.clone();
    let loader: AsyncFontLoader = Rc::new(move |font: &Font, done| l.borrow_mut().push((font.clone(), done)));
    r.set_async_loader(loader);
    let async_font = |path: &str| {
        Font::new_with(
            FontSource::File(path.into()),
            FontWeight::Normal,
            FontStyle::Normal,
            None,
            FontLoadingStrategy::Async,
        )
    };
    let first = async_font("first.ttf");
    let second = async_font("second.ttf");
    let family = FontFamily::new([first.clone(), second.clone()]);
    let state = r.resolve(Some(&family), FontWeight::Bold, FontStyle::Normal, FontSynthesis::All);
    // Meanwhile the default family, faked for bold.
    let default = r.typeface(None, FontWeight::Bold, FontStyle::Normal, FontSynthesis::All);
    assert_eq!(state.peek(), default);
    assert_eq!(loads.borrow().iter().map(|(f, _)| f.clone()).collect::<Vec<_>>(), vec![first.clone(), second.clone()]);
    // The second loads first: the first may still, so nothing changes yet.
    let (_, done_second) = loads.borrow_mut().pop().unwrap();
    done_second(Some(Arc::new(font_bytes(NOTO_HEBREW))));
    assert_eq!(state.peek(), default);
    // The first fails: the second it is, faked for bold (its font is regular).
    let (_, done_first) = loads.borrow_mut().pop().unwrap();
    done_first(None);
    let t = state.peek();
    assert_eq!(FontRegistry::get(t.font).unwrap().bytes(), font_bytes(NOTO_HEBREW).as_slice());
    assert!(t.synthesis.embolden && t.weight == FontWeight::Bold);
    // Resolved again: the same state; the failed font stays failed, the loaded one is used at once.
    assert!(state == r.resolve(Some(&family), FontWeight::Bold, FontStyle::Normal, FontSynthesis::All));
    let now = r.typeface(Some(&family), FontWeight::Normal, FontStyle::Normal, FontSynthesis::All);
    assert!(now.font == t.font && !now.synthesis.embolden && loads.borrow().is_empty());
    // A family whose async fonts all fail keeps the default family.
    let failing = FontFamily::new([async_font("bad.ttf")]);
    let state = r.resolve(Some(&failing), FontWeight::Normal, FontStyle::Normal, FontSynthesis::All);
    let (_, done) = loads.borrow_mut().pop().unwrap();
    done(None);
    assert_eq!(state.peek().font, default_typeface(&r).font);
}

#[test]
fn async_files_load_on_a_background_thread() {
    let r = resolver();
    let font = Font::new_with(
        FontSource::File(font_path(NOTO_DEVANAGARI)),
        FontWeight::Normal,
        FontStyle::Normal,
        None,
        FontLoadingStrategy::Async,
    );
    let state = r.resolve(Some(&FontFamily::new([font])), FontWeight::Normal, FontStyle::Normal, FontSynthesis::All);
    let deadline = Instant::now() + Duration::from_secs(10);
    while state.peek().font == default_typeface(&r).font {
        assert!(Instant::now() < deadline, "the font didn't load");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(FontRegistry::get(state.peek().font).unwrap().bytes(), font_bytes(NOTO_DEVANAGARI).as_slice());
}

#[test]
fn preloading_loads_a_familys_async_fonts() {
    let r = resolver();
    let loads: Rc<RefCell<Vec<(Font, Box<dyn FnOnce(Option<FontBytes>) + Send>)>>> = Rc::new(RefCell::new(Vec::new()));
    let l = loads.clone();
    r.set_async_loader(Rc::new(move |font: &Font, done| l.borrow_mut().push((font.clone(), done))));
    let async_font = |path: &str, weight| {
        Font::new_with(FontSource::File(path.into()), weight, FontStyle::Normal, None, FontLoadingStrategy::Async)
    };
    let family = FontFamily::new([
        async_font("regular.ttf", FontWeight::Normal),
        async_font("bold.ttf", FontWeight::Bold),
        Font::file(font_path(KOMPOZ_TEST)),
    ]);
    let mut preload = pin!(r.preload(&family));
    assert!(poll(preload.as_mut()).is_pending());
    assert_eq!(loads.borrow().len(), 2);
    for (_, done) in loads.borrow_mut().drain(..) {
        done(Some(Arc::new(font_bytes(KOMPOZ_TEST))));
    }
    assert!(matches!(poll(preload.as_mut()), Poll::Ready(Ok(()))));
    // Preloaded: resolved at once.
    let t = r.typeface(Some(&family), FontWeight::Bold, FontStyle::Normal, FontSynthesis::All);
    assert!(!t.synthesis.embolden && loads.borrow().is_empty());
    // A font that fails fails the preload.
    let bad = FontFamily::new([async_font("bad.ttf", FontWeight::Normal)]);
    let mut preload = pin!(r.preload(&bad));
    let (_, done) = loads.borrow_mut().pop().unwrap();
    done(None);
    match poll(preload.as_mut()) {
        Poll::Ready(Err(e)) => assert!(e.font.is_some() && e.to_string().contains("bad.ttf")),
        _ => panic!("the preload should have failed"),
    }
    // Nothing to load: done at once.
    assert!(matches!(poll(pin!(r.preload(&FontFamily::Serif))), Poll::Ready(Ok(()))));
}

#[test]
fn variation_settings_set_a_variable_fonts_coordinates() {
    let r = FontFamilyResolverImpl::registered_only();
    let b = r.harfrust_backend().clone();
    let cantarell = |weight: FontWeight, settings: Option<FontVariationSettings>| {
        let font = Font::new_with(
            FontSource::File(font_path(CANTARELL)),
            weight,
            FontStyle::Normal,
            settings,
            FontLoadingStrategy::Blocking,
        );
        r.typeface(Some(&FontFamily::new([font])), weight, FontStyle::Normal, FontSynthesis::All)
    };
    let coords = |t: &Typeface| {
        let runs = runs(&*b, "Kompoz", t, 16.0);
        assert_eq!(runs.len(), 1);
        runs[0].font.coords.to_vec()
    };
    // Its axis: wght 100 to 800, 400 the default: regular is the default location.
    let regular = cantarell(FontWeight::Normal, None);
    assert_eq!(coords(&regular), Vec::<i16>::new());
    let heaviest = cantarell(FontWeight::W800, None);
    assert_eq!(coords(&heaviest), vec![16384]);
    let lightest = cantarell(FontWeight::Thin, None);
    assert_eq!(coords(&lightest), vec![-16384]);
    let bold = coords(&cantarell(FontWeight::Bold, None));
    assert!(bold.len() == 1 && bold[0] > 0 && bold[0] < 16384);
    // Clamped to the axis; settings of one's own; an axis the font lacks is left out.
    let beyond = FontVariationSettings::new([FontVariation::weight(1000), FontVariation::setting("wdth", 50.0)]);
    assert_eq!(coords(&cantarell(FontWeight::Normal, Some(beyond))), vec![16384]);
    assert_eq!(b.coords(regular.font, &[(*b"wght", 800.0)]).to_vec(), vec![16384]);
    assert_eq!(b.coords(regular.font, &[(*b"XXXX", 1.0)]).to_vec(), Vec::<i16>::new());
    // Heavier is wider.
    let shaped = |t: &Typeface| shape_run(&*b, "Kompoz", &runs(&*b, "Kompoz", t, 16.0)[0], false).advance;
    assert!(shaped(&heaviest) > shaped(&regular) && shaped(&regular) > shaped(&lightest));
    // A setting in sp follows the font scale: an optical size of 12 sp at scale 2 is 24.
    let opsz = FontVariationSettings::new([FontVariation::optical_sizing(12.sp())]);
    let density = kompoz_graphics::Density { density: 1.0, font_scale: 2.0 };
    assert_eq!(opsz.values(Some(density)), vec![(*b"opsz", 24.0)]);
    // A face designed bold and asked for bold isn't faked; the design's weight is the font's.
    assert_eq!(b.design(regular.font), Some((400.0, false)));
}

#[test]
fn a_collection_registers_families_and_its_fallbacks() {
    let c = FontCollection::registered_only();
    assert_eq!(c.register(Arc::new(font_bytes(KOMPOZ_TEST))), vec!["Kompoz Test".to_string()]);
    assert_eq!(c.register_file(font_path(NOTO_SANS)), Some(vec!["Noto Sans".to_string()]));
    assert_eq!(c.register_file("missing.ttf"), None);
    let mut names = c.family_names();
    names.sort();
    assert_eq!(names, vec!["Kompoz Test".to_string(), "Noto Sans".to_string()]);
    let kompoz = c.family_id("kompoz test").expect("by name, any case");
    assert_eq!(c.family_name(kompoz).as_deref(), Some("Kompoz Test"));
    assert_eq!(c.registered_families().len(), 2);
    // Nothing generic until said.
    assert!(c.generic_families(GenericFontFamily::Serif).is_empty());
    c.set_generic_families(GenericFontFamily::Serif, &["Noto Sans", "Kompoz Test", "Missing"]);
    assert_eq!(c.generic_families(GenericFontFamily::Serif).len(), 2);
    c.set_fallback_families(*b"Arab", Some(&Locale::new("fa")), &["Kompoz Test"]);
    assert_eq!(c.fallback_families(*b"Arab", Some(&Locale::new("fa"))), vec![kompoz]);
    assert!(c.fallback_families(*b"Arab", None).is_empty());
    // A face for a weight and style: the one there is, its design's weight.
    let face = c.face(kompoz, FontWeight::Bold, FontStyle::Italic).unwrap();
    assert!(face.weight == 400.0 && !face.italic && face.variations.is_empty());
    assert_eq!(c.face(kompoz, FontWeight::Normal, FontStyle::Normal).unwrap().font, face.font);
}

/// A directory's fonts alone (`KOMPOZ_FONTS`): its font files registered (others left out), every generic family its
/// first family until set otherwise, no system fonts.
#[test]
fn a_collection_of_a_directorys_fonts_alone() {
    let c = FontCollection::of_directory(font_path(""));
    assert!(!c.has_system_fonts());
    let mut names = c.family_names();
    names.sort();
    assert!(names.contains(&"Noto Sans".to_string()) && names.contains(&"Kompoz Test".to_string()), "{names:?}");
    // (Cantarell-VF-Subset.otf comes first by name; generate.py and README aren't fonts)
    let first = c.family_id("Cantarell").expect("Cantarell");
    for generic in [GenericFontFamily::SansSerif, GenericFontFamily::Serif, GenericFontFamily::Monospace] {
        assert_eq!(c.generic_families(generic), vec![first]);
    }
    c.set_generic_families(GenericFontFamily::Monospace, &["Noto Sans"]);
    assert_eq!(c.generic_families(GenericFontFamily::Monospace), vec![c.family_id("Noto Sans").unwrap()]);
    assert!(FontCollection::of_directory("missing-directory").family_names().is_empty());
}

/// The system's fonts (fontconfig on Linux, loaded at run time): set KOMPOZ_SYSTEM_FONTS=1 to run it on a machine
/// that has fonts.
#[test]
fn system_fonts_resolve_and_shape() {
    if std::env::var("KOMPOZ_SYSTEM_FONTS").as_deref() != Ok("1") {
        eprintln!("system_fonts_resolve_and_shape: skipped (KOMPOZ_SYSTEM_FONTS=1 runs it)");
        return;
    }
    let r = FontFamilyResolverImpl::system();
    assert!(r.collection().has_system_fonts() && !r.collection().family_names().is_empty());
    let t = default_typeface(&r);
    let b = r.backend();
    let text = "Merhaba dünya مرحبا 👍";
    let runs = runs(&*b, text, &t, 16.0);
    assert_eq!(runs.first().unwrap().font.font, t.font);
    assert_eq!(runs.last().unwrap().range.end, text.len());
    for run in &runs {
        let shaped = shape_run(&*b, text, run, false);
        assert!(shaped.advance > 0.0, "{:?} shaped", &text[run.range.clone()]);
    }
    let m = b.metrics(&runs[0].font);
    assert!(m.ascent > 0.0 && m.descent > 0.0);
    eprintln!(
        "system fonts: {} runs, {:?}",
        runs.len(),
        runs.iter().map(|run| family_of(&r, run.font.font)).collect::<Vec<_>>()
    );
}
