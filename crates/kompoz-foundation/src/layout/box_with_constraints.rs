//! `BoxWithConstraints`, as Compose's: a `Box` whose content is composed as it is measured, knowing the
//! constraints it is measured in.

use super::box_::box_measure_policy;
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::kompozable;
use kompoz_ui::layout::SubcomposeLayout;
use kompoz_ui::{Alignment, Constraints, Measurable, Modifier};

/// A `Box` whose content is composed with the constraints its parent measures it in (Compose's
/// `BoxWithConstraints`): `content` gets a [`BoxWithConstraintsScope`] telling them (in px, and its bounds in dp),
/// and can show something else when there is less room. Composed as it measures (a `SubcomposeLayout`, in a slot
/// of its own), its content sees the constraints from its first composition, and is composed again when they
/// change. Its children are placed as a Box's are (`content_alignment`, `propagate_min_constraints`). Asking it for
/// an intrinsic size panics, as Compose's throws.
#[kompozable(plain(measure))]
pub fn BoxWithConstraints(
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(false)] propagate_min_constraints: bool,
    content: impl Fn(&BoxWithConstraintsScope) + 'static,
) {
    let measure_policy = box_measure_policy(content_alignment, propagate_min_constraints);
    let content = Rc::new(content);
    SubcomposeLayout(
        modifier = modifier,
        measure_policy = move |scope, constraints: Constraints| {
            let box_scope = BoxWithConstraintsScope { density: scope.density(), constraints };
            let content = content.clone();
            let measurables = scope.subcompose((), move || content(&box_scope));
            let refs: Vec<&dyn Measurable> = measurables.iter().map(|m| &**m).collect();
            measure_policy.measure(scope, &refs, constraints)
        },
    );
}

/// What a [`BoxWithConstraints`]'s content is composed in (Compose's `BoxWithConstraintsScope`, a `BoxScope`: its
/// children's `align` and `match_parent_size` are a Box's): the constraints it is measured in, in px, and its
/// bounds in dp.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BoxWithConstraintsScope {
    density: Density,
    constraints: Constraints,
}

impl BoxWithConstraintsScope {
    /// The constraints its parent measures it in, in px (Compose's `constraints`).
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }

    /// The least width it may be (Compose's `minWidth`).
    pub fn min_width(&self) -> Dp {
        self.density.to_dp(self.constraints.min_width as f32)
    }

    /// The most width it may be, [`Dp::Infinity`] if it isn't bounded (Compose's `maxWidth`).
    pub fn max_width(&self) -> Dp {
        if self.constraints.has_bounded_width() {
            self.density.to_dp(self.constraints.max_width as f32)
        } else {
            Dp::Infinity
        }
    }

    /// The least height it may be (Compose's `minHeight`).
    pub fn min_height(&self) -> Dp {
        self.density.to_dp(self.constraints.min_height as f32)
    }

    /// The most height it may be, [`Dp::Infinity`] if it isn't bounded (Compose's `maxHeight`).
    pub fn max_height(&self) -> Dp {
        if self.constraints.has_bounded_height() {
            self.density.to_dp(self.constraints.max_height as f32)
        } else {
            Dp::Infinity
        }
    }
}
