//! Material's Text, as Compose's Text.kt: foundation's `BasicText` in the theme's text style ([`LocalTextStyle`],
//! `body_large` inside a `MaterialTheme`, 14 sp outside one) and the content color around it
//! ([`LocalContentColor`]), what is given replacing the style's; an annotated text's links in the theme's primary
//! color, underlined, where they have no styles of their own. [`ProvideTextStyle`] merges a style into the one
//! around.
//!
//! Compose's overloads by the color's type are two composables here: [`Text`] (a color) and [`TextColorProducer`]
//! (a color read as it draws); each takes a plain or an annotated text (with its inline content).

use crate::content_color::LocalContentColor;
use crate::material_theme::MaterialTheme;
use crate::tokens::DefaultTextStyle;
use kompoz_foundation::text::{BasicText, BasicTextValue, InlineContentMap, OnTextLayout, TextAutoSize};
use kompoz_graphics::{Color, ColorProducer, TextUnit};
use kompoz_runtime::{KompozitionLocal, kompozable, remember_keyed};
use kompoz_text::{
    AnnotatedString, Annotation, FontFamily, FontStyle, FontWeight, LinkAnnotation, Range, SpanStyle, TextAlign,
    TextDecoration, TextLinkStyles, TextOverflow, TextStyle,
};
use kompoz_ui::Modifier;
use std::rc::Rc;

/// The text style text takes where it is (Compose's LocalTextStyle): `MaterialTheme`'s `body_large`, a component's
/// own (a button's label large), Material's default (14 sp, lines centred in their height) where nothing provides
/// one. Its readers are told when another style, not an equal one, is provided.
pub static LocalTextStyle: KompozitionLocal<TextStyle> = KompozitionLocal::new(|| DefaultTextStyle);

/// `value` merged into the text style around (what it gives replacing that one's) for `content` (Compose's
/// `ProvideTextStyle`).
#[kompozable]
pub fn ProvideTextStyle(value: TextStyle, content: impl Fn() + 'static) {
    let merged_style = LocalTextStyle.current().merge(&value);
    kompoz_runtime::KompozitionLocalProvider(&[LocalTextStyle.provides(merged_style)], content);
}

/// Text in the theme's style (Compose's `Text(text, …)`): `text` (a string, or an annotated string whose links
/// without styles of their own take the theme's primary color, underlined, and whose `inline_content` fills its
/// placeholders) in `style` ([`LocalTextStyle`]'s) with what is given replacing that style's: `color` (else the
/// style's, else the content color around, [`LocalContentColor`]), `font_size`, `font_style`, `font_weight`,
/// `font_family`, `letter_spacing`, `text_decoration`, `text_align`, `line_height`. Laid out as foundation's
/// `BasicText`: cut as `overflow` says, wrapping where `soft_wrap` says, in `min_lines` to `max_lines` lines, sized
/// by `auto_size` where given, `on_text_layout` told of each layout.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Text(
    text: impl Into<BasicTextValue>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Color::Unspecified)] color: Color,
    #[default(None)] auto_size: Option<Rc<dyn TextAutoSize>>,
    #[default(TextUnit::Unspecified)] font_size: TextUnit,
    #[default(None)] font_style: Option<FontStyle>,
    #[default(None)] font_weight: Option<FontWeight>,
    #[default(None)] font_family: Option<FontFamily>,
    #[default(TextUnit::Unspecified)] letter_spacing: TextUnit,
    #[default(None)] text_decoration: Option<TextDecoration>,
    #[default(None)] text_align: Option<TextAlign>,
    #[default(TextUnit::Unspecified)] line_height: TextUnit,
    #[default(TextOverflow::Clip)] overflow: TextOverflow,
    #[default(true)] soft_wrap: bool,
    #[default(usize::MAX)] max_lines: usize,
    #[default(1)] min_lines: usize,
    #[default(InlineContentMap::new())] inline_content: InlineContentMap,
    #[default(None)] on_text_layout: Option<OnTextLayout>,
    #[default(LocalTextStyle.current())] style: TextStyle,
) {
    let text_color = color.take_or_else(|| style.color.take_or_else(|| LocalContentColor.current()));
    let text = with_material_link_styles(text);
    let style = merged(
        &style,
        text_color,
        font_size,
        font_weight,
        text_align,
        line_height,
        font_family,
        text_decoration,
        font_style,
        letter_spacing,
    );
    BasicText(
        text,
        modifier = modifier,
        style = style,
        on_text_layout = on_text_layout,
        overflow = overflow,
        soft_wrap = soft_wrap,
        max_lines = max_lines,
        min_lines = min_lines,
        inline_content = inline_content,
        auto_size = auto_size,
    );
}

/// Text in the theme's style whose color `color` gives as it draws (Compose's `Text(text, color: ColorProducer,
/// …)`): an animated color redraws it without composing it again. The rest as [`Text`]'s.
///
/// Kotlin: `Text(text: String, color: ColorProducer)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextColorProducer(
    text: impl Into<BasicTextValue>,
    color: ColorProducer,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] auto_size: Option<Rc<dyn TextAutoSize>>,
    #[default(TextUnit::Unspecified)] font_size: TextUnit,
    #[default(None)] font_style: Option<FontStyle>,
    #[default(None)] font_weight: Option<FontWeight>,
    #[default(None)] font_family: Option<FontFamily>,
    #[default(TextUnit::Unspecified)] letter_spacing: TextUnit,
    #[default(None)] text_decoration: Option<TextDecoration>,
    #[default(None)] text_align: Option<TextAlign>,
    #[default(TextUnit::Unspecified)] line_height: TextUnit,
    #[default(TextOverflow::Clip)] overflow: TextOverflow,
    #[default(true)] soft_wrap: bool,
    #[default(usize::MAX)] max_lines: usize,
    #[default(1)] min_lines: usize,
    #[default(InlineContentMap::new())] inline_content: InlineContentMap,
    #[default(None)] on_text_layout: Option<OnTextLayout>,
    #[default(LocalTextStyle.current())] style: TextStyle,
) {
    let text = with_material_link_styles(text);
    let style = merged(
        &style,
        Color::Unspecified,
        font_size,
        font_weight,
        text_align,
        line_height,
        font_family,
        text_decoration,
        font_style,
        letter_spacing,
    );
    BasicText(
        text,
        modifier = modifier,
        style = style,
        on_text_layout = on_text_layout,
        overflow = overflow,
        soft_wrap = soft_wrap,
        max_lines = max_lines,
        min_lines = min_lines,
        inline_content = inline_content,
        color = Some(color),
        auto_size = auto_size,
    );
}

/// `style` with what is given replacing its own (Compose's `TextStyle.merge(color, fontSize, …)`).
#[allow(clippy::too_many_arguments)]
fn merged(
    style: &TextStyle,
    color: Color,
    font_size: TextUnit,
    font_weight: Option<FontWeight>,
    text_align: Option<TextAlign>,
    line_height: TextUnit,
    font_family: Option<FontFamily>,
    text_decoration: Option<TextDecoration>,
    font_style: Option<FontStyle>,
    letter_spacing: TextUnit,
) -> TextStyle {
    style.merge(&TextStyle {
        color,
        font_size,
        font_weight,
        text_align: text_align.unwrap_or(TextAlign::Unspecified),
        line_height,
        font_family,
        text_decoration,
        font_style,
        letter_spacing,
        ..TextStyle::Default
    })
}

/// An annotated text's links without styles of their own in the theme's primary color, underlined (Compose's
/// `rememberTextLinkStyles` and `createTextWithLinkStyles`); a plain text as it is.
fn with_material_link_styles(text: BasicTextValue) -> BasicTextValue {
    let BasicTextValue::Annotated(text) = text else { return text };
    let primary_color = MaterialTheme::color_scheme().primary;
    let link_styles = remember_keyed(primary_color, || {
        TextLinkStyles::new(
            Some(SpanStyle {
                color: primary_color,
                text_decoration: Some(TextDecoration::Underline),
                ..SpanStyle::Default
            }),
            None,
            None,
            None,
        )
    });
    let styled =
        remember_keyed((text.clone(), link_styles.clone()), || create_text_with_link_styles(&text, &link_styles));
    BasicTextValue::Annotated(styled)
}

fn create_text_with_link_styles(text: &AnnotatedString, link_styles: &TextLinkStyles) -> AnnotatedString {
    text.map_annotations(|range| match &range.item {
        Annotation::Link(link)
            if link.styles().is_none()
                && matches!(**link, LinkAnnotation::Url { .. } | LinkAnnotation::Clickable { .. }) =>
        {
            Range { item: Annotation::Link(Box::new(link.with_styles(Some(link_styles.clone())))), ..range.clone() }
        }
        _ => range.clone(),
    })
}
