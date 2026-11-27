//! The font family resolver over a [`FontCollection`] (Compose's FontFamilyResolverImpl with its
//! FontListFontFamilyTypefaceAdapter and PlatformFontFamilyTypefaceAdapter): the system's generic families as the
//! collection has them, families of one's own matched as CSS does with their fonts loaded by their strategy
//! (blocking, optional local, async), device fonts by name, files, assets and bytes; what it resolves shaped by
//! the [`HarfrustBackend`] over the same collection.

use super::collection::{FontCollection, load_font_file};
use super::family::{FontFamily, FontListFontFamily, GenericFontFamily};
use super::font::{Font, FontLoadingStrategy, FontSource};
use super::matcher::FontMatcher;
use super::registry::{FontBytes, FontId, FontRegistry};
use super::resolver::{FontFamilyResolver, FontLoadError, Typeface, fixed_state};
use super::synthesis::{FontStyle, FontSynthesis};
use super::variation::{FontVariation, FontVariationSettings};
use super::weight::FontWeight;
use crate::draw::GlyphSynthesis;
use crate::shape::{FontBackend, HarfrustBackend};
use kompoz_runtime::clock::Instant;
use kompoz_runtime::{MutableState, State, mutable_state_of};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

/// Reads an asset's bytes by its path (the platform's: an APK's assets, files next to the binary).
pub type AssetLoader = Rc<dyn Fn(&str) -> Option<FontBytes>>;

/// Loads a font away from the UI thread (a file or an asset; a download): calls `done` with its bytes, or None if
/// it can't be loaded, from any thread (Compose's PlatformFontLoader.awaitLoad).
pub type AsyncFontLoader = Rc<dyn Fn(&Font, Box<dyn FnOnce(Option<FontBytes>) + Send>)>;

/// A font loaded: its id, and the settings its axes take (a device family's variable face for the weight asked).
#[derive(Clone, PartialEq, Debug)]
struct LoadedFont {
    font: FontId,
    variations: Vec<([u8; 4], f32)>,
}

type Waiter = Box<dyn FnOnce(Option<LoadedFont>) + Send>;

/// The fonts loaded and loading (Compose's AsyncTypefaceCache): a font that failed stays failed; what waits for a
/// font being loaded is told when it is.
#[derive(Default)]
struct LoadedFonts {
    results: HashMap<Font, Option<LoadedFont>>,
    pending: HashMap<Font, Vec<Waiter>>,
}

/// What a family, weight, style and synthesis were resolved to (Compose's TypefaceRequest).
#[derive(Clone, PartialEq, Eq, Hash)]
struct TypefaceRequest {
    family: Option<FontFamily>,
    weight: FontWeight,
    style: FontStyle,
    synthesis: FontSynthesis,
}

/// A face nothing has: what a collection without fonts resolves to (every character its missing glyph).
static NO_FONT: LazyLock<FontId> = LazyLock::new(|| FontRegistry::register(Arc::from(Vec::<u8>::new()), 0));

/// Resolves families to faces of a [`FontCollection`] (Compose's FontFamily.Resolver as `createFontFamilyResolver`
/// makes it), shaping through a [`HarfrustBackend`] over it:
/// - the default and the generic families: the collection's generic family's first face for the weight and
///   style (its axes set for them), faked where it lacks them as [`FontSynthesis`] allows;
/// - a family of one's own: its fonts matched as CSS does ([`FontMatcher`]), the first available taken as
///   Compose's resolver takes it — a `Blocking` font loaded now (the default family if it fails), an
///   `OptionalLocal` one if it loads now, an `Async` one if it has loaded, else loaded in the background while the
///   text is laid out in the default family, the typeface's state then changing to it (a load taking longer than
///   [`Font::MaximumAsyncTimeoutMillis`] fails); faked as Compose fakes it;
/// - a typeface loaded already: itself.
///
/// Fonts come from files (mapped), assets (through the asset loader), bytes, or the collection's families by name
/// (device fonts); async ones through the async loader if one is set, else files on a background thread and
/// the rest at once. Results are cached: a font loads once, a request resolves once.
pub struct FontFamilyResolverImpl {
    collection: Rc<FontCollection>,
    backend: Rc<HarfrustBackend>,
    asset_loader: RefCell<Option<AssetLoader>>,
    async_loader: RefCell<Option<AsyncFontLoader>>,
    typefaces: RefCell<HashMap<TypefaceRequest, State<Typeface>>>,
    loaded: Arc<Mutex<LoadedFonts>>,
}

thread_local! {
    static SYSTEM: Rc<FontFamilyResolverImpl> = Rc::new(FontFamilyResolverImpl::new(Rc::new(FontCollection::system())));
}

impl FontFamilyResolverImpl {
    /// A resolver over `collection`.
    pub fn new(collection: Rc<FontCollection>) -> FontFamilyResolverImpl {
        FontFamilyResolverImpl {
            backend: Rc::new(HarfrustBackend::new(collection.clone())),
            collection,
            asset_loader: RefCell::new(None),
            async_loader: RefCell::new(None),
            typefaces: RefCell::new(HashMap::new()),
            loaded: Arc::new(Mutex::new(LoadedFonts::default())),
        }
    }

    /// The thread's resolver over the system's fonts (fontconfig's on Linux, through fontique; scanned when text
    /// is first resolved): what a host lays text out with unless told otherwise (Compose's
    /// `createFontFamilyResolver(context)`).
    pub fn system() -> Rc<FontFamilyResolverImpl> {
        SYSTEM.with(Rc::clone)
    }

    /// A resolver over a collection of only the fonts registered with it (`collection().register(…)`): what
    /// tests use, so nothing depends on the machine.
    pub fn registered_only() -> FontFamilyResolverImpl {
        FontFamilyResolverImpl::new(Rc::new(FontCollection::registered_only()))
    }

    /// Reads asset fonts through `loader`.
    pub fn set_asset_loader(&self, loader: AssetLoader) {
        *self.asset_loader.borrow_mut() = Some(loader);
    }

    /// Loads async fonts (files and assets) through `loader` instead of reading files on a background thread and
    /// assets at once.
    pub fn set_async_loader(&self, loader: AsyncFontLoader) {
        *self.async_loader.borrow_mut() = Some(loader);
    }

    /// The collection its faces come from.
    pub fn collection(&self) -> &Rc<FontCollection> {
        &self.collection
    }

    /// Its backend.
    pub fn harfrust_backend(&self) -> &Rc<HarfrustBackend> {
        &self.backend
    }

    /// The face a family, weight, style and synthesis take now (what `resolve`'s state holds at first).
    pub fn typeface(
        &self,
        family: Option<&FontFamily>,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> Typeface {
        self.resolve(family, weight, style, synthesis).peek()
    }

    /// A generic family's face (the default family's: sans-serif's) for a weight and style.
    fn generic(
        &self,
        generic: GenericFontFamily,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> Typeface {
        let face = self
            .collection
            .generic_families(generic)
            .into_iter()
            .find_map(|family| self.collection.face(family, weight, style));
        let Some(face) = face else {
            return Typeface {
                font: *NO_FONT,
                variation_settings: FontVariationSettings::Empty,
                synthesis: GlyphSynthesis::None,
                weight,
                style,
                fallback: generic,
            };
        };
        let actual_weight = FontWeight::new((face.effective_weight().round() as i32).clamp(1, 1000));
        let actual_style = if face.effective_italic() { FontStyle::Italic } else { FontStyle::Normal };
        Typeface {
            font: face.font,
            variation_settings: settings_of(&face.variations),
            synthesis: synthesize(synthesis, weight, actual_weight, style, actual_style),
            weight,
            style,
            fallback: generic,
        }
    }

    /// The default family's face for a request (Compose's createDefaultTypeface).
    fn default_typeface(&self, weight: FontWeight, style: FontStyle, synthesis: FontSynthesis) -> Typeface {
        self.generic(GenericFontFamily::Default, weight, style, synthesis)
    }

    /// A font loaded now, from cache if it was (a failure is remembered).
    fn load_blocking(&self, font: &Font) -> Option<LoadedFont> {
        if let Some(r) = self.loaded.lock().expect("loaded fonts").results.get(font) {
            return r.clone();
        }
        let result = match &font.source {
            FontSource::File(path) => load_font_file(path).map(|b| loaded(b, 0)),
            FontSource::Asset(path) => {
                let loader = self.asset_loader.borrow().clone();
                loader.and_then(|l| l(path)).map(|b| loaded(b, 0))
            }
            FontSource::Bytes { data, index, .. } => Some(loaded(Arc::new(data.clone()), *index)),
            FontSource::Device(name) => self.device(name.name(), font),
        };
        self.loaded.lock().expect("loaded fonts").results.insert(font.clone(), result.clone());
        result
    }

    /// A device family's face for a font's weight and style.
    fn device(&self, name: &str, font: &Font) -> Option<LoadedFont> {
        let face = self.collection.face_by_name(name, font.weight, font.style)?;
        Some(LoadedFont { font: face.font, variations: face.variations })
    }

    /// Loads a font in the background (or at once, if it can be), telling `waiter` how it went (from any thread).
    fn load_async(&self, font: &Font, waiter: Waiter) {
        {
            let mut l = self.loaded.lock().expect("loaded fonts");
            if let Some(r) = l.results.get(font).cloned() {
                drop(l);
                waiter(r);
                return;
            }
            if let Some(waiting) = l.pending.get_mut(font) {
                waiting.push(waiter);
                return;
            }
            l.pending.insert(font.clone(), vec![waiter]);
        }
        let shared = self.loaded.clone();
        let key = font.clone();
        let finish = move |result: Option<LoadedFont>| {
            let waiters = {
                let mut l = shared.lock().expect("loaded fonts");
                l.results.insert(key.clone(), result.clone());
                l.pending.remove(&key).unwrap_or_default()
            };
            for w in waiters {
                w(result.clone());
            }
        };
        match &font.source {
            FontSource::Device(name) => finish(self.device(name.name(), font)),
            FontSource::Bytes { data, index, .. } => finish(Some(loaded(Arc::new(data.clone()), *index))),
            FontSource::File(_) | FontSource::Asset(_) => {
                let started = Instant::now();
                let timeout = Duration::from_millis(Font::MaximumAsyncTimeoutMillis as u64);
                let done: Box<dyn FnOnce(Option<FontBytes>) + Send> = Box::new(move |bytes| {
                    // A load that took longer than Compose waits failed.
                    let bytes = bytes.filter(|_| started.elapsed() <= timeout);
                    finish(bytes.map(|b| loaded(b, 0)))
                });
                let async_loader = self.async_loader.borrow().clone();
                match (async_loader, &font.source) {
                    (Some(loader), _) => loader(font, done),
                    (None, FontSource::File(path)) => {
                        let path = path.clone();
                        kompoz_coroutines::run_in_background(move || done(load_font_file(&path)));
                    }
                    (None, FontSource::Asset(path)) => {
                        let loader = self.asset_loader.borrow().clone();
                        done(loader.and_then(|l| l(path)))
                    }
                    (None, _) => done(None),
                }
            }
        }
    }

    /// A family of one's own resolved (Compose's FontListFontFamilyTypefaceAdapter.resolve).
    fn resolve_list(
        &self,
        family: &FontListFontFamily,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> State<Typeface> {
        let matched = FontMatcher.match_family(family, weight, style);
        let (asynchronous, typeface) = self.first_immediately_available(&matched, weight, style, synthesis);
        if asynchronous.is_empty() {
            return fixed_state(typeface);
        }
        let list = Arc::new(Mutex::new(AsyncList {
            results: vec![None; asynchronous.len()],
            fonts: asynchronous.clone(),
            state: None,
            weight,
            style,
            synthesis,
        }));
        for (i, font) in asynchronous.iter().enumerate() {
            let list = list.clone();
            self.load_async(
                font,
                Box::new(move |result| {
                    let (state, pick) = {
                        let mut l = list.lock().expect("async fonts");
                        l.results[i] = Some(result);
                        (l.state.clone(), l.pick())
                    };
                    if let (Some(state), Some(typeface)) = (state, pick) {
                        state.set(typeface);
                    }
                }),
            );
        }
        let mut l = list.lock().expect("async fonts");
        let state = mutable_state_of(l.pick().unwrap_or(typeface));
        l.state = Some(state.clone());
        State::from(state)
    }

    /// The first of the matched fonts available now (Compose's firstImmediatelyAvailable): the async fonts before
    /// it that are yet to load, and the face (the default family's if none is).
    fn first_immediately_available(
        &self,
        matched: &[Font],
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> (Vec<Font>, Typeface) {
        let mut asynchronous = Vec::new();
        for font in matched {
            match font.loading_strategy {
                FontLoadingStrategy::Blocking => {
                    let typeface = match self.load_blocking(font) {
                        Some(l) => typeface_of(&l, font, weight, style, synthesis),
                        None => self.default_typeface(weight, style, synthesis),
                    };
                    return (asynchronous, typeface);
                }
                FontLoadingStrategy::OptionalLocal => {
                    if let Some(l) = self.load_blocking(font) {
                        return (asynchronous, typeface_of(&l, font, weight, style, synthesis));
                    }
                }
                FontLoadingStrategy::Async => {
                    let cached = self.loaded.lock().expect("loaded fonts").results.get(font).cloned();
                    match cached {
                        None => asynchronous.push(font.clone()),
                        // Failed for good.
                        Some(None) => {}
                        Some(Some(l)) => return (asynchronous, typeface_of(&l, font, weight, style, synthesis)),
                    }
                }
            }
        }
        (asynchronous, self.default_typeface(weight, style, synthesis))
    }
}

/// An async list's loads (Compose's AsyncFontListLoader, its fonts loaded together rather than one after
/// another, the first in order that loads taken once those before it have failed).
struct AsyncList {
    fonts: Vec<Font>,
    results: Vec<Option<Option<LoadedFont>>>,
    state: Option<MutableState<Typeface>>,
    weight: FontWeight,
    style: FontStyle,
    synthesis: FontSynthesis,
}

impl AsyncList {
    /// The face to show: the first font that loaded, if every one before it failed; None while one before it is
    /// loading, or if all failed (the default family's stays).
    fn pick(&self) -> Option<Typeface> {
        for (font, result) in self.fonts.iter().zip(&self.results) {
            match result {
                None => return None,
                Some(None) => continue,
                Some(Some(l)) => return Some(typeface_of(l, font, self.weight, self.style, self.synthesis)),
            }
        }
        None
    }
}

fn loaded(bytes: FontBytes, index: u32) -> LoadedFont {
    LoadedFont { font: FontRegistry::register_bytes(bytes, index), variations: Vec::new() }
}

/// Settings of axes as a font's variation settings.
fn settings_of(variations: &[([u8; 4], f32)]) -> FontVariationSettings {
    if variations.is_empty() {
        return FontVariationSettings::Empty;
    }
    FontVariationSettings::new(
        variations.iter().filter_map(|(t, v)| std::str::from_utf8(t).ok().map(|n| FontVariation::setting(n, *v))),
    )
}

/// What is faked of a face of `actual_weight` and `actual_style` for the weight and style asked (Compose's
/// FontSynthesis.synthesizeTypeface, with minikin's rule for weight).
fn synthesize(
    synthesis: FontSynthesis,
    weight: FontWeight,
    actual_weight: FontWeight,
    style: FontStyle,
    actual_style: FontStyle,
) -> GlyphSynthesis {
    GlyphSynthesis {
        embolden: synthesis.synthesizes_weight(weight, actual_weight),
        skew: if synthesis.synthesizes_style(style, actual_style) { GlyphSynthesis::ITALIC_SKEW } else { 0.0 },
    }
}

/// The face of a loaded font of a family of one's own: its settings (the font's own, else those its device family
/// set for the weight), faked by the weight and style it is declared as.
fn typeface_of(
    loaded: &LoadedFont,
    font: &Font,
    weight: FontWeight,
    style: FontStyle,
    synthesis: FontSynthesis,
) -> Typeface {
    Typeface {
        font: loaded.font,
        variation_settings: if font.variation_settings.is_empty() {
            settings_of(&loaded.variations)
        } else {
            font.variation_settings.clone()
        },
        synthesis: synthesize(synthesis, weight, font.weight, style, font.style),
        weight,
        style,
        fallback: GenericFontFamily::Default,
    }
}

impl FontFamilyResolver for FontFamilyResolverImpl {
    fn preload(&self, family: &FontFamily) -> Pin<Box<dyn Future<Output = Result<(), FontLoadError>>>> {
        let FontFamily::List(list) = family else {
            return Box::pin(async { Ok(()) });
        };
        // The async fonts each style an async font is for takes first (those after an available font are never
        // needed).
        let mut styles: Vec<(FontWeight, FontStyle)> = Vec::new();
        for f in list.fonts() {
            if f.loading_strategy == FontLoadingStrategy::Async && !styles.contains(&(f.weight, f.style)) {
                styles.push((f.weight, f.style));
            }
        }
        let mut to_load: Vec<Font> = Vec::new();
        for (weight, style) in styles {
            let matched = FontMatcher.match_family(list, weight, style);
            let (asynchronous, _) = self.first_immediately_available(&matched, weight, style, FontSynthesis::All);
            if let Some(first) = asynchronous.into_iter().next()
                && !to_load.contains(&first)
            {
                to_load.push(first);
            }
        }
        let wait = Arc::new(Mutex::new(Preload { remaining: to_load.len(), error: None, waker: None }));
        for font in to_load {
            let wait = wait.clone();
            let f = font.clone();
            self.load_async(
                &font,
                Box::new(move |result| {
                    let waker = {
                        let mut w = wait.lock().expect("preload");
                        w.remaining -= 1;
                        if result.is_none() && w.error.is_none() {
                            w.error = Some(FontLoadError { font: Some(f), message: "Unable to load font".to_string() });
                        }
                        w.waker.take()
                    };
                    if let Some(waker) = waker {
                        waker.wake();
                    }
                }),
            );
        }
        Box::pin(PreloadFuture { wait })
    }

    fn resolve(
        &self,
        family: Option<&FontFamily>,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> State<Typeface> {
        let request = TypefaceRequest { family: family.cloned(), weight, style, synthesis };
        if let Some(state) = self.typefaces.borrow().get(&request) {
            return state.clone();
        }
        let state = match family {
            None => fixed_state(self.default_typeface(weight, style, synthesis)),
            Some(FontFamily::Loaded(t)) => fixed_state(t.clone()),
            Some(FontFamily::List(list)) => self.resolve_list(list, weight, style, synthesis),
            Some(generic) => {
                fixed_state(self.generic(generic.as_generic().unwrap_or_default(), weight, style, synthesis))
            }
        };
        self.typefaces.borrow_mut().insert(request, state.clone());
        state
    }

    fn backend(&self) -> Rc<dyn FontBackend> {
        self.backend.clone()
    }
}

/// Its collection.
impl fmt::Debug for FontFamilyResolverImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FontFamilyResolverImpl({:?})", self.collection)
    }
}

struct Preload {
    remaining: usize,
    error: Option<FontLoadError>,
    waker: Option<Waker>,
}

/// Done when every font preloaded has loaded or one has failed.
struct PreloadFuture {
    wait: Arc<Mutex<Preload>>,
}

impl Future for PreloadFuture {
    type Output = Result<(), FontLoadError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut w = self.wait.lock().expect("preload");
        if let Some(e) = w.error.take() {
            return Poll::Ready(Err(e));
        }
        if w.remaining == 0 {
            return Poll::Ready(Ok(()));
        }
        w.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}
