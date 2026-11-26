//! NavDisplay.kt (+ NavDisplay.android.kt's defaults, the non-Android actuals here), LocalNavAnimatedContentScope.kt.

use super::scene_ref::{AnimatedSceneKey, AnyScene};
use crate::runtime::{
    BackStackList, IntoMetadataValue, Metadata, NavEntry, NavEntryDecorator, NavEntryKey, NavMetadataKey,
    remember_decorated_nav_entries, remember_saveable_state_holder_nav_entry_decorator,
};
use crate::scene::{
    LocalCurrentScene, LocalEntriesToExcludeFromCurrentScene, OverlayScene, SceneDecoratorStrategy, SceneInfo,
    SceneNavigationBackHandler, SceneState, SceneStrategy, SinglePaneSceneStrategy,
    remember_navigation_event_state_of_scene_state, remember_scene_state,
};
use kompoz_animation::{
    AnimatedContentScope, AnimatedContentTransitionScope, ContentTransform, SeekableTransitionState,
    SharedTransitionScope, SizeTransform, animate, fade_in, fade_out, remember_transition, scale_out, spring, tween,
};
use kompoz_coroutines::{LaunchedEffect, coroutine_scope, snapshot_flow};
use kompoz_lifecycle::{LifecycleState, LocalLifecycleOwner, remember_lifecycle_owner};
use kompoz_navigationevent::{NavigationEvent, NavigationEventState, NavigationEventTransitionState};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, MutableState, SnapshotStateList, key, kompozable,
    mutable_confined_state_list_of, mutable_confined_state_of, mutable_confined_state_of_with_policy, named_args,
    never_equal_policy, remember, remember_keyed,
};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

/// The scope of `NavDisplay`'s contents (Compose's `LocalNavAnimatedContentScope`): the `AnimatedContentScope` an
/// entry's content is animated in (shared elements: `Modifier.sharedElement(…, animatedVisibilityScope =
/// LocalNavAnimatedContentScope.current)`). No default: read outside a `NavDisplay`'s entry it panics (an overlay
/// scene's is a scope that doesn't animate).
///
/// Kotlin: `LocalNavAnimatedContentScope`
pub static LocalNavAnimatedContentScope: KompozitionLocal<AnimatedContentScope> = KompozitionLocal::new(|| {
    panic_kt(
        "IllegalStateException",
        "Unexpected access to LocalNavAnimatedContentScope. You should only access LocalNavAnimatedContentScope inside \
         a NavEntry passed to NavDisplay. AnimatedContentScope in OverlayScenes are no-op.",
    )
});

/// A transition of `NavDisplay` given by an entry's or a scene's metadata (Kotlin's `AnimatedContentTransitionScope<
/// Scene<*>>.() -> ContentTransform?`): none falls back to `NavDisplay`'s.
pub type SceneTransitionSpec = Rc<dyn Fn(&AnimatedContentTransitionScope<AnyScene>) -> Option<ContentTransform>>;

/// A predictive pop's transition given by metadata (Kotlin's `AnimatedContentTransitionScope<Scene<*>>.(@SwipeEdge
/// Int) -> ContentTransform?`), of the gesture's swipe edge (`NavigationEvent::EDGE_…`).
pub type ScenePredictivePopTransitionSpec =
    Rc<dyn Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> Option<ContentTransform>>;

/// A lambda giving a transform, made a [`SceneTransitionSpec`].
#[doc(hidden)]
pub struct GivesTransform;
/// A lambda giving a transform or none, made a [`SceneTransitionSpec`].
#[doc(hidden)]
pub struct GivesTransformOrNone;

impl<F: Fn(&AnimatedContentTransitionScope<AnyScene>) -> ContentTransform + 'static>
    IntoMetadataValue<SceneTransitionSpec, GivesTransform> for F
{
    fn into_metadata_value(self) -> SceneTransitionSpec {
        Rc::new(move |scope| Some(self(scope)))
    }
}

impl<F: Fn(&AnimatedContentTransitionScope<AnyScene>) -> Option<ContentTransform> + 'static>
    IntoMetadataValue<SceneTransitionSpec, GivesTransformOrNone> for F
{
    fn into_metadata_value(self) -> SceneTransitionSpec {
        Rc::new(self)
    }
}

/// A lambda of a transition key written as Kotlin's syntax writes every `put` lambda (`put(NavDisplay.TransitionKey) {
/// slideIn… }`: the Kotlin index gives `put`'s lambda the predictive pop's parameters, the swipe edge): its edge none.
#[doc(hidden)]
pub struct GivesTransformOfEdge;
/// The same, giving a transform or none.
#[doc(hidden)]
pub struct GivesTransformOrNoneOfEdge;

impl<F: Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> ContentTransform + 'static>
    IntoMetadataValue<SceneTransitionSpec, GivesTransformOfEdge> for F
{
    fn into_metadata_value(self) -> SceneTransitionSpec {
        Rc::new(move |scope| Some(self(scope, 0)))
    }
}

impl<F: Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> Option<ContentTransform> + 'static>
    IntoMetadataValue<SceneTransitionSpec, GivesTransformOrNoneOfEdge> for F
{
    fn into_metadata_value(self) -> SceneTransitionSpec {
        Rc::new(move |scope| self(scope, 0))
    }
}

impl<F: Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> ContentTransform + 'static>
    IntoMetadataValue<ScenePredictivePopTransitionSpec, GivesTransform> for F
{
    fn into_metadata_value(self) -> ScenePredictivePopTransitionSpec {
        Rc::new(move |scope, edge| Some(self(scope, edge)))
    }
}

impl<F: Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> Option<ContentTransform> + 'static>
    IntoMetadataValue<ScenePredictivePopTransitionSpec, GivesTransformOrNone> for F
{
    fn into_metadata_value(self) -> ScenePredictivePopTransitionSpec {
        Rc::new(self)
    }
}

/// The key of the metadata giving the transition of a navigation to an entry or scene (Compose's
/// `NavDisplay.TransitionKey`). `NavDisplay` reads only the scene's metadata (a scene's by default its last entry's).
///
/// Kotlin: `NavDisplay.TransitionKey`
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavDisplayTransitionKey;
impl NavMetadataKey<SceneTransitionSpec> for NavDisplayTransitionKey {}

/// The key of the metadata giving the transition of a pop (Compose's `NavDisplay.PopTransitionKey`).
///
/// Kotlin: `NavDisplay.PopTransitionKey`
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavDisplayPopTransitionKey;
impl NavMetadataKey<SceneTransitionSpec> for NavDisplayPopTransitionKey {}

/// The key of the metadata giving the transition of a pop by a predictive back gesture (Compose's
/// `NavDisplay.PredictivePopTransitionKey`), of the gesture's swipe edge.
///
/// Kotlin: `NavDisplay.PredictivePopTransitionKey`
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavDisplayPredictivePopTransitionKey;
impl NavMetadataKey<ScenePredictivePopTransitionSpec> for NavDisplayPredictivePopTransitionKey {}

/// What `NavDisplay` handles of entries' and scenes' metadata (Compose's object `NavDisplay`): the transition keys and
/// the functions making their metadata. (`NavDisplay(…)` is the composable.)
pub struct NavDisplay {
    _object: (),
}

#[allow(non_upper_case_globals)]
impl NavDisplay {
    /// Kotlin: `NavDisplay.TransitionKey`
    pub const TransitionKey: NavDisplayTransitionKey = NavDisplayTransitionKey;
    /// Kotlin: `NavDisplay.PopTransitionKey`
    pub const PopTransitionKey: NavDisplayPopTransitionKey = NavDisplayPopTransitionKey;
    /// Kotlin: `NavDisplay.PredictivePopTransitionKey`
    pub const PredictivePopTransitionKey: NavDisplayPredictivePopTransitionKey = NavDisplayPredictivePopTransitionKey;

    /// The metadata giving the transition of a navigation to it (Compose's `NavDisplay.transitionSpec(…)`): none
    /// falls back to `NavDisplay`'s.
    ///
    /// Kotlin: `transitionSpec(transitionSpec: AnimatedContentTransitionScope<Scene<*>>.() -> ContentTransform?): Map<String, Any>`
    pub fn transition_spec(
        transition_spec: impl Fn(&AnimatedContentTransitionScope<AnyScene>) -> Option<ContentTransform> + 'static,
    ) -> Metadata {
        let spec: SceneTransitionSpec = Rc::new(transition_spec);
        Metadata::of(NavDisplayTransitionKey.metadata_key_name(), spec)
    }

    /// The metadata giving the transition of a pop from it (Compose's `NavDisplay.popTransitionSpec(…)`).
    ///
    /// Kotlin: `popTransitionSpec(popTransitionSpec: AnimatedContentTransitionScope<Scene<*>>.() -> ContentTransform?): Map<String, Any>`
    pub fn pop_transition_spec(
        pop_transition_spec: impl Fn(&AnimatedContentTransitionScope<AnyScene>) -> Option<ContentTransform> + 'static,
    ) -> Metadata {
        let spec: SceneTransitionSpec = Rc::new(pop_transition_spec);
        Metadata::of(NavDisplayPopTransitionKey.metadata_key_name(), spec)
    }

    /// The metadata giving the transition of a predictive pop from it (Compose's
    /// `NavDisplay.predictivePopTransitionSpec(…)`).
    ///
    /// Kotlin: `predictivePopTransitionSpec(predictivePopTransitionSpec: AnimatedContentTransitionScope<Scene<*>>.(Int) -> ContentTransform?): Map<String, Any>`
    pub fn predictive_pop_transition_spec(
        predictive_pop_transition_spec: impl Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> Option<ContentTransform>
        + 'static,
    ) -> Metadata {
        let spec: ScenePredictivePopTransitionSpec = Rc::new(predictive_pop_transition_spec);
        Metadata::of(NavDisplayPredictivePopTransitionKey.metadata_key_name(), spec)
    }
}

/// The default duration of `NavDisplay`'s transitions (Compose's internal `DEFAULT_TRANSITION_DURATION_MILLISECOND`).
pub(crate) const DEFAULT_TRANSITION_DURATION_MILLISECOND: i32 = 700;

fn default_fade() -> ContentTransform {
    let spec = || -> Arc<dyn kompoz_animation::FiniteAnimationSpec<f32>> {
        Arc::new(named_args!(tween(DEFAULT_TRANSITION_DURATION_MILLISECOND)))
    };
    named_args!(ContentTransform(fade_in(animation_spec = spec()), fade_out(animation_spec = spec())))
}

/// `NavDisplay`'s default transition of a navigation (Compose's `defaultTransitionSpec()`, Android's actual): a fade in
/// and out over 700 ms.
///
/// Kotlin: `defaultTransitionSpec(): AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform`
pub fn default_transition_spec(_: &AnimatedContentTransitionScope<AnyScene>) -> ContentTransform {
    default_fade()
}

/// `NavDisplay`'s default transition of a pop (Compose's `defaultPopTransitionSpec()`): a fade in and out over 700 ms.
///
/// Kotlin: `defaultPopTransitionSpec(): AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform`
pub fn default_pop_transition_spec(_: &AnimatedContentTransitionScope<AnyScene>) -> ContentTransform {
    default_fade()
}

/// `NavDisplay`'s default transition of a predictive pop (Compose's `defaultPredictivePopTransitionSpec()`): a fade in
/// (material3's default effects spring: damping 1, stiffness 1600) and a scale out to 0.7.
///
/// Kotlin: `defaultPredictivePopTransitionSpec(): AnimatedContentTransitionScope<Scene<T>>.(Int) -> ContentTransform`
pub fn default_predictive_pop_transition_spec(
    _: &AnimatedContentTransitionScope<AnyScene>,
    _swipe_edge: i32,
) -> ContentTransform {
    let spring_spec: kompoz_animation::SpringSpec<f32> = named_args!(spring(damping_ratio = 1.0, stiffness = 1600.0));
    let enter_spec: Arc<dyn kompoz_animation::FiniteAnimationSpec<f32>> = Arc::new(spring_spec);
    named_args!(ContentTransform(fade_in(animation_spec = enter_spec), scale_out(target_scale = 0.7)))
}

/// What a display does on back (Kotlin's `onBack: () -> Unit` with its default popping the back stack given): a
/// lambda (`|| back_stack.remove_last_or_null()`), or [`RemoveLastOrNull`], the default.
pub trait NavDisplayOnBack: 'static {
    /// Handles back; `pop` pops the display's back stack (if it is mutable).
    fn on_back(&self, pop: &dyn Fn());
}

impl<F: Fn() + 'static> NavDisplayOnBack for F {
    fn on_back(&self, _: &dyn Fn()) {
        self()
    }
}

/// `NavDisplay`'s default back: its back stack's last key removed if the back stack is mutable (Kotlin's `if (backStack
/// is MutableList<T>) backStack.removeLastOrNull()`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RemoveLastOrNull;

impl NavDisplayOnBack for RemoveLastOrNull {
    fn on_back(&self, pop: &dyn Fn()) {
        pop()
    }
}

/// The scene strategies `NavDisplay` takes by default: a single pane's.
#[doc(hidden)]
pub fn default_scene_strategies<T: NavEntryKey>() -> Vec<Rc<dyn SceneStrategy<T>>> {
    vec![Rc::new(SinglePaneSceneStrategy())]
}

/// Shows the back stack `back_stack` (Compose's `NavDisplay(backStack, …)`): its keys' entries (made by
/// `entry_provider`, decorated by `entry_decorators`: by default with a saveable state holder's) shown in the scenes
/// `scene_strategies` calculate (a single pane when none does), decorated by `scene_decorator_strategies`, a change of
/// scene animated: a navigation by `transition_spec`, a pop by `pop_transition_spec`, a predictive back gesture's pop
/// by `predictive_pop_transition_spec` (the scene's metadata, else the entry's, gives others: [`NavDisplay`]'s keys);
/// entries shared between scenes in `shared_transition_scope`. Back calls `on_back` (by default the back stack's
/// last key removed, if it is mutable). The back stack must not be empty.
///
/// Kotlin: `NavDisplay(backStack: List<T>, modifier: Modifier = Modifier, contentAlignment: Alignment = Alignment.TopStart, onBack: () -> Unit = { if (backStack is MutableList<T>) backStack.removeLastOrNull() }, entryDecorators: List<NavEntryDecorator<T>> = listOf(rememberSaveableStateHolderNavEntryDecorator()), sceneStrategies: List<SceneStrategy<T>> = listOf(SinglePaneSceneStrategy()), sceneDecoratorStrategies: List<SceneDecoratorStrategy<T>> = emptyList(), sharedTransitionScope: SharedTransitionScope? = null, sizeTransform: SizeTransform? = null, transitionSpec: AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform = defaultTransitionSpec(), popTransitionSpec: AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform = defaultPopTransitionSpec(), predictivePopTransitionSpec: AnimatedContentTransitionScope<Scene<T>>.(Int) -> ContentTransform = defaultPredictivePopTransitionSpec(), entryProvider: (key: T) -> NavEntry<T>)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn NavDisplay<T: NavEntryKey>(
    back_stack: impl BackStackList<T> + Clone + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(RemoveLastOrNull)] on_back: impl NavDisplayOnBack,
    #[default(vec![remember_saveable_state_holder_nav_entry_decorator()])] entry_decorators: Vec<NavEntryDecorator<T>>,
    #[default(default_scene_strategies())] scene_strategies: Vec<Rc<dyn SceneStrategy<T>>>,
    #[default(Vec::new())] scene_decorator_strategies: Vec<Rc<dyn SceneDecoratorStrategy<T>>>,
    #[default(None)] shared_transition_scope: Option<SharedTransitionScope>,
    #[default(None)] size_transform: Option<Rc<dyn SizeTransform>>,
    #[default(default_transition_spec)] transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
    ) -> ContentTransform
    + 'static,
    #[default(default_pop_transition_spec)] pop_transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
    ) -> ContentTransform
    + 'static,
    #[default(default_predictive_pop_transition_spec)] predictive_pop_transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
        i32,
    ) -> ContentTransform
    + 'static,
    entry_provider: impl Fn(T) -> NavEntry<T> + 'static,
) {
    let keys = back_stack.keys();
    if keys.is_empty() {
        panic_kt("IllegalArgumentException", "NavDisplay backstack cannot be empty");
    }
    let entries = remember_decorated_nav_entries(keys, entry_decorators, entry_provider);
    let stack = back_stack.clone();
    let pop = move || {
        stack.remove_last_key();
    };
    NavDisplayOfEntries(
        entries,
        modifier,
        content_alignment,
        scene_strategies,
        scene_decorator_strategies,
        shared_transition_scope,
        size_transform,
        transition_spec,
        pop_transition_spec,
        predictive_pop_transition_spec,
        move || on_back.on_back(&pop),
    );
}

/// Shows `entries` (Compose's `NavDisplay(entries, …)`): what [`NavDisplay`] does with entries made already (by
/// [`remember_decorated_nav_entries`], with their states: back stacks switched, each with its decorators' states, or
/// concatenated). Back calls `on_back`. The entries must not be empty.
///
/// Kotlin: `NavDisplay(entries: List<NavEntry<T>>, modifier: Modifier = Modifier, contentAlignment: Alignment = Alignment.TopStart, sceneStrategies: List<SceneStrategy<T>> = listOf(SinglePaneSceneStrategy()), sceneDecoratorStrategies: List<SceneDecoratorStrategy<T>> = emptyList(), sharedTransitionScope: SharedTransitionScope? = null, sizeTransform: SizeTransform? = null, transitionSpec: AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform = defaultTransitionSpec(), popTransitionSpec: AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform = defaultPopTransitionSpec(), predictivePopTransitionSpec: AnimatedContentTransitionScope<Scene<T>>.(Int) -> ContentTransform = defaultPredictivePopTransitionSpec(), onBack: () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavDisplayOfEntries<T: NavEntryKey>(
    entries: Vec<NavEntry<T>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(default_scene_strategies())] scene_strategies: Vec<Rc<dyn SceneStrategy<T>>>,
    #[default(Vec::new())] scene_decorator_strategies: Vec<Rc<dyn SceneDecoratorStrategy<T>>>,
    #[default(None)] shared_transition_scope: Option<SharedTransitionScope>,
    #[default(None)] size_transform: Option<Rc<dyn SizeTransform>>,
    #[default(default_transition_spec)] transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
    ) -> ContentTransform
    + 'static,
    #[default(default_pop_transition_spec)] pop_transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
    ) -> ContentTransform
    + 'static,
    #[default(default_predictive_pop_transition_spec)] predictive_pop_transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
        i32,
    ) -> ContentTransform
    + 'static,
    on_back: impl Fn() + 'static,
) {
    if entries.is_empty() {
        panic_kt("IllegalArgumentException", "NavDisplay entries cannot be empty");
    }
    let on_back: Rc<dyn Fn()> = Rc::new(on_back);
    let ob = on_back.clone();
    let scene_state = named_args!(remember_scene_state(
        entries,
        scene_strategies,
        scene_decorator_strategies = scene_decorator_strategies,
        shared_transition_scope = shared_transition_scope,
        on_back = move || ob()
    ));
    // (predictive back)
    let navigation_event_state = remember_navigation_event_state_of_scene_state(scene_state.clone());
    let ob = on_back.clone();
    named_args!(SceneNavigationBackHandler(
        scene_state.clone(),
        state = Some(navigation_event_state.clone()),
        on_back_completed = move || ob()
    ));
    NavDisplayOfSceneState(
        scene_state,
        navigation_event_state,
        modifier,
        content_alignment,
        size_transform,
        transition_spec,
        pop_transition_spec,
        predictive_pop_transition_spec,
    );
}

/// Whether going from `old` to `new` is a pop (Compose's private `isPop`): the same first key, `new` a shorter start
/// of `old`.
fn is_pop(old: &[SlotKey], new: &[SlotKey]) -> bool {
    match (old.first(), new.first()) {
        (Some(a), Some(b)) if a == b => {}
        _ => return false,
    }
    if new.len() > old.len() {
        return false;
    }
    let diverging = (0..new.len()).find(|&i| new[i] != old[i]);
    diverging.is_none() && new.len() != old.len()
}

/// The overlay scenes as a key (Kotlin's `LaunchedEffect(overlayScenes)`: compared as scenes).
struct OverlaysKey<T: NavEntryKey>(Vec<Rc<dyn OverlayScene<T>>>);

impl<T: NavEntryKey> PartialEq for OverlaysKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().zip(&other.0).all(|(a, b)| **a == **b)
    }
}

/// Shows `scene_state` (Compose's `NavDisplay(sceneState, navigationEventState, …)`): its current scene, a change
/// of it animated (by the transitions given, or the scene's metadata's), its overlay scenes above it; a predictive back
/// gesture of `navigation_event_state` (registered by a navigation back handler) seeks the animation to the previous
/// scene. The current scene's lifecycle is RESUMED once the animation has settled and no overlay is shown (STARTED
/// till then); the top-most overlay's RESUMED, the others' STARTED.
///
/// Kotlin: `NavDisplay(sceneState: SceneState<T>, navigationEventState: NavigationEventState<SceneInfo<T>>, modifier: Modifier = Modifier, contentAlignment: Alignment = Alignment.TopStart, sizeTransform: SizeTransform? = null, transitionSpec: AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform = defaultTransitionSpec(), popTransitionSpec: AnimatedContentTransitionScope<Scene<T>>.() -> ContentTransform = defaultPopTransitionSpec(), predictivePopTransitionSpec: AnimatedContentTransitionScope<Scene<T>>.(Int) -> ContentTransform = defaultPredictivePopTransitionSpec())`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavDisplayOfSceneState<T: NavEntryKey>(
    scene_state: SceneState<T>,
    navigation_event_state: NavigationEventState<SceneInfo<T>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(None)] size_transform: Option<Rc<dyn SizeTransform>>,
    #[default(default_transition_spec)] transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
    ) -> ContentTransform
    + 'static,
    #[default(default_pop_transition_spec)] pop_transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
    ) -> ContentTransform
    + 'static,
    #[default(default_predictive_pop_transition_spec)] predictive_pop_transition_spec: impl Fn(
        &AnimatedContentTransitionScope<AnyScene>,
        i32,
    ) -> ContentTransform
    + 'static,
) {
    // (the current scene, the transition)
    let scene = AnyScene::of(scene_state.current_scene());
    let s0 = scene.clone();
    let transition_state: SeekableTransitionState<AnyScene> = remember(move || SeekableTransitionState::new(s0));
    let transition = named_args!(remember_transition(transition_state.clone(), label = Some("scene".to_string())));

    // (the back gesture)
    let previous_scene = scene_state.previous_scenes().last().cloned().map(AnyScene::of);
    let gesture = navigation_event_state.transition_state();
    let in_predictive_back = gesture.is_in_progress() && previous_scene.is_some();
    let (progress, swipe_edge) = match &gesture {
        NavigationEventTransitionState::InProgress { latest_event, .. } => {
            (latest_event.progress, latest_event.swipe_edge)
        }
        _ => (0.0, NavigationEvent::EDGE_NONE),
    };

    // (whether this is a pop: the entries before, kept)
    let content_keys = |state: &SceneState<T>| state.entries().iter().map(NavEntry::content_key).collect::<Vec<_>>();
    let first_keys = content_keys(&scene_state);
    let previous_entries: MutableState<Vec<SlotKey>> = remember(move || mutable_confined_state_of(first_keys));
    let pe = previous_entries.clone();
    let new_keys = content_keys(&scene_state);
    let is_pop = remember_keyed(scene_state.entries(), move || {
        let old = pe.peek();
        let result = is_pop(&old, &new_keys);
        pe.set(new_keys);
        result
    });

    // (the scenes shown and their z-indices)
    let scene_map: MutableState<Vec<(AnimatedSceneKey, AnyScene)>> =
        remember(|| mutable_confined_state_of_with_policy(Vec::new(), never_equal_policy()));
    let z_indices: Rc<RefCell<Vec<(AnimatedSceneKey, f32)>>> = remember(|| Rc::new(RefCell::new(Vec::new())));
    let initial_key = transition.current_state().animated_key();
    let target_key = transition.target_state().animated_key();
    let z_of = |k: &AnimatedSceneKey| z_indices.borrow().iter().find(|(x, _)| x == k).map(|(_, z)| *z);
    let initial_z_index = match z_of(&initial_key) {
        Some(z) => z,
        None => {
            z_indices.borrow_mut().push((initial_key.clone(), 0.0));
            0.0
        }
    };
    let shown: Vec<AnimatedSceneKey> = scene_map.get().iter().map(|(k, _)| k.clone()).collect();
    let target_z_index = if let Some(z) = z_of(&target_key) {
        // (AnimatedContent keeps the z-index of a content on the screen already: the one it has)
        z
    } else if is_pop || in_predictive_back {
        shown.iter().filter_map(&z_of).reduce(f32::min).unwrap_or(initial_z_index) - 1.0
    } else {
        shown.iter().filter_map(&z_of).reduce(f32::max).unwrap_or(initial_z_index) + 1.0
    };
    {
        // (written when it changes: Compose's state map isn't changed by the scene it holds already)
        let mut map = scene_map.peek();
        let target = transition.target_state();
        let changed = match map.iter_mut().find(|(k, _)| *k == target_key) {
            Some(e) if e.1.ptr_eq(&target) => false,
            Some(e) => {
                e.1 = target;
                true
            }
            None => {
                map.push((target_key.clone(), target));
                true
            }
        };
        if changed {
            scene_map.set(map);
        }
        let mut z = z_indices.borrow_mut();
        match z.iter_mut().find(|(k, _)| *k == target_key) {
            Some(e) => e.1 = target_z_index,
            None => z.push((target_key.clone(), target_z_index)),
        }
    }

    // (the overlay scenes on the back stack, and those popped still animating out)
    let overlay_scenes = scene_state.overlay_scenes();
    let current_overlay_scenes: SnapshotStateList<Rc<dyn OverlayScene<T>>> =
        remember(|| mutable_confined_state_list_of(Vec::new()));
    let (overlays, current) = (overlay_scenes.clone(), current_overlay_scenes.clone());
    LaunchedEffect(OverlaysKey(overlay_scenes.clone()), async move {
        // (unique, in order: the z-order)
        for o in overlays {
            let key = o.key();
            if !current.to_vec().iter().any(|c| c.key() == key) {
                current.add(o);
            }
        }
    });

    // (the entries each scene shown composes: an entry by the top-most scene showing it — the target one, while
    // popping)
    let scene_entries: Vec<(AnimatedSceneKey, AnyScene)> = scene_map.get();
    let overlay_list: Vec<AnyScene> = current_overlay_scenes.to_vec().into_iter().map(overlay_any).collect();
    let z_string = format!("{:?}", z_indices.borrow());
    let target_state = transition.target_state();
    let (zi, ts) = (z_indices.clone(), target_state.clone());
    let excluded: Rc<HashMap<AnimatedSceneKey, Rc<HashSet<SlotKey>>>> = remember_keyed(
        (
            ScenesKey(scene_entries.clone()),
            ScenesKey(overlay_list.iter().map(|o| (o.animated_key(), o.clone())).collect()),
            z_string,
            target_state.clone(),
        ),
        move || {
            let z_of = |k: &AnimatedSceneKey| zi.borrow().iter().find(|(x, _)| x == k).map(|(_, z)| *z).unwrap_or(0.0);
            let mut sorted = scene_entries.clone();
            sorted.sort_by(|a, b| z_of(&b.0).total_cmp(&z_of(&a.0)));
            let mut scenes: Vec<AnyScene> = Vec::new();
            for (_, s) in sorted {
                if !scenes.contains(&s) {
                    scenes.push(s);
                }
            }
            let popping = scenes.first().is_some_and(|first| ts != *first);
            // (overlays top-most first, then the scenes by z-index; reversed while popping, so that the incoming
            // scene claims its entries first: shared elements shown in the target)
            let mut in_z_order: Vec<AnyScene> = overlay_list.clone();
            in_z_order.extend(scenes);
            if popping {
                in_z_order.reverse();
            }
            let mut covered: HashSet<SlotKey> = HashSet::new();
            let mut map = HashMap::new();
            for s in in_z_order {
                map.insert(s.animated_key(), Rc::new(covered.clone()));
                covered.extend(s.entry_content_keys());
            }
            Rc::new(map)
        },
    );

    // (the transition of the higher z-index gives its metadata's transition)
    let transition_scene =
        if initial_z_index >= target_z_index { transition.current_state() } else { transition.target_state() };

    // (seeking while a back gesture goes on; animating otherwise)
    if in_predictive_back {
        let ts = transition_state.clone();
        let prev = previous_scene.clone().expect("a previous scene");
        LaunchedEffect((prev.clone(), progress.to_bits()), async move {
            // (retargeted when the key changes; sought on the progress)
            named_args!(ts.seek_to(progress, target_state = prev)).await;
        });
    } else {
        let (ts, t, sc) = (transition_state.clone(), transition.clone(), scene.clone());
        LaunchedEffect(scene.clone(), async move {
            if ts.current_state() != sc {
                // (a navigation or a pop: animated to its end)
                named_args!(ts.animate_to(target_state = sc)).await;
            } else {
                // (a predictive back completed or cancelled: on from where it was sought, to its end or back)
                let total_duration = t.total_duration_nanos() / 1_000_000;
                let completed = t.target_state() == sc;
                let fraction = ts.fraction();
                let (final_fraction, remaining) = if completed {
                    (1.0, ((1.0 - fraction) * total_duration as f32) as i32)
                } else {
                    (0.0, (fraction * total_duration as f32) as i32)
                };
                let (ts2, sc2) = (ts.clone(), sc.clone());
                coroutine_scope(move |scope| async move {
                    let tween_spec: kompoz_animation::TweenSpec<f32> = named_args!(tween(duration_millis = remaining));
                    let spec: Arc<dyn kompoz_animation::AnimationSpec<f32>> = Arc::new(tween_spec);
                    named_args!(animate(
                        fraction,
                        final_fraction,
                        animation_spec = spec,
                        block = move |value: f32, _: f32| {
                            let (ts, sc) = (ts2.clone(), sc2.clone());
                            scope.launch(async move {
                                if value != final_fraction {
                                    // (sought towards the end)
                                    named_args!(ts.seek_to(value)).await;
                                }
                                if value == final_fraction {
                                    // (done: snapped to the scene)
                                    ts.snap_to(sc).await;
                                }
                            });
                        }
                    ))
                    .await;
                })
                .await;
            }
        });
    }

    // (the transform: the predictive pop's, the pop's or the navigation's — the scene's metadata's first)
    let scene_specs = TransitionSpecs {
        transition_scene: transition_scene.clone(),
        in_predictive_back,
        is_pop,
        swipe_edge,
        transition_spec: Rc::new(transition_spec),
        pop_transition_spec: Rc::new(pop_transition_spec),
        predictive_pop_transition_spec: Rc::new(predictive_pop_transition_spec),
    };
    let specs = Rc::new(scene_specs);

    // (the content's scope, for overlay scenes too: LocalNavAnimatedContentScope isn't nullable)
    let animated_content_scope: Rc<RefCell<Option<AnimatedContentScope>>> = remember(|| Rc::new(RefCell::new(None)));

    let t_for_content = transition.clone();
    let (acs, cos, ex) = (animated_content_scope.clone(), current_overlay_scenes.clone(), excluded.clone());
    let content_key: Option<Rc<dyn Fn(&AnyScene) -> SlotKey>> =
        Some(Rc::new(|s: &AnyScene| SlotKey::new(s.animated_key())));
    let size = size_transform.clone();
    named_args!(transition.AnimatedContent(
        modifier = modifier,
        transition_spec = move |scope: &AnimatedContentTransitionScope<AnyScene>| {
            let transform = specs.content_transform(scope);
            named_args!(ContentTransform(
                transform.target_content_enter(),
                transform.initial_content_exit(),
                target_content_z_index = target_z_index,
                size_transform = size.clone()
            ))
        },
        content_alignment = content_alignment,
        content_key = content_key,
        content = move |scope: &AnimatedContentScope, target_scene: AnyScene| {
            // (RESUMED only once settled, no overlay shown)
            let is_settled = t_for_content.current_state() == t_for_content.target_state();
            let max = if is_settled && cos.is_empty() { LifecycleState::Resumed } else { LifecycleState::Started };
            let owner = named_args!(remember_lifecycle_owner(max_lifecycle = max));
            let first = scope.clone();
            let kept = remember(move || first);
            *acs.borrow_mut() = Some(kept.clone());
            let excluded = ex.get(&target_scene.animated_key()).cloned().unwrap_or_default();
            let ts = target_scene.clone();
            let provided = [
                LocalLifecycleOwner.provides(owner),
                LocalNavAnimatedContentScope.provides(kept),
                LocalCurrentScene.provides(Some(target_scene.clone())),
                LocalEntriesToExcludeFromCurrentScene.provides(excluded),
            ];
            kompoz_runtime::kompozable_block!({
                KompozitionLocalProvider(&provided, move || ts.Content());
            });
        }
    ));

    // (the book-keeping of the scenes cleaned up once the transition is done)
    let (t, map, z) = (transition.clone(), scene_map.clone(), z_indices.clone());
    LaunchedEffect(transition.clone(), async move {
        let t2 = t.clone();
        snapshot_flow(move || t2.is_running())
            .filter(|running| !*running)
            .collect(move |_| {
                let target = t.target_state().animated_key();
                let kept: Vec<(AnimatedSceneKey, AnyScene)> =
                    map.peek().into_iter().filter(|(k, _)| *k == target).collect();
                map.set(kept);
                z.borrow_mut().retain(|(k, _)| *k == target);
            })
            .await;
    });

    // (the overlay scenes above the animated content, the lowest first)
    let overlay_keys: Vec<SlotKey> = overlay_scenes.iter().map(|o| o.key()).collect();
    let top = overlay_scenes.first().cloned();
    for overlay in current_overlay_scenes.to_vec().into_iter().rev() {
        let any = overlay_any(overlay.clone());
        key(&any.animated_key(), || {
            let is_top = top.as_ref().is_some_and(|t| **t == *overlay);
            let max = if is_top { LifecycleState::Resumed } else { LifecycleState::Started };
            let owner = named_args!(remember_lifecycle_owner(max_lifecycle = max));
            let excluded = excluded.get(&any.animated_key()).cloned().unwrap_or_default();
            let scope = animated_content_scope.borrow().clone();
            let mut provided = vec![
                LocalLifecycleOwner.provides(owner),
                LocalEntriesToExcludeFromCurrentScene.provides(excluded),
                LocalCurrentScene.provides(Some(any.clone())),
            ];
            if let Some(scope) = scope {
                provided.push(LocalNavAnimatedContentScope.provides(scope));
            }
            let a = any.clone();
            KompozitionLocalProvider(&provided, move || a.Content());
        });
        // (popped: its exit runs before it leaves the composition)
        if !overlay_keys.contains(&overlay.key()) {
            let (o, current) = (overlay.clone(), current_overlay_scenes.clone());
            LaunchedEffect(overlay.key(), async move {
                o.on_remove().await;
                current.retain(|c| !Rc::ptr_eq(c, &o));
            });
        }
    }
}

/// An overlay scene as a scene of any key type.
fn overlay_any<T: NavEntryKey>(o: Rc<dyn OverlayScene<T>>) -> AnyScene {
    AnyScene::of::<T>(o)
}

/// Scenes as a `remember` key: compared as scenes (Kotlin's `sceneMap.entries.toList()`).
#[derive(Clone, PartialEq)]
struct ScenesKey(Vec<(AnimatedSceneKey, AnyScene)>);

/// What picks the transform of a change (Compose's `contentTransform` lambda of `NavDisplay`).
struct TransitionSpecs {
    transition_scene: AnyScene,
    in_predictive_back: bool,
    is_pop: bool,
    swipe_edge: i32,
    transition_spec: Rc<dyn Fn(&AnimatedContentTransitionScope<AnyScene>) -> ContentTransform>,
    pop_transition_spec: Rc<dyn Fn(&AnimatedContentTransitionScope<AnyScene>) -> ContentTransform>,
    predictive_pop_transition_spec: Rc<dyn Fn(&AnimatedContentTransitionScope<AnyScene>, i32) -> ContentTransform>,
}

impl TransitionSpecs {
    fn content_transform(&self, scope: &AnimatedContentTransitionScope<AnyScene>) -> ContentTransform {
        let metadata = self.transition_scene.metadata();
        if self.in_predictive_back {
            metadata
                .get(NavDisplayPredictivePopTransitionKey)
                .and_then(|spec: ScenePredictivePopTransitionSpec| spec(scope, self.swipe_edge))
                .unwrap_or_else(|| (self.predictive_pop_transition_spec)(scope, self.swipe_edge))
        } else if self.is_pop {
            metadata
                .get(NavDisplayPopTransitionKey)
                .and_then(|spec: SceneTransitionSpec| spec(scope))
                .unwrap_or_else(|| (self.pop_transition_spec)(scope))
        } else {
            metadata
                .get(NavDisplayTransitionKey)
                .and_then(|spec: SceneTransitionSpec| spec(scope))
                .unwrap_or_else(|| (self.transition_spec)(scope))
        }
    }
}
