//! Material's time pickers as Compose's material3 (1.5.0-alpha17) has them, with Compose for desktop's actuals: the
//! C++ Kompoz's time picker tests by name (kompoz_tests.cpp), Compose's behaviour (the state and what is typed, 12-
//! and 24-hour clocks, the input's validity and errors, the colors, the layouts for the window, the dial's keys,
//! semantics and descriptions, the texts of the platform's language, the dialog and its defaults), and the M15
//! samples of TimePickerSamples.kt.
//!
//! The C++ pickers took their texts as options (`.hourLabel`, `.am`, …); Compose's take them from the platform's
//! language: the tests that wrote Turkish texts provide Turkish (`LocalStrings`), whose texts they assert. Where M15's
//! behaviour or values differ from the C++'s, the test asserts M15's and says so at the spot.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::{HapticFeedbackType, PlatformCallbacks};
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::strings::{LocalStrings, Strings};
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    time_picker_dial_picks_hours_then_minutes,
    time_picker_lays_out_side_by_side_in_landscape,
    time_picker_twelve_hour_clock,
    time_input_types_hours_and_minutes,
    time_picker_dialog_switches_modes_and_confirms,
    time_picker_states_hold_the_time_and_what_is_typed,
    time_picker_states_take_the_locales_clock_and_are_saved,
    time_picker_colors_come_from_the_scheme_and_copy_what_is_given,
    time_picker_layout_and_dial_size_follow_the_window,
    time_picker_parts_tell_accessibility_what_they_are,
    dial_numbers_take_keys_out_of_touch_mode,
    time_input_on_a_twelve_hour_clock_moves_on_and_flags_errors,
    time_pickers_follow_the_platforms_language_live,
    time_picker_dialog_lays_out_and_has_its_defaults,
    time_picker_sample,
    time_input_sample,
    time_picker_switchable_sample,
);

// ---- Helpers ---------------------------------------------------------------------------------------------------

type StateCell = Rc<RefCell<Option<Rc<dyn TimePickerState>>>>;

fn state_cell() -> StateCell {
    Rc::new(RefCell::new(None))
}

fn state_of(cell: &StateCell) -> Rc<dyn TimePickerState> {
    cell.borrow().clone().expect("the picker's state")
}

/// `content` in Turkish (the texts of a Turkish platform: `LocalStrings`).
#[kompozable]
fn Turkish(content: impl Fn() + 'static) {
    KompozitionLocalProvider(&[LocalStrings.provides(Strings::turkish())], content);
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn described(rule: &KompozTestRule, description: &str) -> Rect {
    rule.on_node_with_content_description(description).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

fn exists_described(rule: &KompozTestRule, description: &str) -> bool {
    rule.on_node_with_content_description(description).exists()
}

/// A dial number, by its description alone (an hour or minute box is described by what it picks and its value).
fn dial<'a>(rule: &'a KompozTestRule, description: &str) -> kompoz_test::SemanticsNodeInteraction<'a> {
    rule.on_node(kompoz_test::SemanticsMatcher::expect_value(
        SemanticsProperties::ContentDescription,
        vec![description.to_string()],
    ))
}

fn exists_dial(rule: &KompozTestRule, description: &str) -> bool {
    dial(rule, description).exists()
}

/// Whether the texts drawn hold `text` (spaces and line breaks aside).
fn drawn_has(rule: &KompozTestRule, text: &str) -> bool {
    drawn(rule).replace(' ', "").contains(&text.replace(' ', ""))
}

/// Whether a node shows `text` (a time input's hidden field shows its box's text too: any of them).
fn exists_text(rule: &KompozTestRule, text: &str) -> bool {
    !rule.on_all_nodes_with_text(text).is_empty()
}

/// The texts drawn, a line a text: joined by spaces.
fn drawn(rule: &KompozTestRule) -> String {
    rule.drawn_texts().join(" ")
}

/// The C++ tests' `rule.swipe(from, to, frames)`: a move each frame, up at once.
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn key(rule: &KompozTestRule, key: Key) {
    rule.host().key_down(key, KeyModifiers::None);
    rule.host().key_up(key, KeyModifiers::None);
    rule.wait_for_idle();
}

fn type_text(rule: &KompozTestRule, text: &str) {
    rule.host().text_input(text);
    rule.wait_for_idle();
}

/// Lets the dial's animations and the auto switch's delay end.
fn settle(rule: &KompozTestRule) {
    rule.advance_time_by(1000);
    rule.wait_for_idle();
}

/// A platform keeping the haptic feedbacks asked for.
#[derive(Default)]
struct HapticPlatform {
    feedbacks: RefCell<Vec<HapticFeedbackType>>,
}

impl PlatformCallbacks for HapticPlatform {
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.feedbacks.borrow_mut().push(kind);
    }
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

/// The C++'s 24-hour dial (its `rememberTimePickerState(9, 30)` defaulted to 24 hours; Compose's default is the
/// locale's: Turkish's is 24 hours). M15's 24-hour dial: 0..11 outside (`Hours[i] % 12`, one digit as
/// `toLocalString()` writes it: the C++ had 12 there), 12..23 inside (the C++ had 00 at the top). The dial's numbers
/// are found by their descriptions (Compose's numbers clear their text's semantics: "15 saat", "40 dakika"); the
/// hour box is described "Saat seçin" (Compose's `TimePickerHourSelection`; the C++'s option "Saat").
fn time_picker_dial_picks_hours_then_minutes() {
    let cell = state_cell();
    let c = cell.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Turkish(move || {
                let s = remember_time_picker_state(initial_hour = 9, initial_minute = 30);
                *c.borrow_mut() = Some(s.clone());
                TimePicker(s, layout_type = TimePickerLayoutType::Vertical);
            });
        }),
        IntSize::new(400, 800),
    );
    let state = state_of(&cell);
    assert!(state.is24hour(), "Turkish writes times on a 24-hour clock");
    // 24-hour dial: 0..11 outside, 12..23 inside, 32 dp in.
    let outer_top = described(&rule, "0 saat");
    let inner_top = described(&rule, "12 saat");
    assert!(exists_text(&rule, "09"));
    assert!(near(outer_top.center().x, inner_top.center().x, 0.5), "{outer_top:?} {inner_top:?}");
    assert!(near(inner_top.top - outer_top.top, 32.0, 0.5), "{outer_top:?} {inner_top:?}");
    rule.on_node_with_content_description("15 saat").perform_click(); // the inner ring
    settle(&rule);
    assert_eq!(state.hour(), 15);
    assert_eq!(state.selection(), TimePickerSelectionMode::Minute);
    assert!(exists_described(&rule, "55 dakika") && !exists_described(&rule, "23 saat")); // the minutes now
    // Drag round to 22 past: the minute follows the finger, less the touch slop (Compose's detectDragGestures gives
    // its first move less the slop: the C++'s drag to 22.2 picks 21 here, one to 23.2 picks 22); letting go keeps to
    // the minutes.
    let top = described(&rule, "0 dakika").center();
    let left = described(&rule, "45 dakika").center();
    let center = Offset::new(top.x, left.y);
    let on_dial = |minute: f32, r: f32| {
        let a = minute * std::f32::consts::PI / 30.0;
        Offset::new(center.x + r * a.sin(), center.y - r * a.cos())
    };
    swipe(&rule, on_dial(15.0, 90.0), on_dial(23.2, 90.0), 10);
    settle(&rule);
    assert_eq!(state.minute(), 22);
    assert_eq!(state.selection(), TimePickerSelectionMode::Minute);
    tap(&rule, on_dial(41.4, 101.0)); // a tap beside 40 picks 40
    settle(&rule);
    assert_eq!(state.minute(), 40);
    // Back to the hours with the hour box; a drag that ends on the outer 3 picks 3 and moves on.
    rule.on_node_with_content_description("Saat seçin").perform_click();
    settle(&rule);
    assert_eq!(state.selection(), TimePickerSelectionMode::Hour);
    assert!(exists_described(&rule, "23 saat"));
    swipe(&rule, on_dial(30.0, 70.0), on_dial(15.0, 100.0), 10);
    settle(&rule);
    assert_eq!((state.hour(), state.minute()), (3, 40));
    assert_eq!(state.selection(), TimePickerSelectionMode::Minute);
    assert!(exists_text(&rule, "03") && exists_text(&rule, "40"));
}

/// A window wider than tall lays the picker out side by side (Compose's `TimePickerDefaults.layoutType()`, Compose
/// for desktop's actual: the window's container size).
fn time_picker_lays_out_side_by_side_in_landscape() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            TimePicker(remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false));
        }),
        IntSize::new(800, 400),
    );
    let hour = text_bounds(&rule, "09");
    let minute = text_bounds(&rule, "30");
    let am = text_bounds(&rule, "AM");
    let twelve = described(&rule, "12 o'clock");
    assert!(near(hour.top, minute.top, 0.5) && minute.left > hour.right, "{hour:?} {minute:?}"); // the time in a row...
    assert!(am.top > hour.bottom && twelve.left > minute.right + 52.0, "{am:?} {twelve:?}"); // ...over the period, the dial beside
}

/// A 12-hour clock: AM and PM beside the time, the hours 12, 1..11 (Compose's texts by language: Turkish's "ÖÖ" and
/// "ÖS", the C++'s options).
fn time_picker_twelve_hour_clock() {
    let cell = state_cell();
    let c = cell.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Turkish(move || {
                let s = remember_time_picker_state(initial_hour = 21, initial_minute = 5, is_24_hour = false);
                *c.borrow_mut() = Some(s.clone());
                TimePicker(s, layout_type = TimePickerLayoutType::Vertical);
            });
        }),
        IntSize::new(400, 800),
    );
    let state = state_of(&cell);
    assert!(exists_text(&rule, "09") && exists_text(&rule, "05") && !exists_described(&rule, "Saat 0"));
    rule.on_node_with_text("ÖÖ").perform_click();
    rule.wait_for_idle();
    assert_eq!(state.hour(), 9);
    rule.on_node_with_content_description("Saat 12").perform_click(); // 12 in the morning
    settle(&rule);
    assert_eq!(state.hour(), 0);
    assert!(exists_text(&rule, "12"));
    assert_eq!(state.selection(), TimePickerSelectionMode::Minute);
    rule.on_node_with_text("ÖS").perform_click();
    rule.wait_for_idle();
    assert_eq!(state.hour(), 12);
}

/// The C++'s time input, in M15's ways: what is typed is what the state holds as typed (`hourInput`, `minuteInput`):
/// a number up to 99 is taken, an hour over 23 flagged an error (the C++ dropped it for its last digit), a third
/// digit at the end is an error (a reject haptic; the C++ started the field over with it), a digit typed at the
/// start of a full field starts it over (Compose's `text.length == 3 && selection.start == 1`), an emptied field
/// sets 0 (the C++ kept the time), what isn't a number is dropped. The fields' "Saat"/"Dakika" are their supporting
/// texts (no semantics, as Compose's: found among the drawn texts).
fn time_input_types_hours_and_minutes() {
    let cell = state_cell();
    let c = cell.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Turkish(move || {
                let s = remember_time_picker_state(initial_hour = 9, initial_minute = 30);
                *c.borrow_mut() = Some(s.clone());
                TimeInput(s);
            });
        }),
        IntSize::new(400, 800),
    );
    let haptics = Rc::new(HapticPlatform::default());
    rule.host().set_callbacks(haptics.clone());
    rule.wait_for_idle();
    let state = state_of(&cell);
    let texts = rule.drawn_texts();
    assert!(texts.contains(&"Saat".to_string()) && texts.contains(&"Dakika".to_string()), "{texts:?}");
    let hour_field = || rule.on_node_with_content_description("saat");
    let minute_field = || rule.on_node_with_content_description("dakika");
    let retype = |field: kompoz_test::SemanticsNodeInteraction, typed: &str| {
        field.perform_click();
        rule.wait_for_idle();
        key(&rule, Key::MoveEnd);
        key(&rule, Key::Backspace);
        key(&rule, Key::Backspace);
        type_text(&rule, typed);
    };
    retype(hour_field(), "14");
    assert_eq!(state.hour(), 14);
    type_text(&rule, "7"); // "147": a third digit is an error
    assert_eq!(state.hour(), 14);
    assert_eq!(haptics.feedbacks.borrow().as_slice(), &[HapticFeedbackType::Reject]);
    key(&rule, Key::MoveHome);
    type_text(&rule, "7"); // "714" typed at the start: the field starts over
    assert_eq!(state.hour(), 7);
    hour_field().assert_text_equals(&["7"]);
    type_text(&rule, "5"); // 75 isn't an hour: taken as typed, flagged
    assert_eq!((state.hour(), state.hour_input()), (7, 75));
    assert!(!state.is_hour_input_valid());
    assert!(drawn(&rule).contains("Saat 0-23 arasında olmalıdır"), "{}", drawn(&rule));
    key(&rule, Key::Backspace);
    assert_eq!((state.hour(), state.hour_input()), (7, 7));
    // On to the minutes by their box: the hour's box shows it in full.
    rule.on_node_with_content_description("Dakikayı seçin").perform_click();
    settle(&rule);
    assert_eq!(state.selection(), TimePickerSelectionMode::Minute);
    assert!(exists_text(&rule, "07"));
    retype(minute_field(), "45");
    assert_eq!(state.minute(), 45);
    type_text(&rule, "9"); // "459": an error
    assert_eq!(state.minute(), 45);
    key(&rule, Key::Backspace);
    key(&rule, Key::Backspace); // emptied: the minute is 0
    assert_eq!(state.minute(), 0);
    minute_field().assert_text_equals(&[""]);
    type_text(&rule, "x"); // not a number: dropped
    assert_eq!(state.minute(), 0);
    minute_field().assert_text_equals(&[""]);
    type_text(&rule, "7");
    assert_eq!(state.minute(), 7);
    minute_field().assert_text_equals(&["7"]);
}

/// The C++'s dialog: a title, the buttons, a mode toggle of the app's own switching between the dial and the input
/// (24 hours, as Turkish's clock).
fn time_picker_dialog_switches_modes_and_confirms() {
    let open = mutable_state_of(true);
    let confirmed: Rc<RefCell<Option<(i32, i32)>>> = Rc::new(RefCell::new(None));
    let (o, c) = (open.clone(), confirmed.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (o, c) = (o.clone(), c.clone());
            Turkish(move || {
                if !o.get() {
                    return;
                }
                let s = remember_time_picker_state(initial_hour = 8, initial_minute = 15);
                let typing = remember(|| mutable_state_of(false));
                let (o1, o2, o3, c, s1, s2, t1, t2) =
                    (o.clone(), o.clone(), o.clone(), c.clone(), s.clone(), s.clone(), typing.clone(), typing.clone());
                TimePickerDialog(
                    move || o1.set(false),
                    move || {
                        let (o, c, s) = (o2.clone(), c.clone(), s1.clone());
                        TextButton(
                            move || {
                                *c.borrow_mut() = Some((s.hour(), s.minute()));
                                o.set(false);
                            },
                            content = || Text("Tamam"),
                        );
                    },
                    || Text("Saat seçin"),
                    dismiss_button = Some(Rc::new(move || {
                        let o = o3.clone();
                        TextButton(move || o.set(false), content = || Text("İptal"));
                    })),
                    mode_toggle_button = Some(Rc::new(move || {
                        let t = t1.clone();
                        let shown = t.get();
                        IconButton(
                            move || t.set(!t.get()),
                            content = move || {
                                let icon = if shown { Icons::Filled::Schedule() } else { Icons::Filled::Keyboard() };
                                VectorIcon(icon, Some("Mod".into()));
                            },
                        );
                    })),
                    content = move || {
                        if t2.get() {
                            TimeInput(s2.clone());
                        } else {
                            TimePicker(s2.clone(), layout_type = TimePickerLayoutType::Vertical);
                        }
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    assert!(exists_text(&rule, "Saat seçin") && exists_described(&rule, "12 saat"));
    rule.on_node_with_content_description("20 saat").perform_click();
    settle(&rule);
    rule.on_node_with_content_description("Mod").perform_click();
    settle(&rule);
    assert!(!exists_described(&rule, "12 saat"));
    assert!(drawn(&rule).contains("Dakika") && exists_text(&rule, "20"));
    rule.on_node_with_text("Tamam").perform_click();
    rule.wait_for_idle();
    assert_eq!(*confirmed.borrow(), Some((20, 15)));
    assert!(!open.peek() && rule.host().overlay_count() == 0);
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// A state the app makes itself, of what it must (Kotlin's interface's default `hourInput`/`minuteInput`: the time
/// itself, set only to a time).
struct OwnState {
    hour: std::cell::Cell<i32>,
    minute: std::cell::Cell<i32>,
}

impl TimePickerState for OwnState {
    fn minute(&self) -> i32 {
        self.minute.get()
    }
    fn set_minute(&self, value: i32) {
        self.minute.set(value);
    }
    fn hour(&self) -> i32 {
        self.hour.get()
    }
    fn set_hour(&self, value: i32) {
        self.hour.set(value);
    }
    fn is24hour(&self) -> bool {
        true
    }
    fn set_is24hour(&self, _: bool) {}
    fn selection(&self) -> TimePickerSelectionMode {
        TimePickerSelectionMode::Hour
    }
    fn set_selection(&self, _: TimePickerSelectionMode) {}
}

fn time_picker_states_hold_the_time_and_what_is_typed() {
    let s = TimePickerState(21, 5, false);
    assert_eq!((s.hour(), s.minute(), s.is24hour(), s.selection()), (21, 5, false, TimePickerSelectionMode::Hour));
    assert!(s.is_pm() && s.is_input_valid());
    // What is typed is kept as typed; the time only when it is one.
    s.set_hour_input(30);
    assert_eq!((s.hour(), s.hour_input()), (21, 30));
    assert!(!s.is_hour_input_valid() && !s.is_input_valid());
    s.set_hour(8); // setting the time sets what is typed
    assert_eq!((s.hour(), s.hour_input()), (8, 8));
    assert!(!s.is_pm() && s.is_input_valid());
    s.set_minute_input(-1);
    assert_eq!((s.minute(), s.minute_input()), (5, -1));
    assert!(!s.is_minute_input_valid());
    s.set_minute_input(59);
    assert_eq!((s.minute(), s.minute_input()), (59, 59));
    s.set_is24hour(true);
    s.set_selection(TimePickerSelectionMode::Minute);
    assert!(s.is24hour() && s.selection() == TimePickerSelectionMode::Minute);
    // Equal by identity.
    let other = TimePickerState(8, 59, true);
    assert!(*s == *s.clone() && *s != *other);
    // The interface's defaults: what is typed is the time.
    let own = OwnState { hour: std::cell::Cell::new(7), minute: std::cell::Cell::new(45) };
    own.set_hour_input(24);
    own.set_minute_input(12);
    assert_eq!((own.hour(), own.hour_input(), own.minute(), own.minute_input()), (7, 7, 12, 12));
    // Compose's `require`s (the panics' messages kept quiet).
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (bad_hour, bad_minute) = (
        std::panic::catch_unwind(|| TimePickerState(24, 0, true)).is_err(),
        std::panic::catch_unwind(|| TimePickerState(0, 60, true)).is_err(),
    );
    std::panic::set_hook(hook);
    assert!(bad_hour && bad_minute);
    // The value classes: their values and names.
    assert_eq!((TimePickerSelectionMode::Hour.value(), TimePickerSelectionMode::Minute.value()), (0, 1));
    assert_eq!(format!("{} {}", TimePickerSelectionMode::Hour, TimePickerSelectionMode::Minute), "Hour Minute");
    assert_eq!(
        format!("{} {:?}", TimePickerLayoutType::Horizontal, TimePickerLayoutType::Vertical),
        "Horizontal Vertical"
    );
    assert_eq!(format!("{} {}", TimePickerDisplayMode::Picker, TimePickerDisplayMode::Input), "Picker Input");
}

/// The state of a time input shown or not (for the save and restore below).
fn saved_time_input(shown: &MutableState<bool>, cell: &StateCell) -> impl Content + 'static {
    let (shown, cell) = (shown.clone(), cell.clone());
    kompozable_lambda!(move || {
        if shown.get() {
            let s = remember_time_picker_state(initial_hour = 10, initial_minute = 20, is_24_hour = true);
            *cell.borrow_mut() = Some(s.clone());
            TimeInput(s);
        }
    })
}

/// `rememberTimePickerState()`'s clock is the locale's (Compose for desktop: the locale's CLDR time format):
/// English's 12 hours, Turkish's 24; the time and the clock, and what a time input's fields hold, are saved with the
/// host.
fn time_picker_states_take_the_locales_clock_and_are_saved() {
    let (en, tr, given) = (state_cell(), state_cell(), state_cell());
    let (e, t, g) = (en.clone(), tr.clone(), given.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        *e.borrow_mut() = Some(remember_time_picker_state());
        let t = t.clone();
        Turkish(move || *t.borrow_mut() = Some(remember_time_picker_state()));
        *g.borrow_mut() = Some(remember_time_picker_state(initial_hour = 13, is_24_hour = true));
    }));
    let (en, tr, given) = (state_of(&en), state_of(&tr), state_of(&given));
    assert_eq!((en.hour(), en.minute(), en.is24hour()), (0, 0, false));
    assert!(tr.is24hour());
    assert_eq!((given.hour(), given.minute(), given.is24hour()), (13, 0, true));

    let bytes = {
        let (shown, cell) = (mutable_state_of(true), state_cell());
        let rule = KompozTestRule::new(saved_time_input(&shown, &cell));
        let s = state_of(&cell);
        s.set_hour(17);
        s.set_minute(42);
        s.set_is24hour(false);
        rule.wait_for_idle();
        rule.on_node_with_content_description("for hour").perform_click();
        rule.wait_for_idle();
        key(&rule, Key::MoveEnd);
        key(&rule, Key::Backspace);
        rule.host().save_state()
    };
    let (shown, cell) = (mutable_state_of(false), state_cell());
    let rule = KompozTestRule::new(saved_time_input(&shown, &cell));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    let s = state_of(&cell);
    assert_eq!((s.hour(), s.minute(), s.is24hour()), (12, 42, false));
    // (what the hour field held, "0": its value is saved apart from the state)
    rule.on_node_with_content_description("for hour").assert_text_equals(&["0"]);
}

type ColorsCell = Rc<RefCell<Option<TimePickerColors>>>;

fn time_picker_colors_come_from_the_scheme_and_copy_what_is_given() {
    let cs = light_color_scheme();
    let (colors, given): (ColorsCell, ColorsCell) = (Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None)));
    let (c, g) = (colors.clone(), given.clone());
    let red = Color::hex(0xFF0000);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *c.borrow_mut() = Some(TimePickerDefaults.colors());
        let mine = TimePickerDefaults.colors(
            time_selector_selected_content_color = Color::hex(0x123456),
            time_selector_unselected_content_color = Color::hex(0x654321),
            period_selector_selected_content_color = Color::hex(0x00AA00),
            clock_dial_color = red,
        );
        *g.borrow_mut() = Some(mine);
        TimePicker(
            remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false),
            colors = mine,
            layout_type = TimePickerLayoutType::Vertical,
        );
    }));
    let colors = colors.borrow().unwrap();
    let expected = TimePickerColors::new(
        cs.surface_container_highest,
        cs.primary,
        cs.surface_container_high,
        cs.outline,
        cs.on_primary,
        cs.on_surface,
        cs.tertiary_container,
        Color::Transparent,
        cs.on_tertiary_container,
        cs.on_surface_variant,
        cs.primary_container,
        cs.surface_container_highest,
        cs.on_primary_container,
        cs.on_surface,
    );
    assert_eq!(colors, expected);
    assert_eq!(colors.clock_dial_selected_content_color, cs.on_primary);
    // Compose's copy: the dial's color is by default the container's.
    let copied = colors.copy();
    assert_eq!(copied.clock_dial_color, colors.container_color);
    assert_eq!(
        named_args!(colors.copy(clock_dial_color = Color::Unspecified)).clock_dial_color,
        colors.clock_dial_color
    );
    // Compose's equals leaves the dial's content colors out.
    assert_eq!(
        named_args!(colors.copy(clock_dial_color = colors.clock_dial_color, clock_dial_selected_content_color = red)),
        colors
    );
    assert_ne!(named_args!(colors.copy(clock_dial_color = colors.clock_dial_color, selector_color = red)), colors);
    // The colors given, drawn: the selected hour box's text, the minute's, AM's, the dial.
    let given = given.borrow().unwrap();
    assert_eq!(given.clock_dial_color, red);
    assert_eq!(given.selector_color, cs.primary);
    let canvas = rule.draw();
    let color_of = |text: &str| canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i]);
    assert_eq!(color_of("09"), Some(Color::hex(0x123456)));
    assert_eq!(color_of("30"), Some(Color::hex(0x654321)));
    assert_eq!(color_of("AM"), Some(Color::hex(0x00AA00)));
    assert_eq!(color_of("PM"), Some(cs.on_surface_variant));
    assert_eq!(color_of("10"), Some(cs.on_surface)); // a dial number: the unselected content color
    assert!(canvas.rect_colors.contains(&red) && canvas.rect_colors.contains(&cs.primary));
}

/// Compose for desktop's `TimePickerDefaults.layoutType()`: side by side when the window is wider than tall; the
/// side-by-side dial is 256 dp from a height of 384 dp, 238 from 330, else 200 (its numbers' ring the dial's 101/256).
fn time_picker_layout_and_dial_size_follow_the_window() {
    let layout: Rc<RefCell<Option<TimePickerLayoutType>>> = Rc::new(RefCell::new(None));
    let height = mutable_state_of(400.0_f32);
    let (l, h) = (layout.clone(), height.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *l.borrow_mut() = Some(TimePickerDefaults.layout_type());
            Box(
                modifier = Modifier.height(Dp(h.get())),
                content = || {
                    TimePicker(
                        remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false),
                        layout_type = TimePickerLayoutType::Horizontal,
                    );
                },
            );
        }),
        IntSize::new(800, 600),
    );
    assert_eq!(*layout.borrow(), Some(TimePickerLayoutType::Horizontal));
    let ring =
        |rule: &KompozTestRule| described(rule, "6 o'clock").center().y - described(rule, "12 o'clock").center().y;
    assert!(near(ring(&rule), 2.0 * 101.0, 0.6), "{}", ring(&rule));
    height.set(340.0);
    rule.wait_for_idle();
    assert!(near(ring(&rule), 2.0 * 238.0 * 101.0 / 256.0, 0.6), "{}", ring(&rule));
    height.set(300.0);
    rule.wait_for_idle();
    assert!(near(ring(&rule), 2.0 * 200.0 * 101.0 / 256.0, 0.6), "{}", ring(&rule));
    rule.host().set_viewport_size(IntSize::new(600, 800));
    rule.wait_for_idle();
    assert_eq!(*layout.borrow(), Some(TimePickerLayoutType::Vertical));
}

fn selected(rule: &KompozTestRule, description: &str) -> Option<bool> {
    let node = if description.starts_with("Select") {
        rule.on_node_with_content_description(description)
    } else {
        dial(rule, description)
    };
    node.fetch_semantics_node().config().get(SemanticsProperties::Selected)
}

/// The boxes are radio buttons described by what they pick and their value ("Select hour", "9 o'clock"), selected
/// while picked; the dial's numbers are described as times, the one under the selector selected, ordered for
/// traversal; the period toggle is a traversal group described "Select AM or PM", its halves selected as the time
/// is; the picker is a traversal group.
fn time_picker_parts_tell_accessibility_what_they_are() {
    let cell = state_cell();
    let c = cell.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false);
        *c.borrow_mut() = Some(s.clone());
        TimePicker(s, modifier = Modifier.test_tag("picker"), layout_type = TimePickerLayoutType::Vertical);
    }));
    let state = state_of(&cell);
    let hour_box = rule.on_node_with_content_description("Select hour").fetch_semantics_node();
    assert_eq!(hour_box.config().get(SemanticsProperties::Role), Some(Role::RadioButton));
    assert_eq!(
        hour_box.config().get(SemanticsProperties::ContentDescription),
        Some(vec!["Select hour".to_string(), "9 o'clock".to_string()])
    );
    assert_eq!(selected(&rule, "Select hour"), Some(true));
    assert_eq!(selected(&rule, "Select minutes"), Some(false));
    assert_eq!(selected(&rule, "9 o'clock"), Some(true)); // under the selector
    assert_eq!(selected(&rule, "10 o'clock"), Some(false));
    let toggle = rule.on_node_with_content_description("Select AM or PM").fetch_semantics_node();
    assert_eq!(toggle.config().get(SemanticsProperties::IsTraversalGroup), Some(true));
    let selected_text =
        |text: &str| rule.on_node_with_text(text).fetch_semantics_node().config().get(SemanticsProperties::Selected);
    assert_eq!((selected_text("AM"), selected_text("PM")), (Some(true), Some(false)));
    let picker = rule.on_node_with_tag("picker").fetch_semantics_node();
    assert_eq!(picker.config().get(SemanticsProperties::IsTraversalGroup), Some(true));
    let index = |d: &str| dial(&rule, d).fetch_semantics_node().config().get(SemanticsProperties::TraversalIndex);
    assert_eq!((index("12 o'clock"), index("3 o'clock")), (Some(1.0), Some(4.0)));
    // The minutes; a 24-hour clock's hours.
    rule.on_node_with_content_description("Select minutes").perform_click();
    settle(&rule);
    assert!(exists_dial(&rule, "30 minutes") && exists_dial(&rule, "55 minutes"));
    assert_eq!(selected(&rule, "30 minutes"), Some(true));
    assert_eq!(selected(&rule, "Select minutes"), Some(true));
    state.set_is24hour(true);
    state.set_selection(TimePickerSelectionMode::Hour);
    settle(&rule);
    assert!(exists_dial(&rule, "13 hours") && exists_dial(&rule, "0 hours"));
    assert_eq!(index("13 hours"), Some(13.0));
    assert!(exists_dial(&rule, "9 hours"));
}

/// Out of touch mode the dial's numbers take the focus: the arrows move it on (down, right) and back (up, left),
/// Enter picks the number (at once: a snap).
fn dial_numbers_take_keys_out_of_touch_mode() {
    let cell = state_cell();
    let c = cell.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false);
        *c.borrow_mut() = Some(s.clone());
        TimePicker(s, layout_type = TimePickerLayoutType::Vertical);
    }));
    let state = state_of(&cell);
    key(&rule, Key::Tab); // keyboard mode
    dial(&rule, "3 o'clock").request_focus();
    rule.wait_for_idle();
    dial(&rule, "3 o'clock").assert_is_focused();
    key(&rule, Key::DirectionDown);
    dial(&rule, "4 o'clock").assert_is_focused();
    key(&rule, Key::DirectionLeft);
    dial(&rule, "3 o'clock").assert_is_focused();
    key(&rule, Key::Enter);
    settle(&rule);
    assert_eq!(state.hour(), 3);
    assert_eq!(state.selection(), TimePickerSelectionMode::Minute);
}

/// A 12-hour clock's input: an hour over 1 moves on to the minute; the hour typed is in the half of the day the
/// period toggle says (an emptied field in the afternoon is 12; "13" in it is 25, an error: "Hour must be 1–12").
fn time_input_on_a_twelve_hour_clock_moves_on_and_flags_errors() {
    let cell = state_cell();
    let c = cell.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false);
        *c.borrow_mut() = Some(s.clone());
        TimeInput(s);
    }));
    let state = state_of(&cell);
    let retype = |typed: &str| {
        rule.on_node_with_content_description("for hour").perform_click();
        rule.wait_for_idle();
        key(&rule, Key::MoveEnd);
        key(&rule, Key::Backspace);
        key(&rule, Key::Backspace);
        if !typed.is_empty() {
            type_text(&rule, typed);
        }
    };
    retype("3");
    settle(&rule);
    assert_eq!((state.hour(), state.selection()), (3, TimePickerSelectionMode::Minute));
    rule.on_node_with_text("PM").perform_click();
    rule.wait_for_idle();
    assert_eq!(state.hour(), 15);
    assert!(exists_text(&rule, "03"));
    rule.on_node_with_content_description("Select hour").perform_click();
    settle(&rule);
    retype("");
    assert_eq!(state.hour(), 12); // emptied, in the afternoon
    type_text(&rule, "13");
    assert_eq!((state.hour(), state.hour_input()), (12, 25));
    assert!(!state.is_input_valid());
    assert!(drawn_has(&rule, "Hour must be 1–12"), "{}", drawn(&rule));
}

/// The pickers' texts follow `LocalStrings`' language as it changes (Compose's `getString` reads it): AM/PM, the
/// boxes' and fields' descriptions, the fields' supporting texts, the dial's descriptions.
fn time_pickers_follow_the_platforms_language_live() {
    let turkish = mutable_state_of(false);
    let t = turkish.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let strings = if t.get() { Strings::turkish() } else { Strings::english() };
            KompozitionLocalProvider(&[LocalStrings.provides(strings)], || {
                Column(
                    content = || {
                        TimePicker(
                            remember_time_picker_state(initial_hour = 21, initial_minute = 5, is_24_hour = false),
                            layout_type = TimePickerLayoutType::Vertical,
                        );
                        TimeInput(remember_time_picker_state(
                            initial_hour = 10,
                            initial_minute = 30,
                            is_24_hour = false,
                        ));
                    },
                );
            });
        }),
        IntSize::new(400, 1200),
    );
    assert!(rule.on_all_nodes_with_text("AM").len() == 2 && exists_dial(&rule, "9 o'clock"));
    assert!(exists_described(&rule, "Select hour") && exists_described(&rule, "for minutes"));
    assert!(drawn(&rule).contains("Hour") && !drawn(&rule).contains("Saat"));
    turkish.set(true);
    rule.wait_for_idle();
    assert!(rule.on_all_nodes_with_text("ÖÖ").len() == 2 && !exists_text(&rule, "AM"));
    assert!(exists_dial(&rule, "Saat 9") && !exists_dial(&rule, "9 o'clock"));
    assert!(exists_described(&rule, "Saat seçin") && exists_described(&rule, "dakika"));
    assert!(drawn(&rule).contains("Saat") && !drawn(&rule).contains("Hour"));
}

type DialogDefaults = Rc<RefCell<Option<(std::sync::Arc<dyn Shape>, Color)>>>;

/// The dialog's layout (portrait: the title 24 dp in, the content centered under it, the buttons under that;
/// landscape — a side-by-side dial —: the content 24 dp in, the title at the top start), its defaults, the title and
/// the display mode toggle by mode.
fn time_picker_dialog_lays_out_and_has_its_defaults() {
    let horizontal = mutable_state_of(false);
    let mode = mutable_state_of(TimePickerDisplayMode::Picker);
    let defaults: DialogDefaults = Rc::new(RefCell::new(None));
    let (h, m, d) = (horizontal.clone(), mode.clone(), defaults.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *d.borrow_mut() = Some((TimePickerDialogDefaults.shape(), TimePickerDialogDefaults.container_color()));
            let (h, m, m2) = (h.clone(), m.clone(), m.clone());
            TimePickerDialog(
                || {},
                || TextButton(|| {}, content = || Text("OK")),
                move || TimePickerDialogDefaults.Title(m.get(), modifier = Modifier.test_tag("title")),
                modifier = Modifier.test_tag("dialog"),
                mode_toggle_button = Some(Rc::new(move || {
                    let m = m2.clone();
                    let shown = m.get();
                    TimePickerDialogDefaults.DisplayModeToggle(
                        move || {
                            m.set(if m.get() == TimePickerDisplayMode::Picker {
                                TimePickerDisplayMode::Input
                            } else {
                                TimePickerDisplayMode::Picker
                            })
                        },
                        shown,
                    );
                })),
                content = move || {
                    TimePicker(
                        remember_time_picker_state(initial_hour = 9, initial_minute = 30, is_24_hour = false),
                        modifier = Modifier.test_tag("picker"),
                        layout_type =
                            if h.get() { TimePickerLayoutType::Horizontal } else { TimePickerLayoutType::Vertical },
                    );
                },
            );
        }),
        IntSize::new(800, 1000),
    );
    let cs = light_color_scheme();
    let (shape, color) = defaults.borrow().clone().unwrap();
    assert!(shape == RoundedCornerShape(Dp(28.0)) && color == cs.surface_container_high);
    assert_eq!(TimePickerDialogDefaults::MinHeightForTimePicker, Dp(300.0));
    let bounds = |tag: &str| rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    let (dialog, title, picker, ok) = (bounds("dialog"), bounds("title"), bounds("picker"), text_bounds(&rule, "OK"));
    assert!(near(title.left - dialog.left, 24.0, 0.5) && near(title.top - dialog.top, 24.0, 0.5), "{title:?}");
    assert!(near(dialog.width(), picker.width() + 48.0, 0.5), "{dialog:?} {picker:?}");
    assert!(
        near(picker.center().x, dialog.center().x, 0.5) && picker.top >= title.bottom - 0.5,
        "{picker:?} {dialog:?} {title:?}"
    );
    assert!(ok.top >= picker.bottom - 0.5 && ok.bottom <= dialog.bottom - 24.0 + 0.5, "{ok:?} {dialog:?}");
    assert!(exists_text(&rule, "Select Time") && exists_described(&rule, "Switch to text input mode"));
    rule.on_node_with_content_description("Switch to text input mode").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Enter Time") && exists_described(&rule, "Switch to clock mode"));
    // Landscape: a side-by-side picker, wider than tall.
    horizontal.set(true);
    rule.wait_for_idle();
    let (dialog, title, picker) = (bounds("dialog"), bounds("title"), bounds("picker"));
    assert!(picker.width() > picker.height());
    assert!(near(picker.left - dialog.left, 24.0, 0.5) && near(title.left - dialog.left, 24.0, 0.5), "{picker:?}");
    assert!(near(title.top - dialog.top, 24.0, 0.5));
}

// ---- M15's TimePickerSamples -----------------------------------------------------------------------------------

/// The samples' `SimpleDateFormat("hh:mm a", Locale.getDefault()).format(cal.time)` (English: "03:30 PM").
fn hh_mm_a(hour: i32, minute: i32) -> String {
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{twelve:02}:{minute:02} {}", if hour < 12 { "AM" } else { "PM" })
}

/// The samples' opening: "Set Time" with a snackbar host.
#[kompozable]
fn SampleScaffold(show: MutableState<bool>, snack_state: SnackbarHostState) {
    Box(
        propagate_min_constraints = false,
        content = move || {
            let show = show.clone();
            Button(move || show.set(true), modifier = Modifier.align(Alignment::Center), content = || Text("Set Time"));
            SnackbarHost(snack_state.clone());
        },
    );
}

/// The samples' confirm button: the time to the snackbar ("Entered time: …"), the dialog closed.
#[kompozable]
fn SampleConfirm(
    state: Rc<dyn TimePickerState>,
    show: MutableState<bool>,
    snack_state: SnackbarHostState,
    snack_scope: kompoz_coroutines::CoroutineScope,
    enabled: bool,
) {
    let scope = snack_scope;
    TextButton(
        move || {
            // (the sample's `state.isHourInputValid` statement, whose value it drops)
            let _ = state.is_hour_input_valid();
            let message = format!("Entered time: {}", hh_mm_a(state.hour(), state.minute()));
            let snack_state = snack_state.clone();
            scope.launch(async move {
                named_args!(snack_state.show_snackbar(message)).await;
            });
            show.set(false);
        },
        enabled = enabled,
        content = || Text("Ok"),
    );
}

/// TimePickerSample: a dial picker in a dialog; Ok (enabled while the input is valid) shows the time picked.
fn time_picker_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let show_time_picker = remember(|| mutable_state_of(false));
        let state = remember_time_picker_state();
        let snack_state = remember(SnackbarHostState::new);
        let snack_scope = kompoz_coroutines::remember_coroutine_scope();
        SampleScaffold(show_time_picker.clone(), snack_state.clone());
        if show_time_picker.get() {
            let (s1, s2, s3, st) =
                (show_time_picker.clone(), show_time_picker.clone(), show_time_picker.clone(), state.clone());
            TimePickerDialog(
                move || s1.set(false),
                move || {
                    SampleConfirm(st.clone(), s2.clone(), snack_state.clone(), snack_scope.clone(), st.is_input_valid())
                },
                || TimePickerDialogDefaults.Title(TimePickerDisplayMode::Picker),
                dismiss_button = Some(Rc::new(move || {
                    let s = s3.clone();
                    TextButton(move || s.set(false), content = || Text("Cancel"));
                })),
                mode_toggle_button = Some(Rc::new(|| {})),
                content = move || TimePicker(state.clone()),
            );
        }
    }));
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Select Time") && exists_dial(&rule, "12 o'clock"));
    dial(&rule, "3 o'clock").perform_click();
    settle(&rule);
    dial(&rule, "30 minutes").perform_click();
    settle(&rule);
    rule.on_node_with_text("PM").perform_click();
    rule.on_node_with_text("Ok").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Entered time: 03:30 PM") && !exists_text(&rule, "Select Time"));
    // (the snackbar, over the button as the sample's Box lays them out, goes after its short duration)
    rule.advance_time_by(5000);
    rule.wait_for_idle();
    assert!(!exists_text(&rule, "Entered time: 03:30 PM"));
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Cancel").perform_click();
    rule.wait_for_idle();
    assert!(!exists_text(&rule, "Select Time") && rule.host().overlay_count() == 0);
}

/// TimeInputSample: a time input in a dialog.
fn time_input_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let show_time_picker = remember(|| mutable_state_of(false));
        let state = remember_time_picker_state();
        let snack_state = remember(SnackbarHostState::new);
        let snack_scope = kompoz_coroutines::remember_coroutine_scope();
        SampleScaffold(show_time_picker.clone(), snack_state.clone());
        if show_time_picker.get() {
            let (s1, s2, s3, st) =
                (show_time_picker.clone(), show_time_picker.clone(), show_time_picker.clone(), state.clone());
            TimePickerDialog(
                move || s1.set(false),
                move || SampleConfirm(st.clone(), s2.clone(), snack_state.clone(), snack_scope.clone(), true),
                || TimePickerDialogDefaults.Title(TimePickerDisplayMode::Input),
                dismiss_button = Some(Rc::new(move || {
                    let s = s3.clone();
                    TextButton(move || s.set(false), content = || Text("Cancel"));
                })),
                mode_toggle_button = Some(Rc::new(|| {})),
                content = move || TimeInput(state.clone()),
            );
        }
    }));
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Enter Time"));
    rule.on_node_with_content_description("for hour").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::MoveEnd);
    key(&rule, Key::Backspace);
    key(&rule, Key::Backspace);
    type_text(&rule, "7"); // on to the minute
    settle(&rule);
    key(&rule, Key::MoveEnd);
    key(&rule, Key::Backspace);
    key(&rule, Key::Backspace);
    type_text(&rule, "45");
    rule.on_node_with_text("Ok").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Entered time: 07:45 AM"), "{}", drawn(&rule));
}

/// TimePickerSwitchableSample: the dialog's display mode toggle switches between the dial and the input while the
/// screen is taller than `MinHeightForTimePicker` (Android's `LocalConfiguration.current.screenHeightDp`: the
/// window's height here); a shorter one gets the input and no toggle.
fn time_picker_switchable_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let show_time_picker = remember(|| mutable_state_of(false));
        let state = remember_time_picker_state();
        let snack_state = remember(SnackbarHostState::new);
        let snack_scope = kompoz_coroutines::remember_coroutine_scope();
        let display_mode = remember(|| mutable_state_of(TimePickerDisplayMode::Picker));
        let density = kompoz_ui::platform::LocalDensity.current();
        let screen_height =
            density.to_dp(kompoz_ui::platform::LocalWindowInfo.current().container_size().height as f32);
        SampleScaffold(show_time_picker.clone(), snack_state.clone());
        if show_time_picker.get() {
            let (s1, s2, s3, st) =
                (show_time_picker.clone(), show_time_picker.clone(), show_time_picker.clone(), state.clone());
            let (d1, d2, d3) = (display_mode.clone(), display_mode.clone(), display_mode.clone());
            TimePickerDialog(
                move || s1.set(false),
                move || SampleConfirm(st.clone(), s2.clone(), snack_state.clone(), snack_scope.clone(), true),
                move || TimePickerDialogDefaults.Title(d1.get()),
                dismiss_button = Some(Rc::new(move || {
                    let s = s3.clone();
                    TextButton(move || s.set(false), content = || Text("Cancel"));
                })),
                mode_toggle_button = Some(Rc::new(move || {
                    if screen_height > TimePickerDialogDefaults::MinHeightForTimePicker {
                        let d = d2.clone();
                        let shown = d.get();
                        TimePickerDialogDefaults.DisplayModeToggle(
                            move || {
                                d.set(if d.get() == TimePickerDisplayMode::Picker {
                                    TimePickerDisplayMode::Input
                                } else {
                                    TimePickerDisplayMode::Picker
                                })
                            },
                            shown,
                        );
                    }
                })),
                content = move || {
                    if d3.get() == TimePickerDisplayMode::Picker
                        && screen_height > TimePickerDialogDefaults::MinHeightForTimePicker
                    {
                        TimePicker(state.clone());
                    } else {
                        TimeInput(state.clone());
                    }
                },
            );
        }
    }));
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Select Time") && exists_dial(&rule, "12 o'clock"));
    rule.on_node_with_content_description("Switch to text input mode").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Enter Time") && exists_described(&rule, "for hour"));
    assert!(!exists_dial(&rule, "12 o'clock"));
    rule.on_node_with_content_description("Switch to clock mode").perform_click();
    rule.wait_for_idle();
    assert!(exists_dial(&rule, "12 o'clock"));
    // A short window: the input, no toggle.
    rule.host().set_viewport_size(IntSize::new(800, 280));
    rule.wait_for_idle();
    assert!(exists_described(&rule, "for hour") && !exists_described(&rule, "Switch to text input mode"));
    rule.on_node_with_text("Ok").perform_click();
    rule.wait_for_idle();
    assert!(exists_text(&rule, "Entered time: 12:00 AM"), "{}", drawn(&rule));
}
