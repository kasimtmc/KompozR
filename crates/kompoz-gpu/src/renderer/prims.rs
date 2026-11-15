//! The primitives drawn as one quad each, covered exactly by their distance in the fragment shader: rounded rects
//! filled (with a color or a gradient) and stroked, lines, arcs, and rounded rects' shadows (C++'s fillRoundRect,
//! strokeRoundRect, drawLine, drawArc, drawShadow, fillGradient).

use super::{Inputs, Renderer, paint::premultiplied};
use crate::shaders::{ARC, FILL, LINE, Prim, SHADOW, STROKE};
use kompoz_graphics::{Brush, Color, Offset, Rect, Shadow};
use kompoz_ui::graphics_context::CornerRadii;

impl Renderer {
    /// A rect in the target's px: x, y, w and h.
    fn px_rect(&self, rect: Rect) -> (f32, f32, f32, f32) {
        let s = self.scale;
        (rect.left * s - self.ox, rect.top * s - self.oy, rect.width() * s, rect.height() * s)
    }

    /// Each corner in the target's px, within half the rect's shorter side.
    fn px_radii(&self, radii: CornerRadii, half: f32) -> [f32; 4] {
        let px = |r: f32| (r * self.scale).clamp(0.0, half.max(0.0));
        [px(radii.top_left), px(radii.top_right), px(radii.bottom_right), px(radii.bottom_left)]
    }
}

pub(super) fn fill_round_rect(r: &mut Renderer, rect: Rect, radii: CornerRadii, color: Color) {
    if color.alpha() <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let (x, y, w, h) = r.px_rect(rect);
    let p = Prim {
        bounds: [x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0],
        shape: [x, y, x + w, y + h],
        radii: r.px_radii(radii, w.min(h) / 2.0),
        color: premultiplied(color),
        ..Prim::default()
    };
    r.add(FILL, &p, Inputs::default());
}

pub(super) fn stroke_round_rect(r: &mut Renderer, rect: Rect, radii: CornerRadii, width: f32, color: Color) {
    if color.alpha() <= 0.0 || width <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let (x, y, w, h) = r.px_rect(rect);
    let half = w.min(h) / 2.0;
    let p = Prim {
        bounds: [x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0],
        shape: [x, y, x + w, y + h],
        radii: r.px_radii(radii, half),
        params: [(width * r.scale).min(half), 0.0, 0.0, 0.0],
        color: premultiplied(color),
        ..Prim::default()
    };
    r.add(STROKE, &p, Inputs::default());
}

pub(super) fn draw_line(r: &mut Renderer, from: Offset, to: Offset, width: f32, color: Color) {
    let s = r.scale;
    let (x0, y0, x1, y1) = (from.x * s - r.ox, from.y * s - r.oy, to.x * s - r.ox, to.y * s - r.oy);
    if (x1 - x0).hypot(y1 - y0) < 1e-3 || color.alpha() <= 0.0 {
        return;
    }
    // Out to its square caps' corners (h past each end and to each side, turned with it), and a pixel.
    let h = width * s / 2.0;
    let pad = ((x1 - x0).abs() + (y1 - y0).abs()) / (x1 - x0).hypot(y1 - y0) * h + 1.0;
    let p = Prim {
        bounds: [x0.min(x1) - pad, y0.min(y1) - pad, x0.max(x1) + pad, y0.max(y1) + pad],
        shape: [x0, y0, x1, y1],
        params: [h, 0.0, 0.0, 0.0],
        color: premultiplied(color),
        ..Prim::default()
    };
    r.add(LINE, &p, Inputs::default());
}

pub(super) fn draw_arc(
    r: &mut Renderer,
    oval: Rect,
    start: f32,
    sweep: f32,
    width: f32,
    color: Color,
    round_cap: bool,
) {
    if color.alpha() <= 0.0 || width <= 0.0 || sweep == 0.0 {
        return;
    }
    let s = r.scale;
    let (cx, cy) = ((oval.left + oval.width() / 2.0) * s - r.ox, (oval.top + oval.height() / 2.0) * s - r.oy);
    let radius = oval.width().min(oval.height()) / 2.0 * s;
    let h = width * s / 2.0;
    let pad = radius + h + 1.0;
    let radians = std::f32::consts::PI / 180.0;
    let p = Prim {
        bounds: [cx - pad, cy - pad, cx + pad, cy + pad],
        shape: [cx, cy, radius, 0.0],
        params: [h, start * radians, sweep * radians, if round_cap { 1.0 } else { 0.0 }],
        color: premultiplied(color),
        ..Prim::default()
    };
    r.add(ARC, &p, Inputs::default());
}

/// Analytically: a SHADOW primitive, the rounded rect spread and moved, blurred by Skia's sigma for the radius
/// (erf across, four samples down: primitives.slang's blurredRoundRect); an inner one cut to the rect, where the
/// blurred hole isn't.
pub(super) fn draw_shadow(r: &mut Renderer, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
    if shadow.color.alpha() <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let s = r.scale;
    let blur = shadow.blur_radius.max(0.0) * s;
    let sigma = if blur > 0.0 { 0.57735 * blur + 0.5 } else { 0.0 }; // Skia's for the radius
    let grow = (if inner { -shadow.spread } else { shadow.spread }) * s;
    let (x, y, w, h) = r.px_rect(rect);
    let radii = radii.scaled(s);
    // The shadow's shape: the rect spread (in for an inner one's hole) and moved.
    let (sx, sy) = (x - grow + shadow.offset.x * s, y - grow + shadow.offset.y * s);
    let (mut sw, mut sh) = (w + 2.0 * grow, h + 2.0 * grow);
    if sw <= 0.0 || sh <= 0.0 {
        if !inner {
            return;
        }
        (sw, sh) = (0.0, 0.0); // no hole: the whole rect shadowed
    }
    let shaped = radii.grown(grow);
    let clamp_to = |v: f32, half_side: f32| v.clamp(0.0, half_side.max(0.0));
    let half_shape = sw.min(sh) / 2.0;
    let mut p = Prim {
        shape: [sx, sy, sx + sw, sy + sh],
        radii: [
            clamp_to(shaped.top_left, half_shape),
            clamp_to(shaped.top_right, half_shape),
            clamp_to(shaped.bottom_right, half_shape),
            clamp_to(shaped.bottom_left, half_shape),
        ],
        params: [sigma, if inner { 1.0 } else { 0.0 }, 0.0, 0.0],
        color: premultiplied(shadow.color),
        ..Prim::default()
    };
    if inner {
        p.bounds = [x, y, x + w, y + h];
        p.gradient = [x, y, x + w, y + h]; // (the rect the shader cuts it to: see SHADOW in primitives.slang)
        let half = w.min(h) / 2.0;
        p.uv = [
            clamp_to(radii.top_left, half),
            clamp_to(radii.top_right, half),
            clamp_to(radii.bottom_right, half),
            clamp_to(radii.bottom_left, half),
        ];
    } else {
        let pad = 3.0 * sigma + 1.0;
        p.bounds = [sx - pad, sy - pad, sx + sw + pad, sy + sh + pad];
    }
    r.add(SHADOW, &p, Inputs::default());
}

pub(super) fn fill_gradient(r: &mut Renderer, rect: Rect, radii: CornerRadii, brush: &Brush, area: Rect) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let (x, y, w, h) = r.px_rect(rect);
    let mut p = r.painted(brush, area, 1.0, 1.0);
    p.bounds = [x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0];
    p.shape = [x, y, x + w, y + h];
    p.radii = r.px_radii(radii, w.min(h) / 2.0);
    r.add(FILL, &p, Inputs::default());
}
