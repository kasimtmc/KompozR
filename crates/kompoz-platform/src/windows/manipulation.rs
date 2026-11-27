//! A precision touchpad's pinches and pans on Windows, through DirectManipulation (Linux's touchpad gestures:
//! pointer-gestures' pinches; C++ had only the Ctrl+wheel Windows makes of a pinch, and the wheel of a pan): a
//! viewport per window (`IDirectManipulationManager::CreateViewport`, a fake content area its transform moves in, as
//! Chromium's), the touchpad's contacts given to it as Windows hit-tests them on the window (DM_POINTERHITTEST →
//! `SetContact`), its content's transform heard (an `IDirectManipulationViewportEventHandler` of ours) and turned into
//! the core's gestures: a scale changing as a pinch ([`Event::PinchBegin`], [`Event::Pinch`] by the change since the
//! last, [`Event::PinchEnd`]), a translation as a pan ([`Event::PanBegin`], [`Event::Pan`], [`Event::PanEnd`]: Compose's
//! trackpad pan, whose scrollables fling at its end from its velocity — so DirectManipulation's own inertia isn't
//! asked for: the momentum is the app's, as on Android), both about the mouse's position (a touchpad's gestures
//! don't move it). A gesture that began as a pan may turn into a pinch (DirectManipulation tells a slow pinch from a
//! pan late), never back. Each gesture over, the content is put back where it began (its transform none), so each
//! starts from nothing.
//!
//! The viewport is updated manually (`DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE`): the update manager runs on
//! the loop's frames, at the display's pace, while a gesture is under way (`Update` in the window's `composed`, frames
//! asked for meanwhile): nothing blocks the message pump, and the transforms come on the UI thread. Mouse wheels stay
//! wheels, Ctrl+wheel zoom; a touch screen's fingers stay the app's own (`pointer`). `KOMPOZ_DIRECT_MANIPULATION=0`
//! turns it off (the touchpad's wheel messages then, as C++'s).
//!
//! The turning of the content's transform into gestures is pure (tested on every host); the COM part is Windows'.

use crate::app::Event;

/// `DIRECTMANIPULATION_STATUS` (directmanipulation.h).
pub mod status {
    pub const ENABLED: i32 = 1;
    pub const RUNNING: i32 = 3;
    pub const INERTIA: i32 = 4;
    pub const READY: i32 = 5;
}

/// The size of the fake viewport the content moves in (pixels: its transform's units).
pub const VIEWPORT_SIZE: i32 = 1000;

/// How long frames are asked for after a contact was given to the viewport while it says nothing (ns): a touch of
/// the touchpad that never became a gesture.
const CONTACT_FRAMES: i64 = 1_000_000_000;

/// A gesture's step as the viewport's transform tells it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Gesture {
    PinchBegin,
    /// Scaled by `scale` since the last.
    Pinch {
        scale: f32,
    },
    PinchEnd,
    PanBegin,
    /// The content moved by `dx`, `dy` pixels since the last (as the fingers move it, the user's scroll direction
    /// applied by Windows).
    Pan {
        dx: f32,
        dy: f32,
    },
    PanEnd,
}

/// What a gesture is so far.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Phase {
    #[default]
    None,
    Pan,
    Pinch,
}

/// A viewport's gesture as its status and its content's transform tell it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tracker {
    /// The transform last heard: its scale and translation.
    scale: f32,
    x: f32,
    y: f32,
    phase: Phase,
    status: i32,
    /// When a contact was given to the viewport (the platform's clock), until it says how it goes.
    contact: Option<i64>,
}

impl Default for Tracker {
    fn default() -> Tracker {
        Tracker { scale: 1.0, x: 0.0, y: 0.0, phase: Phase::None, status: status::ENABLED, contact: None }
    }
}

/// Floats equal but for rounding.
fn about(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5 * a.abs().max(b.abs()).max(1.0)
}

impl Tracker {
    /// A contact given to the viewport at `now`: frames are wanted until it goes one way or the other.
    pub fn contact(&mut self, now: i64) {
        self.contact = Some(now);
    }

    /// Whether the update manager is to run on the coming frames: a gesture under way, or a contact given lately.
    pub fn active(&self, now: i64) -> bool {
        matches!(self.status, status::RUNNING | status::INERTIA)
            || self.contact.is_some_and(|t| now.saturating_sub(t) < CONTACT_FRAMES)
    }

    /// The viewport's status changed: what ends with it, and whether the content is to be put back where it began
    /// (the gesture over, ready for the next: its transform none — not again if it is there already).
    pub fn status(&mut self, current: i32) -> (Vec<Gesture>, bool) {
        self.status = current;
        if matches!(current, status::RUNNING | status::INERTIA | status::READY) {
            self.contact = None; // (it went one way or the other)
        }
        if current == status::RUNNING {
            return (Vec::new(), false);
        }
        // (Inertia isn't asked for, but where it comes, the gesture ends with the fingers lifting all the same.)
        let ended = match self.phase {
            Phase::None => Vec::new(),
            Phase::Pan => vec![Gesture::PanEnd],
            Phase::Pinch => vec![Gesture::PinchEnd],
        };
        self.phase = Phase::None;
        let reset = matches!(current, status::READY | status::ENABLED)
            && !(self.scale == 1.0 && self.x == 0.0 && self.y == 0.0);
        if reset {
            (self.scale, self.x, self.y) = (1.0, 0.0, 0.0);
        }
        (ended, reset)
    }

    /// The content's transform (`GetContentTransform`: scale, 0, 0, scale, x, y) as the gesture's steps: a scale
    /// changed — a pinch from then on —, else a move a pan. A scale of 0 (which DirectManipulation may give) is
    /// nothing; the transform the reset puts back (none) is nothing either.
    pub fn content(&mut self, m: &[f32; 6]) -> Vec<Gesture> {
        let (scale, x, y) = (m[0], m[4], m[5]);
        if scale == 0.0 || !scale.is_finite() {
            return Vec::new();
        }
        let mut steps = Vec::new();
        if self.phase != Phase::Pinch && !about(scale, 1.0) {
            if self.phase == Phase::Pan {
                steps.push(Gesture::PanEnd);
            }
            self.phase = Phase::Pinch;
            steps.push(Gesture::PinchBegin);
        }
        match self.phase {
            Phase::Pinch => {
                if !about(scale, self.scale) {
                    steps.push(Gesture::Pinch { scale: scale / self.scale });
                }
            }
            _ => {
                let (dx, dy) = (x - self.x, y - self.y);
                if dx != 0.0 || dy != 0.0 {
                    if self.phase == Phase::None {
                        self.phase = Phase::Pan;
                        steps.push(Gesture::PanBegin);
                    }
                    steps.push(Gesture::Pan { dx, dy });
                }
            }
        }
        (self.scale, self.x, self.y) = (scale, x, y);
        steps
    }
}

/// A gesture's step as the core's event with the mouse at (`x`, `y`) in the window (its points: pixels): a pan's
/// move as the scroll the fingers make (Compose's `panOffset`: the content's move, opposite).
pub fn event_of(gesture: Gesture, x: f32, y: f32) -> Event {
    match gesture {
        Gesture::PinchBegin => Event::PinchBegin { x, y },
        Gesture::Pinch { scale } => Event::Pinch { x, y, scale, rotation: 0.0 },
        Gesture::PinchEnd => Event::PinchEnd { x, y },
        Gesture::PanBegin => Event::PanBegin { x, y },
        Gesture::Pan { dx, dy } => Event::Pan { x, y, dx: -dx, dy: -dy },
        Gesture::PanEnd => Event::PanEnd { x, y },
    }
}

#[cfg(windows)]
pub use self::system::{Manipulation, enabled, unavailable};

#[cfg(windows)]
#[allow(unsafe_code)]
mod system {
    use super::{Gesture, Tracker, VIEWPORT_SIZE};
    use crate::windows::sys::Hwnd;
    use ::windows::Win32::Foundation::{HWND, RECT};
    use ::windows::Win32::Graphics::DirectManipulation::{
        DIRECTMANIPULATION_CONFIGURATION_INTERACTION, DIRECTMANIPULATION_CONFIGURATION_RAILS_X,
        DIRECTMANIPULATION_CONFIGURATION_RAILS_Y, DIRECTMANIPULATION_CONFIGURATION_SCALING,
        DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X, DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y,
        DIRECTMANIPULATION_STATUS, DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE, DirectManipulationManager,
        IDirectManipulationContent, IDirectManipulationManager, IDirectManipulationPrimaryContent,
        IDirectManipulationUpdateManager, IDirectManipulationViewport, IDirectManipulationViewportEventHandler,
        IDirectManipulationViewportEventHandler_Impl,
    };
    use ::windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use ::windows::Win32::UI::Input::Pointer::GetPointerType;
    use ::windows::Win32::UI::WindowsAndMessaging::{POINTER_INPUT_TYPE, PT_TOUCHPAD};
    use ::windows_core::{Ref, Result, implement};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// What a viewport's handler hands its gestures' steps to (the window's: its events at the mouse).
    pub type Sink = Box<dyn Fn(Gesture)>;

    /// The viewport's handler: its status and its content's transform into the tracker, the steps to the sink,
    /// the content put back as each gesture ends.
    #[implement(IDirectManipulationViewportEventHandler)]
    struct Handler {
        tracker: Rc<RefCell<Tracker>>,
        sink: Sink,
    }

    impl IDirectManipulationViewportEventHandler_Impl for Handler_Impl {
        fn OnViewportStatusChanged(
            &self,
            viewport: Ref<IDirectManipulationViewport>,
            current: DIRECTMANIPULATION_STATUS,
            _previous: DIRECTMANIPULATION_STATUS,
        ) -> Result<()> {
            let (steps, reset) = self.tracker.borrow_mut().status(current.0);
            for s in steps {
                (self.sink)(s);
            }
            if reset && let Some(viewport) = viewport.as_ref() {
                let size = VIEWPORT_SIZE as f32;
                // SAFETY: the viewport this handler is the handler of, on its thread; its content's update this
                // brings is heard again (the tracker no longer borrowed), and is nothing.
                unsafe { viewport.ZoomToRect(0.0, 0.0, size, size, false)? };
            }
            Ok(())
        }

        fn OnViewportUpdated(&self, _viewport: Ref<IDirectManipulationViewport>) -> Result<()> {
            Ok(())
        }

        fn OnContentUpdated(
            &self,
            _viewport: Ref<IDirectManipulationViewport>,
            content: Ref<IDirectManipulationContent>,
        ) -> Result<()> {
            let Some(content) = content.as_ref() else { return Ok(()) };
            let mut m = [0f32; 6];
            // SAFETY: the content's transform into a buffer of its six floats.
            unsafe { content.GetContentTransform(&mut m)? };
            let steps = self.tracker.borrow_mut().content(&m);
            for s in steps {
                (self.sink)(s);
            }
            Ok(())
        }
    }

    /// A viewport handler over `tracker` handing steps to `sink` (the viewport's; tests call it as
    /// DirectManipulation would).
    pub fn handler(tracker: Rc<RefCell<Tracker>>, sink: Sink) -> IDirectManipulationViewportEventHandler {
        Handler { tracker, sink }.into()
    }

    /// Whether windows take touchpads through DirectManipulation (`KOMPOZ_DIRECT_MANIPULATION=0`: not).
    pub fn enabled() -> bool {
        kompoz_runtime::environment::var("KOMPOZ_DIRECT_MANIPULATION").is_none_or(|v| v.trim() != "0")
    }

    /// DirectManipulation isn't there (`why`): said once.
    pub fn unavailable(why: &str) {
        static SAID: std::sync::Once = std::sync::Once::new();
        SAID.call_once(|| {
            crate::system::log(&format!(
                "kompoz: no DirectManipulation ({why}): touchpads scroll and zoom by their wheels"
            ));
        });
    }

    fn raw(hwnd: Hwnd) -> HWND {
        HWND(hwnd.value() as *mut core::ffi::c_void)
    }

    /// A window's DirectManipulation: its manager, update manager and viewport, its handler's cookie, the tracker.
    pub struct Manipulation {
        hwnd: Hwnd,
        manager: IDirectManipulationManager,
        update: IDirectManipulationUpdateManager,
        viewport: IDirectManipulationViewport,
        cookie: u32,
        tracker: Rc<RefCell<Tracker>>,
    }

    impl Manipulation {
        /// Window `hwnd`'s, its gestures' steps to `sink`; why not where DirectManipulation isn't there (COM not
        /// initialized on the thread, a system without it, wine's partial one failing).
        pub fn new(hwnd: Hwnd, sink: Sink) -> std::result::Result<Manipulation, String> {
            let tracker = Rc::new(RefCell::new(Tracker::default()));
            let fail = |what: &str, e: ::windows_core::Error| format!("{what}: {e}");
            // SAFETY: COM calls on the UI thread (its apartment OLE's), on objects made here; the window this
            // thread's.
            unsafe {
                let manager: IDirectManipulationManager =
                    CoCreateInstance(&DirectManipulationManager, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| fail("no DirectManipulation manager", e))?;
                let update: IDirectManipulationUpdateManager =
                    manager.GetUpdateManager().map_err(|e| fail("no update manager", e))?;
                let viewport: IDirectManipulationViewport =
                    manager.CreateViewport(None, raw(hwnd)).map_err(|e| fail("no viewport", e))?;
                // (Pans along their axis where they begin along one, pinches; no inertia: the app flings.)
                let configuration = DIRECTMANIPULATION_CONFIGURATION_INTERACTION
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y
                    | DIRECTMANIPULATION_CONFIGURATION_RAILS_X
                    | DIRECTMANIPULATION_CONFIGURATION_RAILS_Y
                    | DIRECTMANIPULATION_CONFIGURATION_SCALING;
                viewport.ActivateConfiguration(configuration).map_err(|e| fail("no configuration", e))?;
                viewport
                    .SetViewportOptions(DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE)
                    .map_err(|e| fail("no manual updates", e))?;
                let handler = handler(tracker.clone(), sink);
                let cookie =
                    viewport.AddEventHandler(Some(raw(hwnd)), &handler).map_err(|e| fail("no event handler", e))?;
                let made = Manipulation { hwnd, manager, update, viewport, cookie, tracker };
                let rect = RECT { left: 0, top: 0, right: VIEWPORT_SIZE, bottom: VIEWPORT_SIZE };
                made.viewport.SetViewportRect(&rect).map_err(|e| fail("no viewport rectangle", e))?;
                // (A pinch's scale as far as the fingers take it: each gesture starts again from none.)
                if let Ok(primary) = made.viewport.GetPrimaryContent::<IDirectManipulationPrimaryContent>() {
                    let _ = primary.SetZoomBoundaries(0.01, 100.0);
                }
                made.manager.Activate(raw(hwnd)).map_err(|e| fail("not activated", e))?;
                made.viewport.Enable().map_err(|e| fail("not enabled", e))?;
                made.update.Update(None).map_err(|e| fail("no update", e))?;
                Ok(made)
            }
        }

        /// DM_POINTERHITTEST for pointer `id` at `now`: a touchpad's contact given to the viewport (whether it was:
        /// the message answered then).
        pub fn hit_test(&self, id: u32, now: i64) -> bool {
            let mut t = POINTER_INPUT_TYPE::default();
            // SAFETY: an out parameter; an id that isn't a pointer's fails.
            if unsafe { GetPointerType(id, &mut t) }.is_err() || t != PT_TOUCHPAD {
                return false;
            }
            // SAFETY: the viewport's own call, on its thread.
            if unsafe { self.viewport.SetContact(id) }.is_err() {
                return false;
            }
            self.tracker.borrow_mut().contact(now);
            true
        }

        /// Whether frames are to run the update manager.
        pub fn active(&self, now: i64) -> bool {
            self.tracker.borrow().active(now)
        }

        /// The update manager run (a frame's): the viewport's transforms and statuses heard.
        pub fn update(&self, now: i64) {
            if self.active(now) {
                // SAFETY: the update manager's own call, on its thread (the handler borrows the tracker itself).
                let _ = unsafe { self.update.Update(None) };
            }
        }
    }

    impl Drop for Manipulation {
        /// The handler removed, the viewport let go, the window no longer DirectManipulation's.
        fn drop(&mut self) {
            // SAFETY: the objects' own calls, on their thread; the window still there (closed after).
            unsafe {
                let _ = self.viewport.RemoveEventHandler(self.cookie);
                let _ = self.viewport.Stop();
                let _ = self.manager.Deactivate(raw(self.hwnd));
                let _ = self.viewport.Abandon();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::app::Event;
        use crate::windows::manipulation::{event_of, status};
        use ::windows::Win32::Graphics::DirectManipulation::{
            IDirectManipulationContent_Impl, IDirectManipulationViewport,
        };
        use ::windows_core::{GUID, IUnknown};
        use std::cell::Cell;

        /// A content whose transform a test sets (what DirectManipulation's content tells its handler).
        #[implement(IDirectManipulationContent)]
        struct Content(Rc<Cell<[f32; 6]>>);

        impl IDirectManipulationContent_Impl for Content_Impl {
            fn GetContentRect(&self) -> Result<RECT> {
                Ok(RECT::default())
            }
            fn SetContentRect(&self, _: *const RECT) -> Result<()> {
                Ok(())
            }
            fn GetViewport(&self, _: *const GUID, _: *mut *mut core::ffi::c_void) -> Result<()> {
                Err(::windows::Win32::Foundation::E_NOTIMPL.into())
            }
            fn GetTag(&self, _: *const GUID, _: *mut *mut core::ffi::c_void, _: *mut u32) -> Result<()> {
                Err(::windows::Win32::Foundation::E_NOTIMPL.into())
            }
            fn SetTag(&self, _: Ref<IUnknown>, _: u32) -> Result<()> {
                Ok(())
            }
            fn GetOutputTransform(&self, matrix: *mut f32, count: u32) -> Result<()> {
                self.GetContentTransform(matrix, count)
            }
            fn GetContentTransform(&self, matrix: *mut f32, count: u32) -> Result<()> {
                let m = self.0.get();
                // SAFETY: the caller's buffer of `count` floats.
                unsafe { std::ptr::copy_nonoverlapping(m.as_ptr(), matrix, (count as usize).min(6)) };
                Ok(())
            }
            fn SyncContentTransform(&self, _: *const f32, _: u32) -> Result<()> {
                Ok(())
            }
        }

        /// The handler driven through its interface as DirectManipulation drives it (wine's DirectManipulation makes
        /// no gestures of its own): a pan's transforms, a pinch's, the gestures' ends; the viewport absent, the
        /// content's put back is skipped.
        #[test]
        fn the_handler_turns_transforms_into_the_cores_gestures() {
            let tracker = Rc::new(RefCell::new(Tracker::default()));
            let events = Rc::new(RefCell::new(Vec::new()));
            let e = events.clone();
            let handler = handler(tracker.clone(), Box::new(move |g| e.borrow_mut().push(event_of(g, 5.0, 6.0))));
            let transform = Rc::new(Cell::new([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
            let content: IDirectManipulationContent = Content(transform.clone()).into();
            let set = |m: [f32; 6]| transform.set(m);
            let viewport: Option<IDirectManipulationViewport> = None;
            let status = |s: i32| {
                // SAFETY: the handler's own method, with no viewport (as DirectManipulation may pass).
                unsafe {
                    handler
                        .OnViewportStatusChanged(
                            viewport.as_ref(),
                            DIRECTMANIPULATION_STATUS(s),
                            DIRECTMANIPULATION_STATUS(status::ENABLED),
                        )
                        .unwrap()
                };
            };
            let updated = || {
                // SAFETY: the handler's own method, with the test's content.
                unsafe { handler.OnContentUpdated(viewport.as_ref(), &content).unwrap() };
            };
            status(status::RUNNING);
            assert!(tracker.borrow().active(0));
            set([1.0, 0.0, 0.0, 1.0, 0.0, -10.0]);
            updated();
            set([1.0, 0.0, 0.0, 1.0, 3.0, -25.0]);
            updated();
            status(status::READY);
            assert_eq!(
                *events.borrow(),
                [
                    Event::PanBegin { x: 5.0, y: 6.0 },
                    Event::Pan { x: 5.0, y: 6.0, dx: -0.0, dy: 10.0 },
                    Event::Pan { x: 5.0, y: 6.0, dx: -3.0, dy: 15.0 },
                    Event::PanEnd { x: 5.0, y: 6.0 },
                ]
            );
            assert!(!tracker.borrow().active(0));
            events.borrow_mut().clear();
            // A pinch (its translation the zoom's centring: left out); the reset content's identity: nothing.
            status(status::RUNNING);
            set([2.0, 0.0, 0.0, 2.0, -500.0, -500.0]);
            updated();
            set([3.0, 0.0, 0.0, 3.0, -1000.0, -1000.0]);
            updated();
            status(status::READY);
            set([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            updated();
            assert_eq!(
                *events.borrow(),
                [
                    Event::PinchBegin { x: 5.0, y: 6.0 },
                    Event::Pinch { x: 5.0, y: 6.0, scale: 2.0, rotation: 0.0 },
                    Event::Pinch { x: 5.0, y: 6.0, scale: 1.5, rotation: 0.0 },
                    Event::PinchEnd { x: 5.0, y: 6.0 },
                ]
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(scale: f32, x: f32, y: f32) -> [f32; 6] {
        [scale, 0.0, 0.0, scale, x, y]
    }

    #[test]
    fn a_pan_moves_and_ends_and_the_content_is_put_back() {
        let mut t = Tracker::default();
        assert!(!t.active(0));
        t.contact(100);
        assert!(t.active(100 + CONTACT_FRAMES - 1));
        assert!(!t.active(100 + CONTACT_FRAMES)); // (a touch that never became a gesture)
        assert_eq!(t.status(status::RUNNING), (vec![], false));
        assert!(t.active(i64::MAX));
        assert_eq!(t.content(&transform(1.0, 0.0, -4.0)), [Gesture::PanBegin, Gesture::Pan { dx: 0.0, dy: -4.0 }]);
        assert_eq!(t.content(&transform(1.0, 2.0, -10.0)), [Gesture::Pan { dx: 2.0, dy: -6.0 }]);
        assert_eq!(t.content(&transform(1.0, 2.0, -10.0)), []); // (nothing moved)
        assert_eq!(t.content(&transform(0.0, 9.0, 9.0)), []); // (a scale of 0: nothing)
        // The fingers lift: the pan ends (its fling the app's), the content to be put back.
        assert_eq!(t.status(status::READY), (vec![Gesture::PanEnd], true));
        assert!(!t.active(0));
        // Put back: its transform none, which is nothing; ready again: nothing more to put back.
        assert_eq!(t.content(&transform(1.0, 0.0, 0.0)), []);
        assert_eq!(t.status(status::READY), (vec![], false));
    }

    #[test]
    fn a_pinch_scales_by_its_steps_and_a_pan_may_turn_into_one() {
        let mut t = Tracker::default();
        t.status(status::RUNNING);
        // A slow pinch that began as a pan: the pan ends, the pinch begins.
        assert_eq!(t.content(&transform(1.0, 1.0, 1.0)), [Gesture::PanBegin, Gesture::Pan { dx: 1.0, dy: 1.0 }]);
        assert_eq!(
            t.content(&transform(1.25, -10.0, -10.0)),
            [Gesture::PanEnd, Gesture::PinchBegin, Gesture::Pinch { scale: 1.25 }]
        );
        // Its translations are the zoom's centring: left out; its scale back to 1 is still the pinch.
        assert_eq!(t.content(&transform(1.25, -20.0, -20.0)), []);
        assert_eq!(t.content(&transform(1.0, -5.0, -5.0)), [Gesture::Pinch { scale: 0.8 }]);
        // Inertia (were it asked for) ends it as the fingers lift; nothing to put back yet, then ready: put back.
        assert_eq!(t.status(status::INERTIA), (vec![Gesture::PinchEnd], false));
        assert_eq!(t.status(status::READY), (vec![], true));
    }

    #[test]
    fn gestures_as_the_cores_events() {
        assert_eq!(event_of(Gesture::PinchBegin, 1.0, 2.0), Event::PinchBegin { x: 1.0, y: 2.0 });
        assert_eq!(
            event_of(Gesture::Pinch { scale: 1.5 }, 1.0, 2.0),
            Event::Pinch { x: 1.0, y: 2.0, scale: 1.5, rotation: 0.0 }
        );
        assert_eq!(event_of(Gesture::PinchEnd, 1.0, 2.0), Event::PinchEnd { x: 1.0, y: 2.0 });
        assert_eq!(event_of(Gesture::PanBegin, 1.0, 2.0), Event::PanBegin { x: 1.0, y: 2.0 });
        // The content moved down and left by the fingers: the scroll they make is up and right.
        assert_eq!(
            event_of(Gesture::Pan { dx: -3.0, dy: 4.0 }, 1.0, 2.0),
            Event::Pan { x: 1.0, y: 2.0, dx: 3.0, dy: -4.0 }
        );
        assert_eq!(event_of(Gesture::PanEnd, 1.0, 2.0), Event::PanEnd { x: 1.0, y: 2.0 });
    }
}
