//! SceneInfo.kt, NavigationEventHandler.kt, RememberNavigationEventState.kt: scenes in the navigation event history,
//! back handled for a scene state.

use super::scene::Scene;
use super::state::SceneState;
use crate::runtime::NavEntryKey;
use kompoz_navigationevent::{
    NavigationBackHandler, NavigationEventInfo, NavigationEventState, remember_navigation_event_state,
};
use kompoz_runtime::kompozable;
use std::fmt;
use std::rc::Rc;

/// A scene in the navigation event history (Compose's `SceneInfo`, a `NavigationEventInfo`): what a predictive back
/// animation is built from. Equal when the scenes are.
pub struct SceneInfo<T> {
    scene: Rc<dyn Scene<T>>,
}

impl<T: NavEntryKey> Clone for SceneInfo<T> {
    fn clone(&self) -> Self {
        SceneInfo { scene: self.scene.clone() }
    }
}

impl<T: NavEntryKey> SceneInfo<T> {
    /// Its scene (Kotlin's `scene`).
    pub fn scene(&self) -> Rc<dyn Scene<T>> {
        self.scene.clone()
    }
}

/// Kotlin: `SceneInfo(scene: Scene<T>)`
pub fn SceneInfo<T: NavEntryKey>(scene: Rc<dyn Scene<T>>) -> SceneInfo<T> {
    SceneInfo { scene }
}

impl<T: NavEntryKey> PartialEq for SceneInfo<T> {
    fn eq(&self, other: &Self) -> bool {
        *self.scene == *other.scene
    }
}

impl<T: NavEntryKey> fmt::Debug for SceneInfo<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SceneInfo(scene={:?})", self.scene)
    }
}

impl<T: NavEntryKey> NavigationEventInfo for SceneInfo<T> {}

/// A [`NavigationEventState`] of `scene_state` remembered across recompositions (Compose's
/// `rememberNavigationEventState(sceneState)`): its current info the current scene's, its back infos the previous
/// scenes', kept to them.
///
/// Kotlin: `rememberNavigationEventState(sceneState: SceneState<T>): NavigationEventState<SceneInfo<T>>`
#[kompozable]
pub fn remember_navigation_event_state_of_scene_state<T: NavEntryKey>(
    scene_state: SceneState<T>,
) -> NavigationEventState<SceneInfo<T>> {
    let current = SceneInfo(scene_state.current_scene());
    let back: Vec<SceneInfo<T>> = scene_state.previous_scenes().into_iter().map(SceneInfo).collect();
    kompoz_runtime::named_args!(remember_navigation_event_state(current, back_info = back))
}

/// Handles back for `scene_state` (Compose's `NavigationBackHandler(sceneState, state, onBackCancelled,
/// onBackCompleted)`, scene's): enabled while the current scene has previous entries; a completed back calls
/// `on_back_completed` once per entry the scene's back pops (the entries beyond its previous entries).
///
/// Kotlin: `NavigationBackHandler(sceneState: SceneState<T>, state: NavigationEventState<SceneInfo<T>> = rememberNavigationEventState(sceneState), onBackCancelled: () -> Unit = {}, onBackCompleted: () -> Unit)`
#[kompozable]
pub fn SceneNavigationBackHandler<T: NavEntryKey>(
    scene_state: SceneState<T>,
    #[default(None)] state: Option<NavigationEventState<SceneInfo<T>>>,
    #[default(|| {})] on_back_cancelled: impl Fn() + 'static,
    on_back_completed: impl Fn() + 'static,
) {
    // (Kotlin's default, `rememberNavigationEventState(sceneState)`, where none is given)
    let given = state.is_some();
    let remembered =
        kompoz_runtime::group(|| (!given).then(|| remember_navigation_event_state_of_scene_state(scene_state.clone())));
    let state = state.or(remembered).expect("a navigation event state");
    let enabled = !scene_state.current_scene().previous_entries().is_empty();
    let s = scene_state.clone();
    NavigationBackHandler(
        state,
        is_back_enabled = enabled,
        on_back_cancelled = on_back_cancelled,
        on_back_completed = move || {
            // (a stale `enabled` — a gesture dispatched in the frame it turned off — pops nothing rather than
            // too much)
            let n = s.entries().len().saturating_sub(s.current_scene().previous_entries().len());
            for _ in 0..n {
                on_back_completed();
            }
        },
    );
}
