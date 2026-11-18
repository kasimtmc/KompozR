//! Bring-into-view requesters (Compose's foundation `relocation`): a handle a layout asks the scroll containers
//! above it through to bring it (or a rect of it) into view.

use kompoz_graphics::Rect;
use kompoz_runtime::named;
use kompoz_ui::focus::{BoundsProvider, BringIntoViewFuture, BringIntoViewModifierNode, bring_into_view};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

/// Asks the bring-into-view nodes above the layouts it is attached to (`Modifier.bring_into_view_requester`) to
/// bring them into view (Compose's BringIntoViewRequester). Remember one per use.
#[derive(Clone, Default)]
pub struct BringIntoViewRequester {
    nodes: Rc<RefCell<Vec<NodeHandle>>>,
}

impl PartialEq for BringIntoViewRequester {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.nodes, &other.nodes)
    }
}

impl BringIntoViewRequester {
    pub fn new() -> BringIntoViewRequester {
        BringIntoViewRequester::default()
    }

    /// Brings `rect` of each of its layouts (in its coordinates; none: all of it) into view, one after the other
    /// (Compose's `bringIntoView(rect)`).
    pub fn bring_into_view_rect(&self, rect: Option<Rect>) -> impl Future<Output = ()> + 'static {
        let nodes: Vec<NodeHandle> = self.nodes.borrow().clone();
        async move {
            for node in nodes {
                let Some(cx) = node.cx() else { continue };
                let bounds: Option<BoundsProvider> = rect.map(|r| Rc::new(move || Some(r)) as BoundsProvider);
                bring_into_view(&cx, bounds).await;
            }
        }
    }

    fn add(&self, cx: &NodeCx) {
        self.nodes.borrow_mut().push(cx.handle());
    }

    fn remove(&self, cx: &NodeCx) {
        self.nodes.borrow_mut().retain(|h| h.cx().is_some_and(|c| !c.is_same_node(cx)));
    }
}

#[named]
impl BringIntoViewRequester {
    /// Brings its layouts (or `rect` of them) into view (Compose's `bringIntoView(rect = null)`).
    pub fn bring_into_view(&self, #[default(None)] rect: Option<Rect>) -> impl Future<Output = ()> + 'static {
        self.bring_into_view_rect(rect)
    }
}

/// The relocation modifiers of foundation.
#[named(trait RelocationModifiers)]
impl Modifier {
    /// Attaches `requester` here: asking it brings this layout into view (Compose's
    /// `Modifier.bringIntoViewRequester`).
    fn bring_into_view_requester(self, requester: &BringIntoViewRequester) -> Modifier {
        self.then_element(BringIntoViewRequesterElement { requester: requester.clone() })
    }
}

#[derive(PartialEq)]
struct BringIntoViewRequesterElement {
    requester: BringIntoViewRequester,
}

impl ModifierNodeElement for BringIntoViewRequesterElement {
    type Node = BringIntoViewRequesterNode;
    fn create(&self) -> BringIntoViewRequesterNode {
        BringIntoViewRequesterNode { requester: self.requester.clone(), handle: None }
    }
    fn update(&self, node: &mut BringIntoViewRequesterNode) {
        if let Some(cx) = node.handle.as_ref().and_then(|h| h.cx()) {
            node.requester.remove(&cx);
            self.requester.add(&cx);
        }
        node.requester = self.requester.clone();
    }
    fn name(&self) -> &'static str {
        "bringIntoViewRequester"
    }
}

/// `Modifier.bring_into_view_requester`'s node (Compose's BringIntoViewRequesterNode).
pub struct BringIntoViewRequesterNode {
    requester: BringIntoViewRequester,
    handle: Option<NodeHandle>,
}

impl BringIntoViewRequesterNode {
    /// A node `requester` brings into view (Compose's `BringIntoViewRequesterNode(requester)`, what a text field's
    /// text layout node delegates to).
    pub(crate) fn new(requester: BringIntoViewRequester) -> BringIntoViewRequesterNode {
        BringIntoViewRequesterNode { requester, handle: None }
    }

    /// Another requester brings it into view (Compose's `updateRequester`).
    pub(crate) fn update_requester(&mut self, requester: BringIntoViewRequester) {
        if let Some(cx) = self.handle.as_ref().and_then(NodeHandle::cx) {
            self.requester.remove(&cx);
            requester.add(&cx);
        }
        self.requester = requester;
    }
}

impl ModifierNode for BringIntoViewRequesterNode {
    fn should_auto_invalidate(&self) -> bool {
        false
    }
    fn on_attach(&mut self, cx: &NodeCx) {
        self.requester.add(cx);
        self.handle = Some(cx.handle());
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        self.requester.remove(cx);
        self.handle = None;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Responders

/// What brings a rect of its content into view (Compose's deprecated BringIntoViewResponder, what a scrollable's
/// ContentInViewNode is): where a rect of it will be once it has brought it in, for the containers above it, and
/// bringing it in (a future done once it is, or once the request was let go).
pub trait BringIntoViewResponder {
    /// Where `local_rect` (in its coordinates) will be once it has brought it into view (Compose's
    /// `calculateRectForParent`).
    fn calculate_rect_for_parent(&self, local_rect: Rect) -> Rect;

    /// Brings `local_rect()` (in its coordinates, asked again as things move; none: gone) into view (Compose's
    /// `bringChildIntoView`).
    fn bring_child_into_view(self: Rc<Self>, local_rect: Rc<dyn Fn() -> Option<Rect>>) -> BringIntoViewFuture;
}

/// The responder modifier.
#[named(trait BringIntoViewResponderModifiers)]
impl Modifier {
    /// Makes this layout bring what asks under it into view through `responder` (Compose's deprecated
    /// `Modifier.bringIntoViewResponder`), and the containers above it as `responder` says it will be.
    fn bring_into_view_responder(self, responder: Rc<dyn BringIntoViewResponder>) -> Modifier {
        self.then_element(BringIntoViewResponderElement { responder })
    }
}

struct BringIntoViewResponderElement {
    responder: Rc<dyn BringIntoViewResponder>,
}

impl PartialEq for BringIntoViewResponderElement {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.responder), Rc::as_ptr(&other.responder))
    }
}

impl ModifierNodeElement for BringIntoViewResponderElement {
    type Node = BringIntoViewResponderNode;
    fn create(&self) -> BringIntoViewResponderNode {
        BringIntoViewResponderNode::new(self.responder.clone())
    }
    fn update(&self, node: &mut BringIntoViewResponderNode) {
        node.responder = self.responder.clone();
    }
    fn name(&self) -> &'static str {
        "bringIntoViewResponder"
    }
}

/// The node bringing a child's rect into view through its responder, and through the containers above it
/// (Compose's BringIntoViewResponderNode): both at once.
pub struct BringIntoViewResponderNode {
    pub(crate) responder: Rc<dyn BringIntoViewResponder>,
    shared: Rc<ResponderShared>,
}

#[derive(Default)]
struct ResponderShared {
    handle: RefCell<Option<NodeHandle>>,
    /// Placed once (Compose's `hasBeenPlaced`: nothing is worked out before).
    has_been_placed: std::cell::Cell<bool>,
}

impl BringIntoViewResponderNode {
    /// A node of `responder` (what a scrollable delegates to with its ContentInViewNode).
    pub fn new(responder: Rc<dyn BringIntoViewResponder>) -> BringIntoViewResponderNode {
        BringIntoViewResponderNode { responder, shared: Rc::new(ResponderShared::default()) }
    }
}

impl ModifierNode for BringIntoViewResponderNode {
    kompoz_ui::node_capabilities!(bring_into_view, layout_aware);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
    }
}

impl kompoz_ui::layout::LayoutAwareModifierNode for BringIntoViewResponderNode {
    fn on_placed(&mut self, _coordinates: &kompoz_ui::LayoutCoordinates) {
        self.shared.has_been_placed.set(true);
    }

    fn observes_remeasure(&self) -> bool {
        false
    }
}

impl BringIntoViewModifierNode for BringIntoViewResponderNode {
    fn bring_into_view(
        &mut self,
        child_coordinates: kompoz_ui::LayoutCoordinates,
        bounds_provider: BoundsProvider,
    ) -> BringIntoViewFuture {
        let shared = self.shared.clone();
        let responder = self.responder.clone();
        // The rect in this node's coordinates, while both are attached and it was placed (Compose's `localRect`).
        let local_rect: Rc<dyn Fn() -> Option<Rect>> = Rc::new(move || {
            let cx = shared.handle.borrow().as_ref().and_then(|h| h.cx())?;
            if !cx.is_attached() || !shared.has_been_placed.get() {
                return None;
            }
            let layout_coordinates = cx.coordinates()?;
            if !child_coordinates.is_attached() {
                return None;
            }
            let rect = bounds_provider()?;
            let local = layout_coordinates.local_bounding_box_of(&child_coordinates, false);
            Some(rect.translate(local.top_left()))
        });
        let parent_rect: BoundsProvider = {
            let (local_rect, responder) = (local_rect.clone(), responder.clone());
            Rc::new(move || local_rect().map(|r| responder.calculate_rect_for_parent(r)))
        };
        let parent = self.shared.handle.borrow().as_ref().and_then(|h| h.cx());
        Box::pin(async move {
            // Both at once: the child into this one's view, this one's rect into its parents'.
            kompoz_coroutines::coroutine_scope(move |scope| async move {
                scope.launch(responder.bring_child_into_view(local_rect));
                if let Some(cx) = parent {
                    scope.launch(bring_into_view(&cx, Some(parent_rect)));
                }
            })
            .await
        })
    }
}
