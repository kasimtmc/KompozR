//! A selection container's state for the app (Compose's SelectionState.kt).

use super::manager::SelectionManager;
use super::selection::{AnchorInfo, Selection};
use kompoz_runtime::{
    MutableState, Saveable, SavedValue, Saver, kompozable, list_saver, mutable_state_of, remember_saveable_with,
};
use kompoz_text::{AnnotatedString, ResolvedTextDirection, TextRange};
use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

struct Inner {
    selection: MutableState<Option<Selection>>,
    manager: RefCell<Weak<SelectionManager>>,
    selected_texts: MutableState<Vec<AnnotatedString>>,
}

/// The state of a [`SelectionContainerWithState`](super::SelectionContainerWithState) (Compose's SelectionState):
/// its selected texts (observable), and selecting all, clearing, extending by a word, selecting a range from
/// code. One container's only; remember it with [`remember_selection_state`]. Cheap to clone (the same state);
/// equal when the same.
#[derive(Clone)]
pub struct SelectionState(Rc<Inner>);

impl Default for SelectionState {
    fn default() -> Self {
        SelectionState::new()
    }
}

impl PartialEq for SelectionState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for SelectionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SelectionState").field("selection", &self.0.selection.peek()).finish_non_exhaustive()
    }
}

impl SelectionState {
    pub fn new() -> SelectionState {
        SelectionState(Rc::new(Inner {
            selection: mutable_state_of(None),
            manager: RefCell::new(Weak::new()),
            selected_texts: mutable_state_of(Vec::new()),
        }))
    }

    /// The selection (read from a state).
    pub fn selection(&self) -> Option<Selection> {
        self.0.selection.get()
    }

    pub(crate) fn set_selection(&self, selection: Option<Selection>) {
        self.0.selection.set(selection);
    }

    pub(crate) fn manager(&self) -> Option<Rc<SelectionManager>> {
        self.0.manager.borrow().upgrade()
    }

    /// Binds it to a container's manager (none: unbinds it); a state bound to another container is an error.
    pub(crate) fn set_manager(&self, manager: Option<&Rc<SelectionManager>>) {
        if let (Some(new), Some(old)) = (manager, self.manager())
            && !Rc::ptr_eq(new, &old)
        {
            panic!(
                "A SelectionState can only be bound to one SelectionContainer. Please use remember_selection_state() \
                 to create a unique state for each container."
            );
        }
        *self.0.manager.borrow_mut() = manager.map_or_else(Weak::new, Rc::downgrade);
    }

    /// The selected part of each text of the container, in their order (read from a state: what reads it is
    /// told when they change).
    pub fn selected_texts(&self) -> Vec<AnnotatedString> {
        self.0.selected_texts.get()
    }

    pub(crate) fn update_selected_texts(&self, texts: Vec<AnnotatedString>) {
        self.0.selected_texts.set(texts);
    }

    /// Selects every text of the container (Compose's `selectAll`): in a lazy list, the composed ones.
    pub fn select_all(&self) {
        if let Some(m) = self.manager() {
            m.select_all();
        }
    }

    /// Clears the selection and its handles (Compose's `clear`).
    pub fn clear(&self) {
        if let Some(m) = self.manager() {
            m.on_release();
        }
    }

    /// Extends the selection to the next word boundary at its moving edge, the way the text goes (into the next
    /// text at a text's end); with none, selects the first word (Compose's `extendSelectionByWord`). Words are
    /// ICU's (UAX #29).
    pub fn extend_selection_by_word(&self) {
        if let Some(m) = self.manager() {
            m.extend_selection_by_word();
        }
    }

    /// The whole text of each selectable text of the container, in their order; texts in `DisableSelection` aren't
    /// (Compose's `getSelectableTexts`). Not during composition: it needs the layout.
    pub fn get_selectable_texts(&self) -> Vec<AnnotatedString> {
        self.manager().map(|m| m.get_selectable_texts()).unwrap_or_default()
    }

    /// Selects `range` of the container's texts taken end to end, UTF-8 byte offsets (Compose's `select`). Not
    /// during composition: it needs the layout.
    pub fn select(&self, range: TextRange) {
        if let Some(m) = self.manager() {
            m.set_selection_range(range);
        }
    }

    /// Saves its selected texts and its selection (Compose's SelectionState.Saver).
    pub fn saver() -> Saver<SelectionState> {
        list_saver(
            |state: &SelectionState| {
                let texts = SavedValue::list(state.0.selected_texts.peek().iter().map(|t| t.save()));
                let selection = state.0.selection.peek();
                let anchor = |a: Option<AnchorInfo>| -> [SavedValue; 3] {
                    match a {
                        Some(a) => [
                            SavedValue::from(a.offset),
                            SavedValue::from(a.selectable_id),
                            SavedValue::from(direction_name(a.direction)),
                        ],
                        None => [SavedValue::Null, SavedValue::Null, SavedValue::Null],
                    }
                };
                let [start_offset, start_id, start_direction] = anchor(selection.map(|s| s.start));
                let [end_offset, end_id, end_direction] = anchor(selection.map(|s| s.end));
                vec![
                    texts,
                    start_offset,
                    start_id,
                    start_direction,
                    end_offset,
                    end_id,
                    end_direction,
                    selection.map_or(SavedValue::Null, |s| SavedValue::from(s.handles_crossed)),
                ]
            },
            |saved| {
                let state = SelectionState::new();
                let texts = saved.first()?.as_list()?.iter().filter_map(AnnotatedString::restore).collect();
                state.update_selected_texts(texts);
                if let Some(start_offset) = saved.get(1).and_then(|v| v.as_int()) {
                    let anchor = |offset: i64, id: &SavedValue, direction: &SavedValue| {
                        Some(AnchorInfo::new(direction_of(direction.as_str()?)?, offset as usize, id.as_int()? as u64))
                    };
                    let start = anchor(start_offset, saved.get(2)?, saved.get(3)?)?;
                    let end = anchor(saved.get(4)?.as_int()?, saved.get(5)?, saved.get(6)?)?;
                    let handles_crossed = saved.get(7)?.as_bool()?;
                    state.set_selection(Some(Selection::new(start, end, handles_crossed)));
                }
                Some(state)
            },
        )
    }
}

fn direction_name(direction: ResolvedTextDirection) -> &'static str {
    match direction {
        ResolvedTextDirection::Ltr => "Ltr",
        ResolvedTextDirection::Rtl => "Rtl",
    }
}

fn direction_of(name: &str) -> Option<ResolvedTextDirection> {
    match name {
        "Ltr" => Some(ResolvedTextDirection::Ltr),
        "Rtl" => Some(ResolvedTextDirection::Rtl),
        _ => None,
    }
}

/// A [`SelectionState`] remembered, saved and restored with the composition (Compose's `rememberSelectionState`).
#[kompozable]
pub fn remember_selection_state() -> SelectionState {
    remember_saveable_with(&SelectionState::saver(), SelectionState::new)
}
