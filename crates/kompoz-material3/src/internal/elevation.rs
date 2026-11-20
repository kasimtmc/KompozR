//! Elevation animated between interactions, as Compose's internal Elevation.kt: a component's elevation (a
//! button's, a card's) going to the one of a press, a drag, a hover or a focus fast, and back more slowly — a
//! hover's at the speed it came.

use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, CubicBezierEasing, Easing, FastOutSlowInEasing, TweenSpec,
};
use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, Interaction, PressInteraction,
};
use kompoz_graphics::Dp;
use kompoz_runtime::named_args;
use std::future::Future;
use std::sync::Arc;

/// Animating an elevation (Compose's internal `Animatable<Dp, *>.animateElevation`).
pub(crate) trait AnimateElevation {
    /// Animates to `target`, the elevation of the `to` interaction (none: the component's default state), from the
    /// `from` one (none: none before it): along [`ElevationDefaults`]' spec for going to `to`, else for leaving
    /// `from`; with neither (the first value, or back from a disabled or unknown state), snapping to it.
    fn animate_elevation(
        &self,
        target: Dp,
        from: Option<&dyn Interaction>,
        to: Option<&dyn Interaction>,
    ) -> impl Future<Output = ()> + 'static;
}

impl AnimateElevation for Animatable<Dp, AnimationVector1D> {
    fn animate_elevation(
        &self,
        target: Dp,
        from: Option<&dyn Interaction>,
        to: Option<&dyn Interaction>,
    ) -> impl Future<Output = ()> + 'static {
        let spec = match (to, from) {
            // Moving to a new state
            (Some(to), _) => ElevationDefaults::incoming_animation_spec_for_interaction(to),
            // Moving to default, from a previous state
            (None, Some(from)) => ElevationDefaults::outgoing_animation_spec_for_interaction(from),
            // Loading the initial state, or moving back to the baseline state from a disabled / unknown state, so
            // just snap to the final value.
            (None, None) => None,
        };
        let animatable = self.clone();
        async move {
            match spec {
                Some(spec) => {
                    named_args!(animatable.animate_to(target, animation_spec = spec)).await;
                }
                None => animatable.snap_to(target).await,
            }
        }
    }
}

/// The specs elevation animates by between interactions (Compose's private `ElevationDefaults`).
struct ElevationDefaults;

impl ElevationDefaults {
    /// The spec of going to `interaction`, from another or from the default state; none for an interaction it
    /// doesn't know (Compose's `incomingAnimationSpecForInteraction`).
    fn incoming_animation_spec_for_interaction(interaction: &dyn Interaction) -> Option<Arc<dyn AnimationSpec<Dp>>> {
        if interaction.is::<PressInteraction::Press>()
            || interaction.is::<DragInteraction::Start>()
            || interaction.is::<HoverInteraction::Enter>()
            || interaction.is::<FocusInteraction::Focus>()
        {
            Some(DefaultIncomingSpec())
        } else {
            None
        }
    }

    /// The spec of leaving `interaction` for the default state; none for an interaction it doesn't know (Compose's
    /// `outgoingAnimationSpecForInteraction`).
    fn outgoing_animation_spec_for_interaction(interaction: &dyn Interaction) -> Option<Arc<dyn AnimationSpec<Dp>>> {
        if interaction.is::<PressInteraction::Press>()
            || interaction.is::<DragInteraction::Start>()
            || interaction.is::<FocusInteraction::Focus>()
        {
            Some(DefaultOutgoingSpec())
        } else if interaction.is::<HoverInteraction::Enter>() {
            Some(HoveredOutgoingSpec())
        } else {
            None
        }
    }
}

/// The easing of going back (Compose's private `OutgoingSpecEasing`).
const OutgoingSpecEasing: CubicBezierEasing = CubicBezierEasing::constant(0.40, 0.00, 0.60, 1.00);

/// 120 ms, fast out, slow in (Compose's private `DefaultIncomingSpec`).
fn DefaultIncomingSpec() -> Arc<dyn AnimationSpec<Dp>> {
    shared_tween(120, FastOutSlowInEasing)
}

/// 150 ms, [`OutgoingSpecEasing`] (Compose's private `DefaultOutgoingSpec`).
fn DefaultOutgoingSpec() -> Arc<dyn AnimationSpec<Dp>> {
    shared_tween(150, OutgoingSpecEasing)
}

/// 120 ms, [`OutgoingSpecEasing`] (Compose's private `HoveredOutgoingSpec`).
fn HoveredOutgoingSpec() -> Arc<dyn AnimationSpec<Dp>> {
    shared_tween(120, OutgoingSpecEasing)
}

/// A tween of `duration_millis` and `easing`, no delay.
fn shared_tween(duration_millis: i32, easing: impl Easing) -> Arc<dyn AnimationSpec<Dp>> {
    Arc::new(TweenSpec::<Dp>::new(duration_millis, 0, easing))
}
