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
@binding(3) @group(0) var table_texture_0 : texture_2d<f32>;

@binding(4) @group(0) var table_sampler_0 : sampler;

@binding(1) @group(0) var source_texture_0 : texture_2d<f32>;

@binding(2) @group(0) var source_sampler_0 : sampler;

fn fraction_0( c_0 : vec2<f32>) -> f32
{
    var _S1 : vec2<f32> = max(c_0 - uniforms_0.origin_0.xy, vec2<f32>(vec2<i32>(i32(0))));
    var g_0 : vec4<f32> = uniforms_0.geometry_0;
    var _S2 : f32;
    if(((uniforms_0.mask_0.x) < 1.5f))
    {
        var _S3 : vec2<f32> = g_0.xy;
        var ba_0 : vec2<f32> = g_0.zw - _S3;
        var l_0 : f32 = dot(ba_0, ba_0);
        if((l_0 > 0.0f))
        {
            _S2 = saturate(dot(_S1 - _S3, ba_0) / l_0);
        }
        else
        {
            _S2 = 0.0f;
        }
        return _S2;
    }
    var _S4 : f32 = g_0.z;
    if((_S4 > 0.0f))
    {
        _S2 = saturate(distance(_S1, g_0.xy) / _S4);
    }
    else
    {
        _S2 = 0.0f;
    }
    return _S2;
}

fn intensity_0( fraction_1 : f32) -> f32
{
    var between_0 : bool;
    var result_0 : f32;
    var _S5 : i32 = i32(uniforms_0.mask_0.z + 0.5f);
    var row_0 : i32 = i32(uniforms_0.mask_0.y + 0.5f);
    var _S6 : vec4<f32> = (textureLoad((table_texture_0), (vec2<i32>(i32(0), row_0)), 0));
    var a_0 : vec2<f32> = _S6.xy;
    if((fraction_1 <= (a_0.x)))
    {
        return a_0.y;
    }
    var a_1 : vec2<f32> = a_0;
    var i_0 : i32 = i32(1);
    loop
    {
        if((i_0 < _S5))
        {
        }
        else
        {
            between_0 = false;
            result_0 = 0.0f;
            break;
        }
        var _S7 : vec4<f32> = (textureLoad((table_texture_0), (vec2<i32>(i_0, row_0)), 0));
        var b_0 : vec2<f32> = _S7.xy;
        var _S8 : f32 = a_1.x;
        if((fraction_1 >= _S8))
        {
            between_0 = (fraction_1 <= (b_0.x));
        }
        else
        {
            between_0 = false;
        }
        if(between_0)
        {
            var _S9 : f32 = mix(a_1.y, b_0.y, saturate((fraction_1 - _S8) / max(b_0.x - _S8, 9.9999997473787516e-05f)));
            between_0 = true;
            result_0 = _S9;
            break;
        }
        var i_1 : i32 = i_0 + i32(1);
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

fn content_0( p_0 : vec2<f32>) -> vec4<f32>
{
    return (textureSampleLevel((source_texture_0), (source_sampler_0), ((p_0 * uniforms_0.map_0.xy + uniforms_0.map_0.zw) * uniforms_0.extra_0.zw), (0.0f)));
}

fn gaussian_0( x_0 : f32,  sigma_0 : f32) -> f32
{
    return exp(- (x_0 * x_0) / (2.0f * sigma_0 * sigma_0));
}

fn progressive_0( c_1 : vec2<f32>) -> vec4<f32>
{
    var sum_0 : vec4<f32>;
    var axis_0 : vec2<f32>;
    if(((uniforms_0.params_0.z) > 0.5f))
    {
        axis_0 = vec2<f32>(0.0f, 1.0f);
    }
    else
    {
        axis_0 = vec2<f32>(1.0f, 0.0f);
    }
    var _S10 : f32 = dot(uniforms_0.valid_0.xy, axis_0);
    var _S11 : f32 = dot(uniforms_0.valid_0.zw, axis_0);
    var side_0 : i32;
    if(((uniforms_0.params_0.y) > 2.5f))
    {
        side_0 = i32(1);
    }
    else
    {
        side_0 = i32(0);
    }
    var _S12 : f32 = f32(side_0);
    var _S13 : f32 = uniforms_0.extra_0.x;
    var over_0 : f32;
    if(((uniforms_0.mask_0.x) > 2.5f))
    {
        over_0 = (textureSampleLevel((table_texture_0), (table_sampler_0), (c_1 * uniforms_0.origin_0.zw), (0.0f))).w;
    }
    else
    {
        var _S14 : f32 = intensity_0(fraction_0(c_1));
        over_0 = _S14;
    }
    var radius_0 : f32 = uniforms_0.params_0.w * clamp(over_0, 0.0f, 1.0f);
    var r_0 : f32 = floor(radius_0);
    var sum_1 : vec4<f32> = content_0(c_1);
    if((r_0 < 1.0f))
    {
        return sum_1;
    }
    var _S15 : f32 = max(radius_0 / 2.0f, 1.0f);
    var i_2 : f32 = 1.0f;
    var weights_0 : f32 = 1.0f;
    var sum_2 : vec4<f32> = sum_1;
    loop
    {
        if((i_2 < 150.0f))
        {
        }
        else
        {
            break;
        }
        if((i_2 >= r_0))
        {
            break;
        }
        var high1_0 : f32 = gaussian_0(i_2 + 1.0f, _S15);
        var w_0 : f32 = gaussian_0(i_2, _S15) + high1_0;
        var _S16 : vec2<f32> = axis_0 * vec2<f32>(((i_2 + high1_0 / w_0) * _S13));
        side_0 = i32(-1);
        sum_0 = sum_2;
        loop
        {
            if((side_0 <= i32(1)))
            {
            }
            else
            {
                break;
            }
            var p_1 : vec2<f32> = c_1 + vec2<f32>(f32(side_0)) * _S16;
            var along_0 : f32 = dot(p_1, axis_0);
            var in_0 : f32 = step(_S10, along_0) * (1.0f - step(_S11, along_0));
            var weights_1 : f32 = weights_0 + w_0 * max(in_0, _S12);
            if((in_0 > 0.0f))
            {
                sum_0 = sum_0 + vec4<f32>(w_0) * content_0(p_1);
            }
            side_0 = side_0 + i32(2);
            weights_0 = weights_1;
        }
        i_2 = i_2 + 2.0f;
        sum_2 = sum_0;
    }
    var _S17 : f32 = gaussian_0(r_0, _S15) * ((r_0 - 2.0f * floor(r_0 / 2.0f)) * (1.0f - step(150.0f, r_0)));
    side_0 = i32(-1);
    sum_0 = sum_2;
    loop
    {
        if((side_0 <= i32(1)))
        {
        }
        else
        {
            break;
        }
        var p_2 : vec2<f32> = c_1 + vec2<f32>(f32(side_0)) * axis_0 * vec2<f32>((r_0 * _S13));
        var along_1 : f32 = dot(p_2, axis_0);
        var in_1 : f32 = step(_S10, along_1) * (1.0f - step(_S11, along_1));
        var weights_2 : f32 = weights_0 + _S17 * max(in_1, _S12);
        if((in_1 > 0.0f))
        {
            sum_0 = sum_0 + vec4<f32>(_S17) * content_0(p_2);
        }
        side_0 = side_0 + i32(2);
        weights_0 = weights_2;
    }
    return sum_0 / vec4<f32>(weights_0);
}

fn tiled_0( t_0 : f32,  low_0 : f32,  high_0 : f32,  tile_0 : i32) -> f32
{
    var n_0 : f32 = high_0 - low_0;
    var _S18 : bool;
    if((t_0 >= low_0))
    {
        _S18 = (t_0 < high_0);
    }
    else
    {
        _S18 = false;
    }
    if(_S18)
    {
        return t_0;
    }
    if(tile_0 == i32(0))
    {
        return clamp(t_0, low_0, high_0 - 1.0f);
    }
    if(tile_0 == i32(1))
    {
        var _S19 : f32 = t_0 - low_0;
        return low_0 + (_S19 - n_0 * floor((_S19 + 0.5f) / n_0));
    }
    if(tile_0 == i32(2))
    {
        var _S20 : f32 = t_0 - low_0;
        var _S21 : f32 = 2.0f * n_0;
        var m_0 : f32 = _S20 - _S21 * floor((_S20 + 0.5f) / _S21);
        var _S22 : f32;
        if((m_0 < n_0))
        {
            _S22 = m_0;
        }
        else
        {
            _S22 = _S21 - 1.0f - m_0;
        }
        return low_0 + _S22;
    }
    return -1.0f;
}

fn fetch_0( t_1 : vec2<f32>) -> vec4<f32>
{
    var tile_1 : i32 = i32(uniforms_0.params_0.y + 0.5f);
    var x_1 : f32 = tiled_0(t_1.x, uniforms_0.valid_0.x, uniforms_0.valid_0.z, tile_1);
    var y_0 : f32 = tiled_0(t_1.y, uniforms_0.valid_0.y, uniforms_0.valid_0.w, tile_1);
    var _S23 : bool;
    if((x_1 < 0.0f))
    {
        _S23 = true;
    }
    else
    {
        _S23 = (y_0 < 0.0f);
    }
    if(_S23)
    {
        return vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
    }
    return (textureSampleLevel((source_texture_0), (source_sampler_0), ((vec2<f32>(x_1, y_0) + vec2<f32>(0.5f)) * uniforms_0.extra_0.zw), (0.0f)));
}

struct pixelOutput_0
{
    @location(0) output_0 : vec4<f32>,
};

struct pixelInput_0
{
    @location(0) texel_0 : vec2<f32>,
};

@fragment
fn fs( _S24 : pixelInput_0, @builtin(position) position_0 : vec4<f32>) -> pixelOutput_0
{
    var t_2 : vec2<f32> = floor(_S24.texel_0);
    if(i32(uniforms_0.params_0.x + 0.5f) == i32(2))
    {
        var _S25 : vec4<f32> = progressive_0(t_2 + vec2<f32>(0.5f));
        var _S26 : pixelOutput_0 = pixelOutput_0( _S25 );
        return _S26;
    }
    var first_0 : vec2<f32> = t_2 * uniforms_0.map_0.xy + uniforms_0.map_0.zw;
    var j_0 : i32;
    var sum_3 : vec4<f32>;
    if(i32(uniforms_0.params_0.x + 0.5f) == i32(0))
    {
        var fx_0 : i32 = i32(uniforms_0.map_0.x + 0.5f);
        var fy_0 : i32 = i32(uniforms_0.map_0.y + 0.5f);
        const _S27 : vec4<f32> = vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
        j_0 = i32(0);
        sum_3 = _S27;
        loop
        {
            if((j_0 < fy_0))
            {
            }
            else
            {
                break;
            }
            var i_3 : i32 = i32(0);
            loop
            {
                if((i_3 < fx_0))
                {
                }
                else
                {
                    break;
                }
                var sum_4 : vec4<f32> = sum_3 + fetch_0(first_0 + vec2<f32>(f32(i_3), f32(j_0)));
                i_3 = i_3 + i32(1);
                sum_3 = sum_4;
            }
            j_0 = j_0 + i32(1);
        }
        var _S28 : pixelOutput_0 = pixelOutput_0( sum_3 / vec4<f32>(f32(fx_0 * fy_0)) );
        return _S28;
    }
    var _S29 : vec2<f32>;
    if(((uniforms_0.params_0.z) > 0.5f))
    {
        _S29 = vec2<f32>(0.0f, 1.0f);
    }
    else
    {
        _S29 = vec2<f32>(1.0f, 0.0f);
    }
    var _S30 : f32 = uniforms_0.params_0.w;
    var _S31 : i32 = i32(uniforms_0.extra_0.x + 0.5f);
    var _S32 : vec4<f32> = fetch_0(first_0);
    j_0 = i32(1);
    sum_3 = _S32;
    var weights_3 : f32 = 1.0f;
    loop
    {
        if((j_0 <= _S31))
        {
        }
        else
        {
            break;
        }
        var w_1 : f32 = exp(- f32(j_0 * j_0) / (2.0f * _S30 * _S30));
        var _S33 : vec2<f32> = _S29 * vec2<f32>(f32(j_0));
        var sum_5 : vec4<f32> = sum_3 + vec4<f32>(w_1) * (fetch_0(first_0 - _S33) + fetch_0(first_0 + _S33));
        var weights_4 : f32 = weights_3 + 2.0f * w_1;
        j_0 = j_0 + i32(1);
        sum_3 = sum_5;
        weights_3 = weights_4;
    }
    var _S34 : pixelOutput_0 = pixelOutput_0( sum_3 / vec4<f32>(weights_3) );
    return _S34;
}

