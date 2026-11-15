//! A text as a selectable (Compose's MultiWidgetSelectionDelegate.kt).

use super::helpers::{get_line_height, get_selection_handle_coordinates, previous_char};
use super::layout::{Direction, SelectionLayoutBuilder, resolve_2d_direction};
use super::selectable::Selectable;
use super::selection::{AnchorInfo, Selection};
use crate::relocation::BringIntoViewRequester;
use kompoz_graphics::{Offset, Rect};
use kompoz_text::{AnnotatedString, TextLayoutResult, TextRange};
use kompoz_ui::LayoutCoordinates;
use kompoz_ui::layout::PinnableContainerRef;
use std::cell::RefCell;
use std::rc::Rc;

type CoordinatesCallback = Rc<dyn Fn() -> Option<LayoutCoordinates>>;
type LayoutResultCallback = Rc<dyn Fn() -> Option<TextLayoutResult>>;
type PinnableContainerCallback = Rc<dyn Fn() -> Option<PinnableContainerRef>>;

/// A text as a selectable (Compose's MultiWidgetSelectionDelegate): where it is and how it is laid out come from
/// its callbacks (its node's latest).
pub struct MultiWidgetSelectionDelegate {
    selectable_id: u64,
    coordinates_callback: CoordinatesCallback,
    layout_result_callback: LayoutResultCallback,
    pinnable_container_callback: PinnableContainerCallback,
    bring_into_view_requester: Option<BringIntoViewRequester>,
    /// The last visible offset of the last layout asked about (a layout seldom changes).
    last_visible: RefCell<Option<(TextLayoutResult, usize)>>,
}

impl MultiWidgetSelectionDelegate {
    pub fn new(
        selectable_id: u64,
        coordinates_callback: impl Fn() -> Option<LayoutCoordinates> + 'static,
        layout_result_callback: impl Fn() -> Option<TextLayoutResult> + 'static,
        pinnable_container_callback: impl Fn() -> Option<PinnableContainerRef> + 'static,
        bring_into_view_requester: Option<BringIntoViewRequester>,
    ) -> MultiWidgetSelectionDelegate {
        MultiWidgetSelectionDelegate {
            selectable_id,
            coordinates_callback: Rc::new(coordinates_callback),
            layout_result_callback: Rc::new(layout_result_callback),
            pinnable_container_callback: Rc::new(pinnable_container_callback),
            bring_into_view_requester,
            last_visible: RefCell::new(None),
        }
    }

    fn layout_result(&self) -> Option<TextLayoutResult> {
        (self.layout_result_callback)()
    }

    /// The end of the last line not cut off (Compose's `TextLayoutResult.lastVisibleOffset`): with `max_lines`
    /// or no overflow the last line, else the last whose top is above the text's height.
    fn last_visible_offset_of(&self, layout: &TextLayoutResult) -> usize {
        if let Some((last, offset)) = &*self.last_visible.borrow()
            && same_layout(last, layout)
        {
            return *offset;
        }
        let line_count = layout.line_count();
        let last_visible_line = if !layout.did_overflow_height() || layout.multi_paragraph.did_exceed_max_lines() {
            line_count.saturating_sub(1)
        } else {
            let height = layout.size.height as f32;
            let mut line = layout.get_line_for_vertical_position(height).min(line_count.saturating_sub(1)) as isize;
            // A line whose top is at or below the height isn't visible: the ones above are looked at.
            while line >= 0 && layout.get_line_top(line as usize) >= height {
                line -= 1;
            }
            line.max(0) as usize
        };
        let offset = layout.get_line_end(last_visible_line, true);
        *self.last_visible.borrow_mut() = Some((layout.clone(), offset));
        offset
    }
}

/// The same layout (Compose's instance check).
fn same_layout(a: &TextLayoutResult, b: &TextLayoutResult) -> bool {
    Rc::ptr_eq(&a.layout_input, &b.layout_input)
        && Rc::ptr_eq(&a.multi_paragraph, &b.multi_paragraph)
        && a.size == b.size
}

impl Selectable for MultiWidgetSelectionDelegate {
    fn selectable_id(&self) -> u64 {
        self.selectable_id
    }

    fn pinnable_container(&self) -> Option<PinnableContainerRef> {
        (self.pinnable_container_callback)()
    }

    fn bring_into_view_requester(&self) -> Option<BringIntoViewRequester> {
        self.bring_into_view_requester.clone()
    }

    fn append_selectable_info_to_builder(&self, builder: &mut SelectionLayoutBuilder) {
        let Some(layout_coordinates) = self.get_layout_coordinates() else { return };
        let Some(text_layout_result) = self.layout_result() else { return };
        let relative_position = builder.container_coordinates.local_position_of(&layout_coordinates, Offset::Zero);
        let local_position = builder.current_position - relative_position;
        // The previous raw offset is the previous layout's: the selectable may have moved since (scrolled while
        // dragging), and the previous position can't be mapped through its coordinates of then.
        let raw_previous_handle_offset = builder.previous_layout.as_ref().and_then(|l| {
            l.info_for_selectable(self.selectable_id).map(|info| {
                if builder.is_start_handle { info.raw_start_handle_offset } else { info.raw_end_handle_offset }
            })
        });
        let local_previous_handle_position = if builder.previous_handle_position.is_unspecified() {
            Offset::Unspecified
        } else {
            builder.previous_handle_position - relative_position
        };
        append_selectable_info(
            builder,
            &text_layout_result,
            local_position,
            raw_previous_handle_offset,
            local_previous_handle_position,
            self.selectable_id,
        );
    }

    fn get_select_all_selection(&self) -> Option<Selection> {
        let layout = self.layout_result()?;
        let text = layout.layout_input.text.text();
        let end = text.len();
        Some(Selection {
            start: AnchorInfo::new(layout.get_bidi_run_direction(0), 0, self.selectable_id),
            end: AnchorInfo::new(layout.get_bidi_run_direction(previous_char(text, end)), end, self.selectable_id),
            handles_crossed: false,
        })
    }

    fn get_handle_position(&self, selection: &Selection, is_start_handle: bool) -> Offset {
        let anchor = if is_start_handle { selection.start } else { selection.end };
        if anchor.selectable_id != self.selectable_id {
            return Offset::Unspecified;
        }
        if self.get_layout_coordinates().is_none() {
            return Offset::Unspecified;
        }
        let Some(layout) = self.layout_result() else { return Offset::Unspecified };
        let offset = anchor.offset.min(self.last_visible_offset_of(&layout));
        get_selection_handle_coordinates(&layout, offset, is_start_handle, selection.handles_crossed)
    }

    fn get_layout_coordinates(&self) -> Option<LayoutCoordinates> {
        (self.coordinates_callback)().filter(|c| c.is_attached())
    }

    fn text_layout_result(&self) -> Option<TextLayoutResult> {
        self.layout_result()
    }

    fn get_text(&self) -> AnnotatedString {
        self.layout_result().map_or_else(|| AnnotatedString::new(""), |l| l.layout_input.text.clone())
    }

    fn get_bounding_box(&self, offset: usize) -> Rect {
        let Some(layout) = self.layout_result() else { return Rect::Zero };
        let text = layout.layout_input.text.text();
        if text.is_empty() {
            return Rect::Zero;
        }
        // Within the text: its last character's start at most.
        let offset = if offset >= text.len() { previous_char(text, text.len()) } else { offset };
        layout.get_bounding_box(offset)
    }

    fn get_line_left(&self, offset: usize) -> f32 {
        let Some(layout) = self.layout_result() else { return -1.0 };
        let line = layout.get_line_for_offset(offset);
        if line >= layout.line_count() {
            return -1.0;
        }
        layout.get_line_left(line)
    }

    fn get_line_right(&self, offset: usize) -> f32 {
        let Some(layout) = self.layout_result() else { return -1.0 };
        let line = layout.get_line_for_offset(offset);
        if line >= layout.line_count() {
            return -1.0;
        }
        layout.get_line_right(line)
    }

    fn get_center_y_for_offset(&self, offset: usize) -> f32 {
        let Some(layout) = self.layout_result() else { return -1.0 };
        let line = layout.get_line_for_offset(offset);
        if line >= layout.line_count() {
            return -1.0;
        }
        let top = layout.get_line_top(line);
        let bottom = layout.get_line_bottom(line);
        (bottom - top) / 2.0 + top
    }

    fn get_range_of_line_containing(&self, offset: usize) -> TextRange {
        let Some(layout) = self.layout_result() else { return TextRange::Zero };
        let visible_text_length = self.last_visible_offset_of(&layout);
        if visible_text_length < 1 {
            return TextRange::Zero;
        }
        let text = layout.layout_input.text.text();
        let offset = if offset >= visible_text_length { previous_char(text, visible_text_length) } else { offset };
        let line = layout.get_line_for_offset(offset);
        TextRange::new(layout.get_line_start(line), layout.get_line_end(line, true))
    }

    fn get_last_visible_offset(&self) -> usize {
        self.layout_result().map_or(0, |l| self.last_visible_offset_of(&l))
    }

    fn get_line_height(&self, offset: usize) -> f32 {
        self.layout_result().map_or(0.0, |l| get_line_height(&l, offset))
    }
}

/// Adds a text to `builder` (Compose's `SelectionLayoutBuilder.appendSelectableInfo`): where each end is from it,
/// and their raw offsets in it — the moving end's at `local_position`, the other one's from the previous selection
/// (its start or end for a text before or after the one it is in). Nothing is added for a text the selection
/// doesn't reach, unless the space between texts selects.
pub fn append_selectable_info(
    builder: &mut SelectionLayoutBuilder,
    text_layout_result: &TextLayoutResult,
    local_position: Offset,
    given_raw_previous_handle_offset: Option<usize>,
    previous_handle_position: Offset,
    selectable_id: u64,
) {
    let bounds = Rect::new(0.0, 0.0, text_layout_result.size.width as f32, text_layout_result.size.height as f32);
    let current_x_direction = x_direction(local_position, bounds);
    let current_y_direction = y_direction(local_position, bounds);
    let other_direction = |anchor: Option<&AnchorInfo>| match anchor {
        Some(a) => direction_by_id(builder, a.selectable_id, selectable_id),
        None => resolve_2d_direction(current_x_direction, current_y_direction),
    };
    let previous_selection = builder.previous_selection;
    let (other, start_x, start_y, end_x, end_y);
    if builder.is_start_handle {
        other = other_direction(previous_selection.as_ref().map(|s| &s.end));
        (start_x, start_y, end_x, end_y) = (current_x_direction, current_y_direction, other, other);
    } else {
        other = other_direction(previous_selection.as_ref().map(|s| &s.start));
        (start_x, start_y, end_x, end_y) = (other, other, current_x_direction, current_y_direction);
    }
    if !builder.allow_selection_between_selectables
        && !is_selected(resolve_2d_direction(current_x_direction, current_y_direction), other)
    {
        return;
    }
    let text_length = text_layout_result.layout_input.text.text().len();
    let previous_adjusted_offset =
        |anchor: &AnchorInfo| match builder.compare_selectable_ids(anchor.selectable_id, selectable_id) {
            std::cmp::Ordering::Less => 0,
            std::cmp::Ordering::Greater => text_length,
            std::cmp::Ordering::Equal => anchor.offset,
        };
    let (raw_start_handle_offset, raw_end_handle_offset);
    if builder.is_start_handle {
        raw_start_handle_offset = offset_for_position(local_position, text_layout_result);
        raw_end_handle_offset =
            previous_selection.as_ref().map_or(raw_start_handle_offset, |s| previous_adjusted_offset(&s.end));
    } else {
        raw_end_handle_offset = offset_for_position(local_position, text_layout_result);
        raw_start_handle_offset =
            previous_selection.as_ref().map_or(raw_end_handle_offset, |s| previous_adjusted_offset(&s.start));
    }
    let raw_previous_handle_offset = match given_raw_previous_handle_offset {
        Some(o) => Some(o),
        None if previous_handle_position.is_unspecified() => None,
        None => Some(offset_for_position(previous_handle_position, text_layout_result)),
    };
    builder.append_info(
        selectable_id,
        raw_start_handle_offset,
        start_x,
        start_y,
        raw_end_handle_offset,
        end_x,
        end_y,
        raw_previous_handle_offset,
        text_layout_result.clone(),
    );
}

fn x_direction(position: Offset, bounds: Rect) -> Direction {
    if position.x < bounds.left {
        Direction::Before
    } else if position.x > bounds.right {
        Direction::After
    } else {
        Direction::On
    }
}

fn y_direction(position: Offset, bounds: Rect) -> Direction {
    if position.y < bounds.top {
        Direction::Before
    } else if position.y > bounds.bottom {
        Direction::After
    } else {
        Direction::On
    }
}

fn direction_by_id(
    builder: &SelectionLayoutBuilder,
    anchor_selectable_id: u64,
    current_selectable_id: u64,
) -> Direction {
    match builder.compare_selectable_ids(anchor_selectable_id, current_selectable_id) {
        std::cmp::Ordering::Less => Direction::Before,
        std::cmp::Ordering::Greater => Direction::After,
        std::cmp::Ordering::Equal => Direction::On,
    }
}

/// Whether a text between the two ends is selected: one end is on it, or they aren't on the same side of it.
fn is_selected(current_direction: Direction, other_direction: Direction) -> bool {
    current_direction == Direction::On || current_direction != other_direction
}

/// The offset at a position: above the text its start, below it its end.
fn offset_for_position(position: Offset, layout: &TextLayoutResult) -> usize {
    if position.y <= 0.0 {
        0
    } else if position.y >= layout.multi_paragraph.height() {
        layout.layout_input.text.text().len()
    } else {
        layout.get_offset_for_position(position)
    }
}
