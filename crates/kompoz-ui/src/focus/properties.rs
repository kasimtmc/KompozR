//! Focus properties (Compose's FocusProperties, FocusEnterExitScope and `Modifier.focusProperties`): whether a
//! target can take the focus, where each direction takes it from there, what entering or leaving it does, the
//! rect it is searched by.

use super::requester::FocusRequester;
use super::{FocusDirection, FocusPropertiesModifierNode};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::node_capabilities;
use kompoz_graphics::Rect;
use std::fmt;
use std::rc::Rc;

/// What entering or leaving a target does (Compose's `FocusEnterExitScope.() -> Unit`).
pub type FocusEnterExitBlock = Rc<dyn Fn(&mut FocusEnterExitScope)>;

/// Where an enter or exit is going and whether it is called off (Compose's FocusEnterExitScope): a block may
/// cancel the change, or move the focus itself (a requester's `request_focus`), which redirects it.
pub struct FocusEnterExitScope {
    requested_focus_direction: FocusDirection,
    canceled: bool,
}

impl FocusEnterExitScope {
    pub(crate) fn new(direction: FocusDirection) -> FocusEnterExitScope {
        FocusEnterExitScope { requested_focus_direction: direction, canceled: false }
    }

    /// The way the focus is moving.
    pub fn requested_focus_direction(&self) -> FocusDirection {
        self.requested_focus_direction
    }

    /// Calls the change off: the focus stays where it is.
    pub fn cancel_focus_change(&mut self) {
        self.canceled = true;
    }

    pub(crate) fn is_canceled(&self) -> bool {
        self.canceled
    }
}

/// The focus properties of a target (Compose's FocusProperties), as the property nodes above it set them: whether
/// it can take the focus; where each direction goes from it (a requester's target; `FocusRequester::Default`: the
/// usual search; `FocusRequester::Cancel`: nowhere); what entering and leaving it does; the rect it is searched
/// by (none: its bounds).
#[derive(Clone)]
pub struct FocusProperties {
    pub can_focus: bool,
    pub next: FocusRequester,
    pub previous: FocusRequester,
    pub up: FocusRequester,
    pub down: FocusRequester,
    pub left: FocusRequester,
    pub right: FocusRequester,
    /// Left in a left-to-right layout, right in a right-to-left one (over `left`/`right` when set).
    pub start: FocusRequester,
    pub end: FocusRequester,
    pub on_enter: Option<FocusEnterExitBlock>,
    pub on_exit: Option<FocusEnterExitBlock>,
    pub focus_rect: Option<Rect>,
}

impl Default for FocusProperties {
    fn default() -> Self {
        FocusProperties {
            can_focus: true,
            next: FocusRequester::Default,
            previous: FocusRequester::Default,
            up: FocusRequester::Default,
            down: FocusRequester::Default,
            left: FocusRequester::Default,
            right: FocusRequester::Default,
            start: FocusRequester::Default,
            end: FocusRequester::Default,
            on_enter: None,
            on_exit: None,
            focus_rect: None,
        }
    }
}

impl fmt::Debug for FocusProperties {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FocusProperties")
            .field("can_focus", &self.can_focus)
            .field("focus_rect", &self.focus_rect)
            .finish_non_exhaustive()
    }
}

impl FocusProperties {
    /// What entering does, `block` given the scope (Compose's `onEnter = { … }`).
    pub fn set_on_enter(&mut self, block: impl Fn(&mut FocusEnterExitScope) + 'static) {
        self.on_enter = Some(Rc::new(block));
    }

    /// What leaving does (Compose's `onExit = { … }`).
    pub fn set_on_exit(&mut self, block: impl Fn(&mut FocusEnterExitScope) + 'static) {
        self.on_exit = Some(Rc::new(block));
    }

    /// Where entering goes, by the requester `enter` gives for the direction (Compose's deprecated `enter = { … }`:
    /// `Cancel` cancels, `Default` leaves it to the search).
    pub fn set_enter(&mut self, enter: impl Fn(FocusDirection) -> FocusRequester + 'static) {
        self.on_enter = Some(to_enter_exit_block(enter));
    }

    /// Where leaving goes (Compose's deprecated `exit = { … }`).
    pub fn set_exit(&mut self, exit: impl Fn(FocusDirection) -> FocusRequester + 'static) {
        self.on_exit = Some(to_enter_exit_block(exit));
    }
}

/// A requester-giving block as an enter/exit block (Compose's `toUsingEnterExitScope`).
fn to_enter_exit_block(f: impl Fn(FocusDirection) -> FocusRequester + 'static) -> FocusEnterExitBlock {
    Rc::new(move |scope: &mut FocusEnterExitScope| {
        let requester = f(scope.requested_focus_direction());
        if requester == FocusRequester::Cancel {
            scope.cancel_focus_change();
        } else if requester != FocusRequester::Default {
            requester.request_focus();
        }
    })
}

impl Modifier {
    /// Sets the focus properties of the targets below, up to the next target (Compose's `Modifier.focusProperties`).
    pub fn focus_properties(self, scope: impl Fn(&mut FocusProperties) + 'static) -> Modifier {
        self.then_element(FocusPropertiesElement { scope: Rc::new(scope) })
    }
}

struct FocusPropertiesElement {
    scope: Rc<dyn Fn(&mut FocusProperties)>,
}

impl PartialEq for FocusPropertiesElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.scope, &other.scope)
    }
}

impl ModifierNodeElement for FocusPropertiesElement {
    type Node = FocusPropertiesNode;
    fn create(&self) -> FocusPropertiesNode {
        FocusPropertiesNode { scope: self.scope.clone() }
    }
    fn update(&self, node: &mut FocusPropertiesNode) {
        node.scope = self.scope.clone();
    }
    fn name(&self) -> &'static str {
        "focusProperties"
    }
}

/// `Modifier.focus_properties`' node (Compose's FocusPropertiesNode).
pub struct FocusPropertiesNode {
    scope: Rc<dyn Fn(&mut FocusProperties)>,
}

impl ModifierNode for FocusPropertiesNode {
    node_capabilities!(focus_properties);
}

impl FocusPropertiesModifierNode for FocusPropertiesNode {
    fn apply_focus_properties(&mut self, properties: &mut FocusProperties) {
        (self.scope)(properties)
    }
}
