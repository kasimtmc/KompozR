//! A pane that animates, as Compose's adaptive-layout Pane.kt (1.2.0): [`AnimatedPane`](ThreePaneScaffoldPaneScope::AnimatedPane)
//! (a pane's content coming in, going out and moving with the scaffold's motion), its scope ([`AnimatedPaneScope`]),
//! its override API ([`AnimatedPaneOverride`], [`LocalAnimatedPaneOverride`]).

use super::animate_bounds_modifier::animate_bounds;
use super::internal::delegable_modifier::delegable_semantics;
use super::internal::remember_utils::remember_ref;
use super::internal::strings::{Strings, get_string_of_node};
use super::pane_adapted_value::{PaneAdaptedValue, PaneScrim};
use super::pane_motion::{PaneMotion, PaneMotionDefaults, PaneScaffoldMotionDataProviderExt};
use super::pane_scaffold::{PaneScaffoldPaneScope, PaneScaffoldScope, PaneScaffoldTransitionScope, animated_pane};
use super::pane_scaffold_role::ThreePaneScaffoldRole;
use super::three_pane_scaffold_scope::ThreePaneScaffoldPaneScope;
use super::three_pane_scaffold_value::ThreePaneScaffoldValue;
use kompoz_animation::{AnimatedVisibilityScope, EnterTransition, ExitTransition, FiniteAnimationSpec};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Dp, IntOffset, IntRect};
use kompoz_runtime::{KompozitionLocal, kompozable, kompozable_block, named};
use kompoz_ui::Modifier;
use kompoz_ui::focus::FocusProperties;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

#[named]
impl ThreePaneScaffoldPaneScope {
    /// The pane's content, animated with the scaffold (Compose's `ExtendedPaneScaffoldPaneScope.AnimatedPane`):
    /// shown while the pane isn't hidden, coming in by `enter_transition` and going out by `exit_transition` (by
    /// default the pane's motion's: sliding, expanding, fading as a modal), its bounds animated along
    /// `bounds_animation_spec` while it stays shown; focus and accessibility only while it takes interaction; a
    /// levitated pane casts a 15 dp shadow and brings its scrim. `content` is kept (its saveable state) while the
    /// pane is hidden. Composable.
    ///
    /// Kotlin: `AnimatedPane(modifier: Modifier = Modifier, enterTransition: EnterTransition = motionDataProvider.calculateDefaultEnterTransition(paneRole), exitTransition: ExitTransition = motionDataProvider.calculateDefaultExitTransition(paneRole), boundsAnimationSpec: FiniteAnimationSpec<IntRect> = PaneMotionDefaults.AnimationSpec, content: @Composable AnimatedPaneScope.() -> Unit)`
    #[track_caller]
    pub fn AnimatedPane(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(self.motion_data_provider().calculate_default_enter_transition(self.pane_role()))]
        enter_transition: EnterTransition,
        #[default(self.motion_data_provider().calculate_default_exit_transition(self.pane_role()))]
        exit_transition: ExitTransition,
        #[default(PaneMotionDefaults::AnimationSpec())] bounds_animation_spec: Arc<dyn FiniteAnimationSpec<IntRect>>,
        content: impl Fn(&AnimatedPaneScope) + 'static,
    ) {
        let scope = AnimatedPaneOverrideScope {
            scope: self.clone(),
            modifier,
            enter_transition,
            exit_transition,
            bounds_animation_spec,
            content: Rc::new(content),
        };
        kompozable_block!({
            LocalAnimatedPaneOverride.current().AnimatedPane(&scope);
        })
    }
}

/// How an animated pane is made (Compose's experimental `AnimatedPaneOverride`): provided by
/// [`LocalAnimatedPaneOverride`] to the panes below.
pub trait AnimatedPaneOverride {
    /// Composes the pane of `scope`'s arguments (Compose's `AnimatedPaneOverrideScope.AnimatedPane()`).
    fn AnimatedPane(&self, scope: &AnimatedPaneOverrideScope);
}

/// What an override makes an animated pane of (Compose's experimental `AnimatedPaneOverrideScope`): the pane's
/// scope and the arguments.
#[derive(Clone)]
pub struct AnimatedPaneOverrideScope {
    pub scope: ThreePaneScaffoldPaneScope,
    pub modifier: Modifier,
    pub enter_transition: EnterTransition,
    pub exit_transition: ExitTransition,
    pub bounds_animation_spec: Arc<dyn FiniteAnimationSpec<IntRect>>,
    pub content: Rc<dyn Fn(&AnimatedPaneScope)>,
}

/// The override the panes below are made by (Compose's experimental `LocalAnimatedPaneOverride`): by default
/// Compose's private `DefaultAnimatedPaneOverride`.
pub static LocalAnimatedPaneOverride: KompozitionLocal<Rc<dyn AnimatedPaneOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultAnimatedPaneOverride));

struct DefaultAnimatedPaneOverride;

impl AnimatedPaneOverride for DefaultAnimatedPaneOverride {
    #[track_caller]
    fn AnimatedPane(&self, scope: &AnimatedPaneOverrideScope) {
        let scope = scope.clone();
        kompozable_block!({
            DefaultAnimatedPane(scope);
        })
    }
}

#[kompozable]
fn DefaultAnimatedPane(override_scope: AnimatedPaneOverrideScope) {
    let scope = override_scope.scope.clone();
    let provider = scope.motion_data_provider();
    let scale_state = provider.predictive_back_scale_state.clone();
    let scale_conversion: Rc<dyn Fn(IntOffset) -> IntOffset> = Rc::new(move |offset| scale_state.convert(offset));
    let animating_bounds = scope.pane_motion() == PaneMotion::AnimateBounds;
    let progress_scope = scope.clone();
    let motion_progress: Rc<dyn Fn() -> f32> = Rc::new(move || progress_scope.motion_progress());
    let transition = scope.scaffold_state_transition();
    let pane_role = scope.pane_role();
    let pane_value = transition.target_state().get_ref(pane_role).clone();
    let modifier = animate_bounds(
        animated_pane(override_scope.modifier.clone()),
        motion_progress,
        override_scope.bounds_animation_spec.clone(),
        scale_conversion,
        scope.lookahead_scope(),
        animating_bounds,
    )
    .focus_requester(&scope.focus_requester(pane_role));
    let modifier = focusable_in_whole_tree(modifier, scope.is_interactable(), pane_role)
        .then(if pane_value.is_levitated() {
            kompoz_runtime::named_args!(Modifier.shadow(AnimatedPaneDefaults::ShadowElevation))
        } else {
            Modifier
        })
        .then(if animating_bounds { Modifier } else { Modifier.clip_to_bounds() });
    let (content, holder, content_scope) =
        (override_scope.content.clone(), scope.saveable_state_holder(), scope.clone());
    transition.AnimatedVisibility(
        visible = move |value: &ThreePaneScaffoldValue| *value.get_ref(pane_role) != PaneAdaptedValue::Hidden,
        modifier = modifier,
        enter = override_scope.enter_transition.clone(),
        exit = override_scope.exit_transition.clone(),
        content = move |visibility: &AnimatedVisibilityScope| {
            let (content, visibility) = (content.clone(), visibility.clone());
            let _ = &content_scope;
            holder.SaveableStateProvider(&pane_role.to_string(), move || {
                content(&AnimatedPaneScope::create(visibility.clone()));
            });
        },
    );

    let scrim = remember_ref::<PaneScrim>(None);
    if let PaneAdaptedValue::Levitated { scrim: levitated, .. } = &pane_value {
        scrim.set_value(levitated.get());
    }
    if let Some(scrim) = scrim.value() {
        // Display a scrim when the pane gets levitated
        transition.AnimatedVisibility(
            visible = move |value: &ThreePaneScaffoldValue| *value.get_ref(pane_role) != PaneAdaptedValue::Hidden,
            enter = override_scope.enter_transition.clone(),
            exit = override_scope.exit_transition.clone(),
            content = move |_: &AnimatedVisibilityScope| scrim(),
        );
    }
}

/// The scope of an animated pane's content (Compose's sealed interface `AnimatedPaneScope`): an
/// [`AnimatedVisibilityScope`] (reached through `Deref`).
#[derive(Clone)]
pub struct AnimatedPaneScope(AnimatedVisibilityScope);

impl AnimatedPaneScope {
    /// The scope of `animated_visibility_scope` (Compose's `AnimatedPaneScope.create`).
    pub fn create(animated_visibility_scope: AnimatedVisibilityScope) -> AnimatedPaneScope {
        AnimatedPaneScope(animated_visibility_scope)
    }
}

impl Deref for AnimatedPaneScope {
    type Target = AnimatedVisibilityScope;

    fn deref(&self) -> &AnimatedVisibilityScope {
        &self.0
    }
}

/// Compose's internal `AnimatedPaneDefaults`.
struct AnimatedPaneDefaults;

impl AnimatedPaneDefaults {
    const ShadowElevation: Dp = Dp(15.0);
}

/// The pane a focus group taking the focus (and accessibility, titled by its role) only while `focusable`
/// (Compose's private `focusableInWholeTree`).
fn focusable_in_whole_tree(modifier: Modifier, focusable: bool, role: ThreePaneScaffoldRole) -> Modifier {
    // Workaround(b/342653995): Make the whole pane a focus group but cancel any focusing attempts so the whole
    // subtree won't be focusable.
    let modifier = modifier.focus_group().focus_properties(move |p: &mut FocusProperties| {
        if !focusable {
            p.can_focus = false;
            p.on_enter = Some(Rc::new(|scope| scope.cancel_focus_change()));
        }
    });
    if focusable {
        delegable_semantics(
            modifier,
            false,
            Rc::new(move |r| {
                r.set_is_traversal_group(true);
                if let Some(title) = pane_title(role) {
                    let text = get_string_of_node(r.node(), title);
                    r.set_pane_title(text);
                }
            }),
        )
    } else {
        // Workaround(b/343950986): clear all semantics under the tree so no children can get the a11y focus.
        modifier.clear_and_set_semantics(|_| {})
    }
}

/// The default title of a pane of `role` (Compose's private `PaneScaffoldRole.paneTitle`).
fn pane_title(role: ThreePaneScaffoldRole) -> Option<Strings> {
    Some(match role {
        ThreePaneScaffoldRole::Primary => Strings::defaultPaneTitlePrimary,
        ThreePaneScaffoldRole::Secondary => Strings::defaultPaneTitleSecondary,
        ThreePaneScaffoldRole::Tertiary => Strings::defaultPaneTitleTertiary,
    })
}
