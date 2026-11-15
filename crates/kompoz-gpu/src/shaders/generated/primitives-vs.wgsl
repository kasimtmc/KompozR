diagnostic(off, derivative_uniformity);
struct Uniforms_std430_0
{
    @align(16) toClip_0 : vec4<f32>,
    @align(16) texels_0 : vec4<f32>,
    @align(16) blend_0 : vec4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms_0 : Uniforms_std430_0;
var<private> corners_0 : array<vec2<f32>, i32(6)> = array<vec2<f32>, i32(6)>( vec2<f32>(0.0f, 0.0f), vec2<f32>(1.0f, 0.0f), vec2<f32>(1.0f, 1.0f), vec2<f32>(0.0f, 0.0f), vec2<f32>(1.0f, 1.0f), vec2<f32>(0.0f, 1.0f) );
struct Varyings_0
{
    @builtin(position) position_0 : vec4<f32>,
    @location(0) local_0 : vec2<f32>,
    @location(1) w_0 : f32,
    @location(2) texcoord_0 : vec2<f32>,
    @location(3) vertexColor_0 : vec4<f32>,
    @interpolate(flat) @location(4) shape_0 : vec4<f32>,
    @interpolate(flat) @location(5) radii_0 : vec4<f32>,
    @interpolate(flat) @location(6) color_0 : vec4<f32>,
    @interpolate(flat) @location(7) params_0 : vec4<f32>,
    @interpolate(flat) @location(8) uv_0 : vec4<f32>,
    @interpolate(flat) @location(9) kindFlags_0 : vec2<f32>,
    @interpolate(flat) @location(10) paint_0 : vec4<f32>,
    @interpolate(flat) @location(11) gradient_0 : vec4<f32>,
    @interpolate(flat) @location(12) filter_0 : vec4<f32>,
    @interpolate(flat) @location(13) steps_0 : vec4<f32>,
    @location(14) inside_0 : vec4<f32>,
};

struct vertexInput_0
{
    @location(0) bounds_0 : vec4<f32>,
    @location(1) shape_1 : vec4<f32>,
    @location(2) radii_1 : vec4<f32>,
    @location(3) color_1 : vec4<f32>,
    @location(4) params_1 : vec4<f32>,
    @location(5) uv_1 : vec4<f32>,
    @location(6) xform_0 : vec4<f32>,
    @location(7) extra_0 : vec4<f32>,
    @location(8) paint_1 : vec4<f32>,
    @location(9) gradient_1 : vec4<f32>,
    @location(10) filter_1 : vec4<f32>,
    @location(11) projection_0 : vec4<f32>,
};

@vertex
fn vs( _S1 : vertexInput_0, @builtin(vertex_index) vertex_0 : u32) -> Varyings_0
{
    var _S2 : u32 = vertex_0 % u32(6);
    var local_1 : vec2<f32> = mix(_S1.bounds_0.xy, _S1.bounds_0.zw, corners_0[_S2]);
    var _S3 : f32 = _S1.xform_0.x;
    var _S4 : f32 = _S1.xform_0.z;
    var _S5 : f32 = _S1.xform_0.y;
    var _S6 : f32 = _S1.xform_0.w;
    var _S7 : f32 = _S1.projection_0.x;
    var _S8 : f32 = _S1.projection_0.y;
    var m_0 : mat3x3<f32> = mat3x3<f32>(_S3, _S4, _S1.extra_0.x, _S5, _S6, _S1.extra_0.y, _S7, _S8, _S1.projection_0.z);
    var placed_0 : vec3<f32> = (((vec3<f32>(local_1, 1.0f)) * (m_0)));
    var _S9 : f32 = placed_0.z;
    var placed_1 : vec3<f32>;
    var local_2 : vec2<f32>;
    if((_S9 > 0.0f))
    {
        var _S10 : vec2<f32> = vec2<f32>(16.0f);
        var snapped_0 : vec2<f32> = round(placed_0.xy / vec2<f32>(_S9) * _S10) / _S10;
        var back_0 : vec3<f32> = cross(m_0[i32(1)], m_0[i32(2)]) * vec3<f32>(snapped_0.x) + cross(m_0[i32(2)], m_0[i32(0)]) * vec3<f32>(snapped_0.y) + cross(m_0[i32(0)], m_0[i32(1)]);
        var _S11 : f32 = back_0.z;
        if(((abs(_S11)) > 9.99999968265522539e-21f))
        {
            var local_3 : vec2<f32> = back_0.xy / vec2<f32>(_S11);
            placed_1 = (((vec3<f32>(local_3, 1.0f)) * (m_0)));
            local_2 = local_3;
        }
        else
        {
            placed_1 = placed_0;
            local_2 = local_1;
        }
    }
    else
    {
        placed_1 = placed_0;
        local_2 = local_1;
    }
    var out_0 : Varyings_0;
    var _S12 : f32 = placed_1.z;
    out_0.position_0 = vec4<f32>(placed_1.xy * uniforms_0.toClip_0.xy + vec2<f32>(_S12) * uniforms_0.toClip_0.zw, 0.0f, _S12);
    out_0.local_0 = local_2;
    out_0.w_0 = _S12;
    var kind_0 : i32 = i32(_S1.extra_0.z + 0.5f);
    var flat_0 : bool;
    if(kind_0 == i32(5))
    {
        flat_0 = true;
    }
    else
    {
        flat_0 = kind_0 == i32(6);
    }
    var across_0 : vec2<f32>;
    if(flat_0)
    {
        var _S13 : vec2<f32> = _S1.shape_1.xy;
        across_0 = (local_2 - _S13) / (_S1.shape_1.zw - _S13);
    }
    else
    {
        across_0 = corners_0[_S2];
    }
    var _S14 : vec2<f32> = _S1.uv_1.zw;
    out_0.texcoord_0 = mix(_S1.uv_1.xy, _S14, across_0);
    const _S15 : vec4<f32> = vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
    out_0.vertexColor_0 = _S15;
    out_0.shape_0 = _S1.shape_1;
    out_0.radii_0 = _S1.radii_1;
    out_0.color_0 = _S1.color_1;
    out_0.params_0 = _S1.params_1;
    out_0.uv_0 = _S1.uv_1;
    out_0.kindFlags_0 = _S1.extra_0.zw;
    out_0.paint_0 = _S1.paint_1;
    out_0.gradient_0 = _S1.gradient_1;
    out_0.filter_0 = _S1.filter_1;
    var det_0 : f32 = _S3 * _S6 - _S4 * _S5;
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
        flat_0 = ((abs(det_0)) > 9.99999968265522539e-21f);
    }
    else
    {
        flat_0 = false;
    }
    var a_0 : vec4<f32>;
    if(flat_0)
    {
        a_0 = vec4<f32>(_S6, - _S5, - _S4, _S3) / vec4<f32>(det_0);
    }
    else
    {
        a_0 = _S15;
    }
    out_0.steps_0 = a_0;
    var flags_0 : i32 = i32(_S1.extra_0.w + 0.5f);
    if(flat_0)
    {
        flat_0 = (_S1.filter_1.x) == 0.0f;
    }
    else
    {
        flat_0 = false;
    }
    var plain_0 : bool;
    if(flat_0)
    {
        plain_0 = (_S1.filter_1.z) == 0.0f;
    }
    else
    {
        plain_0 = false;
    }
    var solid_0 : bool;
    if(kind_0 == i32(0))
    {
        solid_0 = i32(_S1.paint_1.x + 0.5f) == i32(0);
    }
    else
    {
        solid_0 = false;
    }
    var texels_1 : bool;
    if(kind_0 == i32(7))
    {
        texels_1 = ((flags_0 & (i32(9)))) == i32(0);
    }
    else
    {
        texels_1 = false;
    }
    out_0.inside_0 = vec4<f32>(2.0f, 2.0f, 2.0f, 2.0f);
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
        var _S16 : vec2<f32> = vec2<f32>(0.5f);
        var reach_0 : vec2<f32> = _S16 * vec2<f32>(abs(out_0.steps_0.x) + abs(out_0.steps_0.z), abs(out_0.steps_0.y) + abs(out_0.steps_0.w));
        var _S17 : f32 = max(max(_S1.radii_1.x, _S1.radii_1.y), max(_S1.radii_1.z, _S1.radii_1.w));
        var _S18 : f32 = reach_0.x;
        var _S19 : f32 = reach_0.y;
        var inA_0 : vec2<f32> = vec2<f32>(max(_S18, _S17), _S19);
        var inB_0 : vec2<f32> = vec2<f32>(_S18, max(_S19, _S17));
        var _S20 : vec2<f32> = _S1.shape_1.xy;
        var _S21 : vec2<f32> = _S1.shape_1.zw;
        var a_1 : vec4<f32> = vec4<f32>(_S20 + inA_0, _S21 - inA_0);
        var b_0 : vec4<f32> = vec4<f32>(_S20 + inB_0, _S21 - inB_0);
        var b_1 : vec4<f32>;
        if(texels_1)
        {
            var _S22 : vec2<f32> = _S1.params_1.xy;
            var within_0 : vec4<f32> = vec4<f32>(_S22 + _S16, _S1.params_1.zw - _S16);
            var _S23 : vec2<f32> = within_0.xy;
            var _S24 : vec2<f32> = within_0.zw;
            var a_2 : vec4<f32> = vec4<f32>(max(a_1.xy, _S23), min(a_1.zw, _S24));
            var b_2 : vec4<f32> = vec4<f32>(max(b_0.xy, _S23), min(b_0.zw, _S24));
            out_0.texcoord_0 = (local_2 - _S22) * _S14;
            a_0 = a_2;
            b_1 = b_2;
        }
        else
        {
            a_0 = a_1;
            b_1 = b_0;
        }
        var _S25 : vec2<f32> = a_0.zw;
        var _S26 : vec2<f32> = a_0.xy;
        if((all(((_S25 > _S26)))))
        {
            var _S27 : vec2<f32> = (local_2 - (_S26 + _S25) * _S16) / ((_S25 - _S26) * _S16);
            out_0.inside_0.x = _S27.x;
            out_0.inside_0.y = _S27.y;
        }
        var _S28 : vec2<f32> = b_1.zw;
        var _S29 : vec2<f32> = b_1.xy;
        if((all(((_S28 > _S29)))))
        {
            var _S30 : vec2<f32> = (local_2 - (_S29 + _S28) * _S16) / ((_S28 - _S29) * _S16);
            out_0.inside_0.z = _S30.x;
            out_0.inside_0.w = _S30.y;
        }
    }
    return out_0;
}

