//! The window (C++'s openWindow, sizes and scale, configure and decoration listeners, window actions): an
//! xdg-shell toplevel, its size in the compositor's points, its scale (fractional-scale's 120ths shown through a
//! viewport, else the whole scale the surface prefers or its outputs have), its state, and what KompozR's own
//! decorations need of it (the title bar, its buttons as the desktop lays them out; the window's actions; its
//! edges, [`decorations`](super::decorations), kept as it is).

use super::generated::*;
use super::{OutputObject, WaylandPlatform, Window};
use crate::app::{Event, TitleBar, WindowAction, WindowSpec};
use crate::linux::wayland::ffi::{Object, ObjectId};
use kompoz_ui::window::WindowPlacement;

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
    /// Whether the window floats: its size its own, neither maximized, fullscreen nor tiled on any side.
    pub fn floating(&self) -> bool {
        !self.maximized && !self.fullscreen && !self.tiled.iter().any(|t| *t)
    }

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
    pub(super) fn make_window(&mut self, app: &WindowSpec) -> Result<(), String> {
        self.win.title = app.title.clone();
        self.win.width = (app.width.0.round() as i32).max(1);
        self.win.height = (app.height.0.round() as i32).max(1);
        self.win.undecorated = app.undecorated;
        self.win.configured = false;
        self.win.state = WindowState::default();
        self.win.pending_state = WindowState::default();
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
        toplevel.set_title(&self.win.title);
        toplevel.set_app_id("dev.kompoz.app");
        // The compositor's decorations where it has them (unless KOMPOZ_DECORATIONS=kompoz, or the window is
        // undecorated: client side, KompozR drawing none), else KompozR's own: as the decoration's configure says.
        let decoration = g.decorations.as_ref().and_then(|d| d.get_toplevel_decoration(&toplevel));
        match &decoration {
            Some(d) => {
                let own = app.undecorated || std::env::var("KOMPOZ_DECORATIONS").is_ok_and(|v| v == "kompoz");
                d.set_mode(if own {
                    ZxdgToplevelDecorationV1Mode::CLIENT_SIDE
                } else {
                    ZxdgToplevelDecorationV1Mode::SERVER_SIDE
                });
            }
            None => self.win.own_decorations = true,
        }
        // How it is placed, as it opens (its first configure says what the compositor did).
        match app.placement {
            WindowPlacement::Maximized => toplevel.set_maximized(),
            WindowPlacement::Fullscreen => toplevel.set_fullscreen(None),
            WindowPlacement::Floating => {}
        }
        if !app.resizable {
            toplevel.set_min_size(self.win.width, self.win.height);
            toplevel.set_max_size(self.win.width, self.win.height);
        }
        surface.commit();
        self.win.window = Some(Window {
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
            if self.win.configured {
                break;
            }
            self.roundtrip().map_err(|why| format!("the compositor didn't configure the window ({why})"))?;
        }
        if !self.win.configured {
            return Err("the compositor didn't configure the window".into());
        }
        self.apply_size();
        self.update_edges();
        if app.minimized
            && let Some(w) = &self.win.window
        {
            w.toplevel.set_minimized();
            self.win.events.push_back(Event::Minimized);
        }
        Ok(())
    }

    /// The window's scale, px per point (a window without a surface — the app's composition, a hidden window —
    /// the outputs' largest).
    pub(super) fn scale(&self) -> f64 {
        let Some(w) = &self.win.window else {
            return self.outputs.iter().map(|o| o.info.scale).max().unwrap_or(1).max(1) as f64;
        };
        window_scale(self.win.scale120, w.viewport.is_some(), self.win.buffer_scale)
    }

    /// The outputs as the window is on them.
    fn window_outputs(&self) -> Vec<Output> {
        let on = |o: &OutputObject| self.win.entered.contains(&o.output.id());
        self.outputs.iter().map(|o| Output { entered: on(o), ..o.info }).collect()
    }

    pub(super) fn refresh_rate(&self) -> f32 {
        refresh_rate(&self.window_outputs())
    }

    /// The surface's size for its scale (C++'s applySize): the drawable's, and the viewport's destination (the
    /// buffer at the scale's pixels; a viewport whenever there is one, so that a buffer of the old size committed
    /// with the new scale is never an error), else the whole buffer scale.
    pub(super) fn apply_size(&mut self) {
        let scale = self.scale();
        self.win.scale_cell.set(scale);
        self.win.drawable.set((pixels(self.win.width, scale), pixels(self.win.height, scale)));
        // (The cursors at the scale of the window the pointer is in.)
        if self.pointer_window.is_none_or(|w| w == self.win.id) {
            self.shared.cursor.borrow_mut().set_scale(scale.ceil() as i32);
            self.shared.tablet.borrow_mut().set_scale(scale.ceil() as i32);
        }
        let Some(w) = &self.win.window else { return };
        match &w.viewport {
            Some(viewport) => {
                w.surface.set_buffer_scale(1);
                viewport.set_destination(self.win.width, self.win.height);
            }
            None => w.surface.set_buffer_scale(self.win.buffer_scale.max(1)),
        }
    }

    fn scale_changed(&mut self) {
        self.apply_size();
        self.update_edges(); // (their shadows drawn at the new scale)
        self.win.events.push_back(Event::MetricsChanged);
        self.win.events.push_back(Event::Resized);
    }

    /// Without fractional-scale or the surface's preferred scale: the largest scale of the outputs it is on.
    pub(super) fn output_scale_changed(&mut self) {
        let preferred = self.win.window.as_ref().is_some_and(|w| w.surface.version() >= 6);
        if self.win.scale120 > 0 || preferred || self.win.window.is_none() {
            return;
        }
        let best = output_scale(&self.window_outputs());
        if best != self.win.buffer_scale {
            self.win.buffer_scale = best;
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
            }
            _ => {}
        }
    }

    pub(super) fn on_surface(&mut self, sender: ObjectId, e: WlSurfaceEvent) {
        if !self.win.window.as_ref().is_some_and(|w| w.surface.id() == sender) {
            return;
        }
        match e {
            WlSurfaceEvent::Enter { output } => {
                if !self.win.entered.contains(&output) {
                    self.win.entered.push(output);
                }
                self.output_scale_changed();
                self.win.events.push_back(Event::MetricsChanged); // (its refresh rate)
            }
            WlSurfaceEvent::Leave { output } => {
                self.win.entered.retain(|&o| o != output);
                self.output_scale_changed();
            }
            WlSurfaceEvent::PreferredBufferScale { factor } => {
                if factor != self.win.buffer_scale {
                    self.win.buffer_scale = factor.max(1);
                    if self.win.scale120 == 0 {
                        self.scale_changed();
                    }
                }
            }
            WlSurfaceEvent::PreferredBufferTransform { .. } => {}
        }
    }

    pub(super) fn on_fractional_scale(&mut self, e: WpFractionalScaleV1Event) {
        let WpFractionalScaleV1Event::PreferredScale { scale } = e;
        if scale != self.win.scale120 {
            self.win.scale120 = scale;
            self.scale_changed();
        }
    }

    /// A configure done (C++'s xdg_surface listener): acknowledged, the toplevel's size and state taken.
    pub(super) fn on_xdg_surface(&mut self, e: XdgSurfaceEvent) {
        let XdgSurfaceEvent::Configure { serial } = e;
        let Some(w) = &self.win.window else { return };
        w.xdg_surface.ack_configure(serial);
        let mut resized = false;
        if self.win.pending_width <= 0
            && self.win.pending_height <= 0
            && self.win.pending_state.floating()
            && !self.win.state.floating()
            && self.win.floating_width > 0
        {
            // Back from maximized, fullscreen or tiled with the size left to the app: the size it floated at.
            self.win.pending_width = self.win.floating_width;
            self.win.pending_height = self.win.floating_height;
        }
        if self.win.pending_width > 0
            && self.win.pending_height > 0
            && (self.win.pending_width != self.win.width || self.win.pending_height != self.win.height)
        {
            self.win.width = self.win.pending_width;
            self.win.height = self.win.pending_height;
            resized = true;
        }
        let was = self.win.state;
        self.win.state = self.win.pending_state;
        let state_changed = was != self.win.state;
        if self.win.state.floating() {
            (self.win.floating_width, self.win.floating_height) = (self.win.width, self.win.height);
        }
        if !self.win.configured {
            self.win.configured = true;
            self.win.events.push_back(Event::Shown);
            self.win.events.push_back(Event::Resized);
            if self.win.state.suspended {
                self.win.events.push_back(Event::Hidden); // (mapped where it can't be seen: no frames come until it is)
            }
            return;
        }
        if resized {
            self.apply_size();
            self.win.events.push_back(Event::Resized);
        }
        if resized || state_changed {
            self.update_edges(); // (where they are, whether they are, how active their shadows are)
        }
        if state_changed {
            self.win.events.push_back(Event::MetricsChanged); // (the title bar's)
            if self.win.state.suspended != was.suspended {
                self.win.events.push_back(if self.win.state.suspended { Event::Hidden } else { Event::Shown });
            }
        }
    }

    pub(super) fn on_toplevel(&mut self, e: XdgToplevelEvent) {
        match e {
            XdgToplevelEvent::Configure { width, height, states } => {
                self.win.pending_width = width; // (0: the app's own size)
                self.win.pending_height = height;
                self.win.pending_state = WindowState::from_states(&states);
            }
            XdgToplevelEvent::Close => self.win.events.push_back(Event::CloseRequest),
            XdgToplevelEvent::ConfigureBounds { .. } => {}
            XdgToplevelEvent::WmCapabilities { capabilities } => {
                let told: Vec<XdgToplevelWmCapabilities> =
                    words(&capabilities).map(XdgToplevelWmCapabilities).collect();
                self.win.can_minimize = told.contains(&XdgToplevelWmCapabilities::MINIMIZE);
                self.win.can_maximize = told.contains(&XdgToplevelWmCapabilities::MAXIMIZE);
                self.win.events.push_back(Event::MetricsChanged);
            }
        }
    }

    /// Whether the compositor decorates the window, or KompozR (C++'s decoration listener).
    pub(super) fn on_decoration(&mut self, e: ZxdgToplevelDecorationV1Event) {
        let ZxdgToplevelDecorationV1Event::Configure { mode } = e;
        let own = mode == ZxdgToplevelDecorationV1Mode::CLIENT_SIDE;
        if own == self.win.own_decorations {
            return;
        }
        self.win.own_decorations = own;
        if self.win.configured {
            self.update_edges();
            self.win.events.push_back(Event::MetricsChanged);
        }
    }

    /// The title bar KompozR draws: its buttons as the desktop lays them out, those the window manager does,
    /// maximized or not.
    pub(super) fn title_bar(&self) -> TitleBar {
        self.desktop.title_bar(self.win.state.maximized, self.win.can_minimize, self.win.can_maximize)
    }

    /// What KompozR's title bar asks of the window (C++'s windowAction): moved by the press it was dragged with,
    /// minimized, maximized or restored, closed, its menu.
    pub(super) fn act(&mut self, action: WindowAction, x: f32, y: f32) {
        let Some(w) = &self.win.window else { return };
        let seat = self.globals.seat.as_ref();
        match action {
            WindowAction::Move => {
                let Some(seat) = seat else { return };
                w.toplevel.r#move(seat, self.press_serial);
                self.let_go();
            }
            WindowAction::Minimize => w.toplevel.set_minimized(),
            WindowAction::ToggleMaximize => {
                if self.win.state.maximized {
                    w.toplevel.unset_maximized();
                } else {
                    w.toplevel.set_maximized();
                }
            }
            WindowAction::Close => self.win.events.push_back(Event::CloseRequest),
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
                self.win.events.push_back(Event::MouseUp {
                    x: self.pointer_x as f32,
                    y: self.pointer_y as f32,
                    button: crate::app::MouseButton::Left,
                    modifiers,
                    time,
                });
            }
            Some(id) => {
                if let Some((x, y)) = self.touches.remove(&id) {
                    self.win.events.push_back(Event::TouchUp { pointer: id as i64, x: x as f32, y: y as f32, time });
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

    #[test]
    fn a_window_floats_unless_maximized_fullscreen_or_tiled() {
        let array = |states: &[u32]| -> Vec<u8> { states.iter().flat_map(|s| s.to_ne_bytes()).collect() };
        assert!(WindowState::default().floating());
        assert!(WindowState::from_states(&array(&[4])).floating()); // (activated only)
        for state in [1, 2, 5, 6, 7, 8] {
            assert!(!WindowState::from_states(&array(&[state])).floating(), "state {state}");
        }
    }
}
