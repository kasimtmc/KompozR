//! Progress semantics (Compose's ProgressSemantics.kt): what a progress indicator says to accessibility.

use kompoz_runtime::named;
use kompoz_ui::Modifier;
use kompoz_ui::semantics::ProgressBarRangeInfo;
use std::ops::RangeInclusive;

/// The progress semantics modifiers.
#[named(trait ProgressSemanticsModifiers)]
impl Modifier {
    /// A progress of `value` (kept in `value_range`) in `steps` steps, its descendants merged (Compose's
    /// `progressSemantics(value, valueRange, steps)`).
    fn progress_semantics(
        self,
        value: f32,
        #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
        #[default(0)] steps: i32,
    ) -> Modifier {
        self.progress_semantics_with(value, value_range, steps)
    }

    /// [`progress_semantics`](ProgressSemanticsModifiers::progress_semantics) with all its parameters, for plain
    /// code.
    fn progress_semantics_with(self, value: f32, value_range: RangeInclusive<f32>, steps: i32) -> Modifier {
        let current = value.clamp(*value_range.start(), *value_range.end());
        let info = ProgressBarRangeInfo::new(current, value_range, steps);
        self.semantics_with(true, move |c| c.set_progress_bar_range_info(info.clone()))
    }

    /// An indeterminate progress, its descendants merged (Compose's `progressSemantics()`).
    fn progress_semantics_indeterminate(self) -> Modifier {
        self.semantics_with(true, |c| c.set_progress_bar_range_info(ProgressBarRangeInfo::Indeterminate))
    }
}
