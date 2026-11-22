//! Material Expressive's button groups, as Compose's ButtonGroup.kt: a row of buttons that grows the one pressed
//! and squeezes its neighbours ([`ButtonGroup`], its items put into an overflow menu when they don't all fit, and the
//! deprecated [`ButtonGroupWithoutOverflowIndicator`], whose content is composed as written), what the content says
//! to the group ([`ButtonGroupScope`]: a child's weight, the interaction source it grows with, its vertical
//! alignment, the items of the group), the state of the overflow menu ([`ButtonGroupMenuState`]) and the defaults
//! ([`ButtonGroupDefaults`]: the spacing, the ratio a pressed button grows by, the overflow indicator and the shapes
//! of a connected group's buttons).
//!
//! Compose's two overloads Kotlin source can call are two composables here: `ButtonGroup(overflowIndicator, …)` is
//! [`ButtonGroup`], the deprecated `ButtonGroup(modifier, expandedRatio, horizontalArrangement, content)`
//! [`ButtonGroupWithoutOverflowIndicator`]. The scope's modifiers (Kotlin's member extensions `Modifier.weight(…)`,
//! `Modifier.animateWidth(…)`, `Modifier.align(…)`) are its methods taking the modifier they extend.

// (the deprecated overload's own generated items name it)
#![allow(deprecated)]

use crate::button::{Button, ButtonDefaults};
use crate::icon::VectorIcon;
use crate::icon_button::{FilledIconButton, IconButtonColors};
use crate::icon_button_defaults::IconButtonDefaults;
use crate::internal::icons::Icons;
use crate::internal::strings::{Strings, get_string};
use crate::menu_desktop::{DropdownMenu, DropdownMenuItem};
use crate::shapes::ShapeDefaults;
use crate::text::Text;
use crate::toggle_button::{ToggleButton, ToggleButtonShapes};
use crate::tokens::{ButtonGroupSmallTokens, ConnectedButtonGroupSmallTokens, MotionSchemeKeyTokens, ShapeTokens};
use crate::tooltip::{
    TooltipAnchorPosition, TooltipBox, TooltipDefaults, TooltipScope, TooltipScopeComposables, remember_tooltip_state,
};
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector1D};
use kompoz_coroutines::{Job, with_frame_millis};
use kompoz_foundation::interaction::{InteractionSource, MutableInteractionSource, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Density, Dp, LayoutDirection, RoundedCornerShape, Shape};
use kompoz_runtime::{
    DerivedState, MutableState, SavedValue, Saver, State, derived_state_of_with_policy, kompozable, kompozable_block,
    mutable_int_state_of, mutable_state_of, named, named_args, never_equal_policy, remember, remember_keyed,
    remember_saveable_with, remember_updated_state,
};
use kompoz_ui::layout::{
    MultiContentLayout, MultiContentMeasurePolicy, ParentDataModifierNode, Placeable, layout_into,
};
use kompoz_ui::{
    Alignment, Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier, ModifierNode,
    ModifierNodeElement, NodeCx, NodeHandle, VerticalAlignment, node_capabilities,
};
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- ButtonGroup -----------------------------------------------------------------------------------------------

/// A row of buttons that grows the one pressed and squeezes its neighbours, its content composed as written (Compose's
/// deprecated `ButtonGroup(modifier, expandedRatio, horizontalArrangement, content)`: it cuts off what doesn't fit —
/// [`ButtonGroup`] puts that into a menu). A child given [`ButtonGroupScope::animate_width`] with its interaction
/// source grows by `expanded_ratio` of its width while pressed (0: not at all, 1: to twice its width), taking that
/// from its neighbours (half from each for a middle one); its children are `horizontal_arrangement`'d, each as wide
/// as its widest content unless weighted ([`ButtonGroupScope::weight`]), at the top.
///
/// Kotlin: `ButtonGroup(modifier: Modifier = Modifier, expandedRatio: Float = ButtonGroupDefaults.ExpandedRatio, horizontalArrangement: Arrangement.Horizontal = ButtonGroupDefaults.HorizontalArrangement, content: @Composable ButtonGroupScope.() -> Unit)`
#[deprecated(
    note = "Please use the overload with overflowIndicator parameter. This overload will create a composable that is cut off if there are too many items to fit on the screen neatly."
)]
#[kompozable]
pub fn ButtonGroupWithoutOverflowIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(ButtonGroupDefaults::ExpandedRatio)] expanded_ratio: f32,
    #[default(ButtonGroupDefaults::HorizontalArrangement())] horizontal_arrangement: HorizontalArrangement,
    content: impl Fn(&ButtonGroupScope) + 'static,
) {
    // TODO Load the motionScheme tokens from the component tokens file
    let default_animation_spec: Arc<dyn AnimationSpec<f32>> = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let scope = remember(move || ButtonGroupScope::new(default_animation_spec));

    // (Compose's `remember(horizontalArrangement)`: a new expanded ratio alone keeps the policy made first)
    let arrangement = horizontal_arrangement.clone();
    let measure_policy = remember_keyed(horizontal_arrangement, move || {
        Rc::new(NonAdaptiveButtonGroupMeasurePolicy { horizontal_arrangement: arrangement, expanded_ratio })
            as Rc<dyn MeasurePolicy>
    });

    layout_into(modifier, measure_policy, false, move || content(&scope));
}

/// A row of buttons that grows the one pressed and squeezes its neighbours, what doesn't fit put into a menu
/// (Compose's `ButtonGroup(overflowIndicator, modifier, expandedRatio, horizontalArrangement, verticalAlignment,
/// content)`). `content` says the group's items ([`ButtonGroupScope::clickable_item`], `toggleable_item`,
/// `custom_item`; it is run again when a state it reads changes); they are shown in a row, `horizontal_arrangement`'d,
/// each as wide as its widest content unless weighted, `vertical_alignment`'d (unless an item says otherwise); those
/// that don't fit go into a [`DropdownMenu`] that `overflow_indicator` (shown at the end of the row, then) opens
/// with the [`ButtonGroupMenuState`] it is given. An item growing as it is pressed grows by `expanded_ratio` of its
/// width (0: not at all, 1: to twice its width), taking that from its neighbours.
///
/// Kotlin: `ButtonGroup(overflowIndicator: @Composable (ButtonGroupMenuState) -> Unit, modifier: Modifier = Modifier, expandedRatio: Float = ButtonGroupDefaults.ExpandedRatio, horizontalArrangement: Arrangement.Horizontal = ButtonGroupDefaults.HorizontalArrangement, verticalAlignment: Alignment.Vertical = Alignment.Top, content: ButtonGroupScope.() -> Unit)`
#[kompozable]
pub fn ButtonGroup(
    overflow_indicator: impl Fn(&ButtonGroupMenuState) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(ButtonGroupDefaults::ExpandedRatio)] expanded_ratio: f32,
    #[default(ButtonGroupDefaults::HorizontalArrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(Alignment::Top)] vertical_alignment: VerticalAlignment,
    content: impl Fn(&ButtonGroupScope) + 'static,
) {
    // TODO Load the motionScheme tokens from the component tokens file
    let default_animation_spec: Arc<dyn AnimationSpec<f32>> = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let scope = remember_button_group_scope_state(Rc::new(content), default_animation_spec);
    let menu_state = remember(|| ButtonGroupMenuState::new(false));
    let overflow_state = remember_overflow_state();

    // (Compose's `remember(horizontalArrangement, overflowState)`: the vertical alignment and the expanded ratio
    // are those of the policy made first while these stay the same)
    let state = overflow_state.clone();
    let arrangement = horizontal_arrangement.clone();
    let measure_policy =
        remember_keyed((horizontal_arrangement, OverflowStateKey(overflow_state.clone())), move || {
            Rc::new(ButtonGroupMeasurePolicy {
                overflow_state: state,
                horizontal_arrangement: arrangement,
                vertical_alignment,
                expanded_ratio,
            }) as Rc<dyn MultiContentMeasurePolicy>
        });

    let overflow_indicator: Rc<dyn Fn(&ButtonGroupMenuState)> = Rc::new(overflow_indicator);
    let items_scope = scope.clone();
    MultiContentLayout(
        contents = vec![
            Rc::new(move || ButtonGroupItems(items_scope.clone())) as Rc<dyn Fn()>,
            Rc::new(move || {
                let (overflow_indicator, menu_state, scope, overflow_state) =
                    (overflow_indicator.clone(), menu_state.clone(), scope.clone(), overflow_state.clone());
                Box(content = move || {
                    overflow_indicator(&menu_state);
                    let dismissed = menu_state.clone();
                    let (menu, scope, overflow_state) = (menu_state.clone(), scope.clone(), overflow_state.clone());
                    DropdownMenu(
                        expanded = menu_state.is_showing(),
                        on_dismiss_request = move || dismissed.dismiss(),
                        content = move || {
                            let scope = scope.value();
                            let items = scope.items();
                            for i in overflow_state.visible_item_count()..overflow_state.total_item_count() {
                                items[i as usize].MenuContent(&menu);
                            }
                        },
                    );
                });
            }) as Rc<dyn Fn()>,
        ],
        measure_policy = measure_policy,
        modifier = modifier,
    );
}

/// The group's items in the row (Compose's first content lambda, `{ scope.items.fastForEach { it.ButtonGroupContent()
/// } }`, a restartable scope of its own: a change of the items composes it again, not the group).
#[kompozable]
fn ButtonGroupItems(scope: State<ButtonGroupScope>) {
    for item in scope.value().items().iter() {
        item.ButtonGroupContent();
    }
}

// ---- ButtonGroupDefaults ---------------------------------------------------------------------------------------

/// Default values used by [`ButtonGroup`] (Compose's `ButtonGroupDefaults`; a unit value: its composables and
/// functions with defaults are methods, `ButtonGroupDefaults.connected_leading_button_shapes()`).
#[derive(Clone, Copy, Debug)]
pub struct ButtonGroupDefaults;

impl ButtonGroupDefaults {
    /// The default share of a pressed child's width it grows by, taken from its neighbours: 15%.
    pub const ExpandedRatio: f32 = 0.15;

    /// The default spacing between a connected group's children, 2 dp.
    pub const ConnectedSpaceBetween: Dp = ConnectedButtonGroupSmallTokens::BetweenSpace;

    /// The default arrangement between a standard group's children: spaced by 12 dp (Compose's
    /// `HorizontalArrangement`).
    #[allow(non_snake_case)]
    pub fn HorizontalArrangement() -> HorizontalArrangement {
        Arrangement::spaced_by(ButtonGroupSmallTokens::BetweenSpace)
    }

    /// The default shape of a connected group's leading button: fully round at the start, small corners at the end.
    pub fn connected_leading_button_shape() -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(
            ShapeDefaults::CornerFull,
            ConnectedButtonGroupSmallTokens::InnerCornerCornerSize,
            ConnectedButtonGroupSmallTokens::InnerCornerCornerSize,
            ShapeDefaults::CornerFull,
        ))
    }

    /// The default shape of a connected group's leading button while pressed: extra small corners at the end.
    pub fn connected_leading_button_press_shape() -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(
            ShapeDefaults::CornerFull,
            ConnectedButtonGroupSmallTokens::PressedInnerCornerCornerSize,
            ConnectedButtonGroupSmallTokens::PressedInnerCornerCornerSize,
            ShapeDefaults::CornerFull,
        ))
    }

    /// The default shape of a connected group's trailing button: small corners at the start, fully round at the end.
    pub fn connected_trailing_button_shape() -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(
            ConnectedButtonGroupSmallTokens::InnerCornerCornerSize,
            ShapeDefaults::CornerFull,
            ShapeDefaults::CornerFull,
            ConnectedButtonGroupSmallTokens::InnerCornerCornerSize,
        ))
    }

    /// The default shape of a connected group's trailing button while pressed: extra small corners at the start.
    pub fn connected_trailing_button_press_shape() -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(
            ConnectedButtonGroupSmallTokens::PressedInnerCornerCornerSize,
            ShapeDefaults::CornerFull,
            ShapeDefaults::CornerFull,
            ConnectedButtonGroupSmallTokens::PressedInnerCornerCornerSize,
        ))
    }

    /// The default shape of a connected group's checked buttons: fully round (Compose's `connectedButtonCheckedShape
    /// = ShapeTokens.CornerFull`, a `RoundedCornerShape`).
    pub fn connected_button_checked_shape() -> RoundedCornerShape {
        ShapeTokens::CornerFull
    }

    /// The default shape of a connected group's middle buttons while pressed: extra small corners.
    pub fn connected_middle_button_press_shape() -> Arc<dyn Shape> {
        Arc::new(kompoz_graphics::RoundedCornerShape(ConnectedButtonGroupSmallTokens::PressedInnerCornerCornerSize))
    }
}

#[named]
impl ButtonGroupDefaults {
    /// The shapes of a connected group's start button (Compose's `connectedLeadingButtonShapes`).
    ///
    /// Kotlin: `connectedLeadingButtonShapes(shape: Shape = connectedLeadingButtonShape, pressedShape: Shape = connectedLeadingButtonPressShape, checkedShape: Shape = connectedButtonCheckedShape): ToggleButtonShapes`
    pub fn connected_leading_button_shapes(
        &self,
        #[default(ButtonGroupDefaults::connected_leading_button_shape())] shape: Arc<dyn Shape>,
        #[default(ButtonGroupDefaults::connected_leading_button_press_shape())] pressed_shape: Arc<dyn Shape>,
        #[default(Arc::new(ButtonGroupDefaults::connected_button_checked_shape()))] checked_shape: Arc<dyn Shape>,
    ) -> ToggleButtonShapes {
        ToggleButtonShapes::new(shape, pressed_shape, checked_shape)
    }

    /// The shapes of a connected group's middle button, one neither at the start nor at the end (Compose's
    /// `connectedMiddleButtonShapes`).
    ///
    /// Kotlin: `connectedMiddleButtonShapes(shape: Shape = ShapeDefaults.Small, pressedShape: Shape = connectedMiddleButtonPressShape, checkedShape: Shape = connectedButtonCheckedShape): ToggleButtonShapes`
    pub fn connected_middle_button_shapes(
        &self,
        #[default(Arc::new(ShapeDefaults::Small()))] shape: Arc<dyn Shape>,
        #[default(ButtonGroupDefaults::connected_middle_button_press_shape())] pressed_shape: Arc<dyn Shape>,
        #[default(Arc::new(ButtonGroupDefaults::connected_button_checked_shape()))] checked_shape: Arc<dyn Shape>,
    ) -> ToggleButtonShapes {
        ToggleButtonShapes::new(shape, pressed_shape, checked_shape)
    }

    /// The shapes of a connected group's end button (Compose's `connectedTrailingButtonShapes`).
    ///
    /// Kotlin: `connectedTrailingButtonShapes(shape: Shape = connectedTrailingButtonShape, pressedShape: Shape = connectedTrailingButtonPressShape, checkedShape: Shape = connectedButtonCheckedShape): ToggleButtonShapes`
    pub fn connected_trailing_button_shapes(
        &self,
        #[default(ButtonGroupDefaults::connected_trailing_button_shape())] shape: Arc<dyn Shape>,
        #[default(ButtonGroupDefaults::connected_trailing_button_press_shape())] pressed_shape: Arc<dyn Shape>,
        #[default(Arc::new(ButtonGroupDefaults::connected_button_checked_shape()))] checked_shape: Arc<dyn Shape>,
    ) -> ToggleButtonShapes {
        ToggleButtonShapes::new(shape, pressed_shape, checked_shape)
    }

    /// The default overflow indicator of a [`ButtonGroup`]: a [`FilledIconButton`] showing three dots ("More
    /// options", told to accessibility and shown in a tooltip above it) that opens the menu of `menu_state`, or
    /// closes it if it is showing. Composable.
    ///
    /// Kotlin: `OverflowIndicator(menuState: ButtonGroupMenuState, modifier: Modifier = Modifier, enabled: Boolean = true, shape: Shape = IconButtonDefaults.filledShape, colors: IconButtonColors = IconButtonDefaults.filledIconButtonColors(), interactionSource: MutableInteractionSource? = null)`
    #[track_caller]
    #[allow(non_snake_case)]
    pub fn OverflowIndicator(
        &self,
        menu_state: &ButtonGroupMenuState,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(IconButtonDefaults::filled_shape())] shape: Arc<dyn Shape>,
        #[default(IconButtonDefaults.filled_icon_button_colors())] colors: IconButtonColors,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
    ) {
        kompozable_block!({
            OverflowIndicatorImpl(menu_state.clone(), modifier, enabled, shape, colors, interaction_source);
        });
    }
}

/// [`ButtonGroupDefaults::OverflowIndicator`]'s body.
#[kompozable]
fn OverflowIndicatorImpl(
    menu_state: ButtonGroupMenuState,
    modifier: Modifier,
    enabled: bool,
    shape: Arc<dyn Shape>,
    colors: IconButtonColors,
    interaction_source: Option<MutableInteractionSource>,
) {
    let content_description = get_string(Strings::ButtonGroupMoreOptions);

    let tooltip_text = content_description.clone();
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |scope: &dyn TooltipScope| {
            let text = tooltip_text.clone();
            scope.PlainTooltip(content = move || Text(text.clone()));
        },
        state = remember_tooltip_state(),
        content = move || {
            let (menu_state, description) = (menu_state.clone(), content_description.clone());
            FilledIconButton(
                on_click = move || {
                    if menu_state.is_showing() {
                        menu_state.dismiss();
                    } else {
                        menu_state.show();
                    }
                },
                modifier = modifier.clone(),
                enabled = enabled,
                shape = shape.clone(),
                colors = colors,
                interaction_source = interaction_source.clone(),
                content = move || VectorIcon(Icons::Filled::MoreVert(), Some(description.clone())),
            );
        },
    );
}

// ---- ButtonGroupMenuState --------------------------------------------------------------------------------------

/// The state of a [`ButtonGroup`]'s overflow menu (Compose's `ButtonGroupMenuState`): whether it is showing (a
/// state). Clones are the same state.
#[derive(Clone)]
pub struct ButtonGroupMenuState {
    is_showing: MutableState<bool>,
}

impl ButtonGroupMenuState {
    /// A menu state, showing if `initial_is_showing`.
    pub fn new(initial_is_showing: bool) -> ButtonGroupMenuState {
        ButtonGroupMenuState { is_showing: mutable_state_of(initial_is_showing) }
    }

    /// Whether the overflow menu is showing (Compose's `isShowing`; a state read).
    pub fn is_showing(&self) -> bool {
        self.is_showing.get()
    }

    /// Closes the overflow menu.
    pub fn dismiss(&self) {
        self.is_showing.set(false);
    }

    /// Shows the overflow menu.
    pub fn show(&self) {
        self.is_showing.set(true);
    }
}

impl PartialEq for ButtonGroupMenuState {
    /// The same state (Kotlin's identity of a class without `equals`).
    fn eq(&self, other: &Self) -> bool {
        self.is_showing == other.is_showing
    }
}

impl fmt::Debug for ButtonGroupMenuState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ButtonGroupMenuState(isShowing={})", self.is_showing.peek())
    }
}

/// Compose's `ButtonGroupMenuState(initialIsShowing = false)`.
///
/// Kotlin: `ButtonGroupMenuState(initialIsShowing: Boolean = false)`
#[named(type)]
pub fn ButtonGroupMenuState(#[default(false)] initial_is_showing: bool) -> ButtonGroupMenuState {
    ButtonGroupMenuState::new(initial_is_showing)
}

// ---- The measure policies --------------------------------------------------------------------------------------

/// Java's `Math.round(float)` (Kotlin's `roundToInt`, Compose's `fastRoundToInt`): half up.
fn round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// What a child of the group says (Compose's `ButtonGroupParentData`): its weight, the value its press animates
/// (0 to 1), its vertical alignment.
#[derive(Clone)]
pub(crate) struct ButtonGroupParentData {
    pub(crate) weight: f32,
    pub(crate) pressed_animatable: Animatable<f32, AnimationVector1D>,
    pub(crate) alignment: Option<VerticalAlignment>,
}

impl Default for ButtonGroupParentData {
    fn default() -> Self {
        ButtonGroupParentData { weight: 0.0, pressed_animatable: Animatable(0.0f32), alignment: None }
    }
}

impl PartialEq for ButtonGroupParentData {
    /// Kotlin's data class equality (the Animatable by identity).
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
            && self.pressed_animatable == other.pressed_animatable
            && self.alignment == other.alignment
    }
}

/// A child's parent data of the group, if it is one (Compose's `IntrinsicMeasurable.buttonGroupParentData`).
fn button_group_parent_data(parent_data: Option<Rc<dyn Any>>) -> Option<ButtonGroupParentData> {
    parent_data?.downcast_ref::<ButtonGroupParentData>().cloned()
}

/// The parent data a parent-data node is given, if it is the group's.
fn previous_parent_data(parent_data: Option<Box<dyn Any>>) -> Option<ButtonGroupParentData> {
    parent_data.and_then(|data| data.downcast::<ButtonGroupParentData>().ok()).map(|data| *data)
}

/// Its weight, 0 for none (Compose's `ButtonGroupParentData?.weight`).
fn weight_of(data: &Option<ButtonGroupParentData>) -> f32 {
    data.as_ref().map_or(0.0, |d| d.weight)
}

/// The weights' part of both policies: each child's constraints (`None` until given), its main axis size, the
/// fixed space and the weighted space (Compose's first half of `measure`, the same in both policies).
struct WeightedSizes {
    children_constraints: Vec<Option<Constraints>>,
    children_main_axis_size: Vec<i32>,
    fixed_space: i32,
    weighted_space: i32,
}

fn weighted_sizes(
    measurables: &[&dyn Measurable],
    constraints: Constraints,
    arrangement_spacing_int: i32,
) -> WeightedSizes {
    let arrangement_spacing_px = arrangement_spacing_int as i64;
    let size = measurables.len();
    let mut total_weight = 0.0f32;
    let mut fixed_space = 0i32;
    let mut weight_children_count = 0i64;
    let mut children_main_axis_size = vec![0i32; size];
    let mut children_constraints: Vec<Option<Constraints>> = vec![None; size];

    let main_axis_min = constraints.min_width;
    let main_axis_max = constraints.max_width;

    // First obtain constraints of children with zero weight
    let mut space_after_last_no_weight = 0;
    for (i, child) in measurables.iter().enumerate() {
        let parent_data = button_group_parent_data(child.parent_data());
        let weight = weight_of(&parent_data);

        if weight > 0.0 {
            total_weight += weight;
            weight_children_count += 1;
        } else {
            let remaining = main_axis_max.wrapping_sub(fixed_space);
            let desired_width = child.max_intrinsic_width(constraints.max_height);
            children_constraints[i] =
                Some(named_args!(constraints.copy(min_width = 0, max_width = desired_width.max(0))));

            children_main_axis_size[i] = desired_width;

            space_after_last_no_weight = arrangement_spacing_int.min(remaining.wrapping_sub(desired_width).max(0));

            fixed_space = fixed_space.wrapping_add(desired_width).wrapping_add(space_after_last_no_weight);
        }
    }

    let mut weighted_space = 0i32;
    if weight_children_count == 0 {
        // fixedSpace contains an extra spacing after the last non-weight child.
        fixed_space -= space_after_last_no_weight;
    } else {
        // obtain the constraints of the rest according to their weights.
        let target_space = if main_axis_max != Constraints::Infinity { main_axis_max } else { main_axis_min };

        let arrangement_spacing_total = arrangement_spacing_px * (weight_children_count - 1);
        let remaining_to_target = (target_space.wrapping_sub(fixed_space) as i64 - arrangement_spacing_total).max(0);

        let weight_unit_space = remaining_to_target as f32 / total_weight;
        let mut remainder = remaining_to_target;
        for measurable in measurables {
            let item_weight = weight_of(&button_group_parent_data(measurable.parent_data()));
            let weighted_size = weight_unit_space * item_weight;
            remainder -= round_to_int(weighted_size) as i64;
        }

        for (i, child) in measurables.iter().enumerate() {
            if children_constraints[i].is_none() {
                let weight = weight_of(&button_group_parent_data(child.parent_data()));

                // After the weightUnitSpace rounding, the total space going to be occupied
                // can be smaller or larger than remainingToTarget. Here we distribute the
                // loss or gain remainder evenly to the first children.
                let remainder_unit = remainder.signum() as i32;
                remainder -= remainder_unit as i64;
                let weighted_size = weight_unit_space * weight;

                let child_main_axis_size = 0.max(round_to_int(weighted_size) + remainder_unit);

                children_constraints[i] = Some(named_args!(constraints.copy(
                    min_width = if child_main_axis_size != Constraints::Infinity { child_main_axis_size } else { 0 },
                    max_width = child_main_axis_size,
                )));

                children_main_axis_size[i] = child_main_axis_size;
                weighted_space += child_main_axis_size;
            }
            // (Compose's: inside the loop, the spacing added for every child; panics as Kotlin's `coerceIn` throws
            // when the range is empty)
            weighted_space =
                ((weighted_space as i64 + arrangement_spacing_total) as i32).clamp(0, main_axis_max - fixed_space);
        }
    }

    WeightedSizes { children_constraints, children_main_axis_size, fixed_space, weighted_space }
}

/// The expand-and-compress part of both policies over the first `count` children (`size` of them in all): each one's
/// growth (its pressed value × `expanded_ratio` × its width) taken from its neighbours' `widths`, how much each grew
/// in `growths`.
fn expand_and_compress(
    animatables: &[Animatable<f32, AnimationVector1D>],
    expanded_ratio: f32,
    widths: &mut [i32],
    growths: &mut [i32],
    count: usize,
) {
    for index in 0..count {
        // The amount the current item is expanding
        let growth = animatables[index].value() * expanded_ratio * widths[index] as f32;
        if index >= 1 && index + 1 < count {
            // We are a middle button, so we must compress both neighbors
            growths[index] = round_to_int(growth / 2.0);
            widths[index - 1] -= round_to_int(growth / 2.0);
            widths[index + 1] -= round_to_int(growth / 2.0);
        } else {
            if index == 0 {
                // We are the first item, so we need to compress the next item
                widths[index + 1] -= round_to_int(growth);
            } else {
                // We are the last item, so we need to compress the previous item
                widths[index - 1] -= round_to_int(growth);
            }
            growths[index] = round_to_int(growth);
        }

        widths[index] += round_to_int(growth);
    }
}

/// How far a child is moved for the growths before it (Compose's placement `growth`): from the one before it left to
/// right, from the one after it right to left.
fn placement_growth(growths: &[i32], index: usize, count: usize, layout_direction: LayoutDirection) -> i32 {
    match layout_direction {
        LayoutDirection::Ltr => {
            if index > 0 {
                growths[index - 1] - growths[index]
            } else {
                0
            }
        }
        LayoutDirection::Rtl => {
            if index + 1 < count {
                growths[index + 1] - growths[index]
            } else {
                0
            }
        }
    }
}

/// The deprecated overload's policy (Compose's private `NonAdaptiveButtonGroupMeasurePolicy`): every child in the row,
/// cut off where it doesn't fit. Compose's default intrinsics (it measures to answer).
struct NonAdaptiveButtonGroupMeasurePolicy {
    horizontal_arrangement: HorizontalArrangement,
    expanded_ratio: f32,
}

impl MeasurePolicy for NonAdaptiveButtonGroupMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let arrangement_spacing_int = scope.round_to_px(self.horizontal_arrangement.spacing());
        let size = measurables.len();
        let configs: Vec<ButtonGroupParentData> =
            measurables.iter().map(|m| button_group_parent_data(m.parent_data()).unwrap_or_default()).collect();
        let animatables: Vec<_> = configs.iter().map(|c| c.pressed_animatable.clone()).collect();

        let main_axis_min = constraints.min_width;

        let WeightedSizes { children_constraints, children_main_axis_size, fixed_space, weighted_space } =
            weighted_sizes(measurables, constraints, arrangement_spacing_int);

        // The item's widths that we'll adjust for animation
        let mut widths: Vec<i32> = children_constraints.iter().map(|c| c.unwrap_or(constraints).max_width).collect();
        // The growths used to know how much each
        // item should be adjusted in the horizontal placement
        let mut growths = vec![0i32; size];

        if size > 1 {
            expand_and_compress(&animatables, self.expanded_ratio, &mut widths, &mut growths, size);
        }

        let placeables: Vec<Placeable> = measurables
            .iter()
            .enumerate()
            .map(|(index, placeable)| {
                placeable.measure(named_args!(
                    children_constraints[index]
                        .unwrap_or(constraints)
                        .copy(min_width = widths[index], max_width = widths[index])
                ))
            })
            .collect();

        // Compute the row size and position the children.
        let main_axis_layout_size = (fixed_space + weighted_space).max(0).max(main_axis_min);
        let mut main_axis_positions = vec![0i32; size];
        self.horizontal_arrangement.arrange(
            &scope.density(),
            main_axis_layout_size,
            &children_main_axis_size,
            scope.layout_direction(),
            &mut main_axis_positions,
        );

        let height = placeables.iter().map(|p| p.height()).max().unwrap_or(constraints.min_height);
        let layout_direction = scope.layout_direction();
        scope.layout(main_axis_layout_size, height, move |p| {
            for (index, placeable) in placeables.iter().enumerate() {
                // We adjust the placement here depending on the expansion/compression of items
                let growth = placement_growth(&growths, index, placeables.len(), layout_direction);
                p.place(placeable, main_axis_positions[index] + growth, 0);
            }
        })
    }
}

/// [`ButtonGroup`]'s policy (Compose's private `ButtonGroupMeasurePolicy`): the items that fit in the row, the
/// overflow indicator after them when some don't (how many are shown and in all told to `overflow_state`). Compose's
/// default intrinsics (it measures to answer).
struct ButtonGroupMeasurePolicy {
    overflow_state: Rc<dyn ButtonGroupOverflowState>,
    horizontal_arrangement: HorizontalArrangement,
    vertical_alignment: VerticalAlignment,
    expanded_ratio: f32,
}

impl MultiContentMeasurePolicy for ButtonGroupMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let [content_measurables, overflow_measurables] = measurables else {
            panic!("ButtonGroup: two contents");
        };
        self.overflow_state.set_total_item_count(content_measurables.len() as i32);
        let arrangement_spacing_int = scope.round_to_px(self.horizontal_arrangement.spacing());
        let size = content_measurables.len();
        let configs: Vec<ButtonGroupParentData> =
            content_measurables.iter().map(|m| button_group_parent_data(m.parent_data()).unwrap_or_default()).collect();
        let animatables: Vec<_> = configs.iter().map(|c| c.pressed_animatable.clone()).collect();

        let main_axis_min = constraints.min_width;
        let main_axis_max = constraints.max_width;

        let WeightedSizes { children_constraints, children_main_axis_size, .. } =
            weighted_sizes(content_measurables, constraints, arrangement_spacing_int);

        let mut remaining_space = main_axis_max;
        // Total space required for shown items and overflow
        let mut main_space = 0i32;
        // Space required for shown items
        let mut shown_item_space = 0i32;
        // The item's widths that we'll adjust for animation
        let mut widths: Vec<i32> = children_constraints.iter().map(|c| c.unwrap_or(constraints).max_width).collect();
        // The desired widths of all of the items plus the spacing between the items
        let desired_width = widths.iter().fold(0i32, |a, w| a.wrapping_add(*w))
            + arrangement_spacing_int * (content_measurables.len() as i32 - 1);
        let mut last_item = 0usize;
        let overflow_placeables: Option<Vec<Placeable>> = if desired_width <= main_axis_max {
            // Everything can fit, so we can place all of the items
            last_item = widths.len();
            main_space = desired_width;
            None
        } else {
            // All items can't fit, so we must account for the overflow indicator.
            let overflow_width =
                overflow_measurables.iter().map(|m| m.max_intrinsic_width(constraints.max_height)).max().unwrap_or(0);
            // We want to see how many items can fit in the space that is left after accounting
            // for the overflow indicator.
            remaining_space -= overflow_width;
            main_space += overflow_width;
            // Figure out how many items can fit with overflow included
            while last_item < widths.len() && widths[last_item] <= remaining_space {
                main_space += widths[last_item];
                shown_item_space += widths[last_item];
                remaining_space -= widths[last_item] + arrangement_spacing_int;
                last_item += 1;
            }

            // Add spacing to mainSpace
            main_space += arrangement_spacing_int * last_item as i32;
            shown_item_space += arrangement_spacing_int * last_item as i32;

            Some(
                overflow_measurables
                    .iter()
                    .map(|m| m.measure(named_args!(constraints.copy(max_width = remaining_space + overflow_width))))
                    .collect(),
            )
        };

        self.overflow_state.set_visible_item_count(last_item as i32);

        // The growths used to know how much each
        // item should be adjusted in the horizontal placement
        let mut growths = vec![0i32; last_item];
        if size > 1 {
            // The expand and compress logic of button groups.
            expand_and_compress(&animatables, self.expanded_ratio, &mut widths, &mut growths, last_item);
        }

        let placeables: Vec<Placeable> = (0..last_item)
            .map(|index| {
                content_measurables[index].measure(named_args!(
                    children_constraints[index]
                        .unwrap_or(constraints)
                        .copy(min_width = widths[index], max_width = widths[index])
                ))
            })
            .collect();
        let alignments: Vec<Option<VerticalAlignment>> = (0..last_item)
            .map(|index| button_group_parent_data(content_measurables[index].parent_data()).and_then(|d| d.alignment))
            .collect();

        // Compute the row size and position the children.
        let main_axis_layout_size = main_space.max(0).max(main_axis_min);
        let mut main_axis_positions = vec![0i32; last_item];
        self.horizontal_arrangement.arrange(
            &scope.density(),
            main_axis_layout_size,
            &children_main_axis_size[..last_item],
            scope.layout_direction(),
            &mut main_axis_positions,
        );

        let height = placeables.iter().map(|p| p.height()).max().unwrap_or(constraints.min_height);

        let layout_direction = scope.layout_direction();
        let vertical_alignment = self.vertical_alignment.clone();
        scope.layout(main_axis_layout_size, height, move |p| {
            for (index, placeable) in placeables.iter().enumerate() {
                // We adjust the placement here depending on the expansion/compression of items
                let growth = placement_growth(&growths, index, placeables.len(), layout_direction);
                let y_position = match &alignments[index] {
                    Some(alignment) => alignment.align(placeable.height(), height),
                    None => vertical_alignment.align(placeable.height(), height),
                };
                p.place(placeable, main_axis_positions[index] + growth, y_position);
            }
            if let Some(overflow_placeables) = &overflow_placeables {
                for it in overflow_placeables {
                    let y_position = vertical_alignment.align(it.height(), height);
                    p.place_relative(it, shown_item_space, y_position);
                }
            }
        })
    }
}

// ---- ButtonGroupScope ------------------------------------------------------------------------------------------

/// The scope of a button group's content (Compose's `ButtonGroupScope`, an interface whose one implementation is
/// the internal `ButtonGroupScopeImpl`): the modifiers its children tell the group with, and the items of a
/// [`ButtonGroup`]. Clones are the same scope.
#[derive(Clone)]
pub struct ButtonGroupScope {
    inner: Rc<ButtonGroupScopeImpl>,
}

/// Compose's private `ButtonGroupScopeImpl` (and `ButtonGroupItemProvider`): the spec a press animates by and the
/// items added.
struct ButtonGroupScopeImpl {
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    items: RefCell<Vec<Rc<dyn ButtonGroupItem>>>,
}

impl ButtonGroupScope {
    fn new(animation_spec: Arc<dyn AnimationSpec<f32>>) -> ButtonGroupScope {
        ButtonGroupScope { inner: Rc::new(ButtonGroupScopeImpl { animation_spec, items: RefCell::new(Vec::new()) }) }
    }

    /// The items added (Compose's `items`).
    fn items(&self) -> Vec<Rc<dyn ButtonGroupItem>> {
        self.inner.items.borrow().clone()
    }

    /// How many items were added (Compose's `itemsCount`).
    #[allow(dead_code)] // (Compose's ButtonGroupItemProvider has it; nothing reads it)
    fn items_count(&self) -> usize {
        self.inner.items.borrow().len()
    }

    fn add(&self, item: Rc<dyn ButtonGroupItem>) {
        self.inner.items.borrow_mut().push(item);
    }
}

impl PartialEq for ButtonGroupScope {
    /// The same scope (Kotlin's identity).
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for ButtonGroupScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ButtonGroupScope(items={})", self.inner.items.borrow().len())
    }
}

#[named]
impl ButtonGroupScope {
    /// `modifier` sized as wide as its `weight`'s share of the width left after the unweighted children (Compose's
    /// `Modifier.weight(weight)`; infinity is taken as `f32::MAX`).
    ///
    /// Panics (Kotlin's IllegalArgumentException) for a weight that isn't greater than zero.
    ///
    /// Kotlin: `weight(weight: Float): Modifier`
    pub fn weight(&self, modifier: Modifier, weight: f32) -> Modifier {
        assert!(weight > 0.0, "invalid weight {weight:?}; must be greater than zero");
        // Coerce Float.POSITIVE_INFINITY to Float.MAX_VALUE to avoid errors
        modifier.then_element(ButtonGroupElement { weight: weight.min(f32::MAX) })
    }

    /// `modifier` growing while `interaction_source` says it is pressed, by the group's expanded ratio of its width,
    /// its neighbours squeezed (Compose's `Modifier.animateWidth(interactionSource)`).
    ///
    /// Kotlin: `animateWidth(interactionSource: InteractionSource): Modifier`
    pub fn animate_width(&self, modifier: Modifier, interaction_source: impl Into<InteractionSource>) -> Modifier {
        modifier.then_element(EnlargeOnPressElement {
            interaction_source: interaction_source.into(),
            animation_spec: self.inner.animation_spec.clone(),
        })
    }

    /// `modifier` aligned vertically in the [`ButtonGroup`] by `alignment`, before the group's own vertical
    /// alignment (Compose's `Modifier.align(alignment)`).
    ///
    /// Kotlin: `align(alignment: Alignment.Vertical): Modifier`
    pub fn align(&self, modifier: Modifier, alignment: VerticalAlignment) -> Modifier {
        modifier.then_element(VerticalAlignElement { alignment })
    }

    /// Adds a clickable item (Compose's `clickableItem`): a [`Button`] of `icon` and `label` in the row, growing as it
    /// is pressed (its `weight` as [`weight`](Self::weight)'s, NaN for none), a menu item of them in the overflow
    /// menu (clicked, it closes the menu); `on_click` runs on a click of either, unless not `enabled`.
    ///
    /// Panics (Kotlin's IllegalArgumentException) for a weight neither greater than zero nor NaN.
    ///
    /// Kotlin: `clickableItem(onClick: () -> Unit, label: String, icon: (@Composable () -> Unit)? = null, weight: Float = Float.NaN, enabled: Boolean = true)`
    pub fn clickable_item(
        &self,
        on_click: impl Fn() + 'static,
        label: impl Into<String>,
        #[default(None)] icon: Option<Rc<dyn Fn()>>,
        #[default(f32::NAN)] weight: f32,
        #[default(true)] enabled: bool,
    ) {
        assert!(weight > 0.0 || weight.is_nan(), "invalid weight {weight:?}; must be greater than zero or Float.NaN");
        self.add(Rc::new(ClickableButtonGroupItem {
            on_click: Rc::new(on_click),
            icon,
            enabled,
            weight,
            animation_spec: self.inner.animation_spec.clone(),
            label: label.into(),
        }));
    }

    /// Adds a toggleable item (Compose's `toggleableItem`): a [`ToggleButton`] of `icon` and `label` in the row,
    /// `checked` or not, growing as it is pressed, a menu item of them in the overflow menu (clicked, it toggles and
    /// closes the menu); `on_check_change` is given the other state, unless not `enabled`.
    ///
    /// Panics (Kotlin's IllegalArgumentException) for a weight neither greater than zero nor NaN.
    ///
    /// Kotlin: `toggleableItem(checked: Boolean, label: String, onCheckedChange: (Boolean) -> Unit, icon: (@Composable () -> Unit)? = null, weight: Float = Float.NaN, enabled: Boolean = true)`
    pub fn toggleable_item(
        &self,
        checked: bool,
        label: impl Into<String>,
        on_checked_change: impl Fn(bool) + 'static,
        #[default(None)] icon: Option<Rc<dyn Fn()>>,
        #[default(f32::NAN)] weight: f32,
        #[default(true)] enabled: bool,
    ) {
        assert!(weight > 0.0 || weight.is_nan(), "invalid weight {weight:?}; must be greater than zero or Float.NaN");
        self.add(Rc::new(ToggleableButtonGroupItem {
            checked,
            on_checked_change: Rc::new(on_checked_change),
            icon,
            enabled,
            weight,
            animation_spec: self.inner.animation_spec.clone(),
            label: label.into(),
        }));
    }

    /// Adds an item of one's own (Compose's `customItem`): `button_group_content` in the row, `menu_content` (given
    /// the menu's state) in the overflow menu.
    ///
    /// Kotlin: `customItem(buttonGroupContent: @Composable () -> Unit, menuContent: @Composable (ButtonGroupMenuState) -> Unit)`
    pub fn custom_item(
        &self,
        button_group_content: impl Fn() + 'static,
        menu_content: impl Fn(&ButtonGroupMenuState) + 'static,
    ) {
        self.add(Rc::new(CustomButtonGroupItem {
            button_group_content: Rc::new(button_group_content),
            menu_content: Rc::new(menu_content),
        }));
    }
}

/// Remembers the scope of a [`ButtonGroup`] made of the latest `content` (Compose's private
/// `rememberButtonGroupScopeState`): a derived state, made again when a state `content` reads changes or a new
/// content is given (`animation_spec` the one given first).
#[track_caller]
fn remember_button_group_scope_state(
    content: Rc<dyn Fn(&ButtonGroupScope)>,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
) -> State<ButtonGroupScope> {
    kompozable_block!({
        let latest_content = remember_updated_state!(content);
        let derived: DerivedState<ButtonGroupScope> = remember(move || {
            derived_state_of_with_policy(never_equal_policy(), move || {
                let scope = ButtonGroupScope::new(animation_spec.clone());
                (latest_content.value())(&scope);
                scope
            })
        });
        State::from(derived)
    })
}

// ---- The items -------------------------------------------------------------------------------------------------

/// An item of a [`ButtonGroup`] (Compose's internal `ButtonGroupItem`): what it shows in the row and in the overflow
/// menu.
pub(crate) trait ButtonGroupItem {
    /// The item in the button group (Compose's `ButtonGroupContent`). Composable.
    #[allow(non_snake_case)]
    fn ButtonGroupContent(&self);

    /// The item in the overflow menu, given the menu's state (Compose's `MenuContent`). Composable.
    #[allow(non_snake_case)]
    fn MenuContent(&self, state: &ButtonGroupMenuState);
}

/// The modifier of an item growing as pressed through `interaction_source` and weighted by `weight` unless it is NaN.
fn item_modifier(
    interaction_source: &MutableInteractionSource,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    weight: f32,
) -> Modifier {
    Modifier.then_element(EnlargeOnPressElement { interaction_source: interaction_source.into(), animation_spec }).then(
        if !weight.is_nan() {
            Modifier.then_element(ButtonGroupElement { weight: weight.min(f32::MAX) })
        } else {
            Modifier
        },
    )
}

/// An item's icon (if any, then the button's icon spacing) and label, as the items' buttons show them.
fn item_label(icon: Option<Rc<dyn Fn()>>, label: String) -> impl Fn() + 'static {
    move || {
        kompozable_block!({
            if let Some(icon) = &icon {
                icon();
                Spacer(Modifier.size(ButtonDefaults::IconSpacing));
            }
            Text(label.clone());
        });
    }
}

/// A clickable item (Compose's internal `ClickableButtonGroupItem`).
struct ClickableButtonGroupItem {
    on_click: Rc<dyn Fn()>,
    icon: Option<Rc<dyn Fn()>>,
    weight: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    enabled: bool,
    label: String,
}

impl ButtonGroupItem for ClickableButtonGroupItem {
    fn ButtonGroupContent(&self) {
        kompozable_block!({
            ClickableButtonGroupItemContent(
                self.on_click.clone(),
                self.icon.clone(),
                self.weight,
                self.animation_spec.clone(),
                self.enabled,
                self.label.clone(),
            );
        });
    }

    fn MenuContent(&self, state: &ButtonGroupMenuState) {
        kompozable_block!({
            let (on_click, state, label) = (self.on_click.clone(), state.clone(), self.label.clone());
            DropdownMenuItem(
                enabled = self.enabled,
                leading_icon = self.icon.clone(),
                text = move || Text(label.clone()),
                on_click = move || {
                    on_click();
                    state.dismiss();
                },
            );
        });
    }
}

/// [`ClickableButtonGroupItem`]'s content in the row.
#[kompozable]
fn ClickableButtonGroupItemContent(
    on_click: Rc<dyn Fn()>,
    icon: Option<Rc<dyn Fn()>>,
    weight: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    enabled: bool,
    label: String,
) {
    let interaction_source = remember(MutableInteractionSource::new);
    let modifier = item_modifier(&interaction_source, animation_spec, weight);
    Button(
        on_click = move || on_click(),
        modifier = modifier,
        interaction_source = Some(interaction_source),
        enabled = enabled,
        content = item_label(icon, label),
    );
}

/// A toggleable item (Compose's internal `ToggleableButtonGroupItem`).
struct ToggleableButtonGroupItem {
    checked: bool,
    on_checked_change: Rc<dyn Fn(bool)>,
    weight: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    icon: Option<Rc<dyn Fn()>>,
    enabled: bool,
    label: String,
}

impl ButtonGroupItem for ToggleableButtonGroupItem {
    fn ButtonGroupContent(&self) {
        kompozable_block!({
            ToggleableButtonGroupItemContent(
                self.checked,
                self.on_checked_change.clone(),
                self.weight,
                self.animation_spec.clone(),
                self.icon.clone(),
                self.enabled,
                self.label.clone(),
            );
        });
    }

    fn MenuContent(&self, state: &ButtonGroupMenuState) {
        kompozable_block!({
            let (on_checked_change, checked, state, label) =
                (self.on_checked_change.clone(), self.checked, state.clone(), self.label.clone());
            DropdownMenuItem(
                enabled = self.enabled,
                leading_icon = self.icon.clone(),
                text = move || Text(label.clone()),
                on_click = move || {
                    on_checked_change(!checked);
                    state.dismiss();
                },
            );
        });
    }
}

/// [`ToggleableButtonGroupItem`]'s content in the row.
#[kompozable]
fn ToggleableButtonGroupItemContent(
    checked: bool,
    on_checked_change: Rc<dyn Fn(bool)>,
    weight: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    icon: Option<Rc<dyn Fn()>>,
    enabled: bool,
    label: String,
) {
    let interaction_source = remember(MutableInteractionSource::new);
    let modifier = item_modifier(&interaction_source, animation_spec, weight);
    ToggleButton(
        checked = checked,
        on_checked_change = move |c| on_checked_change(c),
        modifier = modifier,
        interaction_source = Some(interaction_source),
        enabled = enabled,
        content = item_label(icon, label),
    );
}

/// An item of one's own (Compose's internal `CustomButtonGroupItem`).
struct CustomButtonGroupItem {
    button_group_content: Rc<dyn Fn()>,
    menu_content: Rc<dyn Fn(&ButtonGroupMenuState)>,
}

impl ButtonGroupItem for CustomButtonGroupItem {
    fn ButtonGroupContent(&self) {
        (self.button_group_content)();
    }

    fn MenuContent(&self, state: &ButtonGroupMenuState) {
        (self.menu_content)(state);
    }
}

// ---- The children's modifiers ----------------------------------------------------------------------------------

/// A child's weight (Compose's internal `ButtonGroupElement`).
#[derive(PartialEq)]
pub(crate) struct ButtonGroupElement {
    pub(crate) weight: f32,
}

impl ModifierNodeElement for ButtonGroupElement {
    type Node = ButtonGroupNode;
    fn create(&self) -> ButtonGroupNode {
        ButtonGroupNode { weight: self.weight }
    }
    fn update(&self, node: &mut ButtonGroupNode) {
        node.weight = self.weight;
    }
    fn name(&self) -> &'static str {
        "weight"
    }
}

/// The node of a child's weight (Compose's internal `ButtonGroupNode`): it sets the weight of the parent data the
/// nodes after it made, or of a new one.
pub(crate) struct ButtonGroupNode {
    weight: f32,
}

impl ModifierNode for ButtonGroupNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for ButtonGroupNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = previous_parent_data(parent_data).unwrap_or_default();
        data.weight = self.weight;
        Some(Box::new(data))
    }
}

/// A child growing while pressed (Compose's internal `EnlargeOnPressElement`).
pub(crate) struct EnlargeOnPressElement {
    pub(crate) interaction_source: InteractionSource,
    pub(crate) animation_spec: Arc<dyn AnimationSpec<f32>>,
}

impl PartialEq for EnlargeOnPressElement {
    /// Never (Compose's `equals` asks whether the other is an `EnlargeOnPressNode`, which an element never is:
    /// unequal unless the same instance, so each new element updates the node).
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl ModifierNodeElement for EnlargeOnPressElement {
    type Node = EnlargeOnPressNode;
    fn create(&self) -> EnlargeOnPressNode {
        EnlargeOnPressNode {
            interaction_source: self.interaction_source.clone(),
            animation_spec: Rc::new(RefCell::new(self.animation_spec.clone())),
            pressed_animatable: Animatable(0.0f32),
            collection_job: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut EnlargeOnPressNode) {
        if node.interaction_source != self.interaction_source {
            node.interaction_source = self.interaction_source.clone();
            node.launch_collection_job();
        }
        *node.animation_spec.borrow_mut() = self.animation_spec.clone();
    }
    fn name(&self) -> &'static str {
        "EnlargeOnPressElement"
    }
}

/// The node of a child growing while pressed (Compose's internal `EnlargeOnPressNode`): it collects its source's
/// presses, animating its value to 1 while any is held and back to 0 when none is (once it is past 0.75, or after a
/// second), and gives the value to the group in its parent data.
pub(crate) struct EnlargeOnPressNode {
    interaction_source: InteractionSource,
    /// (read by the collecting coroutine each time it animates, as Compose's node property)
    animation_spec: Rc<RefCell<Arc<dyn AnimationSpec<f32>>>>,
    pressed_animatable: Animatable<f32, AnimationVector1D>,
    collection_job: Option<Job>,
    handle: Option<NodeHandle>,
}

impl EnlargeOnPressNode {
    /// Compose's `launchCollectionJob`: the collecting coroutine started again in the node's scope.
    fn launch_collection_job(&mut self) {
        if let Some(job) = self.collection_job.take() {
            job.cancel();
        }
        let Some(scope) = self.handle.as_ref().and_then(|h| h.coroutine_scope()) else { return };
        let interactions = self.interaction_source.interactions();
        let (pressed_animatable, animation_spec) = (self.pressed_animatable.clone(), self.animation_spec.clone());
        let inner_scope = scope.clone();
        self.collection_job = Some(scope.launch(async move {
            let press_interactions: Rc<RefCell<Vec<PressInteraction::Press>>> = Rc::new(RefCell::new(Vec::new()));
            let collecting = interactions
                .map(move |interaction| {
                    let mut presses = press_interactions.borrow_mut();
                    // (Kotlin's `remove(interaction.press)`: the first equal press, if any)
                    let ended = if let Some(press) = interaction.downcast_ref::<PressInteraction::Press>() {
                        presses.push(*press);
                        None
                    } else if let Some(release) = interaction.downcast_ref::<PressInteraction::Release>() {
                        Some(release.press)
                    } else {
                        interaction.downcast_ref::<PressInteraction::Cancel>().map(|cancel| cancel.press)
                    };
                    if let Some(press) = ended
                        && let Some(i) = presses.iter().position(|p| *p == press)
                    {
                        presses.remove(i);
                    }
                    !presses.is_empty()
                })
                .distinct_until_changed()
                .collect_latest(move |pressed| {
                    let (animatable, spec, scope) =
                        (pressed_animatable.clone(), animation_spec.borrow().clone(), inner_scope.clone());
                    async move {
                        if pressed {
                            scope.launch(async move {
                                named_args!(animatable.animate_to(1.0, animation_spec = spec)).await;
                            });
                        } else {
                            let waited = animatable.clone();
                            wait_until(move || waited.value() > 0.75).await;
                            named_args!(animatable.animate_to(0.0, animation_spec = spec)).await;
                        }
                    }
                });
            collecting.await;
        }));
    }
}

impl ModifierNode for EnlargeOnPressNode {
    node_capabilities!(parent_data);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.launch_collection_job();
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.collection_job = None;
    }
}

impl ParentDataModifierNode for EnlargeOnPressNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let prev = previous_parent_data(parent_data);
        Some(Box::new(ButtonGroupParentData {
            weight: weight_of(&prev),
            pressed_animatable: self.pressed_animatable.clone(),
            alignment: prev.and_then(|p| p.alignment),
        }))
    }
}

/// A child's vertical alignment (Compose's internal `VerticalAlignElement`).
#[derive(PartialEq)]
pub(crate) struct VerticalAlignElement {
    pub(crate) alignment: VerticalAlignment,
}

impl ModifierNodeElement for VerticalAlignElement {
    type Node = VerticalAlignNode;
    fn create(&self) -> VerticalAlignNode {
        VerticalAlignNode { alignment: self.alignment.clone() }
    }
    fn update(&self, node: &mut VerticalAlignNode) {
        node.alignment = self.alignment.clone();
    }
    fn name(&self) -> &'static str {
        "align"
    }
}

/// The node of a child's vertical alignment (Compose's internal `VerticalAlignNode`).
pub(crate) struct VerticalAlignNode {
    alignment: VerticalAlignment,
}

impl ModifierNode for VerticalAlignNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for VerticalAlignNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = previous_parent_data(parent_data).unwrap_or_default();
        data.alignment = Some(self.alignment.clone());
        Some(Box::new(data))
    }
}

// ---- The overflow state ----------------------------------------------------------------------------------------

/// What a [`ButtonGroup`] knows of its overflow (Compose's private `ButtonGroupOverflowState`): how many items the
/// content has (not the indicator), how many are shown.
trait ButtonGroupOverflowState {
    fn total_item_count(&self) -> i32;
    fn set_total_item_count(&self, count: i32);
    fn visible_item_count(&self) -> i32;
    fn set_visible_item_count(&self, count: i32);
}

/// Compose's private `OverflowStateImpl`: the counts as states.
struct OverflowStateImpl {
    total_item_count: MutableState<i32>,
    visible_item_count: MutableState<i32>,
}

impl OverflowStateImpl {
    fn new() -> OverflowStateImpl {
        OverflowStateImpl { total_item_count: mutable_int_state_of(0), visible_item_count: mutable_int_state_of(0) }
    }

    /// Saves both counts (Compose's companion's `Saver`).
    #[allow(non_snake_case)]
    fn Saver() -> Saver<Rc<dyn ButtonGroupOverflowState>> {
        Saver::new(
            |state: &Rc<dyn ButtonGroupOverflowState>| {
                SavedValue::list([
                    SavedValue::Int(state.total_item_count() as i64),
                    SavedValue::Int(state.visible_item_count() as i64),
                ])
            },
            |saved| {
                let list = saved.as_list()?;
                let state = OverflowStateImpl::new();
                state.set_total_item_count(list.first()?.as_int()? as i32);
                state.set_visible_item_count(list.get(1)?.as_int()? as i32);
                Some(Rc::new(state) as Rc<dyn ButtonGroupOverflowState>)
            },
        )
    }
}

impl ButtonGroupOverflowState for OverflowStateImpl {
    fn total_item_count(&self) -> i32 {
        self.total_item_count.get()
    }
    fn set_total_item_count(&self, count: i32) {
        self.total_item_count.set(count);
    }
    fn visible_item_count(&self) -> i32 {
        self.visible_item_count.get()
    }
    fn set_visible_item_count(&self, count: i32) {
        self.visible_item_count.set(count);
    }
}

/// Remembers and saves the overflow state of a [`ButtonGroup`] (Compose's private `rememberOverflowState`).
#[track_caller]
fn remember_overflow_state() -> Rc<dyn ButtonGroupOverflowState> {
    kompozable_block!({
        remember_saveable_with(&OverflowStateImpl::Saver(), || {
            Rc::new(OverflowStateImpl::new()) as Rc<dyn ButtonGroupOverflowState>
        })
    })
}

/// An overflow state compared by its instance (Compose's `remember(…, overflowState)`).
struct OverflowStateKey(Rc<dyn ButtonGroupOverflowState>);

impl PartialEq for OverflowStateKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Waits frame by frame until `condition` holds, or a second has gone by (Compose's private `waitUntil`).
async fn wait_until(condition: impl Fn() -> bool) {
    let initial_time_millis = with_frame_millis(|it| it).await;
    while !condition() {
        let time_millis = with_frame_millis(|it| it).await;
        if time_millis - initial_time_millis > MAX_WAIT_TIME_MILLIS {
            return;
        }
    }
}

const MAX_WAIT_TIME_MILLIS: i64 = 1_000;
