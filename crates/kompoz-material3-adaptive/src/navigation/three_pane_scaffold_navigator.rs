//! A three-pane scaffold's navigation, as Compose's adaptive-navigation ThreePaneScaffoldNavigator.kt and
//! BackNavigationBehavior.kt (1.2.0): the navigator a scaffold is driven by ([`ThreePaneScaffoldNavigator`]: its
//! directive, state and value, the destination history it navigates and goes back in, seeking back for a predictive
//! back gesture), the remembered navigators of the list-detail and supporting pane scaffolds, and how far back goes
//! ([`BackNavigationBehavior`]).

use crate::layout::{
    ListDetailPaneScaffoldDefaults, ListDetailPaneScaffoldRole, MutableThreePaneScaffoldState, PaneScaffoldDirective,
    SupportingPaneScaffoldDefaults, SupportingPaneScaffoldRole, ThreePaneScaffoldAdaptStrategies,
    ThreePaneScaffoldDestinationItem, ThreePaneScaffoldRole, ThreePaneScaffoldState, ThreePaneScaffoldValue,
    calculate_pane_scaffold_directive, calculate_three_pane_scaffold_value,
    calculate_three_pane_scaffold_value_of_destination,
};
use crate::window_adaptive_info::current_window_adaptive_info;
use kompoz_foundation::gestures::LocalFuture;
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{
    DerivedState, MutableState, Saveable, SavedValue, Saver, SnapshotStateList, derived_state_of, kompozable,
    list_saver, mutable_state_of, named_args, remember_saveable_with,
};
use std::fmt;
use std::rc::Rc;

/// How far back navigation goes (Compose's value class `BackNavigationBehavior`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BackNavigationBehavior(&'static str);

impl BackNavigationBehavior {
    /// Back to the previous destination, whatever changes.
    pub const PopLatest: BackNavigationBehavior = BackNavigationBehavior("PopLatest");
    /// Back to the latest destination whose scaffold value differs from the current one (what the user sees
    /// changes).
    pub const PopUntilScaffoldValueChange: BackNavigationBehavior =
        BackNavigationBehavior("PopUntilScaffoldValueChange");
    /// Back to the latest destination of another pane.
    pub const PopUntilCurrentDestinationChange: BackNavigationBehavior =
        BackNavigationBehavior("PopUntilCurrentDestinationChange");
    /// Back to the latest destination of another content key, or whose scaffold value differs.
    pub const PopUntilContentChange: BackNavigationBehavior = BackNavigationBehavior("PopUntilContentChange");
}

/// Kotlin's `toString`: its name.
impl fmt::Display for BackNavigationBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl KtToString for BackNavigationBehavior {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self.0);
    }
}

/// What a content key of a navigator's destinations is: a value saved with the UI's state (Compose's "storable in
/// a Bundle"), held in snapshot state.
pub trait NavigatorContentKey: Clone + PartialEq + Saveable + 'static {}

impl<T: Clone + PartialEq + Saveable + 'static> NavigatorContentKey for T {}

/// The navigator of a three-pane scaffold (Compose's interface `ThreePaneScaffoldNavigator<T>`): the directive and
/// state the scaffold lays out by, its value (of the destination history), the history it navigates to and back in.
/// [`remember_list_detail_pane_scaffold_navigator`] and [`remember_supporting_pane_scaffold_navigator`] make the
/// default one.
pub trait ThreePaneScaffoldNavigator<T> {
    /// The directive the scaffold lays out by.
    fn scaffold_directive(&self) -> PaneScaffoldDirective;

    /// The state the scaffold is moved by.
    fn scaffold_state(&self) -> Rc<dyn ThreePaneScaffoldState>;

    /// The scaffold's value of the current destination history.
    fn scaffold_value(&self) -> ThreePaneScaffoldValue;

    /// The scaffold's value going back by `back_navigation_behavior` would give (the current one if going back
    /// isn't possible).
    fn peek_previous_scaffold_value_with(
        &self,
        back_navigation_behavior: BackNavigationBehavior,
    ) -> ThreePaneScaffoldValue;

    /// The current destination, if any.
    fn current_destination(&self) -> Option<ThreePaneScaffoldDestinationItem<T>>;

    /// Whether the scaffold's value is calculated of the whole history (else of the current destination alone).
    fn is_destination_history_aware(&self) -> bool;

    /// Sets [`is_destination_history_aware`](Self::is_destination_history_aware).
    fn set_is_destination_history_aware(&self, value: bool);

    /// Navigates to `pane` with `content_key`, animating the scaffold there; done when it is there.
    fn navigate_to_with(&self, pane: ThreePaneScaffoldRole, content_key: Option<T>) -> LocalFuture<'static, ()>;

    /// Whether going back by `back_navigation_behavior` is possible.
    fn can_navigate_back_with(&self, back_navigation_behavior: BackNavigationBehavior) -> bool;

    /// Goes back by `back_navigation_behavior` (the history emptied if it can't), animating the scaffold; whether
    /// it could.
    fn navigate_back_with(&self, back_navigation_behavior: BackNavigationBehavior) -> LocalFuture<'static, bool>;

    /// Seeks the scaffold `fraction` of the way back (a predictive back gesture's progress); 0 goes back to the
    /// current value.
    fn seek_back_with(
        &self,
        back_navigation_behavior: BackNavigationBehavior,
        fraction: f32,
    ) -> LocalFuture<'static, ()>;
}

/// A navigator as the trait object it is shared as (what [`ThreePaneScaffoldNavigatorExt`] is implemented for).
pub type DynThreePaneScaffoldNavigator<T> = dyn ThreePaneScaffoldNavigator<T>;

/// The navigator's members with Kotlin's defaults (Kotlin's interface's defaults: `PopUntilScaffoldValueChange`,
/// no content key, a fraction of 1), what Kotlin calls: `navigator.navigate_back()`.
#[kompoz_runtime::named(trait ThreePaneScaffoldNavigatorExt)]
impl<T: NavigatorContentKey> DynThreePaneScaffoldNavigator<T> {
    /// Navigates to `pane` with `content_key` (see [`ThreePaneScaffoldNavigator::navigate_to_with`]).
    ///
    /// Kotlin: `navigateTo(pane: ThreePaneScaffoldRole, contentKey: T? = null)`
    fn navigate_to(
        &self,
        pane: ThreePaneScaffoldRole,
        #[default(None)] content_key: Option<T>,
    ) -> LocalFuture<'static, ()> {
        self.navigate_to_with(pane, content_key)
    }

    /// Whether going back is possible (see [`ThreePaneScaffoldNavigator::can_navigate_back_with`]).
    ///
    /// Kotlin: `canNavigateBack(backNavigationBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange): Boolean`
    fn can_navigate_back(
        &self,
        #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)]
        back_navigation_behavior: BackNavigationBehavior,
    ) -> bool {
        self.can_navigate_back_with(back_navigation_behavior)
    }

    /// Goes back (see [`ThreePaneScaffoldNavigator::navigate_back_with`]).
    ///
    /// Kotlin: `navigateBack(backNavigationBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange): Boolean`
    fn navigate_back(
        &self,
        #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)]
        back_navigation_behavior: BackNavigationBehavior,
    ) -> LocalFuture<'static, bool> {
        self.navigate_back_with(back_navigation_behavior)
    }

    /// Seeks back (see [`ThreePaneScaffoldNavigator::seek_back_with`]).
    ///
    /// Kotlin: `seekBack(backNavigationBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange, fraction: Float = 1.0f)`
    fn seek_back(
        &self,
        #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)]
        back_navigation_behavior: BackNavigationBehavior,
        #[default(1.0)] fraction: f32,
    ) -> LocalFuture<'static, ()> {
        self.seek_back_with(back_navigation_behavior, fraction)
    }

    /// The value going back would give (see [`ThreePaneScaffoldNavigator::peek_previous_scaffold_value_with`]).
    ///
    /// Kotlin: `peekPreviousScaffoldValue(backNavigationBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange): ThreePaneScaffoldValue`
    fn peek_previous_scaffold_value(
        &self,
        #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)]
        back_navigation_behavior: BackNavigationBehavior,
    ) -> ThreePaneScaffoldValue {
        self.peek_previous_scaffold_value_with(back_navigation_behavior)
    }
}

/// The list-detail scaffold's navigator, saved and restored with the UI's state (Compose's
/// `rememberListDetailPaneScaffoldNavigator<T>`): of `scaffold_directive` (by default the window's,
/// [`calculate_pane_scaffold_directive`]) and `adapt_strategies`, its history `initial_destination_history` at first
/// (the list pane).
///
/// Kotlin: `rememberListDetailPaneScaffoldNavigator(scaffoldDirective: PaneScaffoldDirective = calculatePaneScaffoldDirective(currentWindowAdaptiveInfo()), adaptStrategies: ThreePaneScaffoldAdaptStrategies = ListDetailPaneScaffoldDefaults.adaptStrategies(), isDestinationHistoryAware: Boolean = true, initialDestinationHistory: List<ThreePaneScaffoldDestinationItem<T>> = DefaultListDetailPaneHistory): ThreePaneScaffoldNavigator<T>`
#[kompozable]
pub fn remember_list_detail_pane_scaffold_navigator<T: NavigatorContentKey>(
    #[default(calculate_pane_scaffold_directive(&current_window_adaptive_info()))]
    scaffold_directive: PaneScaffoldDirective,
    #[default(named_args!(ListDetailPaneScaffoldDefaults.adapt_strategies()))]
    adapt_strategies: ThreePaneScaffoldAdaptStrategies,
    #[default(true)] is_destination_history_aware: bool,
    #[default(vec![ThreePaneScaffoldDestinationItem { pane: ListDetailPaneScaffoldRole::List, content_key: None }])]
    initial_destination_history: Vec<ThreePaneScaffoldDestinationItem<T>>,
) -> Rc<dyn ThreePaneScaffoldNavigator<T>> {
    remember_three_pane_scaffold_navigator(
        scaffold_directive,
        adapt_strategies,
        is_destination_history_aware,
        initial_destination_history,
    )
}

/// The list-detail scaffold's navigator of content keys of any saveable kind (Compose's non-generic
/// `rememberListDetailPaneScaffoldNavigator`, a navigator of `Any`: Kotlin's keys "storable in a Bundle" — here
/// the [`SavedValue`]s the UI's state is saved as). (Kotlin's syntax calls the generic one for both overloads: its
/// `rememberListDetailPaneScaffoldNavigator()` without a type argument takes the key type the navigator is used
/// with, as Rust infers it; this one is Rust's name for the `Any` overload.)
#[kompozable]
pub fn remember_list_detail_pane_scaffold_navigator_of_any(
    #[default(calculate_pane_scaffold_directive(&current_window_adaptive_info()))]
    scaffold_directive: PaneScaffoldDirective,
    #[default(named_args!(ListDetailPaneScaffoldDefaults.adapt_strategies()))]
    adapt_strategies: ThreePaneScaffoldAdaptStrategies,
    #[default(true)] is_destination_history_aware: bool,
) -> Rc<dyn ThreePaneScaffoldNavigator<SavedValue>> {
    remember_three_pane_scaffold_navigator(
        scaffold_directive,
        adapt_strategies,
        is_destination_history_aware,
        vec![ThreePaneScaffoldDestinationItem { pane: ListDetailPaneScaffoldRole::List, content_key: None }],
    )
}

/// The supporting pane scaffold's navigator, saved and restored with the UI's state (Compose's
/// `rememberSupportingPaneScaffoldNavigator<T>`): its history the main pane at first.
///
/// Kotlin: `rememberSupportingPaneScaffoldNavigator(scaffoldDirective: PaneScaffoldDirective = calculatePaneScaffoldDirective(currentWindowAdaptiveInfo()), adaptStrategies: ThreePaneScaffoldAdaptStrategies = SupportingPaneScaffoldDefaults.adaptStrategies(), isDestinationHistoryAware: Boolean = true, initialDestinationHistory: List<ThreePaneScaffoldDestinationItem<T>> = DefaultSupportingPaneHistory): ThreePaneScaffoldNavigator<T>`
#[kompozable]
pub fn remember_supporting_pane_scaffold_navigator<T: NavigatorContentKey>(
    #[default(calculate_pane_scaffold_directive(&current_window_adaptive_info()))]
    scaffold_directive: PaneScaffoldDirective,
    #[default(named_args!(SupportingPaneScaffoldDefaults.adapt_strategies()))]
    adapt_strategies: ThreePaneScaffoldAdaptStrategies,
    #[default(true)] is_destination_history_aware: bool,
    #[default(vec![ThreePaneScaffoldDestinationItem { pane: SupportingPaneScaffoldRole::Main, content_key: None }])]
    initial_destination_history: Vec<ThreePaneScaffoldDestinationItem<T>>,
) -> Rc<dyn ThreePaneScaffoldNavigator<T>> {
    remember_three_pane_scaffold_navigator(
        scaffold_directive,
        adapt_strategies,
        is_destination_history_aware,
        initial_destination_history,
    )
}

/// The supporting pane scaffold's navigator of content keys of any saveable kind (Compose's non-generic
/// `rememberSupportingPaneScaffoldNavigator`; see [`remember_list_detail_pane_scaffold_navigator_of_any`] for its
/// Kotlin syntax).
#[kompozable]
pub fn remember_supporting_pane_scaffold_navigator_of_any(
    #[default(calculate_pane_scaffold_directive(&current_window_adaptive_info()))]
    scaffold_directive: PaneScaffoldDirective,
    #[default(named_args!(SupportingPaneScaffoldDefaults.adapt_strategies()))]
    adapt_strategies: ThreePaneScaffoldAdaptStrategies,
    #[default(true)] is_destination_history_aware: bool,
) -> Rc<dyn ThreePaneScaffoldNavigator<SavedValue>> {
    remember_three_pane_scaffold_navigator(
        scaffold_directive,
        adapt_strategies,
        is_destination_history_aware,
        vec![ThreePaneScaffoldDestinationItem { pane: SupportingPaneScaffoldRole::Main, content_key: None }],
    )
}

/// The default navigator, saved (its history) and restored, given the directive, strategies and history
/// awareness of each composition (Compose's internal `rememberThreePaneScaffoldNavigator`).
#[kompozable]
fn remember_three_pane_scaffold_navigator<T: NavigatorContentKey>(
    scaffold_directive: PaneScaffoldDirective,
    adapt_strategies: ThreePaneScaffoldAdaptStrategies,
    is_destination_history_aware: bool,
    initial_destination_history: Vec<ThreePaneScaffoldDestinationItem<T>>,
) -> Rc<dyn ThreePaneScaffoldNavigator<T>> {
    let saver = DefaultThreePaneScaffoldNavigator::<T>::saver(
        scaffold_directive.clone(),
        adapt_strategies.clone(),
        is_destination_history_aware,
    );
    let (directive, strategies) = (scaffold_directive.clone(), adapt_strategies.clone());
    let navigator = remember_saveable_with(&saver, move || {
        DefaultThreePaneScaffoldNavigator::new(
            initial_destination_history,
            directive,
            strategies,
            is_destination_history_aware,
        )
    });
    navigator.inner.scaffold_directive.set(scaffold_directive);
    navigator.inner.adapt_strategies.set(adapt_strategies);
    navigator.inner.is_destination_history_aware.set(is_destination_history_aware);
    // (one shared navigator while it is the same one: what keys it, and Kotlin's identity, see the same)
    let handle = navigator.clone();
    kompoz_runtime::remember_keyed(SameNavigator(navigator), move || {
        Rc::new(handle) as Rc<dyn ThreePaneScaffoldNavigator<T>>
    })
}

/// A navigator as a key: the same one.
struct SameNavigator<T: NavigatorContentKey>(DefaultThreePaneScaffoldNavigator<T>);

impl<T: NavigatorContentKey> PartialEq for SameNavigator<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0.inner, &other.0.inner)
    }
}

/// The default navigator (Compose's internal `DefaultThreePaneScaffoldNavigator`): a snapshot list of the
/// destinations, the directive, strategies and history awareness in states, the value derived of them, a
/// [`MutableThreePaneScaffoldState`] animated to it. A cheap handle: clones are the same navigator.
pub(crate) struct DefaultThreePaneScaffoldNavigator<T: NavigatorContentKey> {
    inner: Rc<NavigatorImpl<T>>,
}

impl<T: NavigatorContentKey> Clone for DefaultThreePaneScaffoldNavigator<T> {
    fn clone(&self) -> Self {
        DefaultThreePaneScaffoldNavigator { inner: self.inner.clone() }
    }
}

struct NavigatorImpl<T: NavigatorContentKey> {
    destination_history: SnapshotStateList<ThreePaneScaffoldDestinationItem<T>>,
    scaffold_directive: MutableState<PaneScaffoldDirective>,
    is_destination_history_aware: MutableState<bool>,
    adapt_strategies: MutableState<ThreePaneScaffoldAdaptStrategies>,
    scaffold_value: DerivedState<ThreePaneScaffoldValue>,
    scaffold_state: MutableThreePaneScaffoldState,
    /// The state as the scaffold takes it, one shared `Rc` (what keys it see the same state).
    scaffold_state_dyn: Rc<dyn ThreePaneScaffoldState>,
}

impl<T: NavigatorContentKey> DefaultThreePaneScaffoldNavigator<T> {
    pub(crate) fn new(
        initial_destination_history: Vec<ThreePaneScaffoldDestinationItem<T>>,
        initial_scaffold_directive: PaneScaffoldDirective,
        initial_adapt_strategies: ThreePaneScaffoldAdaptStrategies,
        initial_is_destination_history_aware: bool,
    ) -> DefaultThreePaneScaffoldNavigator<T> {
        let destination_history = SnapshotStateList::new_confined(initial_destination_history);
        let scaffold_directive = mutable_state_of(initial_scaffold_directive);
        let is_destination_history_aware = mutable_state_of(initial_is_destination_history_aware);
        let adapt_strategies = kompoz_runtime::mutable_state_of!(initial_adapt_strategies);
        let (h, d, a, s) = (
            destination_history.clone(),
            scaffold_directive.clone(),
            is_destination_history_aware.clone(),
            adapt_strategies.clone(),
        );
        let scaffold_value = derived_state_of(move || {
            let last_index = h.len() as i32 - 1;
            calculate_scaffold_value(&h, &d.get(), &s.get(), a.get(), last_index)
        });
        let scaffold_state = MutableThreePaneScaffoldState::new(scaffold_value.get());
        DefaultThreePaneScaffoldNavigator {
            inner: Rc::new(NavigatorImpl {
                destination_history,
                scaffold_directive,
                is_destination_history_aware,
                adapt_strategies,
                scaffold_value,
                scaffold_state_dyn: Rc::new(scaffold_state.clone()),
                scaffold_state,
            }),
        }
    }

    fn calculate_scaffold_value(&self, destination_index: i32) -> ThreePaneScaffoldValue {
        let i = &self.inner;
        calculate_scaffold_value(
            &i.destination_history,
            &i.scaffold_directive.get(),
            &i.adapt_strategies.get(),
            i.is_destination_history_aware.get(),
            destination_index,
        )
    }

    fn animate_state_to_current_scaffold_value(&self) -> impl std::future::Future<Output = ()> + 'static {
        let value = self.inner.scaffold_value.get();
        named_args!(self.inner.scaffold_state.animate_to(target_state = value))
    }

    fn get_previous_destination_index(&self, back_nav_behavior: BackNavigationBehavior) -> i32 {
        let history = self.inner.destination_history.to_vec();
        if history.len() <= 1 {
            return -1;
        }
        let last_index = history.len() as i32 - 1;
        let current = history.last().cloned();
        let scaffold_value = self.inner.scaffold_value.get();
        if back_nav_behavior == BackNavigationBehavior::PopLatest {
            return last_index - 1;
        } else if back_nav_behavior == BackNavigationBehavior::PopUntilScaffoldValueChange {
            for previous in (0..last_index).rev() {
                if self.calculate_scaffold_value(previous) != scaffold_value {
                    return previous;
                }
            }
        } else if back_nav_behavior == BackNavigationBehavior::PopUntilCurrentDestinationChange {
            for previous in (0..last_index).rev() {
                if Some(history[previous as usize].pane) != current.as_ref().map(|c| c.pane) {
                    return previous;
                }
            }
        } else if back_nav_behavior == BackNavigationBehavior::PopUntilContentChange {
            for previous in (0..last_index).rev() {
                let content_key = &history[previous as usize].content_key;
                if Some(content_key) != current.as_ref().map(|c| &c.content_key) {
                    return previous;
                }
                if self.calculate_scaffold_value(previous) != scaffold_value {
                    return previous;
                }
            }
        }
        -1
    }

    /// Saves the history, restored with the directive, strategies and awareness given now (Compose's `saver`).
    pub(crate) fn saver(
        initial_scaffold_directive: PaneScaffoldDirective,
        initial_adapt_strategies: ThreePaneScaffoldAdaptStrategies,
        initial_destination_history_aware: bool,
    ) -> Saver<DefaultThreePaneScaffoldNavigator<T>> {
        list_saver(
            |navigator: &DefaultThreePaneScaffoldNavigator<T>| {
                navigator.inner.destination_history.to_vec().iter().map(destination_item_save).collect()
            },
            move |saved: &[SavedValue]| {
                let history = saved.iter().map(destination_item_restore).collect::<Option<Vec<_>>>()?;
                Some(DefaultThreePaneScaffoldNavigator::new(
                    history,
                    initial_scaffold_directive.clone(),
                    initial_adapt_strategies.clone(),
                    initial_destination_history_aware,
                ))
            },
        )
    }
}

/// The scaffold value of the history up to `destination_index` (none: -1) (Compose's `calculateScaffoldValue`).
fn calculate_scaffold_value<T: NavigatorContentKey>(
    destination_history: &SnapshotStateList<ThreePaneScaffoldDestinationItem<T>>,
    directive: &PaneScaffoldDirective,
    adapt_strategies: &ThreePaneScaffoldAdaptStrategies,
    is_destination_history_aware: bool,
    destination_index: i32,
) -> ThreePaneScaffoldValue {
    if destination_index == -1 {
        named_args!(calculate_three_pane_scaffold_value_of_destination(
            directive.max_horizontal_partitions,
            adapt_strategies,
            None::<&ThreePaneScaffoldDestinationItem<T>>,
            max_vertical_partitions = directive.max_vertical_partitions
        ))
    } else if is_destination_history_aware {
        destination_history.with(|history| {
            named_args!(calculate_three_pane_scaffold_value(
                directive.max_horizontal_partitions,
                adapt_strategies,
                &history[..=destination_index as usize],
                max_vertical_partitions = directive.max_vertical_partitions
            ))
        })
    } else {
        let destination = destination_history.get(destination_index as usize);
        named_args!(calculate_three_pane_scaffold_value_of_destination(
            directive.max_horizontal_partitions,
            adapt_strategies,
            destination.as_ref(),
            max_vertical_partitions = directive.max_vertical_partitions
        ))
    }
}

/// A destination saved as the list of its pane and content key (Compose's internal `destinationItemSaver`).
fn destination_item_save<T: NavigatorContentKey>(item: &ThreePaneScaffoldDestinationItem<T>) -> SavedValue {
    SavedValue::list([item.pane.save(), item.content_key.save()])
}

fn destination_item_restore<T: NavigatorContentKey>(saved: &SavedValue) -> Option<ThreePaneScaffoldDestinationItem<T>> {
    let items = saved.as_list()?;
    Some(ThreePaneScaffoldDestinationItem {
        pane: ThreePaneScaffoldRole::restore(items.first()?)?,
        content_key: Option::<T>::restore(items.get(1)?)?,
    })
}

impl<T: NavigatorContentKey> ThreePaneScaffoldNavigator<T> for DefaultThreePaneScaffoldNavigator<T> {
    fn scaffold_directive(&self) -> PaneScaffoldDirective {
        self.inner.scaffold_directive.get()
    }

    fn scaffold_state(&self) -> Rc<dyn ThreePaneScaffoldState> {
        self.inner.scaffold_state_dyn.clone()
    }

    fn scaffold_value(&self) -> ThreePaneScaffoldValue {
        self.inner.scaffold_value.get()
    }

    fn peek_previous_scaffold_value_with(
        &self,
        back_navigation_behavior: BackNavigationBehavior,
    ) -> ThreePaneScaffoldValue {
        let index = self.get_previous_destination_index(back_navigation_behavior);
        if index == -1 { self.scaffold_value() } else { self.calculate_scaffold_value(index) }
    }

    fn current_destination(&self) -> Option<ThreePaneScaffoldDestinationItem<T>> {
        self.inner.destination_history.with(|h| h.last().cloned())
    }

    fn is_destination_history_aware(&self) -> bool {
        self.inner.is_destination_history_aware.get()
    }

    fn set_is_destination_history_aware(&self, value: bool) {
        self.inner.is_destination_history_aware.set(value);
    }

    fn navigate_to_with(&self, pane: ThreePaneScaffoldRole, content_key: Option<T>) -> LocalFuture<'static, ()> {
        self.inner.destination_history.add(ThreePaneScaffoldDestinationItem { pane, content_key });
        Box::pin(self.animate_state_to_current_scaffold_value())
    }

    fn can_navigate_back_with(&self, back_navigation_behavior: BackNavigationBehavior) -> bool {
        self.get_previous_destination_index(back_navigation_behavior) >= 0
    }

    fn navigate_back_with(&self, back_navigation_behavior: BackNavigationBehavior) -> LocalFuture<'static, bool> {
        let previous_destination_index = self.get_previous_destination_index(back_navigation_behavior);
        if previous_destination_index < 0 {
            self.inner.destination_history.clear();
            let animate = self.animate_state_to_current_scaffold_value();
            return Box::pin(async move {
                animate.await;
                false
            });
        }
        let target_size = previous_destination_index as usize + 1;
        self.inner.destination_history.update(|history| history.truncate(target_size));
        let animate = self.animate_state_to_current_scaffold_value();
        Box::pin(async move {
            animate.await;
            true
        })
    }

    fn seek_back_with(
        &self,
        back_navigation_behavior: BackNavigationBehavior,
        fraction: f32,
    ) -> LocalFuture<'static, ()> {
        if fraction == 0.0 {
            Box::pin(self.animate_state_to_current_scaffold_value())
        } else {
            let previous_scaffold_value = self.peek_previous_scaffold_value_with(back_navigation_behavior);
            Box::pin(named_args!(self.inner.scaffold_state.seek_to(
                fraction,
                target_state = previous_scaffold_value,
                is_predictive_back_in_progress = true
            )))
        }
    }
}
