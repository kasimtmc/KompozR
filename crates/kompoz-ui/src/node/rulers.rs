//! The rulers a coordinator provides and who read them (Compose's LookaheadCapablePlaceable ruler handling and its
//! RulerTrackingMap): values captured as it is placed, worked out again when what the rulers block read changes or
//! (when it read its coordinates) when it moves; a reader placed again when a value it read changes, or when a
//! layout nearer to it starts providing that ruler.

use super::{LayoutNode, NodeCoordinator, ReaderScope};
use crate::layout::{IsRulerProvided, LayoutCoordinates, MeasureResult, Ruler, RulerProvider, RulerScope};
use crate::layout::{RulerScopeState, RulersBlock};
use kompoz_graphics::{IntOffset, LayoutDirection};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// How a value was set in the capture under way (Compose's access flags).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    /// Set to the value it had.
    NoChange,
    /// Set to another value.
    Changed,
    /// Not set (yet): dropped at the end of the capture.
    NotSet,
    /// Set, newly.
    New,
}

/// A coordinator's rulers: their values, who read which, and what provides them.
#[derive(Default)]
pub(crate) struct CoordinatorRulers {
    values: RefCell<Vec<(Ruler, f32, Access)>>,
    readers: RefCell<Vec<(Ruler, Vec<Weak<LayoutNode>>)>>,
    lambda: RefCell<Option<RulersBlock>>,
    provided: RefCell<Option<IsRulerProvided>>,
    provider: RefCell<Option<RulerProvider>>,
    scope: RulerScopeState,
    /// A provider's state for each ruler it provided.
    scopes: RefCell<Vec<(Ruler, Rc<RulerScopeState>)>>,
    /// What the rulers block read.
    reads: RefCell<Option<Rc<ReaderScope>>>,
    /// What the provider read for each ruler.
    single_reads: RefCell<Vec<(Ruler, Rc<ReaderScope>)>>,
    /// The measure result last captured from.
    result: RefCell<Option<MeasureResult>>,
    capturing: Cell<bool>,
}

impl CoordinatorRulers {
    fn value(&self, ruler: &Ruler) -> Option<f32> {
        self.values.borrow().iter().find(|(r, _, _)| r == ruler).map(|v| v.1)
    }

    fn contains(&self, ruler: &Ruler) -> bool {
        self.values.borrow().iter().any(|(r, _, _)| r == ruler)
    }

    fn set(&self, ruler: Ruler, value: f32) {
        let mut values = self.values.borrow_mut();
        match values.iter_mut().find(|(r, _, _)| *r == ruler) {
            None => values.push((ruler, value, Access::New)),
            Some(entry) => {
                if entry.1 != value {
                    entry.1 = value;
                    entry.2 = Access::Changed;
                } else if entry.2 == Access::NotSet {
                    entry.2 = Access::NoChange;
                }
            }
        }
    }

    fn remove(&self, ruler: &Ruler) {
        self.values.borrow_mut().retain(|(r, _, _)| r != ruler);
    }

    fn take_readers(&self, ruler: &Ruler) -> Vec<Weak<LayoutNode>> {
        let mut readers = self.readers.borrow_mut();
        match readers.iter().position(|(r, _)| r == ruler) {
            Some(i) => readers.remove(i).1,
            None => Vec::new(),
        }
    }
}

/// Each of `nodes` placed again (Compose's `notifyRulerValueChange`).
fn relayout(nodes: Vec<Weak<LayoutNode>>) {
    for node in nodes.into_iter().filter_map(|n| n.upgrade()) {
        node.request_relayout();
    }
}

impl NodeCoordinator {
    /// Its rulers' state, made when first needed.
    fn rulers(&self) -> Rc<CoordinatorRulers> {
        if let Some(r) = self.rulers.borrow().as_ref() {
            return r.clone();
        }
        let r = Rc::new(CoordinatorRulers::default());
        *self.rulers.borrow_mut() = Some(r.clone());
        r
    }

    fn rulers_if_any(&self) -> Option<Rc<CoordinatorRulers>> {
        self.rulers.borrow().clone()
    }

    pub(crate) fn provide_ruler_value(&self, ruler: Ruler, value: f32) {
        self.rulers().set(ruler, value);
    }

    pub(crate) fn provide_relative_ruler_value(&self, ruler: Ruler, value: f32) {
        let ltr = self.layout_node().is_none_or(|n| n.layout_direction() == LayoutDirection::Ltr);
        let value = if ltr { value } else { self.measured_size.get().width as f32 - value };
        self.rulers().set(ruler, value);
    }

    /// Where it is in the window, rounded, and its size (what a ruler provider reading its coordinates depends on).
    fn position_and_size(&self) -> (IntOffset, kompoz_graphics::IntSize) {
        let coordinates = LayoutCoordinates::of(self.rc());
        (coordinates.position_in_window().round(), coordinates.size())
    }

    /// `ruler`'s value where this places, else `default` (Compose's `findRulerValue`): from itself up, the first
    /// coordinator holding a value for it, or a provider that may provide it (run now, observed), mapped into this
    /// one; the reader is noted where it was found (at the root when nothing provides it).
    pub(crate) fn find_ruler_value(&self, ruler: &Ruler, default: f32) -> f32 {
        if self.placing_for_alignment.get() {
            return default;
        }
        let Some(reader) = self.layout_node() else { return default };
        let this = self.rc();
        let mut p = this.clone();
        loop {
            let rulers = p.rulers_if_any();
            if let Some(value) = rulers.as_ref().and_then(|r| r.value(ruler)) {
                p.add_ruler_reader(&reader, ruler);
                return ruler.calculate_coordinate(value, &p, &this);
            }
            if let Some(rulers) = &rulers {
                let provider = rulers.provider.borrow().clone();
                let provided = rulers.provided.borrow().clone();
                if let (Some(provider), Some(provided)) = (provider, provided)
                    && provided(ruler)
                {
                    p.provide_single_ruler(rulers, &provider, ruler);
                    p.add_ruler_reader(&reader, ruler);
                    if let Some(value) = rulers.value(ruler) {
                        return ruler.calculate_coordinate(value, &p, &this);
                    }
                }
            }
            match p.wrapped_by() {
                Some(parent) => p = parent,
                None => {
                    p.add_ruler_reader(&reader, ruler);
                    return default;
                }
            }
        }
    }

    /// Runs the provider for `ruler`, what it reads observed (a change drops the value and places its readers
    /// again: Compose's `onCommitAffectingSingleRuler`).
    fn provide_single_ruler(&self, rulers: &Rc<CoordinatorRulers>, provider: &RulerProvider, ruler: &Ruler) {
        let state = {
            let mut scopes = rulers.scopes.borrow_mut();
            match scopes.iter().find(|(r, _)| r == ruler) {
                Some((_, s)) => s.clone(),
                None => {
                    let s = Rc::new(RulerScopeState::default());
                    scopes.push((ruler.clone(), s.clone()));
                    s
                }
            }
        };
        state.coordinates_accessed.set(false);
        let this = self.rc();
        let density = self.layout_node().map(|n| n.density()).unwrap_or_default();
        let run = || {
            let scope = RulerScope { coordinator: &this, state: &state, density };
            provider(&scope, ruler);
        };
        match self.layout_node().and_then(|n| n.owner()) {
            Some(owner) => {
                let observer = owner.snapshot_observer().clone();
                let reads = {
                    let mut single = rulers.single_reads.borrow_mut();
                    match single.iter().find(|(r, _)| r == ruler) {
                        Some((_, s)) => s.clone(),
                        None => {
                            let weak = Rc::downgrade(&this);
                            let which = ruler.clone();
                            let s = observer.scope(move || {
                                if let Some(c) = weak.upgrade().filter(|c| c.is_attached()) {
                                    c.reset_single_ruler_read(&which);
                                }
                            });
                            single.push((ruler.clone(), s.clone()));
                            s
                        }
                    }
                };
                observer.observe(&reads, run);
            }
            None => run(),
        }
    }

    /// Notes that `reader` read `ruler` from here.
    fn add_ruler_reader(&self, reader: &Rc<LayoutNode>, ruler: &Ruler) {
        let rulers = self.rulers();
        let mut readers = rulers.readers.borrow_mut();
        for (_, set) in readers.iter_mut() {
            set.retain(|n| n.upgrade().is_some_and(|n| n.is_attached()));
        }
        readers.retain(|(_, set)| !set.is_empty());
        let weak = Rc::downgrade(reader);
        match readers.iter_mut().find(|(r, _)| r == ruler) {
            Some((_, set)) => {
                if !set.iter().any(|n| n.ptr_eq(&weak)) {
                    set.push(weak);
                }
            }
            None => readers.push((ruler.clone(), vec![weak])),
        }
    }

    /// The nearest coordinator from here up providing `ruler`, else the root (Compose's
    /// `findAncestorRulerDefiner`).
    fn find_ancestor_ruler_definer(&self, ruler: &Ruler) -> Rc<NodeCoordinator> {
        let mut p = self.rc();
        loop {
            if p.rulers_if_any().is_some_and(|r| r.contains(ruler)) {
                return p;
            }
            match p.wrapped_by() {
                Some(parent) => p = parent,
                None => return p,
            }
        }
    }

    /// What read `ruler` from the coordinator that provided it to what this places placed again: this provides
    /// it now (Compose's `invalidateChildrenOfDefiningRuler`).
    fn invalidate_children_of_defining_ruler(&self, ruler: &Ruler) {
        let definer = self.find_ancestor_ruler_definer(ruler);
        let readers = definer.rulers_if_any().map(|r| r.take_readers(ruler)).unwrap_or_default();
        relayout(readers);
    }

    /// Captures the rulers its measure result provides, if they have to be (Compose's `captureRulersIfNeeded`, as
    /// it is placed): another block or provider, or one that read its coordinates now that it moved.
    pub(crate) fn capture_rulers_if_needed(&self, result: Option<&MeasureResult>) {
        if self.placing_for_alignment.get() {
            return;
        }
        let Some(result) = result else {
            self.reset_provided_rulers();
            return;
        };
        if result.rulers.is_none() && result.ruler_provider.is_none() && self.rulers_if_any().is_none() {
            return;
        }
        let rulers = self.rulers();
        *rulers.result.borrow_mut() = Some(result.clone());
        if let Some((provided, provider)) = &result.ruler_provider {
            let same = rulers.provider.borrow().as_ref().is_some_and(|p| Rc::ptr_eq(p, provider))
                && rulers.provided.borrow().as_ref().is_some_and(|p| Rc::ptr_eq(p, provided));
            if !same {
                *rulers.provider.borrow_mut() = Some(provider.clone());
                *rulers.provided.borrow_mut() = Some(provided.clone());
                *rulers.lambda.borrow_mut() = None;
                self.reset_provided_rulers();
                return;
            }
            let accessed: Vec<(Ruler, Rc<RulerScopeState>)> = rulers
                .scopes
                .borrow()
                .iter()
                .filter(|(_, s)| s.coordinates_accessed.get())
                .map(|(r, s)| (r.clone(), s.clone()))
                .collect();
            if accessed.is_empty() {
                return;
            }
            let (position, size) = self.position_and_size();
            for (ruler, state) in rulers.scopes.borrow().iter() {
                let reevaluate = state.coordinates_accessed.get()
                    && (state.size.get() != size || state.position.get() != Some(position));
                state.size.set(size);
                state.position.set(Some(position));
                state.coordinates_accessed.set(false);
                if reevaluate {
                    rulers.remove(ruler);
                    relayout(rulers.take_readers(ruler));
                }
            }
            return;
        }
        let Some(lambda) = result.rulers.clone() else {
            // Nothing provided any more: what read a value placed again.
            self.reset_provided_rulers();
            *rulers.lambda.borrow_mut() = None;
            *rulers.provider.borrow_mut() = None;
            *rulers.provided.borrow_mut() = None;
            rulers.scope.coordinates_accessed.set(false);
            rulers.scope.position.set(None);
            return;
        };
        *rulers.provider.borrow_mut() = None;
        *rulers.provided.borrow_mut() = None;
        let mut recapture = !rulers.lambda.borrow().as_ref().is_some_and(|l| Rc::ptr_eq(l, &lambda));
        let mut at = (None, kompoz_graphics::IntSize::Zero);
        if !recapture && rulers.scope.coordinates_accessed.get() {
            let (position, size) = self.position_and_size();
            at = (Some(position), size);
            recapture = Some(position) != rulers.scope.position.get() || size != rulers.scope.size.get();
        }
        if recapture {
            self.capture_rulers(&rulers, &lambda, at.0, at.1);
            *rulers.lambda.borrow_mut() = Some(lambda);
        }
    }

    /// Runs the rulers block, what it reads observed (a change captures them again), then tells what read a value
    /// that changed, or that a nearer layout provides now (Compose's `captureRulers` and `notifyChanged`).
    fn capture_rulers(
        &self,
        rulers: &Rc<CoordinatorRulers>,
        lambda: &RulersBlock,
        position: Option<IntOffset>,
        size: kompoz_graphics::IntSize,
    ) {
        if rulers.capturing.replace(true) {
            return;
        }
        for entry in rulers.values.borrow_mut().iter_mut() {
            entry.2 = Access::NotSet;
        }
        let this = self.rc();
        let density = self.layout_node().map(|n| n.density()).unwrap_or_default();
        let run = || {
            rulers.scope.coordinates_accessed.set(false);
            rulers.scope.position.set(position);
            rulers.scope.size.set(size);
            let scope = RulerScope { coordinator: &this, state: &rulers.scope, density };
            lambda(&scope);
        };
        match self.layout_node().and_then(|n| n.owner()) {
            Some(owner) => {
                let observer = owner.snapshot_observer().clone();
                let kept = rulers.reads.borrow().clone();
                let reads = kept.unwrap_or_else(|| {
                    let weak = Rc::downgrade(&this);
                    let s = observer.scope(move || {
                        if let Some(c) = weak.upgrade().filter(|c| c.is_attached()) {
                            c.on_commit_affecting_rulers();
                        }
                    });
                    *rulers.reads.borrow_mut() = Some(s.clone());
                    s
                });
                observer.observe(&reads, run);
            }
            None => run(),
        }
        rulers.capturing.set(false);
        // What changed: its readers placed again; what is new: the readers of a farther provider of it.
        let mut changed = Vec::new();
        let mut new = Vec::new();
        {
            let mut values = rulers.values.borrow_mut();
            for (ruler, _, access) in values.iter() {
                match access {
                    Access::New => new.push(ruler.clone()),
                    Access::Changed | Access::NotSet => changed.push(ruler.clone()),
                    Access::NoChange => {}
                }
            }
            values.retain(|(_, _, access)| *access != Access::NotSet);
            for entry in values.iter_mut() {
                entry.2 = Access::NoChange;
            }
        }
        let mut nodes = Vec::new();
        for ruler in &changed {
            nodes.extend(rulers.take_readers(ruler));
        }
        let parent = self.wrapped_by();
        for ruler in &new {
            match &parent {
                Some(p) => p.invalidate_children_of_defining_ruler(ruler),
                None => self.invalidate_children_of_defining_ruler(ruler),
            }
        }
        relayout(nodes);
    }

    /// What the rulers block read changed (Compose's `onCommitAffectingRuler`): captured again from the last measure
    /// result.
    fn on_commit_affecting_rulers(&self) {
        let Some(rulers) = self.rulers_if_any() else { return };
        let Some(result) = rulers.result.borrow().clone() else { return };
        if result.ruler_provider.is_some() {
            self.reset_provided_rulers();
        } else if let Some(lambda) = result.rulers.clone() {
            self.capture_rulers(&rulers, &lambda, None, kompoz_graphics::IntSize::Zero);
            *rulers.lambda.borrow_mut() = Some(lambda);
        } else {
            *rulers.lambda.borrow_mut() = None;
            self.reset_provided_rulers();
        }
    }

    /// What the provider read for `ruler` changed: its value dropped, its readers placed again (Compose's
    /// `resetSingleRulerRead`).
    fn reset_single_ruler_read(&self, ruler: &Ruler) {
        let Some(rulers) = self.rulers_if_any() else { return };
        rulers.remove(ruler);
        relayout(rulers.take_readers(ruler));
    }

    /// Every value dropped, every reader placed again (Compose's `resetProvidedRulers`).
    pub(crate) fn reset_provided_rulers(&self) {
        let Some(rulers) = self.rulers_if_any() else { return };
        rulers.values.borrow_mut().clear();
        let readers = std::mem::take(&mut *rulers.readers.borrow_mut());
        for (_, nodes) in readers {
            relayout(nodes);
        }
    }
}
