//! A row of an app bar's items, as Compose's AppBarRow.kt: [`AppBarRow`] lays out the items its block adds
//! ([`AppBarRowScope`]) side by side, those that don't fit (or past its slots) going into an overflow menu opened by
//! an indicator at its end.

use crate::app_bar_dsl::{
    AppBarItem, AppBarItemProvider, AppBarMenuState, AppBarOverflowIndicator, AppBarScope, AppBarScopeImpl,
    OverflowMeasurePolicy, OverflowStateKey, remember_app_bar_overflow_state, sealed,
};
use crate::menu_desktop::DropdownMenu;
use kompoz_foundation::prelude::*;
use kompoz_runtime::{
    DerivedState, derived_state_of_with_policy, kompozable, never_equal_policy, remember, remember_keyed,
    remember_updated_state, restartable,
};
use kompoz_ui::Modifier;
use kompoz_ui::layout::{MultiContentLayout, MultiContentMeasurePolicy};
use std::rc::Rc;

/// A row of an app bar's items (Compose's `AppBarRow`): the items `content` adds (clickable and toggleable icon
/// items, items of their own: [`AppBarScopeItems`](crate::AppBarScopeItems)) side by side, as many as fit in its
/// width (room kept for the overflow indicator, which the last item may take) and at most `max_item_count` slots
/// (the indicator taking the last one when items are left out). The items left out are in a [`DropdownMenu`]
/// below the indicator, which `overflow_indicator` composes (an [`AppBarOverflowIndicator`] by default) given the
/// menu's state to show it. The block runs again when a state it reads changes, without composing the row again.
/// (Compose's hidden deprecated overload, the indicator first, isn't ported.)
///
/// Kotlin: `AppBarRow(modifier: Modifier = Modifier, overflowIndicator: @Composable (AppBarMenuState) -> Unit = { menuState -> AppBarOverflowIndicator(menuState) }, maxItemCount: Int = Int.MAX_VALUE, content: AppBarRowScope.() -> Unit)`
#[kompozable]
pub fn AppBarRow(
    #[default(Modifier)] modifier: Modifier,
    #[default(|menu_state: AppBarMenuState| kompoz_runtime::kompozable_block!({ AppBarOverflowIndicator(menu_state) }))]
    overflow_indicator: impl Fn(AppBarMenuState) + 'static,
    #[default(i32::MAX)] max_item_count: i32,
    content: impl Fn(&dyn AppBarRowScope) + 'static,
) {
    // (a block made anew at each run is a change: it is read in the derived state below, which only the contents'
    // own scopes read, as Compose's)
    let latest_content = remember_updated_state!(Rc::new(content) as Rc<dyn Fn(&dyn AppBarRowScope)>);
    // (Compose's `derivedStateOf`: a new scope each time, unequal to the last as a class without `equals` is)
    let scope: DerivedState<Rc<AppBarRowScopeImpl>> = remember(move || {
        derived_state_of_with_policy(never_equal_policy(), move || {
            let scope = Rc::new(AppBarRowScopeImpl { impl_: AppBarScopeImpl::default() });
            (latest_content.get())(&*scope);
            scope
        })
    });
    let menu_state = remember(AppBarMenuState::new);
    let overflow_state = remember_app_bar_overflow_state();
    let policy_state = overflow_state.clone();
    let measure_policy = remember_keyed((OverflowStateKey(overflow_state.clone()), max_item_count), move || {
        Rc::new(OverflowMeasurePolicy { overflow_state: policy_state, max_item_count, is_vertical: false })
            as Rc<dyn MultiContentMeasurePolicy>
    });

    let overflow_indicator = Rc::new(overflow_indicator);
    let items_scope = scope.clone();
    MultiContentLayout(
        contents = vec![
            // (each content a composable lambda, a restartable scope of its own as Compose's: what it reads — the
            // block's items, the menu's state — runs it again, not the bar)
            Rc::new(move || {
                let items_scope = items_scope.clone();
                restartable((), None, move |_| {
                    for it in items_scope.get().items() {
                        it.AppbarContent();
                    }
                });
            }) as Rc<dyn Fn()>,
            Rc::new(move || {
                let (overflow_indicator, menu_state, scope) =
                    (overflow_indicator.clone(), menu_state.clone(), scope.clone());
                let overflow_state = overflow_state.clone();
                restartable((), None, move |_| {
                    let (overflow_indicator, menu_state, scope) =
                        (overflow_indicator.clone(), menu_state.clone(), scope.clone());
                    let overflow_state = overflow_state.clone();
                    Box(content = move || {
                        overflow_indicator(menu_state.clone());
                        let (dismissed, menu_state) = (menu_state.clone(), menu_state.clone());
                        let (scope, overflow_state) = (scope.clone(), overflow_state.clone());
                        DropdownMenu(
                            expanded = menu_state.is_showing(),
                            on_dismiss_request = move || dismissed.dismiss(),
                            content = move || {
                                let items = scope.get().items();
                                let (from, to) =
                                    (overflow_state.visible_item_count(), overflow_state.total_item_count());
                                for item in items.iter().take(to.max(0) as usize).skip(from.max(0) as usize) {
                                    item.MenuContent(menu_state.clone());
                                }
                            },
                        );
                    });
                });
            }) as Rc<dyn Fn()>,
        ],
        modifier = modifier,
        measure_policy = measure_policy,
    );
}

/// The scope an [`AppBarRow`]'s block adds its items in (Compose's interface `AppBarRowScope`, an
/// [`AppBarScope`]).
pub trait AppBarRowScope: AppBarScope {}

/// Compose's private `AppBarRowScopeImpl`: the items' list it delegates to.
struct AppBarRowScopeImpl {
    impl_: AppBarScopeImpl,
}

impl sealed::Sealed for AppBarRowScopeImpl {
    fn add_item(&self, item: Rc<dyn AppBarItem>) {
        sealed::Sealed::add_item(&self.impl_, item);
    }
}

impl AppBarScope for AppBarRowScopeImpl {}

impl AppBarRowScope for AppBarRowScopeImpl {}

impl AppBarItemProvider for AppBarRowScopeImpl {
    fn items_count(&self) -> usize {
        self.impl_.items_count()
    }

    fn items(&self) -> Vec<Rc<dyn AppBarItem>> {
        self.impl_.items()
    }
}
