//! What a text node lays its text out in (Compose's LayoutUtils.kt): the constraints and line count a paragraph
//! is given from the node's constraints and arguments, and whole pixels from a paragraph's sizes.

use kompoz_graphics::Constraints;
use kompoz_text::TextOverflow;

/// The constraints a paragraph is laid out in (Compose's finalConstraints): no minimums, the width
/// [`final_max_width`] finds, the node's max height.
pub(crate) fn final_constraints(
    constraints: Constraints,
    soft_wrap: bool,
    overflow: TextOverflow,
    max_intrinsic_width: f32,
) -> Constraints {
    Constraints::fit_prioritizing_width(
        0,
        final_max_width(constraints, soft_wrap, overflow, max_intrinsic_width),
        0,
        constraints.max_height,
    )
}

/// The width a paragraph is laid out in (Compose's finalMaxWidth): the node's max width where it wraps or
/// ellipsizes (unbounded otherwise); a fixed width as it is, else the text's unwrapped width within the node's
/// min and that max.
pub(crate) fn final_max_width(
    constraints: Constraints,
    soft_wrap: bool,
    overflow: TextOverflow,
    max_intrinsic_width: f32,
) -> i32 {
    let width_matters = soft_wrap || is_ellipsis(overflow);
    let max_width =
        if width_matters && constraints.has_bounded_width() { constraints.max_width } else { Constraints::Infinity };
    if constraints.min_width == max_width {
        max_width
    } else {
        ceil_to_int_px(max_intrinsic_width).clamp(constraints.min_width, max_width)
    }
}

/// The lines a paragraph may have (Compose's finalMaxLines): one where text that doesn't wrap is ellipsized (a
/// layout ellipsizes one line, not each), else `max_lines` (one at least).
pub(crate) fn final_max_lines(soft_wrap: bool, overflow: TextOverflow, max_lines: usize) -> usize {
    if !soft_wrap && is_ellipsis(overflow) { 1 } else { max_lines.max(1) }
}

/// Whether `overflow` ellipsizes (Compose's TextOverflow.isEllipsis).
pub(crate) fn is_ellipsis(overflow: TextOverflow) -> bool {
    matches!(overflow, TextOverflow::Ellipsis | TextOverflow::StartEllipsis | TextOverflow::MiddleEllipsis)
}

/// `v` rounded up to a whole pixel (Compose's ceilToIntPx), saturating.
pub(crate) fn ceil_to_int_px(v: f32) -> i32 {
    v.ceil() as i32
}

/// `v` rounded to the nearest whole pixel, a half up (Compose's fastRoundToInt).
pub(crate) fn round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}
