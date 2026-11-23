//! What composables keep a loaded resource in (CMP's ResourceState.kt, desktop's blocking form): a state made by
//! loading it at once (Kotlin's `runBlocking`) for the environment here, made again when the keys or the
//! environment change.

use crate::caches::run_blocking;
use crate::environment::{LocalComposeEnvironment, ResourceEnvironment};
use kompoz_runtime::{State, kompozable, mutable_confined_state_of_with_policy, never_equal_policy, remember_keyed};
use std::future::Future;

/// The state of `block`'s value for the environment here, loaded now; kept while `keys` and the environment stay
/// (CMP's blocking `rememberResourceState`; `get_default` is what an asynchronous platform shows meanwhile, unused
/// here).
#[kompozable]
pub(crate) fn remember_resource_state<K, T, F>(
    keys: K,
    get_default: fn() -> T,
    block: impl FnOnce(ResourceEnvironment) -> F,
) -> State<T>
where
    K: PartialEq + 'static,
    T: Clone + 'static,
    F: Future<Output = T>,
{
    let _ = get_default;
    let environment = LocalComposeEnvironment.current().remember_environment();
    remember_keyed((keys, environment.clone()), move || {
        mutable_confined_state_of_with_policy(run_blocking(block(environment)), never_equal_policy()).into()
    })
}
