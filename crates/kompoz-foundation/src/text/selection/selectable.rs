//! What a selection container selects in (Compose's Selectable.kt).

use super::layout::SelectionLayoutBuilder;
use super::selection::Selection;
use crate::relocation::BringIntoViewRequester;
use kompoz_graphics::{Offset, Rect};
use kompoz_text::{AnnotatedString, TextLayoutResult, TextRange};
use kompoz_ui::LayoutCoordinates;
use kompoz_ui::layout::PinnableContainerRef;

/// Something a selection container selects in (Compose's Selectable): a text subscribed to the container's
/// [`SelectionRegistrar`](super::SelectionRegistrar). Offsets are UTF-8 byte offsets into its text.
pub trait Selectable {
    /// Its id from the registrar ([`SelectionRegistrar::next_selectable_id`](super::SelectionRegistrar)), never
    /// [`InvalidSelectableId`](super::InvalidSelectableId).
    fn selectable_id(&self) -> u64;

    /// The container a lazy layout keeps it composed in while it has a selection (Compose's `pinnableContainer`).
    fn pinnable_container(&self) -> Option<PinnableContainerRef>;

    /// What brings a dragged position into view while selecting (in its own coordinates).
    fn bring_into_view_requester(&self) -> Option<BringIntoViewRequester>;

    /// Adds what it knows of the gesture's positions to `builder` (Compose's `appendSelectableInfoToBuilder`).
    fn append_selectable_info_to_builder(&self, builder: &mut SelectionLayoutBuilder);

    /// A selection of all of it, none if it can't give one (Compose's `getSelectAllSelection`).
    fn get_select_all_selection(&self) -> Option<Selection>;

    /// Where the handle of an end of `selection` points, in its coordinates; unspecified if that end isn't in it
    /// (Compose's `getHandlePosition`).
    fn get_handle_position(&self, selection: &Selection, is_start_handle: bool) -> Offset;

    /// Its layout coordinates, none before it is placed or once detached.
    fn get_layout_coordinates(&self) -> Option<LayoutCoordinates>;

    /// Its text's layout, none before it is laid out.
    fn text_layout_result(&self) -> Option<TextLayoutResult>;

    /// Its text.
    fn get_text(&self) -> AnnotatedString;

    /// The box of the character at `offset`, `Rect::Zero` for an empty one.
    fn get_bounding_box(&self, offset: usize) -> Rect;

    /// The left of the line `offset` is on (-1 if it can't tell).
    fn get_line_left(&self, offset: usize) -> f32;

    /// The right of the line `offset` is on (-1 if it can't tell).
    fn get_line_right(&self, offset: usize) -> f32;

    /// The middle of the line `offset` is on (the first's before the text, the last's past it; -1 if it can't
    /// tell).
    fn get_center_y_for_offset(&self, offset: usize) -> f32;

    /// The start and end of the line holding `offset`, of the last visible line past the visible text,
    /// `TextRange::Zero` for an empty one (Compose's `getRangeOfLineContaining`).
    fn get_range_of_line_containing(&self, offset: usize) -> TextRange;

    /// The end of its last visible line: lines cut off by `max_lines` or its height aren't (Compose's
    /// `getLastVisibleOffset`).
    fn get_last_visible_offset(&self) -> usize;

    /// The height of the line `offset` is on.
    fn get_line_height(&self, offset: usize) -> f32;
}
