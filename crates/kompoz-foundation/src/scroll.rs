//! Scroll containers, as Compose's Scroll.kt: a [`ScrollState`] (how far, px, up to how far), remembered and saved
//! with [`remember_scroll_state`]; `Modifier.vertical_scroll` and `horizontal_scroll` lay their content out as long
//! as it is along their axis and scroll it by the state, through a scrollable area (drags, flings, the wheel,
//! overscroll, clipping, bringing into view).

use crate::clip_scrollable_container::check_scrollable_container_constraints;
use crate::gestures::{
    FlingBehavior, LocalFuture, Orientation, ScrollBlock, ScrollIndicatorState, ScrollableState,
    ScrollableStateExtensions,
};
use crate::interaction::{InteractionSource, MutableInteractionSource};
use crate::overscroll::OverscrollEffect;
use kompoz_animation::{AnimationSpec, MutatePriority, spring};
use kompoz_graphics::IntOffset;
use kompoz_runtime::{
    DerivedState, MutableState, SavedValue, Saver, derived_state_of, kompozable, mutable_state_of, named,
    remember_saveable_with, without_read_observation,
};
use kompoz_ui::layout::{GraphicsLayerScope, LayerBlock, LayoutModifierNode};
use kompoz_ui::semantics::{ScrollAxisRange, SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{
    Constraints, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasureResult, MeasureScope, Modifier,
    ModifierNode, ModifierNodeElement, node_capabilities,
};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// The state

/// How far a scroll container scrolled (Compose's ScrollState): its value (px, from its start) and its maximum, the
/// length of its viewport and content, as states (reading them recomposes as they change); a [`ScrollableState`]
/// of its own whose deltas move the value (whole pixels, the rest kept for the next). Clones are the same state.
#[derive(Clone)]
pub struct ScrollState {
    inner: Rc<ScrollStateInner>,
}

struct ScrollStateInner {
    value: MutableState<i32>,
    max_value_state: MutableState<i32>,
    viewport_size: MutableState<i32>,
    content_size: MutableState<i32>,
    reverse_scrolling: MutableState<bool>,
    internal_interaction_source: MutableInteractionSource,
    accumulator: Cell<f32>,
    scrollable_state: crate::gestures::DefaultScrollableState,
    can_scroll_forward: DerivedState<bool>,
    can_scroll_backward: DerivedState<bool>,
}

impl PartialEq for ScrollState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for ScrollState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScrollState(value={}, maxValue={})", self.inner.value.peek(), self.inner.max_value_state.peek())
    }
}

/// Java's `Math.round(float)` (Compose's `fastRoundToInt`): half up.
fn fast_round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// A scroll state at `initial` px (Compose's `ScrollState(initial)`).
pub fn ScrollState(initial: i32) -> ScrollState {
    ScrollState::new(initial)
}

impl ScrollState {
    /// A state at `initial` px (Compose's constructor).
    pub fn new(initial: i32) -> ScrollState {
        let inner = Rc::new_cyclic(|me: &std::rc::Weak<ScrollStateInner>| {
            let value = mutable_state_of(initial);
            let max_value_state = mutable_state_of(i32::MAX);
            let me = me.clone();
            let scrollable_state = crate::gestures::ScrollableState(move |it| {
                let Some(s) = me.upgrade() else { return 0.0 };
                let current = s.value.get();
                let absolute = current as f32 + it + s.accumulator.get();
                let new_value = absolute.clamp(0.0, s.max_value_state.get() as f32);
                let changed = absolute != new_value;
                let consumed = new_value - current as f32;
                let consumed_int = fast_round_to_int(consumed);
                s.value.set(current + consumed_int);
                s.accumulator.set(consumed - consumed_int as f32);
                // (no float rounding error)
                if changed { consumed } else { it }
            });
            let (v, m) = (value.clone(), max_value_state.clone());
            let can_scroll_forward = derived_state_of(move || v.get() < m.get());
            let v = value.clone();
            let can_scroll_backward = derived_state_of(move || v.get() > 0);
            ScrollStateInner {
                value,
                max_value_state,
                viewport_size: mutable_state_of(0),
                content_size: mutable_state_of(0),
                reverse_scrolling: mutable_state_of(false),
                internal_interaction_source: MutableInteractionSource::new(),
                accumulator: Cell::new(0.0),
                scrollable_state,
                can_scroll_forward,
                can_scroll_backward,
            }
        });
        ScrollState { inner }
    }

    /// How far it scrolled, px (a state: Compose's `value`).
    pub fn value(&self) -> i32 {
        self.inner.value.get()
    }

    /// How far it can scroll, px (a state; `i32::MAX` until laid out: Compose's `maxValue`).
    pub fn max_value(&self) -> i32 {
        self.inner.max_value_state.get()
    }

    /// Its maximum, the value brought within it, not read (Compose's internal `maxValue` setter): what the scroll
    /// node measured.
    pub(crate) fn set_max_value(&self, new_max: i32) {
        self.inner.max_value_state.set(new_max);
        without_read_observation(|| {
            if self.inner.value.get() > new_max {
                self.inner.value.set(new_max);
            }
        });
    }

    /// The length of what shows of its content, px (a state: Compose's `viewportSize`).
    pub fn viewport_size(&self) -> i32 {
        self.inner.viewport_size.get()
    }

    pub(crate) fn set_viewport_size(&self, size: i32) {
        self.inner.viewport_size.set(size);
    }

    pub(crate) fn content_size(&self) -> i32 {
        self.inner.content_size.get()
    }

    pub(crate) fn set_content_size(&self, size: i32) {
        self.inner.content_size.set(size);
    }

    pub(crate) fn set_reverse_scrolling(&self, reverse: bool) {
        self.inner.reverse_scrolling.set(reverse);
    }

    /// The drags of the containers it scrolls (Compose's `interactionSource`).
    pub fn interaction_source(&self) -> InteractionSource {
        self.inner.internal_interaction_source.as_interaction_source()
    }

    /// The source its containers emit their drags to (Compose's `internalInteractionSource`).
    pub(crate) fn internal_interaction_source(&self) -> MutableInteractionSource {
        self.inner.internal_interaction_source.clone()
    }

    /// Scrolls to `value` px at once, in a mutation of its own: how far it went (Compose's `scrollTo`).
    pub fn scroll_to(&self, value: i32) -> impl std::future::Future<Output = f32> {
        self.scroll_by((value - self.value()) as f32)
    }

    /// The saver of [`remember_scroll_state`] (Compose's `ScrollState.Saver`): its value.
    pub fn saver() -> Saver<ScrollState> {
        Saver::new(
            |s: &ScrollState| SavedValue::from(s.inner.value.peek()),
            |v| v.as_int().map(|value| ScrollState::new(value as i32)),
        )
    }
}

#[named]
impl ScrollState {
    /// Scrolls to `value` px along `animation_spec` (a spring by default), in a mutation of its own (Compose's
    /// `animateScrollTo`).
    pub fn animate_scroll_to(
        &self,
        value: i32,
        #[default(Arc::new(spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> impl std::future::Future<Output = ()> {
        let by = crate::gestures::scrollable_state::animate_scroll_by_of(
            self,
            (value - self.value()) as f32,
            animation_spec,
        );
        async move {
            by.await;
        }
    }
}

impl ScrollableState for ScrollState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        self.inner.scrollable_state.scroll_block(scroll_priority, block)
    }

    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        self.inner.scrollable_state.dispatch_raw_delta(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.inner.scrollable_state.is_scroll_in_progress()
    }

    fn can_scroll_forward(&self) -> bool {
        self.inner.can_scroll_forward.get()
    }

    fn can_scroll_backward(&self) -> bool {
        self.inner.can_scroll_backward.get()
    }

    fn last_scrolled_forward(&self) -> bool {
        self.inner.scrollable_state.last_scrolled_forward()
    }

    fn last_scrolled_backward(&self) -> bool {
        self.inner.scrollable_state.last_scrolled_backward()
    }

    fn scroll_indicator_state(&self) -> Option<Rc<dyn ScrollIndicatorState>> {
        Some(Rc::new(self.clone()))
    }
}

/// What a scrollbar shows of it (Compose's ScrollState `scrollIndicatorState`): its offset from the end it
/// starts at.
impl ScrollIndicatorState for ScrollState {
    fn scroll_offset(&self) -> i32 {
        if self.inner.reverse_scrolling.get() { self.max_value() - self.value() } else { self.value() }
    }
    fn content_size(&self) -> i32 {
        ScrollState::content_size(self)
    }
    fn viewport_size(&self) -> i32 {
        ScrollState::viewport_size(self)
    }
}

/// A scroll state at `initial` px, remembered and saved where it is called (Compose's `rememberScrollState`).
#[kompozable]
pub fn remember_scroll_state(#[default(0)] initial: i32) -> ScrollState {
    remember_saveable_with(&ScrollState::saver(), move || ScrollState::new(initial))
}

// ---------------------------------------------------------------------------------------------------------------
// The modifiers

/// The scroll containers.
#[named(trait ScrollModifiers)]
impl Modifier {
    /// Lays its content out as tall as it is and scrolls it vertically by `state` (Compose's
    /// `Modifier.verticalScroll`): dragged, flung (`fling_behavior`, else the default), wheeled, its content's
    /// requests brought into view, clipped vertically, with [`LocalOverscrollFactory`]'s overscroll; from its end
    /// with `reverse_scrolling` (value 0 at the bottom). Not `enabled`: not by the user (still from code and
    /// bringing into view).
    ///
    /// [`LocalOverscrollFactory`]: crate::overscroll::LocalOverscrollFactory
    fn vertical_scroll(
        self,
        state: &ScrollState,
        #[default(true)] enabled: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(false)] reverse_scrolling: bool,
    ) -> Modifier {
        scroll(self, state, reverse_scrolling, fling_behavior, enabled, true, true, None)
    }

    /// The same with `overscroll_effect` (none: no overscroll) instead of the factory's (Compose's
    /// `verticalScroll(state, overscrollEffect, …)`).
    /// Kotlin: `verticalScroll(state: ScrollState, overscrollEffect: OverscrollEffect?, enabled: Boolean = true, flingBehavior: FlingBehavior? = null, reverseScrolling: Boolean = false)`
    fn vertical_scroll_with_overscroll(
        self,
        state: &ScrollState,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(true)] enabled: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(false)] reverse_scrolling: bool,
    ) -> Modifier {
        scroll(self, state, reverse_scrolling, fling_behavior, enabled, true, false, overscroll_effect)
    }

    /// Lays its content out as wide as it is and scrolls it horizontally by `state` (Compose's
    /// `Modifier.horizontalScroll`; right to left, from the right).
    fn horizontal_scroll(
        self,
        state: &ScrollState,
        #[default(true)] enabled: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(false)] reverse_scrolling: bool,
    ) -> Modifier {
        scroll(self, state, reverse_scrolling, fling_behavior, enabled, false, true, None)
    }

    /// The same with `overscroll_effect` (Compose's `horizontalScroll(state, overscrollEffect, …)`).
    /// Kotlin: `horizontalScroll(state: ScrollState, overscrollEffect: OverscrollEffect?, enabled: Boolean = true, flingBehavior: FlingBehavior? = null, reverseScrolling: Boolean = false)`
    fn horizontal_scroll_with_overscroll(
        self,
        state: &ScrollState,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(true)] enabled: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(false)] reverse_scrolling: bool,
    ) -> Modifier {
        scroll(self, state, reverse_scrolling, fling_behavior, enabled, false, false, overscroll_effect)
    }
}

/// Compose's private `Modifier.scroll`: a scrollable area of the state, then the layout scrolling its content.
#[allow(clippy::too_many_arguments)]
fn scroll(
    m: Modifier,
    state: &ScrollState,
    reverse_scrolling: bool,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    is_scrollable: bool,
    is_vertical: bool,
    use_local_overscroll_factory: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
) -> Modifier {
    let orientation = if is_vertical { Orientation::Vertical } else { Orientation::Horizontal };
    let scrollable_area = if use_local_overscroll_factory {
        crate::scrollable_area::scrollable_area_of(
            m,
            Rc::new(state.clone()),
            orientation,
            is_scrollable,
            reverse_scrolling,
            fling_behavior,
            Some(state.internal_interaction_source()),
            None,
            true,
            None,
        )
    } else {
        crate::scrollable_area::scrollable_area_of(
            m,
            Rc::new(state.clone()),
            orientation,
            is_scrollable,
            reverse_scrolling,
            fling_behavior,
            Some(state.internal_interaction_source()),
            None,
            false,
            overscroll_effect,
        )
    };
    scrollable_area.then_element(ScrollingLayoutElement { scroll_state: state.clone(), reverse_scrolling, is_vertical })
}

#[derive(PartialEq)]
struct ScrollingLayoutElement {
    scroll_state: ScrollState,
    reverse_scrolling: bool,
    is_vertical: bool,
}

impl ModifierNodeElement for ScrollingLayoutElement {
    type Node = ScrollNode;
    fn create(&self) -> ScrollNode {
        ScrollNode {
            state: self.scroll_state.clone(),
            reverse_scrolling: self.reverse_scrolling,
            is_vertical: self.is_vertical,
            layer_block: Rc::new(|_: &mut GraphicsLayerScope| {}),
        }
    }
    fn update(&self, node: &mut ScrollNode) {
        node.state = self.scroll_state.clone();
        node.reverse_scrolling = self.reverse_scrolling;
        node.is_vertical = self.is_vertical;
    }
    fn name(&self) -> &'static str {
        "scroll"
    }
}

/// The layout of a scroll container (Compose's ScrollNode): its content measured unbounded along its axis, placed
/// moved back by the state's value (through a layer), the state told its maximum, viewport and content.
pub struct ScrollNode {
    state: ScrollState,
    reverse_scrolling: bool,
    is_vertical: bool,
    /// What its content is placed with: a layer of no properties (Compose's DefaultLayerBlock).
    layer_block: LayerBlock,
}

impl ModifierNode for ScrollNode {
    node_capabilities!(layout, semantics);
}

impl LayoutModifierNode for ScrollNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let is_vertical = self.is_vertical;
        check_scrollable_container_constraints(
            constraints,
            if is_vertical { Orientation::Vertical } else { Orientation::Horizontal },
        );
        let child_constraints = Constraints {
            max_height: if is_vertical { Constraints::Infinity } else { constraints.max_height },
            max_width: if is_vertical { constraints.max_width } else { Constraints::Infinity },
            ..constraints
        };
        let placeable = measurable.measure(child_constraints);
        let width = placeable.width().min(constraints.max_width);
        let height = placeable.height().min(constraints.max_height);
        let scroll_height = placeable.height() - height;
        let scroll_width = placeable.width() - width;
        let side = if is_vertical { scroll_height } else { scroll_width };
        // (the maximum before the measure ends: what scrolls to the new end in `on_remeasured` sees it)
        let state = self.state.clone();
        state.set_max_value(side);
        state.set_viewport_size(if is_vertical { height } else { width });
        state.set_content_size(if is_vertical { placeable.height() } else { placeable.width() });
        state.set_reverse_scrolling(self.reverse_scrolling);
        let (reverse_scrolling, layer_block) = (self.reverse_scrolling, self.layer_block.clone());
        scope.layout(width, height, move |pl| {
            let scroll = state.value().clamp(0, side);
            let abs_scroll = if reverse_scrolling { scroll - side } else { -scroll };
            let x_offset = if is_vertical { 0 } else { abs_scroll };
            let y_offset = if is_vertical { abs_scroll } else { 0 };
            // (Compose tags it as direct manipulation for lookahead's approach: stage 11's)
            pl.place_relative_with_layer_block(
                &placeable,
                IntOffset::new(x_offset, y_offset),
                layer_block.clone(),
                0.0,
            );
        })
    }

    fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.min_intrinsic_width(if self.is_vertical { Constraints::Infinity } else { height })
    }
    fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.min_intrinsic_height(if self.is_vertical { width } else { Constraints::Infinity })
    }
    fn max_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.max_intrinsic_width(if self.is_vertical { Constraints::Infinity } else { height })
    }
    fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.max_intrinsic_height(if self.is_vertical { width } else { Constraints::Infinity })
    }
}

impl SemanticsModifierNode for ScrollNode {
    /// A traversal group with its axis' range (Compose's ScrollNode `applySemantics`).
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_is_traversal_group(true);
        let (v, m) = (self.state.clone(), self.state.clone());
        let accessibility_scroll_state =
            ScrollAxisRange::new(move || v.value() as f32, move || m.max_value() as f32, self.reverse_scrolling);
        if self.is_vertical {
            config.set_vertical_scroll_axis_range(accessibility_scroll_state);
        } else {
            config.set_horizontal_scroll_axis_range(accessibility_scroll_state);
        }
    }
}
