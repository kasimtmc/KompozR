#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<float2, int(6)> corners_0 = { { float2(0.0f, 0.0f), float2(1.0f, 0.0f), float2(1.0f, 1.0f), float2(0.0f, 0.0f), float2(1.0f, 1.0f), float2(0.0f, 1.0f) } };
struct vs_Result_0
{
    float4 position_0 [[position]];
    float4 edge_0 [[user(EDGE)]];
    float aliased_0 [[user(ALIASED)]];
};

struct vertexInput_0
{
    float4 edge_1 [[attribute(0)]];
    float4 region_0 [[attribute(1)]];
    float4 mode_0 [[attribute(2)]];
};

struct Uniforms_0
{
    float4 toClip_0;
};

struct Varyings_0
{
    float4 position_1;
    [[flat]] float4 edge_2;
    [[flat]] float aliased_1;
};

[[vertex]] vs_Result_0 vs(vertexInput_0 _S1 [[stage_in]], uint vertex_0 [[vertex_id]], Uniforms_0 constant* uniforms_0 [[buffer(0)]])
{
    uint _S2 = vertex_0 % 6U;
    float _S3 = _S1.edge_1.y;
    float _S4 = _S1.edge_1.w;
    float4 _S5 = float4(max(float2(floor(min(_S1.edge_1.x, _S1.edge_1.z)), floor(min(_S3, _S4))), _S1.region_0.xy), min(float2(_S1.region_0.z, ceil(max(_S3, _S4))), _S1.region_0.zw));
    thread float4 quad_0 = _S5;
    quad_0.zw = max(_S5.zw, _S5.xy);
    thread Varyings_0 out_0;
    (&out_0)->position_1 = float4(mix(quad_0.xy, quad_0.zw, corners_0[_S2]) * uniforms_0->toClip_0.xy + uniforms_0->toClip_0.zw, 0.0f, 1.0f);
    (&out_0)->edge_2 = _S1.edge_1;
    (&out_0)->aliased_1 = _S1.mode_0.x;
    thread vs_Result_0 _S6;
    (&_S6)->position_0 = out_0.position_1;
    (&_S6)->edge_0 = out_0.edge_2;
    (&_S6)->aliased_0 = out_0.aliased_1;
    return _S6;
}

