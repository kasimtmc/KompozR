//! A lazy item's modifiers (Compose's LazyItemScope.kt and LazyItemScopeImpl.kt): `fill_parent_max_size`,
//! `fill_parent_max_width`, `fill_parent_max_height` (a fraction of the list's viewport past its content padding)
//! and `animate_item`. Compose's `LazyItemScope` is the receiver of an item's content; a Rust closure has none, so
//! these are methods of one trait on `Modifier` usable anywhere, as the layout scope modifiers are: the list tells
//! its items its viewport through a composition local, and outside a list `fill_parent_max_*` fill the constraints
//! (as `fill_max_*`), `animate_item` does nothing.

use crate::lazy::layout::{LazyLayoutAnimateItemElement, LazyLayoutAnimationSpecs};
use kompoz_animation::{FiniteAnimationSpec, Spring, SpringSpec, VisibilityThreshold};
use kompoz_graphics::IntOffset;
use kompoz_runtime::{KompozitionLocal, MutableState, mutable_state_of, named};
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx,
    node_capabilities,
};
use std::sync::Arc;

/// The viewport a list's items fill a fraction of (Compose's LazyItemScopeImpl: its max size states, set by each
/// measure). Clones are the same.
#[derive(Clone, PartialEq)]
pub(crate) struct LazyItemScopeImpl {
    max_width_state: MutableState<i32>,
    max_height_state: MutableState<i32>,
}

impl Default for LazyItemScopeImpl {
    fn default() -> Self {
        LazyItemScopeImpl { max_width_state: mutable_state_of(i32::MAX), max_height_state: mutable_state_of(i32::MAX) }
    }
}

impl LazyItemScopeImpl {
    pub(crate) fn set_max_size(&self, width: i32, height: i32) {
        self.max_width_state.set(width);
        self.max_height_state.set(height);
    }
}

/// The item scope of the list an item is in (none outside one).
pub(crate) static LocalLazyItemScope: KompozitionLocal<Option<LazyItemScopeImpl>> = KompozitionLocal::new(|| None);

/// The spring `animate_item`'s fades go along by default (Compose's `spring(stiffness = StiffnessMediumLow)`).
fn default_fade_spec() -> Option<Arc<dyn FiniteAnimationSpec<f32>>> {
    Some(Arc::new(SpringSpec::<f32>::new(Spring::DampingRatioNoBouncy, Spring::StiffnessMediumLow, None)))
}

/// The spring its placement goes along by default (with IntOffset's visibility threshold).
fn default_placement_spec() -> Option<Arc<dyn FiniteAnimationSpec<IntOffset>>> {
    Some(Arc::new(SpringSpec::new(
        Spring::DampingRatioNoBouncy,
        Spring::StiffnessMediumLow,
        Some(IntOffset::VisibilityThreshold),
    )))
}

/// The modifiers of a lazy list's items (Compose's `LazyItemScope` modifiers).
#[named(trait LazyItemScopeModifiers)]
impl Modifier {
    /// As big as `fraction` of the list's viewport (past its content padding) both ways (Compose's
    /// `fillParentMaxSize`): the viewport's width in a column, its height too (a column's items aren't bounded
    /// along it, so `fill_max_size` wouldn't do). Outside a list, `fill_max_size`.
    fn fill_parent_max_size(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then_element(ParentSizeElement { fraction, width: true, height: true, name: "fillParentMaxSize" })
    }

    /// As wide as `fraction` of the list's viewport (Compose's `fillParentMaxWidth`).
    fn fill_parent_max_width(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then_element(ParentSizeElement { fraction, width: true, height: false, name: "fillParentMaxWidth" })
    }

    /// As tall as `fraction` of the list's viewport (Compose's `fillParentMaxHeight`).
    fn fill_parent_max_height(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then_element(ParentSizeElement { fraction, width: false, height: true, name: "fillParentMaxHeight" })
    }

    /// Animates the item (Compose's `animateItem`): fades it in along `fade_in_spec` when it appears, glides it
    /// along `placement_spec` when it moves (items before it removed, inserted, moved; not a scroll), fades it out
    /// along `fade_out_spec` where it was when it goes. None of a spec: not that animation. On the item's first
    /// layout (its root) in a lazy layout (grids too); nothing elsewhere.
    fn animate_item(
        self,
        #[default(default_fade_spec())] fade_in_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
        #[default(default_placement_spec())] placement_spec: Option<Arc<dyn FiniteAnimationSpec<IntOffset>>>,
        #[default(default_fade_spec())] fade_out_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
    ) -> Modifier {
        if fade_in_spec.is_none() && placement_spec.is_none() && fade_out_spec.is_none() {
            self
        } else {
            self.then_element(LazyLayoutAnimateItemElement(LazyLayoutAnimationSpecs {
                fade_in_spec,
                placement_spec,
                fade_out_spec,
            }))
        }
    }
}

#[derive(PartialEq)]
struct ParentSizeElement {
    fraction: f32,
    width: bool,
    height: bool,
    name: &'static str,
}

impl ModifierNodeElement for ParentSizeElement {
    type Node = ParentSizeNode;
    fn create(&self) -> ParentSizeNode {
        ParentSizeNode { fraction: self.fraction, width: self.width, height: self.height, cx: None }
    }
    fn update(&self, node: &mut ParentSizeNode) {
        node.fraction = self.fraction;
        node.width = self.width;
        node.height = self.height;
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

/// Compose's ParentSizeNode: the item's constraints fixed to a fraction of the list's viewport (read as a state:
/// measured again as it changes); outside a list, to the constraints' maximum.
struct ParentSizeNode {
    fraction: f32,
    width: bool,
    height: bool,
    cx: Option<NodeCx>,
}

impl ModifierNode for ParentSizeNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.cx = None;
    }
}

impl LayoutModifierNode for ParentSizeNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let item_scope = self.cx.as_ref().and_then(|cx| cx.current_value_of(&LocalLazyItemScope));
        let fraction_of = |max: i32| {
            if max != Constraints::Infinity {
                (max as f32 * self.fraction + 0.5).floor() as i32
            } else {
                Constraints::Infinity
            }
        };
        let (width, height) = match &item_scope {
            Some(s) => (
                if self.width { fraction_of(s.max_width_state.get()) } else { Constraints::Infinity },
                if self.height { fraction_of(s.max_height_state.get()) } else { Constraints::Infinity },
            ),
            // (outside a list: the constraints' maximum, as fill_max_* does)
            None => (
                if self.width { fraction_of(constraints.max_width) } else { Constraints::Infinity },
                if self.height { fraction_of(constraints.max_height) } else { Constraints::Infinity },
            ),
        };
        let child_constraints = Constraints::new(
            if width != Constraints::Infinity { width } else { constraints.min_width },
            if width != Constraints::Infinity { width } else { constraints.max_width },
            if height != Constraints::Infinity { height } else { constraints.min_height },
            if height != Constraints::Infinity { height } else { constraints.max_height },
        );
        let placeable = measurable.measure(child_constraints);
        scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
    }
}
