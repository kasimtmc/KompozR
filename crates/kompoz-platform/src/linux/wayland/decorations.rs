//! KompozR's own decorations around the window (C++'s wayland_platform.cpp, "Kompoz's decorations"), where the
//! compositor leaves the window undecorated (no xdg-decoration, or `KOMPOZ_DECORATIONS=kompoz`): besides the title
//! bar the app core draws, four edges around the window, each a subsurface of the window's below it and outside its
//! geometry, showing a shadow (black fading out from the window over [`EDGE_SIZE`] points, stronger while the window
//! is the active one) drawn into a `wl_shm` buffer of its own (a memfd). The pointer or a finger pressed on an edge
//! resizes the window by it (`xdg_toplevel.resize`: a side, or a corner within [`CORNER_SIZE`] points of one), the
//! pointer over it showing the resize cursor for where it would resize from (by shape, or from the cursor theme).
//! While the window is maximized or fullscreen there are none, nor on the sides it is tiled on. The window's geometry
//! is its surface, without them.

use super::WaylandPlatform;
use super::ffi::{Object, ObjectId};
use super::generated::*;
use super::window::WindowState;
use crate::linux::sys::{self, Mmap};
use std::fs::File;
use std::os::fd::AsFd;

/// How far the edges reach beyond the window, points (C++'s kEdgeSize).
pub(super) const EDGE_SIZE: i32 = 12;
/// How far along each side from a corner the corner's resizing reaches (both ways), points (C++'s kCornerSize).
pub(super) const CORNER_SIZE: i32 = 16;

/// The edges, in C++'s order (that of [`WindowState::tiled`]).
pub(super) const TOP: usize = 0;
pub(super) const BOTTOM: usize = 1;
pub(super) const LEFT: usize = 2;
pub(super) const RIGHT: usize = 3;

/// Which edges are shown: around the window while it floats (not maximized or fullscreen, nor on its tiled sides)
/// and KompozR decorates it, else none.
pub(super) fn shown_edges(own_decorations: bool, state: &WindowState) -> [bool; 4] {
    let floating = own_decorations && !state.maximized && !state.fullscreen;
    std::array::from_fn(|i| floating && !state.tiled[i])
}

/// Where each edge is, `[x, y, width, height]` in points from the window's top left, for a window of `width` by
/// `height` points: above and below it across its width and the side edges shown, beside it along its height.
pub(super) fn edge_places(width: i32, height: i32, shown: [bool; 4]) -> [[i32; 4]; 4] {
    let m = EDGE_SIZE;
    let left = if shown[LEFT] { m } else { 0 };
    let across = left + width + if shown[RIGHT] { m } else { 0 };
    let mut places = [[0; 4]; 4];
    places[TOP] = [-left, -m, across, m];
    places[BOTTOM] = [-left, height, across, m];
    places[LEFT] = [-m, 0, m, height];
    places[RIGHT] = [width, 0, m, height];
    places
}

/// Where the window is resized from at (x, y), a point of an edge (in the window's points, outside it): a side, or a
/// corner within [`CORNER_SIZE`] of it (C++'s resizeEdgeAt).
pub(super) fn resize_edge_at(x: f64, y: f64, width: i32, height: i32) -> XdgToplevelResizeEdge {
    let (width, height, corner) = (width as f64, height as f64, CORNER_SIZE as f64);
    let above_or_below = y < 0.0 || y >= height;
    let beside = x < 0.0 || x >= width;
    let mut edge = 0;
    if y < if beside { corner } else { 0.0 } {
        edge |= XdgToplevelResizeEdge::TOP.0;
    } else if y >= height - if beside { corner } else { 0.0 } {
        edge |= XdgToplevelResizeEdge::BOTTOM.0;
    }
    if x < if above_or_below { corner } else { 0.0 } {
        edge |= XdgToplevelResizeEdge::LEFT.0;
    } else if x >= width - if above_or_below { corner } else { 0.0 } {
        edge |= XdgToplevelResizeEdge::RIGHT.0;
    }
    XdgToplevelResizeEdge(edge)
}

/// The resize cursor's shape for where the window would be resized from (C++'s overEdge's table).
pub(super) fn resize_shape(edge: XdgToplevelResizeEdge) -> WpCursorShapeDeviceV1Shape {
    type Edge = XdgToplevelResizeEdge;
    type Shape = WpCursorShapeDeviceV1Shape;
    match edge {
        Edge::TOP => Shape::N_RESIZE,
        Edge::BOTTOM => Shape::S_RESIZE,
        Edge::LEFT => Shape::W_RESIZE,
        Edge::RIGHT => Shape::E_RESIZE,
        Edge::TOP_LEFT => Shape::NW_RESIZE,
        Edge::TOP_RIGHT => Shape::NE_RESIZE,
        Edge::BOTTOM_LEFT => Shape::SW_RESIZE,
        Edge::BOTTOM_RIGHT => Shape::SE_RESIZE,
        _ => Shape::DEFAULT,
    }
}

/// The names a cursor theme may have the resize cursor under (the CSS names first, then X11's older ones), for the
/// theme's cursors where the compositor has no cursor-shape-v1.
pub(super) fn resize_theme_names(edge: XdgToplevelResizeEdge) -> &'static [&'static str] {
    type Edge = XdgToplevelResizeEdge;
    match edge {
        Edge::TOP => &["n-resize", "top_side"],
        Edge::BOTTOM => &["s-resize", "bottom_side"],
        Edge::LEFT => &["w-resize", "left_side"],
        Edge::RIGHT => &["e-resize", "right_side"],
        Edge::TOP_LEFT => &["nw-resize", "top_left_corner"],
        Edge::TOP_RIGHT => &["ne-resize", "top_right_corner"],
        Edge::BOTTOM_LEFT => &["sw-resize", "bottom_left_corner"],
        Edge::BOTTOM_RIGHT => &["se-resize", "bottom_right_corner"],
        _ => &["default", "left_ptr"],
    }
}

/// The shadow's alpha at (x, y) (points from the window's top left; outside it) around a window of `width` by
/// `height` points (C++'s drawEdge): black fading out over [`EDGE_SIZE`] from the window, squared, 0.32 strong while
/// the window is the active one and 0.16 while it isn't.
pub(super) fn shadow_alpha(x: f64, y: f64, width: i32, height: i32, activated: bool) -> u8 {
    let strength = if activated { 0.32 } else { 0.16 };
    let dx = (-x).max(x - width as f64).max(0.0);
    let dy = (-y).max(y - height as f64).max(0.0);
    let t = (dx.hypot(dy) / EDGE_SIZE as f64).min(1.0);
    (255.0 * strength * (1.0 - t) * (1.0 - t)).round() as u8
}

/// An edge's shadow at `place` (points) as `scale` buffer pixels a point: premultiplied ARGB words, row by row, each
/// pixel's shadow at its center (a pixel a point at scale 1, as C++'s: a blur scaled up looks the same; more on a
/// screen of a higher scale).
pub(super) fn shadow_pixels(place: [i32; 4], width: i32, height: i32, activated: bool, scale: i32) -> Vec<u32> {
    let [x, y, w, h] = place;
    let n = scale.max(1);
    let (pw, ph) = (w * n, h * n);
    let mut pixels = Vec::with_capacity((pw.max(0) * ph.max(0)) as usize);
    for j in 0..ph {
        for i in 0..pw {
            let px = x as f64 + (i as f64 + 0.5) / n as f64;
            let py = y as f64 + (j as f64 + 0.5) / n as f64;
            pixels.push(u32::from(shadow_alpha(px, py, width, height, activated)) << 24);
        }
    }
    pixels
}

/// A buffer an edge was drawn into: let go once it is replaced and the compositor no longer reads it.
struct ShmBuffer {
    buffer: WlBuffer,
    busy: bool,
    retired: bool,
}

/// An edge: its surface and subsurface (made when it is first shown; the subsurface first, dropped before its
/// surface), the buffer it shows, where it is and what its shadow was drawn for.
#[derive(Default)]
struct Edge {
    subsurface: Option<WlSubsurface>,
    surface: Option<WlSurface>,
    buffer: Option<ObjectId>,
    place: [i32; 4],
    scale: i32,
    activated: bool,
    shown: bool,
}

/// The window's edges and the buffers drawn for them (the window's own, dropped with it before its surface).
#[derive(Default)]
pub(super) struct Edges {
    edges: [Edge; 4],
    buffers: Vec<ShmBuffer>,
}

impl Edges {
    /// Which edge `surface` is.
    pub fn edge_of(&self, surface: ObjectId) -> Option<usize> {
        if surface == ObjectId::NULL {
            return None;
        }
        self.edges.iter().position(|e| e.surface.as_ref().is_some_and(|s| s.id() == surface))
    }

    /// Where edge `i` is (points from the window's top left).
    pub fn place(&self, i: usize) -> [i32; 4] {
        self.edges[i].place
    }

    /// A buffer the compositor is done reading (its `release`).
    pub fn released(&mut self, buffer: ObjectId) -> bool {
        let Some(b) = self.buffers.iter_mut().find(|b| b.buffer.id() == buffer) else { return false };
        b.busy = false;
        true
    }

    fn retire(&mut self, buffer: Option<ObjectId>) {
        if let Some(b) = buffer.and_then(|id| self.buffers.iter_mut().find(|b| b.buffer.id() == id)) {
            b.retired = true;
        }
    }
}

/// A buffer of `pixels` (premultiplied ARGB words, row by row) in shared memory (a memfd named `name`), the
/// compositor's buffer over it: what an edge's shadow and a drag's icon are shown from.
pub(super) fn shm_buffer(shm: &WlShm, name: &str, pixels: &[u32], width: i32, height: i32) -> Option<WlBuffer> {
    let stride = width.checked_mul(4)?;
    let size = usize::try_from(stride).ok()?.checked_mul(usize::try_from(height).ok()?)?;
    let fd = sys::memfd(name).ok()?;
    let file = File::from(fd);
    file.set_len(size as u64).ok()?;
    {
        let mut map = Mmap::shared(file.as_fd(), size, true).ok()?;
        for (word, p) in map.as_mut_slice().as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
            *word = p.to_le_bytes(); // (ARGB8888 is little endian)
        }
    }
    let pool = shm.create_pool(file.as_fd(), i32::try_from(size).ok()?)?;
    pool.create_buffer(0, width, height, stride, WlShmFormat::ARGB8888)
    // (The pool goes now, the fd with it: the buffer keeps the memory.)
}

impl WaylandPlatform {
    /// The edges as the window is now (C++'s updateEdges): shown around it while it floats and KompozR decorates
    /// it, each placed and drawn again where its place, the window's activation or its scale changed, hidden where
    /// it isn't shown; buffers the compositor is done with let go; the window's geometry its surface.
    pub(super) fn update_edges(&mut self) {
        let surface_scale = self.scale().ceil().max(1.0) as i32;
        let shown = shown_edges(self.win.own_decorations, &self.win.state);
        let places = edge_places(self.win.width, self.win.height, shown);
        let (width, height, activated) = (self.win.width, self.win.height, self.win.state.activated);
        let g = &self.globals;
        let Some(w) = &mut self.win.window else { return };
        w.edges.buffers.retain(|b| !b.retired || b.busy); // (a buffer dropped is destroyed)
        for (i, place) in places.into_iter().enumerate() {
            let edges = &mut w.edges;
            if !shown[i] {
                let e = &mut edges.edges[i];
                if e.shown {
                    if let Some(s) = &e.surface {
                        s.attach(None, 0, 0);
                        s.commit();
                    }
                    e.shown = false;
                    let old = e.buffer.take();
                    edges.retire(old);
                }
                continue;
            }
            let e = &mut edges.edges[i];
            if e.surface.is_none() {
                let (Some(compositor), Some(subcompositor)) = (&g.compositor, &g.subcompositor) else { continue };
                let Some(surface) = compositor.create_surface() else { continue };
                let Some(subsurface) = subcompositor.get_subsurface(&surface, &w.surface) else { continue };
                subsurface.place_below(&w.surface);
                e.surface = Some(surface);
                e.subsurface = Some(subsurface);
            }
            let (Some(surface), Some(subsurface)) = (&e.surface, &e.subsurface) else { continue };
            subsurface.set_position(place[0], place[1]);
            // (A buffer scale needs wl_surface 3.)
            let scale = if surface.version() >= 3 { surface_scale } else { 1 };
            if e.shown && e.place == place && e.activated == activated && e.scale == scale {
                continue;
            }
            let Some(shm) = &g.shm else { continue };
            let pixels = shadow_pixels(place, width, height, activated, scale);
            let Some(buffer) = shm_buffer(shm, "kompoz-edge", &pixels, place[2] * scale, place[3] * scale) else {
                crate::system::log("kompoz: Wayland: no memory for the window's edges");
                continue;
            };
            surface.attach(Some(&buffer), 0, 0);
            if scale > 1 {
                surface.set_buffer_scale(scale);
            } else if surface.version() >= 3 {
                surface.set_buffer_scale(1);
            }
            surface.damage(0, 0, place[2], place[3]);
            // (Applied with the window's next commit: a subsurface's state waits for its parent's.)
            surface.commit();
            (e.place, e.activated, e.scale, e.shown) = (place, activated, scale, true);
            let old = e.buffer.replace(buffer.id());
            edges.retire(old);
            edges.buffers.push(ShmBuffer { buffer, busy: true, retired: false });
        }
        w.xdg_surface.set_window_geometry(0, 0, width, height);
    }

    /// Which edge of the window `surface` is.
    pub(super) fn edge_of(&self, surface: ObjectId) -> Option<usize> {
        self.win.window.as_ref()?.edges.edge_of(surface)
    }

    /// The compositor is done with a buffer (an edge's).
    pub(super) fn buffer_released(&mut self, buffer: ObjectId) {
        if let Some(w) = &mut self.win.window {
            w.edges.released(buffer);
        }
    }

    /// The pointer over edge `i` at (x, y) of it (entered with `serial`, or moved): where it is in the window's points
    /// kept for a press, the resize cursor for where it would resize from shown.
    pub(super) fn over_edge(&mut self, i: usize, x: f64, y: f64, entered: Option<u32>) {
        let Some(w) = &self.win.window else { return };
        let [ex, ey, ..] = w.edges.place(i);
        self.win.edge_x = ex as f64 + x;
        self.win.edge_y = ey as f64 + y;
        let edge = resize_edge_at(self.win.edge_x, self.win.edge_y, self.win.width, self.win.height);
        let mut cursor = self.shared.cursor.borrow_mut();
        match entered {
            Some(serial) => cursor.entered_edge(serial, edge),
            None => cursor.over_edge(edge),
        }
    }

    /// A press on an edge (the pointer's left button, or a finger, with `serial`) at (x, y) in the window's points:
    /// the window manager resizes the window by it.
    pub(super) fn resize_from_edge(&self, serial: u32, x: f64, y: f64) {
        let (Some(w), Some(seat)) = (&self.win.window, &self.globals.seat) else { return };
        w.toplevel.resize(seat, serial, resize_edge_at(x, y, self.win.width, self.win.height));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Edge = XdgToplevelResizeEdge;

    #[test]
    fn edges_show_while_the_window_floats() {
        let floating = WindowState::default();
        assert_eq!(shown_edges(true, &floating), [true; 4]);
        assert_eq!(shown_edges(false, &floating), [false; 4]); // (the compositor's decorations)
        assert_eq!(shown_edges(true, &WindowState { maximized: true, ..floating }), [false; 4]);
        assert_eq!(shown_edges(true, &WindowState { fullscreen: true, ..floating }), [false; 4]);
        let tiled = WindowState { tiled: [false, false, true, false], ..floating };
        assert_eq!(shown_edges(true, &tiled), [true, true, false, true]);
    }

    #[test]
    fn edges_are_around_the_window() {
        // A 300 by 200 window: above and below across it and the side edges, beside it along it (C++'s places).
        let all = edge_places(300, 200, [true; 4]);
        assert_eq!(all[TOP], [-12, -12, 324, 12]);
        assert_eq!(all[BOTTOM], [-12, 200, 324, 12]);
        assert_eq!(all[LEFT], [-12, 0, 12, 200]);
        assert_eq!(all[RIGHT], [300, 0, 12, 200]);
        // Tiled on the left: the top and bottom from the window's left side.
        let tiled = edge_places(300, 200, [true, true, false, true]);
        assert_eq!(tiled[TOP], [0, -12, 312, 12]);
        assert_eq!(tiled[BOTTOM], [0, 200, 312, 12]);
    }

    #[test]
    fn edges_resize_from_sides_and_corners() {
        let at = |x, y| resize_edge_at(x, y, 300, 200);
        // The sides, away from the corners.
        assert_eq!(at(150.0, -5.0), Edge::TOP);
        assert_eq!(at(150.0, 205.0), Edge::BOTTOM);
        assert_eq!(at(-5.0, 100.0), Edge::LEFT);
        assert_eq!(at(305.0, 100.0), Edge::RIGHT);
        // The corners: within 16 points of them, along either side.
        assert_eq!(at(-5.0, -5.0), Edge::TOP_LEFT);
        assert_eq!(at(15.9, -5.0), Edge::TOP_LEFT);
        assert_eq!(at(16.0, -5.0), Edge::TOP);
        assert_eq!(at(-5.0, 15.9), Edge::TOP_LEFT);
        assert_eq!(at(-5.0, 16.0), Edge::LEFT);
        assert_eq!(at(284.0, -1.0), Edge::TOP_RIGHT);
        assert_eq!(at(283.9, -1.0), Edge::TOP);
        assert_eq!(at(305.0, 190.0), Edge::BOTTOM_RIGHT);
        assert_eq!(at(305.0, 183.9), Edge::RIGHT);
        assert_eq!(at(-1.0, 211.0), Edge::BOTTOM_LEFT);
        assert_eq!(at(10.0, 200.0), Edge::BOTTOM_LEFT);
        // Each its cursor, by shape and from the theme.
        for (edge, shape, name) in [
            (Edge::TOP, WpCursorShapeDeviceV1Shape::N_RESIZE, "n-resize"),
            (Edge::BOTTOM, WpCursorShapeDeviceV1Shape::S_RESIZE, "s-resize"),
            (Edge::LEFT, WpCursorShapeDeviceV1Shape::W_RESIZE, "w-resize"),
            (Edge::RIGHT, WpCursorShapeDeviceV1Shape::E_RESIZE, "e-resize"),
            (Edge::TOP_LEFT, WpCursorShapeDeviceV1Shape::NW_RESIZE, "nw-resize"),
            (Edge::TOP_RIGHT, WpCursorShapeDeviceV1Shape::NE_RESIZE, "ne-resize"),
            (Edge::BOTTOM_LEFT, WpCursorShapeDeviceV1Shape::SW_RESIZE, "sw-resize"),
            (Edge::BOTTOM_RIGHT, WpCursorShapeDeviceV1Shape::SE_RESIZE, "se-resize"),
            (Edge::NONE, WpCursorShapeDeviceV1Shape::DEFAULT, "default"),
        ] {
            assert_eq!(resize_shape(edge), shape);
            assert_eq!(resize_theme_names(edge)[0], name);
            assert_eq!(resize_theme_names(edge).len(), 2);
        }
    }

    /// C++'s drawEdge, as it wrote it: a pixel a point, its center's distance from the window.
    fn cpp_pixel(ex: i32, ey: i32, i: i32, j: i32, width: i32, height: i32, activated: bool) -> u32 {
        let strength = if activated { 0.32 } else { 0.16 };
        let (x, y) = (ex as f64 + i as f64 + 0.5, ey as f64 + j as f64 + 0.5);
        let max3 = |a: f64, b: f64, c: f64| a.max(b).max(c);
        let distance = f64::hypot(max3(0.0, -x, x - width as f64), max3(0.0, -y, y - height as f64));
        let t = (distance / 12.0).min(1.0);
        ((255.0 * strength * (1.0 - t) * (1.0 - t)).round() as u32) << 24
    }

    #[test]
    fn shadows_are_drawn_as_cpp_draws_them() {
        let (width, height) = (300, 200);
        for activated in [true, false] {
            for place in edge_places(width, height, [true; 4]) {
                let pixels = shadow_pixels(place, width, height, activated, 1);
                assert_eq!(pixels.len(), (place[2] * place[3]) as usize);
                for j in 0..place[3] {
                    for i in 0..place[2] {
                        let p = pixels[(j * place[2] + i) as usize];
                        assert_eq!(p, cpp_pixel(place[0], place[1], i, j, width, height, activated));
                    }
                }
            }
        }
        // Next to the window: 0.32 × (23/24)² of opaque black (75), inactive half that (37); fading out to nothing
        // at the edge's end and in its far corner; black, premultiplied.
        let top = edge_places(width, height, [true; 4])[TOP];
        let pixels = shadow_pixels(top, width, height, true, 1);
        let at = |i: i32, j: i32| pixels[(j * top[2] + i) as usize];
        assert_eq!(at(162, 11), 75 << 24);
        assert_eq!(at(162, 0), 0); // (11.5 points away: (1/24)² × 81.6 rounds to 0)
        assert_eq!(at(0, 0), 0);
        assert_eq!(at(11, 11), 72 << 24); // (the corner's pixel next to the window's corner, diagonally)
        assert_eq!(shadow_pixels(top, width, height, false, 1)[(11 * top[2] + 162) as usize], 37 << 24);
        // At scale 2: twice the pixels each way, each at its own center (the one nearest the window stronger).
        let doubled = shadow_pixels(top, width, height, true, 2);
        assert_eq!(doubled.len(), (top[2] * top[3] * 4) as usize);
        let at2 = |i: i32, j: i32| doubled[(j * top[2] * 2 + i) as usize];
        assert_eq!(at2(324, 23), u32::from(shadow_alpha(150.25, -0.25, width, height, true)) << 24);
        assert!(at2(324, 23) > at(162, 11) && at2(324, 22) < at(162, 11));
    }
}
