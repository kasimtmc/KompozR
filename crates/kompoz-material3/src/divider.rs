//! Material's dividers, as Compose's Divider.kt: a thin line across ([`HorizontalDivider`]) or down
//! ([`VerticalDivider`]) in the scheme's outline variant, 1 dp thick.

// (the deprecated `Divider`'s own generated items name it)
#![allow(deprecated)]

use crate::tokens::DividerTokens;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Offset};
use kompoz_runtime::{kompozable, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::platform::LocalDensity;

/// A line across the width it is given (Compose's `HorizontalDivider`), `thickness` tall, in `color`.
#[kompozable]
pub fn HorizontalDivider(
    #[default(Modifier)] modifier: Modifier,
    #[default(DividerDefaults::Thickness)] thickness: Dp,
    #[default(DividerDefaults::color())] color: Color,
) {
    Canvas(modifier.fill_max_width().height(thickness), move |d| {
        let (t, width) = (d.to_px(thickness), d.size().width);
        named_args!(d.draw_line(color, Offset::new(0.0, t / 2.0), Offset::new(width, t / 2.0), stroke_width = t));
    });
}

/// A line down the height it is given (Compose's `VerticalDivider`), `thickness` wide, in `color`.
#[kompozable]
pub fn VerticalDivider(
    #[default(Modifier)] modifier: Modifier,
    #[default(DividerDefaults::Thickness)] thickness: Dp,
    #[default(DividerDefaults::color())] color: Color,
) {
    Canvas(modifier.fill_max_height().width(thickness), move |d| {
        let (t, height) = (d.to_px(thickness), d.size().height);
        named_args!(d.draw_line(color, Offset::new(t / 2.0, 0.0), Offset::new(t / 2.0, height), stroke_width = t));
    });
}

/// A line across (Compose's deprecated `Divider`, renamed `HorizontalDivider`): a Box of `thickness` (a hairline:
/// one px) in `color`.
#[deprecated(note = "Renamed to HorizontalDivider")]
#[kompozable]
pub fn Divider(
    #[default(Modifier)] modifier: Modifier,
    #[default(DividerDefaults::Thickness)] thickness: Dp,
    #[default(DividerDefaults::color())] color: Color,
) {
    let target_thickness = if thickness == Dp::Hairline { Dp(1.0 / LocalDensity.current().density) } else { thickness };
    Box(modifier =
        modifier.fill_max_width().height(target_thickness).background_with(color, kompoz_graphics::RectangleShape));
}

/// The dividers' defaults (Compose's DividerDefaults).
pub struct DividerDefaults;

#[allow(non_snake_case)]
impl DividerDefaults {
    /// 1 dp.
    pub const Thickness: Dp = DividerTokens::Thickness;

    /// The scheme's outline variant (Compose's `DividerDefaults.color`).
    pub fn color() -> Color {
        DividerTokens::Color.value()
    }
}
