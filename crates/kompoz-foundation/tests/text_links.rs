//! BasicText's links and its bridge to a selection: a node over each link opening its URL or telling its listener
//! by touch, mouse, keyboard and accessibility, its styles as it is focused, hovered or pressed, a hand over it;
//! a text in a selection container working with its selection controller (a fake registrar here).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::selection::{
    AnchorInfo, LocalSelectionRegistrar, LocalTextSelectionColors, Selectable, Selection, SelectionAdjustment,
    SelectionRegistrar, SelectionRegistrarRef, Subselections, TextSelectionColors,
};
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::*;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::platform::{PlatformCallbacks, PointerIcon};
use kompoz_ui::semantics::{SemanticsAction, SemanticsNode, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
const YELLOW: Color = Color::new(1.0, 1.0, 0.0, 1.0);

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn sized(px: f32) -> TextStyle {
    TextStyle { font_size: px.sp(), ..TextStyle::Default }
}

/// A platform writing down the URIs it is asked to open and the cursors it is told.
#[derive(Default)]
struct Platform {
    opened: RefCell<Vec<String>>,
    cursors: RefCell<Vec<PointerIcon>>,
}

impl PlatformCallbacks for Platform {
    fn open_uri(&self, uri: &str) {
        self.opened.borrow_mut().push(uri.to_string());
    }
    fn cursor(&self, icon: PointerIcon) {
        self.cursors.borrow_mut().push(icon);
    }
}

/// The color the text `text` is drawn in now.
fn color_of(rule: &KompozTestRule, text: &str) -> Color {
    let c = rule.draw();
    c.colors[c.texts.iter().position(|t| t == text).unwrap_or_else(|| panic!("no {text:?} among {:?}", c.texts))]
}

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

/// A key pressed and let go.
fn key(rule: &KompozTestRule, k: Key, modifiers: KeyModifiers) {
    rule.host().key_down(k, modifiers);
    rule.host().key_up(k, modifiers);
    rule.wait_for_idle();
}

/// The links' nodes (Compose's LinkTestMarker), in the tree's order.
fn link_nodes(rule: &KompozTestRule) -> Vec<SemanticsNode> {
    rule.on_all_nodes(SemanticsMatcher::key_is_defined(SemanticsProperties::LinkTestMarker)).fetch_semantics_nodes()
}

/// Tags told to listeners.
type Tags = Arc<Mutex<Vec<String>>>;

fn tag_listener(tags: &Tags) -> LinkInteractionListener {
    let tags = tags.clone();
    LinkInteractionListener::new(move |l: &LinkAnnotation| {
        tags.lock().unwrap().push(l.as_tag().unwrap_or("").to_string())
    })
}

fn link_annotations_open_urls_call_listeners_and_restyle_on_hover_and_press() {
    // C++ 29946 at 10 sp (5 px a character, lines 12.5): "aa " is 15 px, "site" 20. A link without styles has no
    // look of its own (Compose's; C++ underlined it in the primary color): "tık" is drawn with the text before it.
    let tags: Tags = Default::default();
    let listener = tag_listener(&tags);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let text = build_annotated_string(|b| {
            b.append("aa ");
            let styles = TextLinkStyles {
                style: Some(SpanStyle { color: RED, ..SpanStyle::Default }),
                hovered_style: Some(SpanStyle { color: GREEN, ..SpanStyle::Default }),
                pressed_style: Some(SpanStyle { color: BLUE, ..SpanStyle::Default }),
                ..TextLinkStyles::default()
            };
            b.with_link(LinkAnnotation::url_with("https://kompoz.dev", Some(styles), None), |b| b.append("site"));
            b.append(" bb ");
            b.with_link(LinkAnnotation::clickable_with("etiket", None, Some(listener.clone())), |b| b.append("tık"));
        });
        BasicText(text, style = sized(10.0));
    }));
    let platform = Rc::new(Platform::default());
    rule.host().set_callbacks(platform.clone());
    assert_eq!(color_of(&rule, "site"), RED); // its style
    assert_eq!(color_of(&rule, " bb tık"), Color::Black);
    // The mouse over it, then pressed on it.
    rule.host().pointer_move(at(25.0, 5.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(color_of(&rule, "site"), GREEN);
    assert_eq!(platform.cursors.borrow().last(), Some(&PointerIcon::Hand));
    rule.host().pointer_down(at(25.0, 5.0), -1, 0, PointerType::Mouse);
    rule.advance_frame();
    rule.wait_for_idle();
    assert_eq!(color_of(&rule, "site"), BLUE);
    rule.host().pointer_up(at(25.0, 5.0), -1, 0);
    rule.advance_time_by(400);
    assert_eq!(*platform.opened.borrow(), ["https://kompoz.dev"]);
    assert_eq!(color_of(&rule, "site"), GREEN);
    rule.host().pointer_move(at(5.0, 5.0), -1, 0); // off the link
    rule.wait_for_idle();
    assert_eq!(color_of(&rule, "site"), RED);
    assert_eq!(platform.cursors.borrow().last(), Some(&PointerIcon::Default));
    rule.host().pointer_move(at(25.0, 5.0), -1, 0);
    rule.host().pointer_move(at(300.0, 300.0), -1, 0); // over the link, then out of the text altogether
    rule.wait_for_idle();
    assert_eq!(color_of(&rule, "site"), RED);
    tap(&rule, at(60.0, 5.0)); // "aa site bb " is 55 px: "tık" after it
    rule.advance_time_by(400);
    assert_eq!(*tags.lock().unwrap(), ["etiket"]);
    assert_eq!(platform.opened.borrow().len(), 1);
}

fn links_take_the_keyboard_focus_and_accessibility_clicks() {
    // C++ 30001: each link a node of its own over it, clickable (Compose's link nodes: a click action and the link
    // test marker; C++ described them with their text).
    let tags: Tags = Default::default();
    let (l1, l2) = (tag_listener(&tags), tag_listener(&tags));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2) = (l1.clone(), l2.clone());
        Column(move || {
            let focused = Some(SpanStyle { color: YELLOW, ..SpanStyle::Default });
            let text = build_annotated_string(|b| {
                b.append("aa ");
                let styles = TextLinkStyles {
                    style: Some(SpanStyle { color: RED, ..SpanStyle::Default }),
                    focused_style: focused.clone(),
                    ..TextLinkStyles::default()
                };
                b.with_link(LinkAnnotation::url_with("https://kompoz.dev", Some(styles), None), |b| b.append("site"));
                b.append(" bb ");
                let styles = TextLinkStyles { focused_style: focused.clone(), ..TextLinkStyles::default() };
                b.with_link(LinkAnnotation::clickable_with("etiket", Some(styles), Some(l1.clone())), |b| {
                    b.append("tık")
                });
            });
            BasicText(text, style = sized(10.0));
            // A link over two lines (30 px wide).
            let long = build_annotated_string(|b| {
                b.with_link(LinkAnnotation::clickable_with("uzun", None, Some(l2.clone())), |b| b.append("abc def"));
            });
            BasicText(long, modifier = Modifier.width(30.dp()).test_tag("uzun"), style = sized(10.0));
        });
    }));
    let platform = Rc::new(Platform::default());
    rule.host().set_callbacks(platform.clone());
    let links = link_nodes(&rule);
    assert_eq!(links.len(), 3);
    let site = links[0].bounds_in_root();
    assert!(near(site.left, 15.0) && near(site.width(), 20.0), "{site:?}");
    assert!(links.iter().all(|n| n.config().contains(kompoz_ui::semantics::SemanticsActions::OnClick)));
    // The long link's node over both its lines.
    let long = rule.on_node_with_tag("uzun").get_unclipped_bounds_in_root();
    let long_link = links[2].bounds_in_root();
    assert!(near(long_link.top, long.top) && long_link.height() >= 25.0, "{long_link:?} {long:?}");
    // Tab: the first link, its focused style; Enter opens it.
    key(&rule, Key::Tab, KeyModifiers::None);
    assert_eq!((color_of(&rule, "site"), color_of(&rule, " bb ")), (YELLOW, Color::Black));
    key(&rule, Key::Enter, KeyModifiers::None);
    assert_eq!(*platform.opened.borrow(), ["https://kompoz.dev"]);
    // Tab on: the next one; Shift+Tab back.
    key(&rule, Key::Tab, KeyModifiers::None);
    assert_eq!((color_of(&rule, "site"), color_of(&rule, "tık")), (RED, YELLOW));
    key(&rule, Key::Enter, KeyModifiers::None);
    assert_eq!(*tags.lock().unwrap(), ["etiket"]);
    key(&rule, Key::Tab, KeyModifiers::Shift);
    assert_eq!(color_of(&rule, "site"), YELLOW);
    // An accessibility service's click opens it too.
    assert!(rule.host().perform_semantics_action(links[1].id(), SemanticsAction::Click));
    rule.wait_for_idle();
    assert_eq!(*tags.lock().unwrap(), ["etiket", "etiket"]);
    // The long link: a tap on either line, once each.
    tap(&rule, at(long.left + 5.0, long.top + 5.0));
    tap(&rule, at(long.left + 5.0, long.top + 17.0));
    rule.advance_time_by(400);
    assert_eq!(*tags.lock().unwrap(), ["etiket", "etiket", "uzun", "uzun"]);
    assert_eq!(platform.opened.borrow().len(), 1);
}

fn a_hand_is_over_a_link_and_only_there() {
    // C++ 26856's link part: a link wrapped onto a second line (8 px a character, 20 px lines, 80 px wide): a hand
    // over its second line, none over the text before it on its first line (its node clipped to its shape).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let text = build_annotated_string(|b| {
            b.append("ab ");
            b.with_link(LinkAnnotation::clickable_with("bağ", None, None), |b| b.append("cdefgh ijklm"));
        });
        BasicText(text, modifier = Modifier.width(80.dp()).test_tag("bağ"), style = sized(16.0));
    }));
    let platform = Rc::new(Platform::default());
    rule.host().set_callbacks(platform.clone());
    let b = rule.on_node_with_tag("bağ").get_unclipped_bounds_in_root();
    let over = |x: f32, y: f32| {
        rule.host().pointer_move(at(b.left + x, b.top + y), -1, 0);
        rule.wait_for_idle();
        platform.cursors.borrow().last().copied().unwrap_or_default()
    };
    assert_eq!(over(12.0, 30.0), PointerIcon::Hand);
    assert_eq!(over(4.0, 10.0), PointerIcon::Default);
    assert_eq!(over(40.0, 10.0), PointerIcon::Hand);
}

fn links_follow_the_text_when_it_changes() {
    // Links' nodes are made for the links the text has now, and placed over where they are now.
    let second = mutable_state_of(false);
    let s = second.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let text = build_annotated_string(|b| {
            if s.get() {
                b.append("önce ");
            }
            b.with_link(LinkAnnotation::url("https://a.dev"), |b| b.append("bir"));
            if s.get() {
                b.append(" ");
                b.with_link(LinkAnnotation::url("https://b.dev"), |b| b.append("iki"));
            }
        });
        BasicText(text, style = sized(10.0));
    }));
    let links = link_nodes(&rule);
    assert!(links.len() == 1 && near(links[0].bounds_in_root().left, 0.0));
    second.set(true);
    rule.wait_for_idle();
    let links = link_nodes(&rule);
    assert_eq!(links.len(), 2);
    assert!(near(links[0].bounds_in_root().left, 25.0) && near(links[1].bounds_in_root().left, 45.0));
}

// ---- The selection's bridge ------------------------------------------------------------------------------------

/// A selection standing in for a selection container's registrar: it writes down what its texts' controllers
/// tell it, and gives the parts of the selection it is set up with.
struct FakeSelection {
    log: Rc<RefCell<Vec<String>>>,
    next: Cell<u64>,
    selected: RefCell<HashMap<u64, Selection>>,
}

impl FakeSelection {
    fn say(&self, line: String) {
        self.log.borrow_mut().push(line);
    }
}

impl SelectionRegistrar for FakeSelection {
    fn subselections(&self) -> Subselections {
        Arc::new(self.selected.borrow().clone())
    }
    fn subscribe(&self, selectable: Rc<dyn Selectable>) -> Rc<dyn Selectable> {
        let text = selectable.get_text();
        self.say(format!("subscribed {} {}", selectable.selectable_id(), text.text()));
        selectable
    }
    fn unsubscribe(&self, selectable: &dyn Selectable) {
        self.say(format!("unsubscribed {}", selectable.selectable_id()));
    }
    fn next_selectable_id(&self) -> u64 {
        self.next.set(self.next.get() + 1);
        self.next.get()
    }
    fn notify_position_change(&self, selectable_id: u64) {
        self.say(format!("moved {selectable_id}"));
    }
    fn notify_selection_update_start(&self, _: &LayoutCoordinates, at: Offset, _: SelectionAdjustment, touch: bool) {
        self.say(format!("start {} {} {touch}", at.x, at.y));
    }
    fn notify_selection_update_select_all(&self, selectable_id: u64, _: bool) {
        self.say(format!("select all {selectable_id}"));
    }
    fn notify_selection_update(
        &self,
        _: &LayoutCoordinates,
        _: Offset,
        _: Offset,
        _: bool,
        _: SelectionAdjustment,
        _: bool,
    ) -> bool {
        true
    }
    fn notify_selection_update_end(&self) {
        self.say("end".to_string());
    }
    fn notify_selectable_change(&self, selectable_id: u64) {
        self.say(format!("changed {selectable_id}"));
    }
}

fn a_text_in_a_selection_works_with_its_controller() {
    // Where a selection registrar is provided, BasicText takes an id from it and makes its selection controller
    // (with the selection colors' background): the controller's modifier on its layout (the selection gestures —
    // a mouse's press starts a selection — and the text cursor), a selectable text node that tells it where the
    // text is placed, laid out and moved (its selectable subscribed, its text changing told), and its part of the
    // selection drawn under the text.
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let fake = Rc::new(FakeSelection { log: log.clone(), next: Cell::new(0), selected: RefCell::new(HashMap::new()) });
    // The second text's whole text selected.
    let anchor = |offset| AnchorInfo::new(ResolvedTextDirection::Ltr, offset, 2);
    fake.selected.borrow_mut().insert(2, Selection::new(anchor(0), anchor("seçilir".len()), false));
    let registrar = SelectionRegistrarRef(fake.clone());
    let (shown, text) = (mutable_state_of(true), mutable_state_of("seçilir".to_string()));
    let (s, t) = (shown.clone(), text.clone());
    let colors = TextSelectionColors { handle_color: BLUE, background_color: YELLOW };
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, t) = (s.clone(), t.clone());
        KompozitionLocalProvider(
            &[LocalSelectionRegistrar.provides(Some(registrar.clone())), LocalTextSelectionColors.provides(colors)],
            move || {
                Column(move || {
                    BasicText("üst", style = sized(16.0));
                    if s.get() {
                        BasicText(t.get(), modifier = Modifier.test_tag("metin"), style = sized(16.0));
                    }
                });
            },
        );
    }));
    let platform = Rc::new(Platform::default());
    rule.host().set_callbacks(platform.clone());
    let taken = || std::mem::take(&mut *log.borrow_mut());
    let first = taken();
    for expected in ["subscribed 1 üst", "subscribed 2 seçilir", "moved 1", "moved 2"] {
        assert!(first.iter().any(|l| l == expected), "{expected:?} not in {first:?}");
    }
    // The selection drawn under the text, in the selection colors' background: the second text's whole line.
    let c = rule.draw();
    let text_bounds = rule.on_node_with_tag("metin").get_unclipped_bounds_in_root();
    let under: Vec<Rect> =
        c.path_colors.iter().zip(&c.paths).filter(|(col, _)| **col == YELLOW).map(|(_, r)| *r).collect();
    assert_eq!(under.len(), 1, "{under:?}");
    assert!(near(under[0].top, text_bounds.top) && near(under[0].height(), 20.0) && near(under[0].width(), 7.0 * 8.0));
    // Its gestures: a mouse's press starts a selection at it; the text cursor over it.
    rule.host().pointer_move(text_bounds.center(), -1, 0);
    rule.wait_for_idle();
    assert_eq!(platform.cursors.borrow().last(), Some(&PointerIcon::Text));
    rule.host().pointer_down(at(text_bounds.left + 10.0, text_bounds.top + 5.0), -1, 0, PointerType::Mouse);
    rule.host().pointer_up(at(text_bounds.left + 10.0, text_bounds.top + 5.0), -1, 0);
    rule.wait_for_idle();
    let pressed = taken();
    assert!(pressed.iter().any(|l| l == "start 10 5 false") && pressed.iter().any(|l| l == "end"), "{pressed:?}");
    // Another text: told.
    text.set("başka".to_string());
    rule.wait_for_idle();
    let changed = taken();
    assert!(changed.iter().any(|l| l == "changed 2"), "{changed:?}");
    // Gone: unsubscribed.
    shown.set(false);
    rule.wait_for_idle();
    let gone = taken();
    assert!(gone.iter().any(|l| l == "unsubscribed 2"), "{gone:?}");
}

tests!(
    link_annotations_open_urls_call_listeners_and_restyle_on_hover_and_press,
    links_take_the_keyboard_focus_and_accessibility_clicks,
    a_hand_is_over_a_link_and_only_there,
    links_follow_the_text_when_it_changes,
    a_text_in_a_selection_works_with_its_controller,
);
