//! Bring-into-view requesters (Compose's foundation `relocation`): a handle a layout asks the scroll containers
//! above it through to bring it (or a rect of it) into view.

use kompoz_graphics::Rect;
use kompoz_runtime::named;
use kompoz_ui::focus::{BoundsProvider, bring_into_view};
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
