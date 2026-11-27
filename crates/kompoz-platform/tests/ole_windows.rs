//! OLE's drag and drop both ways with the app in a real window (`KOMPOZ_DISPLAY_TESTS=1`; on Windows, and under
//! wine here; skipped otherwise), run in a process of its own (this test run again), driven from the app's UI thread
//! by a timer of the test's (outside the app's own step, as OLE's calls come):
//!
//! - Drops in: data objects of the test's dropped through the window's registered drop target, called as a drag's
//!   source calls it (DragEnter, DragOver, DragLeave, Drop: `kompoz_platform::windows::drop_target`). The app's
//!   targets receive text, files and an image (and virtual files: FileGroupDescriptorW with FileContents, written
//!   where the app reads them), read only as they ask; the effect follows the app's answer — a copy over a target that
//!   wants the drag (a move with Shift), none where no target wants it or where the target under it refuses the drop.
//! - Drags out: a drag in the app (the mouse's messages posted to its window) taken past the window's edge goes to
//!   OLE (DoDragDrop) and is dropped on a window of the test's beside it (a popup with a drop target of its own, on
//!   top, the cursor put on it: DoDragDrop finds its target there); the target reads its text through the app's data
//!   object, and the app's source hears the action it took.

#![allow(non_snake_case)]

#[cfg(windows)]
mod on_windows {
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::*;
    use kompoz_runtime::*;
    use kompoz_ui::Modifier;
    use kompoz_ui::draganddrop::{
        DragAndDropEvent, DragAndDropTarget, DragAndDropTransferAction, DragAndDropTransferData,
        drag_and_drop_transfer_data,
    };
    use std::cell::RefCell;
    use std::process::{Command, Stdio};
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{
        DV_E_FORMATETC, DV_E_TYMED, E_NOTIMPL, HGLOBAL, HWND, LPARAM, LRESULT, OLE_E_ADVISENOTSUPPORTED, POINT, POINTL,
        S_OK, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::*;
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows_core::{BOOL, HRESULT, PCWSTR, Ref, Result, implement, w};

    pub const CHILD: &str = "KOMPOZ_OLE_WINDOWS_CHILD";
    const TITLE: &str = "OLE Sürükle";
    const TEXT: &str = "Merhaba, sürükle bırak!";
    const OUT_TEXT: &str = "dışarı sürüklenen";

    // Where things are in the window (px: KOMPOZ_SCALE=1): the target that takes drops (0..200 × 0..200), the one
    // that refuses them (200..400), a strip of no target (200..250), the drag source (0..100 × 250..300).
    const TAKES: (i32, i32) = (60, 60);
    const REFUSES: (i32, i32) = (300, 60);
    const NOTHING: (i32, i32) = (60, 225);
    const SOURCE: (i32, i32) = (50, 275);

    thread_local! {
        /// What the app's targets and source said.
        static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
    }

    fn log(line: String) {
        LOG.with(|l| l.borrow_mut().push(line));
    }

    fn logged() -> Vec<String> {
        LOG.with(|l| l.borrow().clone())
    }

    /// A target logging what is dropped on it (its text, its files with what each holds, its image's size), and
    /// taking it or not.
    struct Logging {
        name: &'static str,
        takes: bool,
    }

    impl DragAndDropTarget for Logging {
        fn on_drop(&self, e: &DragAndDropEvent) -> bool {
            let entry = e.transfer_data.as_ref().map(|d| d.clip_entry.clone()).unwrap_or_default();
            let text = entry.read_plain_text().unwrap_or_default();
            let files: Vec<String> = entry
                .files()
                .iter()
                .map(|f| {
                    let path = std::path::Path::new(f);
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if path.is_dir() {
                        let inside: Vec<String> = std::fs::read_dir(path)
                            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                            .unwrap_or_default();
                        format!("{name}/[{}]", inside.join(","))
                    } else {
                        format!("{name}={}", std::fs::read_to_string(path).unwrap_or_else(|_| "?".into()))
                    }
                })
                .collect();
            let image = entry
                .items()
                .iter()
                .find_map(|i| i.image())
                .map(|i| format!("{}x{}", i.width(), i.height()))
                .unwrap_or_default();
            log(format!("{} drop text={text} files={} image={image}", self.name, files.join(";")));
            self.takes
        }
    }

    /// The app: a target that takes text, files and images (200 × 200), one beside it that wants text but refuses
    /// it at the drop, a strip of no target, and a drag source of text (Copy and Move).
    #[kompozable]
    pub fn Screen() {
        Column(
            modifier = Modifier.fill_max_size().background(Color::White),
            content = || {
                Row(|| {
                    let takes: Rc<dyn DragAndDropTarget> =
                        remember(|| Rc::new(Logging { name: "takes", takes: true }) as Rc<dyn DragAndDropTarget>);
                    Box(modifier = Modifier
                        .size_wh(200.dp(), 200.dp())
                        .background(Color::from_argb(0xffe0e0e0))
                        .drag_and_drop_target(
                            |e: &DragAndDropEvent| {
                                ["text/plain", "text/uri-list", "image/png"].iter().any(|m| e.has_mime_type(m))
                            },
                            takes,
                        ));
                    let refuses: Rc<dyn DragAndDropTarget> =
                        remember(|| Rc::new(Logging { name: "refuses", takes: false }) as Rc<dyn DragAndDropTarget>);
                    Box(modifier = Modifier
                        .size_wh(200.dp(), 200.dp())
                        .drag_and_drop_target(|e: &DragAndDropEvent| e.has_mime_type("text/plain"), refuses));
                });
                Spacer(Modifier.height(50.dp()));
                Box(modifier = Modifier
                    .size_wh(100.dp(), 50.dp())
                    .background(Color::from_argb(0xff3f51b5))
                    .drag_and_drop_source(|_| {
                        Some(DragAndDropTransferData {
                            supported_actions: vec![DragAndDropTransferAction::Copy, DragAndDropTransferAction::Move],
                            on_transfer_completed: Some(Rc::new(|a: Option<DragAndDropTransferAction>| {
                                log(format!("completed {a:?}"))
                            })),
                            ..named_args!(drag_and_drop_transfer_data(
                                text = OUT_TEXT.to_string(),
                                mime_types = vec!["text/plain".into()],
                            ))
                        })
                    }));
            },
        );
    }

    // ---- A source's data, as another app offers it ----------------------------------------------------------------

    fn global(bytes: &[u8]) -> HGLOBAL {
        // SAFETY: a new movable block, written while locked.
        unsafe {
            let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).unwrap();
            let p = GlobalLock(memory);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
            let _ = GlobalUnlock(memory);
            memory
        }
    }

    fn global_bytes(memory: HGLOBAL) -> Vec<u8> {
        // SAFETY: the block locked while read.
        unsafe {
            let p = GlobalLock(memory);
            let bytes = std::slice::from_raw_parts(p as *const u8, GlobalSize(memory)).to_vec();
            let _ = GlobalUnlock(memory);
            bytes
        }
    }

    fn register(name: &str) -> u32 {
        let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: a 0-terminated name.
        unsafe { windows::Win32::System::DataExchange::RegisterClipboardFormatW(PCWSTR(name.as_ptr())) }
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
    }

    /// A DROPFILES of `paths`.
    fn drop_files(paths: &[&str]) -> Vec<u8> {
        let mut b = vec![20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
        for p in paths {
            b.extend(utf16(p));
        }
        b.extend([0, 0]);
        b
    }

    /// A FileGroupDescriptorW of (name, a folder?) — FILEDESCRIPTORW: 592 bytes, attributes at 36, the name at 72.
    fn file_group(files: &[(&str, bool)]) -> Vec<u8> {
        let mut b = (files.len() as u32).to_le_bytes().to_vec();
        for (name, folder) in files {
            let mut d = vec![0u8; 592];
            d[0..4].copy_from_slice(&4u32.to_le_bytes()); // (FD_ATTRIBUTES)
            d[36..40].copy_from_slice(&(if *folder { 0x10u32 } else { 0x80 }).to_le_bytes());
            for (i, u) in name.encode_utf16().enumerate() {
                d[72 + 2 * i..74 + 2 * i].copy_from_slice(&u.to_le_bytes());
            }
            b.extend(d);
        }
        b
    }

    /// A 2 × 1 PNG (its bytes as our encoder makes them).
    fn png() -> Vec<u8> {
        encode_png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]).unwrap()
    }

    /// Another app's data object: formats as global memory, virtual files' contents by index; what was asked for
    /// with GetData logged.
    #[implement(IDataObject)]
    struct TestData {
        formats: Vec<(u32, Vec<u8>)>,
        contents: Vec<Vec<u8>>,
        file_contents: u32,
        asked: Rc<RefCell<Vec<u32>>>,
    }

    impl IDataObject_Impl for TestData_Impl {
        fn GetData(&self, f: *const FORMATETC) -> Result<STGMEDIUM> {
            // SAFETY: the caller's FORMATETC.
            let f = unsafe { *f };
            if f.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
                return Err(DV_E_TYMED.into());
            }
            let format = u32::from(f.cfFormat);
            let bytes = if format == self.file_contents {
                self.contents.get(f.lindex as usize).cloned()
            } else {
                self.formats.iter().find(|(x, _)| *x == format).map(|(_, b)| b.clone())
            };
            let bytes = bytes.ok_or(DV_E_FORMATETC)?;
            self.asked.borrow_mut().push(format);
            Ok(STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: STGMEDIUM_0 { hGlobal: global(&bytes) },
                pUnkForRelease: std::mem::ManuallyDrop::new(None),
            })
        }
        fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> Result<()> {
            Err(E_NOTIMPL.into())
        }
        fn QueryGetData(&self, f: *const FORMATETC) -> HRESULT {
            // SAFETY: the caller's FORMATETC.
            let format = u32::from(unsafe { (*f).cfFormat });
            let has = self.formats.iter().any(|(x, _)| *x == format)
                || (format == self.file_contents && !self.contents.is_empty());
            if has { S_OK } else { DV_E_FORMATETC }
        }
        fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT {
            E_NOTIMPL
        }
        fn SetData(&self, _: *const FORMATETC, _: *const STGMEDIUM, _: BOOL) -> Result<()> {
            Err(E_NOTIMPL.into())
        }
        fn EnumFormatEtc(&self, _: u32) -> Result<IEnumFORMATETC> {
            let mut all: Vec<FORMATETC> = self.formats.iter().map(|(f, _)| etc(*f)).collect();
            if !self.contents.is_empty() {
                all.push(etc(self.file_contents));
            }
            // SAFETY: the shell's enumerator over a copy.
            unsafe { SHCreateStdEnumFmtEtc(&all) }
        }
        fn DAdvise(&self, _: *const FORMATETC, _: u32, _: Ref<IAdviseSink>) -> Result<u32> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
        fn DUnadvise(&self, _: u32) -> Result<()> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
        fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
    }

    fn etc(format: u32) -> FORMATETC {
        FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }

    // ---- A target of the test's, for the app's drag out -----------------------------------------------------------

    /// A target that copies what is dropped, its text read through the source's data object.
    #[implement(IDropTarget)]
    struct TestTarget {
        got: Rc<RefCell<Vec<String>>>,
    }

    impl IDropTarget_Impl for TestTarget_Impl {
        fn DragEnter(&self, _: Ref<IDataObject>, _: MODIFIERKEYS_FLAGS, _: &POINTL, e: *mut DROPEFFECT) -> Result<()> {
            // SAFETY: OLE's effect.
            unsafe { *e = DROPEFFECT_COPY };
            self.got.borrow_mut().push("entered".into());
            Ok(())
        }
        fn DragOver(&self, _: MODIFIERKEYS_FLAGS, _: &POINTL, e: *mut DROPEFFECT) -> Result<()> {
            // SAFETY: OLE's effect.
            unsafe { *e = DROPEFFECT_COPY };
            Ok(())
        }
        fn DragLeave(&self) -> Result<()> {
            self.got.borrow_mut().push("left".into());
            Ok(())
        }
        fn Drop(&self, data: Ref<IDataObject>, _: MODIFIERKEYS_FLAGS, _: &POINTL, e: *mut DROPEFFECT) -> Result<()> {
            let data = data.ok()?;
            // SAFETY: the source's data object read as a target reads it; the medium released.
            let text = unsafe {
                let mut m = data.GetData(&etc(13))?;
                let bytes = global_bytes(m.u.hGlobal);
                ReleaseStgMedium(&mut m);
                let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&p| u16::from_le_bytes(p)).collect();
                let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
                String::from_utf16_lossy(&units[..end])
            };
            self.got.borrow_mut().push(format!("dropped {text}"));
            // SAFETY: OLE's effect.
            unsafe { *e = DROPEFFECT_COPY };
            Ok(())
        }
    }

    // ---- The driver -----------------------------------------------------------------------------------------------

    /// What the test does a tick at a time (the app's UI thread, between its steps).
    struct Driver {
        ticks: u32,
        stage: u32,
        hwnd: Option<HWND>,
        said: Vec<String>,
        popup: Option<HWND>,
        got: Rc<RefCell<Vec<String>>>,
        waited: u32,
    }

    fn screen(hwnd: HWND, (x, y): (i32, i32)) -> POINTL {
        let mut p = POINT { x, y };
        // SAFETY: the app's window; the point written.
        let _ = unsafe { ClientToScreen(hwnd, &mut p) };
        POINTL { x: p.x, y: p.y }
    }

    const MK_LBUTTON: u32 = 1;
    const MK_SHIFT: u32 = 4;

    /// A drop target's calls as a source makes them: the effects it answers.
    struct Source {
        target: IDropTarget,
        hwnd: HWND,
    }

    impl Source {
        fn enter(&self, data: &IDataObject, keys: u32, at: (i32, i32)) -> u32 {
            let mut e = DROPEFFECT_COPY | DROPEFFECT_MOVE;
            // SAFETY: the window's own target, called on its thread as OLE would.
            unsafe { self.target.DragEnter(data, MODIFIERKEYS_FLAGS(keys), screen(self.hwnd, at), &mut e) }.unwrap();
            e.0
        }
        fn over(&self, keys: u32, at: (i32, i32)) -> u32 {
            let mut e = DROPEFFECT_COPY | DROPEFFECT_MOVE;
            // SAFETY: as above.
            unsafe { self.target.DragOver(MODIFIERKEYS_FLAGS(keys), screen(self.hwnd, at), &mut e) }.unwrap();
            e.0
        }
        fn leave(&self) {
            // SAFETY: as above.
            unsafe { self.target.DragLeave() }.unwrap();
        }
        fn drop(&self, data: &IDataObject, keys: u32, at: (i32, i32)) -> u32 {
            let mut e = DROPEFFECT_COPY | DROPEFFECT_MOVE;
            // SAFETY: as above.
            unsafe { self.target.Drop(data, MODIFIERKEYS_FLAGS(keys), screen(self.hwnd, at), &mut e) }.unwrap();
            e.0
        }
    }

    fn check(ok: bool, what: String) -> std::result::Result<String, String> {
        if ok { Ok(format!("ok {what}")) } else { Err(what) }
    }

    /// Drops in, all through the window's drop target.
    fn drops_in(hwnd: HWND) -> std::result::Result<Vec<String>, String> {
        let mut said = Vec::new();
        let target = kompoz_platform::windows::drop_target(hwnd.0 as isize).ok_or("the window has no drop target")?;
        let s = Source { target, hwnd };
        let asked = Rc::new(RefCell::new(Vec::new()));
        let (png_format, html) = (register("PNG"), register("HTML Format"));
        let path = std::env::temp_dir().join("kompoz ole ç.txt");
        std::fs::write(&path, "dosya").map_err(|e| e.to_string())?;
        let path = path.to_string_lossy().into_owned();
        let data: IDataObject = TestData {
            formats: vec![(13, utf16(TEXT)), (15, drop_files(&[&path])), (png_format, png())],
            contents: Vec::new(),
            file_contents: register("FileContents"),
            asked: asked.clone(),
        }
        .into();
        let none = DROPEFFECT_NONE.0;
        let (copy, move_) = (DROPEFFECT_COPY.0, DROPEFFECT_MOVE.0);
        // Over the target that wants it: a copy, a move with Shift; over nothing that wants it: none; over the one
        // that refuses drops (it wants text): a copy offered — nothing read meanwhile.
        let e = s.enter(&data, MK_LBUTTON, TAKES);
        said.push(check(e == copy, format!("entered over the target: effect {e}"))?);
        let e = s.over(MK_LBUTTON, NOTHING);
        said.push(check(e == none, format!("over no target: effect {e}"))?);
        let e = s.over(MK_LBUTTON | MK_SHIFT, TAKES);
        said.push(check(e == move_, format!("back with Shift: effect {e}"))?);
        let e = s.over(MK_LBUTTON, REFUSES);
        said.push(check(e == copy, format!("over the refusing target: effect {e}"))?);
        said.push(check(asked.borrow().is_empty(), format!("nothing read while dragging ({:?})", asked.borrow()))?);
        let e = s.over(MK_LBUTTON, TAKES);
        said.push(check(e == copy, format!("over the target again: effect {e}"))?);
        // Dropped on it: taken (a copy); text, the file and the image read as the target asked.
        let e = s.drop(&data, 0, TAKES);
        let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
        let wanted = format!("takes drop text={TEXT} files={name}=dosya image=2x1");
        let log = logged();
        said.push(check(e == copy && log.last() == Some(&wanted), format!("dropped: effect {e}, {log:?}"))?);
        let read = asked.borrow().clone();
        said.push(check(read.contains(&13) && read.contains(&15), format!("read as asked: {read:?}"))?);
        // On the target that refuses it: none.
        let e = s.enter(&data, MK_LBUTTON, REFUSES);
        said.push(check(e == copy, format!("entered over the refusing target: effect {e}"))?);
        let e = s.drop(&data, 0, REFUSES);
        let log = logged();
        let refused = log.last().is_some_and(|l| l.starts_with("refuses drop text=Merhaba"));
        said.push(check(e == none && refused, format!("refused: effect {e}, {log:?}"))?);
        // Entering where nothing wants it, then leaving: none.
        let e = s.enter(&data, MK_LBUTTON, NOTHING);
        said.push(check(e == none, format!("entered over nothing: effect {e}"))?);
        s.leave();
        // What no target wants (HTML alone): none even over the target.
        let html_only: IDataObject = TestData {
            formats: vec![(html, b"Version:0.9\r\n<b>x</b>\0".to_vec())],
            contents: Vec::new(),
            file_contents: register("FileContents"),
            asked: Rc::new(RefCell::new(Vec::new())),
        }
        .into();
        let e = s.enter(&html_only, MK_LBUTTON, TAKES);
        said.push(check(e == none, format!("HTML alone over the target: effect {e}"))?);
        s.leave();
        // Virtual files (an archive's entries): written where the app reads them, a folder and a file.
        let virtual_files: IDataObject = TestData {
            formats: vec![(
                register("FileGroupDescriptorW"),
                file_group(&[("Ekler", true), ("Ekler\\not.txt", false), ("rapor.txt", false)]),
            )],
            contents: vec![Vec::new(), "içerik".as_bytes().to_vec(), b"rapor".to_vec()],
            file_contents: register("FileContents"),
            asked: Rc::new(RefCell::new(Vec::new())),
        }
        .into();
        let e = s.enter(&virtual_files, MK_LBUTTON, TAKES);
        said.push(check(e == copy, format!("virtual files over the target: effect {e}"))?);
        let e = s.drop(&virtual_files, 0, TAKES);
        let log = logged();
        let wanted = "takes drop text= files=Ekler/[not.txt];rapor.txt=rapor image=";
        said.push(check(
            e == copy && log.last().map(String::as_str) == Some(wanted),
            format!("virtual files: {log:?}"),
        )?);
        let _ = std::fs::remove_file(&path);
        Ok(said)
    }

    unsafe extern "system" fn popup_procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        // SAFETY: Windows' default handling of the popup's messages.
        unsafe { DefWindowProcW(hwnd, message, w, l) }
    }

    /// Where the test's target is: 80 px right of the app's window, at its top's height.
    fn target_point(app: HWND) -> POINT {
        let mut r = windows::Win32::Foundation::RECT::default();
        // SAFETY: the app's window's rectangle written.
        let _ = unsafe { GetWindowRect(app, &mut r) };
        POINT { x: r.right + 80, y: r.top + 80 }
    }

    /// The cursor put on the test's target (where DoDragDrop looks for its target: wine's idea of the cursor goes
    /// back to 0, 0 when the real pointer is over no window of wine's, as on a Wayland desktop) — put there again
    /// each tick while the drag runs.
    fn cursor_on_target(app: HWND) {
        let p = target_point(app);
        // SAFETY: a cursor position.
        let _ = unsafe { SetCursorPos(p.x, p.y) };
    }

    /// A popup of the test's with a drop target, on top beside the app's window, the cursor on it. (A class
    /// of its own: a STATIC control answers WM_NCHITTEST with HTTRANSPARENT, and WindowFromPoint passes through it.)
    fn popup_under_cursor(app: HWND, got: Rc<RefCell<Vec<String>>>) -> Option<HWND> {
        // SAFETY: a window class and a window of the test's on this thread; OLE (the app's) initialized on it.
        unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(popup_procedure),
                lpszClassName: w!("KompozOleTestTarget"),
                ..Default::default()
            };
            RegisterClassExW(&class);
            let p = target_point(app);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                w!("KompozOleTestTarget"),
                w!("hedef"),
                WS_POPUP | WS_VISIBLE,
                p.x - 60,
                p.y - 60,
                120,
                120,
                None,
                None,
                None,
                None,
            )
            .ok()?;
            let target: IDropTarget = TestTarget { got }.into();
            RegisterDragDrop(hwnd, &target).ok()?;
            Some(hwnd)
        }
    }

    fn post(hwnd: HWND, message: u32, w: usize, (x, y): (i32, i32)) {
        let l = ((y as isize & 0xffff) << 16) | (x as isize & 0xffff);
        // SAFETY: the app's window.
        let _ = unsafe { PostMessageW(Some(hwnd), message, WPARAM(w), LPARAM(l)) };
    }

    /// The path the mouse takes from the source out of the window (past its top).
    const PATH: &[(i32, i32)] = &[(50, 268), (50, 255), (50, 230), (50, 180), (50, 120), (50, 60), (50, 10), (50, -40)];

    impl Driver {
        fn finish(&mut self, outcome: std::result::Result<(), String>) {
            match outcome {
                Ok(()) => {
                    for l in &self.said {
                        eprintln!("ole: {l}");
                    }
                    eprintln!("ole: done");
                }
                Err(why) => {
                    for l in &self.said {
                        eprintln!("ole: {l}");
                    }
                    eprintln!("ole: FAILED {why}");
                }
            }
            if let Some(p) = self.popup.take() {
                // SAFETY: the test's popup.
                unsafe {
                    let _ = RevokeDragDrop(p);
                    let _ = DestroyWindow(p);
                }
            }
            if let Some(h) = self.hwnd {
                // SAFETY: the app's window: closed as its close button does.
                let _ = unsafe { PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            }
            self.stage = u32::MAX;
        }

        fn tick(&mut self) {
            self.ticks += 1;
            if self.stage == u32::MAX {
                return;
            }
            if self.ticks > 400 {
                return self.finish(Err(format!("timed out at stage {} ({:?})", self.stage, logged())));
            }
            match self.stage {
                0 => {
                    let title: Vec<u16> = TITLE.encode_utf16().chain([0]).collect();
                    // SAFETY: a lookup by class and title.
                    let found = unsafe { FindWindowW(w!("KompozWindow"), PCWSTR(title.as_ptr())) }.ok();
                    if let Some(h) = found
                        && kompoz_platform::windows::drop_target(h.0 as isize).is_some()
                    {
                        self.hwnd = Some(h);
                        self.stage = 1;
                        self.waited = 0;
                    }
                }
                // (a few frames: the targets laid out)
                1 if self.waited < 8 => self.waited += 1,
                1 => match drops_in(self.hwnd.unwrap()) {
                    Ok(said) => {
                        self.said.extend(said);
                        self.stage = 2;
                    }
                    Err(why) => self.finish(Err(why)),
                },
                2 => {
                    cursor_on_target(self.hwnd.unwrap());
                    let Some(p) = popup_under_cursor(self.hwnd.unwrap(), self.got.clone()) else {
                        return self.finish(Err("no popup under the cursor".into()));
                    };
                    self.popup = Some(p);
                    post(self.hwnd.unwrap(), WM_MOUSEMOVE, 0, SOURCE);
                    self.stage = 3;
                }
                3 => {
                    post(self.hwnd.unwrap(), WM_LBUTTONDOWN, MK_LBUTTON as usize, SOURCE);
                    self.stage = 4;
                    self.waited = 0;
                }
                4 => {
                    cursor_on_target(self.hwnd.unwrap());
                    let i = self.waited as usize;
                    if i < PATH.len() {
                        post(self.hwnd.unwrap(), WM_MOUSEMOVE, MK_LBUTTON as usize, PATH[i]);
                        self.waited += 1;
                    } else {
                        self.stage = 5;
                        self.waited = 0;
                    }
                }
                5 => {
                    cursor_on_target(self.hwnd.unwrap());
                    let completed = logged().into_iter().find(|l| l.starts_with("completed"));
                    let got = self.got.borrow().clone();
                    if let Some(c) = completed {
                        let dropped = got.iter().any(|g| *g == format!("dropped {OUT_TEXT}"));
                        let ok = dropped && c == "completed Some(Copy)";
                        let what = format!("dragged out: the target {got:?}, the source {c}");
                        let outcome = if ok {
                            self.said.push(format!("ok {what}"));
                            Ok(())
                        } else {
                            Err(what)
                        };
                        return self.finish(outcome);
                    }
                    self.waited += 1;
                    if self.waited > 100 {
                        self.finish(Err(format!("the drag out didn't end (target {got:?}, log {:?})", logged())));
                    }
                }
                _ => {}
            }
        }
    }

    unsafe extern "system" fn tick(_: HWND, _: u32, _: usize, _: u32) {
        DRIVER.with(|d| {
            if let Ok(mut d) = d.try_borrow_mut()
                && let Some(d) = d.as_mut()
            {
                d.tick();
            }
        });
    }

    pub fn run_child() -> std::process::ExitCode {
        DRIVER.with(|d| {
            *d.borrow_mut() = Some(Driver {
                ticks: 0,
                stage: 0,
                hwnd: None,
                said: Vec::new(),
                popup: None,
                got: Rc::new(RefCell::new(Vec::new())),
                waited: 0,
            })
        });
        // SAFETY: a thread timer: its callback runs on this (the app's UI) thread as the loop dispatches it.
        unsafe { SetTimer(None, 0, 100, Some(tick)) };
        kompoz_platform::App::new(TITLE).size(400.dp(), 300.dp()).run(Screen)
    }

    pub fn ole_drags_both_ways() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
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
        assert!(status.success() && log.contains("ole: done"), "the drags failed ({status}):\n{log}");
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
    let ok = std::panic::catch_unwind(on_windows::ole_drags_both_ways).is_ok();
    println!("{} ole_drags_both_ways", if ok { "ok    " } else { "FAILED" });
    if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

#[cfg(not(windows))]
fn main() {
    println!("skipped: OLE runs on Windows");
}
