diagnostic(off, derivative_uniformity);
struct Uniforms_std430_0
{
    @align(16) toClip_0 : vec4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms_0 : Uniforms_std430_0;
var<private> corners_0 : array<vec2<f32>, i32(6)> = array<vec2<f32>, i32(6)>( vec2<f32>(0.0f, 0.0f), vec2<f32>(1.0f, 0.0f), vec2<f32>(1.0f, 1.0f), vec2<f32>(0.0f, 0.0f), vec2<f32>(1.0f, 1.0f), vec2<f32>(0.0f, 1.0f) );
struct Varyings_0
{
    @builtin(position) position_0 : vec4<f32>,
    @interpolate(flat) @location(0) edge_0 : vec4<f32>,
    @interpolate(flat) @location(1) aliased_0 : f32,
};

struct vertexInput_0
{
    @location(0) edge_1 : vec4<f32>,
    @location(1) region_0 : vec4<f32>,
    @location(2) mode_0 : vec4<f32>,
};

@vertex
fn vs( _S1 : vertexInput_0, @builtin(vertex_index) vertex_0 : u32) -> Varyings_0
{
    var _S2 : u32 = vertex_0 % u32(6);
    var _S3 : f32 = _S1.edge_1.y;
    var _S4 : f32 = _S1.edge_1.w;
    var _S5 : vec4<f32> = vec4<f32>(max(vec2<f32>(floor(min(_S1.edge_1.x, _S1.edge_1.z)), floor(min(_S3, _S4))), _S1.region_0.xy), min(vec2<f32>(_S1.region_0.z, ceil(max(_S3, _S4))), _S1.region_0.zw));
    var quad_0 : vec4<f32> = _S5;
    var _S6 : vec2<f32> = max(_S5.zw, _S5.xy);
    quad_0.z = _S6.x;
    quad_0.w = _S6.y;
    var out_0 : Varyings_0;
    out_0.position_0 = vec4<f32>(mix(quad_0.xy, quad_0.zw, corners_0[_S2]) * uniforms_0.toClip_0.xy + uniforms_0.toClip_0.zw, 0.0f, 1.0f);
    out_0.edge_0 = _S1.edge_1;
    out_0.aliased_0 = _S1.mode_0.x;
    return out_0;
}

