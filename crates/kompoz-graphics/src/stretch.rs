//! Android's stretch overscroll as its renderer draws it (hwui's `StretchEffect`, libs/hwui/effects/StretchEffect.h
//! and .cpp, AOSP, Copyright 2021 The Android Open Source Project, Apache License 2.0): a layer property that warps
//! what the layer drew, pulled from an edge, the content nearest the edge stretched most. Android's EdgeEffect sets
//! it on the RenderNode it draws into (RenderNode.stretch, merged with what is set already); foundation's stretch
//! overscroll sets it on a graphics layer the same way.
//!
//! What it holds is hwui's: the stretch direction (each axis -1 to 1 of the layer's size: positive pulled from the
//! left or top, negative from the right or bottom), and how far it may stretch at most each way in px (EdgeEffect's
//! size: kept, but the warp itself doesn't read it, as hwui's doesn't). The warp is the GPU renderer's port of hwui's
//! shader; [`StretchEffect::uniforms`] works out what hwui's `getShader` gives that shader, and
//! [`StretchUniforms::content_position`] is the shader's own mapping, on the CPU.

use crate::geometry::Offset;
use crate::matrix::Matrix;

/// The epsilon a stretch is none within (hwui's NON_ZERO_EPSILON): less than in MathUtils, as the range is 0-1 for an
/// entire screen and should be well under a pixel for a smooth stretch animation (1/25,000 of a screen: a quarter
/// pixel cut off on screens of less than ~8000 px).
const NON_ZERO_EPSILON: f32 = 0.00004;

/// Whether `value` is none as a stretch (hwui's StretchEffect::isZero).
fn is_zero(value: f32) -> bool {
    value.abs() <= NON_ZERO_EPSILON
}

/// The shader's interpolation strength (hwui's INTERPOLATION_STRENGTH_VALUE): 0 stretches evenly over all of the
/// layer, 1 the most at the edge pulled and none at the other.
pub const STRETCH_INTERPOLATION_STRENGTH: f32 = 0.7;

/// A layer's stretch (hwui's StretchEffect): its direction, and the most it may stretch each way (px). Two are equal
/// when all three are (hwui's operator==).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct StretchEffect {
    /// The most it may stretch across, px (hwui's maxStretchAmountX: EdgeEffect's width).
    pub max_stretch_amount_x: f32,
    /// The most it may stretch down, px (hwui's maxStretchAmountY: EdgeEffect's height).
    pub max_stretch_amount_y: f32,
    stretch_direction: Offset,
}

impl StretchEffect {
    /// None (hwui's StretchEffect()).
    pub const Empty: StretchEffect =
        StretchEffect { max_stretch_amount_x: 0.0, max_stretch_amount_y: 0.0, stretch_direction: Offset::Zero };

    /// Stretched by `direction` (each axis a fraction of the layer's size: positive from the left or top, negative
    /// from the right or bottom), `max_stretch_amount_x` and `_y` px at most.
    pub const fn new(direction: Offset, max_stretch_amount_x: f32, max_stretch_amount_y: f32) -> StretchEffect {
        StretchEffect { max_stretch_amount_x, max_stretch_amount_y, stretch_direction: direction }
    }

    /// Its direction (hwui's getStretchDirection).
    pub fn stretch_direction(&self) -> Offset {
        self.stretch_direction
    }

    /// Whether it stretches nothing: its direction none either way (hwui's isEmpty).
    pub fn is_empty(&self) -> bool {
        is_zero(self.stretch_direction.x) && is_zero(self.stretch_direction.y)
    }

    /// Made none (hwui's setEmpty).
    pub fn set_empty(&mut self) {
        *self = StretchEffect::Empty;
    }

    /// `other` added to it (hwui's mergeWith, what RenderNode.stretch does): none changes nothing; onto none it is
    /// `other`; else the directions add up (none if they cancel out) and each most is the larger.
    pub fn merge_with(&mut self, other: &StretchEffect) {
        if other.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = *other;
            return;
        }
        self.stretch_direction = self.stretch_direction + other.stretch_direction;
        if self.is_empty() {
            return self.set_empty();
        }
        self.max_stretch_amount_x = self.max_stretch_amount_x.max(other.max_stretch_amount_x);
        self.max_stretch_amount_y = self.max_stretch_amount_y.max(other.max_stretch_amount_y);
    }

    /// Whether the layer it is on has to be drawn offscreen (hwui's requiresLayer): whenever it stretches.
    pub fn requires_layer(&self) -> bool {
        !self.is_empty()
    }

    /// A layer of `width` x `height` px scaled from the edge pulled instead of warped (hwui's makeLinearStretch:
    /// Android's StretchEffectBehavior::UniformScale).
    pub fn make_linear_stretch(&self, width: f32, height: f32) -> Matrix {
        let Offset { x: sx, y: sy } = self.stretch_direction;
        let (px, py) = (if sx > 0.0 { 0.0 } else { width }, if sy > 0.0 { 0.0 } else { height });
        let mut m = Matrix::new();
        m.translate(px, py, 0.0);
        m.scale(1.0 + sx.abs(), 1.0 + sy.abs(), 1.0);
        m.translate(-px, -py, 0.0);
        m
    }

    /// Where a point `normalized_x` (0 to 1) across the content shows stretched, a fraction of the layer's width
    /// (hwui's computeStretchedPositionX: the warp's reverse).
    pub fn compute_stretched_position_x(&self, normalized_x: f32) -> f32 {
        compute_reverse_overscroll(self.stretch_direction.x, normalized_x)
    }

    /// The same down (hwui's computeStretchedPositionY).
    pub fn compute_stretched_position_y(&self, normalized_y: f32) -> f32 {
        compute_reverse_overscroll(self.stretch_direction.y, normalized_y)
    }

    /// What hwui's shader is given for a layer of `width` x `height` px (StretchEffect::getShader's uniforms); none
    /// when it stretches nothing.
    pub fn uniforms(&self, width: f32, height: f32) -> Option<StretchUniforms> {
        if self.is_empty() {
            return None;
        }
        let norm_over_scroll_dist_x = self.stretch_direction.x;
        let norm_over_scroll_dist_y = self.stretch_direction.y;
        let distance_stretched_x = width / (1.0 + norm_over_scroll_dist_x.abs());
        let distance_stretched_y = height / (1.0 + norm_over_scroll_dist_y.abs());
        let inverse_distance_stretched_x = 1.0 / width;
        let inverse_distance_stretched_y = 1.0 / height;
        let diff_x = distance_stretched_x - width;
        let diff_y = distance_stretched_y - height;
        Some(StretchUniforms {
            max_stretch_intensity: 0.0, // (declared by the shader, never set by getShader nor read)
            stretch_affected_dist_x: width,
            stretch_affected_dist_y: height,
            distance_stretched_x,
            distance_stretched_y,
            inverse_distance_stretched_x,
            inverse_distance_stretched_y,
            dist_diff_x: diff_x,
            dist_diff_y: diff_y,
            scroll_x: 0.0,
            scroll_y: 0.0,
            overscroll_x: norm_over_scroll_dist_x,
            overscroll_y: norm_over_scroll_dist_y,
            viewport_width: width,
            viewport_height: height,
            interpolation_strength: STRETCH_INTERPOLATION_STRENGTH,
        })
    }
}

/// hwui's reverseMapStart: where `input` (normalized) shows, overscrolled from the top or left.
fn reverse_map_start(overscroll: f32, input: f32) -> f32 {
    let numerator = (-input * overscroll * overscroll) - (2.0 * input * overscroll) - input;
    let denominator = 1.0 + (0.3 * overscroll) + (0.7 * input * overscroll * overscroll) + (0.7 * input * overscroll);
    -(numerator / denominator)
}

/// hwui's reverseMapEnd: the same overscrolled from the bottom or right.
fn reverse_map_end(overscroll: f32, input: f32) -> f32 {
    let numerator = (0.3 * overscroll * overscroll) - (0.3 * input * overscroll * overscroll)
        + (1.3 * input * overscroll)
        - overscroll
        - input;
    let denominator =
        (0.7 * input * overscroll * overscroll) - (0.7 * input * overscroll) - (0.7 * overscroll * overscroll)
            + overscroll
            - 1.0;
    numerator / denominator
}

/// hwui's computeReverseOverscroll: where `normalized_input` shows, from the top or left (`overscroll` positive) or
/// the bottom or right (negative), as hwui works it out.
fn compute_reverse_overscroll(overscroll: f32, normalized_input: f32) -> f32 {
    let distance_stretched = 1.0 / (1.0 + overscroll.abs());
    let distance_diff = distance_stretched - 1.0;
    if overscroll > 0.0 {
        let output = reverse_map_start(overscroll, normalized_input);
        if output <= 1.0 {
            return output;
        } else if output >= distance_stretched {
            return output - distance_diff;
        }
    }
    if overscroll < 0.0 {
        let output = reverse_map_end(overscroll, normalized_input);
        if output >= 0.0 {
            return output;
        } else if output < 0.0 {
            return output + distance_diff;
        }
    }
    normalized_input
}

/// hwui's stretch shader's uniforms, by their names there (StretchEffect::getShader sets them; the GPU renderer's
/// port of the shader reads them as they are). Lengths are the layer's px.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct StretchUniforms {
    /// uMaxStretchIntensity: declared, never set nor read.
    pub max_stretch_intensity: f32,
    /// uStretchAffectedDistX, Y: the layer's size.
    pub stretch_affected_dist_x: f32,
    pub stretch_affected_dist_y: f32,
    /// uDistanceStretchedX, Y: the size over one more than the stretch.
    pub distance_stretched_x: f32,
    pub distance_stretched_y: f32,
    /// uInverseDistanceStretchedX, Y: one over the size (what the shader takes the stretched distance as).
    pub inverse_distance_stretched_x: f32,
    pub inverse_distance_stretched_y: f32,
    /// uDistDiffX, Y: the stretched distance less the size.
    pub dist_diff_x: f32,
    pub dist_diff_y: f32,
    /// uScrollX, Y: none.
    pub scroll_x: f32,
    pub scroll_y: f32,
    /// uOverscrollX, Y: the stretch's direction.
    pub overscroll_x: f32,
    pub overscroll_y: f32,
    /// viewportWidth, viewportHeight: the layer's size.
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// uInterpolationStrength.
    pub interpolation_strength: f32,
}

impl StretchUniforms {
    /// Their values in the shader's order of them.
    pub fn values(&self) -> [f32; 16] {
        [
            self.max_stretch_intensity,
            self.stretch_affected_dist_x,
            self.stretch_affected_dist_y,
            self.distance_stretched_x,
            self.distance_stretched_y,
            self.inverse_distance_stretched_x,
            self.inverse_distance_stretched_y,
            self.dist_diff_x,
            self.dist_diff_y,
            self.scroll_x,
            self.scroll_y,
            self.overscroll_x,
            self.overscroll_y,
            self.viewport_width,
            self.viewport_height,
            self.interpolation_strength,
        ]
    }

    /// Where the content the stretched layer shows at `coord` (its px) comes from: hwui's shader's `main` before it
    /// samples the content there.
    pub fn content_position(&self, coord: Offset) -> Offset {
        let in_u = coord.x + self.scroll_x;
        let in_v = coord.y + self.scroll_y;
        let out_u = compute_overscroll(
            in_u,
            self.overscroll_x,
            self.stretch_affected_dist_x,
            self.inverse_distance_stretched_x,
            self.distance_stretched_x,
            self.dist_diff_x,
            self.interpolation_strength,
            self.viewport_width,
        );
        let out_v = compute_overscroll(
            in_v,
            self.overscroll_y,
            self.stretch_affected_dist_y,
            self.inverse_distance_stretched_y,
            self.distance_stretched_y,
            self.dist_diff_y,
            self.interpolation_strength,
            self.viewport_height,
        );
        Offset::new(out_u, out_v)
    }
}

/// The shader's easeIn.
fn ease_in(t: f32, d: f32) -> f32 {
    t * d
}

/// GLSL's mix.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The shader's computeOverscrollStart.
fn compute_overscroll_start(
    in_pos: f32,
    overscroll: f32,
    stretch_affected_dist: f32,
    inverse_stretch_affected_dist: f32,
    distance_stretched: f32,
    interpolation_strength: f32,
) -> f32 {
    let offset_pos = stretch_affected_dist - in_pos;
    let pos_based_variation = mix(1.0, ease_in(offset_pos, inverse_stretch_affected_dist), interpolation_strength);
    let stretch_intensity = overscroll * pos_based_variation;
    distance_stretched - (offset_pos / (1.0 + stretch_intensity))
}

/// The shader's computeOverscrollEnd.
#[allow(clippy::too_many_arguments)]
fn compute_overscroll_end(
    in_pos: f32,
    overscroll: f32,
    reverse_stretch_dist: f32,
    _stretch_affected_dist: f32,
    inverse_stretch_affected_dist: f32,
    distance_stretched: f32,
    interpolation_strength: f32,
    viewport_dimension: f32,
) -> f32 {
    let offset_pos = in_pos - reverse_stretch_dist;
    let pos_based_variation = mix(1.0, ease_in(offset_pos, inverse_stretch_affected_dist), interpolation_strength);
    let stretch_intensity = (-overscroll) * pos_based_variation;
    viewport_dimension - (distance_stretched - (offset_pos / (1.0 + stretch_intensity)))
}

/// The shader's computeOverscroll: along one axis.
#[allow(clippy::too_many_arguments)]
fn compute_overscroll(
    in_pos: f32,
    overscroll: f32,
    stretch_affected_dist: f32,
    inverse_stretch_affected_dist: f32,
    distance_stretched: f32,
    distance_diff: f32,
    interpolation_strength: f32,
    viewport_dimension: f32,
) -> f32 {
    if overscroll > 0.0 {
        if in_pos <= stretch_affected_dist {
            compute_overscroll_start(
                in_pos,
                overscroll,
                stretch_affected_dist,
                inverse_stretch_affected_dist,
                distance_stretched,
                interpolation_strength,
            )
        } else {
            distance_diff + in_pos
        }
    } else if overscroll < 0.0 {
        let stretch_affected_dist_end = viewport_dimension - stretch_affected_dist;
        if in_pos >= stretch_affected_dist_end {
            compute_overscroll_end(
                in_pos,
                overscroll,
                stretch_affected_dist_end,
                stretch_affected_dist,
                inverse_stretch_affected_dist,
                distance_stretched,
                interpolation_strength,
                viewport_dimension,
            )
        } else {
            -distance_diff + in_pos
        }
    } else {
        in_pos
    }
}
