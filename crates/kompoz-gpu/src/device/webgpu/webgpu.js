// The WebGPU device's calls (kompoz-gpu's device/webgpu: sys.rs declares them, mod.rs makes them), a part of the
// web glue (kompoz-web's js/kompoz.js): WebGPU's objects in the glue's table, Rust holding their handles, the numbers
// Rust passes for WebGPU's strings looked up here.
//
// - Textures are records { texture, views } (a view per mip level, made when first asked for); buffers records
//   { buffer, state, data } (a read-back's bytes copied out once it is mapped, the buffer then destroyed).
// - Errors: WebGPU reports what it finds wrong asynchronously (uncaptured errors, counted per device and logged);
//   what a call throws is caught, logged and counted the same way (it must not unwind through Rust's frames).
// - requestGpu, a loader step (before the module starts: WebGPU's device is asked for asynchronously), puts the
//   adapter and device among the page's objects ("gpu.adapter", "gpu.device"), or why there is none
//   ("gpu.unavailable").

const FORMATS = ["rgba8unorm", "bgra8unorm", "r16float", "rgba32float", "rgba16float"];
const VERTEX_FORMATS = ["float32", "float32x2", "float32x3", "float32x4", "uint32x4"];
const FACTORS = [
    "zero",
    "one",
    "src-alpha",
    "one-minus-src-alpha",
    "dst-alpha",
    "one-minus-dst-alpha",
    "src",
    "one-minus-src",
    "dst",
    "one-minus-dst",
];
const OPS = ["add", "subtract", "reverse-subtract", "min", "max"];
const WRAPS = ["clamp-to-edge", "repeat", "mirror-repeat"];
const MAX_TEXTURES = 6;
// A pass's area cleared (WebGPU clears whole targets): a triangle over the target, scissored to the area, writing
// ones that the blend makes the clear color (its constant).
const CLEAR_SHADER = `
@vertex fn vs(@builtin(vertex_index) i : u32) -> @builtin(position) vec4f {
    return vec4f(f32((i << 1u) & 2u) * 2 - 1, f32(i & 2u) * 2 - 1, 0, 1);
}
@fragment fn fs() -> @location(0) vec4f { return vec4f(1); }
`;

// The page's WebGPU adapter and device, asked for before the module starts (a step of the loader's `before`).
export async function requestGpu(glue) {
    try {
        if (!navigator.gpu) throw new Error("this browser has no WebGPU (navigator.gpu)");
        const adapter = await navigator.gpu.requestAdapter();
        if (!adapter) throw new Error("no WebGPU adapter");
        // (Timestamps where the adapter has them: Device::set_gpu_timing.)
        const requiredFeatures = adapter.features.has("timestamp-query") ? ["timestamp-query"] : [];
        const device = await adapter.requestDevice({ requiredFeatures });
        glue.page.set("gpu.adapter", adapter);
        glue.page.set("gpu.device", device);
    } catch (e) {
        glue.page.set("gpu.unavailable", String((e && e.message) || e));
    }
}

export function gpuImports(glue) {
    // Each device's errors (WebGPU's uncaptured ones and what the calls threw).
    const devices = new WeakMap();
    let thrown = 0;
    const contexts = new WeakMap();
    // Each device's pipelines clearing an area, by format.
    const clears = new WeakMap();
    const get = (handle) => glue.get(handle);
    const u32s = (pointer, count) => new Uint32Array(glue.instance.exports.memory.buffer, pointer, count);

    const view = (record, level) => {
        const key = level === 0xffffffff ? "all" : level;
        let v = record.views.get(key);
        if (!v) {
            v =
                level === 0xffffffff
                    ? record.texture.createView()
                    : record.texture.createView({ baseMipLevel: level, mipLevelCount: 1 });
            record.views.set(key, v);
        }
        return v;
    };

    const imports = {
        gpu_unavailable: () => glue.hold(glue.page.get("gpu.unavailable") ?? null),

        gpu_device_attach: (device) => {
            const d = get(device);
            if (!devices.has(d)) {
                const state = { errors: 0 };
                devices.set(d, state);
                d.addEventListener("uncapturederror", (event) => {
                    state.errors++;
                    glue.log(2, `WebGPU: ${event.error.message}`);
                });
                d.lost.then((info) => {
                    if (info.reason !== "destroyed") glue.log(2, `WebGPU: the device was lost: ${info.message}`);
                });
            }
            return d.features.has("timestamp-query") ? 1 : 0;
        },

        gpu_device_name: (device) => {
            const d = get(device);
            const adapter = glue.page.get("gpu.adapter");
            const info = d.adapterInfo || (adapter && adapter.info) || {};
            const name = info.description || [info.vendor, info.architecture, info.device].filter(Boolean).join(" ");
            return glue.hold(name || "WebGPU");
        },

        gpu_device_errors: (device) => (devices.get(get(device))?.errors ?? 0) + thrown,

        gpu_preferred_format: () => (navigator.gpu.getPreferredCanvasFormat() === "rgba8unorm" ? 0 : 1),

        gpu_create_texture: (device, width, height, format, attachment, levels) => {
            let usage = GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC;
            if (attachment) usage |= GPUTextureUsage.RENDER_ATTACHMENT;
            const texture = get(device).createTexture({
                size: [width, height],
                format: FORMATS[format],
                usage,
                mipLevelCount: levels,
            });
            return glue.add({ texture, views: new Map() });
        },

        gpu_destroy_texture: (texture) => {
            const record = get(texture);
            if (record) record.texture.destroy();
            glue.release(texture);
        },

        gpu_shader_module: (device, pointer, length) =>
            glue.add(get(device).createShaderModule({ code: glue.string(pointer, length) })),

        gpu_bind_group_layout: (device, textures, texelTextures, uniforms) => {
            const entries = [
                {
                    binding: 0,
                    visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
                    buffer: { type: "uniform", hasDynamicOffset: true, minBindingSize: uniforms },
                },
            ];
            for (let i = 0; i < textures && i < MAX_TEXTURES; i++) {
                const texel = (texelTextures >> i) & 1;
                entries.push({
                    binding: 2 * i + 1,
                    visibility: GPUShaderStage.FRAGMENT,
                    texture: { sampleType: texel ? "unfilterable-float" : "float", viewDimension: "2d" },
                });
                entries.push({
                    binding: 2 * i + 2,
                    visibility: GPUShaderStage.FRAGMENT,
                    sampler: { type: texel ? "non-filtering" : "filtering" },
                });
            }
            return glue.add(get(device).createBindGroupLayout({ entries }));
        },

        gpu_mipmap_layout: (device) =>
            glue.add(
                get(device).createBindGroupLayout({
                    entries: [
                        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "float" } },
                        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "filtering" } },
                    ],
                }),
            ),

        gpu_render_pipeline: (device, layout, vertex, vsEntry, vsLength, fragment, fsEntry, fsLength, desc, words) => {
            const d = get(device);
            const w = Array.from(u32s(desc, words));
            const [target, blending, srcColor, dstColor, colorOp, srcAlpha, dstAlpha, alphaOp, stride, count] = w;
            let at = 10;
            const attributes = [];
            for (let i = 0; i < count; i++, at += 3)
                attributes.push({ shaderLocation: w[at], format: VERTEX_FORMATS[w[at + 1]], offset: w[at + 2] });
            const constants = {};
            const constantCount = w[at++];
            for (let i = 0; i < constantCount; i++) constants[String(i)] = w[at + i] | 0; // (ints)
            // (Min and max take no factors: WebGPU wants them one.)
            const component = (op, src, dst) =>
                op >= 3
                    ? { operation: OPS[op], srcFactor: "one", dstFactor: "one" }
                    : { operation: OPS[op], srcFactor: FACTORS[src], dstFactor: FACTORS[dst] };
            const color = { format: FORMATS[target] };
            if (blending)
                color.blend = {
                    color: component(colorOp, srcColor, dstColor),
                    alpha: component(alphaOp, srcAlpha, dstAlpha),
                };
            const pipeline = d.createRenderPipeline({
                layout: d.createPipelineLayout({ bindGroupLayouts: [get(layout)] }),
                vertex: {
                    module: get(vertex),
                    entryPoint: glue.string(vsEntry, vsLength),
                    buffers: stride ? [{ arrayStride: stride, stepMode: "instance", attributes }] : [],
                },
                fragment: {
                    module: get(fragment),
                    entryPoint: glue.string(fsEntry, fsLength),
                    targets: [color],
                    constants,
                },
                primitive: { topology: "triangle-list" },
            });
            return glue.add(pipeline);
        },

        gpu_sampler: (device, linear, wrapU, wrapV, lodMax) => {
            const filter = linear ? "linear" : "nearest";
            return glue.add(
                get(device).createSampler({
                    addressModeU: WRAPS[wrapU],
                    addressModeV: WRAPS[wrapV],
                    magFilter: filter,
                    minFilter: filter,
                    mipmapFilter: "nearest",
                    lodMaxClamp: lodMax,
                }),
            );
        },

        gpu_create_buffer: (device, size, usage) => {
            const usages = [
                GPUBufferUsage.VERTEX | GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
                GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
                GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
            ];
            const buffer = get(device).createBuffer({ size, usage: usages[usage] });
            return glue.add({ buffer, state: -1, data: null });
        },

        gpu_destroy_buffer: (buffer) => {
            const record = get(buffer);
            if (record) record.buffer.destroy();
            glue.release(buffer);
        },

        gpu_write_buffer: (device, buffer, offset, pointer, length) =>
            get(device).queue.writeBuffer(get(buffer).buffer, offset, glue.view(pointer, length)),

        gpu_command_encoder: (device) => glue.add(get(device).createCommandEncoder()),

        gpu_submit: (device, encoder) => {
            const commands = get(encoder).finish();
            glue.release(encoder);
            get(device).queue.submit([commands]);
        },

        gpu_copy_buffer_to_texture: (encoder, buffer, offset, bytesPerRow, texture, x, y, width, height) =>
            get(encoder).copyBufferToTexture(
                { buffer: get(buffer).buffer, offset, bytesPerRow, rowsPerImage: height },
                { texture: get(texture).texture, origin: [x, y] },
                [width, height],
            ),

        gpu_copy_texture_to_texture: (encoder, source, sx, sy, destination, dx, dy, width, height) =>
            get(encoder).copyTextureToTexture(
                { texture: get(source).texture, origin: [sx, sy] },
                { texture: get(destination).texture, origin: [dx, dy] },
                [width, height],
            ),

        gpu_copy_texture_to_buffer: (encoder, texture, x, y, width, height, buffer, bytesPerRow) =>
            get(encoder).copyTextureToBuffer(
                { texture: get(texture).texture, origin: [x, y] },
                { buffer: get(buffer).buffer, bytesPerRow, rowsPerImage: height },
                [width, height],
            ),

        gpu_copy_buffer_to_buffer: (encoder, source, destination, size) =>
            get(encoder).copyBufferToBuffer(get(source).buffer, 0, get(destination).buffer, 0, size),

        gpu_query_set: (device, count) => glue.add(get(device).createQuerySet({ type: "timestamp", count })),

        gpu_destroy_query_set: (queries) => {
            const set = get(queries);
            if (set) set.destroy();
            glue.release(queries);
        },

        gpu_resolve_queries: (encoder, queries, count, buffer) =>
            get(encoder).resolveQuerySet(get(queries), 0, count, get(buffer).buffer, 0),

        gpu_begin_pass: (encoder, texture, level, clear, r, g, b, a, queries, firstQuery) => {
            const descriptor = {
                colorAttachments: [
                    {
                        view: view(get(texture), level),
                        loadOp: clear ? "clear" : "load",
                        storeOp: "store",
                        clearValue: { r, g, b, a },
                    },
                ],
            };
            if (queries)
                descriptor.timestampWrites = {
                    querySet: get(queries),
                    beginningOfPassWriteIndex: firstQuery,
                    endOfPassWriteIndex: firstQuery + 1,
                };
            return glue.add(get(encoder).beginRenderPass(descriptor));
        },

        gpu_pass_clear_area: (device, pass, format, x, y, width, height, r, g, b, a) => {
            const d = get(device);
            let pipelines = clears.get(d);
            if (!pipelines) clears.set(d, (pipelines = new Map()));
            let pipeline = pipelines.get(format);
            if (!pipeline) {
                const module = d.createShaderModule({ code: CLEAR_SHADER });
                // (What it writes is the blend constant: the source times it, the destination times nothing.)
                const constant = { operation: "add", srcFactor: "constant", dstFactor: "zero" };
                pipeline = d.createRenderPipeline({
                    layout: d.createPipelineLayout({ bindGroupLayouts: [] }),
                    vertex: { module, entryPoint: "vs" },
                    fragment: {
                        module,
                        entryPoint: "fs",
                        targets: [{ format: FORMATS[format], blend: { color: constant, alpha: constant } }],
                    },
                    primitive: { topology: "triangle-list" },
                });
                pipelines.set(format, pipeline);
            }
            const p = get(pass);
            p.setPipeline(pipeline);
            p.setBlendConstant({ r, g, b, a });
            p.setScissorRect(x, y, width, height);
            p.draw(3);
        },

        gpu_pass_pipeline: (pass, pipeline) => get(pass).setPipeline(get(pipeline)),

        gpu_pass_scissor: (pass, x, y, width, height) => get(pass).setScissorRect(x, y, width, height),

        gpu_pass_bind_group: (pass, group, offset) =>
            offset < 0 ? get(pass).setBindGroup(0, get(group)) : get(pass).setBindGroup(0, get(group), [offset]),

        gpu_pass_vertex_buffer: (pass, buffer, offset) => get(pass).setVertexBuffer(0, get(buffer).buffer, offset),

        gpu_pass_draw: (pass, vertices, instances) => get(pass).draw(vertices, instances),

        gpu_pass_end: (pass) => {
            get(pass).end();
            glue.release(pass);
        },

        gpu_bind_group: (device, layout, pointer, count) => {
            const w = u32s(pointer, count * 4);
            const entries = [];
            for (let i = 0; i < count; i++) {
                const [binding, kind, object, extra] = [w[4 * i], w[4 * i + 1], w[4 * i + 2], w[4 * i + 3]];
                const resource =
                    kind === 0
                        ? { buffer: get(object).buffer, offset: 0, size: extra }
                        : kind === 1
                          ? view(get(object), extra)
                          : get(object);
                entries.push({ binding, resource });
            }
            return glue.add(get(device).createBindGroup({ layout: get(layout), entries }));
        },

        gpu_map_read: (buffer, size) => {
            const record = get(buffer);
            record.state = -1;
            glue.track(
                record.buffer.mapAsync(GPUMapMode.READ, 0, size).then(
                    () => {
                        record.data = new Uint8Array(record.buffer.getMappedRange(0, size).slice(0));
                        record.buffer.unmap();
                        record.buffer.destroy();
                        record.state = record.data.length;
                    },
                    (e) => {
                        record.state = -2;
                        glue.log(1, `WebGPU: a read-back wasn't mapped: ${(e && e.message) || e}`);
                    },
                ),
            );
        },

        gpu_map_state: (buffer) => get(buffer).state,

        gpu_map_take: (buffer, pointer) => {
            const record = get(buffer);
            if (record.data) glue.bytes().set(record.data, pointer);
            record.data = null;
        },

        gpu_canvas_configure: (device, canvas, format, width, height) => {
            const c = get(canvas);
            if (c.width !== width) c.width = width;
            if (c.height !== height) c.height = height;
            let context = contexts.get(c);
            if (!context) {
                context = c.getContext("webgpu");
                if (!context) return 0;
                contexts.set(c, context);
            }
            context.configure({
                device: get(device),
                format: FORMATS[format],
                alphaMode: "opaque",
                // (The renderer copies from it, as from Metal's drawables.)
                usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
            });
            return 1;
        },

        gpu_canvas_texture: (canvas) => {
            const context = contexts.get(get(canvas));
            return context ? glue.add({ texture: context.getCurrentTexture(), views: new Map() }) : 0;
        },
    };

    // What a call throws is logged and counted, and the call returns 0 (none).
    for (const [name, call] of Object.entries(imports)) {
        imports[name] = (...args) => {
            try {
                return call(...args);
            } catch (e) {
                thrown++;
                glue.log(2, `WebGPU: ${name}: ${(e && e.message) || e}`);
                return 0;
            }
        };
    }
    return imports;
}
