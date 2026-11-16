//! The pointer's cursor (C++'s showCursor): by shape where the compositor has cursor-shape-v1, else drawn from the
//! cursor theme (`XCURSOR_THEME`, `XCURSOR_SIZE`) through libwayland-cursor on a surface of its own, at the
//! window's scale, an animated cursor's frames shown in turn: the host's over the window, a resize cursor over
//! KompozR's edges around it. Shared with the host's `cursor` callback.

use super::decorations::{resize_shape, resize_theme_names};
use super::ffi::{Connection, CursorTheme};
use super::generated::*;
use kompoz_ui::platform::PointerIcon;
use std::rc::Rc;

/// cursor-shape-v1's shape for a cursor (C++'s table).
pub(super) fn shape_of(icon: PointerIcon) -> WpCursorShapeDeviceV1Shape {
    type Shape = WpCursorShapeDeviceV1Shape;
    match icon {
        PointerIcon::Default => Shape::DEFAULT,
        PointerIcon::Hand => Shape::POINTER,
        PointerIcon::Text => Shape::TEXT,
        PointerIcon::Crosshair => Shape::CROSSHAIR,
        PointerIcon::Move => Shape::MOVE,
        PointerIcon::ResizeHorizontal => Shape::EW_RESIZE,
        PointerIcon::ResizeVertical => Shape::NS_RESIZE,
        PointerIcon::NotAllowed => Shape::NOT_ALLOWED,
        PointerIcon::Wait => Shape::WAIT,
    }
}

/// The names a cursor theme may have a cursor under (the CSS names cursor-shape-v1 uses first, then X11's older
/// ones), the first a theme has is shown.
pub(super) fn theme_names(icon: PointerIcon) -> &'static [&'static str] {
    match icon {
        PointerIcon::Default => &["default", "left_ptr"],
        PointerIcon::Hand => &["pointer", "hand2", "hand1"],
        PointerIcon::Text => &["text", "xterm", "ibeam"],
        PointerIcon::Crosshair => &["crosshair", "cross", "tcross"],
        PointerIcon::Move => &["move", "fleur", "all-scroll"],
        PointerIcon::ResizeHorizontal => &["ew-resize", "sb_h_double_arrow", "h_double_arrow"],
        PointerIcon::ResizeVertical => &["ns-resize", "sb_v_double_arrow", "v_double_arrow"],
        PointerIcon::NotAllowed => &["not-allowed", "crossed_circle", "forbidden"],
        PointerIcon::Wait => &["wait", "watch"],
    }
}

/// The theme's size for cursors at scale 1 (`XCURSOR_SIZE`, else 24).
pub(super) fn theme_size() -> i32 {
    std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.trim().parse().ok()).filter(|&s: &i32| s > 0).unwrap_or(24)
}

/// Drawn from the theme: its surface, the theme at the scale it was loaded for, the animation shown.
struct Themed {
    surface: WlSurface,
    theme: Option<(CursorTheme, i32)>,
    /// An animated cursor's start (monotonic ns) and its next frame's time.
    animation: Option<(i64, i64)>,
}

/// The pointer and its cursor, as the platform and the host's `cursor` callback share them.
pub(crate) struct Cursors {
    connection: Connection,
    pub pointer: Option<WlPointer>,
    /// cursor-shape-v1's device for the pointer.
    device: Option<WpCursorShapeDeviceV1>,
    /// Else the theme's.
    themed: Option<Themed>,
    compositor: Option<Rc<WlCompositor>>,
    shm: Option<Rc<WlShm>>,
    /// The pointer's enter's serial (0: not over the window).
    serial: u32,
    icon: PointerIcon,
    /// Over one of KompozR's edges: the resize cursor for where it would resize from, instead of the host's.
    resize: Option<XdgToplevelResizeEdge>,
    /// The window's scale, whole (the theme's cursors are drawn at it).
    scale: i32,
}

impl Cursors {
    pub fn new(connection: Connection) -> Cursors {
        Cursors {
            connection,
            pointer: None,
            device: None,
            themed: None,
            compositor: None,
            shm: None,
            serial: 0,
            icon: PointerIcon::Default,
            resize: None,
            scale: 1,
        }
    }

    /// The pointer given its cursor's means: cursor-shape-v1's device, or a surface for the theme's.
    pub fn attach(
        &mut self,
        shapes: Option<&WpCursorShapeManagerV1>,
        compositor: Option<Rc<WlCompositor>>,
        shm: Option<Rc<WlShm>>,
    ) {
        self.compositor = self.compositor.take().or(compositor);
        self.shm = self.shm.take().or(shm);
        let Some(pointer) = &self.pointer else { return };
        if self.device.is_some() || self.themed.is_some() {
            return;
        }
        if let Some(shapes) = shapes {
            self.device = shapes.get_pointer(pointer);
        } else if let Some(surface) = self.compositor.as_ref().and_then(|c| c.create_surface()) {
            self.themed = Some(Themed { surface, theme: None, animation: None });
        }
    }

    /// The pointer gone (the seat has none any more), with its cursor.
    pub fn release_pointer(&mut self) {
        self.device = None;
        self.themed = None;
        self.serial = 0;
        self.pointer = None;
    }

    /// The pointer entered a surface of the window with `serial`: its cursor shown.
    pub fn entered(&mut self, serial: u32) {
        self.serial = serial;
        self.resize = None;
        self.show();
    }

    /// The pointer entered one of KompozR's edges with `serial`, where it would resize the window from `edge`: the
    /// resize cursor shown.
    pub fn entered_edge(&mut self, serial: u32, edge: XdgToplevelResizeEdge) {
        self.serial = serial;
        self.resize = Some(edge);
        self.show();
    }

    /// The pointer moved over an edge to where it would resize from `edge`: its cursor shown, if another.
    pub fn over_edge(&mut self, edge: XdgToplevelResizeEdge) {
        if self.resize != Some(edge) {
            self.resize = Some(edge);
            self.show();
        }
    }

    pub fn left(&mut self) {
        self.serial = 0;
        if let Some(t) = &mut self.themed {
            t.animation = None;
        }
    }

    /// The host's cursor.
    pub fn set_icon(&mut self, icon: PointerIcon) {
        if icon != self.icon {
            self.icon = icon;
            if self.resize.is_none() {
                self.show();
            }
        }
    }

    /// The window's scale (whole): the theme's cursors drawn again at it.
    pub fn set_scale(&mut self, scale: i32) {
        let scale = scale.max(1);
        if scale != self.scale {
            self.scale = scale;
            self.show();
        }
    }

    /// The cursor shown, while the pointer is over the window.
    fn show(&mut self) {
        if self.serial == 0 {
            return;
        }
        if let Some(device) = &self.device {
            device.set_shape(self.serial, self.resize.map_or(shape_of(self.icon), resize_shape));
            return;
        }
        let start = crate::linux::sys::monotonic_now();
        self.draw(start, start);
    }

    /// The theme's cursor drawn as it is at `now`, its animation begun at `start`.
    fn draw(&mut self, start: i64, now: i64) {
        let (Some(pointer), Some(themed), Some(shm)) = (&self.pointer, &mut self.themed, &self.shm) else { return };
        let scale = self.scale;
        if themed.theme.as_ref().is_none_or(|(_, s)| *s != scale) {
            let name = std::env::var("XCURSOR_THEME").ok().filter(|n| !n.is_empty());
            themed.theme = match CursorTheme::load(&self.connection, name.as_deref(), theme_size() * scale, shm) {
                Ok(theme) => Some((theme, scale)),
                Err(why) => {
                    crate::system::log(&format!("kompoz: Wayland: no cursor theme ({why})"));
                    None
                }
            };
        }
        let Some((theme, _)) = &themed.theme else { return };
        let names = self.resize.map_or(theme_names(self.icon), resize_theme_names);
        let Some(cursor) = names.iter().find_map(|n| theme.cursor(n)) else { return };
        let elapsed_ms = ((now - start).max(0) / 1_000_000) as u32;
        let (frame, duration) = cursor.frame_at(elapsed_ms);
        let Some(image) = cursor.image(frame) else { return };
        let surface = &themed.surface;
        surface.attach(Some(image.buffer()), 0, 0);
        surface.set_buffer_scale(scale);
        surface.damage_buffer(0, 0, image.width, image.height);
        surface.commit();
        pointer.set_cursor(self.serial, Some(surface), image.hotspot_x / scale, image.hotspot_y / scale);
        themed.animation =
            (cursor.image_count() > 1 && duration > 0).then(|| (start, now + i64::from(duration) * 1_000_000));
    }

    /// When an animated cursor's next frame is due (monotonic ns).
    pub fn next_frame_at(&self) -> Option<i64> {
        self.themed.as_ref()?.animation.map(|(_, next)| next)
    }

    /// An animated cursor's frame shown, if one is due by `now`.
    pub fn animate(&mut self, now: i64) {
        let Some((start, next)) = self.themed.as_ref().and_then(|t| t.animation) else { return };
        if now >= next && self.serial != 0 {
            self.draw(start, now);
            let _ = self.connection.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cursor_has_a_shape_and_theme_names() {
        for icon in [
            PointerIcon::Default,
            PointerIcon::Hand,
            PointerIcon::Text,
            PointerIcon::Crosshair,
            PointerIcon::Move,
            PointerIcon::ResizeHorizontal,
            PointerIcon::ResizeVertical,
            PointerIcon::NotAllowed,
            PointerIcon::Wait,
        ] {
            assert!(!theme_names(icon).is_empty());
            assert_ne!(shape_of(icon), WpCursorShapeDeviceV1Shape(0));
        }
        assert_eq!(shape_of(PointerIcon::Hand), WpCursorShapeDeviceV1Shape::POINTER);
        assert_eq!(theme_names(PointerIcon::Text)[0], "text");
    }
}
