//! Material's icons by theme (Compose's `object Icons` and its themes' objects): a module a theme, a function an
//! icon.

/// The filled icons, the baseline theme (Compose's `Icons.Filled`).
#[cfg(feature = "filled")]
#[rustfmt::skip]
#[path = "generated/filled.rs"]
pub mod Filled;
/// The outlined icons: a thin stroke, empty inside (Compose's `Icons.Outlined`).
#[cfg(feature = "outlined")]
#[rustfmt::skip]
#[path = "generated/outlined.rs"]
pub mod Outlined;
/// The rounded icons (Compose's `Icons.Rounded`).
#[cfg(feature = "rounded")]
#[rustfmt::skip]
#[path = "generated/rounded.rs"]
pub mod Rounded;
/// The sharp icons: straight corners (Compose's `Icons.Sharp`).
#[cfg(feature = "sharp")]
#[rustfmt::skip]
#[path = "generated/sharp.rs"]
pub mod Sharp;
/// The two-tone icons: a translucent fill under the outline (Compose's `Icons.TwoTone`).
#[cfg(feature = "two-tone")]
#[rustfmt::skip]
#[path = "generated/two_tone.rs"]
pub mod TwoTone;

/// The filled icons (Compose's `Icons.Default`).
#[cfg(feature = "filled")]
pub use Filled as Default;

/// The icons mirrored right to left (a back arrow points to the start; Compose's `Icons.AutoMirrored`), in each
/// theme.
#[cfg(feature = "auto-mirrored")]
#[path = "auto_mirrored.rs"]
pub mod AutoMirrored;
