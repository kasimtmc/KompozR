//! A shared element of a key and its entries, as Compose's internal `SharedElement.kt` and `SharedElementEntry.kt`:
//! each `shared_element`/`shared_bounds` of a key is an entry of the key's shared element; the element finds the
//! match between its entries (one becoming visible, the others going), its state machine where the bounds animate
//! to, and each entry draws its content in the scope's overlay while the transition is active.

use crate::bounds_animation::BoundsAnimation;
use crate::deferred_enter_exit::SharedMutableTransformState;
use crate::shared_content_node::calculate_pivot;
use crate::shared_transition::{
    LayerRenderer, Observed, OverlayClip, PlaceholderSize, SharedContentState, SharedTransitionScopeImpl,
};
use crate::shared_transition_state_machine::{MatchState, SharedTransitionStateMachine};
use crate::{Animatable, AnimationVector2D, Spring, SpringSpec, VectorConverter, VisibilityThreshold, spring};
use kompoz_graphics::{Offset, Path, Rect, Size};
use kompoz_runtime::{MutableState, RememberObserver, mutable_state_of, named_args};
use kompoz_ui::LookaheadScope;
use kompoz_ui::draw::{DrawScope, GraphicsLayer};
use kompoz_ui::layout::PlacementScope;
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::PlatformHandle;
use kompoz_ui::unit::Velocity;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---- Bounds providers -----------------------------------------------------------------------------------------------

/// What an entry's node tells of its bounds (Compose's internal BoundsProvider).
pub(crate) trait BoundsProvider {
    /// Its bounds in the scope as last placed, none once it is detached (Compose's
    /// `lastBoundsInSharedTransitionScope`).
    fn last_bounds_in_shared_transition_scope(&self) -> Option<Rect>;
    /// The bounds to animate to once it is gone (Compose's `calculateAlternativeTargetBounds`).
    fn calculate_alternative_target_bounds(&self, target_bounds_before_disposed: Rect) -> Option<Rect>;
    /// The mutable transform state of its content's enter/exit (Compose's `modifierLocalTransformState`).
    fn modifier_local_transform_state(&self) -> Option<SharedMutableTransformState> {
        None
    }
}

/// Whether two providers are the same one (Compose's `==` of nodes: by identity).
pub(crate) fn same_provider(a: &Option<Rc<dyn BoundsProvider>>, b: &Option<Rc<dyn BoundsProvider>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

// ---- The shared element -------------------------------------------------------------------------------------------

/// The spring a fling's momentum dies down along (Compose's private `DefaultMomentumSpring`).
fn default_momentum_spring() -> Arc<dyn crate::AnimationSpec<Offset>> {
    Arc::new(named_args!(spring(
        stiffness = Spring::StiffnessMediumLow,
        visibility_threshold = Some(Offset::new(3.0, 3.0))
    )))
}

/// Compose's internal SharedElement: the entries of a key in a scope, the match between them.
pub(crate) struct SharedElement {
    pub(crate) key: SlotKey,
    scope: Weak<SharedTransitionScopeImpl>,
    state_machine: SharedTransitionStateMachine,
    /// Each `shared_element`/`shared_bounds` call of the key: one (no match) or two (a match) mostly; more in an
    /// interrupted transition (several scenes at once), one of them becoming visible (its target), else none animates.
    all_entries: Observed<Vec<Rc<SharedElementEntry>>>,
    enabled_entries: Observed<Vec<Rc<SharedElementEntry>>>,
    momentum_animation: Animatable<Offset, AnimationVector2D>,
    animation_spec_finalized: Cell<bool>,
    /// Compose's `momentumAnimationOffset` lambda, one instance.
    pub(crate) momentum_animation_offset: Rc<dyn Fn() -> Offset>,
}

impl SharedElement {
    pub(crate) fn new(key: SlotKey, scope: &Rc<SharedTransitionScopeImpl>) -> Rc<SharedElement> {
        Rc::new_cyclic(|weak: &Weak<SharedElement>| {
            let w = weak.clone();
            SharedElement {
                key,
                scope: Rc::downgrade(scope),
                state_machine: SharedTransitionStateMachine::new(),
                all_entries: Observed::new(Vec::new()),
                enabled_entries: Observed::new(Vec::new()),
                momentum_animation: Animatable::new(Offset::Zero, Offset::VectorConverter, None, "Animatable"),
                animation_spec_finalized: Cell::new(false),
                momentum_animation_offset: Rc::new(move || w.upgrade().map_or(Offset::Zero, |e| e.momentum_offset())),
            }
        })
    }

    /// Its scope (alive while it is).
    pub(crate) fn scope(&self) -> Rc<SharedTransitionScopeImpl> {
        self.scope.upgrade().expect("kompoz: a shared element outlived its scope")
    }

    /// Its state (a state read).
    pub(crate) fn state(&self) -> MatchState {
        self.state_machine.state()
    }

    /// Its entries that are enabled (a state read).
    pub(crate) fn enabled_entries(&self) -> Vec<Rc<SharedElementEntry>> {
        self.enabled_entries.get()
    }

    /// All its entries (a state read).
    pub(crate) fn all_entries(&self) -> Vec<Rc<SharedElementEntry>> {
        self.all_entries.get()
    }

    /// Whether an enabled entry's bounds animate (Compose's `isAnimating`).
    pub(crate) fn is_animating(&self) -> bool {
        self.enabled_entries().iter().any(|e| e.bounds_animation().is_running())
    }

    /// Whether an enabled entry's content is transformed by hand in a deferred phase (Compose's `isMutating`).
    pub(crate) fn is_mutating(&self) -> bool {
        self.enabled_entries().iter().any(|e| e.active_mutable_transform_state().is_some_and(|s| s.is_mutating()))
    }

    /// Finds the enabled entries and whether one becomes visible, and defers what that means for the match
    /// (Compose's `updateMatch`).
    pub(crate) fn update_match(&self) {
        let mut new_enabled = Vec::new();
        let mut has_visible_content = false;
        for entry in self.all_entries() {
            if entry.is_enabled() {
                if entry.bounds_animation().target() {
                    has_visible_content = true;
                }
                new_enabled.push(entry);
            }
        }
        self.enabled_entries.set(new_enabled);
        self.state_machine.check_for_and_defer_state_updates(self, has_visible_content);
    }

    /// Starts the momentum of a fling the transition starts with (Compose's `updateExitVelocity`): launched
    /// (dispatched), it takes the next frame as its start, as the shared element's animation does.
    pub(crate) fn update_exit_velocity(self: &Rc<Self>, velocity: Velocity) {
        let this = self.clone();
        // (dispatched, as Kotlin's `launch`; its spec finalized at the first frame)
        self.scope().coroutine_scope.launch(async move {
            named_args!(this.momentum_animation.animate_to(
                Offset::Zero,
                animation_spec = default_momentum_spring(),
                initial_velocity = Offset::new(velocity.x, velocity.y)
            ))
            .await;
            this.animation_spec_finalized.set(true);
        });
    }

    /// The offset a fling's momentum adds (Compose's `momentumAnimationOffset`): once the target's animation is
    /// known, the momentum dies down along its spring.
    fn momentum_offset(&self) -> Offset {
        if !self.animation_spec_finalized.get()
            && self.scope().is_transition_active()
            && self.momentum_animation.is_running()
            && let Some(target) = self.enabled_entries().into_iter().find(|e| e.target())
        {
            let target_spec = target.bounds_animation().animation_spec();
            if let Some(s) = target_spec.as_any().downcast_ref::<SpringSpec<kompoz_graphics::Rect>>() {
                // (the target's animation found: the momentum's spec is its spring's)
                let spring: Arc<dyn crate::AnimationSpec<Offset>> =
                    Arc::new(SpringSpec::new(s.damping_ratio, s.stiffness, Some(Offset::VisibilityThreshold)));
                let momentum = self.momentum_animation.clone();
                self.scope().coroutine_scope.launch(async move {
                    named_args!(momentum.animate_to(Offset::Zero, animation_spec = spring)).await;
                });
            }
            self.animation_spec_finalized.set(true);
        }
        self.momentum_animation.value()
    }

    pub(crate) fn invalidate_target_bounds_provider(&self) {
        self.state_machine.invalidate_target_bounds_provider(self);
    }

    pub(crate) fn try_initializing_current_bounds(&self) -> Option<Rect> {
        self.state_machine.try_initializing_current_bounds(self)
    }

    /// The scope's transition is over (Compose's `onSharedTransitionFinished`): no match unless there is one to
    /// keep.
    pub(crate) fn on_shared_transition_finished(&self) {
        let enabled = self.enabled_entries();
        if enabled.len() <= 1 || !enabled.iter().any(|e| e.bounds_animation().target()) {
            self.state_machine.reset_state();
        }
    }

    /// Whether a match is (or may be about to be) found (Compose's `foundMatch`): asked by the developer to set up
    /// animations in composition, so a deferred request is peeked into.
    pub(crate) fn found_match(&self) -> bool {
        let state = self.state();
        state.active_match_found()
            || state.match_is_or_has_been_configured()
            || self.state_machine.active_match_deferred()
    }

    /// Whether its bounds transform is active (Compose's `boundsTransformIsActive`).
    pub(crate) fn bounds_transform_is_active(&self) -> bool {
        self.state().match_is_or_has_been_configured()
    }

    /// An entry was placed in lookahead (Compose's `onLookaheadPlaced`): the one becoming visible configures the
    /// match with its lookahead size, its lookahead position in the scope and its structural offset.
    pub(crate) fn on_lookahead_placed(&self, placement_scope: &PlacementScope, entry: &SharedElementEntry) {
        self.state_machine.process_pending_request(self);
        if self.state().is_no_match_found() || !entry.is_enabled() {
            return;
        }
        let match_state = self.state();
        if entry.bounds_animation().target() && match_state.active_match_found() {
            let Some(coordinates) = placement_scope.coordinates() else { return };
            let lookahead_size = coordinates.size().to_size();
            let scope = entry.shared_element().scope();
            let lookahead_root = scope.lookahead_root();
            let top_left = scope.local_lookahead_position_of(&lookahead_root, &coordinates, Offset::Zero, true);
            let structural_offset = lookahead_root.local_position_of_with(&coordinates, Offset::Zero, false);
            self.state_machine.configure_active_match(self, lookahead_size, top_left, structural_offset);
        }
    }

    /// What its nodes observe (Compose's `observingVisibilityChange`): whether an entry becomes visible.
    pub(crate) fn observing_visibility_change(&self) -> bool {
        self.all_entries().iter().any(|e| e.target() && e.is_enabled())
    }

    pub(crate) fn add_entry(&self, entry: Rc<SharedElementEntry>) {
        let mut all = self.all_entries.get();
        all.push(entry);
        self.all_entries.set(all);
        self.update_match();
    }

    pub(crate) fn remove_entry(&self, entry: &Rc<SharedElementEntry>) {
        let mut all = self.all_entries.get();
        if let Some(i) = all.iter().position(|e| Rc::ptr_eq(e, entry)) {
            all.remove(i);
        }
        self.all_entries.set(all);
        let mut enabled = self.enabled_entries.get();
        if let Some(i) = enabled.iter().position(|e| Rc::ptr_eq(e, entry)) {
            enabled.remove(i);
        }
        self.enabled_entries.set(enabled);
        self.update_match();
    }

    /// Its last target's last bounds, if that target is one of its entries still (Compose's
    /// `obtainBoundsFromLastTarget`): disabled ones too, so they give bounds to start from.
    pub(crate) fn obtain_bounds_from_last_target(
        &self,
        last_target_bounds_provider: Option<&Rc<dyn BoundsProvider>>,
    ) -> Option<Rect> {
        let last = last_target_bounds_provider?;
        let present = self.all_entries().iter().any(|e| e.bounds_provider().is_some_and(|p| Rc::ptr_eq(&p, last)));
        if present { last.last_bounds_in_shared_transition_scope() } else { None }
    }

    /// Lets go of what it holds once it is gone from its scope (KompozR's: its state machine holds its entries'
    /// nodes, which hold it).
    pub(crate) fn release(&self) {
        self.state_machine.release();
    }
}

// ---- An entry -----------------------------------------------------------------------------------------------------

/// Compose's internal SharedElementEntry: a call site's `shared_element`/`shared_bounds` of a key, remembered there
/// (told as it enters and leaves the composition); what draws it in the scope's overlay.
pub(crate) struct SharedElementEntry {
    pub(crate) is_attached: MutableState<bool>,
    z_index: MutableState<f32>,
    render_in_overlay_during_transition: MutableState<bool>,
    shared_element: Observed<Rc<SharedElement>>,
    bounds_animation: Observed<Rc<BoundsAnimation>>,
    placeholder_size: Observed<PlaceholderSize>,
    render_only_when_visible: MutableState<bool>,
    overlay_clip: Observed<Rc<dyn OverlayClip>>,
    user_state: Observed<SharedContentState>,
    /// Whether the hand-off of a deferred phase took place (Compose's `hasHandoffOccurred`).
    pub(crate) has_handoff_occurred: Cell<bool>,
    /// Where its overlay drawing is clipped (Compose's `clipPathInOverlay`), worked out as it draws.
    pub(crate) clip_path_in_overlay: RefCell<Option<Path>>,
    parent_state: RefCell<Option<Rc<SharedElementEntry>>>,
    bounds_provider: RefCell<Option<Rc<dyn BoundsProvider>>>,
    /// The layer its content is recorded in to be drawn in the overlay (Compose's `layer`, a state: drawing follows
    /// it).
    layer: Observed<Option<GraphicsLayer>>,
    /// What draws its visual debugging (Compose's node's `lookaheadAnimationVisualDebugHelper`), made as it is first
    /// asked to.
    pub(crate) debug_helper: RefCell<Option<Rc<crate::lookahead_debug::LookaheadAnimationVisualDebugHelper>>>,
}

/// Remembers an entry where it is called: it is added to its scope as it enters the composition, removed as it
/// leaves (Compose's RememberObserver of SharedElementEntry).
pub(crate) struct EntryObserver(pub(crate) Rc<SharedElementEntry>);

impl RememberObserver for EntryObserver {
    fn on_remembered(&self) {
        let shared_element = self.0.shared_element();
        shared_element.scope().on_entry_added(&self.0);
        shared_element.invalidate_target_bounds_provider();
    }
    fn on_forgotten(&self) {
        let shared_element = self.0.shared_element();
        shared_element.scope().on_entry_removed(&self.0);
        shared_element.invalidate_target_bounds_provider();
    }
}

impl SharedElementEntry {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        shared_element: Rc<SharedElement>,
        bounds_animation: Rc<BoundsAnimation>,
        placeholder_size: PlaceholderSize,
        render_only_when_visible: bool,
        overlay_clip: Rc<dyn OverlayClip>,
        render_in_overlay_during_transition: bool,
        user_state: SharedContentState,
        z_index: f32,
    ) -> SharedElementEntry {
        SharedElementEntry {
            is_attached: mutable_state_of(false),
            z_index: mutable_state_of(z_index),
            render_in_overlay_during_transition: mutable_state_of(render_in_overlay_during_transition),
            shared_element: Observed::new(shared_element),
            bounds_animation: Observed::new(bounds_animation),
            placeholder_size: Observed::new(placeholder_size),
            render_only_when_visible: mutable_state_of(render_only_when_visible),
            overlay_clip: Observed::new(overlay_clip),
            user_state: Observed::new(user_state),
            has_handoff_occurred: Cell::new(false),
            clip_path_in_overlay: RefCell::new(None),
            parent_state: RefCell::new(None),
            bounds_provider: RefCell::new(None),
            layer: Observed::new(None),
            debug_helper: RefCell::new(None),
        }
    }

    pub(crate) fn shared_element(&self) -> Rc<SharedElement> {
        self.shared_element.get()
    }
    pub(crate) fn set_shared_element(&self, value: Rc<SharedElement>) {
        self.shared_element.set(value);
    }
    pub(crate) fn bounds_animation(&self) -> Rc<BoundsAnimation> {
        self.bounds_animation.get()
    }
    pub(crate) fn set_bounds_animation(&self, value: Rc<BoundsAnimation>) {
        self.bounds_animation.set(value);
    }
    pub(crate) fn placeholder_size(&self) -> PlaceholderSize {
        self.placeholder_size.get()
    }
    pub(crate) fn set_placeholder_size(&self, value: PlaceholderSize) {
        self.placeholder_size.set(value);
    }
    pub(crate) fn set_render_only_when_visible(&self, value: bool) {
        self.render_only_when_visible.set(value);
    }
    pub(crate) fn overlay_clip(&self) -> Rc<dyn OverlayClip> {
        self.overlay_clip.get()
    }
    pub(crate) fn set_overlay_clip(&self, value: Rc<dyn OverlayClip>) {
        self.overlay_clip.set(value);
    }
    pub(crate) fn user_state(&self) -> SharedContentState {
        self.user_state.get()
    }
    pub(crate) fn set_user_state(&self, value: SharedContentState) {
        self.user_state.set(value);
    }
    pub(crate) fn set_render_in_overlay_during_transition(&self, value: bool) {
        self.render_in_overlay_during_transition.set(value);
    }

    /// Its z order in the overlay; a change sorts the overlay again (Compose's `zIndex` setter).
    pub(crate) fn set_z_index(&self, value: f32) {
        if self.z_index.peek() != value {
            self.z_index.set(value);
            self.shared_element().scope().z_order_changed();
        }
    }

    /// The entry of the shared element its node is in (Compose's `parentState`); a change sorts the overlay again.
    pub(crate) fn set_parent_state(&self, value: Option<Rc<SharedElementEntry>>) {
        let changed = match (&*self.parent_state.borrow(), &value) {
            (None, None) => false,
            (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
            _ => true,
        };
        if changed {
            *self.parent_state.borrow_mut() = value;
            self.shared_element().scope().z_order_changed();
        }
    }

    pub(crate) fn bounds_provider(&self) -> Option<Rc<dyn BoundsProvider>> {
        self.bounds_provider.borrow().clone()
    }
    pub(crate) fn set_bounds_provider(&self, value: Option<Rc<dyn BoundsProvider>>) {
        *self.bounds_provider.borrow_mut() = value;
    }

    pub(crate) fn layer(&self) -> Option<GraphicsLayer> {
        self.layer.get()
    }
    pub(crate) fn set_layer(&self, value: Option<GraphicsLayer>) {
        self.layer.set(value);
    }

    /// Takes another layer, letting the old one go (Compose's node's `layer` setter: `releaseGraphicsLayer`).
    pub(crate) fn replace_layer(&self, value: Option<GraphicsLayer>) {
        let old = self.layer.peek();
        if old != value {
            if let Some(old) = old {
                old.release();
            }
            self.set_layer(value);
        }
    }

    /// The mutable transform state that drives a deferred phase's transforms (Compose's
    /// `activeMutableTransformState`): during one, the leaving content's transforms apply to both shared elements,
    /// so the entry coming in takes the leaving one's state. None if it doesn't take part in deferred transforms.
    pub(crate) fn active_mutable_transform_state(&self) -> Option<SharedMutableTransformState> {
        if !self.user_state().config().permit_transform_during_deferred_transition() {
            return None;
        }
        let transform_state = self.bounds_provider().and_then(|p| p.modifier_local_transform_state());
        // (during a deferred phase the transition is held back at its state: `target` is the other way round)
        let is_incoming = if self.is_mutating() { !self.target() } else { self.target() };
        if is_incoming {
            let exiting = self
                .shared_element()
                .enabled_entries()
                .into_iter()
                .find(|e| if e.is_mutating() { e.target() } else { !e.target() });
            return exiting
                .and_then(|e| e.bounds_provider())
                .and_then(|p| p.modifier_local_transform_state())
                .or(transform_state);
        }
        transform_state
    }

    /// Whether its container is transformed by hand during a deferred phase (Compose's private `isMutating`).
    fn is_mutating(&self) -> bool {
        self.bounds_provider().and_then(|p| p.modifier_local_transform_state()).is_some_and(|s| s.is_mutating())
    }

    /// Whether it takes part (Compose's `isEnabled`): attached, and enabled by its user's configuration.
    pub(crate) fn is_enabled(&self) -> bool {
        self.is_attached.get() && self.user_state().is_enabled_by_user()
    }

    /// The bounds to animate to once its target is gone (Compose's `calculateTargetBounds`): the configuration's.
    pub(crate) fn calculate_target_bounds(&self, previous_target_bounds_before_losing_target: Rect) -> Option<Rect> {
        let user_state = self.user_state();
        let layout_size: Size = self.shared_element().scope().lookahead_root().size().to_size();
        user_state.config().alternative_target_bounds_in_transition_scope_after_removal(
            &user_state,
            previous_target_bounds_before_losing_target,
            layout_size,
        )
    }

    /// Whether it becomes visible (Compose's `target`).
    pub(crate) fn target(&self) -> bool {
        self.bounds_animation().target()
    }

    /// Whether its content is drawn at all (Compose's private `shouldRenderAtAll`): not an outgoing shared element's.
    fn should_render_at_all(&self) -> bool {
        let state = self.shared_element().state();
        self.bounds_animation().target()
            // (only while the active match was removed during the transition)
            || (state.match_is_or_has_been_configured() && !state.active_match_found())
            || !self.render_only_when_visible.get()
    }

    /// Whether it is drawn in the overlay (Compose's `shouldRenderInOverlay`): during the transition.
    pub(crate) fn should_render_in_overlay(&self) -> bool {
        let shared_element = self.shared_element();
        self.should_render_at_all()
            && shared_element.bounds_transform_is_active()
            && self.is_enabled()
            && self.render_in_overlay_during_transition.get()
            && (shared_element.scope().is_transition_active() || self.is_mutating())
    }

    /// Whether it is drawn where it is (Compose's `shouldRenderInPlace`).
    pub(crate) fn should_render_in_place(&self) -> bool {
        !self.shared_element().bounds_transform_is_active()
            || (!self.should_render_in_overlay() && self.should_render_at_all())
    }
}

impl LayerRenderer for SharedElementEntry {
    fn parent_state(&self) -> Option<Rc<SharedElementEntry>> {
        self.parent_state.borrow().clone()
    }

    fn z_index(&self) -> f32 {
        self.z_index.get()
    }

    fn as_entry(&self) -> Option<&SharedElementEntry> {
        Some(self)
    }

    fn draw_in_overlay(&self, scope: &mut DrawScope, platform: &PlatformHandle) {
        let should_render_in_overlay = self.should_render_in_overlay();
        if should_render_in_overlay && self.layer().is_none() {
            // (made here if a parent between the overlay and its node records before its node draws: the overlay
            // has a layer to draw, which the node fills in the same frame)
            self.set_layer(Some(GraphicsLayer::new(Some(platform.clone()))));
        }
        let Some(layer) = self.layer() else { return };
        // (no bounds yet: not placed since the match was found — composed but not laid out, precomposed — so not
        // drawn where it is either)
        let shared_element = self.shared_element();
        let Some(current_bounds) = shared_element.state().current_bounds() else { return };
        if !should_render_in_overlay {
            return;
        }
        let (x, y) = (current_bounds.left, current_bounds.top);
        let (mut scale, mut offset_x, mut offset_y, mut pivot) = (1.0, 0.0, 0.0, Offset::Zero);
        let mutable_transform_state = self.active_mutable_transform_state();
        let parent_coordinates = mutable_transform_state.as_ref().and_then(|s| s.parent_layout_coordinates());
        let root = shared_element.scope().root();
        if let (Some(state), Some(parent)) = (&mutable_transform_state, &parent_coordinates)
            && state.is_mutating()
            && parent.is_attached()
            && root.is_attached()
        {
            scale = state.active_scale();
            let offset = state.active_offset();
            offset_x = offset.x as f32;
            offset_y = offset.y as f32;
            pivot = calculate_pivot(parent, &root, state.active_transform_origin());
        }
        let clip_path = self.clip_path_in_overlay.borrow().clone();
        scope.translate_with(offset_x, offset_y, |s| {
            s.scale_with(scale, scale, pivot, |s| match &clip_path {
                Some(path) => s.clip_path_with(path, kompoz_ui::graphics_context::ClipOp::Intersect, |s| {
                    s.translate_with(x, y, |s| s.draw_layer(&layer));
                }),
                None => s.translate_with(x, y, |s| s.draw_layer(&layer)),
            });
        });
    }
}
