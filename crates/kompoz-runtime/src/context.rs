//! Child compositions (Compose's subcomposition): a [`CompositionContext`] taken at a place in a composition
//! ([`remember_kompozition_context`]) links the compositions made from it to that place — they see the composition
//! locals provided there, their composite keys continue its key (so saveable state is found again by them), and
//! they are recomposed with their parent's, after it. Content composed in them can be reused
//! ([`ReusableContent`], [`ReusableContentHost`]) and composed a piece at a time ([`PausedComposition`]).

use crate::composer::{Composer, GroupId, Provided, key_of};
use crate::kompozable;
use crate::local::KompozitionLocalContext;
use crate::snapshot::StateKey;
use std::cell::{Cell, RefCell};
use std::panic::Location;
use std::rc::{Rc, Weak};

/// What a root composition's child compositions share (Compose's Recomposer, as far as they need one): the time
/// of the frame it last composed, which a child composing at once (in a measure pass) composes at.
#[derive(Default)]
pub(crate) struct Recomposer {
    pub(crate) frame_time: Cell<i64>,
}

/// A context's state: where it was taken, what is provided there, the compositions made from it.
pub(crate) struct ContextData {
    /// The composition it was taken in, and its group there.
    pub(crate) composer: Weak<Composer>,
    pub(crate) group: Cell<Option<GroupId>>,
    /// The locals provided where it was taken, as of the last composition of that place (outermost first).
    pub(crate) locals: RefCell<Vec<Provided>>,
    /// What a child composition's scope reading a local nothing provides, or all of them, reads: told when the
    /// locals provided there are others.
    pub(crate) locals_key: StateKey,
    pub(crate) children: RefCell<Vec<Weak<Composer>>>,
    pub(crate) recomposer: Rc<Recomposer>,
}

/// A place in a composition that child compositions are made from (Compose's `CompositionContext`, from
/// `rememberCompositionContext`): give it to [`Composition::with_parent`](crate::Composition::with_parent). The
/// compositions made from it see the composition locals provided where it was taken (and are told when they
/// change), continue its composite key, and are recomposed with the composition it was taken in, after it.
/// Clones are the same context.
#[derive(Clone)]
pub struct CompositionContext(pub(crate) Rc<ContextData>);

impl PartialEq for CompositionContext {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl CompositionContext {
    /// The composite key where it was taken (Compose's `compositeKeyHashCode`), which its child compositions'
    /// keys continue; 0 once that composition is gone.
    pub fn composite_key_hash(&self) -> u64 {
        match self.0.composer.upgrade() {
            Some(c) => c.key_from(self.0.group.get(), None),
            None => 0,
        }
    }

    /// The composition locals provided where it was taken (Compose's `getCompositionLocalScope`).
    pub fn locals(&self) -> KompozitionLocalContext {
        KompozitionLocalContext::from_values(self.0.locals.borrow().clone())
    }

    /// Whether the composition it was taken in is still there.
    pub fn is_alive(&self) -> bool {
        self.0.composer.strong_count() > 0
    }
}

/// A context at this place of the composition, for child compositions (Compose's `rememberCompositionContext`):
/// the same one while this place stays; what it gives them (the locals provided here) is updated whenever this
/// place composes again, and a child composition's readers of what changed are told.
#[track_caller]
pub fn remember_kompozition_context() -> CompositionContext {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let kept = c.take_slot().and_then(|slot| {
        let ctx = slot.value.downcast_ref::<CompositionContext>().cloned();
        c.put_slot(slot);
        ctx
    });
    let ctx = match kept {
        Some(ctx) => ctx,
        None => {
            let ctx = CompositionContext(Rc::new(ContextData {
                composer: Rc::downgrade(&c),
                group: Cell::new(Some(c.current_group())),
                locals: RefCell::new(Vec::new()),
                locals_key: StateKey::new(),
                children: RefCell::new(Vec::new()),
                recomposer: c.recomposer.clone(),
            }));
            c.contexts.borrow_mut().push(Rc::downgrade(&ctx.0));
            c.set_remembered(Box::new(ctx.clone()), None);
            ctx
        }
    };
    ctx.0.group.set(Some(c.current_group()));
    update_locals(&ctx, c.locals_kept());
    c.end_group();
    ctx
}

/// The locals a context gives are `new`: the readers in its child compositions of those replaced (a static
/// local's holder, a context provided again) are told, and if other locals are provided, those that read one
/// nothing provided (or all of them) too. (A dynamic local given another value is told as any state is.)
fn update_locals(ctx: &CompositionContext, new: Vec<Provided>) {
    let old = ctx.0.locals.replace(new);
    let new = ctx.0.locals.borrow();
    let others = old.len() != new.len() || old.iter().zip(new.iter()).any(|(a, b)| a.local != b.local);
    let replaced: Vec<StateKey> = old
        .iter()
        .filter(|o| !new.iter().any(|n| n.local == o.local && Rc::ptr_eq(&n.holder, &o.holder)))
        .map(|o| o.holder.key)
        .collect();
    drop(new);
    if replaced.is_empty() && !others {
        return;
    }
    let children: Vec<Rc<Composer>> = ctx.0.children.borrow().iter().filter_map(|c| c.upgrade()).collect();
    for child in children {
        for key in &replaced {
            child.invalidate_readers(*key);
        }
        if others {
            child.invalidate_readers(ctx.0.locals_key);
        }
    }
    drop(old);
}

// ---------------------------------------------------------------------------------------------------------------
// Reusable content

/// Content whose state goes when `key` changes but whose nodes stay (Compose's `ReusableContent`): composed
/// again with another key, the groups it had that match are composed as new ones — what they remembered
/// forgotten, their effects disposed of and run again — and the nodes they emit are reused
/// ([`Node::on_reuse`](crate::Node::on_reuse)) rather than made again.
#[kompozable(inline)]
pub fn ReusableContent<K: PartialEq + 'static>(key: K, content: impl FnOnce()) {
    // (in the group of its own an inline composable has)
    let c = Composer::require();
    let changed = match c.take_slot() {
        Some(mut slot) => {
            let changed = match slot.value.downcast_mut::<K>() {
                Some(old) if *old == key => false,
                Some(old) => {
                    *old = key;
                    true
                }
                None => true,
            };
            c.put_slot(slot);
            changed
        }
        None => {
            // (the first time; or its key forgotten as content around it is reused, which reuses this too)
            c.set_remembered(Box::new(key), None);
            false
        }
    };
    if changed {
        c.begin_reuse();
    }
    content();
    if changed {
        c.end_reuse();
    }
}

/// Content that can be deactivated and activated again (Compose's `ReusableContentHost`): while `active` it is
/// composed; turned off, what it remembered is forgotten and its effects disposed of, but its groups and nodes are
/// kept, deactivated ([`Node::on_deactivate`](crate::Node::on_deactivate)), until it is active again and composed
/// reusing them, as [`ReusableContent`] with another key.
#[kompozable(inline)]
pub fn ReusableContentHost(active: bool, content: impl FnOnce()) {
    let c = Composer::require();
    let was_active = match c.take_slot() {
        Some(mut slot) => {
            let was = slot.value.downcast_mut::<bool>().map(|v| std::mem::replace(v, active));
            c.put_slot(slot);
            was
        }
        None => {
            c.set_remembered(Box::new(active), None);
            None
        }
    };
    if active {
        let reactivated = was_active == Some(false);
        if reactivated {
            c.begin_reuse();
        }
        content();
        if reactivated {
            c.end_reuse();
        }
    } else {
        if was_active != Some(false) {
            c.deactivate_rest();
        }
        c.skip_rest();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Paused compositions

/// Whether a paused composition should pause (Compose's `ShouldPauseCallback`): a closure `|| -> bool`, asked at
/// each restartable call; true asks for a pause there (it may not be honoured where nothing can pause).
pub trait ShouldPauseCallback {
    fn should_pause(&self) -> bool;
}

impl<F: Fn() -> bool> ShouldPauseCallback for F {
    fn should_pause(&self) -> bool {
        self()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PausedState {
    Pending,
    Applied,
    Cancelled,
}

/// A composition's content composed a piece at a time (Compose's `PausedComposition`, from
/// [`Composition::set_pausable_content`](crate::Composition::set_pausable_content)): [`resume`](Self::resume) it
/// until it is [complete](Self::is_complete), pausing at restartable calls when asked to (between frames: a lazy
/// list's items composed before they scroll in), then [`apply`](Self::apply) it, which runs what it remembered
/// and its effects as a composition's end does. Until applied, it isn't recomposed and what it composed isn't to be
/// used. A state it read that changes before it is applied makes it incomplete again.
pub struct PausedComposition {
    composer: Rc<Composer>,
    state: Cell<PausedState>,
}

impl PausedComposition {
    pub(crate) fn new(composer: Rc<Composer>) -> PausedComposition {
        PausedComposition { composer, state: Cell::new(PausedState::Pending) }
    }

    fn check_pending(&self) {
        match self.state.get() {
            PausedState::Pending => {}
            PausedState::Applied => panic!("kompoz: the paused composition was applied already"),
            PausedState::Cancelled => panic!("kompoz: the paused composition was cancelled"),
        }
    }

    /// Whether nothing is left to compose (Compose's `isComplete`): what [`resume`](Self::resume) last said,
    /// unless a state it read changed since.
    pub fn is_complete(&self) -> bool {
        self.state.get() == PausedState::Pending && self.composer.paused_complete()
    }

    /// Whether it was applied.
    pub fn is_applied(&self) -> bool {
        self.state.get() == PausedState::Applied
    }

    /// Whether it was cancelled (its composition is then to be disposed of).
    pub fn is_cancelled(&self) -> bool {
        self.state.get() == PausedState::Cancelled
    }

    /// Composes what is left, pausing at a restartable call when `should_pause` says so (Compose's `resume`); its
    /// first call composes the content. Whether it is complete. Panics once applied or cancelled.
    pub fn resume(&self, should_pause: impl ShouldPauseCallback + 'static) -> bool {
        self.check_pending();
        self.composer.resume_paused(Rc::new(move || should_pause.should_pause()))
    }

    /// Applies it (Compose's `apply`): what it remembered is told so and its effects run, as at the end of a
    /// composition, and its composition is recomposed as any other from now on. Panics unless it is complete.
    pub fn apply(&self) {
        self.check_pending();
        assert!(
            self.composer.paused_complete(),
            "kompoz: a paused composition applied before it is complete: resume it until it is"
        );
        self.state.set(PausedState::Applied);
        self.composer.apply_paused();
    }

    /// Cancels it (Compose's `cancel`): what it remembered is abandoned, its effects never run; its composition is
    /// to be disposed of.
    pub fn cancel(&self) {
        if self.state.get() != PausedState::Pending {
            return;
        }
        self.state.set(PausedState::Cancelled);
        self.composer.cancel_paused();
    }
}
