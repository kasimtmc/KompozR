//! The window's adaptive info, as Compose's adaptive WindowAdaptiveInfo.kt (material3-adaptive 1.2.0, Compose for
//! desktop's actual: no postures off Android) and its internal WindowSizeClassHelper.kt: the window's size class
//! and posture ([`current_window_adaptive_info`], [`WindowAdaptiveInfo`]) and its size ([`current_window_size`],
//! [`current_window_dp_size`]).

use crate::posture::Posture;
use crate::window_core::WindowSizeClass;
use kompoz_graphics::{Dp, DpSize, IntSize, Size};
use kompoz_runtime::kompozable;
use kompoz_runtime::kt::KtToString;
use kompoz_ui::platform::{LocalDensity, LocalWindowInfo};
use std::fmt;

/// What a layout adapts to (Compose's `WindowAdaptiveInfo`): the window's size class and its posture. Equal by
/// both.
// (WP8 navsuite: KotlinProperties — Kotlin reads `info.windowSizeClass`, `with(info) { windowPosture }`)
#[derive(Clone, PartialEq, Debug, kompoz_runtime::KotlinProperties)]
pub struct WindowAdaptiveInfo {
    pub window_size_class: WindowSizeClass,
    pub window_posture: Posture,
}

/// Kotlin: `WindowAdaptiveInfo(windowSizeClass: WindowSizeClass, windowPosture: Posture)`
pub fn WindowAdaptiveInfo(window_size_class: WindowSizeClass, window_posture: Posture) -> WindowAdaptiveInfo {
    WindowAdaptiveInfo { window_size_class, window_posture }
}

/// Kotlin's `toString`: `WindowAdaptiveInfo(windowSizeClass=…, windowPosture=…)`.
impl fmt::Display for WindowAdaptiveInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WindowAdaptiveInfo(windowSizeClass={}, windowPosture={})",
            self.window_size_class, self.window_posture
        )
    }
}

impl KtToString for WindowAdaptiveInfo {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// The current window's adaptive info (Compose's `currentWindowAdaptiveInfo`): its size class from
/// [`current_window_dp_size`] — with the Large (1200 dp) and Extra Large (1600 dp) width classes only when
/// `support_large_and_x_large_width` — and its posture (on the desktop the default one, as Compose for desktop's:
/// postures and hinges are Android's).
///
/// Kotlin: `currentWindowAdaptiveInfo(supportLargeAndXLargeWidth: Boolean = false): WindowAdaptiveInfo`
#[kompozable]
pub fn current_window_adaptive_info(#[default(false)] support_large_and_x_large_width: bool) -> WindowAdaptiveInfo {
    let window_size = current_window_dp_size();
    WindowAdaptiveInfo {
        window_size_class: if support_large_and_x_large_width {
            compute_from_dp_size_v2(window_size)
        } else {
            compute_from_dp_size(window_size, DP_WIDTH_SIZE_CLASSES_DEFAULT, DP_HEIGHT_SIZE_CLASSES_DEFAULT)
        },
        window_posture: calculate_posture(),
    }
}

/// The current window's adaptive info with the Large (1200 dp) and Extra Large (1600 dp) width classes (adaptive
/// 1.3's `currentWindowAdaptiveInfoV2`, which material3 1.5's navigation suite samples call): [`current_window_adaptive_info`]
/// with `support_large_and_x_large_width`.
///
/// Kotlin: `currentWindowAdaptiveInfoV2(): WindowAdaptiveInfo`
#[kompozable]
pub fn current_window_adaptive_info_v2() -> WindowAdaptiveInfo {
    current_window_adaptive_info(support_large_and_x_large_width = true)
}

/// The current window's size in dp (Compose's `currentWindowDpSize`): [`current_window_size`] by the density.
#[kompozable]
pub fn current_window_dp_size() -> DpSize {
    let density = LocalDensity.current();
    let size = current_window_size();
    density.size_to_dp(Size::new(size.width as f32, size.height as f32))
}

/// The current window's size in pixels (Compose's `currentWindowSize`): `LocalWindowInfo`'s container size.
#[kompozable]
pub fn current_window_size() -> IntSize {
    LocalWindowInfo.current().container_size()
}

/// The window's posture (Compose's internal `expect fun calculatePosture`): Compose for desktop's actual, the
/// default posture — postures and hinges are relevant to Android devices only.
#[kompozable]
fn calculate_posture() -> Posture {
    Posture::default()
}

// ---- WindowSizeClassHelper.kt (internal) ----

/// The lower bounds of the width size classes (Compose's private `DpWidthSizeClasses`): compact 0, medium 600,
/// expanded 840, large 1200, extra large 1600 dp.
mod dp_width_size_classes {
    use super::Dp;
    use crate::window_core::WindowSizeClass;
    pub const Compact: Dp = Dp(0.0);
    pub const Medium: Dp = Dp(WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND as f32);
    pub const Expanded: Dp = Dp(WindowSizeClass::WIDTH_DP_EXPANDED_LOWER_BOUND as f32);
    pub const Large: Dp = Dp(1200.0);
    pub const ExtraLarge: Dp = Dp(1600.0);
}

/// The lower bounds of the height size classes (Compose's private `DpHeightSizeClasses`): compact 0, medium 480,
/// expanded 900 dp.
mod dp_height_size_classes {
    use super::Dp;
    use crate::window_core::WindowSizeClass;
    pub const Compact: Dp = Dp(0.0);
    pub const Medium: Dp = Dp(WindowSizeClass::HEIGHT_DP_MEDIUM_LOWER_BOUND as f32);
    pub const Expanded: Dp = Dp(WindowSizeClass::HEIGHT_DP_EXPANDED_LOWER_BOUND as f32);
}

/// `DpWidthSizeClasses.Default`: compact, medium, expanded (the classes before 1.2.0's large ones).
const DP_WIDTH_SIZE_CLASSES_DEFAULT: &[Dp] =
    &[dp_width_size_classes::Compact, dp_width_size_classes::Medium, dp_width_size_classes::Expanded];
/// `DpWidthSizeClasses.DefaultV2`: the default ones and large, extra large.
const DP_WIDTH_SIZE_CLASSES_DEFAULT_V2: &[Dp] = &[
    dp_width_size_classes::Compact,
    dp_width_size_classes::Medium,
    dp_width_size_classes::Expanded,
    dp_width_size_classes::Large,
    dp_width_size_classes::ExtraLarge,
];
/// `DpHeightSizeClasses.Default`.
const DP_HEIGHT_SIZE_CLASSES_DEFAULT: &[Dp] =
    &[dp_height_size_classes::Compact, dp_height_size_classes::Medium, dp_height_size_classes::Expanded];

/// The window size class of a size: each bound the largest supported one the size reaches (Compose's internal
/// `WindowSizeClass.Companion.computeFromDpSize`).
fn compute_from_dp_size(window_size: DpSize, supported_width: &[Dp], supported_height: &[Dp]) -> WindowSizeClass {
    let max_of = |classes: &[Dp], size: Dp| {
        classes
            .iter()
            .filter(|c| size >= **c)
            .map(|c| c.0)
            .reduce(f32::max)
            .expect("Collection contains no element matching the predicate.")
    };
    WindowSizeClass::of_float(max_of(supported_width, window_size.width), max_of(supported_height, window_size.height))
}

/// The same with the large widths (Compose's internal `computeFromDpSizeV2`).
fn compute_from_dp_size_v2(window_size: DpSize) -> WindowSizeClass {
    compute_from_dp_size(window_size, DP_WIDTH_SIZE_CLASSES_DEFAULT_V2, DP_HEIGHT_SIZE_CLASSES_DEFAULT)
}
