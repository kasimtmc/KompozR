//! Android's accessibility, content capture, translation, autofill and scroll capture as they read the host and act
//! on it (kompoz-platform's `android::semantics`, plain Rust on every host): a screen with a heading, a button, a
//! switch, a field and a list that scrolls, seen as Android's virtual views (classes, texts, states, actions, scroll
//! axes, lines, characters' boxes) and acted on as TalkBack would (clicks, scrolls by direction, onto the screen,
//! texts and selections set); its texts captured, translated and shown back; a form's autofill structure and what
//! a service fills in; the scrollable a long screenshot captures, scrolled by offsets. One test after another on
//! this thread (the UI's).

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_platform::accessibility::{Snapshot, Window};
use kompoz_platform::android::accessibility::{
    AndroidTree, HOST_VIEW_ID, Request, action, android_tree, change, events, flag, granularity,
};
use kompoz_platform::android::autofill::{AutofillValue, autofill_type};
use kompoz_platform::android::capture::{CaptureEvent, CaptureTree, capture_events};
use kompoz_platform::android::semantics::{self, RelativeScroller, scroll_into};
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::semantics::{ContentDataType, ContentType, FillableData, Role, SemanticsProperties};
use kompoz_ui::{KompozHost, Modifier};
use std::cell::Cell;
use std::rc::Rc;

/// The host's tree as Android sees it (and the snapshot it is of).
fn tree(host: &KompozHost) -> (Snapshot, AndroidTree) {
    let nodes = host.accessibility_tree();
    let extras = semantics::extras(host, &nodes);
    let snapshot = Snapshot::new(nodes, Window::default());
    let tree = android_tree(&snapshot, &extras);
    (snapshot, tree)
}

/// Waits with the clock going on its own (animations to their end).
fn settle(rule: &KompozTestRule) {
    let clock = rule.main_clock();
    clock.set_auto_advance(true);
    rule.wait_for_idle();
}

fn id(rule: &KompozTestRule, tag: &str) -> u64 {
    rule.on_node_with_tag(tag).fetch_semantics_node().id()
}

fn a_screen_is_androids_virtual_views_and_takes_talkbacks_actions() {
    let clicks = Rc::new(Cell::new(0));
    let on = mutable_state_of(false);
    let scroll = ScrollState::new(0);
    let field = TextFieldState::new("merhaba dünya");
    let (c, o, sc, f) = (clicks.clone(), on.clone(), scroll.clone(), field.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (c, o, sc, f) = (c.clone(), o.clone(), sc.clone(), f.clone());
            Column(
                modifier = Modifier.fill_max_size(),
                content = move || {
                    Text("Başlık", modifier = Modifier.semantics(|s| s.heading()).test_tag("başlık"));
                    let c = c.clone();
                    Box(
                        modifier = Modifier.width(80.dp()).height(40.dp()).test_tag("düğme").clickable_with(
                            true,
                            None,
                            Some(Role::Button),
                            None,
                            move || c.set(c.get() + 1),
                        ),
                        content = || Text("Tamam"),
                    );
                    let o2 = o.clone();
                    Box(modifier = Modifier.width(80.dp()).height(40.dp()).test_tag("anahtar").toggleable_with(
                        o.get(),
                        true,
                        Some(Role::Switch),
                        None,
                        move |v| o2.set(v),
                    ));
                    BasicTextField(f.clone(), modifier = Modifier.width(200.dp()).test_tag("alan"));
                    Column(
                        modifier = Modifier.height(100.dp()).fill_max_width().vertical_scroll(&sc).test_tag("liste"),
                        content = || {
                            for i in 0..20 {
                                Text(
                                    format!("Satır {i}"),
                                    modifier = Modifier.height(30.dp()).test_tag(format!("s{i}")),
                                );
                            }
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    settle(&rule);
    let host = rule.host();
    let (s1, t1) = tree(host);
    let vid = |tag: &str| id(&rule, tag) as i32;
    let host_node = t1.get(HOST_VIEW_ID).expect("the view itself");
    assert!(!host_node.children.is_empty());

    let heading = t1.get(vid("başlık")).unwrap();
    assert_eq!((heading.class_name, heading.text.as_deref()), ("android.widget.TextView", Some("Başlık")));
    assert!(heading.flags & flag::HEADING != 0 && heading.granularities & granularity::LINE != 0);
    assert_eq!(heading.lines, [(0, 0.0)]);
    let boxes = semantics::character_bounds(host, heading.semantics_id, 0, 6);
    assert_eq!(boxes.len(), 6);
    assert!(
        boxes
            .iter()
            .all(|b| b.is_some_and(|b| b.left >= heading.bounds.left - 0.5 && b.right <= heading.bounds.right + 0.5))
    );

    let button = t1.get(vid("düğme")).unwrap();
    assert_eq!((button.class_name, button.text.as_deref()), ("android.widget.Button", Some("Tamam")));
    assert!(button.actions & action::CLICK != 0 && button.flags & flag::SCREEN_READER_FOCUSABLE != 0);
    let center = |b: Rect| ((b.left + b.right) / 2.0, (b.top + b.bottom) / 2.0);
    let (x, y) = center(button.bounds);
    assert_eq!(t1.hit(x, y), button.id);
    assert!(semantics::perform(host, button.semantics_id, Request::Click));
    assert_eq!(clicks.get(), 1);

    let switch = t1.get(vid("anahtar")).unwrap();
    assert_eq!(switch.class_name, "android.widget.Switch");
    assert!(switch.flags & flag::CHECKABLE != 0 && switch.flags & flag::CHECKED == 0);
    assert!(semantics::perform(host, switch.semantics_id, Request::Click));
    settle(&rule);
    assert!(on.get());
    let (s2, t2) = tree(host);
    assert!(t2.get(switch.id).unwrap().flags & flag::CHECKED != 0);
    let told = events((&s1, &t1), (&s2, &t2));
    assert!(told.iter().any(|e| matches!(e, kompoz_platform::android::accessibility::Event::ContentChanged { id, changes } if *id == switch.id && changes & change::CHECKED != 0)), "{told:?}");

    let alan = t2.get(vid("alan")).unwrap();
    assert_eq!((alan.class_name, alan.text.as_deref()), ("android.widget.EditText", Some("merhaba dünya")));
    assert!(alan.actions & action::SET_TEXT != 0 && alan.flags & flag::EDITABLE != 0);
    // (Not focused: no lines for TalkBack, Compose's.)
    assert_eq!(alan.granularities & granularity::LINE, 0);
    assert!(semantics::perform(host, alan.semantics_id, Request::SetText("selam dünya".into())));
    settle(&rule);
    assert_eq!(field.text().as_str(), "selam dünya");
    // A selection in UTF-16 offsets: "selam d|ün|ya" are bytes 7 and 10.
    assert!(semantics::perform(host, alan.semantics_id, Request::SetSelection { start: 7, end: 9, traversal: false }));
    settle(&rule);
    assert_eq!((field.selection().start, field.selection().end), (7, 10));
    let (_, t3) = tree(host);
    assert_eq!(t3.get(alan.id).unwrap().selection, Some((7, 9)));

    let list = t3.get(vid("liste")).unwrap();
    assert_eq!(list.class_name, "android.widget.ScrollView");
    let (value, max) = list.scroll_y.unwrap();
    assert!(value == 0 && max > 0 && list.flags & flag::SCROLLABLE != 0);
    assert!(
        list.actions & (action::SCROLL_FORWARD | action::SCROLL_DOWN) == action::SCROLL_FORWARD | action::SCROLL_DOWN
    );
    assert_eq!(list.actions & action::SCROLL_UP, 0);
    assert!(semantics::perform(host, list.semantics_id, Request::ScrollDown));
    settle(&rule);
    let down = scroll.value();
    assert!(down > 0, "{down}");
    assert!(semantics::perform(host, list.semantics_id, Request::ScrollUp));
    settle(&rule);
    assert_eq!(scroll.value(), 0);
    // The last row shown: the list scrolled just enough.
    assert!(semantics::perform(host, id(&rule, "s19"), Request::ShowOnScreen));
    settle(&rule);
    assert_eq!(scroll.value(), scroll.max_value());
    // Nothing scrolls above the heading: the view's own parents are asked.
    assert!(!semantics::perform(host, heading.semantics_id, Request::ShowOnScreen));
    assert_eq!(scroll_into(Rect::new(0.0, 0.0, 100.0, 100.0), Rect::new(0.0, 150.0, 10.0, 180.0)), (0.0, 80.0));
    assert_eq!(scroll_into(Rect::new(0.0, 0.0, 100.0, 100.0), Rect::new(0.0, -50.0, 10.0, 150.0)), (0.0, 0.0));
}

fn texts_are_captured_translated_and_autofilled() {
    let greeting = mutable_state_of("Merhaba".to_string());
    let user = TextFieldState::new("");
    let (g, u) = (greeting.clone(), user.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (g, u) = (g.clone(), u.clone());
        Column(move || {
            Text(g.get(), modifier = Modifier.test_tag("selam"));
            BasicTextField(
                u.clone(),
                modifier = Modifier
                    .semantics(|c| c.set_content_type(ContentType::Username()))
                    .width(200.dp())
                    .test_tag("kullanıcı"),
            );
        });
    }));
    settle(&rule);
    let host = rule.host();
    let selam = rule.on_node_with_tag_unmerged("selam").fetch_semantics_node().id();

    // Content capture: everything appears, then a text changes.
    let first = CaptureTree::new(semantics::capture_nodes(host));
    let node = first.get(selam).expect("the text is captured");
    assert_eq!((node.class_name, node.text.as_deref()), (Some("android.widget.TextView"), Some("Merhaba")));
    assert!(node.text_size.is_some_and(|s| s > 0.0));
    let appeared = capture_events(&CaptureTree::default(), &first);
    assert!(appeared.iter().any(|e| matches!(e, CaptureEvent::Appeared(n) if n.id == selam)));
    greeting.set("Günaydın".into());
    settle(&rule);
    let second = CaptureTree::new(semantics::capture_nodes(host));
    assert!(capture_events(&first, &second).contains(&CaptureEvent::TextChanged(selam, "Günaydın".into())));

    // Translation: asked for, set, shown, hidden, cleared.
    assert_eq!(semantics::translation_text(host, selam).as_deref(), Some("Günaydın"));
    assert!(semantics::set_translation(host, selam, "Good morning"));
    semantics::show_translations(host, Some(true));
    settle(&rule);
    let showing = |rule: &KompozTestRule| {
        rule.on_node_with_tag_unmerged("selam")
            .fetch_semantics_node()
            .unmerged_config()
            .get(SemanticsProperties::IsShowingTextSubstitution)
    };
    assert_eq!(showing(&rule), Some(true));
    assert!(rule.drawn_texts().iter().any(|t| t.contains("Good morning")), "{:?}", rule.drawn_texts());
    semantics::show_translations(host, Some(false));
    settle(&rule);
    assert_eq!(showing(&rule), Some(false));
    semantics::show_translations(host, None);
    settle(&rule);
    assert_eq!(showing(&rule), None);

    // Autofill: the field in the view's structure, then filled.
    let structure = semantics::autofill_nodes(host);
    assert_eq!(structure[0].id, HOST_VIEW_ID);
    let field_id = id(&rule, "kullanıcı");
    let field = structure.iter().find(|n| n.id == field_id as i32).expect("the field's structure");
    assert_eq!((field.autofill_type, field.class_name), (autofill_type::TEXT, Some("android.widget.EditText")));
    assert_eq!(field.hints, ["username"]);
    assert_eq!(field.value, Some(AutofillValue::Text(String::new())));
    assert_eq!(semantics::content_data_type(host, field_id), Some(ContentDataType::Text));
    assert!(host.perform_autofill(field_id, FillableData::Text("ada".into())));
    settle(&rule);
    assert_eq!(user.text().as_str(), "ada");
}

fn a_long_screenshot_captures_the_deepest_tallest_scrollable() {
    let outer = ScrollState::new(0);
    let inner = ScrollState::new(0);
    let (o, i) = (outer.clone(), inner.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (o, i) = (o.clone(), i.clone());
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&o).test_tag("dış"),
                content = move || {
                    Column(
                        modifier = Modifier.height(200.dp()).fill_max_width().vertical_scroll(&i).test_tag("iç"),
                        content = || {
                            for n in 0..30 {
                                Text(format!("Satır {n}"), modifier = Modifier.height(30.dp()));
                            }
                        },
                    );
                    Box(modifier = Modifier.height(1000.dp()));
                },
            );
        }),
        IntSize::new(400, 600),
    );
    settle(&rule);
    let host = rule.host();
    let (found, viewport) = semantics::scroll_capture_candidate(host).expect("a scrollable");
    assert_eq!(found, id(&rule, "iç"));
    assert!(viewport.bottom - viewport.top > 0);
    // Scrolled by an offset in a coroutine: how far it went.
    let went = Rc::new(Cell::new(None));
    let w = went.clone();
    let scroll = semantics::scroll_by_offset(host, found, 90.0).expect("it scrolls by offsets");
    let scope = kompoz_coroutines::CoroutineScope::new(host.dispatcher());
    scope.launch(async move { w.set(Some(scroll.await)) });
    settle(&rule);
    assert_eq!(went.get(), Some(90.0));
    assert_eq!(inner.value(), 90);
    // Where scroll capture puts what it asks for.
    let r = RelativeScroller { viewport: 200, amount: 0.0 };
    assert_eq!(r.delta_to_center(0, 100), None);
    assert_eq!(r.delta_to_center(300, 400), Some(250.0));
    let moved = RelativeScroller { viewport: 200, amount: 250.0 };
    assert_eq!((moved.to_viewport(300), moved.to_viewport(400), moved.to_viewport(500)), (50, 150, 200));
}

fn main() {
    kompoz_runtime::set_ui_thread();
    let tests: &[(&str, fn())] = &[
        (
            "a_screen_is_androids_virtual_views_and_takes_talkbacks_actions",
            a_screen_is_androids_virtual_views_and_takes_talkbacks_actions,
        ),
        ("texts_are_captured_translated_and_autofilled", texts_are_captured_translated_and_autofilled),
        (
            "a_long_screenshot_captures_the_deepest_tallest_scrollable",
            a_long_screenshot_captures_the_deepest_tallest_scrollable,
        ),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let ok = std::panic::catch_unwind(test).is_ok();
        failed += usize::from(!ok);
        println!("{} {name}", if ok { "ok    " } else { "FAILED" });
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
