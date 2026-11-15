//! Text with styles and annotations on its ranges (Compose's AnnotatedString), and what builds it (Compose's
//! AnnotatedString.Builder and buildAnnotatedString).

use crate::intl::{LocaleList, capitalize, decapitalize, to_lower_case, to_upper_case};
use crate::link::{LinkAnnotation, TtsAnnotation, UrlAnnotation};
use crate::paragraph_style::ParagraphStyle;
use crate::range::TextRange;
use crate::span_style::SpanStyle;
use kompoz_runtime::named;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

/// The tag of the annotation [`AnnotatedStringBuilder::append_inline_content`] puts on its alternate text (Compose
/// foundation's INLINE_CONTENT_TAG): where a Text's inline content goes.
pub const INLINE_CONTENT_TAG: &str = "androidx.compose.foundation.text.inlineContent";

/// What an annotated string's range carries (Compose's AnnotatedString.Annotation): a span style, a paragraph
/// style, a string annotation (a tag's item: a URL, an id), a text-to-speech or URL annotation, a link (boxed: its
/// four link styles would make every annotation as big).
#[derive(Clone, PartialEq, Debug)]
// (Span styles are what most annotations are: kept inline.)
#[allow(clippy::large_enum_variant)]
pub enum Annotation {
    SpanStyle(SpanStyle),
    ParagraphStyle(ParagraphStyle),
    String(String),
    Tts(TtsAnnotation),
    Url(UrlAnnotation),
    Link(Box<LinkAnnotation>),
}

impl From<SpanStyle> for Annotation {
    fn from(s: SpanStyle) -> Annotation {
        Annotation::SpanStyle(s)
    }
}
impl From<ParagraphStyle> for Annotation {
    fn from(s: ParagraphStyle) -> Annotation {
        Annotation::ParagraphStyle(s)
    }
}
impl From<TtsAnnotation> for Annotation {
    fn from(a: TtsAnnotation) -> Annotation {
        Annotation::Tts(a)
    }
}
impl From<UrlAnnotation> for Annotation {
    fn from(a: UrlAnnotation) -> Annotation {
        Annotation::Url(a)
    }
}
impl From<LinkAnnotation> for Annotation {
    fn from(a: LinkAnnotation) -> Annotation {
        Annotation::Link(Box::new(a))
    }
}

/// An item on `start..end` of a text (UTF-8 byte offsets), with a tag (a string annotation's; empty for the
/// others) (Compose's AnnotatedString.Range).
#[derive(Clone, PartialEq, Debug)]
pub struct Range<T> {
    pub item: T,
    pub start: usize,
    pub end: usize,
    pub tag: String,
}

impl<T> Range<T> {
    /// Untagged (a range ending before it starts panics, as Compose's).
    pub fn new(item: T, start: usize, end: usize) -> Range<T> {
        Range::tagged(item, start, end, "")
    }
    /// Tagged.
    pub fn tagged(item: T, start: usize, end: usize, tag: impl Into<String>) -> Range<T> {
        assert!(start <= end, "kompoz: Reversed range is not supported");
        Range { item, start, end, tag: tag.into() }
    }
    /// The same item and tag on another range.
    pub fn moved(&self, start: usize, end: usize) -> Range<T>
    where
        T: Clone,
    {
        Range::tagged(self.item.clone(), start, end, self.tag.clone())
    }
    /// Its range.
    pub fn range(&self) -> TextRange {
        TextRange::new(self.start, self.end)
    }
}

/// Whether `target` lies in `base` (Compose's contains): a collapsed range counts inside a range, except at its
/// end.
pub fn contains(base_start: usize, base_end: usize, target_start: usize, target_end: usize) -> bool {
    base_start <= target_start
        && target_end <= base_end
        && (base_end != target_end || (target_start == target_end) == (base_start == base_end))
}

/// Whether two ranges meet as annotation queries count it (Compose's intersect): they overlap, or one of them is
/// collapsed at the other's start.
pub fn intersect(l_start: usize, l_end: usize, r_start: usize, r_end: usize) -> bool {
    ((l_start == l_end || r_start == r_end) && l_start == r_start) || (l_start < r_end && r_start < l_end)
}

/// Text with styles and annotations on its ranges (Compose's AnnotatedString): span styles, paragraph styles
/// (each range a paragraph of its own; nested ones merge, partly overlapping ones panic), string annotations,
/// text-to-speech and URL annotations, links. Offsets are UTF-8 bytes. Cheap to clone; equal by text and
/// annotations (in their order).
#[derive(Clone)]
pub struct AnnotatedString {
    text: Arc<str>,
    annotations: Arc<[Range<Annotation>]>,
    span_styles: Arc<[Range<SpanStyle>]>,
    paragraph_styles: Arc<[Range<ParagraphStyle>]>,
}

impl AnnotatedString {
    /// Plain text.
    pub fn new(text: impl Into<Arc<str>>) -> AnnotatedString {
        AnnotatedString::from_annotations(text, Vec::new())
    }

    /// Text with these span and paragraph styles (Compose's AnnotatedString(text, spanStyles, paragraphStyles)).
    pub fn with_styles(
        text: impl Into<Arc<str>>,
        span_styles: Vec<Range<SpanStyle>>,
        paragraph_styles: Vec<Range<ParagraphStyle>>,
    ) -> AnnotatedString {
        let annotations = span_styles
            .into_iter()
            .map(|r| Range::tagged(Annotation::SpanStyle(r.item), r.start, r.end, r.tag))
            .chain(
                paragraph_styles
                    .into_iter()
                    .map(|r| Range::tagged(Annotation::ParagraphStyle(r.item), r.start, r.end, r.tag)),
            )
            .collect();
        AnnotatedString::from_annotations(text, annotations)
    }

    /// All of `text` in a span style, and a paragraph style if given (Compose's AnnotatedString(text, spanStyle,
    /// paragraphStyle)).
    pub fn styled(
        text: impl Into<Arc<str>>,
        span_style: SpanStyle,
        paragraph_style: Option<ParagraphStyle>,
    ) -> AnnotatedString {
        let text = text.into();
        let len = text.len();
        AnnotatedString::with_styles(
            text,
            vec![Range::new(span_style, 0, len)],
            paragraph_style.map(|p| Range::new(p, 0, len)).into_iter().collect(),
        )
    }

    /// All of `text` in a paragraph style (Compose's AnnotatedString(text, paragraphStyle)).
    pub fn with_paragraph_style(text: impl Into<Arc<str>>, paragraph_style: ParagraphStyle) -> AnnotatedString {
        let text = text.into();
        let len = text.len();
        AnnotatedString::with_styles(text, Vec::new(), vec![Range::new(paragraph_style, 0, len)])
    }

    /// Text with these annotations, in this order (Compose's AnnotatedString(text, annotations)). Paragraph styles
    /// that partly overlap panic, as Compose's do.
    pub fn from_annotations(text: impl Into<Arc<str>>, annotations: Vec<Range<Annotation>>) -> AnnotatedString {
        let mut span_styles = Vec::new();
        let mut paragraph_styles = Vec::new();
        for a in &annotations {
            match &a.item {
                Annotation::SpanStyle(s) => span_styles.push(Range::tagged(s.clone(), a.start, a.end, a.tag.clone())),
                Annotation::ParagraphStyle(p) => {
                    paragraph_styles.push(Range::tagged(p.clone(), a.start, a.end, a.tag.clone()))
                }
                _ => {}
            }
        }
        check_paragraphs_nest(&paragraph_styles);
        AnnotatedString {
            text: text.into(),
            annotations: annotations.into(),
            span_styles: span_styles.into(),
            paragraph_styles: paragraph_styles.into(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    /// Its text, shared.
    pub fn text_arc(&self) -> Arc<str> {
        self.text.clone()
    }
    /// Its length in bytes.
    pub fn len(&self) -> usize {
        self.text.len()
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    /// All its annotations, in their order.
    pub fn annotations(&self) -> &[Range<Annotation>] {
        &self.annotations
    }
    pub fn span_styles(&self) -> &[Range<SpanStyle>] {
        &self.span_styles
    }
    pub fn paragraph_styles(&self) -> &[Range<ParagraphStyle>] {
        &self.paragraph_styles
    }

    /// Its part `start..end` (byte offsets on character boundaries), with the annotations that meet it (as
    /// [`intersect`] says) cut to it (Compose's subSequence). A range ending before it starts panics.
    pub fn sub_sequence(&self, start: usize, end: usize) -> AnnotatedString {
        assert!(start <= end, "kompoz: start ({start}) should be less or equal to end ({end})");
        if start == 0 && end == self.text.len() {
            return self.clone();
        }
        let annotations = self
            .annotations
            .iter()
            .filter(|a| intersect(start, end, a.start, a.end))
            .map(|a| a.moved(start.max(a.start) - start, end.min(a.end) - start))
            .collect();
        AnnotatedString::from_annotations(&self.text[start..end], annotations)
    }

    /// Its part a range covers.
    pub fn sub_sequence_range(&self, range: TextRange) -> AnnotatedString {
        self.sub_sequence(range.min(), range.max())
    }

    /// The string annotations of `tag` meeting `start..end` (an offset finds the ones around it: Compose's
    /// getStringAnnotations, ClickableText's lookup).
    pub fn get_string_annotations(&self, tag: &str, start: usize, end: usize) -> Vec<Range<String>> {
        self.string_annotations(start, end).filter(|a| a.tag == tag).collect()
    }

    /// Whether any string annotation of `tag` meets `start..end`.
    pub fn has_string_annotations(&self, tag: &str, start: usize, end: usize) -> bool {
        self.string_annotations(start, end).any(|a| a.tag == tag)
    }

    /// The string annotations of any tag meeting `start..end`.
    pub fn get_all_string_annotations(&self, start: usize, end: usize) -> Vec<Range<String>> {
        self.string_annotations(start, end).collect()
    }

    fn string_annotations(&self, start: usize, end: usize) -> impl Iterator<Item = Range<String>> + '_ {
        self.annotations.iter().filter(move |a| intersect(start, end, a.start, a.end)).filter_map(|a| match &a.item {
            Annotation::String(s) => Some(Range::tagged(s.clone(), a.start, a.end, a.tag.clone())),
            _ => None,
        })
    }

    /// The text-to-speech annotations meeting `start..end`.
    pub fn get_tts_annotations(&self, start: usize, end: usize) -> Vec<Range<TtsAnnotation>> {
        self.items(start, end, |a| if let Annotation::Tts(t) = a { Some(t.clone()) } else { None })
    }

    /// The URL annotations meeting `start..end`.
    pub fn get_url_annotations(&self, start: usize, end: usize) -> Vec<Range<UrlAnnotation>> {
        self.items(start, end, |a| if let Annotation::Url(u) = a { Some(u.clone()) } else { None })
    }

    /// The links meeting `start..end`.
    pub fn get_link_annotations(&self, start: usize, end: usize) -> Vec<Range<LinkAnnotation>> {
        self.items(start, end, |a| if let Annotation::Link(l) = a { Some((**l).clone()) } else { None })
    }

    /// Whether any link meets `start..end`.
    pub fn has_link_annotations(&self, start: usize, end: usize) -> bool {
        self.annotations.iter().any(|a| matches!(a.item, Annotation::Link(_)) && intersect(start, end, a.start, a.end))
    }

    fn items<T>(&self, start: usize, end: usize, pick: impl Fn(&Annotation) -> Option<T>) -> Vec<Range<T>> {
        self.annotations
            .iter()
            .filter(|a| intersect(start, end, a.start, a.end))
            .filter_map(|a| pick(&a.item).map(|item| Range::tagged(item, a.start, a.end, a.tag.clone())))
            .collect()
    }

    /// Whether the two have the same annotations (their texts aside).
    pub fn has_equal_annotations(&self, other: &AnnotatedString) -> bool {
        self.annotations == other.annotations
    }

    /// The same text, each annotation replaced by what `transform` makes of it (Compose's mapAnnotations).
    pub fn map_annotations(&self, transform: impl FnMut(&Range<Annotation>) -> Range<Annotation>) -> AnnotatedString {
        let mut b = AnnotatedStringBuilder::from(self);
        b.map_annotations(transform);
        b.to_annotated_string()
    }

    /// The same text, each annotation replaced by the ones `transform` makes of it (Compose's
    /// flatMapAnnotations).
    pub fn flat_map_annotations(
        &self,
        transform: impl FnMut(&Range<Annotation>) -> Vec<Range<Annotation>>,
    ) -> AnnotatedString {
        let mut b = AnnotatedStringBuilder::from(self);
        b.flat_map_annotations(transform);
        b.to_annotated_string()
    }

    /// Its paragraph styles as the paragraphs they make, each merged over `default` (and nested ones over the ones
    /// around them), the stretches between them in `default`, in order and covering the whole text (Compose's
    /// normalizedParagraphStyles: what lays a text out paragraph by paragraph).
    pub fn normalized_paragraph_styles(&self, default: &ParagraphStyle) -> Vec<Range<ParagraphStyle>> {
        let mut sorted: Vec<&Range<ParagraphStyle>> = self.paragraph_styles.iter().collect();
        sorted.sort_by_key(|r| r.start);
        let length = self.text.len();
        let mut result: Vec<Range<ParagraphStyle>> = Vec::new();
        let mut last_added = 0;
        let mut stack: Vec<Range<ParagraphStyle>> = Vec::new();
        for r in sorted {
            let current = Range::new(default.merge(Some(&r.item)), r.start, r.end);
            while last_added < current.start && !stack.is_empty() {
                let top = stack.last().expect("stack").clone();
                if current.start < top.end {
                    result.push(Range::new(top.item, last_added, current.start));
                    last_added = current.start;
                } else {
                    result.push(Range::new(top.item, last_added, top.end));
                    last_added = top.end;
                    while stack.last().is_some_and(|t| t.end == last_added) {
                        stack.pop();
                    }
                }
            }
            if last_added < current.start {
                result.push(Range::new(default.clone(), last_added, current.start));
                last_added = current.start;
            }
            match stack.last().cloned() {
                Some(top) if top.start == current.start && top.end == current.end => {
                    stack.pop();
                    stack.push(Range::new(top.item.merge(Some(&current.item)), current.start, current.end));
                }
                Some(top) if top.start == top.end => {
                    result.push(Range::new(top.item.clone(), top.start, top.end));
                    stack.pop();
                    stack.push(current);
                }
                Some(top) if top.end < current.end => {
                    panic!("kompoz: paragraph styles may not partly overlap")
                }
                Some(top) => stack.push(Range::new(top.item.merge(Some(&current.item)), current.start, current.end)),
                None => stack.push(current),
            }
        }
        while last_added <= length && !stack.is_empty() {
            let top = stack.last().expect("stack").clone();
            result.push(Range::new(top.item, last_added, top.end));
            last_added = top.end;
            while stack.last().is_some_and(|t| t.end == last_added) {
                stack.pop();
            }
        }
        if last_added < length {
            result.push(Range::new(default.clone(), last_added, length));
        }
        if result.is_empty() {
            result.push(Range::new(default.clone(), 0, 0));
        }
        result
    }

    /// Its part `start..end` without its paragraph styles (Compose's substringWithoutParagraphStyles).
    pub fn substring_without_paragraph_styles(&self, start: usize, end: usize) -> AnnotatedString {
        let annotations = if start == end {
            Vec::new()
        } else {
            self.local_annotations(start, end)
                .into_iter()
                .filter(|a| !matches!(a.item, Annotation::ParagraphStyle(_)))
                .collect()
        };
        AnnotatedString::from_annotations(&self.text[start..end], annotations)
    }

    /// Each paragraph [`normalized_paragraph_styles`](AnnotatedString::normalized_paragraph_styles) makes, as its
    /// text (its paragraph styles left out) and its style, given to `block` (Compose's mapEachParagraphStyle).
    pub fn map_each_paragraph_style<T>(
        &self,
        default: &ParagraphStyle,
        mut block: impl FnMut(AnnotatedString, &Range<ParagraphStyle>) -> T,
    ) -> Vec<T> {
        self.normalized_paragraph_styles(default)
            .iter()
            .map(|p| block(self.substring_without_paragraph_styles(p.start, p.end), p))
            .collect()
    }

    /// The annotations meeting `start..end`, cut to it and relative to its start (Compose's getLocalAnnotations).
    fn local_annotations(&self, start: usize, end: usize) -> Vec<Range<Annotation>> {
        if start == end {
            return Vec::new();
        }
        if start == 0 && end >= self.text.len() {
            return self.annotations.to_vec();
        }
        self.annotations
            .iter()
            .filter(|a| intersect(start, end, a.start, a.end))
            .map(|a| a.moved(a.start.clamp(start, end) - start, a.end.clamp(start, end) - start))
            .collect()
    }

    /// Its text in upper case as `locales`' first locale (the current one if none) writes it, the annotations
    /// kept on what they covered (Compose's toUpperCase).
    pub fn to_upper_case(&self, locales: &LocaleList) -> AnnotatedString {
        let locale = locales.first_or_current();
        self.transform(|s, _| to_upper_case(s, &locale))
    }

    /// In lower case (Compose's toLowerCase).
    pub fn to_lower_case(&self, locales: &LocaleList) -> AnnotatedString {
        let locale = locales.first_or_current();
        self.transform(|s, _| to_lower_case(s, &locale))
    }

    /// Its first character in title case (Compose's capitalize).
    pub fn capitalize(&self, locales: &LocaleList) -> AnnotatedString {
        let locale = locales.first_or_current();
        self.transform(|s, start| if start == 0 { capitalize(s, &locale) } else { s.to_string() })
    }

    /// Its first character in lower case (Compose's decapitalize).
    pub fn decapitalize(&self, locales: &LocaleList) -> AnnotatedString {
        let locale = locales.first_or_current();
        self.transform(|s, start| if start == 0 { decapitalize(s, &locale) } else { s.to_string() })
    }

    /// The text changed piece by piece between annotation ends (`transform` given each piece and where it
    /// starts), the annotations moved onto what their pieces became (Compose's transform).
    fn transform(&self, mut transform: impl FnMut(&str, usize) -> String) -> AnnotatedString {
        let mut cuts: BTreeSet<usize> = [0, self.text.len()].into_iter().collect();
        for a in self.annotations.iter() {
            cuts.insert(a.start);
            cuts.insert(a.end);
        }
        let cuts: Vec<usize> = cuts.into_iter().collect();
        let mut out = String::with_capacity(self.text.len());
        let mut moved = std::collections::HashMap::from([(0usize, 0usize)]);
        for w in cuts.windows(2) {
            out.push_str(&transform(&self.text[w[0]..w[1]], w[0]));
            moved.insert(w[1], out.len());
        }
        let annotations = self.annotations.iter().map(|a| a.moved(moved[&a.start], moved[&a.end])).collect();
        AnnotatedString::from_annotations(out, annotations)
    }
}

/// Panics if two paragraph styles partly overlap (Compose's check: nested or apart).
fn check_paragraphs_nest(paragraphs: &[Range<ParagraphStyle>]) {
    if paragraphs.len() < 2 {
        return;
    }
    let mut sorted: Vec<(usize, usize)> = paragraphs.iter().map(|p| (p.start, p.end)).collect();
    sorted.sort_by_key(|p| p.0);
    let mut ends = vec![sorted[0].1];
    for &(start, end) in &sorted[1..] {
        while let Some(&previous_end) = ends.last() {
            if start >= previous_end {
                ends.pop();
            } else {
                assert!(
                    end <= previous_end,
                    "kompoz: Paragraph overlap not allowed, end {end} should be less than or equal to {previous_end}"
                );
                break;
            }
        }
        ends.push(end);
    }
}

/// Empty.
impl Default for AnnotatedString {
    fn default() -> AnnotatedString {
        AnnotatedString::new("")
    }
}

impl PartialEq for AnnotatedString {
    fn eq(&self, o: &AnnotatedString) -> bool {
        self.text == o.text && self.annotations == o.annotations
    }
}

/// Its text, then its annotations.
impl fmt::Debug for AnnotatedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.annotations.is_empty() {
            write!(f, "{:?}", self.text)
        } else {
            f.debug_struct("AnnotatedString").field("text", &self.text).field("annotations", &self.annotations).finish()
        }
    }
}

/// Its text (Compose's toString).
impl fmt::Display for AnnotatedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl From<&str> for AnnotatedString {
    fn from(text: &str) -> AnnotatedString {
        AnnotatedString::new(text)
    }
}
impl From<String> for AnnotatedString {
    fn from(text: String) -> AnnotatedString {
        AnnotatedString::new(text)
    }
}
impl From<&String> for AnnotatedString {
    fn from(text: &String) -> AnnotatedString {
        AnnotatedString::new(text.as_str())
    }
}
impl From<&AnnotatedString> for AnnotatedString {
    fn from(text: &AnnotatedString) -> AnnotatedString {
        text.clone()
    }
}

/// Both, one after the other, with their annotations (Compose's plus).
impl std::ops::Add<&AnnotatedString> for &AnnotatedString {
    type Output = AnnotatedString;
    fn add(self, other: &AnnotatedString) -> AnnotatedString {
        let mut b = AnnotatedStringBuilder::from(self);
        b.append(other);
        b.to_annotated_string()
    }
}

// ---- The builder ------------------------------------------------------------------------------------------------

/// An annotation being built: its end not known while it is pushed.
#[derive(Clone)]
struct MutableRange {
    item: Annotation,
    start: usize,
    end: Option<usize>,
    tag: String,
}

impl MutableRange {
    fn of(r: &Range<Annotation>) -> MutableRange {
        MutableRange { item: r.item.clone(), start: r.start, end: Some(r.end), tag: r.tag.clone() }
    }
    fn to_range(&self, default_end: usize) -> Range<Annotation> {
        Range::tagged(self.item.clone(), self.start, self.end.unwrap_or(default_end), self.tag.clone())
    }
}

/// Builds an [`AnnotatedString`] (Compose's AnnotatedString.Builder): text appended, styles and annotations added
/// on ranges, or pushed on what is appended until popped (an annotation goes in when it is pushed, so an outer
/// style comes before the ones inside it).
#[derive(Clone, Default)]
pub struct AnnotatedStringBuilder {
    text: String,
    style_stack: Vec<usize>,
    annotations: Vec<MutableRange>,
}

/// What a builder appends ([`AnnotatedStringBuilder::append`]): text, a character, or an annotated string with
/// its annotations.
pub trait BuilderAppend {
    fn append_to(self, builder: &mut AnnotatedStringBuilder);
}

impl BuilderAppend for &str {
    fn append_to(self, b: &mut AnnotatedStringBuilder) {
        b.text.push_str(self);
    }
}
impl BuilderAppend for String {
    fn append_to(self, b: &mut AnnotatedStringBuilder) {
        b.text.push_str(&self);
    }
}
impl BuilderAppend for &String {
    fn append_to(self, b: &mut AnnotatedStringBuilder) {
        b.text.push_str(self);
    }
}
impl BuilderAppend for char {
    fn append_to(self, b: &mut AnnotatedStringBuilder) {
        b.text.push(self);
    }
}
impl BuilderAppend for &AnnotatedString {
    fn append_to(self, b: &mut AnnotatedStringBuilder) {
        let start = b.text.len();
        b.text.push_str(&self.text);
        for a in self.annotations.iter() {
            b.annotations.push(MutableRange {
                item: a.item.clone(),
                start: start + a.start,
                end: Some(start + a.end),
                tag: a.tag.clone(),
            });
        }
    }
}
impl BuilderAppend for AnnotatedString {
    fn append_to(self, b: &mut AnnotatedStringBuilder) {
        (&self).append_to(b);
    }
}

/// A style a builder adds or pushes: a span style or a paragraph style.
pub trait BuilderStyle {
    fn into_annotation(self) -> Annotation;
}
impl BuilderStyle for SpanStyle {
    fn into_annotation(self) -> Annotation {
        Annotation::SpanStyle(self)
    }
}
impl BuilderStyle for ParagraphStyle {
    fn into_annotation(self) -> Annotation {
        Annotation::ParagraphStyle(self)
    }
}

impl AnnotatedStringBuilder {
    pub fn new() -> AnnotatedStringBuilder {
        AnnotatedStringBuilder::default()
    }
    /// Room for `capacity` bytes of text.
    pub fn with_capacity(capacity: usize) -> AnnotatedStringBuilder {
        AnnotatedStringBuilder { text: String::with_capacity(capacity), ..Default::default() }
    }

    /// The text so far's length (bytes): where what is appended next starts.
    pub fn len(&self) -> usize {
        self.text.len()
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Appends text, a character, or an annotated string with its annotations.
    pub fn append(&mut self, text: impl BuilderAppend) {
        text.append_to(self);
    }

    /// Appends `text`'s part `start..end` with the annotations meeting it, cut to it (Compose's append(text,
    /// start, end)).
    pub fn append_range(&mut self, text: &AnnotatedString, start: usize, end: usize) {
        let at = self.text.len();
        self.text.push_str(&text.text[start..end]);
        for a in text.local_annotations(start, end) {
            self.annotations.push(MutableRange {
                item: a.item,
                start: at + a.start,
                end: Some(at + a.end),
                tag: a.tag,
            });
        }
    }

    /// A span or paragraph style on `start..end`.
    pub fn add_style(&mut self, style: impl BuilderStyle, start: usize, end: usize) {
        self.add(style.into_annotation(), start, end, "");
    }
    /// A string annotation: `tag`'s item `annotation` on `start..end`.
    pub fn add_string_annotation(
        &mut self,
        tag: impl Into<String>,
        annotation: impl Into<String>,
        start: usize,
        end: usize,
    ) {
        self.add(Annotation::String(annotation.into()), start, end, tag);
    }
    pub fn add_tts_annotation(&mut self, annotation: TtsAnnotation, start: usize, end: usize) {
        self.add(Annotation::Tts(annotation), start, end, "");
    }
    pub fn add_url_annotation(&mut self, annotation: UrlAnnotation, start: usize, end: usize) {
        self.add(Annotation::Url(annotation), start, end, "");
    }
    /// A link on `start..end`.
    pub fn add_link(&mut self, link: LinkAnnotation, start: usize, end: usize) {
        self.add(Annotation::Link(Box::new(link)), start, end, "");
    }

    fn add(&mut self, item: Annotation, start: usize, end: usize, tag: impl Into<String>) {
        self.annotations.push(MutableRange { item, start, end: Some(end), tag: tag.into() });
    }

    /// A span or paragraph style on what is appended until it is popped; its index (for [`pop_to`](Self::pop_to)).
    pub fn push_style(&mut self, style: impl BuilderStyle) -> usize {
        self.push(style.into_annotation(), "")
    }
    /// A string annotation on what is appended until it is popped.
    pub fn push_string_annotation(&mut self, tag: impl Into<String>, annotation: impl Into<String>) -> usize {
        self.push(Annotation::String(annotation.into()), tag)
    }
    pub fn push_tts_annotation(&mut self, annotation: TtsAnnotation) -> usize {
        self.push(Annotation::Tts(annotation), "")
    }
    pub fn push_url_annotation(&mut self, annotation: UrlAnnotation) -> usize {
        self.push(Annotation::Url(annotation), "")
    }
    /// A link on what is appended until it is popped.
    pub fn push_link(&mut self, link: LinkAnnotation) -> usize {
        self.push(Annotation::Link(Box::new(link)), "")
    }

    fn push(&mut self, item: Annotation, tag: impl Into<String>) -> usize {
        self.annotations.push(MutableRange { item, start: self.text.len(), end: None, tag: tag.into() });
        self.style_stack.push(self.annotations.len() - 1);
        self.style_stack.len() - 1
    }

    /// Ends the style or annotation pushed last, here. Nothing pushed panics (as Compose's).
    pub fn pop(&mut self) {
        let at = self.style_stack.pop().expect("kompoz: Nothing to pop.");
        self.annotations[at].end = Some(self.text.len());
    }

    /// Ends the one pushed as `index` and all pushed after it (Compose's pop(index)); an index not pushed panics.
    pub fn pop_to(&mut self, index: usize) {
        assert!(index < self.style_stack.len(), "kompoz: {index} should be less than {}", self.style_stack.len());
        while self.style_stack.len() > index {
            self.pop();
        }
    }

    /// What `block` appends in a span or paragraph style (Compose's withStyle).
    pub fn with_style<R>(&mut self, style: impl BuilderStyle, block: impl FnOnce(&mut Self) -> R) -> R {
        let index = self.push_style(style);
        let r = block(self);
        self.pop_to(index);
        r
    }
    /// What `block` appends annotated with `tag`'s `annotation` (Compose's withAnnotation(tag, annotation)).
    pub fn with_annotation<R>(
        &mut self,
        tag: impl Into<String>,
        annotation: impl Into<String>,
        block: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let index = self.push_string_annotation(tag, annotation);
        let r = block(self);
        self.pop_to(index);
        r
    }
    /// What `block` appends read as `annotation` says (Compose's withAnnotation(ttsAnnotation)).
    pub fn with_tts_annotation<R>(&mut self, annotation: TtsAnnotation, block: impl FnOnce(&mut Self) -> R) -> R {
        let index = self.push_tts_annotation(annotation);
        let r = block(self);
        self.pop_to(index);
        r
    }
    /// What `block` appends with a URL annotation (Compose's withAnnotation(urlAnnotation)).
    pub fn with_url_annotation<R>(&mut self, annotation: UrlAnnotation, block: impl FnOnce(&mut Self) -> R) -> R {
        let index = self.push_url_annotation(annotation);
        let r = block(self);
        self.pop_to(index);
        r
    }
    /// What `block` appends a link (Compose's withLink): no look of its own unless its styles give one.
    pub fn with_link<R>(&mut self, link: LinkAnnotation, block: impl FnOnce(&mut Self) -> R) -> R {
        let index = self.push_link(link);
        let r = block(self);
        self.pop_to(index);
        r
    }

    /// Room for a Text's inline content `id`, its `alternate_text` standing for it (what copying the text and
    /// accessibility get) (Compose foundation's appendInlineContent).
    pub fn append_inline_content_with(&mut self, id: impl Into<String>, alternate_text: &str) {
        self.push_string_annotation(INLINE_CONTENT_TAG, id);
        self.append(alternate_text);
        self.pop();
    }

    /// The annotated string built (what is still pushed ends with the text).
    pub fn to_annotated_string(&self) -> AnnotatedString {
        let end = self.text.len();
        AnnotatedString::from_annotations(
            self.text.as_str(),
            self.annotations.iter().map(|a| a.to_range(end)).collect(),
        )
    }

    /// Each annotation replaced by what `transform` makes of it (its end now if still pushed).
    pub fn map_annotations(&mut self, mut transform: impl FnMut(&Range<Annotation>) -> Range<Annotation>) {
        let end = self.text.len();
        for a in self.annotations.iter_mut() {
            *a = MutableRange::of(&transform(&a.to_range(end)));
        }
    }

    /// Each annotation replaced by the ones `transform` makes of it.
    pub fn flat_map_annotations(&mut self, mut transform: impl FnMut(&Range<Annotation>) -> Vec<Range<Annotation>>) {
        let end = self.text.len();
        let replaced: Vec<MutableRange> =
            self.annotations.iter().flat_map(|a| transform(&a.to_range(end))).map(|r| MutableRange::of(&r)).collect();
        self.annotations = replaced;
        self.style_stack.clear();
    }
}

#[named]
impl AnnotatedStringBuilder {
    /// Room for a Text's inline content `id`, `alternate_text` (U+FFFD unless given) standing for it (Compose
    /// foundation's appendInlineContent(id, alternateText = REPLACEMENT_CHAR)).
    pub fn append_inline_content(&mut self, id: String, #[default("\u{FFFD}".to_string())] alternate_text: String) {
        self.append_inline_content_with(id, &alternate_text);
    }
}

/// A builder starting with a text's text and annotations (Compose's Builder(text)).
impl From<&AnnotatedString> for AnnotatedStringBuilder {
    fn from(text: &AnnotatedString) -> AnnotatedStringBuilder {
        let mut b = AnnotatedStringBuilder::with_capacity(text.len());
        b.append(text);
        b
    }
}

/// A builder starting with plain text.
impl From<&str> for AnnotatedStringBuilder {
    fn from(text: &str) -> AnnotatedStringBuilder {
        let mut b = AnnotatedStringBuilder::with_capacity(text.len());
        b.append(text);
        b
    }
}

/// An annotated string `build` builds (Compose's buildAnnotatedString).
pub fn build_annotated_string(build: impl FnOnce(&mut AnnotatedStringBuilder)) -> AnnotatedString {
    let mut b = AnnotatedStringBuilder::new();
    build(&mut b);
    b.to_annotated_string()
}
