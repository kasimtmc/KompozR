//! The host: a composition, its layout tree and what the platform feeds it (Compose's AndroidComposeView /
//! ComposeScene, C++'s KompozHost). It knows nothing of windows or GPUs: a platform calls `frame` and `draw`, sends
//! input and the environment, and gets the callbacks only it can serve.

use crate::back::BackDispatcher;
use crate::focus::{FocusManager, FocusOwner};
use crate::graphics_context::GraphicsContext;
use crate::input::{InputState, Key, KeyModifiers, PointerType};
use crate::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable};
use crate::node::{LayoutNode, Owner};
use crate::platform::*;
use crate::semantics::SemanticsOwner;
use crate::unit::Constraints;
use crate::window::Overlays;
use kompoz_coroutines::{Dispatcher, LocalDispatcher};
use kompoz_graphics::{Density, IntOffset, IntSize, LayoutDirection, Offset};
use kompoz_runtime::{
    Composition, Content, KompozitionLocalProvider, MutableState, RecompositionStats, Snapshot,
    global_snapshot_changed, kompozable_block, mutable_state_of, restartable,
};
use parking_lot::Mutex;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// What the host provides at the root, as the platform sets it.
#[derive(Clone, PartialEq, Debug)]
struct Environment {
    density: Density,
    layout_direction: LayoutDirection,
    theme: SystemTheme,
    insets: SystemInsets,
    view_configuration: ViewConfiguration,
}

/// The root's measure policy (Compose's RootMeasurePolicy): the children measured in the window's constraints, as
/// big as the biggest, each placed at the top left through a layer.
struct RootMeasurePolicy {
    layer: crate::layout::LayerBlock,
}

impl MeasurePolicy for RootMeasurePolicy {
    fn measure(&self, scope: &MeasureScope, measurables: &[&dyn Measurable], c: Constraints) -> MeasureResult {
        if measurables.is_empty() {
            return scope.layout(c.min_width, c.min_height, |_| {});
        }
        let placeables: Vec<Placeable> = measurables.iter().map(|m| m.measure(c)).collect();
        let width = placeables.iter().map(|p| p.width()).max().unwrap_or(0);
        let height = placeables.iter().map(|p| p.height()).max().unwrap_or(0);
        let layer = self.layer.clone();
        scope.layout(c.constrain_width(width), c.constrain_height(height), move |p| {
            for placeable in &placeables {
                p.place_relative_with_layer_block(placeable, IntOffset::Zero, layer.clone(), 0.0);
            }
        })
    }
}

/// What another thread may do to a host: post a task to run at the next frame, ask for a frame, have everything
/// composed again. Cheap to clone, `Send` and `Sync`.
#[derive(Clone)]
pub struct HostHandle {
    posted: Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>,
    waker: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
    invalid: Arc<AtomicBool>,
}

impl HostHandle {
    /// Runs `task` on the UI thread at the start of the next frame (asking for one).
    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        self.posted.lock().push(Box::new(task));
        self.wake();
    }

    /// Asks the platform for a frame.
    pub fn wake(&self) {
        let waker = self.waker.lock().clone();
        if let Some(w) = waker {
            w();
        }
    }

    /// Everything composed and measured again at the next frame.
    pub fn invalidate(&self) {
        self.invalid.store(true, Ordering::Release);
        self.wake();
    }
}

/// A host (C++'s KompozHost): owns the composition, the root layout node, the dispatcher its coroutines run on, the
/// owner (the measure-and-layout delegate, the read observer), and the state of the later packages: input, focus,
/// overlays, back handlers.
pub struct KompozHost {
    owner: Rc<Owner>,
    root: Rc<LayoutNode>,
    composition: RefCell<Option<Composition>>,
    dispatcher: Dispatcher,
    platform: PlatformHandle,
    handle: HostHandle,
    environment: MutableState<Environment>,
    window_info: WindowInfo,
    input_mode_manager: InputModeManager,
    viewport: Cell<IntSize>,
    frame_time: Cell<i64>,
    layout_check: Cell<bool>,
    layout_check_mismatches: RefCell<Vec<String>>,
    /// Counted up by `invalidate`: the content's scope composes again when it changes.
    generation: Rc<Cell<u64>>,
    /// The input package's state.
    pub input: RefCell<InputState>,
    /// The focus package's state.
    pub focus: Rc<RefCell<FocusOwner>>,
    /// The windows package's overlays.
    pub overlays: RefCell<Overlays>,
    /// The back handlers.
    pub back: RefCell<BackDispatcher>,
}

impl KompozHost {
    /// A host composing `content`, headless until given callbacks; its viewport 0 by 0 until set.
    pub fn new(content: impl Content + 'static) -> KompozHost {
        let dispatcher = Dispatcher::new();
        let owner = Owner::new(dispatcher.clone());
        let root = LayoutNode::new();
        root.set_measure_policy(Rc::new(RootMeasurePolicy { layer: Rc::new(|_| {}) }));
        let focus = Rc::new(RefCell::new(FocusOwner::default()));
        owner.set_focus_owner(&focus);
        owner.set_root(root.clone());
        let environment = mutable_state_of(Environment {
            density: Density::default(),
            layout_direction: LayoutDirection::Ltr,
            theme: SystemTheme::Light,
            insets: SystemInsets::default(),
            view_configuration: ViewConfiguration::default(),
        });
        let host = KompozHost {
            owner,
            root: root.clone(),
            composition: RefCell::new(None),
            dispatcher,
            platform: PlatformHandle::default(),
            handle: HostHandle {
                posted: Arc::new(Mutex::new(Vec::new())),
                waker: Arc::new(Mutex::new(None)),
                invalid: Arc::new(AtomicBool::new(false)),
            },
            environment,
            window_info: WindowInfo::default(),
            input_mode_manager: InputModeManager::default(),
            viewport: Cell::new(IntSize::Zero),
            frame_time: Cell::new(0),
            layout_check: Cell::new(false),
            layout_check_mismatches: RefCell::new(Vec::new()),
            generation: Rc::new(Cell::new(0)),
            input: RefCell::new(InputState::default()),
            focus,
            overlays: RefCell::new(Overlays::default()),
            back: RefCell::new(BackDispatcher::default()),
        };
        let composition = Composition::new(Some(root as Rc<dyn kompoz_runtime::Node>));
        composition.set_content(host.root_content(Rc::new(move || content.compose())));
        *host.composition.borrow_mut() = Some(composition);
        host
    }

    /// The root content: the environment's locals provided, then `content` in a scope of its own (composed again
    /// only when what it reads changes, not when the environment does; all of it after `invalidate`).
    fn root_content(&self, content: Rc<dyn Fn()>) -> impl Fn() + 'static {
        let generation = self.generation.clone();
        let environment = self.environment.clone();
        let window_info = self.window_info.clone();
        let input_mode_manager = self.input_mode_manager.clone();
        let platform = self.platform.clone();
        let dispatcher = self.dispatcher.clone();
        let focus_manager = FocusManager { owner: Some(Rc::downgrade(&self.focus)) };
        move || {
            let env = environment.get();
            let content = content.clone();
            let values = [
                LocalDensity.provides(env.density),
                LocalLayoutDirection.provides(env.layout_direction),
                LocalViewConfiguration.provides(env.view_configuration),
                LocalSystemTheme.provides(env.theme),
                LocalSystemInsets.provides(env.insets),
                LocalWindowInfo.provides(window_info.clone()),
                LocalInputModeManager.provides(input_mode_manager.clone()),
                LocalHapticFeedback.provides(HapticFeedback(platform.clone())),
                LocalClipboard.provides(Clipboard(platform.clone())),
                LocalUriHandler.provides(UriHandler(platform.clone())),
                LocalSoftwareKeyboardController.provides(SoftwareKeyboardController(platform.clone())),
                LocalPlatform.provides(platform.clone()),
                LocalFocusManager.provides(focus_manager.clone()),
                LocalDispatcher.provides(Some(dispatcher.clone())),
            ];
            let same_generation: fn(&u64, &u64) -> bool = |a, b| a == b;
            let g = generation.get();
            kompozable_block!({
                KompozitionLocalProvider(&values, move || restartable(g, Some(same_generation), move |_| content()))
            })
        }
    }

    /// Composes `content` instead (at the next frame).
    pub fn set_content(&self, content: impl Content + 'static) {
        let root = self.root_content(Rc::new(move || content.compose()));
        if let Some(c) = self.composition.borrow().as_ref() {
            c.set_content(root);
        }
    }

    /// The platform's callbacks (and the waker they give, which `wake` and `post` call from any thread).
    pub fn set_callbacks(&self, callbacks: Rc<dyn PlatformCallbacks>) {
        let waker = callbacks.waker();
        *self.handle.waker.lock() = waker.clone();
        if let Some(w) = waker {
            let notify = w.clone();
            self.dispatcher.set_notify(move || notify());
            kompoz_runtime::set_global_write_observer(move || w());
        }
        self.platform.set(callbacks);
    }

    /// What other threads use to post tasks and ask for frames.
    pub fn handle(&self) -> HostHandle {
        self.handle.clone()
    }

    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        self.handle.post(task);
    }

    pub fn wake(&self) {
        self.handle.wake();
    }

    /// Everything composed and measured again at the next frame (thread-safe through `handle()`).
    pub fn invalidate(&self) {
        self.handle.invalidate();
    }

    // ---- The environment ----

    /// The window's size in px: the root's constraints (Compose's root is measured within it, not stretched).
    pub fn set_viewport_size(&self, size: IntSize) {
        self.viewport.set(size);
        self.owner.update_root_constraints(Constraints::new(0, size.width, 0, size.height));
        self.window_info.container_size.set(size);
    }

    pub fn viewport_size(&self) -> IntSize {
        self.viewport.get()
    }

    fn update_environment(&self, f: impl FnOnce(&mut Environment)) {
        let mut env = self.environment.peek();
        f(&mut env);
        if env != self.environment.peek() {
            self.environment.set(env);
        }
    }

    /// Pixels per dp and the font scale (LocalDensity).
    pub fn set_density(&self, density: Density) {
        self.update_environment(|e| {
            e.density = density;
            e.view_configuration = ViewConfiguration::for_density(density);
        });
    }

    pub fn density(&self) -> Density {
        self.environment.peek().density
    }

    /// The system font size (LocalDensity's font scale).
    pub fn set_font_scale(&self, font_scale: f32) {
        self.update_environment(|e| e.density.font_scale = font_scale);
    }

    /// The app's direction (LocalLayoutDirection).
    pub fn set_layout_direction(&self, direction: LayoutDirection) {
        self.update_environment(|e| e.layout_direction = direction);
    }

    /// The system's theme (what `is_system_in_dark_theme` reads).
    pub fn set_system_dark_theme(&self, dark: bool) {
        self.update_environment(|e| e.theme = if dark { SystemTheme::Dark } else { SystemTheme::Light });
    }

    /// What each kind of system UI covers (LocalSystemInsets, the window insets).
    pub fn set_system_insets(&self, insets: SystemInsets) {
        self.update_environment(|e| e.insets = insets);
    }

    /// The platform's gesture thresholds (LocalViewConfiguration).
    pub fn set_view_configuration(&self, configuration: ViewConfiguration) {
        self.update_environment(|e| e.view_configuration = configuration);
    }

    /// Whether the window has the focus (LocalWindowInfo).
    pub fn set_window_focused(&self, focused: bool) {
        self.window_info.is_window_focused.set(focused);
    }

    /// Where the window's content is on the screen's (what `position_in_window` adds).
    pub fn set_window_offset(&self, offset: Offset) {
        self.owner.set_window_offset(offset);
        self.owner.dispatch_root_positioned();
    }

    // ---- Frames ----

    /// A frame at `time_nanos` (C++'s `frame`): posted tasks, then the dispatcher (due timers, then frame waiters),
    /// then up to four passes of {applied changes told to their readers → recompose → measure and layout → global
    /// position callbacks} while a pass wrote state. Whether a redraw is due.
    pub fn frame(&self, time_nanos: i64) -> bool {
        let tasks = std::mem::take(&mut *self.handle.posted.lock());
        for task in tasks {
            task();
        }
        self.frame_time.set(time_nanos);
        self.dispatcher.run(time_nanos);
        let composition = self.composition.borrow();
        let Some(composition) = composition.as_ref() else { return false };
        for pass in 0..4 {
            Snapshot::send_apply_notifications();
            self.owner.snapshot_observer().process_changes();
            if self.handle.invalid.swap(false, Ordering::AcqRel) {
                self.generation.set(self.generation.get() + 1);
                composition.invalidate_all();
                self.root.invalidate_measurement_for_subtree();
            }
            if pass == 0 {
                composition.invalidate_frame_scopes(time_nanos);
            }
            let recomposed = composition.recompose(time_nanos);
            if recomposed {
                self.owner.run_end_apply_listeners();
                // (what the composition's own snapshot changed reaches the layout's readers too)
                self.owner.snapshot_observer().process_changes();
            }
            if !self.owner.has_pending_measure_or_layout() && !self.owner.has_pending_on_positioned_callbacks() {
                if global_snapshot_changed() || self.owner.snapshot_observer().has_pending() {
                    continue;
                }
                break;
            }
            self.owner.measure_and_layout();
            self.owner.dispatch_on_positioned_callbacks();
            self.overlays.borrow_mut().prune();
            self.focus.borrow_mut().prune();
            if global_snapshot_changed() || self.owner.snapshot_observer().has_pending() {
                continue;
            }
            if self.layout_check.get() && !composition.has_invalidations() {
                let mismatches = self.owner.check_layout();
                self.layout_check_mismatches.borrow_mut().extend(mismatches);
            }
            break;
        }
        self.owner.take_needs_draw()
    }

    /// Draws the tree, then the overlays, into `canvas`.
    pub fn draw(&self, canvas: &mut dyn GraphicsContext) {
        self.root.draw(canvas);
        self.overlays.borrow().draw(canvas);
    }

    /// Whether the next frame has work: states changed and not yet seen, scopes or layouts waiting, coroutines
    /// woken or waiting for a frame, tasks posted (not a timer: see `next_deadline`; not a redraw alone).
    pub fn has_pending_work(&self) -> bool {
        let composition = self.composition.borrow();
        let composing = composition.as_ref().is_some_and(|c| c.has_invalidations() || c.has_frame_scopes());
        composing
            || global_snapshot_changed()
            || self.owner.snapshot_observer().has_pending()
            || self.owner.has_pending_measure_or_layout()
            || self.dispatcher.has_ready()
            || self.dispatcher.has_frame_waiters()
            || !self.handle.posted.lock().is_empty()
            || self.handle.invalid.load(Ordering::Acquire)
    }

    /// When the next coroutine timer is due (the platform wakes then), if any.
    pub fn next_deadline(&self) -> Option<i64> {
        self.dispatcher.next_deadline()
    }

    /// The time of the last frame.
    pub fn frame_time(&self) -> i64 {
        self.frame_time.get()
    }

    // ---- Input (the input and focus packages dispatch it) ----

    /// A pointer went down at `position` (px in the window), at `time_nanos` on the frame clock (-1: the last
    /// frame's). Whether a node took it.
    pub fn pointer_down(&self, position: Offset, time_nanos: i64, pointer_id: i64, pointer_type: PointerType) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_down(&mut self.input.borrow_mut(), &self.root, position, t, pointer_id, pointer_type)
    }

    pub fn pointer_move(&self, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_move(&mut self.input.borrow_mut(), &self.root, position, t, pointer_id)
    }

    pub fn pointer_up(&self, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_up(&mut self.input.borrow_mut(), &self.root, position, t, pointer_id)
    }

    pub fn pointer_cancel(&self) {
        crate::input::pointer_cancel(&mut self.input.borrow_mut(), &self.root);
    }

    pub fn pointer_exit(&self) {
        crate::input::pointer_exit(&mut self.input.borrow_mut(), &self.root);
    }

    /// A wheel or touchpad scrolled by `delta` px at `position`.
    pub fn scroll(&self, position: Offset, delta: Offset) -> bool {
        crate::input::scroll(&mut self.input.borrow_mut(), &self.root, position, delta)
    }

    /// A key went down; whether a node (or the focus) used it.
    pub fn key_down(&self, key: Key, modifiers: KeyModifiers) -> bool {
        crate::input::key_down(&mut self.input.borrow_mut(), &self.root, key, modifiers)
    }

    pub fn key_up(&self, key: Key, modifiers: KeyModifiers) -> bool {
        crate::input::key_up(&mut self.input.borrow_mut(), &self.root, key, modifiers)
    }

    /// Back pressed (or a back gesture done): the most recent enabled back handler; false if none took it.
    pub fn back_invoked(&self) -> bool {
        self.back.borrow_mut().back_invoked()
    }

    /// An event's time on the frame clock: as given, or the last frame's; the dispatcher's clock moved to it.
    fn event_time(&self, time_nanos: i64) -> i64 {
        let t = if time_nanos < 0 { self.frame_time.get() } else { time_nanos };
        self.dispatcher.advance_to(t);
        t
    }

    // ---- What the tree tells ----

    /// The root layout node.
    pub fn root(&self) -> &Rc<LayoutNode> {
        &self.root
    }

    /// The semantics of the tree.
    pub fn semantics_owner(&self) -> SemanticsOwner {
        SemanticsOwner { root: Rc::downgrade(&self.root) }
    }

    /// The dispatcher the host runs coroutines on.
    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    pub fn stats(&self) -> RecompositionStats {
        self.composition.borrow().as_ref().map(|c| c.stats()).unwrap_or_default()
    }

    /// Whether a `keep_screen_on` is in the tree: the platform keeps the screen on.
    pub fn keep_screen_on(&self) -> bool {
        self.owner.keep_screen_on.get() > 0
    }

    /// Whether something sensitive is in the tree: the platform protects the window.
    pub fn sensitive_content(&self) -> bool {
        self.owner.sensitive_content.get() > 0
    }

    /// The fastest refresh rate a node in the tree asked for (Hz), 0 if none.
    pub fn preferred_frame_rate(&self) -> f32 {
        self.owner
            .frame_rates
            .borrow()
            .iter()
            .filter_map(|(_, r)| if let FrameRate::Rate(hz) = r { Some(*hz) } else { None })
            .fold(0.0, f32::max)
    }

    /// The highest refresh rate category a node asked for.
    pub fn preferred_frame_rate_category(&self) -> FrameRateCategory {
        self.owner
            .frame_rates
            .borrow()
            .iter()
            .filter_map(|(_, r)| if let FrameRate::Category(c) = r { Some(*c) } else { None })
            .max()
            .unwrap_or_default()
    }

    /// How many times nodes were measured or placed so far (C++'s `relaidOutCount`; tests read it).
    pub fn relaid_out_count(&self) -> u64 {
        self.owner.relaid_out.get()
    }

    /// Turns the layout check on (C++'s KOMPOZ_LAYOUT_CHECK): after each settled frame every node is measured and
    /// placed again from scratch, which must come out the same.
    pub fn set_layout_check(&self, on: bool) {
        self.layout_check.set(on);
    }

    /// The differences the layout check found so far (taken).
    pub fn take_layout_check_mismatches(&self) -> Vec<String> {
        std::mem::take(&mut *self.layout_check_mismatches.borrow_mut())
    }

    /// Whether a redraw is due (something drawn changed since the last frame said so).
    pub fn needs_draw(&self) -> bool {
        self.owner.needs_draw()
    }

    /// Whether the semantics changed since this was last asked (what an accessibility bridge sends on).
    pub fn take_semantics_changed(&self) -> bool {
        self.owner.take_semantics_dirty()
    }
}

impl Drop for KompozHost {
    /// The composition disposed of (what it holds forgotten), then the tree detached.
    fn drop(&mut self) {
        self.composition.borrow_mut().take();
        if self.root.is_attached() {
            self.root.detach();
        }
    }
}
