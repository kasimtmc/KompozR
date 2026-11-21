//! Material's tab rows, as Compose's TabRow.kt: a row of [`Tab`](crate::Tab)s with an indicator under the selected
//! one and a divider under them all — fixed rows sharing their width equally ([`PrimaryTabRow`]: a short rounded
//! indicator as wide as the tab's content; [`SecondaryTabRow`]: a line under the whole tab), and scrolling rows of
//! tabs as wide as their content ([`PrimaryScrollableTabRow`], [`SecondaryScrollableTabRow`]) that bring the selected
//! tab into the middle. The indicator is drawn in a [`TabIndicatorScope`]: `Modifier.tab_indicator_offset` slides it
//! under the selected tab (the default spatial motion), `tab_indicator_layout` lays it out by the tabs' positions
//! ([`TabPosition`]). The deprecated [`TabRow`] and [`ScrollableTabRow`] give their indicator the positions instead.

// (the deprecated TabRow, ScrollableTabRow and TabRowDefaults' deprecated members are defined and used here)
#![allow(deprecated)]

use crate::divider::HorizontalDivider;
use crate::surface::Surface;
use crate::tab::HorizontalTextPadding;
use crate::tokens::{MotionSchemeKeyTokens, PrimaryNavigationTabTokens, SecondaryNavigationTabTokens};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, FiniteAnimationSpec, VectorConverter, animate_dp_as_state,
};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_foundation::scroll::ScrollState;
use kompoz_graphics::{Color, Density, Dp, IntOffset, Shape};
use kompoz_runtime::{MutableState, kompozable, mutable_state_of, named, named_args, remember, remember_keyed};
use kompoz_ui::layout::{
    LayoutModifierNode, MultiContentLayout, MultiContentMeasurePolicy, Placeable, SubcomposeLayout,
};
use kompoz_ui::{
    Alignment, Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement,
    NodeCx, NodeHandle, node_capabilities,
};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- The tab rows ----------------------------------------------------------------------------------------------

/// A primary tab row (Compose's `PrimaryTabRow`): `tabs` sharing the row's width equally, as tall as the tallest,
/// on the surface in the primary color (`container_color`, `content_color`), `divider` (a [`HorizontalDivider`]) at
/// the bottom and `indicator` over it. `indicator` None is Kotlin's default: a [`TabRowDefaults::PrimaryIndicator`]
/// as wide as the selected tab's content (`tab_indicator_offset(selected_tab_index, match_content_size = true)`). The
/// tabs are one selectable group.
///
/// Kotlin: `PrimaryTabRow(selectedTabIndex: Int, modifier: Modifier = Modifier, containerColor: Color = TabRowDefaults.primaryContainerColor, contentColor: Color = TabRowDefaults.primaryContentColor, indicator: @Composable TabIndicatorScope.() -> Unit, divider: @Composable () -> Unit = @Composable { HorizontalDivider() }, tabs: @Composable () -> Unit)`
#[kompozable]
pub fn PrimaryTabRow(
    selected_tab_index: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(TabRowDefaults.primary_container_color())] container_color: Color,
    #[default(TabRowDefaults.primary_content_color())] content_color: Color,
    #[default(None)] indicator: Option<Rc<dyn Fn(&TabIndicatorScope)>>,
    #[default(DefaultDivider)] divider: impl Fn() + 'static,
    tabs: impl Fn() + 'static,
) {
    let indicator = indicator.unwrap_or_else(|| {
        Rc::new(move |scope: &TabIndicatorScope| {
            TabRowDefaults.PrimaryIndicator(
                modifier = scope.tab_indicator_offset(Modifier, selected_tab_index, match_content_size = true),
                width = Dp::Unspecified,
            );
        })
    });
    TabRowImpl(modifier, container_color, content_color, indicator, Rc::new(divider), Rc::new(tabs));
}

/// A secondary tab row (Compose's `SecondaryTabRow`): as [`PrimaryTabRow`] in the secondary colors, its default
/// indicator (None) a [`TabRowDefaults::SecondaryIndicator`] under the whole selected tab
/// (`tab_indicator_offset(selected_tab_index, match_content_size = false)`).
///
/// Kotlin: `SecondaryTabRow(selectedTabIndex: Int, modifier: Modifier = Modifier, containerColor: Color = TabRowDefaults.secondaryContainerColor, contentColor: Color = TabRowDefaults.secondaryContentColor, indicator: @Composable TabIndicatorScope.() -> Unit, divider: @Composable () -> Unit = @Composable { HorizontalDivider() }, tabs: @Composable () -> Unit)`
#[kompozable]
pub fn SecondaryTabRow(
    selected_tab_index: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(TabRowDefaults.secondary_container_color())] container_color: Color,
    #[default(TabRowDefaults.secondary_content_color())] content_color: Color,
    #[default(None)] indicator: Option<Rc<dyn Fn(&TabIndicatorScope)>>,
    #[default(DefaultDivider)] divider: impl Fn() + 'static,
    tabs: impl Fn() + 'static,
) {
    let indicator = indicator.unwrap_or_else(|| {
        Rc::new(move |scope: &TabIndicatorScope| {
            TabRowDefaults.SecondaryIndicator(
                modifier = scope.tab_indicator_offset(Modifier, selected_tab_index, match_content_size = false),
            );
        })
    });
    TabRowImpl(modifier, container_color, content_color, indicator, Rc::new(divider), Rc::new(tabs));
}

/// A primary scrolling tab row (Compose's `PrimaryScrollableTabRow`): `tabs` each as wide as its content (at least
/// `min_tab_width`), `edge_padding` before the first and after the last, scrolled by `scroll_state` (one of its own
/// by default) — by hand, and to bring a newly selected tab into the middle (on the default spatial motion). In the
/// primary colors, `divider` under them all, `indicator` (None: Kotlin's default, a
/// [`TabRowDefaults::PrimaryIndicator`] as wide as the selected tab's content) over it.
///
/// Kotlin: `PrimaryScrollableTabRow(selectedTabIndex: Int, modifier: Modifier = Modifier, scrollState: ScrollState = rememberScrollState(), containerColor: Color = TabRowDefaults.primaryContainerColor, contentColor: Color = TabRowDefaults.primaryContentColor, edgePadding: Dp = TabRowDefaults.ScrollableTabRowEdgeStartPadding, indicator: @Composable TabIndicatorScope.() -> Unit, divider: @Composable () -> Unit = @Composable { HorizontalDivider() }, minTabWidth: Dp = TabRowDefaults.ScrollableTabRowMinTabWidth, tabs: @Composable () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn PrimaryScrollableTabRow(
    selected_tab_index: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(remember_scroll_state())] scroll_state: ScrollState,
    #[default(TabRowDefaults.primary_container_color())] container_color: Color,
    #[default(TabRowDefaults.primary_content_color())] content_color: Color,
    #[default(TabRowDefaults::ScrollableTabRowEdgeStartPadding)] edge_padding: Dp,
    #[default(None)] indicator: Option<Rc<dyn Fn(&TabIndicatorScope)>>,
    #[default(DefaultDivider)] divider: impl Fn() + 'static,
    #[default(TabRowDefaults::ScrollableTabRowMinTabWidth)] min_tab_width: Dp,
    tabs: impl Fn() + 'static,
) {
    let indicator = indicator.unwrap_or_else(|| {
        Rc::new(move |scope: &TabIndicatorScope| {
            TabRowDefaults.PrimaryIndicator(
                modifier = scope.tab_indicator_offset(Modifier, selected_tab_index, match_content_size = true),
                width = Dp::Unspecified,
            );
        })
    });
    ScrollableTabRowImpl(
        selected_tab_index,
        modifier,
        container_color,
        content_color,
        edge_padding,
        min_tab_width,
        scroll_state,
        indicator,
        Rc::new(divider),
        Rc::new(tabs),
    );
}

/// A secondary scrolling tab row (Compose's `SecondaryScrollableTabRow`): as [`PrimaryScrollableTabRow`] in the
/// secondary colors, its default indicator (None) a [`TabRowDefaults::SecondaryIndicator`] under the whole selected
/// tab.
///
/// Kotlin: `SecondaryScrollableTabRow(selectedTabIndex: Int, modifier: Modifier = Modifier, scrollState: ScrollState = rememberScrollState(), containerColor: Color = TabRowDefaults.secondaryContainerColor, contentColor: Color = TabRowDefaults.secondaryContentColor, edgePadding: Dp = TabRowDefaults.ScrollableTabRowEdgeStartPadding, indicator: @Composable TabIndicatorScope.() -> Unit, divider: @Composable () -> Unit = @Composable { HorizontalDivider() }, minTabWidth: Dp = TabRowDefaults.ScrollableTabRowMinTabWidth, tabs: @Composable () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SecondaryScrollableTabRow(
    selected_tab_index: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(remember_scroll_state())] scroll_state: ScrollState,
    #[default(TabRowDefaults.secondary_container_color())] container_color: Color,
    #[default(TabRowDefaults.secondary_content_color())] content_color: Color,
    #[default(TabRowDefaults::ScrollableTabRowEdgeStartPadding)] edge_padding: Dp,
    #[default(None)] indicator: Option<Rc<dyn Fn(&TabIndicatorScope)>>,
    #[default(DefaultDivider)] divider: impl Fn() + 'static,
    #[default(TabRowDefaults::ScrollableTabRowMinTabWidth)] min_tab_width: Dp,
    tabs: impl Fn() + 'static,
) {
    let indicator = indicator.unwrap_or_else(|| {
        Rc::new(move |scope: &TabIndicatorScope| {
            TabRowDefaults.SecondaryIndicator(
                modifier = scope.tab_indicator_offset(Modifier, selected_tab_index, match_content_size = false),
            );
        })
    });
    ScrollableTabRowImpl(
        selected_tab_index,
        modifier,
        container_color,
        content_color,
        edge_padding,
        min_tab_width,
        scroll_state,
        indicator,
        Rc::new(divider),
        Rc::new(tabs),
    );
}

/// A tab row's default divider (Kotlin's `@Composable { HorizontalDivider() }`): a function, as a default of a
/// content parameter is.
fn DefaultDivider() {
    kompoz_runtime::kompozable_block!({
        HorizontalDivider();
    })
}

// ---- TabIndicatorScope -----------------------------------------------------------------------------------------

/// What a tab row's indicator is composed in (Compose's `TabIndicatorScope`): modifiers laying the indicator out by
/// the positions of the row's tabs. (Kotlin's interface, implemented by the tab rows' own object (Compose's
/// `remember { object : TabIndicatorScope, TabPositionsHolder }`): a value of the rows here, as the animation crate's
/// scopes are; its member extensions are its methods taking the modifier first.)
#[derive(Clone)]
pub struct TabIndicatorScope {
    tab_positions: MutableState<Vec<TabPosition>>,
    /// The indicator's motion, as the row first composed it (Compose's object captures it then).
    animation_spec: Arc<dyn FiniteAnimationSpec<Dp>>,
}

impl PartialEq for TabIndicatorScope {
    fn eq(&self, other: &Self) -> bool {
        self.tab_positions == other.tab_positions
    }
}

impl TabIndicatorScope {
    fn new(animation_spec: Arc<dyn FiniteAnimationSpec<Dp>>) -> TabIndicatorScope {
        TabIndicatorScope { tab_positions: mutable_state_of(Vec::new()), animation_spec }
    }

    /// The tabs' positions as the row last measured them (Compose's internal `TabPositionsHolder.setTabPositions`).
    fn set_tab_positions(&self, positions: Vec<TabPosition>) {
        self.tab_positions.set(positions);
    }
}

#[named]
impl TabIndicatorScope {
    /// `modifier` laying the indicator out by `measure` (Compose's `Modifier.tabIndicatorLayout` of the scope): given
    /// the indicator, its constraints and the tabs' positions as the row last measured them (re-measured as they
    /// change).
    ///
    /// Kotlin: `tabIndicatorLayout(measure: MeasureScope.(Measurable, Constraints, List<TabPosition>) -> MeasureResult)`
    pub fn tab_indicator_layout(
        &self,
        modifier: Modifier,
        measure: impl Fn(&MeasureScope, &dyn Measurable, Constraints, Vec<TabPosition>) -> MeasureResult + 'static,
    ) -> Modifier {
        let tab_positions = self.tab_positions.clone();
        modifier.layout(move |scope, measurable, constraints| {
            measure(scope, measurable, constraints, tab_positions.value())
        })
    }

    /// `modifier` placing the indicator under the tab `selected_tab_index` (Compose's `Modifier.tabIndicatorOffset`
    /// of the scope): as wide as that tab's content where `match_content_size` (at least 24 dp), else as the whole
    /// tab; it slides and resizes to a newly selected tab on the default spatial motion.
    ///
    /// Kotlin: `tabIndicatorOffset(selectedTabIndex: Int, matchContentSize: Boolean = false)`
    pub fn tab_indicator_offset(
        &self,
        modifier: Modifier,
        selected_tab_index: i32,
        #[default(false)] match_content_size: bool,
    ) -> Modifier {
        modifier.then_element(TabIndicatorModifier {
            tab_positions_state: self.tab_positions.clone(),
            selected_tab_index,
            follow_content_size: match_content_size,
            animation_spec: self.animation_spec.clone(),
        })
    }
}

impl fmt::Debug for TabIndicatorScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TabIndicatorScope").field("tab_positions", &self.tab_positions.peek()).finish()
    }
}

// ---- The fixed row ---------------------------------------------------------------------------------------------

/// The fixed tab rows' layout (Compose's private `TabRowImpl`): on a surface, the tabs an equal share of the width
/// each (as tall as the tallest is at that width), the divider at the bottom, the indicator over it, laid out with
/// the tabs' positions told to the indicator's scope.
#[kompozable]
fn TabRowImpl(
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    indicator: Rc<dyn Fn(&TabIndicatorScope)>,
    divider: Rc<dyn Fn()>,
    tabs: Rc<dyn Fn()>,
) {
    Surface(
        modifier = modifier.selectable_group(),
        color = container_color,
        content_color = Some(content_color),
        content = move || {
            // TODO Load the motionScheme tokens from the component tokens file
            let tab_indicator_animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<Dp>();
            let scope = remember(move || TabIndicatorScope::new(tab_indicator_animation_spec));

            let s = scope.clone();
            let policy = remember_keyed(scope.clone(), move || {
                let scope = s.clone();
                Rc::new(move |m: &MeasureScope, measurables: &[Vec<&dyn Measurable>], constraints: Constraints| {
                    let (tab_measurables, divider_measurables, indicator_measurables) =
                        (&measurables[0], &measurables[1], &measurables[2]);
                    let tab_row_width = constraints.max_width;
                    let tab_count = tab_measurables.len() as i32;
                    let mut tab_width = 0;
                    if tab_count > 0 {
                        tab_width = tab_row_width / tab_count;
                    }
                    let tab_row_height =
                        tab_measurables.iter().fold(0, |max, curr| curr.max_intrinsic_height(tab_width).max(max));

                    scope.set_tab_positions(
                        tab_measurables
                            .iter()
                            .enumerate()
                            .map(|(index, measurable)| {
                                let mut content_width =
                                    m.to_dp(measurable.max_intrinsic_width(tab_row_height).min(tab_width) as f32);
                                content_width = content_width - HorizontalTextPadding * 2.0;
                                // Enforce minimum touch target of 24.dp
                                let indicator_width = max_dp(content_width, Dp(24.0));

                                TabPosition::new(
                                    m.to_dp(tab_width as f32) * index as f32,
                                    m.to_dp(tab_width as f32),
                                    indicator_width,
                                )
                            })
                            .collect(),
                    );

                    let tab_placeables: Vec<Placeable> = tab_measurables
                        .iter()
                        .map(|it| {
                            it.measure(named_args!(constraints.copy(
                                min_width = tab_width,
                                max_width = tab_width,
                                min_height = tab_row_height,
                                max_height = tab_row_height,
                            )))
                        })
                        .collect();

                    let divider_placeables: Vec<Placeable> = divider_measurables
                        .iter()
                        .map(|it| it.measure(named_args!(constraints.copy(min_height = 0))))
                        .collect();

                    let indicator_placeables: Vec<Placeable> = indicator_measurables
                        .iter()
                        .map(|it| {
                            it.measure(named_args!(constraints.copy(
                                min_width = tab_width,
                                max_width = tab_width,
                                min_height = 0,
                                max_height = tab_row_height,
                            )))
                        })
                        .collect();

                    m.layout(tab_row_width, tab_row_height, move |p| {
                        for (index, placeable) in tab_placeables.iter().enumerate() {
                            p.place_relative(placeable, index as i32 * tab_width, 0);
                        }

                        for placeable in &divider_placeables {
                            p.place_relative(placeable, 0, tab_row_height - placeable.height());
                        }

                        for it in &indicator_placeables {
                            p.place_relative(it, 0, tab_row_height - it.height());
                        }
                    })
                }) as Rc<dyn MultiContentMeasurePolicy>
            });

            let (tabs, divider, indicator) = (tabs.clone(), divider.clone(), indicator.clone());
            let indicator_content: Rc<dyn Fn()> = Rc::new(move || indicator(&scope));
            MultiContentLayout(
                contents = vec![tabs, divider, indicator_content],
                modifier = Modifier.fill_max_width(),
                measure_policy = policy,
            );
        },
    );
}

// ---- The scrolling row -----------------------------------------------------------------------------------------

/// What a scrolling tab row's measure block is made of, compared as Compose's memoized lambda is (by what it
/// captures).
#[derive(Clone, PartialEq)]
struct ScrollableTabRowKey {
    selected_tab_index: i32,
    edge_padding: Dp,
    min_tab_width: Dp,
    scrollable_tab_data: Rc<ScrollableTabData>,
    scope: TabIndicatorScope,
}

/// The scrolling tab rows' layout (Compose's private `ScrollableTabRowImpl`): on a surface, the divider under a
/// horizontally scrolled, clipped row of the tabs (each at least `min_tab_width`, `edge_padding` at either end) with
/// the indicator under the selected one; laid out, it scrolls a newly selected tab into the middle.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ScrollableTabRowImpl(
    selected_tab_index: i32,
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    edge_padding: Dp,
    min_tab_width: Dp,
    scroll_state: ScrollState,
    indicator: Rc<dyn Fn(&TabIndicatorScope)>,
    divider: Rc<dyn Fn()>,
    tabs: Rc<dyn Fn()>,
) {
    Surface(
        modifier = modifier,
        color = container_color,
        content_color = Some(content_color),
        content = move || {
            let coroutine_scope = remember_coroutine_scope();
            // TODO Load the motionScheme tokens from the component tokens file
            let scroll_animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
            let tab_indicator_animation_spec: Arc<dyn FiniteAnimationSpec<Dp>> =
                MotionSchemeKeyTokens::DefaultSpatial.value::<Dp>();
            let (s, c) = (scroll_state.clone(), coroutine_scope.clone());
            let scrollable_tab_data = remember_keyed((scroll_state.clone(), coroutine_scope), move || {
                Rc::new(ScrollableTabData::new(s, c, scroll_animation_spec))
            });

            let scope = remember(move || TabIndicatorScope::new(tab_indicator_animation_spec));
            let key = ScrollableTabRowKey {
                selected_tab_index,
                edge_padding,
                min_tab_width,
                scrollable_tab_data,
                scope: scope.clone(),
            };
            let policy = remember_keyed(key.clone(), move || scrollable_tab_row_policy(key));
            let (scroll_state, divider, tabs, indicator) =
                (scroll_state.clone(), divider.clone(), tabs.clone(), indicator.clone());
            Box(
                content_alignment = Alignment::BottomStart,
                content = move || {
                    divider();
                    let indicator = indicator.clone();
                    let scope = scope.clone();
                    let indicator_content: Rc<dyn Fn()> = Rc::new(move || indicator(&scope));
                    MultiContentLayout(
                        contents = vec![tabs.clone(), indicator_content],
                        modifier = Modifier
                            .fill_max_width()
                            .wrap_content_size_with(Alignment::CenterStart, false)
                            .horizontal_scroll(&scroll_state)
                            .selectable_group()
                            .clip_to_bounds(),
                        measure_policy = policy.clone(),
                    );
                },
            );
        },
    );
}

/// The scrolling tab row's measure block (the lambda of Compose's `ScrollableTabRowImpl`'s `Layout`).
fn scrollable_tab_row_policy(key: ScrollableTabRowKey) -> Rc<dyn MultiContentMeasurePolicy> {
    Rc::new(move |m: &MeasureScope, measurables: &[Vec<&dyn Measurable>], constraints: Constraints| {
        let ScrollableTabRowKey { selected_tab_index, edge_padding, min_tab_width, scrollable_tab_data, scope } =
            key.clone();
        let (tab_measurables, indicator_measurables) = (&measurables[0], &measurables[1]);
        let padding = m.round_to_px(edge_padding);
        let layout_height = tab_measurables
            .iter()
            .fold(0, |curr, measurable| curr.max(measurable.max_intrinsic_height(Constraints::Infinity)));
        let mut layout_width = padding * 2;
        let tab_constraints = named_args!(constraints.copy(
            min_width = m.round_to_px(min_tab_width),
            min_height = layout_height,
            max_height = layout_height,
        ));

        let mut left = edge_padding;
        let tab_placeables: Vec<Placeable> = tab_measurables.iter().map(|it| it.measure(tab_constraints)).collect();
        // Get indicator widths based on incoming content size, not based on forced minimum
        // width applied below.
        let indicator_width: Vec<i32> =
            tab_measurables.iter().map(|it| it.max_intrinsic_width(Constraints::Infinity)).collect();

        let positions: Vec<TabPosition> = tab_placeables
            .iter()
            .enumerate()
            .map(|(index, placeable)| {
                let tab_width = max_dp(min_tab_width, m.to_dp(placeable.width() as f32));
                layout_width += m.round_to_px(tab_width);
                // Enforce minimum touch target of 24.dp
                let content_width =
                    max_dp(m.to_dp(indicator_width[index] as f32) - (HorizontalTextPadding * 2.0), Dp(24.0));
                let tab_position = TabPosition::new(left, tab_width, content_width);
                left = left + tab_width;
                tab_position
            })
            .collect();
        scope.set_tab_positions(positions.clone());

        let selected = &positions[selected_tab_index as usize];
        let indicator_placeables: Vec<Placeable> = indicator_measurables
            .iter()
            .map(|it| {
                it.measure(named_args!(constraints.copy(
                    min_width = 0,
                    max_width = m.round_to_px(selected.content_width),
                    min_height = 0,
                    max_height = layout_height,
                )))
            })
            .collect();

        let density: Density = **m;
        m.layout(layout_width, layout_height, move |p| {
            let mut left = edge_padding;
            for (index, placeable) in tab_placeables.iter().enumerate() {
                p.place_relative(placeable, density.round_to_px(left), 0);
                left = left + positions[index].width;
            }

            for it in &indicator_placeables {
                let relative_offset =
                    0.max((density.round_to_px(positions[selected_tab_index as usize].width) - it.width()) / 2);
                p.place_relative(it, relative_offset, layout_height - it.height());
            }

            scrollable_tab_data.on_laid_out(&density, padding, &positions, selected_tab_index);
        })
    })
}

// ---- The indicator's offset ------------------------------------------------------------------------------------

/// `Modifier.tab_indicator_offset`'s element (Compose's internal `TabIndicatorModifier`).
struct TabIndicatorModifier {
    tab_positions_state: MutableState<Vec<TabPosition>>,
    selected_tab_index: i32,
    follow_content_size: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<Dp>>,
}

impl PartialEq for TabIndicatorModifier {
    fn eq(&self, other: &Self) -> bool {
        // (Compose's data class: the spec by its equality, here by its instance)
        self.tab_positions_state == other.tab_positions_state
            && self.selected_tab_index == other.selected_tab_index
            && self.follow_content_size == other.follow_content_size
            && Arc::ptr_eq(&self.animation_spec, &other.animation_spec)
    }
}

impl ModifierNodeElement for TabIndicatorModifier {
    type Node = TabIndicatorOffsetNode;
    fn create(&self) -> TabIndicatorOffsetNode {
        TabIndicatorOffsetNode {
            tab_positions_state: self.tab_positions_state.clone(),
            selected_tab_index: self.selected_tab_index,
            follow_content_size: self.follow_content_size,
            animation_spec: self.animation_spec.clone(),
            offset_animatable: None,
            width_animatable: None,
            initial_offset: None,
            initial_width: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut TabIndicatorOffsetNode) {
        node.tab_positions_state = self.tab_positions_state.clone();
        node.selected_tab_index = self.selected_tab_index;
        node.follow_content_size = self.follow_content_size;
        node.animation_spec = self.animation_spec.clone();
    }
    fn name(&self) -> &'static str {
        // Show nothing in the inspector.
        "tabIndicatorOffset"
    }
}

/// The indicator's offset and width (Compose's internal `TabIndicatorOffsetNode`): those of the selected tab (its
/// content's width where it follows it), animated to from the first ones it measured with.
struct TabIndicatorOffsetNode {
    tab_positions_state: MutableState<Vec<TabPosition>>,
    selected_tab_index: i32,
    follow_content_size: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<Dp>>,
    offset_animatable: Option<Animatable<Dp, AnimationVector1D>>,
    width_animatable: Option<Animatable<Dp, AnimationVector1D>>,
    initial_offset: Option<Dp>,
    initial_width: Option<Dp>,
    handle: Option<NodeHandle>,
}

impl TabIndicatorOffsetNode {
    /// Animates `animatable` to `target` in the node's coroutine scope (Compose's `coroutineScope.launch {
    /// animateTo(…) }`).
    fn launch_animation(&self, animatable: &Animatable<Dp, AnimationVector1D>, target: Dp) {
        let Some(scope) = self.handle.as_ref().and_then(|h| h.coroutine_scope()) else { return };
        let (animatable, spec) = (animatable.clone(), self.animation_spec.clone() as Arc<dyn AnimationSpec<Dp>>);
        scope.launch(async move {
            named_args!(animatable.animate_to(target, animation_spec = spec)).await;
        });
    }
}

impl ModifierNode for TabIndicatorOffsetNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
}

impl LayoutModifierNode for TabIndicatorOffsetNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let tab_positions = self.tab_positions_state.value();
        if tab_positions.is_empty() {
            return scope.layout(0, 0, |_| {});
        }
        let selected = tab_positions[self.selected_tab_index as usize];

        let current_tab_width = if self.follow_content_size { selected.content_width } else { selected.width };

        if let Some(initial_width) = self.initial_width {
            let width_anim =
                self.width_animatable.get_or_insert_with(|| Animatable((initial_width, Dp::VectorConverter))).clone();

            if current_tab_width != width_anim.target_value() {
                self.launch_animation(&width_anim, current_tab_width);
            }
        } else {
            self.initial_width = Some(current_tab_width);
        }

        let indicator_offset = selected.left;

        if let Some(initial_offset) = self.initial_offset {
            let offset_anim =
                self.offset_animatable.get_or_insert_with(|| Animatable((initial_offset, Dp::VectorConverter))).clone();

            if indicator_offset != offset_anim.target_value() {
                self.launch_animation(&offset_anim, indicator_offset);
            }
        } else {
            self.initial_offset = Some(indicator_offset);
        }

        let offset_value = self.offset_animatable.as_ref().map_or(indicator_offset, |a| a.value());
        let offset = if scope.layout_direction() == kompoz_graphics::LayoutDirection::Ltr {
            offset_value
        } else {
            -offset_value
        };

        let width = self.width_animatable.as_ref().map_or(current_tab_width, |a| a.value());

        let placeable = measurable.measure(named_args!(
            constraints.copy(min_width = scope.round_to_px(width), max_width = scope.round_to_px(width),)
        ));

        let x = scope.round_to_px(offset);
        scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, x, 0))
    }
}

// ---- The deprecated rows ---------------------------------------------------------------------------------------

/// A fixed tab row whose indicator is given the tabs' positions (Compose's deprecated `TabRow`, replaced by
/// [`PrimaryTabRow`] and [`SecondaryTabRow`]): `tabs` sharing the width, `divider` at the bottom, `indicator` over
/// the whole row with the positions (None: Kotlin's default, a [`TabRowDefaults::SecondaryIndicator`] under the
/// selected tab by [`TabRowDefaults::tab_indicator_offset`]). Its parts are composed as it measures.
///
/// Kotlin: `TabRow(selectedTabIndex: Int, modifier: Modifier = Modifier, containerColor: Color = TabRowDefaults.primaryContainerColor, contentColor: Color = TabRowDefaults.primaryContentColor, indicator: @Composable (tabPositions: List<TabPosition>) -> Unit, divider: @Composable () -> Unit = @Composable { HorizontalDivider() }, tabs: @Composable () -> Unit)`
#[deprecated(note = "Replaced with PrimaryTabRow and SecondaryTabRow.")]
#[kompozable]
pub fn TabRow(
    selected_tab_index: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(TabRowDefaults.primary_container_color())] container_color: Color,
    #[default(TabRowDefaults.primary_content_color())] content_color: Color,
    #[default(None)] indicator: Option<Rc<dyn Fn(Vec<TabPosition>)>>,
    #[default(DefaultDivider)] divider: impl Fn() + 'static,
    tabs: impl Fn() + 'static,
) {
    let indicator = indicator.unwrap_or_else(|| {
        Rc::new(move |tab_positions: Vec<TabPosition>| {
            if (selected_tab_index as usize) < tab_positions.len() {
                TabRowDefaults.SecondaryIndicator(
                    modifier =
                        TabRowDefaults.tab_indicator_offset(Modifier, tab_positions[selected_tab_index as usize]),
                );
            }
        })
    });
    TabRowWithSubcomposeImpl(modifier, container_color, content_color, indicator, Rc::new(divider), Rc::new(tabs));
}

/// A scrolling tab row whose indicator is given the tabs' positions (Compose's deprecated `ScrollableTabRow`,
/// replaced by [`PrimaryScrollableTabRow`] and [`SecondaryScrollableTabRow`]): tabs at least
/// [`TabRowDefaults::ScrollableTabRowMinTabWidth`] wide, `edge_padding` at either end, a scroll state of its own;
/// `indicator` (None: Kotlin's default, a [`TabRowDefaults::SecondaryIndicator`] under the selected tab) over the
/// whole row with the positions.
///
/// Kotlin: `ScrollableTabRow(selectedTabIndex: Int, modifier: Modifier = Modifier, containerColor: Color = TabRowDefaults.primaryContainerColor, contentColor: Color = TabRowDefaults.primaryContentColor, edgePadding: Dp = TabRowDefaults.ScrollableTabRowEdgeStartPadding, indicator: @Composable (tabPositions: List<TabPosition>) -> Unit, divider: @Composable () -> Unit = @Composable { HorizontalDivider() }, tabs: @Composable () -> Unit)`
#[deprecated(note = "Replaced with PrimaryScrollableTabRow and SecondaryScrollableTabRow tab variants.")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ScrollableTabRow(
    selected_tab_index: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(TabRowDefaults.primary_container_color())] container_color: Color,
    #[default(TabRowDefaults.primary_content_color())] content_color: Color,
    #[default(TabRowDefaults::ScrollableTabRowEdgeStartPadding)] edge_padding: Dp,
    #[default(None)] indicator: Option<Rc<dyn Fn(Vec<TabPosition>)>>,
    #[default(DefaultDivider)] divider: impl Fn() + 'static,
    tabs: impl Fn() + 'static,
) {
    let indicator = indicator.unwrap_or_else(|| {
        Rc::new(move |tab_positions: Vec<TabPosition>| {
            TabRowDefaults.SecondaryIndicator(
                modifier = TabRowDefaults.tab_indicator_offset(Modifier, tab_positions[selected_tab_index as usize]),
            );
        })
    });
    ScrollableTabRowWithSubcomposeImpl(
        selected_tab_index,
        indicator,
        modifier,
        container_color,
        content_color,
        edge_padding,
        Rc::new(divider),
        Rc::new(tabs),
        remember_scroll_state(0),
    );
}

/// The slots of the deprecated rows' subcomposition (Compose's private `TabSlots`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum TabSlots {
    Tabs,
    Divider,
    Indicator,
}

/// The deprecated fixed row's layout (Compose's private `TabRowWithSubcomposeImpl`): the tabs composed and measured
/// first, the divider and the indicator (given the tabs' positions, over the whole row) composed as it places them.
#[kompozable]
fn TabRowWithSubcomposeImpl(
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    indicator: Rc<dyn Fn(Vec<TabPosition>)>,
    divider: Rc<dyn Fn()>,
    tabs: Rc<dyn Fn()>,
) {
    Surface(
        modifier = modifier.selectable_group(),
        color = container_color,
        content_color = Some(content_color),
        content = move || {
            let (indicator, divider, tabs) = (indicator.clone(), divider.clone(), tabs.clone());
            SubcomposeLayout(
                modifier = Modifier.fill_max_width(),
                measure_policy = move |m, constraints| {
                    let tab_row_width = constraints.max_width;
                    let tabs = tabs.clone();
                    let tab_measurables = m.subcompose(TabSlots::Tabs, move || tabs());
                    let tab_count = tab_measurables.len() as i32;
                    let mut tab_width = 0;
                    if tab_count > 0 {
                        tab_width = tab_row_width / tab_count;
                    }
                    let tab_row_height =
                        tab_measurables.iter().fold(0, |max, curr| curr.max_intrinsic_height(tab_width).max(max));

                    let tab_placeables: Vec<Placeable> = tab_measurables
                        .iter()
                        .map(|it| {
                            it.measure(named_args!(constraints.copy(
                                min_width = tab_width,
                                max_width = tab_width,
                                min_height = tab_row_height,
                                max_height = tab_row_height,
                            )))
                        })
                        .collect();

                    let tab_positions: Vec<TabPosition> = tab_measurables
                        .iter()
                        .enumerate()
                        .map(|(index, measurable)| {
                            let mut content_width =
                                m.to_dp(measurable.max_intrinsic_width(tab_row_height).min(tab_width) as f32);
                            content_width = content_width - HorizontalTextPadding * 2.0;
                            // Enforce minimum touch target of 24.dp
                            let indicator_width = max_dp(content_width, Dp(24.0));
                            TabPosition::new(
                                m.to_dp(tab_width as f32) * index as f32,
                                m.to_dp(tab_width as f32),
                                indicator_width,
                            )
                        })
                        .collect();

                    let (m2, divider, indicator) = (m.clone(), divider.clone(), indicator.clone());
                    m.layout(tab_row_width, tab_row_height, move |p| {
                        for (index, placeable) in tab_placeables.iter().enumerate() {
                            p.place_relative(placeable, index as i32 * tab_width, 0);
                        }

                        let divider = divider.clone();
                        for it in m2.subcompose(TabSlots::Divider, move || divider()) {
                            let placeable = it.measure(named_args!(constraints.copy(min_height = 0)));
                            p.place_relative(&placeable, 0, tab_row_height - placeable.height());
                        }

                        let (indicator, tab_positions) = (indicator.clone(), tab_positions.clone());
                        for it in m2.subcompose(TabSlots::Indicator, move || indicator(tab_positions.clone())) {
                            p.place_relative(&it.measure(Constraints::fixed(tab_row_width, tab_row_height)), 0, 0);
                        }
                    })
                },
            );
        },
    );
}

/// The deprecated scrolling row's layout (Compose's private `ScrollableTabRowWithSubcomposeImpl`): the tabs composed
/// and measured first (at least the default minimum width), the divider and the indicator composed as it places
/// them; laid out, it scrolls a newly selected tab into the middle.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ScrollableTabRowWithSubcomposeImpl(
    selected_tab_index: i32,
    indicator: Rc<dyn Fn(Vec<TabPosition>)>,
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    edge_padding: Dp,
    divider: Rc<dyn Fn()>,
    tabs: Rc<dyn Fn()>,
    scroll_state: ScrollState,
) {
    Surface(
        modifier = modifier,
        color = container_color,
        content_color = Some(content_color),
        content = move || {
            let coroutine_scope = remember_coroutine_scope();
            // TODO Load the motionScheme tokens from the component tokens file
            let scroll_animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
            let (s, c) = (scroll_state.clone(), coroutine_scope.clone());
            let scrollable_tab_data = remember_keyed((scroll_state.clone(), coroutine_scope), move || {
                Rc::new(ScrollableTabData::new(s, c, scroll_animation_spec))
            });
            let (indicator, divider, tabs) = (indicator.clone(), divider.clone(), tabs.clone());
            SubcomposeLayout(
                modifier = Modifier
                    .fill_max_width()
                    .wrap_content_size_with(Alignment::CenterStart, false)
                    .horizontal_scroll(&scroll_state)
                    .selectable_group()
                    .clip_to_bounds(),
                measure_policy = move |m, constraints| {
                    let min_tab_width = m.round_to_px(TabRowDefaults::ScrollableTabRowMinTabWidth);
                    let padding = m.round_to_px(edge_padding);

                    let tabs = tabs.clone();
                    let tab_measurables = m.subcompose(TabSlots::Tabs, move || tabs());

                    let layout_height = tab_measurables
                        .iter()
                        .fold(0, |curr, measurable| curr.max(measurable.max_intrinsic_height(Constraints::Infinity)));

                    let tab_constraints = named_args!(constraints.copy(
                        min_width = min_tab_width,
                        min_height = layout_height,
                        max_height = layout_height,
                    ));

                    let mut tab_placeables = Vec::new();
                    let mut tab_content_widths = Vec::new();
                    for it in &tab_measurables {
                        let placeable = it.measure(tab_constraints);
                        let mut content_width =
                            m.to_dp(it.max_intrinsic_width(placeable.height()).min(placeable.width()) as f32);
                        content_width = content_width - HorizontalTextPadding * 2.0;
                        tab_placeables.push(placeable);
                        tab_content_widths.push(content_width);
                    }

                    let layout_width =
                        tab_placeables.iter().fold(padding * 2, |curr, measurable| curr + measurable.width());

                    // Position the children.
                    let (m2, divider, indicator, data) =
                        (m.clone(), divider.clone(), indicator.clone(), scrollable_tab_data.clone());
                    m.layout(layout_width, layout_height, move |p| {
                        // Place the tabs
                        let mut tab_positions = Vec::new();
                        let mut left = padding;
                        for (index, placeable) in tab_placeables.iter().enumerate() {
                            p.place_relative(placeable, left, 0);
                            tab_positions.push(TabPosition::new(
                                m2.to_dp(left as f32),
                                m2.to_dp(placeable.width() as f32),
                                tab_content_widths[index],
                            ));
                            left += placeable.width();
                        }

                        // The divider is measured with its own height, and width equal to the total width
                        // of the tab row, and then placed on top of the tabs.
                        let divider = divider.clone();
                        for it in m2.subcompose(TabSlots::Divider, move || divider()) {
                            let placeable = it.measure(named_args!(constraints.copy(
                                min_height = 0,
                                min_width = layout_width,
                                max_width = layout_width,
                            )));
                            p.place_relative(&placeable, 0, layout_height - placeable.height());
                        }

                        // The indicator container is measured to fill the entire space occupied by the tab
                        // row, and then placed on top of the divider.
                        let (indicator, positions) = (indicator.clone(), tab_positions.clone());
                        for it in m2.subcompose(TabSlots::Indicator, move || indicator(positions.clone())) {
                            p.place_relative(&it.measure(Constraints::fixed(layout_width, layout_height)), 0, 0);
                        }

                        let density: Density = **m2;
                        data.on_laid_out(&density, padding, &tab_positions, selected_tab_index);
                    })
                },
            );
        },
    );
}

// ---- TabPosition -----------------------------------------------------------------------------------------------

/// Where a tab is in its row (Compose's `TabPosition`): its left edge (from the row's start) and width, and the width
/// of its content (what an indicator matching the content is as wide as). Made by the tab rows.
#[derive(Clone, Copy, PartialEq)]
pub struct TabPosition {
    left: Dp,
    width: Dp,
    content_width: Dp,
}

impl TabPosition {
    /// Compose's internal constructor.
    pub(crate) fn new(left: Dp, width: Dp, content_width: Dp) -> TabPosition {
        TabPosition { left, width, content_width }
    }

    /// Its left edge, from the row's start (Compose's `left`).
    pub fn left(&self) -> Dp {
        self.left
    }

    /// Its width (Compose's `width`).
    pub fn width(&self) -> Dp {
        self.width
    }

    /// Its content's width (Compose's `contentWidth`).
    pub fn content_width(&self) -> Dp {
        self.content_width
    }

    /// Its right edge: `left + width` (Compose's `right`).
    pub fn right(&self) -> Dp {
        self.left + self.width
    }
}

impl fmt::Debug for TabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TabPosition(left={:?}, right={:?}, width={:?}, contentWidth={:?})",
            self.left,
            self.right(),
            self.width,
            self.content_width
        )
    }
}

// ---- TabRowDefaults --------------------------------------------------------------------------------------------

/// The tab rows' defaults (Compose's `TabRowDefaults`): the scrolling rows' sizes, the rows' colors, the indicators.
/// Its composables are methods of the value, as Kotlin calls them (`TabRowDefaults.primary_container_color()`,
/// `TabRowDefaults.PrimaryIndicator(…)`).
#[derive(Clone, Copy, Debug)]
pub struct TabRowDefaults;

impl TabRowDefaults {
    /// A scrolling row's least tab width, 90 dp (Compose's `ScrollableTabRowMinTabWidth`).
    pub const ScrollableTabRowMinTabWidth: Dp = Dp(90.0);

    /// The padding before a scrolling row's first tab and after its last, 52 dp (Compose's
    /// `ScrollableTabRowEdgeStartPadding`).
    pub const ScrollableTabRowEdgeStartPadding: Dp = Dp(52.0);
}

#[named]
impl TabRowDefaults {
    /// A tab row's container color (Compose's deprecated `containerColor`): the surface.
    #[deprecated(note = "Use TabRowDefaults.primaryContainerColor instead")]
    #[track_caller]
    pub fn container_color(&self) -> Color {
        PrimaryNavigationTabTokens::ContainerColor.value()
    }

    /// A primary tab row's container color: the surface (Compose's `primaryContainerColor`).
    #[track_caller]
    pub fn primary_container_color(&self) -> Color {
        PrimaryNavigationTabTokens::ContainerColor.value()
    }

    /// A secondary tab row's container color: the surface (Compose's `secondaryContainerColor`).
    #[track_caller]
    pub fn secondary_container_color(&self) -> Color {
        SecondaryNavigationTabTokens::ContainerColor.value()
    }

    /// A tab row's content color (Compose's deprecated `contentColor`): the primary color.
    #[deprecated(note = "Use TabRowDefaults.primaryContentColor instead")]
    #[track_caller]
    pub fn content_color(&self) -> Color {
        PrimaryNavigationTabTokens::ActiveLabelTextColor.value()
    }

    /// A primary tab row's content color: the primary color (Compose's `primaryContentColor`).
    #[track_caller]
    pub fn primary_content_color(&self) -> Color {
        PrimaryNavigationTabTokens::ActiveLabelTextColor.value()
    }

    /// A secondary tab row's content color: the surface's content color (Compose's `secondaryContentColor`).
    #[track_caller]
    pub fn secondary_content_color(&self) -> Color {
        SecondaryNavigationTabTokens::ActiveLabelTextColor.value()
    }

    /// The deprecated indicator (Compose's `TabRowDefaults.Indicator`, replaced by [`SecondaryIndicator`]): a line
    /// of `height` in `color` across what it is given.
    ///
    /// [`SecondaryIndicator`]: TabRowDefaults::SecondaryIndicator
    ///
    /// Kotlin: `Indicator(modifier: Modifier = Modifier, height: Dp = PrimaryNavigationTabTokens.ActiveIndicatorHeight, color: Color = MaterialTheme.colorScheme.fromToken(PrimaryNavigationTabTokens.ActiveIndicatorColor))`
    #[deprecated(note = "Use SecondaryIndicator instead.")]
    #[track_caller]
    pub fn Indicator(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(PrimaryNavigationTabTokens::ActiveIndicatorHeight)] height: Dp,
        #[default(PrimaryNavigationTabTokens::ActiveIndicatorColor.value())] color: Color,
    ) {
        indicator_line(modifier, height, color);
    }

    /// The primary indicator (Compose's `TabRowDefaults.PrimaryIndicator`): a bar of `width` (24 dp; `Dp::Unspecified`
    /// takes what it is given, as the primary rows' default does) and `height` (3 dp) in `color` (the primary color),
    /// of `shape` (rounded).
    ///
    /// Kotlin: `PrimaryIndicator(modifier: Modifier = Modifier, width: Dp = 24.dp, height: Dp = PrimaryNavigationTabTokens.ActiveIndicatorHeight, color: Color = PrimaryNavigationTabTokens.ActiveIndicatorColor.value, shape: Shape = PrimaryNavigationTabTokens.ActiveIndicatorShape)`
    #[track_caller]
    pub fn PrimaryIndicator(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(Dp(24.0))] width: Dp,
        #[default(PrimaryNavigationTabTokens::ActiveIndicatorHeight)] height: Dp,
        #[default(PrimaryNavigationTabTokens::ActiveIndicatorColor.value())] color: Color,
        #[default(Arc::new(PrimaryNavigationTabTokens::ActiveIndicatorShape))] shape: Arc<dyn Shape>,
    ) {
        kompoz_runtime::kompozable_block!({
            Spacer(modifier.required_height(height).required_width(width).background(color, shape));
        })
    }

    /// The secondary indicator (Compose's `TabRowDefaults.SecondaryIndicator`): a line of `height` (3 dp) in `color`
    /// (the primary color) across what it is given.
    ///
    /// Kotlin: `SecondaryIndicator(modifier: Modifier = Modifier, height: Dp = PrimaryNavigationTabTokens.ActiveIndicatorHeight, color: Color = PrimaryNavigationTabTokens.ActiveIndicatorColor.value)`
    #[track_caller]
    pub fn SecondaryIndicator(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(PrimaryNavigationTabTokens::ActiveIndicatorHeight)] height: Dp,
        #[default(PrimaryNavigationTabTokens::ActiveIndicatorColor.value())] color: Color,
    ) {
        indicator_line(modifier, height, color);
    }

    /// `modifier` placing the deprecated indicator under the tab at `current_tab_position` (Compose's deprecated
    /// `TabRowDefaults.Modifier.tabIndicatorOffset`, for use with [`TabRow`]/[`ScrollableTabRow`]'s positions): across
    /// the row, at its bottom start, offset to the tab and as wide, both animated on the default spatial motion.
    /// Composable: Compose's `composed { }` animates where the modifier is used; KompozR has no `composed`, so it
    /// animates where it is made (the same place in the indicator's content).
    ///
    /// Kotlin: `tabIndicatorOffset(currentTabPosition: TabPosition)`
    #[deprecated(
        note = "Solely for use alongside deprecated TabRowDefaults.Indicator method. For recommended PrimaryIndicator \
                and SecondaryIndicator methods, please use TabIndicatorScope.tabIndicatorOffset method."
    )]
    #[track_caller]
    pub fn tab_indicator_offset(&self, modifier: Modifier, current_tab_position: TabPosition) -> Modifier {
        kompoz_runtime::kompozable_block!({
            // TODO Load the motionScheme tokens from the component tokens file
            let current_tab_width = animate_dp_as_state(
                current_tab_position.width,
                animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<Dp>(),
            );
            let indicator_offset = animate_dp_as_state(
                current_tab_position.left,
                animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<Dp>(),
            );
            modifier
                .fill_max_width()
                .wrap_content_size_with(Alignment::BottomStart, false)
                .offset_px(move |density: &Density| IntOffset::new(density.round_to_px(indicator_offset.value()), 0))
                .width(current_tab_width.value())
        })
    }
}

/// A line of `height` in `color` across what it is given (the body of Compose's `Indicator` and
/// `SecondaryIndicator`).
#[kompozable]
fn indicator_line(modifier: Modifier, height: Dp, color: Color) {
    Box(modifier =
        modifier.fill_max_width().height(height).background(color, Arc::new(kompoz_graphics::RectangleShape)));
}

// ---- ScrollableTabData -----------------------------------------------------------------------------------------

/// What a scrolling row keeps to scroll a newly selected tab into view (Compose's private `ScrollableTabData`).
struct ScrollableTabData {
    scroll_state: ScrollState,
    coroutine_scope: CoroutineScope,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    selected_tab: Cell<Option<i32>>,
}

impl PartialEq for ScrollableTabData {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl ScrollableTabData {
    fn new(
        scroll_state: ScrollState,
        coroutine_scope: CoroutineScope,
        animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    ) -> ScrollableTabData {
        ScrollableTabData { scroll_state, coroutine_scope, animation_spec, selected_tab: Cell::new(None) }
    }

    /// Laid out with `selected_tab` (Compose's `onLaidOut`): a tab other than the last one (or the first time)
    /// scrolled to the middle, as near as the row goes.
    fn on_laid_out(&self, density: &Density, edge_offset: i32, tab_positions: &[TabPosition], selected_tab: i32) {
        // Animate if the new tab is different from the old tab, or this is called for the first
        // time (i.e selectedTab is `null`).
        if self.selected_tab.get() != Some(selected_tab) {
            self.selected_tab.set(Some(selected_tab));
            if let Some(it) = usize::try_from(selected_tab).ok().and_then(|i| tab_positions.get(i)) {
                // Scrolls to the tab with [tabPosition], trying to place it in the center of the
                // screen or as close to the center as possible.
                let calculated_offset = self.calculate_tab_offset(it, density, edge_offset, tab_positions);
                if self.scroll_state.value() != calculated_offset {
                    let (scroll_state, spec) =
                        (self.scroll_state.clone(), self.animation_spec.clone() as Arc<dyn AnimationSpec<f32>>);
                    self.coroutine_scope.launch(async move {
                        named_args!(scroll_state.animate_scroll_to(calculated_offset, animation_spec = spec)).await;
                    });
                }
            }
        }
    }

    /// The scroll putting `tab` in the middle, within what the row scrolls (Compose's `calculateTabOffset`).
    fn calculate_tab_offset(
        &self,
        tab: &TabPosition,
        density: &Density,
        edge_offset: i32,
        tab_positions: &[TabPosition],
    ) -> i32 {
        let last = tab_positions.last().expect("a tab");
        let total_tab_row_width = density.round_to_px(last.right()) + edge_offset;
        let visible_width = total_tab_row_width - self.scroll_state.max_value();
        let tab_offset = density.round_to_px(tab.left);
        let scroller_center = visible_width / 2;
        let tab_width = density.round_to_px(tab.width);
        let centered_tab_offset = tab_offset - (scroller_center - tab_width / 2);
        // How much space we have to scroll. If the visible width is <= to the total width, then
        // we have no space to scroll as everything is always visible.
        let available_space = (total_tab_row_width - visible_width).max(0);
        centered_tab_offset.clamp(0, available_space)
    }
}

/// The larger of two lengths (Kotlin's `maxOf` of Dps).
fn max_dp(a: Dp, b: Dp) -> Dp {
    if a >= b { a } else { b }
}
