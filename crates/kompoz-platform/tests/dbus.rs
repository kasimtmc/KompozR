//! Our D-Bus client and what is made of it, on a D-Bus of the tests' own (a dbus-daemon started for them on an
//! abstract socket: nothing of the desktop's is touched), as C++'s platform tests: connections (authentication,
//! calls, errors, timeouts, signals, serving), the settings portal (a fake one: ReadOne, an older portal's Read,
//! SettingChanged), the desktop services' events, and the AT-SPI bridge, where the tests play the AT-SPI registry,
//! the accessibility bus's launcher and a screen reader; the tray icons (StatusNotifierItem and dbusmenu) and the
//! notifications, where the tests play the tray's StatusNotifierWatcher, its host and the notifications' server.
//! One thread, in sequence.
//!
//! Without dbus-daemon they are skipped, unless `KOMPOZ_DBUS_REQUIRED=1` (CI) makes that a failure.

#[cfg(target_os = "linux")]
mod linux {
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::{Color, IntSize, Rect, UnitExt};
    use kompoz_platform::app::{Event, TrayIcon, TrayImage, TrayMenuItem, WindowButton};
    use kompoz_platform::linux::atspi::{AtspiBridge, AtspiOptions, Request, Window};
    use kompoz_platform::linux::dbus::message::NO_REPLY_EXPECTED;
    use kompoz_platform::linux::dbus::{Connection, Error, MatchRule, Message, MessageType, Type, Value};
    use kompoz_platform::linux::desktop::{DesktopOptions, DesktopServices, Notify};
    use kompoz_platform::linux::portal::{DesktopSettings, PortalOptions};
    use kompoz_platform::linux::status_notifier::StatusNotifier;
    use kompoz_platform::linux::wallpaper::WallpaperOptions;
    use kompoz_runtime::*;
    use kompoz_ui::input::{Key as UiKey, KeyModifiers};
    use kompoz_ui::semantics::{AccessibilityNode, LiveRegionMode, ProgressBarRangeInfo, Role, SemanticsAction};
    use kompoz_ui::window::{MenuItemKind, Notification, NotificationType};
    use kompoz_ui::{KompozHost, Modifier};
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_secs(5);

    /// A dbus-daemon of the tests' own, on an abstract socket, stopped (and its directory gone) when dropped.
    pub struct TestBus {
        child: Child,
        pub address: String,
        dir: PathBuf,
    }

    impl TestBus {
        /// One started; none without dbus-daemon (a failure with KOMPOZ_DBUS_REQUIRED=1).
        pub fn start() -> Option<TestBus> {
            static COUNT: AtomicUsize = AtomicUsize::new(0);
            let name = format!("kompoz-platform-tests-{}-{}", std::process::id(), COUNT.fetch_add(1, Ordering::SeqCst));
            let address = format!("unix:abstract={name}");
            let dir = std::env::temp_dir().join(&name);
            std::fs::create_dir_all(&dir).unwrap();
            let config = dir.join("bus.conf");
            std::fs::write(
                &config,
                format!(
                    "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN\" \
                     \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n\
                     <busconfig><type>session</type><listen>{address}</listen><auth>EXTERNAL</auth>\
                     <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
                     <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>\n"
                ),
            )
            .unwrap();
            let spawned = Command::new("dbus-daemon")
                .arg(format!("--config-file={}", config.display()))
                .args(["--nofork", "--nopidfile"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match spawned {
                Ok(child) => Some(TestBus { child, address, dir }),
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&dir);
                    if std::env::var_os("KOMPOZ_DBUS_REQUIRED").is_some_and(|v| v == "1") {
                        panic!("no dbus-daemon ({e}), and KOMPOZ_DBUS_REQUIRED=1");
                    }
                    println!("  (no dbus-daemon: skipped)");
                    None
                }
            }
        }

        /// A connection to it, once it listens.
        pub fn connect(&self) -> Connection {
            let start = Instant::now();
            loop {
                match Connection::open(&self.address) {
                    Ok(c) => return c,
                    Err(e) if start.elapsed() > TIMEOUT => panic!("the test bus doesn't answer: {e}"),
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        }
    }

    impl Drop for TestBus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Until `done`, at most `timeout`; whether it was.
    fn wait_for(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        done()
    }

    fn counter() -> (Arc<AtomicUsize>, Notify) {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        (
            count,
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        )
    }

    // ---- The tray ----

    /// What the tray's side heard: icons registered (the item's sender and path), notifications (their arguments).
    #[derive(Debug)]
    enum Heard {
        Registered(String, String),
        Notified(Vec<Value>),
    }

    /// The tray's StatusNotifierWatcher and the notifications' server on one connection of the test's own, until
    /// `stop`; what it heard sent.
    fn tray_side(address: String, stop: Arc<AtomicBool>, heard: mpsc::Sender<Heard>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut b = Connection::open(&address).unwrap();
            assert_eq!(b.request_name("org.kde.StatusNotifierWatcher", 4).unwrap(), 1);
            assert_eq!(b.request_name("org.freedesktop.Notifications", 4).unwrap(), 1);
            while !stop.load(Ordering::SeqCst) {
                let Some(m) = b.read(Some(Duration::from_millis(50)), None).unwrap() else { continue };
                if m.kind != MessageType::MethodCall || b.answer_peer(&m).unwrap() {
                    continue;
                }
                match m.member.as_deref().unwrap() {
                    "RegisterStatusNotifierItem" => {
                        let path = m.arg(0).and_then(Value::as_str).unwrap_or("").to_string();
                        heard.send(Heard::Registered(m.sender.clone().unwrap_or_default(), path)).unwrap();
                        b.reply(&m, vec![]).unwrap();
                    }
                    "Notify" => {
                        heard.send(Heard::Notified(m.body.clone())).unwrap();
                        b.reply(&m, vec![Value::Uint32(1)]).unwrap();
                    }
                    _ => b.reply_error(&m, "org.freedesktop.DBus.Error.UnknownMethod", "no").unwrap(),
                }
            }
        })
    }

    /// A tray icon as the app core gives it: a red picture, a tooltip, a menu (an item with a mnemonic and a
    /// shortcut, a checkbox checked, a separator, a submenu).
    fn tray_icon(tooltip: &str) -> TrayIcon {
        let item = |kind, text: &str, items| TrayMenuItem {
            kind,
            text: text.to_string(),
            enabled: true,
            mnemonic: None,
            shortcut: None,
            icon: None,
            items,
        };
        let mut open = item(MenuItemKind::Item, "Open", vec![]);
        open.mnemonic = Some('O');
        open.shortcut = Some((KeyModifiers::Ctrl, kompoz_platform::app::key_name(UiKey::Q)));
        TrayIcon {
            id: 7,
            images: vec![TrayImage { width: 2, height: 1, rgba: vec![255, 0, 0, 255, 255, 0, 0, 255] }],
            tooltip: tooltip.into(),
            menu: vec![
                open,
                item(MenuItemKind::Checkbox { checked: true }, "Check", vec![]),
                item(MenuItemKind::Separator, "", vec![]),
                item(MenuItemKind::Menu, "More", vec![item(MenuItemKind::Item, "Quit", vec![])]),
            ],
        }
    }

    /// The tray icon registered with a watcher that came after it (its NameOwnerChanged), its properties and menu
    /// read by the tray's host as KDE's reads them, its activation and its items' clicks told to the app (by where
    /// they are in the menu), a change signalled (no second registration), its notifications sent (a type's icon by
    /// its name; without one, the icon's picture).
    pub fn tray_icons_and_notifications_on_the_bus() {
        let Some(bus) = TestBus::start() else { return };
        drop(bus.connect()); // (the bus listening)
        let (woken, notify) = counter();
        let tray = StatusNotifier::new("KompozR test", notify, Some(bus.address.clone()));
        tray.sync(&[tray_icon("KompozR")]);
        std::thread::sleep(Duration::from_millis(200));
        assert!(!tray.available(), "no watcher yet");
        let (heard, hear) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let side = tray_side(bus.address.clone(), stop.clone(), heard);
        let Ok(Heard::Registered(sender, path)) = hear.recv_timeout(TIMEOUT) else { panic!("not registered") };
        assert_eq!(path, "/StatusNotifierItem/7");
        assert!(wait_for(TIMEOUT, || tray.available()), "the watcher seen");
        // The host reads the item.
        let mut host = bus.connect();
        let rule = MatchRule::signal().sender(&sender);
        host.add_match(&rule).unwrap();
        let get = |host: &mut Connection, path: &str, iface: &str, name: &str| {
            host.get_property(&sender, path, iface, name, TIMEOUT).unwrap().unwrap_variants().clone()
        };
        let item = "org.kde.StatusNotifierItem";
        let tip = get(&mut host, &path, item, "ToolTip");
        assert_eq!(tip.fields().unwrap()[2].as_str(), Some("KompozR"));
        assert_eq!(get(&mut host, &path, item, "Status").as_str(), Some("Active"));
        let menu_path = match get(&mut host, &path, item, "Menu") {
            Value::ObjectPath(p) => p,
            other => panic!("{other:?}"),
        };
        let pixmaps = get(&mut host, &path, item, "IconPixmap");
        let first = &pixmaps.items().unwrap()[0];
        let [w, h, argb] = first.fields().unwrap() else { panic!("{first:?}") };
        assert_eq!((w.as_i32(), h.as_i32()), (Some(2), Some(1)));
        let bytes: Vec<u8> = argb.items().unwrap().iter().filter_map(Value::as_u8).collect();
        assert_eq!(bytes, [255, 255, 0, 0, 255, 255, 0, 0], "ARGB, network order");
        let all = host
            .call_method(&sender, &path, "org.freedesktop.DBus.Properties", "GetAll", vec![Value::str(item)], TIMEOUT)
            .unwrap();
        assert!(all[0].get("IconPixmap").is_some() && all[0].get("Menu").is_some());
        // Its menu, as dbusmenu lays it out.
        let menu = "com.canonical.dbusmenu";
        let layout_of = |host: &mut Connection| {
            let args = vec![Value::Int32(0), Value::Int32(-1), Value::array(Type::String, vec![])];
            host.call_method(&sender, &menu_path, menu, "GetLayout", args, TIMEOUT).unwrap()
        };
        let layout = layout_of(&mut host);
        assert_eq!(layout[0].as_u32(), Some(1), "its revision");
        let [_, _, children] = layout[1].fields().unwrap() else { panic!() };
        let children: Vec<Value> = children.items().unwrap().iter().map(|c| c.unwrap_variants().clone()).collect();
        assert_eq!(children.len(), 4);
        let props = |i: usize| children[i].fields().unwrap()[1].clone();
        assert_eq!(props(0).get("label").map(|v| v.unwrap_variants().clone()), Some(Value::str("_Open")));
        let shortcut = props(0).get("shortcut").unwrap().unwrap_variants().clone();
        let keys: Vec<&str> = shortcut.items().unwrap()[0].items().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(keys, ["Control", "q"]);
        assert_eq!(props(1).get("toggle-type").map(|v| v.unwrap_variants().clone()), Some(Value::str("checkmark")));
        assert_eq!(props(1).get("toggle-state").map(|v| v.unwrap_variants().clone()), Some(Value::Int32(1)));
        assert_eq!(props(2).get("type").map(|v| v.unwrap_variants().clone()), Some(Value::str("separator")));
        let sub = children[3].fields().unwrap()[2].items().unwrap()[0].unwrap_variants().clone();
        assert_eq!(
            sub.fields().unwrap()[1].get("label").map(|v| v.unwrap_variants().clone()),
            Some(Value::str("Quit"))
        );
        let sub_id = sub.fields().unwrap()[0].as_i32().unwrap();
        // Clicks: an item's, the submenu's item's, the icon's activation.
        let event = |host: &mut Connection, id: i32| {
            let args = vec![Value::Int32(id), Value::str("clicked"), Value::variant(Value::Int32(0)), Value::Uint32(0)];
            host.call_method(&sender, &menu_path, menu, "Event", args, TIMEOUT).unwrap();
        };
        let id_of = |i: usize| children[i].fields().unwrap()[0].as_i32().unwrap();
        event(&mut host, id_of(0));
        event(&mut host, sub_id);
        event(&mut host, id_of(2)); // (a separator: nothing)
        host.call_method(&sender, &path, item, "Activate", vec![Value::Int32(0), Value::Int32(0)], TIMEOUT).unwrap();
        let mut actions = Vec::new();
        assert!(wait_for(TIMEOUT, || {
            actions.extend(tray.take_actions());
            actions.len() >= 3
        }));
        assert!(woken.load(Ordering::SeqCst) > 0, "the platform's loop woken");
        assert_eq!(
            actions,
            [
                Event::TrayMenuItem { tray: 7, path: vec![0] },
                Event::TrayMenuItem { tray: 7, path: vec![3, 0] },
                Event::TrayAction { tray: 7 },
            ]
        );
        // A change: signalled, its menu's revision moved on, not registered again.
        tray.sync(&[tray_icon("KompozR 2")]);
        let mut signals = Vec::new();
        assert!(
            wait_for(TIMEOUT, || {
                while let Ok(Some(m)) = host.read(Some(Duration::from_millis(10)), None) {
                    if m.kind == MessageType::Signal {
                        signals.push(m.member.clone().unwrap_or_default());
                    }
                }
                signals.iter().any(|s| s == "LayoutUpdated") && signals.iter().any(|s| s == "NewToolTip")
            }),
            "{signals:?}"
        );
        assert_eq!(layout_of(&mut host)[0].as_u32(), Some(2));
        let tip = get(&mut host, &path, item, "ToolTip");
        assert_eq!(tip.fields().unwrap()[2].as_str(), Some("KompozR 2"));
        // Notifications: Info's icon by its name; None's the tray icon's picture.
        tray.notify(&tray_icon("KompozR 2"), &Notification::new("Başlık", "İleti", NotificationType::Info));
        tray.notify(&tray_icon("KompozR 2"), &Notification::new("Plain", "text", NotificationType::None));
        let mut notified = Vec::new();
        while notified.len() < 2 {
            match hear.recv_timeout(TIMEOUT) {
                Ok(Heard::Notified(args)) => notified.push(args),
                Ok(other) => panic!("registered again: {other:?}"),
                Err(e) => panic!("no notification: {e}"),
            }
        }
        let info = &notified[0];
        assert_eq!(info[0].as_str(), Some("KompozR test"));
        assert_eq!(info[2].as_str(), Some("dialog-information"));
        assert_eq!((info[3].as_str(), info[4].as_str()), (Some("Başlık"), Some("İleti")));
        assert!(info[6].get("image-data").is_none());
        assert_eq!(info[7], Value::Int32(-1));
        let plain = &notified[1];
        assert_eq!(plain[2].as_str(), Some(""));
        let data = plain[6].get("image-data").expect("the icon's picture").unwrap_variants().clone();
        let f = data.fields().unwrap();
        assert_eq!(
            (f[0].as_i32(), f[1].as_i32(), f[2].as_i32(), f[3].as_bool()),
            (Some(2), Some(1), Some(8), Some(true))
        );
        drop(tray);
        stop.store(true, Ordering::SeqCst);
        side.join().unwrap();
    }

    // ---- The client ----

    pub fn connections_call_serve_and_hear() {
        let Some(bus) = TestBus::start() else { return };
        let mut a = bus.connect();
        assert!(a.unique_name().starts_with(":1."), "{}", a.unique_name());
        let address = bus.address.clone();
        let (ready, is_ready) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let mut b = Connection::open(&address).unwrap();
            assert_eq!(b.request_name("org.kompoz.Test", 4).unwrap(), 1); // (its primary owner)
            ready.send(b.unique_name().to_string()).unwrap();
            let mut quiet_calls = 0;
            while let Some(m) = b.read(Some(TIMEOUT), None).unwrap() {
                if m.kind != MessageType::MethodCall || b.answer_peer(&m).unwrap() {
                    continue;
                }
                match m.member.as_deref().unwrap() {
                    "Echo" => b.reply(&m, m.body.clone()).unwrap(),
                    "Fail" => b.reply_error(&m, "org.kompoz.Error.Failed", "as asked").unwrap(),
                    "Slow" => {
                        std::thread::sleep(Duration::from_millis(300));
                        b.reply(&m, vec![]).unwrap();
                    }
                    "Signal" => {
                        for which in ["other", "mine"] {
                            let s = Message::signal("/org/kompoz/Test", "org.kompoz.Test", "Changed")
                                .with_body(vec![Value::str(which), Value::Uint32(7)]);
                            b.send(&s).unwrap();
                        }
                        b.reply(&m, vec![]).unwrap();
                    }
                    "Quiet" => {
                        assert!(m.no_reply());
                        quiet_calls += 1;
                        b.reply(&m, vec![]).unwrap(); // (not sent: it wants none)
                    }
                    "Quit" => {
                        b.reply(&m, vec![Value::Uint32(quiet_calls)]).unwrap();
                        return;
                    }
                    _ => b.reply_error(&m, "org.freedesktop.DBus.Error.UnknownMethod", "no").unwrap(),
                }
            }
        });
        let b_name = is_ready.recv_timeout(TIMEOUT).unwrap();
        let call = |member: &str| {
            Message::method_call(Some("org.kompoz.Test"), "/org/kompoz/Test", Some("org.kompoz.Test"), member)
        };

        // Every kind of value through the bus and back, a big array among them (read in several pieces).
        let values = vec![
            Value::Byte(1),
            Value::Boolean(true),
            Value::Int16(-3),
            Value::Uint16(3),
            Value::Int32(-70000),
            Value::Uint32(70000),
            Value::Int64(-1 << 50),
            Value::Uint64(1 << 63),
            Value::Double(0.1),
            Value::str("Görüntü"),
            Value::path("/org/kompoz/a_b"),
            Value::Signature("a{sv}(so)".into()),
            Value::variant(Value::Struct(vec![Value::Double(1.0), Value::Double(0.5), Value::Double(0.0)])),
            Value::dict(
                Type::String,
                Type::Variant,
                vec![(Value::str("k"), Value::variant(Value::array(Type::Int64, vec![Value::Int64(5)])))],
            ),
            Value::array(Type::Byte, (0..200_000u32).map(|i| Value::Byte(i as u8)).collect()),
        ];
        let reply = a.call(call("Echo").with_body(values.clone()), TIMEOUT).unwrap();
        assert_eq!(reply.body, values);
        assert_eq!(reply.sender.as_deref(), Some(b_name.as_str()));
        // Errors, by their name.
        match a.call(call("Fail"), TIMEOUT) {
            Err(Error::Remote { name, message }) => {
                assert_eq!((name.as_str(), message.as_str()), ("org.kompoz.Error.Failed", "as asked"))
            }
            other => panic!("{other:?}"),
        }
        let nobody = Message::method_call(Some("org.kompoz.Nobody"), "/", None, "Hm");
        assert_eq!(
            a.call(nobody, TIMEOUT).unwrap_err().remote_name(),
            Some("org.freedesktop.DBus.Error.ServiceUnknown")
        );
        // No reply in time; the late one doesn't stand for the next call's.
        let start = Instant::now();
        assert!(matches!(a.call(call("Slow"), Duration::from_millis(50)), Err(Error::Timeout)));
        assert!(start.elapsed() < Duration::from_millis(250));
        assert!(a.call(call("Echo").with_body(vec![Value::Int32(9)]), TIMEOUT).unwrap().body == vec![Value::Int32(9)]);
        // Peer's methods, answered by every connection.
        let ping = Message::method_call(Some(&b_name), "/", Some("org.freedesktop.DBus.Peer"), "Ping");
        assert!(a.call(ping, TIMEOUT).is_ok());
        // Signals as the match rule says (its arg0: one of the two).
        a.add_match(&MatchRule::signal().interface("org.kompoz.Test").member("Changed").arg(0, "mine")).unwrap();
        a.call(call("Signal"), TIMEOUT).unwrap();
        let mut heard = Vec::new();
        while let Some(m) = a.read(Some(Duration::from_millis(300)), None).unwrap() {
            if m.is_signal("org.kompoz.Test", "Changed") {
                heard.push(m.body.clone());
            }
        }
        assert_eq!(heard, vec![vec![Value::str("mine"), Value::Uint32(7)]]);
        // Calls that want no reply get none.
        a.send(&call("Quiet").with_flags(NO_REPLY_EXPECTED)).unwrap();
        a.send(&call("Quiet").with_flags(NO_REPLY_EXPECTED)).unwrap();
        assert_eq!(a.call(call("Quit"), TIMEOUT).unwrap().body, vec![Value::Uint32(2)]);
        server.join().unwrap();
        // A wake ends a wait for messages.
        let wake = kompoz_platform::linux::sys::eventfd().unwrap();
        kompoz_platform::linux::sys::eventfd_write(std::os::fd::AsFd::as_fd(&wake), 1).unwrap();
        let start = Instant::now();
        while a.read(Some(TIMEOUT), Some(std::os::fd::AsFd::as_fd(&wake))).unwrap().is_some() {}
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    pub fn connections_fail_as_they_should() {
        assert!(matches!(Connection::open("unix:abstract=kompoz-no-such-bus-here"), Err(Error::NoAddress(_))));
        assert!(matches!(Connection::open("tcp:host=localhost,port=1"), Err(Error::NoAddress(_))));
        // A server that refuses the credentials.
        let name = format!("kompoz-refusing-{}", std::process::id());
        let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
        let listener = std::os::unix::net::UnixListener::bind_addr(&address).unwrap();
        let server = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 256];
            let mut got = Vec::new();
            while !got.ends_with(b"\r\n") {
                let n = s.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
            }
            s.write_all(b"REJECTED EXTERNAL\r\n").unwrap();
            String::from_utf8_lossy(&got).into_owned()
        });
        use std::os::linux::net::SocketAddrExt;
        let result = Connection::open(&format!("unix:abstract={name}"));
        assert!(matches!(result, Err(Error::Auth(_))), "{:?}", result.err());
        let uid = std::fs::metadata("/proc/self").map(|m| std::os::unix::fs::MetadataExt::uid(&m)).unwrap();
        let hex: String = uid.to_string().bytes().map(|b| format!("{b:02x}")).collect();
        assert_eq!(server.join().unwrap(), format!("\0AUTH EXTERNAL {hex}\r\n"));
    }

    // ---- The settings portal ----

    /// A portal of the tests' own: `org.freedesktop.portal.Desktop`'s Settings, with ReadOne or (an older one)
    /// only Read, its values changed with SettingChanged.
    struct FakePortal {
        values: Arc<Mutex<HashMap<(String, String), Value>>>,
        signals: mpsc::Sender<Message>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl FakePortal {
        fn start(bus: &TestBus, read_one: bool, values: &[(&str, &str, Value)]) -> FakePortal {
            let values: HashMap<_, _> =
                values.iter().map(|(n, k, v)| ((n.to_string(), k.to_string()), v.clone())).collect();
            let values = Arc::new(Mutex::new(values));
            let stop = Arc::new(AtomicBool::new(false));
            let (signals, to_send) = mpsc::channel::<Message>();
            let (ready, is_ready) = mpsc::channel();
            let mut c = bus.connect();
            let (their_values, their_stop) = (values.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                assert_eq!(c.request_name("org.freedesktop.portal.Desktop", 4).unwrap(), 1);
                ready.send(()).unwrap();
                while !their_stop.load(Ordering::SeqCst) {
                    while let Ok(s) = to_send.try_recv() {
                        c.send(&s).unwrap();
                    }
                    let Some(m) = c.read(Some(Duration::from_millis(10)), None).unwrap() else { continue };
                    if m.kind != MessageType::MethodCall {
                        continue;
                    }
                    let member = m.member.clone().unwrap_or_default();
                    if member == "ReadOne" && !read_one {
                        c.reply_error(&m, "org.freedesktop.DBus.Error.UnknownMethod", "no ReadOne").unwrap();
                        continue;
                    }
                    let key = (
                        m.arg(0).and_then(Value::as_str).unwrap_or("").to_string(),
                        m.arg(1).and_then(Value::as_str).unwrap_or("").to_string(),
                    );
                    match their_values.lock().unwrap().get(&key) {
                        Some(v) if member == "ReadOne" => c.reply(&m, vec![Value::variant(v.clone())]).unwrap(),
                        Some(v) => c.reply(&m, vec![Value::variant(Value::variant(v.clone()))]).unwrap(),
                        None => c.reply_error(&m, "org.freedesktop.portal.Error.NotFound", "no such key").unwrap(),
                    }
                }
            });
            is_ready.recv_timeout(TIMEOUT).unwrap();
            FakePortal { values, signals, stop, thread: Some(thread) }
        }

        /// A value changed, as the desktop changes it.
        fn set(&self, namespace: &str, key: &str, value: Value) {
            self.values.lock().unwrap().insert((namespace.into(), key.into()), value.clone());
            let s =
                Message::signal("/org/freedesktop/portal/desktop", "org.freedesktop.portal.Settings", "SettingChanged")
                    .with_body(vec![Value::str(namespace), Value::str(key), Value::variant(value)]);
            self.signals.send(s).unwrap();
        }
    }

    impl Drop for FakePortal {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(t) = self.thread.take() {
                t.join().unwrap();
            }
        }
    }

    fn rgb(r: f64, g: f64, b: f64) -> Value {
        Value::Struct(vec![Value::Double(r), Value::Double(g), Value::Double(b)])
    }

    fn desktop_values() -> Vec<(&'static str, &'static str, Value)> {
        vec![
            ("org.freedesktop.appearance", "accent-color", rgb(1.0, 0.5, 0.0)),
            ("org.freedesktop.appearance", "color-scheme", Value::Uint32(1)),
            ("org.gnome.desktop.wm.preferences", "button-layout", Value::str("close,minimize:")),
            ("org.gnome.desktop.interface", "text-scaling-factor", Value::Double(1.25)),
        ]
    }

    pub fn portal_settings_are_read_and_followed() {
        let Some(bus) = TestBus::start() else { return };
        for read_one in [true, false] {
            let portal = FakePortal::start(&bus, read_one, &desktop_values());
            let (notified, notify) = counter();
            let options = PortalOptions { address: Some(bus.address.clone()), accent: true };
            let settings = DesktopSettings::new(notify, options);
            assert!(settings.listening());
            assert!(wait_for(TIMEOUT, || settings.text_scaling_factor().is_some()), "read_one: {read_one}");
            assert_eq!(settings.accent(), Some(Color::new(1.0, 0.5, 0.0, 1.0)));
            assert_eq!(settings.dark_scheme(), Some(true));
            assert_eq!(settings.button_layout().as_deref(), Some("close,minimize:"));
            assert_eq!(settings.text_scaling_factor(), Some(1.25));
            assert!(notified.load(Ordering::SeqCst) > 0);
            // Followed as they change: a light preference, no accent, a bigger text.
            let before = notified.load(Ordering::SeqCst);
            portal.set("org.freedesktop.appearance", "color-scheme", Value::Uint32(2));
            portal.set("org.freedesktop.appearance", "accent-color", rgb(-1.0, -1.0, -1.0));
            portal.set("org.gnome.desktop.interface", "text-scaling-factor", Value::Double(1.5));
            portal.set("org.gnome.desktop.interface", "cursor-size", Value::Int32(48)); // (not followed)
            assert!(wait_for(TIMEOUT, || settings.text_scaling_factor() == Some(1.5)));
            assert_eq!(settings.dark_scheme(), Some(false));
            assert_eq!(settings.accent(), None);
            assert!(wait_for(TIMEOUT, || notified.load(Ordering::SeqCst) >= before + 3));
            // The same value again: nothing told.
            let now = notified.load(Ordering::SeqCst);
            portal.set("org.freedesktop.appearance", "color-scheme", Value::Uint32(2));
            std::thread::sleep(Duration::from_millis(200));
            assert_eq!(notified.load(Ordering::SeqCst), now);
            drop(settings);
        }
        // KOMPOZ_NO_DESKTOP_ACCENT: the rest only.
        let _portal = FakePortal::start(&bus, true, &desktop_values());
        let (_, notify) = counter();
        let settings =
            DesktopSettings::new(notify, PortalOptions { address: Some(bus.address.clone()), accent: false });
        assert!(wait_for(TIMEOUT, || settings.text_scaling_factor().is_some()));
        assert_eq!((settings.accent(), settings.dark_scheme()), (None, Some(true)));
        // No bus: nothing.
        let (_, notify) = counter();
        let none = DesktopSettings::new(notify, PortalOptions { address: None, accent: true });
        assert!(!none.listening() && none.settings() == Default::default());
    }

    pub fn desktop_services_tell_the_app() {
        let Some(bus) = TestBus::start() else { return };
        let portal = FakePortal::start(&bus, true, &desktop_values());
        let (woken, wake) = counter();
        let mut desktop = DesktopServices::new();
        // Before the desktop says: Minimize, Maximize, Close at the end.
        let bar = desktop.title_bar(false, true, true);
        assert_eq!(
            (bar.start, bar.end),
            (vec![], vec![WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close])
        );
        let options = DesktopOptions {
            portal: PortalOptions { address: Some(bus.address.clone()), accent: true },
            atspi: AtspiOptions { forced: Some(false), ..AtspiOptions::default() },
            wallpaper: WallpaperOptions { config_home: PathBuf::new(), desktop: String::new(), enabled: false },
            tray: None,
        };
        desktop.start_with("Test app", wake, options);
        let mut events = Vec::new();
        assert!(wait_for(TIMEOUT, || {
            events.extend(desktop.take_events());
            desktop.font_scale().is_some() && desktop.button_layout().is_some()
        }));
        assert!(woken.load(Ordering::SeqCst) > 0);
        assert!(events.contains(&Event::AccentChanged) && events.contains(&Event::MetricsChanged), "{events:?}");
        assert!(desktop.dark_theme());
        assert_eq!(desktop.accent(), Some(Color::new(1.0, 0.5, 0.0, 1.0)));
        assert_eq!(desktop.font_scale(), Some(1.25));
        let bar = desktop.title_bar(true, false, true); // (a window manager that can't minimize)
        assert_eq!((bar.start, bar.end, bar.maximized), (vec![WindowButton::Close], vec![], true));
        // The text size alone: the metrics, not the accent.
        portal.set("org.gnome.desktop.interface", "text-scaling-factor", Value::Double(2.0));
        let mut events = Vec::new();
        assert!(wait_for(TIMEOUT, || {
            events.extend(desktop.take_events());
            desktop.font_scale() == Some(2.0)
        }));
        assert_eq!(events, vec![Event::MetricsChanged]);
        // The accent alone.
        portal.set("org.freedesktop.appearance", "accent-color", rgb(0.0, 0.0, 1.0));
        let mut events = Vec::new();
        assert!(wait_for(TIMEOUT, || {
            events.extend(desktop.take_events());
            !events.is_empty()
        }));
        assert_eq!(events, vec![Event::AccentChanged]);
        assert_eq!(desktop.accent(), Some(Color::new(0.0, 0.0, 1.0, 1.0)));
        desktop.stop();
        assert!(desktop.take_events().is_empty());
    }

    // ---- AT-SPI ----

    /// The other end of the bridge: a connection to the test bus that answers what the bridge asks (`answer`: true
    /// if it replied) and keeps the signals it hears, as "Member detail detail1 detail2 object data".
    struct Peer {
        c: Connection,
        bridge: String,
        answer: Option<Box<dyn FnMut(&mut Connection, &Message) -> bool>>,
        heard: Vec<String>,
        last_error: String,
    }

    /// A path's last element.
    fn last(path: &str) -> &str {
        &path[path.rfind('/').map_or(0, |i| i + 1)..]
    }

    impl Peer {
        fn new(bus: &TestBus) -> Peer {
            let mut c = bus.connect();
            for interface in
                ["org.a11y.atspi.Event.Object", "org.a11y.atspi.Event.Window", "org.a11y.atspi.Event.Focus"]
            {
                c.add_match(&MatchRule::signal().interface(interface)).unwrap();
            }
            Peer { c, bridge: String::new(), answer: None, heard: Vec::new(), last_error: String::new() }
        }

        fn own(&mut self, name: &str) -> bool {
            self.c.request_name(name, 4).unwrap() == 1
        }

        fn take(&mut self, m: Message) {
            match m.kind {
                MessageType::MethodCall => {
                    let answered = match self.answer.as_mut() {
                        Some(answer) => answer(&mut self.c, &m),
                        None => false,
                    };
                    if !answered {
                        self.c.reply_error(&m, "org.freedesktop.DBus.Error.UnknownMethod", "no").unwrap();
                    }
                }
                MessageType::Signal
                    if m.interface.as_deref().is_some_and(|i| i.starts_with("org.a11y.atspi.Event")) =>
                {
                    let s = |i: usize| m.arg(i).and_then(Value::as_str).unwrap_or("").to_string();
                    let n = |i: usize| m.arg(i).and_then(Value::as_i32).unwrap_or(0);
                    let mut line = format!(
                        "{} {} {} {} {}",
                        m.member.as_deref().unwrap(),
                        s(0),
                        n(1),
                        n(2),
                        last(m.path.as_deref().unwrap())
                    );
                    match m.arg(3).map(Value::unwrap_variants) {
                        Some(Value::String(t)) => line += &format!(" {t}"),
                        Some(Value::Double(d)) => line += &format!(" {}", *d as i32),
                        Some(Value::Struct(f)) => line += &format!(" {}", last(f[1].as_str().unwrap())),
                        _ => {}
                    }
                    self.heard.push(line);
                }
                _ => {}
            }
        }

        /// Messages for `ms` milliseconds, or until `done`.
        fn serve(&mut self, ms: u64, mut done: impl FnMut(&Peer) -> bool) -> bool {
            let end = Instant::now() + Duration::from_millis(ms);
            while Instant::now() < end {
                if done(self) {
                    return true;
                }
                if let Some(m) = self.c.read(Some(Duration::from_millis(10)), None).unwrap() {
                    self.take(m);
                }
            }
            done(self)
        }

        fn heard_line(&self, line: &str) -> bool {
            self.heard.iter().any(|h| h == line)
        }

        /// A call to the bridge: its reply's values (an error's name kept).
        fn call(&mut self, path: &str, interface: &str, method: &str, args: Vec<Value>) -> Option<Vec<Value>> {
            let m = Message::method_call(Some(&self.bridge), path, Some(interface), method).with_body(args);
            match self.c.call(m, TIMEOUT) {
                Ok(reply) => Some(reply.body),
                Err(Error::Remote { name, .. }) => {
                    self.last_error = name;
                    None
                }
                Err(e) => panic!("{e}"),
            }
        }

        fn succeeds(&mut self, path: &str, interface: &str, method: &str, args: Vec<Value>) -> bool {
            self.call(path, interface, method, args).is_some()
        }

        fn ask(&mut self, path: &str, interface: &str, method: &str, args: Vec<Value>) -> Vec<Value> {
            self.call(path, interface, method, args).unwrap_or_else(|| panic!("{path} {method}: {}", self.last_error))
        }

        fn property(&mut self, path: &str, interface: &str, name: &str) -> String {
            let reply = self.ask(path, "org.freedesktop.DBus.Properties", "Get", vec![interface.into(), name.into()]);
            match reply[0].unwrap_variants() {
                Value::String(s) => s.clone(),
                Value::Int32(i) => i.to_string(),
                Value::Double(d) => format!("{d:.6}")[..4].to_string(),
                Value::Struct(f) => format!("{} {}", f[0].as_str().unwrap(), f[1].as_str().unwrap()),
                _ => "?".into(),
            }
        }

        fn children(&mut self, path: &str) -> Vec<String> {
            let reply = self.ask(path, "org.a11y.atspi.Accessible", "GetChildren", vec![]);
            reply[0]
                .items()
                .unwrap()
                .iter()
                .map(|r| last(r.fields().unwrap()[1].as_str().unwrap()).to_string())
                .collect()
        }

        fn role(&mut self, path: &str) -> u32 {
            self.ask(path, "org.a11y.atspi.Accessible", "GetRole", vec![])[0].as_u32().unwrap()
        }

        fn state(&mut self, path: &str, which: u32) -> bool {
            let reply = self.ask(path, "org.a11y.atspi.Accessible", "GetState", vec![]);
            let bits: Vec<u32> = reply[0].items().unwrap().iter().map(|v| v.as_u32().unwrap()).collect();
            bits[(which / 32) as usize] >> (which % 32) & 1 != 0
        }

        fn extents(&mut self, path: &str, coord_type: u32) -> String {
            let reply = self.ask(path, "org.a11y.atspi.Component", "GetExtents", vec![Value::Uint32(coord_type)]);
            let f: Vec<String> = reply[0].fields().unwrap().iter().map(|v| v.as_i32().unwrap().to_string()).collect();
            f.join(" ")
        }

        fn text(&mut self, path: &str, interface: &str, method: &str, args: Vec<Value>) -> String {
            let reply = self.ask(path, interface, method, args);
            let mut out = reply[0].as_str().unwrap().to_string();
            for v in &reply[1..] {
                if let Some(i) = v.as_i32() {
                    out += &format!(" {i}");
                }
            }
            out
        }

        fn boolean(&mut self, path: &str, interface: &str, method: &str, args: Vec<Value>) -> bool {
            self.ask(path, interface, method, args)[0].as_bool().unwrap()
        }
    }

    const A11Y: &str = "/org/a11y/atspi/accessible/";
    const ROOT: &str = "/org/a11y/atspi/accessible/root";
    const WINDOW: &str = "/org/a11y/atspi/accessible/window";

    fn node(id: u64) -> String {
        format!("{A11Y}{id}")
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    fn test_tree() -> Vec<AccessibilityNode> {
        let n = |id: u64| AccessibilityNode { id, enabled: true, ..AccessibilityNode::default() };
        vec![
            AccessibilityNode { bounds: rect(10.0, 20.0, 30.0, 40.0), label: "OK".into(), clickable: true, ..n(1) },
            AccessibilityNode {
                bounds: rect(0.0, 70.0, 24.0, 24.0),
                role: Some(Role::Checkbox),
                clickable: true,
                checked: Some(false),
                ..n(2)
            },
            AccessibilityNode {
                bounds: rect(0.0, 100.0, 200.0, 56.0),
                label: "Name".into(),
                value: "héllo wörld".into(),
                editable: true,
                ..n(3)
            },
            AccessibilityNode {
                bounds: rect(0.0, 160.0, 200.0, 40.0),
                state_description: "3".into(),
                range: Some(ProgressBarRangeInfo { current: 3.0, range: 0.0..=10.0, steps: 4 }),
                settable: true,
                ..n(4)
            },
            AccessibilityNode {
                bounds: rect(0.0, 200.0, 200.0, 100.0),
                scrollable: true,
                custom_actions: vec!["Archive".into()],
                ..n(5)
            },
            AccessibilityNode { parent: Some(5), bounds: rect(0.0, 210.0, 200.0, 20.0), label: "Item".into(), ..n(6) },
            AccessibilityNode {
                bounds: rect(0.0, 300.0, 200.0, 56.0),
                value: "abc".into(),
                editable: true,
                password: true,
                ..n(7)
            },
        ]
    }

    fn window(focused: bool, title: &str) -> Window {
        Window { x: 100.0, y: 50.0, width: 400.0, height: 300.0, px_to_points: 2.0, focused, title: title.into() }
    }

    fn s(v: &str) -> Value {
        Value::str(v)
    }

    pub fn atspi_bridge_serves_the_tree() {
        let Some(bus) = TestBus::start() else { return };
        let mut peer = Peer::new(&bus);
        assert!(peer.own("org.a11y.atspi.Registry"));
        let embedded = Rc::new(RefCell::new(None::<(String, String)>));
        let e = embedded.clone();
        peer.answer = Some(Box::new(move |c, m| {
            if m.member.as_deref() != Some("Embed") {
                return false;
            }
            let reference = m.arg(0).and_then(Value::fields).unwrap();
            *e.borrow_mut() = Some((reference[0].as_str().unwrap().into(), reference[1].as_str().unwrap().into()));
            c.reply(m, vec![Value::Struct(vec![s("org.a11y.atspi.Registry"), Value::path(ROOT)])]).unwrap();
            true
        }));
        let (notified, notify) = counter();
        let options = AtspiOptions {
            session_address: Some(bus.address.clone()),
            a11y_address: Some(bus.address.clone()),
            forced: Some(true),
        };
        let bridge = AtspiBridge::new("Test app", notify, options);
        assert!(bridge.listening());
        assert!(peer.serve(3000, |_| bridge.active())); // connected, and into the registry's desktop
        let (name, path) = embedded.borrow().clone().expect("embedded");
        assert_eq!(path, ROOT);
        peer.bridge = name;
        assert!(bridge.take_tree_request() && !bridge.take_tree_request());
        bridge.publish(test_tree(), window(true, "Main"));
        peer.serve(300, |_| false);
        assert!(peer.heard_line("Activate  0 0 window Main"), "{:?}", peer.heard);
        assert!(peer.heard_line("StateChanged active 1 0 window"));

        // The application, its window, the nodes in it.
        assert_eq!((peer.role(ROOT), peer.role(WINDOW)), (75, 23));
        assert_eq!(peer.property(ROOT, "org.a11y.atspi.Accessible", "Name"), "Test app");
        assert_eq!(
            peer.property(ROOT, "org.a11y.atspi.Accessible", "Parent"),
            "org.a11y.atspi.Registry /org/a11y/atspi/accessible/root"
        );
        assert_eq!(peer.property(ROOT, "org.a11y.atspi.Application", "ToolkitName"), "Kompoz");
        assert_eq!(peer.children(ROOT), ["window"]);
        assert_eq!(peer.property(WINDOW, "org.a11y.atspi.Accessible", "Name"), "Main");
        let bridge_name = peer.bridge.clone();
        assert_eq!(peer.property(WINDOW, "org.a11y.atspi.Accessible", "Parent"), format!("{bridge_name} {ROOT}"));
        assert_eq!(peer.children(WINDOW), ["1", "2", "3", "4", "5", "7"]);
        assert!(peer.state(WINDOW, 1)); // active
        assert_eq!(peer.extents(WINDOW, 0), "100 50 400 300");
        // A button: where it is (px to points, from the screen, the window, its parent), what it does.
        assert_eq!(peer.role(&node(1)), 43);
        assert_eq!(peer.property(&node(1), "org.a11y.atspi.Accessible", "Name"), "OK");
        assert_eq!(peer.property(&node(1), "org.a11y.atspi.Accessible", "Parent"), format!("{bridge_name} {WINDOW}"));
        assert_eq!(peer.extents(&node(1), 0), "120 90 60 80");
        assert_eq!(peer.extents(&node(1), 1), "20 40 60 80");
        assert_eq!(peer.extents(&node(6), 2), "0 20 400 40");
        assert!(
            peer.state(&node(1), 8) && peer.state(&node(1), 24) && peer.state(&node(1), 11) && peer.state(&node(1), 30)
        );
        assert!(!peer.state(&node(1), 12) && !peer.state(&node(1), 7));
        assert_eq!(peer.text(&node(1), "org.a11y.atspi.Action", "GetName", vec![Value::Int32(0)]), "click");
        assert!(peer.boolean(&node(1), "org.a11y.atspi.Action", "DoAction", vec![Value::Int32(0)]));
        assert!(!peer.boolean(&node(1), "org.a11y.atspi.Action", "DoAction", vec![Value::Int32(1)]));
        assert!(notified.load(Ordering::SeqCst) > 0); // the main loop woken
        let requests = bridge.take_requests();
        assert_eq!(requests, [Request { id: 1, action: SemanticsAction::Click }]);
        assert!(bridge.take_requests().is_empty());
        assert!(!peer.succeeds(&node(1), "org.a11y.atspi.Text", "GetText", vec![Value::Int32(0), Value::Int32(-1)]));
        assert_eq!(peer.last_error, "org.freedesktop.DBus.Error.UnknownMethod"); // a button has no text
        // A checkbox.
        assert_eq!(peer.role(&node(2)), 7);
        assert!(peer.state(&node(2), 41) && !peer.state(&node(2), 4));
        // A text field: offsets in characters, words, what is set.
        let text = "org.a11y.atspi.Text";
        let editable = "org.a11y.atspi.EditableText";
        let (i, u) = (Value::Int32, Value::Uint32);
        assert_eq!(peer.role(&node(3)), 79);
        assert!(peer.state(&node(3), 7) && peer.state(&node(3), 11));
        assert_eq!(peer.property(&node(3), text, "CharacterCount"), "11");
        assert_eq!(peer.text(&node(3), text, "GetText", vec![i(1), i(3)]), "él");
        assert_eq!(peer.text(&node(3), text, "GetText", vec![i(6), i(-1)]), "wörld");
        assert_eq!(peer.text(&node(3), text, "GetStringAtOffset", vec![i(7), u(1)]), "wörld 6 11");
        assert_eq!(peer.text(&node(3), text, "GetStringAtOffset", vec![i(5), u(1)]), "héllo  0 6");
        assert_eq!(peer.text(&node(3), text, "GetStringAtOffset", vec![i(1), u(0)]), "é 1 2");
        assert_eq!(peer.text(&node(3), text, "GetTextAfterOffset", vec![i(1), u(1)]), "wörld 6 11");
        assert_eq!(peer.text(&node(3), text, "GetTextBeforeOffset", vec![i(7), u(1)]), "héllo  0 6");
        assert_eq!(peer.text(&node(3), text, "GetTextAtOffset", vec![i(3), u(5)]), "héllo wörld 0 11");
        assert_eq!(peer.ask(&node(3), text, "GetCharacterAtOffset", vec![i(1)])[0], i(0xE9));
        assert!(peer.boolean(&node(3), editable, "SetTextContents", vec![s("x")]));
        assert!(peer.boolean(&node(3), editable, "DeleteText", vec![i(0), i(6)]));
        assert!(peer.boolean(&node(3), editable, "InsertText", vec![i(5), s("!?"), i(1)]));
        assert!(peer.boolean(&node(3), "org.a11y.atspi.Component", "GrabFocus", vec![]));
        assert!(peer.boolean(&node(1), "org.a11y.atspi.Component", "GrabFocus", vec![])); // a button: Tab goes to it
        assert!(!peer.boolean(&node(6), "org.a11y.atspi.Component", "GrabFocus", vec![])); // a label: nothing to focus
        let requests = bridge.take_requests();
        assert_eq!(
            requests,
            [
                Request { id: 3, action: SemanticsAction::SetText("x".into()) },
                Request { id: 3, action: SemanticsAction::SetText("wörld".into()) },
                Request { id: 3, action: SemanticsAction::SetText("héllo! wörld".into()) },
                Request { id: 3, action: SemanticsAction::Focus },
                Request { id: 1, action: SemanticsAction::Focus },
            ]
        );
        // A password field: what is typed isn't said.
        assert_eq!(peer.role(&node(7)), 40);
        assert_eq!(peer.text(&node(7), text, "GetText", vec![i(0), i(-1)]), "•••");
        // A slider.
        let value = "org.a11y.atspi.Value";
        assert_eq!(peer.role(&node(4)), 51);
        assert_eq!(peer.property(&node(4), value, "CurrentValue"), "3.00");
        assert_eq!(peer.property(&node(4), value, "MaximumValue"), "10.0");
        assert_eq!(peer.property(&node(4), value, "MinimumIncrement"), "2.00");
        assert_eq!(peer.property(&node(4), value, "Text"), "3");
        assert_eq!(peer.property(&node(4), "org.a11y.atspi.Accessible", "Description"), "3");
        let set = vec![s(value), s("CurrentValue"), Value::variant(Value::Double(7.0))];
        assert!(peer.succeeds(&node(4), "org.freedesktop.DBus.Properties", "Set", set));
        assert_eq!(bridge.take_requests(), [Request { id: 4, action: SemanticsAction::SetProgress(7.0) }]);
        // A scrolling list: its actions (its own too), the node in it.
        let action = "org.a11y.atspi.Action";
        assert_eq!((peer.role(&node(5)), peer.role(&node(6))), (49, 29));
        assert_eq!(peer.children(&node(5)), ["6"]);
        assert_eq!(peer.property(&node(5), action, "NActions"), "3");
        assert_eq!(peer.text(&node(5), action, "GetName", vec![i(2)]), "Archive");
        assert!(peer.boolean(&node(5), action, "DoAction", vec![i(1)]));
        assert!(peer.boolean(&node(5), action, "DoAction", vec![i(2)]));
        assert_eq!(
            bridge.take_requests(),
            [
                Request { id: 5, action: SemanticsAction::ScrollBackward },
                Request { id: 5, action: SemanticsAction::CustomAction(0) }
            ]
        );
        let mut interfaces = |path: &str| {
            let reply = peer.ask(path, "org.a11y.atspi.Accessible", "GetInterfaces", vec![]);
            reply[0].items().unwrap().iter().map(|v| v.as_str().unwrap().to_string() + " ").collect::<String>()
        };
        assert_eq!(interfaces(&node(6)), "org.a11y.atspi.Accessible org.a11y.atspi.Component "); // a label does nothing
        assert!(interfaces(&node(1)).contains("org.a11y.atspi.Action"));
        let accessible = "org.a11y.atspi.Accessible";
        assert_eq!(peer.ask(&node(6), accessible, "GetIndexInParent", vec![])[0], i(0));
        assert_eq!(peer.ask(&node(3), accessible, "GetIndexInParent", vec![])[0], i(2));
        let mut child_at = |path: &str, index: i32| {
            let reply = peer.ask(path, accessible, "GetChildAtIndex", vec![i(index)]);
            reply[0].fields().unwrap()[1].as_str().unwrap().to_string()
        };
        assert_eq!(child_at(WINDOW, 2), node(3));
        assert_eq!(child_at(WINDOW, 6), "/org/a11y/atspi/null");
        let component = "org.a11y.atspi.Component";
        let at_point = peer.ask(&node(5), component, "GetAccessibleAtPoint", vec![i(150), i(475), u(0)]);
        assert_eq!(at_point[0].fields().unwrap()[1].as_str().unwrap(), node(6));
        assert_eq!(peer.ask(&node(5), component, "Contains", vec![i(150), i(800), u(0)])[0], Value::Boolean(false));
        // Introspection, and Peer's methods.
        let xml = peer.ask(&node(4), "org.freedesktop.DBus.Introspectable", "Introspect", vec![]);
        assert!(xml[0].as_str().unwrap().contains("<interface name=\"org.a11y.atspi.Value\"/>"));
        assert!(peer.succeeds(&node(1), "org.freedesktop.DBus.Peer", "Ping", vec![]));
        // What isn't there.
        assert!(!peer.succeeds(&node(99), accessible, "GetRole", vec![]));
        assert_eq!(peer.last_error, "org.freedesktop.DBus.Error.UnknownObject");
        assert!(!peer.succeeds(&node(1), accessible, "Frobnicate", vec![]));
        assert_eq!(peer.last_error, "org.freedesktop.DBus.Error.UnknownMethod");
        assert!(peer.succeeds("/org/a11y/atspi/cache", "org.a11y.atspi.Cache", "GetItems", vec![]));

        // What changed is told.
        let mut next = test_tree();
        next[0].label = "Done".into();
        next[0].live_region = Some(LiveRegionMode::Polite);
        next[1].checked = Some(true);
        next[2].value = "héllo, wörld".into();
        next[2].focused = true;
        next[3].range.as_mut().unwrap().current = 5.0;
        next.remove(5); // the list's item
        next.push(AccessibilityNode { id: 8, label: "New".into(), enabled: true, ..AccessibilityNode::default() });
        peer.heard.clear();
        bridge.publish(next.clone(), window(false, "Main"));
        peer.serve(300, |_| false);
        let lines = [
            "PropertyChange accessible-name 0 0 1 Done",
            "Announcement  1 0 1 Done",
            "StateChanged checked 1 0 2",
            "TextChanged insert 5 1 3 ,",
            "PropertyChange accessible-value 0 0 4 5",
            "ChildrenChanged remove 0 0 5 6",
            "ChildrenChanged add 6 0 window 8",
            "Deactivate  0 0 window Main",
            "StateChanged active 0 0 window",
            "StateChanged focused 1 0 3",
            "Focus  0 0 3",
        ];
        for line in lines {
            assert!(peer.heard_line(line), "not heard: {line} (heard {:?})", peer.heard);
        }
        assert_eq!(peer.heard.len(), 11, "{:?}", peer.heard);
        // Typed away, and focus going elsewhere.
        let mut after = next;
        after[2].value = "hé  wörld".into();
        after[2].focused = false;
        after[0].live_region = Some(LiveRegionMode::Assertive);
        after[0].state_description = "now".into();
        after[1].enabled = false;
        // (A label with something in it: a panel.)
        after.push(AccessibilityNode {
            id: 9,
            parent: Some(8),
            label: "Inside".into(),
            enabled: true,
            ..Default::default()
        });
        peer.heard.clear();
        bridge.publish(after, window(false, "Other"));
        peer.serve(300, |_| false);
        let lines = [
            "TextChanged delete 2 4 3 llo,",
            "TextChanged insert 2 1 3  ",
            "StateChanged focused 0 0 3",
            "Announcement  2 0 1 Done, now",
            "PropertyChange accessible-description 0 0 1 now",
            "PropertyChange accessible-name 0 0 window Other",
            "StateChanged enabled 0 0 2",
            "StateChanged sensitive 0 0 2",
            "ChildrenChanged add 0 0 8 9",
        ];
        for line in lines {
            assert!(peer.heard_line(line), "not heard: {line} (heard {:?})", peer.heard);
        }
        assert_eq!(peer.heard.len(), 9, "{:?}", peer.heard);
        assert_eq!((peer.role(&node(8)), peer.role(&node(9))), (39, 29));
        assert!(!peer.state(&node(2), 8) && !peer.state(&node(2), 24));
        assert!(!peer.boolean(&node(2), component, "GrabFocus", vec![])); // disabled: Tab doesn't go to it
        // In the spaces after a word: that word's.
        assert_eq!(peer.text(&node(3), text, "GetStringAtOffset", vec![i(3), u(1)]), "hé   0 4");
        drop(bridge);
    }

    /// Without KOMPOZ_ATSPI it waits for the desktop's accessibility to be turned on (org.a11y.Status), then finds
    /// the accessibility bus as the launcher says.
    pub fn atspi_bridge_follows_the_desktop() {
        let Some(bus) = TestBus::start() else { return };
        let mut launcher = Peer::new(&bus);
        assert!(launcher.own("org.a11y.Bus") && launcher.own("org.a11y.atspi.Registry"));
        let enabled = Rc::new(Cell::new(false));
        let asked = Rc::new(Cell::new(0));
        let (e, a, address) = (enabled.clone(), asked.clone(), bus.address.clone());
        launcher.answer = Some(Box::new(move |c, m| {
            let reply = match m.member.as_deref().unwrap() {
                "Get" => {
                    a.set(a.get() + 1);
                    let name = m.arg(1).and_then(Value::as_str).unwrap_or("");
                    vec![Value::variant(Value::Boolean(e.get() && name == "ScreenReaderEnabled"))]
                }
                "GetAddress" => vec![s(&address)],
                "Embed" => vec![Value::Struct(vec![s("org.a11y.atspi.Registry"), Value::path(ROOT)])],
                _ => return false,
            };
            c.reply(m, reply).unwrap();
            true
        }));
        let (notified, notify) = counter();
        let options = AtspiOptions { session_address: Some(bus.address.clone()), a11y_address: None, forced: None };
        let bridge = AtspiBridge::new("Test app", notify, options);
        assert!(launcher.serve(1000, |_| asked.get() >= 2)); // IsEnabled, ScreenReaderEnabled: both off
        launcher.serve(200, |_| false);
        assert!(!bridge.active());
        enabled.set(true); // a screen reader started
        let changes = Value::dict(
            Type::String,
            Type::Variant,
            vec![(s("ScreenReaderEnabled"), Value::variant(Value::Boolean(true)))],
        );
        let changed = Message::signal("/org/a11y/bus", "org.freedesktop.DBus.Properties", "PropertiesChanged")
            .with_body(vec![s("org.a11y.Status"), changes, Value::array(Type::String, vec![])]);
        launcher.c.send(&changed).unwrap();
        assert!(launcher.serve(3000, |_| bridge.active()));
        assert!(launcher.serve(1000, |_| notified.load(Ordering::SeqCst) > 0)); // to publish the tree
        assert!(bridge.take_tree_request());
        drop(bridge);

        // KOMPOZ_ATSPI=0: never.
        let (_, notify) = counter();
        let off = AtspiBridge::new("Test app", notify, AtspiOptions { forced: Some(false), ..options_for(&bus) });
        assert!(!off.listening() && !off.active());
    }

    fn options_for(bus: &TestBus) -> AtspiOptions {
        AtspiOptions {
            session_address: Some(bus.address.clone()),
            a11y_address: Some(bus.address.clone()),
            forced: None,
        }
    }

    #[allow(non_snake_case)]
    #[kompozable]
    fn Plus(clicks: Rc<Cell<i32>>) {
        Box(modifier = Modifier
            .size_wh(120.dp(), 80.dp())
            .clickable(role = Some(Role::Button), on_click = move || clicks.set(clicks.get() + 1))
            .semantics_with(false, |c| c.set_content_description("Plus")));
    }

    /// The desktop services as a platform runs them, on a host: the tree published as the bridge asks for it and as
    /// the window's focus changes, a screen reader's click performed on the host.
    pub fn desktop_services_serve_the_hosts_tree() {
        let Some(bus) = TestBus::start() else { return };
        let mut peer = Peer::new(&bus);
        assert!(peer.own("org.a11y.atspi.Registry"));
        let embedded = Rc::new(RefCell::new(None::<String>));
        let e = embedded.clone();
        peer.answer = Some(Box::new(move |c, m| {
            if m.member.as_deref() != Some("Embed") {
                return false;
            }
            *e.borrow_mut() = Some(m.arg(0).and_then(Value::fields).unwrap()[0].as_str().unwrap().to_string());
            c.reply(m, vec![Value::Struct(vec![s("org.a11y.atspi.Registry"), Value::path(ROOT)])]).unwrap();
            true
        }));
        let clicks = Rc::new(Cell::new(0));
        let c = clicks.clone();
        let host = KompozHost::new(kompozable_lambda!(move || Plus(c.clone())));
        host.set_viewport_size(IntSize::new(400, 300));
        let (woken, wake) = counter();
        let mut desktop = DesktopServices::new();
        let options = DesktopOptions {
            portal: PortalOptions { address: None, accent: true },
            atspi: AtspiOptions { forced: Some(true), ..options_for(&bus) },
            wallpaper: WallpaperOptions { config_home: PathBuf::new(), desktop: String::new(), enabled: false },
            tray: None,
        };
        desktop.start_with("Test app", wake, options);
        // The platform's loop: what the services told handled, a frame composed, the window 400 by 300 points at
        // (10, 20), a px a point.
        let mut t = 0;
        let mut frame = |desktop: &mut DesktopServices| {
            for e in desktop.take_events() {
                desktop.handled(&host, &e);
            }
            t += 100_000_000;
            let redraw = host.frame(t);
            desktop.composed(&host, redraw, t, Rect::new(10.0, 20.0, 410.0, 320.0), 1.0, "Main");
        };
        // (Connected, it wakes the loop for the tree; published, it wants no more frames.)
        assert!(peer.serve(3000, |_| {
            frame(&mut desktop);
            woken.load(Ordering::SeqCst) > 0 && !desktop.wants_frames()
        }));
        peer.bridge = embedded.borrow().clone().unwrap();
        peer.serve(100, |_| false);
        // The host's root, the button in it.
        let roots = peer.children(WINDOW);
        assert_eq!(roots.len(), 1, "{roots:?}");
        let root = format!("{A11Y}{}", roots[0]);
        assert_eq!(peer.role(&root), 39);
        let buttons = peer.children(&root);
        assert_eq!(buttons.len(), 1, "{buttons:?}");
        let plus = format!("{A11Y}{}", buttons[0]);
        assert_eq!(peer.role(&plus), 43);
        assert_eq!(peer.property(&plus, "org.a11y.atspi.Accessible", "Name"), "Plus");
        assert_eq!(peer.extents(&plus, 0), "10 20 120 80");
        // A screen reader's click: performed on the host.
        assert!(peer.boolean(&plus, "org.a11y.atspi.Action", "DoAction", vec![Value::Int32(0)]));
        assert!(wait_for(TIMEOUT, || {
            frame(&mut desktop);
            clicks.get() == 1
        }));
        // The window focused: the tree published again, its activation told.
        peer.heard.clear();
        desktop.handled(&host, &Event::FocusGained);
        assert!(desktop.wants_frames());
        frame(&mut desktop);
        assert!(peer.serve(1000, |p| p.heard_line("Activate  0 0 window Main")), "{:?}", peer.heard);
        desktop.stop();
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        let tests: &[(&str, fn())] = &[
            ("connections_call_serve_and_hear", linux::connections_call_serve_and_hear),
            ("connections_fail_as_they_should", linux::connections_fail_as_they_should),
            ("portal_settings_are_read_and_followed", linux::portal_settings_are_read_and_followed),
            ("desktop_services_tell_the_app", linux::desktop_services_tell_the_app),
            ("atspi_bridge_serves_the_tree", linux::atspi_bridge_serves_the_tree),
            ("atspi_bridge_follows_the_desktop", linux::atspi_bridge_follows_the_desktop),
            ("desktop_services_serve_the_hosts_tree", linux::desktop_services_serve_the_hosts_tree),
            ("tray_icons_and_notifications_on_the_bus", linux::tray_icons_and_notifications_on_the_bus),
        ];
        let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
        let mut failed = 0;
        for (name, test) in tests {
            if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                continue;
            }
            let ok = std::panic::catch_unwind(test).is_ok();
            failed += usize::from(!ok);
            println!("{} {name}", if ok { "ok    " } else { "FAILED" });
        }
        if failed > 0 {
            std::process::exit(1);
        }
    }
}
