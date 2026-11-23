//! Image resources (CMP's ImageResources.kt, ImageDecoders.kt, their skiko forms): a drawable's painter — a
//! bitmap's (PNG, JPEG, WebP, BMP), a vector's (an Android vector drawable, `.xml`), an SVG's — its bitmap, scaled
//! down when its item's density is above the screen's (Android only scales drawables down), its vector; decoded
//! once a path (and a screen density) for the process.

use crate::caches::AsyncCache;
use crate::environment::{ResourceEnvironment, ResourceSelectionError, remember_resource_environment};
use crate::qualifier::{DensityQualifier, Qualifier};
use crate::reader::{DefaultResourceReader, LocalResourceReader, MissingResourceException, ResourceReader};
use crate::resource::DrawableResource;
use crate::state::remember_resource_state;
use crate::svg::{SvgPainter, SvgParseError};
use crate::vector::{VectorParseError, decode_to_image_vector};
use kompoz_graphics::{Density, FilterQuality, ImageBitmap, ImageRequestSize, ImageVector, decode_image};
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::draw::{BitmapPainter, Painter, remember_vector_painter};
use kompoz_ui::platform::LocalDensity;
use std::fmt;
use std::rc::Rc;

/// Why an image resource can't be read: its file, its item, its bytes (Skia's "Failed to Image::makeFromEncoded"),
/// its vector's or SVG's text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImageResourceError {
    Missing(MissingResourceException),
    Selection(ResourceSelectionError),
    Decode(String),
    Vector(VectorParseError),
    Svg(SvgParseError),
}

impl fmt::Display for ImageResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageResourceError::Missing(e) => e.fmt(f),
            ImageResourceError::Selection(e) => e.fmt(f),
            ImageResourceError::Decode(m) => f.write_str(m),
            ImageResourceError::Vector(e) => e.fmt(f),
            ImageResourceError::Svg(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ImageResourceError {}

impl From<MissingResourceException> for ImageResourceError {
    fn from(e: MissingResourceException) -> Self {
        ImageResourceError::Missing(e)
    }
}

impl From<ResourceSelectionError> for ImageResourceError {
    fn from(e: ResourceSelectionError) -> Self {
        ImageResourceError::Selection(e)
    }
}

/// A decoded image kept (CMP's `ImageCache`): a bitmap or a vector (any thread's); SVG painters are kept by each
/// thread (a painter lives on its UI thread).
#[derive(Clone)]
enum ImageCache {
    Bitmap(ImageBitmap),
    Vector(ImageVector),
}

static IMAGE_CACHE: AsyncCache<String, ImageCache> = AsyncCache::new();

thread_local! {
    static SVG_CACHE: Rc<AsyncCache<String, Rc<SvgPainter>>> = Rc::new(AsyncCache::new());
}

pub(crate) fn clear_cache() {
    IMAGE_CACHE.clear();
    SVG_CACHE.with(|c| c.clear());
}

/// Panics with the error, as Kotlin throws it where a composable reads it.
fn or_throw<T>(result: Result<T, ImageResourceError>) -> T {
    result.unwrap_or_else(|e| panic!("{e}"))
}

/// What a drawable resource paints with here (CMP's `painterResource(resource)`): its item for the environment, a
/// vector's painter (`.xml`), an SVG's (`.svg`), else a bitmap's. Panics if it can't be read.
#[kompozable]
pub fn painter_resource(resource: DrawableResource) -> Rc<dyn Painter> {
    let environment = crate::environment::LocalComposeEnvironment.current().remember_environment();
    let r = resource.clone();
    let file_path = remember_keyed((resource.clone(), environment.clone()), move || {
        or_throw(r.get_resource_item_by_environment(&environment).map(|i| i.path.clone()).map_err(Into::into))
    });
    let lower = file_path.to_ascii_lowercase();
    if lower.ends_with(".xml") {
        Rc::new(remember_vector_painter(vector_resource(resource)))
    } else if lower.ends_with(".svg") {
        svg_painter(resource)
    } else {
        let bitmap = image_resource(resource);
        remember_keyed(bitmap.clone(), move || {
            let painter: Rc<dyn Painter> = Rc::new(BitmapPainter::with_quality(bitmap, FilterQuality::Low));
            painter
        })
    }
}

/// The bitmap of a drawable resource here (CMP's `imageResource(resource)`): decoded once for its path and the
/// screen's density; an item of a higher density than the screen's scaled down to it (one of a lower density
/// stays as it is, as Android does).
#[kompozable]
pub fn image_resource(resource: DrawableResource) -> ImageBitmap {
    let reader = LocalResourceReader.current();
    let environment = remember_resource_environment();
    let keys = (resource.clone(), reader.clone(), environment.clone());
    let state = remember_resource_state(keys, empty_image_bitmap, move |env| async move {
        or_throw(load_image_bitmap(&resource, &*reader, &env, environment.density.dpi()).await)
    });
    state.get()
}

async fn load_image_bitmap(
    resource: &DrawableResource,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
    screen_density: i32,
) -> Result<ImageBitmap, ImageResourceError> {
    let item = resource.get_resource_item_by_environment(environment)?;
    let resource_density = item
        .qualifiers
        .iter()
        .find_map(|q| match q {
            Qualifier::Density(d) => Some(d.dpi()),
            _ => None,
        })
        .unwrap_or(DensityQualifier::Mdpi.dpi());
    let path = item.path.clone();
    let key = format!("{path}-{screen_density}dpi");
    let cached = IMAGE_CACHE
        .get_or_load(key, || async {
            let bytes = reader.read(&path).await?;
            Ok::<_, ImageResourceError>(ImageCache::Bitmap(to_image_bitmap(&bytes, resource_density, screen_density)?))
        })
        .await?;
    match cached {
        ImageCache::Bitmap(b) => Ok(b),
        ImageCache::Vector(_) => Err(ImageResourceError::Decode(format!("{path} is kept as a vector"))),
    }
}

/// A 1 x 1 bitmap: what an asynchronous platform shows while it loads (CMP's `emptyImageBitmap`).
fn empty_image_bitmap() -> ImageBitmap {
    ImageBitmap::from_pixels(1, 1, vec![0; 4]).expect("a pixel")
}

/// The vector of a drawable resource here (CMP's `vectorResource(resource)`): its Android vector drawable read at
/// the composition's density (a `px` size through it), once a path.
#[kompozable]
pub fn vector_resource(resource: DrawableResource) -> ImageVector {
    let reader = LocalResourceReader.current();
    let density = LocalDensity.current();
    let keys = (resource.clone(), reader.clone(), density);
    let state = remember_resource_state(keys, empty_image_vector, move |env| async move {
        or_throw(load_image_vector(&resource, &*reader, &env, density).await)
    });
    state.get()
}

async fn load_image_vector(
    resource: &DrawableResource,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
    density: Density,
) -> Result<ImageVector, ImageResourceError> {
    let path = resource.get_resource_item_by_environment(environment)?.path.clone();
    let cached = IMAGE_CACHE
        .get_or_load(path.clone(), || async {
            let bytes = reader.read(&path).await?;
            let vector = decode_to_image_vector(&bytes, density).map_err(ImageResourceError::Vector)?;
            Ok::<_, ImageResourceError>(ImageCache::Vector(vector))
        })
        .await?;
    match cached {
        ImageCache::Vector(v) => Ok(v),
        ImageCache::Bitmap(_) => Err(ImageResourceError::Decode(format!("{path} is kept as a bitmap"))),
    }
}

/// A 1 x 1 dp vector of nothing (CMP's `emptyImageVector`).
fn empty_image_vector() -> ImageVector {
    kompoz_graphics::vector::ImageVectorBuilder::new(kompoz_graphics::Dp(1.0), kompoz_graphics::Dp(1.0), 1.0, 1.0)
        .build()
}

/// An SVG drawable's painter here (CMP's private `svgPainter`), once a path (on this thread).
#[kompozable]
fn svg_painter(resource: DrawableResource) -> Rc<dyn Painter> {
    let reader = LocalResourceReader.current();
    let density = LocalDensity.current();
    let keys = (resource.clone(), reader.clone(), density);
    let state = remember_resource_state(keys, empty_svg_painter, move |env| async move {
        let painter = or_throw(load_svg_painter(&resource, &*reader, &env, density).await);
        let painter: Rc<dyn Painter> = painter;
        painter
    });
    state.get()
}

fn empty_svg_painter() -> Rc<dyn Painter> {
    Rc::new(BitmapPainter::with_quality(empty_image_bitmap(), FilterQuality::Low))
}

async fn load_svg_painter(
    resource: &DrawableResource,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
    density: Density,
) -> Result<Rc<SvgPainter>, ImageResourceError> {
    let path = resource.get_resource_item_by_environment(environment)?.path.clone();
    let load = || async {
        let bytes = reader.read(&path).await?;
        let painter = SvgPainter::decode(&bytes, density).map_err(ImageResourceError::Svg)?;
        Ok::<_, ImageResourceError>(Rc::new(painter))
    };
    let cache = SVG_CACHE.with(Rc::clone);
    cache.get_or_load(path.clone(), load).await
}

/// A drawable's bytes for `environment`, read by the default reader (CMP's `getDrawableResourceBytes`).
pub async fn get_drawable_resource_bytes(
    environment: &ResourceEnvironment,
    resource: &DrawableResource,
) -> Result<Vec<u8>, ImageResourceError> {
    let item = resource.get_resource_item_by_environment(environment)?;
    Ok(DefaultResourceReader.read(&item.path).await?)
}

/// An image file's bytes decoded (CMP's `ByteArray.decodeToImageBitmap()`): unscaled.
pub fn decode_to_image_bitmap(bytes: &[u8]) -> Result<ImageBitmap, ImageResourceError> {
    to_image_bitmap(bytes, 0, 0)
}

/// An SVG's bytes as a painter at `density` (CMP's `ByteArray.decodeToSvgPainter(density)`).
pub fn decode_to_svg_painter(bytes: &[u8], density: Density) -> Result<Rc<dyn Painter>, ImageResourceError> {
    Ok(Rc::new(SvgPainter::decode(bytes, density).map_err(ImageResourceError::Svg)?))
}

/// An image decoded; scaled down by `target / resource` where its density is above the screen's (CMP-5657: as
/// Android, never up), linearly sampled (Skia's `SamplingMode.LINEAR`) into whole pixels.
pub(crate) fn to_image_bitmap(
    bytes: &[u8],
    resource_density: i32,
    target_density: i32,
) -> Result<ImageBitmap, ImageResourceError> {
    let image = decode_image(bytes, ImageRequestSize::default())
        .ok_or_else(|| ImageResourceError::Decode("Failed to Image::makeFromEncoded".into()))?;
    if resource_density <= target_density {
        return Ok(image);
    }
    let scale = target_density as f32 / resource_density as f32;
    let (w, h) = ((image.width() as f32 * scale) as i32, (image.height() as f32 * scale) as i32);
    if w <= 0 || h <= 0 {
        return Err(ImageResourceError::Decode("Failed to Surface::makeRasterN32Premul".into()));
    }
    let src = image.pixels();
    let (sw, sh) = (image.width() as usize, image.height() as usize);
    // premultiplied, as Skia samples
    let at = |x: usize, y: usize| -> [f32; 4] {
        let i = (y * sw + x) * 4;
        let a = src[i + 3] as f32 / 255.0;
        [src[i] as f32 * a, src[i + 1] as f32 * a, src[i + 2] as f32 * a, src[i + 3] as f32]
    };
    let mut out = vec![0u8; w as usize * h as usize * 4];
    for y in 0..h as usize {
        let fy = ((y as f32 + 0.5) / scale - 0.5).clamp(0.0, (sh - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy - fy.floor());
        let y1 = (y0 + 1).min(sh - 1);
        for x in 0..w as usize {
            let fx = ((x as f32 + 0.5) / scale - 0.5).clamp(0.0, (sw - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx - fx.floor());
            let x1 = (x0 + 1).min(sw - 1);
            let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
            let mut p = [0f32; 4];
            for k in 0..4 {
                let top = a[k] + (b[k] - a[k]) * tx;
                let bottom = c[k] + (d[k] - c[k]) * tx;
                p[k] = top + (bottom - top) * ty;
            }
            let o = (y * w as usize + x) * 4;
            let alpha = p[3] / 255.0;
            for k in 0..3 {
                out[o + k] = if alpha > 0.0 { (p[k] / alpha).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            out[o + 3] = p[3].round().clamp(0.0, 255.0) as u8;
        }
    }
    ImageBitmap::from_pixels(w, h, out).ok_or_else(|| ImageResourceError::Decode("Failed to scale the image".into()))
}
