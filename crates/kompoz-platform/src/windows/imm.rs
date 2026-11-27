//! IMM32 as the Windows platform's input method path uses it (C++'s, completed): a window's input context (its
//! composition string, attributes and caret, where its windows go, its composition cancelled or replaced, opened
//! and closed), the context associated with the window or not, the input scopes a field asks for (msctf's
//! `SetInputScopes`, which IMM32's input methods and the touch keyboard read too), and the answers to an input
//! method's requests (WM_IME_REQUEST), written into the buffers it passes.
//!
//! An FFI module: each `unsafe` block says why it holds. The buffers of WM_IME_REQUEST are the input method's, valid
//! while the message is handled: they are read and written only from the window procedure's [`WindowMessage`].

#![allow(unsafe_code)]

use super::ime::CharPosition;
use super::sys::{Hwnd, WindowMessage};
use ::windows::Win32::Foundation::{HWND, POINT, RECT};
use ::windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, CPS_CANCEL, CPS_COMPLETE, GCS_COMPATTR, GCS_COMPSTR,
    GCS_CURSORPOS, GCS_RESULTSTR, HIMC, IACE_DEFAULT, IME_COMPOSITION_STRING, IMECHARPOSITION, IMR_CANDIDATEWINDOW,
    IMR_COMPOSITIONWINDOW, IMR_CONFIRMRECONVERTSTRING, IMR_DOCUMENTFEED, IMR_QUERYCHARPOSITION, IMR_RECONVERTSTRING,
    ImmAssociateContextEx, ImmGetCompositionStringW, ImmGetContext, ImmNotifyIME, ImmReleaseContext,
    ImmSetCandidateWindow, ImmSetCompositionStringW, ImmSetCompositionWindow, NI_COMPOSITIONSTR, SCS_SETSTR,
};
use ::windows::Win32::UI::WindowsAndMessaging::WM_IME_REQUEST;

/// What GCS_* asks of a composition (imm.h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Composition,
    Result,
}

/// A window's input context, released when dropped (ImmGetContext's pairing).
pub struct Context {
    hwnd: HWND,
    himc: HIMC,
}

impl Context {
    /// The input context of `hwnd`; none where it has none (not associated: no field takes typing, or a password).
    pub fn of(hwnd: Hwnd) -> Option<Context> {
        let hwnd = HWND(hwnd.value() as *mut _);
        // SAFETY: a query of the window's context (Win32 checks the handle); released in `drop`.
        let himc = unsafe { ImmGetContext(hwnd) };
        (!himc.is_invalid()).then_some(Context { hwnd, himc })
    }

    fn bytes(&self, index: IME_COMPOSITION_STRING) -> Vec<u8> {
        // SAFETY: the length query (no buffer), then the bytes into a buffer of that length.
        unsafe {
            let len = ImmGetCompositionStringW(self.himc, index, None, 0);
            if len <= 0 {
                return Vec::new();
            }
            let mut buffer = vec![0u8; len as usize];
            let got = ImmGetCompositionStringW(self.himc, index, Some(buffer.as_mut_ptr().cast()), len as u32);
            buffer.truncate(got.max(0) as usize);
            buffer
        }
    }

    /// The composing text or the result (UTF-16).
    pub fn string(&self, part: Part) -> Vec<u16> {
        let index = match part {
            Part::Composition => GCS_COMPSTR,
            Part::Result => GCS_RESULTSTR,
        };
        self.bytes(index).as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect()
    }

    /// The composing text's attributes (ATTR_*: a byte per unit).
    pub fn attributes(&self) -> Vec<u8> {
        self.bytes(GCS_COMPATTR)
    }

    /// The composition's caret (a unit of it), none where the input method has none.
    pub fn cursor(&self) -> Option<usize> {
        // SAFETY: GCS_CURSORPOS answers in the return value (no buffer).
        let at = unsafe { ImmGetCompositionStringW(self.himc, GCS_CURSORPOS, None, 0) };
        usize::try_from(at).ok()
    }

    /// Its composition window at `point` (CFS_POINT: where an input method that shows one would show it — KompozR
    /// draws the composing text itself) and its candidate windows kept out of `exclude` (CFS_EXCLUDE: below the
    /// caret's box, or above it where there is no room), client coordinates.
    pub fn place(&self, point: (i32, i32), exclude: CharPosition) {
        let area = RECT { left: exclude.left, top: exclude.top, right: exclude.right, bottom: exclude.bottom };
        let composition =
            COMPOSITIONFORM { dwStyle: CFS_POINT, ptCurrentPos: POINT { x: point.0, y: point.1 }, rcArea: area };
        // SAFETY: the forms are read during the calls.
        unsafe {
            let _ = ImmSetCompositionWindow(self.himc, &composition);
            for index in 0..4 {
                let candidate = CANDIDATEFORM {
                    dwIndex: index,
                    dwStyle: CFS_EXCLUDE,
                    ptCurrentPos: POINT { x: exclude.left, y: exclude.bottom },
                    rcArea: area,
                };
                let _ = ImmSetCandidateWindow(self.himc, &candidate);
            }
        }
    }

    /// Where its composition window is (the style, the point), as set.
    #[cfg(test)]
    pub fn composition_window(&self) -> Option<(u32, (i32, i32))> {
        let mut form = COMPOSITIONFORM::default();
        // SAFETY: the form is written during the call.
        let ok = unsafe { ::windows::Win32::UI::Input::Ime::ImmGetCompositionWindow(self.himc, &mut form) }.as_bool();
        ok.then_some((form.dwStyle, (form.ptCurrentPos.x, form.ptCurrentPos.y)))
    }

    /// Where its first candidate window is (the style, the rectangle kept out of), as set.
    #[cfg(test)]
    pub fn candidate_window(&self) -> Option<(u32, CharPosition)> {
        let mut form = CANDIDATEFORM::default();
        // SAFETY: the form is written during the call.
        let ok = unsafe { ::windows::Win32::UI::Input::Ime::ImmGetCandidateWindow(self.himc, 0, &mut form) }.as_bool();
        let r = form.rcArea;
        ok.then_some((form.dwStyle, CharPosition { left: r.left, top: r.top, right: r.right, bottom: r.bottom }))
    }

    /// The composition given up (NI_COMPOSITIONSTR, CPS_CANCEL), or kept as it stands (CPS_COMPLETE: its result
    /// comes as a composition's).
    pub fn end_composition(&self, keep: bool) {
        let how = if keep { CPS_COMPLETE } else { CPS_CANCEL };
        // SAFETY: a notification of the context.
        let _ = unsafe { ImmNotifyIME(self.himc, NI_COMPOSITIONSTR, how, 0) };
    }

    /// The composing text replaced (SCS_SETSTR: what the app made of it, or a test's); whether the input method
    /// took it.
    pub fn set_composition(&self, text: &[u16]) -> bool {
        let bytes = (text.len() * 2) as u32;
        // SAFETY: the string is read during the call (its length in bytes); no reading string.
        unsafe { ImmSetCompositionStringW(self.himc, SCS_SETSTR, Some(text.as_ptr().cast()), bytes, None, 0) }.as_bool()
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        // SAFETY: a query of the context.
        unsafe { ::windows::Win32::UI::Input::Ime::ImmGetOpenStatus(self.himc) }.as_bool()
    }

    #[cfg(test)]
    pub fn set_open(&self, open: bool) {
        // SAFETY: a change of the context's state.
        let _ = unsafe { ::windows::Win32::UI::Input::Ime::ImmSetOpenStatus(self.himc, open) };
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: the context `of` got for this window.
        let _ = unsafe { ImmReleaseContext(self.hwnd, self.himc) };
    }
}

/// The window's default input context associated with it (a field takes typing) or none (the input methods off,
/// as C++'s outside fields).
pub fn associate(hwnd: Hwnd, on: bool) {
    let hwnd = HWND(hwnd.value() as *mut _);
    // SAFETY: Win32 checks the handle; IACE_DEFAULT restores the window's default context, 0 with none removes it.
    let _ = unsafe { ImmAssociateContextEx(hwnd, HIMC::default(), if on { IACE_DEFAULT } else { 0 }) };
}

/// The input scopes of the window's typing (`SetInputScopes`, msctf's: IMM32's input methods, the touch keyboard
/// and TSF read them); none: the default. Whether it took them.
pub fn set_input_scopes(hwnd: Hwnd, scopes: &[i32]) -> bool {
    ::windows_core::link!("msctf.dll" "system" fn SetInputScopes(hwnd: HWND, scopes: *const i32, count: u32, phrases: *const *const u16, phrase_count: u32, regex: *const u16, srgs: *const u16) -> ::windows_core::HRESULT);
    let hwnd = HWND(hwnd.value() as *mut _);
    let (ptr, count) = if scopes.is_empty() { (std::ptr::null(), 0) } else { (scopes.as_ptr(), scopes.len() as u32) };
    // SAFETY: the scopes are read during the call (no phrases, expressions or grammars).
    unsafe { SetInputScopes(hwnd, ptr, count, std::ptr::null(), 0, std::ptr::null(), std::ptr::null()) }.is_ok()
}

/// What an input method asks of the window (WM_IME_REQUEST's wParam), as the platform answers it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Request {
    /// Where the composition window goes (a COMPOSITIONFORM to fill).
    CompositionWindow,
    /// Where a candidate window goes (a CANDIDATEFORM to fill, its index set).
    CandidateWindow,
    /// The selection to reconvert (a RECONVERTSTRING: its size asked first, with no buffer).
    Reconvert,
    /// The part it reconverts, which it may have moved (the RECONVERTSTRING it was given, changed).
    ConfirmReconvert,
    /// Where a character of the composition is (an IMECHARPOSITION).
    CharPosition,
    /// The text around the composition (a RECONVERTSTRING, as `Reconvert`).
    DocumentFeed,
}

impl Request {
    /// The request a message is, none if it isn't one of these.
    pub fn of(m: &WindowMessage) -> Option<Request> {
        if m.id() != WM_IME_REQUEST {
            return None;
        }
        Some(match m.w() as u32 {
            IMR_COMPOSITIONWINDOW => Request::CompositionWindow,
            IMR_CANDIDATEWINDOW => Request::CandidateWindow,
            IMR_RECONVERTSTRING => Request::Reconvert,
            IMR_CONFIRMRECONVERTSTRING => Request::ConfirmReconvert,
            IMR_QUERYCHARPOSITION => Request::CharPosition,
            IMR_DOCUMENTFEED => Request::DocumentFeed,
            _ => return None,
        })
    }
}

/// The most of a RECONVERTSTRING read back (a sane bound on what an input method says its size is).
const RECONVERT_MAX_BYTES: usize = 1 << 20;

/// A RECONVERTSTRING request answered with `encoded` (one [`Reconversion::encode`](super::ime::Reconversion::encode)
/// made): its size where no buffer came (the input method asks for it first), else the buffer filled where it is
/// large enough (its dwSize), and its size; 0 where it isn't.
pub fn answer_reconversion(m: &WindowMessage, encoded: &[u8]) -> isize {
    if !matches!(Request::of(m), Some(Request::Reconvert | Request::DocumentFeed)) {
        return 0;
    }
    if m.l() == 0 {
        return encoded.len() as isize;
    }
    // SAFETY: a RECONVERTSTRING request's lParam, when not 0, points to a buffer of the input method's whose first
    // DWORD (dwSize) says how large it is, valid while the message is handled; written only within that size.
    unsafe {
        let buffer = m.l() as *mut u8;
        let size = (buffer as *const u32).read_unaligned() as usize;
        if size < encoded.len() {
            return 0;
        }
        std::ptr::copy_nonoverlapping(encoded.as_ptr(), buffer, encoded.len());
    }
    encoded.len() as isize
}

/// The RECONVERTSTRING an IMR_CONFIRMRECONVERTSTRING passes, as its bytes (its dwSize of them).
pub fn confirmed_reconversion(m: &WindowMessage) -> Option<Vec<u8>> {
    if Request::of(m) != Some(Request::ConfirmReconvert) || m.l() == 0 {
        return None;
    }
    // SAFETY: the request's lParam points to the input method's RECONVERTSTRING, dwSize bytes long (bounded),
    // valid while the message is handled.
    unsafe {
        let buffer = m.l() as *const u8;
        let size = (buffer as *const u32).read_unaligned() as usize;
        if !(super::ime::RECONVERTSTRING_HEADER..=RECONVERT_MAX_BYTES).contains(&size) {
            return None;
        }
        Some(std::slice::from_raw_parts(buffer, size).to_vec())
    }
}

/// An IMR_QUERYCHARPOSITION answered: the character's top left, its line's height and the document's rectangle
/// (screen coordinates); 1 (answered).
pub fn answer_char_position(m: &WindowMessage, at: CharPosition, document: CharPosition) -> isize {
    if Request::of(m) != Some(Request::CharPosition) || m.l() == 0 {
        return 0;
    }
    // SAFETY: the request's lParam points to the input method's IMECHARPOSITION (its dwSize and dwCharPos set),
    // valid while the message is handled; its other fields written.
    unsafe {
        let p = &mut *(m.l() as *mut IMECHARPOSITION);
        p.pt = POINT { x: at.left, y: at.top };
        p.cLineHeight = (at.bottom - at.top).max(1) as u32;
        p.rcDocument = RECT { left: document.left, top: document.top, right: document.right, bottom: document.bottom };
    }
    1
}

/// An IMR_COMPOSITIONWINDOW or IMR_CANDIDATEWINDOW answered with where the windows go (as [`Context::place`]
/// sets them); 1 (answered).
pub fn answer_window(m: &WindowMessage, point: (i32, i32), exclude: CharPosition) -> isize {
    let area = RECT { left: exclude.left, top: exclude.top, right: exclude.right, bottom: exclude.bottom };
    match Request::of(m) {
        _ if m.l() == 0 => 0,
        Some(Request::CompositionWindow) => {
            // SAFETY: the request's lParam points to the input method's COMPOSITIONFORM, valid while the message is
            // handled.
            unsafe {
                *(m.l() as *mut COMPOSITIONFORM) = COMPOSITIONFORM {
                    dwStyle: CFS_POINT,
                    ptCurrentPos: POINT { x: point.0, y: point.1 },
                    rcArea: area,
                };
            }
            1
        }
        Some(Request::CandidateWindow) => {
            // SAFETY: the request's lParam points to the input method's CANDIDATEFORM (its dwIndex set), valid
            // while the message is handled.
            unsafe {
                let form = &mut *(m.l() as *mut CANDIDATEFORM);
                form.dwStyle = CFS_EXCLUDE;
                form.ptCurrentPos = POINT { x: exclude.left, y: exclude.bottom };
                form.rcArea = area;
            }
            1
        }
        _ => 0,
    }
}
