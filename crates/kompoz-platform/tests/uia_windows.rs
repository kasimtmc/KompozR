//! The app's window read and driven through UI Automation's own client (`KOMPOZ_DISPLAY_TESTS=1`; on Windows, and
//! under wine here; skipped otherwise), as Narrator does it, from a thread of the app's process: the Counter's
//! button found by its name and invoked (the count it shows changes), a field's text read through its TextPattern
//! (and its value), a switch toggled, the focus put on the field heard as a focus event. Run in a process of its own
//! (this test run again): its client thread reports, the app's loop serves.
//!
//! Wine's client lacks most of Windows' client-side patterns (E_NOTIMPL): there the client asks the app's providers
//! themselves (`kompoz_platform::windows::uia_root_provider`), as UI Automation would; on Windows (CI's windows job)
//! the client's own patterns are required.

#![allow(non_snake_case)]

#[cfg(windows)]
mod on_windows {
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::*;
    use kompoz_runtime::*;
    use kompoz_text::TextStyle;
    use kompoz_ui::Modifier;
    use kompoz_ui::semantics::Role;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{E_NOTIMPL, LPARAM, WPARAM};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Variant::{VARIANT, VT_BSTR};
    use windows::Win32::UI::Accessibility::*;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE};
    use windows_core::{Interface, Ref, Result, implement, w};

    pub const CHILD: &str = "KOMPOZ_UIA_WINDOWS_CHILD";
    const TITLE: &str = "UIA Sayaç";

    /// The Counter's button ("Ekle") and count ("Sayaç: n"), a field ("Ad") and a switch ("Wi-Fi").
    #[kompozable]
    pub fn Screen() {
        let count = remember(|| mutable_state_of(0));
        let on = remember(|| mutable_state_of(false));
        let field = remember_text_field_state(initial_text = "merhaba dünya".to_string());
        let style = TextStyle { font_size: 20.sp(), color: Color::Black, ..TextStyle::Default };
        Column(
            modifier = Modifier.fill_max_size().background(Color::White).padding(16.dp()),
            vertical_arrangement = Arrangement::spaced_by(12.dp()),
            content = move || {
                let (count, on, field, style) = (count.clone(), on.clone(), field.clone(), style.clone());
                let add = count.clone();
                Box(
                    modifier = Modifier
                        .size_wh(120.dp(), 48.dp())
                        .background(Color::from_argb(0xff3f51b5))
                        .clickable(role = Some(Role::Button), on_click = move || add.set(add.get() + 1)),
                    content = || BasicText("Ekle"),
                );
                BasicText(format!("Sayaç: {}", count.get()), style = style.clone());
                BasicTextField(
                    field,
                    modifier = Modifier.width(240.dp()).semantics(|c| c.set_content_description("Ad")),
                    text_style = style.clone(),
                );
                let toggle = on.clone();
                Row(
                    modifier = Modifier.toggleable(
                        on.get(),
                        role = Some(Role::Switch),
                        on_value_change = move |v| toggle.set(v),
                    ),
                    content = move || BasicText("Wi-Fi", style = style.clone()),
                );
            },
        );
    }

    /// A SAFEARRAY of doubles read (and destroyed).
    fn doubles(array: *mut windows::Win32::System::Com::SAFEARRAY) -> Vec<f64> {
        use windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetUBound};
        if array.is_null() {
            return Vec::new();
        }
        // SAFETY: a one-dimension VT_R8 array UI Automation gave, read in its bounds, destroyed once.
        unsafe {
            let upper = SafeArrayGetUBound(array, 1).unwrap_or(-1);
            let out = (0..=upper)
                .map(|i| {
                    let mut v = 0f64;
                    let _ = SafeArrayGetElement(array, &i, &mut v as *mut f64 as *mut _);
                    v
                })
                .collect();
            let _ = SafeArrayDestroy(array);
            out
        }
    }

    fn under_wine() -> bool {
        // SAFETY: queries of the process's ntdll.
        unsafe {
            GetModuleHandleW(w!("ntdll.dll"))
                .is_ok_and(|m| GetProcAddress(m, windows_core::s!("wine_get_version")).is_some())
        }
    }

    fn name_of(e: &IUIAutomationElement) -> String {
        // SAFETY: a client call.
        unsafe { e.CurrentName() }.map(|b| b.to_string()).unwrap_or_default()
    }

    /// The focus events heard, by the name of what got it.
    #[implement(IUIAutomationFocusChangedEventHandler)]
    struct Focus(Arc<Mutex<Vec<String>>>);

    impl IUIAutomationFocusChangedEventHandler_Impl for Focus_Impl {
        fn HandleFocusChangedEvent(&self, sender: Ref<IUIAutomationElement>) -> Result<()> {
            self.0.lock().unwrap().push(sender.ok().map(name_of).unwrap_or_default());
            Ok(())
        }
    }

    /// Until `f` says something, or `seconds` pass.
    fn wait_for<T>(seconds: u64, mut f: impl FnMut() -> Option<T>) -> Option<T> {
        let end = Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(v) = f() {
                return Some(v);
            }
            if Instant::now() > end {
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A VARIANT's string (read here: Wine's propsys converts few).
    fn text_of_variant(v: &VARIANT) -> String {
        if v.vt() != VT_BSTR {
            return String::new();
        }
        // SAFETY: a VT_BSTR VARIANT's string, borrowed while the VARIANT is.
        unsafe { v.Anonymous.Anonymous.Anonymous.bstrVal.to_string() }
    }

    /// A provider of the app's window by its name, walked to with Navigate as UI Automation does.
    fn provider(root: &IRawElementProviderSimple, wanted: &str) -> Option<IRawElementProviderSimple> {
        fn walk(parent: &IRawElementProviderFragment, wanted: &str) -> Option<IRawElementProviderSimple> {
            // SAFETY: provider calls (free-threaded).
            let mut child = unsafe { parent.Navigate(NavigateDirection_FirstChild) }.ok();
            while let Some(c) = child {
                let simple: IRawElementProviderSimple = c.cast().ok()?;
                // SAFETY: as above.
                let name = unsafe { simple.GetPropertyValue(UIA_NamePropertyId) }.unwrap_or_default();
                if text_of_variant(&name) == wanted {
                    return Some(simple);
                }
                if let Some(inside) = walk(&c, wanted) {
                    return Some(inside);
                }
                // SAFETY: as above.
                child = unsafe { c.Navigate(NavigateDirection_NextSibling) }.ok();
            }
            None
        }
        walk(&root.cast().ok()?, wanted)
    }

    fn pattern<T: Interface>(p: &IRawElementProviderSimple, id: UIA_PATTERN_ID) -> Option<T> {
        // SAFETY: a provider call.
        unsafe { p.GetPatternProvider(id) }.ok().and_then(|u| u.cast().ok())
    }

    /// Whether a client call failed for want of what Wine's client has (E_NOTIMPL, under wine: on Windows every
    /// client call is required).
    fn lacking(e: &windows_core::Error) -> bool {
        e.code() == E_NOTIMPL && under_wine()
    }

    /// What the client does, as lines of what it found ("ok …"; "(wine) …" where Wine's client lacks a pattern and
    /// the providers were asked themselves) or why it failed.
    pub fn client() -> std::result::Result<Vec<String>, String> {
        let mut said = Vec::new();
        // SAFETY: COM on the client's own thread; client calls on objects of UI Automation's; provider calls on the
        // app's providers (free-threaded).
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(|e| format!("no client: {e}"))?;
            let title: Vec<u16> = TITLE.encode_utf16().chain([0]).collect();
            let hwnd = wait_for(60, || {
                FindWindowW(w!("KompozWindow"), windows_core::PCWSTR(title.as_ptr())).ok().filter(|h| !h.is_invalid())
            })
            .ok_or("the window didn't open")?;
            let window = uia.ElementFromHandle(hwnd).map_err(|e| format!("no element of the window: {e}"))?;
            let walker = uia.CreateTreeWalker(&uia.CreateTrueCondition().unwrap()).unwrap();
            fn find(
                walker: &IUIAutomationTreeWalker,
                e: &IUIAutomationElement,
                wanted: &str,
            ) -> Option<IUIAutomationElement> {
                // SAFETY: client calls.
                let mut child = unsafe { walker.GetFirstChildElement(e) }.ok();
                while let Some(c) = child {
                    if name_of(&c) == wanted {
                        return Some(c);
                    }
                    if let Some(inside) = find(walker, &c, wanted) {
                        return Some(inside);
                    }
                    // SAFETY: as above.
                    child = unsafe { walker.GetNextSiblingElement(&c) }.ok();
                }
                None
            }
            // The tree comes as the app publishes it (asked for by the first WM_GETOBJECT).
            let button = wait_for(20, || find(&walker, &window, "Ekle")).ok_or("no \"Ekle\" button")?;
            if button.CurrentControlType().map(|t| t.0) != Ok(UIA_ButtonControlTypeId.0) {
                return Err("\"Ekle\" isn't a button".into());
            }
            find(&walker, &window, "Sayaç: 0").ok_or("no \"Sayaç: 0\"")?;
            said.push("ok found the Counter's button and count".into());
            let root =
                kompoz_platform::windows::uia_root_provider(hwnd.0 as isize).ok_or("no provider of the window")?;
            let focus = Arc::new(Mutex::new(Vec::new()));
            let handler: IUIAutomationFocusChangedEventHandler = Focus(focus.clone()).into();
            uia.AddFocusChangedEventHandler(None, &handler).map_err(|e| format!("focus handler: {e}"))?;

            // Invoked: the count changes.
            match button.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
                Ok(invoke) => {
                    invoke.Invoke().map_err(|e| format!("invoke: {e}"))?;
                    said.push("ok invoked".into());
                }
                Err(e) if lacking(&e) => {
                    let p = provider(&root, "Ekle").ok_or("no \"Ekle\" provider")?;
                    let invoke: IInvokeProvider = pattern(&p, UIA_InvokePatternId).ok_or("no invoke pattern")?;
                    invoke.Invoke().map_err(|e| format!("invoke: {e}"))?;
                    said.push("(wine) invoked through its provider".into());
                }
                Err(e) => return Err(format!("no invoke pattern: {e}")),
            }
            wait_for(10, || find(&walker, &window, "Sayaç: 1")).ok_or("the count didn't change")?;
            said.push("ok the count changed: Sayaç: 1".into());

            // The field's text, through its TextPattern and its value.
            let field = find(&walker, &window, "Ad").ok_or("no \"Ad\" field")?;
            if field.CurrentControlType().map(|t| t.0) != Ok(UIA_EditControlTypeId.0) {
                return Err("\"Ad\" isn't an edit".into());
            }
            // (Its first word's and first character's rectangles too: where they are laid out.)
            let (read, first, word_rects, char_rects) =
                match field.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                    Ok(text) => {
                        let all = text.DocumentRange().map_err(|e| format!("document range: {e}"))?;
                        let word = all.Clone().unwrap();
                        word.ExpandToEnclosingUnit(TextUnit_Word).map_err(|e| format!("expand: {e}"))?;
                        let char = all.Clone().unwrap();
                        char.ExpandToEnclosingUnit(TextUnit_Character).map_err(|e| format!("expand: {e}"))?;
                        said.push("ok read through the client's TextPattern".into());
                        (
                            all.GetText(-1).unwrap().to_string(),
                            word.GetText(-1).unwrap().to_string(),
                            doubles(word.GetBoundingRectangles().map_err(|e| format!("rectangles: {e}"))?),
                            doubles(char.GetBoundingRectangles().map_err(|e| format!("rectangles: {e}"))?),
                        )
                    }
                    Err(e) if lacking(&e) => {
                        let p = provider(&root, "Ad").ok_or("no \"Ad\" provider")?;
                        let text: ITextProvider = pattern(&p, UIA_TextPatternId).ok_or("no text pattern")?;
                        let all = text.DocumentRange().map_err(|e| format!("document range: {e}"))?;
                        let word = all.Clone().unwrap();
                        word.ExpandToEnclosingUnit(TextUnit_Word).map_err(|e| format!("expand: {e}"))?;
                        let char = all.Clone().unwrap();
                        char.ExpandToEnclosingUnit(TextUnit_Character).map_err(|e| format!("expand: {e}"))?;
                        said.push("(wine) read through its provider's TextPattern".into());
                        (
                            all.GetText(-1).unwrap().to_string(),
                            word.GetText(-1).unwrap().to_string(),
                            doubles(word.GetBoundingRectangles().map_err(|e| format!("rectangles: {e}"))?),
                            doubles(char.GetBoundingRectangles().map_err(|e| format!("rectangles: {e}"))?),
                        )
                    }
                    Err(e) => return Err(format!("no text pattern: {e}")),
                };
            // A rectangle each (one line): the character inside the word, the word inside the field, narrower than it.
            let field_box = field.CurrentBoundingRectangle().map_err(|e| format!("field's box: {e}"))?;
            let field_width = (field_box.right - field_box.left) as f64;
            let ([wx, _, ww, wh], [cx, _, cw, _]) = match (word_rects.as_slice(), char_rects.as_slice()) {
                ([a, b, c, d], [e, f, g, h]) => ([*a, *b, *c, *d], [*e, *f, *g, *h]),
                _ => return Err(format!("the word's rectangles {word_rects:?}, its character's {char_rects:?}")),
            };
            if !(cw > 0.0 && cw < ww && ww < field_width && wh > 0.0 && cx >= wx && wx >= field_box.left as f64) {
                return Err(format!("the word at {word_rects:?}, its character at {char_rects:?}, in {field_box:?}"));
            }
            said.push(format!("ok the first word {ww:.0} px wide, its first character {cw:.0} px: laid out"));
            if (read.as_str(), first.as_str()) != ("merhaba dünya", "merhaba ") {
                return Err(format!("the field's text read \"{read}\", its first word \"{first}\""));
            }
            said.push("ok the field's text: merhaba dünya".into());
            let value = match field.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
                Ok(value) => value.CurrentValue().map_err(|e| format!("value: {e}"))?.to_string(),
                Err(e) if lacking(&e) => {
                    let p = provider(&root, "Ad").ok_or("no \"Ad\" provider")?;
                    let value: IValueProvider = pattern(&p, UIA_ValuePatternId).ok_or("no value pattern")?;
                    value.Value().map_err(|e| format!("value: {e}"))?.to_string()
                }
                Err(e) => return Err(format!("no value pattern: {e}")),
            };
            if value != "merhaba dünya" {
                return Err(format!("the field's value \"{value}\""));
            }

            // The switch toggled.
            let switch = find(&walker, &window, "Wi-Fi").ok_or("no \"Wi-Fi\" switch")?;
            if switch.CurrentControlType().map(|t| t.0) != Ok(UIA_ButtonControlTypeId.0) {
                return Err("\"Wi-Fi\" isn't a (toggle) button".into());
            }
            let state: Box<dyn Fn() -> Option<ToggleState>> =
                match switch.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId) {
                    Ok(toggle) => {
                        let before = toggle.CurrentToggleState().ok();
                        toggle.Toggle().map_err(|e| format!("toggle: {e}"))?;
                        said.push(format!("ok toggled through the client (was {before:?})"));
                        Box::new(move || toggle.CurrentToggleState().ok())
                    }
                    Err(e) if lacking(&e) => {
                        let p = provider(&root, "Wi-Fi").ok_or("no \"Wi-Fi\" provider")?;
                        let toggle: IToggleProvider = pattern(&p, UIA_TogglePatternId).ok_or("no toggle pattern")?;
                        if toggle.ToggleState() != Ok(ToggleState_Off) {
                            return Err("the switch wasn't off".into());
                        }
                        toggle.Toggle().map_err(|e| format!("toggle: {e}"))?;
                        said.push("(wine) toggled through its provider".into());
                        Box::new(move || toggle.ToggleState().ok())
                    }
                    Err(e) => return Err(format!("no toggle pattern: {e}")),
                };
            wait_for(10, || (state() == Some(ToggleState_On)).then_some(())).ok_or("the switch didn't turn on")?;
            said.push("ok the switch is on".into());

            // The focus put on the field: heard.
            match field.SetFocus() {
                Ok(()) => said.push("ok focused through the client".into()),
                Err(e) if lacking(&e) => {
                    let p: IRawElementProviderFragment =
                        provider(&root, "Ad").ok_or("no \"Ad\" provider")?.cast().map_err(|e| e.to_string())?;
                    p.SetFocus().map_err(|e| format!("SetFocus: {e}"))?;
                    said.push("(wine) focused through its provider".into());
                }
                Err(e) => return Err(format!("SetFocus: {e}")),
            }
            wait_for(10, || focus.lock().unwrap().iter().any(|n| n == "Ad").then_some(()))
                .ok_or_else(|| format!("no focus event on \"Ad\" (heard {:?})", focus.lock().unwrap()))?;
            said.push("ok the focus on the field heard".into());
            let _ = uia.RemoveAllEventHandlers();
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        Ok(said)
    }

    pub fn run_child() -> std::process::ExitCode {
        use kompoz_graphics::UnitExt;
        let report = Arc::new(Mutex::new(None));
        let r = report.clone();
        let client = std::thread::spawn(move || {
            let outcome = client();
            if outcome.is_err() {
                // (The app closed whatever happened.)
                // SAFETY: a lookup and a posted message.
                unsafe {
                    let title: Vec<u16> = TITLE.encode_utf16().chain([0]).collect();
                    if let Ok(h) = FindWindowW(w!("KompozWindow"), windows_core::PCWSTR(title.as_ptr())) {
                        let _ = PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
            }
            *r.lock().unwrap() = Some(outcome);
        });
        let code = kompoz_platform::App::new(TITLE).size(360.dp(), 480.dp()).run(Screen);
        let _ = client.join();
        match report.lock().unwrap().take() {
            Some(Ok(lines)) => {
                for l in lines {
                    eprintln!("uia: {l}");
                }
                eprintln!("uia: done");
                code
            }
            Some(Err(why)) => {
                eprintln!("uia: FAILED {why}");
                std::process::ExitCode::FAILURE
            }
            None => {
                eprintln!("uia: FAILED the client didn't report");
                std::process::ExitCode::FAILURE
            }
        }
    }

    pub fn ui_automation_drives_the_app() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .env_remove("KOMPOZ_UIA")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the app starts");
        let deadline = Instant::now() + Duration::from_secs(180);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the app's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the app didn't quit within three minutes");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut log = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut log);
        }
        println!("{log}");
        assert!(status.success() && log.contains("uia: done"), "the client failed ({status}):\n{log}");
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    if std::env::var_os(on_windows::CHILD).is_some() {
        return on_windows::run_child();
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return std::process::ExitCode::SUCCESS;
    }
    let ok = std::panic::catch_unwind(on_windows::ui_automation_drives_the_app).is_ok();
    println!("{} ui_automation_drives_the_app", if ok { "ok    " } else { "FAILED" });
    if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

#[cfg(not(windows))]
fn main() {
    println!("skipped: UI Automation runs on Windows");
}
