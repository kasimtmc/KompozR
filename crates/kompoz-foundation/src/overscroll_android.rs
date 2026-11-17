//! Android's stretch overscroll, as Compose's AndroidOverscroll.android.kt and EdgeEffectCompat.android.kt on
//! Android 12 and later: an [`EdgeEffect`] per edge takes what the scrolls leave (pulled, the finger's place along
//! the edge noted), relaxes before the content scrolls back, absorbs what the flings leave and springs back; the
//! node draws the content through a layer stretched as the effects say (hwui's stretch, [`GraphicsLayer::stretch`]).
//! It is KompozR's overscroll on every platform (the C++ Kompoz's, stage 10's decision D6); Compose's glow (before
//! Android 12) isn't ported.
//!
//! Deviations: Compose negates the last frame's stretch with mirrored effects because RenderNode.clearStretch is
//! hidden API; KompozR's layer has [`GraphicsLayer::clear_stretch`], so the node clears and stretches each frame as
//! Android's views do (the same stretch shows). The layer is the node's size widened across by
//! [`MaxSupportedElevation`] (Compose's RenderNode is its canvas', which the scroll container's clip layer makes
//! the node's size).

use crate::clip_scrollable_container::MaxSupportedElevation;
use crate::edge_effect::EdgeEffect;
use crate::gestures::{AwaitPointerEventScopeGestures, LocalFuture, PointerInputScopeGestures};
use crate::layout::PaddingValues;
use crate::overscroll::{OverscrollEffect, OverscrollFactory, PerformFling};
use kompoz_animation::PlatformFlingScrollFriction;
use kompoz_graphics::{Color, Density, IntOffset, IntSize, Offset, Size};
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalAccessorScope, MutableState, kompozable, mutable_state_of_with_policy, named,
    never_equal_policy, remember_keyed,
};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, GraphicsLayer};
use kompoz_ui::input::{
    NestedScrollSource, PointerEventPass, PointerId, PointerInputScope, SuspendingPointerInputModifierNode,
};
use kompoz_ui::modifier::DelegatableNode;
use kompoz_ui::platform::{LocalDensity, LocalPlatform};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{ModifierNode, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Android's EdgeEffect's default color, the glow's (EdgeEffect's `0xff666666`; Compose's DefaultGlowColor).
const DefaultGlowColor: Color = Color::from_argb(0xff666666);

/// How much faster a fling relaxes a stretch than a drag does (Compose's FlingDestretchFactor).
const FlingDestretchFactor: f32 = 4.0;

// ---------------------------------------------------------------------------------------------------------------
// The configuration and the factory

/// The deprecated way of configuring Android's overscroll (Compose's OverscrollConfiguration): the glow's color
/// and padding (KompozR's overscroll is the stretch, which has neither). Provided as none through
/// [`LocalOverscrollConfiguration`] it turns the default overscroll off; Compose replaced it with
/// [`LocalOverscrollFactory`](crate::overscroll::LocalOverscrollFactory) and
/// [`remember_platform_overscroll_factory`].
#[derive(Clone, PartialEq, Debug)]
pub struct OverscrollConfiguration {
    pub glow_color: Color,
    pub draw_padding: PaddingValues,
}

/// Compose's `OverscrollConfiguration(glowColor, drawPadding)`.
#[named(type)]
pub fn OverscrollConfiguration(
    #[default(DefaultGlowColor)] glow_color: Color,
    #[default(PaddingValues::all(kompoz_graphics::Dp(0.0)))] draw_padding: PaddingValues,
) -> OverscrollConfiguration {
    OverscrollConfiguration { glow_color, draw_padding }
}

/// The deprecated configuration local (Compose's LocalOverscrollConfiguration): none turns the default overscroll
/// off.
pub static LocalOverscrollConfiguration: KompozitionLocal<Option<OverscrollConfiguration>> =
    KompozitionLocal::new(|| {
        Some(OverscrollConfiguration {
            glow_color: DefaultGlowColor,
            draw_padding: PaddingValues::all(kompoz_graphics::Dp(0.0)),
        })
    });

/// What [`LocalOverscrollFactory`](crate::overscroll::LocalOverscrollFactory) gives where nothing provides one
/// (Compose's Android `defaultOverscrollFactory`): Android's, for the density there, unless the deprecated
/// configuration is none.
pub(crate) fn default_overscroll_factory(scope: &KompozitionLocalAccessorScope) -> Option<Rc<dyn OverscrollFactory>> {
    let density = scope.current_value(&LocalDensity);
    let config = scope.current_value(&LocalOverscrollConfiguration)?;
    Some(Rc::new(AndroidEdgeEffectOverscrollFactory {
        density,
        glow_color: config.glow_color,
        glow_draw_padding: config.draw_padding,
    }))
}

/// Android's overscroll factory for this density (Compose's `rememberPlatformOverscrollFactory`): `glow_color` and
/// `glow_draw_padding` are the glow's (before Android 12; KompozR's stretch has neither), kept for Compose's API.
#[kompozable]
pub fn remember_platform_overscroll_factory(
    #[default(DefaultGlowColor)] glow_color: Color,
    #[default(PaddingValues::all(kompoz_graphics::Dp(0.0)))] glow_draw_padding: PaddingValues,
) -> Rc<dyn OverscrollFactory> {
    let density = LocalDensity.current();
    Rc::new(AndroidEdgeEffectOverscrollFactory { density, glow_color, glow_draw_padding })
}

/// The deprecated `ScrollableDefaults.overscrollEffect()`'s effect (Compose's `rememberPlatformOverscrollEffect`):
/// Android's, remembered for this density and configuration; none where the configuration is none.
#[kompozable]
pub(crate) fn remember_platform_overscroll_effect() -> Option<Rc<dyn OverscrollEffect>> {
    let density = LocalDensity.current();
    let config = LocalOverscrollConfiguration.current()?;
    let key = (density, config.clone());
    Some(remember_keyed(key, move || {
        Rc::new(AndroidEdgeEffectOverscrollEffect::new(density)) as Rc<dyn OverscrollEffect>
    }))
}

/// Android's overscroll factory (Compose's AndroidEdgeEffectOverscrollFactory): equal to another of the same
/// density and glow.
#[derive(PartialEq)]
struct AndroidEdgeEffectOverscrollFactory {
    density: Density,
    glow_color: Color,
    glow_draw_padding: PaddingValues,
}

impl OverscrollFactory for AndroidEdgeEffectOverscrollFactory {
    fn create_overscroll_effect(&self) -> Rc<dyn OverscrollEffect> {
        Rc::new(AndroidEdgeEffectOverscrollEffect::new(self.density))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The effect

/// Android's overscroll effect (Compose's AndroidEdgeEffectOverscrollEffect): a handle; clones are the same effect,
/// equal to it only.
#[derive(Clone)]
pub struct AndroidEdgeEffectOverscrollEffect {
    inner: Rc<EffectInner>,
}

impl PartialEq for AndroidEdgeEffectOverscrollEffect {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

struct EffectInner {
    density: Density,
    pointer_position: Cell<Offset>,
    pointer_id: Cell<PointerId>,
    edge_effect_wrapper: RefCell<EdgeEffectWrapper>,
    /// Read where it is drawn: set, it is drawn again (Compose's `redrawSignal`).
    redraw_signal: MutableState<()>,
    invalidation_enabled: Cell<bool>,
    scroll_cycle_in_progress: Cell<bool>,
    container_size: Cell<Size>,
    /// Its node's handle while attached: the host's clock.
    handle: RefCell<Option<NodeHandle>>,
    node: RefCell<Option<DelegatableNode>>,
}

impl AndroidEdgeEffectOverscrollEffect {
    /// An effect for a screen of `density` (what its absorbed flings are measured with).
    pub fn new(density: Density) -> AndroidEdgeEffectOverscrollEffect {
        let inner = Rc::new(EffectInner {
            density,
            pointer_position: Cell::new(Offset::Unspecified),
            pointer_id: Cell::new(PointerId(-1)),
            edge_effect_wrapper: RefCell::new(EdgeEffectWrapper::default()),
            redraw_signal: mutable_state_of_with_policy((), never_equal_policy()),
            invalidation_enabled: Cell::new(true),
            scroll_cycle_in_progress: Cell::new(false),
            container_size: Cell::new(Size::Zero),
            handle: RefCell::new(None),
            node: RefCell::new(None),
        });
        let node = DelegatableNode::new(StretchOverscrollNode {
            effect: Rc::downgrade(&inner),
            layer: None,
            delegated_pointer_input: false,
        });
        *inner.node.borrow_mut() = Some(node);
        AndroidEdgeEffectOverscrollEffect { inner }
    }

    /// How far each edge is stretched now (top, bottom, left, right: fractions of the size), for tests.
    #[doc(hidden)]
    pub fn distances(&self) -> [f32; 4] {
        let w = self.inner.edge_effect_wrapper.borrow();
        let d = |e: &Option<EdgeEffect>| e.as_ref().map_or(0.0, |e| e.distance());
        [d(&w.top_effect), d(&w.bottom_effect), d(&w.left_effect), d(&w.right_effect)]
    }
}

impl EffectInner {
    /// The host's clock, ms (Android's `AnimationUtils.currentAnimationTimeMillis`).
    fn now(&self) -> i64 {
        self.handle.borrow().as_ref().and_then(|h| h.dispatcher()).map_or(0, |d| d.now() / 1_000_000)
    }

    /// Compose's `invalidateOverscroll`.
    fn invalidate_overscroll(&self) {
        if self.invalidation_enabled.get() {
            self.redraw_signal.set(());
        }
    }

    /// Compose's `updateSize`.
    fn update_size(&self, size: Size) {
        let initial_set_size = self.container_size.get() == Size::Zero;
        let different_size = size != self.container_size.get();
        self.container_size.set(size);
        if different_size {
            self.edge_effect_wrapper
                .borrow_mut()
                .update_size(IntSize::new(size.width.round() as i32, size.height.round() as i32));
        }
        if !initial_set_size && different_size {
            self.animate_to_release_if_needed();
        }
    }

    /// Where the pointer is along the size (its middle when none is down: Compose's `displacement`).
    fn displacement(&self) -> Offset {
        let size = self.container_size.get();
        let p = self.pointer_position.get();
        let pointer = if p.is_specified() { p } else { size.center() };
        Offset::new(pointer.x / size.width, pointer.y / size.height)
    }

    /// Compose's `animateToReleaseIfNeeded`.
    fn animate_to_release_if_needed(&self) {
        let now = self.now();
        let mut needs_invalidation = false;
        self.edge_effect_wrapper.borrow_mut().for_each_effect(|e| {
            e.on_release(now);
            needs_invalidation = !e.is_finished() || needs_invalidation;
        });
        if needs_invalidation {
            self.invalidate_overscroll();
        }
    }

    /// Compose's `releaseOppositeOverscroll` (a stretch's only: its `onReleaseWithOppositeDelta` is `onRelease`).
    fn release_opposite_overscroll(&self, delta: Offset) -> bool {
        let now = self.now();
        let mut w = self.edge_effect_wrapper.borrow_mut();
        let mut needs_invalidation = false;
        if w.is_left_animating() && delta.x < 0.0 {
            w.get_or_create_left_effect().on_release(now);
            needs_invalidation = w.is_left_animating();
        }
        if w.is_right_animating() && delta.x > 0.0 {
            w.get_or_create_right_effect().on_release(now);
            needs_invalidation = needs_invalidation || w.is_right_animating();
        }
        if w.is_top_animating() && delta.y < 0.0 {
            w.get_or_create_top_effect().on_release(now);
            needs_invalidation = needs_invalidation || w.is_top_animating();
        }
        if w.is_bottom_animating() && delta.y > 0.0 {
            w.get_or_create_bottom_effect().on_release(now);
            needs_invalidation = needs_invalidation || w.is_bottom_animating();
        }
        needs_invalidation
    }

    fn pull_top(&self, scroll: Offset) -> f32 {
        let displacement_x = self.displacement().x;
        let height = self.container_size.get().height;
        let pull_y = scroll.y / height;
        let now = self.now();
        let mut w = self.edge_effect_wrapper.borrow_mut();
        let top_effect = w.get_or_create_top_effect();
        let consumed = top_effect.on_pull_distance(pull_y, displacement_x, now) * height;
        // Showing: all of the scroll taken, given back as it was (no float rounding: b/265363356).
        if top_effect.distance() != 0.0 { scroll.y } else { consumed }
    }

    fn pull_bottom(&self, scroll: Offset) -> f32 {
        let displacement_x = self.displacement().x;
        let height = self.container_size.get().height;
        let pull_y = scroll.y / height;
        let now = self.now();
        let mut w = self.edge_effect_wrapper.borrow_mut();
        let bottom_effect = w.get_or_create_bottom_effect();
        let consumed = -bottom_effect.on_pull_distance(-pull_y, 1.0 - displacement_x, now) * height;
        if bottom_effect.distance() != 0.0 { scroll.y } else { consumed }
    }

    fn pull_left(&self, scroll: Offset) -> f32 {
        let displacement_y = self.displacement().y;
        let width = self.container_size.get().width;
        let pull_x = scroll.x / width;
        let now = self.now();
        let mut w = self.edge_effect_wrapper.borrow_mut();
        let left_effect = w.get_or_create_left_effect();
        let consumed = left_effect.on_pull_distance(pull_x, 1.0 - displacement_y, now) * width;
        if left_effect.distance() != 0.0 { scroll.x } else { consumed }
    }

    fn pull_right(&self, scroll: Offset) -> f32 {
        let displacement_y = self.displacement().y;
        let width = self.container_size.get().width;
        let pull_x = scroll.x / width;
        let now = self.now();
        let mut w = self.edge_effect_wrapper.borrow_mut();
        let right_effect = w.get_or_create_right_effect();
        let consumed = -right_effect.on_pull_distance(-pull_x, displacement_y, now) * width;
        if right_effect.distance() != 0.0 { scroll.x } else { consumed }
    }

    /// A stretched edge's pull, whole if it relaxed it, else what it took of the destretched delta (Compose's
    /// `consumedPixelsY` and `X` branches).
    #[allow(clippy::too_many_arguments)]
    fn relax(
        &self,
        delta: f32,
        destretch_delta: Offset,
        destretch_component: f32,
        destretch_multiplier: f32,
        pull: impl FnOnce(&Self, Offset) -> f32,
        still_stretched: impl Fn(&EdgeEffectWrapper) -> bool,
        finish: impl FnOnce(&mut EdgeEffectWrapper),
    ) -> f32 {
        let consumed = pull(self, destretch_delta);
        // Reset state if the stretch fully relaxed.
        if !still_stretched(&self.edge_effect_wrapper.borrow()) {
            finish(&mut self.edge_effect_wrapper.borrow_mut());
        }
        // (no float errors from dividing if all the delta was consumed)
        if consumed == destretch_component { delta } else { consumed / destretch_multiplier }
    }
}

impl OverscrollEffect for AndroidEdgeEffectOverscrollEffect {
    fn apply_to_scroll(
        &self,
        delta: Offset,
        source: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset {
        let s = &*self.inner;
        // Early return
        if s.container_size.get().is_empty() {
            return perform_scroll(delta);
        }
        if !s.scroll_cycle_in_progress.get() {
            // A new scroll cycle: an active stretch is "caught" where it is (pulled by nothing), so the user goes on
            // with it instead of it fading away under the input.
            let (left, right, top, bottom) = {
                let w = s.edge_effect_wrapper.borrow();
                (w.is_left_stretched(), w.is_right_stretched(), w.is_top_stretched(), w.is_bottom_stretched())
            };
            if left {
                s.pull_left(Offset::Zero);
            }
            if right {
                s.pull_right(Offset::Zero);
            }
            if top {
                s.pull_top(Offset::Zero);
            }
            if bottom {
                s.pull_bottom(Offset::Zero);
            }
            s.scroll_cycle_in_progress.set(true);
        }
        // Relax existing stretches before scrolling; a fling relaxes them faster.
        let destretch_multiplier = destretch_multiplier(source);
        let destretch_delta = delta * destretch_multiplier;
        let (top_stretched, bottom_stretched, left_stretched, right_stretched) = {
            let w = s.edge_effect_wrapper.borrow();
            (w.is_top_stretched(), w.is_bottom_stretched(), w.is_left_stretched(), w.is_right_stretched())
        };
        let consumed_pixels_y = if delta.y == 0.0 {
            0.0
        } else if top_stretched && delta.y < 0.0 {
            s.relax(
                delta.y,
                destretch_delta,
                destretch_delta.y,
                destretch_multiplier,
                EffectInner::pull_top,
                EdgeEffectWrapper::is_top_stretched,
                |w| w.get_or_create_top_effect().finish(),
            )
        } else if bottom_stretched && delta.y > 0.0 {
            s.relax(
                delta.y,
                destretch_delta,
                destretch_delta.y,
                destretch_multiplier,
                EffectInner::pull_bottom,
                EdgeEffectWrapper::is_bottom_stretched,
                |w| w.get_or_create_bottom_effect().finish(),
            )
        } else {
            0.0
        };
        let consumed_pixels_x = if delta.x == 0.0 {
            0.0
        } else if left_stretched && delta.x < 0.0 {
            s.relax(
                delta.x,
                destretch_delta,
                destretch_delta.x,
                destretch_multiplier,
                EffectInner::pull_left,
                EdgeEffectWrapper::is_left_stretched,
                |w| w.get_or_create_left_effect().finish(),
            )
        } else if right_stretched && delta.x > 0.0 {
            s.relax(
                delta.x,
                destretch_delta,
                destretch_delta.x,
                destretch_multiplier,
                EffectInner::pull_right,
                EdgeEffectWrapper::is_right_stretched,
                |w| w.get_or_create_right_effect().finish(),
            )
        } else {
            0.0
        };
        let consumed_offset = Offset::new(consumed_pixels_x, consumed_pixels_y);
        if consumed_offset != Offset::Zero {
            s.invalidate_overscroll();
        }

        let left_for_delta = delta - consumed_offset;
        let consumed_by_delta = perform_scroll(left_for_delta);
        let left_for_overscroll = left_for_delta - consumed_by_delta;

        // Scrolling took some of what was there for it while stretched: it scrolls again (an item added at the
        // end, say), so the stretch lets go instead of staying stuck. (x and y apart: -0 isn't 0 as an Offset.)
        if (left_for_delta.x != 0.0 || left_for_delta.y != 0.0)
            && (consumed_by_delta.x != 0.0 || consumed_by_delta.y != 0.0)
        {
            let any_stretched = {
                let w = s.edge_effect_wrapper.borrow();
                w.is_left_stretched() || w.is_top_stretched() || w.is_right_stretched() || w.is_bottom_stretched()
            };
            if any_stretched {
                s.animate_to_release_if_needed();
            }
        }

        let mut needs_invalidation = false;
        if source == NestedScrollSource::UserInput {
            // Small deltas (< 0.5) are left: float rounding, which could lock scrolling up (b/265363356).
            let applied_horizontal_overscroll = if left_for_overscroll.x > 0.5 {
                s.pull_left(left_for_overscroll);
                true
            } else if left_for_overscroll.x < -0.5 {
                s.pull_right(left_for_overscroll);
                true
            } else {
                false
            };
            let applied_vertical_overscroll = if left_for_overscroll.y > 0.5 {
                s.pull_top(left_for_overscroll);
                true
            } else if left_for_overscroll.y < -0.5 {
                s.pull_bottom(left_for_overscroll);
                true
            } else {
                false
            };
            needs_invalidation = applied_horizontal_overscroll || applied_vertical_overscroll;
        }

        // Delta left over (the overscroll didn't take it): the opposite edges let go.
        if left_for_delta != Offset::Zero {
            needs_invalidation = s.release_opposite_overscroll(delta) || needs_invalidation;
        }
        if needs_invalidation {
            s.invalidate_overscroll();
        }
        consumed_offset + consumed_by_delta
    }

    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()> {
        Box::pin(async move {
            let s = &*self.inner;
            // Early return
            if s.container_size.get().is_empty() {
                perform_fling(velocity).await;
                return;
            }
            let size = s.container_size.get();
            let now = s.now();
            // Relax existing stretches before flinging.
            let consumed_x = {
                let mut w = s.edge_effect_wrapper.borrow_mut();
                if w.is_left_stretched() && velocity.x < 0.0 {
                    absorb_to_relax_if_needed(w.get_or_create_left_effect(), velocity.x, size.width, s.density, now)
                } else if w.is_right_stretched() && velocity.x > 0.0 {
                    -absorb_to_relax_if_needed(w.get_or_create_right_effect(), -velocity.x, size.width, s.density, now)
                } else {
                    0.0
                }
            };
            let consumed_y = {
                let mut w = s.edge_effect_wrapper.borrow_mut();
                if w.is_top_stretched() && velocity.y < 0.0 {
                    absorb_to_relax_if_needed(w.get_or_create_top_effect(), velocity.y, size.height, s.density, now)
                } else if w.is_bottom_stretched() && velocity.y > 0.0 {
                    -absorb_to_relax_if_needed(
                        w.get_or_create_bottom_effect(),
                        -velocity.y,
                        size.height,
                        s.density,
                        now,
                    )
                } else {
                    0.0
                }
            };
            let consumed = Velocity::new(consumed_x, consumed_y);
            if consumed != Velocity::Zero {
                s.invalidate_overscroll();
            }

            let remaining_velocity = velocity - consumed;
            let consumed_by_velocity = perform_fling(remaining_velocity).await;
            let left_for_overscroll = remaining_velocity - consumed_by_velocity;

            s.scroll_cycle_in_progress.set(false);
            // Stretch with any leftover velocity.
            let now = s.now();
            {
                let mut w = s.edge_effect_wrapper.borrow_mut();
                if left_for_overscroll.x > 0.0 {
                    w.get_or_create_left_effect().on_absorb(left_for_overscroll.x.round() as i32, now);
                } else if left_for_overscroll.x < 0.0 {
                    w.get_or_create_right_effect().on_absorb(-(left_for_overscroll.x.round() as i32), now);
                }
                if left_for_overscroll.y > 0.0 {
                    w.get_or_create_top_effect().on_absorb(left_for_overscroll.y.round() as i32, now);
                } else if left_for_overscroll.y < 0.0 {
                    w.get_or_create_bottom_effect().on_absorb(-(left_for_overscroll.y.round() as i32), now);
                }
            }
            // Release the rest (a stretch only when the velocity is exactly 0: nothing was absorbed).
            s.animate_to_release_if_needed();
        })
    }

    fn is_in_progress(&self) -> bool {
        let mut in_progress = false;
        self.inner.edge_effect_wrapper.borrow_mut().for_each_effect(|e| {
            if e.distance() != 0.0 {
                in_progress = true;
            }
        });
        in_progress
    }

    fn node(&self) -> DelegatableNode {
        self.inner.node.borrow().clone().expect("its node")
    }
}

/// Compose's `destretchMultiplier`.
fn destretch_multiplier(source: NestedScrollSource) -> f32 {
    if source == NestedScrollSource::SideEffect { FlingDestretchFactor } else { 1.0 }
}

/// A fling against a stretch: absorbed (all its velocity taken) if it wouldn't fling further than the stretch
/// shows, else left to relax it as it scrolls (Compose's `absorbToRelaxIfNeeded`).
fn absorb_to_relax_if_needed(
    effect: &mut EdgeEffect,
    velocity: f32,
    edge_effect_length: f32,
    density: Density,
    now: i64,
) -> f32 {
    let fling_distance = fling_distance(density, velocity);
    let actual_distance = effect.distance() * edge_effect_length;
    if fling_distance <= actual_distance {
        effect.on_absorb(velocity.round() as i32, now);
        velocity
    } else {
        0.0
    }
}

/// How far Android's spline flings at `velocity` (Compose's EdgeEffectCompat `flingDistance`).
fn fling_distance(density: Density, velocity: f32) -> f32 {
    const Inflection: f32 = 0.35;
    const GravityEarth: f32 = 9.80665;
    const InchesPerMeter: f32 = 39.37;
    let deceleration_rate = 0.78f64.ln() / 0.9f64.ln();
    let decel_minus_one = deceleration_rate - 1.0;
    let magic_physical_coefficient = (GravityEarth * InchesPerMeter * density.density * 160.0 * 0.84) as f64;
    let l =
        ((Inflection * velocity.abs()) as f64 / (PlatformFlingScrollFriction as f64 * magic_physical_coefficient)).ln();
    let distance = PlatformFlingScrollFriction as f64
        * magic_physical_coefficient
        * (deceleration_rate / decel_minus_one * l).exp();
    distance as f32
}

// ---------------------------------------------------------------------------------------------------------------
// The edges

/// The four edges' effects, made when first needed (Compose's EdgeEffectWrapper, without the negating effects).
#[derive(Default)]
struct EdgeEffectWrapper {
    size: IntSize,
    top_effect: Option<EdgeEffect>,
    bottom_effect: Option<EdgeEffect>,
    left_effect: Option<EdgeEffect>,
    right_effect: Option<EdgeEffect>,
}

fn is_stretched(e: &Option<EdgeEffect>) -> bool {
    e.as_ref().is_some_and(|e| e.distance() != 0.0)
}

fn is_animating(e: &Option<EdgeEffect>) -> bool {
    e.as_ref().is_some_and(|e| !e.is_finished())
}

impl EdgeEffectWrapper {
    fn for_each_effect(&mut self, mut action: impl FnMut(&mut EdgeEffect)) {
        for e in [&mut self.top_effect, &mut self.bottom_effect, &mut self.left_effect, &mut self.right_effect]
            .into_iter()
            .flatten()
        {
            action(e);
        }
    }

    fn is_top_stretched(&self) -> bool {
        is_stretched(&self.top_effect)
    }
    fn is_bottom_stretched(&self) -> bool {
        is_stretched(&self.bottom_effect)
    }
    fn is_left_stretched(&self) -> bool {
        is_stretched(&self.left_effect)
    }
    fn is_right_stretched(&self) -> bool {
        is_stretched(&self.right_effect)
    }
    fn is_top_animating(&self) -> bool {
        is_animating(&self.top_effect)
    }
    fn is_bottom_animating(&self) -> bool {
        is_animating(&self.bottom_effect)
    }
    fn is_left_animating(&self) -> bool {
        is_animating(&self.left_effect)
    }
    fn is_right_animating(&self) -> bool {
        is_animating(&self.right_effect)
    }

    /// An effect along a vertical (top, bottom) or horizontal (left, right) edge, sized (Compose's
    /// `createEdgeEffect`: the left and right ones are sized across).
    fn create_edge_effect(size: IntSize, vertical: bool) -> EdgeEffect {
        let mut e = EdgeEffect::new();
        if size != IntSize::Zero {
            if vertical {
                e.set_size(size.width, size.height);
            } else {
                e.set_size(size.height, size.width);
            }
        }
        e
    }

    fn get_or_create_top_effect(&mut self) -> &mut EdgeEffect {
        let size = self.size;
        self.top_effect.get_or_insert_with(|| Self::create_edge_effect(size, true))
    }
    fn get_or_create_bottom_effect(&mut self) -> &mut EdgeEffect {
        let size = self.size;
        self.bottom_effect.get_or_insert_with(|| Self::create_edge_effect(size, true))
    }
    fn get_or_create_left_effect(&mut self) -> &mut EdgeEffect {
        let size = self.size;
        self.left_effect.get_or_insert_with(|| Self::create_edge_effect(size, false))
    }
    fn get_or_create_right_effect(&mut self) -> &mut EdgeEffect {
        let size = self.size;
        self.right_effect.get_or_insert_with(|| Self::create_edge_effect(size, false))
    }

    fn update_size(&mut self, size: IntSize) {
        self.size = size;
        for e in [&mut self.top_effect, &mut self.bottom_effect].into_iter().flatten() {
            e.set_size(size.width, size.height);
        }
        for e in [&mut self.left_effect, &mut self.right_effect].into_iter().flatten() {
            e.set_size(size.height, size.width);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node

/// The node drawing Android's stretch (Compose's StretchOverscrollNode): it notes where the pointer is (its
/// pointer input delegate) and draws its content through a layer stretched by the edges.
struct StretchOverscrollNode {
    effect: Weak<EffectInner>,
    layer: Option<GraphicsLayer>,
    delegated_pointer_input: bool,
}

impl ModifierNode for StretchOverscrollNode {
    node_capabilities!(draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        if let Some(effect) = self.effect.upgrade() {
            *effect.handle.borrow_mut() = Some(cx.handle());
        }
        self.layer = Some(GraphicsLayer::new(Some(cx.current_value_of(&LocalPlatform))));
        if !self.delegated_pointer_input {
            // (Compose delegates to it as it is made; a node has its context once attached)
            self.delegated_pointer_input = true;
            let effect = self.effect.clone();
            cx.delegate(SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
                track_pointer(scope, effect.clone())
            }));
        }
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        if let Some(layer) = self.layer.take() {
            layer.release();
        }
        if let Some(effect) = self.effect.upgrade() {
            *effect.handle.borrow_mut() = None;
        }
    }
}

/// The pointer the stretch follows along its edge (Compose's `pointerInputNode`): the first down, then whichever
/// stays down; its place kept after all are up, so what stretches doesn't move.
async fn track_pointer(scope: PointerInputScope, effect: Weak<EffectInner>) {
    scope
        .await_each_gesture(move |s| {
            let effect = effect.clone();
            async move {
                let down = s.await_first_down_with(false, PointerEventPass::Main).await;
                if let Some(e) = effect.upgrade() {
                    e.pointer_id.set(down.id);
                    e.pointer_position.set(down.position);
                }
                loop {
                    let event = s.await_pointer_event().await;
                    let pressed: Vec<_> = event.changes.iter().filter(|c| c.pressed).collect();
                    if let Some(e) = effect.upgrade() {
                        let change =
                            pressed.iter().find(|c| c.id == e.pointer_id.get()).or_else(|| pressed.first()).copied();
                        if let Some(change) = change {
                            e.pointer_id.set(change.id);
                            e.pointer_position.set(change.position);
                        }
                    }
                    if pressed.is_empty() {
                        break;
                    }
                }
                if let Some(e) = effect.upgrade() {
                    e.pointer_id.set(PointerId(-1));
                }
            }
        })
        .await
}

impl DrawModifierNode for StretchOverscrollNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let Some(effect) = self.effect.upgrade() else {
            scope.draw_content();
            return;
        };
        let size = scope.size();
        effect.update_size(size);
        effect.redraw_signal.get(); // (read: drawn again when it is set)
        if size.is_empty() {
            // Draw any out of bounds content.
            scope.draw_content();
            return;
        }
        let Some(layer) = self.layer.clone() else {
            scope.draw_content();
            return;
        };
        let max_elevation = scope.density().to_px(MaxSupportedElevation);
        let (width, height) = (size.width.round() as i32, size.height.round() as i32);
        // (the springs stepped to the frame's time first: what is drawn is decided as it will be by another draw
        // of this frame — the keep check's, a test's)
        let now = effect.now();
        let mut needs_invalidate = false;
        effect.edge_effect_wrapper.borrow_mut().for_each_effect(|e| needs_invalidate |= e.step(now));
        let (should_draw_vertical_stretch, should_draw_horizontal_stretch) = {
            let w = effect.edge_effect_wrapper.borrow();
            (w.is_top_animating() || w.is_bottom_animating(), w.is_left_animating() || w.is_right_animating())
        };
        let layer_size = match (should_draw_vertical_stretch, should_draw_horizontal_stretch) {
            // Both ways: the canvas' size (each way clipped).
            (true, true) => IntSize::new(width, height),
            // Vertically: wider, so nothing is cut across.
            (true, false) => IntSize::new(width + max_elevation.round() as i32 * 2, height),
            // Horizontally: taller.
            (false, true) => IntSize::new(width, height + max_elevation.round() as i32 * 2),
            // No stretch: drawn as it is.
            (false, false) => {
                if needs_invalidate {
                    effect.invalidate_overscroll();
                }
                layer.clear_stretch();
                scope.draw_content();
                return;
            }
        };
        // (the stretch of this frame only: Android's views clear the RenderNode's each frame)
        layer.clear_stretch();
        {
            let mut w = effect.edge_effect_wrapper.borrow_mut();
            if w.is_left_animating() {
                needs_invalidate = w.get_or_create_left_effect().draw(270.0, &layer, now) || needs_invalidate;
            }
            if w.is_top_animating() {
                needs_invalidate = w.get_or_create_top_effect().draw(0.0, &layer, now) || needs_invalidate;
            }
            if w.is_right_animating() {
                needs_invalidate = w.get_or_create_right_effect().draw(90.0, &layer, now) || needs_invalidate;
            }
            if w.is_bottom_animating() {
                needs_invalidate = w.get_or_create_bottom_effect().draw(180.0, &layer, now) || needs_invalidate;
            }
        }
        if needs_invalidate {
            effect.invalidate_overscroll();
        }
        // The content recorded at its size, moved into the wider layer across (none along the stretch: it clips
        // there), and the layer drawn moved back.
        let left = if should_draw_horizontal_stretch { 0.0 } else { max_elevation };
        let top = if should_draw_vertical_stretch { 0.0 } else { max_elevation };
        scope.record_with(&layer, layer_size, |s| s.translate_with(left, top, |s| s.draw_content()));
        layer.set_top_left(IntOffset::new(-(left as i32), -(top as i32)));
        scope.draw_layer(&layer);
    }
}
