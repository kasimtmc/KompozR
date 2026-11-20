//! Material-specific anchor layout logic which considers lookahead, as Compose's internal DraggableAnchors.kt: a
//! layout's anchored-draggable state given its anchors from the layout's size (sheets, drawers, swipe-to-dismiss
//! rows), and the layout moved by the state's offset.

use crate::flags::ComposeMaterial3Flags;
use kompoz_foundation::gestures::{AnchorKey, AnchoredDraggableState, DraggableAnchors, Orientation};
use kompoz_graphics::{IntSize, LayoutDirection};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::named_args;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx,
    NodeHandle, node_capabilities,
};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// What gives a layout's anchors from its size and the constraints it is measured in, and the target it suggests
/// (Compose's `anchors: (size: IntSize, constraints: Constraints) -> Pair<DraggableAnchors<T>, T>`).
pub(crate) type AnchorsFn<T> = Rc<dyn Fn(IntSize, Constraints) -> (Arc<dyn DraggableAnchors<T>>, T)>;

/// The anchors modifier (Compose's internal `Modifier.draggableAnchors`).
pub(crate) trait DraggableAnchorsModifiers {
    /// Gives `state` its anchors from the layout's size and the incoming constraints (`anchors`: subcomposition
    /// avoided) as it is measured — in the lookahead pass, else in the main one —, and places what it wraps at the
    /// state's offset along `orientation` (the target's position while looking ahead) (Compose's internal
    /// `Modifier.draggableAnchors`).
    fn draggable_anchors<T: AnchorKey + fmt::Debug>(
        self,
        state: AnchoredDraggableState<T>,
        orientation: Orientation,
        anchors: impl Fn(IntSize, Constraints) -> (Arc<dyn DraggableAnchors<T>>, T) + 'static,
    ) -> Modifier;
}

impl DraggableAnchorsModifiers for Modifier {
    fn draggable_anchors<T: AnchorKey + fmt::Debug>(
        self,
        state: AnchoredDraggableState<T>,
        orientation: Orientation,
        anchors: impl Fn(IntSize, Constraints) -> (Arc<dyn DraggableAnchors<T>>, T) + 'static,
    ) -> Modifier {
        self.then_element(DraggableAnchorsElement { state, anchors: Rc::new(anchors), orientation })
    }
}

/// Compose's private `DraggableAnchorsElement`: equal for an equal state, the same anchors function (by instance)
/// and orientation.
struct DraggableAnchorsElement<T: AnchorKey> {
    state: AnchoredDraggableState<T>,
    anchors: AnchorsFn<T>,
    orientation: Orientation,
}

impl<T: AnchorKey> PartialEq for DraggableAnchorsElement<T> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && Rc::ptr_eq(&self.anchors, &other.anchors) && self.orientation == other.orientation
    }
}

impl<T: AnchorKey + fmt::Debug> ModifierNodeElement for DraggableAnchorsElement<T> {
    type Node = DraggableAnchorsNode<T>;

    fn create(&self) -> DraggableAnchorsNode<T> {
        DraggableAnchorsNode {
            state: self.state.clone(),
            anchors: self.anchors.clone(),
            orientation: self.orientation,
            did_initialize_anchors: Rc::new(Cell::new(false)),
            handle: None,
        }
    }

    fn update(&self, node: &mut DraggableAnchorsNode<T>) {
        node.update(self.state.clone(), self.anchors.clone(), self.orientation);
    }

    fn name(&self) -> &'static str {
        "draggableAnchors"
    }
}

/// Compose's private `DraggableAnchorsNode`.
struct DraggableAnchorsNode<T: AnchorKey> {
    state: AnchoredDraggableState<T>,
    anchors: AnchorsFn<T>,
    orientation: Orientation,
    /// (read by the placement as it runs, as Kotlin's field)
    did_initialize_anchors: Rc<Cell<bool>>,
    handle: Option<NodeHandle>,
}

impl<T: AnchorKey + fmt::Debug> DraggableAnchorsNode<T> {
    /// Compose's `update`: a new state measures again (the invalidation fix's flag on).
    fn update(&mut self, state: AnchoredDraggableState<T>, anchors: AnchorsFn<T>, orientation: Orientation) {
        let should_invalidate_measure =
            ComposeMaterial3Flags::is_anchored_draggable_components_invalidation_fix_enabled() && self.state != state;
        self.state = state;
        self.anchors = anchors;
        self.orientation = orientation;
        if should_invalidate_measure {
            self.did_initialize_anchors.set(false);
            if let Some(h) = &self.handle {
                h.invalidate_measurement();
            }
        }
    }
}

impl<T: AnchorKey + fmt::Debug> ModifierNode for DraggableAnchorsNode<T> {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.did_initialize_anchors.set(false);
        self.handle = None;
    }
}

impl<T: AnchorKey + fmt::Debug> LayoutModifierNode for DraggableAnchorsNode<T> {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let is_looking_ahead = scope.is_looking_ahead();
        // If we are in a lookahead pass, we only want to update the anchors here and not in post-lookahead. If there
        // is no lookahead happening (!isLookingAhead && !didLookahead), update the anchors in the main pass.
        if !is_looking_ahead || !self.did_initialize_anchors.get() {
            let size = IntSize::new(placeable.width(), placeable.height());
            let (new_anchors, suggested_target) = (self.anchors)(size, constraints);

            if ComposeMaterial3Flags::is_anchored_draggable_components_anchor_recovery_enabled() {
                // Edge case where AnchoredDraggable target value is removed from set of available anchors before
                // placement.
                let validated_target = if new_anchors.has_position_for(&suggested_target) {
                    suggested_target
                } else {
                    new_anchors.anchor_at(0).unwrap_or(suggested_target)
                };
                named_args!(self.state.update_anchors(new_anchors, new_target = Some(validated_target)));
            } else {
                // Previous behavior which places provided target naively.
                named_args!(self.state.update_anchors(new_anchors, new_target = Some(suggested_target)));
            }
            self.did_initialize_anchors.set(true);
        }

        self.did_initialize_anchors.set(is_looking_ahead || self.did_initialize_anchors.get());
        let state = self.state.clone();
        let orientation = self.orientation;
        let did_initialize_anchors = self.did_initialize_anchors.clone();
        // (Compose's `isReverseDirection`, read as it places)
        let is_reverse_direction =
            scope.layout_direction() == LayoutDirection::Rtl && orientation == Orientation::Horizontal;
        scope.layout(placeable.width(), placeable.height(), move |p| {
            // In a lookahead pass, we use the position of the current target as this is where any ongoing
            // animations would move. If the component is in a settled state, lookahead and post-lookahead will
            // converge.
            let offset =
                if is_looking_ahead { state.anchors().position_of(&state.target_value()) } else { state.offset() };

            // By default, we want to be strict about cases with uninitialized offsets and throw an exception.
            if ComposeMaterial3Flags::is_anchored_draggable_components_strict_offset_check_enabled() {
                check_offset_is_valid(&state, offset, is_looking_ahead, did_initialize_anchors.get());
            } else {
                // For debugging purposes, we allow the offset to be uninitialized by disabling the flag. In that
                // case, we don't place anything.
                if offset.is_nan() {
                    return;
                }
            }

            let rtl_modifier = if is_reverse_direction { -1.0 } else { 1.0 };
            let x_offset = if orientation == Orientation::Horizontal { offset * rtl_modifier } else { 0.0 };
            let y_offset = if orientation == Orientation::Vertical { offset } else { 0.0 };
            // Tagging as motion frame of reference placement, meaning the placement contains scrolling. This allows
            // the consumer of this placement offset to differentiate this offset vs. offsets from structural
            // changes. Generally speaking, this signals a preference to directly apply changes rather than
            // animating, to avoid a chasing effect to scrolling.
            p.with_motion_frame_of_reference_placement(|p| {
                p.place(&placeable, round_to_int(x_offset), round_to_int(y_offset));
            });
        })
    }
}

/// Kotlin's `Float.roundToInt` (Math.round: a half up).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// Require the state's offset to be a valid float, or throw an [`AnchoredDraggableUninitializedException`] with
/// more information otherwise (Compose's private `checkOffsetIsValid`).
fn check_offset_is_valid<T: AnchorKey + fmt::Debug>(
    state: &AnchoredDraggableState<T>,
    offset: f32,
    is_looking_ahead: bool,
    did_lookahead: bool,
) {
    if offset.is_nan() {
        let exception = AnchoredDraggableUninitializedException::new(
            is_looking_ahead,
            did_lookahead,
            &*state.anchors(),
            &state.target_value(),
        );
        panic_kt("AnchoredDraggableUninitializedException", exception.message());
    }
}

/// The offset of an anchored-draggable component wasn't set when it was placed (Compose's internal
/// `AnchoredDraggableUninitializedException`, a Throwable: thrown, it panics).
pub(crate) struct AnchoredDraggableUninitializedException {
    message: String,
}

impl AnchoredDraggableUninitializedException {
    pub(crate) fn new<T: 'static + fmt::Debug>(
        is_looking_ahead: bool,
        did_lookahead: bool,
        anchors: &dyn DraggableAnchors<T>,
        target_value: &T,
    ) -> AnchoredDraggableUninitializedException {
        let message = format!(
            "AnchoredDraggableState was not initialized correctly. isLookingAhead={is_looking_ahead},\
             didLookahead={did_lookahead},anchors={anchors:?},targetValue={target_value:?}"
        );
        AnchoredDraggableUninitializedException { message }
    }

    /// Its message (Compose's `message`).
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AnchoredDraggableUninitializedException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AnchoredDraggableUninitializedException: {}", self.message)
    }
}

impl fmt::Debug for AnchoredDraggableUninitializedException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// What `confirmValueChange` is deprecated for (Compose's internal `ConfirmValueChangeDeprecated`).
pub(crate) const ConfirmValueChangeDeprecated: &str = "confirmValueChange is deprecated without replacement. Rather \
     than relying on a callback to veto state changes, the anchor set should not include disallowed anchors. See \
     androidx.compose.foundation.samples.AnchoredDraggableDynamicAnchorsSample for an example of using dynamic \
     anchors over confirmValueChange.";
