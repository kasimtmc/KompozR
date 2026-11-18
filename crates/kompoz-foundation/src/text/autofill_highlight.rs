//! What an autofilled field is highlighted with (Compose's AutofillHighlight.kt and its desktop actual): drawn over
//! the field once an autofill service filled it, until its text changes.

use kompoz_graphics::{Brush, Color, SolidColor};
use kompoz_runtime::KompozitionLocal;

/// The color an autofilled field is highlighted with by default (Compose's `autofillHighlightColor`, the
/// desktop's: none).
pub(crate) fn autofill_highlight_color() -> Color {
    Color::Unspecified
}

/// What autofilled fields below are highlighted with (Compose's LocalAutofillHighlightBrush): by default the
/// platform's color.
pub static LocalAutofillHighlightBrush: KompozitionLocal<Brush> =
    KompozitionLocal::new(|| SolidColor(autofill_highlight_color()));

/// The color autofilled fields below are highlighted with (Compose's deprecated LocalAutofillHighlightColor): a
/// color other than the platform's wins over the brush.
pub static LocalAutofillHighlightColor: KompozitionLocal<Color> = KompozitionLocal::new(autofill_highlight_color);

/// The highlight: `color` if it was provided (another than `default_color`), else `brush` (Compose's
/// `resolveAutofillHighlight`).
pub(crate) fn resolve_autofill_highlight(brush: Brush, color: Color, default_color: Color) -> Brush {
    let overridden = !(color == default_color || color.is_unspecified() && default_color.is_unspecified());
    if overridden { SolidColor(color) } else { brush }
}
