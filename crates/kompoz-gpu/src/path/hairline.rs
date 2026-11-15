//! A hairline not anti-aliased, as Skia draws one (C++'s aliasedHairline).

use kompoz_graphics::{Contour, Offset, StrokeCap};

/// The polygons that cover the pixels Skia's aliased hairline along `contours` takes: a pixel each step along each
/// segment's longer axis. Each segment is a band a pixel across that axis (half a pixel either side; its ends half a
/// pixel further for a square or round cap at an open contour's ends), all turned the same way (their winding adds
/// up where they meet), holding the middles of the pixels it takes. A contour of one point is a dot (with a cap).
pub fn aliased_hairline(contours: &[Contour], cap: StrokeCap) -> Vec<Contour> {
    let mut out = Vec::new();
    for c in contours {
        let n = c.points.len();
        if n == 0 {
            continue;
        }
        let segments = if c.closed { n } else { n - 1 };
        for i in 0..segments.max(1) {
            let mut a = c.points[i];
            let mut b = c.points[if n == 1 { 0 } else { (i + 1) % n }];
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = dx.hypot(dy);
            let across = dx.abs() >= dy.abs(); // (x the longer axis)
            if cap != StrokeCap::Butt && !c.closed {
                // The contour's ends, half a pixel on.
                let u = if len > 0.0 { Offset::new(dx / len, dy / len) } else { Offset::new(1.0, 0.0) };
                let ux = if !across {
                    0.0
                } else if u.x >= 0.0 {
                    0.5
                } else {
                    -0.5
                };
                let uy = if across {
                    0.0
                } else if u.y >= 0.0 {
                    0.5
                } else {
                    -0.5
                };
                if i == 0 {
                    a = Offset::new(a.x - ux, a.y - uy);
                }
                if i + 1 == segments || segments == 0 {
                    b = Offset::new(b.x + ux, b.y + uy);
                }
            } else if len == 0.0 {
                continue;
            }
            let o = if across { Offset::new(0.0, 0.5) } else { Offset::new(0.5, 0.0) };
            let mut band = vec![a - o, b - o, b + o, a + o];
            let twice: f32 = (0..4).map(|k| band[k].x * band[(k + 1) % 4].y - band[(k + 1) % 4].x * band[k].y).sum();
            if twice < 0.0 {
                band.reverse();
            }
            out.push(Contour { points: band, closed: true });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contour(points: &[(f32, f32)], closed: bool) -> Contour {
        Contour { points: points.iter().map(|&(x, y)| Offset::new(x, y)).collect(), closed }
    }

    /// Twice the signed area: positive for every band.
    fn twice_area(c: &Contour) -> f32 {
        let p = &c.points;
        (0..p.len()).map(|k| p[k].x * p[(k + 1) % p.len()].y - p[(k + 1) % p.len()].x * p[k].y).sum()
    }

    #[test]
    fn hairlines_take_a_pixel_across_their_longer_axis() {
        // Mostly across: half a pixel above and below; mostly down: half a pixel either side.
        let bands = aliased_hairline(&[contour(&[(1.0, 1.0), (9.0, 3.0), (10.0, 13.0)], false)], StrokeCap::Butt);
        assert_eq!(bands.len(), 2);
        assert_eq!(bands[0], contour(&[(1.0, 0.5), (9.0, 2.5), (9.0, 3.5), (1.0, 1.5)], true));
        assert_eq!(bands[1], contour(&[(9.5, 3.0), (10.5, 13.0), (9.5, 13.0), (8.5, 3.0)], true));
        assert!(bands.iter().all(|b| twice_area(b) > 0.0));
        // Square or round caps: the open contour's ends half a pixel on, along the longer axis (backwards too).
        let capped = aliased_hairline(&[contour(&[(9.0, 1.0), (1.0, 1.0)], false)], StrokeCap::Square);
        assert_eq!(capped[0].points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min), 0.5);
        assert_eq!(capped[0].points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max), 9.5);
        // A closed contour: a segment back to its start too, and no caps.
        let closed = aliased_hairline(&[contour(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)], true)], StrokeCap::Round);
        assert_eq!(closed.len(), 3);
        // A dot: a pixel with a cap, nothing without; a segment that goes nowhere, nothing.
        let dot = aliased_hairline(&[contour(&[(2.0, 2.0)], false)], StrokeCap::Round);
        assert_eq!(dot, [contour(&[(1.5, 1.5), (2.5, 1.5), (2.5, 2.5), (1.5, 2.5)], true)]);
        assert!(aliased_hairline(&[contour(&[(2.0, 2.0)], false)], StrokeCap::Butt).is_empty());
        assert!(aliased_hairline(&[contour(&[(2.0, 2.0), (2.0, 2.0)], false)], StrokeCap::Butt).is_empty());
        assert!(aliased_hairline(&[contour(&[], false)], StrokeCap::Round).is_empty());
    }
}
