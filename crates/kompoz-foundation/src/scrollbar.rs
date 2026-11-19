//! Scrollbars, as Compose for desktop's Scrollbar.skiko.kt: [`VerticalScrollbar`] and [`HorizontalScrollbar`] —
//! a track as long as their modifier makes them, `style.thickness` thick, a thumb as long as the share of the
//! content in view (at least `style.minimal_height`) where the content is; dragging the thumb scrolls by as much as
//! its travel stands for, pressing the track scrolls a viewport towards the press (again after 300 ms, then every
//! 100 ms, while held); the thumb darker while hovered or dragged (animated over `style.hover_duration_millis`); no
//! thumb when there is nothing to scroll. They read a [`ScrollbarAdapter`](crate::v2::ScrollbarAdapter) (made of a
//! scroll state, a lazy list's or a lazy grid's: [`remember_scrollbar_adapter`]), or the deprecated
//! [`OldScrollbarAdapter`].

use crate::background::BackgroundModifiers;
use crate::gestures::{AwaitPointerEventScopeGestures, LocalFuture, PointerInputScopeGestures};
use crate::hoverable::HoverableModifiers;
use crate::interaction::{DragInteraction, MutableInteractionSource};
use crate::layout::Box;
use crate::lazy::LazyListState;
use crate::lazy::grid::LazyGridState;
use crate::scroll::ScrollState;
use crate::v2::{LazyGridLines, LazyLineContentAdapter, LazyListLines, ScrollableScrollbarAdapter, ScrollbarAdapter};
use kompoz_animation::{AnimationSpec, FastOutSlowInEasing, TweenSpec, animate_color_as_state};
use kompoz_coroutines::{CoroutineScope, CoroutineStart, Job, delay_ms, remember_coroutine_scope};
use kompoz_graphics::{Color, Dp, LayoutDirection, Offset, RoundedCornerShape, Shape};
use kompoz_runtime::{
    DerivedState, DisposableEffect, KompozitionLocal, MutableState, derived_state_of, kompozable, mutable_state_of,
    named_args, on_dispose, remember, remember_keyed,
};
use kompoz_ui::Modifier;
use kompoz_ui::input::PointerInputScope;
use kompoz_ui::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, layout_into};
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::unit::Constraints;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// Style

/// How scrollbars look (Compose for desktop's `ScrollbarStyle`): the thumb's least length, its thickness and
/// shape, how long its color takes to change when the mouse comes or goes, its colors.
#[derive(Clone)]
pub struct ScrollbarStyle {
    pub minimal_height: Dp,
    pub thickness: Dp,
    pub shape: Arc<dyn Shape>,
    pub hover_duration_millis: i32,
    pub unhover_color: Color,
    pub hover_color: Color,
}

impl PartialEq for ScrollbarStyle {
    fn eq(&self, other: &Self) -> bool {
        self.minimal_height == other.minimal_height
            && self.thickness == other.thickness
            && Arc::ptr_eq(&self.shape, &other.shape)
            && self.hover_duration_millis == other.hover_duration_millis
            && self.unhover_color == other.unhover_color
            && self.hover_color == other.hover_color
    }
}

impl std::fmt::Debug for ScrollbarStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("ScrollbarStyle")
            .field("minimal_height", &self.minimal_height)
            .field("thickness", &self.thickness)
            .field("hover_duration_millis", &self.hover_duration_millis)
            .field("unhover_color", &self.unhover_color)
            .field("hover_color", &self.hover_color)
            .finish()
    }
}

/// The plain style, without a theme (Compose for desktop's `defaultScrollbarStyle()`): a 16 dp thumb at least, 8 dp
/// thick, corners of 4 dp, 300 ms, black at 12 % and 50 %.
pub fn default_scrollbar_style() -> ScrollbarStyle {
    ScrollbarStyle {
        minimal_height: Dp(16.0),
        thickness: Dp(8.0),
        shape: Arc::new(RoundedCornerShape(Dp(4.0))),
        hover_duration_millis: 300,
        unhover_color: Color::new(0.0, 0.0, 0.0, 0.12),
        hover_color: Color::new(0.0, 0.0, 0.0, 0.50),
    }
}

/// The scrollbars' style (Compose for desktop's `LocalScrollbarStyle`): usually set by a theme.
pub static LocalScrollbarStyle: KompozitionLocal<ScrollbarStyle> =
    KompozitionLocal::new_static(default_scrollbar_style);

// ---------------------------------------------------------------------------------------------------------------
// Adapters

/// What a scrollbar adapter is made of (Kotlin's overloads of `ScrollbarAdapter(scrollState)` and
/// `rememberScrollbarAdapter(scrollState)`: a [`ScrollState`], a [`LazyListState`], a [`LazyGridState`], a
/// value-based field's [`TextFieldScrollState`](crate::text::TextFieldScrollState)).
pub trait ScrollbarAdapterSource: PartialEq + Clone + 'static {
    fn scrollbar_adapter(&self) -> Rc<dyn ScrollbarAdapter>;
}

impl ScrollbarAdapterSource for ScrollState {
    fn scrollbar_adapter(&self) -> Rc<dyn ScrollbarAdapter> {
        Rc::new(ScrollableScrollbarAdapter { scroll_state: self.clone() })
    }
}

impl ScrollbarAdapterSource for LazyListState {
    fn scrollbar_adapter(&self) -> Rc<dyn ScrollbarAdapter> {
        Rc::new(LazyLineContentAdapter::new(LazyListLines { state: self.clone() }))
    }
}

impl ScrollbarAdapterSource for crate::text::TextFieldScrollState {
    fn scrollbar_adapter(&self) -> Rc<dyn ScrollbarAdapter> {
        Rc::new(crate::v2::TextFieldScrollbarAdapter { scroll_state: self.clone() })
    }
}

impl ScrollbarAdapterSource for LazyGridState {
    fn scrollbar_adapter(&self) -> Rc<dyn ScrollbarAdapter> {
        Rc::new(LazyLineContentAdapter::new(LazyGridLines::new(self.clone())))
    }
}

/// The adapter of a scroll state (`verticalScroll`/`horizontalScroll`'s), a lazy list's (its content's size
/// estimated from the items in view) or a lazy grid's (from the lines in view) (Compose for desktop's
/// `ScrollbarAdapter(scrollState)`).
pub fn ScrollbarAdapter(scroll_state: impl ScrollbarAdapterSource) -> Rc<dyn ScrollbarAdapter> {
    scroll_state.scrollbar_adapter()
}

/// [`ScrollbarAdapter()`](fn@ScrollbarAdapter) remembered while `scroll_state` is the same (Compose for desktop's
/// `rememberScrollbarAdapter`).
#[kompozable]
pub fn remember_scrollbar_adapter(scroll_state: impl ScrollbarAdapterSource) -> Rc<dyn ScrollbarAdapter> {
    remember_keyed(scroll_state.clone(), move || scroll_state.scrollbar_adapter())
}

/// The deprecated scrollbar adapter (Compose for desktop's `ScrollbarAdapter` interface, "Use
/// androidx.compose.foundation.v2.ScrollbarAdapter instead"): px as `f32`, sizes given the container's.
pub trait OldScrollbarAdapter {
    /// How far the content is scrolled from its start, px.
    fn scroll_offset(&self) -> f32;
    /// Jumps to `scroll_offset` px (kept within 0 and the maximum), the container `container_size` px long.
    fn scroll_to(&self, container_size: i32, scroll_offset: f32) -> LocalFuture<'static, ()>;
    /// The largest offset for a container `container_size` px long.
    fn max_scroll_offset(&self, container_size: i32) -> f32;
}

/// A new adapter as an old one (Compose's `NewScrollbarAdapterAsOld`).
struct NewScrollbarAdapterAsOld(Rc<dyn ScrollbarAdapter>);

impl OldScrollbarAdapter for NewScrollbarAdapterAsOld {
    fn scroll_offset(&self) -> f32 {
        self.0.scroll_offset() as f32
    }
    fn scroll_to(&self, _container_size: i32, scroll_offset: f32) -> LocalFuture<'static, ()> {
        self.0.scroll_to(scroll_offset as f64)
    }
    fn max_scroll_offset(&self, _container_size: i32) -> f32 {
        self.0.max_scroll_offset() as f32
    }
}

/// An old adapter as a new one, the track as long as the viewport (Compose's `OldScrollbarAdapterAsNew`).
struct OldScrollbarAdapterAsNew {
    old: Rc<dyn OldScrollbarAdapter>,
    track_size: i32,
}

impl ScrollbarAdapter for OldScrollbarAdapterAsNew {
    fn scroll_offset(&self) -> f64 {
        self.old.scroll_offset() as f64
    }
    fn content_size(&self) -> f64 {
        (self.old.max_scroll_offset(self.track_size) + self.track_size as f32) as f64
    }
    fn viewport_size(&self) -> f64 {
        self.track_size as f64
    }
    fn scroll_to(&self, scroll_offset: f64) -> LocalFuture<'static, ()> {
        self.old.scroll_to(self.track_size, scroll_offset as f32)
    }
}

/// The deprecated adapter of a scroll state, a lazy list's or a lazy grid's (Compose for desktop's deprecated
/// `ScrollbarAdapter(scrollState)` of the old interface, "Use ScrollbarAdapter() instead").
pub fn old_scrollbar_adapter(scroll_state: impl ScrollbarAdapterSource) -> Rc<dyn OldScrollbarAdapter> {
    Rc::new(NewScrollbarAdapterAsOld(scroll_state.scrollbar_adapter()))
}

/// [`old_scrollbar_adapter`] remembered (Compose for desktop's deprecated `rememberOldScrollbarAdapter`).
#[kompozable]
pub fn remember_old_scrollbar_adapter(scroll_state: impl ScrollbarAdapterSource) -> Rc<dyn OldScrollbarAdapter> {
    remember_keyed(scroll_state.clone(), move || old_scrollbar_adapter(scroll_state.clone()))
}

/// What a scrollbar takes (Kotlin's two overloads of `VerticalScrollbar`: a v2 adapter, or the deprecated one),
/// made a v2 adapter once the track's length is known.
#[derive(Clone)]
pub enum AnyScrollbarAdapter {
    New(Rc<dyn ScrollbarAdapter>),
    Old(Rc<dyn OldScrollbarAdapter>),
}

impl AnyScrollbarAdapter {
    /// The same adapter (for remembering).
    fn identity(&self) -> usize {
        match self {
            AnyScrollbarAdapter::New(a) => Rc::as_ptr(a) as *const () as usize,
            AnyScrollbarAdapter::Old(a) => Rc::as_ptr(a) as *const () as usize,
        }
    }

    /// The adapter for a track `track_size` px long (an old one unwrapped when it is a new one's).
    fn for_track(&self, track_size: i32) -> Rc<dyn ScrollbarAdapter> {
        match self {
            AnyScrollbarAdapter::New(a) => a.clone(),
            AnyScrollbarAdapter::Old(a) => Rc::new(OldScrollbarAdapterAsNew { old: a.clone(), track_size }),
        }
    }
}

impl PartialEq for AnyScrollbarAdapter {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl From<Rc<dyn ScrollbarAdapter>> for AnyScrollbarAdapter {
    fn from(a: Rc<dyn ScrollbarAdapter>) -> Self {
        AnyScrollbarAdapter::New(a)
    }
}

impl From<Rc<dyn OldScrollbarAdapter>> for AnyScrollbarAdapter {
    fn from(a: Rc<dyn OldScrollbarAdapter>) -> Self {
        AnyScrollbarAdapter::Old(a)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The scrollbars

/// A vertical scrollbar of the scrollable `adapter` reads (Compose for desktop's `VerticalScrollbar`): put it
/// beside the content (`Modifier.align(Alignment::CenterEnd).fill_max_height()`). `reverse_layout` for a reversed
/// list (its start at the bottom); drags told to `interaction_source` as DragInteractions, hovers as
/// HoverInteractions.
#[kompozable]
pub fn VerticalScrollbar(
    adapter: impl Into<AnyScrollbarAdapter>,
    #[default(Modifier)] modifier: Modifier,
    #[default(false)] reverse_layout: bool,
    #[default(LocalScrollbarStyle.current())] style: ScrollbarStyle,
    #[default(remember(MutableInteractionSource::new))] interaction_source: MutableInteractionSource,
) {
    old_or_new_scrollbar(adapter, modifier, reverse_layout, style, interaction_source, true);
}

/// A horizontal scrollbar (Compose for desktop's `HorizontalScrollbar`): as [`VerticalScrollbar`], below the
/// content; right to left its start on the right (`reverse_layout` flipped).
#[kompozable]
pub fn HorizontalScrollbar(
    adapter: impl Into<AnyScrollbarAdapter>,
    #[default(Modifier)] modifier: Modifier,
    #[default(false)] reverse_layout: bool,
    #[default(LocalScrollbarStyle.current())] style: ScrollbarStyle,
    #[default(remember(MutableInteractionSource::new))] interaction_source: MutableInteractionSource,
) {
    let reverse = if LocalLayoutDirection.current() == LayoutDirection::Rtl { !reverse_layout } else { reverse_layout };
    old_or_new_scrollbar(adapter, modifier, reverse, style, interaction_source, false);
}

/// The scrollbar (Compose's `OldOrNewScrollbar`).
#[track_caller]
fn old_or_new_scrollbar(
    old_or_new_adapter: AnyScrollbarAdapter,
    modifier: Modifier,
    reverse_layout: bool,
    style: ScrollbarStyle,
    interaction_source: MutableInteractionSource,
    is_vertical: bool,
) {
    kompoz_runtime::kompozable_block!({
        let density = LocalDensity.current();
        let drag_interaction: MutableState<Option<DragInteraction::Start>> = remember(|| mutable_state_of(None));
        let (cancelled, source) = (drag_interaction.clone(), interaction_source.clone());
        DisposableEffect(interaction_source.clone(), move || {
            on_dispose(move || {
                if let Some(interaction) = cancelled.peek() {
                    source.try_emit(DragInteraction::Cancel::new(interaction));
                    cancelled.set(None);
                }
            })
        });
        let container_size: MutableState<i32> = remember(|| mutable_state_of(0));
        let is_hovered = interaction_source.collect_is_hovered_as_state();
        let is_highlighted: DerivedState<bool> = remember(|| {
            let (hovered, dragged) = (is_hovered.clone(), drag_interaction.clone());
            derived_state_of(move || hovered.get() || dragged.get().is_some())
        });
        let minimal_height = density.to_px(style.minimal_height);
        let size = container_size.get();
        let adapter = {
            let a = old_or_new_adapter.clone();
            remember_keyed((old_or_new_adapter.identity(), size), move || a.for_track(size))
        };
        let coroutine_scope = remember_coroutine_scope();
        let slider_adapter = {
            let (a, scope) = (adapter.clone(), coroutine_scope.clone());
            remember_keyed(
                (
                    Rc::as_ptr(&adapter) as *const () as usize,
                    size,
                    minimal_height.to_bits(),
                    reverse_layout,
                    is_vertical,
                ),
                move || Rc::new(SliderAdapter::new(a, size, minimal_height, reverse_layout, is_vertical, scope)),
            )
        };
        let scroll_thickness = density.round_to_px(style.thickness);
        let measure_policy: Rc<dyn MeasurePolicy> = {
            let (slider, set) = (slider_adapter.clone(), container_size.clone());
            remember_keyed((Rc::as_ptr(&slider_adapter) as usize, scroll_thickness), move || {
                Rc::new(ScrollbarMeasurePolicy {
                    slider_adapter: slider,
                    set_container_size: set,
                    scroll_thickness,
                    is_vertical,
                }) as Rc<dyn MeasurePolicy>
            })
        };
        let hover_spec: Arc<dyn AnimationSpec<Color>> =
            Arc::new(TweenSpec::new(style.hover_duration_millis, 0, FastOutSlowInEasing));
        let color = animate_color_as_state(
            if is_highlighted.get() { style.hover_color } else { style.unhover_color },
            animation_spec = hover_spec,
        )
        .value();
        let is_visible = (slider_adapter.thumb_size() as f32) < size as f32;
        let thumb = Modifier
            .background(if is_visible { color } else { Color::Transparent }, style.shape.clone())
            .then_element(ScrollbarDragElement {
                interaction_source: interaction_source.clone(),
                dragged_interaction: drag_interaction.clone(),
                slider_adapter: slider_adapter.clone(),
            });
        let track = modifier.hoverable(interaction_source.clone(), true).then_element(ScrollOnPressTrackElement {
            is_vertical,
            reverse_layout,
            slider_adapter: slider_adapter.clone(),
        });
        layout_into(track, measure_policy, false, move || {
            Box(modifier = thumb);
        });
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The slider

/// Where the thumb is, and scrolling as it is dragged (Compose's internal `SliderAdapter`).
pub(crate) struct SliderAdapter {
    adapter: Rc<dyn ScrollbarAdapter>,
    track_size: i32,
    min_height: f32,
    reverse_layout: bool,
    is_vertical: bool,
    coroutine_scope: CoroutineScope,
    /// How much of the drag was ignored at the scrollbar's ends.
    unscrolled_drag_distance: Cell<f64>,
    /// Drag deltas waiting to be applied, in order (Compose's `dragMutex`: each applied after those before).
    pending_deltas: RefCell<VecDeque<Offset>>,
    applying: Cell<bool>,
}

impl SliderAdapter {
    fn new(
        adapter: Rc<dyn ScrollbarAdapter>,
        track_size: i32,
        min_height: f32,
        reverse_layout: bool,
        is_vertical: bool,
        coroutine_scope: CoroutineScope,
    ) -> SliderAdapter {
        SliderAdapter {
            adapter,
            track_size,
            min_height,
            reverse_layout,
            is_vertical,
            coroutine_scope,
            unscrolled_drag_distance: Cell::new(0.0),
            pending_deltas: RefCell::new(VecDeque::new()),
            applying: Cell::new(false),
        }
    }

    fn visible_part(&self) -> f64 {
        let content_size = self.adapter.content_size();
        if content_size == 0.0 { 1.0 } else { (self.adapter.viewport_size() / content_size).min(1.0) }
    }

    fn thumb_size(&self) -> f64 {
        (self.track_size as f64 * self.visible_part()).max(self.min_height as f64)
    }

    fn scroll_scale(&self) -> f64 {
        let extra_scrollbar_space = self.track_size as f64 - self.thumb_size();
        let extra_content_space = self.adapter.max_scroll_offset();
        if extra_content_space == 0.0 { 1.0 } else { extra_scrollbar_space / extra_content_space }
    }

    fn raw_position(&self) -> f64 {
        self.scroll_scale() * self.adapter.scroll_offset()
    }

    fn position(&self) -> f64 {
        if self.reverse_layout {
            self.track_size as f64 - self.thumb_size() - self.raw_position()
        } else {
            self.raw_position()
        }
    }

    /// The thumb's pixels along the track, `start..end` (Compose's `thumbPixelRange`).
    fn thumb_pixel_range(&self) -> (i32, i32) {
        let start = self.position().round() as i32;
        (start, start + self.thumb_size().round() as i32)
    }

    fn on_drag_started(&self) {
        self.unscrolled_drag_distance.set(0.0);
    }

    fn set_position(&self, value: f64) -> LocalFuture<'static, ()> {
        let raw_position = if self.reverse_layout { self.track_size as f64 - self.thumb_size() - value } else { value };
        self.adapter.scroll_to(raw_position / self.scroll_scale())
    }

    /// A move of the dragged thumb (Compose's `onDragDelta`): applied after the ones before it.
    fn on_drag_delta(self: &Rc<Self>, offset: Offset) {
        self.pending_deltas.borrow_mut().push_back(offset);
        if self.applying.replace(true) {
            return;
        }
        let me = self.clone();
        named_args!(self.coroutine_scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
            /// Lets the next delta start a new run however this one ends.
            struct Done(Rc<SliderAdapter>);
            impl Drop for Done {
                fn drop(&mut self) {
                    self.0.applying.set(false);
                }
            }
            let _done = Done(me.clone());
            loop {
                let next = me.pending_deltas.borrow_mut().pop_front();
                let Some(offset) = next else { break };
                let drag_delta = if me.is_vertical { offset.y } else { offset.x } as f64;
                let max_scroll_position = me.adapter.max_scroll_offset() * me.scroll_scale();
                let current_position = me.position();
                let target_position = (current_position + drag_delta + me.unscrolled_drag_distance.get())
                    .clamp(0.0, max_scroll_position.max(0.0));
                let slider_delta = target_position - current_position;
                // (from the position as it is now: items of other sizes scroll smoothly)
                let new_position = me.position() + slider_delta;
                me.set_position(new_position).await;
                me.unscrolled_drag_distance.set(me.unscrolled_drag_distance.get() + drag_delta - slider_delta);
            }
        }));
    }
}

/// The scrollbar's measure (Compose's `verticalMeasurePolicy` and `horizontalMeasurePolicy`): the track's length the
/// constraints' maximum (told to the scrollbar), the thumb measured to its pixels and placed there.
struct ScrollbarMeasurePolicy {
    slider_adapter: Rc<SliderAdapter>,
    set_container_size: MutableState<i32>,
    scroll_thickness: i32,
    is_vertical: bool,
}

impl MeasurePolicy for ScrollbarMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let (start, end) = self.slider_adapter.thumb_pixel_range();
        let length = end - start;
        if self.is_vertical {
            self.set_container_size.set(constraints.max_height);
            let placeable =
                measurables[0].measure(Constraints::fixed(constraints.constrain_width(self.scroll_thickness), length));
            let width = placeable.width();
            scope.layout(width, constraints.max_height, move |p| p.place(&placeable, 0, start))
        } else {
            self.set_container_size.set(constraints.max_width);
            let placeable =
                measurables[0].measure(Constraints::fixed(length, constraints.constrain_height(self.scroll_thickness)));
            let height = placeable.height();
            scope.layout(constraints.max_width, height, move |p| p.place(&placeable, start, 0))
        }
    }
}

// ---- Dragging the thumb ----

#[derive(Clone)]
struct ScrollbarDragElement {
    interaction_source: MutableInteractionSource,
    dragged_interaction: MutableState<Option<DragInteraction::Start>>,
    slider_adapter: Rc<SliderAdapter>,
}

impl PartialEq for ScrollbarDragElement {
    fn eq(&self, other: &Self) -> bool {
        self.interaction_source == other.interaction_source
            && self.dragged_interaction == other.dragged_interaction
            && Rc::ptr_eq(&self.slider_adapter, &other.slider_adapter)
    }
}

impl kompoz_ui::ModifierNodeElement for ScrollbarDragElement {
    type Node = ScrollbarDragNode;
    fn create(&self) -> ScrollbarDragNode {
        ScrollbarDragNode { state: Rc::new(RefCell::new(self.clone())), input: None }
    }
    fn update(&self, node: &mut ScrollbarDragNode) {
        *node.state.borrow_mut() = self.clone();
    }
    fn name(&self) -> &'static str {
        "scrollbarDrag"
    }
}

/// The thumb's drag (Compose's ScrollbarDragModifierNode): from any press (consumed or not), each move scrolls.
struct ScrollbarDragNode {
    state: Rc<RefCell<ScrollbarDragElement>>,
    input: Option<kompoz_ui::modifier::DelegatedNode<kompoz_ui::input::SuspendingPointerInputModifierNode>>,
}

impl kompoz_ui::ModifierNode for ScrollbarDragNode {
    kompoz_ui::node_capabilities!();

    fn on_attach(&mut self, cx: &kompoz_ui::NodeCx) {
        if self.input.is_some() {
            return;
        }
        let state = self.state.clone();
        self.input = Some(cx.delegate(kompoz_ui::input::SuspendingPointerInputModifierNode::new(
            move |scope: PointerInputScope| {
                let state = state.clone();
                async move {
                    scope
                        .await_each_gesture(move |s| {
                            let state = state.clone();
                            async move {
                                let down =
                                    s.await_first_down_with(false, kompoz_ui::input::PointerEventPass::Main).await;
                                let e = state.borrow().clone();
                                let interaction = DragInteraction::Start::new();
                                e.interaction_source.try_emit(interaction);
                                e.dragged_interaction.set(Some(interaction));
                                e.slider_adapter.on_drag_started();
                                let is_success = s
                                    .drag(down.id, |change| {
                                        state.borrow().slider_adapter.on_drag_delta(change.position_change());
                                        change.consume();
                                    })
                                    .await;
                                let e = state.borrow().clone();
                                if is_success {
                                    e.interaction_source.try_emit(DragInteraction::Stop::new(interaction));
                                } else {
                                    e.interaction_source.try_emit(DragInteraction::Cancel::new(interaction));
                                }
                                e.dragged_interaction.set(None);
                            }
                        })
                        .await
                }
            },
        )));
    }
}

// ---- Pressing the track ----

/// The delay between the first and the second scroll while the track is pressed outside the thumb, ms (Compose's
/// `DelayBeforeSecondScrollOnTrackPress`).
pub(crate) const DelayBeforeSecondScrollOnTrackPress: u64 = 300;
/// The delay between the scrolls after the second, ms (Compose's `DelayBetweenScrollsOnTrackPress`).
pub(crate) const DelayBetweenScrollsOnTrackPress: u64 = 100;

#[derive(Clone)]
struct ScrollOnPressTrackElement {
    is_vertical: bool,
    reverse_layout: bool,
    slider_adapter: Rc<SliderAdapter>,
}

impl PartialEq for ScrollOnPressTrackElement {
    fn eq(&self, other: &Self) -> bool {
        self.is_vertical == other.is_vertical
            && self.reverse_layout == other.reverse_layout
            && Rc::ptr_eq(&self.slider_adapter, &other.slider_adapter)
    }
}

impl kompoz_ui::ModifierNodeElement for ScrollOnPressTrackElement {
    type Node = ScrollOnPressTrackNode;
    fn create(&self) -> ScrollOnPressTrackNode {
        ScrollOnPressTrackNode { element: self.clone(), scroller: Rc::new(RefCell::new(None)), input: None }
    }
    fn update(&self, node: &mut ScrollOnPressTrackNode) {
        let needs_reset = *self != node.element;
        node.element = self.clone();
        if needs_reset {
            node.recreate_scroller();
            if let Some(input) = &node.input {
                input.borrow().reset_pointer_input_handler();
            }
        }
    }
    fn name(&self) -> &'static str {
        "scrollOnPressTrack"
    }
}

/// Scrolls while the track is pressed outside the thumb (Compose's ScrollOnPressTrackNode).
struct ScrollOnPressTrackNode {
    element: ScrollOnPressTrackElement,
    scroller: Rc<RefCell<Option<Rc<TrackPressScroller>>>>,
    input: Option<kompoz_ui::modifier::DelegatedNode<kompoz_ui::input::SuspendingPointerInputModifierNode>>,
}

impl ScrollOnPressTrackNode {
    fn recreate_scroller(&mut self) {
        let scope = self.element.slider_adapter.coroutine_scope.clone();
        *self.scroller.borrow_mut() = Some(Rc::new(TrackPressScroller {
            coroutine_scope: scope,
            slider_adapter: self.element.slider_adapter.clone(),
            reverse_layout: self.element.reverse_layout,
            direction: Cell::new(0),
            offset: Cell::new(None),
            job: RefCell::new(None),
        }));
    }
}

impl kompoz_ui::ModifierNode for ScrollOnPressTrackNode {
    kompoz_ui::node_capabilities!();

    fn on_attach(&mut self, cx: &kompoz_ui::NodeCx) {
        self.recreate_scroller();
        if self.input.is_some() {
            return;
        }
        let (scroller, element) = (self.scroller.clone(), Rc::new(Cell::new(self.element.is_vertical)));
        let is_vertical = element;
        self.input = Some(cx.delegate(kompoz_ui::input::SuspendingPointerInputModifierNode::new(
            move |scope: PointerInputScope| {
                let scroller = scroller.borrow().clone().expect("made at attach");
                detect_scroll_via_track_gestures(scope, is_vertical.get(), scroller)
            },
        )));
    }
}

/// Compose's `detectScrollViaTrackGestures`: a press tells the scroller where, its moves where it is, its release
/// or cancel that it ended.
async fn detect_scroll_via_track_gestures(
    scope: PointerInputScope,
    is_vertical: bool,
    scroller: Rc<TrackPressScroller>,
) {
    let on_axis = move |o: Offset| if is_vertical { o.y } else { o.x };
    scope
        .await_each_gesture(move |s| {
            let scroller = scroller.clone();
            async move {
                let down = s.await_first_down().await;
                scroller.on_press(on_axis(down.position));
                loop {
                    let drag = if is_vertical {
                        s.await_vertical_drag_or_cancellation(down.id).await
                    } else {
                        s.await_horizontal_drag_or_cancellation(down.id).await
                    };
                    match drag {
                        None => {
                            scroller.on_gesture_cancelled();
                            break;
                        }
                        Some(d) if !d.pressed => {
                            scroller.on_release();
                            break;
                        }
                        Some(d) => scroller.on_move_pressed(on_axis(d.position)),
                    }
                }
            }
        })
        .await
}

/// Scrolls while the track is pressed outside the thumb (Compose's TrackPressScroller): a viewport towards the
/// press at once, again after 300 ms, then every 100 ms, as long as the thumb hasn't reached the press.
struct TrackPressScroller {
    coroutine_scope: CoroutineScope,
    slider_adapter: Rc<SliderAdapter>,
    reverse_layout: bool,
    /// 1 down or right, -1 up or left, 0 not scrolling.
    direction: Cell<i32>,
    /// Where the press is along the track, px.
    offset: Cell<Option<f32>>,
    job: RefCell<Option<Job>>,
}

impl TrackPressScroller {
    fn direction_of_scroll_towards(&self, offset: f32) -> i32 {
        let (first, end) = self.slider_adapter.thumb_pixel_range();
        let last = end - 1;
        if offset < first as f32 {
            if self.reverse_layout { 1 } else { -1 }
        } else if offset > last as f32 {
            if self.reverse_layout { -1 } else { 1 }
        } else {
            0
        }
    }

    async fn scroll_towards_current_offset(&self) {
        let Some(offset) = self.offset.get() else { return };
        let current_direction = self.direction_of_scroll_towards(offset);
        if current_direction != self.direction.get() {
            return;
        }
        let adapter = &self.slider_adapter.adapter;
        adapter.scroll_to(adapter.scroll_offset() + current_direction as f64 * adapter.viewport_size()).await;
    }

    fn start_scrolling(self: &Rc<Self>) {
        if let Some(job) = self.job.borrow_mut().take() {
            job.cancel();
        }
        let me = self.clone();
        let job = self.coroutine_scope.launch(async move {
            me.scroll_towards_current_offset().await;
            delay_ms(DelayBeforeSecondScrollOnTrackPress).await;
            loop {
                me.scroll_towards_current_offset().await;
                delay_ms(DelayBetweenScrollsOnTrackPress).await;
            }
        });
        *self.job.borrow_mut() = Some(job);
    }

    fn on_press(self: &Rc<Self>, offset: f32) {
        self.offset.set(Some(offset));
        self.direction.set(self.direction_of_scroll_towards(offset));
        if self.direction.get() != 0 {
            self.start_scrolling();
        }
    }

    fn on_move_pressed(&self, offset: f32) {
        self.offset.set(Some(offset));
    }

    fn cleanup_after_gesture(&self) {
        if let Some(job) = self.job.borrow_mut().take() {
            job.cancel();
        }
        self.direction.set(0);
        self.offset.set(None);
    }

    fn on_release(&self) {
        self.cleanup_after_gesture();
    }

    fn on_gesture_cancelled(&self) {
        self.cleanup_after_gesture();
    }
}
