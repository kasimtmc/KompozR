//! Compose for desktop's DragGesture.skiko.kt: drags of the pointer events a [`PointerMatcher`] takes —
//! `detect_drag_gestures_with_matcher` (Kotlin's `detectDragGestures(matcher, onDragStart, onDragCancel, onDragEnd,
//! onDrag)`) and `Modifier.on_drag`.

use super::AwaitPointerEventScopeGestures;
use super::drag::await_pointer_slop_or_cancellation;
use super::tap::await_press;
use crate::pointer_matcher::PointerMatcher;
use kompoz_coroutines::{Job, coroutine_scope, current_job};
use kompoz_graphics::Offset;
use kompoz_runtime::named;
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventType, PointerInputScope, PointerType,
    SuspendingPointerInputModifierNode,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::cell::{Cell, RefCell};
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::rc::Rc;

/// `detect_drag_gestures_with_matcher`'s callbacks, set by name, then awaited (Compose for desktop's
/// `detectDragGestures(matcher = …, onDragStart = …, onDragCancel = …, onDragEnd = …) { delta -> }`).
pub struct DetectMatchedDragGestures {
    scope: PointerInputScope,
    matcher: PointerMatcher,
    on_drag_start: Rc<dyn Fn(Offset)>,
    on_drag_cancel: Rc<dyn Fn()>,
    on_drag_end: Rc<dyn Fn()>,
    on_drag: Rc<dyn Fn(Offset)>,
}

impl DetectMatchedDragGestures {
    pub(crate) fn new(scope: PointerInputScope, matcher: PointerMatcher, on_drag: impl Fn(Offset) + 'static) -> Self {
        DetectMatchedDragGestures {
            scope,
            matcher,
            on_drag_start: Rc::new(|_| {}),
            on_drag_cancel: Rc::new(|| {}),
            on_drag_end: Rc::new(|| {}),
            on_drag: Rc::new(on_drag),
        }
    }

    /// The drag started past the touch slop: where the press was.
    pub fn on_drag_start(mut self, on_drag_start: impl Fn(Offset) + 'static) -> Self {
        self.on_drag_start = Rc::new(on_drag_start);
        self
    }

    /// The drag ended some other way than a release the matcher takes (another gesture consumed it, say).
    pub fn on_drag_cancel(mut self, on_drag_cancel: impl Fn() + 'static) -> Self {
        self.on_drag_cancel = Rc::new(on_drag_cancel);
        self
    }

    /// The drag ended: the matched pointer (button) let go.
    pub fn on_drag_end(mut self, on_drag_end: impl Fn() + 'static) -> Self {
        self.on_drag_end = Rc::new(on_drag_end);
        self
    }
}

/// Compose desktop's `isReleased`: a mouse's release (of a button), or every pointer gone up.
fn is_released(event: &PointerEvent) -> bool {
    (event.r#type == PointerEventType::Release && event.changes.iter().all(|c| c.r#type == PointerType::Mouse))
        || event.changes.iter().all(|c| c.changed_to_up_ignore_consumed())
}

/// Compose desktop's `awaitDragStartOnSlop`: how far past the slop the press was dragged, none if it ended first.
async fn await_drag_start_on_slop(s: &AwaitPointerEventScope, initial_down: &PointerEvent) -> Option<Offset> {
    let over_slop = Cell::new(Offset::Zero);
    let down = &initial_down.changes[0];
    loop {
        let drag = await_pointer_slop_or_cancellation(s, down.id, down.r#type, None, Offset::Zero, |change, over| {
            change.consume();
            over_slop.set(over);
        })
        .await;
        match drag {
            None => return None,
            Some(d) if d.is_consumed() => return Some(over_slop.get()),
            Some(_) => {}
        }
    }
}

impl IntoFuture for DetectMatchedDragGestures {
    type Output = ();
    type IntoFuture = Pin<Box<dyn Future<Output = ()>>>;

    fn into_future(self) -> Self::IntoFuture {
        let DetectMatchedDragGestures { scope, matcher, on_drag_start, on_drag_cancel, on_drag_end, on_drag } = self;
        let filter: Rc<dyn Fn(&PointerEvent) -> bool> = Rc::new(move |e| matcher.matches(e));
        Box::pin(async move {
            loop {
                let (scope, filter) = (scope.clone(), filter.clone());
                let (on_drag_start, on_drag_cancel, on_drag_end, on_drag) =
                    (on_drag_start.clone(), on_drag_cancel.clone(), on_drag_end.clone(), on_drag.clone());
                // Compose's two coroutines: one waiting for the drag and following it, one (from its start) cancelling
                // it at the matched release — a mouse's drag ends only when all its buttons are up otherwise.
                coroutine_scope(move |cs| async move {
                    let watcher_scope = cs.clone();
                    cs.launch(async move {
                        let drag_job = current_job();
                        let watched = scope.clone();
                        scope
                            .await_pointer_event_scope(move |s| async move {
                                let press = await_press(&s, &*filter, false).await;
                                let Some(over_slop) = await_drag_start_on_slop(&s, &press).await else { return };
                                let pointer_id = press.changes[0].id;
                                let f = filter.clone();
                                let watcher = watcher_scope.launch(watch_for_release(watched, drag_job, f));
                                on_drag_start(press.changes[0].position);
                                if over_slop != Offset::Zero {
                                    on_drag(over_slop);
                                }
                                // (Compose's `finally`: an end or a cancel, however the drag stops)
                                let _end = DragEnd { s: s.clone(), filter, on_drag_end, on_drag_cancel, watcher };
                                s.drag(pointer_id, |change| {
                                    on_drag(change.position_change());
                                    change.consume();
                                })
                                .await;
                            })
                            .await;
                    });
                })
                .await;
            }
        })
    }
}

/// Tells how a drag stopped when dropped (done, or cancelled where it waits): ended by a release the matcher takes,
/// else cancelled. The release watcher stops with it: Compose's waits for one more event first, which the next
/// press would be, missed by the next drag (a drag the in-app drag and drop took ends with no event after it).
struct DragEnd {
    s: AwaitPointerEventScope,
    filter: Rc<dyn Fn(&PointerEvent) -> bool>,
    on_drag_end: Rc<dyn Fn()>,
    on_drag_cancel: Rc<dyn Fn()>,
    watcher: Job,
}

impl Drop for DragEnd {
    fn drop(&mut self) {
        self.watcher.cancel();
        let event = self.s.current_event();
        if is_released(&event) && (self.filter)(&event) { (self.on_drag_end)() } else { (self.on_drag_cancel)() }
    }
}

/// Compose's second coroutine: cancels the drag at the release the matcher takes.
async fn watch_for_release(scope: PointerInputScope, drag_job: Job, filter: Rc<dyn Fn(&PointerEvent) -> bool>) {
    scope
        .await_pointer_event_scope(move |s| async move {
            while drag_job.is_active() {
                let event = s.await_pointer_event().await;
                if is_released(&event) && filter(&s.current_event()) {
                    drag_job.cancel();
                }
            }
        })
        .await;
}

/// Drags of the events a matcher takes (Compose for desktop's matcher-taking `detectDragGestures`).
pub trait PointerInputScopeMatchedDrags {
    /// Waits for a press `matcher` takes and a drag past the touch slop, then gives `on_drag` each move (the first
    /// one less the slop), consumed (Compose for desktop's `detectDragGestures(matcher, …, onDrag: (Offset) ->
    /// Unit)`): set the other callbacks on what this gives, then await it.
    /// Kotlin: `detectDragGestures(matcher: PointerMatcher = PointerMatcher.Primary, onDrag: (Offset) -> Unit)`
    fn detect_drag_gestures_with_matcher(
        &self,
        matcher: PointerMatcher,
        on_drag: impl Fn(Offset) + 'static,
    ) -> DetectMatchedDragGestures;
}

impl PointerInputScopeMatchedDrags for PointerInputScope {
    fn detect_drag_gestures_with_matcher(
        &self,
        matcher: PointerMatcher,
        on_drag: impl Fn(Offset) + 'static,
    ) -> DetectMatchedDragGestures {
        DetectMatchedDragGestures::new(self.clone(), matcher, on_drag)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.onDrag

/// `Modifier.on_drag` of the desktop's foundation.
#[named(trait OnDragModifiers)]
impl Modifier {
    /// Drags of the pointer events `matcher` takes, past the touch slop in any direction: `on_drag` gets each move
    /// (Compose for desktop's `Modifier.onDrag`); `on_drag_start` where the press was, `on_drag_end` at the matched
    /// release, `on_drag_cancel` otherwise. Not `enabled`: nothing.
    fn on_drag(
        self,
        #[default(true)] enabled: bool,
        #[default(PointerMatcher::Primary)] matcher: PointerMatcher,
        #[default(|_| {})] on_drag_start: impl Fn(Offset) + 'static,
        #[default(|| {})] on_drag_cancel: impl Fn() + 'static,
        #[default(|| {})] on_drag_end: impl Fn() + 'static,
        on_drag: impl Fn(Offset) + 'static,
    ) -> Modifier {
        self.on_drag_with(enabled, matcher, on_drag_start, on_drag_cancel, on_drag_end, on_drag)
    }

    /// [`on_drag`](OnDragModifiers::on_drag) with every parameter, for plain code.
    fn on_drag_with(
        self,
        enabled: bool,
        matcher: PointerMatcher,
        on_drag_start: impl Fn(Offset) + 'static,
        on_drag_cancel: impl Fn() + 'static,
        on_drag_end: impl Fn() + 'static,
        on_drag: impl Fn(Offset) + 'static,
    ) -> Modifier {
        if !enabled {
            return self;
        }
        self.then_element(OnDragElement {
            callbacks: Rc::new(OnDragCallbacks {
                matcher,
                on_drag_start: Rc::new(on_drag_start),
                on_drag_cancel: Rc::new(on_drag_cancel),
                on_drag_end: Rc::new(on_drag_end),
                on_drag: Rc::new(on_drag),
            }),
        })
    }
}

struct OnDragCallbacks {
    matcher: PointerMatcher,
    on_drag_start: Rc<dyn Fn(Offset)>,
    on_drag_cancel: Rc<dyn Fn()>,
    on_drag_end: Rc<dyn Fn()>,
    on_drag: Rc<dyn Fn(Offset)>,
}

/// (Compose compares the lambdas by identity: a new modifier's are new ones, which the node takes.)
struct OnDragElement {
    callbacks: Rc<OnDragCallbacks>,
}

impl PartialEq for OnDragElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.callbacks, &other.callbacks)
    }
}

impl ModifierNodeElement for OnDragElement {
    type Node = OnDragNode;
    fn create(&self) -> OnDragNode {
        OnDragNode { callbacks: Rc::new(RefCell::new(self.callbacks.clone())), input: None }
    }
    fn update(&self, node: &mut OnDragNode) {
        *node.callbacks.borrow_mut() = self.callbacks.clone();
    }
    fn name(&self) -> &'static str {
        "onDrag"
    }
}

/// `Modifier.on_drag`'s node (Compose's OnDragNode): a pointer input node detecting the drags with the latest
/// callbacks (its detector isn't restarted when they change).
pub struct OnDragNode {
    callbacks: Rc<RefCell<Rc<OnDragCallbacks>>>,
    input: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
}

impl ModifierNode for OnDragNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        if self.input.is_some() {
            return;
        }
        let callbacks = self.callbacks.clone();
        self.input = Some(cx.delegate(SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
            let c = callbacks.clone();
            let latest = move || c.borrow().clone();
            let (start, cancel, end, drag) = (latest.clone(), latest.clone(), latest.clone(), latest.clone());
            let matcher = PointerMatcher::new(move |e| latest().matcher.matches(e));
            async move {
                scope
                    .detect_drag_gestures_with_matcher(matcher, move |delta| (drag().on_drag)(delta))
                    .on_drag_start(move |at| (start().on_drag_start)(at))
                    .on_drag_end(move || (end().on_drag_end)())
                    .on_drag_cancel(move || (cancel().on_drag_cancel)())
                    .await
            }
        })));
    }
}
