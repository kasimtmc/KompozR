//! The scaffolds driven by a navigator with back handling, as Compose's adaptive-navigation
//! AndroidThreePaneScaffold.android.kt and ThreePaneScaffoldPredictiveBackHandler.android.kt (1.2.0):
//! [`NavigableListDetailPaneScaffold`], [`NavigableSupportingPaneScaffold`], [`ThreePaneScaffoldPredictiveBackHandler`].
//!
//! Compose has them in androidMain only (Compose for desktop 1.2.0 has neither: its desktop has no back events);
//! KompozR's desktop has back (Escape, the back mouse button, a predictive gesture where the platform gives one:
//! kompoz-ui's `PredictiveBackHandler`, activity-compose's on navigationevent, as Android's), so Android's are ported
//! as they are.

use super::three_pane_scaffold_navigator::{BackNavigationBehavior, NavigatorContentKey, ThreePaneScaffoldNavigator};
use crate::layout::{
    PaneAdaptedValue, PaneExpansionState, StateListDetailPaneScaffold, StateSupportingPaneScaffold,
    ThreePaneDragHandle, ThreePanePane, ThreePaneScaffoldPaneScope, ThreePaneScaffoldValue,
};
use kompoz_animation::{CubicBezierEasing, Easing};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_runtime::{kompozable, kompozable_block, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::back::PredictiveBackHandler;
use std::rc::Rc;

/// A [`ListDetailPaneScaffold`](crate::layout::ListDetailPaneScaffold) driven by `navigator`, back (and a
/// predictive back gesture) navigating back by `default_back_behavior` (Compose's `NavigableListDetailPaneScaffold`).
///
/// Kotlin: `NavigableListDetailPaneScaffold(navigator: ThreePaneScaffoldNavigator<T>, listPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, detailPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, modifier: Modifier = Modifier, extraPane: (@Composable ThreePaneScaffoldPaneScope.() -> Unit)? = null, defaultBackBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange, paneExpansionDragHandle: (@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit)? = null, paneExpansionState: PaneExpansionState? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigableListDetailPaneScaffold<T: NavigatorContentKey>(
    navigator: Rc<dyn ThreePaneScaffoldNavigator<T>>,
    list_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    detail_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] extra_pane: Option<ThreePanePane>,
    #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)] default_back_behavior: BackNavigationBehavior,
    #[default(None)] pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    #[default(None)] pane_expansion_state: Option<PaneExpansionState>,
) {
    ThreePaneScaffoldPredictiveBackHandler(navigator.clone(), default_back_behavior);
    StateListDetailPaneScaffold(
        navigator.scaffold_directive(),
        navigator.scaffold_state(),
        list_pane,
        detail_pane,
        modifier = modifier,
        extra_pane = extra_pane,
        pane_expansion_drag_handle = pane_expansion_drag_handle,
        pane_expansion_state = pane_expansion_state,
    );
}

/// A [`SupportingPaneScaffold`](crate::layout::SupportingPaneScaffold) driven by `navigator`, back navigating back
/// by `default_back_behavior` (Compose's `NavigableSupportingPaneScaffold`).
///
/// Kotlin: `NavigableSupportingPaneScaffold(navigator: ThreePaneScaffoldNavigator<T>, mainPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, supportingPane: @Composable ThreePaneScaffoldPaneScope.() -> Unit, modifier: Modifier = Modifier, extraPane: (@Composable ThreePaneScaffoldPaneScope.() -> Unit)? = null, defaultBackBehavior: BackNavigationBehavior = BackNavigationBehavior.PopUntilScaffoldValueChange, paneExpansionDragHandle: (@Composable ThreePaneScaffoldScope.(PaneExpansionState) -> Unit)? = null, paneExpansionState: PaneExpansionState? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigableSupportingPaneScaffold<T: NavigatorContentKey>(
    navigator: Rc<dyn ThreePaneScaffoldNavigator<T>>,
    main_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    supporting_pane: impl Fn(&ThreePaneScaffoldPaneScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] extra_pane: Option<ThreePanePane>,
    #[default(BackNavigationBehavior::PopUntilScaffoldValueChange)] default_back_behavior: BackNavigationBehavior,
    #[default(None)] pane_expansion_drag_handle: Option<ThreePaneDragHandle>,
    #[default(None)] pane_expansion_state: Option<PaneExpansionState>,
) {
    ThreePaneScaffoldPredictiveBackHandler(navigator.clone(), default_back_behavior);
    StateSupportingPaneScaffold(
        navigator.scaffold_directive(),
        navigator.scaffold_state(),
        main_pane,
        supporting_pane,
        modifier = modifier,
        extra_pane = extra_pane,
        pane_expansion_drag_handle = pane_expansion_drag_handle,
        pane_expansion_state = pane_expansion_state,
    );
}

/// Back navigating `navigator` back by `back_behavior` while it can (Compose's `ThreePaneScaffoldPredictiveBackHandler`):
/// a predictive gesture seeks the scaffold back along its progress (eased, a tenth to a fifth of the way by the
/// panes expanded), then navigates back; called off, it seeks back to where it was.
///
/// Kotlin: `ThreePaneScaffoldPredictiveBackHandler(navigator: ThreePaneScaffoldNavigator<T>, backBehavior: BackNavigationBehavior)`
#[kompozable]
pub fn ThreePaneScaffoldPredictiveBackHandler<T: NavigatorContentKey>(
    navigator: Rc<dyn ThreePaneScaffoldNavigator<T>>,
    back_behavior: BackNavigationBehavior,
) {
    // (Compose's `key(navigator, backBehavior)`: a new handler for another navigator or behavior)
    let key = (Rc::as_ptr(&navigator) as *const () as usize, back_behavior.to_string());
    kompoz_runtime::key(&key, move || {
        let navigator = navigator.clone();
        kompozable_block!({
            let enabled = navigator.can_navigate_back_with(back_behavior);
            let navigator = navigator.clone();
            // (where the cancelled gesture's seeking back runs: Kotlin's `withContext(NonCancellable)` outlives the
            // cancelled coroutine; a scope of the handler's composition here)
            let scope = remember_coroutine_scope();
            PredictiveBackHandler(
                enabled = enabled,
                on_back = move |progress| {
                    let (navigator, scope) = (navigator.clone(), scope.clone());
                    async move {
                        // (Kotlin's `catch (CancellationException) { withContext(NonCancellable) { seekBack(…, 0f) } }`:
                        // a guard seeking back to 0 when the gesture's coroutine is cancelled before it is done)
                        struct SeekBackOnCancel<T: NavigatorContentKey> {
                            navigator: Rc<dyn ThreePaneScaffoldNavigator<T>>,
                            back_behavior: BackNavigationBehavior,
                            scope: CoroutineScope,
                            done: bool,
                        }
                        impl<T: NavigatorContentKey> Drop for SeekBackOnCancel<T> {
                            fn drop(&mut self) {
                                if !self.done {
                                    let seek = self.navigator.seek_back_with(self.back_behavior, 0.0);
                                    named_args!(self.scope.launch(seek));
                                }
                            }
                        }
                        let mut guard =
                            SeekBackOnCancel { navigator: navigator.clone(), back_behavior, scope, done: false };
                        let seeking = navigator.clone();
                        progress
                            .collect_async(move |back_event| {
                                let navigator = seeking.clone();
                                async move {
                                    let fraction = back_progress_to_state_progress(
                                        back_event.progress,
                                        &navigator.scaffold_value(),
                                    );
                                    navigator.seek_back_with(back_behavior, fraction).await;
                                }
                            })
                            .await;
                        navigator.navigate_back_with(back_behavior).await;
                        guard.done = true;
                    }
                },
            );
        })
    });
}

/// The scaffold's progress of a back gesture's (Compose's private `backProgressToStateProgress`).
fn back_progress_to_state_progress(progress: f32, scaffold_value: &ThreePaneScaffoldValue) -> f32 {
    let ratio = match expanded_count(scaffold_value) {
        1 => SinglePaneProgressRatio,
        2 => DualPaneProgressRatio,
        _ => TriplePaneProgressRatio,
    };
    ThreePaneScaffoldPredictiveBackEasing.transform(progress) * ratio
}

const ThreePaneScaffoldPredictiveBackEasing: CubicBezierEasing = CubicBezierEasing::constant(0.1, 0.1, 0.0, 1.0);
const SinglePaneProgressRatio: f32 = 0.1;
const DualPaneProgressRatio: f32 = 0.15;
const TriplePaneProgressRatio: f32 = 0.2;

/// How many panes are expanded (Compose's private `ThreePaneScaffoldValue.expandedCount`).
fn expanded_count(value: &ThreePaneScaffoldValue) -> i32 {
    [&value.primary, &value.secondary, &value.tertiary]
        .into_iter()
        .filter(|v| **v == PaneAdaptedValue::Expanded)
        .count() as i32
}
