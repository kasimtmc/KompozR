//! KompozR's platforms (C++ Kompoz's platform/): the app core every platform runs an app with — composed by
//! kompoz-ui's host, drawn by kompoz-gpu's renderer into a kept frame where only what changed is drawn again, shown
//! in the platform's window, its events, frames, scripts and flags ([`app`]) — the app's entry ([`App`]), the
//! headless platform ([`headless`]), what every platform needs of the system ([`system`], [`image_loader`]), what
//! every platform's accessibility bridge is made of (the semantics tree published and what changed in it:
//! [`accessibility`]), what Linux and Android share of the kernel (`unix`: eventfds, timerfds, the monotonic clock,
//! the waker), on Linux what its platforms share: the event loop, the kernel's other facilities, keyboards through
//! xkbcommon (`linux`); what the app offers other apps through the clipboard and drags (`transfer`, Linux's,
//! Windows', macOS's and the web's); the Windows platform (`windows`: Win32); the macOS platform (`macos`: AppKit);
//! the web platform (`web`: the page's canvas, WebGPU, our own JavaScript glue); and the Android platform
//! ([`android`]: the app's library, its natives, the activity's surface, frames and lifecycle; its input's decoding
//! built on every target).
//!
//! `unsafe` code is confined to `unix::sys`, `linux::sys`, `windows::sys` and the FFI modules (`linux::xkb`'s,
//! `linux::wayland`'s `ffi` and Vulkan surface, `linux::x11`'s, `linux::curl`'s, `windows`' Vulkan surface, WinHTTP,
//! IMM32, TSF, UI Automation's providers and OLE's drag and drop, `android`'s JNI, NDK, natives (and the input
//! method's own native, in `text_input`) and Vulkan surface, `web::sys`) and the macOS platform's that define or
//! call Objective-C classes through objc2 (`macos`), each block with a `// SAFETY:` comment; everything else is safe.

//!
//! What an app's build script adds to its program for a platform (the Windows program's resources) is [`build`]'s,
//! the one module of the crate without its default `platform` feature.

#![deny(unsafe_code, unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

#[cfg(feature = "platform")]
pub mod accessibility;
#[cfg(feature = "platform")]
pub mod android;
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
#[cfg(all(
    feature = "platform",
    any(target_os = "linux", windows, target_os = "macos", all(target_family = "wasm", target_os = "unknown"))
))]
pub mod transfer;
#[cfg(all(feature = "platform", any(target_os = "linux", target_os = "android")))]
pub mod unix;
// (The web's: on its target, and its parts that are plain Rust in the tests on Linux and Windows, which have the
// clipboard's `transfer` it shares.)
#[cfg(all(
    feature = "platform",
    any(all(target_family = "wasm", target_os = "unknown"), all(test, any(target_os = "linux", windows)))
))]
pub mod web;
#[cfg(all(feature = "platform", windows))]
pub mod windows;

// The Windows platform's transfer formats (the clipboard's and drag and drop's), its input methods' translations
// (what they see of a field, what they do to it) and UI Automation's view of the semantics tree, tested on every host.
#[cfg(all(test, feature = "platform", not(windows)))]
#[allow(dead_code)]
#[path = "windows"]
mod windows_tested_here {
    // (on the hosts with the clipboard's `transfer` it shares)
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    mod formats;
    mod ime;
    mod uia;
}

// The macOS platform's parts that need no AppKit (its key codes and menus' key equivalents, what its input methods
// see of a field, the pasteboard's types, what NSAccessibility says of the semantics tree), tested on every host.
#[cfg(all(test, feature = "platform", not(target_os = "macos")))]
#[allow(dead_code)]
#[path = "macos"]
mod macos_tested_here {
    mod ax;
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
