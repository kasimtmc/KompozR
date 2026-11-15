// The device tests' color: each instance's, with red and blue swapped where the specialization constant says.
//   glslc --target-env=vulkan1.1 -O color.frag -o color.frag.spv
#version 450

layout(constant_id = 0) const int SWAP_RED_AND_BLUE = 0;

layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 outColor;

void main() {
    outColor = SWAP_RED_AND_BLUE == 1 ? v_color.bgra : v_color;
}
