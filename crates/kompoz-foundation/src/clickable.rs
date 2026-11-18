//! Clicks, as Compose's Clickable.kt: `Modifier.clickable` and `Modifier.combined_clickable`, and the node behind
//! them, `selectable`, `toggleable` and `tri_state_toggleable` too (Compose's AbstractClickableNode with its
//! ClickableNode and CombinedClickableNode).
//!
//! A clickable node takes the pointer events of its layout itself (a press is a down nobody consumed, a click its
//! release in bounds — a finger's minimum touch target counting — that nobody consumed meanwhile), takes Enter,
//! NumPadEnter, Space and DPad center while it (or something in it) has the focus, and delegates to a focusable
//! (`Focusability::SystemDefined`: reached with keys, not by touch), to a gesture node (what the gestures around it
//! ask) and to the node its indication makes for its interaction source. It merges what is in it into one
//! accessibility node with a click action, its role, and disabled when it is.
//!
//! What the node does in user code (`on_click`, a long or double click) runs once the node is no longer borrowed:
//! right after the event that set it off.

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use crate::focusable::{FocusableNode, FocusableShared, apply_focusable_semantics};
use crate::gesture_node::{GestureConnection, GestureNode, GestureState, has_waiting_parent};
use crate::gestures::is_changed_to_down;
use crate::indication::{Indication, IndicationModifiers, IndicationNode, LocalIndication};
use crate::interaction::{HoverInteraction, Interaction, MutableInteractionSource, PressInteraction};
use kompoz_coroutines::{CoroutineScope, Job, delay_ms};
use kompoz_graphics::{IntSize, Offset, Size};
use kompoz_runtime::named;
use kompoz_ui::focus::Focusability;
use kompoz_ui::input::{
    Key, KeyEvent, KeyEventType, KeyInputModifierNode, PointerEvent, PointerEventPass, PointerEventType,
    PointerInputChange, PointerInputModifierNode, when_node_released,
};
use kompoz_ui::modifier::{DelegatedNode, ObserverModifierNode};
use kompoz_ui::platform::{HapticFeedbackType, LocalHapticFeedback, LocalViewConfiguration};
use kompoz_ui::semantics::{
    AccessibilityAction, ActionFn, ContentDataType, FillableData, Role, SemanticsActions, SemanticsConfiguration,
    SemanticsModifierNode, ToggleableState,
};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// A callback of a click (`on_long_click`, `on_double_click`: optional ones are `Option<Callback>`).
pub type Callback = Rc<dyn Fn()>;

/// How long a press waits before it is shown (emitted) when a gesture around it is still deciding whether it is a
/// drag or a scroll (Compose's TapIndicationDelay: Android's tap timeout).
pub const TapIndicationDelay: i64 = 100;

/// Whether the host's root is inside a scrolling container of the platform's, so presses wait to show (Compose's
/// `isComposeRootInScrollableContainer`: Android's views around the ComposeView). A KompozR host is the whole
/// window: never.
pub fn is_compose_root_in_scrollable_container(cx: &NodeCx) -> bool {
    let _ = cx;
    false
}

/// Whether the key presses or clicks a clickable (Compose's `isEnter`): Enter, NumPadEnter, DPad center, Space.
fn is_enter(event: &KeyEvent) -> bool {
    matches!(event.key, Key::DirectionCenter | Key::Enter | Key::NumPadEnter | Key::Spacebar)
}

/// A key going down that presses a clickable (Compose's `isPress`).
fn is_press(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyDown && is_enter(event)
}

/// A key going up that clicks it (Compose's `isClick`).
fn is_click(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyUp && is_enter(event)
}

// ---------------------------------------------------------------------------------------------------------------
// The modifiers

/// The click modifiers (Compose's `Modifier.clickable` and `Modifier.combinedClickable` overloads).
#[named(trait ClickableModifiers)]
impl Modifier {
    /// Makes the layout clickable (Compose's `Modifier.clickable`): `on_click` on a tap, a mouse click, Enter or
    /// Space while focused, and accessibility's click (labelled `on_click_label`; `role` tells what it is). It
    /// shows [`LocalIndication`]'s indication of its interactions, emitted to `interaction_source` (none: a source
    /// of its own, made when first needed). Not `enabled`: no clicks, disabled to accessibility.
    fn clickable(
        self,
        #[default(true)] enabled: bool,
        #[default(None)] on_click_label: Option<String>,
        #[default(None)] role: Option<Role>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.clickable_with(enabled, on_click_label, role, interaction_source, on_click)
    }

    /// [`clickable`](ClickableModifiers::clickable) with every parameter, for plain code.
    fn clickable_with(
        self,
        enabled: bool,
        on_click_label: Option<String>,
        role: Option<Role>,
        interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let common = Common::new(interaction_source, None, true, enabled, on_click_label, role, Rc::new(on_click));
        self.then_element(ClickableElement { common })
    }

    /// The same showing `indication` instead (Compose's `clickable(interactionSource, indication, …)`): none shows
    /// nothing. With no source, a node factory's own source is made when first needed.
    /// Kotlin: `clickable(interactionSource: MutableInteractionSource?, indication: Indication?, enabled: Boolean = true, onClickLabel: String? = null, role: Role? = null, onClick: () -> Unit)`
    fn clickable_indication(
        self,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        #[default(true)] enabled: bool,
        #[default(None)] on_click_label: Option<String>,
        #[default(None)] role: Option<Role>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.clickable_indication_with(interaction_source, indication, enabled, on_click_label, role, on_click)
    }

    /// [`clickable_indication`](ClickableModifiers::clickable_indication) with every parameter, for plain code.
    fn clickable_indication_with(
        self,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        enabled: bool,
        on_click_label: Option<String>,
        role: Option<Role>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let on_click: Callback = Rc::new(on_click);
        clickable_with_indication_if_needed(self, interaction_source, indication, move |source, factory| {
            let common = Common::new(source, factory, false, enabled, on_click_label.clone(), role, on_click.clone());
            Modifier::element(ClickableElement { common })
        })
    }

    /// Makes the layout take clicks, double clicks and long clicks (Compose's `Modifier.combinedClickable`): a
    /// click waits for the double tap timeout when there is an `on_double_click`; a long press (or a key held
    /// that long) calls `on_long_click`, felt when `haptic_feedback_enabled`; accessibility gets a long click
    /// action labelled `on_long_click_label`. The rest is [`clickable`](ClickableModifiers::clickable)'s.
    fn combined_clickable(
        self,
        #[default(true)] enabled: bool,
        #[default(None)] on_click_label: Option<String>,
        #[default(None)] role: Option<Role>,
        #[default(None)] on_long_click_label: Option<String>,
        #[default(None)] on_long_click: Option<Callback>,
        #[default(None)] on_double_click: Option<Callback>,
        #[default(true)] haptic_feedback_enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.combined_clickable_with(
            enabled,
            on_click_label,
            role,
            on_long_click_label,
            on_long_click,
            on_double_click,
            haptic_feedback_enabled,
            interaction_source,
            on_click,
        )
    }

    /// [`combined_clickable`](ClickableModifiers::combined_clickable) with every parameter, for plain code.
    fn combined_clickable_with(
        self,
        enabled: bool,
        on_click_label: Option<String>,
        role: Option<Role>,
        on_long_click_label: Option<String>,
        on_long_click: Option<Callback>,
        on_double_click: Option<Callback>,
        haptic_feedback_enabled: bool,
        interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let common = Common::new(interaction_source, None, true, enabled, on_click_label, role, Rc::new(on_click));
        self.then_element(CombinedClickableElement {
            common,
            on_long_click_label,
            on_long_click,
            on_double_click,
            haptic_feedback_enabled,
        })
    }

    /// The same showing `indication` instead (Compose's `combinedClickable(interactionSource, indication, …)`).
    fn combined_clickable_indication(
        self,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        #[default(true)] enabled: bool,
        #[default(None)] on_click_label: Option<String>,
        #[default(None)] role: Option<Role>,
        #[default(None)] on_long_click_label: Option<String>,
        #[default(None)] on_long_click: Option<Callback>,
        #[default(None)] on_double_click: Option<Callback>,
        #[default(true)] haptic_feedback_enabled: bool,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.combined_clickable_indication_with(
            interaction_source,
            indication,
            enabled,
            on_click_label,
            role,
            on_long_click_label,
            on_long_click,
            on_double_click,
            haptic_feedback_enabled,
            on_click,
        )
    }

    /// [`combined_clickable_indication`](ClickableModifiers::combined_clickable_indication) with every parameter,
    /// for plain code.
    fn combined_clickable_indication_with(
        self,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        enabled: bool,
        on_click_label: Option<String>,
        role: Option<Role>,
        on_long_click_label: Option<String>,
        on_long_click: Option<Callback>,
        on_double_click: Option<Callback>,
        haptic_feedback_enabled: bool,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let on_click: Callback = Rc::new(on_click);
        clickable_with_indication_if_needed(self, interaction_source, indication, move |source, factory| {
            let common = Common::new(source, factory, false, enabled, on_click_label.clone(), role, on_click.clone());
            Modifier::element(CombinedClickableElement {
                common,
                on_long_click_label: on_long_click_label.clone(),
                on_long_click: on_long_click.clone(),
                on_double_click: on_double_click.clone(),
                haptic_feedback_enabled,
            })
        })
    }
}

/// Compose's `clickableWithIndicationIfNeeded`: a node factory (or none) goes into the clickable itself; another
/// indication is drawn by `Modifier.indication` over the clickable's source. (Compose's last case, such an
/// indication with no source, remembers a source for both; an indication that isn't a node factory draws nothing
/// more than the content, so the clickable alone is the same.)
pub(crate) fn clickable_with_indication_if_needed(
    modifier: Modifier,
    interaction_source: Option<MutableInteractionSource>,
    indication: Option<Rc<dyn Indication>>,
    create: impl Fn(Option<MutableInteractionSource>, Option<Rc<dyn Indication>>) -> Modifier,
) -> Modifier {
    match indication {
        Some(i) if i.as_node_factory().is_some() => modifier.then(create(interaction_source, Some(i))),
        None => modifier.then(create(interaction_source, None)),
        Some(i) => match interaction_source {
            Some(source) => {
                modifier.then(Modifier.indication(&source, Some(i)).then(create(Some(source.clone()), None)))
            }
            None => modifier.then(create(None, None)),
        },
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The elements

/// What every clickable element has (Compose's ClickableElement's parameters).
#[derive(Clone)]
pub(crate) struct Common {
    interaction_source: Option<MutableInteractionSource>,
    indication_node_factory: Option<Rc<dyn Indication>>,
    use_local_indication: bool,
    enabled: bool,
    on_click_label: Option<String>,
    role: Option<Role>,
    on_click: Callback,
}

impl Common {
    pub(crate) fn new(
        interaction_source: Option<MutableInteractionSource>,
        indication_node_factory: Option<Rc<dyn Indication>>,
        use_local_indication: bool,
        enabled: bool,
        on_click_label: Option<String>,
        role: Option<Role>,
        on_click: Callback,
    ) -> Common {
        Common {
            interaction_source,
            indication_node_factory,
            use_local_indication,
            enabled,
            on_click_label,
            role,
            on_click,
        }
    }
}

impl PartialEq for Common {
    fn eq(&self, o: &Self) -> bool {
        self.interaction_source == o.interaction_source
            && same_indication(&self.indication_node_factory, &o.indication_node_factory)
            && self.use_local_indication == o.use_local_indication
            && self.enabled == o.enabled
            && self.on_click_label == o.on_click_label
            && self.role == o.role
            && Rc::ptr_eq(&self.on_click, &o.on_click)
    }
}

fn same_indication(a: &Option<Rc<dyn Indication>>, b: &Option<Rc<dyn Indication>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => **a == **b,
        (None, None) => true,
        _ => false,
    }
}

fn same_callback(a: &Option<Callback>, b: &Option<Callback>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// `Modifier.clickable`'s element (Compose's ClickableElement).
#[derive(PartialEq)]
struct ClickableElement {
    common: Common,
}

impl ModifierNodeElement for ClickableElement {
    type Node = ClickableNode;
    fn create(&self) -> ClickableNode {
        ClickableNode::new(&self.common, Kind::Click(ClickState::new(Extra::None)))
    }
    fn update(&self, node: &mut ClickableNode) {
        node.core.update_common(&self.common);
    }
    fn name(&self) -> &'static str {
        "clickable"
    }
}

/// `Modifier.combined_clickable`'s element (Compose's CombinedClickableElement).
struct CombinedClickableElement {
    common: Common,
    on_long_click_label: Option<String>,
    on_long_click: Option<Callback>,
    on_double_click: Option<Callback>,
    haptic_feedback_enabled: bool,
}

impl PartialEq for CombinedClickableElement {
    fn eq(&self, o: &Self) -> bool {
        self.common == o.common
            && self.on_long_click_label == o.on_long_click_label
            && same_callback(&self.on_long_click, &o.on_long_click)
            && same_callback(&self.on_double_click, &o.on_double_click)
            && self.haptic_feedback_enabled == o.haptic_feedback_enabled
    }
}

impl ModifierNodeElement for CombinedClickableElement {
    type Node = ClickableNode;
    fn create(&self) -> ClickableNode {
        let combined = CombinedState {
            on_long_click_label: RefCell::new(self.on_long_click_label.clone()),
            on_long_click: RefCell::new(self.on_long_click.clone()),
            on_double_click: RefCell::new(self.on_double_click.clone()),
            haptic_feedback_enabled: Cell::new(self.haptic_feedback_enabled),
            long_key_press_jobs: RefCell::new(Vec::new()),
            double_key_click_states: RefCell::new(Vec::new()),
            down_event: RefCell::new(None),
            long_press_job: RefCell::new(None),
            tap_job: RefCell::new(None),
            is_second_tap: Cell::new(false),
            long_press_triggered: Cell::new(false),
            first_tap_up_time: Cell::new(-1),
            ignore_next_up: Cell::new(false),
        };
        ClickableNode::new(&self.common, Kind::Combined(combined))
    }
    fn update(&self, node: &mut ClickableNode) {
        node.core.update_combined(self);
    }
    fn name(&self) -> &'static str {
        "combinedClickable"
    }
}

/// `Modifier.selectable`'s element (Compose's SelectableElement).
#[derive(PartialEq)]
pub(crate) struct SelectableElement {
    pub(crate) selected: bool,
    pub(crate) common: Common,
}

impl ModifierNodeElement for SelectableElement {
    type Node = ClickableNode;
    fn create(&self) -> ClickableNode {
        ClickableNode::new(&self.common, Kind::Click(ClickState::new(Extra::Selected(self.selected))))
    }
    fn update(&self, node: &mut ClickableNode) {
        node.core.update_extra(Extra::Selected(self.selected));
        node.core.update_common(&self.common);
    }
    fn name(&self) -> &'static str {
        "selectable"
    }
}

/// `Modifier.toggleable`'s element (Compose's ToggleableElement): its click calls `on_value_change` with the other
/// value.
pub(crate) struct ToggleableElement {
    pub(crate) value: bool,
    pub(crate) on_value_change: Rc<dyn Fn(bool)>,
    pub(crate) common: Common,
}

impl PartialEq for ToggleableElement {
    fn eq(&self, o: &Self) -> bool {
        self.value == o.value && Rc::ptr_eq(&self.on_value_change, &o.on_value_change) && self.common == o.common
    }
}

impl ModifierNodeElement for ToggleableElement {
    type Node = ClickableNode;
    fn create(&self) -> ClickableNode {
        let extra = Extra::Toggle { value: self.value, on_value_change: self.on_value_change.clone() };
        let node = ClickableNode::new(&self.common, Kind::Click(ClickState::new(extra)));
        node.core.set_toggle_click();
        node
    }
    fn update(&self, node: &mut ClickableNode) {
        node.core.update_extra(Extra::Toggle { value: self.value, on_value_change: self.on_value_change.clone() });
        let mut common = self.common.clone();
        if let Some(click) = node.core.toggle_click() {
            common.on_click = click;
        }
        node.core.update_common(&common);
    }
    fn name(&self) -> &'static str {
        "toggleable"
    }
}

/// `Modifier.tri_state_toggleable`'s element (Compose's TriStateToggleableElement).
#[derive(PartialEq)]
pub(crate) struct TriStateToggleableElement {
    pub(crate) state: ToggleableState,
    pub(crate) common: Common,
}

impl ModifierNodeElement for TriStateToggleableElement {
    type Node = ClickableNode;
    fn create(&self) -> ClickableNode {
        ClickableNode::new(&self.common, Kind::Click(ClickState::new(Extra::TriState(self.state))))
    }
    fn update(&self, node: &mut ClickableNode) {
        node.core.update_extra(Extra::TriState(self.state));
        node.core.update_common(&self.common);
    }
    fn name(&self) -> &'static str {
        "triStateToggleable"
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node

/// What a plain click node adds to what it means (Compose's ClickableNode subclasses' `applyAdditionalSemantics`).
enum Extra {
    None,
    /// A selectable's: selected or not.
    Selected(bool),
    /// A toggleable's: its value, and what it is told on a click.
    Toggle {
        value: bool,
        on_value_change: Rc<dyn Fn(bool)>,
    },
    /// A tri-state toggleable's state.
    TriState(ToggleableState),
}

/// A plain click node's own state (Compose's ClickableNode).
struct ClickState {
    down_event: RefCell<Option<PointerInputChange>>,
    extra: RefCell<Extra>,
    /// A toggleable's click, reading its value and callback when called (Compose's `_onClick`).
    toggle_click: RefCell<Option<Callback>>,
}

impl ClickState {
    fn new(extra: Extra) -> ClickState {
        ClickState { down_event: RefCell::new(None), extra: RefCell::new(extra), toggle_click: RefCell::new(None) }
    }
}

/// A combined click node's own state (Compose's CombinedClickableNode).
struct CombinedState {
    on_long_click_label: RefCell<Option<String>>,
    on_long_click: RefCell<Option<Callback>>,
    on_double_click: RefCell<Option<Callback>>,
    haptic_feedback_enabled: Cell<bool>,
    /// The long click each key held waits for, by key.
    long_key_press_jobs: RefCell<Vec<(Key, Job)>>,
    /// The first click of each key that may become a double click, by key.
    double_key_click_states: RefCell<Vec<(Key, DoubleKeyClickState)>>,
    down_event: RefCell<Option<PointerInputChange>>,
    long_press_job: RefCell<Option<Job>>,
    tap_job: RefCell<Option<Job>>,
    is_second_tap: Cell<bool>,
    long_press_triggered: Cell<bool>,
    first_tap_up_time: Cell<i64>,
    ignore_next_up: Cell<bool>,
}

/// A key's first click waiting for a second (Compose's DoubleKeyClickState).
#[derive(Clone)]
struct DoubleKeyClickState {
    job: Job,
    double_tap_min_time_millis_elapsed: Rc<Cell<bool>>,
}

/// Which clickable it is.
enum Kind {
    Click(ClickState),
    Combined(CombinedState),
}

/// What a clickable node and what it launches and delegates to share (Compose's AbstractClickableNode's fields):
/// the node's own borrow is never needed to act on it.
pub(crate) struct ClickableCore {
    me: Weak<ClickableCore>,
    interaction_source: RefCell<Option<MutableInteractionSource>>,
    indication_node_factory: RefCell<Option<Rc<dyn Indication>>>,
    use_local_indication: Cell<bool>,
    enabled: Cell<bool>,
    on_click_label: RefCell<Option<String>>,
    role: Cell<Option<Role>>,
    on_click: RefCell<Callback>,
    /// Its focusable's state (the focusable delegated to while it is enabled).
    focusable: Rc<FocusableShared>,
    focusable_node: RefCell<Option<DelegatedNode<FocusableNode>>>,
    local_indication_node_factory: RefCell<Option<Rc<dyn Indication>>>,
    gesture_node: RefCell<Option<DelegatedNode<GestureNode>>>,
    gesture_state: Cell<GestureState>,
    indication_node: RefCell<Option<DelegatedNode<IndicationNode>>>,
    press_interaction: Cell<Option<PressInteraction::Press>>,
    hover_interaction: Cell<Option<HoverInteraction::Enter>>,
    current_key_press_interactions: RefCell<Vec<(Key, PressInteraction::Press)>>,
    center_offset: Cell<Offset>,
    /// The source it was given (it makes one of its own when given none).
    user_provided_interaction_source: RefCell<Option<MutableInteractionSource>>,
    lazily_create_indication: Cell<bool>,
    delay_job: RefCell<Option<Job>>,
    handle: RefCell<Option<NodeHandle>>,
    kind: Kind,
}

/// The node of the clickables (Compose's ClickableNode, CombinedClickableNode and the selectable and toggleable
/// nodes built on them).
pub struct ClickableNode {
    core: Rc<ClickableCore>,
}

impl ClickableNode {
    fn new(common: &Common, kind: Kind) -> ClickableNode {
        let core = Rc::new_cyclic(|me: &Weak<ClickableCore>| {
            let on_focus = me.clone();
            let focusable = FocusableNode::new(
                common.interaction_source.clone(),
                Focusability::SystemDefined,
                Some(Rc::new(move |focused| {
                    if let Some(core) = on_focus.upgrade() {
                        core.on_focus_change(focused);
                    }
                })),
            );
            ClickableCore {
                me: me.clone(),
                interaction_source: RefCell::new(common.interaction_source.clone()),
                indication_node_factory: RefCell::new(common.indication_node_factory.clone()),
                use_local_indication: Cell::new(common.use_local_indication),
                enabled: Cell::new(common.enabled),
                on_click_label: RefCell::new(common.on_click_label.clone()),
                role: Cell::new(common.role),
                on_click: RefCell::new(common.on_click.clone()),
                focusable: focusable.shared().clone(),
                focusable_node: RefCell::new(None),
                local_indication_node_factory: RefCell::new(None),
                gesture_node: RefCell::new(None),
                gesture_state: Cell::new(GestureState::Idle),
                indication_node: RefCell::new(None),
                press_interaction: Cell::new(None),
                hover_interaction: Cell::new(None),
                current_key_press_interactions: RefCell::new(Vec::new()),
                center_offset: Cell::new(Offset::Zero),
                user_provided_interaction_source: RefCell::new(common.interaction_source.clone()),
                lazily_create_indication: Cell::new(common.interaction_source.is_none()),
                delay_job: RefCell::new(None),
                handle: RefCell::new(None),
                kind,
            }
        });
        ClickableNode { core }
    }
}

impl ClickableCore {
    fn rc(&self) -> Rc<ClickableCore> {
        self.me.upgrade().expect("kompoz: a clickable's state outlived it")
    }

    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    fn cx(&self) -> Option<NodeCx> {
        self.handle().and_then(|h| h.cx())
    }

    /// The node's coroutine scope, while it is attached.
    fn scope(&self) -> Option<CoroutineScope> {
        self.handle().and_then(|h| h.coroutine_scope())
    }

    fn invalidate_semantics(&self) {
        if let Some(h) = self.handle() {
            h.invalidate_semantics();
        }
    }

    fn source(&self) -> Option<MutableInteractionSource> {
        self.interaction_source.borrow().clone()
    }

    /// Launches `f` in the node's scope; nothing if it isn't attached.
    fn launch(&self, f: impl std::future::Future<Output = ()> + 'static) -> Option<Job> {
        self.scope().filter(CoroutineScope::is_active).map(|s| s.launch(f))
    }

    /// Emits `interaction` to `source` from the node's scope, or at once if it has none (Compose's `launch { emit
    /// }`, with its fallback to `tryEmit` once the scope is gone).
    fn emit(&self, source: &MutableInteractionSource, interaction: impl Interaction) {
        match self.scope().filter(CoroutineScope::is_active) {
            Some(scope) => {
                let source = source.clone();
                scope.launch(async move { source.emit(interaction).await });
            }
            None => {
                source.try_emit(interaction);
            }
        }
    }

    fn should_lazily_create_indication(&self) -> bool {
        self.user_provided_interaction_source.borrow().is_none()
    }

    /// Its click, user code: run once the node is released (Compose's `performClick`; KompozR's platforms play no
    /// click sound).
    fn perform_click_later(&self) {
        let on_click = self.on_click.borrow().clone();
        when_node_released(move || on_click());
    }

    /// Compose's `updateCommon`.
    fn update_common(&self, common: &Common) {
        let mut is_indication_node_dirty = false;
        if *self.user_provided_interaction_source.borrow() != common.interaction_source {
            self.dispose_interactions();
            *self.user_provided_interaction_source.borrow_mut() = common.interaction_source.clone();
            *self.interaction_source.borrow_mut() = common.interaction_source.clone();
            is_indication_node_dirty = true;
        }
        if !same_indication(&self.indication_node_factory.borrow(), &common.indication_node_factory) {
            *self.indication_node_factory.borrow_mut() = common.indication_node_factory.clone();
            is_indication_node_dirty = true;
        }
        if self.use_local_indication.get() != common.use_local_indication {
            self.use_local_indication.set(common.use_local_indication);
            if common.use_local_indication {
                self.on_observed_reads_changed();
            }
            is_indication_node_dirty = true;
        }
        if self.enabled.get() != common.enabled {
            if common.enabled {
                self.delegate_focusable();
            } else {
                self.undelegate_focusable();
                self.dispose_interactions();
            }
            self.invalidate_semantics();
            if !common.enabled {
                self.undelegate_gesture_node();
                self.gesture_state.set(GestureState::Idle);
            }
            self.enabled.set(common.enabled);
        }
        if *self.on_click_label.borrow() != common.on_click_label {
            *self.on_click_label.borrow_mut() = common.on_click_label.clone();
            self.invalidate_semantics();
        }
        if self.role.get() != common.role {
            self.role.set(common.role);
            self.invalidate_semantics();
        }
        *self.on_click.borrow_mut() = common.on_click.clone();
        if self.lazily_create_indication.get() != self.should_lazily_create_indication() {
            self.lazily_create_indication.set(self.should_lazily_create_indication());
            if !self.lazily_create_indication.get() && self.indication_node.borrow().is_none() {
                is_indication_node_dirty = true;
            }
        }
        if is_indication_node_dirty {
            self.recreate_indication_if_needed();
        }
        FocusableNode::update_shared(&self.focusable, self.source());
    }

    /// A selectable's or a toggleable's own part changed: told to accessibility.
    fn update_extra(&self, new: Extra) {
        let Kind::Click(state) = &self.kind else { return };
        let changed = match (&*state.extra.borrow(), &new) {
            (Extra::Selected(a), Extra::Selected(b)) => a != b,
            (Extra::Toggle { value: a, .. }, Extra::Toggle { value: b, .. }) => a != b,
            (Extra::TriState(a), Extra::TriState(b)) => a != b,
            _ => true,
        };
        *state.extra.borrow_mut() = new;
        if changed {
            self.invalidate_semantics();
        }
    }

    /// A toggleable's click: its callback told the other value, both read when it is called.
    fn toggle_click(&self) -> Option<Callback> {
        let Kind::Click(state) = &self.kind else { return None };
        state.toggle_click.borrow().clone()
    }

    fn set_toggle_click(&self) {
        let Kind::Click(state) = &self.kind else { return };
        let me = self.me.clone();
        let click: Callback = Rc::new(move || {
            let Some(core) = me.upgrade() else { return };
            let Kind::Click(state) = &core.kind else { return };
            let call = match &*state.extra.borrow() {
                Extra::Toggle { value, on_value_change } => Some((on_value_change.clone(), !*value)),
                _ => None,
            };
            if let Some((f, value)) = call {
                f(value);
            }
        });
        *state.toggle_click.borrow_mut() = Some(click.clone());
        *self.on_click.borrow_mut() = click;
    }

    /// Compose's CombinedClickableNode.update.
    fn update_combined(&self, element: &CombinedClickableElement) {
        let Kind::Combined(state) = &self.kind else { return };
        state.haptic_feedback_enabled.set(element.haptic_feedback_enabled);
        let mut reset_pointer_input_handling = false;
        if *state.on_long_click_label.borrow() != element.on_long_click_label {
            *state.on_long_click_label.borrow_mut() = element.on_long_click_label.clone();
            self.invalidate_semantics();
        }
        // Adding or taking away the long click: presses under way are cancelled, the semantics change.
        if state.on_long_click.borrow().is_none() != element.on_long_click.is_none() {
            self.dispose_interactions();
            self.invalidate_semantics();
            reset_pointer_input_handling = true;
        }
        *state.on_long_click.borrow_mut() = element.on_long_click.clone();
        if state.on_double_click.borrow().is_none() != element.on_double_click.is_none() {
            reset_pointer_input_handling = true;
        }
        *state.on_double_click.borrow_mut() = element.on_double_click.clone();
        if self.enabled.get() != element.common.enabled {
            reset_pointer_input_handling = true;
        }
        self.update_common(&element.common);
        if reset_pointer_input_handling {
            self.combined_cancel_input();
        }
    }

    // ---- Attach and detach ----

    /// Compose's `onAttach`.
    fn on_attach(&self, cx: &NodeCx) {
        *self.handle.borrow_mut() = Some(cx.handle());
        self.on_observed_reads_changed();
        if !self.lazily_create_indication.get() {
            self.initialize_indication_and_interaction_source_if_needed();
        }
        if self.enabled.get() {
            self.delegate_focusable();
        }
    }

    /// Compose's `onDetach`: what it emitted ended, its own source forgotten (it may be reused), its indication and
    /// gesture nodes taken back.
    fn on_detach(&self, cx: &NodeCx) {
        self.dispose_interactions();
        if self.user_provided_interaction_source.borrow().is_none() {
            *self.interaction_source.borrow_mut() = None;
        }
        let indication = self.indication_node.borrow_mut().take();
        if let Some(node) = indication {
            cx.undelegate(&node);
        }
        let gesture = self.gesture_node.borrow_mut().take();
        if let Some(node) = gesture {
            cx.undelegate(&node);
        }
    }

    /// Compose's `onObservedReadsChanged`: [`LocalIndication`] read (observed) again; another factory makes the
    /// indication again. (A provider's change reaches a node as its locals are read: this runs at attach and before
    /// each input the node handles.)
    fn on_observed_reads_changed(&self) {
        if !self.use_local_indication.get() {
            return;
        }
        let Some(handle) = self.handle() else { return };
        let indication = handle.observe_reads(|| handle.current_value_of(&LocalIndication));
        assert!(indication.as_node_factory().is_some(), "{}", unsupported_indication_exception_message());
        let previous = self.local_indication_node_factory.replace(Some(indication.clone()));
        if let Some(previous) = previous
            && *previous != *indication
        {
            self.recreate_indication_if_needed();
        }
    }

    fn delegate_focusable(&self) {
        if self.focusable_node.borrow().is_some() {
            return;
        }
        let Some(cx) = self.cx() else { return };
        let node = FocusableNode::from_shared(self.focusable.clone(), Focusability::SystemDefined);
        let delegated = cx.delegate(node);
        *self.focusable_node.borrow_mut() = Some(delegated);
    }

    fn undelegate_focusable(&self) {
        let node = self.focusable_node.borrow_mut().take();
        if let (Some(node), Some(cx)) = (node, self.cx()) {
            cx.undelegate(&node);
        }
    }

    fn undelegate_gesture_node(&self) {
        let node = self.gesture_node.borrow_mut().take();
        if let (Some(node), Some(cx)) = (node, self.cx()) {
            cx.undelegate(&node);
        }
    }

    /// Ends what it emitted at once (Compose's `disposeInteractions`).
    fn dispose_interactions(&self) {
        if let Some(source) = self.source() {
            if let Some(press) = self.press_interaction.get() {
                source.try_emit(PressInteraction::Cancel::new(press));
            }
            if let Some(enter) = self.hover_interaction.get() {
                source.try_emit(HoverInteraction::Exit::new(enter));
            }
            for (_, press) in self.current_key_press_interactions.borrow().iter() {
                source.try_emit(PressInteraction::Cancel::new(*press));
            }
        }
        self.press_interaction.set(None);
        self.hover_interaction.set(None);
        self.current_key_press_interactions.borrow_mut().clear();
    }

    /// Compose's `onFocusChange`: focused, its indication made; unfocused, the key presses under way cancelled.
    fn on_focus_change(&self, is_focused: bool) {
        if is_focused {
            self.initialize_indication_and_interaction_source_if_needed();
            return;
        }
        let presses: Vec<PressInteraction::Press> =
            self.current_key_press_interactions.borrow_mut().drain(..).map(|(_, p)| p).collect();
        if let Some(source) = self.source() {
            for press in presses {
                self.emit(&source, PressInteraction::Cancel::new(press));
            }
        }
        self.on_cancel_key_input();
    }

    fn recreate_indication_if_needed(&self) {
        if self.indication_node.borrow().is_some() || !self.lazily_create_indication.get() {
            let old = self.indication_node.borrow_mut().take();
            if let (Some(old), Some(cx)) = (old, self.cx()) {
                cx.undelegate(&old);
            }
            self.initialize_indication_and_interaction_source_if_needed();
        }
    }

    /// Makes its indication node (and a source of its own, if it has none) unless it has one (Compose's
    /// `initializeIndicationAndInteractionSourceIfNeeded`).
    fn initialize_indication_and_interaction_source_if_needed(&self) {
        if self.indication_node.borrow().is_some() {
            return;
        }
        let factory = if self.use_local_indication.get() {
            self.local_indication_node_factory.borrow().clone()
        } else {
            self.indication_node_factory.borrow().clone()
        };
        let Some(factory) = factory else { return };
        let Some(node_factory) = factory.as_node_factory() else { return };
        let Some(cx) = self.cx() else { return };
        if self.interaction_source.borrow().is_none() {
            *self.interaction_source.borrow_mut() = Some(MutableInteractionSource::new());
        }
        let source = self.source().expect("its source");
        FocusableNode::update_shared(&self.focusable, Some(source.clone()));
        let node = IndicationNode::create(node_factory, source.as_interaction_source());
        let delegated = cx.delegate(node);
        *self.indication_node.borrow_mut() = Some(delegated);
    }

    fn initialize_gesture_coordination(&self) {
        if self.gesture_node.borrow().is_some() {
            return;
        }
        let Some(cx) = self.cx() else { return };
        let connection: Rc<dyn GestureConnection> = self.rc();
        let node = cx.delegate(GestureNode::new(connection));
        *self.gesture_node.borrow_mut() = Some(node);
    }

    // ---- Pointer input ----

    /// A finger's minimum touch target past `size`, half of it each side (Compose's `getExtendedTouchPadding`).
    fn extended_touch_padding(&self, size: IntSize) -> Size {
        let minimum = self
            .handle()
            .map_or_else(|| LocalViewConfiguration.default_value(), |h| h.current_value_of(&LocalViewConfiguration))
            .minimum_touch_target_size;
        Size::new(
            (minimum.width - size.width as f32).max(0.0) / 2.0,
            (minimum.height - size.height as f32).max(0.0) / 2.0,
        )
    }

    /// Compose's AbstractClickableNode.onPointerEvent, then the kind's.
    fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.on_observed_reads_changed();
        self.center_offset.set(bounds.center().to_offset());
        self.initialize_indication_and_interaction_source_if_needed();
        if self.enabled.get() {
            self.initialize_gesture_coordination();
            if pass == PointerEventPass::Main {
                match event.r#type {
                    PointerEventType::Enter => self.emit_hover_enter(),
                    PointerEventType::Exit => self.emit_hover_exit(),
                    _ => {}
                }
            }
        }
        match &self.kind {
            Kind::Click(state) => self.click_pointer_event(state, event, pass, bounds),
            Kind::Combined(state) => self.combined_pointer_event(state, event, pass, bounds),
        }
    }

    /// Compose's AbstractClickableNode.onCancelPointerInput, then the kind's.
    fn on_cancel_pointer_input(&self) {
        if let (Some(source), Some(enter)) = (self.source(), self.hover_interaction.get()) {
            source.try_emit(HoverInteraction::Exit::new(enter));
        }
        self.hover_interaction.set(None);
        match &self.kind {
            Kind::Click(_) => self.click_cancel_input(),
            Kind::Combined(_) => self.combined_cancel_input(),
        }
    }

    fn click_pointer_event(&self, state: &ClickState, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        match pass {
            PointerEventPass::Main => {
                if state.down_event.borrow().is_none() {
                    if is_changed_to_down(event, true, crate::gestures::FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY)
                    {
                        let down = event.changes[0].clone();
                        down.consume();
                        let position = down.position;
                        *state.down_event.borrow_mut() = Some(down);
                        if self.enabled.get() {
                            self.gesture_state.set(GestureState::Waiting);
                            self.handle_press_interaction_start(position);
                        }
                    }
                } else if event.changes.iter().all(|c| c.changed_to_up()) {
                    // All pointers are up.
                    event.changes[0].consume();
                    let down = state.down_event.borrow_mut().take();
                    if self.enabled.get() {
                        self.gesture_state.set(GestureState::Recognized);
                        let position = down.map_or(Offset::Zero, |d| d.position);
                        self.handle_press_interaction_release(position);
                        self.perform_click_later();
                    }
                } else if self.is_cancelled_by(event, bounds) {
                    self.click_cancel_input();
                }
            }
            PointerEventPass::Final => {
                let cancelled = state.down_event.borrow().as_ref().is_some_and(|down| consumed_by_another(event, down));
                if cancelled {
                    self.click_cancel_input();
                }
                // The first Final pass after the click was recognized: back to idle.
                if self.gesture_state.get() == GestureState::Recognized {
                    self.gesture_state.set(GestureState::Idle);
                }
            }
            PointerEventPass::Initial => {}
        }
    }

    /// Whether a move cancels the press: consumed by another, or out of its bounds (and touch target).
    fn is_cancelled_by(&self, event: &PointerEvent, bounds: IntSize) -> bool {
        let padding = self.extended_touch_padding(bounds);
        event.changes.iter().any(|c| c.is_consumed() || c.is_out_of_bounds(bounds, padding))
    }

    fn click_cancel_input(&self) {
        if let Kind::Click(state) = &self.kind {
            *state.down_event.borrow_mut() = None;
        }
        self.handle_press_interaction_cancel();
        self.gesture_state.set(GestureState::Idle);
    }

    fn combined_pointer_event(
        &self,
        state: &CombinedState,
        event: &PointerEvent,
        pass: PointerEventPass,
        bounds: IntSize,
    ) {
        match pass {
            PointerEventPass::Main => {
                if state.down_event.borrow().is_none() {
                    if is_changed_to_down(event, true, crate::gestures::FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY)
                    {
                        self.combined_handle_down(state, event.changes[0].clone());
                    }
                    return;
                }
                if event.is_deep_press() {
                    self.handle_deep_press(state);
                }
                if state.long_press_triggered.get() {
                    // The long click was called: everything consumed until the pointers are up, which ends it
                    // even when a child consumed the up.
                    if event.changes.iter().all(|c| c.changed_to_up_ignore_consumed()) {
                        let up = &event.changes[0];
                        up.consume();
                        self.combined_handle_up(state, up.uptime_millis);
                    } else {
                        event.changes.iter().for_each(|c| c.consume());
                    }
                    return;
                }
                if event.changes.iter().all(|c| c.changed_to_up()) {
                    let up = &event.changes[0];
                    up.consume();
                    self.combined_handle_up(state, up.uptime_millis);
                } else if self.is_cancelled_by(event, bounds) {
                    self.combined_cancel_input();
                }
            }
            PointerEventPass::Final => {
                let cancelled = !state.long_press_triggered.get()
                    && state.down_event.borrow().as_ref().is_some_and(|down| consumed_by_another(event, down));
                if cancelled {
                    self.combined_cancel_input();
                }
            }
            PointerEventPass::Initial => {}
        }
    }

    fn combined_handle_down(&self, state: &CombinedState, down: PointerInputChange) {
        down.consume();
        let (position, uptime) = (down.position, down.uptime_millis);
        *state.down_event.borrow_mut() = Some(down);
        if !self.enabled.get() {
            return;
        }
        let tap_active = state.tap_job.borrow().as_ref().is_some_and(Job::is_active);
        if tap_active {
            let min_time = self.view_configuration().double_tap_min_time_millis();
            if uptime - state.first_tap_up_time.get() < min_time {
                // Too soon after the first: this down doesn't count (no long press, no press shown).
                state.ignore_next_up.set(true);
                return;
            }
            state.is_second_tap.set(true);
            if let Some(job) = state.tap_job.borrow_mut().take() {
                job.cancel();
            }
        }
        state.long_press_triggered.set(false);
        self.handle_press_interaction_start(position);
        if state.on_long_click.borrow().is_some() {
            let me = self.me.clone();
            let timeout = self.view_configuration().long_press_timeout_millis();
            let job = self.launch(async move {
                delay_ms(timeout as u64).await;
                let Some(core) = me.upgrade() else { return };
                let Kind::Combined(state) = &core.kind else { return };
                let on_long_click = state.on_long_click.borrow().clone();
                if let Some(f) = on_long_click {
                    f();
                }
                core.perform_long_press_haptics(state);
                state.long_press_triggered.set(true);
                if let Some(job) = state.tap_job.borrow_mut().take() {
                    job.cancel();
                }
                *state.long_press_job.borrow_mut() = None;
            });
            *state.long_press_job.borrow_mut() = job.filter(Job::is_active);
        }
    }

    fn combined_handle_up(&self, state: &CombinedState, uptime_millis: i64) {
        let down = state.down_event.borrow_mut().take();
        if self.enabled.get() && !state.ignore_next_up.get() {
            self.handle_press_interaction_release(down.map_or(Offset::Zero, |d| d.position));
            state.first_tap_up_time.set(uptime_millis);
            if !state.long_press_triggered.get() {
                if state.is_second_tap.get() {
                    let on_double_click = state.on_double_click.borrow().clone();
                    if let Some(f) = on_double_click {
                        when_node_released(move || f());
                    }
                } else if state.on_double_click.borrow().is_some() {
                    // (the click sound would be played now, even if it becomes a double click)
                    let me = self.me.clone();
                    let timeout = self.view_configuration().double_tap_timeout_millis();
                    let job = self.launch(async move {
                        delay_ms(timeout as u64).await;
                        let Some(core) = me.upgrade() else { return };
                        let on_click = core.on_click.borrow().clone();
                        on_click();
                        if let Kind::Combined(state) = &core.kind {
                            *state.tap_job.borrow_mut() = None;
                        }
                    });
                    *state.tap_job.borrow_mut() = job.filter(Job::is_active);
                } else {
                    self.perform_click_later();
                }
            }
        }
        state.ignore_next_up.set(false);
        state.is_second_tap.set(false);
        if let Some(job) = state.long_press_job.borrow_mut().take() {
            job.cancel();
        }
        state.long_press_triggered.set(false);
    }

    /// A deep press is a long click at once (Compose's `handleDeepPress`).
    fn handle_deep_press(&self, state: &CombinedState) {
        let on_long_click = state.on_long_click.borrow().clone();
        if state.long_press_triggered.get() || !self.enabled.get() {
            return;
        }
        let Some(f) = on_long_click else { return };
        if let Some(job) = state.long_press_job.borrow_mut().take() {
            job.cancel();
        }
        when_node_released(move || f());
        self.perform_long_press_haptics(state);
        state.long_press_triggered.set(true);
    }

    fn perform_long_press_haptics(&self, state: &CombinedState) {
        if state.haptic_feedback_enabled.get()
            && let Some(h) = self.handle()
        {
            h.current_value_of(&LocalHapticFeedback).perform_haptic_feedback(HapticFeedbackType::LongPress);
        }
    }

    fn combined_cancel_input(&self) {
        if let Kind::Combined(state) = &self.kind {
            *state.down_event.borrow_mut() = None;
            if let Some(job) = state.long_press_job.borrow_mut().take() {
                job.cancel();
            }
            if let Some(job) = state.tap_job.borrow_mut().take() {
                job.cancel();
            }
            state.is_second_tap.set(false);
            state.long_press_triggered.set(false);
            state.first_tap_up_time.set(-1);
            state.ignore_next_up.set(false);
        }
        self.handle_press_interaction_cancel();
    }

    fn view_configuration(&self) -> kompoz_ui::platform::ViewConfiguration {
        self.handle()
            .map_or_else(|| LocalViewConfiguration.default_value(), |h| h.current_value_of(&LocalViewConfiguration))
    }

    // ---- Press and hover interactions ----

    /// Whether a press waits before it is shown (Compose's `delayPressInteraction`): a gesture above is still
    /// deciding, or the host is in a scrolling container.
    fn delay_press_interaction(&self) -> bool {
        self.cx().is_some_and(|cx| has_waiting_parent(&cx) || is_compose_root_in_scrollable_container(&cx))
    }

    /// Compose's `handlePressInteractionStart`.
    fn handle_press_interaction_start(&self, position: Offset) {
        let Some(source) = self.source() else { return };
        let press = PressInteraction::Press::new(position);
        if self.delay_press_interaction() {
            let me = self.me.clone();
            let job = self.launch(async move {
                delay_ms(TapIndicationDelay as u64).await;
                source.emit(press).await;
                if let Some(core) = me.upgrade() {
                    core.press_interaction.set(Some(press));
                }
            });
            *self.delay_job.borrow_mut() = job;
        } else {
            self.press_interaction.set(Some(press));
            self.emit(&source, press);
        }
    }

    /// Compose's `handlePressInteractionRelease`: a press still waiting to be shown is shown and released at once.
    fn handle_press_interaction_release(&self, offset: Offset) {
        let Some(source) = self.source() else { return };
        let job = self.delay_job.borrow().clone();
        match job {
            Some(job) if job.is_active() => {
                job.cancel();
                self.launch(async move {
                    job.join().await;
                    let press = PressInteraction::Press::new(offset);
                    source.emit(press).await;
                    source.emit(PressInteraction::Release::new(press)).await;
                });
            }
            _ => {
                if let Some(press) = self.press_interaction.get() {
                    self.emit(&source, PressInteraction::Release::new(press));
                }
            }
        }
        self.press_interaction.set(None);
    }

    /// Compose's `handlePressInteractionCancel`: a press not yet shown is never shown.
    fn handle_press_interaction_cancel(&self) {
        let Some(source) = self.source() else { return };
        let job = self.delay_job.borrow().clone();
        match job {
            Some(job) if job.is_active() => job.cancel(),
            _ => {
                if let Some(press) = self.press_interaction.get() {
                    self.emit(&source, PressInteraction::Cancel::new(press));
                }
            }
        }
        self.press_interaction.set(None);
    }

    fn emit_hover_enter(&self) {
        if self.hover_interaction.get().is_none() {
            let interaction = HoverInteraction::Enter::new();
            if let Some(source) = self.source() {
                self.emit(&source, interaction);
            }
            self.hover_interaction.set(Some(interaction));
        }
    }

    fn emit_hover_exit(&self) {
        if let Some(enter) = self.hover_interaction.take()
            && let Some(source) = self.source()
        {
            self.emit(&source, HoverInteraction::Exit::new(enter));
        }
    }

    // ---- Keys ----

    /// Compose's AbstractClickableNode.onKeyEvent: an Enter-like key going down presses it (once: repeats are
    /// ignored), going up clicks it if it pressed it.
    fn on_key_event(&self, event: &KeyEvent) -> bool {
        self.on_observed_reads_changed();
        // Key events usually need the focus, but one a focused child didn't take bubbles up here unfocused.
        self.initialize_indication_and_interaction_source_if_needed();
        if self.enabled.get() && is_press(event) {
            let mut was_interaction_handled = false;
            let known = self.current_key_press_interactions.borrow().iter().any(|(k, _)| *k == event.key);
            if !known {
                let press = PressInteraction::Press::new(self.center_offset.get());
                self.current_key_press_interactions.borrow_mut().push((event.key, press));
                if let Some(source) = self.source() {
                    self.emit(&source, press);
                }
                was_interaction_handled = true;
            }
            self.on_click_key_down_event(event) || was_interaction_handled
        } else if self.enabled.get() && is_click(event) {
            let press = {
                let mut presses = self.current_key_press_interactions.borrow_mut();
                let at = presses.iter().position(|(k, _)| *k == event.key);
                at.map(|i| presses.remove(i).1)
            };
            if let Some(press) = press {
                if let Some(source) = self.source() {
                    self.emit(&source, PressInteraction::Release::new(press));
                }
                // (not clicked unless it was pressed: focused after the down, or reused since)
                self.on_click_key_up_event(event);
            }
            press.is_some()
        } else {
            false
        }
    }

    fn on_click_key_down_event(&self, event: &KeyEvent) -> bool {
        let Kind::Combined(state) = &self.kind else { return false };
        let key = event.key;
        let mut handled_by_long_click = false;
        if state.on_long_click.borrow().is_some() && !state.long_key_press_jobs.borrow().iter().any(|(k, _)| *k == key)
        {
            let me = self.me.clone();
            let timeout = self.view_configuration().long_press_timeout_millis();
            let job = self.launch(async move {
                delay_ms(timeout as u64).await;
                let Some(core) = me.upgrade() else { return };
                let Kind::Combined(state) = &core.kind else { return };
                let on_long_click = state.on_long_click.borrow().clone();
                if let Some(f) = on_long_click {
                    f();
                }
            });
            if let Some(job) = job {
                state.long_key_press_jobs.borrow_mut().push((key, job));
            }
            handled_by_long_click = true;
        }
        let double = state.double_key_click_states.borrow().iter().find(|(k, _)| *k == key).map(|(_, s)| s.clone());
        // A second down: maybe a double click.
        if let Some(double) = double {
            if double.job.is_active() {
                double.job.cancel();
                // Before the double tap minimum time: not a double click, the first one a click of its own.
                if !double.double_tap_min_time_millis_elapsed.get() {
                    let on_click = self.on_click.borrow().clone();
                    when_node_released(move || on_click());
                    state.double_key_click_states.borrow_mut().retain(|(k, _)| *k != key);
                }
            } else {
                // The timeout passed and clicked already: no longer a double click.
                state.double_key_click_states.borrow_mut().retain(|(k, _)| *k != key);
            }
        }
        handled_by_long_click
    }

    fn on_click_key_up_event(&self, event: &KeyEvent) -> bool {
        let state = match &self.kind {
            Kind::Click(_) => {
                self.perform_click_later();
                return true;
            }
            Kind::Combined(state) => state,
        };
        let key = event.key;
        let mut long_click_invoked = false;
        let long = {
            let mut jobs = state.long_key_press_jobs.borrow_mut();
            let at = jobs.iter().position(|(k, _)| *k == key);
            at.map(|i| jobs.remove(i).1)
        };
        if let Some(job) = long {
            if job.is_active() {
                job.cancel();
            } else {
                // The timeout passed: the long click was called, so no click.
                long_click_invoked = true;
            }
        }
        if state.on_double_click.borrow().is_some() {
            let first = !state.double_key_click_states.borrow().iter().any(|(k, _)| *k == key);
            if first {
                // Only a click that wasn't a long one may become a double one.
                if !long_click_invoked {
                    // (the click sound would be played now, even if it becomes a double click)
                    let elapsed = Rc::new(Cell::new(false));
                    let (me, e) = (self.me.clone(), elapsed.clone());
                    let vc = self.view_configuration();
                    let (min_time, timeout) = (vc.double_tap_min_time_millis(), vc.double_tap_timeout_millis());
                    let job = self.launch(async move {
                        delay_ms(min_time as u64).await;
                        e.set(true);
                        delay_ms((timeout - min_time).max(0) as u64).await;
                        // No second press in time: a click.
                        if let Some(core) = me.upgrade() {
                            let on_click = core.on_click.borrow().clone();
                            on_click();
                        }
                    });
                    if let Some(job) = job {
                        let entry = DoubleKeyClickState { job, double_tap_min_time_millis_elapsed: elapsed };
                        state.double_key_click_states.borrow_mut().push((key, entry));
                    }
                }
            } else {
                if !long_click_invoked {
                    let on_double_click = state.on_double_click.borrow().clone();
                    if let Some(f) = on_double_click {
                        when_node_released(move || f());
                    }
                }
                state.double_key_click_states.borrow_mut().retain(|(k, _)| *k != key);
            }
        } else if !long_click_invoked {
            self.perform_click_later();
        }
        true
    }

    /// The focus left while keys were held (Compose's `onCancelKeyInput`; CombinedClickableNode's resets its key
    /// state).
    fn on_cancel_key_input(&self) {
        self.reset_key_press_state();
    }

    fn reset_key_press_state(&self) {
        if let Kind::Combined(state) = &self.kind {
            for (_, job) in state.long_key_press_jobs.borrow_mut().drain(..) {
                job.cancel();
            }
            for (_, s) in state.double_key_click_states.borrow_mut().drain(..) {
                s.job.cancel();
            }
        }
    }

    // ---- Semantics ----

    /// Compose's `applySemantics`: its role, its click action, focusable or disabled, and its kind's own.
    fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        if let Some(role) = self.role.get() {
            config.set_role(role);
        }
        let me = self.me.clone();
        let click: Rc<ActionFn> = Rc::new(move || {
            if let Some(core) = me.upgrade() {
                let on_click = core.on_click.borrow().clone();
                on_click();
            }
            true
        });
        let label = self.on_click_label.borrow().clone();
        config.set(SemanticsActions::OnClick, AccessibilityAction::new(label, Some(click)));
        if self.enabled.get() {
            apply_focusable_semantics(&self.focusable, config);
        } else {
            config.disabled();
        }
        self.apply_additional_semantics(config);
    }

    fn apply_additional_semantics(&self, config: &mut SemanticsConfiguration) {
        match &self.kind {
            Kind::Combined(state) => {
                if state.on_long_click.borrow().is_some() {
                    let me = self.me.clone();
                    let long_click: Rc<ActionFn> = Rc::new(move || {
                        let f = me.upgrade().and_then(|core| match &core.kind {
                            Kind::Combined(state) => state.on_long_click.borrow().clone(),
                            Kind::Click(_) => None,
                        });
                        if let Some(f) = f {
                            f();
                        }
                        true
                    });
                    let label = state.on_long_click_label.borrow().clone();
                    config.set(SemanticsActions::OnLongClick, AccessibilityAction::new(label, Some(long_click)));
                }
            }
            Kind::Click(state) => match &*state.extra.borrow() {
                Extra::None => {}
                Extra::Selected(selected) => config.set_selected(*selected),
                Extra::Toggle { value, .. } => {
                    config.set_toggleable_state(ToggleableState::of(*value));
                    apply_fillable_toggle(config, Some(*value));
                }
                Extra::TriState(state) => {
                    config.set_toggleable_state(*state);
                    // (an indeterminate state fills no value)
                    apply_fillable_toggle(config, Some(*state != ToggleableState::Indeterminate));
                }
            },
        }
    }
}

/// A toggle's autofill semantics (Compose's toggleable nodes'): its data type and value, and the fill action
/// (Compose's sets the state on the configuration it captured, which tells nothing: here it says whether the data
/// was a boolean).
fn apply_fillable_toggle(config: &mut SemanticsConfiguration, value: Option<bool>) {
    config.set_content_data_type(ContentDataType::Toggle);
    if let Some(v) = value {
        config.set_fillable_data(FillableData::Boolean(v));
    }
    let fill: Rc<dyn Fn(FillableData) -> bool> = Rc::new(|data: FillableData| data.boolean_value().is_some());
    config.set(SemanticsActions::OnFillData, AccessibilityAction::new(None, Some(fill)));
}

/// Whether a change of `event` other than the down was consumed (Compose's `it.isConsumed && it != downEvent`).
fn consumed_by_another(event: &PointerEvent, down: &PointerInputChange) -> bool {
    event.changes.iter().any(|c| c.is_consumed() && !c.shares_consumption_with(down))
}

fn unsupported_indication_exception_message() -> String {
    "clickable only supports IndicationNodeFactory instances provided to LocalIndication, but Indication was \
     provided instead. Either migrate the Indication implementation to implement IndicationNodeFactory, or use the \
     other clickable overload that takes an Indication parameter, and explicitly pass LocalIndication.current there."
        .to_string()
}

impl GestureConnection for ClickableCore {
    fn gesture_state(&self) -> GestureState {
        self.gesture_state.get()
    }
}

impl ModifierNode for ClickableNode {
    node_capabilities!(pointer_input, key_input, semantics, observer);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        self.core.on_attach(cx);
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        self.core.on_detach(cx);
    }

    fn on_reset(&mut self) {
        self.core.reset_key_press_state();
    }
}

impl PointerInputModifierNode for ClickableNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.core.on_pointer_event(event, pass, bounds);
    }

    fn on_cancel_pointer_input(&mut self) {
        self.core.on_cancel_pointer_input();
    }
}

impl KeyInputModifierNode for ClickableNode {
    fn on_key_event(&mut self, event: &KeyEvent) -> bool {
        self.core.on_key_event(event)
    }

    fn on_pre_key_event(&mut self, _event: &KeyEvent) -> bool {
        false
    }
}

impl SemanticsModifierNode for ClickableNode {
    fn should_merge_descendant_semantics(&self) -> bool {
        true
    }

    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        self.core.apply_semantics(config);
    }
}

impl ObserverModifierNode for ClickableNode {
    fn on_observed_reads_changed(&mut self) {
        self.core.on_observed_reads_changed();
    }
}
