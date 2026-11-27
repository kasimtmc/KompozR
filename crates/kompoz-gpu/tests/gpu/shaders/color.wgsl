// The device tests' color on WebGPU (color.frag's): each instance's, with red and blue swapped where the constant
// says.
@id(0) override SWAP_RED_AND_BLUE : i32 = 0;

@fragment fn main(@location(0) color : vec4f) -> @location(0) vec4f {
    if (SWAP_RED_AND_BLUE == 1) {
        return color.bgra;
    }
    return color;
}
