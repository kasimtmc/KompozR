//! The WebGPU calls the device makes, as imports of the web glue's module `kompoz` (kompoz-web; their JavaScript is
//! webgpu.js beside this file), each wrapped safely. Objects are the glue's handles (u32, 0: none): GPU objects
//! made here are owned by [`JsObject`]s; the page's (its `GPUDevice`, its canvas) are borrowed handles. Numbers
//! name what WebGPU names with strings (formats, blend factors …): webgpu.js has the tables.

#![allow(unsafe_code)]

use kompoz_web::JsObject;

/// The imports (the wrappers below have their names).
mod imports {
    #[link(wasm_import_module = "kompoz")]
    unsafe extern "C" {
        pub(super) fn gpu_unavailable() -> i32;
        pub(super) fn gpu_device_attach(device: u32) -> u32;
        pub(super) fn gpu_device_name(device: u32) -> i32;
        pub(super) fn gpu_device_errors(device: u32) -> u32;
        pub(super) fn gpu_preferred_format() -> u32;
        pub(super) fn gpu_create_texture(
            device: u32,
            width: u32,
            height: u32,
            format: u32,
            attachment: u32,
            levels: u32,
        ) -> u32;
        pub(super) fn gpu_destroy_texture(texture: u32);
        pub(super) fn gpu_shader_module(device: u32, code: *const u8, length: usize) -> u32;
        pub(super) fn gpu_bind_group_layout(device: u32, textures: u32, texel_textures: u32, uniforms: u32) -> u32;
        pub(super) fn gpu_mipmap_layout(device: u32) -> u32;
        pub(super) fn gpu_render_pipeline(
            device: u32,
            layout: u32,
            vertex: u32,
            vertex_entry: *const u8,
            vertex_entry_length: usize,
            fragment: u32,
            fragment_entry: *const u8,
            fragment_entry_length: usize,
            desc: *const u32,
            desc_words: usize,
        ) -> u32;
        pub(super) fn gpu_sampler(device: u32, linear: u32, wrap_u: u32, wrap_v: u32, lod_max: f32) -> u32;
        pub(super) fn gpu_create_buffer(device: u32, size: f64, usage: u32) -> u32;
        pub(super) fn gpu_destroy_buffer(buffer: u32);
        pub(super) fn gpu_write_buffer(device: u32, buffer: u32, offset: f64, data: *const u8, length: usize);
        pub(super) fn gpu_command_encoder(device: u32) -> u32;
        pub(super) fn gpu_submit(device: u32, encoder: u32);
        pub(super) fn gpu_copy_buffer_to_texture(
            encoder: u32,
            buffer: u32,
            offset: f64,
            bytes_per_row: u32,
            texture: u32,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        );
        pub(super) fn gpu_copy_texture_to_texture(
            encoder: u32,
            source: u32,
            sx: u32,
            sy: u32,
            destination: u32,
            dx: u32,
            dy: u32,
            width: u32,
            height: u32,
        );
        pub(super) fn gpu_copy_texture_to_buffer(
            encoder: u32,
            texture: u32,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
            buffer: u32,
            bytes_per_row: u32,
        );
        pub(super) fn gpu_copy_buffer_to_buffer(encoder: u32, source: u32, destination: u32, size: f64);
        pub(super) fn gpu_query_set(device: u32, count: u32) -> u32;
        pub(super) fn gpu_destroy_query_set(queries: u32);
        pub(super) fn gpu_resolve_queries(encoder: u32, queries: u32, count: u32, buffer: u32);
        pub(super) fn gpu_begin_pass(
            encoder: u32,
            texture: u32,
            level: u32,
            clear: u32,
            r: f64,
            g: f64,
            b: f64,
            a: f64,
            queries: u32,
            first_query: u32,
        ) -> u32;
        pub(super) fn gpu_pass_clear_area(
            device: u32,
            pass: u32,
            format: u32,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
            r: f64,
            g: f64,
            b: f64,
            a: f64,
        );
        pub(super) fn gpu_pass_pipeline(pass: u32, pipeline: u32);
        pub(super) fn gpu_pass_scissor(pass: u32, x: u32, y: u32, width: u32, height: u32);
        pub(super) fn gpu_pass_bind_group(pass: u32, group: u32, offset: i32);
        pub(super) fn gpu_pass_vertex_buffer(pass: u32, buffer: u32, offset: f64);
        pub(super) fn gpu_pass_draw(pass: u32, vertices: u32, instances: u32);
        pub(super) fn gpu_pass_end(pass: u32);
        pub(super) fn gpu_bind_group(device: u32, layout: u32, entries: *const u32, count: usize) -> u32;
        pub(super) fn gpu_map_read(buffer: u32, size: f64);
        pub(super) fn gpu_map_state(buffer: u32) -> i32;
        pub(super) fn gpu_map_take(buffer: u32, into: *mut u8);
        pub(super) fn gpu_canvas_configure(device: u32, canvas: u32, format: u32, width: u32, height: u32) -> u32;
        pub(super) fn gpu_canvas_texture(canvas: u32) -> u32;
    }
}

/// Imports taking only numbers (handles, sizes, enumerations), wrapped: they read and write none of our memory.
macro_rules! numbers_only {
    ($($(#[$meta:meta])* fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        $(
            $(#[$meta])*
            #[allow(clippy::too_many_arguments)]
            pub fn $name($($arg: $ty),*) $(-> $ret)? {
                // SAFETY: the import takes numbers only (handles of the glue's table, sizes, enumerations) and
                // touches none of the wasm memory; a handle with nothing behind it is the page's error, counted or
                // logged there.
                unsafe { imports::$name($($arg),*) }
            }
        )*
    };
}

numbers_only! {
    /// Watches the page's device (its errors counted, its loss logged); its features (bit 0: timestamp queries).
    fn gpu_device_attach(device: u32) -> u32;
    /// The errors WebGPU reported for it so far (uncaptured ones: every validation error).
    fn gpu_device_errors(device: u32) -> u32;
    /// The canvas format the browser prefers (0: RGBA8, 1: BGRA8).
    fn gpu_preferred_format() -> u32;
    /// A texture of the format (webgpu.js's table: RGBA8, BGRA8, R16F, RGBA32F, RGBA16F), drawn into if
    /// `attachment` is 1, with `levels` mip levels.
    fn gpu_create_texture(device: u32, width: u32, height: u32, format: u32, attachment: u32, levels: u32) -> u32;
    /// Destroys the texture (after the work submitted with it: WebGPU's rule) and lets go of its handle.
    fn gpu_destroy_texture(texture: u32);
    /// The layout of a pipeline's bind group: the uniforms (binding 0, `uniforms` bytes at a dynamic offset), then
    /// each texture's texture and sampler (2i + 1, 2i + 2), unfilterable with a non-filtering sampler where
    /// `texel_textures` has its bit.
    fn gpu_bind_group_layout(device: u32, textures: u32, texel_textures: u32, uniforms: u32) -> u32;
    /// The mipmaps' bind group layout: the level before (binding 0) and a filtering sampler (1).
    fn gpu_mipmap_layout(device: u32) -> u32;
    /// A sampler (`linear`: 1 filters linearly; wraps: 0 clamp, 1 repeat, 2 mirror; mip levels up to `lod_max`).
    fn gpu_sampler(device: u32, linear: u32, wrap_u: u32, wrap_v: u32, lod_max: f32) -> u32;
    /// A buffer of `size` bytes for `usage` (0: the frame's memory, 1: a read-back, 2: resolved queries).
    fn gpu_create_buffer(device: u32, size: f64, usage: u32) -> u32;
    /// Destroys the buffer (after the work submitted with it) and lets go of its handle.
    fn gpu_destroy_buffer(buffer: u32);
    fn gpu_command_encoder(device: u32) -> u32;
    /// Finishes the encoder (letting go of it) and submits its commands.
    fn gpu_submit(device: u32, encoder: u32);
    fn gpu_copy_buffer_to_texture(
        encoder: u32,
        buffer: u32,
        offset: f64,
        bytes_per_row: u32,
        texture: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    );
    fn gpu_copy_texture_to_texture(
        encoder: u32,
        source: u32,
        sx: u32,
        sy: u32,
        destination: u32,
        dx: u32,
        dy: u32,
        width: u32,
        height: u32,
    );
    fn gpu_copy_texture_to_buffer(
        encoder: u32,
        texture: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        buffer: u32,
        bytes_per_row: u32,
    );
    fn gpu_copy_buffer_to_buffer(encoder: u32, source: u32, destination: u32, size: f64);
    /// A set of `count` timestamp queries.
    fn gpu_query_set(device: u32, count: u32) -> u32;
    fn gpu_destroy_query_set(queries: u32);
    fn gpu_resolve_queries(encoder: u32, queries: u32, count: u32, buffer: u32);
    /// A render pass into the texture's mip `level`, cleared (`clear` 1) or loaded, its start and end written into
    /// queries `first_query` and the next of `queries` (0: not timed).
    fn gpu_begin_pass(
        encoder: u32,
        texture: u32,
        level: u32,
        clear: u32,
        r: f64,
        g: f64,
        b: f64,
        a: f64,
        queries: u32,
        first_query: u32,
    ) -> u32;
    /// Clears a part of the pass's target (of the format) to the color: a triangle over it, scissored to the part,
    /// writing the blend constant (WebGPU clears whole targets only). The pass's pipeline and scissor are then
    /// another's.
    fn gpu_pass_clear_area(
        device: u32,
        pass: u32,
        format: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        r: f64,
        g: f64,
        b: f64,
        a: f64,
    );
    fn gpu_pass_pipeline(pass: u32, pipeline: u32);
    fn gpu_pass_scissor(pass: u32, x: u32, y: u32, width: u32, height: u32);
    /// Group 0, with its dynamic offset (negative: none).
    fn gpu_pass_bind_group(pass: u32, group: u32, offset: i32);
    fn gpu_pass_vertex_buffer(pass: u32, buffer: u32, offset: f64);
    fn gpu_pass_draw(pass: u32, vertices: u32, instances: u32);
    /// Ends the pass (letting go of it).
    fn gpu_pass_end(pass: u32);
    /// Maps a read-back buffer: once it is, its bytes are copied out on the page and the buffer destroyed
    /// ([`gpu_map_state`] then says how many).
    fn gpu_map_read(buffer: u32, size: f64);
    /// A mapped read's bytes: their number once they are there, -1 while being mapped, -2 if it failed.
    fn gpu_map_state(buffer: u32) -> i32;
    /// Configures the canvas's `webgpu` context for the device (its size the drawable's); 0 if it can't be.
    fn gpu_canvas_configure(device: u32, canvas: u32, format: u32, width: u32, height: u32) -> u32;
    /// The canvas's texture for this frame (a handle of its own; 0: none).
    fn gpu_canvas_texture(canvas: u32) -> u32;
}

/// Why the page has no WebGPU device (as the loader found when it asked for one); none if it said nothing.
pub fn unavailable() -> Option<String> {
    // SAFETY: the import holds the reason as the glue's result and returns its length (or -1).
    kompoz_web::take_string(unsafe { imports::gpu_unavailable() })
}

/// The device's adapter's name (as `GPUAdapterInfo` has it).
pub fn device_name(device: u32) -> String {
    // SAFETY: the import holds the name as the glue's result and returns its length.
    kompoz_web::take_string(unsafe { imports::gpu_device_name(device) }).unwrap_or_default()
}

/// A WGSL module.
pub fn shader_module(device: u32, code: &str) -> Option<JsObject> {
    // SAFETY: the import reads `code.len()` bytes at its pointer, which are the string's.
    JsObject::from_raw(unsafe { imports::gpu_shader_module(device, code.as_ptr(), code.len()) })
}

/// A render pipeline (its layout made from `layout`), described by `desc`'s words as webgpu.js reads them.
pub fn render_pipeline(
    device: u32,
    layout: u32,
    vertex: (u32, &str),
    fragment: (u32, &str),
    desc: &[u32],
) -> Option<JsObject> {
    // SAFETY: the import reads the two entry points' bytes and `desc.len()` words at their pointers, which are the
    // strings' and the slice's.
    JsObject::from_raw(unsafe {
        imports::gpu_render_pipeline(
            device,
            layout,
            vertex.0,
            vertex.1.as_ptr(),
            vertex.1.len(),
            fragment.0,
            fragment.1.as_ptr(),
            fragment.1.len(),
            desc.as_ptr(),
            desc.len(),
        )
    })
}

/// `data` into the buffer at `offset` (the queue's write, before the commands submitted after it).
pub fn write_buffer(device: u32, buffer: u32, offset: u64, data: &[u8]) {
    // SAFETY: the import reads `data.len()` bytes at its pointer, which are the slice's, copying them at once.
    unsafe { imports::gpu_write_buffer(device, buffer, offset as f64, data.as_ptr(), data.len()) }
}

/// A bind group of the layout, its entries 4 words each: binding, kind (0: a buffer, `size` bytes of it; 1: a
/// texture's view of a mip level, of all of them for `u32::MAX`; 2: a sampler), the object, and the size or level.
pub fn bind_group(device: u32, layout: u32, entries: &[[u32; 4]]) -> Option<JsObject> {
    // SAFETY: the import reads `entries.len()` · 4 words at the pointer, which are the slice's.
    JsObject::from_raw(unsafe { imports::gpu_bind_group(device, layout, entries.as_ptr().cast(), entries.len()) })
}

/// A mapped read's bytes ([`gpu_map_state`]'s count of them), copied out (the page then holds them no longer).
pub fn map_take(buffer: u32, length: usize) -> Vec<u8> {
    let mut bytes = Vec::<u8>::with_capacity(length);
    // SAFETY: the page holds exactly `length` bytes for the buffer (gpu_map_state said so just before, and nothing
    // ran in between), which the import copies into the capacity at the pointer.
    unsafe {
        imports::gpu_map_take(buffer, bytes.as_mut_ptr());
        bytes.set_len(length);
    }
    bytes
}
