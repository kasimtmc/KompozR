//! The icons mirrored right to left, by theme (Compose's `Icons.AutoMirrored`): the core set's and the extended
//! set's.

/// The extended set's auto-mirrored filled icons (data/auto_mirrored_filled.txt).
#[cfg(feature = "filled")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_filled.rs"]
mod filled;
/// The extended set's auto-mirrored outlined icons.
#[cfg(feature = "outlined")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_outlined.rs"]
mod outlined;
/// The extended set's auto-mirrored rounded icons.
#[cfg(feature = "rounded")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_rounded.rs"]
mod rounded;
/// The extended set's auto-mirrored sharp icons.
#[cfg(feature = "sharp")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_sharp.rs"]
mod sharp;
/// The extended set's auto-mirrored two-tone icons.
#[cfg(feature = "two-tone")]
#[rustfmt::skip]
#[path = "generated/auto_mirrored_two_tone.rs"]
mod two_tone;

/// Compose's `Icons.AutoMirrored.Filled`.
#[cfg(feature = "filled")]
pub mod Filled {
    pub use super::filled::*;
    pub use kompoz_material_icons_core::Icons::AutoMirrored::Filled::*;
}
/// Compose's `Icons.AutoMirrored.Outlined`.
#[cfg(feature = "outlined")]
pub mod Outlined {
    pub use super::outlined::*;
    pub use kompoz_material_icons_core::Icons::AutoMirrored::Outlined::*;
}
/// Compose's `Icons.AutoMirrored.Rounded`.
#[cfg(feature = "rounded")]
pub mod Rounded {
    pub use super::rounded::*;
    pub use kompoz_material_icons_core::Icons::AutoMirrored::Rounded::*;
}
/// Compose's `Icons.AutoMirrored.Sharp`.
#[cfg(feature = "sharp")]
pub mod Sharp {
    pub use super::sharp::*;
    pub use kompoz_material_icons_core::Icons::AutoMirrored::Sharp::*;
}
/// Compose's `Icons.AutoMirrored.TwoTone`.
#[cfg(feature = "two-tone")]
pub mod TwoTone {
    pub use super::two_tone::*;
    pub use kompoz_material_icons_core::Icons::AutoMirrored::TwoTone::*;
}

/// The filled ones (Compose's `Icons.AutoMirrored.Default`).
#[cfg(feature = "filled")]
pub use Filled as Default;
