diagnostic(off, derivative_uniformity);
@binding(1) @group(0) var texture0_texture_0 : texture_2d<f32>;

@binding(2) @group(0) var texture0_sampler_0 : sampler;

struct Uniforms_std430_0
{
    @align(16) toClip_0 : vec4<f32>,
    @align(16) texels_0 : vec4<f32>,
    @align(16) blend_0 : vec4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms_0 : Uniforms_std430_0;
@binding(7) @group(0) var texture3_texture_0 : texture_2d<f32>;

@binding(8) @group(0) var texture3_sampler_0 : sampler;

@binding(5) @group(0) var texture2_texture_0 : texture_2d<f32>;

@binding(6) @group(0) var texture2_sampler_0 : sampler;

@binding(11) @group(0) var texture5_texture_0 : texture_2d<f32>;

@binding(12) @group(0) var texture5_sampler_0 : sampler;

@binding(3) @group(0) var texture1_texture_0 : texture_2d<f32>;

@binding(4) @group(0) var texture1_sampler_0 : sampler;

@binding(9) @group(0) var texture4_texture_0 : texture_2d<f32>;

@binding(10) @group(0) var texture4_sampler_0 : sampler;

@id(3) override FILTERED_0 : i32 = i32(1);

@id(2) override CLIPPED_0 : i32 = i32(1);

@id(1) override PAINTS_0 : i32 = i32(63);

@id(5) override FLAGS_0 : i32 = i32(255);

@id(4) override HOW_0 : i32 = i32(-1);

@id(0) override KINDS_0 : i32 = i32(2047);

fn drawn_0( kind_0 : i32) -> bool
{
    return ((KINDS_0 & (((i32(1) << (u32(kind_0))))))) != i32(0);
}

fn sat_0( c_0 : vec3<f32>) -> f32
{
    var _S1 : f32 = c_0.x;
    var _S2 : f32 = c_0.y;
    var _S3 : f32 = c_0.z;
    return max(_S1, max(_S2, _S3)) - min(_S1, min(_S2, _S3));
}

fn setSat_0( c_1 : vec3<f32>,  s_0 : f32) -> vec3<f32>
{
    var _S4 : f32 = c_1.x;
    var _S5 : f32 = c_1.y;
    var _S6 : f32 = c_1.z;
    var _S7 : f32 = min(_S4, min(_S5, _S6));
    var _S8 : f32 = max(_S4, max(_S5, _S6));
    var _S9 : vec3<f32>;
    if((_S8 > _S7))
    {
        _S9 = (c_1 - vec3<f32>(_S7)) * vec3<f32>(s_0) / vec3<f32>((_S8 - _S7));
    }
    else
    {
        _S9 = vec3<f32>(0.0f, 0.0f, 0.0f);
    }
    return _S9;
}

fn lum_0( c_2 : vec3<f32>) -> f32
{
    return dot(c_2, vec3<f32>(0.30000001192092896f, 0.5899999737739563f, 0.10999999940395355f));
}

fn clipColor_0( c_3 : vec3<f32>) -> vec3<f32>
{
    var l_0 : f32 = lum_0(c_3);
    var _S10 : f32 = c_3.x;
    var _S11 : f32 = c_3.y;
    var _S12 : f32 = c_3.z;
    var _S13 : f32 = min(_S10, min(_S11, _S12));
    var _S14 : f32 = max(_S10, max(_S11, _S12));
    var _S15 : bool;
    if((_S13 < 0.0f))
    {
        _S15 = ((l_0 - _S13) > 0.0f);
    }
    else
    {
        _S15 = false;
    }
    var out_0 : vec3<f32>;
    if(_S15)
    {
        var _S16 : vec3<f32> = vec3<f32>(l_0);
        out_0 = _S16 + (c_3 - _S16) * _S16 / vec3<f32>((l_0 - _S13));
    }
    else
    {
        out_0 = c_3;
    }
    if((_S14 > 1.0f))
    {
        _S15 = ((_S14 - l_0) > 0.0f);
    }
    else
    {
        _S15 = false;
    }
    if(_S15)
    {
        var _S17 : vec3<f32> = vec3<f32>(l_0);
        out_0 = _S17 + (out_0 - _S17) * vec3<f32>((1.0f - l_0)) / vec3<f32>((_S14 - l_0));
    }
    return out_0;
}

fn setLum_0( c_4 : vec3<f32>,  l_1 : f32) -> vec3<f32>
{
    return clipColor_0(c_4 + vec3<f32>((l_1 - lum_0(c_4))));
}

fn hardLight_0( s_1 : f32,  b_0 : f32) -> f32
{
    var _S18 : f32;
    if((s_1 <= 0.5f))
    {
        _S18 = b_0 * 2.0f * s_1;
    }
    else
    {
        var _S19 : f32 = 2.0f * s_1 - 1.0f;
        _S18 = b_0 + _S19 - b_0 * _S19;
    }
    return _S18;
}

fn separable_0( mode_0 : i32,  cs_0 : f32,  cb_0 : f32) -> f32
{
    var r_0 : f32;
    if(mode_0 == i32(6))
    {
        r_0 = cs_0 * cb_0;
    }
    else
    {
        if(mode_0 == i32(7))
        {
            r_0 = cs_0 + cb_0 - cs_0 * cb_0;
        }
        else
        {
            if(mode_0 == i32(8))
            {
                r_0 = min(cs_0, cb_0);
            }
            else
            {
                if(mode_0 == i32(9))
                {
                    r_0 = max(cs_0, cb_0);
                }
                else
                {
                    if(mode_0 == i32(18))
                    {
                        r_0 = hardLight_0(cb_0, cs_0);
                    }
                    else
                    {
                        if(mode_0 == i32(19))
                        {
                            if((cb_0 <= 0.0f))
                            {
                                r_0 = 0.0f;
                            }
                            else
                            {
                                if((cs_0 >= 1.0f))
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
                            if(mode_0 == i32(20))
                            {
                                if((cb_0 >= 1.0f))
                                {
                                    r_0 = 1.0f;
                                }
                                else
                                {
                                    if((cs_0 <= 0.0f))
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
                                if(mode_0 == i32(21))
                                {
                                    r_0 = hardLight_0(cs_0, cb_0);
                                }
                                else
                                {
                                    if(mode_0 == i32(22))
                                    {
                                        if((cb_0 <= 0.25f))
                                        {
                                            r_0 = ((16.0f * cb_0 - 12.0f) * cb_0 + 4.0f) * cb_0;
                                        }
                                        else
                                        {
                                            r_0 = sqrt(cb_0);
                                        }
                                        if((cs_0 <= 0.5f))
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
                                        if(mode_0 == i32(23))
                                        {
                                            r_0 = abs(cs_0 - cb_0);
                                        }
                                        else
                                        {
                                            if(mode_0 == i32(24))
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

fn blendColors_0( s_2 : vec4<f32>,  d_0 : vec4<f32>,  mode_1 : i32) -> vec4<f32>
{
    var sa_0 : f32 = s_2.w;
    var da_0 : f32 = d_0.w;
    var cs_1 : vec3<f32>;
    if((sa_0 > 0.0f))
    {
        cs_1 = s_2.xyz / vec3<f32>(sa_0);
    }
    else
    {
        cs_1 = vec3<f32>(0.0f, 0.0f, 0.0f);
    }
    var cb_1 : vec3<f32>;
    if((da_0 > 0.0f))
    {
        cb_1 = d_0.xyz / vec3<f32>(da_0);
    }
    else
    {
        cb_1 = vec3<f32>(0.0f, 0.0f, 0.0f);
    }
    var fs_0 : f32;
    var fd_0 : f32;
    if(mode_1 == i32(11))
    {
        fs_0 = 0.0f;
        fd_0 = 0.0f;
    }
    else
    {
        if(mode_1 == i32(0))
        {
            fs_0 = 1.0f;
            fd_0 = 0.0f;
        }
        else
        {
            if(mode_1 == i32(12))
            {
                fs_0 = 0.0f;
                fd_0 = 1.0f;
            }
            else
            {
                if(mode_1 == i32(1))
                {
                    var _S20 : f32 = 1.0f - sa_0;
                    fs_0 = 1.0f;
                    fd_0 = _S20;
                }
                else
                {
                    if(mode_1 == i32(13))
                    {
                        fs_0 = 1.0f - da_0;
                        fd_0 = 1.0f;
                    }
                    else
                    {
                        if(mode_1 == i32(2))
                        {
                            fs_0 = da_0;
                            fd_0 = 0.0f;
                        }
                        else
                        {
                            if(mode_1 == i32(4))
                            {
                                fs_0 = 0.0f;
                                fd_0 = sa_0;
                            }
                            else
                            {
                                if(mode_1 == i32(14))
                                {
                                    fs_0 = 1.0f - da_0;
                                    fd_0 = 0.0f;
                                }
                                else
                                {
                                    if(mode_1 == i32(15))
                                    {
                                        var _S21 : f32 = 1.0f - sa_0;
                                        fs_0 = 0.0f;
                                        fd_0 = _S21;
                                    }
                                    else
                                    {
                                        if(mode_1 == i32(3))
                                        {
                                            var _S22 : f32 = 1.0f - sa_0;
                                            fs_0 = da_0;
                                            fd_0 = _S22;
                                        }
                                        else
                                        {
                                            if(mode_1 == i32(16))
                                            {
                                                fs_0 = 1.0f - da_0;
                                                fd_0 = sa_0;
                                            }
                                            else
                                            {
                                                if(mode_1 == i32(17))
                                                {
                                                    var _S23 : f32 = 1.0f - sa_0;
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
    var out_1 : vec4<f32>;
    if((fs_0 >= 0.0f))
    {
        out_1 = s_2 * vec4<f32>(fs_0) + d_0 * vec4<f32>(fd_0);
    }
    else
    {
        if(mode_1 == i32(10))
        {
            out_1 = min(s_2 + d_0, vec4<f32>(1.0f, 1.0f, 1.0f, 1.0f));
        }
        else
        {
            if(mode_1 == i32(5))
            {
                out_1 = s_2 * d_0;
            }
            else
            {
                var b_1 : vec3<f32>;
                if(mode_1 == i32(25))
                {
                    b_1 = setLum_0(setSat_0(cs_1, sat_0(cb_1)), lum_0(cb_1));
                }
                else
                {
                    if(mode_1 == i32(26))
                    {
                        b_1 = setLum_0(setSat_0(cb_1, sat_0(cs_1)), lum_0(cb_1));
                    }
                    else
                    {
                        if(mode_1 == i32(27))
                        {
                            b_1 = setLum_0(cs_1, lum_0(cb_1));
                        }
                        else
                        {
                            if(mode_1 == i32(28))
                            {
                                b_1 = setLum_0(cb_1, lum_0(cs_1));
                            }
                            else
                            {
                                b_1 = vec3<f32>(separable_0(mode_1, cs_1.x, cb_1.x), separable_0(mode_1, cs_1.y, cb_1.y), separable_0(mode_1, cs_1.z, cb_1.z));
                            }
                        }
                    }
                }
                var _S24 : f32 = sa_0 * da_0;
                out_1 = vec4<f32>(s_2.xyz * vec3<f32>((1.0f - da_0)) + d_0.xyz * vec3<f32>((1.0f - sa_0)) + vec3<f32>(_S24) * b_1, sa_0 + da_0 - _S24);
            }
        }
    }
    return out_1;
}

fn finish_0( src_0 : vec4<f32>,  cover_0 : f32,  pixel_0 : vec2<f32>) -> vec4<f32>
{
    var how_0 : i32;
    if((HOW_0 >= i32(0)))
    {
        how_0 = HOW_0;
    }
    else
    {
        how_0 = i32(uniforms_0.blend_0.y + 0.5f);
    }
    var _S25 : vec4<f32> = vec4<f32>(cover_0);
    var _S26 : vec4<f32> = src_0 * _S25;
    var out_2 : vec4<f32>;
    if(how_0 == i32(1))
    {
        out_2 = mix(vec4<f32>(1.0f, 1.0f, 1.0f, 1.0f), src_0, _S25);
    }
    else
    {
        if(how_0 == i32(2))
        {
            out_2 = vec4<f32>(0.0f, 0.0f, 0.0f, mix(1.0f, src_0.w, cover_0));
        }
        else
        {
            if(how_0 == i32(3))
            {
                out_2 = vec4<f32>(0.0f, 0.0f, 0.0f, cover_0);
            }
            else
            {
                if(how_0 == i32(4))
                {
                    var _S27 : vec4<f32> = (textureSampleLevel((texture3_texture_0), (texture3_sampler_0), (pixel_0 * uniforms_0.blend_0.zw), (0.0f)));
                    out_2 = mix(_S27, blendColors_0(src_0, _S27, i32(uniforms_0.blend_0.x + 0.5f)), _S25);
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

fn paints_0( paint_0 : i32) -> bool
{
    return ((PAINTS_0 & (((i32(1) << (u32(paint_0))))))) != i32(0);
}

fn tableWidth_0() -> f32
{
    return round(1.0f / uniforms_0.texels_0.z);
}

fn tableTexel_0( x_0 : f32,  row_0 : f32) -> vec4<f32>
{
    var _S28 : vec4<f32> = (textureLoad((texture2_texture_0), (vec2<i32>(i32(x_0 + 0.5f), i32(row_0 + 0.5f))), 0));
    return _S28;
}

fn paintOne_0( p_0 : vec2<f32>,  gx_0 : vec2<f32>,  gy_0 : vec2<f32>,  color_0 : vec4<f32>,  paint_1 : vec4<f32>,  g_0 : vec4<f32>,  second_0 : bool) -> vec4<f32>
{
    var kind_1 : i32 = i32(paint_1.x + 0.5f);
    var found_0 : bool;
    if(kind_1 == i32(0))
    {
        found_0 = true;
    }
    else
    {
        found_0 = ((PAINTS_0 & (i32(-2)))) == i32(0);
    }
    if(found_0)
    {
        return color_0;
    }
    var bits_0 : i32 = i32(paint_1.y + 0.5f);
    var row_1 : f32 = paint_1.w;
    var _S29 : vec2<f32>;
    var _S30 : vec2<f32>;
    var _S31 : vec2<f32>;
    if(((bits_0 & (i32(8)))) != i32(0))
    {
        var h_0 : vec3<f32> = vec3<f32>(p_0, 1.0f);
        var last_0 : f32 = tableWidth_0() - 1.0f;
        var _S32 : vec4<f32> = tableTexel_0(last_0 - 2.0f, row_1);
        var r0_0 : vec3<f32> = _S32.xyz;
        var _S33 : vec4<f32> = tableTexel_0(last_0 - 1.0f, row_1);
        var r1_0 : vec3<f32> = _S33.xyz;
        var _S34 : vec4<f32> = tableTexel_0(last_0, row_1);
        var r2_0 : vec3<f32> = _S34.xyz;
        var _S35 : vec2<f32> = vec2<f32>(dot(r2_0, h_0));
        var _S36 : vec2<f32> = vec2<f32>(dot(r0_0, h_0), dot(r1_0, h_0)) / _S35;
        var _S37 : vec2<f32> = r2_0.xy;
        var jx_0 : vec2<f32> = (r0_0.xy - vec2<f32>(_S36.x) * _S37) / _S35;
        var jy_0 : vec2<f32> = (r1_0.xy - vec2<f32>(_S36.y) * _S37) / _S35;
        var _S38 : vec2<f32> = vec2<f32>(dot(jx_0, gx_0), dot(jy_0, gx_0));
        var _S39 : vec2<f32> = vec2<f32>(dot(jx_0, gy_0), dot(jy_0, gy_0));
        _S29 = _S36;
        _S30 = _S38;
        _S31 = _S39;
    }
    else
    {
        _S29 = p_0;
        _S30 = gx_0;
        _S31 = gy_0;
    }
    if(paints_0(i32(4)))
    {
        found_0 = kind_1 == i32(4);
    }
    else
    {
        found_0 = false;
    }
    var t_0 : f32;
    var before_0 : f32;
    var texel_0 : vec4<f32>;
    if(found_0)
    {
        if(second_0)
        {
            texel_0 = (textureSampleGrad((texture5_texture_0), (texture5_sampler_0), (_S29), (_S30), (_S31)));
        }
        else
        {
            texel_0 = (textureSampleGrad((texture1_texture_0), (texture1_sampler_0), (_S29), (_S30), (_S31)));
        }
        if(((bits_0 & (i32(48)))) != i32(0))
        {
            var size_0 : vec2<f32>;
            if(second_0)
            {
                var _S40 : f32 = size_0[i32(0)];
                var _S41 : f32 = size_0[i32(1)];
                {var dim = textureDimensions((texture5_texture_0));((_S40)) = f32(dim.x);((_S41)) = f32(dim.y);};
                size_0[i32(0)] = _S40;
                size_0[i32(1)] = _S41;
            }
            else
            {
                var _S42 : f32 = size_0[i32(0)];
                var _S43 : f32 = size_0[i32(1)];
                {var dim = textureDimensions((texture1_texture_0));((_S42)) = f32(dim.x);((_S43)) = f32(dim.y);};
                size_0[i32(0)] = _S42;
                size_0[i32(1)] = _S43;
            }
            var at_0 : vec2<f32> = _S29 * size_0;
            var fade_0 : vec2<f32> = saturate(vec2<f32>(1.0f) - abs(at_0 - clamp(at_0, vec2<f32>(0.5f), size_0 - vec2<f32>(0.5f))));
            if(((bits_0 & (i32(16)))) != i32(0))
            {
                t_0 = fade_0.x;
            }
            else
            {
                t_0 = 1.0f;
            }
            if(((bits_0 & (i32(32)))) != i32(0))
            {
                before_0 = fade_0.y;
            }
            else
            {
                before_0 = 1.0f;
            }
            texel_0 = texel_0 * vec4<f32>((t_0 * before_0));
        }
        return texel_0 * vec4<f32>(color_0.w);
    }
    if(!paints_0(i32(1)))
    {
        found_0 = !paints_0(i32(2));
    }
    else
    {
        found_0 = false;
    }
    if(found_0)
    {
        found_0 = !paints_0(i32(3));
    }
    else
    {
        found_0 = false;
    }
    if(found_0)
    {
        return color_0;
    }
    if(kind_1 == i32(1))
    {
        var _S44 : vec2<f32> = g_0.xy;
        var d_1 : vec2<f32> = g_0.zw - _S44;
        var len2_0 : f32 = dot(d_1, d_1);
        if((len2_0 > 0.0f))
        {
            t_0 = dot(_S29 - _S44, d_1) / len2_0;
        }
        else
        {
            t_0 = 0.0f;
        }
    }
    else
    {
        if(kind_1 == i32(2))
        {
            var _S45 : f32 = g_0.z;
            if((_S45 > 0.0f))
            {
                t_0 = length(_S29 - g_0.xy) / _S45;
            }
            else
            {
                t_0 = 0.0f;
            }
        }
        else
        {
            var a_0 : f32 = atan2(_S29.y - g_0.y, _S29.x - g_0.x);
            if((a_0 < 0.0f))
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
    var tile_0 : i32 = (bits_0 & (i32(7)));
    var _S46 : bool = tile_0 == i32(3);
    if(_S46)
    {
        if((t_0 < 0.0f))
        {
            found_0 = true;
        }
        else
        {
            found_0 = (t_0 > 1.0f);
        }
    }
    else
    {
        found_0 = false;
    }
    var i_0 : i32;
    if(found_0)
    {
        i_0 = i32(0);
    }
    else
    {
        i_0 = i32(1);
    }
    var shown_0 : f32 = f32(i_0);
    if(tile_0 == i32(0))
    {
        found_0 = true;
    }
    else
    {
        found_0 = _S46;
    }
    if(found_0)
    {
        t_0 = saturate(t_0);
    }
    else
    {
        if(tile_0 == i32(1))
        {
            t_0 = t_0 - floor(t_0);
        }
        else
        {
            var _S47 : f32 = abs(t_0);
            var t_1 : f32 = _S47 - 2.0f * floor(_S47 / 2.0f);
            if((t_1 > 1.0f))
            {
                t_0 = 2.0f - t_1;
            }
            else
            {
                t_0 = t_1;
            }
        }
    }
    var _S48 : i32 = i32(paint_1.z + 0.5f);
    var previous_0 : vec4<f32> = tableTexel_0(0.0f, row_1);
    var _S49 : vec4<f32> = tableTexel_0(1.0f, row_1);
    var before_1 : f32 = _S49.x;
    found_0 = (t_0 <= before_1);
    before_0 = before_1;
    texel_0 = previous_0;
    var straight_0 : vec4<f32> = previous_0;
    i_0 = i32(1);
    loop
    {
        if((i_0 < _S48))
        {
        }
        else
        {
            break;
        }
        var _S50 : f32 = f32(i_0) * 2.0f;
        var next_0 : vec4<f32> = tableTexel_0(_S50, row_1);
        var _S51 : vec4<f32> = tableTexel_0(_S50 + 1.0f, row_1);
        var offset_0 : f32 = _S51.x;
        var _S52 : bool;
        if(!found_0)
        {
            _S52 = (t_0 <= offset_0);
        }
        else
        {
            _S52 = false;
        }
        if(_S52)
        {
            var span_0 : f32 = offset_0 - before_0;
            var _S53 : f32;
            if((span_0 > 0.0f))
            {
                _S53 = (t_0 - before_0) / span_0;
            }
            else
            {
                _S53 = 1.0f;
            }
            var _S54 : vec4<f32> = mix(texel_0, next_0, vec4<f32>(_S53));
            found_0 = true;
            straight_0 = _S54;
        }
        var i_1 : i32 = i_0 + i32(1);
        before_0 = offset_0;
        texel_0 = next_0;
        i_0 = i_1;
    }
    if(found_0)
    {
    }
    else
    {
        straight_0 = texel_0;
    }
    var _S55 : f32 = straight_0.w;
    return vec4<f32>(straight_0.xyz * vec3<f32>(_S55), _S55) * vec4<f32>(color_0.w) * vec4<f32>(shown_0);
}

fn paintAt_0( p_1 : vec2<f32>,  gx_1 : vec2<f32>,  gy_1 : vec2<f32>,  color_1 : vec4<f32>,  paint_2 : vec4<f32>,  g_1 : vec4<f32>) -> vec4<f32>
{
    var _S56 : bool;
    if(!paints_0(i32(5)))
    {
        _S56 = true;
    }
    else
    {
        _S56 = i32(paint_2.x + 0.5f) != i32(5);
    }
    if(_S56)
    {
        var _S57 : vec4<f32> = paintOne_0(p_1, gx_1, gy_1, color_1, paint_2, g_1, false);
        return _S57;
    }
    var texel_1 : array<vec4<f32>, i32(5)>;
    var i_2 : i32 = i32(0);
    loop
    {
        if((i_2 < i32(5)))
        {
        }
        else
        {
            break;
        }
        var _S58 : vec4<f32> = tableTexel_0(f32(i_2), paint_2.w);
        texel_1[i_2] = _S58;
        i_2 = i_2 + i32(1);
    }
    const one_0 : vec4<f32> = vec4<f32>(1.0f, 1.0f, 1.0f, 1.0f);
    var d_2 : vec4<f32> = paintOne_0(p_1, gx_1, gy_1, one_0, texel_1[i32(0)], texel_1[i32(1)], false);
    var s_3 : vec4<f32> = paintOne_0(p_1, gx_1, gy_1, one_0, texel_1[i32(2)], texel_1[i32(3)], true);
    return blendColors_0(s_3, d_2, i32(texel_1[i32(4)].x + 0.5f)) * vec4<f32>(color_1.w);
}

fn edgeCoverage_0( d_3 : f32,  g_2 : vec2<f32>) -> f32
{
    var _S59 : f32 = max(length(g_2), 9.99999997475242708e-07f);
    var n_0 : vec2<f32> = abs(g_2) / vec2<f32>(_S59);
    var _S60 : f32 = n_0.x;
    var _S61 : f32 = n_0.y;
    var _S62 : f32 = max(_S60, _S61);
    var _S63 : f32 = min(_S60, _S61);
    var _S64 : f32 = _S62 + _S63;
    var u_0 : f32 = clamp(0.5f * _S64 - d_3 / _S59, 0.0f, _S64);
    var _S65 : f32 = max(2.0f * _S62 * _S63, 9.999999960041972e-13f);
    var _S66 : f32;
    if((u_0 <= _S63))
    {
        _S66 = u_0 * u_0 / _S65;
    }
    else
    {
        if((u_0 <= _S62))
        {
            _S66 = (u_0 - 0.5f * _S63) / _S62;
        }
        else
        {
            var _S67 : f32 = _S64 - u_0;
            _S66 = 1.0f - _S67 * _S67 / _S65;
        }
    }
    return _S66;
}

fn pixelSteps_0( n_1 : vec2<f32>,  dx_0 : vec2<f32>,  dy_0 : vec2<f32>) -> vec2<f32>
{
    return vec2<f32>(dot(dx_0, n_1), dot(dy_0, n_1));
}

fn roundRectCoverage_0( p_2 : vec2<f32>,  rect_0 : vec4<f32>,  radii_0 : vec4<f32>,  dx_1 : vec2<f32>,  dy_1 : vec2<f32>) -> f32
{
    var _S68 : f32 = dx_1.x;
    var _S69 : f32 = dy_1.x;
    var _S70 : f32 = dx_1.y;
    var _S71 : f32 = dy_1.y;
    var _S72 : vec2<f32> = vec2<f32>(0.5f);
    var _S73 : vec2<f32> = rect_0.xy;
    var _S74 : vec2<f32> = rect_0.zw;
    var _S75 : vec2<f32> = min(p_2 - _S73, _S74 - p_2);
    var _S76 : f32 = radii_0.x;
    var _S77 : f32 = radii_0.y;
    var _S78 : f32 = radii_0.z;
    var _S79 : f32 = radii_0.w;
    var _S80 : f32 = max(max(_S76, _S77), max(_S78, _S79));
    var _S81 : bool;
    if((all(((_S75 >= (_S72 * vec2<f32>(abs(_S68) + abs(_S69), abs(_S70) + abs(_S71))))))))
    {
        if(((_S75.x) >= _S80))
        {
            _S81 = true;
        }
        else
        {
            _S81 = ((_S75.y) >= _S80);
        }
    }
    else
    {
        _S81 = false;
    }
    if(_S81)
    {
        return 1.0f;
    }
    var gx_2 : vec2<f32> = vec2<f32>(_S68, _S69);
    var gy_2 : vec2<f32> = vec2<f32>(_S70, _S71);
    var _S82 : f32 = p_2.x;
    var _S83 : f32 = p_2.y;
    var straight_1 : f32 = saturate(edgeCoverage_0(rect_0.x - _S82, (vec2<f32>(0) - gx_2)) + edgeCoverage_0(_S82 - rect_0.z, gx_2) - 1.0f) * saturate(edgeCoverage_0(rect_0.y - _S83, (vec2<f32>(0) - gy_2)) + edgeCoverage_0(_S83 - rect_0.w, gy_2) - 1.0f);
    var halfSize_0 : vec2<f32> = (_S74 - _S73) * _S72;
    var q_0 : vec2<f32> = p_2 - (_S73 + _S74) * _S72;
    var _S84 : bool = ((q_0.x) < 0.0f);
    var r_1 : f32;
    if(_S84)
    {
        if(((q_0.y) < 0.0f))
        {
            r_1 = _S76;
        }
        else
        {
            r_1 = _S79;
        }
    }
    else
    {
        if(((q_0.y) < 0.0f))
        {
            r_1 = _S77;
        }
        else
        {
            r_1 = _S78;
        }
    }
    var corner_0 : vec2<f32> = abs(q_0) - halfSize_0 + vec2<f32>(r_1);
    var len_0 : f32 = length(corner_0);
    var _S85 : vec2<f32>;
    if((len_0 > 0.0f))
    {
        _S85 = corner_0 / vec2<f32>(len_0);
    }
    else
    {
        _S85 = vec2<f32>(0.70709997415542603f, 0.70709997415542603f);
    }
    var _S86 : i32;
    if(_S84)
    {
        _S86 = i32(-1);
    }
    else
    {
        _S86 = i32(1);
    }
    var _S87 : f32 = f32(_S86);
    if(((q_0.y) < 0.0f))
    {
        _S86 = i32(-1);
    }
    else
    {
        _S86 = i32(1);
    }
    var curved_0 : f32 = edgeCoverage_0(len_0 - r_1, pixelSteps_0(_S85 * vec2<f32>(_S87, f32(_S86)), dx_1, dy_1));
    if((r_1 > 0.0f))
    {
        _S81 = ((corner_0.x) > 0.0f);
    }
    else
    {
        _S81 = false;
    }
    if(_S81)
    {
        _S81 = ((corner_0.y) > 0.0f);
    }
    else
    {
        _S81 = false;
    }
    if(_S81)
    {
        r_1 = min(curved_0, straight_1);
    }
    else
    {
        r_1 = straight_1;
    }
    return r_1;
}

fn arcCap_0( q_1 : vec2<f32>,  radius_0 : f32,  halfSize_1 : f32,  start_0 : f32,  sweep_0 : f32,  roundCaps_0 : bool) -> vec4<f32>
{
    var aperture_0 : f32 = abs(sweep_0) * 0.5f;
    const _S88 : vec4<f32> = vec4<f32>(0.0f, 1.0f, 0.0f, 0.0f);
    var _S89 : f32 = - (start_0 + sweep_0 * 0.5f);
    var c_5 : f32 = cos(_S89);
    var s_4 : f32 = sin(_S89);
    var _S90 : f32 = q_1.x;
    var _S91 : f32 = q_1.y;
    var _S92 : f32 = s_4 * _S90 + c_5 * _S91;
    var p_3 : vec2<f32> = vec2<f32>(c_5 * _S90 - s_4 * _S91, _S92);
    var _S93 : i32;
    if((_S92 < 0.0f))
    {
        _S93 = i32(-1);
    }
    else
    {
        _S93 = i32(1);
    }
    var mirror_0 : f32 = f32(_S93);
    p_3[i32(1)] = abs(p_3.y);
    var _S94 : f32 = cos(aperture_0);
    var _S95 : f32 = sin(aperture_0);
    var end_0 : vec2<f32> = vec2<f32>(_S94, _S95);
    var _S96 : bool;
    if((aperture_0 < 3.1415901184082031f))
    {
        _S96 = ((atan2(p_3.y, p_3.x)) > aperture_0);
    }
    else
    {
        _S96 = false;
    }
    var field_0 : vec4<f32>;
    if(_S96)
    {
        var v_0 : vec2<f32> = p_3 - end_0 * vec2<f32>(radius_0);
        var outward_0 : vec2<f32> = vec2<f32>(- _S95, _S94);
        var n_2 : vec2<f32> = outward_0;
        var d_4 : f32;
        var _S97 : vec2<f32>;
        if(roundCaps_0)
        {
            var l_2 : f32 = length(v_0);
            var _S98 : f32 = l_2 - halfSize_1;
            if((l_2 > 0.0f))
            {
                _S97 = v_0 / vec2<f32>(l_2);
            }
            else
            {
                _S97 = outward_0;
            }
            n_2 = _S97;
            d_4 = _S98;
        }
        else
        {
            var along_0 : f32 = dot(v_0, outward_0);
            var toward_0 : f32 = dot(v_0, end_0);
            var across_0 : f32 = abs(toward_0) - halfSize_1;
            var _S99 : f32 = max(along_0, 0.0f);
            var _S100 : f32 = max(across_0, 0.0f);
            var l_3 : f32 = length(vec2<f32>(_S99, _S100));
            var _S101 : bool = (across_0 <= 0.0f);
            if(_S101)
            {
                d_4 = along_0;
            }
            else
            {
                d_4 = l_3;
            }
            if(_S101)
            {
                _S96 = true;
            }
            else
            {
                _S96 = (l_3 <= 0.0f);
            }
            if(_S96)
            {
                _S97 = outward_0;
            }
            else
            {
                var _S102 : vec2<f32> = outward_0 * vec2<f32>(_S99);
                if((toward_0 < 0.0f))
                {
                    _S93 = i32(-1);
                }
                else
                {
                    _S93 = i32(1);
                }
                _S97 = (_S102 + end_0 * vec2<f32>(f32(_S93)) * vec2<f32>(_S100)) / vec2<f32>(l_3);
            }
            n_2 = _S97;
        }
        n_2[i32(1)] = n_2[i32(1)] * mirror_0;
        field_0 = vec4<f32>(d_4, c_5 * n_2.x + s_4 * n_2.y, - s_4 * n_2.x + c_5 * n_2.y, 1.0f);
    }
    else
    {
        field_0 = _S88;
    }
    return field_0;
}

fn erf2_0( x_1 : vec2<f32>) -> vec2<f32>
{
    var _S103 : vec2<f32> = vec2<f32>((vec2<i32>(sign((x_1)))));
    var a_1 : vec2<f32> = abs(x_1);
    var _S104 : vec2<f32> = vec2<f32>(1.0f) + (vec2<f32>(0.27839300036430359f) + (vec2<f32>(0.23038899898529053f) + vec2<f32>(0.07810799777507782f) * (a_1 * a_1)) * a_1) * a_1;
    var _S105 : vec2<f32> = _S104 * _S104;
    return _S103 - _S103 / (_S105 * _S105);
}

fn gaussian_0( x_2 : f32,  sigma_0 : f32) -> f32
{
    return exp(- (x_2 * x_2) / (2.0f * sigma_0 * sigma_0)) / (sqrt(6.2831854820251465f) * sigma_0);
}

fn blurredRoundRect_0( p_4 : vec2<f32>,  rect_1 : vec4<f32>,  radii_1 : vec4<f32>,  sigma_1 : f32) -> f32
{
    var _S106 : vec2<f32> = rect_1.xy;
    var _S107 : vec2<f32> = rect_1.zw;
    var _S108 : vec2<f32> = vec2<f32>(0.5f);
    var halfSize_2 : vec2<f32> = (_S107 - _S106) * _S108;
    var q_2 : vec2<f32> = p_4 - (_S106 + _S107) * _S108;
    var _S109 : f32 = q_2.x;
    var _S110 : f32;
    if((_S109 < 0.0f))
    {
        if(((q_2.y) < 0.0f))
        {
            _S110 = radii_1.x;
        }
        else
        {
            _S110 = radii_1.w;
        }
    }
    else
    {
        if(((q_2.y) < 0.0f))
        {
            _S110 = radii_1.y;
        }
        else
        {
            _S110 = radii_1.z;
        }
    }
    var _S111 : f32 = q_2.y;
    var _S112 : f32 = halfSize_2.y;
    var low_0 : f32 = _S111 - _S112;
    var high_0 : f32 = _S111 + _S112;
    var start_1 : f32 = clamp(-3.0f * sigma_1, low_0, high_0);
    var step_0 : f32 = (clamp(3.0f * sigma_1, low_0, high_0) - start_1) / 4.0f;
    var _S113 : f32 = start_1 + step_0 * 0.5f;
    var i_3 : i32 = i32(0);
    var y_0 : f32 = _S113;
    var value_0 : f32 = 0.0f;
    loop
    {
        if((i_3 < i32(4)))
        {
        }
        else
        {
            break;
        }
        var _S114 : f32 = min(_S112 - _S110 - abs(_S111 - y_0), 0.0f);
        var curved_1 : f32 = halfSize_2.x - _S110 + sqrt(max(0.0f, _S110 * _S110 - _S114 * _S114));
        var integral_0 : vec2<f32> = _S108 + _S108 * erf2_0((vec2<f32>(_S109) + vec2<f32>(- curved_1, curved_1)) * vec2<f32>((sqrt(0.5f) / sigma_1)));
        var value_1 : f32 = value_0 + (integral_0.y - integral_0.x) * gaussian_0(y_0, sigma_1) * step_0;
        var y_1 : f32 = y_0 + step_0;
        i_3 = i_3 + i32(1);
        y_0 = y_1;
        value_0 = value_1;
    }
    return value_0;
}

fn mitchell_0( x_3 : f32) -> f32
{
    var _S115 : f32 = abs(x_3);
    var _S116 : f32;
    if((_S115 < 1.0f))
    {
        _S116 = (7.0f * _S115 * _S115 * _S115 - 12.0f * _S115 * _S115 + 5.3333334922790527f) / 6.0f;
    }
    else
    {
        if((_S115 < 2.0f))
        {
            _S116 = (-2.3333332538604736f * _S115 * _S115 * _S115 + 12.0f * _S115 * _S115 - 20.0f * _S115 + 10.666666984558105f) / 6.0f;
        }
        else
        {
            _S116 = 0.0f;
        }
    }
    return _S116;
}

fn cubicSample_0( uv_0 : vec2<f32>,  size_1 : vec2<f32>) -> vec4<f32>
{
    var _S117 : vec2<f32> = vec2<f32>(0.5f);
    var t_2 : vec2<f32> = uv_0 * size_1 - _S117;
    var base_0 : vec2<f32> = floor(t_2);
    var _S118 : vec2<f32> = t_2 - base_0;
    var sum_0 : vec4<f32> = vec4<f32>(vec4<i32>(i32(0)));
    var j_0 : i32 = i32(-1);
    loop
    {
        if((j_0 <= i32(2)))
        {
        }
        else
        {
            break;
        }
        var _S119 : f32 = f32(j_0);
        var _S120 : f32 = mitchell_0(_S119 - _S118.y);
        var i_4 : i32 = i32(-1);
        loop
        {
            if((i_4 <= i32(2)))
            {
            }
            else
            {
                break;
            }
            var _S121 : f32 = f32(i_4);
            sum_0 = sum_0 + (textureSampleLevel((texture0_texture_0), (texture0_sampler_0), ((base_0 + vec2<f32>(_S121, _S119) + _S117) / size_1), (0.0f))) * vec4<f32>((mitchell_0(_S121 - _S118.x) * _S120));
            i_4 = i_4 + i32(1);
        }
        j_0 = j_0 + i32(1);
    }
    sum_0[i32(3)] = saturate(sum_0.w);
    return vec4<f32>(clamp(sum_0.xyz, vec3<f32>(vec3<i32>(i32(0))), vec3<f32>(sum_0.w)), sum_0.w);
}

struct Varyings_0
{
     position_0 : vec4<f32>,
     local_0 : vec2<f32>,
     w_0 : f32,
     texcoord_0 : vec2<f32>,
     vertexColor_0 : vec4<f32>,
    @interpolate(flat)  shape_0 : vec4<f32>,
    @interpolate(flat)  radii_2 : vec4<f32>,
    @interpolate(flat)  color_2 : vec4<f32>,
    @interpolate(flat)  params_0 : vec4<f32>,
    @interpolate(flat)  uv_1 : vec4<f32>,
    @interpolate(flat)  kindFlags_0 : vec2<f32>,
    @interpolate(flat)  paint_3 : vec4<f32>,
    @interpolate(flat)  gradient_0 : vec4<f32>,
    @interpolate(flat)  filter_0 : vec4<f32>,
    @interpolate(flat)  steps_0 : vec4<f32>,
     inside_0 : vec4<f32>,
};

fn sampleAt_0( uv_2 : vec2<f32>,  v_1 : Varyings_0,  tx_0 : vec2<f32>,  ty_0 : vec2<f32>) -> vec4<f32>
{
    var _S122 : vec2<f32>;
    var _S123 : vec2<f32>;
    if(((FLAGS_0 & (i32(32)))) == i32(0))
    {
        var across_1 : vec2<f32> = (v_1.uv_1.zw - v_1.uv_1.xy) / (v_1.shape_0.zw - v_1.shape_0.xy);
        var _S124 : vec2<f32> = across_1 * v_1.steps_0.zw;
        _S122 = across_1 * v_1.steps_0.xy;
        _S123 = _S124;
    }
    else
    {
        _S122 = tx_0;
        _S123 = ty_0;
    }
    return (textureSampleGrad((texture0_texture_0), (texture0_sampler_0), (uv_2), (_S122), (_S123)));
}

fn pathCoverage_0( sum_1 : f32,  evenOdd_0 : bool) -> f32
{
    var _S125 : f32 = abs(sum_1);
    if(!evenOdd_0)
    {
        return min(_S125, 1.0f);
    }
    var m_0 : f32 = _S125 - 2.0f * floor(_S125 / 2.0f);
    var _S126 : f32;
    if((m_0 <= 1.0f))
    {
        _S126 = m_0;
    }
    else
    {
        _S126 = 2.0f - m_0;
    }
    return _S126;
}

fn roundRectDistance_0( p_5 : vec2<f32>,  rect_2 : vec4<f32>,  radii_3 : vec4<f32>) -> f32
{
    var _S127 : vec2<f32> = rect_2.xy;
    var _S128 : vec2<f32> = rect_2.zw;
    var _S129 : vec2<f32> = vec2<f32>(0.5f);
    var halfSize_3 : vec2<f32> = (_S128 - _S127) * _S129;
    var q_3 : vec2<f32> = p_5 - (_S127 + _S128) * _S129;
    var r_2 : f32;
    if(((q_3.x) < 0.0f))
    {
        if(((q_3.y) < 0.0f))
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
        if(((q_3.y) < 0.0f))
        {
            r_2 = radii_3.y;
        }
        else
        {
            r_2 = radii_3.z;
        }
    }
    var d_5 : vec2<f32> = abs(q_3) - halfSize_3 + vec2<f32>(r_2);
    return min(max(d_5.x, d_5.y), 0.0f) + length(max(d_5, vec2<f32>(vec2<i32>(i32(0))))) - r_2;
}

fn maskTexcoord_0( texel_2 : vec2<f32>) -> vec2<f32>
{
    return (floor(texel_2) + vec2<f32>(0.5f)) * uniforms_0.texels_0.xy;
}

fn clipCoverage_0( pixel_1 : vec2<f32>,  count_0 : i32,  row_2 : f32) -> f32
{
    var i_5 : i32 = i32(0);
    var inside_1 : f32 = 1.0f;
    loop
    {
        if((i_5 < count_0))
        {
        }
        else
        {
            break;
        }
        var x_4 : f32 = f32(i_5) * 6.0f;
        var head_0 : vec4<f32> = tableTexel_0(x_4, row_2);
        var a_2 : vec4<f32> = tableTexel_0(x_4 + 1.0f, row_2);
        var b_2 : vec4<f32> = tableTexel_0(x_4 + 2.0f, row_2);
        var c_6 : f32;
        if(((head_0.x) > 0.5f))
        {
            var within_0 : bool;
            if((all(((pixel_1 >= (b_2.xy))))))
            {
                within_0 = (all(((pixel_1 < (b_2.zw)))));
            }
            else
            {
                within_0 = false;
            }
            var sum_2 : f32 = (textureSampleLevel((texture4_texture_0), (texture4_sampler_0), (maskTexcoord_0(pixel_1 - a_2.xy)), (0.0f))).x;
            if(within_0)
            {
                c_6 = pathCoverage_0(sum_2, ((head_0.z) > 0.5f));
            }
            else
            {
                c_6 = 0.0f;
            }
        }
        else
        {
            var _S130 : vec4<f32> = tableTexel_0(x_4 + 3.0f, row_2);
            var r0_1 : vec3<f32> = _S130.xyz;
            var _S131 : vec4<f32> = tableTexel_0(x_4 + 4.0f, row_2);
            var r1_1 : vec3<f32> = _S131.xyz;
            var _S132 : vec4<f32> = tableTexel_0(x_4 + 5.0f, row_2);
            var r2_1 : vec3<f32> = _S132.xyz;
            var h_1 : vec3<f32> = vec3<f32>(pixel_1, 1.0f);
            var _S133 : f32 = dot(r2_1, h_1);
            var _S134 : vec2<f32> = vec2<f32>(_S133);
            var p_6 : vec2<f32> = vec2<f32>(dot(r0_1, h_1), dot(r1_1, h_1)) / _S134;
            var _S135 : f32 = r2_1.x;
            var _S136 : f32 = p_6.x;
            var _S137 : f32 = p_6.y;
            var dx_2 : vec2<f32> = vec2<f32>(r0_1.x - _S135 * _S136, r1_1.x - _S135 * _S137) / _S134;
            var _S138 : f32 = r2_1.y;
            var dy_2 : vec2<f32> = vec2<f32>(r0_1.y - _S138 * _S136, r1_1.y - _S138 * _S137) / _S134;
            if((_S133 > 0.0f))
            {
                c_6 = roundRectCoverage_0(p_6, a_2, b_2, dx_2, dy_2);
            }
            else
            {
                c_6 = 0.0f;
            }
        }
        var _S139 : f32;
        if(((head_0.y) > 0.5f))
        {
            _S139 = 1.0f - c_6;
        }
        else
        {
            _S139 = c_6;
        }
        var inside_2 : f32 = inside_1 * _S139;
        i_5 = i_5 + i32(1);
        inside_1 = inside_2;
    }
    return inside_1;
}

fn applyFilters_0( color_3 : vec4<f32>,  filter_1 : vec4<f32>) -> vec4<f32>
{
    var count_1 : i32 = i32(filter_1.x + 0.5f);
    var _S140 : f32 = color_3.w;
    var c_7 : vec4<f32>;
    if((_S140 > 0.0f))
    {
        c_7 = vec4<f32>(color_3.xyz / vec3<f32>(_S140), _S140);
    }
    else
    {
        c_7 = vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
    }
    var i_6 : i32 = i32(0);
    loop
    {
        if((i_6 < count_1))
        {
        }
        else
        {
            break;
        }
        var x_5 : f32 = f32(i_6) * 6.0f;
        var _S141 : f32 = filter_1.y;
        var head_1 : vec4<f32> = tableTexel_0(x_5, _S141);
        var kind_2 : i32 = i32(head_1.x + 0.5f);
        var a_3 : vec4<f32> = tableTexel_0(x_5 + 1.0f, _S141);
        var b_3 : vec4<f32> = tableTexel_0(x_5 + 2.0f, _S141);
        if(kind_2 == i32(1))
        {
            var r2_2 : vec4<f32> = tableTexel_0(x_5 + 3.0f, _S141);
            var r3_0 : vec4<f32> = tableTexel_0(x_5 + 4.0f, _S141);
            var added_0 : vec4<f32> = tableTexel_0(x_5 + 5.0f, _S141);
            c_7 = saturate(vec4<f32>(dot(a_3, c_7), dot(b_3, c_7), dot(r2_2, c_7), dot(r3_0, c_7)) + added_0);
        }
        else
        {
            var c_8 : vec4<f32>;
            if(kind_2 == i32(2))
            {
                c_8 = vec4<f32>(saturate(c_7.xyz * a_3.xyz + b_3.xyz), c_7.w);
            }
            else
            {
                var _S142 : f32 = a_3.w;
                var _S143 : f32 = c_7.w;
                var tinted_0 : vec4<f32> = blendColors_0(vec4<f32>(a_3.xyz * vec3<f32>(_S142), _S142), vec4<f32>(c_7.xyz * vec3<f32>(_S143), _S143), i32(head_1.y + 0.5f));
                var _S144 : f32 = tinted_0.w;
                if((_S144 > 0.0f))
                {
                    c_8 = vec4<f32>(saturate(tinted_0.xyz / vec3<f32>(_S144)), min(_S144, 1.0f));
                }
                else
                {
                    c_8 = vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
                }
            }
            c_7 = c_8;
        }
        i_6 = i_6 + i32(1);
    }
    if((count_1 > i32(0)))
    {
        var _S145 : f32 = c_7.w;
        c_7 = vec4<f32>(c_7.xyz * vec3<f32>(_S145), _S145);
    }
    else
    {
        c_7 = color_3;
    }
    return c_7;
}

fn shade_0( v_2 : Varyings_0,  dx_3 : vec2<f32>,  dy_3 : vec2<f32>,  tx_1 : vec2<f32>,  ty_1 : vec2<f32>) -> vec4<f32>
{
    var kind_3 : i32 = i32(v_2.kindFlags_0.x + 0.5f);
    var flags_0 : i32 = (i32(v_2.kindFlags_0.y + 0.5f) & (FLAGS_0));
    var _S146 : vec2<f32>;
    var _S147 : vec2<f32>;
    if(((FLAGS_0 & (i32(32)))) == i32(0))
    {
        var _S148 : vec2<f32> = v_2.steps_0.zw;
        _S146 = v_2.steps_0.xy;
        _S147 = _S148;
    }
    else
    {
        _S146 = dx_3;
        _S147 = dy_3;
    }
    var _S149 : bool = kind_3 == i32(10);
    var painted_0 : vec2<f32>;
    if(_S149)
    {
        painted_0 = v_2.texcoord_0;
    }
    else
    {
        painted_0 = v_2.local_0;
    }
    var gx_3 : vec2<f32>;
    if(_S149)
    {
        gx_3 = tx_1;
    }
    else
    {
        gx_3 = _S146;
    }
    var gy_3 : vec2<f32>;
    if(_S149)
    {
        gy_3 = ty_1;
    }
    else
    {
        gy_3 = _S147;
    }
    if(((flags_0 & (i32(4)))) != i32(0))
    {
        var h_2 : vec3<f32> = vec3<f32>(v_2.local_0, 1.0f);
        var _S150 : vec2<f32> = vec2<f32>(dot(v_2.shape_0.xyz, h_2));
        var painted_1 : vec2<f32> = vec2<f32>(dot(v_2.radii_2.xyz, h_2), dot(v_2.uv_1.xyz, h_2)) / _S150;
        var _S151 : vec2<f32> = v_2.shape_0.xy;
        var jx_1 : vec2<f32> = (v_2.radii_2.xy - vec2<f32>(painted_1.x) * _S151) / _S150;
        var jy_1 : vec2<f32> = (v_2.uv_1.xy - vec2<f32>(painted_1.y) * _S151) / _S150;
        var gx_4 : vec2<f32> = vec2<f32>(dot(jx_1, gx_3), dot(jy_1, gx_3));
        var gy_4 : vec2<f32> = vec2<f32>(dot(jx_1, gy_3), dot(jy_1, gy_3));
        painted_0 = painted_1;
        gx_3 = gx_4;
        gy_3 = gy_4;
    }
    var aliased_0 : bool = ((flags_0 & (i32(16)))) != i32(0);
    if(aliased_0)
    {
        var _S152 : vec2<f32> = vec2<f32>(9.9999997473787516e-05f);
        var _S153 : vec2<f32> = _S147 * _S152;
        _S146 = _S146 * _S152;
        _S147 = _S153;
    }
    var paint_4 : vec4<f32> = paintAt_0(painted_0, gx_3, gy_3, v_2.color_2, v_2.paint_3, v_2.gradient_0);
    var edged_0 : bool = ((flags_0 & (i32(2)))) != i32(0);
    var _S154 : vec2<f32> = v_2.uv_1.xy;
    var _S155 : vec2<f32> = v_2.uv_1.zw;
    var _S156 : vec2<f32> = v_2.params_0.zw;
    var lowUv_0 : vec2<f32> = min(_S154, _S155) + _S156;
    var highUv_0 : vec2<f32> = max(_S154, _S155) - _S156;
    var texcoord_1 : vec2<f32>;
    if(edged_0)
    {
        texcoord_1 = clamp(v_2.texcoord_0, lowUv_0, highUv_0);
    }
    else
    {
        texcoord_1 = v_2.texcoord_0;
    }
    var edge_0 : f32;
    if(edged_0)
    {
        edge_0 = roundRectCoverage_0(v_2.local_0, v_2.shape_0, vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f), _S146, _S147);
    }
    else
    {
        edge_0 = 1.0f;
    }
    var _S157 : bool;
    if(drawn_0(i32(0)))
    {
        _S157 = kind_3 == i32(0);
    }
    else
    {
        _S157 = false;
    }
    var cover_1 : f32;
    var src_1 : vec4<f32>;
    if(_S157)
    {
        var _S158 : f32 = roundRectCoverage_0(v_2.local_0, v_2.shape_0, v_2.radii_2, _S146, _S147);
        src_1 = paint_4;
        cover_1 = _S158;
    }
    else
    {
        if(drawn_0(i32(1)))
        {
            _S157 = kind_3 == i32(1);
        }
        else
        {
            _S157 = false;
        }
        if(_S157)
        {
            var w_1 : f32 = v_2.params_0.x;
            var _S159 : vec2<f32> = vec2<f32>(w_1);
            var _S160 : f32 = saturate(roundRectCoverage_0(v_2.local_0, v_2.shape_0, v_2.radii_2, _S146, _S147) - roundRectCoverage_0(v_2.local_0, vec4<f32>(v_2.shape_0.xy + _S159, v_2.shape_0.zw - _S159), max(v_2.radii_2 - vec4<f32>(w_1), vec4<f32>(vec4<i32>(i32(0)))), _S146, _S147));
            src_1 = paint_4;
            cover_1 = _S160;
        }
        else
        {
            if(drawn_0(i32(2)))
            {
                _S157 = kind_3 == i32(2);
            }
            else
            {
                _S157 = false;
            }
            if(_S157)
            {
                var a_4 : vec2<f32> = v_2.shape_0.xy;
                var _S161 : vec2<f32> = v_2.shape_0.zw - a_4;
                var len_1 : f32 = length(_S161);
                var dir_0 : vec2<f32>;
                if((len_1 > 0.0f))
                {
                    dir_0 = _S161 / vec2<f32>(len_1);
                }
                else
                {
                    dir_0 = vec2<f32>(1.0f, 0.0f);
                }
                var side_0 : vec2<f32> = vec2<f32>(- dir_0.y, dir_0.x);
                var d_6 : vec2<f32> = v_2.local_0 - a_4;
                var _S162 : f32 = len_1 * 0.5f;
                var along_1 : f32 = dot(d_6, dir_0) - _S162;
                var across_2 : f32 = dot(d_6, side_0);
                var h_3 : f32 = v_2.params_0.x;
                var reach_0 : f32 = _S162 + h_3;
                var gSide_0 : vec2<f32> = pixelSteps_0(side_0, _S146, _S147);
                var gDir_0 : vec2<f32> = pixelSteps_0(dir_0, _S146, _S147);
                var _S163 : f32 = saturate(edgeCoverage_0(across_2 - h_3, gSide_0) + edgeCoverage_0(- across_2 - h_3, (vec2<f32>(0) - gSide_0)) - 1.0f) * saturate(edgeCoverage_0(along_1 - reach_0, gDir_0) + edgeCoverage_0(- along_1 - reach_0, (vec2<f32>(0) - gDir_0)) - 1.0f);
                src_1 = paint_4;
                cover_1 = _S163;
            }
            else
            {
                if(drawn_0(i32(3)))
                {
                    _S157 = kind_3 == i32(3);
                }
                else
                {
                    _S157 = false;
                }
                if(_S157)
                {
                    var q_4 : vec2<f32> = v_2.local_0 - v_2.shape_0.xy;
                    var len_2 : f32 = length(q_4);
                    var radius_1 : f32 = v_2.shape_0.z;
                    var h_4 : f32 = v_2.params_0.x;
                    if((len_2 > 0.0f))
                    {
                        painted_0 = q_4 / vec2<f32>(len_2);
                    }
                    else
                    {
                        painted_0 = vec2<f32>(1.0f, 0.0f);
                    }
                    var g_3 : vec2<f32> = pixelSteps_0(painted_0, _S146, _S147);
                    var ring_0 : f32 = saturate(edgeCoverage_0(len_2 - radius_1 - h_4, g_3) + edgeCoverage_0(radius_1 - h_4 - len_2, (vec2<f32>(0) - g_3)) - 1.0f);
                    var cap_0 : vec4<f32> = arcCap_0(q_4, radius_1, h_4, v_2.params_0.y, v_2.params_0.z, ((v_2.params_0.w) > 0.5f));
                    if(((cap_0.w) > 0.5f))
                    {
                        edge_0 = edgeCoverage_0(cap_0.x, pixelSteps_0(cap_0.yz, _S146, _S147));
                    }
                    else
                    {
                        edge_0 = ring_0;
                    }
                    src_1 = paint_4;
                    cover_1 = edge_0;
                }
                else
                {
                    if(drawn_0(i32(4)))
                    {
                        _S157 = kind_3 == i32(4);
                    }
                    else
                    {
                        _S157 = false;
                    }
                    if(_S157)
                    {
                        var sigma_2 : f32 = v_2.params_0.x;
                        if((sigma_2 > 0.0099999997764825821f))
                        {
                            cover_1 = blurredRoundRect_0(v_2.local_0, v_2.shape_0, v_2.radii_2, sigma_2);
                        }
                        else
                        {
                            cover_1 = roundRectCoverage_0(v_2.local_0, v_2.shape_0, v_2.radii_2, _S146, _S147);
                        }
                        if(((v_2.params_0.y) > 0.5f))
                        {
                            edge_0 = (1.0f - cover_1) * roundRectCoverage_0(v_2.local_0, v_2.gradient_0, v_2.uv_1, _S146, _S147);
                        }
                        else
                        {
                            edge_0 = cover_1;
                        }
                        src_1 = paint_4;
                        cover_1 = edge_0;
                    }
                    else
                    {
                        if(drawn_0(i32(5)))
                        {
                            _S157 = kind_3 == i32(5);
                        }
                        else
                        {
                            _S157 = false;
                        }
                        if(_S157)
                        {
                            if(((v_2.params_0.y) > 0.5f))
                            {
                                src_1 = cubicSample_0(texcoord_1, vec2<f32>(0.5f) / _S156);
                            }
                            else
                            {
                                src_1 = sampleAt_0(texcoord_1, v_2, tx_1, ty_1);
                            }
                            var colored_0 : bool = ((v_2.params_0.x) > 0.5f);
                            var _S164 : vec4<f32>;
                            if(colored_0)
                            {
                                var _S165 : f32 = src_1.w;
                                _S164 = vec4<f32>(src_1.xyz * vec3<f32>(_S165), _S165) * vec4<f32>(v_2.color_2.w);
                            }
                            else
                            {
                                _S164 = paint_4;
                            }
                            if(colored_0)
                            {
                                cover_1 = 1.0f;
                            }
                            else
                            {
                                cover_1 = src_1.w;
                            }
                            var _S166 : f32 = cover_1 * edge_0;
                            src_1 = _S164;
                            cover_1 = _S166;
                        }
                        else
                        {
                            if(drawn_0(i32(6)))
                            {
                                _S157 = kind_3 == i32(6);
                            }
                            else
                            {
                                _S157 = false;
                            }
                            if(_S157)
                            {
                                if(((v_2.params_0.y) > 0.5f))
                                {
                                    src_1 = cubicSample_0(texcoord_1, vec2<f32>(0.5f) / _S156);
                                }
                                else
                                {
                                    src_1 = sampleAt_0(texcoord_1, v_2, tx_1, ty_1);
                                }
                                src_1 = src_1 * vec4<f32>(v_2.params_0.x);
                                cover_1 = edge_0;
                            }
                            else
                            {
                                if(drawn_0(i32(7)))
                                {
                                    _S157 = kind_3 == i32(7);
                                }
                                else
                                {
                                    _S157 = false;
                                }
                                if(_S157)
                                {
                                    var _S167 : vec2<f32> = v_2.params_0.xy;
                                    var _S168 : vec2<f32> = vec2<f32>(0.5f);
                                    var mixed_0 : bool = ((flags_0 & (i32(8)))) != i32(0);
                                    var _S169 : vec4<f32> = (textureSampleLevel((texture0_texture_0), (texture0_sampler_0), ((clamp(v_2.local_0, _S167 + _S168, _S156 - _S168) - _S167) * _S155), (0.0f)));
                                    if(mixed_0)
                                    {
                                        edge_0 = 1.0f;
                                    }
                                    else
                                    {
                                        edge_0 = v_2.color_2.w;
                                    }
                                    var _S170 : vec4<f32> = _S169 * vec4<f32>(edge_0);
                                    if(((flags_0 & (i32(1)))) != i32(0))
                                    {
                                        cover_1 = pathCoverage_0((textureSampleLevel((texture1_texture_0), (texture1_sampler_0), ((v_2.local_0 - v_2.radii_2.xy) * uniforms_0.texels_0.xy), (0.0f))).x, false);
                                    }
                                    else
                                    {
                                        cover_1 = roundRectCoverage_0(v_2.local_0, v_2.shape_0, v_2.radii_2, _S146, _S147);
                                    }
                                    if(mixed_0)
                                    {
                                        cover_1 = cover_1 * v_2.color_2.w;
                                    }
                                    src_1 = _S170;
                                }
                                else
                                {
                                    if(drawn_0(i32(9)))
                                    {
                                        _S157 = kind_3 == i32(9);
                                    }
                                    else
                                    {
                                        _S157 = false;
                                    }
                                    if(_S157)
                                    {
                                        var d_7 : f32 = - roundRectDistance_0(v_2.local_0, v_2.shape_0, v_2.radii_2);
                                        var t_3 : f32 = saturate(d_7 / v_2.params_0.x);
                                        if((d_7 > 0.0f))
                                        {
                                            var _S171 : f32 = v_2.params_0.y;
                                            if((_S171 <= 0.0f))
                                            {
                                                _S157 = true;
                                            }
                                            else
                                            {
                                                _S157 = (d_7 < (_S171 - 0.0009765625f));
                                            }
                                        }
                                        else
                                        {
                                            _S157 = false;
                                        }
                                        if(_S157)
                                        {
                                            var _S172 : f32 = 1.0f - t_3;
                                            edge_0 = max(exp(-4.0f * _S172 * _S172) - 0.017999999225139618f, 0.0f);
                                        }
                                        else
                                        {
                                            edge_0 = 0.0f;
                                        }
                                        src_1 = paint_4;
                                        cover_1 = edge_0;
                                    }
                                    else
                                    {
                                        if(drawn_0(i32(8)))
                                        {
                                            _S157 = kind_3 == i32(8);
                                        }
                                        else
                                        {
                                            _S157 = false;
                                        }
                                        if(_S157)
                                        {
                                            var _S173 : vec4<f32> = (textureLoad((texture0_texture_0), (vec2<i32>(floor(v_2.local_0 - v_2.params_0.xy))), 0));
                                            var _S174 : f32 = pathCoverage_0(_S173.x, ((v_2.params_0.z) > 0.5f));
                                            src_1 = paint_4;
                                            cover_1 = _S174;
                                        }
                                        else
                                        {
                                            if(drawn_0(i32(10)))
                                            {
                                                _S157 = _S149;
                                            }
                                            else
                                            {
                                                _S157 = false;
                                            }
                                            if(_S157)
                                            {
                                                if(((v_2.params_0.z) > 0.5f))
                                                {
                                                    src_1 = blendColors_0(paint_4, v_2.vertexColor_0, i32(v_2.params_0.x + 0.5f));
                                                }
                                                else
                                                {
                                                    src_1 = paint_4;
                                                }
                                                src_1 = src_1 * vec4<f32>(v_2.params_0.y);
                                                cover_1 = 1.0f;
                                            }
                                            else
                                            {
                                                src_1 = paint_4;
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
        var _S175 : i32;
        if((cover_1 > 0.5f))
        {
            _S175 = i32(1);
        }
        else
        {
            _S175 = i32(0);
        }
        cover_1 = f32(_S175);
    }
    if(CLIPPED_0 != i32(0))
    {
        var _S176 : f32 = clipCoverage_0(v_2.position_0.xy, i32(v_2.filter_0.z + 0.5f), v_2.filter_0.w);
        cover_1 = cover_1 * _S176;
    }
    if(FILTERED_0 != i32(0))
    {
        var _S177 : vec4<f32> = applyFilters_0(src_1, v_2.filter_0);
        src_1 = _S177;
    }
    return finish_0(src_1, cover_1, v_2.position_0.xy);
}

struct pixelOutput_0
{
    @location(0) output_0 : vec4<f32>,
};

struct pixelInput_0
{
    @location(0) local_1 : vec2<f32>,
    @location(1) w_2 : f32,
    @location(2) texcoord_2 : vec2<f32>,
    @location(3) vertexColor_1 : vec4<f32>,
    @interpolate(flat) @location(4) shape_1 : vec4<f32>,
    @interpolate(flat) @location(5) radii_4 : vec4<f32>,
    @interpolate(flat) @location(6) color_4 : vec4<f32>,
    @interpolate(flat) @location(7) params_1 : vec4<f32>,
    @interpolate(flat) @location(8) uv_3 : vec4<f32>,
    @interpolate(flat) @location(9) kindFlags_1 : vec2<f32>,
    @interpolate(flat) @location(10) paint_5 : vec4<f32>,
    @interpolate(flat) @location(11) gradient_1 : vec4<f32>,
    @interpolate(flat) @location(12) filter_2 : vec4<f32>,
    @interpolate(flat) @location(13) steps_1 : vec4<f32>,
    @location(14) inside_3 : vec4<f32>,
};

@fragment
fn fs( _S178 : pixelInput_0, @builtin(position) position_1 : vec4<f32>) -> pixelOutput_0
{
    var _S179 : Varyings_0 = Varyings_0( position_1, _S178.local_1, _S178.w_2, _S178.texcoord_2, _S178.vertexColor_1, _S178.shape_1, _S178.radii_4, _S178.color_4, _S178.params_1, _S178.uv_3, _S178.kindFlags_1, _S178.paint_5, _S178.gradient_1, _S178.filter_2, _S178.steps_1, _S178.inside_3 );
    var _S180 : vec2<f32> = vec2<f32>(vec2<i32>(i32(0)));
    var dx_4 : vec2<f32>;
    var dy_4 : vec2<f32>;
    var tx_2 : vec2<f32>;
    var ty_2 : vec2<f32>;
    if(((FLAGS_0 & (i32(32)))) != i32(0))
    {
        var q_5 : f32 = 1.0f / _S178.w_2;
        var _S181 : vec2<f32> = vec2<f32>(q_5);
        var lq_0 : vec2<f32> = _S178.local_1 * _S181;
        var dy_5 : vec2<f32> = (dpdy(lq_0) - _S178.local_1 * vec2<f32>(dpdy(q_5))) / _S181;
        var tx_3 : vec2<f32> = dpdx(_S178.texcoord_2);
        var ty_3 : vec2<f32> = dpdy(_S178.texcoord_2);
        dx_4 = (dpdx(lq_0) - _S178.local_1 * vec2<f32>(dpdx(q_5))) / _S181;
        dy_4 = dy_5;
        tx_2 = tx_3;
        ty_2 = ty_3;
    }
    else
    {
        dx_4 = _S180;
        dy_4 = _S180;
        tx_2 = _S180;
        ty_2 = _S180;
    }
    var _S182 : bool = drawn_0(i32(0));
    var quick_0 : bool;
    if(_S182)
    {
        quick_0 = true;
    }
    else
    {
        quick_0 = drawn_0(i32(7));
    }
    if(quick_0)
    {
        var a_5 : vec4<f32> = abs(_S178.inside_3);
        if(((a_5.x) <= 1.0f))
        {
            quick_0 = ((a_5.y) <= 1.0f);
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
            if(((a_5.z) <= 1.0f))
            {
                quick_0 = ((a_5.w) <= 1.0f);
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
    var result_0 : vec4<f32>;
    if(quick_0)
    {
        var layer_0 : bool;
        if(drawn_0(i32(7)))
        {
            if(!_S182)
            {
                quick_0 = true;
            }
            else
            {
                quick_0 = i32(_S178.kindFlags_1.x + 0.5f) == i32(7);
            }
            layer_0 = quick_0;
        }
        else
        {
            layer_0 = false;
        }
        if(layer_0)
        {
            result_0 = (textureSampleLevel((texture0_texture_0), (texture0_sampler_0), (_S178.texcoord_2), (0.0f))) * vec4<f32>(_S178.color_4.w);
        }
        else
        {
            result_0 = _S178.color_4;
        }
        result_0 = finish_0(result_0, 1.0f, position_1.xy);
    }
    else
    {
        var _S183 : vec4<f32> = shade_0(_S179, dx_4, dy_4, tx_2, ty_2);
        result_0 = _S183;
    }
    var _S184 : pixelOutput_0 = pixelOutput_0( result_0 );
    return _S184;
}

