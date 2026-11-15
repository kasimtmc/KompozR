//! The `Layout` composables (Compose's `Layout` and `MultiMeasureLayout`): a layout node emitted with a measure
//! policy, a modifier and the content that emits its children.

use super::{Measurable, MeasurePolicy, MeasureResult, MeasureScope};
use crate::modifier::Modifier;
use crate::node::LayoutNode;
use crate::platform::{LocalDensity, LocalLayoutDirection};
use crate::unit::Constraints;
use kompoz_runtime::{current_composite_key_hash, current_kompozition_local_map, emit, kompozable};
use std::rc::Rc;

/// A measure policy from a closure (Compose's `MeasurePolicy { measurables, constraints -> … }`), its parameter
/// types inferred: `Layout(content = …, measure_policy(|scope, measurables, constraints| scope.layout(…)))`.
pub fn measure_policy<F>(measure: F) -> F
where
    F: Fn(&MeasureScope, &[&dyn Measurable], Constraints) -> MeasureResult + 'static,
{
    measure
}

/// A layout: the children `content` emits, measured and placed by `measure_policy`, decorated by `modifier`
/// (Compose's `Layout`). Without content, a leaf (Compose's `Layout(modifier, measurePolicy)`). A policy made anew
/// at each composition (a closure) measures the layout again whenever this composes again.
#[kompozable(inline)]
pub fn Layout(
    #[default(|| {})] content: impl FnOnce(),
    #[default(Modifier)] modifier: Modifier,
    measure_policy: impl MeasurePolicy,
) {
    layout_into(modifier, Rc::new(measure_policy), false, content);
}

/// A layout whose policy may measure a child more than once (Compose's deprecated `MultiMeasureLayout`).
#[kompozable(inline)]
pub fn MultiMeasureLayout(
    #[default(Modifier)] modifier: Modifier,
    content: impl FnOnce(),
    measure_policy: impl MeasurePolicy,
) {
    layout_into(modifier, Rc::new(measure_policy), true, content);
}

/// Emits a layout node (what `Layout` does, for composables that keep their measure policy: `Box`, `Row`, …):
/// its policy, density, layout direction and composition locals set, then its modifier, as Compose's
/// ComposeUiNode setters, then its children.
#[track_caller]
pub fn layout_into(
    modifier: Modifier,
    measure_policy: Rc<dyn MeasurePolicy>,
    multi_measure: bool,
    content: impl FnOnce(),
) {
    let density = LocalDensity.current();
    let layout_direction = LocalLayoutDirection.current();
    let locals = current_kompozition_local_map();
    let key = current_composite_key_hash();
    emit(
        LayoutNode::new,
        move |node: &LayoutNode| {
            node.set_measure_policy(measure_policy);
            node.can_multi_measure.set(multi_measure);
            node.set_density(density);
            node.set_layout_direction(layout_direction);
            node.set_locals(locals);
            node.modifier_set(modifier);
            node.set_composite_key(key);
            node.note_composed();
        },
        content,
    );
}
