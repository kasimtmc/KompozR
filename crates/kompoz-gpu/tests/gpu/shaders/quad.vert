// The device tests' vertex shader: each instance a rectangle (left, top, right, bottom in px) and a color
// (premultiplied), drawn as two triangles made from the vertex index; the texture coordinates go over the source
// rectangle in the uniforms. Compiled once, its SPIR-V committed beside it:
//   glslc --target-env=vulkan1.1 -O quad.vert -o quad.vert.spv
#version 450

layout(location = 0) in vec4 rect;
layout(location = 1) in vec4 color;

layout(push_constant) uniform Uniforms {
    vec4 toClip;  // px to clip space: x * toClip.xy + toClip.zw
    vec4 source;  // the texture coordinates at the rectangle's corners: left, top, right, bottom
} u;

layout(location = 0) out vec4 v_color;
layout(location = 1) out vec2 v_uv;

void main() {
    const vec2 corners[6] = vec2[](vec2(0, 0), vec2(1, 0), vec2(0, 1), vec2(1, 0), vec2(1, 1), vec2(0, 1));
    vec2 corner = corners[gl_VertexIndex];
    vec2 p = mix(rect.xy, rect.zw, corner);
    gl_Position = vec4(p * u.toClip.xy + u.toClip.zw, 0, 1);
    v_color = color;
    v_uv = mix(u.source.xy, u.source.zw, corner);
}
