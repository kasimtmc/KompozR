//! `Modifier.on_focused_bounds_changed`, as Compose's FocusedBounds.kt: told where the focused focusable below is.

use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::modifier::TraversableNode;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::any::Any;
use std::rc::Rc;

/// What the observers are found by (Compose's FocusedBoundsObserverNode.TraverseKey).
const FocusedBoundsObserverTraverseKey: &str = "androidx.compose.foundation.FocusedBoundsObserverNode";

type OnPositioned = Rc<dyn Fn(Option<&LayoutCoordinates>)>;

/// The focused-bounds modifier of foundation.
#[kompoz_runtime::named(trait FocusedBoundsModifiers)]
impl Modifier {
    /// Calls `on_positioned` with the coordinates of the focusable below that has the focus as it takes it and as
    /// it moves, and with none when it loses it (Compose's `Modifier.onFocusedBoundsChanged`, deprecated there as
    /// best effort: a move that isn't a focusable's own layout change may go untold). The nearest observer is told
    /// first, then the one above it.
    fn on_focused_bounds_changed(self, on_positioned: impl Fn(Option<&LayoutCoordinates>) + 'static) -> Modifier {
        self.then_element(FocusedBoundsObserverElement { on_positioned: Rc::new(on_positioned) })
    }
}

struct FocusedBoundsObserverElement {
    on_positioned: OnPositioned,
}

impl PartialEq for FocusedBoundsObserverElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_positioned, &other.on_positioned)
    }
}

impl ModifierNodeElement for FocusedBoundsObserverElement {
    type Node = FocusedBoundsObserverNode;
    fn create(&self) -> FocusedBoundsObserverNode {
        FocusedBoundsObserverNode { on_positioned: self.on_positioned.clone() }
    }
    fn update(&self, node: &mut FocusedBoundsObserverNode) {
        node.on_positioned = self.on_positioned.clone();
    }
    fn name(&self) -> &'static str {
        "onFocusedBoundsChanged"
    }
}

/// The node of `Modifier.on_focused_bounds_changed` (Compose's FocusedBoundsObserverNode).
pub struct FocusedBoundsObserverNode {
    on_positioned: OnPositioned,
}

impl ModifierNode for FocusedBoundsObserverNode {
    node_capabilities!(traversable);
}

impl TraversableNode for FocusedBoundsObserverNode {
    fn traverse_key(&self) -> &dyn Any {
        &FocusedBoundsObserverTraverseKey
    }
}

/// Tells the observers above `cx`'s node where the focused bounds are now, nearest first (Compose's
/// `focusedBoundsObserver?.onFocusBoundsChanged`, which bubbles up): each callback run with no node borrowed.
pub(crate) fn notify_focused_bounds_observers(cx: &NodeCx, focused_bounds: Option<LayoutCoordinates>) {
    let Some(observer) = cx.find_nearest_ancestor(&FocusedBoundsObserverTraverseKey) else { return };
    let callback = observer.with(|o: &mut FocusedBoundsObserverNode| o.on_positioned.clone());
    if let Some(f) = callback {
        f(focused_bounds.as_ref());
    }
    notify_focused_bounds_observers(&observer, focused_bounds);
}
