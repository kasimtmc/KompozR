//! The graphics types as the Kotlin syntax sees them: written as Compose's `toString()` writes them (a string
//! template's `"$size"`: `Size(10.0, 20.0)`, `16.0.dp`), and `Dp` ranges (`0.dp..10.dp`).

use crate::{
    Color, Constraints, CornerRadius, Density, Dp, DpOffset, DpRect, DpSize, IntOffset, IntRect, IntSize,
    LayoutDirection, Offset, Rect, RoundRect, Size, TextUnit, TextUnitType,
};
use kompoz_runtime::kt::{KtProgression, KtRangeBound, KtToString};

/// Compose's `Float.toStringAsFixed(digits)` (ui-geometry's GeometryUtils): shifted by 10^digits, rounded half up
/// by hand, shifted back, written as Kotlin writes a `Float`.
fn as_fixed(v: f32, digits: i32) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    let digits = digits.max(0);
    let pow = 10f32.powi(digits);
    let shifted = v * pow;
    // (Kotlin's toInt: toward zero, saturated)
    let whole = shifted as i32;
    let decimal = shifted - whole as f32;
    let rounded_shifted = if decimal >= 0.5 { whole.wrapping_add(1) } else { whole };
    let rounded = rounded_shifted as f32 / pow;
    if digits > 0 { rounded.kt_to_string() } else { (rounded as i32).to_string() }
}

/// `16.0.dp`, `Dp.Unspecified`.
impl KtToString for Dp {
    fn kt_write(&self, out: &mut String) {
        if self.is_unspecified() {
            out.push_str("Dp.Unspecified");
        } else {
            self.0.kt_write(out);
            out.push_str(".dp");
        }
    }
}

/// `16.0.sp`, `1.5.em`, `Unspecified`.
impl KtToString for TextUnit {
    fn kt_write(&self, out: &mut String) {
        match self.unit_type() {
            TextUnitType::Unspecified => out.push_str("Unspecified"),
            TextUnitType::Sp => {
                self.value().kt_write(out);
                out.push_str(".sp");
            }
            TextUnitType::Em => {
                self.value().kt_write(out);
                out.push_str(".em");
            }
        }
    }
}

/// `Sp`, `Em`, `Unspecified`.
impl KtToString for TextUnitType {
    fn kt_write(&self, out: &mut String) {
        out.push_str(match self {
            TextUnitType::Unspecified => "Unspecified",
            TextUnitType::Sp => "Sp",
            TextUnitType::Em => "Em",
        });
    }
}

/// `(1.0.dp, 2.0.dp)`, `DpOffset.Unspecified`.
impl KtToString for DpOffset {
    fn kt_write(&self, out: &mut String) {
        if !self.is_specified() {
            out.push_str("DpOffset.Unspecified");
            return;
        }
        out.push('(');
        self.x.kt_write(out);
        out.push_str(", ");
        self.y.kt_write(out);
        out.push(')');
    }
}

/// `10.0.dp x 20.0.dp`, `DpSize.Unspecified`.
impl KtToString for DpSize {
    fn kt_write(&self, out: &mut String) {
        if !self.is_specified() {
            out.push_str("DpSize.Unspecified");
            return;
        }
        self.width.kt_write(out);
        out.push_str(" x ");
        self.height.kt_write(out);
    }
}

/// A data class's: `DpRect(left=0.0.dp, top=0.0.dp, right=1.0.dp, bottom=1.0.dp)`.
impl KtToString for DpRect {
    fn kt_write(&self, out: &mut String) {
        out.push_str("DpRect(left=");
        self.left.kt_write(out);
        out.push_str(", top=");
        self.top.kt_write(out);
        out.push_str(", right=");
        self.right.kt_write(out);
        out.push_str(", bottom=");
        self.bottom.kt_write(out);
        out.push(')');
    }
}

/// `Offset(1.0, 2.5)` (one decimal), `Offset.Unspecified`.
impl KtToString for Offset {
    fn kt_write(&self, out: &mut String) {
        if !self.is_specified() {
            out.push_str("Offset.Unspecified");
        } else {
            out.push_str(&format!("Offset({}, {})", as_fixed(self.x, 1), as_fixed(self.y, 1)));
        }
    }
}

/// `Size(10.0, 20.0)` (one decimal), `Size.Unspecified`.
impl KtToString for Size {
    fn kt_write(&self, out: &mut String) {
        if !self.is_specified() {
            out.push_str("Size.Unspecified");
        } else {
            out.push_str(&format!("Size({}, {})", as_fixed(self.width, 1), as_fixed(self.height, 1)));
        }
    }
}

fn ltrb(left: f32, top: f32, right: f32, bottom: f32) -> String {
    format!("{}, {}, {}, {}", as_fixed(left, 1), as_fixed(top, 1), as_fixed(right, 1), as_fixed(bottom, 1))
}

/// `Rect.fromLTRB(0.0, 0.0, 10.0, 10.0)`.
impl KtToString for Rect {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("Rect.fromLTRB({})", ltrb(self.left, self.top, self.right, self.bottom)));
    }
}

/// `CornerRadius.circular(4.0)`, `CornerRadius.elliptical(4.0, 2.0)`.
impl KtToString for CornerRadius {
    fn kt_write(&self, out: &mut String) {
        if self.x == self.y {
            out.push_str(&format!("CornerRadius.circular({})", as_fixed(self.x, 1)));
        } else {
            out.push_str(&format!("CornerRadius.elliptical({}, {})", as_fixed(self.x, 1), as_fixed(self.y, 1)));
        }
    }
}

/// `RoundRect(rect=0.0, 0.0, 10.0, 10.0, radius=2.0)` and the other forms Compose's toString has.
impl KtToString for RoundRect {
    fn kt_write(&self, out: &mut String) {
        let rect = ltrb(self.left, self.top, self.right, self.bottom);
        let (tl, tr, br, bl) = (
            self.top_left_corner_radius,
            self.top_right_corner_radius,
            self.bottom_right_corner_radius,
            self.bottom_left_corner_radius,
        );
        if tl == tr && tr == br && br == bl {
            if tl.x == tl.y {
                out.push_str(&format!("RoundRect(rect={rect}, radius={})", as_fixed(tl.x, 1)));
            } else {
                out.push_str(&format!("RoundRect(rect={rect}, x={}, y={})", as_fixed(tl.x, 1), as_fixed(tl.y, 1)));
            }
            return;
        }
        out.push_str(&format!(
            "RoundRect(rect={rect}, topLeft={}, topRight={}, bottomRight={}, bottomLeft={})",
            tl.kt_to_string(),
            tr.kt_to_string(),
            br.kt_to_string(),
            bl.kt_to_string()
        ));
    }
}

/// `(1, 2)`.
impl KtToString for IntOffset {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("({}, {})", self.x, self.y));
    }
}

/// `10 x 20`.
impl KtToString for IntSize {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("{} x {}", self.width, self.height));
    }
}

/// `IntRect.fromLTRB(0, 0, 10, 10)`.
impl KtToString for IntRect {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("IntRect.fromLTRB({}, {}, {}, {})", self.left, self.top, self.right, self.bottom));
    }
}

/// `Color(1.0, 0.0, 0.0, 1.0, sRGB IEC61966-2.1)`.
impl KtToString for Color {
    fn kt_write(&self, out: &mut String) {
        out.push_str("Color(");
        self.red().kt_write(out);
        out.push_str(", ");
        self.green().kt_write(out);
        out.push_str(", ");
        self.blue().kt_write(out);
        out.push_str(", ");
        self.alpha().kt_write(out);
        out.push_str(", ");
        out.push_str(self.color_space().name());
        out.push(')');
    }
}

/// `Constraints(minWidth = 0, maxWidth = Infinity, minHeight = 0, maxHeight = 100)`.
impl KtToString for Constraints {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("{self:?}"));
    }
}

/// Compose's DensityImpl, a data class: `DensityImpl(density=2.0, fontScale=1.0)`.
impl KtToString for Density {
    fn kt_write(&self, out: &mut String) {
        out.push_str("DensityImpl(density=");
        self.density.kt_write(out);
        out.push_str(", fontScale=");
        self.font_scale.kt_write(out);
        out.push(')');
    }
}

/// `Ltr`, `Rtl`.
impl KtToString for LayoutDirection {
    fn kt_write(&self, out: &mut String) {
        out.push_str(match self {
            LayoutDirection::Ltr => "Ltr",
            LayoutDirection::Rtl => "Rtl",
        });
    }
}

/// `0.dp..10.dp`: a range of what isn't counted (`Dp` is `Comparable` in Compose).
impl KtRangeBound for Dp {
    fn kt_until(start: Dp, end: Dp) -> KtProgression<Dp> {
        KtProgression::open_end(start, end)
    }
    fn kt_contains(range: &KtProgression<Dp>, value: &Dp) -> bool {
        kompoz_runtime::kt::range::ordered_contains(range, value)
    }
    fn kt_is_empty(range: &KtProgression<Dp>) -> bool {
        kompoz_runtime::kt::range::ordered_is_empty(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UnitExt;
    use kompoz_runtime::kt::{range_to, template};

    #[test]
    fn written_as_compose_writes_them() {
        assert_eq!(template!(16.dp()), "16.0.dp");
        assert_eq!(template!(Dp::Unspecified), "Dp.Unspecified");
        assert_eq!(template!(14.sp(), " ", 1.5.em(), " ", TextUnit::Unspecified), "14.0.sp 1.5.em Unspecified");
        assert_eq!(template!(Offset(1.25, -1.25)), "Offset(1.3, -1.2)");
        assert_eq!(template!(Offset::Unspecified), "Offset.Unspecified");
        assert_eq!(template!(Size(10.0, 20.04)), "Size(10.0, 20.0)");
        assert_eq!(template!(Rect((0.0, 0.0, 10.0, 5.0))), "Rect.fromLTRB(0.0, 0.0, 10.0, 5.0)");
        assert_eq!(template!(CornerRadius(4.0)), "CornerRadius.circular(4.0)");
        assert_eq!(template!(CornerRadius((4.0, 2.0))), "CornerRadius.elliptical(4.0, 2.0)");
        assert_eq!(
            template!(RoundRect::from_rect_radius(Rect((0.0, 0.0, 10.0, 10.0)), CornerRadius(2.0))),
            "RoundRect(rect=0.0, 0.0, 10.0, 10.0, radius=2.0)"
        );
        assert_eq!(template!(IntOffset(1, 2), " ", IntSize(3, 4)), "(1, 2) 3 x 4");
        assert_eq!(template!(IntRect((0, 0, 10, 10))), "IntRect.fromLTRB(0, 0, 10, 10)");
        assert_eq!(template!(DpOffset(1.dp(), 2.dp())), "(1.0.dp, 2.0.dp)");
        assert_eq!(template!(DpSize(1.dp(), 2.dp())), "1.0.dp x 2.0.dp");
        assert_eq!(template!(Color::Red), "Color(1.0, 0.0, 0.0, 1.0, sRGB IEC61966-2.1)");
        assert_eq!(
            template!(Constraints::new(0, 100, 0, Constraints::Infinity)),
            "Constraints(minWidth = 0, maxWidth = 100, minHeight = 0, maxHeight = Infinity)"
        );
        assert_eq!(template!(Density((2.0, 1.5))), "DensityImpl(density=2.0, fontScale=1.5)");
        assert_eq!(template!(vec![1.dp(), 2.dp()]), "[1.0.dp, 2.0.dp]");
        assert!(kompoz_runtime::kt::is_in(&4.dp(), &range_to(0.dp(), 10.dp())));
        assert_eq!(template!(range_to(0.dp(), 10.dp())), "0.0.dp..10.0.dp");
    }

    #[test]
    fn constructors_as_compose_writes_them() {
        assert_eq!(Color(0xFF6200EE_i64), Color::from_argb(0xFF6200EE));
        assert_eq!(Color(0x80FF0000_u32.cast_signed()), Color::from_argb(0x80FF0000));
        assert_eq!(Color((255, 0, 0)), Color::Red);
        assert_eq!(Color((0, 0, 255, 0x80)), Color::from_argb(0x800000FF));
        assert_eq!(Color((1.0, 0.0, 0.0)), Color::Red);
        assert_eq!(Color((0.0, 1.0, 0.0, 1.0)), Color::Green);
        assert_eq!(Color((1.0, 0.0, 0.0, 1.0, &crate::colorspace::ColorSpaces::Srgb)), Color::Red);
        assert_eq!(Color(Color::Blue.value()), Color::Blue);
        assert_eq!(Rect((Offset(1.0, 2.0), Size(3.0, 4.0))), Rect::new(1.0, 2.0, 4.0, 6.0));
        assert_eq!(Rect((Offset(5.0, 5.0), 2.0)), Rect::new(3.0, 3.0, 7.0, 7.0));
        assert_eq!(IntRect((IntOffset(1, 1), IntOffset(3, 4))), IntRect::new(1, 1, 3, 4));
        assert_eq!(Density(2.0).font_scale(), 1.0);
        let mut size = Size(1.0, 2.0);
        assert_eq!((size.width(), size.height()), (1.0, 2.0));
        size.width = 3.0;
        assert_eq!(size.width(), 3.0);
        assert_eq!(Constraints::new(1, 2, 3, 4).max_height(), 4);
        assert_eq!(DpRect((DpOffset(1.dp(), 1.dp()), DpSize(2.dp(), 3.dp()))).bottom(), 4.dp());
    }
}
