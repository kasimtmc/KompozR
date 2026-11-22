//! How a picture is told from its golden one, as the C++ Kompoz's `matchesGolden`: every pixel within 3 of the
//! golden one in every channel (GPUs, and two renderers, round a little differently). The GPU tests match their
//! scenes against tests/golden with it; the showcase (examples/showcase) its shots against the C++ showcase's.
//! Pictures are RGBA, 4 bytes a pixel, of one size.

/// How far a channel may be off: more is a difference.
pub const TOLERANCE: i32 = 3;

/// The largest difference of two pixels' channels.
pub fn max_difference(a: [u8; 4], b: [u8; 4]) -> i32 {
    a.iter().zip(b).map(|(&a, b)| (a as i32 - b as i32).abs()).max().unwrap_or(0)
}

/// How two pictures of a size differ: the largest channel difference, and how many pixels differ by more than
/// [`TOLERANCE`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Difference {
    pub worst: i32,
    pub count: usize,
}

/// How `a` differs from `b` (RGBA of one size), pixel by pixel; the pixels `skip` says to leave out (by their
/// index) aren't counted.
pub fn difference_except(a: &[u8], b: &[u8], skip: impl Fn(usize) -> bool) -> Difference {
    let mut d = Difference::default();
    for (i, (p, q)) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0).enumerate() {
        if skip(i) {
            continue;
        }
        let m = max_difference(*p, *q);
        d.worst = d.worst.max(m);
        d.count += (m > TOLERANCE) as usize;
    }
    d
}

/// How `a` differs from `b` (RGBA of one size).
pub fn difference(a: &[u8], b: &[u8]) -> Difference {
    difference_except(a, b, |_| false)
}

/// The golden picture faded, the pixels of `actual` off by more than [`TOLERANCE`] in red.
pub fn diff_picture(golden: &[u8], actual: &[u8]) -> Vec<u8> {
    let mut out = golden.to_vec();
    for (o, (g, a)) in
        out.as_chunks_mut::<4>().0.iter_mut().zip(golden.as_chunks::<4>().0.iter().zip(actual.as_chunks::<4>().0))
    {
        *o = if max_difference(*g, *a) > TOLERANCE {
            [255, 0, 0, 255]
        } else {
            [g[0] / 4 + 191, g[1] / 4 + 191, g[2] / 4 + 191, 255]
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_three_is_no_difference() {
        let a = [10, 20, 30, 255, 0, 0, 0, 255];
        assert_eq!(difference(&a, &[13, 17, 30, 255, 0, 0, 0, 255]), Difference { worst: 3, count: 0 });
        assert_eq!(difference(&a, &[14, 20, 30, 255, 0, 0, 9, 255]), Difference { worst: 9, count: 2 });
        assert_eq!(
            difference_except(&a, &[14, 20, 30, 255, 0, 0, 9, 255], |i| i == 1),
            Difference { worst: 4, count: 1 }
        );
        assert_eq!(diff_picture(&a, &[10, 20, 30, 255, 0, 0, 9, 255])[4..], [255, 0, 0, 255]);
    }
}
