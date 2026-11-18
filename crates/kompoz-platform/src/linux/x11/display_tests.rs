//! The X11 platform against a real X server (`KOMPOZ_DISPLAY_TESTS=1`, `DISPLAY` naming one: Xwayland on a Wayland
//! desktop, Xvfb in CI; else they pass without running): the clipboard between two apps' windows (each with a
//! connection and a thread of its own: the owner answers while the reader waits) as UTF-8, Latin-1, its targets and
//! in INCR parts both ways, ownership lost to another; a drop from another client (an XDND 5 source written here)
//! with its status and finish (once the core says a target took it); the keyboard's keys and text through
//! xkbcommon-x11, and through the input method's context; the input context's preedit style (on the spot where the
//! method can). The windows are never mapped. (Drags of the app's own out to others: tests/drag_display.rs.)

use super::ffi::{self, Decoded, Display, InputMethod, KeyEvent, NONE, Window, XEvent};
use super::selection::{Encoding, decode};
use super::{X11Platform, sys};
use crate::app::Event;
use kompoz_ui::input::{Key, KeyModifiers};
use std::ffi::c_long;
use std::sync::mpsc;
use std::time::Duration;

fn enabled() -> bool {
    std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_some_and(|v| v == "1")
}

/// A platform on the display with a window of its own, not shown.
fn platform() -> X11Platform {
    let p = X11Platform::connect().expect("an X display (KOMPOZ_DISPLAY_TESTS=1)");
    let window = p.display().create_window(100, 100, ffi::PROPERTY_CHANGE_MASK).expect("a window");
    p.shared.window.set(window);
    p
}

/// The first event `matches` takes within five seconds.
fn wait(d: &Display, mut matches: impl FnMut(&XEvent) -> bool) -> Option<XEvent> {
    let deadline = sys::monotonic_now() + 5_000_000_000;
    d.flush();
    while sys::monotonic_now() < deadline {
        if let Some(e) = d.check_if_event(&mut matches) {
            return Some(e);
        }
        let _ = sys::poll(&mut [sys::PollFd::readable(d.connection_fd())], Some(Duration::from_millis(20)));
    }
    None
}

/// What the clipboard's owner is told to do.
enum Owner {
    Set(String),
    /// Answer requests until told otherwise; then say whether it still owns the clipboard.
    Stop,
}

#[test]
fn the_clipboard_goes_between_windows() {
    if !enabled() {
        return;
    }
    let (commands, received) = mpsc::channel::<Owner>();
    let (told, replies) = mpsc::channel::<bool>();
    let owner = std::thread::spawn(move || {
        let mut a = platform();
        loop {
            match received.try_recv() {
                Ok(Owner::Set(text)) => {
                    a.shared.write_clipboard(&text);
                    told.send(a.shared.clipboard.borrow().owned).unwrap();
                }
                Ok(Owner::Stop) => break,
                Err(_) => {}
            }
            a.pump();
            a.wait(Some(Duration::from_millis(5)));
        }
        a.pump();
        told.send(a.shared.clipboard.borrow().owned).unwrap();
    });
    let b = platform();
    let set = |text: &str| {
        commands.send(Owner::Set(text.to_string())).unwrap();
        assert!(replies.recv().unwrap(), "the owner owns the clipboard");
    };

    // Text with letters beyond Latin-1, as UTF8_STRING.
    set("Sayaç: ğüşıöç €");
    assert_eq!(b.shared.read_clipboard().as_deref(), Some("Sayaç: ğüşıöç €"));
    // What it can be had as; as STRING, Latin-1 (what Latin-1 lacks as '?').
    let a = b.shared.atoms;
    let (targets, kind) = b.shared.convert(a.CLIPBOARD, a.TARGETS, ffi::CURRENT_TIME).expect("the targets");
    assert_eq!(kind, ffi::XA_ATOM);
    let (longs, _) = targets.as_chunks::<{ size_of::<c_long>() }>();
    let targets: Vec<u64> = longs.iter().map(|&l| c_long::from_ne_bytes(l) as u64).collect();
    for t in [a.TARGETS, a.UTF8_STRING, ffi::XA_STRING, a.TEXT, a.TEXT_PLAIN_UTF8, a.TEXT_PLAIN] {
        assert!(targets.contains(&t), "{t} among the targets");
    }
    let (latin1, kind) = b.shared.convert(a.CLIPBOARD, ffi::XA_STRING, ffi::CURRENT_TIME).expect("as STRING");
    assert_eq!((decode(&latin1, Encoding::Latin1).as_str(), kind), ("Sayaç: ?ü??öç ?", ffi::XA_STRING));
    let (text, kind) = b.shared.convert(a.CLIPBOARD, a.TEXT, ffi::CURRENT_TIME).expect("as TEXT");
    assert_eq!((String::from_utf8(text).unwrap().as_str(), kind), ("Sayaç: ğüşıöç €", a.UTF8_STRING));
    assert!(b.shared.convert(a.CLIPBOARD, a.XdndActionCopy, ffi::CURRENT_TIME).is_none()); // (not a target)

    // A text larger than a request carries: in INCR parts, sent and read.
    let large: String = (0..300_000).map(|i| char::from(b'a' + (i % 26) as u8)).chain("ş".chars()).collect();
    assert!(large.len() > super::part_size(b.display().max_request_bytes()));
    set(&large);
    assert_eq!(b.shared.read_clipboard().as_deref(), Some(large.as_str()));

    // The reader puts its own text there: the owner loses it, and the reader reads its own.
    b.shared.write_clipboard("benim");
    assert_eq!(b.shared.read_clipboard().as_deref(), Some("benim"));
    commands.send(Owner::Stop).unwrap();
    assert!(!replies.recv().unwrap(), "the owner was told it lost the clipboard");
    owner.join().unwrap();
}

#[test]
fn drops_come_from_another_client() {
    if !enabled() {
        return;
    }
    let mut t = platform();
    let target = t.window();
    // The drag's source: another client, on its own thread, offering two files.
    let source = std::thread::spawn(move || {
        let d = Display::open().unwrap();
        let names = [
            c"XdndEnter",
            c"XdndPosition",
            c"XdndStatus",
            c"XdndDrop",
            c"XdndFinished",
            c"XdndSelection",
            c"text/uri-list",
            c"XdndActionCopy",
        ];
        let [enter, position, status, drop, finished, selection, uri_list, copy] =
            d.intern_atoms(&names).try_into().unwrap();
        let src: Window = d.create_window(1, 1, ffi::PROPERTY_CHANGE_MASK).unwrap();
        d.set_selection_owner(selection, src, ffi::CURRENT_TIME);
        let send = |kind, data: [c_long; 5]| d.send_client_message(target, target, kind, data, ffi::NO_EVENT_MASK);
        send(enter, [src as c_long, 5 << 24, uri_list as c_long, 0, 0]);
        send(position, [src as c_long, 0, (50 << 16) | 60, 0, copy as c_long]);
        let message = |kind| {
            let e =
                wait(&d, |e| matches!(e.decode(), Decoded::ClientMessage { message_type, .. } if message_type == kind));
            match e.map(|e| e.decode()) {
                Some(Decoded::ClientMessage { data, .. }) => data,
                _ => panic!("no message {kind} from the target"),
            }
        };
        let status = message(status);
        send(drop, [src as c_long, 0, 1234, 0, 0]);
        let request = wait(&d, |e| matches!(e.decode(), Decoded::SelectionRequest { .. })).expect("the data asked for");
        let Decoded::SelectionRequest { requestor, selection: asked, target: kind, property, time } = request.decode()
        else {
            unreachable!()
        };
        assert_eq!((asked, kind, time), (selection, uri_list, 1234));
        d.set_property_8(
            requestor,
            property,
            uri_list,
            b"file:///tmp/a%20b.png\r\nfile:///home/kasim/G%C3%B6r.txt\r\n",
        );
        d.send_selection_notify(requestor, selection, kind, property, time);
        let finished = message(finished);
        (status, finished, src, copy)
    });
    let mut events = Vec::new();
    let deadline = sys::monotonic_now() + 10_000_000_000;
    while !events.iter().any(|e| matches!(e, Event::DropComplete { .. })) && sys::monotonic_now() < deadline {
        t.pump();
        events.extend(t.take_events());
        t.wait(Some(Duration::from_millis(5)));
    }
    // The core took it (a target did): the finish goes.
    crate::app::Platform::drop_done(&mut t, true);
    let (status, finished, src, copy) = source.join().unwrap();
    assert_eq!(
        events,
        [
            Event::DropBegin { x: 50.0, y: 60.0 },
            Event::DropMove { x: 50.0, y: 60.0 },
            Event::DropFile { x: 50.0, y: 60.0, path: "/tmp/a b.png".into() },
            Event::DropFile { x: 50.0, y: 60.0, path: "/home/kasim/Gör.txt".into() },
            Event::DropComplete { x: 50.0, y: 60.0 },
        ]
    );
    // Taken, each move to be told, copied; then finished, accepted, copied.
    assert_eq!((status[0] as Window, status[1], status[4] as u64), (target, 3, copy));
    assert_eq!((finished[0] as Window, finished[1], finished[2] as u64), (target, 1, copy));
    assert_ne!(src, NONE);
}

#[test]
fn keys_and_text_are_waylands() {
    if !enabled() {
        return;
    }
    let mut p = platform();
    let window = p.window();
    let d = p.shared.display.clone();
    // Without an input method's context: xkbcommon's keys, its text through the compose table.
    let none = InputMethod::new(d.clone());
    let key = |p: &mut X11Platform, ime: &InputMethod, state: u32, press: bool, typing: bool| {
        p.keyboard.key(&d, ime, &KeyEvent::synthetic(&d, window, 38, state, press), 7, typing) // (AC01: A)
    };
    assert!(p.keyboard.xkb.is_some(), "the keymap through xkbcommon-x11");
    let a =
        |code_point: char, modifiers| Event::KeyDown { key: Key::A, code_point: code_point as i32, modifiers, time: 7 };
    assert_eq!(key(&mut p, &none, 0, true, true), [a('a', KeyModifiers::None), Event::Text("a".into())]);
    assert_eq!(key(&mut p, &none, 0, true, false), [a('a', KeyModifiers::None)]);
    assert_eq!(key(&mut p, &none, 1, true, true), [a('A', KeyModifiers::Shift), Event::Text("A".into())]);
    // With Ctrl: no text, the control character (xkbcommon's, as on Wayland).
    assert_eq!(key(&mut p, &none, 4, true, true), [a('\u{1}', KeyModifiers::Ctrl)]);
    assert_eq!(key(&mut p, &none, 0x40, true, true)[0], a('a', KeyModifiers::Meta), "Super is Meta");
    assert!(matches!(key(&mut p, &none, 0, false, true)[..], [Event::KeyUp { key: Key::A, .. }]));
    // Through the input method's context (the desktop's, or X's own), focused: its lookup types.
    let mut ime = InputMethod::new(d.clone());
    if ime.open() {
        ime.create_context(window, ffi::XPoint { x: 10, y: 20 });
        assert!(ime.has_context(), "a context for the window");
        ime.focus(true, ffi::XPoint { x: 10, y: 30 });
        assert_eq!(key(&mut p, &ime, 1, true, true), [a('A', KeyModifiers::Shift), Event::Text("A".into())]);
        ime.focus(false, ffi::XPoint::default());
        ime.destroy_context();
    }
}

#[test]
fn input_contexts_compose_on_the_spot_where_the_method_can() {
    if !enabled() {
        return;
    }
    let p = platform();
    let d = p.shared.display.clone();
    let mut ime = InputMethod::new(d);
    assert!(ime.open(), "the desktop's input method, or X's own");
    ime.create_context(p.window(), ffi::XPoint { x: 10, y: 20 });
    assert!(ime.has_context(), "a context for the window");
    // On the spot (XIMPreeditCallbacks) if the method has it, else over the spot (XIMPreeditPosition), else its own
    // way (XIMPreeditNothing); the status always its own (XIMStatusNothing).
    let (style, styles) = ime.preedit_styles();
    let has = |preedit: std::ffi::c_ulong| styles.contains(&(preedit | 0x0400));
    let expected = [0x0002, 0x0004].into_iter().find(|&s| has(s)).unwrap_or(0x0008);
    assert_eq!(style, expected, "styles {styles:x?}");
    // Told where the caret is, focused, given up and unfocused: nothing composed, nothing told.
    ime.focus(true, ffi::XPoint { x: 12, y: 40 });
    ime.reset();
    assert_eq!(ime.take_preedit(), None);
    assert_eq!(ime.preedit_text(), "");
    ime.focus(false, ffi::XPoint::default());
    ime.destroy_context();
    assert!(!ime.has_context());
}
