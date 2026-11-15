//! Paths (C++'s drawPath, polygonsOf, rasterize, fillContours, drawKeptPath): a path's polygons in the target's
//! pixels (its stroke's outline, a hairline's, through the transform), their edges rasterized into the frame's
//! coverage mask (a generation of it, shelf by shelf; shaders/path.slang adds each edge's area and cover, as
//! FreeType's rasterizer does), then a PATH primitive reading the mask by the non-zero or even-odd rule. A path drawn
//! again as it was but whole pixels on takes the coverage it had, kept in a mask of its own.

use super::{Inputs, KEPT_MASK, KEPT_MOST, KeptPath, MASK_SIZE, Mask, Renderer, intersect};
use crate::device::{FNV1A_BASIS, Filter, Format, IRect, Sampler, TextureUsage, fnv1a_from};
use crate::path::{PathPlacement, append_edges};
use crate::shaders::{EdgeInstance, PATH, Prim};
use kompoz_graphics::{
    Brush, Contour, Offset, Path, PathCommand, PathFillType, PathStroke, Rect, StrokeCap, StrokeJoin, path_effected,
    res_scale_of,
};

/// The whole pixels the polygons touch; none if they have no points.
fn bounds_of(contours: &[Contour]) -> Option<IRect> {
    let (mut x0, mut y0, mut x1, mut y1) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
    for q in contours.iter().flat_map(|c| &c.points) {
        (x0, y0, x1, y1) = (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y));
    }
    (x0 <= x1).then(|| super::pixels_of(Rect::new(x0, y0, x1, y1)))
}

/// A path's commands as bytes (what a kept path is known by).
pub(super) fn command_bytes(path: &Path) -> Vec<u8> {
    let mut out = Vec::with_capacity(path.commands().len() * 28);
    let point = |out: &mut Vec<u8>, p: Offset| {
        out.extend_from_slice(&p.x.to_ne_bytes());
        out.extend_from_slice(&p.y.to_ne_bytes());
    };
    for c in path.commands() {
        match *c {
            PathCommand::Move(p) => {
                out.push(0);
                point(&mut out, p);
            }
            PathCommand::Line(p) => {
                out.push(1);
                point(&mut out, p);
            }
            PathCommand::Quadratic(c1, p) => {
                out.push(2);
                point(&mut out, c1);
                point(&mut out, p);
            }
            PathCommand::Conic(c1, p, w) => {
                out.push(3);
                point(&mut out, c1);
                point(&mut out, p);
                out.extend_from_slice(&w.to_ne_bytes());
            }
            PathCommand::Cubic(c1, c2, p) => {
                out.push(4);
                point(&mut out, c1);
                point(&mut out, c2);
                point(&mut out, p);
            }
            PathCommand::Close => out.push(5),
        }
    }
    out
}

fn cap_index(cap: StrokeCap) -> f32 {
    match cap {
        StrokeCap::Butt => 0.0,
        StrokeCap::Round => 1.0,
        StrokeCap::Square => 2.0,
    }
}

fn join_index(join: StrokeJoin) -> f32 {
    match join {
        StrokeJoin::Miter => 0.0,
        StrokeJoin::Round => 1.0,
        StrokeJoin::Bevel => 2.0,
    }
}

impl Renderer {
    /// A path's polygons in the target's pixels (its stroke's outline if `stroke` has a width), through the current
    /// transform.
    pub(super) fn polygons_of(&self, path: &Path, origin: Offset, stroke: &PathStroke) -> Vec<Contour> {
        let place = PathPlacement::new(origin, self.scale, Offset::new(self.ox, self.oy));
        crate::path::polygons_of(path, &place, stroke, &self.transform, self.aliased())
    }

    /// Rasterizes polygons into the current pass's coverage mask, within `region` (px of the target, or of a layer
    /// before its transform): where the mask's texel (0, 0) is in those pixels, or none if the region is empty.
    pub(super) fn rasterize(&mut self, contours: &[Contour], region: IRect, aliased: bool) -> Option<Offset> {
        let pass = self.pass_index()?;
        if region.is_empty() {
            return None;
        }
        let target = self.passes[pass].target;
        let shelf = &mut self.shelf;
        // As big as the target, or as a region bigger than it (a layer reaching past it).
        shelf.width = shelf.width.max(MASK_SIZE).max(target.width() + 2).max(region.width + 1);
        shelf.height = shelf.height.max(MASK_SIZE).max(target.height() + 2).max(region.height + 1);
        // A spot on the shelves (a texel apart), in this generation's mask or the next one's.
        let (w, h) = (region.width, region.height);
        if shelf.x + w + 1 > shelf.width {
            shelf.x = 0;
            shelf.y += shelf.row;
            shelf.row = 0;
        }
        if shelf.y + h + 1 > shelf.height {
            shelf.generation += 1;
            (shelf.x, shelf.y, shelf.row) = (0, 0, 0);
        }
        let (mx, my) = (shelf.x, shelf.y);
        shelf.x += w + 1;
        shelf.row = shelf.row.max(h + 1);
        let generation = shelf.generation;
        self.edge_scratch.clear();
        append_edges(contours, &mut self.edge_scratch);
        let (dx, dy) = ((mx - region.x) as f32, (my - region.y) as f32);
        let r = [mx as f32, my as f32, (mx + w) as f32, (my + h) as f32];
        let pass = &mut self.passes[pass];
        for e in &self.edge_scratch {
            pass.edges.push(EdgeInstance {
                edge: [e.x0 + dx, e.y0 + dy, e.x1 + dx, e.y1 + dy],
                region: r,
                mode: [if aliased { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
            });
            pass.edge_generations.push(generation);
        }
        Some(Offset::new((region.x - mx) as f32, (region.y - my) as f32))
    }

    /// rasterize() for a primitive about to be added: in the same mask generation as the paths of the clips it is
    /// drawn in.
    pub(super) fn rasterize_in_clips(&mut self, contours: &[Contour], region: IRect, aliased: bool) -> Option<Offset> {
        let chain = self.clips.last().and_then(|c| c.chain);
        let mut origin = None;
        for _ in 0..4 {
            if let Some(chain) = chain {
                self.clip_row(chain);
            }
            let generation = self.shelf.generation;
            origin = self.rasterize(contours, region, aliased);
            if self.shelf.generation == generation {
                break; // (else in a new one: the clips' paths go there too)
            }
        }
        origin
    }

    /// The polygons filled with `paint` by the non-zero or even-odd rule: rasterized where they are within the clip,
    /// and a PATH primitive over those pixels.
    pub(super) fn fill_contours(&mut self, contours: &[Contour], even_odd: bool, paint: &Prim) {
        if self.targets.is_empty() || (paint.paint[0] == 0.0 && paint.color[3] <= 0.0) {
            return;
        }
        // The pixels the polygons touch, within the clip.
        let Some(bounds) = bounds_of(contours) else { return };
        let region = intersect(bounds, self.scissor());
        let aliased = self.aliased();
        let Some(origin) = self.rasterize_in_clips(contours, region, aliased) else { return };
        let mut p = *paint;
        p.bounds =
            [region.x as f32, region.y as f32, (region.x + region.width) as f32, (region.y + region.height) as f32];
        p.params = [origin.x, origin.y, if even_odd { 1.0 } else { 0.0 }, 0.0];
        self.add(PATH, &p, Inputs { mask: Mask::Texture0, ..Inputs::default() });
    }

    /// draw_path through the kept paths: false where it isn't kept there (in perspective, or new to it with no room
    /// left), to be drawn as any path.
    fn draw_kept_path(
        &mut self,
        path: &Path,
        origin: Offset,
        stroke: &PathStroke,
        even_odd: bool,
        paint: &Prim,
    ) -> bool {
        if self.targets.is_empty() || (paint.paint[0] == 0.0 && paint.color[3] <= 0.0) {
            return true; // (as fill_contours: nothing to draw)
        }
        let m = self.transform.m;
        if m[6] != 0.0 || m[7] != 0.0 || m[8] != 1.0 {
            return false; // (in perspective: its coverage changes as it moves)
        }
        // Where its origin goes on the target, to a 64th of a pixel (it is drawn there: at most 1/128 px off, the
        // same coverage wherever it is): the whole pixels it moves by, and the rest, which with all else that shapes
        // its coverage makes its key. (A float's fraction alone differs with the whole part: 3.3 and 10.3's.)
        let s = self.scale;
        let (px, py) = (origin.x * s - self.ox, origin.y * s - self.oy);
        let cx = m[0] * px + m[1] * py + m[2];
        let cy = m[3] * px + m[4] * py + m[5];
        let (nx, ny) = ((cx * 64.0).round() / 64.0, (cy * 64.0).round() / 64.0);
        let (wx, wy) = (nx.floor(), ny.floor());
        let aliased = self.aliased();
        let shape = [
            s,
            m[0],
            m[1],
            m[3],
            m[4],
            nx - wx,
            ny - wy,
            stroke.width,
            stroke.miter,
            cap_index(stroke.cap),
            join_index(stroke.join),
            if stroke.hairline { 1.0 } else { 0.0 },
            if aliased { 1.0 } else { 0.0 },
        ];
        let shape_bytes: Vec<u8> = shape.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let key = fnv1a_from(fnv1a_from(FNV1A_BASIS, &command_bytes(path)), &shape_bytes);
        if self.kept_path_map.contains_key(&key) {
            self.kept_paths += 1;
        } else {
            if self.path_mask_full {
                return false;
            }
            let mut contours = self.polygons_of(path, origin, stroke);
            for q in contours.iter_mut().flat_map(|c| &mut c.points) {
                *q = Offset::new(q.x + (nx - cx), q.y + (ny - cy));
            }
            let Some(bounds) = bounds_of(&contours) else { return true }; // (nothing in it)
            // A spot of its own (a texel apart), never used before: the mask is clear there. (Too big for one, or
            // none left: drawn as any path, where it is.)
            let shelf = &mut self.path_shelf;
            (shelf.width, shelf.height) = (KEPT_MASK, KEPT_MASK);
            if shelf.x + bounds.width + 1 > shelf.width {
                (shelf.x, shelf.y, shelf.row) = (0, shelf.y + shelf.row, 0);
            }
            let big = bounds.width as i64 * bounds.height as i64 > KEPT_MOST as i64 * KEPT_MOST as i64
                || bounds.width > KEPT_MASK / 2
                || bounds.height > KEPT_MASK / 2;
            if big || shelf.y + bounds.height + 1 > shelf.height {
                if !big {
                    self.path_mask_full = true;
                }
                for q in contours.iter_mut().flat_map(|c| &mut c.points) {
                    *q = Offset::new(q.x - (nx - cx), q.y - (ny - cy));
                }
                self.fill_contours(&contours, even_odd, paint);
                return true;
            }
            let at = IRect::new(shelf.x, shelf.y, bounds.width, bounds.height);
            shelf.x += bounds.width + 1;
            shelf.row = shelf.row.max(bounds.height + 1);
            self.edge_scratch.clear();
            append_edges(&contours, &mut self.edge_scratch);
            let (dx, dy) = ((at.x - bounds.x) as f32, (at.y - bounds.y) as f32);
            let r = [at.x as f32, at.y as f32, (at.x + at.width) as f32, (at.y + at.height) as f32];
            for e in &self.edge_scratch {
                self.path_edges.push(EdgeInstance {
                    edge: [e.x0 + dx, e.y0 + dy, e.x1 + dx, e.y1 + dy],
                    region: r,
                    mode: [if aliased { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                });
            }
            let whole = IRect::new(bounds.x - wx as i32, bounds.y - wy as i32, bounds.width, bounds.height);
            self.kept_path_map.insert(key, KeptPath { at, bounds: whole, drawn: 0 });
        }
        let kept = self.kept_path_map.get_mut(&key).expect("kept just now");
        if kept.drawn != self.path_frame {
            kept.drawn = self.path_frame;
            self.paths_drawn_area += (kept.at.width + 1) as i64 * (kept.at.height + 1) as i64;
        }
        let kept = *kept;
        let bounds =
            IRect::new(kept.bounds.x + wx as i32, kept.bounds.y + wy as i32, kept.bounds.width, kept.bounds.height);
        let region = intersect(bounds, self.scissor());
        if region.is_empty() {
            return true;
        }
        if self.path_mask.is_none() {
            self.path_mask = self.device.create_texture(
                KEPT_MASK,
                KEPT_MASK,
                Format::R16f,
                TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET,
            );
        }
        let Some(mask) = self.path_mask.as_ref().map(|t| t.id()) else { return false };
        let mut p = *paint;
        p.bounds =
            [region.x as f32, region.y as f32, (region.x + region.width) as f32, (region.y + region.height) as f32];
        p.params =
            [(bounds.x - kept.at.x) as f32, (bounds.y - kept.at.y) as f32, if even_odd { 1.0 } else { 0.0 }, 0.0];
        let nearest = Sampler { filter: Filter::Nearest, ..Sampler::default() };
        self.add(PATH, &p, Inputs::texture(mask, nearest));
        true
    }
}

pub(super) fn draw_path(
    r: &mut Renderer,
    source: &Path,
    origin: Offset,
    brush: &Brush,
    area: Rect,
    given: &PathStroke,
) {
    if source.is_empty() {
        return;
    }
    // Its path effect at the pixels it is drawn at (Skia's resScale: the transform's larger scale).
    let mut stroke = given.clone();
    let m = r.transform.m;
    let path = path_effected(source, &mut stroke, r.scale * res_scale_of(m[0], m[3], m[1], m[4]));
    if path.is_empty() {
        return;
    }
    let even_odd = !stroke.strokes() && path.fill_type == PathFillType::EvenOdd;
    let mut paint = r.painted(brush, area, 1.0, 1.0);
    if !r.transform.is_identity() && paint.paint[0] != 0.0 {
        // A gradient: worked out where each pixel is in the path's space.
        let back = r.transform.inverse();
        paint.radii = [back.m[0], back.m[1], back.m[2], 0.0];
        paint.uv = [back.m[3], back.m[4], back.m[5], 0.0];
        paint.shape = [back.m[6], back.m[7], back.m[8], 0.0];
        paint.extra[3] = crate::shaders::FLAG_PAINT_MAPPED as f32;
    }
    r.path_count += 1;
    if r.draw_kept_path(&path, origin, &stroke, even_odd, &paint) {
        return;
    }
    let contours = r.polygons_of(&path, origin, &stroke);
    r.fill_contours(&contours, even_odd, &paint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_known_by_their_commands() {
        let mut a = Path::new();
        a.move_to(1.0, 2.0);
        a.cubic_to(3.0, 4.0, 5.0, 6.0, 7.0, 8.0);
        a.close();
        let mut b = a.clone();
        assert_eq!(command_bytes(&a), command_bytes(&b));
        b.line_to(1.0, 2.0);
        assert_ne!(command_bytes(&a), command_bytes(&b));
        assert_eq!(command_bytes(&a).len(), 1 + 8 + 1 + 24 + 1);
        let square = [Contour {
            points: vec![Offset::new(0.5, 1.0), Offset::new(3.0, 1.0), Offset::new(3.0, 2.5)],
            closed: true,
        }];
        assert_eq!(bounds_of(&square), Some(IRect::new(0, 1, 3, 2)));
        assert_eq!(bounds_of(&[]), None);
    }
}
