//! A shared element's state machine, as Compose's internal `SharedTransitionStateMachine.kt`: whether a match is
//! found and where its animation goes. The states:
//!
//! NoMatchFound → ActiveMatchFoundConfigPending → ActiveMatchConfigured (its animations set up, ready to run) →
//! ActiveMatchRemovedDuringTransition (the visible entry removed during the transition);
//!
//! and back: ActiveMatchFoundConfigPending → NoMatchFound or → ActiveMatchRemovedDuringTransition,
//! ActiveMatchConfigured → NoMatchFound, ActiveMatchRemovedDuringTransition → NoMatchFound.

use crate::shared_element::{BoundsProvider, SharedElement, same_provider};
use crate::shared_transition::{Observed, Same};
use kompoz_graphics::{Offset, Rect, Size};
use kompoz_runtime::{MutableState, mutable_state_of};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What the shared element's state machine was asked to do, deferred (Compose's internal StateChangeRequest).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StateChangeRequest {
    NoRequest,
    MatchFound,
    VisibleContentAbsentDuringTransition,
    #[expect(dead_code, reason = "Compose 1.12.1 has it and requests it nowhere")]
    NoMatchFound,
}

/// Where the target is, finely (Compose's internal TargetData): its size, and its offset as a structural offset
/// plus a motion frame of reference offset (structural + MFR = the target's offset) — so structural changes are
/// animated while what the motion frame of reference adds (scrolling) is applied as it is. Its fields are states.
pub(crate) struct TargetData {
    size: MutableState<Size>,
    /// The motion frame of reference offset when the target changed, from the lookahead placement.
    initial_mfr_offset: MutableState<Offset>,
    /// The target's offset less its motion frame of reference offset, from the lookahead placement.
    target_structural_offset: MutableState<Offset>,
    /// The motion frame of reference offset now, from the lookahead placement (taken as the approach's too).
    current_mfr_offset: MutableState<Offset>,
}

impl TargetData {
    pub(crate) fn new(size: Size, initial_mfr_offset: Offset, target_structural_offset: Offset) -> Rc<TargetData> {
        Rc::new(TargetData {
            size: mutable_state_of(size),
            initial_mfr_offset: mutable_state_of(initial_mfr_offset),
            target_structural_offset: mutable_state_of(target_structural_offset),
            current_mfr_offset: mutable_state_of(initial_mfr_offset),
        })
    }

    pub(crate) fn size(&self) -> Size {
        self.size.get()
    }

    /// The bounds it animates to (Compose's `targetBounds`): the structural offset with the initial motion frame of
    /// reference offset, so changes of the latter aren't animated.
    pub(crate) fn target_bounds(&self) -> Rect {
        Rect::from_offset_size(self.initial_mfr_offset.get() + self.target_structural_offset.get(), self.size.get())
    }

    /// Where animated bounds' top left is with what the motion frame of reference moved since the animation started
    /// (Compose's `calculateOffsetFromDirectManipulation`).
    pub(crate) fn calculate_offset_from_direct_manipulation(&self, animated_bounds: Rect) -> Offset {
        animated_bounds.top_left() - self.initial_mfr_offset.get() + self.current_mfr_offset.get()
    }
}

/// Updates the target from a lookahead placement (Compose's private `updateTargetData`): the bounds only when the
/// structural offset or the size changed (or the target's provider did), so no animation starts only to be
/// interrupted later in the same frame.
fn update_target_data(
    target_data: &TargetData,
    lookahead_size: Size,
    top_left: Offset,
    structural_offset: Offset,
    target_bounds_provider_changed: bool,
) {
    if target_data.target_structural_offset.peek() != structural_offset
        || target_data.size.peek() != lookahead_size
        || target_bounds_provider_changed
    {
        target_data.size.set(lookahead_size);
        target_data.target_structural_offset.set(structural_offset);
        if target_bounds_provider_changed {
            let current = target_data.current_mfr_offset.peek();
            let initial = target_data.initial_mfr_offset.peek();
            target_data.initial_mfr_offset.set((top_left - structural_offset) - (current - initial));
        }
    }
    target_data.current_mfr_offset.set(top_left - structural_offset);
}

/// Compose's ActiveMatchFoundConfigPending: a match found, its animation not yet set up.
pub(crate) struct ActiveMatchFoundConfigPending {
    target_bounds_provider_before_config: RefCell<Option<Rc<dyn BoundsProvider>>>,
    target_data: Option<Rc<TargetData>>,
    /// The visible bounds of the shared element now (reset when no match is found, not when the transition ends).
    current_bounds: MutableState<Option<Rect>>,
}

/// Compose's ActiveMatchConfigured: its animation set up, ready to run.
pub(crate) struct ActiveMatchConfigured {
    target_data: Rc<TargetData>,
    target_bounds_provider: Observed<Rc<dyn BoundsProvider>>,
    current_bounds: MutableState<Rect>,
}

/// Compose's ActiveMatchRemovedDuringTransition: the visible entry removed while the transition ran.
pub(crate) struct ActiveMatchRemovedDuringTransition {
    target_data: Rc<TargetData>,
    current_bounds: MutableState<Rect>,
}

/// The state of a shared element's match (Compose's sealed `SharedTransitionStateMachine.State`).
#[derive(Clone)]
pub(crate) enum MatchState {
    NoMatchFound,
    ActiveMatchFoundConfigPending(Rc<ActiveMatchFoundConfigPending>),
    ActiveMatchConfigured(Rc<ActiveMatchConfigured>),
    ActiveMatchRemovedDuringTransition(Rc<ActiveMatchRemovedDuringTransition>),
}

impl Same for MatchState {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (MatchState::NoMatchFound, MatchState::NoMatchFound) => true,
            (MatchState::ActiveMatchFoundConfigPending(a), MatchState::ActiveMatchFoundConfigPending(b)) => {
                Rc::ptr_eq(a, b)
            }
            (MatchState::ActiveMatchConfigured(a), MatchState::ActiveMatchConfigured(b)) => Rc::ptr_eq(a, b),
            (MatchState::ActiveMatchRemovedDuringTransition(a), MatchState::ActiveMatchRemovedDuringTransition(b)) => {
                Rc::ptr_eq(a, b)
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for MatchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MatchState::NoMatchFound => "NoMatchFound",
            MatchState::ActiveMatchFoundConfigPending(_) => "ActiveMatchFoundConfigPending",
            MatchState::ActiveMatchConfigured(_) => "ActiveMatchConfigured",
            MatchState::ActiveMatchRemovedDuringTransition(_) => "ActiveMatchRemovedDuringTransition",
        })
    }
}

impl MatchState {
    fn config_pending(
        target_bounds_provider_before_config: Option<Rc<dyn BoundsProvider>>,
        target_data: Option<Rc<TargetData>>,
        current_bounds: Option<Rect>,
    ) -> MatchState {
        MatchState::ActiveMatchFoundConfigPending(Rc::new(ActiveMatchFoundConfigPending {
            target_bounds_provider_before_config: RefCell::new(target_bounds_provider_before_config),
            target_data,
            current_bounds: mutable_state_of(current_bounds),
        }))
    }

    /// Where its animation goes (Compose's `targetData`): set once a match is configured.
    pub(crate) fn target_data(&self) -> Option<Rc<TargetData>> {
        match self {
            MatchState::NoMatchFound => None,
            MatchState::ActiveMatchFoundConfigPending(s) => s.target_data.clone(),
            MatchState::ActiveMatchConfigured(s) => Some(s.target_data.clone()),
            MatchState::ActiveMatchRemovedDuringTransition(s) => Some(s.target_data.clone()),
        }
    }

    /// The shared element's visible bounds now (Compose's `currentBounds`; a state read).
    pub(crate) fn current_bounds(&self) -> Option<Rect> {
        match self {
            MatchState::NoMatchFound => None,
            MatchState::ActiveMatchFoundConfigPending(s) => s.current_bounds.get(),
            MatchState::ActiveMatchConfigured(s) => Some(s.current_bounds.get()),
            MatchState::ActiveMatchRemovedDuringTransition(s) => Some(s.current_bounds.get()),
        }
    }

    /// Whether a match is active (Compose's `activeMatchFound`).
    pub(crate) fn active_match_found(&self) -> bool {
        matches!(self, MatchState::ActiveMatchFoundConfigPending(_) | MatchState::ActiveMatchConfigured(_))
    }

    /// Whether a match is or has been configured (Compose's `matchIsOrHasBeenConfigured`).
    pub(crate) fn match_is_or_has_been_configured(&self) -> bool {
        matches!(self, MatchState::ActiveMatchConfigured(_) | MatchState::ActiveMatchRemovedDuringTransition(_))
    }

    pub(crate) fn is_no_match_found(&self) -> bool {
        matches!(self, MatchState::NoMatchFound)
    }

    /// Takes the visible bounds (Compose's `updateBounds`).
    pub(crate) fn update_bounds(&self, bounds: Rect) {
        match self {
            MatchState::NoMatchFound => {}
            MatchState::ActiveMatchFoundConfigPending(s) => s.current_bounds.set(Some(bounds)),
            MatchState::ActiveMatchConfigured(s) => s.current_bounds.set(bounds),
            MatchState::ActiveMatchRemovedDuringTransition(s) => s.current_bounds.set(bounds),
        }
    }

    /// The state a found match leads to (Compose's `onMatchFound`).
    fn on_match_found(&self, previous_target_bounds_provider: Option<Rc<dyn BoundsProvider>>) -> MatchState {
        match self {
            MatchState::NoMatchFound => MatchState::config_pending(previous_target_bounds_provider, None, None),
            MatchState::ActiveMatchFoundConfigPending(s) => {
                let mut before = s.target_bounds_provider_before_config.borrow_mut();
                if before.is_none() {
                    *before = previous_target_bounds_provider;
                }
                self.clone()
            }
            MatchState::ActiveMatchConfigured(_) => self.clone(),
            MatchState::ActiveMatchRemovedDuringTransition(s) => MatchState::config_pending(
                previous_target_bounds_provider,
                Some(s.target_data.clone()),
                Some(s.current_bounds.get()),
            ),
        }
    }

    /// The state the visible content's removal during the transition leads to (Compose's
    /// `onVisibleContentRemovedDuringTransition`): an alternative target the configuration gives, or no match.
    fn on_visible_content_removed_during_transition(&self) -> MatchState {
        match self {
            MatchState::NoMatchFound | MatchState::ActiveMatchRemovedDuringTransition(_) => self.clone(),
            MatchState::ActiveMatchFoundConfigPending(_) => MatchState::NoMatchFound,
            MatchState::ActiveMatchConfigured(s) => {
                let target_data = &s.target_data;
                let current_mfr = target_data.current_mfr_offset.get();
                let last_target = Rect::from_offset_size(
                    current_mfr + target_data.target_structural_offset.get(),
                    target_data.size.get(),
                );
                match s.target_bounds_provider.get().calculate_alternative_target_bounds(last_target) {
                    // (no alternative target once the target is lost: the animations end)
                    None => MatchState::NoMatchFound,
                    Some(new_target) => {
                        let updated = TargetData::new(
                            new_target.size(),
                            target_data.initial_mfr_offset.get(),
                            new_target.top_left() - current_mfr,
                        );
                        updated.current_mfr_offset.set(current_mfr);
                        MatchState::ActiveMatchRemovedDuringTransition(Rc::new(ActiveMatchRemovedDuringTransition {
                            target_data: updated,
                            current_bounds: mutable_state_of(s.current_bounds.get()),
                        }))
                    }
                }
            }
        }
    }

    /// The bounds it starts from, found where they can be (Compose's `initializeCurrentBounds`): a pending match's
    /// from its last target's last bounds.
    fn initialize_current_bounds(&self, shared_element: &SharedElement) -> Option<Rect> {
        let MatchState::ActiveMatchFoundConfigPending(s) = self else { return self.current_bounds() };
        if let Some(bounds) = s.current_bounds.get() {
            return Some(bounds);
        }
        let last_target =
            s.target_bounds_provider_before_config.borrow().clone().or_else(|| first_enabled_provider(shared_element));
        if let Some(bounds) = shared_element.obtain_bounds_from_last_target(last_target.as_ref()) {
            s.current_bounds.set(Some(bounds));
        }
        s.current_bounds.get()
    }

    /// Sets the active match's animation up, or updates its target (Compose's `configureActiveMatch`), from a
    /// lookahead placement of the entry that becomes visible.
    fn configure_active_match(
        &self,
        shared_element: &SharedElement,
        target_bounds_provider: Rc<dyn BoundsProvider>,
        lookahead_size: Size,
        top_left: Offset,
        structural_offset: Offset,
    ) -> MatchState {
        match self {
            MatchState::ActiveMatchFoundConfigPending(s) => {
                // (from here until the transition ends the target data is set)
                let target_data = s.target_data.clone().unwrap_or_else(|| {
                    TargetData::new(lookahead_size, top_left - structural_offset, structural_offset)
                });
                let current_bounds = s
                    .current_bounds
                    .get()
                    .or_else(|| {
                        let last = s
                            .target_bounds_provider_before_config
                            .borrow()
                            .clone()
                            .or_else(|| first_enabled_provider(shared_element));
                        shared_element.obtain_bounds_from_last_target(last.as_ref())
                    })
                    .unwrap_or_else(|| Rect::from_offset_size(top_left, lookahead_size));
                update_target_data(&target_data, lookahead_size, top_left, structural_offset, true);
                MatchState::ActiveMatchConfigured(Rc::new(ActiveMatchConfigured {
                    target_data,
                    target_bounds_provider: Observed::new(target_bounds_provider),
                    current_bounds: mutable_state_of(current_bounds),
                }))
            }
            MatchState::ActiveMatchConfigured(s) => {
                let changed = !Rc::ptr_eq(&s.target_bounds_provider.get(), &target_bounds_provider);
                update_target_data(&s.target_data, lookahead_size, top_left, structural_offset, changed);
                s.target_bounds_provider.set(target_bounds_provider);
                self.clone()
            }
            _ => panic!(
                "Active match can only be configured in ActiveMatchFoundConfigPending or ActiveMatchConfigured \
                 state. Current state: {self:?}"
            ),
        }
    }
}

/// The bounds provider of the first entry of all that is enabled (Compose's `allEntries.fastFirstOrNull {
/// enabledEntries.contains(it) }?.boundsProvider`).
fn first_enabled_provider(shared_element: &SharedElement) -> Option<Rc<dyn BoundsProvider>> {
    let enabled = shared_element.enabled_entries();
    shared_element
        .all_entries()
        .into_iter()
        .find(|e| enabled.iter().any(|x| Rc::ptr_eq(x, e)))
        .and_then(|e| e.bounds_provider())
}

/// Compose's internal SharedTransitionStateMachine: the shared element's [`MatchState`], changed by requests
/// deferred until all compositions are done (two entries can be in separate (sub)compositions: the visible one
/// becoming invisible in one, the other visible in the next — handled at once that would look like no match, then a
/// match, and the animation would pause), and the provider of its target bounds.
pub(crate) struct SharedTransitionStateMachine {
    state: Observed<MatchState>,
    last_handled_request_id: Cell<i32>,
    request_id: MutableState<i32>,
    request_to_be_handled: Cell<StateChangeRequest>,
    target_bounds_provider: RefCell<Option<Rc<dyn BoundsProvider>>>,
    /// A request id rather than a dirty flag: handled, the bookkeeping changes, not a state (no invalidation).
    target_bounds_provider_update_request_id: MutableState<i32>,
    last_handled_target_provider_update_request_id: Cell<i32>,
}

impl SharedTransitionStateMachine {
    pub(crate) fn new() -> SharedTransitionStateMachine {
        SharedTransitionStateMachine {
            state: Observed::new(MatchState::NoMatchFound),
            last_handled_request_id: Cell::new(0),
            request_id: mutable_state_of(0),
            request_to_be_handled: Cell::new(StateChangeRequest::NoRequest),
            target_bounds_provider: RefCell::new(None),
            target_bounds_provider_update_request_id: mutable_state_of(0),
            last_handled_target_provider_update_request_id: Cell::new(0),
        }
    }

    /// Its state (a state read).
    pub(crate) fn state(&self) -> MatchState {
        self.state.get()
    }

    /// Whether a found match waits to be handled (Compose's `activeMatchDeferred`).
    pub(crate) fn active_match_deferred(&self) -> bool {
        self.request_to_be_handled.get() == StateChangeRequest::MatchFound
    }

    /// Back to no match (Compose's `resetState`).
    pub(crate) fn reset_state(&self) {
        self.request_to_be_handled.set(StateChangeRequest::NoRequest);
        self.last_handled_request_id.set(self.request_id.get());
        self.state.set(MatchState::NoMatchFound);
    }

    /// What it holds let go of, its shared element gone from the scope (KompozR's: what Compose's garbage collector
    /// does — its state and its target's provider hold the entries' nodes, which hold it).
    pub(crate) fn release(&self) {
        self.reset_state();
        *self.target_bounds_provider.borrow_mut() = None;
    }

    /// Defers a request to when all compositions are done (Compose's `deferRequest`).
    fn defer_request(&self, request: StateChangeRequest) {
        self.request_to_be_handled.set(request);
        self.request_id.set(self.last_handled_request_id.get() + 1);
    }

    /// Handles the request deferred, if any (Compose's `processPendingRequest`): in measure and in the lookahead
    /// placement, both after the lookahead measure where shared elements may be composed.
    pub(crate) fn process_pending_request(&self, shared_element: &SharedElement) {
        if self.request_id.get() != self.last_handled_request_id.get() {
            self.last_handled_request_id.set(self.request_id.get());
            let state = self.state.get();
            let new_state = match self.request_to_be_handled.get() {
                StateChangeRequest::NoMatchFound => MatchState::NoMatchFound,
                StateChangeRequest::NoRequest => state,
                StateChangeRequest::MatchFound => state.on_match_found(self.target_bounds_provider.borrow().clone()),
                StateChangeRequest::VisibleContentAbsentDuringTransition => {
                    let target = self.target_bounds_provider.borrow().clone();
                    if shared_element.enabled_entries().iter().any(|e| same_provider(&e.bounds_provider(), &target)) {
                        // (the last target is there still, no longer visible)
                        MatchState::NoMatchFound
                    } else {
                        state.on_visible_content_removed_during_transition()
                    }
                }
            };
            self.state.set(new_state);
            self.request_to_be_handled.set(StateChangeRequest::NoRequest);
        }
        self.update_target_bounds_provider(shared_element);
    }

    /// Defers what the entries now say (Compose's `checkForAndDeferStateUpdates`, observed: called again when what
    /// it reads changes).
    pub(crate) fn check_for_and_defer_state_updates(&self, shared_element: &SharedElement, has_visible_content: bool) {
        if shared_element.enabled_entries().len() > 1 && has_visible_content {
            self.defer_request(StateChangeRequest::MatchFound);
        } else if shared_element.scope().is_transition_active() {
            // (with visible content here, there is one enabled entry at most: nothing to do)
            if !has_visible_content {
                self.defer_request(StateChangeRequest::VisibleContentAbsentDuringTransition);
            }
        } else {
            self.reset_state();
        }
        self.invalidate_target_bounds_provider(shared_element);
    }

    /// Asks for the target's provider to be found again before it is used (Compose's
    /// `invalidateTargetBoundsProvider`): an entry added, removed, or with another target state.
    pub(crate) fn invalidate_target_bounds_provider(&self, shared_element: &SharedElement) {
        let target = shared_element.enabled_entries().into_iter().find(|e| e.target());
        let current = self.target_bounds_provider.borrow().clone();
        if target.is_none() && current.is_none() {
            return;
        }
        if same_provider(&target.and_then(|t| t.bounds_provider()), &current) {
            return;
        }
        // (once however often it is asked before it is handled: one invalidation)
        self.target_bounds_provider_update_request_id
            .set(self.last_handled_target_provider_update_request_id.get() + 1);
    }

    /// Finds the target's provider again if asked to (Compose's `updateTargetBoundsProvider`): just before the
    /// lookahead placement and the approach measure need it, once all compositions are done.
    pub(crate) fn update_target_bounds_provider(&self, shared_element: &SharedElement) {
        let requested = self.target_bounds_provider_update_request_id.get();
        if requested != self.last_handled_target_provider_update_request_id.get() {
            let entries = if shared_element.scope().is_transition_active() {
                shared_element.enabled_entries()
            } else {
                shared_element.all_entries()
            };
            let new_provider = entries.into_iter().find(|e| e.target()).and_then(|e| e.bounds_provider());
            if !same_provider(&new_provider, &self.target_bounds_provider.borrow()) {
                *self.target_bounds_provider.borrow_mut() = new_provider;
            }
            self.last_handled_target_provider_update_request_id.set(requested);
        }
    }

    /// The bounds it starts from, after the deferred request (Compose's `tryInitializingCurrentBounds`, in the
    /// approach measure).
    pub(crate) fn try_initializing_current_bounds(&self, shared_element: &SharedElement) -> Option<Rect> {
        self.process_pending_request(shared_element);
        self.state.get().initialize_current_bounds(shared_element)
    }

    /// Sets the active match up from a lookahead placement (Compose's `configureActiveMatch`).
    pub(crate) fn configure_active_match(
        &self,
        shared_element: &SharedElement,
        lookahead_size: Size,
        top_left: Offset,
        structural_offset: Offset,
    ) {
        let provider =
            self.target_bounds_provider.borrow().clone().expect("kompoz: an active match without its target's bounds");
        let state = self.state.get();
        self.state.set(state.configure_active_match(
            shared_element,
            provider,
            lookahead_size,
            top_left,
            structural_offset,
        ));
    }
}
