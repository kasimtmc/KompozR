//! The suspending twins of foundation's inline functions (what the Kotlin syntax calls where the lambda of Kotlin's
//! inline function suspends in a coroutine) do what their plain forms do, their lambdas suspending between: a change
//! list's `forEachChange`/`forEachChangeReversed` (the count read again each time), anchors' `forEach`, an interval
//! content's `withInterval`.

use crate::gestures::DraggableAnchors;
use crate::lazy::layout::{LazyLayoutInterval, LazyLayoutIntervalContent, MutableIntervalList};
use crate::text::input::{ChangeList, ChangeTracker};
use kompoz_text::TextRange;
use std::sync::Arc;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// A suspension point: pending once, then ready (as a `delay` would be).
struct Pending(bool);
impl std::future::Future for Pending {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if self.0 {
            std::task::Poll::Ready(())
        } else {
            self.0 = true;
            std::task::Poll::Pending
        }
    }
}
fn pending() -> Pending {
    Pending(false)
}

#[test]
fn change_lists_go_through_their_changes_suspending() {
    let mut changes = ChangeTracker::new();
    changes.track_change(0, 1, 2, false);
    changes.track_change(5, 5, 3, false);
    changes.track_change(9, 12, 0, true);
    let mut plain: Vec<(TextRange, TextRange)> = Vec::new();
    changes.for_each_change(|a, b| plain.push((a, b)));
    let mut twin: Vec<(TextRange, TextRange)> = Vec::new();
    block_on(changes.for_each_change_async(async |a, b| {
        pending().await;
        twin.push((a, b));
    }));
    assert_eq!(plain.len(), 3);
    assert_eq!(plain, twin);
    let mut plain = Vec::new();
    changes.for_each_change_reversed(|a, b| plain.push((a, b)));
    let mut twin = Vec::new();
    block_on(changes.for_each_change_reversed_async(async |a, b| {
        pending().await;
        twin.push((a, b));
    }));
    assert_eq!(plain, twin);
    assert_eq!(twin.first(), plain.first());
}

#[test]
fn anchors_go_through_their_keys_suspending() {
    let anchors = DraggableAnchors(|c| {
        c.at("start", 0.0);
        c.at("middle", 50.0);
        c.at("end", 100.0);
    });
    let mut plain = Vec::new();
    anchors.for_each(&mut |k, p| plain.push((k, p)));
    let mut twin = Vec::new();
    block_on(anchors.for_each_async(async |k, p| {
        pending().await;
        twin.push((k, p));
    }));
    assert_eq!(plain, [("start", 0.0), ("middle", 50.0), ("end", 100.0)]);
    assert_eq!(plain, twin);
    // (the shared form a state holds)
    let shared: Arc<dyn DraggableAnchors<&str>> = Arc::new(anchors);
    let mut keys = Vec::new();
    block_on(shared.for_each_async(async |k, _| {
        pending().await;
        keys.push(k);
    }));
    assert_eq!(keys, ["start", "middle", "end"]);
}

struct Named(&'static str);
impl LazyLayoutInterval for Named {}

struct Content(MutableIntervalList<Named>);
impl LazyLayoutIntervalContent for Content {
    type Interval = Named;
    fn intervals(&self) -> &MutableIntervalList<Named> {
        &self.0
    }
}

#[test]
fn interval_content_finds_an_items_interval_suspending() {
    let mut list = MutableIntervalList::new();
    list.add_interval(2, Named("a"));
    list.add_interval(3, Named("b"));
    let content = Content(list);
    for index in 0..5 {
        let plain = content.with_interval(index, |local, i| (local, i.0));
        let twin = block_on(content.with_interval_async(index, async |local, i| {
            pending().await;
            (local, i.0)
        }));
        assert_eq!(plain, twin);
    }
    assert_eq!(content.with_interval(3, |local, i| (local, i.0)), (1, "b"));
}
