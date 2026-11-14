//! `Spacer`, as Compose's: empty space of its modifier's size.

use kompoz_runtime::kompozable;
use kompoz_ui::layout::layout_into;
use kompoz_ui::{Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier};
use std::rc::Rc;

/// Empty space, as big as its modifier makes it (`Modifier.size`, `width`, `weight`, …): a fixed side its size, any
/// other 0 (Compose's `Spacer`).
#[kompozable(inline)]
pub fn Spacer(modifier: Modifier) {
    layout_into(modifier, SPACER_MEASURE_POLICY.with(Rc::clone), false, || {});
}

thread_local! {
    /// The one policy every Spacer shares (Compose's SpacerMeasurePolicy object).
    static SPACER_MEASURE_POLICY: Rc<dyn MeasurePolicy> = Rc::new(SpacerMeasurePolicy);
}

/// Compose's SpacerMeasurePolicy: a fixed side the size given, any other 0.
struct SpacerMeasurePolicy;

impl MeasurePolicy for SpacerMeasurePolicy {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        let width = if constraints.has_fixed_width() { constraints.max_width } else { 0 };
        let height = if constraints.has_fixed_height() { constraints.max_height } else { 0 };
        scope.layout(width, height, |_| {})
    }
}
