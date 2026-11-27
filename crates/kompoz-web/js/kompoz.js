// KompozR's JavaScript glue (crates/kompoz-web; its Rust side is src/glue.rs): the loader that joins the parts'
// imports into the wasm module's one import module, `kompoz`, and the glue they share:
//
// - the page's objects in a table, Rust holding their handles (u32; 0 is none): add, get, release, clone;
// - strings and bytes from Rust as a pointer and a length into the memory, read through a view made anew each time
//   (the memory's buffer is replaced when it grows);
// - strings and bytes to Rust as the glue's result: an import holds them (hold) and returns their length, Rust
//   copies them out (result_take) — no export is called back, so Rust never runs inside Rust;
// - what the page made before the module started, by name (page objects: the GPUDevice, asked for
//   asynchronously), and the page's query string, languages and time zone (kompoz_web::start).
//
// A part (the WebGPU device's webgpu.js, the web platform's) is a function of the glue returning its imports by
// name, each catching what the page throws (a JavaScript exception must not unwind through Rust's frames) and
// saying so in what it returns.
//
//   import { instantiate } from "./kompoz.js";
//   import { gpuImports, requestGpu } from "./webgpu.js";
//   const { glue, instance } = await instantiate(fetch("app.wasm"), { parts: [gpuImports], before: [requestGpu] });
//   instance.exports.main(0, 0);

export class Glue {
    constructor() {
        this.objects = [undefined]; // (handle 0: none)
        this.free = [];
        this.result = null;
        this.page = new Map();
        this.instance = null;
        this.decoder = new TextDecoder();
        this.encoder = new TextEncoder();
        // Where messages go besides the console (the test runner listens): (level, text) => {}.
        this.onLog = null;
        // Promises of work Rust started that settles later (a buffer's mapping): a page waiting for all of it
        // (the test runner's) waits for these; each removes itself once settled.
        this.pending = new Set();
        // Called when such work has settled and Rust has something to pick up (the web platform's loop schedules
        // a turn: kompoz-gpu's device::webgpu::finish_reads).
        this.onSettled = null;
    }

    add(object) {
        if (object === undefined || object === null) return 0;
        const handle = this.free.length ? this.free.pop() : this.objects.length;
        this.objects[handle] = object;
        return handle;
    }

    get(handle) {
        return this.objects[handle];
    }

    release(handle) {
        if (handle === 0 || this.objects[handle] === undefined) return;
        this.objects[handle] = undefined;
        this.free.push(handle);
    }

    // The memory as bytes, viewed now.
    bytes() {
        return new Uint8Array(this.instance.exports.memory.buffer);
    }

    // `length` bytes at `pointer`, viewed now (valid until the memory grows: copy them to keep them).
    view(pointer, length) {
        return this.bytes().subarray(pointer, pointer + length);
    }

    string(pointer, length) {
        return this.decoder.decode(this.view(pointer, length));
    }

    // Holds `value` (a string, UTF-8, or bytes) as the result; its length (none: -1).
    hold(value) {
        if (value === undefined || value === null) {
            this.result = null;
            return -1;
        }
        this.result = typeof value === "string" ? this.encoder.encode(value) : value;
        return this.result.length;
    }

    // Work Rust started that settles later, kept until it does.
    track(promise) {
        const tracked = promise.finally(() => {
            this.pending.delete(tracked);
            if (this.onSettled) this.onSettled();
        });
        this.pending.add(tracked);
        return tracked;
    }

    log(level, text) {
        (level === 2 ? console.error : level === 1 ? console.warn : console.log)(text);
        if (this.onLog) this.onLog(level, text);
    }
}

// The imports every part shares (kompoz_web's glue.rs).
export function coreImports(glue) {
    return {
        handle_release: (handle) => glue.release(handle),
        handle_clone: (handle) => glue.add(glue.get(handle)),
        result_take: (pointer) => {
            if (glue.result) glue.bytes().set(glue.result, pointer);
            glue.result = null;
        },
        result_drop: () => {
            glue.result = null;
        },
        log_message: (level, pointer, length) => glue.log(level, glue.string(pointer, length)),
        clock_monotonic: () => performance.now(),
        clock_unix: () => Date.now(),
        page_query: () => {
            const lines = [];
            for (const [name, value] of new URLSearchParams(location.search)) lines.push(`${name}=${value}`);
            return glue.hold(lines.join("\n"));
        },
        page_languages: () => glue.hold((navigator.languages || [navigator.language]).filter(Boolean).join("\n")),
        page_time_zone: () => {
            try {
                return glue.hold(Intl.DateTimeFormat().resolvedOptions().timeZone || "");
            } catch {
                return glue.hold("");
            }
        },
        page_find: (pointer, length) => glue.add(glue.page.get(glue.string(pointer, length))),
    };
}

// Instantiates the module (`source`: a fetch's response, or its bytes) with the core's imports and the parts'
// (functions of the glue), after the asynchronous steps the parts need first (`before`: functions of the glue
// returning promises, as webgpu.js's requestGpu, which puts the GPUDevice among the page objects).
export async function instantiate(source, { parts = [], before = [], page = {} } = {}) {
    const glue = new Glue();
    for (const [name, object] of Object.entries(page)) glue.page.set(name, object);
    for (const step of before) await step(glue);
    const kompoz = coreImports(glue);
    for (const part of parts) Object.assign(kompoz, part(glue));
    const imports = { kompoz };
    const response = await source;
    const { instance } =
        response instanceof Response
            ? await WebAssembly.instantiateStreaming(response, imports)
            : await WebAssembly.instantiate(response, imports);
    glue.instance = instance;
    return { glue, instance };
}
