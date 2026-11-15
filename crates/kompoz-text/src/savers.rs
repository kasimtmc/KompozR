//! How text values are saved with an app's state (Compose's text Savers): `remember_saveable` keeps a text
//! field's value, a selection, an annotated string with its styles, annotations and links (a link's listener and
//! a family of one's own fonts aside: they can't outlive the process).

use crate::annotated_string::{AnnotatedString, Annotation, Range};
use crate::font::{FontFamily, FontStyle, FontSynthesis, FontWeight, GenericFontFamily};
use crate::input::TextFieldValue;
use crate::intl::{Locale, LocaleList};
use crate::link::{LinkAnnotation, TextLinkStyles, TtsAnnotation, UrlAnnotation};
use crate::paragraph_style::ParagraphStyle;
use crate::platform_style::{EmojiSupportMatch, PlatformParagraphStyle};
use crate::range::TextRange;
use crate::span_style::SpanStyle;
use crate::style::line_break::{Strategy, Strictness, WordBreak};
use crate::style::line_height_style::{Alignment, Mode, Trim};
use crate::style::{
    BaselineShift, Hyphens, LineBreak, LineHeightStyle, Linearity, TextAlign, TextDecoration, TextDirection,
    TextGeometricTransform, TextIndent, TextMotion,
};
use kompoz_graphics::{Color, Offset, Shadow, TextUnit, TextUnitType};
use kompoz_runtime::{Saveable, SavedValue};

fn int(v: i64) -> SavedValue {
    SavedValue::Int(v)
}
fn double(v: f32) -> SavedValue {
    SavedValue::Double(v as f64)
}
fn list(items: impl IntoIterator<Item = SavedValue>) -> SavedValue {
    SavedValue::list(items)
}
fn opt<T>(v: &Option<T>, save: impl FnOnce(&T) -> SavedValue) -> SavedValue {
    v.as_ref().map_or(SavedValue::Null, save)
}
fn at(items: &[SavedValue], i: usize) -> &SavedValue {
    static NONE: SavedValue = SavedValue::Null;
    items.get(i).unwrap_or(&NONE)
}
fn f32_of(v: &SavedValue) -> Option<f32> {
    v.as_f64().map(|d| d as f32)
}

fn save_color(c: Color) -> SavedValue {
    if c.is_unspecified() { SavedValue::Null } else { int(c.value() as i64) }
}
fn restore_color(v: &SavedValue) -> Color {
    v.as_int().map_or(Color::Unspecified, |n| Color::from_value(n as u64))
}

fn save_text_unit(t: TextUnit) -> SavedValue {
    let unit = match t.unit_type() {
        TextUnitType::Unspecified => return SavedValue::Null,
        TextUnitType::Sp => 1,
        TextUnitType::Em => 2,
    };
    list([double(t.value()), int(unit)])
}
fn restore_text_unit(v: &SavedValue) -> TextUnit {
    let Some(l) = v.as_list() else { return TextUnit::Unspecified };
    match (f32_of(at(l, 0)), at(l, 1).as_int()) {
        (Some(value), Some(1)) => TextUnit::new(value, TextUnitType::Sp),
        (Some(value), Some(2)) => TextUnit::new(value, TextUnitType::Em),
        _ => TextUnit::Unspecified,
    }
}

fn save_offset(o: Offset) -> SavedValue {
    if o.is_specified() { list([double(o.x), double(o.y)]) } else { SavedValue::Null }
}
fn restore_offset(v: &SavedValue) -> Offset {
    v.as_list().and_then(|l| Some(Offset::new(f32_of(at(l, 0))?, f32_of(at(l, 1))?))).unwrap_or(Offset::Unspecified)
}

fn save_shadow(s: &Shadow) -> SavedValue {
    list([save_color(s.color), save_offset(s.offset), double(s.blur_radius), double(s.spread), double(s.alpha)])
}
fn restore_shadow(v: &SavedValue) -> Option<Shadow> {
    let l = v.as_list()?;
    Some(Shadow {
        color: restore_color(at(l, 0)),
        offset: restore_offset(at(l, 1)),
        blur_radius: f32_of(at(l, 2))?,
        spread: f32_of(at(l, 3)).unwrap_or(0.0),
        alpha: f32_of(at(l, 4)).unwrap_or(1.0),
        ..Shadow::default()
    })
}

fn save_font_family(f: &FontFamily) -> SavedValue {
    match f.as_generic() {
        Some(GenericFontFamily::Default) => SavedValue::String("default".into()),
        Some(g) => SavedValue::String(g.name().unwrap_or("default").into()),
        None => SavedValue::Null,
    }
}
fn restore_font_family(v: &SavedValue) -> Option<FontFamily> {
    let name = v.as_str()?;
    Some(if name == "default" { FontFamily::Default } else { FontFamily::generic(GenericFontFamily::from_name(name)?) })
}

impl Saveable for TextRange {
    fn save(&self) -> SavedValue {
        list([int(self.start as i64), int(self.end as i64)])
    }
    fn restore(saved: &SavedValue) -> Option<TextRange> {
        let l = saved.as_list()?;
        Some(TextRange::new(usize::try_from(at(l, 0).as_int()?).ok()?, usize::try_from(at(l, 1).as_int()?).ok()?))
    }
}

impl Saveable for Locale {
    fn save(&self) -> SavedValue {
        SavedValue::String(self.to_language_tag())
    }
    fn restore(saved: &SavedValue) -> Option<Locale> {
        Some(Locale::new(saved.as_str()?))
    }
}

impl Saveable for LocaleList {
    fn save(&self) -> SavedValue {
        list(self.iter().map(Saveable::save))
    }
    fn restore(saved: &SavedValue) -> Option<LocaleList> {
        Some(LocaleList::new(saved.as_list()?.iter().filter_map(Locale::restore)))
    }
}

impl Saveable for SpanStyle {
    fn save(&self) -> SavedValue {
        let fg = self.foreground();
        list([
            save_color(fg.color()),
            save_text_unit(self.font_size),
            opt(&self.font_weight, |w| int(w.weight as i64)),
            opt(&self.font_style, |s| int(s.value() as i64)),
            opt(&self.font_synthesis, |s| int(s.value as i64)),
            opt(&self.font_family, save_font_family),
            opt(&self.font_feature_settings, |s| SavedValue::String(s.clone())),
            save_text_unit(self.letter_spacing),
            opt(&self.baseline_shift, |b| double(b.0)),
            opt(&self.text_geometric_transform, |t| list([double(t.scale_x), double(t.skew_x)])),
            opt(&self.locale_list, Saveable::save),
            save_color(self.background),
            opt(&self.text_decoration, |d| int(d.mask as i64)),
            opt(&self.shadow, save_shadow),
        ])
    }
    fn restore(saved: &SavedValue) -> Option<SpanStyle> {
        let l = saved.as_list()?;
        Some(SpanStyle {
            color: restore_color(at(l, 0)),
            font_size: restore_text_unit(at(l, 1)),
            font_weight: at(l, 2).as_int().map(|w| FontWeight::new(w as i32)),
            font_style: at(l, 3).as_int().map(|s| if s == 1 { FontStyle::Italic } else { FontStyle::Normal }),
            font_synthesis: at(l, 4).as_int().map(|v| FontSynthesis { value: v as i32 }),
            font_family: restore_font_family(at(l, 5)),
            font_feature_settings: at(l, 6).as_str().map(str::to_string),
            letter_spacing: restore_text_unit(at(l, 7)),
            baseline_shift: f32_of(at(l, 8)).map(BaselineShift),
            text_geometric_transform: at(l, 9)
                .as_list()
                .and_then(|t| Some(TextGeometricTransform::new(f32_of(at(t, 0))?, f32_of(at(t, 1))?))),
            locale_list: LocaleList::restore(at(l, 10)),
            background: restore_color(at(l, 11)),
            text_decoration: at(l, 12).as_int().map(|m| TextDecoration { mask: m as i32 }),
            shadow: restore_shadow(at(l, 13)),
            ..SpanStyle::Default
        })
    }
}

fn save_line_height_style(s: &LineHeightStyle) -> SavedValue {
    let trim = match (s.trim.is_trim_first_line_top(), s.trim.is_trim_last_line_bottom()) {
        (true, true) => 3,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 0,
    };
    let mode = match s.mode {
        Mode::Fixed => 0,
        Mode::Minimum => 1,
        Mode::Tight => 2,
    };
    list([double(s.alignment.top_ratio()), int(trim), int(mode)])
}
fn restore_line_height_style(v: &SavedValue) -> Option<LineHeightStyle> {
    let l = v.as_list()?;
    let trim = match at(l, 1).as_int()? {
        3 => Trim::Both,
        1 => Trim::FirstLineTop,
        2 => Trim::LastLineBottom,
        _ => Trim::None,
    };
    let mode = match at(l, 2).as_int()? {
        1 => Mode::Minimum,
        2 => Mode::Tight,
        _ => Mode::Fixed,
    };
    Some(LineHeightStyle::new(Alignment::new(f32_of(at(l, 0))?), trim, mode))
}

const ALIGNS: [TextAlign; 7] = [
    TextAlign::Unspecified,
    TextAlign::Left,
    TextAlign::Right,
    TextAlign::Center,
    TextAlign::Justify,
    TextAlign::Start,
    TextAlign::End,
];
const DIRECTIONS: [TextDirection; 6] = [
    TextDirection::Unspecified,
    TextDirection::Ltr,
    TextDirection::Rtl,
    TextDirection::Content,
    TextDirection::ContentOrLtr,
    TextDirection::ContentOrRtl,
];
const STRATEGIES: [Strategy; 4] = [Strategy::Unspecified, Strategy::Simple, Strategy::HighQuality, Strategy::Balanced];
const STRICTNESSES: [Strictness; 5] =
    [Strictness::Unspecified, Strictness::Default, Strictness::Loose, Strictness::Normal, Strictness::Strict];
const WORD_BREAKS: [WordBreak; 3] = [WordBreak::Unspecified, WordBreak::Default, WordBreak::Phrase];
const HYPHENS: [Hyphens; 3] = [Hyphens::Unspecified, Hyphens::None, Hyphens::Auto];
const EMOJI: [EmojiSupportMatch; 3] = [EmojiSupportMatch::Default, EmojiSupportMatch::None, EmojiSupportMatch::All];
const LINEARITIES: [Linearity; 3] = [Linearity::Linear, Linearity::FontHinting, Linearity::None];

fn index_of<T: PartialEq>(values: &[T], v: &T) -> SavedValue {
    int(values.iter().position(|x| x == v).unwrap_or(0) as i64)
}
fn value_at<T: Copy>(values: &[T], v: &SavedValue) -> Option<T> {
    values.get(usize::try_from(v.as_int()?).ok()?).copied()
}

impl Saveable for ParagraphStyle {
    fn save(&self) -> SavedValue {
        let lb = self.line_break;
        list([
            index_of(&ALIGNS, &self.text_align),
            index_of(&DIRECTIONS, &self.text_direction),
            save_text_unit(self.line_height),
            opt(&self.text_indent, |t| list([save_text_unit(t.first_line), save_text_unit(t.rest_line)])),
            opt(&self.platform_style, |p| {
                list([SavedValue::Bool(p.include_font_padding), index_of(&EMOJI, &p.emoji_support_match)])
            }),
            opt(&self.line_height_style, save_line_height_style),
            list([
                index_of(&STRATEGIES, &lb.strategy),
                index_of(&STRICTNESSES, &lb.strictness),
                index_of(&WORD_BREAKS, &lb.word_break),
            ]),
            index_of(&HYPHENS, &self.hyphens),
            opt(&self.text_motion, |m| {
                list([index_of(&LINEARITIES, &m.linearity), SavedValue::Bool(m.subpixel_text_positioning)])
            }),
        ])
    }
    fn restore(saved: &SavedValue) -> Option<ParagraphStyle> {
        let l = saved.as_list()?;
        let lb = at(l, 6).as_list()?;
        Some(ParagraphStyle {
            text_align: value_at(&ALIGNS, at(l, 0))?,
            text_direction: value_at(&DIRECTIONS, at(l, 1))?,
            line_height: restore_text_unit(at(l, 2)),
            text_indent: at(l, 3)
                .as_list()
                .map(|t| TextIndent::new(restore_text_unit(at(t, 0)), restore_text_unit(at(t, 1)))),
            platform_style: at(l, 4).as_list().and_then(|p| {
                Some(PlatformParagraphStyle {
                    include_font_padding: at(p, 0).as_bool()?,
                    emoji_support_match: value_at(&EMOJI, at(p, 1))?,
                })
            }),
            line_height_style: restore_line_height_style(at(l, 5)),
            line_break: LineBreak::new(
                value_at(&STRATEGIES, at(lb, 0))?,
                value_at(&STRICTNESSES, at(lb, 1))?,
                value_at(&WORD_BREAKS, at(lb, 2))?,
            ),
            hyphens: value_at(&HYPHENS, at(l, 7))?,
            text_motion: at(l, 8).as_list().and_then(|m| {
                Some(TextMotion {
                    linearity: value_at(&LINEARITIES, at(m, 0))?,
                    subpixel_text_positioning: at(m, 1).as_bool()?,
                })
            }),
        })
    }
}

impl Saveable for TextLinkStyles {
    fn save(&self) -> SavedValue {
        list([
            opt(&self.style, Saveable::save),
            opt(&self.focused_style, Saveable::save),
            opt(&self.hovered_style, Saveable::save),
            opt(&self.pressed_style, Saveable::save),
        ])
    }
    fn restore(saved: &SavedValue) -> Option<TextLinkStyles> {
        let l = saved.as_list()?;
        Some(TextLinkStyles::new(
            SpanStyle::restore(at(l, 0)),
            SpanStyle::restore(at(l, 1)),
            SpanStyle::restore(at(l, 2)),
            SpanStyle::restore(at(l, 3)),
        ))
    }
}

/// An annotation's kind and item (a link's listener left out).
fn save_annotation(a: &Annotation) -> (i64, SavedValue) {
    match a {
        Annotation::ParagraphStyle(p) => (0, p.save()),
        Annotation::SpanStyle(s) => (1, s.save()),
        Annotation::Tts(TtsAnnotation::Verbatim(v)) => (2, SavedValue::String(v.clone())),
        Annotation::Url(u) => (3, SavedValue::String(u.url.clone())),
        Annotation::Link(link) => match &**link {
            LinkAnnotation::Url { url, styles, .. } => {
                (4, list([SavedValue::String(url.clone()), opt(styles, Saveable::save)]))
            }
            LinkAnnotation::Clickable { tag, styles, .. } => {
                (5, list([SavedValue::String(tag.clone()), opt(styles, Saveable::save)]))
            }
        },
        Annotation::String(s) => (6, SavedValue::String(s.clone())),
    }
}

fn restore_annotation(kind: i64, item: &SavedValue) -> Option<Annotation> {
    Some(match kind {
        0 => Annotation::ParagraphStyle(ParagraphStyle::restore(item)?),
        1 => Annotation::SpanStyle(SpanStyle::restore(item)?),
        2 => Annotation::Tts(TtsAnnotation::verbatim(item.as_str()?)),
        3 => Annotation::Url(UrlAnnotation::new(item.as_str()?)),
        4 | 5 => {
            let l = item.as_list()?;
            let name = at(l, 0).as_str()?;
            let styles = TextLinkStyles::restore(at(l, 1));
            Annotation::Link(Box::new(if kind == 4 {
                LinkAnnotation::url_with(name, styles, None)
            } else {
                LinkAnnotation::clickable_with(name, styles, None)
            }))
        }
        6 => Annotation::String(item.as_str()?.to_string()),
        _ => return None,
    })
}

impl Saveable for AnnotatedString {
    fn save(&self) -> SavedValue {
        let ranges = self.annotations().iter().map(|r| {
            let (kind, item) = save_annotation(&r.item);
            list([int(kind), item, int(r.start as i64), int(r.end as i64), SavedValue::String(r.tag.clone())])
        });
        list([SavedValue::String(self.text().to_string()), list(ranges)])
    }
    fn restore(saved: &SavedValue) -> Option<AnnotatedString> {
        let l = saved.as_list()?;
        let text = at(l, 0).as_str()?;
        let mut annotations = Vec::new();
        for r in at(l, 1).as_list().unwrap_or(&[]) {
            let r = r.as_list()?;
            let item = restore_annotation(at(r, 0).as_int()?, at(r, 1))?;
            let start = usize::try_from(at(r, 2).as_int()?).ok()?;
            let end = usize::try_from(at(r, 3).as_int()?).ok()?;
            if start > end || end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                return None;
            }
            annotations.push(Range::tagged(item, start, end, at(r, 4).as_str().unwrap_or("")));
        }
        Some(AnnotatedString::from_annotations(text, annotations))
    }
}

impl Saveable for TextFieldValue {
    fn save(&self) -> SavedValue {
        list([self.annotated_string().save(), self.selection().save()])
    }
    fn restore(saved: &SavedValue) -> Option<TextFieldValue> {
        let l = saved.as_list()?;
        Some(TextFieldValue::new(AnnotatedString::restore(at(l, 0))?, TextRange::restore(at(l, 1))?, None))
    }
}
