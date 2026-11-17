#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<float2, int(6)> corners_0 = { { float2(0.0f, 0.0f), float2(1.0f, 0.0f), float2(1.0f, 1.0f), float2(0.0f, 0.0f), float2(1.0f, 1.0f), float2(0.0f, 1.0f) } };
struct vs_Result_0
{
    float4 position_0 [[position]];
    float2 texel_0 [[user(TEXEL)]];
};

struct vertexInput_0
{
    float4 rect_0 [[attribute(0)]];
};

struct Uniforms_0
{
    float4 toClip_0;
    float4 local_0;
    float4 source_0;
    float4 edges_0;
    float uMaxStretchIntensity_0;
    float uStretchAffectedDistX_0;
    float uStretchAffectedDistY_0;
    float uDistanceStretchedX_0;
    float uDistanceStretchedY_0;
    float uInverseDistanceStretchedX_0;
    float uInverseDistanceStretchedY_0;
    float uDistDiffX_0;
    float uDistDiffY_0;
    float uScrollX_0;
    float uScrollY_0;
    float uOverscrollX_0;
    float uOverscrollY_0;
    float viewportWidth_0;
    float viewportHeight_0;
    float uInterpolationStrength_0;
};

struct Varyings_0
{
    float4 position_1;
    float2 texel_1;
};

[[vertex]] vs_Result_0 vs(vertexInput_0 _S1 [[stage_in]], uint vertex_0 [[vertex_id]], Uniforms_0 constant* uniforms_0 [[buffer(0)]])
{
    float2 p_0 = mix(_S1.rect_0.xy, _S1.rect_0.zw, corners_0[vertex_0 % 6U]);
    thread Varyings_0 out_0;
    (&out_0)->position_1 = float4(p_0 * uniforms_0->toClip_0.xy + uniforms_0->toClip_0.zw, 0.0f, 1.0f);
    (&out_0)->texel_1 = p_0;
    thread vs_Result_0 _S2;
    (&_S2)->position_0 = out_0.position_1;
    (&_S2)->texel_0 = out_0.texel_1;
    return _S2;
}

