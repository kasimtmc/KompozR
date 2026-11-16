//! The text types as the Kotlin syntax writes them (a string template's `"$weight"`), as Compose's `toString()`.

use crate::font::{FontStyle, FontWeight};
use crate::style::{TextAlign, TextDirection};
use kompoz_runtime::kt::KtToString;

/// `FontWeight(weight=700)`.
impl KtToString for FontWeight {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("FontWeight(weight={})", self.weight));
    }
}

/// `Normal`, `Italic`.
impl KtToString for FontStyle {
    fn kt_write(&self, out: &mut String) {
        out.push_str(match self {
            FontStyle::Normal => "Normal",
            FontStyle::Italic => "Italic",
        });
    }
}

/// `Left`, `Center`, … `Unspecified`.
impl KtToString for TextAlign {
    fn kt_write(&self, out: &mut String) {
        out.push_str(match self {
            TextAlign::Unspecified => "Unspecified",
            TextAlign::Left => "Left",
            TextAlign::Right => "Right",
            TextAlign::Center => "Center",
            TextAlign::Justify => "Justify",
            TextAlign::Start => "Start",
            TextAlign::End => "End",
        });
    }
}

/// `Ltr`, `Rtl`, `Content`, `ContentOrLtr`, `ContentOrRtl`, `Unspecified`.
impl KtToString for TextDirection {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("{self:?}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextStyle;
    use kompoz_graphics::UnitExt;
    use kompoz_runtime::kt::template;

    #[test]
    fn text_values_as_compose_writes_them() {
        assert_eq!(template!(FontWeight::Bold), "FontWeight(weight=700)");
        assert_eq!(template!(FontStyle::Italic, " ", TextAlign::Center), "Italic Center");
        assert_eq!(template!(TextDirection::ContentOrLtr), "ContentOrLtr");
        let style = TextStyle { font_size: 30.sp(), ..TextStyle::Default };
        assert_eq!(style.font_size().value(), 30.0);
        assert_eq!(style.font_weight(), None);
        assert_eq!(FontWeight::Bold.weight(), 700);
    }
}
