//! Screen readers (VoiceOver), Accessibility Inspector and the other assistive technologies on macOS read each window
//! through NSAccessibility (the AT-SPI bridge's parity): the window's semantics tree as last published (the shared
//! [`Snapshot`], read on the main thread, where AppKit asks), an element of our own for each of its nodes
//! ([`KompozElement`], an NSAccessibilityElement kept by its node's id while the node is there, so that VoiceOver's
//! cursor stays on it), under the window's view (whose `accessibilityChildren`, `accessibilityHitTest:` and
//! `accessibilityFocusedUIElement` are the tree's), each saying what [`ax`](super::ax) works out — its role and
//! subrole, label, title, value and its range, help, states, the actions it offers, a field's text, selection, lines
//! and the caret's place, a collection's rows and an item's place —, and its frame on the screen (flipped: AppKit's
//! screen goes up from the bottom).
//!
//! What an assistive technology does (a press, a slider's step, the menu, a confirmation, a page scrolled, a custom
//! action, a value or a selection set, the focus put on a node) is kept as requests for the node's semantics action,
//! told to the app core as `Event::AccessibilityRequest` (the loop woken) and performed on the host as it handles it
//! (`Platform::handled`). What changed from one published tree to the next is posted as AppKit's notifications
//! ([`ax::notices`]); announcements on the application.
//!
//! The tree is published as AppKit first asks for it (an assistive technology came: the view's children asked), at
//! once if VoiceOver is already on as the window opens; then as it changes, at most every 50 ms (500 ms while the
//! app is busy), at once after an assistive technology's action. `KOMPOZ_NSACCESSIBILITY=1` publishes it from the
//! start, `KOMPOZ_NSACCESSIBILITY=0` never (the view has no children; KOMPOZ_ATSPI's counterpart).

#![allow(unsafe_code)]

use super::app::wake_loop;
use super::ax::{self, Kind, Notice, Target, Value};
use super::text_input::{NOT_FOUND, byte_of_utf16, utf16_len};
use super::window::WindowState;
use crate::accessibility::text::text_of;
use crate::accessibility::{Snapshot, Window};
use crate::app::Event;
use block2::RcBlock;
use kompoz_graphics::Rect;
use kompoz_ui::KompozHost;
use kompoz_ui::semantics::{AccessibilityNode, SemanticsAction};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibilityAnnouncementKey, NSAccessibilityCustomAction, NSAccessibilityElement,
    NSAccessibilityPostNotification, NSAccessibilityPostNotificationWithUserInfo, NSAccessibilityPriorityKey,
    NSAccessibilityRoleDescription, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSInteger, NSNumber, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// How often a changing tree is published at most (as the AT-SPI bridge and the web's mirror do), ns; while the app
/// is busy (an animation).
const INTERVAL: i64 = 50_000_000;
const BUSY_INTERVAL: i64 = 500_000_000;

/// NSAccessibilityPriorityLevel's: what interrupts (an assertive live region), what waits its turn.
const PRIORITY_HIGH: isize = 90;
const PRIORITY_MEDIUM: isize = 50;

/// A window's accessibility (in its state): whether it is served, its published tree, its nodes' elements, the
/// assistive technologies' requests not yet performed, when to publish next.
pub struct Accessibility {
    /// `KOMPOZ_NSACCESSIBILITY` isn't 0.
    enabled: bool,
    /// An assistive technology asked (or VoiceOver was on, or `KOMPOZ_NSACCESSIBILITY=1`): the tree is published.
    active: Cell<bool>,
    tree: RefCell<Snapshot>,
    elements: RefCell<HashMap<u64, Retained<KompozElement>>>,
    requests: RefCell<Vec<(u64, SemanticsAction)>>,
    /// A tree is to be published; at once (an assistive technology just asked or acted).
    dirty: Cell<bool>,
    urgent: Cell<bool>,
    /// When the last was published, on the platform's clock.
    last: Cell<Option<i64>>,
    /// The focused field's selection as last published (the field, UTF-16 location and length).
    selection: Cell<Option<(u64, (usize, usize))>>,
    /// The focused field is multi-line (its input method's options): an `AXTextArea`.
    field_multi_line: Cell<bool>,
}

impl Accessibility {
    pub fn new() -> Accessibility {
        let forced = kompoz_runtime::environment::var("KOMPOZ_NSACCESSIBILITY");
        let forced = forced.as_deref().map(str::trim);
        Accessibility {
            enabled: forced != Some("0"),
            active: Cell::new(forced == Some("1")),
            tree: RefCell::new(Snapshot::default()),
            elements: RefCell::new(HashMap::new()),
            requests: RefCell::new(Vec::new()),
            dirty: Cell::new(forced == Some("1")),
            urgent: Cell::new(false),
            last: Cell::new(None),
            selection: Cell::new(None),
            field_multi_line: Cell::new(false),
        }
    }

    /// A tree is still to be published.
    pub fn dirty(&self) -> bool {
        self.active.get() && self.dirty.get()
    }

    /// The assistive technologies' requests since, in order.
    pub fn take_requests(&self) -> Vec<(u64, SemanticsAction)> {
        std::mem::take(&mut self.requests.borrow_mut())
    }

    pub fn set_field_multi_line(&self, multi_line: bool) {
        self.field_multi_line.set(multi_line);
    }

    /// Whether node `n` (focused or not) is a multi-line field: its input method's options say so, or its text has
    /// lines.
    fn multi_line(&self, n: &AccessibilityNode) -> bool {
        n.editable && ((n.focused && self.field_multi_line.get()) || n.value.contains('\n'))
    }
}

/// The window opened: served from the start if VoiceOver is on (it won't ask for the view's children before it
/// hears of them).
pub fn opened(state: &WindowState) {
    let a = &state.a11y;
    if a.enabled && !a.active.get() && NSWorkspace::sharedWorkspace().isVoiceOverEnabled() {
        a.active.set(true);
        a.dirty.set(true);
    }
}

/// The window closed: its elements and its tree let go (AppKit tells the window's going itself).
pub fn closed(state: &WindowState) {
    let a = &state.a11y;
    a.elements.borrow_mut().clear();
    *a.tree.borrow_mut() = Snapshot::default();
    a.requests.borrow_mut().clear();
    a.selection.set(None);
    a.last.set(None);
}

/// An assistive technology asked for the tree: served from now on (published at once, the loop woken for it).
fn asked(state: &WindowState) {
    let a = &state.a11y;
    if a.enabled && !a.active.get() {
        a.active.set(true);
        a.dirty.set(true);
        a.urgent.set(true);
        state.push(Event::AccessibilityRequest);
        wake_loop();
    }
}

/// An assistive technology's action on node `id`: kept for the app core (the loop woken), published at once after.
fn request(state: &WindowState, id: u64, action: SemanticsAction) {
    let a = &state.a11y;
    a.requests.borrow_mut().push((id, action));
    a.dirty.set(true);
    a.urgent.set(true);
    state.push(Event::AccessibilityRequest);
    wake_loop();
}

/// After a frame (`redraw`: it drew one, at `frame_time`): the host's tree published as it changed, what changed
/// posted.
pub fn publish(state: &Rc<WindowState>, host: &KompozHost, redraw: bool, frame_time: i64) {
    let a = &state.a11y;
    if !a.active.get() || state.view().is_none() {
        return;
    }
    a.dirty.set(a.dirty.get() | redraw | host.take_semantics_changed());
    let interval = if host.has_pending_work() { BUSY_INTERVAL } else { INTERVAL };
    if !a.dirty.get() || (!a.urgent.get() && a.last.get().is_some_and(|t| frame_time - t <= interval)) {
        return;
    }
    a.last.set(Some(frame_time));
    a.dirty.set(false);
    a.urgent.set(false);
    let (width, height) = state.points();
    let window = Window {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
        px_to_points: (1.0 / state.px_per_point()) as f32,
        focused: state.is_key(),
        title: state.title(),
    };
    let next = Snapshot::new(host.accessibility_tree(), window);
    let field = next.focused().filter(|&f| next.node(f).editable);
    let selection = field.zip(state.field_selection().map(|(s, _)| s));
    let old = a.tree.replace(next);
    let notices = {
        let next = a.tree.borrow();
        ax::notices(&old, &next, (a.selection.replace(selection), selection), a.field_multi_line.get())
    };
    // (The elements of nodes gone kept until they are told destroyed.)
    let gone: HashMap<u64, Retained<KompozElement>> = {
        let tree = a.tree.borrow();
        let mut elements = a.elements.borrow_mut();
        let ids: Vec<u64> = elements.keys().copied().filter(|&id| !tree.contains(id)).collect();
        ids.into_iter().filter_map(|id| elements.remove(&id).map(|e| (id, e))).collect()
    };
    post(state, &notices, &gone);
}

/// The notices posted: on the nodes' elements (one gone, its last; one made for its coming or its focus; the
/// others only if an assistive technology has it), the view, the application.
fn post(state: &Rc<WindowState>, notices: &[Notice], gone: &HashMap<u64, Retained<KompozElement>>) {
    let (Some(view), Some(shared)) = (state.view(), state.shared()) else { return };
    for n in notices {
        let target: Retained<AnyObject> = match n.target {
            Target::Node(id) => {
                let cached = state.a11y.elements.borrow().get(&id).cloned();
                let make = matches!(n.name, ax::notification::CREATED | ax::notification::FOCUSED_UI_ELEMENT_CHANGED);
                match gone.get(&id).cloned().or(cached) {
                    Some(e) => e.into(),
                    None if make && state.a11y.tree.borrow().contains(id) => element_of(state, id).into(),
                    None => continue,
                }
            }
            Target::View => view.clone().into(),
            Target::App => shared.app.clone().into(),
        };
        let name = NSString::from_str(n.name);
        match &n.announcement {
            Some((text, assertive)) => {
                let priority = if *assertive { PRIORITY_HIGH } else { PRIORITY_MEDIUM };
                let values: [Retained<AnyObject>; 2] =
                    [NSString::from_str(text).into(), NSNumber::new_isize(priority).into()];
                // SAFETY: AppKit's keys, statics valid for the process.
                let keys = unsafe { [NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey] };
                let info = NSDictionary::<NSString, AnyObject>::from_retained_objects(&keys, &values);
                // SAFETY: an announcement's user info as AppKit documents it (its text, its priority).
                unsafe { NSAccessibilityPostNotificationWithUserInfo(&target, &name, Some(&info)) };
            }
            // SAFETY: the element (an accessibility element, the view, the application) and a notification's name.
            None => unsafe { NSAccessibilityPostNotification(&target, &name) },
        }
    }
}

/// The element of node `id` (made as it is first asked for, kept while the node is there).
fn element_of(state: &Rc<WindowState>, id: u64) -> Retained<KompozElement> {
    if let Some(e) = state.a11y.elements.borrow().get(&id) {
        return e.clone();
    }
    let mtm = MainThreadMarker::new().expect("AppKit asks on the main thread");
    let e = KompozElement::new(mtm, state, id);
    state.a11y.elements.borrow_mut().insert(id, e.clone());
    e
}

/// The elements of `ids`, as an array AppKit takes.
fn array_of(state: &Rc<WindowState>, ids: &[u64]) -> Retained<NSArray> {
    let elements: Vec<Retained<AnyObject>> = ids.iter().map(|&id| element_of(state, id).into()).collect();
    NSArray::from_retained_slice(&elements)
}

/// A rectangle of the window's, px, on the screen (AppKit's points, up from the bottom of the main screen).
fn on_screen(state: &WindowState, r: Rect) -> NSRect {
    let (Some(view), Some(window)) = (state.view(), state.window()) else { return NSRect::ZERO };
    let k = 1.0 / state.px_per_point();
    let in_view = NSRect::new(
        NSPoint::new(r.left as f64 * k, r.top as f64 * k),
        NSSize::new(r.width().max(0.0) as f64 * k, r.height().max(0.0) as f64 * k),
    );
    window.convertRectToScreen(view.convertRect_toView(in_view, None))
}

/// A point of the screen in the window, px.
fn in_window(state: &WindowState, point: NSPoint) -> Option<(f32, f32)> {
    let (view, window) = (state.view()?, state.window()?);
    let p = view.convertPoint_fromView(window.convertPointFromScreen(point), None);
    let k = state.px_per_point();
    Some(((p.x * k) as f32, (p.y * k) as f32))
}

// ---- The view's part ----------------------------------------------------------------------------------------------

/// The view's children: the elements of the tree's top nodes (AppKit asking: an assistive technology came).
pub fn view_children(state: &Rc<WindowState>) -> Retained<NSArray> {
    asked(state);
    if !state.a11y.active.get() {
        return NSArray::new();
    }
    let top = state.a11y.tree.borrow().children(None).to_vec();
    array_of(state, &top)
}

/// The element at a point of the screen (none: the view's own place).
pub fn view_hit_test(state: &Rc<WindowState>, point: NSPoint) -> Option<Retained<AnyObject>> {
    asked(state);
    let (x, y) = in_window(state, point)?;
    let id = ax::hit_test(&state.a11y.tree.borrow(), None, x, y)?;
    Some(element_of(state, id).into())
}

/// The element of the focused node (none: the view itself).
pub fn view_focused(state: &Rc<WindowState>) -> Option<Retained<AnyObject>> {
    asked(state);
    let id = state.a11y.tree.borrow().focused()?;
    Some(element_of(state, id).into())
}

// ---- The nodes' elements ------------------------------------------------------------------------------------------

/// An element's state: its window's, its node's id.
pub struct ElementIvars {
    state: Weak<WindowState>,
    id: u64,
}

/// What an element's methods read: its window's state, the tree, its node, what it is.
struct Read<'a> {
    state: &'a Rc<WindowState>,
    tree: &'a Snapshot,
    n: &'a AccessibilityNode,
    kind: Kind,
}

impl Read<'_> {
    /// The focused field's selection (UTF-16) where this is the focused field.
    fn field_selection(&self) -> Option<(usize, usize)> {
        if !(self.n.focused && self.n.editable) {
            return None;
        }
        self.state.field_selection().map(|(s, _)| s)
    }

    fn text(&self) -> String {
        text_of(self.n)
    }

    /// A UTF-16 range of its text clamped to it.
    fn clamp(&self, range: NSRange) -> (usize, usize) {
        let len = utf16_len(&self.text());
        let start = range.location.min(len);
        (start, range.length.min(len - start))
    }
}

fn string(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn ns_range((location, length): (usize, usize)) -> NSRange {
    NSRange::new(location, length)
}

define_class!(
    // SAFETY: NSAccessibilityElement's subclassing: its NSAccessibility getters, setters and actions overridden
    // (calling super's for what isn't ours); no Drop.
    #[unsafe(super(NSAccessibilityElement, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KompozAccessibilityElement"]
    #[ivars = ElementIvars]
    pub struct KompozElement;

    impl KompozElement {
        /// An element while its node is there (one an assistive technology keeps after it went: none).
        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            self.read(|_| true).unwrap_or(false)
        }

        #[unsafe(method_id(accessibilityParent))]
        fn accessibility_parent(&self) -> Option<Retained<AnyObject>> {
            self.parent()
        }

        #[unsafe(method_id(accessibilityChildren))]
        fn accessibility_children(&self) -> Option<Retained<NSArray>> {
            self.children()
        }

        /// A tab group's, a list's selected children.
        #[unsafe(method_id(accessibilitySelectedChildren))]
        fn accessibility_selected_children(&self) -> Option<Retained<NSArray>> {
            self.selected_children()
        }

        /// Where it is on the screen.
        #[unsafe(method(accessibilityFrame))]
        fn accessibility_frame(&self) -> NSRect {
            self.read(|r| on_screen(r.state, r.n.bounds)).unwrap_or(NSRect::ZERO)
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Option<Retained<NSString>> {
            self.read(|r| string(r.kind.role))
        }

        #[unsafe(method_id(accessibilitySubrole))]
        fn accessibility_subrole(&self) -> Option<Retained<NSString>> {
            self.read(|r| r.kind.subrole.map(string)).flatten()
        }

        /// AppKit's description of its role and subrole (localized); ours where AppKit's would say another thing
        /// (a heading, a dialog's or a carousel's group: AppKit knows no such).
        #[unsafe(method_id(accessibilityRoleDescription))]
        fn accessibility_role_description(&self) -> Option<Retained<NSString>> {
            self.read(|r| match r.kind.description {
                Some(ours) => Some(string(ours)),
                None => NSAccessibilityRoleDescription(&string(r.kind.role), r.kind.subrole.map(string).as_deref()),
            })
            .flatten()
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Option<Retained<NSString>> {
            self.read(|r| ax::label_of(r.n, &r.kind).map(|l| string(&l))).flatten()
        }

        #[unsafe(method_id(accessibilityTitle))]
        fn accessibility_title(&self) -> Option<Retained<NSString>> {
            self.read(|r| ax::title_of(r.n, &r.kind).map(|t| string(&t))).flatten()
        }

        #[unsafe(method_id(accessibilityHelp))]
        fn accessibility_help(&self) -> Option<Retained<NSString>> {
            self.read(|r| ax::help_of(r.n).map(|h| string(&h))).flatten()
        }

        #[unsafe(method_id(accessibilityIdentifier))]
        fn accessibility_identifier(&self) -> Option<Retained<NSString>> {
            self.read(|r| r.n.test_tag.as_deref().map(string)).flatten()
        }

        #[unsafe(method_id(accessibilityValue))]
        fn accessibility_value(&self) -> Option<Retained<AnyObject>> {
            self.read(|r| {
                ax::value_of(r.n, &r.kind).map(|v| -> Retained<AnyObject> {
                    match v {
                        Value::Number(x) => NSNumber::new_f64(x).into(),
                        Value::Text(t) => string(&t).into(),
                    }
                })
            })
            .flatten()
        }

        /// A field's text set, a slider's value.
        #[unsafe(method(setAccessibilityValue:))]
        fn set_accessibility_value(&self, value: Option<&AnyObject>) {
            let Some(value) = value else { return };
            let v = if let Some(s) = value.downcast_ref::<NSString>() {
                Value::Text(s.to_string())
            } else if let Some(n) = value.downcast_ref::<NSNumber>() {
                Value::Number(n.as_f64())
            } else {
                return;
            };
            self.act(|r| ax::set_value(r.n, &r.kind, &v));
        }

        #[unsafe(method_id(accessibilityValueDescription))]
        fn accessibility_value_description(&self) -> Option<Retained<NSString>> {
            self.read(|r| ax::value_description_of(r.n).map(|d| string(&d))).flatten()
        }

        #[unsafe(method_id(accessibilityMinValue))]
        fn accessibility_min_value(&self) -> Option<Retained<AnyObject>> {
            self.read(|r| ax::bounds_of(r.n).map(|(min, _)| NSNumber::new_f64(min).into())).flatten()
        }

        #[unsafe(method_id(accessibilityMaxValue))]
        fn accessibility_max_value(&self) -> Option<Retained<AnyObject>> {
            self.read(|r| ax::bounds_of(r.n).map(|(_, max)| NSNumber::new_f64(max).into())).flatten()
        }

        // ---- States ----

        #[unsafe(method(isAccessibilityEnabled))]
        fn is_accessibility_enabled(&self) -> bool {
            self.read(|r| r.n.enabled).unwrap_or(false)
        }

        #[unsafe(method(isAccessibilityFocused))]
        fn is_accessibility_focused(&self) -> bool {
            self.read(|r| r.n.focused).unwrap_or(false)
        }

        /// The keyboard's focus put on it (its focus action), or taken away (nothing: another node takes it).
        #[unsafe(method(setAccessibilityFocused:))]
        fn set_accessibility_focused(&self, focused: bool) {
            if focused {
                self.act(|r| (!r.n.focused).then_some(SemanticsAction::Focus));
            }
        }

        #[unsafe(method(isAccessibilitySelected))]
        fn is_accessibility_selected(&self) -> bool {
            self.read(|r| ax::selected_of(r.n, &r.kind)).unwrap_or(false)
        }

        /// Selected (a tab, an option: its click); unselecting is choosing another.
        #[unsafe(method(setAccessibilitySelected:))]
        fn set_accessibility_selected(&self, selected: bool) {
            self.act(|r| (selected && !ax::selected_of(r.n, &r.kind)).then_some(SemanticsAction::Click));
        }

        #[unsafe(method(isAccessibilityExpanded))]
        fn is_accessibility_expanded(&self) -> bool {
            self.read(|r| r.n.expanded.unwrap_or(false)).unwrap_or(false)
        }

        #[unsafe(method(setAccessibilityExpanded:))]
        fn set_accessibility_expanded(&self, expanded: bool) {
            self.act(|r| match r.n.expanded {
                Some(now) if now != expanded => {
                    Some(if expanded { SemanticsAction::Expand } else { SemanticsAction::Collapse })
                }
                _ => None,
            });
        }

        #[unsafe(method(isAccessibilityRequired))]
        fn is_accessibility_required(&self) -> bool {
            false
        }

        // ---- Actions ----

        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_perform_press(&self) -> bool {
            self.perform(ax::action::PRESS)
        }

        #[unsafe(method(accessibilityPerformIncrement))]
        fn accessibility_perform_increment(&self) -> bool {
            self.perform(ax::action::INCREMENT)
        }

        #[unsafe(method(accessibilityPerformDecrement))]
        fn accessibility_perform_decrement(&self) -> bool {
            self.perform(ax::action::DECREMENT)
        }

        /// Its menu (a long click's).
        #[unsafe(method(accessibilityPerformShowMenu))]
        fn accessibility_perform_show_menu(&self) -> bool {
            self.perform(ax::action::SHOW_MENU)
        }

        #[unsafe(method(accessibilityPerformConfirm))]
        fn accessibility_perform_confirm(&self) -> bool {
            self.perform(ax::action::CONFIRM)
        }

        #[unsafe(method(accessibilityPerformCancel))]
        fn accessibility_perform_cancel(&self) -> bool {
            self.perform(ax::action::CANCEL)
        }

        /// The actions it offers by name (AppKit's, and a scroll area's pages), for the assistive technologies that
        /// ask by name (and VoiceOver's scrolling by pages, which has no method of the protocol's).
        #[unsafe(method_id(accessibilityActionNames))]
        fn accessibility_action_names(&self) -> Retained<NSArray<NSString>> {
            let names = self.read(|r| ax::actions_of(r.n, &r.kind)).unwrap_or_default();
            let names: Vec<Retained<NSString>> = names.into_iter().map(string).collect();
            NSArray::from_retained_slice(&names)
        }

        #[unsafe(method(accessibilityPerformAction:))]
        fn accessibility_perform_action(&self, name: &NSString) {
            self.perform(&name.to_string());
        }

        /// What an action does, in AppKit's words (ours for the pages it has none for).
        #[unsafe(method_id(accessibilityActionDescription:))]
        fn accessibility_action_description(&self, name: &NSString) -> Option<Retained<NSString>> {
            let described = objc2_app_kit::NSAccessibilityActionDescription(name).filter(|d| d.length() > 0);
            described.or_else(|| {
                let ours = match name.to_string().as_str() {
                    ax::action::SCROLL_UP_BY_PAGE => "scroll up by a page",
                    ax::action::SCROLL_DOWN_BY_PAGE => "scroll down by a page",
                    ax::action::SCROLL_LEFT_BY_PAGE => "scroll left by a page",
                    ax::action::SCROLL_RIGHT_BY_PAGE => "scroll right by a page",
                    _ => return None,
                };
                Some(string(ours))
            })
        }

        /// Its custom actions (`SemanticsAction::CustomAction`'s), by their labels.
        #[unsafe(method_id(accessibilityCustomActions))]
        fn accessibility_custom_actions(&self) -> Option<Retained<NSArray<NSAccessibilityCustomAction>>> {
            self.custom_actions()
        }

        /// The attributes it has: AppKit's (its properties'), and WebKit's AXInvalid where its input is wrong.
        #[unsafe(method_id(accessibilityAttributeNames))]
        fn accessibility_attribute_names(&self) -> Retained<NSArray<NSString>> {
            self.attribute_names()
        }

        #[unsafe(method_id(accessibilityAttributeValue:))]
        fn accessibility_attribute_value(&self, name: &NSString) -> Option<Retained<AnyObject>> {
            self.attribute_value(name)
        }

        // ---- A field's text ----

        #[unsafe(method(accessibilityNumberOfCharacters))]
        fn accessibility_number_of_characters(&self) -> NSInteger {
            self.read(|r| utf16_len(&r.text()) as NSInteger).unwrap_or(0)
        }

        /// The focused field's selection (as its input methods see it); another field's none, at its start.
        #[unsafe(method(accessibilitySelectedTextRange))]
        fn accessibility_selected_text_range(&self) -> NSRange {
            self.read(|r| ns_range(r.field_selection().unwrap_or((0, 0)))).unwrap_or(NSRange::new(NOT_FOUND, 0))
        }

        #[unsafe(method_id(accessibilitySelectedText))]
        fn accessibility_selected_text(&self) -> Option<Retained<NSString>> {
            self.read(|r| {
                let (location, length) = r.field_selection()?;
                let text = r.text();
                let (a, b) = (byte_of_utf16(&text, location), byte_of_utf16(&text, location + length));
                Some(string(&text[a..b]))
            })
            .flatten()
        }

        /// The selection set (its SetSelection action, by the text's bytes).
        #[unsafe(method(setAccessibilitySelectedTextRange:))]
        fn set_accessibility_selected_text_range(&self, range: NSRange) {
            self.act(|r| {
                if !r.n.editable {
                    return None;
                }
                let (location, length) = r.clamp(range);
                let text = &r.n.value;
                let (start, end) = (byte_of_utf16(text, location), byte_of_utf16(text, location + length));
                Some(SemanticsAction::SetSelection {
                    start: start as i32,
                    end: end as i32,
                    relative_to_original_text: true,
                })
            });
        }

        /// What shows of its text: all of it (a field's text is laid out whole).
        #[unsafe(method(accessibilityVisibleCharacterRange))]
        fn accessibility_visible_character_range(&self) -> NSRange {
            self.read(|r| NSRange::new(0, utf16_len(&r.text()))).unwrap_or(NSRange::new(0, 0))
        }

        /// The line the caret is on (0 outside the focused field).
        #[unsafe(method(accessibilityInsertionPointLineNumber))]
        fn accessibility_insertion_point_line_number(&self) -> NSInteger {
            self.read(|r| {
                let caret = r.field_selection().map_or(0, |(l, n)| l + n);
                ax::line_of(&r.text(), caret) as NSInteger
            })
            .unwrap_or(0)
        }

        #[unsafe(method(accessibilityLineForIndex:))]
        fn accessibility_line_for_index(&self, index: NSInteger) -> NSInteger {
            self.read(|r| ax::line_of(&r.text(), index.max(0) as usize) as NSInteger).unwrap_or(0)
        }

        #[unsafe(method(accessibilityRangeForLine:))]
        fn accessibility_range_for_line(&self, line: NSInteger) -> NSRange {
            self.read(|r| ax::line_range(&r.text(), line.max(0) as usize).map(ns_range))
                .flatten()
                .unwrap_or(NSRange::new(NOT_FOUND, 0))
        }

        #[unsafe(method_id(accessibilityStringForRange:))]
        fn accessibility_string_for_range(&self, range: NSRange) -> Option<Retained<NSString>> {
            self.read(|r| {
                let (location, length) = r.clamp(range);
                let text = r.text();
                let (a, b) = (byte_of_utf16(&text, location), byte_of_utf16(&text, location + length));
                string(&text[a..b])
            })
        }

        /// Where a range of its text is on the screen: the caret's box in the focused field, else the field's (the
        /// text's layout isn't the platform's to ask, as AT-SPI's character extents).
        #[unsafe(method(accessibilityFrameForRange:))]
        fn accessibility_frame_for_range(&self, _range: NSRange) -> NSRect {
            self.read(|r| {
                let caret = r.field_selection().and_then(|_| r.state.field_selection()).map(|(_, area)| area);
                on_screen(r.state, caret.unwrap_or(r.n.bounds))
            })
            .unwrap_or(NSRect::ZERO)
        }

        /// The range at a point of the screen: the caret where it is in the focused field (as the input methods'
        /// `characterIndexForPoint:`), none elsewhere.
        #[unsafe(method(accessibilityRangeForPosition:))]
        fn accessibility_range_for_position(&self, point: NSPoint) -> NSRange {
            self.read(|r| {
                let ((location, length), area) = r.field_selection().and_then(|_| r.state.field_selection())?;
                let (x, y) = in_window(r.state, point)?;
                let inside = x >= area.left && x <= area.right && y >= area.top && y <= area.bottom;
                inside.then(|| NSRange::new(location + length, 0))
            })
            .flatten()
            .unwrap_or(NSRange::new(NOT_FOUND, 0))
        }

        // ---- Collections ----

        #[unsafe(method(accessibilityRowCount))]
        fn accessibility_row_count(&self) -> NSInteger {
            self.read(|r| ax::counts_of(r.n).and_then(|(rows, _)| rows).map_or(0, |v| v as NSInteger)).unwrap_or(0)
        }

        #[unsafe(method(accessibilityColumnCount))]
        fn accessibility_column_count(&self) -> NSInteger {
            self.read(|r| ax::counts_of(r.n).and_then(|(_, columns)| columns).map_or(0, |v| v as NSInteger))
                .unwrap_or(0)
        }

        /// A row's place in its list.
        #[unsafe(method(accessibilityIndex))]
        fn accessibility_index(&self) -> NSInteger {
            self.read(|r| ax::place_of(r.tree, r.n).map_or(0, |(row, ..)| row as NSInteger)).unwrap_or(0)
        }

        #[unsafe(method(accessibilityRowIndexRange))]
        fn accessibility_row_index_range(&self) -> NSRange {
            self.read(|r| ax::place_of(r.tree, r.n).map(|(row, span, ..)| NSRange::new(row, span)))
                .flatten()
                .unwrap_or(NSRange::new(NOT_FOUND, 0))
        }

        #[unsafe(method(accessibilityColumnIndexRange))]
        fn accessibility_column_index_range(&self) -> NSRange {
            self.read(|r| ax::place_of(r.tree, r.n).map(|(.., column, span)| NSRange::new(column, span)))
                .flatten()
                .unwrap_or(NSRange::new(NOT_FOUND, 0))
        }

        // ---- Where things are ----

        /// The deepest element of its subtree at a point of the screen (itself where none of its children is).
        #[unsafe(method_id(accessibilityHitTest:))]
        fn accessibility_hit_test(&self, point: NSPoint) -> Option<Retained<AnyObject>> {
            self.hit_test(point)
        }

        #[unsafe(method_id(accessibilityFocusedUIElement))]
        fn accessibility_focused_ui_element(&self) -> Option<Retained<AnyObject>> {
            self.focused_ui_element()
        }

        #[unsafe(method_id(accessibilityWindow))]
        fn accessibility_window(&self) -> Option<Retained<AnyObject>> {
            self.window()
        }

        #[unsafe(method_id(accessibilityTopLevelUIElement))]
        fn accessibility_top_level_ui_element(&self) -> Option<Retained<AnyObject>> {
            self.top_level_ui_element()
        }
    }

    // SAFETY: NSObject's protocol, nothing of its own.
    unsafe impl NSObjectProtocol for KompozElement {}
);

impl KompozElement {
    fn new(mtm: MainThreadMarker, state: &Rc<WindowState>, id: u64) -> Retained<KompozElement> {
        let this = KompozElement::alloc(mtm).set_ivars(ElementIvars { state: Rc::downgrade(state), id });
        // SAFETY: NSAccessibilityElement's `init` (NSObject's) on a freshly allocated element with its ivars set.
        unsafe { msg_send![super(this), init] }
    }

    /// `f` of its node as published (none: the node or the window gone).
    fn read<R>(&self, f: impl FnOnce(&Read<'_>) -> R) -> Option<R> {
        let state = self.ivars().state.upgrade()?;
        let tree = state.a11y.tree.borrow();
        let n = tree.get(self.ivars().id)?;
        let kind = ax::kind_of(&tree, n, state.a11y.multi_line(n));
        Some(f(&Read { state: &state, tree: &tree, n, kind }))
    }

    /// The semantics action `f` says, requested of its node (none: nothing to do); whether one was.
    fn act(&self, f: impl FnOnce(&Read<'_>) -> Option<SemanticsAction>) -> bool {
        let Some(action) = self.read(f).flatten() else { return false };
        let Some(state) = self.ivars().state.upgrade() else { return false };
        request(&state, self.ivars().id, action);
        true
    }

    fn parent(&self) -> Option<Retained<AnyObject>> {
        let state = self.ivars().state.upgrade()?;
        let parent = state.a11y.tree.borrow().get(self.ivars().id).map(|n| n.parent)?;
        match parent {
            Some(p) => Some(element_of(&state, p).into()),
            None => state.view().map(Into::into),
        }
    }

    fn children(&self) -> Option<Retained<NSArray>> {
        let state = self.ivars().state.upgrade()?;
        let kids = state.a11y.tree.borrow().children(Some(self.ivars().id)).to_vec();
        Some(array_of(&state, &kids))
    }

    fn selected_children(&self) -> Option<Retained<NSArray>> {
        let state = self.ivars().state.upgrade()?;
        let selected: Vec<u64> = self.read(|r| {
            r.tree
                .children(Some(r.n.id))
                .iter()
                .copied()
                .filter(|&k| {
                    let kid = r.tree.node(k);
                    ax::selected_of(kid, &ax::kind_of(r.tree, kid, false))
                })
                .collect()
        })?;
        Some(array_of(&state, &selected))
    }

    fn custom_actions(&self) -> Option<Retained<NSArray<NSAccessibilityCustomAction>>> {
        let labels = self.read(|r| r.n.custom_actions.clone())?;
        let actions: Vec<Retained<NSAccessibilityCustomAction>> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let (state, id) = (self.ivars().state.clone(), self.ivars().id);
                let handler = RcBlock::new(move || -> Bool {
                    let Some(state) = state.upgrade() else { return Bool::NO };
                    if !state.a11y.tree.borrow().contains(id) {
                        return Bool::NO;
                    }
                    request(&state, id, SemanticsAction::CustomAction(i));
                    Bool::YES
                });
                NSAccessibilityCustomAction::initWithName_handler(
                    NSAccessibilityCustomAction::alloc(),
                    &string(label),
                    Some(&handler),
                )
            })
            .collect();
        Some(NSArray::from_retained_slice(&actions))
    }

    fn attribute_names(&self) -> Retained<NSArray<NSString>> {
        // SAFETY: NSObject's (NSAccessibility) own, the names AppKit derives from the properties implemented.
        let names: Retained<NSArray<NSString>> = unsafe { msg_send![super(self), accessibilityAttributeNames] };
        if self.read(|r| r.n.error.is_some()) != Some(true) {
            return names;
        }
        let mut all: Vec<Retained<NSString>> = names.to_vec();
        all.push(string(ax::INVALID_ATTRIBUTE));
        NSArray::from_retained_slice(&all)
    }

    fn attribute_value(&self, name: &NSString) -> Option<Retained<AnyObject>> {
        if name.to_string() == ax::INVALID_ATTRIBUTE {
            let invalid = self.read(|r| r.n.error.is_some()).unwrap_or(false);
            return Some(string(if invalid { "true" } else { "false" }).into());
        }
        // SAFETY: NSObject's (NSAccessibility) own, with the name AppKit gave this override.
        unsafe { msg_send![super(self), accessibilityAttributeValue: name] }
    }

    fn hit_test(&self, point: NSPoint) -> Option<Retained<AnyObject>> {
        let state = self.ivars().state.upgrade()?;
        let (x, y) = in_window(&state, point)?;
        let id = ax::hit_test(&state.a11y.tree.borrow(), Some(self.ivars().id), x, y)?;
        Some(element_of(&state, id).into())
    }

    fn focused_ui_element(&self) -> Option<Retained<AnyObject>> {
        let state = self.ivars().state.upgrade()?;
        let id = state.a11y.tree.borrow().focused()?;
        Some(element_of(&state, id).into())
    }

    fn window(&self) -> Option<Retained<AnyObject>> {
        self.ivars().state.upgrade()?.window().map(Into::into)
    }

    fn top_level_ui_element(&self) -> Option<Retained<AnyObject>> {
        self.ivars().state.upgrade()?.window().map(Into::into)
    }

    /// AppKit's action `name`, if the node offers it.
    fn perform(&self, name: &str) -> bool {
        self.act(|r| ax::action_for(r.n, &r.kind, name))
    }
}

#[cfg(test)]
mod tests {
    use super::ax::{action, notification, role, subrole};
    use objc2_app_kit::*;

    /// The names `ax` says are AppKit's constants' strings.
    #[test]
    fn the_names_are_appkits() {
        // SAFETY: AppKit's constants, statics valid for the process.
        let pairs: Vec<(&str, &objc2_foundation::NSString)> = unsafe {
            vec![
                (role::BUTTON, NSAccessibilityButtonRole),
                (role::CHECK_BOX, NSAccessibilityCheckBoxRole),
                (role::RADIO_BUTTON, NSAccessibilityRadioButtonRole),
                (role::RADIO_GROUP, NSAccessibilityRadioGroupRole),
                (role::TAB_GROUP, NSAccessibilityTabGroupRole),
                (role::SLIDER, NSAccessibilitySliderRole),
                (role::PROGRESS_INDICATOR, NSAccessibilityProgressIndicatorRole),
                (role::INCREMENTOR, NSAccessibilityIncrementorRole),
                (role::POP_UP_BUTTON, NSAccessibilityPopUpButtonRole),
                (role::TEXT_FIELD, NSAccessibilityTextFieldRole),
                (role::TEXT_AREA, NSAccessibilityTextAreaRole),
                (role::STATIC_TEXT, NSAccessibilityStaticTextRole),
                (role::IMAGE, NSAccessibilityImageRole),
                (role::LINK, NSAccessibilityLinkRole),
                (role::LIST, NSAccessibilityListRole),
                (role::GRID, NSAccessibilityGridRole),
                (role::ROW, NSAccessibilityRowRole),
                (role::CELL, NSAccessibilityCellRole),
                (role::GROUP, NSAccessibilityGroupRole),
                (role::SCROLL_AREA, NSAccessibilityScrollAreaRole),
                (subrole::SWITCH, NSAccessibilitySwitchSubrole),
                (subrole::TAB_BUTTON, NSAccessibilityTabButtonSubrole),
                (subrole::SECURE_TEXT_FIELD, NSAccessibilitySecureTextFieldSubrole),
                (subrole::TABLE_ROW, NSAccessibilityTableRowSubrole),
                (subrole::CONTENT_LIST, NSAccessibilityContentListSubrole),
                (action::PRESS, NSAccessibilityPressAction),
                (action::INCREMENT, NSAccessibilityIncrementAction),
                (action::DECREMENT, NSAccessibilityDecrementAction),
                (action::SHOW_MENU, NSAccessibilityShowMenuAction),
                (action::CONFIRM, NSAccessibilityConfirmAction),
                (action::CANCEL, NSAccessibilityCancelAction),
                (notification::FOCUSED_UI_ELEMENT_CHANGED, NSAccessibilityFocusedUIElementChangedNotification),
                (notification::VALUE_CHANGED, NSAccessibilityValueChangedNotification),
                (notification::SELECTED_TEXT_CHANGED, NSAccessibilitySelectedTextChangedNotification),
                (notification::SELECTED_CHILDREN_CHANGED, NSAccessibilitySelectedChildrenChangedNotification),
                (notification::TITLE_CHANGED, NSAccessibilityTitleChangedNotification),
                (notification::LAYOUT_CHANGED, NSAccessibilityLayoutChangedNotification),
                (notification::CREATED, NSAccessibilityCreatedNotification),
                (notification::UI_ELEMENT_DESTROYED, NSAccessibilityUIElementDestroyedNotification),
                (notification::ANNOUNCEMENT_REQUESTED, NSAccessibilityAnnouncementRequestedNotification),
            ]
        };
        for (ours, appkits) in pairs {
            assert_eq!(ours, appkits.to_string(), "AppKit's {appkits}");
        }
    }
}
