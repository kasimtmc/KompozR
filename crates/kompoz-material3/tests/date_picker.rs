//! Material's date pickers as Compose's material3 (1.5.0-alpha17) has them, with Compose for desktop's actuals: the
//! C++ Kompoz's date picker tests by name (kompoz_tests.cpp; where Compose behaves otherwise than the C++ did, the
//! test asserts Compose's and says so), Compose's behaviour (the state's limits and saving, the selectable dates,
//! the formatter, the colors, the headline's and the days' descriptions, the range's band and actions), and the M15
//! samples of DatePickerSamples.kt. "Today" comes from a test clock (`kompoz_material3::testing`), dates from CLDR
//! in the locale of `LocalStrings` when the state is made.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::remember_coroutine_scope;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_material3::testing::set_today_for_tests;
use kompoz_runtime::kt::{LocalDate, YearMonth};
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::strings::{LocalStrings, Strings};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    date_picker_picks_days_months_and_years,
    date_picker_takes_a_typed_date_in_input_mode,
    date_picker_dialog_confirms_the_date,
    date_range_picker_takes_typed_dates_in_input_mode,
    date_range_picker_picks_a_range,
    date_picker_states_keep_to_their_years_and_save,
    date_picker_disables_the_dates_it_may_not_pick,
    date_picker_formats_and_colors_by_its_defaults,
    date_picker_headline_and_days_read_out_their_dates,
    date_picker_samples,
    date_range_picker_samples,
    date_pickers_take_and_give_java_time_dates,
    pickers_take_their_texts_from_local_strings_unless_given,
);

/// The C++ test with Compose's pickers (date, time, time input, date range): Material's texts — titles, arrows'
/// and selectors' descriptions, AM/PM, the field labels — follow `LocalStrings`' language at once (Compose's
/// `getString`), as does the date picker's headline (it formats in `defaultLocale()`); the dates of the months, the
/// days and the year button keep the locale their state was remembered in (Compose's states hold a `locale`, made
/// once by `rememberSaveable`; the C++ switched them too). Compose has no per-picker text overrides (the C++'s
/// `.pm = "Akşam"`, `.minuteLabel = "Dk"`): its texts are the locale's.
fn pickers_take_their_texts_from_local_strings_unless_given() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let turkish = mutable_state_of(false);
    let t = turkish.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let strings = if t.get() { Strings::turkish() } else { Strings::english() };
            KompozitionLocalProvider(&[LocalStrings.provides(strings)], || {
                Column(
                    content = || {
                        DatePicker(remember_date_picker_state(Some(millis(2026, 8, 17))));
                        TimePicker(
                            remember_time_picker_state(21, 5, false),
                            layout_type = TimePickerLayoutType::Vertical,
                        );
                        TimeInput(remember_time_picker_state(10, 30, true), modifier = Modifier.test_tag("input"));
                        DateRangePicker(
                            remember_date_range_picker_state(None, None, Some(Some(millis(2026, 3, 1)))),
                            modifier = Modifier.height(500.dp()),
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 2400),
    );
    rule.on_node_with_text("August 2026").assert_exists();
    rule.on_node_with_content_description("Change to next month").assert_exists();
    rule.on_node_with_text("Monday, August 17, 2026").assert_is_selected();
    rule.on_node_with_text("Aug 17, 2026").assert_exists();
    assert!(rule.on_node_with_text("AM").exists() && rule.on_node_with_text("PM").exists());
    // TimeInput's field labels under the fields (drawn; Compose clears their semantics: the fields say them).
    assert!(shows(&rule, "Hour") && shows(&rule, "Minute"));
    assert!(rule.on_node_with_text("March 2026").exists() && rule.on_node_with_text("Select dates").exists());
    turkish.set(true);
    rule.wait_for_idle();
    rule.on_node_with_content_description("Sonraki aya değiştir").assert_exists();
    rule.on_node_with_text("17 Ağu 2026").assert_exists(); // the headline: the locale now
    rule.on_node_with_text("August 2026").assert_exists(); // the state's locale: English still
    rule.on_node_with_text("Monday, August 17, 2026").assert_is_selected();
    assert!(rule.on_node_with_text("ÖÖ").exists() && rule.on_node_with_text("ÖS").exists());
    assert!(!rule.on_node_with_text("AM").exists());
    assert!(shows(&rule, "Saat") && shows(&rule, "Dakika") && !shows(&rule, "Hour"));
    assert!(rule.on_node_with_text("Tarihleri seçin").exists() && !rule.on_node_with_text("Select dates").exists());
    assert!(rule.on_node_with_text("March 2026").exists());
    set_today_for_tests(None);
}

// ---- Helpers ---------------------------------------------------------------------------------------------------

const DAY: i64 = 86_400_000;

/// UTC milliseconds of a date's midnight.
fn millis(year: i32, month: u32, day: u32) -> i64 {
    // Days from the civil date (Howard Hinnant's), as the pickers count.
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * DAY
}

#[kompozable]
fn Turkish(content: impl Fn() + 'static) {
    KompozitionLocalProvider(&[LocalStrings.provides(Strings::turkish())], content);
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

fn shows(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

fn says(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().join(" ").contains(text)
}

fn select_all_and_type(rule: &KompozTestRule, digits: &str) {
    rule.host().key_down(Key::A, KeyModifiers::Ctrl);
    rule.host().key_up(Key::A, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    rule.host().text_input(digits);
    rule.wait_for_idle();
}

fn swipe(rule: &KompozTestRule, from: Offset, to: Offset) {
    rule.on_root().perform_touch_input(|s| s.swipe_with(from, to, 200));
    rule.wait_for_idle();
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

/// The C++ test's calendar, as Compose's: Turkish (the C++'s `DatePickerStrings::turkish()` is Compose's texts in
/// the locale), weeks from Monday (CLDR's for Turkey), a day's node is its full date read out (Compose's day
/// semantics: `text` "17 Ağustos 2026 Pazartesi", not the C++'s content description "17 Ağustos 2026"), the headline
/// the medium date ("17 Ağu 2026"), "Sonraki aya değiştir" the next month's arrow, the year button opening the years
/// (each read out "2030 yılına gidin"). Compose's selection stays where it was picked when the month or the year
/// changes (the C++ moved the picked day along with them, its own behaviour).
fn date_picker_picks_days_months_and_years() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let state: Rc<RefCell<Option<Rc<dyn DatePickerState>>>> = Rc::default();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Turkish(move || {
                let st = remember_date_picker_state(
                    None,
                    Some(Some(millis(2026, 8, 1))),
                    DatePickerDefaults::YearRange(),
                    DisplayMode::Picker,
                    DatePickerDefaults::AllDates(),
                );
                *s.borrow_mut() = Some(st.clone());
                DatePicker(st);
            })
        }),
        IntSize::new(400, 800),
    );
    let st = state.borrow().clone().unwrap();
    assert!(rule.on_node_with_text("Ağustos 2026").exists() && rule.on_node_with_text("Seçilen tarih").exists());
    assert!(rule.on_node_with_text("Tarih seç").exists());
    // Today, outlined and read out as today.
    assert!(rule.on_node_with_text("Bugün, 4 Ağustos 2026 Salı").exists());
    // 1 August 2026 is a Saturday: weeks start on Monday, so the 3rd opens the next row.
    let first = rule.on_node_with_text("1 Ağustos 2026 Cumartesi").get_unclipped_bounds_in_root();
    let third = rule.on_node_with_text("3 Ağustos 2026 Pazartesi").get_unclipped_bounds_in_root();
    assert!((third.top - (first.top + 48.0)).abs() < 0.5 && third.left < first.left, "{first:?} {third:?}");
    rule.on_node_with_text("17 Ağustos 2026 Pazartesi").perform_click();
    rule.wait_for_idle();
    assert_eq!(st.selected_date_millis(), Some(millis(2026, 8, 17)));
    assert!(rule.on_node_with_text("17 Ağu 2026").exists());
    rule.on_node_with_content_description("Sonraki aya değiştir").perform_click();
    rule.wait_for_idle();
    assert_eq!(st.displayed_month_millis(), millis(2026, 9, 1));
    assert!(rule.on_node_with_text("Eylül 2026").exists());
    // (Compose: the picked date stays the 17th of August.)
    assert_eq!(st.selected_date_millis(), Some(millis(2026, 8, 17)));
    assert!(rule.on_node_with_text("17 Ağu 2026").exists());
    let y = rule.on_node_with_text("16 Eylül 2026 Çarşamba").get_unclipped_bounds_in_root().center().y;
    swipe(&rule, Offset::new(60.0, y), Offset::new(360.0, y)); // back a month
    assert_eq!(st.displayed_month_millis(), millis(2026, 8, 1));
    rule.on_node_with_text("Ağustos 2026").perform_click(); // the year grid, around 2026
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("2030 yılına gidin").exists());
    // (Compose's year grid grows over the month, which stays composed under it: the C++ removed the days.)
    rule.on_node_with_text("2030 yılına gidin").perform_click();
    rule.wait_for_idle();
    assert_eq!(st.displayed_month_millis(), millis(2030, 8, 1));
    assert!(rule.on_node_with_text("Ağustos 2030").exists());
    assert!(rule.on_node_with_text("31 Ağustos 2030 Cumartesi").exists());
    assert_eq!(st.selected_date_millis(), Some(millis(2026, 8, 17)));
    rule.on_node_with_text("31 Ağustos 2030 Cumartesi").perform_click();
    rule.on_node_with_content_description("Sonraki aya değiştir").perform_click();
    rule.wait_for_idle();
    assert_eq!(st.selected_date_millis(), Some(millis(2030, 8, 31)));
    assert_eq!(st.displayed_month_millis(), millis(2030, 9, 1));
    st.set_displayed_month_millis(millis(2031, 1, 1)); // from code: only shows the month
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("Ocak 2031").exists());
    assert_eq!(st.selected_date_millis(), Some(millis(2030, 8, 31)));
    set_today_for_tests(None);
}

/// The C++ test as Compose's input mode: Turkish's short date "d.MM.y" typed as "DD.MM.YYYY" (Compose's
/// `datePatternAsInputFormat`), the delimiters shown as typed (Compose's visual transformation), the field labelled
/// "Tarih" and read out with the pattern, Compose's error texts.
fn date_picker_takes_a_typed_date_in_input_mode() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let state: Rc<RefCell<Option<Rc<dyn DatePickerState>>>> = Rc::default();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Turkish(move || {
                let st = remember_date_picker_state(
                    None,
                    Some(Some(millis(2026, 8, 1))),
                    2000..=2030,
                    DisplayMode::Picker,
                    DatePickerDefaults::AllDates(),
                );
                *s.borrow_mut() = Some(st.clone());
                DatePicker(st);
            })
        }),
        IntSize::new(400, 800),
    );
    let st = state.borrow().clone().unwrap();
    rule.on_node_with_content_description("Metin giriş moduna geç").perform_click(); // the pencil by the headline
    rule.wait_for_idle();
    assert_eq!(st.display_mode(), DisplayMode::Input);
    assert!(rule.on_node_with_text("Girilen tarih").exists() && !rule.on_node_with_text("Ağustos 2026").exists());
    let field = rule.on_node_with_content_description("Tarih, DD.MM.YYYY").get_unclipped_bounds_in_root();
    tap(&rule, field.center());
    rule.host().text_input("1508"); // half typed: the delimiters show, nothing picked yet
    rule.wait_for_idle();
    assert!(shows(&rule, "15.08.") && st.selected_date_millis().is_none());
    rule.host().text_input("2026");
    rule.wait_for_idle();
    assert!(shows(&rule, "15.08.2026"));
    assert_eq!(st.selected_date_millis(), Some(millis(2026, 8, 15)));
    assert!(shows(&rule, "15 Ağu 2026"));
    rule.host().text_input("7"); // a ninth digit: refused
    rule.wait_for_idle();
    assert!(shows(&rule, "15.08.2026"));
    assert_eq!(st.selected_date_millis(), Some(millis(2026, 8, 15)));
    rule.host().key_down(Key::Backspace, KeyModifiers::None); // a digit short again: nothing picked
    rule.host().key_up(Key::Backspace, KeyModifiers::None);
    rule.wait_for_idle();
    assert!(shows(&rule, "15.08.202") && st.selected_date_millis().is_none());
    select_all_and_type(&rule, "31022026"); // no such day
    assert!(st.selected_date_millis().is_none());
    assert!(says(&rule, "Tarih, istenen biçimle eşleşmiyor: DD.MM.YYYY"), "{:?}", rule.drawn_texts());
    select_all_and_type(&rule, "01012040"); // past the year range
    assert!(st.selected_date_millis().is_none() && says(&rule, "Tarih, istenen 2000-2030 yıl aralığının dışında"));
    select_all_and_type(&rule, "09122027");
    assert_eq!(st.selected_date_millis(), Some(millis(2027, 12, 9)));
    rule.on_node_with_content_description("Takvim giriş moduna geç").perform_click(); // the calendar, on its month
    rule.wait_for_idle();
    assert_eq!(st.display_mode(), DisplayMode::Picker);
    assert_eq!(st.selected_date_millis(), Some(millis(2027, 12, 9)));
    assert!(rule.on_node_with_text("Aralık 2027").exists());
    set_today_for_tests(None);
}

/// The C++ test as Compose's: the dialog's surface exactly 360 dp wide, centered in a 400 dp window (Compose for
/// desktop's `requiredWidth(ContainerWidth)` without the platform's width), the picked date confirmed by the
/// app's button, the dialog gone when the app stops composing it.
fn date_picker_dialog_confirms_the_date() {
    set_today_for_tests(Some(|| (2026, 3, 1)));
    let open = mutable_state_of(true);
    let confirmed: Rc<Cell<Option<i64>>> = Rc::default();
    let (o, c) = (open.clone(), confirmed.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (o, c) = (o.clone(), c.clone());
            Turkish(move || {
                if !o.get() {
                    return;
                }
                let st = remember_date_picker_state(Some(millis(2026, 3, 5)));
                let (o1, o2, o3, c1, s1) = (o.clone(), o.clone(), o.clone(), c.clone(), st.clone());
                DatePickerDialog(
                    move || o1.set(false),
                    move || {
                        let (o2, c1, s1) = (o2.clone(), c1.clone(), s1.clone());
                        TextButton(
                            move || {
                                c1.set(s1.selected_date_millis());
                                o2.set(false);
                            },
                            content = || Text("Tamam"),
                        );
                    },
                    dismiss_button = Some(Rc::new(move || {
                        let o3 = o3.clone();
                        TextButton(move || o3.set(false), content = || Text("Vazgeç"));
                    })),
                    content = {
                        let st = st.clone();
                        move || DatePicker(st.clone(), modifier = Modifier.test_tag("seçici"))
                    },
                );
            })
        }),
        IntSize::new(400, 800),
    );
    assert!(rule.on_node_with_text("5 Mar 2026").exists() && rule.on_node_with_text("Mart 2026").exists());
    let picker = rule.on_node_with_tag("seçici").get_unclipped_bounds_in_root();
    assert!((picker.width() - 360.0).abs() < 0.5 && (picker.left - 20.0).abs() < 0.5, "{picker:?}");
    rule.on_node_with_text("20 Mart 2026 Cuma").perform_click();
    rule.on_node_with_text("Tamam").perform_click();
    rule.wait_for_idle();
    assert_eq!(confirmed.get(), Some(millis(2026, 3, 20)));
    assert!(!open.peek() && rule.host().overlay_count() == 0);
    set_today_for_tests(None);
}

/// The C++ test as Compose's range input: "Başlangıç tarihi" and "Bitiş tarihi" fields (read out with the pattern),
/// an end before the start said and not picked. Compose's state takes a selection as a whole: a start typed after
/// the end clears the range (an end without a start isn't one; the error, validated again against no end, isn't
/// said), so the start typed right after picks a start alone — the end's field still shows its digits (the C++
/// said the error and kept the end picked).
fn date_range_picker_takes_typed_dates_in_input_mode() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let state: Rc<RefCell<Option<Rc<dyn DateRangePickerState>>>> = Rc::default();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Turkish(move || {
                let st = remember_date_range_picker_state(
                    None,
                    None,
                    Some(Some(millis(2026, 8, 1))),
                    1900..=2100,
                    DisplayMode::Input,
                    DatePickerDefaults::AllDates(),
                );
                *s.borrow_mut() = Some(st.clone());
                DateRangePicker(st);
            })
        }),
        IntSize::new(360, 800),
    );
    let st = state.borrow().clone().unwrap();
    let type_in = |field: &str, digits: &str| {
        let at = rule.on_node_with_content_description(field).get_unclipped_bounds_in_root().center();
        tap(&rule, at);
        select_all_and_type(&rule, digits);
    };
    let (start, end) = ("Başlangıç tarihi, DD.MM.YYYY", "Bitiş tarihi, DD.MM.YYYY");
    assert!(
        rule.on_node_with_content_description(start).exists() && rule.on_node_with_content_description(end).exists()
    );
    type_in(start, "10082026");
    assert_eq!((st.selected_start_date_millis(), st.selected_end_date_millis()), (Some(millis(2026, 8, 10)), None));
    type_in(end, "05082026"); // before the start: said, not picked
    assert!(says(&rule, "Geçersiz tarih aralığı girişi") && st.selected_end_date_millis().is_none());
    type_in(end, "20082026");
    assert!(!says(&rule, "Geçersiz tarih aralığı girişi"));
    assert_eq!(st.selected_end_date_millis(), Some(millis(2026, 8, 20)));
    // After the end: the state drops the range (an end without a start isn't one), and the field's error is
    // validated again as its text changed (Compose's `rememberSaveable(text)`) against no end: none is said.
    type_in(start, "25082026");
    assert!(!says(&rule, "Geçersiz tarih aralığı girişi"), "{:?}", rule.drawn_texts());
    assert_eq!((st.selected_start_date_millis(), st.selected_end_date_millis()), (None, None));
    type_in(start, "12082026");
    assert_eq!((st.selected_start_date_millis(), st.selected_end_date_millis()), (Some(millis(2026, 8, 12)), None));
    assert!(shows(&rule, "20.08.2026"));
    rule.on_node_with_content_description("Takvim giriş moduna geç").perform_click();
    rule.wait_for_idle();
    assert_eq!(st.display_mode(), DisplayMode::Picker);
    assert!(rule.on_node_with_text("Ağustos 2026").exists() && shows(&rule, "12 Ağu 2026"));
    // The mode button stays at the header's end: its 40 dp button centered in its 48 dp touch target, 12 dp in
    // from the end (the C++ measured its 24 dp icon).
    let button = rule.on_node_with_content_description("Metin giriş moduna geç").get_unclipped_bounds_in_root();
    assert!((button.right - (360.0 - 12.0 - 4.0)).abs() < 0.5 && (button.width() - 40.0).abs() < 0.5, "{button:?}");
    set_today_for_tests(None);
}

/// The C++ test as Compose's range picker: the months one under another (no arrows, no year grid, no start/end
/// tabs in Compose), a tap starting the range, a later one ending it, an earlier one starting it again, the days
/// between read out "Aralıkta", the ends "Başlangıç tarihi" / "Bitiş tarihi"; `set_selection` with an end before
/// the start picks nothing (Compose's; the C++ kept the start).
fn date_range_picker_picks_a_range() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let state: Rc<RefCell<Option<Rc<dyn DateRangePickerState>>>> = Rc::default();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Turkish(move || {
                let st = remember_date_range_picker_state(None, None, Some(Some(millis(2026, 8, 1))));
                *s.borrow_mut() = Some(st.clone());
                DateRangePicker(st);
            })
        }),
        IntSize::new(400, 800),
    );
    let st = state.borrow().clone().unwrap();
    // The headline's texts are one description to accessibility (Compose's clearAndSetSemantics): drawn.
    // (the end's placeholder wraps in the headline's room: "Bitiş" / "tarihi")
    assert!(shows(&rule, "Başlangıç tarihi") && shows(&rule, "Bitiş"), "{:?}", rule.drawn_texts());
    rule.on_node_with_content_description("Başlangıç tarihi: Yok, Bitiş tarihi: Yok").assert_exists();
    assert!(rule.on_node_with_text("Ağustos 2026").exists() && rule.on_node_with_text("Tarihleri seçin").exists());
    rule.on_node_with_text("12 Ağustos 2026 Çarşamba").perform_click();
    rule.wait_for_idle();
    assert_eq!((st.selected_start_date_millis(), st.selected_end_date_millis()), (Some(millis(2026, 8, 12)), None));
    assert!(shows(&rule, "12 Ağu 2026"));
    // (a start alone isn't a range yet: the day reads out its date only)
    rule.on_node_with_text("12 Ağustos 2026 Çarşamba").assert_exists();
    rule.on_node_with_text("1 Ağustos 2026 Cumartesi").perform_click(); // earlier: a new start
    rule.wait_for_idle();
    assert_eq!(st.selected_start_date_millis(), Some(millis(2026, 8, 1)));
    rule.on_node_with_text("20 Ağustos 2026 Perşembe").perform_click(); // later: the end
    rule.wait_for_idle();
    assert_eq!(st.selected_end_date_millis(), Some(millis(2026, 8, 20)));
    rule.on_node_with_text("Başlangıç tarihi, 1 Ağustos 2026 Cumartesi").assert_exists();
    rule.on_node_with_text("Aralıkta, 15 Ağustos 2026 Cumartesi").assert_exists();
    rule.on_node_with_text("Bitiş tarihi, 20 Ağustos 2026 Perşembe").assert_exists();
    rule.on_node_with_text("21 Ağustos 2026 Cuma").assert_exists();
    assert!(shows(&rule, "20 Ağu 2026"));
    rule.on_node_with_text("25 Ağustos 2026 Salı").perform_click(); // a whole range already: a new start
    rule.wait_for_idle();
    assert_eq!((st.selected_start_date_millis(), st.selected_end_date_millis()), (Some(millis(2026, 8, 25)), None));
    // The month names scroll a month on for accessibility.
    rule.on_node_with_text("Ağustos 2026")
        .perform_custom_accessibility_action_with_label("Sonraki ayı göstermek için kaydırın");
    rule.wait_for_idle();
    assert_eq!(st.displayed_month_millis(), millis(2026, 9, 1));
    st.set_selection(Some(millis(2026, 5, 9)), Some(millis(2026, 5, 1))); // an end before the start: none
    assert_eq!((st.selected_start_date_millis(), st.selected_end_date_millis()), (None, None));
    st.set_selection(None, Some(millis(2026, 5, 1))); // an end without a start: none
    assert_eq!(st.selected_end_date_millis(), None);
    set_today_for_tests(None);
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// Compose's states: dates outside the year range aren't picked (or shown: the current month then), the display
/// mode shows the picked date's month, the saver keeps the selection, the month, the years and the mode.
fn date_picker_states_keep_to_their_years_and_save() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let en = CalendarLocale::for_language_tag("en-US");
    let st = named_args!(date_picker_state(en.clone(), initial_selected_date_millis = Some(millis(1850, 1, 1))));
    assert_eq!(st.selected_date_millis(), None);
    assert_eq!(st.displayed_month_millis(), millis(2026, 8, 1)); // out of the years: today's month
    st.set_selected_date_millis(Some(millis(2030, 2, 3) + 5 * 3_600_000)); // the day's midnight
    assert_eq!(st.selected_date_millis(), Some(millis(2030, 2, 3)));
    st.set_selected_date_millis(Some(millis(2200, 1, 1)));
    assert_eq!(st.selected_date_millis(), None);
    st.set_selected_date_millis(Some(millis(2030, 2, 3)));
    st.set_displayed_month_millis(millis(1899, 12, 1)); // out of the years: kept
    assert_eq!(st.displayed_month_millis(), millis(2026, 8, 1));
    st.set_display_mode(DisplayMode::Input);
    assert_eq!((st.display_mode(), st.displayed_month_millis()), (DisplayMode::Input, millis(2030, 2, 1)));
    assert_eq!(
        format!("{} {} {:?}", DisplayMode::Picker, DisplayMode::Input, DisplayMode::Input),
        "Picker Input Input"
    );
    assert_eq!(st.locale(), en);
    // Saved and restored with the UI's state.
    // (one composable for both compositions: the saved values are found by their place in it)
    type Shown = Rc<RefCell<Option<Rc<dyn DatePickerState>>>>;
    fn saved_picker(registry: &SaveableStateRegistry, shown: &Shown, initial: Option<i64>) -> KompozTestRule {
        let (r, s) = (registry.clone(), shown.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = s.clone();
            ProvideSaveableStateRegistry(&r, move || {
                *s.borrow_mut() = Some(remember_date_picker_state(initial, year_range = 2000..=2050));
            });
        }))
    }
    let registry = SaveableStateRegistry::new(None);
    let shown: Shown = Rc::default();
    let rule = saved_picker(&registry, &shown, Some(millis(2026, 8, 17)));
    let first = shown.borrow().clone().unwrap();
    first.set_display_mode(DisplayMode::Input);
    let saved = registry.perform_save();
    drop(rule);
    let _rule = saved_picker(&SaveableStateRegistry::new(Some(&saved)), &shown, None);
    let restored = shown.borrow().clone().unwrap();
    assert!(!Rc::ptr_eq(&first, &restored));
    assert_eq!(restored.selected_date_millis(), Some(millis(2026, 8, 17)));
    assert_eq!((restored.year_range(), restored.display_mode()), (2000..=2050, DisplayMode::Input));
    // The range state's saver too.
    let range = named_args!(date_range_picker_state(
        en,
        initial_selected_start_date_millis = Some(millis(2026, 8, 20)),
        initial_selected_end_date_millis = Some(millis(2026, 8, 10)),
    ));
    assert_eq!((range.selected_start_date_millis(), range.selected_end_date_millis()), (None, None));
    set_today_for_tests(None);
}

/// Weekends not selectable, years before 2025 neither (as DatePickerWithDateSelectableDatesSample): their days and
/// years show disabled and don't pick; a typed date of them is said not allowed.
fn date_picker_disables_the_dates_it_may_not_pick() {
    struct Weekdays;
    impl SelectableDates for Weekdays {
        fn is_selectable_date(&self, utc_time_millis: i64) -> bool {
            // 1 January 1970 was a Thursday: Saturday and Sunday are 2 and 3 days after a Thursday.
            !matches!((utc_time_millis / DAY).rem_euclid(7), 2 | 3)
        }
        fn is_selectable_year(&self, year: i32) -> bool {
            year > 2024
        }
    }
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let state: Rc<RefCell<Option<Rc<dyn DatePickerState>>>> = Rc::default();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            let st = remember_date_picker_state(
                None,
                Some(Some(millis(2026, 8, 1))),
                DatePickerDefaults::YearRange(),
                DisplayMode::Picker,
                remember(|| Rc::new(Weekdays) as Rc<dyn SelectableDates>),
            );
            *s.borrow_mut() = Some(st.clone());
            DatePicker(st);
        }),
        IntSize::new(400, 800),
    );
    let st = state.borrow().clone().unwrap();
    rule.on_node_with_text("Saturday, August 1, 2026").assert_is_not_enabled();
    rule.on_node_with_text("Saturday, August 1, 2026").perform_click();
    assert_eq!(st.selected_date_millis(), None);
    rule.on_node_with_text("Monday, August 3, 2026").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    assert_eq!(st.selected_date_millis(), Some(millis(2026, 8, 3)));
    rule.on_node_with_text("August 2026").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Navigate to year 2024").assert_is_not_enabled();
    rule.on_node_with_text("Navigate to year 2025").assert_is_enabled();
    set_today_for_tests(None);
}

/// The default formatter writes the skeletons' dates in the locale (equal to another of the same skeletons); the
/// default colors are the scheme's, `copy` keeps what's unspecified; the headline is the default's.
fn date_picker_formats_and_colors_by_its_defaults() {
    let en = CalendarLocale::for_language_tag("en-US");
    let f = DatePickerDefaults.date_formatter();
    assert_eq!(f.format_month_year(Some(millis(2026, 8, 17)), &en).as_deref(), Some("August 2026"));
    assert_eq!(f.format_date(Some(millis(2026, 8, 17)), &en, false).as_deref(), Some("Aug 17, 2026"));
    assert_eq!(f.format_date(Some(millis(2026, 8, 17)), &en, true).as_deref(), Some("Monday, August 17, 2026"));
    assert_eq!(f.format_date(None, &en, false), None);
    assert!(*f == *DatePickerDefaults.date_formatter());
    let custom = named_args!(DatePickerDefaults.date_formatter(selected_date_skeleton = "dd/MM/yyyy".to_string()));
    assert_eq!(custom.format_date(Some(millis(2026, 8, 17)), &en, false).as_deref(), Some("17/08/2026"));
    assert!(*f != *custom);
    let colors: Rc<RefCell<Vec<DatePickerColors>>> = Rc::default();
    let c = colors.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let scheme = MaterialTheme::color_scheme();
        let defaults = DatePickerDefaults.colors();
        assert_eq!(defaults.container_color, scheme.surface_container_high);
        assert_eq!(defaults.selected_day_container_color, scheme.primary);
        assert_eq!(defaults.today_date_border_color, scheme.primary);
        assert_eq!(defaults.navigation_content_color, scheme.on_surface_variant);
        assert_eq!(defaults.disabled_day_content_color, named_args!(scheme.on_surface.copy(alpha = 0.38)));
        let custom = named_args!(DatePickerDefaults.colors(container_color = Color::Red));
        c.borrow_mut().extend([defaults, custom]);
    }));
    let colors = colors.borrow();
    assert_eq!(colors[1].container_color, Color::Red);
    assert_eq!(colors[1].day_content_color, colors[0].day_content_color);
    assert!(
        colors[0] != colors[1] && colors[0] == named_args!(colors[1].copy(container_color = colors[0].container_color))
    );
    assert_eq!(DatePickerDefaults::TonalElevation, Dp(0.0));
    assert_eq!(DatePickerDefaults::YearRange(), 1900..=2100);
}

/// What a screen reader hears: the headline's "Current selection: …" (politely), the year button's month, the
/// weekdays' full names, today's "Today, …".
fn date_picker_headline_and_days_read_out_their_dates() {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| DatePicker(remember_date_picker_state(Some(millis(2026, 8, 17))))),
        IntSize::new(400, 800),
    );
    rule.on_node_with_content_description("Current selection: Monday, August 17, 2026").assert_exists();
    rule.on_node_with_text("Aug 17, 2026").assert_exists();
    rule.on_node_with_content_description("August 2026").assert_exists();
    rule.on_node_with_content_description("Sunday").assert_exists();
    rule.on_node_with_text("Today, Tuesday, August 4, 2026").assert_exists();
    rule.on_node_with_text("Select date").assert_exists();
    // US weeks start on Sunday: Sunday's letter first.
    let sunday = rule.on_node_with_content_description("Sunday").get_unclipped_bounds_in_root();
    let monday = rule.on_node_with_content_description("Monday").get_unclipped_bounds_in_root();
    assert!(sunday.left < monday.left);
    set_today_for_tests(None);
}

// ---- The M15 samples (DatePickerSamples.kt) ---------------------------------------------------------------------

#[kompozable]
fn DatePickerSample() {
    Column(
        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = || {
            // Pre-select a date for January 4, 2020
            let date_picker_state = remember_date_picker_state(Some(1578096000000));
            DatePicker(date_picker_state.clone(), modifier = Modifier.padding(16.dp()));

            Text(
                format!(
                    "Selected date timestamp: {}",
                    date_picker_state.selected_date_millis().map_or("no selection".to_string(), |m| m.to_string())
                ),
                modifier = Modifier.align(Alignment::CenterHorizontally),
            );
        },
    );
}

#[kompozable]
fn DatePickerDialogSample() {
    // Decoupled snackbar host state from scaffold state for demo purposes.
    let snack_state = remember(SnackbarHostState::new);
    let snack_scope = remember_coroutine_scope();
    SnackbarHost(snack_state.clone(), Modifier);
    let open_dialog = remember(|| mutable_state_of(true));
    // TODO demo how to read the selected date from the state.
    if open_dialog.get() {
        let date_picker_state = remember_date_picker_state();
        let confirm_enabled = {
            let s = date_picker_state.clone();
            remember(move || derived_state_of(move || s.selected_date_millis().is_some()))
        };
        let (o1, o2, o3) = (open_dialog.clone(), open_dialog.clone(), open_dialog.clone());
        let st = date_picker_state.clone();
        DatePickerDialog(
            move || {
                // Dismiss the dialog when the user clicks outside the dialog or on the back button. If you want to
                // disable that functionality, simply use an empty onDismissRequest.
                o1.set(false)
            },
            move || {
                let (o2, st, snack_state, snack_scope) =
                    (o2.clone(), st.clone(), snack_state.clone(), snack_scope.clone());
                TextButton(
                    move || {
                        o2.set(false);
                        let (snack_state, millis) = (snack_state.clone(), st.selected_date_millis());
                        snack_scope.launch(async move {
                            snack_state
                                .show_snackbar(format!(
                                    "Selected date timestamp: {}",
                                    millis.map_or("null".into(), |m| m.to_string())
                                ))
                                .await;
                        });
                    },
                    enabled = confirm_enabled.get(),
                    content = || Text("OK"),
                );
            },
            dismiss_button = Some(Rc::new(move || {
                let o3 = o3.clone();
                TextButton(move || o3.set(false), content = || Text("Cancel"));
            })),
            content = move || {
                // The verticalScroll will allow scrolling to show the entire month in case there is not enough
                // horizontal space (for example, when in landscape mode). Note that it's still currently
                // recommended to use a DisplayMode.Input at the state in those cases.
                DatePicker(date_picker_state.clone(), modifier = Modifier.vertical_scroll(&remember_scroll_state()));
            },
        );
    }
}

#[kompozable]
fn DatePickerWithDateSelectableDatesSample() {
    struct Selectable;
    impl SelectableDates for Selectable {
        // Blocks Sunday and Saturday from being selected. (Kotlin: java.time's day of the week of the UTC day.)
        fn is_selectable_date(&self, utc_time_millis: i64) -> bool {
            !matches!((utc_time_millis / DAY).rem_euclid(7), 2 | 3)
        }

        // Allow selecting dates from year 2023 forward.
        fn is_selectable_year(&self, year: i32) -> bool {
            year > 2022
        }
    }
    let date_picker_state =
        remember_date_picker_state(selectable_dates = remember(|| Rc::new(Selectable) as Rc<dyn SelectableDates>));
    Column(
        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = move || {
            DatePicker(date_picker_state.clone());
            Text(
                format!(
                    "Selected date timestamp: {}",
                    date_picker_state.selected_date_millis().map_or("no selection".to_string(), |m| m.to_string())
                ),
                modifier = Modifier.align(Alignment::CenterHorizontally),
            );
        },
    );
}

#[kompozable]
fn DateInputSample() {
    Column(
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = || {
            let state = remember_date_picker_state(initial_display_mode = DisplayMode::Input);
            DatePicker(state.clone(), modifier = Modifier.padding(16.dp()));

            Text(
                format!(
                    "Entered date timestamp: {}",
                    state.selected_date_millis().map_or("no input".to_string(), |m| m.to_string())
                ),
                modifier = Modifier.align(Alignment::CenterHorizontally),
            );
        },
    );
}

fn date_picker_samples() {
    set_today_for_tests(Some(|| (2023, 4, 3)));
    // DatePickerSample: January 2020 shown, the 4th picked, its timestamp under the picker.
    let rule = KompozTestRule::with_viewport(DatePickerSample, IntSize::new(400, 900));
    rule.on_node_with_text("January 2020").assert_exists();
    rule.on_node_with_text("Selected date timestamp: 1578096000000").assert_exists();
    rule.on_node_with_text("Monday, January 6, 2020").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("Selected date timestamp: {}", millis(2020, 1, 6))).assert_exists();
    // DatePickerDialogSample: OK disabled until a date is picked, then a snackbar with its timestamp.
    let rule = KompozTestRule::with_viewport(DatePickerDialogSample, IntSize::new(400, 900));
    rule.on_node_with_text("OK").assert_is_not_enabled();
    rule.on_node_with_text("Wednesday, April 12, 2023").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("OK").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
    rule.on_node_with_text(&format!("Selected date timestamp: {}", millis(2023, 4, 12))).assert_exists();
    // DatePickerWithDateSelectableDatesSample: weekends and 2022 disabled.
    let rule = KompozTestRule::with_viewport(DatePickerWithDateSelectableDatesSample, IntSize::new(400, 900));
    rule.on_node_with_text("Saturday, April 1, 2023").assert_is_not_enabled();
    rule.on_node_with_text("Today, Monday, April 3, 2023").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("Selected date timestamp: {}", millis(2023, 4, 3))).assert_exists();
    // DateInputSample: typed as the locale's MM/DD/YYYY.
    let rule = KompozTestRule::with_viewport(DateInputSample, IntSize::new(400, 900));
    rule.on_node_with_text("Entered date timestamp: no input").assert_exists();
    let field = rule.on_node_with_content_description("Date, MM/DD/YYYY").get_unclipped_bounds_in_root();
    tap(&rule, field.center());
    rule.host().text_input("04152023");
    rule.wait_for_idle();
    assert!(shows(&rule, "04/15/2023"));
    rule.on_node_with_text(&format!("Entered date timestamp: {}", millis(2023, 4, 15))).assert_exists();
    // DatePickerCustomLocaleSample (and its Api26 form): a state in Hebrew, right to left (Android's Configuration
    // and LocaleList there; the state's locale and the layout direction here): its months and days in Hebrew.
    let he = CalendarLocale::for_language_tag("HE");
    let month = DatePickerDefaults.date_formatter().format_month_year(Some(1578096000000), &he).unwrap();
    let day = DatePickerDefaults.date_formatter().format_date(Some(1578096000000), &he, true).unwrap();
    assert_ne!(month, "January 2020");
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                let date_picker_state = remember(|| {
                    named_args!(date_picker_state_of_date(
                        CalendarLocale::for_language_tag("HE"),
                        Some(LocalDate::of(2020, 1, 4)),
                    ))
                });
                DatePicker(date_picker_state, modifier = Modifier.padding(16.dp()));
            });
        }),
        IntSize::new(400, 900),
    );
    rule.on_node_with_text(&month).assert_exists();
    rule.on_node_with_text(&day).assert_is_selected();
    set_today_for_tests(None);
}

#[kompozable]
fn DateRangePickerSample() {
    // Decoupled snackbar host state from scaffold state for demo purposes.
    let snack_state = remember(SnackbarHostState::new);
    let snack_scope = remember_coroutine_scope();
    SnackbarHost(snack_state.clone(), Modifier.z_index(1.0));

    let state = remember_date_range_picker_state();
    Column(
        modifier = Modifier.fill_max_size(),
        vertical_arrangement = Arrangement::Top,
        content = move || {
            // Add a row with "Save" and dismiss actions.
            let (state, snack_state, snack_scope) = (state.clone(), snack_state.clone(), snack_scope.clone());
            let save_state = state.clone();
            Row(
                modifier = named_args!(
                    Modifier
                        .fill_max_width()
                        .background(DatePickerDefaults.colors().container_color)
                        .padding_sides(start = 12.dp(), end = 12.dp())
                ),
                vertical_alignment = Alignment::CenterVertically,
                horizontal_arrangement = Arrangement::SpaceBetween,
                content = move || {
                    TooltipBox(
                        position_provider =
                            TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        tooltip = |s: &dyn TooltipScope| s.PlainTooltip(content = || Text("Close")),
                        state = remember_tooltip_state(),
                        content = || {
                            IconButton(
                                || { /* dismiss the UI */ },
                                content = || {
                                    VectorIcon(
                                        kompoz_material_icons_core::Icons::Filled::Close(),
                                        Some("Close".to_string()),
                                    )
                                },
                            )
                        },
                    );
                    let (state, snack_state, snack_scope) =
                        (save_state.clone(), snack_state.clone(), snack_scope.clone());
                    let enabled = state.selected_end_date_millis().is_some();
                    TextButton(
                        move || {
                            let (start, end) = (
                                state.selected_start_date_millis().unwrap(),
                                state.selected_end_date_millis().unwrap(),
                            );
                            let snack_state = snack_state.clone();
                            snack_scope.launch(async move {
                                // Kotlin's LongRange prints as "start..end".
                                snack_state.show_snackbar(format!("Saved range (timestamps): {start}..{end}")).await;
                            });
                        },
                        enabled = enabled,
                        content = || Text("Save"),
                    );
                },
            );
            DateRangePicker(state.clone(), modifier = Modifier.weight(1.0));
        },
    );
}

#[kompozable]
fn DatePickerApi26Sample() {
    Column(
        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = || {
            // Pre-select a date for April 15, 2023
            let date_picker_state = remember_date_picker_state_of_date(Some(LocalDate::of(2023, 4, 15)));
            DatePicker(date_picker_state.clone(), modifier = Modifier.padding(16.dp()));

            Text(
                format!(
                    "Selected date: {}",
                    date_picker_state.get_selected_date().map_or("no selection".to_string(), |d| d.to_string())
                ),
                modifier = Modifier.align(Alignment::CenterHorizontally),
            );
        },
    );
}

#[kompozable]
fn DateRangePickerApi26Sample() {
    // Decoupled snackbar host state from scaffold state for demo purposes.
    let snack_state = remember(SnackbarHostState::new);
    let snack_scope = remember_coroutine_scope();
    SnackbarHost(snack_state.clone(), Modifier.z_index(1.0));

    // Creates a state with pre-selected date range.
    let state =
        remember_date_range_picker_state_of_dates(Some(LocalDate::of(2023, 4, 15)), Some(LocalDate::of(2023, 4, 20)));
    Column(
        modifier = Modifier.fill_max_size(),
        vertical_arrangement = Arrangement::Top,
        content = move || {
            // Add a row with "Save" and dismiss actions.
            let (state, snack_state, snack_scope) = (state.clone(), snack_state.clone(), snack_scope.clone());
            let save_state = state.clone();
            Row(
                modifier = named_args!(
                    Modifier
                        .fill_max_width()
                        .background(DatePickerDefaults.colors().container_color)
                        .padding_sides(start = 12.dp(), end = 12.dp())
                ),
                vertical_alignment = Alignment::CenterVertically,
                horizontal_arrangement = Arrangement::SpaceBetween,
                content = move || {
                    TooltipBox(
                        position_provider =
                            TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        tooltip = |s: &dyn TooltipScope| s.PlainTooltip(content = || Text("Close")),
                        state = remember_tooltip_state(),
                        content = || {
                            IconButton(
                                || { /* dismiss the UI */ },
                                content = || {
                                    VectorIcon(
                                        kompoz_material_icons_core::Icons::Filled::Close(),
                                        Some("Close".to_string()),
                                    )
                                },
                            )
                        },
                    );
                    let (state, snack_state, snack_scope) =
                        (save_state.clone(), snack_state.clone(), snack_scope.clone());
                    let enabled = state.get_selected_end_date().is_some();
                    TextButton(
                        move || {
                            let (start, end) =
                                (state.get_selected_start_date().unwrap(), state.get_selected_end_date().unwrap());
                            let snack_state = snack_state.clone();
                            snack_scope.launch(async move {
                                // Kotlin's ClosedRange<LocalDate> prints as "start..end".
                                snack_state.show_snackbar(format!("Saved range: {start}..{end}")).await;
                            });
                        },
                        enabled = enabled,
                        content = || Text("Save"),
                    );
                },
            );
            DateRangePicker(state.clone(), modifier = Modifier.weight(1.0));
        },
    );
}

/// The `java.time` forms (Compose's JVM API): states made of and read as `LocalDate` / `YearMonth`, the M15
/// DatePickerApi26Sample and DateRangePickerApi26Sample.
fn date_pickers_take_and_give_java_time_dates() {
    set_today_for_tests(Some(|| (2023, 4, 3)));
    let en = CalendarLocale::for_language_tag("en-US");
    let st = named_args!(date_picker_state_of_date(en.clone(), Some(LocalDate::of(2023, 4, 15))));
    assert_eq!(st.selected_date_millis(), Some(millis(2023, 4, 15)));
    assert_eq!(
        (st.get_selected_date(), st.get_displayed_month()),
        (Some(LocalDate::of(2023, 4, 15)), YearMonth::of(2023, 4))
    );
    st.set_selected_date(Some(LocalDate::of(2024, 2, 29)));
    st.set_displayed_month(YearMonth::of(2025, 7));
    assert_eq!(
        (st.get_selected_date(), st.get_displayed_month()),
        (Some(LocalDate::of(2024, 2, 29)), YearMonth::of(2025, 7))
    );
    let range = named_args!(date_range_picker_state_of_dates(
        en,
        Some(LocalDate::of(2023, 4, 15)),
        Some(LocalDate::of(2023, 4, 20)),
        initial_displayed_month = Some(Some(YearMonth::of(2023, 6))),
    ));
    assert_eq!(range.get_displayed_month(), YearMonth::of(2023, 6));
    range.set_selection_of_dates(Some(LocalDate::of(2023, 5, 1)), Some(LocalDate::of(2023, 5, 9)));
    assert_eq!(
        (range.get_selected_start_date(), range.get_selected_end_date()),
        (Some(LocalDate::of(2023, 5, 1)), Some(LocalDate::of(2023, 5, 9)))
    );
    // DatePickerApi26Sample: April 2023 shown, the 15th picked and printed as java.time prints it.
    let rule = KompozTestRule::with_viewport(DatePickerApi26Sample, IntSize::new(400, 900));
    rule.on_node_with_text("April 2023").assert_exists();
    rule.on_node_with_text("Selected date: 2023-04-15").assert_exists();
    rule.on_node_with_text("Friday, April 21, 2023").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Selected date: 2023-04-21").assert_exists();
    // DateRangePickerApi26Sample: the 15th to the 20th picked, saved as dates.
    let rule = KompozTestRule::with_viewport(DateRangePickerApi26Sample, IntSize::new(400, 900));
    rule.on_node_with_text("Start date, Saturday, April 15, 2023").assert_exists();
    rule.on_node_with_text("Save").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Saved range: 2023-04-15..2023-04-20").assert_exists();
    set_today_for_tests(None);
}

fn date_range_picker_samples() {
    set_today_for_tests(Some(|| (2023, 4, 3)));
    let rule = KompozTestRule::with_viewport(DateRangePickerSample, IntSize::new(400, 900));
    rule.on_node_with_text("Save").assert_is_not_enabled();
    rule.on_node_with_text("Today, Monday, April 3, 2023").perform_click();
    rule.on_node_with_text("Friday, April 7, 2023").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Save").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("Saved range (timestamps): {}..{}", millis(2023, 4, 3), millis(2023, 4, 7)))
        .assert_exists();
    set_today_for_tests(None);
}
