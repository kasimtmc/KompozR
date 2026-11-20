//! Material's mutator mutex, as Compose's internal InternalMutatorMutex.kt: "an internal copy of
//! androidx.compose.foundation.MutatorMutex with an additional tryMutate method" (its JVM actual's
//! `InternalAtomicReference` a `java.util.concurrent.atomic.AtomicReference`).
//!
//! KompozR's one mutator mutex, kompoz-animation's [`MutatorMutex`] (what foundation re-exports), behaves as this
//! copy does, `try_mutate` included: `mutate` and `mutate_with` make the new mutator the current one, cancelling the
//! old one if its priority is no higher (else the new one is cancelled, "Current mutation had a higher priority"),
//! then run the block under the lock, clearing the current mutator after it if it is still theirs; `try_mutate`
//! runs its block at once if the lock is free, cancelling nothing, and says whether it did. The current mutator
//! is one value only the UI thread sets (KompozR's coroutines run there), which the atomic reference's
//! compare-and-set loop guarded; it is used as it is.

#[allow(unused_imports)] // (used as the components come)
pub(crate) use kompoz_animation::MutatorMutex as InternalMutatorMutex;
