//! Clips (C++'s pushClip, pushClipRoundRect, pushClipPath, clipRow, clearOfClips): an axis-aligned rect is the
//! scissor, its edges at the nearest pixel; a rect turned or in perspective, a rounded rect, what is outside one, and
//! a path are a chain of clip elements the fragment shader cuts each primitive's coverage by (clipCoverage), from a
//! row of the frame's table; a path's polygons go into the coverage mask, in the same generation as the primitives
//! drawn in it. No stencil.

use super::{Clip, ClipChain, ClipElement, Renderer, STOPS_WIDTH, intersect, pixels_of};
use crate::device::IRect;
use crate::path::PathPlacement;
use crate::shaders::Prim;
use kompoz_graphics::{Offset, Path, PathFillType, PathStroke, Rect};
use kompoz_ui::graphics_context::{ClipOp, CornerRadii};
use std::rc::Rc;

impl Renderer {
    /// Whether all of what `p` reaches lies where each clip of the chain covers every pixel whole (inside its edges
    /// and clear of its corners, as primitives.slang's roundRectCoverage takes a pixel): the chain changes nothing
    /// there, and isn't asked for (a card's content, clear of its rounded corners).
    pub(super) fn clear_of_clips(&self, p: &Prim, chain: usize) -> bool {
        if p.projection[0] != 0.0 || p.projection[1] != 0.0 || p.projection[2] != 1.0 {
            return false;
        }
        // Its quad on the target (the pixels whose middles it covers), and those pixels' boxes.
        let (mut l, mut t, mut r, mut b) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for c in 0..4 {
            let x = p.bounds[if c & 1 != 0 { 2 } else { 0 }];
            let y = p.bounds[if c & 2 != 0 { 3 } else { 1 }];
            let dx = p.xform[0] * x + p.xform[2] * y + p.extra[0];
            let dy = p.xform[1] * x + p.xform[3] * y + p.extra[1];
            (l, t, r, b) = (l.min(dx), t.min(dy), r.max(dx), b.max(dy));
        }
        (l, t, r, b) = (l - 0.5, t - 0.5, r + 0.5, b + 0.5);
        self.clip_chains[chain].elements.iter().all(|e| {
            if e.outside || e.polygons.is_some() || !e.to_local.is_identity() {
                return false;
            }
            let corner = e.radii[0].max(e.radii[1]).max(e.radii[2].max(e.radii[3]));
            let across = l >= e.rect[0] && r <= e.rect[2] && t >= e.rect[1] + corner && b <= e.rect[3] - corner;
            let down = l >= e.rect[0] + corner && r <= e.rect[2] - corner && t >= e.rect[1] && b <= e.rect[3];
            across || down
        })
    }

    /// The chain as a row of the table (six texels a clip: see clipCoverage), its paths in this generation's
    /// coverage mask.
    pub(super) fn clip_row(&mut self, chain: usize) -> i32 {
        let pass = self.pass_index();
        let c = &self.clip_chains[chain];
        if let Some(row) = c.row
            && (!c.paths || (c.generation == self.shelf.generation && c.pass == pass))
        {
            return row;
        }
        // Its paths in this generation's coverage mask, all of them (one that starts a new generation starts them
        // all over in it).
        let paths: Vec<(usize, Rc<Vec<kompoz_graphics::Contour>>, IRect)> =
            c.elements.iter().enumerate().filter_map(|(i, e)| e.polygons.clone().map(|p| (i, p, e.region))).collect();
        let mut origins = vec![Offset::Zero; c.elements.len()];
        for _ in 0..4 {
            let generation = self.shelf.generation;
            for (i, polygons, region) in &paths {
                origins[*i] = self.rasterize(polygons, *region, false).unwrap_or(Offset::Zero);
            }
            if self.shelf.generation == generation {
                break;
            }
        }
        let row = self.table_rows();
        let at = self.stops.len();
        self.stops.resize(at + STOPS_WIDTH * 4, 0.0);
        let c = &mut self.clip_chains[chain];
        c.generation = self.shelf.generation;
        c.pass = pass;
        c.row = Some(row);
        for (i, e) in c.elements.iter().take(STOPS_WIDTH / 6).enumerate() {
            let t = &mut self.stops[at + i * 24..at + i * 24 + 24];
            t[0] = if e.polygons.is_some() { 1.0 } else { 0.0 };
            t[1] = if e.outside { 1.0 } else { 0.0 };
            t[2] = if e.even_odd { 1.0 } else { 0.0 };
            if e.polygons.is_some() {
                t[4] = origins[i].x;
                t[5] = origins[i].y;
                let r = e.region;
                t[8..12].copy_from_slice(&[r.x as f32, r.y as f32, (r.x + r.width) as f32, (r.y + r.height) as f32]);
            } else {
                t[4..8].copy_from_slice(&e.rect);
                t[8..12].copy_from_slice(&e.radii);
                for r in 0..3 {
                    t[12 + r * 4..12 + r * 4 + 3].copy_from_slice(&e.to_local.m[r * 3..r * 3 + 3]);
                }
            }
        }
        row
    }

    /// A clip in the chain inside the current one: a new chain, the clips it leaves (whole pixels).
    fn push_chain(&mut self, element: ClipElement, scissor: IRect) {
        let mut chain = ClipChain::default();
        if let Some(outer) = self.clips.last().and_then(|c| c.chain) {
            chain.elements = self.clip_chains[outer].elements.clone();
            chain.paths = self.clip_chains[outer].paths;
        }
        chain.paths |= element.polygons.is_some();
        chain.elements.push(element);
        self.clip_chains.push(chain);
        let scissor = match self.clips.last() {
            Some(outer) => intersect(scissor, outer.scissor),
            None => scissor,
        };
        self.clips.push(Clip { scissor, chain: Some(self.clip_chains.len() - 1) });
    }
}

pub(super) fn push_clip(r: &mut Renderer, rect: Rect) {
    let s = r.scale;
    let (x0, y0, x1, y1) = (rect.left * s - r.ox, rect.top * s - r.oy, rect.right * s - r.ox, rect.bottom * s - r.oy);
    let mut clip = Clip { scissor: IRect::default(), chain: r.clips.last().and_then(|c| c.chain) };
    let on = r.mapped(x0, y0, x1, y1);
    match on {
        Some(on) if r.transform.axis_aligned() => {
            // Edges go to the nearest pixel (rounding outwards would show a row of what scrolled past).
            let (left, top) = (on.left.round() as i32, on.top.round() as i32);
            let (right, bottom) = (on.right.round() as i32, on.bottom.round() as i32);
            clip.scissor = IRect::new(left, top, (right - left).max(0), (bottom - top).max(0));
        }
        _ => {
            // Turned (or in perspective): the pixels it can reach, and the rect itself in the chain, its edges
            // anti-aliased.
            let mut chain = ClipChain::default();
            if let Some(outer) = clip.chain {
                chain.elements = r.clip_chains[outer].elements.clone();
                chain.paths = r.clip_chains[outer].paths; // (its paths too, as push_chain's: C++ left them out here)
            }
            chain.elements.push(ClipElement {
                rect: [x0, y0, x1, y1],
                to_local: r.transform.inverse(),
                ..ClipElement::default()
            });
            clip.chain = Some(r.clip_chains.len());
            r.clip_chains.push(chain);
            clip.scissor = on.map_or_else(|| r.scissor(), pixels_of);
        }
    }
    if let Some(outer) = r.clips.last() {
        clip.scissor = intersect(clip.scissor, outer.scissor);
    }
    r.clips.push(clip);
}

pub(super) fn push_clip_round_rect(r: &mut Renderer, rect: Rect, radii: CornerRadii, op: ClipOp) {
    if op == ClipOp::Intersect && radii.max() <= 0.0 {
        return push_clip(r, rect);
    }
    let s = r.scale;
    let (x0, y0, x1, y1) = (rect.left * s - r.ox, rect.top * s - r.oy, rect.right * s - r.ox, rect.bottom * s - r.oy);
    let half = ((x1 - x0).min(y1 - y0) / 2.0).max(0.0);
    let px = |v: f32| (v * s).clamp(0.0, half);
    let element = ClipElement {
        rect: [x0, y0, x1, y1],
        radii: [px(radii.top_left), px(radii.top_right), px(radii.bottom_right), px(radii.bottom_left)],
        to_local: r.transform.inverse(),
        outside: op == ClipOp::Difference,
        ..ClipElement::default()
    };
    // Inside: the pixels it reaches; outside: all the clips around it leave.
    let mut reach = r.scissor();
    if op == ClipOp::Intersect
        && let Some(on) = r.mapped(x0, y0, x1, y1)
    {
        reach = pixels_of(on);
    }
    r.push_chain(element, reach);
}

pub(super) fn push_clip_path(r: &mut Renderer, path: &Path, origin: Offset, op: ClipOp) {
    let place = PathPlacement::new(origin, r.scale, Offset::new(r.ox, r.oy));
    let polygons = crate::path::polygons_of(path, &place, &PathStroke::default(), &r.transform, false);
    let (mut l, mut t, mut rr, mut b) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
    for q in polygons.iter().flat_map(|c| &c.points) {
        (l, t, rr, b) = (l.min(q.x), t.min(q.y), rr.max(q.x), b.max(q.y));
    }
    let around = r.scissor();
    let region = if l > rr { IRect::default() } else { intersect(pixels_of(Rect::new(l, t, rr, b)), around) };
    let element = ClipElement {
        polygons: Some(Rc::new(polygons)),
        even_odd: path.fill_type == PathFillType::EvenOdd,
        region,
        outside: op == ClipOp::Difference,
        ..ClipElement::default()
    };
    r.push_chain(element, if op == ClipOp::Intersect { region } else { around });
}

pub(super) fn pop_clip(r: &mut Renderer) {
    r.clips.pop();
}
