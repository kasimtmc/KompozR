//! `LazyLayout`, as Compose's LazyLayout.kt, LazyLayoutMeasureScope.kt, LazyLayoutMeasurePolicy.kt,
//! LazyLayoutItemContentFactory.kt and LazySaveableStateHolder.kt: a subcomposing layout whose measure composes
//! only the items it asks for (each in a slot of its own, keyed by the item's key), their content kept by key so an
//! item isn't composed again for nothing, their `remember_saveable` state kept by key while they are away, the
//! compositions of items that left kept for reuse by items of the same content type (up to 7 each, as
//! RecyclerView), and items prefetched through its [`LazyLayoutPrefetchState`].

use super::item_provider::LazyLayoutItemProvider;
use super::key::LazyKey;
use super::prefetch::{
    LazyLayoutPrefetchState, PrefetchHandleProvider, remember_default_prefetch_scheduler, traversable_prefetch_state,
};
use kompoz_runtime::{
    DisposableEffect, LocalSaveableStateRegistry, ProvideSaveableStateRegistry, SaveableStateHolder,
    SaveableStateRegistry, SavedValue, Saver, kompozable, kompozable_block, on_dispose, remember,
    remember_saveable_keyed_with, remember_updated_state,
};
use kompoz_ui::layout::{
    SlotIdsSet, SlotKey, SubcomposeLayout, SubcomposeLayoutState, SubcomposeMeasureScope, SubcomposeSlotReusePolicy,
};
use kompoz_ui::{Constraints, Measurable, MeasureResult, MeasureScope, Modifier, Placeable};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// What makes a lazy layout's items, as the layout reads it each time it needs it (Compose's `() ->
/// LazyLayoutItemProvider`).
pub type ItemProviderLambda = Rc<dyn Fn() -> Rc<dyn LazyLayoutItemProvider>>;

/// How a [`LazyLayout`] measures (Compose's LazyLayoutMeasurePolicy): it composes the items it needs through the
/// scope ([`LazyLayoutMeasureScope::compose`]), measures and places them. A closure `|scope, constraints| …` is one.
pub trait LazyLayoutMeasurePolicy {
    fn measure(&self, scope: &LazyLayoutMeasureScope, constraints: Constraints) -> MeasureResult;
}

impl<F: Fn(&LazyLayoutMeasureScope, Constraints) -> MeasureResult> LazyLayoutMeasurePolicy for F {
    fn measure(&self, scope: &LazyLayoutMeasureScope, constraints: Constraints) -> MeasureResult {
        self(scope, constraints)
    }
}

/// A layout that composes and lays out only the items it needs now (Compose's `LazyLayout`): what they are is
/// `item_provider`'s (read again whenever needed), which ones `measure_policy` asks for as it measures — in view,
/// and what it keeps besides (pinned items, beyond-bounds items) — the rest left out of the composition; with a
/// `prefetch_state`, items are composed ahead in the host's pauses between frames. For a ready list see
/// `LazyColumn` and `LazyRow`.
#[kompozable(plain(measure))]
pub fn LazyLayout(
    item_provider: ItemProviderLambda,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] prefetch_state: Option<LazyLayoutPrefetchState>,
    measure_policy: Rc<dyn LazyLayoutMeasurePolicy>,
) {
    let current_item_provider = remember_updated_state!(item_provider);
    LazySaveableStateHolderProvider(move |saveable_state_holder: Rc<LazySaveableStateHolder>| {
        let current_item_provider = current_item_provider.clone();
        let item_content_factory = remember(move || {
            let current = current_item_provider.clone();
            let provider: ItemProviderLambda = Rc::new(move || (current.get())());
            Rc::new(LazyLayoutItemContentFactory::new(saveable_state_holder, provider))
        });
        let factory = item_content_factory.clone();
        let subcompose_layout_state =
            remember(move || SubcomposeLayoutState::new(Rc::new(LazyLayoutItemReusePolicy::new(factory))));
        if let Some(prefetch_state) = prefetch_state.clone() {
            let executor = remember_default_prefetch_scheduler();
            let keys = (
                prefetch_state.clone(),
                Rc::as_ptr(&item_content_factory) as usize,
                subcompose_layout_state.clone(),
                Rc::as_ptr(&executor) as *const () as usize,
            );
            let (factory, subcompose) = (item_content_factory.clone(), subcompose_layout_state.clone());
            DisposableEffect(keys, move || {
                prefetch_state
                    .set_prefetch_handle_provider(Some(PrefetchHandleProvider::new(factory, subcompose, executor)));
                on_dispose(move || {
                    // (its handle provider let go)
                    if let Some(provider) = prefetch_state.inner().prefetch_handle_provider.borrow().clone() {
                        provider.on_disposed();
                    }
                    prefetch_state.set_prefetch_handle_provider(None);
                })
            });
        }
        let factory = item_content_factory.clone();
        let measure_policy = measure_policy.clone();
        SubcomposeLayout(
            state = Some(subcompose_layout_state),
            modifier = traversable_prefetch_state(modifier.clone(), prefetch_state.clone()),
            measure_policy = move |scope: &SubcomposeMeasureScope, constraints: Constraints| {
                let scope = LazyLayoutMeasureScope::new(factory.clone(), scope.clone());
                measure_policy.measure(&scope, constraints)
            },
        );
    });
}

/// Keeps up to this many compositions of items that left for each content type (RecyclerView's 5 scrap + 2 cache).
const MaxItemsToRetainForReuse: usize = 7;

/// A lazy layout's reuse policy (Compose's LazyLayoutItemReusePolicy): up to 7 slots kept for each content type, a
/// slot reused for an item of the same content type.
struct LazyLayoutItemReusePolicy {
    factory: Rc<LazyLayoutItemContentFactory>,
}

impl LazyLayoutItemReusePolicy {
    fn new(factory: Rc<LazyLayoutItemContentFactory>) -> Self {
        LazyLayoutItemReusePolicy { factory }
    }
}

impl SubcomposeSlotReusePolicy for LazyLayoutItemReusePolicy {
    fn get_slots_to_retain(&self, slot_ids: &mut SlotIdsSet) {
        let count_per_type: RefCell<HashMap<Option<SlotKey>, usize>> = RefCell::new(HashMap::new());
        slot_ids.retain_if(|slot_id| {
            let content_type = self.factory.get_content_type(slot_id);
            let mut counts = count_per_type.borrow_mut();
            let count = counts.entry(content_type).or_insert(0);
            if *count == MaxItemsToRetainForReuse {
                false
            } else {
                *count += 1;
                true
            }
        });
    }

    fn are_compatible(&self, slot_id: &SlotKey, reusable_slot_id: &SlotKey) -> bool {
        self.factory.get_content_type(slot_id) == self.factory.get_content_type(reusable_slot_id)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure scope

/// What a [`LazyLayout`]'s measure policy measures in (Compose's LazyLayoutMeasureScope): a measure scope (it
/// derefs to one: `scope.layout(…)`) that composes items by index ([`compose`](Self::compose)); an item asked
/// for twice in a measure is composed once.
pub struct LazyLayoutMeasureScope {
    item_content_factory: Rc<LazyLayoutItemContentFactory>,
    subcompose_measure_scope: SubcomposeMeasureScope,
    item_provider: Rc<dyn LazyLayoutItemProvider>,
    placeables_cache: RefCell<HashMap<i32, Vec<Placeable>>>,
    measurables_cache: RefCell<HashMap<i32, Vec<Rc<dyn Measurable>>>>,
}

impl std::ops::Deref for LazyLayoutMeasureScope {
    type Target = MeasureScope;
    fn deref(&self) -> &MeasureScope {
        &self.subcompose_measure_scope
    }
}

impl LazyLayoutMeasureScope {
    pub(crate) fn new(
        item_content_factory: Rc<LazyLayoutItemContentFactory>,
        subcompose_measure_scope: SubcomposeMeasureScope,
    ) -> LazyLayoutMeasureScope {
        let item_provider = item_content_factory.item_provider();
        LazyLayoutMeasureScope {
            item_content_factory,
            subcompose_measure_scope,
            item_provider,
            placeables_cache: RefCell::new(HashMap::new()),
            measurables_cache: RefCell::new(HashMap::new()),
        }
    }

    /// The item provider of this measure.
    pub fn item_provider(&self) -> &Rc<dyn LazyLayoutItemProvider> {
        &self.item_provider
    }

    /// Composes item `index` (Compose's `compose`): the layouts its content emits, to measure.
    pub fn compose(&self, index: i32) -> Vec<Rc<dyn Measurable>> {
        if let Some(cached) = self.measurables_cache.borrow().get(&index) {
            return cached.clone();
        }
        let key = self.item_provider.get_key(index);
        let content_type = self.item_provider.get_content_type(index);
        let item_content = self.item_content_factory.get_content(index, &key, &content_type);
        let measurables = self.subcompose_measure_scope.subcompose_shared(key, item_content);
        self.measurables_cache.borrow_mut().insert(index, measurables.clone());
        measurables
    }

    /// Composes item `index` and measures its layouts in `constraints` (Compose's deprecated `measure`; the
    /// same as `compose(index)` each measured).
    pub fn measure(&self, index: i32, constraints: Constraints) -> Vec<Placeable> {
        if let Some(cached) = self.placeables_cache.borrow().get(&index) {
            return cached.clone();
        }
        let placeables: Vec<Placeable> = self.compose(index).iter().map(|m| m.measure(constraints)).collect();
        self.placeables_cache.borrow_mut().insert(index, placeables.clone());
        placeables
    }

    /// The measure scope it is.
    pub fn measure_scope(&self) -> &MeasureScope {
        &self.subcompose_measure_scope
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The item content factory

/// The content of the items, kept by key (Compose's LazyLayoutItemContentFactory): an item subcomposed again with
/// the same content isn't composed again for itself; the content finds the item's index by its key when the items
/// change, and composes it inside its saveable state.
pub(crate) struct LazyLayoutItemContentFactory {
    saveable_state_holder: Rc<LazySaveableStateHolder>,
    item_provider: ItemProviderLambda,
    lambdas_cache: RefCell<HashMap<LazyKey, Rc<CachedItemContent>>>,
}

/// An item's content as it was made (Compose's CachedItemContent): for its key and content type, at the index it
/// was found at last.
struct CachedItemContent {
    index: Cell<i32>,
    key: LazyKey,
    content_type: Option<SlotKey>,
    content: RefCell<Option<Rc<dyn Fn()>>>,
}

impl LazyLayoutItemContentFactory {
    pub(crate) fn new(saveable_state_holder: Rc<LazySaveableStateHolder>, item_provider: ItemProviderLambda) -> Self {
        LazyLayoutItemContentFactory {
            saveable_state_holder,
            item_provider,
            lambdas_cache: RefCell::new(HashMap::new()),
        }
    }

    /// The items now.
    pub(crate) fn item_provider(&self) -> Rc<dyn LazyLayoutItemProvider> {
        (self.item_provider)()
    }

    /// The content type of the item whose slot is `slot_id` (Compose's `getContentType`): as its content was made,
    /// else as the items say now.
    pub(crate) fn get_content_type(&self, slot_id: &SlotKey) -> Option<SlotKey> {
        let key = slot_id.downcast_ref::<LazyKey>()?;
        if let Some(cached) = self.lambdas_cache.borrow().get(key) {
            return cached.content_type.clone();
        }
        let item_provider = self.item_provider();
        let index = item_provider.get_index(key);
        if index != -1 { item_provider.get_content_type(index) } else { None }
    }

    /// The content of the item of `key` at `index`: the one made before if it was made for the same index and
    /// content type, else a new one (Compose's `getContent`).
    pub(crate) fn get_content(
        self: &Rc<Self>,
        index: i32,
        key: &LazyKey,
        content_type: &Option<SlotKey>,
    ) -> Rc<dyn Fn()> {
        let cached = self.lambdas_cache.borrow().get(key).cloned();
        if let Some(cached) = cached
            && cached.index.get() == index
            && cached.content_type == *content_type
        {
            return self.content_of(&cached);
        }
        let new_content = Rc::new(CachedItemContent {
            index: Cell::new(index),
            key: key.clone(),
            content_type: content_type.clone(),
            content: RefCell::new(None),
        });
        self.lambdas_cache.borrow_mut().insert(key.clone(), new_content.clone());
        self.content_of(&new_content)
    }

    /// The cached content's lambda, made the first time it is needed (and again after it was disposed of).
    fn content_of(self: &Rc<Self>, cached: &Rc<CachedItemContent>) -> Rc<dyn Fn()> {
        if let Some(content) = cached.content.borrow().clone() {
            return content;
        }
        let factory = Rc::downgrade(self);
        let weak = Rc::downgrade(cached);
        let content: Rc<dyn Fn()> = Rc::new(move || {
            let (Some(factory), Some(cached)) = (factory.upgrade(), weak.upgrade()) else { return };
            kompozable_block!({
                ItemContent(factory, cached);
            });
        });
        *cached.content.borrow_mut() = Some(content.clone());
        content
    }
}

/// An item's content: the item found by its key (its index moved as items before it changed), composed in its
/// saveable state; the cached lambda forgotten once its composition goes (so it doesn't keep its scopes).
#[kompozable]
fn ItemContent(factory: Rc<LazyLayoutItemContentFactory>, cached: Rc<CachedItemContent>) {
    let item_provider = factory.item_provider();
    let key = cached.key.clone();
    let mut index = cached.index.get();
    if index >= item_provider.item_count() || item_provider.get_key(index) != key {
        index = item_provider.get_index(&key);
        if index != -1 {
            cached.index.set(index);
        }
    }
    if index != -1 {
        SkippableItem(
            ItemProviderArg(item_provider),
            HolderArg(factory.saveable_state_holder.clone()),
            index,
            key.clone(),
        );
    }
    let cached = Rc::downgrade(&cached);
    DisposableEffect(key, move || {
        on_dispose(move || {
            if let Some(cached) = cached.upgrade() {
                *cached.content.borrow_mut() = None;
            }
        })
    });
}

/// An item provider as a skippable composable's argument: equal when it gives the same items.
#[derive(Clone)]
struct ItemProviderArg(Rc<dyn LazyLayoutItemProvider>);

impl PartialEq for ItemProviderArg {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.0.same_items(&*other.0)
    }
}

/// The saveable state holder as a skippable composable's argument (Compose's StableValue): equal when it is.
#[derive(Clone)]
struct HolderArg(Rc<LazySaveableStateHolder>);

impl PartialEq for HolderArg {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The item in its saveable state (Compose's SkippableItem): skipped while the items, the holder, the index and
/// the key stay the same.
#[kompozable]
fn SkippableItem(item_provider: ItemProviderArg, saveable_state_holder: HolderArg, index: i32, key: LazyKey) {
    let provider = item_provider.0.clone();
    let item_key = key.clone();
    saveable_state_holder.0.SaveableStateProvider(&key, move || provider.item(index, &item_key));
}

// ---------------------------------------------------------------------------------------------------------------
// The saveable state holder

/// A lazy layout's saveable state (Compose's LazySaveableStateHolder): a holder keeping each item's
/// `remember_saveable` values by its key while it is away, and a registry for what the layout itself saves. Saved
/// with the state, it leaves out the items that aren't composed (Compose removes their state as it saves).
pub(crate) struct LazySaveableStateHolder {
    registry: SaveableStateRegistry,
    wrapped_holder: SaveableStateHolder,
    /// The keys of the items composed before and not now.
    previously_composed_keys: RefCell<HashSet<String>>,
    /// Whether the layout is leaving the composition (its items with it): those items are kept as they were, as
    /// Compose's holder saved them as they left (its wrapped holder saving each item's state on its way out, before
    /// the layout's own save leaves the previously composed ones out).
    leaving: Cell<bool>,
}

impl LazySaveableStateHolder {
    fn new(parent_registry: Option<SaveableStateRegistry>, restored: Option<&SavedValue>) -> Self {
        let registry_saved = restored.map(|r| r.get("registry").clone()).filter(|v| !v.is_null());
        let registry = SaveableStateRegistry::with_can_be_saved(registry_saved.as_ref(), move |v| {
            parent_registry.as_ref().is_none_or(|p| p.can_be_saved(v))
        });
        let holder = restored
            .map(|r| r.get("holder").clone())
            .filter(|v| !v.is_null())
            .and_then(|v| <SaveableStateHolder as kompoz_runtime::Saveable>::restore(&v))
            .unwrap_or_default();
        LazySaveableStateHolder {
            registry,
            wrapped_holder: holder,
            previously_composed_keys: RefCell::new(HashSet::new()),
            leaving: Cell::new(false),
        }
    }

    /// What it writes out (Compose's `performSave`): the items' values but those of items not composed now, and
    /// its registry's.
    fn perform_save(&self) -> SavedValue {
        let holder = <SaveableStateHolder as kompoz_runtime::Saveable>::save(&self.wrapped_holder);
        let left_out = self.previously_composed_keys.borrow();
        let holder = match holder.as_map() {
            Some(map) => {
                SavedValue::map(map.iter().filter(|(k, _)| !left_out.contains(*k)).map(|(k, v)| (k.clone(), v.clone())))
            }
            None => holder,
        };
        SavedValue::map([("registry", self.registry.perform_save()), ("holder", holder)])
    }

    /// Composes `content` in the saveable state of the item of `key` (Compose's `SaveableStateProvider`).
    #[allow(non_snake_case)]
    #[track_caller]
    pub(crate) fn SaveableStateProvider(self: &Rc<Self>, key: &LazyKey, content: impl FnOnce()) {
        let name = key.holder_key();
        self.wrapped_holder.ReusableSaveableStateProvider(&name, content);
        let me = self.clone();
        kompozable_block!({
            DisposableEffect(name.clone(), move || {
                me.previously_composed_keys.borrow_mut().remove(&name);
                on_dispose(move || {
                    if !me.leaving.get() {
                        me.previously_composed_keys.borrow_mut().insert(name);
                    }
                })
            });
        });
    }

    /// Forgets the state of the item of `key` (Compose's `removeState`).
    #[allow(dead_code)]
    pub(crate) fn remove_state(&self, key: &LazyKey) {
        self.wrapped_holder.remove_state(&key.holder_key());
    }

    fn saver(parent_registry: Option<SaveableStateRegistry>) -> Saver<Rc<LazySaveableStateHolder>> {
        Saver::new(
            |h: &Rc<LazySaveableStateHolder>| h.perform_save(),
            move |saved| Some(Rc::new(LazySaveableStateHolder::new(parent_registry.clone(), Some(saved)))),
        )
    }
}

/// Composes `content` with a lazy layout's saveable state holder, saved with the state and provided as the
/// registry (Compose's `LazySaveableStateHolderProvider`).
#[kompozable(inline)]
fn LazySaveableStateHolderProvider(content: impl FnOnce(Rc<LazySaveableStateHolder>)) {
    let current_registry = LocalSaveableStateRegistry.current();
    let parent = current_registry.clone();
    let holder = remember_saveable_keyed_with(
        current_registry.clone(),
        &LazySaveableStateHolder::saver(current_registry),
        move || Rc::new(LazySaveableStateHolder::new(parent, None)),
    );
    let registry = holder.registry.clone();
    let h = holder.clone();
    ProvideSaveableStateRegistry(&registry, move || content(h));
    // (after the content: told first as all of it leaves — what leaves is told in the reverse of its order —, so the
    // items leaving with the layout are kept as they were)
    DisposableEffect(HolderArg(holder.clone()), move || {
        holder.leaving.set(false);
        on_dispose(move || holder.leaving.set(true))
    });
}
