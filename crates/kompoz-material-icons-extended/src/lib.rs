//! KompozR's extended Material icons, as Compose's `androidx.compose.material:material-icons-extended` (1.7.8): the
//! rest of Material's icons, beside the core set's, in each of Material's themes — `Icons::Outlined::StarBorder()`,
//! `Icons::Filled::Tune()`, `Icons::AutoMirrored::Filled::MenuOpen()` (mirrored right to left) — each an
//! `ImageVector` built once, when first asked for, and shared after. [`Icons::Default`] is the filled theme.
//!
//! Compose's extended icons are extension properties of core's `Icons.Filled` …, so one `Icons.Filled` names both
//! sets; so do this crate's modules: `Icons::Filled::Add()` (the core set's, re-exported) and `Icons::Filled::Tune()`
//! alike, as the umbrella's `kompoz::Icons`. Building one's own icon is the core set's
//! (`kompoz_material_icons_core::material_icon`).
//!
//! A cargo feature a theme (`filled`, `outlined`, `rounded`, `sharp`, `two-tone`, `auto-mirrored`; all by default)
//! leaves the others out: the set is large (2 083 icons a theme, 138 auto-mirrored), and each theme compiles its
//! own functions. The icons are data (`data/<theme>.txt`: each icon's paths as SVG path data) converted from
//! Compose's Kotlin by `cargo xtask material-icons`, built by the core set's `materialIcon`/`materialPath`.
//!
//! In Kotlin's syntax, `Icons.Outlined.StarBorder` is `Icons::Outlined::StarBorder()` (the generator's rule for
//! `Icons.<theme>.<name>`).

#![allow(non_snake_case, non_upper_case_globals)]

// (the core set's: each theme's file is `crate::icons! { … }`)
#[cfg(any(
    feature = "filled",
    feature = "outlined",
    feature = "rounded",
    feature = "sharp",
    feature = "two-tone"
))]
use kompoz_material_icons_core::icons;

/// Material's icons with the extended set (Compose's `object Icons` and the extended set's extension properties of
/// its themes): a module a theme, a function an icon.
#[path = "icons.rs"]
pub mod Icons;
