#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant int fc_FILTERED_0 [[function_constant(3)]];
constant int FILTERED_0 = is_function_constant_defined(fc_FILTERED_0) ? fc_FILTERED_0 : int(1);
constant int fc_CLIPPED_0 [[function_constant(2)]];
constant int CLIPPED_0 = is_function_constant_defined(fc_CLIPPED_0) ? fc_CLIPPED_0 : int(1);
constant int fc_PAINTS_0 [[function_constant(1)]];
constant int PAINTS_0 = is_function_constant_defined(fc_PAINTS_0) ? fc_PAINTS_0 : int(63);
constant int fc_FLAGS_0 [[function_constant(5)]];
constant int FLAGS_0 = is_function_constant_defined(fc_FLAGS_0) ? fc_FLAGS_0 : int(255);
constant int fc_HOW_0 [[function_constant(4)]];
constant int HOW_0 = is_function_constant_defined(fc_HOW_0) ? fc_HOW_0 : int(-1);
constant int fc_KINDS_0 [[function_constant(0)]];
constant int KINDS_0 = is_function_constant_defined(fc_KINDS_0) ? fc_KINDS_0 : int(2047);
bool drawn_0(int kind_0)
{
    return (KINDS_0 & (int(1) << uint(kind_0))) != int(0);
}

float sat_0(float3 c_0)
{
    float _S1 = c_0.x;
    float _S2 = c_0.y;
    float _S3 = c_0.z;
    return max(_S1, max(_S2, _S3)) - min(_S1, min(_S2, _S3));
}

float3 setSat_0(float3 c_1, float s_0)
{
    float _S4 = c_1.x;
    float _S5 = c_1.y;
    float _S6 = c_1.z;
    float _S7 = min(_S4, min(_S5, _S6));
    float _S8 = max(_S4, max(_S5, _S6));
    float3 _S9;
    if(_S8 > _S7)
    {
        _S9 = (c_1 - float3(_S7) ) * float3(s_0)  / float3((_S8 - _S7)) ;
    }
    else
    {
        _S9 = float3(0.0f, 0.0f, 0.0f);
    }
    return _S9;
}

float lum_0(float3 c_2)
{
    return dot(c_2, float3(0.30000001192092896f, 0.5899999737739563f, 0.10999999940395355f));
}

float3 clipColor_0(float3 c_3)
{
    float l_0 = lum_0(c_3);
    float _S10 = c_3.x;
    float _S11 = c_3.y;
    float _S12 = c_3.z;
    float _S13 = min(_S10, min(_S11, _S12));
    float _S14 = max(_S10, max(_S11, _S12));
    bool _S15;
    if(_S13 < 0.0f)
    {
        _S15 = (l_0 - _S13) > 0.0f;
    }
    else
    {
        _S15 = false;
    }
    float3 out_0;
    if(_S15)
    {
        float3 _S16 = float3(l_0) ;
        out_0 = _S16 + (c_3 - _S16) * _S16 / float3((l_0 - _S13)) ;
    }
    else
    {
        out_0 = c_3;
    }
    if(_S14 > 1.0f)
    {
        _S15 = (_S14 - l_0) > 0.0f;
    }
    else
    {
        _S15 = false;
    }
    if(_S15)
    {
        float3 _S17 = float3(l_0) ;
        out_0 = _S17 + (out_0 - _S17) * float3((1.0f - l_0))  / float3((_S14 - l_0)) ;
    }
    return out_0;
}

float3 setLum_0(float3 c_4, float l_1)
{
    return clipColor_0(c_4 + float3((l_1 - lum_0(c_4))) );
}

float hardLight_0(float s_1, float b_0)
{
    float _S18;
    if(s_1 <= 0.5f)
    {
        _S18 = b_0 * 2.0f * s_1;
    }
    else
    {
        float _S19 = 2.0f * s_1 - 1.0f;
        _S18 = b_0 + _S19 - b_0 * _S19;
    }
    return _S18;
}

float separable_0(int mode_0, float cs_0, float cb_0)
{
    float r_0;
    if(mode_0 == int(6))
    {
        r_0 = cs_0 * cb_0;
    }
    else
    {
        if(mode_0 == int(7))
        {
            r_0 = cs_0 + cb_0 - cs_0 * cb_0;
        }
        else
        {
            if(mode_0 == int(8))
            {
                r_0 = min(cs_0, cb_0);
            }
            else
            {
                if(mode_0 == int(9))
                {
                    r_0 = max(cs_0, cb_0);
                }
                else
                {
                    if(mode_0 == int(18))
                    {
                        r_0 = hardLight_0(cb_0, cs_0);
                    }
                    else
                    {
                        if(mode_0 == int(19))
                        {
                            if(cb_0 <= 0.0f)
                            {
                                r_0 = 0.0f;
                            }
                            else
                            {
                                if(cs_0 >= 1.0f)
                                {
                                    r_0 = 1.0f;
                                }
                                else
                                {
                                    r_0 = min(1.0f, cb_0 / (1.0f - cs_0));
                                }
                            }
                        }
                        else
                        {
                            if(mode_0 == int(20))
                            {
                                if(cb_0 >= 1.0f)
                                {
                                    r_0 = 1.0f;
                                }
                                else
                                {
                                    if(cs_0 <= 0.0f)
                                    {
                                        r_0 = 0.0f;
                                    }
                                    else
                                    {
                                        r_0 = 1.0f - min(1.0f, (1.0f - cb_0) / cs_0);
                                    }
                                }
                            }
                            else
                            {
                                if(mode_0 == int(21))
                                {
                                    r_0 = hardLight_0(cs_0, cb_0);
                                }
                                else
                                {
                                    if(mode_0 == int(22))
                                    {
                                        if(cb_0 <= 0.25f)
                                        {
                                            r_0 = ((16.0f * cb_0 - 12.0f) * cb_0 + 4.0f) * cb_0;
                                        }
                                        else
                                        {
                                            r_0 = sqrt(cb_0);
                                        }
                                        if(cs_0 <= 0.5f)
                                        {
                                            r_0 = cb_0 - (1.0f - 2.0f * cs_0) * cb_0 * (1.0f - cb_0);
                                        }
                                        else
                                        {
                                            r_0 = cb_0 + (2.0f * cs_0 - 1.0f) * (r_0 - cb_0);
                                        }
                                    }
                                    else
                                    {
                                        if(mode_0 == int(23))
                                        {
                                            r_0 = abs(cs_0 - cb_0);
                                        }
                                        else
                                        {
                                            if(mode_0 == int(24))
                                            {
                                                r_0 = cs_0 + cb_0 - 2.0f * cs_0 * cb_0;
                                            }
                                            else
                                            {
                                                r_0 = cs_0;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return r_0;
}

float4 blendColors_0(float4 s_2, float4 d_0, int mode_1)
{
    float sa_0 = s_2.w;
    float da_0 = d_0.w;
    float3 cs_1;
    if(sa_0 > 0.0f)
    {
        cs_1 = s_2.xyz / float3(sa_0) ;
    }
    else
    {
        cs_1 = float3(0.0f, 0.0f, 0.0f);
    }
    float3 cb_1;
    if(da_0 > 0.0f)
    {
        cb_1 = d_0.xyz / float3(da_0) ;
    }
    else
    {
        cb_1 = float3(0.0f, 0.0f, 0.0f);
    }
    float fs_0;
    float fd_0;
    if(mode_1 == int(11))
    {
        fs_0 = 0.0f;
        fd_0 = 0.0f;
    }
    else
    {
        if(mode_1 == int(0))
        {
            fs_0 = 1.0f;
            fd_0 = 0.0f;
        }
        else
        {
            if(mode_1 == int(12))
            {
                fs_0 = 0.0f;
                fd_0 = 1.0f;
            }
            else
            {
                if(mode_1 == int(1))
                {
                    float _S20 = 1.0f - sa_0;
                    fs_0 = 1.0f;
                    fd_0 = _S20;
                }
                else
                {
                    if(mode_1 == int(13))
                    {
                        fs_0 = 1.0f - da_0;
                        fd_0 = 1.0f;
                    }
                    else
                    {
                        if(mode_1 == int(2))
                        {
                            fs_0 = da_0;
                            fd_0 = 0.0f;
                        }
                        else
                        {
                            if(mode_1 == int(4))
                            {
                                fs_0 = 0.0f;
                                fd_0 = sa_0;
                            }
                            else
                            {
                                if(mode_1 == int(14))
                                {
                                    fs_0 = 1.0f - da_0;
                                    fd_0 = 0.0f;
                                }
                                else
                                {
                                    if(mode_1 == int(15))
                                    {
                                        float _S21 = 1.0f - sa_0;
                                        fs_0 = 0.0f;
                                        fd_0 = _S21;
                                    }
                                    else
                                    {
                                        if(mode_1 == int(3))
                                        {
                                            float _S22 = 1.0f - sa_0;
                                            fs_0 = da_0;
                                            fd_0 = _S22;
                                        }
                                        else
                                        {
                                            if(mode_1 == int(16))
                                            {
                                                fs_0 = 1.0f - da_0;
                                                fd_0 = sa_0;
                                            }
                                            else
                                            {
                                                if(mode_1 == int(17))
                                                {
                                                    float _S23 = 1.0f - sa_0;
                                                    fs_0 = 1.0f - da_0;
                                                    fd_0 = _S23;
                                                }
                                                else
                                                {
                                                    fs_0 = -1.0f;
                                                    fd_0 = 0.0f;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    float4 out_1;
    if(fs_0 >= 0.0f)
    {
        out_1 = s_2 * float4(fs_0)  + d_0 * float4(fd_0) ;
    }
    else
    {
        if(mode_1 == int(10))
        {
            out_1 = min(s_2 + d_0, float4(1.0f, 1.0f, 1.0f, 1.0f));
        }
        else
        {
            if(mode_1 == int(5))
            {
                out_1 = s_2 * d_0;
            }
            else
            {
                float3 b_1;
                if(mode_1 == int(25))
                {
                    b_1 = setLum_0(setSat_0(cs_1, sat_0(cb_1)), lum_0(cb_1));
                }
                else
                {
                    if(mode_1 == int(26))
                    {
                        b_1 = setLum_0(setSat_0(cb_1, sat_0(cs_1)), lum_0(cb_1));
                    }
                    else
                    {
                        if(mode_1 == int(27))
                        {
                            b_1 = setLum_0(cs_1, lum_0(cb_1));
                        }
                        else
                        {
                            if(mode_1 == int(28))
                            {
                                b_1 = setLum_0(cb_1, lum_0(cs_1));
                            }
                            else
                            {
                                b_1 = float3(separable_0(mode_1, cs_1.x, cb_1.x), separable_0(mode_1, cs_1.y, cb_1.y), separable_0(mode_1, cs_1.z, cb_1.z));
                            }
                        }
                    }
                }
                float _S24 = sa_0 * da_0;
                out_1 = float4(s_2.xyz * float3((1.0f - da_0))  + d_0.xyz * float3((1.0f - sa_0))  + float3(_S24)  * b_1, sa_0 + da_0 - _S24);
            }
        }
    }
    return out_1;
}

struct Uniforms_0
{
    float4 toClip_0;
    float4 texels_0;
    float4 blend_0;
};

struct KernelContext_0
{
    texture2d<float, access::sample> texture0_texture_0;
    sampler texture0_sampler_0;
    Uniforms_0 constant* uniforms_0;
    texture2d<float, access::sample> texture3_texture_0;
    sampler texture3_sampler_0;
    texture2d<float, access::sample> texture2_texture_0;
    sampler texture2_sampler_0;
    texture2d<float, access::sample> texture5_texture_0;
    sampler texture5_sampler_0;
    texture2d<float, access::sample> texture1_texture_0;
    sampler texture1_sampler_0;
    texture2d<float, access::sample> texture4_texture_0;
    sampler texture4_sampler_0;
};

float4 finish_0(float4 src_0, float cover_0, float2 pixel_0, KernelContext_0 thread* kernelContext_0)
{
    int how_0;
    if(HOW_0 >= int(0))
    {
        how_0 = HOW_0;
    }
    else
    {
        how_0 = int(kernelContext_0->uniforms_0->blend_0.y + 0.5f);
    }
    float4 _S25 = float4(cover_0) ;
    float4 _S26 = src_0 * _S25;
    float4 out_2;
    if(how_0 == int(1))
    {
        out_2 = mix(float4(1.0f, 1.0f, 1.0f, 1.0f), src_0, _S25);
    }
    else
    {
        if(how_0 == int(2))
        {
            out_2 = float4(0.0f, 0.0f, 0.0f, mix(1.0f, src_0.w, cover_0));
        }
        else
        {
            if(how_0 == int(3))
            {
                out_2 = float4(0.0f, 0.0f, 0.0f, cover_0);
            }
            else
            {
                if(how_0 == int(4))
                {
                    float4 _S27 = ((kernelContext_0->texture3_texture_0).sample((kernelContext_0->texture3_sampler_0), (pixel_0 * kernelContext_0->uniforms_0->blend_0.zw), level((0.0f))));
                    out_2 = mix(_S27, blendColors_0(src_0, _S27, int(kernelContext_0->uniforms_0->blend_0.x + 0.5f)), _S25);
                }
                else
                {
                    out_2 = _S26;
                }
            }
        }
    }
    return out_2;
}

bool paints_0(int paint_0)
{
    return (PAINTS_0 & (int(1) << uint(paint_0))) != int(0);
}

float tableWidth_0(KernelContext_0 thread* kernelContext_1)
{
    return round(1.0f / kernelContext_1->uniforms_0->texels_0.z);
}

float4 tableTexel_0(float x_0, float row_0, KernelContext_0 thread* kernelContext_2)
{
    float4 _S28 = ((kernelContext_2->texture2_texture_0).read(uint2((int2(int(x_0 + 0.5f), int(row_0 + 0.5f))))));
    return _S28;
}

float4 tableTexel_1(float x_1, float row_1, KernelContext_0 thread* kernelContext_3)
{
    float4 _S29 = ((kernelContext_3->texture2_texture_0).read(uint2((int2(int(x_1 + 0.5f), int(row_1 + 0.5f))))));
    return _S29;
}

float4 paintOne_0(float2 p_0, float2 gx_0, float2 gy_0, float4 color_0, float4 paint_1, float4 g_0, bool second_0, KernelContext_0 thread* kernelContext_4)
{
    int kind_1 = int(paint_1.x + 0.5f);
    bool found_0;
    if(kind_1 == int(0))
    {
        found_0 = true;
    }
    else
    {
        found_0 = (PAINTS_0 & int(-2)) == int(0);
    }
    if(found_0)
    {
        return color_0;
    }
    int bits_0 = int(paint_1.y + 0.5f);
    float row_2 = paint_1.w;
    float2 _S30;
    float2 _S31;
    float2 _S32;
    if((bits_0 & int(8)) != int(0))
    {
        float3 h_0 = float3(p_0, 1.0f);
        float _S33 = tableWidth_0(kernelContext_4);
        float last_0 = _S33 - 1.0f;
        float4 _S34 = tableTexel_0(last_0 - 2.0f, row_2, kernelContext_4);
        float3 r0_0 = _S34.xyz;
        float4 _S35 = tableTexel_0(last_0 - 1.0f, row_2, kernelContext_4);
        float3 r1_0 = _S35.xyz;
        float4 _S36 = tableTexel_0(last_0, row_2, kernelContext_4);
        float3 r2_0 = _S36.xyz;
        float2 _S37 = float2(dot(r2_0, h_0)) ;
        float2 _S38 = float2(dot(r0_0, h_0), dot(r1_0, h_0)) / _S37;
        float2 _S39 = r2_0.xy;
        float2 jx_0 = (r0_0.xy - float2(_S38.x)  * _S39) / _S37;
        float2 jy_0 = (r1_0.xy - float2(_S38.y)  * _S39) / _S37;
        float2 _S40 = float2(dot(jx_0, gx_0), dot(jy_0, gx_0));
        float2 _S41 = float2(dot(jx_0, gy_0), dot(jy_0, gy_0));
        _S30 = _S38;
        _S31 = _S40;
        _S32 = _S41;
    }
    else
    {
        _S30 = p_0;
        _S31 = gx_0;
        _S32 = gy_0;
    }
    if(paints_0(int(4)))
    {
        found_0 = kind_1 == int(4);
    }
    else
    {
        found_0 = false;
    }
    float t_0;
    float before_0;
    float4 texel_0;
    if(found_0)
    {
        if(second_0)
        {
            texel_0 = ((kernelContext_4->texture5_texture_0).sample((kernelContext_4->texture5_sampler_0), (_S30), gradient2d((_S31), (_S32))));
        }
        else
        {
            texel_0 = ((kernelContext_4->texture1_texture_0).sample((kernelContext_4->texture1_sampler_0), (_S30), gradient2d((_S31), (_S32))));
        }
        if((bits_0 & int(48)) != int(0))
        {
            thread float2 size_0;
            if(second_0)
            {
                thread float _S42 = size_0.x;
                thread float _S43 = size_0.y;
                texture2d<float, access::sample> _S44 = kernelContext_4->texture5_texture_0;
                (*((&_S42)) = (_S44).get_width(0)),(*((&_S43)) = (_S44).get_height(0));
                size_0.y = _S43;
                size_0.x = _S42;
            }
            else
            {
                thread float _S45 = size_0.x;
                thread float _S46 = size_0.y;
                texture2d<float, access::sample> _S47 = kernelContext_4->texture1_texture_0;
                (*((&_S45)) = (_S47).get_width(0)),(*((&_S46)) = (_S47).get_height(0));
                size_0.y = _S46;
                size_0.x = _S45;
            }
            float2 at_0 = _S30 * size_0;
            float2 fade_0 = saturate(float2(1.0f)  - abs(at_0 - clamp(at_0, float2(0.5f) , size_0 - float2(0.5f) )));
            if((bits_0 & int(16)) != int(0))
            {
                t_0 = fade_0.x;
            }
            else
            {
                t_0 = 1.0f;
            }
            if((bits_0 & int(32)) != int(0))
            {
                before_0 = fade_0.y;
            }
            else
            {
                before_0 = 1.0f;
            }
            texel_0 = texel_0 * float4((t_0 * before_0)) ;
        }
        return texel_0 * float4(color_0.w) ;
    }
    if(!paints_0(int(1)))
    {
        found_0 = !paints_0(int(2));
    }
    else
    {
        found_0 = false;
    }
    if(found_0)
    {
        found_0 = !paints_0(int(3));
    }
    else
    {
        found_0 = false;
    }
    if(found_0)
    {
        return color_0;
    }
    if(kind_1 == int(1))
    {
        float2 _S48 = g_0.xy;
        float2 d_1 = g_0.zw - _S48;
        float len2_0 = dot(d_1, d_1);
        if(len2_0 > 0.0f)
        {
            t_0 = dot(_S30 - _S48, d_1) / len2_0;
        }
        else
        {
            t_0 = 0.0f;
        }
    }
    else
    {
        if(kind_1 == int(2))
        {
            float _S49 = g_0.z;
            if(_S49 > 0.0f)
            {
                t_0 = length(_S30 - g_0.xy) / _S49;
            }
            else
            {
                t_0 = 0.0f;
            }
        }
        else
        {
            float a_0 = atan2(_S30.y - g_0.y, _S30.x - g_0.x);
            if(a_0 < 0.0f)
            {
                t_0 = a_0 + 6.2831854820251465f;
            }
            else
            {
                t_0 = a_0;
            }
            t_0 = t_0 / 6.2831854820251465f;
        }
    }
    int tile_0 = bits_0 & int(7);
    bool _S50 = tile_0 == int(3);
    if(_S50)
    {
        if(t_0 < 0.0f)
        {
            found_0 = true;
        }
        else
        {
            found_0 = t_0 > 1.0f;
        }
    }
    else
    {
        found_0 = false;
    }
    int i_0;
    if(found_0)
    {
        i_0 = int(0);
    }
    else
    {
        i_0 = int(1);
    }
    float shown_0 = float(i_0);
    if(tile_0 == int(0))
    {
        found_0 = true;
    }
    else
    {
        found_0 = _S50;
    }
    if(found_0)
    {
        t_0 = saturate(t_0);
    }
    else
    {
        if(tile_0 == int(1))
        {
            t_0 = t_0 - floor(t_0);
        }
        else
        {
            float _S51 = abs(t_0);
            float t_1 = _S51 - 2.0f * floor(_S51 / 2.0f);
            if(t_1 > 1.0f)
            {
                t_0 = 2.0f - t_1;
            }
            else
            {
                t_0 = t_1;
            }
        }
    }
    int _S52 = int(paint_1.z + 0.5f);
    float4 _S53 = tableTexel_0(0.0f, row_2, kernelContext_4);
    float4 _S54 = tableTexel_0(1.0f, row_2, kernelContext_4);
    float before_1 = _S54.x;
    found_0 = t_0 <= before_1;
    before_0 = before_1;
    texel_0 = _S53;
    float4 straight_0 = _S53;
    i_0 = int(1);
    for(;;)
    {
        if(i_0 < _S52)
        {
        }
        else
        {
            break;
        }
        float _S55 = float(i_0) * 2.0f;
        float4 _S56 = tableTexel_0(_S55, row_2, kernelContext_4);
        float4 _S57 = tableTexel_0(_S55 + 1.0f, row_2, kernelContext_4);
        float offset_0 = _S57.x;
        bool _S58;
        if(!found_0)
        {
            _S58 = t_0 <= offset_0;
        }
        else
        {
            _S58 = false;
        }
        if(_S58)
        {
            float span_0 = offset_0 - before_0;
            float _S59;
            if(span_0 > 0.0f)
            {
                _S59 = (t_0 - before_0) / span_0;
            }
            else
            {
                _S59 = 1.0f;
            }
            float4 _S60 = mix(texel_0, _S56, float4(_S59) );
            found_0 = true;
            straight_0 = _S60;
        }
        int i_1 = i_0 + int(1);
        before_0 = offset_0;
        texel_0 = _S56;
        i_0 = i_1;
    }
    if(found_0)
    {
    }
    else
    {
        straight_0 = texel_0;
    }
    float _S61 = straight_0.w;
    return float4(straight_0.xyz * float3(_S61) , _S61) * float4(color_0.w)  * float4(shown_0) ;
}

float4 paintAt_0(float2 p_1, float2 gx_1, float2 gy_1, float4 color_1, float4 paint_2, float4 g_1, KernelContext_0 thread* kernelContext_5)
{
    bool _S62;
    if(!paints_0(int(5)))
    {
        _S62 = true;
    }
    else
    {
        _S62 = int(paint_2.x + 0.5f) != int(5);
    }
    if(_S62)
    {
        float4 _S63 = paintOne_0(p_1, gx_1, gy_1, color_1, paint_2, g_1, false, kernelContext_5);
        return _S63;
    }
    thread array<float4, int(5)> texel_1;
    int i_2 = int(0);
    for(;;)
    {
        if(i_2 < int(5))
        {
        }
        else
        {
            break;
        }
        float4 _S64 = tableTexel_0(float(i_2), paint_2.w, kernelContext_5);
        texel_1[i_2] = _S64;
        i_2 = i_2 + int(1);
    }
    float4 one_0 = float4(1.0f, 1.0f, 1.0f, 1.0f);
    float4 _S65 = paintOne_0(p_1, gx_1, gy_1, one_0, texel_1[int(0)], texel_1[int(1)], false, kernelContext_5);
    float4 _S66 = paintOne_0(p_1, gx_1, gy_1, one_0, texel_1[int(2)], texel_1[int(3)], true, kernelContext_5);
    return blendColors_0(_S66, _S65, int(texel_1[int(4)].x + 0.5f)) * float4(color_1.w) ;
}

float edgeCoverage_0(float d_2, float2 g_2)
{
    float _S67 = max(length(g_2), 9.99999997475242708e-07f);
    float2 n_0 = abs(g_2) / float2(_S67) ;
    float _S68 = n_0.x;
    float _S69 = n_0.y;
    float _S70 = max(_S68, _S69);
    float _S71 = min(_S68, _S69);
    float _S72 = _S70 + _S71;
    float u_0 = clamp(0.5f * _S72 - d_2 / _S67, 0.0f, _S72);
    float _S73 = max(2.0f * _S70 * _S71, 9.999999960041972e-13f);
    float _S74;
    if(u_0 <= _S71)
    {
        _S74 = u_0 * u_0 / _S73;
    }
    else
    {
        if(u_0 <= _S70)
        {
            _S74 = (u_0 - 0.5f * _S71) / _S70;
        }
        else
        {
            float _S75 = _S72 - u_0;
            _S74 = 1.0f - _S75 * _S75 / _S73;
        }
    }
    return _S74;
}

float2 pixelSteps_0(float2 n_1, float2 dx_0, float2 dy_0)
{
    return float2(dot(dx_0, n_1), dot(dy_0, n_1));
}

float roundRectCoverage_0(float2 p_2, float4 rect_0, float4 radii_0, float2 dx_1, float2 dy_1)
{
    float _S76 = dx_1.x;
    float _S77 = dy_1.x;
    float _S78 = dx_1.y;
    float _S79 = dy_1.y;
    float2 _S80 = float2(0.5f) ;
    float2 _S81 = rect_0.xy;
    float2 _S82 = rect_0.zw;
    float2 _S83 = min(p_2 - _S81, _S82 - p_2);
    float _S84 = radii_0.x;
    float _S85 = radii_0.y;
    float _S86 = radii_0.z;
    float _S87 = radii_0.w;
    float _S88 = max(max(_S84, _S85), max(_S86, _S87));
    bool _S89;
    if(all(_S83 >= (_S80 * float2(abs(_S76) + abs(_S77), abs(_S78) + abs(_S79)))))
    {
        if((_S83.x) >= _S88)
        {
            _S89 = true;
        }
        else
        {
            _S89 = (_S83.y) >= _S88;
        }
    }
    else
    {
        _S89 = false;
    }
    if(_S89)
    {
        return 1.0f;
    }
    float2 gx_2 = float2(_S76, _S77);
    float2 gy_2 = float2(_S78, _S79);
    float _S90 = p_2.x;
    float _S91 = p_2.y;
    float straight_1 = saturate(edgeCoverage_0(rect_0.x - _S90, - gx_2) + edgeCoverage_0(_S90 - rect_0.z, gx_2) - 1.0f) * saturate(edgeCoverage_0(rect_0.y - _S91, - gy_2) + edgeCoverage_0(_S91 - rect_0.w, gy_2) - 1.0f);
    float2 halfSize_0 = (_S82 - _S81) * _S80;
    float2 q_0 = p_2 - (_S81 + _S82) * _S80;
    bool _S92 = (q_0.x) < 0.0f;
    float r_1;
    if(_S92)
    {
        if((q_0.y) < 0.0f)
        {
            r_1 = _S84;
        }
        else
        {
            r_1 = _S87;
        }
    }
    else
    {
        if((q_0.y) < 0.0f)
        {
            r_1 = _S85;
        }
        else
        {
            r_1 = _S86;
        }
    }
    float2 corner_0 = abs(q_0) - halfSize_0 + float2(r_1) ;
    float len_0 = length(corner_0);
    float2 _S93;
    if(len_0 > 0.0f)
    {
        _S93 = corner_0 / float2(len_0) ;
    }
    else
    {
        _S93 = float2(0.70709997415542603f, 0.70709997415542603f);
    }
    int _S94;
    if(_S92)
    {
        _S94 = int(-1);
    }
    else
    {
        _S94 = int(1);
    }
    float _S95 = float(_S94);
    if((q_0.y) < 0.0f)
    {
        _S94 = int(-1);
    }
    else
    {
        _S94 = int(1);
    }
    float curved_0 = edgeCoverage_0(len_0 - r_1, pixelSteps_0(_S93 * float2(_S95, float(_S94)), dx_1, dy_1));
    if(r_1 > 0.0f)
    {
        _S89 = (corner_0.x) > 0.0f;
    }
    else
    {
        _S89 = false;
    }
    if(_S89)
    {
        _S89 = (corner_0.y) > 0.0f;
    }
    else
    {
        _S89 = false;
    }
    if(_S89)
    {
        r_1 = min(curved_0, straight_1);
    }
    else
    {
        r_1 = straight_1;
    }
    return r_1;
}

float4 arcCap_0(float2 q_1, float radius_0, float halfSize_1, float start_0, float sweep_0, bool roundCaps_0)
{
    float aperture_0 = abs(sweep_0) * 0.5f;
    float4 _S96 = float4(0.0f, 1.0f, 0.0f, 0.0f);
    float _S97 = - (start_0 + sweep_0 * 0.5f);
    float c_5 = cos(_S97);
    float s_3 = sin(_S97);
    float _S98 = q_1.x;
    float _S99 = q_1.y;
    float _S100 = s_3 * _S98 + c_5 * _S99;
    thread float2 p_3 = float2(c_5 * _S98 - s_3 * _S99, _S100);
    int _S101;
    if(_S100 < 0.0f)
    {
        _S101 = int(-1);
    }
    else
    {
        _S101 = int(1);
    }
    float mirror_0 = float(_S101);
    p_3.y = abs(p_3.y);
    float _S102 = cos(aperture_0);
    float _S103 = sin(aperture_0);
    float2 end_0 = float2(_S102, _S103);
    bool _S104;
    if(aperture_0 < 3.1415901184082031f)
    {
        _S104 = (atan2(p_3.y, p_3.x)) > aperture_0;
    }
    else
    {
        _S104 = false;
    }
    float4 field_0;
    if(_S104)
    {
        float2 v_0 = p_3 - end_0 * float2(radius_0) ;
        float2 outward_0 = float2(- _S103, _S102);
        thread float2 n_2 = outward_0;
        float d_3;
        float2 _S105;
        if(roundCaps_0)
        {
            float l_2 = length(v_0);
            float _S106 = l_2 - halfSize_1;
            if(l_2 > 0.0f)
            {
                _S105 = v_0 / float2(l_2) ;
            }
            else
            {
                _S105 = outward_0;
            }
            n_2 = _S105;
            d_3 = _S106;
        }
        else
        {
            float along_0 = dot(v_0, outward_0);
            float toward_0 = dot(v_0, end_0);
            float across_0 = abs(toward_0) - halfSize_1;
            float _S107 = max(along_0, 0.0f);
            float _S108 = max(across_0, 0.0f);
            float l_3 = length(float2(_S107, _S108));
            bool _S109 = across_0 <= 0.0f;
            if(_S109)
            {
                d_3 = along_0;
            }
            else
            {
                d_3 = l_3;
            }
            if(_S109)
            {
                _S104 = true;
            }
            else
            {
                _S104 = l_3 <= 0.0f;
            }
            if(_S104)
            {
                _S105 = outward_0;
            }
            else
            {
                float2 _S110 = outward_0 * float2(_S107) ;
                if(toward_0 < 0.0f)
                {
                    _S101 = int(-1);
                }
                else
                {
                    _S101 = int(1);
                }
                _S105 = (_S110 + end_0 * float2(float(_S101))  * float2(_S108) ) / float2(l_3) ;
            }
            n_2 = _S105;
        }
        n_2.y = n_2.y * mirror_0;
        field_0 = float4(d_3, c_5 * n_2.x + s_3 * n_2.y, - s_3 * n_2.x + c_5 * n_2.y, 1.0f);
    }
    else
    {
        field_0 = _S96;
    }
    return field_0;
}

float2 erf2_0(float2 x_2)
{
    float2 _S111 = float2((vec<int,2>(sign((x_2)))));
    float2 a_1 = abs(x_2);
    float2 _S112 = float2(1.0f)  + (float2(0.27839300036430359f)  + (float2(0.23038899898529053f)  + float2(0.07810799777507782f)  * (a_1 * a_1)) * a_1) * a_1;
    float2 _S113 = _S112 * _S112;
    return _S111 - _S111 / (_S113 * _S113);
}

float gaussian_0(float x_3, float sigma_0)
{
    return exp(- (x_3 * x_3) / (2.0f * sigma_0 * sigma_0)) / (sqrt(6.2831854820251465f) * sigma_0);
}

float blurredRoundRect_0(float2 p_4, float4 rect_1, float4 radii_1, float sigma_1)
{
    float2 _S114 = rect_1.xy;
    float2 _S115 = rect_1.zw;
    float2 _S116 = float2(0.5f) ;
    float2 halfSize_2 = (_S115 - _S114) * _S116;
    float2 q_2 = p_4 - (_S114 + _S115) * _S116;
    float _S117 = q_2.x;
    float _S118;
    if(_S117 < 0.0f)
    {
        if((q_2.y) < 0.0f)
        {
            _S118 = radii_1.x;
        }
        else
        {
            _S118 = radii_1.w;
        }
    }
    else
    {
        if((q_2.y) < 0.0f)
        {
            _S118 = radii_1.y;
        }
        else
        {
            _S118 = radii_1.z;
        }
    }
    float _S119 = q_2.y;
    float _S120 = halfSize_2.y;
    float low_0 = _S119 - _S120;
    float high_0 = _S119 + _S120;
    float start_1 = clamp(-3.0f * sigma_1, low_0, high_0);
    float step_0 = (clamp(3.0f * sigma_1, low_0, high_0) - start_1) / 4.0f;
    float _S121 = start_1 + step_0 * 0.5f;
    int i_3 = int(0);
    float y_0 = _S121;
    float value_0 = 0.0f;
    for(;;)
    {
        if(i_3 < int(4))
        {
        }
        else
        {
            break;
        }
        float _S122 = min(_S120 - _S118 - abs(_S119 - y_0), 0.0f);
        float curved_1 = halfSize_2.x - _S118 + sqrt(max(0.0f, _S118 * _S118 - _S122 * _S122));
        float2 integral_0 = _S116 + _S116 * erf2_0((float2(_S117)  + float2(- curved_1, curved_1)) * float2((sqrt(0.5f) / sigma_1)) );
        float value_1 = value_0 + (integral_0.y - integral_0.x) * gaussian_0(y_0, sigma_1) * step_0;
        float y_1 = y_0 + step_0;
        i_3 = i_3 + int(1);
        y_0 = y_1;
        value_0 = value_1;
    }
    return value_0;
}

float mitchell_0(float x_4)
{
    float _S123 = abs(x_4);
    float _S124;
    if(_S123 < 1.0f)
    {
        _S124 = (7.0f * _S123 * _S123 * _S123 - 12.0f * _S123 * _S123 + 5.3333334922790527f) / 6.0f;
    }
    else
    {
        if(_S123 < 2.0f)
        {
            _S124 = (-2.3333332538604736f * _S123 * _S123 * _S123 + 12.0f * _S123 * _S123 - 20.0f * _S123 + 10.666666984558105f) / 6.0f;
        }
        else
        {
            _S124 = 0.0f;
        }
    }
    return _S124;
}

float4 cubicSample_0(float2 uv_0, float2 size_1, KernelContext_0 thread* kernelContext_6)
{
    float2 _S125 = float2(0.5f) ;
    float2 t_2 = uv_0 * size_1 - _S125;
    float2 base_0 = floor(t_2);
    float2 _S126 = t_2 - base_0;
    thread float4 sum_0 = float4(int4(int(0)) );
    int j_0 = int(-1);
    for(;;)
    {
        if(j_0 <= int(2))
        {
        }
        else
        {
            break;
        }
        float _S127 = float(j_0);
        float _S128 = mitchell_0(_S127 - _S126.y);
        int i_4 = int(-1);
        for(;;)
        {
            if(i_4 <= int(2))
            {
            }
            else
            {
                break;
            }
            float _S129 = float(i_4);
            sum_0 = sum_0 + ((kernelContext_6->texture0_texture_0).sample((kernelContext_6->texture0_sampler_0), ((base_0 + float2(_S129, _S127) + _S125) / size_1), level((0.0f)))) * float4((mitchell_0(_S129 - _S126.x) * _S128)) ;
            i_4 = i_4 + int(1);
        }
        j_0 = j_0 + int(1);
    }
    sum_0.w = saturate(sum_0.w);
    return float4(clamp(sum_0.xyz, float3(int3(int(0)) ), float3(sum_0.w) ), sum_0.w);
}

struct Varyings_0
{
    float4 position_0;
    float2 local_0;
    float w_0;
    float2 texcoord_0;
    float4 vertexColor_0;
    [[flat]] float4 shape_0;
    [[flat]] float4 radii_2;
    [[flat]] float4 color_2;
    [[flat]] float4 params_0;
    [[flat]] float4 uv_1;
    [[flat]] float2 kindFlags_0;
    [[flat]] float4 paint_3;
    [[flat]] float4 gradient_0;
    [[flat]] float4 filter_0;
    [[flat]] float4 steps_0;
    float4 inside_0;
};

float4 sampleAt_0(float2 uv_2, const Varyings_0 thread* v_1, float2 tx_0, float2 ty_0, KernelContext_0 thread* kernelContext_7)
{
    float2 _S130;
    float2 _S131;
    if((FLAGS_0 & int(32)) == int(0))
    {
        float2 across_1 = (v_1->uv_1.zw - v_1->uv_1.xy) / (v_1->shape_0.zw - v_1->shape_0.xy);
        float2 _S132 = across_1 * v_1->steps_0.zw;
        _S130 = across_1 * v_1->steps_0.xy;
        _S131 = _S132;
    }
    else
    {
        _S130 = tx_0;
        _S131 = ty_0;
    }
    return ((kernelContext_7->texture0_texture_0).sample((kernelContext_7->texture0_sampler_0), (uv_2), gradient2d((_S130), (_S131))));
}

float pathCoverage_0(float sum_1, bool evenOdd_0)
{
    float _S133 = abs(sum_1);
    if(!evenOdd_0)
    {
        return min(_S133, 1.0f);
    }
    float m_0 = _S133 - 2.0f * floor(_S133 / 2.0f);
    float _S134;
    if(m_0 <= 1.0f)
    {
        _S134 = m_0;
    }
    else
    {
        _S134 = 2.0f - m_0;
    }
    return _S134;
}

float roundRectDistance_0(float2 p_5, float4 rect_2, float4 radii_3)
{
    float2 _S135 = rect_2.xy;
    float2 _S136 = rect_2.zw;
    float2 _S137 = float2(0.5f) ;
    float2 halfSize_3 = (_S136 - _S135) * _S137;
    float2 q_3 = p_5 - (_S135 + _S136) * _S137;
    float r_2;
    if((q_3.x) < 0.0f)
    {
        if((q_3.y) < 0.0f)
        {
            r_2 = radii_3.x;
        }
        else
        {
            r_2 = radii_3.w;
        }
    }
    else
    {
        if((q_3.y) < 0.0f)
        {
            r_2 = radii_3.y;
        }
        else
        {
            r_2 = radii_3.z;
        }
    }
    float2 d_4 = abs(q_3) - halfSize_3 + float2(r_2) ;
    return min(max(d_4.x, d_4.y), 0.0f) + length(max(d_4, float2(int2(int(0)) ))) - r_2;
}

float2 maskTexcoord_0(float2 texel_2, KernelContext_0 thread* kernelContext_8)
{
    return (floor(texel_2) + float2(0.5f) ) * kernelContext_8->uniforms_0->texels_0.xy;
}

float clipCoverage_0(float2 pixel_1, int count_0, float row_3, KernelContext_0 thread* kernelContext_9)
{
    int i_5 = int(0);
    float inside_1 = 1.0f;
    for(;;)
    {
        if(i_5 < count_0)
        {
        }
        else
        {
            break;
        }
        float x_5 = float(i_5) * 6.0f;
        float4 _S138 = tableTexel_0(x_5, row_3, kernelContext_9);
        float4 _S139 = tableTexel_0(x_5 + 1.0f, row_3, kernelContext_9);
        float4 _S140 = tableTexel_0(x_5 + 2.0f, row_3, kernelContext_9);
        float c_6;
        if((_S138.x) > 0.5f)
        {
            bool within_0;
            if(all(pixel_1 >= (_S140.xy)))
            {
                within_0 = all(pixel_1 < (_S140.zw));
            }
            else
            {
                within_0 = false;
            }
            float2 _S141 = maskTexcoord_0(pixel_1 - _S139.xy, kernelContext_9);
            float sum_2 = ((kernelContext_9->texture4_texture_0).sample((kernelContext_9->texture4_sampler_0), (_S141), level((0.0f)))).x;
            if(within_0)
            {
                c_6 = pathCoverage_0(sum_2, (_S138.z) > 0.5f);
            }
            else
            {
                c_6 = 0.0f;
            }
        }
        else
        {
            float4 _S142 = tableTexel_0(x_5 + 3.0f, row_3, kernelContext_9);
            float3 r0_1 = _S142.xyz;
            float4 _S143 = tableTexel_0(x_5 + 4.0f, row_3, kernelContext_9);
            float3 r1_1 = _S143.xyz;
            float4 _S144 = tableTexel_0(x_5 + 5.0f, row_3, kernelContext_9);
            float3 r2_1 = _S144.xyz;
            float3 h_1 = float3(pixel_1, 1.0f);
            float _S145 = dot(r2_1, h_1);
            float2 _S146 = float2(_S145) ;
            float2 p_6 = float2(dot(r0_1, h_1), dot(r1_1, h_1)) / _S146;
            float _S147 = r2_1.x;
            float _S148 = p_6.x;
            float _S149 = p_6.y;
            float2 dx_2 = float2(r0_1.x - _S147 * _S148, r1_1.x - _S147 * _S149) / _S146;
            float _S150 = r2_1.y;
            float2 dy_2 = float2(r0_1.y - _S150 * _S148, r1_1.y - _S150 * _S149) / _S146;
            if(_S145 > 0.0f)
            {
                c_6 = roundRectCoverage_0(p_6, _S139, _S140, dx_2, dy_2);
            }
            else
            {
                c_6 = 0.0f;
            }
        }
        float _S151;
        if((_S138.y) > 0.5f)
        {
            _S151 = 1.0f - c_6;
        }
        else
        {
            _S151 = c_6;
        }
        float inside_2 = inside_1 * _S151;
        i_5 = i_5 + int(1);
        inside_1 = inside_2;
    }
    return inside_1;
}

float4 applyFilters_0(float4 color_3, float4 filter_1, KernelContext_0 thread* kernelContext_10)
{
    int count_1 = int(filter_1.x + 0.5f);
    float _S152 = color_3.w;
    float4 c_7;
    if(_S152 > 0.0f)
    {
        c_7 = float4(color_3.xyz / float3(_S152) , _S152);
    }
    else
    {
        c_7 = float4(0.0f, 0.0f, 0.0f, 0.0f);
    }
    int i_6 = int(0);
    for(;;)
    {
        if(i_6 < count_1)
        {
        }
        else
        {
            break;
        }
        float x_6 = float(i_6) * 6.0f;
        float _S153 = filter_1.y;
        float4 _S154 = tableTexel_1(x_6, _S153, kernelContext_10);
        int kind_2 = int(_S154.x + 0.5f);
        float4 _S155 = tableTexel_1(x_6 + 1.0f, _S153, kernelContext_10);
        float4 _S156 = tableTexel_1(x_6 + 2.0f, _S153, kernelContext_10);
        if(kind_2 == int(1))
        {
            float4 _S157 = tableTexel_1(x_6 + 3.0f, _S153, kernelContext_10);
            float4 _S158 = tableTexel_1(x_6 + 4.0f, _S153, kernelContext_10);
            float4 _S159 = tableTexel_1(x_6 + 5.0f, _S153, kernelContext_10);
            c_7 = saturate(float4(dot(_S155, c_7), dot(_S156, c_7), dot(_S157, c_7), dot(_S158, c_7)) + _S159);
        }
        else
        {
            float4 c_8;
            if(kind_2 == int(2))
            {
                c_8 = float4(saturate(c_7.xyz * _S155.xyz + _S156.xyz), c_7.w);
            }
            else
            {
                float _S160 = _S155.w;
                float _S161 = c_7.w;
                float4 tinted_0 = blendColors_0(float4(_S155.xyz * float3(_S160) , _S160), float4(c_7.xyz * float3(_S161) , _S161), int(_S154.y + 0.5f));
                float _S162 = tinted_0.w;
                if(_S162 > 0.0f)
                {
                    c_8 = float4(saturate(tinted_0.xyz / float3(_S162) ), min(_S162, 1.0f));
                }
                else
                {
                    c_8 = float4(0.0f, 0.0f, 0.0f, 0.0f);
                }
            }
            c_7 = c_8;
        }
        i_6 = i_6 + int(1);
    }
    if(count_1 > int(0))
    {
        float _S163 = c_7.w;
        c_7 = float4(c_7.xyz * float3(_S163) , _S163);
    }
    else
    {
        c_7 = color_3;
    }
    return c_7;
}

float4 shade_0(const Varyings_0 thread* v_2, float2 dx_3, float2 dy_3, float2 tx_1, float2 ty_1, KernelContext_0 thread* kernelContext_11)
{
    int kind_3 = int(v_2->kindFlags_0.x + 0.5f);
    int flags_0 = int(v_2->kindFlags_0.y + 0.5f) & FLAGS_0;
    float2 _S164;
    float2 _S165;
    if((FLAGS_0 & int(32)) == int(0))
    {
        float2 _S166 = v_2->steps_0.zw;
        _S164 = v_2->steps_0.xy;
        _S165 = _S166;
    }
    else
    {
        _S164 = dx_3;
        _S165 = dy_3;
    }
    bool _S167 = kind_3 == int(10);
    float2 painted_0;
    if(_S167)
    {
        painted_0 = v_2->texcoord_0;
    }
    else
    {
        painted_0 = v_2->local_0;
    }
    float2 gx_3;
    if(_S167)
    {
        gx_3 = tx_1;
    }
    else
    {
        gx_3 = _S164;
    }
    float2 gy_3;
    if(_S167)
    {
        gy_3 = ty_1;
    }
    else
    {
        gy_3 = _S165;
    }
    if((flags_0 & int(4)) != int(0))
    {
        float3 h_2 = float3(v_2->local_0, 1.0f);
        float2 _S168 = float2(dot(v_2->shape_0.xyz, h_2)) ;
        float2 painted_1 = float2(dot(v_2->radii_2.xyz, h_2), dot(v_2->uv_1.xyz, h_2)) / _S168;
        float2 _S169 = v_2->shape_0.xy;
        float2 jx_1 = (v_2->radii_2.xy - float2(painted_1.x)  * _S169) / _S168;
        float2 jy_1 = (v_2->uv_1.xy - float2(painted_1.y)  * _S169) / _S168;
        float2 gx_4 = float2(dot(jx_1, gx_3), dot(jy_1, gx_3));
        float2 gy_4 = float2(dot(jx_1, gy_3), dot(jy_1, gy_3));
        painted_0 = painted_1;
        gx_3 = gx_4;
        gy_3 = gy_4;
    }
    bool aliased_0 = (flags_0 & int(16)) != int(0);
    if(aliased_0)
    {
        float2 _S170 = float2(9.9999997473787516e-05f) ;
        float2 _S171 = _S165 * _S170;
        _S164 = _S164 * _S170;
        _S165 = _S171;
    }
    float4 _S172 = v_2->color_2;
    float4 _S173 = v_2->gradient_0;
    float4 _S174 = paintAt_0(painted_0, gx_3, gy_3, v_2->color_2, v_2->paint_3, v_2->gradient_0, kernelContext_11);
    bool edged_0 = (flags_0 & int(2)) != int(0);
    float4 _S175 = v_2->uv_1;
    float2 _S176 = v_2->uv_1.xy;
    float2 _S177 = v_2->uv_1.zw;
    float4 _S178 = v_2->params_0;
    float2 _S179 = v_2->params_0.zw;
    float2 lowUv_0 = min(_S176, _S177) + _S179;
    float2 highUv_0 = max(_S176, _S177) - _S179;
    float2 texcoord_1;
    if(edged_0)
    {
        texcoord_1 = clamp(v_2->texcoord_0, lowUv_0, highUv_0);
    }
    else
    {
        texcoord_1 = v_2->texcoord_0;
    }
    float edge_0;
    if(edged_0)
    {
        edge_0 = roundRectCoverage_0(v_2->local_0, v_2->shape_0, float4(0.0f, 0.0f, 0.0f, 0.0f), _S164, _S165);
    }
    else
    {
        edge_0 = 1.0f;
    }
    bool _S180;
    if(drawn_0(int(0)))
    {
        _S180 = kind_3 == int(0);
    }
    else
    {
        _S180 = false;
    }
    float cover_1;
    float4 src_1;
    if(_S180)
    {
        float _S181 = roundRectCoverage_0(v_2->local_0, v_2->shape_0, v_2->radii_2, _S164, _S165);
        src_1 = _S174;
        cover_1 = _S181;
    }
    else
    {
        if(drawn_0(int(1)))
        {
            _S180 = kind_3 == int(1);
        }
        else
        {
            _S180 = false;
        }
        if(_S180)
        {
            float w_1 = _S178.x;
            float2 _S182 = float2(w_1) ;
            float _S183 = saturate(roundRectCoverage_0(v_2->local_0, v_2->shape_0, v_2->radii_2, _S164, _S165) - roundRectCoverage_0(v_2->local_0, float4(v_2->shape_0.xy + _S182, v_2->shape_0.zw - _S182), max(v_2->radii_2 - float4(w_1) , float4(int4(int(0)) )), _S164, _S165));
            src_1 = _S174;
            cover_1 = _S183;
        }
        else
        {
            if(drawn_0(int(2)))
            {
                _S180 = kind_3 == int(2);
            }
            else
            {
                _S180 = false;
            }
            if(_S180)
            {
                float2 a_2 = v_2->shape_0.xy;
                float2 _S184 = v_2->shape_0.zw - a_2;
                float len_1 = length(_S184);
                float2 dir_0;
                if(len_1 > 0.0f)
                {
                    dir_0 = _S184 / float2(len_1) ;
                }
                else
                {
                    dir_0 = float2(1.0f, 0.0f);
                }
                float2 side_0 = float2(- dir_0.y, dir_0.x);
                float2 d_5 = v_2->local_0 - a_2;
                float _S185 = len_1 * 0.5f;
                float along_1 = dot(d_5, dir_0) - _S185;
                float across_2 = dot(d_5, side_0);
                float h_3 = _S178.x;
                float reach_0 = _S185 + h_3;
                float2 gSide_0 = pixelSteps_0(side_0, _S164, _S165);
                float2 gDir_0 = pixelSteps_0(dir_0, _S164, _S165);
                float _S186 = saturate(edgeCoverage_0(across_2 - h_3, gSide_0) + edgeCoverage_0(- across_2 - h_3, - gSide_0) - 1.0f) * saturate(edgeCoverage_0(along_1 - reach_0, gDir_0) + edgeCoverage_0(- along_1 - reach_0, - gDir_0) - 1.0f);
                src_1 = _S174;
                cover_1 = _S186;
            }
            else
            {
                if(drawn_0(int(3)))
                {
                    _S180 = kind_3 == int(3);
                }
                else
                {
                    _S180 = false;
                }
                if(_S180)
                {
                    float2 q_4 = v_2->local_0 - v_2->shape_0.xy;
                    float len_2 = length(q_4);
                    float radius_1 = v_2->shape_0.z;
                    float h_4 = _S178.x;
                    if(len_2 > 0.0f)
                    {
                        painted_0 = q_4 / float2(len_2) ;
                    }
                    else
                    {
                        painted_0 = float2(1.0f, 0.0f);
                    }
                    float2 g_3 = pixelSteps_0(painted_0, _S164, _S165);
                    float ring_0 = saturate(edgeCoverage_0(len_2 - radius_1 - h_4, g_3) + edgeCoverage_0(radius_1 - h_4 - len_2, - g_3) - 1.0f);
                    float4 cap_0 = arcCap_0(q_4, radius_1, h_4, _S178.y, _S178.z, (_S178.w) > 0.5f);
                    if((cap_0.w) > 0.5f)
                    {
                        edge_0 = edgeCoverage_0(cap_0.x, pixelSteps_0(cap_0.yz, _S164, _S165));
                    }
                    else
                    {
                        edge_0 = ring_0;
                    }
                    src_1 = _S174;
                    cover_1 = edge_0;
                }
                else
                {
                    if(drawn_0(int(4)))
                    {
                        _S180 = kind_3 == int(4);
                    }
                    else
                    {
                        _S180 = false;
                    }
                    if(_S180)
                    {
                        float sigma_2 = _S178.x;
                        if(sigma_2 > 0.0099999997764825821f)
                        {
                            cover_1 = blurredRoundRect_0(v_2->local_0, v_2->shape_0, v_2->radii_2, sigma_2);
                        }
                        else
                        {
                            cover_1 = roundRectCoverage_0(v_2->local_0, v_2->shape_0, v_2->radii_2, _S164, _S165);
                        }
                        if((_S178.y) > 0.5f)
                        {
                            edge_0 = (1.0f - cover_1) * roundRectCoverage_0(v_2->local_0, _S173, _S175, _S164, _S165);
                        }
                        else
                        {
                            edge_0 = cover_1;
                        }
                        src_1 = _S174;
                        cover_1 = edge_0;
                    }
                    else
                    {
                        if(drawn_0(int(5)))
                        {
                            _S180 = kind_3 == int(5);
                        }
                        else
                        {
                            _S180 = false;
                        }
                        if(_S180)
                        {
                            if((_S178.y) > 0.5f)
                            {
                                float4 _S187 = cubicSample_0(texcoord_1, float2(0.5f)  / _S179, kernelContext_11);
                                src_1 = _S187;
                            }
                            else
                            {
                                float4 _S188 = sampleAt_0(texcoord_1, v_2, tx_1, ty_1, kernelContext_11);
                                src_1 = _S188;
                            }
                            bool colored_0 = (_S178.x) > 0.5f;
                            float4 _S189;
                            if(colored_0)
                            {
                                float _S190 = src_1.w;
                                _S189 = float4(src_1.xyz * float3(_S190) , _S190) * float4(_S172.w) ;
                            }
                            else
                            {
                                _S189 = _S174;
                            }
                            if(colored_0)
                            {
                                cover_1 = 1.0f;
                            }
                            else
                            {
                                cover_1 = src_1.w;
                            }
                            float _S191 = cover_1 * edge_0;
                            src_1 = _S189;
                            cover_1 = _S191;
                        }
                        else
                        {
                            if(drawn_0(int(6)))
                            {
                                _S180 = kind_3 == int(6);
                            }
                            else
                            {
                                _S180 = false;
                            }
                            if(_S180)
                            {
                                if((_S178.y) > 0.5f)
                                {
                                    float4 _S192 = cubicSample_0(texcoord_1, float2(0.5f)  / _S179, kernelContext_11);
                                    src_1 = _S192;
                                }
                                else
                                {
                                    float4 _S193 = sampleAt_0(texcoord_1, v_2, tx_1, ty_1, kernelContext_11);
                                    src_1 = _S193;
                                }
                                src_1 = src_1 * float4(_S178.x) ;
                                cover_1 = edge_0;
                            }
                            else
                            {
                                if(drawn_0(int(7)))
                                {
                                    _S180 = kind_3 == int(7);
                                }
                                else
                                {
                                    _S180 = false;
                                }
                                if(_S180)
                                {
                                    float2 _S194 = v_2->local_0;
                                    float2 _S195 = _S178.xy;
                                    float2 _S196 = float2(0.5f) ;
                                    bool mixed_0 = (flags_0 & int(8)) != int(0);
                                    float4 _S197 = ((kernelContext_11->texture0_texture_0).sample((kernelContext_11->texture0_sampler_0), ((clamp(v_2->local_0, _S195 + _S196, _S179 - _S196) - _S195) * _S177), level((0.0f))));
                                    if(mixed_0)
                                    {
                                        edge_0 = 1.0f;
                                    }
                                    else
                                    {
                                        edge_0 = _S172.w;
                                    }
                                    float4 _S198 = _S197 * float4(edge_0) ;
                                    if((flags_0 & int(1)) != int(0))
                                    {
                                        cover_1 = pathCoverage_0(((kernelContext_11->texture1_texture_0).sample((kernelContext_11->texture1_sampler_0), ((_S194 - v_2->radii_2.xy) * kernelContext_11->uniforms_0->texels_0.xy), level((0.0f)))).x, false);
                                    }
                                    else
                                    {
                                        cover_1 = roundRectCoverage_0(_S194, v_2->shape_0, v_2->radii_2, _S164, _S165);
                                    }
                                    if(mixed_0)
                                    {
                                        cover_1 = cover_1 * _S172.w;
                                    }
                                    src_1 = _S198;
                                }
                                else
                                {
                                    if(drawn_0(int(9)))
                                    {
                                        _S180 = kind_3 == int(9);
                                    }
                                    else
                                    {
                                        _S180 = false;
                                    }
                                    if(_S180)
                                    {
                                        float d_6 = - roundRectDistance_0(v_2->local_0, v_2->shape_0, v_2->radii_2);
                                        float t_3 = saturate(d_6 / _S178.x);
                                        if(d_6 > 0.0f)
                                        {
                                            float _S199 = _S178.y;
                                            if(_S199 <= 0.0f)
                                            {
                                                _S180 = true;
                                            }
                                            else
                                            {
                                                _S180 = d_6 < (_S199 - 0.0009765625f);
                                            }
                                        }
                                        else
                                        {
                                            _S180 = false;
                                        }
                                        if(_S180)
                                        {
                                            float _S200 = 1.0f - t_3;
                                            edge_0 = max(exp(-4.0f * _S200 * _S200) - 0.017999999225139618f, 0.0f);
                                        }
                                        else
                                        {
                                            edge_0 = 0.0f;
                                        }
                                        src_1 = _S174;
                                        cover_1 = edge_0;
                                    }
                                    else
                                    {
                                        if(drawn_0(int(8)))
                                        {
                                            _S180 = kind_3 == int(8);
                                        }
                                        else
                                        {
                                            _S180 = false;
                                        }
                                        if(_S180)
                                        {
                                            float4 _S201 = ((kernelContext_11->texture0_texture_0).read(uint2((int2(floor(v_2->local_0 - _S178.xy))))));
                                            float _S202 = pathCoverage_0(_S201.x, (_S178.z) > 0.5f);
                                            src_1 = _S174;
                                            cover_1 = _S202;
                                        }
                                        else
                                        {
                                            if(drawn_0(int(10)))
                                            {
                                                _S180 = _S167;
                                            }
                                            else
                                            {
                                                _S180 = false;
                                            }
                                            if(_S180)
                                            {
                                                if((_S178.z) > 0.5f)
                                                {
                                                    src_1 = blendColors_0(_S174, v_2->vertexColor_0, int(_S178.x + 0.5f));
                                                }
                                                else
                                                {
                                                    src_1 = _S174;
                                                }
                                                src_1 = src_1 * float4(_S178.y) ;
                                                cover_1 = 1.0f;
                                            }
                                            else
                                            {
                                                src_1 = _S174;
                                                cover_1 = 0.0f;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if(aliased_0)
    {
        int _S203;
        if(cover_1 > 0.5f)
        {
            _S203 = int(1);
        }
        else
        {
            _S203 = int(0);
        }
        cover_1 = float(_S203);
    }
    if(CLIPPED_0 != int(0))
    {
        float _S204 = clipCoverage_0(v_2->position_0.xy, int(v_2->filter_0.z + 0.5f), v_2->filter_0.w, kernelContext_11);
        cover_1 = cover_1 * _S204;
    }
    if(FILTERED_0 != int(0))
    {
        float4 _S205 = applyFilters_0(src_1, v_2->filter_0, kernelContext_11);
        src_1 = _S205;
    }
    float4 _S206 = finish_0(src_1, cover_1, v_2->position_0.xy, kernelContext_11);
    return _S206;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    float2 local_1 [[user(LOCAL)]];
    float w_2 [[user(DEPTH_W)]];
    float2 texcoord_2 [[user(TEXCOORD)]];
    float4 vertexColor_1 [[user(VERTEXCOLOR)]];
    [[flat]] float4 shape_1 [[user(SHAPE)]];
    [[flat]] float4 radii_4 [[user(RADII)]];
    [[flat]] float4 color_4 [[user(COLOR)]];
    [[flat]] float4 params_1 [[user(PARAMS)]];
    [[flat]] float4 uv_3 [[user(UV)]];
    [[flat]] float2 kindFlags_1 [[user(KIND)]];
    [[flat]] float4 paint_4 [[user(PAINT)]];
    [[flat]] float4 gradient_1 [[user(GRADIENT)]];
    [[flat]] float4 filter_2 [[user(FILTER)]];
    [[flat]] float4 steps_1 [[user(STEPS)]];
    float4 inside_3 [[user(INSIDE)]];
};

[[fragment]] pixelOutput_0 fs(pixelInput_0 _S207 [[stage_in]], float4 position_1 [[position]], texture2d<float, access::sample> texture0_texture_1 [[texture(0)]], sampler texture0_sampler_1 [[sampler(0)]], Uniforms_0 constant* uniforms_1 [[buffer(0)]], texture2d<float, access::sample> texture3_texture_1 [[texture(3)]], sampler texture3_sampler_1 [[sampler(3)]], texture2d<float, access::sample> texture2_texture_1 [[texture(2)]], sampler texture2_sampler_1 [[sampler(2)]], texture2d<float, access::sample> texture5_texture_1 [[texture(5)]], sampler texture5_sampler_1 [[sampler(5)]], texture2d<float, access::sample> texture1_texture_1 [[texture(1)]], sampler texture1_sampler_1 [[sampler(1)]], texture2d<float, access::sample> texture4_texture_1 [[texture(4)]], sampler texture4_sampler_1 [[sampler(4)]])
{
    thread KernelContext_0 kernelContext_12;
    (&kernelContext_12)->texture0_texture_0 = texture0_texture_1;
    (&kernelContext_12)->texture0_sampler_0 = texture0_sampler_1;
    (&kernelContext_12)->uniforms_0 = uniforms_1;
    (&kernelContext_12)->texture3_texture_0 = texture3_texture_1;
    (&kernelContext_12)->texture3_sampler_0 = texture3_sampler_1;
    (&kernelContext_12)->texture2_texture_0 = texture2_texture_1;
    (&kernelContext_12)->texture2_sampler_0 = texture2_sampler_1;
    (&kernelContext_12)->texture5_texture_0 = texture5_texture_1;
    (&kernelContext_12)->texture5_sampler_0 = texture5_sampler_1;
    (&kernelContext_12)->texture1_texture_0 = texture1_texture_1;
    (&kernelContext_12)->texture1_sampler_0 = texture1_sampler_1;
    (&kernelContext_12)->texture4_texture_0 = texture4_texture_1;
    (&kernelContext_12)->texture4_sampler_0 = texture4_sampler_1;
    float2 _S208 = float2(int2(int(0)) );
    float2 dx_4;
    float2 dy_4;
    float2 tx_2;
    float2 ty_2;
    if((FLAGS_0 & int(32)) != int(0))
    {
        float q_5 = 1.0f / _S207.w_2;
        float2 _S209 = float2(q_5) ;
        float2 lq_0 = _S207.local_1 * _S209;
        float2 dy_5 = (dfdy(lq_0) - _S207.local_1 * float2(dfdy(q_5)) ) / _S209;
        float2 tx_3 = dfdx(_S207.texcoord_2);
        float2 ty_3 = dfdy(_S207.texcoord_2);
        dx_4 = (dfdx(lq_0) - _S207.local_1 * float2(dfdx(q_5)) ) / _S209;
        dy_4 = dy_5;
        tx_2 = tx_3;
        ty_2 = ty_3;
    }
    else
    {
        dx_4 = _S208;
        dy_4 = _S208;
        tx_2 = _S208;
        ty_2 = _S208;
    }
    bool _S210 = drawn_0(int(0));
    bool quick_0;
    if(_S210)
    {
        quick_0 = true;
    }
    else
    {
        quick_0 = drawn_0(int(7));
    }
    if(quick_0)
    {
        float4 a_3 = abs(_S207.inside_3);
        if((a_3.x) <= 1.0f)
        {
            quick_0 = (a_3.y) <= 1.0f;
        }
        else
        {
            quick_0 = false;
        }
        if(quick_0)
        {
            quick_0 = true;
        }
        else
        {
            if((a_3.z) <= 1.0f)
            {
                quick_0 = (a_3.w) <= 1.0f;
            }
            else
            {
                quick_0 = false;
            }
        }
    }
    else
    {
        quick_0 = false;
    }
    float4 result_0;
    if(quick_0)
    {
        bool layer_0;
        if(drawn_0(int(7)))
        {
            if(!_S210)
            {
                quick_0 = true;
            }
            else
            {
                quick_0 = int(_S207.kindFlags_1.x + 0.5f) == int(7);
            }
            layer_0 = quick_0;
        }
        else
        {
            layer_0 = false;
        }
        if(layer_0)
        {
            result_0 = (((&kernelContext_12)->texture0_texture_0).sample(((&kernelContext_12)->texture0_sampler_0), (_S207.texcoord_2), level((0.0f)))) * float4(_S207.color_4.w) ;
        }
        else
        {
            result_0 = _S207.color_4;
        }
        float4 _S211 = finish_0(result_0, 1.0f, position_1.xy, &kernelContext_12);
        result_0 = _S211;
    }
    else
    {
        thread Varyings_0 _S212;
        (&_S212)->position_0 = position_1;
        (&_S212)->local_0 = _S207.local_1;
        (&_S212)->w_0 = _S207.w_2;
        (&_S212)->texcoord_0 = _S207.texcoord_2;
        (&_S212)->vertexColor_0 = _S207.vertexColor_1;
        (&_S212)->shape_0 = _S207.shape_1;
        (&_S212)->radii_2 = _S207.radii_4;
        (&_S212)->color_2 = _S207.color_4;
        (&_S212)->params_0 = _S207.params_1;
        (&_S212)->uv_1 = _S207.uv_3;
        (&_S212)->kindFlags_0 = _S207.kindFlags_1;
        (&_S212)->paint_3 = _S207.paint_4;
        (&_S212)->gradient_0 = _S207.gradient_1;
        (&_S212)->filter_0 = _S207.filter_2;
        (&_S212)->steps_0 = _S207.steps_1;
        (&_S212)->inside_0 = _S207.inside_3;
        float4 _S213 = shade_0(&_S212, dx_4, dy_4, tx_2, ty_2, &kernelContext_12);
        result_0 = _S213;
    }
    pixelOutput_0 _S214 = { result_0 };
    return _S214;
}

