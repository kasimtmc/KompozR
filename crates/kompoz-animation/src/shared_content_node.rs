//! The node of `shared_element`/`shared_bounds`, as Compose's internal `SharedContentNode.kt` (SharedBoundsNode): it
//! records the target's bounds in the lookahead pass of the entry becoming visible, animates the layout's bounds in
//! the approach pass, and draws its content in place or, during the transition, in a layer the scope's overlay draws.

use crate::bounds::BoundsTransform;
use crate::bounds_animation::BoundsAnimation;
use crate::converter::fast_round_to_int;
use crate::deferred_enter_exit::{ModifierLocalSharedMutableTransformState, SharedMutableTransformState};
use crate::lookahead_debug::{
    IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED, LocalLookaheadAnimationVisualDebugColor,
    LocalLookaheadAnimationVisualDebugConfig, LookaheadAnimationVisualDebugConfig, LookaheadAnimationVisualDebugHelper,
};
use crate::shared_element::{BoundsProvider, SharedElement, SharedElementEntry};
use crate::shared_transition_state_machine::TargetData;
use crate::{AnimationVector4D, FiniteAnimationSpec, VisibilityThreshold, spring};
use kompoz_graphics::{Density, Dp, IntOffset, IntSize, LayoutDirection, Offset, Rect};
use kompoz_runtime::named_args;
use kompoz_text::{FontFamilyResolver, TextMeasurer};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, GraphicsLayer};
use kompoz_ui::layout::{
    ApproachLayoutModifierNode, ApproachMeasureScope, LayoutCoordinates, LayoutModifierNode, Measurable, MeasureResult,
    MeasureScope, Placeable, PlacementScope, TransformOrigin,
};
use kompoz_ui::modifier::{ModifierLocal, ModifierLocalMap, ModifierLocalModifierNode, ObserverModifierNode};
use kompoz_ui::platform::{LocalFontFamilyResolver, LocalPlatform};
use kompoz_ui::{
    Constraints, LookaheadScope, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The modifier local a shared element's node provides its entry by (Compose's internal
/// `ModifierLocalSharedElementInternalState`): the entry of the shared element a node is in.
pub(crate) static ModifierLocalSharedElementInternalState: ModifierLocal<Option<Rc<SharedElementEntry>>> =
    kompoz_ui::modifier::modifier_local_of(|| None);

/// The element (Compose's SharedBoundsNodeElement): equal for the same entry.
pub(crate) struct SharedBoundsNodeElement {
    pub(crate) shared_element_state: Rc<SharedElementEntry>,
}

impl PartialEq for SharedBoundsNodeElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared_element_state, &other.shared_element_state)
    }
}

impl ModifierNodeElement for SharedBoundsNodeElement {
    type Node = SharedBoundsNode;
    fn create(&self) -> SharedBoundsNode {
        SharedBoundsNode::new(self.shared_element_state.clone())
    }
    fn update(&self, node: &mut SharedBoundsNode) {
        node.set_shared_element_entry(self.shared_element_state.clone());
    }
    fn name(&self) -> &'static str {
        "sharedBounds"
    }
}

/// What of the node its placement blocks, its entry (as its bounds provider) and the state machine reach while it
/// isn't running (Compose's fields of SharedBoundsNode: one object here, the node's own `RefCell` kept for its
/// callbacks).
pub(crate) struct SharedBoundsNodeState {
    handle: RefCell<Option<NodeHandle>>,
    entry: RefCell<Rc<SharedElementEntry>>,
    forced_handoff_bounds: Cell<Option<Rect>>,
    forced_handoff_velocity: Cell<Option<AnimationVector4D>>,
    bounds_before_detached: Cell<Option<Rect>>,
    resolved_transform_state: RefCell<Option<SharedMutableTransformState>>,
    is_placed: Cell<bool>,
    weak_self: Weak<SharedBoundsNodeState>,
}

impl SharedBoundsNodeState {
    fn entry(&self) -> Rc<SharedElementEntry> {
        self.entry.borrow().clone()
    }

    fn shared_element(&self) -> Rc<SharedElement> {
        self.entry().shared_element()
    }

    fn bounds_animation(&self) -> Rc<BoundsAnimation> {
        self.entry().bounds_animation()
    }

    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    fn is_attached(&self) -> bool {
        self.handle().is_some_and(|h| h.is_attached())
    }

    /// Its layout's coordinates (Compose's `requireLayoutCoordinates()`, the approach's).
    fn approach_coordinates(&self) -> Option<LayoutCoordinates> {
        self.handle().and_then(|h| h.coordinates())
    }

    /// The coordinates of the scope's root (Compose's `rootCoords`).
    fn root_coordinates(&self) -> LayoutCoordinates {
        self.shared_element().scope().root()
    }

    /// Its layout's lookahead coordinates (Compose's `requireLookaheadLayoutCoordinates`).
    fn lookahead_layout_coordinates(&self) -> LayoutCoordinates {
        let coordinates = self.approach_coordinates().expect("kompoz: a shared element's node without coordinates");
        self.shared_element().scope().to_lookahead_coordinates(&coordinates)
    }

    fn set_layer(&self, value: Option<GraphicsLayer>) {
        self.entry().replace_layer(value);
    }

    /// Where it is when the match outlives the transition (Compose's `approachPlaceMatchBeyondTransition`): the
    /// visible one where it is; one that isn't visible follows the visible one's bounds.
    fn approach_place_match_beyond_transition(&self, p: &PlacementScope, placeable: &Placeable, current_bounds: Rect) {
        if !self.bounds_animation().target() {
            // (its coordinates unknown: never placed yet)
            let at = match p.coordinates() {
                Some(coordinates) => {
                    let position_in_scope = self.root_coordinates().local_position_of(&coordinates, Offset::Zero);
                    round(current_bounds.top_left() - position_in_scope)
                }
                None => IntOffset::Zero,
            };
            p.place(placeable, at.x, at.y);
        } else {
            p.place(placeable, 0, 0);
        }
    }

    /// Places a matched shared element during the transition along its bounds animation, which starts here
    /// (Compose's `approachPlaceMatchInTransition`): once started, the target changes only with the target's
    /// structural offset; what the motion frame of reference adds (scrolling) is applied to the animated value as
    /// it is.
    fn approach_place_match_in_transition(
        &self,
        p: &PlacementScope,
        placeable: &Placeable,
        target_data: &TargetData,
        current_bounds: Rect,
    ) {
        let Some(coordinates) = p.coordinates() else {
            // (a shallow placement: left to the real one)
            p.place(placeable, 0, 0);
            return;
        };
        let shared_element = self.shared_element();
        let bounds_animation = self.bounds_animation();
        let root = self.root_coordinates();
        let active_match_removed = !shared_element.state().active_match_found();
        let position_in_scope = root.local_position_of(&coordinates, Offset::Zero);
        let target_bounds = target_data.target_bounds();
        let debugging = IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED
            && self.handle().is_some_and(|h| h.current_value_of(&LocalLookaheadAnimationVisualDebugConfig).is_enabled);
        let mut spec: Option<Arc<dyn FiniteAnimationSpec<Rect>>> = None;
        let (forced_value, forced_velocity) = (self.forced_handoff_bounds.get(), self.forced_handoff_velocity.get());
        if active_match_removed {
            let transform = removed_match_bounds_transform();
            if debugging {
                spec = Some(transform.create_animation_spec(current_bounds, target_bounds));
            }
            bounds_animation.animate(current_bounds, target_bounds, Some(transform), forced_value, forced_velocity);
        } else {
            if debugging {
                spec = Some(Arc::new(spring::<Rect>()));
            }
            bounds_animation.animate(current_bounds, target_bounds, None, forced_value, forced_velocity);
        }
        if debugging && let (Some(helper), Some(spec)) = (self.entry().debug_helper.borrow().clone(), spec) {
            helper.calculate_path(&spec, current_bounds, target_bounds, Rect::Zero);
        }
        self.forced_handoff_bounds.set(None);
        self.forced_handoff_velocity.set(None);
        let animated_bounds = bounds_animation.value();
        let animated_top_left = animated_bounds.map(|b| target_data.calculate_offset_from_direct_manipulation(b));
        let top_left = if bounds_animation.target() || active_match_removed {
            // (the visible one sets the bounds, animated or, once done, its own)
            let top_left = animated_top_left.unwrap_or(position_in_scope);
            let bounds = match (animated_top_left, animated_bounds) {
                (Some(at), Some(animated)) => Rect::from_offset_size(at, animated.size()),
                _ => Rect::from_offset_size(position_in_scope, coordinates.size().to_size()),
            };
            shared_element.state().update_bounds(bounds);
            top_left
        } else {
            animated_top_left.unwrap_or(current_bounds.top_left())
        };
        let mut final_top_left = top_left;
        if let Some(state) = self.entry().active_mutable_transform_state()
            && state.is_mutating()
        {
            if !bounds_animation.is_running() {
                final_top_left = position_in_scope;
            }
            if let Some(parent) = state.parent_layout_coordinates()
                && parent.is_attached()
                && root.is_attached()
            {
                let pivot = calculate_pivot(&parent, &root, state.active_transform_origin());
                final_top_left = transform(top_left, pivot, state.active_scale(), state.active_offset());
            }
        }
        let local_offset = coordinates.local_position_of(&root, final_top_left);
        p.place(placeable, fast_round_to_int(local_offset.x), fast_round_to_int(local_offset.y));
    }

    /// The deferred phase's last values handed over to the transition's animation, once (Compose's
    /// `updateDeferredHandoffValues`).
    fn update_deferred_handoff_values(&self) {
        let shared_element = self.shared_element();
        let state = shared_element.state();
        if state.target_data().is_none() {
            return;
        }
        let Some(current_bounds) = state.current_bounds() else { return };
        let entry = self.entry();
        let mutable_state = entry.active_mutable_transform_state();
        let Some(mutable_state) = mutable_state.filter(|s| s.is_handoff_active()) else {
            entry.has_handoff_occurred.set(false);
            return;
        };
        if entry.has_handoff_occurred.get() {
            return;
        }
        let root = self.root_coordinates();
        let Some(parent) = mutable_state.parent_layout_coordinates().filter(|p| p.is_attached()) else { return };
        if !root.is_attached() {
            return;
        }
        let manual_scale = mutable_state.last_manual_scale();
        let manual_offset = mutable_state.last_manual_slide();
        let pivot = calculate_pivot(&parent, &root, mutable_state.last_transform_origin());
        let new_top_left = transform(current_bounds.top_left(), pivot, manual_scale, manual_offset);
        let is_own_container_mutating = entry
            .bounds_provider()
            .and_then(|p| p.modifier_local_transform_state())
            .is_some_and(|s| s == mutable_state);
        let container_applies_transforms = !entry.should_render_in_overlay() && is_own_container_mutating;
        let scale = if container_applies_transforms { 1.0 } else { manual_scale };
        let new_right = new_top_left.x + current_bounds.width() * scale;
        let new_bottom = new_top_left.y + current_bounds.height() * scale;
        self.forced_handoff_bounds.set(Some(Rect::new(new_top_left.x, new_top_left.y, new_right, new_bottom)));
        if !container_applies_transforms {
            let scale_velocity = mutable_state.scale_handoff_velocity().map_or(0.0, |v| v.value);
            let offset_velocity = mutable_state.slide_handoff_velocity();
            let offset_velocity_x = offset_velocity.map_or(0.0, |v| v.v1);
            let offset_velocity_y = offset_velocity.map_or(0.0, |v| v.v2);
            let velocity_left = (current_bounds.left - pivot.x) * scale_velocity + offset_velocity_x;
            let velocity_top = (current_bounds.top - pivot.y) * scale_velocity + offset_velocity_y;
            let velocity_right = (current_bounds.right - pivot.x) * scale_velocity + offset_velocity_x;
            let velocity_bottom = (current_bounds.bottom - pivot.y) * scale_velocity + offset_velocity_y;
            self.forced_handoff_velocity.set(Some(AnimationVector4D {
                v1: velocity_left,
                v2: velocity_top,
                v3: velocity_right,
                v4: velocity_bottom,
            }));
        } else {
            self.forced_handoff_velocity.set(None);
        }
        entry.has_handoff_occurred.set(true);
    }
}

impl BoundsProvider for SharedBoundsNodeState {
    fn last_bounds_in_shared_transition_scope(&self) -> Option<Rect> {
        // (detached, or detached and attached again since it was placed: its last place is gone)
        if !self.is_attached() {
            return None;
        }
        // (attached, not placed yet)
        if !self.is_placed.get() {
            return self.bounds_before_detached.get();
        }
        let approach = self.approach_coordinates()?;
        Some(Rect::from_offset_size(
            self.root_coordinates().local_position_of(&approach, Offset::Zero),
            approach.size().to_size(),
        ))
    }

    fn calculate_alternative_target_bounds(&self, target_bounds_before_disposed: Rect) -> Option<Rect> {
        self.entry().calculate_target_bounds(target_bounds_before_disposed)
    }

    fn modifier_local_transform_state(&self) -> Option<SharedMutableTransformState> {
        if !self.is_attached() {
            return None;
        }
        if let Some(state) = self.resolved_transform_state.borrow().clone() {
            return Some(state);
        }
        let state = self.handle()?.cx()?.modifier_local_current(&ModifierLocalSharedMutableTransformState);
        *self.resolved_transform_state.borrow_mut() = state.clone();
        state
    }
}

/// What animates a match whose visible entry was removed (Compose's literal `BoundsTransform { _, _ ->
/// spring(visibilityThreshold = Rect.VisibilityThreshold) }`).
fn removed_match_bounds_transform() -> Rc<dyn BoundsTransform> {
    Rc::new(|_: Rect, _: Rect| named_args!(spring(visibility_threshold = Some(Rect::VisibilityThreshold))))
}

/// An offset rounded to whole pixels, each way half up (Compose's `Offset.round`).
fn round(offset: Offset) -> IntOffset {
    IntOffset::new(fast_round_to_int(offset.x), fast_round_to_int(offset.y))
}

/// Compose's internal SharedBoundsNode: an approach layout, a modifier local's provider (its entry), an observer of
/// its shared element's visibility; its drawing is [`SharedBoundsDrawNode`]'s, after it in the chain.
pub(crate) struct SharedBoundsNode {
    state: Rc<SharedBoundsNodeState>,
    provided_values: ModifierLocalMap,
}

impl SharedBoundsNode {
    fn new(entry: Rc<SharedElementEntry>) -> SharedBoundsNode {
        let provided_values = ModifierLocalMap::of(&ModifierLocalSharedElementInternalState, Some(entry.clone()));
        SharedBoundsNode {
            state: Rc::new_cyclic(|weak_self| SharedBoundsNodeState {
                handle: RefCell::new(None),
                entry: RefCell::new(entry),
                forced_handoff_bounds: Cell::new(None),
                forced_handoff_velocity: Cell::new(None),
                bounds_before_detached: Cell::new(None),
                resolved_transform_state: RefCell::new(None),
                is_placed: Cell::new(false),
                weak_self: weak_self.clone(),
            }),
            provided_values,
        }
    }

    /// Takes another entry (Compose's `sharedElementEntry` setter): the old one no longer attached, the new one set
    /// up as it is.
    fn set_shared_element_entry(&mut self, value: Rc<SharedElementEntry>) {
        let old = self.state.entry();
        if Rc::ptr_eq(&old, &value) {
            return;
        }
        old.is_attached.set(false);
        *self.state.entry.borrow_mut() = value.clone();
        let attached = self.state.is_attached();
        value.is_attached.set(attached);
        if attached && let Some(cx) = self.state.handle().and_then(|h| h.cx()) {
            self.setup(&cx);
        }
    }

    /// Provides its entry, which takes its parent's and it as its bounds provider (Compose's `setup`).
    fn setup(&mut self, cx: &NodeCx) {
        let entry = self.state.entry();
        self.provided_values = ModifierLocalMap::of(&ModifierLocalSharedElementInternalState, Some(entry.clone()));
        entry.set_parent_state(cx.modifier_local_current(&ModifierLocalSharedElementInternalState));
        self.state.set_layer(None);
        self.state.is_placed.set(false);
        let provider: Rc<dyn BoundsProvider> = self.state.weak_self.upgrade().expect("its own state");
        entry.set_bounds_provider(Some(provider));
    }

    /// Lays it out at its size along the transition (Compose's `approachPlace`): a matched one's size its
    /// placeholder's, placed where the match says.
    fn approach_place(&self, scope: &ApproachMeasureScope, placeable: Placeable) -> MeasureResult {
        let state = self.state.clone();
        let placeable_size = IntSize::new(placeable.width(), placeable.height());
        let size = if state.shared_element().state().match_is_or_has_been_configured() {
            // (a match found, animating)
            let lookahead_size = state.lookahead_layout_coordinates().size();
            state.entry().placeholder_size().calculate_size(lookahead_size, placeable_size)
        } else {
            placeable_size
        };
        scope.layout(size.width, size.height, move |p: &PlacementScope| {
            state.is_placed.set(true);
            state.bounds_before_detached.set(None);
            let match_state = state.shared_element().state();
            if !state.entry().is_enabled() {
                p.place(&placeable, 0, 0);
            } else if match_state.match_is_or_has_been_configured() {
                let target_data = match_state.target_data().unwrap_or_else(|| {
                    panic!("Match State is configured, but target data is null. State = {match_state:?}")
                });
                let current_bounds = match_state.current_bounds().unwrap_or_else(|| {
                    panic!("Match State is configured, but current bounds is null. State = {match_state:?}")
                });
                if state.shared_element().scope().is_transition_active() {
                    state.approach_place_match_in_transition(p, &placeable, &target_data, current_bounds);
                } else {
                    // (the match outlives the transition: the one not visible was left in the tree)
                    state.approach_place_match_beyond_transition(p, &placeable, current_bounds);
                }
            } else {
                // (no match yet, or the active match not configured yet)
                p.place(&placeable, 0, 0);
            }
        })
    }
}

/// The drawing of `shared_element`/`shared_bounds` (Compose's SharedBoundsNode's `draw`): a node of its own after
/// the layout node, so it draws as Compose's draw of a layout node does — in the space of what the layout node wraps
/// (the content where the approach placed it, at its size), which KompozR's coordinators give to the nodes after a
/// layout node, not to the layout node itself.
pub(crate) struct SharedBoundsDrawElement {
    pub(crate) shared_element_state: Rc<SharedElementEntry>,
}

impl PartialEq for SharedBoundsDrawElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared_element_state, &other.shared_element_state)
    }
}

impl ModifierNodeElement for SharedBoundsDrawElement {
    type Node = SharedBoundsDrawNode;
    fn create(&self) -> SharedBoundsDrawNode {
        SharedBoundsDrawNode {
            entry: self.shared_element_state.clone(),
            handle: None,
            text_measurer: None,
            current_resolver: None,
            current_density: None,
            current_layout_direction: None,
        }
    }
    fn update(&self, node: &mut SharedBoundsDrawNode) {
        node.entry = self.shared_element_state.clone();
    }
    fn name(&self) -> &'static str {
        "sharedBounds"
    }
}

/// [`SharedBoundsDrawElement`]'s node: the content drawn in place, or recorded in the entry's layer for the overlay.
pub(crate) struct SharedBoundsDrawNode {
    entry: Rc<SharedElementEntry>,
    handle: Option<NodeHandle>,
    text_measurer: Option<TextMeasurer>,
    current_resolver: Option<Rc<dyn FontFamilyResolver>>,
    current_density: Option<Density>,
    current_layout_direction: Option<LayoutDirection>,
}

impl ModifierNode for SharedBoundsDrawNode {
    node_capabilities!(draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
}

impl SharedBoundsDrawNode {
    /// Draws the content, with the visual debugging's marks when it is on (Compose's
    /// `drawContentWithOptionalDebug`): into `layer` if given.
    fn draw_content_with_optional_debug(
        &mut self,
        scope: &mut ContentDrawScope,
        layer: Option<&GraphicsLayer>,
        bounds: Option<Rect>,
    ) {
        let handle = self.handle.clone();
        let config = if IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED {
            handle.as_ref().map(|h| h.current_value_of(&LocalLookaheadAnimationVisualDebugConfig))
        } else {
            None
        };
        match (config, layer) {
            (Some(config), _) if config.is_enabled => {
                self.draw_content_with_lookahead_animation_debug(scope, layer, bounds, &config);
            }
            (_, Some(layer)) => {
                let size = layer_size(scope);
                scope.record_with(layer, size, |s| s.draw_content());
            }
            _ => scope.draw_content(),
        }
    }

    /// The same with the marks (Compose's `drawContentWithLookaheadAnimationDebug`).
    fn draw_content_with_lookahead_animation_debug(
        &mut self,
        scope: &mut ContentDrawScope,
        layer: Option<&GraphicsLayer>,
        bounds: Option<Rect>,
        config: &LookaheadAnimationVisualDebugConfig,
    ) {
        let Some(handle) = self.handle.clone() else { return };
        if self.entry.debug_helper.borrow().is_none() {
            *self.entry.debug_helper.borrow_mut() = Some(Rc::new(LookaheadAnimationVisualDebugHelper::new()));
        }
        if self.current_density.is_none() {
            self.current_density = Some(handle.density());
            self.current_layout_direction = Some(handle.layout_direction());
        }
        let stroke_weight = scope.density().to_px(Dp(2.5));
        let target_data = self.entry.shared_element().state().target_data();
        self.update_text_measurer(handle.current_value_of(&LocalFontFamilyResolver));
        let helper = self.entry.debug_helper.borrow().clone().expect("the helper made above");
        let entry = self.entry.clone();
        let shared_element = entry.shared_element();
        let animation_color = handle.current_value_of(&LocalLookaheadAnimationVisualDebugColor);
        let text_measurer = self.text_measurer.as_ref();
        let key = format!("{:?}", shared_element.key);
        let draw_debug = |s: &mut kompoz_ui::draw::DrawScope| {
            if !entry.is_enabled() {
                return;
            }
            let inactive = |s: &mut kompoz_ui::draw::DrawScope| {
                helper.draw_inactive_visualizations(
                    s,
                    config.inactive_element_color,
                    config.is_show_key_label_enabled,
                    stroke_weight,
                    &key,
                    text_measurer,
                );
            };
            if !shared_element.scope().is_transition_active() {
                inactive(s);
                return;
            }
            if shared_element.bounds_transform_is_active() {
                let enabled = shared_element.enabled_entries().len();
                if enabled > 2 {
                    helper.draw_multiple_matches_element(
                        s,
                        config.multiple_matches_color,
                        config.is_show_key_label_enabled,
                        &key,
                        enabled - 1,
                        text_measurer.expect("its text measurer"),
                        stroke_weight * 3.0,
                    );
                } else if let (Some(target_data), Some(bounds)) = (&target_data, bounds) {
                    let target_bounds = target_data.target_bounds();
                    if bounds != target_bounds {
                        let center = s.size().center();
                        helper.draw_local_visualizations(
                            s,
                            animation_color,
                            target_bounds.top_left(),
                            target_data.size(),
                            bounds,
                            center,
                            config.is_show_key_label_enabled,
                            stroke_weight,
                            &key,
                            text_measurer,
                        );
                    } else {
                        inactive(s);
                    }
                }
            } else if !shared_element.found_match() {
                helper.draw_unmatched_element(
                    s,
                    config.unmatched_element_color,
                    config.is_show_key_label_enabled,
                    &key,
                    text_measurer.expect("its text measurer"),
                    stroke_weight,
                );
            } else {
                inactive(s);
            }
        };
        match layer {
            Some(layer) => {
                let size = layer_size(scope);
                scope.record_with(layer, size, |s| {
                    s.draw_content();
                    draw_debug(s);
                });
            }
            None => {
                scope.draw_content();
                draw_debug(scope);
            }
        }
    }

    /// The text measurer for the key, made again with another font resolver (Compose's `updateTextMeasurer`).
    fn update_text_measurer(&mut self, font_family_resolver: Rc<dyn FontFamilyResolver>) {
        let stale = self.current_resolver.as_ref().is_none_or(|r| !Rc::ptr_eq(r, &font_family_resolver));
        if self.text_measurer.is_none() || stale {
            self.text_measurer = Some(TextMeasurer::new(
                font_family_resolver.clone(),
                self.current_density.unwrap_or_default(),
                self.current_layout_direction.unwrap_or(LayoutDirection::Ltr),
                kompoz_text::DefaultCacheSize,
            ));
            self.current_resolver = Some(font_family_resolver);
        }
    }
}

/// A content draw scope's size in whole pixels (Compose's default size of `GraphicsLayer.record`).
fn layer_size(scope: &ContentDrawScope) -> IntSize {
    let size = scope.size();
    IntSize::new(size.width.round() as i32, size.height.round() as i32)
}

impl ModifierNode for SharedBoundsNode {
    node_capabilities!(layout, approach_layout, modifier_local, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.state.handle.borrow_mut() = Some(cx.handle());
        let shared_element = self.state.shared_element();
        cx.observe_reads(|| shared_element.observing_visibility_change());
        self.setup(cx);
        self.state.entry().is_attached.set(true);
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        let state = &self.state;
        *state.resolved_transform_state.borrow_mut() = None;
        // (the scope's root never placed when it goes: no last bounds)
        if let Some(root) = state.shared_element().scope().nullable_root() {
            let placed_coordinates =
                state.approach_coordinates().filter(|_| root.is_attached() && state.is_placed.get());
            // (its position in the root (cached positions), else none: the scope is gone, the shared element no
            // longer valid)
            state.bounds_before_detached.set(placed_coordinates.map(|approach| {
                Rect::from_offset_size(approach.position_in_root() - root.position_in_root(), approach.size().to_size())
            }));
        }
        state.set_layer(None);
        let entry = state.entry();
        entry.set_parent_state(None);
        entry.set_bounds_provider(None);
        entry.is_attached.set(false);
        state.is_placed.set(false);
    }

    fn on_reset(&mut self) {
        self.state.bounds_before_detached.set(None);
        self.state.set_layer(None);
    }
}

impl ModifierLocalModifierNode for SharedBoundsNode {
    fn provided_values(&self) -> &ModifierLocalMap {
        &self.provided_values
    }
}

impl ObserverModifierNode for SharedBoundsNode {
    fn on_observed_reads_changed(&mut self) {
        let shared_element = self.state.shared_element();
        shared_element.update_match();
        if let Some(handle) = self.state.handle() {
            handle.observe_reads(|| shared_element.observing_visibility_change());
        }
    }
}

impl LayoutModifierNode for SharedBoundsNode {
    /// The lookahead pass: its lookahead size and coordinates recorded.
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let state = self.state.clone();
        scope.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
            p.place(&placeable, 0, 0);
            // (after its child is placed: the child's placement may move the bounds)
            let entry = state.entry();
            entry.shared_element().on_lookahead_placed(p, &entry);
        })
    }
}

impl ApproachLayoutModifierNode for SharedBoundsNode {
    fn is_measurement_approach_in_progress(&mut self, _lookahead_size: IntSize) -> bool {
        let entry = self.state.entry();
        let shared_element = entry.shared_element();
        entry.is_enabled() && shared_element.found_match() && shared_element.scope().is_transition_active()
    }

    fn approach_measure(
        &mut self,
        scope: &ApproachMeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        self.state.update_deferred_handoff_values();
        let shared_element = self.state.shared_element();
        let bounds_animation = self.state.bounds_animation();
        // (measured with the current bounds while no animation runs, or before it starts: once a match is found,
        // all of them are measured with the constraints of those bounds, a transition active or not)
        let bounds = self
            .state
            .forced_handoff_bounds
            .get()
            .or_else(|| bounds_animation.value())
            .or_else(|| shared_element.try_initializing_current_bounds());
        let resolved_constraints = match bounds {
            Some(bounds) => {
                let (width, height) = (fast_round_to_int(bounds.width()), fast_round_to_int(bounds.height()));
                assert!(
                    width != Constraints::Infinity && height != Constraints::Infinity,
                    "Error: Infinite width/height is invalid. animated bounds: {:?}, current bounds: {:?}",
                    bounds_animation.value(),
                    shared_element.state().current_bounds()
                );
                Constraints::fixed(width.max(0), height.max(0))
            }
            None => constraints,
        };
        let placeable = measurable.measure(resolved_constraints);
        self.approach_place(scope, placeable)
    }
}

impl DrawModifierNode for SharedBoundsDrawNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let entry = self.entry.clone();
        let bounds = entry.shared_element().state().current_bounds();
        if !entry.should_render_in_overlay() {
            *entry.clip_path_in_overlay.borrow_mut() = None;
            entry.replace_layer(None);
            if entry.should_render_in_place() {
                self.draw_content_with_optional_debug(scope, None, bounds);
            }
            return;
        }
        let clip_path = bounds.and_then(|b| {
            entry.overlay_clip().get_clip_path(&entry.user_state(), b, scope.layout_direction(), scope.density())
        });
        *entry.clip_path_in_overlay.borrow_mut() = clip_path;
        if entry.layer().is_none() {
            let platform = self.handle.as_ref().map(|h| h.current_value_of(&LocalPlatform));
            entry.replace_layer(Some(GraphicsLayer::new(platform)));
        }
        let layer = entry.layer().expect("Error: shared element does not have a layer for rendering in the overlay.");
        self.draw_content_with_optional_debug(scope, Some(&layer), bounds);
        if entry.should_render_in_place() {
            scope.draw_layer(&layer);
        }
    }
}

/// The point of the parent's transform origin in the root, its bounds' (Compose's internal `calculatePivot`): a
/// shared element drawn in the overlay scales about it with its parent container, staying where it is in it.
pub(crate) fn calculate_pivot(
    parent_coordinates: &LayoutCoordinates,
    root_coordinates: &LayoutCoordinates,
    transform_origin: TransformOrigin,
) -> Offset {
    let parent_bounds_in_root = root_coordinates.local_bounding_box_of(parent_coordinates, false);
    Offset::new(
        parent_bounds_in_root.left + parent_bounds_in_root.width() * transform_origin.pivot_fraction_x,
        parent_bounds_in_root.top + parent_bounds_in_root.height() * transform_origin.pivot_fraction_y,
    )
}

/// A point scaled about `pivot` and moved by `offset` (Compose's internal `Offset.transform`).
pub(crate) fn transform(point: Offset, pivot: Offset, scale: f32, offset: IntOffset) -> Offset {
    Offset::new(
        (point.x - pivot.x) * scale + pivot.x + offset.x as f32,
        (point.y - pivot.y) * scale + pivot.y + offset.y as f32,
    )
}
