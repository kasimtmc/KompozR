//! What the UI asks of the platform and what it tells the UI (Compose's `androidx.compose.ui.platform`): the
//! platform's callbacks (C++'s `KompozHost::Callbacks`, each doing nothing by default, so a headless host works),
//! the view configuration, what the system UI covers (kept in states the host updates) and moving the keyboard by
//! hand, the text toolbar (Compose's TextToolbar, the host's handing it to the platform), the ui composition locals
//! the host provides at the root (the font family resolver text resolves its families with among them), and the
//! modifiers whose effect is the window's
//! (`keep_screen_on`, `sensitive_content`, `preferred_frame_rate`, `system_gesture_exclusion`).

use crate::graphics_context::GraphicsContext;
use crate::layout::LayoutCoordinates;
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_graphics::{Density, ImageBitmap, IntRect, IntSize, LayoutDirection, Path, Rect, Size};
use kompoz_runtime::{KompozitionLocal, MutableState, SnapshotMutationPolicy, mutable_state_of};
use kompoz_text::{FakeFontFamilyResolver, FontFamilyResolver, FontFamilyResolverImpl};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------------------------------------------
// Values the callbacks carry

/// The text, selection and composition of the focused text field, as an input method sees them (C++'s
/// TextEditState); indices are byte offsets in `text`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TextEditState {
    pub text: String,
    pub selection_start: usize,
    pub selection_end: usize,
    /// The span an input method is composing, if any.
    pub composition: Option<(usize, usize)>,
}

impl TextEditState {
    /// `text` with the caret at its end.
    pub fn caret_at_end(text: impl Into<String>) -> TextEditState {
        let text = text.into();
        let end = text.len();
        TextEditState { text, selection_start: end, selection_end: end, composition: None }
    }
}

/// The software keyboard a field wants (Compose's KeyboardType).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum KeyboardType {
    #[default]
    Unspecified,
    Text,
    Ascii,
    Number,
    Phone,
    Uri,
    Email,
    Password,
    NumberPassword,
    Decimal,
}

/// The action key a field shows (Compose's ImeAction).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ImeAction {
    #[default]
    Unspecified,
    Default,
    None,
    Go,
    Search,
    Send,
    Previous,
    Next,
    Done,
}

/// How the keyboard capitalizes (Compose's KeyboardCapitalization).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum KeyboardCapitalization {
    #[default]
    Unspecified,
    None,
    Characters,
    Words,
    Sentences,
}

/// What a focused field asks of the input method (C++'s ImeOptions: Compose's KeyboardOptions and a field's
/// single-lineness).
#[derive(Clone, PartialEq, Debug)]
pub struct ImeOptions {
    pub keyboard_type: KeyboardType,
    pub ime_action: ImeAction,
    pub capitalization: KeyboardCapitalization,
    pub auto_correct: bool,
    pub single_line: bool,
    /// False: the typing goes to the field, the software keyboard stays as it is.
    pub show_keyboard: bool,
}

impl Default for ImeOptions {
    fn default() -> Self {
        ImeOptions {
            keyboard_type: KeyboardType::Unspecified,
            ime_action: ImeAction::Unspecified,
            capitalization: KeyboardCapitalization::Unspecified,
            auto_correct: true,
            single_line: true,
            show_keyboard: true,
        }
    }
}

/// The mouse cursor over something (Compose's PointerIcon, and C++'s other cursors).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PointerIcon {
    #[default]
    Default,
    Hand,
    Text,
    Crosshair,
    Move,
    ResizeHorizontal,
    ResizeVertical,
    NotAllowed,
    Wait,
}

/// A kind of haptic feedback (Compose's HapticFeedbackType).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HapticFeedbackType {
    LongPress,
    TextHandleMove,
    Confirm,
    Reject,
    ContextClick,
    GestureEnd,
    GestureThresholdActivate,
    KeyboardTap,
    SegmentTick,
    SegmentFrequentTick,
    ToggleOn,
    ToggleOff,
    VirtualKey,
}

/// The size an image is wanted at, for a big one to be decoded smaller (C++'s ImageRequestSize): 0 by 0 for the
/// whole image; `fill`, it is to cover that box rather than fit in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ImageRequestSize {
    pub width: i32,
    pub height: i32,
    pub fill: bool,
}

/// What a platform does for the UI (C++'s `KompozHost::Callbacks`). Every method does nothing by default (a
/// headless host).
pub trait PlatformCallbacks {
    /// What schedules a frame, callable from any thread (the host keeps it).
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        None
    }
    /// Shows (or shows again) or hides the keyboard or input method for the focused field, over `area`.
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        let _ = (active, area, state, options);
    }
    /// The app changed the focused field's text itself: the input method synced again.
    fn text_input_changed(&self, state: &TextEditState) {
        let _ = state;
    }
    fn cursor(&self, icon: PointerIcon) {
        let _ = icon;
    }
    /// The icon a hovering stylus shows, none for the platform's own (Compose's `setStylusHoverIcon`).
    fn stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        let _ = icon;
    }
    /// Decodes an image asset.
    fn load_image(&self, path: &str) -> Option<ImageBitmap> {
        let _ = path;
        None
    }
    /// Loads an image away from the UI thread (an asset or file path, or an http(s) URL) and calls `done` with it,
    /// from any thread; a big one may be decoded smaller for `size`.
    fn load_image_async(&self, model: &str, size: ImageRequestSize, done: Box<dyn FnOnce(Option<ImageBitmap>) + Send>) {
        let _ = (model, size);
        done(None);
    }
    fn get_clipboard(&self) -> Option<String> {
        None
    }
    fn set_clipboard(&self, text: &str) {
        let _ = text;
    }
    /// Dark icons over light system bars (true) or light icons over dark ones.
    fn system_bars_appearance(&self, light_status_bars: bool, light_navigation_bars: bool) {
        let _ = (light_status_bars, light_navigation_bars);
    }
    /// The screen shown changed (a NavHost's top entry, as its path): the web puts it in the address.
    fn location_changed(&self, path: &str) {
        let _ = path;
    }
    fn open_uri(&self, uri: &str) {
        let _ = uri;
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        let _ = kind;
    }
    fn hide_software_keyboard(&self) {}
    /// Whether the keyboard can be moved by hand (Android 11+'s WindowInsetsAnimationController): what
    /// `ime_nested_scroll` drives through `ime_control_by` and `ime_control_end`.
    fn can_control_ime(&self) -> bool {
        false
    }
    /// The keyboard moved by hand (Android 11+): by `px` up (down if negative).
    fn ime_control_by(&self, px: f32) {
        let _ = px;
    }
    /// Let go with a velocity (px/s, up positive), to finish shown or hidden.
    fn ime_control_end(&self, velocity: f32) {
        let _ = velocity;
    }
    /// Draws `draw` (px, from 0, 0) offscreen and returns the pixels (a GraphicsLayer's `toImageBitmap`).
    fn render_image(&self, size: Size, draw: &dyn Fn(&mut dyn GraphicsContext)) -> Option<ImageBitmap> {
        let _ = (size, draw);
        None
    }
    /// Autofill: a field (by its semantics id) got the focus (with its bounds) or lost it.
    fn autofill_focus(&self, id: u64, bounds: Rect, entered: bool) {
        let _ = (id, bounds, entered);
    }
    /// Autofill: its text changed.
    fn autofill_value(&self, id: u64, text: &str) {
        let _ = (id, text);
    }
    /// Autofill: it came into the layout or went.
    fn autofill_visibility(&self, id: u64, visible: bool) {
        let _ = (id, visible);
    }
    /// Autofill: the form was sent (commit) or dropped.
    fn autofill_done(&self, commit: bool) {
        let _ = commit;
    }
    /// Where the system's edge swipes (back, home) are kept off, px in the window: told whenever it changes
    /// (Android's `View.setSystemGestureExclusionRects`).
    fn system_gesture_exclusion(&self, rects: &[IntRect]) {
        let _ = rects;
    }
    /// An asset's bytes (the app's assets, or a file next to the binary).
    fn read_asset(&self, path: &str) -> Option<Vec<u8>> {
        let _ = path;
        None
    }
    /// Shows (or shows again) the platform's floating text toolbar over `rect` (px in the window) with the actions
    /// it offers (the host's [`TextToolbar`], Android's floating ActionMode).
    fn show_text_toolbar(&self, rect: Rect, actions: &TextToolbarActions) {
        let _ = (rect, actions);
    }
    /// Hides the text toolbar.
    fn hide_text_toolbar(&self) {}
}

/// The callbacks of a headless host: nothing.
pub struct NoPlatform;
impl PlatformCallbacks for NoPlatform {}

// ---------------------------------------------------------------------------------------------------------------
// The view configuration

/// The platform's gesture thresholds (Compose's ViewConfiguration), in px for the lengths.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ViewConfiguration {
    pub long_press_timeout: Duration,
    pub double_tap_timeout: Duration,
    pub double_tap_min_time: Duration,
    /// How far a pointer moves before it is a drag.
    pub touch_slop: f32,
    /// The least size of something touched (48 dp at density 1: Compose's default).
    pub minimum_touch_target_size: Size,
    /// The fastest fling, px/s.
    pub maximum_fling_velocity: f32,
    /// The slowest fling, px/s.
    pub minimum_fling_velocity: f32,
    /// How far a mouse moves before a press is a drag (Compose desktop's handwriting slop is separate).
    pub handwriting_slop: f32,
}

impl Default for ViewConfiguration {
    /// Android's at density 1 (C++'s defaults).
    fn default() -> Self {
        ViewConfiguration {
            long_press_timeout: Duration::from_millis(400),
            double_tap_timeout: Duration::from_millis(300),
            double_tap_min_time: Duration::from_millis(40),
            touch_slop: 8.0,
            minimum_touch_target_size: Size::new(48.0, 48.0),
            maximum_fling_velocity: 8000.0,
            minimum_fling_velocity: 50.0,
            handwriting_slop: 2.0,
        }
    }
}

impl ViewConfiguration {
    /// The defaults scaled to `density` (the lengths are dp in Android's defaults).
    pub fn for_density(density: Density) -> ViewConfiguration {
        let d = density.density;
        let base = ViewConfiguration::default();
        ViewConfiguration {
            touch_slop: base.touch_slop * d,
            minimum_touch_target_size: Size::new(48.0 * d, 48.0 * d),
            maximum_fling_velocity: base.maximum_fling_velocity * d,
            minimum_fling_velocity: base.minimum_fling_velocity * d,
            handwriting_slop: base.handwriting_slop * d,
            ..base
        }
    }
    pub fn long_press_timeout_millis(&self) -> i64 {
        self.long_press_timeout.as_millis() as i64
    }
    pub fn double_tap_timeout_millis(&self) -> i64 {
        self.double_tap_timeout.as_millis() as i64
    }
    pub fn double_tap_min_time_millis(&self) -> i64 {
        self.double_tap_min_time.as_millis() as i64
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Handles the locals give

/// The window the UI is in (Compose's WindowInfo): whether it has the focus, its size, the keyboard modifiers
/// held. Read through states: what reads them is told when they change.
#[derive(Clone, PartialEq)]
pub struct WindowInfo {
    pub(crate) is_window_focused: MutableState<bool>,
    pub(crate) container_size: MutableState<IntSize>,
    pub(crate) keyboard_modifiers: MutableState<u32>,
}

impl Default for WindowInfo {
    fn default() -> Self {
        WindowInfo {
            is_window_focused: mutable_state_of(true),
            container_size: mutable_state_of(IntSize::Zero),
            keyboard_modifiers: mutable_state_of(0),
        }
    }
}

impl WindowInfo {
    pub fn is_window_focused(&self) -> bool {
        self.is_window_focused.get()
    }
    /// The window's size in px.
    pub fn container_size(&self) -> IntSize {
        self.container_size.get()
    }
    /// The keyboard modifiers held (the input package's bit set).
    pub fn keyboard_modifiers(&self) -> u32 {
        self.keyboard_modifiers.get()
    }
}

/// How the user is interacting (Compose's InputMode): touch, or keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputMode {
    Touch,
    Keyboard,
}

/// The input mode and asking for another (Compose's InputModeManager).
#[derive(Clone, PartialEq)]
pub struct InputModeManager {
    pub(crate) mode: MutableState<InputMode>,
}

impl Default for InputModeManager {
    fn default() -> Self {
        InputModeManager { mode: mutable_state_of(InputMode::Touch) }
    }
}

impl InputModeManager {
    pub fn input_mode(&self) -> InputMode {
        self.mode.get()
    }
    /// Asks for `mode`; whether it is the mode now.
    pub fn request_input_mode(&self, mode: InputMode) -> bool {
        self.mode.set(mode);
        true
    }
}

/// What a handle of the platform's reaches: the host's callbacks, whichever they are now (the host can be given
/// others without its locals changing).
#[derive(Clone)]
pub struct PlatformHandle(pub(crate) Rc<std::cell::RefCell<Rc<dyn PlatformCallbacks>>>);

impl PlatformHandle {
    pub(crate) fn new(callbacks: Rc<dyn PlatformCallbacks>) -> PlatformHandle {
        PlatformHandle(Rc::new(std::cell::RefCell::new(callbacks)))
    }
    /// The callbacks now.
    pub fn get(&self) -> Rc<dyn PlatformCallbacks> {
        self.0.borrow().clone()
    }
    pub(crate) fn set(&self, callbacks: Rc<dyn PlatformCallbacks>) {
        *self.0.borrow_mut() = callbacks;
    }
}

impl PartialEq for PlatformHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for PlatformHandle {
    fn default() -> Self {
        PlatformHandle::new(Rc::new(NoPlatform))
    }
}

/// Haptic feedback (Compose's HapticFeedback).
#[derive(Clone, PartialEq, Default)]
pub struct HapticFeedback(pub(crate) PlatformHandle);

impl HapticFeedback {
    pub fn perform_haptic_feedback(&self, kind: HapticFeedbackType) {
        self.0.get().haptic_feedback(kind);
    }
}

/// The clipboard (Compose's Clipboard), its text.
#[derive(Clone, PartialEq, Default)]
pub struct Clipboard(pub(crate) PlatformHandle);

impl Clipboard {
    pub fn get_text(&self) -> Option<String> {
        self.0.get().get_clipboard()
    }
    pub fn set_text(&self, text: &str) {
        self.0.get().set_clipboard(text);
    }
}

/// Whether a text toolbar is shown (Compose's TextToolbarStatus).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextToolbarStatus {
    Shown,
    Hidden,
}

/// An action a text toolbar offers: none when it isn't offered.
pub type TextToolbarAction = Option<Rc<dyn Fn()>>;

/// The floating toolbar over selected text (Compose's TextToolbar): copy, paste, cut, select all and autofill for
/// a selection or a field. What shows one (a SelectionContainer, a text field) calls `show_menu` with the rect of
/// the selection (px in the window) and the actions that can be done now — none for the others — while it should
/// show, again when they change, and `hide` once it shouldn't. Provided through [`LocalTextToolbar`]: the host's
/// hands it to the platform, an app's own draws it its own way.
pub trait TextToolbar {
    /// Shows the toolbar over `rect` with the actions given (Compose's `showMenu`).
    fn show_menu(
        &self,
        rect: Rect,
        on_copy_requested: TextToolbarAction,
        on_paste_requested: TextToolbarAction,
        on_cut_requested: TextToolbarAction,
        on_select_all_requested: TextToolbarAction,
        on_autofill_requested: TextToolbarAction,
    );
    /// Hides it.
    fn hide(&self);
    /// Whether it is shown.
    fn status(&self) -> TextToolbarStatus;
}

/// The actions of a text toolbar as the platform is given them ([`PlatformCallbacks::show_text_toolbar`]): each
/// one it offers, none for those it doesn't.
#[derive(Clone, Default)]
pub struct TextToolbarActions {
    pub on_copy_requested: TextToolbarAction,
    pub on_paste_requested: TextToolbarAction,
    pub on_cut_requested: TextToolbarAction,
    pub on_select_all_requested: TextToolbarAction,
    pub on_autofill_requested: TextToolbarAction,
}

impl std::fmt::Debug for TextToolbarActions {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("TextToolbarActions")
            .field("copy", &self.on_copy_requested.is_some())
            .field("paste", &self.on_paste_requested.is_some())
            .field("cut", &self.on_cut_requested.is_some())
            .field("select_all", &self.on_select_all_requested.is_some())
            .field("autofill", &self.on_autofill_requested.is_some())
            .finish()
    }
}

/// The host's text toolbar (Compose's AndroidTextToolbar): it hands the menu to the platform
/// ([`PlatformCallbacks::show_text_toolbar`] and `hide_text_toolbar`, which do nothing on a headless host) and
/// keeps whether it is shown.
pub struct PlatformTextToolbar {
    platform: PlatformHandle,
    status: Cell<TextToolbarStatus>,
}

impl PlatformTextToolbar {
    pub fn new(platform: PlatformHandle) -> PlatformTextToolbar {
        PlatformTextToolbar { platform, status: Cell::new(TextToolbarStatus::Hidden) }
    }
}

impl TextToolbar for PlatformTextToolbar {
    fn show_menu(
        &self,
        rect: Rect,
        on_copy_requested: TextToolbarAction,
        on_paste_requested: TextToolbarAction,
        on_cut_requested: TextToolbarAction,
        on_select_all_requested: TextToolbarAction,
        on_autofill_requested: TextToolbarAction,
    ) {
        self.status.set(TextToolbarStatus::Shown);
        let actions = TextToolbarActions {
            on_copy_requested,
            on_paste_requested,
            on_cut_requested,
            on_select_all_requested,
            on_autofill_requested,
        };
        self.platform.get().show_text_toolbar(rect, &actions);
    }

    fn hide(&self) {
        self.status.set(TextToolbarStatus::Hidden);
        self.platform.get().hide_text_toolbar();
    }

    fn status(&self) -> TextToolbarStatus {
        self.status.get()
    }
}

/// Opens links (Compose's UriHandler): the app's own (those a navigation host answers, see [`DeepLinks`]) in it,
/// the others by the platform (C++'s `openUri`).
#[derive(Clone, PartialEq, Default)]
pub struct UriHandler(pub(crate) PlatformHandle, pub(crate) DeepLinks);

impl UriHandler {
    pub fn open_uri(&self, uri: &str) {
        if self.1.answers(uri) {
            self.1.open(uri);
        } else {
            self.0.get().open_uri(uri);
        }
    }
}

struct DeepLinkState {
    /// The link the app was opened with or sent, until a navigation host takes it.
    pending: std::cell::RefCell<Option<String>>,
    /// Counted up as links come: what takes them reads it, and is composed again when one comes.
    generation: MutableState<u64>,
    /// What the navigation hosts in the composition answer, by owner.
    handlers: std::cell::RefCell<Vec<(usize, Rc<dyn Fn(&str) -> bool>)>>,
}

/// The links the app is opened with or sent while it runs (C++'s deep links: a URI such as "kompoz://item/42",
/// or a route's path), for navigation's deep links: the first navigation host with a destination for one takes it.
/// Provided at the root by the host ([`LocalDeepLinks`]); cheap to clone, equal when the same.
#[derive(Clone)]
pub struct DeepLinks(Rc<DeepLinkState>);

impl DeepLinks {
    pub fn new() -> DeepLinks {
        DeepLinks(Rc::new(DeepLinkState {
            pending: std::cell::RefCell::new(None),
            generation: mutable_state_of(0),
            handlers: std::cell::RefCell::new(Vec::new()),
        }))
    }

    /// A link to go to (C++'s `openDeepLink`): kept until something takes it; what reads the links is composed
    /// again.
    pub fn open(&self, link: &str) {
        *self.0.pending.borrow_mut() = Some(link.to_string());
        self.0.generation.set(self.0.generation.peek() + 1);
    }

    /// The link waiting, taken, if `accepts` it (C++'s `takeDeepLink`); the caller is composed again when another
    /// comes.
    pub fn take(&self, accepts: impl FnOnce(&str) -> bool) -> Option<String> {
        self.0.generation.get();
        let mut pending = self.0.pending.borrow_mut();
        if pending.as_deref().is_some_and(accepts) { pending.take() } else { None }
    }

    /// The link waiting, if any (read by no one).
    pub fn pending(&self) -> Option<String> {
        self.0.pending.borrow().clone()
    }

    /// `owner` (a navigation host) answers the links `accepts` says (C++'s `addDeepLinkHandler`): opening one
    /// ([`UriHandler::open_uri`]) goes to it rather than to the platform; a later call of the same owner replaces
    /// it.
    pub fn add_handler(&self, owner: usize, accepts: impl Fn(&str) -> bool + 'static) {
        self.remove_handler(owner);
        self.0.handlers.borrow_mut().push((owner, Rc::new(accepts)));
    }

    pub fn remove_handler(&self, owner: usize) {
        self.0.handlers.borrow_mut().retain(|(o, _)| *o != owner);
    }

    /// Whether a handler answers `uri`.
    pub fn answers(&self, uri: &str) -> bool {
        let handlers: Vec<Rc<dyn Fn(&str) -> bool>> = self.0.handlers.borrow().iter().map(|(_, h)| h.clone()).collect();
        handlers.iter().any(|h| h(uri))
    }
}

impl Default for DeepLinks {
    fn default() -> Self {
        DeepLinks::new()
    }
}

impl PartialEq for DeepLinks {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The software keyboard (Compose's SoftwareKeyboardController): hidden on request; shown by a focused field.
#[derive(Clone, PartialEq, Default)]
pub struct SoftwareKeyboardController(pub(crate) PlatformHandle);

impl SoftwareKeyboardController {
    pub fn hide(&self) {
        self.0.get().hide_software_keyboard();
    }
}

/// The system's theme (Compose desktop's SystemTheme).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SystemTheme {
    #[default]
    Light,
    Dark,
    Unknown,
}

/// What each kind of system UI covers of the window, in px (C++'s SystemInsets; the window insets of the layout
/// package are made from it).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Insets {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Insets {
    pub const Zero: Insets = Insets { left: 0, top: 0, right: 0, bottom: 0 };
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Insets {
        Insets { left, top, right, bottom }
    }
}

/// A kind of system UI the platform reports insets for (Android's WindowInsetsCompat types; the waterfall is the
/// display cutout's curved edges).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InsetsKind {
    StatusBars,
    NavigationBars,
    CaptionBar,
    Ime,
    DisplayCutout,
    SystemGestures,
    MandatorySystemGestures,
    TappableElement,
    Waterfall,
}

impl InsetsKind {
    /// Every kind.
    pub const All: [InsetsKind; 9] = [
        InsetsKind::StatusBars,
        InsetsKind::NavigationBars,
        InsetsKind::CaptionBar,
        InsetsKind::Ime,
        InsetsKind::DisplayCutout,
        InsetsKind::SystemGestures,
        InsetsKind::MandatorySystemGestures,
        InsetsKind::TappableElement,
        InsetsKind::Waterfall,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

/// A kind of system UI moving (Android's WindowInsetsAnimationCompat): from where it was to where it goes, how far
/// along (its interpolated fraction), its alpha and how long it lasts.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InsetsAnimation {
    pub kind: InsetsKind,
    pub source: Insets,
    pub target: Insets,
    pub fraction: f32,
    pub alpha: f32,
    pub duration_millis: i64,
}

/// What the system UI covers (C++'s SystemInsets, Android's WindowInsetsCompat types): what each kind covers now,
/// whether it is shown and what it would cover shown, the display cutouts, and what is moving.
#[derive(Clone, PartialEq, Debug)]
pub struct SystemInsets {
    pub status_bars: Insets,
    pub navigation_bars: Insets,
    pub ime: Insets,
    pub display_cutout: Insets,
    pub system_gestures: Insets,
    pub caption_bar: Insets,
    pub mandatory_system_gestures: Insets,
    pub tappable_element: Insets,
    pub waterfall: Insets,
    /// Whether each kind is shown (Android's `isVisible`): one hidden covers nothing now (its insets above are
    /// none), though it would cover what its `*_ignoring_visibility` says. Shown unless told otherwise; the
    /// waterfall is shown when it covers something.
    pub status_bars_visible: bool,
    pub navigation_bars_visible: bool,
    pub caption_bar_visible: bool,
    pub display_cutout_visible: bool,
    pub system_gestures_visible: bool,
    pub mandatory_system_gestures_visible: bool,
    pub tappable_element_visible: bool,
    /// What each kind would cover shown (Android's `getInsetsIgnoringVisibility`; the keyboard has none): none,
    /// what it covers now.
    pub status_bars_ignoring_visibility: Option<Insets>,
    pub navigation_bars_ignoring_visibility: Option<Insets>,
    pub caption_bar_ignoring_visibility: Option<Insets>,
    pub display_cutout_ignoring_visibility: Option<Insets>,
    pub system_gestures_ignoring_visibility: Option<Insets>,
    pub mandatory_system_gestures_ignoring_visibility: Option<Insets>,
    pub tappable_element_ignoring_visibility: Option<Insets>,
    /// The display cutouts' shape (Android's `DisplayCutout.getCutoutPath`, px in the window), if the platform
    /// knows it, and each cutout's bounds (`getBoundingRects`).
    pub cutout_path: Option<Path>,
    pub cutout_rects: Vec<IntRect>,
    /// The keyboard sliding in or out: `ime` follows it frame by frame from `ime_animation_source` to
    /// `ime_animation_target`.
    pub ime_animating: bool,
    /// Whether the keyboard is up or on its way up.
    pub ime_visible: bool,
    pub ime_animation_source: Insets,
    pub ime_animation_target: Insets,
    /// Other kinds moving (and the keyboard's animation in full, in place of `ime_animating`'s).
    pub animations: Vec<InsetsAnimation>,
}

impl Default for SystemInsets {
    /// Nothing covered; every kind shown but the keyboard.
    fn default() -> Self {
        SystemInsets {
            status_bars: Insets::Zero,
            navigation_bars: Insets::Zero,
            ime: Insets::Zero,
            display_cutout: Insets::Zero,
            system_gestures: Insets::Zero,
            caption_bar: Insets::Zero,
            mandatory_system_gestures: Insets::Zero,
            tappable_element: Insets::Zero,
            waterfall: Insets::Zero,
            status_bars_visible: true,
            navigation_bars_visible: true,
            caption_bar_visible: true,
            display_cutout_visible: true,
            system_gestures_visible: true,
            mandatory_system_gestures_visible: true,
            tappable_element_visible: true,
            status_bars_ignoring_visibility: None,
            navigation_bars_ignoring_visibility: None,
            caption_bar_ignoring_visibility: None,
            display_cutout_ignoring_visibility: None,
            system_gestures_ignoring_visibility: None,
            mandatory_system_gestures_ignoring_visibility: None,
            tappable_element_ignoring_visibility: None,
            cutout_path: None,
            cutout_rects: Vec::new(),
            ime_animating: false,
            ime_visible: false,
            ime_animation_source: Insets::Zero,
            ime_animation_target: Insets::Zero,
            animations: Vec::new(),
        }
    }
}

impl SystemInsets {
    /// What `kind` covers now.
    pub fn insets(&self, kind: InsetsKind) -> Insets {
        match kind {
            InsetsKind::StatusBars => self.status_bars,
            InsetsKind::NavigationBars => self.navigation_bars,
            InsetsKind::CaptionBar => self.caption_bar,
            InsetsKind::Ime => self.ime,
            InsetsKind::DisplayCutout => self.display_cutout,
            InsetsKind::SystemGestures => self.system_gestures,
            InsetsKind::MandatorySystemGestures => self.mandatory_system_gestures,
            InsetsKind::TappableElement => self.tappable_element,
            InsetsKind::Waterfall => self.waterfall,
        }
    }

    /// Whether `kind` is shown.
    pub fn is_visible(&self, kind: InsetsKind) -> bool {
        match kind {
            InsetsKind::StatusBars => self.status_bars_visible,
            InsetsKind::NavigationBars => self.navigation_bars_visible,
            InsetsKind::CaptionBar => self.caption_bar_visible,
            InsetsKind::Ime => self.ime_visible,
            InsetsKind::DisplayCutout => self.display_cutout_visible,
            InsetsKind::SystemGestures => self.system_gestures_visible,
            InsetsKind::MandatorySystemGestures => self.mandatory_system_gestures_visible,
            InsetsKind::TappableElement => self.tappable_element_visible,
            InsetsKind::Waterfall => self.waterfall != Insets::Zero,
        }
    }

    /// What `kind` would cover shown (the keyboard: none; the waterfall: what it covers).
    pub fn ignoring_visibility(&self, kind: InsetsKind) -> Option<Insets> {
        let given = match kind {
            InsetsKind::StatusBars => self.status_bars_ignoring_visibility,
            InsetsKind::NavigationBars => self.navigation_bars_ignoring_visibility,
            InsetsKind::CaptionBar => self.caption_bar_ignoring_visibility,
            InsetsKind::Ime => return None,
            InsetsKind::DisplayCutout => self.display_cutout_ignoring_visibility,
            InsetsKind::SystemGestures => self.system_gestures_ignoring_visibility,
            InsetsKind::MandatorySystemGestures => self.mandatory_system_gestures_ignoring_visibility,
            InsetsKind::TappableElement => self.tappable_element_ignoring_visibility,
            InsetsKind::Waterfall => None,
        };
        Some(given.unwrap_or_else(|| self.insets(kind)))
    }

    /// How `kind` is moving, if it is: its entry in `animations`, else (the keyboard) `ime_animating`'s.
    pub fn animation(&self, kind: InsetsKind) -> Option<InsetsAnimation> {
        if let Some(a) = self.animations.iter().find(|a| a.kind == kind) {
            return Some(*a);
        }
        (kind == InsetsKind::Ime && self.ime_animating).then_some(InsetsAnimation {
            kind,
            source: self.ime_animation_source,
            target: self.ime_animation_target,
            fraction: 0.0,
            alpha: 1.0,
            duration_millis: 0,
        })
    }
}

/// One kind's states.
struct KindStates {
    insets: MutableState<Insets>,
    visible: MutableState<bool>,
    ignoring_visibility: MutableState<Option<Insets>>,
    animation: MutableState<Option<InsetsAnimation>>,
}

struct HolderStates {
    kinds: Vec<KindStates>,
    cutout_path: MutableState<Option<Path>>,
    cutout_rects: MutableState<Vec<IntRect>>,
}

/// What the system UI covers as the host keeps it, each kind in states of its own (Compose's WindowInsetsHolder and
/// InsetsListener's values): what reads a kind (a layout measuring by it, a composable) is told when the platform
/// reports another value for it, and only then: the keyboard sliding in measures again what reads the keyboard,
/// nothing else, and composes nothing. Cheap to clone; equal when the same.
#[derive(Clone)]
pub struct SystemInsetsHolder(Rc<HolderStates>);

impl SystemInsetsHolder {
    /// One holding `insets`.
    pub fn new(insets: &SystemInsets) -> SystemInsetsHolder {
        let kinds = InsetsKind::All
            .iter()
            .map(|&k| KindStates {
                insets: mutable_state_of(insets.insets(k)),
                visible: mutable_state_of(insets.is_visible(k)),
                ignoring_visibility: mutable_state_of(insets.ignoring_visibility(k)),
                animation: mutable_state_of(insets.animation(k)),
            })
            .collect();
        SystemInsetsHolder(Rc::new(HolderStates {
            kinds,
            cutout_path: mutable_state_of(insets.cutout_path.clone()),
            cutout_rects: mutable_state_of(insets.cutout_rects.clone()),
        }))
    }

    fn kind(&self, kind: InsetsKind) -> &KindStates {
        &self.0.kinds[kind.index()]
    }

    /// What `kind` covers now (read: the reader is told when it changes).
    pub fn insets(&self, kind: InsetsKind) -> Insets {
        self.kind(kind).insets.get()
    }

    /// The same, read by no one.
    pub fn peek_insets(&self, kind: InsetsKind) -> Insets {
        self.kind(kind).insets.peek()
    }

    /// Whether `kind` is shown (read).
    pub fn is_visible(&self, kind: InsetsKind) -> bool {
        self.kind(kind).visible.get()
    }

    /// What `kind` would cover shown (read); the keyboard: none.
    pub fn ignoring_visibility(&self, kind: InsetsKind) -> Option<Insets> {
        self.kind(kind).ignoring_visibility.get()
    }

    /// The same, read by no one.
    pub fn peek_ignoring_visibility(&self, kind: InsetsKind) -> Option<Insets> {
        self.kind(kind).ignoring_visibility.peek()
    }

    /// How `kind` is moving, if it is (read).
    pub fn animation(&self, kind: InsetsKind) -> Option<InsetsAnimation> {
        self.kind(kind).animation.get()
    }

    /// The display cutouts' shape, if known (read).
    pub fn cutout_path(&self) -> Option<Path> {
        self.0.cutout_path.get()
    }

    /// Each display cutout's bounds, px in the window (read).
    pub fn cutout_rects(&self) -> Vec<IntRect> {
        self.0.cutout_rects.get()
    }

    /// All of it as one value (every state read).
    pub fn get(&self) -> SystemInsets {
        let k = |kind| self.insets(kind);
        let v = |kind| self.is_visible(kind);
        let i = |kind| self.ignoring_visibility(kind);
        let ime = self.animation(InsetsKind::Ime);
        SystemInsets {
            status_bars: k(InsetsKind::StatusBars),
            navigation_bars: k(InsetsKind::NavigationBars),
            ime: k(InsetsKind::Ime),
            display_cutout: k(InsetsKind::DisplayCutout),
            system_gestures: k(InsetsKind::SystemGestures),
            caption_bar: k(InsetsKind::CaptionBar),
            mandatory_system_gestures: k(InsetsKind::MandatorySystemGestures),
            tappable_element: k(InsetsKind::TappableElement),
            waterfall: k(InsetsKind::Waterfall),
            status_bars_visible: v(InsetsKind::StatusBars),
            navigation_bars_visible: v(InsetsKind::NavigationBars),
            caption_bar_visible: v(InsetsKind::CaptionBar),
            display_cutout_visible: v(InsetsKind::DisplayCutout),
            system_gestures_visible: v(InsetsKind::SystemGestures),
            mandatory_system_gestures_visible: v(InsetsKind::MandatorySystemGestures),
            tappable_element_visible: v(InsetsKind::TappableElement),
            status_bars_ignoring_visibility: i(InsetsKind::StatusBars),
            navigation_bars_ignoring_visibility: i(InsetsKind::NavigationBars),
            caption_bar_ignoring_visibility: i(InsetsKind::CaptionBar),
            display_cutout_ignoring_visibility: i(InsetsKind::DisplayCutout),
            system_gestures_ignoring_visibility: i(InsetsKind::SystemGestures),
            mandatory_system_gestures_ignoring_visibility: i(InsetsKind::MandatorySystemGestures),
            tappable_element_ignoring_visibility: i(InsetsKind::TappableElement),
            cutout_path: self.cutout_path(),
            cutout_rects: self.cutout_rects(),
            ime_animating: ime.is_some(),
            ime_visible: v(InsetsKind::Ime),
            ime_animation_source: ime.map_or(Insets::Zero, |a| a.source),
            ime_animation_target: ime.map_or(Insets::Zero, |a| a.target),
            animations: InsetsKind::All.iter().filter_map(|&kind| self.animation(kind)).collect(),
        }
    }

    /// Holds `insets` from now on: the states whose values changed are set (their readers told), the others left
    /// (a state set to the value it has tells no one).
    pub fn set(&self, insets: &SystemInsets) {
        for &kind in &InsetsKind::All {
            let k = self.kind(kind);
            k.insets.set(insets.insets(kind));
            k.visible.set(insets.is_visible(kind));
            k.ignoring_visibility.set(insets.ignoring_visibility(kind));
            k.animation.set(insets.animation(kind));
        }
        self.0.cutout_path.set(insets.cutout_path.clone());
        self.0.cutout_rects.set(insets.cutout_rects.clone());
    }
}

impl Default for SystemInsetsHolder {
    /// Holding nothing covered (what nothing provides: a host updates its own).
    fn default() -> Self {
        SystemInsetsHolder::new(&SystemInsets::default())
    }
}

impl PartialEq for SystemInsetsHolder {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for SystemInsetsHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "SystemInsetsHolder({:?})", kompoz_runtime::without_read_observation(|| self.get()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Moving the keyboard by hand

struct ImeControlState {
    platform: PlatformHandle,
    insets: SystemInsetsHolder,
    focus: Option<std::rc::Weak<crate::focus::FocusOwner>>,
    controlling: Cell<bool>,
    /// Where the keyboard's top is (its height, px) as it is moved.
    height: Cell<f32>,
}

/// Moving the software keyboard by hand (C++'s `KompozHost::imeControlBy` / `imeControlEnd`, what
/// `ime_nested_scroll` drives; Android 11+'s WindowInsetsAnimationController): the host keeps whether it is being
/// moved and where it is, so content composed again mid-drag loses nothing. Cheap to clone; equal when the same.
#[derive(Clone)]
pub struct ImeController(Rc<ImeControlState>);

impl ImeController {
    pub(crate) fn new(
        platform: PlatformHandle,
        insets: SystemInsetsHolder,
        focus: Option<std::rc::Weak<crate::focus::FocusOwner>>,
    ) -> ImeController {
        ImeController(Rc::new(ImeControlState {
            platform,
            insets,
            focus,
            controlling: Cell::new(false),
            height: Cell::new(0.0),
        }))
    }

    /// Whether the keyboard is being moved by hand.
    pub fn is_controlling(&self) -> bool {
        self.0.controlling.get()
    }

    /// Whether a field that can be edited has the focus (what the keyboard is for).
    fn editable_focused(&self) -> bool {
        let Some(focus) = self.0.focus.as_ref().and_then(|f| f.upgrade()) else { return false };
        let Some(node) = focus.focused_layout_node() else { return false };
        node.semantics_configuration()
            .and_then(|c| c.get(crate::semantics::SemanticsProperties::IsEditable))
            .unwrap_or(false)
    }

    /// Moves the keyboard by `px` up (down if negative) under the finger, as far as it goes (no lower than hidden,
    /// no higher than its height when it is sliding up to it); what it moved (C++'s `imeControlBy`). It starts
    /// being moved only with a field that can be edited focused and a platform that can move it; otherwise
    /// nothing moves.
    pub fn control_by(&self, px: f32) -> f32 {
        let s = &self.0;
        let platform = s.platform.get();
        if !s.controlling.get() {
            if !platform.can_control_ime() || !self.editable_focused() {
                return 0.0;
            }
            s.controlling.set(true);
            s.height.set(s.insets.peek_insets(InsetsKind::Ime).bottom as f32);
        }
        let mut next = (s.height.get() + px).max(0.0);
        // (shown: its height)
        if let Some(a) = kompoz_runtime::without_read_observation(|| s.insets.animation(InsetsKind::Ime))
            && a.target.bottom > 0
        {
            next = next.min(a.target.bottom as f32);
        }
        let used = next - s.height.get();
        s.height.set(next);
        if used != 0.0 {
            platform.ime_control_by(used);
        }
        used
    }

    /// Lets go of the keyboard with `velocity` (px/s, up positive): the platform finishes it shown or hidden (C++'s
    /// `imeControlEnd`).
    pub fn control_end(&self, velocity: f32) {
        if !self.0.controlling.replace(false) {
            return;
        }
        self.0.platform.get().ime_control_end(velocity);
    }
}

impl Default for ImeController {
    /// One that moves nothing (what nothing provides: headless).
    fn default() -> Self {
        ImeController::new(PlatformHandle::default(), SystemInsetsHolder::default(), None)
    }
}

impl PartialEq for ImeController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The locals

/// Pixels per dp and the font scale (Compose's LocalDensity).
pub static LocalDensity: KompozitionLocal<Density> = KompozitionLocal::new_static(Density::default);
/// Which way layouts go (Compose's LocalLayoutDirection).
pub static LocalLayoutDirection: KompozitionLocal<LayoutDirection> =
    KompozitionLocal::new_static(|| LayoutDirection::Ltr);
/// The gesture thresholds (Compose's LocalViewConfiguration).
pub static LocalViewConfiguration: KompozitionLocal<ViewConfiguration> =
    KompozitionLocal::new_static(ViewConfiguration::default);
/// The window (Compose's LocalWindowInfo).
pub static LocalWindowInfo: KompozitionLocal<WindowInfo> = KompozitionLocal::new_static(WindowInfo::default);
/// The input mode (Compose's LocalInputModeManager).
pub static LocalInputModeManager: KompozitionLocal<InputModeManager> =
    KompozitionLocal::new_static(InputModeManager::default);
/// Haptic feedback (Compose's LocalHapticFeedback).
pub static LocalHapticFeedback: KompozitionLocal<HapticFeedback> =
    KompozitionLocal::new_static(HapticFeedback::default);
/// The clipboard (Compose's LocalClipboard).
pub static LocalClipboard: KompozitionLocal<Clipboard> = KompozitionLocal::new_static(Clipboard::default);
/// The floating toolbar over selected text (Compose's LocalTextToolbar): the host's, which hands it to the
/// platform; an app provides its own to draw it itself. Readers are told when it is another one.
pub static LocalTextToolbar: KompozitionLocal<Rc<dyn TextToolbar>> =
    KompozitionLocal::with_policy(SnapshotMutationPolicy::new(same_text_toolbar), || {
        Rc::new(PlatformTextToolbar::new(PlatformHandle::default()))
    });

fn same_text_toolbar(a: &Rc<dyn TextToolbar>, b: &Rc<dyn TextToolbar>) -> bool {
    Rc::ptr_eq(a, b)
}

/// Opening links (Compose's LocalUriHandler).
pub static LocalUriHandler: KompozitionLocal<UriHandler> = KompozitionLocal::new_static(UriHandler::default);
/// The software keyboard (Compose's LocalSoftwareKeyboardController).
pub static LocalSoftwareKeyboardController: KompozitionLocal<SoftwareKeyboardController> =
    KompozitionLocal::new_static(SoftwareKeyboardController::default);
/// The focus manager (Compose's LocalFocusManager).
pub static LocalFocusManager: KompozitionLocal<crate::focus::FocusManager> =
    KompozitionLocal::new_static(crate::focus::FocusManager::default);
/// The system's theme (Compose desktop's LocalSystemTheme; what `is_system_in_dark_theme` reads). Dynamic, unlike
/// Compose desktop's, as C++'s: the theme changing composes again only what reads it.
pub static LocalSystemTheme: KompozitionLocal<SystemTheme> = KompozitionLocal::new(SystemTheme::default);
/// What the system UI covers, as the host keeps it (what the window insets read; Compose's WindowInsetsHolder).
pub static LocalSystemInsets: KompozitionLocal<SystemInsetsHolder> =
    KompozitionLocal::new_static(SystemInsetsHolder::default);
/// Moving the keyboard by hand (what `ime_nested_scroll` drives).
pub static LocalImeController: KompozitionLocal<ImeController> = KompozitionLocal::new_static(ImeController::default);
/// The links the app is opened with (the host's; what navigation's deep links take).
pub static LocalDeepLinks: KompozitionLocal<DeepLinks> = KompozitionLocal::new_static(DeepLinks::new);
/// The system's palettes as Android 12+ gives them ("Material You", C++'s `setDynamicTonalPalette`; on a Linux
/// desktop made from its accent color): what a dynamic color scheme reads; none where the system has none.
pub static LocalDynamicTonalPalette: KompozitionLocal<Option<DynamicTonalPalette>> = KompozitionLocal::new(|| None);
/// Palettes made from the wallpaper's own colors where the platform reads them (C++'s
/// `setWallpaperTonalPalette`): what a wallpaper color scheme reads; none where it can't.
pub static LocalWallpaperTonalPalette: KompozitionLocal<Option<DynamicTonalPalette>> = KompozitionLocal::new(|| None);
/// The platform's callbacks, for what calls them directly (images, assets, autofill).
pub static LocalPlatform: KompozitionLocal<PlatformHandle> = KompozitionLocal::new_static(PlatformHandle::default);
/// What text resolves its font families with, and shapes and measures through (Compose's
/// LocalFontFamilyResolver): the host's (see [`crate::KompozHost::set_font_family_resolver`]); readers are told
/// when it is another one. Without a host: one over kompoz-text's FakeBackend.
pub static LocalFontFamilyResolver: KompozitionLocal<Rc<dyn FontFamilyResolver>> =
    KompozitionLocal::with_policy(SnapshotMutationPolicy::new(same_resolver), fake_resolver);

fn same_resolver(a: &Rc<dyn FontFamilyResolver>, b: &Rc<dyn FontFamilyResolver>) -> bool {
    Rc::ptr_eq(a, b)
}

/// A resolver over kompoz-text's FakeBackend: what the test rule installs (text measured as the C++ Kompoz's tests
/// measured it).
pub fn fake_resolver() -> Rc<dyn FontFamilyResolver> {
    Rc::new(FakeFontFamilyResolver::new())
}

/// The thread's resolver over the system's fonts (kompoz-text's `FontFamilyResolverImpl::system()`, the fonts
/// scanned when text first needs them): what a host starts with (Compose's `createFontFamilyResolver(context)`).
pub fn system_resolver() -> Rc<dyn FontFamilyResolver> {
    FontFamilyResolverImpl::system()
}

/// Whether the system is in its dark theme (Compose's `isSystemInDarkTheme`): reading it recomposes the reader when
/// the platform says otherwise.
pub fn is_system_in_dark_theme() -> bool {
    LocalSystemTheme.current() == SystemTheme::Dark
}

// ---------------------------------------------------------------------------------------------------------------
// The window's modifiers

impl Modifier {
    /// Keeps the screen on while the layout is in the tree (Compose's `Modifier.keepScreenOn`).
    pub fn keep_screen_on(self) -> Modifier {
        self.then_element(KeepScreenOnElement)
    }

    /// Marks the layout's content sensitive while it is in the tree: the platform protects the window (from
    /// screen sharing; Compose's `Modifier.sensitiveContent`).
    pub fn sensitive_content(self, is_content_sensitive: bool) -> Modifier {
        self.then_element(SensitiveContentElement { sensitive: is_content_sensitive })
    }

    /// The refresh rate the layout would like while it is in the tree: a rate in Hz, or a category (Compose's
    /// `Modifier.preferredFrameRate`). The host gathers the fastest rate and the highest category asked.
    pub fn preferred_frame_rate(self, frame_rate: impl Into<FrameRate>) -> Modifier {
        self.then_element(FrameRateElement { rate: frame_rate.into() })
    }

    /// Keeps the system's edge swipes (back, home) off the layout's bounds: a drag starting on it stays the app's
    /// (Compose's foundation `Modifier.systemGestureExclusion`). The host gathers the rects of all of them and
    /// tells the platform as they change.
    pub fn system_gesture_exclusion(self) -> Modifier {
        self.then_element(GestureExclusionElement { exclusion: None })
    }

    /// The same for the part of it `exclusion` gives, in its own coordinates (Compose's
    /// `systemGestureExclusion(exclusion)`): a slider's edge, say.
    pub fn system_gesture_exclusion_with(self, exclusion: impl Fn(&LayoutCoordinates) -> Rect + 'static) -> Modifier {
        self.then_element(GestureExclusionElement { exclusion: Some(Rc::new(exclusion)) })
    }
}

type GestureExclusion = Rc<dyn Fn(&LayoutCoordinates) -> Rect>;

struct GestureExclusionElement {
    exclusion: Option<GestureExclusion>,
}

impl PartialEq for GestureExclusionElement {
    fn eq(&self, other: &Self) -> bool {
        match (&self.exclusion, &other.exclusion) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl ModifierNodeElement for GestureExclusionElement {
    type Node = GestureExclusionNode;
    fn create(&self) -> GestureExclusionNode {
        GestureExclusionNode { exclusion: self.exclusion.clone(), owner: None }
    }
    fn update(&self, node: &mut GestureExclusionNode) {
        node.exclusion = self.exclusion.clone();
    }
    fn name(&self) -> &'static str {
        "systemGestureExclusion"
    }
}

/// The node of `Modifier.system_gesture_exclusion` (Compose's ExcludeFromSystemGestureNode over RectListNode):
/// where it is in the window after each layout pass that moved it, its rect kept by the host until it is detached.
pub struct GestureExclusionNode {
    exclusion: Option<GestureExclusion>,
    owner: Option<std::rc::Weak<crate::node::Owner>>,
}

impl GestureExclusionNode {
    /// Its rect in the root, rounded out as Compose's `calcBounds` (the bounds of the rect's corners).
    fn rect(&self, coordinates: &LayoutCoordinates) -> IntRect {
        let round = |v: f32| (v + 0.5).floor() as i32;
        let bounds = match &self.exclusion {
            None => coordinates.bounds_in_root(),
            Some(exclusion) => {
                let r = exclusion(coordinates);
                let root = coordinates.find_root_coordinates();
                let corners = [r.top_left(), r.top_right(), r.bottom_left(), r.bottom_right()]
                    .map(|c| root.local_position_of(coordinates, c));
                let xs = corners.map(|c| c.x);
                let ys = corners.map(|c| c.y);
                Rect::new(
                    xs.iter().copied().fold(f32::INFINITY, f32::min),
                    ys.iter().copied().fold(f32::INFINITY, f32::min),
                    xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                    ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                )
            }
        };
        IntRect::new(round(bounds.left), round(bounds.top), round(bounds.right), round(bounds.bottom))
    }

    fn key(&self) -> usize {
        self as *const GestureExclusionNode as usize
    }
}

impl ModifierNode for GestureExclusionNode {
    crate::node_capabilities!(global_position_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.owner = cx.layout_node().and_then(|n| n.owner()).map(|o| Rc::downgrade(&o));
    }

    fn on_detach(&mut self, _: &NodeCx) {
        if let Some(owner) = self.owner.take().and_then(|o| o.upgrade()) {
            owner.set_gesture_exclusion(self.key(), None);
        }
    }
}

impl crate::layout::GlobalPositionAwareModifierNode for GestureExclusionNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        let rect = self.rect(coordinates);
        if let Some(owner) = self.owner.as_ref().and_then(|o| o.upgrade()) {
            // (an empty rect excludes nothing: none kept)
            let kept = (rect.right > rect.left && rect.bottom > rect.top).then_some(rect);
            owner.set_gesture_exclusion(self.key(), kept);
        }
    }
}

#[derive(PartialEq)]
struct KeepScreenOnElement;

impl ModifierNodeElement for KeepScreenOnElement {
    type Node = KeepScreenOnNode;
    fn create(&self) -> KeepScreenOnNode {
        KeepScreenOnNode
    }
    fn update(&self, _: &mut KeepScreenOnNode) {}
    fn name(&self) -> &'static str {
        "keepScreenOn"
    }
}

/// The node of `Modifier.keep_screen_on`: counted by the host while attached.
pub struct KeepScreenOnNode;

impl ModifierNode for KeepScreenOnNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        if let Some(owner) = cx.layout_node().and_then(|n| n.owner()) {
            owner.keep_screen_on.set(owner.keep_screen_on.get() + 1);
        }
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(owner) = cx.layout_node().and_then(|n| n.owner()) {
            owner.keep_screen_on.set(owner.keep_screen_on.get() - 1);
        }
    }
}

#[derive(PartialEq)]
struct SensitiveContentElement {
    sensitive: bool,
}

impl ModifierNodeElement for SensitiveContentElement {
    type Node = SensitiveContentNode;
    fn create(&self) -> SensitiveContentNode {
        SensitiveContentNode { sensitive: self.sensitive, counted: false, handle: None }
    }
    fn update(&self, node: &mut SensitiveContentNode) {
        node.sensitive = self.sensitive;
        node.recount();
    }
    fn name(&self) -> &'static str {
        "sensitiveContent"
    }
}

/// The node of `Modifier.sensitive_content`: counted by the host while attached and sensitive.
pub struct SensitiveContentNode {
    sensitive: bool,
    counted: bool,
    handle: Option<crate::modifier::NodeHandle>,
}

impl SensitiveContentNode {
    fn recount(&mut self) {
        let attached = self.handle.as_ref().is_some_and(|h| h.is_attached());
        let want = attached && self.sensitive;
        if want == self.counted {
            return;
        }
        if let Some(owner) = self.handle.as_ref().and_then(|h| h.layout_node()).and_then(|n| n.owner()) {
            owner.sensitive_content.set(owner.sensitive_content.get() + if want { 1 } else { -1 });
            self.counted = want;
        }
    }
}

impl ModifierNode for SensitiveContentNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.recount();
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        if self.counted
            && let Some(owner) = cx.layout_node().and_then(|n| n.owner())
        {
            owner.sensitive_content.set(owner.sensitive_content.get() - 1);
            self.counted = false;
        }
        self.handle = None;
    }
}

/// How often a layout would like the screen refreshed (Compose's frame rate category, or a rate).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FrameRate {
    /// A rate in Hz.
    Rate(f32),
    Category(FrameRateCategory),
}

/// Compose's FrameRateCategory.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum FrameRateCategory {
    #[default]
    Default,
    NoPreference,
    Normal,
    High,
}

impl From<f32> for FrameRate {
    fn from(rate: f32) -> FrameRate {
        FrameRate::Rate(rate)
    }
}

impl From<FrameRateCategory> for FrameRate {
    fn from(category: FrameRateCategory) -> FrameRate {
        FrameRate::Category(category)
    }
}

#[derive(PartialEq)]
struct FrameRateElement {
    rate: FrameRate,
}

impl ModifierNodeElement for FrameRateElement {
    type Node = FrameRateNode;
    fn create(&self) -> FrameRateNode {
        FrameRateNode { rate: self.rate, handle: None }
    }
    fn update(&self, node: &mut FrameRateNode) {
        node.rate = self.rate;
        node.publish();
    }
    fn name(&self) -> &'static str {
        "preferredFrameRate"
    }
}

/// The node of `Modifier.preferred_frame_rate`: its rate known to the host while attached.
pub struct FrameRateNode {
    rate: FrameRate,
    handle: Option<crate::modifier::NodeHandle>,
}

impl FrameRateNode {
    fn publish(&self) {
        let Some(node) = self.handle.as_ref().and_then(|h| h.layout_node()) else { return };
        let Some(owner) = node.owner() else { return };
        let mut rates = owner.frame_rates.borrow_mut();
        rates.retain(|(id, _)| *id != node.id());
        rates.push((node.id(), self.rate));
    }
}

impl ModifierNode for FrameRateNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.publish();
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(node) = cx.layout_node()
            && let Some(owner) = node.owner()
        {
            owner.frame_rates.borrow_mut().retain(|(id, _)| *id != node.id());
        }
        self.handle = None;
    }
}
