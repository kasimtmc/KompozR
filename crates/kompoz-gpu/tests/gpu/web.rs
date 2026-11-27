//! The tests in a browser (`cargo xtask web-gpu-test`: tests/gpu/web/runner.js is the page's side): what the run
//! prints goes to the page (the glue's log, which the runner forwards), the pictures it writes to the runner, and its
//! result too.
//!
//! The tests read pixels back as they do natively, at once: WebGPU only maps a buffer asynchronously, so here, and
//! only here, the read waits for it through JSPI (JavaScript Promise Integration: the page calls `main` as a
//! promising function, and [`wait_for_gpu`] is a suspending import that returns once the page's pending mappings
//! settled). The device and the renderer read back asynchronously everywhere ([`Device::read_pixels_async`]); this
//! keeps the tests the same on every backend.
//!
//! [`Device::read_pixels_async`]: kompoz_gpu::device::Device::read_pixels_async

#![allow(unsafe_code)]

#[link(wasm_import_module = "kompoz")]
unsafe extern "C" {
    fn test_wait_for_gpu();
    fn test_save_file(path: *const u8, path_length: usize, bytes: *const u8, length: usize);
    fn test_finish(checks: u32, failures: u32);
}

/// Waits (the module suspended, the page going on) until the GPU work the page tracks settled, then hands the reads
/// their pixels.
pub fn wait_for_gpu() {
    // SAFETY: no arguments; the import suspends this call until the page's pending promises settle (JSPI), nothing
    // of Rust running meanwhile.
    unsafe { test_wait_for_gpu() };
    kompoz_gpu::device::webgpu::finish_reads();
}

/// A picture the run wrote (at its path as the run names it), for the runner to save.
pub fn save_file(path: &str, bytes: &[u8]) {
    // SAFETY: the import reads the path's and the bytes' lengths at their pointers, which are theirs, copying them.
    unsafe { test_save_file(path.as_ptr(), path.len(), bytes.as_ptr(), bytes.len()) }
}

/// The run's result, for the runner.
pub fn finish(checks: usize, failures: usize) {
    // SAFETY: numbers only.
    unsafe { test_finish(checks as u32, failures as u32) }
}
