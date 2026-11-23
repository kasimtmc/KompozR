//! Scene.kt, OverlayScene.kt, SceneStrategy.kt, SinglePaneScene.kt, SceneDecoratorStrategy.kt.

use crate::runtime::{Metadata, NavEntry, NavEntryKey};
use kompoz_navigationevent::DynEq;
use kompoz_ui::layout::SlotKey;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;

/// What renders one or more entries (Compose's interface `Scene`), calculated by a [`SceneStrategy`] from the back
/// stack's entries; a change of scene (its type and [`key`](Self::key)) is what `NavDisplay` animates.
///
/// Its [`content`](Self::content) composes each of its [`entries`](Self::entries) at most once, with any UI of its
/// own around them. Two scenes may show one entry: while both are shown (an animation, predictive back), the entry
/// is composed in the one that is the latest target. [`previous_entries`](Self::previous_entries) are the entries
/// after a back: their scene is what predictive back shows. Scenes compare structurally (Kotlin: a data class, or
/// `equals`): `#[derive(PartialEq)]`. An [`OverlayScene`] says it is one ([`as_overlay_scene`](Self::as_overlay_scene)).
pub trait Scene<T>: DynEq + fmt::Debug + 'static {
    /// What identifies it among the scenes of its type (with its type: the key of `NavDisplay`'s animation).
    fn key(&self) -> SlotKey;

    /// The entries it may show (each composed by the latest target scene showing it).
    fn entries(&self) -> Vec<NavEntry<T>>;

    /// The entries after a back from it: their scene is what predictive back animates to.
    fn previous_entries(&self) -> Vec<NavEntry<T>>;

    /// Its content: its entries' content, with what it adds around them (Compose's `content` property). Composable.
    fn content(&self);

    /// What it tells `NavDisplay` (transitions…): by default its last entry's metadata.
    fn metadata(&self) -> Metadata {
        self.entries().last().map(NavEntry::metadata).unwrap_or_default()
    }

    /// It as an [`OverlayScene`] if it is one (Kotlin's `scene is OverlayScene`): an overlay scene returns
    /// `Some(self)`.
    fn as_overlay_scene(self: Rc<Self>) -> Option<Rc<dyn OverlayScene<T>>> {
        None
    }
}

impl<T: NavEntryKey> PartialEq for dyn Scene<T> {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_any())
    }
}

impl<T: NavEntryKey> dyn Scene<T> {
    /// It as an `S`, if it is one (Kotlin's `scene as? S`).
    pub fn downcast_ref<S: 'static>(&self) -> Option<&S> {
        self.as_any().downcast_ref::<S>()
    }

    /// Whether it is an `S` (Kotlin's `scene is S`).
    pub fn is<S: 'static>(&self) -> bool {
        self.as_any().is::<S>()
    }
}

/// A scene shown above the others, in a window of its own (a dialog, a popup, a sheet: Compose's interface
/// `OverlayScene`): the scene of its [`overlaid_entries`](Self::overlaid_entries) (never empty) is shown below it.
/// Calculating that scene starts again from the first strategy (several overlay scenes may be shown: their keys
/// tell them apart). Its [`as_overlay_scene`](Scene::as_overlay_scene) returns itself.
pub trait OverlayScene<T>: Scene<T> {
    /// The entries a scene below it shows (as a rule its previous entries).
    fn overlaid_entries(&self) -> Vec<NavEntry<T>>;

    /// What it does once it has been popped, before it leaves the composition (Compose's suspending `onRemove`):
    /// its exit animation. By default nothing.
    fn on_remove(&self) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async {})
    }
}

impl<T: NavEntryKey> PartialEq for dyn OverlayScene<T> {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_any())
    }
}

/// What a [`SceneStrategy`] calculates a scene in (Compose's open class `SceneStrategyScope`): its
/// [`on_back`](Self::on_back) is what a scene handling back itself calls (a window of its own, a gesture of its UI),
/// the display's back.
pub struct SceneStrategyScope<T> {
    on_back: Rc<dyn Fn()>,
    _t: PhantomData<fn(T)>,
}

impl<T> Clone for SceneStrategyScope<T> {
    fn clone(&self) -> Self {
        SceneStrategyScope { on_back: self.on_back.clone(), _t: PhantomData }
    }
}

impl<T> SceneStrategyScope<T> {
    /// A scope of `on_back` (Compose's internal constructor).
    pub fn with_on_back(on_back: impl Fn() + 'static) -> SceneStrategyScope<T> {
        SceneStrategyScope { on_back: Rc::new(on_back), _t: PhantomData }
    }

    /// The display's back (Compose's `onBack`).
    pub fn on_back(&self) -> Rc<dyn Fn()> {
        self.on_back.clone()
    }
}

/// A scope whose back does nothing, for calculating scenes alone (Compose's public `SceneStrategyScope()`; a test
/// of `on_back`: [`remember_scene_state`](super::remember_scene_state)'s).
///
/// Kotlin: `SceneStrategyScope()`
pub fn SceneStrategyScope<T>() -> SceneStrategyScope<T> {
    SceneStrategyScope::with_on_back(|| {})
}

/// What calculates a scene from the back stack's entries (Compose's fun interface `SceneStrategy`): a scene showing
/// some of the last entries, or none (then the next strategy is asked; at the end, a single pane). A closure
/// `|scope, entries| …` is one (Kotlin's `SceneStrategy { entries -> … }`, `this` the scope).
pub trait SceneStrategy<T>: 'static {
    /// The scene of `entries` this strategy shows, if it shows one (Compose's `SceneStrategyScope<T>.calculateScene`).
    fn calculate_scene(&self, scope: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>>;
}

impl<T: NavEntryKey, F: Fn(&SceneStrategyScope<T>, Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> + 'static>
    SceneStrategy<T> for F
{
    fn calculate_scene(&self, scope: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> {
        self(scope, entries)
    }
}

/// Kotlin's SAM constructor `SceneStrategy { entries -> … }`: the strategy of this closure.
///
/// Kotlin: `SceneStrategy(calculateScene: SceneStrategyScope<T>.(entries: List<NavEntry<T>>) -> Scene<T>?)`
pub fn SceneStrategy<T: NavEntryKey>(
    calculate_scene: impl Fn(&SceneStrategyScope<T>, Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> + 'static,
) -> Rc<dyn SceneStrategy<T>> {
    Rc::new(calculate_scene)
}

/// The strategy asking `first` and then `second` (Compose's deprecated infix `then`: the first scene either
/// calculates).
#[deprecated(
    note = "Deprecated in favor of List<SceneStrategy> APIs that take the output of this operator has been refactored to take a list of strategies instead."
)]
pub fn then<T: NavEntryKey>(
    first: Rc<dyn SceneStrategy<T>>,
    second: Rc<dyn SceneStrategy<T>>,
) -> Rc<dyn SceneStrategy<T>> {
    Rc::new(move |scope: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>| {
        first.calculate_scene(scope, entries.clone()).or_else(|| second.calculate_scene(scope, entries))
    })
}

/// The scene of one entry (Compose's internal data class `SinglePaneScene`).
pub(crate) struct SinglePaneScene<T> {
    pub(crate) key: SlotKey,
    pub(crate) entry: NavEntry<T>,
    pub(crate) previous_entries: Vec<NavEntry<T>>,
}

impl<T: PartialEq> PartialEq for SinglePaneScene<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.entry == other.entry && self.previous_entries == other.previous_entries
    }
}

impl<T: fmt::Debug> fmt::Debug for SinglePaneScene<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SinglePaneScene(key={:?}, entry={:?}, previousEntries={:?}, entries=[{:?}])",
            self.key, self.entry, self.previous_entries, self.entry
        )
    }
}

impl<T: NavEntryKey> Scene<T> for SinglePaneScene<T> {
    fn key(&self) -> SlotKey {
        self.key.clone()
    }
    fn entries(&self) -> Vec<NavEntry<T>> {
        vec![self.entry.clone()]
    }
    fn previous_entries(&self) -> Vec<NavEntry<T>> {
        self.previous_entries.clone()
    }
    fn content(&self) {
        self.entry.Content();
    }
}

/// The strategy showing the last entry alone (Compose's `SinglePaneSceneStrategy`): it always calculates one; what
/// `NavDisplay` falls back to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SinglePaneSceneStrategy {
    _object: (),
}

/// Kotlin: `SinglePaneSceneStrategy()`
pub fn SinglePaneSceneStrategy() -> SinglePaneSceneStrategy {
    SinglePaneSceneStrategy { _object: () }
}

impl<T: NavEntryKey> SceneStrategy<T> for SinglePaneSceneStrategy {
    fn calculate_scene(&self, _: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> {
        let last = entries.last().expect("a scene of no entries").clone();
        Some(Rc::new(SinglePaneScene {
            key: last.content_key(),
            entry: last,
            previous_entries: entries[..entries.len() - 1].to_vec(),
        }))
    }
}

/// The first scene `scene_strategies` calculate, else the single pane's (Compose's internal
/// `calculateSceneWithSinglePaneFallback`).
pub(crate) fn calculate_scene_with_single_pane_fallback<T: NavEntryKey>(
    scene_strategies: &[Rc<dyn SceneStrategy<T>>],
    scope: &SceneStrategyScope<T>,
    entries: Vec<NavEntry<T>>,
) -> Rc<dyn Scene<T>> {
    for strategy in scene_strategies {
        if let Some(scene) = strategy.calculate_scene(scope, entries.clone()) {
            return scene;
        }
    }
    SinglePaneSceneStrategy().calculate_scene(scope, entries).expect("a single pane")
}

/// What a [`SceneDecoratorStrategy`] decorates a scene in (Compose's `SceneDecoratorStrategyScope`, a
/// [`SceneStrategyScope`]: it derefs to one).
pub struct SceneDecoratorStrategyScope<T> {
    scope: SceneStrategyScope<T>,
}

impl<T> Clone for SceneDecoratorStrategyScope<T> {
    fn clone(&self) -> Self {
        SceneDecoratorStrategyScope { scope: self.scope.clone() }
    }
}

impl<T> std::ops::Deref for SceneDecoratorStrategyScope<T> {
    type Target = SceneStrategyScope<T>;
    fn deref(&self) -> &SceneStrategyScope<T> {
        &self.scope
    }
}

impl<T> SceneDecoratorStrategyScope<T> {
    /// A scope of `on_back` (Compose's internal constructor).
    pub fn with_on_back(on_back: impl Fn() + 'static) -> SceneDecoratorStrategyScope<T> {
        SceneDecoratorStrategyScope { scope: SceneStrategyScope::with_on_back(on_back) }
    }
}

/// A scope whose back does nothing, for decorating scenes alone (Compose's public `SceneDecoratorStrategyScope()`).
///
/// Kotlin: `SceneDecoratorStrategyScope()`
pub fn SceneDecoratorStrategyScope<T>() -> SceneDecoratorStrategyScope<T> {
    SceneDecoratorStrategyScope::with_on_back(|| {})
}

/// What decorates a scene with another (Compose's fun interface `SceneDecoratorStrategy`): the scene it gives may
/// show the given one's content or not. Overlay scenes aren't decorated (they are animated apart). A closure `|scope,
/// scene| …` is one.
pub trait SceneDecoratorStrategy<T>: 'static {
    /// `scene` decorated (Compose's `SceneDecoratorStrategyScope<T>.decorateScene`).
    fn decorate_scene(&self, scope: &SceneDecoratorStrategyScope<T>, scene: Rc<dyn Scene<T>>) -> Rc<dyn Scene<T>>;
}

impl<T: NavEntryKey, F: Fn(&SceneDecoratorStrategyScope<T>, Rc<dyn Scene<T>>) -> Rc<dyn Scene<T>> + 'static>
    SceneDecoratorStrategy<T> for F
{
    fn decorate_scene(&self, scope: &SceneDecoratorStrategyScope<T>, scene: Rc<dyn Scene<T>>) -> Rc<dyn Scene<T>> {
        self(scope, scene)
    }
}

/// Kotlin's SAM constructor `SceneDecoratorStrategy { scene -> … }`.
///
/// Kotlin: `SceneDecoratorStrategy(decorateScene: SceneDecoratorStrategyScope<T>.(scene: Scene<T>) -> Scene<T>)`
pub fn SceneDecoratorStrategy<T: NavEntryKey>(
    decorate_scene: impl Fn(&SceneDecoratorStrategyScope<T>, Rc<dyn Scene<T>>) -> Rc<dyn Scene<T>> + 'static,
) -> Rc<dyn SceneDecoratorStrategy<T>> {
    Rc::new(decorate_scene)
}
