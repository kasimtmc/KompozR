//! The expansion of two panes side by side, as Compose's adaptive-layout PaneExpansionState.kt (1.2.0): the state a
//! scaffold lays its first of two expanded panes out by — a width, a proportion or a dragged offset, and the anchors
//! a drag settles on ([`PaneExpansionState`], [`remember_pane_expansion_state`], [`PaneExpansionAnchor`]); the keys
//! it is remembered by ([`PaneExpansionStateKey`], [`PaneExpansionStateKeyProvider`]), its data and their savers.

use super::internal::remember_utils::{remember_persistently_with_key, remember_updated_ref};
use super::internal::strings::{Strings, get_string_formatted, get_string_of_node_formatted};
use super::three_pane_scaffold_value::TwoPaneExpansionStateKeyImpl;
use kompoz_animation::{FiniteAnimationSpec, MutatePriority, MutatorMutex, animate, spring};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::gestures::{
    DragBlock, DragScope, DraggableState, FlingBehavior, LocalFuture, ScrollScope, ScrollableDefaults,
};
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{
    MutableState, Saveable, SavedValue, Saver, kompozable, list_saver, mutable_float_state_of, mutable_int_state_of,
    mutable_state_of, named_args, remember, remember_keyed, without_read_observation,
};
use kompoz_ui::modifier::NodeHandle;
use kompoz_ui::util::{pack_ints, unpack_int1, unpack_int2};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- Keys -------------------------------------------------------------------------------------------------------

/// What gives a [`PaneExpansionStateKey`] to remember and find a pane expansion state by (Compose's sealed
/// interface `PaneExpansionStateKeyProvider`): a scaffold's value.
pub trait PaneExpansionStateKeyProvider {
    /// The key of the provider's state.
    fn pane_expansion_state_key(&self) -> PaneExpansionStateKey;
}

/// A key a pane expansion state is remembered and found by (Compose's sealed interface `PaneExpansionStateKey`):
/// the default one (one state shared whatever the scaffold shows) or that of two expanded panes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PaneExpansionStateKey {
    /// The default key (Compose's `PaneExpansionStateKey.Default`, a private object equal only to itself): the same
    /// state whatever the scaffold's value.
    Default,
    /// The key of two expanded panes (Compose's internal `TwoPaneExpansionStateKeyImpl`).
    TwoPane(TwoPaneExpansionStateKeyImpl),
}

// ---- rememberPaneExpansionState ---------------------------------------------------------------------------------

/// A pane expansion state remembered (and saved with the UI's state) for `key` (Compose's
/// `rememberPaneExpansionState(key, …)`): `anchors` its drags settle on, at `anchors[initial_anchored_index]` to start
/// with (-1: none), settling on `anchoring_animation_spec` after a `fling_behavior` fling; `consume_drag_delta` takes
/// each drag's delta first and gives back what is left to drag the split by (the latest given is used). Each key
/// has a state data of its own: the same state reads the data of the key it is given.
///
/// Kotlin: `rememberPaneExpansionState(key: PaneExpansionStateKey = PaneExpansionStateKey.Default, anchors: List<PaneExpansionAnchor> = emptyList(), initialAnchoredIndex: Int = -1, anchoringAnimationSpec: FiniteAnimationSpec<Float> = DefaultAnchoringAnimationSpec, flingBehavior: FlingBehavior = ScrollableDefaults.flingBehavior(), consumeDragDelta: ((Float) -> Float) = PaneExpansionState.noOpConsumeDragDelta): PaneExpansionState`
#[kompozable]
pub fn remember_pane_expansion_state(
    #[default(PaneExpansionStateKey::Default)] key: PaneExpansionStateKey,
    #[default(Vec::new())] anchors: Vec<PaneExpansionAnchor>,
    #[default(-1)] initial_anchored_index: i32,
    #[default(PaneExpansionState::DefaultAnchoringAnimationSpec())] anchoring_animation_spec: Arc<
        dyn FiniteAnimationSpec<f32>,
    >,
    #[default(ScrollableDefaults::fling_behavior())] fling_behavior: Rc<dyn FlingBehavior>,
    #[default(PaneExpansionState::no_op_consume_drag_delta())] consume_drag_delta: Rc<dyn Fn(f32) -> f32>,
) -> PaneExpansionState {
    let initial_anchor = {
        let anchors = anchors.clone();
        remember_keyed((anchors.clone(), initial_anchored_index), move || {
            if initial_anchored_index == -1 { None } else { Some(anchors[initial_anchored_index as usize]) }
        })
    };
    // Note that this will only be updated when the current anchors change, which will serve as a fallback if the
    // current anchor is no longer in the current anchors.
    let initial_anchor_for_current_anchors = remember_keyed(anchors.clone(), move || initial_anchor);
    let data = remember_persistently_with_key(key, PaneExpansionStateKeySaver(), PaneExpansionStateDataSaver(), || {
        named_args!(PaneExpansionStateData(current_anchor = initial_anchor))
    });
    // Create a stable reference to the latest consumeDragDelta
    let consume_drag_delta_ref = remember_updated_ref(consume_drag_delta);
    let expansion_state = {
        let data = data.clone();
        remember(move || PaneExpansionState::new(data, Rc::new(move |delta| (consume_drag_delta_ref.value())(delta))))
    };
    let state = expansion_state.clone();
    let (keys_anchors, keys_spec, keys_fling) =
        (anchors.clone(), anchoring_animation_spec.clone(), fling_behavior.clone());
    LaunchedEffect((key, keys_anchors, keys_spec, keys_fling), async move {
        state
            .restore(data, anchors, anchoring_animation_spec, fling_behavior, initial_anchor_for_current_anchors)
            .await;
    });
    expansion_state
}

/// [`remember_pane_expansion_state`] for the key `key_provider` gives (Compose's `rememberPaneExpansionState(
/// keyProvider, …)`): a scaffold's value.
///
/// Kotlin: `rememberPaneExpansionState(keyProvider: PaneExpansionStateKeyProvider, anchors: List<PaneExpansionAnchor> = emptyList(), initialAnchoredIndex: Int = -1, anchoringAnimationSpec: FiniteAnimationSpec<Float> = DefaultAnchoringAnimationSpec, flingBehavior: FlingBehavior = ScrollableDefaults.flingBehavior(), consumeDragDelta: ((delta: Float) -> Float) = PaneExpansionState.noOpConsumeDragDelta): PaneExpansionState`
#[kompozable]
pub fn remember_pane_expansion_state_of_key_provider(
    key_provider: &dyn PaneExpansionStateKeyProvider,
    #[default(Vec::new())] anchors: Vec<PaneExpansionAnchor>,
    #[default(-1)] initial_anchored_index: i32,
    #[default(PaneExpansionState::DefaultAnchoringAnimationSpec())] anchoring_animation_spec: Arc<
        dyn FiniteAnimationSpec<f32>,
    >,
    #[default(ScrollableDefaults::fling_behavior())] fling_behavior: Rc<dyn FlingBehavior>,
    #[default(PaneExpansionState::no_op_consume_drag_delta())] consume_drag_delta: Rc<dyn Fn(f32) -> f32>,
) -> PaneExpansionState {
    remember_pane_expansion_state(
        key_provider.pane_expansion_state_key(),
        anchors,
        initial_anchored_index,
        anchoring_animation_spec,
        fling_behavior,
        consume_drag_delta,
    )
}

/// The pane expansion state of a scaffold (Compose's internal `rememberDefaultPaneExpansionState`): a remembered
/// [`remember_pane_expansion_state`] for the key `key_provider` gives when `mutable` (the scaffold has a drag
/// handle), else a state of its own that nothing changes (a stub, to avoid the overhead).
#[track_caller]
pub(crate) fn remember_default_pane_expansion_state(
    key_provider: impl FnOnce() -> PaneExpansionStateKey,
    mutable: bool,
) -> PaneExpansionState {
    kompoz_runtime::kompozable_block!({
        if mutable {
            remember_pane_expansion_state(key_provider())
        } else {
            remember(PaneExpansionState::default) // Use a stub impl to avoid performance overhead
        }
    })
}

// ---- PaneExpansionState -----------------------------------------------------------------------------------------

/// The expansion of the first of two expanded panes (Compose's `PaneExpansionState`): by default none (the
/// scaffold's widths), else a width ([`set_first_pane_width`](Self::set_first_pane_width)), a proportion
/// ([`set_first_pane_proportion`](Self::set_first_pane_proportion)) or where a drag of the handle left the split —
/// settling on the nearest of its anchors (or the next one the fling goes toward), animated
/// ([`animate_to`](Self::animate_to)). A cheap handle: its clones are the same state, equal only to themselves.
#[derive(Clone)]
pub struct PaneExpansionState {
    inner: Rc<Inner>,
}

struct Inner {
    // TODO(conradchen): Handle state change during dragging and settling
    data: MutableState<PaneExpansionStateData>,
    consume_drag_delta: Rc<dyn Fn(f32) -> f32>,
    is_dragging: MutableState<bool>,
    is_settling: MutableState<bool>,
    max_expansion_width: MutableState<i32>,
    /// Use this field to store the dragging offset decided by measuring instead of dragging to prevent redundant
    /// re-composition.
    current_measured_dragging_offset: Cell<i32>,
    anchors: MutableState<Vec<PaneExpansionAnchor>>,
    measured_anchor_positions: RefCell<IndexedAnchorPositionList>,
    /// (Kotlin's `lateinit`: set by the first `restore`)
    anchoring_animation_spec: RefCell<Option<Arc<dyn FiniteAnimationSpec<f32>>>>,
    /// (Kotlin's `lateinit`: set by the first `restore`)
    fling_behavior: RefCell<Option<Rc<dyn FlingBehavior>>>,
    measured_density: Cell<Option<Density>>,
    drag_mutex: MutatorMutex,
}

impl PartialEq for PaneExpansionState {
    /// The same state (Kotlin's `==` of a class without `equals`: its identity).
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for PaneExpansionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PaneExpansionState@{:p}", Rc::as_ptr(&self.inner))
    }
}

/// A state of no expansion and no anchors (Compose's internal constructor's defaults: `PaneExpansionState()`).
impl Default for PaneExpansionState {
    fn default() -> Self {
        PaneExpansionState::new(named_args!(PaneExpansionStateData()), PaneExpansionState::no_op_consume_drag_delta())
    }
}

impl PaneExpansionState {
    /// The value of a width or an offset not set (Compose's `Unspecified`).
    pub const Unspecified: i32 = -1;

    /// A fling at least this fast (px/s) settles on the next anchor its way (Compose's private
    /// `AnchoringVelocityThreshold`).
    const AnchoringVelocityThreshold: f32 = 200.0;

    /// The default anchoring animation: a spring of damping 0.8, stiffness 380, within 1 px (Compose's internal
    /// `DefaultAnchoringAnimationSpec`).
    pub(crate) fn DefaultAnchoringAnimationSpec() -> Arc<dyn FiniteAnimationSpec<f32>> {
        Arc::new(named_args!(spring(damping_ratio = 0.8, stiffness = 380.0, visibility_threshold = Some(1.0))))
    }

    /// Takes all of a drag's delta (Compose's internal `noOpConsumeDragDelta`).
    pub(crate) fn no_op_consume_drag_delta() -> Rc<dyn Fn(f32) -> f32> {
        thread_local! {
            static NO_OP: Rc<dyn Fn(f32) -> f32> = Rc::new(|delta| delta);
        }
        NO_OP.with(Rc::clone)
    }

    /// Compose's internal constructor.
    pub(crate) fn new(data: PaneExpansionStateData, consume_drag_delta: Rc<dyn Fn(f32) -> f32>) -> PaneExpansionState {
        PaneExpansionState {
            inner: Rc::new(Inner {
                data: mutable_state_of(data),
                consume_drag_delta,
                is_dragging: mutable_state_of(false),
                is_settling: mutable_state_of(false),
                max_expansion_width: mutable_int_state_of(Unspecified),
                current_measured_dragging_offset: Cell::new(Unspecified),
                anchors: mutable_state_of(Vec::new()),
                measured_anchor_positions: RefCell::new(IndexedAnchorPositionList::with_capacity(0)),
                anchoring_animation_spec: RefCell::new(None),
                fling_behavior: RefCell::new(None),
                measured_density: Cell::new(None),
                drag_mutex: MutatorMutex::new(),
            }),
        }
    }

    fn data(&self) -> PaneExpansionStateData {
        self.inner.data.get()
    }

    /// What takes each drag's delta first (Compose's internal, visible for testing, `consumeDragDelta`).
    #[cfg_attr(not(test), allow(dead_code))] // (Compose's: visible for testing)
    pub(crate) fn consume_drag_delta(&self) -> Rc<dyn Fn(f32) -> f32> {
        self.inner.consume_drag_delta.clone()
    }

    /// The first pane's width set, within the room measured ([`Unspecified`](Self::Unspecified) if either isn't
    /// known; Compose's internal `firstPaneWidth`).
    pub(crate) fn first_pane_width(&self) -> i32 {
        let max_expansion_width = self.max_expansion_width();
        let first_pane_width = self.data().first_pane_width_state.get();
        if max_expansion_width == Unspecified || first_pane_width == Unspecified {
            Unspecified
        } else {
            first_pane_width.clamp(0, max_expansion_width)
        }
    }

    /// The first pane's proportion set (NaN if none; Compose's internal `firstPaneProportion`).
    pub(crate) fn first_pane_proportion(&self) -> f32 {
        self.data().first_pane_proportion_state.get()
    }

    /// Where a drag or a settling animation put the split ([`Unspecified`](Self::Unspecified) if none; Compose's
    /// internal `currentDraggingOffset`).
    pub(crate) fn current_dragging_offset(&self) -> i32 {
        self.data().current_dragging_offset_state.get()
    }

    /// (Compose's private setter of `currentDraggingOffset`: within the room measured, also the measured offset.)
    fn set_current_dragging_offset(&self, value: i32) {
        let coerced_value = value.clamp(0, self.max_expansion_width());
        let data = self.data();
        if coerced_value == data.current_dragging_offset_state.get() {
            return;
        }
        data.current_dragging_offset_state.set(coerced_value);
        self.inner.current_measured_dragging_offset.set(coerced_value);
    }

    /// The anchor the split is at or settling on, if any (Compose's `currentAnchor`).
    pub fn current_anchor(&self) -> Option<PaneExpansionAnchor> {
        self.data().current_anchor_state.get()
    }

    fn set_current_anchor(&self, value: Option<PaneExpansionAnchor>) {
        self.data().current_anchor_state.set(value);
    }

    /// The anchor after the split's position, wrapping to the first (Compose's internal `nextAnchor`): none before
    /// it is measured or without anchors.
    pub(crate) fn next_anchor(&self) -> Option<PaneExpansionAnchor> {
        // maxExpansionWidth will be initialized in onMeasured and is backed by a state. Check it here so the next
        // anchor will be updated when measuring is done.
        let anchors = self.inner.anchors.get();
        if self.max_expansion_width() == Unspecified || anchors.is_empty() {
            return None;
        }
        let current_offset = if self.current_dragging_offset() == Unspecified {
            self.inner.current_measured_dragging_offset.get()
        } else {
            self.current_dragging_offset()
        };
        for (index, position) in self.inner.measured_anchor_positions.borrow().iter() {
            if current_offset < position {
                return Some(anchors[index as usize]);
            }
        }
        Some(anchors[0])
    }

    /// Whether the split is being dragged (Compose's internal `isDragging`).
    pub(crate) fn is_dragging(&self) -> bool {
        self.inner.is_dragging.get()
    }

    /// Whether the split is settling on an anchor (Compose's internal `isSettling`).
    pub(crate) fn is_settling(&self) -> bool {
        self.inner.is_settling.get()
    }

    /// Whether it is being dragged or settling (Compose's internal `isDraggingOrSettling`).
    pub(crate) fn is_dragging_or_settling(&self) -> bool {
        self.is_dragging() || self.is_settling()
    }

    /// The room the split moves in, as last measured ([`Unspecified`](Self::Unspecified) before; Compose's
    /// internal, visible for testing, `maxExpansionWidth`).
    pub(crate) fn max_expansion_width(&self) -> i32 {
        self.inner.max_expansion_width.get()
    }

    /// The split's offset as the last measuring placed it (Compose's internal, visible for testing,
    /// `currentMeasuredDraggingOffset`).
    #[cfg_attr(not(test), allow(dead_code))] // (Compose's: visible for testing)
    pub(crate) fn current_measured_dragging_offset(&self) -> i32 {
        self.inner.current_measured_dragging_offset.get()
    }

    /// The anchors' positions in the room measured, by position (Compose's internal `measuredAnchorPositions`).
    #[cfg_attr(not(test), allow(dead_code))] // (Compose's: visible for testing)
    pub(crate) fn measured_anchor_positions(&self) -> IndexedAnchorPositionList {
        self.inner.measured_anchor_positions.borrow().clone()
    }

    /// What the split is dragged through (Compose's internal `draggableState`): its delta given to the drag
    /// consumer first, the rest moving the split from where it was measured; a drag marks it dragging.
    pub(crate) fn draggable_state(&self) -> Rc<dyn DraggableState> {
        Rc::new(PaneExpansionDraggableState(self.clone()))
    }

    /// Whether nothing is set: no width, no proportion, no dragged offset (Compose's `isUnspecified`).
    pub fn is_unspecified(&self) -> bool {
        self.first_pane_width() == Unspecified
            && self.first_pane_proportion().is_nan()
            && self.current_dragging_offset() == Unspecified
    }

    /// Sets the first pane's width, in px, clearing a proportion, a dragged offset and the anchor (Compose's
    /// `setFirstPaneWidth`).
    pub fn set_first_pane_width(&self, first_pane_width: i32) {
        let data = self.data();
        data.first_pane_proportion_state.set(f32::NAN);
        data.current_dragging_offset_state.set(Unspecified);
        data.first_pane_width_state.set(first_pane_width);
        self.set_current_anchor(None);
    }

    /// Sets the first pane's proportion of the room, from 0 to 1, clearing a width, a dragged offset and the anchor
    /// (Compose's `setFirstPaneProportion`). Panics outside 0..1 (Kotlin's `require`).
    pub fn set_first_pane_proportion(&self, first_pane_proportion: f32) {
        assert!((0.0..=1.0).contains(&first_pane_proportion), "Proportion value needs to be in [0f, 1f]");
        let data = self.data();
        data.first_pane_width_state.set(Unspecified);
        data.current_dragging_offset_state.set(Unspecified);
        data.first_pane_proportion_state.set(first_pane_proportion);
        self.set_current_anchor(None);
    }

    /// Clears the width, the proportion and the dragged offset (Compose's `clear`): the anchor stays.
    pub fn clear(&self) {
        let data = self.data();
        data.first_pane_width_state.set(Unspecified);
        data.first_pane_proportion_state.set(f32::NAN);
        data.current_dragging_offset_state.set(Unspecified);
    }

    /// Takes the data, the anchors, the anchoring animation and the fling of where it is remembered (Compose's
    /// internal suspending `restore`), stopping a drag or a settling: the anchors' positions recalculated if it was
    /// measured, the current anchor `initial_anchor_for_current_anchors` if it isn't among them.
    pub(crate) async fn restore(
        &self,
        data: PaneExpansionStateData,
        anchors: Vec<PaneExpansionAnchor>,
        anchoring_animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
        fling_behavior: Rc<dyn FlingBehavior>,
        initial_anchor_for_current_anchors: Option<PaneExpansionAnchor>,
    ) {
        let this = self.clone();
        let block = async move {
            this.inner.data.set(data);
            this.inner.anchors.set(anchors.clone());
            if let Some(density) = this.inner.measured_density.get() {
                // When maxExpansionWidth is updated, the anchor positions will be recalculated.
                *this.inner.measured_anchor_positions.borrow_mut() =
                    to_positions(&anchors, this.max_expansion_width(), &density);
            }
            let current_anchor = this.current_anchor();
            if !current_anchor.is_some_and(|a| anchors.contains(&a)) {
                this.set_current_anchor(initial_anchor_for_current_anchors);
            }
            *this.inner.anchoring_animation_spec.borrow_mut() = Some(anchoring_animation_spec);
            *this.inner.fling_behavior.borrow_mut() = Some(fling_behavior);
        };
        named_args!(self.inner.drag_mutex.mutate(priority = MutatePriority::PreventUserInput, block = block)).await;
    }

    /// Takes the room measured for the split and the density (Compose's internal `onMeasured`): when either
    /// changed, the anchors' positions in it, and the split at the current anchor's (unless dragged or settling)
    /// or its offset coerced into the room again — none of it read as a measurement's dependency.
    pub(crate) fn on_measured(&self, measured_width: i32, density: &Density) {
        if measured_width == self.max_expansion_width() && self.inner.measured_density.get().as_ref() == Some(density) {
            return;
        }
        self.inner.max_expansion_width.set(measured_width);
        self.inner.measured_density.set(Some(*density));
        without_read_observation(|| {
            *self.inner.measured_anchor_positions.borrow_mut() =
                to_positions(&self.inner.anchors.get(), measured_width, density);
            // Changes will always apply to the ongoing measurement, no need to trigger remeasuring
            let current_anchor = self.current_anchor();
            if !self.is_dragging_or_settling()
                && let Some(current_anchor) = current_anchor
            {
                self.set_current_dragging_offset(current_anchor.position_in(measured_width, density));
            } else if self.current_dragging_offset() != Unspecified {
                // To re-coerce the value
                self.set_current_dragging_offset(self.current_dragging_offset());
            }
        });
    }

    /// Takes the split's offset as measured (Compose's internal `onExpansionOffsetMeasured`).
    pub(crate) fn on_expansion_offset_measured(&self, measured_offset: i32) {
        self.inner.current_measured_dragging_offset.set(measured_offset);
    }

    /// Puts the split at `anchor` at once, if it was measured (Compose's internal `snapToAnchor`).
    #[allow(dead_code)]
    pub(crate) fn snap_to_anchor(&self, anchor: PaneExpansionAnchor) {
        without_read_observation(|| {
            if let Some(density) = self.inner.measured_density.get() {
                self.set_current_dragging_offset(anchor.position_in(self.max_expansion_width(), &density));
            }
        });
    }

    /// After a drag let go at `velocity`: flings, then settles on the anchor nearest to where the fling left the
    /// split (the next one its way, for a fast enough fling), animated; nothing without anchors (Compose's
    /// internal suspending `settleToAnchorIfNeeded`).
    pub(crate) async fn settle_to_anchor_if_needed(&self, velocity: f32) {
        if self.inner.measured_anchor_positions.borrow().is_empty() {
            return;
        }
        let this = self.clone();
        let block = async move {
            this.inner.is_settling.set(true);
            // (Kotlin's `finally`: not settling any more, also when cancelled)
            let _finally = SettlingFlag(this.inner.is_settling.clone());
            let fling = this.inner.fling_behavior.borrow().clone().expect("restored before a drag settles");
            let scope: Rc<dyn ScrollScope> = Rc::new(ExpansionDragScope(this.clone()));
            let left_velocity = fling.perform_fling(scope, velocity).await;
            let anchor_position = this.position_of_the_closest_anchor(
                &this.inner.measured_anchor_positions.borrow(),
                this.inner.current_measured_dragging_offset.get(),
                left_velocity,
            );
            let anchors = this.inner.anchors.peek();
            this.set_current_anchor(Some(anchors[anchor_position.index() as usize]));
            this.animate_to_internal(anchor_position.position(), left_velocity).await;
        };
        named_args!(self.inner.drag_mutex.mutate(priority = MutatePriority::PreventUserInput, block = block)).await;
    }

    /// Animates the split from where it was measured to `offset` (Compose's private suspending
    /// `animateToInternal`): settling while it moves; at `offset` in the end, also when cancelled.
    async fn animate_to_internal(&self, offset: i32, initial_velocity: f32) {
        // (Kotlin's `finally`)
        struct Finally(PaneExpansionState, i32);
        impl Drop for Finally {
            fn drop(&mut self) {
                self.0.set_current_dragging_offset(self.1);
                self.0.inner.is_settling.set(false);
            }
        }
        let _finally = Finally(self.clone(), offset);
        self.inner.is_settling.set(true);
        let spec = self.inner.anchoring_animation_spec.borrow().clone().expect("restored before it animates");
        let this = self.clone();
        named_args!(animate(
            self.inner.current_measured_dragging_offset.get() as f32,
            offset as f32,
            initial_velocity = initial_velocity,
            animation_spec = spec,
            block = move |value, _| this.set_current_dragging_offset(value as i32),
        ))
        .await;
    }

    /// The anchor to settle on from `current_position` after a fling left `velocity` (Compose's private
    /// `IndexedAnchorPositionList.getPositionOfTheClosestAnchor`): fast enough, the nearest the fling's way (the
    /// nearest of all if there is none that way); else the nearest.
    fn position_of_the_closest_anchor(
        &self,
        positions: &IndexedAnchorPositionList,
        current_position: i32,
        velocity: f32,
    ) -> IndexedAnchorPosition {
        let max_expansion_width = self.max_expansion_width();
        if velocity >= Self::AnchoringVelocityThreshold {
            positions.min_by(|anchor_position| {
                let delta = anchor_position - current_position;
                // If there's no anchor on the swiping direction, use the closet anchor
                if delta < 0 { max_expansion_width - delta } else { delta }
            })
        } else if velocity <= -Self::AnchoringVelocityThreshold {
            positions.min_by(|anchor_position| {
                let delta = current_position - anchor_position;
                // If there's no anchor on the swiping direction, use the closet anchor
                if delta < 0 { max_expansion_width - delta } else { delta }
            })
        } else {
            positions.min_by(|anchor_position| (current_position - anchor_position).abs())
        }
    }
}

#[kompoz_runtime::named]
impl PaneExpansionState {
    /// Animates the split to `anchor` (one of the state's anchors: panics if not, Kotlin's `require`), from
    /// `initial_velocity`, done when it is there (Compose's suspending `animateTo`); before it is measured it only
    /// becomes the current anchor.
    ///
    /// Kotlin: `animateTo(anchor: PaneExpansionAnchor, initialVelocity: Float = 0F)`
    pub fn animate_to(
        &self,
        anchor: PaneExpansionAnchor,
        #[default(0.0)] initial_velocity: f32,
    ) -> LocalFuture<'static, ()> {
        let this = self.clone();
        Box::pin(async move {
            assert!(
                this.inner.anchors.peek().contains(&anchor),
                "The provided {} is not in the anchor list!",
                anchor.kt_to_string()
            );
            this.set_current_anchor(Some(anchor));
            if let Some(density) = this.inner.measured_density.get() {
                let position = anchor.position_in(this.max_expansion_width(), &density);
                this.animate_to_internal(position, initial_velocity).await;
            }
        })
    }
}

/// `is_settling` back to false when dropped (Kotlin's `finally`).
struct SettlingFlag(MutableState<bool>);

impl Drop for SettlingFlag {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Compose's `Unspecified` for this file's own use.
const Unspecified: i32 = PaneExpansionState::Unspecified;

/// The state's drag and scroll scope (Compose's private `dragScope`: a `DragScope` and, for flings, a
/// `ScrollScope`).
struct ExpansionDragScope(PaneExpansionState);

impl DragScope for ExpansionDragScope {
    fn drag_by(&self, pixels: f32) {
        PaneExpansionDraggableState(self.0.clone()).dispatch_raw_delta(pixels);
    }
}

impl ScrollScope for ExpansionDragScope {
    /// To support fling.
    fn scroll_by(&self, pixels: f32) -> f32 {
        let offset_before_drag = self.0.current_dragging_offset();
        self.drag_by(pixels);
        let consumed = self.0.current_dragging_offset() - offset_before_drag;
        consumed as f32
    }
}

/// The state's draggable state (Compose's internal `draggableState`, an object of the state). Equal for the same
/// pane expansion state.
struct PaneExpansionDraggableState(PaneExpansionState);

impl PartialEq for PaneExpansionDraggableState {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl DraggableState for PaneExpansionDraggableState {
    fn drag_block(&self, drag_priority: MutatePriority, block: DragBlock) -> LocalFuture<'_, ()> {
        let state = self.0.clone();
        Box::pin(async move {
            // (Compose's: not dragging any more only when the drag ends, not when it is cancelled)
            state.inner.is_dragging.set(true);
            let scope: Rc<dyn DragScope> = Rc::new(ExpansionDragScope(state.clone()));
            named_args!(state.inner.drag_mutex.mutate_with(scope, priority = drag_priority, block = block)).await;
            state.inner.is_dragging.set(false);
        })
    }

    fn dispatch_raw_delta(&self, delta: f32) {
        let state = &self.0;
        let remaining_delta = (state.inner.consume_drag_delta)(delta);
        let measured = state.inner.current_measured_dragging_offset.get();
        if measured == Unspecified {
            return;
        }
        state.set_current_dragging_offset((measured as f32 + remaining_delta) as i32);
    }
}

// ---- PaneExpansionStateData -------------------------------------------------------------------------------------

/// What a pane expansion state is made of, saved per key (Compose's internal `PaneExpansionStateData`): the first
/// pane's width, its proportion, the dragged offset and the current anchor, as states. Equal by their values (read
/// without being observed); its clones share the states.
#[derive(Clone)]
pub(crate) struct PaneExpansionStateData {
    pub(crate) first_pane_width_state: MutableState<i32>,
    pub(crate) first_pane_proportion_state: MutableState<f32>,
    pub(crate) current_dragging_offset_state: MutableState<i32>,
    pub(crate) current_anchor_state: MutableState<Option<PaneExpansionAnchor>>,
}

/// Compose's internal constructor of the data.
///
/// Kotlin: `PaneExpansionStateData(firstPaneWidth: Int = Unspecified, firstPaneProportion: Float = Float.NaN, currentDraggingOffset: Int = Unspecified, currentAnchor: PaneExpansionAnchor? = null)`
#[kompoz_runtime::named(type)]
pub(crate) fn PaneExpansionStateData(
    #[default(Unspecified)] first_pane_width: i32,
    #[default(f32::NAN)] first_pane_proportion: f32,
    #[default(Unspecified)] current_dragging_offset: i32,
    #[default(None)] current_anchor: Option<PaneExpansionAnchor>,
) -> PaneExpansionStateData {
    PaneExpansionStateData {
        first_pane_width_state: mutable_int_state_of(first_pane_width),
        first_pane_proportion_state: mutable_float_state_of(first_pane_proportion),
        current_dragging_offset_state: mutable_int_state_of(current_dragging_offset),
        current_anchor_state: mutable_state_of(current_anchor),
    }
}

/// Compose's `equals`, its values read without being observed (a NaN proportion equal to a NaN one: Kotlin's
/// `Float` property compared with `!=` is IEEE's, so two NaN proportions are unequal there; Compose's own
/// `hashCode`/`equals` of the boxed values aside, `firstPaneProportionState != other.firstPaneProportionState`
/// compares primitives — kept: NaN unequal).
impl PartialEq for PaneExpansionStateData {
    fn eq(&self, other: &Self) -> bool {
        without_read_observation(|| {
            if self.first_pane_width_state == other.first_pane_width_state
                && self.first_pane_proportion_state == other.first_pane_proportion_state
                && self.current_dragging_offset_state == other.current_dragging_offset_state
                && self.current_anchor_state == other.current_anchor_state
            {
                return true; // (Kotlin's `this === other`: the same states)
            }
            self.first_pane_width_state.get() == other.first_pane_width_state.get()
                && self.first_pane_proportion_state.get() == other.first_pane_proportion_state.get()
                && self.current_dragging_offset_state.get() == other.current_dragging_offset_state.get()
                && self.current_anchor_state.get() == other.current_anchor_state.get()
        })
    }
}

impl fmt::Debug for PaneExpansionStateData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PaneExpansionStateData")
            .field("first_pane_width", &self.first_pane_width_state.peek())
            .field("first_pane_proportion", &self.first_pane_proportion_state.peek())
            .field("current_dragging_offset", &self.current_dragging_offset_state.peek())
            .field("current_anchor", &self.current_anchor_state.peek())
            .finish()
    }
}

// ---- PaneExpansionAnchor ----------------------------------------------------------------------------------------

/// Where a drag of the split settles (Compose's sealed class `PaneExpansionAnchor`): a proportion of the room
/// ([`Proportion`](Self::Proportion)), or an offset from its start or end ([`Offset`](Self::Offset), made by
/// [`PaneExpansionAnchorOffset::from_start`] / [`from_end`](PaneExpansionAnchorOffset::from_end)). Equal by its
/// values; written as Kotlin's `toString`: `PaneExpansionAnchor(Proportion = 0.5)`, `PaneExpansionAnchor(Offset =
/// 120.0.dp)`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaneExpansionAnchor {
    /// The first pane's proportion of the room, from 0 to 1 (Compose's `PaneExpansionAnchor.Proportion`).
    Proportion(f32),
    /// An offset from the start or the end (Compose's abstract `PaneExpansionAnchor.Offset`).
    Offset(PaneExpansionAnchorOffset),
}

impl PaneExpansionAnchor {
    /// Compose's internal type of no anchor (the savers').
    const UnspecifiedType: i32 = 0;
    /// Compose's internal type of a proportion.
    const ProportionType: i32 = 1;
    /// Compose's internal type of an offset from the start.
    const OffsetFromStartType: i32 = 2;
    /// Compose's internal type of an offset from the end.
    const OffsetFromEndType: i32 = 3;

    /// Its position in a room of `total_size_px` (Compose's internal `positionIn`): a proportion's rounded and
    /// within the room, an offset's rounded to px from the start or the end.
    pub(crate) fn position_in(&self, total_size_px: i32, density: &Density) -> i32 {
        match self {
            PaneExpansionAnchor::Proportion(proportion) => {
                round_to_int(total_size_px as f32 * proportion).clamp(0, total_size_px)
            }
            PaneExpansionAnchor::Offset(offset) => {
                if offset.direction == PaneExpansionAnchorOffsetDirection::FromStart {
                    density.round_to_px(offset.offset)
                } else {
                    total_size_px - density.round_to_px(offset.offset)
                }
            }
        }
    }

    /// Compose's internal `type`.
    fn kind(&self) -> i32 {
        match self {
            PaneExpansionAnchor::Proportion(_) => Self::ProportionType,
            PaneExpansionAnchor::Offset(offset) => offset.direction.0,
        }
    }

    /// Its description, in the UI's language (Compose's composable `description`): "50 percent", "120 DPs from
    /// start", "120 DPs from end".
    pub fn description(&self) -> String {
        let (string, value) = self.description_parts();
        get_string_formatted(string, &[&value])
    }

    /// The same in the language of where `node`'s layout was composed (Compose's internal
    /// `CompositionLocalConsumerModifierNode.description`).
    pub(crate) fn description_of_node(&self, node: &NodeHandle) -> String {
        let (string, value) = self.description_parts();
        get_string_of_node_formatted(node, string, &[&value])
    }

    fn description_parts(&self) -> (Strings, i32) {
        match self {
            PaneExpansionAnchor::Proportion(proportion) => {
                (Strings::defaultPaneExpansionProportionAnchorDescription, (proportion * 100.0) as i32)
            }
            PaneExpansionAnchor::Offset(offset)
                if offset.direction == PaneExpansionAnchorOffsetDirection::FromStart =>
            {
                (Strings::defaultPaneExpansionStartOffsetAnchorDescription, offset.offset.0 as i32)
            }
            PaneExpansionAnchor::Offset(offset) => {
                (Strings::defaultPaneExpansionEndOffsetAnchorDescription, offset.offset.0 as i32)
            }
        }
    }
}

impl From<PaneExpansionAnchorOffset> for PaneExpansionAnchor {
    fn from(offset: PaneExpansionAnchorOffset) -> PaneExpansionAnchor {
        PaneExpansionAnchor::Offset(offset)
    }
}

/// Kotlin's `toString`.
impl fmt::Display for PaneExpansionAnchor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

impl KtToString for PaneExpansionAnchor {
    fn kt_write(&self, out: &mut String) {
        match self {
            PaneExpansionAnchor::Proportion(proportion) => {
                out.push_str("PaneExpansionAnchor(Proportion = ");
                proportion.kt_write(out);
            }
            PaneExpansionAnchor::Offset(offset) => {
                out.push_str("PaneExpansionAnchor(Offset = ");
                offset.offset.kt_write(out);
            }
        }
        out.push(')');
    }
}

/// An offset anchor (Compose's `PaneExpansionAnchor.Offset` and its private `StartOffset` / `EndOffset`): an offset
/// from the start or from the end of the room. Equal by both.
///
/// Kotlin: `PaneExpansionAnchor.Offset`
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaneExpansionAnchorOffset {
    offset: Dp,
    direction: PaneExpansionAnchorOffsetDirection,
}

impl PaneExpansionAnchorOffset {
    /// An anchor `offset` from the start (Compose's `Offset.fromStart`). Panics if negative (Kotlin's `require`).
    ///
    /// Kotlin: `fromStart(offset: Dp): Offset`
    pub fn from_start(offset: Dp) -> PaneExpansionAnchorOffset {
        assert!(offset >= Dp(0.0), "Offset must larger than or equal to 0 dp.");
        PaneExpansionAnchorOffset { offset, direction: PaneExpansionAnchorOffsetDirection::FromStart }
    }

    /// An anchor `offset` from the end (Compose's `Offset.fromEnd`). Panics if negative (Kotlin's `require`).
    ///
    /// Kotlin: `fromEnd(offset: Dp): Offset`
    pub fn from_end(offset: Dp) -> PaneExpansionAnchorOffset {
        assert!(offset >= Dp(0.0), "Offset must larger than or equal to 0 dp.");
        PaneExpansionAnchorOffset { offset, direction: PaneExpansionAnchorOffsetDirection::FromEnd }
    }

    /// The offset (Compose's `offset`).
    pub fn offset(&self) -> Dp {
        self.offset
    }

    /// From where (Compose's `direction`).
    pub fn direction(&self) -> PaneExpansionAnchorOffsetDirection {
        self.direction
    }
}

/// From where an offset anchor is (Compose's value class `PaneExpansionAnchor.Offset.Direction`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PaneExpansionAnchorOffsetDirection(i32);

impl PaneExpansionAnchorOffsetDirection {
    /// From the start of the room.
    pub const FromStart: PaneExpansionAnchorOffsetDirection =
        PaneExpansionAnchorOffsetDirection(PaneExpansionAnchor::OffsetFromStartType);
    /// From the end of the room.
    pub const FromEnd: PaneExpansionAnchorOffsetDirection =
        PaneExpansionAnchorOffsetDirection(PaneExpansionAnchor::OffsetFromEndType);
}

/// Kotlin's `Float.roundToInt()`: half up.
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

// ---- Savers -----------------------------------------------------------------------------------------------------

/// Compose's private key types (the key saver's).
const DefaultPaneExpansionStateKey: i64 = 0;
const TwoPaneExpansionStateKey: i64 = 1;

/// A key saved as `[type, its two panes or null]` (Compose's internal, visible for testing,
/// `PaneExpansionStateKeySaver`).
pub(crate) fn PaneExpansionStateKeySaver() -> Saver<PaneExpansionStateKey> {
    list_saver(
        |key: &PaneExpansionStateKey| match key {
            PaneExpansionStateKey::Default => vec![SavedValue::Int(DefaultPaneExpansionStateKey), SavedValue::Null],
            PaneExpansionStateKey::TwoPane(two) => vec![SavedValue::Int(TwoPaneExpansionStateKey), two.save()],
        },
        |items: &[SavedValue]| {
            let key_type = i64::restore(items.first()?)?;
            let second = items.get(1).unwrap_or(&SavedValue::Null);
            if key_type == DefaultPaneExpansionStateKey || second.is_null() {
                Some(PaneExpansionStateKey::Default)
            } else {
                TwoPaneExpansionStateKeyImpl::restore(second).map(PaneExpansionStateKey::TwoPane)
            }
        },
    )
}

/// The data saved as `[width, proportion, dragged offset, anchor type, anchor value or null]` (Compose's internal,
/// visible for testing, `PaneExpansionStateDataSaver`).
pub(crate) fn PaneExpansionStateDataSaver() -> Saver<PaneExpansionStateData> {
    list_saver(
        |data: &PaneExpansionStateData| {
            let current_anchor = data.current_anchor_state.peek();
            vec![
                SavedValue::Int(data.first_pane_width_state.peek() as i64),
                SavedValue::Double(data.first_pane_proportion_state.peek() as f64),
                SavedValue::Int(data.current_dragging_offset_state.peek() as i64),
                SavedValue::Int(current_anchor.map_or(PaneExpansionAnchor::UnspecifiedType, |a| a.kind()) as i64),
                match current_anchor {
                    Some(PaneExpansionAnchor::Proportion(proportion)) => SavedValue::Double(proportion as f64),
                    Some(PaneExpansionAnchor::Offset(offset)) => SavedValue::Double(offset.offset.0 as f64),
                    None => SavedValue::Null,
                },
            ]
        },
        |items: &[SavedValue]| {
            let float = |v: &SavedValue| match v {
                SavedValue::Double(d) => Some(*d as f32),
                SavedValue::Int(i) => Some(*i as f32),
                _ => None,
            };
            let current_anchor_type = i64::restore(items.get(3)?)? as i32;
            let current_anchor = match current_anchor_type {
                PaneExpansionAnchor::ProportionType => Some(PaneExpansionAnchor::Proportion(float(items.get(4)?)?)),
                PaneExpansionAnchor::OffsetFromStartType => {
                    Some(PaneExpansionAnchorOffset::from_start(Dp(float(items.get(4)?)?)).into())
                }
                PaneExpansionAnchor::OffsetFromEndType => {
                    Some(PaneExpansionAnchorOffset::from_end(Dp(float(items.get(4)?)?)).into())
                }
                _ => None,
            };
            let (first_pane_width, first_pane_proportion, current_dragging_offset) =
                (i64::restore(items.first()?)? as i32, float(items.get(1)?)?, i64::restore(items.get(2)?)? as i32);
            Some(named_args!(PaneExpansionStateData(
                first_pane_width = first_pane_width,
                first_pane_proportion = first_pane_proportion,
                current_dragging_offset = current_dragging_offset,
                current_anchor = current_anchor,
            )))
        },
    )
}

// ---- Anchor positions -------------------------------------------------------------------------------------------

/// The anchors' positions in a room of `max_expansion_width`, each with its anchor's index, by position (Compose's
/// private `List<PaneExpansionAnchor>.toPositions`).
fn to_positions(
    anchors: &[PaneExpansionAnchor],
    max_expansion_width: i32,
    density: &Density,
) -> IndexedAnchorPositionList {
    let mut positions = IndexedAnchorPositionList::with_capacity(anchors.len());
    for (index, anchor) in anchors.iter().enumerate() {
        positions.add(IndexedAnchorPosition::new(anchor.position_in(max_expansion_width, density), index as i32));
    }
    positions.sort();
    positions
}

/// Anchor positions with their anchors' indices (Compose's internal value class `IndexedAnchorPositionList` over a
/// `MutableLongList`): a `Vec<i64>` of packed [`IndexedAnchorPosition`]s (the position in the high int, the index in
/// the low one), so sorting the longs sorts by position, then index.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct IndexedAnchorPositionList(Vec<i64>);

impl IndexedAnchorPositionList {
    pub(crate) fn with_capacity(size: usize) -> IndexedAnchorPositionList {
        IndexedAnchorPositionList(Vec::with_capacity(size))
    }

    #[allow(dead_code)]
    pub(crate) fn size(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn add(&mut self, position: IndexedAnchorPosition) {
        self.0.push(position.0);
    }

    pub(crate) fn sort(&mut self) {
        self.0.sort_unstable();
    }

    #[allow(dead_code)]
    pub(crate) fn get(&self, index: usize) -> IndexedAnchorPosition {
        IndexedAnchorPosition(self.0[index])
    }

    /// Each `(index, position)` by position (Compose's internal `forEach`).
    pub(crate) fn iter(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.0.iter().map(|&v| {
            let p = IndexedAnchorPosition(v);
            (p.index(), p.position())
        })
    }

    /// The first of the least by `selector` of the positions (Compose's private `minBy`). Panics if empty (Kotlin's
    /// `NoSuchElementException`).
    fn min_by<T: PartialOrd>(&self, selector: impl Fn(i32) -> T) -> IndexedAnchorPosition {
        let first = IndexedAnchorPosition(*self.0.first().expect("NoSuchElementException"));
        let mut min_elem = first;
        let mut min_value = selector(first.position());
        for &v in &self.0[1..] {
            let elem = IndexedAnchorPosition(v);
            let value = selector(elem.position());
            if min_value > value {
                min_elem = elem;
                min_value = value;
            }
        }
        min_elem
    }
}

/// An anchor's position and index packed in a long (Compose's internal value class `IndexedAnchorPosition`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct IndexedAnchorPosition(i64);

impl IndexedAnchorPosition {
    pub(crate) fn new(position: i32, index: i32) -> IndexedAnchorPosition {
        IndexedAnchorPosition(pack_ints(position, index))
    }

    pub(crate) fn position(&self) -> i32 {
        unpack_int1(self.0)
    }

    pub(crate) fn index(&self) -> i32 {
        unpack_int2(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn density() -> Density {
        Density((2.0, 1.0))
    }

    #[test]
    fn anchors_take_their_positions_in_the_room() {
        let d = density();
        assert_eq!(PaneExpansionAnchor::Proportion(0.5).position_in(1001, &d), 501);
        assert_eq!(PaneExpansionAnchor::Proportion(1.0).position_in(1000, &d), 1000);
        assert_eq!(
            PaneExpansionAnchor::from(PaneExpansionAnchorOffset::from_start(Dp(100.0))).position_in(1000, &d),
            200
        );
        assert_eq!(
            PaneExpansionAnchor::from(PaneExpansionAnchorOffset::from_end(Dp(100.0))).position_in(1000, &d),
            800
        );
        // Sorted by position, each with its index.
        let anchors = [
            PaneExpansionAnchor::Proportion(0.5),
            PaneExpansionAnchorOffset::from_start(Dp(10.0)).into(),
            PaneExpansionAnchorOffset::from_end(Dp(10.0)).into(),
        ];
        let positions = to_positions(&anchors, 1000, &d);
        assert_eq!(positions.iter().collect::<Vec<_>>(), [(1, 20), (0, 500), (2, 980)]);
        assert_eq!(positions.size(), 3);
        assert_eq!(positions.get(1), IndexedAnchorPosition::new(500, 0));
    }

    #[test]
    fn the_closest_anchor_follows_a_fast_fling() {
        let state = PaneExpansionState::default();
        state.inner.max_expansion_width.set(1000);
        let anchors = [
            PaneExpansionAnchor::Proportion(0.0),
            PaneExpansionAnchor::Proportion(0.5),
            PaneExpansionAnchor::Proportion(1.0),
        ];
        let positions = to_positions(&anchors, 1000, &density());
        let closest = |at, velocity| state.position_of_the_closest_anchor(&positions, at, velocity).position();
        assert_eq!(closest(300, 0.0), 500);
        assert_eq!(closest(200, 0.0), 0);
        assert_eq!(closest(200, 199.0), 0);
        assert_eq!(closest(200, 200.0), 500); // the next one forward
        assert_eq!(closest(700, -200.0), 500); // back
        assert_eq!(closest(1000, 500.0), 1000); // none forward: the closest
    }

    #[test]
    fn anchors_are_written_as_kotlin_writes_them() {
        assert_eq!(PaneExpansionAnchor::Proportion(0.5).to_string(), "PaneExpansionAnchor(Proportion = 0.5)");
        assert_eq!(
            PaneExpansionAnchor::from(PaneExpansionAnchorOffset::from_end(Dp(120.0))).to_string(),
            "PaneExpansionAnchor(Offset = 120.0.dp)"
        );
        assert_ne!(
            PaneExpansionAnchor::from(PaneExpansionAnchorOffset::from_start(Dp(1.0))),
            PaneExpansionAnchor::from(PaneExpansionAnchorOffset::from_end(Dp(1.0)))
        );
        // (outside a composition: English)
        assert_eq!(PaneExpansionAnchor::Proportion(0.333).description(), "33 percent");
    }

    #[test]
    #[should_panic(expected = "Offset must larger than or equal to 0 dp.")]
    fn a_negative_offset_anchor_is_refused() {
        PaneExpansionAnchorOffset::from_start(Dp(-1.0));
    }
}
