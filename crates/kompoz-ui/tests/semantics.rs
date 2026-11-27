//! The semantics tests: the properties and actions and how they merge and combine, merging, clearing and hiding,
//! fake nodes, the reading order, the change listeners, and what accessibility sees and does (the C++ Kompoz's
//! semantics tests without Material: clickables, toggleables and selectables for its buttons, switches, checkboxes
//! and radio buttons, semantics modifiers for its icons). Numbers in px.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::semantics::*;
use kompoz_ui::window::Dialog;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn find<'a>(tree: &'a [AccessibilityNode], label: &str) -> Option<&'a AccessibilityNode> {
    tree.iter().find(|n| n.label == label)
}

fn any(tree: &[AccessibilityNode], part: &str) -> bool {
    tree.iter().any(|n| n.label.contains(part))
}

/// A clickable box, its role a button (what a Material button is to semantics): its clicks count.
#[kompozable(inline)]
fn Button(clicks: Rc<Cell<i32>>, content: impl FnOnce()) {
    Box(
        Modifier.clickable_with(true, None, Some(Role::Button), None, move || clicks.set(clicks.get() + 1)),
        content = content,
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Ported from C++

fn semantics_set_on_a_receiver() {
    // C++'s semantics_set_on_a_receiver: semantics { }, semantics(mergeDescendants = true) { },
    // clearAndSetSemantics { }.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Text("Ayarlar", modifier = Modifier.semantics(|c| c.heading()));
                Row(
                    Modifier.semantics(merge_descendants = true, |c| c.set_state_description("açık")),
                    content = || {
                        Text("Wi-Fi");
                        Text("Bağlı");
                    },
                );
                Box(
                    Modifier.clear_and_set_semantics(|c| c.set_content_description("Puan: 4 / 5")),
                    content = || {
                        Text("4/5");
                    },
                );
            },
        );
    }));
    let tree = rule.host().accessibility_tree();
    assert!(find(&tree, "Ayarlar").is_some_and(|n| n.heading));
    let merged = tree.iter().find(|n| n.label.contains("Wi-Fi") && n.label.contains("Bağlı"));
    assert_eq!(merged.map(|n| n.state_description.as_str()), Some("açık"));
    assert!(find(&tree, "Puan: 4 / 5").is_some() && !any(&tree, "4/5"));
    // The same through the test rule's finders (Compose's merged tree).
    rule.on_node_with_text("Wi-Fi").assert_text_equals(&["Wi-Fi", "Bağlı"]).assert_value_equals("açık");
    rule.on_node_with_text("4/5").assert_does_not_exist();
    rule.on_node_with(has_text("4/5", false, false), true).assert_exists();
}

fn semantics_merge_clear_and_hide() {
    // C++'s semantics_merge_clear_and_hide: semantics boxes for its icons, clickables for its buttons.
    let clicks = Rc::new(Cell::new(0));
    let c1 = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c1 = c1.clone();
        Column(
            content = move || {
                let c1 = c1.clone();
                Row(
                    Modifier.semantics_with(true, |_| {}),
                    content = move || {
                        // read as one
                        Box(Modifier.size(10.dp()).semantics(|c| c.set_content_description("Yıldızlı")));
                        Text("Favoriler");
                        Text("gizli", modifier = Modifier.semantics(|c| c.hide_from_accessibility())); // not part of it
                        Box(
                            Modifier.clear_and_set_semantics(|c| c.set_content_description("3 yorum")),
                            content = || {
                                Text("üç"); // its own words only
                            },
                        );
                        Button(c1.clone(), || Text("Aç")); // interactive: on its own
                    },
                );
                Box(
                    Modifier.clear_and_set_semantics(|c| c.set_content_description("Puan: 4 / 5")),
                    content = || {
                        Row(content = || {
                            for _ in 0..4 {
                                Box(Modifier.size(10.dp()).semantics(|c| c.set_content_description("yıldız")));
                            }
                            Text("4/5");
                            Button(Rc::new(Cell::new(0)), || Text("İç düğme"));
                        });
                    },
                );
                Text("Süs", modifier = Modifier.semantics(|c| c.hide_from_accessibility()));
                Box(
                    Modifier.semantics(|c| c.hide_from_accessibility()),
                    content = || {
                        Button(Rc::new(Cell::new(0)), || Text("Gizli düğme"));
                    },
                );
                // Two semantics modifiers on one node: the toggle's role and state, the description after.
                Box(Modifier
                    .size(40.dp())
                    .toggleable(true, role = Some(Role::Switch), on_value_change = |_| {})
                    .semantics(|c| c.set_content_description("Wi-Fi"))
                    .test_tag("anahtar"));
                Text("Düz metin");
                Text("Temizlenen metin", modifier = Modifier.clear_and_set_semantics(|_| {})); // says nothing, not its text
                Text(
                    "Süs 2",
                    modifier = Modifier.semantics(|c| c.heading()).semantics(|c| c.hide_from_accessibility()),
                );
                // The first role set wins (the outer modifier is applied last); a state set only later is kept.
                Box(Modifier.size(40.dp()).semantics(|c| c.set_role(Role::Tab)).semantics(|c| {
                    c.set_content_description("Sekme");
                    c.set_role(Role::Image);
                }));
                Box(Modifier.size(40.dp()).semantics(|c| c.set_content_description("Bildirim")).toggleable(
                    false,
                    role = Some(Role::Switch),
                    on_value_change = |_| {},
                ));
            },
        );
    }));
    let tree = rule.host().accessibility_tree();
    let row = find(&tree, "Yıldızlı Favoriler 3 yorum");
    let open = find(&tree, "Aç");
    assert!(row.is_some() && open.is_some(), "{tree:#?}");
    assert!(open.unwrap().parent == Some(row.unwrap().id) && open.unwrap().clickable);
    assert!(find(&tree, "Favoriler").is_none() && find(&tree, "Yıldızlı").is_none()); // merged, not on their own
    let rating = find(&tree, "Puan: 4 / 5");
    assert!(rating.is_some() && !any(&tree, "yıldız") && !any(&tree, "4/5") && !any(&tree, "İç düğme"));
    assert!(!any(&tree, "Süs") && !any(&tree, "Gizli düğme"));
    let wifi = find(&tree, "Wi-Fi").expect("Wi-Fi");
    assert!(wifi.role == Some(Role::Switch) && wifi.checked == Some(true) && wifi.clickable);
    assert!(find(&tree, "Düz metin").is_some());
    assert!(!any(&tree, "gizli") && !any(&tree, "üç") && !any(&tree, "Temizlenen metin") && !any(&tree, "Süs 2"));
    assert_eq!(find(&tree, "Sekme").and_then(|n| n.role), Some(Role::Tab));
    let bell = find(&tree, "Bildirim").expect("Bildirim");
    assert!(bell.role == Some(Role::Switch) && bell.checked == Some(false));
    rule.on_node_with_tag("anahtar").assert_is_on().assert_content_description_equals(&["Wi-Fi"]);
    assert!(rule.on_node_with_content_description("Wi-Fi").exists());
    assert!(rule.on_node_with_content_description("Puan: 4 / 5").exists());
    // Compose's merged tree keeps a hidden child's text in its merging parent (accessibility leaves it out).
    rule.on_node_with_text("Favoriler").assert_text_contains("gizli");
    // The click goes to the button itself.
    assert!(rule.host().perform_semantics_action(open.unwrap().id, SemanticsAction::Click));
    assert_eq!(clicks.get(), 1);
}

fn selectable_groups_are_told_to_accessibility() {
    // C++'s selectable_groups_are_told_to_accessibility.
    let chosen = mutable_state_of(1);
    let ch = chosen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let ch = ch.clone();
        Row(
            Modifier.selectable_group().test_tag("grup"),
            content = move || {
                for i in 0..3 {
                    let (selected, ch) = (ch.get() == i, ch.clone());
                    Box(Modifier.size(40.dp()).selectable(
                        selected,
                        role = Some(Role::RadioButton),
                        on_click = move || ch.set(i),
                    ));
                }
            },
        );
        Row(Modifier.test_tag("düz"), content = || Text("grup değil"));
    }));
    let tree = rule.host().accessibility_tree();
    let groups: Vec<_> = tree.iter().filter(|n| n.selectable_group).collect();
    assert_eq!(groups.len(), 1);
    let id = groups[0].id;
    let items: Vec<_> = tree.iter().filter(|n| n.parent == Some(id)).collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items.iter().filter(|n| n.checked == Some(true)).count(), 1);
    // Clicked through accessibility: the choice moves.
    assert!(rule.host().perform_semantics_action(items[2].id, SemanticsAction::Click));
    rule.wait_for_idle();
    assert_eq!(chosen.peek(), 2);
    rule.on_all_nodes(has_parent(has_test_tag("grup"))).assert_count_equals(3).get(2).assert_is_selected();
}

fn semantics_ranges_live_regions_custom_actions_and_order() {
    // C++'s semantics_ranges_live_regions_custom_actions_and_order without its Material slider and progress
    // indicators: a live region, custom actions (the latest composition's called), two semantics on one node,
    // traversal order.
    let said = Rc::new(RefCell::new(Vec::new()));
    let round = mutable_state_of(0);
    let (s, r) = (said.clone(), round.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, r) = (s.clone(), r.clone());
        Column(
            content = move || {
                let n = r.get();
                Text(
                    format!("{n} yeni ileti"),
                    modifier = Modifier.semantics(|c| c.set_live_region(LiveRegionMode::Polite)),
                );
                let s = s.clone();
                Box(Modifier.size(10.dp()).semantics(move |c| {
                    let s = s.clone();
                    c.set_content_description("İleti");
                    c.set_custom_actions(vec![
                        CustomAccessibilityAction::new("Arşivle", move || {
                            s.borrow_mut().push(format!("arşivle {n}"));
                            true
                        }),
                        CustomAccessibilityAction::new("Sil", || false),
                    ]);
                }));
                // Two semantics on one node: their actions together (the outer's first), the outer's range, live
                // region and index.
                Box(Modifier
                    .size(10.dp())
                    .semantics(|c| {
                        c.set_content_description("iki");
                        c.set_progress_bar_range_info(ProgressBarRangeInfo::new(1.0, 0.0..=2.0, 0));
                        c.set_custom_actions(vec![CustomAccessibilityAction::new("bir", || true)]);
                    })
                    .semantics(|c| {
                        c.set_progress_bar_range_info(ProgressBarRangeInfo::new(9.0, 0.0..=9.0, 0));
                        c.set_progress(|_| true);
                        c.set_live_region(LiveRegionMode::Assertive);
                        c.set_custom_actions(vec![CustomAccessibilityAction::new("iki", || true)]);
                        c.set_traversal_index(3.0);
                    }));
                // Read in the order their traversal indexes give: b, c, then a and what is inside it.
                Column(
                    Modifier.semantics(|c| c.set_content_description("sıra")),
                    content = || {
                        Column(
                            Modifier.semantics(|c| {
                                c.set_content_description("a");
                                c.set_traversal_index(2.0);
                            }),
                            content = || Text("a1"),
                        );
                        Text("b", modifier = Modifier.semantics(|c| c.set_traversal_index(-1.0)));
                        Text("c");
                    },
                );
            },
        );
    }));
    let tree = || rule.host().accessibility_tree();
    let node = |label: &str| tree().into_iter().find(|n| n.label == label).unwrap_or_else(|| panic!("{label}"));
    assert_eq!(node("0 yeni ileti").live_region, Some(LiveRegionMode::Polite));
    assert_eq!(node("İleti").live_region, None);
    assert_eq!(node("İleti").custom_actions, ["Arşivle", "Sil"]);
    round.set(1);
    rule.wait_for_idle();
    let message = node("İleti");
    assert!(rule.host().perform_semantics_action(message.id, SemanticsAction::CustomAction(0)));
    assert!(!rule.host().perform_semantics_action(message.id, SemanticsAction::CustomAction(1))); // it couldn't
    assert!(!rule.host().perform_semantics_action(message.id, SemanticsAction::CustomAction(2))); // there is none
    assert_eq!(*said.borrow(), ["arşivle 1"]);
    let two = node("iki");
    // (the outer modifier is applied last: its values win, Compose's semantics order)
    assert_eq!(two.custom_actions, ["bir"]);
    assert_eq!(two.range, Some(ProgressBarRangeInfo::new(1.0, 0.0..=2.0, 0)));
    assert!(two.settable && two.live_region == Some(LiveRegionMode::Assertive));
    // Traversal order among the nodes of a parent, parents before what is inside them.
    let all = tree();
    let parent = node("sıra").id;
    let mut order = Vec::new();
    fn walk(all: &[AccessibilityNode], p: u64, order: &mut Vec<String>) {
        for n in all.iter().filter(|n| n.parent == Some(p)) {
            order.push(n.label.clone());
            walk(all, n.id, order);
        }
    }
    walk(&all, parent, &mut order);
    assert_eq!(order, ["b", "c", "a", "a1"]);
    let mut listed = Vec::new();
    let mut inside = false;
    for n in &all {
        if n.label == "sıra" {
            inside = true;
        } else if inside {
            listed.push(n.label.clone());
        }
    }
    assert_eq!(listed, ["b", "c", "a", "a1", "iki"]);
}

fn semantics_tree_merges_buttons_and_performs_actions() {
    // C++'s semantics_tree_merges_buttons_and_performs_actions without Material: a clickable for its button, a
    // toggleable for its checkbox, a heading, and a dialog that makes the tree its own.
    let clicks = Rc::new(Cell::new(0));
    let checked = mutable_state_of(true);
    let dialog = mutable_state_of(false);
    let (c1, ch, dl) = (clicks.clone(), checked.clone(), dialog.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c1, ch, dl) = (c1.clone(), ch.clone(), dl.clone());
        Column(
            content = move || {
                Text("Başlık", modifier = Modifier.semantics(|c| c.heading()));
                Button(c1.clone(), || {
                    Box(Modifier.size(10.dp()).semantics(|c| c.set_content_description("Artı")));
                    Text("Ekle");
                });
                let on = ch.get();
                let ch2 = ch.clone();
                Box(Modifier.size(20.dp()).toggleable(
                    on,
                    role = Some(Role::Checkbox),
                    on_value_change = move |v| ch2.set(v),
                ));
                if dl.get() {
                    Dialog(on_dismiss_request = || {}, content = || Text("Uyarı"));
                }
            },
        );
    }));
    let tree = rule.host().accessibility_tree();
    let button = tree.iter().find(|n| n.role == Some(Role::Button)).cloned().expect("button");
    let checkbox = tree.iter().find(|n| n.role == Some(Role::Checkbox)).cloned().expect("checkbox");
    assert!(button.label == "Artı Ekle" && button.clickable);
    assert!(find(&tree, "Ekle").is_none()); // merged into the button
    assert!(checkbox.checked == Some(true) && checkbox.label.is_empty());
    assert!(find(&tree, "Başlık").is_some_and(|n| n.heading));
    assert!(rule.host().perform_semantics_action(button.id, SemanticsAction::Click) && clicks.get() == 1);
    assert!(rule.host().perform_semantics_action(checkbox.id, SemanticsAction::Click));
    rule.wait_for_idle();
    assert!(!checked.peek());
    // Ids stay.
    let again = rule.host().accessibility_tree();
    assert!(again.iter().any(|n| n.id == checkbox.id && n.checked == Some(false)));
    dialog.set(true);
    rule.wait_for_idle();
    let modal = rule.host().accessibility_tree();
    let dialog_node = modal.iter().find(|n| n.is_dialog).expect("the dialog");
    assert!(find(&modal, "Uyarı").is_some_and(|n| n.parent == Some(dialog_node.id)), "{modal:#?}");
    assert!(!modal.iter().any(|n| n.role == Some(Role::Button)));
}

fn semantics_tree_has_the_keyboard_focus() {
    // C++'s semantics_tree_has_the_keyboard_focus with clickables for its buttons (no Material): the keyboard
    // focus is in the tree wherever Tab took it (a clickable takes it in keyboard mode), and Focus puts it on what
    // can take it.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(Modifier.clickable(role = Some(Role::Button), on_click = || {}), content = || Text("Bir"));
                Box(
                    Modifier.clickable(enabled = false, role = Some(Role::Button), on_click = || {}),
                    content = || Text("Kapalı"),
                );
                Box(Modifier.clickable(role = Some(Role::Button), on_click = || {}), content = || Text("İki"));
                Box(
                    Modifier
                        .focus_properties(|p| p.can_focus = false)
                        .clickable(role = Some(Role::Button), on_click = || {}),
                    content = || Text("Odaksız"),
                );
                Text("Etiket");
            },
        );
    }));
    let focused_label =
        || rule.host().accessibility_tree().into_iter().find(|n| n.focused).map_or("(none)".to_string(), |n| n.label);
    let id_of = |label: &str| rule.host().accessibility_tree().into_iter().find(|n| n.label == label).unwrap().id;
    assert_eq!(focused_label(), "(none)");
    rule.host().key_down(input::Key::Tab, input::KeyModifiers::None);
    assert_eq!(focused_label(), "Bir");
    rule.host().key_down(input::Key::Tab, input::KeyModifiers::None); // past the disabled one
    assert_eq!(focused_label(), "İki");
    assert!(rule.host().perform_semantics_action(id_of("Bir"), SemanticsAction::Focus));
    assert_eq!(focused_label(), "Bir");
    assert!(!rule.host().perform_semantics_action(id_of("Kapalı"), SemanticsAction::Focus));
    assert!(!rule.host().perform_semantics_action(id_of("Etiket"), SemanticsAction::Focus));
    assert!(!rule.host().perform_semantics_action(id_of("Odaksız"), SemanticsAction::Focus)); // its properties say no
    assert_eq!(focused_label(), "Bir");
    assert!(!rule.host().accessibility_tree().into_iter().find(|n| n.label == "Kapalı").unwrap().enabled);
}

// ---------------------------------------------------------------------------------------------------------------
// Compose's behaviour

fn configurations_set_merge_and_combine_as_compose() {
    let mut c = SemanticsConfiguration::new();
    c.set_text("a");
    c.set(SemanticsActions::OnClick, AccessibilityAction::<ActionFn>::new(Some("aç".into()), Some(Rc::new(|| true))));
    // An action set again without a label keeps the label it had (Compose's AccessibilityAction rule).
    c.on_click(|| false);
    let click = c.get(SemanticsActions::OnClick).unwrap();
    assert_eq!(click.label.as_deref(), Some("aç"));
    assert!(!(click.action.unwrap())());
    let mut child = SemanticsConfiguration::new();
    child.set_text("b");
    child.set_content_description("d");
    child.set_test_tag("çocuk");
    child.set_role(Role::Image);
    let mut parent = c.copy();
    parent.set_role(Role::Button);
    parent.merge_child(&child);
    assert_eq!(parent.get(SemanticsProperties::Text), Some(vec!["a".into(), "b".into()]));
    assert_eq!(parent.get(SemanticsProperties::ContentDescription), Some(vec!["d".into()]));
    assert_eq!(parent.get(SemanticsProperties::TestTag), None); // a parent keeps its own (none)
    assert_eq!(parent.get(SemanticsProperties::Role), Some(Role::Button));
    // A peer's: only what is not set here, an action's label taken where this one lacks it.
    let mut a = SemanticsConfiguration::new();
    a.on_long_click(|| true);
    let mut b = SemanticsConfiguration::new();
    b.set(
        SemanticsActions::OnLongClick,
        AccessibilityAction::<ActionFn>::new(Some("uzun".into()), Some(Rc::new(|| false))),
    );
    b.set_text("peer");
    a.collapse_peer(&b);
    let long = a.get(SemanticsActions::OnLongClick).unwrap();
    assert!(long.label.as_deref() == Some("uzun") && (long.action.unwrap())());
    assert_eq!(a.get(SemanticsProperties::Text), Some(vec!["peer".into()]));
    assert_eq!(
        format!("{:?}", {
            let mut x = SemanticsConfiguration::new();
            x.heading();
            x
        }),
        "Heading : ()"
    );
    assert!(a.contains_important_for_accessibility());
    let mut unimportant = SemanticsConfiguration::new();
    unimportant.set_test_tag("x");
    assert!(!unimportant.contains_important_for_accessibility());
    // Unmergeable ones panic when merged, as Compose's throw.
    let mut popup = SemanticsConfiguration::new();
    popup.popup();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut p = SemanticsConfiguration::new();
        p.popup();
        p.merge_child(&popup);
    }));
    assert!(r.is_err());
}

fn trees_merge_clear_and_add_fake_nodes() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(
                    Modifier.test_tag("düğme").semantics(merge_descendants = true, |c| {
                        c.set_role(Role::Button);
                        c.set_content_description("açıklama");
                    }),
                    content = || {
                        Text("Tamam");
                        Box(
                            Modifier.test_tag("iç").semantics(merge_descendants = true, |_| {}),
                            content = || Text("Ayrı"),
                        );
                    },
                );
                Box(
                    Modifier.test_tag("temiz").clear_and_set_semantics(|c| c.set_text("yerine")),
                    content = || Text("eski"),
                );
            },
        );
    }));
    let button = rule.on_node_with_tag("düğme").fetch_semantics_node();
    assert_eq!(button.config().get(SemanticsProperties::Text), Some(vec!["Tamam".into()]));
    // In the merged tree its children are the merging nodes below it; in the unmerged one all of them.
    assert_eq!(button.children().len(), 1);
    let unmerged = rule.on_node_with_tag_unmerged("düğme").fetch_semantics_node();
    assert_eq!(unmerged.children().len(), 2);
    // Accessibility (which reads the unmerged tree) sees its fake nodes: its description before its children, its
    // role after.
    let replaced = unmerged.replaced_children();
    assert_eq!(replaced.len(), 4);
    assert!(
        replaced[0].is_fake()
            && replaced[0].config().get(SemanticsProperties::ContentDescription) == Some(vec!["açıklama".into()])
    );
    assert!(replaced[3].is_fake() && replaced[3].config().get(SemanticsProperties::Role) == Some(Role::Button));
    assert!(replaced[0].parent().is_some_and(|p| p.id() == button.id()));
    assert_ne!(replaced[0].id(), replaced[3].id());
    assert!(button.replaced_children().iter().all(|c| !c.is_fake())); // none in the merged tree
    // A clearing node has no children in the merged tree; its own words only.
    let cleared = rule.on_node_with_tag("temiz").fetch_semantics_node();
    assert!(cleared.children().is_empty());
    rule.on_node_with_text("eski").assert_does_not_exist();
    rule.on_node_with_tag("temiz").assert_text_equals(&["yerine"]);
    // Parents.
    let inner = rule.on_node_with_tag("iç").fetch_semantics_node();
    assert_eq!(inner.parent().map(|p| p.id()), Some(button.id()));
    assert!(rule.on_root().fetch_semantics_node().is_root());
    rule.on_node_with_tag("iç").assert(has_parent(has_test_tag("düğme")));
    rule.on_node_with_tag("düğme").assert(has_any_descendant(has_text("Ayrı", false, false)));
}

fn listeners_are_told_of_changes_after_the_frame() {
    struct Recorder(RefCell<Vec<String>>);
    impl SemanticsListener for Recorder {
        fn on_semantics_changed(&self, _node: &Rc<LayoutNode>, previous: Option<&SemanticsConfiguration>) {
            let before = previous
                .and_then(|p| p.get(SemanticsProperties::Text))
                .map(|t| t.iter().map(|t| t.text()).collect::<Vec<_>>().join(","));
            self.0.borrow_mut().push(format!("önce {}", before.unwrap_or_else(|| "-".into())));
        }
    }
    let label = mutable_state_of("bir".to_string());
    let l = label.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Box(Modifier.size(10.dp()).semantics(move |c| c.set_text(l.get())));
    }));
    let recorder = Rc::new(Recorder(RefCell::new(Vec::new())));
    rule.host().semantics_owner().add_listener(recorder.clone());
    rule.host().accessibility_tree(); // (what a bridge reads: the semantics worked out, their reads observed)
    label.set("iki".into());
    rule.wait_for_idle();
    assert_eq!(*recorder.0.borrow(), ["önce bir"]);
    assert!(rule.host().take_semantics_changed());
    let l: Rc<dyn SemanticsListener> = recorder.clone();
    rule.host().semantics_owner().remove_listener(&l);
    rule.host().accessibility_tree();
    label.set("üç".into());
    rule.wait_for_idle();
    assert_eq!(recorder.0.borrow().len(), 1);
}

fn reading_order_groups_rows_and_follows_traversal_groups() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Row(content = || {
                    Box(Modifier
                        .offset_with(60.dp(), 0.dp())
                        .size_wh(40.dp(), 20.dp())
                        .semantics(|c| c.set_content_description("sağ üst")));
                    Box(Modifier
                        .offset_with((-40).dp(), 0.dp())
                        .size_wh(40.dp(), 20.dp())
                        .semantics(|c| c.set_content_description("sol üst")));
                });
                // A traversal group: what is inside it is read together, after it.
                Row(
                    Modifier.semantics(|c| c.set_is_traversal_group(true)),
                    content = || {
                        Column(
                            Modifier.semantics(|c| c.set_is_traversal_group(true)),
                            content = || {
                                Text("A1");
                                Text("A2");
                            },
                        );
                        Column(
                            content = || {
                                Text("B1");
                                Text("B2");
                            },
                        );
                    },
                );
            },
        );
    }));
    let root = rule.on_root().fetch_semantics_node();
    let sorted = root.subtree_sorted_by_geometry_grouping(
        &|n| !n.is_hidden(),
        &|n| {
            n.unmerged_config().contains(SemanticsProperties::ContentDescription)
                || n.unmerged_config().contains(SemanticsProperties::Text)
        },
        root.children(),
    );
    let labels: Vec<String> = sorted
        .iter()
        .map(|n| {
            let c = n.config();
            c.get(SemanticsProperties::ContentDescription)
                .or(c.get(SemanticsProperties::Text).map(|t| t.iter().map(|t| t.text().to_string()).collect()))
                .map(|v| v.join(""))
                .unwrap_or_default()
        })
        .filter(|s| !s.is_empty())
        .collect();
    // Left to right in the first row (by where they are drawn), then the group's A column before B's.
    assert_eq!(labels, ["sol üst", "sağ üst", "A1", "A2", "B1", "B2"]);
}

fn scroll_actions_page_by_the_viewport() {
    let scrolled = Rc::new(RefCell::new(Vec::new()));
    let s = scrolled.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(Modifier
            .size_wh(100.dp(), 300.dp())
            .semantics(move |c| {
                let s = s.clone();
                c.set_vertical_scroll_axis_range(ScrollAxisRange::new(|| 250.0, || 1000.0, false));
                c.scroll_by(move |x, y| {
                    s.borrow_mut().push((x, y));
                    true
                });
            })
            .test_tag("liste"));
    }));
    let id = rule.on_node_with_tag("liste").fetch_semantics_node().id();
    let tree = rule.host().accessibility_tree();
    let list = tree.iter().find(|n| n.id == id).unwrap();
    assert!(list.scrollable && list.horizontal_scroll.is_none());
    // (Where it is as the tree was made: what UI Automation's scroll percentages are worked out of.)
    assert_eq!(
        list.vertical_scroll,
        Some(ScrollPosition { value: 250.0, max_value: 1000.0, reverse_scrolling: false })
    );
    assert!(rule.host().perform_semantics_action(id, SemanticsAction::ScrollForward));
    assert!(rule.host().perform_semantics_action(id, SemanticsAction::ScrollBackward));
    assert!(rule.host().perform_semantics_action(id, SemanticsAction::ScrollBy { x: 0.0, y: 125.0 }));
    assert_eq!(*scrolled.borrow(), [(0.0, 300.0), (0.0, -300.0), (0.0, 125.0)]);
    rule.on_node_with_tag("liste").assert(has_scroll_action());
}

fn disabled_nodes_offer_and_take_no_actions_but_selection_and_copy() {
    // Compose's accessibility delegate: a disabled node keeps its actions in its configuration, but accessibility
    // isn't offered set-progress on it and can't perform anything on it but setting a selection and copying.
    let done = Rc::new(RefCell::new(Vec::<String>::new()));
    let enabled = mutable_state_of(false);
    let (d, e) = (done.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (d, on) = (d.clone(), e.get());
        Box(Modifier.size(10.dp()).test_tag("kapalı").semantics(move |c| {
            if !on {
                c.disabled();
            }
            c.set_progress_bar_range_info(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0));
            let d1 = d.clone();
            c.set_progress(None, move |v| {
                d1.borrow_mut().push(format!("ilerleme {v}"));
                true
            });
            let d2 = d.clone();
            c.on_click(None, move || {
                d2.borrow_mut().push("tık".to_string());
                true
            });
            let d3 = d.clone();
            c.copy_text(None, move || {
                d3.borrow_mut().push("kopya".to_string());
                true
            });
        }));
    }));
    let node =
        || rule.host().accessibility_tree().into_iter().find(|n| n.test_tag.as_deref() == Some("kapalı")).unwrap();
    let n = node();
    assert!(!n.enabled && !n.settable && n.range.is_some() && n.clickable);
    // (still in its configuration)
    assert!(rule.on_node_with_tag("kapalı").fetch_semantics_node().config().contains(SemanticsActions::SetProgress));
    assert!(!rule.host().perform_semantics_action(n.id, SemanticsAction::SetProgress(0.2)));
    assert!(!rule.host().perform_semantics_action(n.id, SemanticsAction::Click));
    assert!(rule.host().perform_semantics_action(n.id, SemanticsAction::CopyText));
    assert_eq!(*done.borrow(), ["kopya"]);
    enabled.set(true);
    rule.wait_for_idle();
    let n = node();
    assert!(n.enabled && n.settable);
    assert!(rule.host().perform_semantics_action(n.id, SemanticsAction::SetProgress(0.2)));
    assert!(rule.host().perform_semantics_action(n.id, SemanticsAction::Click));
    assert_eq!(*done.borrow(), ["kopya", "ilerleme 0.2", "tık"]);
}

tests!(
    semantics_set_on_a_receiver,
    semantics_merge_clear_and_hide,
    selectable_groups_are_told_to_accessibility,
    semantics_ranges_live_regions_custom_actions_and_order,
    semantics_tree_merges_buttons_and_performs_actions,
    semantics_tree_has_the_keyboard_focus,
    configurations_set_merge_and_combine_as_compose,
    trees_merge_clear_and_add_fake_nodes,
    listeners_are_told_of_changes_after_the_frame,
    reading_order_groups_rows_and_follows_traversal_groups,
    scroll_actions_page_by_the_viewport,
    disabled_nodes_offer_and_take_no_actions_but_selection_and_copy,
);
