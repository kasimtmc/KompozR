//! SceneState.kt, SceneSetupNavEntryDecorator.kt, SharedEntryInSceneNavEntryDecorator.kt,
//! BackStackAwareLifecycleNavEntryDecorator.kt.

use super::scene::{
    OverlayScene, Scene, SceneDecoratorStrategy, SceneDecoratorStrategyScope, SceneStrategy,
    calculate_scene_with_single_pane_fallback,
};
use crate::runtime::{NavEntry, NavEntryDecorator, NavEntryKey, remember_decorated_nav_entries_of};
use crate::ui::AnyScene;
use crate::ui::LocalNavAnimatedContentScope;
use kompoz_animation::SharedTransitionScope;
use kompoz_foundation::layout::Box;
use kompoz_lifecycle::{LifecycleState, LocalLifecycleOwner, remember_lifecycle_owner};
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, MovableContentOf, key, kompozable, movable_content_with_param_of,
    remember, remember_keyed, remember_updated_state,
};
use kompoz_ui::Modifier;
use kompoz_ui::layout::SlotKey;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

/// What a display shows of its entries (Compose's `SceneState`): the entries, the overlay scenes (top-most first),
/// the current scene (below them) and the scenes before it (the earliest first). What `NavDisplay` and the
/// navigation back handler read.
pub struct SceneState<T> {
    entries: Vec<NavEntry<T>>,
    overlay_scenes: Vec<Rc<dyn OverlayScene<T>>>,
    current_scene: Rc<dyn Scene<T>>,
    previous_scenes: Vec<Rc<dyn Scene<T>>>,
}

impl<T: NavEntryKey> Clone for SceneState<T> {
    fn clone(&self) -> Self {
        SceneState {
            entries: self.entries.clone(),
            overlay_scenes: self.overlay_scenes.clone(),
            current_scene: self.current_scene.clone(),
            previous_scenes: self.previous_scenes.clone(),
        }
    }
}

impl<T: NavEntryKey> SceneState<T> {
    /// All of its entries (Kotlin's `entries`).
    pub fn entries(&self) -> Vec<NavEntry<T>> {
        self.entries.clone()
    }
    /// Its overlay scenes, the top-most first (Kotlin's `overlayScenes`).
    pub fn overlay_scenes(&self) -> Vec<Rc<dyn OverlayScene<T>>> {
        self.overlay_scenes.clone()
    }
    /// The scene shown below the overlays (Kotlin's `currentScene`).
    pub fn current_scene(&self) -> Rc<dyn Scene<T>> {
        self.current_scene.clone()
    }
    /// The scenes before the current one, the earliest first (Kotlin's `previousScenes`).
    pub fn previous_scenes(&self) -> Vec<Rc<dyn Scene<T>>> {
        self.previous_scenes.clone()
    }
}

fn same_scenes<T: NavEntryKey>(a: &[Rc<dyn Scene<T>>], b: &[Rc<dyn Scene<T>>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| **x == **y)
}

impl<T: NavEntryKey> PartialEq for SceneState<T> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
            && self.overlay_scenes.len() == other.overlay_scenes.len()
            && self.overlay_scenes.iter().zip(&other.overlay_scenes).all(|(x, y)| **x == **y)
            && *self.current_scene == *other.current_scene
            && same_scenes(&self.previous_scenes, &other.previous_scenes)
    }
}

impl<T: NavEntryKey> fmt::Debug for SceneState<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SceneState(entries={:?}, overlayScenes={:?}, currentScene={:?}, previousScenes={:?})",
            self.entries, self.overlay_scenes, self.current_scene, self.previous_scenes
        )
    }
}

/// A list of strategies by identity (Kotlin's `remember(sceneStrategies.toList(), …)`: strategy objects compare as
/// themselves).
#[derive(Clone)]
struct StrategiesKey<T: NavEntryKey>(Vec<Rc<dyn SceneStrategy<T>>>);

impl<T: NavEntryKey> PartialEq for StrategiesKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(&other.0).all(|(a, b)| std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b)))
    }
}

/// The scene state of `entries` remembered while they and the strategies are the same (Compose's
/// `rememberSceneState`): its scenes calculated by `scene_strategies` in order (a single pane when none does), the
/// non-overlay ones decorated by `scene_decorator_strategies`; the overlay scenes' overlaid entries calculated again
/// down to a scene that isn't one; the previous scenes from each scene's previous entries. The entries are decorated
/// again first: shared between scenes in `shared_transition_scope` (if given: the entries' content must be shown by a
/// `NavDisplay`, for its `LocalNavAnimatedContentScope`), each in a movable content (composed by one scene only, moved
/// between them), their lifecycle capped at CREATED once off the back stack. `on_back` is what a scene's own back
/// calls (the latest given).
///
/// Kotlin: `rememberSceneState(entries: List<NavEntry<T>>, sceneStrategies: List<SceneStrategy<T>>, sceneDecoratorStrategies: List<SceneDecoratorStrategy<T>> = emptyList(), sharedTransitionScope: SharedTransitionScope? = null, onBack: () -> Unit): SceneState<T>`
#[kompozable]
pub fn remember_scene_state<T: NavEntryKey>(
    entries: Vec<NavEntry<T>>,
    scene_strategies: Vec<Rc<dyn SceneStrategy<T>>>,
    #[default(Vec::new())] scene_decorator_strategies: Vec<Rc<dyn SceneDecoratorStrategy<T>>>,
    #[default(None)] shared_transition_scope: Option<SharedTransitionScope>,
    on_back: impl Fn() + 'static,
) -> SceneState<T> {
    let on_back: Rc<dyn Fn()> = Rc::new(on_back);
    let current_on_back = remember_updated_state!(on_back);
    let shared = shared_transition_scope.map(remember_shared_entry_in_scene_nav_entry_decorator);
    let mut decorators: Vec<NavEntryDecorator<T>> = Vec::new();
    decorators.extend(shared);
    decorators.push(remember_scene_setup_nav_entry_decorator());
    decorators.push(remember_back_stack_aware_lifecycle_nav_entry_decorator(entries.clone()));
    let decorated = remember_decorated_nav_entries_of(entries, decorators);
    let strategies = StrategiesKey(scene_strategies);
    let decorator_strategies = scene_decorator_strategies;
    let d = decorated.clone();
    remember_keyed((strategies.clone(), decorated), move || {
        let scope = SceneDecoratorStrategyScope::<T>::with_on_back(move || (current_on_back.get())());
        let mut all_scenes = vec![provide_scene(&scope, d.clone(), &strategies.0, &decorator_strategies)];
        // (the overlay scenes, down to a scene that isn't one)
        loop {
            let last = all_scenes.last().expect("a scene").clone();
            let Some(overlay) = last.as_overlay_scene() else { break };
            let overlaid = overlay.overlaid_entries();
            if overlaid.is_empty() {
                kompoz_runtime::kt::panic_kt(
                    "IllegalArgumentException",
                    &format!("Overlaid entries from {:?} must not be empty", overlay),
                );
            }
            all_scenes.push(provide_scene(&scope, overlaid, &strategies.0, &decorator_strategies));
        }
        let current_scene = all_scenes.pop().expect("a scene");
        let overlay_scenes: Vec<Rc<dyn OverlayScene<T>>> =
            all_scenes.into_iter().map(|s| s.as_overlay_scene().expect("an overlay scene")).collect();
        // (the previous scenes, from the current one's previous entries back)
        let mut previous_scenes: Vec<Rc<dyn Scene<T>>> = vec![current_scene.clone()];
        loop {
            let previous_entries = previous_scenes[0].previous_entries();
            if previous_entries.is_empty() {
                break;
            }
            previous_scenes.insert(0, provide_scene(&scope, previous_entries, &strategies.0, &decorator_strategies));
        }
        if let Some(i) = previous_scenes.iter().position(|s| **s == *current_scene) {
            previous_scenes.remove(i);
        }
        SceneState { entries: d.clone(), overlay_scenes, current_scene, previous_scenes }
    })
}

/// Compose's private `provideScene`: the scene the strategies calculate, decorated by the decorator strategies
/// unless it is an overlay scene.
fn provide_scene<T: NavEntryKey>(
    scope: &SceneDecoratorStrategyScope<T>,
    entries: Vec<NavEntry<T>>,
    scene_strategies: &[Rc<dyn SceneStrategy<T>>],
    scene_decorators: &[Rc<dyn SceneDecoratorStrategy<T>>],
) -> Rc<dyn Scene<T>> {
    let mut scene = calculate_scene_with_single_pane_fallback(scene_strategies, scope, entries);
    for decorator in scene_decorators {
        if scene.clone().as_overlay_scene().is_none() {
            scene = decorator.decorate_scene(scope, scene);
        }
    }
    scene
}

// ---- SceneSetupNavEntryDecorator ----

/// The content keys of the entries the scene composing here must not compose (another scene does: Compose's internal
/// `LocalEntriesToExcludeFromCurrentScene`); none by default.
pub(crate) static LocalEntriesToExcludeFromCurrentScene: KompozitionLocal<Rc<HashSet<SlotKey>>> =
    KompozitionLocal::new(|| Rc::new(HashSet::new()));

/// A movable content of an entry's content (Compose's `movableContentOf { content -> content() }`).
type EntryMovable = MovableContentOf<Rc<dyn Fn()>>;

/// The decorator putting each entry in a movable content of its own (Compose's internal `SceneSetupNavEntryDecorator`),
/// so that scenes may compose it anywhere, by one scene at a time (the one it isn't excluded from), its state moving
/// with it; its movable content kept while it is shown (even popped, animating out).
fn scene_setup_nav_entry_decorator<T: NavEntryKey>(
    movable_content_map: Rc<RefCell<HashMap<SlotKey, EntryMovable>>>,
) -> NavEntryDecorator<T> {
    let (popped, map) = (movable_content_map.clone(), movable_content_map);
    NavEntryDecorator::new(
        move |content_key: SlotKey| {
            popped.borrow_mut().remove(&content_key);
        },
        move |entry: NavEntry<T>| {
            let content_key = entry.content_key();
            let excluded = LocalEntriesToExcludeFromCurrentScene.current();
            if !excluded.contains(&content_key) {
                let map = map.clone();
                key(&content_key.clone(), move || {
                    let k = content_key.clone();
                    let movable = remember(move || {
                        let found = map.borrow().get(&k).cloned();
                        match found {
                            Some(m) => m,
                            None => {
                                let m: EntryMovable = movable_content_with_param_of(|content: &Rc<dyn Fn()>| content());
                                map.borrow_mut().insert(k.clone(), m.clone());
                                m
                            }
                        }
                    });
                    let e = entry.clone();
                    movable.call(Rc::new(move || e.Content()));
                });
            }
        },
    )
}

#[kompozable]
fn remember_scene_setup_nav_entry_decorator<T: NavEntryKey>() -> NavEntryDecorator<T> {
    remember(|| scene_setup_nav_entry_decorator(Rc::new(RefCell::new(HashMap::new()))))
}

// ---- SharedEntryInSceneNavEntryDecorator ----

/// The scene the content composing here is shown by (Compose's internal `LocalCurrentScene`, a `Scene<*>`); none
/// outside a display.
pub(crate) static LocalCurrentScene: KompozitionLocal<Option<AnyScene>> = KompozitionLocal::new(|| None);

/// The decorator sharing each entry's content between the scenes of `shared_transition_scope` (Compose's internal
/// `SharedEntryInSceneNavEntryDecorator`): in a scene that isn't an overlay, the entry is a shared element of its
/// content key, animated by the display's `LocalNavAnimatedContentScope`. Around the scene setup's.
fn shared_entry_in_scene_nav_entry_decorator<T: NavEntryKey>(
    shared_transition_scope: SharedTransitionScope,
) -> NavEntryDecorator<T> {
    NavEntryDecorator::new(
        |_| {},
        move |entry: NavEntry<T>| {
            let current = LocalCurrentScene.current();
            if current.is_some_and(|s| !s.is_overlay()) {
                let scope = shared_transition_scope.clone();
                let state = scope.remember_shared_content_state(entry.content_key());
                let animated = LocalNavAnimatedContentScope.current();
                let modifier = kompoz_runtime::named_args!(scope.shared_element(
                    Modifier,
                    state,
                    animated_visibility_scope = animated.animated_visibility_scope()
                ));
                let e = entry.clone();
                kompoz_runtime::named_args!(Box(modifier, content = move || e.Content()));
            } else {
                entry.Content();
            }
        },
    )
}

#[kompozable]
fn remember_shared_entry_in_scene_nav_entry_decorator<T: NavEntryKey>(
    shared_transition_scope: SharedTransitionScope,
) -> NavEntryDecorator<T> {
    let s = shared_transition_scope.clone();
    remember_keyed(shared_transition_scope, move || shared_entry_in_scene_nav_entry_decorator(s))
}

// ---- BackStackAwareLifecycleNavEntryDecorator ----

/// The decorator capping each entry's lifecycle at CREATED once it is off `entries` (Compose's internal
/// `rememberBackStackAwareLifecycleNavEntryDecorator`): an entry on the back stack may be RESUMED; one animating out
/// is CREATED.
#[kompozable]
fn remember_back_stack_aware_lifecycle_nav_entry_decorator<T: NavEntryKey>(
    entries: Vec<NavEntry<T>>,
) -> NavEntryDecorator<T> {
    let updated_entries = remember_updated_state!(entries);
    remember(move || {
        let updated_entries = updated_entries.clone();
        NavEntryDecorator::new(
            |_| {},
            move |entry: NavEntry<T>| {
                let content_key = entry.content_key();
                let in_back_stack = updated_entries.get().iter().any(|e| e.content_key() == content_key);
                let max = if in_back_stack { LifecycleState::Resumed } else { LifecycleState::Created };
                let owner = kompoz_runtime::named_args!(remember_lifecycle_owner(max_lifecycle = max));
                let e = entry.clone();
                kompoz_runtime::kompozable_block!({
                    KompozitionLocalProvider(&[LocalLifecycleOwner.provides(owner)], move || e.Content());
                });
            },
        )
    })
}
