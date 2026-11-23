//! The list-detail and supporting pane scaffolds, as Compose's adaptive-layout ListDetailPaneScaffold.kt and
//! SupportingPaneScaffold.kt (1.2.0): a three-pane scaffold of a list pane and its detail (and an extra pane), or of
//! a main pane and a supporting one (and an extra pane), with their roles and defaults.

use super::pane_adapted_value::{AdaptStrategy, ThreePaneScaffoldAdaptStrategies};
use super::pane_expansion_state::{
    PaneExpansionState, PaneExpansionStateKeyProvider, remember_default_pane_expansion_state,
};
use super::pane_scaffold_directive::PaneScaffoldDirective;
use super::pane_scaffold_role::ThreePaneScaffoldRole;
use super::three_pane_scaffold::{ThreePaneDragHandle, ThreePanePane, ThreePaneScaffold, ThreePaneScaffoldOfValue};
use super::three_pane_scaffold_scope::ThreePaneScaffoldPaneScope;
use super::three_pane_scaffold_state::ThreePaneScaffoldState;
use super::three_pane_scaffold_value::{ThreePaneScaffoldHorizontalOrder, ThreePaneScaffoldValue};
use kompoz_foundation::prelude::*;
use kompoz_runtime::kompozable;
use kompoz_ui::Modifier;
use std::rc::Rc;

/// The expansion state given, or the default one of the key `key` gives (mutable with a drag handle).
#[track_caller]
fn expansion_state_or_default(
    pane_expansion_state: Option<PaneExpansionState>,
    key: impl FnOnce() -> super::pane_expansion_state::PaneExpansionStateKey,
    mutable: bool,
) -> PaneExpansionState {
    match pane_expansion_state {
        Some(state) => state,
        None => remember_default_pane_expansion_state(key, mutable),
    }
}

/// A list pane and its detail pane, side by side when there's room, one at a time otherwise (Compose's
/// `ListDetailPaneScaffold(directive, value, …)`): a three-pane scaffold of `directive` at `value` — the detail
/// pane primary, the list secondary (on its start), the optional `extra_pane` tertiary (at the end) — filling the
/// room it is given; the panes typically [`AnimatedPane`](ThreePaneScaffoldPaneScope::AnimatedPane)s. With
/// `pane_expansion_drag_handle`, two expanded panes' split can be dragged (by `pane_expansion_state`'s rules, else
/// a state remembered for the value).
///
/// Kotlin: `ListDetailPaneScaffold(directive: PaneScaffoldDirective, value: ThreePaneScaffoldValue, listPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, detailPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, modifier: Modifier = Modifier, extraPane: (@Composable ThreePaneScaffoldPaneScope.() -> Unit)? = null, paneExpansionDragHandle: (@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit)? = null, paneExpansionState: PaneExpansionState? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ListDetailPaneScaffold(
    directive: PaneScaffoldDirective,
    value: ThreePaneScaffoldValue,
    list_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    detail_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] extra_pane: Option<ThreePanePane>,
    #[default(None)] pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    #[default(None)] pane_expansion_state: Option<PaneExpansionState>,
) {
    let key_value = value.clone();
    let expansion_state = expansion_state_or_default(
        pane_expansion_state,
        move || key_value.pane_expansion_state_key(),
        pane_expansion_drag_handle.is_some(),
    );
    ThreePaneScaffoldOfValue(
        modifier.fill_max_size(),
        directive,
        value,
        ListDetailPaneScaffoldDefaults::PaneOrder,
        Rc::new(list_pane),
        extra_pane,
        Some(expansion_state),
        pane_expansion_drag_handle,
        Rc::new(detail_pane),
    );
}

/// The same moved by `scaffold_state` (Compose's `ListDetailPaneScaffold(directive, scaffoldState, …)`): seeking
/// along its transition (a predictive back gesture's) as it is moved.
///
/// Kotlin: `ListDetailPaneScaffold(directive: PaneScaffoldDirective, scaffoldState: ThreePaneScaffoldState, listPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, detailPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, modifier: Modifier = Modifier, extraPane: (@Composable ThreePaneScaffoldPaneScope.() -> Unit)? = null, paneExpansionDragHandle: (@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit)? = null, paneExpansionState: PaneExpansionState? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StateListDetailPaneScaffold(
    directive: PaneScaffoldDirective,
    scaffold_state: Rc<dyn ThreePaneScaffoldState>,
    list_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    detail_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] extra_pane: Option<ThreePanePane>,
    #[default(None)] pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    #[default(None)] pane_expansion_state: Option<PaneExpansionState>,
) {
    let state = scaffold_state.clone();
    let expansion_state = expansion_state_or_default(
        pane_expansion_state,
        move || state.target_state().pane_expansion_state_key(),
        pane_expansion_drag_handle.is_some(),
    );
    ThreePaneScaffold(
        modifier.fill_max_size(),
        directive,
        scaffold_state,
        ListDetailPaneScaffoldDefaults::PaneOrder,
        Rc::new(list_pane),
        extra_pane,
        Some(expansion_state),
        pane_expansion_drag_handle,
        Rc::new(detail_pane),
    );
}

/// The list-detail scaffold's defaults (Compose's `ListDetailPaneScaffoldDefaults`).
pub struct ListDetailPaneScaffoldDefaults;

#[kompoz_runtime::named]
impl ListDetailPaneScaffoldDefaults {
    /// The panes' strategies, each hide by default (Compose's `adaptStrategies`).
    ///
    /// Kotlin: `adaptStrategies(detailPaneAdaptStrategy: AdaptStrategy = AdaptStrategy.Hide, listPaneAdaptStrategy: AdaptStrategy = AdaptStrategy.Hide, extraPaneAdaptStrategy: AdaptStrategy = AdaptStrategy.Hide): ThreePaneScaffoldAdaptStrategies`
    pub fn adapt_strategies(
        &self,
        #[default(AdaptStrategy::Hide)] detail_pane_adapt_strategy: AdaptStrategy,
        #[default(AdaptStrategy::Hide)] list_pane_adapt_strategy: AdaptStrategy,
        #[default(AdaptStrategy::Hide)] extra_pane_adapt_strategy: AdaptStrategy,
    ) -> ThreePaneScaffoldAdaptStrategies {
        super::pane_adapted_value::ThreePaneScaffoldAdaptStrategies(
            detail_pane_adapt_strategy,
            list_pane_adapt_strategy,
            extra_pane_adapt_strategy,
        )
    }
}

impl ListDetailPaneScaffoldDefaults {
    /// The list, then the detail, then the extra pane (Compose's internal `PaneOrder`).
    pub(crate) const PaneOrder: ThreePaneScaffoldHorizontalOrder = ThreePaneScaffoldHorizontalOrder {
        first_pane: ThreePaneScaffoldRole::Secondary,
        second_pane: ThreePaneScaffoldRole::Primary,
        third_pane: ThreePaneScaffoldRole::Tertiary,
    };
}

/// The list-detail scaffold's panes by their roles (Compose's object `ListDetailPaneScaffoldRole`).
pub struct ListDetailPaneScaffoldRole;

impl ListDetailPaneScaffoldRole {
    /// The list pane: the secondary pane.
    pub const List: ThreePaneScaffoldRole = ThreePaneScaffoldRole::Secondary;
    /// The detail pane: the primary pane.
    pub const Detail: ThreePaneScaffoldRole = ThreePaneScaffoldRole::Primary;
    /// The extra pane: the tertiary pane.
    pub const Extra: ThreePaneScaffoldRole = ThreePaneScaffoldRole::Tertiary;
}

/// A main pane and a supporting one, side by side when there's room (the supporting pane at the end), the
/// supporting pane reflowed under the main one or shown alone otherwise (Compose's `SupportingPaneScaffold(directive,
/// value, …)`): a three-pane scaffold — the main pane primary, the supporting secondary, the optional `extra_pane`
/// tertiary — filling the room it is given.
///
/// Kotlin: `SupportingPaneScaffold(directive: PaneScaffoldDirective, value: ThreePaneScaffoldValue, mainPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, supportingPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, modifier: Modifier = Modifier, extraPane: (@Composable ThreePaneScaffoldPaneScope.() -> Unit)? = null, paneExpansionDragHandle: (@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit)? = null, paneExpansionState: PaneExpansionState? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SupportingPaneScaffold(
    directive: PaneScaffoldDirective,
    value: ThreePaneScaffoldValue,
    main_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    supporting_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] extra_pane: Option<ThreePanePane>,
    #[default(None)] pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    #[default(None)] pane_expansion_state: Option<PaneExpansionState>,
) {
    let key_value = value.clone();
    let expansion_state = expansion_state_or_default(
        pane_expansion_state,
        move || key_value.pane_expansion_state_key(),
        pane_expansion_drag_handle.is_some(),
    );
    ThreePaneScaffoldOfValue(
        modifier.fill_max_size(),
        directive,
        value,
        SupportingPaneScaffoldDefaults::PaneOrder,
        Rc::new(supporting_pane),
        extra_pane,
        Some(expansion_state),
        pane_expansion_drag_handle,
        Rc::new(main_pane),
    );
}

/// The same moved by `scaffold_state` (Compose's `SupportingPaneScaffold(directive, scaffoldState, …)`).
///
/// Kotlin: `SupportingPaneScaffold(directive: PaneScaffoldDirective, scaffoldState: ThreePaneScaffoldState, mainPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, supportingPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, modifier: Modifier = Modifier, extraPane: (@Composable ThreePaneScaffoldPaneScope.() -> Unit)? = null, paneExpansionDragHandle: (@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit)? = null, paneExpansionState: PaneExpansionState? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StateSupportingPaneScaffold(
    directive: PaneScaffoldDirective,
    scaffold_state: Rc<dyn ThreePaneScaffoldState>,
    main_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    supporting_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] extra_pane: Option<ThreePanePane>,
    #[default(None)] pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    #[default(None)] pane_expansion_state: Option<PaneExpansionState>,
) {
    let state = scaffold_state.clone();
    let expansion_state = expansion_state_or_default(
        pane_expansion_state,
        move || state.target_state().pane_expansion_state_key(),
        pane_expansion_drag_handle.is_some(),
    );
    ThreePaneScaffold(
        modifier.fill_max_size(),
        directive,
        scaffold_state,
        SupportingPaneScaffoldDefaults::PaneOrder,
        Rc::new(supporting_pane),
        extra_pane,
        Some(expansion_state),
        pane_expansion_drag_handle,
        Rc::new(main_pane),
    );
}

/// The supporting pane scaffold's defaults (Compose's `SupportingPaneScaffoldDefaults`).
pub struct SupportingPaneScaffoldDefaults;

#[kompoz_runtime::named]
impl SupportingPaneScaffoldDefaults {
    /// The panes' strategies: the supporting pane reflows under the main one by default, the others hide (Compose's
    /// `adaptStrategies`).
    ///
    /// Kotlin: `adaptStrategies(mainPaneAdaptStrategy: AdaptStrategy = AdaptStrategy.Hide, supportingPaneAdaptStrategy: AdaptStrategy = AdaptStrategy.Reflow(SupportingPaneScaffoldRole.Main), extraPaneAdaptStrategy: AdaptStrategy = AdaptStrategy.Hide): ThreePaneScaffoldAdaptStrategies`
    pub fn adapt_strategies(
        &self,
        #[default(AdaptStrategy::Hide)] main_pane_adapt_strategy: AdaptStrategy,
        #[default(AdaptStrategy::reflow(SupportingPaneScaffoldRole::Main))]
        supporting_pane_adapt_strategy: AdaptStrategy,
        #[default(AdaptStrategy::Hide)] extra_pane_adapt_strategy: AdaptStrategy,
    ) -> ThreePaneScaffoldAdaptStrategies {
        super::pane_adapted_value::ThreePaneScaffoldAdaptStrategies(
            main_pane_adapt_strategy,
            supporting_pane_adapt_strategy,
            extra_pane_adapt_strategy,
        )
    }
}

impl SupportingPaneScaffoldDefaults {
    /// The main, then the supporting, then the extra pane (Compose's internal `PaneOrder`).
    pub(crate) const PaneOrder: ThreePaneScaffoldHorizontalOrder = ThreePaneScaffoldHorizontalOrder {
        first_pane: ThreePaneScaffoldRole::Primary,
        second_pane: ThreePaneScaffoldRole::Secondary,
        third_pane: ThreePaneScaffoldRole::Tertiary,
    };
}

/// The supporting pane scaffold's panes by their roles (Compose's object `SupportingPaneScaffoldRole`).
pub struct SupportingPaneScaffoldRole;

impl SupportingPaneScaffoldRole {
    /// The main pane: the primary pane.
    pub const Main: ThreePaneScaffoldRole = ThreePaneScaffoldRole::Primary;
    /// The supporting pane: the secondary pane.
    pub const Supporting: ThreePaneScaffoldRole = ThreePaneScaffoldRole::Secondary;
    /// The extra pane: the tertiary pane.
    pub const Extra: ThreePaneScaffoldRole = ThreePaneScaffoldRole::Tertiary;
}
