//! BasicTooltipBox, as Compose's: a long press, the mouse, accessibility's long click or code shows the tooltip in
//! a popup where its position provider says, at once; one at a time; persistent or for a while; nothing from the
//! user when its input is off.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::PointerType;
use kompoz_ui::semantics::SemanticsActions;
use kompoz_ui::window::PopupPositionProvider;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Right of the anchor, its top lined up with the anchor's.
struct RightOf;

impl PopupPositionProvider for RightOf {
    fn calculate_position(&self, anchor: IntRect, _: IntSize, _: LayoutDirection, _: IntSize) -> IntOffset {
        IntOffset::new(anchor.right + 10, anchor.top)
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// (the anchor merges what is in it: its tags are in the unmerged tree)
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn long_press(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

#[kompozable]
fn Tagged(tag: String, width: i32, height: i32, color: Color) {
    Box(Modifier.size_wh(width.dp(), height.dp()).background(color).test_tag(tag));
}

fn basic_tooltip_box_shows_at_its_position_at_once() {
    // C++'s basic_tooltip_box_shows_at_its_position_at_once.
    let states: Rc<RefCell<Vec<Rc<dyn BasicTooltipState>>>> = Rc::new(RefCell::new(Vec::new()));
    let s = states.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let a = remember_basic_tooltip_state();
            let b = remember_basic_tooltip_state();
            let c = remember_basic_tooltip_state();
            *s.borrow_mut() = vec![a.clone(), b.clone(), c.clone()];
            let right: Rc<dyn PopupPositionProvider> = Rc::new(RightOf);
            Column(
                modifier = Modifier.padding_sides(top = 100.dp()),
                content = move || {
                    BasicTooltipBox(
                        position_provider = right.clone(),
                        tooltip = || Tagged("ipucu1".into(), 50, 20, Color::Transparent),
                        state = a.clone(),
                        content = || Tagged("çapa1".into(), 40, 40, Color::Transparent),
                    );
                    BasicTooltipBox(
                        position_provider = right.clone(),
                        tooltip = || Tagged("ipucu2".into(), 50, 20, Color::Black),
                        state = b.clone(),
                        content = || Tagged("çapa2".into(), 40, 40, Color::Transparent),
                    );
                    BasicTooltipBox(
                        position_provider = right.clone(),
                        tooltip = || Tagged("ipucu3".into(), 50, 20, Color::Transparent),
                        state = c.clone(),
                        focusable = true,
                        enable_user_input = false,
                        content = || Tagged("çapa3".into(), 40, 40, Color::Transparent),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let state = |i: usize| states.borrow()[i].clone();
    // A long press shows it where told, at once (no growing, no fading); persistent by default.
    let anchor = bounds(&rule, "çapa1");
    long_press(&rule, anchor.center());
    assert!(state(0).is_visible() && state(0).is_persistent());
    let tip = bounds(&rule, "ipucu1");
    assert!(near(tip.left, anchor.right + 10.0) && near(tip.top, anchor.top), "{tip:?} {anchor:?}");
    let canvas = rule.draw();
    assert!(canvas.layer_alphas.iter().all(|a| *a >= 0.999));
    rule.advance_time_by(3000);
    assert!(state(0).is_visible());
    // One shows at a time: showing another dismisses it. It shows whole on its first frame. (Compose's mutex: the
    // long press showed the first with UserInput's priority, which a Default `show()` can't interrupt — it is
    // cancelled instead, the first staying; C++'s `show()` took over: here it is shown with UserInput's.)
    let scope = CoroutineScope::new(rule.host().dispatcher());
    scope.launch(state(1).show(MutatePriority::Default));
    rule.advance_frame();
    assert!(state(0).is_visible() && !state(1).is_visible());
    scope.launch(state(1).show(MutatePriority::UserInput));
    rule.advance_frame();
    let drawn = rule.draw();
    let second = bounds(&rule, "ipucu2");
    let whole = drawn.rects.iter().zip(&drawn.rect_colors).any(|(r, c)| *r == second && *c == Color::Black);
    assert!(whole && drawn.layer_alphas.iter().all(|a| *a >= 0.999));
    assert!(drawn.layer_transforms.iter().all(|t| t.scale_x >= 0.999));
    rule.wait_for_idle();
    assert!(state(1).is_visible() && !state(0).is_visible());
    rule.on_node_with_tag("ipucu1").assert_does_not_exist();
    state(1).dismiss();
    rule.wait_for_idle();
    rule.on_node_with_tag("ipucu2").assert_does_not_exist();
    // Without user input a long press does nothing.
    let still = bounds(&rule, "çapa3");
    long_press(&rule, still.center());
    rule.wait_for_idle();
    assert!(!state(2).is_visible());
}

fn a_tooltip_that_isnt_persistent_goes_after_a_while() {
    // Compose's BasicTooltipDefaults.TooltipDuration (1500 ms) for a tooltip that isn't persistent; the mouse over the
    // anchor shows it, leaving dismisses it; accessibility's long click shows it, labelled "show tooltip".
    let holder: Rc<RefCell<Option<Rc<dyn BasicTooltipState>>>> = Rc::new(RefCell::new(None));
    let h = holder.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_basic_tooltip_state(is_persistent = false);
        *h.borrow_mut() = Some(state.clone());
        BasicTooltipBox(
            position_provider = Rc::new(RightOf) as Rc<dyn PopupPositionProvider>,
            tooltip = || Tagged("ipucu".into(), 50, 20, Color::Transparent),
            state = state,
            modifier = Modifier.test_tag("çapa"),
            content = || Tagged("içerik".into(), 40, 40, Color::Transparent),
        );
    }));
    let state = || holder.borrow().clone().unwrap();
    // The mouse over it shows it; for a while (it isn't persistent).
    rule.on_node_with_tag("çapa").perform_mouse_input(|m| m.move_to(Offset::new(20.0, 20.0)));
    rule.advance_frame();
    assert!(state().is_visible() && !state().is_persistent());
    rule.on_node_with_tag("ipucu").assert_exists();
    rule.advance_time_by(1600);
    assert!(!state().is_visible());
    // The long click's label, and what it does.
    let long_click = rule.on_node_with_tag("çapa").fetch_semantics_node().config();
    let long_click = long_click.get(SemanticsActions::OnLongClick).expect("a long click");
    assert_eq!(long_click.label.as_deref(), Some("show tooltip"));
    // (called as accessibility calls it: `perform_semantics_action` waits for idle, the 1.5 s with it)
    (long_click.action.expect("its action"))();
    rule.advance_frame();
    assert!(state().is_visible());
    // The mouse leaving dismisses it.
    rule.on_node_with_tag("çapa").perform_mouse_input(|m| m.move_to(Offset::new(200.0, 300.0)));
    rule.advance_frame();
    assert!(!state().is_visible());
}

tests![basic_tooltip_box_shows_at_its_position_at_once, a_tooltip_that_isnt_persistent_goes_after_a_while];
