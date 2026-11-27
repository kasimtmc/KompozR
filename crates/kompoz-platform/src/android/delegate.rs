//! The app's side of accessibility, content capture, translation, autofill and scroll capture on Android (Compose's
//! AndroidComposeViewAccessibilityDelegateCompat, AndroidContentCaptureManager, AndroidAutofillManager and
//! ScrollCapture, as AndroidComposeView wires them): KompozView's Kotlin classes (KompozAccessibility,
//! KompozContentCapture, KompozAutofill, KompozScrollCapture) ask through the natives here, on the UI thread, and are
//! told back through KompozView's methods.
//!
//! Nothing is pushed while nobody listens: after a frame whose semantics or layout changed, KompozView is told once
//! (`accessibilityChanged`) until it asks again; it asks at most every 100 ms (Compose's
//! SendRecurringAccessibilityEventsIntervalMillis), and only while accessibility or content capture is on. Each ask
//! builds the tree from the host there and then (the UI thread holds the host: no waiting for a frame to publish
//! it, C++'s sleep not copied) and gives back the tree, and the events since the tree last told. A node's info
//! comes from KompozAccessibility's copy of the last tree, asked for again whenever it is stale; while the app is
//! busy (Java calling back into a native call of ours) an ask is answered with nothing and the copy kept.
//!
//! The host's autofill callbacks are queued and told after the frame (their values typed by their nodes' content
//! data types, read from the host then). Scroll capture's scrolls run in a coroutine of the host's without
//! animation (Compose's DisableAnimationMotionDurationScale); its pictures are the kept frame read back a frame after
//! each scroll (Compose's `withFrameNanos`), as KompozScrollCapture draws them into the capture's surface.
//!
//! An FFI module: what Java passes is raw; each `unsafe` block says why it holds.

#![allow(unsafe_code)]

use super::accessibility::{
    AndroidTree, INVALID_ID, Writer, android_tree, decode_request, encode_events, encode_tree, events,
};
use super::app::{Running, with_view};
use super::autofill::{AutofillValue, Notice, encode_notices, encode_structure};
use super::capture::{CaptureTree, Translation, all, capture_events};
use super::jni::{Env, Global, JNIEnv, JNINativeMethod, JObject, MethodId, Vm, jboolean, jfloat, jint, jlong, native};
use super::natives::{env, guard};
use super::semantics::{self, RelativeScroller};
use crate::accessibility::{Snapshot, Window};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{IntRect, Rect};
use kompoz_ui::KompozHost;
use kompoz_ui::MotionDurationScale;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

/// KompozView's methods called here.
struct Calls {
    vm: Vm,
    view: Global,
    accessibility_changed: MethodId,
    autofill_notify: MethodId,
    scroll_capture_image: MethodId,
    scroll_capture_ended: MethodId,
}

impl Calls {
    fn new(env: &Env<'_>, view: JObject) -> Option<Calls> {
        let class = env.class_of(view)?;
        let m = |name: &std::ffi::CStr, signature: &std::ffi::CStr| env.method(&class, name, signature);
        Some(Calls {
            vm: env.vm()?,
            view: env.new_global(view)?,
            accessibility_changed: m(c"accessibilityChanged", c"()V")?,
            autofill_notify: m(c"autofillNotify", c"([B)V")?,
            scroll_capture_image: m(c"scrollCaptureImage", c"(I[I[BII)V")?,
            scroll_capture_ended: m(c"scrollCaptureEnded", c"(I)V")?,
        })
    }

    fn with(&self, f: impl FnOnce(&Env<'static>, JObject) -> Option<()>) {
        if let Some(env) = self.vm.env() {
            let _ = f(&env, self.view.object());
        }
    }
}

/// What a scroll capture session has under way.
struct ScrollCapture {
    id: u64,
    viewport: IntRect,
    scroller: Rc<Cell<RelativeScroller>>,
    scope: CoroutineScope,
    /// A picture asked for: its token, the area asked (px from where the scroll started, the viewport's
    /// coordinates), and whether its scroll is done (then a frame drawn after it before it is read).
    image: Option<(i32, IntRect, Rc<Cell<bool>>, bool)>,
    /// The session's end asked for: its token and whether the scroll back is done.
    end: Option<(i32, Rc<Cell<bool>>)>,
}

/// What this keeps between calls.
#[derive(Default)]
struct Delegate {
    calls: Option<Calls>,
    /// The tree last built: what the finger is found in while the app is busy, what ids are of.
    current: AndroidTree,
    /// The tree last told, with its snapshot: what the next events are of.
    told: Option<(Snapshot, AndroidTree)>,
    /// KompozView was told the semantics changed and hasn't asked since.
    changed_told: bool,
    /// What content capture was last told, and whether a session is on.
    captured: CaptureTree,
    capturing: bool,
    translation: Translation,
    scroll: Option<ScrollCapture>,
}

thread_local! {
    static DELEGATE: RefCell<Delegate> = RefCell::new(Delegate::default());
    /// The host's autofill callbacks, told after the frame.
    static NOTICES: RefCell<Vec<Pending>> = const { RefCell::new(Vec::new()) };
}

/// An autofill notification waiting for the frame's end.
pub(super) enum Pending {
    Notice(Notice),
    /// A value as the host told it (text), typed by its node's content data type when told.
    Value(u64, String),
}

/// The activity's view: what it is told through (as its app is made).
pub(super) fn attach(env: &Env<'_>, view: JObject) {
    let calls = Calls::new(env, view);
    if calls.is_none() {
        super::log_error("kompoz: KompozView's accessibility methods aren't there: no accessibility");
    }
    DELEGATE.with(|d| *d.borrow_mut() = Delegate { calls, ..Delegate::default() });
    NOTICES.with(|n| n.borrow_mut().clear());
}

/// The app gone: nothing kept.
pub(super) fn detach() {
    DELEGATE.with(|d| {
        if let Ok(mut d) = d.try_borrow_mut() {
            *d = Delegate::default();
        }
    });
}

/// An autofill notification from the host's callbacks, told after the frame.
pub(super) fn queue_autofill(pending: Pending) {
    NOTICES.with(|n| n.borrow_mut().push(pending));
}

/// After a frame's composition (`Platform::composed`): KompozView told once that the tree changed (its semantics,
/// or what was drawn: bounds move), the autofill notifications told.
pub(super) fn composed(host: &KompozHost, redraw: bool) {
    let changed = host.take_semantics_changed() || redraw;
    DELEGATE.with(|d| {
        let Ok(mut d) = d.try_borrow_mut() else { return };
        if changed && !d.changed_told {
            d.changed_told = true;
            if let Some(c) = &d.calls {
                c.with(|env, view| env.call_void(view, c.accessibility_changed, &[]).then_some(()));
            }
        }
    });
    let pending = NOTICES.with(|n| std::mem::take(&mut *n.borrow_mut()));
    if pending.is_empty() {
        return;
    }
    let notices: Vec<Notice> = pending
        .into_iter()
        .filter_map(|p| match p {
            Pending::Notice(n) => Some(n),
            Pending::Value(id, text) => {
                let value = AutofillValue::parse(semantics::content_data_type(host, id), &text)?;
                Some(Notice::Value { id: i32::try_from(id).ok()?, value })
            }
        })
        .collect();
    let bytes = encode_notices(&notices);
    DELEGATE.with(|d| {
        let Ok(d) = d.try_borrow() else { return };
        if let Some(c) = &d.calls {
            c.with(|env, view| {
                let array = env.new_bytes(&bytes)?;
                env.call_void(view, c.autofill_notify, &[array.object().into()]).then_some(())
            });
        }
    });
}

/// The tree now (built from the host) and, if `events`, what changed since the tree last told.
fn build(host: &KompozHost, with_events: bool) -> Vec<u8> {
    let nodes = host.accessibility_tree();
    let extras = semantics::extras(host, &nodes);
    let snapshot = Snapshot::new(nodes, Window::default());
    let tree = android_tree(&snapshot, &extras);
    let mut w = Writer(encode_tree(&tree));
    DELEGATE.with(|d| {
        let mut d = d.borrow_mut();
        d.changed_told = false;
        if with_events {
            let told = d.told.take();
            let list = match &told {
                Some((s, t)) => events((s, t), (&snapshot, &tree)),
                None => Vec::new(),
            };
            encode_events(&list, &mut w);
            d.current = tree.clone();
            d.told = Some((snapshot, tree));
        } else {
            d.current = tree;
        }
    });
    w.0
}

// ---- Scroll capture's frames ----

/// After each frame: a picture asked for once its scroll is done and a frame drawn after it, read and handed to
/// KompozScrollCapture; the session's end told once its scroll back is done; frames asked for while either waits.
pub(super) fn after_frame(app: &mut Running) {
    let mut image = None;
    let mut ended = None;
    let mut waiting = false;
    DELEGATE.with(|d| {
        let Ok(mut d) = d.try_borrow_mut() else { return };
        let Some(s) = d.scroll.as_mut() else { return };
        if let Some((token, area, done, drawn)) = s.image.as_mut() {
            if done.get() && *drawn {
                image = Some((*token, *area, s.viewport, s.scroller.get()));
                s.image = None;
            } else {
                *drawn = done.get();
                waiting = true;
            }
        }
        if let Some((token, done)) = &s.end {
            if done.get() {
                ended = Some(*token);
                d.scroll = None;
            } else {
                waiting = true;
            }
        }
    });
    let had_image = image.is_some();
    if let Some((token, area, viewport, scroller)) = image {
        // (The part of the area in the viewport now; its place in the capture where the scroll put it.)
        let (top, bottom) = (scroller.to_viewport(area.top), scroller.to_viewport(area.bottom));
        let mut result = [0; 4];
        let mut pixels = None;
        if top != bottom {
            let shift = scroller.amount.round() as i32;
            result = [area.left, top + shift, area.right, bottom + shift];
            let (x, y) = (viewport.left + area.left, viewport.top + top);
            pixels = app.read_frame(x, y, area.right - area.left, bottom - top);
        }
        tell_image(token, result, pixels);
    }
    if let Some(token) = ended {
        DELEGATE.with(|d| {
            let d = d.borrow();
            if let Some(c) = &d.calls {
                c.with(|env, view| env.call_void(view, c.scroll_capture_ended, &[token.into()]).then_some(()));
            }
        });
    }
    if waiting || had_image {
        app.request_frame();
    }
}

/// A picture for KompozScrollCapture: the rectangle it covers in the capture, and its pixels (none: nothing to
/// draw).
fn tell_image(token: i32, rect: [i32; 4], pixels: Option<(i32, i32, Vec<u8>)>) {
    DELEGATE.with(|d| {
        let d = d.borrow();
        let Some(c) = &d.calls else { return };
        c.with(|env, view| {
            let rect = env.new_ints(&rect)?;
            let (w, h) = pixels.as_ref().map_or((0, 0), |p| (p.0, p.1));
            let bytes = pixels.as_ref().and_then(|p| env.new_bytes(&p.2));
            let args = [
                token.into(),
                rect.object().into(),
                bytes.as_ref().map_or(JObject::null(), |b| b.object()).into(),
                w.into(),
                h.into(),
            ];
            env.call_void(view, c.scroll_capture_image, &args).then_some(())
        });
    });
}

/// Scrolls the captured node by `delta` (px from where it is), in the capture's coroutine; `done` set once it has
/// (the scroller's amount moved by how far it went).
fn scroll_capture_by(host: &KompozHost, s: &ScrollCapture, delta: f32, done: Rc<Cell<bool>>) {
    let Some(scroll) = semantics::scroll_by_offset(host, s.id, delta) else {
        done.set(true);
        return;
    };
    let scroller = s.scroller.clone();
    s.scope.launch(async move {
        let consumed = scroll.await;
        let mut r = scroller.get();
        r.amount += consumed;
        scroller.set(r);
        done.set(true);
    });
}

// ---- The natives (KompozView's) ----

/// KompozView's natives of this module.
pub(super) fn view_natives() -> Vec<JNINativeMethod> {
    vec![
        native(c"nativeAccessibilityTree", c"(Z)[B", native_tree as *mut c_void),
        native(c"nativeAccessibilityHit", c"(FF)I", native_hit as *mut c_void),
        native(c"nativeAccessibilityAction", c"(IIIIF[B)Z", native_action as *mut c_void),
        native(c"nativeCharacterBounds", c"(III)[F", native_character_bounds as *mut c_void),
        native(c"nativeContentCapture", c"(I)[B", native_content_capture as *mut c_void),
        native(c"nativeTranslationText", c"(J)[B", native_translation_text as *mut c_void),
        native(c"nativeTranslation", c"(J[B)Z", native_translation as *mut c_void),
        native(c"nativeShowTranslation", c"(I)V", native_show_translation as *mut c_void),
        native(c"nativeAutofillStructure", c"()[B", native_autofill_structure as *mut c_void),
        native(c"nativeAutofill", c"(IIJ[B)Z", native_autofill as *mut c_void),
        native(c"nativeScrollCaptureSearch", c"()[I", native_scroll_capture_search as *mut c_void),
        native(c"nativeScrollCaptureStart", c"()Z", native_scroll_capture_start as *mut c_void),
        native(c"nativeScrollCaptureRequest", c"(IIIII)V", native_scroll_capture_request as *mut c_void),
        native(c"nativeScrollCaptureEnd", c"(I)V", native_scroll_capture_end as *mut c_void),
    ]
}

/// `f` on the host of the view's app (what it reads); none while the app is busy or before it started.
fn with_host<R>(env: &Env<'_>, view: JObject, f: impl FnOnce(&KompozHost) -> R) -> Option<R> {
    with_view(env, view, |app| app.host().map(f)).flatten()
}

/// [`with_host`] for what changes the app: a frame asked for after it (what it did shows).
fn acting_on_host<R>(env: &Env<'_>, view: JObject, f: impl FnOnce(&KompozHost) -> R) -> Option<R> {
    with_view(env, view, |app| {
        let r = app.host().map(f);
        app.request_frame();
        r
    })
    .flatten()
}

/// A byte array for Java (null: none).
fn bytes_out(env: &Env<'_>, bytes: Option<Vec<u8>>) -> JObject {
    bytes.and_then(|b| env.new_bytes(&b)).map_or(JObject::null(), |a| a.into_object())
}

/// The semantics id of virtual view `id` in the tree last built.
fn semantics_id(id: i32) -> Option<u64> {
    DELEGATE.with(|d| d.borrow().current.get(id).map(|n| n.semantics_id))
}

/// `nativeAccessibilityTree(events): ByteArray?`: the tree now, and (if `events`) what changed since it was last
/// told; null while the app is busy.
extern "system" fn native_tree(raw: *mut JNIEnv, view: JObject, with_events: jboolean) -> JObject {
    guard("nativeAccessibilityTree", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let bytes = with_host(&env, view, |host| build(host, with_events != 0));
        bytes_out(&env, bytes)
    })
}

/// `nativeAccessibilityHit(x, y): Int`: the virtual view a screen reader's finger at (x, y) (px in the view) finds.
extern "system" fn native_hit(_raw: *mut JNIEnv, _view: JObject, x: jfloat, y: jfloat) -> jint {
    guard("nativeAccessibilityHit", INVALID_ID, || DELEGATE.with(|d| d.borrow().current.hit(x, y)))
}

/// `nativeAccessibilityAction(id, request, a, b, value, text): Boolean`: KompozAccessibility's request on a virtual
/// view (its `REQUEST_*` with its arguments).
#[allow(clippy::too_many_arguments)]
extern "system" fn native_action(
    raw: *mut JNIEnv,
    view: JObject,
    id: jint,
    code: jint,
    a: jint,
    b: jint,
    value: jfloat,
    text: JObject,
) -> jboolean {
    guard("nativeAccessibilityAction", 0, || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(request) = decode_request(code, a, b, value, env.utf8(text)) else { return 0 };
        let Some(target) = semantics_id(id) else { return 0 };
        acting_on_host(&env, view, |host| semantics::perform(host, target, request)).unwrap_or(false) as jboolean
    })
}

/// `nativeCharacterBounds(id, start, length): FloatArray?`: each character's box (four floats, px in the view; NaN
/// for one not shown).
extern "system" fn native_character_bounds(
    raw: *mut JNIEnv,
    view: JObject,
    id: jint,
    start: jint,
    length: jint,
) -> JObject {
    guard("nativeCharacterBounds", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(target) = semantics_id(id) else { return JObject::null() };
        let Some(boxes) = with_host(&env, view, |host| semantics::character_bounds(host, target, start, length)) else {
            return JObject::null();
        };
        let nan = Rect { left: f32::NAN, top: f32::NAN, right: f32::NAN, bottom: f32::NAN };
        let floats: Vec<f32> = boxes
            .into_iter()
            .flat_map(|b| {
                let r = b.unwrap_or(nan);
                [r.left, r.top, r.right, r.bottom]
            })
            .collect();
        env.new_floats(&floats).map_or(JObject::null(), |a| a.into_object())
    })
}

/// What `nativeContentCapture` is asked: the changes, every node appearing (a session started), every node
/// disappearing (it stopped).
const CAPTURE_CHANGES: i32 = 0;
const CAPTURE_START: i32 = 1;
const CAPTURE_STOP: i32 = 2;

/// `nativeContentCapture(mode): ByteArray?`: what the content capture session is told (KompozContentCapture's
/// events); null while the app is busy.
extern "system" fn native_content_capture(raw: *mut JNIEnv, view: JObject, mode: jint) -> JObject {
    guard("nativeContentCapture", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        // (Acting: nodes that came show their translations, which needs a frame.)
        let bytes = acting_on_host(&env, view, |host| {
            let tree = CaptureTree::new(semantics::capture_nodes(host));
            let (list, translated) = DELEGATE.with(|d| {
                let mut d = d.borrow_mut();
                d.changed_told = false; // (told again at the next change: content capture asks too)
                let list = match mode {
                    CAPTURE_START => all(&tree, true),
                    CAPTURE_STOP => all(&d.captured, false),
                    CAPTURE_CHANGES if d.capturing => capture_events(&d.captured, &tree),
                    _ => Vec::new(),
                };
                d.capturing = mode != CAPTURE_STOP && (d.capturing || mode == CAPTURE_START);
                d.captured = if mode == CAPTURE_STOP { CaptureTree::default() } else { tree };
                (list, d.translation == Translation::Translated)
            });
            // (Nodes come while translated show their translations: Compose's `updateTranslationOnAppeared`.)
            let appeared: Vec<u64> = list
                .iter()
                .filter_map(|e| if let super::capture::CaptureEvent::Appeared(n) = e { Some(n.id) } else { None })
                .collect();
            if mode != CAPTURE_STOP {
                semantics::translate_appeared(host, &appeared, translated);
            }
            super::capture::encode_events(&list)
        });
        bytes_out(&env, bytes)
    })
}

/// `nativeTranslationText(id): ByteArray?`: what node `id` asks to be translated (null: nothing).
extern "system" fn native_translation_text(raw: *mut JNIEnv, view: JObject, id: jlong) -> JObject {
    guard("nativeTranslationText", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let text = with_host(&env, view, |host| semantics::translation_text(host, id as u64)).flatten();
        bytes_out(&env, text.map(String::into_bytes))
    })
}

/// `nativeTranslation(id, text): Boolean`: node `id`'s translation.
extern "system" fn native_translation(raw: *mut JNIEnv, view: JObject, id: jlong, text: JObject) -> jboolean {
    guard("nativeTranslation", 0, || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(text) = env.utf8(text) else { return 0 };
        acting_on_host(&env, view, |host| semantics::set_translation(host, id as u64, &text)).unwrap_or(false)
            as jboolean
    })
}

/// `nativeShowTranslation(mode)`: the translations shown (1), the texts' own shown (0), or the translations
/// cleared (2): the view's translation callback's.
extern "system" fn native_show_translation(raw: *mut JNIEnv, view: JObject, mode: jint) {
    guard("nativeShowTranslation", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let translated = mode == 1;
        DELEGATE.with(|d| {
            d.borrow_mut().translation = if translated { Translation::Translated } else { Translation::Original }
        });
        let show = match mode {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        acting_on_host(&env, view, |host| semantics::show_translations(host, show));
    })
}

/// `nativeAutofillStructure(): ByteArray?`: the view's autofill structure (null while the app is busy).
extern "system" fn native_autofill_structure(raw: *mut JNIEnv, view: JObject) -> JObject {
    guard("nativeAutofillStructure", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let bytes = with_host(&env, view, |host| encode_structure(&semantics::autofill_nodes(host)));
        bytes_out(&env, bytes)
    })
}

/// `nativeAutofill(id, type, number, text): Boolean`: what an autofill service filled into node `id` (its
/// AutofillValue's type, its toggle, list index or date, its text).
extern "system" fn native_autofill(
    raw: *mut JNIEnv,
    view: JObject,
    id: jint,
    kind: jint,
    number: jlong,
    text: JObject,
) -> jboolean {
    guard("nativeAutofill", 0, || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(data) = AutofillValue::fillable(kind, number, env.utf8(text)) else { return 0 };
        let Ok(id) = u64::try_from(id) else { return 0 };
        acting_on_host(&env, view, |host| host.perform_autofill(id, data)).unwrap_or(false) as jboolean
    })
}

/// `nativeScrollCaptureSearch(): IntArray?`: the scrollable a long screenshot captures — its viewport (left, top,
/// right, bottom, px in the view) — kept for the session; null if there is none.
extern "system" fn native_scroll_capture_search(raw: *mut JNIEnv, view: JObject) -> JObject {
    guard("nativeScrollCaptureSearch", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let found = acting_on_host(&env, view, |host| {
            let (id, viewport) = semantics::scroll_capture_candidate(host)?;
            // (No animation: each scroll goes at once, as Compose's DisableAnimationMotionDurationScale.)
            let context = host.effect_coroutine_context().plus_element(MotionDurationScale::new(0.0));
            let scope = CoroutineScope::new_with_context(host.dispatcher(), context);
            let scroller = RelativeScroller { viewport: viewport.bottom - viewport.top, amount: 0.0 };
            let capture =
                ScrollCapture { id, viewport, scroller: Rc::new(Cell::new(scroller)), scope, image: None, end: None };
            DELEGATE.with(|d| {
                if let Some(old) = d.borrow_mut().scroll.replace(capture) {
                    old.scope.cancel();
                }
            });
            Some(viewport)
        })
        .flatten();
        let Some(v) = found else { return JObject::null() };
        env.new_ints(&[v.left, v.top, v.right, v.bottom]).map_or(JObject::null(), |a| a.into_object())
    })
}

/// `nativeScrollCaptureStart(): Boolean`: a session starts on the scrollable found (from where it is).
extern "system" fn native_scroll_capture_start(_raw: *mut JNIEnv, _view: JObject) -> jboolean {
    guard("nativeScrollCaptureStart", 0, || {
        DELEGATE.with(|d| {
            let mut d = d.borrow_mut();
            let Some(s) = d.scroll.as_mut() else { return 0 };
            let mut r = s.scroller.get();
            r.amount = 0.0;
            s.scroller.set(r);
            s.image = None;
            s.end = None;
            1
        })
    })
}

/// `nativeScrollCaptureRequest(token, left, top, right, bottom)`: the picture of that area (px from where the
/// scrollable was as the session started, in its viewport's coordinates): scrolled to center it, then told back
/// (`scrollCaptureImage`) a frame later.
extern "system" fn native_scroll_capture_request(
    raw: *mut JNIEnv,
    view: JObject,
    token: jint,
    left: jint,
    top: jint,
    right: jint,
    bottom: jint,
) {
    guard("nativeScrollCaptureRequest", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let done = Rc::new(Cell::new(false));
        let area = IntRect { left, top, right, bottom };
        let started = acting_on_host(&env, view, |host| {
            DELEGATE.with(|d| {
                let mut d = d.borrow_mut();
                let Some(s) = d.scroll.as_mut() else { return false };
                s.image = Some((token, area, done.clone(), false));
                match s.scroller.get().delta_to_center(top, bottom.max(top)) {
                    Some(delta) => scroll_capture_by(host, s, delta, done.clone()),
                    None => done.set(true),
                }
                true
            })
        });
        if started != Some(true) {
            tell_image(token, [0; 4], None);
        }
    })
}

/// `nativeScrollCaptureEnd(token)`: the session ends: the scrollable back where it was, then told
/// (`scrollCaptureEnded`).
extern "system" fn native_scroll_capture_end(raw: *mut JNIEnv, view: JObject, token: jint) {
    guard("nativeScrollCaptureEnd", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let done = Rc::new(Cell::new(false));
        let ended = acting_on_host(&env, view, |host| {
            DELEGATE.with(|d| {
                let mut d = d.borrow_mut();
                let Some(s) = d.scroll.as_mut() else { return false };
                s.image = None;
                s.end = Some((token, done.clone()));
                let back = -s.scroller.get().amount;
                if back != 0.0 {
                    scroll_capture_by(host, s, back, done.clone());
                } else {
                    done.set(true);
                }
                true
            })
        });
        if ended != Some(true) {
            DELEGATE.with(|d| {
                let d = d.borrow();
                if let Some(c) = &d.calls {
                    c.with(|env, view| env.call_void(view, c.scroll_capture_ended, &[token.into()]).then_some(()));
                }
            });
        }
    })
}
