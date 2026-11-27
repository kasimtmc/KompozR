//! The Text Services Framework (TSF) as the Windows platform's input method path uses it where it can: the UI
//! thread's thread manager activated once ([`ThreadManager`]: its keystroke manager fed the window's keys before
//! they are translated, as a TSF-aware program's loop does), and for the focused field a document manager with a
//! context of its own over KompozR's text store ([`FieldContext`], [`TextStore`]: ITextStoreACP and
//! ITextStoreACP2, its lock protocol — granted at once, or queued while one is held and granted when it ends,
//! TS_E_SYNCHRONOUS for a synchronous one then —, the text services' compositions through
//! ITfContextOwnerCompositionSink, their display attributes read at the end of each edit through ITfTextEditSink),
//! the field's input scopes as its ITfInputScope ([`InputScopes`]), a password's keyboard disabled. What the text
//! services — the desktop's input methods, the touch keyboard, handwriting, voice typing — do to the document comes
//! back to the platform once their lock is released ([`Owner::edited`]); what the app does to the field is told
//! to them (OnTextChange, OnSelectionChange, OnLayoutChange; a composition the app ended terminated).
//!
//! The document's model and its translation to the core's events are plain Rust (`ime`); this is the COM shell.
//! An FFI module: each `unsafe` block says why it holds. COM's objects are the thread's (an apartment-threaded UI
//! thread): no borrow of the store's state is held across a call into TSF, which may call back into it.

#![allow(unsafe_code)]

use super::ime::{self, Document};
use super::sys::{Hwnd, Message};
use ::windows::Win32::Foundation::{
    E_FAIL, E_INVALIDARG, E_NOTIMPL, HWND, LPARAM, POINT, RECT, RPC_E_CHANGED_MODE, WPARAM,
};
use ::windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoUninitialize,
    FORMATETC, IDataObject,
};
use ::windows::Win32::System::Ole::{CONNECT_E_ADVISELIMIT, CONNECT_E_NOCONNECTION};
use ::windows::Win32::System::Variant::{VARIANT, VT_I4, VT_UNKNOWN, VariantClear};
use ::windows::Win32::UI::TextServices::*;
use ::windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP};
use ::windows_core::{BOOL, BSTR, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Ref, Result, implement};
use kompoz_graphics::Rect;
use kompoz_ui::platform::{ImeOptions, TextEditState};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// What the text store answers to and tells (the platform's input method on the window).
pub trait Owner {
    /// A text service's edit is done (its lock released): the document as the core is to have it.
    fn edited(&self, presented: TextEditState);
    /// The caret's box, px in the window's client area.
    fn caret(&self) -> Rect;
}

// ---- The thread manager -----------------------------------------------------------------------------------------

/// The UI thread's TSF: its thread manager (activated: the text services' client id), its keystroke manager, the
/// category and display attribute managers the compositions' styles are read through, an empty document manager
/// focused where no field takes typing (the text services then have nothing to edit).
pub struct ThreadManager {
    manager: ITfThreadMgr,
    client: u32,
    keystrokes: Option<ITfKeystrokeMgr>,
    categories: Option<ITfCategoryMgr>,
    display_attributes: Option<ITfDisplayAttributeMgr>,
    empty: Option<ITfDocumentMgr>,
    /// COM initialized here (balanced at the end).
    initialized: bool,
}

thread_local! {
    /// The thread's TSF once tried: none where it can't be had (no COM, no msctf, `KOMPOZ_WIN_IME=imm32`).
    static THREAD: RefCell<Option<Option<Rc<ThreadManager>>>> = const { RefCell::new(None) };
}

impl ThreadManager {
    /// The thread's TSF, activated the first time it is asked for; none where TSF isn't there.
    pub fn get() -> Option<Rc<ThreadManager>> {
        if let Some(tried) = THREAD.with(|t| t.borrow().clone()) {
            return tried;
        }
        let made = ThreadManager::activate().map(Rc::new);
        THREAD.with(|t| *t.borrow_mut() = Some(made.clone()));
        made
    }

    /// The thread's TSF if it was activated (nothing tried here).
    pub fn current() -> Option<Rc<ThreadManager>> {
        THREAD.with(|t| t.borrow().clone().flatten())
    }

    fn activate() -> Option<ThreadManager> {
        // SAFETY: COM initialized for the thread (apartment-threaded, as a UI thread's; already so: counted once
        // more, balanced in `shutdown`), the thread manager made and activated; the other managers are optional.
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if hr == RPC_E_CHANGED_MODE || hr.is_err() {
                return None;
            }
            let made = (|| -> Result<ThreadManager> {
                let manager: ITfThreadMgr = CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
                let client = manager.Activate()?;
                let keystrokes = manager.cast::<ITfKeystrokeMgr>().ok();
                let categories = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER).ok();
                let display_attributes =
                    CoCreateInstance(&CLSID_TF_DisplayAttributeMgr, None, CLSCTX_INPROC_SERVER).ok();
                let empty = manager.CreateDocumentMgr().ok();
                Ok(ThreadManager {
                    manager,
                    client,
                    keystrokes,
                    categories,
                    display_attributes,
                    empty,
                    initialized: true,
                })
            })();
            match made {
                Ok(m) => Some(m),
                Err(_) => {
                    CoUninitialize();
                    None
                }
            }
        }
    }

    /// The thread's TSF deactivated (the platform closes): what it was is forgotten.
    pub fn shutdown() {
        let Some(Some(m)) = THREAD.with(|t| t.borrow_mut().take()) else { return };
        // SAFETY: the thread manager activated in `activate`; COM uninitialized as many times as it was here.
        unsafe {
            let _ = m.manager.Deactivate();
        }
        let initialized = m.initialized;
        drop(m);
        if initialized {
            // SAFETY: balances `activate`'s CoInitializeEx (the managers released above).
            unsafe { CoUninitialize() };
        }
    }

    /// The window's focus given to `documents` (none: the empty document manager) when it gets the keyboard's
    /// focus; now, where it has it.
    fn focus(&self, hwnd: Hwnd, documents: Option<&ITfDocumentMgr>) {
        let documents = documents.or(self.empty.as_ref());
        // SAFETY: the thread manager's calls on this thread; the window's handle checked by TSF.
        unsafe {
            let _ = self.manager.AssociateFocus(HWND(hwnd.value() as *mut _), documents);
            if hwnd.has_focus()
                && let Some(d) = documents
            {
                let _ = self.manager.SetFocus(d);
            }
        }
    }
}

/// A key message given to the text services first (a TSF-aware program's loop: the keystroke manager's test, then
/// its handling): whether one of them ate it (it isn't translated or dispatched then).
pub fn keystroke(message: &Message) -> bool {
    let Some(m) = ThreadManager::current() else { return false };
    let Some(keys) = &m.keystrokes else { return false };
    let (w, l) = (WPARAM(message.wparam()), LPARAM(message.lparam()));
    // SAFETY: the keystroke manager's calls on this thread with the message's own parameters.
    unsafe {
        match message.id() {
            WM_KEYDOWN => {
                keys.TestKeyDown(w, l).is_ok_and(|eaten| eaten.as_bool())
                    && keys.KeyDown(w, l).is_ok_and(|eaten| eaten.as_bool())
            }
            WM_KEYUP => {
                keys.TestKeyUp(w, l).is_ok_and(|eaten| eaten.as_bool())
                    && keys.KeyUp(w, l).is_ok_and(|eaten| eaten.as_bool())
            }
            _ => false,
        }
    }
}

// ---- The focused field's context ----------------------------------------------------------------------------------

/// The focused field's document manager and context over its text store (pushed, the window's focus associated
/// with it); popped and the window's focus given back to the empty document manager when dropped.
pub struct FieldContext {
    manager: Rc<ThreadManager>,
    hwnd: Hwnd,
    documents: ITfDocumentMgr,
    context: ITfContext,
    store: ::windows_core::ComObject<TextStore>,
    edit_sink: Option<u32>,
}

impl FieldContext {
    /// A context for the field (`state`, `options`) on `hwnd`, told to `owner`; none where TSF won't make one.
    pub fn new(
        manager: Rc<ThreadManager>,
        hwnd: Hwnd,
        state: &TextEditState,
        options: &ImeOptions,
        owner: Weak<dyn Owner>,
    ) -> Option<FieldContext> {
        let store =
            ::windows_core::ComObject::new(TextStore::new(hwnd, owner, state, options, Rc::downgrade(&manager)));
        let unknown: IUnknown = store.to_interface();
        // SAFETY: the document manager and context made and pushed on this thread; out parameters written by the
        // calls.
        unsafe {
            let documents = manager.manager.CreateDocumentMgr().ok()?;
            let mut context = None;
            let mut cookie = 0;
            documents.CreateContext(manager.client, 0, &unknown, &mut context, &mut cookie).ok()?;
            let context = context?;
            documents.Push(&context).ok()?;
            let edit_sink = context.cast::<ITfSource>().ok().and_then(|source| {
                let sink: ITfTextEditSink = store.to_interface();
                source.AdviseSink(&ITfTextEditSink::IID, &sink).ok()
            });
            let field = FieldContext { manager, hwnd, documents, context, store, edit_sink };
            field.set_keyboard_disabled(ime::hides_text(options));
            field.manager.focus(hwnd, Some(&field.documents));
            Some(field)
        }
    }

    /// The field's options again: its input scopes, its keyboard disabled for a password.
    pub fn set_options(&self, options: &ImeOptions) {
        *self.store.options.borrow_mut() = options.clone();
        self.set_keyboard_disabled(ime::hides_text(options));
    }

    /// The text services kept off the field (GUID_COMPARTMENT_KEYBOARD_DISABLED: a password's composing text
    /// would show what is typed).
    fn set_keyboard_disabled(&self, disabled: bool) {
        // SAFETY: the context's compartment set on this thread; the VARIANT is a plain I4.
        unsafe {
            let Ok(compartments) = self.context.cast::<ITfCompartmentMgr>() else { return };
            let Ok(compartment) = compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_DISABLED) else { return };
            let mut value = VARIANT::default();
            (*value.Anonymous.Anonymous).vt = VT_I4;
            (*value.Anonymous.Anonymous).Anonymous.lVal = disabled as i32;
            let _ = compartment.SetValue(self.manager.client, &value);
        }
    }

    /// The field moved (its caret's box): the text services told (OnLayoutChange).
    pub fn layout_changed(&self) {
        self.store.layout_changed();
    }

    /// The app changed the field (or took less of what was typed): a composition going on terminated (the text
    /// service ends it as it stands; what it does meanwhile isn't the app's), the document made the field, the text
    /// services told.
    pub fn changed_by_app(&self, state: &TextEditState) {
        if self.store.composing() {
            self.store.terminating.set(true);
            // SAFETY: the context's composition services, on this thread (TSF calls back into the store).
            unsafe {
                if let Ok(services) = self.context.cast::<ITfContextOwnerCompositionServices>() {
                    let _ = services.TerminateComposition(None);
                }
            }
            self.store.terminating.set(false);
        }
        self.store.changed_by_app(state);
    }

    /// The text store (what tests drive as TSF would).
    #[cfg(test)]
    pub fn store(&self) -> &TextStore {
        &self.store
    }

    #[cfg(test)]
    pub fn context(&self) -> &ITfContext {
        &self.context
    }
}

impl Drop for FieldContext {
    fn drop(&mut self) {
        // SAFETY: the calls undo `new`'s on this thread.
        unsafe {
            if self.store.composing()
                && let Ok(services) = self.context.cast::<ITfContextOwnerCompositionServices>()
            {
                self.store.terminating.set(true);
                let _ = services.TerminateComposition(None);
            }
            if let (Some(cookie), Ok(source)) = (self.edit_sink, self.context.cast::<ITfSource>()) {
                let _ = source.UnadviseSink(cookie);
            }
            self.manager.focus(self.hwnd, None);
            let _ = self.documents.Pop(TF_POPF_ALL);
        }
    }
}

// ---- The text store ---------------------------------------------------------------------------------------------

/// What TSF's sink was told nothing of yet (the app changed the field while a lock was held): the text's change
/// (TS_TEXTCHANGE), the selection, the layout.
#[derive(Clone, Copy, Default, Debug)]
struct Pending {
    text: Option<(usize, usize, usize)>,
    selection: bool,
    layout: bool,
}

/// KompozR's text store: the focused field's document as the text services read and edit it under the locks TSF
/// asks for, its composition and display attributes, its input scopes.
#[implement(ITextStoreACP, ITextStoreACP2, ITfContextOwnerCompositionSink, ITfTextEditSink)]
pub struct TextStore {
    hwnd: Hwnd,
    owner: Weak<dyn Owner>,
    manager: Weak<ThreadManager>,
    sink: RefCell<Option<(ITextStoreACPSink, u32)>>,
    /// The lock held (TS_LF_READ, TS_LF_READWRITE), 0 for none.
    lock: Cell<u32>,
    /// The asynchronous lock asked for while one was held (the strongest), 0 for none.
    queued: Cell<u32>,
    document: RefCell<Document>,
    composition: RefCell<Option<ITfCompositionView>>,
    /// A composition the app ended is being terminated: what the text service does meanwhile isn't sent.
    terminating: Cell<bool>,
    pending: Cell<Pending>,
    options: RefCell<ImeOptions>,
    /// The attributes asked for (RequestAttrsAtPosition, RequestSupportedAttrs): the input scope's, if it was.
    asked_input_scope: Cell<bool>,
    /// A test's composition (units): wine's TSF has no compositions (ITfContextComposition) to make views with.
    #[cfg(test)]
    composed: Cell<Option<(usize, usize)>>,
}

/// Whether a lock `held` lets one read (`write` false) or write.
fn allows(held: u32, write: bool) -> bool {
    let need = if write { TS_LF_READWRITE.0 } else { TS_LF_READ.0 };
    held & need == need
}

impl TextStore {
    fn new(
        hwnd: Hwnd,
        owner: Weak<dyn Owner>,
        state: &TextEditState,
        options: &ImeOptions,
        manager: Weak<ThreadManager>,
    ) -> TextStore {
        TextStore {
            hwnd,
            owner,
            manager,
            sink: RefCell::new(None),
            lock: Cell::new(0),
            queued: Cell::new(0),
            document: RefCell::new(Document::of(state)),
            composition: RefCell::new(None),
            terminating: Cell::new(false),
            pending: Cell::new(Pending::default()),
            options: RefCell::new(options.clone()),
            asked_input_scope: Cell::new(false),
            #[cfg(test)]
            composed: Cell::new(None),
        }
    }

    /// The document as it is (tests).
    pub fn document(&self) -> Document {
        self.document.borrow().clone()
    }

    fn composing(&self) -> bool {
        #[cfg(test)]
        if self.composed.get().is_some() {
            return true;
        }
        self.composition.borrow().is_some()
    }

    fn check(&self, write: bool) -> Result<()> {
        if allows(self.lock.get(), write) { Ok(()) } else { Err(TS_E_NOLOCK.into()) }
    }

    /// The composition's range now (its view's: units), none without one.
    fn composition_range(&self) -> Option<(usize, usize)> {
        #[cfg(test)]
        if let Some(range) = self.composed.get() {
            return Some(range);
        }
        let view = self.composition.borrow().clone()?;
        // SAFETY: the view's range read on this thread (ITfRangeACP's extent needs no edit cookie).
        unsafe {
            let range = view.GetRange().ok()?.cast::<ITfRangeACP>().ok()?;
            let (mut anchor, mut count) = (0, 0);
            range.GetExtent(&mut anchor, &mut count).ok()?;
            let anchor = usize::try_from(anchor).ok()?;
            Some((anchor, anchor + usize::try_from(count).ok()?))
        }
    }

    /// `flags` granted: the sink's OnLockGranted with the lock held; its answer.
    fn grant(&self, sink: &ITextStoreACPSink, flags: u32) -> HRESULT {
        self.lock.set(flags);
        // SAFETY: the sink's call on this thread; it calls back into the store, which holds no borrow meanwhile.
        let r = unsafe { sink.OnLockGranted(TEXT_STORE_LOCK_FLAGS(flags)) };
        self.lock.set(0);
        match r {
            Ok(()) => HRESULT(0),
            Err(e) => e.code(),
        }
    }

    /// A lock released: the composition as its view now has it, what the text services did sent to the core (not
    /// while a composition the app ended is terminated), what the sink wasn't told while it was held told now.
    fn released(&self) {
        let range = self.composition_range();
        let presented = {
            let mut document = self.document.borrow_mut();
            document.set_composition(range);
            document.presented()
        };
        if !self.terminating.get()
            && let Some(owner) = self.owner.upgrade()
        {
            owner.edited(presented);
        }
        self.tell();
    }

    /// What the sink wasn't told yet, told (none while a lock is held).
    fn tell(&self) {
        if self.lock.get() != 0 {
            return;
        }
        let pending = self.pending.take();
        let Some((sink, mask)) = self.sink.borrow().clone() else { return };
        // SAFETY: the sink's notifications on this thread, no lock held (it may ask for one meanwhile).
        unsafe {
            if let Some((start, old_end, new_end)) = pending.text
                && mask & TS_AS_TEXT_CHANGE != 0
            {
                let change =
                    TS_TEXTCHANGE { acpStart: start as i32, acpOldEnd: old_end as i32, acpNewEnd: new_end as i32 };
                let _ = sink.OnTextChange(TEXT_STORE_TEXT_CHANGE_FLAGS(0), &change);
            }
            if pending.selection && mask & TS_AS_SEL_CHANGE != 0 {
                let _ = sink.OnSelectionChange();
            }
            if pending.layout && mask & TS_AS_LAYOUT_CHANGE != 0 {
                let _ = sink.OnLayoutChange(TS_LC_CHANGE, 0);
            }
        }
    }

    /// The app's field: the document made it, the change told (once no lock is held).
    fn changed_by_app(&self, state: &TextEditState) {
        let (old, new) = {
            let mut document = self.document.borrow_mut();
            let old = document.state.text.clone();
            *document = Document::of(state);
            (old, document.state.text.clone())
        };
        *self.composition.borrow_mut() = None;
        #[cfg(test)]
        self.composed.set(None);
        let mut pending = self.pending.get();
        if let Some(change) = ime::text_change(&old, &new) {
            pending.text = Some(match pending.text {
                // (Two changes untold: the whole text.)
                Some(_) => (0, ime::utf16_len(&old).max(change.1), ime::utf16_len(&new)),
                None => change,
            });
        }
        pending.selection = true;
        self.pending.set(pending);
        self.tell();
    }

    fn layout_changed(&self) {
        let mut pending = self.pending.get();
        pending.layout = true;
        self.pending.set(pending);
        self.tell();
    }

    /// The caret's box on the screen.
    fn caret_on_screen(&self) -> RECT {
        let area = self.owner.upgrade().map_or(Rect::default(), |o| o.caret());
        let p = ime::char_position(area, self.hwnd.client_origin());
        RECT { left: p.left, top: p.top, right: p.right, bottom: p.bottom }
    }

    /// The text store's static flags: no hidden text; the touch keyboard's corrections and predictions where the
    /// field wants them.
    fn status(&self) -> TS_STATUS {
        let mut flags = TS_SS_NOHIDDENTEXT;
        if self.options.borrow().auto_correct {
            flags |= TS_SS_TKBAUTOCORRECTENABLE | TS_SS_TKBPREDICTIONENABLE;
        }
        TS_STATUS { dwDynamicFlags: 0, dwStaticFlags: flags }
    }

    fn advise_sink(&self, riid: *const GUID, punk: Ref<IUnknown>, mask: u32) -> Result<()> {
        // SAFETY: TSF passes a valid IID.
        if unsafe { *riid } != ITextStoreACPSink::IID {
            return Err(E_INVALIDARG.into());
        }
        let sink: ITextStoreACPSink = punk.ok()?.cast()?;
        let mut current = self.sink.borrow_mut();
        match current.as_mut() {
            Some((s, m)) if s.cast::<IUnknown>()? == sink.cast::<IUnknown>()? => *m = mask,
            Some(_) => return Err(CONNECT_E_ADVISELIMIT.into()),
            None => *current = Some((sink, mask)),
        }
        Ok(())
    }

    fn unadvise_sink(&self, punk: Ref<IUnknown>) -> Result<()> {
        let unknown: IUnknown = punk.ok()?.cast()?;
        let mut current = self.sink.borrow_mut();
        match current.as_ref() {
            Some((s, _)) if s.cast::<IUnknown>()? == unknown => {
                *current = None;
                Ok(())
            }
            _ => Err(CONNECT_E_NOCONNECTION.into()),
        }
    }

    /// RequestLock: granted now where none is held (then the asynchronous ones asked for meanwhile), else queued
    /// (TS_S_ASYNC) or refused if synchronous (TS_E_SYNCHRONOUS).
    fn request_lock(&self, flags: u32) -> Result<HRESULT> {
        let Some((sink, _)) = self.sink.borrow().clone() else { return Err(E_FAIL.into()) };
        let kind = flags & TS_LF_READWRITE.0;
        if self.lock.get() != 0 {
            if flags & TS_LF_SYNC != 0 {
                return Ok(TS_E_SYNCHRONOUS);
            }
            self.queued.set(self.queued.get() | kind);
            return Ok(TS_S_ASYNC);
        }
        let answer = self.grant(&sink, kind);
        loop {
            let queued = self.queued.replace(0);
            if queued == 0 {
                break;
            }
            let _ = self.grant(&sink, queued);
        }
        self.released();
        Ok(answer)
    }

    fn get_selection(&self, index: u32, count: u32, selection: *mut TS_SELECTION_ACP, fetched: *mut u32) -> Result<()> {
        self.check(false)?;
        if index != 0 && index != TF_DEFAULT_SELECTION {
            return Err(TS_E_NOSELECTION.into());
        }
        if selection.is_null() || fetched.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let document = self.document.borrow();
        let (start, end) = document.selection();
        let ase = match (start == end, document.active_start) {
            (true, _) => TS_AE_NONE,
            (false, true) => TS_AE_START,
            (false, false) => TS_AE_END,
        };
        // SAFETY: TSF's out parameters: at least `count` selections, the count fetched.
        unsafe {
            if count == 0 {
                *fetched = 0;
                return Ok(());
            }
            *selection = TS_SELECTION_ACP {
                acpStart: start as i32,
                acpEnd: end as i32,
                style: TS_SELECTIONSTYLE { ase, fInterimChar: false.into() },
            };
            *fetched = 1;
        }
        Ok(())
    }

    fn set_selection(&self, count: u32, selection: *const TS_SELECTION_ACP) -> Result<()> {
        self.check(true)?;
        if count != 1 || selection.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: TSF passes `count` selections.
        let s = unsafe { *selection };
        let end = self.document.borrow().end() as i32;
        if s.acpStart < 0 || s.acpEnd < s.acpStart || s.acpEnd > end {
            return Err(TS_E_INVALIDPOS.into());
        }
        let active_start = s.style.ase == TS_AE_START;
        self.document.borrow_mut().set_selection(s.acpStart as usize, s.acpEnd as usize, active_start);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn get_text(
        &self,
        start: i32,
        end: i32,
        plain: PWSTR,
        plain_wanted: u32,
        plain_got: *mut u32,
        runs: *mut TS_RUNINFO,
        runs_wanted: u32,
        runs_got: *mut u32,
        next: *mut i32,
    ) -> Result<()> {
        self.check(false)?;
        let document = self.document.borrow();
        let len = document.end() as i32;
        if start < 0 || start > len || (end != -1 && (end < start || end > len)) {
            return Err(TS_E_INVALIDPOS.into());
        }
        let text = document.text_of(start as usize, (end != -1).then_some(end as usize));
        let n = text.len().min(plain_wanted as usize);
        // SAFETY: TSF's buffers: `plain_wanted` units of text, `runs_wanted` runs, and the counts (each written
        // where it isn't null).
        unsafe {
            if n > 0 && !plain.0.is_null() {
                std::ptr::copy_nonoverlapping(text.as_ptr(), plain.0, n);
            }
            if !plain_got.is_null() {
                *plain_got = n as u32;
            }
            let run = runs_wanted > 0 && !runs.is_null() && n > 0;
            if run {
                *runs = TS_RUNINFO { uCount: n as u32, r#type: TS_RT_PLAIN };
            }
            if !runs_got.is_null() {
                *runs_got = run as u32;
            }
            if !next.is_null() {
                *next = start + n as i32;
            }
        }
        Ok(())
    }

    fn set_text(&self, start: i32, end: i32, text: &PCWSTR, count: u32) -> Result<TS_TEXTCHANGE> {
        self.check(true)?;
        let len = self.document.borrow().end() as i32;
        if start < 0 || end < start || end > len {
            return Err(TS_E_INVALIDPOS.into());
        }
        let text = units(text, count);
        let (a, b, c) = self.document.borrow_mut().replace(start as usize, end as usize, &text);
        Ok(TS_TEXTCHANGE { acpStart: a as i32, acpOldEnd: b as i32, acpNewEnd: c as i32 })
    }

    fn query_insert(&self, start: i32, end: i32, result_start: *mut i32, result_end: *mut i32) -> Result<()> {
        let len = self.document.borrow().end() as i32;
        if start < 0 || end < start || start > len {
            return Err(E_INVALIDARG.into());
        }
        let (a, b) = self.document.borrow().query_insert(start as usize, end as usize);
        // SAFETY: TSF's out parameters.
        unsafe {
            if !result_start.is_null() {
                *result_start = a as i32;
            }
            if !result_end.is_null() {
                *result_end = b as i32;
            }
        }
        Ok(())
    }

    fn insert_text_at_selection(
        &self,
        flags: u32,
        text: &PCWSTR,
        count: u32,
        start: *mut i32,
        end: *mut i32,
        change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        let (s0, s1) = self.document.borrow().selection();
        if flags & TS_IAS_QUERYONLY != 0 {
            self.check(false)?;
            // SAFETY: TSF's out parameters.
            unsafe {
                if !start.is_null() {
                    *start = s0 as i32;
                }
                if !end.is_null() {
                    *end = s1 as i32;
                }
            }
            return Ok(());
        }
        self.check(true)?;
        let text = units(text, count);
        let (a, b, c) = self.document.borrow_mut().replace(s0, s1, &text);
        // SAFETY: TSF's out parameters (null with TS_IAS_NOQUERY).
        unsafe {
            if !start.is_null() {
                *start = a as i32;
            }
            if !end.is_null() {
                *end = c as i32;
            }
            if !change.is_null() {
                *change = TS_TEXTCHANGE { acpStart: a as i32, acpOldEnd: b as i32, acpNewEnd: c as i32 };
            }
        }
        Ok(())
    }

    /// Attributes asked for: the input scope's the one the store has.
    fn ask_attributes(&self, count: u32, attributes: *const GUID) {
        let asked = if attributes.is_null() || count == 0 {
            &[][..]
        } else {
            // SAFETY: TSF passes `count` GUIDs.
            unsafe { std::slice::from_raw_parts(attributes, count as usize) }
        };
        self.asked_input_scope.set(asked.contains(&GUID_PROP_INPUTSCOPE));
    }

    fn retrieve_attributes(&self, count: u32, values: *mut TS_ATTRVAL, fetched: *mut u32) -> Result<()> {
        if values.is_null() || fetched.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let give = count > 0 && self.asked_input_scope.replace(false);
        // SAFETY: TSF's out parameters: `count` attribute values (a VARIANT with our ITfInputScope, its reference
        // the caller's to release), the count fetched.
        unsafe {
            *fetched = 0;
            if give {
                let scopes: ITfInputScope = InputScopes(ime::input_scopes(&self.options.borrow())).into();
                let mut value = VARIANT::default();
                (*value.Anonymous.Anonymous).vt = VT_UNKNOWN;
                (*value.Anonymous.Anonymous).Anonymous.punkVal =
                    std::mem::ManuallyDrop::new(Some(scopes.cast::<IUnknown>()?));
                values.write(TS_ATTRVAL { idAttr: GUID_PROP_INPUTSCOPE, dwOverlapId: 0, varValue: value });
                *fetched = 1;
            }
        }
        Ok(())
    }

    fn text_ext(&self, clipped: *mut BOOL, rect: *mut RECT) -> Result<()> {
        self.check(false)?;
        let r = self.caret_on_screen();
        // SAFETY: TSF's out parameters.
        unsafe {
            if !rect.is_null() {
                *rect = r;
            }
            if !clipped.is_null() {
                *clipped = false.into();
            }
        }
        Ok(())
    }

    fn screen_ext(&self) -> RECT {
        let (x, y) = self.hwnd.client_origin();
        let (w, h) = self.hwnd.client_size();
        RECT { left: x, top: y, right: x + w, bottom: y + h }
    }

    /// The character at a point of the screen: the caret's where it is in its box, none elsewhere (KompozR's text
    /// layout isn't the platform's to ask).
    fn acp_from_point(&self, point: *const POINT) -> Result<i32> {
        self.check(false)?;
        // SAFETY: TSF passes a point.
        let p = unsafe { *point };
        let r = self.caret_on_screen();
        if p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom {
            Ok(self.document.borrow().selection().1 as i32)
        } else {
            Err(TS_E_INVALIDPOINT.into())
        }
    }

    /// The display attributes of the composition's runs read (with the edit's read-only cookie): their TF_ATTR_*
    /// kept with the document.
    fn read_display_attributes(&self, context: &ITfContext, cookie: u32) {
        let Some(view) = self.composition.borrow().clone() else { return };
        let Some(manager) = self.manager.upgrade() else { return };
        let (Some(categories), Some(display)) = (&manager.categories, &manager.display_attributes) else { return };
        let mut runs = Vec::new();
        // SAFETY: the context's property read with the edit's cookie, on this thread; each VARIANT cleared.
        unsafe {
            let Ok(property) = context.GetProperty(&GUID_PROP_ATTRIBUTE) else { return };
            let Ok(range) = view.GetRange() else { return };
            let mut ranges = None;
            if property.EnumRanges(cookie, &mut ranges, &range).is_err() {
                return;
            }
            let Some(ranges) = ranges else { return };
            loop {
                let mut one = [None];
                let mut got = 0;
                if ranges.Next(&mut one, &mut got).is_err() || got == 0 {
                    break;
                }
                let Some(r) = one[0].take() else { break };
                let Ok(mut value) = property.GetValue(cookie, &r) else { continue };
                let atom = ((*value.Anonymous.Anonymous).vt == VT_I4)
                    .then(|| (*value.Anonymous.Anonymous).Anonymous.lVal as u32);
                let _ = VariantClear(&mut value);
                let Some(atom) = atom else { continue };
                let Ok(guid) = categories.GetGUID(atom) else { continue };
                let mut info = None;
                let mut owner = GUID::zeroed();
                if display.GetDisplayAttributeInfo(&guid, &mut info, &mut owner).is_err() {
                    continue;
                }
                let Some(info) = info else { continue };
                let mut attribute = TF_DISPLAYATTRIBUTE::default();
                if info.GetAttributeInfo(&mut attribute).is_err() {
                    continue;
                }
                let Ok(acp) = r.cast::<ITfRangeACP>() else { continue };
                let (mut anchor, mut count) = (0, 0);
                if acp.GetExtent(&mut anchor, &mut count).is_ok() && anchor >= 0 && count > 0 {
                    let a = anchor as usize;
                    runs.push((a, a + count as usize, attribute.bAttr.0.clamp(0, 255) as u8));
                }
            }
        }
        self.document.borrow_mut().attributes = runs;
    }
}

/// TSF's string (`count` units) as ours.
fn units(text: &PCWSTR, count: u32) -> Vec<u16> {
    if text.is_null() || count == 0 {
        return Vec::new();
    }
    // SAFETY: TSF passes `count` units of text.
    unsafe { std::slice::from_raw_parts(text.0, count as usize) }.to_vec()
}

impl ITextStoreACP_Impl for TextStore_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>, mask: u32) -> Result<()> {
        self.advise_sink(riid, punk, mask)
    }
    fn UnadviseSink(&self, punk: Ref<IUnknown>) -> Result<()> {
        self.unadvise_sink(punk)
    }
    fn RequestLock(&self, flags: u32) -> Result<HRESULT> {
        self.request_lock(flags)
    }
    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(self.status())
    }
    fn QueryInsert(&self, start: i32, end: i32, _count: u32, rs: *mut i32, re: *mut i32) -> Result<()> {
        self.query_insert(start, end, rs, re)
    }
    fn GetSelection(&self, index: u32, count: u32, s: *mut TS_SELECTION_ACP, fetched: *mut u32) -> Result<()> {
        self.get_selection(index, count, s, fetched)
    }
    fn SetSelection(&self, count: u32, s: *const TS_SELECTION_ACP) -> Result<()> {
        self.set_selection(count, s)
    }
    fn GetText(
        &self,
        start: i32,
        end: i32,
        plain: PWSTR,
        plain_wanted: u32,
        plain_got: *mut u32,
        runs: *mut TS_RUNINFO,
        runs_wanted: u32,
        runs_got: *mut u32,
        next: *mut i32,
    ) -> Result<()> {
        self.get_text(start, end, plain, plain_wanted, plain_got, runs, runs_wanted, runs_got, next)
    }
    fn SetText(&self, _flags: u32, start: i32, end: i32, text: &PCWSTR, count: u32) -> Result<TS_TEXTCHANGE> {
        self.set_text(start, end, text, count)
    }
    fn GetFormattedText(&self, _start: i32, _end: i32) -> Result<IDataObject> {
        Err(E_NOTIMPL.into())
    }
    fn GetEmbedded(&self, _at: i32, _service: *const GUID, _riid: *const GUID) -> Result<IUnknown> {
        Err(E_NOTIMPL.into())
    }
    fn QueryInsertEmbedded(&self, _service: *const GUID, _format: *const FORMATETC) -> Result<BOOL> {
        Ok(false.into())
    }
    fn InsertEmbedded(&self, _: u32, _: i32, _: i32, _: Ref<IDataObject>) -> Result<TS_TEXTCHANGE> {
        Err(E_NOTIMPL.into())
    }
    fn InsertTextAtSelection(
        &self,
        flags: u32,
        text: &PCWSTR,
        count: u32,
        start: *mut i32,
        end: *mut i32,
        change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        self.insert_text_at_selection(flags, text, count, start, end, change)
    }
    fn InsertEmbeddedAtSelection(
        &self,
        _: u32,
        _: Ref<IDataObject>,
        _: *mut i32,
        _: *mut i32,
        _: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn RequestSupportedAttrs(&self, _flags: u32, count: u32, attributes: *const GUID) -> Result<()> {
        self.ask_attributes(count, attributes);
        Ok(())
    }
    fn RequestAttrsAtPosition(&self, _at: i32, count: u32, attributes: *const GUID, _flags: u32) -> Result<()> {
        self.ask_attributes(count, attributes);
        Ok(())
    }
    fn RequestAttrsTransitioningAtPosition(&self, _: i32, _: u32, _: *const GUID, _: u32) -> Result<()> {
        self.asked_input_scope.set(false); // (none changes along the text)
        Ok(())
    }
    fn FindNextAttrTransition(
        &self,
        _start: i32,
        halt: i32,
        _: u32,
        _: *const GUID,
        _: u32,
        next: *mut i32,
        found: *mut BOOL,
        offset: *mut i32,
    ) -> Result<()> {
        // SAFETY: TSF's out parameters (none changes along the text).
        unsafe {
            if !next.is_null() {
                *next = halt;
            }
            if !found.is_null() {
                *found = false.into();
            }
            if !offset.is_null() {
                *offset = 0;
            }
        }
        Ok(())
    }
    fn RetrieveRequestedAttrs(&self, count: u32, values: *mut TS_ATTRVAL, fetched: *mut u32) -> Result<()> {
        self.retrieve_attributes(count, values, fetched)
    }
    fn GetEndACP(&self) -> Result<i32> {
        self.check(false)?;
        Ok(self.document.borrow().end() as i32)
    }
    fn GetActiveView(&self) -> Result<u32> {
        Ok(0)
    }
    fn GetACPFromPoint(&self, _view: u32, point: *const POINT, _flags: u32) -> Result<i32> {
        self.acp_from_point(point)
    }
    fn GetTextExt(&self, _view: u32, _start: i32, _end: i32, rect: *mut RECT, clipped: *mut BOOL) -> Result<()> {
        self.text_ext(clipped, rect)
    }
    fn GetScreenExt(&self, _view: u32) -> Result<RECT> {
        Ok(self.screen_ext())
    }
    fn GetWnd(&self, _view: u32) -> Result<HWND> {
        Ok(HWND(self.hwnd.value() as *mut _))
    }
}

impl ITextStoreACP2_Impl for TextStore_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>, mask: u32) -> Result<()> {
        self.advise_sink(riid, punk, mask)
    }
    fn UnadviseSink(&self, punk: Ref<IUnknown>) -> Result<()> {
        self.unadvise_sink(punk)
    }
    fn RequestLock(&self, flags: u32) -> Result<HRESULT> {
        self.request_lock(flags)
    }
    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(self.status())
    }
    fn QueryInsert(&self, start: i32, end: i32, _count: u32, rs: *mut i32, re: *mut i32) -> Result<()> {
        self.query_insert(start, end, rs, re)
    }
    fn GetSelection(&self, index: u32, count: u32, s: *mut TS_SELECTION_ACP, fetched: *mut u32) -> Result<()> {
        self.get_selection(index, count, s, fetched)
    }
    fn SetSelection(&self, count: u32, s: *const TS_SELECTION_ACP) -> Result<()> {
        self.set_selection(count, s)
    }
    fn GetText(
        &self,
        start: i32,
        end: i32,
        plain: PWSTR,
        plain_wanted: u32,
        plain_got: *mut u32,
        runs: *mut TS_RUNINFO,
        runs_wanted: u32,
        runs_got: *mut u32,
        next: *mut i32,
    ) -> Result<()> {
        self.get_text(start, end, plain, plain_wanted, plain_got, runs, runs_wanted, runs_got, next)
    }
    fn SetText(&self, _flags: u32, start: i32, end: i32, text: &PCWSTR, count: u32) -> Result<TS_TEXTCHANGE> {
        self.set_text(start, end, text, count)
    }
    fn GetFormattedText(&self, _start: i32, _end: i32) -> Result<IDataObject> {
        Err(E_NOTIMPL.into())
    }
    fn GetEmbedded(&self, _at: i32, _service: *const GUID, _riid: *const GUID) -> Result<IUnknown> {
        Err(E_NOTIMPL.into())
    }
    fn QueryInsertEmbedded(&self, _service: *const GUID, _format: *const FORMATETC) -> Result<BOOL> {
        Ok(false.into())
    }
    fn InsertEmbedded(&self, _: u32, _: i32, _: i32, _: Ref<IDataObject>) -> Result<TS_TEXTCHANGE> {
        Err(E_NOTIMPL.into())
    }
    fn InsertTextAtSelection(
        &self,
        flags: u32,
        text: &PCWSTR,
        count: u32,
        start: *mut i32,
        end: *mut i32,
        change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        self.insert_text_at_selection(flags, text, count, start, end, change)
    }
    fn InsertEmbeddedAtSelection(
        &self,
        _: u32,
        _: Ref<IDataObject>,
        _: *mut i32,
        _: *mut i32,
        _: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn RequestSupportedAttrs(&self, _flags: u32, count: u32, attributes: *const GUID) -> Result<()> {
        self.ask_attributes(count, attributes);
        Ok(())
    }
    fn RequestAttrsAtPosition(&self, _at: i32, count: u32, attributes: *const GUID, _flags: u32) -> Result<()> {
        self.ask_attributes(count, attributes);
        Ok(())
    }
    fn RequestAttrsTransitioningAtPosition(&self, _: i32, _: u32, _: *const GUID, _: u32) -> Result<()> {
        self.asked_input_scope.set(false);
        Ok(())
    }
    fn FindNextAttrTransition(
        &self,
        start: i32,
        halt: i32,
        count: u32,
        attributes: *const GUID,
        flags: u32,
        next: *mut i32,
        found: *mut BOOL,
        offset: *mut i32,
    ) -> Result<()> {
        ITextStoreACP_Impl::FindNextAttrTransition(self, start, halt, count, attributes, flags, next, found, offset)
    }
    fn RetrieveRequestedAttrs(&self, count: u32, values: *mut TS_ATTRVAL, fetched: *mut u32) -> Result<()> {
        self.retrieve_attributes(count, values, fetched)
    }
    fn GetEndACP(&self) -> Result<i32> {
        ITextStoreACP_Impl::GetEndACP(self)
    }
    fn GetActiveView(&self) -> Result<u32> {
        Ok(0)
    }
    fn GetACPFromPoint(&self, _view: u32, point: *const POINT, _flags: u32) -> Result<i32> {
        self.acp_from_point(point)
    }
    fn GetTextExt(&self, _view: u32, _start: i32, _end: i32, rect: *mut RECT, clipped: *mut BOOL) -> Result<()> {
        self.text_ext(clipped, rect)
    }
    fn GetScreenExt(&self, _view: u32) -> Result<RECT> {
        Ok(self.screen_ext())
    }
}

impl ITfContextOwnerCompositionSink_Impl for TextStore_Impl {
    /// A text service begins composing: taken (a composition the app is ending refused).
    fn OnStartComposition(&self, view: Ref<ITfCompositionView>) -> Result<BOOL> {
        if self.terminating.get() {
            return Ok(false.into());
        }
        *self.composition.borrow_mut() = view.cloned();
        Ok(true.into())
    }
    fn OnUpdateComposition(&self, view: Ref<ITfCompositionView>, _range: Ref<ITfRange>) -> Result<()> {
        if let Some(view) = view.cloned() {
            *self.composition.borrow_mut() = Some(view);
        }
        Ok(())
    }
    fn OnEndComposition(&self, _view: Ref<ITfCompositionView>) -> Result<()> {
        *self.composition.borrow_mut() = None;
        self.document.borrow_mut().set_composition(None);
        Ok(())
    }
}

impl ITfTextEditSink_Impl for TextStore_Impl {
    /// A text service's edit session ended (its lock still held): the composition's range and display attributes
    /// read while its cookie is good.
    fn OnEndEdit(&self, context: Ref<ITfContext>, cookie: u32, _record: Ref<ITfEditRecord>) -> Result<()> {
        let range = self.composition_range();
        self.document.borrow_mut().set_composition(range);
        if let Some(context) = context.as_ref() {
            self.read_display_attributes(context, cookie);
        }
        Ok(())
    }
}

/// A field's input scopes as TSF reads them (ITfInputScope: the text store's GUID_PROP_INPUTSCOPE attribute).
#[implement(ITfInputScope)]
pub struct InputScopes(pub Vec<i32>);

impl ITfInputScope_Impl for InputScopes_Impl {
    fn GetInputScopes(&self, scopes: *mut *mut InputScope, count: *mut u32) -> Result<()> {
        if scopes.is_null() || count.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let n = self.0.len();
        // SAFETY: the array is the caller's to free (CoTaskMemFree), as the interface says; filled before it is
        // handed out.
        unsafe {
            let array = CoTaskMemAlloc(n.max(1) * std::mem::size_of::<InputScope>()) as *mut InputScope;
            if array.is_null() {
                return Err(::windows::Win32::Foundation::E_OUTOFMEMORY.into());
            }
            for (i, scope) in self.0.iter().enumerate() {
                array.add(i).write(InputScope(*scope));
            }
            *scopes = array;
            *count = n as u32;
        }
        Ok(())
    }
    fn GetPhrase(&self, _phrases: *mut *mut BSTR, _count: *mut u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetRegularExpression(&self) -> Result<BSTR> {
        Err(E_NOTIMPL.into())
    }
    fn GetSRGS(&self) -> Result<BSTR> {
        Err(E_NOTIMPL.into())
    }
    fn GetXML(&self) -> Result<BSTR> {
        Err(E_NOTIMPL.into())
    }
}

/// The text store's sink of a test (TSF's side: it is granted the locks it asks for and runs what the test gave it
/// under them), and what it was told.
#[cfg(test)]
pub(super) mod test_sink {
    use ::windows::Win32::UI::TextServices::*;
    use ::windows_core::{Result, implement};
    use std::cell::RefCell;

    type Edit = Box<dyn FnOnce(TEXT_STORE_LOCK_FLAGS)>;

    #[implement(ITextStoreACPSink)]
    pub struct Sink {
        pub edits: RefCell<Vec<Edit>>,
        pub told: RefCell<Vec<String>>,
    }

    impl ITextStoreACPSink_Impl for Sink_Impl {
        fn OnTextChange(&self, _flags: TEXT_STORE_TEXT_CHANGE_FLAGS, change: *const TS_TEXTCHANGE) -> Result<()> {
            // SAFETY: the store passes its change.
            let c = unsafe { *change };
            self.told.borrow_mut().push(format!("text {} {} {}", c.acpStart, c.acpOldEnd, c.acpNewEnd));
            Ok(())
        }
        fn OnSelectionChange(&self) -> Result<()> {
            self.told.borrow_mut().push("selection".into());
            Ok(())
        }
        fn OnLayoutChange(&self, _code: TsLayoutCode, _view: u32) -> Result<()> {
            self.told.borrow_mut().push("layout".into());
            Ok(())
        }
        fn OnStatusChange(&self, _flags: u32) -> Result<()> {
            Ok(())
        }
        fn OnAttrsChange(&self, _: i32, _: i32, _: u32, _: *const ::windows_core::GUID) -> Result<()> {
            Ok(())
        }
        fn OnLockGranted(&self, flags: TEXT_STORE_LOCK_FLAGS) -> Result<()> {
            let edit = {
                let mut edits = self.edits.borrow_mut();
                (!edits.is_empty()).then(|| edits.remove(0))
            };
            if let Some(edit) = edit {
                edit(flags);
            }
            Ok(())
        }
        fn OnStartEditTransaction(&self) -> Result<()> {
            Ok(())
        }
        fn OnEndEditTransaction(&self) -> Result<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_sink::Sink;
    use super::*;
    use crate::app::Event;
    use crate::windows::ime::Field;
    use ::windows_core::ComObject;

    struct TestOwner {
        field: RefCell<Field>,
        events: RefCell<Vec<Event>>,
    }

    impl Owner for TestOwner {
        fn edited(&self, presented: TextEditState) {
            let events = ime::transition(&self.field.borrow().state, &presented);
            let events = self.field.borrow_mut().send(events);
            self.events.borrow_mut().extend(events);
        }
        fn caret(&self) -> Rect {
            Rect::new(10.0, 20.0, 11.0, 40.0)
        }
    }

    fn owner(state: &TextEditState) -> Rc<TestOwner> {
        Rc::new(TestOwner {
            field: RefCell::new(Field { active: true, state: state.clone(), ..Field::default() }),
            events: RefCell::new(Vec::new()),
        })
    }

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[implement(ITfEditSession)]
    struct Session(Box<dyn Fn(u32)>);

    impl ITfEditSession_Impl for Session_Impl {
        fn DoEditSession(&self, ec: u32) -> Result<()> {
            (self.0)(ec);
            Ok(())
        }
    }

    fn state(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> TextEditState {
        TextEditState { text: text.into(), selection_start: selection.0, selection_end: selection.1, composition }
    }

    /// A store of its own (its sink the test's, TSF's side), the owner it tells.
    fn store_with_sink(
        text: &TextEditState,
        options: &ImeOptions,
    ) -> (Rc<TestOwner>, ComObject<TextStore>, ITextStoreACP, ComObject<Sink>) {
        let o = owner(text);
        let weak: Weak<dyn Owner> = Rc::downgrade(&(o.clone() as Rc<dyn Owner>));
        let store = ComObject::new(TextStore::new(Hwnd::NONE, weak, text, options, Weak::new()));
        let acp: ITextStoreACP = store.to_interface();
        let sink = ComObject::new(Sink { edits: RefCell::new(Vec::new()), told: RefCell::new(Vec::new()) });
        let sink_interface: ITextStoreACPSink = sink.to_interface();
        let mask = TS_AS_TEXT_CHANGE | TS_AS_SEL_CHANGE | TS_AS_LAYOUT_CHANGE;
        // SAFETY: the store's interface on this thread.
        unsafe { acp.AdviseSink(&ITextStoreACPSink::IID, &sink_interface, mask) }.expect("the sink advised");
        (o, store, acp, sink)
    }

    /// `edit` run under a lock of `flags` (TS_LF_*), as TSF's sink runs a text service's edit (granted at once).
    fn locked(acp: &ITextStoreACP, sink: &Sink, flags: u32, edit: impl FnOnce(TEXT_STORE_LOCK_FLAGS) + 'static) {
        sink.edits.borrow_mut().push(Box::new(edit));
        // SAFETY: the store's interface on this thread.
        let answer = unsafe { acp.RequestLock(flags) }.expect("a lock");
        assert_eq!(answer, HRESULT(0));
    }

    fn text(acp: &ITextStoreACP) -> String {
        let mut buffer = [0u16; 64];
        let (mut got, mut runs_got, mut next) = (0, 0, 0);
        let mut run = [TS_RUNINFO::default()];
        // SAFETY: the buffers outlive the call.
        unsafe {
            acp.GetText(0, -1, &mut buffer, &mut got, &mut run, &mut runs_got, &mut next).expect("its text");
        }
        assert_eq!((runs_got, run[0].uCount, next), (1, got, got as i32));
        String::from_utf16_lossy(&buffer[..got as usize])
    }

    fn selection(acp: &ITextStoreACP) -> (i32, i32, TsActiveSelEnd) {
        let mut s = [TS_SELECTION_ACP::default()];
        let mut fetched = 0;
        // SAFETY: the out parameters outlive the call.
        unsafe { acp.GetSelection(TF_DEFAULT_SELECTION, &mut s, &mut fetched) }.expect("its selection");
        assert_eq!(fetched, 1);
        (s[0].acpStart, s[0].acpEnd, s[0].style.ase)
    }

    #[test]
    fn the_text_store_under_its_locks() {
        let options = ImeOptions { keyboard_type: kompoz_ui::platform::KeyboardType::Email, ..ImeOptions::default() };
        let (o, store, acp, sink) = store_with_sink(&state("ab", (2, 2), None), &options);
        // Without a lock: nothing read. SAFETY: the store's interface on this thread (as below).
        assert_eq!(unsafe { acp.GetEndACP() }.unwrap_err().code(), TS_E_NOLOCK);
        // A synchronous write lock: text inserted at the selection; meanwhile another synchronous lock refused, an
        // asynchronous one queued and granted after it (it selects the first character).
        let inner = acp.clone();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        sink.edits.borrow_mut().push(Box::new(move |flags| unsafe {
            assert_eq!(flags, TS_LF_READWRITE);
            assert_eq!(text(&inner), "ab");
            assert_eq!(selection(&inner), (2, 2, TS_AE_NONE));
            let (mut start, mut end, mut change) = (0, 0, TS_TEXTCHANGE::default());
            inner.InsertTextAtSelection(0, &units("ç"), &mut start, &mut end, &mut change).unwrap();
            assert_eq!((start, end, change.acpOldEnd, change.acpNewEnd), (2, 3, 2, 3));
            assert_eq!(inner.RequestLock(TS_LF_READ.0 | TS_LF_SYNC).unwrap(), TS_E_SYNCHRONOUS);
            assert_eq!(inner.RequestLock(TS_LF_READWRITE.0).unwrap(), TS_S_ASYNC);
        }));
        let inner = acp.clone();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        sink.edits.borrow_mut().push(Box::new(move |_| unsafe {
            let s = TS_SELECTION_ACP {
                acpStart: 0,
                acpEnd: 1,
                style: TS_SELECTIONSTYLE { ase: TS_AE_START, fInterimChar: false.into() },
            };
            inner.SetSelection(&[s]).unwrap();
            assert_eq!(selection(&inner), (0, 1, TS_AE_START));
        }));
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        let answer = unsafe { acp.RequestLock(TS_LF_READWRITE.0 | TS_LF_SYNC) }.unwrap();
        assert_eq!(answer, HRESULT(0));
        assert!(sink.edits.borrow().is_empty(), "the queued lock granted after the first");
        assert_eq!(o.events.take(), [Event::Text("ç".into()), Event::SetSelection { start: 0, end: 1 }]);
        // A read lock doesn't let one write.
        let inner = acp.clone();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        locked(&acp, &sink, TS_LF_READ.0 | TS_LF_SYNC, move |_| unsafe {
            let error = inner.SetText(0, 0, 0, &units("x")).unwrap_err();
            assert_eq!(error.code(), TS_E_NOLOCK);
            // Where the characters are: the caret's box (its window none here: the client area at 0, 0).
            let (mut rect, mut clipped) = (RECT::default(), BOOL::default());
            inner.GetTextExt(0, 0, 1, &mut rect, &mut clipped).unwrap();
            assert_eq!((rect.left, rect.top, rect.right, rect.bottom, clipped.as_bool()), (10, 20, 11, 40, false));
            let point = POINT { x: 10, y: 25 };
            assert_eq!(inner.GetACPFromPoint(0, &point, 0).unwrap(), 1);
        });
        assert!(o.events.take().is_empty());
        // A composition (a text service's, its view's range: a test's here) with its clause being converted:
        // the core's composing text, that clause highlighted; then committed as the text service left it.
        let (inner, s) = (acp.clone(), store.clone());
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        locked(&acp, &sink, TS_LF_READWRITE.0 | TS_LF_SYNC, move |_| unsafe {
            let mut change = TS_TEXTCHANGE::default();
            let _ = inner.SetSelection(&[TS_SELECTION_ACP { acpStart: 3, acpEnd: 3, ..Default::default() }]);
            inner
                .InsertTextAtSelection(0, &units("かな"), std::ptr::null_mut(), std::ptr::null_mut(), &mut change)
                .unwrap();
            s.composed.set(Some((3, 5)));
            s.document.borrow_mut().attributes = vec![(3, 4, ime::ATTR_TARGET_CONVERTED), (4, 5, ime::ATTR_INPUT)];
        });
        assert_eq!(
            o.events.take(),
            [Event::SetSelection { start: 4, end: 4 }, Event::Preedit { text: "かな".into(), cursor: Some((0, 3)) }]
        );
        let (inner, s) = (acp.clone(), store.clone());
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        locked(&acp, &sink, TS_LF_READWRITE.0 | TS_LF_SYNC, move |_| unsafe {
            inner.SetText(0, 3, 5, &units("仮名")).unwrap();
            s.composed.set(None);
        });
        assert_eq!(o.events.take(), [Event::Text("仮名".into())]);
        assert_eq!(o.field.borrow().state, state("abç仮名", (10, 10), None));
        // The field's input scopes, as TSF reads them from the store.
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        unsafe {
            acp.RequestAttrsAtPosition(0, &[GUID_PROP_INPUTSCOPE], 0).unwrap();
            let mut value = [TS_ATTRVAL::default()];
            let mut fetched = 0;
            acp.RetrieveRequestedAttrs(&mut value, &mut fetched).unwrap();
            assert_eq!((fetched, value[0].idAttr), (1, GUID_PROP_INPUTSCOPE));
            let inner = &*value[0].varValue.Anonymous.Anonymous;
            assert_eq!(inner.vt, VT_UNKNOWN);
            let scopes: ITfInputScope = inner.Anonymous.punkVal.as_ref().unwrap().cast().unwrap();
            let (mut array, mut count) = (std::ptr::null_mut(), 0);
            scopes.GetInputScopes(&mut array, &mut count).unwrap();
            assert_eq!(std::slice::from_raw_parts(array, count as usize), [InputScope(ime::IS_EMAIL_SMTPEMAILADDRESS)]);
            ::windows::Win32::System::Com::CoTaskMemFree(Some(array as *const _));
            let _ = VariantClear(&mut value[0].varValue);
        }
        // The app's change: the sink told (once no lock is held); the store's own status.
        sink.told.take();
        let s = store.clone();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        locked(&acp, &sink, TS_LF_READ.0 | TS_LF_SYNC, move |_| s.changed_by_app(&state("abc", (1, 1), None)));
        assert_eq!(sink.told.take(), ["text 2 5 3", "selection"]);
        o.events.take();
        store.layout_changed();
        assert_eq!(sink.told.take(), ["layout"]);
        assert_eq!(store.document().state, state("abc", (1, 1), None));
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        let status = unsafe { acp.GetStatus() }.unwrap();
        assert_eq!(status.dwStaticFlags, TS_SS_NOHIDDENTEXT | TS_SS_TKBAUTOCORRECTENABLE | TS_SS_TKBPREDICTIONENABLE);
        // A second sink refused; the first unadvised.
        let other: ITextStoreACPSink =
            ComObject::new(Sink { edits: RefCell::new(Vec::new()), told: RefCell::new(Vec::new()) }).to_interface();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        let refused = unsafe { acp.AdviseSink(&ITextStoreACPSink::IID, &other, TS_AS_TEXT_CHANGE) }.unwrap_err();
        assert_eq!(refused.code(), CONNECT_E_ADVISELIMIT);
        let first: ITextStoreACPSink = sink.to_interface();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        unsafe { acp.UnadviseSink(&first) }.unwrap();
        // SAFETY: the store's interface on this thread, its out parameters living through the calls.
        assert_eq!(unsafe { acp.RequestLock(TS_LF_READ.0) }.unwrap_err().code(), E_FAIL);
    }

    /// Through TSF itself (msctf: wine's has edit sessions and their insertions, no compositions): the context made
    /// over the store, a text service's edit session inserting text at the selection — the store's lock asked for
    /// by TSF, its sink TSF's —, the core's events from it; the app's change told to TSF.
    #[test]
    fn a_text_services_edit_session_through_tsf() {
        let manager = ThreadManager::get().expect("TSF (msctf)");
        let state = TextEditState::caret_at_end("ab");
        let o = owner(&state);
        let weak: Weak<dyn Owner> = Rc::downgrade(&(o.clone() as Rc<dyn Owner>));
        let options = ImeOptions::default();
        let field = FieldContext::new(manager.clone(), Hwnd::NONE, &state, &options, weak).expect("a context");
        let context = field.context().clone();
        let session: ITfEditSession = Session(Box::new(move |ec| {
            // SAFETY: the context's insertion with the session's cookie, on this thread.
            unsafe {
                let insert: ITfInsertAtSelection = context.cast().unwrap();
                insert.InsertTextAtSelection(ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &units("şu")).unwrap();
            }
        }))
        .into();
        // SAFETY: an edit session asked for on this thread (granted synchronously).
        let answer =
            unsafe { field.context().RequestEditSession(manager.client, &session, TF_ES_SYNC | TF_ES_READWRITE) };
        assert_eq!(answer.ok(), Some(HRESULT(0)));
        assert_eq!(o.events.take(), [Event::Text("şu".into())]);
        assert_eq!(field.store().document().state, state_of("abşu"));
        field.changed_by_app(&state_of("x"));
        assert_eq!(field.store().document().state, state_of("x"));
        assert!(o.events.take().is_empty());
        field.set_options(&ImeOptions { keyboard_type: kompoz_ui::platform::KeyboardType::Password, ..options });
        drop(field);
    }

    fn state_of(text: &str) -> TextEditState {
        ime::normalized(&TextEditState::caret_at_end(text))
    }
}
