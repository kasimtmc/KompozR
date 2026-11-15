//! How a gesture's positions relate to the selectables of a container (Compose's SelectionLayout.kt).
//!
//! # Slots
//!
//! A slot is either *on* a selectable (odd: the pointer is on that text) or *between* two (even: in padding, or
//! on a text that can't be selected). Slot `2i + 1` is on the selectable at index `i`; slot `2i` is just before
//! it, slot `2n` after the last one.
//!
//! ```text
//! slot value  0  1  2  3  4  5  6  7  8  9  10
//! info index     0     1     2     3     4
//! ```
//!
//! The end that isn't being dragged is always on a selectable (it is anchored somewhere), so when both slots are
//! the same they are odd.

use super::helpers::get_text_direction_for_offset;
use super::selection::{AnchorInfo, Selection};
use kompoz_graphics::Offset;
use kompoz_text::TextLayoutResult;
use kompoz_ui::LayoutCoordinates;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Whether a selection's start comes after its end (Compose's CrossStatus).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CrossStatus {
    /// The start comes after the end.
    Crossed,
    /// The start comes before the end.
    NotCrossed,
    /// The start is the end.
    Collapsed,
}

/// Where a pointer is from a selectable (Compose's Direction).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Direction {
    Before,
    On,
    After,
}

/// The direction of a position from its x and y ones (Compose's `resolve2dDirection`): its y one, unless it is on
/// the selectable's rows, then its x one.
pub fn resolve_2d_direction(x: Direction, y: Direction) -> Direction {
    match y {
        Direction::On => x,
        other => other,
    }
}

/// A slot not assigned yet (Compose's UNASSIGNED_SLOT).
pub const UNASSIGNED_SLOT: i32 = -1;

/// What a selectable is in a [`SelectionLayout`] (Compose's SelectableInfo): its slot, the raw offsets of the
/// gesture's two ends in its text, the offset of the dragged end's previous position, and its text's layout.
#[derive(Clone)]
pub struct SelectableInfo {
    pub selectable_id: u64,
    pub slot: i32,
    pub raw_start_handle_offset: usize,
    pub raw_end_handle_offset: usize,
    /// None: unknown (a new selection).
    pub raw_previous_handle_offset: Option<usize>,
    pub text_layout_result: TextLayoutResult,
}

impl SelectableInfo {
    /// Its text.
    pub fn input_text(&self) -> &str {
        self.text_layout_result.layout_input.text.text()
    }

    /// Its text's length (bytes).
    pub fn text_length(&self) -> usize {
        self.input_text().len()
    }

    /// Whether its raw offsets are crossed.
    pub fn raw_cross_status(&self) -> CrossStatus {
        match self.raw_start_handle_offset.cmp(&self.raw_end_handle_offset) {
            Ordering::Less => CrossStatus::NotCrossed,
            Ordering::Greater => CrossStatus::Crossed,
            Ordering::Equal => CrossStatus::Collapsed,
        }
    }

    /// Whether it changed from `other` in a way that changes the selection (Compose's
    /// `shouldRecomputeSelection`).
    pub fn should_recompute_selection(&self, other: &SelectableInfo) -> bool {
        self.selectable_id != other.selectable_id
            || self.raw_start_handle_offset != other.raw_start_handle_offset
            || self.raw_end_handle_offset != other.raw_end_handle_offset
    }

    /// An anchor in it at `offset` (Compose's `anchorForOffset`).
    pub fn anchor_for_offset(&self, offset: usize) -> AnchorInfo {
        AnchorInfo {
            direction: get_text_direction_for_offset(&self.text_layout_result, offset),
            offset,
            selectable_id: self.selectable_id,
        }
    }

    /// A selection within it from `start` to `end` (Compose's `makeSingleLayoutSelection`).
    pub fn make_single_layout_selection(&self, start: usize, end: usize) -> Selection {
        Selection {
            start: self.anchor_for_offset(start),
            end: self.anchor_for_offset(end),
            handles_crossed: start > end,
        }
    }
}

impl fmt::Debug for SelectableInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let layout = &self.text_layout_result;
        write!(
            f,
            "SelectionInfo(id={}, range=({}-{:?},{}-{:?}), prevOffset={:?})",
            self.selectable_id,
            self.raw_start_handle_offset,
            get_text_direction_for_offset(layout, self.raw_start_handle_offset),
            self.raw_end_handle_offset,
            get_text_direction_for_offset(layout, self.raw_end_handle_offset),
            self.raw_previous_handle_offset,
        )
    }
}

/// How a gesture's positions relate to the selectables of a container (Compose's SelectionLayout, its
/// SingleSelectionLayout and MultiSelectionLayout): the selectables it reached, in order, the slots of its start
/// and end, which end moves and the selection before.
#[derive(Clone)]
pub struct SelectionLayout {
    infos: Vec<SelectableInfo>,
    id_to_index: HashMap<u64, usize>,
    start_slot: i32,
    end_slot: i32,
    is_start_handle: bool,
    previous_selection: Option<Selection>,
}

impl SelectionLayout {
    fn new(
        infos: Vec<SelectableInfo>,
        start_slot: i32,
        end_slot: i32,
        is_start_handle: bool,
        previous_selection: Option<Selection>,
    ) -> SelectionLayout {
        let id_to_index = infos.iter().enumerate().map(|(i, info)| (info.selectable_id, i)).collect();
        SelectionLayout { infos, id_to_index, start_slot, end_slot, is_start_handle, previous_selection }
    }

    /// One selectable only (Compose's SingleSelectionLayout).
    fn is_single(&self) -> bool {
        self.infos.len() == 1
    }

    /// How many selectables it has.
    pub fn size(&self) -> usize {
        self.infos.len()
    }
    pub fn start_slot(&self) -> i32 {
        self.start_slot
    }
    pub fn end_slot(&self) -> i32 {
        self.end_slot
    }
    /// Whether the start moves (else the end).
    pub fn is_start_handle(&self) -> bool {
        self.is_start_handle
    }
    /// The selection it changes.
    pub fn previous_selection(&self) -> Option<&Selection> {
        self.previous_selection.as_ref()
    }

    /// Whether its start comes after its end, by their slots and, on one selectable, its offsets.
    pub fn cross_status(&self) -> CrossStatus {
        match self.start_slot.cmp(&self.end_slot) {
            Ordering::Less => CrossStatus::NotCrossed,
            Ordering::Greater => CrossStatus::Crossed,
            // Equal slots are odd (one end is anchored on a selectable): half of it is that selectable's index.
            Ordering::Equal if self.is_single() => self.infos[0].raw_cross_status(),
            Ordering::Equal => self.infos[(self.start_slot / 2) as usize].raw_cross_status(),
        }
    }

    fn slot_to_index(slot: i32, is_minimum_slot: bool) -> usize {
        let adjustment = if is_minimum_slot { 0 } else { 1 };
        ((slot - adjustment) / 2).max(0) as usize
    }

    fn start_or_end_slot_to_index(&self, slot: i32, is_start_slot: bool) -> usize {
        let is_minimum_slot = match self.cross_status() {
            CrossStatus::Collapsed => true,
            CrossStatus::NotCrossed => is_start_slot,
            CrossStatus::Crossed => !is_start_slot,
        };
        Self::slot_to_index(slot, is_minimum_slot).min(self.infos.len() - 1)
    }

    /// The selectable the start is on.
    pub fn start_info(&self) -> &SelectableInfo {
        if self.is_single() {
            return &self.infos[0];
        }
        &self.infos[self.start_or_end_slot_to_index(self.start_slot, true)]
    }

    /// The selectable the end is on.
    pub fn end_info(&self) -> &SelectableInfo {
        if self.is_single() {
            return &self.infos[0];
        }
        &self.infos[self.start_or_end_slot_to_index(self.end_slot, false)]
    }

    /// The selectable the moving end is on.
    pub fn current_info(&self) -> &SelectableInfo {
        if self.is_start_handle { self.start_info() } else { self.end_info() }
    }

    /// The first selectable of the selection.
    pub fn first_info(&self) -> &SelectableInfo {
        if self.cross_status() == CrossStatus::Crossed { self.end_info() } else { self.start_info() }
    }

    /// The last selectable of the selection.
    pub fn last_info(&self) -> &SelectableInfo {
        if self.cross_status() == CrossStatus::Crossed { self.start_info() } else { self.end_info() }
    }

    /// Runs `block` on each selectable between the first and the last (Compose's `forEachMiddleInfo`).
    pub fn for_each_middle_info(&self, mut block: impl FnMut(&SelectableInfo)) {
        if self.is_single() {
            return;
        }
        let min = self.id_to_index[&self.first_info().selectable_id];
        let max = self.id_to_index[&self.last_info().selectable_id];
        for info in self.infos.iter().take(max).skip(min + 1) {
            block(info);
        }
    }

    /// The selectable of `selectable_id`, if it has it.
    pub fn info_for_selectable(&self, selectable_id: u64) -> Option<&SelectableInfo> {
        self.id_to_index.get(&selectable_id).map(|&i| &self.infos[i])
    }

    /// Whether it changed from `other` in a way that changes the selection (Compose's
    /// `shouldRecomputeSelection`).
    pub fn should_recompute_selection(&self, other: Option<&SelectionLayout>) -> bool {
        let Some(other) = other else { return true };
        self.previous_selection.is_none()
            || self.is_single() != other.is_single()
            || self.is_start_handle != other.is_start_handle
            || self.start_slot != other.start_slot
            || self.end_slot != other.end_slot
            || self.infos.len() != other.infos.len()
            || self.infos.iter().zip(&other.infos).any(|(a, b)| a.should_recompute_selection(b))
    }

    /// `selection` split into the part of it in each selectable (Compose's `createSubSelections`).
    pub fn create_sub_selections(&self, selection: &Selection) -> HashMap<u64, Selection> {
        if self.is_single() {
            let info = &self.infos[0];
            let mut s = *selection;
            // Uncrossed if its offsets aren't.
            if (!s.handles_crossed && s.start.offset > s.end.offset)
                || (s.handles_crossed && s.start.offset <= s.end.offset)
            {
                s.handles_crossed = !s.handles_crossed;
            }
            return HashMap::from([(info.selectable_id, s)]);
        }
        let crossed = selection.handles_crossed;
        create_sub_selections(
            selection,
            &self.infos,
            |info| info.selectable_id,
            |info, _| {
                Some(if crossed {
                    info.make_single_layout_selection(info.text_length(), 0)
                } else {
                    info.make_single_layout_selection(0, info.text_length())
                })
            },
            |info, is_start, offset, _| {
                Some(match (is_start, crossed) {
                    (true, true) => info.make_single_layout_selection(offset, 0),
                    (true, false) => info.make_single_layout_selection(offset, info.text_length()),
                    (false, true) => info.make_single_layout_selection(info.text_length(), offset),
                    (false, false) => info.make_single_layout_selection(0, offset),
                })
            },
        )
    }
}

impl fmt::Debug for SelectionLayout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_single() {
            write!(
                f,
                "SingleSelectionLayout(isStartHandle={}, crossed={:?}, info=\n\t{:?})",
                self.is_start_handle,
                self.cross_status(),
                self.infos[0]
            )
        } else {
            write!(
                f,
                "MultiSelectionLayout(isStartHandle={}, startPosition={}, endPosition={}, crossed={:?}, infos={:?})",
                self.is_start_handle,
                (self.start_slot + 1) as f32 / 2.0,
                (self.end_slot + 1) as f32 / 2.0,
                self.cross_status(),
                self.infos
            )
        }
    }
}

/// The layout of a text field's selection: its one text (Compose's `getTextFieldSelectionLayout`).
/// `raw_previous_handle_offset` is the moving end's offset before (none: unknown); `is_start_of_selection` says
/// there is no selection before (else it is `previous_selection_range`).
pub fn get_text_field_selection_layout(
    layout_result: &TextLayoutResult,
    raw_start_handle_offset: usize,
    raw_end_handle_offset: usize,
    raw_previous_handle_offset: Option<usize>,
    previous_selection_range: kompoz_text::TextRange,
    is_start_of_selection: bool,
    is_start_handle: bool,
) -> SelectionLayout {
    const DEFAULT_SLOT: i32 = 1;
    const DEFAULT_SELECTABLE_ID: u64 = 1;
    let previous_selection = (!is_start_of_selection).then(|| Selection {
        start: AnchorInfo {
            direction: get_text_direction_for_offset(layout_result, previous_selection_range.start),
            offset: previous_selection_range.start,
            selectable_id: DEFAULT_SELECTABLE_ID,
        },
        end: AnchorInfo {
            direction: get_text_direction_for_offset(layout_result, previous_selection_range.end),
            offset: previous_selection_range.end,
            selectable_id: DEFAULT_SELECTABLE_ID,
        },
        handles_crossed: previous_selection_range.reversed(),
    });
    let info = SelectableInfo {
        selectable_id: DEFAULT_SELECTABLE_ID,
        slot: DEFAULT_SLOT,
        raw_start_handle_offset,
        raw_end_handle_offset,
        raw_previous_handle_offset,
        text_layout_result: layout_result.clone(),
    };
    SelectionLayout::new(vec![info], DEFAULT_SLOT, DEFAULT_SLOT, is_start_handle, previous_selection)
}

/// Whether `selection` selects nothing (Compose's `isCollapsed`): in one selectable, its offsets are the same;
/// across several, it runs from the first one's end to the last one's start over empty ones only.
pub fn is_collapsed(selection: Option<&Selection>, layout: Option<&SelectionLayout>) -> bool {
    let (Some(selection), Some(layout)) = (selection, layout) else { return true };
    if selection.start.selectable_id == selection.end.selectable_id {
        return selection.start.offset == selection.end.offset;
    }
    let max_anchor = if selection.handles_crossed { selection.start } else { selection.end };
    if max_anchor.offset != 0 {
        return false;
    }
    let min_anchor = if selection.handles_crossed { selection.end } else { selection.start };
    if layout.first_info().text_length() != min_anchor.offset {
        return false;
    }
    let mut all_texts_empty = true;
    layout.for_each_middle_info(|info| {
        if !info.input_text().is_empty() {
            all_texts_empty = false;
        }
    });
    all_texts_empty
}

/// `selection` split into the part of it in each of `sorted_items` it takes in (Compose's `createSubSelections`):
/// the whole of those between its ends (`get_select_all`), from or to its offset in the ones its ends are in
/// (`create_boundary_selection`, told whether it is the start, the offset and whether the ends are crossed).
pub fn create_sub_selections<T>(
    selection: &Selection,
    sorted_items: &[T],
    get_id: impl Fn(&T) -> u64,
    get_select_all: impl Fn(&T, bool) -> Option<Selection>,
    create_boundary_selection: impl Fn(&T, bool, usize, bool) -> Option<Selection>,
) -> HashMap<u64, Selection> {
    let start_id = selection.start.selectable_id;
    let end_id = selection.end.selectable_id;
    if start_id == end_id {
        return HashMap::from([(start_id, *selection)]);
    }
    let start_index = sorted_items.iter().position(|it| get_id(it) == start_id);
    let end_index = sorted_items.iter().position(|it| get_id(it) == end_id);
    let (Some(start_index), Some(end_index)) = (start_index, end_index) else { return HashMap::new() };
    let first_index = start_index.min(end_index);
    let last_index = start_index.max(end_index);
    let handles_crossed = start_index > end_index;
    let mut result = HashMap::new();
    for item in &sorted_items[first_index..=last_index] {
        let id = get_id(item);
        let sub_selection = if id == start_id {
            create_boundary_selection(item, true, selection.start.offset, handles_crossed)
        } else if id == end_id {
            create_boundary_selection(item, false, selection.end.offset, handles_crossed)
        } else {
            get_select_all(item, handles_crossed)
        };
        if let Some(s) = sub_selection {
            result.insert(id, s);
        }
    }
    result
}

/// Builds a [`SelectionLayout`] from the selectables of a container in their order, each adding itself
/// ([`Selectable::append_selectable_info_to_builder`](super::Selectable)) (Compose's SelectionLayoutBuilder).
/// `previous_handle_position` is where the moving end was (unspecified for a new selection), `previous_layout` the
/// layout made then (its raw offsets taken over: selectables may have moved since), `order` the index of each
/// selectable by its id, `allow_selection_between_selectables` whether the empty space between them selects (a
/// mouse in a container).
pub struct SelectionLayoutBuilder {
    pub current_position: Offset,
    pub previous_handle_position: Offset,
    pub container_coordinates: LayoutCoordinates,
    pub is_start_handle: bool,
    pub previous_selection: Option<Selection>,
    pub previous_layout: Option<Rc<SelectionLayout>>,
    order: Rc<HashMap<u64, usize>>,
    pub allow_selection_between_selectables: bool,
    infos: Vec<SelectableInfo>,
    start_slot: i32,
    end_slot: i32,
    current_slot: i32,
}

impl SelectionLayoutBuilder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        current_position: Offset,
        previous_handle_position: Offset,
        container_coordinates: LayoutCoordinates,
        is_start_handle: bool,
        previous_selection: Option<Selection>,
        previous_layout: Option<Rc<SelectionLayout>>,
        order: Rc<HashMap<u64, usize>>,
        allow_selection_between_selectables: bool,
    ) -> SelectionLayoutBuilder {
        SelectionLayoutBuilder {
            current_position,
            previous_handle_position,
            container_coordinates,
            is_start_handle,
            previous_selection,
            previous_layout,
            order,
            allow_selection_between_selectables,
            infos: Vec::new(),
            start_slot: UNASSIGNED_SLOT,
            end_slot: UNASSIGNED_SLOT,
            current_slot: UNASSIGNED_SLOT,
        }
    }

    /// How two selectables are ordered, by their ids (Compose's `selectableIdOrderingComparator`); one the
    /// container doesn't have comes last.
    pub fn compare_selectable_ids(&self, a: u64, b: u64) -> Ordering {
        let index = |id: u64| self.order.get(&id).copied().unwrap_or(usize::MAX);
        index(a).cmp(&index(b))
    }

    /// The layout, none if no selectable was added (Compose's `build`).
    pub fn build(self) -> Option<SelectionLayout> {
        if self.infos.is_empty() {
            return None;
        }
        let last_slot = self.current_slot + 1;
        let start_slot = if self.start_slot == UNASSIGNED_SLOT { last_slot } else { self.start_slot };
        let end_slot = if self.end_slot == UNASSIGNED_SLOT { last_slot } else { self.end_slot };
        Some(SelectionLayout::new(self.infos, start_slot, end_slot, self.is_start_handle, self.previous_selection))
    }

    /// Adds a selectable (Compose's `appendInfo`): its raw offsets and where each end is from it.
    #[allow(clippy::too_many_arguments)]
    pub fn append_info(
        &mut self,
        selectable_id: u64,
        raw_start_handle_offset: usize,
        start_x_handle_direction: Direction,
        start_y_handle_direction: Direction,
        raw_end_handle_offset: usize,
        end_x_handle_direction: Direction,
        end_y_handle_direction: Direction,
        raw_previous_handle_offset: Option<usize>,
        text_layout_result: TextLayoutResult,
    ) -> &SelectableInfo {
        // The current slot is the last info's when the layout is built: moved on before adding it.
        self.current_slot += 2;
        let info = SelectableInfo {
            selectable_id,
            slot: self.current_slot,
            raw_start_handle_offset,
            raw_end_handle_offset,
            raw_previous_handle_offset,
            text_layout_result,
        };
        self.start_slot = self.update_slot(self.start_slot, start_x_handle_direction, start_y_handle_direction);
        self.end_slot = self.update_slot(self.end_slot, end_x_handle_direction, end_y_handle_direction);
        self.infos.push(info);
        self.infos.last().expect("just added")
    }

    /// The slot of an end, from where it is from this selectable (Compose's `updateSlot`): once found it stays;
    /// before this selectable it is the slot just before it, on it its slot.
    fn update_slot(&self, slot: i32, x: Direction, y: Direction) -> i32 {
        if slot != UNASSIGNED_SLOT {
            return slot;
        }
        match resolve_2d_direction(x, y) {
            Direction::Before => self.current_slot - 1,
            Direction::On => self.current_slot,
            Direction::After => slot,
        }
    }
}
