//! Item animations (Compose's LazyLayoutItemAnimator.kt, LazyLayoutItemAnimation.kt,
//! LazyLayoutAnimateItemModifier.kt and LazyLayoutItemAnimatorModifier.kt): an item with `Modifier.animate_item`
//! glides from where it was to where it is when items around it change (an `Animatable` of its offset delta), fades
//! in when it appears, and fades out where it was when it goes — drawn by the layout through the graphics layer it
//! was placed with, after its composition is gone. A scroll isn't animated.

use super::item_provider::LazyLayoutKeyIndexMap;
use super::key::LazyKey;
use super::measured_item::{LazyLayoutMeasuredItem, LazyLayoutMeasuredItemProvider};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, AnimationVector2D, FiniteAnimationSpec, Spring, SpringSpec,
    VectorConverter, VisibilityThreshold,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, IntOffset, IntSize};
use kompoz_runtime::{MutableState, mutable_state_of, named_args};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, GraphicsLayer};
use kompoz_ui::layout::ParentDataModifierNode;
use kompoz_ui::platform::PlatformHandle;
use kompoz_ui::{Constraints, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// The specs an item gives (Modifier.animate_item)

/// The specs of an item's animations, as `Modifier.animate_item` tells them to the layout (Compose's
/// LazyLayoutAnimationSpecsNode, its parent data).
#[derive(Clone)]
pub(crate) struct LazyLayoutAnimationSpecs {
    pub(crate) fade_in_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
    pub(crate) placement_spec: Option<Arc<dyn FiniteAnimationSpec<IntOffset>>>,
    pub(crate) fade_out_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
}

fn same_spec<T: 'static>(
    a: &Option<Arc<dyn FiniteAnimationSpec<T>>>,
    b: &Option<Arc<dyn FiniteAnimationSpec<T>>>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => **a == **b,
        (None, None) => true,
        _ => false,
    }
}

/// `Modifier.animate_item`'s element (Compose's LazyLayoutAnimateItemElement).
pub(crate) struct LazyLayoutAnimateItemElement(pub(crate) LazyLayoutAnimationSpecs);

impl PartialEq for LazyLayoutAnimateItemElement {
    fn eq(&self, other: &Self) -> bool {
        same_spec(&self.0.fade_in_spec, &other.0.fade_in_spec)
            && same_spec(&self.0.placement_spec, &other.0.placement_spec)
            && same_spec(&self.0.fade_out_spec, &other.0.fade_out_spec)
    }
}

impl ModifierNodeElement for LazyLayoutAnimateItemElement {
    type Node = LazyLayoutAnimationSpecsNode;
    fn create(&self) -> LazyLayoutAnimationSpecsNode {
        LazyLayoutAnimationSpecsNode { specs: self.0.clone() }
    }
    fn update(&self, node: &mut LazyLayoutAnimationSpecsNode) {
        node.specs = self.0.clone();
    }
    fn name(&self) -> &'static str {
        "animateItem"
    }
}

pub(crate) struct LazyLayoutAnimationSpecsNode {
    specs: LazyLayoutAnimationSpecs,
}

impl ModifierNode for LazyLayoutAnimationSpecsNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for LazyLayoutAnimationSpecsNode {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(self.specs.clone()))
    }
}

/// The animation specs an item's placeable tells, if any.
fn specs_of(parent_data: Option<Rc<dyn Any>>) -> Option<LazyLayoutAnimationSpecs> {
    parent_data.and_then(|d| d.downcast_ref::<LazyLayoutAnimationSpecs>().cloned())
}

// ---------------------------------------------------------------------------------------------------------------
// One placeable's animations

/// The spring an interrupted placement animation goes on with (unless its spec is a spring).
fn interruption_spec() -> Arc<dyn FiniteAnimationSpec<IntOffset>> {
    Arc::new(SpringSpec::new(
        Spring::DampingRatioNoBouncy,
        Spring::StiffnessMediumLow,
        Some(IntOffset::VisibilityThreshold),
    ))
}

/// The animations of one placeable of an item (Compose's LazyLayoutItemAnimation): its placement delta, its fade
/// in and out (the alpha of the layer it is placed with).
pub(crate) struct LazyLayoutItemAnimation {
    coroutine_scope: CoroutineScope,
    platform: Option<PlatformHandle>,
    on_layer_property_changed: Rc<dyn Fn()>,
    pub(crate) fade_in_spec: RefCell<Option<Arc<dyn FiniteAnimationSpec<f32>>>>,
    pub(crate) placement_spec: RefCell<Option<Arc<dyn FiniteAnimationSpec<IntOffset>>>>,
    pub(crate) fade_out_spec: RefCell<Option<Arc<dyn FiniteAnimationSpec<f32>>>>,
    is_running_moving_away_animation: Cell<bool>,
    is_placement_animation_in_progress: MutableState<bool>,
    is_appearance_animation_in_progress: MutableState<bool>,
    is_disappearance_animation_in_progress: MutableState<bool>,
    is_disappearance_animation_finished: MutableState<bool>,
    /// Where it is going, from the layout's content start (NotInitialized before it is known).
    pub(crate) target_offset: Cell<IntOffset>,
    /// Where it was placed last (where a disappearing item is drawn).
    pub(crate) placement_offset: Cell<IntOffset>,
    /// The lookahead pass's offset: where the approach animates the item to (in a lookahead scope).
    pub(crate) lookahead_offset: Cell<IntOffset>,
    pub(crate) layer: RefCell<Option<GraphicsLayer>>,
    placement_delta_animation: Animatable<IntOffset, AnimationVector2D>,
    visibility_animation: Animatable<f32, AnimationVector1D>,
    placement_delta: MutableState<IntOffset>,
    me: Weak<LazyLayoutItemAnimation>,
}

impl LazyLayoutItemAnimation {
    pub(crate) const NotInitialized: IntOffset = IntOffset::new(i32::MAX, i32::MAX);

    fn new(
        coroutine_scope: CoroutineScope,
        platform: Option<PlatformHandle>,
        on_layer_property_changed: Rc<dyn Fn()>,
    ) -> Rc<LazyLayoutItemAnimation> {
        let layer = platform.as_ref().map(|p| GraphicsLayer::new(Some(p.clone())));
        Rc::new_cyclic(|me| LazyLayoutItemAnimation {
            coroutine_scope,
            platform,
            on_layer_property_changed,
            fade_in_spec: RefCell::new(None),
            placement_spec: RefCell::new(None),
            fade_out_spec: RefCell::new(None),
            is_running_moving_away_animation: Cell::new(false),
            is_placement_animation_in_progress: mutable_state_of(false),
            is_appearance_animation_in_progress: mutable_state_of(false),
            is_disappearance_animation_in_progress: mutable_state_of(false),
            is_disappearance_animation_finished: mutable_state_of(false),
            target_offset: Cell::new(Self::NotInitialized),
            placement_offset: Cell::new(IntOffset::Zero),
            lookahead_offset: Cell::new(Self::NotInitialized),
            layer: RefCell::new(layer),
            placement_delta_animation: Animatable((IntOffset::Zero, IntOffset::VectorConverter)),
            visibility_animation: Animatable(1.0f32),
            placement_delta: mutable_state_of(IntOffset::Zero),
            me: me.clone(),
        })
    }

    pub(crate) fn is_placement_animation_in_progress(&self) -> bool {
        self.is_placement_animation_in_progress.get()
    }
    pub(crate) fn is_disappearance_animation_in_progress(&self) -> bool {
        self.is_disappearance_animation_in_progress.get()
    }
    pub(crate) fn is_disappearance_animation_finished(&self) -> bool {
        self.is_disappearance_animation_finished.get()
    }
    pub(crate) fn is_running_moving_away_animation(&self) -> bool {
        self.is_running_moving_away_animation.get()
    }
    /// How far from its target it is placed now (a state: placing reads it).
    pub(crate) fn placement_delta(&self) -> IntOffset {
        self.placement_delta.get()
    }
    pub(crate) fn layer(&self) -> Option<GraphicsLayer> {
        self.layer.borrow().clone()
    }

    pub(crate) fn apply_scroll_offset(&self, offset: IntOffset) {
        self.target_offset.set(self.target_offset.get() + offset);
    }

    /// Stops the placement animation where its delta is 0 (Compose's `cancelPlacementAnimation`).
    pub(crate) fn cancel_placement_animation(&self) {
        if self.is_placement_animation_in_progress.peek() {
            let me = self.me.clone();
            self.coroutine_scope.launch(async move {
                let Some(me) = me.upgrade() else { return };
                me.placement_delta_animation.snap_to(IntOffset::Zero).await;
                me.placement_delta.set(IntOffset::Zero);
                me.is_placement_animation_in_progress.set(false);
            });
        }
    }

    /// Animates from `delta` away from its target to it (Compose's `animatePlacementDelta`): an animation running
    /// goes on from where it is, with a spring.
    pub(crate) fn animate_placement_delta(&self, delta: IntOffset, is_moving_away: bool) {
        let Some(spec) = self.placement_spec.borrow().clone() else { return };
        let total_delta = self.placement_delta.peek() - delta;
        self.placement_delta.set(total_delta);
        self.is_placement_animation_in_progress.set(true);
        self.is_running_moving_away_animation.set(is_moving_away);
        let me = self.me.clone();
        self.coroutine_scope.launch(async move {
            let Some(me) = me.upgrade() else { return };
            let running = me.placement_delta_animation.is_running();
            let final_spec: Arc<dyn FiniteAnimationSpec<IntOffset>> = if running {
                // (interrupted: the default spring, unless the spec is a spring)
                if spec.as_any().is::<SpringSpec<IntOffset>>() { spec } else { interruption_spec() }
            } else {
                spec
            };
            if !running {
                // (from the whole delta to zero)
                me.placement_delta_animation.snap_to(total_delta).await;
                (me.on_layer_property_changed)();
            }
            // Its target is zero unless one runs: then on from where it is, the whole delta still to go.
            let animation_target = me.placement_delta_animation.value() - total_delta;
            let weak = me.me.clone();
            let block: Box<dyn for<'a> FnMut(&'a Animatable<IntOffset, AnimationVector2D>)> = Box::new(move |a| {
                if let Some(me) = weak.upgrade() {
                    me.placement_delta.set(a.value() - animation_target);
                    (me.on_layer_property_changed)();
                }
            });
            let spec: Arc<dyn AnimationSpec<IntOffset>> = final_spec;
            let _ = named_args!(me.placement_delta_animation.animate_to(
                animation_target,
                animation_spec = spec,
                block = Some(block)
            ))
            .await;
            me.is_placement_animation_in_progress.set(false);
            me.is_running_moving_away_animation.set(false);
        });
    }

    /// Fades it in from 0 (or from where a fade out left it) (Compose's `animateAppearance`).
    pub(crate) fn animate_appearance(&self) {
        let layer = self.layer();
        let spec = self.fade_in_spec.borrow().clone();
        let (Some(layer), Some(spec), false) = (layer.clone(), spec, self.is_appearance_animation_in_progress.peek())
        else {
            if self.is_disappearance_animation_in_progress.peek() {
                // (appearing again while fading out, without a fade in: at once)
                if let Some(layer) = &layer {
                    layer.set_alpha(1.0);
                }
                let me = self.me.clone();
                self.coroutine_scope.launch(async move {
                    if let Some(me) = me.upgrade() {
                        me.visibility_animation.snap_to(1.0).await;
                    }
                });
            }
            return;
        };
        self.is_appearance_animation_in_progress.set(true);
        let should_reset_value = !self.is_disappearance_animation_in_progress.peek();
        if should_reset_value {
            layer.set_alpha(0.0);
        }
        let me = self.me.clone();
        self.coroutine_scope.launch(async move {
            let Some(me) = me.upgrade() else { return };
            let _done = InProgressGuard(me.is_appearance_animation_in_progress.clone());
            if should_reset_value {
                me.visibility_animation.snap_to(0.0).await;
            }
            let weak = me.me.clone();
            let block: Box<dyn for<'a> FnMut(&'a Animatable<f32, AnimationVector1D>)> = Box::new(move |a| {
                layer.set_alpha(a.value());
                if let Some(me) = weak.upgrade() {
                    (me.on_layer_property_changed)();
                }
            });
            let spec: Arc<dyn AnimationSpec<f32>> = spec;
            let _ =
                named_args!(me.visibility_animation.animate_to(1.0, animation_spec = spec, block = Some(block))).await;
        });
    }

    /// Fades it out (Compose's `animateDisappearance`); finished, the layout lets it go.
    pub(crate) fn animate_disappearance(&self) {
        let (Some(layer), Some(spec)) = (self.layer(), self.fade_out_spec.borrow().clone()) else { return };
        if self.is_disappearance_animation_in_progress.peek() {
            return;
        }
        self.is_disappearance_animation_in_progress.set(true);
        let me = self.me.clone();
        self.coroutine_scope.launch(async move {
            let Some(me) = me.upgrade() else { return };
            let _done = InProgressGuard(me.is_disappearance_animation_in_progress.clone());
            let weak = me.me.clone();
            let block: Box<dyn for<'a> FnMut(&'a Animatable<f32, AnimationVector1D>)> = Box::new(move |a| {
                layer.set_alpha(a.value());
                if let Some(me) = weak.upgrade() {
                    (me.on_layer_property_changed)();
                }
            });
            let spec: Arc<dyn AnimationSpec<f32>> = spec;
            let _ =
                named_args!(me.visibility_animation.animate_to(0.0, animation_spec = spec, block = Some(block))).await;
            me.is_disappearance_animation_finished.set(true);
        });
    }

    /// Stops its animations and lets its layer go (Compose's `release`).
    pub(crate) fn release(&self) {
        if self.is_placement_animation_in_progress.peek() {
            self.is_placement_animation_in_progress.set(false);
            let me = self.me.clone();
            self.coroutine_scope.launch(async move {
                if let Some(me) = me.upgrade() {
                    me.placement_delta_animation.stop().await;
                }
            });
        }
        if self.is_appearance_animation_in_progress.peek() {
            self.is_appearance_animation_in_progress.set(false);
            let me = self.me.clone();
            self.coroutine_scope.launch(async move {
                if let Some(me) = me.upgrade() {
                    me.visibility_animation.stop().await;
                }
            });
        }
        if self.is_disappearance_animation_in_progress.peek() {
            self.is_disappearance_animation_in_progress.set(false);
            let me = self.me.clone();
            self.coroutine_scope.launch(async move {
                if let Some(me) = me.upgrade() {
                    me.visibility_animation.stop().await;
                }
            });
        }
        self.is_running_moving_away_animation.set(false);
        self.placement_delta.set(IntOffset::Zero);
        self.target_offset.set(Self::NotInitialized);
        if let Some(layer) = self.layer.borrow_mut().take() {
            layer.release();
        }
        let _ = &self.platform;
        *self.fade_in_spec.borrow_mut() = None;
        *self.fade_out_spec.borrow_mut() = None;
        *self.placement_spec.borrow_mut() = None;
    }
}

/// Puts an "in progress" state back to false when the animation's coroutine ends however (Kotlin's `finally`).
struct InProgressGuard(MutableState<bool>);

impl Drop for InProgressGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The animator

/// What the animator knows of an item it animates (Compose's ItemInfo): its placeables' animations and where it
/// was laid out.
struct ItemInfo {
    animations: Vec<Option<Rc<LazyLayoutItemAnimation>>>,
    constraints: Option<Constraints>,
    cross_axis_offset: i32,
    lane: i32,
    span: i32,
    layout_min_offset: i32,
    layout_max_offset: i32,
}

impl ItemInfo {
    fn new() -> ItemInfo {
        ItemInfo {
            animations: Vec::new(),
            constraints: None,
            cross_axis_offset: 0,
            lane: 0,
            span: 1,
            layout_min_offset: 0,
            layout_max_offset: 0,
        }
    }

    fn is_running_placement(&self) -> bool {
        self.animations.iter().any(|a| a.as_ref().is_some_and(|a| a.is_running_moving_away_animation()))
    }

    /// Takes the item's specs (Compose's `updateAnimation`): an animation for each placeable with specs.
    #[allow(clippy::too_many_arguments)]
    fn update_animation<T: LazyLayoutMeasuredItem>(
        &mut self,
        positioned_item: &T,
        coroutine_scope: &CoroutineScope,
        platform: &Option<PlatformHandle>,
        on_layer_property_changed: &Rc<dyn Fn()>,
        layout_min_offset: i32,
        layout_max_offset: i32,
        is_vertical: bool,
        cross_axis_offset: Option<i32>,
    ) {
        if !self.is_running_placement() {
            self.layout_min_offset = layout_min_offset;
            self.layout_max_offset = layout_max_offset;
        }
        let count = positioned_item.placeables_count();
        for a in self.animations.iter().skip(count).flatten() {
            a.release();
        }
        self.animations.resize(count, None);
        self.constraints = Some(positioned_item.constraints());
        self.cross_axis_offset =
            cross_axis_offset.unwrap_or_else(|| cross_axis_offset_of(positioned_item, is_vertical));
        self.lane = positioned_item.lane();
        self.span = positioned_item.span();
        for (index, slot) in self.animations.iter_mut().enumerate().take(count) {
            match specs_of(positioned_item.get_parent_data(index)) {
                None => {
                    if let Some(a) = slot.take() {
                        a.release();
                    }
                }
                Some(specs) => {
                    let animation = slot
                        .get_or_insert_with(|| {
                            LazyLayoutItemAnimation::new(
                                coroutine_scope.clone(),
                                platform.clone(),
                                on_layer_property_changed.clone(),
                            )
                        })
                        .clone();
                    *animation.fade_in_spec.borrow_mut() = specs.fade_in_spec;
                    *animation.placement_spec.borrow_mut() = specs.placement_spec;
                    *animation.fade_out_spec.borrow_mut() = specs.fade_out_spec;
                }
            }
        }
    }
}

fn main_axis_offset_of<T: LazyLayoutMeasuredItem>(item: &T, is_vertical: bool) -> i32 {
    let o = item.get_offset(0);
    if is_vertical { o.y } else { o.x }
}

fn cross_axis_offset_of<T: LazyLayoutMeasuredItem>(item: &T, is_vertical: bool) -> i32 {
    let o = item.get_offset(0);
    if !is_vertical { o.y } else { o.x }
}

fn has_animations<T: LazyLayoutMeasuredItem>(item: &T) -> bool {
    (0..item.placeables_count()).any(|i| specs_of(item.get_parent_data(i)).is_some())
}

/// Animates a lazy layout's items as they move, come and go (Compose's LazyLayoutItemAnimator): told each measure
/// what was positioned, it starts placement animations for items whose offset changed other than by the scroll,
/// keeps items that moved out of view placed (measured again) while they glide out, fades in new items and fades
/// out removed ones, which the layout draws ([`lazy_layout_item_animator`]).
pub(crate) struct LazyLayoutItemAnimator<T> {
    key_to_item_info_map: RefCell<HashMap<LazyKey, ItemInfo>>,
    /// The key index map of the last measure.
    key_index_map: RefCell<Option<Rc<dyn LazyLayoutKeyIndexMap>>>,
    first_visible_index: Cell<i32>,
    disappearing_items: RefCell<Vec<Rc<LazyLayoutItemAnimation>>>,
    /// The layout's node drawing the disappearing items.
    pub(crate) displaying_node: RefCell<Option<NodeHandle>>,
    _items: std::marker::PhantomData<T>,
}

impl<T> Default for LazyLayoutItemAnimator<T> {
    fn default() -> Self {
        LazyLayoutItemAnimator {
            key_to_item_info_map: RefCell::new(HashMap::new()),
            key_index_map: RefCell::new(None),
            first_visible_index: Cell::new(0),
            disappearing_items: RefCell::new(Vec::new()),
            displaying_node: RefCell::new(None),
            _items: std::marker::PhantomData,
        }
    }
}

impl<T: LazyLayoutMeasuredItem + 'static> LazyLayoutItemAnimator<T> {
    fn invalidate_displaying_node(&self) {
        if let Some(n) = self.displaying_node.borrow().as_ref() {
            n.invalidate_draw();
        }
    }

    /// What a layer's property changing does: the layout drawing the disappearing items draws again.
    fn on_layer_property_changed(self: &Rc<Self>) -> Rc<dyn Fn()> {
        let weak = Rc::downgrade(self);
        Rc::new(move || {
            if let Some(a) = weak.upgrade() {
                a.invalidate_displaying_node();
            }
        })
    }

    /// Takes a measure's positioned items (Compose's `onMeasured`): see the type's doc. Items moving away are added
    /// to `positioned_items`, before or after the others.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn on_measured(
        self: &Rc<Self>,
        consumed_scroll: i32,
        layout_width: i32,
        layout_height: i32,
        positioned_items: &mut Vec<Rc<T>>,
        key_index_map: Rc<dyn LazyLayoutKeyIndexMap>,
        item_provider: &dyn LazyLayoutMeasuredItemProvider<T>,
        is_vertical: bool,
        is_looking_ahead: bool,
        lane_count: i32,
        has_lookahead_occurred: bool,
        layout_min_offset: i32,
        layout_max_offset: i32,
        coroutine_scope: &CoroutineScope,
        platform: &Option<PlatformHandle>,
    ) {
        let previous_key_to_index_map = self.key_index_map.replace(Some(key_index_map.clone()));

        let has_any_animations = positioned_items.iter().any(|i| has_animations(&**i));
        if !has_any_animations && self.key_to_item_info_map.borrow().is_empty() {
            // (no animations: nothing to do, what was known let go)
            self.release_animations();
            return;
        }

        let previous_first_visible_index = self.first_visible_index.get();
        self.first_visible_index.set(positioned_items.first().map_or(0, |i| i.index()));

        // (the consumed scroll is a delta not to animate)
        let scroll_offset =
            if is_vertical { IntOffset::new(0, consumed_scroll) } else { IntOffset::new(consumed_scroll, 0) };
        // (animations are set up when the pass knows its targets: no lookahead here)
        let should_setup_animation = is_looking_ahead || !has_lookahead_occurred;
        let on_layer_property_changed = self.on_layer_property_changed();

        let mut moving_away_keys: HashSet<LazyKey> = self.key_to_item_info_map.borrow().keys().cloned().collect();
        let mut moving_in_from_start_bound: Vec<Rc<T>> = Vec::new();
        let mut moving_in_from_end_bound: Vec<Rc<T>> = Vec::new();

        for item in positioned_items.iter() {
            let key = item.key();
            moving_away_keys.remove(&key);
            if has_animations(&**item) {
                let previous_index = previous_key_to_index_map.as_ref().map_or(-1, |m| m.get_index(&key));
                let should_animate_appearance = previous_index == -1 && previous_key_to_index_map.is_some();
                let exists = self.key_to_item_info_map.borrow().contains_key(&key);
                if !exists {
                    let mut info = ItemInfo::new();
                    info.update_animation(
                        &**item,
                        coroutine_scope,
                        platform,
                        &on_layer_property_changed,
                        layout_min_offset,
                        layout_max_offset,
                        is_vertical,
                        None,
                    );
                    self.key_to_item_info_map.borrow_mut().insert(key.clone(), info);
                    if item.index() != previous_index && previous_index != -1 {
                        if previous_index < previous_first_visible_index {
                            // (the larger index will be at the start of the list)
                            moving_in_from_start_bound.push(item.clone());
                        } else {
                            moving_in_from_end_bound.push(item.clone());
                        }
                    } else {
                        self.initialize_animation(&**item, main_axis_offset_of(&**item, is_vertical), is_vertical);
                        if should_animate_appearance {
                            for a in self.animations_of(&key).into_iter().flatten() {
                                a.animate_appearance();
                            }
                        }
                    }
                } else if should_setup_animation {
                    {
                        let mut map = self.key_to_item_info_map.borrow_mut();
                        let info = map.get_mut(&key).unwrap();
                        info.update_animation(
                            &**item,
                            coroutine_scope,
                            platform,
                            &on_layer_property_changed,
                            layout_min_offset,
                            layout_max_offset,
                            is_vertical,
                            None,
                        );
                    }
                    for a in self.animations_of(&key).into_iter().flatten() {
                        if a.target_offset.get() != LazyLayoutItemAnimation::NotInitialized {
                            a.apply_scroll_offset(scroll_offset);
                        }
                    }
                    if should_animate_appearance {
                        for a in self.animations_of(&key).into_iter().flatten() {
                            if a.is_disappearance_animation_in_progress() {
                                self.disappearing_items.borrow_mut().retain(|d| !Rc::ptr_eq(d, &a));
                                self.invalidate_displaying_node();
                            }
                            a.animate_appearance();
                        }
                    }
                    self.start_placement_animations_if_needed(&**item, false);
                }
            } else {
                // (no animation: cleaned up)
                self.remove_info_for_key(&key);
            }
        }

        let mut accumulated_offset_per_lane = vec![0i32; lane_count.max(1) as usize];
        if should_setup_animation && let Some(previous) = &previous_key_to_index_map {
            if !moving_in_from_start_bound.is_empty() {
                moving_in_from_start_bound.sort_by_key(|i| std::cmp::Reverse(previous.get_index(&i.key())));
                for item in &moving_in_from_start_bound {
                    let accumulated =
                        update_and_return_offset_for(&mut accumulated_offset_per_lane, &**item, is_vertical);
                    let main_axis_offset = layout_min_offset - accumulated;
                    self.initialize_animation(&**item, main_axis_offset, is_vertical);
                    self.start_placement_animations_if_needed(&**item, false);
                }
                accumulated_offset_per_lane.fill(0);
            }
            if !moving_in_from_end_bound.is_empty() {
                moving_in_from_end_bound.sort_by_key(|i| previous.get_index(&i.key()));
                for item in &moving_in_from_end_bound {
                    let accumulated =
                        update_and_return_offset_for(&mut accumulated_offset_per_lane, &**item, is_vertical);
                    let main_axis_offset =
                        layout_max_offset + accumulated - item.main_axis_size_with_spacings(is_vertical);
                    self.initialize_animation(&**item, main_axis_offset, is_vertical);
                    self.start_placement_animations_if_needed(&**item, false);
                }
                accumulated_offset_per_lane.fill(0);
            }
        }

        let mut moving_away_to_start_bound: Vec<Rc<T>> = Vec::new();
        let mut moving_away_to_end_bound: Vec<Rc<T>> = Vec::new();
        let mut keys: Vec<LazyKey> = moving_away_keys.into_iter().collect();
        // (the order the map had, as far as it matters: by the index they had)
        if let Some(previous) = &previous_key_to_index_map {
            keys.sort_by_key(|k| previous.get_index(k));
        }
        for key in keys {
            // (the map and the keys may disagree: then nothing can be played, the item is gone)
            let new_index = key_index_map.get_index(&key);
            let (constraints, lane, span) = {
                let mut map = self.key_to_item_info_map.borrow_mut();
                let Some(info) = map.get_mut(&key) else { continue };
                // (measured again with fewer lanes: lane and span kept within)
                info.span = lane_count.min(info.span);
                info.lane = (lane_count - info.span).min(info.lane);
                (info.constraints, info.lane, info.span)
            };
            if new_index == -1 {
                let mut is_progress = false;
                let animations = self.animations_of(&key);
                for (index, animation) in animations.into_iter().enumerate() {
                    let Some(animation) = animation else { continue };
                    if animation.is_disappearance_animation_in_progress() {
                        is_progress = true;
                    } else if animation.is_disappearance_animation_finished() {
                        animation.release();
                        if let Some(info) = self.key_to_item_info_map.borrow_mut().get_mut(&key) {
                            info.animations[index] = None;
                        }
                        self.disappearing_items.borrow_mut().retain(|d| !Rc::ptr_eq(d, &animation));
                        self.invalidate_displaying_node();
                    } else {
                        if animation.layer().is_some() {
                            animation.animate_disappearance();
                        }
                        if animation.is_disappearance_animation_in_progress() {
                            self.disappearing_items.borrow_mut().push(animation.clone());
                            self.invalidate_displaying_node();
                            is_progress = true;
                        } else {
                            animation.release();
                            if let Some(info) = self.key_to_item_info_map.borrow_mut().get_mut(&key) {
                                info.animations[index] = None;
                            }
                        }
                    }
                }
                if !is_progress {
                    self.remove_info_for_key(&key);
                }
            } else {
                let Some(constraints) = constraints else { continue };
                let item = item_provider.get_and_measure(new_index, lane, span, constraints);
                item.make_non_scrollable();
                // (any placement animation running on it?)
                let in_progress =
                    self.animations_of(&key).iter().flatten().any(|a| a.is_placement_animation_in_progress());
                let previous_index = previous_key_to_index_map.as_ref().map(|m| m.get_index(&key));
                if !in_progress && Some(new_index) == previous_index {
                    self.remove_info_for_key(&key);
                } else {
                    // (an item composed again and used: its info updated)
                    {
                        let mut map = self.key_to_item_info_map.borrow_mut();
                        let info = map.get_mut(&key).unwrap();
                        let cross = info.cross_axis_offset;
                        info.update_animation(
                            &*item,
                            coroutine_scope,
                            platform,
                            &on_layer_property_changed,
                            layout_min_offset,
                            layout_max_offset,
                            is_vertical,
                            Some(cross),
                        );
                    }
                    if new_index < self.first_visible_index.get() {
                        moving_away_to_start_bound.push(item);
                    } else {
                        moving_away_to_end_bound.push(item);
                    }
                }
            }
        }

        if !moving_away_to_start_bound.is_empty() {
            moving_away_to_start_bound.sort_by_key(|i| std::cmp::Reverse(key_index_map.get_index(&i.key())));
            for item in &moving_away_to_start_bound {
                let (layout_min, cross) = {
                    let map = self.key_to_item_info_map.borrow();
                    let info = &map[&item.key()];
                    (info.layout_min_offset, info.cross_axis_offset)
                };
                let accumulated = update_and_return_offset_for(&mut accumulated_offset_per_lane, &**item, is_vertical);
                let main_axis_offset =
                    if is_looking_ahead { main_axis_offset_of(&*positioned_items[0], is_vertical) } else { layout_min }
                        - accumulated;
                item.position(main_axis_offset, cross, layout_width, layout_height);
                if should_setup_animation {
                    self.start_placement_animations_if_needed(&**item, true);
                }
            }
            accumulated_offset_per_lane.fill(0);
        }

        if !moving_away_to_end_bound.is_empty() {
            moving_away_to_end_bound.sort_by_key(|i| key_index_map.get_index(&i.key()));
            for item in &moving_away_to_end_bound {
                let (layout_max, cross) = {
                    let map = self.key_to_item_info_map.borrow();
                    let info = &map[&item.key()];
                    (info.layout_max_offset, info.cross_axis_offset)
                };
                let accumulated = update_and_return_offset_for(&mut accumulated_offset_per_lane, &**item, is_vertical);
                let main_axis_offset = layout_max - item.main_axis_size_with_spacings(is_vertical) + accumulated;
                item.position(main_axis_offset, cross, layout_width, layout_height);
                if should_setup_animation {
                    self.start_placement_animations_if_needed(&**item, true);
                }
            }
        }

        // (added keeping the positioned items sorted by index)
        moving_away_to_start_bound.reverse();
        let mut items = moving_away_to_start_bound;
        items.append(positioned_items);
        items.append(&mut moving_away_to_end_bound);
        *positioned_items = items;
    }

    fn animations_of(&self, key: &LazyKey) -> Vec<Option<Rc<LazyLayoutItemAnimation>>> {
        self.key_to_item_info_map.borrow().get(key).map(|i| i.animations.clone()).unwrap_or_default()
    }

    fn remove_info_for_key(&self, key: &LazyKey) {
        let removed = self.key_to_item_info_map.borrow_mut().remove(key);
        if let Some(info) = removed {
            for a in info.animations.into_iter().flatten() {
                a.release();
            }
        }
    }

    /// Forgets everything (Compose's `reset`): a jump isn't animated.
    pub(crate) fn reset(&self) {
        self.release_animations();
        *self.key_index_map.borrow_mut() = None;
        self.first_visible_index.set(-1);
    }

    fn release_animations(&self) {
        let infos: Vec<ItemInfo> = self.key_to_item_info_map.borrow_mut().drain().map(|(_, v)| v).collect();
        for info in infos {
            for a in info.animations.into_iter().flatten() {
                a.release();
            }
        }
    }

    /// Its placeables' targets put at `main_axis_offset` (their offsets from the first kept).
    fn initialize_animation(&self, item: &T, main_axis_offset: i32, is_vertical: bool) {
        let first_placeable_offset = item.get_offset(0);
        let target_first_placeable_offset = if is_vertical {
            IntOffset::new(first_placeable_offset.x, main_axis_offset)
        } else {
            IntOffset::new(main_axis_offset, first_placeable_offset.y)
        };
        for (placeable_index, animation) in self.animations_of(&item.key()).into_iter().enumerate() {
            if let Some(animation) = animation {
                let diff = item.get_offset(placeable_index) - first_placeable_offset;
                animation.target_offset.set(target_first_placeable_offset + diff);
            }
        }
    }

    /// Animates each placeable whose target moved from where it was going (Compose's
    /// `startPlacementAnimationsIfNeeded`).
    fn start_placement_animations_if_needed(&self, item: &T, is_moving_away: bool) {
        for (placeable_index, animation) in self.animations_of(&item.key()).into_iter().enumerate() {
            if let Some(animation) = animation {
                let new_target = item.get_offset(placeable_index);
                let current_target = animation.target_offset.get();
                if current_target != LazyLayoutItemAnimation::NotInitialized && current_target != new_target {
                    animation.animate_placement_delta(new_target - current_target, is_moving_away);
                }
                animation.target_offset.set(new_target);
            }
        }
    }

    /// The animation of the item of `key`'s placeable `placeable_index`, if it has one.
    pub(crate) fn get_animation(&self, key: &LazyKey, placeable_index: usize) -> Option<Rc<LazyLayoutItemAnimation>> {
        self.key_to_item_info_map.borrow().get(key).and_then(|i| i.animations.get(placeable_index).cloned().flatten())
    }

    /// How big the layout has to be to show the items fading out where they are (Compose's
    /// `minSizeToFitDisappearingItems`).
    pub(crate) fn min_size_to_fit_disappearing_items(&self) -> IntSize {
        let mut size = IntSize::Zero;
        for a in self.disappearing_items.borrow().iter() {
            if let Some(layer) = a.layer() {
                let target = a.target_offset.get();
                size = IntSize::new(
                    size.width.max(target.x + layer.size().width),
                    size.height.max(target.y + layer.size().height),
                );
            }
        }
        size
    }
}

/// The offset taken so far in the lanes `item` spans, after adding it (Compose's `updateAndReturnOffsetFor`).
fn update_and_return_offset_for<T: LazyLayoutMeasuredItem>(offsets: &mut [i32], item: &T, is_vertical: bool) -> i32 {
    let lane = item.lane().max(0) as usize;
    let span = item.span().max(1) as usize;
    let mut max_offset = 0;
    for o in offsets.iter_mut().skip(lane).take(span) {
        *o += item.main_axis_size_with_spacings(is_vertical);
        max_offset = max_offset.max(*o);
    }
    max_offset
}

/// What the layout's draw node needs of its animator, whatever its items.
pub(crate) trait ItemAnimatorHandle {
    fn set_displaying_node(&self, node: Option<NodeHandle>);
    fn reset(&self);
    /// Draws the items fading out, each at where it was placed last (Compose's `onDrawDisappearingItems`).
    fn draw_disappearing_items(&self, scope: &mut ContentDrawScope);
}

impl<T: LazyLayoutMeasuredItem + 'static> ItemAnimatorHandle for LazyLayoutItemAnimator<T> {
    fn set_displaying_node(&self, node: Option<NodeHandle>) {
        *self.displaying_node.borrow_mut() = node;
    }
    fn reset(&self) {
        LazyLayoutItemAnimator::reset(self);
    }
    fn draw_disappearing_items(&self, scope: &mut ContentDrawScope) {
        let items = self.disappearing_items.borrow().clone();
        for item in items {
            let Some(layer) = item.layer() else { continue };
            let at = item.placement_offset.get();
            let top_left = layer.top_left();
            scope.translate_with((at.x - top_left.x) as f32, (at.y - top_left.y) as f32, |d| d.draw_layer(&layer));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The layout's node drawing the disappearing items

/// The layout draws its animator's items fading out, under its content (Compose's `lazyLayoutItemAnimator`).
pub(crate) fn lazy_layout_item_animator(modifier: Modifier, animator: Rc<dyn ItemAnimatorHandle>) -> Modifier {
    modifier.then_element(DisplayingDisappearingItemsElement(animator))
}

struct DisplayingDisappearingItemsElement(Rc<dyn ItemAnimatorHandle>);

impl PartialEq for DisplayingDisappearingItemsElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNodeElement for DisplayingDisappearingItemsElement {
    type Node = DisplayingDisappearingItemsNode;
    fn create(&self) -> DisplayingDisappearingItemsNode {
        DisplayingDisappearingItemsNode { animator: self.0.clone(), cx: None }
    }
    fn update(&self, node: &mut DisplayingDisappearingItemsNode) {
        if !Rc::ptr_eq(&node.animator, &self.0)
            && let Some(cx) = &node.cx
        {
            node.animator.reset();
            self.0.set_displaying_node(Some(cx.handle()));
            node.animator = self.0.clone();
        }
    }
    fn name(&self) -> &'static str {
        "DisplayingDisappearingItemsElement"
    }
}

struct DisplayingDisappearingItemsNode {
    animator: Rc<dyn ItemAnimatorHandle>,
    cx: Option<NodeCx>,
}

impl ModifierNode for DisplayingDisappearingItemsNode {
    node_capabilities!(draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
        self.animator.set_displaying_node(Some(cx.handle()));
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.cx = None;
        self.animator.reset();
    }
}

impl DrawModifierNode for DisplayingDisappearingItemsNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        self.animator.draw_disappearing_items(scope);
        scope.draw_content();
    }
}
