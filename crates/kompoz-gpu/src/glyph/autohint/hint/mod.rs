//! Entry point to hinting algorithm.

pub(super) mod diacritics;
mod edges;
mod outline;

use super::{
    ScaleFlags,
    adjust::{self, ReverseCharmap},
    metrics::{Scale, UnscaledStyleMetrics, scale_style_metrics},
    outline::Outline,
    style::{GlyphStyle, ScriptGroup},
    topo::{self, Axis, Dimension},
};

/// Captures adjusted horizontal scale and outer edge positions to be used
/// for horizontal metrics adjustments.
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub(crate) struct EdgeMetrics {
    pub left_opos: i32,
    pub left_pos: i32,
    pub right_opos: i32,
    pub right_pos: i32,
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub(crate) struct HintedMetrics {
    pub x_scale: i32,
    /// This will be `None` when we've identified fewer than two horizontal
    /// edges in the outline. This will occur for empty outlines and outlines
    /// that are degenerate (all x coordinates have the same value, within
    /// a threshold). In these cases, horizontal metrics will not be adjusted.
    pub edge_metrics: Option<EdgeMetrics>,
}

/// Applies the complete hinting process to a latin outline, with FreeType
/// 2.14's diacritic adjustments for `glyph` by its character in
/// `reverse_charmap` (see [`diacritics`]).
///
/// FreeType analyzes both axes before fitting either; here each axis is
/// analyzed and fitted in turn, which comes to the same since neither reads
/// what the other moves. FreeType 2.14's steps keep its order on the
/// vertical axis: a tilde stretched and aligned before the axis is analyzed,
/// blue zones ignored for a diacritic attached to its base before edges are
/// given blue zones, and diacritics moved clear of their base once the axis
/// is fitted.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L3554>
pub(crate) fn hint_outline(
    outline: &mut Outline,
    metrics: &UnscaledStyleMetrics,
    scale: &Scale,
    glyph_style: Option<GlyphStyle>,
    glyph: u32,
    reverse_charmap: &ReverseCharmap,
) -> HintedMetrics {
    let scaled_metrics = scale_style_metrics(metrics, *scale);
    let scale = &scaled_metrics.scale;
    let mut axis = Axis::default();
    let hint_top_to_bottom = metrics.style_class().script.hint_top_to_bottom;
    outline.scale(&scaled_metrics.scale);
    let mut hinted_metrics = HintedMetrics { x_scale: scale.x_scale, ..Default::default() };
    let group = metrics.style_class().script.group;
    // For default script group, we don't proceed with hinting if we're
    // missing alignment zones. FreeType swaps in a "dummy" hinter here
    // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.c#L475>
    if group == ScriptGroup::Default && scaled_metrics.axes[1].blues.is_empty() {
        return hinted_metrics;
    }
    // The glyph's character's adjustments (only the Latin writing system has
    // a reverse character map).
    let flags = if group == ScriptGroup::Default { reverse_charmap.flags(glyph) } else { 0 };
    // The diacritic height limit of the vertical separation (0 without
    // flags, or with NO_HEIGHT_CHECK: no diacritic is moved then).
    let mut accent_height_limit = 0;
    for dim in [Dimension::Horizontal, Dimension::Vertical] {
        if (dim == Dimension::Horizontal && scale.flags.contains(ScaleFlags::NO_HORIZONTAL))
            || (dim == Dimension::Vertical && scale.flags.contains(ScaleFlags::NO_VERTICAL))
        {
            continue;
        }
        if dim == Dimension::Vertical {
            diacritics::fit_tildes(outline, flags);
        }
        axis.reset(dim, outline.orientation);
        topo::compute_segments(outline, &mut axis, group);
        topo::link_segments(outline, &mut axis, scaled_metrics.axes[dim].scale, group, metrics.axes[dim].max_width());
        topo::compute_edges(
            &mut axis,
            &scaled_metrics.axes[dim],
            hint_top_to_bottom,
            scaled_metrics.scale.y_scale,
            group,
        );
        if dim == Dimension::Vertical {
            if flags != 0 {
                let blues = &scaled_metrics.axes[dim].blues;
                if flags & adjust::NO_HEIGHT_CHECK == 0 {
                    accent_height_limit = diacritics::accent_height_limit(blues, scale.units_per_em, scale.y_scale);
                }
                diacritics::ignore_blues(&mut axis, blues, flags);
            }
            if group != ScriptGroup::Default || glyph_style.map(|style| !style.is_non_base()).unwrap_or(true) {
                topo::compute_blue_edges(
                    &mut axis,
                    scale,
                    &metrics.axes[dim].blues,
                    &scaled_metrics.axes[dim].blues,
                    group,
                );
            }
        } else {
            hinted_metrics.x_scale = scaled_metrics.axes[0].scale;
        }
        edges::hint_edges(&mut axis, outline, &scaled_metrics.axes[dim], group, scale, hint_top_to_bottom);
        outline::align_edge_points(outline, &axis, group, scale);
        outline::align_strong_points(outline, &mut axis);
        outline::align_weak_points(outline, dim);
        if dim == Dimension::Vertical {
            diacritics::separate(outline, flags, accent_height_limit);
        }
        if dim == Dimension::Horizontal && axis.edges.len() > 1 {
            let left = axis.edges.first().unwrap();
            let right = axis.edges.last().unwrap();
            hinted_metrics.edge_metrics = Some(EdgeMetrics {
                left_pos: left.pos,
                left_opos: left.opos,
                right_pos: right.pos,
                right_opos: right.opos,
            });
        }
    }
    hinted_metrics
}
