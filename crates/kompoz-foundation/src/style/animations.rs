//! The animations of a style's properties (Compose's StyleAnimations.kt): one per animated property, a fraction
//! from 0 (the value it animates from) to 1 (its target) run by an `Animatable` in the node's coroutine scope,
//! started when a property marked animated changes, interrupted (restarted from where it is) when it changes again,
//! dropped once it has finished and is no longer animated.

use super::properties::{FirstObjectProperty, PrimitivePropertyCount, object_phase_flags_of, primitive_phase_flags_of};
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector1D, spring};
use kompoz_coroutines::{CoroutineScope, Job};
use kompoz_runtime::{MutableState, mutable_state_of, named_args};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// An animation spec of the fraction a style's property animates by.
pub(crate) type FloatSpec = Arc<dyn AnimationSpec<f32>>;

thread_local! {
    /// Compose's DefaultSpringSpec (`spring<Float>()`), shared so it is known by its identity as well.
    static DEFAULT_SPRING_SPEC: FloatSpec = Arc::new(spring::<f32>());
}

/// Compose's DefaultSpringSpec: what a property animates by when nothing says otherwise.
pub(crate) fn default_spring_spec() -> FloatSpec {
    DEFAULT_SPRING_SPEC.with(Clone::clone)
}

/// Whether two specs are equal (Compose's `==` of specs).
pub(crate) fn same_spec(a: &FloatSpec, b: &FloatSpec) -> bool {
    Arc::ptr_eq(a, b) || **a == **b
}

/// Where an animation is in a resolution (Compose's EntryState).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EntryState {
    Untouched,
    Unchanged,
    Changed,
    Inserted,
    Interrupted,
    Removing,
}

/// One property's animation (Compose's Entry).
struct Entry {
    spec: RefCell<FloatSpec>,
    animation: RefCell<Animatable<f32, AnimationVector1D>>,
    /// Which animatable it is: read with it, written when it is replaced.
    generation: MutableState<u32>,
    state: Cell<EntryState>,
    job: RefCell<Option<Job>>,
}

/// The animations of a resolved style's properties, by property id (Compose's StyleAnimations).
#[derive(Default)]
pub(crate) struct StyleAnimations {
    inner: Rc<AnimationsInner>,
}

#[derive(Default)]
struct AnimationsInner {
    entries: RefCell<BTreeMap<u32, Rc<Entry>>>,
}

impl StyleAnimations {
    /// The phases of the properties animated (Compose's `phaseFlags`).
    pub(crate) fn phase_flags(&self) -> u32 {
        let mut primitives = 0u64;
        let mut objects = 0u32;
        for id in self.inner.entries.borrow().keys() {
            if *id < PrimitivePropertyCount {
                primitives |= 1 << id;
            } else {
                objects |= 1 << (id - FirstObjectProperty);
            }
        }
        object_phase_flags_of(objects) | primitive_phase_flags_of(primitives)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.entries.borrow().is_empty()
    }

    /// The fraction property `id` is at: 0 where it has no animation or one interrupted (Compose's `timeOf`).
    pub(crate) fn time_of(&self, id: u32) -> f32 {
        let entry = self.inner.entries.borrow().get(&id).cloned();
        let Some(e) = entry else { return 0.0 };
        // (read first: an interrupted one's coroutine starting tells the reader)
        e.generation.get();
        if e.state.get() == EntryState::Interrupted {
            return 0.0;
        }
        let animation = e.animation.borrow().clone();
        animation.value()
    }

    /// The properties whose animations run, or are about to start again (Compose's `inFlight`): bits of a `u64` by
    /// id, primitives then objects.
    pub(crate) fn in_flight(&self) -> u64 {
        let entries: Vec<(u32, Rc<Entry>)> =
            self.inner.entries.borrow().iter().map(|(id, e)| (*id, e.clone())).collect();
        let mut in_flight = 0u64;
        for (id, entry) in entries {
            entry.generation.get();
            let running = entry.state.get() == EntryState::Interrupted || {
                let animation = entry.animation.borrow().clone();
                animation.is_running()
            };
            if running {
                in_flight |= 1u64 << id;
            }
        }
        in_flight
    }

    /// Records a resolution's animated properties (`animating`) and which of them changed (`changes`), each with
    /// its spec (the property's `to` spec, else the one it animated back with, else its `from` spec, else the
    /// default spring): a new changed property starts an animation, a changed one restarts (is interrupted), one no
    /// longer animated is let finish and removed (Compose's `recordAnimations`). Which properties started (or
    /// restarted) animations.
    pub(crate) fn record_animations(
        &self,
        animating: u64,
        changes: u64,
        to_specs: Option<&BTreeMap<u32, FloatSpec>>,
        from_specs: Option<&BTreeMap<u32, FloatSpec>>,
        previous_from_specs: Option<&BTreeMap<u32, FloatSpec>>,
        scope: Option<&CoroutineScope>,
    ) -> u64 {
        let mut started = 0u64;
        self.pre_record();
        let mut ids = animating;
        while ids != 0 {
            let id = ids.trailing_zeros();
            ids ^= 1u64 << id;
            let spec = to_specs
                .and_then(|s| s.get(&id))
                .or_else(|| previous_from_specs.and_then(|s| s.get(&id)))
                .or_else(|| from_specs.and_then(|s| s.get(&id)))
                .cloned()
                .unwrap_or_else(default_spring_spec);
            let changed = changes & (1u64 << id) != 0;
            if self.record(id, changed, spec) {
                started |= 1u64 << id;
            }
        }
        self.post_record(scope);
        started
    }

    fn record(&self, id: u32, changed: bool, spec: FloatSpec) -> bool {
        let entry = self.inner.entries.borrow().get(&id).cloned();
        match entry {
            Some(entry) => {
                let same = same_spec(&entry.spec.borrow(), &spec);
                if changed || !same {
                    *entry.spec.borrow_mut() = spec;
                    entry.state.set(EntryState::Changed);
                    true
                } else {
                    entry.state.set(EntryState::Unchanged);
                    false
                }
            }
            None => {
                if changed {
                    let entry = Rc::new(Entry {
                        spec: RefCell::new(spec),
                        animation: RefCell::new(Animatable(0.0)),
                        generation: mutable_state_of(0),
                        state: Cell::new(EntryState::Inserted),
                        job: RefCell::new(None),
                    });
                    self.inner.entries.borrow_mut().insert(id, entry);
                }
                true
            }
        }
    }

    fn pre_record(&self) {
        for entry in self.inner.entries.borrow().values() {
            if matches!(entry.state.get(), EntryState::Inserted | EntryState::Unchanged | EntryState::Changed) {
                entry.state.set(EntryState::Untouched);
            }
        }
    }

    fn post_record(&self, scope: Option<&CoroutineScope>) {
        let entries: Vec<Rc<Entry>> = self.inner.entries.borrow().values().cloned().collect();
        for entry in entries {
            match entry.state.get() {
                EntryState::Inserted => self.animate(&entry, scope),
                EntryState::Untouched => entry.state.set(EntryState::Removing),
                EntryState::Changed => {
                    entry.state.set(EntryState::Interrupted);
                    self.animate(&entry, scope);
                }
                _ => {}
            }
        }
    }

    /// Starts (or restarts, Compose's `interrupted`) an entry's animation: from 0 to 1, by its spec, at the
    /// velocity it had (Compose's Entry.animate), in the node's scope.
    fn animate(&self, entry: &Rc<Entry>, scope: Option<&CoroutineScope>) {
        let old = entry.job.borrow_mut().take();
        if let Some(job) = old {
            job.cancel();
        }
        let Some(scope) = scope else { return };
        let (weak_entry, weak_inner) = (Rc::downgrade(entry), Rc::downgrade(&self.inner));
        let job = scope.launch(async move {
            let Some(entry) = weak_entry.upgrade() else { return };
            if entry.state.get() == EntryState::Interrupted {
                entry.state.set(EntryState::Unchanged);
            }
            // (Compose's `finally`: on a normal end the finished ones go; on a cancellation, which Kotlin runs
            // later, those not about to start)
            let mut guard = CleanupGuard { inner: weak_inner.clone(), cancelled: true };
            let velocity = entry.animation.borrow().velocity();
            let animation: Animatable<f32, AnimationVector1D> = Animatable(0.0);
            *entry.animation.borrow_mut() = animation.clone();
            // (what read the animatable it replaces — a measure, a draw run before this coroutine started, as the
            // host runs its coroutines after the frame's layout — is told it changed)
            entry.generation.set(entry.generation.peek() + 1);
            let spec = entry.spec.borrow().clone();
            drop(entry);
            named_args!(animation.animate_to(1.0, animation_spec = spec, initial_velocity = velocity)).await;
            guard.cancelled = false;
        });
        *entry.job.borrow_mut() = Some(job);
    }

    /// Cancels and drops them all (Compose's `close`).
    #[cfg(test)]
    pub(crate) fn close(&self) {
        let entries: Vec<Rc<Entry>> = std::mem::take(&mut *self.inner.entries.borrow_mut()).into_values().collect();
        for entry in entries {
            let job = entry.job.borrow_mut().take();
            if let Some(job) = job {
                job.cancel();
            }
        }
    }
}

impl AnimationsInner {
    /// Compose's `cleanupAnimations`: the animations neither interrupted nor running go (on a cancellation, not
    /// those inserted and about to start, which Kotlin's later `finally` finds running).
    fn cleanup(&self, cancelled: bool) {
        let Ok(mut entries) = self.entries.try_borrow_mut() else { return };
        entries.retain(|_, entry| {
            let state = entry.state.get();
            if state == EntryState::Interrupted || (cancelled && state == EntryState::Inserted) {
                return true;
            }
            entry.animation.try_borrow().map(|a| a.is_running()).unwrap_or(true)
        });
    }
}

/// Runs the cleanup when an entry's coroutine ends, however it ends.
struct CleanupGuard {
    inner: Weak<AnimationsInner>,
    cancelled: bool,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.cleanup(self.cancelled);
        }
    }
}

/// Compose's `isNullOrEmpty` of the animations.
pub(crate) fn is_null_or_empty(animations: Option<&StyleAnimations>) -> bool {
    animations.is_none_or(StyleAnimations::is_empty)
}

/// The primitives' part of an animation set (Compose's `toPrimitivesSet`: its mask reaching one bit past them, the
/// first object's, which no primitive id reads).
pub(crate) fn to_primitives_set(set: u64) -> u64 {
    set & ((1u64 << (PrimitivePropertyCount + 1)) - 1)
}

/// The objects' part of an animation set (Compose's `toObjectsSet`).
pub(crate) fn to_objects_set(set: u64) -> u32 {
    (set >> PrimitivePropertyCount) as u32
}

/// The primitives and objects as one animation set (Compose's `propertySetsToAnimationSet`).
pub(crate) fn property_sets_to_animation_set(primitives_set: u64, objects_set: u32) -> u64 {
    primitives_set | ((objects_set as u64) << FirstObjectProperty)
}
