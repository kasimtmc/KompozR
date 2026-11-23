//! A pane's margins, as Compose's internal (unpublished) PaneMargins.kt (adaptive-layout 1.2.0): fixed margins and
//! window insets given as rulers that a scaffold keeps a pane's edges inside ([`pane_margins`], [`PaneMargins`],
//! [`WindowInsetsRulers`]), through the pane's parent data.

// (Compose keeps it internal and unpublished; the scaffold places a pane by it through its parent data)
#![allow(dead_code)]

use super::pane_scaffold::modify_pane_scaffold_parent_data;
use kompoz_foundation::layout::PaddingValues;
use kompoz_graphics::{Density, LayoutDirection};
use kompoz_ui::layout::{HorizontalRuler, ParentDataModifierNode, PlacementScope, VerticalRuler};
use kompoz_ui::modifier::{ModifierNode, ModifierNodeElement};
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::{Modifier, node_capabilities};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// `modifier` keeping the pane inside `window_insets` (Compose's internal composable `Modifier.paneMargins(vararg
/// windowInsets)`): [`pane_margins_with_fixed_margins`] of no fixed margins.
// TODO(conradchen): move the modifier declarations to PaneScaffoldPaneScope when we can publish it.
#[allow(dead_code)]
#[track_caller]
pub(crate) fn pane_margins(modifier: Modifier, window_insets: &[WindowInsetsRulers]) -> Modifier {
    pane_margins_with_fixed_margins(modifier, PaddingValues::all(kompoz_graphics::Dp(0.0)), window_insets)
}

/// `modifier` keeping the pane inside `fixed_margins` and `window_insets` (Compose's internal composable
/// `Modifier.paneMargins(fixedMargins, vararg windowInsets)`): the margins in the density and layout direction where
/// it is composed, as the pane's parent data.
// TODO(conradchen): move the modifier declarations to PaneScaffoldPaneScope when we can publish it.
#[allow(dead_code)]
#[track_caller]
pub(crate) fn pane_margins_with_fixed_margins(
    modifier: Modifier,
    fixed_margins: PaddingValues,
    window_insets: &[WindowInsetsRulers],
) -> Modifier {
    let (density, layout_direction) =
        kompoz_runtime::kompozable_block!((LocalDensity.current(), LocalLayoutDirection.current()));
    let pane_margins: Rc<dyn PaneMargins> =
        Rc::new(PaneMarginsImpl::new(fixed_margins, window_insets.to_vec(), &density, layout_direction));
    modifier.then_element(PaneMarginsElement { pane_margins })
}

/// Compose's private data class `PaneMarginsElement` (equal by its margins: by identity, as Kotlin's class
/// without `equals`).
struct PaneMarginsElement {
    pane_margins: Rc<dyn PaneMargins>,
}

impl PartialEq for PaneMarginsElement {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.pane_margins), Rc::as_ptr(&other.pane_margins))
    }
}

impl ModifierNodeElement for PaneMarginsElement {
    type Node = PaneMarginsNode;

    fn create(&self) -> PaneMarginsNode {
        PaneMarginsNode { pane_margins: self.pane_margins.clone() }
    }

    fn update(&self, node: &mut PaneMarginsNode) {
        node.pane_margins = self.pane_margins.clone();
    }

    fn name(&self) -> &'static str {
        "paneMargins"
    }
}

/// Compose's private `PaneMarginsNode`: the margins as the pane's parent data.
struct PaneMarginsNode {
    pane_margins: Rc<dyn PaneMargins>,
}

impl ModifierNode for PaneMarginsNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for PaneMarginsNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let pane_margins = self.pane_margins.clone();
        modify_pane_scaffold_parent_data(parent_data, |d| d.pane_margins = pane_margins)
    }
}

/// Where a pane's edges may go (Compose's internal interface `PaneMargins`): each edge from where it was measured
/// and where the parent's edge is, while placing; by default where it was measured.
pub(crate) trait PaneMargins {
    /// The pane's left edge.
    fn get_pane_left(&self, _p: &PlacementScope, measured_left: i32) -> i32 {
        measured_left
    }

    /// The pane's top edge.
    fn get_pane_top(&self, _p: &PlacementScope, measured_top: i32) -> i32 {
        measured_top
    }

    /// The pane's right edge.
    fn get_pane_right(&self, _p: &PlacementScope, measured_right: i32, _parent_right: i32) -> i32 {
        measured_right
    }

    /// The pane's bottom edge.
    fn get_pane_bottom(&self, _p: &PlacementScope, measured_bottom: i32, _parent_bottom: i32) -> i32 {
        measured_bottom
    }
}

impl fmt::Debug for dyn PaneMargins {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PaneMargins@{:p}", self as *const dyn PaneMargins as *const ())
    }
}

/// No margins (Compose's companion's `PaneMargins.Unspecified`, one object).
pub(crate) fn unspecified_pane_margins() -> Rc<dyn PaneMargins> {
    struct Unspecified;
    impl PaneMargins for Unspecified {}
    thread_local! {
        static UNSPECIFIED: Rc<dyn PaneMargins> = Rc::new(Unspecified);
    }
    UNSPECIFIED.with(Rc::clone)
}

/// Fixed margins and window insets' rulers (Compose's internal class `PaneMarginsImpl`): each edge kept inside both
/// (the fixed margins rounded to px in the density, left and right by the layout direction).
pub(crate) struct PaneMarginsImpl {
    fixed_margin_left: i32,
    fixed_margin_top: i32,
    fixed_margin_right: i32,
    fixed_margin_bottom: i32,
    rulers: Vec<WindowInsetsRulers>,
}

impl PaneMarginsImpl {
    pub(crate) fn new(
        fixed_margins: PaddingValues,
        window_insets: Vec<WindowInsetsRulers>,
        density: &Density,
        layout_direction: LayoutDirection,
    ) -> PaneMarginsImpl {
        PaneMarginsImpl {
            fixed_margin_left: density.round_to_px(fixed_margins.calculate_left_padding(layout_direction)),
            fixed_margin_top: density.round_to_px(fixed_margins.calculate_top_padding()),
            fixed_margin_right: density.round_to_px(fixed_margins.calculate_right_padding(layout_direction)),
            fixed_margin_bottom: density.round_to_px(fixed_margins.calculate_bottom_padding()),
            rulers: window_insets,
        }
    }
}

/// Kotlin's `Float.roundToInt()`: half up.
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

impl PaneMargins for PaneMarginsImpl {
    fn get_pane_left(&self, p: &PlacementScope, measured_left: i32) -> i32 {
        let insets = self.rulers.iter().map(|r| round_to_int(r.left.current(p, 0.0))).max().unwrap_or(0);
        measured_left.max(self.fixed_margin_left).max(insets)
    }

    fn get_pane_top(&self, p: &PlacementScope, measured_top: i32) -> i32 {
        let insets = self.rulers.iter().map(|r| round_to_int(r.top.current(p, 0.0))).max().unwrap_or(0);
        measured_top.max(self.fixed_margin_top).max(insets)
    }

    fn get_pane_right(&self, p: &PlacementScope, measured_right: i32, parent_right: i32) -> i32 {
        let insets =
            self.rulers.iter().map(|r| round_to_int(r.right.current(p, f32::MAX))).min().unwrap_or(parent_right);
        measured_right.min(parent_right - self.fixed_margin_right).min(insets)
    }

    fn get_pane_bottom(&self, p: &PlacementScope, measured_bottom: i32, parent_bottom: i32) -> i32 {
        let insets =
            self.rulers.iter().map(|r| round_to_int(r.bottom.current(p, f32::MAX))).min().unwrap_or(parent_bottom);
        measured_bottom.min(parent_bottom - self.fixed_margin_bottom).min(insets)
    }
}

/// A window's insets as rulers (Compose's internal class `WindowInsetsRulers`, a stand-in of its own until
/// foundation's): one for each edge, which layouts provide. Its clones are the same rulers.
// TODO(conradchen): Move to use the foundation definition when it's available
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct WindowInsetsRulers {
    pub(crate) left: VerticalRuler,
    pub(crate) top: HorizontalRuler,
    pub(crate) right: VerticalRuler,
    pub(crate) bottom: HorizontalRuler,
}

impl WindowInsetsRulers {
    #[allow(dead_code)]
    pub(crate) fn new() -> WindowInsetsRulers {
        WindowInsetsRulers {
            left: VerticalRuler::new(),
            top: HorizontalRuler::new(),
            right: VerticalRuler::new(),
            bottom: HorizontalRuler::new(),
        }
    }
}
