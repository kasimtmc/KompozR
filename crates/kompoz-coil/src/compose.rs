//! Coil's compose layer (coil-compose-core 3.4 and coil-compose's singleton forms): `AsyncImage` (a layout that
//! loads its image at the size its constraints give and paints it), `rememberAsyncImagePainter` (a painter that
//! loads when remembered, at the original size), `SubcomposeAsyncImage` (slots for loading, success and error),
//! the painter's states (`Empty`, `Loading`, `Success`, `Error`), placeholder, error and fallback painters,
//! cross-fades, and the size resolver of a layout's constraints.

use crate::loader::{ImageLoader, SingletonImageLoader};
use crate::request::{
    CoilSize, DataSource, Dimension, ErrorResult, ImageError, ImageRequest, ImageResult, PlatformContext, Precision,
    Scale, SizeResolver, SuccessResult,
};
use kompoz_coroutines::{CoroutineScope, CoroutineStart, Dispatcher, Job, remember_coroutine_scope};
use kompoz_foundation::layout::{Box, BoxWithConstraints};
use kompoz_graphics::{ColorFilter, ContentScale, DefaultFilterQuality, FilterQuality, IntSize, Size};
use kompoz_runtime::{
    KompozitionLocal, MutableState, RememberObserver, kompozable, mutable_confined_state_of_with_policy,
    never_equal_policy, remember, remember_keyed,
};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, DrawScope, Painter};
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
    layout_into,
};
use kompoz_ui::modifier::{ModifierNode, ModifierNodeElement, NodeCx};
use kompoz_ui::platform::LocalPlatform;
use kompoz_ui::semantics::{Role, SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Alignment, Constraints, MeasurePolicy, Modifier, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// The platform context here (Coil's `LocalPlatformContext`): the host's platform, whose image loader loads.
pub static LocalPlatformContext: KompozitionLocal<PlatformContext> =
    KompozitionLocal::new_computed(|s| PlatformContext(s.current_value(&LocalPlatform)));

// ---- Sizes --------------------------------------------------------------------------------------------------------

/// A size resolver of a layout's constraints (Coil's `ConstraintsSizeResolver`): its size waits for the layout's
/// first constraints that aren't zero, then is their maximum (an unbounded side undefined).
pub struct ConstraintsSizeResolver {
    latest: Cell<Constraints>,
    waiting: RefCell<Vec<Waker>>,
}

impl Default for ConstraintsSizeResolver {
    fn default() -> Self {
        ConstraintsSizeResolver { latest: Cell::new(Constraints::new(0, 0, 0, 0)), waiting: RefCell::new(Vec::new()) }
    }
}

fn is_zero(c: Constraints) -> bool {
    c.max_width == 0 && c.max_height == 0
}

fn dimension(v: i32) -> Dimension {
    if v != Constraints::Infinity { Dimension::Pixels(v) } else { Dimension::Undefined }
}

impl ConstraintsSizeResolver {
    /// The size, once the layout has constraints.
    pub fn size(&self) -> impl Future<Output = CoilSize> + '_ {
        std::future::poll_fn(move |cx: &mut Context<'_>| {
            let c = self.latest.get();
            if is_zero(c) {
                self.waiting.borrow_mut().push(cx.waker().clone());
                Poll::Pending
            } else {
                Poll::Ready(CoilSize { width: dimension(c.max_width), height: dimension(c.max_height) })
            }
        })
    }
    /// The layout's constraints now; who waits for them is told.
    pub fn set_constraints(&self, constraints: Constraints) {
        self.latest.set(constraints);
        if !is_zero(constraints) {
            let waiting = std::mem::take(&mut *self.waiting.borrow_mut());
            waiting.into_iter().for_each(Waker::wake);
        }
    }
}

/// A size resolver of the layout it is a modifier of (Coil's `rememberConstraintsSizeResolver()`).
#[kompozable]
pub fn remember_constraints_size_resolver() -> Rc<ConstraintsSizeResolver> {
    remember(|| Rc::new(ConstraintsSizeResolver::default()))
}

/// What a content scale fits by (Coil's `ContentScale.toScale()`): `Fit` and `Inside` fit, the rest fill.
pub fn content_scale_to_scale(content_scale: &ContentScale) -> Scale {
    if *content_scale == ContentScale::Fit || *content_scale == ContentScale::Inside { Scale::Fit } else { Scale::Fill }
}

// ---- The painter's states -------------------------------------------------------------------------------------------

/// What an [`AsyncImagePainter`] is at (Coil's `AsyncImagePainter.State`): nothing asked yet, loading (its
/// placeholder), loaded, failed (its error image). Equal when of one kind with the same painter (by instance) and
/// result.
///
/// Kotlin: `AsyncImagePainter.State`
#[derive(Clone)]
pub enum AsyncImagePainterState {
    /// Kotlin: `State.Empty`
    Empty,
    /// Kotlin: `State.Loading(painter: Painter?)`
    Loading { painter: Option<Rc<dyn Painter>> },
    /// Kotlin: `State.Success(painter: Painter, result: SuccessResult)`
    Success { painter: Rc<dyn Painter>, result: SuccessResult },
    /// Kotlin: `State.Error(painter: Painter?, result: ErrorResult)`
    Error { painter: Option<Rc<dyn Painter>>, result: ErrorResult },
}

impl AsyncImagePainterState {
    /// Its painter (Coil's `painter`).
    pub fn painter(&self) -> Option<Rc<dyn Painter>> {
        match self {
            AsyncImagePainterState::Empty => None,
            AsyncImagePainterState::Loading { painter } | AsyncImagePainterState::Error { painter, .. } => {
                painter.clone()
            }
            AsyncImagePainterState::Success { painter, .. } => Some(painter.clone()),
        }
    }
    /// The same state with another painter (Kotlin's data class `copy(painter = …)`).
    pub fn with_painter(&self, painter: Option<Rc<dyn Painter>>) -> AsyncImagePainterState {
        match self {
            AsyncImagePainterState::Empty => AsyncImagePainterState::Empty,
            AsyncImagePainterState::Loading { .. } => AsyncImagePainterState::Loading { painter },
            AsyncImagePainterState::Success { painter: old, result } => AsyncImagePainterState::Success {
                painter: painter.unwrap_or_else(|| old.clone()),
                result: result.clone(),
            },
            AsyncImagePainterState::Error { result, .. } => {
                AsyncImagePainterState::Error { painter, result: result.clone() }
            }
        }
    }
    pub fn is_loading(&self) -> bool {
        matches!(self, AsyncImagePainterState::Loading { .. })
    }
    pub fn is_success(&self) -> bool {
        matches!(self, AsyncImagePainterState::Success { .. })
    }
    pub fn is_error(&self) -> bool {
        matches!(self, AsyncImagePainterState::Error { .. })
    }
}

fn same_painter(a: &Option<Rc<dyn Painter>>, b: &Option<Rc<dyn Painter>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for AsyncImagePainterState {
    fn eq(&self, other: &AsyncImagePainterState) -> bool {
        use AsyncImagePainterState::*;
        match (self, other) {
            (Empty, Empty) => true,
            (Loading { painter: a }, Loading { painter: b }) => same_painter(a, b),
            (Success { painter: a, result: r }, Success { painter: b, result: s }) => Rc::ptr_eq(a, b) && r == s,
            (Error { painter: a, result: r }, Error { painter: b, result: s }) => same_painter(a, b) && r == s,
            _ => false,
        }
    }
}

impl fmt::Debug for AsyncImagePainterState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AsyncImagePainterState::Empty => f.write_str("Empty"),
            AsyncImagePainterState::Loading { painter } => write!(f, "Loading(painter={})", painter.is_some()),
            AsyncImagePainterState::Success { result, .. } => write!(f, "Success({:?})", result.data_source),
            AsyncImagePainterState::Error { result, .. } => write!(f, "Error({})", result.throwable),
        }
    }
}

/// What turns a state into the one shown (Coil's `transform: (State) -> State`).
pub type StateTransform = Rc<dyn Fn(AsyncImagePainterState) -> AsyncImagePainterState>;
/// What is told each state (Coil's `onState: ((State) -> Unit)?`).
pub type StateListener = Rc<dyn Fn(&AsyncImagePainterState)>;

/// The identity transform (Coil's `AsyncImagePainter.DefaultTransform`).
pub fn default_transform() -> StateTransform {
    thread_local! {
        static DEFAULT: StateTransform = Rc::new(|s| s);
    }
    DEFAULT.with(Rc::clone)
}

/// Placeholder, error and fallback painters as a transform (Coil's internal `transformOf`).
pub fn transform_of(
    placeholder: Option<Rc<dyn Painter>>,
    error: Option<Rc<dyn Painter>>,
    fallback: Option<Rc<dyn Painter>>,
) -> StateTransform {
    if placeholder.is_none() && error.is_none() && fallback.is_none() {
        return default_transform();
    }
    Rc::new(move |state| match &state {
        AsyncImagePainterState::Loading { .. } if placeholder.is_some() => state.with_painter(placeholder.clone()),
        AsyncImagePainterState::Error { result, .. } => {
            let replacement =
                if result.throwable == ImageError::NullRequestData { fallback.clone() } else { error.clone() };
            if replacement.is_some() { state.with_painter(replacement) } else { state }
        }
        _ => state,
    })
}

/// The three listeners as one (Coil's internal `onStateOf`).
pub fn on_state_of(
    on_loading: Option<StateListener>,
    on_success: Option<StateListener>,
    on_error: Option<StateListener>,
) -> Option<StateListener> {
    if on_loading.is_none() && on_success.is_none() && on_error.is_none() {
        return None;
    }
    Some(Rc::new(move |state| {
        let listener = match state {
            AsyncImagePainterState::Loading { .. } => &on_loading,
            AsyncImagePainterState::Success { .. } => &on_success,
            AsyncImagePainterState::Error { .. } => &on_error,
            AsyncImagePainterState::Empty => &None,
        };
        if let Some(l) = listener {
            l(state);
        }
    }))
}

// ---- The cross-fade ---------------------------------------------------------------------------------------------

/// The time cross-fades go by, ms: the frame clock's (the main dispatcher's).
fn now_millis() -> f64 {
    Dispatcher::main().now() as f64 / 1e6
}

/// A painter fading from `start` into `end` over `duration` ms from when it is first drawn (Coil's
/// `CrossfadePainter`): each drawn at its alpha in the middle of the size, scaled by `content_scale`; drawn
/// again each frame until done.
pub struct CrossfadePainter {
    start: RefCell<Option<Rc<dyn Painter>>>,
    end: Option<Rc<dyn Painter>>,
    content_scale: ContentScale,
    duration_millis: i32,
    fade_start: bool,
    intrinsic_size: Size,
    start_time: Cell<Option<f64>>,
    is_done: Cell<bool>,
    invalidate_tick: MutableState<i32>,
}

impl CrossfadePainter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start: Option<Rc<dyn Painter>>,
        end: Option<Rc<dyn Painter>>,
        content_scale: ContentScale,
        duration_millis: i32,
        fade_start: bool,
        prefer_exact_intrinsic_size: bool,
        prefer_end_first_intrinsic_size: bool,
    ) -> CrossfadePainter {
        let size_of = |p: &Option<Rc<dyn Painter>>| p.as_ref().map_or(Size::Zero, |p| p.intrinsic_size());
        let (s, e) = (size_of(&start), size_of(&end));
        let specified = |z: Size| z.is_specified();
        let intrinsic_size = if prefer_end_first_intrinsic_size && specified(e) {
            e
        } else if prefer_end_first_intrinsic_size && specified(s) {
            s
        } else if specified(s) && specified(e) {
            Size::new(s.width.max(e.width), s.height.max(e.height))
        } else if prefer_exact_intrinsic_size && specified(s) {
            s
        } else if prefer_exact_intrinsic_size && specified(e) {
            e
        } else {
            Size::Unspecified
        };
        CrossfadePainter {
            start: RefCell::new(start),
            end,
            content_scale,
            duration_millis,
            fade_start,
            intrinsic_size,
            start_time: Cell::new(None),
            is_done: Cell::new(false),
            invalidate_tick: mutable_confined_state_of_with_policy(0, never_equal_policy()),
        }
    }
    pub fn start(&self) -> Option<Rc<dyn Painter>> {
        self.start.borrow().clone()
    }
    pub fn end(&self) -> Option<Rc<dyn Painter>> {
        self.end.clone()
    }

    fn draw_painter(
        &self,
        scope: &mut DrawScope<'_>,
        painter: &Option<Rc<dyn Painter>>,
        alpha: f32,
        color_filter: Option<&ColorFilter>,
    ) {
        let Some(painter) = painter else { return };
        if alpha <= 0.0 {
            return;
        }
        let size = scope.size();
        let src = painter.intrinsic_size();
        let draw_size = if !src.is_specified() || src.is_empty() || !size.is_specified() || size.is_empty() {
            size
        } else {
            let f = self.content_scale.compute_scale_factor(src, size);
            Size::new(src.width * f.scale_x, src.height * f.scale_y)
        };
        if !size.is_specified() || size.is_empty() {
            painter.draw(scope, draw_size, alpha, color_filter);
        } else {
            let (h, v) = ((size.width - draw_size.width) / 2.0, (size.height - draw_size.height) / 2.0);
            scope.inset(h, v, h, v, |s| painter.draw(s, draw_size, alpha, color_filter));
        }
    }
}

impl Painter for CrossfadePainter {
    fn intrinsic_size(&self) -> Size {
        self.intrinsic_size
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, max_alpha: f32, color_filter: Option<&ColorFilter>) {
        let _ = self.invalidate_tick.get(); // (read: the draw comes again when it ticks)
        if self.is_done.get() {
            self.draw_painter(scope, &self.end, max_alpha, color_filter);
            return;
        }
        let now = now_millis();
        let start_time = match self.start_time.get() {
            Some(t) => t,
            None => {
                self.start_time.set(Some(now));
                now
            }
        };
        let percent = ((now - start_time).floor() / self.duration_millis as f64) as f32;
        let end_alpha = percent.clamp(0.0, 1.0) * max_alpha;
        let start_alpha = if self.fade_start { max_alpha - end_alpha } else { max_alpha };
        self.is_done.set(percent >= 1.0);
        let start = self.start.borrow().clone();
        self.draw_painter(scope, &start, start_alpha, color_filter);
        self.draw_painter(scope, &self.end, end_alpha, color_filter);
        if self.is_done.get() {
            *self.start.borrow_mut() = None;
        } else {
            self.invalidate_tick.set(self.invalidate_tick.peek() + 1);
        }
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A cross-fade into a successful load that didn't come from the memory cache, where the request asks for one
/// (Coil's `maybeNewCrossfadePainter`): from the loading state's painter.
fn maybe_new_crossfade_painter(
    previous: &AsyncImagePainterState,
    current: &AsyncImagePainterState,
    content_scale: &ContentScale,
) -> Option<Rc<dyn Painter>> {
    let AsyncImagePainterState::Success { painter, result } = current else { return None };
    if result.data_source == DataSource::MemoryCache || result.request.crossfade_millis <= 0 {
        return None;
    }
    let start = if previous.is_loading() { previous.painter() } else { None };
    Some(Rc::new(CrossfadePainter::new(
        start,
        Some(painter.clone()),
        content_scale.clone(),
        result.request.crossfade_millis,
        !result.is_placeholder_cached,
        false,
        result.request.prefer_end_first_intrinsic_size,
    )))
}

// ---- The painter ----------------------------------------------------------------------------------------------

/// What a painter loads (Coil's `AsyncImagePainter.Input`): its loader and request.
#[derive(Clone, PartialEq, Debug)]
pub struct Input {
    pub image_loader: ImageLoader,
    pub request: ImageRequest,
}

/// A painter that loads its image (Coil's `AsyncImagePainter`): once remembered (or attached, in an `AsyncImage`)
/// it runs its request, the states it goes through ([`state`](AsyncImagePainter::state)) transformed and told,
/// and paints the current state's painter (a cross-fade into a fresh load). Its intrinsic size its painter's.
pub struct AsyncImagePainter {
    painter: MutableState<Option<Rc<dyn Painter>>>,
    state: MutableState<AsyncImagePainterState>,
    is_remembered: Cell<bool>,
    remember_job: RefCell<Option<Job>>,
    pub(crate) scope: RefCell<Option<CoroutineScope>>,
    pub(crate) transform: RefCell<StateTransform>,
    pub(crate) on_state: RefCell<Option<StateListener>>,
    pub(crate) content_scale: RefCell<ContentScale>,
    pub(crate) filter_quality: Cell<FilterQuality>,
    input: RefCell<Option<Input>>,
    this: RefCell<std::rc::Weak<AsyncImagePainter>>,
}

impl AsyncImagePainter {
    pub(crate) fn new(input: Input) -> Rc<AsyncImagePainter> {
        let painter = Rc::new(AsyncImagePainter {
            painter: mutable_confined_state_of_with_policy(None, never_equal_policy()),
            state: mutable_confined_state_of_with_policy(AsyncImagePainterState::Empty, never_equal_policy()),
            is_remembered: Cell::new(false),
            remember_job: RefCell::new(None),
            scope: RefCell::new(None),
            transform: RefCell::new(default_transform()),
            on_state: RefCell::new(None),
            content_scale: RefCell::new(ContentScale::Fit),
            filter_quality: Cell::new(DefaultFilterQuality),
            input: RefCell::new(Some(input)),
            this: RefCell::new(std::rc::Weak::new()),
        });
        *painter.this.borrow_mut() = Rc::downgrade(&painter);
        painter
    }

    /// Its state now (Coil's `state.value`; read: who reads it is told of the next).
    pub fn state(&self) -> AsyncImagePainterState {
        self.state.get()
    }

    /// Its state as a state (Coil's `state.collectAsState()`).
    pub fn state_flow(&self) -> MutableState<AsyncImagePainterState> {
        self.state.clone()
    }

    /// What it loads (Coil's `input.value`).
    pub fn input(&self) -> Option<Input> {
        self.input.borrow().clone()
    }

    /// Sets what it loads: another input restarts it (Coil's `_input` setter).
    pub(crate) fn set_input(&self, input: Option<Input>) {
        if *self.input.borrow() == input {
            return;
        }
        *self.input.borrow_mut() = input;
        self.restart();
    }

    /// Runs its request again (Coil's `restart()`).
    pub fn restart(&self) {
        if self.input.borrow().is_none() {
            self.set_job(None);
        } else if self.is_remembered.get() {
            self.launch_job();
        }
    }

    fn set_job(&self, job: Option<Job>) {
        if let Some(old) = std::mem::replace(&mut *self.remember_job.borrow_mut(), job) {
            old.cancel();
        }
    }

    pub(crate) fn remembered(&self) {
        self.launch_job();
        self.is_remembered.set(true);
    }

    pub(crate) fn forgotten(&self) {
        self.set_job(None);
        self.is_remembered.set(false);
    }

    fn launch_job(&self) {
        let Some(input) = self.input.borrow().clone() else { return };
        let Some(scope) = self.scope.borrow().clone() else { return };
        let this = self.this.borrow().clone();
        // (Coil's launchWithDeferredDispatch: at once, up to its first suspension)
        let job = kompoz_runtime::named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
            let Some(me) = this.upgrade() else { return };
            let request = me.update_request(&input.request);
            let weak = Rc::downgrade(&me);
            drop(me);
            let on_start = move |placeholder: Option<crate::request::Image>, _cached: bool| {
                if let Some(me) = weak.upgrade() {
                    let filter_quality = me.filter_quality.get();
                    let mut painter = placeholder.map(|p| p.as_painter(filter_quality));
                    if painter.is_none() && request_uses_existing(&me) {
                        painter = me.painter.peek();
                    }
                    me.update_state(AsyncImagePainterState::Loading { painter });
                }
            };
            let result = input.image_loader.execute_with(request, &on_start).await;
            if let Some(me) = this.upgrade() {
                let state = me.to_state(result);
                me.update_state(state);
            }
        }));
        self.set_job(Some(job));
    }

    /// The request as the painter runs it (Coil's `updateRequest`): the original size unless one is set, the
    /// content scale's scale, inexact.
    fn update_request(&self, request: &ImageRequest) -> ImageRequest {
        let mut r = request.clone();
        if r.size_resolver.is_none() {
            r.size_resolver = Some(SizeResolver::ORIGINAL);
        }
        if r.scale.is_none() {
            r.scale = Some(content_scale_to_scale(&self.content_scale.borrow()));
        }
        if r.precision.is_none() {
            r.precision = Some(Precision::Inexact);
        }
        r
    }

    fn to_state(&self, result: ImageResult) -> AsyncImagePainterState {
        let q = self.filter_quality.get();
        match result {
            ImageResult::Success(s) => AsyncImagePainterState::Success { painter: s.image.as_painter(q), result: s },
            ImageResult::Error(e) => {
                AsyncImagePainterState::Error { painter: e.image.as_ref().map(|i| i.as_painter(q)), result: e }
            }
        }
    }

    fn update_state(&self, state: AsyncImagePainterState) {
        let previous = self.state.peek();
        let transform = self.transform.borrow().clone();
        let current = transform(state);
        self.state.set(current.clone());
        let painter = maybe_new_crossfade_painter(&previous, &current, &self.content_scale.borrow())
            .or_else(|| current.painter());
        self.painter.set(painter);
        let on_state = self.on_state.borrow().clone();
        if let Some(listener) = on_state {
            listener(&current);
        }
    }

    /// The painter it paints now (the state's, or a cross-fade).
    pub fn current_painter(&self) -> Option<Rc<dyn Painter>> {
        self.painter.get()
    }
}

fn request_uses_existing(painter: &AsyncImagePainter) -> bool {
    painter.input.borrow().as_ref().is_some_and(|i| i.request.use_existing_image_as_placeholder)
}

impl Painter for AsyncImagePainter {
    fn intrinsic_size(&self) -> Size {
        self.painter.get().map_or(Size::Unspecified, |p| p.intrinsic_size())
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        if let Some(p) = self.painter.get() {
            let size = scope.size();
            p.draw(scope, size, alpha, color_filter);
        }
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The remembered painter's lifecycle (Compose's RememberObserver of Coil's painter).
struct RememberedPainter(Rc<AsyncImagePainter>);

impl RememberObserver for RememberedPainter {
    fn on_remembered(&self) {
        self.0.remembered();
    }
    fn on_forgotten(&self) {
        self.0.forgotten();
    }
    fn on_abandoned(&self) {
        self.0.forgotten();
    }
}

/// The request of a model (Coil's internal `requestOf`).
#[kompozable]
fn request_of(model: ImageRequest) -> ImageRequest {
    model
}

/// A painter of `model` loaded with `image_loader` (Coil's `rememberAsyncImagePainter(model, imageLoader,
/// transform, onState, contentScale, filterQuality)`): it loads once remembered, at the original size unless the
/// request sets one; another model restarts it.
///
/// Kotlin: `rememberAsyncImagePainter(model: Any?, imageLoader: ImageLoader, transform: (State) -> State = DefaultTransform, onState: ((State) -> Unit)? = null, contentScale: ContentScale = ContentScale.Fit, filterQuality: FilterQuality = DefaultFilterQuality)`
#[kompozable]
pub fn remember_async_image_painter_with(
    model: impl Into<ImageRequest>,
    image_loader: ImageLoader,
    #[default(default_transform())] transform: StateTransform,
    #[default(None)] on_state: Option<StateListener>,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
) -> Rc<AsyncImagePainter> {
    let request = request_of(model.into());
    let input = Input { image_loader, request };
    let i = input.clone();
    let remembered = kompoz_runtime::remember_observer(move || RememberedPainter(AsyncImagePainter::new(i)));
    let painter = remembered.0.clone();
    *painter.scope.borrow_mut() = Some(remember_coroutine_scope());
    *painter.transform.borrow_mut() = transform;
    *painter.on_state.borrow_mut() = on_state;
    *painter.content_scale.borrow_mut() = content_scale;
    painter.filter_quality.set(filter_quality);
    painter.set_input(Some(input));
    painter
}

/// A painter of `model` loaded by the platform's singleton loader (coil-compose's `rememberAsyncImagePainter(model,
/// placeholder, error, fallback, onLoading, onSuccess, onError, contentScale, filterQuality)`): the placeholder
/// while it loads, the error image if it fails, the fallback for no data.
///
/// Kotlin: `rememberAsyncImagePainter(model: Any?, placeholder: Painter? = null, error: Painter? = null, fallback: Painter? = error, onLoading: ((State.Loading) -> Unit)? = null, onSuccess: ((State.Success) -> Unit)? = null, onError: ((State.Error) -> Unit)? = null, contentScale: ContentScale = ContentScale.Fit, filterQuality: FilterQuality = DefaultFilterQuality)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn remember_async_image_painter(
    model: impl Into<ImageRequest>,
    #[default(None)] placeholder: Option<Rc<dyn Painter>>,
    #[default(None)] error: Option<Rc<dyn Painter>>,
    #[default(None)] fallback: Option<Rc<dyn Painter>>,
    #[default(None)] on_loading: Option<StateListener>,
    #[default(None)] on_success: Option<StateListener>,
    #[default(None)] on_error: Option<StateListener>,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
) -> Rc<AsyncImagePainter> {
    let loader = SingletonImageLoader::get(&LocalPlatformContext.current());
    let fallback = fallback.or_else(|| error.clone());
    remember_async_image_painter_with(
        model,
        loader,
        transform = transform_of(placeholder, error, fallback),
        on_state = on_state_of(on_loading, on_success, on_error),
        content_scale = content_scale,
        filter_quality = filter_quality,
    )
}

// ---- AsyncImage --------------------------------------------------------------------------------------------------

/// Coil's `UseMinConstraintsMeasurePolicy`: as small as it may be.
struct UseMinConstraints;

impl MeasurePolicy for UseMinConstraints {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        scope.layout(constraints.min_width, constraints.min_height, |_| {})
    }
}

fn use_min_constraints() -> Rc<dyn MeasurePolicy> {
    thread_local! {
        static POLICY: Rc<dyn MeasurePolicy> = Rc::new(UseMinConstraints);
    }
    POLICY.with(Rc::clone)
}

/// The request with a size resolver: the one it has, else (Coil's `requestOfWithSizeResolver`) the original size
/// for `ContentScale.None`, else the layout's constraints.
#[kompozable]
fn request_of_with_size_resolver(model: ImageRequest, content_scale: &ContentScale) -> ImageRequest {
    if model.size_resolver.is_some() {
        return model;
    }
    let is_none = *content_scale == ContentScale::None;
    let resolver = remember_keyed(is_none, move || {
        if is_none {
            SizeResolver::ORIGINAL
        } else {
            SizeResolver::Constraints(Rc::new(ConstraintsSizeResolver::default()))
        }
    });
    remember_keyed((model.clone(), resolver.clone()), move || ImageRequest { size_resolver: Some(resolver), ..model })
}

/// Coil's `AbstractContentPainterNode` sizing and drawing.
struct ContentPainterCore {
    painter: Rc<dyn Painter>,
    async_painter: Option<Rc<AsyncImagePainter>>,
    alignment: Alignment,
    content_scale: ContentScale,
    alpha: f32,
    color_filter: Option<ColorFilter>,
    clip_to_bounds: bool,
    content_description: Option<String>,
    constraint_size_resolver: Option<Rc<ConstraintsSizeResolver>>,
}

fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

impl ContentPainterCore {
    fn calculate_scaled_size(&self, dst: Size) -> Size {
        if dst.is_empty() {
            return Size::Zero;
        }
        let intrinsic = self.painter.intrinsic_size();
        if !intrinsic.is_specified() {
            return dst;
        }
        let src = Size::new(
            if intrinsic.width.is_finite() { intrinsic.width } else { dst.width },
            if intrinsic.height.is_finite() { intrinsic.height } else { dst.height },
        );
        let f = self.content_scale.compute_scale_factor(src, dst);
        if !f.scale_x.is_finite() || !f.scale_y.is_finite() {
            return dst;
        }
        Size::new(src.width * f.scale_x, src.height * f.scale_y)
    }

    fn modify_constraints(&self, c: Constraints) -> Constraints {
        let (fixed_w, fixed_h) = (c.has_fixed_width(), c.has_fixed_height());
        if fixed_w && fixed_h {
            return c;
        }
        let bounded = c.has_bounded_width() && c.has_bounded_height();
        let intrinsic = self.painter.intrinsic_size();
        if !intrinsic.is_specified() {
            let no_painter = self.async_painter.as_ref().is_some_and(|p| p.state.peek().painter().is_none());
            return if !bounded || no_painter {
                c
            } else {
                Constraints::new(c.max_width, c.max_width, c.max_height, c.max_height)
            };
        }
        let constrain_w = |w: f32| w.clamp(c.min_width as f32, c.max_width as f32);
        let constrain_h = |h: f32| h.clamp(c.min_height as f32, c.max_height as f32);
        let (dw, dh) = if bounded && (fixed_w || fixed_h) {
            (c.max_width as f32, c.max_height as f32)
        } else {
            (
                if intrinsic.width.is_finite() { constrain_w(intrinsic.width) } else { c.min_width as f32 },
                if intrinsic.height.is_finite() { constrain_h(intrinsic.height) } else { c.min_height as f32 },
            )
        };
        let scaled = self.calculate_scaled_size(Size::new(dw, dh));
        Constraints::new(
            c.constrain_width(round(scaled.width)),
            c.max_width,
            c.constrain_height(round(scaled.height)),
            c.max_height,
        )
    }

    fn measure(&self, scope: &MeasureScope, measurable: &dyn Measurable, constraints: Constraints) -> MeasureResult {
        if let Some(r) = &self.constraint_size_resolver {
            r.set_constraints(constraints);
        }
        let placeable = measurable.measure(self.modify_constraints(constraints));
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn intrinsic(&self, constraints: Constraints, of_measurable: i32, width: bool) -> i32 {
        if let Some(r) = &self.constraint_size_resolver {
            r.set_constraints(constraints);
        }
        if self.painter.intrinsic_size().is_specified() {
            let m = self.modify_constraints(constraints);
            of_measurable.max(if width { m.min_width } else { m.min_height })
        } else {
            of_measurable
        }
    }

    fn draw(&self, scope: &mut ContentDrawScope) {
        let size = scope.size();
        let scaled = self.calculate_scaled_size(size);
        let at = self.alignment.align(
            IntSize::new(round(scaled.width), round(scaled.height)),
            IntSize::new(round(size.width), round(size.height)),
            scope.layout_direction(),
        );
        let (painter, alpha, cf, clip) = (self.painter.clone(), self.alpha, self.color_filter, self.clip_to_bounds);
        let paint = move |s: &mut DrawScope<'_>| {
            s.translate_with(at.x as f32, at.y as f32, |s| painter.draw(s, scaled, alpha, cf.as_ref()));
        };
        if clip {
            scope.clip_rect_with(
                0.0,
                0.0,
                size.width,
                size.height,
                kompoz_ui::graphics_context::ClipOp::Intersect,
                |s| paint(s),
            );
        } else {
            paint(scope);
        }
        scope.draw_content();
    }

    fn semantics(&self, config: &mut SemanticsConfiguration) {
        if let Some(d) = &self.content_description {
            config.set_content_description(d.clone());
            config.set_role(Role::Image);
        }
    }
}

/// `AsyncImage`'s modifier element (Coil's `ContentPainterElement`): a painter of its own, made with the node.
struct ContentPainterElement {
    request: ImageRequest,
    image_loader: ImageLoader,
    transform: StateTransform,
    on_state: Option<StateListener>,
    filter_quality: FilterQuality,
    alignment: Alignment,
    content_scale: ContentScale,
    alpha: f32,
    color_filter: Option<ColorFilter>,
    clip_to_bounds: bool,
    content_description: Option<String>,
}

impl PartialEq for ContentPainterElement {
    fn eq(&self, o: &Self) -> bool {
        self.request == o.request
            && self.image_loader == o.image_loader
            && Rc::ptr_eq(&self.transform, &o.transform)
            && match (&self.on_state, &o.on_state) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && self.filter_quality == o.filter_quality
            && self.alignment == o.alignment
            && self.content_scale == o.content_scale
            && self.alpha == o.alpha
            && self.color_filter == o.color_filter
            && self.clip_to_bounds == o.clip_to_bounds
            && self.content_description == o.content_description
    }
}

impl ContentPainterElement {
    fn configure(&self, painter: &AsyncImagePainter) {
        *painter.transform.borrow_mut() = self.transform.clone();
        *painter.on_state.borrow_mut() = self.on_state.clone();
        *painter.content_scale.borrow_mut() = self.content_scale.clone();
        painter.filter_quality.set(self.filter_quality);
    }
    fn constraint_resolver(&self) -> Option<Rc<ConstraintsSizeResolver>> {
        match &self.request.size_resolver {
            Some(SizeResolver::Constraints(r)) => Some(r.clone()),
            _ => None,
        }
    }
}

impl ModifierNodeElement for ContentPainterElement {
    type Node = ContentPainterNode;
    fn create(&self) -> ContentPainterNode {
        let input = Input { image_loader: self.image_loader.clone(), request: self.request.clone() };
        let painter = AsyncImagePainter::new(input);
        self.configure(&painter);
        ContentPainterNode {
            core: ContentPainterCore {
                painter: painter.clone(),
                async_painter: Some(painter.clone()),
                alignment: self.alignment.clone(),
                content_scale: self.content_scale.clone(),
                alpha: self.alpha,
                color_filter: self.color_filter,
                clip_to_bounds: self.clip_to_bounds,
                content_description: self.content_description.clone(),
                constraint_size_resolver: self.constraint_resolver(),
            },
            painter,
            cx: None,
        }
    }
    fn update(&self, node: &mut ContentPainterNode) {
        let previous_intrinsics = node.painter.intrinsic_size();
        let previous_resolver = node.core.constraint_size_resolver.clone();
        self.configure(&node.painter);
        node.painter.set_input(Some(Input { image_loader: self.image_loader.clone(), request: self.request.clone() }));
        let intrinsics_changed = previous_intrinsics != node.painter.intrinsic_size();
        node.core.alignment = self.alignment.clone();
        node.core.constraint_size_resolver = self.constraint_resolver();
        node.core.content_scale = self.content_scale.clone();
        node.core.alpha = self.alpha;
        node.core.color_filter = self.color_filter;
        node.core.clip_to_bounds = self.clip_to_bounds;
        let cx = node.cx.clone();
        if node.core.content_description != self.content_description {
            node.core.content_description = self.content_description.clone();
            if let Some(cx) = &cx {
                cx.invalidate_semantics();
            }
        }
        let resolver_changed = match (&previous_resolver, &node.core.constraint_size_resolver) {
            (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        };
        if let Some(cx) = &cx {
            if intrinsics_changed || resolver_changed {
                cx.invalidate_measurement();
            }
            cx.invalidate_draw();
        }
    }
    fn name(&self) -> &'static str {
        "content"
    }
}

/// `AsyncImage`'s node (Coil's `ContentPainterNode`): its painter remembered while it is attached.
struct ContentPainterNode {
    core: ContentPainterCore,
    painter: Rc<AsyncImagePainter>,
    cx: Option<NodeCx>,
}

impl ModifierNode for ContentPainterNode {
    node_capabilities!(layout, draw, semantics);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
        *self.painter.scope.borrow_mut() = Some(cx.coroutine_scope());
        self.painter.remembered();
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.painter.forgotten();
        self.cx = None;
    }
    fn on_reset(&mut self) {
        self.painter.set_input(None);
    }
}

macro_rules! content_painter_node {
    ($node:ty) => {
        impl LayoutModifierNode for $node {
            fn measure(&mut self, scope: &MeasureScope, measurable: &dyn Measurable, c: Constraints) -> MeasureResult {
                self.core.measure(scope, measurable, c)
            }
            fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, h: i32) -> i32 {
                let c = Constraints::new(0, Constraints::Infinity, 0, h);
                self.core.intrinsic(c, m.min_intrinsic_width(h), true)
            }
            fn max_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, h: i32) -> i32 {
                let c = Constraints::new(0, Constraints::Infinity, 0, h);
                self.core.intrinsic(c, m.max_intrinsic_width(h), true)
            }
            fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, w: i32) -> i32 {
                let c = Constraints::new(0, w, 0, Constraints::Infinity);
                self.core.intrinsic(c, m.min_intrinsic_height(w), false)
            }
            fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, w: i32) -> i32 {
                let c = Constraints::new(0, w, 0, Constraints::Infinity);
                self.core.intrinsic(c, m.max_intrinsic_height(w), false)
            }
        }

        impl DrawModifierNode for $node {
            fn draw(&mut self, scope: &mut ContentDrawScope) {
                self.core.draw(scope);
            }
        }

        impl SemanticsModifierNode for $node {
            fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
                self.core.semantics(config);
            }
        }
    };
}

content_painter_node!(ContentPainterNode);

/// An image loaded from `model` with `image_loader` and painted (Coil's `AsyncImage(model, contentDescription,
/// imageLoader, modifier, transform, onState, alignment, contentScale, alpha, colorFilter, filterQuality,
/// clipToBounds)`): loaded at the size its constraints give (the original size for `ContentScale.None`), sized
/// as `Image` sizes a painter, described to accessibility as an image.
///
/// Kotlin: `AsyncImage(model: Any?, contentDescription: String?, imageLoader: ImageLoader, modifier: Modifier = Modifier, transform: (State) -> State = DefaultTransform, onState: ((State) -> Unit)? = null, alignment: Alignment = Alignment.Center, contentScale: ContentScale = ContentScale.Fit, alpha: Float = DefaultAlpha, colorFilter: ColorFilter? = null, filterQuality: FilterQuality = DefaultFilterQuality, clipToBounds: Boolean = true)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AsyncImageWith(
    model: impl Into<ImageRequest>,
    content_description: Option<String>,
    image_loader: ImageLoader,
    #[default(Modifier)] modifier: Modifier,
    #[default(default_transform())] transform: StateTransform,
    #[default(None)] on_state: Option<StateListener>,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
    #[default(true)] clip_to_bounds: bool,
) {
    let request = request_of_with_size_resolver(model, &content_scale);
    let element = ContentPainterElement {
        request,
        image_loader,
        transform,
        on_state,
        filter_quality,
        alignment,
        content_scale,
        alpha,
        color_filter,
        clip_to_bounds,
        content_description,
    };
    layout_into(modifier.then_element(element), use_min_constraints(), false, || {});
}

/// An image loaded from `model` by the platform's singleton loader and painted (coil-compose's `AsyncImage(model,
/// contentDescription, modifier, placeholder, error, fallback, onLoading, onSuccess, onError, alignment,
/// contentScale, alpha, colorFilter, filterQuality, clipToBounds)`): the placeholder painter while it loads, the
/// error one if it fails, the fallback for no data.
///
/// Kotlin: `AsyncImage(model: Any?, contentDescription: String?, modifier: Modifier = Modifier, placeholder: Painter? = null, error: Painter? = null, fallback: Painter? = error, onLoading: ((State.Loading) -> Unit)? = null, onSuccess: ((State.Success) -> Unit)? = null, onError: ((State.Error) -> Unit)? = null, alignment: Alignment = Alignment.Center, contentScale: ContentScale = ContentScale.Fit, alpha: Float = DefaultAlpha, colorFilter: ColorFilter? = null, filterQuality: FilterQuality = DefaultFilterQuality, clipToBounds: Boolean = true)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AsyncImage(
    model: impl Into<ImageRequest>,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] placeholder: Option<Rc<dyn Painter>>,
    #[default(None)] error: Option<Rc<dyn Painter>>,
    #[default(None)] fallback: Option<Rc<dyn Painter>>,
    #[default(None)] on_loading: Option<StateListener>,
    #[default(None)] on_success: Option<StateListener>,
    #[default(None)] on_error: Option<StateListener>,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
    #[default(true)] clip_to_bounds: bool,
) {
    let loader = SingletonImageLoader::get(&LocalPlatformContext.current());
    let fallback = fallback.or_else(|| error.clone());
    let transform = remember_keyed(
        (PainterKey(placeholder.clone()), PainterKey(error.clone()), PainterKey(fallback.clone())),
        move || transform_of(placeholder, error, fallback),
    );
    AsyncImageWith(
        model,
        content_description,
        loader,
        modifier = modifier,
        transform = transform,
        on_state = on_state_of(on_loading, on_success, on_error),
        alignment = alignment,
        content_scale = content_scale,
        alpha = alpha,
        color_filter = color_filter,
        filter_quality = filter_quality,
        clip_to_bounds = clip_to_bounds,
    );
}

/// A painter as a key: the same instance, or painters that draw the same.
struct PainterKey(Option<Rc<dyn Painter>>);

impl PartialEq for PainterKey {
    fn eq(&self, o: &PainterKey) -> bool {
        match (&self.0, &o.0) {
            (Some(a), Some(b)) => **a == **b,
            (None, None) => true,
            _ => false,
        }
    }
}

// ---- SubcomposeAsyncImage ---------------------------------------------------------------------------------------

/// What a `SubcomposeAsyncImage`'s content is given (Coil's `SubcomposeAsyncImageScope`, a BoxScope): its painter
/// and how the image is shown.
#[derive(Clone)]
pub struct SubcomposeAsyncImageScope {
    pub painter: Rc<AsyncImagePainter>,
    pub content_description: Option<String>,
    pub alignment: Alignment,
    pub content_scale: ContentScale,
    pub alpha: f32,
    pub color_filter: Option<ColorFilter>,
    pub clip_to_bounds: bool,
}

/// The content of a subcomposed image's state (Coil's `@Composable SubcomposeAsyncImageScope.() -> Unit`).
pub type SubcomposeContent = Rc<dyn Fn(&SubcomposeAsyncImageScope)>;
/// A slot of one state (Coil's `@Composable SubcomposeAsyncImageScope.(State.Loading) -> Unit` …).
pub type SubcomposeSlot = Rc<dyn Fn(&SubcomposeAsyncImageScope, &AsyncImagePainterState)>;

/// `SubcomposeAsyncImageContent`'s element (Coil's `SubcomposeContentPainterElement`).
struct SubcomposeContentPainterElement {
    painter: Rc<dyn Painter>,
    alignment: Alignment,
    content_scale: ContentScale,
    alpha: f32,
    color_filter: Option<ColorFilter>,
    clip_to_bounds: bool,
    content_description: Option<String>,
}

impl PartialEq for SubcomposeContentPainterElement {
    fn eq(&self, o: &Self) -> bool {
        Rc::ptr_eq(&self.painter, &o.painter)
            && self.alignment == o.alignment
            && self.content_scale == o.content_scale
            && self.alpha == o.alpha
            && self.color_filter == o.color_filter
            && self.clip_to_bounds == o.clip_to_bounds
            && self.content_description == o.content_description
    }
}

struct SubcomposeContentPainterNode {
    core: ContentPainterCore,
    cx: Option<NodeCx>,
}

impl ModifierNode for SubcomposeContentPainterNode {
    node_capabilities!(layout, draw, semantics);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.cx = None;
    }
}

content_painter_node!(SubcomposeContentPainterNode);

impl ModifierNodeElement for SubcomposeContentPainterElement {
    type Node = SubcomposeContentPainterNode;
    fn create(&self) -> SubcomposeContentPainterNode {
        SubcomposeContentPainterNode {
            core: ContentPainterCore {
                painter: self.painter.clone(),
                async_painter: None,
                alignment: self.alignment.clone(),
                content_scale: self.content_scale.clone(),
                alpha: self.alpha,
                color_filter: self.color_filter,
                clip_to_bounds: self.clip_to_bounds,
                content_description: self.content_description.clone(),
                constraint_size_resolver: None,
            },
            cx: None,
        }
    }
    fn update(&self, node: &mut SubcomposeContentPainterNode) {
        let intrinsics_changed = node.core.painter.intrinsic_size() != self.painter.intrinsic_size();
        node.core.painter = self.painter.clone();
        node.core.alignment = self.alignment.clone();
        node.core.content_scale = self.content_scale.clone();
        node.core.alpha = self.alpha;
        node.core.color_filter = self.color_filter;
        node.core.clip_to_bounds = self.clip_to_bounds;
        let cx = node.cx.clone();
        if node.core.content_description != self.content_description {
            node.core.content_description = self.content_description.clone();
            if let Some(cx) = &cx {
                cx.invalidate_semantics();
            }
        }
        if let Some(cx) = &cx {
            if intrinsics_changed {
                cx.invalidate_measurement();
            }
            cx.invalidate_draw();
        }
    }
    fn name(&self) -> &'static str {
        "content"
    }
}

/// The image of a `SubcomposeAsyncImage` as its scope says (Coil's `SubcomposeAsyncImageScope.
/// SubcomposeAsyncImageContent(modifier, painter, contentDescription, alignment, contentScale, alpha, colorFilter,
/// clipToBounds)`).
///
/// Kotlin: `SubcomposeAsyncImageContent(modifier: Modifier = Modifier, painter: Painter = this.painter, contentDescription: String? = this.contentDescription, alignment: Alignment = this.alignment, contentScale: ContentScale = this.contentScale, alpha: Float = this.alpha, colorFilter: ColorFilter? = this.colorFilter, clipToBounds: Boolean = this.clipToBounds)`
#[kompozable]
pub fn SubcomposeAsyncImageContent(scope: SubcomposeAsyncImageScope, #[default(Modifier)] modifier: Modifier) {
    subcompose_async_image_content(scope, modifier);
}

#[kompozable]
fn subcompose_async_image_content(scope: SubcomposeAsyncImageScope, modifier: Modifier) {
    let element = SubcomposeContentPainterElement {
        painter: scope.painter.clone(),
        alignment: scope.alignment.clone(),
        content_scale: scope.content_scale.clone(),
        alpha: scope.alpha,
        color_filter: scope.color_filter,
        clip_to_bounds: scope.clip_to_bounds,
        content_description: scope.content_description.clone(),
    };
    layout_into(modifier.then_element(element), use_min_constraints(), false, || {});
}

/// The content of the loading, success and error slots (Coil's internal `contentOf`): a slot for the state if
/// one is given, else the image.
pub fn content_of(
    loading: Option<SubcomposeSlot>,
    success: Option<SubcomposeSlot>,
    error: Option<SubcomposeSlot>,
) -> SubcomposeContent {
    Rc::new(move |scope: &SubcomposeAsyncImageScope| {
        let state = scope.painter.state();
        let slot = match &state {
            AsyncImagePainterState::Loading { .. } => &loading,
            AsyncImagePainterState::Success { .. } => &success,
            AsyncImagePainterState::Error { .. } => &error,
            AsyncImagePainterState::Empty => &None,
        };
        match slot {
            Some(slot) => slot(scope, &state),
            None => subcompose_async_image_content(scope.clone(), Modifier),
        }
    })
}

/// An image whose states compose content (Coil's `SubcomposeAsyncImage(model, contentDescription, imageLoader,
/// modifier, transform, onState, alignment, contentScale, alpha, colorFilter, filterQuality, clipToBounds,
/// content)`): in a box (with its constraints when they give the size), `content` composed with the painter.
///
/// Kotlin: `SubcomposeAsyncImage(model: Any?, contentDescription: String?, imageLoader: ImageLoader, modifier: Modifier = Modifier, transform: (State) -> State = DefaultTransform, onState: ((State) -> Unit)? = null, alignment: Alignment = Alignment.Center, contentScale: ContentScale = ContentScale.Fit, alpha: Float = DefaultAlpha, colorFilter: ColorFilter? = null, filterQuality: FilterQuality = DefaultFilterQuality, clipToBounds: Boolean = true, content: @Composable SubcomposeAsyncImageScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SubcomposeAsyncImageWith(
    model: impl Into<ImageRequest>,
    content_description: Option<String>,
    image_loader: ImageLoader,
    #[default(Modifier)] modifier: Modifier,
    #[default(default_transform())] transform: StateTransform,
    #[default(None)] on_state: Option<StateListener>,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
    #[default(true)] clip_to_bounds: bool,
    content: SubcomposeContent,
) {
    let request = request_of_with_size_resolver(model, &content_scale);
    let resolver = match &request.size_resolver {
        Some(SizeResolver::Constraints(r)) => Some(r.clone()),
        _ => None,
    };
    let painter = remember_async_image_painter_with(
        request,
        image_loader,
        transform = transform,
        on_state = on_state,
        content_scale = content_scale.clone(),
        filter_quality = filter_quality,
    );
    let scope = SubcomposeAsyncImageScope {
        painter,
        content_description,
        alignment: alignment.clone(),
        content_scale,
        alpha,
        color_filter,
        clip_to_bounds,
    };
    match resolver {
        None => {
            Box(modifier, alignment, true, move || content(&scope));
        }
        Some(resolver) => {
            BoxWithConstraints(modifier, alignment, true, move |box_scope| {
                resolver.set_constraints(box_scope.constraints());
                content(&scope);
            });
        }
    }
}

/// The singleton loader's (coil-compose's `SubcomposeAsyncImage(model, contentDescription, modifier, transform,
/// loading, success, error, onLoading, onSuccess, onError, alignment, contentScale, alpha, colorFilter,
/// filterQuality, clipToBounds)`): a slot composed for each state it is given, the image otherwise.
///
/// Kotlin: `SubcomposeAsyncImage(model: Any?, contentDescription: String?, modifier: Modifier = Modifier, transform: (State) -> State = DefaultTransform, loading: @Composable (SubcomposeAsyncImageScope.(State.Loading) -> Unit)? = null, success: @Composable (SubcomposeAsyncImageScope.(State.Success) -> Unit)? = null, error: @Composable (SubcomposeAsyncImageScope.(State.Error) -> Unit)? = null, onLoading: ((State.Loading) -> Unit)? = null, onSuccess: ((State.Success) -> Unit)? = null, onError: ((State.Error) -> Unit)? = null, alignment: Alignment = Alignment.Center, contentScale: ContentScale = ContentScale.Fit, alpha: Float = DefaultAlpha, colorFilter: ColorFilter? = null, filterQuality: FilterQuality = DefaultFilterQuality, clipToBounds: Boolean = true)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SubcomposeAsyncImage(
    model: impl Into<ImageRequest>,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(default_transform())] transform: StateTransform,
    #[default(None)] loading: Option<SubcomposeSlot>,
    #[default(None)] success: Option<SubcomposeSlot>,
    #[default(None)] error: Option<SubcomposeSlot>,
    #[default(None)] on_loading: Option<StateListener>,
    #[default(None)] on_success: Option<StateListener>,
    #[default(None)] on_error: Option<StateListener>,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
    #[default(true)] clip_to_bounds: bool,
) {
    let loader = SingletonImageLoader::get(&LocalPlatformContext.current());
    SubcomposeAsyncImageWith(
        model,
        content_description,
        loader,
        modifier = modifier,
        transform = transform,
        on_state = on_state_of(on_loading, on_success, on_error),
        alignment = alignment,
        content_scale = content_scale,
        alpha = alpha,
        color_filter = color_filter,
        filter_quality = filter_quality,
        clip_to_bounds = clip_to_bounds,
        content = content_of(loading, success, error),
    );
}
