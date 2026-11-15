#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct Uniforms_0
{
    float4 toClip_0;
    float4 map_0;
    float4 valid_0;
    float4 params_0;
    float4 extra_0;
    float4 mask_0;
    float4 geometry_0;
    float4 origin_0;
};

struct KernelContext_0
{
    Uniforms_0 constant* uniforms_0;
    texture2d<float, access::sample> table_texture_0;
    sampler table_sampler_0;
    texture2d<float, access::sample> source_texture_0;
    sampler source_sampler_0;
};

float fraction_0(float2 c_0, KernelContext_0 thread* kernelContext_0)
{
    float2 _S1 = max(c_0 - kernelContext_0->uniforms_0->origin_0.xy, float2(int2(int(0)) ));
    float4 g_0 = kernelContext_0->uniforms_0->geometry_0;
    float _S2;
    if((kernelContext_0->uniforms_0->mask_0.x) < 1.5f)
    {
        float2 _S3 = g_0.xy;
        float2 ba_0 = g_0.zw - _S3;
        float l_0 = dot(ba_0, ba_0);
        if(l_0 > 0.0f)
        {
            _S2 = saturate(dot(_S1 - _S3, ba_0) / l_0);
        }
        else
        {
            _S2 = 0.0f;
        }
        return _S2;
    }
    float _S4 = g_0.z;
    if(_S4 > 0.0f)
    {
        _S2 = saturate(distance(_S1, g_0.xy) / _S4);
    }
    else
    {
        _S2 = 0.0f;
    }
    return _S2;
}

float intensity_0(float fraction_1, KernelContext_0 thread* kernelContext_1)
{
    bool between_0;
    float result_0;
    int _S5 = int(kernelContext_1->uniforms_0->mask_0.z + 0.5f);
    int row_0 = int(kernelContext_1->uniforms_0->mask_0.y + 0.5f);
    float4 _S6 = ((kernelContext_1->table_texture_0).read(uint2((int2(int(0), row_0)))));
    float2 a_0 = _S6.xy;
    if(fraction_1 <= (a_0.x))
    {
        return a_0.y;
    }
    float2 a_1 = a_0;
    int i_0 = int(1);
    for(;;)
    {
        if(i_0 < _S5)
        {
        }
        else
        {
            between_0 = false;
            result_0 = 0.0f;
            break;
        }
        float4 _S7 = ((kernelContext_1->table_texture_0).read(uint2((int2(i_0, row_0)))));
        float2 b_0 = _S7.xy;
        float _S8 = a_1.x;
        if(fraction_1 >= _S8)
        {
            between_0 = fraction_1 <= (b_0.x);
        }
        else
        {
            between_0 = false;
        }
        if(between_0)
        {
            float _S9 = mix(a_1.y, b_0.y, saturate((fraction_1 - _S8) / max(b_0.x - _S8, 9.9999997473787516e-05f)));
            between_0 = true;
            result_0 = _S9;
            break;
        }
        int i_1 = i_0 + int(1);
        a_1 = b_0;
        i_0 = i_1;
    }
    if(between_0)
    {
    }
    else
    {
        result_0 = a_1.y;
    }
    return result_0;
}

float4 content_0(float2 p_0, KernelContext_0 thread* kernelContext_2)
{
    return ((kernelContext_2->source_texture_0).sample((kernelContext_2->source_sampler_0), ((p_0 * kernelContext_2->uniforms_0->map_0.xy + kernelContext_2->uniforms_0->map_0.zw) * kernelContext_2->uniforms_0->extra_0.zw), level((0.0f))));
}

float gaussian_0(float x_0, float sigma_0)
{
    return exp(- (x_0 * x_0) / (2.0f * sigma_0 * sigma_0));
}

float4 progressive_0(float2 c_1, KernelContext_0 thread* kernelContext_3)
{
    float4 sum_0;
    float2 axis_0;
    if((kernelContext_3->uniforms_0->params_0.z) > 0.5f)
    {
        axis_0 = float2(0.0f, 1.0f);
    }
    else
    {
        axis_0 = float2(1.0f, 0.0f);
    }
    float _S10 = dot(kernelContext_3->uniforms_0->valid_0.xy, axis_0);
    float _S11 = dot(kernelContext_3->uniforms_0->valid_0.zw, axis_0);
    int side_0;
    if((kernelContext_3->uniforms_0->params_0.y) > 2.5f)
    {
        side_0 = int(1);
    }
    else
    {
        side_0 = int(0);
    }
    float _S12 = float(side_0);
    float _S13 = kernelContext_3->uniforms_0->extra_0.x;
    float over_0;
    if((kernelContext_3->uniforms_0->mask_0.x) > 2.5f)
    {
        over_0 = ((kernelContext_3->table_texture_0).sample((kernelContext_3->table_sampler_0), (c_1 * kernelContext_3->uniforms_0->origin_0.zw), level((0.0f)))).w;
    }
    else
    {
        float _S14 = fraction_0(c_1, kernelContext_3);
        float _S15 = intensity_0(_S14, kernelContext_3);
        over_0 = _S15;
    }
    float radius_0 = kernelContext_3->uniforms_0->params_0.w * clamp(over_0, 0.0f, 1.0f);
    float r_0 = floor(radius_0);
    float4 _S16 = content_0(c_1, kernelContext_3);
    if(r_0 < 1.0f)
    {
        return _S16;
    }
    float _S17 = max(radius_0 / 2.0f, 1.0f);
    float i_2 = 1.0f;
    float weights_0 = 1.0f;
    float4 sum_1 = _S16;
    for(;;)
    {
        if(i_2 < 150.0f)
        {
        }
        else
        {
            break;
        }
        if(i_2 >= r_0)
        {
            break;
        }
        float high1_0 = gaussian_0(i_2 + 1.0f, _S17);
        float w_0 = gaussian_0(i_2, _S17) + high1_0;
        float2 _S18 = axis_0 * float2(((i_2 + high1_0 / w_0) * _S13)) ;
        side_0 = int(-1);
        sum_0 = sum_1;
        for(;;)
        {
            if(side_0 <= int(1))
            {
            }
            else
            {
                break;
            }
            float2 p_1 = c_1 + float2(float(side_0))  * _S18;
            float along_0 = dot(p_1, axis_0);
            float in_0 = step(_S10, along_0) * (1.0f - step(_S11, along_0));
            float weights_1 = weights_0 + w_0 * max(in_0, _S12);
            if(in_0 > 0.0f)
            {
                float4 _S19 = content_0(p_1, kernelContext_3);
                sum_0 = sum_0 + float4(w_0)  * _S19;
            }
            side_0 = side_0 + int(2);
            weights_0 = weights_1;
        }
        i_2 = i_2 + 2.0f;
        sum_1 = sum_0;
    }
    float _S20 = gaussian_0(r_0, _S17) * ((r_0 - 2.0f * floor(r_0 / 2.0f)) * (1.0f - step(150.0f, r_0)));
    side_0 = int(-1);
    sum_0 = sum_1;
    for(;;)
    {
        if(side_0 <= int(1))
        {
        }
        else
        {
            break;
        }
        float2 p_2 = c_1 + float2(float(side_0))  * axis_0 * float2((r_0 * _S13)) ;
        float along_1 = dot(p_2, axis_0);
        float in_1 = step(_S10, along_1) * (1.0f - step(_S11, along_1));
        float weights_2 = weights_0 + _S20 * max(in_1, _S12);
        if(in_1 > 0.0f)
        {
            float4 _S21 = content_0(p_2, kernelContext_3);
            sum_0 = sum_0 + float4(_S20)  * _S21;
        }
        side_0 = side_0 + int(2);
        weights_0 = weights_2;
    }
    return sum_0 / float4(weights_0) ;
}

float tiled_0(float t_0, float low_0, float high_0, int tile_0)
{
    float n_0 = high_0 - low_0;
    bool _S22;
    if(t_0 >= low_0)
    {
        _S22 = t_0 < high_0;
    }
    else
    {
        _S22 = false;
    }
    if(_S22)
    {
        return t_0;
    }
    if(tile_0 == int(0))
    {
        return clamp(t_0, low_0, high_0 - 1.0f);
    }
    if(tile_0 == int(1))
    {
        float _S23 = t_0 - low_0;
        return low_0 + (_S23 - n_0 * floor((_S23 + 0.5f) / n_0));
    }
    if(tile_0 == int(2))
    {
        float _S24 = t_0 - low_0;
        float _S25 = 2.0f * n_0;
        float m_0 = _S24 - _S25 * floor((_S24 + 0.5f) / _S25);
        float _S26;
        if(m_0 < n_0)
        {
            _S26 = m_0;
        }
        else
        {
            _S26 = _S25 - 1.0f - m_0;
        }
        return low_0 + _S26;
    }
    return -1.0f;
}

float4 fetch_0(float2 t_1, KernelContext_0 thread* kernelContext_4)
{
    int tile_1 = int(kernelContext_4->uniforms_0->params_0.y + 0.5f);
    float x_1 = tiled_0(t_1.x, kernelContext_4->uniforms_0->valid_0.x, kernelContext_4->uniforms_0->valid_0.z, tile_1);
    float y_0 = tiled_0(t_1.y, kernelContext_4->uniforms_0->valid_0.y, kernelContext_4->uniforms_0->valid_0.w, tile_1);
    bool _S27;
    if(x_1 < 0.0f)
    {
        _S27 = true;
    }
    else
    {
        _S27 = y_0 < 0.0f;
    }
    if(_S27)
    {
        return float4(0.0f, 0.0f, 0.0f, 0.0f);
    }
    return ((kernelContext_4->source_texture_0).sample((kernelContext_4->source_sampler_0), ((float2(x_1, y_0) + float2(0.5f) ) * kernelContext_4->uniforms_0->extra_0.zw), level((0.0f))));
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    float2 texel_0 [[user(TEXEL)]];
};

[[fragment]] pixelOutput_0 fs(pixelInput_0 _S28 [[stage_in]], float4 position_0 [[position]], Uniforms_0 constant* uniforms_1 [[buffer(0)]], texture2d<float, access::sample> table_texture_1 [[texture(1)]], sampler table_sampler_1 [[sampler(1)]], texture2d<float, access::sample> source_texture_1 [[texture(0)]], sampler source_sampler_1 [[sampler(0)]])
{
    thread KernelContext_0 kernelContext_5;
    (&kernelContext_5)->uniforms_0 = uniforms_1;
    (&kernelContext_5)->table_texture_0 = table_texture_1;
    (&kernelContext_5)->table_sampler_0 = table_sampler_1;
    (&kernelContext_5)->source_texture_0 = source_texture_1;
    (&kernelContext_5)->source_sampler_0 = source_sampler_1;
    float2 t_2 = floor(_S28.texel_0);
    if(int(uniforms_1->params_0.x + 0.5f) == int(2))
    {
        float4 _S29 = progressive_0(t_2 + float2(0.5f) , &kernelContext_5);
        pixelOutput_0 _S30 = { _S29 };
        return _S30;
    }
    float2 first_0 = t_2 * (&kernelContext_5)->uniforms_0->map_0.xy + (&kernelContext_5)->uniforms_0->map_0.zw;
    int j_0;
    float4 sum_2;
    if(int(uniforms_1->params_0.x + 0.5f) == int(0))
    {
        int fx_0 = int((&kernelContext_5)->uniforms_0->map_0.x + 0.5f);
        int fy_0 = int((&kernelContext_5)->uniforms_0->map_0.y + 0.5f);
        float4 _S31 = float4(0.0f, 0.0f, 0.0f, 0.0f);
        j_0 = int(0);
        sum_2 = _S31;
        for(;;)
        {
            if(j_0 < fy_0)
            {
            }
            else
            {
                break;
            }
            int i_3 = int(0);
            for(;;)
            {
                if(i_3 < fx_0)
                {
                }
                else
                {
                    break;
                }
                float4 _S32 = fetch_0(first_0 + float2(float(i_3), float(j_0)), &kernelContext_5);
                float4 sum_3 = sum_2 + _S32;
                i_3 = i_3 + int(1);
                sum_2 = sum_3;
            }
            j_0 = j_0 + int(1);
        }
        pixelOutput_0 _S33 = { sum_2 / float4(float(fx_0 * fy_0))  };
        return _S33;
    }
    float2 _S34;
    if((uniforms_1->params_0.z) > 0.5f)
    {
        _S34 = float2(0.0f, 1.0f);
    }
    else
    {
        _S34 = float2(1.0f, 0.0f);
    }
    float _S35 = uniforms_1->params_0.w;
    int _S36 = int((&kernelContext_5)->uniforms_0->extra_0.x + 0.5f);
    float4 _S37 = fetch_0(first_0, &kernelContext_5);
    j_0 = int(1);
    sum_2 = _S37;
    float weights_3 = 1.0f;
    for(;;)
    {
        if(j_0 <= _S36)
        {
        }
        else
        {
            break;
        }
        float w_1 = exp(- float(j_0 * j_0) / (2.0f * _S35 * _S35));
        float2 _S38 = _S34 * float2(float(j_0)) ;
        float4 _S39 = fetch_0(first_0 - _S38, &kernelContext_5);
        float4 _S40 = fetch_0(first_0 + _S38, &kernelContext_5);
        float4 sum_4 = sum_2 + float4(w_1)  * (_S39 + _S40);
        float weights_4 = weights_3 + 2.0f * w_1;
        j_0 = j_0 + int(1);
        sum_2 = sum_4;
        weights_3 = weights_4;
    }
    pixelOutput_0 _S41 = { sum_2 / float4(weights_3)  };
    return _S41;
}

