// texture.frag in MSL, for the Metal backend: slot 0's texel at the texture coordinates (its texture and sampler at
// index 0, as Metal binds slot i; binding 1 on Vulkan), times the instance's alpha.
#include <metal_stdlib>
using namespace metal;

struct Varyings {
    float4 color [[user(locn0)]];
    float2 uv [[user(locn1)]];
};

fragment float4 texture_fragment(Varyings in [[stage_in]], texture2d<float> source [[texture(0)]],
                                 sampler sourceSampler [[sampler(0)]]) {
    return source.sample(sourceSampler, in.uv) * in.color.a;
}
