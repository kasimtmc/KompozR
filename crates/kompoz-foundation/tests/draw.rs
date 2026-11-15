//! foundation's drawing: `background` and `border` as Compose's (outlines, borders worked out for a size: strokes,
//! rings, bitmaps of generic outlines), `Canvas`, and `Image`'s semantics.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draw::{BitmapPainter, Painter, pending_draws};
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn same_rect(r: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
    near(r.left, x) && near(r.top, y) && near(r.width(), w) && near(r.height(), h)
}

fn backgrounds_fill_their_shapes_as_composes() {
    // A rectangle as a rect; a rounded rect of one radius as a rounded rect; of several as its path; a brush faded.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(modifier = Modifier.size(20.dp()).background(RED));
            Box(modifier = Modifier.size(20.dp()).background(RED, shape = Arc::new(RoundedCornerShape(4.0f32))));
            let corners = RoundedCornerShape::new(2.0f32, 4.0f32, 6.0f32, 8.0f32);
            Box(modifier = Modifier.size(20.dp()).background_with(BLUE, corners));
            let ramp = Brush::horizontal_gradient(&[RED, BLUE], 0.0, f32::INFINITY, TileMode::Clamp);
            Box(modifier = Modifier.size(20.dp()).background_brush(ramp, alpha = 0.5));
        });
    }));
    let c = rule.draw();
    assert!(c.rects.len() == 3 && c.paths.len() == 1 && c.gradients == 1);
    assert!(same_rect(c.rects[0], 0.0, 0.0, 20.0, 20.0) && c.rect_radii[0] == 0.0 && c.rect_colors[0] == RED);
    assert!(same_rect(c.rects[1], 0.0, 20.0, 20.0, 20.0) && near(c.rect_radii[1], 4.0));
    assert!(same_rect(c.paths[0], 0.0, 40.0, 20.0, 20.0) && c.path_colors[0] == BLUE);
    assert!((c.rect_colors[2].alpha() - 0.5).abs() < 0.01); // faded
    // Behind what comes after it.
    assert_eq!(c.order, "rrpr");
}

fn borders_are_worked_out_for_a_size() {
    // A rect stroked half its width inside; one too wide filled whole; a rounded rect of corners smaller than half
    // the width drawn outside a clip of its inside; one of several radii a ring of paths; a hairline a px.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(modifier = Modifier.size(20.dp()).border(2.dp(), RED));
            Box(modifier = Modifier.size(19.dp()).border(15.dp(), RED)); // kept to 10 px: more than half, filled
            Box(modifier = Modifier.size(20.dp()).border_with(4.dp(), RED, RoundedCornerShape(1.0f32)));
            Box(modifier = Modifier.size(20.dp()).border_with(
                2.dp(),
                BLUE,
                RoundedCornerShape::new(2.0f32, 4.0f32, 6.0f32, 8.0f32),
            ));
            Box(modifier = Modifier.size(20.dp()).border(Dp::Hairline, RED));
        });
    }));
    let c = rule.draw();
    // The stroke along the rect 1 in (backends stroke half each side): the bounds.
    assert!(c.strokes.len() == 2 && same_rect(c.strokes[0], 0.0, 0.0, 20.0, 20.0));
    assert!(same_rect(c.rects[0], 0.0, 20.0, 19.0, 19.0)); // filled
    assert!(c.clip_shapes.len() == 1 && same_rect(c.clip_shapes[0].bounds, 4.0, 43.0, 12.0, 12.0));
    assert!(same_rect(c.rects[1], 0.0, 39.0, 20.0, 20.0) && near(c.rect_radii[1], 1.0));
    assert!(
        c.paths.len() == 1 && same_rect(c.paths[0], 0.0, 59.0, 20.0, 20.0) && c.path_shapes[0].commands().len() > 8
    );
    // A hairline: a 1 px stroke.
    assert!(same_rect(c.strokes[1], 0.0, 79.0, 20.0, 20.0));
}

fn generic_borders_are_drawn_into_a_bitmap() {
    // As Compose's: a generic outline's ring drawn into a bitmap of its bounds (an Alpha8 one tinted for a color),
    // the outline stroked twice the width, what is outside it cleared; drawn again from the cache.
    let triangle = GenericShape(|p, size, _| {
        p.move_to(size.width / 2.0, 0.0);
        p.line_to(size.width, size.height);
        p.line_to(0.0, size.height);
        p.close();
    });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(modifier = Modifier.size(40.dp()).border_with(3.dp(), RED, triangle.clone()));
    }));
    let c = rule.draw();
    assert!(c.images.len() == 1 && c.mask_colors == [RED] && same_rect(c.image_destinations[0], 0.0, 0.0, 40.0, 40.0));
    let bitmap = c.drawn_images[0].clone();
    assert!(bitmap.config() == ImageBitmapConfig::Alpha8 && bitmap.width() == 40);
    let mut drawn = RecordingCanvas::new();
    for draws in pending_draws(&bitmap) {
        draws.replay(&mut drawn);
    }
    assert!(drawn.rect_blends == [BlendMode::Clear] && drawn.path_strokes == [6.0, 0.0]);
    assert!(drawn.path_blends == [BlendMode::SrcOver, BlendMode::Clear]);
    // Drawn again: the same bitmap.
    assert_eq!(rule.draw().drawn_images[0], bitmap);
}

fn canvases_and_images_say_what_they_show() {
    // Canvas and Image: their content descriptions; an Image without one decorative.
    let photo = ImageBitmap::from_pixels(2, 2, vec![255; 16]).unwrap();
    let drawn = Rc::new(Cell::new(false));
    let d = drawn.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (photo, d) = (photo.clone(), d.clone());
        Column(|| {
            Canvas(Modifier.size(10.dp()), content_description = Some("çizim".into()), on_draw = move |_| d.set(true));
            BitmapImage(photo.clone(), Some("foto".into()), modifier = Modifier.size(10.dp()));
            let painter: Rc<dyn Painter> = Rc::new(BitmapPainter::with_quality(photo.clone(), FilterQuality::None));
            Image(painter, None, modifier = Modifier.size(10.dp()).test_tag("süs"));
        });
    }));
    assert!(
        rule.on_node_with_content_description("çizim").exists()
            && rule.on_node_with_content_description("foto").exists()
    );
    let decorative = rule.on_node_with_tag("süs").fetch_semantics_node();
    assert!(decorative.config().get(kompoz_ui::semantics::SemanticsProperties::ContentDescription).is_none());
    let c = rule.draw();
    assert!(drawn.get() && c.images.len() == 2 && c.image_qualities[1] == FilterQuality::None);
}

fn down(rule: &KompozTestRule, x: f32, y: f32) {
    rule.host().pointer_down(Offset::new(x, y), -1, 0, kompoz_ui::input::PointerType::Touch);
}

fn up(rule: &KompozTestRule, x: f32, y: f32) {
    rule.host().pointer_up(Offset::new(x, y), -1, 0);
}

fn corners(r: &kompoz_ui::graphics_context::CornerRadii) -> [f32; 4] {
    [r.top_left, r.top_right, r.bottom_right, r.bottom_left]
}

fn same_corners(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
}

/// What the host draws now, through a display list (as a screen gets it), and whether the frame changed anything.
fn through_display_list(rule: &KompozTestRule, list: &mut DisplayList) -> (RecordingCanvas, bool) {
    list.begin(Size::new(400.0, 800.0));
    rule.host().draw(list);
    let changed = list.end().is_some();
    let mut drawn = RecordingCanvas::new();
    list.replay(&mut drawn, None);
    (drawn, changed)
}

/// Whether `canvas` drew the debug indication's press: black at 0.3 over `bounds`.
fn pressed_over(canvas: &RecordingCanvas, bounds: Rect) -> bool {
    canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| *r == bounds && c.red() == 0.0 && near(c.alpha(), 0.3))
}

fn shapes_with_corners_of_their_own_reach_the_canvas_mirrored_in_rtl() {
    // The canvas part of C++'s shapes_with_corners_of_their_own_reach_the_canvas_mirrored_in_rtl (the radii
    // themselves are kompoz-graphics' corners_of_their_own_fit_and_mirror_in_rtl), the debug indication for its
    // ripple: a shadow, background, border and clip of one shape reach the canvas through a display list with its
    // corners, mirrored right to left (corners of their own are a path's to fill and stroke, as Compose's outline
    // drawing: C++ had rounded rects); a press shows inside the clip.
    let tab = RoundedCornerShape::new(16.0, 8.0, 0.0, 4.0);
    let rtl = mutable_state_of(false);
    let (r, t) = (rtl.clone(), tab.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let t = t.clone();
        KompozitionLocalProvider(&[kompoz_ui::platform::LocalLayoutDirection.provides(direction)], move || {
            Box(modifier = Modifier
                .size_wh(100.dp(), 40.dp())
                .shadow(2.dp(), shape = Arc::new(t.clone()))
                .background(RED, shape = Arc::new(t.clone()))
                .border(1.dp(), Color::Black, shape = Arc::new(t.clone()))
                .clip(t.clone())
                .clickable(|| {}));
        });
    }));
    let box_bounds = Rect::from_offset_size(Offset::Zero, Size::new(100.0, 40.0));
    let mut list = DisplayList::new();
    let (drawn, _) = through_display_list(&rule, &mut list);
    let check = |drawn: &RecordingCanvas, expected: [f32; 4]| {
        // The shadow's outline, the background's and the border's paths, the shadow's clip and the clip.
        assert_eq!(drawn.elevation_shadows.len(), 1);
        let shadow = &drawn.elevation_shadows[0];
        assert!(matches!(shadow.outline, Outline::Rounded(_)) && same_rect(shadow.bounds, 0.0, 0.0, 100.0, 40.0));
        let fill = drawn.path_colors.iter().zip(&drawn.path_strokes).position(|(c, s)| *c == RED && *s == 0.0);
        assert!(fill.is_some_and(|i| same_rect(drawn.paths[i], 0.0, 0.0, 100.0, 40.0)));
        assert!(drawn.path_colors.contains(&Color::Black)); // (a ring of corners of their own: a path filled)
        assert!(drawn.rect_corners.is_empty() && drawn.clip_shapes.len() == 2, "{:?}", drawn.clip_shapes);
        assert!(drawn.clip_shapes.iter().all(|k| same_corners(corners(&k.radii), expected)), "{:?}", drawn.clip_shapes);
    };
    check(&drawn, [16.0, 8.0, 0.0, 4.0]);
    down(&rule, 50.0, 20.0);
    rule.advance_frame();
    rule.advance_frame();
    let pressed = rule.draw();
    assert!(pressed.clip_shapes.len() == 2 && pressed_over(&pressed, box_bounds)); // the press in the clip
    up(&rule, 50.0, 20.0);
    rule.wait_for_idle();
    through_display_list(&rule, &mut list);
    rtl.set(true);
    rule.wait_for_idle();
    let (mirrored, changed) = through_display_list(&rule, &mut list);
    assert!(changed); // only the corners changed, and that is a change
    check(&mirrored, [8.0, 16.0, 4.0, 0.0]);

    // A graphics layer's clip, and a source's press drawn by `indication` (it follows presses as it draws).
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let other = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, tab) = (s.clone(), tab.clone());
        KompozitionLocalProvider(
            &[kompoz_ui::platform::LocalLayoutDirection.provides(LayoutDirection::Rtl)],
            move || {
                let (s, tab) = (s.clone(), tab.clone());
                Column(move || {
                    Box(modifier = Modifier
                        .size_wh(100.dp(), 40.dp())
                        .graphics_layer_block({
                            let tab = tab.clone();
                            move |l| {
                                l.alpha = 0.5;
                                l.shape = Arc::new(tab.clone());
                                l.clip = true;
                            }
                        })
                        .background(BLUE));
                    let debug: Rc<dyn Indication> = Rc::new(DefaultDebugIndication);
                    Box(modifier = Modifier
                        .size_wh(100.dp(), 40.dp())
                        .clickable_indication(Some(s.clone()), None, on_click = || {})
                        .background(Color::new(0.0, 1.0, 0.0, 1.0), shape = Arc::new(tab.clone()))
                        .indication(&s, Some(debug)));
                });
            },
        );
    }));
    down(&other, 50.0, 60.0);
    let mut layered = RecordingCanvas::new();
    for _ in 0..3 {
        other.advance_frame();
        layered = other.draw();
    }
    assert_eq!(layered.layer_corners.len(), 1);
    assert!(layered.layer_corners.iter().all(|k| same_corners(corners(k), [8.0, 16.0, 4.0, 0.0])));
    assert!(pressed_over(&layered, Rect::from_offset_size(Offset::new(0.0, 40.0), Size::new(100.0, 40.0))));
    up(&other, 50.0, 60.0);
}

fn outline_shapes_fill_stroke_clip_and_shadow() {
    // C++'s outline_shapes_fill_stroke_clip_shadow_and_ripple with the debug indication for its ripple: an outline
    // filled, shadowed and clipped to (a shape's clip and a graphics layer's, path clips); a press on it and a
    // source's press drawn by `indication`; a display list replays path clips; right to left, mirrored. (A generic
    // outline's border is drawn into a bitmap, as Compose's: C++ stroked its path.)
    fn triangle() -> GenericShape {
        // pointing up
        GenericShape(|p, s, _| {
            p.move_to(s.width / 2.0, 0.0);
            p.line_to(s.width, s.height);
            p.line_to(0.0, s.height);
        })
    }
    fn arrow() -> GenericShape {
        // pointing to the end
        GenericShape(|p, s, d| {
            let (start, end) = if d == LayoutDirection::Rtl { (s.width, 0.0) } else { (0.0, s.width) };
            p.move_to(start, 0.0);
            p.line_to(end, s.height / 2.0);
            p.line_to(start, s.height);
        })
    }
    let (tri, arr) = (triangle(), arrow());
    let rtl = mutable_state_of(false);
    let taps = Rc::new(Cell::new(0));
    let source = MutableInteractionSource::new();
    let (r, t, s) = (rtl.clone(), taps.clone(), source.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let (tri, arr, t, s) = (tri.clone(), arr.clone(), t.clone(), s.clone());
        KompozitionLocalProvider(&[kompoz_ui::platform::LocalLayoutDirection.provides(direction)], move || {
            let (tri, arr, t, s) = (tri.clone(), arr.clone(), t.clone(), s.clone());
            Column(
                modifier = Modifier.padding(20.dp()),
                content = move || {
                    Box(modifier = Modifier
                        .size_wh(100.dp(), 50.dp())
                        .shadow(4.dp(), shape = Arc::new(tri.clone()), clip = Some(false))
                        .background(RED, shape = Arc::new(tri.clone()))
                        .test_tag("üçgen"));
                    Box(modifier = Modifier.size(60.dp()).clip(arr.clone()).background(Color::new(0.0, 1.0, 0.0, 1.0)));
                    Box(modifier = Modifier
                        .size(80.dp())
                        .graphics_layer_block({
                            let arr = arr.clone();
                            move |l| {
                                l.clip = true;
                                l.shape = Arc::new(arr.clone());
                            }
                        })
                        .background(Color::Black));
                    let t = t.clone();
                    Box(modifier = Modifier
                        .size(70.dp())
                        .background(Color::new(1.0, 1.0, 0.0, 1.0), shape = Arc::new(tri.clone()))
                        .clickable(move || t.set(t.get() + 1))
                        .test_tag("düğme"));
                    let debug: Rc<dyn Indication> = Rc::new(DefaultDebugIndication);
                    Box(modifier = Modifier
                        .size(50.dp())
                        .background(Color::new(1.0, 0.0, 1.0, 1.0), shape = Arc::new(tri.clone()))
                        .indication(&s, Some(debug))
                        .test_tag("kendi"));
                },
            );
        });
    }));
    let bounds = |tag: &str| rule.on_node_with_tag(tag).get_bounds_in_root();
    let tri_bounds = bounds("üçgen");
    let c = rule.draw();
    // The background is the outline filled over the bounds; the shadow is of the outline, 4 dp up.
    let fill = c.path_colors.iter().zip(&c.path_strokes).position(|(k, s)| *k == RED && *s == 0.0).expect("the fill");
    assert!(c.paths[fill] == tri_bounds && c.path_shapes[fill].commands().len() == 4); // three points, closed
    assert!(c.elevation_shadows.len() == 1 && matches!(c.elevation_shadows[0].outline, Outline::Generic(_)));
    assert!(c.elevation_shadows[0].bounds == tri_bounds && near(c.elevation_shadows[0].elevation(), 4.0));
    assert!(!c.rect_colors.contains(&RED));
    // Clips: a shape's and a graphics layer's, each its outline as a path clip.
    assert_eq!(c.clip_shapes.len(), 2);
    let clip = c.clip_shapes[0].path.as_ref().expect("a path clip");
    assert!(same_rect(clip.get_bounds(), 0.0, 0.0, 60.0, 60.0));
    assert!(same_rect(c.clip_shapes[1].path.as_ref().expect("a path clip").get_bounds(), 0.0, 0.0, 80.0, 80.0));
    let xs = |p: &Path| p.commands().iter().filter_map(|c| c.end()).map(|o| o.x).collect::<Vec<_>>();
    assert!(xs(clip)[0] == 0.0 && xs(clip)[1] == 60.0); // from the start to the point at the end
    // A press on the clickable over its outline: the press shown over it.
    let button = bounds("düğme");
    down(&rule, button.center().x, button.center().y);
    rule.main_clock().advance_time_by(100);
    assert!(pressed_over(&rule.draw(), button));
    up(&rule, button.center().x, button.center().y);
    rule.wait_for_idle();
    assert_eq!(taps.get(), 1);
    // A source's press drawn by indication(), then gone.
    let press = PressInteraction::Press::new(Offset::new(25.0, 25.0));
    source.try_emit(press);
    rule.wait_for_idle();
    assert!(pressed_over(&rule.draw(), bounds("kendi")));
    source.try_emit(PressInteraction::Release::new(press));
    rule.wait_for_idle();
    assert!(!pressed_over(&rule.draw(), bounds("kendi")));
    // A display list replays path clips as such.
    let mut list = DisplayList::new();
    let (replayed, _) = through_display_list(&rule, &mut list);
    let c = rule.draw();
    assert!(replayed.clip_shapes.len() == 2 && replayed.clip_shapes[0].path == c.clip_shapes[0].path);
    assert!(replayed.clip_shapes[1].path == c.clip_shapes[1].path);
    // Right to left: mirrored, the arrow points to the left.
    rtl.set(true);
    rule.wait_for_idle();
    let c = rule.draw();
    let clip = c.clip_shapes[0].path.as_ref().expect("a path clip");
    assert!(c.clip_shapes.len() == 2 && xs(clip)[0] == 60.0 && xs(clip)[1] == 0.0, "{:?}", c.clip_shapes);
}

fn backgrounds_and_borders_tell_their_shape_and_images_are_images() {
    // Compose's: a background's and a border's shape is their layout's `Shape` semantics property (a later one
    // changed, it is told again); an image with a description is an image to accessibility (one without says
    // nothing).
    let round = mutable_state_of(true);
    let r = round.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let shape: Arc<dyn Shape> = if r.get() { Arc::new(CircleShape) } else { Arc::new(RectangleShape) };
        Column(move || {
            Box(modifier = Modifier.size(20.dp()).background(RED, shape = shape.clone()).test_tag("arka"));
            Box(modifier = Modifier.size(20.dp()).border(1.dp(), BLUE, shape = shape.clone()).test_tag("kenar"));
            let painter: Rc<dyn Painter> = Rc::new(kompoz_ui::draw::ColorPainter(RED));
            Image(painter.clone(), Some("resim".to_string()), modifier = Modifier.size(20.dp()));
            Image(painter, None, modifier = Modifier.size(20.dp()).test_tag("süs"));
        });
    }));
    let shape_of =
        |tag: &str| rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Shape);
    let circle: &dyn Shape = &CircleShape;
    assert!(shape_of("arka").is_some_and(|s| *s == *circle) && shape_of("kenar").is_some_and(|s| *s == *circle));
    round.set(false);
    rule.wait_for_idle();
    let rect: &dyn Shape = &RectangleShape;
    assert!(shape_of("arka").is_some_and(|s| *s == *rect) && shape_of("kenar").is_some_and(|s| *s == *rect));
    rule.on_node_with_content_description("resim").assert(has_role(Role::Image));
    let decorative = rule.on_node_with_tag("süs").fetch_semantics_node().config();
    assert!(decorative.get(SemanticsProperties::Role).is_none());
}

tests!(
    backgrounds_fill_their_shapes_as_composes,
    borders_are_worked_out_for_a_size,
    generic_borders_are_drawn_into_a_bitmap,
    canvases_and_images_say_what_they_show,
    shapes_with_corners_of_their_own_reach_the_canvas_mirrored_in_rtl,
    outline_shapes_fill_stroke_clip_and_shadow,
    backgrounds_and_borders_tell_their_shape_and_images_are_images,
);
