//! KompozR's `AsyncImage`, as Coil 3's compose layer (coil-compose-core and coil-compose 3.4): an image loaded
//! from a model — a path, a `file:` or `http(s):` URI, bytes — away from the UI thread and painted, with Coil's
//! API: [`AsyncImage`] (loaded at the size its layout's constraints give), [`remember_async_image_painter`] (a
//! painter, at the original size), [`SubcomposeAsyncImage`] (composable slots for loading, success and error), the
//! painter's [`AsyncImagePainterState`]s, placeholder, error and fallback painters, cross-fades
//! ([`ImageRequest`]'s `crossfade`), [`ImageLoader`]s.
//!
//! What loads is KompozR's platform image loader (the host's `PlatformCallbacks::load_image_async`:
//! kompoz-platform's decoders, scaled down as they decode, downloads in its disk cache), behind a memory cache of
//! what was loaded (by model, the biggest so far, 64 MB, the C++ Kompoz's): an image of the model already there is
//! shown while a bigger one loads.
//!
//! ```ignore
//! AsyncImage("https://example.com/photo.jpg", Some("Photo".into()), modifier = Modifier.size(100.dp()),
//!     content_scale = ContentScale::Crop);
//! ```

#![allow(non_snake_case, non_upper_case_globals)]

pub mod compose;
pub mod loader;
pub mod request;

pub use compose::{
    AsyncImage, AsyncImagePainter, AsyncImagePainterState, AsyncImageWith, ConstraintsSizeResolver, CrossfadePainter,
    Input, LocalPlatformContext, StateListener, StateTransform, SubcomposeAsyncImage, SubcomposeAsyncImageContent,
    SubcomposeAsyncImageScope, SubcomposeAsyncImageWith, SubcomposeContent, SubcomposeSlot, content_of,
    content_scale_to_scale, default_transform, on_state_of, remember_async_image_painter,
    remember_async_image_painter_with, remember_constraints_size_resolver, transform_of,
};
pub use loader::{ImageLoader, ImageLoaderBuilder, SingletonImageLoader, clear_image_cache, image_cache_size};
pub use request::{
    CoilSize, DataSource, Dimension, ErrorResult, Image, ImageData, ImageError, ImageRequest, ImageRequestBuilder,
    ImageResult, PlatformContext, Precision, Scale, SizeResolver, SuccessResult,
};

/// What an app writes with (its modules left out).
pub mod prelude {
    pub use crate::{
        AsyncImage, AsyncImagePainter, AsyncImagePainterState, AsyncImageWith, ImageLoader, ImageRequest,
        LocalPlatformContext, SingletonImageLoader, SubcomposeAsyncImage, SubcomposeAsyncImageContent,
        SubcomposeAsyncImageScope, SubcomposeAsyncImageWith, remember_async_image_painter,
        remember_async_image_painter_with,
    };
}
