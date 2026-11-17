//! How many lines a field shows and how it scrolls (Compose's TextFieldLineLimits).

/// A field's line limits (Compose's TextFieldLineLimits): one line, scrolling horizontally (Enter doesn't break
/// it: it performs the keyboard action), or several, between a least and most height in lines, scrolling
/// vertically.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextFieldLineLimits {
    /// One line (Compose's `TextFieldLineLimits.SingleLine`).
    SingleLine,
    /// Wrapping lines, at least `min_height_in_lines` tall and at most `max_height_in_lines` (Compose's `MultiLine`;
    /// make it with [`TextFieldLineLimits::multi_line`]).
    MultiLine { min_height_in_lines: i32, max_height_in_lines: i32 },
}

impl TextFieldLineLimits {
    /// Compose's default: wrapping lines, at least one, no most.
    pub const Default: TextFieldLineLimits =
        TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: i32::MAX };

    /// Wrapping lines between `min_height_in_lines` and `max_height_in_lines` (Compose's `MultiLine(…)`). Panics
    /// unless 1 ≤ min ≤ max.
    ///
    /// Kotlin: `TextFieldLineLimits.MultiLine(minHeightInLines: Int = 1, maxHeightInLines: Int = Int.MAX_VALUE)`
    pub fn multi_line(min_height_in_lines: i32, max_height_in_lines: i32) -> TextFieldLineLimits {
        assert!(
            1 <= min_height_in_lines && min_height_in_lines <= max_height_in_lines,
            "Expected 1 ≤ minHeightInLines ≤ maxHeightInLines, were {min_height_in_lines}, {max_height_in_lines}"
        );
        TextFieldLineLimits::MultiLine { min_height_in_lines, max_height_in_lines }
    }

    /// Whether it is one line.
    pub fn is_single_line(&self) -> bool {
        matches!(self, TextFieldLineLimits::SingleLine)
    }
}

impl Default for TextFieldLineLimits {
    fn default() -> Self {
        TextFieldLineLimits::Default
    }
}

/// `TextFieldLineLimits.SingleLine`, `MultiLine(minHeightInLines=…, maxHeightInLines=…)`, as Compose's.
impl std::fmt::Display for TextFieldLineLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TextFieldLineLimits::SingleLine => f.write_str("TextFieldLineLimits.SingleLine"),
            TextFieldLineLimits::MultiLine { min_height_in_lines, max_height_in_lines } => {
                write!(f, "MultiLine(minHeightInLines={min_height_in_lines}, maxHeightInLines={max_height_in_lines})")
            }
        }
    }
}
