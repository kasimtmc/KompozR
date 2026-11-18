//! libX11 with XInput 2 (libXi), and libXcursor, libXrandr and libX11-xcb where they are, loaded at run time as the
//! C++ Kompoz loads them (`dlopen`: on a Wayland-only desktop the app runs on another platform), their structs written
//! out by hand (`repr(C)`, each checked against the system's headers: the layout tests below), the display and what
//! the platform asks of it in safe wrappers: the connection, atoms, windows and their properties, events (core,
//! XInput 2's, XRandR's), selections, cursors, the input method (XIM), the keyboard's keymap through xkbcommon-x11,
//! the Vulkan surface; for a drag of the app's own (its XDND source) the grabs, the window tree under the pointer, an
//! icon window of the screen's ARGB visual filled with an image, and libXext's shapes (its outline, no input). libXcursor,
//! libXrandr, libX11-xcb and libXext are optional (the cursors are then X's own, the refresh rate 60 Hz, the keyboard
//! Xlib's, a drag's icon a rectangle). The only `unsafe` code of the X11 platform, each block saying why it holds.
//!
//! X errors (a drag's source gone, a property not there) are the requests' own failures, ignored where they happen:
//! Xlib's default handler would end the process.

#![allow(unsafe_code)]

use super::preedit::{CaretDirection, MARKING_FEEDBACK, Preedit, text_chars};
use crate::linux::sys::Library;
use crate::linux::xkb::{Context, KeyboardState, Keymap};
use kompoz_gpu::device::vulkan::VulkanSurface;
use kompoz_gpu::device::vulkan::ash::{khr, vk};
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void};
use std::os::fd::RawFd;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::OnceLock;

/// An X resource's id (`XID`): a window, a cursor.
pub type Xid = c_ulong;
pub type Window = Xid;
pub type Cursor = Xid;
pub type Pixmap = Xid;
type Colormap = Xid;
type Gc = *mut c_void;
pub type Atom = c_ulong;
pub type Time = c_ulong;
pub type KeySym = c_ulong;
type Bool = c_int;
type Status = c_int;
type XPointer = *mut c_char;
type Xim = *mut c_void;
type Xic = *mut c_void;

/// `None` (no window, no atom, no property).
pub const NONE: Xid = 0;
/// `CurrentTime`.
pub const CURRENT_TIME: Time = 0;

/// Xlib's `Display` (opaque).
#[repr(C)]
pub struct RawDisplay {
    _private: [u8; 0],
}

// ---- Constants (X.h, Xatom.h, Xutil.h, cursorfont.h, XI2.h, randr.h) ------------------------------------------------

// Event types.
pub const KEY_PRESS: c_int = 2;
pub const KEY_RELEASE: c_int = 3;
pub const FOCUS_IN: c_int = 9;
pub const FOCUS_OUT: c_int = 10;
pub const EXPOSE: c_int = 12;
pub const CONFIGURE_NOTIFY: c_int = 22;
pub const PROPERTY_NOTIFY: c_int = 28;
pub const SELECTION_CLEAR: c_int = 29;
pub const SELECTION_REQUEST: c_int = 30;
pub const SELECTION_NOTIFY: c_int = 31;
pub const CLIENT_MESSAGE: c_int = 33;
pub const MAPPING_NOTIFY: c_int = 34;
pub const GENERIC_EVENT: c_int = 35;

// Focus modes and details.
pub const NOTIFY_GRAB: c_int = 1;
pub const NOTIFY_UNGRAB: c_int = 2;
pub const NOTIFY_INFERIOR: c_int = 2;
pub const NOTIFY_POINTER: c_int = 5;

// Property states, mapping requests.
pub const PROPERTY_NEW_VALUE: c_int = 0;
pub const PROPERTY_DELETE: c_int = 1;
pub const MAPPING_MODIFIER: c_int = 0;
pub const MAPPING_KEYBOARD: c_int = 1;

// Event masks.
pub const NO_EVENT_MASK: c_long = 0;
pub const KEY_PRESS_MASK: c_long = 1 << 0;
pub const KEY_RELEASE_MASK: c_long = 1 << 1;
pub const EXPOSURE_MASK: c_long = 1 << 15;
pub const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;
pub const SUBSTRUCTURE_NOTIFY_MASK: c_long = 1 << 19;
pub const SUBSTRUCTURE_REDIRECT_MASK: c_long = 1 << 20;
pub const FOCUS_CHANGE_MASK: c_long = 1 << 21;
pub const PROPERTY_CHANGE_MASK: c_long = 1 << 22;

// Modifier masks (an event's state).
pub const SHIFT_MASK: c_uint = 1 << 0;
pub const CONTROL_MASK: c_uint = 1 << 2;
pub const MOD1_MASK: c_uint = 1 << 3;
pub const MOD4_MASK: c_uint = 1 << 6;

// Predefined atoms.
pub const XA_ATOM: Atom = 4;
pub const XA_CARDINAL: Atom = 6;
pub const XA_STRING: Atom = 31;
pub const XA_WM_NAME: Atom = 39;

// Window creation.
const CW_BACK_PIXEL: c_ulong = 1 << 1;
const CW_BORDER_PIXEL: c_ulong = 1 << 3;
const CW_BIT_GRAVITY: c_ulong = 1 << 4;
const CW_EVENT_MASK: c_ulong = 1 << 11;
const CW_OVERRIDE_REDIRECT: c_ulong = 1 << 9;
const CW_SAVE_UNDER: c_ulong = 1 << 10;
const CW_COLORMAP: c_ulong = 1 << 13;
const TRUE_COLOR: c_int = 4;
const ALLOC_NONE: c_int = 0;
const IS_VIEWABLE: c_int = 2;
const Z_PIXMAP: c_int = 2;
const LSB_FIRST: c_int = 0;
const GRAB_MODE_ASYNC: c_int = 1;
const GRAB_SUCCESS: c_int = 0;
// libXext's shapes.
const SHAPE_BOUNDING: c_int = 0;
const SHAPE_INPUT: c_int = 2;
const SHAPE_SET: c_int = 0;
const UNSORTED: c_int = 0;
const NORTH_WEST_GRAVITY: c_int = 1;
const INPUT_OUTPUT: c_uint = 1;
const COPY_FROM_PARENT: c_int = 0;
const PROP_MODE_REPLACE: c_int = 0;
const ANY_PROPERTY_TYPE: Atom = 0;
const SUCCESS: Status = 0;

// The cursor font's shapes.
pub const XC_X_CURSOR: c_uint = 0;
pub const XC_CIRCLE: c_uint = 24;
pub const XC_CROSSHAIR: c_uint = 34;
pub const XC_FLEUR: c_uint = 52;
pub const XC_HAND2: c_uint = 60;
pub const XC_LEFT_PTR: c_uint = 68;
pub const XC_SB_H_DOUBLE_ARROW: c_uint = 108;
pub const XC_SB_V_DOUBLE_ARROW: c_uint = 116;
pub const XC_WATCH: c_uint = 150;
pub const XC_XTERM: c_uint = 152;

// The input method's styles and lookups.
const XIM_PREEDIT_CALLBACKS: c_ulong = 0x0002;
const XIM_PREEDIT_POSITION: c_ulong = 0x0004;
const XIM_PREEDIT_NOTHING: c_ulong = 0x0008;
const XIM_STATUS_NOTHING: c_ulong = 0x0400;
const X_BUFFER_OVERFLOW: Status = -1;
const X_LOOKUP_CHARS: Status = 2;
const X_LOOKUP_BOTH: Status = 4;
const XN_INPUT_STYLE: &CStr = c"inputStyle";
const XN_CLIENT_WINDOW: &CStr = c"clientWindow";
const XN_FOCUS_WINDOW: &CStr = c"focusWindow";
const XN_PREEDIT_ATTRIBUTES: &CStr = c"preeditAttributes";
const XN_SPOT_LOCATION: &CStr = c"spotLocation";
const XN_QUERY_INPUT_STYLE: &CStr = c"queryInputStyle";
const XN_DESTROY_CALLBACK: &CStr = c"destroyCallback";
const XN_PREEDIT_START_CALLBACK: &CStr = c"preeditStartCallback";
const XN_PREEDIT_DONE_CALLBACK: &CStr = c"preeditDoneCallback";
const XN_PREEDIT_DRAW_CALLBACK: &CStr = c"preeditDrawCallback";
const XN_PREEDIT_CARET_CALLBACK: &CStr = c"preeditCaretCallback";

// XInput 2.
pub const XI_BUTTON_PRESS: c_int = 4;
pub const XI_BUTTON_RELEASE: c_int = 5;
pub const XI_MOTION: c_int = 6;
pub const XI_ENTER: c_int = 7;
pub const XI_LEAVE: c_int = 8;
pub const XI_HIERARCHY_CHANGED: c_int = 11;
pub const XI_TOUCH_BEGIN: c_int = 18;
pub const XI_TOUCH_UPDATE: c_int = 19;
pub const XI_TOUCH_END: c_int = 20;
pub const XI_GESTURE_PINCH_BEGIN: c_int = 27;
pub const XI_GESTURE_PINCH_UPDATE: c_int = 28;
pub const XI_GESTURE_PINCH_END: c_int = 29;
/// `XI_LASTEVENT` (XI 2.4's `XI_GestureSwipeEnd`).
const XI_LAST_EVENT: c_int = 32;
/// `XIMaskLen(XI_LASTEVENT)`.
const XI_MASK_LEN: usize = (XI_LAST_EVENT as usize >> 3) + 1;
pub const XI_ALL_DEVICES: c_int = 0;
pub const XI_ALL_MASTER_DEVICES: c_int = 1;
pub const XI_SLAVE_POINTER: c_int = 3;
const XI_SCROLL_CLASS: c_int = 3;
const XI_TOUCH_CLASS: c_int = 8;
const XI_SCROLL_TYPE_VERTICAL: c_int = 1;
const XI_DIRECT_TOUCH: c_int = 1;
/// An event of a device emulated from another (a touch's pointer, a smooth wheel's buttons).
pub const XI_POINTER_EMULATED: c_int = 1 << 16;
pub const XI_NOTIFY_NORMAL: c_int = 0;
pub const XI_NOTIFY_UNGRAB: c_int = 2;

// XRandR.
const RR_SCREEN_CHANGE_NOTIFY_MASK: c_int = 1 << 0;
const RR_CRTC_CHANGE_NOTIFY_MASK: c_int = 1 << 1;
const RR_SCREEN_CHANGE_NOTIFY: c_int = 0;
const RR_NOTIFY: c_int = 1;
pub const RR_INTERLACE: c_ulong = 0x10;
pub const RR_DOUBLE_SCAN: c_ulong = 0x20;

// ---- Structs (Xlib.h, Xutil.h, XInput2.h, Xrandr.h) -----------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct XAnyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XKeyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    root: Window,
    subwindow: Window,
    time: Time,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same_screen: Bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XFocusChangeEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    mode: c_int,
    detail: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XExposeEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    count: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    event: Window,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    above: Window,
    override_redirect: Bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XPropertyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    atom: Atom,
    time: Time,
    state: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XSelectionClearEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    selection: Atom,
    time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XSelectionRequestEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    owner: Window,
    requestor: Window,
    selection: Atom,
    target: Atom,
    property: Atom,
    time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XSelectionEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    requestor: Window,
    selection: Atom,
    target: Atom,
    property: Atom,
    time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    message_type: Atom,
    format: c_int,
    /// (A union of 20 bytes, 10 shorts or 5 longs: the longs, as format 32 has them.)
    data: [c_long; 5],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XMappingEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    window: Window,
    request: c_int,
    first_keycode: c_int,
    count: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XGenericEventCookie {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    extension: c_int,
    evtype: c_int,
    cookie: c_uint,
    data: *mut c_void,
}

/// `XEvent`: 24 longs, read as the struct its type says.
#[repr(C)]
#[derive(Clone, Copy)]
union RawEvent {
    type_: c_int,
    any: XAnyEvent,
    key: XKeyEvent,
    focus: XFocusChangeEvent,
    expose: XExposeEvent,
    configure: XConfigureEvent,
    property: XPropertyEvent,
    selection_clear: XSelectionClearEvent,
    selection_request: XSelectionRequestEvent,
    selection: XSelectionEvent,
    client: XClientMessageEvent,
    mapping: XMappingEvent,
    cookie: XGenericEventCookie,
    pad: [c_long; 24],
}

#[repr(C)]
struct XSetWindowAttributes {
    background_pixmap: Xid,
    background_pixel: c_ulong,
    border_pixmap: Xid,
    border_pixel: c_ulong,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: Bool,
    event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: Bool,
    colormap: Xid,
    cursor: Cursor,
}

/// `XWindowAttributes`.
#[repr(C)]
struct XWindowAttributes {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    depth: c_int,
    visual: *mut c_void,
    root: Window,
    class: c_int,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: Bool,
    colormap: Colormap,
    map_installed: Bool,
    map_state: c_int,
    all_event_masks: c_long,
    your_event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: Bool,
    screen: *mut c_void,
}

/// `XVisualInfo`.
#[repr(C)]
struct XVisualInfo {
    visual: *mut c_void,
    visualid: c_ulong,
    screen: c_int,
    depth: c_int,
    class: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
    colormap_size: c_int,
    bits_per_rgb: c_int,
}

/// `XImage`: an image of the client's memory to put on a drawable (its functions filled by `XInitImage`).
#[repr(C)]
struct XImage {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut c_char,
    byte_order: c_int,
    bitmap_unit: c_int,
    bitmap_bit_order: c_int,
    bitmap_pad: c_int,
    depth: c_int,
    bytes_per_line: c_int,
    bits_per_pixel: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
    obdata: XPointer,
    f: [*mut c_void; 6],
}

/// `XRectangle`.
#[repr(C)]
struct XRectangle {
    x: c_short,
    y: c_short,
    width: c_ushort,
    height: c_ushort,
}

#[repr(C)]
struct XClassHint {
    res_name: *mut c_char,
    res_class: *mut c_char,
}

#[repr(C)]
struct XimStyles {
    count_styles: c_ushort,
    supported_styles: *mut c_ulong,
}

#[repr(C)]
struct XimCallback {
    client_data: XPointer,
    callback: Option<unsafe extern "C" fn(Xim, XPointer, XPointer)>,
}

/// `XICCallback`: an input context's callback (the preedit's; what it returns counts only for the start's).
#[repr(C)]
struct XicCallback {
    client_data: XPointer,
    callback: Option<unsafe extern "C" fn(Xic, XPointer, XPointer) -> c_int>,
}

/// `XIMText`: what a preedit draw puts in (its string none: only the feedback changed).
#[repr(C)]
struct XimText {
    length: c_ushort,
    feedback: *mut c_ulong,
    encoding_is_wchar: Bool,
    /// `multi_byte` (a C string), or `wide_char` (`length` wchar_t's).
    string: *mut c_void,
}

/// `XIMPreeditDrawCallbackStruct`.
#[repr(C)]
struct XimPreeditDraw {
    caret: c_int,
    chg_first: c_int,
    chg_length: c_int,
    text: *mut XimText,
}

/// `XIMPreeditCaretCallbackStruct` (its position given back).
#[repr(C)]
struct XimPreeditCaret {
    position: c_int,
    direction: c_int,
    style: c_int,
}

/// `XPoint`: where an input method shows what is being composed.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct XPoint {
    pub x: c_short,
    pub y: c_short,
}

#[repr(C)]
struct XiEventMask {
    deviceid: c_int,
    mask_len: c_int,
    mask: *mut c_uchar,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XiModifierState {
    base: c_int,
    latched: c_int,
    locked: c_int,
    effective: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XiButtonState {
    mask_len: c_int,
    mask: *mut c_uchar,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XiValuatorState {
    mask_len: c_int,
    mask: *mut c_uchar,
    values: *mut f64,
}

#[repr(C)]
struct XiDeviceEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    extension: c_int,
    evtype: c_int,
    time: Time,
    deviceid: c_int,
    sourceid: c_int,
    detail: c_int,
    root: Window,
    event: Window,
    child: Window,
    root_x: f64,
    root_y: f64,
    event_x: f64,
    event_y: f64,
    flags: c_int,
    buttons: XiButtonState,
    valuators: XiValuatorState,
    mods: XiModifierState,
    group: XiModifierState,
}

#[repr(C)]
struct XiEnterEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    extension: c_int,
    evtype: c_int,
    time: Time,
    deviceid: c_int,
    sourceid: c_int,
    detail: c_int,
    root: Window,
    event: Window,
    child: Window,
    root_x: f64,
    root_y: f64,
    event_x: f64,
    event_y: f64,
    mode: c_int,
    focus: Bool,
    same_screen: Bool,
    buttons: XiButtonState,
    mods: XiModifierState,
    group: XiModifierState,
}

#[repr(C)]
struct XiGesturePinchEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: Bool,
    display: *mut RawDisplay,
    extension: c_int,
    evtype: c_int,
    time: Time,
    deviceid: c_int,
    sourceid: c_int,
    detail: c_int,
    root: Window,
    event: Window,
    child: Window,
    root_x: f64,
    root_y: f64,
    event_x: f64,
    event_y: f64,
    delta_x: f64,
    delta_y: f64,
    delta_unaccel_x: f64,
    delta_unaccel_y: f64,
    scale: f64,
    delta_angle: f64,
    flags: c_int,
    mods: XiModifierState,
    group: XiModifierState,
}

#[repr(C)]
struct XiDeviceInfo {
    deviceid: c_int,
    name: *mut c_char,
    use_: c_int,
    attachment: c_int,
    enabled: Bool,
    num_classes: c_int,
    classes: *mut *mut XiAnyClassInfo,
}

#[repr(C)]
struct XiAnyClassInfo {
    type_: c_int,
    sourceid: c_int,
}

#[repr(C)]
struct XiScrollClassInfo {
    type_: c_int,
    sourceid: c_int,
    number: c_int,
    scroll_type: c_int,
    increment: f64,
    flags: c_int,
}

#[repr(C)]
struct XiTouchClassInfo {
    type_: c_int,
    sourceid: c_int,
    mode: c_int,
    num_touches: c_int,
}

#[repr(C)]
struct XrrModeInfo {
    id: Xid,
    width: c_uint,
    height: c_uint,
    dot_clock: c_ulong,
    h_sync_start: c_uint,
    h_sync_end: c_uint,
    h_total: c_uint,
    h_skew: c_uint,
    v_sync_start: c_uint,
    v_sync_end: c_uint,
    v_total: c_uint,
    name: *mut c_char,
    name_length: c_uint,
    mode_flags: c_ulong,
}

#[repr(C)]
struct XrrScreenResources {
    timestamp: Time,
    config_timestamp: Time,
    ncrtc: c_int,
    crtcs: *mut Xid,
    noutput: c_int,
    outputs: *mut Xid,
    nmode: c_int,
    modes: *mut XrrModeInfo,
}

#[repr(C)]
struct XrrCrtcInfo {
    timestamp: Time,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
    mode: Xid,
    rotation: c_ushort,
    noutput: c_int,
    outputs: *mut Xid,
    rotations: c_ushort,
    npossible: c_int,
    possible: *mut Xid,
}

// ---- The functions --------------------------------------------------------------------------------------------------

type ErrorHandler = unsafe extern "C" fn(*mut RawDisplay, *mut c_void) -> c_int;
type EventPredicate = unsafe extern "C" fn(*mut RawDisplay, *mut RawEvent, XPointer) -> Bool;
type InstantiateCallback = unsafe extern "C" fn(*mut RawDisplay, XPointer, XPointer);
type D = *mut RawDisplay;

/// libX11's functions used (as C++'s x11_dynamic.hpp's list, with the screen and connection macros' functions).
struct X11Functions {
    init_threads: unsafe extern "C" fn() -> Status,
    open_display: unsafe extern "C" fn(*const c_char) -> D,
    close_display: unsafe extern "C" fn(D) -> c_int,
    connection_number: unsafe extern "C" fn(D) -> c_int,
    default_screen: unsafe extern "C" fn(D) -> c_int,
    root_window: unsafe extern "C" fn(D, c_int) -> Window,
    set_error_handler: unsafe extern "C" fn(Option<ErrorHandler>) -> Option<ErrorHandler>,
    intern_atoms: unsafe extern "C" fn(D, *mut *mut c_char, c_int, Bool, *mut Atom) -> Status,
    get_atom_name: unsafe extern "C" fn(D, Atom) -> *mut c_char,
    create_window: unsafe extern "C" fn(
        D,
        Window,
        c_int,
        c_int,
        c_uint,
        c_uint,
        c_uint,
        c_int,
        c_uint,
        *mut c_void,
        c_ulong,
        *mut XSetWindowAttributes,
    ) -> Window,
    destroy_window: unsafe extern "C" fn(D, Window) -> c_int,
    map_window: unsafe extern "C" fn(D, Window) -> c_int,
    change_property: unsafe extern "C" fn(D, Window, Atom, Atom, c_int, c_int, *const c_uchar, c_int) -> c_int,
    delete_property: unsafe extern "C" fn(D, Window, Atom) -> c_int,
    #[allow(clippy::type_complexity)]
    get_window_property: unsafe extern "C" fn(
        D,
        Window,
        Atom,
        c_long,
        c_long,
        Bool,
        Atom,
        *mut Atom,
        *mut c_int,
        *mut c_ulong,
        *mut c_ulong,
        *mut *mut c_uchar,
    ) -> c_int,
    free: unsafe extern "C" fn(*mut c_void) -> c_int,
    set_wm_protocols: unsafe extern "C" fn(D, Window, *mut Atom, c_int) -> Status,
    set_class_hint: unsafe extern "C" fn(D, Window, *mut XClassHint) -> c_int,
    pending: unsafe extern "C" fn(D) -> c_int,
    next_event: unsafe extern "C" fn(D, *mut RawEvent) -> c_int,
    filter_event: unsafe extern "C" fn(*mut RawEvent, Window) -> Bool,
    flush: unsafe extern "C" fn(D) -> c_int,
    send_event: unsafe extern "C" fn(D, Window, Bool, c_long, *mut RawEvent) -> Status,
    resize_window: unsafe extern "C" fn(D, Window, c_uint, c_uint) -> c_int,
    translate_coordinates:
        unsafe extern "C" fn(D, Window, Window, c_int, c_int, *mut c_int, *mut c_int, *mut Window) -> Bool,
    define_cursor: unsafe extern "C" fn(D, Window, Cursor) -> c_int,
    create_font_cursor: unsafe extern "C" fn(D, c_uint) -> Cursor,
    free_cursor: unsafe extern "C" fn(D, Cursor) -> c_int,
    set_selection_owner: unsafe extern "C" fn(D, Atom, Window, Time) -> c_int,
    get_selection_owner: unsafe extern "C" fn(D, Atom) -> Window,
    convert_selection: unsafe extern "C" fn(D, Atom, Atom, Atom, Window, Time) -> c_int,
    lookup_keysym: unsafe extern "C" fn(*mut XKeyEvent, c_int) -> KeySym,
    utf8_lookup_string:
        unsafe extern "C" fn(Xic, *mut XKeyEvent, *mut c_char, c_int, *mut KeySym, *mut Status) -> c_int,
    lookup_string: unsafe extern "C" fn(*mut XKeyEvent, *mut c_char, c_int, *mut KeySym, *mut c_void) -> c_int,
    open_im: unsafe extern "C" fn(D, *mut c_void, *mut c_char, *mut c_char) -> Xim,
    close_im: unsafe extern "C" fn(Xim) -> Status,
    create_ic: unsafe extern "C" fn(Xim, ...) -> Xic,
    destroy_ic: unsafe extern "C" fn(Xic),
    set_ic_focus: unsafe extern "C" fn(Xic),
    unset_ic_focus: unsafe extern "C" fn(Xic),
    utf8_reset_ic: unsafe extern "C" fn(Xic) -> *mut c_char,
    set_ic_values: unsafe extern "C" fn(Xic, ...) -> *mut c_char,
    set_im_values: unsafe extern "C" fn(Xim, ...) -> *mut c_char,
    get_im_values: unsafe extern "C" fn(Xim, ...) -> *mut c_char,
    va_create_nested_list: unsafe extern "C" fn(c_int, ...) -> *mut c_void,
    set_locale_modifiers: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    register_im_instantiate_callback:
        unsafe extern "C" fn(D, *mut c_void, *mut c_char, *mut c_char, Option<InstantiateCallback>, XPointer) -> Bool,
    unregister_im_instantiate_callback:
        unsafe extern "C" fn(D, *mut c_void, *mut c_char, *mut c_char, Option<InstantiateCallback>, XPointer) -> Bool,
    xkb_set_detectable_auto_repeat: unsafe extern "C" fn(D, Bool, *mut Bool) -> Bool,
    select_input: unsafe extern "C" fn(D, Window, c_long) -> c_int,
    query_extension: unsafe extern "C" fn(D, *const c_char, *mut c_int, *mut c_int, *mut c_int) -> Bool,
    get_event_data: unsafe extern "C" fn(D, *mut XGenericEventCookie) -> Bool,
    free_event_data: unsafe extern "C" fn(D, *mut XGenericEventCookie),
    extended_max_request_size: unsafe extern "C" fn(D) -> c_long,
    max_request_size: unsafe extern "C" fn(D) -> c_long,
    refresh_keyboard_mapping: unsafe extern "C" fn(*mut XMappingEvent) -> c_int,
    check_if_event: unsafe extern "C" fn(D, *mut RawEvent, Option<EventPredicate>, XPointer) -> Bool,
    grab_keyboard: unsafe extern "C" fn(D, Window, Bool, c_int, c_int, Time) -> c_int,
    ungrab_keyboard: unsafe extern "C" fn(D, Time) -> c_int,
    query_tree: unsafe extern "C" fn(D, Window, *mut Window, *mut Window, *mut *mut Window, *mut c_uint) -> Status,
    get_window_attributes: unsafe extern "C" fn(D, Window, *mut XWindowAttributes) -> Status,
    match_visual_info: unsafe extern "C" fn(D, c_int, c_int, c_int, *mut XVisualInfo) -> Status,
    create_colormap: unsafe extern "C" fn(D, Window, *mut c_void, c_int) -> Colormap,
    free_colormap: unsafe extern "C" fn(D, Colormap) -> c_int,
    create_pixmap: unsafe extern "C" fn(D, Xid, c_uint, c_uint, c_uint) -> Pixmap,
    free_pixmap: unsafe extern "C" fn(D, Pixmap) -> c_int,
    create_bitmap_from_data: unsafe extern "C" fn(D, Xid, *const c_char, c_uint, c_uint) -> Pixmap,
    create_gc: unsafe extern "C" fn(D, Xid, c_ulong, *mut c_void) -> Gc,
    free_gc: unsafe extern "C" fn(D, Gc) -> c_int,
    init_image: unsafe extern "C" fn(*mut XImage) -> Status,
    put_image: unsafe extern "C" fn(D, Xid, Gc, *mut XImage, c_int, c_int, c_int, c_int, c_uint, c_uint) -> c_int,
    set_window_background_pixmap: unsafe extern "C" fn(D, Window, Pixmap) -> c_int,
    clear_window: unsafe extern "C" fn(D, Window) -> c_int,
    move_window: unsafe extern "C" fn(D, Window, c_int, c_int) -> c_int,
    map_raised: unsafe extern "C" fn(D, Window) -> c_int,
    sync: unsafe extern "C" fn(D, Bool) -> c_int,
}

/// libXi's.
struct XiFunctions {
    query_version: unsafe extern "C" fn(D, *mut c_int, *mut c_int) -> Status,
    select_events: unsafe extern "C" fn(D, Window, *mut XiEventMask, c_int) -> c_int,
    query_device: unsafe extern "C" fn(D, c_int, *mut c_int) -> *mut XiDeviceInfo,
    free_device_info: unsafe extern "C" fn(*mut XiDeviceInfo),
    grab_device: unsafe extern "C" fn(D, c_int, Window, Time, Cursor, c_int, c_int, Bool, *mut XiEventMask) -> Status,
    ungrab_device: unsafe extern "C" fn(D, c_int, Time) -> Status,
}

/// libXext's shapes.
struct ShapeFunctions {
    query_extension: unsafe extern "C" fn(D, *mut c_int, *mut c_int) -> Bool,
    combine_mask: unsafe extern "C" fn(D, Window, c_int, c_int, c_int, Pixmap, c_int),
    combine_rectangles: unsafe extern "C" fn(D, Window, c_int, c_int, c_int, *mut XRectangle, c_int, c_int, c_int),
}

/// libXrandr's.
struct RandrFunctions {
    query_extension: unsafe extern "C" fn(D, *mut c_int, *mut c_int) -> Bool,
    select_input: unsafe extern "C" fn(D, Window, c_int),
    get_screen_resources_current: unsafe extern "C" fn(D, Window) -> *mut XrrScreenResources,
    free_screen_resources: unsafe extern "C" fn(*mut XrrScreenResources),
    get_crtc_info: unsafe extern "C" fn(D, *mut XrrScreenResources, Xid) -> *mut XrrCrtcInfo,
    free_crtc_info: unsafe extern "C" fn(*mut XrrCrtcInfo),
}

/// The libraries, loaded once for the process (as C++'s), and their functions.
struct Xlib {
    x: X11Functions,
    xi: XiFunctions,
    /// libXcursor's `XcursorLibraryLoadCursor`.
    xcursor_load: Option<unsafe extern "C" fn(D, *const c_char) -> Cursor>,
    randr: Option<RandrFunctions>,
    /// libXext's shapes.
    shape: Option<ShapeFunctions>,
    /// libX11-xcb's `XGetXCBConnection` (the keyboard's keymap through xkbcommon-x11).
    xcb_connection: Option<unsafe extern "C" fn(D) -> *mut c_void>,
    _libraries: Vec<Library>,
}

/// Looks `name` up in `library` as a function pointer of type `F`.
///
/// # Safety
/// `F` must be the symbol's C signature as an `unsafe extern "C" fn`.
unsafe fn function<F: Copy>(library: &Library, name: &CStr) -> Result<F, String> {
    let p = library.symbol(name).ok_or_else(|| format!("no {}", name.to_string_lossy()))?;
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<*mut c_void>());
    // SAFETY: the caller says `F` is the symbol's signature, a function pointer as big as `p`.
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&p) })
}

impl Xlib {
    fn load() -> Result<Xlib, String> {
        let x11 = Library::open(&["libX11.so.6", "libX11.so"])?;
        let xi = Library::open(&["libXi.so.6", "libXi.so"])?;
        let l = &x11;
        // SAFETY: each `function` call's type is Xlib.h's (Xutil.h's, XKBlib.h's) signature of its symbol, as
        // `X11Functions` declares it.
        let x = unsafe {
            X11Functions {
                init_threads: function(l, c"XInitThreads")?,
                open_display: function(l, c"XOpenDisplay")?,
                close_display: function(l, c"XCloseDisplay")?,
                connection_number: function(l, c"XConnectionNumber")?,
                default_screen: function(l, c"XDefaultScreen")?,
                root_window: function(l, c"XRootWindow")?,
                set_error_handler: function(l, c"XSetErrorHandler")?,
                intern_atoms: function(l, c"XInternAtoms")?,
                get_atom_name: function(l, c"XGetAtomName")?,
                create_window: function(l, c"XCreateWindow")?,
                destroy_window: function(l, c"XDestroyWindow")?,
                map_window: function(l, c"XMapWindow")?,
                change_property: function(l, c"XChangeProperty")?,
                delete_property: function(l, c"XDeleteProperty")?,
                get_window_property: function(l, c"XGetWindowProperty")?,
                free: function(l, c"XFree")?,
                set_wm_protocols: function(l, c"XSetWMProtocols")?,
                set_class_hint: function(l, c"XSetClassHint")?,
                pending: function(l, c"XPending")?,
                next_event: function(l, c"XNextEvent")?,
                filter_event: function(l, c"XFilterEvent")?,
                flush: function(l, c"XFlush")?,
                send_event: function(l, c"XSendEvent")?,
                resize_window: function(l, c"XResizeWindow")?,
                translate_coordinates: function(l, c"XTranslateCoordinates")?,
                define_cursor: function(l, c"XDefineCursor")?,
                create_font_cursor: function(l, c"XCreateFontCursor")?,
                free_cursor: function(l, c"XFreeCursor")?,
                set_selection_owner: function(l, c"XSetSelectionOwner")?,
                get_selection_owner: function(l, c"XGetSelectionOwner")?,
                convert_selection: function(l, c"XConvertSelection")?,
                lookup_keysym: function(l, c"XLookupKeysym")?,
                utf8_lookup_string: function(l, c"Xutf8LookupString")?,
                lookup_string: function(l, c"XLookupString")?,
                open_im: function(l, c"XOpenIM")?,
                close_im: function(l, c"XCloseIM")?,
                create_ic: function(l, c"XCreateIC")?,
                destroy_ic: function(l, c"XDestroyIC")?,
                set_ic_focus: function(l, c"XSetICFocus")?,
                unset_ic_focus: function(l, c"XUnsetICFocus")?,
                utf8_reset_ic: function(l, c"Xutf8ResetIC")?,
                set_ic_values: function(l, c"XSetICValues")?,
                set_im_values: function(l, c"XSetIMValues")?,
                get_im_values: function(l, c"XGetIMValues")?,
                va_create_nested_list: function(l, c"XVaCreateNestedList")?,
                set_locale_modifiers: function(l, c"XSetLocaleModifiers")?,
                register_im_instantiate_callback: function(l, c"XRegisterIMInstantiateCallback")?,
                unregister_im_instantiate_callback: function(l, c"XUnregisterIMInstantiateCallback")?,
                xkb_set_detectable_auto_repeat: function(l, c"XkbSetDetectableAutoRepeat")?,
                select_input: function(l, c"XSelectInput")?,
                query_extension: function(l, c"XQueryExtension")?,
                get_event_data: function(l, c"XGetEventData")?,
                free_event_data: function(l, c"XFreeEventData")?,
                extended_max_request_size: function(l, c"XExtendedMaxRequestSize")?,
                max_request_size: function(l, c"XMaxRequestSize")?,
                refresh_keyboard_mapping: function(l, c"XRefreshKeyboardMapping")?,
                check_if_event: function(l, c"XCheckIfEvent")?,
                grab_keyboard: function(l, c"XGrabKeyboard")?,
                ungrab_keyboard: function(l, c"XUngrabKeyboard")?,
                query_tree: function(l, c"XQueryTree")?,
                get_window_attributes: function(l, c"XGetWindowAttributes")?,
                match_visual_info: function(l, c"XMatchVisualInfo")?,
                create_colormap: function(l, c"XCreateColormap")?,
                free_colormap: function(l, c"XFreeColormap")?,
                create_pixmap: function(l, c"XCreatePixmap")?,
                free_pixmap: function(l, c"XFreePixmap")?,
                create_bitmap_from_data: function(l, c"XCreateBitmapFromData")?,
                create_gc: function(l, c"XCreateGC")?,
                free_gc: function(l, c"XFreeGC")?,
                init_image: function(l, c"XInitImage")?,
                put_image: function(l, c"XPutImage")?,
                set_window_background_pixmap: function(l, c"XSetWindowBackgroundPixmap")?,
                clear_window: function(l, c"XClearWindow")?,
                move_window: function(l, c"XMoveWindow")?,
                map_raised: function(l, c"XMapRaised")?,
                sync: function(l, c"XSync")?,
            }
        };
        let l = &xi;
        // SAFETY: as above, XInput2.h's signatures.
        let xi_functions = unsafe {
            XiFunctions {
                query_version: function(l, c"XIQueryVersion")?,
                select_events: function(l, c"XISelectEvents")?,
                query_device: function(l, c"XIQueryDevice")?,
                free_device_info: function(l, c"XIFreeDeviceInfo")?,
                grab_device: function(l, c"XIGrabDevice")?,
                ungrab_device: function(l, c"XIUngrabDevice")?,
            }
        };
        let mut libraries = vec![x11, xi];
        let xcursor = Library::open(&["libXcursor.so.1", "libXcursor.so"]).ok();
        // SAFETY: Xcursor.h's signature.
        let xcursor_load = xcursor.as_ref().and_then(|l| unsafe { function(l, c"XcursorLibraryLoadCursor") }.ok());
        let randr_library = Library::open(&["libXrandr.so.2", "libXrandr.so"]).ok();
        let randr = randr_library.as_ref().and_then(|l| {
            // SAFETY: as above, Xrandr.h's signatures.
            unsafe {
                Some(RandrFunctions {
                    query_extension: function(l, c"XRRQueryExtension").ok()?,
                    select_input: function(l, c"XRRSelectInput").ok()?,
                    get_screen_resources_current: function(l, c"XRRGetScreenResourcesCurrent").ok()?,
                    free_screen_resources: function(l, c"XRRFreeScreenResources").ok()?,
                    get_crtc_info: function(l, c"XRRGetCrtcInfo").ok()?,
                    free_crtc_info: function(l, c"XRRFreeCrtcInfo").ok()?,
                })
            }
        });
        let xext = Library::open(&["libXext.so.6", "libXext.so"]).ok();
        let shape = xext.as_ref().and_then(|l| {
            // SAFETY: as above, shape.h's signatures.
            unsafe {
                Some(ShapeFunctions {
                    query_extension: function(l, c"XShapeQueryExtension").ok()?,
                    combine_mask: function(l, c"XShapeCombineMask").ok()?,
                    combine_rectangles: function(l, c"XShapeCombineRectangles").ok()?,
                })
            }
        });
        let xcb = Library::open(&["libX11-xcb.so.1", "libX11-xcb.so"]).ok();
        // SAFETY: Xlib-xcb.h's signature (an xcb_connection_t* as a pointer).
        let xcb_connection = xcb.as_ref().and_then(|l| unsafe { function(l, c"XGetXCBConnection") }.ok());
        libraries.extend([xcursor, randr_library, xext, xcb].into_iter().flatten());
        // SAFETY: XInitThreads before any other of Xlib's calls in the process (the library only just loaded, and
        // only this module calls it), once (the libraries are loaded once).
        if unsafe { (x.init_threads)() } == 0 {
            return Err("Xlib can't be used from several threads".into());
        }
        Ok(Xlib { x, xi: xi_functions, xcursor_load, randr, shape, xcb_connection, _libraries: libraries })
    }
}

/// The libraries, loaded the first time they are asked for, Xlib made safe to use from several threads
/// (`XInitThreads`: the GPU driver's threads use the display too); they stay loaded for the process (Xlib keeps
/// state of its own, its threads' locks among it). Why not if libX11 or libXi isn't there.
fn xlib() -> Result<&'static Xlib, String> {
    static XLIB: OnceLock<Result<Xlib, String>> = OnceLock::new();
    XLIB.get_or_init(Xlib::load).as_ref().map_err(Clone::clone)
}

/// An X error is the request's own failure, ignored where it is made (C++'s `ignoreError`).
unsafe extern "C" fn ignore_error(_: *mut RawDisplay, _: *mut c_void) -> c_int {
    0
}

// ---- Events ---------------------------------------------------------------------------------------------------------

/// An X event (Xlib's `XEvent`), as it came: what [`decode`](XEvent::decode) reads, what the input method filters.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct XEvent(RawEvent);

/// A key pressed or let go (Xlib's `XKeyEvent`): what the input method and Xlib's lookups take.
#[derive(Clone, Copy)]
pub struct KeyEvent(XKeyEvent);

impl KeyEvent {
    /// The X keycode (evdev's + 8; 0: an input method's commit, a press of no key).
    pub fn keycode(&self) -> u32 {
        self.0.keycode
    }
    /// The modifiers and the keyboard group (bits 13 and 14) held before it.
    pub fn state(&self) -> u32 {
        self.0.state
    }
    pub fn time(&self) -> Time {
        self.0.time
    }
    pub fn is_press(&self) -> bool {
        self.0.type_ == KEY_PRESS
    }

    /// A key event made up, as the server would send it to `window` (tests: the keyboard and the input method's
    /// lookups without a key pressed).
    #[cfg(test)]
    pub fn synthetic(display: &Display, window: Window, keycode: u32, state: u32, press: bool) -> KeyEvent {
        KeyEvent(XKeyEvent {
            type_: if press { KEY_PRESS } else { KEY_RELEASE },
            serial: 0,
            send_event: 0,
            display: display.raw,
            window,
            root: display.root,
            subwindow: NONE,
            time: CURRENT_TIME,
            x: 0,
            y: 0,
            x_root: 0,
            y_root: 0,
            state,
            keycode,
            same_screen: 1,
        })
    }
}

/// An X event's parts, as the platform handles them.
#[derive(Clone, Copy)]
pub enum Decoded {
    Key(KeyEvent),
    Focus {
        focus_in: bool,
        mode: i32,
        detail: i32,
    },
    Expose {
        count: i32,
    },
    Configure {
        window: Window,
        width: i32,
        height: i32,
        x: i32,
        y: i32,
        send_event: bool,
    },
    Property {
        window: Window,
        atom: Atom,
        state: i32,
    },
    ClientMessage {
        /// The window it is about (XDND's target, whichever window it was sent to).
        window: Window,
        message_type: Atom,
        data: [c_long; 5],
    },
    SelectionRequest {
        requestor: Window,
        selection: Atom,
        target: Atom,
        property: Atom,
        time: Time,
    },
    SelectionNotify {
        requestor: Window,
        selection: Atom,
        property: Atom,
    },
    SelectionClear {
        selection: Atom,
    },
    Mapping {
        request: i32,
    },
    /// An extension's (XInput 2's: [`Display::xi_event`]; XRandR's: [`Display::is_randr_event`]).
    Other,
}

impl XEvent {
    /// Its type.
    pub fn kind(&self) -> c_int {
        // SAFETY: every X event begins with its type (an int), whatever its struct.
        unsafe { self.0.type_ }
    }

    /// Its parts, as its type says.
    pub fn decode(&self) -> Decoded {
        let e = &self.0;
        // SAFETY: each arm reads the union as the struct Xlib wrote for the event's type (Xlib.h's table).
        unsafe {
            match e.type_ {
                KEY_PRESS | KEY_RELEASE => Decoded::Key(KeyEvent(e.key)),
                FOCUS_IN | FOCUS_OUT => {
                    Decoded::Focus { focus_in: e.type_ == FOCUS_IN, mode: e.focus.mode, detail: e.focus.detail }
                }
                EXPOSE => Decoded::Expose { count: e.expose.count },
                CONFIGURE_NOTIFY => Decoded::Configure {
                    window: e.configure.window,
                    width: e.configure.width,
                    height: e.configure.height,
                    x: e.configure.x,
                    y: e.configure.y,
                    send_event: e.configure.send_event != 0,
                },
                PROPERTY_NOTIFY => {
                    Decoded::Property { window: e.property.window, atom: e.property.atom, state: e.property.state }
                }
                CLIENT_MESSAGE => Decoded::ClientMessage {
                    window: e.client.window,
                    message_type: e.client.message_type,
                    data: e.client.data,
                },
                SELECTION_REQUEST => Decoded::SelectionRequest {
                    requestor: e.selection_request.requestor,
                    selection: e.selection_request.selection,
                    target: e.selection_request.target,
                    property: e.selection_request.property,
                    time: e.selection_request.time,
                },
                SELECTION_NOTIFY => Decoded::SelectionNotify {
                    requestor: e.selection.requestor,
                    selection: e.selection.selection,
                    property: e.selection.property,
                },
                SELECTION_CLEAR => Decoded::SelectionClear { selection: e.selection_clear.selection },
                MAPPING_NOTIFY => Decoded::Mapping { request: e.mapping.request },
                _ => Decoded::Other,
            }
        }
    }

    fn zeroed() -> XEvent {
        XEvent(RawEvent { pad: [0; 24] })
    }
}

/// What a pointer, a touch or a wheel did, as XInput 2 tells it (its `XIDeviceEvent`'s parts).
#[derive(Clone, PartialEq, Debug)]
pub struct XiDevice {
    /// `XI_Motion`, `XI_ButtonPress`, `XI_ButtonRelease`, `XI_TouchBegin`, `XI_TouchUpdate`, `XI_TouchEnd`.
    pub evtype: c_int,
    pub time: Time,
    /// The (master) device it is of, and the physical one it came from.
    pub device: i32,
    pub source: i32,
    /// The button, or the touch's id.
    pub detail: i32,
    /// In the window, its pixels; on the screen.
    pub x: f64,
    pub y: f64,
    pub root_x: f64,
    pub root_y: f64,
    pub flags: i32,
    /// The modifiers in effect (an X state mask).
    pub mods: u32,
    /// The valuators it has a value for: their numbers and values.
    pub valuators: Vec<(usize, f64)>,
}

/// An XInput 2 event the platform takes.
#[derive(Clone, PartialEq, Debug)]
pub enum XiEvent {
    /// Devices came, went or changed.
    Hierarchy,
    Device(XiDevice),
    /// The pointer came into the window or left it.
    Crossing {
        enter: bool,
        source: i32,
        x: f64,
        y: f64,
        mode: i32,
        mods: u32,
    },
    /// A touchpad's pinch (`XI_GesturePinchBegin` / `Update` / `End`): the scale since it began, the angle since the
    /// last update (degrees, clockwise).
    Pinch {
        evtype: c_int,
        x: f64,
        y: f64,
        scale: f64,
        delta_angle: f64,
        mods: u32,
    },
    Other,
}

/// A device's classes as the platform needs them.
#[derive(Clone, PartialEq, Debug)]
pub struct DeviceInfo {
    pub id: i32,
    /// `XISlavePointer` and the like.
    pub use_: i32,
    /// Its scroll valuators: number, vertical, the increment a notch is.
    pub scroll: Vec<(usize, bool, f64)>,
    /// A touch screen (a direct touch class).
    pub direct_touch: bool,
}

/// A display's CRTC (where a monitor shows the screen), and its mode's timings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Crtc {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Its mode's pixel clock (Hz), total width and height, flags (`RR_Interlace`, `RR_DoubleScan`); none if it
    /// shows nothing.
    pub mode: Option<(c_ulong, u32, u32, c_ulong)>,
}

/// A window property's value.
#[derive(Clone, PartialEq, Debug)]
pub struct Property {
    /// Its type.
    pub kind: Atom,
    /// 8, 16 or 32.
    pub format: i32,
    /// Its bytes (32-bit items as longs, as Xlib gives them).
    pub bytes: Vec<u8>,
}

impl Property {
    /// Its 32-bit items (atoms, windows, numbers).
    pub fn longs(&self) -> Vec<c_long> {
        let (longs, _) = self.bytes.as_chunks::<{ std::mem::size_of::<c_long>() }>();
        longs.iter().map(|&c| c_long::from_ne_bytes(c)).collect()
    }
}

// ---- The display ----------------------------------------------------------------------------------------------------

/// A connection to an X server (usable from several threads: the GPU driver's use it too), XInput 2.2 or later on
/// it, X errors ignored; closed when dropped (after everything holding it).
pub struct Display {
    x: &'static Xlib,
    raw: D,
    root: Window,
    xi_opcode: c_int,
    xi_minor: c_int,
    /// XRandR's first event, if it is there.
    randr_event: Option<c_int>,
}

impl Display {
    /// The display `$DISPLAY` names, with XInput 2.2 or later; why not if it can't be had.
    pub fn open() -> Result<Display, String> {
        let x = xlib()?;
        // SAFETY: a plain open of the environment's display (Xlib's threads set up as it was loaded).
        let raw = unsafe { (x.x.open_display)(std::ptr::null()) };
        if raw.is_null() {
            return Err("the X display can't be opened".into());
        }
        // SAFETY: the display is open; the handler only returns 0 (it never unwinds).
        let root = unsafe {
            (x.x.set_error_handler)(Some(ignore_error));
            (x.x.root_window)(raw, (x.x.default_screen)(raw))
        };
        let mut display = Display { x, raw, root, xi_opcode: 0, xi_minor: 0, randr_event: None };
        let (mut event, mut error, mut major, mut minor) = (0, 0, 2, 4);
        // SAFETY: the display is open; the out pointers are valid locals; the name a valid C string.
        let xi = unsafe {
            (x.x.query_extension)(raw, c"XInputExtension".as_ptr(), &mut display.xi_opcode, &mut event, &mut error) != 0
                && (x.xi.query_version)(raw, &mut major, &mut minor) == SUCCESS
        };
        if !xi || major < 2 || (major == 2 && minor < 2) {
            return Err("the X server has no XInput 2.2".into());
        }
        display.xi_minor = minor;
        if let Some(rr) = &x.randr {
            let (mut event, mut error) = (0, 0);
            // SAFETY: the display is open; the out pointers are valid locals.
            if unsafe { (rr.query_extension)(raw, &mut event, &mut error) } != 0 {
                display.randr_event = Some(event);
                // SAFETY: as above; the root window is the display's.
                unsafe { (rr.select_input)(raw, root, RR_SCREEN_CHANGE_NOTIFY_MASK | RR_CRTC_CHANGE_NOTIFY_MASK) };
            }
        }
        Ok(display)
    }

    pub fn root(&self) -> Window {
        self.root
    }

    /// XInput 2's minor version (2 to 4: touchpad pinches from 4).
    pub fn xi_minor(&self) -> i32 {
        self.xi_minor
    }

    /// The connection's socket (what the loop polls).
    pub fn connection_fd(&self) -> RawFd {
        // SAFETY: the display is open.
        unsafe { (self.x.x.connection_number)(self.raw) }
    }

    /// Whether the server closed the connection (its socket at its end): C++'s POLLHUP, told without reading.
    pub fn connection_closed(&self) -> bool {
        let mut byte = 0u8;
        // SAFETY: a peek of one byte into a valid local from the display's open socket, not waiting: nothing is
        // taken from Xlib's stream.
        let n = unsafe {
            libc::recv(self.connection_fd(), (&mut byte as *mut u8).cast(), 1, libc::MSG_PEEK | libc::MSG_DONTWAIT)
        };
        if n == 0 {
            return true;
        }
        n < 0 && {
            let e = std::io::Error::last_os_error();
            !matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted)
        }
    }

    /// The atoms named `names`, made if they aren't (in one request).
    pub fn intern_atoms(&self, names: &[&CStr]) -> Vec<Atom> {
        let mut pointers: Vec<*mut c_char> = names.iter().map(|n| n.as_ptr().cast_mut()).collect();
        let mut atoms = vec![NONE; names.len()];
        // SAFETY: `pointers` are valid C strings (Xlib only reads them), `atoms` as many writable atoms.
        unsafe {
            (self.x.x.intern_atoms)(self.raw, pointers.as_mut_ptr(), names.len() as c_int, 0, atoms.as_mut_ptr())
        };
        atoms
    }

    /// The atom named `name`, made if it isn't (NONE for a name with a NUL).
    pub fn intern_atom(&self, name: &str) -> Atom {
        match std::ffi::CString::new(name) {
            Ok(name) => self.intern_atoms(&[name.as_c_str()])[0],
            Err(_) => NONE,
        }
    }

    /// `atom`'s name; none for an atom that isn't one (its error ignored).
    pub fn atom_name(&self, atom: Atom) -> Option<String> {
        if atom == NONE {
            return None;
        }
        // SAFETY: the display is open; the result is Xlib's allocation (or null for a bad atom), read then freed.
        unsafe {
            let name = (self.x.x.get_atom_name)(self.raw, atom);
            if name.is_null() {
                return None;
            }
            let owned = CStr::from_ptr(name).to_string_lossy().into_owned();
            (self.x.x.free)(name.cast());
            Some(owned)
        }
    }

    /// A top-level window of `width` by `height` pixels, the screen's visual, its contents kept at the top left
    /// while it is resized, `events` selected on it; none if it can't be made.
    pub fn create_window(&self, width: u32, height: u32, events: c_long) -> Option<Window> {
        // SAFETY: an all-zero XSetWindowAttributes is a valid one (only the fields the mask names are read).
        let mut attributes: XSetWindowAttributes = unsafe { std::mem::zeroed() };
        attributes.event_mask = events;
        attributes.bit_gravity = NORTH_WEST_GRAVITY;
        // SAFETY: the display is open; `attributes` is valid for the fields the mask names; the visual is the
        // parent's (null: CopyFromParent).
        let w = unsafe {
            (self.x.x.create_window)(
                self.raw,
                self.root,
                0,
                0,
                width.max(1),
                height.max(1),
                0,
                COPY_FROM_PARENT,
                INPUT_OUTPUT,
                std::ptr::null_mut(),
                CW_EVENT_MASK | CW_BIT_GRAVITY,
                &mut attributes,
            )
        };
        (w != NONE).then_some(w)
    }

    pub fn destroy_window(&self, window: Window) {
        // SAFETY: the display is open (a window not there is an X error, ignored).
        unsafe { (self.x.x.destroy_window)(self.raw, window) };
    }

    pub fn map_window(&self, window: Window) {
        // SAFETY: as above.
        unsafe { (self.x.x.map_window)(self.raw, window) };
    }

    pub fn resize_window(&self, window: Window, width: u32, height: u32) {
        // SAFETY: as above.
        unsafe { (self.x.x.resize_window)(self.raw, window, width.max(1), height.max(1)) };
    }

    pub fn flush(&self) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.flush)(self.raw) };
    }

    /// Replaces `window`'s `property` with 8-bit items (text) of type `kind`.
    pub fn set_property_8(&self, window: Window, property: Atom, kind: Atom, bytes: &[u8]) {
        // SAFETY: `bytes` is `len` readable bytes (format 8), copied by Xlib into the request.
        unsafe {
            (self.x.x.change_property)(
                self.raw,
                window,
                property,
                kind,
                8,
                PROP_MODE_REPLACE,
                bytes.as_ptr(),
                bytes.len().min(c_int::MAX as usize) as c_int,
            )
        };
    }

    /// Replaces `window`'s `property` with 32-bit items (atoms, numbers: longs, as Xlib takes them) of type `kind`.
    pub fn set_property_32(&self, window: Window, property: Atom, kind: Atom, items: &[c_long]) {
        // SAFETY: `items` is `len` readable longs (format 32: Xlib reads longs), copied into the request.
        unsafe {
            (self.x.x.change_property)(
                self.raw,
                window,
                property,
                kind,
                32,
                PROP_MODE_REPLACE,
                items.as_ptr().cast(),
                items.len() as c_int,
            )
        };
    }

    pub fn delete_property(&self, window: Window, property: Atom) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.delete_property)(self.raw, window, property) };
    }

    /// `window`'s `property`, all of it (deleted after if `delete`); none if it isn't there.
    pub fn property(&self, window: Window, property: Atom, delete: bool) -> Option<Property> {
        let (mut kind, mut format, mut count, mut after) = (NONE, 0, 0 as c_ulong, 0 as c_ulong);
        let mut data: *mut c_uchar = std::ptr::null_mut();
        // SAFETY: the display is open; the out pointers are valid locals; `data` is set to Xlib's allocation (or
        // null), freed below.
        let status = unsafe {
            (self.x.x.get_window_property)(
                self.raw,
                window,
                property,
                0,
                0x1fff_ffff,
                Bool::from(delete),
                ANY_PROPERTY_TYPE,
                &mut kind,
                &mut format,
                &mut count,
                &mut after,
                &mut data,
            )
        };
        let unit = if format == 32 { std::mem::size_of::<c_long>() } else { (format / 8).max(0) as usize };
        let bytes = if data.is_null() || status != SUCCESS || kind == NONE {
            None
        } else {
            // SAFETY: Xlib's buffer holds `count` items of `unit` bytes each.
            Some(unsafe { std::slice::from_raw_parts(data, count as usize * unit) }.to_vec())
        };
        if !data.is_null() {
            // SAFETY: Xlib's allocation, freed once, not used after.
            unsafe { (self.x.x.free)(data.cast()) };
        }
        if status != SUCCESS || kind == NONE {
            return None;
        }
        Some(Property { kind, format, bytes: bytes.unwrap_or_default() })
    }

    /// The window manager's protocols `window` takes part in (`WM_PROTOCOLS`).
    pub fn set_wm_protocols(&self, window: Window, protocols: &[Atom]) {
        let mut protocols = protocols.to_vec();
        // SAFETY: `protocols` is `len` atoms, read by Xlib only.
        unsafe { (self.x.x.set_wm_protocols)(self.raw, window, protocols.as_mut_ptr(), protocols.len() as c_int) };
    }

    /// `WM_CLASS`: its instance's name and its class.
    pub fn set_class_hint(&self, window: Window, name: &CStr, class: &CStr) {
        let mut hint = XClassHint { res_name: name.as_ptr().cast_mut(), res_class: class.as_ptr().cast_mut() };
        // SAFETY: `hint` holds valid C strings that Xlib only reads, for the call.
        unsafe { (self.x.x.set_class_hint)(self.raw, window, &mut hint) };
    }

    pub fn select_input(&self, window: Window, events: c_long) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.select_input)(self.raw, window, events) };
    }

    /// XInput 2's events `types` of `device` (`XIAllDevices`, `XIAllMasterDevices`) selected on `window`.
    pub fn xi_select_events(&self, window: Window, device: c_int, types: &[c_int]) {
        let mut mask = [0u8; XI_MASK_LEN];
        for &t in types {
            mask[(t >> 3) as usize] |= 1 << (t & 7);
        }
        let mut m = XiEventMask { deviceid: device, mask_len: XI_MASK_LEN as c_int, mask: mask.as_mut_ptr() };
        // SAFETY: `m` points at `mask`, alive for the call.
        unsafe { (self.x.xi.select_events)(self.raw, window, &mut m, 1) };
    }

    /// The devices XInput 2 knows, with their scroll valuators and whether they are touch screens.
    pub fn xi_devices(&self) -> Vec<DeviceInfo> {
        let mut count = 0;
        // SAFETY: the display is open; `count` a valid local.
        let devices = unsafe { (self.x.xi.query_device)(self.raw, XI_ALL_DEVICES, &mut count) };
        if devices.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0..count.max(0) as usize {
            // SAFETY: libXi's array holds `count` device infos.
            let d = unsafe { &*devices.add(i) };
            let mut info = DeviceInfo { id: d.deviceid, use_: d.use_, scroll: Vec::new(), direct_touch: false };
            for c in 0..d.num_classes.max(0) as usize {
                // SAFETY: the device's `num_classes` class pointers, each to a class info whose type says its struct.
                let class = unsafe { *d.classes.add(c) };
                if class.is_null() {
                    continue;
                }
                // SAFETY: as above.
                match unsafe { (*class).type_ } {
                    XI_SCROLL_CLASS => {
                        // SAFETY: a scroll class's info is an XIScrollClassInfo.
                        let s = unsafe { &*class.cast::<XiScrollClassInfo>() };
                        let increment = if s.increment != 0.0 { s.increment } else { 1.0 };
                        info.scroll.push((
                            s.number.max(0) as usize,
                            s.scroll_type == XI_SCROLL_TYPE_VERTICAL,
                            increment,
                        ));
                    }
                    XI_TOUCH_CLASS => {
                        // SAFETY: a touch class's info is an XITouchClassInfo.
                        let t = unsafe { &*class.cast::<XiTouchClassInfo>() };
                        info.direct_touch |= t.mode == XI_DIRECT_TOUCH;
                    }
                    _ => {}
                }
            }
            out.push(info);
        }
        // SAFETY: libXi's array, freed once, not used after.
        unsafe { (self.x.xi.free_device_info)(devices) };
        out
    }

    /// The XInput 2 event `e` is, if it is one: its data fetched, read and freed.
    pub fn xi_event(&self, e: &XEvent) -> Option<XiEvent> {
        if e.kind() != GENERIC_EVENT {
            return None;
        }
        // SAFETY: a GenericEvent is an XGenericEventCookie.
        let mut cookie = unsafe { e.0.cookie };
        if cookie.extension != self.xi_opcode {
            return None;
        }
        // SAFETY: the cookie of an event read from this display; its data, once got, is freed below.
        if unsafe { (self.x.x.get_event_data)(self.raw, &mut cookie) } == 0 || cookie.data.is_null() {
            return Some(XiEvent::Other);
        }
        // SAFETY: libXi's data for the cookie's event type is the struct XInput2.h gives that type.
        let decoded = unsafe { decode_xi(cookie.evtype, cookie.data) };
        // SAFETY: the data got above, freed once.
        unsafe { (self.x.x.free_event_data)(self.raw, &mut cookie) };
        Some(decoded)
    }

    /// Whether `e` is XRandR's (the screen's configuration or a CRTC changed).
    pub fn is_randr_event(&self, e: &XEvent) -> bool {
        self.randr_event.is_some_and(|base| e.kind() == base + RR_SCREEN_CHANGE_NOTIFY || e.kind() == base + RR_NOTIFY)
    }

    /// The screen's CRTCs (XRandR's; none without it).
    pub fn crtcs(&self) -> Vec<Crtc> {
        let (Some(rr), Some(_)) = (&self.x.randr, self.randr_event) else { return Vec::new() };
        // SAFETY: the display is open, XRandR on it; the root window is its.
        let resources = unsafe { (rr.get_screen_resources_current)(self.raw, self.root) };
        if resources.is_null() {
            return Vec::new();
        }
        // SAFETY: libXrandr's resources, alive until freed below.
        let r = unsafe { &*resources };
        let modes: &[XrrModeInfo] = if r.modes.is_null() || r.nmode <= 0 {
            &[]
        } else {
            // SAFETY: `nmode` modes there.
            unsafe { std::slice::from_raw_parts(r.modes, r.nmode as usize) }
        };
        let mut out = Vec::new();
        for i in 0..r.ncrtc.max(0) as usize {
            // SAFETY: `ncrtc` CRTC ids there.
            let id = unsafe { *r.crtcs.add(i) };
            // SAFETY: the display, the resources and a CRTC of theirs; the info is freed below.
            let crtc = unsafe { (rr.get_crtc_info)(self.raw, resources, id) };
            if crtc.is_null() {
                continue;
            }
            // SAFETY: libXrandr's info, alive until freed below.
            let c = unsafe { &*crtc };
            let mode = (c.mode != NONE)
                .then(|| modes.iter().find(|m| m.id == c.mode))
                .flatten()
                .map(|m| (m.dot_clock, m.h_total, m.v_total, m.mode_flags));
            out.push(Crtc { x: c.x, y: c.y, width: c.width, height: c.height, mode });
            // SAFETY: freed once, not used after.
            unsafe { (rr.free_crtc_info)(crtc) };
        }
        // SAFETY: as above.
        unsafe { (rr.free_screen_resources)(resources) };
        out
    }

    /// How many events are waiting (read from the connection if none are queued).
    pub fn pending(&self) -> i32 {
        // SAFETY: the display is open.
        unsafe { (self.x.x.pending)(self.raw) }
    }

    /// The next event (waiting for one if none is queued).
    pub fn next_event(&self) -> XEvent {
        let mut e = XEvent::zeroed();
        // SAFETY: `e` is a writable XEvent.
        unsafe { (self.x.x.next_event)(self.raw, &mut e.0) };
        e
    }

    /// Whether the input method took `e` (it is then not the app's).
    pub fn filter_event(&self, e: &mut XEvent) -> bool {
        // SAFETY: an event read from this display; None: the input context's focus window.
        unsafe { (self.x.x.filter_event)(&mut e.0, NONE) != 0 }
    }

    /// The first queued event `matches` takes, taken out of the queue (the others stay); none yet.
    pub fn check_if_event(&self, matches: &mut dyn FnMut(&XEvent) -> bool) -> Option<XEvent> {
        unsafe extern "C" fn trampoline(_: *mut RawDisplay, e: *mut RawEvent, arg: XPointer) -> Bool {
            // SAFETY: `arg` is the `&mut &mut dyn FnMut` check_if_event passed, alive for its call; `e` an event of
            // Xlib's queue, an XEvent (XEvent is RawEvent, transparent).
            let (f, e) = unsafe { (&mut *arg.cast::<&mut dyn FnMut(&XEvent) -> bool>(), &*e.cast::<XEvent>()) };
            // (Never unwinding into Xlib.)
            Bool::from(std::panic::catch_unwind(AssertUnwindSafe(|| f(e))).unwrap_or(false))
        }
        let mut e = XEvent::zeroed();
        let mut f: &mut dyn FnMut(&XEvent) -> bool = matches;
        // SAFETY: the predicate gets `&mut f`, alive for the call; Xlib calls it only during it.
        let found = unsafe {
            (self.x.x.check_if_event)(
                self.raw,
                &mut e.0,
                Some(trampoline),
                (&mut f as *mut &mut dyn FnMut(&XEvent) -> bool).cast(),
            )
        };
        (found != 0).then_some(e)
    }

    /// `window`'s position in `to` (the root window: on the screen); none if they aren't on one screen.
    pub fn translate_coordinates(&self, from: Window, to: Window, x: i32, y: i32) -> Option<(i32, i32)> {
        let (mut tx, mut ty, mut child) = (0, 0, NONE);
        // SAFETY: the out pointers are valid locals.
        let same = unsafe { (self.x.x.translate_coordinates)(self.raw, from, to, x, y, &mut tx, &mut ty, &mut child) };
        (same != 0).then_some((tx, ty))
    }

    /// Sends a client message (format 32) to `destination` about `window`, to the clients selecting `mask` there
    /// (none: its owner).
    pub fn send_client_message(
        &self,
        destination: Window,
        window: Window,
        kind: Atom,
        data: [c_long; 5],
        mask: c_long,
    ) {
        let mut e = XEvent::zeroed();
        e.0.client = XClientMessageEvent {
            type_: CLIENT_MESSAGE,
            serial: 0,
            send_event: 1,
            display: self.raw,
            window,
            message_type: kind,
            format: 32,
            data,
        };
        // SAFETY: a filled client message event, read by Xlib only.
        unsafe { (self.x.x.send_event)(self.raw, destination, 0, mask, &mut e.0) };
    }

    /// Tells `requestor` its selection request was answered into `property` (none: refused).
    pub fn send_selection_notify(&self, requestor: Window, selection: Atom, target: Atom, property: Atom, time: Time) {
        let mut e = XEvent::zeroed();
        e.0.selection = XSelectionEvent {
            type_: SELECTION_NOTIFY,
            serial: 0,
            send_event: 1,
            display: self.raw,
            requestor,
            selection,
            target,
            property,
            time,
        };
        // SAFETY: a filled selection event, read by Xlib only.
        unsafe { (self.x.x.send_event)(self.raw, requestor, 0, NO_EVENT_MASK, &mut e.0) };
    }

    pub fn set_selection_owner(&self, selection: Atom, owner: Window, time: Time) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.set_selection_owner)(self.raw, selection, owner, time) };
    }

    pub fn selection_owner(&self, selection: Atom) -> Window {
        // SAFETY: the display is open.
        unsafe { (self.x.x.get_selection_owner)(self.raw, selection) }
    }

    /// Asks `selection`'s owner for it as `target`, into `requestor`'s `property`.
    pub fn convert_selection(&self, selection: Atom, target: Atom, property: Atom, requestor: Window, time: Time) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.convert_selection)(self.raw, selection, target, property, requestor, time) };
    }

    /// The most a request carries, in bytes (BIG-REQUESTS' if the server has it).
    pub fn max_request_bytes(&self) -> usize {
        // SAFETY: the display is open.
        let most = unsafe {
            let extended = (self.x.x.extended_max_request_size)(self.raw);
            if extended > 0 { extended } else { (self.x.x.max_request_size)(self.raw) }
        };
        most.max(0) as usize * 4
    }

    /// A held key's repeats come as presses alone (no release between them).
    pub fn set_detectable_auto_repeat(&self) {
        // SAFETY: the display is open; the "supported" result isn't asked for.
        unsafe { (self.x.x.xkb_set_detectable_auto_repeat)(self.raw, 1, std::ptr::null_mut()) };
    }

    /// Xlib's keyboard mapping read again after a `MappingNotify` `e`.
    pub fn refresh_keyboard_mapping(&self, e: &XEvent) {
        if e.kind() != MAPPING_NOTIFY {
            return;
        }
        // SAFETY: a MappingNotify is an XMappingEvent.
        let mut mapping = unsafe { e.0.mapping };
        // SAFETY: a mapping event read from this display.
        unsafe { (self.x.x.refresh_keyboard_mapping)(&mut mapping) };
    }

    /// The keysym of `key` at `index` of its core mapping (0: unshifted).
    pub fn lookup_keysym(&self, key: &KeyEvent, index: i32) -> KeySym {
        let mut k = key.0;
        // SAFETY: a key event of this display's.
        unsafe { (self.x.x.lookup_keysym)(&mut k, index) }
    }

    /// What `key` types without an input method (Xlib's Latin-1 lookup), as UTF-8.
    pub fn lookup_latin1(&self, key: &KeyEvent) -> String {
        let mut k = key.0;
        let mut buffer = [0u8; 64];
        let mut sym: KeySym = 0;
        // SAFETY: a key event of this display's; the buffer is 64 writable bytes; no compose status.
        let n = unsafe {
            (self.x.x.lookup_string)(
                &mut k,
                buffer.as_mut_ptr().cast(),
                buffer.len() as c_int,
                &mut sym,
                std::ptr::null_mut(),
            )
        };
        buffer[..n.clamp(0, 64) as usize].iter().map(|&b| char::from(b)).collect()
    }

    /// The cursor shape `shape` of X's cursor font.
    pub fn font_cursor(&self, shape: c_uint) -> Cursor {
        // SAFETY: the display is open.
        unsafe { (self.x.x.create_font_cursor)(self.raw, shape) }
    }

    /// The desktop theme's cursor named `name` (libXcursor's); none without it.
    pub fn theme_cursor(&self, name: &CStr) -> Option<Cursor> {
        let load = self.x.xcursor_load?;
        // SAFETY: the display is open; `name` a valid C string.
        let c = unsafe { load(self.raw, name.as_ptr()) };
        (c != NONE).then_some(c)
    }

    pub fn define_cursor(&self, window: Window, cursor: Cursor) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.define_cursor)(self.raw, window, cursor) };
    }

    pub fn free_cursor(&self, cursor: Cursor) {
        // SAFETY: the display is open; a cursor made on it.
        unsafe { (self.x.x.free_cursor)(self.raw, cursor) };
    }

    /// The core keyboard's keymap and a state following it, through xkbcommon-x11 (on the connection's xcb side:
    /// libX11-xcb), if they can be had.
    pub fn keyboard_keymap(&self, context: &Context) -> Option<(Keymap, KeyboardState)> {
        let get = self.x.xcb_connection?;
        // SAFETY: the display is open; its xcb connection lives as long as it.
        let connection = unsafe { get(self.raw) };
        if connection.is_null() {
            return None;
        }
        // SAFETY: the connection is live (the display is open while `self` is borrowed); the extension is set up on
        // it before the device is asked for.
        unsafe {
            context.x11_setup_xkb_extension(connection)?;
            let device = context.x11_core_keyboard(connection)?;
            context.x11_keymap(connection, device)
        }
    }
}

/// A window's place and size on its parent (border outside), and whether it is shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub border: i32,
    pub viewable: bool,
}

impl WindowGeometry {
    /// Whether (x, y) on its parent is in it (its border too).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let b = self.border;
        x >= self.x && y >= self.y && x < self.x + self.width + 2 * b && y < self.y + self.height + 2 * b
    }
}

/// A window of the screen's ARGB visual (or of its default one) and its colormap (none: the parent's).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VisualWindow {
    pub window: Window,
    pub depth: u32,
    colormap: Colormap,
}

impl Display {
    /// XInput 2's grab of `device` (a master pointer) for `window` from `time`, showing `cursor`, its `types` of
    /// events reported to the window (asynchronously, not to their own windows); again by the same client: the
    /// cursor changed. Whether it was taken.
    pub fn xi_grab_device(&self, device: c_int, window: Window, time: Time, cursor: Cursor, types: &[c_int]) -> bool {
        let mut mask = [0u8; XI_MASK_LEN];
        for &t in types {
            mask[(t >> 3) as usize] |= 1 << (t & 7);
        }
        let mut m = XiEventMask { deviceid: device, mask_len: XI_MASK_LEN as c_int, mask: mask.as_mut_ptr() };
        // SAFETY: the display is open; `m` points at `mask`, alive for the call.
        let status = unsafe {
            (self.x.xi.grab_device)(self.raw, device, window, time, cursor, GRAB_MODE_ASYNC, GRAB_MODE_ASYNC, 0, &mut m)
        };
        status == SUCCESS
    }

    pub fn xi_ungrab_device(&self, device: c_int, time: Time) {
        // SAFETY: the display is open.
        unsafe { (self.x.xi.ungrab_device)(self.raw, device, time) };
    }

    /// The core keyboard grabbed for `window` (its keys reported to it) from `time`; whether it was taken.
    pub fn grab_keyboard(&self, window: Window, time: Time) -> bool {
        // SAFETY: the display is open.
        unsafe { (self.x.x.grab_keyboard)(self.raw, window, 0, GRAB_MODE_ASYNC, GRAB_MODE_ASYNC, time) == GRAB_SUCCESS }
    }

    pub fn ungrab_keyboard(&self, time: Time) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.ungrab_keyboard)(self.raw, time) };
    }

    /// `window`'s children, bottom to top (the stacking order).
    pub fn children(&self, window: Window) -> Vec<Window> {
        let (mut root, mut parent, mut children, mut count) = (NONE, NONE, std::ptr::null_mut(), 0);
        // SAFETY: the out pointers are valid locals; `children` is Xlib's allocation (or null), freed below.
        let ok = unsafe { (self.x.x.query_tree)(self.raw, window, &mut root, &mut parent, &mut children, &mut count) };
        let mut out = Vec::new();
        if ok != 0 && !children.is_null() {
            // SAFETY: `count` windows there.
            out.extend_from_slice(unsafe { std::slice::from_raw_parts(children, count as usize) });
        }
        if !children.is_null() {
            // SAFETY: Xlib's allocation, freed once.
            unsafe { (self.x.x.free)(children.cast()) };
        }
        out
    }

    /// `window`'s place, size and whether it is shown; none if it isn't there.
    pub fn geometry(&self, window: Window) -> Option<WindowGeometry> {
        // SAFETY: an all-zero XWindowAttributes is a valid out struct.
        let mut a: XWindowAttributes = unsafe { std::mem::zeroed() };
        // SAFETY: the display is open; `a` a valid out struct.
        if unsafe { (self.x.x.get_window_attributes)(self.raw, window, &mut a) } == 0 {
            return None;
        }
        Some(WindowGeometry {
            x: a.x,
            y: a.y,
            width: a.width,
            height: a.height,
            border: a.border_width,
            viewable: a.map_state == IS_VIEWABLE,
        })
    }

    /// Where (x, y) of `from` is on `to`, and `to`'s child there (none: none); none if they aren't on one screen.
    pub fn child_at(&self, from: Window, to: Window, x: i32, y: i32) -> Option<(i32, i32, Window)> {
        let (mut tx, mut ty, mut child) = (0, 0, NONE);
        // SAFETY: the out pointers are valid locals.
        let same = unsafe { (self.x.x.translate_coordinates)(self.raw, from, to, x, y, &mut tx, &mut ty, &mut child) };
        (same != 0).then_some((tx, ty, child))
    }

    /// The default screen's number.
    pub fn screen(&self) -> c_int {
        // SAFETY: the display is open.
        unsafe { (self.x.x.default_screen)(self.raw) }
    }

    /// An override-redirect window on the root at (x, y), `width` by `height`, kept out of the window manager's
    /// way, of the screen's 32-bit TrueColor visual where `argb` and the screen has one (its own colormap), else of
    /// the root's; transparent black until filled. None if it can't be made.
    pub fn create_visual_window(&self, x: i32, y: i32, width: u32, height: u32, argb: bool) -> Option<VisualWindow> {
        // SAFETY: an all-zero XVisualInfo is a valid one.
        let mut info: XVisualInfo = unsafe { std::mem::zeroed() };
        // SAFETY: an all-zero XSetWindowAttributes is a valid one (only the fields the mask names are read).
        let mut attributes: XSetWindowAttributes = unsafe { std::mem::zeroed() };
        attributes.override_redirect = 1;
        attributes.save_under = 1;
        let mut mask = CW_OVERRIDE_REDIRECT | CW_SAVE_UNDER | CW_BORDER_PIXEL | CW_BACK_PIXEL;
        // SAFETY: the display is open; `info` a valid out struct.
        let found =
            argb && unsafe { (self.x.x.match_visual_info)(self.raw, self.screen(), 32, TRUE_COLOR, &mut info) } != 0;
        let (visual, depth, colormap) = if found {
            // SAFETY: the display is open; the visual is the screen's (just matched).
            let colormap = unsafe { (self.x.x.create_colormap)(self.raw, self.root, info.visual, ALLOC_NONE) };
            attributes.colormap = colormap;
            mask |= CW_COLORMAP;
            (info.visual, 32, colormap)
        } else {
            (std::ptr::null_mut(), COPY_FROM_PARENT, NONE)
        };
        // SAFETY: the display is open; `attributes` valid for the fields the mask names; the visual the screen's
        // (or null: CopyFromParent, with the parent's depth).
        let window = unsafe {
            (self.x.x.create_window)(
                self.raw,
                self.root,
                x,
                y,
                width.max(1),
                height.max(1),
                0,
                depth,
                INPUT_OUTPUT,
                visual,
                mask,
                &mut attributes,
            )
        };
        if window == NONE {
            if colormap != NONE {
                // SAFETY: made above, not used.
                unsafe { (self.x.x.free_colormap)(self.raw, colormap) };
            }
            return None;
        }
        let depth = if found { 32 } else { self.root_depth() };
        Some(VisualWindow { window, depth, colormap })
    }

    /// The root window's depth.
    fn root_depth(&self) -> u32 {
        // SAFETY: an all-zero XWindowAttributes is a valid out struct.
        let mut a: XWindowAttributes = unsafe { std::mem::zeroed() };
        // SAFETY: the display is open; `a` a valid out struct.
        if unsafe { (self.x.x.get_window_attributes)(self.raw, self.root, &mut a) } == 0 {
            return 24;
        }
        a.depth.max(1) as u32
    }

    /// The window made by [`create_visual_window`](Display::create_visual_window) destroyed, its colormap freed.
    pub fn destroy_visual_window(&self, w: VisualWindow) {
        // SAFETY: the display is open; both made on it, not used after.
        unsafe {
            (self.x.x.destroy_window)(self.raw, w.window);
            if w.colormap != NONE {
                (self.x.x.free_colormap)(self.raw, w.colormap);
            }
        }
    }

    /// `w` shows `pixels` (ARGB words, `width` × `height`, row by row: premultiplied for a 32-bit window, else their
    /// colors alone) as its background, which the server draws again whenever it is exposed.
    pub fn set_window_pixels(&self, w: &VisualWindow, width: u32, height: u32, pixels: &[u32]) {
        let n = width as usize * height as usize;
        if width == 0 || height == 0 || pixels.len() < n {
            return;
        }
        let mut data: Vec<u32> = pixels[..n].to_vec();
        let mut image = XImage {
            width: width as c_int,
            height: height as c_int,
            xoffset: 0,
            format: Z_PIXMAP,
            data: data.as_mut_ptr().cast(),
            byte_order: if cfg!(target_endian = "little") { LSB_FIRST } else { 1 },
            bitmap_unit: 32,
            bitmap_bit_order: LSB_FIRST,
            bitmap_pad: 32,
            depth: w.depth as c_int,
            bytes_per_line: width as c_int * 4,
            bits_per_pixel: 32,
            red_mask: 0xff0000,
            green_mask: 0xff00,
            blue_mask: 0xff,
            obdata: std::ptr::null_mut(),
            f: [std::ptr::null_mut(); 6],
        };
        // SAFETY: `image` describes `data` (alive and unmoved for the calls); XInitImage only fills its functions,
        // XPutImage reads `width` × `height` words of it; the pixmap and GC are made on the display and freed here
        // (the window keeps its background: the server holds it); Xlib never frees `data` (the image isn't
        // destroyed through it).
        unsafe {
            if (self.x.x.init_image)(&mut image) == 0 {
                return;
            }
            let pixmap = (self.x.x.create_pixmap)(self.raw, w.window, width, height, w.depth);
            if pixmap == NONE {
                return;
            }
            let gc = (self.x.x.create_gc)(self.raw, pixmap, 0, std::ptr::null_mut());
            if !gc.is_null() {
                (self.x.x.put_image)(self.raw, pixmap, gc, &mut image, 0, 0, 0, 0, width, height);
                (self.x.x.free_gc)(self.raw, gc);
            }
            (self.x.x.set_window_background_pixmap)(self.raw, w.window, pixmap);
            (self.x.x.free_pixmap)(self.raw, pixmap);
            (self.x.x.clear_window)(self.raw, w.window);
        }
        drop(data);
    }

    /// Whether libXext's shapes can be set (the library and the server's extension).
    pub fn has_shapes(&self) -> bool {
        let Some(shape) = &self.x.shape else { return false };
        let (mut event, mut error) = (0, 0);
        // SAFETY: the display is open; the out pointers are valid locals.
        unsafe { (shape.query_extension)(self.raw, &mut event, &mut error) != 0 }
    }

    /// `window` takes no input (its input shape empty: the pointer's events, and what is under the pointer, go
    /// through it). Whether it could be set.
    pub fn set_no_input(&self, window: Window) -> bool {
        let Some(shape) = self.x.shape.as_ref().filter(|_| self.has_shapes()) else { return false };
        // SAFETY: the display is open; no rectangles (an empty region).
        unsafe {
            (shape.combine_rectangles)(
                self.raw,
                window,
                SHAPE_INPUT,
                0,
                0,
                std::ptr::null_mut(),
                0,
                SHAPE_SET,
                UNSORTED,
            )
        };
        true
    }

    /// `window`'s outline: the pixels of `mask` set (a bitmap, `width` × `height`, rows of whole bytes, the least
    /// significant bit first). Whether it could be set.
    pub fn set_outline(&self, window: Window, width: u32, height: u32, mask: &[u8]) -> bool {
        let Some(shape) = self.x.shape.as_ref().filter(|_| self.has_shapes()) else { return false };
        if mask.len() < width.div_ceil(8) as usize * height as usize {
            return false;
        }
        // SAFETY: the display is open; `mask` holds the bitmap's rows (XCreateBitmapFromData's layout), copied into
        // the pixmap, which is freed once the shape is set.
        unsafe {
            let bitmap = (self.x.x.create_bitmap_from_data)(self.raw, window, mask.as_ptr().cast(), width, height);
            if bitmap == NONE {
                return false;
            }
            (shape.combine_mask)(self.raw, window, SHAPE_BOUNDING, 0, 0, bitmap, SHAPE_SET);
            (self.x.x.free_pixmap)(self.raw, bitmap);
        }
        true
    }

    pub fn move_window(&self, window: Window, x: i32, y: i32) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.move_window)(self.raw, window, x, y) };
    }

    /// `window` shown, above its siblings.
    pub fn map_raised(&self, window: Window) {
        // SAFETY: the display is open.
        unsafe { (self.x.x.map_raised)(self.raw, window) };
    }

    /// Every request sent and handled by the server.
    pub fn sync(&self) {
        // SAFETY: the display is open; the events stay queued.
        unsafe { (self.x.x.sync)(self.raw, 0) };
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        // SAFETY: the display, closed once; nothing made on it is used after (its holders are gone: they hold it).
        unsafe { (self.x.x.close_display)(self.raw) };
    }
}

/// An XInput 2 event's parts.
///
/// # Safety
/// `data` must be libXi's data for an event of type `evtype`.
unsafe fn decode_xi(evtype: c_int, data: *mut c_void) -> XiEvent {
    match evtype {
        XI_HIERARCHY_CHANGED => XiEvent::Hierarchy,
        XI_MOTION | XI_BUTTON_PRESS | XI_BUTTON_RELEASE | XI_TOUCH_BEGIN | XI_TOUCH_UPDATE | XI_TOUCH_END => {
            // SAFETY: these types' data is an XIDeviceEvent (the caller's word).
            let d = unsafe { &*data.cast::<XiDeviceEvent>() };
            let mut valuators = Vec::new();
            if !d.valuators.mask.is_null() && !d.valuators.values.is_null() {
                let mut value = d.valuators.values;
                for i in 0..d.valuators.mask_len.max(0) as usize * 8 {
                    // SAFETY: the mask is `mask_len` bytes.
                    let set = unsafe { *d.valuators.mask.add(i >> 3) } & (1 << (i & 7)) != 0;
                    if set {
                        // SAFETY: a value for each bit set, in order.
                        valuators.push((i, unsafe { *value }));
                        // SAFETY: as above (one past the last at most).
                        value = unsafe { value.add(1) };
                    }
                }
            }
            XiEvent::Device(XiDevice {
                evtype,
                time: d.time,
                device: d.deviceid,
                source: d.sourceid,
                detail: d.detail,
                x: d.event_x,
                y: d.event_y,
                root_x: d.root_x,
                root_y: d.root_y,
                flags: d.flags,
                mods: d.mods.effective as u32,
                valuators,
            })
        }
        XI_ENTER | XI_LEAVE => {
            // SAFETY: these types' data is an XIEnterEvent.
            let c = unsafe { &*data.cast::<XiEnterEvent>() };
            XiEvent::Crossing {
                enter: evtype == XI_ENTER,
                source: c.sourceid,
                x: c.event_x,
                y: c.event_y,
                mode: c.mode,
                mods: c.mods.effective as u32,
            }
        }
        XI_GESTURE_PINCH_BEGIN | XI_GESTURE_PINCH_UPDATE | XI_GESTURE_PINCH_END => {
            // SAFETY: these types' data is an XIGesturePinchEvent.
            let g = unsafe { &*data.cast::<XiGesturePinchEvent>() };
            XiEvent::Pinch {
                evtype,
                x: g.event_x,
                y: g.event_y,
                scale: g.scale,
                delta_angle: g.delta_angle,
                mods: g.mods.effective as u32,
            }
        }
        _ => XiEvent::Other,
    }
}

// ---- The input method -----------------------------------------------------------------------------------------------

/// What XIM's callbacks tell (called inside Xlib's event processing: they only set these, the platform acts after).
#[derive(Default)]
struct ImSignals {
    /// The input method went (its server quit): its input context with it.
    destroyed: Cell<bool>,
    /// An input method came (after none could be opened).
    instantiated: Cell<bool>,
    /// What an on-the-spot input method is composing (its preedit callbacks').
    preedit: RefCell<Preedit>,
}

/// The signals a callback's client data is.
///
/// # Safety
/// `client` must be the boxed signals the callback was registered with, alive while it can be called.
unsafe fn signals<'a>(client: XPointer) -> &'a ImSignals {
    // SAFETY: as the caller says.
    unsafe { &*client.cast::<ImSignals>() }
}

/// The preedit, unless it is being read (never: it is only read between Xlib's calls; a callback can't unwind).
fn with_preedit<T: Default>(signals: &ImSignals, f: impl FnOnce(&mut Preedit) -> T) -> T {
    signals.preedit.try_borrow_mut().map(|mut p| f(&mut p)).unwrap_or_default()
}

unsafe extern "C" fn preedit_start(_: Xic, client: XPointer, _: XPointer) -> c_int {
    // SAFETY: the context's preedit callbacks were given the input method's boxed signals, alive while it is.
    with_preedit(unsafe { signals(client) }, Preedit::clear);
    -1 // (no limit to what it composes)
}

unsafe extern "C" fn preedit_done(_: Xic, client: XPointer, _: XPointer) -> c_int {
    // SAFETY: as preedit_start's.
    with_preedit(unsafe { signals(client) }, Preedit::clear);
    0
}

unsafe extern "C" fn preedit_draw(_: Xic, client: XPointer, call: XPointer) -> c_int {
    // SAFETY: as preedit_start's.
    let signals = unsafe { signals(client) };
    // SAFETY: Xlib calls a draw callback with its draw (or none).
    let Some(draw) = (unsafe { call.cast::<XimPreeditDraw>().as_ref() }) else { return 0 };
    // SAFETY: the draw's text, if any, is Xlib's for the call: `length` feedbacks, a C string or `length` wchar_t's.
    let text = unsafe { draw.text.as_ref() }.map(|t| {
        let length = usize::from(t.length);
        let marked: Vec<bool> = if t.feedback.is_null() {
            Vec::new()
        } else {
            // SAFETY: as above.
            let feedback = unsafe { std::slice::from_raw_parts(t.feedback, length) };
            feedback.iter().map(|&f| f & MARKING_FEEDBACK != 0).collect()
        };
        let chars = if t.string.is_null() {
            None
        } else if t.encoding_is_wchar != 0 {
            // SAFETY: as above (wchar_t is 32 bits on Linux).
            let wide = unsafe { std::slice::from_raw_parts(t.string.cast::<i32>(), length) };
            Some(text_chars(Some(wide), None, length))
        } else {
            // SAFETY: as above.
            let bytes = unsafe { CStr::from_ptr(t.string.cast::<c_char>()) }.to_bytes();
            Some(text_chars(None, Some(bytes), length))
        };
        (chars, marked)
    });
    with_preedit(signals, |p| match &text {
        // (No text: the stretch deleted.)
        None => p.draw(draw.caret, draw.chg_first, draw.chg_length, Some(&[]), &[]),
        Some((chars, marked)) => p.draw(draw.caret, draw.chg_first, draw.chg_length, chars.as_deref(), marked),
    });
    0
}

unsafe extern "C" fn preedit_caret(_: Xic, client: XPointer, call: XPointer) -> c_int {
    // SAFETY: as preedit_start's.
    let signals = unsafe { signals(client) };
    // SAFETY: Xlib calls a caret callback with its caret move, the position written back.
    let Some(caret) = (unsafe { call.cast::<XimPreeditCaret>().as_mut() }) else { return 0 };
    if let Some(direction) = CaretDirection::of(caret.direction) {
        let position = caret.position;
        caret.position = with_preedit(signals, |p| p.move_caret(position, direction)) as c_int;
    }
    0
}

/// How the window's input context shows what is being composed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum PreeditStyle {
    /// Where the input method shows it (its own window).
    #[default]
    Nothing,
    /// Over the spot it is told (the input method's window there).
    Position,
    /// In the field itself (on the spot): the preedit callbacks.
    Callbacks,
}

impl PreeditStyle {
    /// Its `XIMPreedit…` bit.
    fn value(self) -> c_ulong {
        match self {
            PreeditStyle::Nothing => XIM_PREEDIT_NOTHING,
            PreeditStyle::Position => XIM_PREEDIT_POSITION,
            PreeditStyle::Callbacks => XIM_PREEDIT_CALLBACKS,
        }
    }
}

unsafe extern "C" fn im_destroyed(_: Xim, client: XPointer, _: XPointer) {
    // SAFETY: `client` is the boxed signals the input method was given, alive while it is.
    unsafe { (*client.cast::<ImSignals>()).destroyed.set(true) };
}

unsafe extern "C" fn im_instantiated(_: *mut RawDisplay, client: XPointer, _: XPointer) {
    // SAFETY: `client` is the boxed signals the callback was registered with, alive until it is unregistered.
    unsafe { (*client.cast::<ImSignals>()).instantiated.set(true) };
}

/// The input method (XIM) and the window's input context (C++'s `openInputMethod`, `makeInputContext`,
/// `tellInputMethod`): opened in the desktop's locale (else X's own, `@im=none`, for compose keys), or once one comes;
/// opened again if it goes. What is being composed is shown in the field itself where the method can (on the spot:
/// `XIMPreeditCallbacks`, its callbacks changing a [`Preedit`] the platform reads after Xlib's calls:
/// [`take_preedit`](Self::take_preedit)), else by the input method over the field where it can be told where (over
/// the spot: `XIMPreeditPosition`), else where it shows it (`XIMPreeditNothing`). The spot follows the field's caret
/// (for its candidates too, where the method takes it in the on-the-spot style).
pub struct InputMethod {
    display: Rc<Display>,
    im: Xim,
    ic: Xic,
    style: PreeditStyle,
    /// The instantiate callback is registered.
    waiting: bool,
    signals: Box<ImSignals>,
}

impl InputMethod {
    /// None open yet.
    pub fn new(display: Rc<Display>) -> InputMethod {
        InputMethod {
            display,
            im: std::ptr::null_mut(),
            ic: std::ptr::null_mut(),
            style: PreeditStyle::Nothing,
            waiting: false,
            signals: Box::default(),
        }
    }

    /// What the on-the-spot input method composes, if it changed since asked: its text and its caret (or the part
    /// of it being converted) in its bytes.
    pub fn take_preedit(&self) -> Option<(String, (usize, usize))> {
        self.signals.preedit.borrow_mut().take_change()
    }

    /// The context's preedit style (`XIMPreedit…`), and the input method's styles (the tests ask).
    #[cfg(test)]
    pub(super) fn preedit_styles(&self) -> (c_ulong, Vec<c_ulong>) {
        (self.style.value(), self.styles())
    }

    /// What the on-the-spot input method is composing now.
    pub fn preedit_text(&self) -> String {
        self.signals.preedit.borrow().text()
    }

    /// The context's composing given up (`Xutf8ResetIC`: the field it was for is gone, or the app changed its
    /// text): what it was composing is dropped (the field has it as it was, or as the app made it) and not told.
    pub fn reset(&mut self) {
        if self.has_context() {
            // SAFETY: the context is alive; what it returns is Xlib's, freed once.
            unsafe {
                let left = (self.display.x.x.utf8_reset_ic)(self.ic);
                if !left.is_null() {
                    (self.display.x.x.free)(left.cast());
                }
            }
        }
        let mut preedit = self.signals.preedit.borrow_mut();
        preedit.clear();
        preedit.take_change();
    }

    fn signals_ptr(&self) -> XPointer {
        (&*self.signals as *const ImSignals).cast_mut().cast()
    }

    /// Whether an input method is open.
    pub fn is_open(&self) -> bool {
        !self.im.is_null() && !self.signals.destroyed.get()
    }

    /// Whether the window has an input context (typing goes through the input method then).
    pub fn has_context(&self) -> bool {
        self.is_open() && !self.ic.is_null()
    }

    /// Opens the input method as the desktop's locale has it, else X's own; else waits for one to come. True if
    /// one is open.
    pub fn open(&mut self) -> bool {
        if self.is_open() {
            return true;
        }
        let x = &self.display.x.x;
        let raw = self.display.raw;
        // The locale and the modifiers set only while it opens, as SDL does (and C++).
        // SAFETY: setlocale with null only reads the current locale; the result is copied at once.
        let previous = unsafe {
            let p = libc::setlocale(libc::LC_ALL, std::ptr::null());
            if p.is_null() { c"C".to_owned() } else { CStr::from_ptr(p).to_owned() }
        };
        // SAFETY: valid C strings; the display is open; no resource database, names or classes.
        let im = unsafe {
            libc::setlocale(libc::LC_ALL, c"".as_ptr());
            (x.set_locale_modifiers)(c"".as_ptr());
            let mut im = (x.open_im)(raw, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            if im.is_null() {
                (x.set_locale_modifiers)(c"@im=none".as_ptr());
                im = (x.open_im)(raw, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            }
            libc::setlocale(libc::LC_ALL, previous.as_ptr());
            im
        };
        self.signals.destroyed.set(false);
        if im.is_null() {
            self.wait_for_one();
            return false;
        }
        self.im = im;
        let mut destroyed = XimCallback { client_data: self.signals_ptr(), callback: Some(im_destroyed) };
        // SAFETY: the input method is open; the callback's data is the boxed signals, alive as long as `self` (the
        // method is closed before they go); a null ends the list.
        unsafe {
            (x.set_im_values)(
                im,
                XN_DESTROY_CALLBACK.as_ptr(),
                &mut destroyed as *mut XimCallback,
                std::ptr::null_mut::<c_void>(),
            )
        };
        true
    }

    /// Registers for an input method coming (its server starting).
    fn wait_for_one(&mut self) {
        if self.waiting {
            return;
        }
        // SAFETY: the display is open; the client data is the boxed signals, unregistered before they go.
        self.waiting = unsafe {
            (self.display.x.x.register_im_instantiate_callback)(
                self.display.raw,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                Some(im_instantiated),
                self.signals_ptr(),
            ) != 0
        };
    }

    fn stop_waiting(&mut self) {
        if !std::mem::take(&mut self.waiting) {
            return;
        }
        // SAFETY: as registered.
        unsafe {
            (self.display.x.x.unregister_im_instantiate_callback)(
                self.display.raw,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                Some(im_instantiated),
                self.signals_ptr(),
            )
        };
    }

    /// What the callbacks told since, acted on (after the events are read): a method gone is forgotten (it took its
    /// context with it) and waited for again; one come is opened, with a context for `window` if there is one. True
    /// if the context changed (to be told the field again).
    pub fn update(&mut self, window: Option<Window>, spot: XPoint) -> bool {
        let mut changed = false;
        if self.signals.destroyed.get() {
            self.im = std::ptr::null_mut();
            self.ic = std::ptr::null_mut();
            self.signals.destroyed.set(false);
            self.signals.preedit.borrow_mut().clear();
            self.wait_for_one();
            changed = true;
        }
        if self.signals.instantiated.replace(false) {
            self.stop_waiting();
            if self.open()
                && let Some(w) = window
            {
                self.create_context(w, spot);
            }
            changed = true;
        }
        changed
    }

    /// The styles the input method has (`XNQueryInputStyle`).
    fn styles(&self) -> Vec<c_ulong> {
        let x = &self.display.x.x;
        let mut styles: *mut XimStyles = std::ptr::null_mut();
        // SAFETY: the input method is open; `styles` is set to Xlib's allocation (freed below); a null ends the list.
        let failed = unsafe {
            (x.get_im_values)(
                self.im,
                XN_QUERY_INPUT_STYLE.as_ptr(),
                &mut styles as *mut *mut XimStyles,
                std::ptr::null_mut::<c_void>(),
            )
        };
        if !failed.is_null() || styles.is_null() {
            return Vec::new();
        }
        // SAFETY: Xlib's styles: `count_styles` of them.
        let all = unsafe {
            let s = &*styles;
            std::slice::from_raw_parts(s.supported_styles, usize::from(s.count_styles)).to_vec()
        };
        // SAFETY: Xlib's allocation, freed once.
        unsafe { (x.free)(styles.cast()) };
        all
    }

    /// The window's input context (C++'s `makeInputContext`): on the spot if the method can, else over the spot,
    /// else its own way (each tried in turn should the method refuse one).
    pub fn create_context(&mut self, window: Window, spot: XPoint) {
        if !self.is_open() || !self.ic.is_null() {
            return;
        }
        let styles = self.styles();
        for style in [PreeditStyle::Callbacks, PreeditStyle::Position, PreeditStyle::Nothing] {
            let value = style.value() | XIM_STATUS_NOTHING;
            if style != PreeditStyle::Nothing && !styles.contains(&value) {
                continue;
            }
            self.ic = self.make_context(window, style, spot);
            if !self.ic.is_null() {
                self.style = style;
                return;
            }
        }
    }

    /// An input context for `window` of `style` (null if the method refuses it).
    fn make_context(&self, window: Window, style: PreeditStyle, spot: XPoint) -> Xic {
        let x = &self.display.x.x;
        let input_style = style.value() | XIM_STATUS_NOTHING;
        let client = self.signals_ptr();
        let callback = |f: unsafe extern "C" fn(Xic, XPointer, XPointer) -> c_int| XicCallback {
            client_data: client,
            callback: Some(f),
        };
        let (mut start, mut done) = (callback(preedit_start), callback(preedit_done));
        let (mut draw, mut caret) = (callback(preedit_draw), callback(preedit_caret));
        let mut spot = spot;
        // SAFETY: the input method is open; the values are of the types their names take (an XIMStyle and windows as
        // unsigned longs, a nested list of a point or of XICCallbacks, which Xlib copies: their client data the boxed
        // signals, alive while the method is, its contexts destroyed first), each list ended by a null; the nested
        // list is freed after.
        unsafe {
            let list = match style {
                PreeditStyle::Callbacks => (x.va_create_nested_list)(
                    0,
                    XN_PREEDIT_START_CALLBACK.as_ptr(),
                    &mut start as *mut XicCallback,
                    XN_PREEDIT_DONE_CALLBACK.as_ptr(),
                    &mut done as *mut XicCallback,
                    XN_PREEDIT_DRAW_CALLBACK.as_ptr(),
                    &mut draw as *mut XicCallback,
                    XN_PREEDIT_CARET_CALLBACK.as_ptr(),
                    &mut caret as *mut XicCallback,
                    std::ptr::null_mut::<c_void>(),
                ),
                PreeditStyle::Position => (x.va_create_nested_list)(
                    0,
                    XN_SPOT_LOCATION.as_ptr(),
                    &mut spot as *mut XPoint,
                    std::ptr::null_mut::<c_void>(),
                ),
                PreeditStyle::Nothing => std::ptr::null_mut(),
            };
            if list.is_null() {
                (x.create_ic)(
                    self.im,
                    XN_INPUT_STYLE.as_ptr(),
                    input_style,
                    XN_CLIENT_WINDOW.as_ptr(),
                    window,
                    XN_FOCUS_WINDOW.as_ptr(),
                    window,
                    std::ptr::null_mut::<c_void>(),
                )
            } else {
                let ic = (x.create_ic)(
                    self.im,
                    XN_INPUT_STYLE.as_ptr(),
                    input_style,
                    XN_CLIENT_WINDOW.as_ptr(),
                    window,
                    XN_FOCUS_WINDOW.as_ptr(),
                    window,
                    XN_PREEDIT_ATTRIBUTES.as_ptr(),
                    list,
                    std::ptr::null_mut::<c_void>(),
                );
                (x.free)(list);
                ic
            }
        }
    }

    /// The window's context goes (the window is closing), with what it was composing.
    pub fn destroy_context(&mut self) {
        if !self.ic.is_null() && self.is_open() {
            // SAFETY: a context of the open method, destroyed once.
            unsafe { (self.display.x.x.destroy_ic)(self.ic) };
        }
        self.ic = std::ptr::null_mut();
        self.signals.preedit.borrow_mut().clear();
    }

    /// The context focused (a field takes typing in the focused window: told where its caret is — on the spot as
    /// well, for the method's candidates, though Xlib may refuse the spot in that style: then the method places
    /// them itself) or not (C++'s `tellInputMethod`).
    pub fn focus(&mut self, on: bool, spot: XPoint) {
        if !self.has_context() {
            return;
        }
        let x = &self.display.x.x;
        // SAFETY: the context is alive (its method open); the nested list is made, passed and freed here; each
        // list is ended by a null.
        unsafe {
            if on {
                if self.style != PreeditStyle::Nothing {
                    let mut spot = spot;
                    let list = (x.va_create_nested_list)(
                        0,
                        XN_SPOT_LOCATION.as_ptr(),
                        &mut spot as *mut XPoint,
                        std::ptr::null_mut::<c_void>(),
                    );
                    (x.set_ic_values)(self.ic, XN_PREEDIT_ATTRIBUTES.as_ptr(), list, std::ptr::null_mut::<c_void>());
                    if !list.is_null() {
                        (x.free)(list);
                    }
                }
                (x.set_ic_focus)(self.ic);
            } else {
                (x.unset_ic_focus)(self.ic);
            }
        }
    }

    /// What `key` types through the input context (an input method's commit included), as UTF-8; none without a
    /// context.
    pub fn lookup(&self, key: &KeyEvent) -> Option<String> {
        if !self.has_context() {
            return None;
        }
        let lookup = self.display.x.x.utf8_lookup_string;
        let mut k = key.0;
        let mut buffer = vec![0u8; 64];
        let (mut sym, mut status): (KeySym, Status) = (0, 0);
        // SAFETY: the context is alive; the buffer is `len` writable bytes; the out pointers valid locals.
        let mut n = unsafe {
            lookup(self.ic, &mut k, buffer.as_mut_ptr().cast(), buffer.len() as c_int, &mut sym, &mut status)
        };
        if status == X_BUFFER_OVERFLOW {
            buffer.resize(n.max(0) as usize, 0);
            // SAFETY: as above, the buffer as big as the text.
            n = unsafe {
                lookup(self.ic, &mut k, buffer.as_mut_ptr().cast(), buffer.len() as c_int, &mut sym, &mut status)
            };
        }
        let typed = status == X_LOOKUP_CHARS || status == X_LOOKUP_BOTH;
        buffer.truncate(if typed { n.clamp(0, buffer.len() as c_int) as usize } else { 0 });
        Some(String::from_utf8_lossy(&buffer).into_owned())
    }
}

impl Drop for InputMethod {
    fn drop(&mut self) {
        self.destroy_context();
        self.stop_waiting();
        if self.is_open() {
            // SAFETY: the method, closed once (its destroy callback's signals still alive: dropped after).
            unsafe { (self.display.x.x.close_im)(self.im) };
        }
    }
}

// ---- Vulkan ---------------------------------------------------------------------------------------------------------

/// What presenting to `window` with Vulkan needs (`VK_KHR_xlib_surface`, C++'s `createVulkanSurface`): the surface
/// made on the display (kept open while the device lives: the surface's maker holds it).
pub fn vulkan_surface(display: &Rc<Display>, window: Window) -> VulkanSurface {
    let display = display.clone();
    VulkanSurface {
        instance_extensions: vec![khr::surface::NAME, khr::xlib_surface::NAME],
        create_surface: Box::new(move |entry, instance| {
            let loader = khr::xlib_surface::Instance::new(entry, instance);
            let info = vk::XlibSurfaceCreateInfoKHR::default().dpy(display.raw.cast()).window(window);
            // SAFETY: the instance was made with VK_KHR_xlib_surface (these instance_extensions); the display is open
            // (held by this closure) and the window is its.
            unsafe { loader.create_xlib_surface(&info, None) }.ok()
        }),
        native_window: None,
    }
}

// ---- Layouts --------------------------------------------------------------------------------------------------------

/// The structs' sizes and fields' offsets as the system's headers have them on 64-bit Linux (a C program printing
/// `sizeof` and `offsetof` against /usr/include/X11, Xlib 1.8, XInput 2.4, XRandR 1.6).
#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn events_are_laid_out_as_xlib_has_them() {
        assert_eq!(size_of::<RawEvent>(), 192);
        assert_eq!(size_of::<XAnyEvent>(), 40);
        assert_eq!(offset_of!(XAnyEvent, send_event), 16);
        assert_eq!(offset_of!(XAnyEvent, window), 32);
        assert_eq!(size_of::<XKeyEvent>(), 96);
        assert_eq!(offset_of!(XKeyEvent, time), 56);
        assert_eq!(offset_of!(XKeyEvent, x_root), 72);
        assert_eq!(offset_of!(XKeyEvent, state), 80);
        assert_eq!(offset_of!(XKeyEvent, keycode), 84);
        assert_eq!(offset_of!(XKeyEvent, same_screen), 88);
        assert_eq!(size_of::<XFocusChangeEvent>(), 48);
        assert_eq!(offset_of!(XFocusChangeEvent, mode), 40);
        assert_eq!(offset_of!(XFocusChangeEvent, detail), 44);
        assert_eq!(size_of::<XExposeEvent>(), 64);
        assert_eq!(offset_of!(XExposeEvent, count), 56);
        assert_eq!(size_of::<XConfigureEvent>(), 88);
        assert_eq!(offset_of!(XConfigureEvent, window), 40);
        assert_eq!(offset_of!(XConfigureEvent, x), 48);
        assert_eq!(offset_of!(XConfigureEvent, width), 56);
        assert_eq!(offset_of!(XConfigureEvent, above), 72);
        assert_eq!(offset_of!(XConfigureEvent, override_redirect), 80);
        assert_eq!(size_of::<XPropertyEvent>(), 64);
        assert_eq!(offset_of!(XPropertyEvent, atom), 40);
        assert_eq!(offset_of!(XPropertyEvent, state), 56);
        assert_eq!(size_of::<XSelectionClearEvent>(), 56);
        assert_eq!(offset_of!(XSelectionClearEvent, selection), 40);
        assert_eq!(size_of::<XSelectionRequestEvent>(), 80);
        assert_eq!(offset_of!(XSelectionRequestEvent, requestor), 40);
        assert_eq!(offset_of!(XSelectionRequestEvent, property), 64);
        assert_eq!(offset_of!(XSelectionRequestEvent, time), 72);
        assert_eq!(size_of::<XSelectionEvent>(), 72);
        assert_eq!(offset_of!(XSelectionEvent, requestor), 32);
        assert_eq!(offset_of!(XSelectionEvent, property), 56);
        assert_eq!(offset_of!(XSelectionEvent, time), 64);
        assert_eq!(size_of::<XClientMessageEvent>(), 96);
        assert_eq!(offset_of!(XClientMessageEvent, message_type), 40);
        assert_eq!(offset_of!(XClientMessageEvent, format), 48);
        assert_eq!(offset_of!(XClientMessageEvent, data), 56);
        assert_eq!(size_of::<XMappingEvent>(), 56);
        assert_eq!(offset_of!(XMappingEvent, request), 40);
        assert_eq!(offset_of!(XMappingEvent, count), 48);
        assert_eq!(size_of::<XGenericEventCookie>(), 56);
        assert_eq!(offset_of!(XGenericEventCookie, extension), 32);
        assert_eq!(offset_of!(XGenericEventCookie, evtype), 36);
        assert_eq!(offset_of!(XGenericEventCookie, cookie), 40);
        assert_eq!(offset_of!(XGenericEventCookie, data), 48);
    }

    #[test]
    fn xlib_structs_are_laid_out_as_xlib_has_them() {
        assert_eq!(size_of::<XSetWindowAttributes>(), 112);
        assert_eq!(offset_of!(XSetWindowAttributes, background_pixel), 8);
        assert_eq!(offset_of!(XSetWindowAttributes, bit_gravity), 32);
        assert_eq!(offset_of!(XSetWindowAttributes, backing_store), 40);
        assert_eq!(offset_of!(XSetWindowAttributes, backing_planes), 48);
        assert_eq!(offset_of!(XSetWindowAttributes, save_under), 64);
        assert_eq!(offset_of!(XSetWindowAttributes, event_mask), 72);
        assert_eq!(offset_of!(XSetWindowAttributes, override_redirect), 88);
        assert_eq!(offset_of!(XSetWindowAttributes, cursor), 104);
        assert_eq!(size_of::<XClassHint>(), 16);
        assert_eq!(size_of::<XWindowAttributes>(), 136);
        assert_eq!(offset_of!(XWindowAttributes, map_state), 92);
        assert_eq!(offset_of!(XWindowAttributes, override_redirect), 120);
        assert_eq!(offset_of!(XWindowAttributes, screen), 128);
        assert_eq!(size_of::<XVisualInfo>(), 64);
        assert_eq!((offset_of!(XVisualInfo, depth), offset_of!(XVisualInfo, class)), (20, 24));
        assert_eq!(size_of::<XImage>(), 136);
        assert_eq!(offset_of!(XImage, data), 16);
        assert_eq!(offset_of!(XImage, bytes_per_line), 44);
        assert_eq!(offset_of!(XImage, bits_per_pixel), 48);
        assert_eq!(offset_of!(XImage, red_mask), 56);
        assert_eq!(offset_of!(XImage, obdata), 80);
        assert_eq!(offset_of!(XImage, f), 88);
        assert_eq!(size_of::<XRectangle>(), 8);
        assert_eq!(size_of::<XimStyles>(), 16);
        assert_eq!(offset_of!(XimStyles, supported_styles), 8);
        assert_eq!(size_of::<XimCallback>(), 16);
        assert_eq!(offset_of!(XimCallback, callback), 8);
        assert_eq!((size_of::<XicCallback>(), offset_of!(XicCallback, callback)), (16, 8));
        assert_eq!(size_of::<XimText>(), 32);
        assert_eq!((offset_of!(XimText, feedback), offset_of!(XimText, encoding_is_wchar)), (8, 16));
        assert_eq!(offset_of!(XimText, string), 24);
        assert_eq!((size_of::<XimPreeditDraw>(), offset_of!(XimPreeditDraw, text)), (24, 16));
        assert_eq!((size_of::<XimPreeditCaret>(), offset_of!(XimPreeditCaret, style)), (12, 8));
        assert_eq!((size_of::<XPoint>(), std::mem::align_of::<XPoint>()), (4, 2));
    }

    #[test]
    fn xinput_structs_are_laid_out_as_libxi_has_them() {
        assert_eq!(size_of::<XiEventMask>(), 16);
        assert_eq!(offset_of!(XiEventMask, mask), 8);
        assert_eq!((size_of::<XiModifierState>(), std::mem::align_of::<XiModifierState>()), (16, 4));
        assert_eq!(size_of::<XiButtonState>(), 16);
        assert_eq!(size_of::<XiValuatorState>(), 24);
        assert_eq!(offset_of!(XiValuatorState, values), 16);
        assert_eq!(size_of::<XiDeviceEvent>(), 200);
        assert_eq!(offset_of!(XiDeviceEvent, extension), 32);
        assert_eq!(offset_of!(XiDeviceEvent, time), 40);
        assert_eq!(offset_of!(XiDeviceEvent, sourceid), 52);
        assert_eq!(offset_of!(XiDeviceEvent, detail), 56);
        assert_eq!(offset_of!(XiDeviceEvent, child), 80);
        assert_eq!(offset_of!(XiDeviceEvent, event_x), 104);
        assert_eq!(offset_of!(XiDeviceEvent, flags), 120);
        assert_eq!(offset_of!(XiDeviceEvent, buttons), 128);
        assert_eq!(offset_of!(XiDeviceEvent, valuators), 144);
        assert_eq!(offset_of!(XiDeviceEvent, mods), 168);
        assert_eq!(offset_of!(XiDeviceEvent, group), 184);
        assert_eq!(size_of::<XiEnterEvent>(), 184);
        assert_eq!(offset_of!(XiEnterEvent, event_x), 104);
        assert_eq!(offset_of!(XiEnterEvent, mode), 120);
        assert_eq!(offset_of!(XiEnterEvent, same_screen), 128);
        assert_eq!(offset_of!(XiEnterEvent, buttons), 136);
        assert_eq!(offset_of!(XiEnterEvent, mods), 152);
        assert_eq!(offset_of!(XiEnterEvent, group), 168);
        assert_eq!(size_of::<XiGesturePinchEvent>(), 208);
        assert_eq!(offset_of!(XiGesturePinchEvent, delta_x), 120);
        assert_eq!(offset_of!(XiGesturePinchEvent, scale), 152);
        assert_eq!(offset_of!(XiGesturePinchEvent, delta_angle), 160);
        assert_eq!(offset_of!(XiGesturePinchEvent, flags), 168);
        assert_eq!(offset_of!(XiGesturePinchEvent, mods), 172);
        assert_eq!(offset_of!(XiGesturePinchEvent, group), 188);
        assert_eq!(size_of::<XiDeviceInfo>(), 40);
        assert_eq!(offset_of!(XiDeviceInfo, name), 8);
        assert_eq!(offset_of!(XiDeviceInfo, use_), 16);
        assert_eq!(offset_of!(XiDeviceInfo, num_classes), 28);
        assert_eq!(offset_of!(XiDeviceInfo, classes), 32);
        assert_eq!(size_of::<XiAnyClassInfo>(), 8);
        assert_eq!(size_of::<XiScrollClassInfo>(), 32);
        assert_eq!(offset_of!(XiScrollClassInfo, scroll_type), 12);
        assert_eq!(offset_of!(XiScrollClassInfo, increment), 16);
        assert_eq!(offset_of!(XiScrollClassInfo, flags), 24);
        assert_eq!(size_of::<XiTouchClassInfo>(), 16);
        assert_eq!(offset_of!(XiTouchClassInfo, mode), 8);
    }

    #[test]
    fn xrandr_structs_are_laid_out_as_libxrandr_has_them() {
        assert_eq!(size_of::<XrrModeInfo>(), 80);
        assert_eq!(offset_of!(XrrModeInfo, dot_clock), 16);
        assert_eq!(offset_of!(XrrModeInfo, h_total), 32);
        assert_eq!(offset_of!(XrrModeInfo, v_total), 48);
        assert_eq!(offset_of!(XrrModeInfo, name), 56);
        assert_eq!(offset_of!(XrrModeInfo, mode_flags), 72);
        assert_eq!(size_of::<XrrScreenResources>(), 64);
        assert_eq!(offset_of!(XrrScreenResources, ncrtc), 16);
        assert_eq!(offset_of!(XrrScreenResources, crtcs), 24);
        assert_eq!(offset_of!(XrrScreenResources, nmode), 48);
        assert_eq!(offset_of!(XrrScreenResources, modes), 56);
        assert_eq!(size_of::<XrrCrtcInfo>(), 64);
        assert_eq!(offset_of!(XrrCrtcInfo, x), 8);
        assert_eq!(offset_of!(XrrCrtcInfo, width), 16);
        assert_eq!(offset_of!(XrrCrtcInfo, mode), 24);
        assert_eq!(offset_of!(XrrCrtcInfo, rotation), 32);
        assert_eq!(offset_of!(XrrCrtcInfo, noutput), 36);
        assert_eq!(offset_of!(XrrCrtcInfo, rotations), 48);
        assert_eq!(offset_of!(XrrCrtcInfo, possible), 56);
    }

    #[test]
    fn xi_masks_cover_xinput_2_4() {
        assert_eq!(XI_MASK_LEN, 5);
        assert!((XI_GESTURE_PINCH_END as usize >> 3) < XI_MASK_LEN);
    }

    #[test]
    fn preedit_callbacks_apply_what_xlib_gives_them() {
        let signals = Box::<ImSignals>::default();
        let client: XPointer = (&*signals as *const ImSignals).cast_mut().cast();
        let ic: Xic = std::ptr::null_mut();
        let none: XPointer = std::ptr::null_mut();
        let take = || signals.preedit.borrow_mut().take_change();
        let draw = |caret, first, length, text: *mut XimText| {
            let mut d = XimPreeditDraw { caret, chg_first: first, chg_length: length, text };
            // SAFETY: a draw as Xlib gives one, with the signals the callbacks are registered with.
            unsafe { preedit_draw(ic, client, (&mut d as *mut XimPreeditDraw).cast()) };
        };
        // SAFETY: as Xlib calls it.
        assert_eq!(unsafe { preedit_start(ic, client, none) }, -1, "no limit");
        // Multibyte (the locale's UTF-8), its first two characters reversed (being converted): that part marked.
        let utf8 = c"çay";
        let mut feedback: [c_ulong; 3] = [1, 1, 2];
        let mut text = XimText {
            length: 3,
            feedback: feedback.as_mut_ptr(),
            encoding_is_wchar: 0,
            string: utf8.as_ptr().cast_mut().cast(),
        };
        draw(3, 0, 0, &mut text);
        assert_eq!(take(), Some(("çay".into(), (0, 3))));
        // Wide characters in place of all of it, nothing marked; the caret after the first.
        let mut wide: [i32; 3] = ['ç' as i32, 'ğ' as i32, 'ı' as i32];
        let mut text = XimText {
            length: 3,
            feedback: std::ptr::null_mut(),
            encoding_is_wchar: 1,
            string: wide.as_mut_ptr().cast(),
        };
        draw(1, 0, 3, &mut text);
        assert_eq!(take(), Some(("çğı".into(), (2, 2))));
        // Only the feedback (no string): the last one highlighted.
        let mut feedback: [c_ulong; 1] = [4];
        let mut text =
            XimText { length: 1, feedback: feedback.as_mut_ptr(), encoding_is_wchar: 0, string: none.cast() };
        draw(1, 2, 1, &mut text);
        assert_eq!(take(), Some(("çğı".into(), (4, 6))));
        // No text: deleted.
        draw(0, 0, 1, std::ptr::null_mut());
        assert_eq!(take(), Some(("ğı".into(), (2, 4))));
        // The caret moved to the end: its position told back.
        let mut caret = XimPreeditCaret { position: 0, direction: 9, style: 0 };
        // SAFETY: as Xlib calls it.
        unsafe { preedit_caret(ic, client, (&mut caret as *mut XimPreeditCaret).cast()) };
        assert_eq!(caret.position, 2);
        // Done: nothing composed.
        // SAFETY: as Xlib calls it.
        unsafe { preedit_done(ic, client, none) };
        assert_eq!(take(), Some((String::new(), (0, 0))));
    }
}
