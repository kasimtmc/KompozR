//! Requests and results (Coil's `coil3.request` and `coil3.size`, what the compose layer uses of them): what to
//! load (a path, a `file:`/`http(s):` URI, bytes), at what size and scale, cross-faded or not, with which
//! placeholder, error and fallback images; what came of it.

use crate::compose::ConstraintsSizeResolver;
use kompoz_graphics::{FilterQuality, ImageBitmap};
use kompoz_ui::draw::{BitmapPainter, Painter};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// What a request loads (Coil's `data: Any?`): what KompozR's platform image loader takes — a path (an asset, a
/// file next to the program), a `file:` or `http(s):` URI — or an image file's bytes; none (Coil's
/// `NullRequestData`: an error, the fallback shown).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum ImageData {
    #[default]
    Null,
    Uri(Arc<str>),
    Bytes(Arc<[u8]>),
}

impl ImageData {
    /// The key it is kept by in the memory cache (Coil's memory cache key's data): its text, or its bytes' hash.
    pub(crate) fn key(&self) -> String {
        match self {
            ImageData::Null => String::new(),
            ImageData::Uri(s) => s.to_string(),
            ImageData::Bytes(b) => {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for x in b.iter() {
                    h ^= *x as u64;
                    h = h.wrapping_mul(0x0100_0000_01b3);
                }
                format!("bytes:{h:016x}:{}", b.len())
            }
        }
    }
}

impl From<&str> for ImageData {
    fn from(s: &str) -> Self {
        ImageData::Uri(s.into())
    }
}

impl From<String> for ImageData {
    fn from(s: String) -> Self {
        ImageData::Uri(s.into())
    }
}

impl From<&String> for ImageData {
    fn from(s: &String) -> Self {
        ImageData::Uri(s.as_str().into())
    }
}

impl From<Vec<u8>> for ImageData {
    fn from(b: Vec<u8>) -> Self {
        ImageData::Bytes(b.into())
    }
}

impl<T: Into<ImageData>> From<Option<T>> for ImageData {
    fn from(o: Option<T>) -> Self {
        o.map_or(ImageData::Null, Into::into)
    }
}

/// A side of a size (Coil's `Dimension`): pixels, or undefined (as big as it is).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dimension {
    Pixels(i32),
    Undefined,
}

/// The size an image is wanted at (Coil's `coil3.size.Size`); `ORIGINAL`: both undefined, the whole image.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CoilSize {
    pub width: Dimension,
    pub height: Dimension,
}

impl CoilSize {
    /// Kotlin: `Size.ORIGINAL`
    pub const ORIGINAL: CoilSize = CoilSize { width: Dimension::Undefined, height: Dimension::Undefined };

    pub fn new(width: i32, height: i32) -> CoilSize {
        CoilSize { width: Dimension::Pixels(width), height: Dimension::Pixels(height) }
    }
}

/// How an image is fitted to the size asked (Coil's `Scale`): to fill it (cropped) or to fit in it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Scale {
    /// Kotlin: `FILL`
    Fill,
    /// Kotlin: `FIT`
    Fit,
}

/// Whether the size must be met exactly (Coil's `Precision`): `Inexact` lets a bigger or smaller image do (what
/// the compose layer asks: the painter scales it as it draws).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Precision {
    Exact,
    Inexact,
}

/// Where a request's size comes from (Coil's `SizeResolver`): a size, or the layout's constraints once it is
/// measured (Coil compose's `ConstraintsSizeResolver`).
#[derive(Clone)]
pub enum SizeResolver {
    Size(CoilSize),
    Constraints(Rc<ConstraintsSizeResolver>),
}

impl SizeResolver {
    /// Kotlin: `SizeResolver.ORIGINAL`
    pub const ORIGINAL: SizeResolver = SizeResolver::Size(CoilSize::ORIGINAL);

    /// The size, once known (a constraints resolver waits for its layout's first measure).
    pub async fn size(&self) -> CoilSize {
        match self {
            SizeResolver::Size(s) => *s,
            SizeResolver::Constraints(c) => c.size().await,
        }
    }
}

impl PartialEq for SizeResolver {
    fn eq(&self, other: &SizeResolver) -> bool {
        match (self, other) {
            (SizeResolver::Size(a), SizeResolver::Size(b)) => a == b,
            (SizeResolver::Constraints(a), SizeResolver::Constraints(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for SizeResolver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SizeResolver::Size(s) => write!(f, "SizeResolver({s:?})"),
            SizeResolver::Constraints(_) => f.write_str("ConstraintsSizeResolver"),
        }
    }
}

/// An image a request gives (Coil's `Image`): a bitmap here.
#[derive(Clone, PartialEq, Debug)]
pub struct Image {
    pub bitmap: ImageBitmap,
}

impl Image {
    pub fn new(bitmap: ImageBitmap) -> Image {
        Image { bitmap }
    }
    pub fn width(&self) -> i32 {
        self.bitmap.width()
    }
    pub fn height(&self) -> i32 {
        self.bitmap.height()
    }
    /// Its painter (Coil compose's `Image.asPainter(context, filterQuality)`).
    pub fn as_painter(&self, filter_quality: FilterQuality) -> Rc<dyn Painter> {
        Rc::new(BitmapPainter::with_quality(self.bitmap.clone(), filter_quality))
    }
}

impl From<ImageBitmap> for Image {
    fn from(bitmap: ImageBitmap) -> Image {
        Image::new(bitmap)
    }
}

/// A request (Coil's `ImageRequest`): its data, its size resolver, scale and precision (none: the loader's),
/// its cross-fade (ms; 0: none), its placeholder, error and fallback images. Equal by all of it (Coil's `@Poko`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImageRequest {
    pub data: ImageData,
    pub size_resolver: Option<SizeResolver>,
    pub scale: Option<Scale>,
    pub precision: Option<Precision>,
    pub crossfade_millis: i32,
    pub placeholder: Option<Image>,
    pub error: Option<Image>,
    pub fallback: Option<Image>,
    pub use_existing_image_as_placeholder: bool,
    pub prefer_end_first_intrinsic_size: bool,
}

impl ImageRequest {
    /// A builder (Coil's `ImageRequest.Builder(context)`).
    #[allow(non_snake_case)]
    pub fn Builder(context: &PlatformContext) -> ImageRequestBuilder {
        let _ = context;
        ImageRequestBuilder { request: ImageRequest::default() }
    }
    /// A builder of the same request (Coil's `newBuilder()`).
    pub fn new_builder(&self) -> ImageRequestBuilder {
        ImageRequestBuilder { request: self.clone() }
    }
    /// The size resolver, the original size where none was set (Coil's `sizeResolver`).
    pub fn size_resolver(&self) -> SizeResolver {
        self.size_resolver.clone().unwrap_or(SizeResolver::ORIGINAL)
    }
}

macro_rules! request_of {
    ($($t:ty),*) => {$(
        impl From<$t> for ImageRequest {
            fn from(data: $t) -> ImageRequest {
                ImageRequest { data: data.into(), ..ImageRequest::default() }
            }
        }
    )*};
}

request_of!(ImageData, &str, String, &String, Vec<u8>, Option<&str>, Option<String>);

/// Coil's `ImageRequest.Builder`.
pub struct ImageRequestBuilder {
    request: ImageRequest,
}

impl ImageRequestBuilder {
    pub fn data(mut self, data: impl Into<ImageData>) -> Self {
        self.request.data = data.into();
        self
    }
    /// Cross-faded over Coil's default 200 ms, or not (Coil's `crossfade(enable)`).
    pub fn crossfade(mut self, enable: bool) -> Self {
        self.request.crossfade_millis = if enable { 200 } else { 0 };
        self
    }
    /// Cross-faded over `duration_millis` (Coil's `crossfade(durationMillis)`).
    ///
    /// Kotlin: `crossfade(durationMillis: Int)`
    pub fn crossfade_millis(mut self, duration_millis: i32) -> Self {
        self.request.crossfade_millis = duration_millis.max(0);
        self
    }
    /// Kotlin: `size(width: Int, height: Int)`
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.request.size_resolver = Some(SizeResolver::Size(CoilSize::new(width, height)));
        self
    }
    /// Kotlin: `size(resolver: SizeResolver)`
    pub fn size_resolver(mut self, resolver: SizeResolver) -> Self {
        self.request.size_resolver = Some(resolver);
        self
    }
    pub fn scale(mut self, scale: Scale) -> Self {
        self.request.scale = Some(scale);
        self
    }
    pub fn precision(mut self, precision: Precision) -> Self {
        self.request.precision = Some(precision);
        self
    }
    pub fn placeholder(mut self, image: Option<Image>) -> Self {
        self.request.placeholder = image;
        self
    }
    pub fn error(mut self, image: Option<Image>) -> Self {
        self.request.error = image;
        self
    }
    pub fn fallback(mut self, image: Option<Image>) -> Self {
        self.request.fallback = image;
        self
    }
    /// Coil compose's `useExistingImageAsPlaceholder(enable)`.
    pub fn use_existing_image_as_placeholder(mut self, enable: bool) -> Self {
        self.request.use_existing_image_as_placeholder = enable;
        self
    }
    /// Coil compose's `preferEndFirstIntrinsicSize(enable)`.
    pub fn prefer_end_first_intrinsic_size(mut self, enable: bool) -> Self {
        self.request.prefer_end_first_intrinsic_size = enable;
        self
    }
    pub fn build(self) -> ImageRequest {
        self.request
    }
}

/// Where an image came from (Coil's `DataSource`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DataSource {
    /// Kotlin: `MEMORY_CACHE`
    MemoryCache,
    /// Kotlin: `MEMORY`
    Memory,
    /// Kotlin: `DISK`
    Disk,
    /// Kotlin: `NETWORK`
    Network,
}

/// A request that gave an image (Coil's `SuccessResult`).
#[derive(Clone, PartialEq, Debug)]
pub struct SuccessResult {
    pub image: Image,
    pub request: ImageRequest,
    pub data_source: DataSource,
    /// Whether the placeholder shown meanwhile came from the memory cache (Coil's `isPlaceholderCached`).
    pub is_placeholder_cached: bool,
}

/// Why a request gave no image (Coil's `throwable`): its data was none (`NullRequestDataException`), or it
/// couldn't be loaded or decoded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImageError {
    NullRequestData,
    LoadFailed(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageError::NullRequestData => f.write_str("The request's data is null."),
            ImageError::LoadFailed(m) => write!(f, "Failed to load the image: {m}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// A request that gave no image (Coil's `ErrorResult`): its error image, if it has one.
#[derive(Clone, PartialEq, Debug)]
pub struct ErrorResult {
    pub image: Option<Image>,
    pub request: ImageRequest,
    pub throwable: ImageError,
}

/// What came of a request (Coil's `ImageResult`).
#[derive(Clone, PartialEq, Debug)]
pub enum ImageResult {
    Success(SuccessResult),
    Error(ErrorResult),
}

impl ImageResult {
    pub fn image(&self) -> Option<&Image> {
        match self {
            ImageResult::Success(s) => Some(&s.image),
            ImageResult::Error(e) => e.image.as_ref(),
        }
    }
    pub fn request(&self) -> &ImageRequest {
        match self {
            ImageResult::Success(s) => &s.request,
            ImageResult::Error(e) => &e.request,
        }
    }
}

/// What requests are made in (Coil's `PlatformContext`: Android's Context, elsewhere nothing): here the platform
/// whose image loader loads them (the host's callbacks).
#[derive(Clone, PartialEq, Default)]
pub struct PlatformContext(pub kompoz_ui::platform::PlatformHandle);

impl fmt::Debug for PlatformContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("PlatformContext")
    }
}
