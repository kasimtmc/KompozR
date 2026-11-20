//! The theme, as Compose's MaterialTheme.kt: [`MaterialTheme`] gives what is inside it a color scheme, a motion
//! scheme, shapes and a type scale (read as `MaterialTheme::color_scheme()`, …, Kotlin's `MaterialTheme.colorScheme`),
//! the ripple as its indication, the text selection colors of its primary color and `body_large` as the text
//! style; [`MaterialExpressiveTheme`] Material Expressive's (its light scheme and bouncing motion) where no
//! Expressive theme is around. As the C++ Kompoz's (and Compose for desktop's Material theme), it also gives the
//! scrollbars their colors from its scheme's `on_surface` ([`LocalScrollbarStyle`]: 12% at rest, 50% hovered).

use crate::color_scheme::{ColorScheme, expressive_light_color_scheme, light_color_scheme};
use crate::motion_scheme::MotionScheme;
use crate::precision_pointer::EnsurePrecisionPointerListenersRegistered;
use crate::ripples::ripple;
use crate::shapes::Shapes;
use crate::text::ProvideTextStyle;
use crate::typography::Typography;
use kompoz_foundation::indication::LocalIndication;
use kompoz_foundation::scrollbar::{LocalScrollbarStyle, ScrollbarStyle};
use kompoz_foundation::text::selection::{LocalTextSelectionColors, TextSelectionColors};
use kompoz_graphics::Color;
use kompoz_runtime::{KompozitionLocal, KompozitionLocalProvider, kompozable, named_args, remember_keyed};
use std::fmt;
use std::rc::Rc;

/// The theme the components read (Compose's `MaterialTheme(colorScheme, motionScheme, shapes, typography,
/// content)`): `content` composed with `color_scheme`, `motion_scheme`, `shapes` and `typography` (each the theme
/// around's unless given), the bounded [`ripple`] as `LocalIndication`, text selections in the primary color (its
/// handles, 40% of it behind the text), the scrollbars in its `on_surface`, and `typography.body_large` as the text
/// style ([`ProvideTextStyle`]).
#[kompozable]
pub fn MaterialTheme(
    #[default(MaterialTheme::color_scheme())] color_scheme: ColorScheme,
    #[default(MaterialTheme::motion_scheme())] motion_scheme: MotionScheme,
    #[default(MaterialTheme::shapes())] shapes: Shapes,
    #[default(MaterialTheme::typography())] typography: Typography,
    content: impl Fn() + 'static,
) {
    let body_large = typography.body_large.clone();
    let theme = MaterialThemeValues { color_scheme, typography, shapes, motion_scheme };
    let ripple_indication = ripple();
    let selection_colors = remember_text_selection_colors(&color_scheme);
    let scrollbar_style = scrollbar_style(&color_scheme);
    let content = Rc::new(content);
    let values = [
        LocalMaterialTheme.provides(theme),
        LocalIndication.provides_unequal(ripple_indication),
        LocalTextSelectionColors.provides(selection_colors),
        LocalScrollbarStyle.provides(scrollbar_style),
    ];
    KompozitionLocalProvider(&values, move || {
        EnsurePrecisionPointerListenersRegistered(move || {
            ProvideTextStyle(body_large, move || content());
        });
    });
}

/// What `MaterialTheme` reads its theme with (Compose's `object MaterialTheme`): the color scheme, type scale,
/// shapes and motion scheme of the theme around (Material's baseline outside any).
pub struct MaterialTheme {}

#[allow(non_snake_case)]
impl MaterialTheme {
    /// The theme's color scheme (Compose's `MaterialTheme.colorScheme`).
    pub fn color_scheme() -> ColorScheme {
        LocalMaterialTheme.current().color_scheme
    }
    /// The theme's type scale (Compose's `MaterialTheme.typography`).
    pub fn typography() -> Typography {
        LocalMaterialTheme.current().typography
    }
    /// The theme's shapes (Compose's `MaterialTheme.shapes`).
    pub fn shapes() -> Shapes {
        LocalMaterialTheme.current().shapes
    }
    /// The theme's motion scheme (Compose's `MaterialTheme.motionScheme`).
    pub fn motion_scheme() -> MotionScheme {
        LocalMaterialTheme.current().motion_scheme
    }
    /// The local the theme is provided in (Compose's `MaterialTheme.LocalMaterialTheme`).
    pub fn LocalMaterialTheme() -> &'static KompozitionLocal<MaterialThemeValues> {
        &LocalMaterialTheme
    }
    /// The theme's motion scheme as a local of its own (Compose's deprecated `MaterialTheme.LocalMotionScheme`: a
    /// local computed from [`LocalMaterialTheme`](MaterialTheme::LocalMaterialTheme) where it is read).
    #[deprecated(note = "Use [LocalMaterialTheme.current.motionScheme] instead")]
    pub fn LocalMotionScheme() -> &'static KompozitionLocal<MotionScheme> {
        &LocalMotionScheme
    }
}

/// A theme's values (Compose's `MaterialTheme.Values`): equal by its schemes, type scale and shapes.
///
/// Kotlin: `MaterialTheme.Values`
#[derive(Clone, PartialEq)]
pub struct MaterialThemeValues {
    pub color_scheme: ColorScheme,
    pub typography: Typography,
    pub shapes: Shapes,
    pub motion_scheme: MotionScheme,
}

impl MaterialThemeValues {
    pub fn new(
        color_scheme: ColorScheme,
        typography: Typography,
        shapes: Shapes,
        motion_scheme: MotionScheme,
    ) -> MaterialThemeValues {
        MaterialThemeValues { color_scheme, typography, shapes, motion_scheme }
    }
}

impl Default for MaterialThemeValues {
    /// Material's baseline: the light scheme, its type scale and shapes, the standard motion.
    fn default() -> MaterialThemeValues {
        MaterialThemeValues::new(light_color_scheme(), Typography(), Shapes(), MotionScheme::standard())
    }
}

/// Compose's toString.
impl fmt::Display for MaterialThemeValues {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Values(colorScheme={}, typography={}, shapes={}, motionScheme={:?})",
            self.color_scheme, self.typography, self.shapes, self.motion_scheme
        )
    }
}

impl fmt::Debug for MaterialThemeValues {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The theme (Compose's `_localMaterialTheme`, a static local): Material's baseline where no `MaterialTheme` is.
static LocalMaterialTheme: KompozitionLocal<MaterialThemeValues> =
    KompozitionLocal::new_static(MaterialThemeValues::default);

/// Compose's deprecated `MaterialTheme.LocalMotionScheme`: the theme's motion scheme, computed where it is read.
static LocalMotionScheme: KompozitionLocal<MotionScheme> =
    KompozitionLocal::new_computed(|scope| scope.current_value(&LocalMaterialTheme).motion_scheme);

/// Material Expressive's theme (Compose's `MaterialExpressiveTheme`): inside another Expressive theme, a
/// `MaterialTheme` of what is given, the rest the theme around's; else one of what is given, the rest Expressive's
/// (its light scheme, its motion), Material's shapes and type scale — and what is inside knows it is in one
/// ([`LocalUsingExpressiveTheme`]).
#[kompozable]
pub fn MaterialExpressiveTheme(
    #[default(None)] color_scheme: Option<ColorScheme>,
    #[default(None)] motion_scheme: Option<MotionScheme>,
    #[default(None)] shapes: Option<Shapes>,
    #[default(None)] typography: Option<Typography>,
    content: impl Fn() + 'static,
) {
    let content = Rc::new(content);
    if LocalUsingExpressiveTheme.current() {
        MaterialTheme(
            color_scheme = color_scheme.unwrap_or_else(MaterialTheme::color_scheme),
            motion_scheme = motion_scheme.unwrap_or_else(MaterialTheme::motion_scheme),
            typography = typography.unwrap_or_else(MaterialTheme::typography),
            shapes = shapes.unwrap_or_else(MaterialTheme::shapes),
            content = move || content(),
        );
    } else {
        let color_scheme = color_scheme.unwrap_or_else(expressive_light_color_scheme);
        let motion_scheme = motion_scheme.unwrap_or_else(MotionScheme::expressive);
        let shapes = shapes.unwrap_or_else(Shapes);
        let typography = typography.unwrap_or_else(Typography);
        KompozitionLocalProvider(&[LocalUsingExpressiveTheme.provides(true)], move || {
            MaterialTheme(
                color_scheme = color_scheme,
                motion_scheme = motion_scheme,
                shapes = shapes,
                typography = typography,
                content = move || content(),
            );
        });
    }
}

/// Whether an Expressive theme is around (Compose's internal LocalUsingExpressiveTheme): components take its
/// shapes and motion where it is.
pub static LocalUsingExpressiveTheme: KompozitionLocal<bool> = KompozitionLocal::new_static(|| false);

/// The selection colors of a scheme's primary color (Compose's internal `rememberTextSelectionColors`): its
/// handles in it, 40% of it behind the text.
pub(crate) fn remember_text_selection_colors(color_scheme: &ColorScheme) -> TextSelectionColors {
    let primary_color = color_scheme.primary;
    remember_keyed(primary_color, || {
        TextSelectionColors::new(primary_color, named_args!(primary_color.copy(alpha = TextSelectionBackgroundOpacity)))
    })
}

/// How opaque the selection's background is (Compose's internal `TextSelectionBackgroundOpacity`).
pub(crate) const TextSelectionBackgroundOpacity: f32 = 0.4;

/// The scrollbars in a scheme's `on_surface` (the C++ Kompoz's, and Compose for desktop's Material theme): 12% at
/// rest, 50% hovered; the rest as the style around has it.
fn scrollbar_style(color_scheme: &ColorScheme) -> ScrollbarStyle {
    let on_surface: Color = color_scheme.on_surface;
    ScrollbarStyle {
        unhover_color: named_args!(on_surface.copy(alpha = 0.12)),
        hover_color: named_args!(on_surface.copy(alpha = 0.5)),
        ..LocalScrollbarStyle.current()
    }
}
