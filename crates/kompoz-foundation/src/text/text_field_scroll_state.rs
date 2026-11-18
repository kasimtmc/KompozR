//! A value-based field's scroll state an app holds (Compose for desktop's TextFieldScrollState.skiko.kt): what a
//! scrollbar beside the field reads ([`remember_scrollbar_adapter`](crate::scrollbar::remember_scrollbar_adapter)),
//! given to `BasicTextFieldString`/`BasicTextFieldValue`'s `scroll_state`.

use super::text_field_scroll::TextFieldScrollerPosition;
use crate::gestures::{DefaultScrollableState, LocalFuture, Orientation, ScrollBlock, ScrollableState};
use kompoz_animation::MutatePriority;
use kompoz_runtime::{kompozable, remember_keyed};
use std::rc::Rc;

/// How far a value-based field scrolled, which an app can hold to read and scroll it (Compose for desktop's
/// `TextFieldScrollState`): a [`ScrollableState`] over the field's scroller position. Horizontal for a single line
/// that doesn't wrap, vertical otherwise. Clones are the same state.
#[derive(Clone)]
pub struct TextFieldScrollState {
    pub(crate) scroller_position: TextFieldScrollerPosition,
    scrollable_state: Rc<DefaultScrollableState>,
}

impl PartialEq for TextFieldScrollState {
    fn eq(&self, other: &Self) -> bool {
        self.scroller_position == other.scroller_position
    }
}

impl TextFieldScrollState {
    /// Scrolling `orientation`'s way, `initial` px scrolled (Compose's `TextFieldScrollState(orientation, initial)`).
    /// Kotlin: `TextFieldScrollState(orientation: Orientation, initial: Int = 0)`
    pub fn new(orientation: Orientation, initial: i32) -> TextFieldScrollState {
        let scroller_position = TextFieldScrollerPosition::new(orientation, initial as f32);
        let p = scroller_position.clone();
        let scrollable_state = Rc::new(crate::gestures::ScrollableState(move |delta| {
            let prev_offset = p.offset();
            let offset = (prev_offset + delta).clamp(0.0, p.maximum().max(0.0));
            p.set_offset(offset);
            // (a floating-point rounding error kept out)
            if offset != prev_offset { offset - prev_offset } else { delta }
        }));
        TextFieldScrollState { scroller_position, scrollable_state }
    }

    /// The scroll offset, px (Compose's `offset`).
    pub fn offset(&self) -> f32 {
        self.scroller_position.offset()
    }

    pub fn set_offset(&self, offset: f32) {
        self.scroller_position.set_offset(offset);
    }

    /// The most it can scroll, px (Compose's `maxOffset`).
    pub fn max_offset(&self) -> f32 {
        self.scroller_position.maximum()
    }

    /// The field's visible size along the way it scrolls, px (Compose's `viewportSize`).
    pub fn viewport_size(&self) -> i32 {
        self.scroller_position.viewport_size()
    }
}

impl ScrollableState for TextFieldScrollState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        self.scrollable_state.scroll_block(scroll_priority, block)
    }
    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        self.scrollable_state.dispatch_raw_delta(delta)
    }
    fn is_scroll_in_progress(&self) -> bool {
        self.scrollable_state.is_scroll_in_progress()
    }
}

/// A [`TextFieldScrollState`] remembered while `orientation` stays (Compose for desktop's
/// `rememberTextFieldScrollState`): `initial` taken the first time only.
#[kompozable]
pub fn remember_text_field_scroll_state(orientation: Orientation, #[default(0)] initial: i32) -> TextFieldScrollState {
    remember_keyed(orientation, move || TextFieldScrollState::new(orientation, initial))
}

/// A vertical one, for a field of several lines or one that wraps (Compose for desktop's
/// `rememberTextFieldVerticalScrollState`).
#[kompozable]
pub fn remember_text_field_vertical_scroll_state(#[default(0)] initial: i32) -> TextFieldScrollState {
    remember_text_field_scroll_state(Orientation::Vertical, initial = initial)
}

/// A horizontal one, for a single line that doesn't wrap (Compose for desktop's
/// `rememberTextFieldHorizontalScrollState`).
#[kompozable]
pub fn remember_text_field_horizontal_scroll_state(#[default(0)] initial: i32) -> TextFieldScrollState {
    remember_text_field_scroll_state(Orientation::Horizontal, initial = initial)
}
