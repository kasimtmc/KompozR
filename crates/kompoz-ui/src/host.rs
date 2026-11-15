//! The host: a composition, its layout tree and what the platform feeds it (Compose's AndroidComposeView /
//! ComposeScene, C++'s KompozHost). It knows nothing of windows or GPUs: a platform calls `frame` and `draw`, sends
//! input and the environment, and gets the callbacks only it can serve.

use crate::back::{BackDispatcher, BackDispatcherRef, BackEventCompat, LocalBackDispatcher};
use crate::display_list::DisplayList;
use crate::focus::{FocusDirection, FocusManager, FocusOwner};
use crate::graphics_context::GraphicsContext;
use crate::input::{
    IndirectPointerEvent, IndirectPointerEventPrimaryDirectionalMotionAxis, IndirectPointerEventType, InputState, Key,
    KeyEvent, KeyEventType, KeyModifiers, PointerButton, PointerType, RotaryScrollEvent,
};
use crate::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable};
use crate::node::{LayoutNode, Owner};
use crate::platform::*;
use crate::semantics::{AccessibilityNode, SemanticsAction, SemanticsOwner};
use crate::unit::Constraints;
use crate::window::Overlays;
use kompoz_coroutines::{Dispatcher, LocalDispatcher};
use kompoz_graphics::{Density, IntOffset, IntSize, LayoutDirection, Offset};
use kompoz_runtime::{
    Composition, Content, KompozitionLocalProvider, MutableState, ProvideSaveableStateRegistry, RecompositionStats,
    SaveableStateRegistry, SavedValue, Snapshot, global_snapshot_changed, kompozable_block, mutable_state_of,
    restartable,
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

/// A frame's time on the display's beat, as Android's Choreographer gives it (C++'s `frameTimeOnBeat`): a whole
/// number of refresh periods after the last frame's (`last`; none yet: `now`), the nearest to `now`, so what
/// animates moves by equal steps however late in its period each frame starts; at most half a period from `now`,
/// never before `last` (frames within half a period of it share its time). No period known: `now`.
pub fn frame_time_on_beat(last: Option<i64>, now: i64, period: i64) -> i64 {
    match last {
        Some(last) if period > 0 => last + ((now - last + period / 2) / period).max(0) * period,
        _ => now,
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
    /// What the system UI covers, in states of its own (read live by what measures by the insets).
    system_insets: SystemInsetsHolder,
    /// Moving the keyboard by hand.
    ime_controller: ImeController,
    /// The composition's saveable values (`remember_saveable` outside any holder, what nodes register), written out
    /// with the host's state.
    saveable: SaveableStateRegistry,
    viewport: Cell<IntSize>,
    frame_time: Cell<i64>,
    layout_check: Cell<bool>,
    layout_check_mismatches: RefCell<Vec<String>>,
    /// The keep check's: on, its two lists (one keeping the nodes that didn't change, one drawing all), and what it
    /// found.
    keep_check: Cell<bool>,
    keep_check_lists: RefCell<Option<Box<(DisplayList, DisplayList)>>>,
    keep_check_mismatches: RefCell<Vec<String>>,
    /// Counted up by `invalidate`: the content's scope composes again when it changes.
    generation: Rc<Cell<u64>>,
    /// The input package's state.
    pub input: RefCell<InputState>,
    /// The focus: the active target, moving and clearing it, key dispatch.
    pub focus: Rc<FocusOwner>,
    /// The popups' and dialogs' roots.
    pub overlays: Overlays,
    /// The back handlers.
    pub back: Rc<BackDispatcher>,
}

impl KompozHost {
    /// A host composing `content`, headless until given callbacks; its viewport 0 by 0 until set.
    pub fn new(content: impl Content + 'static) -> KompozHost {
        let dispatcher = Dispatcher::new();
        let owner = Owner::new(dispatcher.clone());
        let root = LayoutNode::new();
        root.set_measure_policy(Rc::new(RootMeasurePolicy { layer: Rc::new(|_| {}) }));
        let system_insets = SystemInsetsHolder::default();
        root.set_modifier(
            crate::focus::window_root_modifier().then(crate::layout::window_insets_rulers(system_insets.clone())),
        );
        let focus = Rc::new(FocusOwner::default());
        *focus.owner.borrow_mut() = Rc::downgrade(&owner);
        owner.set_focus_owner(&focus);
        owner.set_root(root.clone());
        let environment = mutable_state_of(Environment {
            density: Density::default(),
            layout_direction: LayoutDirection::Ltr,
            theme: SystemTheme::Light,
            view_configuration: ViewConfiguration::default(),
        });
        let overlays = Overlays { owner: Rc::downgrade(&owner) };
        let platform = PlatformHandle::default();
        let host = KompozHost {
            owner,
            root: root.clone(),
            composition: RefCell::new(None),
            dispatcher,
            platform: platform.clone(),
            handle: HostHandle {
                posted: Arc::new(Mutex::new(Vec::new())),
                waker: Arc::new(Mutex::new(None)),
                invalid: Arc::new(AtomicBool::new(false)),
            },
            environment,
            window_info: WindowInfo::default(),
            input_mode_manager: InputModeManager::default(),
            ime_controller: ImeController::new(platform.clone(), system_insets.clone(), Some(Rc::downgrade(&focus))),
            system_insets,
            saveable: SaveableStateRegistry::new(None),
            viewport: Cell::new(IntSize::Zero),
            frame_time: Cell::new(0),
            layout_check: Cell::new(false),
            layout_check_mismatches: RefCell::new(Vec::new()),
            keep_check: Cell::new(false),
            keep_check_lists: RefCell::new(None),
            keep_check_mismatches: RefCell::new(Vec::new()),
            generation: Rc::new(Cell::new(0)),
            input: RefCell::new(InputState::default()),
            overlays,
            focus,
            back: Rc::new(BackDispatcher::default()),
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
        let system_insets = self.system_insets.clone();
        let ime_controller = self.ime_controller.clone();
        let saveable = self.saveable.clone();
        let platform = self.platform.clone();
        let dispatcher = self.dispatcher.clone();
        let focus_manager = FocusManager { owner: Some(Rc::downgrade(&self.focus)) };
        let back = BackDispatcherRef(Rc::downgrade(&self.back));
        move || {
            let env = environment.get();
            let content = content.clone();
            let values = [
                LocalDensity.provides(env.density),
                LocalLayoutDirection.provides(env.layout_direction),
                LocalViewConfiguration.provides(env.view_configuration),
                LocalSystemTheme.provides(env.theme),
                LocalSystemInsets.provides(system_insets.clone()),
                LocalImeController.provides(ime_controller.clone()),
                LocalWindowInfo.provides(window_info.clone()),
                LocalInputModeManager.provides(input_mode_manager.clone()),
                LocalHapticFeedback.provides(HapticFeedback(platform.clone())),
                LocalClipboard.provides(Clipboard(platform.clone())),
                LocalUriHandler.provides(UriHandler(platform.clone())),
                LocalSoftwareKeyboardController.provides(SoftwareKeyboardController(platform.clone())),
                LocalPlatform.provides(platform.clone()),
                LocalFocusManager.provides(focus_manager.clone()),
                LocalBackDispatcher.provides(Some(back.clone())),
                LocalDispatcher.provides(Some(dispatcher.clone())),
            ];
            let same_generation: fn(&u64, &u64) -> bool = |a, b| a == b;
            let g = generation.get();
            let saveable = saveable.clone();
            kompozable_block!({
                KompozitionLocalProvider(&values, move || {
                    ProvideSaveableStateRegistry(&saveable, move || {
                        restartable(g, Some(same_generation), move |_| content())
                    })
                })
            })
        }
    }

    /// Composes `content` instead (at the next frame): a generation of its own, so the content's scope (which
    /// skips while its generation is the same) composes the new content.
    pub fn set_content(&self, content: impl Content + 'static) {
        self.generation.set(self.generation.get() + 1);
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

    /// What each kind of system UI covers (LocalSystemInsets, the window insets): the kinds that changed are set
    /// in their states, so what measures by them measures again (an IME animation composes nothing).
    pub fn set_system_insets(&self, insets: SystemInsets) {
        self.system_insets.set(&insets);
    }

    /// The state to keep while the platform ends the app (C++'s `saveState`): the composition's saveable values
    /// (`remember_saveable` outside any holder, what nodes registered: a focus restorer's child), as bytes.
    pub fn save_state(&self) -> Vec<u8> {
        SavedValue::map([("composition", self.saveable.save())]).encode()
    }

    /// Gives back a state `save_state` wrote (C++'s `restoreState`): call it before the first frame, and what
    /// composes finds its values again. Bytes that aren't such a state are ignored.
    pub fn restore_state(&self, bytes: &[u8]) {
        let Some(saved) = SavedValue::decode(bytes) else { return };
        let composition = saved.get("composition");
        if composition.as_map().is_some() {
            self.saveable.restore(composition);
        }
    }

    /// Moves the keyboard by `px` up (down if negative) by hand (C++'s `imeControlBy`); what it moved. See
    /// [`ImeController::control_by`].
    pub fn ime_control_by(&self, px: f32) -> f32 {
        self.ime_controller.control_by(px)
    }

    /// Lets go of the keyboard moved by hand (C++'s `imeControlEnd`).
    pub fn ime_control_end(&self, velocity: f32) {
        self.ime_controller.control_end(velocity);
    }

    /// Whether the keyboard is being moved by hand (C++'s `imeControlling`).
    pub fn ime_controlling(&self) -> bool {
        self.ime_controller.is_controlling()
    }

    /// What the system UI covers, as last set.
    pub fn system_insets(&self) -> SystemInsets {
        kompoz_runtime::without_read_observation(|| self.system_insets.get())
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
            // (focus targets whose properties read what changed, worked out)
            self.focus.process_invalidations();
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
                // (the focus nodes the composition changed, worked out: Compose's FocusInvalidationManager)
                self.focus.process_invalidations();
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
            self.focus.process_invalidations();
            self.focus.prune();
            if global_snapshot_changed() || self.owner.snapshot_observer().has_pending() {
                continue;
            }
            if self.layout_check.get() && !composition.has_invalidations() {
                let mismatches = self.owner.check_layout();
                self.layout_check_mismatches.borrow_mut().extend(mismatches);
            }
            break;
        }
        self.owner.dispatch_semantics_changes();
        if self.owner.take_gesture_exclusion_changed() {
            let rects = self.owner.gesture_exclusion();
            self.platform.get().system_gesture_exclusion(&rects);
        }
        let redraw = self.owner.take_needs_draw();
        if self.keep_check.get() {
            self.check_kept();
            self.owner.take_needs_draw(); // (drawn for the check: no frame asked for by it)
        }
        redraw
    }

    /// The keep check (C++'s `checkKept`): the frame drawn into a display list that keeps the nodes that didn't
    /// change since it last drew, and into one that draws them all, which must record the same. Its keeping list
    /// goes by a dirty flag of its own (swapped in while it draws), so the platform's keeping list still sees what
    /// changed since it drew.
    fn check_kept(&self) {
        let mut lists = self.keep_check_lists.borrow_mut().take().unwrap_or_else(|| {
            let mut kept = DisplayList::new();
            kept.set_keep_nodes(true);
            Box::new((kept, DisplayList::new()))
        });
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![self.root.clone()];
        stack.extend(self.overlays.roots());
        while let Some(n) = stack.pop() {
            if seen.insert(n.id()) {
                stack.extend(n.children());
                nodes.push(n);
            }
        }
        let swap = |nodes: &[Rc<LayoutNode>]| {
            for n in nodes {
                let dirty = n.draw_dirty.replace(n.keep_check_dirty.get());
                n.keep_check_dirty.set(dirty);
            }
        };
        let viewport = self.viewport.get().to_size();
        swap(&nodes);
        let (kept, fresh) = &mut *lists;
        for list in [&mut *kept, &mut *fresh] {
            list.begin(viewport);
            self.draw(list);
            list.end();
        }
        swap(&nodes);
        if let Err(why) = kept.same_frame(fresh) {
            self.keep_check_mismatches.borrow_mut().push(format!("kompoz: keep check: {why}"));
        }
        *self.keep_check_lists.borrow_mut() = Some(lists);
    }

    /// Draws the tree, then the overlays, into `canvas`.
    pub fn draw(&self, canvas: &mut dyn GraphicsContext) {
        self.root.draw(canvas);
        self.overlays.draw(canvas);
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
            || self.focus.has_pending_invalidation()
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
        // (a finger puts the UI in touch mode, as Compose's AndroidComposeView does)
        if pointer_type == PointerType::Touch && self.input_mode_manager.mode.peek() != InputMode::Touch {
            self.input_mode_manager.request_input_mode(InputMode::Touch);
        }
        crate::input::pointer_down(self, position, t, pointer_id, pointer_type)
    }

    /// A pointer moved: a finger down, else the mouse (hovering, or pressed).
    pub fn pointer_move(&self, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_move(self, position, t, pointer_id)
    }

    /// Several fingers moved at once, `(pointer_id, position)` each: one event (a platform batching its pointers).
    pub fn pointer_moves(&self, moves: &[(i64, Offset)], time_nanos: i64) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_moves(self, moves, t)
    }

    pub fn pointer_up(&self, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_up(self, position, t, pointer_id)
    }

    /// A mouse button pressed or let go with the mouse at `position`.
    pub fn mouse_button(&self, position: Offset, time_nanos: i64, button: PointerButton, pressed: bool) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::mouse_button(self, position, t, button, pressed, None)
    }

    pub fn pointer_cancel(&self) {
        crate::input::pointer_cancel(self);
    }

    pub fn pointer_exit(&self) {
        crate::input::pointer_exit(self);
    }

    /// A wheel or touchpad scrolled by `delta` px (down and right positive) with the mouse at `position`; whether
    /// a node took it.
    pub fn scroll(&self, position: Offset, delta: Offset) -> bool {
        crate::input::scroll(self, position, delta)
    }

    /// A key went down (its character the one it types on a US keyboard); whether a node, the focus or back
    /// took it.
    pub fn key_down(&self, key: Key, modifiers: KeyModifiers) -> bool {
        self.send_key_event(KeyEvent::new(key, KeyEventType::KeyDown, modifiers))
    }

    pub fn key_up(&self, key: Key, modifiers: KeyModifiers) -> bool {
        self.send_key_event(KeyEvent::new(key, KeyEventType::KeyUp, modifiers))
    }

    /// A key event (with the platform's character): along the focused path (previews down, events up), then the
    /// default (the root's: Tab and arrows move the focus; Escape and Back go back). Whether it was taken.
    pub fn send_key_event(&self, event: KeyEvent) -> bool {
        crate::input::key_event(self, event)
    }

    /// A rotary scroll (a crown, a wheel over the focused node) along the focused path; whether a node took it.
    pub fn rotary_scroll(&self, event: RotaryScrollEvent) -> bool {
        crate::input::rotary_event(self, event)
    }

    /// An indirect pointer event (a touchpad not over the window, a remote's swipe surface), along the focused path
    /// (Compose's `sendIndirectPointerEvent`); whether a node consumed it.
    pub fn send_indirect_pointer_event(&self, event: &IndirectPointerEvent) -> bool {
        crate::input::indirect_pointer_event(self, event)
    }

    /// One indirect pointer pressed, moved or let go at `position` (on its device) at `time_nanos` (-1: the last
    /// frame's): an event made with where it was before, along the focused path. Whether a node consumed it.
    pub fn indirect_pointer(
        &self,
        r#type: IndirectPointerEventType,
        pointer_id: i64,
        position: Offset,
        pressure: f32,
        time_nanos: i64,
        axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
    ) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::indirect_pointer(self, r#type, pointer_id, position, pressure, t, axis)
    }

    /// The indirect pointers' stream called off (Compose's `dispatchIndirectPointerCancel`).
    pub fn indirect_pointer_cancel(&self) {
        crate::input::indirect_pointer_cancel(self);
    }

    // ---- Back ----

    /// The back scope of the focusable popup or dialog on top, if any.
    fn top_back_scope(&self) -> Option<crate::back::BackScope> {
        self.owner.top_focusable_popup().and_then(|p| p.popup_spec()).map(|s| s.back_scope.clone())
    }

    /// Back pressed (not a gesture): a focusable popup or a dialog on top takes it (its handlers, else its
    /// dismissal); else the most recent enabled back handler. False if nothing took it (the platform's default:
    /// leaving the app).
    pub(crate) fn back_pressed(&self) -> bool {
        if let Some(taken) = crate::window::popup_back(self) {
            return taken;
        }
        self.back.pressed(None)
    }

    /// A back gesture started: it goes to the handler that would take back now (a popup's on top; none there:
    /// the popup is dismissed when the gesture ends).
    pub fn back_started(&self, event: BackEventCompat) {
        let scope = self.top_back_scope();
        self.back.started(scope.as_ref(), event);
    }

    /// A step of the back gesture.
    pub fn back_progressed(&self, event: BackEventCompat) {
        self.back.progressed(event);
    }

    /// The back gesture was called off.
    pub fn back_cancelled(&self) {
        self.back.cancelled();
    }

    /// Back invoked (a press, or a gesture's end): the gesture's handler if it still takes back, else as a press.
    /// False if nothing took it.
    pub fn back_invoked(&self) -> bool {
        let scope = self.top_back_scope();
        if self.back.gesture_invoked(scope.as_ref()) {
            return true;
        }
        self.back_pressed()
    }

    /// Whether back would do something (a focusable popup or a dialog on top, or an enabled handler): the
    /// platform lets the app handle back (Android's predictive back) only then.
    pub fn wants_back(&self) -> bool {
        self.owner.top_focusable_popup().is_some() || self.back.target(None).is_some()
    }

    // ---- Focus ----

    /// The layout node of the focused target, if any.
    pub fn focused_node(&self) -> Option<Rc<LayoutNode>> {
        self.focus.focused_layout_node()
    }

    /// Moves the focus (the focus manager's `move_focus`); whether it moved.
    pub fn move_focus(&self, direction: FocusDirection) -> bool {
        self.focus.move_focus(direction)
    }

    /// Takes the focus from where it is (the focus manager's `clear_focus`); whether it was cleared.
    pub fn clear_focus(&self, force: bool) -> bool {
        self.focus.clear_focus(force)
    }

    pub(crate) fn owner(&self) -> &Rc<Owner> {
        &self.owner
    }

    /// The window info's keyboard modifiers (what key events carry).
    pub(crate) fn window_info_keyboard_modifiers(&self, modifiers: KeyModifiers) {
        if self.window_info.keyboard_modifiers.peek() != modifiers.0 {
            self.window_info.keyboard_modifiers.set(modifiers.0);
        }
    }

    /// A key puts the UI in keyboard mode (as Android leaves touch mode).
    pub(crate) fn request_keyboard_mode(&self) {
        if self.input_mode_manager.mode.peek() != InputMode::Keyboard {
            self.input_mode_manager.request_input_mode(InputMode::Keyboard);
        }
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

    /// The semantics of the tree (its window's and its popups').
    pub fn semantics_owner(&self) -> SemanticsOwner {
        SemanticsOwner { owner: Rc::downgrade(&self.owner) }
    }

    /// The nodes accessibility sees (C++'s `semanticsTree`): see [`SemanticsOwner::accessibility_tree`].
    pub fn accessibility_tree(&self) -> Vec<AccessibilityNode> {
        self.semantics_owner().accessibility_tree()
    }

    /// Performs an accessibility action on the semantics node with `id` (C++'s `performSemanticsAction`);
    /// whether it was done.
    pub fn perform_semantics_action(&self, id: u64, action: SemanticsAction) -> bool {
        self.semantics_owner().perform_semantics_action(id, action)
    }

    /// How many popups and dialogs are shown (C++'s `overlayCount`).
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
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

    /// Where the system's edge swipes are kept off (`system_gesture_exclusion`), px in the window, as of the last
    /// layout (C++'s `systemGestureExclusion`): the platform is told them as they change.
    pub fn system_gesture_exclusion(&self) -> Vec<kompoz_graphics::IntRect> {
        self.owner.gesture_exclusion()
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

    /// Turns the keep check on (C++'s KOMPOZ_KEEP_CHECK): after each frame the tree is drawn into a display list
    /// keeping the nodes that didn't change and into one drawing them all, which must record the same.
    pub fn set_keep_check(&self, on: bool) {
        self.keep_check.set(on);
        if !on {
            self.keep_check_lists.borrow_mut().take();
        }
    }

    /// The frames the keep check found recorded otherwise so far (taken).
    pub fn take_keep_check_mismatches(&self) -> Vec<String> {
        std::mem::take(&mut *self.keep_check_mismatches.borrow_mut())
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
