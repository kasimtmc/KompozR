//! The display list: damage as what changed on the screen, in separate areas; moves of what scrolled by whole
//! pixels; layers known from frame to frame; kept nodes; replay of an area; what an opaque rect hides; transforms,
//! clips, effects, backdrops, shadows and bitmaps; what the drawing package draws recorded and replayed the same;
//! the host's keep check. Scrolling is a placement-time offset here (a layout modifier reading a state inside a
//! clip), until foundation's scroll comes.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_foundation::{BackgroundModifiers, BitmapImage};
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::display_list::DisplayList;
use kompoz_ui::draw::{BlurType, BlurredEdgeTreatment, DrawScope, DrawStyle, Fill, Paint, PointMode, Stroke};
use kompoz_ui::graphics_context::{
    ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerContent, LayerTransform, NodeRecording, Shadow,
    ShadowLight, TextDraw, shadow_path_reach,
};
use kompoz_ui::layout::CompositingStrategy;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

fn same_rect(a: Rect, b: Rect) -> bool {
    (a.left - b.left).abs() < 1e-3
        && (a.top - b.top).abs() < 1e-3
        && (a.right - b.right).abs() < 1e-3
        && (a.bottom - b.bottom).abs() < 1e-3
}

fn covers(outer: Rect, inner: Rect) -> bool {
    outer.left <= inner.left && outer.top <= inner.top && outer.right >= inner.right && outer.bottom >= inner.bottom
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn sz(w: f32, h: f32) -> Size {
    Size::new(w, h)
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const SRC: BlendMode = BlendMode::SrcOver;

fn hgrad(colors: &[Color]) -> Brush {
    Brush::horizontal_gradient(colors, 0.0, f32::INFINITY, TileMode::Clamp)
}

fn vgrad(colors: &[Color]) -> Brush {
    Brush::vertical_gradient(colors, 0.0, f32::INFINITY, TileMode::Clamp)
}

fn stroke(width: f32) -> DrawStyle {
    DrawStyle::Stroke(Stroke::new(width, 4.0, StrokeCap::Butt, StrokeJoin::Miter, None))
}

/// A one-line text shaped by kompoz-text's FakeBackend (C++'s FakeTextMeasurer's numbers) at `baseline`, `size`
/// px.
fn text(s: &str, baseline: Offset, size: f32) -> TextDraw {
    kompoz_text::FakeBackend.text_draw(s, baseline, Rc::new(kompoz_text::TextRunStyle::new(size)), false)
}

/// A path's commands as text: "M0,0 L10,0 …", each end to a hundredth.
fn path_text(path: &Path) -> String {
    let num = |v: f32| format!("{}", (v * 100.0).round() / 100.0);
    let mut out = String::new();
    for c in path.commands() {
        if !out.is_empty() {
            out.push(' ');
        }
        let (letter, p) = match *c {
            PathCommand::Move(p) => ("M", p),
            PathCommand::Line(p) => ("L", p),
            PathCommand::Quadratic(_, p) => ("Q", p),
            PathCommand::Conic(_, p, _) => ("K", p),
            PathCommand::Cubic(_, _, p) => ("C", p),
            PathCommand::Close => {
                out.push('Z');
                continue;
            }
        };
        out += &format!("{letter}{},{}", num(p.x), num(p.y));
    }
    out
}

fn recording(layer_answer: LayerContent) -> RecordingCanvas {
    let mut canvas = RecordingCanvas::new();
    canvas.layer_answer = layer_answer;
    canvas
}

/// The host's frame recorded into a display list of `viewport`, then replayed into a recording canvas.
fn replayed(rule: &KompozTestRule, viewport: Size) -> RecordingCanvas {
    let mut list = DisplayList::new();
    list.begin(viewport);
    rule.host().draw(&mut list);
    list.end();
    let mut canvas = RecordingCanvas::new();
    list.replay(&mut canvas, None);
    canvas
}

/// A vertical scroll of the tests' own (verticalScroll's): what it wraps measured as tall as it is, cut to the
/// scroller, moved up by `scroll` px (as far as it can go), placed again as it changes.
fn scrolled(m: Modifier, scroll: &MutableState<i32>) -> Modifier {
    let scroll = scroll.clone();
    m.clip_to_bounds().layout(move |scope, measurable, c| {
        let p = measurable.measure(Constraints::new(c.min_width, c.max_width, 0, Constraints::Infinity));
        let (width, height) = (c.constrain_width(p.width()), c.constrain_height(p.height()));
        let scroll = scroll.clone();
        scope.layout(width, height, move |pl| {
            let by = scroll.get().clamp(0, (p.height() - height).max(0));
            pl.place(&p, 0, -by)
        })
    })
}

/// What it wraps, moved by (`x`, `y`) px.
fn moved(m: Modifier, x: i32, y: i32) -> Modifier {
    m.layout(move |scope, measurable, c| {
        let p = measurable.measure(c);
        scope.layout(p.width(), p.height(), move |pl| pl.place(&p, x, y))
    })
}

// ---- Damage ----------------------------------------------------------------------------------------------------

fn display_list_damage_is_what_changed_on_the_screen() {
    // C++'s display_list_damage_is_what_changed_on_the_screen, at density 2 (C++ recorded dp with a pixel of half a
    // dp): its numbers in px.
    let blink = mutable_state_of(false);
    let spin = mutable_state_of(0.0f32);
    let more = mutable_state_of(false);
    let scroll = mutable_state_of(0);
    let (b, sp, mo, sc) = (blink.clone(), spin.clone(), more.clone(), scroll.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (b, sp, mo, sc) = (b.clone(), sp.clone(), mo.clone(), sc.clone());
            Column(Modifier.fill_max_size(), move || {
                Box(Modifier
                    .size_wh(100.dp(), 50.dp())
                    .background_with(if b.get() { RED } else { BLUE }, RectangleShape));
                let degrees = sp.get();
                // (clips what scrolled out)
                Column(scrolled(Modifier.height(200.dp()), &sc), move || {
                    Box(Modifier.size_wh(100.dp(), 300.dp()));
                    // At 300: out of view.
                    Box(Modifier.size(40.dp()).rotate(degrees).background_with(GREEN, RectangleShape));
                });
                Text("sabit");
                if mo.get() {
                    Text("yeni satır");
                }
            });
        }),
        IntSize::new(800, 1600),
    );
    rule.host().set_density(Density { density: 2.0, font_scale: 1.0 });
    rule.wait_for_idle();
    let mut list = DisplayList::new();
    let mut frame = || {
        list.begin(Size::new(800.0, 1600.0));
        rule.host().draw(&mut list);
        list.end()
    };
    let d = frame().unwrap();
    assert!(d.width() == 800.0 && d.height() == 1600.0); // the first: all of it
    assert!(frame().is_none()); // the same again: nothing to draw
    blink.set(true);
    rule.wait_for_idle();
    let d = frame().unwrap();
    assert!(d.left == 0.0 && d.top == 0.0 && d.right == 202.0 && d.bottom == 102.0, "{d:?}"); // the box, and two px
    spin.set(45.0);
    rule.wait_for_idle();
    assert!(frame().is_none()); // turning out of view changes no pixel
    let text_top = rule.on_node_with_text("sabit").get_bounds_in_root().bottom;
    more.set(true);
    rule.wait_for_idle();
    let d = frame().unwrap();
    // The new line only, not what was above.
    assert!(d.top >= text_top - 16.0 && d.top > 400.0 && d.bottom < 800.0, "{d:?}");
    let added = d;
    more.set(false);
    rule.wait_for_idle();
    let d = frame().unwrap();
    assert!(d.top == added.top && d.bottom == added.bottom); // where it was
    scroll.set(300); // (as far as it goes: 280)
    rule.wait_for_idle();
    let d = frame().unwrap();
    // Only the box that came into view.
    assert!(d.left == 0.0 && d.top >= 398.0 && d.bottom <= 502.0 && d.right < 104.0, "{d:?}");
}

fn display_list_damage_comes_in_separate_areas() {
    // C++'s display_list_damage_comes_in_separate_areas: what changed in separate places is drawn again in separate
    // areas (bounded in number: those that grow least by it are made one); end() gives their bounds.
    let mut list = DisplayList::new();
    let mut frame = |changed: usize| {
        // the first `changed` boxes in another color
        list.begin(Size::new(400.0, 400.0));
        for i in 0..12 {
            list.begin_node(i as u64 + 1);
            let r = rect((i % 4) as f32 * 100.0 + 10.0, (i / 4) as f32 * 130.0 + 10.0, 20.0, 20.0);
            list.fill_round_rect(r, CornerRadii::Zero, if i < changed { Color::Black } else { Color::White });
            list.end_node();
        }
        let d = list.end();
        (d, list.areas().to_vec())
    };
    frame(0);
    let (d, areas) = frame(2); // the first two: a box at 10 and one at 110 across
    assert!(areas.len() == 2 && same_rect(areas[0], rect(8.0, 8.0, 24.0, 24.0)));
    assert!(same_rect(areas[1], rect(108.0, 8.0, 24.0, 24.0)));
    assert!(same_rect(d.unwrap(), rect(8.0, 8.0, 124.0, 24.0)));
    let (d, areas) = frame(12); // ten more change: no more than eight areas, nothing left out
    let drawn: f32 = areas.iter().map(|a| a.width() * a.height()).sum();
    assert!(areas.len() <= 8 && areas.len() >= 2 && same_rect(d.unwrap(), rect(8.0, 8.0, 324.0, 284.0)));
    assert!(drawn < 324.0 * 284.0 / 2.0);
    for i in 2..12 {
        let b = rect((i % 4) as f32 * 100.0 + 8.0, (i / 4) as f32 * 130.0 + 8.0, 24.0, 24.0);
        assert!(areas.iter().any(|&a| covers(a, b)));
    }
    let (d, areas) = frame(12);
    assert!(d.is_none() && areas.is_empty());
}

fn display_list_damage_of_a_changed_node_only() {
    // A node drawn again the same in a host is no damage; one whose color changed is damage where it is, in whole
    // px, whatever else was drawn again (its parent composed again, its sibling measured again).
    let tint = mutable_state_of(RED);
    let width = mutable_state_of(50);
    let (t, w) = (tint.clone(), width.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, w) = (t.clone(), w.clone());
            Column(Modifier.padding(10.dp()), move || {
                Box(Modifier.size(30.dp()).background_with(Color::Black, RectangleShape));
                Box(Modifier.size(30.dp()).background_with(t.get(), RoundedCornerShape(4.0)));
                Box(Modifier.size_wh(w.get().dp(), 30.dp()).background_with(Color::Black, RectangleShape));
            });
        }),
        IntSize::new(200, 200),
    );
    let mut list = DisplayList::new();
    let mut frame = || {
        list.begin(Size::new(200.0, 200.0));
        rule.host().draw(&mut list);
        (list.end(), list.areas().to_vec())
    };
    frame();
    tint.set(BLUE);
    rule.wait_for_idle();
    let (d, areas) = frame();
    assert!(areas.len() == 1 && same_rect(d.unwrap(), rect(8.0, 38.0, 34.0, 34.0)), "{d:?}");
    width.set(60); // only the third box grows
    rule.wait_for_idle();
    let (d, _) = frame();
    assert!(same_rect(d.unwrap(), rect(8.0, 68.0, 64.0, 34.0)), "{d:?}");
    tint.set(BLUE); // the same: nothing
    rule.wait_for_idle();
    assert!(frame().0.is_none());
}

fn display_list_damage_where_a_bitmap_was_drawn_into() {
    // C++'s bitmaps_are_composes, its display list part: a bitmap drawn into since is a change where it is drawn, as
    // an image and in a shader.
    let shown = ImageBitmap::new(2, 2, ImageBitmapConfig::Argb8888, true, &colorspace::ColorSpaces::Srgb);
    let mut list = DisplayList::new();
    let mut frame = || {
        list.begin(Size::new(100.0, 100.0));
        list.draw_image(&shown, rect(0.0, 0.0, 2.0, 2.0), rect(10.0, 10.0, 20.0, 20.0), 1.0, FilterQuality::Low);
        let brush = ShaderBrush(ImageShader(shown.clone(), TileMode::Clamp, TileMode::Clamp));
        list.fill_gradient(rect(50.0, 50.0, 10.0, 10.0), CornerRadii::Zero, &brush, rect(50.0, 50.0, 10.0, 10.0));
        list.end()
    };
    frame();
    assert!(frame().is_none());
    shown.store(&[255; 16], true);
    let d = frame().unwrap();
    assert!(d.left <= 10.0 && d.right >= 60.0, "{d:?}");
    assert!(frame().is_none());
}

// ---- Moves -----------------------------------------------------------------------------------------------------

fn display_list_moves_what_scrolled_by_whole_pixels() {
    // C++'s display_list_moves_what_scrolled_by_whole_pixels: content that moved by whole pixels within a rect clip
    // (a scrolled list) moves the kept pixels: only what that leaves wrong is drawn again (the strip that came into
    // view, what doesn't move with it where it is and where moving took it). What would make moved pixels wrong
    // (something under them that isn't one color, a clip of another shape, a part of a pixel) draws what changed
    // again instead.
    let scroll = mutable_state_of(0);
    let (fab, gradient, rounded, raised) =
        (mutable_state_of(false), mutable_state_of(false), mutable_state_of(false), mutable_state_of(false));
    let (sc, f, g, ro, ra) = (scroll.clone(), fab.clone(), gradient.clone(), rounded.clone(), raised.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (sc, f, g, ro, ra) = (sc.clone(), f.clone(), g.clone(), ro.clone(), ra.clone());
            Box(
                Modifier.fill_max_size().background_with(Color::White, RectangleShape),
                content = move || {
                    let mut list = Modifier.fill_max_size();
                    if ro.get() {
                        list = list.clip(RoundedCornerShape(16.0));
                    }
                    if g.get() {
                        list = list.background_brush_with(vgrad(&[RED, BLUE]), RectangleShape, 1.0);
                    }
                    let raised = ra.get();
                    Column(scrolled(list, &sc), move || {
                        for i in 0..30 {
                            let mut item = Modifier.fill_max_width().height(50.dp()).padding(4.dp());
                            if raised {
                                item =
                                    item.shadow_with(4.dp(), RoundedCornerShape(8.0), true, Color::Black, Color::Black);
                            }
                            Box(item
                                .background_with(Color::new(0.0, 0.5, i as f32 / 30.0, 1.0), RoundedCornerShape(8.0)));
                        }
                    });
                    if f.get() {
                        let fab = Modifier.align(Alignment::BottomEnd).padding(16.dp()).size(56.dp());
                        Box(fab.background_with(RED, RoundedCornerShape(16.0)));
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let list = RefCell::new(DisplayList::new());
    list.borrow_mut().set_allow_moves(true);
    let frame = || {
        let mut list = list.borrow_mut();
        list.begin(Size::new(400.0, 800.0));
        rule.host().draw(&mut *list);
        list.end()
    };
    let scroll_by = |px: i32| {
        scroll.set(scroll.peek() + px);
        rule.wait_for_idle();
        frame()
    };
    let moves = || list.borrow().moves().to_vec();
    let areas = || list.borrow().areas().to_vec();
    let drawn_again = |r: Rect| areas().iter().any(|&a| covers(a, r));
    let drawn = || areas().iter().map(|a| a.width() * a.height()).sum::<f32>();
    frame();
    // Up by 30: the pixels below 30 move up, the strip at the bottom is new.
    let d = scroll_by(30);
    let m = moves();
    assert!(d.is_some() && m.len() == 1 && same_rect(m[0].from, rect(0.0, 30.0, 400.0, 770.0)), "{m:?}");
    assert!(m[0].by == Offset::new(0.0, -30.0));
    assert!(areas().len() == 1 && same_rect(areas()[0], rect(0.0, 770.0, 400.0, 30.0)), "{:?}", areas());
    // And back down.
    let d = scroll_by(-10);
    let m = moves();
    assert!(d.is_some() && m.len() == 1 && m[0].by.y == 10.0);
    assert!(areas().len() == 1 && same_rect(areas()[0], rect(0.0, 0.0, 400.0, 10.0)));
    assert!(frame().is_none() && moves().is_empty());
    // Over it, what doesn't move: drawn again where it is and where its pixels moved to.
    fab.set(true);
    rule.wait_for_idle();
    frame();
    let d = scroll_by(20);
    assert!(d.is_some() && moves().len() == 1 && drawn_again(rect(0.0, 780.0, 400.0, 20.0)));
    assert!(drawn_again(rect(328.0, 728.0, 56.0, 56.0)) && drawn_again(rect(328.0, 708.0, 56.0, 56.0)));
    assert!(drawn() < 400.0 * 800.0 / 8.0);
    fab.set(false);
    rule.wait_for_idle();
    frame();
    // Under it, a gradient that stays: moved pixels would take it along. What changed is drawn again.
    let all_of_it = |d: Option<Rect>| d.is_some_and(|d| d.top <= 0.0 && d.bottom >= 800.0);
    gradient.set(true);
    rule.wait_for_idle();
    frame();
    let d = scroll_by(20);
    assert!(moves().is_empty() && all_of_it(d));
    gradient.set(false);
    rounded.set(true); // a clip of another shape around it
    rule.wait_for_idle();
    frame();
    let d = scroll_by(20);
    assert!(moves().is_empty() && all_of_it(d));
    rounded.set(false);
    // Each item raised: its elevation shadow falls from the light, which doesn't scroll with it (moved, it would
    // look as it did where it was); drawn again where it was and is.
    raised.set(true);
    rule.wait_for_idle();
    frame();
    let d = scroll_by(20);
    assert!(moves().is_empty() && all_of_it(d));
    raised.set(false);
    rule.wait_for_idle();
    frame();
    list.borrow_mut().set_allow_moves(false);
    let d = scroll_by(20);
    assert!(moves().is_empty() && all_of_it(d));

    // By hand: an item in a scroller's clip (drawn through `shift`), at C++'s half a dp a pixel: its numbers twice.
    let mut part = DisplayList::new();
    part.set_allow_moves(true);
    let (scroller, item) = (1, 2);
    let mut record = |draw: &dyn Fn(&mut DisplayList), clip_height: f32, shift: f32| {
        part.begin(Size::new(200.0, 200.0));
        part.begin_node(scroller);
        part.push_clip(rect(0.0, 0.0, 200.0, clip_height));
        part.push_translation(Offset::new(0.0, shift));
        part.begin_node(item);
        draw(&mut part);
        part.end_node();
        part.pop_translation();
        part.pop_clip();
        part.end_node();
        (part.end(), part.moves().to_vec())
    };
    let boxed =
        |y: f32| move |p: &mut DisplayList| p.fill_round_rect(rect(20.0, y, 100.0, 100.0), CornerRadii::Zero, RED);
    record(&boxed(40.0), 200.0, 0.0);
    let (d, m) = record(&boxed(60.0), 200.0, 0.0);
    assert!(d.is_some() && m.len() == 1 && m[0].by.y == 20.0 && same_rect(m[0].from, rect(0.0, 0.0, 200.0, 180.0)));
    // Moved by part of a pixel: drawn again where it was and is (in whole pixels).
    let (d, m) = record(&boxed(60.5), 200.0, 0.0);
    assert!(m.is_empty() && same_rect(d.unwrap(), rect(18.0, 58.0, 104.0, 105.0)), "{d:?}");
    // Its parts moved apart: the text further than the box.
    record(
        &|p: &mut DisplayList| {
            boxed(40.0)(p);
            p.draw_text(&text("a", at(24.0, 80.0), 20.0), RED);
        },
        200.0,
        0.0,
    );
    let (d, m) = record(
        &|p: &mut DisplayList| {
            boxed(60.0)(p);
            p.draw_text(&text("a", at(24.0, 104.0), 20.0), RED);
        },
        200.0,
        0.0,
    );
    assert!(d.is_some() && m.is_empty());
    // A part drawn twice the size: moved as far in its place, twice as far on the screen.
    let scaled = |y: f32| {
        move |p: &mut DisplayList| {
            p.fill_round_rect(rect(20.0, y, 40.0, 40.0), CornerRadii::Zero, RED);
            let mut twice = Matrix::new();
            twice.scale(2.0, 2.0, 1.0);
            p.push_transform(&twice);
            p.fill_round_rect(rect(20.0, y, 20.0, 20.0), CornerRadii::Zero, RED);
            p.pop_transform();
        }
    };
    record(&scaled(20.0), 200.0, 0.0);
    let (d, m) = record(&scaled(40.0), 200.0, 0.0);
    assert!(d.is_some() && m.is_empty());
    // The clip around it changed (pixels it cut before would come into it).
    record(&boxed(40.0), 160.0, 0.0);
    let (d, m) = record(&boxed(60.0), 200.0, 0.0);
    assert!(d.is_some() && m.is_empty());
    // Drawn through a translation that changed as much the other way: in its place on the screen.
    record(&boxed(60.0), 200.0, 0.0);
    let (d, m) = record(&boxed(40.0), 200.0, 20.0);
    assert!(d.is_some() && m.is_empty());
}

// ---- Layers ----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Look {
    alpha: f32,
    transform: LayerTransform,
    /// Where the layer is; where its first rect is in it.
    x: f32,
    inside: f32,
    color: Color,
}

impl Default for Look {
    fn default() -> Look {
        Look { alpha: 1.0, transform: LayerTransform::Identity, x: 50.0, inside: 0.0, color: Color::Black }
    }
}

fn display_list_tells_canvases_which_layers_stayed_the_same() {
    // C++'s display_list_tells_canvases_which_layers_stayed_the_same: a layer is known by its node from frame to
    // frame (key_layer), with whether what is drawn in it is what was last frame in its own space: so when it only
    // faded, turned or moved with it, not when what is in it changed. Kept by the canvas, what is in it isn't
    // replayed; to be kept, all of it is (outside the area replayed too).
    let list = RefCell::new(DisplayList::new());
    let (node, other) = (1, 2);
    let keys = |l: Look| {
        let mut list = list.borrow_mut();
        list.begin(Size::new(400.0, 400.0));
        list.begin_node(node);
        list.push_layer(rect(l.x, 50.0, 100.0, 100.0), CornerRadii::all(8.0), l.alpha, l.transform);
        list.fill_round_rect(rect(l.x + 10.0 + l.inside, 60.0, 30.0, 30.0), CornerRadii::Zero, l.color);
        list.fill_round_rect(rect(l.x + 60.0, 110.0, 30.0, 30.0), CornerRadii::Zero, Color::Black);
        list.pop_layer();
        list.end_node();
        list.begin_node(other);
        list.push_layer(rect(300.0, 300.0, 50.0, 50.0), CornerRadii::Zero, 1.0, LayerTransform::Identity);
        list.fill_round_rect(rect(300.0, 300.0, 20.0, 20.0), CornerRadii::Zero, Color::Black);
        list.pop_layer();
        // (another of the same node's)
        list.push_layer(rect(300.0, 300.0, 50.0, 50.0), CornerRadii::Zero, 1.0, LayerTransform::Identity);
        list.fill_round_rect(rect(310.0, 310.0, 20.0, 20.0), CornerRadii::Zero, Color::White);
        list.pop_layer();
        list.end_node();
        list.end();
        let mut canvas = RecordingCanvas::new();
        list.replay(&mut canvas, None);
        canvas.layer_keys
    };
    let first = keys(Look::default());
    assert!(first.len() == 3 && first.iter().all(|k| !k.1)); // (none before)
    assert!(first[0].0 != first[1].0 && first[1].0 != first[2].0 && first[0].0 != first[2].0);
    let again = keys(Look::default());
    assert!(again.len() == 3 && again.iter().all(|k| k.1));
    assert!(again.iter().zip(&first).all(|(a, b)| a.0 == b.0));
    assert!(keys(Look { alpha: 0.5, ..Look::default() })[0].1); // faded
    let turned = LayerTransform { scale_x: 1.2, rotation_z: 8.0, ..LayerTransform::Identity };
    assert!(keys(Look { alpha: 0.5, transform: turned, ..Look::default() })[0].1); // turned
    assert!(keys(Look { alpha: 0.5, transform: turned, x: 70.0, ..Look::default() })[0].1); // moved, with what is in it
    let changed = keys(Look { x: 70.0, color: Color::White, ..Look::default() });
    assert!(!changed[0].1 && changed[1].1); // what is in it changed (not in the other)
    assert!(!keys(Look { x: 70.0, inside: 5.0, color: Color::White, ..Look::default() })[0].1); // moved in it
    // Kept by the canvas: what is in the layers isn't replayed (they are, to be composited).
    let list = list.borrow();
    let mut kept = recording(LayerContent::Kept);
    list.replay(&mut kept, None);
    assert!(kept.layers == 3 && kept.rects.is_empty() && kept.depth == 0);
    // To be kept: all of what is in it, though one of its rects is outside the area replayed.
    let mut whole = recording(LayerContent::DrawAll);
    let mut part = RecordingCanvas::new();
    list.replay(&mut whole, Some(rect(75.0, 55.0, 20.0, 20.0)));
    list.replay(&mut part, Some(rect(75.0, 55.0, 20.0, 20.0)));
    assert!(whole.layers == 1 && whole.rects.len() == 2 && part.layers == 1 && part.rects.len() == 1);
}

fn display_list_keeps_a_layers_content_whatever_its_transform() {
    // C++'s display_list_keeps_a_layers_content_whatever_its_transform: what a layer holds is kept as its content,
    // not as far as its transform takes it: what shows at rest stays recorded as the layer grows past the screen (a
    // stretch pulled further: the same layer), and what only its transform brings onto the screen (shrinking) is
    // recorded too.
    let mut list = DisplayList::new();
    let node = 1;
    let mut frame = |scale_y: f32| {
        list.begin(Size::new(100.0, 200.0));
        list.begin_node(node);
        let t = LayerTransform { scale_y, origin: at(0.5, 0.0), ..LayerTransform::Identity };
        list.push_layer(rect(0.0, 0.0, 100.0, 200.0), CornerRadii::Zero, 1.0, t);
        for i in 0..5 {
            // (the last below the screen)
            list.fill_round_rect(rect(10.0, i as f32 * 50.0 + 5.0, 80.0, 40.0), CornerRadii::Zero, Color::Black);
        }
        list.pop_layer();
        list.end_node();
        list.end();
        let mut canvas = recording(LayerContent::DrawAll);
        list.replay(&mut canvas, None);
        (canvas.rects.len(), canvas.layer_keys.first().is_some_and(|k| k.1))
    };
    assert_eq!(frame(1.0).0, 4); // at rest: the four on the screen
    assert_eq!(frame(1.1), (4, true)); // grown
    assert_eq!(frame(1.35), (4, true)); // grown so far the fourth is past the screen's edge: kept all the same
    assert_eq!(frame(0.6), (5, false)); // shrunk: the fifth comes onto the screen
    // What a clip around it or in it cuts off, at rest and grown alike, isn't kept.
    list.begin(Size::new(100.0, 200.0));
    list.begin_node(node);
    list.push_clip(rect(0.0, 0.0, 100.0, 100.0));
    let t = LayerTransform { scale_y: 1.35, origin: at(0.5, 0.0), ..LayerTransform::Identity };
    list.push_layer(rect(0.0, 0.0, 100.0, 200.0), CornerRadii::Zero, 1.0, t);
    // Shown at rest only; neither way.
    assert!(!list.quick_reject(rect(10.0, 80.0, 80.0, 10.0)) && list.quick_reject(rect(10.0, 150.0, 80.0, 10.0)));
    list.fill_round_rect(rect(10.0, 150.0, 80.0, 10.0), CornerRadii::Zero, Color::Black); // past the clip around it
    list.push_clip(rect(0.0, 0.0, 100.0, 50.0));
    list.fill_round_rect(rect(10.0, 60.0, 80.0, 10.0), CornerRadii::Zero, Color::Black); // past the clip in it
    list.pop_clip();
    list.pop_layer();
    list.pop_clip();
    list.end_node();
    list.end();
    let mut cut = recording(LayerContent::DrawAll);
    list.replay(&mut cut, None);
    assert!(cut.rects.is_empty());
}

fn display_list_lines_frames_up_by_node_and_replays_an_area() {
    // C++'s display_list_lines_frames_up_by_node_and_replays_an_area.
    let mut list = DisplayList::new();
    let (a, b, c, layer) = (1, 2, 3, 4); // stand-ins for layout nodes
    let mut frame = |swapped: bool, alpha: f32, with_c: bool| {
        list.begin(Size::new(400.0, 400.0));
        let boxed = |list: &mut DisplayList, node: u64, r: Rect, color: Color| {
            list.begin_node(node);
            list.fill_round_rect(r, CornerRadii::Zero, color);
            list.end_node();
        };
        if swapped {
            boxed(&mut list, b, rect(50.0, 50.0, 100.0, 100.0), BLUE);
            boxed(&mut list, a, rect(0.0, 0.0, 100.0, 100.0), RED);
        } else {
            boxed(&mut list, a, rect(0.0, 0.0, 100.0, 100.0), RED);
            boxed(&mut list, b, rect(50.0, 50.0, 100.0, 100.0), BLUE);
        }
        list.begin_node(layer);
        let twice = LayerTransform { scale_x: 2.0, scale_y: 2.0, ..LayerTransform::Identity };
        list.push_layer(rect(200.0, 200.0, 100.0, 100.0), CornerRadii::all(8.0), alpha, twice); // around its center: 150..350
        if with_c {
            boxed(&mut list, c, rect(200.0, 200.0, 10.0, 10.0), GREEN);
        }
        list.fill_round_rect(rect(500.0, 0.0, 10.0, 10.0), CornerRadii::Zero, Color::Black); // off the screen: not kept
        list.push_clip(rect(0.0, 0.0, 10.0, 10.0));
        list.fill_round_rect(rect(20.0, 20.0, 10.0, 10.0), CornerRadii::Zero, Color::Black); // clipped away: not kept either
        list.pop_clip();
        list.pop_layer();
        list.end_node();
        (list.end(), list.len())
    };
    frame(false, 1.0, false);
    let (d, len) = frame(false, 1.0, false);
    assert!(d.is_none() && len == 6); // two boxes, the layer, the clip
    // The same boxes, the other one on top: the one that moved up covers the overlap.
    let d = frame(true, 1.0, false).0.unwrap();
    assert!(d.left <= 0.0 && d.right >= 100.0 && d.right <= 104.0 && d.bottom <= 104.0, "{d:?}");
    // The layer fades: where it is composited.
    let d = frame(true, 0.5, false).0.unwrap();
    assert!(d.left <= 150.0 && d.left >= 145.0 && d.right >= 350.0 && d.right <= 355.0, "{d:?}");
    // A node appears in it, scaled twice around the center.
    let d = frame(true, 0.5, true).0.unwrap();
    assert!(d.left <= 150.0 && d.right >= 170.0 && d.left >= 145.0 && d.right <= 175.0, "{d:?}");
    let mut all = RecordingCanvas::new();
    list.replay(&mut all, None);
    assert!(all.rects.len() == 3 && all.layers == 1 && all.depth == 0 && all.clips.len() == 1);
    // Only what touches it; the layer isn't even pushed.
    let mut part = RecordingCanvas::new();
    list.replay(&mut part, Some(rect(0.0, 0.0, 40.0, 40.0)));
    assert!(part.rects.len() == 1 && part.rects[0].width() == 100.0 && part.layers == 0 && part.clips.is_empty());
}

// ---- Kept nodes ------------------------------------------------------------------------------------------------

/// C++'s display_list_keeps_what_unchanged_nodes_drew scene's state.
#[derive(Default)]
struct Scene {
    scroll: f32,
    list_changed: bool,
    clip_wide: bool,
    twice: bool,
    item_changed: Option<usize>,
}

const ROOT: u64 = 1;
const LIST: u64 = 2;
fn item(i: usize) -> u64 {
    10 + i as u64
}
fn icon(i: usize) -> u64 {
    100 + i as u64
}

/// Draws a node as a layout node draws: asked first whether it is kept (where it is in its parent: `at`), then
/// between begin_node and end_node, translated there (unless it is at none).
fn node(
    g: &mut dyn GraphicsContext,
    answers: Option<&RefCell<Vec<NodeRecording>>>,
    id: u64,
    at: Offset,
    unchanged: bool,
    body: &mut dyn FnMut(&mut dyn GraphicsContext),
) {
    let r = g.record_node(id, at, unchanged);
    if let Some(a) = answers {
        a.borrow_mut().push(r);
    }
    if r == NodeRecording::Kept {
        return;
    }
    g.begin_node(id);
    if at != Offset::Zero {
        g.push_translation(at);
    }
    body(g);
    if at != Offset::Zero {
        g.pop_translation();
    }
    g.end_node();
}

fn scene(g: &mut dyn GraphicsContext, s: &Scene, answers: Option<&RefCell<Vec<NodeRecording>>>) {
    node(g, answers, ROOT, Offset::Zero, false, &mut |g| {
        g.push_clip(rect(0.0, 0.0, if s.clip_wide { 400.0 } else { 300.0 }, 400.0));
        let times = if s.twice { 2 } else { 1 }; // (a second time in the frame: drawn)
        for _ in 0..times {
            let list_at = at(0.0, -s.scroll);
            node(g, answers, LIST, list_at, !s.list_changed, &mut |g| {
                g.fill_round_rect(rect(0.0, 0.0, 300.0, 600.0), CornerRadii::Zero, Color::White);
                for i in 0..10 {
                    let item_at = at(0.0, i as f32 * 60.0);
                    node(g, answers, item(i), item_at, s.item_changed != Some(i), &mut |g| {
                        if i == 7 && g.quick_reject(rect(0.0, 0.0, 300.0, 50.0)) {
                            return; // (a card clipped to itself)
                        }
                        let color = Color::new(0.0, 0.0, i as f32 / 10.0, 1.0);
                        g.fill_round_rect(rect(4.0, 4.0, 292.0, 42.0), CornerRadii::all(8.0), color);
                        g.draw_text(&text("item", at(60.0, 30.0), 14.0), Color::Black);
                        if i == 3 {
                            let _ = g.light_center(); // (a shadow cast from the light)
                        }
                        if i == 9 {
                            // (clipped away, and off the screen)
                            let outline = Outline::Rectangle(rect(0.0, 0.0, 292.0, 42.0));
                            g.draw_elevation_shadow(&outline, at(4.0, 4.0), &ElevationShadow::default());
                            g.draw_elevation_shadow(&outline, at(4.0, 4.0), &ElevationShadow::flat(8.0));
                        }
                        if i == 5 {
                            let mut m = Matrix::new();
                            m.translate(10.0, 0.0, 0.0);
                            g.push_transform(&m);
                            g.fill_round_rect(rect(0.0, 0.0, 4.0, 4.0), CornerRadii::Zero, Color::Black);
                            g.pop_transform();
                        }
                        let icon_at = at(12.0, 13.0);
                        node(g, answers, icon(i), icon_at, true, &mut |g| {
                            let mut p = Path::new();
                            p.add_oval(rect(0.0, 0.0, 24.0, 24.0), PathDirection::CounterClockwise);
                            let black = SolidColor(Color::Black);
                            g.draw_path(&p, Offset::Zero, &black, rect(0.0, 0.0, 24.0, 24.0), &PathStroke::default());
                        });
                    });
                }
            });
        }
        g.pop_clip();
    });
}

fn display_list_keeps_what_unchanged_nodes_drew() {
    // C++'s display_list_keeps_what_unchanged_nodes_drew, drawn as layout nodes draw (each in its own space,
    // translated to where it is in its parent): where nodes are kept, one unchanged since the last frame records
    // again what it drew then, as drawing it would record it (moved: its translation); where what it left out or
    // asked comes out otherwise (scrolled into view, a quick_reject answered otherwise), or it can't be moved so
    // (the light of a shadow), it is drawn. (C++ drew in root coordinates, so a transform's matrix was from where
    // the node was and a node with one was drawn; here the matrix is the node's own: it is kept.)
    let mut kept = DisplayList::new();
    let mut drawn = DisplayList::new();
    kept.set_keep_nodes(true);
    let answers = RefCell::new(Vec::new());
    let mut s = Scene::default();
    let mut frame = |s: &Scene| {
        answers.borrow_mut().clear();
        kept.begin(Size::new(400.0, 400.0));
        scene(&mut kept, s, Some(&answers));
        let a = kept.end();
        drawn.begin(Size::new(400.0, 400.0));
        scene(&mut drawn, s, None);
        let b = drawn.end();
        let same = kept.same_frame(&drawn);
        let damage_same = a.is_some() == b.is_some() && a.zip(b).is_none_or(|(a, b)| same_rect(a, b));
        assert!(same.is_ok() && damage_same, "{same:?} {a:?} {b:?}");
        (answers.borrow().clone(), kept.areas().to_vec())
    };
    use NodeRecording::*;
    let count = |a: &[NodeRecording], r: NodeRecording| a.iter().filter(|&&x| x == r).count();
    // Its answer, after the list's (and those of the items before it, and their icons).
    let item_answer = |a: &[NodeRecording], i: usize| {
        let mut at = 2;
        for _ in 0..i {
            at += if a[at] == Kept { 1 } else { 2 };
        }
        a[at]
    };
    // The root, the list, the items and their icons (the seventh rejected itself before its icon).
    let (a, _) = frame(&s);
    assert!(count(&a, Kept) == 0 && count(&a, Fresh) == 21);
    // Nothing changed: the list is kept whole.
    let (a, areas) = frame(&s);
    assert!(a.len() == 2 && a[1] == Kept && areas.is_empty());
    // Scrolled: the item scrolled into view was left out, so the list is drawn; of its items, those that showed are
    // kept; the one cast from the light and the one whose shadows were clipped away from it are drawn.
    s.scroll = 30.0;
    let (a, _) = frame(&s);
    assert_eq!(a[1], Fresh);
    for i in [0, 1, 2, 4, 5, 6] {
        assert_eq!(item_answer(&a, i), Kept, "item {i}");
    }
    assert_eq!(item_answer(&a, 3), Fresh);
    assert_eq!(item_answer(&a, 9), Fresh); // its shadow clipped away is clipped from the light, which doesn't move
    // Scrolled further: the card rejected while off the screen comes into view.
    s.scroll = 80.0;
    let (a, _) = frame(&s);
    assert_eq!(item_answer(&a, 7), Fresh);
    let (a, _) = frame(&s); // and staying put, it is all kept again
    assert!(a.len() == 2 && a[1] == Kept);
    // A changed item is drawn, the rest kept; a node drawn in other clips is drawn.
    s.list_changed = true;
    s.item_changed = Some(4);
    let (a, _) = frame(&s);
    assert!(item_answer(&a, 4) == Fresh && item_answer(&a, 2) == Kept);
    s.list_changed = false;
    s.item_changed = None;
    s.clip_wide = true;
    let (a, _) = frame(&s);
    assert_eq!(a[1], Fresh);
    s.clip_wide = false;
    frame(&s);
    // Drawn twice in a frame: the second time is another's.
    s.twice = true;
    let (a, _) = frame(&s);
    assert!(*a.last().unwrap() != Kept && count(&a, Other) >= 1);
    s.twice = false;
    frame(&s);
    // Scrolled back and forth by fractions: what is kept is still what drawing it records.
    for to in [12.5, 47.25, 0.0, 133.0, 7.0] {
        s.scroll = to;
        frame(&s);
    }
}

fn display_list_kept_nodes_recheck_quick_rejects_and_culls() {
    // A node kept moved asks again what it asked (quick_reject) and checks again what it left out (drawn clipped
    // away): answered as before, it is kept; otherwise drawn.
    let mut list = DisplayList::new();
    list.set_keep_nodes(true);
    let mut frame = |y: f32, clip: f32| {
        list.begin(Size::new(200.0, 200.0));
        list.push_clip(rect(0.0, 0.0, 200.0, clip));
        let mut answer = None;
        let r = list.record_node(7, at(0.0, y), true);
        if r != NodeRecording::Kept {
            list.begin_node(7);
            list.push_translation(at(0.0, y));
            answer = Some(list.quick_reject(rect(0.0, 0.0, 50.0, 50.0)));
            if answer == Some(false) {
                list.fill_round_rect(rect(0.0, 0.0, 50.0, 50.0), CornerRadii::Zero, RED);
            }
            list.fill_round_rect(rect(0.0, 60.0, 50.0, 10.0), CornerRadii::Zero, BLUE); // past the clip at first
            list.pop_translation();
            list.end_node();
        }
        list.pop_clip();
        list.end();
        (r, answer, list.kept_nodes(), list.drawn_nodes())
    };
    assert_eq!(frame(0.0, 50.0).0, NodeRecording::Fresh);
    assert!(matches!(frame(0.0, 50.0), (NodeRecording::Kept, None, 1, 0))); // where it was
    assert!(matches!(frame(-5.0, 50.0), (NodeRecording::Kept, None, 1, 0))); // moved, still accepted, still cut off
    // Moved so far its blue bar comes into view: drawn.
    assert!(matches!(frame(-20.0, 50.0), (NodeRecording::Fresh, Some(false), 0, 1)));
    // Moved out of view: drawn, its quick_reject answered otherwise.
    assert!(matches!(frame(-200.0, 50.0), (NodeRecording::Fresh, Some(true), 0, 1)));
    assert!(matches!(frame(-190.0, 50.0), (NodeRecording::Kept, None, 1, 0))); // still out of view
    assert!(matches!(frame(-20.0, 50.0), (NodeRecording::Fresh, Some(false), 0, 1))); // back
    // In another clip: drawn.
    assert_eq!(frame(-20.0, 60.0).0, NodeRecording::Fresh);
}

fn display_list_keeps_a_node_moved_again_and_again() {
    // A node kept as it moves, frame after frame, to its parent's top left and away again, records what drawing it
    // records (its translation there, or none at none), its child kept with it.
    let mut kept = DisplayList::new();
    kept.set_keep_nodes(true);
    let mut drawn = DisplayList::new();
    let scene = |g: &mut dyn GraphicsContext, y: f32| {
        node(g, None, ROOT, Offset::Zero, false, &mut |g| {
            node(g, None, LIST, at(0.0, y), true, &mut |g| {
                g.fill_round_rect(rect(0.0, 0.0, 50.0, 20.0), CornerRadii::Zero, RED);
                node(g, None, item(0), at(5.0, 5.0), true, &mut |g| {
                    g.fill_round_rect(rect(0.0, 0.0, 10.0, 10.0), CornerRadii::Zero, BLUE);
                });
            });
        });
    };
    for (i, y) in [0.0, 10.0, 25.0, 0.0, 0.0, 7.5, 30.0].into_iter().enumerate() {
        kept.begin(Size::new(100.0, 100.0));
        scene(&mut kept, y);
        let a = kept.end();
        drawn.begin(Size::new(100.0, 100.0));
        scene(&mut drawn, y);
        let b = drawn.end();
        assert!(kept.same_frame(&drawn).is_ok() && a == b, "{y}: {:?} {a:?} {b:?}", kept.same_frame(&drawn));
        assert_eq!(kept.kept_nodes(), (i > 0) as usize, "{y}");
    }
}

fn display_list_keeps_unchanged_nodes_of_a_host() {
    // Over a host, a list keeping nodes records what drawing every node records (the host's keep check says so each
    // frame too): a changed node is drawn, the others kept; a scrolled list's items are kept, moved with it.
    let tint = mutable_state_of(RED);
    let scroll = mutable_state_of(0);
    let (t, sc) = (tint.clone(), scroll.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, sc) = (t.clone(), sc.clone());
            Column(Modifier.fill_max_size(), move || {
                Box(Modifier.size(30.dp()).background_with(t.get(), RectangleShape));
                Column(scrolled(Modifier.height(200.dp()), &sc), || {
                    for i in 0..20 {
                        let color = Color::new(0.0, i as f32 / 20.0, 0.0, 1.0);
                        Box(Modifier.fill_max_width().height(30.dp()).background_with(color, RectangleShape));
                    }
                });
                Text("altta");
            });
        }),
        IntSize::new(200, 400),
    );
    let mut kept = DisplayList::new();
    kept.set_keep_nodes(true);
    let mut fresh = DisplayList::new();
    let mut frame = || {
        kept.begin(Size::new(200.0, 400.0));
        rule.host().draw(&mut kept);
        let a = kept.end();
        fresh.begin(Size::new(200.0, 400.0));
        rule.host().draw(&mut fresh);
        let b = fresh.end();
        let same = kept.same_frame(&fresh);
        assert!(same.is_ok() && a == b, "{same:?} {a:?} {b:?}");
        (kept.kept_nodes(), kept.drawn_nodes(), a)
    };
    let (k, n, _) = frame();
    assert!(k == 0 && n > 20);
    let (k, n, d) = frame(); // nothing changed: the root is kept whole
    assert!(k == 1 && n == 0 && d.is_none());
    tint.set(BLUE);
    rule.wait_for_idle();
    let (k, n, d) = frame();
    assert!(k > 0 && n >= 1 && same_rect(d.unwrap(), rect(0.0, 0.0, 32.0, 32.0)), "{k} {n} {d:?}");
    scroll.set(45);
    rule.wait_for_idle();
    let (k, n, _) = frame();
    assert!(k >= 6 && n >= 1, "{k} {n}"); // the items that showed are kept, moved with what they are in
    frame();
}

// ---- Replay and cover ------------------------------------------------------------------------------------------

fn display_list_leaves_out_what_an_opaque_rect_hides() {
    // C++'s display_list_leaves_out_what_an_opaque_rect_hides: an opaque rect square to the screen that covers all of
    // an area hides what came before it there: replayed with it, that is left out (its state isn't). One that lets
    // something through doesn't.
    let list = RefCell::new(DisplayList::new());
    let area = rect(10.0, 10.0, 100.0, 100.0);
    let record = |draw: &dyn Fn(&mut DisplayList)| {
        let mut list = list.borrow_mut();
        list.begin(Size::new(400.0, 400.0));
        list.begin_node(1);
        draw(&mut list);
        list.end_node();
        list.end();
    };
    record(&|l| {
        l.fill_round_rect(rect(0.0, 0.0, 400.0, 400.0), CornerRadii::Zero, Color::White); // 0
        l.push_translation(at(20.0, 20.0)); // 1: all of it hidden
        l.fill_round_rect(rect(0.0, 0.0, 30.0, 30.0), CornerRadii::all(4.0), Color::Red); // 2
        l.pop_translation(); // 3
        l.push_translation(at(5.0, 5.0)); // 4
        l.push_clip(rect(0.0, 0.0, 300.0, 300.0)); // 5
        l.fill_round_rect(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::Zero, BLUE); // 6: covers 5..205
        l.pop_clip(); // 7
        l.pop_translation(); // 8
        l.fill_round_rect(rect(50.0, 50.0, 10.0, 10.0), CornerRadii::Zero, Color::Black); // 9
    });
    let cover = list.borrow().opaque_cover(area);
    assert_eq!(cover, Some(6));
    let mut replayed = RecordingCanvas::new();
    list.borrow().replay_from(&mut replayed, Some(area), 6);
    assert!(replayed.rects.len() == 2 && replayed.rect_colors == [BLUE, Color::Black] && replayed.stack == "MCcm");
    let mut all = RecordingCanvas::new();
    list.borrow().replay(&mut all, Some(area));
    assert!(all.rects.len() == 4 && all.stack == "MmMCcm");
    // What lets something through: none covers.
    let covered = |draw: &dyn Fn(&mut DisplayList)| {
        record(&|l| {
            l.fill_round_rect(rect(0.0, 0.0, 400.0, 400.0), CornerRadii::Zero, Color::White);
            draw(l);
        });
        list.borrow().opaque_cover(area).is_some_and(|c| c > 0)
    };
    let square = |l: &mut DisplayList| l.fill_round_rect(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::Zero, BLUE);
    assert!(covered(&square));
    assert!(covered(&|l| {
        // after a layer, once it is done
        l.push_layer(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::Zero, 0.5, LayerTransform::Identity);
        l.fill_round_rect(rect(20.0, 20.0, 50.0, 50.0), CornerRadii::Zero, BLUE);
        l.pop_layer();
        square(l);
    }));
    let translucent = Color::new(0.0, 0.0, 1.0, 0.5);
    assert!(!covered(&|l| l.fill_round_rect(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::Zero, translucent)));
    assert!(!covered(&|l| l.fill_round_rect(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::all(8.0), BLUE))); // round corners
    assert!(covered(&|l| l.fill_round_rect(area, CornerRadii::Zero, BLUE))); // all of its pixels: its edges on theirs
    // Its edge anti-aliased on the area's border pixels.
    assert!(!covered(&|l| l.fill_round_rect(rect(10.5, 10.0, 100.0, 100.0), CornerRadii::Zero, BLUE)));
    record(&|l| l.fill_round_rect(rect(10.5, 10.0, 100.0, 100.0), CornerRadii::Zero, BLUE));
    assert!(list.borrow().opaque_cover(rect(10.5, 10.0, 100.0, 100.0)).is_none()); // an area through a pixel: all of it
    assert!(!covered(&|l| {
        // a clip that cuts the area
        l.push_clip(rect(0.0, 0.0, 50.0, 400.0));
        square(l);
        l.pop_clip();
    }));
    assert!(!covered(&|l| {
        l.push_clip_round_rect(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::all(16.0), ClipOp::Intersect);
        square(l);
        l.pop_clip();
    }));
    assert!(!covered(&|l| {
        l.push_layer(rect(0.0, 0.0, 200.0, 200.0), CornerRadii::Zero, 1.0, LayerTransform::Identity);
        square(l);
        l.pop_layer();
    }));
    assert!(!covered(&|l| {
        l.push_blend_mode(BlendMode::Multiply);
        square(l);
        l.pop_blend_mode();
    }));
    assert!(!covered(&|l| {
        let mut turned = Matrix::new();
        turned.rotate_z(10.0);
        l.push_transform(&turned);
        l.fill_round_rect(rect(-50.0, -50.0, 300.0, 300.0), CornerRadii::Zero, BLUE);
        l.pop_transform();
    }));
}

/// Every kind of command, once.
fn every_kind(l: &mut dyn GraphicsContext, image: &ImageBitmap) {
    let black = SolidColor(Color::Black);
    let gradient = hgrad(&[RED, BLUE]);
    let mut tri = Path::new();
    tri.move_to(0.0, 0.0);
    tri.line_to(20.0, 0.0);
    tri.line_to(10.0, 15.0);
    tri.close();
    let some_shadow = |x: f32| Shadow::new(Color::Black, at(x, 2.0), 3.0, 1.0);
    l.begin_node(1);
    l.push_translation(at(5.0, 6.0));
    l.fill_round_rect(rect(0.0, 0.0, 30.0, 20.0), CornerRadii::all(3.0), RED);
    l.stroke_round_rect(rect(40.0, 0.0, 30.0, 20.0), CornerRadii::all(3.0), 2.0, BLUE);
    l.draw_line(at(0.0, 30.0), at(50.0, 35.0), 1.5, GREEN);
    l.draw_text(&text("yazı", at(0.0, 60.0), 12.0), Color::Black);
    l.draw_text_brush(&text("fırça", at(60.0, 60.0), 12.0), &gradient, rect(60.0, 50.0, 40.0, 15.0));
    l.draw_text_shadow(&text("gölge", at(120.0, 60.0), 12.0), Color::Black, 2.0);
    l.draw_shadow(rect(0.0, 70.0, 30.0, 30.0), CornerRadii::all(4.0), &some_shadow(1.0), false);
    l.draw_arc(rect(40.0, 70.0, 30.0, 30.0), 10.0, 200.0, 3.0, RED, true);
    l.draw_image(image, rect(0.0, 0.0, 4.0, 4.0), rect(80.0, 70.0, 16.0, 16.0), 0.8, FilterQuality::High);
    l.fill_gradient(rect(100.0, 70.0, 30.0, 30.0), CornerRadii::Zero, &gradient, rect(100.0, 70.0, 30.0, 30.0));
    l.draw_path(&tri, at(140.0, 70.0), &black, rect(140.0, 70.0, 20.0, 15.0), &PathStroke::new(2.0));
    let turned = LayerTransform { rotation_z: 5.0, ..LayerTransform::Identity };
    l.push_layer(rect(0.0, 110.0, 50.0, 50.0), CornerRadii::all(5.0), 0.7, turned);
    l.fill_round_rect(rect(5.0, 115.0, 10.0, 10.0), CornerRadii::Zero, Color::Black);
    l.pop_layer();
    l.push_outline_layer(rect(60.0, 110.0, 40.0, 40.0), &tri, 1.0, LayerTransform::Identity);
    l.fill_round_rect(rect(60.0, 110.0, 10.0, 10.0), CornerRadii::Zero, Color::Black);
    l.pop_layer();
    l.push_effect_layer(rect(110.0, 110.0, 40.0, 40.0), &BlurEffect(3.0, 3.0, TileMode::Decal), 1.0);
    l.fill_round_rect(rect(115.0, 115.0, 10.0, 10.0), CornerRadii::Zero, Color::Black);
    l.pop_layer();
    l.push_clip(rect(0.0, 170.0, 50.0, 50.0));
    l.push_clip_round_rect(rect(0.0, 170.0, 40.0, 40.0), CornerRadii::all(6.0), ClipOp::Intersect);
    l.push_clip_path(&tri, at(0.0, 170.0), ClipOp::Difference);
    l.fill_round_rect(rect(0.0, 170.0, 50.0, 50.0), CornerRadii::Zero, GREEN);
    l.pop_clip();
    l.pop_clip();
    l.pop_clip();
    l.push_blend_mode(BlendMode::Multiply);
    l.push_anti_alias(false);
    l.push_color_filter(&ColorFilter::tint(RED, BlendMode::SrcIn));
    let mut m = Matrix::new();
    m.rotate_z(15.0);
    l.push_transform(&m);
    l.fill_round_rect(rect(60.0, 170.0, 20.0, 20.0), CornerRadii::Zero, BLUE);
    l.pop_transform();
    l.pop_color_filter();
    l.pop_anti_alias();
    l.pop_blend_mode();
    l.draw_shadow_path(&tri, at(100.0, 170.0), &some_shadow(2.0), false);
    l.draw_backdrop(rect(130.0, 170.0, 30.0, 30.0), &BlurEffect(5.0, 5.0, TileMode::Clamp), 1.0);
    let elevated = Outline::Rectangle(rect(0.0, 0.0, 30.0, 20.0));
    l.draw_elevation_shadow(&elevated, at(0.0, 230.0), &ElevationShadow::flat(6.0));
    let brushed = Shadow { brush: Some(gradient.clone()), blur_radius: 4.0, ..Shadow::default() };
    let ltr = LayoutDirection::Ltr;
    l.draw_shape_shadow(
        rect(40.0, 230.0, 30.0, 20.0),
        &RoundedCornerShape(5.0),
        ltr,
        &Density::default(),
        &brushed,
        false,
    );
    let corners = [at(0.0, 0.0), at(20.0, 0.0), at(0.0, 20.0)];
    let vertices = Vertices::new(VertexMode::Triangles, &corners, &corners, &[RED, GREEN, BLUE], &[]);
    l.draw_vertices(&vertices, at(80.0, 230.0), BlendMode::Modulate, &black, rect(80.0, 230.0, 20.0, 20.0), 1.0);
    let mask_at = rect(110.0, 230.0, 16.0, 16.0);
    l.draw_image_mask(image, rect(0.0, 0.0, 4.0, 4.0), mask_at, &gradient, mask_at, 1.0, FilterQuality::Low);
    l.pop_translation();
    l.end_node();
}

fn display_list_replays_every_kind_as_drawn() {
    // A frame of every kind of command replayed into another list records what drawing it there does; replayed
    // again the same, it is no damage; through a recording canvas, each comes as drawn.
    let image = ImageBitmap::new(4, 4, ImageBitmapConfig::Argb8888, true, &colorspace::ColorSpaces::Srgb);
    let mut drawn = DisplayList::new();
    drawn.begin(Size::new(300.0, 300.0));
    every_kind(&mut drawn, &image);
    drawn.end();
    let mut replayed = DisplayList::new();
    for _ in 0..2 {
        replayed.begin(Size::new(300.0, 300.0));
        replayed.begin_node(1); // (as it was drawn: the node isn't replayed, what it drew is)
        drawn.replay(&mut replayed, None);
        replayed.end_node();
        replayed.end();
    }
    assert!(replayed.areas().is_empty());
    assert_eq!(replayed.same_frame(&drawn), Ok(()));
    assert!(drawn.len() > 31);
    let mut canvas = RecordingCanvas::new();
    drawn.replay(&mut canvas, None);
    assert!(canvas.texts.iter().any(|t| t == "yazı") && canvas.texts.iter().any(|t| t == "fırça"));
    assert!(canvas.shadow_texts == ["gölge"] && canvas.backdrops.len() == 1 && canvas.elevation_shadows.len() == 1);
    assert!(canvas.brushed_shadows.len() == 1 && canvas.meshes.len() == 1 && canvas.layer_outlines.len() == 1);
    assert!(canvas.clip_shapes.len() == 2 && canvas.arcs.len() == 1 && canvas.stack.contains("BAF"));
    assert_eq!(canvas.layer_effects.iter().filter(|e| e.is_some()).count(), 1);
}

fn display_list_translations_are_replayed() {
    // C++'s translations_move_drawn_content_and_are_replayed, its display list half: a scope's translation recorded
    // and replayed as it was drawn; moved, damage where it was and where it is; an area away from it replays nothing.
    let by = mutable_state_of(10.0f32);
    let b = by.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        Box(
            modifier = Modifier.padding(20.dp()).draw_with_content(move |d| {
                let x = b.get();
                d.translate_with(x, 5.0, |moved| {
                    moved.draw_content(); // the content moves too
                    moved.draw_rect_with(GREEN, Offset::Zero, sz(4.0, 4.0), 1.0, Fill, None, SRC);
                });
            }),
            content = || Box(modifier = Modifier.size(30.dp()).background_with(RED, RectangleShape)),
        );
    }));
    let mut list = DisplayList::new();
    let frame = |list: &mut DisplayList| {
        list.begin(Size::new(400.0, 800.0));
        rule.host().draw(list);
        list.end()
    };
    frame(&mut list);
    assert!(frame(&mut list).is_none());
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    let direct = rule.draw();
    assert!(
        replayed.rects.len() == 2 && replayed.rects == direct.rects && replayed.translations == direct.translations
    );
    assert!(replayed.rects[0].left == 30.0 && replayed.translation == Offset::Zero);
    by.set(50.0);
    rule.wait_for_idle();
    let d = frame(&mut list).unwrap(); // where it was and where it is
    assert!(d.left <= 30.0 && d.left >= 28.0 && d.right >= 100.0 && d.right <= 102.0, "{d:?}");
    let mut part = RecordingCanvas::new();
    list.replay(&mut part, Some(rect(300.0, 300.0, 10.0, 10.0))); // away from the box: nothing replayed
    assert!(part.rects.is_empty());
}

fn display_list_replays_paints_without_anti_aliasing() {
    // C++'s paints_turn_anti_aliasing_off, its display list half: what a paint draws not anti-aliased is recorded and
    // replayed so.
    let sharp = Paint { color: RED, is_anti_alias: false, ..Paint::default() };
    let mut list = DisplayList::new();
    list.begin(Size::new(100.0, 100.0));
    {
        let mut c = kompoz_ui::draw::Canvas::new(&mut list, Offset::Zero);
        let mut p = Path::new();
        p.add_rect(rect(0.0, 0.0, 5.0, 5.0), PathDirection::CounterClockwise);
        c.draw_path(&p, &sharp);
    }
    list.end();
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    assert!(replayed.stack == "Aa" && replayed.path_anti_aliased == [false], "{}", replayed.stack);
}

fn display_list_records_vertices() {
    // C++'s vertices_are_composes, its display list half: a display list records a mesh, replays it, and sees no
    // change in the same one drawn again.
    let mut list = DisplayList::new();
    let mut frame = |origin: Offset, corner: Color| {
        list.begin(Size::new(200.0, 200.0));
        let corners = [at(0.0, 0.0), at(20.0, 0.0), at(0.0, 20.0)];
        let mesh = Vertices::new(VertexMode::Triangles, &corners, &corners, &[corner, BLUE, RED], &[]);
        let area = Rect::from_offset_size(origin, Size::Zero);
        list.draw_vertices(&mesh, origin, BlendMode::Screen, &SolidColor(BLUE), area, 0.7);
        (list.end(), list.len())
    };
    frame(at(10.0, 10.0), RED);
    let (d, len) = frame(at(10.0, 10.0), RED);
    assert!(d.is_none() && len == 1);
    let d = frame(at(10.0, 10.0), BLUE).0.unwrap(); // the same place, another color: where it is
    assert!(d.left <= 10.0 && d.right >= 30.0);
    frame(at(10.0, 10.0), RED);
    let d = frame(at(50.0, 10.0), RED).0.unwrap();
    assert!(d.left <= 10.0 && d.right >= 70.0);
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    let m = &replayed.meshes;
    assert!(m.len() == 1 && m[0].origin == at(50.0, 10.0) && m[0].blend_mode == BlendMode::Screen && m[0].alpha == 0.7);
}

fn display_list_replays_draw_scopes_parameters() {
    // C++'s draw_scopes_take_composes_parameters, its display list half: images keep how they are sampled and
    // strokes their styles through a display list.
    let photo = ImageBitmap::from_pixels(8, 6, vec![200; 8 * 6 * 4]).unwrap();
    let images = KompozTestRule::new(kompozable_lambda!(move || {
        let photo = photo.clone();
        kompoz_foundation::Canvas(
            Modifier.size_wh(100.dp(), 100.dp()),
            on_draw = move |d: &mut DrawScope| {
                d.draw_image_with(&photo, at(5.0, 5.0), 0.5, Fill, None, SRC);
                let (src_at, src_size) = (IntOffset::new(2, 1), IntSize::new(4, 3));
                let (dst_at, dst_size) = (IntOffset::new(20, 20), IntSize::new(40, 30));
                let q = FilterQuality::None;
                d.draw_image_rect_with(&photo, src_at, src_size, dst_at, dst_size, 1.0, Fill, None, BlendMode::Src, q);
                d.draw_image_rect(&photo);
            },
        );
    }));
    let replayed_images = replayed(&images, Size::new(400.0, 800.0));
    let q = &replayed_images.image_qualities;
    assert!(q.len() == 3 && q[0] == FilterQuality::Low && q[1] == FilterQuality::None, "{q:?}");
    let lines = KompozTestRule::new(kompozable_lambda!(|| {
        kompoz_foundation::Canvas(
            Modifier.size_wh(100.dp(), 100.dp()),
            on_draw = |d: &mut DrawScope| {
                let r = hgrad(&[RED, BLUE]);
                d.draw_round_rect_with(
                    RED,
                    at(50.0, 0.0),
                    sz(10.0, 10.0),
                    CornerRadius::new(2.0, 4.0),
                    0.25,
                    Fill,
                    None,
                    SRC,
                );
                d.draw_rect_with(
                    RED,
                    at(60.0, 0.0),
                    sz(10.0, 10.0),
                    1.0,
                    DrawStyle::Stroke(Stroke::default()),
                    None,
                    SRC,
                );
                let bevel = DrawStyle::Stroke(Stroke::new(2.0, 10.0, StrokeCap::Square, StrokeJoin::Bevel, None));
                d.draw_rect_with(RED, at(70.0, 0.0), sz(10.0, 10.0), 1.0, bevel, None, SRC);
                d.draw_line_with(r, at(0.0, 20.0), at(40.0, 20.0), 3.0, StrokeCap::Butt, None, 0.5, None, SRC);
                d.draw_line(RED, at(0.0, 30.0), at(40.0, 30.0));
                d.draw_oval_with(BLUE, at(0.0, 50.0), sz(20.0, 10.0), 0.5, stroke(1.0), None, SRC);
                d.draw_points(&[at(80.0, 80.0), at(90.0, 90.0)], PointMode::Points, RED);
                d.draw_line_with(RED, at(0.0, 60.0), at(40.0, 60.0), 2.0, StrokeCap::Butt, None, 1.0, None, SRC);
            },
        );
    }));
    let again = replayed(&lines, Size::new(400.0, 800.0));
    let s = &again.path_stroke_styles;
    assert!(s.len() == 8 && s[1].hairline && s[2].miter == 10.0, "{s:?}");
    assert_eq!(again.path_stroke_styles, lines.draw().path_stroke_styles);
}

// ---- Transforms, clips, effects, shadows -----------------------------------------------------------------------

fn display_list_path_effects_are_part_of_what_is_drawn() {
    // C++'s display_list_path_effects_are_part_of_what_is_drawn: a path drawn the same but through another path
    // effect has changed: damage where it is.
    let mut list = DisplayList::new();
    let mut line = Path::new();
    line.move_to(10.0, 10.0);
    line.line_to(90.0, 10.0);
    let mut frame = |phase: f32| {
        list.begin(Size::new(200.0, 200.0));
        list.begin_node(1);
        let stroke = PathStroke {
            width: 2.0,
            path_effect: Some(PathEffect::dash_path_effect(&[10.0, 10.0], phase)),
            ..PathStroke::default()
        };
        list.draw_path(&line, Offset::Zero, &SolidColor(Color::Black), Rect::Zero, &stroke);
        list.end_node();
        list.end()
    };
    frame(0.0);
    assert!(frame(0.0).is_none());
    let d = frame(5.0).unwrap();
    assert!(d.left <= 10.0 && d.right >= 90.0);
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    let shapes = &replayed.path_shapes;
    assert!(shapes.len() == 1 && path_text(&shapes[0]).starts_with("M10,9 L15,9 L15,11 L10,11 M25,9"), "{shapes:?}");
}

fn display_list_follows_transforms() {
    // C++'s display_list_follows_transforms: what is drawn through a transform touches where the transform takes it;
    // a transform that changes is damage where its drawing was and where it is; replayed, it is pushed again.
    let list = RefCell::new(DisplayList::new());
    let frame = |m: &Matrix| {
        let mut list = list.borrow_mut();
        list.begin(Size::new(400.0, 400.0));
        list.begin_node(1);
        list.push_transform(m);
        list.fill_round_rect(rect(100.0, 100.0, 40.0, 20.0), CornerRadii::Zero, Color::Black);
        list.pop_transform();
        list.end_node();
        list.end()
    };
    // A quarter about the rect's middle: x 110..130, y 90..130.
    let mut turn = Matrix::new();
    turn.translate(120.0, 110.0, 0.0);
    turn.rotate_z(90.0);
    turn.translate(-120.0, -110.0, 0.0);
    frame(&turn);
    assert!(frame(&turn).is_none());
    // The same, 100 to the right.
    let mut moved = Matrix::new();
    moved.translate(220.0, 110.0, 0.0);
    moved.rotate_z(90.0);
    moved.translate(-120.0, -110.0, 0.0);
    let d = frame(&moved).unwrap();
    assert!(d.left <= 110.0 && d.left >= 106.0 && d.top <= 90.0 && d.top >= 86.0, "{d:?}");
    assert!(d.right >= 230.0 && d.right <= 234.0 && d.bottom >= 130.0 && d.bottom <= 134.0, "{d:?}");
    let mut replayed = RecordingCanvas::new();
    list.borrow().replay(&mut replayed, None);
    assert!(replayed.transforms == [moved] && replayed.transform_depth == 0 && replayed.rects.len() == 1);
    // Part of it behind the eye: it may touch anything (w = 1 - x / 125: below 0 right of x = 125).
    let mut behind = Matrix::new();
    behind.set(0, 3, -0.008);
    let d = frame(&behind).unwrap();
    assert!(d.left <= 0.0 && d.right >= 400.0 && d.top <= 0.0 && d.bottom >= 400.0);
    let mut list = list.borrow_mut();
    list.begin(Size::new(400.0, 400.0));
    assert!(list.quick_reject(rect(195.0, -55.0, 10.0, 10.0))); // above the screen
    // An eighth of a turn about the middle: (200, -50) goes to (377, 23).
    let mut eighth = Matrix::new();
    eighth.translate(200.0, 200.0, 0.0);
    eighth.rotate_z(45.0);
    eighth.translate(-200.0, -200.0, 0.0);
    list.push_transform(&eighth);
    assert!(!list.quick_reject(rect(195.0, -55.0, 10.0, 10.0)));
    assert!(list.quick_reject(rect(195.0, -400.0, 10.0, 10.0))); // (still out of view)
    list.pop_transform();
    list.end();
}

fn display_list_follows_clips_of_any_shape() {
    // C++'s display_list_follows_clips_of_any_shape: a clip inside its shape keeps what it clips from damaging
    // anything past the shape; one outside it doesn't; replayed, each is pushed again as it was.
    let list = RefCell::new(DisplayList::new());
    let mut tri = Path::new();
    tri.move_to(50.0, 0.0);
    tri.line_to(100.0, 80.0);
    tri.line_to(0.0, 80.0);
    tri.close();
    let frame = |color: Color, op: ClipOp, rounded: bool| {
        let mut list = list.borrow_mut();
        list.begin(Size::new(400.0, 400.0));
        list.begin_node(1);
        if rounded {
            list.push_clip_round_rect(rect(100.0, 100.0, 60.0, 40.0), CornerRadii::all(12.0), op);
        } else {
            list.push_clip_path(&tri, at(200.0, 200.0), op);
        }
        list.fill_round_rect(rect(0.0, 0.0, 400.0, 400.0), CornerRadii::Zero, color);
        list.pop_clip();
        list.end_node();
        list.end()
    };
    frame(RED, ClipOp::Intersect, false);
    let d = frame(BLUE, ClipOp::Intersect, false).unwrap(); // only where the triangle is (200..300, 200..280)
    assert!(d.left >= 196.0 && d.left <= 200.0 && d.right >= 300.0 && d.right <= 304.0 && d.bottom <= 284.0);
    frame(RED, ClipOp::Intersect, true);
    let d = frame(BLUE, ClipOp::Intersect, true).unwrap();
    assert!(d.left >= 96.0 && d.right <= 164.0 && d.top >= 96.0 && d.bottom <= 144.0);
    frame(RED, ClipOp::Difference, true);
    let d = frame(BLUE, ClipOp::Difference, true).unwrap(); // outside it: anywhere
    assert!(d.left <= 0.0 && d.right >= 400.0);
    let mut replayed = RecordingCanvas::new();
    list.borrow().replay(&mut replayed, None);
    let c = &replayed.clip_shapes;
    assert!(
        c.len() == 1 && c[0].op == ClipOp::Difference && c[0].path.is_none() && c[0].radii == CornerRadii::all(12.0)
    );
    frame(RED, ClipOp::Intersect, false);
    let mut with_path = RecordingCanvas::new();
    list.borrow().replay(&mut with_path, None);
    let c = &with_path.clip_shapes;
    assert!(
        c.len() == 1 && c[0].path.as_ref() == Some(&tri) && c[0].op == ClipOp::Intersect && c[0].bounds.left == 200.0
    );
}

fn display_list_damage_reaches_the_shadows_blur() {
    // C++'s shadow_default_draws_fading_rings_and_display_list_damage_reaches_the_blur, its display list part (the
    // rings are graphics_context's).
    let mut list = DisplayList::new();
    let shadow = Shadow::new(Color::Black, at(0.0, 10.0), 5.0, 2.0);
    list.begin(Size::new(400.0, 400.0));
    list.draw_shadow(rect(100.0, 100.0, 40.0, 40.0), CornerRadii::Zero, &shadow, false);
    list.end();
    list.begin(Size::new(400.0, 400.0));
    let d = list.end().unwrap(); // gone: its area is the damage
    assert!(d.left <= 100.0 - 2.0 - 15.0 && d.bottom >= 140.0 + 2.0 + 10.0 + 15.0 && d.right >= 140.0 + 2.0 + 15.0);
    list.begin(Size::new(400.0, 400.0));
    list.draw_shadow(rect(100.0, 100.0, 40.0, 40.0), CornerRadii::Zero, &shadow, true);
    list.end();
    list.begin(Size::new(400.0, 400.0));
    let d = list.end().unwrap(); // an inner one stays inside its rect
    assert!(d.left >= 100.0 - 3.0 && d.right <= 140.0 + 3.0 && d.bottom <= 140.0 + 3.0);
}

fn display_list_damage_reaches_what_effects_and_spreads_draw() {
    // C++'s display_list_damage_reaches_what_effects_and_spreads_draw: what changed is drawn again as far as drawing
    // it reaches: a path's stamps (a path effect) past the path, and a drop shadow's spread out to its miter at a
    // star's points.
    let mut list = DisplayList::new();
    let mut line = Path::new();
    line.move_to(20.0, 100.0);
    line.line_to(380.0, 100.0);
    let mut stamp = Path::new(); // across the line, 30 up and down
    stamp.add_rect(rect(-2.0, -30.0, 4.0, 60.0), PathDirection::CounterClockwise);
    let stamped = |phase: f32| PathStroke {
        path_effect: Some(PathEffect::stamped_path_effect(&stamp, 40.0, phase, StampedPathEffectStyle::Rotate)),
        ..PathStroke::new(1.0)
    };
    let drawn = |phase: f32| {
        // where the stamps are at `phase`
        let mut s = stamped(phase);
        let mut effected = Path::new();
        let effect = s.path_effect.clone().unwrap();
        effect.filter_path(&mut effected, &line, &mut s, 1.0);
        effected.get_bounds()
    };
    let black = SolidColor(Color::Black);
    list.begin(Size::new(400.0, 400.0));
    list.draw_path(&line, Offset::Zero, &black, Rect::Zero, &stamped(0.0));
    list.end();
    assert!(stamped(0.0).path_effect.unwrap().reach() >= 30.0);
    assert_eq!(PathEffect::dash_path_effect(&[4.0, 4.0], 0.0).reach(), 0.0);
    list.begin(Size::new(400.0, 400.0));
    list.draw_path(&line, Offset::Zero, &black, Rect::Zero, &stamped(15.0));
    let d = list.end().unwrap();
    assert!(covers(d, drawn(0.0)) && covers(d, drawn(15.0)) && drawn(15.0).top < 75.0);
    // A drop shadow spread round a star: its points' miters reach past the spread.
    let mut star = Path::new();
    for i in 0..10 {
        let a = i as f32 * std::f32::consts::PI / 5.0;
        let r = if i % 2 == 1 { 12.0 } else { 40.0 };
        let (x, y) = (200.0 + r * a.sin(), 250.0 - r * a.cos());
        if i == 0 { star.move_to(x, y) } else { star.line_to(x, y) }
    }
    star.close();
    let spread = Shadow::new(Color::new(0.0, 0.0, 0.0, 0.5), Offset::Zero, 0.0, 6.0);
    list.begin(Size::new(400.0, 400.0));
    list.end();
    list.begin(Size::new(400.0, 400.0));
    list.draw_shadow_path(&star, Offset::Zero, &spread, false);
    let d = list.end().unwrap();
    let reach = shadow_path_reach(star.get_bounds(), &spread, false);
    assert!(covers(d, reach) && covers(d, rect(200.0 - 40.0 * 0.96 - 6.0, 210.0 - 6.0, 80.0, 10.0)));
}

fn display_list_backdrops_are_drawn_anew_with_what_is_behind() {
    // C++'s display_list_backdrops_are_drawn_anew_with_what_is_behind: a glass at 120 reading what is behind it
    // blurred (a Morph blur: a backdrop).
    let (behind, over, far) = (mutable_state_of(0.0f32), mutable_state_of(0.0f32), mutable_state_of(0.0f32));
    let (b, o, f) = (behind.clone(), over.clone(), far.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (b, o, f) = (b.clone(), o.clone(), f.clone());
            Box(
                Modifier.fill_max_size(),
                content = move || {
                    // Near the glass: what it reads; far from it.
                    Box(moved(Modifier, 100, 100)
                        .size(10.dp())
                        .background_with(Color::new(b.get(), 0.0, 1.0, 1.0), RectangleShape));
                    Box(moved(Modifier, 300, 300)
                        .size(10.dp())
                        .background_with(Color::new(f.get(), 0.0, 1.0, 1.0), RectangleShape));
                    let o = o.clone();
                    let glass = moved(Modifier, 120, 120).size(100.dp()).blur_xy_with(
                        10.dp(),
                        10.dp(),
                        BlurredEdgeTreatment::Rectangle,
                        BlurType::Morph,
                    );
                    Box(
                        glass,
                        content = move || {
                            // On the glass.
                            Box(Modifier
                                .size(10.dp())
                                .background_with(Color::new(o.get(), 1.0, 0.0, 1.0), RectangleShape));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 400),
    );
    let mut list = DisplayList::new();
    let mut frame = || {
        list.begin(Size::new(400.0, 400.0));
        rule.host().draw(&mut list);
        list.end()
    };
    frame();
    // What it reads: from three sigmas past it; it shows within it (clipped).
    let reach = 3.0 * (0.57735 * 10.0 + 0.5);
    let whole = |d: Option<Rect>| {
        d.is_some_and(|d| {
            d.left <= 120.0 - reach && d.top <= 120.0 - reach && d.right >= 220.0 + reach && d.bottom >= 220.0 + reach
        })
    };
    behind.set(1.0); // changed behind it, outside it: it changes, and all it reads is drawn anew
    rule.wait_for_idle();
    assert!(whole(frame()));
    over.set(1.0); // on it: the same
    rule.wait_for_idle();
    assert!(whole(frame()));
    far.set(1.0); // nowhere near: only that
    rule.wait_for_idle();
    let d = frame().unwrap();
    assert!(d.left >= 290.0 && d.top >= 290.0, "{d:?}");
    // Replayed where something behind it changed: it too.
    behind.set(0.5);
    rule.wait_for_idle();
    let d = frame().unwrap();
    let mut c = RecordingCanvas::new();
    list.replay(&mut c, Some(d));
    assert!(
        c.backdrops.len() == 1 && same_rect(c.backdrops[0].bounds, rect(120.0, 120.0, 100.0, 100.0)),
        "{:?}",
        c.backdrops
    );
}

fn display_list_effect_layers_change_as_a_whole() {
    // C++'s display_list_effect_layers_change_as_a_whole: a blurred node partly off the screen.
    let tint = mutable_state_of(0.0f32);
    let t = tint.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.clone();
            let blurred = moved(Modifier, -50, 100).size(100.dp()).blur_xy_with(
                10.dp(),
                10.dp(),
                BlurredEdgeTreatment::Unbounded,
                BlurType::Family,
            );
            Box(
                blurred,
                content = move || {
                    // Off the screen: still blurred into it.
                    Box(Modifier.size(10.dp()).background_with(Color::new(t.get(), 0.0, 1.0, 1.0), RectangleShape));
                    Box(moved(Modifier, 60, 60).size(10.dp()).background_with(GREEN, RectangleShape));
                },
            );
        }),
        IntSize::new(400, 400),
    );
    let mut list = DisplayList::new();
    let mut frame = || {
        list.begin(Size::new(400.0, 400.0));
        rule.host().draw(&mut list);
        list.end()
    };
    frame();
    tint.set(1.0);
    rule.wait_for_idle();
    let d = frame().unwrap();
    // A change off the screen: all of what the blur spreads it over on the screen changes (three sigmas past the
    // bounds, and two pixels).
    let reach = 3.0 * (0.57735 * 10.0 + 0.5) + 2.0;
    assert!(d.left == 0.0 && near(d.top, 100.0 - reach, 1.01) && near(d.right, 50.0 + reach, 1.01), "{d:?}");
    assert!(near(d.bottom, 200.0 + reach, 1.01), "{d:?}");
    // Replaying a part of it replays all that is in it, what is off the screen too.
    let mut part = RecordingCanvas::new();
    list.replay(&mut part, Some(rect(40.0, 120.0, 4.0, 4.0)));
    assert!(part.layers == 1 && part.rects.len() == 2, "{} {:?}", part.layers, part.rects);
}

fn display_list_replays_blur_layers() {
    // C++'s blur_is_a_graphics_layer_with_a_blur_effect, its display list half: recorded and replayed the same way.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let blurred = Modifier.padding(50.dp()).size(100.dp()).blur_xy_with(
            8.dp(),
            4.dp(),
            BlurredEdgeTreatment::Unbounded,
            BlurType::Family,
        );
        Box(blurred.background_with(Color::Black, RectangleShape));
    }));
    let c = replayed(&rule, Size::new(400.0, 800.0));
    assert!(c.layer_effects.len() == 1 && c.layer_effects[0] == Some(BlurEffect(8.0, 4.0, TileMode::Decal)));
    assert!(same_rect(c.layer_bounds[0], rect(50.0, 50.0, 100.0, 100.0)) && c.clips.is_empty());
}

fn display_list_replays_blend_modes_in_an_offscreen_layer() {
    // C++'s blend_modes_fade_an_offscreen_layer, its display list half: the content, the mask DstIn, the punched
    // corner and hole recorded and replayed so, in the layer of their own.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier
                .size_wh(100.dp(), 200.dp())
                .graphics_layer_block(|l| l.compositing_strategy = CompositingStrategy::Offscreen)
                .draw_with_content(|d| {
                    d.draw_content();
                    let fade = vgrad(&[Color::Black, Color::Transparent]);
                    d.draw_rect_with(fade, Offset::Zero, Size::Unspecified, 1.0, Fill, None, BlendMode::DstIn);
                    let mut corner = Path::new();
                    corner.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), PathDirection::CounterClockwise);
                    d.draw_path_with(&corner, Color::Black, 1.0, Fill, None, BlendMode::Clear);
                    d.draw_circle_with(Color::Black, 5.0, at(50.0, 50.0), 1.0, Fill, None, BlendMode::DstOut);
                }),
            content = || Box(modifier = Modifier.fill_max_size().background_with(Color::hex(0x6750A4), RectangleShape)),
        );
    }));
    let c = replayed(&rule, Size::new(400.0, 800.0));
    assert!(c.layers == 1 && c.rect_blends == [BlendMode::SrcOver, BlendMode::DstIn, BlendMode::DstOut]);
    assert!(c.path_blends == [BlendMode::Clear] && c.blend_stack.is_empty());
}

fn display_list_replays_drop_and_inner_shadows() {
    // C++'s drop_and_inner_shadows_of_shapes, its display list half: recorded and replayed.
    let soft = Shadow::new(Color::new(0.0, 0.0, 0.0, 0.4), at(0.0, 6.0), 10.0, 2.0);
    let s = soft.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(modifier = Modifier.padding(20.dp()), move || {
            let r12 = RoundedCornerShape(12.0f32);
            let drop = Modifier.size_wh(80.dp(), 40.dp()).drop_shadow(r12.clone(), s.clone());
            Box(modifier = drop.background_with(RED, r12));
            let hard = Shadow::new(Color::Black, at(1.0, 2.0), 4.0, 0.0);
            let inner = Modifier
                .size_wh(80.dp(), 40.dp())
                .background_with(GREEN, RectangleShape)
                .inner_shadow(CircleShape, hard);
            Box(
                modifier = inner,
                content = || Box(modifier = Modifier.size(10.dp()).background_with(BLUE, RectangleShape)),
            );
            Box(modifier = Modifier.size(30.dp()).drop_shadow(MaterialShapes::Heart().to_shape(0), s.clone()));
        });
    }));
    let direct = rule.draw();
    let c = replayed(&rule, Size::new(400.0, 800.0));
    assert!(c.shadows.len() == 2 && c.shadows[0].shadow == soft && c.shadows[1].inner);
    assert!(c.path_shadows.len() == 1 && c.path_shadows[0].shadow == soft);
    assert!(same_rect(c.path_shadows[0].bounds, direct.path_shadows[0].bounds));
}

fn display_list_keeps_brushed_shadows_whole() {
    // C++'s drop_shadows_take_brushes_alphas_and_blend_modes, its display list half: a display list keeps a brushed
    // shadow whole, for a canvas to draw its own way, replayed onto one without as Compose draws it; a new brush
    // damages as far as its bitmap.
    let tint = mutable_state_of(RED);
    let t = tint.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let brush = Brush::linear_gradient(&[t.get(), BLUE], Offset::Zero, Offset::Infinite, TileMode::Clamp);
            let shadow = Shadow { offset: at(3.0, 4.0), blur_radius: 6.0, brush: Some(brush), ..Shadow::default() };
            Box(modifier = moved(Modifier, 20, 20).size(40.dp()).drop_shadow(RectangleShape, shadow));
        }),
        IntSize::new(400, 400),
    );
    let mut list = DisplayList::new();
    list.begin(Size::new(400.0, 400.0));
    rule.host().draw(&mut list);
    list.end();
    let mut own = RecordingCanvas::new();
    list.replay(&mut own, None);
    assert!(own.brushed_shadows.len() == 1 && same_rect(own.brushed_shadows[0], rect(20.0, 20.0, 40.0, 40.0)));
    assert!(own.layers == 1 && own.order == "sr" && own.rect_blends == [BlendMode::SrcIn], "{}", own.order);
    tint.set(GREEN);
    rule.wait_for_idle();
    list.begin(Size::new(400.0, 400.0));
    rule.host().draw(&mut list);
    let d = list.end().unwrap();
    let bitmap = rect(20.0 - 6.0 + 3.0, 20.0 - 6.0 + 4.0, 52.0, 52.0);
    assert!(covers(d, bitmap) && d.width() <= 56.0 && d.height() <= 56.0, "{d:?}");
}

fn display_list_elevation_shadows_reach_from_the_light() {
    // C++'s elevation_shadows_blur_as_skia_where_nothing_does_better, its display list part: the light's center
    // where the transform takes it; the shadow's reach its area; raised, all it reaches changes.
    let up = mutable_state_of(8.0f32);
    let u = up.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let u = u.clone();
            Box(modifier =
                moved(Modifier, 50, 60).size(40.dp()).graphics_layer_block(move |l| l.shadow_elevation = u.get()));
        }),
        IntSize::new(400, 400),
    );
    let mut list = DisplayList::new();
    list.set_light(ShadowLight { center: at(200.0, 0.0), ..ShadowLight::default() });
    list.begin(Size::new(400.0, 400.0));
    list.push_translation(at(10.0, 20.0));
    let center = list.light_center();
    assert!(near(center.x, 190.0, 1e-4) && near(center.y, -20.0, 1e-4));
    list.pop_translation();
    rule.host().draw(&mut list);
    list.end();
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    assert!(replayed.elevation_shadows.len() == 1 && near(replayed.elevation_shadows[0].elevation(), 8.0, 1e-6));
    let light = list.light();
    let reach = ElevationShadow::flat(8.0).reach(rect(50.0, 60.0, 40.0, 40.0), &light, light.center);
    assert!(reach.left < 50.0 && reach.right > 90.0 && reach.bottom > 100.0);
    // Raised: all it reaches changes.
    up.set(12.0);
    rule.wait_for_idle();
    list.begin(Size::new(400.0, 400.0));
    rule.host().draw(&mut list);
    let d = list.end().unwrap();
    let higher = ElevationShadow::flat(12.0).reach(rect(50.0, 60.0, 40.0, 40.0), &light, light.center);
    assert!(covers(d, higher), "{d:?} {higher:?}");
}

// ---- The keep check --------------------------------------------------------------------------------------------

fn kept_nodes_draw_what_their_images_hold_now() {
    // C++'s kept_nodes_draw_what_their_images_hold_now: a bitmap drawn into (a canvas over it) changes what a node
    // drawing it draws, though the node doesn't change: kept, it records what the image holds now (the keep check
    // finds nothing), and a list sees the change.
    let bitmap = ImageBitmap::from_pixels(4, 4, vec![255; 64]).unwrap();
    let b = bitmap.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        BitmapImage(b.clone(), None, modifier = Modifier.size(40.dp()));
    }));
    let mut list = DisplayList::new();
    list.set_keep_nodes(true);
    let mut frame = || {
        list.begin(Size::new(400.0, 800.0));
        rule.host().draw(&mut list);
        (list.end(), list.kept_nodes())
    };
    frame();
    assert!(matches!(frame(), (None, 1)));
    let version = bitmap.version();
    bitmap.store(&[128; 64], true); // drawn into (a Canvas over an ImageBitmap's)
    rule.advance_frame(); // (the keep check: nothing)
    assert_ne!(bitmap.version(), version);
    let (d, kept) = frame();
    assert!(kept == 1 && d.is_some_and(|d| covers(d, rect(0.0, 0.0, 40.0, 40.0))), "{d:?}");
}

fn keep_check_catches_a_stale_node() {
    // The host's keep check (on in the rule) draws each frame keeping the nodes that didn't change and drawing them
    // all: a node that changed without saying so (drawing from what isn't observed) is kept stale, and the check
    // fails the frame.
    let color = Rc::new(Cell::new(RED));
    let c = color.clone();
    let sneaky = move |c: Rc<Cell<Color>>| Modifier.size(20.dp()).draw_behind(move |d| d.draw_rect(c.get()));
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Column(Modifier, move || {
                Box(sneaky(c.clone()));
                Text("yanında");
            });
        }),
        IntSize::new(100, 100),
    );
    rule.advance_frame(); // the same: kept, which is what drawing it records
    color.set(BLUE); // not observed: nothing drawn again
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rule.advance_frame()));
    let message = caught.expect_err("the keep check fails the frame");
    let message = message.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("keep check") && message.contains("color"), "{message}");
    // Off, nothing is checked.
    rule.set_keep_check(false);
    color.set(GREEN);
    rule.advance_frame();
    // A host on its own tells what it found.
    let c = color.clone();
    let host = KompozHost::new(kompozable_lambda!(move || Box(sneaky(c.clone()))));
    host.set_viewport_size(IntSize::new(100, 100));
    host.set_keep_check(true);
    host.frame(16_000_000);
    host.frame(32_000_000);
    assert!(host.take_keep_check_mismatches().is_empty());
    color.set(RED);
    host.frame(48_000_000);
    let found = host.take_keep_check_mismatches();
    assert!(found.len() == 1 && found[0].contains("color"), "{found:?}");
}

tests!(
    display_list_damage_is_what_changed_on_the_screen,
    display_list_damage_comes_in_separate_areas,
    display_list_damage_of_a_changed_node_only,
    display_list_damage_where_a_bitmap_was_drawn_into,
    display_list_moves_what_scrolled_by_whole_pixels,
    display_list_tells_canvases_which_layers_stayed_the_same,
    display_list_keeps_a_layers_content_whatever_its_transform,
    display_list_lines_frames_up_by_node_and_replays_an_area,
    display_list_keeps_what_unchanged_nodes_drew,
    display_list_kept_nodes_recheck_quick_rejects_and_culls,
    display_list_keeps_a_node_moved_again_and_again,
    display_list_keeps_unchanged_nodes_of_a_host,
    display_list_leaves_out_what_an_opaque_rect_hides,
    display_list_replays_every_kind_as_drawn,
    display_list_translations_are_replayed,
    display_list_replays_paints_without_anti_aliasing,
    display_list_records_vertices,
    display_list_replays_draw_scopes_parameters,
    display_list_path_effects_are_part_of_what_is_drawn,
    display_list_follows_transforms,
    display_list_follows_clips_of_any_shape,
    display_list_damage_reaches_the_shadows_blur,
    display_list_damage_reaches_what_effects_and_spreads_draw,
    display_list_backdrops_are_drawn_anew_with_what_is_behind,
    display_list_effect_layers_change_as_a_whole,
    display_list_replays_blur_layers,
    display_list_replays_blend_modes_in_an_offscreen_layer,
    display_list_replays_drop_and_inner_shadows,
    display_list_keeps_brushed_shadows_whole,
    display_list_elevation_shadows_reach_from_the_light,
    kept_nodes_draw_what_their_images_hold_now,
    keep_check_catches_a_stale_node,
);
