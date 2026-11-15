//! What selectables subscribe to (Compose's SelectionRegistrar.kt and SelectionRegistrarImpl.kt).

use super::adjustment::SelectionAdjustment;
use super::selectable::Selectable;
use super::selection::Selection;
use kompoz_graphics::Offset;
use kompoz_runtime::{KompozitionLocal, MutableState, SavedValue, Saver, mutable_state_of};
use kompoz_ui::LayoutCoordinates;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// An id no selectable has (Compose's SelectionRegistrar.InvalidSelectableId).
pub const InvalidSelectableId: u64 = 0;

/// The part of the selection in each selectable, by its id (Compose's `subselections`).
pub type Subselections = Arc<HashMap<u64, Selection>>;

/// What selectables subscribe to and tell their gestures and changes to (Compose's SelectionRegistrar): a
/// selection container's, provided to its content through [`LocalSelectionRegistrar`].
pub trait SelectionRegistrar {
    /// The part of the selection in each selectable (read from a state: what reads it is told when it changes).
    fn subselections(&self) -> Subselections;

    /// Subscribes `selectable` to the container's selection; it is given back.
    fn subscribe(&self, selectable: Rc<dyn Selectable>) -> Rc<dyn Selectable>;

    /// Unsubscribes it.
    fn unsubscribe(&self, selectable: &dyn Selectable);

    /// An id for a new selectable (never [`InvalidSelectableId`]).
    fn next_selectable_id(&self) -> u64;

    /// The selectable moved in the window.
    fn notify_position_change(&self, selectable_id: u64);

    /// A selection gesture started at `start_position` in `layout_coordinates` (a long press, a click), adjusted
    /// by `adjustment`; `notify_selection_update` may follow, `notify_selection_update_end` always does.
    fn notify_selection_update_start(
        &self,
        layout_coordinates: &LayoutCoordinates,
        start_position: Offset,
        adjustment: SelectionAdjustment,
        is_in_touch_mode: bool,
    );

    /// A selection gesture selected all of a selectable.
    fn notify_selection_update_select_all(&self, selectable_id: u64, is_in_touch_mode: bool);

    /// An end of the selection moved from `previous_position` to `new_position` (in `layout_coordinates`):
    /// whether the move was taken — if so the caller starts its next move from here, else it keeps adding up
    /// moves from the previous position.
    fn notify_selection_update(
        &self,
        layout_coordinates: &LayoutCoordinates,
        new_position: Offset,
        previous_position: Offset,
        is_start_handle: bool,
        adjustment: SelectionAdjustment,
        is_in_touch_mode: bool,
    ) -> bool;

    /// The selection gesture ended.
    fn notify_selection_update_end(&self);

    /// The selectable's content changed (its text).
    fn notify_selectable_change(&self, selectable_id: u64);
}

/// Whether the selectable has a part of the selection (Compose's `SelectionRegistrar?.hasSelection`).
pub fn has_selection(registrar: Option<&dyn SelectionRegistrar>, selectable_id: u64) -> bool {
    registrar.is_some_and(|r| r.subselections().contains_key(&selectable_id))
}

/// A [`SelectionRegistrar`] as [`LocalSelectionRegistrar`] holds it: compared by identity.
#[derive(Clone)]
pub struct SelectionRegistrarRef(pub Rc<dyn SelectionRegistrar>);

impl PartialEq for SelectionRegistrarRef {
    fn eq(&self, other: &SelectionRegistrarRef) -> bool {
        std::ptr::eq(Rc::as_ptr(&self.0) as *const (), Rc::as_ptr(&other.0) as *const ())
    }
}

impl fmt::Debug for SelectionRegistrarRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SelectionRegistrarRef({:p})", Rc::as_ptr(&self.0) as *const ())
    }
}

/// The registrar texts subscribe to, none outside a selection container or in `DisableSelection` (Compose's
/// LocalSelectionRegistrar).
pub static LocalSelectionRegistrar: KompozitionLocal<Option<SelectionRegistrarRef>> = KompozitionLocal::new(|| None);

type StartCallback = Rc<dyn Fn(bool, &LayoutCoordinates, Offset, SelectionAdjustment)>;
type UpdateCallback = Rc<dyn Fn(bool, &LayoutCoordinates, Offset, Offset, bool, SelectionAdjustment) -> bool>;
type IdCallback = Rc<dyn Fn(u64)>;

/// A selection container's registrar (Compose's SelectionRegistrarImpl): the selectables subscribed, sorted in
/// reading order when asked, the parts of the selection, and the callbacks its selection manager set to hear the
/// gestures.
pub struct SelectionRegistrarImpl {
    /// Whether the selectables are sorted already.
    pub(crate) sorted: Cell<bool>,
    selectables: RefCell<Vec<Rc<dyn Selectable>>>,
    selectable_map: RefCell<HashMap<u64, Rc<dyn Selectable>>>,
    /// The next id (0 is no selectable's).
    increment_id: Cell<u64>,
    subselections: MutableState<Subselections>,
    pub(crate) on_position_change_callback: RefCell<Option<IdCallback>>,
    pub(crate) on_selection_update_start_callback: RefCell<Option<StartCallback>>,
    pub(crate) on_selection_update_select_all: RefCell<Option<Rc<dyn Fn(bool, u64)>>>,
    pub(crate) on_selection_update_callback: RefCell<Option<UpdateCallback>>,
    pub(crate) on_selection_update_end_callback: RefCell<Option<Rc<dyn Fn()>>>,
    pub(crate) on_selectable_change_callback: RefCell<Option<IdCallback>>,
    pub(crate) after_selectable_unsubscribe: RefCell<Option<IdCallback>>,
}

impl Default for SelectionRegistrarImpl {
    fn default() -> Self {
        SelectionRegistrarImpl::with_initial_id(1)
    }
}

impl SelectionRegistrarImpl {
    pub fn new() -> SelectionRegistrarImpl {
        SelectionRegistrarImpl::default()
    }

    fn with_initial_id(initial_increment_id: u64) -> SelectionRegistrarImpl {
        SelectionRegistrarImpl {
            sorted: Cell::new(false),
            selectables: RefCell::new(Vec::new()),
            selectable_map: RefCell::new(HashMap::new()),
            increment_id: Cell::new(initial_increment_id),
            subselections: mutable_state_of(Arc::new(HashMap::new())),
            on_position_change_callback: RefCell::new(None),
            on_selection_update_start_callback: RefCell::new(None),
            on_selection_update_select_all: RefCell::new(None),
            on_selection_update_callback: RefCell::new(None),
            on_selection_update_end_callback: RefCell::new(None),
            on_selectable_change_callback: RefCell::new(None),
            after_selectable_unsubscribe: RefCell::new(None),
        }
    }

    /// Saves its next id (Compose's SelectionRegistrarImpl.Saver): the ids of restored selectables stay unique.
    pub fn saver() -> Saver<Rc<SelectionRegistrarImpl>> {
        Saver::new(
            |r: &Rc<SelectionRegistrarImpl>| SavedValue::from(r.increment_id.get()),
            |v| v.as_int().map(|id| Rc::new(SelectionRegistrarImpl::with_initial_id(id as u64))),
        )
    }

    /// The selectables subscribed (in reading order once sorted).
    pub fn selectables(&self) -> Vec<Rc<dyn Selectable>> {
        self.selectables.borrow().clone()
    }

    /// The selectable of `selectable_id`, if subscribed.
    pub fn selectable(&self, selectable_id: u64) -> Option<Rc<dyn Selectable>> {
        self.selectable_map.borrow().get(&selectable_id).cloned()
    }

    /// Sets the parts of the selection.
    pub fn set_subselections(&self, subselections: HashMap<u64, Selection>) {
        self.subselections.set(Arc::new(subselections));
    }

    /// The parts of the selection, without reading the state.
    pub(crate) fn peek_subselections(&self) -> Subselections {
        self.subselections.peek()
    }

    /// The selectables in reading order (Compose's `sort`): top to bottom, left to right for those in a row, by
    /// where they are in `container_layout_coordinates`; sorted again only after one moved or subscribed.
    pub fn sort(&self, container_layout_coordinates: &LayoutCoordinates) -> Vec<Rc<dyn Selectable>> {
        if !self.sorted.get() {
            let mut selectables = self.selectables.borrow().clone();
            let boxes: HashMap<u64, (Offset, Offset)> = selectables
                .iter()
                .map(|s| {
                    let corners = match s.get_layout_coordinates() {
                        Some(c) => {
                            let top_left = container_layout_coordinates.local_position_of(&c, Offset::Zero);
                            let size = c.size();
                            (top_left, Offset::new(top_left.x + size.width as f32, top_left.y + size.height as f32))
                        }
                        None => (Offset::Zero, Offset::Zero),
                    };
                    (s.selectable_id(), corners)
                })
                .collect();
            let before = |a: &Rc<dyn Selectable>, b: &Rc<dyn Selectable>| {
                let (a_top_left, a_bottom_right) = boxes[&a.selectable_id()];
                let (b_top_left, b_bottom_right) = boxes[&b.selectable_id()];
                // In a row (b/439758956's heuristic): by x; else by y.
                if in_a_row(a_top_left, a_bottom_right, b_top_left, b_bottom_right) {
                    a_top_left.x < b_top_left.x
                } else {
                    a_top_left.y < b_top_left.y
                }
            };
            // A stable insertion sort: the heuristic isn't a total order (std's sorts may refuse it).
            for i in 1..selectables.len() {
                let mut j = i;
                while j > 0 && before(&selectables[j], &selectables[j - 1]) {
                    selectables.swap(j, j - 1);
                    j -= 1;
                }
            }
            *self.selectables.borrow_mut() = selectables;
            self.sorted.set(true);
        }
        self.selectables()
    }
}

impl SelectionRegistrar for SelectionRegistrarImpl {
    fn subselections(&self) -> Subselections {
        self.subselections.get()
    }

    fn subscribe(&self, selectable: Rc<dyn Selectable>) -> Rc<dyn Selectable> {
        let selectable_id = selectable.selectable_id();
        assert!(selectable_id != InvalidSelectableId, "The selectable contains an invalid id: {selectable_id}");
        assert!(
            !self.selectable_map.borrow().contains_key(&selectable_id),
            "Another selectable with the id: {selectable_id} has already subscribed."
        );
        self.selectable_map.borrow_mut().insert(selectable_id, selectable.clone());
        self.selectables.borrow_mut().push(selectable.clone());
        self.sorted.set(false);
        selectable
    }

    fn unsubscribe(&self, selectable: &dyn Selectable) {
        let selectable_id = selectable.selectable_id();
        if self.selectable_map.borrow_mut().remove(&selectable_id).is_none() {
            return;
        }
        self.selectables.borrow_mut().retain(|s| s.selectable_id() != selectable_id);
        let callback = self.after_selectable_unsubscribe.borrow().clone();
        if let Some(callback) = callback {
            callback(selectable_id);
        }
    }

    fn next_selectable_id(&self) -> u64 {
        let mut id = self.increment_id.get();
        while id == InvalidSelectableId {
            id = id.wrapping_add(1);
        }
        self.increment_id.set(id.wrapping_add(1));
        id
    }

    fn notify_position_change(&self, selectable_id: u64) {
        // Moved: sorted again when next asked.
        self.sorted.set(false);
        let callback = self.on_position_change_callback.borrow().clone();
        if let Some(callback) = callback {
            callback(selectable_id);
        }
    }

    fn notify_selection_update_start(
        &self,
        layout_coordinates: &LayoutCoordinates,
        start_position: Offset,
        adjustment: SelectionAdjustment,
        is_in_touch_mode: bool,
    ) {
        let callback = self.on_selection_update_start_callback.borrow().clone();
        if let Some(callback) = callback {
            callback(is_in_touch_mode, layout_coordinates, start_position, adjustment);
        }
    }

    fn notify_selection_update_select_all(&self, selectable_id: u64, is_in_touch_mode: bool) {
        let callback = self.on_selection_update_select_all.borrow().clone();
        if let Some(callback) = callback {
            callback(is_in_touch_mode, selectable_id);
        }
    }

    fn notify_selection_update(
        &self,
        layout_coordinates: &LayoutCoordinates,
        new_position: Offset,
        previous_position: Offset,
        is_start_handle: bool,
        adjustment: SelectionAdjustment,
        is_in_touch_mode: bool,
    ) -> bool {
        let callback = self.on_selection_update_callback.borrow().clone();
        match callback {
            Some(callback) => callback(
                is_in_touch_mode,
                layout_coordinates,
                new_position,
                previous_position,
                is_start_handle,
                adjustment,
            ),
            None => true,
        }
    }

    fn notify_selection_update_end(&self) {
        let callback = self.on_selection_update_end_callback.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    fn notify_selectable_change(&self, selectable_id: u64) {
        let callback = self.on_selectable_change_callback.borrow().clone();
        if let Some(callback) = callback {
            callback(selectable_id);
        }
    }
}

/// Whether two boxes are in a row (Compose's `inARow`): their rows overlap by at least half of either's height,
/// and their columns by less than half of each one's width.
pub fn in_a_row(
    box_a_top_left: Offset,
    box_a_bottom_right: Offset,
    box_b_top_left: Offset,
    box_b_bottom_right: Offset,
) -> bool {
    let height_a = box_a_bottom_right.y - box_a_top_left.y;
    let width_a = box_a_bottom_right.x - box_a_top_left.x;
    let height_b = box_b_bottom_right.y - box_b_top_left.y;
    let width_b = box_b_bottom_right.x - box_b_top_left.x;
    let vertical = (box_a_bottom_right.y.min(box_b_bottom_right.y) - box_a_top_left.y.max(box_b_top_left.y)).max(0.0);
    let horizontal = (box_a_bottom_right.x.min(box_b_bottom_right.x) - box_a_top_left.x.max(box_b_top_left.x)).max(0.0);
    let vertically_aligned = vertical >= height_a * 0.5 || vertical >= height_b * 0.5;
    let horizontally_distinct = horizontal < width_a * 0.5 && horizontal < width_b * 0.5;
    vertically_aligned && horizontally_distinct
}
