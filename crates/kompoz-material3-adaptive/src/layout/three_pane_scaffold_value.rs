//! A three-pane scaffold's value and orders, as Compose's adaptive-layout ThreePaneScaffoldValue.kt,
//! PaneScaffoldValue.kt, PaneScaffoldHorizontalOrder.kt, ThreePaneScaffoldHorizontalOrder.kt and
//! ThreePaneScaffoldDestinationItem.kt (1.2.0): [`ThreePaneScaffoldValue`] (each pane's adapted value) and how it is
//! calculated from the directive, the strategies and the destinations ([`calculate_three_pane_scaffold_value`]),
//! [`ThreePaneScaffoldHorizontalOrder`], [`ThreePaneScaffoldDestinationItem`].

use super::pane_adapted_value::{AdaptStrategy, PaneAdaptedValue, ThreePaneScaffoldAdaptStrategies};
use super::pane_expansion_state::{PaneExpansionStateKey, PaneExpansionStateKeyProvider};
use super::pane_scaffold_role::{PaneScaffoldRole, ThreePaneScaffoldRole};
use kompoz_graphics::LayoutDirection;
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{Saveable, SavedValue};
use std::fmt;
use std::sync::OnceLock;

/// The adapted values of a pane scaffold's panes (Compose's sealed interface `PaneScaffoldValue<Role>`).
pub trait PaneScaffoldValue {
    /// The roles it indexes its panes by.
    type Role: PaneScaffoldRole + Copy;

    /// The adapted value of the pane of `role` (Kotlin's operator `get`).
    fn get(&self, role: Self::Role) -> PaneAdaptedValue;
}

/// The horizontal order of a pane scaffold's panes, a 1-to-1 mapping of its roles to their indices (Compose's
/// sealed interface `PaneScaffoldHorizontalOrder<Role>`).
pub trait PaneScaffoldHorizontalOrder {
    /// The roles it orders.
    type Role: PaneScaffoldRole + Copy;

    /// The number of panes in the order (Compose's `size`).
    fn size(&self) -> i32;

    /// The index of `role` in the order (-1 if it isn't in it).
    fn index_of(&self, role: Self::Role) -> i32;

    /// `action` of each pane in the order.
    fn for_each(&self, action: &mut dyn FnMut(Self::Role));

    /// `action` of each pane in the order, with its index.
    fn for_each_indexed(&self, action: &mut dyn FnMut(i32, Self::Role));

    /// `action` of each pane in the order, with its index, from the last.
    fn for_each_indexed_reversed(&self, action: &mut dyn FnMut(i32, Self::Role));
}

/// The horizontal order of a three-pane scaffold's panes from start to end (Compose's
/// `ThreePaneScaffoldHorizontalOrder`). Equal by its panes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ThreePaneScaffoldHorizontalOrder {
    pub(crate) first_pane: ThreePaneScaffoldRole,
    pub(crate) second_pane: ThreePaneScaffoldRole,
    pub(crate) third_pane: ThreePaneScaffoldRole,
}

impl ThreePaneScaffoldHorizontalOrder {
    /// Compose's internal constructor: panics unless the panes are distinct (Kotlin's `require`).
    pub(crate) fn new(
        first_pane: ThreePaneScaffoldRole,
        second_pane: ThreePaneScaffoldRole,
        third_pane: ThreePaneScaffoldRole,
    ) -> ThreePaneScaffoldHorizontalOrder {
        assert!(
            first_pane != second_pane && second_pane != third_pane && first_pane != third_pane,
            "invalid ThreePaneScaffoldHorizontalOrder({first_pane}, {second_pane}, {third_pane}) - panes must be unique"
        );
        ThreePaneScaffoldHorizontalOrder { first_pane, second_pane, third_pane }
    }

    /// The pane at `index` (Kotlin's operator `get`); panics out of 0..3 (Kotlin's IndexOutOfBoundsException).
    pub fn get(&self, index: i32) -> ThreePaneScaffoldRole {
        match index {
            0 => self.first_pane,
            1 => self.second_pane,
            2 => self.third_pane,
            _ => panic!("Invalid pane index {index}"),
        }
    }

    /// The order left to right: reversed right to left (Compose's internal `toLtrOrder`).
    pub(crate) fn to_ltr_order(self, layout_direction: LayoutDirection) -> ThreePaneScaffoldHorizontalOrder {
        if layout_direction == LayoutDirection::Rtl {
            ThreePaneScaffoldHorizontalOrder::new(self.third_pane, self.second_pane, self.first_pane)
        } else {
            self
        }
    }
}

impl PaneScaffoldHorizontalOrder for ThreePaneScaffoldHorizontalOrder {
    type Role = ThreePaneScaffoldRole;

    fn size(&self) -> i32 {
        3
    }

    fn index_of(&self, role: ThreePaneScaffoldRole) -> i32 {
        if role == self.first_pane {
            0
        } else if role == self.second_pane {
            1
        } else if role == self.third_pane {
            2
        } else {
            -1
        }
    }

    fn for_each(&self, action: &mut dyn FnMut(ThreePaneScaffoldRole)) {
        action(self.first_pane);
        action(self.second_pane);
        action(self.third_pane);
    }

    fn for_each_indexed(&self, action: &mut dyn FnMut(i32, ThreePaneScaffoldRole)) {
        action(0, self.first_pane);
        action(1, self.second_pane);
        action(2, self.third_pane);
    }

    fn for_each_indexed_reversed(&self, action: &mut dyn FnMut(i32, ThreePaneScaffoldRole)) {
        action(2, self.third_pane);
        action(1, self.second_pane);
        action(0, self.first_pane);
    }
}

/// A navigation destination of a three-pane scaffold (Compose's `ThreePaneScaffoldDestinationItem<T>`): its pane
/// and the key of its content, if any (a value saved with the navigator's state: [`Saveable`]). Equal by both.
#[derive(Clone, PartialEq, Debug, kompoz_runtime::KotlinProperties)]
pub struct ThreePaneScaffoldDestinationItem<T> {
    pub pane: ThreePaneScaffoldRole,
    pub content_key: Option<T>,
}

/// Kotlin: `ThreePaneScaffoldDestinationItem(pane: ThreePaneScaffoldRole, contentKey: T? = null)`
pub fn ThreePaneScaffoldDestinationItem<T>(
    pane: ThreePaneScaffoldRole,
    content_key: Option<T>,
) -> ThreePaneScaffoldDestinationItem<T> {
    ThreePaneScaffoldDestinationItem { pane, content_key }
}

/// Kotlin's `toString`: `ThreePaneScaffoldDestinationItem(pane=Primary, contentKey=3)`.
impl<T: KtToString> fmt::Display for ThreePaneScaffoldDestinationItem<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.content_key.as_ref().map_or("null".to_string(), |k| k.kt_to_string());
        write!(f, "ThreePaneScaffoldDestinationItem(pane={}, contentKey={key})", self.pane)
    }
}

impl<T: KtToString> KtToString for ThreePaneScaffoldDestinationItem<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// A three-pane scaffold's value (Compose's `ThreePaneScaffoldValue`): each pane's adapted value, and the role of
/// the current destination it was calculated for (if any). Equal by the three values.
#[derive(Clone)]
pub struct ThreePaneScaffoldValue {
    pub primary: PaneAdaptedValue,
    pub secondary: PaneAdaptedValue,
    pub tertiary: PaneAdaptedValue,
    pub(crate) current_destination: Option<ThreePaneScaffoldRole>,
    /// (Compose's `by lazy` key)
    key: OnceLock<PaneExpansionStateKey>,
}

/// Kotlin: `ThreePaneScaffoldValue(primary: PaneAdaptedValue, secondary: PaneAdaptedValue, tertiary: PaneAdaptedValue)`
pub fn ThreePaneScaffoldValue(
    primary: PaneAdaptedValue,
    secondary: PaneAdaptedValue,
    tertiary: PaneAdaptedValue,
) -> ThreePaneScaffoldValue {
    ThreePaneScaffoldValue::new(primary, secondary, tertiary, None)
}

impl ThreePaneScaffoldValue {
    /// Compose's internal constructor (with the current destination).
    pub(crate) fn new(
        primary: PaneAdaptedValue,
        secondary: PaneAdaptedValue,
        tertiary: PaneAdaptedValue,
        current_destination: Option<ThreePaneScaffoldRole>,
    ) -> ThreePaneScaffoldValue {
        ThreePaneScaffoldValue { primary, secondary, tertiary, current_destination, key: OnceLock::new() }
    }

    /// How many panes are expanded (Compose's internal `expandedCount`).
    pub(crate) fn expanded_count(&self) -> i32 {
        let mut count = 0;
        self.for_each(|_, value| {
            if *value == PaneAdaptedValue::Expanded {
                count += 1;
            }
        });
        count
    }

    /// `action` of each role and its value, primary first (Compose's internal `forEach`).
    pub(crate) fn for_each(&self, mut action: impl FnMut(ThreePaneScaffoldRole, &PaneAdaptedValue)) {
        action(ThreePaneScaffoldRole::Primary, &self.primary);
        action(ThreePaneScaffoldRole::Secondary, &self.secondary);
        action(ThreePaneScaffoldRole::Tertiary, &self.tertiary);
    }

    /// The value of the pane of `role` (Kotlin's operator `get`), by reference.
    pub fn get_ref(&self, role: ThreePaneScaffoldRole) -> &PaneAdaptedValue {
        match role {
            ThreePaneScaffoldRole::Primary => &self.primary,
            ThreePaneScaffoldRole::Secondary => &self.secondary,
            ThreePaneScaffoldRole::Tertiary => &self.tertiary,
        }
    }

    /// Whether the pane of `role` takes the user's interaction (Compose's internal `isInteractable`): not hidden,
    /// and a levitated one, or any while no levitated pane has a scrim.
    pub(crate) fn is_interactable(&self, role: ThreePaneScaffoldRole) -> bool {
        match self.get_ref(role) {
            PaneAdaptedValue::Hidden => false,
            PaneAdaptedValue::Levitated { .. } => true,
            _ => !self.has_levitated_pane_with_scrim(),
        }
    }

    fn has_levitated_pane_with_scrim(&self) -> bool {
        let mut found = false;
        self.for_each(|_, value| found |= value.levitated_scrim().is_some());
        found
    }
}

impl PaneScaffoldValue for ThreePaneScaffoldValue {
    type Role = ThreePaneScaffoldRole;

    fn get(&self, role: ThreePaneScaffoldRole) -> PaneAdaptedValue {
        self.get_ref(role).clone()
    }
}

/// The key of the expansion state of two expanded panes, the default key otherwise.
impl PaneExpansionStateKeyProvider for ThreePaneScaffoldValue {
    fn pane_expansion_state_key(&self) -> PaneExpansionStateKey {
        *self.key.get_or_init(|| {
            if self.expanded_count() != 2 {
                PaneExpansionStateKey::Default
            } else {
                let mut expanded = Vec::with_capacity(2);
                self.for_each(|role, value| {
                    if *value == PaneAdaptedValue::Expanded {
                        expanded.push(role);
                    }
                });
                PaneExpansionStateKey::TwoPane(TwoPaneExpansionStateKeyImpl {
                    first_expanded_pane: expanded[0],
                    second_expanded_pane: expanded[1],
                })
            }
        })
    }
}

impl PartialEq for ThreePaneScaffoldValue {
    fn eq(&self, other: &Self) -> bool {
        self.primary == other.primary && self.secondary == other.secondary && self.tertiary == other.tertiary
    }
}

/// Kotlin's `toString`: `ThreePaneScaffoldValue(primary=…, secondary=…, tertiary=…)`.
impl fmt::Display for ThreePaneScaffoldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ThreePaneScaffoldValue(primary={}, secondary={}, tertiary={})",
            self.primary, self.secondary, self.tertiary
        )
    }
}

impl fmt::Debug for ThreePaneScaffoldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl KtToString for ThreePaneScaffoldValue {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// The key of two expanded panes' expansion state (Compose's internal `TwoPaneExpansionStateKeyImpl`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TwoPaneExpansionStateKeyImpl {
    pub(crate) first_expanded_pane: ThreePaneScaffoldRole,
    pub(crate) second_expanded_pane: ThreePaneScaffoldRole,
}

/// Saved as the list of its two roles (Compose's `TwoPaneExpansionStateKeyImpl.saver()`).
impl Saveable for TwoPaneExpansionStateKeyImpl {
    fn save(&self) -> SavedValue {
        SavedValue::List(std::sync::Arc::new(vec![self.first_expanded_pane.save(), self.second_expanded_pane.save()]))
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        match saved {
            SavedValue::List(items) if items.len() == 2 => Some(TwoPaneExpansionStateKeyImpl {
                first_expanded_pane: ThreePaneScaffoldRole::restore(&items[0])?,
                second_expanded_pane: ThreePaneScaffoldRole::restore(&items[1])?,
            }),
            _ => None,
        }
    }
}

/// The scaffold's value for its current destination (Compose's `calculateThreePaneScaffoldValue(…,
/// currentDestination, …)`): the destination history of it alone.
///
/// Kotlin: `calculateThreePaneScaffoldValue(maxHorizontalPartitions: Int, adaptStrategies: ThreePaneScaffoldAdaptStrategies, currentDestination: ThreePaneScaffoldDestinationItem<*>?, maxVerticalPartitions: Int = 1): ThreePaneScaffoldValue`
#[kompoz_runtime::named]
pub fn calculate_three_pane_scaffold_value_of_destination<T: 'static>(
    max_horizontal_partitions: i32,
    adapt_strategies: &ThreePaneScaffoldAdaptStrategies,
    current_destination: Option<&ThreePaneScaffoldDestinationItem<T>>,
    #[default(1)] max_vertical_partitions: i32,
) -> ThreePaneScaffoldValue {
    let history: Vec<ThreePaneScaffoldRole> = current_destination.map(|d| d.pane).into_iter().collect();
    calculate_three_pane_scaffold_value_of_panes(
        max_horizontal_partitions,
        adapt_strategies,
        &history,
        max_vertical_partitions,
    )
}

/// The scaffold's value (Compose's `calculateThreePaneScaffoldValue`): the current destination (the history's
/// last) is levitated if its strategy levitates; then by priority — the destinations from the most recent, then
/// primary, secondary, tertiary — panes are expanded while there are horizontal partitions, a pane reflowing
/// under an expanded (or expandable) pane in a single-pane layout of more vertical partitions is reflowed (once);
/// the rest are hidden.
///
/// Kotlin: `calculateThreePaneScaffoldValue(maxHorizontalPartitions: Int, adaptStrategies: ThreePaneScaffoldAdaptStrategies, destinationHistory: List<ThreePaneScaffoldDestinationItem<*>>, maxVerticalPartitions: Int = 1): ThreePaneScaffoldValue`
#[kompoz_runtime::named]
pub fn calculate_three_pane_scaffold_value<T: 'static>(
    max_horizontal_partitions: i32,
    adapt_strategies: &ThreePaneScaffoldAdaptStrategies,
    destination_history: &[ThreePaneScaffoldDestinationItem<T>],
    #[default(1)] max_vertical_partitions: i32,
) -> ThreePaneScaffoldValue {
    let history: Vec<ThreePaneScaffoldRole> = destination_history.iter().map(|d| d.pane).collect();
    calculate_three_pane_scaffold_value_of_panes(
        max_horizontal_partitions,
        adapt_strategies,
        &history,
        max_vertical_partitions,
    )
}

/// What both forms calculate by: only the destinations' panes are read (Kotlin's star projection).
pub(crate) fn calculate_three_pane_scaffold_value_of_panes(
    max_horizontal_partitions: i32,
    adapt_strategies: &ThreePaneScaffoldAdaptStrategies,
    destination_history: &[ThreePaneScaffoldRole],
    max_vertical_partitions: i32,
) -> ThreePaneScaffoldValue {
    let mut expanded_count = 0;
    let mut values: [Option<PaneAdaptedValue>; 3] = [None, None, None];
    let index = |role: ThreePaneScaffoldRole| role as usize;

    let mut check_reflowed_pane = max_horizontal_partitions == 1
        && max_vertical_partitions > 1
        && ThreePaneScaffoldRole::ENTRIES.iter().any(|r| adapt_strategies.get(*r).is_reflow());

    let current_destination = destination_history.last().copied();

    // Only levitate a pane when it is the current destination
    if let Some(pane) = current_destination
        && let AdaptStrategy::Levitate { alignment, scrim } = adapt_strategies.get(pane)
    {
        values[index(pane)] = Some(PaneAdaptedValue::Levitated { alignment: alignment.clone(), scrim: scrim.clone() });
    }

    // (forEachPaneByPriority: the history from the most recent, then the panes by priority)
    let by_priority = destination_history.iter().rev().copied().chain(ThreePaneScaffoldRole::ENTRIES);
    for pane in by_priority {
        let has_available_partition = expanded_count < max_horizontal_partitions;
        if !has_available_partition && !check_reflowed_pane {
            break; // No need to check more panes
        }
        if values[index(pane)].is_some() {
            continue; // Pane already adapted
        }
        let mut reflowed_pane: Option<ThreePaneScaffoldRole> = None;
        let mut anchor_pane = pane;
        let mut anchor_pane_value: Option<PaneAdaptedValue> = None;
        if check_reflowed_pane
            && let AdaptStrategy::Reflow { reflow_under } = adapt_strategies.get(pane)
            && let Some(under) = reflow_under.as_three_pane()
        {
            reflowed_pane = Some(pane);
            anchor_pane = under;
            anchor_pane_value = values[index(anchor_pane)].clone();
        }
        match anchor_pane_value {
            None => {
                if adapt_strategies.get(anchor_pane).is_levitate() {
                    continue; // The anchor pane can only be levitated
                } else if has_available_partition {
                    // Expand the anchor pane to reflow the pane
                    values[index(anchor_pane)] = Some(PaneAdaptedValue::Expanded);
                    expanded_count += 1;
                } else {
                    continue; // Cannot expand the anchor pane
                }
            }
            Some(PaneAdaptedValue::Expanded) => {} // Anchor pane is expanded, do nothing
            Some(_) => continue,                   // Anchor pane is not expanded
        }
        if let Some(reflowed) = reflowed_pane {
            values[index(reflowed)] = Some(PaneAdaptedValue::reflowed(anchor_pane));
            check_reflowed_pane = false;
        }
    }
    let [primary, secondary, tertiary] = values;
    ThreePaneScaffoldValue::new(
        primary.unwrap_or(PaneAdaptedValue::Hidden),
        secondary.unwrap_or(PaneAdaptedValue::Hidden),
        tertiary.unwrap_or(PaneAdaptedValue::Hidden),
        current_destination,
    )
}
