//! Text input sessions as Compose 1.7+ has them (`androidx.compose.ui.platform.PlatformTextInputModifierNode` and
//! its desktop request, Compose desktop's `PlatformTextInputMethodRequest`): a modifier node that takes typing
//! ([`PlatformTextInputModifierNode`]) establishes a session from its coroutine
//! ([`establish_text_input_session`]), and inside it starts the platform's input method with a request
//! ([`PlatformTextInputSession::start_input_method`]) saying how to read the editor's state and how to give it what
//! the input method does. A new session cancels the one before (the owner's session mutex); a new input method
//! request cancels the one before within a session. [`InterceptPlatformTextInput`] puts a
//! [`PlatformTextInputInterceptor`] between the fields below it and the platform.
//!
//! Underneath is the host's [`TextInputService`]: a request becomes a session of it, its editor read again after
//! each batch of commands and whenever what it holds changes, where the input method is shown changes, or its
//! selection leaves a composition (which is finished then, as Compose desktop's DesktopTextInputService2 does).

use crate::clipboard::ClipEntry;
use crate::modifier::{ModifierNode, NodeCx};
use crate::platform::{ImeAction, ImeOptions};
use crate::text_input::{ContentReceiver, EditCommand, LocalTextInputService, TextInputService, TextInputSession};
use kompoz_coroutines::{CoroutineScope, CoroutineStart, Job, coroutine_scope, current_job, snapshot_flow};
use kompoz_graphics::{Offset, Rect};
use kompoz_runtime::{KompozitionLocal, KompozitionLocalProvider, MutableState, mutable_state_of, remember_keyed};
use kompoz_text::input::TextFieldValue;
use kompoz_text::{TextLayoutResult, TextRange};
use std::cell::RefCell;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// A boxed future of a session: it never ends on its own (Kotlin's `Nothing`), only by being cancelled.
pub type SessionFuture = Pin<Box<dyn Future<Output = Infallible>>>;

// ---------------------------------------------------------------------------------------------------------------
// The session mutex

/// One session at a time, a new one cancelling the one before (Compose's `SessionMutex`).
#[derive(Clone, Default)]
pub(crate) struct SessionMutex(Rc<RefCell<Option<Job>>>);

/// Lets go of the mutex when its session ends, unless a newer one holds it (Compose's `compareAndSet(…, null)`).
struct ReleaseOnDrop(Rc<RefCell<Option<Job>>>, Job);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let mut current = self.0.borrow_mut();
        if current.as_ref() == Some(&self.1) {
            current.take();
        }
    }
}

impl SessionMutex {
    pub(crate) fn new() -> SessionMutex {
        SessionMutex::default()
    }

    /// Cancels the session before (awaiting its end) and runs `session` with what `initializer` makes of a scope of
    /// the session's own (Compose's `withSessionCancellingPrevious`): it in turn is cancelled when another one
    /// starts.
    pub(crate) async fn with_session_cancelling_previous<T: 'static, Fut: Future<Output = Infallible> + 'static>(
        &self,
        initializer: impl FnOnce(CoroutineScope) -> T + 'static,
        session: impl FnOnce(T) -> Fut + 'static,
    ) -> Infallible {
        let holder = self.0.clone();
        coroutine_scope(move |scope| async move {
            let job = current_job();
            let value = initializer(scope);
            let previous = holder.borrow_mut().replace(job.clone());
            if let Some(previous) = previous {
                previous.cancel();
                previous.join().await;
            }
            let _release = ReleaseOnDrop(holder, job);
            session(value).await
        })
        .await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The request

/// The editor's state as an input method reads it (Compose desktop's TextEditorState).
pub trait TextEditorState {
    /// Its text.
    fn text(&self) -> String;
    /// Its selection.
    fn selection(&self) -> TextRange;
    /// What is being composed, if anything.
    fn composition(&self) -> Option<TextRange>;
    /// The text's length (bytes).
    fn length(&self) -> usize {
        self.text().len()
    }
}

/// What an input method may do to the editor's text, applied together after the block that does it (Compose
/// desktop's TextEditingScope). Lengths and positions in bytes, whole characters; code points where said.
pub trait TextEditingScope {
    /// Deletes `before` code points before the selection and `after` after it (as
    /// [`EditCommand::DeleteSurroundingTextInCodePoints`]).
    fn delete_surrounding_text_in_code_points(&mut self, before: usize, after: usize);
    /// Commits `text`, the caret then `new_cursor_position` from it (as [`EditCommand::CommitText`]).
    fn commit_text(&mut self, text: &str, new_cursor_position: i32);
    /// Composes `text`, the caret then `new_cursor_position` from it (as [`EditCommand::SetComposingText`]).
    fn set_composing_text(&mut self, text: &str, new_cursor_position: i32);
    /// Finishes composing, the text as it is (as [`EditCommand::FinishComposingText`]).
    fn finish_composing_text(&mut self);
}

/// A field's request for the platform's input method (Compose desktop's PlatformTextInputMethodRequest): how to read
/// the editor, what it wants of the input method, where to give what the input method does, where it is.
#[derive(Clone)]
pub struct PlatformTextInputMethodRequest {
    /// The editor's value now (read in a snapshot).
    pub value: Rc<dyn Fn() -> TextFieldValue>,
    /// The editor's state.
    pub state: Rc<dyn TextEditorState>,
    /// The keyboard it wants: single line, autocorrect, …
    pub ime_options: ImeOptions,
    /// Applies the input method's commands to the editor.
    pub on_edit_command: Rc<dyn Fn(&[EditCommand])>,
    /// The input method's action key.
    pub on_ime_action: Option<Rc<dyn Fn(ImeAction)>>,
    /// The editor's text layout, if laid out.
    pub text_layout_result: Rc<dyn Fn() -> Option<TextLayoutResult>>,
    /// Where editing happens (the caret), in the root, if laid out.
    pub focused_rect_in_root: Rc<dyn Fn() -> Option<Rect>>,
    /// The field, in the root, if laid out.
    pub text_field_rect_in_root: Rc<dyn Fn() -> Option<Rect>>,
    /// What shows of the text, in the root, if laid out.
    pub text_clipping_rect_in_root: Rc<dyn Fn() -> Option<Rect>>,
    /// Where the text's layout's top left is in the root (it moves as the field scrolls), if laid out.
    pub unclipped_text_offset_in_root: Rc<dyn Fn() -> Option<Offset>>,
    /// Edits the text: what the block does through its scope is applied together after it.
    pub edit_text: Rc<dyn Fn(&mut dyn FnMut(&mut dyn TextEditingScope))>,
    /// The MIME types of content the editor takes from the input method besides text (Android's
    /// `EditorInfo.contentMimeTypes`; `*/*`: any); empty: text only.
    pub content_mime_types: Vec<String>,
    /// What takes content the input method commits (an image a keyboard sends: Android's `commitContent`), given it
    /// and its link URI: whether it took it. None: text only.
    pub on_commit_content: Option<Rc<dyn Fn(ClipEntry, Option<String>) -> bool>>,
}

// ---------------------------------------------------------------------------------------------------------------
// Sessions

/// What starts the platform's input method in a session (Compose's PlatformTextInputSession): the owner's, or an
/// interceptor's next handler. Cheap to clone.
#[derive(Clone)]
pub struct PlatformTextInputSession(Rc<dyn Fn(PlatformTextInputMethodRequest) -> SessionFuture>);

impl PlatformTextInputSession {
    /// One starting input methods with `start`.
    pub fn new(start: impl Fn(PlatformTextInputMethodRequest) -> SessionFuture + 'static) -> PlatformTextInputSession {
        PlatformTextInputSession(Rc::new(start))
    }

    /// Starts the input method for `request` until cancelled (Compose's `startInputMethod`): a call made again in
    /// the session cancels the one before.
    pub fn start_input_method(&self, request: PlatformTextInputMethodRequest) -> SessionFuture {
        (self.0)(request)
    }
}

/// A session's scope (Compose's PlatformTextInputSessionScope): it starts input methods, and launches what the
/// session runs alongside (cancelled with it).
#[derive(Clone)]
pub struct PlatformTextInputSessionScope {
    session: PlatformTextInputSession,
    scope: CoroutineScope,
}

impl PlatformTextInputSessionScope {
    /// Starts the input method for `request` until cancelled (Compose's `startInputMethod`).
    pub fn start_input_method(&self, request: PlatformTextInputMethodRequest) -> SessionFuture {
        self.session.start_input_method(request)
    }

    /// The session as a handler (what an interceptor is given as the next one).
    pub fn session(&self) -> PlatformTextInputSession {
        self.session.clone()
    }

    /// The session's coroutine scope (Kotlin's CoroutineScope the session scope is).
    pub fn coroutine_scope(&self) -> &CoroutineScope {
        &self.scope
    }
}

#[kompoz_runtime::named]
impl PlatformTextInputSessionScope {
    /// Launches `coroutine` in the session's scope, as `start` says (the scope is a Kotlin CoroutineScope:
    /// [`CoroutineScope::launch`]).
    ///
    /// Kotlin: `launch(start: CoroutineStart = CoroutineStart.DEFAULT, block: suspend CoroutineScope.() -> Unit): Job`
    pub fn launch(
        &self,
        #[default(CoroutineStart::DEFAULT)] start: CoroutineStart,
        coroutine: impl Future<Output = ()> + 'static,
    ) -> Job {
        kompoz_runtime::named_args!(self.scope.launch(start = start, coroutine = coroutine))
    }
}

/// A modifier node that takes typing (Compose's PlatformTextInputModifierNode): it may
/// [`establish_text_input_session`]s.
pub trait PlatformTextInputModifierNode: ModifierNode {}

/// Establishes a text input session for `node` (attached: `cx` is its context; Compose's
/// `establishTextInputSession`): the interceptors around it (innermost first) and the host's text input service
/// get `block`, which must start the input method ([`PlatformTextInputSessionScope::start_input_method`]). It runs
/// until cancelled (the node's coroutine cancelled, or another session established anywhere in the window).
pub fn establish_text_input_session<N: PlatformTextInputModifierNode + ?Sized, Fut>(
    node: &N,
    cx: &NodeCx,
    block: impl FnOnce(PlatformTextInputSessionScope) -> Fut + 'static,
) -> SessionFuture
where
    Fut: Future<Output = Infallible> + 'static,
{
    let _ = node;
    assert!(cx.is_attached(), "establishTextInputSession called from an unattached node");
    let service = cx.current_value_of(&LocalTextInputService);
    let interceptor = cx.current_value_of(&LocalChainedPlatformTextInputInterceptor);
    let block: SessionBlock = Box::new(move |scope| Box::pin(block(scope)));
    intercepted_text_input_session(service, interceptor.0, block)
}

type SessionBlock = Box<dyn FnOnce(PlatformTextInputSessionScope) -> SessionFuture>;

fn intercepted_text_input_session(
    service: TextInputService,
    interceptor: Option<Rc<ChainedPlatformTextInputInterceptor>>,
    session: SessionBlock,
) -> SessionFuture {
    match interceptor {
        None => service.text_input_session(session),
        Some(chained) => chained.text_input_session(service, session),
    }
}

impl TextInputService {
    /// A session of the 1.7+ API (Compose desktop's RootNodeOwner `textInputSession`): `session` gets a scope
    /// whose input method requests become sessions of this service, the one before cancelled.
    fn text_input_session(&self, session: SessionBlock) -> SessionFuture {
        let mutex = self.text_input_sessions();
        let service = self.clone();
        Box::pin(async move {
            mutex
                .with_session_cancelling_previous(
                    move |scope| {
                        let inner = SessionMutex::new();
                        let start = PlatformTextInputSession::new(move |request| {
                            let (inner, service) = (inner.clone(), service.clone());
                            Box::pin(async move {
                                inner
                                    .with_session_cancelling_previous(
                                        |_| (),
                                        move |_| service.start_input_method(request),
                                    )
                                    .await
                            })
                        });
                        PlatformTextInputSessionScope { session: start, scope }
                    },
                    session,
                )
                .await
        })
    }

    /// Starts the platform's input method for `request` until cancelled (Compose desktop's PlatformContext
    /// `startInputMethod`, DesktopTextInputService2's): a session of this service reading the request's editor;
    /// while it runs, its editor's value and where it is are followed, and a composition is finished when the
    /// selection leaves its end. Cancelled, the session closes.
    pub fn start_input_method(&self, request: PlatformTextInputMethodRequest) -> SessionFuture {
        let service = self.clone();
        Box::pin(async move {
            let on_ime_action: Rc<dyn Fn(ImeAction)> = match request.on_ime_action.clone() {
                Some(f) => f,
                None => Rc::new(|_| {}),
            };
            let session = service.start_input_reading(
                request.ime_options.clone(),
                request.value.clone(),
                request.on_edit_command.clone(),
                on_ime_action,
            );
            let version = session.version();
            let content = request.on_commit_content.clone().map(|on_commit_content| ContentReceiver {
                mime_types: request.content_mime_types.clone(),
                on_commit_content,
            });
            service.set_content_receiver(&session, content);
            let _close = CloseOnDrop(session);
            coroutine_scope(move |scope| async move {
                // What the editor holds, followed: the input method told where it differs from what it sees. (With
                // the session's version: the editor going back to a value it had, after the input method's commands
                // moved its view, is a change.)
                let (value, s) = (request.value.clone(), service.clone());
                scope.launch(snapshot_flow(move || (value(), version.get())).collect(move |_| s.resync_open()));
                // Where it is, followed.
                let (rect, s) = (request.focused_rect_in_root.clone(), service.clone());
                scope.launch(snapshot_flow(move || rect()).collect(move |r| {
                    if let Some(r) = r {
                        s.notify_open_focused_rect(r);
                    }
                }));
                // A selection leaving the composition (a click elsewhere) finishes it. (Compose desktop finishes it
                // on any selection but a caret at its end: its AWT input methods never put one inside; KompozR's
                // platforms do, as Android's input methods do — text-input-v3's preedit cursor, XIM's caret.)
                let (state, edit) = (request.state.clone(), request.edit_text.clone());
                scope.launch(snapshot_flow(move || (state.selection(), state.composition())).collect(
                    move |(selection, composition)| {
                        let Some(composition) = composition else { return };
                        if selection.min() < composition.min() || selection.max() > composition.max() {
                            edit(&mut |scope: &mut dyn TextEditingScope| scope.finish_composing_text());
                        }
                    },
                ));
                std::future::pending::<Infallible>().await
            })
            .await
        })
    }
}

/// Closes a session when the coroutine running it ends.
struct CloseOnDrop(TextInputSession);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.dispose();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Interceptors

/// Intercepts the input method requests of the fields below an [`InterceptPlatformTextInput`] (Compose's
/// PlatformTextInputInterceptor): it may change the request, start the input method through `next_handler` (or
/// not), or start something else; what it gives never ends on its own.
pub trait PlatformTextInputInterceptor {
    fn intercept_start_input_method(
        &self,
        request: PlatformTextInputMethodRequest,
        next_handler: PlatformTextInputSession,
    ) -> SessionFuture;
}

impl<F: Fn(PlatformTextInputMethodRequest, PlatformTextInputSession) -> SessionFuture> PlatformTextInputInterceptor
    for F
{
    fn intercept_start_input_method(
        &self,
        request: PlatformTextInputMethodRequest,
        next_handler: PlatformTextInputSession,
    ) -> SessionFuture {
        self(request, next_handler)
    }
}

/// An interceptor in the chain of those around a field (Compose's ChainedPlatformTextInputInterceptor): its
/// interceptor may change while a session runs (the request is intercepted again by the new one then).
struct ChainedPlatformTextInputInterceptor {
    interceptor: RefCell<Rc<dyn PlatformTextInputInterceptor>>,
    /// Counted up as the interceptor changes (what a running session follows).
    version: MutableState<u64>,
    parent: Option<Rc<ChainedPlatformTextInputInterceptor>>,
}

impl ChainedPlatformTextInputInterceptor {
    fn update_interceptor(&self, interceptor: Rc<dyn PlatformTextInputInterceptor>) {
        let same = Rc::ptr_eq(&*self.interceptor.borrow(), &interceptor);
        if !same {
            *self.interceptor.borrow_mut() = interceptor;
            self.version.set(self.version.peek() + 1);
        }
    }

    /// A session through this interceptor (and those around it): the session's input method requests go to the
    /// interceptor, with the session outside as its next handler.
    fn text_input_session(self: Rc<Self>, service: TextInputService, session: SessionBlock) -> SessionFuture {
        let me = self.clone();
        intercepted_text_input_session(
            service,
            self.parent.clone(),
            Box::new(move |parent_scope: PlatformTextInputSessionScope| {
                let parent_session = parent_scope.session();
                let input_method_mutex = SessionMutex::new();
                let start = PlatformTextInputSession::new(move |request| {
                    let (mutex, me, parent_session) = (input_method_mutex.clone(), me.clone(), parent_session.clone());
                    Box::pin(async move {
                        mutex
                            .with_session_cancelling_previous(
                                |_| (),
                                move |_| async move {
                                    // The interceptor followed: a new one intercepts the request again.
                                    coroutine_scope(move |scope| async move {
                                        let running: Rc<RefCell<Option<Job>>> = Rc::new(RefCell::new(None));
                                        let version = me.version.clone();
                                        let watched = me.clone();
                                        snapshot_flow(move || version.get())
                                            .collect(move |_| {
                                                if let Some(job) = running.borrow_mut().take() {
                                                    job.cancel();
                                                }
                                                let interceptor = watched.interceptor.borrow().clone();
                                                let (request, next) = (request.clone(), parent_session.clone());
                                                let job = scope.launch(async move {
                                                    interceptor.intercept_start_input_method(request, next).await;
                                                });
                                                *running.borrow_mut() = Some(job);
                                            })
                                            .await;
                                        panic!("Interceptors flow should never terminate.")
                                    })
                                    .await
                                },
                            )
                            .await
                    })
                });
                session(PlatformTextInputSessionScope { session: start, scope: parent_scope.scope.clone() })
            }),
        )
    }
}

/// The interceptors around (Compose's LocalChainedPlatformTextInputInterceptor).
#[derive(Clone, Default)]
struct ChainedInterceptorRef(Option<Rc<ChainedPlatformTextInputInterceptor>>);

impl PartialEq for ChainedInterceptorRef {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

static LocalChainedPlatformTextInputInterceptor: KompozitionLocal<ChainedInterceptorRef> =
    KompozitionLocal::new_static(ChainedInterceptorRef::default);

/// Intercepts the input method requests of the text fields in `content` (Compose's InterceptPlatformTextInput):
/// innermost interceptors first, each given the next one out (the platform last). An interceptor changed while a
/// session runs intercepts its request again.
#[kompoz_runtime::kompozable]
pub fn InterceptPlatformTextInput(interceptor: Rc<dyn PlatformTextInputInterceptor>, content: impl Fn() + 'static) {
    let parent = LocalChainedPlatformTextInputInterceptor.current();
    let initial = interceptor.clone();
    let chained = remember_keyed(parent.clone(), move || {
        ChainedInterceptorRef(Some(Rc::new(ChainedPlatformTextInputInterceptor {
            interceptor: RefCell::new(initial),
            version: mutable_state_of(0),
            parent: parent.0.clone(),
        })))
    });
    if let Some(c) = &chained.0 {
        c.update_interceptor(interceptor);
    }
    KompozitionLocalProvider(&[LocalChainedPlatformTextInputInterceptor.provides(chained)], content);
}

/// Suspends until cancelled (Kotlin's `awaitCancellation`): what an interceptor that starts no input method gives.
pub async fn await_cancellation() -> Infallible {
    std::future::pending().await
}
