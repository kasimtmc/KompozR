//! Material's alert dialogs as Compose's material3 (1.5.0-alpha17) has them, with Compose for desktop's actual: the
//! C++ Kompoz's tests by name (kompoz_tests.cpp; the Material halves of those kompoz-ui's windows.rs ports with a
//! dialog of its own), Compose's behaviour (the parts' paddings and styles, the defaults, the pane title, the
//! buttons right to left, the component override, the deprecated overload), and the M15 samples of
//! AlertDialogSamples.kt.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, SemanticsMatcher};
use kompoz_ui::back::BackHandler;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::SemanticsProperties;
use kompoz_ui::window::{Dialog, DialogProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    basic_alert_dialog_is_280_to_560_wide_and_can_skip_the_platform_margins,
    alert_dialog_buttons_wrap_with_the_confirming_one_above,
    alert_dialog_text_gives_way_to_its_buttons_in_a_short_window,
    dialog_draws_on_top_takes_input_and_dismisses,
    back_stops_at_a_dialog_that_cannot_be_dismissed,
    alert_dialog_parts_take_their_paddings_and_styles,
    alert_dialog_defaults_follow_the_tokens,
    alert_dialogs_are_panes_titled_dialog,
    alert_dialog_buttons_follow_the_layout_direction,
    basic_alert_dialogs_can_be_made_by_an_override,
    deprecated_alert_dialog_of_content_is_a_basic_one,
    alert_dialog_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.right, b.right, 0.01)
        && near(a.bottom, b.bottom, 0.01)
}

/// The color `text` was drawn in.
fn color_of(canvas: &RecordingCanvas, text: &str) -> Option<Color> {
    canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i])
}

/// The style's font size `text` was drawn with.
fn font_size_of(canvas: &RecordingCanvas, text: &str) -> Option<f32> {
    canvas.texts.iter().position(|t| t == text).map(|i| canvas.text_styles[i].font_size)
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

fn escape(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    taken
}

type Log = Rc<RefCell<Vec<String>>>;

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

fn basic_alert_dialog_is_280_to_560_wide_and_can_skip_the_platform_margins() {
    let wide = mutable_state_of(false);
    let platform = mutable_state_of(true);
    let fill = mutable_state_of(false);
    let (w, p, f) = (wide.clone(), platform.clone(), fill.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (w, f) = (w.clone(), f.clone());
            BasicAlertDialog(
                || {},
                modifier = Modifier.test_tag("diyalog"),
                properties = DialogProperties { use_platform_default_width: p.get(), ..DialogProperties::default() },
                content = move || {
                    let size = if f.get() {
                        Modifier.fill_max_width()
                    } else {
                        Modifier.width(if w.get() { 700.dp() } else { 100.dp() })
                    };
                    Box(Modifier.then(size).height(60.dp()).test_tag("içerik"));
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    // Small content gets the least width (280), centered; large is held to 560.
    let content = bounds(&rule, "içerik");
    assert!(near(content.width(), 280.0, 0.01) && near(content.center().x, 500.0, 0.5), "{content:?}");
    wide.set(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "içerik").width(), 560.0, 0.01));
    // In a 400 dp window: the platform's 48 dp margins leave 304; without them, all 400.
    rule.host().set_viewport_size(IntSize::new(400, 800));
    fill.set(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "içerik").width(), 304.0, 0.01), "{:?}", bounds(&rule, "içerik"));
    platform.set(false);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "içerik").width(), 400.0, 0.01));
}

fn alert_dialog_buttons_wrap_with_the_confirming_one_above() {
    let longer = mutable_state_of(false);
    let cs = light_color_scheme();
    let l = longer.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l1, l2) = (l.clone(), l.clone());
            AlertDialog(
                || {},
                move || {
                    let l1 = l1.clone();
                    TextButton(
                        || {},
                        modifier = Modifier.test_tag("onay"),
                        content = move || {
                            Text(if l1.get() { "Şimdi güncelle ve yeniden başlat" } else { "Tamam" });
                        },
                    );
                },
                dismiss_button = slot(move || {
                    TextButton(|| {}, modifier = Modifier.test_tag("ret"), content = || Text("Sonra"));
                    if l2.get() {
                        TextButton(
                            || {},
                            modifier = Modifier.test_tag("asla"),
                            content = || Text("Bir daha hiç sorma bana lütfen"),
                        );
                    }
                }),
                icon = slot(|| VectorIcon(Icons::Filled::Info(), Some("Bilgi".into()))),
                title = slot(|| Text("Güncelleme")),
                text = slot(|| Text("Yeni bir sürüm var.")),
                shape = std::sync::Arc::new(RoundedCornerShape(8.dp())),
                container_color = cs.primary_container,
                icon_content_color = cs.tertiary,
                title_content_color = cs.on_primary_container,
                text_content_color = cs.primary,
                modifier = Modifier.test_tag("diyalog"),
            );
        }),
        IntSize::new(400, 800),
    );
    // Side by side at the end, 8 dp apart.
    let (dialog, ok, later) = (bounds(&rule, "diyalog"), bounds(&rule, "onay"), bounds(&rule, "ret"));
    assert!(near(ok.right, dialog.right - 24.0, 0.5), "{dialog:?} {ok:?}");
    assert!(near(ok.left - later.right, 8.0, 0.5) && near(ok.top, later.top, 0.5), "{ok:?} {later:?}");
    // Its own colors.
    let canvas = rule.draw();
    let container = canvas
        .rects
        .iter()
        .zip(canvas.rect_colors.iter().zip(&canvas.rect_radii))
        .any(|(r, (c, radius))| *c == cs.primary_container && same_rect(*r, dialog) && near(*radius, 8.0, 0.01)); // with its own shape
    assert!(container);
    assert_eq!(color_of(&canvas, "Güncelleme"), Some(cs.on_primary_container));
    assert_eq!(color_of(&canvas, "Yeni bir sürüm var."), Some(cs.primary));
    assert_eq!(canvas.vector_colors, [cs.tertiary]);
    // Too long to share a row: one row each, all at the end, the confirming one on top. Compose's flow row fills its
    // rows in the buttons' order (confirm, "Sonra", then the long one; the C++ put each later one above the rows
    // before), 8 dp apart less what the 48 dp of room adds to a 40 dp button (0 here): the buttons (their
    // clickables, 40 dp, in 48 of room) 8 dp apart; the C++ had 12.
    longer.set(true);
    rule.wait_for_idle();
    let (dialog, ok, later, never) =
        (bounds(&rule, "diyalog"), bounds(&rule, "onay"), bounds(&rule, "ret"), bounds(&rule, "asla"));
    assert!(
        near(later.top - ok.bottom, 8.0, 0.5) && near(never.top - later.bottom, 8.0, 0.5),
        "{ok:?} {later:?} {never:?}"
    );
    for b in [ok, later, never] {
        assert!(near(b.right, dialog.right - 24.0, 0.5), "{b:?} {dialog:?}");
    }
}

fn alert_dialog_text_gives_way_to_its_buttons_in_a_short_window() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            AlertDialog(
                || {},
                || TextButton(|| {}, modifier = Modifier.test_tag("onay"), content = || Text("Tamam")),
                title = slot(|| Text("Başlık")),
                text = slot(|| {
                    Column(|| {
                        for i in 0..40 {
                            Text(format!("Satır {i}"));
                        }
                    });
                }),
                modifier = Modifier.test_tag("diyalog"),
            );
        }),
        IntSize::new(400, 500),
    );
    // The dialog keeps within the window's 24 dp margins, its button still in it (its 40 dp clickable in 48 dp of
    // room: Compose's button bounds; the C++'s were the room).
    let (dialog, ok) = (bounds(&rule, "diyalog"), bounds(&rule, "onay"));
    assert!(dialog.top >= 24.0 - 0.5 && dialog.bottom <= 500.0 - 24.0 + 0.5, "{dialog:?}");
    assert!(near(ok.bottom, dialog.bottom - 24.0 - 4.0, 0.5) && near(ok.height(), 40.0, 0.5), "{ok:?} {dialog:?}");
}

fn dialog_draws_on_top_takes_input_and_dismisses() {
    // C++'s dialog_draws_on_top_takes_input_and_dismisses, its Material half (kompoz-ui's windows.rs has it with a
    // Dialog of its own): Material's Button below, an AlertDialog over it.
    let open = mutable_state_of(false);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (o, l) = (open.clone(), log.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (o, l) = (o.clone(), l.clone());
            Column(move || {
                let lb = l.clone();
                Button(
                    move || lb.borrow_mut().push("below".into()),
                    modifier = Modifier.test_tag("below"),
                    content = || Text("Alttaki"),
                );
                if o.get() {
                    let (o2, l2, l3) = (o.clone(), l.clone(), l.clone());
                    AlertDialog(
                        move || {
                            l2.borrow_mut().push("dismiss".into());
                            o2.set(false);
                        },
                        move || {
                            let l3 = l3.clone();
                            TextButton(move || l3.borrow_mut().push("ok".into()), content = || Text("Tamam"));
                        },
                        title = slot(|| Text("Başlık")),
                        text = slot(|| Text("Gövde metni")),
                    );
                }
            });
        }),
        IntSize::new(400, 800),
    );
    open.set(true);
    rule.wait_for_idle();
    let texts = rule.drawn_texts();
    assert!(texts.len() == 4 && texts[0] == "Alttaki" && texts[1] == "Başlık", "{texts:?}"); // the dialog after the screen
    assert_eq!(rule.host().overlay_count(), 1);
    // Centered: Compose's dialog takes its least width, 280 dp, where its content needs no more (the C++'s took the
    // window less its 48 dp margins).
    let title = text_bounds(&rule, "Başlık");
    assert!(near(title.left, (400.0 - 280.0) / 2.0 + 24.0, 0.5) && title.top > 300.0, "{title:?}");
    rule.on_node_with_text("Gövde metni").perform_click(); // on the dialog: nothing happens
    assert!(taken(&log).is_empty());
    tap(&rule, bounds(&rule, "below").center()); // on the scrim: closes, doesn't click below
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["dismiss"]);
    rule.on_node_with_text("Başlık").assert_does_not_exist();
    assert_eq!(rule.host().overlay_count(), 0);
    open.set(true);
    rule.wait_for_idle();
    rule.on_node_with_text("Tamam").perform_click();
    assert_eq!(taken(&log), ["ok"]);
    assert!(escape(&rule)); // back closes it
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["dismiss"]);
    assert!(!open.peek());
    assert!(!escape(&rule)); // nothing left to go back from
}

fn back_stops_at_a_dialog_that_cannot_be_dismissed() {
    // C++'s back_stops_at_a_dialog_that_cannot_be_dismissed, its Material half (kompoz-ui's windows.rs has it with
    // the test Text): Material's Text in the Dialog, and the same of an AlertDialog that back doesn't dismiss.
    let backs = Rc::new(Cell::new(0));
    let dismissed = Rc::new(Cell::new(0));
    let alert = mutable_state_of(false);
    let (b, d, a) = (backs.clone(), dismissed.clone(), alert.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b, d) = (b.clone(), d.clone());
        BackHandler(true, move || b.set(b.get() + 1));
        if a.get() {
            AlertDialog(
                move || d.set(d.get() + 1),
                || TextButton(|| {}, content = || Text("Tamam")),
                text = slot(|| Text("Kalıcı uyarı")),
                properties = DialogProperties { dismiss_on_back_press: false, ..DialogProperties::default() },
            );
        } else {
            Dialog(
                on_dismiss_request = || {},
                properties = DialogProperties { dismiss_on_back_press: false, ..DialogProperties::default() },
                content = || Text("Kalıcı"),
            );
        }
    }));
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert_eq!(backs.get(), 0);
    rule.on_node_with_text("Kalıcı").assert_exists();
    alert.set(true);
    rule.wait_for_idle();
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert!(backs.get() == 0 && dismissed.get() == 0);
    rule.on_node_with_text("Kalıcı uyarı").assert_exists();
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

fn alert_dialog_parts_take_their_paddings_and_styles() {
    // 24 dp in; the icon centered, 16 dp over the title (centered under an icon, at the start without), 16 dp over
    // the text (at the start), 24 dp over the buttons (at the end); the title headlineSmall (24 sp), the text
    // bodyMedium (14 sp), what the buttons hold labelLarge (14 sp) in the action color (primary).
    let icon = mutable_state_of(true);
    let i = icon.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            AlertDialog(
                || {},
                || Text("Onayla", modifier = Modifier.test_tag("onay")),
                icon = if i.get() { slot(|| Box(Modifier.size(24.dp()).test_tag("simge"))) } else { None },
                title = slot(|| Text("Başlık", modifier = Modifier.test_tag("başlık"))),
                text = slot(|| Text("Metin", modifier = Modifier.test_tag("metin"))),
                modifier = Modifier.test_tag("diyalog"),
            );
        }),
        IntSize::new(400, 800),
    );
    let cs = light_color_scheme();
    let (dialog, simge, title, text, ok) = (
        bounds(&rule, "diyalog"),
        bounds(&rule, "simge"),
        bounds(&rule, "başlık"),
        bounds(&rule, "metin"),
        bounds(&rule, "onay"),
    );
    assert!(near(dialog.width(), 280.0, 0.01));
    assert!(near(simge.top, dialog.top + 24.0, 0.5) && near(simge.center().x, dialog.center().x, 0.5), "{simge:?}");
    assert!(near(title.top, simge.bottom + 16.0, 0.5) && near(title.center().x, dialog.center().x, 0.5), "{title:?}");
    assert!(near(text.top, title.bottom + 16.0, 0.5) && near(text.left, dialog.left + 24.0, 0.5), "{text:?}");
    assert!(near(ok.top, text.bottom + 24.0, 0.5) && near(ok.right, dialog.right - 24.0, 0.5), "{ok:?}");
    assert!(near(ok.bottom, dialog.bottom - 24.0, 0.5));
    let canvas = rule.draw();
    assert_eq!(font_size_of(&canvas, "Başlık"), Some(24.0));
    assert_eq!(font_size_of(&canvas, "Metin"), Some(14.0));
    assert_eq!(font_size_of(&canvas, "Onayla"), Some(14.0));
    assert_eq!(color_of(&canvas, "Onayla"), Some(cs.primary));
    assert_eq!(color_of(&canvas, "Başlık"), Some(cs.on_surface));
    assert_eq!(color_of(&canvas, "Metin"), Some(cs.on_surface_variant));
    // Without an icon the title starts at the start.
    icon.set(false);
    rule.wait_for_idle();
    let (dialog, title) = (bounds(&rule, "diyalog"), bounds(&rule, "başlık"));
    assert!(near(title.left, dialog.left + 24.0, 0.5) && near(title.top, dialog.top + 24.0, 0.5), "{title:?}");
}

fn alert_dialog_defaults_follow_the_tokens() {
    let seen = Rc::new(RefCell::new(Vec::<Color>::new()));
    let sn = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sn.borrow_mut().extend([
                AlertDialogDefaults.container_color(),
                AlertDialogDefaults.icon_content_color(),
                AlertDialogDefaults.title_content_color(),
                AlertDialogDefaults.text_content_color(),
            ]);
            AlertDialog(
                || {},
                || TextButton(|| {}, content = || Text("Tamam")),
                icon = slot(|| {
                    VectorIcon(
                        Icons::Filled::Favorite(),
                        None,
                        modifier = Modifier.size(AlertDialogDefaults::IconSize()),
                    )
                }),
                modifier = Modifier.test_tag("diyalog"),
            );
        }),
        IntSize::new(400, 800),
    );
    let cs = light_color_scheme();
    assert_eq!(*seen.borrow(), [cs.surface_container_high, cs.secondary, cs.on_surface, cs.on_surface_variant]);
    assert_eq!(AlertDialogDefaults::TonalElevation, Dp(0.0));
    assert_eq!(AlertDialogDefaults::IconSize(), Dp(24.0));
    // The container: the high surface container, 28 dp corners (extra large); the icon in the secondary color.
    let dialog = bounds(&rule, "diyalog");
    let canvas = rule.draw();
    let container =
        canvas.rects.iter().position(|r| same_rect(*r, dialog)).expect("the dialog's container drawn over its bounds");
    assert!(
        canvas.rect_colors[container] == cs.surface_container_high && near(canvas.rect_radii[container], 28.0, 0.01)
    );
    assert_eq!(canvas.vector_colors, [cs.secondary]);
}

fn alert_dialogs_are_panes_titled_dialog() {
    // BasicAlertDialog's pane title: "Dialog" (Strings.Dialog, in the platform's language), the dialog's content in
    // a dialog window (IsDialog).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        AlertDialog(|| {}, || TextButton(|| {}, content = || Text("Tamam")), title = slot(|| Text("Uyarı")));
    }));
    rule.on_node(SemanticsMatcher::expect_value(SemanticsProperties::PaneTitle, "Dialog".to_string())).assert_exists();
    let tree = rule.host().accessibility_tree();
    assert!(tree.iter().any(|n| n.is_dialog));
}

fn alert_dialog_buttons_follow_the_layout_direction() {
    // Right to left: the confirming button at the left end, the dismissing one after it (to its right).
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                AlertDialog(
                    || {},
                    || TextButton(|| {}, modifier = Modifier.test_tag("onay"), content = || Text("Tamam")),
                    dismiss_button =
                        slot(|| TextButton(|| {}, modifier = Modifier.test_tag("ret"), content = || Text("Vazgeç"))),
                    title = slot(|| Text("Başlık", modifier = Modifier.test_tag("başlık"))),
                    modifier = Modifier.test_tag("diyalog"),
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let (dialog, ok, cancel, title) =
        (bounds(&rule, "diyalog"), bounds(&rule, "onay"), bounds(&rule, "ret"), bounds(&rule, "başlık"));
    assert!(near(ok.left, dialog.left + 24.0, 0.5) && near(cancel.left - ok.right, 8.0, 0.5), "{ok:?} {cancel:?}");
    assert!(near(title.right, dialog.right - 24.0, 0.5)); // the title at the start: the right
}

/// An override drawing every basic alert dialog in a frame of its own (and telling it was used).
struct FramedOverride(Rc<Cell<u32>>);

impl BasicAlertDialogOverride for FramedOverride {
    #[track_caller]
    fn BasicAlertDialog(&self, scope: &BasicAlertDialogOverrideScope) {
        self.0.set(self.0.get() + 1);
        let (on_dismiss_request, content, properties, modifier) =
            (scope.on_dismiss_request(), scope.content(), scope.properties(), scope.modifier());
        kompozable_block!({
            Dialog(
                on_dismiss_request = move || on_dismiss_request(),
                properties = properties,
                content = move || {
                    let content = content.clone();
                    let (content, modifier) = (content.clone(), modifier.clone());
                    Box(
                        Modifier.test_tag("çerçeve").padding(10.dp()),
                        content = move || {
                            let content = content.clone();
                            Box(modifier.clone(), content = move || content());
                        },
                    );
                },
            );
        })
    }
}

fn basic_alert_dialogs_can_be_made_by_an_override() {
    // LocalBasicAlertDialogOverride: the dialogs below it (an AlertDialog's too) made by the override provided.
    let used = Rc::new(Cell::new(0));
    let u = used.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let over: Rc<dyn BasicAlertDialogOverride> = Rc::new(FramedOverride(u.clone()));
            KompozitionLocalProvider(&[LocalBasicAlertDialogOverride.provides(over)], || {
                AlertDialog(
                    || {},
                    || TextButton(|| {}, content = || Text("Tamam")),
                    modifier = Modifier.test_tag("diyalog"),
                );
            });
        }),
        IntSize::new(400, 800),
    );
    assert!(used.get() >= 1);
    let (frame, dialog) = (bounds(&rule, "çerçeve"), bounds(&rule, "diyalog"));
    assert!(
        near(dialog.left, frame.left + 10.0, 0.5) && near(dialog.top, frame.top + 10.0, 0.5),
        "{frame:?} {dialog:?}"
    );
    // (no least width: the override's own way)
    assert!(dialog.width() < 280.0, "{dialog:?}");
    rule.on_node(SemanticsMatcher::expect_value(SemanticsProperties::PaneTitle, "Dialog".to_string()))
        .assert_does_not_exist();
}

fn deprecated_alert_dialog_of_content_is_a_basic_one() {
    let dismissed = Rc::new(Cell::new(0));
    let d = dismissed.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let d = d.clone();
            AlertDialogWithContent(
                move || d.set(d.get() + 1),
                modifier = Modifier.test_tag("diyalog"),
                content = || {
                    Text("İçerik");
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(near(bounds(&rule, "diyalog").width(), 280.0, 0.01));
    rule.on_node_with_text("İçerik").assert_exists();
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert_eq!(dismissed.get(), 1);
}

// ---- The M15 samples (AlertDialogSamples.kt) -------------------------------------------------------------------

#[kompozable]
fn AlertDialogSample() {
    let open_dialog = remember(|| mutable_state_of(true));
    let o = open_dialog.clone();
    Button(move || o.set(true), content = || Text("Open dialog"));
    if open_dialog.get() {
        let (o1, o2, o3) = (open_dialog.clone(), open_dialog.clone(), open_dialog.clone());
        AlertDialog(
            move || {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                o1.set(false);
            },
            title = slot(|| Text("Title")),
            text = slot(|| Text("Turned on by default")),
            confirm_button = move || {
                let o2 = o2.clone();
                TextButton(move || o2.set(false), content = || Text("Confirm"));
            },
            dismiss_button = slot(move || {
                let o3 = o3.clone();
                TextButton(move || o3.set(false), content = || Text("Dismiss"));
            }),
        );
    }
}

#[kompozable]
fn AlertDialogWithIconSample() {
    let open_dialog = remember(|| mutable_state_of(true));
    let o = open_dialog.clone();
    Button(move || o.set(true), content = || Text("Open dialog"));
    if open_dialog.get() {
        let (o1, o2, o3) = (open_dialog.clone(), open_dialog.clone(), open_dialog.clone());
        AlertDialog(
            move || {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                o1.set(false);
            },
            icon = slot(|| {
                VectorIcon(Icons::Filled::Favorite(), None, modifier = Modifier.size(AlertDialogDefaults::IconSize()));
            }),
            title = slot(|| Text("Title")),
            text = slot(|| {
                Text(
                    "This area typically contains the supportive text which presents the details regarding the \
                     Dialog's purpose.",
                );
            }),
            confirm_button = move || {
                let o2 = o2.clone();
                TextButton(move || o2.set(false), content = || Text("Confirm"));
            },
            dismiss_button = slot(move || {
                let o3 = o3.clone();
                TextButton(move || o3.set(false), content = || Text("Dismiss"));
            }),
        );
    }
}

#[kompozable]
fn BasicAlertDialogSample() {
    let open_dialog = remember(|| mutable_state_of(true));
    let o = open_dialog.clone();
    Button(move || o.set(true), content = || Text("Open dialog"));
    if open_dialog.get() {
        let (o1, o2) = (open_dialog.clone(), open_dialog.clone());
        BasicAlertDialog(
            move || {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                o1.set(false);
            },
            content = move || {
                let o2 = o2.clone();
                Surface(
                    modifier = Modifier.wrap_content_width().wrap_content_height(),
                    shape = std::sync::Arc::new(MaterialTheme::shapes().large),
                    tonal_elevation = AlertDialogDefaults::TonalElevation,
                    content = move || {
                        let o2 = o2.clone();
                        Column(
                            modifier = Modifier.padding(16.dp()),
                            content = move || {
                                Text(
                                    "This area typically contains the supportive text which presents the details \
                                 regarding the Dialog's purpose.",
                                );
                                Spacer(Modifier.height(24.dp()));
                                let o2 = o2.clone();
                                TextButton(
                                    move || o2.set(false),
                                    modifier = Modifier.align(Alignment::End),
                                    content = || Text("Confirm"),
                                );
                            },
                        );
                    },
                );
            },
        );
    }
}

fn alert_dialog_samples() {
    let size = IntSize::new(400, 800);
    // Each opens at once; Confirm (or Dismiss) closes it; the button opens it again.
    let rules: [(&str, fn() -> KompozTestRule, &str); 4] = [
        ("AlertDialogSample", || KompozTestRule::with_viewport(AlertDialogSample, IntSize::new(400, 800)), "Confirm"),
        ("AlertDialogSample", || KompozTestRule::with_viewport(AlertDialogSample, IntSize::new(400, 800)), "Dismiss"),
        (
            "AlertDialogWithIconSample",
            || KompozTestRule::with_viewport(AlertDialogWithIconSample, IntSize::new(400, 800)),
            "Dismiss",
        ),
        (
            "BasicAlertDialogSample",
            || KompozTestRule::with_viewport(BasicAlertDialogSample, IntSize::new(400, 800)),
            "Confirm",
        ),
    ];
    for (name, make, closer) in rules {
        let rule = make();
        assert_eq!(rule.host().overlay_count(), 1, "{name}");
        rule.on_node_with_text(closer).perform_click();
        rule.wait_for_idle();
        assert_eq!(rule.host().overlay_count(), 0, "{name}");
        rule.on_node_with_text("Open dialog").perform_click();
        rule.wait_for_idle();
        assert_eq!(rule.host().overlay_count(), 1, "{name}");
        assert!(escape(&rule), "{name}");
        rule.wait_for_idle();
        assert_eq!(rule.host().overlay_count(), 0, "{name}");
    }
    // The icon sample's icon: AlertDialogDefaults.IconSize, centered over the title.
    let rule = KompozTestRule::with_viewport(AlertDialogWithIconSample, size);
    let title = text_bounds(&rule, "Title");
    let canvas = rule.draw();
    assert_eq!(canvas.vector_colors, [light_color_scheme().secondary]);
    assert!(title.top > 0.0);
    // The basic sample: its own large-cornered surface, at least 280 wide (its text is wider).
    let rule = KompozTestRule::with_viewport(BasicAlertDialogSample, size);
    let confirm = text_bounds(&rule, "Confirm");
    assert!(confirm.right <= 400.0 - 48.0 - 16.0 + 0.5, "{confirm:?}");
}
