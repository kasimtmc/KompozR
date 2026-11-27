//! A window's frames on its display's beat (what Wayland's frame callbacks are on Linux): a display link that ticks
//! as the display the window is on refreshes, running only while the app wants frames in that window. macOS 14's
//! own (`-[NSView displayLinkWithTarget:selector:]`: a CADisplayLink following the view's display, ticking on the
//! main run loop, in every mode — while AppKit tracks a resize too); before it Core Video's (a CVDisplayLink of the
//! displays, ticking on a thread of its own). A tick marks the window ready for its next frame and wakes the loop.

#![allow(unsafe_code)]
#![allow(deprecated)] // (Core Video's display link: the one before macOS 14, deprecated since 15)

use super::app::wake_loop;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2_app_kit::NSView;
use objc2_core_foundation::CFRetained;
use objc2_core_video::{CVDisplayLink, CVOptionFlags, CVReturn, CVTimeStamp, kCVReturnSuccess};
use objc2_foundation::{NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::CADisplayLink;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The display ticked since the window's last frame (set on the link's thread, taken on the main one).
#[derive(Debug, Default)]
pub struct Tick {
    ready: AtomicBool,
}

impl Tick {
    /// The display refreshed: the window may draw; the loop woken.
    pub fn ticked(&self) {
        self.ready.store(true, Ordering::Release);
        wake_loop();
    }

    /// Whether it ticked since last taken.
    pub fn take(&self) -> bool {
        self.ready.swap(false, Ordering::AcqRel)
    }
}

/// A window's display link.
pub enum DisplayLink {
    /// The view's (macOS 14): it calls the view's `displayLinkTick:`, which ticks the window's [`Tick`].
    View(Retained<CADisplayLink>),
    /// Core Video's: its callback ticks `tick` (a reference of the Arc's, given back when the link goes).
    Video { link: CFRetained<CVDisplayLink>, tick: *const Tick },
}

/// Core Video's callback, on its thread: the window's tick.
unsafe extern "C-unwind" fn video_tick(
    _link: NonNull<CVDisplayLink>,
    _now: NonNull<CVTimeStamp>,
    _output: NonNull<CVTimeStamp>,
    _flags: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    context: *mut c_void,
) -> CVReturn {
    // SAFETY: `context` is the Arc<Tick> reference `DisplayLink::new` gave the link, given back only after the link
    // is stopped (no callback runs after `CVDisplayLinkStop` returns); Tick is Sync.
    let tick = unsafe { &*(context as *const Tick) };
    tick.ticked();
    kCVReturnSuccess
}

impl DisplayLink {
    /// A display link for `view` (paused), ticking `tick`; none where neither can be made. The view's
    /// `displayLinkTick:` must tick `tick`.
    pub fn new(view: &NSView, tick: &Arc<Tick>) -> Option<DisplayLink> {
        if view.respondsToSelector(sel!(displayLinkWithTarget:selector:)) {
            let target: &AnyObject = view;
            // SAFETY: the view implements `displayLinkTick:` taking the link (`KompozView`); the link retains its
            // target until invalidated (`Drop`).
            let link = unsafe { view.displayLinkWithTarget_selector(target, sel!(displayLinkTick:)) };
            link.setPaused(true);
            // SAFETY: the main run loop, and the common modes (a static string of Foundation's).
            unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
            return Some(DisplayLink::View(link));
        }
        let mut raw: *mut CVDisplayLink = std::ptr::null_mut();
        // SAFETY: `raw` is a valid place for the link Core Video makes (+1 retained, owned here).
        let made = unsafe { CVDisplayLink::create_with_active_cg_displays(NonNull::from(&mut raw)) };
        let link = NonNull::new(raw).filter(|_| made == kCVReturnSuccess)?;
        // SAFETY: the link was made by a Create function: the reference is ours.
        let link = unsafe { CFRetained::from_raw(link) };
        let context = Arc::into_raw(tick.clone());
        // SAFETY: the callback reads `context` as a Tick, which it is until `Drop` stops the link and gives the
        // reference back.
        let set = unsafe { link.set_output_callback(Some(video_tick), context as *mut c_void) };
        if set != kCVReturnSuccess {
            // SAFETY: the reference given to the link, which never used it (its callback wasn't set).
            drop(unsafe { Arc::from_raw(context) });
            return None;
        }
        Some(DisplayLink::Video { link, tick: context })
    }

    /// Ticking (frames wanted) or not.
    pub fn set_running(&self, running: bool) {
        match self {
            DisplayLink::View(link) => {
                if link.isPaused() == running {
                    link.setPaused(!running);
                }
            }
            DisplayLink::Video { link, .. } => {
                if link.is_running() != running {
                    if running {
                        link.start();
                    } else {
                        link.stop();
                    }
                }
            }
        }
    }
}

impl Drop for DisplayLink {
    fn drop(&mut self) {
        match self {
            DisplayLink::View(link) => link.invalidate(),
            DisplayLink::Video { link, tick } => {
                link.stop();
                // SAFETY: the reference given to the link in `new`; the link is stopped (no callback runs any more).
                drop(unsafe { Arc::from_raw(*tick) });
            }
        }
    }
}
