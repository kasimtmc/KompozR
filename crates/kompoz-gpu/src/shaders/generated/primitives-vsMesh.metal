#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct vsMesh_Result_0
{
    float4 position_0 [[position]];
    float2 local_0 [[user(LOCAL)]];
    float w_0 [[user(DEPTH_W)]];
    float2 texcoord_0 [[user(TEXCOORD)]];
    float4 vertexColor_0 [[user(VERTEXCOLOR)]];
    float4 shape_0 [[user(SHAPE)]];
    float4 radii_0 [[user(RADII)]];
    float4 color_0 [[user(COLOR)]];
    float4 params_0 [[user(PARAMS)]];
    float4 uv_0 [[user(UV)]];
    float2 kindFlags_0 [[user(KIND)]];
    float4 paint_0 [[user(PAINT)]];
    float4 gradient_0 [[user(GRADIENT)]];
    float4 filter_0 [[user(FILTER)]];
    float4 steps_0 [[user(STEPS)]];
    float4 inside_0 [[user(INSIDE)]];
};

struct vertexInput_0
{
    float4 bounds_0 [[attribute(0)]];
    float4 shape_1 [[attribute(1)]];
    float4 radii_1 [[attribute(2)]];
    float4 color_1 [[attribute(3)]];
    float4 params_1 [[attribute(4)]];
    float4 uv_1 [[attribute(5)]];
    float4 xform_0 [[attribute(6)]];
    float4 extra_0 [[attribute(7)]];
    float4 paint_1 [[attribute(8)]];
    float4 gradient_1 [[attribute(9)]];
    float4 filter_1 [[attribute(10)]];
    float4 projection_0 [[attribute(11)]];
};

struct Uniforms_0
{
    float4 toClip_0;
    float4 texels_0;
    float4 blend_0;
};

struct KernelContext_0
{
    Uniforms_0 constant* uniforms_0;
};

struct Varyings_0
{
    float4 position_1;
    float2 local_1;
    float w_1;
    float2 texcoord_1;
    float4 vertexColor_1;
    [[flat]] float4 shape_2;
    [[flat]] float4 radii_2;
    [[flat]] float4 color_2;
    [[flat]] float4 params_2;
    [[flat]] float4 uv_2;
    [[flat]] float2 kindFlags_1;
    [[flat]] float4 paint_2;
    [[flat]] float4 gradient_2;
    [[flat]] float4 filter_2;
    [[flat]] float4 steps_1;
    float4 inside_1;
};

[[vertex]] vsMesh_Result_0 vsMesh(vertexInput_0 _S1 [[stage_in]], uint vertex_0 [[vertex_id]], Uniforms_0 constant* uniforms_1 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_0;
    (&kernelContext_0)->uniforms_0 = uniforms_1;
    int i_0 = int(vertex_0 % 3U);
    bool _S2 = i_0 == int(0);
    float3 placed_0;
    if(_S2)
    {
        placed_0 = _S1.bounds_0.xyz;
    }
    else
    {
        if(i_0 == int(1))
        {
            placed_0 = float3(_S1.bounds_0.w, _S1.shape_1.xy);
        }
        else
        {
            placed_0 = float3(_S1.shape_1.zw, _S1.radii_1.x);
        }
    }
    float2 texcoord_2;
    if(_S2)
    {
        texcoord_2 = _S1.radii_1.yz;
    }
    else
    {
        if(i_0 == int(1))
        {
            texcoord_2 = float2(_S1.radii_1.w, _S1.uv_1.x);
        }
        else
        {
            texcoord_2 = _S1.uv_1.yz;
        }
    }
    thread Varyings_0 out_0;
    float _S3 = placed_0.z;
    (&out_0)->position_1 = float4(placed_0.xy * (&kernelContext_0)->uniforms_0->toClip_0.xy + float2(_S3)  * (&kernelContext_0)->uniforms_0->toClip_0.zw, 0.0f, _S3);
    (&out_0)->local_1 = float2(0.0f, 0.0f);
    (&out_0)->w_1 = _S3;
    (&out_0)->texcoord_1 = texcoord_2;
    float4 _S4;
    if(_S2)
    {
        _S4 = _S1.xform_0;
    }
    else
    {
        if(i_0 == int(1))
        {
            _S4 = _S1.extra_0;
        }
        else
        {
            _S4 = _S1.projection_0;
        }
    }
    (&out_0)->vertexColor_1 = _S4;
    float4 _S5 = float4(0.0f, 0.0f, 0.0f, 0.0f);
    (&out_0)->shape_2 = _S5;
    (&out_0)->radii_2 = _S5;
    (&out_0)->color_2 = _S1.color_1;
    (&out_0)->params_2 = _S1.params_1;
    (&out_0)->uv_2 = _S5;
    (&out_0)->kindFlags_1 = float2(10.0f, 0.0f);
    (&out_0)->paint_2 = _S1.paint_1;
    (&out_0)->gradient_2 = _S1.gradient_1;
    (&out_0)->filter_2 = _S1.filter_1;
    (&out_0)->steps_1 = _S5;
    (&out_0)->inside_1 = float4(2.0f, 2.0f, 2.0f, 2.0f);
    Varyings_0 _S6 = out_0;
    thread vsMesh_Result_0 _S7;
    (&_S7)->position_0 = _S6.position_1;
    (&_S7)->local_0 = _S6.local_1;
    (&_S7)->w_0 = _S6.w_1;
    (&_S7)->texcoord_0 = _S6.texcoord_1;
    (&_S7)->vertexColor_0 = _S6.vertexColor_1;
    (&_S7)->shape_0 = _S6.shape_2;
    (&_S7)->radii_0 = _S6.radii_2;
    (&_S7)->color_0 = _S6.color_2;
    (&_S7)->params_0 = _S6.params_2;
    (&_S7)->uv_0 = _S6.uv_2;
    (&_S7)->kindFlags_0 = _S6.kindFlags_1;
    (&_S7)->paint_0 = _S6.paint_2;
    (&_S7)->gradient_0 = _S6.gradient_2;
    (&_S7)->filter_0 = _S6.filter_2;
    (&_S7)->steps_0 = _S6.steps_1;
    (&_S7)->inside_0 = _S6.inside_1;
    return _S7;
}

