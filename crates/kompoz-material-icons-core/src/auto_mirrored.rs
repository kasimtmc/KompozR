//! The icons mirrored right to left, by theme (Compose's `Icons.AutoMirrored`).

/// Compose's `Icons.AutoMirrored.Filled`.
#[cfg(feature = "filled")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_filled.rs"]
pub mod Filled;
/// Compose's `Icons.AutoMirrored.Outlined`.
#[cfg(feature = "outlined")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_outlined.rs"]
pub mod Outlined;
/// Compose's `Icons.AutoMirrored.Rounded`.
#[cfg(feature = "rounded")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_rounded.rs"]
pub mod Rounded;
/// Compose's `Icons.AutoMirrored.Sharp`.
#[cfg(feature = "sharp")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_sharp.rs"]
pub mod Sharp;
/// Compose's `Icons.AutoMirrored.TwoTone`.
#[cfg(feature = "two-tone")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_two_tone.rs"]
pub mod TwoTone;

/// The filled ones (Compose's `Icons.AutoMirrored.Default`).
#[cfg(feature = "filled")]
pub use Filled as Default;
