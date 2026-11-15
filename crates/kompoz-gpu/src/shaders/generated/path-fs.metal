#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
float rampIntegral_0(float t_0)
{
    float _S1;
    if(t_0 <= 0.0f)
    {
        _S1 = t_0;
    }
    else
    {
        if(t_0 < 1.0f)
        {
            _S1 = t_0 - 0.5f * t_0 * t_0;
        }
        else
        {
            _S1 = 0.5f;
        }
    }
    return _S1;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    [[flat]] float4 edge_0 [[user(EDGE)]];
    [[flat]] float aliased_0 [[user(ALIASED)]];
};

[[fragment]] pixelOutput_0 fs(pixelInput_0 _S2 [[stage_in]], float4 position_0 [[position]])
{
    float2 cell_0 = floor(position_0.xy);
    float _S3 = _S2.edge_0.w;
    float _S4 = _S2.edge_0.y;
    int _S5;
    if(_S3 > _S4)
    {
        _S5 = int(1);
    }
    else
    {
        _S5 = int(-1);
    }
    float sign_0 = float(_S5);
    float2 from_0;
    float crossing_0;
    if((_S2.aliased_0) > 0.5f)
    {
        float2 middle_0 = cell_0 + float2(0.5f) ;
        float _S6 = max(_S4, _S3);
        float _S7 = middle_0.y;
        bool _S8;
        if((min(_S4, _S3)) < _S7)
        {
            _S8 = _S7 <= _S6;
        }
        else
        {
            _S8 = false;
        }
        if(_S8)
        {
            if((abs(_S3 - _S7)) < (abs(_S4 - _S7)))
            {
                from_0 = _S2.edge_0.zw;
            }
            else
            {
                from_0 = _S2.edge_0.xy;
            }
            if((from_0.x + (_S7 - from_0.y) * (_S2.edge_0.z - _S2.edge_0.x) / (_S3 - _S4)) < (middle_0.x))
            {
                crossing_0 = sign_0;
            }
            else
            {
                crossing_0 = 0.0f;
            }
        }
        else
        {
            crossing_0 = 0.0f;
        }
        pixelOutput_0 _S9 = { float4(crossing_0, 0.0f, 0.0f, 0.0f) };
        return _S9;
    }
    float _S10 = cell_0.y;
    float _S11 = max(min(_S4, _S3), _S10);
    float _S12 = min(max(_S4, _S3), _S10 + 1.0f);
    float height_0 = _S12 - _S11;
    if(height_0 <= 0.0f)
    {
        pixelOutput_0 _S13 = { float4(0.0f, 0.0f, 0.0f, 0.0f) };
        return _S13;
    }
    float dxdy_0 = (_S2.edge_0.z - _S2.edge_0.x) / (_S3 - _S4);
    if((abs(_S3 - _S11)) < (abs(_S4 - _S11)))
    {
        from_0 = _S2.edge_0.zw;
    }
    else
    {
        from_0 = _S2.edge_0.xy;
    }
    float _S14 = from_0.x;
    float _S15 = from_0.y;
    float _S16 = cell_0.x;
    float xa_0 = _S14 + (_S11 - _S15) * dxdy_0 - _S16;
    float xb_0 = _S14 + (_S12 - _S15) * dxdy_0 - _S16;
    float _S17 = xb_0 - xa_0;
    if((abs(_S17)) < 9.9999997473787516e-06f)
    {
        crossing_0 = saturate(1.0f - xa_0);
    }
    else
    {
        crossing_0 = (rampIntegral_0(xb_0) - rampIntegral_0(xa_0)) / _S17;
    }
    pixelOutput_0 _S18 = { float4(sign_0 * height_0 * crossing_0, 0.0f, 0.0f, 0.0f) };
    return _S18;
}

