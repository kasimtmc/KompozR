//! Focus requesters (Compose's FocusRequester, `Modifier.focusRequester` and FocusRequesterModifierNode): a handle
//! the focus can be asked for through, reaching the focus targets below the nodes it is attached to.

use super::{FocusDirection, children, target};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCell, NodeCx, NodeKind, NodeRc};
use kompoz_runtime::named;
use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

/// The nodes a requester is attached to.
#[derive(Default)]
pub(crate) struct RequesterNodes {
    nodes: RefCell<Vec<Weak<NodeCell<dyn ModifierNode>>>>,
}

#[derive(Clone)]
enum Kind {
    Default,
    Cancel,
    Redirect,
    Nodes(Rc<RequesterNodes>),
}

/// Asks for the focus (Compose's FocusRequester): attached to layouts with `Modifier.focus_requester`, it moves
/// the focus to the first focus target below them that takes it. Remember one per use (`remember(FocusRequester::new)`).
/// `FocusRequester::Default` and `FocusRequester::Cancel` are what focus properties say for "the usual search"
/// and "nowhere"; asking them for the focus panics, as in Compose.
#[derive(Clone)]
pub struct FocusRequester {
    kind: Kind,
}

const NOT_INITIALIZED: &str = "FocusRequester is not initialized. Here are some possible fixes:
   1. Remember the FocusRequester: val focusRequester = remember { FocusRequester() }
   2. Did you forget to add a Modifier.focusRequester() ?
   3. Are you attempting to request focus during composition? Focus requests should be made in
   response to some event. Eg Modifier.clickable { focusRequester.requestFocus() }";

const INVALID_INVOCATION: &str = "Please check whether the focusRequester is FocusRequester.Cancel or \
                                  FocusRequester.Default before invoking any functions on the focusRequester.";

impl FocusRequester {
    /// The usual search (what focus properties are by default).
    pub const Default: FocusRequester = FocusRequester { kind: Kind::Default };
    /// Nowhere: the move is called off.
    pub const Cancel: FocusRequester = FocusRequester { kind: Kind::Cancel };
    /// What a custom enter or exit that moved the focus itself comes to.
    pub(crate) const Redirect: FocusRequester = FocusRequester { kind: Kind::Redirect };

    /// A requester of its own, attached to nothing yet (Compose's `FocusRequester()`).
    ///
    /// Kotlin: `FocusRequester()`
    pub fn new() -> FocusRequester {
        FocusRequester { kind: Kind::Nodes(Rc::new(RequesterNodes::default())) }
    }

    /// `N` requesters at once (Compose's `createRefs`): `let [a, b] = FocusRequester::create_refs();`.
    pub fn create_refs<const N: usize>() -> [FocusRequester; N] {
        std::array::from_fn(|_| FocusRequester::new())
    }

    fn nodes(&self) -> Vec<NodeRc> {
        match &self.kind {
            Kind::Nodes(n) => {
                n.nodes.borrow_mut().retain(|w| w.strong_count() > 0);
                n.nodes.borrow().iter().filter_map(Weak::upgrade).collect()
            }
            _ => panic!("{INVALID_INVOCATION}"),
        }
    }

    pub(crate) fn add_node(&self, cell: &NodeRc) {
        if let Kind::Nodes(n) = &self.kind {
            n.nodes.borrow_mut().push(Rc::downgrade(cell));
        }
    }

    pub(crate) fn remove_node(&self, cell: &NodeRc) {
        if let Kind::Nodes(n) = &self.kind {
            let w = Rc::downgrade(cell);
            n.nodes.borrow_mut().retain(|x| !x.ptr_eq(&w));
        }
    }

    /// Asks `on_found` of the targets below its nodes until one takes it (Compose's `findFocusTarget`).
    pub(crate) fn find_focus_target_for_search(&self, on_found: &mut dyn FnMut(&NodeRc) -> bool) -> bool {
        let nodes = self.nodes();
        if nodes.is_empty() {
            eprintln!("FocusRelatedWarning: {NOT_INITIALIZED}");
            return false;
        }
        for node in nodes {
            for t in children(&node, NodeKind::FocusTarget) {
                if on_found(&t) {
                    return true;
                }
            }
        }
        false
    }

    /// Moves the focus `direction` to its target (Compose's `requestFocus(focusDirection)`); whether it moved.
    pub fn request_focus_with(&self, direction: FocusDirection) -> bool {
        self.find_focus_target_for_search(&mut |t| target::request_focus(t, direction))
    }

    /// Captures the focus at its target, if it has it (Compose's `captureFocus`).
    pub fn capture_focus(&self) -> bool {
        self.each_node(|cx| cx.capture_focus())
    }

    /// Frees a captured focus at its target (Compose's `freeFocus`).
    pub fn free_focus(&self) -> bool {
        self.each_node(|cx| cx.free_focus())
    }

    /// Notes the child of its target that has the focus (Compose's `saveFocusedChild`).
    pub fn save_focused_child(&self) -> bool {
        self.each_node(|cx| cx.save_focused_child())
    }

    /// Moves the focus back to the child noted (Compose's `restoreFocusedChild`).
    pub fn restore_focused_child(&self) -> bool {
        let nodes = self.nodes();
        if nodes.is_empty() {
            eprintln!("FocusRelatedWarning: {NOT_INITIALIZED}");
            return false;
        }
        let mut success = false;
        for node in nodes {
            success = NodeCx::new(&node).restore_focused_child() || success;
        }
        success
    }

    fn each_node(&self, f: impl Fn(&NodeCx) -> bool) -> bool {
        let nodes = self.nodes();
        if nodes.is_empty() {
            eprintln!("FocusRelatedWarning: {NOT_INITIALIZED}");
            return false;
        }
        nodes.iter().any(|n| f(&NodeCx::new(n)))
    }
}

#[named]
impl FocusRequester {
    /// Moves the focus to its target (Compose's `requestFocus(focusDirection = Enter)`); whether it moved.
    pub fn request_focus(&self, #[default(FocusDirection::Enter)] focus_direction: FocusDirection) -> bool {
        self.request_focus_with(focus_direction)
    }
}

impl Default for FocusRequester {
    /// A new requester (not `FocusRequester::Default`).
    fn default() -> Self {
        FocusRequester::new()
    }
}

impl PartialEq for FocusRequester {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (Kind::Default, Kind::Default) | (Kind::Cancel, Kind::Cancel) | (Kind::Redirect, Kind::Redirect) => true,
            (Kind::Nodes(a), Kind::Nodes(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for FocusRequester {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            Kind::Default => f.write_str("FocusRequester.Default"),
            Kind::Cancel => f.write_str("FocusRequester.Cancel"),
            Kind::Redirect => f.write_str("FocusRequester.Redirect"),
            Kind::Nodes(n) => write!(f, "FocusRequester@{:p}", Rc::as_ptr(n)),
        }
    }
}

impl Modifier {
    /// Attaches `focus_requester` here: asking it for the focus moves it to the first target below that takes it
    /// (Compose's `Modifier.focusRequester`).
    pub fn focus_requester(self, focus_requester: &FocusRequester) -> Modifier {
        self.then_element(FocusRequesterElement { requester: focus_requester.clone() })
    }
}

#[derive(PartialEq)]
struct FocusRequesterElement {
    requester: FocusRequester,
}

impl ModifierNodeElement for FocusRequesterElement {
    type Node = FocusRequesterNode;
    fn create(&self) -> FocusRequesterNode {
        FocusRequesterNode { requester: self.requester.clone(), cell: None }
    }
    fn update(&self, node: &mut FocusRequesterNode) {
        if let Some(cell) = node.cell.as_ref().and_then(Weak::upgrade) {
            node.requester.remove_node(&cell);
            self.requester.add_node(&cell);
        }
        node.requester = self.requester.clone();
    }
    fn name(&self) -> &'static str {
        "focusRequester"
    }
}

/// `Modifier.focus_requester`'s node (Compose's FocusRequesterNode): its requester reaches the targets below it.
pub struct FocusRequesterNode {
    requester: FocusRequester,
    cell: Option<Weak<NodeCell<dyn ModifierNode>>>,
}

impl ModifierNode for FocusRequesterNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.requester.add_node(&cx.cell);
        self.cell = Some(Rc::downgrade(&cx.cell));
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        self.requester.remove_node(&cx.cell);
        self.cell = None;
    }
}
