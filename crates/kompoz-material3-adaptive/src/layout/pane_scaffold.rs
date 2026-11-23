//! What pane scaffolds share, as Compose's adaptive-layout PaneScaffold.kt (1.2.0): the scopes their panes are
//! composed in ([`PaneScaffoldScope`]: a pane's preferred width and height, the drag handle's modifier, the
//! saveable state holder; [`PaneScaffoldTransitionScope`]: the transition of the scaffold's values and its
//! progress; [`PaneScaffoldPaneScope`]: the pane's role, motion and whether it takes interaction), and the parent
//! data a scaffold measures its panes by ([`PaneScaffoldParentData`]).

use super::pane_expansion_state::PaneExpansionState;
use super::pane_motion::PaneMotion;
use super::pane_motion::PaneScaffoldMotionDataProvider;
use super::three_pane_scaffold_value::PaneScaffoldValue;
use kompoz_animation::Transition;
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::{SaveableStateHolder, named};
use kompoz_ui::layout::{Measurable, ParentDataModifierNode};
use kompoz_ui::semantics::SemanticsConfiguration;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::any::Any;
use std::rc::Rc;

/// A semantics block a drag handle's semantics are given by (Compose's `SemanticsPropertyReceiver.() -> Unit`).
pub type SemanticsProperties = Rc<dyn Fn(&mut SemanticsConfiguration)>;

/// The scope of a pane scaffold's panes (Compose's sealed interface `PaneScaffoldScope`): a pane's preferred size,
/// the pane expansion drag handle's modifier, the holder of the panes' saveable state.
pub trait PaneScaffoldScope {
    /// `modifier` with the pane's preferred width (Compose's `Modifier.preferredWidth(width)`): `Dp::Unspecified`
    /// for the scaffold's default; otherwise above 0 (Kotlin's `require`: panics).
    fn preferred_width(&self, modifier: Modifier, width: Dp) -> Modifier {
        assert!(width.is_unspecified() || width > Dp(0.0), "invalid width");
        modifier.then_element(PreferredWidthElement { width: PreferredSize::of_dp(width) })
    }

    /// `modifier` with the pane's preferred width as a proportion of the scaffold's, 0 to 1 (Compose's
    /// `Modifier.preferredWidth(proportion)`).
    ///
    /// Kotlin: `preferredWidth(proportion: Float): Modifier`
    fn preferred_width_proportion(&self, modifier: Modifier, proportion: f32) -> Modifier {
        assert!((0.0..=1.0).contains(&proportion), "invalid width proportion");
        modifier.then_element(PreferredWidthElement { width: PreferredSize::of_proportion(proportion) })
    }

    /// `modifier` with the pane's preferred height (Compose's `Modifier.preferredHeight(height)`), used when the
    /// pane is levitated or reflowed.
    fn preferred_height(&self, modifier: Modifier, height: Dp) -> Modifier {
        assert!(height.is_unspecified() || height > Dp(0.0), "invalid height");
        modifier.then_element(PreferredHeightElement { height: PreferredSize::of_dp(height) })
    }

    /// `modifier` with the pane's preferred height as a proportion of the scaffold's (Compose's
    /// `Modifier.preferredHeight(proportion)`).
    ///
    /// Kotlin: `preferredHeight(proportion: Float): Modifier`
    fn preferred_height_proportion(&self, modifier: Modifier, proportion: f32) -> Modifier {
        // (Compose's message says "width" here too)
        assert!((0.0..=1.0).contains(&proportion), "invalid width proportion");
        modifier.then_element(PreferredHeightElement { height: PreferredSize::of_proportion(proportion) })
    }

    /// `modifier` making its layout the drag handle of `state`: dragged (and clicked, by its semantics) to move
    /// the split, `min_touch_target_size` at least to touch, `interaction_source` hearing the drags,
    /// `semantics_properties` its semantics (Compose's `Modifier.paneExpansionDraggable`).
    ///
    /// Kotlin: `paneExpansionDraggable(state: PaneExpansionState, minTouchTargetSize: Dp, interactionSource: MutableInteractionSource, semanticsProperties: (SemanticsPropertyReceiver.() -> Unit)? = null): Modifier`
    fn pane_expansion_draggable(
        &self,
        modifier: Modifier,
        state: PaneExpansionState,
        min_touch_target_size: Dp,
        interaction_source: MutableInteractionSource,
        semantics_properties: Option<SemanticsProperties>,
    ) -> Modifier;

    /// The holder of the panes' saveable state (Compose's `saveableStateHolder`): each pane's kept while it is
    /// hidden.
    fn saveable_state_holder(&self) -> SaveableStateHolder;
}

/// [`PaneScaffoldScope::pane_expansion_draggable`] with Kotlin's default: no semantics of its own (the default drag
/// handle semantics then), so Kotlin can leave `semanticsProperties` out — a trait's member has no defaults.
#[named(trait PaneScaffoldScopeDraggable)]
impl<S: PaneScaffoldScope + ?Sized> S {
    /// `modifier` making its layout the drag handle of `state` (Compose's `Modifier.paneExpansionDraggable`, its
    /// `semanticsProperties` defaulting to `null`).
    ///
    /// Kotlin: `paneExpansionDraggable(state: PaneExpansionState, minTouchTargetSize: Dp, interactionSource: MutableInteractionSource, semanticsProperties: (SemanticsPropertyReceiver.() -> Unit)? = null): Modifier`
    fn pane_expansion_draggable_of(
        &self,
        modifier: Modifier,
        state: PaneExpansionState,
        min_touch_target_size: Dp,
        interaction_source: MutableInteractionSource,
        #[default(None)] semantics_properties: Option<SemanticsProperties>,
    ) -> Modifier {
        self.pane_expansion_draggable(modifier, state, min_touch_target_size, interaction_source, semantics_properties)
    }
}

/// The transition of a pane scaffold's values (Compose's sealed interface `PaneScaffoldTransitionScope`).
pub trait PaneScaffoldTransitionScope {
    /// The scaffold's value type.
    type ScaffoldValue: PaneScaffoldValue;
    /// Its motion data provider's type.
    type MotionDataProvider: PaneScaffoldMotionDataProvider<Role = <Self::ScaffoldValue as PaneScaffoldValue>::Role>;

    /// The current transition between the scaffold's values.
    fn scaffold_state_transition(&self) -> Transition<Self::ScaffoldValue>;

    /// The current motion progress, 0 to 1 (1 when not moving).
    fn motion_progress(&self) -> f32;

    /// What the scaffold knows of its panes' motion.
    fn motion_data_provider(&self) -> Rc<Self::MotionDataProvider>;
}

/// What a pane's scope knows of the pane (Compose's sealed interface `PaneScaffoldPaneScope`).
pub trait PaneScaffoldPaneScope {
    /// The role of the pane's.
    type Role;

    /// The pane's role.
    fn pane_role(&self) -> Self::Role;

    /// The pane's motion of the current change.
    fn pane_motion(&self) -> PaneMotion;

    /// Whether the pane takes the user's interaction (shown, and not under a levitated pane's scrim).
    fn is_interactable(&self) -> bool;
}

// ---- Preferred sizes, parent data ----

/// A preferred size: in dp, or a proportion of the scaffold's (Compose's internal data class `PreferredSize`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PreferredSize {
    pub(crate) dp: Dp,
    pub(crate) proportion: f32,
}

/// Kotlin's data class `equals`: floats by `Float.equals` (NaN equal to NaN).
impl PartialEq for PreferredSize {
    fn eq(&self, other: &Self) -> bool {
        self.dp.0.to_bits() == other.dp.0.to_bits() && self.proportion.to_bits() == other.proportion.to_bits()
    }
}

impl PreferredSize {
    pub(crate) const Unspecified: PreferredSize = PreferredSize { dp: Dp::Unspecified, proportion: f32::NAN };

    fn of_dp(dp: Dp) -> PreferredSize {
        PreferredSize { dp, proportion: f32::NAN }
    }

    fn of_proportion(proportion: f32) -> PreferredSize {
        PreferredSize { dp: Dp::Unspecified, proportion }
    }
}

/// What a pane scaffold measures a pane by (Compose's sealed interface `PaneScaffoldParentData`).
pub trait PaneScaffoldParentData {
    /// The preferred width in dp (`Dp::Unspecified`: none).
    fn preferred_width(&self) -> Dp;
    /// The preferred height in dp.
    fn preferred_height(&self) -> Dp;
    /// The preferred width as a proportion of the scaffold's (NaN: none).
    fn preferred_width_in_proportion(&self) -> f32;
    /// The preferred height as a proportion of the scaffold's.
    fn preferred_height_in_proportion(&self) -> f32;
    /// Whether the pane is an [`AnimatedPane`](crate::layout::ThreePaneScaffoldPaneScope::AnimatedPane).
    fn is_animated_pane(&self) -> bool;
    /// The least size to touch a drag handle by (`Dp::Unspecified`: none).
    fn min_touch_target_size(&self) -> Dp;
}

/// The parent data the scaffold's modifiers set (Compose's internal data class `PaneScaffoldParentDataImpl`).
#[derive(Clone, Debug)]
pub(crate) struct PaneScaffoldParentDataImpl {
    pub(crate) preferred_width_internal: PreferredSize,
    pub(crate) preferred_height_internal: PreferredSize,
    pub(crate) is_animated_pane: bool,
    pub(crate) min_touch_target_size: Dp,
    /// (WP8 helper) Where the pane's edges may go (Compose's `paneMargins`).
    pub(crate) pane_margins: Rc<dyn super::pane_margins::PaneMargins>,
}

impl Default for PaneScaffoldParentDataImpl {
    fn default() -> Self {
        PaneScaffoldParentDataImpl {
            preferred_width_internal: PreferredSize::Unspecified,
            preferred_height_internal: PreferredSize::Unspecified,
            is_animated_pane: false,
            min_touch_target_size: Dp::Unspecified,
            pane_margins: super::pane_margins::unspecified_pane_margins(),
        }
    }
}

impl PaneScaffoldParentData for PaneScaffoldParentDataImpl {
    fn preferred_width(&self) -> Dp {
        self.preferred_width_internal.dp
    }
    fn preferred_height(&self) -> Dp {
        self.preferred_height_internal.dp
    }
    fn preferred_width_in_proportion(&self) -> f32 {
        self.preferred_width_internal.proportion
    }
    fn preferred_height_in_proportion(&self) -> f32 {
        self.preferred_height_internal.proportion
    }
    fn is_animated_pane(&self) -> bool {
        self.is_animated_pane
    }
    fn min_touch_target_size(&self) -> Dp {
        self.min_touch_target_size
    }
}

/// The parent data the nodes after one said, as the scaffold's (Kotlin's `(parentData as?
/// PaneScaffoldParentDataImpl) ?: PaneScaffoldParentDataImpl()`), changed by `change`.
pub(crate) fn modify_pane_scaffold_parent_data(
    parent_data: Option<Box<dyn Any>>,
    change: impl FnOnce(&mut PaneScaffoldParentDataImpl),
) -> Option<Box<dyn Any>> {
    let mut data =
        parent_data.and_then(|d| d.downcast::<PaneScaffoldParentDataImpl>().ok()).map(|d| *d).unwrap_or_default();
    change(&mut data);
    Some(Box::new(data))
}

/// A measurable's pane parent data, if it has one.
pub(crate) fn pane_scaffold_parent_data(measurable: &dyn Measurable) -> Option<PaneScaffoldParentDataImpl> {
    measurable.parent_data().and_then(|d| d.downcast_ref::<PaneScaffoldParentDataImpl>().cloned())
}

/// The least size to touch the measurable's drag handle by, 0 if it says none (Compose's internal
/// `Measurable.minTouchTargetSize`).
pub(crate) fn min_touch_target_size(measurable: &dyn Measurable) -> Dp {
    let size = pane_scaffold_parent_data(measurable).map_or(Dp::Unspecified, |d| d.min_touch_target_size);
    if size.is_specified() { size } else { Dp(0.0) }
}

#[derive(PartialEq)]
struct PreferredWidthElement {
    width: PreferredSize,
}

impl ModifierNodeElement for PreferredWidthElement {
    type Node = PreferredWidthNode;
    fn create(&self) -> PreferredWidthNode {
        PreferredWidthNode { width: self.width }
    }
    fn update(&self, node: &mut PreferredWidthNode) {
        node.width = self.width;
    }
    fn name(&self) -> &'static str {
        "preferredWidth"
    }
}

struct PreferredWidthNode {
    width: PreferredSize,
}

impl ModifierNode for PreferredWidthNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for PreferredWidthNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let width = self.width;
        modify_pane_scaffold_parent_data(parent_data, |d| d.preferred_width_internal = width)
    }
}

#[derive(PartialEq)]
struct PreferredHeightElement {
    height: PreferredSize,
}

impl ModifierNodeElement for PreferredHeightElement {
    type Node = PreferredHeightNode;
    fn create(&self) -> PreferredHeightNode {
        PreferredHeightNode { height: self.height }
    }
    fn update(&self, node: &mut PreferredHeightNode) {
        node.height = self.height;
    }
    fn name(&self) -> &'static str {
        "preferredHeight"
    }
}

struct PreferredHeightNode {
    height: PreferredSize,
}

impl ModifierNode for PreferredHeightNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for PreferredHeightNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let height = self.height;
        modify_pane_scaffold_parent_data(parent_data, |d| d.preferred_height_internal = height)
    }
}

/// `modifier` marking its layout an animated pane (Compose's internal `Modifier.animatedPane`).
pub(crate) fn animated_pane(modifier: Modifier) -> Modifier {
    modifier.then_element(AnimatedPaneElement)
}

#[derive(PartialEq)]
struct AnimatedPaneElement;

impl ModifierNodeElement for AnimatedPaneElement {
    type Node = AnimatedPaneNode;
    fn create(&self) -> AnimatedPaneNode {
        AnimatedPaneNode
    }
    fn update(&self, _: &mut AnimatedPaneNode) {}
    fn name(&self) -> &'static str {
        "isPaneComposable"
    }
}

struct AnimatedPaneNode;

impl ModifierNode for AnimatedPaneNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for AnimatedPaneNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        modify_pane_scaffold_parent_data(parent_data, |d| d.is_animated_pane = true)
    }
}
