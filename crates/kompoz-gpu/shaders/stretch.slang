// Android's stretch overscroll (a layer's StretchEffect), ported from hwui's stretch shader
// (libs/hwui/effects/StretchEffect.cpp's stretchShader, SkSL) to Slang:
//
//   Copyright (C) 2021 The Android Open Source Project
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.
//
// Changed from hwui's: a pass that makes a texture of the stretched layer, a texel at a time (the
// renderer composites it as it does an effect's), instead of a shader painting the layer's rect;
// the point a texel is at in the layer's own pixels (what Skia hands the shader as `coord`) and
// where those are in the content's texture come from the uniforms ahead of hwui's (`local`,
// `source`); `uContentTexture.eval` is a linearly filtered sample of the texture clamped to its
// own texels' middles at its edges (SkTileMode::kClamp with SkFilterMode::kLinear); hwui's
// uniforms, set as StretchEffect::getShader sets them, and its functions are as they are there
// (`mix` is Slang's `lerp`).

#ifdef KOMPOZ_SPIRV
#define VERTEX_ID SV_VulkanVertexID
#else
#define VERTEX_ID SV_VertexID
#endif

struct Uniforms {
    float4 toClip;   // clip = position * toClip.xy + toClip.zw
    float4 local;    // the layer's px (from its top left) a destination texel t's middle is at: (t + 0.5) * local.xy + local.zw
    float4 source;   // the content's texture coordinates at a layer px p: p * source.xy + source.zw
    float4 edges;    // the content's texels' middles at its edges, in texture coordinates: x0, y0, x1, y1

    // multiplier to apply to scale effect
    float uMaxStretchIntensity;

    // Maximum percentage to stretch beyond bounds  of target
    float uStretchAffectedDistX;
    float uStretchAffectedDistY;

    // Distance stretched as a function of the normalized overscroll times
    // scale intensity
    float uDistanceStretchedX;
    float uDistanceStretchedY;
    float uInverseDistanceStretchedX;
    float uInverseDistanceStretchedY;
    float uDistDiffX;

    // Difference between the peak stretch amount and overscroll amount normalized
    float uDistDiffY;

    // Horizontal offset represented as a ratio of pixels divided by the target width
    float uScrollX;
    // Vertical offset represented as a ratio of pixels divided by the target height
    float uScrollY;

    // Normalized overscroll amount in the horizontal direction
    float uOverscrollX;

    // Normalized overscroll amount in the vertical direction
    float uOverscrollY;
    float viewportWidth; // target height in pixels
    float viewportHeight; // target width in pixels

    // uInterpolationStrength is the intensity of the interpolation.
    // if uInterpolationStrength is 0, then the stretch is constant for all the
    // uStretchAffectedDist. if uInterpolationStrength is 1, then stretch intensity
    // is interpolated based on the pixel position in the uStretchAffectedDist area;
    // The closer we are from the scroll anchor point, the more it stretches,
    // and the other way around.
    float uInterpolationStrength;
};
[[vk::push_constant]] ConstantBuffer<Uniforms> uniforms;

[[vk::binding(1, 0)]] Sampler2D uContentTexture;

struct Instance {
    float4 rect : RECT;  // what it draws: x0, y0, x1, y1
};

struct Varyings {
    float4 position : SV_Position;
    float2 texel : TEXEL;  // where in the destination
};

[shader("vertex")]
Varyings vs(Instance instance, uint vertex : VERTEX_ID) {
    static const float2 corners[6] = { float2(0, 0), float2(1, 0), float2(1, 1), float2(0, 0), float2(1, 1), float2(0, 1) };
    float2 p = lerp(instance.rect.xy, instance.rect.zw, corners[vertex % 6]);
    Varyings out;
    out.position = float4(p * uniforms.toClip.xy + uniforms.toClip.zw, 0, 1);
    out.texel = p;
    return out;
}

float easeIn(float t, float d) {
    return t * d;
}

float computeOverscrollStart(
    float inPos,
    float overscroll,
    float uStretchAffectedDist,
    float uInverseStretchAffectedDist,
    float distanceStretched,
    float interpolationStrength
) {
    float offsetPos = uStretchAffectedDist - inPos;
    float posBasedVariation = lerp(
            1. ,easeIn(offsetPos, uInverseStretchAffectedDist), interpolationStrength);
    float stretchIntensity = overscroll * posBasedVariation;
    return distanceStretched - (offsetPos / (1. + stretchIntensity));
}

float computeOverscrollEnd(
    float inPos,
    float overscroll,
    float reverseStretchDist,
    float uStretchAffectedDist,
    float uInverseStretchAffectedDist,
    float distanceStretched,
    float interpolationStrength,
    float viewportDimension
) {
    float offsetPos = inPos - reverseStretchDist;
    float posBasedVariation = lerp(
            1. ,easeIn(offsetPos, uInverseStretchAffectedDist), interpolationStrength);
    float stretchIntensity = (-overscroll) * posBasedVariation;
    return viewportDimension - (distanceStretched - (offsetPos / (1. + stretchIntensity)));
}

// Prefer usage of return values over out parameters as it enables
// SKSL to properly inline method calls and works around potential GPU
// driver issues on Wembly. See b/182566543 for details
float computeOverscroll(
    float inPos,
    float overscroll,
    float uStretchAffectedDist,
    float uInverseStretchAffectedDist,
    float distanceStretched,
    float distanceDiff,
    float interpolationStrength,
    float viewportDimension
) {
  if (overscroll > 0) {
    if (inPos <= uStretchAffectedDist) {
        return computeOverscrollStart(
          inPos,
          overscroll,
          uStretchAffectedDist,
          uInverseStretchAffectedDist,
          distanceStretched,
          interpolationStrength
        );
    } else {
        return distanceDiff + inPos;
    }
  } else if (overscroll < 0) {
    float stretchAffectedDist = viewportDimension - uStretchAffectedDist;
    if (inPos >= stretchAffectedDist) {
        return computeOverscrollEnd(
          inPos,
          overscroll,
          stretchAffectedDist,
          uStretchAffectedDist,
          uInverseStretchAffectedDist,
          distanceStretched,
          interpolationStrength,
          viewportDimension
        );
    } else {
        return -distanceDiff + inPos;
    }
  } else {
    return inPos;
  }
}

// uContentTexture.eval(coord): the content at the layer px `coord`, clamped to its edges and
// filtered linearly.
float4 eval(float2 coord) {
    float2 at = clamp(coord * uniforms.source.xy + uniforms.source.zw, uniforms.edges.xy, uniforms.edges.zw);
    return uContentTexture.SampleLevel(at, 0);
}

[shader("fragment")]
float4 fs(Varyings v) : SV_Target {
    float2 coord = (floor(v.texel) + 0.5) * uniforms.local.xy + uniforms.local.zw;
    float inU = coord.x;
    float inV = coord.y;
    float outU;
    float outV;

    inU += uniforms.uScrollX;
    inV += uniforms.uScrollY;
    outU = computeOverscroll(
        inU,
        uniforms.uOverscrollX,
        uniforms.uStretchAffectedDistX,
        uniforms.uInverseDistanceStretchedX,
        uniforms.uDistanceStretchedX,
        uniforms.uDistDiffX,
        uniforms.uInterpolationStrength,
        uniforms.viewportWidth
    );
    outV = computeOverscroll(
        inV,
        uniforms.uOverscrollY,
        uniforms.uStretchAffectedDistY,
        uniforms.uInverseDistanceStretchedY,
        uniforms.uDistanceStretchedY,
        uniforms.uDistDiffY,
        uniforms.uInterpolationStrength,
        uniforms.viewportHeight
    );
    coord.x = outU;
    coord.y = outV;
    return eval(coord);
}
