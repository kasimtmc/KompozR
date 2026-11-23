//! Resources as CMP's components-resources reads them, of tests/composeResources through its generated `Res`: the
//! C++ Kompoz's image resource tests (image_resource_loads_once, image_assets_are_decoded_once_wherever_they_are_used,
//! the latter on CMP's caches), the generated accessors and records, picking items by locale, theme and density
//! (CMP's ResourceEnvironment tests' cases), strings (formatted, quantity strings, arrays, special characters) and
//! their language following LocalStrings, images scaled to the screen's density, vector and SVG painters, fonts,
//! the readers, the caches and the system environment.

#![allow(non_snake_case)]

use kompoz_graphics::{Density, ImageBitmap, Size};
use kompoz_resources::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_text::FontSource;
use kompoz_ui::draw::{Painter, VectorPainter};
use kompoz_ui::strings::{LocalStrings, Strings};
use std::cell::RefCell;
use std::rc::Rc;

compose_resources!(dir = "tests/composeResources", package = "kompoz.tests.generated.resources");

const MD: &str = "composeResources/kompoz.tests.generated.resources/";

// ---- Helpers --------------------------------------------------------------------------------------------------------

/// The default reader, each read counted (the C++ host's `loadImage` callback counted).
struct CountingReader {
    reads: Rc<RefCell<Vec<String>>>,
}

impl ResourceReader for CountingReader {
    fn read<'a>(&'a self, path: &'a str) -> ResourceFuture<'a, Vec<u8>> {
        self.reads.borrow_mut().push(path.trim_start_matches(MD).to_string());
        DefaultResourceReader.read(path)
    }
    fn read_part<'a>(&'a self, path: &'a str, offset: i64, size: i64) -> ResourceFuture<'a, Vec<u8>> {
        self.reads.borrow_mut().push(format!("{}@{offset}", path.trim_start_matches(MD)));
        DefaultResourceReader.read_part(path, offset, size)
    }
    fn get_uri(&self, path: &str) -> Result<String, MissingResourceException> {
        DefaultResourceReader.get_uri(path)
    }
}

fn counting() -> (ResourceReaderRef, Rc<RefCell<Vec<String>>>) {
    let reads = Rc::new(RefCell::new(Vec::new()));
    (ResourceReaderRef::new(CountingReader { reads: reads.clone() }), reads)
}

fn env(language: &str, region: &str, dark: bool, density: DensityQualifier) -> ResourceEnvironment {
    ResourceEnvironment::new(
        LanguageQualifier::new(language),
        RegionQualifier::new(region),
        ThemeQualifier::select_by_value(dark),
        density,
    )
}

fn english() -> ResourceEnvironment {
    env("en", "US", false, DensityQualifier::Mdpi)
}

/// What `read` gives in a composition with `strings` (the UI language) provided.
fn in_language<T: Clone + 'static>(locale: &str, read: impl Fn() -> T + 'static) -> T {
    let seen: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let strings = Strings { locale: locale.into(), ..Strings::english() };
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, read) = (s.clone(), &read);
        KompozitionLocalProvider(&[LocalStrings.provides(strings.clone())], || {
            *s.borrow_mut() = Some(read());
        });
    }));
    seen.borrow().clone().expect("composed")
}

fn pixel(image: &ImageBitmap, x: i32, y: i32) -> [u8; 4] {
    let p = image.pixels();
    let i = ((y * image.width() + x) * 4) as usize;
    [p[i], p[i + 1], p[i + 2], p[i + 3]]
}

// ---- The C++ Kompoz's tests -----------------------------------------------------------------------------------------

/// C++ image_resource_loads_once: an image resource read in a composition is loaded once, remembered across
/// recompositions.
fn image_resource_loads_once() {
    ResourceCaches::clear_blocking();
    let (reader, reads) = counting();
    let show = mutable_state_of(false);
    let tick = mutable_state_of(0);
    let seen: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let (sh, t, s) = (show.clone(), tick.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if !sh.get() {
            return;
        }
        let (t, s) = (t.clone(), s.clone());
        KompozitionLocalProvider(&[LocalResourceReader.provides(reader.clone())], || {
            let image = image_resource(Res::drawable::x());
            *s.borrow_mut() = format!("{}:{}", t.get(), image.width());
        });
    }));
    show.set(true);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), "0:8");
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), "1:8");
    assert_eq!(*reads.borrow(), vec!["drawable/x.png".to_string()]); // remembered across recompositions
}

/// C++ image_assets_are_decoded_once_wherever_they_are_used, on CMP's caches: decoded once wherever they are used
/// and however often their screen comes back; CMP keeps them for the process (no budget: C++'s least recently
/// used beyond 64 MB is evicted, CMP's caches only go when cleared), so two large ones don't push `a` out —
/// clearing the caches does.
fn image_assets_are_decoded_once_wherever_they_are_used() {
    ResourceCaches::clear_blocking();
    let (reader, reads) = counting();
    let shown = mutable_state_of(0);
    let ids: Rc<RefCell<Vec<u64>>> = Rc::new(RefCell::new(Vec::new()));
    let (sh, i) = (shown.clone(), ids.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sh, i) = (sh.clone(), i.clone());
        KompozitionLocalProvider(&[LocalResourceReader.provides(reader.clone())], || match sh.get() {
            1 => {
                let (first, second) = (image_resource(Res::drawable::a()), image_resource(Res::drawable::a()));
                let b = image_resource(Res::drawable::b());
                i.borrow_mut().extend([first.id(), second.id(), b.id()]);
            }
            2 => {
                image_resource(Res::drawable::c());
                image_resource(Res::drawable::d());
            }
            3 => {
                i.borrow_mut().push(image_resource(Res::drawable::a()).id());
            }
            _ => {}
        });
    }));
    shown.set(1);
    rule.wait_for_idle();
    assert_eq!(*reads.borrow(), vec!["drawable/a.png", "drawable/b.png"]);
    assert_eq!(ids.borrow()[0], ids.borrow()[1]); // the same bitmap
    shown.set(0); // the screen goes and comes back: nothing decoded again
    rule.wait_for_idle();
    shown.set(1);
    rule.wait_for_idle();
    assert_eq!(reads.borrow().len(), 2);
    shown.set(2); // 32 MB more
    rule.wait_for_idle();
    shown.set(3);
    rule.wait_for_idle();
    assert_eq!(*reads.borrow(), vec!["drawable/a.png", "drawable/b.png", "drawable/c.png", "drawable/d.png"]);
    let a = *ids.borrow().last().unwrap();
    assert_eq!(a, ids.borrow()[0]); // still the one decoded first
    ResourceCaches::clear_blocking();
    shown.set(0);
    rule.wait_for_idle();
    shown.set(3);
    rule.wait_for_idle();
    assert_eq!(reads.borrow().last().map(String::as_str), Some("drawable/a.png"));
    assert_ne!(*ids.borrow().last().unwrap(), a);
}

// ---- The generated Res --------------------------------------------------------------------------------------------

/// Each resource's id, key, items (path, qualifiers in CMP's order, offsets of records), the maps of all of them,
/// raw files and their URIs.
fn the_generated_res_has_cmps_accessors() {
    let logo = Res::drawable::logo();
    assert_eq!(logo.id(), "drawable:logo");
    let paths: Vec<&str> = logo.items().iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, vec![format!("{MD}drawable-xxhdpi/logo.png"), format!("{MD}drawable/logo.png")]);
    assert_eq!(logo.items()[0].qualifiers, vec![Qualifier::Density(DensityQualifier::Xxhdpi)]);
    assert!(logo.items()[1].qualifiers.is_empty() && logo.items()[1].offset == -1 && logo.items()[1].size == -1);
    assert_eq!(Res::drawable::logo(), logo); // equal by id
    let app = Res::string::app_name();
    assert_eq!((app.id(), app.key()), ("string:app_name", "app_name"));
    let br = app.items().iter().find(|i| i.path.contains("values-pt-rBR")).expect("pt-BR");
    assert_eq!(
        br.qualifiers,
        vec![Qualifier::Language(LanguageQualifier::new("pt")), Qualifier::Region(RegionQualifier::new("BR"))]
    );
    assert_eq!(br.path, format!("{MD}values-pt-rBR/strings.commonMain.cvr"));
    assert!(br.offset > 0 && br.size > 0);
    assert_eq!(Res::string::multi_part().key(), "multi_part"); // (CMP underscores the key too)
    assert_eq!(Res::plurals::messages().id(), "plurals:messages");
    assert_eq!(Res::array::planets().id(), "string-array:planets");
    assert_eq!(Res::font::regular().id(), "font:regular");
    let drawables: Vec<&str> = Res::all_drawable_resources().keys().map(String::as_str).collect();
    assert_eq!(drawables, vec!["a", "b", "c", "d", "icon", "logo", "shape", "theme", "x"]);
    assert_eq!(Res::all_string_resources().len(), 5);
    assert_eq!(Res::all_plural_string_resources().len(), 2);
    assert_eq!(Res::all_string_array_resources()["planets"], Res::array::planets());
    assert_eq!(Res::all_font_resources().len(), 1);
    // Raw files, read and given a URI.
    assert_eq!(run_blocking(Res::read_bytes("files/data.bin")), b"kompoz");
    assert_eq!(run_blocking(Res::read_bytes("files/nested/note.txt")), "iç".as_bytes());
    let missing = run_blocking(Res::try_read_bytes("files/none")).unwrap_err();
    assert_eq!(missing.to_string(), format!("Missing resource with path: {MD}files/none"));
    let uri = Res::get_uri("files/data.bin");
    assert!(uri.starts_with("file://") && uri.ends_with("/tests/composeResources/files/data.bin"), "{uri}");
}

/// The values files as the build converts them: `version:0`, then the records sorted, each `type|key|base64`.
fn values_are_converted_into_cmps_records() {
    let cvr = DefaultResourceReader::read_now(&format!("{MD}values/strings.commonMain.cvr")).unwrap();
    let text = String::from_utf8(cvr).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "version:0");
    let mut sorted = lines[1..].to_vec();
    sorted.sort();
    assert_eq!(lines[1..], sorted[..]);
    assert!(lines.contains(&"string|app_name|S29tcG96"));
    assert!(lines.iter().any(|l| l.starts_with("plurals|messages|ONE:") && l.contains(",OTHER:")));
    assert!(lines.iter().any(|l| l.starts_with("string-array|planets|") && l.matches(',').count() == 2));
    // Each item's offset and size are its record's.
    let item = &Res::string::app_name().items().iter().find(|i| i.qualifiers.is_empty()).unwrap().clone();
    let record = DefaultResourceReader::read_part_now(&item.path, item.offset, item.size).unwrap();
    assert_eq!(record, b"string|app_name|S29tcG96");
}

// ---- Picking items ----------------------------------------------------------------------------------------------

fn item(qualifiers: Vec<Qualifier>, path: &str) -> ResourceItem {
    ResourceItem::new(qualifiers, path, -1, -1)
}

/// CMP's ResourceEnvironment tests' cases: the exact locale, else the language, else the default; the theme;
/// the nearest density at or above, else below (LDPI last), else the default; none or two: errors.
fn items_are_picked_as_android_picks_them() {
    let lang = |l: &str| Qualifier::Language(LanguageQualifier::new(l));
    let region = |r: &str| Qualifier::Region(RegionQualifier::new(r));
    let density = |d: DensityQualifier| Qualifier::Density(d);
    let r = DrawableResource::new(
        "drawable:img",
        vec![
            item(vec![], "default"),
            item(vec![lang("en")], "en"),
            item(vec![lang("en"), region("US")], "en-US"),
            item(vec![Qualifier::Theme(ThemeQualifier::Dark)], "dark"),
            item(vec![density(DensityQualifier::Hdpi)], "hdpi"),
            item(vec![density(DensityQualifier::Xxhdpi)], "xxhdpi"),
            item(vec![density(DensityQualifier::Ldpi)], "ldpi"),
        ],
    );
    let pick = |e: &ResourceEnvironment| r.get_resource_item_by_environment(e).map(|i| i.path.clone());
    assert_eq!(pick(&env("en", "US", false, DensityQualifier::Mdpi)).unwrap(), "en-US");
    assert_eq!(pick(&env("en", "GB", false, DensityQualifier::Mdpi)).unwrap(), "en");
    // No language: theme, then density among the rest.
    assert_eq!(pick(&env("de", "DE", true, DensityQualifier::Mdpi)).unwrap(), "dark");
    assert_eq!(pick(&env("de", "DE", false, DensityQualifier::Mdpi)).unwrap(), "hdpi");
    assert_eq!(pick(&env("de", "DE", false, DensityQualifier::Xhdpi)).unwrap(), "xxhdpi");
    assert_eq!(pick(&env("de", "DE", false, DensityQualifier::Xxxhdpi)).unwrap(), "xxhdpi"); // (the nearest below)
    let only = |q: Vec<Qualifier>| DrawableResource::new("drawable:o", vec![item(q, "q"), item(vec![], "none")]);
    let ldpi = only(vec![density(DensityQualifier::Ldpi)]);
    assert_eq!(ldpi.get_resource_item_by_environment(&english()).unwrap().path, "none"); // the default before LDPI
    let low = DrawableResource::new("drawable:l", vec![item(vec![density(DensityQualifier::Ldpi)], "ldpi")]);
    assert_eq!(low.get_resource_item_by_environment(&english()).unwrap().path, "ldpi");
    // None, and more than one.
    let none = DrawableResource::new("drawable:n", vec![item(vec![lang("tr")], "tr")]);
    assert_eq!(
        none.get_resource_item_by_environment(&english()).unwrap_err().to_string(),
        "Resource with ID='drawable:n' not found"
    );
    let two = DrawableResource::new("drawable:t", vec![item(vec![], "a.png"), item(vec![], "b.png")]);
    assert_eq!(
        two.get_resource_item_by_environment(&english()).unwrap_err().to_string(),
        "Resource with ID='drawable:t' has more than one file: a.png, b.png"
    );
    // The qualifiers of a density and a theme.
    assert_eq!(DensityQualifier::select_by_density(2.0), DensityQualifier::Xhdpi);
    assert_eq!(DensityQualifier::select_by_density(2.5), DensityQualifier::Xxhdpi);
    assert_eq!(DensityQualifier::select_by_value(96), DensityQualifier::Ldpi);
    assert_eq!(DensityQualifier::select_by_value(161), DensityQualifier::Hdpi);
    assert_eq!(DensityQualifier::select_by_value(1000), DensityQualifier::Xxxhdpi);
}

// ---- Strings ------------------------------------------------------------------------------------------------------

/// Strings in the UI's language (LocalStrings' locale): Turkish, Brazilian Portuguese (language and region),
/// Portuguese elsewhere (the language), German (the default), switched while composed.
fn strings_follow_the_ui_language() {
    assert_eq!(in_language("en-US", || string_resource(Res::string::app_name())), "Kompoz");
    assert_eq!(in_language("tr", || string_resource(Res::string::app_name())), "Kompoz TR");
    assert_eq!(in_language("pt-BR", || string_resource(Res::string::app_name())), "Kompoz BR");
    assert_eq!(in_language("pt-PT", || string_resource(Res::string::app_name())), "Kompoz PT");
    assert_eq!(in_language("de", || string_resource(Res::string::app_name())), "Kompoz");
    assert_eq!(in_language("tr", || string_resource(Res::string::only_default())), "Only in English");
    // Switched: the composition reads the new language's.
    let locale = mutable_state_of("en".to_string());
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (l, s) = (locale.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        let strings = Strings { locale: l.get(), ..Strings::english() };
        KompozitionLocalProvider(&[LocalStrings.provides(strings)], || {
            s.borrow_mut().push(string_resource(Res::string::app_name()));
        });
    }));
    locale.set("tr".into());
    rule.wait_for_idle();
    assert_eq!(seen.borrow().last().map(String::as_str), Some("Kompoz TR"));
}

/// `%N$s`/`%N$d` placeholders, quantity strings by CLDR's categories (English, Turkish), string arrays, Android's
/// escapes and XML's entities and CDATA.
fn strings_are_formatted_and_pluralized() {
    let greeting = in_language("en", || string_resource_formatted(Res::string::greeting(), &[&"Ada", &3]));
    assert_eq!(greeting, "Hello, Ada! You have 3 new messages.");
    let tr = in_language("tr", || string_resource_formatted(Res::string::greeting(), &[&"Ada", &3]));
    assert_eq!(tr, "Merhaba Ada! 3 yeni iletiniz var.");
    let plural = |locale: &str, n: i32| {
        in_language(locale, move || plural_string_resource_formatted(Res::plurals::messages(), n, &[&n]))
    };
    assert_eq!(plural("en", 1), "1 message");
    assert_eq!(plural("en", 5), "5 messages");
    assert_eq!(plural("tr", 1), "1 ileti");
    // English has no `zero`: 0 is `other`.
    let apples = in_language("en", || plural_string_resource(Res::plurals::apples(), 0));
    assert_eq!(apples, "%1$d apples");
    assert_eq!(in_language("en", || plural_string_resource(Res::plurals::apples(), 1)), "one apple");
    assert_eq!(in_language("en", || string_array_resource(Res::array::planets())), vec!["Mercury", "Venus", "Earth"]);
    assert_eq!(in_language("tr", || string_array_resource(Res::array::planets()))[2], "Dünya");
    let special = in_language("en", || string_resource(Res::string::special()));
    assert_eq!(special, "Line\nTab\tUnicode ç kept \\n & <b> <i>raw</i>");
}

/// The suspending functions, in an environment given or the system's (CMP's tests replace the latter).
fn strings_load_outside_a_composition() {
    let tr = env("tr", "", false, DensityQualifier::Mdpi);
    assert_eq!(run_blocking(get_string_in(&tr, &Res::string::app_name())).unwrap(), "Kompoz TR");
    assert_eq!(
        run_blocking(get_string_formatted_in(&tr, &Res::string::greeting(), &[&"Can", &2])).unwrap(),
        "Merhaba Can! 2 yeni iletiniz var."
    );
    assert_eq!(
        run_blocking(get_plural_string_formatted_in(&english(), &Res::plurals::messages(), 2, &[&2])).unwrap(),
        "2 messages"
    );
    assert_eq!(run_blocking(get_string_array_in(&tr, &Res::array::planets())).unwrap()[0], "Merkür");
    set_resource_environment_provider(|| env("pt", "BR", false, DensityQualifier::Mdpi));
    assert_eq!(run_blocking(get_string(&Res::string::app_name())).unwrap(), "Kompoz BR");
    assert_eq!(get_system_resource_environment().region, RegionQualifier::new("BR"));
    set_resource_environment_provider(system_environment);
    // A key of another kind: what Kotlin's cast throws.
    let wrong = StringResource::new("string:planets", "planets", Res::array::planets().items().to_vec());
    assert!(matches!(run_blocking(get_string_in(&english(), &wrong)), Err(StringResourceError::WrongType(_))));
}

/// CLDR's rules: Russian's and Arabic's categories, Turkish's, Polish's; a rule that doesn't parse.
fn plural_rules_are_cldrs() {
    let ru = run_blocking(PluralRuleList::get_instance_of("ru"));
    let cats: Vec<PluralCategory> = [1, 2, 5, 11, 21, 22, 25].iter().map(|n| ru.get_category(*n)).collect();
    use PluralCategory::*;
    assert_eq!(cats, vec![One, Few, Many, Many, One, Few, Many]);
    let ar = run_blocking(PluralRuleList::get_instance_of("ar"));
    assert_eq!([0, 1, 2, 3, 11, 100].map(|n| ar.get_category(n)), [Zero, One, Two, Few, Many, Other]);
    let pl = run_blocking(PluralRuleList::get_instance(&LanguageQualifier::new("pl"), &RegionQualifier::new("PL")));
    assert_eq!([1, 3, 5, 22].map(|n| pl.get_category(n)), [One, Few, Many, Few]);
    let tr = run_blocking(PluralRuleList::get_instance(&LanguageQualifier::new("tr"), &RegionQualifier::new("")));
    assert_eq!([1, 2].map(|n| tr.get_category(n)), [One, Other]);
    let rule = PluralRule::new(One, "i = 1 and v = 0").unwrap();
    assert_eq!(rule.condition(), "n = 1"); // (v is 0 for whole numbers)
    let bad = PluralRule::new(One, "n = 1 nor").unwrap_err();
    assert_eq!(bad.to_string(), "Invalid syntax at position 7: n = 1 nor");
}

// ---- Images -------------------------------------------------------------------------------------------------------

/// The image of the screen's density, scaled down to it (an xxhdpi one on an mdpi screen: a third), never up.
fn images_are_scaled_to_the_screens_density() {
    ResourceCaches::clear_blocking();
    let size = |density: f32| {
        let seen: Rc<RefCell<(i32, i32)>> = Rc::new(RefCell::new((0, 0)));
        let s = seen.clone();
        let rule = KompozTestRule::empty();
        rule.host().set_density(Density { density, font_scale: 1.0 });
        rule.set_content(kompozable_lambda!(move || {
            let image = image_resource(Res::drawable::logo());
            *s.borrow_mut() = (image.width(), image.height());
        }));
        rule.wait_for_idle();
        *seen.borrow()
    };
    assert_eq!(size(1.0), (16, 10)); // xxhdpi's 48 x 30 at mdpi
    assert_eq!(size(3.0), (48, 30));
    assert_eq!(size(4.0), (48, 30)); // (the nearest below, not scaled up)
    assert_eq!(size(2.0), (32, 20));
}

/// The dark theme's image where the system is dark.
fn images_follow_the_theme() {
    let corner = |dark: bool| {
        let seen: Rc<RefCell<Option<ImageBitmap>>> = Rc::new(RefCell::new(None));
        let s = seen.clone();
        let rule = KompozTestRule::empty();
        rule.host().set_system_dark_theme(dark);
        rule.set_content(kompozable_lambda!(move || {
            *s.borrow_mut() = Some(image_resource(Res::drawable::theme()));
        }));
        rule.wait_for_idle();
        let image = seen.borrow().clone().unwrap();
        pixel(&image, 0, 0)
    };
    assert_eq!(corner(false), [255, 255, 255, 255]);
    assert_eq!(corner(true), [0, 0, 0, 255]);
}

/// painterResource by the file's kind: a bitmap's, a vector's (its default size in px), an SVG's (its size in px
/// times the density); the vector as CMP's parser reads it (clip paths closed with their group, gradients).
fn painters_are_of_the_files_kind() {
    ResourceCaches::clear_blocking();
    let seen: Rc<RefCell<Vec<Rc<dyn Painter>>>> = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::empty();
    rule.host().set_density(Density { density: 2.0, font_scale: 1.0 });
    rule.set_content(kompozable_lambda!(move || {
        let mut s = s.borrow_mut();
        s.clear();
        s.push(painter_resource(Res::drawable::x()));
        s.push(painter_resource(Res::drawable::icon()));
        s.push(painter_resource(Res::drawable::shape()));
    }));
    rule.wait_for_idle();
    let painters = seen.borrow();
    assert_eq!(painters[0].intrinsic_size(), Size::new(8.0, 4.0));
    let vector = painters[1].as_any().downcast_ref::<VectorPainter>().expect("a vector painter").vector();
    assert_eq!(painters[1].intrinsic_size(), Size::new(48.0, 24.0));
    assert!(vector.auto_mirror() && vector.viewport_width() == 48.0);
    let root = vector.root();
    let g = root.iter().find_map(|n| n.group()).expect("the group");
    assert!(g.name == "g" && g.translation_x == 2.0 && g.rotation == 10.0);
    let clip = g.iter().find_map(|n| n.group()).expect("the clip path's group");
    assert!(clip.name == "clip" && !clip.clip_path_data.is_empty());
    let names: Vec<&str> = clip.iter().filter_map(|n| n.path()).map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["bar", "grad"]);
    let grad = clip.iter().filter_map(|n| n.path()).find(|p| p.name == "grad").unwrap();
    assert!(grad.fill.is_some() && grad.stroke.is_some() && grad.stroke_line_width == 2.0);
    assert_eq!(root.iter().filter_map(|n| n.path()).map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["after"]);
    assert_eq!(painters[2].intrinsic_size(), Size::new(60.0, 40.0));
    assert!(painters[2].as_any().downcast_ref::<SvgPainter>().is_some());
}

/// The byte decoders: a bitmap, a vector at a density (px sizes through it), malformed XML an error.
fn bytes_decode_into_images() {
    let png = run_blocking(get_drawable_resource_bytes(&english(), &Res::drawable::x())).unwrap();
    let image = decode_to_image_bitmap(&png).unwrap();
    assert_eq!((image.width(), image.height()), (8, 4));
    assert_eq!(pixel(&image, 0, 0), [255, 0, 0, 255]);
    assert!(decode_to_image_bitmap(b"not an image").is_err());
    let v = decode_to_image_vector(
        br#"<vector xmlns:android="http://schemas.android.com/apk/res/android" android:width="48px" android:height="10dp" android:viewportWidth="1" android:viewportHeight="1"/>"#,
        Density { density: 2.0, font_scale: 1.0 },
    )
    .unwrap();
    assert!(v.default_width().0 == 24.0 && v.default_height().0 == 10.0);
    assert!(matches!(decode_to_image_vector(b"<vector", Density::default()), Err(VectorParseError::Xml(_))));
    let bad = br#"<vector xmlns:android="http://schemas.android.com/apk/res/android" android:width="1in"/>"#;
    assert!(matches!(decode_to_image_vector(bad, Density::default()), Err(VectorParseError::Value(_))));
}

// ---- Fonts --------------------------------------------------------------------------------------------------------

/// A font resource's font: its bytes, its weight and style, read once for them.
fn fonts_are_read_once_for_their_weight_and_style() {
    ResourceCaches::clear_blocking();
    let (reader, reads) = counting();
    let seen: Rc<RefCell<Vec<kompoz_text::Font>>> = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(&[LocalResourceReader.provides(reader.clone())], || {
            let mut s = s.borrow_mut();
            s.push(font_resource(Res::font::regular()));
            s.push(font_resource(Res::font::regular(), weight = kompoz_text::FontWeight::Bold));
        });
    }));
    rule.wait_for_idle();
    let fonts = seen.borrow();
    assert_eq!(fonts[0].weight, kompoz_text::FontWeight::Normal);
    assert_eq!(fonts[1].weight, kompoz_text::FontWeight::Bold);
    match &fonts[0].source {
        FontSource::Bytes { identity, data, .. } => {
            assert!(identity.starts_with(&format!("{MD}font/regular.ttf:")), "{identity}");
            assert!(data.len() > 1000);
        }
        other => panic!("{other:?}"),
    }
    assert_ne!(fonts[0], fonts[1]);
    assert_eq!(reads.borrow().len(), 2); // (one for each weight)
    let bytes = run_blocking(get_font_resource_bytes(&english(), &Res::font::regular())).unwrap();
    assert!(bytes.starts_with(&[0, 1, 0, 0]));
}

// ---- Readers, environments ----------------------------------------------------------------------------------------

/// A provided environment (CMP's tests' LocalComposeEnvironment) decides, whatever the composition's locale.
fn a_provided_environment_decides() {
    let fixed = ComposeEnvironment::fixed(env("pt", "BR", true, DensityQualifier::Xxhdpi));
    let seen: Rc<RefCell<(String, ResourceEnvironment)>> = Rc::new(RefCell::new((String::new(), english())));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(&[LocalComposeEnvironment.provides(fixed.clone())], || {
            *s.borrow_mut() = (string_resource(Res::string::app_name()), remember_resource_environment());
        });
    }));
    let (text, environment) = seen.borrow().clone();
    assert_eq!(text, "Kompoz BR");
    assert_eq!(environment.theme, ThemeQualifier::Dark);
    // The default environment: the composition's.
    let e = in_language("tr-TR", remember_resource_environment);
    assert_eq!(e, env("tr", "TR", false, DensityQualifier::Mdpi));
}

fn main() {
    set_ui_thread();
    let tests: &[(&str, fn())] = &[
        ("image_resource_loads_once", image_resource_loads_once),
        ("image_assets_are_decoded_once_wherever_they_are_used", image_assets_are_decoded_once_wherever_they_are_used),
        ("the_generated_res_has_cmps_accessors", the_generated_res_has_cmps_accessors),
        ("values_are_converted_into_cmps_records", values_are_converted_into_cmps_records),
        ("items_are_picked_as_android_picks_them", items_are_picked_as_android_picks_them),
        ("strings_follow_the_ui_language", strings_follow_the_ui_language),
        ("strings_are_formatted_and_pluralized", strings_are_formatted_and_pluralized),
        ("strings_load_outside_a_composition", strings_load_outside_a_composition),
        ("plural_rules_are_cldrs", plural_rules_are_cldrs),
        ("images_are_scaled_to_the_screens_density", images_are_scaled_to_the_screens_density),
        ("images_follow_the_theme", images_follow_the_theme),
        ("painters_are_of_the_files_kind", painters_are_of_the_files_kind),
        ("bytes_decode_into_images", bytes_decode_into_images),
        ("fonts_are_read_once_for_their_weight_and_style", fonts_are_read_once_for_their_weight_and_style),
        ("a_provided_environment_decides", a_provided_environment_decides),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{} failed", failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}
