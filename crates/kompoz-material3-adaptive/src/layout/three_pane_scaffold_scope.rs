//! The scopes of a three-pane scaffold, as Compose's adaptive-layout ThreePaneScaffoldScope.kt (1.2.0):
//! [`ThreePaneScaffoldScope`] (what the drag handle and the panes are given: the pane scaffold's scope, the
//! transition of its values, its lookahead scope, the panes' focus requesters) and [`ThreePaneScaffoldPaneScope`]
//! (a pane's: that and the pane's role, motion and interactability).

use super::pane_expansion_draggable_modifier::pane_expansion_draggable_internal;
use super::pane_expansion_state::PaneExpansionState;
use super::pane_motion::PaneMotion;
use super::pane_scaffold::{
    PaneScaffoldPaneScope, PaneScaffoldScope, PaneScaffoldTransitionScope, SemanticsProperties,
};
use super::pane_scaffold_role::ThreePaneScaffoldRole;
use super::three_pane_motion::ThreePaneScaffoldMotionDataProvider;
use super::three_pane_scaffold_state::ThreePaneScaffoldState;
use super::three_pane_scaffold_value::ThreePaneScaffoldValue;
use kompoz_animation::Transition;
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_graphics::{Dp, Offset};
use kompoz_runtime::{MutableState, SaveableStateHolder, kompozable, mutable_state_of, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::layout::{LayoutCoordinates, LookaheadScope, PlacementScope};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

/// The transition part of a three-pane scaffold's scope (Compose's internal `ThreePaneScaffoldTransitionScopeImpl`):
/// set again at each composition of the scaffold.
pub(crate) struct ThreePaneScaffoldTransitionScopeImpl {
    pub(crate) motion_data_provider: Rc<ThreePaneScaffoldMotionDataProvider>,
    pub(crate) scaffold_state_transition: RefCell<Option<Transition<ThreePaneScaffoldValue>>>,
    pub(crate) transition_state: RefCell<Option<Rc<dyn ThreePaneScaffoldState>>>,
}

impl ThreePaneScaffoldTransitionScopeImpl {
    pub(crate) fn new(
        motion_data_provider: Rc<ThreePaneScaffoldMotionDataProvider>,
    ) -> ThreePaneScaffoldTransitionScopeImpl {
        ThreePaneScaffoldTransitionScopeImpl {
            motion_data_provider,
            scaffold_state_transition: RefCell::new(None),
            transition_state: RefCell::new(None),
        }
    }

    fn motion_progress(&self) -> f32 {
        let state = self.transition_state.borrow().clone().expect("kompoz: a scaffold's scope before its state");
        if state.current_state() == state.target_state() { 1.0 } else { state.progress_fraction() }
    }
}

/// The scope of a three-pane scaffold (Compose's sealed interface `ThreePaneScaffoldScope`): the pane scaffold's
/// scope ([`PaneScaffoldScope`]), the transition of its values ([`PaneScaffoldTransitionScope`]), its lookahead
/// scope (it is one: `LookaheadScope`) and a focus requester for each pane (the scaffold asks for the focus of the
/// current destination's). A cheap handle: clones are the same scope.
#[derive(Clone)]
pub struct ThreePaneScaffoldScope {
    inner: Rc<ScopeImpl>,
}

struct ScopeImpl {
    transition_scope: Rc<ThreePaneScaffoldTransitionScopeImpl>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    saveable_state_holder: SaveableStateHolder,
    focus_requesters: [FocusRequester; 3],
}

/// The same scope.
impl PartialEq for ThreePaneScaffoldScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl ThreePaneScaffoldScope {
    /// Compose's internal `ThreePaneScaffoldScopeImpl(transitionScope, lookaheadScope, saveableStateHolder,
    /// focusRequesters)`.
    pub(crate) fn new(
        transition_scope: Rc<ThreePaneScaffoldTransitionScopeImpl>,
        lookahead_scope: Rc<dyn LookaheadScope>,
        saveable_state_holder: SaveableStateHolder,
    ) -> ThreePaneScaffoldScope {
        ThreePaneScaffoldScope {
            inner: Rc::new(ScopeImpl {
                transition_scope,
                lookahead_scope,
                saveable_state_holder,
                focus_requesters: [FocusRequester::new(), FocusRequester::new(), FocusRequester::new()],
            }),
        }
    }

    /// The focus requester of each pane (Compose's `focusRequesters[role]`).
    pub fn focus_requester(&self, role: ThreePaneScaffoldRole) -> FocusRequester {
        self.inner.focus_requesters[role as usize].clone()
    }

    /// Its lookahead scope (Compose's `LookaheadScope by lookaheadScope`) as one to give a modifier.
    pub fn lookahead_scope(&self) -> Rc<dyn LookaheadScope> {
        self.inner.lookahead_scope.clone()
    }
}

impl PaneScaffoldScope for ThreePaneScaffoldScope {
    fn pane_expansion_draggable(
        &self,
        modifier: Modifier,
        state: PaneExpansionState,
        min_touch_target_size: Dp,
        interaction_source: MutableInteractionSource,
        semantics_properties: Option<SemanticsProperties>,
    ) -> Modifier {
        let transition_scope = self.inner.transition_scope.clone();
        pane_expansion_draggable_internal(
            modifier,
            state,
            min_touch_target_size,
            interaction_source,
            semantics_properties,
            Rc::new(move || transition_scope.motion_progress()),
            self.lookahead_scope(),
        )
    }

    fn saveable_state_holder(&self) -> SaveableStateHolder {
        self.inner.saveable_state_holder.clone()
    }
}

impl PaneScaffoldTransitionScope for ThreePaneScaffoldScope {
    type ScaffoldValue = ThreePaneScaffoldValue;
    type MotionDataProvider = ThreePaneScaffoldMotionDataProvider;

    fn scaffold_state_transition(&self) -> Transition<ThreePaneScaffoldValue> {
        self.inner
            .transition_scope
            .scaffold_state_transition
            .borrow()
            .clone()
            .expect("kompoz: a scaffold's scope before its transition")
    }

    fn motion_progress(&self) -> f32 {
        self.inner.transition_scope.motion_progress()
    }

    fn motion_data_provider(&self) -> Rc<ThreePaneScaffoldMotionDataProvider> {
        self.inner.transition_scope.motion_data_provider.clone()
    }
}

/// Its lookahead scope's (Kotlin's `LookaheadScope by lookaheadScope`).
impl LookaheadScope for ThreePaneScaffoldScope {
    fn to_lookahead_coordinates(&self, coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        self.inner.lookahead_scope.to_lookahead_coordinates(coordinates)
    }

    fn lookahead_scope_coordinates(&self, placement_scope: &PlacementScope) -> LayoutCoordinates {
        self.inner.lookahead_scope.lookahead_scope_coordinates(placement_scope)
    }

    fn local_lookahead_position_of(
        &self,
        coordinates: &LayoutCoordinates,
        source_coordinates: &LayoutCoordinates,
        relative_to_source: Offset,
        include_motion_frame_of_reference: bool,
    ) -> Offset {
        self.inner.lookahead_scope.local_lookahead_position_of(
            coordinates,
            source_coordinates,
            relative_to_source,
            include_motion_frame_of_reference,
        )
    }
}

/// The scope of a three-pane scaffold's pane (Compose's sealed interface `ThreePaneScaffoldPaneScope`): its
/// scaffold's ([`ThreePaneScaffoldScope`], reached through `Deref`) and the pane's role, motion and whether it takes
/// the user's interaction. A cheap handle: clones are the same scope.
#[derive(Clone)]
pub struct ThreePaneScaffoldPaneScope {
    inner: Rc<PaneScopeImpl>,
}

struct PaneScopeImpl {
    pane_role: ThreePaneScaffoldRole,
    scaffold_scope: ThreePaneScaffoldScope,
    pane_motion: MutableState<PaneMotion>,
    is_interactable: MutableState<bool>,
}

/// The same scope.
impl PartialEq for ThreePaneScaffoldPaneScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Deref for ThreePaneScaffoldPaneScope {
    type Target = ThreePaneScaffoldScope;

    fn deref(&self) -> &ThreePaneScaffoldScope {
        &self.inner.scaffold_scope
    }
}

impl ThreePaneScaffoldPaneScope {
    /// Its scaffold's scope.
    pub fn scaffold_scope(&self) -> &ThreePaneScaffoldScope {
        &self.inner.scaffold_scope
    }
}

impl PaneScaffoldPaneScope for ThreePaneScaffoldPaneScope {
    type Role = ThreePaneScaffoldRole;

    fn pane_role(&self) -> ThreePaneScaffoldRole {
        self.inner.pane_role
    }

    fn pane_motion(&self) -> PaneMotion {
        self.inner.pane_motion.get()
    }

    fn is_interactable(&self) -> bool {
        self.inner.is_interactable.get()
    }
}

impl PaneScaffoldScope for ThreePaneScaffoldPaneScope {
    fn pane_expansion_draggable(
        &self,
        modifier: Modifier,
        state: PaneExpansionState,
        min_touch_target_size: Dp,
        interaction_source: MutableInteractionSource,
        semantics_properties: Option<SemanticsProperties>,
    ) -> Modifier {
        self.inner.scaffold_scope.pane_expansion_draggable(
            modifier,
            state,
            min_touch_target_size,
            interaction_source,
            semantics_properties,
        )
    }

    fn saveable_state_holder(&self) -> SaveableStateHolder {
        self.inner.scaffold_scope.saveable_state_holder()
    }
}

impl PaneScaffoldTransitionScope for ThreePaneScaffoldPaneScope {
    type ScaffoldValue = ThreePaneScaffoldValue;
    type MotionDataProvider = ThreePaneScaffoldMotionDataProvider;

    fn scaffold_state_transition(&self) -> Transition<ThreePaneScaffoldValue> {
        self.inner.scaffold_scope.scaffold_state_transition()
    }

    fn motion_progress(&self) -> f32 {
        self.inner.scaffold_scope.motion_progress()
    }

    fn motion_data_provider(&self) -> Rc<ThreePaneScaffoldMotionDataProvider> {
        self.inner.scaffold_scope.motion_data_provider()
    }
}

/// The scope of a pane, remembered with its scaffold's scope, its motion and interactability set now (Compose's
/// internal `rememberThreePaneScaffoldPaneScope`).
#[kompozable]
pub(crate) fn remember_three_pane_scaffold_pane_scope(
    pane_role: ThreePaneScaffoldRole,
    scaffold_scope: ThreePaneScaffoldScope,
    pane_motion: PaneMotion,
    is_interactable: bool,
) -> ThreePaneScaffoldPaneScope {
    let scope = remember_keyed(scaffold_scope.clone(), move || ThreePaneScaffoldPaneScope {
        inner: Rc::new(PaneScopeImpl {
            pane_role,
            scaffold_scope: scaffold_scope.clone(),
            pane_motion: mutable_state_of(PaneMotion::ExitToLeft),
            is_interactable: mutable_state_of(false),
        }),
    });
    scope.inner.pane_motion.set(pane_motion);
    scope.inner.is_interactable.set(is_interactable);
    scope
}
