//! The colors of text selection (Compose's TextSelectionColors.kt and DefaultTextSelectionColors).

use kompoz_graphics::Color;
use kompoz_runtime::{KompozitionLocal, KotlinProperties};
use std::fmt;

/// The colors of text selection (Compose's TextSelectionColors): the handles', and the background behind the
/// selected text (with some transparency, about 40 %, to keep the text legible).
#[derive(Clone, Copy, PartialEq, KotlinProperties)]
pub struct TextSelectionColors {
    pub handle_color: Color,
    pub background_color: Color,
}

/// The blue of Compose's logo (Android's DefaultSelectionColor).
const DefaultSelectionColor: Color = Color::from_argb(0xFF4286F4);

impl TextSelectionColors {
    /// Kotlin: `TextSelectionColors(handleColor: Color, backgroundColor: Color)`
    pub const fn new(handle_color: Color, background_color: Color) -> TextSelectionColors {
        TextSelectionColors { handle_color, background_color }
    }

    /// The colors where none are provided (Compose's DefaultTextSelectionColors on Android): the default
    /// selection color, its background at 0.4 alpha.
    pub fn default_colors() -> TextSelectionColors {
        let c = DefaultSelectionColor;
        TextSelectionColors::new(c, Color::new(c.red(), c.green(), c.blue(), 0.4))
    }
}

impl fmt::Debug for TextSelectionColors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "SelectionColors(selectionHandleColor={:?}, selectionBackgroundColor={:?})",
            self.handle_color, self.background_color
        )
    }
}

/// The selection colors of text and fields below (Compose's LocalTextSelectionColors).
pub static LocalTextSelectionColors: KompozitionLocal<TextSelectionColors> =
    KompozitionLocal::new(TextSelectionColors::default_colors);
