//! KompozR's coroutines: Rust futures run on the UI thread by a frame-clock [`Dispatcher`], with Kotlin's
//! structured concurrency (jobs, scopes, `coroutine_scope`, `with_timeout`), work off the UI thread
//! (`with_background`), flows (`Flow`, `StateFlow`, `SharedFlow`), channels, and their place in the composition
//! (`LaunchedEffect`, `remember_coroutine_scope`, `produce_state`, `collect_as_state`, `snapshot_flow`).

#![allow(non_upper_case_globals)]

mod background;
mod channel;
mod compose;
mod context;
mod executor;
mod flow;
mod hot;
mod scope;
// (WP5: snackbars — kotlinx.coroutines.sync's Mutex, SnackbarHostState's queue)
pub mod sync;
mod time;

pub use background::{Background, background_work_in_flight, run_in_background, with_background};
pub use channel::{BufferOverflow, Channel, ReceiveFuture, SendFuture, produce};
pub use compose::{
    LaunchedEffect, LocalDispatcher, LocalEffectCoroutineContext, current_effect_coroutine_context, produce_state,
    remember_coroutine_scope, snapshot_flow,
};
pub use context::{CoroutineContext, current_coroutine_context, with_context};
pub use executor::{
    Cancelled, CoroutineScope, CoroutineStart, Deferred, Dispatcher, DisposableHandle, Failure, Job, cancel_current,
    current_job, run_catching, r#yield,
};
pub use flow::{Flow, FlowCollector, combine, empty_flow, flow, flow_of};
pub use hot::{AsFlow, MutableSharedFlow, MutableStateFlow, SharedFlow, SharingStarted, StateFlow};
pub use scope::{
    TimeoutCancellation, catch_cancellation, coroutine_scope, supervisor_scope, with_timeout, with_timeout_or_null,
};
pub use time::{
    AwaitFrame, AwaitFrameEnd, Delay, DelayTime, await_frame, await_frame_end, delay, delay_ms, with_frame_millis,
    with_frame_nanos,
};
