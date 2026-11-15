//! FreeType 2.14's diacritic steps in the Latin hinter (`aflatin.c`), by the adjustment flags of a glyph's
//! character (see [`adjust`](super::super::adjust)):
//!
//! - before the vertical axis is analyzed, a tilde above (or below) its base stretched to a pixel taller than its
//!   wiggles' depth so that it doesn't flatten into a bar, aligned to the pixel grid (its center, when it is less
//!   than three pixels tall), and kept out of the hinting with every contour beyond it (its points left out of
//!   segments and touched, so that nothing moves them);
//! - before edges are given blue zones, the zones ignored above (or below) a base whose diacritic is attached to it
//!   (a cedilla, an ogonek, a hook): no edge beyond the base's zones, by a seventh of its height and a quarter of a
//!   pixel, aligns to a zone;
//! - once the vertical axis is fitted, a diacritic above (or below) that has come within a pixel of the rest moved
//!   up (down) until a pixel lies between them, with whatever lies as high (low), a tilde's centering kept.
//!
//! Contour extrema are kept as FreeType keeps them: computed when it computes them and read, stale, in between.

use super::super::adjust::{
    DOWN, DOWN2, IGNORE_CAPITAL_BOTTOM, IGNORE_CAPITAL_TOP, IGNORE_SMALL_BOTTOM, IGNORE_SMALL_TOP, TILDE_BOTTOM,
    TILDE_BOTTOM2, TILDE_TOP, TILDE_TOP2, UP, UP2,
};
use super::super::metrics::{BlueZones, ScaledBlue, fixed_mul};
use super::super::outline::Outline;
use super::super::topo::{Axis, TopoFlags};
use skrifa::raw::tables::glyf::PointMarker;

/// Each contour's lowest and highest y (`af_compute_vertical_extrema`): (MAX, MIN) for a contour of two points or
/// fewer.
fn vertical_extrema(outline: &Outline) -> Vec<(i64, i64)> {
    outline
        .contours
        .iter()
        .map(|contour| {
            if contour.range().len() <= 2 {
                return (i64::MAX, i64::MIN);
            }
            outline.points[contour.range()]
                .iter()
                .fold((i64::MAX, i64::MIN), |(lo, hi), p| (lo.min(p.y as i64), hi.max(p.y as i64)))
        })
        .collect()
}

/// `af_find_highest_contour`: the contour reaching highest (of two as high, the shorter).
fn highest(ext: &[(i64, i64)]) -> usize {
    let (mut best, mut best_min, mut best_max) = (0, i64::MAX, i64::MIN);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if hi > best_max || (hi == best_max && lo > best_min) {
            (best, best_min, best_max) = (i, lo, hi);
        }
    }
    best
}

/// `af_find_second_highest_contour`: the contour reaching highest of those starting below the highest one (0 for
/// fewer than three contours).
fn second_highest(ext: &[(i64, i64)]) -> usize {
    if ext.len() < 3 {
        return 0;
    }
    let top = highest(ext);
    let top_min = ext[top].0;
    let (mut best, mut best_max) = (0, i64::MIN);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if i != top && hi > best_max && lo < top_min {
            (best, best_max) = (i, hi);
        }
    }
    best
}

/// `af_find_lowest_contour`: the contour reaching lowest (of two as low, the shorter).
fn lowest(ext: &[(i64, i64)]) -> usize {
    let (mut best, mut best_min, mut best_max) = (0, i64::MAX, i64::MIN);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if lo < best_min || (lo == best_min && hi < best_max) {
            (best, best_min, best_max) = (i, lo, hi);
        }
    }
    best
}

/// `af_find_second_lowest_contour`: the contour reaching lowest of those ending above the lowest one (0 for fewer
/// than three contours).
fn second_lowest(ext: &[(i64, i64)]) -> usize {
    if ext.len() < 3 {
        return 0;
    }
    let bottom = lowest(ext);
    let bottom_max = ext[bottom].1;
    let (mut best, mut best_min) = (0, i64::MAX);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if i != bottom && lo < best_min && hi > bottom_max {
            (best, best_min) = (i, lo);
        }
    }
    best
}

/// `af_move_contour_vertically`: contour `contour` moved `delta` up (FreeType moves by an `int`).
fn move_contour(outline: &mut Outline, contour: usize, delta: i64) {
    let range = outline.contours[contour].range();
    for point in &mut outline.points[range] {
        point.y = point.y.wrapping_add(delta as i32);
    }
}

/// `af_move_contours_up`: the contours (not flat) starting above `limit` moved `delta` up.
fn move_contours_up(outline: &mut Outline, ext: &[(i64, i64)], limit: i64, delta: i64) {
    for (contour, &(lo, hi)) in ext.iter().enumerate() {
        if lo < hi && lo > limit {
            move_contour(outline, contour, delta);
        }
    }
}

/// `af_move_contours_down`: the contours (not flat) ending below `limit` moved `delta` down.
fn move_contours_down(outline: &mut Outline, ext: &[(i64, i64)], limit: i64, delta: i64) {
    for (contour, &(lo, hi)) in ext.iter().enumerate() {
        if lo < hi && hi < limit {
            move_contour(outline, contour, delta.wrapping_neg());
        }
    }
}

/// `af_touch_contour`: contour `contour`'s points left out of segments, its on-curve points touched vertically.
fn touch_contour(outline: &mut Outline, contour: usize) {
    let range = outline.contours[contour].range();
    for point in &mut outline.points[range] {
        point.ignore = true;
        if point.is_on_curve() {
            point.flags.set_marker(PointMarker::TOUCHED_Y);
        }
    }
}

/// `af_touch_top_contours`: the contours (not flat) starting at or above contour `limit_contour`'s bottom touched.
fn touch_top_contours(outline: &mut Outline, ext: &[(i64, i64)], limit_contour: usize) {
    let limit = ext[limit_contour].0;
    for (contour, &(lo, hi)) in ext.iter().enumerate() {
        if lo < hi && lo >= limit {
            touch_contour(outline, contour);
        }
    }
}

/// `af_touch_bottom_contours`: the contours (not flat) ending at or below contour `limit_contour`'s bottom (its
/// bottom, as FreeType has it) touched.
fn touch_bottom_contours(outline: &mut Outline, ext: &[(i64, i64)], limit_contour: usize) {
    let limit = ext[limit_contour].0;
    for (contour, &(lo, hi)) in ext.iter().enumerate() {
        if lo < hi && hi <= limit {
            touch_contour(outline, contour);
        }
    }
}

/// The smallest depth of a tilde's wiggles: of its on-curve points between two controls on a level with them, not
/// at its extremes, with the on-curve points before and after both above (or both below), the distance to its
/// bottom (top), those nearer than an eighth of its height to it left out; 0 without any.
fn wiggle_depth(outline: &Outline, contour: usize, min_y: i64, max_y: i64) -> i64 {
    let points = &outline.points;
    let threshold = max_y.wrapping_sub(min_y) / 8;
    let mut depth = None::<i64>;
    for ix in outline.contours[contour].range() {
        let p = &points[ix];
        let (prev, next) = (&points[p.prev()], &points[p.next()]);
        let y = p.y as i64;
        if !(p.is_on_curve()
            && prev.y == p.y
            && next.y == p.y
            && y != min_y
            && y != max_y
            && !prev.is_on_curve()
            && !next.is_on_curve())
        {
            continue;
        }
        let mut prev_on = prev;
        while !prev_on.is_on_curve() {
            prev_on = &points[prev_on.prev()];
        }
        let mut next_on = next;
        while !next_on.is_on_curve() {
            next_on = &points[next_on.next()];
        }
        let measurement = if next_on.y > p.y && prev_on.y > p.y {
            y.wrapping_sub(min_y)
        } else if next_on.y < p.y && prev_on.y < p.y {
            max_y.wrapping_sub(y)
        } else {
            continue;
        };
        if measurement < threshold {
            continue;
        }
        if depth.is_none_or(|d| measurement < d) {
            depth = Some(measurement);
        }
    }
    depth.unwrap_or(0)
}

/// `af_latin_stretch_top_tilde`: tilde contour `tilde` (its extrema `ext`'s) and what lies at or above its bottom
/// touched, the tilde stretched up from its bottom to a pixel taller than its wiggles' depth if it is shorter.
/// How much taller it got.
fn stretch_top_tilde(outline: &mut Outline, ext: &[(i64, i64)], tilde: usize) -> i64 {
    let (min_y, max_y) = ext[tilde];
    if min_y == max_y {
        return 0;
    }
    let height = max_y.wrapping_sub(min_y);
    let depth = wiggle_depth(outline, tilde, min_y, max_y);
    touch_top_contours(outline, ext, tilde);
    let target = depth + 64;
    if height >= target {
        return 0;
    }
    let range = outline.contours[tilde].range();
    for point in &mut outline.points[range] {
        point.y = ((point.y as i64).wrapping_sub(min_y).wrapping_mul(target) / height).wrapping_add(min_y) as i32;
    }
    target - height
}

/// `af_latin_stretch_bottom_tilde`: the same below a base, stretched down from the tilde's top, what lies at or
/// below it touched.
fn stretch_bottom_tilde(outline: &mut Outline, ext: &[(i64, i64)], tilde: usize) -> i64 {
    let (min_y, max_y) = ext[tilde];
    if min_y == max_y {
        return 0;
    }
    let height = max_y.wrapping_sub(min_y);
    let depth = wiggle_depth(outline, tilde, min_y, max_y);
    touch_bottom_contours(outline, ext, tilde);
    let target = depth + 64;
    if height >= target {
        return 0;
    }
    let range = outline.contours[tilde].range();
    for point in &mut outline.points[range] {
        point.y = ((point.y as i64).wrapping_sub(max_y).wrapping_mul(target) / height).wrapping_add(max_y) as i32;
    }
    target - height
}

/// The extrema of contour `contour` as its points are now.
fn contour_extrema(outline: &Outline, contour: usize) -> (i64, i64) {
    outline.points[outline.contours[contour].range()]
        .iter()
        .fold((i64::MAX, i64::MIN), |(lo, hi), p| (lo.min(p.y as i64), hi.max(p.y as i64)))
}

fn pix_round(v: i64) -> i64 {
    v.wrapping_add(32) & !63
}

/// `af_latin_align_top_tilde`: tilde contour `tilde` (stretched) moved so that its bottom is on the pixel grid, or
/// its center when it is less than three pixels tall. How far it moved.
fn align_top_tilde(outline: &mut Outline, tilde: usize) -> i64 {
    let (min_y, max_y) = contour_extrema(outline, tilde);
    let mut delta = pix_round(min_y).wrapping_sub(min_y);
    let height = max_y.wrapping_sub(min_y);
    if height < 64 * 3 {
        delta += (pix_round(height) - height) / 2;
    }
    move_contour(outline, tilde, delta);
    delta
}

/// `af_latin_align_bottom_tilde`: the same with its top on the pixel grid.
fn align_bottom_tilde(outline: &mut Outline, tilde: usize) -> i64 {
    let (min_y, max_y) = contour_extrema(outline, tilde);
    let mut delta = pix_round(max_y).wrapping_sub(max_y);
    let height = max_y.wrapping_sub(min_y);
    if height < 64 * 3 {
        delta -= (pix_round(height) - height) / 2;
    }
    move_contour(outline, tilde, delta);
    delta
}

/// The tildes of a glyph with `flags` stretched and aligned (`af_latin_hints_apply`, before the vertical axis is
/// analyzed): the inner ones first (below the topmost contour, above the bottommost), moving what lies beyond them
/// by as much, then the outer ones.
pub(super) fn fit_tildes(outline: &mut Outline, flags: u32) {
    if flags & (TILDE_TOP | TILDE_BOTTOM | TILDE_TOP2 | TILDE_BOTTOM2) == 0 {
        return;
    }
    let ext = vertical_extrema(outline);
    if flags & TILDE_TOP2 != 0 {
        let tilde = second_highest(&ext);
        let offset = stretch_top_tilde(outline, &ext, tilde) + align_top_tilde(outline, tilde);
        move_contours_up(outline, &ext, ext[tilde].0, offset);
    }
    if flags & TILDE_BOTTOM2 != 0 {
        let tilde = second_lowest(&ext);
        let offset = stretch_bottom_tilde(outline, &ext, tilde) - align_bottom_tilde(outline, tilde);
        move_contours_down(outline, &ext, ext[tilde].1, offset);
    }
    if flags & TILDE_TOP != 0 {
        let tilde = highest(&ext);
        stretch_top_tilde(outline, &ext, tilde);
        align_top_tilde(outline, tilde);
    }
    if flags & TILDE_BOTTOM != 0 {
        let tilde = lowest(&ext);
        stretch_bottom_tilde(outline, &ext, tilde);
        align_bottom_tilde(outline, tilde);
    }
}

/// `af_latin_get_base_glyph_blues`: the first active top and bottom blue zones of capitals (`capital`) or of small
/// letters (the x-height zone and the small bottom one).
fn base_glyph_blues(blues: &[ScaledBlue], capital: bool) -> (Option<&ScaledBlue>, Option<&ScaledBlue>) {
    let (top, bottom) = if capital {
        (BlueZones::TOP, BlueZones::CAPITAL_BOTTOM)
    } else {
        (BlueZones::ADJUSTMENT, BlueZones::SMALL_BOTTOM)
    };
    let find = |zones: BlueZones| blues.iter().find(|b| b.is_active && b.zones.contains(zones));
    (find(top), find(bottom))
}

/// How tall a diacritic may be and still be moved clear of its base (`af_latin_hints_apply`'s
/// `accent_height_limit`, 26.6): two thirds of the small letters' zones' height, else half the capitals', else four
/// tenths of the em.
pub(super) fn accent_height_limit(blues: &[ScaledBlue], units_per_em: i32, y_scale: i32) -> i64 {
    let shoot = |b: &ScaledBlue| b.overshoot.scaled as i64;
    if let (Some(top), Some(bottom)) = base_glyph_blues(blues, false) {
        return 2 * (shoot(top) - shoot(bottom)) / 3;
    }
    if let (Some(top), Some(bottom)) = base_glyph_blues(blues, true) {
        return (shoot(top) - shoot(bottom)) / 2;
    }
    fixed_mul(units_per_em * 4 / 10, y_scale) as i64
}

/// The blue zones ignored for a glyph with `flags` (`af_latin_ignore_top`, `af_latin_ignore_bottom`): the edges
/// above the top of its base's zones (capitals' or small letters'), by a seventh of the base's height and a quarter
/// of a pixel, (or below their bottom) kept off blue zones.
pub(super) fn ignore_blues(axis: &mut Axis, blues: &[ScaledBlue], flags: u32) {
    let mut ignore = |capital: bool, ignore_top: u32, ignore_bottom: u32| {
        let (Some(top), Some(bottom)) = base_glyph_blues(blues, capital) else {
            return;
        };
        let (top, bottom) = (top.overshoot.scaled, bottom.overshoot.scaled);
        let height = top - bottom;
        if flags & ignore_top != 0 {
            let limit = top + height / 7 + 16;
            for edge in axis.edges.iter_mut().filter(|e| e.pos > limit) {
                edge.flags |= TopoFlags::NO_BLUE;
            }
        }
        if flags & ignore_bottom != 0 {
            let limit = bottom - height / 7 - 16;
            for edge in axis.edges.iter_mut().filter(|e| e.pos < limit) {
                edge.flags |= TopoFlags::NO_BLUE;
            }
        }
    };
    ignore(true, IGNORE_CAPITAL_TOP, IGNORE_CAPITAL_BOTTOM);
    ignore(false, IGNORE_SMALL_TOP, IGNORE_SMALL_BOTTOM);
}

/// `af_check_contour_horizontal_overlap`: whether contour `index` overlaps across the box of the others (contours
/// of two points or fewer left out).
fn overlaps_horizontally(outline: &Outline, index: usize) -> bool {
    let (mut c_min, mut c_max, mut o_min, mut o_max) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for (i, contour) in outline.contours.iter().enumerate() {
        if contour.range().len() <= 2 {
            continue;
        }
        for p in &outline.points[contour.range()] {
            let x = p.x as i64;
            if i == index {
                c_min = c_min.min(x);
                c_max = c_max.max(x);
            } else {
                o_min = o_min.min(x);
                o_max = o_max.max(x);
            }
        }
    }
    (o_min <= c_max && c_max <= o_max) || (o_min <= c_min && c_min <= o_max) || (c_max >= o_max && c_min <= o_min)
}

/// `af_glyph_hints_apply_vertical_separation_adjustments` on the fitted outline of a glyph with `flags`: the
/// diacritic above (or the one below the topmost, with UP2) moved up so that a pixel separates its bottom from the
/// top of any contour below it, with whatever lies as high, a tilde re-centred on the pixel grid; the same
/// downwards; nothing for a diacritic that doesn't overlap the rest across, or is taller than `accent_height_limit`
/// (26.6).
pub(crate) fn separate(outline: &mut Outline, flags: u32, accent_height_limit: i64) {
    let (up, up2, down, down2) = (flags & UP != 0, flags & UP2 != 0, flags & DOWN != 0, flags & DOWN2 != 0);
    let n = outline.contours.len();
    if !(((up || down) && n >= 2) || ((up2 || down2) && n >= 3)) {
        return;
    }
    // Extrema again, of the fitted outline.
    let ext = vertical_extrema(outline);
    if (up && n >= 2) || (up2 && n >= 3) {
        let high = if up2 { second_highest(&ext) } else { highest(&ext) };
        if !overlaps_horizontally(outline, high) {
            return;
        }
        let (high_min, high_max) = ext[high];
        let high_height = high_max.wrapping_sub(high_min);
        if high_height > accent_height_limit {
            return;
        }
        let mut min_distance = 64i64;
        for (i, &(lo, hi)) in ext.iter().enumerate() {
            if i == high {
                continue;
            }
            let distance = high_min.wrapping_sub(hi);
            if distance < 64 && distance < min_distance && lo < high_min {
                min_distance = distance;
            }
        }
        let amount = 64 - min_distance;
        let (is_tilde, is_tilde2) = (flags & TILDE_TOP != 0, flags & TILDE_TOP2 != 0);
        let mut centering = 0i64;
        if is_tilde || is_tilde2 {
            let tilde = if up || is_tilde2 { high } else { highest(&ext) };
            let tilde_height = ext[tilde].1.wrapping_sub(ext[tilde].0);
            let mut pos = high_min.wrapping_add(amount);
            if up2 && is_tilde {
                pos += high_height;
            }
            if pos % 64 == 0 && tilde_height < 3 * 64 {
                centering = (pix_round(tilde_height) - tilde_height) / 2;
            }
        }
        let calculated = if (up && is_tilde) || (up2 && is_tilde2) { amount + centering } else { amount };
        // (A delta of 2/64 allowed for rounding.)
        if calculated != 0 && calculated >= -2 && (calculated <= 66 || amount <= 66) {
            let min_y_limit = high_min.wrapping_sub(high_height / 8);
            move_contours_up(outline, &ext, min_y_limit, amount);
            if up2 && is_tilde {
                move_contours_up(outline, &ext, min_y_limit.wrapping_add(high_height), centering);
            }
        }
    }
    if (down && n >= 2) || (down2 && n >= 3) {
        let low = if down2 { second_lowest(&ext) } else { lowest(&ext) };
        if !overlaps_horizontally(outline, low) {
            return;
        }
        let (low_min, low_max) = ext[low];
        let low_height = low_max.wrapping_sub(low_min);
        if low_height > accent_height_limit {
            return;
        }
        let mut min_distance = 64i64;
        for (i, &(lo, hi)) in ext.iter().enumerate() {
            if i == low {
                continue;
            }
            let distance = lo.wrapping_sub(low_max);
            if distance < 64 && distance < min_distance && hi > low_max {
                min_distance = distance;
            }
        }
        let amount = 64 - min_distance;
        let (is_tilde, is_tilde2) = (flags & TILDE_BOTTOM != 0, flags & TILDE_BOTTOM2 != 0);
        let mut centering = 0i64;
        if is_tilde || is_tilde2 {
            let tilde = if down || is_tilde2 { low } else { lowest(&ext) };
            let tilde_height = ext[tilde].1.wrapping_sub(ext[tilde].0);
            let mut pos = low_max.wrapping_sub(amount);
            if down2 && is_tilde {
                pos -= low_height;
            }
            if pos % 64 == 0 && tilde_height < 3 * 64 {
                centering = (pix_round(tilde_height) - tilde_height) / 2;
            }
        }
        let calculated = if (down && is_tilde) || (down2 && is_tilde2) { amount + centering } else { amount };
        if calculated != 0 && calculated >= -2 && (calculated <= 66 || amount <= 66) {
            let max_y_limit = low_max.wrapping_add(low_height / 8);
            move_contours_down(outline, &ext, max_y_limit, amount);
            if down2 && is_tilde {
                move_contours_down(outline, &ext, max_y_limit.wrapping_sub(low_height), centering);
            }
        }
    }
}
