diagnostic(off, derivative_uniformity);
fn rampIntegral_0( t_0 : f32) -> f32
{
    var _S1 : f32;
    if((t_0 <= 0.0f))
    {
        _S1 = t_0;
    }
    else
    {
        if((t_0 < 1.0f))
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
    @location(0) output_0 : vec4<f32>,
};

struct pixelInput_0
{
    @interpolate(flat) @location(0) edge_0 : vec4<f32>,
    @interpolate(flat) @location(1) aliased_0 : f32,
};

@fragment
fn fs( _S2 : pixelInput_0, @builtin(position) position_0 : vec4<f32>) -> pixelOutput_0
{
    var cell_0 : vec2<f32> = floor(position_0.xy);
    var _S3 : f32 = _S2.edge_0.w;
    var _S4 : f32 = _S2.edge_0.y;
    var _S5 : i32;
    if((_S3 > _S4))
    {
        _S5 = i32(1);
    }
    else
    {
        _S5 = i32(-1);
    }
    var sign_0 : f32 = f32(_S5);
    var from_0 : vec2<f32>;
    var crossing_0 : f32;
    if(((_S2.aliased_0) > 0.5f))
    {
        var middle_0 : vec2<f32> = cell_0 + vec2<f32>(0.5f);
        var _S6 : f32 = max(_S4, _S3);
        var _S7 : f32 = middle_0.y;
        var _S8 : bool;
        if(((min(_S4, _S3)) < _S7))
        {
            _S8 = (_S7 <= _S6);
        }
        else
        {
            _S8 = false;
        }
        if(_S8)
        {
            if(((abs(_S3 - _S7)) < (abs(_S4 - _S7))))
            {
                from_0 = _S2.edge_0.zw;
            }
            else
            {
                from_0 = _S2.edge_0.xy;
            }
            if(((from_0.x + (_S7 - from_0.y) * (_S2.edge_0.z - _S2.edge_0.x) / (_S3 - _S4)) < (middle_0.x)))
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
        var _S9 : pixelOutput_0 = pixelOutput_0( vec4<f32>(crossing_0, 0.0f, 0.0f, 0.0f) );
        return _S9;
    }
    var _S10 : f32 = cell_0.y;
    var _S11 : f32 = max(min(_S4, _S3), _S10);
    var _S12 : f32 = min(max(_S4, _S3), _S10 + 1.0f);
    var height_0 : f32 = _S12 - _S11;
    if((height_0 <= 0.0f))
    {
        var _S13 : pixelOutput_0 = pixelOutput_0( vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f) );
        return _S13;
    }
    var dxdy_0 : f32 = (_S2.edge_0.z - _S2.edge_0.x) / (_S3 - _S4);
    if(((abs(_S3 - _S11)) < (abs(_S4 - _S11))))
    {
        from_0 = _S2.edge_0.zw;
    }
    else
    {
        from_0 = _S2.edge_0.xy;
    }
    var _S14 : f32 = from_0.x;
    var _S15 : f32 = from_0.y;
    var _S16 : f32 = cell_0.x;
    var xa_0 : f32 = _S14 + (_S11 - _S15) * dxdy_0 - _S16;
    var xb_0 : f32 = _S14 + (_S12 - _S15) * dxdy_0 - _S16;
    var _S17 : f32 = xb_0 - xa_0;
    if(((abs(_S17)) < 9.9999997473787516e-06f))
    {
        crossing_0 = saturate(1.0f - xa_0);
    }
    else
    {
        crossing_0 = (rampIntegral_0(xb_0) - rampIntegral_0(xa_0)) / _S17;
    }
    var _S18 : pixelOutput_0 = pixelOutput_0( vec4<f32>(sign_0 * height_0 * crossing_0, 0.0f, 0.0f, 0.0f) );
    return _S18;
}

