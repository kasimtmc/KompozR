//! Stylus handwriting over a text field (Compose's `text/handwriting/StylusHandwriting.kt`): a stylus pressed on a
//! field (or near it: 40 dp above and below, 10 dp to the sides) that moves past the handwriting slop before a
//! long press would be one starts the input method's handwriting — the field takes the focus if it hasn't it, the
//! field's `on_handwriting_slop_exceeded` starts it, and the rest of the stylus's gesture is the input method's
//! (consumed here: nothing under it scrolls or selects). Only where the platform supports it
//! ([`is_stylus_handwriting_supported`]: Android 14 and later); a stylus hovering over the field shows the
//! handwriting icon where the field takes handwriting.

use kompoz_graphics::{Dp, IntSize, Offset};
use kompoz_runtime::named_args;
use kompoz_ui::focus::{FocusEventModifierNode, FocusState};
use kompoz_ui::input::{
    DpTouchBoundsExpansion, PointerEvent, PointerEventPass, PointerId, PointerInputModifierNode, PointerType,
    TouchBoundsExpansion, when_node_released,
};
use kompoz_ui::platform::{LocalViewConfiguration, PointerIcon, is_stylus_handwriting_supported};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::rc::Rc;

/// The handwriting icon (Compose's `handwritingPointerIcon`).
pub const HandwritingPointerIcon: PointerIcon = PointerIcon::Handwriting;

/// How far around a field a stylus still writes into it (Compose's `HandwritingBoundsExpansion`).
pub fn handwriting_bounds_expansion() -> DpTouchBoundsExpansion {
    DpTouchBoundsExpansion::new(Dp(10.0), Dp(40.0), Dp(10.0), Dp(40.0))
}

/// The stylus handwriting modifier (Compose's internal `Modifier.stylusHandwriting`).
pub trait StylusHandwritingModifiers {
    /// Starts stylus handwriting on the field: while `enabled` and the platform supports it, a stylus moving past
    /// the handwriting slop over the field focuses it and calls `on_handwriting_slop_exceeded`;
    /// `show_hover_icon`: a hovering stylus shows the handwriting icon.
    fn stylus_handwriting(
        self,
        enabled: bool,
        show_hover_icon: bool,
        on_handwriting_slop_exceeded: Rc<dyn Fn()>,
    ) -> Modifier;
}

impl StylusHandwritingModifiers for Modifier {
    /// Starts stylus handwriting on the field (Compose's internal `Modifier.stylusHandwriting`): while `enabled`
    /// and the platform supports it, a stylus moving past the handwriting slop over the field focuses it and calls
    /// `on_handwriting_slop_exceeded`; `show_hover_icon`: a hovering stylus shows the handwriting icon.
    fn stylus_handwriting(
        self,
        enabled: bool,
        show_hover_icon: bool,
        on_handwriting_slop_exceeded: Rc<dyn Fn()>,
    ) -> Modifier {
        if !enabled || !is_stylus_handwriting_supported() {
            return self;
        }
        let modifier = if show_hover_icon {
            named_args!(self.stylus_hover_icon(
                HandwritingPointerIcon,
                touch_bounds_expansion = Some(handwriting_bounds_expansion())
            ))
        } else {
            self
        };
        modifier.then_element(StylusHandwritingElement { on_handwriting_slop_exceeded })
    }
}

struct StylusHandwritingElement {
    on_handwriting_slop_exceeded: Rc<dyn Fn()>,
}

impl PartialEq for StylusHandwritingElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_handwriting_slop_exceeded, &other.on_handwriting_slop_exceeded)
    }
}

impl ModifierNodeElement for StylusHandwritingElement {
    type Node = StylusHandwritingNode;
    fn create(&self) -> StylusHandwritingNode {
        StylusHandwritingNode {
            on_handwriting_slop_exceeded: self.on_handwriting_slop_exceeded.clone(),
            focused: false,
            handle: None,
            tracking: None,
            consuming: None,
        }
    }
    fn update(&self, node: &mut StylusHandwritingNode) {
        node.on_handwriting_slop_exceeded = self.on_handwriting_slop_exceeded.clone();
    }
    fn name(&self) -> &'static str {
        "stylusHandwriting"
    }
}

/// A stylus's press followed until it moves past the slop (Compose's gesture between `awaitFirstDown` and the
/// slop): its pointer, where and when it went down, the pass it is followed in.
#[derive(Clone, Copy)]
struct Tracking {
    id: PointerId,
    down_position: Offset,
    down_time: i64,
    pass: PointerEventPass,
}

/// Compose's StylusHandwritingNode: its gesture as a state machine over the passes (Compose's suspending gesture:
/// the first down awaited in the Initial pass, the moves in the Initial pass if the field is focused or the stylus
/// went down inside it — a focused editor before an unfocused one — else in the Main pass).
pub struct StylusHandwritingNode {
    on_handwriting_slop_exceeded: Rc<dyn Fn()>,
    focused: bool,
    handle: Option<NodeHandle>,
    tracking: Option<Tracking>,
    /// The stylus whose gesture is the input method's now: its changes consumed until it lifts.
    consuming: Option<PointerId>,
}

impl StylusHandwritingNode {
    /// The handwriting slop and the long press timeout where the node is.
    fn thresholds(&self) -> (f32, i64) {
        let configuration = match self.handle.as_ref().and_then(NodeHandle::cx) {
            Some(cx) => cx.current_value_of(&LocalViewConfiguration),
            None => Default::default(),
        };
        (configuration.handwriting_slop, configuration.long_press_timeout_millis())
    }

    /// The slop exceeded: the field focused (if it isn't) and its handwriting started, once nothing is borrowed.
    fn slop_exceeded(&self) {
        let (Some(handle), focused) = (self.handle.clone(), self.focused) else { return };
        let on_handwriting_slop_exceeded = self.on_handwriting_slop_exceeded.clone();
        when_node_released(move || {
            if !focused && let Some(cx) = handle.cx() {
                cx.request_focus();
            }
            on_handwriting_slop_exceeded();
        });
    }
}

impl ModifierNode for StylusHandwritingNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.tracking = None;
        self.consuming = None;
    }

    node_capabilities!(pointer_input, focus_event);
}

impl FocusEventModifierNode for StylusHandwritingNode {
    fn on_focus_event(&mut self, state: FocusState) {
        self.focused = state.is_focused();
    }
}

impl PointerInputModifierNode for StylusHandwritingNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        // The rest of a gesture handed to the input method: its pointer's changes taken until it lifts.
        if let Some(id) = self.consuming {
            if pass == PointerEventPass::Initial {
                match event.changes.iter().find(|c| !c.is_consumed() && c.id == id && c.pressed) {
                    Some(change) => change.consume(),
                    None => self.consuming = None,
                }
            }
            return;
        }
        let Some(tracking) = self.tracking else {
            // A stylus's first down, unconsumed, in the Initial pass.
            if pass != PointerEventPass::Initial {
                return;
            }
            let Some(down) = event.changes.iter().find(|c| c.changed_to_down()) else { return };
            if !matches!(down.r#type, PointerType::Stylus | PointerType::Eraser) {
                return;
            }
            let p = down.position;
            let in_bounds = p.x >= 0.0 && p.x < bounds.width as f32 && p.y >= 0.0 && p.y < bounds.height as f32;
            self.tracking = Some(Tracking {
                id: down.id,
                down_position: p,
                down_time: down.uptime_millis,
                pass: if self.focused || in_bounds { PointerEventPass::Initial } else { PointerEventPass::Main },
            });
            return;
        };
        if pass != tracking.pass {
            return;
        }
        // The tracked pointer consumed or lifted: no handwriting.
        let Some(change) = event.changes.iter().find(|c| !c.is_consumed() && c.id == tracking.id && c.pressed) else {
            self.tracking = None;
            return;
        };
        let (slop, long_press) = self.thresholds();
        if change.uptime_millis - tracking.down_time >= long_press || event.is_deep_press() {
            self.tracking = None;
            return;
        }
        if (change.position - tracking.down_position).get_distance() > slop {
            self.tracking = None;
            self.consuming = Some(tracking.id);
            change.consume();
            self.slop_exceeded();
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        self.tracking = None;
        self.consuming = None;
    }

    /// (A stylus near the field writes into it: Compose's HandwritingBoundsExpansion.)
    fn touch_bounds_expansion(&self) -> TouchBoundsExpansion {
        match &self.handle {
            Some(h) => handwriting_bounds_expansion().round_to_touch_bounds_expansion(&h.density()),
            None => TouchBoundsExpansion::None,
        }
    }
}
