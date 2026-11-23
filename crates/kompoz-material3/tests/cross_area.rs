//! The C++ Kompoz's cross-area tests (kompoz_tests.cpp, survey §3) whose components span several Material areas, each
//! ported whole here by its name (the parts the work packages had ported apart were brought together): whole pixels,
//! composing only as components turn (selection controls; overlays, fields and indicators), and Material's states
//! saved with the process. (The cross-area tests ported whole where their package was: buttons_animate_composing_
//! around_them_only_as_they_turn in buttons_and_containers.rs, layouts_that_measure_twice_probe_first_so_sizes_settle
//! in navigation_bar.rs, navigation_items_animate_composing_nothing_around_them in navigation_rail.rs,
//! what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing and scaffold_bodies_and_carousel_items_leave_
//! content_its_own_size_as_compose in scaffold.rs.) M15's values where the C++ had its own, said where they differ.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{MutatePriority, animate_float_as_state};
use kompoz_animation_graphics::{AnimatedImageVector, remember_animated_vector_painter};
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::interaction::{DragInteraction, MutableInteractionSource};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive::prelude::*;
use kompoz_runtime::kt::java_time::{LocalDate, YearMonth};
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_text::TextStyle;
use kompoz_ui::input::PointerType;
use kompoz_ui::layout::FirstBaseline;
use kompoz_ui::text::remember_text_measurer;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    layout_is_in_whole_pixels_as_composes,
    selection_controls_animate_composing_around_them_only_as_they_turn,
    overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn,
    material_states_come_back_after_the_process_ended,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn center(rule: &KompozTestRule, tag: &str) -> Offset {
    bounds(rule, tag).center()
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set while composing")
}

fn some_slot(content: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(content))
}

fn scope_of(rule: &KompozTestRule) -> CoroutineScope {
    CoroutineScope::new(rule.host().dispatcher())
}

// ---- C++ layout_is_in_whole_pixels_as_composes ---------------------------------------------------------------------

fn layout_is_in_whole_pixels_as_composes() {
    // At 2.625 px a dp every layout lands on whole px, as Compose's: sizes rounded, centered offsets rounded,
    // padding rounded before it adds up, spacing rounded each time, weights sharing whole px, offsets rounded, text
    // sizes and baselines rounded. (Bounds here are px.)
    let d = 2.625f32;
    let measured: Slot<IntSize> = slot();
    let m = measured.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let m = m.clone();
            Column(move || {
                Box(
                    modifier = Modifier.size_wh(31.dp(), 20.dp()).test_tag("dış"),
                    content_alignment = Alignment::Center,
                    content = || Box(modifier = Modifier.size_wh(10.dp(), 9.dp()).test_tag("iç")),
                );
                Box(
                    modifier =
                        Modifier.width(40.dp()).test_tag("dolgulu").padding_sides(start = 1.3.dp(), end = 1.3.dp()),
                    content = || Box(modifier = Modifier.fill_max_width().height(4.dp()).test_tag("dolgu içi")),
                );
                Row(
                    modifier = Modifier.test_tag("aralıklı"),
                    horizontal_arrangement = Arrangement::spaced_by(1.3.dp()),
                    content = || {
                        for i in 0..3 {
                            Box(modifier = Modifier.size(10.dp()).test_tag(format!("a{i}")));
                        }
                    },
                );
                Row(
                    modifier = Modifier.width(100.dp()),
                    content = || {
                        for i in 0..3 {
                            Box(modifier = Modifier.weight(1.0).height(4.dp()).test_tag(format!("w{i}")));
                        }
                    },
                );
                Box(modifier = Modifier.offset(x = 0.7.dp(), y = 0.2.dp()).size(5.dp()).test_tag("kaymış"));
                Text("abc", modifier = Modifier.test_tag("yazı"), font_size = 15.sp());
                // (C++'s TextLayoutMeasurer: Compose's TextMeasurer, measuring at the density it was remembered at)
                let measurer = remember_text_measurer();
                let m = m.clone();
                Canvas(
                    Modifier.size(1.dp()),
                    on_draw = move |_| {
                        let style = TextStyle { font_size: 15.sp(), ..TextStyle::Default };
                        *m.borrow_mut() = Some(measurer.measure("abc", style = style).size);
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.host().set_density(Density::new(d, 1.0));
    rule.wait_for_idle();
    let px = |tag: &str| bounds(&rule, tag);
    let whole = |r: Rect| [r.left, r.top, r.width(), r.height()].iter().all(|v| (v - v.round()).abs() < 1e-3);
    for tag in ["dış", "iç", "dolgulu", "dolgu içi", "aralıklı", "a0", "a1", "a2", "w0", "w1", "w2", "kaymış", "yazı"]
    {
        assert!(whole(px(tag)), "{tag} not on whole px: {:?}", px(tag));
    }
    // 31 x 20 dp: 81 x 53 px (81.375, 52.5 a half up); 10 x 9: 26 x 24; centered at 27.5 and 14.5 px: 28, 15.
    assert!(near(px("dış").width(), 81.0, 1e-3) && near(px("dış").height(), 53.0, 1e-3), "{:?}", px("dış"));
    assert!(near(px("iç").width(), 26.0, 1e-3) && near(px("iç").height(), 24.0, 1e-3), "{:?}", px("iç"));
    assert!(near(px("iç").left - px("dış").left, 28.0, 1e-3) && near(px("iç").top - px("dış").top, 15.0, 1e-3));
    // 40 dp less 1.3 dp a side: 105 px less 3 a side (3.41 each, rounded before they add up).
    assert!(near(px("dolgulu").width(), 105.0, 1e-3) && near(px("dolgu içi").width(), 99.0, 1e-3));
    assert!(near(px("dolgu içi").left - px("dolgulu").left, 3.0, 1e-3));
    // 1.3 dp apart: 3 px, each time.
    assert!(near(px("a1").left - px("a0").left, 29.0, 1e-3) && near(px("a2").left - px("a0").left, 58.0, 1e-3));
    assert!(near(px("aralıklı").width(), 84.0, 1e-3), "{:?}", px("aralıklı"));
    // 263 px (262.5) in three: 87.67 rounded to 88 each, one over: the first gives it up.
    assert!(
        near(px("w0").width(), 87.0, 1e-3) && near(px("w1").width(), 88.0, 1e-3) && near(px("w2").width(), 88.0, 1e-3),
        "{:?} {:?} {:?}",
        px("w0"),
        px("w1"),
        px("w2")
    );
    assert!(near(px("w2").right - px("w0").left, 263.0, 1e-3));
    // Moved by 1.84 and 0.53 px: 2 and 1 (its content moved, not its place).
    let node = rule.on_node_with_tag("kaymış").layout_node();
    let (outer, inner) = (node.coordinates(), node.inner_coordinates());
    let place = outer.find_root_coordinates().local_bounding_box_of(&outer, false);
    let moved = inner.find_root_coordinates().local_bounding_box_of(&inner, false);
    assert!(near(moved.left - place.left, 2.0, 1e-3) && near(moved.top - place.top, 1.0, 1e-3), "{place:?} {moved:?}");
    // "abc" at 15 sp: 22.5 x 18.75 dp (the test's measurer), 59.06 x 49.22 px: 60 x 50.
    assert!(near(px("yazı").width(), 60.0, 1e-3) && near(px("yazı").height(), 50.0, 1e-3), "{:?}", px("yazı"));
    // 12 dp: 31.5 px, 32
    let baseline = rule.on_node_with_tag("yazı").fetch_semantics_node().get_alignment_line_position(FirstBaseline);
    assert_eq!(baseline, 32);
    let _ = rule.draw();
    assert_eq!(get(&measured), IntSize::new(60, 50));

    // A tab row's tabs share its whole px out as Compose's (each the row's divided, the rest left over); a held
    // button in a group grows by whole px, its neighbours give up whole px.
    let held = MutableInteractionSource::new();
    let h = held.clone();
    let rows = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let h = h.clone();
            Column(move || {
                PrimaryTabRow(
                    0,
                    modifier = Modifier.width(100.dp()),
                    tabs = || {
                        for i in 0..3 {
                            TextIconTab(
                                i == 0,
                                || {},
                                modifier = Modifier.test_tag(format!("s{i}")),
                                text = some_slot(move || Text(format!("S{i}"))),
                            );
                        }
                    },
                );
                let h = h.clone();
                // (M15's deprecated content overload, as the C++ composed it)
                #[allow(deprecated)]
                ButtonGroupWithoutOverflowIndicator(
                    content = move |group: &ButtonGroupScope| {
                        let g0 = remember(MutableInteractionSource::new);
                        let g2 = remember(MutableInteractionSource::new);
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &g0).test_tag("g0"),
                            content = || Text("Önceki"),
                        );
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &h).test_tag("g1"),
                            interaction_source = Some(h.clone()),
                            content = || Text("Oynat"),
                        );
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &g2).test_tag("g2"),
                            content = || Text("Sonraki"),
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rows.host().set_density(Density::new(d, 1.0));
    rows.wait_for_idle();
    let px = |tag: &str| bounds(&rows, tag);
    // 263 / 3
    assert!(
        near(px("s0").width(), 87.0, 1e-3) && near(px("s1").left, 87.0, 1e-3) && near(px("s2").left, 174.0, 1e-3),
        "{:?}",
        px("s1")
    );
    let (g0, g1, g2) = (px("g0").width(), px("g1").width(), px("g2").width());
    rows.host().pointer_down(px("g1").center(), -1, 0, PointerType::Touch);
    rows.advance_time_by(500);
    // (its natural width a whole px: what it grows by rounded, half of it from each neighbour)
    let growth = g1 * 0.15;
    assert!(near(px("g1").width(), g1 + (growth + 0.5).floor(), 1e-3), "{:?} {g1}", px("g1"));
    assert!(near(px("g0").width(), g0 - (growth / 2.0 + 0.5).floor(), 1e-3));
    assert!(near(px("g2").width(), g2 - (growth / 2.0 + 0.5).floor(), 1e-3));
    for tag in ["g0", "g1", "g2", "s0", "s1", "s2"] {
        assert!(whole(px(tag)), "{tag} not on whole px: {:?}", px(tag));
    }
    rows.host().pointer_up(px("g1").center(), -1, 0);
}

// ---- C++ selection_controls_animate_composing_around_them_only_as_they_turn ----------------------------------------

fn selection_controls_animate_composing_around_them_only_as_they_turn() {
    // Checkbox, Switch, RadioButton, a selectable list item, segmented buttons and a tab row compose what is around
    // them once as they turn, not as they animate (their animations read in drawing and measuring); a drag handle
    // dragged takes its dragged size as it is laid out; a slider's thumb (M15's, of a 20 dp size: the C++'s
    // ShapeThumb) is thinner while held — composed as it is pressed, nothing around it.
    let on = mutable_state_of(false);
    let held = MutableInteractionSource::new();
    let composed = Rc::new(Cell::new(0));
    let (o, h, c) = (on.clone(), held.clone(), composed.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            c.set(c.get() + 1);
            let value = o.get();
            let (o, h) = (o.clone(), h.clone());
            Column(move || {
                let (o1, o2, o3, o4) = (o.clone(), o.clone(), o.clone(), o.clone());
                Checkbox(value, Some(Rc::new(move |x| o1.set(x)) as Rc<dyn Fn(bool)>));
                Switch(value, Some(Rc::new(move |x| o2.set(x)) as Rc<dyn Fn(bool)>));
                RadioButton(value, Some(Rc::new(move || o3.set(true)) as Rc<dyn Fn()>));
                // (the C++'s ListItem(selected, onClick): M15's selectable list item)
                SelectableListItem(value, move || o4.set(!o4.peek()), content = || Text("Seç"));
                SingleChoiceSegmentedButtonRow(
                    modifier = Modifier.width(300.dp()),
                    content = move |row: &SingleChoiceSegmentedButtonRowScope| {
                        for i in 0..2 {
                            row.SegmentedButton(
                                value == (i == 1),
                                || {},
                                SegmentedButtonDefaults.item_shape(i, 2),
                                label = move || Text(format!("S{i}")),
                            );
                        }
                    },
                );
                PrimaryTabRow(
                    if value { 1 } else { 0 },
                    tabs = move || {
                        // (colors of their own: by default the unselected ones are the selected one's)
                        for i in 0..2 {
                            TextIconTab(
                                value == (i == 1),
                                || {},
                                text = some_slot(move || Text(format!("T{i}"))),
                                selected_content_color = Color::Red,
                                unselected_content_color = Some(Color::Blue),
                            );
                        }
                    },
                );
                VerticalDragHandle(modifier = Modifier.test_tag("tutamak"), interaction_source = Some(h.clone()));
                let source = remember(MutableInteractionSource::new);
                let thumb_source = source.clone();
                SlotSlider(
                    0.5,
                    |_| {},
                    modifier = Modifier.width(200.dp()).test_tag("kaydırıcı"),
                    interaction_source = source,
                    thumb = Some(Rc::new(move |_: SliderState| {
                        SliderDefaults.Thumb(thumb_source.clone(), thumb_size = DpSize::new(20.dp(), 20.dp()))
                    }) as Rc<dyn Fn(SliderState)>),
                );
            });
        }),
        IntSize::new(400, 800),
    );
    on.set(true);
    rule.advance_frame(); // composed as they turn...
    let before = composed.get();
    rule.wait_for_idle();
    assert_eq!(composed.get(), before); // ...not as they animate
    // The drag handle (its own layout inside its 48 dp room) dragged, by whatever drags it.
    let handle = || {
        // (its own layout's size, inside the room)
        rule.on_node_with_tag("tutamak").layout_node().inner_coordinates().size().width as f32
    };
    let narrow = handle();
    let start = DragInteraction::Start::new();
    held.try_emit(start);
    rule.advance_frame();
    let before = composed.get();
    rule.advance_frame();
    // (M15's handle switches to its dragged size, 12 dp, as it is next laid out — its size isn't animated, as the
    // C++'s was —: laid out, not composed, around it)
    let dragged = handle();
    assert!(near(narrow, 4.0, 0.01) && near(dragged, 12.0, 0.01), "{narrow} {dragged}");
    rule.wait_for_idle();
    assert_eq!(composed.get(), before);
    held.try_emit(DragInteraction::Stop::new(start));
    rule.wait_for_idle();
    assert!(near(handle(), narrow, 0.01));
    // A slider's thumb grows as it is held: composed as it is pressed, not as it grows.
    let center = center(&rule, "kaydırıcı");
    rule.host().pointer_down(center, -1, 0, PointerType::Touch);
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(300);
    assert_eq!(composed.get(), before);
    rule.host().pointer_up(center, -1, 0);
    rule.wait_for_idle();
}

// ---- C++ overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn ----------------------------

/// The C++'s blinking dot: an 8 dp square whose fill goes from red to blue in 300 ms.
const Blink: &str = r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
        <vector android:width="8dp" android:height="8dp" android:viewportWidth="8" android:viewportHeight="8">
            <path android:name="dot" android:pathData="M0,0h8v8h-8z" android:fillColor="#FF0000"/>
        </vector>
    </aapt:attr>
    <target android:name="dot">
        <aapt:attr name="android:animation">
            <objectAnimator android:propertyName="fillColor" android:valueFrom="#FF0000" android:valueTo="#0000FF" android:duration="300"/>
        </aapt:attr>
    </target>
</animated-vector>"##;

/// The samples' split button trailing icon (M15 has no `TrailingButtonIcon`, the C++'s): `KeyboardArrowDown`
/// turning over as `checked` animates `rotationZ`.
#[kompozable]
fn TrailingArrow(checked: bool) {
    let rotation =
        animate_float_as_state(if checked { 180.0 } else { 0.0 }, label = "Trailing Icon Rotation".to_string());
    VectorIcon(
        Icons::Filled::KeyboardArrowDown(),
        Some("Localized description".into()),
        modifier = Modifier.size(SplitButtonDefaults::TrailingIconSize).graphics_layer_block(move |layer| {
            layer.rotation_z = rotation.value();
        }),
    );
}

fn overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn() {
    // A menu with a checkable item, the exposed menu's trailing icon, a split button's arrow, a filter chip's leading
    // icon, a scrollbar, a modal bottom sheet, a tooltip, a text field's label, a toggle FAB, a wavy indicator, an
    // animated vector, a modal wide rail and a wide rail: opened, they compose what is around them as they turn, not
    // on the way (a rail widening is laid out as it widens); an item checked the same; closed, gone once they are out;
    // a scrollbar hovered the same.
    let open = mutable_state_of(false);
    let text = mutable_state_of(String::new());
    let mark = mutable_state_of(false);
    let rail: Slot<Rc<dyn WideNavigationRailState>> = slot();
    let wide: Slot<Rc<dyn WideNavigationRailState>> = slot();
    let tip: Slot<Rc<dyn TooltipState>> = slot();
    let blink = AnimatedImageVector::from_animated_vector_xml(Blink).expect("the blinking dot");
    let composed = Rc::new(Cell::new(0));
    let (o, t, mk, r, w, tp, c) =
        (open.clone(), text.clone(), mark.clone(), rail.clone(), wide.clone(), tip.clone(), composed.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            c.set(c.get() + 1);
            let shown = o.get();
            let (o, t, mk, r, w, tp, blink) =
                (o.clone(), t.clone(), mk.clone(), r.clone(), w.clone(), tp.clone(), blink.clone());
            Column(move || {
                let (o1, mk) = (o.clone(), mk.clone());
                Box(content = move || {
                    let (o1, mk) = (o1.clone(), mk.clone());
                    DropdownMenu(
                        shown,
                        move || o1.set(false),
                        content = move || {
                            Text("Seçenek");
                            let mk2 = mk.clone();
                            CheckableDropdownMenuItem(
                                mk.get(),
                                move |v| mk2.set(v),
                                || Text("İşaretli"),
                                MenuDefaults.item_shapes(),
                            );
                        },
                    );
                });
                Row(content = move || {
                    ExposedDropdownMenuDefaults.TrailingIcon(shown);
                    TrailingArrow(shown);
                    FilterChip(
                        shown,
                        || {},
                        || Text("Süz"),
                        leading_icon = if shown {
                            some_slot(|| VectorIcon(Icons::Filled::Check(), Some(String::new())))
                        } else {
                            None
                        },
                    );
                    let scrolled = remember_scroll_state(0);
                    Box(
                        modifier = Modifier.size_wh(60.dp(), 100.dp()),
                        content = move || {
                            Column(
                                modifier = Modifier.fill_max_size().vertical_scroll(&scrolled),
                                content = || Box(modifier = Modifier.height(400.dp())),
                            );
                            VerticalScrollbar(
                                remember_scrollbar_adapter(scrolled.clone()),
                                modifier = Modifier.align(Alignment::CenterEnd).fill_max_height().test_tag("çubuk"),
                            );
                        },
                    );
                });
                if shown {
                    let o2 = o.clone();
                    ModalBottomSheet(move || o2.set(false), content = || Text("Sayfa"));
                }
                let state = remember_tooltip_state();
                *tp.borrow_mut() = Some(state.clone());
                TooltipBox(
                    position_provider =
                        TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                    tooltip = |s: &dyn TooltipScope| s.PlainTooltip(content = || Text("İpucu")),
                    state = state,
                    content = || Box(modifier = Modifier.size(40.dp())),
                );
                let t2 = t.clone();
                StringTextField(
                    t.get(),
                    move |s| t2.set(s),
                    modifier = Modifier.test_tag("alan"),
                    label = Some(Rc::new(kompozable_lambda!(|| {
                        Text("Ad");
                    })) as TextFieldSlot),
                );
                let o3 = o.clone();
                ToggleFloatingActionButton(
                    shown,
                    move |v| o3.set(v),
                    content = |_: &ToggleFloatingActionButtonScope| {
                        VectorIcon(Icons::Filled::Add(), Some(String::new()));
                    },
                );
                // (the wave rises from none; M15 reads the progress as it draws)
                let o4 = o.clone();
                LinearWavyProgressIndicator(progress = move || if o4.get() { 0.5 } else { 0.0 });
                Image(Rc::new(remember_animated_vector_painter(blink.clone(), shown)), Some("Göz".into()));
                let rail_state = remember_wide_navigation_rail_state();
                *r.borrow_mut() = Some(rail_state.clone());
                ModalWideNavigationRail(state = Some(rail_state), hide_on_collapse = true, content = || Text("Ray"));
                let wide_state = remember_wide_navigation_rail_state();
                *w.borrow_mut() = Some(wide_state.clone());
                WideNavigationRail(modifier = Modifier.test_tag("geniş"), state = Some(wide_state), content = || {});
            });
        }),
        IntSize::new(400, 800),
    );
    let gone = |rule: &KompozTestRule| {
        !rule.on_node_with_text("Seçenek").exists()
            && !rule.on_node_with_text("İpucu").exists()
            && !rule.on_node_with_text("Ray").exists()
    };
    assert!(gone(&rule));
    let field = center(&rule, "alan"); // tapped, focused: its label floats
    rule.host().pointer_down(field, -1, 0, PointerType::Touch);
    rule.host().pointer_up(field, -1, 0);
    let collapsed = bounds(&rule, "geniş").width();
    let scope = scope_of(&rule);
    open.set(true);
    let (rs, ws, ts) = (get(&rail), get(&wide), get(&tip));
    scope.launch(async move { rs.expand().await });
    scope.launch(async move { ws.expand().await });
    scope.launch(async move { ts.show(MutatePriority::Default).await });
    rule.advance_frame();
    rule.advance_frame(); // composed as they turn...
    let before = composed.get();
    rule.advance_time_by(48);
    assert_eq!(composed.get(), before); // ...and not on the way
    let widening = bounds(&rule, "geniş").width();
    assert!(widening > collapsed + 5.0 && widening < 215.0, "{collapsed} {widening}"); // laid out as it widens
    rule.advance_time_by(700); // there (before the tooltip goes by itself, at 1.5 s)
    assert!(
        rule.on_node_with_text("Seçenek").exists()
            && rule.on_node_with_text("İpucu").exists()
            && rule.on_node_with_text("Ray").exists()
    );
    // An item checked: composed as it turns, not as its color goes over.
    mark.set(true);
    rule.advance_frame();
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(48);
    assert_eq!(composed.get(), before);
    // Closed: gone once they are out (composed as they get there).
    open.set(false);
    let rs = get(&rail);
    scope.launch(async move { rs.collapse().await });
    get(&tip).dismiss();
    rule.wait_for_idle();
    assert!(gone(&rule));
    // A scrollbar hovered (nothing over it now): the same.
    rule.host().pointer_move(center(&rule, "çubuk"), -1, 0);
    rule.advance_frame();
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(48);
    assert_eq!(composed.get(), before);
}

// ---- C++ material_states_come_back_after_the_process_ended ---------------------------------------------------------

/// The states one run of the app made (the C++'s `Run`).
#[derive(Default)]
struct Run {
    bar: Slot<TopAppBarState>,
    drawer: Slot<DrawerState>,
    toolbar: Slot<Rc<dyn FloatingToolbarState>>,
    sheet: Slot<SheetState>,
    half: Slot<SheetState>,
    search: Slot<SearchBarState>,
    swipe: Slot<SwipeToDismissBoxState>,
    time: Slot<Rc<dyn TimePickerState>>,
    rail: Slot<Rc<dyn WideNavigationRailState>>,
    date: Slot<Rc<dyn DatePickerState>>,
    range: Slot<Rc<dyn DateRangePickerState>>,
    pane: Slot<PaneExpansionState>,
}

fn states_app(
    r: &Rc<Run>,
    shown: &MutableState<bool>,
    sheet_shown: &MutableState<bool>,
    height: i32,
) -> KompozTestRule {
    let (r, shown, sheet_shown) = (r.clone(), shown.clone(), sheet_shown.clone());
    let rule = KompozTestRule::empty_with_viewport(IntSize::new(400, height));
    rule.set_content(kompozable_lambda!(move || {
        if !shown.get() {
            return; // made after the state came back, as on Android
        }
        *r.bar.borrow_mut() = Some(remember_top_app_bar_state());
        *r.drawer.borrow_mut() = Some(remember_drawer_state(DrawerValue::Closed));
        *r.toolbar.borrow_mut() = Some(remember_floating_toolbar_state());
        let sheet = remember_modal_bottom_sheet_state(skip_partially_expanded = true);
        *r.sheet.borrow_mut() = Some(sheet.clone());
        let half = remember_modal_bottom_sheet_state();
        *r.half.borrow_mut() = Some(half.clone());
        *r.search.borrow_mut() = Some(remember_search_bar_state());
        let swipe = remember_swipe_to_dismiss_box_state();
        *r.swipe.borrow_mut() = Some(swipe.clone());
        *r.time.borrow_mut() = Some(remember_time_picker_state(initial_hour = 9, initial_minute = 30));
        *r.rail.borrow_mut() = Some(remember_wide_navigation_rail_state());
        *r.date.borrow_mut() = Some(remember_date_picker_state());
        *r.range.borrow_mut() = Some(remember_date_range_picker_state());
        let pane = remember_pane_expansion_state();
        *r.pane.borrow_mut() = Some(pane.clone());
        Column(
            modifier = Modifier.fill_max_size(),
            content = move || {
                SwipeToDismissBox(
                    swipe.clone(),
                    background_content = || Box(modifier = Modifier.fill_max_size()),
                    content = || Text("Satır", modifier = Modifier.fill_max_width().height(60.dp())),
                );
                // (Compose's pane expansion state is measured by the scaffold it splits — its width unspecified
                // until then —: a list and a detail side by side, as the C++'s other pane tests had them)
                let pane = pane.clone();
                Box(
                    modifier = Modifier.fill_max_width().height(100.dp()),
                    content = move || {
                        let directive = named_args!(
                            calculate_pane_scaffold_directive(&current_window_adaptive_info())
                                .copy(max_horizontal_partitions = 2)
                        );
                        let navigator: Rc<dyn ThreePaneScaffoldNavigator<i32>> =
                            remember_list_detail_pane_scaffold_navigator(scaffold_directive = directive);
                        NavigableListDetailPaneScaffold(
                            navigator,
                            list_pane = kompozable_lambda!(|_: &ThreePaneScaffoldPaneScope| {
                                Box(modifier = Modifier.fill_max_size().test_tag("liste"))
                            }),
                            detail_pane = kompozable_lambda!(|_: &ThreePaneScaffoldPaneScope| {
                                Box(modifier = Modifier.fill_max_size().test_tag("ayrıntı"))
                            }),
                            pane_expansion_state = Some(pane.clone()),
                        );
                    },
                );
            },
        );
        if sheet_shown.get() {
            ModalBottomSheet(
                || {},
                sheet_state = Some(sheet),
                content = || Text("Sayfa", modifier = Modifier.height(200.dp())),
            );
            ModalBottomSheet(
                || {},
                sheet_state = Some(half),
                content = || Text("Uzun", modifier = Modifier.height(600.dp())),
            );
        }
    }));
    rule
}

fn material_states_come_back_after_the_process_ended() {
    // Every Material state a remember function saves (the pane expansion state is adaptive's) comes back with the
    // UI's state: an open sheet open at once, not sliding in again; a swiped row off its edge; the rest as set.
    let date = |y, m, d| LocalDate::of(y, m, d);
    let bytes;
    let was_24_hour;
    {
        let r = Rc::new(Run::default());
        let (shown, sheet_shown) = (mutable_state_of(false), mutable_state_of(false));
        let rule = states_app(&r, &shown, &sheet_shown, 800);
        shown.set(true);
        rule.wait_for_idle();
        // swiped away to the start
        rule.host().pointer_down(Offset::new(350.0, 30.0), -1, 0, PointerType::Touch);
        for i in 1..=6 {
            rule.advance_frame();
            rule.host().pointer_move(Offset::new(350.0 - 50.0 * i as f32, 30.0), -1, 0);
        }
        rule.host().pointer_up(Offset::new(50.0, 30.0), -1, 0);
        rule.wait_for_idle();
        assert_eq!(get(&r.swipe).current_value(), SwipeToDismissBoxValue::EndToStart);
        sheet_shown.set(true);
        rule.wait_for_idle();
        // slid in
        assert_eq!(get(&r.sheet).current_value(), SheetValue::Expanded);
        assert_eq!(get(&r.half).current_value(), SheetValue::PartiallyExpanded);
        let bar = get(&r.bar);
        bar.set_height_offset_limit(-64.0);
        bar.set_height_offset(-40.0);
        bar.set_content_offset(-100.0);
        let scope = scope_of(&rule);
        let (drawer, search, rail) = (get(&r.drawer), get(&r.search), get(&r.rail));
        scope.launch(async move { drawer.snap_to(DrawerValue::Open).await });
        scope.launch(async move { search.snap_to(1.0).await }); // (M15's snapTo takes the fraction: expanded)
        scope.launch(async move { rail.expand().await });
        let toolbar = get(&r.toolbar);
        toolbar.set_offset_limit(-80.0); // (M15's limit is negative: how far it slides away)
        toolbar.set_offset(-30.0);
        let time = get(&r.time);
        time.set_hour(15);
        time.set_minute(42);
        time.set_selection(TimePickerSelectionMode::Minute);
        was_24_hour = time.is24hour();
        let date_state = get(&r.date);
        date_state.set_selected_date(Some(date(2026, 10, 12)));
        date_state.set_displayed_month(YearMonth::of(2026, 10));
        date_state.set_display_mode(DisplayMode::Input);
        let range = get(&r.range);
        range.set_selection_of_dates(Some(date(2026, 10, 1)), Some(date(2026, 10, 12)));
        range.set_displayed_month(YearMonth::of(2026, 9));
        get(&r.pane).set_first_pane_width(300);
        rule.wait_for_idle();
        assert!(near(bounds(&rule, "liste").width(), 300.0, 0.5), "{:?}", bounds(&rule, "liste"));
        bytes = rule.host().save_state();
    }
    let r = Rc::new(Run::default());
    let (shown, sheet_shown) = (mutable_state_of(false), mutable_state_of(false));
    let rule = states_app(&r, &shown, &sheet_shown, 1400); // a taller window: the long sheet is too short for half way now
    rule.host().restore_state(&bytes);
    shown.set(true);
    sheet_shown.set(true);
    rule.advance_frame();
    // The open sheets are open at once, not sliding in again. (M15's offset is the sheet's top in the window: at
    // its expanded anchor, the window's height less the sheet's, from the first frame; the C++'s offset was 0 there.
    // The long sheet, 648 dp tall, is under half the window now: no partly expanded anchor, expanded.)
    let (sheet, half) = (get(&r.sheet), get(&r.half));
    assert!(sheet.current_value() == SheetValue::Expanded && sheet.is_visible());
    assert!(half.current_value() == SheetValue::Expanded);
    let (sheet_at, half_at) = (sheet.require_offset(), half.require_offset());
    assert!(
        near(sheet_at, 1400.0 - 200.0 - 48.0, 0.5) && near(half_at, 1400.0 - 600.0 - 48.0, 0.5),
        "{sheet_at} {half_at}"
    );
    rule.wait_for_idle();
    assert!(near(sheet.require_offset(), sheet_at, 0.01) && near(half.require_offset(), half_at, 0.01));
    rule.wait_for_idle();
    let bar = get(&r.bar);
    assert!(near(bar.height_offset_limit(), -64.0, 0.01) && near(bar.height_offset(), -40.0, 0.01));
    assert!(near(bar.content_offset(), -100.0, 0.01));
    // (M15's DrawerState has no open fraction; its offset is its drawer's, none here: open, settled there)
    let drawer = get(&r.drawer);
    assert!(
        drawer.is_open() && drawer.current_value() == DrawerValue::Open && drawer.target_value() == DrawerValue::Open
    );
    let toolbar = get(&r.toolbar);
    assert!(near(toolbar.offset_limit(), -80.0, 0.01) && near(toolbar.offset(), -30.0, 0.01));
    let search = get(&r.search);
    assert!(search.current_value() == SearchBarValue::Expanded && near(search.progress(), 1.0, 0.01));
    let swipe = get(&r.swipe);
    assert!(swipe.current_value() == SwipeToDismissBoxValue::EndToStart && near(swipe.progress(), 1.0, 0.01));
    // off its start edge
    assert!(rule.on_node_with_text("Satır").get_unclipped_bounds_in_root().right < 1.0);
    let time = get(&r.time);
    // (M15's saver keeps the hour, the minute and the clock, not the selection — the C++'s kept it: the hour's again)
    assert_eq!((time.hour(), time.minute(), time.selection()), (15, 42, TimePickerSelectionMode::Hour));
    // (the clock the state had: the locale's, English's 12 hours here; the C++'s locale had 24)
    assert_eq!(time.is24hour(), was_24_hour);
    assert_eq!(get(&r.rail).target_value(), WideNavigationRailValue::Expanded);
    let date_state = get(&r.date);
    assert_eq!(date_state.get_selected_date(), Some(date(2026, 10, 12)));
    assert_eq!(date_state.get_displayed_month(), YearMonth::of(2026, 10));
    assert_eq!(date_state.display_mode(), DisplayMode::Input);
    let range = get(&r.range);
    assert_eq!(range.get_selected_start_date(), Some(date(2026, 10, 1)));
    assert_eq!(range.get_selected_end_date(), Some(date(2026, 10, 12)));
    assert_eq!(range.get_displayed_month(), YearMonth::of(2026, 9));
    // (Compose's `firstPaneWidth` is internal: the list laid out as wide again)
    assert!(!get(&r.pane).is_unspecified());
    assert!(near(bounds(&rule, "liste").width(), 300.0, 0.5), "{:?}", bounds(&rule, "liste"));
}
