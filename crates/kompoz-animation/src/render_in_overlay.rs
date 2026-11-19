//! `renderInSharedTransitionScopeOverlay`'s node, as Compose's internal `RenderInTransitionOverlayNodeElement.kt`:
//! its content recorded in a layer and drawn either where it is or, while it is asked to, in the shared transition
//! scope's overlay (over everything in the scope, in the overlay's z order) — never in both.

use crate::shared_content_node::ModifierLocalSharedElementInternalState;
use crate::shared_element::SharedElementEntry;
use crate::shared_transition::{LayerRenderer, SharedTransitionScopeImpl};
use kompoz_graphics::{IntSize, Offset};
use kompoz_runtime::{MutableState, mutable_state_of};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, DrawScope, GraphicsLayer};
use kompoz_ui::layout::{LayoutModifierNode, Measurable, MeasureResult, MeasureScope, PlacementScope};
use kompoz_ui::platform::{LocalPlatform, PlatformHandle};
use kompoz_ui::{Constraints, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The element (Compose's RenderInTransitionOverlayNodeElement): equal with the same scope, the same
/// `render_in_overlay` and an equal z index.
pub(crate) struct RenderInTransitionOverlayNodeElement {
    pub(crate) shared_transition_scope: Rc<SharedTransitionScopeImpl>,
    pub(crate) render_in_overlay: Rc<dyn Fn() -> bool>,
    pub(crate) z_index_in_overlay: f32,
}

impl PartialEq for RenderInTransitionOverlayNodeElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared_transition_scope, &other.shared_transition_scope)
            && Rc::ptr_eq(&self.render_in_overlay, &other.render_in_overlay)
            && self.z_index_in_overlay == other.z_index_in_overlay
    }
}

impl ModifierNodeElement for RenderInTransitionOverlayNodeElement {
    type Node = RenderInTransitionOverlayNode;
    fn create(&self) -> RenderInTransitionOverlayNode {
        RenderInTransitionOverlayNode {
            state: Rc::new(OverlayNodeState {
                shared_scope: RefCell::new(self.shared_transition_scope.clone()),
                render_in_overlay: RefCell::new(self.render_in_overlay.clone()),
                z_index_in_overlay: mutable_state_of(self.z_index_in_overlay),
                enabled: Cell::new(false),
                position_in_overlay: Cell::new(Offset::Zero),
                layer: RefCell::new(None),
                handle: RefCell::new(None),
            }),
            layer_with_renderer: None,
        }
    }
    fn update(&self, node: &mut RenderInTransitionOverlayNode) {
        let state = &node.state;
        *state.shared_scope.borrow_mut() = self.shared_transition_scope.clone();
        *state.render_in_overlay.borrow_mut() = self.render_in_overlay.clone();
        state.set_z_index_in_overlay(self.z_index_in_overlay);
    }
    fn name(&self) -> &'static str {
        "renderInSharedTransitionOverlay"
    }
}

/// What of the node its placement block and its overlay renderer reach (Compose's fields of
/// RenderInTransitionOverlayNode).
struct OverlayNodeState {
    shared_scope: RefCell<Rc<SharedTransitionScopeImpl>>,
    render_in_overlay: RefCell<Rc<dyn Fn() -> bool>>,
    /// Compose's `zIndexInOverlay`, a float state.
    z_index_in_overlay: MutableState<f32>,
    /// Whether it is drawn in the overlay, and where (Compose's `enabled`, `positionInOverlay`): changed as it is
    /// placed, which invalidates the overlay's drawing and its own in the same frame, so they are plain values
    /// (only the placement observes what decides them).
    enabled: Cell<bool>,
    position_in_overlay: Cell<Offset>,
    layer: RefCell<Option<GraphicsLayer>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl OverlayNodeState {
    fn shared_scope(&self) -> Rc<SharedTransitionScopeImpl> {
        self.shared_scope.borrow().clone()
    }

    fn set_z_index_in_overlay(&self, value: f32) {
        if self.z_index_in_overlay.peek() != value {
            self.z_index_in_overlay.set(value);
            self.shared_scope().z_order_changed();
        }
    }

    /// Both draws asked again (the overlay's and its own) when it changes.
    fn invalidate(&self) {
        if let Some(invalidate) = self.shared_scope().invalidate_overlay.borrow().clone() {
            invalidate();
        }
        if let Some(handle) = self.handle.borrow().as_ref() {
            handle.invalidate_draw();
        }
    }

    fn set_enabled(&self, value: bool) {
        if value != self.enabled.get() {
            self.invalidate();
            self.enabled.set(value);
        }
    }

    fn set_position_in_overlay(&self, value: Offset) {
        if value != self.position_in_overlay.get() {
            self.invalidate();
            self.position_in_overlay.set(value);
        }
    }
}

/// Its layer drawn in the overlay (Compose's inner LayerWithRenderer).
struct LayerWithRenderer {
    state: Rc<OverlayNodeState>,
    layer: GraphicsLayer,
}

impl LayerRenderer for LayerWithRenderer {
    /// The shared element it is in (Compose's `parentState`: the modifier local where its node is).
    fn parent_state(&self) -> Option<Rc<SharedElementEntry>> {
        let cx = self.state.handle.borrow().as_ref().and_then(|h| h.cx())?;
        cx.modifier_local_current(&ModifierLocalSharedElementInternalState)
    }

    fn z_index(&self) -> f32 {
        self.state.z_index_in_overlay.get()
    }

    fn draw_in_overlay(&self, scope: &mut DrawScope, _platform: &PlatformHandle) {
        if self.state.enabled.get() {
            let at = self.state.position_in_overlay.get();
            scope.translate_with(at.x, at.y, |s| s.draw_layer(&self.layer));
        }
    }
}

/// Compose's RenderInTransitionOverlayNode: a layout (where it is in the overlay, found as it is placed) and a
/// drawing (its content recorded, drawn here unless the overlay does).
pub(crate) struct RenderInTransitionOverlayNode {
    state: Rc<OverlayNodeState>,
    layer_with_renderer: Option<Rc<LayerWithRenderer>>,
}

impl ModifierNode for RenderInTransitionOverlayNode {
    node_capabilities!(layout, draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.state.handle.borrow_mut() = Some(cx.handle());
        let layer = GraphicsLayer::new(Some(cx.current_value_of(&LocalPlatform)));
        *self.state.layer.borrow_mut() = Some(layer.clone());
        let renderer = Rc::new(LayerWithRenderer { state: self.state.clone(), layer });
        self.state.shared_scope().on_layer_renderer_created(renderer.clone());
        self.layer_with_renderer = Some(renderer);
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        if let Some(renderer) = &self.layer_with_renderer {
            let as_renderer: Rc<dyn LayerRenderer> = renderer.clone();
            self.state.shared_scope().on_layer_renderer_removed(&as_renderer);
            renderer.layer.release();
        }
    }
}

impl LayoutModifierNode for RenderInTransitionOverlayNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let state = self.state.clone();
        scope.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
            if !p.is_looking_ahead() {
                let render_in_overlay = state.render_in_overlay.borrow().clone();
                if render_in_overlay() {
                    // (the coordinates read from the placement: it is placed again when it moves while it is in
                    // the overlay, and so moves there)
                    if p.coordinates().is_some() {
                        state.set_enabled(true);
                        let own = state.handle.borrow().as_ref().and_then(|h| h.coordinates());
                        if let Some(own) = own {
                            state.set_position_in_overlay(
                                state.shared_scope().root().local_position_of(&own, Offset::Zero),
                            );
                        }
                    }
                } else {
                    state.set_enabled(false);
                }
            }
            p.place(&placeable, 0, 0);
        })
    }
}

impl DrawModifierNode for RenderInTransitionOverlayNode {
    /// Drawn where it is or in the overlay (by `render_in_overlay`), never in both.
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let layer = self.state.layer.borrow().clone().expect("Error: layer never initialized");
        let size = scope.size();
        scope.record_with(&layer, IntSize::new(size.width.round() as i32, size.height.round() as i32), |s| {
            s.draw_content();
        });
        if !self.state.enabled.get() {
            scope.draw_layer(&layer);
        }
    }
}
