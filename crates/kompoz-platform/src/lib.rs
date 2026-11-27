//! KompozR's platforms (C++ Kompoz's platform/): the app core every platform runs an app with — composed by
//! kompoz-ui's host, drawn by kompoz-gpu's renderer into a kept frame where only what changed is drawn again, shown
//! in the platform's window, its events, frames, scripts and flags ([`app`]) — the app's entry ([`App`]), the
//! headless platform ([`headless`]), what every platform needs of the system ([`system`], [`image_loader`]), what
//! every platform's accessibility bridge is made of (the semantics tree published and what changed in it:
//! [`accessibility`]), what Linux and Android share of the kernel (`unix`: eventfds, timerfds, the monotonic clock,
//! the waker), on Linux what its platforms share: the event loop, the kernel's other facilities, keyboards through
//! xkbcommon (`linux`); what the app offers other apps through the clipboard and drags (`transfer`, Linux's,
//! Windows' and macOS's); the Windows platform (`windows`: Win32); and the macOS platform (`macos`: AppKit).
//!
//! `unsafe` code is confined to `unix::sys`, `linux::sys`, `windows::sys` and the FFI modules (`linux::xkb`'s,
//! `linux::wayland`'s `ffi` and Vulkan surface, `linux::x11`'s, `linux::curl`'s, `windows`' Vulkan surface and
//! WinHTTP) and the macOS platform's that define or call Objective-C classes through objc2 (`macos`), each block
//! with a `// SAFETY:` comment; everything else is safe.

//!
//! What an app's build script adds to its program for a platform (the Windows program's resources) is [`build`]'s,
//! the one module of the crate without its default `platform` feature.

#![deny(unsafe_code, unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

#[cfg(feature = "platform")]
pub mod accessibility;
#[cfg(feature = "platform")]
pub mod app;
pub mod build;
#[cfg(feature = "platform")]
pub mod headless;
#[cfg(feature = "platform")]
pub mod image_loader;
#[cfg(feature = "platform")]
mod keys;
#[cfg(all(feature = "platform", target_os = "linux"))]
pub mod linux;
#[cfg(all(feature = "platform", target_os = "macos"))]
pub mod macos;
#[cfg(feature = "platform")]
pub mod system;
#[cfg(all(feature = "platform", any(target_os = "linux", windows, target_os = "macos")))]
pub mod transfer;
#[cfg(all(feature = "platform", any(target_os = "linux", target_os = "android")))]
pub mod unix;
#[cfg(all(feature = "platform", windows))]
pub mod windows;

// The macOS platform's parts that need no AppKit (its key codes and menus' key equivalents, what its input methods
// see of a field, the pasteboard's types), tested on every host.
#[cfg(all(test, feature = "platform", not(target_os = "macos")))]
#[allow(dead_code)]
#[path = "macos"]
mod macos_tested_here {
    mod keyboard;
    mod text_input;
    mod types;
}

#[cfg(feature = "platform")]
pub use app::{
    App, AppConfig, AppContent, AppCore, AppWindows, DISPLAY_PLATFORMS, DisplayPlatform, Event, GpuPriority,
    MakeWindow, MenuBarShown, Metrics, MouseButton, Next, Platform, TitleBar, TitleBarColors, TrayIcon, TrayImage,
    TrayMenuItem, WindowAction, WindowActions, WindowButton, WindowFrame, WindowSpec, application, button_action,
    button_icon, single_window_application,
};
