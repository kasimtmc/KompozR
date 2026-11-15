//! Outlines in font units, as points (skrifa's `outline/unscaled.rs`): what the blue zones are measured from.

use skrifa::raw::model::glyph::outline::UnscaledOutlineSink;
use skrifa::raw::tables::glyf::PointFlags;
use std::ops::Range;

/// The largest coordinate magnitude, in font units, that the autohinter takes.
///
/// Its analysis subtracts points and scales the differences in `i32`, which overflows near `1 << 27`. Only a
/// transformed component far outside the em reaches this bound.
const MAX_COORD: i32 = 1 << 24;

/// Clamps a coordinate in font units to the range the autohinter takes.
pub(crate) fn clamp_coord(value: i32) -> i32 {
    value.clamp(-MAX_COORD, MAX_COORD)
}

/// A point of an outline in font units, with a contour start flag.
#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct UnscaledPoint {
    pub x: i32,
    pub y: i32,
    pub flags: PointFlags,
    pub is_contour_start: bool,
}

impl UnscaledPoint {
    pub fn is_on_curve(self) -> bool {
        self.flags.is_on_curve()
    }
}

/// The points of an outline in font units, contour starts marked.
#[derive(Default)]
pub(crate) struct UnscaledOutlineBuf {
    points: Vec<UnscaledPoint>,
    /// True after a contour starts and before its first point is added.
    starting: bool,
}

impl UnscaledOutlineBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.starting = false;
    }

    pub fn as_ref(&self) -> UnscaledOutlineRef<'_> {
        UnscaledOutlineRef { points: self.points.as_slice() }
    }
}

impl UnscaledOutlineSink for UnscaledOutlineBuf {
    fn start_contour(&mut self) {
        self.starting = true;
    }

    fn point(&mut self, x: i32, y: i32, flags: PointFlags) {
        self.points.push(UnscaledPoint {
            x: clamp_coord(x),
            y: clamp_coord(y),
            flags,
            is_contour_start: self.starting,
        });
        self.starting = false;
    }

    fn end_contour(&mut self) {
        self.starting = false;
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct UnscaledOutlineRef<'a> {
    pub points: &'a [UnscaledPoint],
}

impl UnscaledOutlineRef<'_> {
    /// Returns the range of contour points and the index of the point within that contour for the last point
    /// where `f` returns true.
    ///
    /// This is common code used for finding extrema when materializing blue zones.
    ///
    /// For example: <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L509>
    pub fn find_last_contour(&self, mut f: impl FnMut(&UnscaledPoint) -> bool) -> Option<(Range<usize>, usize)> {
        if self.points.is_empty() {
            return None;
        }
        let mut best_contour = 0..0;
        // Index of the best point relative to the start of the best contour
        let mut best_point = 0;
        let mut cur_contour = 0..0;
        let mut found_best_in_cur_contour = false;
        for (point_ix, point) in self.points.iter().enumerate() {
            if point.is_contour_start {
                if found_best_in_cur_contour {
                    best_contour = cur_contour;
                }
                cur_contour = point_ix..point_ix;
                found_best_in_cur_contour = false;
                // Ignore single point contours
                match self.points.get(point_ix + 1) {
                    Some(next_point) if next_point.is_contour_start => continue,
                    None => continue,
                    _ => {}
                }
            }
            cur_contour.end += 1;
            if f(point) {
                best_point = point_ix - cur_contour.start;
                found_best_in_cur_contour = true;
            }
        }
        if found_best_in_cur_contour {
            best_contour = cur_contour;
        }
        if !best_contour.is_empty() { Some((best_contour, best_point)) } else { None }
    }
}
