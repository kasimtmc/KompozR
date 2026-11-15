diagnostic(off, derivative_uniformity);
struct Uniforms_std430_0
{
    @align(16) toClip_0 : vec4<f32>,
    @align(16) texels_0 : vec4<f32>,
    @align(16) blend_0 : vec4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms_0 : Uniforms_std430_0;
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
fn vsMesh( _S1 : vertexInput_0, @builtin(vertex_index) vertex_0 : u32) -> Varyings_0
{
    var i_0 : i32 = i32(vertex_0 % u32(3));
    var _S2 : bool = i_0 == i32(0);
    var placed_0 : vec3<f32>;
    if(_S2)
    {
        placed_0 = _S1.bounds_0.xyz;
    }
    else
    {
        if(i_0 == i32(1))
        {
            placed_0 = vec3<f32>(_S1.bounds_0.w, _S1.shape_1.xy);
        }
        else
        {
            placed_0 = vec3<f32>(_S1.shape_1.zw, _S1.radii_1.x);
        }
    }
    var texcoord_1 : vec2<f32>;
    if(_S2)
    {
        texcoord_1 = _S1.radii_1.yz;
    }
    else
    {
        if(i_0 == i32(1))
        {
            texcoord_1 = vec2<f32>(_S1.radii_1.w, _S1.uv_1.x);
        }
        else
        {
            texcoord_1 = _S1.uv_1.yz;
        }
    }
    var out_0 : Varyings_0;
    var _S3 : f32 = placed_0.z;
    out_0.position_0 = vec4<f32>(placed_0.xy * uniforms_0.toClip_0.xy + vec2<f32>(_S3) * uniforms_0.toClip_0.zw, 0.0f, _S3);
    out_0.local_0 = vec2<f32>(0.0f, 0.0f);
    out_0.w_0 = _S3;
    out_0.texcoord_0 = texcoord_1;
    var _S4 : vec4<f32>;
    if(_S2)
    {
        _S4 = _S1.xform_0;
    }
    else
    {
        if(i_0 == i32(1))
        {
            _S4 = _S1.extra_0;
        }
        else
        {
            _S4 = _S1.projection_0;
        }
    }
    out_0.vertexColor_0 = _S4;
    const _S5 : vec4<f32> = vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
    out_0.shape_0 = _S5;
    out_0.radii_0 = _S5;
    out_0.color_0 = _S1.color_1;
    out_0.params_0 = _S1.params_1;
    out_0.uv_0 = _S5;
    out_0.kindFlags_0 = vec2<f32>(10.0f, 0.0f);
    out_0.paint_0 = _S1.paint_1;
    out_0.gradient_0 = _S1.gradient_1;
    out_0.filter_0 = _S1.filter_1;
    out_0.steps_0 = _S5;
    out_0.inside_0 = vec4<f32>(2.0f, 2.0f, 2.0f, 2.0f);
    return out_0;
}

