#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
float easeIn_0(float t_0, float d_0)
{
    return t_0 * d_0;
}

float computeOverscrollStart_0(float inPos_0, float overscroll_0, float uStretchAffectedDist_0, float uInverseStretchAffectedDist_0, float distanceStretched_0, float interpolationStrength_0)
{
    float offsetPos_0 = uStretchAffectedDist_0 - inPos_0;
    return distanceStretched_0 - offsetPos_0 / (1.0f + overscroll_0 * mix(1.0f, easeIn_0(offsetPos_0, uInverseStretchAffectedDist_0), interpolationStrength_0));
}

float computeOverscrollEnd_0(float inPos_1, float overscroll_1, float reverseStretchDist_0, float uStretchAffectedDist_1, float uInverseStretchAffectedDist_1, float distanceStretched_1, float interpolationStrength_1, float viewportDimension_0)
{
    float offsetPos_1 = inPos_1 - reverseStretchDist_0;
    return viewportDimension_0 - (distanceStretched_1 - offsetPos_1 / (1.0f + - overscroll_1 * mix(1.0f, easeIn_0(offsetPos_1, uInverseStretchAffectedDist_1), interpolationStrength_1)));
}

float computeOverscroll_0(float inPos_2, float overscroll_2, float uStretchAffectedDist_2, float uInverseStretchAffectedDist_2, float distanceStretched_2, float distanceDiff_0, float interpolationStrength_2, float viewportDimension_1)
{
    if(overscroll_2 > 0.0f)
    {
        if(inPos_2 <= uStretchAffectedDist_2)
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
        if(overscroll_2 < 0.0f)
        {
            float stretchAffectedDist_0 = viewportDimension_1 - uStretchAffectedDist_2;
            if(inPos_2 >= stretchAffectedDist_0)
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

struct Uniforms_0
{
    float4 toClip_0;
    float4 local_0;
    float4 source_0;
    float4 edges_0;
    float uMaxStretchIntensity_0;
    float uStretchAffectedDistX_0;
    float uStretchAffectedDistY_0;
    float uDistanceStretchedX_0;
    float uDistanceStretchedY_0;
    float uInverseDistanceStretchedX_0;
    float uInverseDistanceStretchedY_0;
    float uDistDiffX_0;
    float uDistDiffY_0;
    float uScrollX_0;
    float uScrollY_0;
    float uOverscrollX_0;
    float uOverscrollY_0;
    float viewportWidth_0;
    float viewportHeight_0;
    float uInterpolationStrength_0;
};

struct KernelContext_0
{
    Uniforms_0 constant* uniforms_0;
    texture2d<float, access::sample> uContentTexture_texture_0;
    sampler uContentTexture_sampler_0;
};

float4 eval_0(float2 coord_0, KernelContext_0 thread* kernelContext_0)
{
    return ((kernelContext_0->uContentTexture_texture_0).sample((kernelContext_0->uContentTexture_sampler_0), (clamp(coord_0 * kernelContext_0->uniforms_0->source_0.xy + kernelContext_0->uniforms_0->source_0.zw, kernelContext_0->uniforms_0->edges_0.xy, kernelContext_0->uniforms_0->edges_0.zw)), level((0.0f))));
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    float2 texel_0 [[user(TEXEL)]];
};

[[fragment]] pixelOutput_0 fs(pixelInput_0 _S1 [[stage_in]], float4 position_0 [[position]], Uniforms_0 constant* uniforms_1 [[buffer(0)]], texture2d<float, access::sample> uContentTexture_texture_1 [[texture(0)]], sampler uContentTexture_sampler_1 [[sampler(0)]])
{
    thread KernelContext_0 kernelContext_1;
    (&kernelContext_1)->uniforms_0 = uniforms_1;
    (&kernelContext_1)->uContentTexture_texture_0 = uContentTexture_texture_1;
    (&kernelContext_1)->uContentTexture_sampler_0 = uContentTexture_sampler_1;
    float2 _S2 = (floor(_S1.texel_0) + float2(0.5f) ) * uniforms_1->local_0.xy + uniforms_1->local_0.zw;
    thread float2 coord_1 = _S2;
    float outV_0 = computeOverscroll_0(_S2.y + uniforms_1->uScrollY_0, uniforms_1->uOverscrollY_0, uniforms_1->uStretchAffectedDistY_0, uniforms_1->uInverseDistanceStretchedY_0, uniforms_1->uDistanceStretchedY_0, uniforms_1->uDistDiffY_0, uniforms_1->uInterpolationStrength_0, uniforms_1->viewportHeight_0);
    coord_1.x = computeOverscroll_0(_S2.x + uniforms_1->uScrollX_0, uniforms_1->uOverscrollX_0, uniforms_1->uStretchAffectedDistX_0, uniforms_1->uInverseDistanceStretchedX_0, uniforms_1->uDistanceStretchedX_0, uniforms_1->uDistDiffX_0, uniforms_1->uInterpolationStrength_0, uniforms_1->viewportWidth_0);
    coord_1.y = outV_0;
    float4 _S3 = eval_0(coord_1, &kernelContext_1);
    pixelOutput_0 _S4 = { _S3 };
    return _S4;
}

