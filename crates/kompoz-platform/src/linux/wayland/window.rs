//! The window (C++'s openWindow, sizes and scale, configure and decoration listeners, window actions): an
//! xdg-shell toplevel, its size in the compositor's points, its scale (fractional-scale's 120ths shown through a
//! viewport, else the whole scale the surface prefers or its outputs have), its state, and what KompozR's own
//! decorations need of it (the title bar, its buttons as the desktop lays them out; the window's actions; its
//! edges, [`decorations`](super::decorations), kept as it is).

use super::generated::*;
use super::{OutputObject, WaylandPlatform, Window};
use crate::app::{AppConfig, Event, TitleBar, WindowAction};
use crate::linux::wayland::ffi::{Object, ObjectId};

/// An output's scale and refresh rate, and whether the window is on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Output {
    pub scale: i32,
    /// mHz (0: not told).
    pub refresh_mhz: i32,
    pub entered: bool,
}

impl Default for Output {
    fn default() -> Self {
        Output { scale: 1, refresh_mhz: 0, entered: false }
    }
}

/// The window's state as a configure tells it; tiled: on which sides (top, bottom, left, right) it meets another
/// window or the screen's edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) struct WindowState {
    pub maximized: bool,
    pub fullscreen: bool,
    pub activated: bool,
    /// Not shown at all (minimized, on another workspace: xdg_toplevel 6's suspended).
    pub suspended: bool,
    pub tiled: [bool; 4],
}

/// The words of a Wayland array of `uint`s.
pub(super) fn words(array: &[u8]) -> impl Iterator<Item = u32> + '_ {
    array.as_chunks::<4>().0.iter().map(|w| u32::from_ne_bytes(*w))
}

impl WindowState {
    /// The states of a toplevel's configure.
    pub fn from_states(states: &[u8]) -> WindowState {
        let mut s = WindowState::default();
        for state in words(states).map(XdgToplevelState) {
            match state {
                XdgToplevelState::MAXIMIZED => s.maximized = true,
                XdgToplevelState::FULLSCREEN => s.fullscreen = true,
                XdgToplevelState::ACTIVATED => s.activated = true,
                XdgToplevelState::SUSPENDED => s.suspended = true,
                XdgToplevelState::TILED_TOP => s.tiled[0] = true,
                XdgToplevelState::TILED_BOTTOM => s.tiled[1] = true,
                XdgToplevelState::TILED_LEFT => s.tiled[2] = true,
                XdgToplevelState::TILED_RIGHT => s.tiled[3] = true,
                _ => {}
            }
        }
        s
    }
}

/// The window's scale: fractional-scale's (shown through a viewport), else the whole buffer scale.
pub(super) fn window_scale(scale120: u32, viewport: bool, buffer_scale: i32) -> f64 {
    if scale120 > 0 && viewport { scale120 as f64 / 120.0 } else { buffer_scale.max(1) as f64 }
}

/// Pixels for `points` at `scale` (at least one).
pub(super) fn pixels(points: i32, scale: f64) -> i32 {
    ((points as f64 * scale).round() as i32).max(1)
}

/// The refresh rate of the outputs the window is on, the fastest (or of the first that says, before it is on any);
/// 60 if none says.
pub(super) fn refresh_rate(outputs: &[Output]) -> f32 {
    let mut best = 0;
    for o in outputs {
        if o.entered || best == 0 {
            best = best.max(o.refresh_mhz);
        }
    }
    if best > 0 { best as f32 / 1000.0 } else { 60.0 }
}

/// The largest scale of the outputs the window is on (1 if none).
pub(super) fn output_scale(outputs: &[Output]) -> i32 {
    outputs.iter().filter(|o| o.entered).map(|o| o.scale).max().unwrap_or(1).max(1)
}

impl WaylandPlatform {
    /// The window made and configured (C++'s openWindow before its backend's part): its surface (with a viewport and
    /// fractional scale where the compositor has them), its toplevel titled, its decorations asked for, committed
    /// bare, and the compositor's first configure acknowledged before anything is drawn.
    pub(super) fn make_window(&mut self, app: &AppConfig) -> Result<(), String> {
        self.title = app.title.clone();
        self.width = (app.width.0.round() as i32).max(1);
        self.height = (app.height.0.round() as i32).max(1);
        let g = &self.globals;
        let compositor = g.compositor.as_ref().ok_or("no wl_compositor")?;
        let wm_base = g.wm_base.as_ref().ok_or("no xdg_wm_base")?;
        let surface = compositor.create_surface().ok_or("no surface")?;
        let viewport = g.viewporter.as_ref().and_then(|v| v.get_viewport(&surface));
        let fractional = match (&g.fractional, &viewport) {
            (Some(manager), Some(_)) => manager.get_fractional_scale(&surface),
            _ => None,
        };
        let xdg_surface = wm_base.get_xdg_surface(&surface).ok_or("no xdg_surface")?;
        let toplevel = xdg_surface.get_toplevel().ok_or("no xdg_toplevel")?;
        toplevel.set_title(&self.title);
        toplevel.set_app_id("dev.kompoz.app");
        // The compositor's decorations where it has them (unless KOMPOZ_DECORATIONS=kompoz), else KompozR's own: as
        // the decoration's configure says.
        let decoration = g.decorations.as_ref().and_then(|d| d.get_toplevel_decoration(&toplevel));
        match &decoration {
            Some(d) => {
                let own = std::env::var("KOMPOZ_DECORATIONS").is_ok_and(|v| v == "kompoz");
                d.set_mode(if own {
                    ZxdgToplevelDecorationV1Mode::CLIENT_SIDE
                } else {
                    ZxdgToplevelDecorationV1Mode::SERVER_SIDE
                });
            }
            None => self.own_decorations = true,
        }
        surface.commit();
        self.window = Some(Window {
            edges: Default::default(),
            frame_callback: None,
            inhibitor: None,
            _decoration: decoration,
            toplevel,
            xdg_surface,
            viewport,
            _fractional: fractional,
            surface,
        });
        // (The first configure, acknowledged before anything is drawn.)
        for _ in 0..1000 {
            if self.configured {
                break;
            }
            self.roundtrip().map_err(|why| format!("the compositor didn't configure the window ({why})"))?;
        }
        if !self.configured {
            return Err("the compositor didn't configure the window".into());
        }
        self.apply_size();
        self.update_edges();
        Ok(())
    }

    /// The window's scale, px per point.
    pub(super) fn scale(&self) -> f64 {
        let viewport = self.window.as_ref().is_some_and(|w| w.viewport.is_some());
        window_scale(self.scale120, viewport, self.buffer_scale)
    }

    pub(super) fn refresh_rate(&self) -> f32 {
        let outputs: Vec<Output> = self.outputs.iter().map(|o| o.info).collect();
        refresh_rate(&outputs)
    }

    /// The surface's size for its scale (C++'s applySize): the drawable's, and the viewport's destination (the
    /// buffer at the scale's pixels; a viewport whenever there is one, so that a buffer of the old size committed
    /// with the new scale is never an error), else the whole buffer scale.
    pub(super) fn apply_size(&mut self) {
        let scale = self.scale();
        self.shared.scale.set(scale);
        self.drawable.set((pixels(self.width, scale), pixels(self.height, scale)));
        self.shared.cursor.borrow_mut().set_scale(scale.ceil() as i32);
        self.shared.tablet.borrow_mut().set_scale(scale.ceil() as i32);
        let Some(w) = &self.window else { return };
        match &w.viewport {
            Some(viewport) => {
                w.surface.set_buffer_scale(1);
                viewport.set_destination(self.width, self.height);
            }
            None => w.surface.set_buffer_scale(self.buffer_scale.max(1)),
        }
    }

    fn scale_changed(&mut self) {
        self.apply_size();
        self.update_edges(); // (their shadows drawn at the new scale)
        self.events.push_back(Event::MetricsChanged);
        self.events.push_back(Event::Resized);
    }

    /// Without fractional-scale or the surface's preferred scale: the largest scale of the outputs it is on.
    fn output_scale_changed(&mut self) {
        let preferred = self.window.as_ref().is_some_and(|w| w.surface.version() >= 6);
        if self.scale120 > 0 || preferred {
            return;
        }
        let outputs: Vec<Output> = self.outputs.iter().map(|o| o.info).collect();
        let best = output_scale(&outputs);
        if best != self.buffer_scale {
            self.buffer_scale = best;
            self.scale_changed();
        }
    }

    fn output(&mut self, id: ObjectId) -> Option<&mut OutputObject> {
        self.outputs.iter_mut().find(|o| o.output.id() == id)
    }

    pub(super) fn on_output(&mut self, sender: ObjectId, e: WlOutputEvent) {
        match e {
            WlOutputEvent::Mode { flags, refresh, .. } => {
                if flags.contains(WlOutputMode::CURRENT)
                    && let Some(o) = self.output(sender)
                {
                    o.info.refresh_mhz = refresh;
                }
            }
            WlOutputEvent::Scale { factor } => {
                if let Some(o) = self.output(sender) {
                    o.info.scale = factor;
                }
                self.output_scale_changed();
            }
            _ => {}
        }
    }

    pub(super) fn on_surface(&mut self, sender: ObjectId, e: WlSurfaceEvent) {
        if !self.window.as_ref().is_some_and(|w| w.surface.id() == sender) {
            return;
        }
        match e {
            WlSurfaceEvent::Enter { output } => {
                if let Some(o) = self.output(output) {
                    o.info.entered = true;
                }
                self.output_scale_changed();
                self.events.push_back(Event::MetricsChanged); // (its refresh rate)
            }
            WlSurfaceEvent::Leave { output } => {
                if let Some(o) = self.output(output) {
                    o.info.entered = false;
                }
                self.output_scale_changed();
            }
            WlSurfaceEvent::PreferredBufferScale { factor } => {
                if factor != self.buffer_scale {
                    self.buffer_scale = factor.max(1);
                    if self.scale120 == 0 {
                        self.scale_changed();
                    }
                }
            }
            WlSurfaceEvent::PreferredBufferTransform { .. } => {}
        }
    }

    pub(super) fn on_fractional_scale(&mut self, e: WpFractionalScaleV1Event) {
        let WpFractionalScaleV1Event::PreferredScale { scale } = e;
        if scale != self.scale120 {
            self.scale120 = scale;
            self.scale_changed();
        }
    }

    /// A configure done (C++'s xdg_surface listener): acknowledged, the toplevel's size and state taken.
    pub(super) fn on_xdg_surface(&mut self, e: XdgSurfaceEvent) {
        let XdgSurfaceEvent::Configure { serial } = e;
        let Some(w) = &self.window else { return };
        w.xdg_surface.ack_configure(serial);
        let mut resized = false;
        if self.pending_width > 0
            && self.pending_height > 0
            && (self.pending_width != self.width || self.pending_height != self.height)
        {
            self.width = self.pending_width;
            self.height = self.pending_height;
            resized = true;
        }
        let was = self.state;
        self.state = self.pending_state;
        let state_changed = was != self.state;
        if !self.configured {
            self.configured = true;
            self.events.push_back(Event::Shown);
            self.events.push_back(Event::Resized);
            if self.state.suspended {
                self.events.push_back(Event::Hidden); // (mapped where it can't be seen: no frames come until it is)
            }
            return;
        }
        if resized {
            self.apply_size();
            self.events.push_back(Event::Resized);
        }
        if resized || state_changed {
            self.update_edges(); // (where they are, whether they are, how active their shadows are)
        }
        if state_changed {
            self.events.push_back(Event::MetricsChanged); // (the title bar's)
            if self.state.suspended != was.suspended {
                self.events.push_back(if self.state.suspended { Event::Hidden } else { Event::Shown });
            }
        }
    }

    pub(super) fn on_toplevel(&mut self, e: XdgToplevelEvent) {
        match e {
            XdgToplevelEvent::Configure { width, height, states } => {
                self.pending_width = width; // (0: the app's own size)
                self.pending_height = height;
                self.pending_state = WindowState::from_states(&states);
            }
            XdgToplevelEvent::Close => self.events.push_back(Event::Quit),
            XdgToplevelEvent::ConfigureBounds { .. } => {}
            XdgToplevelEvent::WmCapabilities { capabilities } => {
                let told: Vec<XdgToplevelWmCapabilities> =
                    words(&capabilities).map(XdgToplevelWmCapabilities).collect();
                self.can_minimize = told.contains(&XdgToplevelWmCapabilities::MINIMIZE);
                self.can_maximize = told.contains(&XdgToplevelWmCapabilities::MAXIMIZE);
                self.events.push_back(Event::MetricsChanged);
            }
        }
    }

    /// Whether the compositor decorates the window, or KompozR (C++'s decoration listener).
    pub(super) fn on_decoration(&mut self, e: ZxdgToplevelDecorationV1Event) {
        let ZxdgToplevelDecorationV1Event::Configure { mode } = e;
        let own = mode == ZxdgToplevelDecorationV1Mode::CLIENT_SIDE;
        if own == self.own_decorations {
            return;
        }
        self.own_decorations = own;
        if self.configured {
            self.update_edges();
            self.events.push_back(Event::MetricsChanged);
        }
    }

    /// The title bar KompozR draws: its buttons as the desktop lays them out, those the window manager does,
    /// maximized or not.
    pub(super) fn title_bar(&self) -> TitleBar {
        self.desktop.title_bar(self.state.maximized, self.can_minimize, self.can_maximize)
    }

    /// What KompozR's title bar asks of the window (C++'s windowAction): moved by the press it was dragged with,
    /// minimized, maximized or restored, closed, its menu.
    pub(super) fn act(&mut self, action: WindowAction, x: f32, y: f32) {
        let Some(w) = &self.window else { return };
        let seat = self.globals.seat.as_ref();
        match action {
            WindowAction::Move => {
                let Some(seat) = seat else { return };
                w.toplevel.r#move(seat, self.press_serial);
                self.let_go();
            }
            WindowAction::Minimize => w.toplevel.set_minimized(),
            WindowAction::ToggleMaximize => {
                if self.state.maximized {
                    w.toplevel.unset_maximized();
                } else {
                    w.toplevel.set_maximized();
                }
            }
            WindowAction::Close => self.events.push_back(Event::Quit),
            WindowAction::Menu => {
                if let Some(seat) = seat {
                    w.toplevel.show_window_menu(seat, self.press_serial, x as i32, y as i32);
                }
            }
        }
    }

    /// The pointer or the finger last pressed let go, as far as the app knows: the window manager takes it to move
    /// the window (no release comes to the window after).
    fn let_go(&mut self) {
        let time = crate::linux::sys::monotonic_now();
        match self.press_touch {
            None => {
                let modifiers = self.keyboard.modifiers();
                self.events.push_back(Event::MouseUp {
                    x: self.pointer_x as f32,
                    y: self.pointer_y as f32,
                    button: crate::app::MouseButton::Left,
                    modifiers,
                    time,
                });
            }
            Some(id) => {
                if let Some((x, y)) = self.touches.remove(&id) {
                    self.events.push_back(Event::TouchUp { pointer: id as i64, x: x as f32, y: y as f32, time });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_are_fractional_through_a_viewport() {
        assert_eq!(window_scale(150, true, 2), 1.25);
        assert_eq!(window_scale(150, false, 2), 2.0); // (no viewport: the whole scale)
        assert_eq!(window_scale(0, true, 2), 2.0);
        assert_eq!(window_scale(0, true, 0), 1.0);
        assert_eq!(pixels(801, 1.25), 1001);
        assert_eq!(pixels(800, 1.5), 1200);
        assert_eq!(pixels(0, 2.0), 1);
    }

    #[test]
    fn outputs_give_scale_and_refresh() {
        let o = |scale, refresh_mhz, entered| Output { scale, refresh_mhz, entered };
        assert_eq!(refresh_rate(&[]), 60.0);
        assert_eq!(refresh_rate(&[o(1, 59_951, false), o(2, 144_000, false)]), 59.951);
        assert_eq!(refresh_rate(&[o(1, 59_951, false), o(2, 144_000, true)]), 144.0);
        assert_eq!(output_scale(&[o(1, 0, true), o(3, 0, false), o(2, 0, true)]), 2);
        assert_eq!(output_scale(&[o(3, 0, false)]), 1);
    }

    #[test]
    fn configure_states_are_read() {
        let array = |states: &[u32]| -> Vec<u8> { states.iter().flat_map(|s| s.to_ne_bytes()).collect() };
        let s = WindowState::from_states(&array(&[1, 4, 5, 6]));
        assert!(s.maximized && s.activated && !s.fullscreen && !s.suspended);
        assert_eq!(s.tiled, [false, false, true, true]);
        assert!(WindowState::from_states(&array(&[2, 9])).suspended);
        assert_eq!(WindowState::from_states(&[1, 0]), WindowState::default()); // (not a whole word)
    }
}
