//! KompozR's platform on Android (C++'s platform/android): the app a library (`lib<app>.so`) that the app's
//! activity loads — KompozActivity, a Kotlin class of this crate's `android/` Gradle module, its view a SurfaceView
//! (KompozView) — and that registers its natives as it loads ([`android_app!`](crate::android_app): `JNI_OnLoad`).
//! Java's main thread is the app's UI thread: the activity's lifecycle and configuration, the view's surface and
//! input come to the app core through JNI calls on it (`natives`), its frames on the display's beat from the
//! NDK's choreographer, work from other threads and a coroutine's delay through the UI thread's looper (an eventfd
//! and a timerfd: `unix`'s); there is no loop of KompozR's own. The view's surface is drawn into with Vulkan
//! (`VK_KHR_android_surface`; another window after the background: the device's surface made again). The app's
//! state goes into the activity's saved state (a Bundle; a large one into a file: `saved_state`) and comes back
//! after a process death; the density, the text size, the refresh rate, the night mode, the languages and the time
//! zone come from the activity's configuration (`configuration`: the activity is kept across every configuration
//! change, the density's too); the APK's assets are read through the NDK's asset manager; the log goes to logcat
//! (tag "kompoz"), panics too (each caught before it reaches Java: the activity finishes).
//!
//! Options (`KOMPOZ_SCRIPT`, `KOMPOZ_PROFILE`, …): Android gives an app no environment, so each is a file of its
//! name in the app's files (`kompoz-script`), as C++'s:
//!
//! ```text
//! adb shell run-as dev.kompozr.showcase sh -c "'echo \"wait 60;shot shot.png;quit\" > files/kompoz-script'"
//! ```
//!
//! Our own JNI and NDK bindings (`jni`, `ndk`: no crates; text crosses as UTF-8 `byte[]`). What comes in later
//! packages, each with a Kotlin class of its own beside KompozActivity and KompozView: the input methods, stylus
//! and the full input (KompozIme), accessibility (KompozAccessibility: a node provider over virtual views, from the
//! shared semantics snapshot), autofill (KompozAutofill), content capture and translation (KompozContentCapture),
//! scroll capture (KompozScrollCapture) — the last four through `delegate` —, insets (KompozInsets), back
//! (KompozBack) and the system's services (KompozServices).
//!
//! The input's decoding, the configuration, the saved state's guard, and what accessibility, content capture,
//! autofill and scroll capture are told and do (`accessibility`, `capture`, `autofill`, `semantics`) are plain Rust,
//! built and tested on every host; the rest is Android's alone.

#![cfg_attr(not(target_os = "android"), allow(dead_code))]

pub mod accessibility;
pub mod autofill;
pub mod capture;
mod configuration;
mod devices;
pub mod ime;
pub mod input;
mod saved_state;
pub mod semantics;

#[cfg(target_os = "android")]
mod app;
#[cfg(target_os = "android")]
mod delegate;
#[cfg(target_os = "android")]
pub mod jni;
#[cfg(target_os = "android")]
mod natives;
#[cfg(target_os = "android")]
mod ndk;
#[cfg(target_os = "android")]
mod platform;
#[cfg(target_os = "android")]
mod text_input;
#[cfg(target_os = "android")]
mod vulkan;

#[cfg(target_os = "android")]
pub use natives::on_load;
#[cfg(target_os = "android")]
pub use platform::AndroidPlatform;

use crate::app::{App, AppConfig, AppContent};
use kompoz_runtime::Content;
use std::rc::Rc;

/// The app as Android runs it: what [`android_app!`](crate::android_app) makes each time the activity is created.
pub struct AndroidApp {
    pub config: AppConfig,
    pub content: AppContent,
}

impl AndroidApp {
    /// `app` (its title names its cache's directory; its size is the system's) showing `content`.
    pub fn new(app: App, content: impl Content + 'static) -> AndroidApp {
        AndroidApp { config: app.config().clone(), content: AppContent::Window(Rc::new(content)) }
    }
}

/// A line into logcat (tag "kompoz", info).
#[cfg(target_os = "android")]
pub(crate) fn log_line(message: &str) {
    ndk::log_write(ndk::ANDROID_LOG_INFO, c"kompoz", message);
}

/// An error into logcat (tag "kompoz", error).
#[cfg(target_os = "android")]
pub(crate) fn log_error(message: &str) {
    ndk::log_write(ndk::ANDROID_LOG_ERROR, c"kompoz", message);
}

/// The app's entry on Android, as `fn main` is elsewhere: `JNI_OnLoad` for the app's library (a `cdylib`), which
/// registers KompozR's natives and makes `$app` showing `$content` each time the activity is created (both
/// expressions evaluated then). Nothing on other targets, so the same crate builds its desktop program beside it.
///
/// ```text
/// kompoz::android_app!(App::new("Kompoz Showcase"), showcase::ShowcaseApp);
/// ```
///
/// The library's name is the activity's to load: the manifest's `dev.kompozr.library` meta-data (KompozActivity's
/// `library()`), "main" without it.
#[macro_export]
macro_rules! android_app {
    ($app:expr, $content:expr $(,)?) => {
        #[cfg(target_os = "android")]
        #[unsafe(no_mangle)]
        pub extern "system" fn JNI_OnLoad(vm: $crate::android::jni::Vm, _reserved: *mut ::std::ffi::c_void) -> i32 {
            $crate::android::on_load(vm, || $crate::android::AndroidApp::new($app, $content))
        }
    };
}
