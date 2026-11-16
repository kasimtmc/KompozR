//! Keyboards through xkbcommon: keymaps from rule names (US and Turkish), what keys give with and without Shift,
//! the modifiers (Compose's: Meta apart from Ctrl), the keyboard's events and the text it types, dead keys through
//! the locale's compose table, the keysym table, the key repeat. libxkbcommon and xkeyboard-config's data are
//! needed (and libX11's compose tables); without libxkbcommon the tests are skipped, unless
//! `KOMPOZ_XKB_REQUIRED=1` (CI).

#[cfg(target_os = "linux")]
mod linux {
    use kompoz_platform::Event;
    use kompoz_platform::linux::xkb::{
        ComposeState, ComposeStatus, Context, KeyRepeat, Keyboard, Keymap, RepeatTimer, key_of,
    };
    use kompoz_ui::input::{Key, KeyModifiers};
    use std::time::Duration;

    pub fn context() -> Option<Context> {
        Context::new().map_err(|why| println!("  libxkbcommon: {why}")).ok()
    }

    fn keymap(c: &Context, layout: &str, variant: &str) -> Keymap {
        c.keymap_from_names("evdev", "pc105", layout, variant, "").expect("the keymap compiles")
    }

    fn code(k: &Keymap, name: &str) -> u32 {
        k.key_by_name(name).unwrap_or_else(|| panic!("the keymap has {name}"))
    }

    pub fn keymaps_from_names_give_their_layouts_symbols(c: &Context) {
        let us = keymap(c, "us", "");
        let mut s = us.state().unwrap();
        let a = code(&us, "AC01");
        assert_eq!(a, 38); // (evdev's KEY_A, 30, plus 8)
        assert_eq!(s.key_sym(a), 0x61);
        assert_eq!(s.key_utf8(a), "a");
        let shift = code(&us, "LFSH");
        s.update_key(shift, true);
        assert_eq!((s.key_utf8(a), s.key_utf32(a)), ("A".to_string(), 'A' as u32));
        assert!(s.mod_name_is_active(c"Shift"));
        s.update_key(shift, false);
        assert_eq!(s.key_utf8(a), "a");
        // Turkish Q: dotless ı where US has i, i where US has the apostrophe; ş, ğ, ü, ö, ç.
        let tr = keymap(c, "tr", "");
        let t = tr.state().unwrap();
        let typed: Vec<String> =
            ["AD08", "AC11", "AC10", "AD11", "AD12", "AB08", "AB09"].iter().map(|n| t.key_utf8(code(&tr, n))).collect();
        assert_eq!(typed, ["ı", "i", "ş", "ğ", "ü", "ö", "ç"]);
        assert!(us.key_repeats(a) && !us.key_repeats(shift));
        assert_eq!(us.layouts(), 1);
        assert_eq!(us.key_syms_by_level(code(&us, "AE01"), 0, 1), [0x21]); // (Shift+1: exclam)
        assert!(us.key_by_name("NOPE").is_none());
        // As a compositor hands it over (text, with the NUL Wayland's mapping ends in): the same keymap.
        let text = tr.as_string().expect("the keymap as text");
        assert!(text.starts_with("xkb_keymap {"));
        let handed = c.keymap_from_string(&(text + "\0")).expect("the text compiles");
        let h = handed.state().unwrap();
        assert_eq!(h.key_utf8(code(&handed, "AD08")), "ı");
        assert!(c.keymap_from_string("not a keymap").is_none());
    }

    pub fn the_keyboard_tells_keys_and_types_text(c: &Context) {
        let us = keymap(c, "us", "");
        let (a, one, shift, ctrl, logo, enter) = (
            code(&us, "AC01"),
            code(&us, "AE01"),
            code(&us, "LFSH"),
            code(&us, "LCTL"),
            code(&us, "LWIN"),
            code(&us, "RTRN"),
        );
        let mut k = Keyboard::new(None);
        assert!(k.set_keymap(us));
        let down = |k: &mut Keyboard, key, typing| k.key(key, true, 5, typing).0;
        assert_eq!(
            down(&mut k, a, true),
            [
                Event::KeyDown { key: Key::A, code_point: 'a' as i32, modifiers: KeyModifiers::None, time: 5 },
                Event::Text("a".into())
            ]
        );
        // Not typing (no field takes text): the key alone.
        assert_eq!(down(&mut k, a, false).len(), 1);
        assert_eq!(
            k.key(a, false, 6, true).0,
            [Event::KeyUp { key: Key::A, code_point: 'a' as i32, modifiers: KeyModifiers::None, time: 6 }]
        );
        // Shift+1: the 1 key, typing '!' (its symbol has no key of its own).
        k.update_mask(1, 0, 0, 0); // (Shift's bit in the default keymap)
        assert_eq!(k.modifiers(), KeyModifiers::Shift);
        assert_eq!(
            down(&mut k, one, true),
            [
                Event::KeyDown { key: Key::One, code_point: '!' as i32, modifiers: KeyModifiers::Shift, time: 5 },
                Event::Text("!".into())
            ]
        );
        k.update_mask(0, 0, 0, 0);
        // The modifiers held as the compositor says them (the masks a state gives as the keys go down): Shift, Ctrl
        // (no text typed with it), Meta (the logo key, not Ctrl).
        let mut by_keys = Keyboard::new(None);
        by_keys.set_keymap(keymap(c, "us", ""));
        let mut s = keymap(c, "us", "").state().unwrap();
        s.update_key(shift, true);
        let shift_mask = s.effective_mods();
        s.update_key(shift, false);
        s.update_key(ctrl, true);
        let ctrl_mask = s.effective_mods();
        s.update_key(ctrl, false);
        s.update_key(logo, true);
        let logo_mask = s.effective_mods();
        assert_eq!(shift_mask, 1);
        by_keys.update_mask(shift_mask | ctrl_mask, 0, 0, 0);
        assert_eq!(by_keys.modifiers(), KeyModifiers::Shift | KeyModifiers::Ctrl);
        by_keys.update_mask(ctrl_mask, 0, 0, 0);
        assert_eq!(by_keys.modifiers(), KeyModifiers::Ctrl);
        assert_eq!(down(&mut by_keys, a, true).len(), 1); // (Ctrl+A: a shortcut, no text)
        by_keys.update_mask(logo_mask, 0, 0, 0);
        assert_eq!(by_keys.modifiers(), KeyModifiers::Meta);
        by_keys.update_mask(0, 0, 0, 0);
        // Enter types no text (a control character).
        assert_eq!(down(&mut by_keys, enter, true).len(), 1);
        assert_eq!(by_keys.key_of(enter), Key::Enter);
        // Turkish: ş is no key of Compose's, but it types.
        let mut tr = Keyboard::new(None);
        tr.set_keymap(keymap(c, "tr", ""));
        let s_cedilla = code(tr.keymap().unwrap(), "AC10");
        let events = tr.key(s_cedilla, true, 0, true).0;
        assert_eq!(events[1], Event::Text("ş".into()));
        assert!(matches!(events[0], Event::KeyDown { key: Key::Unknown, code_point: 0x15f, .. }));
    }

    pub fn dead_keys_compose_through_the_locales_table(c: &Context) {
        let table = c.compose_table("en_US.UTF-8").expect("libX11's compose table for en_US.UTF-8");
        let mut s = ComposeState::new(&table).unwrap();
        let dead_acute = c.keysym_from_name("dead_acute").unwrap();
        assert_eq!(dead_acute, 0xfe51);
        assert!(s.feed(dead_acute));
        assert_eq!(s.status(), ComposeStatus::Composing);
        assert!(s.feed(0x65)); // e
        assert_eq!(s.status(), ComposeStatus::Composed);
        assert_eq!(s.utf8(), "é");
        assert_eq!(s.one_sym(), 0xe9); // eacute
        s.reset();
        assert_eq!(s.status(), ComposeStatus::Nothing);
        // A modifier between doesn't break a sequence off; a key that completes nothing does.
        assert!(s.feed(dead_acute));
        assert!(!s.feed(c.keysym_from_name("Shift_L").unwrap()));
        assert_eq!(s.status(), ComposeStatus::Composing);
        assert!(s.feed(c.keysym_from_name("Tab").unwrap()));
        assert_eq!(s.status(), ComposeStatus::Cancelled);
        // The keyboard on US international: ' then e types é, the dead key itself nothing.
        let mut k = Keyboard::new(ComposeState::new(&table));
        k.set_keymap(keymap(c, "us", "intl"));
        let (apostrophe, e) = (code(k.keymap().unwrap(), "AC11"), code(k.keymap().unwrap(), "AD03"));
        let first = k.key(apostrophe, true, 0, true).0;
        assert_eq!(first.len(), 1);
        assert!(matches!(first[0], Event::KeyDown { code_point: 0, .. }));
        k.key(apostrophe, false, 0, true);
        let second = k.key(e, true, 0, true).0;
        assert_eq!(second[1], Event::Text("é".into()));
        assert!(matches!(second[0], Event::KeyDown { key: Key::E, code_point: 0xe9, .. }));
        // Then it types as it is again.
        assert_eq!(k.key(e, true, 0, true).0[1], Event::Text("e".into()));
        // What a keysym types.
        assert_eq!(c.keysym_to_utf8(0x2b9), "ı");
        assert_eq!(c.keysym_to_utf32(0xe7), 'ç' as u32);
        assert_eq!(c.keysym_to_utf8(0xff0d), "\r");
        assert_eq!(c.keysym_to_utf8(dead_acute), "");
    }

    pub fn the_keysym_table_has_every_kind_of_key(c: &Context) {
        let sym = |name: &str| c.keysym_from_name(name).unwrap_or_else(|| panic!("{name}"));
        for (name, key) in [
            ("a", Key::A),
            ("Z", Key::Z),
            ("7", Key::Seven),
            ("F1", Key::F1),
            ("F12", Key::F12),
            ("F13", Key::F13),
            ("F24", Key::F24),
            ("Return", Key::Enter),
            ("KP_Enter", Key::NumPadEnter),
            ("BackSpace", Key::Backspace),
            ("Delete", Key::Delete),
            ("Escape", Key::Escape),
            ("ISO_Left_Tab", Key::Tab),
            ("Home", Key::MoveHome),
            ("End", Key::MoveEnd),
            ("Left", Key::DirectionLeft),
            ("Page_Down", Key::PageDown),
            ("Insert", Key::Insert),
            ("Print", Key::PrintScreen),
            ("Shift_L", Key::ShiftLeft),
            ("Control_R", Key::CtrlRight),
            ("Alt_L", Key::AltLeft),
            ("ISO_Level3_Shift", Key::AltRight),
            ("Super_L", Key::MetaLeft),
            ("Caps_Lock", Key::CapsLock),
            ("Num_Lock", Key::NumLock),
            ("KP_7", Key::NumPad7),
            ("KP_Add", Key::NumPadAdd),
            ("KP_Decimal", Key::NumPadDot),
            ("KP_Home", Key::MoveHome),
            ("space", Key::Spacebar),
            ("comma", Key::Comma),
            ("bracketleft", Key::LeftBracket),
            ("grave", Key::Grave),
            ("XF86AudioPlay", Key::MediaPlayPause),
            ("XF86AudioRaiseVolume", Key::VolumeUp),
            ("XF86AudioMute", Key::VolumeMute),
            ("XF86MonBrightnessUp", Key::BrightnessUp),
            ("XF86Back", Key::Back),
            ("XF86Calculator", Key::Calculator),
            ("XF86Copy", Key::Copy),
            ("Henkan", Key::Henkan),
            ("yen", Key::Yen),
            ("Menu", Key::Menu),
        ] {
            assert_eq!(key_of(sym(name)), key, "{name}");
        }
        for name in ["exclam", "scedilla", "idotless", "dead_acute", "eacute"] {
            assert_eq!(key_of(sym(name)), Key::Unknown, "{name}");
        }
    }

    pub fn a_held_key_repeats_as_the_compositor_says(c: &Context) {
        let us = keymap(c, "us", "");
        let (a, shift) = (code(&us, "AC01"), code(&us, "LFSH"));
        let mut r = KeyRepeat::default();
        let start = r.pressed(Some(&us), a);
        assert_eq!(
            start,
            RepeatTimer::Start { delay: Duration::from_millis(600), interval: Duration::from_millis(40) }
        );
        assert_eq!(r.key(), Some(a));
        assert_eq!(r.pressed(Some(&us), shift), RepeatTimer::Stop); // (a modifier doesn't repeat; the other stops)
        assert_eq!(r.key(), None);
        r.set_info(50, 250);
        assert_eq!(
            r.pressed(Some(&us), a),
            RepeatTimer::Start { delay: Duration::from_millis(250), interval: Duration::from_millis(20) }
        );
        assert_eq!(r.released(shift), RepeatTimer::Keep);
        assert_eq!(r.released(a), RepeatTimer::Stop);
        r.set_info(0, 250);
        assert_eq!(r.pressed(Some(&us), a), RepeatTimer::Stop); // (repeating off)
        // The keyboard's repeats: the held key pressed again for each.
        let mut k = Keyboard::new(None);
        k.set_keymap(us);
        k.repeat().set_info(30, 300);
        let (_, timer) = k.key(a, true, 0, true);
        assert!(matches!(timer, RepeatTimer::Start { .. }));
        let again = k.repeated(2, 9, true);
        assert_eq!(again.len(), 4);
        assert_eq!(again[3], Event::Text("a".into()));
        assert_eq!(k.leave(), RepeatTimer::Stop);
        assert!(k.repeated(1, 9, true).is_empty());
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        use kompoz_platform::linux::xkb::Context;
        let Some(c) = linux::context() else {
            println!("skipped: no libxkbcommon");
            let required = std::env::var_os("KOMPOZ_XKB_REQUIRED").is_some_and(|v| v == "1");
            std::process::exit(if required { 1 } else { 0 });
        };
        let tests: &[(&str, fn(&Context))] = &[
            ("keymaps_from_names_give_their_layouts_symbols", linux::keymaps_from_names_give_their_layouts_symbols),
            ("the_keyboard_tells_keys_and_types_text", linux::the_keyboard_tells_keys_and_types_text),
            ("dead_keys_compose_through_the_locales_table", linux::dead_keys_compose_through_the_locales_table),
            ("the_keysym_table_has_every_kind_of_key", linux::the_keysym_table_has_every_kind_of_key),
            ("a_held_key_repeats_as_the_compositor_says", linux::a_held_key_repeats_as_the_compositor_says),
        ];
        let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
        let mut failed = 0;
        for (name, test) in tests {
            if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                continue;
            }
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&c))).is_ok();
            failed += usize::from(!ok);
            println!("{} {name}", if ok { "ok    " } else { "FAILED" });
        }
        if failed > 0 {
            std::process::exit(1);
        }
    }
}
