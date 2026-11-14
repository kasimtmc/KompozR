//! Path data made by calls (Compose's PathBuilder and PathData).

use super::node::PathNode;

/// Path data made by calls (Compose's PathBuilder), each adding its node:
/// `PathData(|p| { p.move_to(5.0, 12.0).line_to_relative(5.0, 5.0).line_to_relative(9.0, -10.0); })`.
#[derive(Clone, Default, Debug)]
pub struct PathBuilder {
    nodes: Vec<PathNode>,
}

impl PathBuilder {
    pub fn new() -> PathBuilder {
        // (88% of Material icons take 32 nodes or fewer, as Compose's says)
        PathBuilder { nodes: Vec::with_capacity(32) }
    }

    pub fn nodes(&self) -> &[PathNode] {
        &self.nodes
    }

    fn add(&mut self, node: PathNode) -> &mut PathBuilder {
        self.nodes.push(node);
        self
    }

    pub fn close(&mut self) -> &mut PathBuilder {
        self.add(PathNode::Close)
    }
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut PathBuilder {
        self.add(PathNode::MoveTo { x, y })
    }
    pub fn move_to_relative(&mut self, dx: f32, dy: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeMoveTo { dx, dy })
    }
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut PathBuilder {
        self.add(PathNode::LineTo { x, y })
    }
    pub fn line_to_relative(&mut self, dx: f32, dy: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeLineTo { dx, dy })
    }
    pub fn horizontal_line_to(&mut self, x: f32) -> &mut PathBuilder {
        self.add(PathNode::HorizontalTo { x })
    }
    pub fn horizontal_line_to_relative(&mut self, dx: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeHorizontalTo { dx })
    }
    pub fn vertical_line_to(&mut self, y: f32) -> &mut PathBuilder {
        self.add(PathNode::VerticalTo { y })
    }
    pub fn vertical_line_to_relative(&mut self, dy: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeVerticalTo { dy })
    }
    pub fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> &mut PathBuilder {
        self.add(PathNode::CurveTo { x1, y1, x2, y2, x3, y3 })
    }
    pub fn curve_to_relative(
        &mut self,
        dx1: f32,
        dy1: f32,
        dx2: f32,
        dy2: f32,
        dx3: f32,
        dy3: f32,
    ) -> &mut PathBuilder {
        self.add(PathNode::RelativeCurveTo { dx1, dy1, dx2, dy2, dx3, dy3 })
    }
    pub fn reflective_curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut PathBuilder {
        self.add(PathNode::ReflectiveCurveTo { x1, y1, x2, y2 })
    }
    pub fn reflective_curve_to_relative(&mut self, dx1: f32, dy1: f32, dx2: f32, dy2: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeReflectiveCurveTo { dx1, dy1, dx2, dy2 })
    }
    pub fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut PathBuilder {
        self.add(PathNode::QuadTo { x1, y1, x2, y2 })
    }
    pub fn quad_to_relative(&mut self, dx1: f32, dy1: f32, dx2: f32, dy2: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeQuadTo { dx1, dy1, dx2, dy2 })
    }
    pub fn reflective_quad_to(&mut self, x1: f32, y1: f32) -> &mut PathBuilder {
        self.add(PathNode::ReflectiveQuadTo { x: x1, y: y1 })
    }
    pub fn reflective_quad_to_relative(&mut self, dx1: f32, dy1: f32) -> &mut PathBuilder {
        self.add(PathNode::RelativeReflectiveQuadTo { dx: dx1, dy: dy1 })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(
        &mut self,
        horizontal_ellipse_radius: f32,
        vertical_ellipse_radius: f32,
        theta: f32,
        is_more_than_half: bool,
        is_positive_arc: bool,
        x1: f32,
        y1: f32,
    ) -> &mut PathBuilder {
        self.add(PathNode::ArcTo {
            horizontal_ellipse_radius,
            vertical_ellipse_radius,
            theta,
            is_more_than_half,
            is_positive_arc,
            arc_start_x: x1,
            arc_start_y: y1,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to_relative(
        &mut self,
        a: f32,
        b: f32,
        theta: f32,
        is_more_than_half: bool,
        is_positive_arc: bool,
        dx1: f32,
        dy1: f32,
    ) -> &mut PathBuilder {
        self.add(PathNode::RelativeArcTo {
            horizontal_ellipse_radius: a,
            vertical_ellipse_radius: b,
            theta,
            is_more_than_half,
            is_positive_arc,
            arc_start_dx: dx1,
            arc_start_dy: dy1,
        })
    }
}

/// The nodes `block` adds to a [`PathBuilder`] (Compose's PathData).
pub fn PathData(block: impl FnOnce(&mut PathBuilder)) -> Vec<PathNode> {
    let mut builder = PathBuilder::new();
    block(&mut builder);
    builder.nodes
}
