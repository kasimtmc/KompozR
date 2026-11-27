//! The input method, the pointer's icon and the text toolbar as the host asks for them (C++'s `KompozInput.show`,
//! `update`, `hide` and its `setCursor`; Compose's `TextInputServiceAndroid`, `CursorAnchorInfoController` and
//! `AndroidTextToolbar`): KompozView's Kotlin side (KompozIme, KompozTextToolbar) called on the UI thread, and what
//! it sends back — the input connection's batches of edits, its editor actions, the content a keyboard commits, a
//! toolbar item clicked — handed to the host.
//!
//! The input method's view of the field is KompozIme's Editable (the source of truth for what it asks: the text
//! around the caret, the extracted text); [`TextInput`] keeps what that Editable holds (`known`) and tells it the
//! field's state whenever the field's differs: the app changed it, a hardware key typed into it, a filter took an
//! edit back. Each session (the field focused, its keyboard shown) has a number; what comes back from an older one
//! is dropped. Where the caret is (the host's focused rect) and the focused field's bounds (its layout node's) go to
//! the input method as its cursor anchor and editor bounds; once the field is laid out (its session's text layout,
//! Compose's `updateTextLayoutResult`), its characters' boxes, its visible lines, the caret's glyph and the
//! decoration box do (Compose's CursorAnchorInfoBuilder), and Android 14's handwriting gestures are hit-tested
//! against it (`handwriting`: KompozIme's `nativeHandwritingGesture`, registered on its class here).

use super::handwriting::{self, Gesture, kind};
use super::ime::{EditorConfig, Utf16State, commands_of, decode_ops, ime_action_of};
use super::jni::{Env, Global, JNIEnv, JObject, MethodId, Vm, jboolean, jfloat, jint, jvalue, native};
use kompoz_graphics::Rect;
use kompoz_text::TextRange;
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::{ClipEntry, ClipItem, ClipMetadata};
use kompoz_ui::platform::{
    ImeOptions, PointerIcon, TextEditState, TextToolbarActions, is_stylus_handwriting_supported,
};
use kompoz_ui::platform_text_layout::PlatformTextLayout;
use kompoz_ui::text_input::TextHighlightType;
use std::cell::{Cell, RefCell};

/// KompozView's methods KompozR calls (looked up as the app is made: the view's class is the app's).
struct ViewCalls {
    vm: Vm,
    view: Global,
    ime_show: MethodId,
    ime_update: MethodId,
    ime_hide: MethodId,
    ime_hide_keyboard: MethodId,
    ime_cursor: MethodId,
    ime_layout: MethodId,
    ime_start_handwriting: MethodId,
    set_pointer_icon: MethodId,
    set_stylus_pointer_icon: MethodId,
    show_text_toolbar: MethodId,
    hide_text_toolbar: MethodId,
}

impl ViewCalls {
    fn new(env: &Env<'_>, view: JObject) -> Option<ViewCalls> {
        let class = env.class_of(view)?;
        let m = |name: &std::ffi::CStr, signature: &std::ffi::CStr| env.method(&class, name, signature);
        Some(ViewCalls {
            vm: env.vm()?,
            view: env.new_global(view)?,
            ime_show: m(c"imeShow", c"(I[B[III[B[BI)V")?,
            ime_update: m(c"imeUpdate", c"(I[B[I)V")?,
            ime_hide: m(c"imeHide", c"(I)V")?,
            ime_hide_keyboard: m(c"imeHideKeyboard", c"()V")?,
            ime_cursor: m(c"imeCursor", c"([F)V")?,
            ime_layout: m(c"imeLayout", c"([F[I[F[FI[F)V")?,
            ime_start_handwriting: m(c"imeStartHandwriting", c"()V")?,
            set_pointer_icon: m(c"setPointerIconType", c"(I)V")?,
            set_stylus_pointer_icon: m(c"setStylusPointerIconType", c"(I)V")?,
            show_text_toolbar: m(c"showTextToolbar", c"([FI)V")?,
            hide_text_toolbar: m(c"hideTextToolbar", c"()V")?,
        })
    }

    /// `f` with the UI thread's env and the view (the host calls its callbacks there): it makes the call's
    /// arguments (its local references deleted as they go) and calls.
    fn with(&self, f: impl FnOnce(&Env<'static>, JObject) -> Option<()>) {
        if let Some(env) = self.vm.env() {
            let _ = f(&env, self.view.object());
        }
    }

    /// `view.method(args)` of plain arguments.
    fn call(&self, method: MethodId, args: &[jvalue]) {
        self.with(|env, view| env.call_void(view, method, args).then_some(()));
    }
}

/// `PointerIcon.TYPE_*` of a KompozR icon (C++'s `setCursor`, Compose's `AndroidPointerIconType`).
pub fn pointer_icon_type(icon: PointerIcon) -> i32 {
    match icon {
        PointerIcon::Default => 1000,          // TYPE_ARROW
        PointerIcon::Hand => 1002,             // TYPE_HAND
        PointerIcon::Wait => 1004,             // TYPE_WAIT
        PointerIcon::Crosshair => 1007,        // TYPE_CROSSHAIR
        PointerIcon::Text => 1008,             // TYPE_TEXT
        PointerIcon::NotAllowed => 1012,       // TYPE_NO_DROP
        PointerIcon::Move => 1013,             // TYPE_ALL_SCROLL
        PointerIcon::ResizeHorizontal => 1014, // TYPE_HORIZONTAL_DOUBLE_ARROW
        PointerIcon::ResizeVertical => 1015,   // TYPE_VERTICAL_DOUBLE_ARROW
        PointerIcon::Handwriting => 1022,      // TYPE_HANDWRITING (Android 14)
    }
}

/// The text toolbar's items as KompozTextToolbar's bits (its `ITEM_*`).
pub mod toolbar_item {
    pub const COPY: i32 = 1;
    pub const PASTE: i32 = 2;
    pub const CUT: i32 = 4;
    pub const SELECT_ALL: i32 = 8;
    pub const AUTOFILL: i32 = 16;
}

/// The input method's side of the app (shared by the host's callbacks and the natives; all on the UI thread).
pub(super) struct TextInput {
    calls: ViewCalls,
    /// The session now (0: none open); each one opened counts up.
    session: Cell<i32>,
    /// Its field's options (the keyboard asked for).
    options: RefCell<Option<ImeOptions>>,
    /// What KompozIme's Editable holds (none: no session).
    known: RefCell<Option<TextEditState>>,
    /// The caret, px in the view (the host's focused rect).
    area: Cell<Rect>,
    /// The focused field's bounds, px in the view.
    editor: Cell<Option<Rect>>,
    /// What the input method was last told of the caret and the field.
    told_cursor: Cell<Option<(Rect, Option<Rect>)>>,
    /// Its field takes content besides text (as the input method was told).
    content: Cell<bool>,
    /// The text toolbar's actions while it shows.
    toolbar: RefCell<TextToolbarActions>,
    /// The field's text layout as last told (none before it is laid out, or without a session).
    layout: RefCell<Option<PlatformTextLayout>>,
    /// The layout and the Editable's state KompozIme's cursor anchor was last told of.
    told_layout: RefCell<Option<(PlatformTextLayout, TextEditState)>>,
}

impl TextInput {
    /// The input method's side of the view `view` (none: KompozView's methods aren't there).
    pub fn new(env: &Env<'_>, view: JObject) -> Option<TextInput> {
        Some(TextInput {
            calls: ViewCalls::new(env, view)?,
            session: Cell::new(0),
            options: RefCell::new(None),
            known: RefCell::new(None),
            area: Cell::new(Rect::Zero),
            editor: Cell::new(None),
            told_cursor: Cell::new(None),
            content: Cell::new(false),
            toolbar: RefCell::new(TextToolbarActions::default()),
            layout: RefCell::new(None),
            told_layout: RefCell::new(None),
        })
        .inspect(|_| register_ime_natives(env))
    }

    /// The host's `text_input`: a session opened (shown, the keyboard with it as its options say), shown again (the
    /// keyboard asked for again: nothing else changed), moved (the caret elsewhere: the cursor anchor told), or
    /// closed. (A field the focus moves to closes the session before and opens its own; KompozIme keeps the
    /// keyboard up between them.)
    pub fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        if !active {
            let session = self.session.replace(0);
            if session != 0 {
                *self.known.borrow_mut() = None;
                *self.options.borrow_mut() = None;
                self.told_cursor.set(None);
                *self.layout.borrow_mut() = None;
                *self.told_layout.borrow_mut() = None;
                self.calls.call(self.calls.ime_hide, &[session.into()]);
            }
            return;
        }
        let open = self.session.get() != 0;
        let same_options = self.options.borrow().as_ref() == Some(options);
        let moved = self.area.get() != area;
        self.area.set(area);
        if open && same_options && moved {
            // (The caret moved: the session goes on.)
            self.update(state);
            self.tell_cursor();
            return;
        }
        let session = if open && same_options { self.session.get() } else { self.session.get().wrapping_add(1).max(1) };
        if session != self.session.get() {
            // (A new session: its content receiver, if any, is set after it opens: `sync` tells it.)
            self.content.set(false);
        }
        self.session.set(session);
        *self.options.borrow_mut() = Some(options.clone());
        self.show(state, options.show_keyboard);
    }

    /// The session now shown to the input method with `state` (its Editable's from now on), the keyboard shown if
    /// `show_keyboard`.
    fn show(&self, state: &TextEditState, show_keyboard: bool) {
        let session = self.session.get();
        let Some(options) = self.options.borrow().clone() else { return };
        *self.known.borrow_mut() = Some(state.clone());
        let config = EditorConfig::of(&options, self.content.get(), is_stylus_handwriting_supported());
        let u = Utf16State::of(state);
        let flags = config.flags(show_keyboard);
        let method = self.calls.ime_show;
        self.calls.with(|env, view| {
            let text = env.new_bytes(state.text.as_bytes())?;
            let offsets =
                env.new_ints(&[u.selection_start, u.selection_end, u.composition_start, u.composition_end])?;
            let private = config.private_ime_options.as_ref().and_then(|p| env.new_bytes(p.as_bytes()));
            let hints = (!config.hint_locales.is_empty())
                .then(|| env.new_bytes(config.hint_locales.join("\n").as_bytes()))
                .flatten();
            let args = [
                session.into(),
                text.object().into(),
                offsets.object().into(),
                config.input_type.into(),
                config.ime_options.into(),
                private.as_ref().map_or(JObject::null(), |p| p.object()).into(),
                hints.as_ref().map_or(JObject::null(), |h| h.object()).into(),
                flags.into(),
            ];
            env.call_void(view, method, &args).then_some(())
        });
        self.told_cursor.set(None);
        *self.told_layout.borrow_mut() = None;
        self.tell_cursor();
        self.tell_layout();
    }

    /// The host's `update_text_layout_result`: the field laid out again or moved; KompozIme's cursor anchor told
    /// where its characters are.
    pub fn update_text_layout_result(&self, layout: &PlatformTextLayout) {
        if self.session.get() == 0 {
            return;
        }
        *self.layout.borrow_mut() = Some(layout.clone());
        self.tell_layout();
    }

    /// The field's layout as KompozIme's cursor anchor reads it (its characters, visible lines, the caret's glyph,
    /// the decoration box: `handwriting::anchor_info`), where it or the Editable's text and caret changed since it
    /// was told; empty arrays where it isn't laid out for the Editable's text.
    fn tell_layout(&self) {
        let (Some(layout), Some(known)) = (self.layout.borrow().clone(), self.known.borrow().clone()) else { return };
        let now = (layout, known);
        if self.told_layout.borrow().as_ref() == Some(&now) {
            return;
        }
        let info = handwriting::anchor_info(&now.1.text, now.1.selection_end, &now.0);
        *self.told_layout.borrow_mut() = Some(now);
        let method = self.calls.ime_layout;
        self.calls.with(|env, view| {
            let none: (&[f32], &[i32]) = (&[], &[]);
            let (characters, flags, lines, marker, marker_flags, editor) = match &info {
                Some(i) => (
                    &i.characters[..],
                    &i.character_flags[..],
                    &i.lines[..],
                    &i.marker[..],
                    i.marker_flags,
                    &i.editor[..],
                ),
                None => (none.0, none.1, none.0, none.0, 0, none.0),
            };
            let (characters, flags, lines) =
                (env.new_floats(characters)?, env.new_ints(flags)?, env.new_floats(lines)?);
            let (marker, editor) = (env.new_floats(marker)?, env.new_floats(editor)?);
            let args = [
                characters.object().into(),
                flags.object().into(),
                lines.object().into(),
                marker.object().into(),
                marker_flags.into(),
                editor.object().into(),
            ];
            env.call_void(view, method, &args).then_some(())
        });
    }

    /// A handwriting gesture of session `session` (KompozIme's encoding: `handwriting::Gesture::decode`), or its
    /// preview, hit-tested against the field's text layout: what it does (`Outcome::encode`, for KompozIme to make
    /// through its connection), or for a preview whether it highlights something (`[1]`; `[]`: nothing to preview);
    /// a preview's cancellation takes its highlight away. Empty: no session, or not a gesture.
    pub fn handwriting_gesture(
        &self,
        host: &KompozHost,
        session: i32,
        gesture: i32,
        values: &[f32],
        margin: f32,
        preview: bool,
    ) -> Vec<i32> {
        if session == 0 || session != self.session.get() {
            return Vec::new();
        }
        let service = host.text_input_service();
        if gesture == kind::CANCEL_PREVIEW {
            service.highlight_chars_in(TextHighlightType::HandwritingSelectPreview, TextRange::Zero);
            return Vec::new();
        }
        let Some(gesture) = Gesture::decode(gesture, values) else { return Vec::new() };
        let text = self.known.borrow().as_ref().map(|k| k.text.clone()).unwrap_or_default();
        let layout = self.layout.borrow().clone();
        if preview {
            let Some(layout) = layout else { return Vec::new() };
            return match handwriting::preview(&text, &layout, gesture) {
                Some((highlight, range)) => {
                    service.highlight_chars_in(highlight, range);
                    vec![1]
                }
                None => Vec::new(),
            };
        }
        // (Done: a preview's highlight goes.)
        service.highlight_chars_in(TextHighlightType::HandwritingSelectPreview, TextRange::Zero);
        match layout {
            Some(layout) => handwriting::perform(&text, &layout, gesture, margin).encode().to_vec(),
            None => handwriting::Outcome::Fallback.encode().to_vec(),
        }
    }

    /// The field's state, told to the input method where its Editable holds something else (the app changed it,
    /// a key typed, a filter took an edit back).
    pub fn update(&self, state: &TextEditState) {
        let session = self.session.get();
        if session == 0 || self.known.borrow().as_ref() == Some(state) {
            return;
        }
        *self.known.borrow_mut() = Some(state.clone());
        let u = Utf16State::of(state);
        let method = self.calls.ime_update;
        self.calls.with(|env, view| {
            let text = env.new_bytes(state.text.as_bytes())?;
            let offsets =
                env.new_ints(&[u.selection_start, u.selection_end, u.composition_start, u.composition_end])?;
            env.call_void(view, method, &[session.into(), text.object().into(), offsets.object().into()]).then_some(())
        });
    }

    /// The software keyboard hidden (Compose's `hideSoftwareKeyboard`): the session stays.
    pub fn hide_keyboard(&self) {
        self.calls.call(self.calls.ime_hide_keyboard, &[]);
    }

    /// The input method's handwriting started (a stylus wrote over the field).
    pub fn start_stylus_handwriting(&self) {
        if self.session.get() != 0 {
            self.calls.call(self.calls.ime_start_handwriting, &[]);
        }
    }

    /// After a frame (and the input's events): the field's state as the host has it told where the Editable
    /// differs; the field's bounds (its layout node's) and the caret told as the cursor anchor where they moved.
    pub fn sync(&self, host: &KompozHost) {
        if self.session.get() == 0 {
            return;
        }
        let service = host.text_input_service();
        let Some(state) = service.session_state() else { return };
        // The field's content receiver (set as its session opened): the input method told again what it takes.
        let content = !service.session_content_mime_types().is_empty();
        if content != self.content.get() {
            self.content.set(content);
            self.show(&state, false);
        }
        self.update(&state);
        self.editor.set(host.focused_node().map(|n| n.coordinates().bounds_in_root()));
        self.tell_cursor();
        self.tell_layout();
    }

    /// The caret and the field to the input method (its cursor anchor and editor bounds), if they moved.
    fn tell_cursor(&self) {
        let now = (self.area.get(), self.editor.get());
        if self.session.get() == 0 || self.told_cursor.get() == Some(now) {
            return;
        }
        self.told_cursor.set(Some(now));
        let (caret, editor) = now;
        let editor = editor.unwrap_or(Rect::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN));
        let values =
            [caret.left, caret.top, caret.right, caret.bottom, editor.left, editor.top, editor.right, editor.bottom];
        let method = self.calls.ime_cursor;
        self.calls.with(|env, view| {
            let array = env.new_floats(&values)?;
            env.call_void(view, method, &[array.object().into()]).then_some(())
        });
    }

    /// A batch of the input connection's edits (`ops`, `texts`: KompozInputConnection's records) of session
    /// `session`, which left its Editable holding `text` with `offsets` (UTF-16): given to the field as commands,
    /// converted against what the Editable held before them; the Editable told again if the field ends up
    /// elsewhere.
    #[allow(clippy::too_many_arguments)]
    pub fn edit(&self, host: &KompozHost, session: i32, ops: &[i32], texts: &[u8], text: String, offsets: &[i32]) {
        if session != self.session.get() || session == 0 {
            return;
        }
        let edits = decode_ops(ops, texts);
        let mut base = match self.known.borrow().clone() {
            Some(known) => known,
            None => return,
        };
        let commands = commands_of(&mut base, &edits);
        let at = |i: usize| offsets.get(i).copied().unwrap_or(-1);
        let left = Utf16State {
            selection_start: at(0),
            selection_end: at(1),
            composition_start: at(2),
            composition_end: at(3),
        }
        .state(text);
        *self.known.borrow_mut() = Some(left);
        if !commands.is_empty() {
            host.send_edit_commands(&commands);
        }
        self.sync(host);
    }

    /// The input method's action key (`performEditorAction`) of session `session`.
    pub fn action(&self, host: &KompozHost, session: i32, editor_action: i32) {
        if session == self.session.get() && session != 0 {
            host.ime_action(ime_action_of(editor_action));
            self.sync(host);
        }
    }

    /// Content the keyboard commits (an image, a GIF: `commitContent`'s URI, its MIME types one a line, its link)
    /// to session `session`'s field: whether it took it.
    pub fn content(
        &self,
        host: &KompozHost,
        session: i32,
        mime_types: &str,
        uri: String,
        link: Option<String>,
    ) -> bool {
        if session != self.session.get() || session == 0 {
            return false;
        }
        let types: Vec<String> = mime_types.lines().filter(|l| !l.is_empty()).map(String::from).collect();
        let entry = ClipEntry::with_metadata(vec![ClipItem::with_uri(uri)], ClipMetadata::new(types));
        let taken = host.commit_content(entry, link.filter(|l| !l.is_empty()));
        self.sync(host);
        taken
    }

    /// The pointer's icon over what it is over (a mouse's; a hovering stylus's unless one of its own is set).
    pub fn cursor(&self, icon: PointerIcon) {
        let kind = pointer_icon_type(icon);
        self.calls.call(self.calls.set_pointer_icon, &[kind.into()]);
    }

    /// A hovering stylus's own icon (none: the system's).
    pub fn stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        let kind = icon.map_or(0, pointer_icon_type);
        self.calls.call(self.calls.set_stylus_pointer_icon, &[kind.into()]);
    }

    /// The floating text toolbar over `rect` (px in the view) with the items `actions` offers.
    pub fn show_text_toolbar(&self, rect: Rect, actions: &TextToolbarActions) {
        *self.toolbar.borrow_mut() = actions.clone();
        let items = [
            (actions.on_copy_requested.is_some(), toolbar_item::COPY),
            (actions.on_paste_requested.is_some(), toolbar_item::PASTE),
            (actions.on_cut_requested.is_some(), toolbar_item::CUT),
            (actions.on_select_all_requested.is_some(), toolbar_item::SELECT_ALL),
            (actions.on_autofill_requested.is_some(), toolbar_item::AUTOFILL),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .fold(0, |bits, (_, bit)| bits | bit);
        let values = [rect.left, rect.top, rect.right, rect.bottom];
        let method = self.calls.show_text_toolbar;
        self.calls.with(|env, view| {
            let array = env.new_floats(&values)?;
            env.call_void(view, method, &[array.object().into(), items.into()]).then_some(())
        });
    }

    pub fn hide_text_toolbar(&self) {
        *self.toolbar.borrow_mut() = TextToolbarActions::default();
        self.calls.call(self.calls.hide_text_toolbar, &[]);
    }

    /// A toolbar item clicked (its `ITEM_*`): its action (run by the caller with nothing of the host borrowed).
    pub fn toolbar_action(&self, item: i32) -> Option<std::rc::Rc<dyn Fn()>> {
        let actions = self.toolbar.borrow();
        match item {
            toolbar_item::COPY => actions.on_copy_requested.clone(),
            toolbar_item::PASTE => actions.on_paste_requested.clone(),
            toolbar_item::CUT => actions.on_cut_requested.clone(),
            toolbar_item::SELECT_ALL => actions.on_select_all_requested.clone(),
            toolbar_item::AUTOFILL => actions.on_autofill_requested.clone(),
            _ => None,
        }
    }
}

/// KompozIme's natives registered on its class (its handwriting gestures' hit tests); logged if they can't be (the
/// gestures then fail, as in a field that doesn't take them).
fn register_ime_natives(env: &Env<'_>) {
    let methods = [native(
        c"nativeHandwritingGesture",
        c"(Ldev/kompozr/KompozView;II[FFZ)[I",
        native_handwriting_gesture as *mut std::ffi::c_void,
    )];
    let registered = env.find_class(c"dev/kompozr/KompozIme").is_some_and(|c| env.register_natives(&c, &methods));
    if !registered {
        crate::system::log("kompoz: KompozIme's natives couldn't be registered: no handwriting gestures");
    }
}

/// `nativeHandwritingGesture(view, session, kind, values, margin, preview): IntArray`: an input method's handwriting
/// gesture (or its preview) over the field of `view`'s app (`TextInput::handwriting_gesture`).
#[allow(clippy::too_many_arguments, unsafe_code)] // (a native's entry: the env the VM passed it)
extern "system" fn native_handwriting_gesture(
    raw: *mut JNIEnv,
    _ime: JObject,
    view: JObject,
    session: jint,
    gesture: jint,
    values: JObject,
    margin: jfloat,
    preview: jboolean,
) -> JObject {
    super::natives::guard("nativeHandwritingGesture", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { super::natives::env(raw) };
        let values = env.floats(values);
        let out = super::app::with_view(&env, view, |app| {
            app.with_text_input(|t, host| t.handwriting_gesture(host, session, gesture, &values, margin, preview != 0))
        })
        .flatten()
        .unwrap_or_default();
        env.new_ints(&out).map_or(JObject::null(), |a| a.into_object())
    })
}
