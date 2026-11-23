//! The image loader (Coil's `ImageLoader`, `RealImageLoader`, `SingletonImageLoader`) on KompozR's platform image
//! loader (C++ Kompoz's `requestImage` and image cache, kompoz-platform's `image_loader`: decoded away from the UI
//! thread at the size asked, downloads in a disk cache): a memory cache of the images loaded, by their data (the
//! biggest one loaded so far, the least recently used going past 64 MB); one load of a data at a time, who waits
//! for a bigger image than it brought loading another; failures not kept (tried again next time).

use crate::request::{
    CoilSize, DataSource, Dimension, ErrorResult, Image, ImageData, ImageError, ImageRequest, ImageResult,
    PlatformContext, Scale, SuccessResult,
};
use kompoz_graphics::{ImageBitmap, decode_image, image_covers};
use kompoz_ui::platform::ImageRequestSize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

// ---- The memory cache --------------------------------------------------------------------------------------------

struct Entry {
    image: ImageBitmap,
    bytes: usize,
    used: u64,
    /// Not decoded smaller: good for any size.
    whole: bool,
}

/// Who waits for a load of the same data (on the UI thread): what it asked for, where the result goes.
struct Waiter {
    slot: Rc<RefCell<WaitSlot>>,
}

#[derive(Default)]
struct WaitSlot {
    /// The load's result, once it ended (none inside: it failed).
    result: Option<Option<ImageBitmap>>,
    waker: Option<Waker>,
}

/// Images by their data's key, and the loads in flight (C++'s `ImageCache`; the UI thread's).
#[derive(Default)]
struct MemoryCache {
    entries: HashMap<String, Entry>,
    loading: HashMap<String, Vec<Waiter>>,
    total: usize,
    clock: u64,
}

/// Coil's memory cache's budget here (the C++ Kompoz's): 64 MB of pixels.
const MEMORY_CACHE_BYTES: usize = 64 << 20;

fn to_platform_size(size: CoilSize, scale: Scale) -> ImageRequestSize {
    let px = |d: Dimension| match d {
        Dimension::Pixels(p) => p.max(0),
        Dimension::Undefined => 0,
    };
    let (width, height) = (px(size.width), px(size.height));
    ImageRequestSize { width, height, fill: scale == Scale::Fill && (width > 0 || height > 0) }
}

fn graphics_size(s: ImageRequestSize) -> kompoz_graphics::ImageRequestSize {
    kompoz_graphics::ImageRequestSize::new(s.width, s.height, s.fill)
}

fn any(s: ImageRequestSize) -> bool {
    s.width <= 0 && s.height <= 0
}

impl MemoryCache {
    /// The data's image if one big enough for `size` is here (none: any).
    fn find(&mut self, key: &str, size: Option<ImageRequestSize>) -> Option<ImageBitmap> {
        self.clock += 1;
        let clock = self.clock;
        let e = self.entries.get_mut(key)?;
        if let Some(size) = size
            && !e.whole
            && !image_covers(e.image.width(), e.image.height(), graphics_size(size))
        {
            return None;
        }
        e.used = clock;
        Some(e.image.clone())
    }

    fn put(&mut self, key: &str, image: &ImageBitmap, whole: bool) {
        self.clock += 1;
        if let Some(old) = self.entries.get_mut(key) {
            if old.whole || (!whole && old.image.width() >= image.width() && old.image.height() >= image.height()) {
                old.used = self.clock; // (a smaller one than what is here)
                return;
            }
            self.total -= old.bytes;
        }
        let bytes = image.width() as usize * image.height() as usize * 4;
        self.entries.insert(key.to_string(), Entry { image: image.clone(), bytes, used: self.clock, whole });
        self.total += bytes;
        while self.total > MEMORY_CACHE_BYTES && self.entries.len() > 1 {
            let oldest = self.entries.iter().min_by_key(|(_, e)| e.used).map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                let e = self.entries.remove(&k).expect("the oldest");
                self.total -= e.bytes;
            }
        }
    }
}

thread_local! {
    static MEMORY_CACHE: RefCell<MemoryCache> = RefCell::new(MemoryCache::default());
}

/// Empties the memory cache of images (the C++ Kompoz's `clearImageCache`; Coil's `imageLoader.memoryCache?.clear()`).
pub fn clear_image_cache() {
    MEMORY_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.entries.clear();
        c.total = 0;
    });
}

/// How many bytes of images the memory cache holds (Coil's `MemoryCache.size`).
pub fn image_cache_size() -> usize {
    MEMORY_CACHE.with(|c| c.borrow().total)
}

// ---- A load's result from another thread --------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    result: Option<Option<ImageBitmap>>,
    waker: Option<Waker>,
}

/// The platform's `done` (called from any thread), awaited on the UI thread.
struct Loaded(Arc<Mutex<Shared>>);

impl Future for Loaded {
    type Output = Option<ImageBitmap>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ImageBitmap>> {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match s.result.take() {
            Some(r) => Poll::Ready(r),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// Waiting for another request's load of the same data.
struct Waiting(Rc<RefCell<WaitSlot>>);

impl Future for Waiting {
    type Output = Option<ImageBitmap>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ImageBitmap>> {
        let mut s = self.0.borrow_mut();
        match s.result.take() {
            Some(r) => Poll::Ready(r),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// `data` loaded by the platform at `size` (bytes decoded on a background thread).
async fn load_from_platform(
    context: &PlatformContext,
    data: &ImageData,
    size: ImageRequestSize,
) -> Option<ImageBitmap> {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let s = shared.clone();
    let done = Box::new(move |image: Option<ImageBitmap>| {
        let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
        s.result = Some(image);
        if let Some(w) = s.waker.take() {
            w.wake();
        }
    });
    match data {
        ImageData::Uri(model) => context.0.get().load_image_async(model, size, done),
        ImageData::Bytes(bytes) => {
            let bytes = bytes.clone();
            let size = graphics_size(size);
            kompoz_coroutines::run_in_background(move || done(decode_image(&bytes, size)));
        }
        ImageData::Null => done(None),
    }
    Loaded(shared).await
}

/// The data's image for `size`: from the memory cache, or once a load ends; one load per data at a time, who
/// waits for a bigger image than it brought loading another (C++'s `requestImage`). The source it came from.
async fn request_image(
    context: &PlatformContext,
    data: &ImageData,
    size: ImageRequestSize,
) -> Option<(ImageBitmap, DataSource)> {
    let key = data.key();
    loop {
        enum Next {
            Hit(ImageBitmap),
            Wait(Rc<RefCell<WaitSlot>>),
            Load,
        }
        let next = MEMORY_CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if let Some(hit) = c.find(&key, Some(size)) {
                return Next::Hit(hit);
            }
            match c.loading.get_mut(&key) {
                Some(waiters) => {
                    let slot = Rc::new(RefCell::new(WaitSlot::default()));
                    waiters.push(Waiter { slot: slot.clone() });
                    Next::Wait(slot)
                }
                None => {
                    c.loading.insert(key.clone(), Vec::new());
                    Next::Load
                }
            }
        });
        match next {
            Next::Hit(image) => {
                return Some((image, DataSource::MemoryCache));
            }
            Next::Wait(slot) => {
                // (another request's load: its image if it is big enough, else another load)
                let image = Waiting(slot).await?;
                let covers = MEMORY_CACHE.with(|c| c.borrow_mut().find(&key, Some(size)).is_some());
                if covers || any(size) {
                    return Some((image, source_of(data)));
                }
                continue;
            }
            Next::Load => {
                let loaded = load_from_platform(context, data, size).await;
                // Smaller than asked for: the decoder couldn't do better, it is the whole image.
                let whole = loaded
                    .as_ref()
                    .is_some_and(|i| any(size) || !image_covers(i.width(), i.height(), graphics_size(size)));
                let waiters = MEMORY_CACHE.with(|c| {
                    let mut c = c.borrow_mut();
                    if let Some(image) = &loaded {
                        c.put(&key, image, whole); // (failures are tried again next time)
                    }
                    c.loading.remove(&key).unwrap_or_default()
                });
                for w in waiters {
                    let mut s = w.slot.borrow_mut();
                    s.result = Some(loaded.clone());
                    if let Some(waker) = s.waker.take() {
                        waker.wake();
                    }
                }
                return loaded.map(|i| (i, source_of(data)));
            }
        }
    }
}

fn source_of(data: &ImageData) -> DataSource {
    match data {
        ImageData::Uri(u) if u.starts_with("http://") || u.starts_with("https://") => DataSource::Network,
        ImageData::Uri(_) => DataSource::Disk,
        _ => DataSource::Memory,
    }
}

// ---- The loader ----------------------------------------------------------------------------------------------------

struct RealImageLoader {
    context: PlatformContext,
    crossfade_millis: i32,
}

/// What loads images (Coil's `ImageLoader`): through its platform context's image loader, a request's crossfade
/// its own unless it set one. Equal to itself only.
#[derive(Clone)]
pub struct ImageLoader(Rc<RealImageLoader>);

impl PartialEq for ImageLoader {
    fn eq(&self, other: &ImageLoader) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for ImageLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("ImageLoader")
    }
}

/// What the request's target is told as it starts (Coil's `Target.onStart(placeholder)`): its placeholder, and
/// whether that came from the memory cache.
pub(crate) type OnStart<'a> = &'a dyn Fn(Option<Image>, bool);

impl ImageLoader {
    /// A builder (Coil's `ImageLoader.Builder(context)`).
    #[allow(non_snake_case)]
    pub fn Builder(context: &PlatformContext) -> ImageLoaderBuilder {
        ImageLoaderBuilder { context: context.clone(), crossfade_millis: 0 }
    }

    /// Its platform context.
    pub fn context(&self) -> &PlatformContext {
        &self.0.context
    }

    /// Runs `request` (Coil's suspending `execute`).
    pub async fn execute(&self, request: ImageRequest) -> ImageResult {
        self.execute_with(request, &|_, _| {}).await
    }

    /// Runs `request`, its target told as it starts (Coil's `RealImageLoader.execute`): no data an error; the
    /// placeholder (the data's image in the memory cache, of any size, else the request's — KompozR's
    /// placeholder memory cache key), the size resolved, the image from the memory cache or loaded.
    pub(crate) async fn execute_with(&self, mut request: ImageRequest, on_start: OnStart<'_>) -> ImageResult {
        if request.crossfade_millis == 0 {
            request.crossfade_millis = self.0.crossfade_millis;
        }
        if request.data == ImageData::Null {
            on_start(request.placeholder.clone(), false);
            let image = request.fallback.clone().or_else(|| request.error.clone());
            return ImageResult::Error(ErrorResult { image, request, throwable: ImageError::NullRequestData });
        }
        let key = request.data.key();
        let cached = MEMORY_CACHE.with(|c| c.borrow_mut().find(&key, None)).map(Image::new);
        let is_placeholder_cached = cached.is_some();
        on_start(cached.or_else(|| request.placeholder.clone()), is_placeholder_cached);
        let size = request.size_resolver().size().await;
        let scale = request.scale.unwrap_or(Scale::Fit);
        match request_image(&self.0.context, &request.data, to_platform_size(size, scale)).await {
            Some((bitmap, data_source)) => ImageResult::Success(SuccessResult {
                image: Image::new(bitmap),
                request,
                data_source,
                is_placeholder_cached,
            }),
            None => {
                let image = request.error.clone();
                let message = match &request.data {
                    ImageData::Uri(u) => u.to_string(),
                    _ => "bytes".into(),
                };
                ImageResult::Error(ErrorResult { image, request, throwable: ImageError::LoadFailed(message) })
            }
        }
    }
}

/// Coil's `ImageLoader.Builder`.
pub struct ImageLoaderBuilder {
    context: PlatformContext,
    crossfade_millis: i32,
}

impl ImageLoaderBuilder {
    /// Requests cross-faded over 200 ms unless they say otherwise (Coil's `crossfade(enable)`).
    pub fn crossfade(mut self, enable: bool) -> Self {
        self.crossfade_millis = if enable { 200 } else { 0 };
        self
    }
    /// Kotlin: `crossfade(durationMillis: Int)`
    pub fn crossfade_millis(mut self, duration_millis: i32) -> Self {
        self.crossfade_millis = duration_millis.max(0);
        self
    }
    pub fn build(self) -> ImageLoader {
        ImageLoader(Rc::new(RealImageLoader { context: self.context, crossfade_millis: self.crossfade_millis }))
    }
}

thread_local! {
    static SINGLETONS: RefCell<Vec<(PlatformContext, ImageLoader)>> = const { RefCell::new(Vec::new()) };
}

/// The image loader of each platform context (Coil's `SingletonImageLoader`).
pub struct SingletonImageLoader;

impl SingletonImageLoader {
    /// The context's loader, made when first asked (Coil's `SingletonImageLoader.get(context)`).
    pub fn get(context: &PlatformContext) -> ImageLoader {
        SINGLETONS.with(|s| {
            let mut s = s.borrow_mut();
            if let Some((_, l)) = s.iter().find(|(c, _)| c == context) {
                return l.clone();
            }
            let loader = ImageLoader::Builder(context).build();
            s.push((context.clone(), loader.clone()));
            loader
        })
    }
}
