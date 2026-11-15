//! foundation's BasicText with the C++ Kompoz's numbers (its fake measurer: half an em a code point, ascent 0.8,
//! lines 1.25 em; its `Text` was 16 sp, so its tests say 16 sp here): wrapping, overflow, max and min lines,
//! paragraph styles, the layout told to `on_text_layout`, auto size, inline content, text substitution, the layout
//! kept while only the color changes, intrinsics, and text in real fonts (Arabic, Hindi, emoji).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::{InlineTextContent, TextAutoSizeLayoutScope};
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::style::line_height_style::Trim;
use kompoz_text::*;
use kompoz_ui::semantics::{SemanticsActions, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// A text style of `px` px (sp at density 1).
fn sized(px: f32) -> TextStyle {
    TextStyle { font_size: px.sp(), ..TextStyle::Default }
}

/// A node's bounds in the root.
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// The index of the drawn text `text` (the first).
fn index(c: &RecordingCanvas, text: &str) -> usize {
    c.texts.iter().position(|t| t == text).unwrap_or_else(|| panic!("no {text:?} among {:?}", c.texts))
}

/// A callback for `on_text_layout` keeping the last layout.
fn keep(into: &Rc<RefCell<Option<TextLayoutResult>>>) -> OnTextLayout {
    let into = into.clone();
    Rc::new(move |r: &TextLayoutResult| *into.borrow_mut() = Some(r.clone()))
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);

// ---- Measuring, wrapping, overflow -----------------------------------------------------------------------------

fn text_wraps_to_width() {
    // C++ 727: 16 sp, 8 px a character, 20 px lines.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Text("aaaa bbbb cccc", font_size = 16.sp(), modifier = Modifier.width(60.dp()).test_tag("t"));
    }));
    assert_eq!(bounds(&rule, "t").height(), 60.0); // three lines
}

fn text_overflow_soft_wrap_and_max_lines() {
    // C++ 12383: 80 px hold 10 characters.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            let w = || Modifier.width(80.dp());
            BasicText("bir iki üç dört beş", modifier = w().test_tag("wrap"), style = sized(16.0));
            BasicText(
                "bir iki üç dört beş",
                modifier = w().test_tag("ellipsis"),
                style = sized(16.0),
                max_lines = 1,
                overflow = TextOverflow::Ellipsis,
            );
            BasicText("bir iki üç dört beş", modifier = w().test_tag("clip"), style = sized(16.0), max_lines = 1);
            BasicText(
                "bir iki üç dört beş",
                modifier = w().test_tag("nowrap"),
                style = sized(16.0),
                overflow = TextOverflow::Ellipsis,
                soft_wrap = false,
            );
            BasicText("satır\n\nboş satır", modifier = Modifier.test_tag("breaks"), style = sized(16.0));
            BasicText("bir iki üç dört beş", modifier = w().test_tag("taşan"), style = sized(16.0), soft_wrap = false);
        });
    }));
    assert_eq!(bounds(&rule, "wrap").height(), 40.0); // "bir iki üç" / "dört beş"
    assert_eq!(bounds(&rule, "ellipsis").height(), 20.0);
    assert_eq!(bounds(&rule, "clip").height(), 20.0);
    assert_eq!(bounds(&rule, "breaks").height(), 60.0); // the empty line counts
    let texts = rule.drawn_texts();
    // Max lines 1 and no wrapping: cut to fit "…"; clipped: the first line.
    assert_eq!(texts.iter().filter(|t| *t == "bir iki ü…").count(), 2, "{texts:?}");
    assert!(texts.iter().any(|t| t == "bir iki üç"), "{texts:?}");
    // What runs past its size is clipped to it (a plain string's: its lines cut off by max lines aren't laid out,
    // nothing to clip, as Compose's TextStringSimpleNode).
    let c = rule.draw();
    let past = bounds(&rule, "taşan");
    assert_eq!(c.clips.len(), 1, "{:?}", c.clips);
    assert!(near(c.clips[0].top, past.top) && near(c.clips[0].height(), 20.0) && near(c.clips[0].width(), 80.0));
}

fn text_that_gives_way_to_the_width_takes_all_of_it() {
    // C++ 6479: as wide as its widest line unless it wraps to the width (then all of it), as Compose's
    // min(maxIntrinsicWidth, maxWidth); a line cut off or running past it: all of it too.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            Modifier.width(200.dp()),
            content = || {
                BasicText("kısa", modifier = Modifier.test_tag("kısa"), style = sized(16.0));
                BasicText(
                    "bir iki üç dört beş altı yedi sekiz",
                    modifier = Modifier.test_tag("uzun"),
                    style = TextStyle { text_align: TextAlign::Center, ..sized(16.0) },
                );
                BasicText(
                    "tek satırda kesilecek kadar uzun bir metin",
                    modifier = Modifier.test_tag("kesik"),
                    style = sized(16.0),
                    max_lines = 1,
                    overflow = TextOverflow::Ellipsis,
                );
                BasicText("satır\nsatır", modifier = Modifier.test_tag("iki satır"), style = sized(16.0));
                BasicText(
                    "kaydırılmayan çok uzun bir satır metni",
                    modifier = Modifier.test_tag("taşan"),
                    style = sized(16.0),
                    soft_wrap = false,
                );
            },
        );
    }));
    let width = |tag: &str| bounds(&rule, tag).width();
    assert_eq!((width("kısa"), width("iki satır")), (4.0 * 8.0, 5.0 * 8.0));
    assert_eq!((width("uzun"), width("kesik"), width("taşan")), (200.0, 200.0, 200.0));
    // Centered in all of it: the lines' middles at 100.
    let c = rule.draw();
    let left = bounds(&rule, "uzun").left;
    let centered = c
        .texts
        .iter()
        .zip(&c.baselines)
        .filter(|(t, b)| t.starts_with("bir ") && near(b.x - left + t.chars().count() as f32 * 8.0 / 2.0, 100.0))
        .count();
    assert_eq!(centered, 1, "{:?} {:?}", c.texts, c.baselines);
}

fn basic_text_takes_nothing_from_around_it() {
    // C++ 6506's BasicText parts (its Material Text is a later stage's): 14 sp black by default, its own style's
    // size and color, an annotated string's the same.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            BasicText("temel");
            BasicText("kendi", style = TextStyle { font_size: 20.sp(), color: RED, ..TextStyle::Default });
            BasicText(AnnotatedString::new("dizi"));
        });
    }));
    let c = rule.draw();
    let (temel, kendi, dizi) = (index(&c, "temel"), index(&c, "kendi"), index(&c, "dizi"));
    assert!(near(c.text_styles[temel].font_size, 14.0) && c.colors[temel] == Color::Black);
    assert!(near(c.text_styles[kendi].font_size, 20.0) && c.colors[kendi] == RED);
    assert!(near(c.text_styles[dizi].font_size, 14.0) && c.colors[dizi] == Color::Black);
}

fn paragraph_styles_give_parts_of_a_text_their_own_settings() {
    // C++ 6526: a paragraph style's range is a paragraph of its own (5 px a letter at 10 sp, lines 12.5).
    let result = Rc::new(RefCell::new(None::<TextLayoutResult>));
    let on_layout = keep(&result);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let text = build_annotated_string(|b| {
            b.append("sol");
            b.with_style(ParagraphStyle { text_align: TextAlign::Center, ..ParagraphStyle::Default }, |b| {
                b.append("orta")
            });
            b.with_style(
                ParagraphStyle {
                    line_height: 30.sp(),
                    line_height_style: Some(LineHeightStyle::new(
                        style::line_height_style::Alignment::Proportional,
                        Trim::None,
                        style::line_height_style::Mode::Fixed,
                    )),
                    text_indent: Some(TextIndent::new(16.sp(), 0.sp())),
                    ..ParagraphStyle::Default
                },
                |b| b.append("girinti"),
            );
            b.with_style(ParagraphStyle { text_direction: TextDirection::Rtl, ..ParagraphStyle::Default }, |b| {
                b.append("abc")
            });
            b.append("\nson");
            // Nested: the inner one wins.
            b.with_style(ParagraphStyle { text_align: TextAlign::Center, ..ParagraphStyle::Default }, |b| {
                b.append("dış");
                b.with_style(ParagraphStyle { text_align: TextAlign::End, ..ParagraphStyle::Default }, |b| {
                    b.append("iç")
                });
            });
            // A block of its own: trimmed both ways.
            b.with_style(ParagraphStyle { line_height: 30.sp(), ..ParagraphStyle::Default }, |b| b.append("kırp"));
        });
        BasicText(
            text,
            modifier = Modifier.width(200.dp()).test_tag("metin"),
            style = sized(10.0),
            on_text_layout = Some(on_layout.clone()),
        );
    }));
    let t = bounds(&rule, "metin");
    let c = rule.draw();
    let x = |s: &str| c.baselines[index(&c, s)].x - t.left;
    let r = result.borrow().clone().expect("laid out");
    // (Compose's: the "\n" opening the plain piece after "abc" gives it an empty first line; C++ took it as the
    // break between them: 8 lines)
    assert_eq!(r.line_count(), 9);
    assert!(near(x("sol"), 0.0) && near(x("orta"), (200.0 - 20.0) / 2.0) && near(x("girinti"), 16.0));
    assert!(near(x("abc"), 200.0 - 15.0) && near(x("son"), 0.0));
    // Its own line height (untrimmed: all 30); the others the font's 12.5.
    assert!(
        near(r.get_line_bottom(2) - r.get_line_top(2), 30.0) && near(r.get_line_bottom(1) - r.get_line_top(1), 12.5)
    );
    // (centered at a whole pixel, as Android's layout: C++ said 92.5)
    assert!(near(x("dış"), ((200.0f32 - 15.0) / 2.0).floor()) && near(x("iç"), 200.0 - 10.0));
    // 30 apart, but nothing to space from.
    assert!(near(r.get_line_bottom(8) - r.get_line_top(8), 12.5));
}

fn annotated_string_styles_wraps_and_links() {
    // C++ 12345: nested styles both apply, two sizes on a line make it as tall as the bigger, a link is clicked
    // where it is. A link has no style of its own (Compose's; C++ underlined it in the primary color).
    let clicks = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0));
    let c2 = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let clicks = c2.clone();
        Column(move || {
            let clicks = clicks.clone();
            let styled = build_annotated_string(|b| {
                b.append("Merhaba ");
                b.with_style(
                    SpanStyle { font_weight: Some(FontWeight::Bold), color: Color::White, ..SpanStyle::Default },
                    |b| {
                        b.append("koyu ");
                        b.with_style(SpanStyle { font_style: Some(FontStyle::Italic), ..SpanStyle::Default }, |b| {
                            b.append("eğik")
                        });
                    },
                );
                b.append(" ve ");
                let clicks = clicks.clone();
                b.with_link(
                    LinkAnnotation::clickable("bağ", move |_| {
                        clicks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }),
                    |b| b.append("bağlantı"),
                );
            });
            BasicText(styled, modifier = Modifier.test_tag("styled"), style = sized(16.0));
            let mixed = build_annotated_string(|b| {
                b.append("küçük ");
                b.with_style(SpanStyle { font_size: 32.sp(), ..SpanStyle::Default }, |b| b.append("BÜYÜK"));
            });
            BasicText(mixed, modifier = Modifier.test_tag("mixed"), style = sized(16.0));
        });
    }));
    let c = rule.draw();
    assert_eq!(c.texts, ["Merhaba ", "koyu ", "eğik", " ve bağlantı", "küçük ", "BÜYÜK"]);
    assert!(c.text_styles[1].font_weight == FontWeight::Bold && c.text_styles[1].font_style == FontStyle::Normal);
    assert!(c.text_styles[2].font_weight == FontWeight::Bold && c.text_styles[2].font_style == FontStyle::Italic);
    assert!(c.colors[2] == Color::White && c.colors[3] == Color::Black); // a link: the text's own color
    assert_eq!(bounds(&rule, "mixed").height(), 40.0); // 32 * 1.25
    // "Merhaba koyu eğik ve " is 21 characters, the link follows.
    let styled = bounds(&rule, "styled");
    tap(&rule, Offset::new(styled.left + 8.0 * 23.0, styled.top + 10.0)); // on "bağlantı"
    rule.wait_for_idle();
    tap(&rule, Offset::new(styled.left + 8.0 * 3.0, styled.top + 10.0)); // on "Merhaba"
    rule.wait_for_idle();
    assert_eq!(clicks.load(std::sync::atomic::Ordering::Relaxed), 1);
}

fn text_tells_how_it_was_laid_out() {
    // C++ 23980: `on_text_layout` told only when the layout comes out anew, not when the text composes again.
    let calls = Rc::new(Cell::new(0));
    let last = Rc::new(RefCell::new(None::<TextLayoutResult>));
    let width = mutable_state_of(100.0f32);
    let cut = mutable_state_of(false);
    let other = mutable_state_of(0);
    let (c2, l2, w2, cut2, o2) = (calls.clone(), last.clone(), width.clone(), cut.clone(), other.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (calls, last, width, cut, other) = (c2.clone(), l2.clone(), w2.clone(), cut2.clone(), o2.clone());
        Column(move || {
            other.get();
            let (calls, last, cut) = (calls.clone(), last.clone(), cut.clone());
            // A new closure each time it composes.
            let on_layout: OnTextLayout = Rc::new(move |r: &TextLayoutResult| {
                calls.set(calls.get() + 1);
                *last.borrow_mut() = Some(r.clone());
                cut.set(r.has_visual_overflow());
            });
            BasicText(
                "bir iki üç dört beş altı yedi sekiz dokuz on",
                modifier = Modifier.width(width.get().dp()),
                style = sized(10.0),
                max_lines = 2,
                overflow = TextOverflow::Ellipsis,
                on_text_layout = Some(on_layout),
            );
            if cut.get() {
                BasicText("devamını oku");
            }
        });
    }));
    {
        let r = last.borrow().clone().expect("laid out");
        assert!(calls.get() == 1 && r.line_count() == 2 && r.is_line_ellipsized(1));
    }
    rule.on_node_with_text("devamını oku").assert_exists();
    other.set(1); // composed again, laid out the same: not told again
    rule.wait_for_idle();
    assert_eq!(calls.get(), 1);
    width.set(400.0); // all of it fits now
    rule.wait_for_idle();
    let r = last.borrow().clone().expect("laid out");
    assert!(calls.get() == 2 && r.line_count() == 1 && !r.has_visual_overflow());
    rule.on_node_with_text("devamını oku").assert_does_not_exist();
    // As wide as its node.
    assert_eq!(r.get_line_for_offset(r.layout_input.text.len()), 0);
    assert_eq!(r.size.width, 400);
}

fn min_lines_start_and_middle_ellipsis_and_baseline_shift() {
    // C++ 30070.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            BasicText("a", modifier = Modifier.test_tag("üç"), style = sized(10.0), min_lines = 3);
            BasicText("a\nb\nc\nd", modifier = Modifier.test_tag("dört"), style = sized(10.0), min_lines = 3);
            BasicText(
                "abcdefghij",
                modifier = Modifier.width(30.dp()),
                style = sized(10.0),
                max_lines = 1,
                overflow = TextOverflow::StartEllipsis,
            );
            BasicText(
                "klmnopqrst",
                modifier = Modifier.width(30.dp()),
                style = sized(10.0),
                overflow = TextOverflow::MiddleEllipsis,
                soft_wrap = false,
            );
            // Wraps: clipped.
            BasicText(
                "uvw xyz uvw",
                modifier = Modifier.width(30.dp()).test_tag("çok"),
                style = sized(10.0),
                overflow = TextOverflow::MiddleEllipsis,
            );
            let shifted = build_annotated_string(|b| {
                b.append("x");
                b.with_style(
                    SpanStyle { baseline_shift: Some(BaselineShift::Superscript), ..SpanStyle::Default },
                    |b| b.append("2"),
                );
                b.append("H");
                b.with_style(SpanStyle { baseline_shift: Some(BaselineShift::Subscript), ..SpanStyle::Default }, |b| {
                    b.append("o")
                });
            });
            BasicText(shifted, modifier = Modifier.test_tag("simge"), style = sized(10.0));
        });
    }));
    assert_eq!(bounds(&rule, "üç").height(), 38.0); // three lines of 12.5
    assert_eq!(bounds(&rule, "dört").height(), 50.0); // more than that: as many as it has
    let c = rule.draw();
    let has = |t: &str| c.texts.iter().any(|x| x == t);
    assert!(has("…") && has("fghij"), "{:?}", c.texts); // the start cut: its end kept
    assert!(has("klm") && has("st"), "{:?}", c.texts); // the middle cut: the start gets the odd one
    assert!(bounds(&rule, "çok").height() == 38.0 && !has("uvw…")); // more lines: no cut inside
    // Superscript: half its ascent (4) up, the line taller above; subscript: 4 down, taller below.
    let top = bounds(&rule, "simge").top;
    let base = |t: &str| c.baselines[index(&c, t)].y - top;
    assert!(near(base("x"), 12.0) && near(base("2"), 8.0) && near(base("H"), 12.0) && near(base("o"), 16.0));
}

fn annotated_string_builder_and_html_draw_their_styles() {
    // The drawing halves of C++ 6559 (the inner style on top of the outer) and 6395 (a Text of HTML draws the bold
    // word bold).
    let s = build_annotated_string(|b| {
        b.append("a ");
        b.push_style(SpanStyle { font_weight: Some(FontWeight::Bold), ..SpanStyle::Default });
        b.append("kalın ");
        let italic = b.push_style(SpanStyle { font_style: Some(FontStyle::Italic), ..SpanStyle::Default });
        b.append("eğik");
        b.push_string_annotation("url", "https://kompoz.dev");
        b.append(" bağ");
        b.pop_to(italic);
        b.append(" son");
    });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(|| {
            BasicText(s.clone());
            BasicText(AnnotatedString::from_html("düz <b>kalın</b>"));
        });
    }));
    let c = rule.draw();
    let at = |prefix: &str| c.texts.iter().position(|t| t.starts_with(prefix)).expect(prefix);
    let (both, bold_only) = (at("eğik"), at(" son"));
    assert!(c.text_styles[both].font_weight == FontWeight::Bold && c.text_styles[both].font_style == FontStyle::Italic);
    assert!(
        c.text_styles[bold_only].font_weight == FontWeight::Bold
            && c.text_styles[bold_only].font_style == FontStyle::Normal
    );
    let html_bold = c.texts.iter().rposition(|t| t == "kalın").expect("kalın");
    assert_eq!(c.text_styles[html_bold].font_weight, FontWeight::Bold);
}

fn min_intrinsic_widths_of_one_line_and_styled_text() {
    // C++ 6449's parts the test Text couldn't show: max lines left aside for the min intrinsic width, and a styled
    // text's big word at its own size (Compose's min intrinsic width keeps a word's trailing space: "kelimeler "
    // is 80; C++'s 72 left it out).
    let styled = build_annotated_string(|b| {
        b.append("ince ");
        b.with_style(SpanStyle { font_size: 32.sp(), ..SpanStyle::Default }, |b| b.append("KALIN"));
        b.append(" ince");
    });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let styled = styled.clone();
        Column(move || {
            Column(
                Modifier.width_intrinsic(IntrinsicSize::Min).test_tag("dar"),
                content = || {
                    BasicText("kısa kelimeler burada", style = sized(16.0), max_lines = 1);
                    Box(Modifier.fill_max_width().height(2.dp()).test_tag("çizgi"));
                },
            );
            let styled = styled.clone();
            Box(
                Modifier.width_intrinsic(IntrinsicSize::Min).test_tag("biçimli"),
                content = move || {
                    BasicText(styled.clone(), style = sized(16.0));
                },
            );
        });
    }));
    assert_eq!(bounds(&rule, "dar").width(), 10.0 * 8.0);
    assert_eq!(bounds(&rule, "çizgi").width(), 10.0 * 8.0);
    assert_eq!(bounds(&rule, "biçimli").width(), 5.0 * 16.0 + 8.0); // "KALIN " (its space in the text's size)
    // Laid out for real at that width: one line, cut.
    assert_eq!(bounds(&rule, "dar").height(), 20.0 + 2.0);
}

fn text_sizes_in_em_are_shares_of_a_font_size() {
    // C++ 17810's parts without ProvideTextStyle (Material's): letter spacing in em of its own size, a span's em
    // size of the text's, a span's letter spacing of its own size, a line height in em.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            BasicText("aralıklı", style = TextStyle { letter_spacing: 0.1.em(), ..sized(20.0) });
            let text = build_annotated_string(|b| {
                b.append("ana");
                b.with_style(SpanStyle { font_size: 0.5.em(), ..SpanStyle::Default }, |b| b.append("küçük"));
                b.with_style(SpanStyle { font_size: 30.sp(), letter_spacing: 0.1.em(), ..SpanStyle::Default }, |b| {
                    b.append("geniş")
                });
            });
            BasicText(text, style = sized(20.0));
            BasicText("satır", style = TextStyle { line_height: 1.5.em(), ..sized(10.0) });
        });
    }));
    let c = rule.draw();
    let style = |t: &str| c.text_styles[index(&c, t)].clone();
    assert!(near(style("aralıklı").font_size, 20.0) && near(style("aralıklı").letter_spacing, 2.0));
    assert!(near(style("ana").font_size, 20.0) && near(style("küçük").font_size, 10.0));
    assert!(near(style("geniş").font_size, 30.0) && near(style("geniş").letter_spacing, 3.0));
    assert_eq!(style("satır").line_height, Some(15.0));
}

fn texts_line_up_by_their_last_baselines() {
    // C++ 613's part without text fields: a text lined up by its last baseline with another's of three lines.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Row(|| {
            BasicText("son", modifier = Modifier.align_by(LastBaseline), style = sized(16.0));
            BasicText("bir\niki\nüç", modifier = Modifier.align_by(LastBaseline), style = sized(16.0));
            BasicText("düz", modifier = Modifier.align_by_baseline(), style = sized(16.0));
        });
    }));
    let c = rule.draw();
    let y = |t: &str| c.baselines[index(&c, t)].y;
    assert!(near(y("son"), y("üç")) && y("üç") > y("bir") + 39.0);
    assert!(near(y("düz"), y("son"))); // its first baseline where the others' last are
}

// ---- Auto size -------------------------------------------------------------------------------------------------

fn auto_size_text_takes_the_largest_size_that_fits() {
    // C++ 14056: half the size a character, lines 1.25 times the size.
    let step = || <dyn TextAutoSize>::step_based(8.sp(), 64.sp(), 1.sp());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(|| {
            // The height decides: 32.
            BasicText("Merhaba", modifier = Modifier.size_wh(200.dp(), 40.dp()), auto_size = Some(step()));
            // The width: 28.
            BasicText("Merhaba", modifier = Modifier.width(100.dp()), max_lines = 1, auto_size = Some(step()));
            // Nothing fits: the smallest.
            BasicText("Merhaba", modifier = Modifier.width(10.dp()), max_lines = 1, auto_size = Some(step()));
            // Room for the largest.
            BasicText("Merhaba", modifier = Modifier.size(400.dp()), auto_size = Some(step()));
        });
    }));
    let c = rule.draw();
    let sizes: Vec<f32> =
        c.texts.iter().zip(&c.text_styles).filter(|(t, _)| t.starts_with("Me")).map(|(_, s)| s.font_size).collect();
    assert_eq!(sizes, [32.0, 28.0, 8.0, 64.0]);
}

fn auto_size_judges_overflow_as_its_overflow_says() {
    // Compose's didOverflow (D11): an ellipsized text fits where it isn't ellipsized, a clipped one within its
    // bounds; a start or middle ellipsis on more lines falls back to the bounds.
    let step = || <dyn TextAutoSize>::step_based(8.sp(), 64.sp(), 1.sp());
    let laid = Rc::new(RefCell::new(Vec::<TextLayoutResult>::new()));
    let l = laid.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Column(move || {
            let l2 = l.clone();
            let told: OnTextLayout = Rc::new(move |r: &TextLayoutResult| l2.borrow_mut().push(r.clone()));
            // One line ellipsized at its end: 28 is the largest not ellipsized (7 × 14 = 98).
            BasicText(
                "Merhaba",
                modifier = Modifier.width(100.dp()),
                max_lines = 1,
                overflow = TextOverflow::Ellipsis,
                auto_size = Some(step()),
                on_text_layout = Some(told.clone()),
            );
            // Ellipsized in the middle on two lines: by its bounds (two lines in 40 px: 16 at most).
            BasicText(
                "Merhaba dünya",
                modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("orta"),
                overflow = TextOverflow::MiddleEllipsis,
                auto_size = Some(step()),
            );
        });
    }));
    let c = rule.draw();
    let first = c.texts.iter().position(|t| t.starts_with("Me")).expect("drawn");
    assert!(near(c.text_styles[first].font_size, 28.0) && !c.texts[first].contains('…'));
    let r = laid.borrow().last().cloned().expect("told");
    assert!(near(r.layout_input.style.font_size.value(), 28.0) && !r.is_line_ellipsized(0));
    let second = c.texts.iter().rposition(|t| t.starts_with("Me")).expect("drawn");
    assert!(near(c.text_styles[second].font_size, 16.0), "{}", c.text_styles[second].font_size);
    assert_eq!(bounds(&rule, "orta").height(), 40.0);
}

/// An auto size of its own: the largest of `sizes` (em of the style's size) that keeps the text on one line.
#[derive(PartialEq, Debug)]
struct OneLineOf(Vec<f32>);

impl TextAutoSize for OneLineOf {
    fn get_font_size(
        &self,
        scope: &mut TextAutoSizeLayoutScope<'_>,
        constraints: Constraints,
        text: &AnnotatedString,
    ) -> TextUnit {
        let mut best = self.0[0].em();
        for &k in &self.0 {
            let r = scope.perform_layout(constraints, text, k.em());
            if r.line_count() == 1 && !r.has_visual_overflow() {
                best = k.em();
            }
        }
        // (an em length in px: of the style's size)
        assert!(near(scope.text_to_px(1.em()), 10.0) && near(scope.density, 1.0));
        best
    }
}

fn an_auto_size_of_ones_own_lays_out_through_its_scope() {
    // A TextAutoSize of its own: em sizes of the style's (10 sp), the largest that keeps the text on one line in
    // 100 px ("dört kelime" is 11 characters: 1.8 em is 99 px).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        BasicText(
            "dört kelime",
            modifier = Modifier.width(100.dp()).test_tag("kendi"),
            style = sized(10.0),
            auto_size = Some(Rc::new(OneLineOf(vec![1.0, 1.5, 1.8, 2.5, 3.0])) as Rc<dyn TextAutoSize>),
        );
    }));
    let c = rule.draw();
    assert!(near(c.text_styles[index(&c, "dört kelime")].font_size, 18.0));
    assert_eq!(bounds(&rule, "kendi").height(), 23.0); // 22.5, whole pixels
    // Auto sizes are compared by value.
    let a: Rc<dyn TextAutoSize> = <dyn TextAutoSize>::step_based(8.sp(), 64.sp(), 1.sp());
    let b: Rc<dyn TextAutoSize> = <dyn TextAutoSize>::step_based(8.sp(), 64.sp(), 1.sp());
    let o: Rc<dyn TextAutoSize> = Rc::new(OneLineOf(vec![1.0]));
    assert!(*a == *b && *a != *o);
}

// ---- Inline content --------------------------------------------------------------------------------------------

fn inline_content_takes_its_room_in_the_text() {
    // C++ 6307: 16 sp, 8 px a code point, 12.8 above the baseline, 3.2 below, lines 20 apart.
    let told = Rc::new(RefCell::new(Vec::<String>::new()));
    let content = |w: TextUnit, h: TextUnit, align: PlaceholderVerticalAlign, tag: &'static str| {
        let told = told.clone();
        InlineTextContent::new(
            Placeholder::new(w, h, align),
            kompozable_lambda!(move |alternate: &str| {
                told.borrow_mut().push(alternate.to_string());
                Box(Modifier.fill_max_size().test_tag(tag));
            }),
        )
    };
    use PlaceholderVerticalAlign::*;
    let mut contents = InlineContentMap::new();
    for (id, c) in [
        ("yıldız", content(20.sp(), 20.sp(), AboveBaseline, "yıldız")),
        ("geniş", content(1.em(), 0.5.em(), AboveBaseline, "geniş")),
        ("üst", content(10.sp(), 10.sp(), TextTop, "üst")),
        ("alt", content(10.sp(), 10.sp(), TextBottom, "alt")),
        ("orta", content(10.sp(), 10.sp(), TextCenter, "orta")),
        ("taban", content(10.sp(), 10.sp(), AboveBaseline, "taban")),
        ("uzun", content(30.sp(), 30.sp(), Center, "uzun")),
        ("bütün", content(24.sp(), 10.sp(), AboveBaseline, "bütün")),
        ("sağdan", content(40.sp(), 10.sp(), AboveBaseline, "sağdan")),
        ("sonra", content(10.sp(), 10.sp(), AboveBaseline, "sonra")),
    ] {
        contents.insert(id.to_string(), c);
    }
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let contents = contents.clone();
        Column(move || {
            let style = sized(16.0);
            let line = build_annotated_string(|b| {
                b.append("ab");
                b.add_string_annotation("başka", "yıldız", 0, 2); // not inline content: only its own tag is
                b.append_inline_content_with("yıldız", "[yıldız]");
                // A link: its own node comes first.
                b.with_link(LinkAnnotation::clickable_with("bağ", None, None), |b| b.append("cd"));
            });
            BasicText(
                line,
                modifier = Modifier.test_tag("satır"),
                style = style.clone(),
                inline_content = contents.clone(),
            );
            let em = build_annotated_string(|b| {
                b.append("x");
                // Em of the size there.
                b.with_style(SpanStyle { font_size: 32.sp(), ..SpanStyle::Default }, |b| {
                    b.append_inline_content_with("geniş", "\u{FFFD}")
                });
            });
            BasicText(em, modifier = Modifier.test_tag("em"), style = style.clone(), inline_content = contents.clone());
            for id in ["üst", "alt", "orta", "taban", "uzun"] {
                BasicText(
                    around(id, "?", "ab", ""),
                    modifier = Modifier.test_tag(format!("{id} metni")),
                    style = style.clone(),
                    inline_content = contents.clone(),
                );
            }
            // None for it: the alternate text shows.
            BasicText(
                around("yok", "YOK", "ab", ""),
                modifier = Modifier.test_tag("yok"),
                style = style.clone(),
                inline_content = contents.clone(),
            );
            let (c2, s2) = (contents.clone(), style.clone());
            Box(
                Modifier.width(16.dp()),
                content = move || {
                    BasicText(
                        around("bütün", "[a b]", "", ""),
                        modifier = Modifier.test_tag("dar"),
                        style = s2.clone(),
                        inline_content = c2.clone(),
                    );
                },
            );
            BasicText(
                around("sağdan", "[ab]", "שלום ", ""),
                modifier = Modifier.test_tag("rtl"),
                style = style.clone(),
                inline_content = contents.clone(),
            );
            // On the second line, past max lines: not placed.
            let (c3, s3) = (contents.clone(), style.clone());
            Box(
                Modifier.width(24.dp()),
                content = move || {
                    BasicText(
                        around("sonra", "?", "ab ", ""),
                        modifier = Modifier.test_tag("kesik"),
                        style = s3.clone(),
                        max_lines = 1,
                        inline_content = c3.clone(),
                    );
                },
            );
        });
    }));
    let room = |tag: &str, text: &str| {
        let (r, t) = (bounds(&rule, tag), bounds(&rule, text));
        [r.left - t.left, r.top - t.top, r.width(), r.height()]
    };
    // After "ab", its bottom on the baseline; the line grown to hold it.
    assert_eq!(room("yıldız", "satır"), [16.0, 0.0, 20.0, 20.0]);
    let line = bounds(&rule, "satır");
    assert_eq!(line.width(), 16.0 + 20.0 + 16.0);
    assert_eq!(rule.on_node_with_tag("satır").layout_node().coordinates().get(FirstBaseline), 20);
    // The line as its text there (32 sp) needs it, at whole pixels (C++ placed it at 9.6).
    assert_eq!(room("geniş", "em"), [8.0, 10.0, 32.0, 16.0]);
    assert_eq!(room("üst", "üst metni"), [16.0, 0.0, 10.0, 10.0]); // its top at the text's
    assert_eq!(room("alt", "alt metni"), [16.0, 6.0, 10.0, 10.0]); // its bottom at the text's
    assert_eq!(room("orta", "orta metni"), [16.0, 3.0, 10.0, 10.0]); // on the text's middle (4.8 up)
    assert_eq!(room("taban", "taban metni"), [16.0, 3.0, 10.0, 10.0]); // 2.8, whole pixels
    // A tall one in the middle: the line grown both ways.
    assert_eq!(room("uzun", "uzun metni"), [16.0, 0.0, 30.0, 30.0]);
    assert_eq!(bounds(&rule, "uzun metni").height(), 30.0);
    assert_eq!(bounds(&rule, "taban metni").height(), 20.0); // a small one: the line as it was
    // Not drawn: the alternate texts, which the contents are told instead.
    let drawn = rule.drawn_texts().join("|");
    assert!(!drawn.contains("yıldız") && !drawn.contains('?') && drawn.contains("YOK"), "{drawn}");
    assert!(told.borrow().iter().any(|t| t == "[yıldız]") && told.borrow().iter().any(|t| t == "\u{FFFD}"));
    // Never broken, even wider than its line, or among both directions.
    assert_eq!(bounds(&rule, "dar").height(), 20.0);
    assert_eq!(room("bütün", "dar"), [0.0, 3.0, 24.0, 10.0]);
    assert_eq!(room("sağdan", "rtl")[2], 40.0);
    assert_eq!(bounds(&rule, "rtl").width(), 5.0 * 8.0 + 40.0);
    assert!(bounds(&rule, "kesik").height() < 21.0);
    assert!(!rule.on_node_with_tag("sonra").layout_node().is_placed());
    // sp: grown with the font scale (the text's size too: 16 px a character).
    rule.host().set_density(Density::new(1.0, 2.0));
    rule.wait_for_idle();
    assert_eq!(room("yıldız", "satır"), [32.0, 0.0, 40.0, 40.0]);
}

/// `before`, inline content `id` (its alternate text `alternate`), `after`.
fn around(id: &str, alternate: &str, before: &str, after: &str) -> AnnotatedString {
    build_annotated_string(|b| {
        b.append(before);
        b.append_inline_content_with(id, alternate);
        b.append(after);
    })
}

// ---- The text node ---------------------------------------------------------------------------------------------

fn only_the_drawing_changes_with_the_color() {
    // A color (a producer read as it draws, or the style's) draws again without laying out again: the same layout,
    // the same glyph runs; a size lays out again.
    let laid = Rc::new(Cell::new(0));
    let color = mutable_state_of(RED);
    let style_color = mutable_state_of(Color::Black);
    let size = mutable_state_of(16.0f32);
    let (l2, c2, s2, z2) = (laid.clone(), color.clone(), style_color.clone(), size.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l2, c2, s2, z2) = (l2.clone(), c2.clone(), s2.clone(), z2.clone());
        let producer = remember(move || ColorProducer::new(move || c2.get()));
        let told: OnTextLayout = Rc::new(move |_: &TextLayoutResult| l2.set(l2.get() + 1));
        Column(move || {
            BasicText("üretilen", color = Some(producer.clone()), on_text_layout = Some(told.clone()));
            BasicText("biçemli", style = TextStyle { color: s2.get(), font_size: z2.get().sp(), ..TextStyle::Default });
        });
    }));
    let before = rule.draw();
    let (a, b) = (index(&before, "üretilen"), index(&before, "biçemli"));
    assert!(before.colors[a] == RED && before.colors[b] == Color::Black && laid.get() == 1);
    color.set(Color::Blue);
    style_color.set(Color::Green);
    rule.wait_for_idle();
    let after = rule.draw();
    assert!(after.colors[a] == Color::Blue && after.colors[b] == Color::Green);
    assert_eq!(laid.get(), 1);
    assert!(Rc::ptr_eq(&before.text_draws[a].glyphs, &after.text_draws[a].glyphs));
    assert!(Rc::ptr_eq(&before.text_draws[b].glyphs, &after.text_draws[b].glyphs));
    size.set(20.0);
    rule.wait_for_idle();
    let resized = rule.draw();
    assert!(!Rc::ptr_eq(&after.text_draws[b].glyphs, &resized.text_draws[b].glyphs));
    assert!(near(resized.text_styles[b].font_size, 20.0));
}

fn intrinsics_come_from_the_text() {
    // Compose's intrinsics: the min width the widest piece between line breaks (its trailing space with it), the
    // max width all of it on a line, the height laid out in a width (min lines' at least), for a plain string and
    // an annotated one.
    let seen = Rc::new(RefCell::new(Vec::<[i32; 4]>::new()));
    let s2 = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s2 = s2.clone();
        Layout(
            content = || {
                BasicText("bir iki üçüncü", style = sized(16.0));
                BasicText(AnnotatedString::new("bir iki üçüncü"), style = sized(16.0), min_lines = 4);
            },
            measure_policy = measure_policy(move |scope, ms, _| {
                s2.borrow_mut().clear();
                for m in ms {
                    s2.borrow_mut().push([
                        m.min_intrinsic_width(0),
                        m.max_intrinsic_width(0),
                        m.min_intrinsic_height(48),
                        m.max_intrinsic_height(1000),
                    ]);
                }
                scope.layout(10, 10, |_| {})
            }),
        );
    }));
    let seen = seen.borrow();
    // "üçüncü" is 6 characters, all of it 14; in 48 px three lines of 20, on one line 20 (four lines at least for
    // the second: 4 × 20).
    assert_eq!(seen[0], [48, 112, 60, 20]);
    assert_eq!(seen[1], [48, 112, 80, 80]);
}

fn text_substitution_shows_another_text_until_cleared() {
    // Compose's text substitution (a translation): set, shown, hidden, cleared through semantics, for a plain
    // string's node and an annotated string's.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            BasicText("merhaba", modifier = Modifier.test_tag("düz"), style = sized(16.0));
            BasicText(AnnotatedString::new("merhaba"), modifier = Modifier.test_tag("dizi"), style = sized(16.0));
        });
    }));
    for tag in ["düz", "dizi"] {
        let node = rule.on_node_with_tag(tag);
        node.perform_semantics_action_with(SemanticsActions::SetTextSubstitution, |f| {
            assert!(f(AnnotatedString::new("hello there")));
        });
        let config = node.fetch_semantics_node().config();
        let substitution = config.get(SemanticsProperties::TextSubstitution).map(|t| t.text().to_string());
        assert_eq!(substitution.as_deref(), Some("hello there"));
        assert_eq!(config.get(SemanticsProperties::IsShowingTextSubstitution), Some(false));
        assert_eq!(bounds(&rule, tag).width(), 7.0 * 8.0);
        node.perform_semantics_action_with(SemanticsActions::ShowTextSubstitution, |f| assert!(f(true)));
        assert!(rule.drawn_texts().iter().any(|t| t == "hello there"));
        assert_eq!(bounds(&rule, tag).width(), 11.0 * 8.0);
        let config = node.fetch_semantics_node().config();
        assert_eq!(config.get(SemanticsProperties::IsShowingTextSubstitution), Some(true));
        // Its text is still its own.
        node.assert_text_equals(&["merhaba"]);
        node.perform_semantics_action_with(SemanticsActions::ShowTextSubstitution, |f| assert!(f(false)));
        assert_eq!(bounds(&rule, tag).width(), 7.0 * 8.0);
        node.perform_semantics_action(SemanticsActions::ClearTextSubstitution);
        assert!(!node.fetch_semantics_node().config().contains(SemanticsProperties::TextSubstitution));
        // Nothing to show.
        node.perform_semantics_action_with(SemanticsActions::ShowTextSubstitution, |f| assert!(!f(true)));
        assert!(!rule.drawn_texts().iter().any(|t| t == "hello there"));
    }
}

fn a_text_hands_out_its_layout_in_its_color() {
    // GetTextLayoutResult: the layout drawn, in the style with the color drawn in (a plain string's laid out again
    // as Compose's, in the width it was drawn in; an annotated string's the one kept).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            BasicText(
                "bir iki üç",
                modifier = Modifier.width(64.dp()).test_tag("düz"),
                style = TextStyle { text_align: TextAlign::Center, ..sized(16.0) },
                color = Some(remember(|| ColorProducer::new(|| RED))),
            );
            BasicText(
                AnnotatedString::new("bir iki üç"),
                modifier = Modifier.width(64.dp()).test_tag("dizi"),
                style = sized(16.0),
                color = Some(remember(|| ColorProducer::new(|| RED))),
            );
        });
    }));
    for tag in ["düz", "dizi"] {
        let mut layouts: Vec<TextLayoutResult> = Vec::new();
        rule.on_node_with_tag(tag).perform_semantics_action_with(SemanticsActions::GetTextLayoutResult, |f| {
            assert!(f(&mut layouts));
        });
        let r = &layouts[0];
        assert_eq!((r.line_count(), r.size, r.layout_input.style.color), (2, IntSize::new(64, 40), RED));
    }
    // Its lines where they are drawn: centered in the 64 px it was laid out in.
    let mut layouts = Vec::new();
    rule.on_node_with_tag("düz")
        .perform_semantics_action_with(SemanticsActions::GetTextLayoutResult, |f| assert!(f(&mut layouts)));
    let c = rule.draw();
    let left = bounds(&rule, "düz").left;
    assert!(near(layouts[0].get_line_left(1), c.baselines[index(&c, "üç")].x - left));
}

// ---- Real fonts ------------------------------------------------------------------------------------------------

/// A resolver over kompoz-text's test fonts: Noto Sans by default, Noto Sans Arabic, Devanagari and Hebrew for their
/// scripts, Noto Color Emoji for emoji.
fn test_fonts() -> Rc<dyn FontFamilyResolver> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kompoz-text/tests/fonts");
    let resolver = Rc::new(FontFamilyResolverImpl::registered_only());
    let c = resolver.collection();
    for f in [
        "NotoSans-Subset.ttf",
        "NotoSansArabic-Subset.ttf",
        "NotoSansDevanagari-Subset.ttf",
        "NotoSansHebrew-Subset.ttf",
        "NotoColorEmoji-Subset.ttf",
    ] {
        c.register_file(dir.join(f)).expect("a test font");
    }
    c.set_generic_families(GenericFontFamily::SansSerif, &["Noto Sans"]);
    c.set_emoji_families(&["Noto Color Emoji"]);
    c.set_fallback_families(*b"Arab", None, &["Noto Sans Arabic"]);
    c.set_fallback_families(*b"Deva", None, &["Noto Sans Devanagari"]);
    c.set_fallback_families(*b"Hebr", None, &["Noto Sans Hebrew"]);
    resolver
}

fn basic_text_in_real_fonts() {
    // Arabic joined and right to left, Hindi reordered (fewer glyphs than characters) on a line grown for its font,
    // emoji one color glyph, each in its font, through BasicText's nodes.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            BasicText("ab", modifier = Modifier.test_tag("latin"), style = sized(20.0));
            BasicText("مرحبا بالعالم", modifier = Modifier.test_tag("arabic"), style = sized(20.0));
            BasicText("ح", style = sized(20.0));
            BasicText("नमस्ते", modifier = Modifier.test_tag("hindi"), style = sized(20.0));
            BasicText("ab 👨‍👩‍👧", modifier = Modifier.test_tag("emoji"), style = sized(20.0));
        });
    }));
    rule.host().set_font_family_resolver(test_fonts());
    rule.wait_for_idle();
    let c = rule.draw();
    let draw = |t: &str| c.text_draws[index(&c, t)].clone();
    let arabic = draw("مرحبا بالعالم");
    assert!(arabic.rtl && arabic.glyphs.len() == 1);
    // Joined: "ح" inside a word is drawn in another form than alone.
    let alone = draw("ح").glyphs[0].glyphs[0].id;
    assert!(!arabic.glyphs[0].glyphs.iter().any(|g| g.id == alone));
    assert_ne!(arabic.glyphs[0].font, draw("ab").glyphs[0].font);
    // As wide as its glyphs, from its left.
    let at = bounds(&rule, "arabic");
    assert!(at.width() > 0.0 && (arabic.baseline.x - at.left).abs() < 1.0);
    let hindi = draw("नमस्ते");
    assert!(hindi.glyphs[0].glyphs.len() < "नमस्ते".chars().count());
    assert!(bounds(&rule, "hindi").height() > bounds(&rule, "latin").height());
    let emoji = &c.text_draws[c.texts.iter().position(|t| t.starts_with("ab ")).expect("drawn")];
    let color: Vec<&GlyphRun> = emoji.glyphs.iter().filter(|g| g.color == GlyphColorKind::Bitmap).collect();
    assert!(color.len() == 1 && color[0].glyphs.len() == 1);
    rule.on_node_with_tag("arabic").assert_text_equals(&["مرحبا بالعالم"]);
}

tests!(
    text_wraps_to_width,
    text_overflow_soft_wrap_and_max_lines,
    text_that_gives_way_to_the_width_takes_all_of_it,
    basic_text_takes_nothing_from_around_it,
    paragraph_styles_give_parts_of_a_text_their_own_settings,
    annotated_string_styles_wraps_and_links,
    text_tells_how_it_was_laid_out,
    min_lines_start_and_middle_ellipsis_and_baseline_shift,
    annotated_string_builder_and_html_draw_their_styles,
    min_intrinsic_widths_of_one_line_and_styled_text,
    text_sizes_in_em_are_shares_of_a_font_size,
    texts_line_up_by_their_last_baselines,
    auto_size_text_takes_the_largest_size_that_fits,
    auto_size_judges_overflow_as_its_overflow_says,
    an_auto_size_of_ones_own_lays_out_through_its_scope,
    inline_content_takes_its_room_in_the_text,
    only_the_drawing_changes_with_the_color,
    intrinsics_come_from_the_text,
    text_substitution_shows_another_text_until_cleared,
    a_text_hands_out_its_layout_in_its_color,
    basic_text_in_real_fonts,
);

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, kompoz_ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}
