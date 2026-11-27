//! The page as the web platform reads it (web.js's `name=value` lines: the canvas's size, the device pixel ratio,
//! the media queries' answers, the focus, the visibility, the address's fragment), what changed in it as the app's
//! events, and the page's names for what the app asks (the text area's keyboard, the cursor).

use crate::app::Event;
use kompoz_ui::platform::{ImeAction, ImeOptions, KeyboardCapitalization, KeyboardType, PointerIcon};

/// The page as the platform last read it ([`sys::state`]).
#[derive(Clone, PartialEq, Debug)]
pub struct PageState {
    /// The canvas's size, CSS pixels.
    pub width: f64,
    pub height: f64,
    /// Device pixels per CSS pixel.
    pub ratio: f64,
    /// The canvas's size in device pixels as the browser has it (its `devicePixelContentBoxSize`; 0: not told).
    pub device_width: i32,
    pub device_height: i32,
    pub dark: bool,
    pub reduced_motion: bool,
    pub fine_pointer: bool,
    pub coarse_pointer: bool,
    pub focused: bool,
    pub visible: bool,
    pub fullscreen: bool,
    /// The address's fragment (without its `#`).
    pub hash: String,
    /// The screen's safe area's insets, CSS pixels: left, top, right, bottom.
    pub safe: [f64; 4],
    pub screen: (i32, i32),
    pub microphone: bool,
    pub camera: bool,
    /// The display's frames a second, as the page measured them between its animation frames.
    pub refresh: f64,
}

impl Default for PageState {
    fn default() -> PageState {
        PageState {
            width: 1.0,
            height: 1.0,
            ratio: 1.0,
            device_width: 0,
            device_height: 0,
            dark: false,
            reduced_motion: false,
            fine_pointer: true,
            coarse_pointer: false,
            focused: true,
            visible: true,
            fullscreen: false,
            hash: String::new(),
            safe: [0.0; 4],
            screen: (0, 0),
            microphone: false,
            camera: false,
            refresh: 60.0,
        }
    }
}

impl PageState {
    /// From the page's `name=value` lines.
    pub fn parse(lines: &str) -> PageState {
        let mut s = PageState::default();
        for line in lines.lines() {
            let Some((name, value)) = line.split_once('=') else { continue };
            let number = value.trim().parse::<f64>().ok().filter(|n| n.is_finite());
            let flag = value.trim() == "1";
            match name {
                "width" => s.width = number.unwrap_or(1.0).max(1.0),
                "height" => s.height = number.unwrap_or(1.0).max(1.0),
                "ratio" => s.ratio = number.filter(|r| *r > 0.0).unwrap_or(1.0),
                "device_width" => s.device_width = number.unwrap_or(0.0) as i32,
                "device_height" => s.device_height = number.unwrap_or(0.0) as i32,
                "dark" => s.dark = flag,
                "reduced_motion" => s.reduced_motion = flag,
                "fine" => s.fine_pointer = flag,
                "coarse" => s.coarse_pointer = flag,
                "focused" => s.focused = flag,
                "visible" => s.visible = flag,
                "fullscreen" => s.fullscreen = flag,
                "hash" => s.hash = super::events::decoded(value),
                "safe" => {
                    let v: Vec<f64> = value.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                    if let [l, t, r, b] = v[..] {
                        s.safe = [l, t, r, b];
                    }
                }
                "screen" => {
                    if let Some((w, h)) = value.split_once(',') {
                        s.screen = (w.trim().parse().unwrap_or(0), h.trim().parse().unwrap_or(0));
                    }
                }
                "microphone" => s.microphone = flag,
                "camera" => s.camera = flag,
                "refresh" => s.refresh = number.filter(|r| *r >= 20.0).unwrap_or(60.0),
                _ => {}
            }
        }
        s
    }

    /// What changed from `before` to this, as the app's events.
    pub fn changes(&self, before: &PageState) -> Vec<Event> {
        let mut events = Vec::new();
        let sized = (self.width, self.height, self.device_width, self.device_height)
            != (before.width, before.height, before.device_width, before.device_height);
        if sized {
            events.push(Event::Resized);
        }
        if sized
            || self.ratio != before.ratio
            || self.dark != before.dark
            || self.safe != before.safe
            || self.refresh != before.refresh
        {
            events.push(Event::MetricsChanged);
        }
        if (self.fine_pointer, self.coarse_pointer, self.microphone, self.camera)
            != (before.fine_pointer, before.coarse_pointer, before.microphone, before.camera)
        {
            events.push(Event::DevicesChanged);
        }
        if self.focused != before.focused {
            events.push(if self.focused { Event::FocusGained } else { Event::FocusLost });
        }
        if self.visible != before.visible {
            events.push(if self.visible { Event::Shown } else { Event::Hidden });
        }
        if self.fullscreen != before.fullscreen {
            events.push(if self.fullscreen { Event::Maximized } else { Event::Restored });
        }
        events
    }
}

/// The page's keyboard for a field (the text area's `inputmode`, `enterkeyhint`, `autocapitalize`,
/// `autocorrect`), as web.js reads it: four words.
pub(super) fn keyboard_kind(options: &ImeOptions) -> String {
    let mode = match options.keyboard_type {
        KeyboardType::Number | KeyboardType::NumberPassword | KeyboardType::NumberSigned => "numeric",
        KeyboardType::NumberPasswordSigned => "numeric",
        KeyboardType::Decimal | KeyboardType::DecimalSigned | KeyboardType::DecimalPassword => "decimal",
        KeyboardType::DecimalPasswordSigned => "decimal",
        KeyboardType::Phone => "tel",
        KeyboardType::Uri => "url",
        KeyboardType::Email => "email",
        KeyboardType::Filter => "search",
        _ => "text",
    };
    let enter = match options.ime_action {
        ImeAction::Go => "go",
        ImeAction::Search => "search",
        ImeAction::Send => "send",
        ImeAction::Previous => "previous",
        ImeAction::Next => "next",
        ImeAction::Done => "done",
        ImeAction::None | ImeAction::Default | ImeAction::Unspecified if options.single_line => "done",
        _ => "enter",
    };
    let password = matches!(
        options.keyboard_type,
        KeyboardType::Password
            | KeyboardType::NumberPassword
            | KeyboardType::DecimalPassword
            | KeyboardType::NumberPasswordSigned
            | KeyboardType::DecimalPasswordSigned
    );
    let capitalize = match options.capitalization {
        KeyboardCapitalization::Characters => "characters",
        KeyboardCapitalization::Words => "words",
        KeyboardCapitalization::Sentences => "sentences",
        _ => "off",
    };
    let correct = if options.auto_correct && !password { "on" } else { "off" };
    format!("{mode} {enter} {capitalize} {correct}")
}

/// A cursor's CSS name.
pub(super) fn cursor_name(icon: PointerIcon) -> &'static str {
    match icon {
        PointerIcon::Default => "default",
        PointerIcon::Hand => "pointer",
        PointerIcon::Text | PointerIcon::Handwriting => "text",
        PointerIcon::Crosshair => "crosshair",
        PointerIcon::Move => "move",
        PointerIcon::ResizeHorizontal => "ew-resize",
        PointerIcon::ResizeVertical => "ns-resize",
        PointerIcon::NotAllowed => "not-allowed",
        PointerIcon::Wait => "wait",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_state_and_what_changed() {
        let a = PageState::parse("width=440\nheight=860\nratio=2\ndark=0\nfocused=1\nvisible=1\nfine=1\nhash=places");
        assert_eq!((a.width, a.height, a.ratio, a.hash.as_str()), (440.0, 860.0, 2.0, "places"));
        let b = PageState::parse(
            "width=440\nheight=800\nratio=2\ndark=1\nfocused=0\nvisible=1\nfine=0\ncoarse=1\nsafe=0,20,0,10",
        );
        assert_eq!(b.safe, [0.0, 20.0, 0.0, 10.0]);
        assert_eq!(b.changes(&a), [Event::Resized, Event::MetricsChanged, Event::DevicesChanged, Event::FocusLost]);
        assert_eq!(a.changes(&a), []);
        let hidden = PageState { visible: false, ..a.clone() };
        assert_eq!(hidden.changes(&a), [Event::Hidden]);
    }

    #[test]
    fn keyboards_for_fields() {
        let mut o =
            ImeOptions { keyboard_type: KeyboardType::Number, ime_action: ImeAction::Search, ..ImeOptions::default() };
        assert_eq!(keyboard_kind(&o), "numeric search off on");
        o.keyboard_type = KeyboardType::Password;
        o.ime_action = ImeAction::Unspecified;
        o.capitalization = KeyboardCapitalization::Sentences;
        assert_eq!(keyboard_kind(&o), "text done sentences off");
        o.single_line = false;
        assert_eq!(keyboard_kind(&o), "text enter sentences off");
    }
}
