//! Keyboards on Linux through xkbcommon (our own `dlopen` FFI, `ffi`): keymaps (Wayland's handed over as text,
//! X11's from the server, tests' from rule names), keyboard states, compose sequences (dead keys and the Compose
//! key, from the locale's table: what Wayland's and X11-without-XIM typing goes through), the keysym table
//! ([`key_of`]), and the two the platforms share: [`Keyboard`] (a key pressed or let go as KompozR's events, with the
//! text it types) and [`KeyRepeat`] (a held key repeated client-side, as Wayland asks).
//!
//! Modifiers are Compose's: Shift, Ctrl, Alt and Meta (the Super/Logo key) apart. (C++ took Logo as Ctrl, for
//! macOS-style shortcuts on Linux; Compose's key events keep them apart and so does KompozR.)

mod ffi;
mod keysyms;

pub use ffi::{ComposeTable, Context, KeyboardState, Keymap};
pub use keysyms::key_of;

use crate::app::Event;
use kompoz_ui::input::{Key, KeyModifiers};
use std::time::Duration;

/// Where a compose sequence is (xkbcommon's `xkb_compose_status`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ComposeStatus {
    /// None under way.
    Nothing,
    /// Begun (a dead key, the Compose key): what comes next may complete it.
    Composing,
    /// Complete: its text is ready.
    Composed,
    /// Broken off by a keysym that completes nothing.
    Cancelled,
}

/// Typing through a compose table.
pub struct ComposeState(ffi::RawComposeState);

impl ComposeState {
    /// A state on `table`.
    pub fn new(table: &ComposeTable) -> Option<ComposeState> {
        table.state().map(ComposeState)
    }

    /// Feeds `keysym` in; whether a sequence took it (else it types as it is).
    pub fn feed(&mut self, keysym: u32) -> bool {
        self.0.feed(keysym)
    }

    pub fn status(&self) -> ComposeStatus {
        match self.0.status() {
            1 => ComposeStatus::Composing,
            2 => ComposeStatus::Composed,
            3 => ComposeStatus::Cancelled,
            _ => ComposeStatus::Nothing,
        }
    }

    /// The composed text (while composed).
    pub fn utf8(&self) -> String {
        self.0.utf8()
    }

    /// The composed keysym (while composed; 0 if none or several).
    pub fn one_sym(&self) -> u32 {
        self.0.one_sym()
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }
}

/// The locale compose tables are for, as the C library would take it (`LC_ALL`, `LC_CTYPE`, `LANG`; "C" if none).
pub fn compose_locale() -> String {
    for name in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(v) = std::env::var(name)
            && !v.is_empty()
        {
            return v;
        }
    }
    "C".into()
}

/// What to do with the key-repeat timer after a key event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepeatTimer {
    /// Start it (again): the first repeat after `delay`, then every `interval`.
    Start { delay: Duration, interval: Duration },
    /// Stop it.
    Stop,
    /// Leave it as it is.
    Keep,
}

/// A held key repeated client-side (Wayland's `wl_keyboard.repeat_info`: `rate` a second after `delay` ms; X11's
/// server repeats itself): which key repeats and the timer it needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyRepeat {
    rate: i32,
    delay_ms: i32,
    key: Option<u32>,
}

impl Default for KeyRepeat {
    /// 25 a second after 600 ms (Wayland's defaults, until the compositor says).
    fn default() -> Self {
        KeyRepeat { rate: 25, delay_ms: 600, key: None }
    }
}

impl KeyRepeat {
    /// The compositor's rate (repeats a second; 0: none) and delay (ms).
    pub fn set_info(&mut self, rate: i32, delay_ms: i32) {
        self.rate = rate;
        self.delay_ms = delay_ms;
    }

    /// The key repeating now (an X keycode), if any.
    pub fn key(&self) -> Option<u32> {
        self.key
    }

    /// `key` pressed: it repeats if the keymap says it does and repeating is on; else none does.
    pub fn pressed(&mut self, keymap: Option<&Keymap>, key: u32) -> RepeatTimer {
        if self.rate > 0 && keymap.is_some_and(|k| k.key_repeats(key)) {
            self.key = Some(key);
            RepeatTimer::Start {
                delay: Duration::from_millis(self.delay_ms.max(0) as u64),
                interval: Duration::from_nanos(1_000_000_000 / self.rate as u64),
            }
        } else {
            self.stop()
        }
    }

    /// `key` let go: the repeat stops if it was the one repeating.
    pub fn released(&mut self, key: u32) -> RepeatTimer {
        if self.key == Some(key) { self.stop() } else { RepeatTimer::Keep }
    }

    /// No key repeats any more (the keyboard left the window).
    pub fn stop(&mut self) -> RepeatTimer {
        self.key = None;
        RepeatTimer::Stop
    }
}

/// A keyboard (C++'s Wayland and X11 key handling): a keymap and its state, the compose sequence under way, the
/// modifiers held, the repeating key. A key's event is its [`key_of`] key (the symbol it gives, or, for a symbol
/// with no key of its own, its unshifted one: Shift+1 is `Key::One` typing '!'), the character it types, the
/// modifiers; while a field takes typing, its text too, through the compose table where there is one.
pub struct Keyboard {
    keymap: Option<Keymap>,
    state: Option<KeyboardState>,
    compose: Option<ComposeState>,
    modifiers: KeyModifiers,
    repeat: KeyRepeat,
}

impl Keyboard {
    /// A keyboard without a keymap yet, typing through `compose` if given.
    pub fn new(compose: Option<ComposeState>) -> Keyboard {
        Keyboard { keymap: None, state: None, compose, modifiers: KeyModifiers::None, repeat: KeyRepeat::default() }
    }

    /// A new keymap (Wayland's `wl_keyboard.keymap`), with a state of its own; false if it has none.
    pub fn set_keymap(&mut self, keymap: Keymap) -> bool {
        let Some(state) = keymap.state() else { return false };
        self.set_keymap_and_state(keymap, state);
        true
    }

    /// A new keymap and the state following it (X11's from the server's device).
    pub fn set_keymap_and_state(&mut self, keymap: Keymap, state: KeyboardState) {
        self.keymap = Some(keymap);
        self.state = Some(state);
        self.update_modifiers();
    }

    pub fn keymap(&self) -> Option<&Keymap> {
        self.keymap.as_ref()
    }

    pub fn state(&self) -> Option<&KeyboardState> {
        self.state.as_ref()
    }

    /// The modifiers held, as the compositor says (Wayland's `wl_keyboard.modifiers`: the group as the locked
    /// layout).
    pub fn update_mask(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        if let Some(s) = &mut self.state {
            s.update_mask(depressed, latched, locked, 0, 0, group);
        }
        self.update_modifiers();
    }

    /// The modifiers held (Compose's: Shift, Ctrl, Alt, Meta).
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    fn update_modifiers(&mut self) {
        let Some(s) = &self.state else {
            self.modifiers = KeyModifiers::None;
            return;
        };
        let mut m = KeyModifiers::None;
        for (name, flag) in [
            (c"Shift", KeyModifiers::Shift),
            (c"Control", KeyModifiers::Ctrl),
            (c"Mod1", KeyModifiers::Alt),
            (c"Mod4", KeyModifiers::Meta),
        ] {
            if s.mod_name_is_active(name) {
                m = m | flag;
            }
        }
        self.modifiers = m;
    }

    /// The key repeat (its rate, the repeating key).
    pub fn repeat(&mut self) -> &mut KeyRepeat {
        &mut self.repeat
    }

    /// The keyboard left the window: nothing held, no compose sequence, no repeat.
    pub fn leave(&mut self) -> RepeatTimer {
        self.modifiers = KeyModifiers::None;
        if let Some(c) = &mut self.compose {
            c.reset();
        }
        self.repeat.stop()
    }

    /// The key of `key` (an X keycode) now: its symbol's, else its unshifted symbol's.
    pub fn key_of(&self, key: u32) -> Key {
        let (Some(state), Some(keymap)) = (&self.state, &self.keymap) else { return Key::Unknown };
        let k = key_of(state.key_sym(key));
        if k != Key::Unknown {
            return k;
        }
        let layout = state.key_layout(key);
        keymap.key_syms_by_level(key, layout, 0).first().map_or(Key::Unknown, |&s| key_of(s))
    }

    /// `key` (an X keycode: evdev's + 8) pressed or let go at `time`: its event, and while `typing` (a field takes
    /// text) what it types, as C++'s: not with Ctrl held, not a control character; through the compose table
    /// (a dead key types nothing, the key completing the sequence its result). Then what the repeat timer is to
    /// do.
    pub fn key(&mut self, key: u32, down: bool, time: i64, typing: bool) -> (Vec<Event>, RepeatTimer) {
        let events = self.key_events(key, down, time, typing);
        let timer = if down { self.repeat.pressed(self.keymap.as_ref(), key) } else { self.repeat.released(key) };
        (events, timer)
    }

    /// The repeating key pressed again (`repeats` times: the timer's expirations), as [`key`](Keyboard::key)'s
    /// events.
    pub fn repeated(&mut self, repeats: u64, time: i64, typing: bool) -> Vec<Event> {
        let Some(key) = self.repeat.key() else { return Vec::new() };
        (0..repeats.min(64)).flat_map(|_| self.key_events(key, true, time, typing)).collect()
    }

    fn key_events(&mut self, key: u32, down: bool, time: i64, typing: bool) -> Vec<Event> {
        let Some(state) = &self.state else { return Vec::new() };
        let sym = state.key_sym(key);
        let k = self.key_of(key);
        let modifiers = self.modifiers;
        if !down {
            let code_point = state.key_utf32(key) as i32;
            return vec![Event::KeyUp { key: k, code_point, modifiers, time }];
        }
        // What it types: through a compose sequence, or as it is.
        let mut text = state.key_utf8(key);
        if let Some(compose) = &mut self.compose
            && compose.feed(sym)
        {
            match compose.status() {
                ComposeStatus::Composing | ComposeStatus::Cancelled => text.clear(),
                ComposeStatus::Composed => text = compose.utf8(),
                ComposeStatus::Nothing => {}
            }
            if matches!(compose.status(), ComposeStatus::Composed | ComposeStatus::Cancelled) {
                compose.reset();
            }
        }
        let code_point = text.chars().next().map_or(0, |c| c as i32);
        let mut events = vec![Event::KeyDown { key: k, code_point, modifiers, time }];
        let printable = text.as_bytes().first().is_some_and(|&b| b >= 0x20 && b != 0x7f);
        if typing && !modifiers.contains(KeyModifiers::Ctrl) && printable {
            events.push(Event::Text(text));
        }
        events
    }
}
