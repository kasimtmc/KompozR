//! `KOMPOZ_SCRIPT` (C++'s Script, the same command language: CI scripts carry over): drives the app without a
//! user, for UI tests and screenshots.
//!
//! ```text
//! KOMPOZ_SCRIPT="click 100 200; type hello; key enter; wait 30; shot out.png; quit"
//! ```
//!
//! Commands, `;` apart (positions in dp, as C++'s; times in frames or ms):
//! `wait n` (frames), `sleep ms` (real time, for other threads), `realtime` (the rest in real time), `click x y`,
//! `rclick x y`, `mclick x y`, `down x y`, `up x y`, `move x y`, `fdown id x y`, `fmove id x y`, `fup id x y`,
//! `drag x1 y1 x2 y2`, `mdrag x1 y1 x2 y2` (with the mouse), `swipe x1 y1 x2 y2 frames`, `wheel x y notches`,
//! `zoom x y factor`, `dropfile x y path`, `droptext x y text`,
//! `window focuslost|focusgained|minimized|restored|close`, `type text`, `key [ctrl+][shift+][alt+]name` (enter,
//! backspace, escape, tab, space, up, down, left, right, home, end, delete, pageup, pagedown, a letter, a digit,
//! f1–f24), `resize w h` (window points), `shot path.png`, `quit`; `@Title: command` runs `command` in the app's
//! window of that title (a `Window`'s), at its next frame. Time is virtual while a script runs (16.7 ms a frame), so
//! animations are deterministic (the app's windows' frames follow it); then it goes on in real time.

use crate::keys::{DIGITS, FUNCTION_KEYS, LETTERS};
use kompoz_graphics::Offset;
use kompoz_ui::input::{Key, KeyModifiers};

/// A script's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct Script {
    pub commands: Vec<String>,
    pub next: usize,
    pub wait_frames: i32,
    /// `sleep`: real time going by (the platform's clock), for work on other threads.
    pub sleep_until: i64,
    /// Its virtual clock.
    pub clock: i64,
    /// A swipe in progress: one pointer move a frame, so it has a real velocity.
    pub swipe_from: Offset,
    pub swipe_to: Offset,
    pub swipe_frames: i32,
    pub swipe_step: i32,
    /// `realtime`: from there on the clock is real and frames are paced as without a script.
    pub realtime: bool,
}

impl Script {
    /// The script `text` (its commands, `;` apart, blank ones left out).
    pub fn parse(text: &str) -> Script {
        let commands = text.split(';').filter(|c| !c.trim().is_empty()).map(str::to_string).collect();
        Script { commands, ..Script::default() }
    }

    pub fn enabled(&self) -> bool {
        !self.commands.is_empty()
    }

    pub fn running(&self) -> bool {
        self.next < self.commands.len()
            || self.wait_frames > 0
            || self.sleep_until > 0
            || self.swipe_step < self.swipe_frames
    }
}

/// A window event a script sends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WindowEvent {
    FocusLost,
    FocusGained,
    Minimized,
    Restored,
    /// The user closing it (the window manager's close).
    Close,
}

/// A script's command.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Command {
    Wait(i32),
    Realtime,
    Sleep(i32),
    Click(Offset),
    RightClick(Offset),
    MiddleClick(Offset),
    Down(Offset),
    Up(Offset),
    Move(Offset),
    FingerDown(i64, Offset),
    FingerMove(i64, Offset),
    FingerUp(i64, Offset),
    Drag { from: Offset, to: Offset, mouse: bool },
    Swipe { from: Offset, to: Offset, frames: i32 },
    Wheel(Offset, f32),
    Zoom(Offset, f32),
    DropFile(Offset, String),
    DropText(Offset, String),
    Window(Option<WindowEvent>),
    Type(String),
    Key(Option<(Key, KeyModifiers)>),
    Resize(i32, i32),
    Shot(String),
    Quit,
    Unknown(String),
}

/// A line's words as C++'s `istream >>` reads them: each word's start too, for what takes the rest of the line.
struct Words<'a> {
    line: &'a str,
    at: usize,
}

impl<'a> Words<'a> {
    fn next(&mut self) -> Option<&'a str> {
        let rest = &self.line[self.at..];
        let start = rest.len() - rest.trim_start().len();
        let rest = &rest[start..];
        if rest.is_empty() {
            self.at = self.line.len();
            return None;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        self.at += start + end;
        Some(&rest[..end])
    }

    /// A number (0 where there is none or it isn't one, as a failed `>>` leaves it).
    fn number<T: std::str::FromStr + Default>(&mut self) -> T {
        self.next().and_then(|w| w.parse().ok()).unwrap_or_default()
    }

    fn offset(&mut self) -> Offset {
        let x = self.number();
        let y = self.number();
        Offset::new(x, y)
    }

    /// The rest of the line from its next word on (C++'s `getline(in >> ws, …)`).
    fn rest(&mut self) -> String {
        let rest = self.line[self.at..].trim_start();
        self.at = self.line.len();
        rest.to_string()
    }
}

/// A command's line parsed.
pub(crate) fn parse_command(line: &str) -> Option<Command> {
    let mut w = Words { line, at: 0 };
    let name = w.next()?;
    Some(match name {
        "wait" => Command::Wait(w.number()),
        "realtime" => Command::Realtime,
        "sleep" => Command::Sleep(w.number()),
        "click" => Command::Click(w.offset()),
        "rclick" => Command::RightClick(w.offset()),
        "mclick" => Command::MiddleClick(w.offset()),
        "down" => Command::Down(w.offset()),
        "up" => Command::Up(w.offset()),
        "move" => Command::Move(w.offset()),
        "fdown" | "fmove" | "fup" => {
            let id = w.number();
            let at = w.offset();
            match name {
                "fdown" => Command::FingerDown(id, at),
                "fmove" => Command::FingerMove(id, at),
                _ => Command::FingerUp(id, at),
            }
        }
        "drag" | "mdrag" => {
            let from = w.offset();
            let to = w.offset();
            Command::Drag { from, to, mouse: name == "mdrag" }
        }
        "swipe" => {
            let from = w.offset();
            let to = w.offset();
            let frames = w.next().and_then(|f| f.parse().ok()).unwrap_or(10);
            Command::Swipe { from, to, frames }
        }
        "wheel" => {
            let at = w.offset();
            Command::Wheel(at, w.number())
        }
        "zoom" => {
            let at = w.offset();
            Command::Zoom(at, w.next().and_then(|f| f.parse().ok()).unwrap_or(1.0))
        }
        "dropfile" | "droptext" => {
            let at = w.offset();
            let what = w.rest();
            if name == "dropfile" { Command::DropFile(at, what) } else { Command::DropText(at, what) }
        }
        "window" => Command::Window(match w.next() {
            Some("focuslost") => Some(WindowEvent::FocusLost),
            Some("focusgained") => Some(WindowEvent::FocusGained),
            Some("minimized") => Some(WindowEvent::Minimized),
            Some("restored") => Some(WindowEvent::Restored),
            Some("close") => Some(WindowEvent::Close),
            _ => None,
        }),
        "type" => Command::Type(w.rest()),
        "key" => Command::Key(w.next().and_then(key_named)),
        "resize" => {
            let width = w.number();
            Command::Resize(width, w.number())
        }
        "shot" => Command::Shot(w.next().unwrap_or("").to_string()),
        "quit" => Command::Quit,
        other => Command::Unknown(other.to_string()),
    })
}

/// A key as a script names it: `ctrl+`, `shift+` and `alt+` in front (any order), then a name, a letter, a digit
/// or f1–f24.
pub(crate) fn key_named(name: &str) -> Option<(Key, KeyModifiers)> {
    let mut modifiers = KeyModifiers::None;
    let mut bare = name;
    'prefixes: loop {
        for (prefix, flag) in
            [("ctrl+", KeyModifiers::Ctrl), ("shift+", KeyModifiers::Shift), ("alt+", KeyModifiers::Alt)]
        {
            if let Some(rest) = bare.strip_prefix(prefix) {
                modifiers = modifiers | flag;
                bare = rest;
                continue 'prefixes;
            }
        }
        break;
    }
    let named = [
        ("enter", Key::Enter),
        ("backspace", Key::Backspace),
        ("escape", Key::Escape),
        ("tab", Key::Tab),
        ("space", Key::Spacebar),
        ("up", Key::DirectionUp),
        ("down", Key::DirectionDown),
        ("left", Key::DirectionLeft),
        ("right", Key::DirectionRight),
        ("home", Key::MoveHome),
        ("end", Key::MoveEnd),
        ("delete", Key::Delete),
        ("pageup", Key::PageUp),
        ("pagedown", Key::PageDown),
    ];
    if let Some(&(_, key)) = named.iter().find(|(n, _)| *n == bare) {
        return Some((key, modifiers));
    }
    let b = bare.as_bytes();
    let key = match b {
        [c @ b'a'..=b'z'] => LETTERS[(c - b'a') as usize],
        [c @ b'0'..=b'9'] => DIGITS[(c - b'0') as usize],
        [b'f', ..] => FUNCTION_KEYS[bare[1..].parse::<usize>().ok().filter(|n| (1..=24).contains(n))? - 1],
        _ => return None,
    };
    Some((key, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_read_as_cpps() {
        let o = Offset::new;
        assert_eq!(parse_command(" click 70 70"), Some(Command::Click(o(70.0, 70.0))));
        assert_eq!(parse_command("wait 30"), Some(Command::Wait(30)));
        assert_eq!(parse_command("wait"), Some(Command::Wait(0)));
        assert_eq!(
            parse_command("swipe 1 2 3 4"),
            Some(Command::Swipe { from: o(1.0, 2.0), to: o(3.0, 4.0), frames: 10 })
        );
        assert_eq!(parse_command("fdown 2 10.5 20"), Some(Command::FingerDown(2, o(10.5, 20.0))));
        assert_eq!(parse_command("type  merhaba dünya "), Some(Command::Type("merhaba dünya ".into())));
        assert_eq!(
            parse_command("dropfile 5 6 /tmp/a b.png"),
            Some(Command::DropFile(o(5.0, 6.0), "/tmp/a b.png".into()))
        );
        assert_eq!(parse_command("shot out.png"), Some(Command::Shot("out.png".into())));
        assert_eq!(parse_command("window minimized"), Some(Command::Window(Some(WindowEvent::Minimized))));
        assert_eq!(parse_command("zoom 1 2"), Some(Command::Zoom(o(1.0, 2.0), 1.0)));
        assert_eq!(parse_command("dance"), Some(Command::Unknown("dance".into())));
        assert_eq!(parse_command("   "), None);
        let script = Script::parse("wait 30;click 70 70; ;shot shot.png;quit");
        assert_eq!(script.commands.len(), 4);
        assert!(script.enabled() && script.running());
    }

    #[test]
    fn keys_are_named_as_cpps() {
        assert_eq!(key_named("enter"), Some((Key::Enter, KeyModifiers::None)));
        assert_eq!(key_named("ctrl+shift+a"), Some((Key::A, KeyModifiers::Ctrl | KeyModifiers::Shift)));
        assert_eq!(key_named("alt+7"), Some((Key::Seven, KeyModifiers::Alt)));
        assert_eq!(key_named("f12"), Some((Key::F12, KeyModifiers::None)));
        assert_eq!(key_named("f24"), Some((Key::F24, KeyModifiers::None)));
        assert_eq!(key_named("home"), Some((Key::MoveHome, KeyModifiers::None)));
        assert_eq!(key_named("f25"), None);
        assert_eq!(key_named("ctrl+"), None);
        assert_eq!(key_named("ab"), None);
    }
}
