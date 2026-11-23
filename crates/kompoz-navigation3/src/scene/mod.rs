//! navigation3-ui's `androidx.navigation3.scene`: [`Scene`]s and [`OverlayScene`]s, [`SceneStrategy`]s (a single pane,
//! a dialog), [`SceneDecoratorStrategy`]s, [`SceneState`] and [`remember_scene_state`], [`SceneInfo`] and the scene's
//! navigation back handler.

mod dialog;
mod info;
#[allow(clippy::module_inception)]
mod scene;
mod state;

pub use dialog::{DialogKey, DialogSceneStrategy};
pub use info::{SceneInfo, SceneNavigationBackHandler, remember_navigation_event_state_of_scene_state};
pub use scene::*;
pub(crate) use state::{LocalCurrentScene, LocalEntriesToExcludeFromCurrentScene};
pub use state::{SceneState, remember_scene_state};
