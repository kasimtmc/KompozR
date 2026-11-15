//! A text's whole style (Compose's TextStyle): a span style's attributes and a paragraph style's settings.

use crate::font::{FontFamily, FontStyle, FontSynthesis, FontWeight};
use crate::intl::LocaleList;
use crate::paragraph_style::{ParagraphStyle, lerp_paragraph_style};
use crate::platform_style::PlatformTextStyle;
use crate::span_style::{SpanStyle, TextForegroundStyle, copied_foreground, lerp_span_style, set_foreground};
use crate::style::{
    BaselineShift, Hyphens, LineBreak, LineHeightStyle, TextAlign, TextDecoration, TextDirection,
    TextGeometricTransform, TextIndent, TextMotion,
};
use kompoz_graphics::{Brush, Color, DrawStyle, LayoutDirection, Shadow, TextUnit};
use kompoz_runtime::named;
use std::fmt;

/// A text's whole style (Compose's TextStyle): what a [`SpanStyle`](struct@SpanStyle) gives a stretch of text
/// and what a [`ParagraphStyle`](struct@ParagraphStyle) gives its paragraphs, plus the platform's settings of
/// both. What is not given (None, unspecified) is the style's around it ([`merge`](TextStyle::merge)), and in the
/// end Compose's default ([`resolve_defaults`](TextStyle::resolve_defaults)). What paints it is `color`, or
/// `brush` at `alpha`, as a span style's. Write one as `TextStyle { font_size: 20.sp(), ..TextStyle::Default }`,
/// or by name in a kompozable body: `TextStyle(font_size = 20.sp())`.
#[derive(Clone)]
pub struct TextStyle {
    pub color: Color,
    pub brush: Option<Brush>,
    pub alpha: f32,
    pub font_size: TextUnit,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub font_synthesis: Option<FontSynthesis>,
    pub font_family: Option<FontFamily>,
    pub font_feature_settings: Option<String>,
    pub letter_spacing: TextUnit,
    pub baseline_shift: Option<BaselineShift>,
    pub text_geometric_transform: Option<TextGeometricTransform>,
    pub locale_list: Option<LocaleList>,
    pub background: Color,
    pub text_decoration: Option<TextDecoration>,
    pub shadow: Option<Shadow>,
    pub draw_style: Option<DrawStyle>,
    pub text_align: TextAlign,
    pub text_direction: TextDirection,
    pub line_height: TextUnit,
    pub text_indent: Option<TextIndent>,
    pub platform_style: Option<PlatformTextStyle>,
    pub line_height_style: Option<LineHeightStyle>,
    pub line_break: LineBreak,
    pub hyphens: Hyphens,
    pub text_motion: Option<TextMotion>,
}

impl TextStyle {
    /// Nothing given (Compose's TextStyle.Default).
    pub const Default: TextStyle = TextStyle {
        color: Color::Unspecified,
        brush: None,
        alpha: f32::NAN,
        font_size: TextUnit::Unspecified,
        font_weight: None,
        font_style: None,
        font_synthesis: None,
        font_family: None,
        font_feature_settings: None,
        letter_spacing: TextUnit::Unspecified,
        baseline_shift: None,
        text_geometric_transform: None,
        locale_list: None,
        background: Color::Unspecified,
        text_decoration: None,
        shadow: None,
        draw_style: None,
        text_align: TextAlign::Unspecified,
        text_direction: TextDirection::Unspecified,
        line_height: TextUnit::Unspecified,
        text_indent: None,
        platform_style: None,
        line_height_style: None,
        line_break: LineBreak::Unspecified,
        hyphens: Hyphens::Unspecified,
        text_motion: None,
    };

    /// A span style's attributes and a paragraph style's settings (the platform style theirs together).
    pub fn new(span: &SpanStyle, paragraph: &ParagraphStyle) -> TextStyle {
        TextStyle {
            color: span.color,
            brush: span.brush.clone(),
            alpha: span.alpha,
            font_size: span.font_size,
            font_weight: span.font_weight,
            font_style: span.font_style,
            font_synthesis: span.font_synthesis,
            font_family: span.font_family.clone(),
            font_feature_settings: span.font_feature_settings.clone(),
            letter_spacing: span.letter_spacing,
            baseline_shift: span.baseline_shift,
            text_geometric_transform: span.text_geometric_transform,
            locale_list: span.locale_list.clone(),
            background: span.background,
            text_decoration: span.text_decoration,
            shadow: span.shadow.clone(),
            draw_style: span.draw_style.clone(),
            text_align: paragraph.text_align,
            text_direction: paragraph.text_direction,
            line_height: paragraph.line_height,
            text_indent: paragraph.text_indent,
            platform_style: PlatformTextStyle::of(span.platform_style, paragraph.platform_style),
            line_height_style: paragraph.line_height_style,
            line_break: paragraph.line_break,
            hyphens: paragraph.hyphens,
            text_motion: paragraph.text_motion,
        }
    }

    /// What paints it.
    pub fn foreground(&self) -> TextForegroundStyle {
        self.to_span_style().foreground()
    }

    /// Its span style's attributes (Compose's toSpanStyle).
    pub fn to_span_style(&self) -> SpanStyle {
        SpanStyle {
            color: self.color,
            brush: self.brush.clone(),
            alpha: self.alpha,
            font_size: self.font_size,
            font_weight: self.font_weight,
            font_style: self.font_style,
            font_synthesis: self.font_synthesis,
            font_family: self.font_family.clone(),
            font_feature_settings: self.font_feature_settings.clone(),
            letter_spacing: self.letter_spacing,
            baseline_shift: self.baseline_shift,
            text_geometric_transform: self.text_geometric_transform,
            locale_list: self.locale_list.clone(),
            background: self.background,
            text_decoration: self.text_decoration,
            shadow: self.shadow.clone(),
            platform_style: self.platform_style.and_then(|p| p.span_style),
            draw_style: self.draw_style.clone(),
        }
    }

    /// Its paragraph style's settings (Compose's toParagraphStyle).
    pub fn to_paragraph_style(&self) -> ParagraphStyle {
        ParagraphStyle {
            text_align: self.text_align,
            text_direction: self.text_direction,
            line_height: self.line_height,
            text_indent: self.text_indent,
            platform_style: self.platform_style.and_then(|p| p.paragraph_style),
            line_height_style: self.line_height_style,
            line_break: self.line_break,
            hyphens: self.hyphens,
            text_motion: self.text_motion,
        }
    }

    /// `other` over this (Compose's merge): what `other` gives replaces this one's — another text style (None or
    /// the default: this), a span style's attributes, or a paragraph style's settings.
    pub fn merge<'a>(&self, other: impl Into<TextStyleMerge<'a>>) -> TextStyle {
        match other.into() {
            TextStyleMerge::Text(None) => self.clone(),
            TextStyleMerge::Text(Some(o)) => {
                if *o == TextStyle::Default {
                    return self.clone();
                }
                TextStyle::new(
                    &self.to_span_style().merge(Some(&o.to_span_style())),
                    &self.to_paragraph_style().merge(Some(&o.to_paragraph_style())),
                )
            }
            TextStyleMerge::Span(o) => TextStyle::new(&self.to_span_style().merge(Some(o)), &self.to_paragraph_style()),
            TextStyleMerge::Paragraph(o) => {
                TextStyle::new(&self.to_span_style(), &self.to_paragraph_style().merge(Some(o)))
            }
        }
    }

    /// Every attribute and setting given: Compose's defaults where none is, the direction resolved in
    /// `direction` (Compose's resolveDefaults). See [`SpanStyle::resolve_defaults`] and
    /// [`ParagraphStyle::resolve_defaults`].
    pub fn resolve_defaults(&self, direction: LayoutDirection) -> TextStyle {
        let mut style = TextStyle::new(
            &self.to_span_style().resolve_defaults(),
            &self.to_paragraph_style().resolve_defaults(direction),
        );
        style.platform_style = self.platform_style;
        style
    }

    /// Whether the two lay text out alike (Compose's hasSameLayoutAffectingAttributes): the same paragraph
    /// settings and span attributes but for what only paints.
    pub fn has_same_layout_affecting_attributes(&self, other: &TextStyle) -> bool {
        self.to_paragraph_style() == other.to_paragraph_style()
            && self.to_span_style().has_same_layout_affecting_attributes(&other.to_span_style())
    }

    /// Whether the two paint alike (Compose's hasSameDrawAffectingAttributes).
    pub fn has_same_draw_affecting_attributes(&self, other: &TextStyle) -> bool {
        self.to_span_style().has_same_non_layout_attributes(&other.to_span_style())
    }
}

/// What a text style merges ([`TextStyle::merge`]): another text style, a span style, a paragraph style.
pub enum TextStyleMerge<'a> {
    Text(Option<&'a TextStyle>),
    Span(&'a SpanStyle),
    Paragraph(&'a ParagraphStyle),
}

impl<'a> From<&'a TextStyle> for TextStyleMerge<'a> {
    fn from(s: &'a TextStyle) -> Self {
        TextStyleMerge::Text(Some(s))
    }
}
impl<'a> From<Option<&'a TextStyle>> for TextStyleMerge<'a> {
    fn from(s: Option<&'a TextStyle>) -> Self {
        TextStyleMerge::Text(s)
    }
}
impl<'a> From<&'a SpanStyle> for TextStyleMerge<'a> {
    fn from(s: &'a SpanStyle) -> Self {
        TextStyleMerge::Span(s)
    }
}
impl<'a> From<&'a ParagraphStyle> for TextStyleMerge<'a> {
    fn from(s: &'a ParagraphStyle) -> Self {
        TextStyleMerge::Paragraph(s)
    }
}

/// Nothing given.
impl Default for TextStyle {
    fn default() -> TextStyle {
        TextStyle::Default
    }
}

/// Equal span attributes (the foreground as it comes to), paragraph settings and platform style.
impl PartialEq for TextStyle {
    fn eq(&self, o: &TextStyle) -> bool {
        self.to_span_style() == o.to_span_style()
            && self.to_paragraph_style() == o.to_paragraph_style()
            && self.platform_style == o.platform_style
    }
}

/// Its attributes and settings, as Compose writes them.
impl fmt::Debug for TextStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TextStyle")
            .field("span", &self.to_span_style())
            .field("paragraph", &self.to_paragraph_style())
            .field("platform_style", &self.platform_style)
            .finish()
    }
}

/// `other` over this ([`TextStyle::merge`]: Compose's `plus`).
impl std::ops::Add<&TextStyle> for &TextStyle {
    type Output = TextStyle;
    fn add(self, other: &TextStyle) -> TextStyle {
        self.merge(other)
    }
}
impl std::ops::Add<&SpanStyle> for &TextStyle {
    type Output = TextStyle;
    fn add(self, other: &SpanStyle) -> TextStyle {
        self.merge(other)
    }
}
impl std::ops::Add<&ParagraphStyle> for &TextStyle {
    type Output = TextStyle;
    fn add(self, other: &ParagraphStyle) -> TextStyle {
        self.merge(other)
    }
}

#[named]
impl TextStyle {
    /// The same with what is given replaced (Compose's copy): a brush given (one other than its own) paints it,
    /// else a color given (one other than its own) does.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.foreground().color())] color: Color,
        #[default(self.foreground().brush().cloned())] brush: Option<Brush>,
        #[default(self.foreground().alpha())] alpha: f32,
        #[default(self.font_size)] font_size: TextUnit,
        #[default(self.font_weight)] font_weight: Option<FontWeight>,
        #[default(self.font_style)] font_style: Option<FontStyle>,
        #[default(self.font_synthesis)] font_synthesis: Option<FontSynthesis>,
        #[default(self.font_family.clone())] font_family: Option<FontFamily>,
        #[default(self.font_feature_settings.clone())] font_feature_settings: Option<String>,
        #[default(self.letter_spacing)] letter_spacing: TextUnit,
        #[default(self.baseline_shift)] baseline_shift: Option<BaselineShift>,
        #[default(self.text_geometric_transform)] text_geometric_transform: Option<TextGeometricTransform>,
        #[default(self.locale_list.clone())] locale_list: Option<LocaleList>,
        #[default(self.background)] background: Color,
        #[default(self.text_decoration)] text_decoration: Option<TextDecoration>,
        #[default(self.shadow.clone())] shadow: Option<Shadow>,
        #[default(self.draw_style.clone())] draw_style: Option<DrawStyle>,
        #[default(self.text_align)] text_align: TextAlign,
        #[default(self.text_direction)] text_direction: TextDirection,
        #[default(self.line_height)] line_height: TextUnit,
        #[default(self.text_indent)] text_indent: Option<TextIndent>,
        #[default(self.platform_style)] platform_style: Option<PlatformTextStyle>,
        #[default(self.line_height_style)] line_height_style: Option<LineHeightStyle>,
        #[default(self.line_break)] line_break: LineBreak,
        #[default(self.hyphens)] hyphens: Hyphens,
        #[default(self.text_motion)] text_motion: Option<TextMotion>,
    ) -> TextStyle {
        let foreground = copied_foreground(&self.foreground(), color, brush, alpha);
        let mut style = TextStyle {
            font_size,
            font_weight,
            font_style,
            font_synthesis,
            font_family,
            font_feature_settings,
            letter_spacing,
            baseline_shift,
            text_geometric_transform,
            locale_list,
            background,
            text_decoration,
            shadow,
            draw_style,
            text_align,
            text_direction,
            line_height,
            text_indent,
            platform_style,
            line_height_style,
            line_break,
            hyphens,
            text_motion,
            ..TextStyle::Default
        };
        set_foreground(&mut style.color, &mut style.brush, &mut style.alpha, foreground);
        style
    }
}

/// Compose's `TextStyle(color = …, fontSize = …, …)` (or `TextStyle(brush, alpha, …)`): each attribute and setting
/// not given the style's around it.
#[named]
#[allow(clippy::too_many_arguments)]
pub fn TextStyle(
    #[default(Color::Unspecified)] color: Color,
    #[default(None)] brush: Option<Brush>,
    #[default(f32::NAN)] alpha: f32,
    #[default(TextUnit::Unspecified)] font_size: TextUnit,
    #[default(None)] font_weight: Option<FontWeight>,
    #[default(None)] font_style: Option<FontStyle>,
    #[default(None)] font_synthesis: Option<FontSynthesis>,
    #[default(None)] font_family: Option<FontFamily>,
    #[default(None)] font_feature_settings: Option<String>,
    #[default(TextUnit::Unspecified)] letter_spacing: TextUnit,
    #[default(None)] baseline_shift: Option<BaselineShift>,
    #[default(None)] text_geometric_transform: Option<TextGeometricTransform>,
    #[default(None)] locale_list: Option<LocaleList>,
    #[default(Color::Unspecified)] background: Color,
    #[default(None)] text_decoration: Option<TextDecoration>,
    #[default(None)] shadow: Option<Shadow>,
    #[default(None)] draw_style: Option<DrawStyle>,
    #[default(TextAlign::Unspecified)] text_align: TextAlign,
    #[default(TextDirection::Unspecified)] text_direction: TextDirection,
    #[default(TextUnit::Unspecified)] line_height: TextUnit,
    #[default(None)] text_indent: Option<TextIndent>,
    #[default(None)] platform_style: Option<PlatformTextStyle>,
    #[default(None)] line_height_style: Option<LineHeightStyle>,
    #[default(LineBreak::Unspecified)] line_break: LineBreak,
    #[default(Hyphens::Unspecified)] hyphens: Hyphens,
    #[default(None)] text_motion: Option<TextMotion>,
) -> TextStyle {
    TextStyle {
        color,
        brush,
        alpha,
        font_size,
        font_weight,
        font_style,
        font_synthesis,
        font_family,
        font_feature_settings,
        letter_spacing,
        baseline_shift,
        text_geometric_transform,
        locale_list,
        background,
        text_decoration,
        shadow,
        draw_style,
        text_align,
        text_direction,
        line_height,
        text_indent,
        platform_style,
        line_height_style,
        line_break,
        hyphens,
        text_motion,
    }
}

/// Between two text styles (Compose's lerp(TextStyle, TextStyle, fraction)): their span and paragraph styles'.
pub fn lerp_text_style(start: &TextStyle, stop: &TextStyle, fraction: f32) -> TextStyle {
    TextStyle::new(
        &lerp_span_style(&start.to_span_style(), &stop.to_span_style(), fraction),
        &lerp_paragraph_style(&start.to_paragraph_style(), &stop.to_paragraph_style(), fraction),
    )
}
