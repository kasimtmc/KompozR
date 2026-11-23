//! The navigation suite scaffold, as Compose's NavigationSuiteScaffold.kt (material3-adaptive-navigation-suite
//! 1.5.0-alpha17): a screen whose navigation is a short bar, a wide rail (collapsed or expanded), a bar, a rail or
//! a permanent drawer as the window suits ([`NavigationSuiteScaffold`], [`NavigationSuiteType`],
//! [`NavigationSuiteScaffoldDefaults::navigation_suite_type`]), hidden and shown by its state
//! ([`NavigationSuiteScaffoldState`]); its parts on their own ([`NavigationSuiteScaffoldLayout`],
//! [`NavigationSuite`], [`NavigationSuiteItem`]); the older API of items given to a scope
//! ([`NavigationSuiteScaffoldWithLayoutType`], [`NavigationSuiteScope`]); colors, defaults and the override API.

use crate::window_size_class_helper::{HeightSizeClasses, WidthSizeClasses, WindowSizeClassDp};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, SpringSpec, VectorConverter, animate_float_as_state, spring,
};
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::layout::{Arrangement, VerticalArrangement, WindowInsets, WindowInsetsSides};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp};
use kompoz_material3::{
    BadgedBox, DrawerDefaults, MaterialTheme, NavigationBar, NavigationBarDefaults, NavigationBarItem,
    NavigationBarItemColors, NavigationBarItemDefaults, NavigationDrawerItem, NavigationDrawerItemColors,
    NavigationDrawerItemDefaults, NavigationItemColors, NavigationItemIconPosition, NavigationRail,
    NavigationRailDefaults, NavigationRailItem, NavigationRailItemColors, NavigationRailItemDefaults,
    PermanentDrawerSheet, ShortNavigationBar, ShortNavigationBarDefaults, ShortNavigationBarItem,
    ShortNavigationBarItemDefaults, Surface, Text, WideNavigationRail, WideNavigationRailColors,
    WideNavigationRailDefaults, WideNavigationRailItem, WideNavigationRailItemDefaults, WideNavigationRailValue,
    content_color_for, remember_wide_navigation_rail_state,
};
use kompoz_material3_adaptive::{WindowAdaptiveInfo, current_window_adaptive_info};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{
    DerivedState, KompozitionLocal, MovableContent, Saveable, SavedValue, Saver, State, derived_state_of,
    derived_state_of_with_policy, kompozable, kompozable_block, movable_content_of, named, named_args,
    never_equal_policy, remember, remember_saveable_with, remember_updated_state,
};
use kompoz_ui::layout::{Layout, measure_policy};
use kompoz_ui::{Alignment, HorizontalAlignment, Modifier};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- NavigationSuiteScaffoldValue and NavigationSuiteScaffoldState ---------------------------------------------

/// Whether a navigation suite scaffold's navigation component is shown (Compose's enum
/// `NavigationSuiteScaffoldValue`: the possible values of a [`NavigationSuiteScaffoldState`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NavigationSuiteScaffoldValue {
    /// The state of the navigation component of the scaffold when it's visible.
    Visible,
    /// The state of the navigation component of the scaffold when it's hidden.
    Hidden,
}

impl NavigationSuiteScaffoldValue {
    /// Its values in order (Kotlin's `entries`).
    pub const ENTRIES: [NavigationSuiteScaffoldValue; 2] =
        [NavigationSuiteScaffoldValue::Visible, NavigationSuiteScaffoldValue::Hidden];

    /// Whether it is [`Visible`](Self::Visible) (Compose's internal `isVisible`).
    pub(crate) fn is_visible(self) -> bool {
        self == NavigationSuiteScaffoldValue::Visible
    }
}

/// Kotlin's `toString` of an enum entry: its name.
impl fmt::Display for NavigationSuiteScaffoldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NavigationSuiteScaffoldValue::Visible => "Visible",
            NavigationSuiteScaffoldValue::Hidden => "Hidden",
        })
    }
}

impl KtToString for NavigationSuiteScaffoldValue {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl Saveable for NavigationSuiteScaffoldValue {
    fn save(&self) -> SavedValue {
        SavedValue::Int(*self as i64)
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        NavigationSuiteScaffoldValue::ENTRIES.get(ordinal).copied()
    }
}

/// A state an app hoists to watch a navigation suite scaffold's navigation component and to hide or show it
/// (Compose's interface `NavigationSuiteScaffoldState`); [`remember_navigation_suite_scaffold_state`] makes the
/// default one.
pub trait NavigationSuiteScaffoldState {
    /// Whether the state is currently animating (Compose's `isAnimating`).
    fn is_animating(&self) -> bool;

    /// Whether the navigation component is going to be shown or hidden (Compose's `targetValue`).
    fn target_value(&self) -> NavigationSuiteScaffoldValue;

    /// Whether the navigation component is currently shown or hidden (Compose's `currentValue`).
    fn current_value(&self) -> NavigationSuiteScaffoldValue;

    /// Hides the navigation component with animation, done when it is fully hidden (Compose's suspending `hide`).
    fn hide(&self) -> LocalFuture<'static, ()>;

    /// Shows the navigation component with animation, done when it is fully shown (Compose's suspending `show`).
    fn show(&self) -> LocalFuture<'static, ()>;

    /// Hides the navigation component with animation if it is shown, or shows it otherwise, done when it is there
    /// (Compose's suspending `toggle`).
    fn toggle(&self) -> LocalFuture<'static, ()>;

    /// Sets the state to `target_value` without any animation (Compose's suspending `snapTo`).
    fn snap_to(&self, target_value: NavigationSuiteScaffoldValue) -> LocalFuture<'static, ()>;
}

impl PartialEq for dyn NavigationSuiteScaffoldState {
    /// The same state (Kotlin's `==` of a class without `equals`: its identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default navigation suite scaffold state, `initial_value` at first, saved and restored (Compose's
/// `rememberNavigationSuiteScaffoldState`): it moves on a spring (damping 0.9, stiffness 700).
///
/// Kotlin: `rememberNavigationSuiteScaffoldState(initialValue: NavigationSuiteScaffoldValue = NavigationSuiteScaffoldValue.Visible): NavigationSuiteScaffoldState`
#[kompozable]
pub fn remember_navigation_suite_scaffold_state(
    #[default(NavigationSuiteScaffoldValue::Visible)] initial_value: NavigationSuiteScaffoldValue,
) -> Rc<dyn NavigationSuiteScaffoldState> {
    remember_saveable_with(&NavigationSuiteScaffoldStateImpl::Saver(), move || {
        Rc::new(NavigationSuiteScaffoldStateImpl::new(initial_value))
    }) as Rc<dyn NavigationSuiteScaffoldState>
}

/// The internal value of a hidden navigation component (Compose's companion's `Hidden`).
const Hidden: f32 = 0.0;
/// The internal value of a visible navigation component (Compose's companion's `Visible`).
const Visible: f32 = 1.0;

/// The default state (Compose's internal `NavigationSuiteScaffoldStateImpl`): an Animatable from 0 (hidden) to 1
/// (visible), moved by the scaffold's spring; its current value is Visible only all the way in.
pub(crate) struct NavigationSuiteScaffoldStateImpl {
    /// The value it was made with (Compose's `var initialValue`, which nothing reads).
    #[allow(dead_code)]
    pub(crate) initial_value: Cell<NavigationSuiteScaffoldValue>,
    internal_state: Animatable<f32, AnimationVector1D>,
    current_val: DerivedState<NavigationSuiteScaffoldValue>,
}

impl NavigationSuiteScaffoldStateImpl {
    pub(crate) fn new(initial_value: NavigationSuiteScaffoldValue) -> NavigationSuiteScaffoldStateImpl {
        let internal_value = if initial_value.is_visible() { Visible } else { Hidden };
        let internal_state = Animatable((internal_value, f32::VectorConverter));
        let current_val = {
            let internal_state = internal_state.clone();
            derived_state_of(move || {
                if internal_state.value() == Visible {
                    NavigationSuiteScaffoldValue::Visible
                } else {
                    NavigationSuiteScaffoldValue::Hidden
                }
            })
        };
        NavigationSuiteScaffoldStateImpl { initial_value: Cell::new(initial_value), internal_state, current_val }
    }

    /// Saves its target value, restored as a state at it (Compose's companion's `Saver()`).
    #[allow(non_snake_case)]
    pub(crate) fn Saver() -> Saver<Rc<NavigationSuiteScaffoldStateImpl>> {
        Saver::new(
            |state: &Rc<NavigationSuiteScaffoldStateImpl>| state.target_value().save(),
            |saved| {
                let value = NavigationSuiteScaffoldValue::restore(saved)?;
                Some(Rc::new(NavigationSuiteScaffoldStateImpl::new(value)))
            },
        )
    }

    /// Animates the internal value to `target` by the scaffold's spring.
    fn animate_internal_to(&self, target: f32) -> LocalFuture<'static, ()> {
        let animation = named_args!(self.internal_state.animate_to(target, animation_spec = AnimationSpec()));
        Box::pin(async move {
            animation.await;
        })
    }
}

impl NavigationSuiteScaffoldState for NavigationSuiteScaffoldStateImpl {
    fn is_animating(&self) -> bool {
        self.internal_state.is_running()
    }

    fn target_value(&self) -> NavigationSuiteScaffoldValue {
        if self.internal_state.target_value() == Visible {
            NavigationSuiteScaffoldValue::Visible
        } else {
            NavigationSuiteScaffoldValue::Hidden
        }
    }

    fn current_value(&self) -> NavigationSuiteScaffoldValue {
        self.current_val.value()
    }

    fn hide(&self) -> LocalFuture<'static, ()> {
        self.animate_internal_to(Hidden)
    }

    fn show(&self) -> LocalFuture<'static, ()> {
        self.animate_internal_to(Visible)
    }

    fn toggle(&self) -> LocalFuture<'static, ()> {
        self.animate_internal_to(if self.target_value().is_visible() { Hidden } else { Visible })
    }

    fn snap_to(&self, target_value: NavigationSuiteScaffoldValue) -> LocalFuture<'static, ()> {
        let target = if target_value.is_visible() { Visible } else { Hidden };
        Box::pin(self.internal_state.snap_to(target))
    }
}

// ---- NavigationSuiteScaffold -------------------------------------------------------------------------------------

/// The navigation suite scaffold (Compose's `NavigationSuiteScaffold` with `navigationItems`): a [`Surface`] of
/// `container_color` (the scheme's background by default) with `content_color` (on background) holding a
/// [`NavigationSuiteScaffoldLayout`] — the [`NavigationSuite`] of `navigation_suite_type` (by default the one the
/// window suits, [`NavigationSuiteScaffoldDefaults::navigation_suite_type`]) in `navigation_suite_colors`, holding
/// `navigation_items` (typically [`NavigationSuiteItem`]s) arranged by `navigation_item_vertical_arrangement` in a
/// vertical one; `primary_action_content` (typically a floating action button) in a vertical component's header or
/// above a horizontal one at `primary_action_content_horizontal_alignment` (the end by default); and the screen's
/// `content` beside or above it, the insets the navigation component keeps clear of consumed while it shows.
/// `state` hides and shows the navigation component, the screen taking its room. Made by
/// [`LocalNavigationSuiteScaffoldWithPrimaryActionOverride`]'s override
/// ([`DefaultNavigationSuiteScaffoldWithPrimaryActionOverride`] unless provided).
///
/// Kotlin: `NavigationSuiteScaffold(navigationItems: @Composable () -> Unit, modifier: Modifier = Modifier, navigationSuiteType: NavigationSuiteType = NavigationSuiteScaffoldDefaults.navigationSuiteType(WindowAdaptiveInfoDefault), navigationSuiteColors: NavigationSuiteColors = NavigationSuiteDefaults.colors(), containerColor: Color = NavigationSuiteScaffoldDefaults.containerColor, contentColor: Color = NavigationSuiteScaffoldDefaults.contentColor, state: NavigationSuiteScaffoldState = rememberNavigationSuiteScaffoldState(), navigationItemVerticalArrangement: Arrangement.Vertical = NavigationSuiteDefaults.verticalArrangement, primaryActionContent: @Composable (() -> Unit) = {}, primaryActionContentHorizontalAlignment: Alignment.Horizontal = NavigationSuiteScaffoldDefaults.primaryActionContentAlignment, content: @Composable () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigationSuiteScaffold(
    navigation_items: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationSuiteScaffoldDefaults::navigation_suite_type(WindowAdaptiveInfoDefault()))]
    navigation_suite_type: NavigationSuiteType,
    #[default(NavigationSuiteDefaults.colors())] navigation_suite_colors: NavigationSuiteColors,
    #[default(NavigationSuiteScaffoldDefaults::container_color())] container_color: Color,
    #[default(NavigationSuiteScaffoldDefaults::content_color())] content_color: Color,
    #[default(remember_navigation_suite_scaffold_state())] state: Rc<dyn NavigationSuiteScaffoldState>,
    #[default(NavigationSuiteDefaults::vertical_arrangement())]
    navigation_item_vertical_arrangement: VerticalArrangement,
    #[default(|| {})] primary_action_content: impl Fn() + 'static,
    #[default(NavigationSuiteScaffoldDefaults::primary_action_content_alignment())]
    primary_action_content_horizontal_alignment: HorizontalAlignment,
    content: impl Fn() + 'static,
) {
    let scope = NavigationSuiteScaffoldWithPrimaryActionOverrideScope {
        navigation_items: remember_lambda(navigation_items),
        modifier,
        navigation_suite_type,
        navigation_suite_colors,
        container_color,
        content_color,
        state,
        navigation_item_vertical_arrangement,
        primary_action_content: remember_lambda(primary_action_content),
        primary_action_content_horizontal_alignment,
        content: remember_lambda(content),
    };
    let override_ = LocalNavigationSuiteScaffoldWithPrimaryActionOverride.current();
    override_.NavigationSuiteScaffoldWithPrimaryAction(&scope);
}

/// The default way a navigation suite scaffold with a primary action is made (Compose's experimental
/// `DefaultNavigationSuiteScaffoldWithPrimaryActionOverride`): as [`NavigationSuiteScaffold`] says.
#[derive(Clone, Copy, Debug)]
pub struct DefaultNavigationSuiteScaffoldWithPrimaryActionOverride;

impl NavigationSuiteScaffoldWithPrimaryActionOverride for DefaultNavigationSuiteScaffoldWithPrimaryActionOverride {
    #[track_caller]
    fn NavigationSuiteScaffoldWithPrimaryAction(&self, scope: &NavigationSuiteScaffoldWithPrimaryActionOverrideScope) {
        let NavigationSuiteScaffoldWithPrimaryActionOverrideScope {
            navigation_items,
            modifier,
            navigation_suite_type,
            navigation_suite_colors,
            container_color,
            content_color,
            state,
            navigation_item_vertical_arrangement,
            primary_action_content,
            primary_action_content_horizontal_alignment,
            content,
        } = scope.clone();
        kompozable_block!({
            Surface(
                modifier = modifier,
                color = container_color,
                content_color = Some(content_color),
                content = || {
                    let (navigation_items, primary_action) = (navigation_items.clone(), primary_action_content.clone());
                    let (content, consumed_state) = (content.clone(), state.clone());
                    NavigationSuiteScaffoldLayout(
                        navigation_suite = move || {
                            let (navigation_items, primary_action) = (navigation_items.clone(), primary_action.clone());
                            NavigationSuite(
                                navigation_suite_type = navigation_suite_type,
                                colors = navigation_suite_colors,
                                primary_action_content = move || primary_action(),
                                vertical_arrangement = navigation_item_vertical_arrangement.clone(),
                                content = move || navigation_items(),
                            );
                        },
                        navigation_suite_type = navigation_suite_type,
                        state = state.clone(),
                        primary_action_content = move || primary_action_content(),
                        primary_action_content_horizontal_alignment =
                            primary_action_content_horizontal_alignment.clone(),
                        content = move || {
                            let content = content.clone();
                            Box(
                                modifier = navigation_suite_scaffold_consume_window_insets(
                                    Modifier,
                                    navigation_suite_type,
                                    &*consumed_state,
                                ),
                                content = move || content(),
                            );
                        },
                    );
                },
            );
        })
    }
}

/// The navigation suite scaffold of items given to a [`NavigationSuiteScope`] (Compose's `NavigationSuiteScaffold`
/// with `navigationSuiteItems`; Compose recommends the one of [`NavigationSuiteItem`]s,
/// [`NavigationSuiteScaffold`]): a [`Surface`] of `container_color` with `content_color` holding a
/// [`NavigationSuiteScaffoldLayout`] of the [`NavigationSuiteWithLayoutType`] of `layout_type` (by default
/// [`NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info`]: a bar or a rail) and the screen's `content`,
/// the insets of a bar, a rail or a drawer consumed while it shows. Made by
/// [`LocalNavigationSuiteScaffoldOverride`]'s override ([`DefaultNavigationSuiteScaffoldOverride`] unless provided).
///
/// Kotlin: `NavigationSuiteScaffold(navigationSuiteItems: NavigationSuiteScope.() -> Unit, modifier: Modifier = Modifier, layoutType: NavigationSuiteType = NavigationSuiteScaffoldDefaults.calculateFromAdaptiveInfo(WindowAdaptiveInfoDefault), navigationSuiteColors: NavigationSuiteColors = NavigationSuiteDefaults.colors(), containerColor: Color = NavigationSuiteScaffoldDefaults.containerColor, contentColor: Color = NavigationSuiteScaffoldDefaults.contentColor, state: NavigationSuiteScaffoldState = rememberNavigationSuiteScaffoldState(), content: @Composable () -> Unit = {})`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigationSuiteScaffoldWithLayoutType(
    navigation_suite_items: impl Fn(&mut NavigationSuiteScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info(WindowAdaptiveInfoDefault()))]
    layout_type: NavigationSuiteType,
    #[default(NavigationSuiteDefaults.colors())] navigation_suite_colors: NavigationSuiteColors,
    #[default(NavigationSuiteScaffoldDefaults::container_color())] container_color: Color,
    #[default(NavigationSuiteScaffoldDefaults::content_color())] content_color: Color,
    #[default(remember_navigation_suite_scaffold_state())] state: Rc<dyn NavigationSuiteScaffoldState>,
    #[default(|| {})] content: impl Fn() + 'static,
) {
    let scope = NavigationSuiteScaffoldOverrideScope {
        navigation_suite_items: remember_scope_lambda(move |s| navigation_suite_items(s)),
        modifier,
        layout_type,
        navigation_suite_colors,
        container_color,
        content_color,
        state,
        content: remember_lambda(content),
    };
    let override_ = LocalNavigationSuiteScaffoldOverride.current();
    override_.NavigationSuiteScaffold(&scope);
}

/// The default way a navigation suite scaffold of scope items is made (Compose's experimental
/// `DefaultNavigationSuiteScaffoldOverride`): as [`NavigationSuiteScaffoldWithLayoutType`] says — only a bar's, a
/// rail's or a drawer's insets are consumed (Compose's: the short bar and the wide rail types consume none here).
#[derive(Clone, Copy, Debug)]
pub struct DefaultNavigationSuiteScaffoldOverride;

impl NavigationSuiteScaffoldOverride for DefaultNavigationSuiteScaffoldOverride {
    #[track_caller]
    fn NavigationSuiteScaffold(&self, scope: &NavigationSuiteScaffoldOverrideScope) {
        let NavigationSuiteScaffoldOverrideScope {
            navigation_suite_items,
            modifier,
            layout_type,
            navigation_suite_colors,
            container_color,
            content_color,
            state,
            content,
        } = scope.clone();
        kompozable_block!({
            Surface(
                modifier = modifier,
                color = container_color,
                content_color = Some(content_color),
                content = || {
                    let (items, content, consumed_state) =
                        (navigation_suite_items.clone(), content.clone(), state.clone());
                    NavigationSuiteScaffoldLayoutWithLayoutType(
                        navigation_suite = move || {
                            let items = items.clone();
                            NavigationSuiteWithLayoutType(
                                layout_type = layout_type,
                                colors = navigation_suite_colors,
                                content = move |s: &mut NavigationSuiteScope| items(s),
                            );
                        },
                        state = state.clone(),
                        layout_type = layout_type,
                        content = move || {
                            let insets = if consumed_state.current_value() == NavigationSuiteScaffoldValue::Hidden
                                && !consumed_state.is_animating()
                            {
                                NoWindowInsets()
                            } else {
                                match layout_type {
                                    NavigationSuiteType::NavigationBar => {
                                        NavigationBarDefaults::window_insets().only(WindowInsetsSides::Bottom)
                                    }
                                    NavigationSuiteType::NavigationRail => {
                                        NavigationRailDefaults::window_insets().only(WindowInsetsSides::Start)
                                    }
                                    NavigationSuiteType::NavigationDrawer => {
                                        DrawerDefaults::window_insets().only(WindowInsetsSides::Start)
                                    }
                                    _ => NoWindowInsets(),
                                }
                            };
                            let content = content.clone();
                            Box(modifier = Modifier.consume_window_insets(insets), content = move || content());
                        },
                    );
                },
            );
        })
    }
}

// ---- NavigationSuiteScaffoldLayout -------------------------------------------------------------------------------

/// The layout of a [`NavigationSuiteScaffold`]'s parts (Compose's `NavigationSuiteScaffoldLayout` with
/// `navigationSuiteType`), for what the scaffold can't be customized to: as big as it may be, `navigation_suite`
/// (typically a [`NavigationSuite`]) along the bottom for a bar type (the short bars and the bar) and along the
/// start otherwise, `content` taking the rest exactly — each measured in a Box of its own with no minimum size;
/// `primary_action_content` 16 dp above a bar at `primary_action_content_horizontal_alignment` (16 dp in at the start
/// or the end, or centered). As `state` hides the navigation component it slides out (down, or out at the start)
/// on the scaffold's spring, the content following it into its room.
///
/// Kotlin: `NavigationSuiteScaffoldLayout(navigationSuite: @Composable () -> Unit, navigationSuiteType: NavigationSuiteType, state: NavigationSuiteScaffoldState = rememberNavigationSuiteScaffoldState(), primaryActionContent: @Composable (() -> Unit) = {}, primaryActionContentHorizontalAlignment: Alignment.Horizontal = NavigationSuiteScaffoldDefaults.primaryActionContentAlignment, content: @Composable () -> Unit)`
#[kompozable]
pub fn NavigationSuiteScaffoldLayout(
    navigation_suite: impl Fn() + 'static,
    navigation_suite_type: NavigationSuiteType,
    #[default(remember_navigation_suite_scaffold_state())] state: Rc<dyn NavigationSuiteScaffoldState>,
    #[default(|| {})] primary_action_content: impl Fn() + 'static,
    #[default(NavigationSuiteScaffoldDefaults::primary_action_content_alignment())]
    primary_action_content_horizontal_alignment: HorizontalAlignment,
    content: impl Fn() + 'static,
) {
    let animation_progress = animate_float_as_state(
        if state.current_value() == NavigationSuiteScaffoldValue::Hidden { 0.0 } else { 1.0 },
        animation_spec = AnimationSpec(),
    );

    Layout(
        content = move || {
            // Wrap the navigation suite and content composables each in a Box to not propagate the parent's
            // (Surface) min constraints to its children (see b/312664933).
            Box(modifier = Modifier.layout_id(NavigationSuiteLayoutIdTag), content = || navigation_suite());
            Box(modifier = Modifier.layout_id(PrimaryActionContentLayoutIdTag), content = || primary_action_content());
            Box(modifier = Modifier.layout_id(ContentLayoutIdTag), content = || content());
        },
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let animation_progress = animation_progress.value();
            let by_tag = |tag: &'static str| {
                *measurables
                    .iter()
                    .find(|m| m.layout_id().is_some_and(|id| id.is(&tag)))
                    .expect("Collection contains no element matching the predicate.")
            };
            let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
            // Find the navigation suite composable through it's layoutId tag
            let navigation_placeable = by_tag(NavigationSuiteLayoutIdTag).measure(loose_constraints);
            let primary_action_content_placeable = by_tag(PrimaryActionContentLayoutIdTag).measure(loose_constraints);
            let is_navigation_bar = navigation_suite_type.is_navigation_bar();
            let layout_height = constraints.max_height;
            let layout_width = constraints.max_width;
            // Find the content composable through it's layoutId tag.
            let content_placeable = by_tag(ContentLayoutIdTag).measure(if is_navigation_bar {
                let height = layout_height - (navigation_placeable.height() as f32 * animation_progress) as i32;
                named_args!(constraints.copy(min_height = height, max_height = height))
            } else {
                let width = layout_width - (navigation_placeable.width() as f32 * animation_progress) as i32;
                named_args!(constraints.copy(min_width = width, max_width = width))
            });

            let padding = scope.round_to_px(PrimaryActionContentPadding);
            let alignment = primary_action_content_horizontal_alignment.clone();
            named_args!(scope.layout(
                layout_width,
                layout_height,
                placement_block = move |p| {
                    if is_navigation_bar {
                        // Place content above the navigation component.
                        p.place_relative(&content_placeable, 0, 0);
                        // Place the navigation component at the bottom of the screen.
                        p.place_relative(
                            &navigation_placeable,
                            0,
                            layout_height - (navigation_placeable.height() as f32 * animation_progress) as i32,
                        );
                        // Place the primary action content above the navigation component.
                        let position_x = if alignment == Alignment::Start {
                            padding
                        } else if alignment == Alignment::CenterHorizontally {
                            (layout_width - primary_action_content_placeable.width()) / 2
                        } else {
                            layout_width - primary_action_content_placeable.width() - padding
                        };
                        p.place_relative(
                            &primary_action_content_placeable,
                            position_x,
                            layout_height
                                - primary_action_content_placeable.height()
                                - padding
                                - (navigation_placeable.height() as f32 * animation_progress) as i32,
                        );
                    } else {
                        // Place the navigation component at the start of the screen.
                        p.place_relative(
                            &navigation_placeable,
                            (0.0 - navigation_placeable.width() as f32 * (1.0 - animation_progress)) as i32,
                            0,
                        );
                        // Place content to the side of the navigation component.
                        p.place_relative(
                            &content_placeable,
                            (navigation_placeable.width() as f32 * animation_progress) as i32,
                            0,
                        );
                    }
                }
            ))
        }),
    );
}

/// The layout of a navigation suite scaffold's navigation component and content by a layout type (Compose's
/// `NavigationSuiteScaffoldLayout` with `layoutType`; Compose recommends the one with `navigationSuiteType`,
/// [`NavigationSuiteScaffoldLayout`]): that one, without a primary action. `layout_type` defaults to
/// [`NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info`].
///
/// Kotlin: `NavigationSuiteScaffoldLayout(navigationSuite: @Composable () -> Unit, layoutType: NavigationSuiteType = NavigationSuiteScaffoldDefaults.calculateFromAdaptiveInfo(WindowAdaptiveInfoDefault), state: NavigationSuiteScaffoldState = rememberNavigationSuiteScaffoldState(), content: @Composable () -> Unit = {})`
#[kompozable]
pub fn NavigationSuiteScaffoldLayoutWithLayoutType(
    navigation_suite: impl Fn() + 'static,
    #[default(NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info(WindowAdaptiveInfoDefault()))]
    layout_type: NavigationSuiteType,
    #[default(remember_navigation_suite_scaffold_state())] state: Rc<dyn NavigationSuiteScaffoldState>,
    #[default(|| {})] content: impl Fn() + 'static,
) {
    NavigationSuiteScaffoldLayout(
        navigation_suite = navigation_suite,
        navigation_suite_type = layout_type,
        state = state,
        content = content,
    );
}

// ---- NavigationSuite ---------------------------------------------------------------------------------------------

/// The Material navigation component of a navigation suite type (Compose's `NavigationSuite` with
/// `navigationSuiteType`), for a [`NavigationSuiteScaffold`]: a [`ShortNavigationBar`] for the short bar types, a
/// [`WideNavigationRail`] (collapsed, or expanded from the start) for the wide rail ones with
/// `primary_action_content` as its header and `vertical_arrangement`, a short bar at least 80 dp tall in the bar's
/// colors for [`NavigationSuiteType::NavigationBar`] (Compose's: a [`NavigationBar`] would need its RowScope items),
/// a [`NavigationRail`] (`primary_action_content` its header) and a [`PermanentDrawerSheet`] (it first) with weighted
/// spacers placing the items by a Center or Bottom `vertical_arrangement`; nothing for
/// [`NavigationSuiteType::None`]. Each in `colors`; `content` (typically [`NavigationSuiteItem`]s) is movable
/// content: its items keep their state as the type changes.
///
/// Kotlin: `NavigationSuite(navigationSuiteType: NavigationSuiteType, modifier: Modifier = Modifier, colors: NavigationSuiteColors = NavigationSuiteDefaults.colors(), verticalArrangement: Arrangement.Vertical = NavigationSuiteDefaults.verticalArrangement, primaryActionContent: @Composable (() -> Unit) = {}, content: @Composable () -> Unit)`
#[kompozable]
pub fn NavigationSuite(
    navigation_suite_type: NavigationSuiteType,
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationSuiteDefaults.colors())] colors: NavigationSuiteColors,
    #[default(NavigationSuiteDefaults::vertical_arrangement())] vertical_arrangement: VerticalArrangement,
    #[default(|| {})] primary_action_content: impl Fn() + 'static,
    content: impl Fn() + 'static,
) {
    let movable_content = remember_movable_content(content);
    let header: Rc<dyn Fn()> = Rc::new(primary_action_content);
    match navigation_suite_type {
        NavigationSuiteType::ShortNavigationBarCompact => {
            ShortNavigationBar(
                modifier = modifier,
                container_color = colors.short_navigation_bar_container_color,
                content_color = Some(colors.short_navigation_bar_content_color),
                content = move || movable_content.call(),
            );
        }
        NavigationSuiteType::ShortNavigationBarMedium => {
            ShortNavigationBar(
                modifier = modifier,
                container_color = colors.short_navigation_bar_container_color,
                content_color = Some(colors.short_navigation_bar_content_color),
                content = move || movable_content.call(),
            );
        }
        NavigationSuiteType::WideNavigationRailCollapsed => {
            WideNavigationRail(
                modifier = modifier,
                header = Some(header),
                arrangement = vertical_arrangement,
                colors = colors.wide_navigation_rail_colors,
                content = move || movable_content.call(),
            );
        }
        NavigationSuiteType::WideNavigationRailExpanded => {
            WideNavigationRail(
                modifier = modifier,
                header = Some(header),
                state = Some(remember_wide_navigation_rail_state(initial_value = WideNavigationRailValue::Expanded)),
                arrangement = vertical_arrangement,
                colors = colors.wide_navigation_rail_colors,
                content = move || movable_content.call(),
            );
        }
        // Note: This function does not support providing a NavigationBar for the NavigationSuiteType.NavigationBar
        // type instead provides a ShortNavigationBar with a taller height so that it is visually the same.
        // It's advised to to use NavigationSuiteType.ShortNavigationBarVerticalItems instead.
        NavigationSuiteType::NavigationBar => {
            ShortNavigationBar(
                modifier = modifier.height_in(min = TallNavigationBarHeight),
                container_color = colors.navigation_bar_container_color,
                content_color = Some(colors.navigation_bar_content_color),
                content = move || movable_content.call(),
            );
        }
        // It's advised to to use NavigationSuiteType.WideNavigationRail instead of
        // NavigationSuiteType.NavigationRail.
        NavigationSuiteType::NavigationRail => {
            NavigationRail(
                modifier = modifier,
                header = Some(header),
                container_color = colors.navigation_rail_container_color,
                content_color = Some(colors.navigation_rail_content_color),
                content = move || {
                    if vertical_arrangement == VerticalArrangement::Center
                        || vertical_arrangement == VerticalArrangement::Bottom
                    {
                        Spacer(Modifier.weight(1.0));
                    }
                    movable_content.call();
                    if vertical_arrangement == VerticalArrangement::Center {
                        Spacer(Modifier.weight(1.0));
                    }
                },
            );
        }
        // It's advised to to use NavigationSuiteType.WideNavigationRail instead of
        // NavigationSuiteType.NavigationDrawer.
        NavigationSuiteType::NavigationDrawer => {
            PermanentDrawerSheet(
                modifier = modifier,
                drawer_container_color = colors.navigation_drawer_container_color,
                drawer_content_color = Some(colors.navigation_drawer_content_color),
                content = move || {
                    header();
                    if vertical_arrangement == VerticalArrangement::Center
                        || vertical_arrangement == VerticalArrangement::Bottom
                    {
                        Spacer(Modifier.weight(1.0));
                    }
                    movable_content.call();
                    if vertical_arrangement == VerticalArrangement::Center {
                        Spacer(Modifier.weight(1.0));
                    }
                },
            );
        }
        // (Kotlin's `when` statement without a branch for it: None shows nothing.)
        _ => {}
    }
}

/// The Material navigation component of a layout type, its items given to a [`NavigationSuiteScope`] (Compose's
/// `NavigationSuite` with `layoutType`; Compose recommends the one of [`NavigationSuiteItem`]s,
/// [`NavigationSuite`]): a [`NavigationBar`] of [`NavigationBarItem`]s, a [`NavigationRail`] of
/// [`NavigationRailItem`]s or a [`PermanentDrawerSheet`] of [`NavigationDrawerItem`]s (an item without a label: an
/// empty text), each item in its own colors or [`NavigationSuiteDefaults::item_colors`]'; nothing for
/// [`NavigationSuiteType::None`]; any other type the [`NavigationSuite`] of it with the items as
/// [`NavigationSuiteItem`]s. `layout_type` defaults to [`NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info`].
/// The items are collected again when `content` is another one (or what it reads changes).
///
/// Kotlin: `NavigationSuite(modifier: Modifier = Modifier, layoutType: NavigationSuiteType = NavigationSuiteScaffoldDefaults.calculateFromAdaptiveInfo(WindowAdaptiveInfoDefault), colors: NavigationSuiteColors = NavigationSuiteDefaults.colors(), content: NavigationSuiteScope.() -> Unit)`
#[kompozable]
pub fn NavigationSuiteWithLayoutType(
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info(WindowAdaptiveInfoDefault()))]
    layout_type: NavigationSuiteType,
    #[default(NavigationSuiteDefaults.colors())] colors: NavigationSuiteColors,
    content: impl Fn(&mut NavigationSuiteScope) + 'static,
) {
    let scope = remember_state_of_items(move |s| content(s)).value();
    // Define defaultItemColors here since we can't set NavigationSuiteDefaults.itemColors() as a default for the
    // colors param of the NavigationSuiteScope.item non-composable function.
    let default_item_colors = NavigationSuiteDefaults.item_colors();

    match layout_type {
        NavigationSuiteType::NavigationBar => {
            NavigationBar(
                modifier = modifier,
                container_color = colors.navigation_bar_container_color,
                content_color = Some(colors.navigation_bar_content_color),
                content = move || {
                    for it in scope.item_list() {
                        let (icon, badge) = (it.icon.clone(), it.badge.clone());
                        let on_click = it.on_click.clone();
                        NavigationBarItem(
                            modifier = it.modifier.clone(),
                            selected = it.selected,
                            on_click = move || on_click(),
                            icon = move || NavigationItemIcon(icon.clone(), badge.clone()),
                            enabled = it.enabled,
                            label = it.label.clone(),
                            always_show_label = it.always_show_label,
                            colors = it
                                .colors
                                .as_ref()
                                .map(|c| c.navigation_bar_item_colors)
                                .unwrap_or(default_item_colors.navigation_bar_item_colors),
                            interaction_source = it.interaction_source.clone(),
                        );
                    }
                },
            );
        }
        NavigationSuiteType::NavigationRail => {
            NavigationRail(
                modifier = modifier,
                container_color = colors.navigation_rail_container_color,
                content_color = Some(colors.navigation_rail_content_color),
                content = move || {
                    for it in scope.item_list() {
                        let (icon, badge) = (it.icon.clone(), it.badge.clone());
                        let on_click = it.on_click.clone();
                        NavigationRailItem(
                            modifier = it.modifier.clone(),
                            selected = it.selected,
                            on_click = move || on_click(),
                            icon = move || NavigationItemIcon(icon.clone(), badge.clone()),
                            enabled = it.enabled,
                            label = it.label.clone(),
                            always_show_label = it.always_show_label,
                            colors = it
                                .colors
                                .as_ref()
                                .map(|c| c.navigation_rail_item_colors)
                                .unwrap_or(default_item_colors.navigation_rail_item_colors),
                            interaction_source = it.interaction_source.clone(),
                        );
                    }
                },
            );
        }
        NavigationSuiteType::NavigationDrawer => {
            PermanentDrawerSheet(
                modifier = modifier,
                drawer_container_color = colors.navigation_drawer_container_color,
                drawer_content_color = Some(colors.navigation_drawer_content_color),
                content = move || {
                    for it in scope.item_list() {
                        let label = it.label.clone();
                        let on_click = it.on_click.clone();
                        NavigationDrawerItem(
                            modifier = it.modifier.clone(),
                            selected = it.selected,
                            on_click = move || on_click(),
                            icon = Some(it.icon.clone()),
                            badge = it.badge.clone(),
                            label = move || match &label {
                                Some(label) => label(),
                                None => Text(""),
                            },
                            colors = it
                                .colors
                                .as_ref()
                                .map(|c| c.navigation_drawer_item_colors.clone())
                                .unwrap_or_else(|| default_item_colors.navigation_drawer_item_colors.clone()),
                            interaction_source = it.interaction_source.clone(),
                        );
                    }
                },
            );
        }
        NavigationSuiteType::None => { /* Do nothing. */ }
        _ => {
            NavigationSuite(
                navigation_suite_type = layout_type,
                modifier = modifier,
                colors = colors,
                content = move || {
                    for it in scope.item_list() {
                        NavigationSuiteItemOf(
                            true,
                            layout_type,
                            it.selected,
                            it.on_click.clone(),
                            it.icon.clone(),
                            it.label.clone(),
                            it.modifier.clone(),
                            it.enabled,
                            it.badge.clone(),
                            None,
                            Some(it.colors.clone().unwrap_or_else(|| NavigationSuiteDefaults.item_colors())),
                            it.interaction_source.clone(),
                        );
                    }
                },
            );
        }
    }
}

// ---- NavigationSuiteItem -----------------------------------------------------------------------------------------

/// The Material navigation item of a navigation suite type (Compose's `NavigationSuiteItem`), for the
/// [`NavigationSuite`] of that type: a [`ShortNavigationBarItem`] (its icon on top of its label for
/// [`NavigationSuiteType::ShortNavigationBarCompact`], at its start for the medium one), a [`WideNavigationRailItem`]
/// (of an expanded rail for [`NavigationSuiteType::WideNavigationRailExpanded`]), a short bar item 8 dp down in the
/// bar item colors for [`NavigationSuiteType::NavigationBar`] (Compose's: a NavigationBarItem would need a RowScope),
/// a [`NavigationRailItem`] or a [`NavigationDrawerItem`] (always enabled; without a label an empty text). `badge`
/// (if any) on the icon through a [`BadgedBox`] (beside the label in a drawer). `colors` None is Material's for
/// that item (for a bar, a rail or a drawer, made of these colors where given); `navigation_suite_type` defaults to
/// [`NavigationSuiteScaffoldDefaults::navigation_suite_type`]. Its interactions go to `interaction_source` (one of
/// its own where none).
///
/// Kotlin: `NavigationSuiteItem(selected: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit, label: @Composable (() -> Unit)?, modifier: Modifier = Modifier, navigationSuiteType: NavigationSuiteType = NavigationSuiteScaffoldDefaults.navigationSuiteType(WindowAdaptiveInfoDefault), enabled: Boolean = true, badge: @Composable (() -> Unit)? = null, colors: NavigationItemColors? = null, interactionSource: MutableInteractionSource? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigationSuiteItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    label: Option<Rc<dyn Fn()>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationSuiteScaffoldDefaults::navigation_suite_type(WindowAdaptiveInfoDefault()))]
    navigation_suite_type: NavigationSuiteType,
    #[default(true)] enabled: bool,
    #[default(None)] badge: Option<Rc<dyn Fn()>>,
    #[default(None)] colors: Option<NavigationItemColors>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    NavigationSuiteItemOf(
        false,
        navigation_suite_type,
        selected,
        Rc::new(on_click),
        Rc::new(icon),
        label,
        modifier,
        enabled,
        badge,
        colors,
        None,
        interaction_source,
    );
}

/// What [`NavigationSuiteItem`] and the scope's items of [`NavigationSuiteWithLayoutType`] compose (Compose's
/// private `NavigationSuiteItem` with `isNavigationSuite`): the item of the type, colored by
/// `navigation_item_colors` (the public item's) or `navigation_suite_item_colors` (the scope's).
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
fn NavigationSuiteItemOf(
    is_navigation_suite: bool,
    navigation_suite_type: NavigationSuiteType,
    selected: bool,
    on_click: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    label: Option<Rc<dyn Fn()>>,
    modifier: Modifier,
    enabled: bool,
    badge: Option<Rc<dyn Fn()>>,
    navigation_item_colors: Option<NavigationItemColors>,
    navigation_suite_item_colors: Option<NavigationSuiteItemColors>,
    interaction_source: Option<MutableInteractionSource>,
) {
    match navigation_suite_type {
        NavigationSuiteType::ShortNavigationBarCompact | NavigationSuiteType::ShortNavigationBarMedium => {
            let icon_position = if navigation_suite_type == NavigationSuiteType::ShortNavigationBarCompact {
                NavigationItemIconPosition::Top
            } else {
                NavigationItemIconPosition::Start
            };
            ShortNavigationBarItem(
                selected = selected,
                on_click = move || on_click(),
                icon = move || NavigationItemIcon(icon.clone(), badge.clone()),
                label = label,
                modifier = modifier,
                enabled = enabled,
                icon_position = icon_position,
                colors = navigation_item_colors.unwrap_or_else(|| ShortNavigationBarItemDefaults.colors()),
                interaction_source = interaction_source,
            );
        }
        NavigationSuiteType::WideNavigationRailCollapsed | NavigationSuiteType::WideNavigationRailExpanded => {
            WideNavigationRailItem(
                rail_expanded = navigation_suite_type == NavigationSuiteType::WideNavigationRailExpanded,
                selected = selected,
                on_click = move || on_click(),
                icon = move || NavigationItemIcon(icon.clone(), badge.clone()),
                label = label,
                modifier = modifier,
                enabled = enabled,
                colors = navigation_item_colors.unwrap_or_else(|| WideNavigationRailItemDefaults.colors()),
                interaction_source = interaction_source,
            );
        }
        // Note: This function does not support providing a NavigationBarItem for the
        // NavigationSuiteType.NavigationBar type due to the NavigationBarItem being limited to RowScope. Instead we
        // provide ShortNavigationBarItem with a top padding so that it is visually the same.
        // It's advised to to use NavigationSuiteType.ShortNavigationBarVerticalItems instead.
        NavigationSuiteType::NavigationBar => {
            let default_colors = navigation_suite_item_colors
                .as_ref()
                .map(|c| c.navigation_bar_item_colors)
                .unwrap_or_else(|| NavigationBarItemDefaults.colors());
            let actual_colors = if (!is_navigation_suite && navigation_item_colors.is_none()) || is_navigation_suite {
                ShortNavigationBarItemDefaults.colors(
                    selected_icon_color = default_colors.selected_icon_color,
                    selected_text_color = default_colors.selected_text_color,
                    selected_indicator_color = default_colors.selected_indicator_color,
                    unselected_icon_color = default_colors.unselected_icon_color,
                    unselected_text_color = default_colors.unselected_text_color,
                    disabled_icon_color = Some(default_colors.disabled_icon_color),
                    disabled_text_color = Some(default_colors.disabled_text_color),
                )
            } else {
                navigation_item_colors.expect("navigationItemColors!!")
            };

            ShortNavigationBarItem(
                selected = selected,
                on_click = move || on_click(),
                icon = move || NavigationItemIcon(icon.clone(), badge.clone()),
                label = label,
                modifier = modifier.padding_sides(top = Dp(8.0)),
                enabled = enabled,
                colors = actual_colors,
                interaction_source = interaction_source,
            );
        }
        // It's advised to to use NavigationSuiteType.WideNavigationRail instead of
        // NavigationSuiteType.NavigationRail.
        NavigationSuiteType::NavigationRail => {
            let actual_colors = if is_navigation_suite {
                navigation_suite_item_colors
                    .as_ref()
                    .map(|c| c.navigation_rail_item_colors)
                    .unwrap_or_else(|| NavigationRailItemDefaults.colors())
            } else if let Some(navigation_item_colors) = navigation_item_colors {
                NavigationRailItemDefaults.colors(
                    selected_icon_color = navigation_item_colors.selected_icon_color,
                    selected_text_color = navigation_item_colors.selected_text_color,
                    indicator_color = navigation_item_colors.selected_indicator_color,
                    unselected_icon_color = navigation_item_colors.unselected_icon_color,
                    unselected_text_color = navigation_item_colors.unselected_text_color,
                    disabled_icon_color = Some(navigation_item_colors.disabled_icon_color),
                    disabled_text_color = Some(navigation_item_colors.disabled_text_color),
                )
            } else {
                NavigationSuiteDefaults.item_colors().navigation_rail_item_colors
            };
            NavigationRailItem(
                selected = selected,
                on_click = move || on_click(),
                icon = move || NavigationItemIcon(icon.clone(), badge.clone()),
                label = label,
                modifier = modifier,
                enabled = enabled,
                colors = actual_colors,
                interaction_source = interaction_source,
            );
        }
        // It's advised to to use NavigationSuiteType.WideNavigationRail instead of
        // NavigationSuiteType.NavigationDrawer.
        NavigationSuiteType::NavigationDrawer => {
            let actual_colors = if is_navigation_suite {
                navigation_suite_item_colors
                    .as_ref()
                    .map(|c| c.navigation_drawer_item_colors.clone())
                    .unwrap_or_else(|| NavigationDrawerItemDefaults.colors())
            } else if let Some(navigation_item_colors) = navigation_item_colors {
                NavigationDrawerItemDefaults.colors(
                    selected_icon_color = navigation_item_colors.selected_icon_color,
                    selected_text_color = navigation_item_colors.selected_text_color,
                    unselected_icon_color = navigation_item_colors.unselected_icon_color,
                    unselected_text_color = navigation_item_colors.unselected_text_color,
                    selected_container_color = navigation_item_colors.selected_indicator_color,
                )
            } else {
                NavigationSuiteDefaults.item_colors().navigation_drawer_item_colors
            };

            NavigationDrawerItem(
                modifier = modifier,
                selected = selected,
                on_click = move || on_click(),
                icon = Some(icon),
                badge = badge,
                label = move || match &label {
                    Some(label) => label(),
                    None => Text(""),
                },
                colors = actual_colors,
                interaction_source = interaction_source,
            );
        }
        // (Kotlin's `when` statement without a branch for it: None shows nothing.)
        _ => {}
    }
}

// ---- NavigationSuiteScope ----------------------------------------------------------------------------------------

/// What the items of a [`NavigationSuiteWithLayoutType`] (and of a [`NavigationSuiteScaffoldWithLayoutType`]) are
/// given to (Compose's sealed interface `NavigationSuiteScope` and its private implementation
/// `NavigationSuiteScopeImpl`): each [`item`](Self::item) called adds one, composed by the navigation suite in
/// order. Made by the navigation suite only (sealed).
pub struct NavigationSuiteScope {
    item_list: Vec<NavigationSuiteScopeItem>,
}

#[named]
impl NavigationSuiteScope {
    /// Adds an item of the default Material navigation item for the navigation suite's type (Compose's `item`):
    /// a [`NavigationBarItem`], a [`NavigationRailItem`] or a [`NavigationDrawerItem`] (always enabled, its label
    /// always shown there), or the [`NavigationSuiteItem`] of the type — `selected`, `on_click` it, its `icon`,
    /// `label` (shown when unselected too while `always_show_label`), `badge`, `colors` (None:
    /// [`NavigationSuiteDefaults::item_colors`]) and `interaction_source` (None: one of its own).
    ///
    /// Kotlin: `item(selected: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, label: @Composable (() -> Unit)? = null, alwaysShowLabel: Boolean = true, badge: (@Composable () -> Unit)? = null, colors: NavigationSuiteItemColors? = null, interactionSource: MutableInteractionSource? = null)`
    #[allow(clippy::too_many_arguments)]
    pub fn item(
        &mut self,
        selected: bool,
        on_click: impl Fn() + 'static,
        icon: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(None)] label: Option<Rc<dyn Fn()>>,
        #[default(true)] always_show_label: bool,
        #[default(None)] badge: Option<Rc<dyn Fn()>>,
        #[default(None)] colors: Option<NavigationSuiteItemColors>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
    ) {
        self.item_list.push(NavigationSuiteScopeItem {
            selected,
            on_click: Rc::new(on_click),
            icon: Rc::new(icon),
            modifier,
            enabled,
            label,
            always_show_label,
            badge,
            colors,
            interaction_source,
        });
    }
}

impl NavigationSuiteScope {
    fn new() -> NavigationSuiteScope {
        NavigationSuiteScope { item_list: Vec::new() }
    }
}

impl NavigationSuiteItemProvider for NavigationSuiteScope {
    fn items_count(&self) -> usize {
        self.item_list.len()
    }

    fn item_list(&self) -> &[NavigationSuiteScopeItem] {
        &self.item_list
    }
}

/// The items a scope collected (Compose's private interface `NavigationSuiteItemProvider`).
trait NavigationSuiteItemProvider {
    #[allow(dead_code)]
    fn items_count(&self) -> usize;
    fn item_list(&self) -> &[NavigationSuiteScopeItem];
}

/// An item a scope collected, its arguments (Compose's private class `NavigationSuiteItem`, named apart from the
/// public composable here).
struct NavigationSuiteScopeItem {
    selected: bool,
    on_click: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    modifier: Modifier,
    enabled: bool,
    label: Option<Rc<dyn Fn()>>,
    always_show_label: bool,
    badge: Option<Rc<dyn Fn()>>,
    colors: Option<NavigationSuiteItemColors>,
    interaction_source: Option<MutableInteractionSource>,
}

/// The latest `content` given, compared as Compose compares a lambda: by its identity as a memoized closure (the
/// same while what it captures is the same).
#[derive(Clone)]
struct LatestItems {
    id: Option<u64>,
    content: Rc<dyn Fn(&mut NavigationSuiteScope)>,
}

impl PartialEq for LatestItems {
    fn eq(&self, other: &Self) -> bool {
        self.id.is_some() && self.id == other.id
    }
}

/// The items `content` adds to a scope, collected again when `content` is another one or what it reads changes
/// (Compose's private `rememberStateOfItems`: `rememberUpdatedState(content)` and a `derivedStateOf` of a scope it
/// is applied to). Compose's derived state has no policy: a new scope tells its readers.
#[kompozable(inline)]
fn remember_state_of_items(
    #[memoized] content: impl Fn(&mut NavigationSuiteScope) + 'static,
) -> DerivedState<Rc<NavigationSuiteScope>> {
    let id = content.id();
    let content: Rc<dyn Fn(&mut NavigationSuiteScope)> = Rc::new(content.into_value());
    let latest_content: State<LatestItems> = remember_updated_state!(LatestItems { id, content });
    remember(move || {
        derived_state_of_with_policy(never_equal_policy(), move || {
            let mut scope = NavigationSuiteScope::new();
            (latest_content.value().content)(&mut scope);
            Rc::new(scope)
        })
    })
}

/// An item's icon, with `badge` (if any) on it through a [`BadgedBox`] (Compose's private `NavigationItemIcon`).
#[kompozable(inline)]
fn NavigationItemIcon(icon: Rc<dyn Fn()>, #[default(None)] badge: Option<Rc<dyn Fn()>>) {
    if let Some(badge) = badge {
        BadgedBox(badge = move || badge(), content = move || icon());
    } else {
        icon();
    }
}

// ---- NavigationSuiteType -----------------------------------------------------------------------------------------

/// What navigation component a [`NavigationSuite`] is to show (Compose's value class `NavigationSuiteType`, its
/// companion's values the associated consts): written as its name (Kotlin's `toString`, its description).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavigationSuiteType {
    description: &'static str,
}

impl NavigationSuiteType {
    const fn new(description: &'static str) -> NavigationSuiteType {
        NavigationSuiteType { description }
    }

    /// A [`ShortNavigationBar`] of vertical [`ShortNavigationBarItem`]s at the bottom of the screen.
    pub const ShortNavigationBarCompact: NavigationSuiteType = NavigationSuiteType::new("ShortNavigationBarCompact");

    /// A [`ShortNavigationBar`] of horizontal [`ShortNavigationBarItem`]s at the bottom of the screen.
    pub const ShortNavigationBarMedium: NavigationSuiteType = NavigationSuiteType::new("ShortNavigationBarMedium");

    /// A collapsed [`WideNavigationRail`] at the start of the screen.
    pub const WideNavigationRailCollapsed: NavigationSuiteType =
        NavigationSuiteType::new("WideNavigationRailCollapsed");

    /// An expanded [`WideNavigationRail`] at the start of the screen.
    pub const WideNavigationRailExpanded: NavigationSuiteType = NavigationSuiteType::new("WideNavigationRailExpanded");

    /// A [`NavigationBar`] at the bottom of the screen. Note: [`ShortNavigationBarCompact`](Self::ShortNavigationBarCompact)
    /// is recommended instead.
    pub const NavigationBar: NavigationSuiteType = NavigationSuiteType::new("NavigationBar");

    /// A [`NavigationRail`] at the start of the screen. Note: [`WideNavigationRailCollapsed`](Self::WideNavigationRailCollapsed)
    /// is recommended instead.
    pub const NavigationRail: NavigationSuiteType = NavigationSuiteType::new("NavigationRail");

    /// A [`PermanentDrawerSheet`] at the start of the screen. Note: [`WideNavigationRailExpanded`](Self::WideNavigationRailExpanded)
    /// is recommended instead.
    pub const NavigationDrawer: NavigationSuiteType = NavigationSuiteType::new("NavigationDrawer");

    /// No navigation component on the screen. Note: hiding it by a [`NavigationSuiteScaffoldState`] is recommended
    /// instead.
    pub const None: NavigationSuiteType = NavigationSuiteType::new("None");

    /// Whether it is one of the bars, along the bottom (Compose's private `isNavigationBar`).
    fn is_navigation_bar(self) -> bool {
        self == NavigationSuiteType::ShortNavigationBarCompact
            || self == NavigationSuiteType::ShortNavigationBarMedium
            || self == NavigationSuiteType::NavigationBar
    }
}

/// Kotlin's `toString`: its description, the type's name.
impl fmt::Display for NavigationSuiteType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description)
    }
}

impl fmt::Debug for NavigationSuiteType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl KtToString for NavigationSuiteType {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self.description);
    }
}

// ---- Defaults and colors -----------------------------------------------------------------------------------------

/// The navigation suite scaffold's defaults (Compose's `NavigationSuiteScaffoldDefaults`).
pub struct NavigationSuiteScaffoldDefaults;

impl NavigationSuiteScaffoldDefaults {
    /// The recommended navigation suite type for a window, as Material specifies (Compose's `navigationSuiteType`):
    /// a compact width, [`NavigationSuiteType::ShortNavigationBarCompact`]; tabletop or a compact height,
    /// [`NavigationSuiteType::ShortNavigationBarMedium`]; else [`NavigationSuiteType::WideNavigationRailCollapsed`].
    ///
    /// Kotlin: `navigationSuiteType(adaptiveInfo: WindowAdaptiveInfo): NavigationSuiteType`
    pub fn navigation_suite_type(adaptive_info: WindowAdaptiveInfo) -> NavigationSuiteType {
        let WindowAdaptiveInfo { window_size_class, window_posture } = adaptive_info;
        if window_size_class.min_width() == WidthSizeClasses::Compact {
            NavigationSuiteType::ShortNavigationBarCompact
        } else if window_posture.is_tabletop || window_size_class.min_height() == HeightSizeClasses::Compact {
            NavigationSuiteType::ShortNavigationBarMedium
        } else {
            NavigationSuiteType::WideNavigationRailCollapsed
        }
    }

    /// The standard navigation suite type for a window (Compose's `calculateFromAdaptiveInfo`; Compose recommends
    /// [`navigation_suite_type`](Self::navigation_suite_type)): tabletop, a compact height or a compact width,
    /// [`NavigationSuiteType::NavigationBar`]; else [`NavigationSuiteType::NavigationRail`].
    ///
    /// Kotlin: `calculateFromAdaptiveInfo(adaptiveInfo: WindowAdaptiveInfo): NavigationSuiteType`
    pub fn calculate_from_adaptive_info(adaptive_info: WindowAdaptiveInfo) -> NavigationSuiteType {
        let WindowAdaptiveInfo { window_size_class, window_posture } = adaptive_info;
        if window_posture.is_tabletop
            || window_size_class.min_height() == HeightSizeClasses::Compact
            || window_size_class.min_width() == WidthSizeClasses::Compact
        {
            NavigationSuiteType::NavigationBar
        } else {
            NavigationSuiteType::NavigationRail
        }
    }

    /// A navigation suite scaffold's background: the scheme's background (Compose's `containerColor`).
    pub fn container_color() -> Color {
        MaterialTheme::color_scheme().background
    }

    /// The color of a navigation suite scaffold's content: the scheme's on background (Compose's `contentColor`).
    pub fn content_color() -> Color {
        MaterialTheme::color_scheme().on_background
    }

    /// Where the primary action goes above a horizontal navigation component: the end (Compose's
    /// `primaryActionContentAlignment`).
    pub fn primary_action_content_alignment() -> HorizontalAlignment {
        Alignment::End
    }
}

/// The navigation suite's defaults (Compose's `NavigationSuiteDefaults`), used as a value
/// (`NavigationSuiteDefaults.colors()`).
pub struct NavigationSuiteDefaults;

impl NavigationSuiteDefaults {
    /// How a vertical navigation component arranges its items: from the top (Compose's `verticalArrangement`).
    pub fn vertical_arrangement() -> VerticalArrangement {
        Arrangement::Top
    }
}

#[named]
impl NavigationSuiteDefaults {
    /// A navigation suite's colors, as Material specifies (Compose's `colors`): each component's container and
    /// content colors (the wide rail's as [`WideNavigationRailColors`]), by default its own defaults'; a content
    /// color None is Kotlin's default, `content_color_for` its container color (the matching content color of a
    /// theme color, else the current `LocalContentColor`). `Color::Transparent` containers show no color.
    ///
    /// Kotlin: `colors(shortNavigationBarContentColor: Color = ShortNavigationBarDefaults.contentColor, shortNavigationBarContainerColor: Color = ShortNavigationBarDefaults.containerColor, wideNavigationRailColors: WideNavigationRailColors = WideNavigationRailDefaults.colors(), navigationBarContainerColor: Color = NavigationBarDefaults.containerColor, navigationBarContentColor: Color = contentColorFor(navigationBarContainerColor), navigationRailContainerColor: Color = NavigationRailDefaults.ContainerColor, navigationRailContentColor: Color = contentColorFor(navigationRailContainerColor), navigationDrawerContainerColor: Color = DrawerDefaults.containerColor, navigationDrawerContentColor: Color = contentColorFor(navigationDrawerContainerColor)): NavigationSuiteColors`
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub fn colors(
        &self,
        #[default(ShortNavigationBarDefaults::content_color())] short_navigation_bar_content_color: Color,
        #[default(ShortNavigationBarDefaults::container_color())] short_navigation_bar_container_color: Color,
        #[default(WideNavigationRailDefaults.colors())] wide_navigation_rail_colors: WideNavigationRailColors,
        #[default(NavigationBarDefaults::container_color())] navigation_bar_container_color: Color,
        #[default(None)] navigation_bar_content_color: Option<Color>,
        #[default(NavigationRailDefaults::ContainerColor())] navigation_rail_container_color: Color,
        #[default(None)] navigation_rail_content_color: Option<Color>,
        #[default(drawer_container_color())] navigation_drawer_container_color: Color,
        #[default(None)] navigation_drawer_content_color: Option<Color>,
    ) -> NavigationSuiteColors {
        NavigationSuiteColors {
            navigation_drawer_content_color: navigation_drawer_content_color
                .unwrap_or_else(|| content_color_for(navigation_drawer_container_color)),
            short_navigation_bar_content_color,
            short_navigation_bar_container_color,
            wide_navigation_rail_colors,
            navigation_bar_container_color,
            navigation_bar_content_color: navigation_bar_content_color
                .unwrap_or_else(|| content_color_for(navigation_bar_container_color)),
            navigation_rail_container_color,
            navigation_rail_content_color: navigation_rail_content_color
                .unwrap_or_else(|| content_color_for(navigation_rail_container_color)),
            navigation_drawer_container_color,
        }
    }

    /// The colors of a [`NavigationSuiteScope::item`] (Compose's `itemColors`): the bar's, the rail's and the
    /// drawer's item colors, by default their own defaults.
    ///
    /// Kotlin: `itemColors(navigationBarItemColors: NavigationBarItemColors = NavigationBarItemDefaults.colors(), navigationRailItemColors: NavigationRailItemColors = NavigationRailItemDefaults.colors(), navigationDrawerItemColors: NavigationDrawerItemColors = NavigationDrawerItemDefaults.colors()): NavigationSuiteItemColors`
    #[track_caller]
    pub fn item_colors(
        &self,
        #[default(NavigationBarItemDefaults.colors())] navigation_bar_item_colors: NavigationBarItemColors,
        #[default(NavigationRailItemDefaults.colors())] navigation_rail_item_colors: NavigationRailItemColors,
        #[default(NavigationDrawerItemDefaults.colors())] navigation_drawer_item_colors: Rc<
            dyn NavigationDrawerItemColors,
        >,
    ) -> NavigationSuiteItemColors {
        NavigationSuiteItemColors {
            navigation_bar_item_colors,
            navigation_rail_item_colors,
            navigation_drawer_item_colors,
        }
    }
}

/// `DrawerDefaults.containerColor` (deprecated in Compose, which uses it here under `@Suppress("DEPRECATION")`).
#[allow(deprecated)]
fn drawer_container_color() -> Color {
    DrawerDefaults::container_color()
}

/// A navigation suite's colors (Compose's `NavigationSuiteColors`, made by [`NavigationSuiteDefaults::colors`]
/// only: its constructor is internal): each navigation component's container (background) and content colors, the
/// wide rail's as its [`WideNavigationRailColors`]. A class without `equals` in Kotlin: no `==` here.
#[derive(Clone, Copy, Debug)]
pub struct NavigationSuiteColors {
    short_navigation_bar_container_color: Color,
    short_navigation_bar_content_color: Color,
    wide_navigation_rail_colors: WideNavigationRailColors,
    navigation_bar_container_color: Color,
    navigation_bar_content_color: Color,
    navigation_rail_container_color: Color,
    navigation_rail_content_color: Color,
    navigation_drawer_container_color: Color,
    navigation_drawer_content_color: Color,
}

impl NavigationSuiteColors {
    /// The container color of the [`ShortNavigationBar`].
    pub fn short_navigation_bar_container_color(&self) -> Color {
        self.short_navigation_bar_container_color
    }

    /// The content color of the [`ShortNavigationBar`].
    pub fn short_navigation_bar_content_color(&self) -> Color {
        self.short_navigation_bar_content_color
    }

    /// The [`WideNavigationRailColors`] of the [`WideNavigationRail`].
    pub fn wide_navigation_rail_colors(&self) -> WideNavigationRailColors {
        self.wide_navigation_rail_colors
    }

    /// The container color of the [`NavigationBar`].
    pub fn navigation_bar_container_color(&self) -> Color {
        self.navigation_bar_container_color
    }

    /// The content color of the [`NavigationBar`].
    pub fn navigation_bar_content_color(&self) -> Color {
        self.navigation_bar_content_color
    }

    /// The container color of the [`NavigationRail`].
    pub fn navigation_rail_container_color(&self) -> Color {
        self.navigation_rail_container_color
    }

    /// The content color of the [`NavigationRail`].
    pub fn navigation_rail_content_color(&self) -> Color {
        self.navigation_rail_content_color
    }

    /// The container color of the [`PermanentDrawerSheet`].
    pub fn navigation_drawer_container_color(&self) -> Color {
        self.navigation_drawer_container_color
    }

    /// The content color of the [`PermanentDrawerSheet`].
    pub fn navigation_drawer_content_color(&self) -> Color {
        self.navigation_drawer_content_color
    }
}

/// The colors of a [`NavigationSuiteScope::item`] (Compose's `NavigationSuiteItemColors`): the bar's, the rail's and
/// the drawer's item colors. A class without `equals` in Kotlin: no `==` here.
#[derive(Clone, kompoz_runtime::KotlinProperties)]
pub struct NavigationSuiteItemColors {
    /// The colors of the [`NavigationBarItem`] of the item.
    pub navigation_bar_item_colors: NavigationBarItemColors,
    /// The colors of the [`NavigationRailItem`] of the item.
    pub navigation_rail_item_colors: NavigationRailItemColors,
    /// The colors of the [`NavigationDrawerItem`] of the item.
    pub navigation_drawer_item_colors: Rc<dyn NavigationDrawerItemColors>,
}

impl NavigationSuiteItemColors {
    /// Kotlin: `NavigationSuiteItemColors(navigationBarItemColors: NavigationBarItemColors, navigationRailItemColors: NavigationRailItemColors, navigationDrawerItemColors: NavigationDrawerItemColors)`
    pub fn new(
        navigation_bar_item_colors: NavigationBarItemColors,
        navigation_rail_item_colors: NavigationRailItemColors,
        navigation_drawer_item_colors: Rc<dyn NavigationDrawerItemColors>,
    ) -> NavigationSuiteItemColors {
        NavigationSuiteItemColors {
            navigation_bar_item_colors,
            navigation_rail_item_colors,
            navigation_drawer_item_colors,
        }
    }
}

impl fmt::Debug for NavigationSuiteItemColors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NavigationSuiteItemColors")
            .field("navigation_bar_item_colors", &self.navigation_bar_item_colors)
            .field("navigation_rail_item_colors", &self.navigation_rail_item_colors)
            .finish_non_exhaustive()
    }
}

/// The window's adaptive info the defaults go by (Compose's internal `WindowAdaptiveInfoDefault`:
/// `currentWindowAdaptiveInfo()`, without the large widths — "move to V2 when adaptive dependency is updated").
#[track_caller]
#[allow(non_snake_case)]
pub(crate) fn WindowAdaptiveInfoDefault() -> WindowAdaptiveInfo {
    kompozable_block!(current_window_adaptive_info())
}

// ---- Internals ---------------------------------------------------------------------------------------------------

/// Consumes the insets of the navigation component of `navigation_suite_type` on its side while it shows or moves
/// (Compose's private composable `Modifier.navigationSuiteScaffoldConsumeWindowInsets`).
#[track_caller]
fn navigation_suite_scaffold_consume_window_insets(
    modifier: Modifier,
    navigation_suite_type: NavigationSuiteType,
    state: &dyn NavigationSuiteScaffoldState,
) -> Modifier {
    let insets = if state.current_value() == NavigationSuiteScaffoldValue::Hidden && !state.is_animating() {
        NoWindowInsets()
    } else {
        match navigation_suite_type {
            NavigationSuiteType::ShortNavigationBarCompact | NavigationSuiteType::ShortNavigationBarMedium => {
                ShortNavigationBarDefaults::window_insets().only(WindowInsetsSides::Bottom)
            }
            NavigationSuiteType::WideNavigationRailCollapsed | NavigationSuiteType::WideNavigationRailExpanded => {
                WideNavigationRailDefaults::window_insets().only(WindowInsetsSides::Start)
            }
            NavigationSuiteType::NavigationBar => {
                NavigationBarDefaults::window_insets().only(WindowInsetsSides::Bottom)
            }
            NavigationSuiteType::NavigationRail => {
                NavigationRailDefaults::window_insets().only(WindowInsetsSides::Start)
            }
            NavigationSuiteType::NavigationDrawer => DrawerDefaults::window_insets().only(WindowInsetsSides::Start),
            _ => NoWindowInsets(),
        }
    };
    modifier.consume_window_insets(insets)
}

/// `remember(content) { movableContentOf(content) }` (Compose's NavigationSuite): kept while `content`'s identity as
/// a memoized closure stays the same (Compose's lambda equality), so the items keep their state as the navigation
/// suite's type changes.
#[kompozable(inline)]
fn remember_movable_content(#[memoized] content: impl Fn() + 'static) -> MovableContent {
    content.remember(movable_content_of)
}

/// A Kotlin lambda as the override scopes keep it: `f` in an `Rc` kept while its identity as a memoized closure
/// stays the same (Compose's lambdas are the same instance then), so what the scope's lambdas are given to (the
/// navigation suite's movable content and items) sees the same lambda while it is.
#[kompozable(inline)]
fn remember_lambda(#[memoized] f: impl Fn() + 'static) -> Rc<dyn Fn()> {
    f.remember(|f| Rc::new(f) as Rc<dyn Fn()>)
}

/// [`remember_lambda`] of a scope's items.
#[kompozable(inline)]
fn remember_scope_lambda(
    #[memoized] f: impl Fn(&mut NavigationSuiteScope) + 'static,
) -> Rc<dyn Fn(&mut NavigationSuiteScope)> {
    f.remember(|f| Rc::new(f) as Rc<dyn Fn(&mut NavigationSuiteScope)>)
}

/// Compose's private `SpringDefaultSpatialDamping`.
const SpringDefaultSpatialDamping: f32 = 0.9;
/// Compose's private `SpringDefaultSpatialStiffness`.
const SpringDefaultSpatialStiffness: f32 = 700.0;
const NavigationSuiteLayoutIdTag: &str = "navigationSuite";
const PrimaryActionContentLayoutIdTag: &str = "primaryActionContent";
const ContentLayoutIdTag: &str = "content";

/// The least height of a [`NavigationSuiteType::NavigationBar`]'s short bar (Compose's private
/// `TallNavigationBarHeight`).
const TallNavigationBarHeight: Dp = Dp(80.0);
/// The primary action's distance from a bar and the side (Compose's private `PrimaryActionContentPadding`).
const PrimaryActionContentPadding: Dp = Dp(16.0);

/// No insets (Compose's private `NoWindowInsets`, `WindowInsets(0, 0, 0, 0)`).
fn NoWindowInsets() -> WindowInsets {
    WindowInsets::new(0, 0, 0, 0)
}

/// How the navigation component moves as it is hidden and shown (Compose's private `AnimationSpec`): a spring of
/// damping 0.9 and stiffness 700.
fn AnimationSpec() -> Arc<dyn AnimationSpec<f32>> {
    let spec: SpringSpec<f32> =
        named_args!(spring(damping_ratio = SpringDefaultSpatialDamping, stiffness = SpringDefaultSpatialStiffness));
    Arc::new(spec)
}

// ---- The override API --------------------------------------------------------------------------------------------

/// The navigation suite scaffold of an override (Compose's experimental `NavigationSuiteScaffoldOverride`): what
/// makes every [`NavigationSuiteScaffoldWithLayoutType`] below a [`LocalNavigationSuiteScaffoldOverride`] providing
/// it, from the scope's arguments.
pub trait NavigationSuiteScaffoldOverride {
    /// Composes the scaffold of `scope`'s arguments (Compose's `NavigationSuiteScaffoldOverrideScope.NavigationSuiteScaffold()`).
    fn NavigationSuiteScaffold(&self, scope: &NavigationSuiteScaffoldOverrideScope);
}

impl PartialEq for dyn NavigationSuiteScaffoldOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The arguments of a [`NavigationSuiteScaffoldWithLayoutType`], given to its override (Compose's experimental
/// `NavigationSuiteScaffoldOverrideScope`; its constructor is internal).
#[derive(Clone)]
pub struct NavigationSuiteScaffoldOverrideScope {
    navigation_suite_items: Rc<dyn Fn(&mut NavigationSuiteScope)>,
    modifier: Modifier,
    layout_type: NavigationSuiteType,
    navigation_suite_colors: NavigationSuiteColors,
    container_color: Color,
    content_color: Color,
    state: Rc<dyn NavigationSuiteScaffoldState>,
    content: Rc<dyn Fn()>,
}

impl NavigationSuiteScaffoldOverrideScope {
    /// The navigation items to be displayed (Compose's `navigationSuiteItems`).
    pub fn navigation_suite_items(&self) -> Rc<dyn Fn(&mut NavigationSuiteScope)> {
        self.navigation_suite_items.clone()
    }

    /// The scaffold's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The current navigation suite type (Compose's `layoutType`).
    pub fn layout_type(&self) -> NavigationSuiteType {
        self.layout_type
    }

    /// The navigation component's colors (Compose's `navigationSuiteColors`).
    pub fn navigation_suite_colors(&self) -> NavigationSuiteColors {
        self.navigation_suite_colors
    }

    /// The scaffold's background (Compose's `containerColor`).
    pub fn container_color(&self) -> Color {
        self.container_color
    }

    /// The color of the scaffold's content (Compose's `contentColor`).
    pub fn content_color(&self) -> Color {
        self.content_color
    }

    /// The scaffold's state (Compose's `state`).
    pub fn state(&self) -> Rc<dyn NavigationSuiteScaffoldState> {
        self.state.clone()
    }

    /// The screen's content (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the navigation suite scaffolds of scope items below (Compose's experimental
/// `LocalNavigationSuiteScaffoldOverride`): [`DefaultNavigationSuiteScaffoldOverride`] unless provided.
pub static LocalNavigationSuiteScaffoldOverride: KompozitionLocal<Rc<dyn NavigationSuiteScaffoldOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultNavigationSuiteScaffoldOverride));

/// The navigation suite scaffold with a primary action of an override (Compose's experimental
/// `NavigationSuiteScaffoldWithPrimaryActionOverride`): what makes every [`NavigationSuiteScaffold`] below a
/// [`LocalNavigationSuiteScaffoldWithPrimaryActionOverride`] providing it, from the scope's arguments.
pub trait NavigationSuiteScaffoldWithPrimaryActionOverride {
    /// Composes the scaffold of `scope`'s arguments (Compose's
    /// `NavigationSuiteScaffoldWithPrimaryActionOverrideScope.NavigationSuiteScaffoldWithPrimaryAction()`).
    fn NavigationSuiteScaffoldWithPrimaryAction(&self, scope: &NavigationSuiteScaffoldWithPrimaryActionOverrideScope);
}

impl PartialEq for dyn NavigationSuiteScaffoldWithPrimaryActionOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The arguments of a [`NavigationSuiteScaffold`], given to its override (Compose's experimental
/// `NavigationSuiteScaffoldWithPrimaryActionOverrideScope`; its constructor is internal).
#[derive(Clone)]
pub struct NavigationSuiteScaffoldWithPrimaryActionOverrideScope {
    navigation_items: Rc<dyn Fn()>,
    modifier: Modifier,
    navigation_suite_type: NavigationSuiteType,
    navigation_suite_colors: NavigationSuiteColors,
    container_color: Color,
    content_color: Color,
    state: Rc<dyn NavigationSuiteScaffoldState>,
    navigation_item_vertical_arrangement: VerticalArrangement,
    primary_action_content: Rc<dyn Fn()>,
    primary_action_content_horizontal_alignment: HorizontalAlignment,
    content: Rc<dyn Fn()>,
}

impl NavigationSuiteScaffoldWithPrimaryActionOverrideScope {
    /// The navigation items to be displayed, typically [`NavigationSuiteItem`]s (Compose's `navigationItems`).
    pub fn navigation_items(&self) -> Rc<dyn Fn()> {
        self.navigation_items.clone()
    }

    /// The scaffold's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The current navigation suite type (Compose's `navigationSuiteType`).
    pub fn navigation_suite_type(&self) -> NavigationSuiteType {
        self.navigation_suite_type
    }

    /// The navigation component's colors (Compose's `navigationSuiteColors`).
    pub fn navigation_suite_colors(&self) -> NavigationSuiteColors {
        self.navigation_suite_colors
    }

    /// The scaffold's background (Compose's `containerColor`).
    pub fn container_color(&self) -> Color {
        self.container_color
    }

    /// The color of the scaffold's content (Compose's `contentColor`).
    pub fn content_color(&self) -> Color {
        self.content_color
    }

    /// The scaffold's state (Compose's `state`).
    pub fn state(&self) -> Rc<dyn NavigationSuiteScaffoldState> {
        self.state.clone()
    }

    /// The vertical arrangement of the items in a vertical navigation component (Compose's
    /// `navigationItemVerticalArrangement`).
    pub fn navigation_item_vertical_arrangement(&self) -> VerticalArrangement {
        self.navigation_item_vertical_arrangement.clone()
    }

    /// The primary action's content (Compose's `primaryActionContent`).
    pub fn primary_action_content(&self) -> Rc<dyn Fn()> {
        self.primary_action_content.clone()
    }

    /// Where the primary action goes above a horizontal navigation component (Compose's
    /// `primaryActionContentHorizontalAlignment`).
    pub fn primary_action_content_horizontal_alignment(&self) -> HorizontalAlignment {
        self.primary_action_content_horizontal_alignment.clone()
    }

    /// The screen's content (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the navigation suite scaffolds with a primary action below (Compose's experimental
/// `LocalNavigationSuiteScaffoldWithPrimaryActionOverride`): [`DefaultNavigationSuiteScaffoldWithPrimaryActionOverride`]
/// unless provided.
pub static LocalNavigationSuiteScaffoldWithPrimaryActionOverride: KompozitionLocal<
    Rc<dyn NavigationSuiteScaffoldWithPrimaryActionOverride>,
> = KompozitionLocal::new(|| Rc::new(DefaultNavigationSuiteScaffoldWithPrimaryActionOverride));
