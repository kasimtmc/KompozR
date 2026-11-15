//! The edges of polygons, what the coverage pass draws (C++'s appendEdges).

use kompoz_graphics::Contour;

/// An edge of a polygon from (x0, y0) to (x1, y1), in the target's pixels; never horizontal.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Edge {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Appends the edges of `contours` to `out`, every contour closed (as a fill closes it), the horizontal ones left
/// out (they cross no row: they add nothing to any pixel's coverage); a contour of fewer than two points has none.
pub fn append_edges(contours: &[Contour], out: &mut Vec<Edge>) {
    for c in contours {
        let n = c.points.len();
        if n < 2 {
            continue;
        }
        for i in 0..n {
            let (a, b) = (c.points[i], c.points[(i + 1) % n]);
            if a.y != b.y {
                out.push(Edge { x0: a.x, y0: a.y, x1: b.x, y1: b.y });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::Offset;

    #[test]
    fn edges_close_every_contour_and_leave_out_horizontal_ones() {
        let triangle = Contour {
            points: vec![Offset::new(0.0, 0.0), Offset::new(4.0, 0.0), Offset::new(2.0, 3.0)],
            closed: false, // (closed all the same)
        };
        let dot = Contour { points: vec![Offset::new(1.0, 1.0)], closed: true };
        let mut out = vec![Edge { x0: 9.0, y0: 9.0, x1: 9.0, y1: 8.0 }];
        append_edges(&[triangle, dot], &mut out);
        assert_eq!(
            out,
            [
                Edge { x0: 9.0, y0: 9.0, x1: 9.0, y1: 8.0 },
                Edge { x0: 4.0, y0: 0.0, x1: 2.0, y1: 3.0 },
                Edge { x0: 2.0, y0: 3.0, x1: 0.0, y1: 0.0 },
            ]
        );
    }
}
