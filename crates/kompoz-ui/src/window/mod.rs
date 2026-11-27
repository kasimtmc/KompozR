//! Windows of the UI's own, as Compose's `androidx.compose.ui.window` ([`Popup`], [`Dialog`]) with C++'s overlay
//! mechanism: a popup's root layout node is emitted inline where it is composed (so composition locals flow into
//! it), split out of its anchor's children into the anchor's overlays, laid out against the host's window after the
//! main tree (its position worked out from its anchor's bounds), drawn after the main tree and hit first. A
//! focusable popup or a dialog on top takes the focus search, the keys, the back handlers and the accessibility
//! tree; a press outside it dismisses it (if it says so) and goes nowhere else. A non-focusable popup lets input
//! outside it through (a press outside still dismisses it).
//!
//! A popup's coordinates are the host window's (C++'s): its content's `bounds_in_root` are where it is on the
//! window.
//!
//! Windows of the desktop's (Compose Desktop's `Window`, `WindowState`, `application`'s `ApplicationScope`:
//! [`application`]; `singleWindowApplication`'s scope), its tray icons and notifications ([`tray`]), their menus and
//! a window's menu bar ([`menu`], [`FrameWindowScope::MenuBar`]) are asked for here and shown by the platform's app
//! core.

pub mod application;
mod desktop_popup;
mod dialog;
pub mod menu;
mod popup;
pub mod tray;

pub use application::{
    ApplicationControl, ApplicationScope, ComposeWindow, FrameWindowScope, LocalWindowRegistry,
    SingleWindowApplicationScope, Window, WindowContent, WindowId, WindowPlacement, WindowPosition, WindowPositionArgs,
    WindowProperties, WindowRegistry, WindowRequest, WindowScope, WindowState, exit_application, is_tray_supported,
    remember_window_state, set_application_control,
};
pub use menu::{
    KeyShortcut, MenuBarScope, MenuItem, MenuItemKind, MenuScope, item_at, same_items, shortcut_item, shortcut_matches,
};
pub use tray::{
    Notification, NotificationType, Tray, TrayRequest, TrayState, remember_notification, remember_tray_state,
};

pub use desktop_popup::{
    PopupPositionProviderAtPosition, remember_component_rect_position_provider, remember_cursor_position_provider,
    remember_popup_position_provider_at_position,
};
pub use dialog::{Dialog, DialogProperties};
pub use popup::{AlignmentOffsetPositionProvider, Popup, PopupPositionProvider, PopupProperties};

use crate::graphics_context::GraphicsContext;
use crate::host::KompozHost;
use crate::node::{LayoutNode, Owner};
use crate::unit::Constraints;
use kompoz_graphics::{IntOffset, IntRect, IntSize, LayoutDirection, Offset, Rect};
use std::rc::{Rc, Weak};

/// What a popup root is (C++'s OverlaySpec): a popup placed by its provider, or a dialog over the whole window;
/// what dismisses it; the back scope of its handlers.
pub(crate) struct PopupSpec {
    pub(crate) kind: PopupKind,
    pub(crate) on_dismiss_request: Rc<dyn Fn()>,
    /// Its window's navigation (a focusable popup's, a dialog's): where back goes while it is on top.
    pub(crate) navigation: Option<Rc<crate::navigationevent::WindowNavigation>>,
    /// The layout direction where it was composed (its provider's).
    pub(crate) layout_direction: LayoutDirection,
}

pub(crate) enum PopupKind {
    Popup { provider: Rc<dyn PopupPositionProvider>, properties: PopupProperties },
    Dialog { properties: DialogProperties },
}

impl PopupSpec {
    /// Whether it takes the focus, the keys and back while on top (a focusable popup, a dialog).
    pub(crate) fn focusable(&self) -> bool {
        match &self.kind {
            PopupKind::Popup { properties, .. } => properties.focusable,
            PopupKind::Dialog { .. } => true,
        }
    }
    pub(crate) fn dismiss_on_click_outside(&self) -> bool {
        match &self.kind {
            PopupKind::Popup { properties, .. } => properties.dismiss_on_click_outside,
            PopupKind::Dialog { properties } => properties.dismiss_on_click_outside,
        }
    }
    fn is_dialog(&self) -> bool {
        matches!(self.kind, PopupKind::Dialog { .. })
    }
}

/// The overlays of a host: the roots of its popups and dialogs, bottom first.
pub struct Overlays {
    pub(crate) owner: Weak<Owner>,
}

impl Overlays {
    /// The overlays' roots that are in the tree, bottom first (a popup above the window it was opened from, each
    /// after the popups of its anchor's subtree).
    pub fn roots(&self) -> Vec<Rc<LayoutNode>> {
        self.owner.upgrade().map(|o| o.popup_roots()).unwrap_or_default()
    }

    /// How many there are (C++'s `overlayCount`).
    pub fn len(&self) -> usize {
        self.roots().len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots().is_empty()
    }

    /// Draws them over the main tree, bottom first.
    pub(crate) fn draw(&self, canvas: &mut dyn GraphicsContext) {
        for root in self.roots() {
            if root.is_placed() {
                root.draw(canvas);
            }
        }
    }
}

impl LayoutNode {
    /// Whether it is a popup's or a dialog's root.
    pub fn is_popup_root(&self) -> bool {
        self.popup.borrow().is_some()
    }

    /// Whether it is the root of a focusable popup or a dialog (what is below it gets no input).
    pub fn is_modal_popup_root(&self) -> bool {
        self.popup.borrow().as_ref().is_some_and(|s| s.focusable())
    }

    /// The node it is a popup of (the node it was composed in), if it is a popup's root.
    pub fn anchor(&self) -> Option<Rc<LayoutNode>> {
        self.anchor.borrow().upgrade()
    }

    /// The roots of the popups composed in it (not its children).
    pub fn overlays(&self) -> Vec<Rc<LayoutNode>> {
        self.overlays.borrow().clone()
    }

    pub(crate) fn popup_spec(&self) -> Option<Rc<PopupSpec>> {
        self.popup.borrow().clone()
    }

    pub(crate) fn set_popup_spec(&self, spec: Rc<PopupSpec>) {
        *self.popup.borrow_mut() = Some(spec);
        self.request_relayout();
    }

    /// The root of the window it is in: the host's root, or a popup's root.
    pub fn window_root(self: &Rc<Self>) -> Rc<LayoutNode> {
        let mut n = self.clone();
        while let Some(p) = n.parent() {
            n = p;
        }
        n
    }

    /// The path from the host's root to it through anchors: children `(0, index)`, popups `(1, index)` (what orders
    /// the overlays).
    fn tree_path(self: &Rc<Self>) -> Vec<(u8, usize)> {
        let mut path = Vec::new();
        let mut n = self.clone();
        loop {
            if let Some(p) = n.parent() {
                let i = p.children().iter().position(|c| Rc::ptr_eq(c, &n)).unwrap_or(0);
                path.push((0, i));
                n = p;
            } else if let Some(a) = n.anchor() {
                let i = a.overlays().iter().position(|c| Rc::ptr_eq(c, &n)).unwrap_or(0);
                path.push((1, i));
                n = a;
            } else {
                break;
            }
        }
        path.reverse();
        path
    }

    /// Splits the nodes emitted below it: popup roots go to its overlays (attached and detached with it, their
    /// anchor this), the rest are its children (what `set_children` then takes).
    pub(crate) fn split_overlays(self: &Rc<Self>, emitted: Vec<Rc<LayoutNode>>) -> Vec<Rc<LayoutNode>> {
        let (popups, children): (Vec<_>, Vec<_>) = emitted.into_iter().partition(|n| n.is_popup_root());
        if popups.is_empty() && self.overlays.borrow().is_empty() {
            return children;
        }
        let old = self.overlays();
        for o in &old {
            if !popups.iter().any(|p| Rc::ptr_eq(p, o)) {
                if o.is_attached() {
                    o.detach();
                }
                *o.anchor.borrow_mut() = Weak::new();
            }
        }
        *self.overlays.borrow_mut() = popups.clone();
        for p in &popups {
            if !old.iter().any(|o| Rc::ptr_eq(o, p)) {
                *p.anchor.borrow_mut() = Rc::downgrade(self);
                if let Some(owner) = self.owner() {
                    p.attach(&owner);
                }
            }
        }
        if let Some(owner) = self.owner() {
            owner.sort_popups();
        }
        children
    }
}

impl Owner {
    /// The popup roots attached to it, bottom first.
    pub(crate) fn popup_roots(&self) -> Vec<Rc<LayoutNode>> {
        self.popups.borrow().iter().filter_map(Weak::upgrade).filter(|n| n.is_attached()).collect()
    }

    pub(crate) fn has_popups(&self) -> bool {
        !self.popups.borrow().is_empty()
    }

    /// A popup root came into the tree.
    pub(crate) fn register_popup(&self, node: &Rc<LayoutNode>) {
        self.popups.borrow_mut().push(Rc::downgrade(node));
        self.sort_popups();
        self.semantics_changed();
    }

    /// A popup root left.
    pub(crate) fn unregister_popup(&self, node: &LayoutNode) {
        self.popups.borrow_mut().retain(|w| w.strong_count() > 0 && !std::ptr::eq(w.as_ptr(), node));
        self.request_draw();
        self.semantics_changed();
    }

    /// Orders the popup roots bottom first (by their paths through their anchors).
    pub(crate) fn sort_popups(&self) {
        let mut roots: Vec<(Vec<(u8, usize)>, Rc<LayoutNode>)> =
            self.popup_roots().into_iter().map(|n| (n.tree_path(), n)).collect();
        roots.sort_by(|a, b| a.0.cmp(&b.0));
        *self.popups.borrow_mut() = roots.into_iter().map(|(_, n)| Rc::downgrade(&n)).collect();
    }

    /// The topmost focusable popup's or dialog's root (C++'s `inputOverlay`).
    pub(crate) fn top_focusable_popup(&self) -> Option<Rc<LayoutNode>> {
        self.popup_roots().into_iter().rev().find(|r| r.popup_spec().is_some_and(|s| s.focusable()))
    }

    /// The window's size: what the root is measured within.
    pub(crate) fn window_size(&self) -> IntSize {
        self.root_constraints().map_or(IntSize::Zero, |c| IntSize::new(c.max_width, c.max_height))
    }

    /// The constraints a popup root is measured with: the window's (a dialog: exactly).
    pub(crate) fn popup_constraints(&self, node: &LayoutNode) -> Option<Constraints> {
        let spec = node.popup_spec()?;
        let w = self.window_size();
        Some(if spec.is_dialog() {
            Constraints::fixed(w.width, w.height)
        } else {
            Constraints::new(0, w.width, 0, w.height)
        })
    }

    /// Lays the popups out after the main tree (C++'s `layoutOverlays`), bottom first: each measured in the
    /// window, placed where its provider puts it by its anchor's bounds in the window (what the provider reads
    /// observed: a change places it again), kept in the window when it clips.
    pub(crate) fn layout_popups(&self) {
        for popup in self.popup_roots() {
            let Some(spec) = popup.popup_spec() else { continue };
            let Some(constraints) = self.popup_constraints(&popup) else { continue };
            if popup.measure_pending.get() || popup.last_constraints.get() != Some(constraints) {
                self.remeasure_popup(&popup, constraints);
            }
            let size = popup.measured_size.get();
            let position = match &spec.kind {
                PopupKind::Dialog { .. } => IntOffset::Zero,
                PopupKind::Popup { provider, properties } => {
                    // (the anchor's content: Compose's popup takes its parent layout coordinates)
                    let anchor_bounds = popup.anchor().filter(|a| a.is_attached() && a.is_placed()).map(|a| {
                        let c = a.inner_coordinates();
                        let at = c.position_in_root();
                        IntRect::from_offset_size(IntOffset::new(at.x.round() as i32, at.y.round() as i32), c.size())
                    });
                    let Some(anchor_bounds) = anchor_bounds else { continue };
                    let window = self.window_size();
                    let provider = provider.clone();
                    let direction = spec.layout_direction;
                    let scope = popup.reader_scope(&popup.layout_reads, |n| n.request_relayout());
                    let mut p = self
                        .snapshot_observer()
                        .observe(&scope, || provider.calculate_position(anchor_bounds, window, direction, size));
                    if properties.clipping_enabled {
                        p.x = p.x.min(window.width - size.width).max(0);
                        p.y = p.y.min(window.height - size.height).max(0);
                    }
                    p
                }
            };
            if popup.layout_pending.get() || !popup.placed_once.get() || popup.last_position.get() != position {
                popup.is_placed_by_parent.set(true);
                popup.place_outer_coordinator(position, 0.0, None);
                self.relaid_out.set(self.relaid_out.get() + 1);
                self.request_on_positioned_callback(&popup);
                self.semantics_bounds_changed();
            }
        }
    }
}

/// Whether a press at `position` (the host's window) is outside the popups on top (C++'s pointerDown overlay
/// check): the non-focusable popups above the first focusable one it is outside of are dismissed (if they say
/// so) and the press goes on; the first focusable one (a dialog too) it is outside of is dismissed if it says so,
/// and the press goes nowhere else (true). A press inside a focusable popup goes to it.
pub(crate) fn press_outside_popup(host: &KompozHost, position: Offset) -> bool {
    let roots = host.overlays.roots();
    for root in roots.into_iter().rev() {
        let Some(spec) = root.popup_spec() else { continue };
        if !root.is_placed() {
            continue;
        }
        let inside = dismiss_bounds(&root, &spec).contains(position);
        if spec.focusable() {
            if inside {
                return false;
            }
            if spec.dismiss_on_click_outside() {
                let dismiss = spec.on_dismiss_request.clone();
                dismiss();
            }
            return true;
        }
        if !inside && spec.dismiss_on_click_outside() {
            let dismiss = spec.on_dismiss_request.clone();
            dismiss();
        }
    }
    false
}

/// What counts as inside a popup: its bounds; a dialog's, its content's.
fn dismiss_bounds(root: &Rc<LayoutNode>, spec: &PopupSpec) -> Rect {
    let node = if spec.is_dialog() { root.children().into_iter().next() } else { Some(root.clone()) };
    match node {
        Some(n) if n.is_placed() => n.coordinates().bounds_in_root(),
        _ => Rect::Zero,
    }
}

impl Owner {
    fn remeasure_popup(&self, popup: &Rc<LayoutNode>, constraints: Constraints) {
        if popup.intrinsics_usage_by_parent.get() == crate::node::UsageByParent::NotUsed {
            popup.clear_subtree_intrinsics_usage();
        }
        popup.remeasure(constraints);
        self.relaid_out.set(self.relaid_out.get() + 1);
    }
}
