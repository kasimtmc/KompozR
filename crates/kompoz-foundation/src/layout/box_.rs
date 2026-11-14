//! `Box`, as Compose's: its children one over the other, each where its alignment puts it.

use kompoz_graphics::{IntSize, LayoutDirection};
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::layout::layout_into;
use kompoz_ui::{Alignment, Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier, Placeable};
use std::rc::Rc;

/// A layout of its children one over the other, as big as the biggest (at least its min constraints), each placed
/// by `content_alignment` or by its own `Modifier.align` (Compose's `Box`). With `propagate_min_constraints`, the
/// children get the Box's min constraints too. A child with `Modifier.match_parent_size()` doesn't count towards
/// the size and is measured to it once the others are. Without content, an empty box of its modifier's size
/// (Compose's `Box(modifier)`).
#[kompozable(inline)]
pub fn Box(
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(false)] propagate_min_constraints: bool,
    #[default(|| {})] content: impl FnOnce(),
) {
    let policy = box_measure_policy(content_alignment, propagate_min_constraints);
    layout_into(modifier, policy, false, content);
}

/// The policy of a Box, kept while its arguments are equal (Compose's `maybeCachedBoxMeasurePolicy`), so the Box
/// isn't measured again when it recomposes with the same ones.
#[track_caller]
fn box_measure_policy(alignment: Alignment, propagate_min_constraints: bool) -> Rc<dyn MeasurePolicy> {
    remember_keyed((alignment.clone(), propagate_min_constraints), move || {
        Rc::new(BoxMeasurePolicy { alignment, propagate_min_constraints }) as Rc<dyn MeasurePolicy>
    })
}

/// What a Box's child tells it (Compose's BoxChildDataNode): its own alignment, and whether it matches the
/// Box's size.
#[derive(Clone, Debug)]
pub(crate) struct BoxChildData {
    pub(crate) alignment: Alignment,
    pub(crate) match_parent_size: bool,
}

fn box_child_data(measurable: &dyn Measurable) -> Option<BoxChildData> {
    measurable.parent_data()?.downcast_ref::<BoxChildData>().cloned()
}

/// Compose's BoxMeasurePolicy (its intrinsics the default ones).
struct BoxMeasurePolicy {
    alignment: Alignment,
    propagate_min_constraints: bool,
}

impl MeasurePolicy for BoxMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        if measurables.is_empty() {
            return scope.layout(constraints.min_width, constraints.min_height, |_| {});
        }
        let content_constraints =
            if self.propagate_min_constraints { constraints } else { constraints.copy_max_dimensions() };
        let datas: Vec<Option<BoxChildData>> = measurables.iter().map(|m| box_child_data(*m)).collect();
        let matches_parent = |i: usize| datas[i].as_ref().is_some_and(|d| d.match_parent_size);

        if measurables.len() == 1 {
            let (placeable, box_width, box_height) = if !matches_parent(0) {
                let placeable = measurables[0].measure(content_constraints);
                let width = constraints.min_width.max(placeable.width());
                let height = constraints.min_height.max(placeable.height());
                (placeable, width, height)
            } else {
                let placeable =
                    measurables[0].measure(Constraints::fixed(constraints.min_width, constraints.min_height));
                (placeable, constraints.min_width, constraints.min_height)
            };
            let alignment = self.alignment.clone();
            let layout_direction = scope.layout_direction();
            let data = datas.into_iter().next().flatten();
            return scope.layout(box_width, box_height, move |p| {
                place_in_box(p, &placeable, data.as_ref(), layout_direction, box_width, box_height, &alignment)
            });
        }

        // The children that don't match the Box's size first: they make it.
        let mut placeables: Vec<Option<Placeable>> = vec![None; measurables.len()];
        let mut has_match_parent_size_children = false;
        let mut box_width = constraints.min_width;
        let mut box_height = constraints.min_height;
        for (i, measurable) in measurables.iter().enumerate() {
            if !matches_parent(i) {
                let placeable = measurable.measure(content_constraints);
                box_width = box_width.max(placeable.width());
                box_height = box_height.max(placeable.height());
                placeables[i] = Some(placeable);
            } else {
                has_match_parent_size_children = true;
            }
        }
        // Then the ones that do.
        if has_match_parent_size_children {
            // (the infinity check is for default intrinsic measurements)
            let match_parent_size_constraints = Constraints::new(
                if box_width != Constraints::Infinity { box_width } else { 0 },
                box_width,
                if box_height != Constraints::Infinity { box_height } else { 0 },
                box_height,
            );
            for (i, measurable) in measurables.iter().enumerate() {
                if matches_parent(i) {
                    placeables[i] = Some(measurable.measure(match_parent_size_constraints));
                }
            }
        }
        let placeables: Vec<Placeable> = placeables.into_iter().map(|p| p.expect("measured")).collect();
        let alignment = self.alignment.clone();
        let layout_direction = scope.layout_direction();
        scope.layout(box_width, box_height, move |p| {
            for (placeable, data) in placeables.iter().zip(&datas) {
                place_in_box(p, placeable, data.as_ref(), layout_direction, box_width, box_height, &alignment);
            }
        })
    }
}

/// Places a child of a Box by its own alignment or the Box's (Compose's `placeInBox`).
fn place_in_box(
    scope: &kompoz_ui::PlacementScope,
    placeable: &Placeable,
    data: Option<&BoxChildData>,
    layout_direction: LayoutDirection,
    box_width: i32,
    box_height: i32,
    alignment: &Alignment,
) {
    let child_alignment = data.map(|d| &d.alignment).unwrap_or(alignment);
    let position = child_alignment.align(
        IntSize::new(placeable.width(), placeable.height()),
        IntSize::new(box_width, box_height),
        layout_direction,
    );
    scope.place_at(placeable, position, 0.0);
}
