//! The cursor over a layout, as Compose's `PointerIcon.kt`: `Modifier.pointer_hover_icon` (and
//! `stylus_hover_icon`) shows its icon while the pointer is over it — unless a node inside it, the pointer over
//! that too, shows its own, or one around it overrides its descendants — through the platform's callbacks.

use super::hit_test::{DpTouchBoundsExpansion, TouchBoundsExpansion};
use super::{
    PointerEvent, PointerEventPass, PointerEventType, PointerInputModifierNode, PointerType, when_node_released,
};
use crate::modifier::{
    Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, TraversableNode, TraversalDecision,
};
use crate::node_capabilities;
use crate::platform::{LocalPlatform, PlatformHandle, PointerIcon};
use kompoz_graphics::IntSize;
use kompoz_runtime::named;
use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

static POINTER_HOVER_ICON_KEY: &str = "androidx.compose.ui.input.pointer.PointerHoverIcon";
static STYLUS_HOVER_ICON_KEY: &str = "androidx.compose.ui.input.pointer.StylusHoverIcon";

/// Where hover icons are shown (Compose's PointerIconService): the platform's cursor, and its stylus's.
#[derive(Clone)]
pub struct PointerIconService(PlatformHandle);

impl PointerIconService {
    /// The service of the host `cx`'s node is in (LocalPlatform where it was composed).
    pub fn of(cx: &NodeCx) -> PointerIconService {
        PointerIconService(cx.current_value_of(&LocalPlatform))
    }
    /// Shows `icon`, none for the default one.
    pub fn set_icon(&self, icon: Option<PointerIcon>) {
        self.0.get().cursor(icon.unwrap_or_default());
    }
    /// Shows `icon` for a hovering stylus, none for the platform's own.
    pub fn set_stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        self.0.get().stylus_hover_icon(icon);
    }
}

/// A hover icon node's state, shared so other nodes' traversals read it.
struct HoverState {
    icon: Cell<PointerIcon>,
    override_descendants: Cell<bool>,
    cursor_in_bounds: Cell<bool>,
    stylus: bool,
}

impl HoverState {
    fn key(&self) -> &'static dyn Any {
        if self.stylus { &STYLUS_HOVER_ICON_KEY } else { &POINTER_HOVER_ICON_KEY }
    }
    fn is_relevant(&self, r#type: PointerType) -> bool {
        let stylus = r#type == PointerType::Stylus || r#type == PointerType::Eraser;
        stylus == self.stylus
    }
}

/// The node of `pointer_hover_icon` and `stylus_hover_icon` (Compose's HoverIconModifierNode).
pub struct HoverIconModifierNode {
    state: Rc<HoverState>,
    handle: Option<NodeHandle>,
    dp_touch_bounds_expansion: Option<DpTouchBoundsExpansion>,
}

fn state_of(cx: &NodeCx) -> Option<Rc<HoverState>> {
    cx.with::<HoverIconModifierNode, _>(|n| n.state.clone())
}

fn show(cx: &NodeCx, state: &HoverState, icon: Option<PointerIcon>) {
    let service = PointerIconService::of(cx);
    if state.stylus { service.set_stylus_hover_icon(icon) } else { service.set_icon(icon) }
}

/// Compose's `displayIcon()`: its icon, or an ancestor's that overrides it.
fn display_icon(cx: &NodeCx, state: &Rc<HoverState>) {
    let icon = find_overriding_ancestor(cx, state).map_or(state.icon.get(), |s| s.icon.get());
    show(cx, state, Some(icon));
}

fn on_enter(cx: &NodeCx, state: &Rc<HoverState>) {
    state.cursor_in_bounds.set(true);
    display_icon_if_descendants_do_not_have_priority(cx, state);
}

fn on_exit(cx: &NodeCx, state: &Rc<HoverState>) {
    if state.cursor_in_bounds.get() {
        state.cursor_in_bounds.set(false);
        if cx.is_attached() {
            display_icon_from_ancestor_node_with_cursor_in_bounds_or_default_icon(cx, state);
        }
    }
}

fn display_icon_if_descendants_do_not_have_priority(cx: &NodeCx, state: &Rc<HoverState>) {
    let mut rights = true;
    if !state.override_descendants.get() {
        cx.traverse_descendants(state.key(), &mut |d| {
            if state_of(d).is_some_and(|s| s.cursor_in_bounds.get()) {
                rights = false;
                TraversalDecision::CancelTraversal
            } else {
                TraversalDecision::ContinueTraversal
            }
        });
    }
    if rights {
        display_icon(cx, state);
    }
}

fn find_descendant_node_with_cursor_in_bounds(cx: &NodeCx, state: &HoverState) -> Option<(NodeCx, Rc<HoverState>)> {
    let mut found = None;
    cx.traverse_descendants(state.key(), &mut |d| {
        let mut action = TraversalDecision::ContinueTraversal;
        if let Some(s) = state_of(d)
            && s.cursor_in_bounds.get()
        {
            if s.override_descendants.get() {
                action = TraversalDecision::SkipSubtreeAndContinueTraversal;
            }
            found = Some((d.clone(), s));
        }
        action
    });
    found
}

fn display_icon_from_current_node_or_descendants_with_cursor_in_bounds(cx: &NodeCx, state: &Rc<HoverState>) {
    if !state.cursor_in_bounds.get() {
        return;
    }
    let found =
        if state.override_descendants.get() { None } else { find_descendant_node_with_cursor_in_bounds(cx, state) };
    match found {
        Some((c, s)) => display_icon(&c, &s),
        None => display_icon(cx, state),
    }
}

fn find_overriding_ancestor(cx: &NodeCx, state: &HoverState) -> Option<Rc<HoverState>> {
    let mut found = None;
    cx.traverse_ancestors(state.key(), &mut |a| {
        if let Some(s) = state_of(a)
            && s.override_descendants.get()
            && s.cursor_in_bounds.get()
        {
            found = Some(s);
        }
        true
    });
    found
}

fn display_icon_from_ancestor_node_with_cursor_in_bounds_or_default_icon(cx: &NodeCx, state: &Rc<HoverState>) {
    let mut found: Option<(NodeCx, Rc<HoverState>)> = None;
    cx.traverse_ancestors(state.key(), &mut |a| {
        // The nearest with the pointer in it, unless one further out overrides it.
        if let Some(s) = state_of(a)
            && s.cursor_in_bounds.get()
            && (found.is_none() || s.override_descendants.get())
        {
            found = Some((a.clone(), s));
        }
        true
    });
    match found {
        Some((c, s)) => display_icon(&c, &s),
        None => show(cx, state, None),
    }
}

impl HoverIconModifierNode {
    fn new(
        icon: PointerIcon,
        override_descendants: bool,
        stylus: bool,
        expansion: Option<DpTouchBoundsExpansion>,
    ) -> Self {
        HoverIconModifierNode {
            state: Rc::new(HoverState {
                icon: Cell::new(icon),
                override_descendants: Cell::new(override_descendants),
                cursor_in_bounds: Cell::new(false),
                stylus,
            }),
            handle: None,
            dp_touch_bounds_expansion: expansion,
        }
    }

    /// Runs `f` with its context once nothing is borrowed (after the composition's changes are applied).
    fn later(&self, f: impl FnOnce(&NodeCx, &Rc<HoverState>) + 'static) {
        let (Some(handle), state) = (self.handle.clone(), self.state.clone()) else { return };
        if let Some(cx) = handle.cx() {
            let h = handle.clone();
            cx.side_effect(move || {
                if let Some(cx) = h.cx() {
                    f(&cx, &state);
                }
            });
        }
    }

    pub fn icon(&self) -> PointerIcon {
        self.state.icon.get()
    }

    /// Its icon set; shown now if the pointer is over it and nothing inside has priority.
    pub fn set_icon(&mut self, icon: PointerIcon) {
        if self.state.icon.get() != icon {
            self.state.icon.set(icon);
            if self.state.cursor_in_bounds.get() {
                self.later(display_icon_if_descendants_do_not_have_priority);
            }
        }
    }

    pub fn override_descendants(&self) -> bool {
        self.state.override_descendants.get()
    }

    /// Whether it overrides what is inside it; the icon shown follows.
    pub fn set_override_descendants(&mut self, override_descendants: bool) {
        if self.state.override_descendants.get() != override_descendants {
            self.state.override_descendants.set(override_descendants);
            if self.state.cursor_in_bounds.get() {
                if override_descendants {
                    self.later(display_icon);
                } else {
                    self.later(display_icon_from_current_node_or_descendants_with_cursor_in_bounds);
                }
            }
        }
    }

    /// Whether the pointer is over it.
    pub fn is_cursor_in_bounds(&self) -> bool {
        self.state.cursor_in_bounds.get()
    }
}

impl ModifierNode for HoverIconModifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        on_exit(cx, &self.state);
    }

    node_capabilities!(pointer_input, traversable);
}

impl TraversableNode for HoverIconModifierNode {
    fn traverse_key(&self) -> &dyn Any {
        self.state.key()
    }
}

impl PointerInputModifierNode for HoverIconModifierNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, _bounds: IntSize) {
        if pass != PointerEventPass::Main || !event.changes.iter().any(|c| self.state.is_relevant(c.r#type)) {
            return;
        }
        let enter = match event.r#type {
            PointerEventType::Enter => true,
            PointerEventType::Exit => false,
            _ => return,
        };
        let (Some(handle), state) = (self.handle.clone(), self.state.clone()) else { return };
        when_node_released(move || {
            if let Some(cx) = handle.cx() {
                if enter { on_enter(&cx, &state) } else { on_exit(&cx, &state) }
            }
        });
    }

    fn on_cancel_pointer_input(&mut self) {
        let (Some(handle), state) = (self.handle.clone(), self.state.clone()) else { return };
        when_node_released(move || {
            if let Some(cx) = handle.cx() {
                on_exit(&cx, &state);
            }
        });
    }

    fn touch_bounds_expansion(&self) -> TouchBoundsExpansion {
        match (&self.dp_touch_bounds_expansion, &self.handle) {
            (Some(e), Some(h)) => e.round_to_touch_bounds_expansion(&h.density()),
            _ => TouchBoundsExpansion::None,
        }
    }
}

#[derive(PartialEq)]
struct HoverIconElement {
    icon: PointerIcon,
    override_descendants: bool,
    stylus: bool,
    touch_bounds_expansion: Option<DpTouchBoundsExpansion>,
}

impl ModifierNodeElement for HoverIconElement {
    type Node = HoverIconModifierNode;
    fn create(&self) -> HoverIconModifierNode {
        HoverIconModifierNode::new(self.icon, self.override_descendants, self.stylus, self.touch_bounds_expansion)
    }
    fn update(&self, node: &mut HoverIconModifierNode) {
        node.set_icon(self.icon);
        node.set_override_descendants(self.override_descendants);
        node.dp_touch_bounds_expansion = self.touch_bounds_expansion;
    }
    fn name(&self) -> &'static str {
        if self.stylus { "stylusHoverIcon" } else { "pointerHoverIcon" }
    }
}

#[named]
impl Modifier {
    /// Shows `icon` as the cursor while the mouse is over the layout (Compose's `Modifier.pointerHoverIcon`): a
    /// node inside with its own icon wins while the mouse is over it, unless `override_descendants`.
    pub fn pointer_hover_icon(self, icon: PointerIcon, #[default(false)] override_descendants: bool) -> Modifier {
        self.then_element(HoverIconElement { icon, override_descendants, stylus: false, touch_bounds_expansion: None })
    }

    /// The same for a hovering stylus, which also hits the layout `touch_bounds_expansion` past its bounds
    /// (Compose's `Modifier.stylusHoverIcon`).
    pub fn stylus_hover_icon(
        self,
        icon: PointerIcon,
        #[default(false)] override_descendants: bool,
        #[default(None)] touch_bounds_expansion: Option<DpTouchBoundsExpansion>,
    ) -> Modifier {
        self.then_element(HoverIconElement { icon, override_descendants, stylus: true, touch_bounds_expansion })
    }
}
