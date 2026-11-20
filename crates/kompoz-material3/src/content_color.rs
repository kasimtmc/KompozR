//! The content color, as Compose's ContentColor.kt: what text and icons are drawn in where they are, the `on`
//! color of the surface around them.

use kompoz_graphics::Color;
use kompoz_runtime::KompozitionLocal;

/// The color text and icons take where they are (Compose's LocalContentColor): a surface's content color (the
/// `on` color of its own, [`content_color_for`](crate::content_color_for)), black where nothing provides one.
pub static LocalContentColor: KompozitionLocal<Color> = KompozitionLocal::new(|| Color::Black);
