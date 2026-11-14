//! Material Color Utilities, as KompozR uses it: the HCT color space ([`hct`]), tonal palettes ([`palettes`]),
//! a picture's colors ([`quantize`], [`score`], [`wallpaper`]), contrast ([`contrast`]), the disliked colors
//! ([`dislike`]), color temperature ([`temperature`]), and dynamic color: schemes and the colors of their roles
//! ([`dynamiccolor`], [`scheme`]). Colors are ARGB ints (`0xAARRGGBB`) here, as in Material Color Utilities;
//! [`Hct::from_color`](hct::Hct::from_color), [`Hct::to_color`](hct::Hct::to_color) and
//! [`DynamicScheme::get_color`](dynamiccolor::DynamicScheme::get_color) turn them into [`Color`](crate::Color)s.

pub mod contrast;
pub mod dislike;
pub mod dynamiccolor;
pub mod hct;
pub mod palettes;
pub mod quantize;
pub mod scheme;
pub mod score;
pub mod temperature;
pub mod utils;
pub mod wallpaper;
