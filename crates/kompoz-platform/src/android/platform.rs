//! The app core's platform on Android (C++'s `AndroidPlatform : Platform`): the view's window to draw into with
//! Vulkan, its size, the activity's configuration as the metrics, the options from the app's files, logcat, the
//! monotonic clock (System.nanoTime's, MotionEvent's and the choreographer's), the host's waker (the looper's
//! eventfd), the state saved before a process death given back to the host as it starts.
//!
//! What it keeps is [`Shared`] with the natives (`super::app`), which change it as the activity tells them; the core
//! reads it through the platform. All on the UI thread.

use super::configuration::{Configuration, option_file, option_value};
use super::vulkan;
use crate::app::{Metrics, Platform, WindowSpec};
use crate::unix::waker::Waker;
use kompoz_gpu::device::{Backend, Surface};
use kompoz_ui::KompozHost;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::PlatformCallbacks;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// What the platform and the natives share.
pub(super) struct Shared {
    /// The view's native window now (its address; 0: none, the app in the background).
    pub window: Rc<Cell<usize>>,
    /// Its size in pixels, as the view's surface last said.
    pub size: Cell<(i32, i32)>,
    pub configuration: RefCell<Configuration>,
    /// The app's files (`Context.getFilesDir`) and external files (`getExternalFilesDir(null)`; empty: none).
    pub files_dir: PathBuf,
    pub external_files_dir: Option<PathBuf>,
    /// The state saved before the process ended, for the host as it starts.
    pub restored: RefCell<Option<Vec<u8>>>,
    pub waker: Waker,
}

/// The platform (one: Android gives an app one window, so a `Window` gets none).
pub struct AndroidPlatform {
    pub(super) shared: Rc<Shared>,
}

/// What the host asks of the platform (A0: the waker; the input methods, the clipboard, insets and the rest come
/// with KompozIme, KompozServices and the others).
struct AndroidCallbacks {
    waker: Arc<dyn Fn() + Send + Sync>,
}

impl PlatformCallbacks for AndroidCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.waker.clone())
    }
}

impl AndroidPlatform {
    /// An option's own value (none: not set), as its file in the app's files or external files has it, else the
    /// process's environment (`adb shell`'s can't reach an app, but a wrapper script's does: `wrap.sh`).
    fn read_option(&self, name: &str) -> Option<String> {
        let file = option_file(name);
        let dirs = std::iter::once(&self.shared.files_dir).chain(self.shared.external_files_dir.as_ref());
        for dir in dirs {
            if let Ok(text) = std::fs::read_to_string(dir.join(&file)) {
                return Some(option_value(&text));
            }
        }
        kompoz_runtime::environment::var(name)
    }

    fn number(&self, name: &str) -> Option<f32> {
        self.read_option(name).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite() && *v > 0.0)
    }
}

impl Platform for AndroidPlatform {
    fn option(&self, name: &str) -> Option<String> {
        self.read_option(name)
    }

    fn log(&self, message: &str) {
        super::log_line(message);
    }

    fn now(&self) -> i64 {
        crate::unix::sys::monotonic_now()
    }

    /// Vulkan alone: Android 10 (the app's minSdk) has Vulkan 1.1 on every 64-bit device it launched on.
    fn backends(&self) -> Vec<Backend> {
        vec![Backend::Vulkan]
    }

    fn open_window(&mut self, _: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("no {} on Android", backend.name()));
        }
        let shared = self.shared.clone();
        Ok(Surface {
            vulkan: Some(vulkan::surface(self.shared.window.clone())),
            drawable_size: Some(Box::new(move || shared.size.get())),
            ..Surface::default()
        })
    }

    /// (The window is the view's: it stays.)
    fn close_window(&mut self) {}

    fn show_error(&mut self, message: &str) {
        super::log_error(message);
    }

    fn metrics(&self) -> Metrics {
        let (w, h) = self.shared.size.get();
        let scale = self.number("KOMPOZ_SCALE");
        let font_scale = self.number("KOMPOZ_FONT_SCALE");
        self.shared.configuration.borrow().metrics(w, h, scale, font_scale)
    }

    fn pixel_size(&self) -> (i32, i32) {
        let (w, h) = self.shared.size.get();
        (w.max(1), h.max(1))
    }

    /// (The system sizes the activity's window.)
    fn set_window_size(&mut self, _: i32, _: i32) {}

    /// The touchscreen and the keyboard as the configuration says (the input devices themselves, a mouse's
    /// precision, the dock: KompozIme's media info, A1).
    fn media(&mut self, _probe: bool) -> MediaInfo {
        let c = self.shared.configuration.borrow();
        MediaInfo {
            pointer_precision: if c.touchscreen { PointerPrecision::Coarse } else { PointerPrecision::None },
            physical_keyboard: c.physical_keyboard,
            ..MediaInfo::default()
        }
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(AndroidCallbacks { waker: self.shared.waker.as_fn() })
    }

    /// The state saved before the process ended, back in the host (Compose's saved instance state restored).
    fn started(&mut self, host: &KompozHost) {
        if let Some(state) = self.shared.restored.borrow_mut().take() {
            host.restore_state(&state);
        }
    }

    fn composed(&mut self, _: &KompozHost, _: bool, _: i64) {}
}
