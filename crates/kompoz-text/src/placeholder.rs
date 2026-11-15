//! The room inline content takes in a text (Compose's Placeholder and PlaceholderVerticalAlign).

use kompoz_graphics::TextUnit;

/// Where a placeholder sits on its line (Compose's PlaceholderVerticalAlign): its bottom on the baseline; its top,
/// bottom or middle at the line's; or at the text's (the fonts' ascent and descent) top, bottom or middle.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PlaceholderVerticalAlign {
    AboveBaseline,
    Top,
    Bottom,
    Center,
    TextTop,
    TextBottom,
    TextCenter,
}

/// The room inline content takes in a text (Compose's Placeholder): its width and height in sp, or em of the
/// text's font size there, and where it sits on its line.
#[derive(Clone, Copy, PartialEq, Hash, Debug)]
pub struct Placeholder {
    pub width: TextUnit,
    pub height: TextUnit,
    pub placeholder_vertical_align: PlaceholderVerticalAlign,
}

impl Placeholder {
    /// An unspecified width or height panics (as Compose's).
    pub fn new(width: TextUnit, height: TextUnit, placeholder_vertical_align: PlaceholderVerticalAlign) -> Placeholder {
        assert!(width.is_specified(), "kompoz: width cannot be TextUnit.Unspecified");
        assert!(height.is_specified(), "kompoz: height cannot be TextUnit.Unspecified");
        Placeholder { width, height, placeholder_vertical_align }
    }
}
