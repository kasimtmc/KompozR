//! Pens and touch screens on Windows (WM_POINTER, Windows 8's pointer input; C++ took fingers only and left pens to
//! the mouse messages Windows makes of them): fingers by their pointer ids, each frame of theirs (every finger the
//! digitizer reported at once, `GetPointerFrameTouchInfo`) one [`Event::TouchMoves`] with the positions each passed
//! through since its last report before it ([`Event::History`], `GetPointerInfoHistory`), a frame Windows calls off
//! (`POINTER_FLAG_CANCELED`) or a capture lost [`Event::TouchCancel`]; a pen near the window, touching or hovering, as
//! [`Event::Stylus`] reports (`GetPointerPenInfo`: its pressure, 0 to 1024, as 0 to 1; its eraser — the other end
//! or an eraser button, `PEN_FLAG_ERASER`/`PEN_FLAG_INVERTED`; its barrel button, `PEN_FLAG_BARREL`; its history too),
//! [`Event::StylusLeft`] as it goes. The mouse stays on the mouse messages (`EnableMouseInPointer` isn't called: the
//! mouse's own messages carry what the app needs, and its WM_MOUSELEAVE); those Windows makes of fingers and pens are
//! left out (their `GetMessageExtraInfo` signature), so nothing comes twice.
//!
//! What the core doesn't carry, and so isn't read: a pen's tilt and its turning (`tiltX`/`tiltY`/`rotation`:
//! [`Event::Stylus`] has no tilt, as Compose's `PointerInputChange` has none), a finger's contact size and pressure
//! (`rcContact`, `pressure`: Compose's touches carry neither). A precision touchpad is DirectManipulation's
//! (`manipulation`); a touch screen's pans and flings are the app's own (Compose's scrollables, from the fingers'
//! moves), so its fingers aren't given to DirectManipulation.
//!
//! The decoding is pure (tested on every host): what a report says, a pen's pressure and buttons, the time of a
//! report on the platform's clock, the history's order, a frame's fingers as events, a pen's reports as events.
//! Reading them from Windows goes through [`Source`] (user32's functions, or a test's reports: the seam the tests
//! send WM_POINTER messages through, since wine makes none of its own).

use crate::app::Event;
use std::collections::BTreeMap;

/// `POINTER_FLAGS` (winuser.h).
pub mod flag {
    pub const INRANGE: u32 = 0x2;
    pub const INCONTACT: u32 = 0x4;
    pub const CANCELED: u32 = 0x8000;
    // (What a report went down or moved by: its contact tells, these only the tests' reports.)
    #[cfg(test)]
    pub const DOWN: u32 = 0x1_0000;
    #[cfg(test)]
    pub const UPDATE: u32 = 0x2_0000;
    pub const UP: u32 = 0x4_0000;
}

/// `POINTER_PEN_INFO`'s pen flags and mask (winuser.h).
pub mod pen {
    pub const BARREL: u32 = 0x1;
    pub const INVERTED: u32 = 0x2;
    pub const ERASER: u32 = 0x4;
    pub const MASK_PRESSURE: u32 = 0x1;
}

/// The mouse messages Windows makes of pens and fingers (for apps that don't take WM_POINTER) carry this in their
/// `GetMessageExtraInfo` (`MI_WP_SIGNATURE`; the low byte's top bit: a finger's).
const MADE_OF_A_PEN_OR_A_FINGER: u32 = 0xFF51_5700;

/// Whether a mouse message with `extra` (its `GetMessageExtraInfo`) was made of a pen or a finger: one the app has
/// had as WM_POINTER already.
pub fn made_of_a_pen_or_a_finger(extra: isize) -> bool {
    extra as u32 & 0xFFFF_FF00 == MADE_OF_A_PEN_OR_A_FINGER
}

/// A pointer's device (`POINTER_INPUT_TYPE`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Touch,
    Pen,
    Mouse,
    Touchpad,
    Other,
}

impl Kind {
    /// `PT_TOUCH` (2), `PT_PEN` (3), `PT_MOUSE` (4), `PT_TOUCHPAD` (5); `PT_POINTER` (1) and what is unknown: other.
    pub fn of(pointer_type: i32) -> Kind {
        match pointer_type {
            2 => Kind::Touch,
            3 => Kind::Pen,
            4 => Kind::Mouse,
            5 => Kind::Touchpad,
            _ => Kind::Other,
        }
    }
}

/// A pointer's report (what matters of a `POINTER_INFO`): its id, its frame, its flags, where it is in the window
/// (client pixels: the window's points; a pen's to a part of a pixel), when (the platform's clock).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Report {
    pub id: u32,
    pub frame: u32,
    pub flags: u32,
    pub x: f32,
    pub y: f32,
    pub time: i64,
}

impl Report {
    fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Its pointer as the core knows it: Windows' id, one up (never the mouse's 0: Windows' ids may be 0).
    pub fn pointer(&self) -> i64 {
        i64::from(self.id) + 1
    }
}

/// A pen's report (what matters of a `POINTER_PEN_INFO`).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PenReport {
    pub report: Report,
    /// `PEN_FLAG_*`.
    pub flags: u32,
    /// `PEN_MASK_*`: what it measures.
    pub mask: u32,
    /// 0 to 1024 (where the mask says so).
    pub pressure: u32,
}

impl PenReport {
    fn touching(&self) -> bool {
        self.report.has(flag::INCONTACT)
    }

    /// 0 to 1: the pen's pressure, where it measures one; else full while it touches (Android's 1 for a tool
    /// without pressure), none while it hovers.
    pub fn pressure(&self) -> f32 {
        if !self.touching() {
            0.0
        } else if self.mask & pen::MASK_PRESSURE != 0 {
            self.pressure.min(1024) as f32 / 1024.0
        } else {
            1.0
        }
    }

    /// Its other end (inverted, hovering or touching), or an eraser button held on it.
    pub fn eraser(&self) -> bool {
        self.flags & (pen::INVERTED | pen::ERASER) != 0
    }

    /// Its barrel buttons as [`Event::Stylus`]'s bits: Windows tells one (the first).
    pub fn buttons(&self) -> u32 {
        u32::from(self.flags & pen::BARREL != 0)
    }
}

/// The clocks a report's time is told on, read as the message is handled: the platform's (`now`, ns), the
/// performance counter (`counter`, its `frequency` a second; 0: none).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Clock {
    pub now: i64,
    pub counter: i64,
    pub frequency: i64,
}

impl Clock {
    /// A report's time on the platform's clock: from its performance counter value (`PerformanceCount`, what the
    /// platform's clock runs on), else from its milliseconds (`dwTime`) before those of the latest report
    /// (`latest_ms`, taken as now).
    pub fn time(&self, performance_count: u64, ms: u32, latest_ms: u32) -> i64 {
        if performance_count != 0 && self.frequency > 0 && self.counter > 0 {
            let ago = (self.counter - performance_count as i64).max(0) as i128 * 1_000_000_000 / self.frequency as i128;
            return self.now - ago as i64;
        }
        self.now - i64::from(latest_ms.wrapping_sub(ms)) * 1_000_000
    }
}

/// Where a pen is on the screen to a part of a pixel: its device's position (HIMETRIC, `ptHimetricLocation`) put on
/// the display rectangle the device maps onto (`GetPointerDeviceRects`: the device's and the display's, left, top,
/// right, bottom). None where the device's rectangle is empty.
pub fn precise(himetric: (i32, i32), device: [i32; 4], display: [i32; 4]) -> Option<(f64, f64)> {
    let (dw, dh) = (f64::from(device[2] - device[0]), f64::from(device[3] - device[1]));
    if dw <= 0.0 || dh <= 0.0 {
        return None;
    }
    let (sw, sh) = (f64::from(display[2] - display[0]), f64::from(display[3] - display[1]));
    let x = f64::from(display[0]) + f64::from(himetric.0 - device[0]) * sw / dw;
    let y = f64::from(display[1]) + f64::from(himetric.1 - device[1]) * sh / dh;
    Some((x, y))
}

/// The [`Event::History`] of `pointer` from its reports newest first, the latest among them first
/// (`GetPointerInfoHistory`'s order): the ones before the latest, oldest first; none if there are none.
pub fn history(pointer: i64, newest_first: &[Report]) -> Option<Event> {
    let samples: Vec<(f32, f32, i64)> = newest_first.iter().skip(1).rev().map(|r| (r.x, r.y, r.time)).collect();
    (!samples.is_empty()).then_some(Event::History { pointer, samples })
}

/// A finger of a frame: its report, and its history (newest first, the report itself first; empty: none read).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Contact {
    pub report: Report,
    pub history: Vec<Report>,
}

/// What a pen near the window last reported (to leave out a report that tells nothing new).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Told {
    x: f32,
    y: f32,
    down: bool,
    pressure: f32,
    buttons: u32,
    eraser: bool,
}

/// A window's fingers and pens as the core was told of them.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Pointers {
    /// The fingers down on the window, where each was last.
    touches: BTreeMap<u32, (f32, f32)>,
    /// The touch frame handled last (each of its fingers' messages brings it again).
    frame: Option<u32>,
    /// The pens near the window, what each last reported.
    pens: BTreeMap<u32, Told>,
}

impl Pointers {
    /// A touch frame (every finger the digitizer reported at once) as the core's events: the fingers that moved
    /// as one [`Event::TouchMoves`] (their histories first), then those put down, then those lifted; a frame called
    /// off ends the fingers' gesture ([`Event::TouchCancel`]). A frame handled already (another of its fingers'
    /// messages) is nothing.
    pub fn touch_frame(&mut self, frame: &[Contact]) -> Vec<Event> {
        let Some(first) = frame.first() else { return Vec::new() };
        let id = first.report.frame;
        if id != 0 && self.frame == Some(id) {
            return Vec::new();
        }
        self.frame = Some(id);
        if frame.iter().any(|c| c.report.has(flag::CANCELED)) {
            return self.cancel_touches();
        }
        let time = frame.iter().map(|c| c.report.time).max().unwrap_or_default();
        let (mut moves, mut histories, mut downs, mut ups) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut moved = false;
        for c in frame {
            let r = &c.report;
            let (pointer, x, y) = (r.pointer(), r.x, r.y);
            let touching = r.has(flag::INCONTACT) && !r.has(flag::UP);
            match (self.touches.get_mut(&r.id), touching) {
                (None, true) => {
                    self.touches.insert(r.id, (x, y));
                    downs.push(Event::TouchDown { pointer, x, y, time: r.time });
                }
                (Some(_), false) => {
                    self.touches.remove(&r.id);
                    ups.push(Event::TouchUp { pointer, x, y, time: r.time });
                }
                (Some(at), true) => {
                    moved |= *at != (x, y) || c.history.len() > 1;
                    *at = (x, y);
                    moves.push((pointer, x, y));
                    histories.extend(history(pointer, &c.history));
                }
                // (A finger hovering over the screen, which some digitizers tell: not the app's.)
                (None, false) => {}
            }
        }
        let mut events = Vec::new();
        if moved {
            events.extend(histories);
            events.push(Event::TouchMoves { moves, time });
        }
        events.extend(downs);
        events.extend(ups);
        events
    }

    /// Every finger's gesture called off (a frame Windows cancelled, the window's capture of them lost).
    fn cancel_touches(&mut self) -> Vec<Event> {
        if self.touches.is_empty() {
            return Vec::new();
        }
        self.touches.clear();
        vec![Event::TouchCancel]
    }

    /// A pen's report (its history newest first, the report itself first): where it is, whether it touches and how
    /// hard, its eraser and button, after the positions it passed through; gone out of range: it left; called off:
    /// what it began touching cancelled. A report that tells nothing new is nothing.
    pub fn pen(&mut self, report: &PenReport, newest_first: &[Report]) -> Vec<Event> {
        let r = &report.report;
        if r.has(flag::CANCELED) {
            return self.cancel_pen(r.id);
        }
        if !r.has(flag::INRANGE) && !r.has(flag::INCONTACT) {
            return self.pen_left(r.id);
        }
        let told = Told {
            x: r.x,
            y: r.y,
            down: report.touching(),
            pressure: report.pressure(),
            buttons: report.buttons(),
            eraser: report.eraser(),
        };
        if self.pens.get(&r.id) == Some(&told) && newest_first.len() <= 1 {
            return Vec::new();
        }
        self.pens.insert(r.id, told);
        let pointer = r.pointer();
        let mut events: Vec<Event> = history(pointer, newest_first).into_iter().collect();
        events.push(Event::Stylus {
            pointer,
            eraser: told.eraser,
            x: told.x,
            y: told.y,
            down: told.down,
            pressure: told.pressure,
            buttons: told.buttons,
            time: r.time,
        });
        events
    }

    /// Pen `id` went away (WM_POINTERLEAVE: out of range, or off the window).
    pub fn pen_left(&mut self, id: u32) -> Vec<Event> {
        match self.pens.remove(&id) {
            Some(_) => vec![Event::StylusLeft { pointer: i64::from(id) + 1 }],
            None => Vec::new(),
        }
    }

    /// Pen `id`'s report called off: what its touch began cancelled, and it gone.
    fn cancel_pen(&mut self, id: u32) -> Vec<Event> {
        match self.pens.remove(&id) {
            Some(told) if told.down => vec![Event::TouchCancel, Event::StylusLeft { pointer: i64::from(id) + 1 }],
            Some(_) => vec![Event::StylusLeft { pointer: i64::from(id) + 1 }],
            None => Vec::new(),
        }
    }

    /// The window lost pointer `id`'s capture (WM_POINTERCAPTURECHANGED: another window, or the system, took it): a
    /// finger's gesture called off (all the fingers': the core cancels them together), a touching pen's too.
    pub fn capture_lost(&mut self, id: u32) -> Vec<Event> {
        if self.touches.contains_key(&id) {
            return self.cancel_touches();
        }
        if self.pens.get(&id).is_some_and(|t| t.down) {
            return self.cancel_pen(id);
        }
        Vec::new()
    }

    /// Whether a pen is near the window (its cursor the stylus's hover icon).
    pub fn pen_near(&self) -> bool {
        !self.pens.is_empty()
    }

    /// Whether pointer `id` is one of the window's fingers or pens.
    pub fn knows(&self, id: u32) -> bool {
        self.touches.contains_key(&id) || self.pens.contains_key(&id)
    }

    /// The window closed: nothing of it left.
    pub fn clear(&mut self) {
        *self = Pointers::default();
    }
}

#[cfg(windows)]
pub use self::system::{PointerInput, Source};

/// The pointers read from Windows (user32's pointer functions, or a test's [`Source`]) for a window's messages.
#[cfg(windows)]
#[allow(unsafe_code)]
mod system {
    use super::{Clock, Contact, Kind, PenReport, Pointers, Report, precise};
    use crate::app::Event;
    use crate::windows::sys::{self, Hwnd};
    use ::windows::Win32::Foundation::{HANDLE, HWND, RECT};
    use ::windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    use ::windows::Win32::UI::Controls::{
        FEEDBACK_GESTURE_PRESSANDTAP, FEEDBACK_PEN_BARRELVISUALIZATION, FEEDBACK_PEN_DOUBLETAP,
        FEEDBACK_PEN_PRESSANDHOLD, FEEDBACK_PEN_RIGHTTAP, FEEDBACK_PEN_TAP, FEEDBACK_TOUCH_DOUBLETAP,
        FEEDBACK_TOUCH_PRESSANDHOLD, FEEDBACK_TOUCH_RIGHTTAP, FEEDBACK_TOUCH_TAP, FEEDBACK_TYPE,
    };
    use ::windows::Win32::UI::Input::Pointer::{
        GetPointerDeviceRects, GetPointerFrameTouchInfo, GetPointerInfoHistory, GetPointerPenInfo, GetPointerType,
        POINTER_INFO, POINTER_PEN_INFO, POINTER_TOUCH_INFO,
    };
    use ::windows::Win32::UI::WindowsAndMessaging as wm;
    use ::windows::core::BOOL;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::OnceLock;

    /// `SetWindowFeedbackSetting` and `SkipPointerFrameMessages`, Windows 8's, looked up where user32 has them (wine's
    /// hasn't: a call through the import would end the process there).
    type SetFeedback = unsafe extern "system" fn(HWND, FEEDBACK_TYPE, u32, u32, *const core::ffi::c_void) -> BOOL;
    type SkipFrame = unsafe extern "system" fn(u32) -> BOOL;

    fn set_feedback() -> Option<SetFeedback> {
        static F: OnceLock<Option<usize>> = OnceLock::new();
        let f = (*F.get_or_init(|| sys::user32_function(::windows::core::s!("SetWindowFeedbackSetting"))))?;
        // SAFETY: user32's SetWindowFeedbackSetting, whose signature this is.
        Some(unsafe { std::mem::transmute::<usize, SetFeedback>(f) })
    }

    fn skip_frame() -> Option<SkipFrame> {
        static F: OnceLock<Option<usize>> = OnceLock::new();
        let f = (*F.get_or_init(|| sys::user32_function(::windows::core::s!("SkipPointerFrameMessages"))))?;
        // SAFETY: user32's SkipPointerFrameMessages, whose signature this is.
        Some(unsafe { std::mem::transmute::<usize, SkipFrame>(f) })
    }

    /// Where a window's pointer messages are read from: user32 ([`SystemSource`]), or a test's reports (the seam
    /// through which tests send WM_POINTER messages with the reports they make: wine has no pens or touch screens
    /// of its own).
    pub trait Source {
        /// Pointer `id`'s device (`GetPointerType`).
        fn kind(&self, id: u32) -> Kind;
        /// The touch frame pointer `id`'s message came with: every finger of it (`GetPointerFrameTouchInfo`).
        fn touch_frame(&self, id: u32) -> Vec<POINTER_TOUCH_INFO>;
        /// Pen `id`'s report (`GetPointerPenInfo`).
        fn pen(&self, id: u32) -> Option<POINTER_PEN_INFO>;
        /// Pointer `id`'s last `count` reports, newest first (`GetPointerInfoHistory`).
        fn history(&self, id: u32, count: u32) -> Vec<POINTER_INFO>;
        /// The rectangles a device maps between: its own (HIMETRIC) and the display's (`GetPointerDeviceRects`).
        fn device_rects(&self, device: HANDLE) -> Option<([i32; 4], [i32; 4])>;
        /// The rest of pointer `id`'s frame's messages dropped (`SkipPointerFrameMessages`): the frame is handled.
        fn skip_frame(&self, id: u32);
    }

    /// user32's pointer functions.
    pub struct SystemSource;

    fn sides(r: RECT) -> [i32; 4] {
        [r.left, r.top, r.right, r.bottom]
    }

    impl Source for SystemSource {
        fn kind(&self, id: u32) -> Kind {
            let mut t = wm::POINTER_INPUT_TYPE::default();
            // SAFETY: an out parameter of the right type; an id that isn't a pointer's fails.
            match unsafe { GetPointerType(id, &mut t) } {
                Ok(()) => Kind::of(t.0),
                Err(_) => Kind::Other,
            }
        }

        fn touch_frame(&self, id: u32) -> Vec<POINTER_TOUCH_INFO> {
            let mut count = 0u32;
            // SAFETY: the count asked first, then that many entries written into a buffer that long (a frame
            // grown between the calls fails: none read).
            unsafe {
                if GetPointerFrameTouchInfo(id, &mut count, None).is_err() || count == 0 {
                    return Vec::new();
                }
                let mut frame = vec![POINTER_TOUCH_INFO::default(); count as usize];
                if GetPointerFrameTouchInfo(id, &mut count, Some(frame.as_mut_ptr())).is_err() {
                    return Vec::new();
                }
                frame.truncate(count as usize);
                frame
            }
        }

        fn pen(&self, id: u32) -> Option<POINTER_PEN_INFO> {
            let mut info = POINTER_PEN_INFO::default();
            // SAFETY: an out parameter of the right type.
            unsafe { GetPointerPenInfo(id, &mut info) }.ok()?;
            Some(info)
        }

        fn history(&self, id: u32, count: u32) -> Vec<POINTER_INFO> {
            let mut n = count;
            let mut entries = vec![POINTER_INFO::default(); count as usize];
            // SAFETY: a buffer of `count` entries, `n` its length (what was written, after).
            if unsafe { GetPointerInfoHistory(id, &mut n, Some(entries.as_mut_ptr())) }.is_err() {
                return Vec::new();
            }
            entries.truncate(n.min(count) as usize);
            entries
        }

        fn device_rects(&self, device: HANDLE) -> Option<([i32; 4], [i32; 4])> {
            let (mut own, mut display) = (RECT::default(), RECT::default());
            // SAFETY: out parameters; a device handle Windows gave (one gone fails).
            unsafe { GetPointerDeviceRects(device, &mut own, &mut display) }.ok()?;
            Some((sides(own), sides(display)))
        }

        fn skip_frame(&self, id: u32) {
            if let Some(skip) = skip_frame() {
                // SAFETY: an id; one that isn't a pointer's fails.
                let _ = unsafe { skip(id) };
            }
        }
    }

    /// The clocks now.
    fn clock() -> Clock {
        let (mut counter, mut frequency) = (0i64, 0i64);
        // SAFETY: out parameters (both always succeed since Windows XP).
        unsafe {
            let _ = QueryPerformanceCounter(&mut counter);
            let _ = QueryPerformanceFrequency(&mut frequency);
        }
        Clock { now: crate::windows::now(), counter, frequency }
    }

    /// A `POINTER_INFO` as a report in the window whose client area is at `origin` on the screen; a pen's position
    /// to a part of a pixel where its device's rectangles are known.
    fn report_of(
        info: &POINTER_INFO,
        origin: (i32, i32),
        clock: &Clock,
        latest_ms: u32,
        rects: Option<([i32; 4], [i32; 4])>,
    ) -> Report {
        let at = rects
            .and_then(|(own, display)| precise((info.ptHimetricLocation.x, info.ptHimetricLocation.y), own, display))
            .unwrap_or((f64::from(info.ptPixelLocation.x), f64::from(info.ptPixelLocation.y)));
        Report {
            id: info.pointerId,
            frame: info.frameId,
            flags: info.pointerFlags.0,
            x: (at.0 - f64::from(origin.0)) as f32,
            y: (at.1 - f64::from(origin.1)) as f32,
            time: clock.time(info.PerformanceCount, info.dwTime, latest_ms),
        }
    }

    /// A window's pens and fingers: its WM_POINTER messages as the core's events.
    pub struct PointerInput {
        pointers: RefCell<Pointers>,
        source: RefCell<Rc<dyn Source>>,
    }

    impl Default for PointerInput {
        fn default() -> PointerInput {
            PointerInput { pointers: RefCell::default(), source: RefCell::new(Rc::new(SystemSource)) }
        }
    }

    impl PointerInput {
        /// Its reports read from `source` instead of user32 (tests).
        #[cfg(test)]
        pub fn set_source(&self, source: Rc<dyn Source>) {
            *self.source.borrow_mut() = source;
        }

        /// The window `hwnd` made: Windows' own feedback of taps, double taps, presses and holds of fingers and pens
        /// (its circles and its right-click square) off — the app draws its own (ripples, its long press); the
        /// fingers' contact visualization, a setting of the user's, stays.
        pub fn attach(&self, hwnd: Hwnd) {
            let Some(set) = set_feedback() else { return };
            let off = BOOL(0);
            for feedback in [
                FEEDBACK_TOUCH_TAP,
                FEEDBACK_TOUCH_DOUBLETAP,
                FEEDBACK_TOUCH_PRESSANDHOLD,
                FEEDBACK_TOUCH_RIGHTTAP,
                FEEDBACK_PEN_TAP,
                FEEDBACK_PEN_DOUBLETAP,
                FEEDBACK_PEN_PRESSANDHOLD,
                FEEDBACK_PEN_RIGHTTAP,
                FEEDBACK_PEN_BARRELVISUALIZATION,
                FEEDBACK_GESTURE_PRESSANDTAP,
            ] {
                // SAFETY: a window of this thread's, a BOOL's worth of configuration read during the call.
                let _ = unsafe {
                    set(
                        HWND(hwnd.value() as *mut core::ffi::c_void),
                        feedback,
                        0,
                        size_of::<BOOL>() as u32,
                        &off as *const BOOL as *const core::ffi::c_void,
                    )
                };
            }
        }

        /// A WM_POINTER message of window `hwnd` (`w` its wParam: the pointer's id in its low word): its fingers'
        /// and pens' events, or none where it is Windows' to handle (the mouse's, a touchpad's — DirectManipulation
        /// takes those through DM_POINTERHITTEST —, pointers it can't tell).
        pub fn message(&self, hwnd: Hwnd, message: u32, w: usize) -> Option<Vec<Event>> {
            let id = (w & 0xffff) as u32;
            if message == wm::WM_POINTERCAPTURECHANGED {
                let mut pointers = self.pointers.borrow_mut();
                return pointers.knows(id).then(|| pointers.capture_lost(id));
            }
            let source = self.source.borrow().clone();
            match source.kind(id) {
                Kind::Touch => match message {
                    wm::WM_POINTERDOWN | wm::WM_POINTERUPDATE | wm::WM_POINTERUP => self.touch(hwnd, &*source, id),
                    // (A finger coming over the window and leaving it: its down and up tell all.)
                    wm::WM_POINTERENTER | wm::WM_POINTERLEAVE => Some(Vec::new()),
                    _ => None,
                },
                Kind::Pen => match message {
                    wm::WM_POINTERDOWN | wm::WM_POINTERUPDATE | wm::WM_POINTERUP | wm::WM_POINTERENTER => {
                        self.pen(hwnd, &*source, id)
                    }
                    wm::WM_POINTERLEAVE => Some(self.pointers.borrow_mut().pen_left(id)),
                    _ => None,
                },
                Kind::Mouse | Kind::Touchpad | Kind::Other => None,
            }
        }

        /// A finger's message: its frame's fingers (those of this window), each one's history, as events; the
        /// frame's other messages dropped. None where the frame can't be read (Windows' own handling then).
        fn touch(&self, hwnd: Hwnd, source: &dyn Source, id: u32) -> Option<Vec<Event>> {
            let frame = source.touch_frame(id);
            let target = HWND(hwnd.value() as *mut core::ffi::c_void);
            let frame: Vec<&POINTER_TOUCH_INFO> = frame
                .iter()
                .filter(|t| t.pointerInfo.hwndTarget.is_invalid() || t.pointerInfo.hwndTarget == target)
                .collect();
            if frame.is_empty() {
                return None;
            }
            let (origin, clock) = (hwnd.client_origin(), clock());
            let latest = frame.iter().map(|t| t.pointerInfo.dwTime).max().unwrap_or_default();
            let contacts: Vec<Contact> = frame
                .iter()
                .map(|t| {
                    let info = &t.pointerInfo;
                    let history = if info.historyCount > 1 {
                        let entries = source.history(info.pointerId, info.historyCount);
                        entries.iter().map(|e| report_of(e, origin, &clock, latest, None)).collect()
                    } else {
                        Vec::new()
                    };
                    Contact { report: report_of(info, origin, &clock, latest, None), history }
                })
                .collect();
            source.skip_frame(id);
            Some(self.pointers.borrow_mut().touch_frame(&contacts))
        }

        /// A pen's message: its report (and history) as events, its position to a part of a pixel.
        fn pen(&self, hwnd: Hwnd, source: &dyn Source, id: u32) -> Option<Vec<Event>> {
            let info = source.pen(id)?;
            let p = &info.pointerInfo;
            let (origin, clock) = (hwnd.client_origin(), clock());
            let rects = source.device_rects(p.sourceDevice);
            let report = PenReport {
                report: report_of(p, origin, &clock, p.dwTime, rects),
                flags: info.penFlags,
                mask: info.penMask,
                pressure: info.pressure,
            };
            let history: Vec<Report> = if p.historyCount > 1 {
                let entries = source.history(id, p.historyCount);
                entries.iter().map(|e| report_of(e, origin, &clock, p.dwTime, rects)).collect()
            } else {
                Vec::new()
            };
            Some(self.pointers.borrow_mut().pen(&report, &history))
        }

        /// Whether a pen is near the window.
        pub fn pen_near(&self) -> bool {
            self.pointers.borrow().pen_near()
        }

        /// The window closed.
        pub fn clear(&self) {
            self.pointers.borrow_mut().clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(id: u32, frame: u32, flags: u32, x: f32, y: f32, time: i64) -> Report {
        Report { id, frame, flags, x, y, time }
    }

    const DOWN: u32 = flag::DOWN | flag::INRANGE | flag::INCONTACT;
    const MOVE: u32 = flag::UPDATE | flag::INRANGE | flag::INCONTACT;
    const UP: u32 = flag::UP | flag::INRANGE;

    #[test]
    fn mouse_messages_made_of_pens_and_fingers_are_told_apart() {
        assert!(made_of_a_pen_or_a_finger(0xFF51_5780u32 as i32 as isize)); // (a finger's)
        assert!(made_of_a_pen_or_a_finger(0xFF51_5701u32 as i32 as isize)); // (a pen's)
        assert!(!made_of_a_pen_or_a_finger(0));
        assert!(!made_of_a_pen_or_a_finger(0x1234_5678));
        assert_eq!([2, 3, 4, 5, 1].map(Kind::of), [Kind::Touch, Kind::Pen, Kind::Mouse, Kind::Touchpad, Kind::Other]);
        // (Windows' ids may be 0: the mouse's is the core's.)
        assert_eq!(report(0, 0, 0, 0.0, 0.0, 0).pointer(), 1);
    }

    #[test]
    fn a_pens_pressure_eraser_and_button() {
        let pen = |flags, pointer_flags, mask, pressure| PenReport {
            report: report(4, 1, pointer_flags, 1.0, 2.0, 3),
            flags,
            mask,
            pressure,
        };
        assert_eq!(pen(0, DOWN, pen::MASK_PRESSURE, 512).pressure(), 0.5);
        assert_eq!(pen(0, DOWN, pen::MASK_PRESSURE, 4000).pressure(), 1.0); // (past its range: full)
        assert_eq!(pen(0, DOWN, 0, 0).pressure(), 1.0); // (none measured: full while it touches)
        assert_eq!(pen(0, flag::INRANGE, pen::MASK_PRESSURE, 300).pressure(), 0.0); // (hovering)
        assert!(pen(pen::INVERTED, flag::INRANGE, 0, 0).eraser());
        assert!(pen(pen::ERASER, DOWN, 0, 0).eraser());
        assert!(!pen(pen::BARREL, DOWN, 0, 0).eraser());
        assert_eq!(pen(pen::BARREL, DOWN, 0, 0).buttons(), 1);
        assert_eq!(pen(0, DOWN, 0, 0).buttons(), 0);
    }

    #[test]
    fn report_times_on_the_platforms_clock() {
        // The performance counter (10 MHz): 2 ms before now.
        let clock = Clock { now: 1_000_000_000, counter: 50_000_000, frequency: 10_000_000 };
        assert_eq!(clock.time(49_980_000, 0, 0), 998_000_000);
        // (One after the counter read: now.)
        assert_eq!(clock.time(50_000_100, 0, 0), 1_000_000_000);
        // No counter value: the milliseconds before the latest report's, across the tick count's wrap too.
        assert_eq!(clock.time(0, 95, 100), 995_000_000);
        assert_eq!(clock.time(0, u32::MAX, 4), 995_000_000);
        let no_counter = Clock { counter: 0, frequency: 0, ..clock };
        assert_eq!(no_counter.time(49_980_000, 100, 100), 1_000_000_000);
    }

    #[test]
    fn a_pens_position_to_a_part_of_a_pixel() {
        // A tablet of 20000 × 10000 HIMETRIC mapped onto a 1920 × 1080 display right of a 1280-wide one.
        let device = [0, 0, 20_000, 10_000];
        let display = [1280, 0, 3200, 1080];
        assert_eq!(precise((10_000, 5_000), device, display), Some((2240.0, 540.0)));
        let (x, y) = precise((10_001, 2_501), device, display).unwrap();
        assert!((x - 2240.096).abs() < 1e-9 && (y - 270.108).abs() < 1e-9, "{x} {y}");
        assert_eq!(precise((0, 0), [0, 0, 0, 10], display), None);
    }

    #[test]
    fn history_oldest_first_without_the_latest() {
        let newest_first =
            [report(1, 3, MOVE, 30.0, 3.0, 30), report(1, 2, MOVE, 20.0, 2.0, 20), report(1, 1, MOVE, 10.0, 1.0, 10)];
        assert_eq!(
            history(2, &newest_first),
            Some(Event::History { pointer: 2, samples: vec![(10.0, 1.0, 10), (20.0, 2.0, 20)] })
        );
        assert_eq!(history(2, &newest_first[..1]), None);
        assert_eq!(history(2, &[]), None);
    }

    fn contact(id: u32, frame: u32, flags: u32, x: f32, y: f32, time: i64) -> Contact {
        Contact { report: report(id, frame, flags, x, y, time), history: Vec::new() }
    }

    #[test]
    fn touch_frames_become_downs_one_move_of_all_and_ups() {
        let mut p = Pointers::default();
        // Two fingers put down in one frame.
        let events = p.touch_frame(&[contact(7, 1, DOWN, 10.0, 20.0, 5), contact(9, 1, DOWN, 30.0, 40.0, 5)]);
        assert_eq!(
            events,
            [
                Event::TouchDown { pointer: 8, x: 10.0, y: 20.0, time: 5 },
                Event::TouchDown { pointer: 10, x: 30.0, y: 40.0, time: 5 },
            ]
        );
        // The same frame from the other finger's message: nothing more.
        assert_eq!(p.touch_frame(&[contact(9, 1, DOWN, 30.0, 40.0, 5), contact(7, 1, DOWN, 10.0, 20.0, 5)]), []);
        // Both moved: one report, the first's history before it.
        let mut first = contact(7, 2, MOVE, 12.0, 22.0, 9);
        first.history = vec![first.report, report(7, 0, MOVE, 11.0, 21.0, 7)];
        let events = p.touch_frame(&[first, contact(9, 2, MOVE, 31.0, 40.0, 9)]);
        assert_eq!(
            events,
            [
                Event::History { pointer: 8, samples: vec![(11.0, 21.0, 7)] },
                Event::TouchMoves { moves: vec![(8, 12.0, 22.0), (10, 31.0, 40.0)], time: 9 },
            ]
        );
        // A frame where nothing moved: nothing.
        assert_eq!(p.touch_frame(&[contact(7, 3, MOVE, 12.0, 22.0, 10), contact(9, 3, MOVE, 31.0, 40.0, 10)]), []);
        // One lifted while the other moves, a third put down: the move, the down, the up.
        let events = p.touch_frame(&[
            contact(7, 4, UP, 12.0, 23.0, 11),
            contact(9, 4, MOVE, 32.0, 41.0, 11),
            contact(2, 4, DOWN, 50.0, 60.0, 11),
        ]);
        assert_eq!(
            events,
            [
                Event::TouchMoves { moves: vec![(10, 32.0, 41.0)], time: 11 },
                Event::TouchDown { pointer: 3, x: 50.0, y: 60.0, time: 11 },
                Event::TouchUp { pointer: 8, x: 12.0, y: 23.0, time: 11 },
            ]
        );
        // A finger only hovering (no contact): not the app's.
        assert_eq!(p.touch_frame(&[contact(5, 5, flag::INRANGE | flag::UPDATE, 1.0, 1.0, 12)]), []);
    }

    #[test]
    fn a_cancelled_frame_or_a_lost_capture_calls_the_fingers_off() {
        let mut p = Pointers::default();
        p.touch_frame(&[contact(7, 1, DOWN, 10.0, 20.0, 5)]);
        assert_eq!(p.touch_frame(&[contact(7, 2, MOVE | flag::CANCELED, 11.0, 20.0, 6)]), [Event::TouchCancel]);
        // (Its up after: nothing, the finger is no longer the app's.)
        assert_eq!(p.touch_frame(&[contact(7, 3, UP | flag::CANCELED, 11.0, 20.0, 7)]), []);
        p.touch_frame(&[contact(3, 4, DOWN, 1.0, 2.0, 8), contact(4, 4, DOWN, 3.0, 4.0, 8)]);
        assert!(p.knows(3));
        assert_eq!(p.capture_lost(3), [Event::TouchCancel]);
        assert_eq!(p.capture_lost(4), []);
        assert!(!p.knows(4));
    }

    fn pen_report(flags: u32, pointer_flags: u32, x: f32, pressure: u32, time: i64) -> PenReport {
        PenReport { report: report(2, 0, pointer_flags, x, 5.0, time), flags, mask: pen::MASK_PRESSURE, pressure }
    }

    fn stylus(eraser: bool, x: f32, down: bool, pressure: f32, buttons: u32, time: i64) -> Event {
        Event::Stylus { pointer: 3, eraser, x, y: 5.0, down, pressure, buttons, time }
    }

    #[test]
    fn a_pen_hovers_touches_with_pressure_and_leaves() {
        let mut p = Pointers::default();
        let hover = flag::INRANGE | flag::UPDATE;
        assert_eq!(p.pen(&pen_report(0, hover, 1.0, 0, 1), &[]), [stylus(false, 1.0, false, 0.0, 0, 1)]);
        assert!(p.pen_near());
        // The same again: nothing new.
        assert_eq!(p.pen(&pen_report(0, hover, 1.0, 0, 2), &[]), []);
        // Down with its pressure, its history before the report.
        let history = [report(2, 0, DOWN, 3.0, 5.0, 4), report(2, 0, hover, 2.0, 5.0, 3)];
        assert_eq!(
            p.pen(&pen_report(0, DOWN, 3.0, 256, 4), &history),
            [Event::History { pointer: 3, samples: vec![(2.0, 5.0, 3)] }, stylus(false, 3.0, true, 0.25, 0, 4)]
        );
        // Its barrel button, harder.
        assert_eq!(p.pen(&pen_report(pen::BARREL, MOVE, 3.0, 1024, 5), &[]), [stylus(false, 3.0, true, 1.0, 1, 5)]);
        // Lifted, still near; out of range: gone; WM_POINTERLEAVE after: nothing more.
        assert_eq!(p.pen(&pen_report(0, UP, 3.0, 0, 6), &[]), [stylus(false, 3.0, false, 0.0, 0, 6)]);
        assert_eq!(p.pen(&pen_report(0, flag::UPDATE, 3.0, 0, 7), &[]), [Event::StylusLeft { pointer: 3 }]);
        assert_eq!(p.pen_left(2), []);
        assert!(!p.pen_near());
        // Its other end, hovering then erasing; leaving the window.
        assert_eq!(p.pen(&pen_report(pen::INVERTED, hover, 8.0, 0, 8), &[]), [stylus(true, 8.0, false, 0.0, 0, 8)]);
        assert_eq!(
            p.pen(&pen_report(pen::INVERTED | pen::ERASER, DOWN, 8.0, 512, 9), &[]),
            [stylus(true, 8.0, true, 0.5, 0, 9)]
        );
        // Its capture lost while it touches: called off, gone.
        assert_eq!(p.capture_lost(2), [Event::TouchCancel, Event::StylusLeft { pointer: 3 }]);
        // A cancelled report while it hovers: gone only.
        p.pen(&pen_report(0, hover, 1.0, 0, 10), &[]);
        assert_eq!(p.pen(&pen_report(0, hover | flag::CANCELED, 1.0, 0, 11), &[]), [Event::StylusLeft { pointer: 3 }]);
        p.pen(&pen_report(0, hover, 1.0, 0, 12), &[]);
        assert_eq!(p.pen_left(2), [Event::StylusLeft { pointer: 3 }]);
    }
}
