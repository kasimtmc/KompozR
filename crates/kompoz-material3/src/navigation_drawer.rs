//! Material's navigation drawers, as Compose's NavigationDrawer.kt: the drawers ([`ModalNavigationDrawer`] over the
//! content behind a scrim, [`DismissibleNavigationDrawer`] pushing the content aside, [`PermanentNavigationDrawer`]
//! always beside it), their state ([`DrawerState`], [`DrawerValue`], [`remember_drawer_state`]), the sheets inside
//! them ([`ModalDrawerSheet`], [`DismissibleDrawerSheet`], [`PermanentDrawerSheet`]; the drawer state's forms
//! follow a predictive back gesture), their destinations ([`NavigationDrawerItem`], [`NavigationDrawerItemColors`],
//! [`NavigationDrawerItemDefaults`]) and the defaults ([`DrawerDefaults`]).

// (the deprecated members call each other, as Compose's do)
#![allow(deprecated)]

use crate::color_scheme::content_color_for;
use crate::content_color::LocalContentColor;
use crate::internal::back_handler::{BackEventCompat, PredictiveBack, PredictiveBackHandler, SwipeEdge};
use crate::internal::float_producer::FloatProducer;
use crate::internal::strings::{Strings, get_string};
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::scrim::Scrim;
use crate::surface::{SelectableSurface, Surface};
use crate::tokens::{ElevationTokens, MotionSchemeKeyTokens, NavigationDrawerTokens, ScrimTokens};
use kompoz_animation::{AnimationSpec, FiniteAnimationSpec, TweenSpec, animate, snap, tween};
use kompoz_coroutines::{CoroutineScope, Flow, LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::gestures::{
    AnchoredDraggableDefaults, AnchoredDraggableModifiers, AnchoredDraggableState, DraggableAnchors,
    DraggableAnchorsConfig, Orientation, anchored_draggable_state_configured,
};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Density, Dp, IntOffset, LayoutDirection, RectangleShape, Shape, lerp};
use kompoz_runtime::kt::{KtFloat, panic_kt};
use kompoz_runtime::{
    KompozitionLocalProvider, MutableState, Saveable, SavedValue, Saver, SideEffect, State, StateSource, kompozable,
    kompozable_block, mutable_float_state_of, mutable_state_of, named, named_args, remember, remember_keyed,
    without_read_observation,
};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::{Key, KeyEvent, KeyEventType};
use kompoz_ui::layout::{GraphicsLayerScope, Layout, TransformOrigin, measure_policy};
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// DrawerValue

/// The values of a [`DrawerState`] (Compose's enum `DrawerValue`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DrawerValue {
    /// The state of the drawer when it is closed.
    Closed,
    /// The state of the drawer when it is open.
    Open,
}

impl DrawerValue {
    /// Its values in order (Kotlin's `entries`).
    pub const ENTRIES: [DrawerValue; 2] = [DrawerValue::Closed, DrawerValue::Open];
}

/// Its name (Kotlin's enum `toString`).
impl fmt::Display for DrawerValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl Saveable for DrawerValue {
    fn save(&self) -> SavedValue {
        SavedValue::Int(*self as i64)
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        DrawerValue::ENTRIES.get(ordinal).copied()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DrawerState

/// The state of a [`ModalNavigationDrawer`] or a [`DismissibleNavigationDrawer`] (Compose's class `DrawerState`):
/// whether it is open, where its sheet is, and the animations opening and closing it. A cheap handle: clones are the
/// same state (equal when they are).
#[derive(Clone)]
pub struct DrawerState {
    inner: Rc<DrawerStateInner>,
}

struct DrawerStateInner {
    confirm_state_change: Rc<dyn Fn(DrawerValue) -> bool>,
    // (Compose's internal `anchoredDraggableMotionSpec`: the modal drawer sets the theme's; the anchored draggable
    // state was made with its first value, a 256 ms tween, and keeps it, as in Compose)
    anchored_draggable_motion_spec: RefCell<Arc<dyn FiniteAnimationSpec<f32>>>,
    anchored_draggable_state: AnchoredDraggableState<DrawerValue>,
    offset: State<f32>,
    density: MutableState<Option<Density>>,
    open_drawer_motion_spec: RefCell<Arc<dyn FiniteAnimationSpec<f32>>>,
    close_drawer_motion_spec: RefCell<Arc<dyn FiniteAnimationSpec<f32>>>,
}

impl PartialEq for DrawerState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for DrawerState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DrawerState(currentValue={:?}, offset={})", self.current_value(), self.current_offset())
    }
}

/// A drawer's state at `initial_value` (Compose's constructor `DrawerState(initialValue, confirmStateChange = {
/// true })`): `confirm_state_change` may veto a change. In composition, rather [`remember_drawer_state`].
#[named(type)]
pub fn DrawerState(
    initial_value: DrawerValue,
    #[default(|_| true)] confirm_state_change: impl Fn(DrawerValue) -> bool + 'static,
) -> DrawerState {
    DrawerState::new(initial_value, Rc::new(confirm_state_change))
}

impl DrawerState {
    /// [`DrawerState()`](fn@DrawerState) of a shared closure.
    pub(crate) fn new(
        initial_value: DrawerValue,
        confirm_state_change: Rc<dyn Fn(DrawerValue) -> bool>,
    ) -> DrawerState {
        let anchored_draggable_motion_spec = AnchoredDraggableDefaultAnimationSpec();
        let density = mutable_state_of(None::<Density>);
        let inner = Rc::new_cyclic(|this: &Weak<DrawerStateInner>| {
            let confirm = confirm_state_change.clone();
            let (this, threshold_density) = (this.clone(), density.clone());
            let anchored_draggable_state = anchored_draggable_state_configured(
                initial_value,
                |distance: f32| distance * DrawerPositionalThreshold,
                move || {
                    let density = require_density(&threshold_density, &this);
                    density.to_px(DrawerVelocityThreshold)
                },
                anchored_draggable_motion_spec.clone() as Arc<dyn AnimationSpec<f32>>,
                AnchoredDraggableDefaults::DecayAnimationSpec(),
                move |value: &DrawerValue| confirm(*value),
            );
            DrawerStateInner {
                confirm_state_change,
                anchored_draggable_motion_spec: RefCell::new(anchored_draggable_motion_spec),
                offset: State::from_source(OffsetState(anchored_draggable_state.clone())),
                anchored_draggable_state,
                density,
                open_drawer_motion_spec: RefCell::new(Arc::new(snap::<f32>())),
                close_drawer_motion_spec: RefCell::new(Arc::new(snap::<f32>())),
            }
        });
        DrawerState { inner }
    }

    /// Whether the drawer is open (Compose's `isOpen`).
    pub fn is_open(&self) -> bool {
        self.current_value() == DrawerValue::Open
    }

    /// Whether the drawer is closed (Compose's `isClosed`).
    pub fn is_closed(&self) -> bool {
        self.current_value() == DrawerValue::Closed
    }

    /// The value it is at (Compose's `currentValue`): with no swipe or animation going on, where the drawer is;
    /// while one goes on, where it was before it started.
    pub fn current_value(&self) -> DrawerValue {
        self.inner.anchored_draggable_state.settled_value()
    }

    /// Whether an animation runs (Compose's `isAnimationRunning`).
    pub fn is_animation_running(&self) -> bool {
        self.inner.anchored_draggable_state.is_animation_running()
    }

    /// Opens the drawer, animating by the theme's motion, done when it is open or the animation is cancelled
    /// (Compose's suspending `open`).
    pub fn open(&self) -> impl Future<Output = ()> + 'static {
        let spec = self.open_drawer_motion_spec();
        self.animate_to_with(DrawerValue::Open, spec as Arc<dyn AnimationSpec<f32>>)
    }

    /// Closes the drawer, animating by the theme's motion, done when it is closed or the animation is cancelled
    /// (Compose's suspending `close`).
    pub fn close(&self) -> impl Future<Output = ()> + 'static {
        let spec = self.close_drawer_motion_spec();
        self.animate_to_with(DrawerValue::Closed, spec as Arc<dyn AnimationSpec<f32>>)
    }

    /// Animates the drawer to `target_value` along `anim` (Compose's deprecated suspending `animateTo(targetValue,
    /// anim)`).
    #[deprecated(note = "This method has been replaced by the open and close methods. The animation spec is now an \
                         implementation detail of ModalDrawer.")]
    pub fn animate_to(
        &self,
        target_value: DrawerValue,
        anim: Arc<dyn AnimationSpec<f32>>,
    ) -> impl Future<Output = ()> + 'static {
        self.animate_to_with(target_value, anim)
    }

    /// Sets it to `target_value` without an animation, done when it is (Compose's suspending `snapTo`).
    pub fn snap_to(&self, target_value: DrawerValue) -> impl Future<Output = ()> + 'static {
        self.inner.anchored_draggable_state.snap_to(target_value)
    }

    /// The value it is going to (Compose's `targetValue`): while a swipe goes on, where the drawer would settle if
    /// it ended now; while an animation runs, the animation's target; else its current value.
    pub fn target_value(&self) -> DrawerValue {
        self.inner.anchored_draggable_state.target_value()
    }

    /// Where the sheet is, px, as a state (Compose's deprecated `offset`): NaN before it is laid out. The same
    /// state each time.
    #[deprecated(note = "Please access the offset through currentOffset, which returns the value directly instead \
                         of wrapping it in a state object.")]
    pub fn offset(&self) -> State<f32> {
        self.inner.offset.clone()
    }

    /// Where the sheet is, px (Compose's `currentOffset`): NaN before it is laid out.
    pub fn current_offset(&self) -> f32 {
        self.inner.anchored_draggable_state.offset()
    }

    /// Whether it may change to `value` (Compose's internal `confirmStateChange`).
    pub(crate) fn confirm_state_change(&self, value: DrawerValue) -> bool {
        (self.inner.confirm_state_change)(value)
    }

    /// The spec the modal drawer gives it (Compose's internal `anchoredDraggableMotionSpec`; read by nothing in
    /// the library, as in Compose).
    #[allow(dead_code)]
    pub(crate) fn anchored_draggable_motion_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.inner.anchored_draggable_motion_spec.borrow().clone()
    }

    pub(crate) fn set_anchored_draggable_motion_spec(&self, spec: Arc<dyn FiniteAnimationSpec<f32>>) {
        *self.inner.anchored_draggable_motion_spec.borrow_mut() = spec;
    }

    /// Its anchored draggable state (Compose's internal `anchoredDraggableState`).
    pub(crate) fn anchored_draggable_state(&self) -> AnchoredDraggableState<DrawerValue> {
        self.inner.anchored_draggable_state.clone()
    }

    /// The density its velocity threshold is in, set by the drawer it is given to (Compose's internal `density`).
    #[allow(dead_code)]
    pub(crate) fn density(&self) -> Option<Density> {
        self.inner.density.get()
    }

    pub(crate) fn set_density(&self, density: Density) {
        self.inner.density.set(Some(density));
    }

    /// The motion it opens by (Compose's internal `openDrawerMotionSpec`, a snap until a drawer sets the theme's).
    pub(crate) fn open_drawer_motion_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.inner.open_drawer_motion_spec.borrow().clone()
    }

    pub(crate) fn set_open_drawer_motion_spec(&self, spec: Arc<dyn FiniteAnimationSpec<f32>>) {
        *self.inner.open_drawer_motion_spec.borrow_mut() = spec;
    }

    /// The motion it closes by (Compose's internal `closeDrawerMotionSpec`).
    pub(crate) fn close_drawer_motion_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.inner.close_drawer_motion_spec.borrow().clone()
    }

    pub(crate) fn set_close_drawer_motion_spec(&self, spec: Arc<dyn FiniteAnimationSpec<f32>>) {
        *self.inner.close_drawer_motion_spec.borrow_mut() = spec;
    }

    /// Where the sheet is, px (Compose's internal `requireOffset`): panics (Kotlin's IllegalStateException) before
    /// it is laid out.
    #[track_caller]
    pub(crate) fn require_offset(&self) -> f32 {
        self.inner.anchored_draggable_state.require_offset()
    }

    /// Animates it to `target_value` along `animation_spec` from its last velocity (Compose's private
    /// `animateTo(targetValue, animationSpec, velocity = anchoredDraggableState.lastVelocity)`), in an anchored drag
    /// toward it: an overshooting spec goes past the anchor (the drag's offset isn't kept within the anchors).
    fn animate_to_with(
        &self,
        target_value: DrawerValue,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> impl Future<Output = ()> + 'static {
        let velocity = self.inner.anchored_draggable_state.last_velocity();
        let state = self.clone();
        named_args!(self.inner.anchored_draggable_state.anchored_drag_to_target(
            target_value,
            move |drag_scope, anchors, latest_target| {
                let (state, animation_spec) = (state.clone(), animation_spec.clone());
                async move {
                    let target_offset = anchors.position_of(&latest_target);
                    if !target_offset.is_nan() {
                        let current_offset = state.current_offset();
                        let prev = if current_offset.is_nan() { 0.0 } else { current_offset };
                        named_args!(animate(
                            prev,
                            target_offset,
                            initial_velocity = velocity,
                            animation_spec = animation_spec,
                            // Our onDrag coerces the value within the bounds, but an animation may overshoot, for
                            // example a spring animation or an overshooting interpolator. We respect the user's
                            // intention and allow the overshoot, but still use DraggableState's drag for its mutex.
                            block = move |value, velocity| drag_scope.drag_to(value, velocity),
                        ))
                        .await;
                    }
                }
            }
        ))
    }

    /// The default saver of a drawer's state (Compose's `DrawerState.Saver(confirmStateChange)`): its current value
    /// saved, a state at it restored.
    pub fn Saver(confirm_state_change: impl Fn(DrawerValue) -> bool + 'static) -> Saver<DrawerState> {
        DrawerState::saver_of(Rc::new(confirm_state_change))
    }

    /// [`Saver`](Self::Saver) of a shared closure.
    pub(crate) fn saver_of(confirm_state_change: Rc<dyn Fn(DrawerValue) -> bool>) -> Saver<DrawerState> {
        Saver::new(
            |state: &DrawerState| state.current_value().save(),
            move |saved| Some(DrawerState::new(DrawerValue::restore(saved)?, confirm_state_change.clone())),
        )
    }
}

/// The density a drawer state's velocity threshold is in (Compose's private `requireDensity`): panics (Kotlin's
/// IllegalArgumentException) when no drawer has set it.
#[track_caller]
fn require_density(density: &MutableState<Option<Density>>, this: &Weak<DrawerStateInner>) -> Density {
    match density.get() {
        Some(density) => density,
        None => {
            let this = this.upgrade().map(|inner| format!("{:?}", DrawerState { inner }));
            panic_kt(
                "IllegalArgumentException",
                &format!(
                    "The density on DrawerState ({}) was not set. Did you use DrawerState with the \
                     ModalNavigationDrawer or DismissibleNavigationDrawer composables?",
                    this.unwrap_or_default()
                ),
            )
        }
    }
}

/// The source of [`DrawerState::offset`]'s state (Compose's `object : State<Float>` reading the anchored
/// draggable's offset).
struct OffsetState(AnchoredDraggableState<DrawerValue>);

impl StateSource<f32> for OffsetState {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&f32)) {
        let value = if subscribe { self.0.offset() } else { without_read_observation(|| self.0.offset()) };
        f(&value)
    }

    fn identity(&self) -> *const () {
        self as *const OffsetState as *const ()
    }
}

/// A drawer's state, saved and restored by [`DrawerState::Saver`] (Compose's `rememberDrawerState(initialValue,
/// confirmStateChange = { true })`): at `initial_value`; `confirm_state_change` may veto a change.
#[kompozable]
pub fn remember_drawer_state(
    initial_value: DrawerValue,
    #[default(|_| true)] confirm_state_change: impl Fn(DrawerValue) -> bool + 'static,
) -> DrawerState {
    let confirm_state_change: Rc<dyn Fn(DrawerValue) -> bool> = Rc::new(confirm_state_change);
    let saver = DrawerState::saver_of(confirm_state_change.clone());
    kompoz_runtime::remember_saveable_with(&saver, move || DrawerState::new(initial_value, confirm_state_change))
}

// ---------------------------------------------------------------------------------------------------------------
// The drawers

/// A modal navigation drawer (Compose's `ModalNavigationDrawer`): `drawer_content` (a drawer sheet, typically
/// [`ModalDrawerSheet`]) over `content`, the content behind a scrim of `scrim_color` while it is out — the scrim
/// tapped closes it. `drawer_state` (None: a remembered closed one) says where it is; with `gestures_enabled` it is
/// dragged open and shut across the whole drawer (mirrored right to left, where it comes from the right). Open, it
/// takes the keyboard's focus, Escape closes it, and accessibility can dismiss it.
#[kompozable]
pub fn ModalNavigationDrawer(
    drawer_content: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] drawer_state: Option<DrawerState>,
    #[default(true)] gestures_enabled: bool,
    #[default(DrawerDefaults::scrim_color())] scrim_color: Color,
    content: impl Fn() + 'static,
) {
    let drawer_state = match drawer_state {
        Some(drawer_state) => drawer_state,
        None => remember_drawer_state(DrawerValue::Closed),
    };
    let scope = remember_coroutine_scope();
    let navigation_menu = get_string(Strings::NavigationMenu);
    let density = LocalDensity.current();
    let anchors_initialized = remember(|| mutable_state_of(false));
    let min_value = remember_keyed(density, || mutable_float_state_of(0.0));
    let max_value = 0.0f32;
    let focus_requester = remember(FocusRequester::new);

    // TODO Load the motionScheme tokens from the component tokens file
    let anchored_draggable_motion = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
    let open_motion = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
    let close_motion = MotionSchemeKeyTokens::FastEffects.value::<f32>();

    {
        let drawer_state = drawer_state.clone();
        SideEffect(move || {
            drawer_state.set_density(density);
            drawer_state.set_open_drawer_motion_spec(open_motion);
            drawer_state.set_close_drawer_motion_spec(close_motion);
            drawer_state.set_anchored_draggable_motion_spec(anchored_draggable_motion);
        });
    }

    {
        let (is_open, focus_requester) = (drawer_state.is_open(), focus_requester.clone());
        LaunchedEffect(is_open, async move {
            if is_open {
                // Keyboard focus should go to first element of the drawer when it opens
                named_args!(focus_requester.request_focus());
            }
        });
    }

    let is_rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
    let box_modifier = named_args!(modifier.fill_max_size().anchored_draggable_reversed(
        drawer_state.anchored_draggable_state(),
        is_rtl,
        Orientation::Horizontal,
        enabled = gestures_enabled,
    ));
    Box(
        modifier = box_modifier,
        content = move || {
            Box(content = || content());
            let on_dismiss_request: Rc<dyn Fn()> = {
                let (drawer_state, scope) = (drawer_state.clone(), scope.clone());
                Rc::new(move || {
                    if gestures_enabled && drawer_state.confirm_state_change(DrawerValue::Closed) {
                        launch_close(&scope, &drawer_state);
                    }
                })
            };
            let alpha: Rc<dyn Fn() -> f32> = {
                let (drawer_state, min_value) = (drawer_state.clone(), min_value.clone());
                Rc::new(move || calculate_fraction(min_value.get(), max_value, drawer_state.require_offset()))
            };
            Scrim(
                Some(get_string(Strings::CloseDrawer)),
                on_click = if drawer_state.is_open() { Some(on_dismiss_request) } else { None },
                alpha = alpha,
                color = scrim_color,
            );
            let (offset_state, semantics_state, semantics_scope, key_state, key_scope) =
                (drawer_state.clone(), drawer_state.clone(), scope.clone(), drawer_state.clone(), scope.clone());
            let navigation_menu = navigation_menu.clone();
            let layout_modifier = Modifier
                .offset_px(move |density: &Density| {
                    let offset = offset_state.current_offset();
                    let offset_x = if !offset.is_nan() {
                        offset.round_to_int()
                    } else if offset_state.is_open() {
                        // If offset is NaN, set offset based on open/closed state
                        0
                    } else {
                        -density.round_to_px(DrawerDefaults::MaximumDrawerWidth)
                    };
                    IntOffset::new(offset_x, 0)
                })
                .semantics_with(false, move |c| {
                    c.set_pane_title(navigation_menu.clone());
                    if semantics_state.is_open() {
                        let (drawer_state, scope) = (semantics_state.clone(), semantics_scope.clone());
                        named_args!(c.dismiss(move || {
                            if drawer_state.confirm_state_change(DrawerValue::Closed) {
                                launch_close(&scope, &drawer_state);
                            }
                            true
                        }));
                    }
                })
                .on_key_event(move |event: &KeyEvent| {
                    // Drawer should close via escape key.
                    if key_state.is_open() && event.r#type == KeyEventType::KeyUp && event.key == Key::Escape {
                        launch_close(&key_scope, &key_state);
                        return true;
                    }
                    false
                })
                .focus_requester(&focus_requester);
            let (anchors_initialized, min_value, drawer_state) =
                (anchors_initialized.clone(), min_value.clone(), drawer_state.clone());
            Layout(
                content = move || drawer_content(),
                modifier = layout_modifier,
                measure_policy = measure_policy(move |scope, measurables, constraints| {
                    let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
                    let placeables: Vec<_> = measurables.iter().map(|it| it.measure(loose_constraints)).collect();
                    let width = placeables.iter().map(|it| it.width()).max().unwrap_or(0);
                    let height = placeables.iter().map(|it| it.height()).max().unwrap_or(0);

                    let (anchors_initialized, min_value, drawer_state) =
                        (anchors_initialized.clone(), min_value.clone(), drawer_state.clone());
                    scope.layout(width, height, move |p| {
                        let current_closed_anchor =
                            drawer_state.anchored_draggable_state().anchors().position_of(&DrawerValue::Closed);
                        let calculated_closed_anchor = -(width as f32);

                        if !anchors_initialized.get() || current_closed_anchor != calculated_closed_anchor {
                            if !anchors_initialized.get() {
                                anchors_initialized.set(true);
                            }
                            min_value.set(calculated_closed_anchor);
                            let min_value = min_value.get();
                            named_args!(drawer_state.anchored_draggable_state().update_anchors(DraggableAnchors(
                                |a: &mut DraggableAnchorsConfig<DrawerValue>| {
                                    a.at(DrawerValue::Closed, min_value);
                                    a.at(DrawerValue::Open, max_value);
                                }
                            )));
                        }
                        let is_drawer_visible =
                            calculate_fraction(min_value.get(), max_value, drawer_state.require_offset()) != 0.0;
                        if is_drawer_visible {
                            // Only place the drawer when it's visible so that keyboard focus doesn't navigate to
                            // an offscreen element.
                            for it in &placeables {
                                named_args!(p.place_relative(it, 0, 0));
                            }
                        }
                    })
                }),
            );
        },
    );
}

/// Kotlin's `scope.launch { drawerState.close() }`.
fn launch_close(scope: &CoroutineScope, drawer_state: &DrawerState) {
    let drawer_state = drawer_state.clone();
    named_args!(scope.launch(async move { drawer_state.close().await }));
}

/// A dismissible navigation drawer (Compose's `DismissibleNavigationDrawer`): `drawer_content` (a drawer sheet,
/// typically [`DismissibleDrawerSheet`]) beside `content`, pushing it aside by the sheet's width as it comes out.
/// `drawer_state` (None: a remembered closed one) says where it is; with `gestures_enabled` it is dragged open and
/// shut across the whole drawer (mirrored right to left). Open, it takes the keyboard's focus, Escape closes it,
/// and accessibility can dismiss it.
#[kompozable]
pub fn DismissibleNavigationDrawer(
    drawer_content: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] drawer_state: Option<DrawerState>,
    #[default(true)] gestures_enabled: bool,
    content: impl Fn() + 'static,
) {
    let drawer_state = match drawer_state {
        Some(drawer_state) => drawer_state,
        None => remember_drawer_state(DrawerValue::Closed),
    };
    let anchors_initialized = remember(|| mutable_state_of(false));
    let density = LocalDensity.current();
    let focus_requester = remember(FocusRequester::new);

    // TODO Load the motionScheme tokens from the component tokens file
    let open_motion = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
    let close_motion = MotionSchemeKeyTokens::FastEffects.value::<f32>();

    {
        let drawer_state = drawer_state.clone();
        SideEffect(move || {
            drawer_state.set_density(density);
            drawer_state.set_open_drawer_motion_spec(open_motion);
            drawer_state.set_close_drawer_motion_spec(close_motion);
        });
    }

    {
        let (is_open, focus_requester) = (drawer_state.is_open(), focus_requester.clone());
        LaunchedEffect(is_open, async move {
            if is_open {
                // Keyboard focus should go to first element of the drawer when it opens
                named_args!(focus_requester.request_focus());
            }
        });
    }

    let scope = remember_coroutine_scope();
    let navigation_menu = get_string(Strings::NavigationMenu);

    let is_rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
    let box_modifier = named_args!(modifier.anchored_draggable_reversed(
        drawer_state.anchored_draggable_state(),
        is_rtl,
        Orientation::Horizontal,
        enabled = gestures_enabled,
    ));
    Box(
        modifier = box_modifier,
        content = move || {
            let (semantics_state, semantics_scope, key_state, key_scope) =
                (drawer_state.clone(), scope.clone(), drawer_state.clone(), scope.clone());
            let navigation_menu = navigation_menu.clone();
            let sheet_modifier = Modifier
                .semantics_with(false, move |c| {
                    c.set_pane_title(navigation_menu.clone());
                    if semantics_state.is_open() {
                        let (drawer_state, scope) = (semantics_state.clone(), semantics_scope.clone());
                        named_args!(c.dismiss(move || {
                            if drawer_state.confirm_state_change(DrawerValue::Closed) {
                                launch_close(&scope, &drawer_state);
                            }
                            true
                        }));
                    }
                })
                .on_key_event(move |event: &KeyEvent| {
                    // Drawer should close via escape key.
                    if key_state.is_open() && event.r#type == KeyEventType::KeyUp && event.key == Key::Escape {
                        launch_close(&key_scope, &key_state);
                        return true;
                    }
                    false
                })
                .focus_requester(&focus_requester);
            let (anchors_initialized, drawer_state) = (anchors_initialized.clone(), drawer_state.clone());
            Layout(
                content = move || {
                    Box(modifier = sheet_modifier.clone(), content = || drawer_content());
                    Box(content = || content());
                },
                measure_policy = measure_policy(move |scope, measurables, constraints| {
                    let sheet_placeable = measurables[0].measure(constraints);
                    let content_placeable = measurables[1].measure(constraints);
                    let (anchors_initialized, drawer_state) = (anchors_initialized.clone(), drawer_state.clone());
                    scope.layout(content_placeable.width(), content_placeable.height(), move |p| {
                        let current_closed_anchor =
                            drawer_state.anchored_draggable_state().anchors().position_of(&DrawerValue::Closed);
                        let calculated_closed_anchor = -(sheet_placeable.width() as f32);

                        if !anchors_initialized.get() || current_closed_anchor != calculated_closed_anchor {
                            if !anchors_initialized.get() {
                                anchors_initialized.set(true);
                            }
                            named_args!(drawer_state.anchored_draggable_state().update_anchors(DraggableAnchors(
                                |a: &mut DraggableAnchorsConfig<DrawerValue>| {
                                    a.at(DrawerValue::Closed, calculated_closed_anchor);
                                    a.at(DrawerValue::Open, 0.0);
                                }
                            )));
                        }

                        let content_x = sheet_placeable.width() + drawer_state.require_offset().round_to_int();
                        named_args!(p.place_relative(&content_placeable, content_x, 0));
                        // The drawer is visible when the content has been offset.
                        if content_x != 0 {
                            // Only place the drawer when it's visible so that keyboard focus doesn't navigate to an
                            // offscreen element.
                            named_args!(p.place_relative(
                                &sheet_placeable,
                                drawer_state.require_offset().round_to_int(),
                                0
                            ));
                        }
                    })
                }),
            );
        },
    );
}

/// A permanent navigation drawer (Compose's `PermanentNavigationDrawer`): `drawer_content` (typically
/// [`PermanentDrawerSheet`]) always beside `content`, in a Row filling its room. On a phone's screen, rather
/// [`ModalNavigationDrawer`].
#[kompozable]
pub fn PermanentNavigationDrawer(
    drawer_content: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    Row(
        modifier = modifier.fill_max_size(),
        content = || {
            drawer_content();
            Box(content = || content());
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The sheets

/// The content of a modal navigation drawer (Compose's `ModalDrawerSheet(modifier, …)`): `content` in a Column of
/// at least 240 dp and at most [`DrawerDefaults::MaximumDrawerWidth`], filling the height, padded by `window_insets`,
/// on a [`Surface`] of `drawer_shape`, `drawer_container_color`, `drawer_content_color` (None: the container's
/// content color) and `drawer_tonal_elevation`. It doesn't handle back: [`ModalDrawerSheetWithDrawerState`] does.
#[kompozable]
pub fn ModalDrawerSheet(
    #[default(Modifier)] modifier: Modifier,
    #[default(DrawerDefaults::shape())] drawer_shape: Arc<dyn Shape>,
    #[default(DrawerDefaults::modal_container_color())] drawer_container_color: Color,
    #[default(None)] drawer_content_color: Option<Color>,
    #[default(DrawerDefaults::ModalDrawerElevation)] drawer_tonal_elevation: Dp,
    #[default(DrawerDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    DrawerSheet(
        None,
        window_insets,
        modifier = modifier,
        drawer_shape = drawer_shape,
        drawer_container_color = drawer_container_color,
        drawer_content_color = drawer_content_color,
        drawer_tonal_elevation = drawer_tonal_elevation,
        content = content,
    );
}

/// The same handling back while `drawer_state` is open (Compose's `ModalDrawerSheet(drawerState, …)`): a
/// predictive back gesture stretches the sheet out from its edge (shrinks it from the other) and lowers it, then
/// closes the drawer.
///
/// Kotlin: `ModalDrawerSheet(drawerState: DrawerState, modifier: Modifier = Modifier, drawerShape: Shape = DrawerDefaults.shape, drawerContainerColor: Color = DrawerDefaults.modalContainerColor, drawerContentColor: Color = contentColorFor(drawerContainerColor), drawerTonalElevation: Dp = DrawerDefaults.ModalDrawerElevation, windowInsets: WindowInsets = DrawerDefaults.windowInsets, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
pub fn ModalDrawerSheetWithDrawerState(
    drawer_state: DrawerState,
    #[default(Modifier)] modifier: Modifier,
    #[default(DrawerDefaults::shape())] drawer_shape: Arc<dyn Shape>,
    #[default(DrawerDefaults::modal_container_color())] drawer_container_color: Color,
    #[default(None)] drawer_content_color: Option<Color>,
    #[default(DrawerDefaults::ModalDrawerElevation)] drawer_tonal_elevation: Dp,
    #[default(DrawerDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    let content: Rc<dyn Fn()> = Rc::new(content);
    let offset_state = drawer_state.clone();
    DrawerPredictiveBackHandler(drawer_state, move |drawer_predictive_back_state| {
        let (content, offset_state) = (content.clone(), offset_state.clone());
        DrawerSheet(
            Some(drawer_predictive_back_state),
            window_insets.clone(),
            modifier = modifier.clone(),
            drawer_shape = drawer_shape.clone(),
            drawer_container_color = drawer_container_color,
            drawer_content_color = drawer_content_color,
            drawer_tonal_elevation = drawer_tonal_elevation,
            drawer_offset = FloatProducer::new(move || offset_state.anchored_draggable_state().offset()),
            content = move || content(),
        );
    });
}

/// The content of a dismissible navigation drawer (Compose's `DismissibleDrawerSheet(modifier, …)`): as
/// [`ModalDrawerSheet`], by default rectangular in the standard container color, flat. It doesn't handle back:
/// [`DismissibleDrawerSheetWithDrawerState`] does.
#[kompozable]
pub fn DismissibleDrawerSheet(
    #[default(Modifier)] modifier: Modifier,
    #[default(Arc::new(RectangleShape))] drawer_shape: Arc<dyn Shape>,
    #[default(DrawerDefaults::standard_container_color())] drawer_container_color: Color,
    #[default(None)] drawer_content_color: Option<Color>,
    #[default(DrawerDefaults::DismissibleDrawerElevation)] drawer_tonal_elevation: Dp,
    #[default(DrawerDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    DrawerSheet(
        None,
        window_insets,
        modifier = modifier,
        drawer_shape = drawer_shape,
        drawer_container_color = drawer_container_color,
        drawer_content_color = drawer_content_color,
        drawer_tonal_elevation = drawer_tonal_elevation,
        content = content,
    );
}

/// The same handling back while `drawer_state` is open (Compose's `DismissibleDrawerSheet(drawerState, …)`), as
/// [`ModalDrawerSheetWithDrawerState`].
///
/// Kotlin: `DismissibleDrawerSheet(drawerState: DrawerState, modifier: Modifier = Modifier, drawerShape: Shape = RectangleShape, drawerContainerColor: Color = DrawerDefaults.standardContainerColor, drawerContentColor: Color = contentColorFor(drawerContainerColor), drawerTonalElevation: Dp = DrawerDefaults.DismissibleDrawerElevation, windowInsets: WindowInsets = DrawerDefaults.windowInsets, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
pub fn DismissibleDrawerSheetWithDrawerState(
    drawer_state: DrawerState,
    #[default(Modifier)] modifier: Modifier,
    #[default(Arc::new(RectangleShape))] drawer_shape: Arc<dyn Shape>,
    #[default(DrawerDefaults::standard_container_color())] drawer_container_color: Color,
    #[default(None)] drawer_content_color: Option<Color>,
    #[default(DrawerDefaults::DismissibleDrawerElevation)] drawer_tonal_elevation: Dp,
    #[default(DrawerDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    let content: Rc<dyn Fn()> = Rc::new(content);
    let offset_state = drawer_state.clone();
    DrawerPredictiveBackHandler(drawer_state, move |drawer_predictive_back_state| {
        let (content, offset_state) = (content.clone(), offset_state.clone());
        DrawerSheet(
            Some(drawer_predictive_back_state),
            window_insets.clone(),
            modifier = modifier.clone(),
            drawer_shape = drawer_shape.clone(),
            drawer_container_color = drawer_container_color,
            drawer_content_color = drawer_content_color,
            drawer_tonal_elevation = drawer_tonal_elevation,
            drawer_offset = FloatProducer::new(move || offset_state.anchored_draggable_state().offset()),
            content = move || content(),
        );
    });
}

/// The content of a permanent navigation drawer (Compose's `PermanentDrawerSheet`): as [`DismissibleDrawerSheet`],
/// told to accessibility as the navigation menu's pane.
#[kompozable]
pub fn PermanentDrawerSheet(
    #[default(Modifier)] modifier: Modifier,
    #[default(Arc::new(RectangleShape))] drawer_shape: Arc<dyn Shape>,
    #[default(DrawerDefaults::standard_container_color())] drawer_container_color: Color,
    #[default(None)] drawer_content_color: Option<Color>,
    #[default(DrawerDefaults::PermanentDrawerElevation)] drawer_tonal_elevation: Dp,
    #[default(DrawerDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    let navigation_menu = get_string(Strings::NavigationMenu);
    DrawerSheet(
        None,
        window_insets,
        modifier = modifier.semantics_with(false, move |c| c.set_pane_title(navigation_menu.clone())),
        drawer_shape = drawer_shape,
        drawer_container_color = drawer_container_color,
        drawer_content_color = drawer_content_color,
        drawer_tonal_elevation = drawer_tonal_elevation,
        content = content,
    );
}

/// A drawer's sheet (Compose's internal `DrawerSheet`): a [`Surface`] at least 240 dp and at most the drawer's
/// width wide, filling the height, scaled out across by how far `drawer_offset` (px) is past the open anchor (a
/// bouncy opening shows no gap at the edge; its content scaled back to keep its aspect), and by a predictive back
/// gesture through `drawer_predictive_back_state`; `content` in a Column padded by `window_insets`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DrawerSheet(
    drawer_predictive_back_state: Option<DrawerPredictiveBackState>,
    window_insets: WindowInsets,
    #[default(Modifier)] modifier: Modifier,
    #[default(Arc::new(RectangleShape))] drawer_shape: Arc<dyn Shape>,
    #[default(DrawerDefaults::standard_container_color())] drawer_container_color: Color,
    #[default(None)] drawer_content_color: Option<Color>,
    #[default(DrawerDefaults::PermanentDrawerElevation)] drawer_tonal_elevation: Dp,
    #[default(FloatProducer::new(|| 0.0))] drawer_offset: FloatProducer,
    content: impl Fn() + 'static,
) {
    let drawer_content_color = drawer_content_color.unwrap_or_else(|| content_color_for(drawer_container_color));
    let density = LocalDensity.current();
    let max_width = NavigationDrawerTokens::ContainerWidth;
    let max_width_px = density.to_px(max_width);
    let is_rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
    let predictive_back_drawer_container_modifier = match &drawer_predictive_back_state {
        Some(state) => Modifier.predictive_back_drawer_container(state.clone(), is_rtl),
        None => Modifier,
    };
    Surface(
        modifier = named_args!(modifier.size_in(min_width = MinimumDrawerWidth, max_width = max_width))
            // Scale up the Surface horizontally in case the drawer offset it greater than zero. This is done to avoid
            // showing a gap when the drawer opens and bounces when it's applied with a bouncy motion. Note that the
            // content inside the Surface is scaled back down to maintain its aspect ratio (see below).
            .horizontal_scale_up(drawer_offset.clone(), max_width_px, is_rtl)
            .then(predictive_back_drawer_container_modifier)
            .fill_max_height(),
        shape = drawer_shape,
        color = drawer_container_color,
        content_color = Some(drawer_content_color),
        tonal_elevation = drawer_tonal_elevation,
        content = move || {
            let predictive_back_drawer_child_modifier = match &drawer_predictive_back_state {
                Some(state) => Modifier.predictive_back_drawer_child(state.clone(), is_rtl),
                None => Modifier,
            };
            Column(
                modifier = named_args!(Modifier.size_in(min_width = MinimumDrawerWidth, max_width = max_width))
                    // Scale the content down in case the drawer offset is greater than one. The wrapping Surface is
                    // scaled up, so this is done to maintain the content's aspect ratio.
                    .horizontal_scale_down(drawer_offset, max_width_px, is_rtl)
                    .then(predictive_back_drawer_child_modifier)
                    .window_insets_padding(window_insets),
                content = || content(),
            );
        },
    );
}

/// The sheets' scaling modifiers (Compose's private `Modifier.horizontalScaleUp`, …).
trait DrawerSheetModifiers {
    /// Scales the layer up across, about the side away from the screen's edge, by how far `drawer_offset` is past
    /// the open anchor (0) out of `drawer_width` (Compose's private `Modifier.horizontalScaleUp`): a bouncy opening
    /// shows no gap at the edge. Its content takes [`horizontal_scale_down`](Self::horizontal_scale_down).
    fn horizontal_scale_up(self, drawer_offset: FloatProducer, drawer_width: f32, is_rtl: bool) -> Modifier;

    /// Scales the layer back down across as its container is scaled up, keeping its aspect (Compose's private
    /// `Modifier.horizontalScaleDown`).
    fn horizontal_scale_down(self, drawer_offset: FloatProducer, drawer_width: f32, is_rtl: bool) -> Modifier;

    /// Scales the sheet by a predictive back gesture, about the screen's edge half way down (Compose's private
    /// `Modifier.predictiveBackDrawerContainer`).
    fn predictive_back_drawer_container(self, state: DrawerPredictiveBackState, is_rtl: bool) -> Modifier;

    /// Scales the sheet's content back across so it keeps its aspect and its place at the top of the sheet's far
    /// side (Compose's private `Modifier.predictiveBackDrawerChild`).
    fn predictive_back_drawer_child(self, state: DrawerPredictiveBackState, is_rtl: bool) -> Modifier;
}

impl DrawerSheetModifiers for Modifier {
    fn horizontal_scale_up(self, drawer_offset: FloatProducer, drawer_width: f32, is_rtl: bool) -> Modifier {
        self.graphics_layer_block(move |scope| {
            let offset = drawer_offset.invoke();
            scope.scale_x = if offset > 0.0 { 1.0 + offset / drawer_width } else { 1.0 };
            scope.transform_origin = TransformOrigin::new(if is_rtl { 0.0 } else { 1.0 }, 0.5);
        })
    }

    fn horizontal_scale_down(self, drawer_offset: FloatProducer, drawer_width: f32, is_rtl: bool) -> Modifier {
        self.graphics_layer_block(move |scope| {
            let offset = drawer_offset.invoke();
            scope.scale_x = if offset > 0.0 { 1.0 / (1.0 + offset / drawer_width) } else { 1.0 };
            scope.transform_origin = TransformOrigin::new(if is_rtl { 0.0 } else { 1.0 }, 0.0);
        })
    }

    fn predictive_back_drawer_container(self, state: DrawerPredictiveBackState, is_rtl: bool) -> Modifier {
        self.graphics_layer_block(move |scope| {
            scope.scale_x = calculate_predictive_back_scale_x(scope, &state);
            scope.scale_y = calculate_predictive_back_scale_y(scope, &state);
            scope.transform_origin = TransformOrigin::new(if is_rtl { 1.0 } else { 0.0 }, 0.5);
        })
    }

    fn predictive_back_drawer_child(self, state: DrawerPredictiveBackState, is_rtl: bool) -> Modifier {
        self.graphics_layer_block(move |scope| {
            // Preserve the original aspect ratio and container alignment of the child content, and add content
            // margins.
            let container_scale_x = calculate_predictive_back_scale_x(scope, &state);
            let container_scale_y = calculate_predictive_back_scale_y(scope, &state);
            scope.scale_x = if container_scale_x != 0.0 { container_scale_y / container_scale_x } else { 1.0 };
            scope.transform_origin = TransformOrigin::new(if is_rtl { 0.0 } else { 1.0 }, 0.0);
        })
    }
}

/// The sheet's scale across in a predictive back gesture (Compose's private
/// `GraphicsLayerScope.calculatePredictiveBackScaleX`): wider by its distance from the drawer's own edge, narrower
/// from the other.
fn calculate_predictive_back_scale_x(scope: &GraphicsLayerScope, state: &DrawerPredictiveBackState) -> f32 {
    let width = scope.size().width;
    if width.is_nan() || width == 0.0 {
        1.0
    } else {
        let scale_x_direction = if state.swipe_edge_matches_drawer() { 1.0 } else { -1.0 };
        1.0 + state.scale_x_distance() * scale_x_direction / width
    }
}

/// The sheet's scale down in a predictive back gesture (Compose's private
/// `GraphicsLayerScope.calculatePredictiveBackScaleY`).
fn calculate_predictive_back_scale_y(scope: &GraphicsLayerScope, state: &DrawerPredictiveBackState) -> f32 {
    let height = scope.size().height;
    if height.is_nan() || height == 0.0 { 1.0 } else { 1.0 - state.scale_y_distance() / height }
}

/// Handles a predictive back gesture while `drawer_state` is open, `content` given the animation's values
/// (Compose's internal `DrawerPredictiveBackHandler`): the gesture's progress, through Material's easing, sets the
/// sheet's stretch (from the drawer's own edge, else its shrinking) and lowering; done, a stretch goes back across
/// and the drawer closes; called off, it is as it was.
#[kompozable]
pub(crate) fn DrawerPredictiveBackHandler(
    drawer_state: DrawerState,
    content: impl Fn(DrawerPredictiveBackState) + 'static,
) {
    let drawer_predictive_back_state = remember(DrawerPredictiveBackState::new);
    let scope = remember_coroutine_scope();
    let is_rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
    let density = LocalDensity.current();
    let max_scale_x_distance_grow = density.to_px(PredictiveBackDrawerMaxScaleXDistanceGrow);
    let max_scale_x_distance_shrink = density.to_px(PredictiveBackDrawerMaxScaleXDistanceShrink);
    let max_scale_y_distance = density.to_px(PredictiveBackDrawerMaxScaleYDistance);

    {
        let (back_state, scope, closing) = (drawer_predictive_back_state.clone(), scope.clone(), drawer_state.clone());
        PredictiveBackHandler(
            enabled = drawer_state.is_open(),
            on_back = move |progress: Flow<BackEventCompat>| {
                let (back_state, scope, drawer_state) = (back_state.clone(), scope.clone(), closing.clone());
                async move {
                    // (Kotlin's `catch (e: CancellationException) { clear() } finally { … }`: a called-off
                    // gesture's coroutine is dropped here, running nothing more, so the guard does what the catch
                    // and the finally do then — the finally's `drawerState.close()` does nothing in a cancelled
                    // coroutine in Compose, its anchored drag cancelled at once)
                    let mut cancelled = ClearIfCancelled(Some((back_state.clone(), scope.clone())));
                    let steps = back_state.clone();
                    progress
                        .collect_async(move |back_event| {
                            let steps = steps.clone();
                            async move {
                                steps.update(
                                    PredictiveBack::transform(back_event.progress),
                                    back_event.swipe_edge == SwipeEdge::Left,
                                    is_rtl,
                                    max_scale_x_distance_grow,
                                    max_scale_x_distance_shrink,
                                    max_scale_y_distance,
                                );
                            }
                        })
                        .await;
                    cancelled.0 = None;
                    un_stretch(&back_state, &scope);
                    drawer_state.close().await;
                }
            },
        );
    }

    {
        let (is_closed, back_state) = (drawer_state.is_closed(), drawer_predictive_back_state.clone());
        LaunchedEffect(is_closed, async move {
            if is_closed {
                back_state.clear();
            }
        });
    }

    content(drawer_predictive_back_state);
}

/// The predictive back handler's `finally`: if the swipe edge matches the drawer's and the drawer was stretched
/// across, it is un-stretched smoothly so that it hides completely as it closes.
fn un_stretch(state: &DrawerPredictiveBackState, scope: &CoroutineScope) {
    if state.swipe_edge_matches_drawer() {
        let state = state.clone();
        named_args!(scope.launch(async move {
            let distance = state.clone();
            named_args!(animate(
                state.scale_x_distance(),
                0.0,
                block = move |value, _| { distance.set_scale_x_distance(value) }
            ))
            .await;
            state.clear();
        }));
    }
}

/// Clears a predictive back's values when dropped armed, the gesture called off while it collects, then un-stretches
/// as the handler's `finally` does.
struct ClearIfCancelled(Option<(DrawerPredictiveBackState, CoroutineScope)>);

impl Drop for ClearIfCancelled {
    fn drop(&mut self) {
        if let Some((state, scope)) = self.0.take() {
            state.clear();
            un_stretch(&state, &scope);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DrawerDefaults

/// The defaults of the navigation drawers (Compose's object `DrawerDefaults`).
pub struct DrawerDefaults;

impl DrawerDefaults {
    /// The tonal elevation of a [`ModalNavigationDrawer`]'s sheet (Compose's `ModalDrawerElevation`).
    pub const ModalDrawerElevation: Dp = ElevationTokens::Level0;

    /// The tonal elevation of a [`PermanentNavigationDrawer`]'s sheet (Compose's `PermanentDrawerElevation`).
    pub const PermanentDrawerElevation: Dp = NavigationDrawerTokens::StandardContainerElevation;

    /// The tonal elevation of a [`DismissibleNavigationDrawer`]'s sheet (Compose's `DismissibleDrawerElevation`).
    pub const DismissibleDrawerElevation: Dp = NavigationDrawerTokens::StandardContainerElevation;

    /// The default and largest width of a navigation drawer (Compose's `MaximumDrawerWidth`): 360 dp.
    pub const MaximumDrawerWidth: Dp = NavigationDrawerTokens::ContainerWidth;

    /// The shape of a navigation drawer (Compose's `shape`): the theme's large shape at the end.
    pub fn shape() -> Arc<dyn Shape> {
        NavigationDrawerTokens::ContainerShape.value()
    }

    /// The color of the scrim over the content while a modal drawer is out (Compose's `scrimColor`): the scheme's
    /// scrim at 32%.
    pub fn scrim_color() -> Color {
        named_args!(ScrimTokens::ContainerColor.value().copy(alpha = ScrimTokens::ContainerOpacity))
    }

    /// The container color of a navigation drawer (Compose's deprecated `containerColor`): the standard one.
    #[deprecated(note = "Please use standardContainerColor or modalContainerColor instead.")]
    pub fn container_color() -> Color {
        NavigationDrawerTokens::StandardContainerColor.value()
    }

    /// The container color of a [`DismissibleNavigationDrawer`]'s and a [`PermanentNavigationDrawer`]'s sheet
    /// (Compose's `standardContainerColor`): the scheme's surface.
    pub fn standard_container_color() -> Color {
        NavigationDrawerTokens::StandardContainerColor.value()
    }

    /// The container color of a [`ModalNavigationDrawer`]'s sheet (Compose's `modalContainerColor`): the scheme's
    /// low surface container.
    pub fn modal_container_color() -> Color {
        NavigationDrawerTokens::ModalContainerColor.value()
    }

    /// The insets a drawer's sheet pads its content by (Compose's `windowInsets`): the system bars' at the top, the
    /// bottom and the start.
    #[track_caller]
    pub fn window_insets() -> WindowInsets {
        kompozable_block!({
            system_bars_for_visual_components().only(WindowInsetsSides::Vertical + WindowInsetsSides::Start)
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// NavigationDrawerItem

/// A destination in a navigation drawer (Compose's `NavigationDrawerItem`): a selectable pill at least 56 dp high
/// filling the width, of `shape` (by default a full one), its container in the color `colors` give for `selected`;
/// in a Row 16 dp in from the start and 24 dp from the end: `icon` (if any) then 12 dp, `label` filling the rest,
/// then 12 dp and `badge` (if any), each in its color from `colors`. A tab to accessibility; clicked, `on_click`;
/// its interactions emitted to `interaction_source` (None: one of its own).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigationDrawerItem(
    label: impl Fn() + 'static,
    selected: bool,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(None)] badge: Option<Rc<dyn Fn()>>,
    #[default(NavigationDrawerTokens::ActiveIndicatorShape.value())] shape: Arc<dyn Shape>,
    #[default(NavigationDrawerItemDefaults.colors())] colors: Rc<dyn NavigationDrawerItemColors>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let color = colors.container_color(selected).value();
    SelectableSurface(
        selected,
        on_click,
        modifier = named_args!(
            modifier
                .semantics(|c| c.set_role(Role::Tab))
                .height_in(min = NavigationDrawerTokens::ActiveIndicatorHeight)
        )
        .fill_max_width(),
        shape = shape,
        color = color,
        interaction_source = interaction_source,
        content = || {
            Row(
                modifier = named_args!(Modifier.padding_sides(start = Dp(16.0), end = Dp(24.0))),
                vertical_alignment = Alignment::CenterVertically,
                content = || {
                    if let Some(icon) = icon {
                        let icon_color = colors.icon_color(selected).value();
                        KompozitionLocalProvider(&[LocalContentColor.provides(icon_color)], || icon());
                        Spacer(Modifier.width(Dp(12.0)));
                    }
                    let label_color = colors.text_color(selected).value();
                    Box(
                        modifier = named_args!(Modifier.weight(1.0)),
                        content = || KompozitionLocalProvider(&[LocalContentColor.provides(label_color)], || label()),
                    );
                    if let Some(badge) = badge {
                        Spacer(Modifier.width(Dp(12.0)));
                        let badge_color = colors.badge_color(selected).value();
                        KompozitionLocalProvider(&[LocalContentColor.provides(badge_color)], || badge());
                    }
                },
            );
        },
    );
}

/// The colors of a drawer item's parts (Compose's interface `NavigationDrawerItemColors`), each by whether the item
/// is selected, as a state. [`NavigationDrawerItemDefaults::colors`] makes Material's; implement it for colors of
/// one's own (the methods are composable: call them from a composable only).
pub trait NavigationDrawerItemColors: Any {
    /// The icon's color (Compose's composable `iconColor(selected)`).
    #[track_caller]
    fn icon_color(&self, selected: bool) -> State<Color>;

    /// The label's color (Compose's composable `textColor(selected)`).
    #[track_caller]
    fn text_color(&self, selected: bool) -> State<Color>;

    /// The badge's color (Compose's composable `badgeColor(selected)`).
    #[track_caller]
    fn badge_color(&self, selected: bool) -> State<Color>;

    /// The container's color (Compose's composable `containerColor(selected)`).
    #[track_caller]
    fn container_color(&self, selected: bool) -> State<Color>;

    /// Whether `other` is the same colors (Kotlin's `equals`): by default whether it is this one.
    fn equals(&self, other: &dyn NavigationDrawerItemColors) -> bool {
        std::ptr::eq(self as *const Self as *const (), other as *const dyn NavigationDrawerItemColors as *const ())
    }
}

/// By [`NavigationDrawerItemColors::equals`].
impl PartialEq for dyn NavigationDrawerItemColors {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

/// The defaults of [`NavigationDrawerItem`] (Compose's object `NavigationDrawerItemDefaults`).
pub struct NavigationDrawerItemDefaults;

impl NavigationDrawerItemDefaults {
    /// The padding around a [`NavigationDrawerItem`] in a drawer (Compose's `ItemPadding`): 12 dp at the start and
    /// the end.
    pub fn ItemPadding() -> PaddingValues {
        PaddingValues::hv(Dp(12.0), Dp(0.0))
    }
}

#[named]
impl NavigationDrawerItemDefaults {
    /// Material's colors of a drawer item (Compose's `colors`): the containers `selected_container_color` (the
    /// scheme's secondary container) and `unselected_container_color` (transparent), the icons' (on secondary
    /// container, on surface variant), the labels' (the same), the badges' (None: the labels').
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(NavigationDrawerTokens::ActiveIndicatorColor.value())] selected_container_color: Color,
        #[default(Color::Transparent)] unselected_container_color: Color,
        #[default(NavigationDrawerTokens::ActiveIconColor.value())] selected_icon_color: Color,
        #[default(NavigationDrawerTokens::InactiveIconColor.value())] unselected_icon_color: Color,
        #[default(NavigationDrawerTokens::ActiveLabelTextColor.value())] selected_text_color: Color,
        #[default(NavigationDrawerTokens::InactiveLabelTextColor.value())] unselected_text_color: Color,
        #[default(None)] selected_badge_color: Option<Color>,
        #[default(None)] unselected_badge_color: Option<Color>,
    ) -> Rc<dyn NavigationDrawerItemColors> {
        Rc::new(DefaultDrawerItemsColor {
            selected_icon_color,
            unselected_icon_color,
            selected_text_color,
            unselected_text_color,
            selected_container_color,
            unselected_container_color,
            selected_badge_color: selected_badge_color.unwrap_or(selected_text_color),
            unselected_badge_color: unselected_badge_color.unwrap_or(unselected_text_color),
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DrawerPredictiveBackState

/// A predictive back gesture's values as a drawer's sheet animates by them (Compose's internal
/// `DrawerPredictiveBackState`): whether the gesture comes from the drawer's own edge, how far the sheet is
/// stretched (or shrunk) across and lowered, px. A cheap handle of states.
#[derive(Clone, PartialEq)]
pub(crate) struct DrawerPredictiveBackState {
    swipe_edge_matches_drawer: MutableState<bool>,
    scale_x_distance: MutableState<f32>,
    scale_y_distance: MutableState<f32>,
}

impl DrawerPredictiveBackState {
    pub(crate) fn new() -> DrawerPredictiveBackState {
        DrawerPredictiveBackState {
            swipe_edge_matches_drawer: mutable_state_of(true),
            scale_x_distance: mutable_float_state_of(0.0),
            scale_y_distance: mutable_float_state_of(0.0),
        }
    }

    pub(crate) fn swipe_edge_matches_drawer(&self) -> bool {
        self.swipe_edge_matches_drawer.get()
    }

    pub(crate) fn scale_x_distance(&self) -> f32 {
        self.scale_x_distance.get()
    }

    pub(crate) fn set_scale_x_distance(&self, value: f32) {
        self.scale_x_distance.set(value);
    }

    pub(crate) fn scale_y_distance(&self) -> f32 {
        self.scale_y_distance.get()
    }

    /// Takes the gesture at `progress` (eased) from the left edge or not (Compose's `update`): the stretch up to
    /// `max_scale_x_distance_grow` from the drawer's own edge (the left one, the right one right to left), else the
    /// shrinking up to `max_scale_x_distance_shrink`; the lowering up to `max_scale_y_distance`.
    pub(crate) fn update(
        &self,
        progress: f32,
        swipe_edge_left: bool,
        is_rtl: bool,
        max_scale_x_distance_grow: f32,
        max_scale_x_distance_shrink: f32,
        max_scale_y_distance: f32,
    ) {
        self.swipe_edge_matches_drawer.set(swipe_edge_left != is_rtl);
        let max_scale_x_distance =
            if self.swipe_edge_matches_drawer.get() { max_scale_x_distance_grow } else { max_scale_x_distance_shrink };
        self.scale_x_distance.set(lerp(0.0, max_scale_x_distance, progress));
        self.scale_y_distance.set(lerp(0.0, max_scale_y_distance, progress));
    }

    /// Back to no gesture (Compose's `clear`).
    pub(crate) fn clear(&self) {
        self.swipe_edge_matches_drawer.set(true);
        self.scale_x_distance.set(0.0);
        self.scale_y_distance.set(0.0);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DefaultDrawerItemsColor

/// Material's drawer item colors (Compose's private `DefaultDrawerItemsColor`): equal when their colors are.
#[derive(Clone, Copy, PartialEq, Debug)]
struct DefaultDrawerItemsColor {
    selected_icon_color: Color,
    unselected_icon_color: Color,
    selected_text_color: Color,
    unselected_text_color: Color,
    selected_container_color: Color,
    unselected_container_color: Color,
    selected_badge_color: Color,
    unselected_badge_color: Color,
}

impl NavigationDrawerItemColors for DefaultDrawerItemsColor {
    #[track_caller]
    fn icon_color(&self, selected: bool) -> State<Color> {
        remember_updated_color(if selected { self.selected_icon_color } else { self.unselected_icon_color })
    }

    #[track_caller]
    fn text_color(&self, selected: bool) -> State<Color> {
        remember_updated_color(if selected { self.selected_text_color } else { self.unselected_text_color })
    }

    #[track_caller]
    fn badge_color(&self, selected: bool) -> State<Color> {
        remember_updated_color(if selected { self.selected_badge_color } else { self.unselected_badge_color })
    }

    #[track_caller]
    fn container_color(&self, selected: bool) -> State<Color> {
        remember_updated_color(if selected { self.selected_container_color } else { self.unselected_container_color })
    }

    fn equals(&self, other: &dyn NavigationDrawerItemColors) -> bool {
        (other as &dyn Any).downcast_ref::<DefaultDrawerItemsColor>() == Some(self)
    }
}

/// Kotlin's `rememberUpdatedState(color)`: a remembered state given `color` at each composition (an observable
/// state, as Compose's; KompozR's `remember_updated_state` holds the value without telling readers).
#[track_caller]
fn remember_updated_color(color: Color) -> State<Color> {
    kompozable_block!({
        let state = remember(|| mutable_state_of(color));
        state.set(color);
        State::from(state)
    })
}

/// Where `pos` is between `a` and `b`, 0 to 1 (Compose's private `calculateFraction`).
fn calculate_fraction(a: f32, b: f32, pos: f32) -> f32 {
    ((pos - a) / (b - a)).clamp(0.0, 1.0)
}

/// How far between its anchors a drawer must be dragged to settle on the other (Compose's private
/// `DrawerPositionalThreshold`): half way.
const DrawerPositionalThreshold: f32 = 0.5;
/// How fast a fling must be to settle on the other anchor anyway (Compose's private `DrawerVelocityThreshold`).
const DrawerVelocityThreshold: Dp = Dp(400.0);
/// The narrowest a drawer's sheet is (Compose's private `MinimumDrawerWidth`).
const MinimumDrawerWidth: Dp = Dp(240.0);

/// How far a predictive back gesture from the drawer's own edge stretches the sheet across (Compose's internal
/// `PredictiveBackDrawerMaxScaleXDistanceGrow`).
pub(crate) const PredictiveBackDrawerMaxScaleXDistanceGrow: Dp = Dp(12.0);
/// How far one from the other edge shrinks it (Compose's internal `PredictiveBackDrawerMaxScaleXDistanceShrink`).
pub(crate) const PredictiveBackDrawerMaxScaleXDistanceShrink: Dp = Dp(24.0);
/// How far a gesture lowers it (Compose's internal `PredictiveBackDrawerMaxScaleYDistance`).
pub(crate) const PredictiveBackDrawerMaxScaleYDistance: Dp = Dp(48.0);

// TODO: b/177571613 this should be a proper decay settling
// this is taken from the DrawerLayout's DragViewHelper as a min duration.
/// The spec a drawer's anchored draggable state settles by (Compose's private
/// `AnchoredDraggableDefaultAnimationSpec`): a 256 ms tween.
#[allow(non_snake_case)]
fn AnchoredDraggableDefaultAnimationSpec() -> Arc<dyn FiniteAnimationSpec<f32>> {
    let spec: TweenSpec<f32> = named_args!(tween(duration_millis = 256));
    Arc::new(spec)
}

#[cfg(test)]
mod tests {
    //! The private parts (so tested here): a drawer state's density check, the predictive back values, the scrim's
    //! fraction.

    use super::*;

    #[test]
    fn a_drawer_states_velocity_threshold_needs_the_density_a_drawer_gives_it() {
        // (Compose's `requireDensity`: reached by a fling of a state no drawer was given — its anchored draggable
        // state is internal, so only from inside the library)
        let state = DrawerState(DrawerValue::Closed);
        let message = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            require_density(&state.inner.density, &Rc::downgrade(&state.inner))
        }))
        .expect_err("no density");
        let message = message.downcast_ref::<String>().cloned().unwrap_or_default();
        assert_eq!(
            message,
            "IllegalArgumentException: The density on DrawerState (DrawerState(currentValue=Closed, offset=NaN)) was \
             not set. Did you use DrawerState with the ModalNavigationDrawer or DismissibleNavigationDrawer \
             composables?"
        );
        state.set_density(Density::new(2.0, 1.0));
        let density = require_density(&state.inner.density, &Rc::downgrade(&state.inner));
        assert_eq!(density.to_px(DrawerVelocityThreshold), 800.0);
    }

    #[test]
    fn predictive_back_values_follow_the_gesture_and_the_drawers_edge() {
        let state = DrawerPredictiveBackState::new();
        state.update(0.5, true, false, 12.0, 24.0, 48.0);
        assert!(state.swipe_edge_matches_drawer() && state.scale_x_distance() == 6.0);
        assert_eq!(state.scale_y_distance(), 24.0);
        // Right to left the drawer's edge is the right one: from the left it shrinks.
        state.update(0.5, true, true, 12.0, 24.0, 48.0);
        assert!(!state.swipe_edge_matches_drawer() && state.scale_x_distance() == 12.0);
        state.clear();
        assert!(
            state.swipe_edge_matches_drawer() && state.scale_x_distance() == 0.0 && state.scale_y_distance() == 0.0
        );
        assert_eq!(calculate_fraction(-360.0, 0.0, -90.0), 0.75);
        assert_eq!(calculate_fraction(-360.0, 0.0, 20.0), 1.0);
    }
}
