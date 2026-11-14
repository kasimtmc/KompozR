//! Bitmaps (Compose's ImageBitmap) and the pixels read from them (PixelMap), how an image is scaled into a space
//! (ContentScale, ScaleFactor), image files decoded (JPEG, PNG, WebP, BMP) at the size they are wanted at, and PNG
//! written.

mod bitmap;
mod bmp_decoder;
mod codec;
mod content_scale;
mod png_decoder;
mod sample;

pub use bitmap::{ImageBitmap, ImageBitmapConfig, PixelMap};
pub use bmp_decoder::decode_bmp;
pub use codec::{RgbaImage, decode_image, decode_to_image_bitmap, encode_png, save_png};
pub use content_scale::{ComputeScaleFactor, ContentScale, ScaleFactor, lerp_scale_factor};
pub use png_decoder::decode_png;
pub use sample::{ImageRequestSize, MAX_IMAGE_DIMENSION, image_covers, image_sample_size, scale_down};
