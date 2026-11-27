//! The Material halves of the C++ Kompoz's text field tests (kompoz_tests.cpp) whose editing halves are
//! foundation's (kompoz-foundation/tests/{text_field, text_field_selection, text_input, text}.rs, by the same
//! names): the same tests on the real Material fields in a MaterialTheme — the decoration's geometry (the text 16 dp
//! in, 56 dp tall, 280 dp wide by default), the label and placeholder shown or not, the supporting text, the
//! containers and outlines, line limits, baselines, the theme's selection colors (its primary, 40 % behind the
//! text), and taps, selections, handles and the magnifier placed from the field's inset.
//!
//! Where the C++ wrote `TextField(value, onValueChange)` it is Compose's String field (its legacy engine, as
//! Compose's), `TextField(state)` the state-based one. The fake text backend at the theme's body large (16 sp, 0.5 sp
//! letter spacing, 24 sp lines): a character 8 px plus the letter spacing rounded to whole px as still text is
//! spaced (Android's), 9 px — the C++ fake measurer's 8.5 kept the fraction —; lines 24 px, the first baseline
//! 14.8 px down a line. A single-line field's line from 16 to 40 px down (its middle 28, as the C++'s; its bottom
//! 40, the C++'s 36 was its 20 px line's). Density 1: dp are px.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::magnifier::MagnifierPositionInRoot;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::Handle;
use kompoz_foundation::text::selection::{SelectionHandleInfo, SelectionHandleInfoKey};
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_text::TextRange;
use kompoz_text::input::PasswordVisualTransformation;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::layout::LastBaseline;
use kompoz_ui::platform::{
    HapticFeedbackType, ImeAction, ImeOptions, KeyboardType, LocalCursorBlinkEnabled, LocalTextToolbar,
    PlatformCallbacks, TextEditState, TextToolbar, TextToolbarAction, TextToolbarStatus,
};
use kompoz_ui::semantics::SemanticsProperties;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

tests!(
    default_key_mapping;
    text_fields_line_up_by_their_baselines_too,
    basic_text_fields_take_a_decorator_and_fields_line_limits,
    tap_places_the_caret_and_password_shows_dots,
    text_field_label_floats_and_keyboard_actions_run,
    refocused_field_shows_no_stale_selection_and_password_is_not_copied,
    secure_text_field_shows_the_character_just_typed_for_a_moment,
    multi_line_field_wraps_and_grows_up_to_max_lines,
    text_field_state_keeps_its_selection,
    output_transformation_formats_what_is_shown,
    password_field_maps_its_selection_to_the_dots,
    long_press_selects_a_word_with_handles_and_a_toolbar,
    caret_handle_for_fingers_double_click_for_the_mouse,
    a_double_click_takes_letters_digits_underscores_and_what_is_beyond_ascii,
    the_mouse_selects_text_by_dragging_clicking_and_shift,
    dragging_a_handle_shows_the_text_magnifier,
    selection_handles_point_at_their_end_and_hold_its_line_by_the_middle,
    tapping_a_multi_line_field_puts_the_caret_on_that_line,
    text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one,
);

// ---- Helpers --------------------------------------------------------------------------------------------------

/// A character of the body large text: 8 px and the letter spacing's px.
const CHAR: f32 = 9.0;
/// Body large's line height.
const LINE: f32 = 24.0;
/// The fields' padding: the text this far in from the start, and below the top (without a label).
const INSET: f32 = 16.0;

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn text_slot(s: &'static str) -> Option<TextFieldSlot> {
    Some(Rc::new(kompozable_lambda!(move || {
        Text(s);
    })))
}

fn label_slot(s: &'static str) -> Option<TextFieldLabel> {
    Some(Rc::new(move |_: &dyn TextFieldLabelScope| {
        kompozable_block!({
            Text(s);
        })
    }))
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// Where a character of a tagged Material field is: `index` characters past the 16 dp inset, 2 px into it, on line
/// `line`'s middle (the first line 16 dp down).
fn char_at(rule: &KompozTestRule, tag: &str, index: usize, line: usize) -> Offset {
    let r = bounds(rule, tag);
    at(r.left + INSET + CHAR * index as f32 + 2.0, r.top + INSET + LINE / 2.0 + LINE * line as f32)
}

/// A field's selection, as it tells accessibility (UTF-8 bytes).
fn selection(rule: &KompozTestRule, tag: &str) -> TextRange {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::TextSelectionRange).unwrap()
}

fn is_focused(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Focused) == Some(true)
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

/// Mouse clicks at `p`, `n` in a row, after a pause that ends a previous series.
fn clicks(rule: &KompozTestRule, p: Offset, n: usize) {
    rule.advance_time_by(500);
    for _ in 0..n {
        rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
        rule.host().pointer_up(p, -1, 0);
    }
    rule.wait_for_idle();
}

/// A finger's drag from `from`, `steps` moves of `step`, a frame each, held (`release`: lifted after).
fn finger_drag(rule: &KompozTestRule, from: Offset, step: Offset, steps: usize, release: bool) {
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=steps {
        rule.advance_frame();
        rule.host().pointer_move(from + step * i as f32, -1, 0);
    }
    if release {
        rule.host().pointer_up(from + step * steps as f32, -1, 0);
    }
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

/// Lets the field's animations end.
fn settle(rule: &KompozTestRule) {
    rule.advance_time_by(600);
    rule.wait_for_idle();
}

const NONE: KeyModifiers = KeyModifiers::None;
const CTRL: KeyModifiers = KeyModifiers::Ctrl;
const SHIFT: KeyModifiers = KeyModifiers::Shift;

/// The theme's selection highlight: its primary at 40 % (Compose's text selection colors under MaterialTheme).
fn highlight_color() -> Color {
    named_args!(light_color_scheme().primary.copy(alpha = 0.4))
}

/// The selection highlights drawn within `within` (paths in the theme's selection background).
fn highlights(canvas: &RecordingCanvas, within: Rect) -> Vec<Rect> {
    let background = highlight_color();
    (0..canvas.paths.len())
        .filter(|&i| canvas.path_colors[i] == background && within.contains(canvas.paths[i].center()))
        .map(|i| canvas.paths[i])
        .collect()
}

/// Where each text was drawn: its baseline's y (the last drawing of a text if it is drawn twice).
fn baselines(canvas: &RecordingCanvas) -> HashMap<String, f32> {
    canvas.texts.iter().zip(&canvas.baselines).map(|(t, b)| (t.clone(), b.y)).collect()
}

/// The handles shown (their SelectionHandleInfo semantics: positions in the inner text field).
fn handles(rule: &KompozTestRule) -> Vec<SelectionHandleInfo> {
    rule.all_nodes(true).iter().filter_map(|n| n.config().get(SelectionHandleInfoKey)).collect()
}

fn handle(rule: &KompozTestRule, which: Handle) -> Option<SelectionHandleInfo> {
    handles(rule).into_iter().find(|h| h.handle == which)
}

/// Where the magnifier shows the content of, in the root (none while none shows).
fn magnifier_at(rule: &KompozTestRule) -> Option<Offset> {
    rule.all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(MagnifierPositionInRoot))
        .map(|p| (p.0)())
        .find(|o| o.is_specified())
}

/// A platform keeping its clipboard, its haptics, and the input methods shown (their options).
#[derive(Default)]
struct TestPlatform {
    clipboard: RefCell<Option<String>>,
    haptics: RefCell<Vec<HapticFeedbackType>>,
    shown: RefCell<Vec<ImeOptions>>,
    told: RefCell<Vec<bool>>,
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
    fn text_input(&self, active: bool, _: Rect, _: &TextEditState, options: &ImeOptions) {
        self.told.borrow_mut().push(active);
        if active {
            self.shown.borrow_mut().push(options.clone());
        }
    }
}

impl TestPlatform {
    fn clipboard(&self) -> String {
        self.clipboard.borrow().clone().unwrap_or_default()
    }
    fn set_clip(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
    fn felt(&self, kind: HapticFeedbackType) -> usize {
        self.haptics.borrow().iter().filter(|&&k| k == kind).count()
    }
    /// How many input sessions started (told active after inactive).
    fn sessions_started(&self) -> usize {
        let mut active = false;
        let mut started = 0;
        for &a in self.told.borrow().iter() {
            if a && !active {
                started += 1;
            }
            active = a;
        }
        started
    }
    fn last_shown(&self) -> Option<ImeOptions> {
        self.shown.borrow().last().cloned()
    }
}

fn platform(rule: &KompozTestRule) -> Rc<TestPlatform> {
    let p = Rc::new(TestPlatform::default());
    rule.host().set_callbacks(p.clone());
    p
}

/// A text toolbar of the test's own: what it was shown with, how often shown and hidden.
#[derive(Default)]
struct RecordingToolbar {
    shows: Cell<usize>,
    hides: Cell<usize>,
    rect: Cell<Rect>,
    status: Cell<Option<TextToolbarStatus>>,
    copy: RefCell<TextToolbarAction>,
    paste: RefCell<TextToolbarAction>,
    cut: RefCell<TextToolbarAction>,
    select_all: RefCell<TextToolbarAction>,
}

impl TextToolbar for RecordingToolbar {
    fn show_menu(
        &self,
        rect: Rect,
        on_copy_requested: TextToolbarAction,
        on_paste_requested: TextToolbarAction,
        on_cut_requested: TextToolbarAction,
        on_select_all_requested: TextToolbarAction,
        _on_autofill_requested: TextToolbarAction,
    ) {
        self.shows.set(self.shows.get() + 1);
        self.rect.set(rect);
        *self.copy.borrow_mut() = on_copy_requested;
        *self.paste.borrow_mut() = on_paste_requested;
        *self.cut.borrow_mut() = on_cut_requested;
        *self.select_all.borrow_mut() = on_select_all_requested;
        self.status.set(Some(TextToolbarStatus::Shown));
    }
    fn hide(&self) {
        self.hides.set(self.hides.get() + 1);
        self.status.set(Some(TextToolbarStatus::Hidden));
    }
    fn status(&self) -> TextToolbarStatus {
        self.status.get().unwrap_or(TextToolbarStatus::Hidden)
    }
}

impl RecordingToolbar {
    fn shown(&self) -> bool {
        self.status() == TextToolbarStatus::Shown
    }
    /// Which items it offers while shown: copy, paste, cut, select all.
    fn offers(&self) -> (bool, bool, bool, bool) {
        let shown = self.shown();
        (
            shown && self.copy.borrow().is_some(),
            shown && self.paste.borrow().is_some(),
            shown && self.cut.borrow().is_some(),
            shown && self.select_all.borrow().is_some(),
        )
    }
    fn choose(&self, rule: &KompozTestRule, item: &RefCell<TextToolbarAction>) {
        let action = item.borrow().clone().expect("the toolbar offers it");
        action();
        rule.wait_for_idle();
    }
}

/// `content` in a MaterialTheme with `toolbar` as its LocalTextToolbar, the cursor not blinking.
#[kompozable(inline)]
fn Steady(toolbar: Rc<RecordingToolbar>, content: impl Fn() + Clone + 'static) {
    let t: Rc<dyn TextToolbar> = toolbar;
    KompozitionLocalProvider(&[LocalTextToolbar.provides_unequal(t), LocalCursorBlinkEnabled.provides(false)], || {
        MaterialTheme(content = content);
    });
}

/// A single-line Material String field of `value` filling the width under a 200 px box, a 300 px box under it
/// (tags "field" and "below"), as the C++'s.
fn field_under_a_box(
    value: MutableState<String>,
    toolbar: Rc<RecordingToolbar>,
    read_only: MutableState<bool>,
) -> KompozTestRule {
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (v, r) = (value.clone(), read_only.clone());
            Steady(toolbar.clone(), move || {
                let (v, r) = (v.clone(), r.clone());
                Column(move || {
                    Box(modifier = Modifier.height(200.dp()));
                    let v2 = v.clone();
                    StringTextField(
                        v.get(),
                        move |s| v2.set(s),
                        modifier = Modifier.fill_max_width().test_tag("field"),
                        read_only = r.get(),
                        single_line = true,
                    );
                    Box(modifier = Modifier.fill_max_width().height(300.dp()).test_tag("below"));
                });
            });
        }),
        IntSize::new(400, 800),
    )
}

// ---- The C++ tests' Material halves -------------------------------------------------------------------------------

/// C++ `text_fields_line_up_by_their_baselines_too` (C++ 613), its Material fields: in a Row by their baselines a
/// Text, an OutlinedTextField with a label and a TextField with one, 120 dp wide; a multi-line TextField by its last
/// baseline beside a Text and a BasicTextField. (Its foundation half: kompoz-foundation text_field.rs::
/// text_fields_line_up_by_their_baselines_too, text.rs::texts_line_up_by_their_last_baselines.)
///
/// Compose differs from the C++: a field's baselines are its children's, merged (the first the topmost — Compose's
/// FirstBaseline merger is `min` —, the last the bottommost), and a label is a Text too: a field with its label
/// minimized (it has text) lines up by its label's baseline, where the C++ took the text's. Without a label its
/// text's baseline is the field's: those line up with the Text as the C++'s did.
fn text_fields_line_up_by_their_baselines_too() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            MaterialTheme(
                content = || {
                    Column(|| {
                        Row(|| {
                            Text("Ad:", modifier = Modifier.align_by_baseline());
                            StringOutlinedTextField(
                                "ali",
                                |_| {},
                                modifier = Modifier.width(120.dp()).align_by_baseline(),
                                single_line = true,
                                label = text_slot("Ad"),
                            );
                            StringTextField(
                                "veli",
                                |_| {},
                                modifier = Modifier.width(120.dp()).align_by_baseline(),
                                single_line = true,
                                label = text_slot("Soyad"),
                            );
                        });
                        // (without labels)
                        Row(|| {
                            Text("Yaş:", modifier = Modifier.align_by_baseline());
                            StringOutlinedTextField(
                                "kırk",
                                |_| {},
                                modifier = Modifier.width(120.dp()).align_by_baseline(),
                                single_line = true,
                            );
                            StringTextField(
                                "elli",
                                |_| {},
                                modifier = Modifier.width(120.dp()).align_by_baseline(),
                                single_line = true,
                            );
                        });
                        Row(|| {
                            Text("son", modifier = Modifier.align_by(LastBaseline));
                            StringTextField(
                                "bir\niki\nüç",
                                |_| {},
                                modifier = Modifier.width(150.dp()).align_by(LastBaseline),
                            );
                            BasicTextFieldString(
                                "düz",
                                |_: String| {},
                                modifier = Modifier.width(60.dp()).align_by_baseline(),
                                single_line = true,
                            );
                        });
                    });
                },
            );
        }),
        IntSize::new(600, 800),
    );
    rule.wait_for_idle();
    let at = baselines(&rule.draw());
    // With labels: by the labels' baselines (Compose's merged first baselines).
    assert!(near(at["Ad:"], at["Ad"], 0.5) && near(at["Ad"], at["Soyad"], 0.5), "{at:?}");
    assert!(at["ali"] > at["Ad"] + 10.0 && at["veli"] > at["Soyad"] + 10.0, "{at:?}");
    // Without: by the texts', the outlined field's (no room kept for a label) and the filled one's alike.
    assert!(near(at["Yaş:"], at["kırk"], 0.5) && near(at["kırk"], at["elli"], 0.5), "{at:?}");
    // A multi-line one by its last line (body large's 24 dp lines: two below the first).
    assert!(near(at["son"], at["üç"], 0.5) && near(at["üç"], at["bir"] + 2.0 * LINE, 0.5), "{at:?}");
    assert!(near(at["düz"], at["son"], 0.5), "{at:?}");
}

/// C++ `basic_text_fields_take_a_decorator_and_fields_line_limits` (C++ 10781), its Material fields: in a 300 dp
/// column a Material field is 280 dp wide (TextFieldDefaults.MinWidth); `MultiLine(2, 3)` is as tall as the String
/// field's `minLines = 2, maxLines = 3` — two 24 dp lines and the 16 dp paddings, more than 56 + 10 —, three lines at
/// most; `SingleLine` 56 dp, Enter its keyboard action. (Its foundation half — the decorator, the secure field's,
/// the lines' heights of BasicTextFields —: kompoz-foundation text_field.rs::
/// basic_text_fields_take_a_decorator_and_fields_line_limits.)
///
/// Compose differs from the C++: its state-based fields have no `singleLine` beside `lineLimits` (the C++'s "ezen"
/// field, `lineLimits` winning over it), nor an `onSubmit`: Enter in a single-line field runs the keyboard action
/// handler for the action its options name (Done here; for the Default one it does nothing); the C++'s 280 dp
/// BasicSecureTextField is Material's SecureTextField here (Compose's basic one has no minimum width).
fn basic_text_fields_take_a_decorator_and_fields_line_limits() {
    let (a, one) = (TextFieldState::new(""), TextFieldState::new(""));
    let b = mutable_state_of(String::new());
    let submitted = Rc::new(Cell::new(0));
    let (sa, sb, so, sub) = (a.clone(), b.clone(), one.clone(), submitted.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sa, sb, so, sub) = (sa.clone(), sb.clone(), so.clone(), sub.clone());
        MaterialTheme(
            content = move || {
                let (sa, sb, so, sub) = (sa.clone(), sb.clone(), so.clone(), sub.clone());
                Column(
                    modifier = Modifier.width(300.dp()),
                    content = move || {
                        SecureTextField(remember_text_field_state(), modifier = Modifier.test_tag("parola"));
                        TextField(
                            sa.clone(),
                            modifier = Modifier.test_tag("çok"),
                            line_limits = TextFieldLineLimits::multi_line(2, 3),
                        );
                        let sb2 = sb.clone();
                        StringTextField(
                            sb.get(),
                            move |s| sb2.set(s),
                            modifier = Modifier.test_tag("eski"),
                            min_lines = 2,
                            max_lines = 3,
                        );
                        let sub = sub.clone();
                        let handler: Rc<dyn KeyboardActionHandler> =
                            Rc::new(move |_: &dyn Fn()| sub.set(sub.get() + 1));
                        TextField(
                            so.clone(),
                            modifier = Modifier.test_tag("tek"),
                            line_limits = TextFieldLineLimits::SingleLine,
                            keyboard_options = KeyboardOptions(ime_action = ImeAction::Done),
                            on_keyboard_action = Some(handler),
                        );
                    },
                );
            },
        );
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let height = |tag: &str| bounds(&rule, tag).height();
    for tag in ["parola", "çok", "eski", "tek"] {
        assert!(near(bounds(&rule, tag).width(), 280.0, 0.5), "{tag} {:?}", bounds(&rule, tag));
    }
    assert!(near(height("çok"), height("eski"), 0.5) && height("çok") > 56.0 + 10.0);
    assert!(near(height("çok"), INSET + 2.0 * LINE + INSET, 0.5), "{}", height("çok"));
    a.set_text_and_place_cursor_at_end("1\n2\n3\n4\n5");
    b.set("1\n2\n3\n4\n5".to_string());
    rule.wait_for_idle();
    // Three lines tall, no more.
    assert!(near(height("çok"), height("eski"), 0.5) && near(height("çok"), INSET + 3.0 * LINE + INSET, 0.5));
    assert!(near(height("tek"), 56.0, 0.5));
    let field = bounds(&rule, "tek");
    tap(&rule, at(field.left + 20.0, field.top + 20.0));
    type_text(&rule, "a");
    key(&rule, Key::Enter, NONE);
    assert!(one.text() == "a" && submitted.get() == 1);
}

/// C++ `tap_places_the_caret_and_password_shows_dots` (C++ 11305), its Material fields: OutlinedTextFields, their
/// text 16 dp in — a tap three characters past it puts the caret there, one two dots past it on the line's middle
/// (28 dp down) types between them —; the password field (`isError`) draws its supporting text "En az 8 karakter",
/// in the error color, and dots for its text; its keyboard a password's. (Its foundation half: kompoz-foundation
/// text_field.rs::tap_places_the_caret_and_password_shows_dots.)
///
/// Compose differs from the C++ in the backend's numbers: 9 px characters (the C++'s 8); the input method is told
/// again as a field moves (the C++ counted two times told, here two sessions started).
fn tap_places_the_caret_and_password_shows_dots() {
    let value = mutable_state_of("merhaba".to_string());
    let secret = mutable_state_of("gizli".to_string());
    let (v, s) = (value.clone(), secret.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, s) = (v.clone(), s.clone());
        MaterialTheme(
            content = move || {
                let (v, s) = (v.clone(), s.clone());
                Column(move || {
                    let (v2, s2) = (v.clone(), s.clone());
                    StringOutlinedTextField(v.get(), move |t| v2.set(t), modifier = Modifier.test_tag("f"));
                    StringOutlinedTextField(
                        s.get(),
                        move |t| s2.set(t),
                        modifier = Modifier.test_tag("p"),
                        supporting_text = text_slot("En az 8 karakter"),
                        is_error = true,
                        keyboard_options = KeyboardOptions(keyboard_type = KeyboardType::Password),
                        visual_transformation = PasswordVisualTransformation(),
                    );
                });
            },
        );
    }));
    let p = platform(&rule);
    rule.wait_for_idle();
    let f = bounds(&rule, "f");
    tap(&rule, at(f.left + INSET + CHAR * 3.0 + 1.0, f.center().y));
    type_text(&rule, "-");
    assert_eq!(value.peek(), "mer-haba");
    let c = rule.draw();
    assert!(c.texts.contains(&"•••••".to_string()) && !c.texts.contains(&"gizli".to_string()), "{:?}", c.texts);
    let support = c.texts.iter().position(|t| t == "En az 8 karakter").expect("the supporting text");
    assert_eq!(c.colors[support], light_color_scheme().error);
    let pb = bounds(&rule, "p");
    // (the supporting text below: 4 dp and body small's 16 dp line under the 56 dp field)
    assert!(near(pb.height(), 56.0 + 4.0 + 16.0, 0.5) && c.baselines[support].y > pb.top + 56.0, "{pb:?}");
    let dots = c.texts.iter().position(|t| t == "•••••").unwrap();
    assert!(near(c.baselines[dots].x, pb.left + INSET, 0.01));
    tap(&rule, at(pb.left + INSET + CHAR * 2.0 + 1.0, pb.top + 28.0)); // between the 2nd and 3rd dot
    type_text(&rule, "X");
    assert_eq!(secret.peek(), "giXzli");
    // Two input sessions (told again as a field moves), the last a password's.
    assert!(p.sessions_started() == 2 && p.last_shown().is_some_and(|o| o.keyboard_type == KeyboardType::Password));
}

/// C++ `text_field_label_floats_and_keyboard_actions_run` (C++ 11598), its Material field: a single-line TextField
/// with a label and a placeholder draws only its label "Ara" unfocused; focused, the placeholder "örn. kahve" too;
/// 280 x 56 dp; Enter runs its Search action. (Its foundation half — the keyboard action —: kompoz-foundation
/// text_field.rs::keyboard_actions_run.)
///
/// Compose differs from the C++ in the order they are drawn: its layout draws the label before the placeholder
/// (the C++ drew the placeholder first).
fn text_field_label_floats_and_keyboard_actions_run() {
    let value = mutable_state_of(String::new());
    let searched = Rc::new(Cell::new(0));
    let (v, sc) = (value.clone(), searched.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, sc) = (v.clone(), sc.clone());
        MaterialTheme(
            content = move || {
                let (v2, sc) = (v.clone(), sc.clone());
                let search: kompoz_foundation::text::KeyboardAction = Rc::new(move |_| sc.set(sc.get() + 1));
                StringTextField(
                    v.get(),
                    move |t| v2.set(t),
                    modifier = Modifier.test_tag("f"),
                    single_line = true,
                    placeholder = text_slot("örn. kahve"),
                    label = text_slot("Ara"),
                    keyboard_options = KeyboardOptions(ime_action = ImeAction::Search),
                    keyboard_actions = KeyboardActions(on_search = Some(search)),
                );
            },
        );
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let drawn =
        |rule: &KompozTestRule| -> Vec<String> { rule.drawn_texts().into_iter().filter(|t| !t.is_empty()).collect() };
    assert_eq!(drawn(&rule), vec!["Ara".to_string()]); // the label, no placeholder until focused
    rule.on_node_with_tag("f").perform_click();
    settle(&rule);
    assert_eq!(drawn(&rule), vec!["Ara".to_string(), "örn. kahve".to_string()]);
    key(&rule, Key::Enter, NONE);
    assert_eq!(searched.get(), 1);
    let f = bounds(&rule, "f");
    assert!(near(f.width(), 280.0, 0.5) && near(f.height(), 56.0, 0.5), "{f:?}");
}

/// C++ `refocused_field_shows_no_stale_selection_and_password_is_not_copied` (C++ 12272), its Material fields:
/// OutlinedTextFields — what is drawn in the first one is its outline (a stroke) and, all selected, one highlight in
/// the theme's selection color; tabbed to the password field and back, the outline only. (Its foundation half —
/// the selection collapsed as the field lost the focus, the password not copied —: kompoz-foundation
/// text_field_selection.rs::refocused_field_shows_no_stale_selection_and_password_is_not_copied.)
///
/// Compose differs from the C++: Tab in a multi-line field types a tab (the C++ moved the focus), so both fields
/// are single-line here; the C++ counted every filled rect in the field (only its highlight was one), Compose's
/// outlined container is a transparent fill of its shape and its highlight a path, counted by color; its
/// `performClick` is a finger's click, which leaves the legacy field's cursor handle (drawn in the theme's primary,
/// kept through Ctrl+A as Compose's legacy field keeps its handle state).
fn refocused_field_shows_no_stale_selection_and_password_is_not_copied() {
    let value = mutable_state_of("metin".to_string());
    let secret = mutable_state_of("gizli".to_string());
    let (v, s) = (value.clone(), secret.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, s) = (v.clone(), s.clone());
        Steady(Rc::new(RecordingToolbar::default()), move || {
            let (v, s) = (v.clone(), s.clone());
            Column(move || {
                let (v2, s2) = (v.clone(), s.clone());
                StringOutlinedTextField(
                    v.get(),
                    move |t| v2.set(t),
                    modifier = Modifier.test_tag("f"),
                    single_line = true,
                );
                StringOutlinedTextField(
                    s.get(),
                    move |t| s2.set(t),
                    modifier = Modifier.test_tag("p"),
                    single_line = true,
                    visual_transformation = PasswordVisualTransformation(),
                );
            });
        });
    }));
    let p = platform(&rule);
    p.set_clip("önceki");
    rule.wait_for_idle();
    let f = bounds(&rule, "f");
    // The outline: a stroke around the field; nothing else is filled in it but its transparent container.
    let outline = |c: &RecordingCanvas| {
        c.strokes.iter().filter(|s| near(s.width(), f.width(), 2.0) && near(s.top, f.top, 2.0)).count()
            + (0..c.paths.len())
                .filter(|&i| c.path_strokes[i] > 0.0 && near(c.paths[i].width(), f.width(), 2.0))
                .filter(|&i| near(c.paths[i].top, f.top, 2.0))
                .count()
    };
    // (the click's cursor handle aside: in the theme's primary)
    let opaque_fills = |c: &RecordingCanvas| {
        (0..c.rects.len())
            .filter(|&i| f.contains(c.rects[i].center()) && c.rect_colors[i].alpha() > 0.0)
            .filter(|&i| c.rect_colors[i] != light_color_scheme().primary)
            .count()
    };
    rule.on_node_with_tag("f").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::A, CTRL);
    assert_eq!(selection(&rule, "f"), TextRange::new(0, 5));
    let c = rule.draw();
    assert!(highlights(&c, f).len() == 1 && outline(&c) == 1 && opaque_fills(&c) == 0);
    key(&rule, Key::Tab, NONE); // to the password field
    assert!(is_focused(&rule, "p"));
    key(&rule, Key::A, CTRL);
    key(&rule, Key::C, CTRL);
    assert_eq!(p.clipboard(), "önceki"); // not the password
    key(&rule, Key::Tab, SHIFT); // back: the caret at the end, nothing selected
    assert!(is_focused(&rule, "f"));
    let c = rule.draw();
    assert!(highlights(&c, f).is_empty() && outline(&c) == 1 && opaque_fills(&c) == 0);
}

/// C++ `secure_text_field_shows_the_character_just_typed_for_a_moment` (C++ 19767), its Material fields: the filled
/// SecureTextField (its label "Parola") draws its surface container highest container at its top, the
/// OutlinedSecureTextField a stroked outline its width (its own "*" for the text); a tap brings a password
/// keyboard, without suggestions, for one line; what is typed shows a moment in the Material field, then a dot;
/// leaving it hides it, the outlined one types through its max length; Enter is the keyboard action. (Its
/// foundation half — the reveal, the input method's edits, copying refused, the modes —: kompoz-foundation
/// text_field.rs::secure_text_field_shows_the_character_just_typed_for_a_moment.)
///
/// Compose differs from the C++: its filled container is a path of the top-rounded shape (the C++'s a rect); its
/// `onSubmit` is the keyboard action handler, run by Enter for the action the options name (Done here: the secure
/// field's options with it).
fn secure_text_field_shows_the_character_just_typed_for_a_moment() {
    let secret = TextFieldState::new("");
    let other = TextFieldState::new("gizli");
    let submitted = Rc::new(Cell::new(0));
    let (sc, ot, sb) = (secret.clone(), other.clone(), submitted.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (sc, ot, sb) = (sc.clone(), ot.clone(), sb.clone());
            Steady(Rc::new(RecordingToolbar::default()), move || {
                let (sc, ot, sb) = (sc.clone(), ot.clone(), sb.clone());
                Column(move || {
                    let sb = sb.clone();
                    let handler: Rc<dyn KeyboardActionHandler> = Rc::new(move |_: &dyn Fn()| sb.set(sb.get() + 1));
                    SecureTextField(
                        sc.clone(),
                        modifier = Modifier.test_tag("şifre"),
                        label = label_slot("Parola"),
                        keyboard_options = KeyboardOptions::SecureTextField.copy(ime_action = ImeAction::Done),
                        on_keyboard_action = Some(handler),
                    );
                    OutlinedSecureTextField(
                        ot.clone(),
                        modifier = Modifier.test_tag("diğer"),
                        text_obfuscation_character = '*',
                        input_transformation = Some(InputTransformation.max_length(6)),
                    );
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    rule.wait_for_idle();
    let shows = |text: &str| rule.drawn_texts().iter().any(|t| t == text);
    assert!(shows("*****") && shows("Parola"));
    let (filled, outlined) = (bounds(&rule, "şifre"), bounds(&rule, "diğer"));
    let c = rule.draw();
    let container = (0..c.paths.len()).any(|i| {
        c.path_colors[i] == light_color_scheme().surface_container_highest
            && c.path_strokes[i] == 0.0
            && near(c.paths[i].top, filled.top, 0.5)
            && near(c.paths[i].width(), filled.width(), 0.5)
    });
    let outline = c.strokes.iter().any(|s| near(s.width(), outlined.width(), 2.0) && near(s.top, outlined.top, 2.0))
        || (0..c.paths.len()).any(|i| {
            c.path_strokes[i] > 0.0
                && near(c.paths[i].width(), outlined.width(), 2.0)
                && near(c.paths[i].top, outlined.top, 2.0)
        });
    assert!(container && outline, "{container} {outline}");
    assert!(near(filled.width(), 280.0, 0.5) && near(filled.height(), 56.0, 0.5));
    tap(&rule, filled.center()); // a finger's tap
    let keyboard = p.last_shown().expect("a keyboard");
    assert!(keyboard.keyboard_type == KeyboardType::Password && !keyboard.auto_correct && keyboard.single_line);
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
    rule.host().text_input("q");
    rule.advance_frame();
    assert!(shows("••q"));
    // Leaving the field hides it; the outlined one keeps to its six characters.
    key(&rule, Key::Tab, NONE);
    assert!(shows("•••") && is_focused(&rule, "diğer"));
    rule.host().text_input("1");
    rule.host().text_input("2");
    rule.advance_frame();
    assert!(other.text() == "gizli1" && shows("******"));
    // Enter in the filled one: its action, no new line.
    rule.on_node_with_tag("şifre").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::Enter, NONE);
    assert!(submitted.get() == 1 && !secret.text().contains('\n'));
}

/// C++ `multi_line_field_wraps_and_grows_up_to_max_lines` (C++ 21072), its Material field: an OutlinedTextField of
/// at most three lines, 280 dp wide by default, 56 dp tall empty; words of eight letters wrap: two lines are
/// 16 + 2 x 24 + 16 tall; four, capped at three, 16 + 3 x 24 + 16, the first line drawn as wrapped. (Its
/// foundation half: kompoz-foundation text_field.rs::multi_line_field_wraps_and_grows_up_to_max_lines.)
///
/// Compose differs from the C++: the 280 dp is a minimum (`defaultMinSize`) — a field's text is as wide as it
/// wants up to the constraints, so the field widens with it to the 400 dp window (368 for the text: 40 characters
/// of 9 px, four words a line; the C++ kept 280, 248 for 29 of its 8.5 px characters, three words): six words are
/// two lines, sixteen four.
fn multi_line_field_wraps_and_grows_up_to_max_lines() {
    let value = mutable_state_of(String::new());
    let v = value.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let v = v.clone();
            MaterialTheme(
                content = move || {
                    let v2 = v.clone();
                    StringOutlinedTextField(
                        v.get(),
                        move |t| v2.set(t),
                        modifier = Modifier.test_tag("f"),
                        max_lines = 3,
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let f = bounds(&rule, "f");
    assert!(near(f.width(), 280.0, 0.5) && near(f.height(), 56.0, 0.5), "{f:?}");
    let word = "aaaaaaaa ";
    value.set(word.repeat(6)); // four words a line: two lines
    rule.wait_for_idle();
    let f = bounds(&rule, "f");
    assert!(near(f.width(), 400.0, 0.5) && near(f.height(), INSET + 2.0 * LINE + INSET, 0.5), "{f:?}");
    value.set(word.repeat(16)); // four lines: three show, the field scrolls
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), INSET + 3.0 * LINE + INSET, 0.5));
    let texts = rule.drawn_texts();
    assert!(texts.len() >= 3 && texts[0].trim_end() == "aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa", "{texts:?}");
}

/// C++ `text_field_state_keeps_its_selection` (C++ 21202), its Material field: a single-line TextField of a state
/// the app and the keyboard both move the selection of — tabbed to, typed into at the app's caret — shows the
/// state's selection highlighted in the theme's primary at 40 %, over its characters from the 16 dp inset. (Its
/// foundation halves: kompoz-foundation text_input.rs::text_field_state_keeps_its_selection (the model),
/// text_field.rs::a_field_draws_its_cursor_and_its_selection (drawn).)
///
/// Compose differs from the C++: its highlight is a path (the C++'s a rect); the C++'s `singleLine` is
/// `lineLimits = SingleLine`.
fn text_field_state_keeps_its_selection() {
    let state = TextFieldState::new("merhaba");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Steady(Rc::new(RecordingToolbar::default()), move || {
            let s = s.clone();
            Column(move || {
                TextField(s.clone(), modifier = Modifier.test_tag("f"), line_limits = TextFieldLineLimits::SingleLine);
                Text(format!("{}-{}", s.selection().start, s.selection().end), modifier = Modifier.test_tag("seçim"));
            });
        });
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let shown = |rule: &KompozTestRule| {
        rule.on_node_with_tag("seçim").fetch_semantics_node().config().get(SemanticsProperties::Text).unwrap()[0]
            .text()
            .to_string()
    };
    assert_eq!(shown(&rule), "7-7");
    state.edit(|b| b.place_cursor_before_char_at(3)); // before it is focused
    rule.wait_for_idle();
    key(&rule, Key::Tab, NONE); // focused: typed at that caret
    type_text(&rule, "X");
    assert!(state.text() == "merXhaba" && shown(&rule) == "4-4");
    let f = bounds(&rule, "f");
    assert!(highlights(&rule.draw(), f).is_empty());
    key(&rule, Key::DirectionRight, SHIFT); // the keyboard's selection
    assert_eq!(state.selection(), TextRange::new(4, 5));
    state.edit(|b| b.set_selection(TextRange::new(2, 6))); // the app's
    rule.wait_for_idle();
    let drawn = highlights(&rule.draw(), f);
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert!(near(drawn[0].left, f.left + INSET + 2.0 * CHAR, 0.01) && near(drawn[0].width(), 4.0 * CHAR, 0.01));
    assert!(near(drawn[0].top, f.top + INSET, 0.01) && near(drawn[0].height(), LINE, 0.01), "{drawn:?}");
}

/// C++ `output_transformation_formats_what_is_shown` (C++ 21254), its Material fields: single-line TextFields
/// with output transformations show the grouped phone number and the amount between "$" and " USD"; taps from the
/// 16 dp inset — just past a separator, on it — put the caret before the digit after it; the amount all selected
/// is highlighted in the theme's primary at 40 %; accessibility gets the text itself. (Its foundation halves:
/// kompoz-foundation text_field.rs::output_transformation_formats_what_is_shown (drawn and tapped),
/// text_input.rs::output_transformation_formats_what_is_shown (the mapping).)
///
/// Compose differs from the C++: the selection's start keeps the prefix's wedge — "$" is highlighted with the
/// digits, from the inset, six characters wide; the C++'s highlight started after it, five wide —; accessibility
/// gets the text itself as its input text and what is shown as its editable text (the C++'s editable value was the
/// text); 9 px characters (the C++'s 8.5).
fn output_transformation_formats_what_is_shown() {
    let phone = TextFieldState::new("5551234567");
    let amount = TextFieldState::new("250");
    let (p, a) = (phone.clone(), amount.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, a) = (p.clone(), a.clone());
        Steady(Rc::new(RecordingToolbar::default()), move || {
            let (p, a) = (p.clone(), a.clone());
            Column(move || {
                TextField(
                    p.clone(),
                    modifier = Modifier.test_tag("tel"),
                    line_limits = TextFieldLineLimits::SingleLine,
                    output_transformation = Some(output_transformation(|b| {
                        for at in [8, 6, 3] {
                            if b.length() > at {
                                b.insert(at, " ");
                            }
                        }
                    })),
                );
                TextField(
                    a.clone(),
                    modifier = Modifier.test_tag("tutar"),
                    line_limits = TextFieldLineLimits::SingleLine,
                    output_transformation = Some(output_transformation(|b| {
                        b.insert(0, "$");
                        b.append(" USD");
                    })),
                );
            });
        });
    }));
    let _p = platform(&rule);
    rule.wait_for_idle();
    let drawn = |t: &str| rule.drawn_texts().iter().filter(|d| *d == t).count() == 1;
    assert!(drawn("555 123 45 67") && drawn("$250 USD") && phone.text() == "5551234567");
    key(&rule, Key::Tab, NONE);
    type_text(&rule, "8");
    assert!(phone.text() == "55512345678" && drawn("555 123 45 678"));
    let tel = bounds(&rule, "tel");
    tap(&rule, at(tel.left + INSET + 4.0 * CHAR + 1.0, tel.center().y)); // just past a separator
    assert_eq!(phone.selection(), TextRange::at(3));
    type_text(&rule, "0");
    assert!(phone.text() == "555012345678" && drawn("555 012 34 5678"), "{:?}", rule.drawn_texts());
    rule.advance_time_by(500); // not a double tap
    tap(&rule, at(tel.left + INSET + 3.0 * CHAR + 3.0, tel.center().y)); // on the separator: where it was put
    assert_eq!(phone.selection(), TextRange::at(3));
    key(&rule, Key::Tab, NONE); // the amount: its caret between the prefix and the suffix
    assert_eq!(amount.selection(), TextRange::at(3));
    type_text(&rule, "0");
    key(&rule, Key::MoveHome, NONE);
    type_text(&rule, "1");
    assert!(amount.text() == "12500" && drawn("$12500 USD"));
    amount.edit(|b| b.select_all());
    rule.wait_for_idle();
    let tutar = bounds(&rule, "tutar");
    let highlights = highlights(&rule.draw(), tutar);
    assert_eq!(highlights.len(), 1, "{highlights:?}");
    assert!(near(highlights[0].left, tutar.left + INSET, 0.01) && near(highlights[0].width(), 6.0 * CHAR, 0.01));
    let config = rule.on_node_with_tag("tutar").fetch_semantics_node().config().clone();
    let text_of = |key| config.get(key).map(|t: kompoz_text::AnnotatedString| t.text().to_string());
    assert_eq!(text_of(SemanticsProperties::InputText), Some("12500".to_string()));
    assert_eq!(text_of(SemanticsProperties::EditableText), Some("$12500 USD".to_string()));
}

/// C++ `password_field_maps_its_selection_to_the_dots` (C++ 21313), its Material field: a password field's
/// selection "bç" is highlighted in the theme's primary at 40 % over the dots that stand for it — one dot past the
/// 16 dp inset, two dots wide. (Its foundation half: kompoz-foundation text_field_selection.rs::
/// password_field_maps_its_selection_to_the_dots.)
///
/// Compose differs from the C++: a state-based field has no visual transformation — Compose's password field of a
/// state is SecureTextField (the C++'s `TextField(state, visualTransformation = Password)`, single-line).
fn password_field_maps_its_selection_to_the_dots() {
    let secret = TextFieldState::new("abçd");
    let s = secret.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Steady(Rc::new(RecordingToolbar::default()), move || {
            SecureTextField(s.clone(), modifier = Modifier.test_tag("şifre"));
        });
    }));
    let _p = platform(&rule);
    key(&rule, Key::Tab, NONE);
    assert!(is_focused(&rule, "şifre"));
    secret.edit(|b| b.set_selection(TextRange::new(1, 4))); // "bç": one dot in, two dots wide
    rule.wait_for_idle();
    let field = bounds(&rule, "şifre");
    let c = rule.draw();
    let drawn = highlights(&c, field);
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    let dot = drawn[0].left - (field.left + INSET);
    assert!(near(dot, CHAR, 0.01) && near(drawn[0].width(), 2.0 * dot, 0.01), "{drawn:?}");
    let dots = c.texts.iter().position(|t| t == "••••").expect("the dots");
    assert!(near(c.baselines[dots].x, field.left + INSET, 0.01));
}

// ---- The selection tests' Material geometry -----------------------------------------------------------------------

/// C++ `long_press_selects_a_word_with_handles_and_a_toolbar` (C++ 904), its Material geometry: in a single-line
/// TextField a long press at a character of "güzel" — 16 dp in, on the line's middle 28 dp down — selects the word,
/// felt, with both handles and the toolbar over it (its rect from the word's start past the inset, from the line's
/// top 16 dp down to the handles' bottom); copy collapses it; the end handle, hanging under the line's bottom
/// (40 dp down) at the end of "merhaba", dragged takes " güzel" in; select all and cut; a press elsewhere keeps a
/// selection, typing ends it. (Its foundation half — the same on a BasicTextField, Compose's behaviour where the
/// C++ differs —: kompoz-foundation text_field_selection.rs::long_press_selects_a_word_with_handles_and_a_toolbar.)
///
/// Compose differs from the C++ as said there (the toolbar is LocalTextToolbar's; the drag snaps to the word, one
/// tick; a press elsewhere, on nothing focusable, leaves the field focused with its selection, handles and toolbar
/// — the C++ ended it); the String field's (legacy) toolbar rect reaches 25 dp below the line, over the handles
/// (Compose's legacy `getContentRect`; the state-based field's ends at the line); the line's
/// bottom is 40 dp down (the C++'s 36 of its 20 dp line), 9 px characters (the C++'s 8.5).
fn long_press_selects_a_word_with_handles_and_a_toolbar() {
    let value = mutable_state_of("merhaba güzel dünya".to_string());
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = field_under_a_box(value.clone(), toolbar.clone(), mutable_state_of(false));
    let p = platform(&rule);
    rule.wait_for_idle();
    let field = bounds(&rule, "field");
    assert!(near(field.height(), 56.0, 0.5));
    long_press(&rule, char_at(&rule, "field", 10, 0)); // in "güzel" (bytes 8..14: ü takes two)
    assert_eq!(selection(&rule, "field"), TextRange::new(8, 14));
    assert_eq!(*p.haptics.borrow(), vec![HapticFeedbackType::LongPress]);
    assert!(handle(&rule, Handle::SelectionStart).is_some() && handle(&rule, Handle::SelectionEnd).is_some());
    assert_eq!(toolbar.offers(), (true, false, true, true));
    let r = toolbar.rect.get();
    assert!(near(r.left, field.left + INSET + 8.0 * CHAR, 0.01) && near(r.width(), 5.0 * CHAR, 0.01), "{r:?}");
    // (from the line's top to the handles' bottom, 25 dp under the line's)
    assert!(near(r.top, field.top + INSET, 0.01) && near(r.bottom, field.top + INSET + LINE + 25.0, 0.01), "{r:?}");
    toolbar.choose(&rule, &toolbar.copy);
    assert_eq!(p.clipboard(), "güzel");
    assert_eq!(selection(&rule, "field"), TextRange::at(14));
    assert!(handles(&rule).is_empty() && !toolbar.shown());
    long_press(&rule, char_at(&rule, "field", 3, 0));
    assert_eq!(selection(&rule, "field"), TextRange::new(0, 7));
    // The end handle: under the end of "merhaba" (in the inner field: 7 characters, the line's bottom).
    let end = handle(&rule, Handle::SelectionEnd).expect("the end handle");
    assert!(near(end.position.x, 7.0 * CHAR, 0.01) && near(end.position.y, LINE, 0.01), "{end:?}");
    let ticks = p.felt(HapticFeedbackType::TextHandleMove);
    let grab = at(field.left + INSET + 7.0 * CHAR + 11.0, field.top + INSET + LINE + 11.0);
    finger_drag(&rule, grab, at(CHAR / 2.0, 0.0), 12, true); // half a character at a time
    assert_eq!(selection(&rule, "field"), TextRange::new(0, 14));
    assert_eq!(p.felt(HapticFeedbackType::TextHandleMove), ticks + 1);
    let end = handle(&rule, Handle::SelectionEnd).expect("the end handle");
    assert!(near(end.position.x, 13.0 * CHAR, 0.01), "{end:?}"); // now under the end of "güzel"
    toolbar.choose(&rule, &toolbar.select_all);
    assert_eq!(selection(&rule, "field"), TextRange::new(0, "merhaba güzel dünya".len()));
    toolbar.choose(&rule, &toolbar.cut);
    assert!(value.peek().is_empty() && p.clipboard() == "merhaba güzel dünya");
    assert!(handles(&rule).is_empty() && !toolbar.shown());
    value.set("bir iki".to_string());
    rule.wait_for_idle();
    long_press(&rule, char_at(&rule, "field", 5, 0));
    assert_eq!(selection(&rule, "field"), TextRange::new(4, 7));
    tap(&rule, bounds(&rule, "below").center());
    // A press elsewhere (on nothing focusable) leaves the field focused, its selection, handles and toolbar.
    assert!(is_focused(&rule, "field") && selection(&rule, "field") == TextRange::new(4, 7));
    assert!(toolbar.shown() && handles(&rule).len() == 2);
    // Typing ends a selection.
    long_press(&rule, char_at(&rule, "field", 1, 0));
    assert_eq!(selection(&rule, "field"), TextRange::new(0, 3));
    type_text(&rule, "x");
    assert_eq!(value.peek(), "x iki");
    assert!(!toolbar.shown() && handle(&rule, Handle::SelectionStart).is_none());
}

/// C++ `caret_handle_for_fingers_double_click_for_the_mouse` (C++ 1127), its Material geometry: a finger's tap
/// three characters past the 16 dp inset puts the caret there with its handle under the line's bottom; a tap on the
/// handle brings the toolbar (paste, select all); dragged four characters on, the caret follows, the toolbar
/// hidden; the mouse's long press selects nothing, its click shows no handle, its double click takes "güzel". (Its
/// foundation half: kompoz-foundation text_field_selection.rs::caret_handle_for_fingers_double_click_for_the_mouse.)
///
/// Compose differs from the C++: paste is offered only with text in the clipboard; the drag doesn't move by the
/// 8 px touch slop (44 px dragged for four characters; the C++ 32); the String field (legacy) hides the toolbar as
/// its cursor handle is dragged, as the C++ did (the state-based field of the foundation half shows it again); the
/// line's bottom is 40 dp down (the C++'s 36), 9 px characters (the C++'s 8).
fn caret_handle_for_fingers_double_click_for_the_mouse() {
    let value = mutable_state_of("merhaba güzel dünya".to_string());
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = field_under_a_box(value.clone(), toolbar.clone(), mutable_state_of(false));
    let p = platform(&rule);
    p.set_clip("pano");
    rule.wait_for_idle();
    let field = bounds(&rule, "field");
    tap(&rule, at(field.left + INSET + 3.0 * CHAR + 1.0, field.top + 28.0));
    assert_eq!(selection(&rule, "field"), TextRange::at(3));
    let cursor = handle(&rule, Handle::Cursor).expect("the cursor handle");
    assert!(near(cursor.position.x, 3.0 * CHAR, 1.01) && near(cursor.position.y, LINE, 0.01), "{cursor:?}");
    assert!(!toolbar.shown());
    rule.advance_time_by(500); // not a double tap
    let grab = at(field.left + INSET + cursor.position.x, field.top + INSET + LINE + 13.0);
    tap(&rule, grab);
    assert_eq!(toolbar.offers(), (false, true, false, true));
    // Four characters on (the touch slop's 8 px not taken by the drag).
    finger_drag(&rule, grab, at(11.0, 0.0), 4, true);
    assert_eq!(selection(&rule, "field"), TextRange::at(7));
    assert!(!toolbar.shown()); // hidden by the drag, as the C++'s
    rule.advance_time_by(500);
    let mouse_at = char_at(&rule, "field", 10, 0);
    rule.host().pointer_down(mouse_at, -1, 0, PointerType::Mouse);
    rule.advance_time_by(600);
    rule.host().pointer_up(mouse_at, -1, 0);
    rule.wait_for_idle();
    assert!(selection(&rule, "field").collapsed() && handles(&rule).is_empty());
    clicks(&rule, mouse_at, 1);
    assert_eq!(selection(&rule, "field"), TextRange::at(11)); // before "e" ("ü" takes two bytes)
    assert!(handles(&rule).is_empty());
    clicks(&rule, mouse_at, 2);
    assert_eq!(selection(&rule, "field"), TextRange::new(8, 14));
    assert!(handles(&rule).is_empty() && !toolbar.shown());
}

/// C++ `a_double_click_takes_letters_digits_underscores_and_what_is_beyond_ascii` (C++ 1172), its Material
/// geometry: a double click four characters past a TextField's 16 dp inset, on its line, takes "snake_case2ü".
/// (Its foundation half: kompoz-foundation text_field_selection.rs::
/// a_double_click_takes_letters_digits_underscores_and_what_is_beyond_ascii.)
///
/// Compose differs from the C++ only in the backend's numbers (9 px characters, the C++'s 8).
fn a_double_click_takes_letters_digits_underscores_and_what_is_beyond_ascii() {
    let value = mutable_state_of("x snake_case2ü, y".to_string());
    let v = value.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let v = v.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                let v2 = v.clone();
                StringTextField(v.get(), move |t| v2.set(t), modifier = Modifier.fill_max_width().test_tag("f"));
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    clicks(&rule, char_at(&rule, "f", 4, 0), 2); // in "snake"
    assert_eq!(selection(&rule, "f"), TextRange::new(2, 15)); // "snake_case2ü" (ü takes two bytes), not the comma
    assert_eq!(value.peek(), "x snake_case2ü, y");
}

/// C++ `the_mouse_selects_text_by_dragging_clicking_and_shift` (C++ 1191), its Material geometry: in a multi-line
/// TextField, columns counted from the 16 dp inset on the first line: a click places the caret (and focuses),
/// a drag selects, two clicks take "beta" and dragging goes by words, three the paragraph, Shift extends. (Its
/// foundation half: kompoz-foundation text_field_selection.rs::the_mouse_selects_text_by_dragging_clicking_and_shift.)
///
/// Compose's String field (its legacy engine) does as the C++ — dragged back from a word the selection is reversed,
/// 10..0 (the state-based field of the foundation half runs it forwards) —; it differs in that its selection
/// manager works from the value the app gave back, recomposed: a drag's moves come a frame apart here (the C++
/// moved twelve times in one); 9 px characters (the C++'s 8).
fn the_mouse_selects_text_by_dragging_clicking_and_shift() {
    let value = mutable_state_of("alpha beta gamma\ndelta".to_string());
    let v = value.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let v = v.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                let v = v.clone();
                Column(move || {
                    Box(modifier = Modifier.height(100.dp()));
                    let v2 = v.clone();
                    StringTextField(v.get(), move |t| v2.set(t), modifier = Modifier.fill_max_width().test_tag("f"));
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let _p = platform(&rule);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), INSET + 2.0 * LINE + INSET, 0.5));
    let h = rule.host();
    let x = |column: usize| char_at(&rule, "f", column, 0);
    let caret_at = |column: usize| {
        clicks(&rule, x(column), 1);
        selection(&rule, "f").end
    };
    let c1 = caret_at(1);
    assert!(is_focused(&rule, "f") && selection(&rule, "f") == TextRange::at(c1));
    let (c3, c13) = (caret_at(3), caret_at(13));
    assert_eq!((c1, c3, c13), (1, 3, 13));
    rule.advance_time_by(500);
    h.pointer_down(x(1), -1, 0, PointerType::Mouse);
    for i in 2..=13 {
        rule.advance_frame(); // (the String field hears its value back by recomposing: a frame a move)
        h.pointer_move(x(i), -1, 0);
    }
    h.pointer_up(x(13), -1, 0);
    rule.wait_for_idle();
    assert_eq!(selection(&rule, "f"), TextRange::new(c1, c13));
    rule.advance_time_by(500);
    h.pointer_down(x(7), -1, 0, PointerType::Mouse);
    h.pointer_up(x(7), -1, 0);
    h.pointer_down(x(7), -1, 0, PointerType::Mouse);
    rule.wait_for_idle();
    assert_eq!(selection(&rule, "f"), TextRange::new(6, 10)); // "beta"
    h.pointer_move(x(13), -1, 0);
    rule.wait_for_idle();
    assert_eq!(selection(&rule, "f"), TextRange::new(6, 16));
    // ...or back, from the end of "beta" to the start of "alpha": reversed, as the C++'s.
    h.pointer_move(x(1), -1, 0);
    h.pointer_up(x(1), -1, 0);
    rule.wait_for_idle();
    assert_eq!(selection(&rule, "f"), TextRange::new(10, 0));
    clicks(&rule, x(7), 3);
    assert_eq!(selection(&rule, "f"), TextRange::new(0, 16));
    caret_at(3);
    h.set_keyboard_modifiers(SHIFT);
    clicks(&rule, x(13), 1);
    h.set_keyboard_modifiers(NONE);
    rule.wait_for_idle();
    assert_eq!(selection(&rule, "f"), TextRange::new(c3, c13));
    assert_eq!(value.peek(), "alpha beta gamma\ndelta");
}

/// C++ `dragging_a_handle_shows_the_text_magnifier` (C++ 6685), its Material geometry: while a finger drags the
/// end handle of "merhaba" — grabbed under the line's bottom, 40 dp down — the magnifier shows where the drag took
/// the end, at the line's middle (28 dp down), zoomed; the toolbar out of the way; both back as the finger lifts.
/// (Its foundation half: kompoz-foundation text_field_selection.rs::dragging_a_handle_shows_the_text_magnifier.)
///
/// Compose differs from the C++: the drag doesn't move by the 8 px touch slop (40 of the 48 px; the C++'s 48); the
/// line's middle is 28 dp down exactly (the C++ checked it within the field); 9 px characters (the C++'s 8.5).
fn dragging_a_handle_shows_the_text_magnifier() {
    let value = mutable_state_of("merhaba güzel dünya".to_string());
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = field_under_a_box(value.clone(), toolbar.clone(), mutable_state_of(false));
    rule.wait_for_idle();
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 3, 0)); // "merhaba"
    assert!(magnifier_at(&rule).is_none() && toolbar.shown());
    let grab = at(field.left + INSET + 7.0 * CHAR + 11.0, field.top + INSET + LINE + 11.0); // the end handle
    finger_drag(&rule, grab, at(8.0, 0.0), 6, false);
    let shown = magnifier_at(&rule).expect("the magnifier");
    assert!(
        near(shown.x, field.left + INSET + 7.0 * CHAR + 40.0, 0.01) && near(shown.y, field.top + 28.0, 0.01),
        "{shown:?}"
    );
    assert!(!toolbar.shown());
    let canvas = rule.draw();
    assert!(canvas.layer_transforms.iter().any(|t| near(t.scale_x, 1.25, 0.01) && near(t.translation_y, -42.0, 0.01)));
    rule.host().pointer_up(grab + at(48.0, 0.0), -1, 0);
    rule.wait_for_idle();
    assert!(magnifier_at(&rule).is_none() && toolbar.shown());
}

/// C++ `selection_handles_point_at_their_end_and_hold_its_line_by_the_middle` (C++ 6749), its Material geometry: in
/// a multi-line TextField a long press on "burada" (the second line, 16 + 24 + 12 dp down) selects it; each handle's
/// point is a square at its top, towards the text, at the second line's bottom (16 + 2 x 24 dp down) — the start's
/// right edge 6 characters past the 16 dp inset, the end's left edge 12 —; the start handle dragged up goes up a
/// line. (Its foundation half: kompoz-foundation text_field_selection.rs::
/// selection_handles_point_at_their_end_and_hold_its_line_by_the_middle.)
///
/// Compose differs from the C++: its handles are 25 dp, their squares 12.5 (the C++'s 22 and 11); the drag doesn't
/// move by the 8 px touch slop; the String field's (legacy) handle is held from 1 px above its line's bottom
/// (Compose's `getAdjustedCoordinates`), not its middle — three quarters of a line up (22 px past the slop; the C++
/// dragged 18) keeps it on its line, a line and more (25 px) takes it up one (the state-based field's, in the
/// foundation half, holds it by the middle); 9 px characters (the C++'s 8.5).
fn selection_handles_point_at_their_end_and_hold_its_line_by_the_middle() {
    let value = mutable_state_of("merhaba güzel\ndünya burada".to_string());
    let v = value.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let v = v.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                let v = v.clone();
                Column(move || {
                    Box(modifier = Modifier.height(200.dp()));
                    let v2 = v.clone();
                    StringTextField(
                        v.get(),
                        move |t| v2.set(t),
                        modifier = Modifier.fill_max_width().test_tag("field"),
                    );
                });
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 8, 1)); // in "burada", on the second line
    assert_eq!(selection(&rule, "field"), TextRange::new(22, 28)); // (ü takes two bytes)
    let handle_color = light_color_scheme().primary;
    let canvas = rule.draw();
    let mut points: Vec<Rect> = (0..canvas.rects.len())
        .filter(|&i| near(canvas.rects[i].width(), 12.5, 0.01) && near(canvas.rects[i].height(), 12.5, 0.01))
        .filter(|&i| canvas.rect_colors[i] == handle_color)
        .map(|i| canvas.rects[i])
        .collect();
    points.sort_by(|a, b| a.left.total_cmp(&b.left));
    assert_eq!(points.len(), 2, "{points:?}");
    let bottom = field.top + INSET + 2.0 * LINE;
    assert!(
        near(points[0].right, field.left + INSET + 6.0 * CHAR, 0.01) && near(points[0].top, bottom, 0.01),
        "{points:?}"
    );
    assert!(
        near(points[1].left, field.left + INSET + 12.0 * CHAR, 0.01) && near(points[1].top, bottom, 0.01),
        "{points:?}"
    );
    let grab = at(points[0].left, points[0].top + 12.5); // the start handle, in its middle
    // Three quarters of a line (22 px past the slop): held from 1 px above the line's bottom, still on it.
    finger_drag(&rule, grab, at(0.0, -5.0), 6, true);
    let selected = selection(&rule, "field");
    assert!(selected.start >= 14 && selected.end == 28, "{selected:?}");
    // A line and more (25 px past it): up a line.
    rule.advance_time_by(500);
    let start = handle(&rule, Handle::SelectionStart).expect("the start handle");
    let grab = at(field.left + INSET + start.position.x - 12.5, field.top + INSET + start.position.y + 12.5);
    finger_drag(&rule, grab, at(0.0, -5.5), 6, true);
    let selected = selection(&rule, "field");
    assert!(selected.start < 14 && selected.end == 28, "{selected:?}");
}

/// C++ `tapping_a_multi_line_field_puts_the_caret_on_that_line` (C++ 21120), its Material geometry: a tap at the
/// start of a TextField's second line — 16 dp in, 16 + 24 + 12 dp down — puts the caret there; the keyboard is told
/// Enter makes new lines; a single-line one's isn't. (Its foundation half: kompoz-foundation text_field_selection.rs::
/// tapping_a_multi_line_field_puts_the_caret_on_that_line.)
///
/// Compose differs from the C++ only in the backend's numbers: 24 dp lines (the C++ tapped 16 + 20 + 10 down).
fn tapping_a_multi_line_field_puts_the_caret_on_that_line() {
    let value = mutable_state_of("abc\ndefgh\nij".to_string());
    let v = value.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let v = v.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                let v = v.clone();
                Column(move || {
                    let v2 = v.clone();
                    StringTextField(v.get(), move |t| v2.set(t), modifier = Modifier.test_tag("f"));
                    StringTextField("", |_| {}, modifier = Modifier.test_tag("single"), single_line = true);
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), INSET + 3.0 * LINE + INSET, 0.5));
    let f = bounds(&rule, "f");
    tap(&rule, at(f.left + INSET + 2.0, f.top + INSET + LINE + LINE / 2.0)); // the second line's start
    type_text(&rule, "X");
    assert_eq!(value.peek(), "abc\nXdefgh\nij");
    assert_eq!(p.last_shown().map(|o| o.single_line), Some(false));
    rule.on_node_with_tag("single").perform_click();
    rule.wait_for_idle();
    assert!(is_focused(&rule, "single"));
    assert_eq!(p.last_shown().map(|o| o.single_line), Some(true));
}

/// C++ `text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one` (C++ 25487), its Material geometry: a
/// toolbar provided through LocalTextToolbar is told where a long press's selection is in a single-line TextField —
/// "güzel" eight characters past the 16 dp inset, five wide, from the line's top — and what it can do, not again
/// while nothing changes; copy hides it; select all tells it again from the inset; a press elsewhere leaves it, a
/// tap in the field hides it; made read-only while shown it is told again, with copy and select all only. (Its
/// foundation half — the same on a BasicTextField, a SelectionContainer's in selection.rs —: kompoz-foundation
/// text_field_selection.rs::text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one.)
///
/// Compose differs from the C++: a press elsewhere (on nothing focusable) leaves it shown (the C++ hid it); made
/// read-only while shown the String field (legacy) tells it again as the C++ did (the state-based field of the
/// foundation half doesn't); the String field's toolbar is told again each time the field is placed or its
/// handles recompose while shown (Compose's legacy CoreTextField: `showSelectionToolbar()` from
/// `onGloballyPositioned` and `SelectionToolbarAndHandles`), so the C++'s exact counts are counts that grow or stay;
/// its rect reaches 25 dp below the line, over the handles (the C++ only checked it within the field); 9 px
/// characters (the C++'s 8.5).
fn text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one() {
    let value = mutable_state_of("merhaba güzel dünya".to_string());
    let toolbar = Rc::new(RecordingToolbar::default());
    let locked = mutable_state_of(false);
    let rule = field_under_a_box(value.clone(), toolbar.clone(), locked.clone());
    let p = platform(&rule);
    p.set_clip("pano");
    rule.wait_for_idle();
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 10, 0)); // "güzel" selected
    let shows = toolbar.shows.get();
    assert!(shows >= 1 && toolbar.offers() == (true, true, true, true));
    let r = toolbar.rect.get();
    assert!(near(r.left, field.left + INSET + 8.0 * CHAR, 0.01) && near(r.width(), 5.0 * CHAR, 0.01), "{r:?}");
    // (from the line's top, 16 dp down, over the handles hanging below it)
    assert!(near(r.top, field.top + INSET, 0.01) && near(r.bottom, field.top + INSET + LINE + 25.0, 0.01), "{r:?}");
    rule.advance_time_by(100);
    assert_eq!(toolbar.shows.get(), shows); // not told again while nothing changes
    toolbar.choose(&rule, &toolbar.copy);
    assert!(p.clipboard() == "güzel" && !toolbar.shown());
    long_press(&rule, char_at(&rule, "field", 3, 0));
    let shows = toolbar.shows.get();
    toolbar.choose(&rule, &toolbar.select_all);
    assert!(toolbar.shows.get() > shows && near(toolbar.rect.get().left, field.left + INSET, 0.01));
    tap(&rule, bounds(&rule, "below").center()); // a press elsewhere leaves it
    assert!(toolbar.shown());
    rule.advance_time_by(500);
    tap(&rule, char_at(&rule, "field", 3, 0)); // a tap in the field hides it
    assert!(!toolbar.shown());
    rule.advance_time_by(500);
    long_press(&rule, char_at(&rule, "field", 10, 0));
    let shows = toolbar.shows.get();
    assert_eq!(toolbar.offers(), (true, true, true, true));
    // Made read-only while shown: told again, copy and select all only.
    locked.set(true);
    rule.wait_for_idle();
    assert!(toolbar.shows.get() > shows && toolbar.offers() == (true, false, false, true));
}
