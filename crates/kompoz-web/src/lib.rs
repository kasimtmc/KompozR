//! The web's JavaScript glue, KompozR's own (no wasm-bindgen): what every part of KompozR that talks to the page
//! shares, so that each adds only its own calls. A part declares the page's functions it calls as imports of the
//! wasm module `kompoz` (`#[link(wasm_import_module = "kompoz")] unsafe extern "C" { … }`), and its JavaScript gives
//! them: `js/kompoz.js` is the loader, which joins the parts' imports (each part a function of the [`glue`](#the-glue)
//! returning its functions by name) into one `kompoz` import object and instantiates the module. The parts so far:
//! the WebGPU device (kompoz-gpu's `device/webgpu/webgpu.js`); the web platform (the page, its events, the
//! clipboard, fetches, canvas 2D) adds its own the same way.
//!
//! # The glue
//!
//! - **Objects as handles.** JavaScript objects (a `GPUDevice`, a `<canvas>`, a promise's result) stay on the page,
//!   in the glue's table; Rust holds their handles, `u32`s (0: none). An owned one is a [`JsObject`], which lets go
//!   of its entry when dropped; one held by someone else (a handle in a struct the platform fills) is a bare `u32`.
//! - **Strings and bytes into JavaScript**: a pointer and a length into the wasm memory, which the import reads
//!   (UTF-8 for strings). JavaScript views the memory anew on every read: its buffer is replaced when it grows.
//! - **Strings and bytes out of JavaScript**: an import that gives some holds them as the glue's result and returns
//!   their length (negative: none); [`take_result`] copies them into a new `Vec` (and lets go of them). No export is
//!   called back for them, so nothing in Rust runs while Rust is in the middle of something else.
//! - **Errors** stay in JavaScript: an import catches what the page throws and says so in what it returns (a
//!   JavaScript exception unwinding through Rust's frames would skip their cleanup). A Rust panic is Rust's
//!   (unwinding, as natively: the web build rebuilds std with `panic=unwind`); [`start`]'s hook writes it to the
//!   console.
//!
//! [`start`] gives the runtime what the web has instead of a process's: the page's clocks, its query string as the
//! environment (`?KOMPOZ_GPU_VALIDATION=1&LANG=tr_TR`, as C++'s web build had it), the browser's languages and time
//! zone; and objects the page made before the module started, by name ([`page_object`]: the `GPUDevice`, which is
//! asked for asynchronously).
//!
//! On every other target the crate is empty.

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod glue;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use glue::*;
