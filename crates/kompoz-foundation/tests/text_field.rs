//! The state-based text fields — BasicTextField and BasicSecureTextField (WP2b) — and the C++ Kompoz's text field
//! tests ported onto them: their Material TextField's decoration (container, label, indicator, heights) is stage
//! 12's (kompoz-material3/tests/text_field_halves.rs, by the same names), recorded where a test checked it; what
//! remains is the editing — typing, the cursor, taps, the mouse's and touch's selection, handles, the magnifier,
//! the toolbar, keys, the clipboard, scrolling, line limits, secure fields, autofill, the input method. Compose's
//! behaviour where the two differ (said where it matters).
//!
//! The texts are 16 sp in the fake backend unless said: 8 px a character, 20 px lines. Numbers in px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::{TextRange, TextStyle};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::*;
use kompoz_ui::platform::ImeAction;
use kompoz_ui::platform::{HapticFeedbackType, LocalCursorBlinkEnabled, PlatformCallbacks, PointerIcon};
use kompoz_ui::platform_text_layout::{PlatformTextLayout, TextGranularity};
use kompoz_ui::semantics::*;
use kompoz_ui::text_input::TextHighlightType;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The 16 sp style the C++ fields' text had (bodyLarge's size): 8 px a character, 20 px lines.
fn style16() -> TextStyle {
    TextStyle { font_size: 16.sp(), ..TextStyle::Default }
}

/// A platform keeping its clipboard, what it is told of text input, the cursors and haptics.
#[derive(Default)]
struct TestPlatform {
    clipboard: RefCell<Option<String>>,
    haptics: RefCell<Vec<HapticFeedbackType>>,
    cursors: RefCell<Vec<PointerIcon>>,
    keyboard_hidden: Cell<i32>,
    text_inputs: RefCell<Vec<bool>>,
    stylus_icons: RefCell<Vec<Option<PointerIcon>>>,
    handwriting: Cell<i32>,
    layouts: RefCell<Vec<PlatformTextLayout>>,
}

impl PlatformCallbacks for TestPlatform {
    fn get_clip_entry(&self) -> Option<kompoz_ui::clipboard::ClipEntry> {
        self.clipboard.borrow().clone().map(kompoz_ui::clipboard::ClipEntry::with_plain_text)
    }
    fn set_clip_entry(&self, entry: Option<kompoz_ui::clipboard::ClipEntry>) {
        *self.clipboard.borrow_mut() = entry.and_then(|e| e.read_plain_text());
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.haptics.borrow_mut().push(kind);
    }
    fn cursor(&self, icon: PointerIcon) {
        self.cursors.borrow_mut().push(icon);
    }
    fn hide_software_keyboard(&self) {
        self.keyboard_hidden.set(self.keyboard_hidden.get() + 1);
    }
    fn text_input(
        &self,
        active: bool,
        _area: Rect,
        _state: &kompoz_ui::platform::TextEditState,
        _options: &kompoz_ui::platform::ImeOptions,
    ) {
        self.text_inputs.borrow_mut().push(active);
    }
    fn stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        self.stylus_icons.borrow_mut().push(icon);
    }
    fn start_stylus_handwriting(&self) {
        self.handwriting.set(self.handwriting.get() + 1);
    }
    fn update_text_layout_result(&self, layout: &PlatformTextLayout) {
        self.layouts.borrow_mut().push(layout.clone());
    }
}

impl TestPlatform {
    /// How many input sessions started: the platform told one is active after none was (it hears it again as the
    /// field moves).
    fn sessions_started(&self) -> usize {
        let mut active = false;
        let mut started = 0;
        for a in self.text_inputs.borrow().iter() {
            if *a && !active {
                started += 1;
            }
            active = *a;
        }
        started
    }
    fn clipboard(&self) -> String {
        self.clipboard.borrow().clone().unwrap_or_default()
    }
    fn set_clip(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
}

fn platform(rule: &KompozTestRule) -> Rc<TestPlatform> {
    let p = Rc::new(TestPlatform::default());
    rule.host().set_callbacks(p.clone());
    p
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root()
}

/// The bounds of a node merged into another (what a field's decorator holds merges into the field).
fn unmerged_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).fetch_semantics_node().bounds_in_root()
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn long_press(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

/// A drag from `from` to `to` in six moves, a frame each.
fn drag(rule: &KompozTestRule, from: Offset, to: Offset, pointer_type: PointerType) {
    rule.host().pointer_down(from, -1, 0, pointer_type);
    for i in 1..=6 {
        rule.advance_frame();
        let f = i as f32 / 6.0;
        rule.host().pointer_move(at(from.x + (to.x - from.x) * f, from.y + (to.y - from.y) * f), -1, 0);
    }
    rule.host().pointer_up(to, -1, 0);
    rule.wait_for_idle();
}

fn key(rule: &KompozTestRule, key: Key, modifiers: KeyModifiers) {
    rule.host().key_down(key, modifiers);
    rule.host().key_up(key, modifiers);
    rule.wait_for_idle();
}

fn type_text(rule: &KompozTestRule, text: &str) {
    rule.host().text_input(text);
    rule.wait_for_idle();
}

/// Scrolls `state` to `value` from code (a coroutine of the host's), run until idle.
fn scroll_to(rule: &KompozTestRule, state: &ScrollState, value: i32) {
    let s = state.clone();
    kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s.scroll_to(value).await;
    });
    rule.wait_for_idle();
}

fn is_focused(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Focused) == Some(true)
}

/// A stylus pressed at `from` and moved to `to` (a frame each of `steps`), lifted there, gone.
fn stylus_stroke(rule: &KompozTestRule, from: Offset, to: Offset, steps: i32) {
    let h = rule.host();
    let none = PointerButtons::None;
    h.stylus(from, -1, 5, PointerType::Stylus, false, 0.0, none);
    h.stylus(from, -1, 5, PointerType::Stylus, true, 0.5, none);
    for i in 1..=steps {
        rule.advance_frame();
        let f = i as f32 / steps as f32;
        let p = at(from.x + (to.x - from.x) * f, from.y + (to.y - from.y) * f);
        h.stylus(p, -1, 5, PointerType::Stylus, true, 0.5, none);
    }
    h.stylus(to, -1, 5, PointerType::Stylus, false, 0.0, none);
    h.stylus_exit(5);
    rule.wait_for_idle();
}

const NONE: KeyModifiers = KeyModifiers::None;
const CTRL: KeyModifiers = KeyModifiers::Ctrl;
const SHIFT: KeyModifiers = KeyModifiers::Shift;

// ---------------------------------------------------------------------------------------------------------------
// The field itself

/// A field takes the focus from a tap, then what is typed; its text and cursor are drawn.
fn basic_text_field_types_and_draws_its_cursor() {
    let state = TextFieldState::new("");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(s.clone(), modifier = Modifier.width(200.dp()).test_tag("f"), text_style = style16());
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let b = bounds(&rule, "f");
    assert!(near(b.width(), 200.0, 0.5) && near(b.height(), 20.0, 0.5), "{b:?}");
    tap(&rule, b.center());
    assert!(is_focused(&rule, "f"));
    type_text(&rule, "merhaba");
    assert_eq!((state.text().as_str(), state.selection()), ("merhaba", TextRange::at(7)));
    let canvas = rule.draw();
    assert_eq!(canvas.texts, vec!["merhaba".to_string()]);
}

/// Stylus handwriting (Compose's StylusHandwriting, where the platform supports it: Android 14): a stylus moving
/// past the handwriting slop over a field — or near it, 40 dp above — focuses it and starts the input method's
/// handwriting for its session; a short stroke within the slop doesn't, a finger doesn't, a password field only
/// takes the focus; a hovering stylus shows the handwriting icon. Both the state-based and the value-based fields.
fn a_stylus_writing_over_a_field_starts_handwriting() {
    kompoz_ui::platform::set_stylus_handwriting_supported(true);
    let state = TextFieldState::new("");
    let (s, value) = (state.clone(), mutable_state_of(String::new()));
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, v) = (s.clone(), v.clone());
        Column(
            content = move || {
                Spacer(modifier = Modifier.height(100.dp()));
                BasicTextField(s.clone(), modifier = Modifier.width(200.dp()).test_tag("f"), text_style = style16());
                Spacer(modifier = Modifier.height(100.dp()));
                let v2 = v.clone();
                BasicTextFieldString(
                    v.get(),
                    move |it: String| v2.set(it),
                    modifier = Modifier.width(200.dp()).test_tag("g"),
                    text_style = style16(),
                );
                Spacer(modifier = Modifier.height(100.dp()));
                BasicSecureTextField(TextFieldState::new(""), modifier = Modifier.width(200.dp()).test_tag("p"));
            },
        );
    }));
    let p = platform(&rule);
    rule.wait_for_idle();
    let (f, g, secure) = (bounds(&rule, "f"), bounds(&rule, "g"), bounds(&rule, "p"));
    // A short stroke (within the slop) and a finger's drag: no handwriting.
    stylus_stroke(&rule, f.center(), at(f.center().x + 1.0, f.center().y), 2);
    drag(&rule, f.center(), at(f.center().x + 60.0, f.center().y), PointerType::Touch);
    assert_eq!(p.handwriting.get(), 0);
    // A stroke over the field (focused by the short stroke's tap): its session's handwriting started.
    stylus_stroke(&rule, at(f.left + 10.0, f.center().y), at(f.left + 80.0, f.center().y), 4);
    assert!(is_focused(&rule, "f") && p.sessions_started() >= 1);
    assert_eq!(p.handwriting.get(), 1);
    // Over the unfocused value-based field, starting 30 px above it (within its 40 dp): focused, then started.
    stylus_stroke(&rule, at(g.left + 10.0, g.top - 30.0), at(g.left + 90.0, g.top - 30.0), 4);
    assert!(is_focused(&rule, "g"));
    assert_eq!(p.handwriting.get(), 2);
    // The stroke went to the input method: nothing selected, nothing typed.
    assert_eq!((state.text().as_str(), value.get().as_str()), ("", ""));
    // A password field: focused, no handwriting.
    stylus_stroke(&rule, at(secure.left + 10.0, secure.center().y), at(secure.left + 90.0, secure.center().y), 4);
    assert!(is_focused(&rule, "p"));
    assert_eq!(p.handwriting.get(), 2);
    // A hovering stylus over a field shows the handwriting icon.
    let h = rule.host();
    h.stylus(f.center(), -1, 6, PointerType::Stylus, false, 0.0, PointerButtons::None);
    rule.wait_for_idle();
    assert_eq!(p.stylus_icons.borrow().last().copied().flatten(), Some(PointerIcon::Handwriting));
    h.stylus_exit(6);
    rule.wait_for_idle();
    kompoz_ui::platform::set_stylus_handwriting_supported(false);
}

/// C++ 21948: a BasicTextField is just its text: one line tall, nothing drawn but the text (the C++ field's
/// placeholder was its own parameter: a decorator's in Compose); a click and typing edit it.
fn basic_text_field_is_just_the_text() {
    let state = TextFieldState::new("yaz");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(
            s.clone(),
            modifier = Modifier.width(200.dp()).test_tag("f"),
            text_style = style16(),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    // C++ checked 24 dp: bodyLarge's line height (Material's, stage 12); here a line of 16 sp, 20 px.
    assert!(near(bounds(&rule, "f").height(), 20.0, 0.5));
    let canvas = rule.draw();
    assert!(canvas.rects.is_empty() && canvas.texts == vec!["yaz".to_string()]);
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    type_text(&rule, "!");
    assert_eq!(state.text(), "yaz!");
}

/// C++ 1476: Enter in a single-line field is its keyboard action. (Compose: the action is the keyboard options'
/// — with none (Default) Enter does the default action, nothing for Default; the C++'s `onSubmit` ran for it.)
fn text_field_submit() {
    let submitted = Rc::new(Cell::new(0));
    let state = TextFieldState::new("");
    let (s, n) = (state.clone(), submitted.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let n = n.clone();
        let handler: Rc<dyn KeyboardActionHandler> = Rc::new(move |_default: &dyn Fn()| n.set(n.get() + 1));
        BasicTextField(
            s.clone(),
            modifier = Modifier.test_tag("field"),
            keyboard_options = KeyboardOptions(ime_action = ImeAction::Done),
            on_keyboard_action = Some(handler),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("field").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::Enter, NONE);
    assert_eq!(submitted.get(), 1);
}

/// C++ 6296: a secure field draws only its bullets, one line tall.
fn basic_secure_text_field_is_just_the_dots() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let password = remember_text_field_state("gizli".to_string());
        BasicSecureTextField(password, modifier = Modifier.width(200.dp()).test_tag("parola"), text_style = style16());
    }));
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(canvas.texts == vec!["•••••".to_string()] && canvas.rects.is_empty(), "{:?}", canvas.texts);
    // C++ checked 24 dp (bodyLarge's line height); a line of 16 sp here.
    assert!(near(bounds(&rule, "parola").height(), 20.0, 0.5));
}

/// C++ 11046: a field focused from code doesn't show the keyboard when its options say so. (Compose: such a field
/// starts no input session on focus at all — the C++ started one with the keyboard down; a tap starts one with
/// the keyboard up; a long press selects but starts none.)
fn fields_focused_from_code_may_keep_the_keyboard_down() {
    let quiet = FocusRequester::new();
    let loud = FocusRequester::new();
    let (q, l) = (quiet.clone(), loud.clone());
    let quiet_state = TextFieldState::new("ali");
    let qs = quiet_state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (q, l, qs) = (q.clone(), l.clone(), qs.clone());
        Column(move || {
            BasicTextField(
                qs.clone(),
                modifier = Modifier.test_tag("sessiz").focus_requester(&q),
                text_style = style16(),
                keyboard_options = KeyboardOptions(show_keyboard_on_focus = Some(false)),
                line_limits = TextFieldLineLimits::SingleLine,
            );
            BasicTextField(
                remember_text_field_state("veli".to_string()),
                modifier = Modifier.test_tag("normal").focus_requester(&l),
                text_style = style16(),
                line_limits = TextFieldLineLimits::SingleLine,
            );
        });
    }));
    let p = platform(&rule);
    let shown = || p.sessions_started();
    quiet.request_focus();
    rule.wait_for_idle();
    assert!(is_focused(&rule, "sessiz") && shown() == 0);
    loud.request_focus();
    rule.wait_for_idle();
    assert_eq!(shown(), 1);
    key(&rule, Key::Tab, SHIFT);
    assert!(is_focused(&rule, "sessiz") && shown() == 1);
    // Tapped while focused: the keyboard comes up.
    tap(&rule, bounds(&rule, "sessiz").center());
    assert_eq!(shown(), 2);
    rule.host().clear_focus(true);
    rule.wait_for_idle();
    rule.main_clock().advance_time_by(300 + 100); // (past the double tap timeout: at once, a double click)
    tap(&rule, bounds(&rule, "sessiz").center());
    assert_eq!(shown(), 3);
    rule.host().clear_focus(true);
    rule.wait_for_idle();
    let b = bounds(&rule, "sessiz");
    long_press(&rule, at(b.left + 10.0, b.center().y));
    assert!(is_focused(&rule, "sessiz") && quiet_state.selection() == TextRange::new(0, 3) && shown() == 3);
}

/// C++ 11086: focus requesters, the focus manager and the input method's actions: Next moves the focus on, Done
/// hides the keyboard. (Compose: Done keeps the focus — the C++ cleared it; `free_focus` frees a captured focus
/// only, nothing here.)
fn focus_requester_focus_manager_and_ime_actions() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let first = TextFieldState::new("");
    let (f, l) = (first.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let requester = remember(FocusRequester::new);
        let r = requester.clone();
        LaunchedEffect((), async move {
            r.request_focus();
        });
        let (f, l) = (f.clone(), l.clone());
        Column(move || {
            let l = l.clone();
            BasicTextField(
                f.clone(),
                modifier = Modifier.test_tag("first").on_focus_changed(move |s: kompoz_ui::focus::FocusState| {
                    l.borrow_mut().push(format!("first {}", if s.has_focus() { "on" } else { "off" }));
                }),
                keyboard_options = KeyboardOptions(ime_action = ImeAction::Next),
                line_limits = TextFieldLineLimits::SingleLine,
            );
            BasicTextField(
                remember_text_field_state("".to_string()),
                modifier = Modifier.test_tag("second").focus_requester(&requester),
                keyboard_options = KeyboardOptions(ime_action = ImeAction::Done),
                line_limits = TextFieldLineLimits::SingleLine,
            );
        });
    }));
    let p = platform(&rule);
    rule.wait_for_idle();
    assert!(is_focused(&rule, "second"));
    assert!(rule.host().move_focus(kompoz_ui::focus::FocusDirection::Previous));
    rule.wait_for_idle();
    assert!(is_focused(&rule, "first") && log.borrow().last().map(String::as_str) == Some("first on"));
    type_text(&rule, "Ada");
    rule.host().ime_action(ImeAction::Next);
    rule.wait_for_idle();
    assert!(is_focused(&rule, "second") && first.text() == "Ada");
    assert_eq!(log.borrow().last().map(String::as_str), Some("first off"));
    key(&rule, Key::Tab, SHIFT);
    assert!(is_focused(&rule, "first"));
    key(&rule, Key::Tab, NONE);
    let hidden = p.keyboard_hidden.get();
    rule.host().ime_action(ImeAction::Done);
    rule.wait_for_idle();
    assert!(p.keyboard_hidden.get() > hidden && is_focused(&rule, "second"));
}

/// C++ 878: a swipe that starts on a field in a scroll container scrolls it without focusing the field; a tap
/// focuses it (its input session starts). (Compose: a press elsewhere keeps the focus — the C++ cleared it.)
fn swiping_over_a_text_field_does_not_focus_it() {
    let scroll = ScrollState::new(0);
    let sc = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&sc),
            content = || {
                Box(modifier = Modifier.height(100.dp()));
                BasicTextField(
                    remember_text_field_state("".to_string()),
                    modifier = Modifier.fill_max_width().test_tag("field"),
                    text_style = style16(),
                );
                Box(modifier = Modifier.height(2000.dp()));
            },
        );
    }));
    rule.host().set_viewport_size(IntSize::new(400, 800));
    let p = platform(&rule);
    rule.wait_for_idle();
    let field = bounds(&rule, "field").center();
    drag(&rule, field, at(field.x, field.y - 100.0), PointerType::Touch);
    assert!(scroll.value() > 50 && !is_focused(&rule, "field") && p.text_inputs.borrow().is_empty());
    scroll_to(&rule, &scroll, 0);
    tap(&rule, bounds(&rule, "field").center());
    assert!(is_focused(&rule, "field") && p.sessions_started() == 1);
    tap(&rule, at(200.0, 700.0));
    assert!(is_focused(&rule, "field"));
}

/// C++ 10781: a decorator puts the inner field where it wants and widens its touch target (a tap on its icon
/// types into it); line limits: MultiLine(2, 3) is two lines tall empty, three at most; SingleLine one. (C++'s
/// `minLines`/`maxLines` field is the value-based one, WP2c's; its `singleLine` beside `lineLimits` has no
/// Compose counterpart; its 280 dp and 56 dp were Material's, stage 12's: kompoz-material3
/// text_field_halves.rs.)
fn basic_text_fields_take_a_decorator_and_fields_line_limits() {
    let query = TextFieldState::new("");
    let multi = TextFieldState::new("");
    let (q, m) = (query.clone(), multi.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (q, m) = (q.clone(), m.clone());
        Column(
            modifier = Modifier.width(300.dp()),
            content = move || {
                let decorator: Rc<dyn TextFieldDecorator> = Rc::new(|inner: &dyn Fn()| {
                    Row(
                        modifier = Modifier.test_tag("süs").padding(10.dp()),
                        content = || {
                            Box(modifier = Modifier.size(20.dp()).test_tag("ikon"));
                            Box(modifier = Modifier.weight(1.0).test_tag("iç"), content = || inner());
                        },
                    );
                });
                BasicTextField(q.clone(), text_style = style16(), decorator = Some(decorator));
                BasicTextField(
                    m.clone(),
                    modifier = Modifier.test_tag("çok"),
                    text_style = style16(),
                    line_limits = TextFieldLineLimits::multi_line(2, 3),
                );
                BasicTextField(
                    remember_text_field_state("".to_string()),
                    modifier = Modifier.test_tag("tek"),
                    text_style = style16(),
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            },
        );
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let (inner, deco) = (unmerged_bounds(&rule, "iç"), unmerged_bounds(&rule, "süs"));
    assert!(
        near(inner.left - deco.left, 30.0, 0.5) && near(inner.width(), 300.0 - 20.0 - 20.0, 0.5),
        "{inner:?} {deco:?}"
    );
    let icon = unmerged_bounds(&rule, "ikon");
    tap(&rule, at(icon.left + 5.0, icon.top + 5.0));
    type_text(&rule, "kedi");
    assert_eq!(query.text(), "kedi");
    assert!(near(bounds(&rule, "çok").height(), 40.0, 0.5));
    multi.set_text_and_place_cursor_at_end("1\n2\n3\n4\n5");
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "çok").height(), 60.0, 0.5));
    assert!(near(bounds(&rule, "tek").height(), 20.0, 0.5));
}

/// C++ 20895: a multi-line field wraps its text and grows with it up to its max lines, then scrolls. (Its
/// widths and heights were Material's; here 27 characters a line, 20 px lines.)
fn multi_line_field_wraps_and_grows_up_to_max_lines() {
    let state = TextFieldState::new("");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(
            s.clone(),
            modifier = Modifier.width(216.dp()).test_tag("f"),
            text_style = style16(),
            line_limits = TextFieldLineLimits::multi_line(1, 3),
        );
    }));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), 20.0, 0.5));
    let words = "aaaaaaaa ".repeat(6); // three words a line
    state.set_text_and_place_cursor_at_end(&words);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), 40.0, 0.5));
    state.set_text_and_place_cursor_at_end(&words.repeat(2)); // four lines: three show
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), 60.0, 0.5));
    let texts = rule.drawn_texts();
    // (the fourth line drawn too, under the clip)
    assert!(texts.len() >= 3 && texts[0].trim_end() == "aaaaaaaa aaaaaaaa aaaaaaaa", "{texts:?}");
}

/// C++ 20915: in a multi-line field Up and Down keep the column, Home and End go to the line's ends, Ctrl+Home
/// and Ctrl+End to the text's, Enter starts a line, Shift+Down selects a line down.
fn multi_line_field_keys_move_between_lines() {
    let state = TextFieldState::new("abc\ndefgh\nij");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(s.clone(), modifier = Modifier.width(200.dp()).test_tag("f"), text_style = style16());
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::MoveEnd, CTRL);
    let step = |k: Key, modifiers: KeyModifiers, typed: &str| {
        key(&rule, k, modifiers);
        if !typed.is_empty() {
            type_text(&rule, typed);
        }
        state.text()
    };
    assert_eq!(step(Key::DirectionUp, NONE, "1"), "abc\nde1fgh\nij"); // the same column on the line above
    assert_eq!(step(Key::DirectionUp, NONE, "2"), "abc2\nde1fgh\nij"); // as far as that line goes
    assert_eq!(step(Key::DirectionDown, NONE, "3"), "abc2\nde1f3gh\nij");
    assert_eq!(step(Key::MoveHome, NONE, "4"), "abc2\n4de1f3gh\nij"); // the line's start...
    assert_eq!(step(Key::MoveEnd, NONE, "5"), "abc2\n4de1f3gh5\nij"); // ...and end
    assert_eq!(step(Key::Enter, NONE, ""), "abc2\n4de1f3gh5\n\nij"); // a new line
    assert_eq!(step(Key::MoveHome, CTRL, "6"), "6abc2\n4de1f3gh5\n\nij"); // the text's start
    key(&rule, Key::DirectionUp, NONE); // on the first line: to its start
    key(&rule, Key::DirectionDown, SHIFT); // a line down selected
    key(&rule, Key::Backspace, NONE);
    assert_eq!(state.text(), "4de1f3gh5\n\nij");
}

/// C++ 20965: a single-line field shows pasted lines on one line. (Compose: its state keeps the line feed, shown
/// as a space — the C++ replaced it in the value.)
fn single_line_field_joins_pasted_lines() {
    let state = TextFieldState::new("");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(
            s.clone(),
            modifier = Modifier.test_tag("f"),
            text_style = style16(),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    type_text(&rule, "bir\niki");
    assert_eq!(state.text(), "bir\niki");
    assert_eq!(rule.drawn_texts(), vec!["bir iki".to_string()]);
    let editable = rule.on_node_with_tag("f").fetch_semantics_node().config().get(SemanticsProperties::EditableText);
    assert_eq!(editable.map(|t| t.text().to_string()), Some("bir\niki".to_string()));
}

/// C++ 23414: a read-only field tells its interaction source its presses (a tap opens a picker) and its focus.
fn text_field_interaction_source_tells_presses_and_focus() {
    let source = MutableInteractionSource::new();
    let open = Rc::new(Cell::new(false));
    let (src, o) = (source.clone(), open.clone());
    let pressed = Rc::new(Cell::new(false));
    let focused = Rc::new(Cell::new(false));
    let (p2, f2) = (pressed.clone(), focused.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (src, o) = (src.clone(), o.clone());
        let collecting = src.interactions().collect(move |i| {
            if i.is::<PressInteraction::Release>() {
                o.set(true);
            }
        });
        LaunchedEffect((), collecting);
        p2.set(src.collect_is_pressed_as_state().get());
        f2.set(src.collect_is_focused_as_state().get());
        BasicTextField(
            remember_text_field_state("05.10.2026".to_string()),
            modifier = Modifier.test_tag("tarih"),
            text_style = style16(),
            read_only = true,
            interaction_source = Some(src.clone()),
        );
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let c = bounds(&rule, "tarih").center();
    rule.host().pointer_down(c, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    assert!(pressed.get() && !open.get());
    rule.host().pointer_up(c, -1, 0);
    rule.wait_for_idle();
    assert!(open.get() && focused.get() && !pressed.get());
}

/// C++ 11305: a tap places the caret between characters; a secure field shows dots and a tap between two of them
/// types there; its keyboard is a password's. (Its supporting text and error colors were Material's.)
fn tap_places_the_caret_and_password_shows_dots() {
    let value = TextFieldState::new("merhaba");
    let secret = TextFieldState::new("gizli");
    let (v, sc) = (value.clone(), secret.clone());
    let options = Rc::new(RefCell::new(Vec::<kompoz_ui::platform::ImeOptions>::new()));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, sc) = (v.clone(), sc.clone());
        Column(move || {
            BasicTextField(v.clone(), modifier = Modifier.width(200.dp()).test_tag("f"), text_style = style16());
            BasicSecureTextField(sc.clone(), modifier = Modifier.width(200.dp()).test_tag("p"), text_style = style16());
        });
    }));
    struct Ime(Rc<RefCell<Vec<kompoz_ui::platform::ImeOptions>>>);
    impl PlatformCallbacks for Ime {
        fn text_input(
            &self,
            active: bool,
            _: Rect,
            _: &kompoz_ui::platform::TextEditState,
            o: &kompoz_ui::platform::ImeOptions,
        ) {
            if active {
                self.0.borrow_mut().push(o.clone());
            }
        }
    }
    rule.host().set_callbacks(Rc::new(Ime(options.clone())));
    rule.wait_for_idle();
    let f = bounds(&rule, "f");
    tap(&rule, at(f.left + 8.0 * 3.0 + 1.0, f.center().y));
    type_text(&rule, "-");
    assert_eq!(value.text(), "mer-haba");
    let texts = rule.drawn_texts();
    assert!(texts.contains(&"•••••".to_string()) && !texts.contains(&"gizli".to_string()), "{texts:?}");
    let p = bounds(&rule, "p");
    tap(&rule, at(p.left + 8.0 * 2.0 + 1.0, p.center().y)); // between the 2nd and 3rd dot
    type_text(&rule, "X");
    assert_eq!(secret.text(), "giXzli");
    assert_eq!(options.borrow().last().map(|o| o.keyboard_type), Some(kompoz_ui::platform::KeyboardType::Password));
}

/// C++ 19590: a secure field in RevealLastTyped shows the character just typed for 1.5 s; typed in the middle,
/// that one shows; deleting, pasting more than one character or leaving the field hides it; copying and cutting
/// are refused whatever shows; Hidden never shows one, Visible shows the text (still not copied); its own mask
/// character. (Compose's System mode doesn't reveal on the desktop. The C++'s whole-state input method edits have
/// no Compose counterpart; its Material containers are stage 12's, kompoz-material3 text_field_halves.rs; its
/// onSubmit is the keyboard action here.)
fn secure_text_field_shows_the_character_just_typed_for_a_moment() {
    let secret = TextFieldState::new("");
    let other = TextFieldState::new("gizli");
    let mode = mutable_state_of(TextObfuscationMode::RevealLastTyped);
    let submitted = Rc::new(Cell::new(0));
    let (sc, ot, md, sb) = (secret.clone(), other.clone(), mode.clone(), submitted.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sc, ot, md, sb) = (sc.clone(), ot.clone(), md.clone(), sb.clone());
        KompozitionLocalProvider(&[LocalCursorBlinkEnabled.provides(false)], move || {
            let (sc, ot, md, sb) = (sc.clone(), ot.clone(), md.clone(), sb.clone());
            Column(move || {
                let sb = sb.clone();
                let handler: Rc<dyn KeyboardActionHandler> = Rc::new(move |_: &dyn Fn()| sb.set(sb.get() + 1));
                BasicSecureTextField(
                    sc.clone(),
                    modifier = Modifier.width(200.dp()).test_tag("şifre"),
                    text_style = style16(),
                    keyboard_options = KeyboardOptions::SecureTextField.copy(ime_action = ImeAction::Done),
                    on_keyboard_action = Some(handler),
                    text_obfuscation_mode = md.get(),
                );
                BasicSecureTextField(
                    ot.clone(),
                    modifier = Modifier.width(200.dp()).test_tag("diğer"),
                    text_style = style16(),
                    input_transformation = Some(InputTransformation.max_length(6)),
                    text_obfuscation_character = '*',
                );
            });
        });
    }));
    let p = platform(&rule);
    p.set_clip("önceki");
    rule.wait_for_idle();
    let shows = |text: &str| rule.drawn_texts().iter().any(|t| t == text);
    assert!(shows("*****"));
    tap(&rule, bounds(&rule, "şifre").center());
    rule.host().text_input("a");
    rule.advance_frame();
    assert!(shows("a"));
    rule.host().text_input("ç"); // the one before turns into a dot
    rule.advance_frame();
    assert!(secret.text() == "aç" && shows("•ç"));
    rule.advance_time_by(1400);
    assert!(shows("•ç"));
    rule.advance_time_by(200); // 1.5 s on
    assert!(shows("••"));
    // Typed in the middle: that one shows; a deletion hides it at once.
    rule.host().key_down(Key::DirectionLeft, NONE);
    rule.host().key_up(Key::DirectionLeft, NONE);
    rule.host().text_input("b");
    rule.advance_frame();
    assert!(secret.text() == "abç" && shows("•b•"));
    rule.host().key_down(Key::Backspace, NONE);
    rule.host().key_up(Key::Backspace, NONE);
    rule.advance_frame();
    assert!(secret.text() == "aç" && shows("••"));
    // Pasted (more than one character): nothing shows.
    rule.host().text_input("xyz");
    rule.advance_frame();
    assert!(shows("•••••"));
    // Leaving the field hides it too.
    rule.host().text_input("q");
    rule.advance_frame();
    assert!(secret.text() == "axyzqç" && shows("••••q•"));
    rule.host().key_down(Key::Tab, NONE);
    rule.host().key_up(Key::Tab, NONE);
    rule.advance_frame();
    assert!(shows("••••••"));
    rule.host().text_input("1"); // the app's own transformation has its say
    rule.host().text_input("2");
    rule.advance_frame();
    // (the other field in System mode: the desktop reveals nothing — Android's settings would)
    assert!(other.text() == "gizli1" && shows("******"));
    // Never copied, whatever shows; Enter is the action, no new line.
    rule.on_node_with_tag("şifre").perform_click();
    rule.wait_for_idle();
    for k in [Key::A, Key::C, Key::X] {
        rule.host().key_down(k, CTRL);
        rule.host().key_up(k, CTRL);
    }
    rule.wait_for_idle();
    assert!(p.clipboard() == "önceki" && secret.text().len() == 7);
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::Enter, NONE);
    assert!(submitted.get() == 1 && !secret.text().contains('\n'));
    // Hidden: dots only, even what was just typed.
    mode.set(TextObfuscationMode::Hidden);
    rule.advance_frame();
    rule.host().text_input("w");
    rule.advance_frame();
    assert!(shows("•••••••"));
    // Visible: the text itself, still not copied.
    mode.set(TextObfuscationMode::Visible);
    rule.advance_frame();
    assert!(shows(&secret.text()));
    key(&rule, Key::A, CTRL);
    key(&rule, Key::C, CTRL);
    assert_eq!(p.clipboard(), "önceki");
}

/// C++ 1452's field half (its state half is WP2a's, text_input.rs): a click on the focused field shows its
/// keyboard again (Compose's semantics click: `SoftwareKeyboardController.show()`).
fn a_click_on_the_focused_field_brings_the_keyboard_back() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        BasicTextField(
            remember_text_field_state("".to_string()),
            modifier = Modifier.test_tag("field"),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    }));
    let p = platform(&rule);
    rule.on_node_with_tag("field").perform_click();
    rule.wait_for_idle();
    assert_eq!(p.sessions_started(), 1);
    let told = p.text_inputs.borrow().len();
    rule.main_clock().advance_time_by(300 + 100); // (past the double tap timeout: at once, a double click)
    rule.on_node_with_tag("field").perform_click();
    rule.wait_for_idle();
    assert!(
        p.sessions_started() == 1
            && p.text_inputs.borrow().len() > told
            && p.text_inputs.borrow().last() == Some(&true)
    );
}

/// C++ 11418's field part: a disabled field takes no focus and no typing; a focused one disabled lets the focus
/// go and draws no cursor. (Its faded colors, label, placeholder and icons were Material's: stage 12.)
fn disabled_text_fields_take_no_focus() {
    let enabled = mutable_state_of(true);
    let value = TextFieldState::new("ali");
    let (e, v) = (enabled.clone(), value.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, v) = (e.clone(), v.clone());
        Column(move || {
            BasicTextField(
                v.clone(),
                modifier = Modifier.width(200.dp()).test_tag("ad"),
                text_style = style16(),
                enabled = e.get(),
            );
            BasicSecureTextField(
                remember_text_field_state("".to_string()),
                modifier = Modifier.width(200.dp()).test_tag("parola"),
                text_style = style16(),
                enabled = false,
            );
        });
    }));
    let p = platform(&rule);
    rule.wait_for_idle();
    tap(&rule, bounds(&rule, "parola").center());
    assert!(rule.host().focused_node().is_none() && p.sessions_started() == 0);
    assert!(rule.on_node_with_tag("parola").fetch_semantics_node().config().contains(SemanticsProperties::Disabled));
    tap(&rule, bounds(&rule, "ad").center());
    assert!(is_focused(&rule, "ad") && p.sessions_started() == 1);
    enabled.set(false);
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
    assert!(rule.draw().paths.is_empty()); // no cursor
    tap(&rule, bounds(&rule, "ad").center());
    rule.host().text_input("x");
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none() && value.text() == "ali");
}

/// C++ 11598's keyboard action part: Enter in a single-line field runs the action its options name (Search
/// here). (Its floating label and placeholder were Material's.)
fn keyboard_actions_run() {
    let searched = Rc::new(Cell::new(0));
    let sc = searched.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let sc = sc.clone();
        let handler: Rc<dyn KeyboardActionHandler> = Rc::new(move |_: &dyn Fn()| sc.set(sc.get() + 1));
        BasicTextField(
            remember_text_field_state("".to_string()),
            modifier = Modifier.test_tag("f"),
            keyboard_options = KeyboardOptions(ime_action = ImeAction::Search),
            on_keyboard_action = Some(handler),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::Enter, NONE);
    assert_eq!(searched.get(), 1);
    // The input method's own key too.
    rule.host().ime_action(ImeAction::Search);
    rule.wait_for_idle();
    assert_eq!(searched.get(), 2);
}

/// C++ 613's BasicTextField part: a field's text lines up with the texts beside it by its first baseline, a
/// multi-line one by its last. (Its Material fields are stage 12's: kompoz-material3
/// text_field_halves.rs.)
fn text_fields_line_up_by_their_baselines_too() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Row(|| {
                BasicText(
                    "Ad:",
                    modifier = Modifier.align_by_baseline(),
                    style = TextStyle { font_size: 24.sp(), ..TextStyle::Default },
                );
                BasicTextField(
                    remember_text_field_state("ali".to_string()),
                    modifier = Modifier.width(120.dp()).align_by_baseline(),
                    text_style = style16(),
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            });
            Row(|| {
                BasicText("son", modifier = Modifier.align_by(kompoz_ui::layout::LastBaseline), style = style16());
                BasicTextField(
                    remember_text_field_state("bir\niki\nüç".to_string()),
                    modifier = Modifier.width(150.dp()).align_by(kompoz_ui::layout::LastBaseline),
                    text_style = style16(),
                );
            });
        });
    }));
    rule.wait_for_idle();
    let canvas = rule.draw();
    let mut at = std::collections::HashMap::new();
    for (i, t) in canvas.texts.iter().enumerate() {
        at.insert(t.clone(), canvas.baselines[i].y);
    }
    assert!(near(at["Ad:"], at["ali"], 0.5), "{at:?}");
    assert!(near(at["son"], at["üç"], 0.5) && at["üç"] > at["bir"] + 39.0, "{at:?}");
}

/// C++ 17082's field part: a focused field sees a key between the previews going down and the handlers coming up;
/// what it takes (Backspace) goes no further up, what it doesn't (a letter's key down: its text comes through the
/// input session) does. (Compose: a key down without a command isn't the field's — the C++ swallowed it.)
fn key_events_preview_down_then_field_then_bubble_up() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let r = FocusRequester::new();
    let (l, rq) = (log.clone(), r.clone());
    let rule =
        KompozTestRule::new(kompozable_lambda!(move || {
            let handler = |name: &'static str, stop: bool, log: Rc<RefCell<Vec<String>>>| {
                move |e: &KeyEvent| {
                    let kind = if e.r#type == KeyEventType::KeyDown { "down" } else { "up" };
                    log.borrow_mut().push(format!("{name} {kind}"));
                    stop && e.key == Key::F2
                }
            };
            let (l, rq) = (l.clone(), rq.clone());
            let outer = Modifier
                .on_preview_key_event(handler("outer.preview", false, l.clone()))
                .on_key_event(handler("outer", false, l.clone()));
            let inner = Modifier.on_preview_key_event(handler("inner.preview", true, l.clone())).on_key_event(handler(
                "inner",
                false,
                l.clone(),
            ));
            Box(
                modifier = outer,
                content = move || {
                    let rq = rq.clone();
                    Box(
                        modifier = inner.clone(),
                        content = move || {
                            BasicTextField(
                                remember_text_field_state("".to_string()),
                                modifier = Modifier.focus_requester(&rq),
                            );
                        },
                    );
                },
            );
        }));
    let _p = platform(&rule);
    r.request_focus();
    rule.wait_for_idle();
    rule.host().key_down(Key::F1, NONE);
    assert_eq!(*log.borrow(), ["outer.preview down", "inner.preview down", "inner down", "outer down"]);
    log.borrow_mut().clear();
    assert!(rule.host().key_down(Key::F2, NONE)); // the inner preview takes it: the field never sees it
    assert_eq!(*log.borrow(), ["outer.preview down", "inner.preview down"]);
    log.borrow_mut().clear();
    type_text(&rule, "b");
    rule.host().key_down(Key::Backspace, NONE); // the field's
    assert_eq!(*log.borrow(), ["outer.preview down", "inner.preview down"]);
    log.borrow_mut().clear();
    rule.host().key_up(Key::F1, NONE);
    assert!(log.borrow().len() == 4 && log.borrow().last().map(String::as_str) == Some("outer up"));
}

/// C++ 17240's field part: a field that can't be focused takes the focus neither from a click nor from a
/// requester.
fn focus_properties_keep_a_field_from_the_focus() {
    let field = FocusRequester::new();
    let f = field.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(
            remember_text_field_state("".to_string()),
            modifier = Modifier
                .focus_requester(&f)
                .focus_properties(|p: &mut kompoz_ui::focus::FocusProperties| p.can_focus = false)
                .test_tag("alan"),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("alan").perform_click();
    field.request_focus();
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
}

/// C++ 24900: autofill hears of text fields — each as it comes into the layout, the focus entering and leaving
/// one (with its bounds), its text changing —, fills them (the focused one, the others, through their input
/// transformation; a read-only one refuses), and of the form sent or dropped. (Compose's AutofillManager: a
/// field's content type is the app's, else its keyboard type's — Password, Email, Phone —; Compose's manager
/// tells a field's visibility both for its input text and its fillable data appearing, and a value both ways;
/// the form is committed once its fields of a content type are gone.)
fn autofill_hears_of_text_fields_and_fills_them() {
    #[derive(Default)]
    struct Autofill {
        log: RefCell<Vec<String>>,
        focused_at: Cell<Rect>,
    }
    impl PlatformCallbacks for Autofill {
        fn autofill_focus(&self, id: u64, bounds: Rect, entered: bool) {
            self.log.borrow_mut().push(format!("{} {id}", if entered { "giriş" } else { "çıkış" }));
            if entered {
                self.focused_at.set(bounds);
            }
        }
        fn autofill_value(&self, id: u64, text: &str) {
            self.log.borrow_mut().push(format!("değer {id} {text}"));
        }
        fn autofill_visibility(&self, id: u64, visible: bool) {
            self.log.borrow_mut().push(format!("{} {id}", if visible { "görünür" } else { "gitti" }));
        }
        fn autofill_done(&self, commit: bool) {
            self.log.borrow_mut().push(if commit { "kaydet" } else { "bırak" }.to_string());
        }
    }
    let form = mutable_state_of(false);
    let locked = mutable_state_of(false);
    let user = TextFieldState::new("");
    let password = TextFieldState::new("");
    let code = TextFieldState::new("");
    let (fm, lk, us, pw, cd) = (form.clone(), locked.clone(), user.clone(), password.clone(), code.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (lk, us, pw, cd) = (lk.clone(), us.clone(), pw.clone(), cd.clone());
        if fm.get() {
            Column(move || {
                let autofill = kompoz_ui::autofill::LocalAutofillManager.current();
                BasicTextField(
                    us.clone(),
                    modifier = Modifier
                        .semantics(|c| c.set_content_type(ContentType::Username() + ContentType::EmailAddress()))
                        .width(200.dp())
                        .test_tag("kullanıcı"),
                    text_style = style16(),
                );
                // A plain keyboard: hidden text alone makes it a password.
                BasicSecureTextField(
                    pw.clone(),
                    modifier = Modifier.width(200.dp()).test_tag("parola"),
                    text_style = style16(),
                    keyboard_options = KeyboardOptions::Default,
                );
                BasicTextField(
                    remember_text_field_state("sabit".to_string()),
                    modifier = Modifier.width(200.dp()).test_tag("not"),
                    text_style = style16(),
                    keyboard_options = KeyboardOptions(keyboard_type = kompoz_ui::platform::KeyboardType::Password),
                    read_only = lk.get(),
                );
                // Its own content type wins over its keyboard's.
                BasicTextField(
                    cd.clone(),
                    modifier = Modifier
                        .semantics(|c| c.set_content_type(ContentType::SmsOtpCode()))
                        .width(200.dp())
                        .test_tag("kod"),
                    text_style = style16(),
                    keyboard_options =
                        KeyboardOptions(keyboard_type = kompoz_ui::platform::KeyboardType::NumberPassword),
                    input_transformation = Some(InputTransformation.all_caps(kompoz_text::Locale::current())),
                );
                let (commit, cancel) = (autofill.clone(), autofill.clone());
                Box(modifier = Modifier.size(40.dp()).test_tag("giriş").clickable(move || {
                    if let Some(a) = &commit {
                        a.commit();
                    }
                }));
                Box(modifier = Modifier.size(40.dp()).test_tag("vazgeç").clickable(move || {
                    if let Some(a) = &cancel {
                        a.cancel();
                    }
                }));
            });
        }
    }));
    let a = Rc::new(Autofill::default());
    rule.host().set_callbacks(a.clone());
    form.set(true); // shown once the platform listens
    rule.wait_for_idle();
    let id = |tag: &str| rule.on_node_with_tag(tag).fetch_semantics_node().id();
    let (user_id, password_id, note_id, code_id) = (id("kullanıcı"), id("parola"), id("not"), id("kod"));
    let hints = |tag: &str| {
        rule.on_node_with_tag(tag)
            .fetch_semantics_node()
            .config()
            .get(SemanticsProperties::ContentType)
            .map(|c| c.hints)
    };
    let is_password =
        |tag: &str| rule.on_node_with_tag(tag).fetch_semantics_node().config().contains(SemanticsProperties::Password);
    assert_eq!(hints("kullanıcı"), Some(vec!["username", "emailAddress"]));
    assert!(!is_password("kullanıcı"));
    assert!(hints("parola") == Some(vec!["password"]) && is_password("parola"));
    assert!(hints("not") == Some(vec!["password"]) && !is_password("not")); // its keyboard's content type
    assert_eq!(hints("kod"), Some(vec!["smsOTPCode"]));
    // Each field is told as it comes into the layout.
    for field in [user_id, password_id, note_id, code_id] {
        assert!(a.log.borrow().contains(&format!("görünür {field}")), "{:?}", a.log.borrow());
    }
    a.log.borrow_mut().clear();
    // Focused: entered, where it is; typed into: its text.
    tap(&rule, bounds(&rule, "kullanıcı").center());
    type_text(&rule, "ay");
    assert!(a.log.borrow().first() == Some(&format!("giriş {user_id}")), "{:?}", a.log.borrow());
    assert_eq!(a.log.borrow().last(), Some(&format!("değer {user_id} ay")));
    let focused_bounds = rule.host().focused_node().map(|n| n.coordinates().bounds_in_window()).unwrap();
    assert!(a.focused_at.get() == focused_bounds && a.focused_at.get().width() > 100.0);
    a.log.borrow_mut().clear();
    // Filled by the service: the focused one, then another.
    let fill = |field: u64, text: &str| {
        let filled = rule.host().perform_autofill(field, FillableData::Text(text.to_string()));
        rule.wait_for_idle();
        filled
    };
    assert!(fill(user_id, "ayse@example.com") && fill(password_id, "gizli"));
    assert!(user.text() == "ayse@example.com" && password.text() == "gizli");
    assert!(a.log.borrow().contains(&format!("değer {user_id} ayse@example.com")));
    assert!(a.log.borrow().contains(&format!("değer {password_id} gizli")));
    type_text(&rule, "!"); // typing goes on from what was filled
    assert_eq!(user.text(), "ayse@example.com!");
    assert!(fill(code_id, "ab12")); // its input transformation still has its say
    assert!(code.text() == "AB12" && code.selection() == TextRange::at(4));
    locked.set(true);
    rule.wait_for_idle();
    assert!(!fill(note_id, "x")); // read-only
    assert!(!rule.host().perform_autofill(9999, FillableData::Text("x".to_string())));
    a.log.borrow_mut().clear();
    // Sent or dropped.
    rule.on_node_with_tag("giriş").perform_click();
    rule.on_node_with_tag("vazgeç").perform_click();
    rule.wait_for_idle();
    assert!(a.log.borrow().contains(&"kaydet".to_string()) && a.log.borrow().last() == Some(&"bırak".to_string()));
    // The form goes: the focus leaves it, its fields are gone.
    tap(&rule, bounds(&rule, "kullanıcı").center());
    a.log.borrow_mut().clear();
    form.set(false);
    rule.wait_for_idle();
    let log = a.log.borrow();
    assert!(log.contains(&format!("çıkış {user_id}")), "{log:?}");
    assert!(log.contains(&format!("gitti {user_id}")) && log.contains(&format!("gitti {password_id}")), "{log:?}");
}

/// C++ 979's field part: the keyboard controller hides the keyboard with the field still focused; `show` or a
/// click on the field shows it again; Done hides it. (Its links, haptics and clipboard parts are kompoz-ui's.)
fn the_keyboard_controller_hides_and_shows_a_fields_keyboard() {
    let controller = Rc::new(RefCell::new(None::<kompoz_ui::platform::SoftwareKeyboardController>));
    let c = controller.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *c.borrow_mut() = Some(kompoz_ui::platform::LocalSoftwareKeyboardController.current());
        BasicTextField(
            remember_text_field_state("".to_string()),
            modifier = Modifier.test_tag("field"),
            keyboard_options = KeyboardOptions(ime_action = ImeAction::Done),
        );
    }));
    let p = platform(&rule);
    rule.wait_for_idle();
    let keys = controller.borrow().clone().unwrap();
    keys.hide();
    rule.on_node_with_tag("field").perform_click();
    rule.wait_for_idle();
    assert_eq!(p.sessions_started(), 1);
    let hidden = p.keyboard_hidden.get();
    keys.hide();
    assert!(p.keyboard_hidden.get() == hidden + 1 && is_focused(&rule, "field"));
    let told = p.text_inputs.borrow().len();
    keys.show();
    assert!(p.text_inputs.borrow().len() == told + 1 && p.text_inputs.borrow().last() == Some(&true));
    rule.host().ime_action(ImeAction::Done);
    rule.wait_for_idle();
    assert!(p.keyboard_hidden.get() == hidden + 2 && is_focused(&rule, "field"));
    let told = p.text_inputs.borrow().len();
    rule.main_clock().advance_time_by(300 + 100); // (past the double tap timeout: at once, a double click)
    rule.on_node_with_tag("field").perform_click();
    rule.wait_for_idle();
    assert!(p.text_inputs.borrow().len() > told && p.text_inputs.borrow().last() == Some(&true));
}

/// C++ 26856's field part: over a text field the mouse shows the text cursor.
fn a_text_field_shows_the_text_pointer_icon() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        BasicTextField(remember_text_field_state("yaz".to_string()), modifier = Modifier.width(200.dp()).test_tag("f"));
    }));
    let p = platform(&rule);
    rule.wait_for_idle();
    rule.host().pointer_move(bounds(&rule, "f").center(), 0, 0);
    rule.wait_for_idle();
    assert_eq!(p.cursors.borrow().last(), Some(&PointerIcon::Text));
}

/// C++ 27277's field part: a field's text is drawn with its style's letter spacing.
fn a_fields_text_is_letter_spaced_too() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        BasicTextField(
            remember_text_field_state("xy".to_string()),
            text_style = TextStyle { font_size: 10.sp(), letter_spacing: 2.sp(), ..TextStyle::Default },
        );
    }));
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(canvas.texts.iter().zip(&canvas.text_styles).any(|(t, s)| t == "xy" && s.letter_spacing == 2.0));
}

/// The highlights drawn in the selection's color (`within` the bounds given).
fn selection_highlights(canvas: &RecordingCanvas, within: Rect) -> Vec<Rect> {
    let color = kompoz_foundation::text::selection::TextSelectionColors::default_colors().background_color;
    canvas
        .paths
        .iter()
        .zip(&canvas.path_colors)
        .filter(|(r, c)| **c == color && r.overlaps(within))
        .map(|(r, _)| *r)
        .collect()
}

/// C++ 20976's field half (its state half is WP2a's): what is typed into a field is its state's text, read by the
/// composition; the app clearing it tells the input method. (Its OutlinedTextField's label is Material's.)
fn text_field_state_holds_the_text() {
    #[derive(Default)]
    struct Resyncs(RefCell<Vec<String>>);
    impl PlatformCallbacks for Resyncs {
        fn text_input_changed(&self, state: &kompoz_ui::platform::TextEditState) {
            self.0.borrow_mut().push(state.text.clone());
        }
    }
    let state = TextFieldState::new("merhaba");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(move || {
            BasicTextField(s.clone(), modifier = Modifier.width(200.dp()).test_tag("f"));
            BasicText(format!("[{}]", s.text()), modifier = Modifier.test_tag("echo"));
        });
    }));
    let resyncs = Rc::new(Resyncs::default());
    rule.host().set_callbacks(resyncs.clone());
    let echo = |rule: &KompozTestRule| {
        rule.on_node_with_tag("echo").fetch_semantics_node().config().get(SemanticsProperties::Text).unwrap()[0]
            .text()
            .to_string()
    };
    assert_eq!(echo(&rule), "[merhaba]");
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    type_text(&rule, " dünya");
    assert!(state.text() == "merhaba dünya" && echo(&rule) == "[merhaba dünya]");
    state.clear_text();
    rule.wait_for_idle();
    assert!(echo(&rule) == "[]" && resyncs.0.borrow().last().is_some_and(|t| t.is_empty()));
}

/// C++ 21025's drawn half (its state half is WP2a's): a focused field draws its cursor (a line the cursor brush's
/// color) at the caret, and a selection's highlight behind the text, not both. (Its highlight color was
/// Material's primary; here the selection colors' default.)
fn a_field_draws_its_cursor_and_its_selection() {
    let state = TextFieldState::with_selection("merhaba", TextRange::at(3));
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(&[LocalCursorBlinkEnabled.provides(false)], || {
            BasicTextField(
                s.clone(),
                modifier = Modifier.width(200.dp()).test_tag("f"),
                text_style = style16(),
                cursor_brush = SolidColor(Color::Red),
            );
        });
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let f = bounds(&rule, "f");
    // The cursor: a 2 px line (a butt-capped stroke: drawn as a path) in the cursor brush's color.
    let cursors = |canvas: &RecordingCanvas| -> Vec<Rect> {
        canvas.paths.iter().zip(&canvas.path_colors).filter(|(_, c)| **c == Color::Red).map(|(r, _)| *r).collect()
    };
    assert!(cursors(&rule.draw()).is_empty()); // not focused: no cursor
    assert_eq!(state.selection(), TextRange::at(3));
    // (focused by accessibility: a click would put the caret where it clicks)
    rule.on_node_with_tag("f").perform_semantics_action(SemanticsActions::RequestFocus);
    rule.wait_for_idle();
    assert_eq!(state.selection(), TextRange::at(3));
    let canvas = rule.draw();
    // At the caret, 3 characters in (x 24 to 26: the stroke's line at 25), the line's height.
    let cursor = cursors(&canvas);
    assert_eq!(cursor.len(), 1, "{cursor:?}");
    let c = cursor[0];
    assert!(near(c.left, f.left + 25.0, 0.01), "{c:?}");
    assert!(near(c.top, f.top, 0.01) && near(c.height(), 20.0, 0.01), "{c:?}");
    assert!(selection_highlights(&canvas, f).is_empty());
    state.edit(|b| b.set_selection(TextRange::new(3, 7)));
    rule.wait_for_idle();
    let canvas = rule.draw();
    let highlights = selection_highlights(&canvas, f);
    assert!(cursors(&canvas).is_empty() && highlights.len() == 1, "{highlights:?}");
    assert!(near(highlights[0].left, f.left + 24.0, 0.01) && near(highlights[0].width(), 32.0, 0.01));
    // Behind the text: the path first.
    assert!(canvas.order.find('p').is_some());
}

/// C++ 21077's composable half (the mapping half is WP2a's): an output transformation's separators, prefix and
/// suffix drawn; a tap just past a separator is before the digit after it, one on the separator where the caret
/// goes before it; a selection of all is highlighted over the prefix and the digits (the wedges' affinity).
fn output_transformation_formats_what_is_shown() {
    let phone = TextFieldState::new("5551234567");
    let amount = TextFieldState::new("250");
    let (p, a) = (phone.clone(), amount.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, a) = (p.clone(), a.clone());
        Column(move || {
            BasicTextField(
                p.clone(),
                modifier = Modifier.width(200.dp()).test_tag("tel"),
                text_style = style16(),
                line_limits = TextFieldLineLimits::SingleLine,
                output_transformation = Some(output_transformation(|b| {
                    for at in [8, 6, 3] {
                        if b.length() > at {
                            b.insert(at, " ");
                        }
                    }
                })),
            );
            BasicTextField(
                a.clone(),
                modifier = Modifier.width(200.dp()).test_tag("tutar"),
                text_style = style16(),
                line_limits = TextFieldLineLimits::SingleLine,
                output_transformation = Some(output_transformation(|b| {
                    b.insert(0, "$");
                    b.append(" USD");
                })),
            );
        });
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let drawn = |t: &str| rule.drawn_texts().iter().filter(|d| *d == t).count() == 1;
    assert!(drawn("555 123 45 67") && drawn("$250 USD"));
    let tel = bounds(&rule, "tel");
    tap(&rule, at(tel.left + 4.0 * 8.0 + 1.0, tel.center().y)); // just past a separator
    assert_eq!(phone.selection(), TextRange::at(3));
    rule.main_clock().advance_time_by(300 + 100); // (past the double tap timeout: at once, a double click)
    tap(&rule, at(tel.left + 3.0 * 8.0 + 2.0, tel.center().y)); // on the separator
    assert_eq!(phone.selection(), TextRange::at(3));
    type_text(&rule, "0");
    assert!(phone.text() == "55501234567" && drawn("555 012 34 567"), "{:?}", rule.drawn_texts());
    let tutar = bounds(&rule, "tutar");
    tap(&rule, at(tutar.right - 4.0, tutar.center().y)); // past the suffix: the caret before it
    assert_eq!(amount.selection(), TextRange::at(3));
    type_text(&rule, "0");
    key(&rule, Key::MoveHome, NONE); // after the prefix
    type_text(&rule, "1");
    assert!(amount.text() == "12500" && drawn("$12500 USD"));
    amount.edit(|b| b.select_all());
    rule.wait_for_idle();
    // (Compose: the selection's start keeps the prefix's wedge — "$" highlighted with the digits; the C++ left it
    // out — and its end stops before the suffix)
    let highlights = selection_highlights(&rule.draw(), tutar);
    assert!(highlights.len() == 1 && near(highlights[0].left, tutar.left, 0.01), "{highlights:?}");
    assert!(near(highlights[0].width(), 6.0 * 8.0, 0.01));
}

/// C++ 21258's typed half (its transformation half is WP2a's): what is typed into fields goes through their input
/// transformations — all caps in Turkish, digits only and at most five.
fn input_transformations_change_or_refuse_what_is_typed() {
    let name = TextFieldState::new("");
    let digits = TextFieldState::new("");
    let (n, d) = (name.clone(), digits.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (n, d) = (n.clone(), d.clone());
        Column(move || {
            BasicTextField(
                n.clone(),
                modifier = Modifier.width(200.dp()).test_tag("ad"),
                input_transformation = Some(InputTransformation.all_caps(kompoz_text::Locale::new("tr"))),
            );
            // (Kotlin's SAM constructor `InputTransformation { … }`)
            let only_digits = input_transformation(|b: &mut TextFieldBuffer| {
                if !b.as_char_sequence().chars().all(|c| c.is_ascii_digit()) {
                    b.revert_all_changes();
                }
            });
            BasicTextField(
                d.clone(),
                modifier = Modifier.width(200.dp()).test_tag("sayı"),
                input_transformation = Some(only_digits.then(InputTransformation.max_length(5))),
            );
        });
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("ad").perform_click();
    rule.wait_for_idle();
    type_text(&rule, "istanbul");
    assert_eq!(name.text(), "İSTANBUL");
    rule.on_node_with_tag("sayı").perform_click();
    rule.wait_for_idle();
    for t in ["12", "a", "345", "6"] {
        type_text(&rule, t);
    }
    assert_eq!(digits.text(), "12345");
}

/// C++ 760's field part: editing goes by clusters — Backspace takes a whole family emoji, the arrows step over a
/// flag. (Offsets in bytes.)
fn a_field_edits_by_grapheme_clusters() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let turkey = "\u{1F1F9}\u{1F1F7}";
    let state = TextFieldState::new(&format!("ab{family}"));
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BasicTextField(s.clone(), modifier = Modifier.width(200.dp()).test_tag("field"));
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("field").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::Backspace, NONE);
    assert_eq!(state.text(), "ab");
    state.set_text_and_place_cursor_at_end(&format!("x{turkey}y"));
    rule.wait_for_idle();
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    assert_eq!(state.selection(), TextRange::at(1));
}

/// C++ 24839's field part: a field marked sensitive makes the window's content sensitive while it is laid out.
fn a_sensitive_field_marks_the_content_sensitive() {
    let card = mutable_state_of(false);
    let c = card.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if c.get() {
            BasicTextField(remember_text_field_state("4111".to_string()), modifier = Modifier.sensitive_content(true));
        }
    }));
    rule.wait_for_idle();
    assert!(!rule.host().sensitive_content());
    card.set(true);
    rule.wait_for_idle();
    assert!(rule.host().sensitive_content());
    card.set(false);
    rule.wait_for_idle();
    assert!(!rule.host().sensitive_content());
}

/// Compose's TextFieldCoreModifierNode is a layout node that draws: it places the text it wraps at minus its scroll
/// and draws the text, the cursor and the selection in that text's space, so they move with the scroll (a layout
/// node draws in the space of what it wraps). A single line field too narrow for its text, the cursor at its end:
/// scrolled, the text is drawn that far to the left, its end in view.
fn a_scrolled_text_field_draws_its_text_where_it_scrolled() {
    let state = TextFieldState::new("");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(
            modifier = Modifier.padding(20.dp()),
            content = {
                let s = s.clone();
                move || {
                    BasicTextField(
                        s.clone(),
                        modifier = Modifier.width(60.dp()).test_tag("dar"),
                        text_style = style16(),
                        line_limits = TextFieldLineLimits::SingleLine,
                    );
                }
            },
        );
    }));
    let _p = platform(&rule);
    rule.on_node_with_tag("dar").perform_click();
    rule.wait_for_idle();
    state.set_text_and_place_cursor_at_end("ab");
    rule.wait_for_idle();
    let field = bounds(&rule, "dar");
    let c = rule.draw();
    let i = c.texts.iter().position(|t| t == "ab").expect("the text drawn");
    assert!(near(c.baselines[i].x, field.left, 0.5), "{:?} {field:?}", c.baselines);
    state.set_text_and_place_cursor_at_end("abcdefghijklmnopqrstuvwxyz");
    rule.wait_for_idle();
    let c = rule.draw();
    let i = c.texts.iter().position(|t| t == "abcdefghijklmnopqrstuvwxyz").expect("the text drawn");
    let scrolled = field.left - c.baselines[i].x;
    assert!(scrolled > 100.0, "{:?} {field:?}", c.baselines);
    // (its end, where the cursor is, in view: the text's width less the scroll is about the field's)
    let width = c.text_draws[i].size.width;
    assert!(near(width - scrolled, field.width(), 3.0), "{width} {scrolled} {field:?}");
}

/// What the platform was last told of the focused field's text layout.
fn last_layout(p: &TestPlatform) -> PlatformTextLayout {
    p.layouts.borrow().last().cloned().expect("a text layout told")
}

/// A field 20 px from the window's corner, its text 16 sp (8 px a character, 20 px lines), focused.
fn layout_rule(state: &TextFieldState, width: f32, single_line: bool) -> (KompozTestRule, Rc<TestPlatform>) {
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(
            modifier = Modifier.padding(20.dp()),
            content = {
                let s = s.clone();
                move || {
                    BasicTextField(
                        s.clone(),
                        modifier = Modifier.width(width.dp()).test_tag("f"),
                        text_style = style16(),
                        line_limits = if single_line {
                            TextFieldLineLimits::SingleLine
                        } else {
                            TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 10 }
                        },
                    );
                }
            },
        );
    }));
    let p = platform(&rule);
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    (rule, p)
}

/// The focused field's text layout reaches the platform (Compose's `updateTextLayoutResult`): a box per character
/// and per line in the window, the caret's, hit tests; told again as it changes, only while its session is open.
fn the_focused_fields_text_layout_reaches_the_platform() {
    let state = TextFieldState::new("abc");
    let (rule, p) = layout_rule(&state, 200.0, true);
    let layout = last_layout(&p);
    let field = bounds(&rule, "f");
    assert_eq!(layout.text, "abc");
    assert_eq!(layout.characters.len(), 3);
    for (i, c) in layout.characters.iter().enumerate() {
        let left = field.left + 8.0 * i as f32;
        assert!(near(c.rect.left, left, 0.5) && near(c.rect.width(), 8.0, 0.5), "{i}: {c:?} {field:?}");
        assert!(near(c.rect.top, field.top, 0.5) && near(c.rect.height(), 20.0, 0.5), "{c:?}");
        assert!(!c.rtl && c.line == 0 && c.start == i && c.end == i + 1);
    }
    assert_eq!(layout.lines.len(), 1);
    let line = layout.lines[0].rect;
    assert!(near(line.left, field.left, 0.5) && near(line.right, field.left + 24.0, 0.5), "{line:?}");
    assert!(near(layout.cursor_rect(3).left, field.left + 24.0, 0.5));
    assert!(near(layout.cursor_rect(1).left, field.left + 8.0, 0.5));
    assert!(near(layout.inner_text_field_bounds.left, field.left, 0.5), "{:?}", layout.inner_text_field_bounds);
    assert_eq!(layout.decoration_box_bounds, field);
    // Hit tests: a point over "b" is "b"; its right half puts the caret after it.
    let y = field.top + 10.0;
    assert_eq!(layout.character_at(at(field.left + 10.0, y)), Some(1));
    assert_eq!(layout.character_at(at(field.left + 30.0, y)), None);
    assert_eq!(layout.offset_for_position(at(field.left + 14.0, y)), 2);
    assert_eq!(layout.offset_for_position(at(field.left + 9.0, y)), 1);
    assert_eq!(layout.offset_for_position(at(field.left + 100.0, y)), 3);
    assert_eq!(layout.range_bounds(0, 2), Rect::new(field.left, field.top, field.left + 16.0, field.top + 20.0));
    assert_eq!(rule.host().text_input_service().session_text_layout(), Some(layout.clone()));
    // Typing lays it out again: told again.
    type_text(&rule, "d");
    assert_eq!(last_layout(&p).text, "abcd");
    // The same layout isn't told twice.
    let told = p.layouts.borrow().len();
    rule.advance_frame();
    rule.wait_for_idle();
    assert_eq!(p.layouts.borrow().len(), told);
    // Accessibility reads the focused field's layout as its own.
    let tree = rule.host().accessibility_tree();
    let node = tree.iter().find(|n| n.editable).expect("the field");
    let own = node.text_layout.clone().expect("the field's text layout");
    assert_eq!(own.characters.len(), 4);
    assert_eq!(own.characters[0].rect, last_layout(&p).characters[0].rect);
    // Its session closed, none is offered.
    rule.host().clear_focus(true);
    rule.wait_for_idle();
    assert_eq!(rule.host().text_input_service().session_text_layout(), None);
}

/// A right-to-left text's characters go from the right (each a right-to-left run's, the caret at its right edge).
fn a_right_to_left_fields_characters_go_from_the_right() {
    let state = TextFieldState::new("مرحبا");
    let (rule, p) = layout_rule(&state, 200.0, true);
    let layout = last_layout(&p);
    let field = bounds(&rule, "f");
    assert_eq!(layout.characters.len(), 5);
    assert!(layout.characters.iter().all(|c| c.rtl), "{:?}", layout.characters);
    // (Its paragraph goes as the layout does, left to right: Compose's unspecified text direction is the layout's.)
    assert!(!layout.lines[0].rtl, "{:?}", layout.lines);
    for pair in layout.characters.windows(2) {
        assert!(pair[0].rect.left > pair[1].rect.left, "{:?}", layout.characters);
        assert!(near(pair[0].rect.left, pair[1].rect.right, 0.5));
    }
    // (The run starts at its right: its first character at the line's right end.)
    let line = layout.lines[0].rect;
    assert!(near(line.left, field.left, 0.5) && near(line.right, field.left + 40.0, 0.5), "{line:?} {field:?}");
    assert!(near(layout.characters[0].rect.right, line.right, 0.5), "{:?} {line:?}", layout.characters[0]);
    assert!(near(layout.cursor_rect(0).left, layout.characters[0].rect.right, 0.5));
    let first = layout.characters[0].rect;
    let y = first.center().y;
    assert_eq!(layout.character_at(first.center()), Some(0));
    // Its leading half (the right) leaves the caret before it.
    assert_eq!(layout.offset_for_position(at(first.right - 1.0, y)), 0);
    assert_eq!(layout.offset_for_position(at(first.left + 1.0, y)), layout.characters[0].end);
}

/// A field's soft-wrapped lines: a box per line, the characters on each, a range's rectangles one per line.
fn a_wrapped_fields_lines_have_their_own_boxes() {
    // (40 px: five characters a line.)
    let state = TextFieldState::new("abcd efgh ij");
    let (rule, p) = layout_rule(&state, 40.0, false);
    let layout = last_layout(&p);
    let field = bounds(&rule, "f");
    assert_eq!(layout.lines.len(), 3, "{:?}", layout.lines);
    assert_eq!((layout.lines[0].start, layout.lines[1].start, layout.lines[2].start), (0, 5, 10));
    for (i, line) in layout.lines.iter().enumerate() {
        assert!(near(line.rect.top, field.top + 20.0 * i as f32, 0.5), "{line:?}");
    }
    let e = layout.characters[5];
    assert_eq!(e.line, 1);
    assert!(near(e.rect.left, field.left, 0.5) && near(e.rect.top, field.top + 20.0, 0.5), "{e:?}");
    let rects = layout.range_rects(2, 12);
    assert_eq!(rects.len(), 3, "{rects:?}");
    assert!(near(rects[0].left, field.left + 16.0, 0.5) && near(rects[2].top, field.top + 40.0, 0.5));
    let (first, until) = layout.first_rect(2, 12);
    assert_eq!((first, until), (rects[0], 5));
    assert_eq!(layout.line_for_offset(7), 1);
    assert_eq!(layout.offset_for_position(at(field.left + 1.0, field.top + 45.0)), 10);
    // A rectangle over "efgh" and "ij" takes them; by words, whole ones.
    let over = Rect::new(field.left, field.top + 25.0, field.left + 20.0, field.top + 55.0);
    assert_eq!(layout.range_for_rect(over, TextGranularity::Character), Some(TextRange::new(5, 12)));
    let over = Rect::new(field.left + 10.0, field.top + 5.0, field.left + 20.0, field.top + 15.0);
    assert_eq!(layout.range_for_rect(over, TextGranularity::Word), Some(TextRange::new(0, 4)));
}

/// A scrolled field's characters are where they are drawn (its scroll in the transform), what shows of it its
/// inner bounds; told again as it scrolls.
fn a_scrolled_fields_layout_moves_with_its_text() {
    let state = TextFieldState::new("");
    let (rule, p) = layout_rule(&state, 60.0, true);
    state.set_text_and_place_cursor_at_end("abcdefghijklmnopqrstuvwxyz");
    rule.wait_for_idle();
    let field = bounds(&rule, "f");
    let layout = last_layout(&p);
    let c = rule.draw();
    let i = c.texts.iter().position(|t| t == "abcdefghijklmnopqrstuvwxyz").expect("the text drawn");
    let first = layout.characters[0].rect;
    assert!(near(first.left, c.baselines[i].x, 0.5), "{first:?} {:?}", c.baselines);
    assert!(first.left < field.left - 100.0);
    // (The caret at the end, in view.)
    assert!(near(layout.end_cursor.left, field.right, 3.0), "{:?} {field:?}", layout.end_cursor);
    let inner = layout.inner_text_field_bounds;
    assert!(near(inner.left, field.left, 0.5) && near(inner.right, field.right, 0.5), "{inner:?} {field:?}");
    let z = layout.characters[25].rect;
    assert_eq!(layout.character_at(z.center()), Some(25));
    // Back to its start: the layout told again, where it shows now.
    key(&rule, Key::MoveHome, NONE);
    let layout = last_layout(&p);
    assert!(near(layout.characters[0].rect.left, field.left, 0.5), "{:?}", layout.characters[0]);
}

/// An input method's handwriting preview highlights a range of the field's text, an empty range taking it away.
fn an_input_methods_preview_highlights_the_fields_text() {
    let state = TextFieldState::new("abcdef");
    let (rule, _p) = layout_rule(&state, 200.0, true);
    let service = rule.host().text_input_service();
    assert!(service.highlight_chars_in(TextHighlightType::HandwritingDeletePreview, TextRange::new(1, 3)));
    rule.wait_for_idle();
    let highlight = state.value().highlight();
    assert_eq!(highlight, Some((TextHighlightType::HandwritingDeletePreview, TextRange::new(1, 3))));
    assert!(service.highlight_chars_in(TextHighlightType::HandwritingSelectPreview, TextRange::new(0, 0)));
    rule.wait_for_idle();
    assert_eq!(state.value().highlight(), None);
}

/// A text's layout comes with the accessibility tree (Compose's `GetTextLayoutResult`): its characters' boxes in
/// the window, cut to its bounds.
fn a_texts_layout_comes_with_its_accessibility_node() {
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(
            modifier = Modifier.padding(30.dp()),
            content = || {
                BasicText("merhaba", modifier = Modifier.test_tag("t"), style = style16());
            },
        );
    }));
    rule.wait_for_idle();
    let text = bounds(&rule, "t");
    let tree = rule.host().accessibility_tree();
    let node = tree.iter().find(|n| n.test_tag.as_deref() == Some("t")).expect("the text");
    let layout = node.text_layout.clone().expect("its layout");
    assert_eq!(layout.text, "merhaba");
    assert_eq!(layout.characters.len(), 7);
    assert!(near(layout.characters[2].rect.left, text.left + 16.0, 0.5), "{:?} {text:?}", layout.characters[2]);
    assert!(near(layout.characters[2].rect.top, text.top, 0.5));
    assert_eq!(layout.inner_text_field_bounds, node.bounds);
    assert_eq!(layout.character_at(layout.characters[4].rect.center()), Some(4));
}

tests!(
    default_key_mapping;
    a_texts_layout_comes_with_its_accessibility_node,
    the_focused_fields_text_layout_reaches_the_platform,
    a_right_to_left_fields_characters_go_from_the_right,
    a_wrapped_fields_lines_have_their_own_boxes,
    a_scrolled_fields_layout_moves_with_its_text,
    an_input_methods_preview_highlights_the_fields_text,
    a_stylus_writing_over_a_field_starts_handwriting,
    a_scrolled_text_field_draws_its_text_where_it_scrolled,
    basic_text_field_types_and_draws_its_cursor,
    basic_text_field_is_just_the_text,
    text_field_submit,
    basic_secure_text_field_is_just_the_dots,
    fields_focused_from_code_may_keep_the_keyboard_down,
    focus_requester_focus_manager_and_ime_actions,
    swiping_over_a_text_field_does_not_focus_it,
    basic_text_fields_take_a_decorator_and_fields_line_limits,
    multi_line_field_wraps_and_grows_up_to_max_lines,
    multi_line_field_keys_move_between_lines,
    single_line_field_joins_pasted_lines,
    text_field_interaction_source_tells_presses_and_focus,
    tap_places_the_caret_and_password_shows_dots,
    secure_text_field_shows_the_character_just_typed_for_a_moment,
    a_click_on_the_focused_field_brings_the_keyboard_back,
    disabled_text_fields_take_no_focus,
    keyboard_actions_run,
    text_fields_line_up_by_their_baselines_too,
    key_events_preview_down_then_field_then_bubble_up,
    focus_properties_keep_a_field_from_the_focus,
    autofill_hears_of_text_fields_and_fills_them,
    the_keyboard_controller_hides_and_shows_a_fields_keyboard,
    a_text_field_shows_the_text_pointer_icon,
    a_fields_text_is_letter_spaced_too,
    text_field_state_holds_the_text,
    a_field_draws_its_cursor_and_its_selection,
    output_transformation_formats_what_is_shown,
    input_transformations_change_or_refuse_what_is_typed,
    a_field_edits_by_grapheme_clusters,
    a_sensitive_field_marks_the_content_sensitive,
);
