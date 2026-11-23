//! Where a line height puts the room it adds to a line or takes from it (Compose's LineHeightStyle and its
//! LineHeightStyleAlignment, LineHeightStyleTrim and LineHeightStyleMode).

/// Where a paragraph's line height (`line_height`) puts the room it adds to a line or takes from it, whether the
/// text's first line gets its share above and its last line below, and whether a line may be taller than the
/// line height (Compose's LineHeightStyle).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LineHeightStyle {
    pub alignment: LineHeightStyleAlignment,
    pub trim: LineHeightStyleTrim,
    pub mode: LineHeightStyleMode,
}

impl LineHeightStyle {
    /// Proportional, both ends trimmed, fixed (Compose's LineHeightStyle.Default).
    pub const Default: LineHeightStyle = LineHeightStyle::new(
        LineHeightStyleAlignment::Proportional,
        LineHeightStyleTrim::Both,
        LineHeightStyleMode::Fixed,
    );

    pub const fn new(
        alignment: LineHeightStyleAlignment,
        trim: LineHeightStyleTrim,
        mode: LineHeightStyleMode,
    ) -> LineHeightStyle {
        LineHeightStyle { alignment, trim, mode }
    }
}

/// [`LineHeightStyle::Default`].
impl Default for LineHeightStyle {
    fn default() -> LineHeightStyle {
        LineHeightStyle::Default
    }
}

/// Compose's `LineHeightStyle(alignment, trim, mode = LineHeightStyleMode.Fixed)`.
#[kompoz_runtime::named(type)]
pub fn LineHeightStyle(
    alignment: LineHeightStyleAlignment,
    trim: LineHeightStyleTrim,
    #[default(LineHeightStyleMode::Fixed)] mode: LineHeightStyleMode,
) -> LineHeightStyle {
    LineHeightStyle::new(alignment, trim, mode)
}

/// The share of the room a line height adds that goes above the line (Compose's LineHeightStyle.Alignment):
/// `Top` none (all below), `Center` half, `Bottom` all; `Proportional` as the line's ascent and descent stand.
///
/// Kotlin: `LineHeightStyle.Alignment`
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LineHeightStyleAlignment {
    top_ratio: f32,
}

impl LineHeightStyleAlignment {
    pub const Top: LineHeightStyleAlignment = LineHeightStyleAlignment { top_ratio: 0.0 };
    pub const Center: LineHeightStyleAlignment = LineHeightStyleAlignment { top_ratio: 0.5 };
    pub const Proportional: LineHeightStyleAlignment = LineHeightStyleAlignment { top_ratio: -1.0 };
    pub const Bottom: LineHeightStyleAlignment = LineHeightStyleAlignment { top_ratio: 1.0 };

    /// A share of its own, 0 to 1 (or -1: proportional).
    pub fn new(top_ratio: f32) -> LineHeightStyleAlignment {
        assert!(
            (0.0..=1.0).contains(&top_ratio) || top_ratio == -1.0,
            "kompoz: a line height alignment's share is 0 to 1, or -1 (proportional), not {top_ratio}"
        );
        LineHeightStyleAlignment { top_ratio }
    }
    /// The share above (-1: proportional).
    pub fn top_ratio(self) -> f32 {
        self.top_ratio
    }
}

/// Whether the text's first line gets no room above and its last none below (Compose's LineHeightStyle.Trim).
///
/// Kotlin: `LineHeightStyle.Trim`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LineHeightStyleTrim {
    value: u8,
}

impl LineHeightStyleTrim {
    const TOP: u8 = 0x01;
    const BOTTOM: u8 = 0x10;

    /// Nothing above the first line.
    pub const FirstLineTop: LineHeightStyleTrim = LineHeightStyleTrim { value: LineHeightStyleTrim::TOP };
    /// Nothing below the last line.
    pub const LastLineBottom: LineHeightStyleTrim = LineHeightStyleTrim { value: LineHeightStyleTrim::BOTTOM };
    pub const Both: LineHeightStyleTrim =
        LineHeightStyleTrim { value: LineHeightStyleTrim::TOP | LineHeightStyleTrim::BOTTOM };
    pub const None: LineHeightStyleTrim = LineHeightStyleTrim { value: 0 };

    /// Whether the first line's top is trimmed.
    pub fn is_trim_first_line_top(self) -> bool {
        self.value & LineHeightStyleTrim::TOP != 0
    }
    /// Whether the last line's bottom is trimmed.
    pub fn is_trim_last_line_bottom(self) -> bool {
        self.value & LineHeightStyleTrim::BOTTOM != 0
    }
}

/// How strictly lines keep the line height (Compose's LineHeightStyle.Mode).
///
/// Kotlin: `LineHeightStyle.Mode`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LineHeightStyleMode {
    /// Every line the line height, the text's top and bottom padded where its fonts want more (so tall glyphs of
    /// the first and last lines aren't cut; middle lines' may reach into their neighbours).
    Fixed,
    /// Lines never shorter than their fonts make them, the text's top and bottom padded likewise.
    Minimum,
    /// Every line the line height, the first and last too: no padding (tall glyphs may be cut where trimmed).
    Tight,
}

/// [`LineHeightStyleAlignment`] as this module names it (Compose's nested `LineHeightStyle.Alignment`).
pub type Alignment = LineHeightStyleAlignment;
/// [`LineHeightStyleTrim`] as this module names it (Compose's nested `LineHeightStyle.Trim`).
pub type Trim = LineHeightStyleTrim;
/// [`LineHeightStyleMode`] as this module names it (Compose's nested `LineHeightStyle.Mode`).
pub type Mode = LineHeightStyleMode;
