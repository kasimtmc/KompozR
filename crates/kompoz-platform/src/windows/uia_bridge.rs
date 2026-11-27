//! UI Automation's providers of a window's semantics tree (C++'s uia_bridge.cpp, at the AT-SPI bridge's parity):
//! Narrator, NVDA, the touch keyboard and every other UI Automation client read the app through them. The window
//! answers WM_GETOBJECT with its root ([`UiaBridge::answer`]: a fragment root over the window's own HWND provider,
//! which gives the window's name, its frame and its Window and Transform patterns); each semantics node is a
//! fragment under it ([`NodeProvider`], one object per node for as long as it is in the tree), with the patterns of
//! what it does (`uia`'s model: invoke, toggle, selection and its items, value, range value, scroll, expand and
//! collapse, a field's text and its ranges — [`TextRange`]).
//!
//! UI Automation calls the providers on threads of its own: they answer from the tree last published, kept under a
//! mutex (a [`Snapshot`], the focused field's caret box), and never touch the UI's state. What a client asks of a
//! node (invoke it, toggle it, set its value or its selection, focus it, scroll it) is kept as a [`Request`] and the
//! UI thread woken ([`UiaBridge::take_requests`]: performed there as semantics actions). What changed from one
//! published tree to the next (`uia::raises`) is raised as UI Automation's events on the UI thread, once the mutex is
//! let go (UI Automation may call back into the providers as it raises them); the providers of nodes that went are
//! disconnected (what clients still hold of them says it is gone).
//!
//! An FFI module: each `unsafe` block says why it holds.

#![allow(unsafe_code)]

use super::sys::Hwnd;
use super::uia::{self, Axis, Prop, Raise, Text};
use crate::accessibility::text::text_of;
use crate::accessibility::{Snapshot, Window};
use ::windows::Win32::Foundation::{E_INVALIDARG, HWND, LPARAM, WPARAM};
use ::windows::Win32::System::Com::SAFEARRAY;
use ::windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use ::windows::Win32::System::Variant::{VARENUM, VARIANT, VT_I4, VT_R8, VT_UNKNOWN};
use ::windows::Win32::UI::Accessibility::*;
use ::windows_core::{BOOL, BSTR, Error, HRESULT, IUnknown, IUnknownImpl, Interface, PCWSTR, Ref, Result, implement};
use kompoz_graphics::Rect;
use kompoz_ui::semantics::{AccessibilityNode, SemanticsAction};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

/// What a client asked of node `id`.
#[derive(Clone, PartialEq, Debug)]
pub struct Request {
    pub id: u64,
    pub action: UiaAction,
}

/// What a client asked for: a semantics action, or a field's selection set (code points of its text as typed:
/// the UI thread finds its bytes in the field's text).
#[derive(Clone, PartialEq, Debug)]
pub enum UiaAction {
    Do(SemanticsAction),
    Select { start: usize, end: usize },
}

/// A COM pointer of the bridge's own free-threaded objects (or UI Automation's, which are free-threaded too), kept
/// where UI Automation's threads reach it.
struct Agile<T>(T);

// SAFETY: the pointers held are of this module's provider objects, whose state is behind the bridge's mutex and
// atomics (any thread may call them), or of UI Automation's own objects, which are free-threaded.
unsafe impl<T> Send for Agile<T> {}
// SAFETY: as above.
unsafe impl<T> Sync for Agile<T> {}

/// What UI Automation's threads read and the UI thread publishes.
struct State {
    tree: Snapshot,
    /// The focused field's caret box, px in the client area (where the input method has one).
    caret: Option<Rect>,
    /// The bridge is gone (its window closed): what clients still hold says so.
    gone: bool,
    requests: Vec<Request>,
    /// Each node's provider while it is in the tree (the same object to every client, raised events on, and
    /// disconnected as the node goes).
    providers: HashMap<u64, Agile<IRawElementProviderSimple>>,
    root: Option<Agile<IRawElementProviderSimple>>,
}

struct Inner {
    hwnd: Hwnd,
    notify: Arc<dyn Fn() + Send + Sync>,
    /// A client asked for the window's root: the tree is worth publishing.
    asked: AtomicBool,
    /// The tree is wanted now (a client just asked for the root), even if nothing changed.
    tree_request: AtomicBool,
    /// Requests are waiting for the UI thread.
    pending: AtomicBool,
    state: Mutex<State>,
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn request(&self, state: &mut State, id: u64, action: UiaAction) {
        state.requests.push(Request { id, action });
        self.pending.store(true, Ordering::SeqCst);
        (self.notify)();
    }

    /// The client area's top left on the screen (asked as a client asks: the window may have moved since).
    fn origin(&self) -> (f64, f64) {
        let (x, y) = self.hwnd.client_origin();
        (x as f64, y as f64)
    }

    /// A rectangle of the client area's px on the screen: left, top, width, height.
    fn on_screen(&self, r: Rect) -> [f64; 4] {
        let (x, y) = self.origin();
        [x + r.left as f64, y + r.top as f64, r.width().max(0.0) as f64, r.height().max(0.0) as f64]
    }
}

impl State {
    fn node(&self, id: u64) -> Option<&AccessibilityNode> {
        if self.gone { None } else { self.tree.get(id) }
    }

    /// Node `id`'s provider (made the first time it is asked for).
    fn provider(&mut self, inner: &Arc<Inner>, id: u64) -> IRawElementProviderSimple {
        let p = self
            .providers
            .entry(id)
            .or_insert_with(|| Agile(as_simple(NodeProvider { inner: Arc::downgrade(inner), id }.into())));
        p.0.clone()
    }

    fn root(&mut self, inner: &Arc<Inner>) -> IRawElementProviderSimple {
        let p = self.root.get_or_insert_with(|| Agile(as_simple(RootProvider { inner: Arc::downgrade(inner) }.into())));
        p.0.clone()
    }

    fn fragment(&mut self, inner: &Arc<Inner>, id: Option<u64>) -> Result<IRawElementProviderFragment> {
        match id {
            Some(id) => self.provider(inner, id).cast(),
            None => self.root(inner).cast(),
        }
    }
}

/// UIA_E_ELEMENTNOTAVAILABLE: the node went (or the bridge did).
fn not_available() -> Error {
    HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32).into()
}

/// UIA_E_ELEMENTNOTENABLED.
fn not_enabled() -> Error {
    HRESULT(UIA_E_ELEMENTNOTENABLED as i32).into()
}

/// UIA_E_INVALIDOPERATION.
fn invalid_operation() -> Error {
    HRESULT(UIA_E_INVALIDOPERATION as i32).into()
}

/// S_OK with a null out pointer (no such element: `abi`'s shells write the null).
fn none<T>() -> Result<T> {
    Err(Error::empty())
}

/// One of this module's providers as UI Automation's IRawElementProviderSimple (its `abi::Simple`: the same
/// interface).
fn as_simple(object: IUnknown) -> IRawElementProviderSimple {
    object.cast().expect("a provider is an IRawElementProviderSimple")
}

fn as_range(object: IUnknown) -> ITextRangeProvider {
    object.cast().expect("a range is an ITextRangeProvider")
}

fn variant(p: &Prop) -> VARIANT {
    match p {
        Prop::Empty => VARIANT::default(),
        Prop::Bool(b) => VARIANT::from(*b),
        Prop::Int(i) => VARIANT::from(*i),
        Prop::Double(d) => VARIANT::from(*d),
        Prop::Str(s) => VARIANT::from(BSTR::from(s.as_str())),
    }
}

/// A one-dimensional SAFEARRAY of `vt`, `items` put in (each a pointer to its value: an interface's is its own
/// pointer, which the array keeps a reference to).
fn safe_array(vt: VARENUM, items: &[*const std::ffi::c_void]) -> Result<*mut SAFEARRAY> {
    // SAFETY: the array is made with room for every item; each item points to a value of the array's type (an
    // interface pointer for VT_UNKNOWN, AddRef'd by SafeArrayPutElement), valid during the call. The array is the
    // caller's (UI Automation frees it).
    unsafe {
        let array = SafeArrayCreateVector(vt, 0, items.len() as u32);
        if array.is_null() {
            return Err(::windows::Win32::Foundation::E_OUTOFMEMORY.into());
        }
        for (i, item) in items.iter().enumerate() {
            SafeArrayPutElement(array, &(i as i32), *item)?;
        }
        Ok(array)
    }
}

fn i32_array(values: &[i32]) -> Result<*mut SAFEARRAY> {
    let items: Vec<*const std::ffi::c_void> = values.iter().map(|v| v as *const i32 as *const _).collect();
    safe_array(VT_I4, &items)
}

fn f64_array(values: &[f64]) -> Result<*mut SAFEARRAY> {
    let items: Vec<*const std::ffi::c_void> = values.iter().map(|v| v as *const f64 as *const _).collect();
    safe_array(VT_R8, &items)
}

fn unknown_array(values: &[IUnknown]) -> Result<*mut SAFEARRAY> {
    let items: Vec<*const std::ffi::c_void> = values.iter().map(|v| v.as_raw() as *const _).collect();
    safe_array(VT_UNKNOWN, &items)
}

// ---- The bridge -------------------------------------------------------------------------------------------------

/// A window's UI Automation bridge: what the UI thread publishes to it and takes from it (cloned: the window's
/// procedure answers WM_GETOBJECT with it).
#[derive(Clone)]
pub struct UiaBridge {
    inner: Arc<Inner>,
}

impl UiaBridge {
    /// The bridge of window `hwnd`; `notify` wakes the UI thread (called on UI Automation's threads as clients ask
    /// for something).
    pub fn new(hwnd: Hwnd, notify: Arc<dyn Fn() + Send + Sync>) -> UiaBridge {
        let state = State {
            tree: Snapshot::default(),
            caret: None,
            gone: false,
            requests: Vec::new(),
            providers: HashMap::new(),
            root: None,
        };
        let inner = Inner {
            hwnd,
            notify,
            asked: AtomicBool::new(false),
            tree_request: AtomicBool::new(false),
            pending: AtomicBool::new(false),
            state: Mutex::new(state),
        };
        let inner = Arc::new(inner);
        bridges().insert(hwnd.value(), Arc::downgrade(&inner));
        UiaBridge { inner }
    }

    /// `KOMPOZ_UIA`: off where it is "0".
    pub fn enabled() -> bool {
        kompoz_runtime::environment::var("KOMPOZ_UIA").is_none_or(|v| v.trim() != "0")
    }

    /// WM_GETOBJECT answered (on the UI thread): the root for UI Automation (UiaRootObjectId), the tree then wanted;
    /// none for other objects (Windows' own MSAA answer). Whether this was the first time a client asked (the tree
    /// to publish).
    pub fn answer(&self, hwnd: Hwnd, w: usize, l: isize) -> Option<(isize, bool)> {
        if l as i32 != UiaRootObjectId {
            return None;
        }
        let root = self.inner.state().root(&self.inner);
        let first = !self.inner.asked.swap(true, Ordering::SeqCst);
        if first {
            self.inner.tree_request.store(true, Ordering::SeqCst);
        }
        // SAFETY: WM_GETOBJECT's own parameters handed back with the root provider, as the function asks.
        let result = unsafe { UiaReturnRawElementProvider(HWND(hwnd.value() as *mut _), WPARAM(w), LPARAM(l), &root) };
        Some((result.0, first))
    }

    /// A client asked for the window's tree: it is worth publishing.
    pub fn active(&self) -> bool {
        self.inner.asked.load(Ordering::SeqCst)
    }

    /// Whether the tree is wanted now (a client just asked for the root), even if nothing changed.
    pub fn take_tree_request(&self) -> bool {
        self.inner.tree_request.swap(false, Ordering::SeqCst)
    }

    /// Whether requests came since this was last asked (an AccessibilityRequest due).
    pub fn take_pending(&self) -> bool {
        self.inner.pending.swap(false, Ordering::SeqCst)
    }

    /// What clients asked for since, in order.
    pub fn take_requests(&self) -> Vec<Request> {
        std::mem::take(&mut self.inner.state().requests)
    }

    /// The window's tree as it is now (the UI thread's), `caret` the focused field's caret box (px in the client
    /// area) where the input method has one: what changed since the last raised as events, the providers of the
    /// nodes that went disconnected.
    pub fn publish(&self, nodes: Vec<AccessibilityNode>, caret: Option<Rect>) {
        let next = Snapshot::new(nodes, Window::default());
        // SAFETY: a query of UI Automation's.
        let listening = unsafe { UiaClientsAreListening() }.as_bool();
        let (raises, removed) = {
            let mut state = self.inner.state();
            let raises = if listening { uia::raises(&state.tree, &next) } else { Vec::new() };
            let mut out = Vec::new();
            for r in raises {
                let node = match &r {
                    Raise::Property { node, .. } | Raise::Event { node, .. } => Some(*node),
                    Raise::Structure { node, .. } => *node,
                };
                let provider = match node {
                    Some(id) => state.provider(&self.inner, id),
                    None => state.root(&self.inner),
                };
                out.push((provider, r));
            }
            let removed: Vec<IRawElementProviderSimple> = {
                let gone: Vec<u64> = state.providers.keys().copied().filter(|id| !next.contains(*id)).collect();
                gone.into_iter().filter_map(|id| state.providers.remove(&id)).map(|p| p.0).collect()
            };
            state.tree = next;
            state.caret = caret;
            (out, removed)
        };
        for (provider, r) in raises {
            raise(&provider, &r);
        }
        for p in removed {
            // SAFETY: our own provider, no longer in the tree.
            let _ = unsafe { UiaDisconnectProvider(&p) };
        }
    }

    /// The window's root provider (tests ask the providers themselves, as UI Automation does).
    #[cfg(test)]
    pub fn root(&self) -> IRawElementProviderSimple {
        self.inner.state().root(&self.inner)
    }

    /// The window going (before it is destroyed): what UI Automation holds of it released, every provider
    /// disconnected, what clients still hold saying so.
    pub fn close(&self) {
        bridges().remove(&self.inner.hwnd.value());
        let providers: Vec<IRawElementProviderSimple> = {
            let mut state = self.inner.state();
            state.gone = true;
            state.requests.clear();
            let mut all: Vec<IRawElementProviderSimple> = state.providers.drain().map(|(_, p)| p.0).collect();
            all.extend(state.root.take().map(|r| r.0));
            all
        };
        if self.active() {
            // SAFETY: the window's handle, still valid: what UI Automation keeps for it released (as the function's
            // documentation asks on WM_DESTROY).
            unsafe {
                UiaReturnRawElementProvider(
                    HWND(self.inner.hwnd.value() as *mut _),
                    WPARAM(0),
                    LPARAM(0),
                    None::<&IRawElementProviderSimple>,
                );
            }
        }
        for p in providers {
            // SAFETY: our own provider, its node gone with the window.
            let _ = unsafe { UiaDisconnectProvider(&p) };
        }
    }
}

/// The bridges of the process's windows, by their handles' values.
fn bridges() -> MutexGuard<'static, HashMap<isize, Weak<Inner>>> {
    static BRIDGES: std::sync::OnceLock<Mutex<HashMap<isize, Weak<Inner>>>> = std::sync::OnceLock::new();
    BRIDGES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The root UI Automation provider of the app's window `hwnd` (its handle's value) in this process, as
/// UiaReturnRawElementProvider hands it to UI Automation; none where the window has no bridge (`KOMPOZ_UIA=0`, closed).
/// For in-process tools and tests that ask the providers themselves, where a UI Automation client lacks a pattern
/// (Wine's has few): any thread may call them.
pub fn root_provider(hwnd: isize) -> Option<IRawElementProviderSimple> {
    let inner = bridges().get(&hwnd)?.upgrade()?;
    let root = inner.state().root(&inner);
    Some(root)
}

/// An event raised (on the UI thread, the mutex let go).
fn raise(provider: &IRawElementProviderSimple, r: &Raise) {
    // SAFETY: our own provider, the VARIANTs and the runtime id alive through each call.
    unsafe {
        let _ = match r {
            Raise::Property { property, old, new, .. } => UiaRaiseAutomationPropertyChangedEvent(
                provider,
                UIA_PROPERTY_ID(*property),
                &variant(old),
                &variant(new),
            ),
            Raise::Event { event, .. } => UiaRaiseAutomationEvent(provider, UIA_EVENT_ID(*event)),
            Raise::Structure { change, child, .. } => {
                let mut id = uia::runtime_id(*child);
                UiaRaiseStructureChangedEvent(provider, StructureChangeType(*change), id.as_mut_ptr(), id.len() as i32)
            }
        };
    }
}

// ---- The interfaces as UI Automation calls them -------------------------------------------------------------------

/// The provider interfaces whose methods may answer "no element" (S_OK with a null out pointer: no parent, no such
/// pattern, no focus, no text found), declared as their ABI is: the `windows` crate's implementations of them leave
/// the out pointer as the caller had it on that answer, and UI Automation's core (Wine's at least) reads it without
/// having set it. Each shell writes its out pointer first, then calls the `windows` crate's trait implementation of
/// the same method (the module's logic), so the interfaces are UI Automation's own (same IIDs, same vtables).
#[allow(non_snake_case)]
mod abi {
    use ::windows::Win32::System::Com::SAFEARRAY;
    use ::windows::Win32::System::Variant::VARIANT;
    use ::windows::Win32::UI::Accessibility::{
        NavigateDirection, ProviderOptions, TextPatternRangeEndpoint, TextUnit, UIA_PATTERN_ID, UIA_PROPERTY_ID,
        UIA_TEXTATTRIBUTE_ID, UiaRect,
    };
    use ::windows_core::{BOOL, HRESULT, IUnknown, IUnknown_Vtbl, interface};
    use std::ffi::c_void;
    use std::mem::ManuallyDrop;

    /// IRawElementProviderSimple.
    #[interface("d6dd68d1-86fd-4332-8666-9abedea2d24c")]
    pub unsafe trait Simple: IUnknown {
        fn ProviderOptions(&self, out: *mut ProviderOptions) -> HRESULT;
        fn GetPatternProvider(&self, id: UIA_PATTERN_ID, out: *mut *mut c_void) -> HRESULT;
        fn GetPropertyValue(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT;
        fn HostRawElementProvider(&self, out: *mut *mut c_void) -> HRESULT;
    }

    /// IRawElementProviderFragment.
    #[interface("f7063da8-8359-439c-9297-bbc5299a7d87")]
    pub unsafe trait Fragment: IUnknown {
        fn Navigate(&self, direction: NavigateDirection, out: *mut *mut c_void) -> HRESULT;
        fn GetRuntimeId(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
        fn BoundingRectangle(&self, out: *mut UiaRect) -> HRESULT;
        fn GetEmbeddedFragmentRoots(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
        fn SetFocus(&self) -> HRESULT;
        fn FragmentRoot(&self, out: *mut *mut c_void) -> HRESULT;
    }

    /// IRawElementProviderFragmentRoot.
    #[interface("620ce2a5-ab8f-40a9-86cb-de3c75599b58")]
    pub unsafe trait FragmentRoot: IUnknown {
        fn ElementProviderFromPoint(&self, x: f64, y: f64, out: *mut *mut c_void) -> HRESULT;
        fn GetFocus(&self, out: *mut *mut c_void) -> HRESULT;
    }

    /// ISelectionItemProvider.
    #[interface("2acad808-b2d4-452d-a407-91ff1ad167b2")]
    pub unsafe trait SelectionItem: IUnknown {
        fn Select(&self) -> HRESULT;
        fn AddToSelection(&self) -> HRESULT;
        fn RemoveFromSelection(&self) -> HRESULT;
        fn IsSelected(&self, out: *mut BOOL) -> HRESULT;
        fn SelectionContainer(&self, out: *mut *mut c_void) -> HRESULT;
    }

    /// ITextRangeProvider.
    #[interface("5347ad7b-c355-46f8-aff5-909033582f63")]
    pub unsafe trait TextRange: IUnknown {
        fn Clone(&self, out: *mut *mut c_void) -> HRESULT;
        fn Compare(&self, range: *mut c_void, out: *mut BOOL) -> HRESULT;
        fn CompareEndpoints(
            &self,
            endpoint: TextPatternRangeEndpoint,
            target: *mut c_void,
            target_endpoint: TextPatternRangeEndpoint,
            out: *mut i32,
        ) -> HRESULT;
        fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> HRESULT;
        fn FindAttribute(
            &self,
            id: UIA_TEXTATTRIBUTE_ID,
            value: ManuallyDrop<VARIANT>,
            backward: BOOL,
            out: *mut *mut c_void,
        ) -> HRESULT;
        fn FindText(&self, text: *mut c_void, backward: BOOL, ignore_case: BOOL, out: *mut *mut c_void) -> HRESULT;
        fn GetAttributeValue(&self, id: UIA_TEXTATTRIBUTE_ID, out: *mut VARIANT) -> HRESULT;
        fn GetBoundingRectangles(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
        fn GetEnclosingElement(&self, out: *mut *mut c_void) -> HRESULT;
        fn GetText(&self, max: i32, out: *mut *mut c_void) -> HRESULT;
        fn Move(&self, unit: TextUnit, count: i32, out: *mut i32) -> HRESULT;
        fn MoveEndpointByUnit(
            &self,
            endpoint: TextPatternRangeEndpoint,
            unit: TextUnit,
            count: i32,
            out: *mut i32,
        ) -> HRESULT;
        fn MoveEndpointByRange(
            &self,
            endpoint: TextPatternRangeEndpoint,
            target: *mut c_void,
            target_endpoint: TextPatternRangeEndpoint,
        ) -> HRESULT;
        fn Select(&self) -> HRESULT;
        fn AddToSelection(&self) -> HRESULT;
        fn RemoveFromSelection(&self) -> HRESULT;
        fn ScrollIntoView(&self, align_to_top: BOOL) -> HRESULT;
        fn GetChildren(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    }
}

/// A value answered through an out pointer (E_POINTER where there is none).
///
/// # Safety
/// `out` is null or valid for a write of a `T`.
unsafe fn put<T>(out: *mut T, r: Result<T>) -> HRESULT {
    if out.is_null() {
        return ::windows::Win32::Foundation::E_POINTER;
    }
    match r {
        // SAFETY: as the function's contract says.
        Ok(v) => unsafe {
            out.write(v);
            HRESULT(0)
        },
        Err(e) => e.code(),
    }
}

/// An interface answered through an out pointer, null first (none: S_OK and null; a failure: its code and null).
///
/// # Safety
/// `out` is null or valid for a write of a pointer.
unsafe fn give<T: Interface>(out: *mut *mut std::ffi::c_void, r: Result<T>) -> HRESULT {
    if out.is_null() {
        return ::windows::Win32::Foundation::E_POINTER;
    }
    // SAFETY: as the function's contract says; the interface's reference is the caller's from here.
    unsafe {
        out.write(std::ptr::null_mut());
        match r {
            Ok(v) => {
                out.write(v.into_raw());
                HRESULT(0)
            }
            Err(e) => e.code(),
        }
    }
}

/// A result as an HRESULT.
fn done(r: Result<()>) -> HRESULT {
    r.map_or_else(|e| e.code(), |_| HRESULT(0))
}

/// A range passed in by its raw pointer, as the `windows` crate's implementations take it.
///
/// # Safety
/// `raw` is null or an ITextRangeProvider valid during the call.
unsafe fn range_of_raw(raw: *mut std::ffi::c_void) -> Option<ITextRangeProvider> {
    // SAFETY: as the function's contract says (borrowed, then counted by `cloned`).
    unsafe { ITextRangeProvider::from_raw_borrowed(&raw).cloned() }
}

/// The ABI shells of IRawElementProviderSimple and IRawElementProviderFragment for a provider type.
macro_rules! simple_and_fragment {
    ($t:ty) => {
        impl abi::Simple_Impl for $t {
            unsafe fn ProviderOptions(&self, out: *mut ProviderOptions) -> HRESULT {
                // SAFETY: UI Automation's out pointer.
                unsafe { put(out, IRawElementProviderSimple_Impl::ProviderOptions(self)) }
            }
            unsafe fn GetPatternProvider(&self, id: UIA_PATTERN_ID, out: *mut *mut std::ffi::c_void) -> HRESULT {
                // SAFETY: as above.
                unsafe { give(out, IRawElementProviderSimple_Impl::GetPatternProvider(self, id)) }
            }
            unsafe fn GetPropertyValue(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT {
                // SAFETY: as above.
                unsafe { put(out, IRawElementProviderSimple_Impl::GetPropertyValue(self, id)) }
            }
            unsafe fn HostRawElementProvider(&self, out: *mut *mut std::ffi::c_void) -> HRESULT {
                // SAFETY: as above.
                unsafe { give(out, IRawElementProviderSimple_Impl::HostRawElementProvider(self)) }
            }
        }

        impl abi::Fragment_Impl for $t {
            unsafe fn Navigate(&self, direction: NavigateDirection, out: *mut *mut std::ffi::c_void) -> HRESULT {
                // SAFETY: as above.
                unsafe { give(out, IRawElementProviderFragment_Impl::Navigate(self, direction)) }
            }
            unsafe fn GetRuntimeId(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
                // SAFETY: as above.
                unsafe { put(out, IRawElementProviderFragment_Impl::GetRuntimeId(self)) }
            }
            unsafe fn BoundingRectangle(&self, out: *mut UiaRect) -> HRESULT {
                // SAFETY: as above.
                unsafe { put(out, IRawElementProviderFragment_Impl::BoundingRectangle(self)) }
            }
            unsafe fn GetEmbeddedFragmentRoots(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
                // SAFETY: as above.
                unsafe { put(out, IRawElementProviderFragment_Impl::GetEmbeddedFragmentRoots(self)) }
            }
            unsafe fn SetFocus(&self) -> HRESULT {
                done(IRawElementProviderFragment_Impl::SetFocus(self))
            }
            unsafe fn FragmentRoot(&self, out: *mut *mut std::ffi::c_void) -> HRESULT {
                // SAFETY: as above.
                unsafe { give(out, IRawElementProviderFragment_Impl::FragmentRoot(self)) }
            }
        }
    };
}

simple_and_fragment!(RootProvider_Impl);
simple_and_fragment!(NodeProvider_Impl);

impl abi::FragmentRoot_Impl for RootProvider_Impl {
    unsafe fn ElementProviderFromPoint(&self, x: f64, y: f64, out: *mut *mut std::ffi::c_void) -> HRESULT {
        // SAFETY: UI Automation's out pointer.
        unsafe { give(out, IRawElementProviderFragmentRoot_Impl::ElementProviderFromPoint(self, x, y)) }
    }
    unsafe fn GetFocus(&self, out: *mut *mut std::ffi::c_void) -> HRESULT {
        // SAFETY: as above.
        unsafe { give(out, IRawElementProviderFragmentRoot_Impl::GetFocus(self)) }
    }
}

impl abi::SelectionItem_Impl for NodeProvider_Impl {
    unsafe fn Select(&self) -> HRESULT {
        done(ISelectionItemProvider_Impl::Select(self))
    }
    unsafe fn AddToSelection(&self) -> HRESULT {
        done(ISelectionItemProvider_Impl::AddToSelection(self))
    }
    unsafe fn RemoveFromSelection(&self) -> HRESULT {
        done(ISelectionItemProvider_Impl::RemoveFromSelection(self))
    }
    unsafe fn IsSelected(&self, out: *mut BOOL) -> HRESULT {
        // SAFETY: UI Automation's out pointer.
        unsafe { put(out, ISelectionItemProvider_Impl::IsSelected(self)) }
    }
    unsafe fn SelectionContainer(&self, out: *mut *mut std::ffi::c_void) -> HRESULT {
        // SAFETY: as above.
        unsafe { give(out, ISelectionItemProvider_Impl::SelectionContainer(self)) }
    }
}

impl abi::TextRange_Impl for TextRange_Impl {
    unsafe fn Clone(&self, out: *mut *mut std::ffi::c_void) -> HRESULT {
        // SAFETY: UI Automation's out pointer.
        unsafe { give(out, ITextRangeProvider_Impl::Clone(self)) }
    }
    unsafe fn Compare(&self, range: *mut std::ffi::c_void, out: *mut BOOL) -> HRESULT {
        // SAFETY: UI Automation's range and out pointer.
        unsafe {
            let range = range_of_raw(range);
            put(out, ITextRangeProvider_Impl::Compare(self, Ref::from(&range)))
        }
    }
    unsafe fn CompareEndpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        target: *mut std::ffi::c_void,
        target_endpoint: TextPatternRangeEndpoint,
        out: *mut i32,
    ) -> HRESULT {
        // SAFETY: as above.
        unsafe {
            let target = range_of_raw(target);
            put(out, ITextRangeProvider_Impl::CompareEndpoints(self, endpoint, Ref::from(&target), target_endpoint))
        }
    }
    unsafe fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> HRESULT {
        done(ITextRangeProvider_Impl::ExpandToEnclosingUnit(self, unit))
    }
    unsafe fn FindAttribute(
        &self,
        id: UIA_TEXTATTRIBUTE_ID,
        value: std::mem::ManuallyDrop<VARIANT>,
        backward: BOOL,
        out: *mut *mut std::ffi::c_void,
    ) -> HRESULT {
        // SAFETY: UI Automation's out pointer (the VARIANT stays the caller's: not dropped).
        unsafe { give(out, ITextRangeProvider_Impl::FindAttribute(self, id, &value, backward)) }
    }
    unsafe fn FindText(
        &self,
        text: *mut std::ffi::c_void,
        backward: BOOL,
        ignore_case: BOOL,
        out: *mut *mut std::ffi::c_void,
    ) -> HRESULT {
        // SAFETY: the caller's BSTR, borrowed (not freed: ManuallyDrop); UI Automation's out pointer.
        unsafe {
            let text = std::mem::ManuallyDrop::new(BSTR::from_raw(text as *const u16));
            give(out, ITextRangeProvider_Impl::FindText(self, &text, backward, ignore_case))
        }
    }
    unsafe fn GetAttributeValue(&self, id: UIA_TEXTATTRIBUTE_ID, out: *mut VARIANT) -> HRESULT {
        // SAFETY: UI Automation's out pointer.
        unsafe { put(out, ITextRangeProvider_Impl::GetAttributeValue(self, id)) }
    }
    unsafe fn GetBoundingRectangles(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        // SAFETY: as above.
        unsafe { put(out, ITextRangeProvider_Impl::GetBoundingRectangles(self)) }
    }
    unsafe fn GetEnclosingElement(&self, out: *mut *mut std::ffi::c_void) -> HRESULT {
        // SAFETY: as above.
        unsafe { give(out, ITextRangeProvider_Impl::GetEnclosingElement(self)) }
    }
    unsafe fn GetText(&self, max: i32, out: *mut *mut std::ffi::c_void) -> HRESULT {
        let text = ITextRangeProvider_Impl::GetText(self, max).map(|b| b.into_raw() as *mut std::ffi::c_void);
        // SAFETY: as above (the BSTR the caller's to free).
        unsafe { put(out, text) }
    }
    unsafe fn Move(&self, unit: TextUnit, count: i32, out: *mut i32) -> HRESULT {
        // SAFETY: as above.
        unsafe { put(out, ITextRangeProvider_Impl::Move(self, unit, count)) }
    }
    unsafe fn MoveEndpointByUnit(
        &self,
        endpoint: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
        out: *mut i32,
    ) -> HRESULT {
        // SAFETY: as above.
        unsafe { put(out, ITextRangeProvider_Impl::MoveEndpointByUnit(self, endpoint, unit, count)) }
    }
    unsafe fn MoveEndpointByRange(
        &self,
        endpoint: TextPatternRangeEndpoint,
        target: *mut std::ffi::c_void,
        target_endpoint: TextPatternRangeEndpoint,
    ) -> HRESULT {
        // SAFETY: UI Automation's range.
        let target = unsafe { range_of_raw(target) };
        done(ITextRangeProvider_Impl::MoveEndpointByRange(self, endpoint, Ref::from(&target), target_endpoint))
    }
    unsafe fn Select(&self) -> HRESULT {
        done(ITextRangeProvider_Impl::Select(self))
    }
    unsafe fn AddToSelection(&self) -> HRESULT {
        done(ITextRangeProvider_Impl::AddToSelection(self))
    }
    unsafe fn RemoveFromSelection(&self) -> HRESULT {
        done(ITextRangeProvider_Impl::RemoveFromSelection(self))
    }
    unsafe fn ScrollIntoView(&self, align_to_top: BOOL) -> HRESULT {
        done(ITextRangeProvider_Impl::ScrollIntoView(self, align_to_top))
    }
    unsafe fn GetChildren(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        // SAFETY: UI Automation's out pointer.
        unsafe { put(out, ITextRangeProvider_Impl::GetChildren(self)) }
    }
}

// ---- The window's root ------------------------------------------------------------------------------------------

/// The window's fragment root: what is under it (the window's top nodes), what is where, where the focus is; the
/// window's own (its name, its bounds, its runtime id, its Window and Transform patterns) is its HWND provider's.
#[implement(abi::Simple, abi::Fragment, abi::FragmentRoot)]
struct RootProvider {
    inner: Weak<Inner>,
}

impl RootProvider {
    fn inner(&self) -> Result<Arc<Inner>> {
        self.inner.upgrade().ok_or_else(not_available)
    }
}

impl IRawElementProviderSimple_Impl for RootProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, _: UIA_PATTERN_ID) -> Result<IUnknown> {
        none()
    }

    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let inner = self.inner()?;
        if inner.state().gone {
            return Err(not_available());
        }
        Ok(if id.0 == uia::property::FRAMEWORK_ID {
            variant(&Prop::Str(uia::FRAMEWORK.into()))
        } else {
            VARIANT::default()
        })
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        let inner = self.inner()?;
        // SAFETY: the window's handle (UI Automation's provider of the HWND itself).
        unsafe { UiaHostProviderFromHwnd(HWND(inner.hwnd.value() as *mut _)) }
    }
}

impl IRawElementProviderFragment_Impl for RootProvider_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let inner = self.inner()?;
        let mut state = inner.state();
        if state.gone {
            return Err(not_available());
        }
        let top = state.tree.children(None);
        // (Its parent and siblings: the HWND's.)
        let target = if direction == NavigateDirection_FirstChild {
            top.first().copied()
        } else if direction == NavigateDirection_LastChild {
            top.last().copied()
        } else {
            None
        };
        match target {
            Some(id) => state.fragment(&inner, Some(id)),
            None => none(),
        }
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut()) // (the HWND's)
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(UiaRect::default()) // (the HWND's)
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }

    fn SetFocus(&self) -> Result<()> {
        Ok(()) // (the HWND's)
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        let inner = self.inner()?;
        let root = inner.state().root(&inner);
        root.cast()
    }
}

impl IRawElementProviderFragmentRoot_Impl for RootProvider_Impl {
    /// The innermost node at the point (the last laid out there: drawn on top), else the window.
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let inner = self.inner()?;
        let (ox, oy) = inner.origin();
        let mut state = inner.state();
        let (x, y) = ((x - ox) as f32, (y - oy) as f32);
        let found = state
            .tree
            .nodes()
            .iter()
            .filter(|n| x >= n.bounds.left && y >= n.bounds.top && x < n.bounds.right && y < n.bounds.bottom)
            .map(|n| n.id)
            .next_back();
        state.fragment(&inner, found)
    }

    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        let inner = self.inner()?;
        let mut state = inner.state();
        match state.tree.focused() {
            Some(id) => state.fragment(&inner, Some(id)),
            None => none(),
        }
    }
}

// ---- A node -----------------------------------------------------------------------------------------------------

/// A semantics node's provider, with every pattern a node may have (asked for by GetPatternProvider, given where
/// the node does what the pattern is for).
#[implement(
    abi::Simple,
    abi::Fragment,
    IInvokeProvider,
    IToggleProvider,
    ISelectionProvider,
    abi::SelectionItem,
    IValueProvider,
    IRangeValueProvider,
    IScrollProvider,
    IExpandCollapseProvider,
    ITextProvider2
)]
struct NodeProvider {
    inner: Weak<Inner>,
    id: u64,
}

impl NodeProvider {
    /// `f` with the node (and the bridge's state), under the mutex; UIA_E_ELEMENTNOTAVAILABLE where it went.
    fn with<R>(&self, f: impl FnOnce(&Arc<Inner>, &mut State, &AccessibilityNode) -> Result<R>) -> Result<R> {
        let inner = self.inner.upgrade().ok_or_else(not_available)?;
        let mut state = inner.state();
        let node = state.node(self.id).cloned().ok_or_else(not_available)?;
        f(&inner, &mut state, &node)
    }

    /// `action` asked for, where the node is enabled.
    fn act(&self, action: impl FnOnce(&AccessibilityNode) -> Option<SemanticsAction>) -> Result<()> {
        self.with(|inner, state, n| {
            if !n.enabled {
                return Err(not_enabled());
            }
            if let Some(a) = action(n) {
                inner.request(state, self.id, UiaAction::Do(a));
            }
            Ok(())
        })
    }

    fn range(&self, index: usize) -> Result<f64> {
        self.with(|_, _, n| uia::range_numbers(n).map(|r| r[index]).ok_or_else(invalid_operation))
    }

    /// A range of the field's text, [start, end) in code points.
    fn text_range(&self, start: usize, end: usize) -> ITextRangeProvider {
        as_range(TextRange { inner: self.inner.clone(), id: self.id, ends: Mutex::new((start, end)) }.into())
    }

    fn field<R>(&self, f: impl FnOnce(&AccessibilityNode) -> Result<R>) -> Result<R> {
        self.with(|_, _, n| if n.editable { f(n) } else { Err(invalid_operation()) })
    }
}

impl IRawElementProviderSimple_Impl for NodeProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, id: UIA_PATTERN_ID) -> Result<IUnknown> {
        let has = self.with(|_, _, n| Ok(uia::has_pattern(n, id.0)))?;
        if has { Ok(self.to_interface::<IUnknown>()) } else { none() }
    }

    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        self.with(|_, state, n| Ok(variant(&uia::property_of(&state.tree, n, id.0))))
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        none()
    }
}

impl IRawElementProviderFragment_Impl for NodeProvider_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        self.with(|inner, state, n| {
            let siblings = state.tree.children(n.parent);
            let at = siblings.iter().position(|&k| k == n.id).unwrap_or(0);
            let kids = state.tree.children(Some(n.id));
            let target: Option<Option<u64>> = if direction == NavigateDirection_Parent {
                Some(n.parent)
            } else if direction == NavigateDirection_FirstChild {
                kids.first().map(|&k| Some(k))
            } else if direction == NavigateDirection_LastChild {
                kids.last().map(|&k| Some(k))
            } else if direction == NavigateDirection_NextSibling {
                siblings.get(at + 1).map(|&k| Some(k))
            } else if direction == NavigateDirection_PreviousSibling {
                at.checked_sub(1).and_then(|i| siblings.get(i)).map(|&k| Some(k))
            } else {
                None
            };
            match target {
                Some(id) => state.fragment(inner, id),
                None => none(),
            }
        })
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        i32_array(&uia::runtime_id(self.id))
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        self.with(|inner, _, n| {
            let [left, top, width, height] = inner.on_screen(n.bounds);
            Ok(UiaRect { left, top, width, height })
        })
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }

    /// The keyboard's focus put on it, where Tab goes to it (elsewhere nothing: as C++'s).
    fn SetFocus(&self) -> Result<()> {
        self.with(|inner, state, n| {
            if uia::focusable(n) && !n.focused {
                inner.request(state, self.id, UiaAction::Do(SemanticsAction::Focus));
            }
            Ok(())
        })
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        let inner = self.inner.upgrade().ok_or_else(not_available)?;
        let root = inner.state().root(&inner);
        root.cast()
    }
}

impl IInvokeProvider_Impl for NodeProvider_Impl {
    fn Invoke(&self) -> Result<()> {
        self.act(|n| Some(uia::invoke(n)))
    }
}

impl IToggleProvider_Impl for NodeProvider_Impl {
    fn Toggle(&self) -> Result<()> {
        self.act(|_| Some(SemanticsAction::Click))
    }

    fn ToggleState(&self) -> Result<ToggleState> {
        self.with(|_, _, n| uia::toggle_of(n).map(ToggleState).ok_or_else(invalid_operation))
    }
}

impl ISelectionProvider_Impl for NodeProvider_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        let selected = self.with(|inner, state, n| {
            let ids = uia::selection_of(&state.tree, n);
            ids.into_iter().map(|id| state.provider(inner, id).cast::<IUnknown>()).collect::<Result<Vec<_>>>()
        })?;
        unknown_array(&selected)
    }

    fn CanSelectMultiple(&self) -> Result<BOOL> {
        Ok(false.into())
    }

    fn IsSelectionRequired(&self) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ISelectionItemProvider_Impl for NodeProvider_Impl {
    /// Its click, where it isn't selected yet.
    fn Select(&self) -> Result<()> {
        self.act(|n| (uia::selected_of(n) == Some(false)).then_some(SemanticsAction::Click))
    }

    /// (One selected at a time: selecting it.)
    fn AddToSelection(&self) -> Result<()> {
        ISelectionItemProvider_Impl::Select(self)
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Err(invalid_operation())
    }

    fn IsSelected(&self) -> Result<BOOL> {
        self.with(|_, _, n| Ok(uia::selected_of(n).unwrap_or(false).into()))
    }

    fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
        self.with(|inner, state, n| match uia::selection_container(&state.tree, n) {
            Some(group) => Ok(state.provider(inner, group)),
            None => none(),
        })
    }
}

impl IValueProvider_Impl for NodeProvider_Impl {
    fn SetValue(&self, value: &PCWSTR) -> Result<()> {
        // SAFETY: the caller's string, valid during the call.
        let text = unsafe { value.to_string() }.map_err(|_| Error::from(E_INVALIDARG))?;
        self.with(|inner, state, n| {
            if !n.editable {
                return Err(invalid_operation());
            }
            if !n.enabled {
                return Err(not_enabled());
            }
            inner.request(state, self.id, UiaAction::Do(SemanticsAction::SetText(text)));
            Ok(())
        })
    }

    fn Value(&self) -> Result<BSTR> {
        self.with(|_, _, n| Ok(BSTR::from(text_of(n).as_str())))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        self.with(|_, _, n| Ok((!n.editable || !n.enabled).into()))
    }
}

impl IRangeValueProvider_Impl for NodeProvider_Impl {
    fn SetValue(&self, value: f64) -> Result<()> {
        self.with(|inner, state, n| match uia::set_range_value(n, value) {
            Ok(action) if n.enabled => {
                inner.request(state, self.id, UiaAction::Do(action));
                Ok(())
            }
            Ok(_) => Err(not_enabled()),
            Err(uia::RangeRefusal::ReadOnly) => Err(invalid_operation()),
            Err(uia::RangeRefusal::OutOfRange) => Err(E_INVALIDARG.into()),
        })
    }

    fn Value(&self) -> Result<f64> {
        self.range(0)
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        self.with(|_, _, n| Ok((!n.settable).into()))
    }

    fn Maximum(&self) -> Result<f64> {
        self.range(2)
    }

    fn Minimum(&self) -> Result<f64> {
        self.range(1)
    }

    fn LargeChange(&self) -> Result<f64> {
        self.range(4)
    }

    fn SmallChange(&self) -> Result<f64> {
        self.range(3)
    }
}

impl IScrollProvider_Impl for NodeProvider_Impl {
    fn Scroll(&self, horizontal: ScrollAmount, vertical: ScrollAmount) -> Result<()> {
        self.with(|inner, state, n| {
            if !n.enabled {
                return Err(not_enabled());
            }
            match uia::scroll_by(n, horizontal.0, vertical.0) {
                Ok(Some(action)) => {
                    inner.request(state, self.id, UiaAction::Do(action));
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(()) => Err(invalid_operation()),
            }
        })
    }

    fn SetScrollPercent(&self, horizontal: f64, vertical: f64) -> Result<()> {
        self.with(|inner, state, n| {
            if !n.enabled {
                return Err(not_enabled());
            }
            let action = uia::scroll_to_percent(n, horizontal, vertical).ok_or_else(invalid_operation)?;
            inner.request(state, self.id, UiaAction::Do(action));
            Ok(())
        })
    }

    fn HorizontalScrollPercent(&self) -> Result<f64> {
        self.with(|_, _, n| Ok(uia::scroll_percent(n, Axis::Horizontal)))
    }

    fn VerticalScrollPercent(&self) -> Result<f64> {
        self.with(|_, _, n| Ok(uia::scroll_percent(n, Axis::Vertical)))
    }

    fn HorizontalViewSize(&self) -> Result<f64> {
        self.with(|_, _, n| Ok(uia::view_size(n, Axis::Horizontal)))
    }

    fn VerticalViewSize(&self) -> Result<f64> {
        self.with(|_, _, n| Ok(uia::view_size(n, Axis::Vertical)))
    }

    fn HorizontallyScrollable(&self) -> Result<BOOL> {
        self.with(|_, _, n| Ok(uia::scrolls(n, Axis::Horizontal).into()))
    }

    fn VerticallyScrollable(&self) -> Result<BOOL> {
        self.with(|_, _, n| Ok(uia::scrolls(n, Axis::Vertical).into()))
    }
}

impl IExpandCollapseProvider_Impl for NodeProvider_Impl {
    fn Expand(&self) -> Result<()> {
        self.act(|n| (n.expanded == Some(false)).then_some(SemanticsAction::Expand))
    }

    fn Collapse(&self) -> Result<()> {
        self.act(|n| (n.expanded == Some(true)).then_some(SemanticsAction::Collapse))
    }

    fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> {
        self.with(|_, _, n| Ok(ExpandCollapseState(uia::expand_collapse_state(n))))
    }
}

impl ITextProvider_Impl for NodeProvider_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        let ((start, end), _) = self.field(|n| Ok(uia::caret_of(n)))?;
        unknown_array(&[self.text_range(start, end).cast()?])
    }

    fn GetVisibleRanges(&self) -> Result<*mut SAFEARRAY> {
        let len = self.field(|n| Ok(text_of(n).chars().count()))?;
        unknown_array(&[self.text_range(0, len).cast()?])
    }

    /// (A field has no children: its whole text.)
    fn RangeFromChild(&self, _: Ref<IRawElementProviderSimple>) -> Result<ITextRangeProvider> {
        ITextProvider_Impl::DocumentRange(self)
    }

    /// (Where each character is isn't known: the caret.)
    fn RangeFromPoint(&self, _: &UiaPoint) -> Result<ITextRangeProvider> {
        let (_, caret) = self.field(|n| Ok(uia::caret_of(n)))?;
        Ok(self.text_range(caret, caret))
    }

    fn DocumentRange(&self) -> Result<ITextRangeProvider> {
        let len = self.field(|n| Ok(text_of(n).chars().count()))?;
        Ok(self.text_range(0, len))
    }

    fn SupportedTextSelection(&self) -> Result<SupportedTextSelection> {
        Ok(SupportedTextSelection_Single)
    }
}

impl ITextProvider2_Impl for NodeProvider_Impl {
    fn RangeFromAnnotation(&self, _: Ref<IRawElementProviderSimple>) -> Result<ITextRangeProvider> {
        Err(E_INVALIDARG.into())
    }

    /// The caret (where the selection's moving end is), active where the field has the focus.
    fn GetCaretRange(&self, active: *mut BOOL) -> Result<ITextRangeProvider> {
        let (focused, caret) = self.field(|n| Ok((n.focused, uia::caret_of(n).1)))?;
        if !active.is_null() {
            // SAFETY: the caller's out parameter.
            unsafe { *active = focused.into() };
        }
        Ok(self.text_range(caret, caret))
    }
}

// ---- A field's text range ---------------------------------------------------------------------------------------

/// A range of a field's text: its node and its ends (code points, kept in the text as it is now: a text that got
/// shorter clamps them).
#[implement(abi::TextRange)]
struct TextRange {
    inner: Weak<Inner>,
    id: u64,
    ends: Mutex<(usize, usize)>,
}

impl TextRange {
    fn ends(&self) -> MutexGuard<'_, (usize, usize)> {
        self.ends.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `f` with the field's text and node and this range's ends (clamped to the text), under the bridge's mutex.
    fn with<R>(
        &self,
        f: impl FnOnce(&Arc<Inner>, &mut State, &AccessibilityNode, &Text, &mut (usize, usize)) -> Result<R>,
    ) -> Result<R> {
        let inner = self.inner.upgrade().ok_or_else(not_available)?;
        let mut state = inner.state();
        let node = state.node(self.id).filter(|n| n.editable).cloned().ok_or_else(not_available)?;
        let text = Text::new(&text_of(&node));
        let mut ends = self.ends();
        let n = text.len();
        *ends = (ends.0.min(n), ends.1.min(n));
        f(&inner, &mut state, &node, &text, &mut ends)
    }

    /// Another range of this provider's, by what it is (UI Automation hands back only its provider's own).
    fn other<'a>(&self, range: &'a Ref<ITextRangeProvider>) -> Result<&'a TextRange_Impl> {
        let other = range.ok()?.cast_object_ref::<TextRange>().map_err(|_| Error::from(E_INVALIDARG))?;
        if other.id != self.id {
            return Err(E_INVALIDARG.into());
        }
        Ok(other)
    }

    fn endpoint(ends: (usize, usize), endpoint: TextPatternRangeEndpoint) -> usize {
        if endpoint == TextPatternRangeEndpoint_Start { ends.0 } else { ends.1 }
    }

    fn new_range(&self, start: usize, end: usize) -> ITextRangeProvider {
        as_range(TextRange { inner: self.inner.clone(), id: self.id, ends: Mutex::new((start, end)) }.into())
    }
}

impl ITextRangeProvider_Impl for TextRange_Impl {
    fn Clone(&self) -> Result<ITextRangeProvider> {
        let (start, end) = *self.ends();
        Ok(self.new_range(start, end))
    }

    fn Compare(&self, range: Ref<ITextRangeProvider>) -> Result<BOOL> {
        let other = match self.other(&range) {
            Ok(o) => *o.ends(),
            Err(_) => return Ok(false.into()),
        };
        Ok((*self.ends() == other).into())
    }

    fn CompareEndpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        target: Ref<ITextRangeProvider>,
        target_endpoint: TextPatternRangeEndpoint,
    ) -> Result<i32> {
        let theirs = TextRange::endpoint(*self.other(&target)?.ends(), target_endpoint);
        let mine = TextRange::endpoint(*self.ends(), endpoint);
        Ok(mine as i32 - theirs as i32)
    }

    fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> Result<()> {
        self.with(|_, _, _, text, ends| {
            *ends = text.enclosing(ends.0, uia::TextUnit::from_uia(unit.0));
            Ok(())
        })
    }

    /// (The text's attributes are the same throughout: none found where it would change.)
    fn FindAttribute(&self, _: UIA_TEXTATTRIBUTE_ID, _: &VARIANT, _: BOOL) -> Result<ITextRangeProvider> {
        none()
    }

    fn FindText(&self, needle: &BSTR, backward: BOOL, ignore_case: BOOL) -> Result<ITextRangeProvider> {
        let needle = needle.to_string();
        let found = self.with(|_, _, _, text, ends| {
            Ok(text.find(ends.0, ends.1, &needle, backward.as_bool(), ignore_case.as_bool()))
        })?;
        match found {
            Some((start, end)) => Ok(self.new_range(start, end)),
            None => none(),
        }
    }

    fn GetAttributeValue(&self, id: UIA_TEXTATTRIBUTE_ID) -> Result<VARIANT> {
        let read_only = self.with(|_, _, n, _, _| Ok(!n.enabled))?;
        Ok(if id == UIA_IsReadOnlyAttributeId {
            VARIANT::from(read_only)
        } else if id == UIA_IsHiddenAttributeId {
            VARIANT::from(false)
        } else {
            // SAFETY: UI Automation's own "not supported" object.
            VARIANT::from(unsafe { UiaGetReservedNotSupportedValue() }?)
        })
    }

    /// The field's box, the caret's where the range is collapsed at the caret of the focused field (the boxes the
    /// host knows: AT-SPI's text extents are the field's).
    fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> {
        let rects = self.with(|inner, state, n, _, ends| {
            let field = inner.on_screen(n.bounds);
            let at_caret = ends.0 == ends.1 && n.focused && uia::caret_of(n).1 == ends.0;
            let caret = state.caret.filter(|_| at_caret).map(|c| inner.on_screen(c));
            Ok(uia::range_rects(field, caret, ends.0 == ends.1))
        })?;
        f64_array(&rects.concat())
    }

    fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> {
        let inner = self.inner.upgrade().ok_or_else(not_available)?;
        let mut state = inner.state();
        if state.node(self.id).is_none() {
            return Err(not_available());
        }
        Ok(state.provider(&inner, self.id))
    }

    /// Its text, at most `max` UTF-16 units of it (-1: all).
    fn GetText(&self, max: i32) -> Result<BSTR> {
        let s = self.with(|_, _, _, text, ends| Ok(text.slice(ends.0, ends.1).to_string()))?;
        let mut units: Vec<u16> = s.encode_utf16().collect();
        if max >= 0 && units.len() > max as usize {
            let mut cut = max as usize;
            if cut > 0 && (0xD800..0xDC00).contains(&units[cut - 1]) {
                cut -= 1; // (not half a surrogate pair)
            }
            units.truncate(cut);
        }
        Ok(BSTR::from_wide(&units))
    }

    fn Move(&self, unit: TextUnit, count: i32) -> Result<i32> {
        self.with(|_, _, _, text, ends| {
            let (start, end, moved) = text.move_range(ends.0, ends.1, uia::TextUnit::from_uia(unit.0), count);
            *ends = (start, end);
            Ok(moved)
        })
    }

    fn MoveEndpointByUnit(&self, endpoint: TextPatternRangeEndpoint, unit: TextUnit, count: i32) -> Result<i32> {
        self.with(|_, _, _, text, ends| {
            let unit = uia::TextUnit::from_uia(unit.0);
            let (pos, moved) = text.move_endpoint(TextRange::endpoint(*ends, endpoint), unit, count);
            *ends = moved_end(*ends, endpoint, pos);
            Ok(moved)
        })
    }

    fn MoveEndpointByRange(
        &self,
        endpoint: TextPatternRangeEndpoint,
        target: Ref<ITextRangeProvider>,
        target_endpoint: TextPatternRangeEndpoint,
    ) -> Result<()> {
        let pos = TextRange::endpoint(*self.other(&target)?.ends(), target_endpoint);
        let mut ends = self.ends();
        *ends = moved_end(*ends, endpoint, pos);
        Ok(())
    }

    /// The field's selection set to it.
    fn Select(&self) -> Result<()> {
        self.with(|inner, state, n, _, ends| {
            if !n.enabled {
                return Err(not_enabled());
            }
            inner.request(state, self.id, UiaAction::Select { start: ends.0, end: ends.1 });
            Ok(())
        })
    }

    /// (One selection: none added.)
    fn AddToSelection(&self) -> Result<()> {
        Err(invalid_operation())
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Err(invalid_operation())
    }

    /// (A field keeps its caret in view itself.)
    fn ScrollIntoView(&self, _: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetChildren(&self) -> Result<*mut SAFEARRAY> {
        unknown_array(&[])
    }
}

/// A range's ends with `endpoint` moved to `pos` (the other one with it where they would cross).
fn moved_end(ends: (usize, usize), endpoint: TextPatternRangeEndpoint, pos: usize) -> (usize, usize) {
    if endpoint == TextPatternRangeEndpoint_Start { (pos, ends.1.max(pos)) } else { (ends.0.min(pos), pos) }
}

/// The bridge on a window of the test's own, asked through UI Automation's own client (IUIAutomation), as Narrator
/// asks, and through its providers as UI Automation asks them (C++'s tests/platform_tests_windows.cpp). Wine's client
/// lacks some of Windows' (no automation ids to clients, no property-changed or structure-changed handlers, few
/// client-side patterns): what it lacks is checked on Windows (CI) and through the providers everywhere.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::sys::{self, Placed, WindowHandler};
    use ::windows::Win32::Foundation::E_NOTIMPL;
    use ::windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    };
    use ::windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetElement};
    use ::windows::Win32::System::Variant::VT_BOOL;
    use ::windows::Win32::UI::WindowsAndMessaging as wm;
    use kompoz_ui::semantics::{LiveRegionMode, ProgressBarRangeInfo, Role, ScrollPosition, ToggleableState};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    /// The window's procedure: UI Automation's WM_GETOBJECT answered by the bridge.
    struct TestWindow(RefCell<Option<UiaBridge>>);

    impl WindowHandler for TestWindow {
        fn message(&self, hwnd: Hwnd, m: &sys::WindowMessage) -> Option<isize> {
            if m.id() != wm::WM_GETOBJECT {
                return None;
            }
            let bridge = self.0.borrow().clone()?;
            bridge.answer(hwnd, m.w(), m.l()).map(|(answer, _)| answer)
        }
    }

    /// Messages for up to `ms` milliseconds, or until `done` (UI Automation sends the window some).
    fn pump(ms: u64, done: impl Fn() -> bool) -> bool {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            if done() {
                return true;
            }
            while let Some(m) = sys::peek_message() {
                sys::dispatch(&m);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    /// C++'s test tree (its dp at 2 px a dp: px here), a switch and a menu besides.
    fn test_tree() -> Vec<AccessibilityNode> {
        let n = |id: u64, parent: Option<u64>, bounds: Rect, label: &str| AccessibilityNode {
            id,
            parent,
            bounds,
            label: label.into(),
            enabled: true,
            ..Default::default()
        };
        vec![
            AccessibilityNode { clickable: true, ..n(1, None, rect(20.0, 40.0, 60.0, 80.0), "OK") },
            AccessibilityNode {
                role: Some(Role::Checkbox),
                clickable: true,
                toggleable_state: Some(ToggleableState::Off),
                checked: Some(false),
                ..n(2, None, rect(0.0, 140.0, 48.0, 48.0), "Agree")
            },
            AccessibilityNode {
                value: "héllo dünya".into(),
                editable: true,
                text_selection: Some((5, 5)),
                ..n(3, None, rect(0.0, 200.0, 400.0, 112.0), "Name")
            },
            AccessibilityNode {
                state_description: "3".into(),
                range: Some(ProgressBarRangeInfo::new(3.0, 0.0..=10.0, 4)),
                settable: true,
                ..n(4, None, rect(0.0, 320.0, 400.0, 80.0), "Volume")
            },
            AccessibilityNode {
                scrollable: true,
                vertical_scroll: Some(ScrollPosition { value: 50.0, max_value: 200.0, reverse_scrolling: false }),
                ..n(5, None, rect(0.0, 400.0, 400.0, 200.0), "List")
            },
            n(6, Some(5), rect(0.0, 420.0, 400.0, 40.0), "Item"),
            AccessibilityNode {
                value: "abc".into(),
                editable: true,
                password: true,
                ..n(7, None, rect(0.0, 600.0, 400.0, 112.0), "Password")
            },
            AccessibilityNode {
                role: Some(Role::Tab),
                clickable: true,
                selected: Some(false),
                checked: Some(false),
                ..n(8, None, rect(0.0, 0.0, 200.0, 40.0), "Tab")
            },
            AccessibilityNode { heading: true, ..n(9, None, rect(0.0, 720.0, 200.0, 40.0), "Title") },
            n(10, Some(9), rect(0.0, 760.0, 200.0, 40.0), "Subtitle"),
            n(11, None, rect(400.0, 0.0, 200.0, 200.0), "Card"),
            n(12, Some(11), rect(400.0, 0.0, 200.0, 40.0), "Inside"),
            n(13, Some(5), rect(0.0, 460.0, 400.0, 40.0), "Item 2"),
            AccessibilityNode {
                role: Some(Role::Switch),
                clickable: true,
                toggleable_state: Some(ToggleableState::On),
                checked: Some(true),
                ..n(14, None, rect(400.0, 200.0, 100.0, 50.0), "Wi-Fi")
            },
            AccessibilityNode { expanded: Some(false), ..n(15, None, rect(400.0, 260.0, 100.0, 50.0), "Menu") },
        ]
    }

    fn name_of(e: &IUIAutomationElement) -> String {
        // SAFETY: a client call.
        unsafe { e.CurrentName() }.map(|b| b.to_string()).unwrap_or_default()
    }

    /// What the handlers heard, as "kind name what".
    #[implement(
        IUIAutomationEventHandler,
        IUIAutomationPropertyChangedEventHandler,
        IUIAutomationStructureChangedEventHandler,
        IUIAutomationFocusChangedEventHandler
    )]
    struct Handler(Arc<Mutex<Vec<String>>>);

    impl Handler {
        fn add(&self, line: String) {
            self.0.lock().unwrap().push(line);
        }
    }

    impl IUIAutomationEventHandler_Impl for Handler_Impl {
        fn HandleAutomationEvent(&self, sender: Ref<IUIAutomationElement>, event: UIA_EVENT_ID) -> Result<()> {
            self.add(format!("event {} {}", sender.ok().map(name_of).unwrap_or_default(), event.0));
            Ok(())
        }
    }

    impl IUIAutomationPropertyChangedEventHandler_Impl for Handler_Impl {
        fn HandlePropertyChangedEvent(
            &self,
            sender: Ref<IUIAutomationElement>,
            property: UIA_PROPERTY_ID,
            value: &VARIANT,
        ) -> Result<()> {
            let v = match number_of_variant(value) {
                Some(n) if value.vt() == VT_BOOL => (n != 0).to_string(),
                Some(n) => n.to_string(),
                None => text_of_variant(value).unwrap_or_else(|| "?".into()),
            };
            self.add(format!("property {} {} {v}", sender.ok().map(name_of).unwrap_or_default(), property.0));
            Ok(())
        }
    }

    impl IUIAutomationStructureChangedEventHandler_Impl for Handler_Impl {
        fn HandleStructureChangedEvent(
            &self,
            sender: Ref<IUIAutomationElement>,
            change: StructureChangeType,
            _: *const SAFEARRAY,
        ) -> Result<()> {
            self.add(format!("structure {} {}", sender.ok().map(name_of).unwrap_or_default(), change.0));
            Ok(())
        }
    }

    impl IUIAutomationFocusChangedEventHandler_Impl for Handler_Impl {
        fn HandleFocusChangedEvent(&self, sender: Ref<IUIAutomationElement>) -> Result<()> {
            self.add(format!("focus {}", sender.ok().map(name_of).unwrap_or_default()));
            Ok(())
        }
    }

    /// A VT_BOOL's, VT_I4's or VT_R8's VARIANT number (read here: Wine's propsys has no VariantToBoolean).
    fn number_of_variant(v: &VARIANT) -> Option<i64> {
        // SAFETY: the union's member of the VARIANT's type.
        unsafe {
            let u = &v.Anonymous.Anonymous.Anonymous;
            match v.vt() {
                VT_BOOL => Some(i64::from(u.boolVal.0 != 0)),
                VT_I4 => Some(i64::from(u.lVal)),
                VT_R8 => Some(u.dblVal as i64),
                _ => None,
            }
        }
    }

    /// A VT_BSTR VARIANT's string.
    fn text_of_variant(v: &VARIANT) -> Option<String> {
        if v.vt() != ::windows::Win32::System::Variant::VT_BSTR {
            return None;
        }
        // SAFETY: a VT_BSTR VARIANT's string, borrowed while the VARIANT is.
        Some(unsafe { v.Anonymous.Anonymous.Anonymous.bstrVal.to_string() })
    }

    fn string(p: &IRawElementProviderSimple, id: UIA_PROPERTY_ID) -> String {
        // SAFETY: a provider call, as UI Automation makes it.
        let v = unsafe { p.GetPropertyValue(id) }.unwrap_or_default();
        text_of_variant(&v).unwrap_or_default()
    }

    fn number(p: &IRawElementProviderSimple, id: UIA_PROPERTY_ID) -> i32 {
        // SAFETY: as above.
        let v = unsafe { p.GetPropertyValue(id) }.unwrap_or_default();
        number_of_variant(&v).map_or(-1, |n| n as i32)
    }

    /// A provider by its name, walked to with Navigate from `parent`.
    fn walk(parent: &IRawElementProviderFragment, wanted: &str) -> Option<IRawElementProviderSimple> {
        // SAFETY: provider calls, as UI Automation makes them.
        let mut child = unsafe { parent.Navigate(NavigateDirection_FirstChild) }.ok();
        while let Some(c) = child {
            let simple: IRawElementProviderSimple = c.cast().ok()?;
            if string(&simple, UIA_NamePropertyId) == wanted {
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

    fn pattern<T: Interface>(p: &IRawElementProviderSimple, id: UIA_PATTERN_ID) -> Option<T> {
        // SAFETY: as above.
        unsafe { p.GetPatternProvider(id) }.ok().and_then(|u| u.cast().ok())
    }

    /// A client's element by its name, walked to (Wine has no FindFirst).
    fn search(
        kids: &dyn Fn(&IUIAutomationElement) -> Vec<IUIAutomationElement>,
        parent: &IUIAutomationElement,
        wanted: &str,
    ) -> Option<IUIAutomationElement> {
        for c in kids(parent) {
            if name_of(&c) == wanted {
                return Some(c);
            }
            if let Some(inside) = search(kids, &c, wanted) {
                return Some(inside);
            }
        }
        None
    }

    fn code(r: Result<impl Sized>) -> HRESULT {
        r.map_or_else(|e| e.code(), |_| HRESULT(0))
    }

    #[test]
    fn the_bridge_serves_the_tree_to_ui_automation() {
        // SAFETY: COM on this test's thread, as a UI Automation client's (multithreaded, as C++'s test).
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        assert!(sys::register_class("KompozUiaTests"));
        let window_handler = Rc::new(TestWindow(RefCell::new(None)));
        let at = Placed { x: 100, y: 50, width: 400, height: 300 };
        let style = wm::WS_OVERLAPPEDWINDOW;
        let hwnd = sys::create_window("KompozUiaTests", "Main", style, Default::default(), at, window_handler.clone())
            .expect("a window");
        let notified = Arc::new(AtomicUsize::new(0));
        let n2 = notified.clone();
        let bridge = UiaBridge::new(
            hwnd,
            Arc::new(move || {
                n2.fetch_add(1, Ordering::SeqCst);
            }),
        );
        *window_handler.0.borrow_mut() = Some(bridge.clone());
        assert!(!bridge.active());
        // SAFETY: UI Automation's client, in this process.
        let uia: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.expect("UI Automation's client");
        bridge.publish(test_tree(), None);
        // SAFETY: a client call (the window is this thread's: its WM_GETOBJECT answered at once).
        let window = unsafe { uia.ElementFromHandle(HWND(hwnd.value() as *mut _)) }.expect("the window's element");
        assert!(bridge.active() && bridge.take_tree_request() && !bridge.take_tree_request(), "asked for: wanted");
        assert_eq!(name_of(&window), "Main"); // (the HWND's own)

        // The raw view (Wine has no RawViewWalker): the window's own frame (its title bar on Windows) aside.
        // SAFETY: client calls.
        let walker = unsafe { uia.CreateTreeWalker(&uia.CreateTrueCondition().unwrap()) }.expect("a walker");
        let kids = |parent: &IUIAutomationElement| -> Vec<IUIAutomationElement> {
            let mut out = Vec::new();
            // SAFETY: client calls.
            let mut child = unsafe { walker.GetFirstChildElement(parent) }.ok();
            while let Some(c) = child {
                // SAFETY: as above.
                child = unsafe { walker.GetNextSiblingElement(&c) }.ok();
                out.push(c);
            }
            out
        };
        let frame = [uia::control::TITLE_BAR, uia::control::MENU_BAR];
        // SAFETY: a client call.
        let type_of = |e: &IUIAutomationElement| unsafe { e.CurrentControlType() }.map_or(0, |t| t.0);
        let top: Vec<String> = kids(&window).iter().filter(|e| !frame.contains(&type_of(e))).map(name_of).collect();
        assert_eq!(top, ["OK", "Agree", "Name", "Volume", "List", "Password", "Tab", "Title", "Card", "Wi-Fi", "Menu"]);
        let find = |name: &str| search(&kids, &window, name).unwrap_or_else(|| panic!("{name} found"));
        use uia::control::*;
        for (name, t) in [
            ("Item", TEXT),
            ("OK", BUTTON),
            ("Agree", CHECK_BOX),
            ("Tab", TAB_ITEM),
            ("Name", EDIT),
            ("Volume", SLIDER),
            ("List", PANE),
            ("Wi-Fi", BUTTON),
            ("Title", TEXT),
            ("Card", GROUP),
        ] {
            assert_eq!(type_of(&find(name)), t, "{name}'s type");
        }
        assert_eq!(kids(&find("List")).iter().map(name_of).collect::<Vec<_>>(), ["Item", "Item 2"]);
        // A button: where it is (px on the screen).
        let (ox, oy) = hwnd.client_origin();
        // SAFETY: a client call.
        let r = unsafe { find("OK").CurrentBoundingRectangle() }.unwrap();
        assert_eq!((r.left, r.top, r.right - r.left, r.bottom - r.top), (ox + 20, oy + 40, 60, 80));
        // Client-side patterns (Windows': Wine's client has few): invoked, a field's text read.
        // SAFETY: client calls.
        unsafe {
            match find("OK").GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
                Ok(invoke) => {
                    invoke.Invoke().expect("invoked");
                    let asked = bridge.take_requests();
                    assert_eq!(asked, [Request { id: 1, action: UiaAction::Do(SemanticsAction::Click) }]);
                }
                Err(e) => {
                    assert!(crate::windows::sys::under_wine(), "the invoke pattern ({e})");
                    println!("  (wine: no client-side invoke pattern: {e})");
                }
            }
            match find("Name").GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                Ok(text) => {
                    let all = text.DocumentRange().unwrap();
                    assert_eq!(all.GetText(-1).unwrap().to_string(), "héllo dünya");
                    let selection = text.GetSelection().unwrap();
                    assert_eq!(selection.Length().unwrap(), 1);
                    let caret = selection.GetElement(0).unwrap();
                    caret.ExpandToEnclosingUnit(TextUnit_Word).unwrap();
                    assert_eq!(caret.GetText(-1).unwrap().to_string(), "dünya"); // (the caret after the space)
                }
                Err(e) => {
                    assert!(crate::windows::sys::under_wine(), "the text pattern ({e})");
                    println!("  (wine: no client-side text pattern: {e})");
                }
            }
        }
        let _ = bridge.take_requests();

        // The providers themselves, as UI Automation gets them from the window: their patterns and properties.
        let root = bridge.root();
        let root_fragment: IRawElementProviderFragment = root.cast().unwrap();
        let fragment_root: IRawElementProviderFragmentRoot = root.cast().unwrap();
        let provider = |name: &str| walk(&root_fragment, name).unwrap_or_else(|| panic!("{name}'s provider"));
        let requested = |id: u64, action: SemanticsAction| {
            assert_eq!(bridge.take_requests(), [Request { id, action: UiaAction::Do(action) }]);
        };
        let focus = |p: &IRawElementProviderSimple| {
            // SAFETY: a provider call, as UI Automation makes it (from any thread: they are free-threaded).
            unsafe { p.cast::<IRawElementProviderFragment>().unwrap().SetFocus() }.unwrap();
        };
        // SAFETY: provider calls, as UI Automation makes them.
        unsafe {
            // A button: invoked; found from a point in it; its focus.
            let ok = provider("OK");
            assert_eq!((number(&ok, UIA_IsEnabledPropertyId), number(&ok, UIA_IsKeyboardFocusablePropertyId)), (1, 1));
            assert_eq!(string(&ok, UIA_AutomationIdPropertyId), "1");
            assert_eq!(string(&ok, UIA_FrameworkIdPropertyId), "KompozR");
            pattern::<IInvokeProvider>(&ok, UIA_InvokePatternId).unwrap().Invoke().unwrap();
            requested(1, SemanticsAction::Click);
            assert!(pattern::<IToggleProvider>(&ok, UIA_TogglePatternId).is_none());
            let at = |x: i32, y: i32| -> String {
                let found = fragment_root.ElementProviderFromPoint((ox + x) as f64, (oy + y) as f64).unwrap();
                string(&found.cast().unwrap(), UIA_NamePropertyId)
            };
            assert_eq!(at(30, 50), "OK");
            assert_eq!(at(20, 425), "Item"); // (in the list, on its item: the item)
            focus(&ok);
            requested(1, SemanticsAction::Focus);
            let item = provider("Item");
            focus(&item);
            assert!(bridge.take_requests().is_empty()); // (not what Tab goes to)
            assert_eq!(number(&item, UIA_IsKeyboardFocusablePropertyId), 0);
            assert!(notified.load(Ordering::SeqCst) > 0, "the UI thread woken");
            // A checkbox: toggled.
            let agree = provider("Agree");
            let toggle = pattern::<IToggleProvider>(&agree, UIA_TogglePatternId).unwrap();
            assert_eq!(toggle.ToggleState().unwrap(), ToggleState_Off);
            toggle.Toggle().unwrap();
            requested(2, SemanticsAction::Click);
            assert!(pattern::<IInvokeProvider>(&agree, UIA_InvokePatternId).is_none());
            // A switch: a button that toggles, Windows' "toggle switch".
            let switch = provider("Wi-Fi");
            assert_eq!(string(&switch, UIA_LocalizedControlTypePropertyId), "toggle switch");
            let state = pattern::<IToggleProvider>(&switch, UIA_TogglePatternId).unwrap().ToggleState();
            assert_eq!(state.unwrap(), ToggleState_On);
            // A tab: selected.
            let tab = provider("Tab");
            let item_pattern = pattern::<ISelectionItemProvider>(&tab, UIA_SelectionItemPatternId).unwrap();
            assert!(!item_pattern.IsSelected().unwrap().as_bool());
            item_pattern.Select().unwrap();
            requested(8, SemanticsAction::Click);
            // A menu: expanded.
            let menu = provider("Menu");
            let expand = pattern::<IExpandCollapseProvider>(&menu, UIA_ExpandCollapsePatternId).unwrap();
            assert_eq!(expand.ExpandCollapseState().unwrap(), ExpandCollapseState_Collapsed);
            expand.Expand().unwrap();
            requested(15, SemanticsAction::Expand);
            // A text field: its value, set; its text by ranges, where they are, selected; focused.
            let name = provider("Name");
            let flags = (number(&name, UIA_IsKeyboardFocusablePropertyId), number(&name, UIA_IsPasswordPropertyId));
            assert_eq!(flags, (1, 0));
            let value = pattern::<IValueProvider>(&name, UIA_ValuePatternId).unwrap();
            assert_eq!(value.Value().unwrap().to_string(), "héllo dünya");
            assert!(!value.IsReadOnly().unwrap().as_bool());
            value.SetValue(::windows_core::w!("süt")).unwrap();
            requested(3, SemanticsAction::SetText("süt".into()));
            let text = pattern::<ITextProvider2>(&name, UIA_TextPatternId).unwrap();
            let all = text.DocumentRange().unwrap();
            assert_eq!(all.GetText(-1).unwrap().to_string(), "héllo dünya");
            assert_eq!(all.GetText(3).unwrap().to_string(), "hél");
            let mut active = BOOL::default();
            let caret = text.GetCaretRange(&mut active).unwrap();
            assert!(!active.as_bool()); // (not focused)
            let from_start =
                caret.CompareEndpoints(TextPatternRangeEndpoint_Start, &all, TextPatternRangeEndpoint_Start);
            assert_eq!(from_start.unwrap(), 5);
            assert_eq!(caret.Move(TextUnit_Word, 1).unwrap(), 1);
            assert_eq!(caret.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, 3).unwrap(), 3);
            assert_eq!(caret.GetText(-1).unwrap().to_string(), "dün");
            assert!(caret.Compare(&caret.Clone().unwrap()).unwrap().as_bool());
            let found = all.FindText(&BSTR::from("DÜN"), false, true).unwrap();
            assert!(found.Compare(&caret).unwrap().as_bool());
            let rects = caret.GetBoundingRectangles().unwrap();
            let mut values = [0.0f64; 4];
            for (i, v) in values.iter_mut().enumerate() {
                SafeArrayGetElement(rects, &(i as i32), v as *mut f64 as *mut _).unwrap();
            }
            SafeArrayDestroy(rects).unwrap();
            assert_eq!(values, [ox as f64, oy as f64 + 200.0, 400.0, 112.0]); // (the field's box)
            caret.Select().unwrap();
            assert_eq!(bridge.take_requests(), [Request { id: 3, action: UiaAction::Select { start: 6, end: 9 } }]);
            assert_eq!(text.SupportedTextSelection().unwrap(), SupportedTextSelection_Single);
            focus(&name);
            requested(3, SemanticsAction::Focus);
            // A password field: what is typed isn't told.
            let password = provider("Password");
            assert_eq!(number(&password, UIA_IsPasswordPropertyId), 1);
            let typed = pattern::<IValueProvider>(&password, UIA_ValuePatternId).unwrap().Value();
            assert_eq!(typed.unwrap().to_string(), "•••");
            // A slider: its range, set.
            let volume = provider("Volume");
            assert_eq!(string(&volume, UIA_ItemStatusPropertyId), "3");
            let range = pattern::<IRangeValueProvider>(&volume, UIA_RangeValuePatternId).unwrap();
            let numbers = (range.Value().unwrap(), range.Maximum().unwrap(), range.SmallChange().unwrap());
            assert_eq!(numbers, (3.0, 10.0, 2.0));
            assert!(!range.IsReadOnly().unwrap().as_bool());
            range.SetValue(7.0).unwrap();
            assert_eq!(code(range.SetValue(11.0)), E_INVALIDARG); // (out of its range)
            requested(4, SemanticsAction::SetProgress(7.0));
            // A list: scrolled, where it is in percent.
            let list = provider("List");
            let scroll = pattern::<IScrollProvider>(&list, UIA_ScrollPatternId).unwrap();
            assert_eq!(scroll.VerticalScrollPercent().unwrap(), 25.0);
            assert_eq!(scroll.HorizontalScrollPercent().unwrap(), uia::NO_SCROLL);
            scroll.Scroll(ScrollAmount_NoAmount, ScrollAmount_LargeDecrement).unwrap();
            requested(5, SemanticsAction::ScrollBy { x: 0.0, y: -200.0 });
            scroll.SetScrollPercent(uia::NO_SCROLL, 100.0).unwrap();
            requested(5, SemanticsAction::ScrollBy { x: 0.0, y: 150.0 });
            // Up, and to the one before.
            let neighbour = |name: &str, direction: NavigateDirection| -> String {
                let f: IRawElementProviderFragment = provider(name).cast().unwrap();
                f.Navigate(direction).ok().map_or("(none)".into(), |o| string(&o.cast().unwrap(), UIA_NamePropertyId))
            };
            assert_eq!(neighbour("Item", NavigateDirection_Parent), "List");
            assert_eq!(neighbour("Agree", NavigateDirection_PreviousSibling), "OK");
            assert_eq!(neighbour("OK", NavigateDirection_PreviousSibling), "(none)");
            assert_eq!(neighbour("Title", NavigateDirection_NextSibling), "Card");
            assert_eq!(neighbour("Menu", NavigateDirection_NextSibling), "(none)");
            assert_eq!(neighbour("List", NavigateDirection_LastChild), "Item 2");
            assert_eq!(neighbour("List", NavigateDirection_FirstChild), "Item");
            assert_eq!(number(&provider("Title"), UIA_HeadingLevelPropertyId), uia::HEADING_LEVEL_1);
            assert_eq!(number(&provider("Card"), UIA_ControlTypePropertyId), GROUP);
        }

        // What changed is raised.
        let heard = Arc::new(Mutex::new(Vec::new()));
        let handler: IUnknown = Handler(heard.clone()).into();
        let events: IUIAutomationEventHandler = handler.cast().unwrap();
        let properties = [
            UIA_NamePropertyId,
            UIA_ToggleToggleStatePropertyId,
            UIA_ValueValuePropertyId,
            UIA_RangeValueValuePropertyId,
            UIA_IsEnabledPropertyId,
            UIA_SelectionItemIsSelectedPropertyId,
            UIA_ExpandCollapseExpandCollapseStatePropertyId,
        ];
        let subtree = TreeScope_Subtree;
        // SAFETY: client calls; the handler our own.
        let registered = unsafe {
            let changed: IUIAutomationPropertyChangedEventHandler = handler.cast().unwrap();
            let structure: IUIAutomationStructureChangedEventHandler = handler.cast().unwrap();
            let focused: IUIAutomationFocusChangedEventHandler = handler.cast().unwrap();
            [
                uia.AddPropertyChangedEventHandlerNativeArray(&window, subtree, None, &changed, &properties),
                uia.AddAutomationEventHandler(UIA_LiveRegionChangedEventId, &window, subtree, None, &events),
                uia.AddFocusChangedEventHandler(None, &focused),
                uia.AddStructureChangedEventHandler(&window, subtree, None, &structure),
                uia.AddAutomationEventHandler(
                    UIA_SelectionItem_ElementSelectedEventId,
                    &window,
                    subtree,
                    None,
                    &events,
                ),
                uia.AddAutomationEventHandler(UIA_Text_TextChangedEventId, &window, subtree, None, &events),
                uia.AddAutomationEventHandler(UIA_Text_TextSelectionChangedEventId, &window, subtree, None, &events),
            ]
        };
        for r in &registered {
            assert!(r.is_ok() || code(r.clone()) == E_NOTIMPL, "registered: {r:?}");
        }
        let (property_events, structure_events) = (registered[0].is_ok(), registered[3].is_ok());
        if !property_events || !structure_events {
            println!("  (no property or structure handlers here: those events aren't heard)");
        }
        let item = provider("Item");
        let mut next = test_tree();
        next[0].label = "Done".into();
        next[0].live_region = Some(LiveRegionMode::Polite);
        next[1].toggleable_state = Some(ToggleableState::On);
        next[1].checked = Some(true);
        next[1].enabled = false;
        next[2].value = "süt".into();
        next[2].text_selection = Some((3, 3));
        next[2].focused = true;
        next[3].range = Some(ProgressBarRangeInfo::new(5.0, 0.0..=10.0, 4));
        next[7].selected = Some(true);
        next[7].checked = Some(true);
        next[14].expanded = Some(true);
        next.remove(5); // (the list's item)
        bridge.publish(next, None);
        let mut expected = vec![
            "property Done 30005 Done",
            "event Done 20024",
            "property Agree 30086 1",
            "property Name 30045 süt",
            "event Name 20015",
            "event Name 20014",
            "property Volume 30047 5",
            "property Tab 30079 true",
            "event Tab 20012",
            "property Menu 30070 1",
            "focus Name",
            "structure List 1",
            "property Agree 30010 false",
        ];
        let has = |line: &str| heard.lock().unwrap().iter().any(|l| l == line);
        pump(3000, || expected.iter().all(|l| has(l)));
        expected.retain(|l| {
            (property_events || !l.starts_with("property")) && (structure_events || !l.starts_with("structure"))
        });
        let missing: Vec<&&str> = expected.iter().filter(|l| !has(l)).collect();
        assert!(missing.is_empty(), "not heard: {missing:?}; heard: {:?}", heard.lock().unwrap());
        // SAFETY: provider calls; a client call.
        unsafe {
            // Focus where the tree has it; a tab already selected, a checkbox disabled: nothing asked.
            let focused = fragment_root.GetFocus().unwrap();
            assert_eq!(string(&focused.cast().unwrap(), UIA_NamePropertyId), "Name");
            pattern::<ISelectionItemProvider>(&provider("Tab"), UIA_SelectionItemPatternId).unwrap().Select().unwrap();
            let agree = provider("Agree");
            let flags = (number(&agree, UIA_IsEnabledPropertyId), number(&agree, UIA_IsKeyboardFocusablePropertyId));
            assert_eq!(flags, (0, 0));
            let toggled = pattern::<IToggleProvider>(&agree, UIA_TogglePatternId).unwrap().Toggle();
            assert_eq!(code(toggled), HRESULT(UIA_E_ELEMENTNOTENABLED as i32));
            assert!(bridge.take_requests().is_empty());
            // A node gone: what was held of it says so.
            let gone = HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32);
            assert_eq!(code(item.GetPropertyValue(UIA_NamePropertyId)), gone);
            let _ = uia.RemoveAllEventHandlers();
            // The window going: its providers say so too.
            bridge.close();
            assert_eq!(code(root.GetPropertyValue(UIA_FrameworkIdPropertyId)), gone);
        }
        *window_handler.0.borrow_mut() = None;
        hwnd.destroy();
        // SAFETY: balanced with the CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
}
