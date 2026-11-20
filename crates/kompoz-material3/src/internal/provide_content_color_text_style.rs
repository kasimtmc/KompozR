//! A content color and a text style at once, as Compose's internal ProvideContentColorTextStyle.kt: what a
//! component gives its slots (a button's label, a list item's headline) — [`LocalContentColor`], and its text style
//! merged into the one around ([`LocalTextStyle`]) — in one provider.

use crate::content_color::LocalContentColor;
use crate::text::LocalTextStyle;
use kompoz_graphics::Color;
use kompoz_runtime::{KompozitionLocalProvider, ProvidedValue, kompozable};
use kompoz_text::TextStyle;

/// `content` in `content_color`, its text in `text_style` merged into the style around (Compose's internal
/// `ProvideContentColorTextStyle(contentColor, textStyle, content)`).
#[kompozable]
pub(crate) fn ProvideContentColorTextStyle(content_color: Color, text_style: TextStyle, content: impl Fn() + 'static) {
    let merged_style = LocalTextStyle.current().merge(&text_style);
    KompozitionLocalProvider(
        &[LocalContentColor.provides(content_color), LocalTextStyle.provides(merged_style)],
        content,
    );
}

/// The same, `other_values` provided with them (Compose's internal `ProvideContentColorTextStyle(contentColor,
/// textStyle, vararg otherValues, content)`).
///
/// Kotlin: `ProvideContentColorTextStyle(contentColor: Color, textStyle: TextStyle, vararg otherValues: ProvidedValue<*>, content: @Composable () -> Unit)`
#[kompozable]
pub(crate) fn ProvideContentColorTextStyleWithValues(
    content_color: Color,
    text_style: TextStyle,
    other_values: Vec<ProvidedValue>,
    content: impl Fn() + 'static,
) {
    let merged_style = LocalTextStyle.current().merge(&text_style);
    let mut values = vec![LocalContentColor.provides(content_color), LocalTextStyle.provides(merged_style)];
    values.extend(other_values);
    KompozitionLocalProvider(&values, content);
}
