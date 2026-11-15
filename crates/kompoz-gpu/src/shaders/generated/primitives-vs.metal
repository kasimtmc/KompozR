#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<float2, int(6)> corners_0 = { { float2(0.0f, 0.0f), float2(1.0f, 0.0f), float2(1.0f, 1.0f), float2(0.0f, 0.0f), float2(1.0f, 1.0f), float2(0.0f, 1.0f) } };
struct vs_Result_0
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

[[vertex]] vs_Result_0 vs(vertexInput_0 _S1 [[stage_in]], uint vertex_0 [[vertex_id]], Uniforms_0 constant* uniforms_1 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_0;
    (&kernelContext_0)->uniforms_0 = uniforms_1;
    uint _S2 = vertex_0 % 6U;
    float2 local_2 = mix(_S1.bounds_0.xy, _S1.bounds_0.zw, corners_0[_S2]);
    float _S3 = _S1.xform_0.x;
    float _S4 = _S1.xform_0.z;
    float _S5 = _S1.xform_0.y;
    float _S6 = _S1.xform_0.w;
    float _S7 = _S1.projection_0.x;
    float _S8 = _S1.projection_0.y;
    matrix<float,int(3),int(3)>  m_0 = matrix<float,int(3),int(3)> (_S3, _S4, _S1.extra_0.x, _S5, _S6, _S1.extra_0.y, _S7, _S8, _S1.projection_0.z);
    float3 placed_0 = (((float3(local_2, 1.0f)) * (m_0)));
    float _S9 = placed_0.z;
    float3 placed_1;
    float2 local_3;
    if(_S9 > 0.0f)
    {
        float2 _S10 = float2(16.0f) ;
        float2 snapped_0 = round(placed_0.xy / float2(_S9)  * _S10) / _S10;
        float3 back_0 = cross(m_0[int(1)], m_0[int(2)]) * float3(snapped_0.x)  + cross(m_0[int(2)], m_0[int(0)]) * float3(snapped_0.y)  + cross(m_0[int(0)], m_0[int(1)]);
        float _S11 = back_0.z;
        if((abs(_S11)) > 9.99999968265522539e-21f)
        {
            float2 local_4 = back_0.xy / float2(_S11) ;
            placed_1 = (((float3(local_4, 1.0f)) * (m_0)));
            local_3 = local_4;
        }
        else
        {
            placed_1 = placed_0;
            local_3 = local_2;
        }
    }
    else
    {
        placed_1 = placed_0;
        local_3 = local_2;
    }
    thread Varyings_0 out_0;
    float _S12 = placed_1.z;
    (&out_0)->position_1 = float4(placed_1.xy * (&kernelContext_0)->uniforms_0->toClip_0.xy + float2(_S12)  * (&kernelContext_0)->uniforms_0->toClip_0.zw, 0.0f, _S12);
    (&out_0)->local_1 = local_3;
    (&out_0)->w_1 = _S12;
    int kind_0 = int(_S1.extra_0.z + 0.5f);
    bool flat_0;
    if(kind_0 == int(5))
    {
        flat_0 = true;
    }
    else
    {
        flat_0 = kind_0 == int(6);
    }
    float2 across_0;
    if(flat_0)
    {
        float2 _S13 = _S1.shape_1.xy;
        across_0 = (local_3 - _S13) / (_S1.shape_1.zw - _S13);
    }
    else
    {
        across_0 = corners_0[_S2];
    }
    float2 _S14 = _S1.uv_1.zw;
    (&out_0)->texcoord_1 = mix(_S1.uv_1.xy, _S14, across_0);
    float4 _S15 = float4(0.0f, 0.0f, 0.0f, 0.0f);
    (&out_0)->vertexColor_1 = _S15;
    (&out_0)->shape_2 = _S1.shape_1;
    (&out_0)->radii_2 = _S1.radii_1;
    (&out_0)->color_2 = _S1.color_1;
    (&out_0)->params_2 = _S1.params_1;
    (&out_0)->uv_2 = _S1.uv_1;
    (&out_0)->kindFlags_1 = _S1.extra_0.zw;
    (&out_0)->paint_2 = _S1.paint_1;
    (&out_0)->gradient_2 = _S1.gradient_1;
    (&out_0)->filter_2 = _S1.filter_1;
    float det_0 = _S3 * _S6 - _S4 * _S5;
    if(_S7 == 0.0f)
    {
        flat_0 = _S8 == 0.0f;
    }
    else
    {
        flat_0 = false;
    }
    if(flat_0)
    {
        flat_0 = (abs(det_0)) > 9.99999968265522539e-21f;
    }
    else
    {
        flat_0 = false;
    }
    float4 a_0;
    if(flat_0)
    {
        a_0 = float4(_S6, - _S5, - _S4, _S3) / float4(det_0) ;
    }
    else
    {
        a_0 = _S15;
    }
    (&out_0)->steps_1 = a_0;
    int flags_0 = int(_S1.extra_0.w + 0.5f);
    if(flat_0)
    {
        flat_0 = (_S1.filter_1.x) == 0.0f;
    }
    else
    {
        flat_0 = false;
    }
    bool plain_0;
    if(flat_0)
    {
        plain_0 = (_S1.filter_1.z) == 0.0f;
    }
    else
    {
        plain_0 = false;
    }
    bool solid_0;
    if(kind_0 == int(0))
    {
        solid_0 = int(_S1.paint_1.x + 0.5f) == int(0);
    }
    else
    {
        solid_0 = false;
    }
    bool texels_1;
    if(kind_0 == int(7))
    {
        texels_1 = (flags_0 & int(9)) == int(0);
    }
    else
    {
        texels_1 = false;
    }
    (&out_0)->inside_1 = float4(2.0f, 2.0f, 2.0f, 2.0f);
    if(plain_0)
    {
        if(solid_0)
        {
            flat_0 = true;
        }
        else
        {
            flat_0 = texels_1;
        }
    }
    else
    {
        flat_0 = false;
    }
    if(flat_0)
    {
        float2 _S16 = float2(0.5f) ;
        float2 reach_0 = _S16 * float2(abs((&out_0)->steps_1.x) + abs((&out_0)->steps_1.z), abs((&out_0)->steps_1.y) + abs((&out_0)->steps_1.w));
        float _S17 = max(max(_S1.radii_1.x, _S1.radii_1.y), max(_S1.radii_1.z, _S1.radii_1.w));
        float _S18 = reach_0.x;
        float _S19 = reach_0.y;
        float2 inA_0 = float2(max(_S18, _S17), _S19);
        float2 inB_0 = float2(_S18, max(_S19, _S17));
        float2 _S20 = _S1.shape_1.xy;
        float2 _S21 = _S1.shape_1.zw;
        float4 a_1 = float4(_S20 + inA_0, _S21 - inA_0);
        float4 b_0 = float4(_S20 + inB_0, _S21 - inB_0);
        float4 b_1;
        if(texels_1)
        {
            float2 _S22 = _S1.params_1.xy;
            float4 within_0 = float4(_S22 + _S16, _S1.params_1.zw - _S16);
            float2 _S23 = within_0.xy;
            float2 _S24 = within_0.zw;
            float4 a_2 = float4(max(a_1.xy, _S23), min(a_1.zw, _S24));
            float4 b_2 = float4(max(b_0.xy, _S23), min(b_0.zw, _S24));
            (&out_0)->texcoord_1 = (local_3 - _S22) * _S14;
            a_0 = a_2;
            b_1 = b_2;
        }
        else
        {
            a_0 = a_1;
            b_1 = b_0;
        }
        float2 _S25 = a_0.zw;
        float2 _S26 = a_0.xy;
        if(all(_S25 > _S26))
        {
            (&out_0)->inside_1.xy = (local_3 - (_S26 + _S25) * _S16) / ((_S25 - _S26) * _S16);
        }
        float2 _S27 = b_1.zw;
        float2 _S28 = b_1.xy;
        if(all(_S27 > _S28))
        {
            (&out_0)->inside_1.zw = (local_3 - (_S28 + _S27) * _S16) / ((_S27 - _S28) * _S16);
        }
    }
    Varyings_0 _S29 = out_0;
    thread vs_Result_0 _S30;
    (&_S30)->position_0 = _S29.position_1;
    (&_S30)->local_0 = _S29.local_1;
    (&_S30)->w_0 = _S29.w_1;
    (&_S30)->texcoord_0 = _S29.texcoord_1;
    (&_S30)->vertexColor_0 = _S29.vertexColor_1;
    (&_S30)->shape_0 = _S29.shape_2;
    (&_S30)->radii_0 = _S29.radii_2;
    (&_S30)->color_0 = _S29.color_2;
    (&_S30)->params_0 = _S29.params_2;
    (&_S30)->uv_0 = _S29.uv_2;
    (&_S30)->kindFlags_0 = _S29.kindFlags_1;
    (&_S30)->paint_0 = _S29.paint_2;
    (&_S30)->gradient_0 = _S29.gradient_2;
    (&_S30)->filter_0 = _S29.filter_2;
    (&_S30)->steps_0 = _S29.steps_1;
    (&_S30)->inside_0 = _S29.inside_1;
    return _S30;
}

