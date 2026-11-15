//! AnnotatedString: built (the C++ Kompoz's annotated_string_builder_pushes_styles_and_annotations), queried as
//! Compose's annotation intersection says, cut, joined, its paragraphs normalised, its case mapped; read from HTML
//! (annotated_strings_from_html, with Compose's link semantics); saved; a text field's value; text ranges; locales.

use kompoz_graphics::*;
use kompoz_runtime::{Saveable, SavedValue, kompozable_block};
use kompoz_text::annotated_string::{contains, intersect};
use kompoz_text::input::TextFieldValue as TextFieldValueFn;
use kompoz_text::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

fn panics(f: impl FnOnce()) -> bool {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = catch_unwind(AssertUnwindSafe(f)).is_err();
    std::panic::set_hook(previous);
    r
}

fn bold() -> SpanStyle {
    SpanStyle { font_weight: Some(FontWeight::Bold), ..SpanStyle::Default }
}
fn italic() -> SpanStyle {
    SpanStyle { font_style: Some(FontStyle::Italic), ..SpanStyle::Default }
}

/// The span styles over all of `word` (its first occurrence) merged in their order, if any.
fn span_of(a: &AnnotatedString, word: &str) -> Option<SpanStyle> {
    let at = a.text().find(word).expect("the word");
    let end = at + word.len();
    let covering: Vec<&Range<SpanStyle>> = a.span_styles().iter().filter(|s| s.start <= at && s.end >= end).collect();
    (!covering.is_empty()).then(|| covering.iter().fold(SpanStyle::Default, |acc, s| acc.merge(Some(&s.item))))
}

/// The style the run over `word` is drawn in (its first occurrence), the spans over it in their order.
fn run_style(a: &AnnotatedString, word: &str) -> TextRunStyle {
    let at = a.text().find(word).expect("the word");
    let end = at + word.len();
    let spans: Vec<&SpanStyle> =
        a.span_styles().iter().filter(|s| s.start <= at && s.end >= end).map(|s| &s.item).collect();
    TextRunStyle::resolve(&TextStyle::Default, &spans, Density::default())
}

#[test]
fn annotated_string_builder_pushes_styles_and_annotations() {
    let s = build_annotated_string(|b| {
        b.append("a ");
        b.push_style(bold());
        b.append("kalın ");
        let italic_at = b.push_style(italic());
        b.append("eğik");
        b.push_string_annotation("url", "https://kompoz.dev");
        b.append(" bağ");
        b.pop_to(italic_at); // the annotation and the italic end here
        b.append(" son");
        b.with_annotation("id", "7", |b| {
            b.append("!");
        });
        // the bold one is still pushed: it ends with the text
    });
    assert_eq!(s.text(), "a kalın eğik bağ son!");
    let has = |start: usize, end: usize, pred: &dyn Fn(&SpanStyle) -> bool| {
        s.span_styles().iter().any(|r| r.start == start && r.end == end && pred(&r.item))
    };
    assert!(has(9, 19, &|st| st.font_style == Some(FontStyle::Italic)));
    assert!(has(2, 24, &|st| st.font_weight == Some(FontWeight::Bold)));
    // Pushed ones go in when pushed: the outer bold before the italic inside it.
    assert_eq!(s.span_styles()[0].item, bold());
    // Looked up where the text was tapped: an offset finds the annotation around it (not at its end).
    let url = s.get_string_annotations("url", 16, 16);
    assert!(url.len() == 1 && url[0].item == "https://kompoz.dev" && url[0].start == 14 && url[0].end == 19);
    assert_eq!(url[0].tag, "url");
    assert!(s.get_string_annotations("url", 14, 14).len() == 1 && s.get_string_annotations("url", 19, 19).is_empty());
    assert!(s.get_all_string_annotations(0, 24).len() == 2 && s.has_string_annotations("id", 23, 24));
    assert!(!s.has_string_annotations("id", 0, 20));
    // Drawn: the inner style on top of the outer.
    let both = run_style(&s, "eğik");
    assert!(both.font_weight == FontWeight::Bold && both.font_style == FontStyle::Italic);
    let bold_only = run_style(&s, " son");
    assert!(bold_only.font_weight == FontWeight::Bold && bold_only.font_style == FontStyle::Normal);
}

#[test]
fn annotation_ranges_meet_as_composes() {
    // Overlapping, or one collapsed at the other's start.
    assert!(intersect(0, 5, 3, 8) && !intersect(0, 5, 5, 8) && intersect(5, 5, 5, 8) && intersect(2, 2, 0, 5));
    assert!(!intersect(5, 5, 0, 5) && intersect(3, 3, 3, 3) && !intersect(3, 3, 4, 4));
    assert!(contains(0, 5, 2, 2) && !contains(0, 5, 5, 5) && contains(0, 5, 0, 5) && contains(5, 5, 5, 5));
    assert!(panics(|| {
        Range::new((), 5, 3);
    }));
}

#[test]
fn annotated_strings_cut_join_and_compare() {
    let s = build_annotated_string(|b| {
        b.append("a ");
        b.with_style(bold(), |b| {
            b.append("kalın ");
            b.with_style(italic(), |b| {
                b.append("eğik");
                b.with_annotation("url", "u", |b| {
                    b.append(" bağ");
                });
            });
        });
    });
    let cut = s.sub_sequence(9, 19);
    assert_eq!(cut.text(), "eğik bağ");
    assert!(cut.span_styles().iter().all(|r| (r.start, r.end) == (0, 10)) && cut.span_styles().len() == 2);
    assert_eq!(cut.get_string_annotations("url", 0, 10)[0].start, 5);
    assert_eq!(s.sub_sequence(0, s.len()), s);
    assert_eq!(s.sub_sequence_range(TextRange::new(19, 9)), cut);
    assert!(panics(|| {
        s.sub_sequence(5, 2);
    }));
    // Joined: the second's annotations moved along.
    let ab = AnnotatedString::styled("ab", bold(), None);
    let joined = &ab + &AnnotatedString::styled("cd", italic(), None);
    assert_eq!(joined.text(), "abcd");
    assert_eq!(joined.span_styles().iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(), vec![(0, 2), (2, 4)]);
    // A builder starting from one, appending part of another.
    let mut b = AnnotatedStringBuilder::from(&ab);
    b.append_range(&s, 9, 14);
    let built = b.to_annotated_string();
    assert_eq!(built.text(), "abeğik");
    assert_eq!(built.span_styles().len(), 3);
    assert!(built.get_all_string_annotations(0, built.len()).is_empty());
    // Equal by text and annotations; a link's listener only to itself.
    assert_eq!(AnnotatedString::from("x"), AnnotatedString::new("x"));
    assert_ne!(ab, AnnotatedString::new("ab"));
    assert!(ab.has_equal_annotations(&AnnotatedString::styled("zz", bold(), None)));
    let listener = LinkInteractionListener::new(|_| {});
    let link = |l: &LinkInteractionListener| {
        build_annotated_string(|b| {
            b.with_link(LinkAnnotation::clickable_with("t", None, Some(l.clone())), |b| b.append("x"));
        })
    };
    assert_eq!(link(&listener), link(&listener));
    assert_ne!(link(&listener), link(&LinkInteractionListener::new(|_| {})));
    assert_eq!(format!("{}", joined), "abcd");
    assert_eq!(AnnotatedString::default().len(), 0);
}

#[test]
fn links_and_platform_annotations_are_annotations() {
    let opened = Arc::new(Mutex::new(Vec::new()));
    let o = opened.clone();
    let s = build_annotated_string(|b| {
        b.append("bkz. ");
        b.with_link(LinkAnnotation::url("https://kompoz.dev"), |b| b.append("site"));
        b.append(" ");
        b.with_link(
            LinkAnnotation::clickable("ayrıntı", move |l| o.lock().unwrap().push(l.as_tag().unwrap().to_string())),
            |b| b.append("burada"),
        );
        b.with_tts_annotation(TtsAnnotation::verbatim("ABC"), |b| b.append(" ABC"));
        b.with_url_annotation(UrlAnnotation::new("https://a.b"), |b| b.append(" a"));
    });
    // No look of their own (Compose's links; C++ underlined them).
    assert!(s.span_styles().is_empty());
    let links = s.get_link_annotations(0, s.len());
    assert_eq!(links.len(), 2);
    assert_eq!((links[0].item.as_url(), links[0].start, links[0].end), (Some("https://kompoz.dev"), 5, 9));
    assert!(s.has_link_annotations(7, 7) && !s.has_link_annotations(9, 9) && s.has_link_annotations(10, 11));
    let clickable = &links[1].item;
    clickable.link_interaction_listener().expect("a listener").on_click(clickable);
    assert_eq!(*opened.lock().unwrap(), vec!["ayrıntı".to_string()]);
    assert_eq!(format!("{:?}", links[0].item), "LinkAnnotation.Url(url=https://kompoz.dev)");
    assert_eq!(s.get_tts_annotations(0, s.len())[0].item, TtsAnnotation::Verbatim("ABC".into()));
    assert_eq!(s.get_url_annotations(0, s.len())[0].item.url, "https://a.b");
    // Styled links carry their styles.
    let styles = TextLinkStyles { style: Some(bold()), ..TextLinkStyles::default() };
    let styled = LinkAnnotation::url("u").with_styles(Some(styles.clone()));
    assert_eq!(styled.styles(), Some(&styles));
    assert_eq!(kompozable_block!(TextLinkStyles(hovered_style = Some(italic()))).hovered_style, Some(italic()));
    // Added on ranges.
    let mut b = AnnotatedStringBuilder::from("abcdef");
    b.add_link(LinkAnnotation::url("x"), 1, 3);
    b.add_style(bold(), 0, 6);
    b.add_style(ParagraphStyle { text_align: TextAlign::Center, ..ParagraphStyle::Default }, 0, 6);
    b.add_string_annotation("t", "v", 2, 4);
    b.add_tts_annotation(TtsAnnotation::verbatim("x"), 0, 1);
    b.add_url_annotation(UrlAnnotation::new("y"), 0, 1);
    let a = b.to_annotated_string();
    assert_eq!((a.annotations().len(), a.span_styles().len(), a.paragraph_styles().len()), (6, 1, 1));
}

#[test]
fn inline_content_has_its_alternate_text_annotated() {
    let mut b = AnnotatedStringBuilder::new();
    b.append("önce ");
    b.append_inline_content("resim".to_string());
    b.append_inline_content_with("ikon", "[i]");
    let s = b.to_annotated_string();
    assert_eq!(s.text(), "önce \u{FFFD}[i]");
    let rooms = s.get_string_annotations(INLINE_CONTENT_TAG, 0, s.len());
    assert_eq!(
        rooms.iter().map(|r| (r.item.as_str(), r.start, r.end)).collect::<Vec<_>>(),
        vec![("resim", 6, 9), ("ikon", 9, 12)]
    );
}

#[test]
fn the_builder_pops_as_composes() {
    let mut b = AnnotatedStringBuilder::new();
    assert!(panics(|| AnnotatedStringBuilder::new().pop()));
    let first = b.push_style(bold());
    b.append("a");
    b.push_link(LinkAnnotation::url("u"));
    b.push_tts_annotation(TtsAnnotation::verbatim("x"));
    b.push_url_annotation(UrlAnnotation::new("y"));
    b.append("b");
    assert_eq!(first, 0);
    let mut c = b.clone();
    assert!(panics(move || c.pop_to(4)));
    b.pop();
    b.append("c");
    b.pop_to(first);
    b.append("d");
    let s = b.to_annotated_string();
    let ends: Vec<(usize, usize)> = s.annotations().iter().map(|r| (r.start, r.end)).collect();
    assert_eq!(ends, vec![(0, 3), (1, 3), (1, 3), (1, 2)]);
    // Annotations mapped and flat-mapped.
    let red = s.map_annotations(|r| match &r.item {
        Annotation::SpanStyle(st) => {
            Range::new(Annotation::SpanStyle(SpanStyle { color: Color::Red, ..st.clone() }), r.start, r.end)
        }
        _ => r.clone(),
    });
    assert_eq!(red.span_styles()[0].item.color, Color::Red);
    let twice = s.flat_map_annotations(|r| vec![r.clone(), r.clone()]);
    assert_eq!(twice.annotations().len(), 8);
}

#[test]
fn paragraph_styles_nest_and_normalise() {
    let center = ParagraphStyle { text_align: TextAlign::Center, ..ParagraphStyle::Default };
    let justify = ParagraphStyle { text_align: TextAlign::Justify, ..ParagraphStyle::Default };
    let rtl = ParagraphStyle { text_direction: TextDirection::Rtl, ..ParagraphStyle::Default };
    let s = AnnotatedString::with_styles(
        "abcdefghij",
        vec![],
        vec![Range::new(center.clone(), 0, 4), Range::new(justify.clone(), 6, 10), Range::new(rtl.clone(), 7, 9)],
    );
    let default = ParagraphStyle { text_align: TextAlign::Start, ..ParagraphStyle::Default };
    let n = s.normalized_paragraph_styles(&default);
    let got: Vec<(usize, usize, TextAlign, TextDirection)> =
        n.iter().map(|r| (r.start, r.end, r.item.text_align, r.item.text_direction)).collect();
    use TextAlign::*;
    use TextDirection as D;
    assert_eq!(
        got,
        vec![
            (0, 4, Center, D::Unspecified),
            (4, 6, Start, D::Unspecified),
            (6, 7, Justify, D::Unspecified),
            // (the inner one merged over the default first, as Compose's: its alignment the default's)
            (7, 9, Start, D::Rtl),
            (9, 10, Justify, D::Unspecified),
        ]
    );
    let texts = s.map_each_paragraph_style(&default, |text, _| text.text().to_string());
    assert_eq!(texts, vec!["abcd", "ef", "g", "hi", "j"]);
    assert_eq!(AnnotatedString::new("").normalized_paragraph_styles(&default).len(), 1);
    // Partly overlapping ones are refused.
    assert!(panics(|| {
        AnnotatedString::with_styles(
            "abcdefgh",
            vec![],
            vec![Range::new(center.clone(), 0, 5), Range::new(rtl.clone(), 3, 8)],
        );
    }));
    assert_eq!(AnnotatedString::with_paragraph_style("ab", center.clone()).paragraph_styles()[0].end, 2);
}

#[test]
fn case_maps_as_the_locale_writes_it() {
    let tr = LocaleList::from_tags("tr-TR");
    let en = LocaleList::from_tags("en-US");
    let s = AnnotatedString::with_styles("istanbul ılık", vec![Range::new(bold(), 0, 2)], vec![]);
    let upper = s.to_upper_case(&tr);
    assert_eq!(upper.text(), "İSTANBUL ILIK");
    assert_eq!((upper.span_styles()[0].start, upper.span_styles()[0].end), (0, 3));
    assert_eq!(s.to_upper_case(&en).text(), "ISTANBUL ILIK");
    assert_eq!(upper.to_lower_case(&tr).text(), "istanbul ılık");
    assert_eq!(AnnotatedString::new("İI").to_lower_case(&en).text(), "i\u{307}i");
    assert_eq!(AnnotatedString::new("ılık").capitalize(&tr).text(), "Ilık");
    assert_eq!(AnnotatedString::new("ijk").capitalize(&tr).text(), "İjk");
    assert_eq!(AnnotatedString::new("ǆemal").capitalize(&en).text(), "ǅemal");
    assert_eq!(AnnotatedString::new("ﬁne").capitalize(&en).text(), "Fine");
    assert_eq!(AnnotatedString::new("ABC").decapitalize(&en).text(), "aBC");
    assert_eq!(AnnotatedString::new("straße").to_upper_case(&en).text(), "STRASSE");
    // Lithuanian keeps a dotted i under a further accent, and drops the dot when capitalised.
    let lt = LocaleList::from_tags("lt");
    assert_eq!(AnnotatedString::new("I\u{300}").to_lower_case(&lt).text(), "i\u{307}\u{300}");
    assert_eq!(AnnotatedString::new("i\u{307}\u{300}").to_upper_case(&lt).text(), "I\u{300}");
}

/// The C++ Kompoz's annotated_strings_from_html (its drawing part comes with BasicText): Compose's links carry
/// their styles and listener with no look of their own (C++ underlined them and added the style as a span).
#[test]
fn annotated_strings_from_html() {
    let a = AnnotatedString::from_html(
        "<b>kalın</b> ve <i>eğik</i>, <u>altı</u> <del>silik</del> H<sub>2</sub>O x<sup>2</sup>",
    );
    assert_eq!(a.text(), "kalın ve eğik, altı silik H2O x2");
    assert_eq!(span_of(&a, "kalın").unwrap().font_weight, Some(FontWeight::Bold));
    assert_eq!(span_of(&a, "eğik").unwrap().font_style, Some(FontStyle::Italic));
    assert_eq!(span_of(&a, "altı").unwrap().text_decoration, Some(TextDecoration::Underline));
    assert_eq!(span_of(&a, "silik").unwrap().text_decoration, Some(TextDecoration::LineThrough));
    assert_eq!(span_of(&a, "2").unwrap().baseline_shift, Some(BaselineShift::Subscript)); // (the first 2: H₂O)
    assert!(span_of(&a, " ve ").is_none());
    // Paragraphs, headings and lists on lines of their own; white space collapsed; <br>.
    let b = AnnotatedString::from_html(
        "<h1>Başlık</h1>\n<p>Bir   iki\n   üç</p><p>dört<br>beş</p><ul><li>elma</li><li>armut</li></ul>son",
    );
    assert_eq!(b.text(), "Başlık\nBir iki üç\ndört\nbeş\n• elma\n• armut\nson");
    let heading = span_of(&b, "Başlık").unwrap();
    assert!(heading.font_weight == Some(FontWeight::Bold) && heading.font_size == 1.5.em());
    assert!(span_of(&b, "iki").is_none());
    // As it is; no break left at the end.
    let pre = AnnotatedString::from_html("<p>önce</p><pre>a  b\n c</pre><p>sonra</p>\n");
    assert_eq!(pre.text(), "önce\na  b\n c\nsonra");
    assert_eq!(span_of(&pre, "a  b").unwrap().font_family, Some(FontFamily::Monospace));
    // Entities, colors, links, monospace; unknown tags and comments left out; a tag closed late closes those inside.
    let opened = Arc::new(Mutex::new(Vec::new()));
    let o = opened.clone();
    let styles = TextLinkStyles {
        style: Some(SpanStyle { color: Color::new(0.0, 0.0, 1.0, 1.0), ..SpanStyle::Default }),
        ..Default::default()
    };
    let c = AnnotatedString::from_html_with(
        "a &amp; b &lt;c&gt; &#252;&#xE7; &copy;<!-- yorum --><x>d</x> <font color=\"#ff0000\">kırmızı</font> \
         <span style=\"color: teal; background-color: #00F; font-weight: bold\">vurgu</span> \
         <a href=\"https://kompoz.dev\">site</a> \
         <code>kod()</code> <b>k<i>ke</b>düz",
        Some(styles.clone()),
        Some(LinkInteractionListener::new(move |l| o.lock().unwrap().push(l.as_url().unwrap().to_string()))),
    );
    assert_eq!(c.text(), "a & b <c> üç ©d kırmızı vurgu site kod() kkedüz"); // (nothing between © and d)
    assert_eq!(span_of(&c, "kırmızı").unwrap().color, Color::new(1.0, 0.0, 0.0, 1.0));
    let vurgu = span_of(&c, "vurgu").unwrap();
    assert_eq!(vurgu.background, Color::new(0.0, 0.0, 1.0, 1.0));
    assert_eq!(vurgu.color, Color::from_argb(0xFF008080));
    assert_eq!(vurgu.font_weight, Some(FontWeight::Bold));
    let links = c.get_link_annotations(0, c.len());
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].item.as_url(), Some("https://kompoz.dev"));
    assert_eq!(&c.text()[links[0].start..links[0].end], "site");
    assert_eq!(links[0].item.styles(), Some(&styles));
    assert!(span_of(&c, "site").is_none());
    links[0].item.link_interaction_listener().unwrap().on_click(&links[0].item);
    assert_eq!(*opened.lock().unwrap(), vec!["https://kompoz.dev".to_string()]);
    assert_eq!(span_of(&c, "kod()").unwrap().font_family, Some(FontFamily::Monospace));
    let ke = span_of(&c, "ke").unwrap();
    assert!(ke.font_style == Some(FontStyle::Italic) && ke.font_weight == Some(FontWeight::Bold));
    assert!(span_of(&c, "düz").is_none());
    // Drawn: the bold word bold.
    let d = AnnotatedString::from_html("düz <b>kalın</b>");
    assert_eq!(run_style(&d, "kalın").font_weight, FontWeight::Bold);
}

#[test]
fn html_annotations_alignment_and_faces_are_androids() {
    let a = AnnotatedString::from_html(
        "<p style=\"text-align: center\">orta</p><annotation key=\"değer\" boş=\"\">not</annotation> \
         <font face=\"serif\">serif</font> <font face=\"Noto Sans Mono\">mono</font> <big>büyük</big> \
         <small>küçük</small>",
    );
    assert_eq!(a.text(), "orta\nnot serif mono büyük küçük");
    assert_eq!(a.paragraph_styles().len(), 1);
    assert_eq!((a.paragraph_styles()[0].start, a.paragraph_styles()[0].end), (0, 4));
    assert_eq!(a.paragraph_styles()[0].item.text_align, TextAlign::Center);
    let notes = a.get_string_annotations("key", 0, a.len());
    assert_eq!(notes.len(), 1);
    assert_eq!((notes[0].item.as_str(), &a.text()[notes[0].start..notes[0].end]), ("değer", "not"));
    assert!(a.get_string_annotations("boş", 0, a.len()).is_empty());
    assert_eq!(span_of(&a, "serif").unwrap().font_family, Some(FontFamily::Serif));
    let mono = span_of(&a, "mono").unwrap().font_family.unwrap();
    assert!(
        matches!(mono.fonts(), Some([f]) if f.source == FontSource::Device(DeviceFontFamilyName::new("Noto Sans Mono")))
    );
    assert_eq!(span_of(&a, "büyük").unwrap().font_size, 1.25.em());
    assert_eq!(span_of(&a, "küçük").unwrap().font_size, 0.8.em());
    // What isn't a tag stays text.
    assert_eq!(AnnotatedString::from_html("a < b &unknown; c").text(), "a < b &unknown; c");
}

#[test]
fn texts_and_values_are_saved_and_restored() {
    let s = build_annotated_string(|b| {
        b.with_style(
            SpanStyle {
                color: Color::Red,
                font_size: 18.sp(),
                font_family: Some(FontFamily::Serif),
                letter_spacing: 0.1.em(),
                shadow: Some(Shadow::new(Color::Black, Offset::new(1.0, 2.0), 3.0, 0.0)),
                locale_list: Some(LocaleList::from_tags("tr-TR, en")),
                ..bold()
            },
            |b| b.append("kalın"),
        );
        b.with_style(
            ParagraphStyle {
                text_align: TextAlign::Justify,
                line_height: 1.5.em(),
                line_height_style: Some(LineHeightStyle::Default),
                line_break: LineBreak::Paragraph,
                text_motion: Some(TextMotion::Animated),
                ..ParagraphStyle::Default
            },
            |b| b.append("\npar"),
        );
        b.with_annotation("t", "v", |b| b.append(" not"));
        b.with_link(
            LinkAnnotation::url_with(
                "https://k",
                Some(TextLinkStyles { style: Some(italic()), ..Default::default() }),
                None,
            ),
            |b| b.append(" bağ"),
        );
        b.with_tts_annotation(TtsAnnotation::verbatim("x"), |b| b.append("x"));
    });
    let saved = s.save();
    let restored = AnnotatedString::restore(&SavedValue::decode(&saved.encode()).unwrap()).unwrap();
    assert_eq!(restored, s);
    let value = TextFieldValue::new(s.clone(), TextRange::new(2, 5), Some(TextRange::new(1, 2)));
    let back = TextFieldValue::restore(&value.save()).unwrap();
    assert_eq!((back.annotated_string(), back.selection(), back.composition()), (&s, TextRange::new(2, 5), None));
    assert_eq!(TextRange::restore(&TextRange::new(3, 1).save()), Some(TextRange::new(3, 1)));
    assert_eq!(AnnotatedString::restore(&SavedValue::Int(1)), None);
}

#[test]
fn text_field_values_keep_their_ranges_within_the_text() {
    let v = TextFieldValue::from_text("abc", TextRange::new(1, 10), Some(TextRange::new(5, 7)));
    assert_eq!((v.selection(), v.composition()), (TextRange::new(1, 3), Some(TextRange::new(3, 3))));
    // On character boundaries.
    let v = TextFieldValue::from_text("ğa", TextRange::at(1), None);
    assert_eq!(v.selection(), TextRange::Zero);
    let v = TextFieldValue::from_text("merhaba dünya", TextRange::new(8, 11), None);
    assert_eq!(v.get_text_before_selection(3).text(), "ba ");
    assert_eq!(v.get_text_before_selection(0).text(), "");
    assert_eq!(v.get_text_before_selection(100).text(), "merhaba ");
    assert_eq!(v.get_selected_text().text(), "dü");
    assert_eq!(v.get_text_after_selection(2).text(), "ny");
    assert_eq!(v.get_text_after_selection(100).text(), "nya");
    let named = kompozable_block!(TextFieldValueFn(text = "xy".to_string(), selection = TextRange::at(1)));
    assert_eq!((named.text(), named.selection()), ("xy", TextRange::at(1)));
    assert_eq!(kompozable_block!(named.copy(selection = TextRange::new(0, 2))).selection(), TextRange::new(0, 2));
    assert_eq!(named.with_text("x").selection(), TextRange::at(1));
    assert_eq!(TextFieldValue::default().text(), "");
}

#[test]
fn text_ranges_are_byte_ranges() {
    let r = TextRange::new(5, 2);
    assert!(r.reversed() && !r.collapsed() && r.min() == 2 && r.max() == 5 && r.length() == 3);
    assert!(
        TextRange::new(0, 3).intersects(TextRange::new(2, 4)) && !TextRange::new(0, 2).intersects(TextRange::new(2, 4))
    );
    assert!(TextRange::new(0, 5).contains(TextRange::new(1, 5)) && TextRange::new(1, 4).contains_offset(3));
    assert!(!TextRange::new(1, 4).contains_offset(4));
    assert_eq!(TextRange::new(2, 9).coerce_in(3, 6), TextRange::new(3, 6));
    assert_eq!(TextRange::new(0, 2).substring("ğa"), "ğ");
    assert_eq!(format!("{:?}", TextRange::at(4)), "TextRange(4, 4)");
    assert_eq!(TextRange::from(1..3), TextRange::new(1, 3));
}

#[test]
fn locales_are_bcp47_language_tags() {
    let tr = Locale::new("tr-TR");
    assert_eq!((tr.language(), tr.script(), tr.region(), tr.to_language_tag()), ("tr", "", "TR", "tr-TR".to_string()));
    let zh = Locale::new("zh-Hant-TW");
    assert_eq!((zh.language(), zh.script(), zh.region()), ("zh", "Hant", "TW"));
    assert_eq!(Locale::new("en_us").to_language_tag(), "en-US");
    assert_eq!(Locale::new("!!").to_language_tag(), "und");
    assert_eq!(Locale::new("TR-tr"), tr);
    let list = LocaleList::from_tags("tr-TR, en");
    assert!(list.len() == 2 && list.contains(&tr) && list.get(1) == Some(&Locale::new("en")));
    assert_eq!(format!("{list:?}"), "LocaleList(localeList=[tr-TR, en])");
    assert!(LocaleList::empty().is_empty());
    assert_eq!(LocaleList::empty().first_or_current(), Locale::current());
    assert!(!LocaleList::current().is_empty());
}
