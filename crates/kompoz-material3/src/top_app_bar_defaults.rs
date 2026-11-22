//! What Material's top app bars are made of, as Compose's AppBar.kt has it: their scroll behaviors
//! ([`TopAppBarScrollBehavior`]: pinned, entering always, exiting until collapsed — the nested scroll connections
//! moving a [`TopAppBarState`] as the content scrolls, and the settling after a fling or a drag of the bar), their
//! state (how far collapsed, how far the content scrolled under them; saved), their colors ([`TopAppBarColors`]) and
//! defaults ([`TopAppBarDefaults`]: colors, heights, insets, padding, the behaviors).

// (the deprecated colors functions and scroll behavior are defined here)
#![allow(deprecated)]

use crate::color_scheme::ColorScheme;
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::material_theme::MaterialTheme;
use crate::tokens::{
    AppBarLargeFlexibleTokens, AppBarLargeTokens, AppBarMediumFlexibleTokens, AppBarMediumTokens, AppBarSmallTokens,
    AppBarTokens, MotionSchemeKeyTokens,
};
use kompoz_animation::{
    AnimationScope, AnimationSpec, AnimationVector1D, DecayAnimationSpec, Easing, FastOutLinearInEasing,
    FloatDecaySpecOf, float_animation_state, remember_spline_based_decay,
};
use kompoz_foundation::gestures::ScrollableState;
use kompoz_foundation::layout::{PaddingValues, WindowInsets, WindowInsetsSides};
use kompoz_foundation::lazy::LazyListState;
use kompoz_foundation::prelude::ScrollState;
use kompoz_graphics::{Color, Dp, Offset, lerp_color};
use kompoz_runtime::{
    DerivedState, KotlinProperties, MutableState, Saveable, SavedValue, Saver, derived_state_of, kompozable,
    kompozable_block, list_saver, mutable_float_state_of, named, named_args, remember_keyed, remember_saveable_with,
};
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollSource};
use kompoz_ui::unit::Velocity;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- TopAppBarScrollBehavior -------------------------------------------------------------------------------------

/// How a top app bar behaves as the content under it scrolls (Compose's `TopAppBarScrollBehavior`): the
/// [`TopAppBarState`] it moves, whether the bar stays (`is_pinned`), how it settles after a fling (its specs), and
/// the nested scroll connection to put on the scrolled content's parent (`Modifier.nested_scroll(behavior.
/// nested_scroll_connection())`). Made by [`TopAppBarDefaults`]' `…_scroll_behavior` functions; shared as an
/// `Rc<dyn TopAppBarScrollBehavior>`, equal only to itself.
pub trait TopAppBarScrollBehavior {
    /// The state the bar is laid out and colored by, which this behavior moves (Compose's `state`).
    fn state(&self) -> TopAppBarState;

    /// Whether the bar stays where it is as the content scrolls (Compose's `isPinned`): it then takes no drags
    /// either.
    fn is_pinned(&self) -> bool;

    /// What snaps the bar to collapsed or expanded once a fling or drag leaves it in between (Compose's
    /// `snapAnimationSpec`): none, it stays.
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>>;

    /// What carries a fling's speed on into the bar's collapsing or expanding (Compose's `flingAnimationSpec`).
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>>;

    /// The connection to give the scrolled content's parent (Compose's `nestedScrollConnection`): what it hears of
    /// the content's scrolls and flings moves the state.
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection>;
}

impl PartialEq for dyn TopAppBarScrollBehavior {
    /// The same behavior (Kotlin's `==` of these objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl fmt::Debug for dyn TopAppBarScrollBehavior {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TopAppBarScrollBehavior(state={:?}, isPinned={})", self.state(), self.is_pinned())
    }
}

// ---- TopAppBarDefaults -------------------------------------------------------------------------------------------

/// The top app bars' defaults (Compose's `TopAppBarDefaults`): their colors, insets, content padding, heights and
/// scroll behaviors. What Compose reads in composition is a method of the value (`TopAppBarDefaults.
/// top_app_bar_colors()`); its heights are associated consts (`TopAppBarDefaults::TopAppBarExpandedHeight`).
#[derive(Clone, Copy, Debug)]
pub struct TopAppBarDefaults;

impl TopAppBarDefaults {
    /// A small (and center aligned) top app bar's height, and the most a scroll behavior expands it to: 64 dp
    /// (Compose's `TopAppBarExpandedHeight`).
    pub const TopAppBarExpandedHeight: Dp = AppBarSmallTokens::ContainerHeight;

    /// A medium top app bar's height collapsed: 64 dp (Compose's `MediumAppBarCollapsedHeight`).
    pub const MediumAppBarCollapsedHeight: Dp = AppBarSmallTokens::ContainerHeight;

    /// A medium top app bar's height expanded: 112 dp (Compose's `MediumAppBarExpandedHeight`).
    pub const MediumAppBarExpandedHeight: Dp = AppBarMediumTokens::ContainerHeight;

    /// A medium flexible top app bar's height expanded without a subtitle: 112 dp (Compose's
    /// `MediumFlexibleAppBarWithoutSubtitleExpandedHeight`).
    pub const MediumFlexibleAppBarWithoutSubtitleExpandedHeight: Dp = AppBarMediumFlexibleTokens::ContainerHeight;

    /// A medium flexible top app bar's height expanded with a subtitle: 136 dp (Compose's
    /// `MediumFlexibleAppBarWithSubtitleExpandedHeight`).
    pub const MediumFlexibleAppBarWithSubtitleExpandedHeight: Dp = AppBarMediumFlexibleTokens::LargeContainerHeight;

    /// A large top app bar's height collapsed: 64 dp (Compose's `LargeAppBarCollapsedHeight`).
    pub const LargeAppBarCollapsedHeight: Dp = AppBarSmallTokens::ContainerHeight;

    /// A large top app bar's height expanded: 152 dp (Compose's `LargeAppBarExpandedHeight`).
    pub const LargeAppBarExpandedHeight: Dp = AppBarLargeTokens::ContainerHeight;

    /// A large flexible top app bar's height expanded without a subtitle: 120 dp (Compose's
    /// `LargeFlexibleAppBarWithoutSubtitleExpandedHeight`).
    pub const LargeFlexibleAppBarWithoutSubtitleExpandedHeight: Dp = AppBarLargeFlexibleTokens::ContainerHeight;

    /// A large flexible top app bar's height expanded with a subtitle: 152 dp (Compose's
    /// `LargeFlexibleAppBarWithSubtitleExpandedHeight`).
    pub const LargeFlexibleAppBarWithSubtitleExpandedHeight: Dp = AppBarLargeFlexibleTokens::LargeContainerHeight;

    /// The padding a top app bar keeps its content in by default: none (Compose's `ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::all(Dp(0.0))
    }

    /// A scheme's top app bar colors (Compose's internal `ColorScheme.defaultTopAppBarColors`; made afresh, Compose's
    /// per-scheme cache isn't kept): the surface, the high surface container scrolled under, the navigation icon and
    /// title in the surface's content color, the actions and subtitle in its variant's.
    pub(crate) fn default_top_app_bar_colors(color_scheme: &ColorScheme) -> TopAppBarColors {
        TopAppBarColors::new(
            color_scheme.from_token(AppBarTokens::ContainerColor),
            color_scheme.from_token(AppBarTokens::OnScrollContainerColor),
            color_scheme.from_token(AppBarTokens::LeadingIconColor),
            color_scheme.from_token(AppBarTokens::TitleColor),
            color_scheme.from_token(AppBarTokens::TrailingIconColor),
            color_scheme.from_token(AppBarTokens::SubtitleColor),
        )
    }
}

#[named]
impl TopAppBarDefaults {
    /// A top app bar's colors (Compose's `topAppBarColors()` and `topAppBarColors(containerColor, …)`): the theme's
    /// (see [`TopAppBarColors`]), those given in place of theirs (an unspecified one keeps the theme's).
    ///
    /// Kotlin: `topAppBarColors(containerColor: Color = Color.Unspecified, scrolledContainerColor: Color = Color.Unspecified, navigationIconContentColor: Color = Color.Unspecified, titleContentColor: Color = Color.Unspecified, actionIconContentColor: Color = Color.Unspecified, subtitleContentColor: Color = Color.Unspecified): TopAppBarColors`
    #[track_caller]
    pub fn top_app_bar_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] scrolled_container_color: Color,
        #[default(Color::Unspecified)] navigation_icon_content_color: Color,
        #[default(Color::Unspecified)] title_content_color: Color,
        #[default(Color::Unspecified)] action_icon_content_color: Color,
        #[default(Color::Unspecified)] subtitle_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarDefaults::default_top_app_bar_colors(&MaterialTheme::color_scheme()).copy_with(
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            subtitle_content_color,
        )
    }

    /// The insets a top app bar keeps clear of by default (Compose's `windowInsets`): the system bars' at the sides
    /// and the top.
    #[track_caller]
    pub fn window_insets(&self) -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Horizontal + WindowInsetsSides::Top)
    }

    /// A center aligned top app bar's colors (Compose's deprecated `centerAlignedTopAppBarColors()` and
    /// `centerAlignedTopAppBarColors(containerColor, …)`): [`top_app_bar_colors`](Self::top_app_bar_colors)'.
    ///
    /// Kotlin: `centerAlignedTopAppBarColors(containerColor: Color = Color.Unspecified, scrolledContainerColor: Color = Color.Unspecified, navigationIconContentColor: Color = Color.Unspecified, titleContentColor: Color = Color.Unspecified, actionIconContentColor: Color = Color.Unspecified): TopAppBarColors`
    #[deprecated(note = "Use topAppBarColors instead")]
    #[track_caller]
    pub fn center_aligned_top_app_bar_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] scrolled_container_color: Color,
        #[default(Color::Unspecified)] navigation_icon_content_color: Color,
        #[default(Color::Unspecified)] title_content_color: Color,
        #[default(Color::Unspecified)] action_icon_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarDefaults::default_top_app_bar_colors(&MaterialTheme::color_scheme()).copy_with(
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            Color::Unspecified,
        )
    }

    /// A medium top app bar's colors (Compose's deprecated `mediumTopAppBarColors()` and
    /// `mediumTopAppBarColors(containerColor, …)`): [`top_app_bar_colors`](Self::top_app_bar_colors)'.
    ///
    /// Kotlin: `mediumTopAppBarColors(containerColor: Color = Color.Unspecified, scrolledContainerColor: Color = Color.Unspecified, navigationIconContentColor: Color = Color.Unspecified, titleContentColor: Color = Color.Unspecified, actionIconContentColor: Color = Color.Unspecified): TopAppBarColors`
    #[deprecated(note = "Use topAppBarColors instead")]
    #[track_caller]
    pub fn medium_top_app_bar_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] scrolled_container_color: Color,
        #[default(Color::Unspecified)] navigation_icon_content_color: Color,
        #[default(Color::Unspecified)] title_content_color: Color,
        #[default(Color::Unspecified)] action_icon_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarDefaults::default_top_app_bar_colors(&MaterialTheme::color_scheme()).copy_with(
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            Color::Unspecified,
        )
    }

    /// A large top app bar's colors (Compose's deprecated `largeTopAppBarColors()` and
    /// `largeTopAppBarColors(containerColor, …)`): [`top_app_bar_colors`](Self::top_app_bar_colors)'.
    ///
    /// Kotlin: `largeTopAppBarColors(containerColor: Color = Color.Unspecified, scrolledContainerColor: Color = Color.Unspecified, navigationIconContentColor: Color = Color.Unspecified, titleContentColor: Color = Color.Unspecified, actionIconContentColor: Color = Color.Unspecified): TopAppBarColors`
    #[deprecated(note = "Use topAppBarColors instead")]
    #[track_caller]
    pub fn large_top_app_bar_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] scrolled_container_color: Color,
        #[default(Color::Unspecified)] navigation_icon_content_color: Color,
        #[default(Color::Unspecified)] title_content_color: Color,
        #[default(Color::Unspecified)] action_icon_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarDefaults::default_top_app_bar_colors(&MaterialTheme::color_scheme()).copy_with(
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            Color::Unspecified,
        )
    }

    /// A pinned top app bar's behavior (Compose's `pinnedScrollBehavior(state, canScroll)` and
    /// `pinnedScrollBehavior(state, canScroll, isScrollingContentAtStart)`): the bar stays; it only notes how far
    /// the content scrolled under it (what its scrolled color follows) while `can_scroll` says so;
    /// `is_scrolling_content_at_start` tells it whether the content is at its start (a list pre-scrolled, reversed:
    /// the content is under the bar). Remembered while `state` and the two closures (by what they capture) stay.
    ///
    /// Kotlin: `pinnedScrollBehavior(state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }, isScrollingContentAtStart: () -> Boolean = { true }): TopAppBarScrollBehavior`
    #[track_caller]
    pub fn pinned_scroll_behavior(
        &self,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(|| true)]
        #[memoized]
        is_scrolling_content_at_start: impl Fn() -> bool + 'static,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        let (at_start_id, at_start) = shared(is_scrolling_content_at_start);
        kompozable_block!({
            remember_keyed((state.clone(), can_scroll_id, at_start_id), move || {
                Rc::new(PinnedScrollBehavior::new(state, can_scroll, at_start)) as Rc<dyn TopAppBarScrollBehavior>
            })
        })
    }

    /// A pinned top app bar's behavior over a lazy list (Compose's `pinnedScrollBehavior(lazyListState, state,
    /// canScroll)`): [`pinned_scroll_behavior`](Self::pinned_scroll_behavior), told the content is at its start
    /// while `lazy_list_state` can't scroll back (forward, reversed).
    ///
    /// Kotlin: `pinnedScrollBehavior(lazyListState: LazyListState, state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }): TopAppBarScrollBehavior`
    #[track_caller]
    pub fn pinned_scroll_behavior_lazy_list(
        &self,
        lazy_list_state: LazyListState,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        kompozable_block!({
            let at_start = remember_is_scrolling_content_at_start_lazy_list(lazy_list_state);
            remember_keyed((state.clone(), can_scroll_id, at_start.clone()), move || {
                Rc::new(PinnedScrollBehavior::new(state, can_scroll, Rc::new(move || at_start.value())))
                    as Rc<dyn TopAppBarScrollBehavior>
            })
        })
    }

    /// A pinned top app bar's behavior over a scrolled content (Compose's `pinnedScrollBehavior(scrollState,
    /// reverseScrolling, state, canScroll)`): [`pinned_scroll_behavior`](Self::pinned_scroll_behavior), told the
    /// content is at its start while `scroll_state` can't scroll back (forward, `reverse_scrolling`).
    ///
    /// Kotlin: `pinnedScrollBehavior(scrollState: ScrollState, reverseScrolling: Boolean = false, state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }): TopAppBarScrollBehavior`
    #[track_caller]
    pub fn pinned_scroll_behavior_scroll_state(
        &self,
        scroll_state: ScrollState,
        #[default(false)] reverse_scrolling: bool,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        kompozable_block!({
            let at_start = remember_is_scrolling_content_at_start_scroll_state(scroll_state, reverse_scrolling);
            remember_keyed((state.clone(), can_scroll_id, at_start.clone()), move || {
                Rc::new(PinnedScrollBehavior::new(state, can_scroll, Rc::new(move || at_start.value())))
                    as Rc<dyn TopAppBarScrollBehavior>
            })
        })
    }

    /// A top app bar's behavior entering always (Compose's `enterAlwaysScrollBehavior(state, canScroll,
    /// snapAnimationSpec, flingAnimationSpec)` and `enterAlwaysScrollBehavior(…, isScrollingContentAtStart)`): the
    /// bar collapses as the content scrolls up and comes back as soon as it scrolls down (before the content moves),
    /// while `can_scroll` says so; after a fling it goes on along `fling_animation_spec`, then snaps by
    /// `snap_animation_spec` to collapsed or expanded. `is_scrolling_content_at_start` as
    /// [`pinned_scroll_behavior`](Self::pinned_scroll_behavior)'s. Remembered while `state`, `can_scroll` and the
    /// specs stay.
    ///
    /// Kotlin: `enterAlwaysScrollBehavior(state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float>? = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float>? = rememberSplineBasedDecay(), isScrollingContentAtStart: () -> Boolean = { true }): TopAppBarScrollBehavior`
    #[track_caller]
    pub fn enter_always_scroll_behavior(
        &self,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(Some(MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>))]
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(Some(Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>))]
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        #[default(|| true)] is_scrolling_content_at_start: impl Fn() -> bool + 'static,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        let at_start: Rc<dyn Fn() -> bool> = Rc::new(is_scrolling_content_at_start);
        kompozable_block!({
            remember_enter_always(state, can_scroll_id, can_scroll, snap_animation_spec, fling_animation_spec, at_start)
        })
    }

    /// A top app bar's behavior entering always, over a content that may be laid out reversed (Compose's deprecated
    /// `enterAlwaysScrollBehavior(state, canScroll, snapAnimationSpec, flingAnimationSpec, reverseLayout)`): as
    /// [`enter_always_scroll_behavior`](Self::enter_always_scroll_behavior), but over a `reverse_layout` content the
    /// bar takes nothing of the scrolls before the content and doesn't follow what it scrolls.
    ///
    /// Kotlin: `enterAlwaysScrollBehavior(state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float>? = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float>? = rememberSplineBasedDecay(), reverseLayout: Boolean = false): TopAppBarScrollBehavior`
    #[deprecated(
        note = "Please use the enterAlwaysScrollBehavior() function that takes lazyListState or scrollState parameters."
    )]
    #[track_caller]
    pub fn enter_always_scroll_behavior_reverse_layout(
        &self,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(Some(MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>))]
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(Some(Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>))]
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        #[default(false)] reverse_layout: bool,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        kompozable_block!({
            let keys = (
                state.clone(),
                can_scroll_id,
                snap_animation_spec.clone(),
                DecayKey(fling_animation_spec.clone()),
                reverse_layout,
            );
            remember_keyed(keys, move || {
                Rc::new(LegacyEnterAlwaysScrollBehavior::new(
                    state,
                    snap_animation_spec,
                    fling_animation_spec,
                    can_scroll,
                    reverse_layout,
                )) as Rc<dyn TopAppBarScrollBehavior>
            })
        })
    }

    /// A top app bar's behavior entering always over a lazy list (Compose's `enterAlwaysScrollBehavior(
    /// lazyListState, state, canScroll, snapAnimationSpec, flingAnimationSpec)`):
    /// [`enter_always_scroll_behavior`](Self::enter_always_scroll_behavior), told the content is at its start while
    /// `lazy_list_state` can't scroll back (forward, reversed).
    ///
    /// Kotlin: `enterAlwaysScrollBehavior(lazyListState: LazyListState, state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float>? = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float>? = rememberSplineBasedDecay()): TopAppBarScrollBehavior`
    #[track_caller]
    pub fn enter_always_scroll_behavior_lazy_list(
        &self,
        lazy_list_state: LazyListState,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(Some(MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>))]
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(Some(Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>))]
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        kompozable_block!({
            let at_start = remember_is_scrolling_content_at_start_lazy_list(lazy_list_state);
            remember_enter_always(
                state,
                can_scroll_id,
                can_scroll,
                snap_animation_spec,
                fling_animation_spec,
                Rc::new(move || at_start.value()),
            )
        })
    }

    /// A top app bar's behavior entering always over a scrolled content (Compose's `enterAlwaysScrollBehavior(
    /// scrollState, reverseScrolling, state, canScroll, snapAnimationSpec, flingAnimationSpec)`):
    /// [`enter_always_scroll_behavior`](Self::enter_always_scroll_behavior), told the content is at its start while
    /// `scroll_state` can't scroll back (forward, `reverse_scrolling`).
    ///
    /// Kotlin: `enterAlwaysScrollBehavior(scrollState: ScrollState, reverseScrolling: Boolean = false, state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float>? = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float>? = rememberSplineBasedDecay()): TopAppBarScrollBehavior`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn enter_always_scroll_behavior_scroll_state(
        &self,
        scroll_state: ScrollState,
        #[default(false)] reverse_scrolling: bool,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(Some(MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>))]
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(Some(Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>))]
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        kompozable_block!({
            let at_start = remember_is_scrolling_content_at_start_scroll_state(scroll_state, reverse_scrolling);
            remember_enter_always(
                state,
                can_scroll_id,
                can_scroll,
                snap_animation_spec,
                fling_animation_spec,
                Rc::new(move || at_start.value()),
            )
        })
    }

    /// A top app bar's behavior exiting until collapsed (Compose's `exitUntilCollapsedScrollBehavior`): the bar
    /// collapses as the content scrolls up (before it moves) and expands only once the content is back at its top,
    /// while `can_scroll` says so; after a fling it goes on along `fling_animation_spec`, then snaps by
    /// `snap_animation_spec` to collapsed or expanded. Remembered while `state`, `can_scroll` and the specs stay.
    ///
    /// Kotlin: `exitUntilCollapsedScrollBehavior(state: TopAppBarState = rememberTopAppBarState(), canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float>? = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float>? = rememberSplineBasedDecay()): TopAppBarScrollBehavior`
    #[track_caller]
    pub fn exit_until_collapsed_scroll_behavior(
        &self,
        #[default(remember_top_app_bar_state())] state: TopAppBarState,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(Some(MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>))]
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(Some(Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>))]
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    ) -> Rc<dyn TopAppBarScrollBehavior> {
        let (can_scroll_id, can_scroll) = shared(can_scroll);
        kompozable_block!({
            let keys =
                (state.clone(), can_scroll_id, snap_animation_spec.clone(), DecayKey(fling_animation_spec.clone()));
            remember_keyed(keys, move || {
                Rc::new(ExitUntilCollapsedScrollBehavior::new(
                    state,
                    snap_animation_spec,
                    fling_animation_spec,
                    can_scroll,
                )) as Rc<dyn TopAppBarScrollBehavior>
            })
        })
    }
}

/// A `#[memoized]` closure's identity and the closure shared (Kotlin's lambda as a value, what `remember` keys by).
fn shared(f: kompoz_runtime::Memoized<impl Fn() -> bool + 'static>) -> (Option<u64>, Rc<dyn Fn() -> bool>) {
    let id = f.id();
    (id, Rc::new(f.into_value()))
}

/// The entering-always behavior, remembered while the state, `can_scroll` and the specs stay (Compose's
/// `remember(state, canScroll, snapAnimationSpec, flingAnimationSpec) { EnterAlwaysScrollBehavior(…) }`: the
/// content-at-start closure isn't a key, as there).
#[track_caller]
fn remember_enter_always(
    state: TopAppBarState,
    can_scroll_id: Option<u64>,
    can_scroll: Rc<dyn Fn() -> bool>,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    is_scrolling_content_at_start: Rc<dyn Fn() -> bool>,
) -> Rc<dyn TopAppBarScrollBehavior> {
    let keys = (state.clone(), can_scroll_id, snap_animation_spec.clone(), DecayKey(fling_animation_spec.clone()));
    remember_keyed(keys, move || {
        Rc::new(EnterAlwaysScrollBehavior::new(
            state,
            snap_animation_spec,
            fling_animation_spec,
            can_scroll,
            is_scrolling_content_at_start,
        )) as Rc<dyn TopAppBarScrollBehavior>
    })
}

/// A decay spec as a `remember` key: the same instance (Kotlin's `==` of a decay spec, which has no `equals` of its
/// own; a [`FloatDecaySpecOf`] wrapped anew is the same one while it runs the same float decay).
struct DecayKey(Option<Arc<dyn DecayAnimationSpec<f32>>>);

impl PartialEq for DecayKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
                    || match (
                        a.as_any().downcast_ref::<FloatDecaySpecOf<f32>>(),
                        b.as_any().downcast_ref::<FloatDecaySpecOf<f32>>(),
                    ) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a.float_decay_spec(), b.float_decay_spec()),
                        _ => false,
                    }
            }
            _ => false,
        }
    }
}

/// Whether a lazy list is at its start (Compose's private `rememberIsScrollingContentAtStart(lazyListState)`): it
/// can't scroll back (forward, laid out reversed); remembered for the list.
#[kompozable]
fn remember_is_scrolling_content_at_start_lazy_list(lazy_list_state: LazyListState) -> DerivedState<bool> {
    remember_keyed(lazy_list_state.clone(), move || {
        derived_state_of(move || {
            if lazy_list_state.layout_info().reverse_layout() {
                !lazy_list_state.can_scroll_forward()
            } else {
                !lazy_list_state.can_scroll_backward()
            }
        })
    })
}

/// Whether a scrolled content is at its start (Compose's private `rememberIsScrollingContentAtStart(scrollState,
/// reverseScrolling)`): it can't scroll back (forward, `reverse_scrolling`); remembered for them.
#[kompozable]
fn remember_is_scrolling_content_at_start_scroll_state(
    scroll_state: ScrollState,
    reverse_scrolling: bool,
) -> DerivedState<bool> {
    remember_keyed((scroll_state.clone(), reverse_scrolling), move || {
        derived_state_of(move || {
            if reverse_scrolling { !scroll_state.can_scroll_forward() } else { !scroll_state.can_scroll_backward() }
        })
    })
}

// ---- TopAppBarState ----------------------------------------------------------------------------------------------

/// A [`TopAppBarState`] remembered and saved (Compose's `rememberTopAppBarState`): of the given offsets at first.
///
/// Kotlin: `rememberTopAppBarState(initialHeightOffsetLimit: Float = -Float.MAX_VALUE, initialHeightOffset: Float = 0f, initialContentOffset: Float = 0f): TopAppBarState`
#[kompozable]
pub fn remember_top_app_bar_state(
    #[default(-f32::MAX)] initial_height_offset_limit: f32,
    #[default(0.0)] initial_height_offset: f32,
    #[default(0.0)] initial_content_offset: f32,
) -> TopAppBarState {
    remember_saveable_with(&TopAppBarState::Saver(), move || {
        TopAppBarState::new(initial_height_offset_limit, initial_height_offset, initial_content_offset)
    })
}

/// The state of a top app bar's scroll behavior (Compose's `TopAppBarState`): how far the bar is collapsed
/// (`height_offset`, from 0 to `height_offset_limit`, negative), how far the content under it scrolled
/// (`content_offset`), and the fractions the bar is laid out and colored by. A cheap handle: clones are the same
/// state (equal when they are).
#[derive(Clone)]
pub struct TopAppBarState {
    inner: Rc<TopAppBarStateInner>,
}

struct TopAppBarStateInner {
    // (a plain property in Compose: not a state, so nothing observes it)
    height_offset_limit: Cell<f32>,
    height_offset: MutableState<f32>,
    content_offset: MutableState<f32>,
    is_scrolling_content_at_start: RefCell<Rc<dyn Fn() -> bool>>,
}

impl TopAppBarState {
    /// A state of these offsets (Compose's constructor; in composition, rather [`remember_top_app_bar_state`]).
    ///
    /// Kotlin: `TopAppBarState(initialHeightOffsetLimit: Float, initialHeightOffset: Float, initialContentOffset: Float)`
    pub fn new(
        initial_height_offset_limit: f32,
        initial_height_offset: f32,
        initial_content_offset: f32,
    ) -> TopAppBarState {
        TopAppBarState {
            inner: Rc::new(TopAppBarStateInner {
                height_offset_limit: Cell::new(initial_height_offset_limit),
                height_offset: mutable_float_state_of(initial_height_offset),
                content_offset: mutable_float_state_of(initial_content_offset),
                is_scrolling_content_at_start: RefCell::new(Rc::new(|| true)),
            }),
        }
    }

    /// The least the height offset can be, px (Compose's `heightOffsetLimit`): what the bar can collapse by,
    /// negative; set by the bar as it is laid out (not observed: it isn't a state).
    pub fn height_offset_limit(&self) -> f32 {
        self.inner.height_offset_limit.get()
    }

    /// Sets [`height_offset_limit`](Self::height_offset_limit).
    pub fn set_height_offset_limit(&self, height_offset_limit: f32) {
        self.inner.height_offset_limit.set(height_offset_limit);
    }

    /// How far the bar is collapsed, px (Compose's `heightOffset`): from 0 (expanded) down to the limit (collapsed).
    pub fn height_offset(&self) -> f32 {
        self.inner.height_offset.get()
    }

    /// Sets [`height_offset`](Self::height_offset), held between the limit and 0 (a limit above 0 panics, as
    /// Kotlin's `coerceIn` throws).
    pub fn set_height_offset(&self, new_offset: f32) {
        self.inner.height_offset.set(new_offset.clamp(self.height_offset_limit(), 0.0));
    }

    /// How far the content under the bar scrolled, px (Compose's `contentOffset`): what the scroll behaviors noted
    /// of its scrolls, negative as it scrolls up; how far it is under the bar.
    pub fn content_offset(&self) -> f32 {
        self.inner.content_offset.get()
    }

    /// Sets [`content_offset`](Self::content_offset).
    pub fn set_content_offset(&self, content_offset: f32) {
        self.inner.content_offset.set(content_offset);
    }

    /// Whether the content is at its start, as the scroll behavior was told (Compose's internal
    /// `isScrollingContentAtStart`).
    pub(crate) fn is_scrolling_content_at_start(&self) -> bool {
        let f = self.inner.is_scrolling_content_at_start.borrow().clone();
        f()
    }

    /// Sets what tells whether the content is at its start (Compose's internal `isScrollingContentAtStart` set).
    pub(crate) fn set_is_scrolling_content_at_start(&self, is_scrolling_content_at_start: Rc<dyn Fn() -> bool>) {
        *self.inner.is_scrolling_content_at_start.borrow_mut() = is_scrolling_content_at_start;
    }

    /// How far collapsed the bar is (Compose's `collapsedFraction`): from 0 (expanded) to 1 (collapsed); 0 without
    /// a limit.
    pub fn collapsed_fraction(&self) -> f32 {
        let limit = self.height_offset_limit();
        if limit != 0.0 { self.height_offset() / limit } else { 0.0 }
    }

    /// How far the content is under the bar (Compose's `overlappedFraction`): 1 once it scrolled by (or isn't at its
    /// start), down to 0 when it is at its top; what a bar's scrolled color follows.
    pub fn overlapped_fraction(&self) -> f32 {
        let limit = self.height_offset_limit();
        let content_offset = self.content_offset();
        if !self.is_scrolling_content_at_start() && content_offset == 0.0 {
            1.0
        } else if limit != 0.0 {
            1.0 - ((limit + content_offset.abs()).clamp(limit, 0.0) / limit)
        } else {
            0.0
        }
    }

    /// Saves its height offset limit, height offset and content offset, restored as a state of them (Compose's
    /// `TopAppBarState.Saver`).
    #[allow(non_snake_case)]
    pub fn Saver() -> Saver<TopAppBarState> {
        list_saver(
            |state: &TopAppBarState| {
                vec![state.height_offset_limit().save(), state.height_offset().save(), state.content_offset().save()]
            },
            |saved: &[SavedValue]| {
                let [limit, height, content] = saved else { return None };
                Some(TopAppBarState::new(f32::restore(limit)?, f32::restore(height)?, f32::restore(content)?))
            },
        )
    }
}

impl PartialEq for TopAppBarState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for TopAppBarState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TopAppBarState(heightOffsetLimit={}, heightOffset={}, contentOffset={})",
            self.height_offset_limit(),
            self.height_offset(),
            self.content_offset()
        )
    }
}

// ---- TopAppBarColors ---------------------------------------------------------------------------------------------

/// A top app bar's colors (Compose's `TopAppBarColors`): its container's (and scrolled under, the one it turns to
/// as content scrolls under it or it collapses), its navigation icon's, title's, actions' and subtitle's. Equal by
/// its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct TopAppBarColors {
    /// The container's color.
    pub container_color: Color,
    /// The container's color once content is scrolled under it (or, for a two-rows bar, it collapses).
    pub scrolled_container_color: Color,
    /// The navigation icon's color.
    pub navigation_icon_content_color: Color,
    /// The title's color.
    pub title_content_color: Color,
    /// The actions' color.
    pub action_icon_content_color: Color,
    /// The subtitle's color.
    pub subtitle_content_color: Color,
}

impl TopAppBarColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `TopAppBarColors(containerColor: Color, scrolledContainerColor: Color, navigationIconContentColor: Color, titleContentColor: Color, actionIconContentColor: Color, subtitleContentColor: Color)`
    pub fn new(
        container_color: Color,
        scrolled_container_color: Color,
        navigation_icon_content_color: Color,
        title_content_color: Color,
        action_icon_content_color: Color,
        subtitle_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarColors {
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            subtitle_content_color,
        }
    }

    /// Colors of one's own, the subtitle in the title's (Compose's deprecated constructor without
    /// `subtitleContentColor`).
    ///
    /// Kotlin: `TopAppBarColors(containerColor: Color, scrolledContainerColor: Color, navigationIconContentColor: Color, titleContentColor: Color, actionIconContentColor: Color)`
    #[deprecated(note = "Use the TopAppBarColors constructor with subtitleContentColor")]
    pub fn without_subtitle_content_color(
        container_color: Color,
        scrolled_container_color: Color,
        navigation_icon_content_color: Color,
        title_content_color: Color,
        action_icon_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarColors::new(
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            title_content_color,
        )
    }

    /// [`copy`](Self::copy) of every color given (an unspecified one keeps its own).
    pub(crate) fn copy_with(
        &self,
        container_color: Color,
        scrolled_container_color: Color,
        navigation_icon_content_color: Color,
        title_content_color: Color,
        action_icon_content_color: Color,
        subtitle_content_color: Color,
    ) -> TopAppBarColors {
        TopAppBarColors::new(
            container_color.take_or_else(|| self.container_color),
            scrolled_container_color.take_or_else(|| self.scrolled_container_color),
            navigation_icon_content_color.take_or_else(|| self.navigation_icon_content_color),
            title_content_color.take_or_else(|| self.title_content_color),
            action_icon_content_color.take_or_else(|| self.action_icon_content_color),
            subtitle_content_color.take_or_else(|| self.subtitle_content_color),
        )
    }

    /// The container's color `color_transition_fraction` of the way to the scrolled one, along Material's fast out
    /// linear in easing (Compose's internal `containerColor(colorTransitionFraction)`).
    pub(crate) fn container_color_at(&self, color_transition_fraction: f32) -> Color {
        lerp_color(
            self.container_color,
            self.scrolled_container_color,
            FastOutLinearInEasing.transform(color_transition_fraction),
        )
    }
}

#[named]
impl TopAppBarColors {
    /// These colors with those given in place of their own (Compose's `copy`): an unspecified one keeps its own.
    pub fn copy(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] scrolled_container_color: Color,
        #[default(Color::Unspecified)] navigation_icon_content_color: Color,
        #[default(Color::Unspecified)] title_content_color: Color,
        #[default(Color::Unspecified)] action_icon_content_color: Color,
        #[default(Color::Unspecified)] subtitle_content_color: Color,
    ) -> TopAppBarColors {
        self.copy_with(
            container_color,
            scrolled_container_color,
            navigation_icon_content_color,
            title_content_color,
            action_icon_content_color,
            subtitle_content_color,
        )
    }
}

// ---- the scroll behaviors ----------------------------------------------------------------------------------------

/// The pinned behavior (Compose's private `PinnedScrollBehavior`): the bar stays; what the content scrolls is
/// noted (while it can scroll), its offset reset to 0 when a fling down ends.
struct PinnedScrollBehavior {
    state: TopAppBarState,
    connection: Rc<PinnedConnection>,
}

impl PinnedScrollBehavior {
    fn new(
        state: TopAppBarState,
        can_scroll: Rc<dyn Fn() -> bool>,
        is_scrolling_content_at_start: Rc<dyn Fn() -> bool>,
    ) -> PinnedScrollBehavior {
        state.set_is_scrolling_content_at_start(is_scrolling_content_at_start);
        PinnedScrollBehavior { state: state.clone(), connection: Rc::new(PinnedConnection { state, can_scroll }) }
    }
}

impl TopAppBarScrollBehavior for PinnedScrollBehavior {
    fn state(&self) -> TopAppBarState {
        self.state.clone()
    }
    fn is_pinned(&self) -> bool {
        true
    }
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>> {
        None
    }
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>> {
        None
    }
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.connection.clone()
    }
}

struct PinnedConnection {
    state: TopAppBarState,
    can_scroll: Rc<dyn Fn() -> bool>,
}

impl NestedScrollConnection for PinnedConnection {
    fn on_post_scroll(&self, consumed: Offset, _available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        self.state.set_content_offset(self.state.content_offset() + consumed.y);
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        if available.y > 0.0 {
            // Reset the total content offset to zero when scrolling all the way down. This will eliminate some float
            // precision inaccuracies.
            self.state.set_content_offset(0.0);
        }
        // (the default's: nothing consumed)
        Box::pin(async { Velocity::Zero })
    }
}

/// The deprecated entering-always behavior (Compose's private `LegacyEnterAlwaysScrollBehavior`): as
/// [`EnterAlwaysScrollBehavior`], over a reversed content taking nothing before it nor following its scrolls.
struct LegacyEnterAlwaysScrollBehavior {
    state: TopAppBarState,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    connection: Rc<LegacyEnterAlwaysConnection>,
}

impl LegacyEnterAlwaysScrollBehavior {
    fn new(
        state: TopAppBarState,
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        can_scroll: Rc<dyn Fn() -> bool>,
        reverse_layout: bool,
    ) -> LegacyEnterAlwaysScrollBehavior {
        let connection = Rc::new(LegacyEnterAlwaysConnection {
            state: state.clone(),
            snap_animation_spec: snap_animation_spec.clone(),
            fling_animation_spec: fling_animation_spec.clone(),
            can_scroll,
            reverse_layout,
        });
        LegacyEnterAlwaysScrollBehavior { state, snap_animation_spec, fling_animation_spec, connection }
    }
}

impl TopAppBarScrollBehavior for LegacyEnterAlwaysScrollBehavior {
    fn state(&self) -> TopAppBarState {
        self.state.clone()
    }
    fn is_pinned(&self) -> bool {
        false
    }
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>> {
        self.snap_animation_spec.clone()
    }
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>> {
        self.fling_animation_spec.clone()
    }
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.connection.clone()
    }
}

struct LegacyEnterAlwaysConnection {
    state: TopAppBarState,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    can_scroll: Rc<dyn Fn() -> bool>,
    reverse_layout: bool,
}

impl NestedScrollConnection for LegacyEnterAlwaysConnection {
    fn on_pre_scroll(&self, available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        let prev_height_offset = self.state.height_offset();
        self.state.set_height_offset(self.state.height_offset() + available.y);
        // The state's heightOffset is coerce in a minimum value of heightOffsetLimit and a maximum value 0f, so we
        // check if its value was actually changed after the available.y was added to it in order to tell if the top
        // app bar is currently collapsing or expanding.
        // Note that when the content was set with a revered layout, we always return a zero offset.
        if !self.reverse_layout && prev_height_offset != self.state.height_offset() {
            // We're in the middle of top app bar collapse or expand.
            // Consume only the scroll on the Y axis.
            Offset::new(0.0, available.y)
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, consumed: Offset, _available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        self.state.set_content_offset(self.state.content_offset() + consumed.y);
        if !self.reverse_layout {
            self.state.set_height_offset(self.state.height_offset() + consumed.y);
        }
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            let state = &self.state;
            if available.y > 0.0
                && (state.height_offset() == 0.0 || state.height_offset() == state.height_offset_limit())
            {
                // Reset the total content offset to zero when scrolling all the way down. This will eliminate some
                // float precision inaccuracies.
                state.set_content_offset(0.0);
            }
            // (the default's: nothing consumed)
            let super_consumed = Velocity::Zero;
            super_consumed
                + settle_app_bar(
                    state,
                    available.y,
                    self.fling_animation_spec.clone(),
                    self.snap_animation_spec.clone(),
                )
                .await
        })
    }
}

/// The entering-always behavior (Compose's private `EnterAlwaysScrollBehavior`): the bar takes the content's
/// scrolls before it does (collapsing up, coming back down at once), notes what the content scrolls, settles after a
/// fling.
struct EnterAlwaysScrollBehavior {
    state: TopAppBarState,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    connection: Rc<EnterAlwaysConnection>,
}

impl EnterAlwaysScrollBehavior {
    fn new(
        state: TopAppBarState,
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        can_scroll: Rc<dyn Fn() -> bool>,
        is_scrolling_content_at_start: Rc<dyn Fn() -> bool>,
    ) -> EnterAlwaysScrollBehavior {
        state.set_is_scrolling_content_at_start(is_scrolling_content_at_start);
        let connection = Rc::new(EnterAlwaysConnection {
            state: state.clone(),
            snap_animation_spec: snap_animation_spec.clone(),
            fling_animation_spec: fling_animation_spec.clone(),
            can_scroll,
        });
        EnterAlwaysScrollBehavior { state, snap_animation_spec, fling_animation_spec, connection }
    }
}

impl TopAppBarScrollBehavior for EnterAlwaysScrollBehavior {
    fn state(&self) -> TopAppBarState {
        self.state.clone()
    }
    fn is_pinned(&self) -> bool {
        false
    }
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>> {
        self.snap_animation_spec.clone()
    }
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>> {
        self.fling_animation_spec.clone()
    }
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.connection.clone()
    }
}

struct EnterAlwaysConnection {
    state: TopAppBarState,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    can_scroll: Rc<dyn Fn() -> bool>,
}

impl NestedScrollConnection for EnterAlwaysConnection {
    fn on_pre_scroll(&self, available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        let prev_height_offset = self.state.height_offset();
        self.state.set_height_offset(self.state.height_offset() + available.y);
        // The state's heightOffset is coerce in a minimum value of heightOffsetLimit and a maximum value 0f, so we
        // check if its value was actually changed after the available.y was added to it in order to tell if the top
        // app bar is currently collapsing or expanding.
        if prev_height_offset != self.state.height_offset() {
            // We're in the middle of top app bar collapse or expand.
            // Consume only the scroll on the Y axis.
            Offset::new(0.0, available.y)
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, consumed: Offset, _available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        self.state.set_content_offset(self.state.content_offset() + consumed.y);
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            if available.y > 0.0 {
                // Reset the total content offset to zero when scrolling all the way down. This will eliminate some
                // float precision inaccuracies.
                self.state.set_content_offset(0.0);
            }
            // (the default's: nothing consumed)
            let super_consumed = Velocity::Zero;
            super_consumed
                + settle_app_bar(
                    &self.state,
                    available.y,
                    self.fling_animation_spec.clone(),
                    self.snap_animation_spec.clone(),
                )
                .await
        })
    }
}

/// The exiting-until-collapsed behavior (Compose's private `ExitUntilCollapsedScrollBehavior`): the bar takes the
/// content's scrolls up before it does, and down only what the content leaves (once it is at its top); settles
/// after a fling.
struct ExitUntilCollapsedScrollBehavior {
    state: TopAppBarState,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    connection: Rc<ExitUntilCollapsedConnection>,
}

impl ExitUntilCollapsedScrollBehavior {
    fn new(
        state: TopAppBarState,
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        can_scroll: Rc<dyn Fn() -> bool>,
    ) -> ExitUntilCollapsedScrollBehavior {
        let connection = Rc::new(ExitUntilCollapsedConnection {
            state: state.clone(),
            snap_animation_spec: snap_animation_spec.clone(),
            fling_animation_spec: fling_animation_spec.clone(),
            can_scroll,
        });
        ExitUntilCollapsedScrollBehavior { state, snap_animation_spec, fling_animation_spec, connection }
    }
}

impl TopAppBarScrollBehavior for ExitUntilCollapsedScrollBehavior {
    fn state(&self) -> TopAppBarState {
        self.state.clone()
    }
    fn is_pinned(&self) -> bool {
        false
    }
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>> {
        self.snap_animation_spec.clone()
    }
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>> {
        self.fling_animation_spec.clone()
    }
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.connection.clone()
    }
}

struct ExitUntilCollapsedConnection {
    state: TopAppBarState,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    can_scroll: Rc<dyn Fn() -> bool>,
}

impl NestedScrollConnection for ExitUntilCollapsedConnection {
    fn on_pre_scroll(&self, available: Offset, _source: NestedScrollSource) -> Offset {
        // Don't intercept if scrolling down.
        if !(self.can_scroll)() || available.y > 0.0 {
            return Offset::Zero;
        }

        let prev_height_offset = self.state.height_offset();
        self.state.set_height_offset(self.state.height_offset() + available.y);
        if prev_height_offset != self.state.height_offset() {
            // We're in the middle of top app bar collapse or expand.
            // Consume only the scroll on the Y axis.
            Offset::new(0.0, available.y)
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, consumed: Offset, available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        self.state.set_content_offset(self.state.content_offset() + consumed.y);

        if available.y < 0.0 || consumed.y < 0.0 {
            // When scrolling up, just update the state's height offset.
            let old_height_offset = self.state.height_offset();
            self.state.set_height_offset(self.state.height_offset() + consumed.y);
            return Offset::new(0.0, self.state.height_offset() - old_height_offset);
        }

        if available.y > 0.0 {
            // Adjust the height offset in case the consumed delta Y is less than what was recorded as available delta
            // Y in the pre-scroll.
            let old_height_offset = self.state.height_offset();
            self.state.set_height_offset(self.state.height_offset() + available.y);
            return Offset::new(0.0, self.state.height_offset() - old_height_offset);
        }
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            if available.y > 0.0 {
                // Reset the total content offset to zero when scrolling all the way down. This will eliminate some
                // float precision inaccuracies.
                self.state.set_content_offset(0.0);
            }
            // (the default's: nothing consumed)
            let super_consumed = Velocity::Zero;
            super_consumed
                + settle_app_bar(
                    &self.state,
                    available.y,
                    self.fling_animation_spec.clone(),
                    self.snap_animation_spec.clone(),
                )
                .await
        })
    }
}

/// Settles a bar left in between (Compose's private `settleAppBar`): a fling's `velocity` carried on along
/// `fling_animation_spec` (stopping where the bar takes no more of it), then a snap by `snap_animation_spec` to
/// expanded (less than half collapsed) or collapsed. Nothing when it is (all but) expanded or collapsed. The
/// velocity left.
pub(crate) async fn settle_app_bar(
    state: &TopAppBarState,
    velocity: f32,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
) -> Velocity {
    // Check if the app bar is completely collapsed/expanded. If so, no need to settle the app bar, and just return
    // Zero Velocity.
    // Note that we don't check for 0f due to float precision with the collapsedFraction calculation.
    if state.collapsed_fraction() < 0.01 || state.collapsed_fraction() == 1.0 {
        return Velocity::Zero;
    }
    let remaining_velocity = Rc::new(Cell::new(velocity));
    // In case there is an initial velocity that was left after a previous user fling, animate to continue the motion
    // to expand or collapse the app bar.
    if let Some(fling_animation_spec) = fling_animation_spec
        && velocity.abs() > 1.0
    {
        let last_value = Cell::new(0.0f32);
        let (st, remaining) = (state.clone(), remaining_velocity.clone());
        let block = move |scope: &AnimationScope<f32, AnimationVector1D>| {
            let delta = scope.value() - last_value.get();
            let initial_height_offset = st.height_offset();
            st.set_height_offset(initial_height_offset + delta);
            let consumed = (initial_height_offset - st.height_offset()).abs();
            last_value.set(scope.value());
            remaining.set(scope.velocity());
            // avoid rounding errors and stop if anything is unconsumed
            if (delta - consumed).abs() > 0.5 {
                scope.cancel_animation();
            }
        };
        named_args!(
            float_animation_state(0.0, initial_velocity = velocity)
                .animate_decay(fling_animation_spec, block = Box::new(block))
        )
        .await;
    }
    // Snap if animation specs were provided.
    if let Some(snap_animation_spec) = snap_animation_spec
        && state.height_offset() < 0.0
        && state.height_offset() > state.height_offset_limit()
    {
        let target = if state.collapsed_fraction() < 0.5 { 0.0 } else { state.height_offset_limit() };
        let st = state.clone();
        let block = move |scope: &AnimationScope<f32, AnimationVector1D>| st.set_height_offset(scope.value());
        named_args!(float_animation_state(state.height_offset()).animate_to(
            target,
            animation_spec = snap_animation_spec,
            block = Box::new(block)
        ))
        .await;
    }

    Velocity::new(0.0, remaining_velocity.get())
}
