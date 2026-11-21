//! Material's icons by theme with the extended set (Compose's `Icons.Filled` … objects, the core set's icons and the
//! extended set's extension properties of them): a module a theme, a function an icon, the core set's and the
//! extended set's alike.

/// The extended set's filled icons (data/filled.txt).
#[cfg(feature = "filled")]
#[rustfmt::skip]
#[path = "generated/filled.rs"]
mod filled;
/// The extended set's outlined icons.
#[cfg(feature = "outlined")]
#[rustfmt::skip]
#[path = "generated/outlined.rs"]
mod outlined;
/// The extended set's rounded icons.
#[cfg(feature = "rounded")]
#[rustfmt::skip]
#[path = "generated/rounded.rs"]
mod rounded;
/// The extended set's sharp icons.
#[cfg(feature = "sharp")]
#[rustfmt::skip]
#[path = "generated/sharp.rs"]
mod sharp;
/// The extended set's two-tone icons.
#[cfg(feature = "two-tone")]
#[rustfmt::skip]
#[path = "generated/two_tone.rs"]
mod two_tone;

/// The filled icons, the baseline theme (Compose's `Icons.Filled`): the core set's and the extended set's.
#[cfg(feature = "filled")]
pub mod Filled {
    pub use super::filled::*;
    pub use kompoz_material_icons_core::Icons::Filled::*;
}
/// The outlined icons: a thin stroke, empty inside (Compose's `Icons.Outlined`): the core set's and the extended
/// set's.
#[cfg(feature = "outlined")]
pub mod Outlined {
    pub use super::outlined::*;
    pub use kompoz_material_icons_core::Icons::Outlined::*;
}
/// The rounded icons (Compose's `Icons.Rounded`): the core set's and the extended set's.
#[cfg(feature = "rounded")]
pub mod Rounded {
    pub use super::rounded::*;
    pub use kompoz_material_icons_core::Icons::Rounded::*;
}
/// The sharp icons: straight corners (Compose's `Icons.Sharp`): the core set's and the extended set's.
#[cfg(feature = "sharp")]
pub mod Sharp {
    pub use super::sharp::*;
    pub use kompoz_material_icons_core::Icons::Sharp::*;
}
/// The two-tone icons: a translucent fill under the outline (Compose's `Icons.TwoTone`): the core set's and the
/// extended set's.
#[cfg(feature = "two-tone")]
pub mod TwoTone {
    pub use super::two_tone::*;
    pub use kompoz_material_icons_core::Icons::TwoTone::*;
}

/// The filled icons (Compose's `Icons.Default`).
#[cfg(feature = "filled")]
pub use Filled as Default;

/// The icons mirrored right to left (Compose's `Icons.AutoMirrored`), in each theme.
#[cfg(feature = "auto-mirrored")]
#[path = "auto_mirrored.rs"]
pub mod AutoMirrored;
