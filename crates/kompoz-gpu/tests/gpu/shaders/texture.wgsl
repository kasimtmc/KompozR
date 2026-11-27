// The device tests' texture on WebGPU (texture.frag's): binding 1's texel at the texture coordinates (its sampler at
// 2), times the instance's alpha.
@group(0) @binding(1) var source : texture_2d<f32>;
@group(0) @binding(2) var sourceSampler : sampler;

@fragment fn main(@location(0) color : vec4f, @location(1) uv : vec2f) -> @location(0) vec4f {
    return textureSample(source, sourceSampler, uv) * color.a;
}
