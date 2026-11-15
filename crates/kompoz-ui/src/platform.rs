//! What the UI asks of the platform and what it tells the UI (Compose's `androidx.compose.ui.platform`): the
//! platform's callbacks (C++'s `KompozHost::Callbacks`, each doing nothing by default, so a headless host works),
//! the view configuration, the ui composition locals the host provides at the root, and the modifiers whose effect
//! is the window's (`keep_screen_on`, `sensitive_content`, `preferred_frame_rate`).

use crate::graphics_context::GraphicsContext;
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use kompoz_graphics::{Density, ImageBitmap, IntSize, LayoutDirection, Rect, Size};
use kompoz_runtime::{KompozitionLocal, MutableState, mutable_state_of};
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
    /// An asset's bytes (the app's assets, or a file next to the binary).
    fn read_asset(&self, path: &str) -> Option<Vec<u8>> {
        let _ = path;
        None
    }
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

/// Opens links (Compose's UriHandler).
#[derive(Clone, PartialEq, Default)]
pub struct UriHandler(pub(crate) PlatformHandle);

impl UriHandler {
    pub fn open_uri(&self, uri: &str) {
        self.0.get().open_uri(uri);
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

/// What the system UI covers (C++'s SystemInsets, Android's WindowInsetsCompat types).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
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
    /// The keyboard sliding in or out: `ime` follows it frame by frame from `ime_animation_source` to
    /// `ime_animation_target`.
    pub ime_animating: bool,
    pub ime_visible: bool,
    pub ime_animation_source: Insets,
    pub ime_animation_target: Insets,
}

// ---------------------------------------------------------------------------------------------------------------
// The locals

/// Pixels per dp and the font scale (Compose's LocalDensity).
pub static LocalDensity: KompozitionLocal<Density> = KompozitionLocal::new(Density::default);
/// Which way layouts go (Compose's LocalLayoutDirection).
pub static LocalLayoutDirection: KompozitionLocal<LayoutDirection> = KompozitionLocal::new(|| LayoutDirection::Ltr);
/// The gesture thresholds (Compose's LocalViewConfiguration).
pub static LocalViewConfiguration: KompozitionLocal<ViewConfiguration> =
    KompozitionLocal::new(ViewConfiguration::default);
/// The window (Compose's LocalWindowInfo).
pub static LocalWindowInfo: KompozitionLocal<WindowInfo> = KompozitionLocal::new(WindowInfo::default);
/// The input mode (Compose's LocalInputModeManager).
pub static LocalInputModeManager: KompozitionLocal<InputModeManager> = KompozitionLocal::new(InputModeManager::default);
/// Haptic feedback (Compose's LocalHapticFeedback).
pub static LocalHapticFeedback: KompozitionLocal<HapticFeedback> = KompozitionLocal::new(HapticFeedback::default);
/// The clipboard (Compose's LocalClipboard).
pub static LocalClipboard: KompozitionLocal<Clipboard> = KompozitionLocal::new(Clipboard::default);
/// Opening links (Compose's LocalUriHandler).
pub static LocalUriHandler: KompozitionLocal<UriHandler> = KompozitionLocal::new(UriHandler::default);
/// The software keyboard (Compose's LocalSoftwareKeyboardController).
pub static LocalSoftwareKeyboardController: KompozitionLocal<SoftwareKeyboardController> =
    KompozitionLocal::new(SoftwareKeyboardController::default);
/// The focus manager (Compose's LocalFocusManager).
pub static LocalFocusManager: KompozitionLocal<crate::focus::FocusManager> =
    KompozitionLocal::new(crate::focus::FocusManager::default);
/// The system's theme (Compose desktop's LocalSystemTheme; what `is_system_in_dark_theme` reads).
pub static LocalSystemTheme: KompozitionLocal<SystemTheme> = KompozitionLocal::new(SystemTheme::default);
/// What the system UI covers (what the window insets read).
pub static LocalSystemInsets: KompozitionLocal<SystemInsets> = KompozitionLocal::new(SystemInsets::default);
/// The platform's callbacks, for what calls them directly (images, assets, autofill).
pub static LocalPlatform: KompozitionLocal<PlatformHandle> = KompozitionLocal::new(PlatformHandle::default);

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
