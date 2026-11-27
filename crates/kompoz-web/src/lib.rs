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
//! zone, the time zone database (the web has no zoneinfo files: `tzdata/tzdata`, every zone of the IANA database in
//! one bundle of Android's format, made by `cargo xtask tzdata` from a system's /usr/share/zoneinfo, embedded);
//! and objects the page made before the module started, by name ([`page_object`]: the `GPUDevice`, which is asked
//! for asynchronously).
//!
//! On every other target the crate is empty.

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod glue;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use glue::*;

/// The TZif bytes of zone `id` in a bundle of Android's tzdata format (a header with the version and the index's
/// and the data's offsets; the index's 52-byte entries: a zone's name, where its bytes are in the data and how
/// many); none if it has no zone of that name.
pub fn zone_in<'a>(bundle: &'a [u8], id: &str) -> Option<&'a [u8]> {
    let int = |at: usize| Some(u32::from_be_bytes(bundle.get(at..at + 4)?.try_into().ok()?) as usize);
    let (index, data) = (int(12)?, int(16)?);
    let entries = bundle.get(index..data)?;
    entries.as_chunks::<52>().0.iter().find_map(|entry| {
        let name = &entry[..40];
        let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(40)];
        if name != id.as_bytes() {
            return None;
        }
        let start = u32::from_be_bytes(entry[40..44].try_into().ok()?) as usize;
        let length = u32::from_be_bytes(entry[44..48].try_into().ok()?) as usize;
        bundle.get(data.checked_add(start)?..data.checked_add(start)?.checked_add(length)?)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_bundles_zones_are_found_by_name() {
        let bundle = include_bytes!("../tzdata/tzdata");
        assert!(bundle.starts_with(b"tzdata"));
        let istanbul = super::zone_in(bundle, "Europe/Istanbul").expect("Europe/Istanbul");
        assert!(istanbul.starts_with(b"TZif"));
        // (Links share their zone's bytes.)
        assert_eq!(super::zone_in(bundle, "Turkey"), Some(istanbul));
        assert!(super::zone_in(bundle, "UTC").is_some());
        assert_eq!(super::zone_in(bundle, "Nowhere/Else"), None);
    }
}
