//! Path data drawn into a path (Compose's `List<PathNode>.toPath`), its arcs as cubics (Compose's drawArc and
//! arcToBezier).

use super::node::PathNode;
use crate::path::Path;
use std::f64::consts::PI;

/// The path the nodes draw (Compose's `List<PathNode>.toPath`), a new one.
pub fn to_path(nodes: &[PathNode]) -> Path {
    let mut path = Path::new();
    to_path_into(nodes, &mut path);
    path
}

/// The path the nodes draw, into `target` (emptied, its fill type kept), as Compose's toPath(target): relative
/// nodes from the current point, reflective ones mirroring the last control point of a node of their kind (else
/// starting at the current point), H and V keeping the other coordinate, a close going back to its contour's start.
/// An arc becomes cubics of at most an eighth of a turn, as Compose's (its radii made bigger, as SVG has it, where
/// they can't reach its end); where a radius is 0, a line (as SVG has it; Compose's arithmetic would make the path
/// not finite).
pub fn to_path_into(nodes: &[PathNode], target: &mut Path) {
    let fill_type = target.fill_type;
    target.rewind();
    target.fill_type = fill_type;

    let (mut current_x, mut current_y) = (0.0f32, 0.0f32);
    let (mut ctrl_x, mut ctrl_y) = (0.0f32, 0.0f32);
    let (mut segment_x, mut segment_y) = (0.0f32, 0.0f32);
    let mut previous = nodes.first().copied().unwrap_or(PathNode::Close);
    for &node in nodes {
        match node {
            PathNode::Close => {
                current_x = segment_x;
                current_y = segment_y;
                ctrl_x = segment_x;
                ctrl_y = segment_y;
                target.close();
            }
            PathNode::RelativeMoveTo { dx, dy } => {
                current_x += dx;
                current_y += dy;
                target.relative_move_to(dx, dy);
                segment_x = current_x;
                segment_y = current_y;
            }
            PathNode::MoveTo { x, y } => {
                current_x = x;
                current_y = y;
                target.move_to(x, y);
                segment_x = current_x;
                segment_y = current_y;
            }
            PathNode::RelativeLineTo { dx, dy } => {
                target.relative_line_to(dx, dy);
                current_x += dx;
                current_y += dy;
            }
            PathNode::LineTo { x, y } => {
                target.line_to(x, y);
                current_x = x;
                current_y = y;
            }
            PathNode::RelativeHorizontalTo { dx } => {
                target.relative_line_to(dx, 0.0);
                current_x += dx;
            }
            PathNode::HorizontalTo { x } => {
                target.line_to(x, current_y);
                current_x = x;
            }
            PathNode::RelativeVerticalTo { dy } => {
                target.relative_line_to(0.0, dy);
                current_y += dy;
            }
            PathNode::VerticalTo { y } => {
                target.line_to(current_x, y);
                current_y = y;
            }
            PathNode::RelativeCurveTo { dx1, dy1, dx2, dy2, dx3, dy3 } => {
                target.relative_cubic_to(dx1, dy1, dx2, dy2, dx3, dy3);
                ctrl_x = current_x + dx2;
                ctrl_y = current_y + dy2;
                current_x += dx3;
                current_y += dy3;
            }
            PathNode::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                target.cubic_to(x1, y1, x2, y2, x3, y3);
                ctrl_x = x2;
                ctrl_y = y2;
                current_x = x3;
                current_y = y3;
            }
            PathNode::RelativeReflectiveCurveTo { dx1, dy1, dx2, dy2 } => {
                let (rx, ry) = if previous.is_curve() { (current_x - ctrl_x, current_y - ctrl_y) } else { (0.0, 0.0) };
                target.relative_cubic_to(rx, ry, dx1, dy1, dx2, dy2);
                ctrl_x = current_x + dx1;
                ctrl_y = current_y + dy1;
                current_x += dx2;
                current_y += dy2;
            }
            PathNode::ReflectiveCurveTo { x1, y1, x2, y2 } => {
                let (rx, ry) = if previous.is_curve() {
                    (2.0 * current_x - ctrl_x, 2.0 * current_y - ctrl_y)
                } else {
                    (current_x, current_y)
                };
                target.cubic_to(rx, ry, x1, y1, x2, y2);
                ctrl_x = x1;
                ctrl_y = y1;
                current_x = x2;
                current_y = y2;
            }
            PathNode::RelativeQuadTo { dx1, dy1, dx2, dy2 } => {
                target.relative_quadratic_to(dx1, dy1, dx2, dy2);
                ctrl_x = current_x + dx1;
                ctrl_y = current_y + dy1;
                current_x += dx2;
                current_y += dy2;
            }
            PathNode::QuadTo { x1, y1, x2, y2 } => {
                target.quadratic_to(x1, y1, x2, y2);
                ctrl_x = x1;
                ctrl_y = y1;
                current_x = x2;
                current_y = y2;
            }
            PathNode::RelativeReflectiveQuadTo { dx, dy } => {
                let (rx, ry) = if previous.is_quad() { (current_x - ctrl_x, current_y - ctrl_y) } else { (0.0, 0.0) };
                target.relative_quadratic_to(rx, ry, dx, dy);
                ctrl_x = current_x + rx;
                ctrl_y = current_y + ry;
                current_x += dx;
                current_y += dy;
            }
            PathNode::ReflectiveQuadTo { x, y } => {
                let (rx, ry) = if previous.is_quad() {
                    (2.0 * current_x - ctrl_x, 2.0 * current_y - ctrl_y)
                } else {
                    (current_x, current_y)
                };
                target.quadratic_to(rx, ry, x, y);
                ctrl_x = rx;
                ctrl_y = ry;
                current_x = x;
                current_y = y;
            }
            PathNode::RelativeArcTo {
                horizontal_ellipse_radius,
                vertical_ellipse_radius,
                theta,
                is_more_than_half,
                is_positive_arc,
                arc_start_dx,
                arc_start_dy,
            } => {
                let arc_start_x = arc_start_dx + current_x;
                let arc_start_y = arc_start_dy + current_y;
                draw_arc(
                    target,
                    current_x as f64,
                    current_y as f64,
                    arc_start_x as f64,
                    arc_start_y as f64,
                    horizontal_ellipse_radius as f64,
                    vertical_ellipse_radius as f64,
                    theta as f64,
                    is_more_than_half,
                    is_positive_arc,
                );
                current_x = arc_start_x;
                current_y = arc_start_y;
                ctrl_x = current_x;
                ctrl_y = current_y;
            }
            PathNode::ArcTo {
                horizontal_ellipse_radius,
                vertical_ellipse_radius,
                theta,
                is_more_than_half,
                is_positive_arc,
                arc_start_x,
                arc_start_y,
            } => {
                draw_arc(
                    target,
                    current_x as f64,
                    current_y as f64,
                    arc_start_x as f64,
                    arc_start_y as f64,
                    horizontal_ellipse_radius as f64,
                    vertical_ellipse_radius as f64,
                    theta as f64,
                    is_more_than_half,
                    is_positive_arc,
                );
                current_x = arc_start_x;
                current_y = arc_start_y;
                ctrl_x = current_x;
                ctrl_y = current_y;
            }
        }
        previous = node;
    }
}

/// Compose's drawArc: the SVG arc from (x0, y0) to (x1, y1), its radii a and b, turned by `theta` degrees, by its
/// center worked out in the unit circle's space.
#[allow(clippy::too_many_arguments)]
fn draw_arc(
    p: &mut Path,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    a: f64,
    b: f64,
    theta: f64,
    is_more_than_half: bool,
    is_positive_arc: bool,
) {
    if a == 0.0 || b == 0.0 {
        if x0 != x1 || y0 != y1 {
            p.line_to(x1 as f32, y1 as f32);
        }
        return;
    }
    // The rotation in radians, and (x0, y0) and (x1, y1) into unit space: turned back, then scaled back.
    let theta_d = theta / 180.0 * PI;
    let (cos_theta, sin_theta) = (theta_d.cos(), theta_d.sin());
    let x0p = (x0 * cos_theta + y0 * sin_theta) / a;
    let y0p = (-x0 * sin_theta + y0 * cos_theta) / b;
    let x1p = (x1 * cos_theta + y1 * sin_theta) / a;
    let y1p = (-x1 * sin_theta + y1 * cos_theta) / b;

    // Differences and averages; the unit circles through both points.
    let (dx, dy) = (x0p - x1p, y0p - y1p);
    let (xm, ym) = ((x0p + x1p) / 2.0, (y0p + y1p) / 2.0);
    let dsq = dx * dx + dy * dy;
    if dsq == 0.0 {
        return; // the same point
    }
    let disc = 1.0 / dsq - 1.0 / 4.0;
    if disc < 0.0 {
        // Too far apart for these radii: as small as reaches (Compose's: a Float).
        let adjust = ((dsq.sqrt() / 1.99999) as f32) as f64;
        draw_arc(p, x0, y0, x1, y1, a * adjust, b * adjust, theta, is_more_than_half, is_positive_arc);
        return;
    }
    let s = disc.sqrt();
    let (sdx, sdy) = (s * dx, s * dy);
    let (mut cx, mut cy) =
        if is_more_than_half == is_positive_arc { (xm - sdy, ym + sdx) } else { (xm + sdy, ym - sdx) };

    let eta0 = (y0p - cy).atan2(x0p - cx);
    let eta1 = (y1p - cy).atan2(x1p - cx);
    let mut sweep = eta1 - eta0;
    if is_positive_arc != (sweep >= 0.0) {
        if sweep > 0.0 {
            sweep -= 2.0 * PI;
        } else {
            sweep += 2.0 * PI;
        }
    }

    cx *= a;
    cy *= b;
    let tcx = cx;
    cx = cx * cos_theta - cy * sin_theta;
    cy = tcx * sin_theta + cy * cos_theta;

    arc_to_bezier(p, cx, cy, a, b, x0, y0, theta_d, eta0, sweep);
}

/// Compose's arcToBezier: the arc of the ellipse centered on (cx, cy), radii a and b, turned by `theta` radians,
/// from (e1x, e1y) at angle `start` through `sweep` (radians), as cubics of at most an eighth of a turn
/// (spaceroots.org's "Drawing an elliptical arc using polylines, quadratic or cubic Bézier curves").
#[allow(clippy::too_many_arguments)]
fn arc_to_bezier(
    p: &mut Path,
    cx: f64,
    cy: f64,
    a: f64,
    b: f64,
    e1x: f64,
    e1y: f64,
    theta: f64,
    start: f64,
    sweep: f64,
) {
    let (mut eta1x, mut eta1y) = (e1x, e1y);
    // (Kotlin's toInt and Rust's `as`: NaN 0.)
    let segments = (sweep * 4.0 / PI).abs().ceil() as i32;

    let mut eta1 = start;
    let (cos_theta, sin_theta) = (theta.cos(), theta.sin());
    let (cos_eta1, sin_eta1) = (eta1.cos(), eta1.sin());
    let mut ep1x = (-a * cos_theta * sin_eta1) - (b * sin_theta * cos_eta1);
    let mut ep1y = (-a * sin_theta * sin_eta1) + (b * cos_theta * cos_eta1);

    let angle_per_segment = sweep / segments as f64;
    for _ in 0..segments {
        let eta2 = eta1 + angle_per_segment;
        let (sin_eta2, cos_eta2) = (eta2.sin(), eta2.cos());
        let e2x = cx + (a * cos_theta * cos_eta2) - (b * sin_theta * sin_eta2);
        let e2y = cy + (a * sin_theta * cos_eta2) + (b * cos_theta * sin_eta2);
        let ep2x = (-a * cos_theta * sin_eta2) - (b * sin_theta * cos_eta2);
        let ep2y = (-a * sin_theta * sin_eta2) + (b * cos_theta * cos_eta2);
        let tan_diff2 = ((eta2 - eta1) / 2.0).tan();
        let alpha = (eta2 - eta1).sin() * ((4.0 + 3.0 * tan_diff2 * tan_diff2).sqrt() - 1.0) / 3.0;
        let (q1x, q1y) = (eta1x + alpha * ep1x, eta1y + alpha * ep1y);
        let (q2x, q2y) = (e2x - alpha * ep2x, e2y - alpha * ep2y);
        p.cubic_to(q1x as f32, q1y as f32, q2x as f32, q2y as f32, e2x as f32, e2y as f32);
        eta1 = eta2;
        eta1x = e2x;
        eta1y = e2y;
        ep1x = ep2x;
        ep1y = ep2y;
    }
}
