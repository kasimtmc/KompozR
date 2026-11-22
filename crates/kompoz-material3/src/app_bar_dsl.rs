//! The items of an [`AppBarRow`](crate::AppBarRow) and an [`AppBarColumn`](crate::AppBarColumn), as Compose's
//! AppBarDsl.kt: what their block adds through an [`AppBarScope`] — clickable and toggleable icon items (an icon
//! button with a tooltip in the bar, a menu item of their label in the overflow menu) and items of their own —, the
//! overflow menu's state ([`AppBarMenuState`]), the default overflow indicator ([`AppBarOverflowIndicator`]: a ⋮
//! icon button), and the measure policy that puts in the bar what fits (keeping room for the indicator) and the rest
//! in the menu.

use crate::IconButtonColors;
use crate::icon::VectorIcon;
use crate::icon_button::{IconButton, IconToggleButton};
use crate::icon_button_defaults::IconButtonDefaults;
use crate::internal::icons::Icons;
use crate::internal::layout_util::SubtractConstraintSafely;
use crate::internal::strings::{Strings, get_string};
use crate::menu_desktop::DropdownMenuItem;
use crate::text::Text;
use crate::tooltip::{
    TooltipAnchorPosition, TooltipBox, TooltipDefaults, TooltipScope, TooltipScopeComposables, remember_tooltip_state,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::Shape;
use kompoz_runtime::{
    MutableState, Saveable, SavedValue, Saver, kompozable, kompozable_block, list_saver, mutable_state_of, named,
};
use kompoz_ui::layout::{Measurable, MeasureResult, MeasureScope, MultiContentMeasurePolicy, Placeable};
use kompoz_ui::{Constraints, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub(crate) mod sealed {
    use super::AppBarMenuState;
    use std::rc::Rc;

    /// What a scope of an app bar's items is made of (Compose's sealed interface: only material3 implements it).
    pub trait Sealed {
        /// Adds an item (what Compose's `AppBarScopeImpl` does to its list).
        fn add_item(&self, item: Rc<dyn AppBarItem>);
    }

    /// An item of an app bar's block (Compose's internal `AppBarItem`): what it shows in the bar and in the
    /// overflow menu.
    pub trait AppBarItem {
        /// The item in the bar (Compose's `AppbarContent`).
        fn AppbarContent(&self);

        /// The item in the overflow menu (Compose's `MenuContent`), told the menu's state (to close it).
        fn MenuContent(&self, state: AppBarMenuState);
    }
}

pub(crate) use sealed::AppBarItem;

/// The scope an [`AppBarRow`](crate::AppBarRow)'s or [`AppBarColumn`](crate::AppBarColumn)'s block adds its items
/// in (Compose's sealed interface `AppBarScope`): its members are [`AppBarScopeItems`]' — `clickable_item`,
/// `toggleable_item`, `custom_item` — (methods with defaults of every scope, Kotlin's `clickableItem(…)` in the
/// block).
pub trait AppBarScope: sealed::Sealed {}

/// The members of an [`AppBarScope`] (Compose's interface's functions), for every scope (`dyn AppBarRowScope`
/// too).
#[named(trait AppBarScopeItems)]
impl<S: AppBarScope + ?Sized> S {
    /// Adds a clickable item (Compose's `clickableItem`): in the bar an [`IconButton`] of `icon` running
    /// `on_click`, with a plain tooltip of `label` above it; in the overflow menu a menu item of `label` that runs
    /// `on_click` and closes the menu. Disabled (`enabled` false) it takes no clicks in either.
    fn clickable_item(
        &self,
        on_click: impl Fn() + 'static,
        icon: impl Fn() + 'static,
        label: String,
        #[default(true)] enabled: bool,
    ) {
        self.add_item(Rc::new(ClickableAppBarItem {
            on_click: Rc::new(on_click),
            icon: Rc::new(icon),
            enabled,
            label,
        }));
    }

    /// Adds a toggleable item (Compose's `toggleableItem`): in the bar an [`IconToggleButton`] of `icon`,
    /// `checked`, telling `on_checked_change`, with a plain tooltip of `label` above it; in the overflow menu a menu
    /// item of `label` that flips it and closes the menu. Disabled (`enabled` false) it takes no clicks in either.
    fn toggleable_item(
        &self,
        checked: bool,
        on_checked_change: impl Fn(bool) + 'static,
        icon: impl Fn() + 'static,
        label: String,
        #[default(true)] enabled: bool,
    ) {
        self.add_item(Rc::new(ToggleableAppBarItem {
            checked,
            on_checked_change: Rc::new(on_checked_change),
            icon: Rc::new(icon),
            enabled,
            label,
        }));
    }

    /// Adds an item of its own (Compose's `customItem`): `appbar_content` in the bar, `menu_content` (told the
    /// menu's state) in the overflow menu.
    fn custom_item(&self, appbar_content: impl Fn() + 'static, menu_content: impl Fn(AppBarMenuState) + 'static) {
        self.add_item(Rc::new(CustomAppBarItem {
            appbar_content: Rc::new(appbar_content),
            menu_content: Rc::new(menu_content),
        }));
    }
}

/// The items a block added (Compose's internal `AppBarItemProvider`).
pub(crate) trait AppBarItemProvider {
    /// How many there are (Compose's `itemsCount`).
    #[allow(dead_code)]
    fn items_count(&self) -> usize;

    /// The items, in the order added (Compose's `items`).
    fn items(&self) -> Vec<Rc<dyn AppBarItem>>;
}

/// The list a block adds its items to (Compose's internal `AppBarScopeImpl`); the row's and the column's scopes
/// delegate to it.
#[derive(Default)]
pub(crate) struct AppBarScopeImpl {
    items: RefCell<Vec<Rc<dyn AppBarItem>>>,
}

impl sealed::Sealed for AppBarScopeImpl {
    fn add_item(&self, item: Rc<dyn AppBarItem>) {
        self.items.borrow_mut().push(item);
    }
}

impl AppBarScope for AppBarScopeImpl {}

impl AppBarItemProvider for AppBarScopeImpl {
    fn items_count(&self) -> usize {
        self.items.borrow().len()
    }

    fn items(&self) -> Vec<Rc<dyn AppBarItem>> {
        self.items.borrow().clone()
    }
}

/// The icon button of an item, with a plain tooltip of its label above it (what Compose's items' `AppbarContent`
/// write out each: "Icon buttons should have a tooltip associated with them").
#[kompozable]
fn LabelTooltipBox(label: String, content: impl Fn() + 'static) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |scope: &dyn TooltipScope| {
            let label = label.clone();
            scope.PlainTooltip(content = move || Text(label.clone()));
        },
        state = remember_tooltip_state(),
        content = content,
    );
}

/// A clickable item (Compose's internal `ClickableAppBarItem`).
pub(crate) struct ClickableAppBarItem {
    on_click: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    enabled: bool,
    label: String,
}

impl AppBarItem for ClickableAppBarItem {
    fn AppbarContent(&self) {
        let (on_click, icon, enabled) = (self.on_click.clone(), self.icon.clone(), self.enabled);
        let label = self.label.clone();
        kompozable_block!({
            LabelTooltipBox(label, move || {
                let (on_click, icon) = (on_click.clone(), icon.clone());
                IconButton(on_click = move || on_click(), enabled = enabled, content = move || icon());
            });
        });
    }

    fn MenuContent(&self, state: AppBarMenuState) {
        let (on_click, enabled, label) = (self.on_click.clone(), self.enabled, self.label.clone());
        kompozable_block!({
            DropdownMenuItem(
                enabled = enabled,
                text = move || Text(label.clone()),
                on_click = move || {
                    on_click();
                    state.dismiss();
                },
            );
        });
    }
}

/// A toggleable item (Compose's internal `ToggleableAppBarItem`).
pub(crate) struct ToggleableAppBarItem {
    checked: bool,
    on_checked_change: Rc<dyn Fn(bool)>,
    icon: Rc<dyn Fn()>,
    enabled: bool,
    label: String,
}

impl AppBarItem for ToggleableAppBarItem {
    fn AppbarContent(&self) {
        let (checked, on_checked_change, icon) = (self.checked, self.on_checked_change.clone(), self.icon.clone());
        let (enabled, label) = (self.enabled, self.label.clone());
        kompozable_block!({
            LabelTooltipBox(label, move || {
                let (on_checked_change, icon) = (on_checked_change.clone(), icon.clone());
                IconToggleButton(
                    checked = checked,
                    on_checked_change = move |v| on_checked_change(v),
                    enabled = enabled,
                    content = move || icon(),
                );
            });
        });
    }

    fn MenuContent(&self, state: AppBarMenuState) {
        let (checked, on_checked_change) = (self.checked, self.on_checked_change.clone());
        let (enabled, label) = (self.enabled, self.label.clone());
        kompozable_block!({
            DropdownMenuItem(
                enabled = enabled,
                text = move || Text(label.clone()),
                on_click = move || {
                    on_checked_change(!checked);
                    state.dismiss();
                },
            );
        });
    }
}

/// An item of its own (Compose's internal `CustomAppBarItem`).
pub(crate) struct CustomAppBarItem {
    appbar_content: Rc<dyn Fn()>,
    menu_content: Rc<dyn Fn(AppBarMenuState)>,
}

impl AppBarItem for CustomAppBarItem {
    fn AppbarContent(&self) {
        (self.appbar_content)();
    }

    fn MenuContent(&self, state: AppBarMenuState) {
        (self.menu_content)(state);
    }
}

/// The state of an [`AppBarRow`](crate::AppBarRow)'s or [`AppBarColumn`](crate::AppBarColumn)'s overflow menu
/// (Compose's class `AppBarMenuState`): whether it shows; shown and closed by its indicator and its items. A
/// handle: its clones are the same state. (Compose's hidden deprecated `isExpanded` isn't ported.)
#[derive(Clone)]
pub struct AppBarMenuState {
    is_showing: MutableState<bool>,
}

impl AppBarMenuState {
    /// A menu state, not showing (Compose's `AppBarMenuState()`).
    ///
    /// Kotlin: `AppBarMenuState()`
    pub fn new() -> AppBarMenuState {
        AppBarMenuState { is_showing: mutable_state_of(false) }
    }

    /// Whether the overflow menu shows (Compose's `isShowing`).
    pub fn is_showing(&self) -> bool {
        self.is_showing.get()
    }

    /// Closes the overflow menu (Compose's `dismiss`).
    pub fn dismiss(&self) {
        self.is_showing.set(false);
    }

    /// Shows the overflow menu (Compose's `show`).
    pub fn show(&self) {
        self.is_showing.set(true);
    }
}

impl Default for AppBarMenuState {
    fn default() -> Self {
        AppBarMenuState::new()
    }
}

/// The same state (Compose's classes compare by instance).
impl PartialEq for AppBarMenuState {
    fn eq(&self, other: &AppBarMenuState) -> bool {
        self.is_showing == other.is_showing
    }
}

/// How many items a row or column has and how many of them are in the bar (Compose's internal
/// `AppBarOverflowState`): what its measure policy found, the rest being the menu's.
pub(crate) trait AppBarOverflowState {
    fn total_item_count(&self) -> i32;
    fn set_total_item_count(&self, count: i32);
    fn visible_item_count(&self) -> i32;
    fn set_visible_item_count(&self, count: i32);
}

/// The overflow state, saved and restored (Compose's internal `rememberAppBarOverflowState`).
#[kompozable]
pub(crate) fn remember_app_bar_overflow_state() -> Rc<dyn AppBarOverflowState> {
    kompoz_runtime::remember_saveable_with(
        &AppBarOverflowStateImpl::Saver(),
        || Rc::new(AppBarOverflowStateImpl::new()),
    ) as Rc<dyn AppBarOverflowState>
}

/// Compose's private `AppBarOverflowStateImpl`: two int states.
pub(crate) struct AppBarOverflowStateImpl {
    total_item_count: MutableState<i32>,
    visible_item_count: MutableState<i32>,
}

impl AppBarOverflowStateImpl {
    fn new() -> AppBarOverflowStateImpl {
        AppBarOverflowStateImpl { total_item_count: mutable_state_of(0), visible_item_count: mutable_state_of(0) }
    }

    /// Saves the two counts, restored as a state of them (Compose's `AppBarOverflowStateImpl.Saver`).
    #[allow(non_snake_case)]
    fn Saver() -> Saver<Rc<AppBarOverflowStateImpl>> {
        list_saver(
            |it: &Rc<AppBarOverflowStateImpl>| vec![it.total_item_count().save(), it.visible_item_count().save()],
            |saved: &[SavedValue]| {
                let state = AppBarOverflowStateImpl::new();
                state.set_total_item_count(i32::restore(saved.first()?)?);
                state.set_visible_item_count(i32::restore(saved.get(1)?)?);
                Some(Rc::new(state))
            },
        )
    }
}

impl AppBarOverflowState for AppBarOverflowStateImpl {
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

/// The measure policy of a row's or column's two contents — the items, the overflow indicator (with its menu) —
/// (Compose's internal `OverflowMeasurePolicy`): room is kept for the indicator; the items are measured one by one
/// and put in while they fit, the last one also taking the indicator's room, and at most `max_item_count` slots
/// (the indicator taking the last when items are left out); the indicator only when some are left out (in the
/// room left). Side by side (or one under the other, `is_vertical`), as long as they are, as wide (tall) as the
/// widest (tallest).
pub(crate) struct OverflowMeasurePolicy {
    pub(crate) overflow_state: Rc<dyn AppBarOverflowState>,
    pub(crate) max_item_count: i32,
    pub(crate) is_vertical: bool,
}

impl MultiContentMeasurePolicy for OverflowMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let is_vertical = self.is_vertical;
        let loose_constraints = kompoz_runtime::named_args!(constraints.copy(min_width = 0, min_height = 0));
        let (content_measurables, overflow_measurables) = (&measurables[0], &measurables[1]);

        self.overflow_state.set_total_item_count(content_measurables.len() as i32);

        // First, reserve space for the overflow indicator
        let reserved_overflow_space = if is_vertical {
            overflow_measurables.iter().map(|it| it.max_intrinsic_height(constraints.max_width)).max().unwrap_or(0)
        } else {
            overflow_measurables.iter().map(|it| it.max_intrinsic_width(constraints.max_height)).max().unwrap_or(0)
        };
        let mut remaining_space = if is_vertical { constraints.max_height } else { constraints.max_width };
        remaining_space = remaining_space.subtract_constraint_safely(reserved_overflow_space);

        let mut current_space = 0;
        let mut content_placeables: Vec<Placeable> = Vec::new();

        // Measure content until it doesn't fit
        let last_index = content_measurables.len() as i32 - 1;
        for (i, measurable) in content_measurables.iter().enumerate() {
            let i = i as i32;
            let placeable = measurable.measure(loose_constraints);
            let is_last_content = i == last_index;
            if !is_last_content && (i == self.max_item_count.wrapping_sub(1)) {
                break;
            }
            let placeable_space = if is_vertical { placeable.height() } else { placeable.width() };
            let has_sufficient_space = placeable_space <= remaining_space
                || (is_last_content && placeable_space <= remaining_space.saturating_add(reserved_overflow_space));

            if has_sufficient_space {
                current_space += placeable_space;
                remaining_space = remaining_space.subtract_constraint_safely(placeable_space);
                content_placeables.push(placeable);
            } else {
                break;
            }
        }

        self.overflow_state.set_visible_item_count(content_placeables.len() as i32);

        // Measure overflow if needed
        let overflow_placeables: Option<Vec<Placeable>> = if content_placeables.len() != content_measurables.len() {
            let room = remaining_space.saturating_add(reserved_overflow_space);
            let overflow_constraints = if is_vertical {
                kompoz_runtime::named_args!(loose_constraints.copy(max_height = room))
            } else {
                kompoz_runtime::named_args!(loose_constraints.copy(max_width = room))
            };
            Some(overflow_measurables.iter().map(|it| it.measure(overflow_constraints)).collect())
        } else {
            None
        };

        let max_of = |list: &[Placeable], of: fn(&Placeable) -> i32| list.iter().map(of).max().unwrap_or(0);
        let overflow_list: &[Placeable] = overflow_placeables.as_deref().unwrap_or(&[]);
        let overflow_space = if is_vertical {
            max_of(overflow_list, Placeable::height)
        } else {
            max_of(overflow_list, Placeable::width)
        };
        current_space += overflow_space;

        let children_max_space = if is_vertical {
            max_of(&content_placeables, Placeable::width).max(max_of(overflow_list, Placeable::width))
        } else {
            max_of(&content_placeables, Placeable::height).max(max_of(overflow_list, Placeable::height))
        };

        let overflow_placeables = overflow_placeables.unwrap_or_default();
        if is_vertical {
            let width = constraints.constrain_width(children_max_space);
            let height = constraints.constrain_height(current_space);
            scope.layout(width, height, move |p| {
                let mut current_y = 0;
                for it in &content_placeables {
                    p.place_relative(it, 0, current_y);
                    current_y += it.height();
                }
                for it in &overflow_placeables {
                    p.place_relative(it, 0, current_y);
                }
            })
        } else {
            let width = constraints.constrain_width(current_space);
            let height = constraints.constrain_height(children_max_space);
            scope.layout(width, height, move |p| {
                let mut current_x = 0;
                for it in &content_placeables {
                    p.place_relative(it, current_x, 0);
                    current_x += it.width();
                }
                for it in &overflow_placeables {
                    p.place_relative(it, current_x, 0);
                }
            })
        }
    }
}

/// The default overflow indicator of an [`AppBarRow`](crate::AppBarRow) and an
/// [`AppBarColumn`](crate::AppBarColumn) (Compose's `AppBarOverflowIndicator`): an [`IconButton`] of `modifier`,
/// `enabled`, `shape`, `colors` and `interaction_source` showing a ⋮ icon ("More options", in the platform's
/// language, its description and a plain tooltip above it), which shows the menu of `menu_state`, or closes it when
/// it shows.
#[kompozable]
pub fn AppBarOverflowIndicator(
    menu_state: AppBarMenuState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::standard_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let content_description = get_string(Strings::FloatingToolbarMoreOptions);

    LabelTooltipBox(content_description.clone(), move || {
        let (menu_state, content_description) = (menu_state.clone(), content_description.clone());
        IconButton(
            on_click = move || {
                if menu_state.is_showing() { menu_state.dismiss() } else { menu_state.show() }
            },
            modifier = modifier.clone(),
            enabled = enabled,
            shape = shape.clone(),
            colors = colors,
            interaction_source = interaction_source.clone(),
            content =
                move || VectorIcon(Icons::Filled::MoreVert(), content_description = Some(content_description.clone())),
        );
    });
}

/// An overflow state compared by its instance (Compose's `remember(overflowState, …)` key).
pub(crate) struct OverflowStateKey(pub(crate) Rc<dyn AppBarOverflowState>);

impl PartialEq for OverflowStateKey {
    fn eq(&self, other: &OverflowStateKey) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
