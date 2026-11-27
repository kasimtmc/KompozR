//! The app's view and window classes (C++'s KompozView and its window delegate), defined with objc2: the view is
//! the window's content and delegate — backed by a CAMetalLayer the device presents to, flipped (y down, as the
//! app's points are), taking the mouse, the trackpad's gestures, the tablet, the keyboard and the input methods
//! (NSTextInputClient), drops (NSDraggingDestination) and the drags it starts (NSDraggingSource), ticked by its display
//! link — and hands each to its window's state ([`WindowState`], window.rs), where it becomes the app's events. The
//! window class only lets a borderless window be key (an undecorated `Window`), and a window that mustn't take the
//! focus never be.
//!
//! Every method here runs on the main thread, called by AppKit inside the loop's handing of an event (or a timer's,
//! a display link's); none holds a borrow of the state across a call back into AppKit.

#![allow(unsafe_code)]

use super::text_input::NOT_FOUND;
use super::window::{MouseKind, WindowState};
use crate::app::MouseButton;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSDragOperation, NSDraggingContext, NSDraggingDestination, NSDraggingInfo, NSDraggingSession,
    NSDraggingSource, NSEvent, NSResponder, NSScreen, NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSNotification, NSObjectProtocol, NSPoint, NSRange,
    NSRangePointer, NSRect, NSSize, NSString, NSUInteger,
};
use objc2_quartz_core::{CALayer, CAMetalLayer};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// The view's state: its window's, and its tracking area.
pub struct ViewIvars {
    state: Weak<WindowState>,
    tracking: RefCell<Option<Retained<NSTrackingArea>>>,
}

define_class!(
    // SAFETY: NSView's subclassing rules are kept: its overridden methods call super where AppKit needs it
    // (`setFrameSize:`); the view implements no Drop (its ivars drop themselves).
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KompozView"]
    #[ivars = ViewIvars]
    pub struct KompozView;

    impl KompozView {
        /// Its points top down, as the app's.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        /// A click on an inactive window is the app's too (C++'s).
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        /// The layer AppKit backs it with: a CAMetalLayer, what the Metal device presents to.
        #[unsafe(method_id(makeBackingLayer))]
        fn make_backing_layer(&self) -> Retained<CALayer> {
            Retained::into_super(CAMetalLayer::new())
        }

        /// AppKit asks for its contents (it was hidden, its layer's contents lost): drawn again.
        #[unsafe(method(wantsUpdateLayer))]
        fn wants_update_layer(&self) -> bool {
            true
        }

        #[unsafe(method(updateLayer))]
        fn update_layer(&self) {
            if let Some(state) = self.state() {
                state.exposed();
            }
        }

        /// Its size changed (a resize, full screen, a zoom): its drawable's with it.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: NSView's own `setFrameSize:`, as AppKit needs it called.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            if let Some(state) = self.state() {
                state.frame_changed(self);
            }
        }

        /// Its scale (the display's backing scale) changed.
        #[unsafe(method(viewDidChangeBackingProperties))]
        fn view_did_change_backing_properties(&self) {
            if let Some(state) = self.state() {
                state.frame_changed(self);
            }
        }

        /// The appearance (dark or light) changed.
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            if let Some(state) = self.state() {
                state.appearance_changed();
            }
        }

        /// Its tracking area made again for its bounds: the mouse coming and going, moving, its cursor, whether the
        /// window is key or not.
        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            if let Some(old) = self.ivars().tracking.borrow_mut().take() {
                self.removeTrackingArea(&old);
            }
            let options = NSTrackingAreaOptions::MouseEnteredAndExited
                | NSTrackingAreaOptions::MouseMoved
                | NSTrackingAreaOptions::CursorUpdate
                | NSTrackingAreaOptions::ActiveAlways
                | NSTrackingAreaOptions::InVisibleRect
                | NSTrackingAreaOptions::EnabledDuringMouseDrag;
            let owner: &AnyObject = self;
            // SAFETY: the view is the area's owner (it implements the mouse methods); no user info.
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    self.bounds(),
                    options,
                    Some(owner),
                    None,
                )
            };
            self.addTrackingArea(&area);
            *self.ivars().tracking.borrow_mut() = Some(area);
            // SAFETY: NSView's own, as AppKit needs it called.
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
        }

        // ---- The mouse ----

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Down(MouseButton::Left));
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Up(MouseButton::Left));
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Down(MouseButton::Right));
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Up(MouseButton::Right));
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Down(other_button(event)));
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Up(other_button(event)));
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Move);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Drag);
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Drag);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.mouse(event, MouseKind::Drag);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            if let Some(state) = self.state() {
                state.pointer_entered();
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            if let Some(state) = self.state() {
                state.pointer_exited();
            }
        }

        /// The cursor the app shows over the view (AppKit asks as the mouse comes in, and after it showed its own).
        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, _event: &NSEvent) {
            if let Some(state) = self.state() {
                state.show_cursor();
            }
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.scroll(event, self.point_of(event));
            }
        }

        /// A trackpad's pinch.
        #[unsafe(method(magnifyWithEvent:))]
        fn magnify_with_event(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.magnify(event, self.point_of(event));
            }
        }

        /// A trackpad's two fingers turning.
        #[unsafe(method(rotateWithEvent:))]
        fn rotate_with_event(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.rotate(event, self.point_of(event));
            }
        }

        /// A tablet's pen reporting on its own (not as the mouse).
        #[unsafe(method(tabletPoint:))]
        fn tablet_point(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.tablet_point(event, self.point_of(event), None);
            }
        }

        /// A tablet's pen (or its eraser end) coming near or going away.
        #[unsafe(method(tabletProximity:))]
        fn tablet_proximity(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.tablet_proximity(event);
            }
        }

        // ---- The keyboard ----

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.key_down(self, event);
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.key_up(event);
            }
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            if let Some(state) = self.state() {
                state.flags_changed(event);
            }
        }

        /// The display link's tick (macOS 14's: `-[NSView displayLinkWithTarget:selector:]`).
        #[unsafe(method(displayLinkTick:))]
        fn display_link_tick(&self, _link: &AnyObject) {
            if let Some(state) = self.state() {
                state.ticked();
            }
        }
    }

    // SAFETY: NSObject's protocol, nothing of its own.
    unsafe impl NSObjectProtocol for KompozView {}

    // SAFETY: the methods' signatures are NSTextInputClient's; the ranges are UTF-16 code units of the field's text,
    // as AppKit's.
    unsafe impl NSTextInputClient for KompozView {
        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &AnyObject, replacement: NSRange) {
            if let Some(state) = self.state() {
                state.ime_insert(&text_of(string), range_of(replacement));
            }
        }

        /// A key the input method passed on (an arrow, Return, Backspace with nothing composing): the app's key event's.
        #[unsafe(method(doCommandBySelector:))]
        fn do_command_by_selector(&self, _selector: Sel) {
            if let Some(state) = self.state() {
                state.ime_command();
            }
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked_text(&self, string: &AnyObject, selected: NSRange, replacement: NSRange) {
            if let Some(state) = self.state() {
                state.ime_set_marked(&text_of(string), (selected.location, selected.length), range_of(replacement));
            }
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            if let Some(state) = self.state() {
                state.ime_unmark();
            }
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            let (location, length) = self.state().map_or((NOT_FOUND, 0), |s| s.ime_selected_range());
            NSRange::new(location, length)
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            let (location, length) = self.state().and_then(|s| s.ime_marked_range()).unwrap_or((NOT_FOUND, 0));
            NSRange::new(location, length)
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            self.state().is_some_and(|s| s.ime_marked_range().is_some())
        }

        /// The field's text in `range` (what an input method reconverts, what the press-and-hold menu replaces).
        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        fn attributed_substring(&self, range: NSRange, actual: NSRangePointer) -> Option<Retained<NSAttributedString>> {
            let found = self.state().and_then(|s| s.ime_substring(range.location, range.length));
            found.map(|(text, (location, length))| {
                if !actual.is_null() {
                    // SAFETY: AppKit passes a valid NSRange to write the actual range to, or null (checked).
                    unsafe { *actual = NSRange::new(location, length) };
                }
                NSAttributedString::initWithString(NSAttributedString::alloc(), &NSString::from_str(&text))
            })
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes_for_marked_text(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new()
        }

        /// Where the field is on the screen: where the input method shows its candidates.
        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect_for_character_range(&self, range: NSRange, actual: NSRangePointer) -> NSRect {
            if !actual.is_null() {
                // SAFETY: AppKit passes a valid NSRange to write the actual range to, or null (checked).
                unsafe { *actual = range };
            }
            let Some(state) = self.state() else { return NSRect::ZERO };
            let rect = state.ime_area_points();
            let in_window = self.convertRect_toView(rect, None);
            self.window().map_or(NSRect::ZERO, |w| w.convertRectToScreen(in_window))
        }

        /// The character at a point of the screen: the caret's where it is in the field (KompozR's text layout isn't
        /// the platform's to ask), none elsewhere.
        #[unsafe(method(characterIndexForPoint:))]
        fn character_index_for_point(&self, point: NSPoint) -> NSUInteger {
            let (Some(state), Some(window)) = (self.state(), self.window()) else { return NOT_FOUND };
            let at = self.convertPoint_fromView(window.convertPointFromScreen(point), None);
            state.ime_index_at(at.x, at.y)
        }
    }

    // SAFETY: the methods' signatures are NSWindowDelegate's.
    unsafe impl NSWindowDelegate for KompozView {
        /// The window's close button (or ⌘W): the app's close request; AppKit's close is the app's to do.
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            if let Some(state) = self.state() {
                state.close_requested();
            }
            false
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.placement_changed();
            }
        }

        #[unsafe(method(windowDidMiniaturize:))]
        fn window_did_miniaturize(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.minimized_changed(true);
            }
        }

        #[unsafe(method(windowDidDeminiaturize:))]
        fn window_did_deminiaturize(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.minimized_changed(false);
            }
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.focus_changed(true);
            }
        }

        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.focus_changed(false);
            }
        }

        #[unsafe(method(windowDidChangeBackingProperties:))]
        fn window_did_change_backing_properties(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.frame_changed(self);
            }
        }

        /// Moved to another display: its refresh rate (and scale) read again.
        #[unsafe(method(windowDidChangeScreen:))]
        fn window_did_change_screen(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.screen_changed();
            }
        }

        #[unsafe(method(windowDidEnterFullScreen:))]
        fn window_did_enter_full_screen(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.full_screen_changed(true);
            }
        }

        #[unsafe(method(windowDidExitFullScreen:))]
        fn window_did_exit_full_screen(&self, _notification: &NSNotification) {
            if let Some(state) = self.state() {
                state.full_screen_changed(false);
            }
        }
    }

    // SAFETY: the methods' signatures are NSDraggingDestination's.
    unsafe impl NSDraggingDestination for KompozView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            let Some(state) = self.state() else { return NSDragOperation::None };
            state.drag_entered(sender, self.point_in(sender.draggingLocation()))
        }

        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            let Some(state) = self.state() else { return NSDragOperation::None };
            state.drag_moved(sender, self.point_in(sender.draggingLocation()))
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            if let Some(state) = self.state() {
                state.drag_left();
            }
        }

        #[unsafe(method(prepareForDragOperation:))]
        fn prepare_for_drag_operation(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            true
        }

        /// Dropped: what it brings read now, handed to the app, its answer told the source.
        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            let at = self.point_in(sender.draggingLocation());
            self.state().is_some_and(|state| state.dropped(sender, at))
        }
    }

    // SAFETY: the methods' signatures are NSDraggingSource's.
    unsafe impl NSDraggingSource for KompozView {
        /// What the app's own drag allows (as the app said), within the app and outside it alike.
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(&self, _session: &NSDraggingSession, _context: NSDraggingContext) -> NSDragOperation {
            self.state().map_or(NSDragOperation::None, |s| s.own_drag_operations())
        }

        /// The app's own drag ended: taken with that operation, or not.
        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn dragging_session_ended(&self, _session: &NSDraggingSession, _point: NSPoint, operation: NSDragOperation) {
            if let Some(state) = self.state() {
                state.own_drag_ended(operation);
            }
        }
    }
);

impl KompozView {
    /// A view for `state`'s window, `width` by `height` points, its layer made (wanting one: makeBackingLayer).
    pub fn new(mtm: MainThreadMarker, state: &Rc<WindowState>, width: f64, height: f64) -> Retained<KompozView> {
        let this =
            KompozView::alloc(mtm).set_ivars(ViewIvars { state: Rc::downgrade(state), tracking: RefCell::new(None) });
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        // SAFETY: NSView's designated initializer on a freshly allocated view with its ivars set.
        let view: Retained<KompozView> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setWantsLayer(true);
        view
    }

    fn state(&self) -> Option<Rc<WindowState>> {
        self.ivars().state.upgrade()
    }

    /// An event's place in the view, its points from the top left.
    fn point_of(&self, event: &NSEvent) -> (f64, f64) {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        (p.x, p.y)
    }

    /// A point of the window's (a drag's location) in the view.
    fn point_in(&self, in_window: NSPoint) -> (f64, f64) {
        let p = self.convertPoint_fromView(in_window, None);
        (p.x, p.y)
    }

    fn mouse(&self, event: &NSEvent, kind: MouseKind) {
        if let Some(state) = self.state() {
            state.mouse(event, self.point_of(event), kind);
        }
    }

    /// The view as the input methods' client.
    pub fn as_responder(&self) -> &NSResponder {
        self
    }
}

/// The button of an `otherMouse…:` event: the middle one, back, forward (buttons 2, 3, 4).
fn other_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        _ => MouseButton::Middle,
    }
}

/// An input method's string (an NSString or an NSAttributedString) as text.
fn text_of(string: &AnyObject) -> String {
    if let Some(attributed) = string.downcast_ref::<NSAttributedString>() {
        return attributed.string().to_string();
    }
    string.downcast_ref::<NSString>().map(|s| s.to_string()).unwrap_or_default()
}

/// A replacement range, none for NSNotFound's.
fn range_of(range: NSRange) -> Option<(usize, usize)> {
    (range.location != NOT_FOUND).then_some((range.location, range.length))
}

/// The window's state: whether it may become key (an undecorated window may; one that mustn't take the focus not).
pub struct WindowIvars {
    pub focusable: Cell<bool>,
}

define_class!(
    // SAFETY: NSWindow's subclassing rules are kept (two questions answered, the frame's constraint refined); no Drop.
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KompozWindow"]
    #[ivars = WindowIvars]
    pub struct KompozWindow;

    impl KompozWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            self.ivars().focusable.get()
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            self.ivars().focusable.get()
        }

        /// AppKit's own constraint for where the window goes (its title bar kept below the menu bar, on a screen),
        /// its size kept as asked: a window taller than the screen reaches below it, as on Windows and X11, rather
        /// than being cut to the screen (the app's size, a script's resize, are what it draws at).
        #[unsafe(method(constrainFrameRect:toScreen:))]
        fn constrain_frame_rect(&self, rect: NSRect, screen: Option<&NSScreen>) -> NSRect {
            // SAFETY: NSWindow's own method, with the arguments AppKit gave this override.
            let placed: NSRect = unsafe { msg_send![super(self), constrainFrameRect: rect, toScreen: screen] };
            let top = placed.origin.y + placed.size.height;
            NSRect::new(NSPoint::new(placed.origin.x, top - rect.size.height), rect.size)
        }
    }

    // SAFETY: NSObject's protocol, nothing of its own.
    unsafe impl NSObjectProtocol for KompozWindow {}
);

impl KompozWindow {
    /// A window of `style`, its content `width` by `height` points; not released as it closes (the app keeps it
    /// while it needs it).
    pub fn new(
        mtm: MainThreadMarker,
        style: NSWindowStyleMask,
        width: f64,
        height: f64,
        focusable: bool,
    ) -> Retained<KompozWindow> {
        let this = KompozWindow::alloc(mtm).set_ivars(WindowIvars { focusable: Cell::new(focusable) });
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        // SAFETY: NSWindow's designated initializer on a freshly allocated window with its ivars set.
        let window: Retained<KompozWindow> = unsafe {
            msg_send![super(this), initWithContentRect: rect, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false]
        };
        // SAFETY: the window is owned through its Retained, so AppKit mustn't release it as it closes.
        unsafe { window.setReleasedWhenClosed(false) };
        window
    }
}
