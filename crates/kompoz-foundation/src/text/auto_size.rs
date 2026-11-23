//! Text sized to fit (Compose's TextAutoSize.kt and TextAutoSizeLayoutScope): a [`TextAutoSize`] picks the font
//! size a text is laid out in, laying it out in sizes it tries through a [`TextAutoSizeLayoutScope`];
//! [`TextAutoSize::step_based`] takes the largest of a range of steps that fits.

use super::modifiers::multi_paragraph_layout_cache::MultiParagraphLayoutCache;
use kompoz_graphics::{Constraints, Density, TextUnit, TextUnitType, UnitExt};
use kompoz_text::{AnnotatedString, TextLayoutResult, TextOverflow};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// What a text's size comes from when it is sized to fit (Compose's TextAutoSize): [`get_font_size`] lays the text
/// out in the sizes it tries (through the scope's [`perform_layout`](TextAutoSizeLayoutScope::perform_layout)) and
/// says which to use. Compared by value ([`TextAutoSizeEq`]: a `PartialEq` type is), as a text node keeps its
/// layout while its auto size is equal.
///
/// [`get_font_size`]: TextAutoSize::get_font_size
pub trait TextAutoSize: TextAutoSizeEq + fmt::Debug {
    /// The font size to lay `text` out in within `constraints` (an em size is of the text style's).
    fn get_font_size(
        &self,
        scope: &mut TextAutoSizeLayoutScope<'_>,
        constraints: Constraints,
        text: &AnnotatedString,
    ) -> TextUnit;
}

/// How auto sizes compare (Compose's TextAutoSize equals): by type and value, for every `PartialEq` type.
pub trait TextAutoSizeEq: Any {
    /// The auto size itself.
    fn auto_size_as_any(&self) -> &dyn Any;
    /// Whether `other` is one of the same type, equal to this.
    fn auto_size_eq(&self, other: &dyn Any) -> bool;
}

impl<T: PartialEq + Any> TextAutoSizeEq for T {
    fn auto_size_as_any(&self) -> &dyn Any {
        self
    }
    fn auto_size_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
}

impl PartialEq for dyn TextAutoSize {
    fn eq(&self, other: &dyn TextAutoSize) -> bool {
        self.auto_size_eq(other.auto_size_as_any())
    }
}

/// Two auto sizes alike (both none, or equal ones).
pub(crate) fn same_auto_size(a: &Option<Rc<dyn TextAutoSize>>, b: &Option<Rc<dyn TextAutoSize>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => **a == **b,
        _ => false,
    }
}

/// The defaults of the auto sizes (Compose's TextAutoSizeDefaults).
pub struct TextAutoSizeDefaults;

impl TextAutoSizeDefaults {
    /// The smallest size by default: 12 sp.
    pub const MinFontSize: TextUnit = TextUnit::new(12.0, TextUnitType::Sp);
    /// The largest size by default: 112 sp.
    pub const MaxFontSize: TextUnit = TextUnit::new(112.0, TextUnitType::Sp);
}

impl dyn TextAutoSize {
    /// The largest font size that fits (Compose's TextAutoSize.StepBased): from `min_font_size` to
    /// `max_font_size`, every size `step_size` apart from the min; with an ellipsis, a size fits where the text
    /// isn't ellipsized. A min above the max is taken as the max. Unspecified sizes, a step under 0.0001 sp or a
    /// negative size panic.
    ///
    /// Kotlin: `TextAutoSize.StepBased(minFontSize: TextUnit = 12.sp, maxFontSize: TextUnit = 112.sp, stepSize: TextUnit = 0.25.sp)`
    pub fn step_based(min_font_size: TextUnit, max_font_size: TextUnit, step_size: TextUnit) -> Rc<dyn TextAutoSize> {
        Rc::new(AutoSizeStepBased::new(min_font_size, max_font_size, step_size))
    }
}

/// Compose's `TextAutoSize.StepBased(minFontSize = 12.sp, maxFontSize = 112.sp, stepSize = 0.25.sp)` by name; plain
/// code calls [`step_based`](<dyn TextAutoSize>::step_based).
#[kompoz_runtime::named]
pub fn StepBased(
    #[default(TextAutoSizeDefaults::MinFontSize)] min_font_size: TextUnit,
    #[default(TextAutoSizeDefaults::MaxFontSize)] max_font_size: TextUnit,
    #[default(0.25.sp())] step_size: TextUnit,
) -> Rc<dyn TextAutoSize> {
    <dyn TextAutoSize>::step_based(min_font_size, max_font_size, step_size)
}

/// Compose's AutoSizeStepBased: a binary search over the range, then a step up if that fits too.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AutoSizeStepBased {
    min_font_size: TextUnit,
    max_font_size: TextUnit,
    step_size: TextUnit,
}

impl AutoSizeStepBased {
    #[track_caller]
    pub fn new(min_font_size: TextUnit, max_font_size: TextUnit, step_size: TextUnit) -> AutoSizeStepBased {
        assert!(
            min_font_size.is_specified(),
            "kompoz: AutoSize.StepBased: TextUnit.Unspecified is not a valid value for minFontSize. Try using other \
             values e.g. 10.sp"
        );
        assert!(
            max_font_size.is_specified(),
            "kompoz: AutoSize.StepBased: TextUnit.Unspecified is not a valid value for maxFontSize. Try using other \
             values e.g. 100.sp"
        );
        assert!(
            step_size.is_specified(),
            "kompoz: AutoSize.StepBased: TextUnit.Unspecified is not a valid value for stepSize. Try using other \
             values e.g. 0.25.sp"
        );
        let min_font_size = if min_font_size.unit_type() == max_font_size.unit_type()
            && min_font_size.value() > max_font_size.value()
        {
            max_font_size
        } else {
            min_font_size
        };
        assert!(
            !(step_size.is_sp() && step_size.value() < 0.0001),
            "kompoz: AutoSize.StepBased: stepSize must be greater than or equal to 0.0001f.sp"
        );
        assert!(min_font_size.value() >= 0.0, "kompoz: AutoSize.StepBased: minFontSize must not be negative");
        assert!(max_font_size.value() >= 0.0, "kompoz: AutoSize.StepBased: maxFontSize must not be negative");
        AutoSizeStepBased { min_font_size, max_font_size, step_size }
    }
}

impl TextAutoSize for AutoSizeStepBased {
    fn get_font_size(
        &self,
        scope: &mut TextAutoSizeLayoutScope<'_>,
        constraints: Constraints,
        text: &AnnotatedString,
    ) -> TextUnit {
        let step = scope.text_to_px(self.step_size);
        let smallest = scope.text_to_px(self.min_font_size);
        let largest = scope.text_to_px(self.max_font_size);
        let (mut min, mut max) = (smallest, largest);
        let mut current = (min + max) / 2.0;
        while max - min >= step {
            let layout = scope.perform_layout(constraints, text, scope.to_sp(current));
            if did_overflow(&layout) {
                max = current;
            } else {
                min = current;
            }
            current = (min + max) / 2.0;
        }
        // A size the min plus whole steps.
        current = ((min - smallest) / step).floor() * step + smallest;
        // It fits; a step up may too.
        if current + step <= largest {
            let layout = scope.perform_layout(constraints, text, scope.to_sp(current + step));
            if !did_overflow(&layout) {
                current += step;
            }
        }
        scope.to_sp(current)
    }
}

/// Whether a layout doesn't fit, as its overflow says (Compose's didOverflow): clipped or visible text by its
/// bounds, ellipsized text by its ellipsis.
fn did_overflow(r: &TextLayoutResult) -> bool {
    match r.layout_input.overflow {
        TextOverflow::Clip | TextOverflow::Visible => did_overflow_bounds(r),
        TextOverflow::StartEllipsis | TextOverflow::MiddleEllipsis | TextOverflow::Ellipsis => {
            did_overflow_by_ellipsize(r)
        }
    }
}

fn did_overflow_bounds(r: &TextLayoutResult) -> bool {
    r.did_overflow_width() || r.did_overflow_height()
}

/// Compose's didOverflowByEllipsize: one line ellipsized; on more lines, a start or middle ellipsis (which only
/// one line gets) falls back to the bounds, an end one is on the last line.
fn did_overflow_by_ellipsize(r: &TextLayoutResult) -> bool {
    match r.line_count() {
        0 => false,
        1 => r.is_line_ellipsized(0),
        n => match r.layout_input.overflow {
            TextOverflow::StartEllipsis | TextOverflow::MiddleEllipsis => did_overflow_bounds(r),
            TextOverflow::Ellipsis => r.is_line_ellipsized(n - 1),
            _ => false,
        },
    }
}

/// Where an auto size lays its text out (Compose's TextAutoSizeLayoutScope): [`perform_layout`] in a size; the
/// density (it derefs to it), with sp and em lengths turned into px by the text's style
/// ([`text_to_px`](TextAutoSizeLayoutScope::text_to_px)).
///
/// [`perform_layout`]: TextAutoSizeLayoutScope::perform_layout
pub struct TextAutoSizeLayoutScope<'a> {
    pub(crate) cache: &'a mut MultiParagraphLayoutCache,
    pub(crate) density: Density,
}

impl TextAutoSizeLayoutScope<'_> {
    /// The text laid out in `constraints` at `font_size` (an em size of the style's), its other settings its own
    /// (Compose's performLayout: the text it lays out is the node's).
    pub fn perform_layout(
        &mut self,
        constraints: Constraints,
        text: &AnnotatedString,
        font_size: TextUnit,
    ) -> TextLayoutResult {
        let _ = text;
        self.cache.perform_auto_size_layout(constraints, font_size)
    }

    /// An sp length, or an em one of the style's font size, in px (Compose's scope's TextUnit.toPx): an em length
    /// where the style's size is in em, or has none, panics.
    #[track_caller]
    pub fn text_to_px(&self, unit: TextUnit) -> f32 {
        if unit.is_em() {
            let font_size = self.cache.style().font_size;
            assert!(
                !font_size.is_em(),
                "kompoz: InternalAutoSize -> toPx(): Cannot convert Em to Px when style.fontSize is Em\nDeclare the \
                 composable's style.fontSize with Sp units instead."
            );
            assert!(
                font_size.is_specified(),
                "kompoz: InternalAutoSize -> toPx(): Cannot convert Em to Px when style.fontSize is not set. Please \
                 specify a font size."
            );
            return self.density.text_to_px(font_size) * unit.value();
        }
        self.density.text_to_px(unit)
    }
}

impl std::ops::Deref for TextAutoSizeLayoutScope<'_> {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}
