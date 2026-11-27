// The device tests' vertex shader on WebGPU (quad.vert's): each instance a rectangle (left, top, right, bottom in
// px) and a color (premultiplied), drawn as two triangles made from the vertex index; the texture coordinates go over
// the source rectangle in the uniforms (binding 0, as the device binds every pipeline's).
struct Uniforms {
    toClip : vec4f, // px to clip space: x * toClip.xy + toClip.zw
    source : vec4f, // the texture coordinates at the rectangle's corners: left, top, right, bottom
};
@group(0) @binding(0) var<uniform> u : Uniforms;

struct Varyings {
    @builtin(position) position : vec4f,
    @location(0) color : vec4f,
    @location(1) uv : vec2f,
};

@vertex fn main(@builtin(vertex_index) i : u32, @location(0) rect : vec4f, @location(1) color : vec4f) -> Varyings {
    var corners = array<vec2f, 6>(vec2f(0, 0), vec2f(1, 0), vec2f(0, 1), vec2f(1, 0), vec2f(1, 1), vec2f(0, 1));
    let corner = corners[i];
    let p = mix(rect.xy, rect.zw, corner);
    var out : Varyings;
    out.position = vec4f(p * u.toClip.xy + u.toClip.zw, 0, 1);
    out.color = color;
    out.uv = mix(u.source.xy, u.source.zw, corner);
    return out;
}
