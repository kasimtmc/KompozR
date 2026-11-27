// color.frag in MSL, for the Metal backend: each instance's color, with red and blue swapped where the
// specialization constant (function constant 0) says.
#include <metal_stdlib>
using namespace metal;

constant int swapConstant [[function_constant(0)]];
constant int SWAP_RED_AND_BLUE = is_function_constant_defined(swapConstant) ? swapConstant : 0;

struct Varyings {
    float4 color [[user(locn0)]];
};

fragment float4 color_fragment(Varyings in [[stage_in]]) {
    return SWAP_RED_AND_BLUE == 1 ? in.color.bgra : in.color;
}
