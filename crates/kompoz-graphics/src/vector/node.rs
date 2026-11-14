//! One command of a vector's path data (Compose's PathNode), and morphing one list of them into another.

/// One command of a vector's path data (Compose's PathNode), as SVG path data has them: moves, lines, cubics,
/// quadratics (their reflective ones mirror the last control point) and elliptical arcs, to a point or relative to
/// the current one. Its fields are Compose's properties.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathNode {
    /// `z` and `Z`: the contour closed, back to its start.
    Close,
    /// `m`.
    RelativeMoveTo { dx: f32, dy: f32 },
    /// `M`.
    MoveTo { x: f32, y: f32 },
    /// `l`.
    RelativeLineTo { dx: f32, dy: f32 },
    /// `L`.
    LineTo { x: f32, y: f32 },
    /// `h`.
    RelativeHorizontalTo { dx: f32 },
    /// `H`.
    HorizontalTo { x: f32 },
    /// `v`.
    RelativeVerticalTo { dy: f32 },
    /// `V`.
    VerticalTo { y: f32 },
    /// `c`: its two control points and end.
    RelativeCurveTo { dx1: f32, dy1: f32, dx2: f32, dy2: f32, dx3: f32, dy3: f32 },
    /// `C`.
    CurveTo { x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32 },
    /// `s`: its second control point and end (the first the last cubic's second mirrored).
    RelativeReflectiveCurveTo { dx1: f32, dy1: f32, dx2: f32, dy2: f32 },
    /// `S`.
    ReflectiveCurveTo { x1: f32, y1: f32, x2: f32, y2: f32 },
    /// `q`: its control point and end.
    RelativeQuadTo { dx1: f32, dy1: f32, dx2: f32, dy2: f32 },
    /// `Q`.
    QuadTo { x1: f32, y1: f32, x2: f32, y2: f32 },
    /// `t`: its end (its control point the last quadratic's mirrored).
    RelativeReflectiveQuadTo { dx: f32, dy: f32 },
    /// `T`.
    ReflectiveQuadTo { x: f32, y: f32 },
    /// `a`: the ellipse's radii, its turn in degrees, which of the four arcs, and where it ends.
    RelativeArcTo {
        horizontal_ellipse_radius: f32,
        vertical_ellipse_radius: f32,
        theta: f32,
        is_more_than_half: bool,
        is_positive_arc: bool,
        arc_start_dx: f32,
        arc_start_dy: f32,
    },
    /// `A`.
    ArcTo {
        horizontal_ellipse_radius: f32,
        vertical_ellipse_radius: f32,
        theta: f32,
        is_more_than_half: bool,
        is_positive_arc: bool,
        arc_start_x: f32,
        arc_start_y: f32,
    },
}

/// No path data (Compose's EmptyPath).
pub const EmptyPath: &[PathNode] = &[];

impl PathNode {
    /// A cubic (what a reflective cubic after it mirrors).
    pub fn is_curve(&self) -> bool {
        matches!(
            self,
            PathNode::RelativeCurveTo { .. }
                | PathNode::CurveTo { .. }
                | PathNode::RelativeReflectiveCurveTo { .. }
                | PathNode::ReflectiveCurveTo { .. }
        )
    }

    /// A quadratic (what a reflective quadratic after it mirrors).
    pub fn is_quad(&self) -> bool {
        matches!(
            self,
            PathNode::RelativeQuadTo { .. }
                | PathNode::QuadTo { .. }
                | PathNode::RelativeReflectiveQuadTo { .. }
                | PathNode::ReflectiveQuadTo { .. }
        )
    }

    /// Its kind and its numbers in the order Compose's constructor takes them (an arc's flags 0 or 1).
    fn parts(&self) -> (u8, [f32; 7]) {
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        match *self {
            PathNode::Close => (0, [0.0; 7]),
            PathNode::RelativeMoveTo { dx, dy } => (1, [dx, dy, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::MoveTo { x, y } => (2, [x, y, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::RelativeLineTo { dx, dy } => (3, [dx, dy, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::LineTo { x, y } => (4, [x, y, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::RelativeHorizontalTo { dx } => (5, [dx, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::HorizontalTo { x } => (6, [x, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::RelativeVerticalTo { dy } => (7, [dy, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::VerticalTo { y } => (8, [y, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::RelativeCurveTo { dx1, dy1, dx2, dy2, dx3, dy3 } => (9, [dx1, dy1, dx2, dy2, dx3, dy3, 0.0]),
            PathNode::CurveTo { x1, y1, x2, y2, x3, y3 } => (10, [x1, y1, x2, y2, x3, y3, 0.0]),
            PathNode::RelativeReflectiveCurveTo { dx1, dy1, dx2, dy2 } => (11, [dx1, dy1, dx2, dy2, 0.0, 0.0, 0.0]),
            PathNode::ReflectiveCurveTo { x1, y1, x2, y2 } => (12, [x1, y1, x2, y2, 0.0, 0.0, 0.0]),
            PathNode::RelativeQuadTo { dx1, dy1, dx2, dy2 } => (13, [dx1, dy1, dx2, dy2, 0.0, 0.0, 0.0]),
            PathNode::QuadTo { x1, y1, x2, y2 } => (14, [x1, y1, x2, y2, 0.0, 0.0, 0.0]),
            PathNode::RelativeReflectiveQuadTo { dx, dy } => (15, [dx, dy, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::ReflectiveQuadTo { x, y } => (16, [x, y, 0.0, 0.0, 0.0, 0.0, 0.0]),
            PathNode::RelativeArcTo {
                horizontal_ellipse_radius: a,
                vertical_ellipse_radius: b,
                theta,
                is_more_than_half,
                is_positive_arc,
                arc_start_dx,
                arc_start_dy,
            } => (17, [a, b, theta, flag(is_more_than_half), flag(is_positive_arc), arc_start_dx, arc_start_dy]),
            PathNode::ArcTo {
                horizontal_ellipse_radius: a,
                vertical_ellipse_radius: b,
                theta,
                is_more_than_half,
                is_positive_arc,
                arc_start_x,
                arc_start_y,
            } => (18, [a, b, theta, flag(is_more_than_half), flag(is_positive_arc), arc_start_x, arc_start_y]),
        }
    }

    /// The node of a kind from its numbers (an arc's flags set unless 0).
    fn from_parts(kind: u8, v: [f32; 7]) -> PathNode {
        match kind {
            1 => PathNode::RelativeMoveTo { dx: v[0], dy: v[1] },
            2 => PathNode::MoveTo { x: v[0], y: v[1] },
            3 => PathNode::RelativeLineTo { dx: v[0], dy: v[1] },
            4 => PathNode::LineTo { x: v[0], y: v[1] },
            5 => PathNode::RelativeHorizontalTo { dx: v[0] },
            6 => PathNode::HorizontalTo { x: v[0] },
            7 => PathNode::RelativeVerticalTo { dy: v[0] },
            8 => PathNode::VerticalTo { y: v[0] },
            9 => PathNode::RelativeCurveTo { dx1: v[0], dy1: v[1], dx2: v[2], dy2: v[3], dx3: v[4], dy3: v[5] },
            10 => PathNode::CurveTo { x1: v[0], y1: v[1], x2: v[2], y2: v[3], x3: v[4], y3: v[5] },
            11 => PathNode::RelativeReflectiveCurveTo { dx1: v[0], dy1: v[1], dx2: v[2], dy2: v[3] },
            12 => PathNode::ReflectiveCurveTo { x1: v[0], y1: v[1], x2: v[2], y2: v[3] },
            13 => PathNode::RelativeQuadTo { dx1: v[0], dy1: v[1], dx2: v[2], dy2: v[3] },
            14 => PathNode::QuadTo { x1: v[0], y1: v[1], x2: v[2], y2: v[3] },
            15 => PathNode::RelativeReflectiveQuadTo { dx: v[0], dy: v[1] },
            16 => PathNode::ReflectiveQuadTo { x: v[0], y: v[1] },
            17 => PathNode::RelativeArcTo {
                horizontal_ellipse_radius: v[0],
                vertical_ellipse_radius: v[1],
                theta: v[2],
                is_more_than_half: v[3] != 0.0,
                is_positive_arc: v[4] != 0.0,
                arc_start_dx: v[5],
                arc_start_dy: v[6],
            },
            18 => PathNode::ArcTo {
                horizontal_ellipse_radius: v[0],
                vertical_ellipse_radius: v[1],
                theta: v[2],
                is_more_than_half: v[3] != 0.0,
                is_positive_arc: v[4] != 0.0,
                arc_start_x: v[5],
                arc_start_y: v[6],
            },
            _ => PathNode::Close,
        }
    }
}

/// Whether one list of nodes morphs into the other: as many, each of the same kind as its counterpart.
pub fn can_morph(start: &[PathNode], stop: &[PathNode]) -> bool {
    start.len() == stop.len() && start.iter().zip(stop).all(|(a, b)| a.parts().0 == b.parts().0)
}

/// Path data `fraction` of the way from `start` to `stop`, as an animated vector's pathData animates: each number
/// in between, an arc's flag set while it is anywhere but 0 (as Android interpolates its flags as numbers and draws
/// them as set unless 0). Lists that can't morph ([`can_morph`]) go from one to the other halfway.
pub fn lerp_path_nodes(start: &[PathNode], stop: &[PathNode], fraction: f32) -> Vec<PathNode> {
    if !can_morph(start, stop) {
        return if fraction < 0.5 { start.to_vec() } else { stop.to_vec() };
    }
    start
        .iter()
        .zip(stop)
        .map(|(a, b)| {
            let ((kind, from), (_, to)) = (a.parts(), b.parts());
            PathNode::from_parts(kind, std::array::from_fn(|i| from[i] + (to[i] - from[i]) * fraction))
        })
        .collect()
}
