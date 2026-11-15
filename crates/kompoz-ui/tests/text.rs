//! Text drawn through a DrawScope (Compose's DrawScope.drawText over a TextLayoutResult or a TextMeasurer) with
//! the C++ Kompoz's numbers (its fake measurer: half an em a code point, ascent 0.8, line 1.25 em): a measurer
//! remembered in a composition, what a draw call's color, brush, alpha, shadow, decoration and draw style go over
//! (the text's spans keep theirs), shadows under their text, brushes spread over all of it, draw styles, geometric
//! transforms, features and families reaching the canvas, clipping, blending, the display list keeping text runs,
//! and a Text's layout handed out through its semantics.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::{
    AnnotatedString, FontFamily, SpanStyle, TextDecoration, TextGeometricTransform, TextLayoutResult, TextMeasurer,
    TextOverflow, TextStyle, build_annotated_string,
};
use kompoz_ui::draw::*;
use kompoz_ui::semantics::SemanticsActions;
use kompoz_ui::text::remember_text_measurer;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn size(px: f32) -> TextStyle {
    TextStyle { font_size: px.sp(), ..TextStyle::Default }
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// The index of the drawn text `text` (the first).
fn index(c: &RecordingCanvas, text: &str) -> usize {
    c.texts.iter().position(|t| t == text).unwrap_or_else(|| panic!("no {text:?} among {:?}", c.texts))
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);

fn text_measurer_lays_text_out_for_drawing() {
    // C++ 23929's drawing part: a measurer remembered in the composition, its layout drawn twice (the second in a
    // color of its own) and a text measured and drawn at once in a size of its own.
    let measured = Rc::new(RefCell::new(None::<TextLayoutResult>));
    let seen = measured.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let m = remember_text_measurer();
        let seen = seen.clone();
        Canvas(
            Modifier.size_wh(200.dp(), 100.dp()).test_tag("tuval"),
            on_draw = move |d| {
                let drawn = m.measure("Merhaba dünya", style = size(20.0));
                d.draw_text(&drawn, top_left = Offset::new(10.0, 20.0));
                d.draw_text(&drawn, Color::hex(0xD32F2F), top_left = Offset::new(10.0, 60.0));
                d.draw_text_measurer(&m, "kısa", top_left = Offset::new(150.0, 0.0), style = size(10.0));
                *seen.borrow_mut() = Some(drawn);
            },
        );
    }));
    let c = rule.draw();
    let at = bounds(&rule, "tuval");
    let first = index(&c, "Merhaba dünya");
    assert!(near(c.baselines[first].x, at.left + 10.0) && near(c.baselines[first].y, at.top + 36.0));
    assert_eq!(c.colors[first], Color::Black);
    let second = first + 1 + c.texts[first + 1..].iter().position(|t| t == "Merhaba dünya").expect("drawn twice");
    assert!(c.colors[second] == Color::hex(0xD32F2F) && near(c.baselines[second].y, at.top + 76.0));
    let short = index(&c, "kısa");
    assert!(near(c.text_styles[short].font_size, 10.0) && near(c.baselines[short].x, at.left + 150.0));
    let r = measured.borrow().clone().expect("measured");
    assert!(r.size == IntSize::new(130, 25) && r.line_count() == 1 && near(r.first_baseline(), 16.0));
}

fn draw_text_paints_what_the_texts_own_style_did() {
    // C++ 28194: a draw call's color (faded), shadow, decoration and draw style go where the text's spans gave none;
    // a brush where they gave no color; text measured and drawn at once, cut to its size.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let m = remember_text_measurer();
        let text = build_annotated_string(|b| {
            b.append("düz ");
            let span = SpanStyle {
                color: GREEN,
                shadow: Some(Shadow::new(RED, Offset::new(1.0, 1.0), 0.0, 0.0)),
                ..SpanStyle::Default
            };
            b.with_style(span, |b| b.append("yeşil"));
        });
        let layout = m.measure(text.clone(), style = TextStyle { color: BLUE, ..size(10.0) });
        Canvas(
            Modifier.size_wh(200.dp(), 100.dp()),
            on_draw = move |d| {
                d.draw_text_with(
                    &layout,
                    RED,
                    Offset::Zero,
                    0.5,
                    Some(Shadow::new(Color::Black, Offset::new(2.0, 2.0), 0.0, 0.0)),
                    Some(TextDecoration::Underline),
                    Some(DrawStyle::Stroke(Stroke::default())),
                    DefaultBlendMode,
                );
                let gradient = Brush::horizontal_gradient(
                    &[Color::new(1.0, 1.0, 0.0, 1.0), Color::new(0.0, 1.0, 1.0, 1.0)],
                    0.0,
                    f32::INFINITY,
                    TileMode::Clamp,
                );
                d.draw_text_brush(&layout, gradient, top_left = Offset::new(0.0, 20.0));
                d.draw_text_measurer(
                    &m,
                    text.clone(),
                    top_left = Offset::new(0.0, 40.0),
                    style = size(10.0),
                    overflow = TextOverflow::Ellipsis,
                    max_lines = 1,
                    size = Size::new(30.0, 20.0),
                );
            },
        );
    }));
    let c = rule.draw();
    // First: the plain run red, faded, underlined, outlined (a hairline), the call's shadow; the span its own green
    // and shadow, outlined too (it sets no draw style of its own).
    let plain = (0..c.texts.len()).find(|&i| c.baselines[i].y < 15.0 && c.texts[i].starts_with("düz")).expect("düz");
    let span = (0..c.texts.len()).find(|&i| c.baselines[i].y < 15.0 && c.texts[i] == "yeşil").expect("yeşil");
    assert_eq!(c.colors[plain], Color::new(1.0, 0.0, 0.0, 0.5));
    assert!(matches!(&c.text_styles[plain].draw_style, DrawStyle::Stroke(s) if s.width == 0.0));
    assert!(c.colors[span] == GREEN && matches!(c.text_styles[span].draw_style, DrawStyle::Stroke(_)));
    let shadows: Vec<Color> =
        (0..c.shadow_colors.len()).filter(|&i| c.shadow_baselines[i].y < 15.0).map(|i| c.shadow_colors[i]).collect();
    assert!(shadows.contains(&Color::Black) && shadows.contains(&RED) && shadows.len() == 2);
    assert!(!c.lines.is_empty() && c.lines[0].0.y < 15.0); // the plain run underlined
    // Second: the brush where the text's own style painted, the span still green.
    let brushed = (0..c.texts.len())
        .filter(|&i| c.baselines[i].y > 15.0 && c.baselines[i].y < 35.0 && c.colors[i] != GREEN)
        .count();
    assert!(brushed == 1 && c.brush_areas.len() == 1);
    // Third: in 30 px, one line at most, cut with an ellipsis, clipped to its size.
    let third: Vec<usize> = (0..c.texts.len()).filter(|&i| c.baselines[i].y > 35.0).collect();
    assert!(third.iter().any(|&i| c.texts[i].ends_with('…')));
    let ys: std::collections::BTreeSet<i32> = third.iter().map(|&i| c.baselines[i].y as i32).collect();
    assert_eq!(ys.len(), 1);
    assert!(c.clips.iter().any(|r| near(r.width(), 30.0) && near(r.top, 40.0)));
}

fn text_shadows_go_under_and_brushes_spread_over_the_text() {
    // C++ 27516, its texts measured and drawn one under another.
    let soft = Shadow::new(Color::new(0.0, 0.0, 0.0, 0.5), Offset::new(2.0, 3.0), 4.0, 0.0);
    let red_to_blue = Brush::horizontal_gradient(&[RED, BLUE], 0.0, f32::INFINITY, TileMode::Clamp);
    let (s, b) = (soft.clone(), red_to_blue.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let m = remember_text_measurer();
        let (soft, red_to_blue) = (s.clone(), b.clone());
        Canvas(
            Modifier.size_wh(400.dp(), 200.dp()),
            on_draw = move |d| {
                let shadowed = m.measure("gölge", style = TextStyle { shadow: Some(soft.clone()), ..size(10.0) });
                d.draw_text(&shadowed);
                let degrade = m.measure(
                    "degrade",
                    style = TextStyle {
                        brush: Some(red_to_blue.clone()),
                        text_decoration: Some(TextDecoration::Underline),
                        ..size(10.0)
                    },
                );
                d.draw_text(&degrade, top_left = Offset::new(0.0, 20.0));
                let spans = build_annotated_string(|b| {
                    b.append("düz ");
                    let own_shadow = Shadow::new(RED, Offset::new(1.0, 1.0), 0.0, 0.0);
                    b.with_style(SpanStyle { shadow: Some(own_shadow), ..SpanStyle::Default }, |b| b.append("kırmızı"));
                    b.with_style(SpanStyle { color: GREEN, ..SpanStyle::Default }, |b| b.append(" yeşil"));
                    let own = Brush::vertical_gradient(&[GREEN, Color::Black], 0.0, f32::INFINITY, TileMode::Clamp);
                    b.with_style(SpanStyle { brush: Some(own), ..SpanStyle::Default }, |b| b.append(" fırça"));
                });
                let styled = m.measure(spans, style = TextStyle { brush: Some(red_to_blue.clone()), ..size(10.0) });
                d.draw_text(&styled, top_left = Offset::new(0.0, 40.0));
                let brushed = m.measure("boya", style = TextStyle { brush: Some(red_to_blue.clone()), ..size(10.0) });
                d.draw_text(&brushed, GREEN, top_left = Offset::new(0.0, 60.0)); // a color given wins
            },
        );
    }));
    let c = rule.draw();
    // The shadow: moved, its color and blur, drawn before its text.
    assert!(c.shadow_texts[0] == "gölge" && c.shadow_colors[0] == soft.color && c.shadow_blurs[0] == 4.0);
    assert!(near(c.shadow_baselines[0].x, 2.0) && near(c.shadow_baselines[0].y, 8.0 + 3.0));
    assert!(c.text_order.starts_with("st"));
    // The brush spreads over the text's bounds: its middle half red, half blue.
    let degrade = index(&c, "degrade");
    assert!(same(c.brush_areas[0], Rect::new(0.0, 20.0, 35.0, 32.5)));
    let mid = c.colors[degrade];
    assert!(near(mid.red(), 0.5) && near(mid.blue(), 0.5));
    // Spans: a shadow of their own; a color or brush of their own goes over the text's brush, which spreads over
    // the whole text for each of its pieces.
    assert!(c.shadow_texts.len() == 2 && c.shadow_texts[1] == "kırmızı" && c.shadow_colors[1] == RED);
    assert!(c.colors[index(&c, " yeşil")] == GREEN && c.brush_areas.len() == 4);
    assert!(same(c.brush_areas[1], c.brush_areas[2]));
    assert_eq!(c.colors[index(&c, "boya")], GREEN);
    let own = c.colors[index(&c, " fırça")];
    assert!(near(own.green(), 0.5) && own.red() == 0.0 && own.blue() == 0.0);
    // Recorded and replayed as they were.
    let mut list = DisplayList::new();
    list.begin(Size::new(400.0, 200.0));
    rule.host().draw(&mut list);
    list.end();
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    assert!(replayed.shadow_texts == c.shadow_texts && replayed.shadow_blurs == c.shadow_blurs);
    assert!(replayed.brush_areas.len() == 4 && replayed.text_order == c.text_order);
}

fn same(a: Rect, b: Rect) -> bool {
    near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) && near(a.bottom, b.bottom)
}

/// What a measurer over the composition's resolver lays `text` out as, drawn by a Canvas, recorded.
fn drawn(text: AnnotatedString, style: TextStyle) -> RecordingCanvas {
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let m = remember_text_measurer();
        let (text, style) = (text.clone(), style.clone());
        Canvas(
            Modifier.size_wh(300.dp(), 100.dp()),
            on_draw = move |d| {
                let r = m.measure(text.clone(), style = style.clone());
                d.draw_text(&r);
            },
        );
    }));
    rule.draw()
}

fn text_draw_style_strokes_its_glyphs() {
    // C++ 25970: a style's draw style reaches the canvas; a span's own over it, a span's Fill filling again.
    let stroke = |w: f32, join: StrokeJoin| DrawStyle::Stroke(Stroke::new(w, 4.0, StrokeCap::Butt, join, None));
    let c = drawn("dolu".into(), size(16.0));
    assert!(matches!(c.text_styles[0].draw_style, DrawStyle::Fill));
    let c = drawn("çizgi".into(), TextStyle { draw_style: Some(stroke(2.0, StrokeJoin::Round)), ..size(16.0) });
    assert!(
        matches!(&c.text_styles[0].draw_style, DrawStyle::Stroke(s) if s.width == 2.0 && s.join == StrokeJoin::Round)
    );
    let spans = build_annotated_string(|b| {
        b.append("bir ");
        b.with_style(SpanStyle { draw_style: Some(stroke(3.0, StrokeJoin::Round)), ..SpanStyle::Default }, |b| {
            b.append("hat")
        });
    });
    let c = drawn(spans, size(16.0));
    assert!(matches!(c.text_styles[index(&c, "bir ")].draw_style, DrawStyle::Fill));
    assert!(matches!(&c.text_styles[index(&c, "hat")].draw_style, DrawStyle::Stroke(s) if s.width == 3.0));
    let inherited = build_annotated_string(|b| {
        b.append("miras ");
        b.with_style(SpanStyle { draw_style: Some(DrawStyle::Fill), ..SpanStyle::Default }, |b| b.append("dolu"));
    });
    let c = drawn(inherited, TextStyle { draw_style: Some(stroke(1.0, StrokeJoin::Bevel)), ..size(16.0) });
    assert!(
        matches!(&c.text_styles[index(&c, "miras ")].draw_style, DrawStyle::Stroke(s) if s.width == 1.0 && s.join == StrokeJoin::Bevel)
    );
    assert!(matches!(c.text_styles[index(&c, "dolu")].draw_style, DrawStyle::Fill));
    // Stroked glyphs reach further: their ink grows by half the stroke.
    let c = drawn("a".into(), TextStyle { draw_style: Some(stroke(4.0, StrokeJoin::Miter)), ..size(10.0) });
    let ink = c.text_draws[0].ink_bounds;
    assert!(near(ink.left, -2.0) && near(ink.top, -10.0));
}

fn text_geometric_transform_stretches_and_slants() {
    // C++ 26008: advances stretched, the transform reaching the canvas; a span's own.
    let measure = |text: AnnotatedString, style: TextStyle| {
        let m =
            TextMeasurer::new(kompoz_ui::platform::fake_resolver(), Density::new(1.0, 1.0), LayoutDirection::Ltr, 0);
        m.measure_with(
            text,
            style,
            TextOverflow::Clip,
            true,
            usize::MAX,
            Vec::new(),
            Constraints::default(),
            LayoutDirection::Ltr,
            m.default_density(),
            m.default_font_family_resolver().clone(),
            false,
        )
    };
    assert_eq!(measure("abcd".into(), size(20.0)).size.width, 40);
    let wide = TextStyle { text_geometric_transform: Some(TextGeometricTransform::new(2.0, 0.25)), ..size(20.0) };
    assert_eq!(measure("abcd".into(), wide.clone()).size.width, 80);
    let half =
        SpanStyle { text_geometric_transform: Some(TextGeometricTransform::new(0.5, -0.5)), ..SpanStyle::Default };
    let spans = build_annotated_string(|b| {
        b.append("ab");
        b.with_style(half.clone(), |b| b.append("cd"));
    });
    assert_eq!(measure(spans.clone(), size(20.0)).size.width, 30);
    let c = drawn("abcd".into(), wide);
    assert!(near(c.text_styles[0].scale_x, 2.0) && near(c.text_styles[0].skew_x, 0.25));
    assert!(near(c.text_draws[0].glyphs[0].scale_x, 2.0) && near(c.text_draws[0].glyphs[0].skew_x, 0.25));
    let c = drawn(spans, size(20.0));
    let cd = index(&c, "cd");
    assert!(near(c.text_styles[cd].scale_x, 0.5) && near(c.text_styles[cd].skew_x, -0.5));
}

fn font_features_and_families_reach_the_canvas() {
    // C++ 17840 and 12415: the features and family a run was laid out in are what it is drawn in.
    let c = drawn("123".into(), TextStyle { font_feature_settings: Some("tnum".into()), ..size(14.0) });
    assert_eq!(c.text_styles[0].font_feature_settings, "tnum");
    let spans = build_annotated_string(|b| {
        b.append("ab");
        b.with_style(SpanStyle { font_feature_settings: Some("smcp".into()), ..SpanStyle::Default }, |b| {
            b.append("cd")
        });
    });
    let c = drawn(spans, size(14.0));
    assert!(c.text_styles[index(&c, "ab")].font_feature_settings.is_empty());
    assert_eq!(c.text_styles[index(&c, "cd")].font_feature_settings, "smcp");
    let c = drawn("serif".into(), TextStyle { font_family: Some(FontFamily::Serif), ..size(14.0) });
    assert_eq!(c.text_styles[0].font_family, FontFamily::Serif);
    let families = build_annotated_string(|b| {
        b.append("düz ");
        b.with_style(SpanStyle { font_family: Some(FontFamily::Monospace), ..SpanStyle::Default }, |b| {
            b.append("mono")
        });
        let bold = SpanStyle {
            font_weight: Some(kompoz_text::FontWeight::Bold),
            font_family: Some(FontFamily::Cursive),
            ..SpanStyle::Default
        };
        b.with_style(bold, |b| b.append(" el"));
    });
    let c = drawn(families, size(14.0));
    assert_eq!(c.text_styles.len(), 3);
    assert_eq!(c.text_styles[0].font_family, FontFamily::Default);
    assert_eq!(c.text_styles[1].font_family, FontFamily::Monospace);
    assert!(
        c.text_styles[2].font_family == FontFamily::Cursive
            && c.text_styles[2].font_weight == kompoz_text::FontWeight::Bold
    );
}

fn draw_text_takes_compose_named_arguments() {
    // C++ 7983's text part: drawText(measurer, text, topLeft =, style =) by name draws what the positional form
    // draws; drawText(layout) blended through the context.
    let by_name = KompozTestRule::new(kompozable_lambda!(|| {
        let m = remember_text_measurer();
        Box(Modifier.size_wh(100.dp(), 50.dp()).draw_with_content(move |d| {
            d.draw_content();
            d.draw_text_measurer(&m, "ölçek", top_left = Offset::new(2.0, 30.0), style = size(11.0));
        }));
    }))
    .draw();
    let by_place = KompozTestRule::new(kompozable_lambda!(|| {
        let m = remember_text_measurer();
        Box(Modifier.size_wh(100.dp(), 50.dp()).draw_with_content(move |d| {
            d.draw_content();
            d.draw_text_measurer_with(
                &m,
                "ölçek",
                Offset::new(2.0, 30.0),
                size(11.0),
                TextOverflow::Clip,
                true,
                usize::MAX,
                Vec::new(),
                Size::Unspecified,
                DefaultBlendMode,
            );
        }));
    }))
    .draw();
    assert!(by_name.texts == by_place.texts && by_name.baselines == by_place.baselines && by_name.texts == ["ölçek"]);
    let blended = KompozTestRule::new(kompozable_lambda!(|| {
        let m = remember_text_measurer();
        Canvas(
            Modifier.size_wh(100.dp(), 50.dp()),
            on_draw = move |d| {
                let r = m.measure("karışım");
                d.draw_text(&r, blend_mode = BlendMode::Multiply);
            },
        );
    }))
    .draw();
    assert!(blended.stack.contains("Bb") && blended.texts == ["karışım"]);
}

fn a_text_hands_out_its_layout_through_semantics() {
    // GetTextLayoutResult, typed: the layout a Text drew.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Text("bir iki üç", font_size = 16.sp(), modifier = Modifier.width(48.dp()).test_tag("metin"));
    }));
    let mut layouts: Vec<TextLayoutResult> = Vec::new();
    rule.on_node_with_tag("metin").perform_semantics_action_with(SemanticsActions::GetTextLayoutResult, |f| {
        assert!(f(&mut layouts));
    });
    assert_eq!(layouts.len(), 1);
    let r = &layouts[0];
    // 8 px a character in 48: "bir iki" doesn't fit, "iki üç" does.
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.layout_input.text.text(), "bir iki üç");
    assert_eq!(bounds(&rule, "metin").height(), 40.0);
    assert_eq!(rule.drawn_texts(), ["bir", "iki üç"]);
    // Drawn twice, the same glyph runs (its layout kept).
    let (a, b) = (rule.draw(), rule.draw());
    assert!(Rc::ptr_eq(&a.text_draws[0].glyphs, &b.text_draws[0].glyphs));
}

fn remembered_measurers_follow_the_composition() {
    // rememberTextMeasurer: the composition's density and direction, the same measurer while they stay.
    let density = mutable_state_of(1.0f32);
    let seen = Rc::new(RefCell::new(Vec::<Rc<TextMeasurer>>::new()));
    let (d, s) = (density.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let value = d.get();
        let s = s.clone();
        KompozitionLocalProvider(&[kompoz_ui::platform::LocalDensity.provides(Density::new(value, 1.0))], move || {
            s.borrow_mut().push(remember_text_measurer());
        });
    }));
    density.set(2.0);
    rule.wait_for_idle();
    let seen = seen.borrow();
    assert!(seen.len() >= 2);
    assert!(near(seen[0].default_density().density, 1.0) && near(seen.last().unwrap().default_density().density, 2.0));
    assert_eq!(seen[0].default_layout_direction(), LayoutDirection::Ltr);
}

tests!(
    text_measurer_lays_text_out_for_drawing,
    draw_text_paints_what_the_texts_own_style_did,
    text_shadows_go_under_and_brushes_spread_over_the_text,
    text_draw_style_strokes_its_glyphs,
    text_geometric_transform_stretches_and_slants,
    font_features_and_families_reach_the_canvas,
    draw_text_takes_compose_named_arguments,
    a_text_hands_out_its_layout_through_semantics,
    remembered_measurers_follow_the_composition,
);
