//! A style for a stretch of text (Compose's SpanStyle) and what paints its glyphs (Compose's TextForegroundStyle).

use crate::font::{FontFamily, FontStyle, FontSynthesis, FontWeight, lerp_font_weight};
use crate::intl::LocaleList;
use crate::lerp_discrete;
use crate::platform_style::PlatformSpanStyle;
use crate::style::{
    BaselineShift, TextDecoration, TextGeometricTransform, lerp_baseline_shift, lerp_text_geometric_transform,
    lerp_text_unit_inheritable,
};
use kompoz_graphics::{Brush, Color, DrawStyle, Fill, Shadow, TextUnit, TextUnitType, lerp, lerp_color, lerp_shadow};
use kompoz_runtime::named;
use std::fmt;

/// Compose's default font size: 14 sp.
pub const DefaultFontSize: TextUnit = TextUnit::new(14.0, TextUnitType::Sp);
/// Compose's default letter spacing: none.
pub const DefaultLetterSpacing: TextUnit = TextUnit::new(0.0, TextUnitType::Sp);
/// Compose's default background: none.
pub const DefaultBackgroundColor: Color = Color::Transparent;
/// Compose's default text color: black.
pub const DefaultColor: Color = Color::Black;

/// What paints glyphs (Compose's TextForegroundStyle): a color, a brush (not a solid color: that is a color) at an
/// alpha (NaN: the brush's own), or nothing given.
#[derive(Clone, Debug, Default)]
pub enum TextForegroundStyle {
    #[default]
    Unspecified,
    Color(Color),
    Brush {
        brush: Brush,
        alpha: f32,
    },
}

impl TextForegroundStyle {
    /// A color's (unspecified: none).
    pub fn from_color(color: Color) -> TextForegroundStyle {
        if color.is_specified() { TextForegroundStyle::Color(color) } else { TextForegroundStyle::Unspecified }
    }
    /// A brush's at `alpha` (a solid color: that color, faded by the alpha; none: none).
    pub fn from_brush(brush: Option<&Brush>, alpha: f32) -> TextForegroundStyle {
        match brush {
            None => TextForegroundStyle::Unspecified,
            Some(b) => match b.solid_color() {
                Some(c) => TextForegroundStyle::from_color(modulate(c, alpha)),
                None => TextForegroundStyle::Brush { brush: b.clone(), alpha },
            },
        }
    }
    /// Its color (unspecified for a brush or nothing).
    pub fn color(&self) -> Color {
        match self {
            TextForegroundStyle::Color(c) => *c,
            _ => Color::Unspecified,
        }
    }
    pub fn brush(&self) -> Option<&Brush> {
        match self {
            TextForegroundStyle::Brush { brush, .. } => Some(brush),
            _ => None,
        }
    }
    /// A brush's alpha, a color's own, NaN for nothing.
    pub fn alpha(&self) -> f32 {
        match self {
            TextForegroundStyle::Color(c) => c.alpha(),
            TextForegroundStyle::Brush { alpha, .. } => *alpha,
            TextForegroundStyle::Unspecified => f32::NAN,
        }
    }
    pub fn is_specified(&self) -> bool {
        *self != TextForegroundStyle::Unspecified
    }
    /// `other` over this (Compose's merge): a brush over anything; a color over a color or nothing (a color
    /// doesn't replace a brush); two brushes the later, its alpha the earlier's if it has none.
    pub fn merge(&self, other: &TextForegroundStyle) -> TextForegroundStyle {
        match (self, other) {
            (TextForegroundStyle::Brush { alpha: a, .. }, TextForegroundStyle::Brush { brush, alpha }) => {
                TextForegroundStyle::Brush { brush: brush.clone(), alpha: if alpha.is_nan() { *a } else { *alpha } }
            }
            (_, TextForegroundStyle::Brush { .. }) => other.clone(),
            (TextForegroundStyle::Brush { .. }, _) => self.clone(),
            (_, TextForegroundStyle::Unspecified) => self.clone(),
            _ => other.clone(),
        }
    }
}

/// The same color, or brush at the same alpha (NaN the same as NaN, as Compose's data classes compare).
impl PartialEq for TextForegroundStyle {
    fn eq(&self, o: &TextForegroundStyle) -> bool {
        match (self, o) {
            (TextForegroundStyle::Unspecified, TextForegroundStyle::Unspecified) => true,
            (TextForegroundStyle::Color(a), TextForegroundStyle::Color(b)) => a == b,
            (TextForegroundStyle::Brush { brush: a, alpha: x }, TextForegroundStyle::Brush { brush: b, alpha: y }) => {
                a == b && (x == y || x.is_nan() && y.is_nan())
            }
            _ => false,
        }
    }
}

/// A color with its alpha times `alpha` (NaN or 1 and up: as it is).
pub(crate) fn modulate(color: Color, alpha: f32) -> Color {
    if alpha.is_nan() || alpha >= 1.0 {
        color
    } else {
        Color::new_in(color.red(), color.green(), color.blue(), color.alpha() * alpha, color.color_space())
    }
}

/// Between two (Compose's lerp(TextForegroundStyle, …)): colors along, brushes the nearer one at an alpha along,
/// a color and a brush the nearer one.
pub fn lerp_text_foreground_style(
    start: &TextForegroundStyle,
    stop: &TextForegroundStyle,
    fraction: f32,
) -> TextForegroundStyle {
    match (start, stop) {
        (TextForegroundStyle::Brush { brush: b1, alpha: a1 }, TextForegroundStyle::Brush { brush: b2, alpha: a2 }) => {
            TextForegroundStyle::from_brush(Some(lerp_discrete(b1, b2, fraction)), lerp(*a1, *a2, fraction))
        }
        (TextForegroundStyle::Brush { .. }, _) | (_, TextForegroundStyle::Brush { .. }) => {
            lerp_discrete(start, stop, fraction).clone()
        }
        _ => TextForegroundStyle::from_color(lerp_color(start.color(), stop.color(), fraction)),
    }
}

/// A style for a stretch of text (Compose's SpanStyle): what is not given (None, unspecified) is the style's
/// around it. What paints it is `color`, or `brush` at `alpha` (a brush wins over a color; a solid color brush is
/// that color; `alpha` NaN is the brush's own) — [`foreground`](SpanStyle::foreground) is what they come to, and
/// what styles compare by.
#[derive(Clone)]
pub struct SpanStyle {
    pub color: Color,
    pub brush: Option<Brush>,
    pub alpha: f32,
    pub font_size: TextUnit,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub font_synthesis: Option<FontSynthesis>,
    pub font_family: Option<FontFamily>,
    /// OpenType features, as CSS writes them (`"tnum, smcp"`, `"\"liga\" 0"`), read by
    /// [`crate::font::parse_font_feature_settings`].
    pub font_feature_settings: Option<String>,
    /// Room added around each character (half before, half after; none in cursive scripts).
    pub letter_spacing: TextUnit,
    pub baseline_shift: Option<BaselineShift>,
    pub text_geometric_transform: Option<TextGeometricTransform>,
    pub locale_list: Option<LocaleList>,
    pub background: Color,
    pub text_decoration: Option<TextDecoration>,
    pub shadow: Option<Shadow>,
    pub platform_style: Option<PlatformSpanStyle>,
    /// `Stroke`: its glyphs' outlines drawn instead of filled.
    pub draw_style: Option<DrawStyle>,
}

impl SpanStyle {
    /// Nothing given: every attribute the style's around it.
    pub const Default: SpanStyle = SpanStyle {
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
        platform_style: None,
        draw_style: None,
    };

    /// What paints it (Compose's textForegroundStyle).
    pub fn foreground(&self) -> TextForegroundStyle {
        if self.brush.is_some() {
            TextForegroundStyle::from_brush(self.brush.as_ref(), self.alpha)
        } else {
            TextForegroundStyle::from_color(self.color)
        }
    }

    /// The same painted by `foreground` (its color, brush and alpha fields set as it says).
    pub fn with_foreground(mut self, foreground: TextForegroundStyle) -> SpanStyle {
        set_foreground(&mut self.color, &mut self.brush, &mut self.alpha, foreground);
        self
    }

    /// `other` over this (Compose's merge): each attribute `other` gives replaces this one's (the foreground
    /// as [`TextForegroundStyle::merge`], platform styles merged). None: this.
    pub fn merge(&self, other: Option<&SpanStyle>) -> SpanStyle {
        let Some(o) = other else { return self.clone() };
        SpanStyle {
            font_family: o.font_family.clone().or_else(|| self.font_family.clone()),
            font_size: o.font_size.take_or_else(|| self.font_size),
            font_weight: o.font_weight.or(self.font_weight),
            font_style: o.font_style.or(self.font_style),
            font_synthesis: o.font_synthesis.or(self.font_synthesis),
            font_feature_settings: o.font_feature_settings.clone().or_else(|| self.font_feature_settings.clone()),
            letter_spacing: o.letter_spacing.take_or_else(|| self.letter_spacing),
            baseline_shift: o.baseline_shift.or(self.baseline_shift),
            text_geometric_transform: o.text_geometric_transform.or(self.text_geometric_transform),
            locale_list: o.locale_list.clone().or_else(|| self.locale_list.clone()),
            background: o.background.take_or_else(|| self.background),
            text_decoration: o.text_decoration.or(self.text_decoration),
            shadow: o.shadow.clone().or_else(|| self.shadow.clone()),
            platform_style: merge_platform_span_style(self.platform_style, o.platform_style),
            draw_style: o.draw_style.clone().or_else(|| self.draw_style.clone()),
            ..SpanStyle::Default
        }
        .with_foreground(self.foreground().merge(&o.foreground()))
    }

    /// Whether the two lay text out alike (Compose's hasSameLayoutAffectingAttributes): all but what only paints.
    pub fn has_same_layout_affecting_attributes(&self, o: &SpanStyle) -> bool {
        self.font_size == o.font_size
            && self.font_weight == o.font_weight
            && self.font_style == o.font_style
            && self.font_synthesis == o.font_synthesis
            && self.font_family == o.font_family
            && self.font_feature_settings == o.font_feature_settings
            && self.letter_spacing == o.letter_spacing
            && self.baseline_shift == o.baseline_shift
            && self.text_geometric_transform == o.text_geometric_transform
            && self.locale_list == o.locale_list
            && self.background == o.background
            && self.platform_style == o.platform_style
    }

    /// Whether the two paint alike (Compose's hasSameNonLayoutAttributes): foreground, decoration, shadow, draw
    /// style.
    pub fn has_same_non_layout_attributes(&self, o: &SpanStyle) -> bool {
        self.foreground() == o.foreground()
            && self.text_decoration == o.text_decoration
            && self.shadow == o.shadow
            && self.draw_style == o.draw_style
    }

    /// Every attribute given: Compose's defaults where none is (resolveSpanStyleDefaults): black, 14 sp, regular,
    /// upright, all synthesis, the default family, no features, no letter spacing, no shift or transform, the
    /// current locales, no background, decoration or shadow, filled.
    pub fn resolve_defaults(&self) -> SpanStyle {
        let foreground = self.foreground();
        SpanStyle {
            font_size: self.font_size.take_or_else(|| DefaultFontSize),
            font_weight: Some(self.font_weight.unwrap_or(FontWeight::Normal)),
            font_style: Some(self.font_style.unwrap_or(FontStyle::Normal)),
            font_synthesis: Some(self.font_synthesis.unwrap_or(FontSynthesis::All)),
            font_family: Some(self.font_family.clone().unwrap_or(FontFamily::Default)),
            font_feature_settings: Some(self.font_feature_settings.clone().unwrap_or_default()),
            letter_spacing: self.letter_spacing.take_or_else(|| DefaultLetterSpacing),
            baseline_shift: Some(
                self.baseline_shift.unwrap_or(BaselineShift::None).take_or_else(|| BaselineShift::None),
            ),
            text_geometric_transform: Some(self.text_geometric_transform.unwrap_or(TextGeometricTransform::None)),
            locale_list: Some(self.locale_list.clone().unwrap_or_else(LocaleList::current)),
            background: self.background.take_or_else(|| DefaultBackgroundColor),
            text_decoration: Some(self.text_decoration.unwrap_or(TextDecoration::None)),
            shadow: Some(self.shadow.clone().unwrap_or(Shadow::None)),
            platform_style: self.platform_style,
            draw_style: Some(self.draw_style.clone().unwrap_or(Fill)),
            ..SpanStyle::Default
        }
        .with_foreground(if foreground.is_specified() {
            foreground
        } else {
            TextForegroundStyle::Color(DefaultColor)
        })
    }
}

/// Sets a style's color, brush and alpha fields as a foreground says.
pub(crate) fn set_foreground(color: &mut Color, brush: &mut Option<Brush>, alpha: &mut f32, f: TextForegroundStyle) {
    (*color, *brush, *alpha) = match f {
        TextForegroundStyle::Unspecified => (Color::Unspecified, None, f32::NAN),
        TextForegroundStyle::Color(c) => (c, None, f32::NAN),
        TextForegroundStyle::Brush { brush, alpha } => (Color::Unspecified, Some(brush), alpha),
    };
}

fn merge_platform_span_style(a: Option<PlatformSpanStyle>, b: Option<PlatformSpanStyle>) -> Option<PlatformSpanStyle> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some(a.merge(Some(&b))),
    }
}

/// Nothing given.
impl Default for SpanStyle {
    fn default() -> SpanStyle {
        SpanStyle::Default
    }
}

/// Equal in what it lays out and paints (the foreground as it comes to).
impl PartialEq for SpanStyle {
    fn eq(&self, o: &SpanStyle) -> bool {
        self.has_same_layout_affecting_attributes(o) && self.has_same_non_layout_attributes(o)
    }
}

/// The attributes given, as Compose writes them.
impl fmt::Debug for SpanStyle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let fg = self.foreground();
        f.debug_struct("SpanStyle")
            .field("color", &fg.color())
            .field("brush", &fg.brush())
            .field("alpha", &fg.alpha())
            .field("font_size", &self.font_size)
            .field("font_weight", &self.font_weight)
            .field("font_style", &self.font_style)
            .field("font_synthesis", &self.font_synthesis)
            .field("font_family", &self.font_family)
            .field("font_feature_settings", &self.font_feature_settings)
            .field("letter_spacing", &self.letter_spacing)
            .field("baseline_shift", &self.baseline_shift)
            .field("text_geometric_transform", &self.text_geometric_transform)
            .field("locale_list", &self.locale_list)
            .field("background", &self.background)
            .field("text_decoration", &self.text_decoration)
            .field("shadow", &self.shadow)
            .field("platform_style", &self.platform_style)
            .field("draw_style", &self.draw_style)
            .finish()
    }
}

/// `other` over this ([`SpanStyle::merge`]: Compose's `plus`).
impl std::ops::Add<&SpanStyle> for &SpanStyle {
    type Output = SpanStyle;
    fn add(self, other: &SpanStyle) -> SpanStyle {
        self.merge(Some(other))
    }
}

#[named]
impl SpanStyle {
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
        #[default(self.platform_style)] platform_style: Option<PlatformSpanStyle>,
        #[default(self.draw_style.clone())] draw_style: Option<DrawStyle>,
    ) -> SpanStyle {
        let foreground = copied_foreground(&self.foreground(), color, brush, alpha);
        SpanStyle {
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
            platform_style,
            draw_style,
            ..SpanStyle::Default
        }
        .with_foreground(foreground)
    }
}

/// What a copy is painted by: a brush other than its own (or its own at another alpha), else a color other
/// than its own, else its own.
pub(crate) fn copied_foreground(
    own: &TextForegroundStyle,
    color: Color,
    brush: Option<Brush>,
    alpha: f32,
) -> TextForegroundStyle {
    let same_alpha = alpha == own.alpha() || alpha.is_nan() && own.alpha().is_nan();
    if brush.as_ref() != own.brush() || brush.is_some() && !same_alpha {
        TextForegroundStyle::from_brush(brush.as_ref(), alpha)
    } else if color != own.color() {
        TextForegroundStyle::from_color(color)
    } else {
        own.clone()
    }
}

/// Compose's `SpanStyle(color = …, fontSize = …, …)` (or `SpanStyle(brush, alpha, …)`): each attribute not given
/// the style's around it.
#[named]
#[allow(clippy::too_many_arguments)]
pub fn SpanStyle(
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
    #[default(None)] platform_style: Option<PlatformSpanStyle>,
    #[default(None)] draw_style: Option<DrawStyle>,
) -> SpanStyle {
    SpanStyle {
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
        platform_style,
        draw_style,
    }
}

/// Between two span styles (Compose's lerp(SpanStyle, SpanStyle, fraction)): sizes, weights, shifts, transforms,
/// colors and shadows along; the rest the nearer one's.
pub fn lerp_span_style(start: &SpanStyle, stop: &SpanStyle, fraction: f32) -> SpanStyle {
    SpanStyle {
        font_family: lerp_discrete(&start.font_family, &stop.font_family, fraction).clone(),
        font_size: lerp_text_unit_inheritable(start.font_size, stop.font_size, fraction),
        font_weight: Some(lerp_font_weight(
            start.font_weight.unwrap_or(FontWeight::Normal),
            stop.font_weight.unwrap_or(FontWeight::Normal),
            fraction,
        )),
        font_style: lerp_discrete(start.font_style, stop.font_style, fraction),
        font_synthesis: lerp_discrete(start.font_synthesis, stop.font_synthesis, fraction),
        font_feature_settings: lerp_discrete(&start.font_feature_settings, &stop.font_feature_settings, fraction)
            .clone(),
        letter_spacing: lerp_text_unit_inheritable(start.letter_spacing, stop.letter_spacing, fraction),
        baseline_shift: Some(lerp_baseline_shift(
            start.baseline_shift.unwrap_or(BaselineShift(0.0)),
            stop.baseline_shift.unwrap_or(BaselineShift(0.0)),
            fraction,
        )),
        text_geometric_transform: Some(lerp_text_geometric_transform(
            &start.text_geometric_transform.unwrap_or(TextGeometricTransform::None),
            &stop.text_geometric_transform.unwrap_or(TextGeometricTransform::None),
            fraction,
        )),
        locale_list: lerp_discrete(&start.locale_list, &stop.locale_list, fraction).clone(),
        background: lerp_color(start.background, stop.background, fraction),
        text_decoration: lerp_discrete(start.text_decoration, stop.text_decoration, fraction),
        shadow: lerp_nullable_shadow(start.shadow.as_ref(), stop.shadow.as_ref(), fraction),
        platform_style: if start.platform_style.is_none() && stop.platform_style.is_none() {
            None
        } else {
            Some(start.platform_style.unwrap_or_default())
        },
        draw_style: lerp_discrete(&start.draw_style, &stop.draw_style, fraction).clone(),
        ..SpanStyle::Default
    }
    .with_foreground(lerp_text_foreground_style(&start.foreground(), &stop.foreground(), fraction))
}

/// Between two shadows either of which may be missing: from or to the other faded out (Compose's nullSafeLerp).
fn lerp_nullable_shadow(a: Option<&Shadow>, b: Option<&Shadow>, fraction: f32) -> Option<Shadow> {
    let faded = |s: &Shadow| Shadow { color: modulate(s.color, 0.0), ..s.clone() };
    match (a, b) {
        (None, None) => None,
        (None, Some(b)) => Some(lerp_shadow(&faded(b), b, fraction)),
        (Some(a), None) => Some(lerp_shadow(a, &faded(a), fraction)),
        (Some(a), Some(b)) => Some(lerp_shadow(a, b, fraction)),
    }
}
