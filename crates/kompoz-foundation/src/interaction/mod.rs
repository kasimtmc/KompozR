//! Interactions, as Compose's `androidx.compose.foundation.interaction`: what a user does to a component (presses,
//! hovers, focuses, drags it) as [`Interaction`]s a [`MutableInteractionSource`] carries to whoever collects its
//! [`interactions`](InteractionSource::interactions) — an indication drawing them, a component's own look
//! (`collect_is_pressed_as_state` and the others).
//!
//! Compose's interaction classes are the structs of the modules [`PressInteraction`], [`HoverInteraction`],
//! [`FocusInteraction`] and [`DragInteraction`] (`PressInteraction::Press::new(offset)`). An interaction is known by
//! its identity, as Compose's (a Release names the Press it ends): each is made with an id of its own, which its
//! clones keep.

/// An interaction type compared (and hashed) by its identity, as Compose's classes are.
macro_rules! identity_eq {
    ($t:ty) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }
        impl Eq for $t {}
        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }
    };
}

mod drag;
mod focus;
mod hover;
mod press;

pub use drag::DragInteraction;
pub use focus::FocusInteraction;
pub use hover::HoverInteraction;
pub use press::PressInteraction;

use kompoz_coroutines::{AsFlow, Flow, MutableSharedFlow, SharedFlow};
use std::any::Any;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// What a user did to a component (Compose's Interaction): an interface, not a closed set, so components can emit
/// their own. Interactions go through [`MutableInteractionSource`]s as `Arc<dyn Interaction>`; look at one with
/// [`downcast_ref`](<dyn Interaction>::downcast_ref).
pub trait Interaction: Any + Send + Sync + fmt::Debug {}

impl dyn Interaction {
    /// The interaction as a `T`, if it is one (Compose's `is T`).
    pub fn downcast_ref<T: Interaction>(&self) -> Option<&T> {
        let any: &dyn Any = self;
        any.downcast_ref::<T>()
    }

    /// Whether it is a `T`.
    pub fn is<T: Interaction>(&self) -> bool {
        self.downcast_ref::<T>().is_some()
    }
}

/// An interaction's identity (what Compose compares interactions by): unique to the interaction made, kept by its
/// clones.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InteractionId(u64);

impl InteractionId {
    /// A new identity.
    pub fn next() -> InteractionId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        InteractionId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// What can be emitted: an interaction, or one as a flow carries it.
pub trait IntoInteraction {
    fn into_interaction(self) -> Arc<dyn Interaction>;
}

impl<T: Interaction> IntoInteraction for T {
    fn into_interaction(self) -> Arc<dyn Interaction> {
        Arc::new(self)
    }
}

impl IntoInteraction for Arc<dyn Interaction> {
    fn into_interaction(self) -> Arc<dyn Interaction> {
        self
    }
}

/// A stream of [`Interaction`]s to read (Compose's InteractionSource): a component's interactions, as its
/// indication and its look follow them. Clones are the same source.
#[derive(Clone, PartialEq)]
pub struct InteractionSource {
    flow: SharedFlow<Arc<dyn Interaction>>,
}

impl InteractionSource {
    /// The interactions emitted from now on (Compose's `interactions`): what is emitted while nothing collects it
    /// is not seen.
    pub fn interactions(&self) -> Flow<Arc<dyn Interaction>> {
        self.flow.as_flow()
    }
}

impl fmt::Debug for InteractionSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InteractionSource")
    }
}

/// An [`InteractionSource`] that can be emitted to (Compose's MutableInteractionSource): what a component given one
/// emits its interactions to; remember one to read them (`remember(MutableInteractionSource::new)`). Clones are the
/// same source; it derefs to its [`InteractionSource`].
#[derive(Clone)]
pub struct MutableInteractionSource {
    flow: MutableSharedFlow<Arc<dyn Interaction>>,
    source: InteractionSource,
}

impl MutableInteractionSource {
    /// A source of its own (Compose's `MutableInteractionSource()`): every collector gets what is emitted, none is
    /// replayed to a new one.
    pub fn new() -> MutableInteractionSource {
        let flow = MutableSharedFlow::new(0);
        let source = InteractionSource { flow: flow.as_shared_flow() };
        MutableInteractionSource { flow, source }
    }

    /// Emits `interaction` to the collectors (Compose's suspending `emit`; it never waits).
    pub async fn emit(&self, interaction: impl IntoInteraction) {
        self.flow.emit(interaction.into_interaction()).await;
    }

    /// Emits `interaction` at once (Compose's `tryEmit`); whether it was.
    pub fn try_emit(&self, interaction: impl IntoInteraction) -> bool {
        self.flow.try_emit(interaction.into_interaction())
    }

    /// The source to read (what an indication is given).
    pub fn as_interaction_source(&self) -> InteractionSource {
        self.source.clone()
    }
}

impl Default for MutableInteractionSource {
    fn default() -> Self {
        MutableInteractionSource::new()
    }
}

impl std::ops::Deref for MutableInteractionSource {
    type Target = InteractionSource;
    fn deref(&self) -> &InteractionSource {
        &self.source
    }
}

impl PartialEq for MutableInteractionSource {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl fmt::Debug for MutableInteractionSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MutableInteractionSource")
    }
}

impl From<&MutableInteractionSource> for InteractionSource {
    fn from(source: &MutableInteractionSource) -> InteractionSource {
        source.as_interaction_source()
    }
}

impl From<MutableInteractionSource> for InteractionSource {
    fn from(source: MutableInteractionSource) -> InteractionSource {
        source.source
    }
}

/// Collects `source`'s interactions while the caller is composed, keeping the state `track` gives: Compose's
/// `collectIs…AsState` bodies (`track` adds a start, takes away the end that names one, and says whether any is
/// left).
#[track_caller]
fn collect_tracking_as_state<S: 'static>(
    source: &InteractionSource,
    track: impl Fn(&mut Vec<S>, &dyn Interaction) + 'static,
) -> kompoz_runtime::State<bool> {
    kompoz_runtime::group(|| {
        let state = kompoz_runtime::remember(|| kompoz_runtime::mutable_state_of(false));
        let (flow, s) = (source.interactions(), state.clone());
        let mut started: Vec<S> = Vec::new();
        let collecting = flow.collect(move |interaction| {
            track(&mut started, &*interaction);
            s.set(!started.is_empty());
        });
        let key = source.clone();
        kompoz_runtime::kompozable_block!({
            kompoz_coroutines::LaunchedEffect(key, collecting);
        });
        kompoz_runtime::State::from(state)
    })
}
