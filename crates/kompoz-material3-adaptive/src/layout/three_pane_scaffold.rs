//! The three-pane scaffold, as Compose's adaptive-layout ThreePaneScaffold.kt (1.2.0): the internal composable the
//! list-detail and supporting pane scaffolds are made of — three panes by role in an order, adapted by the
//! scaffold's value (expanded side by side within the directive's partitions, reflowed under another, levitated
//! over the others, hidden), moving along the scaffold state's transition, with the pane expansion state and its
//! drag handle between two expanded panes — its measure policy, its override API
//! ([`ThreePaneScaffoldOverride`], [`LocalThreePaneScaffoldOverride`]) and its internal defaults.

use super::pane_adapted_value::PaneAdaptedValue;
use super::pane_expansion_state::{
    PaneExpansionState, PaneExpansionStateKeyProvider, remember_default_pane_expansion_state,
};
use super::pane_motion::PaneScaffoldMotionDataProvider;
use super::pane_scaffold::{min_touch_target_size, pane_scaffold_parent_data};
use super::pane_scaffold_directive::PaneScaffoldDirective;
use super::pane_scaffold_role::ThreePaneScaffoldRole;
use super::predictive_back_scale_state::{CollectPredictiveBackScale, predictive_back_scale};
use super::three_pane_motion::{ThreePaneScaffoldMotionDataProvider, calculate_three_pane_motion};
use super::three_pane_scaffold_scope::{
    ThreePaneScaffoldPaneScope, ThreePaneScaffoldScope, ThreePaneScaffoldTransitionScopeImpl,
    remember_three_pane_scaffold_pane_scope,
};
use super::three_pane_scaffold_state::{MutableThreePaneScaffoldState, ThreePaneScaffoldState};
use super::three_pane_scaffold_value::{
    PaneScaffoldHorizontalOrder, ThreePaneScaffoldHorizontalOrder, ThreePaneScaffoldValue,
};
use kompoz_coroutines::LaunchedEffect;
use kompoz_graphics::{Density, Dp, IntOffset, IntRect, IntSize, LayoutDirection, Offset};
use kompoz_runtime::{
    KompozitionLocal, MutableState, kompozable, kompozable_block, mutable_state_of, named_args, remember,
    remember_keyed, remember_saveable_state_holder,
};
use kompoz_ui::layout::{
    LookaheadScope, Measurable, MeasureResult, MeasureScope, MultiContentLayout, MultiContentMeasurePolicy,
    PlacementScope,
};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{Alignment, Constraints, Modifier};
use std::cell::Cell;
use std::rc::Rc;

/// A pane's content (Compose's `@Composable ThreePaneScaffoldPaneScope.() -> Unit`).
pub type ThreePanePane = Rc<dyn Fn(&ThreePaneScaffoldPaneScope)>;

/// The drag handle's content (Compose's `@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit`).
pub type ThreePaneDragHandle = Rc<dyn Fn(&ThreePaneScaffoldScope, PaneExpansionState)>;

/// The scaffold animating itself to `scaffold_value` whenever it changes (Compose's internal `ThreePaneScaffold(…,
/// scaffoldValue, …)`): a remembered [`MutableThreePaneScaffoldState`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn ThreePaneScaffoldOfValue(
    modifier: Modifier,
    scaffold_directive: PaneScaffoldDirective,
    scaffold_value: ThreePaneScaffoldValue,
    pane_order: ThreePaneScaffoldHorizontalOrder,
    secondary_pane: ThreePanePane,
    tertiary_pane: Option<ThreePanePane>,
    pane_expansion_state: Option<PaneExpansionState>,
    pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    primary_pane: ThreePanePane,
) {
    let initial = scaffold_value.clone();
    // (one shared `Rc`: what keys the state — the predictive back scale's collection — sees the same state)
    let scaffold_state = remember(move || Rc::new(MutableThreePaneScaffoldState::new(initial)));
    let (state, target) = (scaffold_state.clone(), scaffold_value.clone());
    LaunchedEffect(scaffold_value, async move {
        named_args!(state.animate_to(target_state = target)).await;
    });
    ThreePaneScaffold(
        modifier,
        scaffold_directive,
        scaffold_state as Rc<dyn ThreePaneScaffoldState>,
        pane_order,
        secondary_pane,
        tertiary_pane,
        pane_expansion_state,
        pane_expansion_drag_handle,
        primary_pane,
    );
}

/// The scaffold of a state (Compose's internal `ThreePaneScaffold(…, scaffoldState, …)`): the panes composed in a
/// lookahead scope, each in its pane scope, laid out by the override (by default
/// [`ThreePaneScaffoldOverride`]'s default, the measure policy here); the current destination's pane asks for the
/// focus.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn ThreePaneScaffold(
    modifier: Modifier,
    scaffold_directive: PaneScaffoldDirective,
    scaffold_state: Rc<dyn ThreePaneScaffoldState>,
    pane_order: ThreePaneScaffoldHorizontalOrder,
    secondary_pane: ThreePanePane,
    tertiary_pane: Option<ThreePanePane>,
    pane_expansion_state: Option<PaneExpansionState>,
    pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    primary_pane: ThreePanePane,
) {
    let expansion_state = match pane_expansion_state {
        Some(state) => state,
        None => {
            let state = scaffold_state.clone();
            remember_default_pane_expansion_state(
                move || state.target_state().pane_expansion_state_key(),
                pane_expansion_drag_handle.is_some(),
            )
        }
    };
    let layout_direction = LocalLayoutDirection.current();
    let ltr_pane_order =
        remember_keyed((pane_order, layout_direction), move || pane_order.to_ltr_order(layout_direction));
    let pane_motions = calculate_three_pane_motion(&*scaffold_state, ltr_pane_order);
    let motion_data_provider = remember(ThreePaneScaffoldMotionDataProvider::new);
    // TODO(conradchen): Find a better way to provide predictive back state
    *motion_data_provider.scaffold_state.borrow_mut() = Some(scaffold_state.clone());
    motion_data_provider.update(pane_motions, ltr_pane_order);

    let current_transition = scaffold_state.remember_transition();
    let provider = motion_data_provider.clone();
    let transition_scope = remember(move || Rc::new(ThreePaneScaffoldTransitionScopeImpl::new(provider)));
    *transition_scope.transition_state.borrow_mut() = Some(scaffold_state.clone());
    *transition_scope.scaffold_state_transition.borrow_mut() = Some(current_transition.clone());

    let state_holder = remember_saveable_state_holder();

    kompoz_ui::layout::LookaheadScope(move |lookahead_scope: &Rc<dyn LookaheadScope>| {
        let (transition_scope, state_holder, lookahead_scope) =
            (transition_scope.clone(), state_holder.clone(), lookahead_scope.clone());
        let (scaffold_state, primary_pane, secondary_pane, tertiary_pane) =
            (scaffold_state.clone(), primary_pane.clone(), secondary_pane.clone(), tertiary_pane.clone());
        let (modifier, scaffold_directive, expansion_state, drag_handle, motion_data_provider) = (
            modifier.clone(),
            scaffold_directive.clone(),
            expansion_state.clone(),
            pane_expansion_drag_handle.clone(),
            motion_data_provider.clone(),
        );
        let transition = current_transition.clone();
        kompozable_block!({
            let scaffold_scope =
                remember_keyed((TransitionKey(transition.clone()), LookaheadKey(lookahead_scope.clone())), move || {
                    ThreePaneScaffoldScope::new(transition_scope.clone(), lookahead_scope.clone(), state_holder.clone())
                });
            let scaffold_value = scaffold_state.target_state();
            let pane = |role: ThreePaneScaffoldRole, content: ThreePanePane| -> Rc<dyn Fn()> {
                let (scope, motions, value) = (scaffold_scope.clone(), pane_motions, scaffold_value.clone());
                Rc::new(move || {
                    let (scope, content, value) = (scope.clone(), content.clone(), value.clone());
                    kompozable_block!({
                        let pane_scope = remember_three_pane_scaffold_pane_scope(
                            role,
                            scope,
                            motions.get(role),
                            value.is_interactable(role),
                        );
                        content(&pane_scope);
                    })
                })
            };
            let handle: Option<Rc<dyn Fn(PaneExpansionState)>> = drag_handle.clone().map(|handle| {
                let scope = scaffold_scope.clone();
                Rc::new(move |state: PaneExpansionState| handle(&scope, state)) as Rc<dyn Fn(PaneExpansionState)>
            });
            let override_ = LocalThreePaneScaffoldOverride.current();
            override_.ThreePaneScaffold(&ThreePaneScaffoldOverrideScope {
                modifier: modifier.clone(),
                scaffold_directive: scaffold_directive.clone(),
                scaffold_state: scaffold_state.clone(),
                pane_order,
                primary_pane: pane(ThreePaneScaffoldRole::Primary, primary_pane.clone()),
                secondary_pane: pane(ThreePaneScaffoldRole::Secondary, secondary_pane.clone()),
                tertiary_pane: tertiary_pane.clone().map(|t| pane(ThreePaneScaffoldRole::Tertiary, t)),
                pane_expansion_state: expansion_state.clone(),
                pane_expansion_drag_handle: handle,
                motion_data_provider: motion_data_provider.clone(),
            });

            let focus_scope = scaffold_scope.clone();
            let destination = scaffold_value.current_destination;
            LaunchedEffect(destination, async move {
                if let Some(role) = destination {
                    focus_scope.focus_requester(role).request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
                }
            });
        });
    });
}

/// A transition as a key: the same transition (Compose's `remember(currentTransition, this)`).
struct TransitionKey(kompoz_animation::Transition<ThreePaneScaffoldValue>);

impl PartialEq for TransitionKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// A lookahead scope as a key: the same scope.
struct LookaheadKey(Rc<dyn LookaheadScope>);

impl PartialEq for LookaheadKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

// ---- The override API ----

/// How a three-pane scaffold is laid out (Compose's experimental `ThreePaneScaffoldOverride`): provided by
/// [`LocalThreePaneScaffoldOverride`] to the scaffolds below.
pub trait ThreePaneScaffoldOverride {
    /// Lays out the scaffold of `scope`'s arguments (Compose's `ThreePaneScaffoldOverrideScope.ThreePaneScaffold()`).
    fn ThreePaneScaffold(&self, scope: &ThreePaneScaffoldOverrideScope);
}

/// What an override lays a scaffold out of (Compose's experimental `ThreePaneScaffoldOverrideScope`): the scaffold's
/// arguments, its panes composed in their scopes.
#[derive(Clone)]
pub struct ThreePaneScaffoldOverrideScope {
    pub modifier: Modifier,
    pub scaffold_directive: PaneScaffoldDirective,
    pub scaffold_state: Rc<dyn ThreePaneScaffoldState>,
    pub pane_order: ThreePaneScaffoldHorizontalOrder,
    pub primary_pane: Rc<dyn Fn()>,
    pub secondary_pane: Rc<dyn Fn()>,
    pub tertiary_pane: Option<Rc<dyn Fn()>>,
    pub pane_expansion_state: PaneExpansionState,
    pub pane_expansion_drag_handle: Option<Rc<dyn Fn(PaneExpansionState)>>,
    pub(crate) motion_data_provider: Rc<ThreePaneScaffoldMotionDataProvider>,
}

/// The override the scaffolds below lay out by (Compose's experimental `LocalThreePaneScaffoldOverride`): by default
/// Compose's private `DefaultThreePaneScaffoldOverride`.
pub static LocalThreePaneScaffoldOverride: KompozitionLocal<Rc<dyn ThreePaneScaffoldOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultThreePaneScaffoldOverride));

/// The default layout (Compose's private `DefaultThreePaneScaffoldOverride`): the five contents (the three panes,
/// the drag handle, a scrim of a levitated pane not in an animated pane) measured by `ThreePaneContentMeasurePolicy`,
/// scaled during a predictive back gesture.
struct DefaultThreePaneScaffoldOverride;

impl ThreePaneScaffoldOverride for DefaultThreePaneScaffoldOverride {
    #[track_caller]
    fn ThreePaneScaffold(&self, scope: &ThreePaneScaffoldOverrideScope) {
        let scope = scope.clone();
        kompozable_block!({
            DefaultThreePaneScaffold(scope);
        })
    }
}

#[kompozable]
fn DefaultThreePaneScaffold(scope: ThreePaneScaffoldOverrideScope) {
    let layout_direction = LocalLayoutDirection.current();
    let pane_order = scope.pane_order;
    let ltr_pane_order =
        remember_keyed((pane_order, layout_direction), move || pane_order.to_ltr_order(layout_direction));
    let scaffold_value = scope.scaffold_state.target_state();
    let (handle, expansion_state) = (scope.pane_expansion_drag_handle.clone(), scope.pane_expansion_state.clone());
    let scrim_value = scaffold_value.clone();
    let contents: Vec<Rc<dyn Fn()>> = vec![
        scope.primary_pane.clone(),
        scope.secondary_pane.clone(),
        scope.tertiary_pane.clone().unwrap_or_else(|| Rc::new(|| {})),
        Rc::new(move || {
            if let Some(handle) = &handle {
                handle(expansion_state.clone());
            }
        }),
        Rc::new(move || {
            // A default scrim when no AnimatedPane is being used. (Compose's `return@listOf` leaves the lambda after
            // the first pane, levitated or not: only the primary pane's scrim is ever shown here.)
            if let Some(scrim) = scrim_value.primary.levitated_scrim() {
                scrim();
            }
        }),
    ];

    let (directive, value, provider) =
        (scope.scaffold_directive.clone(), scaffold_value.clone(), scope.motion_data_provider.clone());
    let expansion = scope.pane_expansion_state.clone();
    let measure_policy = remember_keyed(ExpansionKey(scope.pane_expansion_state.clone()), move || {
        Rc::new(ThreePaneContentMeasurePolicy::new(directive, value, expansion, ltr_pane_order, provider))
    });
    measure_policy.scaffold_directive.set(scope.scaffold_directive.clone());
    measure_policy.scaffold_value.set(scaffold_value);
    measure_policy.pane_order.set(ltr_pane_order);

    CollectPredictiveBackScale(
        scope.scaffold_state.clone(),
        scope.motion_data_provider.predictive_back_scale_state.clone(),
    );

    MultiContentLayout(
        contents,
        predictive_back_scale(scope.modifier.clone(), scope.motion_data_provider.predictive_back_scale_state.clone()),
        measure_policy as Rc<dyn MultiContentMeasurePolicy>,
    );
}

/// A pane expansion state as a key: the same state.
struct ExpansionKey(PaneExpansionState);

impl PartialEq for ExpansionKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

// ---- The measure policy ----

/// The scaffold's measure policy (Compose's private `ThreePaneContentMeasurePolicy`): its directive, value and
/// order are states set at each composition (a change measures again, not a new policy).
struct ThreePaneContentMeasurePolicy {
    scaffold_directive: MutableState<PaneScaffoldDirective>,
    scaffold_value: MutableState<ThreePaneScaffoldValue>,
    pane_order: MutableState<ThreePaneScaffoldHorizontalOrder>,
    pane_expansion_state: PaneExpansionState,
    motion_data_provider: Rc<ThreePaneScaffoldMotionDataProvider>,
}

impl ThreePaneContentMeasurePolicy {
    fn new(
        scaffold_directive: PaneScaffoldDirective,
        scaffold_value: ThreePaneScaffoldValue,
        pane_expansion_state: PaneExpansionState,
        pane_order: ThreePaneScaffoldHorizontalOrder,
        motion_data_provider: Rc<ThreePaneScaffoldMotionDataProvider>,
    ) -> ThreePaneContentMeasurePolicy {
        ThreePaneContentMeasurePolicy {
            scaffold_directive: mutable_state_of(scaffold_directive),
            scaffold_value: mutable_state_of(scaffold_value),
            pane_order: mutable_state_of(pane_order),
            pane_expansion_state,
            motion_data_provider,
        }
    }
}

impl MultiContentMeasurePolicy for ThreePaneContentMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let retain = |list: &Vec<&dyn Measurable>, at: usize| list.get(at).and_then(|m| m.retain());
        let primary_measurable = retain(&measurables[0], 0);
        let secondary_measurable = retain(&measurables[1], 0);
        let tertiary_measurable = retain(&measurables[2], 0);
        let drag_handle_measurable = retain(&measurables[3], 0);
        // (Compose's getScrimMeasurable: an animated pane's scrim is its content's second layout)
        let scrim_measurable = measurables[0..3]
            .iter()
            .find(|list| list.len() > 1)
            .and_then(|list| list[1].retain())
            .or_else(|| retain(&measurables[4], 0));
        let density = scope.density();
        let layout_direction = scope.layout_direction();
        let is_looking_ahead = scope.is_looking_ahead();
        let (directive, value, pane_order) =
            (self.scaffold_directive.get(), self.scaffold_value.get(), self.pane_order.get());
        let expansion = self.pane_expansion_state.clone();
        let provider = self.motion_data_provider.clone();
        let (max_width, max_height) = (constraints.max_width, constraints.max_height);
        scope.layout(max_width, max_height, move |p: &PlacementScope| {
            let Some(coordinates) = p.coordinates() else {
                return;
            };
            let place = PlaceContext { directive: &directive, provider: &provider, is_looking_ahead };
            let scaffold_size = IntSize::new(max_width, max_height);
            provider.set_scaffold_size(scaffold_size);
            let outer_bounds = IntRect { left: 0, top: 0, right: max_width, bottom: max_height };
            let panes = |predicate: &dyn Fn(&PaneAdaptedValue) -> bool| {
                get_panes_measurables(
                    &pane_order,
                    &value,
                    [&primary_measurable, &secondary_measurable, &tertiary_measurable],
                    &directive,
                    density,
                    scaffold_size,
                    predicate,
                )
            };
            let expanded_panes = panes(&|v| *v == PaneAdaptedValue::Expanded);
            let reflowed_panes = panes(&|v| v.is_reflowed());
            let levitated_panes = panes(&|v| v.is_levitated());
            let hidden_panes = panes(&|v| *v == PaneAdaptedValue::Hidden);

            let drag_handle = drag_handle_measurable.clone().map(|m| DragHandleMeasurable::new(m, density));

            let vertical_spacer_size = density.round_to_px(directive.horizontal_partition_spacer_size);
            let horizontal_spacer_size = density.round_to_px(directive.vertical_partition_spacer_size);
            if !is_looking_ahead {
                expansion.on_measured(outer_bounds.width(), &density);
            }

            if !expansion.is_unspecified() && expanded_panes.len() == 2 {
                // Pane expansion should override everything
                let dragging_offset = expansion.current_dragging_offset();
                if dragging_offset != PaneExpansionState::Unspecified {
                    // Respect the user dragging result if there's any
                    let half_spacer_size = vertical_spacer_size / 2;
                    if dragging_offset <= half_spacer_size {
                        let bounds = if expansion.is_dragging_or_settling() {
                            IntRect { left: dragging_offset * 2 + outer_bounds.left, ..outer_bounds }
                        } else {
                            outer_bounds
                        };
                        place.measure_and_place_panes_in_partition(
                            bounds,
                            horizontal_spacer_size,
                            &expanded_panes[1],
                            &reflowed_panes,
                        );
                    } else if dragging_offset >= outer_bounds.width() - half_spacer_size {
                        let bounds = if expansion.is_dragging_or_settling() {
                            IntRect { right: dragging_offset * 2 - outer_bounds.right, ..outer_bounds }
                        } else {
                            outer_bounds
                        };
                        place.measure_and_place_panes_in_partition(
                            bounds,
                            horizontal_spacer_size,
                            &expanded_panes[0],
                            &reflowed_panes,
                        );
                    } else {
                        place.measure_and_place_panes_in_partition(
                            IntRect { right: dragging_offset - half_spacer_size, ..outer_bounds },
                            horizontal_spacer_size,
                            &expanded_panes[0],
                            &reflowed_panes,
                        );
                        place.measure_and_place_panes_in_partition(
                            IntRect { left: dragging_offset + half_spacer_size, ..outer_bounds },
                            horizontal_spacer_size,
                            &expanded_panes[1],
                            &reflowed_panes,
                        );
                    }
                } else {
                    // Pane expansion settings from non-dragging results
                    let available_width = max_width;
                    if expansion.first_pane_width() == 0 || expansion.first_pane_proportion() == 0.0 {
                        place.measure_and_place_panes_in_partition(
                            outer_bounds,
                            horizontal_spacer_size,
                            &expanded_panes[1],
                            &reflowed_panes,
                        );
                    } else if expansion.first_pane_width() >= available_width - vertical_spacer_size
                        || expansion.first_pane_proportion() >= 1.0
                    {
                        place.measure_and_place_panes_in_partition(
                            outer_bounds,
                            horizontal_spacer_size,
                            &expanded_panes[0],
                            &reflowed_panes,
                        );
                    } else {
                        let first_pane_width = if expansion.first_pane_width() != PaneExpansionState::Unspecified {
                            expansion.first_pane_width()
                        } else {
                            (expansion.first_pane_proportion() * (available_width - vertical_spacer_size) as f32) as i32
                        };
                        let first_pane_right = outer_bounds.left + first_pane_width;
                        place.measure_and_place_panes_in_partition(
                            IntRect { right: first_pane_right, ..outer_bounds },
                            horizontal_spacer_size,
                            &expanded_panes[0],
                            &reflowed_panes,
                        );
                        place.measure_and_place_panes_in_partition(
                            IntRect { left: first_pane_right + vertical_spacer_size, ..outer_bounds },
                            horizontal_spacer_size,
                            &expanded_panes[1],
                            &reflowed_panes,
                        );
                    }
                }
            } else if !directive.excluded_bounds.is_empty() {
                let mut layout_partitions: Vec<IntRect> = Vec::new();
                let mut actual_left = outer_bounds.left;
                let mut actual_right = outer_bounds.right;
                let actual_top = outer_bounds.top;
                let actual_bottom = outer_bounds.bottom;
                // Assume hinge bounds are sorted from left to right, non-overlapped. (Compose's `return@forEach`
                // only skips to the next hinge.)
                for bound in &directive.excluded_bounds {
                    let excluded = bound.translate(coordinates.window_to_local(Offset::Zero)).round_to_int_rect();
                    if excluded.left <= actual_left {
                        // The hinge is at the left of the layout.
                        actual_left = actual_left.max(excluded.right);
                    } else if excluded.right >= actual_right {
                        // The hinge is at the right of the layout: no room for more partitions.
                        actual_right = excluded.left.min(actual_right);
                        continue;
                    } else {
                        // The hinge is inside the layout.
                        layout_partitions.push(IntRect {
                            left: actual_left,
                            top: actual_top,
                            right: excluded.left,
                            bottom: actual_bottom,
                        });
                        actual_left = excluded.right.max(excluded.left + vertical_spacer_size);
                    }
                }
                if actual_left < actual_right {
                    // The last partition
                    layout_partitions.push(IntRect {
                        left: actual_left,
                        top: actual_top,
                        right: actual_right,
                        bottom: actual_bottom,
                    });
                }
                if layout_partitions.is_empty() {
                    // Display nothing
                } else if layout_partitions.len() == 1 {
                    place.measure_and_place_partitions_in_bounds(
                        layout_partitions[0],
                        vertical_spacer_size,
                        horizontal_spacer_size,
                        &expanded_panes,
                        &reflowed_panes,
                    );
                } else if layout_partitions.len() < expanded_panes.len() {
                    // Only two physical partitions but three expanded panes: two panes in the larger partition.
                    if layout_partitions[0].width() > layout_partitions[1].width() {
                        place.measure_and_place_partitions_in_bounds(
                            layout_partitions[0],
                            vertical_spacer_size,
                            horizontal_spacer_size,
                            &expanded_panes[0..2],
                            &reflowed_panes,
                        );
                        place.measure_and_place_panes_in_partition(
                            layout_partitions[1],
                            horizontal_spacer_size,
                            &expanded_panes[2],
                            &reflowed_panes,
                        );
                    } else {
                        place.measure_and_place_panes_in_partition(
                            layout_partitions[0],
                            horizontal_spacer_size,
                            &expanded_panes[0],
                            &reflowed_panes,
                        );
                        place.measure_and_place_partitions_in_bounds(
                            layout_partitions[1],
                            vertical_spacer_size,
                            horizontal_spacer_size,
                            &expanded_panes[1..3],
                            &reflowed_panes,
                        );
                    }
                } else {
                    // Layout each visible pane in a physical partition
                    for (pane, partition) in expanded_panes.iter().zip(&layout_partitions) {
                        place.measure_and_place_panes_in_partition(
                            *partition,
                            horizontal_spacer_size,
                            pane,
                            &reflowed_panes,
                        );
                    }
                }
            } else {
                place.measure_and_place_partitions_in_bounds(
                    outer_bounds,
                    vertical_spacer_size,
                    horizontal_spacer_size,
                    &expanded_panes,
                    &reflowed_panes,
                );
            }

            if expanded_panes.len() == 2
                && let Some(drag_handle) = &drag_handle
            {
                let handle_offset_x = if !expansion.is_dragging_or_settling()
                    || expansion.current_dragging_offset() == PaneExpansionState::Unspecified
                {
                    let spacer_middle_offset = get_spacer_middle_offset_x(&expanded_panes[0], &expanded_panes[1]);
                    if !is_looking_ahead {
                        expansion.on_expansion_offset_measured(spacer_middle_offset);
                    }
                    spacer_middle_offset
                } else {
                    expansion.current_dragging_offset()
                };
                measure_and_place_drag_handle_if_needed(
                    drag_handle,
                    outer_bounds,
                    vertical_spacer_size / 2,
                    handle_offset_x,
                );
            } else if !is_looking_ahead {
                expansion.on_expansion_offset_measured(PaneExpansionState::Unspecified);
            }

            place.place_levitated_panes(&levitated_panes, outer_bounds, layout_direction);

            // Place the hidden panes to ensure a proper motion at the AnimatedVisibility, otherwise the pane will be
            // gone immediately when it's hidden.
            place.place_hidden_panes(&hidden_panes);

            for pane in &expanded_panes {
                pane.do_measure_and_place(p);
            }
            for pane in &reflowed_panes {
                pane.do_measure_and_place(p);
            }
            if let Some(handle) = &drag_handle {
                handle.do_measure_and_place(p);
            }
            if let Some(scrim) = &scrim_measurable {
                let placeable = scrim.measure(Constraints::fixed(outer_bounds.width(), outer_bounds.height()));
                p.place_at(&placeable, IntOffset::Zero, 0.0);
            }
            for pane in &levitated_panes {
                pane.do_measure_and_place(p);
            }
            for pane in &hidden_panes {
                pane.do_measure_and_place(p);
            }
        })
    }
}

/// What the placement's helpers read (Compose's policy members and the placement scope's `isLookingAhead`).
struct PlaceContext<'a> {
    directive: &'a PaneScaffoldDirective,
    provider: &'a Rc<ThreePaneScaffoldMotionDataProvider>,
    is_looking_ahead: bool,
}

/// The panes whose value `predicate` holds for, in the order (Compose's `getPanesMeasurables`).
fn get_panes_measurables(
    pane_order: &ThreePaneScaffoldHorizontalOrder,
    scaffold_value: &ThreePaneScaffoldValue,
    measurables: [&Option<Rc<dyn Measurable>>; 3],
    directive: &PaneScaffoldDirective,
    density: Density,
    scaffold_size: IntSize,
    predicate: &dyn Fn(&PaneAdaptedValue) -> bool,
) -> Vec<PaneMeasurable> {
    let mut list = Vec::new();
    pane_order.for_each(&mut |role| {
        let pane_value = scaffold_value.get_ref(role);
        if predicate(pane_value) {
            let (measurable, priority) = match role {
                ThreePaneScaffoldRole::Primary => (measurables[0], ThreePaneScaffoldDefaults::PrimaryPanePriority),
                ThreePaneScaffoldRole::Secondary => (measurables[1], ThreePaneScaffoldDefaults::SecondaryPanePriority),
                ThreePaneScaffoldRole::Tertiary => (measurables[2], ThreePaneScaffoldDefaults::TertiaryPanePriority),
            };
            if let Some(measurable) = measurable {
                list.push(PaneMeasurable::new(
                    measurable.clone(),
                    priority,
                    role,
                    pane_value.clone(),
                    density.round_to_px(directive.default_pane_preferred_width),
                    density.round_to_px(directive.default_pane_preferred_height),
                    density,
                    scaffold_size,
                ));
            }
        }
    });
    list
}

impl PlaceContext<'_> {
    fn measure_and_place_partitions_in_bounds(
        &self,
        bounds: IntRect,
        vertical_spacer_size: i32,
        horizontal_spacer_size: i32,
        expanded_panes: &[PaneMeasurable],
        reflowed_panes: &[PaneMeasurable],
    ) {
        if expanded_panes.is_empty() {
            return;
        }
        let allocatable_width = bounds.width() - (expanded_panes.len() as i32 - 1) * vertical_spacer_size;
        let total_preferred_width: i32 = expanded_panes.iter().map(|p| p.measuring_width.get()).sum();
        if allocatable_width > total_preferred_width {
            // Allocate the remaining space to the pane with the highest priority. (Kotlin's `maxBy`: the first of
            // the highest)
            let mut highest = &expanded_panes[0];
            for pane in &expanded_panes[1..] {
                if pane.priority > highest.priority {
                    highest = pane;
                }
            }
            highest.measuring_width.set(highest.measuring_width.get() + allocatable_width - total_preferred_width);
        } else if allocatable_width < total_preferred_width {
            // Scale down all panes to fit in the available space.
            let scale = allocatable_width as f32 / total_preferred_width as f32;
            for pane in expanded_panes {
                pane.measuring_width.set((pane.measuring_width.get() as f32 * scale) as i32);
            }
        }
        let mut position_x = bounds.left;
        for expanded_pane in expanded_panes {
            let partition_width = expanded_pane.measuring_width.get();
            self.measure_and_place_panes_in_partition(
                IntRect {
                    left: position_x,
                    top: bounds.top,
                    right: position_x + partition_width,
                    bottom: bounds.bottom,
                },
                horizontal_spacer_size,
                expanded_pane,
                reflowed_panes,
            );
            position_x += partition_width + vertical_spacer_size;
        }
    }

    fn measure_and_place_panes_in_partition(
        &self,
        partition_bounds: IntRect,
        horizontal_spacer_size: i32,
        expanded_pane: &PaneMeasurable,
        reflowed_panes: &[PaneMeasurable],
    ) {
        let reflowed_pane = reflowed_panes.first();
        let reflows_under_it = reflowed_pane
            .and_then(|r| r.value.reflow_under())
            .is_some_and(|under| under.as_three_pane() == Some(expanded_pane.role));
        if reflows_under_it {
            let reflowed_pane = reflowed_pane.expect("checked above");
            // Measure the reflowed pane and adjust the expanded pane's height
            // TODO(conradchen): Avoid hinges
            let available_height = partition_bounds.height() - horizontal_spacer_size;
            expanded_pane
                .measuring_height
                .set((available_height - reflowed_pane.measuring_height.get()).max(available_height / 2));
            reflowed_pane.measuring_height.set(available_height - expanded_pane.measuring_height.get());
        } else {
            expanded_pane.measuring_height.set(partition_bounds.height());
        }
        if let Some(reflowed) = reflowed_pane {
            self.measure_and_place_pane(
                IntRect {
                    top: partition_bounds.top + expanded_pane.measuring_height.get() + horizontal_spacer_size,
                    ..partition_bounds
                },
                reflowed,
            );
        }
        self.measure_and_place_pane(
            IntRect { bottom: partition_bounds.top + expanded_pane.measuring_height.get(), ..partition_bounds },
            expanded_pane,
        );
    }

    fn measure_and_place_pane(&self, pane_bounds: IntRect, measurable: &PaneMeasurable) {
        measurable.measured_bounds.set(Some(pane_bounds));
        self.record_measure_result(measurable);
    }

    fn place_levitated_panes(
        &self,
        measurables: &[PaneMeasurable],
        scaffold_bounds: IntRect,
        layout_direction: LayoutDirection,
    ) {
        for it in measurables {
            let measuring_width = it.measuring_width.get().min(scaffold_bounds.width());
            let measuring_height = it.measuring_height.get().min(scaffold_bounds.height());
            // (Compose's dragToResizeState is always null: "uncomment this when publish the feature")
            let pane_size = IntSize::new(measuring_width, measuring_height);
            let alignment = it.value.levitated_alignment().unwrap_or(Alignment::Center);
            let offset = alignment.align(pane_size, scaffold_bounds.size(), layout_direction);
            self.measure_and_place_pane(
                IntRect {
                    left: offset.x,
                    top: offset.y,
                    right: offset.x + pane_size.width,
                    bottom: offset.y + pane_size.height,
                },
                it,
            );
        }
    }

    fn place_hidden_panes(&self, measurables: &[PaneMeasurable]) {
        // When panes are being hidden, apply each pane's width and position from the cache to maintain those
        // before it's hidden by the AnimatedVisibility.
        for it in measurables {
            if !it.is_animated_pane {
                // When panes are not animated, we don't need to measure and place them. (Compose's `return`: the
                // rest of the hidden panes aren't either)
                return;
            }
            let data = self.provider.get(it.role);
            it.measured_bounds.set(Some(IntRect {
                left: data.target_left(),
                top: data.target_top(),
                right: data.target_right(),
                bottom: data.target_bottom(),
            }));
            it.z_index.set(data.z_index.get() + ThreePaneScaffoldDefaults::HiddenPaneZIndexOffset);
        }
    }

    fn record_measure_result(&self, pane: &PaneMeasurable) {
        let data = self.provider.get(pane.role);
        if !self.is_looking_ahead && pane.value != PaneAdaptedValue::Hidden {
            // Save zIndex for visible panes, so when it's hidden we can keep them at the same zIndex level.
            data.z_index.set(pane.z_index.get());
            return;
        }
        if let Some(bounds) = pane.measured_bounds.get() {
            if !data.is_origin_size_and_position_set.get() {
                // Only the very first measurement of a change is its origin.
                data.set_origin_size(if data.is_target_size_and_position_set.get() {
                    data.target_size()
                } else {
                    bounds.size()
                });
                data.set_origin_position(if data.is_target_size_and_position_set.get() {
                    data.target_position()
                } else {
                    bounds.top_left()
                });
                data.is_origin_size_and_position_set.set(true);
            }
            data.set_target_size(bounds.size());
            data.set_target_position(bounds.top_left());
            data.is_target_size_and_position_set.set(true);
        }
        let _ = self.directive;
    }
}

fn measure_and_place_drag_handle_if_needed(
    drag_handle: &DragHandleMeasurable,
    content_bounds: IntRect,
    min_horizontal_margin: i32,
    offset_x: i32,
) {
    if offset_x == PaneExpansionState::Unspecified {
        return;
    }
    let clamped_offset_x =
        offset_x.clamp(content_bounds.left + min_horizontal_margin, content_bounds.right - min_horizontal_margin);
    let applied_horizontal_margin =
        (clamped_offset_x - content_bounds.left).min(content_bounds.right - clamped_offset_x);
    // When dragged to the end, keep a consistent margin from the middle of the handle to the layout's edge: the
    // touch target is expanded (and shifted) so that it stays the min touch target size.
    let min_drag_handle_width = if applied_horizontal_margin < drag_handle.min_touch_target_size / 2 {
        2 * (drag_handle.min_touch_target_size - applied_horizontal_margin)
    } else {
        drag_handle.min_touch_target_size
    };
    drag_handle.is_visible.set(true);
    drag_handle.min_width.set(min_drag_handle_width);
    drag_handle.max_height.set(content_bounds.height());
    drag_handle.placed_position_center.set(Some(IntOffset::new(clamped_offset_x, content_bounds.center().y)));
}

fn get_spacer_middle_offset_x(pane_left: &PaneMeasurable, pane_right: &PaneMeasurable) -> i32 {
    match (pane_left.measured_and_placed(), pane_right.measured_and_placed()) {
        (true, true) => {
            (pane_left.placed_position_x() + pane_left.measured_width() + pane_right.placed_position_x()) / 2
        }
        (true, false) => pane_left.placed_position_x() + pane_left.measured_width(),
        (false, true) => 0,
        (false, false) => PaneExpansionState::Unspecified,
    }
}

/// A pane as the policy measures it (Compose's private `PaneMeasurable`).
struct PaneMeasurable {
    measurable: Rc<dyn Measurable>,
    priority: i32,
    role: ThreePaneScaffoldRole,
    value: PaneAdaptedValue,
    measuring_width: Cell<i32>,
    measuring_height: Cell<i32>,
    is_animated_pane: bool,
    z_index: Cell<f32>,
    measured_bounds: Cell<Option<IntRect>>,
}

impl PaneMeasurable {
    #[allow(clippy::too_many_arguments)]
    fn new(
        measurable: Rc<dyn Measurable>,
        priority: i32,
        role: ThreePaneScaffoldRole,
        value: PaneAdaptedValue,
        default_preferred_width: i32,
        default_preferred_height: i32,
        density: Density,
        scaffold_size: IntSize,
    ) -> PaneMeasurable {
        let data = pane_scaffold_parent_data(&*measurable).unwrap_or_default();
        let measuring_width = if data.preferred_width_internal.dp.is_specified() {
            density.round_to_px(data.preferred_width_internal.dp)
        } else if data.preferred_width_internal.proportion.is_finite() {
            (scaffold_size.width as f32 * data.preferred_width_internal.proportion) as i32
        } else {
            default_preferred_width
        };
        let measuring_height = if data.preferred_height_internal.dp.is_specified() {
            density.round_to_px(data.preferred_height_internal.dp)
        } else if data.preferred_height_internal.proportion.is_finite() {
            (scaffold_size.height as f32 * data.preferred_height_internal.proportion) as i32
        } else {
            default_preferred_height
        };
        let z_index = if value.is_levitated() { ThreePaneScaffoldDefaults::LevitatedPaneZIndex } else { 0.0 };
        PaneMeasurable {
            measurable,
            priority,
            role,
            value,
            measuring_width: Cell::new(measuring_width),
            measuring_height: Cell::new(measuring_height),
            is_animated_pane: data.is_animated_pane,
            z_index: Cell::new(z_index),
            measured_bounds: Cell::new(None),
        }
    }

    fn measured_width(&self) -> i32 {
        self.measured_bounds.get().map_or(0, |b| b.width())
    }

    fn measured_height(&self) -> i32 {
        self.measured_bounds.get().map_or(0, |b| b.height())
    }

    fn placed_position_x(&self) -> i32 {
        self.measured_bounds.get().map_or(0, |b| b.left)
    }

    fn placed_position_y(&self) -> i32 {
        self.measured_bounds.get().map_or(0, |b| b.top)
    }

    fn measured_and_placed(&self) -> bool {
        self.measured_bounds.get().is_some()
    }

    fn do_measure_and_place(&self, p: &PlacementScope) {
        let placeable = self.measurable.measure(Constraints::fixed(self.measured_width(), self.measured_height()));
        p.place_at(&placeable, IntOffset::new(self.placed_position_x(), self.placed_position_y()), self.z_index.get());
    }
}

/// The drag handle as the policy measures it (Compose's private `DragHandleMeasurable`).
struct DragHandleMeasurable {
    measurable: Rc<dyn Measurable>,
    min_touch_target_size: i32,
    is_visible: Cell<bool>,
    min_width: Cell<i32>,
    max_height: Cell<i32>,
    placed_position_center: Cell<Option<IntOffset>>,
}

impl DragHandleMeasurable {
    fn new(measurable: Rc<dyn Measurable>, density: Density) -> DragHandleMeasurable {
        let min_touch_target_size = density.round_to_px(min_touch_target_size(&*measurable));
        DragHandleMeasurable {
            measurable,
            min_touch_target_size,
            is_visible: Cell::new(false),
            min_width: Cell::new(0),
            max_height: Cell::new(i32::MAX),
            placed_position_center: Cell::new(None),
        }
    }

    fn do_measure_and_place(&self, p: &PlacementScope) {
        if !self.is_visible.get() {
            return;
        }
        let placeable = self.measurable.measure(Constraints::new(
            self.min_width.get(),
            Constraints::Infinity,
            0,
            self.max_height.get(),
        ));
        let center = self.placed_position_center.get().unwrap_or(IntOffset::Zero);
        p.place_at(
            &placeable,
            IntOffset::new(center.x - placeable.width() / 2, center.y - placeable.height() / 2),
            0.0,
        );
    }
}

/// The scaffold's internal defaults (Compose's internal `ThreePaneScaffoldDefaults`).
pub(crate) struct ThreePaneScaffoldDefaults;

#[allow(dead_code)]
impl ThreePaneScaffoldDefaults {
    // TODO(conradchen): consider declaring a value class for priority
    pub(crate) const PrimaryPanePriority: i32 = 10;
    pub(crate) const SecondaryPanePriority: i32 = 5;
    pub(crate) const TertiaryPanePriority: i32 = 1;

    /// The strategies of the three panes, each hide by default (Compose's internal `adaptStrategies`).
    pub(crate) fn adapt_strategies(
        primary_pane_adapt_strategy: super::pane_adapted_value::AdaptStrategy,
        secondary_pane_adapt_strategy: super::pane_adapted_value::AdaptStrategy,
        tertiary_pane_adapt_strategy: super::pane_adapted_value::AdaptStrategy,
    ) -> super::pane_adapted_value::ThreePaneScaffoldAdaptStrategies {
        super::pane_adapted_value::ThreePaneScaffoldAdaptStrategies(
            primary_pane_adapt_strategy,
            secondary_pane_adapt_strategy,
            tertiary_pane_adapt_strategy,
        )
    }

    pub(crate) const LevitatedPaneZIndex: f32 = 1.0;
    pub(crate) const HiddenPaneZIndexOffset: f32 = -0.1;
    pub(crate) const MinPaneWidth: Dp = Dp(48.0);
    pub(crate) const MinPaneHeight: Dp = Dp(48.0);
}
