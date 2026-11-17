diagnostic(off, derivative_uniformity);
struct Uniforms_std430_0
{
    @align(16) toClip_0 : vec4<f32>,
    @align(16) local_0 : vec4<f32>,
    @align(16) source_0 : vec4<f32>,
    @align(16) edges_0 : vec4<f32>,
    @align(16) uMaxStretchIntensity_0 : f32,
    @align(4) uStretchAffectedDistX_0 : f32,
    @align(8) uStretchAffectedDistY_0 : f32,
    @align(4) uDistanceStretchedX_0 : f32,
    @align(16) uDistanceStretchedY_0 : f32,
    @align(4) uInverseDistanceStretchedX_0 : f32,
    @align(8) uInverseDistanceStretchedY_0 : f32,
    @align(4) uDistDiffX_0 : f32,
    @align(16) uDistDiffY_0 : f32,
    @align(4) uScrollX_0 : f32,
    @align(8) uScrollY_0 : f32,
    @align(4) uOverscrollX_0 : f32,
    @align(16) uOverscrollY_0 : f32,
    @align(4) viewportWidth_0 : f32,
    @align(8) viewportHeight_0 : f32,
    @align(4) uInterpolationStrength_0 : f32,
};

@binding(0) @group(0) var<uniform> uniforms_0 : Uniforms_std430_0;
@binding(1) @group(0) var uContentTexture_texture_0 : texture_2d<f32>;

@binding(2) @group(0) var uContentTexture_sampler_0 : sampler;

fn easeIn_0( t_0 : f32,  d_0 : f32) -> f32
{
    return t_0 * d_0;
}

fn computeOverscrollStart_0( inPos_0 : f32,  overscroll_0 : f32,  uStretchAffectedDist_0 : f32,  uInverseStretchAffectedDist_0 : f32,  distanceStretched_0 : f32,  interpolationStrength_0 : f32) -> f32
{
    var offsetPos_0 : f32 = uStretchAffectedDist_0 - inPos_0;
    return distanceStretched_0 - offsetPos_0 / (1.0f + overscroll_0 * mix(1.0f, easeIn_0(offsetPos_0, uInverseStretchAffectedDist_0), interpolationStrength_0));
}

fn computeOverscrollEnd_0( inPos_1 : f32,  overscroll_1 : f32,  reverseStretchDist_0 : f32,  uStretchAffectedDist_1 : f32,  uInverseStretchAffectedDist_1 : f32,  distanceStretched_1 : f32,  interpolationStrength_1 : f32,  viewportDimension_0 : f32) -> f32
{
    var offsetPos_1 : f32 = inPos_1 - reverseStretchDist_0;
    return viewportDimension_0 - (distanceStretched_1 - offsetPos_1 / (1.0f + - overscroll_1 * mix(1.0f, easeIn_0(offsetPos_1, uInverseStretchAffectedDist_1), interpolationStrength_1)));
}

fn computeOverscroll_0( inPos_2 : f32,  overscroll_2 : f32,  uStretchAffectedDist_2 : f32,  uInverseStretchAffectedDist_2 : f32,  distanceStretched_2 : f32,  distanceDiff_0 : f32,  interpolationStrength_2 : f32,  viewportDimension_1 : f32) -> f32
{
    if((overscroll_2 > 0.0f))
    {
        if((inPos_2 <= uStretchAffectedDist_2))
        {
            return computeOverscrollStart_0(inPos_2, overscroll_2, uStretchAffectedDist_2, uInverseStretchAffectedDist_2, distanceStretched_2, interpolationStrength_2);
        }
        else
        {
            return distanceDiff_0 + inPos_2;
        }
    }
    else
    {
        if((overscroll_2 < 0.0f))
        {
            var stretchAffectedDist_0 : f32 = viewportDimension_1 - uStretchAffectedDist_2;
            if((inPos_2 >= stretchAffectedDist_0))
            {
                return computeOverscrollEnd_0(inPos_2, overscroll_2, stretchAffectedDist_0, uStretchAffectedDist_2, uInverseStretchAffectedDist_2, distanceStretched_2, interpolationStrength_2, viewportDimension_1);
            }
            else
            {
                return - distanceDiff_0 + inPos_2;
            }
        }
        else
        {
            return inPos_2;
        }
    }
}

fn eval_0( coord_0 : vec2<f32>) -> vec4<f32>
{
    return (textureSampleLevel((uContentTexture_texture_0), (uContentTexture_sampler_0), (clamp(coord_0 * uniforms_0.source_0.xy + uniforms_0.source_0.zw, uniforms_0.edges_0.xy, uniforms_0.edges_0.zw)), (0.0f)));
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
fn fs( _S1 : pixelInput_0, @builtin(position) position_0 : vec4<f32>) -> pixelOutput_0
{
    var _S2 : vec2<f32> = (floor(_S1.texel_0) + vec2<f32>(0.5f)) * uniforms_0.local_0.xy + uniforms_0.local_0.zw;
    var coord_1 : vec2<f32> = _S2;
    var outV_0 : f32 = computeOverscroll_0(_S2.y + uniforms_0.uScrollY_0, uniforms_0.uOverscrollY_0, uniforms_0.uStretchAffectedDistY_0, uniforms_0.uInverseDistanceStretchedY_0, uniforms_0.uDistanceStretchedY_0, uniforms_0.uDistDiffY_0, uniforms_0.uInterpolationStrength_0, uniforms_0.viewportHeight_0);
    coord_1[i32(0)] = computeOverscroll_0(_S2.x + uniforms_0.uScrollX_0, uniforms_0.uOverscrollX_0, uniforms_0.uStretchAffectedDistX_0, uniforms_0.uInverseDistanceStretchedX_0, uniforms_0.uDistanceStretchedX_0, uniforms_0.uDistDiffX_0, uniforms_0.uInterpolationStrength_0, uniforms_0.viewportWidth_0);
    coord_1[i32(1)] = outV_0;
    var _S3 : pixelOutput_0 = pixelOutput_0( eval_0(coord_1) );
    return _S3;
}

