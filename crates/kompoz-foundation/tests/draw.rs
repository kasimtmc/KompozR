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

tests!(
    backgrounds_fill_their_shapes_as_composes,
    borders_are_worked_out_for_a_size,
    generic_borders_are_drawn_into_a_bitmap,
    canvases_and_images_say_what_they_show,
);
