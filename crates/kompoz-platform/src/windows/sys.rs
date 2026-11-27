//! Win32 as the Windows platform uses it, through the `windows` crate's bindings, made safe here (Linux's `sys` and
//! FFI modules' counterpart): wide strings, events and high-resolution timers, the thread's message queue and the
//! wait on it, the console (Ctrl+C, a GUI program's parent's console), the process's DPI awareness, the window class
//! and its windows (a window's messages handed to a [`WindowHandler`] of the platform's, a panic in it carried out of
//! the window procedure and resumed on the loop), monitors, the keyboard's state and layout, cursors and icons, the
//! clipboard's formats and memory, the registry, power requests, the shell, message boxes, the media devices.
//!
//! A window's handle ([`Hwnd`]) is a plain value: Win32 checks the handles it is given, so a call on a window gone
//! fails (and does nothing) rather than touching freed memory. The one module of the platform with `unsafe` code
//! besides the Vulkan surface's and WinHTTP's: each block says why it holds.

#![allow(unsafe_code)]

use ::windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CR_SUCCESS,
};
use ::windows::Win32::Foundation::GlobalFree;
use ::windows::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_OBJECT_0, WPARAM,
};
use ::windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use ::windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DEVMODEW, DIB_RGB_COLORS, DeleteObject,
    ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW, HBITMAP, HGDIOBJ, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MONITORINFOEXW, MonitorFromPoint, MonitorFromWindow, ScreenToClient,
    ValidateRect,
};
use ::windows::Win32::Media::Audio::waveInGetNumDevs;
use ::windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use ::windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, CTRL_BREAK_EVENT, CTRL_C_EVENT, GetStdHandle, STD_ERROR_HANDLE,
    SetConsoleCtrlHandler, SetStdHandle,
};
use ::windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW, GetClipboardOwner,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use ::windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use ::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use ::windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use ::windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
};
use ::windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use ::windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CancelWaitableTimer, CreateEventW, CreateWaitableTimerExW, INFINITE,
    SetEvent, SetWaitableTimer, TIMER_ALL_ACCESS,
};
use ::windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, HKL, MAP_VIRTUAL_KEY_TYPE, MapVirtualKeyExW, ReleaseCapture,
    SetCapture, TME_LEAVE, TRACKMOUSEEVENT, ToUnicodeEx, TrackMouseEvent,
};
use ::windows::Win32::UI::Input::{GetRawInputDeviceList, RAWINPUTDEVICELIST, RIM_TYPEKEYBOARD};
use ::windows::Win32::UI::Shell::ShellExecuteW;
use ::windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyWindow, DispatchMessageW, GWL_EXSTYLE, GWL_STYLE, GWLP_USERDATA, GetClientRect, GetCursorPos,
    GetMessageExtraInfo, GetSystemMetrics, GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, HCURSOR, HICON,
    HWND_NOTOPMOST, HWND_TOPMOST, ICONINFO, IsIconic, IsWindowVisible, IsZoomed, KillTimer, LoadCursorW, LoadIconW,
    MB_ICONERROR, MB_OK, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, PostMessageW,
    QS_ALLINPUT, RegisterClassExW, SET_WINDOW_POS_FLAGS, SHOW_WINDOW_CMD, SM_DIGITIZER, SM_MOUSEPRESENT, SW_SHOWNORMAL,
    SYSTEM_METRICS_INDEX, SendMessageW, SetCursor, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WINDOWPLACEMENT, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW,
    WindowFromPoint,
};
use ::windows::core::{BOOL, GUID, PCWSTR};
use std::any::Any;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

// ---- Strings ----------------------------------------------------------------------------------------------------

/// `s` as UTF-16 with a terminating 0 (what Win32's wide functions take).
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// UTF-16 as UTF-8, up to its first 0 (unpaired surrogates as U+FFFD).
pub fn narrow(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

/// `GetLastError`'s code, as the log shows it.
pub fn last_error() -> String {
    // SAFETY: reads the thread's last error.
    let code = unsafe { GetLastError() };
    format!("error {}", code.0)
}

// ---- Events, timers, the wait -----------------------------------------------------------------------------------

/// A kernel object's handle, closed when it goes. Kernel handles are the process's, usable from any thread.
#[derive(Debug)]
struct OwnedHandle(HANDLE);

// SAFETY: a kernel object's handle is a process-wide value; the functions called on it (SetEvent, the waits,
// SetWaitableTimer) are thread-safe.
unsafe impl Send for OwnedHandle {}
// SAFETY: as above: shared use only calls thread-safe functions.
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle is ours (made by CreateEventW / CreateWaitableTimerExW) and closed once, here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// An auto-reset event: set from any thread, it wakes one wait and resets.
#[derive(Clone, Debug)]
pub struct EventHandle(Arc<OwnedHandle>);

impl EventHandle {
    /// A new event, not set; none if the system can't make one.
    pub fn new() -> Option<EventHandle> {
        // SAFETY: no attributes, no name: an anonymous auto-reset event.
        let handle = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.ok()?;
        Some(EventHandle(Arc::new(OwnedHandle(handle))))
    }

    /// Set (any thread).
    pub fn set(&self) {
        // SAFETY: the event is alive (held by this Arc).
        let _ = unsafe { SetEvent(self.0.0) };
    }

    fn raw(&self) -> HANDLE {
        self.0.0
    }
}

/// A waitable timer, high-resolution where the system has those (Windows 10 1803: a millisecond and better rather
/// than the scheduler's tick, 15.6 ms by default), else an ordinary one.
#[derive(Debug)]
pub struct WaitableTimer(OwnedHandle);

impl WaitableTimer {
    pub fn new() -> Option<WaitableTimer> {
        // SAFETY: no attributes, no name; the flag is refused (ERROR_INVALID_PARAMETER) before 1803, then the plain
        // timer is made.
        let made = unsafe {
            CreateWaitableTimerExW(None, PCWSTR::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS.0)
        }
        // SAFETY: as above, without the flag.
        .or_else(|_| unsafe { CreateWaitableTimerExW(None, PCWSTR::null(), 0, TIMER_ALL_ACCESS.0) })
        .ok()?;
        Some(WaitableTimer(OwnedHandle(made)))
    }

    /// Signalled `ns` from now (once).
    pub fn set_in(&self, ns: i64) {
        let due: i64 = -(ns / 100).max(1); // (relative: negative, in 100 ns units)
        // SAFETY: the timer is alive; `due` lives through the call; no completion routine.
        let _ = unsafe { SetWaitableTimer(self.0.0, &due, 0, None, None, false) };
    }

    /// Not signalled until set again.
    pub fn cancel(&self) {
        // SAFETY: the timer is alive.
        let _ = unsafe { CancelWaitableTimer(self.0.0) };
    }
}

/// What ended a wait on the thread's queue and two objects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Woken {
    /// The first object (`wait_for_messages`'s order) was signalled.
    First,
    Second,
    /// Messages (or input) came.
    Messages,
    /// The wait failed (the objects gone).
    Failed,
}

/// Until a message comes to the thread's queue (any: input, sent, posted, timers, paints) or `first` or `second` is
/// signalled (C++'s `MsgWaitForMultipleObjectsEx(…, QS_ALLINPUT, MWMO_INPUTAVAILABLE)`: messages already queued but
/// not yet looked at end it too).
pub fn wait_for_messages(first: &EventHandle, second: &WaitableTimer) -> Woken {
    let handles = [first.raw(), second.0.0];
    // SAFETY: both handles are alive (borrowed for the call).
    let woken = unsafe { MsgWaitForMultipleObjectsEx(Some(&handles), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE) };
    match woken.0.wrapping_sub(WAIT_OBJECT_0.0) {
        0 => Woken::First,
        1 => Woken::Second,
        2 => Woken::Messages,
        _ => Woken::Failed,
    }
}

/// A message of the thread's queue, as `PeekMessageW` took it.
pub struct Message(MSG);

impl Message {
    pub fn id(&self) -> u32 {
        self.0.message
    }
    pub fn wparam(&self) -> usize {
        self.0.wParam.0
    }
}

/// The next message of the thread's queue, taken off it (sent messages are delivered to their windows meanwhile);
/// none if there is none.
pub fn peek_message() -> Option<Message> {
    let mut msg = MSG::default();
    // SAFETY: `msg` is written by the call; any window of the thread, any message.
    let got = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) };
    got.as_bool().then_some(Message(msg))
}

/// The next message of the thread's queue for `hwnd` in `first..=last` (left on it), with its time: what a key
/// message looks ahead at (the right Alt's message after the left Ctrl Windows makes up for AltGr).
pub fn peek_next(hwnd: Hwnd, first: u32, last: u32) -> Option<(u32, usize, isize, i32)> {
    let mut msg = MSG::default();
    // SAFETY: `msg` is written; the message stays queued (PM_NOREMOVE).
    let got = unsafe {
        PeekMessageW(&mut msg, Some(hwnd.raw()), first, last, ::windows::Win32::UI::WindowsAndMessaging::PM_NOREMOVE)
    };
    got.as_bool().then_some((msg.message, msg.wParam.0, msg.lParam.0, msg.time as i32))
}

/// The time of the message handled (`GetMessageTime`, ms on the tick count).
pub fn message_time() -> i32 {
    // SAFETY: a query of the thread's last message.
    unsafe { ::windows::Win32::UI::WindowsAndMessaging::GetMessageTime() }
}

/// A message translated (a key's characters posted as WM_CHAR) and handed to its window's procedure.
pub fn dispatch(message: &Message) {
    // SAFETY: the message is one PeekMessageW gave; TranslateMessage and DispatchMessageW take it by pointer for the
    // call. (A modal loop a window enters in its procedure runs inside DispatchMessageW.)
    unsafe {
        let _ = ::windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&message.0);
        DispatchMessageW(&message.0);
    }
    resume_panic();
}

// ---- The console, the debugger's output -------------------------------------------------------------------------

static CTRL_C: AtomicBool = AtomicBool::new(false);
/// The event Ctrl+C sets (its handle's value; 0: none), kept alive by the platform while the handler is installed.
static CTRL_C_WAKE: AtomicIsize = AtomicIsize::new(0);
/// The event behind `CTRL_C_WAKE`, kept for as long as the process (the handler may run at any time).
static CTRL_C_EVENT_KEPT: std::sync::OnceLock<EventHandle> = std::sync::OnceLock::new();

unsafe extern "system" fn console_handler(kind: u32) -> BOOL {
    if kind != CTRL_C_EVENT && kind != CTRL_BREAK_EVENT {
        return BOOL(0);
    }
    CTRL_C.store(true, Ordering::SeqCst);
    let raw = CTRL_C_WAKE.load(Ordering::SeqCst);
    if raw != 0 {
        // SAFETY: the event is kept for the process's life (CTRL_C_EVENT_KEPT); SetEvent is thread-safe (the
        // handler runs on a thread of the console's).
        let _ = unsafe { SetEvent(HANDLE(raw as *mut c_void)) };
    }
    BOOL(1)
}

/// Ctrl+C and Ctrl+Break in the console the program runs in: a quit (taken with [`take_ctrl_c`]), `wake` set (C++'s:
/// the app writes its state on the way out). Once per process.
pub fn on_ctrl_c(wake: &EventHandle) {
    let kept = CTRL_C_EVENT_KEPT.get_or_init(|| wake.clone());
    CTRL_C_WAKE.store(kept.raw().0 as isize, Ordering::SeqCst);
    // SAFETY: the handler is a plain function with the routine's signature, alive for the process's life.
    let _ = unsafe { SetConsoleCtrlHandler(Some(console_handler), true) };
}

/// Whether Ctrl+C came since last asked.
pub fn take_ctrl_c() -> bool {
    CTRL_C.swap(false, Ordering::SeqCst)
}

/// A program of the GUI subsystem (no console of its own) started from a console: its log lines to that console
/// (its standard error attached to the parent's console, where it wasn't given one: not redirected). Nothing for a
/// console program, or one whose standard error goes somewhere already.
pub fn attach_parent_console() {
    // SAFETY: reads the process's standard handle.
    let given = unsafe { GetStdHandle(STD_ERROR_HANDLE) }.is_ok_and(|h| !h.is_invalid() && !h.0.is_null());
    if given {
        return;
    }
    // SAFETY: attaching fails (harmlessly) where the process has a console or the parent has none.
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_err() {
        return;
    }
    let name = wide("CONOUT$");
    // SAFETY: opens the console's output for writing; the name lives through the call.
    let out = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    if let Ok(out) = out {
        // SAFETY: the handle is the process's from now on (never closed: standard error's for its life).
        let _ = unsafe { SetStdHandle(STD_ERROR_HANDLE, out) };
    }
}

/// A line to the debugger (`OutputDebugStringW`: Visual Studio's output, DebugView, wine's `+debugstr`).
pub fn debug_output(line: &str) {
    let text = wide(&format!("{line}\n"));
    // SAFETY: a 0-terminated wide string living through the call.
    unsafe { OutputDebugStringW(PCWSTR(text.as_ptr())) };
}

// ---- The process's DPI awareness ---------------------------------------------------------------------------------

/// The process aware of each monitor's DPI (per-monitor v2: its windows get WM_DPICHANGED, their frames drawn at
/// their DPI), looked up at run time (Windows 10 1703; a manifest's `PerMonitorV2` does the same before the program
/// starts); else aware of the system's DPI. Whether per-monitor v2 is on.
pub fn set_per_monitor_dpi_awareness() -> bool {
    type SetContext = unsafe extern "system" fn(isize) -> BOOL;
    let user32 = wide("user32.dll");
    // SAFETY: user32 is loaded in every GUI process; the name lives through the calls; the function, where it is
    // there, has the signature SetProcessDpiAwarenessContext has (a context handle, BOOL).
    unsafe {
        if let Ok(module) = GetModuleHandleW(PCWSTR(user32.as_ptr()))
            && let Some(f) = GetProcAddress(module, ::windows::core::s!("SetProcessDpiAwarenessContext"))
        {
            let set: SetContext = std::mem::transmute(f);
            const PER_MONITOR_AWARE_V2: isize = -4;
            if set(PER_MONITOR_AWARE_V2).as_bool() {
                return true;
            }
            // (already set, by the manifest: as asked)
            return ::windows::Win32::UI::HiDpi::GetAwarenessFromDpiAwarenessContext(
                ::windows::Win32::UI::HiDpi::GetThreadDpiAwarenessContext(),
            ) == ::windows::Win32::UI::HiDpi::DPI_AWARENESS_PER_MONITOR_AWARE;
        }
        let _ = ::windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
    }
    false
}

/// The system's DPI (the primary monitor's at the session's start).
pub fn system_dpi() -> u32 {
    // SAFETY: a query.
    unsafe { GetDpiForSystem() }.max(1)
}

// ---- Panics in window procedures --------------------------------------------------------------------------------

thread_local! {
    /// A panic caught in a window procedure (it can't unwind through Win32's frames), resumed once out of them.
    static PANIC: RefCell<Option<Box<dyn Any + Send>>> = const { RefCell::new(None) };
}

/// A panic a window procedure caught, unwound from here (out of Win32's frames: after `DispatchMessageW`, after a
/// call that sent messages).
pub fn resume_panic() {
    if let Some(payload) = PANIC.with(|p| p.borrow_mut().take()) {
        std::panic::resume_unwind(payload);
    }
}

// ---- Windows ----------------------------------------------------------------------------------------------------

/// What a window's messages are handed to: its answer, or none for Windows' own handling (`DefWindowProcW`).
pub trait WindowHandler {
    fn message(&self, hwnd: Hwnd, message: &WindowMessage) -> Option<isize>;
}

/// A message a window's procedure is handling: its id and parameters, and what its parameters point to for the
/// messages whose lParam is a pointer (read here, while the message is handled: only a message the procedure was
/// given is one of these).
pub struct WindowMessage {
    id: u32,
    w: usize,
    l: isize,
}

impl WindowMessage {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn w(&self) -> usize {
        self.w
    }
    pub fn l(&self) -> isize {
        self.l
    }

    /// WM_SETTINGCHANGE's setting area's name ("ImmersiveColorSet", "intl"…), where it says one.
    pub fn setting_name(&self) -> Option<String> {
        if self.id != ::windows::Win32::UI::WindowsAndMessaging::WM_SETTINGCHANGE || self.l == 0 {
            return None;
        }
        // SAFETY: WM_SETTINGCHANGE's lParam, when not 0, points to a 0-terminated wide string valid while the
        // message is handled (this value lives only that long: made by the window procedure for the handler).
        let text = unsafe { PCWSTR(self.l as *const u16).to_string() };
        text.ok()
    }

    /// WM_GETMINMAXINFO answered: the window's largest size, the user's dragging and the app's own resizing alike,
    /// `width` × `height` (its frame's), rather than the screen's.
    pub fn set_max_track_size(&self, width: i32, height: i32) {
        if self.id != ::windows::Win32::UI::WindowsAndMessaging::WM_GETMINMAXINFO || self.l == 0 {
            return;
        }
        // SAFETY: WM_GETMINMAXINFO's lParam points to a MINMAXINFO the window may change while the message is
        // handled (see `setting_name`).
        let info = unsafe { &mut *(self.l as *mut ::windows::Win32::UI::WindowsAndMessaging::MINMAXINFO) };
        info.ptMaxTrackSize = POINT { x: width, y: height };
    }

    /// WM_DPICHANGED's suggested rectangle for the window at its new DPI.
    pub fn suggested_rect(&self) -> Option<Rect> {
        if self.id != ::windows::Win32::UI::WindowsAndMessaging::WM_DPICHANGED || self.l == 0 {
            return None;
        }
        // SAFETY: WM_DPICHANGED's lParam points to a RECT valid while the message is handled (see above).
        Some(Rect::of(unsafe { *(self.l as *const RECT) }))
    }
}

/// The handler a window was made with, kept in its `GWLP_USERDATA` from WM_NCCREATE to WM_NCDESTROY.
type Handler = Rc<dyn WindowHandler>;

unsafe extern "system" fn window_procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: WM_NCCREATE's lParam is the CREATESTRUCTW of CreateWindowExW, whose lpCreateParams is the
    // `Box<Handler>` `create_window` leaked; it is the window's from here (its user data) until WM_NCDESTROY takes
    // it back. The handler is cloned before it is called: a window destroyed while its handler runs keeps it alive.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(l.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let data = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Handler;
        if data.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        let handler = (*data).clone();
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(data as *mut Handler));
        }
        let at = Hwnd(hwnd.0 as isize);
        let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handler.message(at, &WindowMessage { id: message, w: w.0, l: l.0 })
        }));
        match answer {
            Ok(Some(result)) => LRESULT(result),
            Ok(None) => DefWindowProcW(hwnd, message, w, l),
            Err(payload) => {
                PANIC.with(|p| *p.borrow_mut() = Some(payload));
                DefWindowProcW(hwnd, message, w, l)
            }
        }
    }
}

/// The program's module (what its resources and classes are of).
fn instance() -> HINSTANCE {
    // SAFETY: the process's own module (none: the executable's).
    unsafe { GetModuleHandleW(PCWSTR::null()) }.map(|m| HINSTANCE(m.0)).unwrap_or_default()
}

/// The window class `name` registered (its windows drawn again in whole as they are resized: CS_HREDRAW |
/// CS_VREDRAW; the arrow cursor; the program's icon of resource id 1, else Windows' application icon). Whether it is
/// (registered before too).
pub fn register_class(name: &str) -> bool {
    let class = wide(name);
    let hinstance = instance();
    // SAFETY: loads resources by id (MAKEINTRESOURCE(1)) and the system's; the class's strings live through
    // RegisterClassExW, which copies them; the procedure is a plain function alive for the process's life.
    unsafe {
        let icon = LoadIconW(Some(hinstance), PCWSTR(std::ptr::without_provenance(1)))
            .or_else(|_| LoadIconW(None, ::windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION))
            .unwrap_or_default();
        let c = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_procedure),
            hInstance: hinstance,
            hIcon: icon,
            hCursor: LoadCursorW(None, ::windows::Win32::UI::WindowsAndMessaging::IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&c) != 0 || GetLastError() == ::windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS
    }
}

/// A window's place and size as made (`CW_USEDEFAULT` for Windows' own place).
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// A window of class `class` (registered), its messages to `handler` (from WM_NCCREATE on); none if it can't be
/// made (logged by the caller, `last_error`).
pub fn create_window(
    class: &str,
    title: &str,
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    at: Placed,
    handler: Handler,
) -> Option<Hwnd> {
    let (class, title) = (wide(class), wide(title));
    let data = Box::into_raw(Box::new(handler));
    // SAFETY: the strings live through the call; `data` is the handler box the window procedure takes at
    // WM_NCCREATE (and frees at WM_NCDESTROY); if the window isn't made it is freed here (a window that got as far
    // as WM_NCCREATE and then failed got WM_NCDESTROY, which freed it: its user data is the sign).
    unsafe {
        let made = CreateWindowExW(
            ex_style,
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
            at.x,
            at.y,
            at.width,
            at.height,
            None,
            None,
            Some(instance()),
            Some(data as *const c_void),
        );
        match made {
            Ok(hwnd) if !hwnd.is_invalid() => Some(Hwnd(hwnd.0 as isize)),
            _ => None,
        }
    }
}

/// A hidden top-level window of class `class`: what owns the clipboard and hears the system's broadcasts (settings,
/// the session ending, displays) whatever windows the app has.
pub fn create_hidden_window(class: &str, handler: Handler) -> Option<Hwnd> {
    create_window(
        class,
        "",
        WINDOW_STYLE(0),
        ::windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW,
        Placed { x: 0, y: 0, width: 0, height: 0 },
        handler,
    )
}

/// A window's handle (its value: see the module's notes). `Hwnd::NONE`: no window.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Hwnd(isize);

/// A rectangle in pixels: its left, top, right, bottom.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    fn of(r: RECT) -> Rect {
        Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }
    fn raw(&self) -> RECT {
        RECT { left: self.left, top: self.top, right: self.right, bottom: self.bottom }
    }
}

/// A monitor as a window sees it: its whole rectangle and its work area (without the taskbar), pixels on the
/// virtual screen; its device's name (what its display settings are asked by).
#[derive(Clone, Debug, Default)]
pub struct Monitor {
    pub rect: Rect,
    pub work: Rect,
    device: Vec<u16>,
}

impl Monitor {
    /// Its current refresh rate (`EnumDisplaySettingsW`), Hz; none if it isn't told (1 and 0 mean the hardware's
    /// default).
    pub fn refresh_rate(&self) -> Option<f32> {
        let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
        // SAFETY: the device name is 0-terminated (from MONITORINFOEXW) and lives through the call; `mode` is
        // written.
        let ok = unsafe { EnumDisplaySettingsW(PCWSTR(self.device.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) };
        (ok.as_bool() && mode.dmDisplayFrequency > 1).then_some(mode.dmDisplayFrequency as f32)
    }
}

fn monitor_info(monitor: ::windows::Win32::Graphics::Gdi::HMONITOR) -> Option<Monitor> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: `info` is a MONITORINFOEXW whose cbSize says so (the call writes the device name too).
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) };
    ok.as_bool().then(|| Monitor {
        rect: Rect::of(info.monitorInfo.rcMonitor),
        work: Rect::of(info.monitorInfo.rcWork),
        device: info.szDevice.to_vec(),
    })
}

/// The primary monitor.
pub fn primary_monitor() -> Option<Monitor> {
    // SAFETY: a query.
    monitor_info(unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) })
}

/// The window's place and its restored rectangle, as `GetWindowPlacement` tells them.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowPlacementInfo {
    /// SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWNORMAL…
    pub show: u32,
    /// Where it goes when restored (workspace coordinates).
    pub normal: Rect,
}

impl Hwnd {
    pub const NONE: Hwnd = Hwnd(0);

    fn raw(self) -> HWND {
        HWND(self.0 as *mut c_void)
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }

    /// Its value (what Vulkan's surface is made with).
    pub fn value(self) -> isize {
        self.0
    }

    /// The program's module's handle's value (what its windows are made with: Vulkan's surface's `hinstance`).
    pub fn module_handle(self) -> isize {
        instance().0 as isize
    }

    /// Destroyed (WM_DESTROY, WM_NCDESTROY sent to it first).
    pub fn destroy(self) {
        // SAFETY: a window of this thread's (DestroyWindow fails for another thread's or one gone).
        let _ = unsafe { DestroyWindow(self.raw()) };
    }

    /// `ShowWindow` with `command` (SW_*); whether it was visible before.
    pub fn show(self, command: SHOW_WINDOW_CMD) -> bool {
        // SAFETY: see the module's notes on handles.
        unsafe { ShowWindow(self.raw(), command) }.as_bool()
    }

    /// Shown normally (its first show: the program's start-up info may say otherwise, as Windows does).
    pub fn show_normal(self) {
        self.show(SW_SHOWNORMAL);
    }

    /// `SetWindowPos`: placed after `after` in the z-order (none: kept), at (x, y), w × h, as `flags` say.
    pub fn set_pos(self, after: Option<isize>, x: i32, y: i32, width: i32, height: i32, flags: SET_WINDOW_POS_FLAGS) {
        let after = after.map(|a| HWND(a as *mut c_void));
        // SAFETY: see the module's notes on handles.
        let _ = unsafe { SetWindowPos(self.raw(), after, x, y, width, height, flags) };
    }

    /// Above all others (topmost) or not.
    pub fn set_topmost(self, on_top: bool, flags: SET_WINDOW_POS_FLAGS) {
        let after = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
        self.set_pos(Some(after.0 as isize), 0, 0, 0, 0, flags);
    }

    /// The client area's size (0 × 0 minimized).
    pub fn client_size(self) -> (i32, i32) {
        let mut r = RECT::default();
        // SAFETY: `r` is written.
        let _ = unsafe { GetClientRect(self.raw(), &mut r) };
        (r.right - r.left, r.bottom - r.top)
    }

    /// The window's rectangle with its frame, pixels on the virtual screen.
    pub fn window_rect(self) -> Rect {
        let mut r = RECT::default();
        // SAFETY: `r` is written.
        let _ = unsafe { GetWindowRect(self.raw(), &mut r) };
        Rect::of(r)
    }

    /// Its DPI (its monitor's, per-monitor aware; the system's otherwise).
    pub fn dpi(self) -> u32 {
        // SAFETY: see the module's notes on handles.
        let dpi = unsafe { GetDpiForWindow(self.raw()) };
        if dpi == 0 { system_dpi() } else { dpi }
    }

    pub fn style(self) -> WINDOW_STYLE {
        // SAFETY: see the module's notes on handles.
        WINDOW_STYLE(unsafe { GetWindowLongPtrW(self.raw(), GWL_STYLE) } as u32)
    }

    pub fn ex_style(self) -> WINDOW_EX_STYLE {
        // SAFETY: see the module's notes on handles.
        WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(self.raw(), GWL_EXSTYLE) } as u32)
    }

    /// Its styles changed (the frame recomputed by a `set_pos` with SWP_FRAMECHANGED after).
    pub fn set_styles(self, style: WINDOW_STYLE, ex_style: WINDOW_EX_STYLE) {
        // SAFETY: see the module's notes on handles.
        unsafe {
            SetWindowLongPtrW(self.raw(), GWL_STYLE, style.0 as isize);
            SetWindowLongPtrW(self.raw(), GWL_EXSTYLE, ex_style.0 as isize);
        }
    }

    /// The window's size with its frame for a client area of `width` × `height` at `dpi`, with its styles.
    pub fn frame_size(self, width: i32, height: i32, dpi: u32) -> (i32, i32) {
        frame_size(self.style(), self.ex_style(), width, height, dpi)
    }

    pub fn set_title(self, title: &str) {
        let text = wide(title);
        // SAFETY: a 0-terminated wide string living through the call (WM_SETTEXT is sent to the window).
        let _ = unsafe { SetWindowTextW(self.raw(), PCWSTR(text.as_ptr())) };
    }

    pub fn is_minimized(self) -> bool {
        // SAFETY: see the module's notes on handles.
        unsafe { IsIconic(self.raw()) }.as_bool()
    }

    pub fn is_maximized(self) -> bool {
        // SAFETY: see the module's notes on handles.
        unsafe { IsZoomed(self.raw()) }.as_bool()
    }

    pub fn is_visible(self) -> bool {
        // SAFETY: see the module's notes on handles.
        unsafe { IsWindowVisible(self.raw()) }.as_bool()
    }

    /// To the foreground (as far as Windows lets the app: when its input was last).
    pub fn set_foreground(self) {
        // SAFETY: see the module's notes on handles.
        let _ = unsafe { SetForegroundWindow(self.raw()) };
    }

    /// The mouse captured by the window (its moves and its buttons' release come to it, outside it too).
    pub fn capture(self) {
        // SAFETY: see the module's notes on handles.
        unsafe { SetCapture(self.raw()) };
    }

    /// WM_MOUSELEAVE asked for when the mouse leaves the client area (once); whether Windows will send it.
    pub fn track_mouse_leave(self) -> bool {
        let mut track = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: self.raw(),
            dwHoverTime: 0,
        };
        // SAFETY: `track` lives through the call.
        unsafe { TrackMouseEvent(&mut track) }.is_ok()
    }

    /// A point on the screen in the client area's pixels.
    pub fn screen_to_client(self, x: i32, y: i32) -> (i32, i32) {
        let mut p = POINT { x, y };
        // SAFETY: `p` is read and written.
        let _ = unsafe { ScreenToClient(self.raw(), &mut p) };
        (p.x, p.y)
    }

    /// The client area's top left on the screen.
    pub fn client_origin(self) -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: `p` is read and written.
        let _ = unsafe { ::windows::Win32::Graphics::Gdi::ClientToScreen(self.raw(), &mut p) };
        (p.x, p.y)
    }

    /// The monitor the window is on (the nearest).
    pub fn monitor(self) -> Option<Monitor> {
        // SAFETY: a query.
        monitor_info(unsafe { MonitorFromWindow(self.raw(), MONITOR_DEFAULTTONEAREST) })
    }

    /// Whether the mouse is over the window now (its topmost window at the cursor is this one).
    pub fn under_cursor(self) -> bool {
        let mut p = POINT::default();
        // SAFETY: `p` is written; WindowFromPoint is a query.
        unsafe { GetCursorPos(&mut p).is_ok() && WindowFromPoint(p) == self.raw() }
    }

    /// A timer of the window's own: WM_TIMER with `id` every `ms` (Windows' least: 10 ms).
    pub fn set_timer(self, id: usize, ms: u32) {
        // SAFETY: no timer procedure: WM_TIMER messages to the window.
        unsafe { SetTimer(Some(self.raw()), id, ms, None) };
    }

    pub fn kill_timer(self, id: usize) {
        // SAFETY: see the module's notes on handles.
        let _ = unsafe { KillTimer(Some(self.raw()), id) };
    }

    /// Its paint validated (KompozR draws it all: no WM_PAINT again until something is invalid again).
    pub fn validate(self) {
        // SAFETY: the whole client area.
        let _ = unsafe { ValidateRect(Some(self.raw()), None) };
    }

    /// A message posted to it (handled by the loop, after what is queued).
    pub fn post(self, message: u32, w: usize, l: isize) {
        // SAFETY: see the module's notes on handles.
        let _ = unsafe { PostMessageW(Some(self.raw()), message, WPARAM(w), LPARAM(l)) };
    }

    /// A message sent to it (its procedure called now).
    pub fn send(self, message: u32, w: usize, l: isize) -> isize {
        // SAFETY: see the module's notes on handles; the message's arguments are plain values.
        unsafe { SendMessageW(self.raw(), message, Some(WPARAM(w)), Some(LPARAM(l))) }.0
    }

    /// `GetWindowPlacement`: how it is shown and where it goes restored.
    pub fn placement(self) -> WindowPlacementInfo {
        let mut p = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
        // SAFETY: `p` is written; its length says what it is.
        let _ = unsafe { GetWindowPlacement(self.raw(), &mut p) };
        WindowPlacementInfo { show: p.showCmd, normal: Rect::of(p.rcNormalPosition) }
    }

    /// A DWM attribute set (a `u32`/BOOL/COLORREF/enum value, as each is); whether DWM took it (older systems and
    /// wine refuse those they don't know).
    pub fn set_dwm_attribute(self, attribute: u32, value: u32) -> bool {
        // SAFETY: the value lives through the call; its size is said.
        unsafe {
            DwmSetWindowAttribute(
                self.raw(),
                DWMWINDOWATTRIBUTE(attribute as i32),
                &value as *const u32 as *const c_void,
                std::mem::size_of::<u32>() as u32,
            )
        }
        .is_ok()
    }

    /// Its icons (WM_SETICON small and big) `icon` (kept by the caller while the window shows it); none: the
    /// class's.
    pub fn set_icon(self, icon: Option<&Icon>) {
        let handle = icon.map_or(0, |i| i.0.0 as isize);
        for kind in [0usize, 1] {
            // (ICON_SMALL, ICON_BIG)
            self.send(::windows::Win32::UI::WindowsAndMessaging::WM_SETICON, kind, handle);
        }
    }
}

/// The window's size with its frame for a client area of `width` × `height` at `dpi`, with `style` and `ex_style`.
pub fn frame_size(style: WINDOW_STYLE, ex_style: WINDOW_EX_STYLE, width: i32, height: i32, dpi: u32) -> (i32, i32) {
    let mut r = Rect { left: 0, top: 0, right: width, bottom: height }.raw();
    // SAFETY: `r` is read and written.
    let _ = unsafe { AdjustWindowRectExForDpi(&mut r, style, false, ex_style, dpi) };
    (r.right - r.left, r.bottom - r.top)
}

/// The mouse let go (by whichever window had it).
pub fn release_capture() {
    // SAFETY: no arguments.
    let _ = unsafe { ReleaseCapture() };
}

/// `GetMessageExtraInfo` of the message handled: what tells mouse messages Windows made of a touch or a pen apart
/// (their signature 0xFF515700, the low byte's top bit: touch).
pub fn message_extra_info() -> isize {
    // SAFETY: a query of the thread's last message.
    unsafe { GetMessageExtraInfo() }.0
}

/// A message box with `text`, the error icon and OK (C++'s `showError`).
pub fn message_box(owner: Option<Hwnd>, title: &str, text: &str) {
    let (title, text) = (wide(title), wide(text));
    // SAFETY: the strings live through the call (a modal loop of its own).
    unsafe {
        ::windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
            owner.map(Hwnd::raw),
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
    resume_panic();
}

// ---- The keyboard ------------------------------------------------------------------------------------------------

/// Whether virtual key `vk` is down now (as of the message being handled: `GetKeyState`'s top bit).
pub fn key_down(vk: u16) -> bool {
    // SAFETY: a query.
    (unsafe { GetKeyState(vk as i32) } as u16 & 0x8000) != 0
}

/// Whether toggle key `vk` (Caps Lock, Num Lock) is on.
pub fn key_toggled(vk: u16) -> bool {
    // SAFETY: a query.
    (unsafe { GetKeyState(vk as i32) } & 1) != 0
}

/// The thread's keyboard layout (its HKL's value).
pub fn keyboard_layout() -> isize {
    // SAFETY: a query (0: this thread).
    unsafe { GetKeyboardLayout(0) }.0 as isize
}

/// `MapVirtualKeyExW` in the thread's layout: `code` mapped as `kind` says (MAPVK_*).
pub fn map_virtual_key(code: u32, kind: u32) -> u32 {
    // SAFETY: a query.
    unsafe { MapVirtualKeyExW(code, MAP_VIRTUAL_KEY_TYPE(kind), Some(HKL(keyboard_layout() as *mut c_void))) }
}

/// The characters key `vk` (scan code `scan`) types in the thread's layout with the keyboard's state now, its own
/// state left as it is (`ToUnicodeEx`'s flag 4, Windows 10 1607: a dead key isn't taken); with `without_modifiers`,
/// as it types alone (its key's own character: what a shortcut names it by). A dead key's character comes as
/// `Err`.
pub fn key_characters(vk: u16, scan: u16, without_modifiers: bool) -> Result<String, String> {
    let mut state = [0u8; 256];
    if !without_modifiers {
        // SAFETY: `state` is written.
        let _ = unsafe { GetKeyboardState(&mut state) };
    }
    let mut buffer = [0u16; 8];
    let layout = HKL(keyboard_layout() as *mut c_void);
    // SAFETY: the state and the buffer live through the call; flag 4 keeps the keyboard's state.
    let n = unsafe { ToUnicodeEx(vk as u32, scan as u32, &state, &mut buffer, 4, Some(layout)) };
    let text = String::from_utf16_lossy(&buffer[..n.unsigned_abs().min(8) as usize]);
    if n < 0 { Err(text) } else { Ok(text) }
}

// ---- Cursors and icons -------------------------------------------------------------------------------------------

/// One of the system's cursors (shared: never destroyed).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cursor(HCURSOR);

impl Cursor {
    /// The system's cursor `id` (IDC_*); none if it isn't there.
    pub fn system(id: PCWSTR) -> Option<Cursor> {
        // SAFETY: a system cursor by id (no module).
        unsafe { LoadCursorW(None, id) }.ok().map(Cursor)
    }

    /// The cursor shown (until the mouse moves into another window's area that sets its own).
    pub fn show(self) {
        // SAFETY: a cursor loaded from the system, never destroyed.
        unsafe { SetCursor(Some(self.0)) };
    }
}

/// An icon made from straight RGBA pixels (a window's), destroyed when it goes.
#[derive(Debug)]
pub struct Icon(HICON);

impl Icon {
    /// `rgba` (`width` × `height`, the top row first) as an icon with an alpha channel (a 32-bit DIB with a mask
    /// all zeros); none if it can't be made.
    pub fn from_rgba(width: i32, height: i32, rgba: &[u8]) -> Option<Icon> {
        if width <= 0 || height <= 0 || rgba.len() < (width * height * 4) as usize {
            return None;
        }
        let header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // (top-down)
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
        let mut bits: *mut c_void = std::ptr::null_mut();
        // SAFETY: the info lives through the call; the section's memory (width × height × 4 bytes, BGRA) is
        // written below while the bitmap lives; the mask is a 1-bit bitmap of zeros (the alpha decides).
        unsafe {
            let color: HBITMAP = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            if bits.is_null() {
                let _ = DeleteObject(HGDIOBJ(color.0));
                return None;
            }
            let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize);
            for (out, p) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(rgba.as_chunks::<4>().0) {
                out.copy_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
            let mask_bytes = vec![0u8; (((width + 15) / 16 * 2) * height) as usize];
            let mask = CreateBitmap(width, height, 1, 1, Some(mask_bytes.as_ptr() as *const c_void));
            let icon_info = ICONINFO { fIcon: BOOL(1), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
            let icon = CreateIconIndirect(&icon_info);
            let _ = DeleteObject(HGDIOBJ(color.0));
            let _ = DeleteObject(HGDIOBJ(mask.0));
            icon.ok().map(Icon)
        }
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: an icon this made (CreateIconIndirect), destroyed once.
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

// ---- The clipboard -----------------------------------------------------------------------------------------------

/// A clipboard format registered by name (the same number in every process of the session); 0 if it can't be.
pub fn register_clipboard_format(name: &str) -> u32 {
    let name = wide(name);
    // SAFETY: a 0-terminated wide string living through the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) }
}

/// A registered format's name (none for the predefined ones).
pub fn clipboard_format_name(format: u32) -> Option<String> {
    let mut buffer = [0u16; 256];
    // SAFETY: the buffer is written up to its length.
    let n = unsafe { GetClipboardFormatNameW(format, &mut buffer) };
    (n > 0).then(|| narrow(&buffer[..n as usize]))
}

/// The clipboard's sequence number (counted up as anything puts something on it).
pub fn clipboard_sequence() -> u32 {
    // SAFETY: a query.
    unsafe { GetClipboardSequenceNumber() }
}

/// Whether the clipboard has `format` (without opening it).
pub fn clipboard_has(format: u32) -> bool {
    // SAFETY: a query.
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// The window owning the clipboard (none: nobody).
pub fn clipboard_owner() -> Hwnd {
    // SAFETY: a query.
    unsafe { GetClipboardOwner() }.map_or(Hwnd::NONE, |h| Hwnd(h.0 as isize))
}

/// The clipboard opened by a window, closed when this goes (one process at a time has it open: others' opens fail
/// meanwhile, so it is held briefly).
pub struct OpenClipboardGuard(());

impl OpenClipboardGuard {
    /// Opened for `owner` (the window that owns what is put on it after `empty`); none if another has it open (tried
    /// a few times, a few ms apart, as clipboard managers hold it briefly after each change).
    pub fn open(owner: Hwnd) -> Option<OpenClipboardGuard> {
        for attempt in 0..5 {
            // SAFETY: opens the clipboard for the window (or the task, with none).
            if unsafe { OpenClipboard((!owner.is_none()).then(|| owner.raw())) }.is_ok() {
                return Some(OpenClipboardGuard(()));
            }
            std::thread::sleep(std::time::Duration::from_millis(2 << attempt));
        }
        None
    }

    /// Emptied: the opener's window owns the clipboard from here (what it holds is the owner's to give).
    pub fn empty(&self) -> bool {
        // SAFETY: the clipboard is open (this guard).
        unsafe { EmptyClipboard() }.is_ok()
    }

    /// The formats it has, in the order their owner put them there.
    pub fn formats(&self) -> Vec<u32> {
        let mut formats = Vec::new();
        let mut f = 0;
        loop {
            // SAFETY: the clipboard is open (this guard); 0 starts the enumeration, 0 ends it.
            f = unsafe { EnumClipboardFormats(f) };
            if f == 0 {
                break;
            }
            formats.push(f);
        }
        formats
    }

    /// `format`'s bytes (its global memory's; a delayed one rendered by its owner first).
    pub fn get(&self, format: u32) -> Option<Vec<u8>> {
        // SAFETY: the clipboard is open (this guard); the data handle is the clipboard's, read (locked) here only.
        let handle = unsafe { GetClipboardData(format) }.ok()?;
        global_bytes(HGLOBAL(handle.0))
    }

    /// `bytes` put on it as `format` (the clipboard owns their memory from here).
    pub fn set(&self, format: u32, bytes: &[u8]) -> bool {
        set_clipboard_data(format, Some(bytes))
    }

    /// `format` promised (delayed rendering: its owner is asked for it with WM_RENDERFORMAT when someone wants it).
    pub fn promise(&self, format: u32) -> bool {
        set_clipboard_data(format, None)
    }
}

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: the clipboard is open (this guard), closed once.
        let _ = unsafe { CloseClipboard() };
    }
}

/// `SetClipboardData`: `bytes` as `format` (in global memory the clipboard owns once it takes it), or promised
/// (none). Called with the clipboard open, or (rendering a promised format: WM_RENDERFORMAT) without.
pub fn set_clipboard_data(format: u32, bytes: Option<&[u8]>) -> bool {
    let Some(bytes) = bytes else {
        // SAFETY: a promise: no memory. (Its answer is no handle whether it took it or not: whether the clipboard
        // has the format now says.)
        let _ = unsafe { SetClipboardData(format, None) };
        return clipboard_has(format);
    };
    let Some(memory) = global_of(bytes) else { return false };
    // SAFETY: the memory is a movable global block of ours; the clipboard owns it once it took it, else it is ours
    // to free.
    unsafe {
        if SetClipboardData(format, Some(HANDLE(memory.0))).is_err() {
            let _ = GlobalFree(Some(memory));
            return false;
        }
    }
    true
}

/// A global block's bytes (all of it: its size may be rounded up past what was put there).
fn global_bytes(memory: HGLOBAL) -> Option<Vec<u8>> {
    // SAFETY: the block is locked while it is read, its size as Windows tells it.
    unsafe {
        let p = GlobalLock(memory);
        if p.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(p as *const u8, GlobalSize(memory)).to_vec();
        let _ = GlobalUnlock(memory);
        Some(bytes)
    }
}

/// `bytes` in a new movable global block.
fn global_of(bytes: &[u8]) -> Option<HGLOBAL> {
    // SAFETY: a new block of at least one byte, written while locked.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).ok()?;
        let p = GlobalLock(memory);
        if p.is_null() {
            let _ = GlobalFree(Some(memory));
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
        let _ = GlobalUnlock(memory);
        Some(memory)
    }
}

// ---- The registry, power, the shell, devices ---------------------------------------------------------------------

/// A DWORD value under HKEY_CURRENT_USER (`key`'s `value`); none if it isn't there.
pub fn user_dword(key: &str, value: &str) -> Option<u32> {
    registry_dword(HKEY_CURRENT_USER, key, value)
}

fn registry_dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (wide(key), wide(value));
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: the strings live through the call; `data` (its size said) is written.
    let error = unsafe {
        RegGetValueW(
            root,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    error.is_ok().then_some(data)
}

/// The display (and the system) kept on, or let sleep again (`SetThreadExecutionState`: the thread's request,
/// which the UI thread holds while it runs).
pub fn keep_display_on(on: bool) {
    let flags = if on { ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED } else { ES_CONTINUOUS };
    // SAFETY: a request of the thread's.
    unsafe { SetThreadExecutionState(flags) };
}

/// `target` opened by the shell (`ShellExecuteW`'s "open": a URL in the browser, a file in its app); whether it
/// was.
pub fn shell_open(target: &str) -> bool {
    let (verb, file) = (wide("open"), wide(target));
    // SAFETY: the strings live through the call; no window, parameters or directory.
    let result = unsafe {
        ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(file.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
    };
    resume_panic();
    result.0 as isize > 32
}

/// A system metric (`GetSystemMetrics`).
pub fn system_metric(index: SYSTEM_METRICS_INDEX) -> i32 {
    // SAFETY: a query.
    unsafe { GetSystemMetrics(index) }
}

/// Whether a mouse is there.
pub fn has_mouse() -> bool {
    system_metric(SM_MOUSEPRESENT) != 0
}

/// Whether a touch screen is there and ready (`SM_DIGITIZER`: NID_READY with an integrated or external touch).
pub fn has_touch_screen() -> bool {
    let digitizer = system_metric(SM_DIGITIZER);
    const NID_INTEGRATED_TOUCH: i32 = 0x01;
    const NID_EXTERNAL_TOUCH: i32 = 0x02;
    const NID_READY: i32 = 0x80;
    digitizer & NID_READY != 0 && digitizer & (NID_INTEGRATED_TOUCH | NID_EXTERNAL_TOUCH) != 0
}

/// Whether a keyboard is attached (a raw input device of the keyboard's kind).
pub fn has_keyboard() -> bool {
    let size = std::mem::size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0u32;
    // SAFETY: the count is written (no list asked for).
    unsafe { GetRawInputDeviceList(None, &mut count, size) };
    if count == 0 {
        return false;
    }
    let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
    // SAFETY: the list has room for `count` entries, as said.
    let got = unsafe { GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, size) };
    got != u32::MAX && list[..got.min(count) as usize].iter().any(|d| d.dwType == RIM_TYPEKEYBOARD)
}

/// Whether a microphone is there (a wave input device).
pub fn has_microphone() -> bool {
    // SAFETY: a query.
    unsafe { waveInGetNumDevs() > 0 }
}

/// Whether a camera is there: a present device interface of KSCATEGORY_VIDEO_CAMERA.
pub fn has_camera() -> bool {
    const VIDEO_CAMERAS: GUID = GUID::from_u128(0xe5323777_f976_4f5b_9b55_b94699c46e44);
    let mut size = 0u32;
    // SAFETY: the size (in characters of the list) is written; no device id.
    let result = unsafe {
        CM_Get_Device_Interface_List_SizeW(
            &mut size,
            &VIDEO_CAMERAS,
            PCWSTR::null(),
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };
    result == CR_SUCCESS && size > 1 // (an empty list is its terminating 0)
}
