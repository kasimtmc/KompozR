//! The keyboard on X11 (C++'s `key`): the server's core key events (what the input method filters first), the keys
//! told as on Wayland — through xkbcommon's keymap of the core keyboard (xkbcommon-x11), its state set from each
//! event's modifiers and group, so that the keys, the characters they give and the modifiers are Wayland's — and the
//! text a key types while a field takes it: through the input method's context where there is one (XIM: its
//! compose keys and commits, `Xutf8LookupString`), else through the locale's compose table (dead keys and the
//! Compose key without XIM, as on Wayland). Held keys repeat as the server repeats them (detectable: presses alone).
//! Without xkbcommon-x11, Xlib's own keysyms and Latin-1 text (C++'s way).

use super::ffi::{Display, InputMethod, KeyEvent};
use super::input::modifiers_of;
use crate::app::Event;
use crate::linux::xkb::{ComposeState, Context, Keyboard, compose_locale, key_of};
use kompoz_ui::input::KeyModifiers;

/// The keyboard: xkbcommon's where it can be had.
pub struct X11Keyboard {
    pub(super) xkb: Option<(Context, Keyboard)>,
}

impl X11Keyboard {
    /// The core keyboard's keymap through xkbcommon-x11, typing through the locale's compose table; Xlib's alone if
    /// it can't be had.
    pub fn new(display: &Display) -> X11Keyboard {
        let xkb = Context::new().ok().and_then(|context| {
            let (keymap, state) = display.keyboard_keymap(&context)?;
            let compose = context.compose_table(&compose_locale()).and_then(|t| ComposeState::new(&t));
            let mut keyboard = Keyboard::new(compose);
            keyboard.set_keymap_and_state(keymap, state);
            keyboard.repeat().set_info(0, 0); // (the server repeats)
            Some((context, keyboard))
        });
        X11Keyboard { xkb }
    }

    /// The keymap changed (`MappingNotify`): read again.
    pub fn refresh(&mut self, display: &Display) {
        if let Some((context, keyboard)) = &mut self.xkb
            && let Some((keymap, state)) = display.keyboard_keymap(context)
        {
            keyboard.set_keymap_and_state(keymap, state);
        }
    }

    /// The focus left the window: no compose sequence goes on.
    pub fn leave(&mut self) {
        if let Some((_, keyboard)) = &mut self.xkb {
            keyboard.leave();
        }
    }

    /// `key` pressed (or repeated) or let go at `time`: its event (none for a press of no key: an input method's
    /// commit), and while `typing` (a field takes text) what it types — through `ime`'s context if it has one —, as
    /// C++'s: not with Ctrl held, not a control character.
    pub fn key(&mut self, display: &Display, ime: &InputMethod, key: &KeyEvent, time: i64, typing: bool) -> Vec<Event> {
        let down = key.is_press();
        let modifiers = modifiers_of(key.state());
        let through_ime = ime.has_context();
        let mut events = Vec::new();
        if key.keycode() != 0 {
            match &mut self.xkb {
                Some((_, keyboard)) => {
                    // The state as the event has it: its modifiers (all as depressed: in effect), its group (bits 13
                    // and 14, XkbGroupForCoreState) as the locked layout.
                    let state = key.state();
                    keyboard.update_mask(state & 0xff, 0, 0, (state >> 13) & 3);
                    let (keyed, _) = keyboard.key(key.keycode(), down, time, typing && !through_ime);
                    events.extend(keyed);
                }
                None => {
                    let k = key_of(display.lookup_keysym(key, 0) as u32);
                    let typed = if down && !through_ime { display.lookup_latin1(key) } else { String::new() };
                    let code_point = typed.chars().next().map_or(0, |c| c as i32);
                    events.push(if down {
                        Event::KeyDown { key: k, code_point, modifiers, time }
                    } else {
                        Event::KeyUp { key: k, code_point, modifiers, time }
                    });
                    if typing && !through_ime && down {
                        events.extend(typed_text(typed, modifiers));
                    }
                }
            }
        }
        if typing && through_ime && down {
            events.extend(ime.lookup(key).and_then(|t| typed_text(t, modifiers)));
        }
        events
    }
}

/// `text` as typed text if it is (C++'s: not with Ctrl held, not a control character).
fn typed_text(text: String, modifiers: KeyModifiers) -> Option<Event> {
    let printable = text.as_bytes().first().is_some_and(|&b| b >= 0x20 && b != 0x7f);
    (printable && !modifiers.contains(KeyModifiers::Ctrl)).then_some(Event::Text(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_printable_text_without_ctrl_is_typed() {
        assert_eq!(typed_text("ş".into(), KeyModifiers::Shift), Some(Event::Text("ş".into())));
        assert_eq!(typed_text("a".into(), KeyModifiers::Ctrl), None);
        assert_eq!(typed_text("\u{8}".into(), KeyModifiers::None), None);
        assert_eq!(typed_text("\u{7f}".into(), KeyModifiers::None), None);
        assert_eq!(typed_text(String::new(), KeyModifiers::None), None);
        assert_eq!(typed_text("€".into(), KeyModifiers::Alt), Some(Event::Text("€".into())));
    }
}
