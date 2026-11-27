//! The host: a composition, its layout tree and what the platform feeds it (Compose's AndroidComposeView /
//! ComposeScene, C++'s KompozHost). It knows nothing of windows or GPUs: a platform calls `frame` and `draw`, sends
//! input and the environment, and gets the callbacks only it can serve.

use crate::back::BackEventCompat;
use crate::display_list::DisplayList;
use crate::draganddrop::{DragAndDropManager, DragAndDropTransferAction, DragAndDropTransferData};
use crate::focus::{FocusDirection, FocusManager, FocusOwner};
use crate::graphics_context::GraphicsContext;
use crate::idle::{IdleEntry, IdleScope, IdleWork, IdleWorkQueue, LocalIdleWorkQueue};
use crate::input::PointerEventType;
use crate::input::{
    IndirectPointerEvent, IndirectPointerEventPrimaryDirectionalMotionAxis, IndirectPointerEventType, InputState, Key,
    KeyEvent, KeyEventType, KeyModifiers, PointerButton, PointerButtons, PointerType, RotaryScrollEvent,
};
use crate::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable};
use crate::media_query::{LocalUiMediaScope, MediaInfo, UiMediaScope};
use crate::navigationevent::{BackRouter, WindowNavigation};
use crate::node::{LayoutNode, Owner};
use crate::platform::*;
use crate::semantics::{AccessibilityNode, SemanticsAction, SemanticsOwner};
use crate::strings::{LocalStrings, Strings};
use crate::text_input::{EditCommand, LocalTextInputService, TextInputService};
use crate::unit::Constraints;
use crate::window::{LocalWindowRegistry, Overlays, WindowRegistry};
use kompoz_coroutines::{CoroutineContext, Dispatcher, LocalDispatcher, LocalEffectCoroutineContext};
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_graphics::{Density, Dp, IntOffset, IntSize, LayoutDirection, Offset};
use kompoz_lifecycle::{
    CreationExtras, HasDefaultViewModelProviderFactory, Lifecycle, LifecycleOwner, LifecycleRegistry, LifecycleState,
    LocalLifecycleOwner, LocalSavedStateRegistryOwner, LocalViewModelStoreOwner, MutableCreationExtras,
    SAVED_STATE_REGISTRY_OWNER_KEY, SavedState, SavedStateHandleSupport, SavedStateRegistry,
    SavedStateRegistryController, SavedStateRegistryOwner, SavedStateViewModelFactory, VIEW_MODEL_STORE_OWNER_KEY,
    ViewModelProviderFactory, ViewModelStore, ViewModelStoreOwner,
};
use kompoz_navigationevent::{NavigationEventDispatcher, NavigationEventDispatcherOwner};
use kompoz_runtime::{
    Composition, Content, KompozitionLocalContext, KompozitionLocalProvider, KompozitionLocalProviderContext,
    MutableState, ProvideSaveableStateRegistry, RecompositionStats, SaveableStateRegistry, SavedValue, Snapshot,
    global_snapshot_changed, kompozable_block, mutable_confined_state_of_with_policy, mutable_state_of,
    never_equal_policy, restartable,
};
use kompoz_text::FontFamilyResolver;
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
    strings: Strings,
    dynamic_palette: Option<DynamicTonalPalette>,
    wallpaper_palette: Option<DynamicTonalPalette>,
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

/// The app's architecture components (CMP's `DefaultArchitectureComponentsOwner`, C++'s `AppLifecycle` and the
/// host's ViewModel store): its lifecycle (resumed from the start, moved by the platform as its window goes to the
/// background and back), its ViewModel store (cleared as it is destroyed: CMP's `setLifecycleState`), its saved
/// state registry (restored from the state it is made with, its ViewModels' handles enabled; saved into the host's
/// state) and its navigation event dispatcher (CMP's `navigationEventDispatcher`: the window's root dispatcher, what
/// back handlers register with, with the window's inputs — the keys', the platform's back —; back goes to it, or to
/// a focusable popup's or a dialog's on top).
struct HostArchitectureComponentsOwner {
    me: std::rc::Weak<HostArchitectureComponentsOwner>,
    registry: LifecycleRegistry,
    view_model_store: ViewModelStore,
    saved_state_controller: SavedStateRegistryController,
    navigation: Rc<WindowNavigation>,
}

impl LifecycleOwner for HostArchitectureComponentsOwner {
    fn lifecycle(&self) -> Lifecycle {
        self.registry.lifecycle()
    }
}

impl ViewModelStoreOwner for HostArchitectureComponentsOwner {
    fn view_model_store(&self) -> ViewModelStore {
        self.view_model_store.clone()
    }
    fn as_has_default_view_model_provider_factory(&self) -> Option<&dyn HasDefaultViewModelProviderFactory> {
        Some(self)
    }
}

impl HasDefaultViewModelProviderFactory for HostArchitectureComponentsOwner {
    fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        Rc::new(SavedStateViewModelFactory)
    }
    /// CMP's `defaultViewModelCreationExtras(this, this)`: the owner as the saved state registry owner and the
    /// ViewModel store owner.
    fn default_view_model_creation_extras(&self) -> CreationExtras {
        let me = self.me.upgrade().expect("kompoz: the host's owner is there");
        let extras = MutableCreationExtras::new(&CreationExtras::default());
        extras.set(&SAVED_STATE_REGISTRY_OWNER_KEY, me.clone() as Rc<dyn SavedStateRegistryOwner>);
        extras.set(&VIEW_MODEL_STORE_OWNER_KEY, me as Rc<dyn ViewModelStoreOwner>);
        extras.into()
    }
}

impl SavedStateRegistryOwner for HostArchitectureComponentsOwner {
    fn saved_state_registry(&self) -> SavedStateRegistry {
        self.saved_state_controller.saved_state_registry()
    }
}

impl NavigationEventDispatcherOwner for HostArchitectureComponentsOwner {
    fn navigation_event_dispatcher(&self) -> NavigationEventDispatcher {
        self.navigation.owner.dispatcher.clone()
    }
}

impl HostArchitectureComponentsOwner {
    /// Made from `saved_state` (what [`save_state`](Self::save_state) wrote), in `INITIALIZED` (CMP's constructor:
    /// the registry attached and restored), handles enabled (CMP's container's `enableSavedStateHandles()`); its
    /// navigation `navigation` (a new window's; the one before's when made again from a saved state: the window's
    /// dispatcher and inputs aren't saved state, what registered with them stays).
    fn new(saved_state: Option<&SavedState>, navigation: Rc<WindowNavigation>) -> Rc<HostArchitectureComponentsOwner> {
        let owner =
            Rc::new_cyclic(|me: &std::rc::Weak<HostArchitectureComponentsOwner>| HostArchitectureComponentsOwner {
                me: me.clone(),
                registry: LifecycleRegistry::new(me.clone()),
                view_model_store: ViewModelStore::new(),
                saved_state_controller: SavedStateRegistryController::create(me.clone()),
                navigation,
            });
        owner.saved_state_controller.perform_attach();
        owner.saved_state_controller.perform_restore(saved_state);
        owner.enable_saved_state_handles();
        owner
    }

    /// CMP's `saveState()`.
    fn save_state(&self) -> SavedState {
        let saved = SavedState::new();
        self.saved_state_controller.perform_save(&saved);
        saved
    }

    /// CMP's `setLifecycleState`: the ViewModels cleared as it is destroyed.
    fn set_lifecycle_state(&self, state: LifecycleState) {
        self.registry.set_current_state(state);
        if state == LifecycleState::Destroyed {
            self.view_model_store.clear();
        }
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
    /// Whether work without a frame was left when the last pass was done, before the keep check drew (what that
    /// drawing sets going — an animation's next frame — is a frame's: see `has_pending_work_before_drawing`).
    pending_before_drawing: Cell<bool>,
    /// The same, recomposition left out (see `has_pending_work_besides_recomposition_before_drawing`).
    pending_besides_recomposition_before_drawing: Cell<bool>,
    keep_check_lists: RefCell<Option<Box<(DisplayList, DisplayList)>>>,
    keep_check_mismatches: RefCell<Vec<String>>,
    /// Counted up by `invalidate`: the content's scope composes again when it changes.
    generation: Rc<Cell<u64>>,
    /// What text resolves its families with (LocalFontFamilyResolver).
    font_family_resolver: Rc<RefCell<Rc<dyn FontFamilyResolver>>>,
    /// The input package's state.
    pub input: RefCell<InputState>,
    /// The focus: the active target, moving and clearing it, key dispatch.
    pub focus: Rc<FocusOwner>,
    /// The popups' and dialogs' roots.
    pub overlays: Overlays,
    back_router: BackRouter,
    /// The focused editor's input sessions.
    text_input: TextInputService,
    /// What the app runs on (LocalUiMediaScope), and the device as the platform last told it.
    media: UiMediaScope,
    media_info: Cell<MediaInfo>,
    /// Drag and drop: the root node, the drag under way (within the app, or from outside).
    pub(crate) drag_and_drop: Rc<DragAndDropManager>,
    /// The links the app is opened with.
    deep_links: DeepLinks,
    /// Work waiting for the time between frames, in the order asked.
    idle_work: IdleWorkQueue,
    /// A trackpad's pinch under way (ScaleStart sent, ScaleEnd not yet).
    scaling: Cell<bool>,
    /// The app's lifecycle, ViewModel store, saved state registry and navigation event dispatcher owner
    /// (LocalLifecycleOwner, LocalViewModelStoreOwner, LocalSavedStateRegistryOwner and
    /// LocalNavigationEventDispatcherOwner at the root): a state, as `restore_state` makes it again.
    architecture: MutableState<Rc<HostArchitectureComponentsOwner>>,
    /// The windows and tray icons its composition asks for (LocalWindowRegistry at the root).
    windows: WindowRegistry,
    /// The app's own host (its dispatcher Kotlin's Dispatchers.Main, its waker the snapshots' global write
    /// observer), not a window's of its own ([`new_window`](KompozHost::new_window)).
    main: bool,
    /// A window's: the composition locals where its `Window` is composed, provided under the host's own.
    parent_locals: Option<MutableState<KompozitionLocalContext>>,
    /// A window's key handlers (Compose Desktop's window's `onPreviewKeyEvent` and `onKeyEvent`), around the focused
    /// path.
    window_keys: RefCell<Option<WindowKeyHandlers>>,
}

/// A window's key handlers: before the focused path sees a key, after it left it; true takes the key.
pub type WindowKeyHandlers = (Rc<dyn Fn(&KeyEvent) -> bool>, Rc<dyn Fn(&KeyEvent) -> bool>);

impl KompozHost {
    /// A host composing `content`, headless until given callbacks; its viewport 0 by 0 until set.
    pub fn new(content: impl Content + 'static) -> KompozHost {
        KompozHost::with_effect_coroutine_context(content, CoroutineContext::new())
    }

    /// The same, the composition's effects and the modifier nodes' coroutines running with
    /// `effect_coroutine_context` (Compose's `Recomposer(effectCoroutineContext)`: a test rule's has its
    /// `InfiniteAnimationPolicy`, a window's its `MotionDurationScale`).
    pub fn with_effect_coroutine_context(
        content: impl Content + 'static,
        effect_coroutine_context: CoroutineContext,
    ) -> KompozHost {
        KompozHost::build(content, effect_coroutine_context, None)
    }

    /// A host for a window of the app's own (a `Window`'s: C++'s per-window AppCore host): composing `content`
    /// with `parent_locals` (the locals where its `Window` is composed, read live) provided under the host's own
    /// root locals (its density, focus, lifecycle and the like are its own; the theme and the app's locals the
    /// caller's). Its dispatcher isn't Kotlin's Dispatchers.Main (the app's host's is), nor is its waker the
    /// snapshots' global write observer (all of an app's windows share one loop: the app's wakes it).
    pub fn new_window(
        content: impl Content + 'static,
        parent_locals: MutableState<KompozitionLocalContext>,
    ) -> KompozHost {
        KompozHost::build(content, CoroutineContext::new(), Some(parent_locals))
    }

    /// The same, the composition's effects and the modifier nodes' coroutines running with
    /// `effect_coroutine_context` (a window's `MotionDurationScale`, as its platform gives it).
    pub fn new_window_with_effect_coroutine_context(
        content: impl Content + 'static,
        parent_locals: MutableState<KompozitionLocalContext>,
        effect_coroutine_context: CoroutineContext,
    ) -> KompozHost {
        KompozHost::build(content, effect_coroutine_context, Some(parent_locals))
    }

    fn build(
        content: impl Content + 'static,
        effect_coroutine_context: CoroutineContext,
        parent_locals: Option<MutableState<KompozitionLocalContext>>,
    ) -> KompozHost {
        let dispatcher = Dispatcher::new();
        let main = parent_locals.is_none();
        // (what a lifecycle's or a ViewModel's scope launches on: Kotlin's Dispatchers.Main)
        if main {
            dispatcher.make_main();
        }
        let owner = Owner::new(dispatcher.clone());
        owner.set_effect_coroutine_context(effect_coroutine_context);
        let root = LayoutNode::new();
        root.set_measure_policy(Rc::new(RootMeasurePolicy { layer: Rc::new(|_| {}) }));
        let system_insets = SystemInsetsHolder::default();
        let platform = PlatformHandle::default();
        let drag_and_drop = DragAndDropManager::new(platform.clone());
        owner.set_drag_and_drop(&drag_and_drop);
        root.set_modifier(
            crate::focus::window_root_modifier()
                .then(crate::layout::window_insets_rulers(system_insets.clone()))
                .then_element(drag_and_drop.root_element()),
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
            strings: Strings::english(),
            dynamic_palette: None,
            wallpaper_palette: None,
        });
        let overlays = Overlays { owner: Rc::downgrade(&owner) };
        let autofill = crate::autofill::HostAutofillManager::new(platform.clone());
        owner.set_autofill(autofill.clone());
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
            pending_before_drawing: Cell::new(false),
            pending_besides_recomposition_before_drawing: Cell::new(false),
            keep_check_lists: RefCell::new(None),
            keep_check_mismatches: RefCell::new(Vec::new()),
            generation: Rc::new(Cell::new(0)),
            font_family_resolver: Rc::new(RefCell::new(system_resolver())),
            input: RefCell::new(InputState::default()),
            overlays,
            focus,
            back_router: BackRouter::default(),
            text_input: TextInputService::new(platform.clone()),
            media: UiMediaScope::new(),
            media_info: Cell::new(MediaInfo::default()),
            drag_and_drop,
            deep_links: DeepLinks::new(),
            idle_work: IdleWorkQueue::new(),
            scaling: Cell::new(false),
            architecture: mutable_confined_state_of_with_policy(
                HostArchitectureComponentsOwner::new(None, WindowNavigation::new()),
                never_equal_policy(),
            ),
            windows: WindowRegistry::default(),
            main,
            parent_locals,
            window_keys: RefCell::new(None),
        };
        // (resumed from the start, as C++'s and CMP's on a focused window; the platform moves it)
        host.architecture.peek().set_lifecycle_state(LifecycleState::Resumed);
        host.semantics_owner().add_listener(autofill);
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
        let effect_coroutine_context = self.owner.effect_coroutine_context();
        let focus_manager = FocusManager { owner: Some(Rc::downgrade(&self.focus)) };
        let font_family_resolver = self.font_family_resolver.clone();
        let text_toolbar: Rc<dyn TextToolbar> = Rc::new(PlatformTextToolbar::new(platform.clone()));
        let accessibility_manager: Rc<dyn AccessibilityManager> = Rc::new(DefaultAccessibilityManager);
        let text_input = self.text_input.clone();
        let media = self.media.clone();
        let deep_links = self.deep_links.clone();
        let idle_work = self.idle_work.clone();
        let autofill = self.owner.autofill().map(|a| crate::autofill::AutofillManager(Rc::downgrade(&a)));
        let architecture = self.architecture.clone();
        let windows = self.windows.clone();
        let parent_locals = self.parent_locals.clone();
        move || {
            let env = environment.get();
            let owner = architecture.get();
            let content = content.clone();
            let resolver = font_family_resolver.borrow().clone();
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
                LocalClipboardManager.provides(ClipboardManager(platform.clone())),
                LocalUriHandler.provides(UriHandler(platform.clone(), deep_links.clone())),
                LocalDeepLinks.provides(deep_links.clone()),
                LocalTextInputService.provides(text_input.clone()),
                LocalUiMediaScope.provides(media.clone()),
                LocalStrings.provides(env.strings.clone()),
                LocalDynamicTonalPalette.provides(env.dynamic_palette),
                LocalWallpaperTonalPalette.provides(env.wallpaper_palette),
                LocalTextToolbar.provides_unequal(text_toolbar.clone()),
                LocalAccessibilityManager.provides_unequal(Some(accessibility_manager.clone())),
                LocalSoftwareKeyboardController.provides(SoftwareKeyboardController::new(text_input.clone())),
                LocalPlatform.provides(platform.clone()),
                LocalFocusManager.provides(focus_manager.clone()),
                kompoz_navigationevent::LocalNavigationEventDispatcherOwner
                    .provides(owner.clone() as Rc<dyn NavigationEventDispatcherOwner>),
                LocalDispatcher.provides(Some(dispatcher.clone())),
                LocalEffectCoroutineContext.provides(effect_coroutine_context.clone()),
                LocalIdleWorkQueue.provides(Some(idle_work.clone())),
                LocalFontFamilyResolver.provides_unequal(resolver),
                crate::autofill::LocalAutofillManager.provides(autofill.clone()),
                LocalLifecycleOwner.provides(owner.clone() as Rc<dyn LifecycleOwner>),
                LocalViewModelStoreOwner.provides(owner.clone() as Rc<dyn ViewModelStoreOwner>),
                LocalSavedStateRegistryOwner.provides(owner as Rc<dyn SavedStateRegistryOwner>),
                LocalWindowRegistry.provides(Some(windows.clone())),
            ];
            let same_generation: fn(&u64, &u64) -> bool = |a, b| a == b;
            let g = generation.get();
            let saveable = saveable.clone();
            // (A window's: the caller's locals first, its own over them.)
            let parent = parent_locals.as_ref().map(MutableState::get).unwrap_or_default();
            kompozable_block!({
                KompozitionLocalProviderContext(&parent, move || {
                    KompozitionLocalProvider(&values, move || {
                        ProvideSaveableStateRegistry(&saveable, move || {
                            restartable(g, Some(same_generation), move |_| content())
                        })
                    })
                })
            })
        }
    }

    /// The app's lifecycle owner (C++'s `lifecycleOwner()`): what [`LocalLifecycleOwner`] gives outside a navigation
    /// entry; resumed from the start, the platform moves it ([`set_lifecycle_state`](Self::set_lifecycle_state)).
    pub fn lifecycle_owner(&self) -> Rc<dyn LifecycleOwner> {
        self.architecture.peek()
    }

    /// The app's ViewModel store owner (CMP's `DefaultArchitectureComponentsOwner`, C++'s host `viewModelStore()`):
    /// what [`LocalViewModelStoreOwner`] gives outside a navigation entry; its ViewModels are cleared as the app is
    /// destroyed.
    pub fn view_model_store_owner(&self) -> Rc<dyn ViewModelStoreOwner> {
        self.architecture.peek()
    }

    /// The app's saved state registry owner: what [`LocalSavedStateRegistryOwner`] gives outside a navigation entry;
    /// its registry is written out with the host's state ([`save_state`](Self::save_state)).
    pub fn saved_state_registry_owner(&self) -> Rc<dyn SavedStateRegistryOwner> {
        self.architecture.peek()
    }

    /// Moves the app's lifecycle to `state`, its observers told at once (C++'s `setLifecycleState`, CMP's
    /// `setLifecycleState`): the platform's, as its window is focused (`RESUMED`), shown without the focus
    /// (`STARTED`), hidden or minimized (`CREATED`); a test rule's.
    ///
    /// # Panics
    /// Out of `DESTROYED` (as Compose's LifecycleRegistry).
    pub fn set_lifecycle_state(&self, state: LifecycleState) {
        self.architecture.peek().set_lifecycle_state(state);
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
        self.windows.set_wake(waker.clone());
        if let Some(w) = waker {
            let notify = w.clone();
            self.dispatcher.set_notify(move || notify());
            if self.main {
                kompoz_runtime::set_global_write_observer(move || w());
            }
        }
        self.platform.set(callbacks);
    }

    /// The windows and tray icons its composition asks for (`Window`, `Tray`), as the platform's app core reads
    /// them.
    pub fn window_registry(&self) -> &WindowRegistry {
        &self.windows
    }

    /// A window's key handlers (its `Window`'s `onPreviewKeyEvent` and `onKeyEvent`), or none.
    pub fn set_window_key_handlers(&self, handlers: Option<WindowKeyHandlers>) {
        *self.window_keys.borrow_mut() = handlers;
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
        self.update_media();
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
        set_current_system_density(density.density);
        self.update_environment(|e| {
            e.density = density;
            e.view_configuration = ViewConfiguration::for_density(density);
        });
        self.update_media();
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
        set_current_system_theme(if dark { SystemTheme::Dark } else { SystemTheme::Light });
        self.update_environment(|e| e.theme = if dark { SystemTheme::Dark } else { SystemTheme::Light });
    }

    /// What each kind of system UI covers (LocalSystemInsets, the window insets): the kinds that changed are set
    /// in their states, so what measures by them measures again (an IME animation composes nothing).
    pub fn set_system_insets(&self, insets: SystemInsets) {
        self.system_insets.set(&insets);
        self.update_media();
    }

    /// The state to keep while the platform ends the app (C++'s `saveState`, CMP's container's `saveState()`), as
    /// bytes: a map of the composition's saveable values under `"composition"` (`remember_saveable` outside any
    /// holder, what nodes registered: a focus restorer's child) and the app's saved state registry's state under
    /// `"savedState"` (CMP's `DefaultArchitectureComponentsOwner.saveState()`: its providers' states, the app's
    /// ViewModels' handles among them, and what was restored and not consumed yet).
    pub fn save_state(&self) -> Vec<u8> {
        let saved_state = self.architecture.peek().save_state();
        SavedValue::map([("composition", self.saveable.save()), ("savedState", saved_state.to_saved_value())]).encode()
    }

    /// Gives back a state `save_state` wrote (C++'s `restoreState`): call it before the first frame, and what
    /// composes finds its values again — the composition's saveable values, and the app's owners made again from
    /// the saved state registry's state (as CMP makes its container's with the state it is given; the ones there
    /// before destroyed, the new ones at their lifecycle state). Bytes that aren't such a state are ignored.
    pub fn restore_state(&self, bytes: &[u8]) {
        let Some(saved) = SavedValue::decode(bytes) else { return };
        let composition = saved.get("composition");
        if composition.as_map().is_some() {
            self.saveable.restore(composition);
        }
        let saved_state = saved.get("savedState");
        if saved_state.as_map().is_some() {
            let old = self.architecture.peek();
            let state = old.lifecycle().current_state();
            let restored = HostArchitectureComponentsOwner::new(
                Some(&SavedState::from_saved_value(saved_state)),
                old.navigation.clone(),
            );
            if state != LifecycleState::Destroyed {
                restored.set_lifecycle_state(state);
                old.set_lifecycle_state(LifecycleState::Destroyed);
            }
            self.architecture.set(restored);
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

    /// What text resolves its font families with, and shapes and measures through (LocalFontFamilyResolver):
    /// the system's fonts' ([`system_resolver`]) until a platform or a test installs another (a collection of
    /// bundled fonts; the test rule installs the fake one). Everything composes and measures again with it at the
    /// next frame.
    pub fn set_font_family_resolver(&self, resolver: Rc<dyn FontFamilyResolver>) {
        *self.font_family_resolver.borrow_mut() = resolver;
        self.invalidate();
    }

    /// What text resolves its font families with now.
    pub fn font_family_resolver(&self) -> Rc<dyn FontFamilyResolver> {
        self.font_family_resolver.borrow().clone()
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

    /// Where the window's top left is on the screen, px (what `local_to_screen` and `position_on_screen` add to a
    /// point of the window; Compose desktop's window location). A platform that can't tell (Wayland) leaves it at
    /// zero.
    pub fn set_window_position_on_screen(&self, position: Offset) {
        self.owner.set_window_position_on_screen(position);
        self.owner.dispatch_root_positioned();
    }

    // ---- Frames ----

    /// A frame at `time_nanos` (C++'s `frame`, Compose's): posted tasks, then the dispatcher (due timers, then the
    /// frame waiters' callbacks), then up to four passes of {applied changes told to their readers → recompose →
    /// measure and layout → global position callbacks} while a pass wrote state, then the coroutines the frame
    /// resumed and those it launched. Whether a redraw is due.
    pub fn frame(&self, time_nanos: i64) -> bool {
        self.pass(time_nanos, true)
    }

    /// What is due at `time_nanos` run without a frame (a test clock between frames, Compose's test scheduler running
    /// the tasks due without a frame callback): posted tasks, the coroutine timers due and what they wake, then the
    /// applied changes told to their readers, composition, measure and layout and the global position callbacks; the
    /// coroutines awaiting a frame and the scopes asking for one wait for the next frame. Whether a redraw is due.
    pub fn run_due(&self, time_nanos: i64) -> bool {
        self.pass(time_nanos, false)
    }

    fn pass(&self, time_nanos: i64, frame: bool) -> bool {
        let tasks = std::mem::take(&mut *self.handle.posted.lock());
        for task in tasks {
            task();
        }
        if frame {
            self.frame_time.set(time_nanos);
            // (the frame callbacks run now; the coroutines go on after the layout, at `run_frame_end`)
            self.dispatcher.send_frame(time_nanos);
        } else {
            self.dispatcher.run_timers(time_nanos);
        }
        // (a drag a coroutine started — a long press's — takes its pointer)
        crate::input::drag_and_drop_takes_pointer(self);
        {
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
                if pass == 0 && frame {
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
        }
        // (the coroutines continuing after the frame — Compose's frame continuations, dispatched after its
        // composition and layout — and those the composition launched (LaunchedEffect's, dispatched) see where it
        // laid things out; what they write is the next frame's)
        if frame {
            self.dispatcher.run_frame_end();
            // (WP5: sheets) What they wrote reaches its readers before the frame is drawn: a draw reading it is
            // drawn again with it — the frame's drawing reads it either way, as Compose's draw after the frame's
            // continuations does —; what it composes or lays out again is the next frame's.
            Snapshot::send_apply_notifications();
            self.owner.snapshot_observer().process_changes();
        }
        self.owner.dispatch_semantics_changes();
        if self.owner.take_gesture_exclusion_changed() {
            let rects = self.owner.gesture_exclusion();
            self.platform.get().system_gesture_exclusion(&rects);
        }
        let mut redraw = self.owner.take_needs_draw();
        self.pending_before_drawing.set(self.has_pending_work_without_frame());
        self.pending_besides_recomposition_before_drawing.set(self.has_pending_work_besides_recomposition());
        if self.keep_check.get() {
            self.check_kept();
            self.owner.take_needs_draw(); // (drawn for the check: no frame asked for by it)
            // (what that drawing wrote — an animation's redraw signal, Compose's `invalidateDraw` from a draw — is a
            // frame's to draw, as the platform's drawing's would be: applied now, its readers told, not left as a
            // write that work without a frame — `run_current` — would wait on)
            if global_snapshot_changed() {
                Snapshot::send_apply_notifications();
                self.owner.snapshot_observer().process_changes();
                redraw |= self.owner.take_needs_draw();
            }
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
        // (a drag within the app: its decoration under the pointer, over all)
        self.drag_and_drop.draw(canvas, self.viewport.get().to_size(), self.density());
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

    /// Whether there is work a pass without a frame ([`run_due`](Self::run_due)) does: what `has_pending_work`
    /// says, the coroutines and scopes waiting for a frame left out.
    pub fn has_pending_work_without_frame(&self) -> bool {
        let composition = self.composition.borrow();
        let composing = composition.as_ref().is_some_and(|c| c.has_invalidations());
        composing || self.has_pending_work_besides_recomposition()
    }

    /// What [`has_pending_work_without_frame`](Self::has_pending_work_without_frame) says, scopes waiting to be
    /// recomposed left out: changes not yet told to their readers, layouts waiting, coroutines woken, tasks posted.
    /// (Compose recomposes at frames only: a test's idling resource counts recomposition as work a frame does, and
    /// what isn't laid out as work that needs none.)
    pub fn has_pending_work_besides_recomposition(&self) -> bool {
        global_snapshot_changed()
            || self.owner.snapshot_observer().has_pending()
            || self.owner.has_pending_measure_or_layout()
            || self.dispatcher.has_ready()
            || self.focus.has_pending_invalidation()
            || !self.handle.posted.lock().is_empty()
            || self.handle.invalid.load(Ordering::Acquire)
    }

    /// [`has_pending_work_without_frame`](Self::has_pending_work_without_frame) as the last pass left it before the
    /// keep check drew: Compose's `runCurrent` runs what is due without a frame, and a drawing is a frame's — what a
    /// draw invalidates (a held overscroll stretch asking to be drawn again, say) waits for the next one.
    pub fn has_pending_work_before_drawing(&self) -> bool {
        self.pending_before_drawing.get()
    }

    /// [`has_pending_work_besides_recomposition`](Self::has_pending_work_besides_recomposition) as the last pass left
    /// it before the keep check drew.
    pub fn has_pending_work_besides_recomposition_before_drawing(&self) -> bool {
        self.pending_besides_recomposition_before_drawing.get()
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

    /// The positions (px) pointer `pointer_id` passed through since its last event, oldest first, each at its time
    /// (ns, on the frame clock), as the platform batched them (Android's historical samples): its next event
    /// carries them (Compose's `PointerInputChange.historical`, what velocity trackers read).
    pub fn pointer_historical(&self, pointer_id: i64, samples: &[(Offset, i64)]) {
        let historical = samples
            .iter()
            .map(|&(position, time)| crate::input::HistoricalChange::new(self.event_time(time) / 1_000_000, position))
            .collect();
        self.input.borrow().set_historical(pointer_id, historical);
    }

    pub fn pointer_up(&self, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
        let t = self.event_time(time_nanos);
        crate::input::pointer_up(self, position, t, pointer_id)
    }

    /// A mouse button pressed or let go with the mouse at `position`. The back and forward buttons also make their
    /// keys (Key::Back, Key::Forward) after the pointer event, as Android's input reader synthesizes them: the Back
    /// key goes back when nothing takes it. Whether the pointer event (or that key) was taken.
    pub fn mouse_button(&self, position: Offset, time_nanos: i64, button: PointerButton, pressed: bool) -> bool {
        let t = self.event_time(time_nanos);
        let consumed = crate::input::mouse_button(self, position, t, button, pressed, None);
        let key = match button {
            PointerButton::Back => Key::Back,
            PointerButton::Forward => Key::Forward,
            _ => return consumed,
        };
        let r#type = if pressed { KeyEventType::KeyDown } else { KeyEventType::KeyUp };
        let modifiers = self.input.borrow().keyboard_modifiers();
        self.send_key_event(KeyEvent::new(key, r#type, modifiers)) || consumed
    }

    pub fn pointer_cancel(&self) {
        crate::input::pointer_cancel(self);
    }

    pub fn pointer_exit(&self) {
        crate::input::pointer_exit(self);
    }

    /// A stylus (`PointerType::Stylus`, or `Eraser`: its other end) at `position` (px), at `time_nanos`: near the
    /// window (hovering), or touching it while `down` with `pressure` (0 to 1), its barrel `buttons` held (Android's
    /// stylus: the first button primary, the second secondary). One stream from its coming near until
    /// [`stylus_exit`](Self::stylus_exit): its touches are Presses and Releases, the rest Moves (hover enters and
    /// exits worked out from them). Whether a node took it.
    #[allow(clippy::too_many_arguments)]
    pub fn stylus(
        &self,
        position: Offset,
        time_nanos: i64,
        pointer_id: i64,
        pointer_type: PointerType,
        down: bool,
        pressure: f32,
        buttons: PointerButtons,
    ) -> bool {
        let t = self.event_time(time_nanos);
        // (a touching stylus puts the UI in touch mode, as on Android)
        if down && self.input_mode_manager.mode.peek() != InputMode::Touch {
            self.input_mode_manager.request_input_mode(InputMode::Touch);
        }
        crate::input::stylus(self, position, t, pointer_id, pointer_type, down, pressure, buttons)
    }

    /// The stylus `pointer_id` went away (out of its tablet's range, or off the window): what it hovered is exited.
    pub fn stylus_exit(&self, pointer_id: i64) {
        crate::input::stylus_exit(self, pointer_id);
    }

    /// A wheel or touchpad scrolled by `delta` notches (down and right positive; a touchpad's or a free-spinning
    /// wheel's parts of notches as they come) with the mouse at `position`: the change's `scroll_delta`, in
    /// Compose's units (foundation's scrollables make a notch 64 dp, Android's scroll factor); whether a node took
    /// it.
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
        // (A window's own handlers around it, as Compose Desktop's ComposeWindow has them.)
        let handlers = self.window_keys.borrow().clone();
        if let Some((preview, _)) = &handlers
            && preview(&event)
        {
            return true;
        }
        if crate::input::key_event(self, event) {
            return true;
        }
        handlers.is_some_and(|(_, after)| after(&event))
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

    /// The app's root navigation event dispatcher (CMP's `DefaultArchitectureComponentsOwner`'s): what
    /// [`LocalNavigationEventDispatcherOwner`] gives at the root, and what the platform's back is sent to.
    ///
    /// [`LocalNavigationEventDispatcherOwner`]: kompoz_navigationevent::LocalNavigationEventDispatcherOwner
    pub fn navigation_event_dispatcher(&self) -> NavigationEventDispatcher {
        self.architecture.peek().navigation_event_dispatcher()
    }

    /// The navigation event dispatcher owner at the root (CMP's `DefaultArchitectureComponentsOwner`, its
    /// `navigationEventDispatcherOwner`): the app's architecture components owner.
    pub fn navigation_event_dispatcher_owner(&self) -> Rc<dyn NavigationEventDispatcherOwner> {
        self.architecture.peek()
    }

    /// The host's window's navigation (its architecture components owner's).
    fn navigation(&self) -> Rc<WindowNavigation> {
        self.architecture.peek().navigation.clone()
    }

    /// The navigation of the window on top: a focusable popup's or a dialog's, else the host's.
    fn top_navigation(&self) -> Rc<WindowNavigation> {
        self.owner
            .top_focusable_popup()
            .and_then(|p| p.popup_spec())
            .and_then(|s| s.navigation.clone())
            .unwrap_or_else(|| self.navigation())
    }

    /// A key event nothing took: Escape or the Back key goes back in the window on top (CMP's
    /// `BackNavigationEventInput`) if a handler there takes back. Whether it did.
    pub(crate) fn back_key(&self, event: &KeyEvent) -> bool {
        self.top_navigation().keys.on_key_event(event)
    }

    /// A back gesture started (Android's `onBackStarted`): it goes to the handler that takes back now (the
    /// handlers composed last, a focusable popup's or a dialog's before the content under it), through its steps
    /// to its end or cancellation. Nothing if nothing takes back (the platform's own gesture). A gesture never
    /// ended is called off first.
    pub fn back_started(&self, event: BackEventCompat) {
        self.back_router.started(&self.top_navigation(), event);
    }

    /// A step of the back gesture (Android's `onBackProgressed`).
    pub fn back_progressed(&self, event: BackEventCompat) {
        self.back_router.progressed(event);
    }

    /// The back gesture was called off (Android's `onBackCancelled`).
    pub fn back_cancelled(&self) {
        self.back_router.cancelled();
    }

    /// Back invoked (Android's `onBackInvoked`: a gesture's end, or the back button): back completed. False if
    /// nothing takes back (the platform's default: leaving the app).
    pub fn back_invoked(&self) -> bool {
        self.back_router.invoked(&self.top_navigation())
    }

    /// Whether back would do something (an enabled handler: a focusable popup's or a dialog's, or the app's): the
    /// platform lets the app handle back (Android's predictive back) only then.
    pub fn wants_back(&self) -> bool {
        self.top_navigation().platform.is_registered()
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

    /// The window info's keyboard modifiers (what key events carry).
    /// Where the last pointer event was (none: the mouse left the window).
    pub(crate) fn note_pointer_position(&self, position: Option<Offset>) {
        self.window_info.last_known_pointer_position.set(position);
    }

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
    /// The focused field's text layout is its input session's (where its text is exactly: its decoration around it),
    /// where that one says where each character the node shows is.
    pub fn accessibility_tree(&self) -> Vec<AccessibilityNode> {
        let mut tree = self.semantics_owner().accessibility_tree();
        if let Some(layout) = self.text_input.session_text_layout()
            && let Some(field) = tree.iter_mut().find(|n| n.focused && n.editable)
            && let Some(shown) = layout.with_text(&field.value)
        {
            field.text_layout = Some(std::sync::Arc::new(shown));
        }
        tree
    }

    /// Performs an accessibility action on the semantics node with `id` (C++'s `performSemanticsAction`);
    /// whether it was done.
    pub fn perform_semantics_action(&self, id: u64, action: SemanticsAction) -> bool {
        self.semantics_owner().perform_semantics_action(id, action)
    }

    /// Fills the node with semantics `id` with what an autofill service gave (Compose's `performAutofill`: its
    /// `OnFillData` action); whether it took it.
    pub fn perform_autofill(&self, id: u64, data: crate::semantics::FillableData) -> bool {
        let Some(node) = self.semantics_owner().find(id, false) else { return false };
        let action = node.config().get(crate::semantics::SemanticsActions::OnFillData);
        match action.and_then(|a| a.action) {
            Some(fill) => fill(data),
            None => false,
        }
    }

    /// How many popups and dialogs are shown (C++'s `overlayCount`).
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    /// The dispatcher the host runs coroutines on.
    /// What the composition's effects and the modifier nodes' coroutines run with (Compose's
    /// `Recomposer.effectCoroutineContext`).
    pub fn effect_coroutine_context(&self) -> CoroutineContext {
        self.owner.effect_coroutine_context()
    }

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

// ---- What the platform tells and asks besides (C++'s setters and entry points) ----

impl KompozHost {
    /// The media scope from the viewport, the density, the keyboard and the device.
    fn update_media(&self) {
        let density = self.environment.peek().density.density.max(f32::MIN_POSITIVE);
        let size = self.viewport.get();
        let ime_visible = kompoz_runtime::without_read_observation(|| self.system_insets.is_visible(InsetsKind::Ime));
        let dp = |px: i32| Dp(px as f32 / density);
        self.media.update(dp(size.width), dp(size.height), ime_visible, self.media_info.get());
    }

    /// The library's texts in the system's language (C++'s `setStrings`; LocalStrings at the root).
    pub fn set_strings(&self, strings: Strings) {
        self.update_environment(|e| e.strings = strings);
    }

    /// The library's texts as last set.
    pub fn strings(&self) -> Strings {
        self.environment.peek().strings
    }

    /// The system's palettes (C++'s `setDynamicTonalPalette`; LocalDynamicTonalPalette): Android 12+'s, or a
    /// desktop's made from its accent color; none where it has none.
    pub fn set_dynamic_tonal_palette(&self, palette: Option<DynamicTonalPalette>) {
        self.update_environment(|e| e.dynamic_palette = palette);
    }

    /// Palettes made from the wallpaper's colors (C++'s `setWallpaperTonalPalette`; LocalWallpaperTonalPalette).
    pub fn set_wallpaper_tonal_palette(&self, palette: Option<DynamicTonalPalette>) {
        self.update_environment(|e| e.wallpaper_palette = palette);
    }

    /// The device as the media scope tells it (C++'s `setMediaInfo`): the pointer, the keyboard, a microphone and a
    /// camera, the viewing distance, the posture (the window's size and the on-screen keyboard come from the
    /// viewport and the insets).
    pub fn set_media_info(&self, info: MediaInfo) {
        self.media_info.set(info);
        self.update_media();
    }

    pub fn media_info(&self) -> MediaInfo {
        self.media_info.get()
    }

    /// The keys held down (Shift, Ctrl, Alt, Meta), as the platform last reported them (C++'s
    /// `setKeyboardModifiers`): pointer events carry them, and the window info.
    pub fn set_keyboard_modifiers(&self, modifiers: KeyModifiers) {
        self.input.borrow().set_keyboard_modifiers(modifiers);
        self.window_info_keyboard_modifiers(modifiers);
    }

    pub fn keyboard_modifiers(&self) -> KeyModifiers {
        self.input.borrow().keyboard_modifiers()
    }

    /// The next frame draws (C++'s `requestRedraw`): what the window shows went (exposed, shown again).
    pub fn request_redraw(&self) {
        self.owner.request_draw();
    }

    /// Of the next frame's work, what it has to show (C++'s `wantsFrame`): states changed, scopes to compose,
    /// layouts to measure, a redraw, coroutines waiting for a frame (not a timer to wait for, work on another thread
    /// to come back, a task posted).
    pub fn wants_frame(&self) -> bool {
        let composition = self.composition.borrow();
        let composing = composition.as_ref().is_some_and(|c| c.has_invalidations() || c.has_frame_scopes());
        composing
            || global_snapshot_changed()
            || self.owner.snapshot_observer().has_pending()
            || self.owner.has_pending_measure_or_layout()
            || self.owner.needs_draw()
            || self.dispatcher.has_frame_waiters()
            || self.handle.invalid.load(Ordering::Acquire)
    }

    /// Work for the time between frames (C++'s `requestIdleWork`, as Compose's prefetch: a lazy list's next item
    /// composed before it scrolls in), one request for each `owner` (a later one replaces it). `idle` does it when
    /// it fits: `estimate_nanos` is what such work took before (0: never timed); `start` makes its change (false:
    /// nothing to do any more), which is composed then (laid out and drawn by the next frame); `took` hears how long
    /// it took.
    pub fn request_idle_work(
        &self,
        owner: usize,
        estimate_nanos: i64,
        start: impl FnOnce() -> bool + 'static,
        took: impl FnOnce(i64) + 'static,
    ) {
        let work = IdleWork::Once { estimate_nanos, start: Box::new(start), took: Box::new(took) };
        self.idle_work.put(IdleEntry { owner, work });
    }

    /// The work waiting for the time between frames (LocalIdleWorkQueue): tasks run while there is time, as
    /// Compose's prefetch requests are.
    pub fn idle_work_queue(&self) -> IdleWorkQueue {
        self.idle_work.clone()
    }

    pub fn has_idle_work(&self) -> bool {
        self.idle_work.has_work()
    }

    /// Does the idle work that fits in `budget_nanos` (the platform's, until the next frame is due), in order: one
    /// whose estimate is more than the time left waits for the next pause (C++'s `idle`); a task runs while there
    /// is time, and one with more to do runs first in the next pause (Compose's prefetch scheduler). What each
    /// changed is composed then (laid out and drawn by the next frame).
    pub fn idle(&self, budget_nanos: i64) {
        let deadline =
            kompoz_runtime::clock::Instant::now() + std::time::Duration::from_nanos(budget_nanos.max(0) as u64);
        let scope = IdleScope::new(deadline);
        loop {
            let left = scope.available_time_nanos();
            let entry = self.idle_work.take_first(|w| match w {
                IdleWork::Once { estimate_nanos, .. } => left > 0 && *estimate_nanos <= left,
                IdleWork::Task(_) => left > 0,
            });
            let Some(entry) = entry else { return };
            let owner = entry.owner;
            match entry.work {
                IdleWork::Once { start, took, .. } => {
                    let started = kompoz_runtime::clock::Instant::now();
                    if !start() {
                        continue;
                    }
                    self.compose_idle_changes();
                    took(started.elapsed().as_nanos() as i64);
                }
                IdleWork::Task(mut task) => {
                    let more = task(&scope);
                    self.compose_idle_changes();
                    if more {
                        self.idle_work.put_back_first(IdleEntry { owner, work: IdleWork::Task(task) });
                        return;
                    }
                }
            }
        }
    }

    /// What idle work changed composed now, as a frame would (laid out by the next frame).
    fn compose_idle_changes(&self) {
        Snapshot::send_apply_notifications();
        self.owner.snapshot_observer().process_changes();
        let recomposed = self.composition.borrow().as_ref().is_some_and(|c| c.recompose(self.frame_time.get()));
        if recomposed {
            self.owner.run_end_apply_listeners();
            self.owner.snapshot_observer().process_changes();
        }
    }

    /// A link the app was opened with, or that was sent to it while it runs (C++'s `openDeepLink`): the first
    /// navigation host with a destination for it goes there.
    pub fn open_deep_link(&self, link: &str) {
        self.deep_links.open(link);
    }

    /// The links the app is opened with (LocalDeepLinks).
    pub fn deep_links(&self) -> DeepLinks {
        self.deep_links.clone()
    }

    // ---- Text input ----

    /// The focused editor's input sessions (LocalTextInputService).
    pub fn text_input_service(&self) -> TextInputService {
        self.text_input.clone()
    }

    /// Text typed (a keyboard's, or an input method's commit), put in place of the composition or the selection of
    /// the editor whose session is open (C++'s `textInput`, Compose's CommitTextCommand); false if none is.
    pub fn text_input(&self, text: &str) -> bool {
        self.send_edit_commands(&[EditCommand::CommitText { text: text.to_string(), new_cursor_position: 1 }])
    }

    /// An input method's composing text (its preedit; empty: none), the caret `new_cursor_position` from it as
    /// [`EditCommand::SetComposingText`] has it; false if no session is open.
    pub fn set_composing_text(&self, text: &str, new_cursor_position: i32) -> bool {
        self.send_edit_commands(&[EditCommand::SetComposingText { text: text.to_string(), new_cursor_position }])
    }

    /// An input method's composing text with its caret at byte `start` of it, or bytes `start` to `end` of it
    /// selected (Compose's SetComposingTextCommand and a SetSelectionCommand inside the composition); false if no
    /// session is open.
    pub fn set_composing_text_with_selection(&self, text: &str, start: usize, end: usize) -> bool {
        self.text_input.set_composing_text_with_selection(text, start, end)
    }

    /// What an input method did, to the open session's editor (Compose's `onEditCommand`); false if none is open.
    pub fn send_edit_commands(&self, commands: &[EditCommand]) -> bool {
        self.text_input.send(commands)
    }

    /// Content an input method commits (an image or a file a keyboard sends, with its link URI: Android's
    /// `InputConnection.commitContent`; Linux's input methods send none) to the open session's editor: whether it
    /// took it (a state-based field under a `contentReceiver`).
    pub fn commit_content(&self, content: ClipEntry, link_uri: Option<String>) -> bool {
        self.text_input.commit_content(content, link_uri)
    }

    /// The input method's action key (Done, Next, Search, …: C++'s `imeAction`) to the open session's editor;
    /// false if none is open.
    pub fn ime_action(&self, action: ImeAction) -> bool {
        self.text_input.perform_ime_action(action)
    }

    // ---- Zoom ----

    /// A trackpad's pinch with the mouse at `position` (px): its start (`ScaleStart`), each step (`ScaleChange`, by
    /// `factor` and `rotation` degrees clockwise since the last) and end (`ScaleEnd`), as Compose's trackpad scale
    /// events; whether a node consumed it.
    pub fn scale(&self, position: Offset, r#type: PointerEventType, factor: f32, rotation: f32) -> bool {
        match r#type {
            PointerEventType::ScaleStart => self.scaling.set(true),
            PointerEventType::ScaleEnd => self.scaling.set(false),
            _ => {}
        }
        crate::input::scale(self, position, r#type, factor, rotation)
    }

    /// A trackpad's two-finger pan with the mouse at `position` (px): its start (`PanStart`), each move
    /// (`PanMove`) and end (`PanEnd`), as Compose's trackpad pan events (Android makes them of a touchpad's
    /// swipe; foundation's scrollables scroll with them); `offset` is the pan offset since the last (px), as
    /// Compose's `panOffset` (Android's AXIS_GESTURE_SCROLL_X/Y_DISTANCE): the scroll the fingers make, the opposite
    /// of their movement (fingers up: positive y). Whether a node consumed it.
    pub fn pan(&self, position: Offset, r#type: PointerEventType, offset: Offset) -> bool {
        crate::input::pan(self, position, r#type, offset)
    }

    /// Zooms what is under `position` (px) by `factor` and turns it by `rotation` degrees clockwise (C++'s `zoom`:
    /// Ctrl+wheel, Ctrl+Shift+wheel, a trackpad's pinch): a scale step, in a pinch of its own unless one is under
    /// way. Whether something took it.
    pub fn zoom(&self, position: Offset, factor: f32, rotation: f32) -> bool {
        if factor.is_nan() || factor <= 0.0 || (factor == 1.0 && rotation == 0.0) {
            return false;
        }
        if self.scaling.get() {
            return self.scale(position, PointerEventType::ScaleChange, factor, rotation);
        }
        self.scale(position, PointerEventType::ScaleStart, 1.0, 0.0);
        let taken = self.scale(position, PointerEventType::ScaleChange, factor, rotation);
        self.scale(position, PointerEventType::ScaleEnd, 1.0, 0.0);
        taken
    }

    // ---- Drag and drop ----

    /// Whether a drag within the app is under way (a source's: the host has the pointer it follows).
    pub fn is_dragging_and_dropping(&self) -> bool {
        self.drag_and_drop.is_dragging()
    }

    /// A drag from outside the app (files from a file manager, text from a browser) came over the window, carrying
    /// one of `mime_types` (C++'s `externalDragEntered`): the drag and drop targets that want it are told it
    /// started. Whether any wants it. (A drag of the app's own the platform took as it left the window, coming
    /// back: with what it carries.)
    pub fn external_drag_entered(&self, mime_types: Vec<String>) -> bool {
        self.drag_and_drop.external_entered(mime_types)
    }

    /// It moved over the window, to `position` (px): the targets it enters, moves over and leaves are told (C++'s
    /// `externalDragMoved`). Whether a target that wants it is under it now (what a drop there would be offered to).
    pub fn external_drag_moved(&self, position: Offset) -> bool {
        self.drag_and_drop.external_moved(position)
    }

    /// It was dropped at `position` (px) with `data` (C++'s `externalDrop`): the target under it takes it, then
    /// every target is told the drag ended. Whether it was taken.
    pub fn external_drop(&self, data: DragAndDropTransferData, position: Offset) -> bool {
        self.drag_and_drop.external_drop(data, position)
    }

    /// It left the window, or ended without a drop (C++'s `externalDragLeft`): the target it was over is told it
    /// left, every one that it ended.
    pub fn external_drag_left(&self) {
        self.drag_and_drop.external_left();
    }

    /// The platform's drag of the app's own (a drag within the app it took as it left the window:
    /// [`PlatformCallbacks::start_drag_and_drop`]) ended, dropped with `action` or not taken (none): its source is
    /// told (Compose desktop's `onTransferCompleted`).
    pub fn platform_drag_ended(&self, action: Option<DragAndDropTransferAction>) {
        self.drag_and_drop.platform_drag_ended(action);
    }
}

impl Drop for KompozHost {
    /// The app's lifecycle destroyed (paused, stopped, destroyed: told while what observes it is still here, as
    /// CMP's container disposed of), the composition disposed of (what it holds forgotten), then the tree detached.
    fn drop(&mut self) {
        let owner = self.architecture.peek();
        if owner.lifecycle().current_state() != LifecycleState::Destroyed {
            owner.set_lifecycle_state(LifecycleState::Destroyed);
        }
        self.composition.borrow_mut().take();
        if self.root.is_attached() {
            self.root.detach();
        }
        // (the handlers left with the composition; the inputs go with the root dispatcher)
        self.navigation().dispose();
    }
}
