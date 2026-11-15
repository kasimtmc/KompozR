//! Text selection — SelectionContainer and DisableSelection, by touch (a long press, handles), by mouse (clicks,
//! double and triple clicks, drags, shift), by keys (copy, select all, Escape), the toolbar it tells, the
//! magnifier, the highlight, the colors, what it does with right-to-left text — and the C++ Kompoz's selection
//! tests (without text fields or Material; Compose's behaviour where the two differ: ICU's word boundaries, the
//! toolbar handed to `LocalTextToolbar` rather than drawn, 25 px handles). The texts are 16 sp in the fake
//! backend: 8 px a character, 20 px lines. Numbers in px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::magnifier::MagnifierPositionInRoot;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::selection::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::{
    AnnotatedString, FontWeight, SpanStyle, TextAlign, TextDirection, TextRange, TextStyle, build_annotated_string,
};
use kompoz_ui::input::*;
use kompoz_ui::platform::{
    HapticFeedbackType, LocalTextToolbar, PlatformCallbacks, PointerIcon, TextToolbar, TextToolbarAction,
    TextToolbarStatus,
};
use kompoz_ui::semantics::*;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

/// C++'s `Text`: a [`BasicText`] in its style, 16 sp unless it says (8 px a character, 20 px lines).
#[kompozable]
fn Text(
    text: impl Into<BasicTextValue>,
    #[default(Modifier)] modifier: Modifier,
    #[default(TextStyle::Default)] style: TextStyle,
) {
    let mut style = style;
    if style.font_size.is_unspecified() {
        style.font_size = 16.sp();
    }
    BasicText(text, modifier = modifier, style = style);
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// A platform keeping its clipboard, the cursors and haptics it is told.
#[derive(Default)]
struct TestPlatform {
    clipboard: RefCell<Option<String>>,
    cursors: RefCell<Vec<PointerIcon>>,
    haptics: RefCell<Vec<HapticFeedbackType>>,
}

impl PlatformCallbacks for TestPlatform {
    fn get_clipboard(&self) -> Option<String> {
        self.clipboard.borrow().clone()
    }
    fn set_clipboard(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
    fn cursor(&self, icon: PointerIcon) {
        self.cursors.borrow_mut().push(icon);
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.haptics.borrow_mut().push(kind);
    }
}

fn platform(rule: &KompozTestRule) -> Rc<TestPlatform> {
    let p = Rc::new(TestPlatform::default());
    rule.host().set_callbacks(p.clone());
    p
}

impl TestPlatform {
    /// The clipboard's text, taken (empty if none).
    fn take(&self) -> String {
        self.clipboard.borrow_mut().take().unwrap_or_default()
    }
}

/// A text toolbar of the test's own (C++'s RecordingToolbar): what it was shown with, how often shown and hidden.
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
    /// Whether it shows copy (C++'s `onNodeWithText("Copy").exists()`).
    fn offers_copy(&self) -> bool {
        self.shown() && self.copy.borrow().is_some()
    }
    fn offers_select_all(&self) -> bool {
        self.shown() && self.select_all.borrow().is_some()
    }
    /// Chooses copy (C++'s `onNodeWithText("Copy").performClick()`).
    fn copy(&self, rule: &KompozTestRule) {
        let action = self.copy.borrow().clone().expect("the toolbar offers copy");
        action();
        rule.wait_for_idle();
    }
    fn select_all(&self, rule: &KompozTestRule) {
        let action = self.select_all.borrow().clone().expect("the toolbar offers select all");
        action();
        rule.wait_for_idle();
    }
}

/// Where a character of a tagged text is: `index` characters in, 2 px into it, on line `line`'s middle.
fn char_at(rule: &KompozTestRule, tag: &str, index: usize, line: usize) -> Offset {
    let r = rule.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root();
    at(r.left + 8.0 * index as f32 + 2.0, r.top + 10.0 + 20.0 * line as f32)
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root()
}

fn long_press(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_time_by(500);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

/// A drag from `from` to `to` in six moves, a frame each (C++'s `drag`).
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

/// Ctrl+C: what it copied (empty if nothing).
fn copy_keys(rule: &KompozTestRule, p: &TestPlatform) -> String {
    p.take();
    rule.host().key_down(Key::C, KeyModifiers::Ctrl);
    rule.host().key_up(Key::C, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    p.take()
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

/// `content` with `toolbar` as its LocalTextToolbar.
#[kompozable(inline)]
fn WithToolbar(toolbar: Rc<RecordingToolbar>, content: impl FnOnce()) {
    let t: Rc<dyn TextToolbar> = toolbar;
    KompozitionLocalProvider(&[LocalTextToolbar.provides_unequal(t)], content);
}

// ---------------------------------------------------------------------------------------------------------------
// The C++ Kompoz's tests

fn selection_container_selects_and_copies_text() {
    // C++ 1326 (selection_container_selects_and_copies_text). The toolbar is the one LocalTextToolbar provides, as
    // Compose's (C++ drew its own with "Copy" and "Select all" buttons: here its copy and select all actions are
    // chosen). Words are ICU's (D11).
    let toolbar = Rc::new(RecordingToolbar::default());
    let t = toolbar.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            WithToolbar(t.clone(), || {
                Column(|| {
                    Box(modifier = Modifier.height(200.dp()));
                    SelectionContainer(
                        modifier = Modifier.fill_max_width().test_tag("container"),
                        content = || {
                            Column(|| {
                                Text("merhaba güzel dünya", modifier = Modifier.test_tag("first"));
                                Text("ikinci satır burada", modifier = Modifier.test_tag("second"));
                                DisableSelection(|| {
                                    Text("seçilmeyen etiket", modifier = Modifier.test_tag("disabled"));
                                });
                                Box(modifier = Modifier.fill_max_width().height(100.dp()).test_tag("space"));
                            });
                        },
                    );
                    Text("dışarıda kalan", modifier = Modifier.test_tag("outside"));
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    long_press(&rule, char_at(&rule, "first", 10, 0));
    assert!(toolbar.offers_copy() && toolbar.offers_select_all());
    toolbar.copy(&rule);
    assert_eq!(p.take(), "güzel");
    assert!(!toolbar.shown());
    // One selection: a word in another text takes the place of the first one; select all takes every selectable
    // text, a line each.
    long_press(&rule, char_at(&rule, "first", 2, 0));
    long_press(&rule, char_at(&rule, "second", 2, 0));
    assert!(toolbar.offers_copy());
    toolbar.select_all(&rule);
    assert!(!toolbar.offers_select_all()); // all of it is selected
    toolbar.copy(&rule);
    assert_eq!(p.take(), "merhaba güzel dünya\nikinci satır burada");
    assert!(!toolbar.shown());
    // The end handle under "merhaba", dragged six characters on and a line down: into the second text. (A handle's
    // drag starts past the touch slop, which it doesn't take: the end lands 8 px short, before "satır"'s end;
    // across into another text it snaps to words.)
    long_press(&rule, char_at(&rule, "first", 3, 0));
    let first = bounds(&rule, "first");
    let handle = at(first.left + 8.0 * 7.0 + 11.0, first.top + 20.0 + 11.0);
    drag(&rule, handle, at(handle.x + 48.0, handle.y + 20.0), PointerType::Touch);
    toolbar.copy(&rule);
    assert_eq!(p.take(), "merhaba güzel dünya\nikinci satır");
    // ...and back up: the selection shrinks into the first text again. (The end moves from its handle's point, the
    // line's bottom, less the slop — C++ held the handle by its line's middle: a line and a half up here.)
    long_press(&rule, char_at(&rule, "first", 3, 0));
    drag(&rule, handle, at(handle.x + 48.0, handle.y + 20.0), PointerType::Touch);
    drag(&rule, at(handle.x + 48.0, handle.y + 20.0), at(handle.x + 48.0, handle.y - 10.0), PointerType::Touch);
    toolbar.copy(&rule);
    assert_eq!(p.take(), "merhaba güzel");
    // The start handle can't pass the end one: dragged past it, they cross.
    long_press(&rule, char_at(&rule, "second", 9, 0));
    let second = bounds(&rule, "second");
    drag(
        &rule,
        at(second.left + 8.0 * 7.0 - 11.0, second.top + 20.0 + 11.0),
        at(second.left + 8.0 * 18.0 - 11.0, second.top + 20.0 + 11.0),
        PointerType::Touch,
    );
    toolbar.copy(&rule);
    assert_eq!(p.take(), " burada");
    // A press elsewhere in the container clears it; text outside, or in DisableSelection, isn't selectable.
    long_press(&rule, char_at(&rule, "second", 2, 0));
    tap(&rule, bounds(&rule, "space").center());
    assert!(!toolbar.shown());
    long_press(&rule, char_at(&rule, "outside", 2, 0));
    assert!(!toolbar.shown());
    // (pressed low in its line: a finger's touch reaches 14 px past a 20 px text, to the 48 px of a touch target,
    // as Compose's hit testing does — the second text's would take a press near its bottom)
    long_press(&rule, char_at(&rule, "disabled", 2, 0) + at(0.0, 8.0));
    assert!(!toolbar.shown());
    // The mouse: a drag selects from text to text, without handles or toolbar; Ctrl+C copies.
    drag(&rule, char_at(&rule, "first", 8, 0), char_at(&rule, "second", 6, 0), PointerType::Mouse);
    assert!(!toolbar.shown());
    assert_eq!(copy_keys(&rule, &p), "güzel dünya\nikinci");
    // Below the last text (the label in DisableSelection doesn't count): to its end.
    let space = bounds(&rule, "space");
    drag(&rule, char_at(&rule, "first", 0, 0), at(space.left + 2.0, space.top + 10.0), PointerType::Mouse);
    assert_eq!(copy_keys(&rule, &p), "merhaba güzel dünya\nikinci satır burada");
    // ...dragged backwards too; a double click takes a word; Escape clears it.
    drag(&rule, char_at(&rule, "second", 6, 0), char_at(&rule, "first", 8, 0), PointerType::Mouse);
    assert_eq!(copy_keys(&rule, &p), "güzel dünya\nikinci");
    for _ in 0..2 {
        rule.host().pointer_down(char_at(&rule, "first", 15, 0), -1, 0, PointerType::Mouse);
        rule.host().pointer_up(char_at(&rule, "first", 15, 0), -1, 0);
        rule.advance_time_by(50);
    }
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "dünya");
    rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "");
    // A click clears it too; the mouse's long press selects nothing.
    drag(&rule, char_at(&rule, "first", 0, 0), char_at(&rule, "first", 7, 0), PointerType::Mouse);
    assert_eq!(copy_keys(&rule, &p), "merhaba");
    rule.host().pointer_down(char_at(&rule, "second", 3, 0), -1, 0, PointerType::Mouse);
    rule.advance_time_by(600);
    rule.host().pointer_up(char_at(&rule, "second", 3, 0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "");
    assert!(!toolbar.shown());
}

fn text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one() {
    // C++ 25310's SelectionContainer part (its text field part is stage 10's): a toolbar provided through
    // LocalTextToolbar is told where the selection is and what it can do — copy and select all — once while
    // nothing changes, again when it does, and hidden when the selection goes. (Compose's toolbar rect is the
    // selection's bounding box in the root, made four handle heights taller: room for the handles under it.)
    let toolbar = Rc::new(RecordingToolbar::default());
    let t = toolbar.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            WithToolbar(t.clone(), || {
                Column(
                    modifier = Modifier.fill_max_size(),
                    content = || {
                        Box(modifier = Modifier.height(200.dp()));
                        SelectionContainer(
                            modifier = Modifier.test_tag("seçilen"),
                            content = || {
                                Column(|| {
                                    Text("Seçilebilir metin", modifier = Modifier.test_tag("metin"));
                                    Box(modifier = Modifier.fill_max_width().height(100.dp()).test_tag("below"));
                                });
                            },
                        );
                        Box(modifier = Modifier.fill_max_width().height(300.dp()).test_tag("outside"));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    let text = bounds(&rule, "metin");
    long_press(&rule, at(text.left + 8.0, text.top + 8.0));
    // Told when the gesture ended, and again as the text was positioned once its handles came (Compose tells it
    // whenever a selected text is positioned).
    let shows = toolbar.shows.get();
    assert!((1..=2).contains(&shows), "{shows}");
    assert!(toolbar.copy.borrow().is_some() && toolbar.paste.borrow().is_none() && toolbar.cut.borrow().is_none());
    assert!(toolbar.select_all.borrow().is_some());
    // Over "Seçilebilir": 11 characters, on the text's line.
    let r = toolbar.rect.get();
    assert!(near(r.left, text.left, 0.01) && near(r.width(), 88.0, 0.01), "{r:?}");
    assert!(near(r.top, text.top, 0.01) && near(r.bottom, text.bottom + 100.0, 0.01), "{r:?}");
    rule.wait_for_idle();
    rule.advance_time_by(100);
    assert_eq!(toolbar.shows.get(), shows); // not again while nothing changes
    toolbar.copy(&rule);
    assert_eq!(p.take(), "Seçilebilir");
    assert!(!toolbar.shown() && toolbar.hides.get() == 1);
    // Select all from it: told again where the whole text is, without select all.
    long_press(&rule, at(text.left + 8.0, text.top + 8.0));
    let shows = toolbar.shows.get();
    toolbar.select_all(&rule);
    assert!(toolbar.shows.get() > shows);
    assert!(near(toolbar.rect.get().left, text.left, 0.01) && near(toolbar.rect.get().width(), 17.0 * 8.0, 0.01));
    assert!(toolbar.select_all.borrow().is_none() && toolbar.copy.borrow().is_some());
    // A press outside the container leaves it (as Compose's: nothing there takes the focus); a press elsewhere in
    // it hides it.
    tap(&rule, bounds(&rule, "outside").center());
    assert!(toolbar.shown() && toolbar.hides.get() == 1);
    tap(&rule, bounds(&rule, "below").center());
    assert!(!toolbar.shown() && toolbar.hides.get() == 2);
}

fn shift(rule: &KompozTestRule, held: bool) {
    if held {
        rule.host().key_down(Key::ShiftLeft, KeyModifiers::Shift);
    } else {
        rule.host().key_up(Key::ShiftLeft, KeyModifiers::None);
    }
}

fn selection_container_mouse_clicks_words_paragraphs_and_shift() {
    // C++ 1265 (selection_container_mouse_clicks_words_paragraphs_and_shift), with ICU's word boundaries (D11:
    // the same words here as C++'s byte rule).
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            SelectionContainer(
                modifier = Modifier.fill_max_width(),
                content = || {
                    Column(|| {
                        Text("alpha beta gamma", modifier = Modifier.test_tag("first"));
                        Text("delta epsilon\nzeta eta", modifier = Modifier.test_tag("second"));
                    });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    let h = rule.host();
    // Two clicks, the second held and dragged on into the next text: word by word.
    rule.advance_time_by(500);
    let beta = char_at(&rule, "first", 7, 0);
    h.pointer_down(beta, -1, 0, PointerType::Mouse);
    h.pointer_up(beta, -1, 0);
    h.pointer_down(beta, -1, 0, PointerType::Mouse);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "beta");
    h.pointer_move(char_at(&rule, "second", 8, 0), -1, 0);
    h.pointer_up(char_at(&rule, "second", 8, 0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "beta gamma\ndelta epsilon");
    // Back over the anchor: from the end of "beta" to the start of "alpha".
    rule.advance_time_by(500);
    h.pointer_down(beta, -1, 0, PointerType::Mouse);
    h.pointer_up(beta, -1, 0);
    h.pointer_down(beta, -1, 0, PointerType::Mouse);
    h.pointer_move(char_at(&rule, "first", 1, 0), -1, 0);
    h.pointer_up(char_at(&rule, "first", 1, 0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "alpha beta");
    // Three clicks: the paragraph, up to the line break.
    clicks(&rule, char_at(&rule, "second", 2, 1), 3);
    assert_eq!(copy_keys(&rule, &p), "zeta eta");
    // A click, then Shift and a click: from the first to the second.
    clicks(&rule, char_at(&rule, "first", 6, 0), 1);
    shift(&rule, true);
    clicks(&rule, char_at(&rule, "second", 5, 0), 1);
    shift(&rule, false);
    assert_eq!(copy_keys(&rule, &p), "beta gamma\ndelta");
}

/// The index of the first draw of `text`.
fn drawn(canvas: &RecordingCanvas, text: &str) -> Option<usize> {
    canvas.texts.iter().position(|t| t == text)
}

/// The index of the draw of `text` on a line of `within`.
fn drawn_in(canvas: &RecordingCanvas, text: &str, within: Rect) -> Option<usize> {
    (0..canvas.texts.len())
        .find(|&i| canvas.texts[i] == text && (within.top..=within.bottom).contains(&canvas.baselines[i].y))
}

fn right_to_left_text_aligns_and_selects_from_the_right() {
    // C++ 6818 (right_to_left_text_aligns_and_selects_from_the_right): right-to-left text starts at the right, its
    // first letters rightmost; selecting goes by its logical order, highlighted where it is drawn. Words are ICU's.
    let toolbar = Rc::new(RecordingToolbar::default());
    let t = toolbar.clone();
    let direction = mutable_state_of(TextDirection::Content);
    let d = direction.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let d = d.clone();
            WithToolbar(t.clone(), move || {
                SelectionContainer(
                    modifier = Modifier.fill_max_width(),
                    content = move || {
                        let rtl_style = TextStyle { text_direction: d.get(), ..TextStyle::Default };
                        Column(move || {
                            Text(
                                "שלום עולם",
                                modifier = Modifier.fill_max_width().test_tag("rtl"),
                                style = rtl_style.clone(),
                            );
                            Text("abc אב", modifier = Modifier.test_tag("mixed"));
                            Text(
                                "left",
                                modifier = Modifier.fill_max_width().test_tag("end"),
                                style = TextStyle { text_align: TextAlign::End, ..TextStyle::Default },
                            );
                            let styled = build_annotated_string(|b| {
                                b.with_style(
                                    SpanStyle { font_weight: Some(FontWeight::Bold), ..SpanStyle::Default },
                                    |b| b.append("אב"),
                                );
                                b.append(" גד");
                            });
                            // (by its content's direction: Compose's unspecified direction is the layout's, C++'s was the
                            // content's)
                            Text(
                                styled,
                                modifier = Modifier.fill_max_width().test_tag("styled"),
                                style = TextStyle { text_direction: TextDirection::Content, ..TextStyle::Default },
                            );
                        });
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    // 8 px a character: "שלום עולם" is 72 px, at the right.
    let (rtl, mixed, end) = (bounds(&rule, "rtl"), bounds(&rule, "mixed"), bounds(&rule, "end"));
    let canvas = rule.draw();
    let hebrew = drawn(&canvas, "שלום עולם").expect("drawn");
    assert!(canvas.rtl[hebrew] && near(canvas.baselines[hebrew].x, rtl.left + 400.0 - 72.0, 0.01));
    // Styled pieces of a right-to-left line go from the right: the first one is rightmost.
    let styled = bounds(&rule, "styled");
    let (first, second) = (drawn_in(&canvas, "אב", styled).expect("drawn"), drawn(&canvas, " גד").expect("drawn"));
    assert!(near(canvas.baselines[first].x, styled.left + 384.0, 0.01), "{:?}", canvas.baselines[first]);
    assert!(near(canvas.baselines[second].x, styled.left + 360.0, 0.01), "{:?}", canvas.baselines[second]);
    let left = drawn(&canvas, "left").expect("drawn");
    assert!(!canvas.rtl[left] && near(canvas.baselines[left].x, end.left + 400.0 - 32.0, 0.01));
    let copied = |p: &TestPlatform| {
        assert!(toolbar.offers_copy());
        toolbar.copy(&rule);
        p.take()
    };
    long_press(&rule, at(rtl.left + 395.0, rtl.top + 10.0)); // the rightmost letter is the first one
    assert_eq!(copied(&p), "שלום");
    long_press(&rule, at(rtl.left + 335.0, rtl.top + 10.0));
    assert_eq!(copied(&p), "עולם");
    // Highlighted where it is drawn: "עולם" is the left part, 328..360.
    long_press(&rule, at(rtl.left + 335.0, rtl.top + 10.0));
    let highlighted = rule.draw();
    let colors = LocalTextSelectionColors.default_value();
    assert!(
        highlighted.paths.iter().zip(&highlighted.path_colors).any(|(r, c)| near(r.left, rtl.left + 328.0, 0.01)
            && near(r.width(), 32.0, 0.01)
            && *c == colors.background_color),
        "{:?}",
        highlighted.paths
    );
    toolbar.copy(&rule);
    // In a left-to-right paragraph the Hebrew word sits after "abc ", its letters reversed: 32..48.
    long_press(&rule, at(mixed.left + 44.0, mixed.top + 10.0));
    assert_eq!(copied(&p), "אב");
    long_press(&rule, at(mixed.left + 4.0, mixed.top + 10.0));
    assert_eq!(copied(&p), "abc");
    // TextDirection::Ltr keeps Hebrew text at the left (a right-to-left run in a left-to-right paragraph: UAX #9's,
    // where C++ drew it left to right).
    direction.set(TextDirection::Ltr);
    rule.wait_for_idle();
    let ltr = rule.draw();
    let i = drawn(&ltr, "שלום עולם").expect("drawn");
    assert!(near(ltr.baselines[i].x, rtl.left, 0.01) && ltr.rtl[i]);
}

/// Where the magnifier shows the content of, in the root (none while none shows).
fn magnifier_at(rule: &KompozTestRule) -> Option<Offset> {
    rule.all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(MagnifierPositionInRoot))
        .map(|p| (p.0)())
        .find(|o| o.is_specified())
}

/// The texts of a container: a 100 px box, then "merhaba güzel dünya" and "ikinci satır burada" (C++ 6685's and
/// 6749's SelectionContainer parts).
fn two_texts_under_a_box(toolbar: Rc<RecordingToolbar>) -> KompozTestRule {
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            WithToolbar(toolbar.clone(), || {
                SelectionContainer(
                    modifier = Modifier.fill_max_width(),
                    content = || {
                        Column(|| {
                            Box(modifier = Modifier.height(100.dp()));
                            Text("merhaba güzel dünya");
                            Text("ikinci satır burada");
                        });
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    )
}

fn dragging_a_handle_shows_the_text_magnifier() {
    // C++ 6685's SelectionContainer part (its text field part is stage 10's): while a finger drags a handle the
    // magnifier shows where the drag is — its x within the line, at the line's middle — and the toolbar is out of
    // the way; both go with the finger. (A handle's drag starts past the 8 px touch slop, which it doesn't move by:
    // 40 of the 48 px. The magnifier jumps between lines, without Compose's spring: animations are a later stage.)
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = two_texts_under_a_box(toolbar.clone());
    long_press(&rule, at(24.0, 110.0));
    assert!(magnifier_at(&rule).is_none()); // selecting doesn't show it
    assert!(toolbar.shown());
    let end = at(56.0 + 11.0, 120.0 + 11.0); // the end handle under the end of "merhaba"
    rule.host().pointer_down(end, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        rule.advance_frame();
        rule.host().pointer_move(at(end.x + 8.0 * i as f32, end.y), -1, 0);
    }
    rule.wait_for_idle();
    let shown = magnifier_at(&rule).expect("the magnifier");
    assert!(near(shown.x, 56.0 + 40.0, 0.01) && near(shown.y, 110.0, 0.01), "{shown:?}");
    assert!(!toolbar.shown());
    // Zoomed in a layer above the source.
    let canvas = rule.draw();
    assert!(canvas.layer_transforms.iter().any(|t| near(t.scale_x, 1.25, 0.01) && near(t.translation_y, -42.0, 0.01)));
    rule.host().pointer_up(at(end.x + 48.0, end.y), -1, 0);
    rule.wait_for_idle();
    assert!(magnifier_at(&rule).is_none() && toolbar.shown()); // gone with the finger, the toolbar back
    // Dragged far from the line's text (more than half the magnifier's width past its end): hidden.
    let end_now = at(152.0 + 11.0, 120.0 + 11.0);
    rule.host().pointer_down(end_now, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        rule.advance_frame();
        rule.host().pointer_move(at(end_now.x + 40.0 * i as f32, end_now.y), -1, 0);
    }
    rule.wait_for_idle();
    assert!(magnifier_at(&rule).is_none());
    rule.host().pointer_up(at(end_now.x + 240.0, end_now.y), -1, 0);
    rule.wait_for_idle();
}

fn selection_handles_point_at_their_end_and_hold_its_line_by_the_middle() {
    // C++ 6749's SelectionContainer part (its text field part is stage 10's): each handle's point is a square at
    // its top, towards the text — the start's on its right, at the selection's start; the end's on its left. A
    // handle dragged moves its end from where it points (the line's bottom, a pixel up), less the touch slop:
    // dragged up 30 px the start goes up a line, into the first text, by word there (C++ held the handle by its
    // line's middle and went up a line at 15 px).
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = two_texts_under_a_box(toolbar.clone());
    let p = platform(&rule);
    long_press(&rule, at(8.0 * 9.0 + 2.0, 130.0)); // "satır"
    let colors = LocalTextSelectionColors.default_value();
    let canvas = rule.draw();
    let mut points: Vec<Rect> = (0..canvas.rects.len())
        .filter(|&i| near(canvas.rects[i].width(), 12.5, 0.01) && near(canvas.rects[i].height(), 12.5, 0.01))
        .filter(|&i| canvas.rect_colors[i] == colors.handle_color)
        .map(|i| canvas.rects[i])
        .collect();
    points.sort_by(|a, b| a.left.total_cmp(&b.left));
    assert_eq!(points.len(), 2, "{points:?}");
    assert!(near(points[0].right, 56.0, 0.01) && near(points[0].top, 140.0, 0.01), "{points:?}");
    assert!(near(points[1].left, 96.0, 0.01) && near(points[1].top, 140.0, 0.01), "{points:?}");
    let start = at(56.0 - 12.5, 140.0 + 12.5); // the start handle, in its middle
    rule.host().pointer_down(start, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        rule.advance_frame();
        rule.host().pointer_move(at(start.x, start.y - 5.0 * i as f32), -1, 0);
    }
    rule.host().pointer_up(at(start.x, start.y - 30.0), -1, 0);
    rule.wait_for_idle();
    toolbar.copy(&rule);
    assert_eq!(p.take(), "merhaba güzel dünya\nikinci satır");
}

fn text_selection_colors_come_from_the_local() {
    // C++ 11568's local part (its MaterialTheme part is stage 12's): the highlight and the handles take
    // LocalTextSelectionColors' — Compose's blue by default, an app's own when provided.
    let own = TextSelectionColors::new(Color::new(1.0, 0.0, 0.0, 1.0), Color::new(0.0, 1.0, 0.0, 0.5));
    let pick = mutable_state_of(false);
    let pk = pick.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let inside = || {
            SelectionContainer(
                content = || {
                    Text("seçilecek metin", modifier = Modifier.test_tag("metin"));
                },
            );
        };
        if pk.get() {
            KompozitionLocalProvider(&[LocalTextSelectionColors.provides(own)], inside);
        } else {
            inside();
        }
    }));
    let highlighted = |colors: TextSelectionColors| {
        let t = bounds(&rule, "metin");
        long_press(&rule, at(t.left + 2.0, t.center().y)); // a long press: the word
        let canvas = rule.draw();
        canvas.path_colors.contains(&colors.background_color) && canvas.rect_colors.contains(&colors.handle_color)
    };
    let default = TextSelectionColors::default_colors();
    assert_eq!(default.handle_color, Color::from_argb(0xFF4286F4));
    assert!(highlighted(default));
    pick.set(true);
    rule.wait_for_idle();
    assert!(highlighted(own));
}

fn pointer_hover_icon_over_selectable_text_is_the_text_cursor() {
    // C++ 26856's text part: over a selectable text the mouse shows the text cursor; over a text that isn't
    // selectable, and over the rest of a selection container, the default one.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Text("düz metin", modifier = Modifier.test_tag("metin"));
            SelectionContainer(
                content = || {
                    Column(|| {
                        Text("seçilir metin", modifier = Modifier.test_tag("seçilir"));
                        Box(modifier = Modifier.size(40.dp()).test_tag("boşluk"));
                    });
                },
            );
        });
    }));
    let p = platform(&rule);
    let over = |tag: &str| {
        rule.host().pointer_move(bounds(&rule, tag).center(), -1, 0);
        rule.wait_for_idle();
        p.cursors.borrow().last().copied().unwrap_or_default()
    };
    assert_eq!(over("seçilir"), PointerIcon::Text);
    assert_eq!(over("metin"), PointerIcon::Default);
    assert_eq!(over("seçilir"), PointerIcon::Text);
    assert_eq!(over("boşluk"), PointerIcon::Default);
}

// ---------------------------------------------------------------------------------------------------------------
// Adjustments, on one text's layout

/// `text` laid out by the fake backend at 16 sp in 400 px.
fn laid_out(text: &str) -> kompoz_text::TextLayoutResult {
    let resolver = kompoz_ui::platform::fake_resolver();
    let measurer = kompoz_text::TextMeasurer::new(resolver.clone(), Density::default(), LayoutDirection::Ltr, 1);
    measurer.measure_with(
        text,
        TextStyle { font_size: 16.sp(), ..TextStyle::Default },
        kompoz_text::TextOverflow::Clip,
        true,
        usize::MAX,
        Vec::new(),
        Constraints::new(0, 400, 0, Constraints::Infinity),
        LayoutDirection::Ltr,
        Density::default(),
        resolver,
        false,
    )
}

/// The selection `adjustment` makes of a gesture on one text from `start` to `end` (the end moving unless
/// `is_start_handle`), after `previous` (none: a new selection), the moving end having been at `previous_offset`.
fn adjusted(
    layout: &kompoz_text::TextLayoutResult,
    adjustment: SelectionAdjustment,
    (start, end): (usize, usize),
    previous: Option<(usize, usize)>,
    previous_offset: Option<usize>,
    is_start_handle: bool,
) -> (usize, usize, bool) {
    let l = get_text_field_selection_layout(
        layout,
        start,
        end,
        previous_offset,
        previous.map_or(TextRange::Zero, |(s, e)| TextRange::new(s, e)),
        previous.is_none(),
        is_start_handle,
    );
    let s = adjustment.adjust(&l);
    (s.start.offset, s.end.offset, s.handles_crossed)
}

fn selection_adjustments_snap_to_characters_words_and_paragraphs() {
    // Compose's SelectionAdjustment: None as it is; Word out to ICU's word boundaries around each end (crossed if
    // the gesture was); Paragraph to the line feeds; Character at least a character once a selection started;
    // CharacterWithWordAccelerate by word growing from a boundary, by character shrinking.
    let layout = laid_out("hello world foo");
    use SelectionAdjustment::*;
    let new = |a, range| adjusted(&layout, a, range, Option::None, Option::None, false);
    assert_eq!(new(None, (2, 8)), (2, 8, false));
    assert_eq!(new(Word, (2, 8)), (0, 11, false));
    assert_eq!(new(Word, (8, 2)), (11, 0, true));
    assert_eq!(new(Word, (13, 13)), (12, 15, false));
    let paragraphs = laid_out("ab cd\nef gh");
    assert_eq!(adjusted(&paragraphs, Paragraph, (1, 1), Option::None, Option::None, false), (0, 5, false));
    assert_eq!(adjusted(&paragraphs, Paragraph, (7, 10), Option::None, Option::None, false), (6, 11, false));
    // Character: a new selection may be collapsed; a collapsed one after a start takes a character.
    assert_eq!(new(Character, (3, 3)), (3, 3, false));
    assert_eq!(adjusted(&layout, Character, (3, 3), Some((3, 5)), Some(5), false), (3, 4, false));
    assert_eq!(adjusted(&layout, Character, (0, 0), Some((0, 2)), Some(2), false), (0, 1, false));
    assert_eq!(adjusted(&layout, Character, (15, 15), Some((15, 13)), Some(13), false), (15, 14, true));
    // Accelerated: the end out of "hello" (a word boundary) into "world" snaps to its end; back in, by character.
    let accelerated = CharacterWithWordAccelerate;
    assert_eq!(adjusted(&layout, accelerated, (0, 7), Some((0, 5)), Some(5), false), (0, 11, false));
    assert_eq!(adjusted(&layout, accelerated, (0, 9), Some((0, 11)), Some(11), false), (0, 9, false));
    // ...and out again from inside a word (not a boundary): by character.
    assert_eq!(adjusted(&layout, accelerated, (0, 10), Some((0, 9)), Some(9), false), (0, 10, false));
    // A grapheme cluster is a character: "é" as e and a combining accent.
    let accent = laid_out("e\u{301}x");
    assert_eq!(adjusted(&accent, Character, (0, 0), Some((0, 3)), Some(3), false), (0, 3, false));
}

fn selection_layouts_slot_ends_between_and_on_texts() {
    // Compose's SelectionLayout pieces: the direction of a position is its y's, or its x's on the text's rows;
    // a selection mode tells which selectables a selection takes in; texts in a row are ordered by x; a selection
    // across texts is cut into each text's part; parts merge back.
    assert_eq!(resolve_2d_direction(Direction::After, Direction::On), Direction::After);
    assert_eq!(resolve_2d_direction(Direction::After, Direction::Before), Direction::Before);
    let row = Rect::new(0.0, 20.0, 100.0, 40.0);
    assert!(SelectionMode::Vertical.is_selected(row, at(5.0, 5.0), at(5.0, 60.0)));
    assert!(!SelectionMode::Vertical.is_selected(row, at(5.0, 50.0), at(5.0, 60.0)));
    assert!(SelectionMode::Horizontal.is_selected(Rect::new(20.0, 0.0, 40.0, 100.0), at(5.0, 5.0), at(60.0, 5.0)));
    assert!(in_a_row(at(0.0, 0.0), at(50.0, 20.0), at(60.0, 2.0), at(100.0, 22.0)));
    assert!(!in_a_row(at(0.0, 0.0), at(50.0, 20.0), at(0.0, 20.0), at(50.0, 40.0)));
    let anchor = |offset, id| AnchorInfo::new(kompoz_text::ResolvedTextDirection::Ltr, offset, id);
    let ids = [1u64, 2, 3];
    let selection = Selection::new(anchor(2, 1), anchor(3, 3), false);
    let lengths = |id: u64| [5usize, 6, 7][id as usize - 1];
    let parts = create_sub_selections(
        &selection,
        &ids,
        |id| *id,
        |id, _| Some(Selection::new(anchor(0, *id), anchor(lengths(*id), *id), false)),
        |id, is_start, offset, _| {
            Some(if is_start {
                Selection::new(anchor(offset, *id), anchor(lengths(*id), *id), false)
            } else {
                Selection::new(anchor(0, *id), anchor(offset, *id), false)
            })
        },
    );
    let part = |id| parts.get(&id).map(|s| (s.start.offset, s.end.offset));
    assert_eq!((part(1), part(2), part(3)), (Some((2, 5)), Some((0, 6)), Some((0, 3))));
    let first = Selection::new(anchor(2, 1), anchor(5, 1), false);
    let second = Selection::new(anchor(0, 2), anchor(4, 2), false);
    assert!(merge(Some(&first), Some(&second)).is_some_and(|m| m.start == anchor(2, 1) && m.end == anchor(4, 2)));
}

// ---------------------------------------------------------------------------------------------------------------
// The container

/// Three texts in a container, with `toolbar` and `state` (none: a container of its own).
fn three_texts(toolbar: Rc<RecordingToolbar>, state: Option<SelectionState>) -> KompozTestRule {
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = state.clone();
            WithToolbar(toolbar.clone(), move || {
                let content = || {
                    Column(|| {
                        Text("bir iki", modifier = Modifier.test_tag("1"));
                        Text("üç dört", modifier = Modifier.test_tag("2"));
                        Text("beş altı", modifier = Modifier.test_tag("3"));
                    });
                };
                let modifier = Modifier.fill_max_width().test_tag("container");
                match &state {
                    Some(s) => SelectionContainerWithState(state = s.clone(), modifier = modifier, content = content),
                    None => SelectionContainer(modifier = modifier, content = content),
                }
            });
        }),
        IntSize::new(400, 800),
    )
}

fn texts(list: Vec<AnnotatedString>) -> Vec<String> {
    list.iter().map(|t| t.text().to_string()).collect()
}

fn a_selection_runs_across_several_texts() {
    // One selection across three texts: a long press in the first, a mouse drag from it into the third; copied a
    // line each, the middle one whole; dragged back up, it shrinks a text at a time. The state tells the selected
    // texts; a range of the texts taken end to end is selected from code; select all, extend by a word and clear
    // too.
    let toolbar = Rc::new(RecordingToolbar::default());
    let state = SelectionState::new();
    let rule = three_texts(toolbar.clone(), Some(state.clone()));
    let p = platform(&rule);
    long_press(&rule, char_at(&rule, "1", 5, 0)); // "iki"
    assert_eq!(texts(state.selected_texts()), ["iki"]);
    toolbar.copy(&rule);
    assert_eq!(p.take(), "iki");
    assert!(state.selected_texts().is_empty() && state.selection().is_none());
    let (from, to) = (char_at(&rule, "1", 4, 0) - at(2.0, 0.0), char_at(&rule, "3", 3, 0) - at(2.0, 0.0));
    rule.host().pointer_down(from, -1, 0, PointerType::Mouse);
    rule.host().pointer_move(to, -1, 0);
    rule.wait_for_idle();
    assert_eq!(texts(state.selected_texts()), ["iki", "üç dört", "beş"]);
    rule.host().pointer_move(char_at(&rule, "2", 2, 0) - at(2.0, 0.0), -1, 0);
    rule.host().pointer_up(char_at(&rule, "2", 2, 0) - at(2.0, 0.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(texts(state.selected_texts()), ["iki", "üç"]);
    assert_eq!(copy_keys(&rule, &p), "iki\nüç");
    assert_eq!(texts(state.get_selectable_texts()), ["bir iki", "üç dört", "beş altı"]);
    // From code: bytes 4..13 of "bir ikiüç dörtbeş altı" ("ü" and "ç" two bytes each).
    state.select(TextRange::new(4, 13));
    rule.wait_for_idle();
    assert_eq!(texts(state.selected_texts()), ["iki", "üç d"]);
    assert_eq!(copy_keys(&rule, &p), "iki\nüç d");
    state.extend_selection_by_word();
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "iki\nüç dört");
    state.extend_selection_by_word(); // on into the next text
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "iki\nüç dört\nbeş");
    state.select_all();
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "bir iki\nüç dört\nbeş altı");
    state.clear();
    rule.wait_for_idle();
    assert!(state.selection().is_none() && copy_keys(&rule, &p).is_empty());
    state.extend_selection_by_word(); // none: the first word
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "bir");
}

fn keys_and_accessibility_copy_and_select_all() {
    // Ctrl+A selects all the container's texts, Ctrl+C (and Ctrl+Insert) copies them; Escape clears. Accessibility
    // copies and sets the selection (a range of its texts end to end) through the container's semantics.
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = three_texts(toolbar.clone(), None);
    let p = platform(&rule);
    clicks(&rule, char_at(&rule, "1", 1, 0), 1); // a click focuses the container (a collapsed selection)
    rule.host().key_down(Key::A, KeyModifiers::Ctrl);
    rule.host().key_up(Key::A, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "bir iki\nüç dört\nbeş altı");
    rule.host().key_down(Key::Insert, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    assert_eq!(p.take(), "bir iki\nüç dört\nbeş altı");
    rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "");
    let container = rule.on_node_with_tag("container").fetch_semantics_node().id();
    let select = SemanticsAction::SetSelection { start: 4, end: 13, relative_to_original_text: false };
    assert!(rule.host().perform_semantics_action(container, select));
    rule.wait_for_idle();
    p.take();
    assert!(rule.host().perform_semantics_action(container, SemanticsAction::CopyText));
    assert_eq!(p.take(), "iki\nüç d");
}

fn a_controlled_container_shows_the_selection_it_is_given() {
    // SelectionContainerWithSelection: the app holds the selection — told each one gestures come to, it shows
    // the one it gives back; it clears it by giving none.
    let selection: MutableState<Option<Selection>> = mutable_state_of(None);
    let told = Rc::new(Cell::new(0));
    let (sel, t) = (selection.clone(), told.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sel, t) = (sel.clone(), t.clone());
        let current = sel.get();
        SelectionContainerWithSelection(
            selection = current,
            on_selection_change = move |s| {
                t.set(t.get() + 1);
                sel.set(s);
            },
            content = || {
                Text("seçilen söz", modifier = Modifier.test_tag("metin"));
            },
        );
    }));
    let p = platform(&rule);
    long_press(&rule, char_at(&rule, "metin", 8, 0));
    let s = selection.peek().expect("a selection");
    assert!(told.get() >= 1 && s.start.offset == 9 && s.end.offset == 13, "{s:?}");
    assert_eq!(copy_keys(&rule, &p), "söz");
    selection.set(None);
    rule.wait_for_idle();
    assert!(rule.draw().path_colors.is_empty()); // no highlight
}

fn right_to_left_handles_point_from_the_right() {
    // In right-to-left text a selection's start is at its right: the start handle points there from the right
    // (anchored at its right, its point at its left), the end one from the left; the highlight is where the word
    // is drawn.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        SelectionContainer(
            modifier = Modifier.fill_max_width(),
            content = || {
                Text(
                    "שלום עולם",
                    modifier = Modifier.fill_max_width().test_tag("rtl"),
                    style = TextStyle { text_direction: TextDirection::Content, ..TextStyle::Default },
                );
            },
        );
    }));
    long_press(&rule, at(395.0, 10.0)); // "שלום": 368..400
    let handles: Vec<SelectionHandleInfo> =
        rule.all_nodes(true).iter().filter_map(|n| n.config().get(SelectionHandleInfoKey)).collect();
    let find = |h| handles.iter().find(|i| i.handle == h).copied().expect("a handle");
    let start = find(kompoz_foundation::text::Handle::SelectionStart);
    let end = find(kompoz_foundation::text::Handle::SelectionEnd);
    assert!(start.visible && end.visible);
    assert!(near(start.position.x, 400.0, 0.01) && near(start.position.y, 20.0, 0.01), "{start:?}");
    assert!(near(end.position.x, 368.0, 0.01), "{end:?}");
    assert_eq!((start.anchor, end.anchor), (SelectionHandleAnchor::Right, SelectionHandleAnchor::Left));
    let canvas = rule.draw();
    let highlight =
        |c: &RecordingCanvas, l: f32, r: f32| c.paths.iter().any(|p| near(p.left, l, 0.01) && near(p.right, r, 0.01));
    assert!(highlight(&canvas, 368.0, 400.0), "{:?}", canvas.paths);
    // Each handle's point square: the start's at its left (towards the text on its left), the end's at its right.
    let colors = LocalTextSelectionColors.default_value();
    let mut points: Vec<Rect> = (0..canvas.rects.len())
        .filter(|&i| near(canvas.rects[i].width(), 12.5, 0.01) && canvas.rect_colors[i] == colors.handle_color)
        .map(|i| canvas.rects[i])
        .collect();
    points.sort_by(|a, b| a.left.total_cmp(&b.left));
    assert!(points.len() == 2 && near(points[0].right, 368.0, 0.01) && near(points[1].left, 400.0, 0.01), "{points:?}");
    // Mixed text: the Hebrew word of a left-to-right paragraph drawn reversed, selected where it is (32..48).
    let mixed = KompozTestRule::new(kompozable_lambda!(|| {
        SelectionContainer(content = || Text("abc אב def"));
    }));
    long_press(&mixed, at(44.0, 10.0));
    assert!(highlight(&mixed.draw(), 32.0, 48.0));
}

fn touch_selection_haptics_double_tap_and_changes() {
    // A finger's long press selects with a haptic tick; a double tap selects a word at once; the text changing
    // drops the selection; another focusable taking the focus clears it.
    let text = mutable_state_of("ilk kelime".to_string());
    let tx = text.clone();
    let requester = kompoz_ui::focus::FocusRequester::new();
    let r = requester.clone();
    let toolbar = Rc::new(RecordingToolbar::default());
    let t = toolbar.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (tx, r) = (tx.clone(), r.clone());
        WithToolbar(t.clone(), move || {
            Column(move || {
                let tx = tx.clone();
                SelectionContainer(
                    content = move || {
                        Text(tx.get(), modifier = Modifier.test_tag("metin"));
                    },
                );
                Box(modifier = Modifier.size(40.dp()).focus_requester(&r).focusable());
            });
        });
    }));
    let p = platform(&rule);
    long_press(&rule, char_at(&rule, "metin", 6, 0));
    assert!(p.haptics.borrow().contains(&HapticFeedbackType::TextHandleMove));
    assert!(toolbar.shown());
    toolbar.copy(&rule);
    assert_eq!(p.take(), "kelime");
    // A double tap: the word at once.
    rule.advance_time_by(500);
    let ilk = char_at(&rule, "metin", 1, 0);
    rule.host().pointer_down(ilk, -1, 0, PointerType::Touch);
    rule.host().pointer_up(ilk, -1, 0);
    rule.advance_time_by(50);
    rule.host().pointer_down(ilk, -1, 0, PointerType::Touch);
    rule.host().pointer_up(ilk, -1, 0);
    rule.wait_for_idle();
    assert!(toolbar.offers_copy());
    toolbar.copy(&rule);
    assert_eq!(p.take(), "ilk");
    long_press(&rule, char_at(&rule, "metin", 6, 0));
    assert!(toolbar.shown());
    text.set("başka metin".to_string());
    rule.wait_for_idle();
    assert!(!toolbar.shown() && copy_keys(&rule, &p).is_empty());
    long_press(&rule, char_at(&rule, "metin", 7, 0));
    assert!(toolbar.shown());
    requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
    rule.wait_for_idle();
    assert!(!toolbar.shown());
    assert!(rule.all_nodes(true).iter().all(|n| n.config().get(SelectionHandleInfoKey).is_none()));
}

fn the_mouse_selects_from_between_texts() {
    // A mouse's press in the container's space between texts selects from there (Compose's selection between
    // texts): from the start of the text below it.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        SelectionContainer(
            modifier = Modifier.fill_max_width(),
            content = || {
                Column(|| {
                    Text("üst satır", modifier = Modifier.test_tag("üst"));
                    Box(modifier = Modifier.height(40.dp()));
                    Text("alt satır", modifier = Modifier.test_tag("alt"));
                });
            },
        );
    }));
    let p = platform(&rule);
    drag(&rule, at(200.0, 40.0), char_at(&rule, "alt", 3, 0), PointerType::Mouse);
    assert_eq!(copy_keys(&rule, &p), "alt");
    // A shift click from there extends it.
    shift(&rule, true);
    clicks(&rule, char_at(&rule, "alt", 6, 0), 1);
    shift(&rule, false);
    assert_eq!(copy_keys(&rule, &p), "alt sa");
}

/// A container shown when `shown` says (the same content for both hosts: its saved values are found by where they
/// are composed).
fn shown_container(shown: &MutableState<bool>) -> impl Content + 'static {
    let s = shown.clone();
    kompozable_lambda!(move || {
        if s.get() {
            SelectionContainer(
                content = || {
                    Column(|| {
                        Text("kalıcı seçim", modifier = Modifier.test_tag("1"));
                        Text("ikinci metin", modifier = Modifier.test_tag("2"));
                    });
                },
            );
        }
    })
}

fn a_selection_outlives_the_process() {
    // The container's state and registrar are saved (Compose's SelectionState and SelectionRegistrarImpl savers,
    // a selected text's id with them): a host made from the saved state shows the same selection, its parts worked
    // out once the container is laid out.
    let bytes = {
        let shown = mutable_state_of(true);
        let rule = KompozTestRule::new(shown_container(&shown));
        let p = platform(&rule);
        long_press(&rule, char_at(&rule, "2", 9, 0)); // "metin"
        assert_eq!(copy_keys(&rule, &p), "metin");
        rule.host().save_state()
    };
    let shown = mutable_state_of(false);
    let rule = KompozTestRule::new(shown_container(&shown));
    let p = platform(&rule);
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    assert_eq!(copy_keys(&rule, &p), "metin");
    assert!(rule.draw().paths.iter().any(|r| near(r.left, 56.0, 0.01) && near(r.right, 96.0, 0.01)));
}

tests!(
    a_selection_outlives_the_process,
    selection_adjustments_snap_to_characters_words_and_paragraphs,
    selection_layouts_slot_ends_between_and_on_texts,
    a_selection_runs_across_several_texts,
    keys_and_accessibility_copy_and_select_all,
    a_controlled_container_shows_the_selection_it_is_given,
    right_to_left_handles_point_from_the_right,
    touch_selection_haptics_double_tap_and_changes,
    the_mouse_selects_from_between_texts,
    dragging_a_handle_shows_the_text_magnifier,
    selection_handles_point_at_their_end_and_hold_its_line_by_the_middle,
    text_selection_colors_come_from_the_local,
    pointer_hover_icon_over_selectable_text_is_the_text_cursor,
    selection_container_selects_and_copies_text,
    right_to_left_text_aligns_and_selects_from_the_right,
    text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one,
    selection_container_mouse_clicks_words_paragraphs_and_shift,
);
