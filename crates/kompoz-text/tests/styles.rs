//! Text styles: span, paragraph and text styles merged, resolved, lerped, copied and compared as Compose's; what
//! they hold (alignments, directions, indents, line heights, line breaking, decorations, shifts, transforms); the
//! style a run is drawn in (the resolution part of the C++ Kompoz's text_sizes_in_em_are_shares_of_a_font_size).

use kompoz_graphics::*;
use kompoz_runtime::kompozable_block;
use kompoz_text::paragraph_style::ParagraphStyle as ParagraphStyleFn;
use kompoz_text::span_style::SpanStyle as SpanStyleFn;
use kompoz_text::style::line_height_style::{Alignment, Mode, Trim};
use kompoz_text::style::{Linearity, lerp_baseline_shift, lerp_text_geometric_transform, lerp_text_indent, line_break};
use kompoz_text::text_style::TextStyle as TextStyleFn;
use kompoz_text::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn panics(f: impl FnOnce()) -> bool {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = catch_unwind(AssertUnwindSafe(f)).is_err();
    std::panic::set_hook(previous);
    r
}

fn gradient(a: Color, b: Color) -> Brush {
    Brush::horizontal_gradient(&[a, b], 0.0, f32::INFINITY, TileMode::Clamp)
}

fn bold() -> SpanStyle {
    SpanStyle { font_weight: Some(FontWeight::Bold), ..SpanStyle::Default }
}

#[test]
fn text_units_are_sp_or_em_and_compare_unspecified_alike() {
    assert!(16.sp() == TextUnit::new(16.0, TextUnitType::Sp) && !16.sp().is_em() && 2.em().is_em());
    assert_eq!(1.5.em(), TextUnit::new(1.5, TextUnitType::Em));
    assert_eq!(TextUnit::Unspecified, TextUnit::Unspecified);
    assert_ne!(16.sp(), 16.em());
    assert_eq!(lerp_text_unit(10.sp(), 20.sp(), 0.25), 12.5.sp());
    assert!(panics(|| {
        lerp_text_unit(10.sp(), 2.em(), 0.5);
    }));
}

#[test]
fn span_styles_merge_what_the_later_gives() {
    let a = SpanStyle { color: Color::Red, font_size: 12.sp(), letter_spacing: 1.sp(), ..SpanStyle::Default };
    let b = SpanStyle { font_size: 14.sp(), font_weight: Some(FontWeight::Bold), ..SpanStyle::Default };
    let m = a.merge(Some(&b));
    assert_eq!(m.foreground(), TextForegroundStyle::Color(Color::Red));
    assert_eq!((m.font_size, m.font_weight, m.letter_spacing), (14.sp(), Some(FontWeight::Bold), 1.sp()));
    assert_eq!(&a + &b, m);
    assert_eq!(a.merge(None), a);
    // Unspecified changes nothing; the default merged changes nothing.
    assert_eq!(a.merge(Some(&SpanStyle::Default)), a);
    assert_eq!(SpanStyle::Default, SpanStyle::default());
}

#[test]
fn foregrounds_merge_as_composes() {
    let g1 = gradient(Color::Red, Color::Blue);
    let g2 = gradient(Color::Green, Color::Yellow);
    let red = SpanStyle { color: Color::Red, ..SpanStyle::Default };
    let brushed = SpanStyle { brush: Some(g1.clone()), alpha: 0.5, ..SpanStyle::Default };
    // A brush goes over a color; a color does not go over a brush.
    assert_eq!(red.merge(Some(&brushed)).foreground(), TextForegroundStyle::Brush { brush: g1.clone(), alpha: 0.5 });
    let blue = SpanStyle { color: Color::Blue, ..SpanStyle::Default };
    assert_eq!(brushed.merge(Some(&blue)).foreground(), TextForegroundStyle::Brush { brush: g1.clone(), alpha: 0.5 });
    // Two brushes: the later, the earlier's alpha if it has none.
    let other = SpanStyle { brush: Some(g2.clone()), ..SpanStyle::Default };
    assert_eq!(brushed.merge(Some(&other)).foreground(), TextForegroundStyle::Brush { brush: g2, alpha: 0.5 });
    // A solid color brush is that color, faded by the alpha.
    let solid = SpanStyle { brush: Some(SolidColor(Color::Red)), alpha: 0.5, ..SpanStyle::Default };
    let f = solid.foreground();
    assert!(f.brush().is_none() && (f.color().alpha() - 0.5).abs() < 0.01 && near(f.color().red(), 1.0));
    // A brush wins over a color given with it; the fields say what it came to after a merge.
    let both = SpanStyle { color: Color::Red, brush: Some(g1.clone()), ..SpanStyle::Default };
    assert!(both.foreground().brush().is_some());
    let merged = SpanStyle::Default.merge(Some(&both));
    assert!(merged.color.is_unspecified() && merged.brush == Some(g1) && merged.alpha.is_nan());
}

#[test]
fn span_style_defaults_are_composes() {
    let r = SpanStyle::Default.resolve_defaults();
    assert_eq!(r.foreground(), TextForegroundStyle::Color(Color::Black));
    assert_eq!((r.font_size, r.letter_spacing), (14.sp(), 0.sp()));
    assert_eq!((r.font_weight, r.font_style), (Some(FontWeight::Normal), Some(FontStyle::Normal)));
    assert_eq!(r.font_synthesis, Some(FontSynthesis::All));
    assert_eq!(r.font_family, Some(FontFamily::Default));
    assert_eq!(r.font_feature_settings.as_deref(), Some(""));
    assert_eq!(r.baseline_shift, Some(BaselineShift::None));
    assert_eq!(r.text_geometric_transform, Some(TextGeometricTransform::None));
    assert_eq!((r.background, r.text_decoration), (Color::Transparent, Some(TextDecoration::None)));
    assert_eq!((r.shadow, r.draw_style), (Some(Shadow::None), Some(DrawStyle::Fill)));
    assert!(r.locale_list.is_some());
    // Given ones stay.
    let given = SpanStyle { font_size: 20.sp(), color: Color::Red, ..SpanStyle::Default }.resolve_defaults();
    assert_eq!((given.font_size, given.color), (20.sp(), Color::Red));
    // An unspecified shift is none.
    let shift = SpanStyle { baseline_shift: Some(BaselineShift::Unspecified), ..SpanStyle::Default };
    assert_eq!(shift.resolve_defaults().baseline_shift, Some(BaselineShift::None));
}

#[test]
fn paragraph_styles_merge_and_resolve() {
    let a = ParagraphStyle { text_align: TextAlign::Center, line_height: 20.sp(), ..ParagraphStyle::Default };
    let b = ParagraphStyle {
        text_direction: TextDirection::Rtl,
        line_break: LineBreak::Heading,
        text_indent: Some(TextIndent::new(4.sp(), 2.sp())),
        ..ParagraphStyle::Default
    };
    let m = a.merge(Some(&b));
    assert_eq!((m.text_align, m.text_direction, m.line_height), (TextAlign::Center, TextDirection::Rtl, 20.sp()));
    assert_eq!((m.line_break, m.text_indent), (LineBreak::Heading, Some(TextIndent::new(4.sp(), 2.sp()))));
    assert_eq!(&a + &b, m);
    let r = ParagraphStyle::Default.resolve_defaults(LayoutDirection::Rtl);
    assert_eq!((r.text_align, r.text_direction), (TextAlign::Start, TextDirection::Rtl));
    assert_eq!((r.line_break, r.hyphens), (LineBreak::Simple, Hyphens::None));
    assert_eq!((r.text_indent, r.text_motion), (Some(TextIndent::None), Some(TextMotion::Static)));
    assert!(r.line_height.is_unspecified() && r.line_height_style.is_none());
    // Content follows the layout's direction where the text has none of its own.
    let content = ParagraphStyle { text_direction: TextDirection::Content, ..ParagraphStyle::Default };
    assert_eq!(content.resolve_defaults(LayoutDirection::Ltr).text_direction, TextDirection::ContentOrLtr);
    assert_eq!(content.resolve_defaults(LayoutDirection::Rtl).text_direction, TextDirection::ContentOrRtl);
    assert_eq!(resolve_text_direction(LayoutDirection::Rtl, TextDirection::Ltr), TextDirection::Ltr);
    // A negative line height is refused.
    assert!(panics(|| {
        kompozable_block!(ParagraphStyleFn(line_height = (-1).sp()));
    }));
}

#[test]
fn text_styles_are_a_span_style_and_a_paragraph_style() {
    let span = SpanStyle { color: Color::Red, font_size: 18.sp(), ..SpanStyle::Default };
    let paragraph = ParagraphStyle {
        text_align: TextAlign::End,
        platform_style: Some(PlatformParagraphStyle { include_font_padding: true, ..Default::default() }),
        ..ParagraphStyle::Default
    };
    let style = TextStyle::new(&span, &paragraph);
    assert_eq!(style.to_span_style(), span);
    assert_eq!(style.to_paragraph_style(), paragraph);
    assert_eq!(style.platform_style, PlatformTextStyle::of(None, paragraph.platform_style));
    // Merged: another text style's given attributes, a span style's, a paragraph style's.
    let other = TextStyle { font_weight: Some(FontWeight::Bold), text_align: TextAlign::Center, ..TextStyle::Default };
    let m = style.merge(&other);
    assert_eq!(
        (m.color, m.font_size, m.font_weight, m.text_align),
        (Color::Red, 18.sp(), Some(FontWeight::Bold), TextAlign::Center)
    );
    assert_eq!(style.merge(&TextStyle::Default), style);
    assert_eq!(style.merge(None::<&TextStyle>), style);
    assert_eq!(style.merge(&bold()).font_weight, Some(FontWeight::Bold));
    let lh = ParagraphStyle { line_height: 30.sp(), ..ParagraphStyle::Default };
    assert_eq!(style.merge(&lh).line_height, 30.sp());
    assert_eq!(&style + &other, m);
    assert_eq!((&style + &bold()).font_weight, Some(FontWeight::Bold));
    // Resolved: every attribute given.
    let r = TextStyle::Default.resolve_defaults(LayoutDirection::Ltr);
    assert_eq!(
        (r.font_size, r.color, r.text_align, r.text_direction),
        (14.sp(), Color::Black, TextAlign::Start, TextDirection::Ltr)
    );
    assert_eq!(TextStyle::Default, TextStyle::default());
    // Layout against paint.
    let recolored = TextStyle { color: Color::Blue, ..style.clone() };
    assert!(
        style.has_same_layout_affecting_attributes(&recolored) && !style.has_same_draw_affecting_attributes(&recolored)
    );
    let resized = TextStyle { font_size: 30.sp(), ..style.clone() };
    assert!(
        !style.has_same_layout_affecting_attributes(&resized) && style.has_same_draw_affecting_attributes(&resized)
    );
}

#[test]
fn styles_take_composes_named_arguments_and_copy() {
    let style = kompozable_block!(TextStyleFn(color = Color::Red, font_size = 20.sp(), text_align = TextAlign::Center));
    assert_eq!((style.color, style.font_size, style.text_align), (Color::Red, 20.sp(), TextAlign::Center));
    let copied = kompozable_block!(style.copy(font_size = 24.sp()));
    assert_eq!((copied.color, copied.font_size, copied.text_align), (Color::Red, 24.sp(), TextAlign::Center));
    // A brush given replaces the color; a color given replaces a brush.
    let g = gradient(Color::Red, Color::Blue);
    let brushed = kompozable_block!(style.copy(brush = Some(g.clone())));
    // (its alpha the color's, as Compose's copy(brush, alpha = this.alpha))
    assert_eq!(brushed.foreground(), TextForegroundStyle::Brush { brush: g.clone(), alpha: 1.0 });
    let recolored = kompozable_block!(brushed.copy(color = Color::Green));
    assert_eq!(recolored.foreground(), TextForegroundStyle::Color(Color::Green));
    let span = kompozable_block!(SpanStyleFn(font_weight = Some(FontWeight::Bold), background = Color::Yellow));
    assert_eq!((span.font_weight, span.background), (Some(FontWeight::Bold), Color::Yellow));
    assert_eq!(kompozable_block!(span.copy(font_weight = None)).font_weight, None);
    let p = kompozable_block!(ParagraphStyleFn(hyphens = Hyphens::Auto));
    assert_eq!(kompozable_block!(p.copy(text_align = TextAlign::Justify)).text_align, TextAlign::Justify);
    assert_eq!(kompozable_block!(p.copy(text_align = TextAlign::Justify)).hyphens, Hyphens::Auto);
}

#[test]
fn styles_lerp_as_composes() {
    let a = SpanStyle {
        font_size: 10.sp(),
        color: Color::Black,
        font_family: Some(FontFamily::Serif),
        ..SpanStyle::Default
    };
    let b = SpanStyle {
        font_size: 20.sp(),
        color: Color::White,
        font_family: Some(FontFamily::Monospace),
        font_weight: Some(FontWeight::Bold),
        shadow: Some(Shadow::new(Color::Black, Offset::new(4.0, 4.0), 2.0, 0.0)),
        ..SpanStyle::Default
    };
    let m = lerp_span_style(&a, &b, 0.5);
    assert_eq!(m.font_size, 15.sp());
    assert_eq!(m.font_weight, Some(FontWeight::new(550)));
    assert_eq!(m.color, lerp_color(Color::Black, Color::White, 0.5));
    assert_eq!(m.font_family, Some(FontFamily::Monospace));
    // A shadow from none: from its own faded out.
    let shadow = m.shadow.expect("a shadow");
    assert!(
        (shadow.color.alpha() - 0.5).abs() < 0.01
            && shadow.offset == Offset::new(4.0, 4.0)
            && near(shadow.blur_radius, 2.0)
    );
    assert_eq!(lerp_span_style(&a, &b, 0.2).font_family, Some(FontFamily::Serif));
    // Units of two kinds: the nearer one.
    let em = SpanStyle { font_size: 2.em(), ..SpanStyle::Default };
    assert_eq!(lerp_span_style(&a, &em, 0.4).font_size, 10.sp());
    let p = lerp_paragraph_style(
        &ParagraphStyle {
            line_height: 10.sp(),
            text_indent: Some(TextIndent::new(0.sp(), 0.sp())),
            ..ParagraphStyle::Default
        },
        &ParagraphStyle {
            line_height: 30.sp(),
            text_indent: Some(TextIndent::new(8.sp(), 4.sp())),
            ..ParagraphStyle::Default
        },
        0.5,
    );
    assert_eq!((p.line_height, p.text_indent), (20.sp(), Some(TextIndent::new(4.sp(), 2.sp()))));
    let t = lerp_text_style(
        &TextStyle { font_size: 10.sp(), ..TextStyle::Default },
        &TextStyle { font_size: 30.sp(), ..TextStyle::Default },
        0.25,
    );
    assert_eq!(t.font_size, 15.sp());
    assert_eq!(lerp_baseline_shift(BaselineShift::Subscript, BaselineShift::Superscript, 0.5), BaselineShift::None);
    assert_eq!(
        lerp_text_geometric_transform(&TextGeometricTransform::None, &TextGeometricTransform::new(2.0, -0.5), 0.5),
        TextGeometricTransform::new(1.5, -0.25)
    );
    assert_eq!(
        lerp_text_indent(&TextIndent::None, &TextIndent::new(10.sp(), 20.sp()), 0.5),
        TextIndent::new(5.sp(), 10.sp())
    );
}

#[test]
fn style_values_are_composes() {
    assert_eq!(
        TextDecoration::Underline + TextDecoration::LineThrough,
        TextDecoration::combine(&[TextDecoration::LineThrough, TextDecoration::Underline])
    );
    let both = TextDecoration::Underline + TextDecoration::LineThrough;
    assert!(both.contains(TextDecoration::Underline) && !TextDecoration::Underline.contains(both));
    assert!(TextDecoration::None.contains(TextDecoration::None));
    assert_eq!(format!("{both:?}"), "TextDecoration[Underline, LineThrough]");
    assert_eq!(format!("{:?}", TextDecoration::Underline), "TextDecoration.Underline");
    assert_eq!(TextDecoration::value_of(2), TextDecoration::LineThrough);
    assert!(panics(|| {
        TextDecoration::value_of(4);
    }));
    assert_eq!(BaselineShift::Unspecified, BaselineShift::Unspecified);
    assert_eq!((BaselineShift::Superscript.multiplier(), BaselineShift::Subscript.0), (0.5, -0.5));
    assert!(!BaselineShift::Unspecified.is_specified());
    assert_eq!(LineHeightStyle::Default, LineHeightStyle::new(Alignment::Proportional, Trim::Both, Mode::Fixed));
    assert!(Trim::Both.is_trim_first_line_top() && Trim::Both.is_trim_last_line_bottom());
    assert!(Trim::FirstLineTop.is_trim_first_line_top() && !Trim::FirstLineTop.is_trim_last_line_bottom());
    assert!(!Trim::None.is_trim_first_line_top() && !Trim::None.is_trim_last_line_bottom());
    assert_eq!((Alignment::Center.top_ratio(), Alignment::new(0.25).top_ratio()), (0.5, 0.25));
    assert!(panics(|| {
        Alignment::new(2.0);
    }));
    assert_eq!(
        LineBreak::Paragraph,
        LineBreak::new(
            line_break::Strategy::HighQuality,
            line_break::Strictness::Strict,
            line_break::WordBreak::Default
        )
    );
    assert_eq!(LineBreak::Simple.strictness, line_break::Strictness::Normal);
    assert_eq!(LineBreak::Heading.word_break, line_break::WordBreak::Phrase);
    assert!(!LineBreak::Unspecified.is_specified() && LineBreak::default() == LineBreak::Unspecified);
    assert_eq!(TextIndent::default(), kompozable_block!(TextIndent()));
    assert_eq!(kompozable_block!(TextIndent(first_line = 4.sp())), TextIndent::new(4.sp(), 0.sp()));
    assert_eq!(TextGeometricTransform::default(), TextGeometricTransform::new(1.0, 0.0));
    assert_eq!(TextMotion::Animated, TextMotion { linearity: Linearity::Linear, subpixel_text_positioning: true });
    assert_eq!(TextMotion::default(), TextMotion::Static);
    assert_eq!(TextAlign::VALUES.len(), 6);
    assert_eq!(TextAlign::Unspecified.take_or_else(|| TextAlign::End), TextAlign::End);
    assert_eq!(TextOverflow::default(), TextOverflow::Clip);
    assert!(panics(|| {
        Placeholder::new(TextUnit::Unspecified, 10.sp(), PlaceholderVerticalAlign::Top);
    }));
    let p = Placeholder::new(2.em(), 10.sp(), PlaceholderVerticalAlign::TextCenter);
    assert_eq!((p.width, p.placeholder_vertical_align), (2.em(), PlaceholderVerticalAlign::TextCenter));
}

/// The style a run is drawn in (the resolution part of the C++ Kompoz's text_sizes_in_em_are_shares_of_a_font_size,
/// its 16 sp default and its ProvideTextStyle aside).
#[test]
fn text_sizes_in_em_are_shares_of_a_font_size() {
    let d = Density::default();
    let twenty = TextStyle { font_size: 20.sp(), ..TextStyle::Default };
    // A span's em size is of the size around it; a letter spacing's of its own size.
    let small = SpanStyle { font_size: 0.5.em(), ..SpanStyle::Default };
    assert!(near(TextRunStyle::resolve(&twenty, &[], d).font_size, 20.0));
    assert!(near(TextRunStyle::resolve(&twenty, &[&small], d).font_size, 10.0));
    let wide = SpanStyle { font_size: 30.sp(), letter_spacing: 0.1.em(), ..SpanStyle::Default };
    let r = TextRunStyle::resolve(&twenty, &[&wide], d);
    assert!(near(r.font_size, 30.0) && near(r.letter_spacing, 3.0));
    let spaced = TextStyle { letter_spacing: 0.1.em(), ..twenty.clone() };
    assert!(near(TextRunStyle::resolve(&spaced, &[], d).letter_spacing, 2.0));
    // A line height in em is of the text's size.
    let line = TextStyle { font_size: 10.sp(), line_height: 1.5.em(), ..TextStyle::Default };
    assert_eq!(TextRunStyle::resolve(&line, &[], d).line_height, Some(15.0));
    assert_eq!(TextRunStyle::resolve(&twenty, &[], d).line_height, None);
    // Nested em sizes compound (as Android's relative size spans).
    assert!(near(TextRunStyle::resolve(&twenty, &[&small, &small], d).font_size, 5.0));
    // A text style merged over another takes its size as given: an em one is of the default 14 sp then (Compose's
    // merge; C++ resolved it against the style around it, 30 here).
    let merged = twenty.merge(&TextStyle { font_size: 1.5.em(), ..TextStyle::Default });
    assert!(near(TextRunStyle::resolve(&merged, &[], d).font_size, 21.0));
    // Sizes in px follow the font scale.
    assert!(near(TextRunStyle::resolve(&twenty, &[], Density::new(1.0, 2.0)).font_size, 40.0));
    assert!(near(TextRunStyle::resolve(&TextStyle::Default, &[], Density::new(2.0, 1.0)).font_size, 28.0));
    // The other attributes: the later span's.
    let italic = SpanStyle {
        font_style: Some(FontStyle::Italic),
        font_feature_settings: Some("smcp".into()),
        ..SpanStyle::Default
    };
    let skewed =
        SpanStyle { text_geometric_transform: Some(TextGeometricTransform::new(1.5, -0.25)), ..SpanStyle::Default };
    let r = TextRunStyle::resolve(&twenty, &[&bold(), &italic, &skewed], d);
    assert_eq!(
        (r.font_weight, r.font_style, r.font_feature_settings.as_str()),
        (FontWeight::Bold, FontStyle::Italic, "smcp")
    );
    assert_eq!((r.scale_x, r.skew_x), (1.5, -0.25));
    assert_eq!(r.font_family, FontFamily::Default);
}

#[test]
fn model_types_cross_threads() {
    fn shareable<T: Send + Sync>() {}
    shareable::<TextStyle>();
    shareable::<SpanStyle>();
    shareable::<ParagraphStyle>();
    shareable::<AnnotatedString>();
    shareable::<TextFieldValue>();
    shareable::<FontFamily>();
    shareable::<Typeface>();
    shareable::<LinkAnnotation>();
    shareable::<LocaleList>();
}
