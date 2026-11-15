diagnostic(off, derivative_uniformity);
struct Uniforms_std430_0
{
    @align(16) toClip_0 : vec4<f32>,
    @align(16) map_0 : vec4<f32>,
    @align(16) valid_0 : vec4<f32>,
    @align(16) params_0 : vec4<f32>,
    @align(16) extra_0 : vec4<f32>,
    @align(16) mask_0 : vec4<f32>,
    @align(16) geometry_0 : vec4<f32>,
    @align(16) origin_0 : vec4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms_0 : Uniforms_std430_0;
var<private> corners_0 : array<vec2<f32>, i32(6)> = array<vec2<f32>, i32(6)>( vec2<f32>(0.0f, 0.0f), vec2<f32>(1.0f, 0.0f), vec2<f32>(1.0f, 1.0f), vec2<f32>(0.0f, 0.0f), vec2<f32>(1.0f, 1.0f), vec2<f32>(0.0f, 1.0f) );
struct Varyings_0
{
    @builtin(position) position_0 : vec4<f32>,
    @location(0) texel_0 : vec2<f32>,
};

struct vertexInput_0
{
    @location(0) rect_0 : vec4<f32>,
};

@vertex
fn vs( _S1 : vertexInput_0, @builtin(vertex_index) vertex_0 : u32) -> Varyings_0
{
    var p_0 : vec2<f32> = mix(_S1.rect_0.xy, _S1.rect_0.zw, corners_0[vertex_0 % u32(6)]);
    var out_0 : Varyings_0;
    out_0.position_0 = vec4<f32>(p_0 * uniforms_0.toClip_0.xy + uniforms_0.toClip_0.zw, 0.0f, 1.0f);
    out_0.texel_0 = p_0;
    return out_0;
}

