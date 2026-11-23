//! The state a style is applied in (Compose's StyleState.kt): pressed, hovered, focused, selected, enabled, a
//! toggle's state, and custom states known by their [`StyleStateKey`]; a [`MutableStyleState`] a component updates,
//! from its own parameters or from the interactions of an [`InteractionSource`].

use crate::interaction::{FocusInteraction, HoverInteraction, Interaction, InteractionSource, PressInteraction};
use kompoz_runtime::{
    MutableState, SnapshotStateMap, kompozable, mutable_confined_state_map_of, mutable_state_of, remember_keyed,
    without_read_observation,
};
use kompoz_ui::state::ToggleableState;
use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PressedStateMask: i32 = 1 << 0;
const HoveredStateMask: i32 = 1 << 1;
const FocusedStateMask: i32 = 1 << 2;
const SelectedStateMask: i32 = 1 << 3;
const EnabledStateMask: i32 = 1 << 4;

const ToggleStateShift: i32 = 5;
const ToggleStateOff: i32 = 0 << ToggleStateShift;
const ToggleStateOn: i32 = 1 << ToggleStateShift;
const ToggleStateIndeterminate: i32 = 2 << ToggleStateShift;
const ToggleStateMask: i32 = 3 << ToggleStateShift;

/// What a custom state key does with an interaction (Compose's overridable `StyleStateKey.processInteraction`):
/// it updates the state as the interaction implies, suspending if it must.
pub type StyleStateInteractionProcessor =
    Rc<dyn Fn(Arc<dyn Interaction>, MutableStyleState) -> Pin<Box<dyn Future<Output = ()>>>>;

/// The key of a value of a [`StyleState`] (Compose's `open class StyleStateKey<T>(defaultValue)`): the predefined
/// states ([`Pressed`](StyleStateKey::Pressed), [`Hovered`](StyleStateKey::Hovered),
/// [`Focused`](StyleStateKey::Focused), [`Selected`](StyleStateKey::Selected), [`Enabled`](StyleStateKey::Enabled),
/// [`Toggle`](StyleStateKey::Toggle)), or a custom one ([`StyleStateKey::new`]) a component sets and a style reads
/// (`state.get(&PlayingKey)`, a block in it: [`state_of`](super::StyleStateScopeExt::state_of)). A custom key is
/// known by its identity (its clones are it); one whose value an interaction implies processes the interactions a
/// state observes ([`with_interaction_processor`](StyleStateKey::with_interaction_processor): Compose's
/// `processInteraction` override).
pub struct StyleStateKey<T: 'static> {
    kind: KeyKind<T>,
}

enum KeyKind<T: 'static> {
    /// A predefined boolean state (Compose's BooleanPredefinedKey): its bit of the state's flags.
    Flag {
        mask: i32,
        default: T,
    },
    /// The predefined toggle state (Compose's PredefinedToggleStateKey).
    Toggle {
        default: T,
    },
    Custom(Rc<CustomKey<T>>),
}

struct CustomKey<T> {
    id: u64,
    default: T,
    processor: Option<StyleStateInteractionProcessor>,
}

impl<T: Clone> Clone for StyleStateKey<T> {
    fn clone(&self) -> Self {
        StyleStateKey {
            kind: match &self.kind {
                KeyKind::Flag { mask, default } => KeyKind::Flag { mask: *mask, default: default.clone() },
                KeyKind::Toggle { default } => KeyKind::Toggle { default: default.clone() },
                KeyKind::Custom(c) => KeyKind::Custom(c.clone()),
            },
        }
    }
}

/// The same key: the same predefined one, or the same custom one.
impl<T> PartialEq for StyleStateKey<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (KeyKind::Flag { mask: a, .. }, KeyKind::Flag { mask: b, .. }) => a == b,
            (KeyKind::Toggle { .. }, KeyKind::Toggle { .. }) => true,
            (KeyKind::Custom(a), KeyKind::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl<T> fmt::Debug for StyleStateKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            KeyKind::Flag { mask, .. } => write!(f, "StyleStateKey(predefined {mask:#x})"),
            KeyKind::Toggle { .. } => write!(f, "StyleStateKey(Toggle)"),
            KeyKind::Custom(c) => write!(f, "StyleStateKey(#{})", c.id),
        }
    }
}

static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

impl<T: Clone + PartialEq + 'static> StyleStateKey<T> {
    /// A custom state key whose value is `default_value` where a state doesn't set it (Compose's
    /// `StyleStateKey(defaultValue)`).
    ///
    /// Kotlin: `StyleStateKey(defaultValue: T)`
    pub fn new(default_value: T) -> StyleStateKey<T> {
        StyleStateKey {
            kind: KeyKind::Custom(Rc::new(CustomKey {
                id: NEXT_KEY.fetch_add(1, Ordering::Relaxed),
                default: default_value,
                processor: None,
            })),
        }
    }

    /// A custom key that updates a state from the interactions it observes, while the state holds the key's value
    /// (Compose's `processInteraction` override of a StyleStateKey subclass).
    pub fn with_interaction_processor(
        default_value: T,
        processor: impl Fn(Arc<dyn Interaction>, MutableStyleState) -> Pin<Box<dyn Future<Output = ()>>> + 'static,
    ) -> StyleStateKey<T> {
        StyleStateKey {
            kind: KeyKind::Custom(Rc::new(CustomKey {
                id: NEXT_KEY.fetch_add(1, Ordering::Relaxed),
                default: default_value,
                processor: Some(Rc::new(processor)),
            })),
        }
    }

    /// Its value where a state doesn't set it (Compose's `defaultValue`).
    pub fn default_value(&self) -> T {
        match &self.kind {
            KeyKind::Flag { default, .. } | KeyKind::Toggle { default } => default.clone(),
            KeyKind::Custom(c) => c.default.clone(),
        }
    }

    fn is_predefined(&self) -> bool {
        !matches!(self.kind, KeyKind::Custom(_))
    }
}

impl StyleStateKey<bool> {
    /// Whether the component is pressed (Compose's `StyleStateKey.Pressed`): what `clickable`'s press interactions
    /// set.
    pub const Pressed: StyleStateKey<bool> =
        StyleStateKey { kind: KeyKind::Flag { mask: PressedStateMask, default: false } };
    /// Whether it is hovered (Compose's `StyleStateKey.Hovered`): `hoverable`'s interactions.
    pub const Hovered: StyleStateKey<bool> =
        StyleStateKey { kind: KeyKind::Flag { mask: HoveredStateMask, default: false } };
    /// Whether it is focused (Compose's `StyleStateKey.Focused`): `focusable`'s interactions.
    pub const Focused: StyleStateKey<bool> =
        StyleStateKey { kind: KeyKind::Flag { mask: FocusedStateMask, default: false } };
    /// Whether it is selected (Compose's `StyleStateKey.Selected`).
    pub const Selected: StyleStateKey<bool> =
        StyleStateKey { kind: KeyKind::Flag { mask: SelectedStateMask, default: false } };
    /// Whether it is enabled (Compose's `StyleStateKey.Enabled`): true until a component says otherwise.
    pub const Enabled: StyleStateKey<bool> =
        StyleStateKey { kind: KeyKind::Flag { mask: EnabledStateMask, default: true } };
}

impl StyleStateKey<ToggleableState> {
    /// Its toggle's state (Compose's `StyleStateKey.Toggle`).
    pub const Toggle: StyleStateKey<ToggleableState> =
        StyleStateKey { kind: KeyKind::Toggle { default: ToggleableState::Off } };
}

/// A custom key as a state keeps it, its type out of the way: what processes the interactions.
trait AnyStyleStateKey {
    fn process(
        &self,
        interaction: Arc<dyn Interaction>,
        state: MutableStyleState,
    ) -> Option<Pin<Box<dyn Future<Output = ()>>>>;
}

impl<T> AnyStyleStateKey for CustomKey<T> {
    fn process(
        &self,
        interaction: Arc<dyn Interaction>,
        state: MutableStyleState,
    ) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
        self.processor.as_ref().map(|p| p(interaction, state))
    }
}

/// The value of a custom state as the snapshot map holds it.
type CustomValue = Rc<dyn Any>;

/// The state of a component a style reads (Compose's sealed StyleState): whether it is pressed, hovered, focused,
/// selected, enabled, checked, its tri-state toggle, and custom states by key. What the component's
/// [`MutableStyleState`] holds; each read is a state read (a style reading it is resolved again when it changes).
/// Clones are the same state; two are equal when they are.
#[derive(Clone)]
pub struct StyleState {
    inner: Rc<StyleStateInner>,
}

struct StyleStateInner {
    interaction_source: Option<InteractionSource>,
    /// The custom states' values, by key (Compose's `customStates`).
    custom_states: SnapshotStateMap<u64, CustomValue>,
    /// The custom keys set, by id.
    custom_keys: RefCell<BTreeMap<u64, Rc<dyn AnyStyleStateKey>>>,
    /// The predefined states' bits (Compose's MutableStateFlagSet: written without being read).
    predefined: MutableState<i32>,
}

impl PartialEq for StyleState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for StyleState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let flags = self.inner.predefined.peek();
        write!(f, "StyleState(flags={flags:#x})")
    }
}

impl StyleState {
    fn flags(&self) -> i32 {
        self.inner.predefined.get()
    }

    fn flag(&self, mask: i32) -> bool {
        self.flags() & mask != 0
    }

    /// Whether the component is enabled (Compose's `isEnabled`): what [`disabled`](super::StyleStateScopeExt::disabled)
    /// reads.
    pub fn is_enabled(&self) -> bool {
        self.flag(EnabledStateMask)
    }
    /// Whether it is focused (Compose's `isFocused`).
    pub fn is_focused(&self) -> bool {
        self.flag(FocusedStateMask)
    }
    /// Whether it is hovered (Compose's `isHovered`).
    pub fn is_hovered(&self) -> bool {
        self.flag(HoveredStateMask)
    }
    /// Whether it is pressed (Compose's `isPressed`).
    pub fn is_pressed(&self) -> bool {
        self.flag(PressedStateMask)
    }
    /// Whether it is selected (Compose's `isSelected`).
    pub fn is_selected(&self) -> bool {
        self.flag(SelectedStateMask)
    }
    /// Whether it is checked: its toggle on (Compose's `isChecked`).
    pub fn is_checked(&self) -> bool {
        self.tri_state_toggle() == ToggleableState::On
    }
    /// Its tri-state toggle (Compose's `triStateToggle`).
    pub fn tri_state_toggle(&self) -> ToggleableState {
        match self.flags() & ToggleStateMask {
            ToggleStateOn => ToggleableState::On,
            ToggleStateOff => ToggleableState::Off,
            _ => ToggleableState::Indeterminate,
        }
    }

    /// The value of `key`'s state (Compose's `state[key]`): a custom one's default where it isn't set.
    ///
    /// Kotlin: `get(key: StyleStateKey<T>): T`
    pub fn get<T: Clone + PartialEq + 'static>(&self, key: &StyleStateKey<T>) -> T {
        match &key.kind {
            KeyKind::Flag { mask, .. } => as_t(self.flag(*mask)),
            KeyKind::Toggle { .. } => as_t(self.tri_state_toggle()),
            KeyKind::Custom(c) => match self.inner.custom_states.get(&c.id) {
                Some(v) => v.downcast_ref::<T>().cloned().expect("kompoz: a style state's value of another type"),
                None => c.default.clone(),
            },
        }
    }

    /// The interaction source whose interactions update it.
    pub(crate) fn interaction_source(&self) -> Option<InteractionSource> {
        self.inner.interaction_source.clone()
    }

    fn update_flags(&self, mask: i32, values: i32) {
        let current = self.inner.predefined.peek();
        let new_value = (current & !mask) | values;
        if current != new_value {
            self.inner.predefined.set(new_value);
        }
    }

    fn update_flag(&self, mask: i32, value: bool) {
        self.update_flags(mask, if value { mask } else { 0 });
    }
}

/// A predefined state's value as the key's type (only a `bool` key is a flag, only a toggle key a toggle).
fn as_t<T: Clone + 'static, V: Any>(value: V) -> T {
    (&value as &dyn Any).downcast_ref::<T>().cloned().expect("kompoz: a predefined style state key of another type")
}

/// The state of a component that it updates for its style (Compose's MutableStyleState): its predefined states set
/// from its parameters (`set_is_enabled(enabled)`), custom states by key, and — given an interaction source — the
/// pressed, hovered and focused states from the source's interactions while a `styleable` uses it. It derefs to
/// the [`StyleState`] it is (`StyleState::from` or `.into()` gives it).
#[derive(Clone, PartialEq, Debug)]
pub struct MutableStyleState(StyleState);

impl std::ops::Deref for MutableStyleState {
    type Target = StyleState;
    fn deref(&self) -> &StyleState {
        &self.0
    }
}

impl From<MutableStyleState> for StyleState {
    fn from(state: MutableStyleState) -> StyleState {
        state.0
    }
}

impl From<&MutableStyleState> for StyleState {
    fn from(state: &MutableStyleState) -> StyleState {
        state.0.clone()
    }
}

impl From<MutableStyleState> for Option<StyleState> {
    fn from(state: MutableStyleState) -> Option<StyleState> {
        Some(state.0)
    }
}

impl MutableStyleState {
    /// A state observing `interaction_source`'s interactions, if given one, enabled and in no other state
    /// (Compose's `MutableStyleState(interactionSource)`; remember it, or use
    /// [`remember_updated_style_state`]).
    ///
    /// Kotlin: `MutableStyleState(interactionSource: InteractionSource?)`
    pub fn new(interaction_source: Option<InteractionSource>) -> MutableStyleState {
        MutableStyleState(StyleState {
            inner: Rc::new(StyleStateInner {
                interaction_source,
                custom_states: mutable_confined_state_map_of([]),
                custom_keys: RefCell::new(BTreeMap::new()),
                predefined: mutable_state_of(EnabledStateMask),
            }),
        })
    }

    /// The mutable state a state is (Compose's StyleState is one).
    pub(crate) fn of(state: StyleState) -> MutableStyleState {
        MutableStyleState(state)
    }

    /// The state it is.
    pub fn as_style_state(&self) -> StyleState {
        self.0.clone()
    }

    pub fn set_is_enabled(&self, value: bool) {
        self.0.update_flag(EnabledStateMask, value);
    }
    pub fn set_is_focused(&self, value: bool) {
        self.0.update_flag(FocusedStateMask, value);
    }
    pub fn set_is_hovered(&self, value: bool) {
        self.0.update_flag(HoveredStateMask, value);
    }
    pub fn set_is_pressed(&self, value: bool) {
        self.0.update_flag(PressedStateMask, value);
    }
    pub fn set_is_selected(&self, value: bool) {
        self.0.update_flag(SelectedStateMask, value);
    }
    pub fn set_tri_state_toggle(&self, value: ToggleableState) {
        let bits = match value {
            ToggleableState::On => ToggleStateOn,
            ToggleableState::Off => ToggleStateOff,
            ToggleableState::Indeterminate => ToggleStateIndeterminate,
        };
        self.0.update_flags(ToggleStateMask, bits);
    }
    /// Checked or not: its toggle on or off.
    pub fn set_is_checked(&self, value: bool) {
        self.set_tri_state_toggle(if value { ToggleableState::On } else { ToggleableState::Off });
    }

    /// Sets the value of `key`'s state (Compose's `state[key] = value`).
    ///
    /// Kotlin: `set(key: StyleStateKey<T>, value: T)`
    pub fn set<T: Clone + PartialEq + 'static>(&self, key: &StyleStateKey<T>, value: T) {
        match &key.kind {
            KeyKind::Flag { mask, .. } => {
                let v: bool = (&value as &dyn Any).downcast_ref::<bool>().copied().expect("a boolean state");
                self.0.update_flag(*mask, v);
            }
            KeyKind::Toggle { .. } => {
                let v = (&value as &dyn Any).downcast_ref::<ToggleableState>().copied().expect("a toggle state");
                self.set_tri_state_toggle(v);
            }
            KeyKind::Custom(c) => {
                let id = c.id;
                self.0.inner.custom_keys.borrow_mut().insert(id, c.clone() as Rc<dyn AnyStyleStateKey>);
                // (the same value again writes nothing, as Compose's state map keeps itself)
                let current = without_read_observation(|| self.0.inner.custom_states.get(&id));
                let same = current.is_some_and(|v| v.downcast_ref::<T>() == Some(&value));
                if !same {
                    let value: CustomValue = Rc::new(value);
                    self.0.inner.custom_states.update(|m| {
                        m.insert(id, value);
                    });
                }
            }
        }
    }

    /// Takes `key`'s state out: a style reads its default again, an interaction no longer updates it (Compose's
    /// `remove`). Panics for a predefined key, as Compose throws.
    #[track_caller]
    pub fn remove<T: Clone + PartialEq + 'static>(&self, key: &StyleStateKey<T>) {
        assert!(!key.is_predefined(), "Cannot remove an internal StyleStateKey");
        if let KeyKind::Custom(c) = &key.kind {
            self.0.inner.custom_states.remove(&c.id);
            self.0.inner.custom_keys.borrow_mut().remove(&c.id);
        }
    }

    /// Follows `interactions`' interactions until cancelled (Compose's `processInteractions`): presses, hovers and
    /// focuses set its states while any of them lasts; any other interaction goes to the custom keys it holds.
    pub(crate) fn process_interactions(&self, interactions: InteractionSource) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let mut pressed: Vec<PressInteraction::Press> = Vec::new();
            let mut hovered: Vec<HoverInteraction::Enter> = Vec::new();
            let mut focused: Vec<FocusInteraction::Focus> = Vec::new();
            state.set_is_pressed(false);
            state.set_is_hovered(false);
            state.set_is_focused(false);
            let s = state.clone();
            interactions
                .interactions()
                .collect_async(move |interaction| {
                    let state = s.clone();
                    let mut pending: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();
                    if let Some(press) = interaction.downcast_ref::<PressInteraction::Press>() {
                        add(&mut pressed, *press);
                        state.set_is_pressed(true);
                    } else if let Some(release) = interaction.downcast_ref::<PressInteraction::Release>() {
                        pressed.retain(|p| *p != release.press);
                        state.set_is_pressed(!pressed.is_empty());
                    } else if let Some(cancel) = interaction.downcast_ref::<PressInteraction::Cancel>() {
                        pressed.retain(|p| *p != cancel.press);
                        state.set_is_pressed(!pressed.is_empty());
                    } else if let Some(enter) = interaction.downcast_ref::<HoverInteraction::Enter>() {
                        add(&mut hovered, *enter);
                        state.set_is_hovered(true);
                    } else if let Some(exit) = interaction.downcast_ref::<HoverInteraction::Exit>() {
                        hovered.retain(|h| *h != exit.enter);
                        state.set_is_hovered(!hovered.is_empty());
                    } else if let Some(focus) = interaction.downcast_ref::<FocusInteraction::Focus>() {
                        add(&mut focused, *focus);
                        state.set_is_focused(true);
                    } else if let Some(unfocus) = interaction.downcast_ref::<FocusInteraction::Unfocus>() {
                        focused.retain(|f| *f != unfocus.focus);
                        state.set_is_focused(!focused.is_empty());
                    } else {
                        let ids: Vec<u64> = state.0.inner.custom_states.with(|m| m.keys().copied().collect());
                        let keys: Vec<Rc<dyn AnyStyleStateKey>> = {
                            let known = state.0.inner.custom_keys.borrow();
                            ids.iter().filter_map(|id| known.get(id).cloned()).collect()
                        };
                        for key in keys {
                            if let Some(f) = key.process(interaction.clone(), state.clone()) {
                                pending.push(f);
                            }
                        }
                    }
                    async move {
                        for f in pending {
                            f.await;
                        }
                    }
                })
                .await;
        }
    }
}

/// Compose's InteractionSet `add`: once.
fn add<T: PartialEq>(set: &mut Vec<T>, interaction: T) {
    if !set.contains(&interaction) {
        set.push(interaction);
    }
}

/// A [`MutableStyleState`] for a `styleable`, remembered while `interaction_source` is the same and brought up to
/// date by `block` each composition (Compose's inline `rememberUpdatedStyleState`): `block` sets the states the
/// component's parameters say (`|s| s.set_is_enabled(enabled)`).
#[kompozable]
pub fn remember_updated_style_state(
    interaction_source: Option<InteractionSource>,
    #[default(|_: &MutableStyleState| {})] block: impl FnOnce(&MutableStyleState),
) -> StyleState {
    let key = interaction_source.clone();
    let state = remember_keyed(key, move || MutableStyleState::new(interaction_source));
    block(&state);
    state.into()
}
