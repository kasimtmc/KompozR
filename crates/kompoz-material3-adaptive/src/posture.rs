//! A foldable's posture, as Compose's adaptive Posture.kt (material3-adaptive 1.2.0): [`Posture`] (tabletop or
//! not, the hinges) and [`HingeInfo`], the hinge bounds a layout keeps clear of ([`PostureHingeBounds`]).

use kompoz_graphics::Rect;
use kompoz_runtime::kt::KtToString;
use std::fmt;

/// What a foldable's posture tells a layout (Compose's adaptive `Posture`): whether the window is in tabletop mode
/// (one half-opened horizontal hinge across its middle) and the hinges relevant to it, their bounds in window
/// coordinates. [`current_window_adaptive_info`](crate::current_window_adaptive_info) gives the window's; on the
/// desktop it is always the default (no hinges, not tabletop), as Compose for desktop's. Equal by its values.
// (WP8 navsuite: KotlinProperties — Kotlin reads `windowPosture.isTabletop`)
#[derive(Clone, PartialEq, Debug, Default, kompoz_runtime::KotlinProperties)]
pub struct Posture {
    /// Whether the window is considered in tabletop mode: one half-opened horizontal hinge in its middle (users
    /// can hardly reach the area around it: keep the keyboard and controls in the bottom half).
    pub is_tabletop: bool,
    /// All hinges that are relevant to the posture.
    pub hinge_list: Vec<HingeInfo>,
}

/// Kotlin: `Posture(isTabletop: Boolean, hingeList: List<HingeInfo>)`
#[kompoz_runtime::named(type)]
pub fn Posture(#[default(false)] is_tabletop: bool, #[default(Vec::new())] hinge_list: Vec<HingeInfo>) -> Posture {
    Posture { is_tabletop, hinge_list }
}

/// Kotlin's `toString`: `Posture(isTabletop=false, hinges=[…])`.
impl fmt::Display for Posture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hinges: Vec<String> = self.hinge_list.iter().map(|h| h.to_string()).collect();
        write!(f, "Posture(isTabletop={}, hinges=[{}])", self.is_tabletop, hinges.join(", "))
    }
}

impl KtToString for Posture {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// The bounds of a posture's hinges by kind (Compose's extension properties of `Posture`).
pub trait PostureHingeBounds {
    /// The vertical hinges' bounds that are separating.
    fn separating_vertical_hinge_bounds(&self) -> Vec<Rect>;
    /// The vertical hinges' bounds that are occluding.
    fn occluding_vertical_hinge_bounds(&self) -> Vec<Rect>;
    /// All vertical hinges' bounds.
    fn all_vertical_hinge_bounds(&self) -> Vec<Rect>;
    /// The horizontal hinges' bounds that are separating.
    fn separating_horizontal_hinge_bounds(&self) -> Vec<Rect>;
    /// The horizontal hinges' bounds that are occluding.
    fn occluding_horizontal_hinge_bounds(&self) -> Vec<Rect>;
    /// All horizontal hinges' bounds.
    fn all_horizontal_hinge_bounds(&self) -> Vec<Rect>;
}

fn get_bounds(hinges: &[HingeInfo], predicate: impl Fn(&HingeInfo) -> bool) -> Vec<Rect> {
    hinges.iter().filter(|h| predicate(h)).map(|h| h.bounds).collect()
}

impl PostureHingeBounds for Posture {
    fn separating_vertical_hinge_bounds(&self) -> Vec<Rect> {
        get_bounds(&self.hinge_list, |h| h.is_vertical && h.is_separating)
    }
    fn occluding_vertical_hinge_bounds(&self) -> Vec<Rect> {
        get_bounds(&self.hinge_list, |h| h.is_vertical && h.is_occluding)
    }
    fn all_vertical_hinge_bounds(&self) -> Vec<Rect> {
        get_bounds(&self.hinge_list, |h| h.is_vertical)
    }
    fn separating_horizontal_hinge_bounds(&self) -> Vec<Rect> {
        get_bounds(&self.hinge_list, |h| !h.is_vertical && h.is_separating)
    }
    fn occluding_horizontal_hinge_bounds(&self) -> Vec<Rect> {
        get_bounds(&self.hinge_list, |h| !h.is_vertical && h.is_occluding)
    }
    fn all_horizontal_hinge_bounds(&self) -> Vec<Rect> {
        get_bounds(&self.hinge_list, |h| !h.is_vertical)
    }
}

/// A hinge relevant to a [`Posture`] (Compose's `HingeInfo`). Equal by its values.
#[derive(Clone, Copy, PartialEq, Debug, kompoz_runtime::KotlinProperties)]
pub struct HingeInfo {
    /// The bounds of the hinge in the relevant viewport.
    pub bounds: Rect,
    /// Whether the hinge is fully open and the window space presented to the user is flat.
    pub is_flat: bool,
    /// Whether the hinge is vertical (separating the viewport into left and right) rather than horizontal.
    pub is_vertical: bool,
    /// Whether the hinge creates two logical display areas.
    pub is_separating: bool,
    /// Whether the hinge conceals part of the display.
    pub is_occluding: bool,
}

/// Kotlin: `HingeInfo(bounds: Rect, isFlat: Boolean, isVertical: Boolean, isSeparating: Boolean, isOccluding: Boolean)`
pub fn HingeInfo(bounds: Rect, is_flat: bool, is_vertical: bool, is_separating: bool, is_occluding: bool) -> HingeInfo {
    HingeInfo { bounds, is_flat, is_vertical, is_separating, is_occluding }
}

/// Kotlin's `toString`: `HingeInfo(bounds=Rect.fromLTRB(…), isFlat=…, isVertical=…, isSeparating=…, isOccluding=…)`.
impl fmt::Display for HingeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HingeInfo(bounds={}, isFlat={}, isVertical={}, isSeparating={}, isOccluding={})",
            self.bounds.kt_to_string(),
            self.is_flat,
            self.is_vertical,
            self.is_separating,
            self.is_occluding
        )
    }
}

impl KtToString for HingeInfo {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}
