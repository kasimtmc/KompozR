//! A style for paragraphs (Compose's ParagraphStyle).

use crate::lerp_discrete;
use crate::platform_style::{PlatformParagraphStyle, lerp_platform_paragraph_style};
use crate::style::{
    Hyphens, LineBreak, LineHeightStyle, TextAlign, TextDirection, TextIndent, TextMotion, lerp_text_indent,
    lerp_text_unit_inheritable, resolve_text_direction,
};
use kompoz_graphics::{LayoutDirection, TextUnit};
use kompoz_runtime::named;

/// A style for paragraphs (Compose's ParagraphStyle): what is not given (unspecified, None) is the style's around
/// it. In an AnnotatedString a paragraph style's range is a paragraph of its own (a line break before and after
/// it); nested ones merge, the inner winning, and they may not partly overlap.
#[derive(Clone, PartialEq, Debug, kompoz_runtime::KotlinProperties)]
pub struct ParagraphStyle {
    pub text_align: TextAlign,
    pub text_direction: TextDirection,
    /// From a line's baseline to the next's: sp, or em of the paragraph's font size.
    pub line_height: TextUnit,
    pub text_indent: Option<TextIndent>,
    pub platform_style: Option<PlatformParagraphStyle>,
    pub line_height_style: Option<LineHeightStyle>,
    pub line_break: LineBreak,
    pub hyphens: Hyphens,
    pub text_motion: Option<TextMotion>,
}

impl ParagraphStyle {
    /// Nothing given.
    pub const Default: ParagraphStyle = ParagraphStyle {
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

    /// `other` over this (Compose's merge): each setting `other` gives replaces this one's (platform styles
    /// merged). None: this.
    pub fn merge(&self, other: Option<&ParagraphStyle>) -> ParagraphStyle {
        let Some(o) = other else { return self.clone() };
        ParagraphStyle {
            line_height: o.line_height.take_or_else(|| self.line_height),
            text_indent: o.text_indent.or(self.text_indent),
            text_align: o.text_align.take_or_else(|| self.text_align),
            text_direction: o.text_direction.take_or_else(|| self.text_direction),
            platform_style: match (self.platform_style, o.platform_style) {
                (None, b) => b,
                (a, None) => a,
                (Some(a), Some(b)) => Some(a.merge(Some(&b))),
            },
            line_height_style: o.line_height_style.or(self.line_height_style),
            line_break: o.line_break.take_or_else(|| self.line_break),
            hyphens: o.hyphens.take_or_else(|| self.hyphens),
            text_motion: o.text_motion.or(self.text_motion),
        }
    }

    /// Every setting given: Compose's defaults where none is (resolveParagraphStyleDefaults): start aligned, the
    /// direction resolved in `direction`, the fonts' own line height, no indent, simple line breaking, no
    /// hyphens, static. (The line height style stays as given: none is the fonts' own spacing.)
    pub fn resolve_defaults(&self, direction: LayoutDirection) -> ParagraphStyle {
        ParagraphStyle {
            text_align: self.text_align.take_or_else(|| TextAlign::Start),
            text_direction: resolve_text_direction(direction, self.text_direction),
            line_height: self.line_height,
            text_indent: Some(self.text_indent.unwrap_or(TextIndent::None)),
            platform_style: self.platform_style,
            line_height_style: self.line_height_style,
            line_break: self.line_break.take_or_else(|| LineBreak::Simple),
            hyphens: self.hyphens.take_or_else(|| Hyphens::None),
            text_motion: Some(self.text_motion.unwrap_or(TextMotion::Static)),
        }
    }
}

/// Nothing given.
impl Default for ParagraphStyle {
    fn default() -> ParagraphStyle {
        ParagraphStyle::Default
    }
}

/// `other` over this ([`ParagraphStyle::merge`]: Compose's `plus`).
impl std::ops::Add<&ParagraphStyle> for &ParagraphStyle {
    type Output = ParagraphStyle;
    fn add(self, other: &ParagraphStyle) -> ParagraphStyle {
        self.merge(Some(other))
    }
}

#[named]
impl ParagraphStyle {
    /// The same with what is given replaced (Compose's copy).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.text_align)] text_align: TextAlign,
        #[default(self.text_direction)] text_direction: TextDirection,
        #[default(self.line_height)] line_height: TextUnit,
        #[default(self.text_indent)] text_indent: Option<TextIndent>,
        #[default(self.platform_style)] platform_style: Option<PlatformParagraphStyle>,
        #[default(self.line_height_style)] line_height_style: Option<LineHeightStyle>,
        #[default(self.line_break)] line_break: LineBreak,
        #[default(self.hyphens)] hyphens: Hyphens,
        #[default(self.text_motion)] text_motion: Option<TextMotion>,
    ) -> ParagraphStyle {
        ParagraphStyle {
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
}

/// Compose's `ParagraphStyle(textAlign = …, lineHeight = …, …)`: each setting not given the style's around it.
/// A negative line height panics (as Compose's).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn ParagraphStyle(
    #[default(TextAlign::Unspecified)] text_align: TextAlign,
    #[default(TextDirection::Unspecified)] text_direction: TextDirection,
    #[default(TextUnit::Unspecified)] line_height: TextUnit,
    #[default(None)] text_indent: Option<TextIndent>,
    #[default(None)] platform_style: Option<PlatformParagraphStyle>,
    #[default(None)] line_height_style: Option<LineHeightStyle>,
    #[default(LineBreak::Unspecified)] line_break: LineBreak,
    #[default(Hyphens::Unspecified)] hyphens: Hyphens,
    #[default(None)] text_motion: Option<TextMotion>,
) -> ParagraphStyle {
    assert!(
        line_height.is_unspecified() || line_height.value() >= 0.0,
        "kompoz: lineHeight can't be negative ({})",
        line_height.value()
    );
    ParagraphStyle {
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

/// Between two paragraph styles (Compose's lerp(ParagraphStyle, …)): line heights and indents along, the rest
/// the nearer one's.
pub fn lerp_paragraph_style(start: &ParagraphStyle, stop: &ParagraphStyle, fraction: f32) -> ParagraphStyle {
    ParagraphStyle {
        text_align: lerp_discrete(start.text_align, stop.text_align, fraction),
        text_direction: lerp_discrete(start.text_direction, stop.text_direction, fraction),
        line_height: lerp_text_unit_inheritable(start.line_height, stop.line_height, fraction),
        text_indent: Some(lerp_text_indent(
            &start.text_indent.unwrap_or(TextIndent::None),
            &stop.text_indent.unwrap_or(TextIndent::None),
            fraction,
        )),
        platform_style: if start.platform_style.is_none() && stop.platform_style.is_none() {
            None
        } else {
            Some(lerp_platform_paragraph_style(
                &start.platform_style.unwrap_or_default(),
                &stop.platform_style.unwrap_or_default(),
                fraction,
            ))
        },
        line_height_style: lerp_discrete(start.line_height_style, stop.line_height_style, fraction),
        line_break: lerp_discrete(start.line_break, stop.line_break, fraction),
        hyphens: lerp_discrete(start.hyphens, stop.hyphens, fraction),
        text_motion: lerp_discrete(start.text_motion, stop.text_motion, fraction),
    }
}
