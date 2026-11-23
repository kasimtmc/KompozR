//! AsyncImage as Coil's compose layer gives it, on KompozR's platform image loader: the C++ Kompoz's AsyncImage tests
//! by name (async_image_loads_away_from_the_ui_thread_and_caches, async_image_painter_paints_the_image_once_loaded,
//! async_image_is_decoded_at_the_size_its_layout_gets, drawing_takes_compose_named_arguments' AsyncImage half),
//! their composable placeholders as `SubcomposeAsyncImage`'s slots (Coil's `AsyncImage` takes painters), and
//! Coil's states, transforms and listeners.

#![allow(non_snake_case)]

use kompoz_coil::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draw::Painter;
use kompoz_ui::platform::{ImageRequestSize, PlatformCallbacks};
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

type Done = Box<dyn FnOnce(Option<ImageBitmap>) + Send>;

/// The host's image loads, kept for the test to finish (the C++ test's `loadImageAsync` callback).
#[derive(Default)]
struct Loads {
    requests: RefCell<Vec<(String, ImageRequestSize, Option<Done>)>>,
}

struct Platform(Rc<Loads>);

impl PlatformCallbacks for Platform {
    fn load_image_async(&self, model: &str, size: ImageRequestSize, done: Done) {
        self.0.requests.borrow_mut().push((model.to_string(), size, Some(done)));
    }
}

impl Loads {
    fn len(&self) -> usize {
        self.requests.borrow().len()
    }
    fn model(&self, i: usize) -> String {
        self.requests.borrow()[i].0.clone()
    }
    fn size(&self, i: usize) -> ImageRequestSize {
        self.requests.borrow()[i].1
    }
    fn done(&self, i: usize) -> Done {
        self.requests.borrow_mut()[i].2.take().expect("not done yet")
    }
}

fn install(rule: &KompozTestRule) -> Rc<Loads> {
    let loads = Rc::new(Loads::default());
    rule.host().set_callbacks(Rc::new(Platform(loads.clone())));
    loads
}

fn photo(w: i32, h: i32) -> ImageBitmap {
    ImageBitmap::from_pixels(w, h, vec![200; (w * h * 4) as usize]).unwrap()
}

fn near_rect(r: Rect, l: f32, t: f32, w: f32, h: f32) -> bool {
    (r.left - l).abs() < 0.5 && (r.top - t).abs() < 0.5 && (r.width() - w).abs() < 0.5 && (r.height() - h).abs() < 0.5
}

fn texts(rule: &KompozTestRule, text: &str) -> usize {
    rule.on_all_nodes_with_text(text).len()
}

/// A slot composing a text (the C++ test's composable placeholder and error).
fn text_slot(text: &'static str) -> Option<SubcomposeSlot> {
    Some(Rc::new(move |_: &SubcomposeAsyncImageScope, _: &AsyncImagePainterState| {
        kompozable_block!({
            BasicText(text);
        });
    }))
}

/// C++ async_image_loads_away_from_the_ui_thread_and_caches: loaded away from the UI thread (finished from another
/// thread, it shows with the next frame), one load per model, cross-faded, cropped to its layout, described; shown
/// again from the memory cache at once (no cross-fade), the failed one tried again. Coil's: the loading slot goes
/// as the image succeeds (C++ kept its placeholder under the fading image); a cropped image is drawn whole,
/// centred and clipped to the layout (C++ drew the cropped part of it).
fn async_image_loads_away_from_the_ui_thread_and_caches() {
    clear_image_cache();
    let shown = mutable_state_of(false);
    let sources: Rc<RefCell<Vec<DataSource>>> = Rc::new(RefCell::new(Vec::new()));
    let (sh, src) = (shown.clone(), sources.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            if !sh.get() {
                return;
            }
            let src = src.clone();
            Column(
                content = move || {
                    let request = ImageRequest::Builder(&PlatformContext::default())
                        .data("images/a.jpg")
                        .crossfade_millis(200)
                        .build();
                    let on_success: StateListener = {
                        let src = src.clone();
                        Rc::new(move |s: &AsyncImagePainterState| {
                            if let AsyncImagePainterState::Success { result, .. } = s {
                                src.borrow_mut().push(result.data_source);
                            }
                        })
                    };
                    SubcomposeAsyncImage(
                        request.clone(),
                        Some("foto".into()),
                        modifier = Modifier.size_wh(100.dp(), 60.dp()).test_tag("a"),
                        loading = text_slot("yükleniyor"),
                        error = text_slot("hata"),
                        on_success = Some(on_success),
                        content_scale = ContentScale::Crop,
                    );
                    // the same model: one load
                    SubcomposeAsyncImage(
                        request,
                        Some("foto 2".into()),
                        modifier = Modifier.size_wh(50.dp(), 50.dp()),
                        loading = text_slot("yükleniyor"),
                        error = text_slot("hata"),
                        content_scale = ContentScale::Crop,
                    );
                    let failing = ImageRequest::Builder(&PlatformContext::default())
                        .data("https://example.com/yok.webp")
                        .crossfade_millis(200)
                        .build();
                    SubcomposeAsyncImage(
                        failing,
                        Some("yok".into()),
                        modifier = Modifier.size_wh(80.dp(), 40.dp()).test_tag("b"),
                        loading = text_slot("yükleniyor"),
                        error = text_slot("hata"),
                        content_scale = ContentScale::Crop,
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let loads = install(&rule);
    shown.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 2);
    assert_eq!(loads.model(0), "images/a.jpg");
    assert_eq!(texts(&rule, "yükleniyor"), 3);
    assert!(near_rect(rule.on_node_with_tag("a").get_unclipped_bounds_in_root(), 0.0, 0.0, 100.0, 60.0));
    let first = loads.done(0);
    let worker = std::thread::spawn(move || first(Some(ImageBitmap::from_pixels(4, 2, vec![255; 32]).unwrap())));
    worker.join().unwrap(); // done from another thread: it reaches the UI thread with the next frame
    (loads.done(1))(None);
    rule.advance_frame();
    // (Coil's cross-fade starts as it is first drawn: nothing of the image yet)
    assert!(rule.draw().images.is_empty());
    rule.advance_time_by(100);

    let mid = rule.draw();
    assert_eq!(mid.images.len(), 2); // cross-fading in
    assert_eq!(texts(&rule, "yükleniyor"), 0);
    assert!(rule.on_node_with_text("hata").exists());
    rule.wait_for_idle();
    let after = rule.draw();
    assert_eq!(after.images.len(), 2);
    // (4 x 2 cropped into 100 x 60: 120 x 60, centred, clipped)
    assert!(near_rect(after.image_destinations[0], -10.0, 0.0, 120.0, 60.0), "{:?}", after.image_destinations[0]);
    assert!(rule.on_node_with_content_description("foto").exists());
    assert_eq!(*sources.borrow(), vec![DataSource::Disk]);
    shown.set(false);
    rule.wait_for_idle();
    shown.set(true); // again: from the cache, at once; the failed one tried again
    rule.wait_for_idle();
    assert_eq!(loads.len(), 3);
    assert_eq!(texts(&rule, "yükleniyor"), 1);
    assert_eq!(rule.draw().images.len(), 2);
    assert_eq!(sources.borrow().last(), Some(&DataSource::MemoryCache)); // (no cross-fade)
}

/// C++ async_image_painter_paints_the_image_once_loaded: nothing to paint (and no size) before; its image's size,
/// drawn, once loaded; loaded at its original size.
fn async_image_painter_paints_the_image_once_loaded() {
    clear_image_cache();
    let shown = mutable_state_of(false);
    let sh = shown.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            if sh.get() {
                let painter: Rc<dyn Painter> = remember_async_image_painter("resim.png");
                Image(painter, Some("resim".into()), modifier = Modifier.test_tag("resim"));
            }
        }),
        IntSize::new(400, 800),
    );
    let loads = install(&rule);
    shown.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 1);
    assert_eq!(loads.size(0), ImageRequestSize::default());
    assert!(rule.draw().images.is_empty());
    (loads.done(0))(Some(photo(30, 20)));
    rule.wait_for_idle();
    let r = rule.on_node_with_tag("resim").get_unclipped_bounds_in_root();
    assert!(near_rect(r, 0.0, 0.0, 30.0, 20.0), "{r:?}");
    assert_eq!(rule.draw().images.len(), 1);
}

/// C++ async_image_is_decoded_at_the_size_its_layout_gets: decoded for its box in pixels (cropped: to cover it);
/// told each state (no loader at first: an error); a bigger one of the same photo shows the small one at once
/// (Coil's placeholder memory cache key: KompozR offers the model's cached image) and loads for its width; the
/// big one in the cache does for both; the painter loads the whole image (the cached one its placeholder); two
/// sizes asked at once: one load, who needs more loading another. Coil sizes the layout by the painter's image as
/// it is (the small one, then the big one), where C++ kept the photo's shape from the first.
fn async_image_is_decoded_at_the_size_its_layout_gets() {
    clear_image_cache();
    let (big, small, whole) = (mutable_state_of(false), mutable_state_of(true), mutable_state_of(false));
    let states: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let widths: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let (b, s, w, st, wi) = (big.clone(), small.clone(), whole.clone(), states.clone(), widths.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (b, s, w, st, wi) = (b.clone(), s.clone(), w.clone(), st.clone(), wi.clone());
            let scroll = remember_scroll_state();
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&scroll),
                content = move || {
                    if s.get() {
                        let log = |name: &'static str| -> Option<StateListener> {
                            let st = st.clone();
                            Some(Rc::new(move |state: &AsyncImagePainterState| {
                                let width = state.painter().map_or(0.0, |p| p.intrinsic_size().width);
                                st.borrow_mut().push(format!("{name} {width}"));
                            }))
                        };
                        AsyncImage(
                            "photo.jpg",
                            Some("küçük".into()),
                            modifier = Modifier.size_wh(100.dp(), 60.dp()).test_tag("small"),
                            on_loading = log("loading"),
                            on_success = log("success"),
                            on_error = log("error"),
                            content_scale = ContentScale::Crop,
                        );
                    }
                    // Fitted to the width in a column that scrolls: only the width counts.
                    if b.get() {
                        AsyncImage(
                            "photo.jpg",
                            Some("büyük".into()),
                            modifier = Modifier.fill_max_width().test_tag("big"),
                        );
                    }
                    if w.get() {
                        let painter = remember_async_image_painter("photo.jpg");
                        let width = painter.state().painter().map_or(0.0, |p| p.intrinsic_size().width);
                        wi.borrow_mut().push(width as i32);
                    }
                },
            );
        }),
        IntSize::new(800, 1600),
    );
    // (the C++ test's 400 x 800 dp at density 2)
    rule.host().set_density(Density { density: 2.0, font_scale: 1.0 });
    let loads = install(&rule);
    small.set(false);
    rule.wait_for_idle();
    small.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 1);
    assert_eq!(loads.size(0), ImageRequestSize { width: 200, height: 120, fill: true }); // its box, in pixels
    (loads.done(0))(Some(photo(200, 150))); // a 4:3 photo decoded to cover it
    rule.wait_for_idle();
    // (bounds in px: density 2)
    assert!(near_rect(rule.on_node_with_tag("small").get_unclipped_bounds_in_root(), 0.0, 0.0, 200.0, 120.0));
    // Told each change: without a loader at first (an error), then loading again, then loaded.
    assert_eq!(*states.borrow(), vec!["loading 0", "error 0", "loading 0", "success 200"]);
    // A bigger one of the same photo: the small one at once, then one decoded for its width.
    big.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 2);
    assert_eq!(loads.size(1), ImageRequestSize { width: 800, height: 0, fill: false });
    let r = rule.on_node_with_tag("big").get_unclipped_bounds_in_root();
    assert!(near_rect(r, 0.0, 120.0, 800.0, 150.0), "{r:?}"); // (the small one's shape, as Coil sizes by it)
    let before = rule.draw();
    assert_eq!(before.images.len(), 2);
    assert_eq!(before.images[0], before.images[1]);
    (loads.done(1))(Some(photo(800, 600)));
    rule.wait_for_idle();
    let after = rule.draw();
    assert_eq!(after.images.len(), 2);
    assert_ne!(after.images[0], after.images[1]);
    let r = rule.on_node_with_tag("big").get_unclipped_bounds_in_root();
    assert!(near_rect(r, 0.0, 120.0, 800.0, 600.0), "{r:?}"); // the photo's shape at its width
    // Both again: the big one in the cache does for both.
    small.set(false);
    big.set(false);
    rule.wait_for_idle();
    small.set(true);
    big.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 2);
    // The whole image (the painter's original size): the 800 one isn't, so it is loaded; meanwhile the cached one
    // is its placeholder; the decoder gives back what it has.
    whole.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 3);
    assert_eq!(loads.size(2), ImageRequestSize::default());
    assert_eq!(widths.borrow().last(), Some(&800));
    (loads.done(2))(Some(photo(1600, 1200)));
    rule.wait_for_idle();
    assert_eq!(widths.borrow().last(), Some(&1600));
    // Two sizes asked for at once: one load; who needs more than it brought gets another.
    clear_image_cache();
    small.set(false);
    big.set(false);
    whole.set(false);
    rule.wait_for_idle();
    small.set(true);
    big.set(true);
    rule.wait_for_idle();
    assert_eq!(loads.len(), 4);
    assert_eq!(loads.size(3), ImageRequestSize { width: 200, height: 120, fill: true });
    (loads.done(3))(Some(photo(200, 150)));
    rule.wait_for_idle();
    assert_eq!(loads.len(), 5);
    assert_eq!(loads.size(4), ImageRequestSize { width: 800, height: 0, fill: false });
    (loads.done(4))(Some(photo(100, 75))); // smaller than asked: the whole image, nothing more to ask for
    rule.wait_for_idle();
    assert_eq!(loads.len(), 5);
    let last = rule.draw();
    assert_eq!(last.images.len(), 2);
    assert_ne!(last.images[0], last.images[1]);
}

/// C++ drawing_takes_compose_named_arguments' AsyncImage half: `SubcomposeAsyncImage(model = …, contentDescription
/// = …, modifier = …, loading = { }, error = { })` by name (Coil's composable slots; C++'s `placeholder`/`error`
/// lambdas), without a loader: loading, then the error slot.
fn drawing_takes_compose_named_arguments() {
    clear_image_cache();
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        SubcomposeAsyncImage(
            model = "yok.png",
            content_description = Some("resim".into()),
            modifier = Modifier.size(40.dp()),
            loading = text_slot("yükleniyor"),
            error = text_slot("hata"),
        );
    }));
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("yükleniyor").exists() || rule.on_node_with_text("hata").exists());
}

/// Coil's placeholder, error and fallback painters (AsyncImage's `transformOf`): the placeholder while loading,
/// the error painter on failure, the fallback for no data; `onState` told each.
fn placeholders_errors_and_fallbacks_are_painters() {
    clear_image_cache();
    let model = mutable_state_of(Some("a.png".to_string()));
    let told: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (m, t) = (model.clone(), told.clone());
    let red: Rc<dyn Painter> = Rc::new(kompoz_ui::draw::ColorPainter { color: Color::Red });
    let blue: Rc<dyn Painter> = Rc::new(kompoz_ui::draw::ColorPainter { color: Color::Blue });
    let green: Rc<dyn Painter> = Rc::new(kompoz_ui::draw::ColorPainter { color: Color::Green });
    let painters = (red.clone(), blue.clone(), green.clone());
    let seen: Rc<RefCell<Option<Rc<AsyncImagePainter>>>> = Rc::new(RefCell::new(None));
    let sn = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = t.clone();
        let on_state: StateListener = Rc::new(move |s: &AsyncImagePainterState| t.borrow_mut().push(format!("{s:?}")));
        let painter = remember_async_image_painter(
            m.get(),
            placeholder = Some(painters.0.clone()),
            error = Some(painters.1.clone()),
            fallback = Some(painters.2.clone()),
            on_loading = Some(on_state.clone()),
            on_error = Some(on_state),
        );
        *sn.borrow_mut() = Some(painter);
    }));
    let loads = install(&rule);
    model.set(Some("b.png".into()));
    rule.wait_for_idle();
    let painter = || seen.borrow().clone().unwrap();
    assert!(matches!(painter().state(), AsyncImagePainterState::Loading { painter: Some(p) } if Rc::ptr_eq(&p, &red)));
    (loads.done(0))(None);
    rule.wait_for_idle();
    assert!(
        matches!(painter().state(), AsyncImagePainterState::Error { painter: Some(p), .. } if Rc::ptr_eq(&p, &blue))
    );
    model.set(None);
    rule.wait_for_idle();
    let AsyncImagePainterState::Error { painter: Some(p), result } = painter().state() else { panic!("an error") };
    assert!(Rc::ptr_eq(&p, &green) && result.throwable == ImageError::NullRequestData);
    assert!(
        told.borrow().iter().any(|s| s.starts_with("Loading")) && told.borrow().iter().any(|s| s.starts_with("Error"))
    );
}

fn main() {
    set_ui_thread();
    let tests: &[(&str, fn())] = &[
        ("async_image_loads_away_from_the_ui_thread_and_caches", async_image_loads_away_from_the_ui_thread_and_caches),
        ("async_image_painter_paints_the_image_once_loaded", async_image_painter_paints_the_image_once_loaded),
        ("async_image_is_decoded_at_the_size_its_layout_gets", async_image_is_decoded_at_the_size_its_layout_gets),
        ("drawing_takes_compose_named_arguments", drawing_takes_compose_named_arguments),
        ("placeholders_errors_and_fallbacks_are_painters", placeholders_errors_and_fallbacks_are_painters),
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
