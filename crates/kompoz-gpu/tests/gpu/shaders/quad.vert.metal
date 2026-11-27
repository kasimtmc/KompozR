// quad.vert in MSL, for the Metal backend (written by hand, as small as it is; the same arithmetic): each instance a
// rectangle (left, top, right, bottom in px) and a color (premultiplied), drawn as two triangles made from the vertex
// index; the texture coordinates go over the source rectangle in the uniforms (buffer 0, as every pipeline's).
// (MSL is C++: an entry point can't be called main, so it is named for what it is.)
#include <metal_stdlib>
using namespace metal;

struct Instance {
    float4 rect [[attribute(0)]];
    float4 color [[attribute(1)]];
};

struct Uniforms {
    float4 toClip;  // px to clip space: x * toClip.xy + toClip.zw
    float4 source;  // the texture coordinates at the rectangle's corners: left, top, right, bottom
};

struct Varyings {
    float4 position [[position]];
    float4 color [[user(locn0)]];
    float2 uv [[user(locn1)]];
};

vertex Varyings quad_vertex(Instance instance [[stage_in]], uint vertex_id [[vertex_id]],
                            constant Uniforms& u [[buffer(0)]]) {
    const float2 corners[6] = {float2(0, 0), float2(1, 0), float2(0, 1), float2(1, 0), float2(1, 1), float2(0, 1)};
    float2 corner = corners[vertex_id];
    float2 p = mix(instance.rect.xy, instance.rect.zw, corner);
    Varyings out;
    out.position = float4(p * u.toClip.xy + u.toClip.zw, 0, 1);
    out.color = instance.color;
    out.uv = mix(u.source.xy, u.source.zw, corner);
    return out;
}
