//! A test `Text`: foundation's [`BasicText`] in a font size (Material's Text, which takes its style from a theme,
//! is a later stage's), measured by the host's font family resolver (the test rule's is the
//! [`FakeBackend`](kompoz_text::FakeBackend)'s: C++'s FakeTextMeasurer's numbers).

use kompoz_foundation::text::BasicText;
use kompoz_graphics::{Color, TextUnit, UnitExt};
use kompoz_runtime::kompozable;
use kompoz_text::TextStyle;
use kompoz_ui::Modifier;

/// `text` as a [`BasicText`] of `font_size` (Compose's 14 sp by default; the C++ Kompoz's tests, ported, say their
/// 16 sp) in `color` (Material's `color` parameter: unspecified, the text style's): wrapping in the width it is
/// given, as wide as its widest line within the constraints, its baselines given, its `Text` semantics and its
/// layout (`GetTextLayoutResult`) handed out, drawn as its layout's runs.
#[kompozable]
pub fn Text(
    text: impl Into<String>,
    #[default(14.sp())] font_size: TextUnit,
    #[default(Modifier)] modifier: Modifier,
    #[default(Color::Unspecified)] color: Color,
) {
    let style = TextStyle { font_size, ..TextStyle::Default };
    let style = if color.is_specified() { TextStyle { color, ..style } } else { style };
    BasicText(text, modifier = modifier, style = style);
}
