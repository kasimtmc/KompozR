//! `HorizontalPager` and `VerticalPager` (Compose's Pager.kt): pages of a page size laid out lazily, snapping to
//! a page when let go; [`PagerDefaults`] (the snapping fling, the pages' nested scroll connection); the pager's
//! paging semantics.

use super::layout_info::{PageSize, PagerSnapDistance};
use super::lazy_layout_pager::Pager;
use super::state::{PagerState, animate_to_next_page, animate_to_previous_page};
use crate::gestures::snapping::{
    SnapPosition, calculate_final_snapping_bound, pager_snap_layout_info_provider, snap_fling_behavior,
};
use crate::gestures::{Orientation, ScrollScope, ScrollableDefaults, ScrollableState, TargetedFlingBehavior};
use crate::layout::PaddingValues;
use crate::lazy::layout::LazyKey;
use crate::overscroll::{OverscrollEffect, remember_overscroll_effect};
use kompoz_animation::{
    AnimationSpec, DecayAnimationSpec, MutatePriority, Spring, SpringSpec, remember_spline_based_decay,
};
use kompoz_coroutines::{CoroutineScope, current_job};
use kompoz_graphics::{Dp, LayoutDirection, Offset};
use kompoz_runtime::{kompozable, kompozable_block, named, remember_keyed};
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollSource};
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Alignment, HorizontalAlignment, Modifier, VerticalAlignment};
use std::rc::Rc;
use std::sync::Arc;

/// A pager turning horizontally (Compose's `HorizontalPager`): `page_content` of each page (given its index),
/// composed and laid out only for the pages in view (and `beyond_viewport_page_count` more each side), `page_size`
/// long (the whole pager by default) with `page_spacing` between; `state` tells and sets the current page; let go,
/// it snaps a page to `snap_position` (`fling_behavior`: [`PagerDefaults::fling_behavior`]); `content_padding`
/// pads the pages, `vertical_alignment` aligns them across; `reverse_layout` puts page 0 at the end;
/// `user_scroll_enabled`; `key` keeps a page's state and its place as pages change; `page_nested_scroll_connection`
/// is what scrolls inside a page meet first ([`PagerDefaults::page_nested_scroll_connection`]); `overscroll_effect`
/// (`None`: `remember_overscroll_effect()`; `Some(None)`: none).
#[kompozable]
pub fn HorizontalPager(
    state: PagerState,
    #[default(Modifier)] modifier: Modifier,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(PageSize::Fill)] page_size: PageSize,
    #[default(PagerDefaults::BeyondViewportPageCount)] beyond_viewport_page_count: i32,
    #[default(Dp(0.0))] page_spacing: Dp,
    #[default(Alignment::CenterVertically)] vertical_alignment: VerticalAlignment,
    #[default(None)] fling_behavior: Option<Rc<dyn TargetedFlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(false)] reverse_layout: bool,
    #[default(None)] key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    #[default(None)] page_nested_scroll_connection: Option<Rc<dyn NestedScrollConnection>>,
    #[default(SnapPosition::Start)] snap_position: SnapPosition,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    page_content: impl Fn(i32) + 'static,
) {
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => PagerDefaults.fling_behavior(&state),
    };
    let page_nested_scroll_connection = match page_nested_scroll_connection {
        Some(c) => c,
        None => PagerDefaults.page_nested_scroll_connection(&state, Orientation::Horizontal),
    };
    let overscroll_effect = match overscroll_effect {
        Some(e) => e,
        None => remember_overscroll_effect(),
    };
    Pager(
        modifier,
        state,
        content_padding,
        reverse_layout,
        Orientation::Horizontal,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        beyond_viewport_page_count,
        page_spacing,
        page_size,
        page_nested_scroll_connection,
        key,
        Alignment::CenterHorizontally,
        vertical_alignment,
        snap_position,
        Rc::new(page_content),
    );
}

/// A pager turning vertically (Compose's `VerticalPager`): as [`HorizontalPager`], along its height;
/// `horizontal_alignment` aligns the pages across.
#[kompozable]
pub fn VerticalPager(
    state: PagerState,
    #[default(Modifier)] modifier: Modifier,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(PageSize::Fill)] page_size: PageSize,
    #[default(PagerDefaults::BeyondViewportPageCount)] beyond_viewport_page_count: i32,
    #[default(Dp(0.0))] page_spacing: Dp,
    #[default(Alignment::CenterHorizontally)] horizontal_alignment: HorizontalAlignment,
    #[default(None)] fling_behavior: Option<Rc<dyn TargetedFlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(false)] reverse_layout: bool,
    #[default(None)] key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    #[default(None)] page_nested_scroll_connection: Option<Rc<dyn NestedScrollConnection>>,
    #[default(SnapPosition::Start)] snap_position: SnapPosition,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    page_content: impl Fn(i32) + 'static,
) {
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => PagerDefaults.fling_behavior(&state),
    };
    let page_nested_scroll_connection = match page_nested_scroll_connection {
        Some(c) => c,
        None => PagerDefaults.page_nested_scroll_connection(&state, Orientation::Vertical),
    };
    let overscroll_effect = match overscroll_effect {
        Some(e) => e,
        None => remember_overscroll_effect(),
    };
    Pager(
        modifier,
        state,
        content_padding,
        reverse_layout,
        Orientation::Vertical,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        beyond_viewport_page_count,
        page_spacing,
        page_size,
        page_nested_scroll_connection,
        key,
        horizontal_alignment,
        Alignment::CenterVertically,
        snap_position,
        Rc::new(page_content),
    );
}

/// A pager's defaults (Compose's PagerDefaults): its composables are methods of the value, as Kotlin calls them
/// (`PagerDefaults.fling_behavior(&state)`).
#[derive(Clone, Copy)]
pub struct PagerDefaults;

impl PagerDefaults {
    /// No pages composed beyond the viewport by default (Compose's `BeyondViewportPageCount`).
    pub const BeyondViewportPageCount: i32 = 0;
}

/// The default snap animation (Compose's `spring(stiffness = StiffnessMediumLow, visibilityThreshold = 1)`).
fn default_snap_animation_spec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(SpringSpec::<f32>::new(Spring::DampingRatioNoBouncy, Spring::StiffnessMediumLow, Some(1.0)))
}

#[named]
impl PagerDefaults {
    /// The pager's snapping fling (Compose's composable `PagerDefaults.flingBehavior`): as far as the fling's
    /// velocity carries it (decaying along `decay_animation_spec`; `None`: the remembered spline decay), at most
    /// `pager_snap_distance` pages; a slow release past `snap_positional_threshold` of a page (0 to 1) turns it;
    /// it snaps along `snap_animation_spec`.
    /// Kotlin: `flingBehavior(state: PagerState, pagerSnapDistance: PagerSnapDistance = PagerSnapDistance.atMost(1), decayAnimationSpec: DecayAnimationSpec<Float> = rememberSplineBasedDecay(), snapAnimationSpec: AnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = Int.VisibilityThreshold.toFloat()), snapPositionalThreshold: Float = 0.5f)`
    #[track_caller]
    pub fn fling_behavior(
        &self,
        state: &PagerState,
        #[default(PagerSnapDistance::at_most(1))] pager_snap_distance: PagerSnapDistance,
        #[default(None)] decay_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        #[default(default_snap_animation_spec())] snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
        #[default(0.5)] snap_positional_threshold: f32,
    ) -> Rc<dyn TargetedFlingBehavior> {
        assert!(
            (0.0..=1.0).contains(&snap_positional_threshold),
            "snapPositionalThreshold should be a number between 0 and 1. You've specified {snap_positional_threshold}"
        );
        let state = state.clone();
        kompozable_block!({
            let decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>> = match decay_animation_spec {
                Some(spec) => spec,
                None => Arc::new(remember_spline_based_decay::<f32>()),
            };
            let density = LocalDensity.current();
            let layout_direction = LocalLayoutDirection.current();
            let keys = (
                state.clone(),
                Arc::as_ptr(&decay_animation_spec) as *const () as usize,
                Arc::as_ptr(&snap_animation_spec) as *const () as usize,
                pager_snap_distance.clone(),
                density.density.to_bits(),
                layout_direction,
            );
            remember_keyed(keys, move || {
                let s = state.clone();
                let snap_layout_info_provider = pager_snap_layout_info_provider(
                    state,
                    pager_snap_distance,
                    Rc::new(move |fling_velocity, lower_bound, upper_bound| {
                        calculate_final_snapping_bound(
                            &s,
                            layout_direction,
                            snap_positional_threshold,
                            fling_velocity,
                            lower_bound,
                            upper_bound,
                        )
                    }),
                );
                snap_fling_behavior(snap_layout_info_provider, decay_animation_spec, snap_animation_spec)
            })
        })
    }

    /// The nested scroll connection of a pager's pages (Compose's composable
    /// `PagerDefaults.pageNestedScrollConnection`): a drag inside a page while the pager is between two pages
    /// moves the pager back to a page first; a fling's scroll it can't take cancels the fling; what velocity an
    /// inner fling leaves along the pager doesn't go on to the pager's parents — and settles the pager when it is
    /// between two pages (the C++ Kompoz's behaviour, kept: Compose's connection takes the velocity, so the pager
    /// then settles as if let go slowly).
    #[track_caller]
    pub fn page_nested_scroll_connection(
        &self,
        state: &PagerState,
        orientation: Orientation,
    ) -> Rc<dyn NestedScrollConnection> {
        let state = state.clone();
        kompozable_block!({
            let layout_direction = LocalLayoutDirection.current();
            remember_keyed((state.clone(), orientation, layout_direction), move || {
                Rc::new(DefaultPagerNestedScrollConnection { state, orientation, layout_direction })
                    as Rc<dyn NestedScrollConnection>
            })
        })
    }
}

/// Compose's DefaultPagerNestedScrollConnection (its `onPostFling` the C++ Kompoz's: see
/// [`PagerDefaults::page_nested_scroll_connection`]).
struct DefaultPagerNestedScrollConnection {
    state: PagerState,
    orientation: Orientation,
    layout_direction: LayoutDirection,
}

impl DefaultPagerNestedScrollConnection {
    fn main_axis(&self, o: Offset) -> f32 {
        if self.orientation == Orientation::Horizontal { o.x } else { o.y }
    }

    fn consume_on_orientation(&self, v: Velocity) -> Velocity {
        if self.orientation == Orientation::Vertical { Velocity::new(0.0, v.y) } else { Velocity::new(v.x, 0.0) }
    }
}

impl NestedScrollConnection for DefaultPagerNestedScrollConnection {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        let state = &self.state;
        let fraction = state.current_page_offset_fraction();
        // (a drag, the pager between pages, along it)
        if source == NestedScrollSource::UserInput && fraction.abs() > 1e-6 && self.main_axis(available).abs() > 0.0 {
            // The current page and the next one the drag's way.
            let layout_info = state.layout_info();
            let current_page_offset = fraction * state.page_size() as f32;
            let page_available_space = (layout_info.page_size + layout_info.page_spacing) as f32;
            let next_closest_page_offset = current_page_offset + page_available_space * -sign(fraction);
            // (the bounds the nested scroll can go to)
            let (min_bound, max_bound) = if fraction > 0.0 {
                (next_closest_page_offset, current_page_offset)
            } else {
                (current_page_offset, next_closest_page_offset)
            };
            let delta = self.main_axis(available);
            let coerced = delta.clamp(min_bound.min(max_bound), max_bound.max(min_bound));
            // (dispatched and given back reversed; a reversed horizontal pager the other way: Compose's
            // isReverseLayoutNestedScrollConnectionInPagerFixEnabled)
            let consumed = if self.orientation == Orientation::Horizontal
                && ((self.layout_direction == LayoutDirection::Rtl) ^ layout_info.reverse_layout)
            {
                state.dispatch_raw_delta(coerced)
            } else {
                -state.dispatch_raw_delta(-coerced)
            };
            Offset::new(
                if self.orientation == Orientation::Horizontal { consumed } else { available.x },
                if self.orientation == Orientation::Vertical { consumed } else { available.y },
            )
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, _consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        if source == NestedScrollSource::SideEffect && self.main_axis(available) != 0.0 {
            // (Compose throws "Scroll cancelled": the fling scrolling is cancelled, at its next wait)
            current_job().cancel();
        }
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            // The C++ Kompoz's behaviour (kept, user-requested): the pager, between two pages, settles with what
            // the inner fling left — flicked past a list's end, the page turns. Compose's connection only takes
            // the velocity (the pager then settles slowly, back to its page if not dragged past its threshold).
            let state = &self.state;
            let fling = state.inner.fling_behavior.borrow().clone();
            if let Some(fling) = fling
                && state.current_page_offset_fraction().abs() > 1e-6
            {
                let finger_velocity =
                    if self.orientation == Orientation::Horizontal { available.x } else { available.y };
                // (the finger's velocity as the pager's scrollable flings it)
                let reverse = ScrollableDefaults::reverse_direction(
                    self.layout_direction,
                    self.orientation,
                    state.layout_info().reverse_layout,
                );
                let velocity = if reverse { -finger_velocity } else { finger_velocity };
                state
                    .scroll_block(
                        MutatePriority::Default,
                        Box::new(move |scope: Rc<dyn ScrollScope>| {
                            Box::pin(async move {
                                fling.perform_fling(scope, velocity).await;
                            })
                        }),
                    )
                    .await;
            }
            self.consume_on_orientation(available)
        })
    }
}

fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// The pager's paging actions (Compose's `pagerSemantics`): page up/down (left/right) animate to the previous or
/// next page while it can scroll that way.
pub(crate) fn pager_semantics(
    modifier: Modifier,
    state: &PagerState,
    is_vertical: bool,
    scope: CoroutineScope,
    user_scroll_enabled: bool,
) -> Modifier {
    if !user_scroll_enabled {
        return modifier;
    }
    let state = state.clone();
    let forward: Rc<dyn Fn() -> bool> = {
        let (state, scope) = (state.clone(), scope.clone());
        Rc::new(move || {
            if state.can_scroll_forward() {
                let s = state.clone();
                scope.launch(async move { animate_to_next_page(&s).await });
                true
            } else {
                false
            }
        })
    };
    let backward: Rc<dyn Fn() -> bool> = Rc::new(move || {
        if state.can_scroll_backward() {
            let s = state.clone();
            scope.launch(async move { animate_to_previous_page(&s).await });
            true
        } else {
            false
        }
    });
    modifier.then(Modifier.semantics(move |config| {
        let (f, b) = (forward.clone(), backward.clone());
        if is_vertical {
            config.page_up(move || b());
            config.page_down(move || f());
        } else {
            config.page_left(move || b());
            config.page_right(move || f());
        }
    }))
}
