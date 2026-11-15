//! The text nodes and what they lay out with (Compose's `foundation.text.modifiers`): a plain string's node
//! ([`text_string_simple`]), an annotated string's ([`text_annotated_string`]), the one of a text in a selection
//! container ([`selectable_text_annotated_string`]) and its bridge to the selection ([`selection_hook`]), the
//! layout caches, min lines and the constraints a paragraph is given.

pub(crate) mod layout_utils;
pub(crate) mod min_lines_constrainer;
pub(crate) mod multi_paragraph_layout_cache;
pub(crate) mod paragraph_layout_cache;
pub(crate) mod selectable_text_annotated_string;
pub mod selection_hook;
pub(crate) mod text_annotated_string;
pub(crate) mod text_string_simple;

use kompoz_graphics::{Color, ColorProducer, Fill, Shadow};
use kompoz_text::{SpanStyle, TextDecoration, TextForegroundStyle, TextPaintStyle, TextStyle};
use kompoz_ui::NodeCx;

/// What a text node paints with (Compose's text nodes' draw): its style's brush at its alpha, or else the override
/// color, the style's, or black; its style's shadow, decoration and draw style (none: none, fill). The override
/// color is read now (a draw read: a change redraws).
pub(crate) fn text_paint_style(style: &TextStyle, override_color: Option<&ColorProducer>) -> TextPaintStyle {
    let foreground = match &style.brush {
        Some(brush) => TextForegroundStyle::from_brush(Some(brush), style.alpha),
        None => TextForegroundStyle::from_color(resolved_text_color(style, override_color)),
    };
    TextPaintStyle {
        foreground,
        shadow: Some(style.shadow.clone().unwrap_or(Shadow::None)),
        decoration: Some(style.text_decoration.unwrap_or(TextDecoration::None)),
        draw_style: Some(style.draw_style.clone().unwrap_or(Fill)),
    }
}

/// The color text is painted in: the override's where it gives one, else the style's, else black.
pub(crate) fn resolved_text_color(style: &TextStyle, override_color: Option<&ColorProducer>) -> Color {
    let over = override_color.map_or(Color::Unspecified, ColorProducer::get);
    if over.is_specified() {
        over
    } else if style.color.is_specified() {
        style.color
    } else {
        Color::Black
    }
}

/// `style` in `color` where it is specified (Compose's `style.merge(color = …)`).
pub(crate) fn with_color(style: &TextStyle, color: Color) -> TextStyle {
    if color.is_specified() { style.merge(&SpanStyle { color, ..SpanStyle::Default }) } else { style.clone() }
}

/// What a change of its text substitution invalidates (Compose's invalidateForTranslate): its semantics,
/// measurement and drawing.
pub(crate) fn invalidate_for_translate(cx: &NodeCx) {
    cx.invalidate_semantics();
    cx.invalidate_measurement();
    cx.invalidate_draw();
}
