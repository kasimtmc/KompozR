//! OLE's drag and drop on Windows, both ways (C++'s IDropTarget, W:299-327 and 1167-1208, made lazy and answered by
//! the app; the drags out it lacked), at Linux's parity (Wayland's data device and drag source, X11's XDND).
//!
//! Drops in: each window registers a drop target ([`DropTarget`]). A drag from another app coming over it is told to
//! the window's core as Linux tells it (`DropBegin` with the MIME types its data object's formats are read as, then
//! `DropMove`s; `DropComplete` alone as it leaves), and the app is stepped there and then — inside OLE's call, as
//! Windows' modal loops step it — so that the effect the source is answered with is the app's: none where no
//! target under the pointer wants the drag ([`Platform::drop_over`](crate::app::Platform::drop_over)), and at the
//! drop none unless a target took it (`drop_done`); else the action the keys held ask for among those the source
//! allows (Windows' own: Ctrl copies, Shift moves, Ctrl+Shift or Alt links; with none another app's is copied).
//! What a drop carries is read only as a target asks for each type, from the data object kept until the core says
//! the drop is done, as Linux keeps its offer: text, HTML, images (PNG, CF_DIBV5, CF_DIB), files (CF_HDROP, and
//! virtual files — FileGroupDescriptorW with FileContents as global memory, a stream or a storage: an e-mail's
//! attachments, a zip's entries — written into a folder of the drop's under the temporary folder, removed as the
//! app ends), URLs, the types of KompozR apps (`formats`). Other apps' drag images are shown over the window
//! (IDropTargetHelper).
//!
//! Drags out: a drag within the app that leaves the window is handed to OLE (`PlatformCallbacks::start_drag_and_drop`
//! queues it; [`drag`] runs DoDragDrop once the step that asked is over: Windows' modal loop, the app's frames
//! stepped meanwhile by the window's modal timer). Its data object offers what the drag carries in the clipboard's
//! formats (the same mapping, [`Formats::offered`]), each made only when a target asks for it (delayed rendering);
//! what targets and the shell set on it kept (the drag image's formats, "Performed DropEffect" of a target that moved
//! files itself). Its image is the decoration the app draws under the pointer, through IDragSourceHelper; its source
//! (IDropSource) drops it as the buttons that started it are let go, calls it off with Escape or another button; the
//! effect the target took ends the app's drag (`OwnDragEnded`). Back over its own window, it is the app's own
//! (`DropOwn`: nothing read, the host has what it carries; moved by default, as within the app).
//!
//! An FFI module: each `unsafe` block says why it holds. Everything here is the UI thread's (OLE's objects are its
//! apartment's: calls from other processes come through its message loop).

#![allow(unsafe_code)]

use super::clipboard::SystemFormats;
use super::formats::{
    self, DROPEFFECT_NONE, Formats, MK_BUTTONS, MK_LBUTTON, MK_MBUTTON, MK_RBUTTON, Medium, Offered, Source,
};
use super::sys::{self, Hwnd};
use crate::app::Event;
use crate::transfer::{Payload, premultiplied_argb};
use ::windows::Win32::Foundation::{
    COLORREF, DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_DVASPECT,
    DV_E_FORMATETC, DV_E_TYMED, E_INVALIDARG, E_NOTIMPL, E_OUTOFMEMORY, HWND, OLE_E_ADVISENOTSUPPORTED, POINT, POINTL,
    S_OK, SIZE,
};
use ::windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP, HGDIOBJ,
};
use ::windows::Win32::System::Com::StructuredStorage::{IStorage, StgCreateDocfile};
use ::windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject,
    IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, IStream, STGC_DEFAULT, STGM_CREATE, STGM_READWRITE,
    STGM_SHARE_EXCLUSIVE, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL, TYMED_ISTORAGE, TYMED_ISTREAM,
};
use ::windows::Win32::System::Ole::{
    DROPEFFECT, DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget, IDropTarget_Impl, OleInitialize,
    OleUninitialize, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use ::windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use ::windows::Win32::UI::Shell::{
    CLSID_DragDropHelper, IDragSourceHelper, IDropTargetHelper, SHCreateStdEnumFmtEtc, SHDRAGIMAGE,
};
use ::windows_core::{BOOL, ComObject, HRESULT, IUnknown, Interface, PCWSTR, Ref, Result, implement};
use kompoz_ui::clipboard::{ClipDataReader, ClipEntry, ClipItem, ClipMetadata};
use kompoz_ui::draganddrop::{DragAndDropTransferAction, PlatformDragAndDrop};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::PathBuf;
use std::rc::Rc;

/// The virtual keys of the mouse's buttons (winuser.h).
const VK_LBUTTON: u16 = 0x01;
const VK_RBUTTON: u16 = 0x02;
const VK_MBUTTON: u16 = 0x04;

thread_local! {
    /// The windows' registered drop targets, by their handles' values (for [`drop_target`]).
    static TARGETS: RefCell<Vec<(isize, IDropTarget)>> = const { RefCell::new(Vec::new()) };
    /// The folders dropped virtual files were written into (removed as OLE is let go: the app ends).
    static DROP_FOLDERS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static NEXT_FOLDER: Cell<u32> = const { Cell::new(0) };
}

// ---- OLE on the UI thread ---------------------------------------------------------------------------------------

/// OLE initialized on the UI thread (its drag and drop needs it; COM apartment-threaded with it: TSF's call after
/// counts once more); whether it could be (each success balanced by [`uninitialize`]).
pub fn initialize() -> bool {
    // SAFETY: OleInitialize on this thread (S_FALSE: already: counted, balanced all the same).
    unsafe { OleInitialize(None) }.is_ok()
}

/// OLE let go (the platform closes): the folders of dropped virtual files removed.
pub fn uninitialize() {
    for folder in DROP_FOLDERS.with(|f| std::mem::take(&mut *f.borrow_mut())) {
        let _ = std::fs::remove_dir_all(folder);
    }
    // SAFETY: balances a successful `initialize` on this thread (the windows' targets revoked before).
    unsafe { OleUninitialize() };
}

fn hwnd_of(hwnd: Hwnd) -> HWND {
    HWND(hwnd.value() as *mut c_void)
}

/// `target` registered as `hwnd`'s (the window drags come over); whether OLE took it.
pub fn register(hwnd: Hwnd, target: IDropTarget) -> bool {
    // SAFETY: the window is ours (this thread's); OLE keeps a reference to the target until it is revoked.
    let registered = unsafe { RegisterDragDrop(hwnd_of(hwnd), &target) }.is_ok();
    if registered {
        TARGETS.with(|t| t.borrow_mut().push((hwnd.value(), target)));
    }
    registered
}

/// `hwnd`'s target revoked (the window closes).
pub fn revoke(hwnd: Hwnd) {
    let had = TARGETS.with(|t| {
        let mut t = t.borrow_mut();
        let before = t.len();
        t.retain(|(h, _)| *h != hwnd.value());
        t.len() != before
    });
    if had {
        // SAFETY: the window's own registration (made by `register`), let go once.
        let _ = unsafe { RevokeDragDrop(hwnd_of(hwnd)) };
    }
}

/// The drop target of the app's window `hwnd` (its handle's value) in this process, as OLE hands it to a drag's
/// source; none where the window has none. For in-process tools and tests that drop through it as a source does: on
/// the UI thread only, outside the app's own step (a timer's callback, a message's handling).
pub fn drop_target(hwnd: isize) -> Option<IDropTarget> {
    TARGETS.with(|t| t.borrow().iter().find(|(h, _)| *h == hwnd).map(|(_, target)| target.clone()))
}

// ---- Drops in ---------------------------------------------------------------------------------------------------

/// What a drop target needs of its window.
pub trait DropSite {
    fn hwnd(&self) -> Hwnd;
    /// The drag's events handed to the window's core and the app stepped to handle them at once (what its targets
    /// answer is then in the window's [`DropAnswers`]).
    fn deliver(&self, events: Vec<Event>);
    /// The app's own drag out of this window, while it runs.
    fn own_drag(&self) -> Option<OwnDrag>;
}

/// The app's own drag under way: its data object (what it is known by, back over its window), what it carries.
#[derive(Clone)]
pub struct OwnDrag {
    pub object: IDataObject,
    pub payload: Rc<Payload>,
}

/// What a window's core answered of a drag over it (its platform's `drop_over`, `take_drop_entry`, `drop_done`),
/// for its drop target.
#[derive(Default)]
pub struct DropAnswers {
    /// Whether a target that wants the drag is under it, as the core last said (taken by the target).
    over: Cell<Option<bool>>,
    /// Whether the drop was taken, once the core said.
    done: Cell<Option<bool>>,
    /// What the drop carries, until the core takes it.
    entry: RefCell<Option<ClipEntry>>,
    /// The dropped data object (its entry reads from it), let go when the drop is done.
    dropped: RefCell<Option<Rc<RefCell<Option<IDataObject>>>>>,
}

impl DropAnswers {
    /// The core's `drop_over`.
    pub fn over(&self, wanted: bool) {
        self.over.set(Some(wanted));
    }

    /// The core's `take_drop_entry`.
    pub fn take_entry(&self) -> Option<ClipEntry> {
        self.entry.borrow_mut().take()
    }

    /// The core's `drop_done`: the dropped data let go (its entry reads nothing after, as Linux's finished offer).
    pub fn done(&self, taken: bool) {
        self.done.set(Some(taken));
        self.entry.borrow_mut().take();
        if let Some(slot) = self.dropped.borrow_mut().take() {
            slot.borrow_mut().take();
        }
    }
}

/// A drag over the window: whether it is the app's own, where it is (px in the window), what the core said of it
/// there.
struct Over {
    own: Option<OwnDrag>,
    at: (f32, f32),
    wanted: Option<bool>,
}

/// A window's drop target (registered with OLE as the window opens).
#[implement(IDropTarget)]
pub struct DropTarget {
    site: Box<dyn DropSite>,
    answers: Rc<DropAnswers>,
    formats: Formats,
    over: RefCell<Option<Over>>,
    /// The shell's helper showing the source's drag image over the window, once made (none: there is none).
    helper: RefCell<Option<Option<IDropTargetHelper>>>,
}

impl DropTarget {
    /// The target of a window (`site`) whose core answers into `answers`.
    pub fn create(site: Box<dyn DropSite>, answers: Rc<DropAnswers>, formats: Formats) -> IDropTarget {
        DropTarget { site, answers, formats, over: RefCell::new(None), helper: RefCell::new(None) }.into()
    }

    /// A point on the screen in the window (px, as its events have them).
    fn client(&self, pt: &POINTL) -> (f32, f32) {
        let (x, y) = self.site.hwnd().screen_to_client(pt.x, pt.y);
        (x as f32, y as f32)
    }

    /// The app's own drag, if `data` is its data object.
    fn own(&self, data: &IDataObject) -> Option<OwnDrag> {
        self.site.own_drag().filter(|o| same_object(&o.object, data))
    }

    fn helper(&self) -> Option<IDropTargetHelper> {
        self.helper
            .borrow_mut()
            .get_or_insert_with(|| {
                // SAFETY: the shell's helper made on this (apartment-threaded) thread; none where it isn't there.
                unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) }.ok()
            })
            .clone()
    }

    /// The MIME types a drag offers: the app's own drag's, else as its formats are read.
    fn mime_types(&self, data: &IDataObject, own: Option<&OwnDrag>) -> Vec<String> {
        let types = match own {
            Some(o) => o.payload.offered(formats::TEXT_TYPES),
            None => self.formats.mime_types(&formats_of(data), &SystemFormats),
        };
        ClipMetadata::of_offered(&types).mime_types().to_vec()
    }

    /// What a dropped data object carries, read from it as a target asks for each type until the drop is done (its
    /// slot then emptied); none if it carries nothing the app reads.
    fn entry_of(&self, data: &IDataObject) -> Option<(ClipEntry, Rc<RefCell<Option<IDataObject>>>)> {
        let types = self.formats.mime_types(&formats_of(data), &SystemFormats);
        let metadata = ClipMetadata::of_offered(&types);
        let mime_types = metadata.mime_types().to_vec();
        if mime_types.is_empty() {
            return None;
        }
        let slot = Rc::new(RefCell::new(Some(data.clone())));
        let reader = DataReader { data: slot.clone(), formats: self.formats, files: RefCell::new(None) };
        let entry = ClipEntry::with_metadata(vec![ClipItem::from_reader(mime_types, Rc::new(reader))], metadata);
        Some((entry, slot))
    }
}

/// The effect a drag over the window gets: none where the app said nothing would take it there, else the one the
/// keys ask for among those allowed (also where the app couldn't be asked: re-entered while it steps, as C++ did).
fn answer(allowed: u32, keys: u32, own: bool, wanted: Option<bool>) -> u32 {
    if wanted == Some(false) { DROPEFFECT_NONE } else { formats::drop_effect(allowed, keys, own) }
}

/// Whether two interface pointers are one COM object (their IUnknowns are the same).
fn same_object(a: &IDataObject, b: &IDataObject) -> bool {
    matches!((a.cast::<IUnknown>(), b.cast::<IUnknown>()), (Ok(a), Ok(b)) if a == b)
}

/// The effect OLE passed in.
fn effect_in(effect: *mut DROPEFFECT) -> Result<u32> {
    if effect.is_null() {
        return Err(E_INVALIDARG.into());
    }
    // SAFETY: OLE's in-out effect, checked above.
    Ok(unsafe { (*effect).0 })
}

fn set_effect(effect: *mut DROPEFFECT, value: u32) {
    // SAFETY: OLE's in-out effect (checked by `effect_in` first).
    unsafe { *effect = DROPEFFECT(value) };
}

fn point(pt: &POINTL) -> POINT {
    POINT { x: pt.x, y: pt.y }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let data = data.ok()?.clone();
        let allowed = effect_in(effect)?;
        let own = self.own(&data);
        let mime_types = self.mime_types(&data, own.as_ref());
        let (x, y) = self.client(pt);
        let is_own = own.is_some();
        *self.over.borrow_mut() = Some(Over { own, at: (x, y), wanted: None });
        self.answers.over.set(None);
        self.site.deliver(vec![Event::DropBegin { x, y, mime_types }, Event::DropMove { x, y }]);
        let wanted = self.answers.over.take();
        if let Some(o) = self.over.borrow_mut().as_mut() {
            o.wanted = wanted;
        }
        let e = answer(allowed, keys.0, is_own, wanted);
        set_effect(effect, e);
        if let Some(helper) = self.helper() {
            // SAFETY: the shell's helper, told of the drag over our window with OLE's own arguments.
            let _ = unsafe { helper.DragEnter(hwnd_of(self.site.hwnd()), &data, &point(pt), DROPEFFECT(e)) };
        }
        Ok(())
    }

    fn DragOver(&self, keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let allowed = effect_in(effect)?;
        let at = self.client(pt);
        let (moved, own) = match self.over.borrow().as_ref() {
            Some(o) => (o.at != at || o.wanted.is_none(), o.own.is_some()),
            None => {
                set_effect(effect, DROPEFFECT_NONE);
                return Ok(());
            }
        };
        // (OLE asks again and again while the pointer rests: the core told only where it moved)
        if moved {
            self.answers.over.set(None);
            self.site.deliver(vec![Event::DropMove { x: at.0, y: at.1 }]);
            let wanted = self.answers.over.take();
            if let Some(o) = self.over.borrow_mut().as_mut() {
                o.at = at;
                o.wanted = wanted;
            }
        }
        let wanted = self.over.borrow().as_ref().and_then(|o| o.wanted);
        let e = answer(allowed, keys.0, own, wanted);
        set_effect(effect, e);
        if let Some(helper) = self.helper() {
            // SAFETY: as in DragEnter.
            let _ = unsafe { helper.DragOver(&point(pt), DROPEFFECT(e)) };
        }
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        let Some(o) = self.over.borrow_mut().take() else { return Ok(()) };
        if let Some(helper) = self.helper() {
            // SAFETY: as in DragEnter.
            let _ = unsafe { helper.DragLeave() };
        }
        // (complete alone: it left, as Linux tells it)
        self.site.deliver(vec![Event::DropComplete { x: o.at.0, y: o.at.1 }]);
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let data = data.ok()?.clone();
        let allowed = effect_in(effect)?;
        self.over.borrow_mut().take();
        let own = self.own(&data).is_some();
        let (x, y) = self.client(pt);
        self.answers.done.set(None);
        let mut events = vec![Event::DropMove { x, y }];
        if own {
            events.push(Event::DropOwn { x, y });
        } else if let Some((entry, slot)) = self.entry_of(&data) {
            *self.answers.entry.borrow_mut() = Some(entry);
            *self.answers.dropped.borrow_mut() = Some(slot);
            events.push(Event::DropData { x, y });
        }
        events.push(Event::DropComplete { x, y });
        self.site.deliver(events);
        // (none unless a target took it; not answered — re-entered while the app steps —: as the keys ask)
        let e = answer(allowed, keys.0, own, self.answers.done.take());
        set_effect(effect, e);
        if let Some(helper) = self.helper() {
            // SAFETY: as in DragEnter.
            let _ = unsafe { helper.Drop(&data, &point(pt), DROPEFFECT(e)) };
        }
        Ok(())
    }
}

/// A FORMATETC asking for `format`'s content (item `index`: a virtual file's; -1: all) as one of `tymed`.
fn format_etc(format: u32, index: i32, tymed: i32) -> FORMATETC {
    FORMATETC {
        cfFormat: format as u16,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: index,
        tymed: tymed as u32,
    }
}

/// The formats a data object offers (as global memory or a stream), in its order.
fn formats_of(data: &IDataObject) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    // SAFETY: the data object's own enumerator, read a format at a time into a FORMATETC of ours (its target device,
    // if any, freed: the caller owns it).
    unsafe {
        let Ok(e) = data.EnumFormatEtc(DATADIR_GET.0 as u32) else { return out };
        loop {
            let mut f = [FORMATETC::default()];
            let mut fetched = 0u32;
            if e.Next(&mut f, Some(&mut fetched)) != S_OK || fetched == 0 {
                break;
            }
            if !f[0].ptd.is_null() {
                ::windows::Win32::System::Com::CoTaskMemFree(Some(f[0].ptd as *const c_void));
            }
            let format = u32::from(f[0].cfFormat);
            let readable = f[0].tymed & (TYMED_HGLOBAL.0 | TYMED_ISTREAM.0 | TYMED_ISTORAGE.0) as u32 != 0;
            if format != 0 && readable && !out.contains(&format) {
                out.push(format);
            }
        }
    }
    out
}

/// A medium's bytes (global memory's, up to `size` where it is known: its block is rounded up; a stream's, all of
/// it), the medium released; none for another medium.
fn medium_bytes(mut medium: STGMEDIUM, size: Option<u64>) -> Option<Vec<u8>> {
    // SAFETY: a medium the data object gave us (ours to release, once, below); its union read as its tymed says.
    unsafe {
        let bytes = if medium.tymed == TYMED_HGLOBAL.0 as u32 {
            sys::global_bytes(medium.u.hGlobal).map(|mut b| {
                if let Some(size) = size {
                    b.truncate(size as usize);
                }
                b
            })
        } else if medium.tymed == TYMED_ISTREAM.0 as u32 {
            medium.u.pstm.as_ref().map(read_stream)
        } else {
            None
        };
        ReleaseStgMedium(&mut medium);
        bytes
    }
}

/// A stream read to its end.
fn read_stream(stream: &IStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let mut read = 0u32;
        // SAFETY: up to the chunk's length written into it; `read` says how much.
        let hr = unsafe { stream.Read(chunk.as_mut_ptr() as *mut c_void, chunk.len() as u32, Some(&mut read)) };
        if hr.is_err() || read == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..read as usize]);
    }
    out
}

/// A dropped data object's types read as a target asks for them, until the drop is done (its slot emptied).
struct DataReader {
    data: Rc<RefCell<Option<IDataObject>>>,
    formats: Formats,
    /// Its virtual files once written out (each drop's once).
    files: RefCell<Option<Option<Vec<String>>>>,
}

impl ClipDataReader for DataReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        let data = self.data.borrow().clone()?;
        let source = ObjectSource { data: &data, formats: self.formats, files: &self.files };
        self.formats.read(&source, mime_type, &SystemFormats)
    }
}

/// A data object as another app's data, by format.
struct ObjectSource<'a> {
    data: &'a IDataObject,
    formats: Formats,
    files: &'a RefCell<Option<Option<Vec<String>>>>,
}

impl ObjectSource<'_> {
    fn medium(&self, format: u32, index: i32, tymed: i32) -> Option<STGMEDIUM> {
        // SAFETY: the data object's GetData with a FORMATETC of ours; the medium is ours to release.
        unsafe { self.data.GetData(&format_etc(format, index, tymed)) }.ok()
    }

    /// Virtual file `index`'s contents written at `path` (global memory, a stream, or a storage: an Outlook
    /// message's): whether it was.
    fn write_contents(&self, index: usize, size: Option<u64>, path: &std::path::Path) -> bool {
        let tymed = TYMED_HGLOBAL.0 | TYMED_ISTREAM.0 | TYMED_ISTORAGE.0;
        let Some(mut medium) = self.medium(self.formats.file_contents, index as i32, tymed) else { return false };
        if medium.tymed == TYMED_ISTORAGE.0 as u32 {
            let name = sys::wide(&path.to_string_lossy());
            // SAFETY: the medium's storage (its tymed says), copied into a compound file of ours, then the medium
            // released once.
            unsafe {
                let copied = medium.u.pstg.as_ref().is_some_and(|source: &IStorage| {
                    let mode = STGM_CREATE | STGM_READWRITE | STGM_SHARE_EXCLUSIVE;
                    StgCreateDocfile(PCWSTR(name.as_ptr()), mode, None).is_ok_and(|target| {
                        source.CopyTo(None, None, &target).is_ok() && target.Commit(STGC_DEFAULT.0 as u32).is_ok()
                    })
                });
                ReleaseStgMedium(&mut medium);
                return copied;
            }
        }
        medium_bytes(medium, size).is_some_and(|bytes| std::fs::write(path, bytes).is_ok())
    }
}

impl Source for ObjectSource<'_> {
    fn has(&self, format: u32) -> bool {
        let tymed = TYMED_HGLOBAL.0 | TYMED_ISTREAM.0;
        // SAFETY: the data object's QueryGetData with a FORMATETC of ours.
        let answer = unsafe { self.data.QueryGetData(&format_etc(format, -1, tymed)) };
        answer == S_OK
    }

    fn get(&self, format: u32) -> Option<Vec<u8>> {
        medium_bytes(self.medium(format, -1, TYMED_HGLOBAL.0 | TYMED_ISTREAM.0)?, None)
    }

    /// Written into a folder of the drop's of its own (under the temporary folder, removed as the app ends), each
    /// under the name it gives (none that would leave the folder), its folders made: the paths of the top ones.
    fn virtual_files(&self) -> Option<Vec<String>> {
        if let Some(done) = self.files.borrow().clone() {
            return done;
        }
        let written = (|| {
            let descriptor = self.get(self.formats.file_group)?;
            let files = formats::virtual_files_of(&descriptor);
            let n = NEXT_FOLDER.with(|n| n.replace(n.get() + 1));
            let folder = std::env::temp_dir().join(format!("kompoz-drop-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&folder).ok()?;
            DROP_FOLDERS.with(|f| f.borrow_mut().push(folder.clone()));
            let mut top: Vec<String> = Vec::new();
            for (index, file) in files.iter().enumerate() {
                let Some(parts) = formats::safe_parts(&file.name) else { continue };
                let path = parts.iter().fold(folder.clone(), |p, part| p.join(part));
                let made = if file.directory {
                    std::fs::create_dir_all(&path).is_ok()
                } else {
                    path.parent().is_some_and(|p| std::fs::create_dir_all(p).is_ok())
                        && self.write_contents(index, file.size, &path)
                };
                let first = folder.join(&parts[0]).to_string_lossy().into_owned();
                if made && !top.contains(&first) {
                    top.push(first);
                }
            }
            Some(top)
        })();
        *self.files.borrow_mut() = Some(written.clone());
        written
    }
}

// ---- Drags out --------------------------------------------------------------------------------------------------

/// What the app's drag carries, as OLE's targets read it: its formats made as they are asked for.
#[implement(IDataObject)]
struct DataObject {
    payload: Rc<Payload>,
    formats: Formats,
    offered: Offered,
    /// What targets and the shell set on it (the drag image's formats, "Performed DropEffect"), by format.
    set: RefCell<Vec<(u32, Vec<u8>)>>,
}

impl DataObject {
    fn has(&self, format: u32) -> bool {
        self.set.borrow().iter().any(|(f, _)| *f == format)
            || self.offered.given.iter().any(|(f, _)| *f == format)
            || self.offered.promised.iter().any(|(f, _)| *f == format)
    }

    /// `format`'s bytes: what was set on it, given, else made now from what the drag carries.
    fn bytes(&self, format: u32) -> Option<Vec<u8>> {
        if let Some((_, b)) = self.set.borrow().iter().find(|(f, _)| *f == format) {
            return Some(b.clone());
        }
        if let Some((_, b)) = self.offered.given.iter().find(|(f, _)| *f == format) {
            return Some(b.clone());
        }
        let (_, mime_type) = self.offered.promised.iter().find(|(f, _)| *f == format)?;
        self.formats.render(&self.payload, format, mime_type, &SystemFormats)
    }

    /// The formats it has, as FORMATETCs (global memory).
    fn format_etcs(&self) -> Vec<FORMATETC> {
        let promised = self.offered.promised.iter().map(|(f, _)| *f);
        let given = self.offered.given.iter().map(|(f, _)| *f);
        let set: Vec<u32> = self.set.borrow().iter().map(|(f, _)| *f).collect();
        let mut out: Vec<FORMATETC> = Vec::new();
        for f in promised.chain(given).chain(set) {
            if !out.iter().any(|e| u32::from(e.cfFormat) == f) {
                out.push(format_etc(f, -1, TYMED_HGLOBAL.0));
            }
        }
        out
    }
}

/// A FORMATETC the object answers: content, as global memory.
fn check(format: *const FORMATETC) -> std::result::Result<FORMATETC, HRESULT> {
    if format.is_null() {
        return Err(E_INVALIDARG);
    }
    // SAFETY: the caller's FORMATETC, checked above, copied.
    let f = unsafe { *format };
    if f.dwAspect != DVASPECT_CONTENT.0 {
        return Err(DV_E_DVASPECT);
    }
    if f.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
        return Err(DV_E_TYMED);
    }
    Ok(f)
}

impl IDataObject_Impl for DataObject_Impl {
    fn GetData(&self, format: *const FORMATETC) -> Result<STGMEDIUM> {
        let f = check(format)?;
        let bytes = self.bytes(u32::from(f.cfFormat)).ok_or(DV_E_FORMATETC)?;
        let memory = sys::global_of(&bytes).ok_or(E_OUTOFMEMORY)?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: memory },
            pUnkForRelease: ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn QueryGetData(&self, format: *const FORMATETC) -> HRESULT {
        match check(format) {
            Ok(f) if self.has(u32::from(f.cfFormat)) => S_OK,
            Ok(_) => DV_E_FORMATETC,
            Err(e) => e,
        }
    }

    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, out: *mut FORMATETC) -> HRESULT {
        if !out.is_null() {
            // SAFETY: the caller's FORMATETC to write (the same format: no target device).
            unsafe { (*out).ptd = std::ptr::null_mut() };
        }
        DATA_S_SAMEFORMATETC
    }

    /// Kept (global memory's): what the shell's drag image helper puts there, what a target says it did.
    fn SetData(&self, format: *const FORMATETC, medium: *const STGMEDIUM, release: BOOL) -> Result<()> {
        if format.is_null() || medium.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: the caller's FORMATETC and medium, checked above; the medium's union read as its tymed says, and
        // released (ours once SetData succeeds with `release`).
        unsafe {
            let (format, m) = (u32::from((*format).cfFormat), &*medium);
            if m.tymed != TYMED_HGLOBAL.0 as u32 {
                return Err(DV_E_TYMED.into());
            }
            let bytes = sys::global_bytes(m.u.hGlobal).ok_or(E_INVALIDARG)?;
            {
                let mut set = self.set.borrow_mut();
                set.retain(|(f, _)| *f != format);
                set.push((format, bytes));
            }
            if release.as_bool() {
                ReleaseStgMedium(medium as *mut STGMEDIUM);
            }
        }
        Ok(())
    }

    fn EnumFormatEtc(&self, direction: u32) -> Result<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        // SAFETY: the shell's standard enumerator over a copy of our FORMATETCs.
        unsafe { SHCreateStdEnumFmtEtc(&self.format_etcs()) }
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

/// The drag's source: dropped as the buttons that started it are let go, called off with Escape or another button.
#[implement(IDropSource)]
struct DropSource {
    buttons: u32,
}

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        let held = keys.0 & MK_BUTTONS;
        if escape.as_bool() || held & !self.buttons != 0 {
            DRAGDROP_S_CANCEL
        } else if held & self.buttons == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// The mouse's buttons held now, as a drag's key state has them (none: the left one, let go already: dropped at once).
fn held_buttons() -> u32 {
    let mut held = 0;
    for (vk, mk) in [(VK_LBUTTON, MK_LBUTTON), (VK_RBUTTON, MK_RBUTTON), (VK_MBUTTON, MK_MBUTTON)] {
        if sys::key_down(vk) {
            held |= mk;
        }
    }
    if held == 0 { MK_LBUTTON } else { held }
}

/// The decoration as the drag's image (the shell's helper keeps it on the data object; it shows it under the
/// pointer over every window, as Explorer's drags show): a 32-bit top-down bitmap of premultiplied pixels, the
/// pointer where it is in the decoration. The helper (kept while the drag runs), or none where nothing draws the
/// decoration or the shell has no helper (wine).
fn drag_image(object: &IDataObject, drag: &PlatformDragAndDrop) -> Option<IDragSourceHelper> {
    let image = drag.render_icon(1.0)?;
    let (w, h) = (image.width(), image.height());
    if w <= 0 || h <= 0 {
        return None;
    }
    let argb = premultiplied_argb(&image, w, h);
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // (top-down)
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
    // SAFETY: the section's memory (w × h words, BGRA little-endian: premultiplied ARGB words as they are) written
    // while the bitmap lives; the helper owns the bitmap once it took it, else it is deleted here.
    unsafe {
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap: HBITMAP = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return None;
        }
        std::ptr::copy_nonoverlapping(argb.as_ptr(), bits as *mut u32, argb.len());
        let helper: Option<IDragSourceHelper> =
            CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER).ok();
        let offset = drag.decoration_offset;
        let shdi = SHDRAGIMAGE {
            sizeDragImage: SIZE { cx: w, cy: h },
            ptOffset: POINT { x: offset.x.round() as i32, y: offset.y.round() as i32 },
            hbmpDragImage: bitmap,
            crColorKey: COLORREF(0xFFFF_FFFF), // (CLR_NONE: the alpha decides)
        };
        let taken = helper.as_ref().is_some_and(|h| h.InitializeFromBitmap(&shdi, object).is_ok());
        if !taken {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return None;
        }
        helper
    }
}

/// The app's drag `drag` out of its window through OLE: its data object and source made, its image given, then
/// DoDragDrop — Windows' modal loop until it is dropped or called off (the window's timer steps the app meanwhile;
/// `started` is told the drag first, so that the window knows it back over itself). The action the target took (a
/// target that moved files itself answers none but says so on the data object: a move); none: not taken.
pub fn drag(
    formats: Formats,
    drag: &PlatformDragAndDrop,
    started: impl FnOnce(OwnDrag),
) -> Option<DragAndDropTransferAction> {
    let payload = Rc::new(Payload::new(&drag.transfer_data));
    let offered = formats.offered(&payload, Medium::Drag, &SystemFormats);
    let allowed = formats::effects_of(&payload.actions);
    let data = ComObject::new(DataObject { payload: payload.clone(), formats, offered, set: RefCell::default() });
    let object: IDataObject = data.to_interface();
    let source: IDropSource = DropSource { buttons: held_buttons() }.into();
    let _helper = drag_image(&object, drag);
    started(OwnDrag { object: object.clone(), payload });
    let mut effect = DROPEFFECT(DROPEFFECT_NONE);
    // SAFETY: our data object and source, alive through the call; OLE's modal loop runs on this thread.
    let hr = unsafe { DoDragDrop(&object, &source, DROPEFFECT(allowed), &mut effect) };
    if hr != DRAGDROP_S_DROP {
        return None;
    }
    formats::action_of(effect.0).or_else(|| {
        let performed = data.bytes(formats.performed_effect)?;
        let performed = u32::from_le_bytes(performed.get(..4)?.try_into().ok()?);
        formats::action_of(performed)
    })
}

#[cfg(test)]
mod tests {
    //! The data object of a drag out read as a target reads it (its formats enumerated, each made when asked for,
    //! what is set on it kept), on Windows and under wine. (Drops in and DoDragDrop end to end: tests/ole_windows.)
    use super::*;
    use kompoz_ui::clipboard::{ClipEntry, ClipItem};
    use kompoz_ui::draganddrop::DragAndDropTransferData;

    #[test]
    fn a_drags_data_object_makes_its_formats_as_they_are_asked_for() {
        assert!(initialize());
        let formats = Formats::register(&SystemFormats);
        let entry = ClipEntry::new(vec![ClipItem::with_html_text("kalın yazı", "<b>kalın</b> yazı")]);
        let data = DragAndDropTransferData { clip_entry: entry, ..Default::default() };
        let payload = Rc::new(Payload::new(&data));
        let offered = formats.offered(&payload, Medium::Drag, &SystemFormats);
        let object: IDataObject =
            DataObject { payload: payload.clone(), formats, offered, set: RefCell::default() }.into();
        let read = |format: u32| {
            // SAFETY: our data object's GetData; the medium released by medium_bytes.
            unsafe { object.GetData(&format_etc(format, -1, TYMED_HGLOBAL.0)) }.ok().and_then(|m| medium_bytes(m, None))
        };
        let listed = formats_of(&object);
        assert_eq!(listed[..3], [formats::CF_UNICODETEXT, formats::CF_TEXT, formats.html], "{listed:?}");
        assert_eq!(formats::utf16_text(&read(formats::CF_UNICODETEXT).unwrap()), "kalın yazı");
        let html = formats::nul_terminated(&read(formats.html).unwrap());
        assert_eq!(formats::html_of_cf_html(&html), "<b>kalın</b> yazı");
        assert!(read(formats.png).is_none());
        // SAFETY: as above.
        let streamed = unsafe { object.QueryGetData(&format_etc(formats.html, -1, TYMED_ISTREAM.0)) };
        assert_eq!(streamed, DV_E_TYMED);
        // What a target sets on it (global memory, released by the object) is kept and read back, listed too.
        let performed = sys::global_of(&2u32.to_le_bytes()).unwrap();
        let medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: performed },
            pUnkForRelease: ManuallyDrop::new(None),
        };
        // SAFETY: a medium of ours handed over (released by the object).
        unsafe { object.SetData(&format_etc(formats.performed_effect, -1, TYMED_HGLOBAL.0), &medium, true) }.unwrap();
        assert_eq!(read(formats.performed_effect).unwrap()[..4], 2u32.to_le_bytes());
        assert!(formats_of(&object).contains(&formats.performed_effect));
        // As a dropped object is read by the app: lazily, nothing once the drop is done.
        let answers = DropAnswers::default();
        let slot = Rc::new(RefCell::new(Some(object.clone())));
        *answers.dropped.borrow_mut() = Some(slot.clone());
        let reader = DataReader { data: slot, formats, files: RefCell::new(None) };
        assert_eq!(reader.read("text/plain").unwrap(), "kalın yazı".as_bytes());
        answers.done(true);
        assert_eq!(reader.read("text/plain"), None);
        drop((object, reader));
        uninitialize();
    }
}
