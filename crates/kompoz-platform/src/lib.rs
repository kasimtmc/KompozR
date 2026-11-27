//! KompozR's platforms (C++ Kompoz's platform/): the app core every platform runs an app with — composed by
//! kompoz-ui's host, drawn by kompoz-gpu's renderer into a kept frame where only what changed is drawn again, shown
//! in the platform's window, its events, frames, scripts and flags ([`app`]) — the app's entry ([`App`]), the
//! headless platform ([`headless`]), what every platform needs of the system ([`system`], [`image_loader`]), what
//! every platform's accessibility bridge is made of (the semantics tree published and what changed in it:
//! [`accessibility`]), what Linux and Android share of the kernel (`unix`: eventfds, timerfds, the monotonic clock,
//! the waker), and on Linux what its platforms share: the event loop, the kernel's other facilities, keyboards
//! through xkbcommon (`linux`).
//!
//! `unsafe` code is confined to `unix::sys`, `linux::sys` and the FFI modules (`linux::xkb`'s, `linux::wayland`'s
//! `ffi` and Vulkan surface, `linux::x11`'s, `linux::curl`'s), each block with a `// SAFETY:` comment; everything
//! else is safe.

#![deny(unsafe_code, unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

pub mod accessibility;
pub mod app;
pub mod headless;
pub mod image_loader;
mod keys;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod system;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod unix;

pub use app::{
    App, AppConfig, AppContent, AppCore, AppWindows, DISPLAY_PLATFORMS, DisplayPlatform, Event, GpuPriority,
    MakeWindow, Metrics, MouseButton, Next, Platform, TitleBar, TitleBarColors, TrayIcon, TrayImage, TrayMenuItem,
    WindowAction, WindowActions, WindowButton, WindowFrame, WindowSpec, application, button_action, button_icon,
};
