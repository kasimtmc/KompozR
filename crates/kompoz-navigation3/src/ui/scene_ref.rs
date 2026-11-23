//! A scene of any key type (Kotlin's `Scene<*>`): `NavDisplay`'s transition states, its transitions' scopes.

use crate::runtime::{Metadata, NavEntryKey};
use crate::scene::Scene;
use kompoz_ui::layout::SlotKey;
use std::any::{Any, TypeId};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// What `NavDisplay` reads of a scene whatever its key type.
trait ErasedScene {
    fn class(&self) -> TypeId;
    fn class_name(&self) -> &'static str;
    fn key(&self) -> SlotKey;
    fn entry_content_keys(&self) -> Vec<SlotKey>;
    fn metadata(&self) -> Metadata;
    fn content(&self);
    fn is_overlay(&self) -> bool;
    fn same(&self, other: &dyn ErasedScene) -> bool;
    fn typed(&self) -> &dyn Any;
    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

struct Typed<T: NavEntryKey>(Rc<dyn Scene<T>>);

impl<T: NavEntryKey> ErasedScene for Typed<T> {
    fn class(&self) -> TypeId {
        self.0.as_any().type_id()
    }
    fn class_name(&self) -> &'static str {
        std::any::type_name_of_val(&*self.0)
    }
    fn key(&self) -> SlotKey {
        self.0.key()
    }
    fn entry_content_keys(&self) -> Vec<SlotKey> {
        self.0.entries().iter().map(|e| e.content_key()).collect()
    }
    fn metadata(&self) -> Metadata {
        self.0.metadata()
    }
    fn content(&self) {
        self.0.content()
    }
    fn is_overlay(&self) -> bool {
        self.0.clone().as_overlay_scene().is_some()
    }
    fn same(&self, other: &dyn ErasedScene) -> bool {
        other.typed().downcast_ref::<Rc<dyn Scene<T>>>().is_some_and(|o| *self.0 == **o)
    }
    fn typed(&self) -> &dyn Any {
        &self.0
    }
    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

/// A scene of any key type (Kotlin's `Scene<*>`, and `Scene<T>` where `NavDisplay` animates it): the state of
/// `NavDisplay`'s transition, what its transitions' scopes give as their `initial_state` and `target_state`. Equal when
/// the scenes are (of one key type). [`scene`](Self::scene) gives it typed.
#[derive(Clone)]
pub struct AnyScene(Rc<dyn ErasedScene>);

impl AnyScene {
    /// `scene`, of any key type.
    pub fn of<T: NavEntryKey>(scene: Rc<dyn Scene<T>>) -> AnyScene {
        AnyScene(Rc::new(Typed(scene)))
    }

    /// The scene, if its keys are `T`s (Kotlin's `scene as Scene<T>`).
    pub fn scene<T: NavEntryKey>(&self) -> Option<Rc<dyn Scene<T>>> {
        self.0.typed().downcast_ref::<Rc<dyn Scene<T>>>().cloned()
    }

    /// Its key (Kotlin's `scene.key`).
    pub fn key(&self) -> SlotKey {
        self.0.key()
    }

    /// Its metadata (Kotlin's `scene.metadata`).
    pub fn metadata(&self) -> Metadata {
        self.0.metadata()
    }

    /// Its entries' content keys.
    pub fn entry_content_keys(&self) -> Vec<SlotKey> {
        self.0.entry_content_keys()
    }

    /// Whether it is an overlay scene (Kotlin's `scene is OverlayScene`).
    pub fn is_overlay(&self) -> bool {
        self.0.is_overlay()
    }

    /// Its type's name.
    pub fn class_name(&self) -> &'static str {
        self.0.class_name()
    }

    /// Composes its content (Kotlin's `scene.content()`).
    #[allow(non_snake_case)]
    pub fn Content(&self) {
        self.0.content()
    }

    /// The key of its content in `NavDisplay`'s animation: its type and its key (Compose's internal
    /// `AnimatedSceneKey`).
    pub(crate) fn animated_key(&self) -> AnimatedSceneKey {
        AnimatedSceneKey { class: self.0.class(), key: self.0.key() }
    }

    /// Whether `self` holds this same scene object.
    pub(crate) fn ptr_eq(&self, other: &AnyScene) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for AnyScene {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.0.same(&*other.0)
    }
}

impl fmt::Debug for AnyScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

/// A scene's type and key (Compose's internal data class `AnimatedSceneKey(clazz, key)`): what tells `NavDisplay`'s
/// contents apart.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct AnimatedSceneKey {
    class: TypeId,
    key: SlotKey,
}

impl Hash for AnimatedSceneKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.class.hash(state);
        self.key.hash(state);
    }
}
