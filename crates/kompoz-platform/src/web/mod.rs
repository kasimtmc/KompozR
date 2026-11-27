//! KompozR's platform on the web (C++'s web_platform, on our own JavaScript glue: kompoz-web, and web.js beside
//! this file): the app in the page's canvas, drawn with WebGPU; its loop on the page's animation frames.
//!
//! - **The loop.** The program's `main` starts the app and returns (the page lives on; nothing ends a web module);
//!   the app's cores live in a thread local, and the page calls in: a turn ([`turn`]) at each animation frame it
//!   asked for (requestAnimationFrame) — the page's events gathered since handed to the cores, then a frame of each
//!   core that wants one, paced by the display (frames only while the page is shown, as the browser runs animation
//!   frames) —, and at a timeout for what is due later (a coroutine's delay), when the hosts' waker asks, when work
//!   the page did for the app settled (a buffer read back, an image decoded). The page's listeners only gather
//!   (web.js's records, [`events`]); nothing in Rust runs inside Rust.
//! - **The canvas** fills the page (or is as big as the app makes it: `set_window_size`, a script's `resize`), its
//!   pixels the CSS pixels times the device pixel ratio (its device-pixel content box where the browser tells it),
//!   watched as the page resizes, zooms or moves to another screen.
//! - **Input**: pointer events (a mouse's buttons with its capture, fingers by their pointer ids, a pen's contact,
//!   pressure, barrel buttons and eraser, and its hovering), the wheel (a touchpad's pinch comes as Ctrl+wheel: the
//!   app zooms), keys by their codes and what they type ([`keys`]); typing through a hidden text area holding the
//!   focused field's text ([`ime`]: input methods' compositions as the field's composing text, their surrounding
//!   text, a phone keyboard's corrections and deletions).
//! - **The clipboard** both ways ([`clipboard`]): Ctrl+C and Ctrl+X answered in the page's copy and cut events,
//!   Ctrl+V held back until the paste event brought what the clipboard holds (as C++ did), a copy of the app's own
//!   through the Clipboard API; drops of files and text in, the app's drags out where the browser lets a page begin
//!   one (a drag of a press the app's drag within it took first: a long press's).
//! - **The page**: the cursor, the theme, reduced motion (animations end at once) and the pointer's precision from
//!   media queries, the screen's safe area, the title, full screen, links opened in a new tab, vibration, the
//!   screen kept on (Wake Lock), the address's fragment as the shown screen's path and a deep link (C++'s), the
//!   focus and the visibility as the app's lifecycle, the app's state kept across reloads of the tab
//!   (sessionStorage, written as the page goes or hides: a process death's restore), screenshots handed to the page
//!   (a download, or a test runner).
//! - **Text and images** (W2): the bundled Noto Sans (Regular, Medium, Bold) as the default family, what no font
//!   has drawn by the browser ([`text`]: its canvas 2D, emoji in colour); images from http(s) fetched and decoded by
//!   the browser ([`images`]), the app's own by its decoders; files dropped or pasted, and assets the app embeds,
//!   read from memory ([`files`]).
//!
//! The ARIA mirror of the semantics tree (screen readers) is a later package's.

// (Off the web only the host's tests build this module: its parts that are plain Rust, what the page's calls use
// of them unused there.)
#![cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), allow(dead_code))]

mod clipboard;
mod events;
pub mod files;
mod ime;
mod keys;
mod page;
pub mod text;

// (What talks to the page: on the web only.)
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod images;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod run;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod sys;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod window;

pub use files::embed_assets;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use run::PLATFORM;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub(crate) use run::STATE_KEY;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use window::WebWindow;
