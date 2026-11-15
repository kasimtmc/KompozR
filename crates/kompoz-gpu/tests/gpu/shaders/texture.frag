// The device tests' texture: binding 1's texel at the texture coordinates, times the instance's alpha.
//   glslc --target-env=vulkan1.1 -O texture.frag -o texture.frag.spv
#version 450

layout(set = 0, binding = 1) uniform sampler2D source;

layout(location = 0) in vec4 v_color;
layout(location = 1) in vec2 v_uv;
layout(location = 0) out vec4 outColor;

void main() {
    outColor = texture(source, v_uv) * v_color.a;
}
