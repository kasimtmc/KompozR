//! What turns gestures into a selection container's selection (Compose's SelectionManager.kt).

use super::adjustment::SelectionAdjustment;
use super::gestures::{MouseSelectionObserver, await_selection_gestures, update_selection_touch_mode};
use super::handles::HandleHeight;
use super::helpers::{get_text_direction_for_offset, is_position_inside_selection, next_char, previous_char};
use super::layout::{SelectionLayout, SelectionLayoutBuilder, create_sub_selections};
use super::mode::contains_inclusive;
use super::registrar::{SelectionRegistrar, SelectionRegistrarImpl};
use super::selectable::Selectable;
use super::selection::{AnchorInfo, Selection, merge};
use crate::gestures::{
    AwaitPointerEventScopeGestures, PointerInputScopeGestures, await_all_pointers_up_with_slop_detection,
};
use crate::text::context_menu::{TextContextMenuComponent, TextContextMenuData, TextContextMenuItems};
use crate::text::long_press_text_drag_observer::{Handle, TextDragObserver};
use crate::text::modifiers::DefaultMouseSelectionObserver;
use kompoz_coroutines::{CoroutineScope, CoroutineStart};
use kompoz_graphics::{IntSize, Offset, Rect, Size};
use kompoz_runtime::{MutableState, mutable_state_of, mutable_state_of_with_policy, named_args, never_equal_policy};
use kompoz_text::{AnnotatedString, AnnotatedStringBuilder, TextRange};
use kompoz_ui::focus::{FocusRequester, FocusState};
use kompoz_ui::input::{Key, KeyEvent, KeyEventType, KeyModifiers, PointerEventPass, PointerInputScope};
use kompoz_ui::layout::PinnedHandle;
use kompoz_ui::platform::{HapticFeedback, HapticFeedbackType, TextToolbar, TextToolbarStatus};
use kompoz_ui::{LayoutCoordinates, Modifier};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

type SelectionCallback = Rc<dyn Fn(Option<Selection>)>;
type CopyHandler = Rc<dyn Fn(AnnotatedString)>;

/// The bridge between gestures and a selection container's texts (Compose's SelectionManager): it hears the
/// registrar's gestures, works out the selection through [`SelectionLayout`]s and adjustments, keeps the handles'
/// positions, shows the toolbar, copies, selects all.
///
/// A controlled component: it tells `on_selection_change` the selection it comes to, which sets
/// [`set_selection`](Self::set_selection) (at once, so gestures between compositions see it).
pub struct SelectionManager {
    this: Weak<SelectionManager>,
    registrar: Rc<SelectionRegistrarImpl>,
    selection: MutableState<Option<Selection>>,
    is_in_touch_mode: MutableState<bool>,
    on_selection_change: RefCell<SelectionCallback>,
    haptic_feedback: RefCell<Option<HapticFeedback>>,
    on_copy_handler: RefCell<Option<CopyHandler>>,
    text_toolbar: RefCell<Option<Rc<dyn TextToolbar>>>,
    focus_requester: FocusRequester,
    has_focus: MutableState<bool>,
    previous_position: Cell<Option<Offset>>,
    container_layout_coordinates: RefCell<Option<LayoutCoordinates>>,
    position_change_state: MutableState<()>,
    start_handle_position: MutableState<Option<Offset>>,
    end_handle_position: MutableState<Option<Offset>>,
    dragging_handle: MutableState<Option<Handle>>,
    current_drag_position: MutableState<Option<Offset>>,
    previous_selection_layout: RefCell<Option<Rc<SelectionLayout>>>,
    coroutine_scope: RefCell<Option<CoroutineScope>>,
    pinned_handle_by_selectable_id: RefCell<HashMap<u64, PinnedHandle>>,
    show_toolbar: Cell<bool>,
    mouse_selection_observer: RefCell<Option<Rc<dyn MouseSelectionObserver>>>,
}

impl SelectionManager {
    /// A manager of `registrar`'s selection, its callbacks set on the registrar.
    pub fn new(registrar: Rc<SelectionRegistrarImpl>) -> Rc<SelectionManager> {
        let manager = Rc::new_cyclic(|this: &Weak<SelectionManager>| {
            let setter = this.clone();
            SelectionManager {
                this: this.clone(),
                registrar: registrar.clone(),
                selection: mutable_state_of(None),
                is_in_touch_mode: mutable_state_of(true),
                on_selection_change: RefCell::new(Rc::new(move |s| {
                    if let Some(m) = setter.upgrade() {
                        m.set_selection(s);
                    }
                })),
                haptic_feedback: RefCell::new(None),
                on_copy_handler: RefCell::new(None),
                text_toolbar: RefCell::new(None),
                focus_requester: FocusRequester::new(),
                has_focus: mutable_state_of(false),
                previous_position: Cell::new(None),
                container_layout_coordinates: RefCell::new(None),
                position_change_state: mutable_state_of_with_policy((), never_equal_policy()),
                start_handle_position: mutable_state_of(None),
                end_handle_position: mutable_state_of(None),
                dragging_handle: mutable_state_of(None),
                current_drag_position: mutable_state_of(None),
                previous_selection_layout: RefCell::new(None),
                coroutine_scope: RefCell::new(None),
                pinned_handle_by_selectable_id: RefCell::new(HashMap::new()),
                show_toolbar: Cell::new(false),
                mouse_selection_observer: RefCell::new(None),
            }
        });
        manager.set_registrar_callbacks();
        manager
    }

    fn set_registrar_callbacks(&self) {
        let r = &self.registrar;
        let this = self.this.clone();
        *r.on_position_change_callback.borrow_mut() = Some(Rc::new(move |selectable_id| {
            let Some(m) = this.upgrade() else { return };
            if m.registrar.peek_subselections().contains_key(&selectable_id) {
                m.position_change_state.set(());
                m.update_handle_offsets();
                m.update_selection_toolbar();
            }
        }));
        let this = self.this.clone();
        *r.on_selection_update_start_callback.borrow_mut() =
            Some(Rc::new(move |is_in_touch_mode, layout_coordinates, raw_position, adjustment| {
                let Some(m) = this.upgrade() else { return };
                let size = layout_coordinates.size();
                let text_rect = Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
                let position = if contains_inclusive(text_rect, raw_position) {
                    raw_position
                } else {
                    Offset::new(
                        raw_position.x.clamp(text_rect.left, text_rect.right),
                        raw_position.y.clamp(text_rect.top, text_rect.bottom),
                    )
                };
                let position_in_container = m.convert_to_container_coordinates(layout_coordinates, position);
                if position_in_container.is_specified() {
                    m.set_in_touch_mode(is_in_touch_mode);
                    m.start_selection(position_in_container, false, adjustment);
                    m.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
                    m.set_show_toolbar(false);
                }
            }));
        let this = self.this.clone();
        *r.on_selection_update_select_all.borrow_mut() = Some(Rc::new(move |is_in_touch_mode, selectable_id| {
            let Some(m) = this.upgrade() else { return };
            let previous = m.selection();
            if let Some((new_selection, new_subselections)) = m.select_all_in_selectable(selectable_id, previous)
                && new_selection != previous
            {
                m.registrar.set_subselections(new_subselections);
                m.call_on_selection_change(new_selection);
            }
            m.set_in_touch_mode(is_in_touch_mode);
            m.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
            m.set_show_toolbar(false);
        }));
        let this = self.this.clone();
        *r.on_selection_update_callback.borrow_mut() = Some(Rc::new(
            move |is_in_touch_mode,
                  layout_coordinates,
                  new_position,
                  previous_position,
                  is_start_handle,
                  adjustment| {
                let Some(m) = this.upgrade() else { return false };
                let new_position_in_container = m.convert_to_container_coordinates(layout_coordinates, new_position);
                let previous_position_in_container =
                    m.convert_to_container_coordinates(layout_coordinates, previous_position);
                m.set_in_touch_mode(is_in_touch_mode);
                m.update_selection(
                    new_position_in_container,
                    previous_position_in_container,
                    is_start_handle,
                    adjustment,
                )
            },
        ));
        let this = self.this.clone();
        *r.on_selection_update_end_callback.borrow_mut() = Some(Rc::new(move || {
            let Some(m) = this.upgrade() else { return };
            m.set_show_toolbar(true);
            // Set by update_selection while dragging: cleared after the gesture.
            m.dragging_handle.set(None);
            m.current_drag_position.set(None);
            *m.previous_selection_layout.borrow_mut() = None;
        }));
        let this = self.this.clone();
        *r.on_selectable_change_callback.borrow_mut() = Some(Rc::new(move |selectable_key| {
            let Some(m) = this.upgrade() else { return };
            if m.registrar.peek_subselections().contains_key(&selectable_key) {
                // The text of a selected selectable changed: no selection any more.
                m.on_release();
                m.set_selection(None);
            }
        }));
        let this = self.this.clone();
        *r.after_selectable_unsubscribe.borrow_mut() = Some(Rc::new(move |selectable_id| {
            let Some(m) = this.upgrade() else { return };
            let selection = m.selection.peek();
            // The selectable of a handle went: the handle is hidden.
            if selection.is_some_and(|s| s.start.selectable_id == selectable_id) {
                m.start_handle_position.set(None);
            }
            if selection.is_some_and(|s| s.end.selectable_id == selectable_id) {
                m.end_handle_position.set(None);
            }
            if m.registrar.peek_subselections().contains_key(&selectable_id) {
                // The selection may be empty now, which hides the toolbar.
                m.update_selection_toolbar();
            }
            let pinned = m.pinned_handle_by_selectable_id.borrow_mut().remove(&selectable_id);
            if let Some(pinned) = pinned {
                pinned.release();
            }
        }));
    }

    // ---- What its container sets ---------------------------------------------------------------------------

    /// The registrar its selectables subscribe to.
    pub fn registrar(&self) -> &Rc<SelectionRegistrarImpl> {
        &self.registrar
    }

    /// The selection (read from a state).
    pub fn selection(&self) -> Option<Selection> {
        self.selection.get()
    }

    /// Sets the selection (Compose's `selection` setter): the handles moved to it, the selectables with a part of
    /// it pinned.
    pub fn set_selection(&self, value: Option<Selection>) {
        if self.selection.peek() == value {
            return;
        }
        self.selection.set(value);
        if value.is_some() {
            self.update_handle_offsets();
        }
        // (the subselections are set by what changes the selection)
        self.update_pinned_selectables();
    }

    /// The selection its container is given (a controlled container's app, a state): one other than its own
    /// (the app changed it) sets the parts of the selection in each selectable too — none clears them (Compose's
    /// manager leaves them to what changed the selection, which is itself there).
    pub fn sync_selection(&self, value: Option<Selection>) {
        if self.selection.peek() == value {
            return;
        }
        self.set_selection(value);
        match value {
            Some(selection) => self.update_sub_selections(&selection),
            None => {
                self.registrar.set_subselections(HashMap::new());
                self.set_show_toolbar(false);
            }
        }
    }

    /// Whether the last pointer was a finger (read from a state).
    pub fn is_in_touch_mode(&self) -> bool {
        self.is_in_touch_mode.get()
    }

    pub fn set_in_touch_mode(&self, value: bool) {
        if self.is_in_touch_mode.peek() != value {
            self.is_in_touch_mode.set(value);
            self.update_selection_toolbar();
        }
    }

    /// What it tells the selection it comes to; that sets the selection at once, then calls
    /// `on_selection_change` (Compose's `onSelectionChange` setter).
    pub fn set_on_selection_change(&self, on_selection_change: impl Fn(Option<Selection>) + 'static) {
        let this = self.this.clone();
        *self.on_selection_change.borrow_mut() = Rc::new(move |s| {
            if let Some(m) = this.upgrade() {
                m.set_selection(s);
            }
            on_selection_change(s);
        });
    }

    fn call_on_selection_change(&self, selection: Option<Selection>) {
        let callback = self.on_selection_change.borrow().clone();
        callback(selection);
    }

    pub fn set_haptic_feedback(&self, haptic_feedback: Option<HapticFeedback>) {
        *self.haptic_feedback.borrow_mut() = haptic_feedback;
    }

    fn perform_haptic_feedback(&self, kind: HapticFeedbackType) {
        let haptic = self.haptic_feedback.borrow().clone();
        if let Some(h) = haptic {
            h.perform_haptic_feedback(kind);
        }
    }

    /// What copies the selected text (none: it can't be copied).
    pub fn set_on_copy_handler(&self, on_copy_handler: Option<Rc<dyn Fn(AnnotatedString)>>) {
        *self.on_copy_handler.borrow_mut() = on_copy_handler;
    }

    /// The toolbar it shows over the selection.
    pub fn set_text_toolbar(&self, text_toolbar: Option<Rc<dyn TextToolbar>>) {
        *self.text_toolbar.borrow_mut() = text_toolbar;
    }

    /// The scope it brings dragged positions into view in.
    pub fn set_coroutine_scope(&self, scope: Option<CoroutineScope>) {
        *self.coroutine_scope.borrow_mut() = scope;
    }

    /// What its container's focus is requested through.
    pub fn focus_requester(&self) -> &FocusRequester {
        &self.focus_requester
    }

    /// Whether something in its container has the focus (read from a state).
    pub fn has_focus(&self) -> bool {
        self.has_focus.get()
    }

    pub fn set_has_focus(&self, has_focus: bool) {
        self.has_focus.set(has_focus);
    }

    /// Where the start handle points, in the container (read from a state): none while it isn't shown.
    pub fn start_handle_position(&self) -> Option<Offset> {
        self.start_handle_position.get()
    }

    /// Where the end handle points, in the container (read from a state).
    pub fn end_handle_position(&self) -> Option<Offset> {
        self.end_handle_position.get()
    }

    /// The handle being dragged, none while none is (read from a state).
    pub fn dragging_handle(&self) -> Option<Handle> {
        self.dragging_handle.get()
    }

    /// While a handle is dragged, where the drag is in the container, not snapped to the text (read from a
    /// state).
    pub fn current_drag_position(&self) -> Option<Offset> {
        self.current_drag_position.get()
    }

    /// The container's layout coordinates.
    pub fn container_layout_coordinates(&self) -> Option<LayoutCoordinates> {
        self.container_layout_coordinates.borrow().clone()
    }

    /// The container laid out (Compose's `containerLayoutCoordinates` setter): a restored selection's parts worked
    /// out once it is, the handles and toolbar moved when it moved in the window.
    pub fn set_container_layout_coordinates(&self, value: Option<LayoutCoordinates>) {
        *self.container_layout_coordinates.borrow_mut() = value.clone();
        if let Some(c) = &value
            && c.is_attached()
            && let Some(current) = self.selection.peek()
            && self.registrar.peek_subselections().is_empty()
        {
            self.update_sub_selections(&current);
            self.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
        }
        if self.has_focus.peek() && self.selection.peek().is_some() {
            let position_in_window = value.as_ref().map(|c| c.position_in_window());
            if self.previous_position.get() != position_in_window {
                self.previous_position.set(position_in_window);
                self.update_handle_offsets();
                self.update_selection_toolbar();
            }
        }
    }

    /// The height of the line the start is on.
    pub fn start_handle_line_height(&self) -> f32 {
        let Some(selection) = self.selection() else { return 0.0 };
        self.get_anchor_selectable(&selection.start).map_or(0.0, |s| s.get_line_height(selection.start.offset))
    }

    /// The height of the line the end is on.
    pub fn end_handle_line_height(&self) -> f32 {
        let Some(selection) = self.selection() else { return 0.0 };
        self.get_anchor_selectable(&selection.end).map_or(0.0, |s| s.get_line_height(selection.end.offset))
    }

    /// Whether the magnifier shows: a handle dragged by a finger over a selection (Compose's
    /// `shouldShowMagnifier`; read from states).
    pub fn should_show_magnifier(&self) -> bool {
        self.dragging_handle.get().is_some() && self.is_in_touch_mode.get() && !self.is_trivially_collapsed_selection()
    }

    /// Whether the toolbar should show now (after a selection gesture, not during one, not without a selection).
    pub fn show_toolbar(&self) -> bool {
        self.show_toolbar.get()
    }

    pub fn set_show_toolbar(&self, value: bool) {
        self.show_toolbar.set(value);
        self.update_selection_toolbar();
    }

    // ---- The container's modifier --------------------------------------------------------------------------

    /// The modifier of its container (Compose's `modifier`): a press that ends without a drag clears the
    /// selection, the container's coordinates and focus kept, keys (copy, select all, Escape), the touch mode,
    /// the mouse's selection between texts, copy and select semantics.
    pub fn modifier(&self) -> Modifier {
        let on_clear = self.this.clone();
        let positioned = self.this.clone();
        let focus = self.this.clone();
        let touch = self.this.clone();
        let keys = self.this.clone();
        let copy_action = self.this.clone();
        let select_action = self.this.clone();
        let mouse_observer = self.mouse_selection_observer();
        let modifier = Modifier
            .pointer_input((), move |scope: PointerInputScope| {
                let on_clear = on_clear.clone();
                async move { on_clear_selection_requested(scope, on_clear).await }
            })
            .on_globally_positioned(move |c| {
                if let Some(m) = positioned.upgrade() {
                    m.set_container_layout_coordinates(Some(c.clone()));
                }
            })
            .focus_requester(&self.focus_requester)
            .on_focus_changed(move |state: FocusState| {
                let Some(m) = focus.upgrade() else { return };
                if !state.has_focus() && m.has_focus.peek() {
                    m.on_release();
                }
                m.has_focus.set(state.has_focus());
            })
            .focus_target();
        update_selection_touch_mode(modifier, move |in_touch| {
            if let Some(m) = touch.upgrade() {
                m.set_in_touch_mode(in_touch);
            }
        })
        .on_key_event(move |event: &KeyEvent| keys.upgrade().is_some_and(|m| m.on_key_event(event)))
        .pointer_input((), move |scope: PointerInputScope| {
            let observer = mouse_observer.clone();
            async move { await_selection_gestures(scope, observer, None).await }
        })
        .semantics(move |config| {
            let copy = copy_action.clone();
            config.copy_text(move || copy.upgrade().is_some_and(|m| m.copy_from_semantics()));
            let select = select_action.clone();
            config.set_selection(move |start, end, _| {
                let Some(m) = select.upgrade() else { return false };
                if start < 0 || end < 0 {
                    return false;
                }
                m.set_selection_range(TextRange::new(start as usize, end as usize));
                true
            });
        })
    }

    /// The keys a container takes (Compose's copy key event, and C++ Kompoz's select all and Escape): Ctrl+C (or
    /// Ctrl+Insert, Copy) copies, Ctrl+A selects all, Escape clears.
    fn on_key_event(&self, event: &KeyEvent) -> bool {
        if event.r#type != KeyEventType::KeyDown {
            return false;
        }
        if is_copy_key_event(event) {
            self.copy();
            return true;
        }
        if event.modifiers == KeyModifiers::Ctrl && event.key == Key::A {
            self.select_all();
            return true;
        }
        if event.modifiers == KeyModifiers::None && event.key == Key::Escape && self.selection.peek().is_some() {
            self.on_release();
            return true;
        }
        false
    }

    /// The mouse's selection observer of the space between texts (Compose's lazy `mouseSelectionObserver`).
    fn mouse_selection_observer(&self) -> Rc<dyn MouseSelectionObserver> {
        let mut observer = self.mouse_selection_observer.borrow_mut();
        observer
            .get_or_insert_with(|| {
                let this = self.this.clone();
                Rc::new(DefaultMouseSelectionObserver::new(
                    self.registrar.clone() as Rc<dyn SelectionRegistrar>,
                    None,
                    Rc::new(move || this.upgrade().and_then(|m| m.container_layout_coordinates())),
                ))
            })
            .clone()
    }

    // ---- Selecting -----------------------------------------------------------------------------------------

    /// The selectable an anchor is in, if it is still subscribed.
    pub fn get_anchor_selectable(&self, anchor: &AnchorInfo) -> Option<Rc<dyn Selectable>> {
        self.registrar.selectable(anchor.selectable_id)
    }

    /// The container's coordinates while it is attached (Compose's `requireContainerCoordinates`, which throws
    /// where this is none: what asks then does nothing).
    pub fn require_container_coordinates(&self) -> Option<LayoutCoordinates> {
        self.container_layout_coordinates.borrow().clone().filter(|c| c.is_attached())
    }

    fn update_handle_offsets(&self) {
        let selection = self.selection.peek();
        let container = self.container_layout_coordinates();
        let start_selectable = selection.and_then(|s| self.get_anchor_selectable(&s.start));
        let end_selectable = selection.and_then(|s| self.get_anchor_selectable(&s.end));
        let start_coordinates = start_selectable.as_ref().and_then(|s| s.get_layout_coordinates());
        let end_coordinates = end_selectable.as_ref().and_then(|s| s.get_layout_coordinates());
        let (Some(selection), Some(container)) = (selection, container) else {
            self.start_handle_position.set(None);
            self.end_handle_position.set(None);
            return;
        };
        if !container.is_attached() || (start_coordinates.is_none() && end_coordinates.is_none()) {
            self.start_handle_position.set(None);
            self.end_handle_position.set(None);
            return;
        }
        let visible_bounds = visible_bounds(&container);
        let dragging = self.dragging_handle.peek();
        // A handle out of the visible bounds is hidden — unless it is being dragged (its popup would go, and its
        // drag with it).
        let position = |selectable: &Option<Rc<dyn Selectable>>,
                        coordinates: &Option<LayoutCoordinates>,
                        is_start: bool,
                        handle: Handle| {
            let (Some(selectable), Some(coordinates)) = (selectable, coordinates) else { return None };
            let handle_position = selectable.get_handle_position(&selection, is_start);
            if handle_position.is_unspecified() {
                return None;
            }
            let p = container.local_position_of(coordinates, handle_position);
            (dragging == Some(handle) || contains_inclusive(visible_bounds, p)).then_some(p)
        };
        self.start_handle_position.set(position(&start_selectable, &start_coordinates, true, Handle::SelectionStart));
        self.end_handle_position.set(position(&end_selectable, &end_coordinates, false, Handle::SelectionEnd));
    }

    /// All of one selectable selected (Compose's `selectAllInSelectable`): the selection and its parts.
    pub fn select_all_in_selectable(
        &self,
        selectable_id: u64,
        previous_selection: Option<Selection>,
    ) -> Option<(Option<Selection>, HashMap<u64, Selection>)> {
        let container = self.require_container_coordinates()?;
        let mut subselections = HashMap::new();
        let mut new_selection: Option<Selection> = None;
        for selectable in self.registrar.sort(&container) {
            let selection =
                if selectable.selectable_id() == selectable_id { selectable.get_select_all_selection() } else { None };
            if let Some(s) = selection {
                subselections.insert(selectable.selectable_id(), s);
            }
            new_selection = merge(new_selection.as_ref(), selection.as_ref());
        }
        if self.is_in_touch_mode.peek() && new_selection != previous_selection {
            self.perform_haptic_feedback(HapticFeedbackType::TextHandleMove);
        }
        Some((new_selection, subselections))
    }

    /// Whether all of the container is selected (Compose's `isEntireContainerSelected`).
    pub fn is_entire_container_selected(&self) -> bool {
        let Some(container) = self.require_container_coordinates() else { return true };
        let selectables = self.registrar.sort(&container);
        let subselections = self.registrar.peek_subselections();
        selectables.iter().all(|s| {
            let text = s.get_text();
            if text.text().is_empty() {
                return true;
            }
            let Some(sub) = subselections.get(&s.selectable_id()) else { return false };
            sub.start.offset.abs_diff(sub.end.offset) == text.text().len()
        })
    }

    /// Sets the parts of `selection` in each selectable (Compose's `updateSubSelections`): for a selection set by
    /// code.
    pub fn update_sub_selections(&self, selection: &Selection) {
        let Some(container) = self.require_container_coordinates() else { return };
        let selectables = self.registrar.sort(&container);
        if selectables.is_empty() {
            return;
        }
        let whole = *selection;
        let subselections = create_sub_selections(
            selection,
            &selectables,
            |s| s.selectable_id(),
            |s, is_crossed| {
                s.get_select_all_selection()
                    .map(|all| if is_crossed { Selection::new(all.end, all.start, true) } else { all })
            },
            |s, is_start, _, is_crossed| {
                s.get_select_all_selection().map(|all| match (is_start, is_crossed) {
                    (true, true) => Selection::new(whole.start, all.start, true),
                    (true, false) => Selection::new(whole.start, all.end, false),
                    (false, true) => Selection::new(all.end, whole.end, true),
                    (false, false) => Selection::new(all.start, whole.end, false),
                })
            },
        );
        self.registrar.set_subselections(subselections);
    }

    /// Sets the selection from code, its parts worked out (Compose's `setSelection(Selection)`).
    pub fn set_selection_and_parts(&self, new_selection: Selection) {
        self.update_sub_selections(&new_selection);
        self.call_on_selection_change(Some(new_selection));
        *self.previous_selection_layout.borrow_mut() = None;
        // The focus, for the handles to show.
        self.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
        self.set_show_toolbar(true);
    }

    /// Selects `range` of the container's texts as one (Compose's `setSelection(TextRange)`): offsets run through
    /// the texts in order, end to end.
    pub fn set_selection_range(&self, range: TextRange) {
        let Some(container) = self.require_container_coordinates() else { return };
        let selectables = self.registrar.sort(&container);
        if selectables.is_empty() {
            return;
        }
        let Some(start_anchor) = find_anchor_for_global_offset(&selectables, range.start) else { return };
        let Some(end_anchor) = find_anchor_for_global_offset(&selectables, range.end) else { return };
        let new_selection = Selection::new(start_anchor, end_anchor, range.start > range.end);
        self.update_sub_selections(&new_selection);
        self.call_on_selection_change(Some(new_selection));
        *self.previous_selection_layout.borrow_mut() = None;
        self.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
        self.set_show_toolbar(true);
    }

    /// Selects all of the container (Compose's `selectAll`).
    pub fn select_all(&self) {
        let Some(container) = self.require_container_coordinates() else { return };
        let selectables = self.registrar.sort(&container);
        if selectables.is_empty() {
            return;
        }
        let mut first: Option<Selection> = None;
        let mut last: Option<Selection> = None;
        let mut new_subselections = HashMap::new();
        for selectable in &selectables {
            let Some(sub) = selectable.get_select_all_selection() else { continue };
            if first.is_none() {
                first = Some(sub);
            }
            last = Some(sub);
            new_subselections.insert(selectable.selectable_id(), sub);
        }
        let (Some(first), Some(last)) = (first, last) else { return };
        let new_selection =
            if new_subselections.len() == 1 { first } else { Selection::new(first.start, last.end, false) };
        self.registrar.set_subselections(new_subselections);
        self.call_on_selection_change(Some(new_selection));
        *self.previous_selection_layout.borrow_mut() = None;
        self.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
        self.set_show_toolbar(true);
    }

    /// Extends the selection by a word the way it goes (on into the next text, or back into the previous one);
    /// with none, selects the first word (Compose's `extendSelectionByWord`).
    pub fn extend_selection_by_word(&self) {
        let Some(container) = self.require_container_coordinates() else { return };
        let selectables = self.registrar.sort(&container);
        if selectables.is_empty() {
            return;
        }
        let Some(current_selection) = self.selection.peek() else {
            let Some(first) = selectables.iter().find(|s| !s.get_text().text().is_empty()) else { return };
            let Some(layout) = first.text_layout_result() else { return };
            let boundary = layout.get_word_boundary(0);
            let text = layout.layout_input.text.text();
            let start_anchor = AnchorInfo::new(get_text_direction_for_offset(&layout, 0), 0, first.selectable_id());
            let end_anchor = AnchorInfo::new(
                get_text_direction_for_offset(&layout, previous_char(text, boundary.end)),
                boundary.end,
                first.selectable_id(),
            );
            let new_selection = Selection::new(start_anchor, end_anchor, false);
            self.registrar.set_subselections(HashMap::from([(first.selectable_id(), new_selection)]));
            self.call_on_selection_change(Some(new_selection));
            *self.previous_selection_layout.borrow_mut() = None;
            self.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
            self.set_show_toolbar(true);
            return;
        };
        let mut current_selectable_id = current_selection.end.selectable_id;
        let Some(mut current_index) = selectables.iter().position(|s| s.selectable_id() == current_selectable_id)
        else {
            return;
        };
        let mut current = selectables[current_index].clone();
        let Some(mut layout) = current.text_layout_result() else { return };
        let mut current_offset = current_selection.end.offset;
        if current_selection.handles_crossed {
            while current_offset == 0 {
                if current_index == 0 {
                    return;
                }
                current_index -= 1;
                current = selectables[current_index].clone();
                current_selectable_id = current.selectable_id();
                let Some(l) = current.text_layout_result() else { return };
                layout = l;
                current_offset = layout.layout_input.text.text().len();
            }
            let text = layout.layout_input.text.text();
            let boundary = layout.get_word_boundary(previous_char(text, current_offset));
            current_offset = if boundary.start < current_offset {
                boundary.start
            } else {
                let peek_offset = previous_char(text, current_offset);
                let next_boundary_start = layout.get_word_boundary(peek_offset).start;
                if next_boundary_start < current_offset { next_boundary_start } else { peek_offset }
            };
        } else {
            let mut max_offset = layout.layout_input.text.text().len();
            while current_offset >= max_offset {
                if current_index + 1 >= selectables.len() {
                    return;
                }
                current_index += 1;
                current = selectables[current_index].clone();
                current_selectable_id = current.selectable_id();
                let Some(l) = current.text_layout_result() else { return };
                layout = l;
                max_offset = layout.layout_input.text.text().len();
                current_offset = 0;
            }
            let text = layout.layout_input.text.text();
            let boundary = layout.get_word_boundary(current_offset);
            current_offset = if boundary.end > current_offset {
                boundary.end
            } else {
                let peek_offset = next_char(text, current_offset).min(previous_char(text, max_offset));
                let next_boundary_end = layout.get_word_boundary(peek_offset).end;
                if next_boundary_end > current_offset { next_boundary_end } else { next_char(text, current_offset) }
            };
        }
        let text = layout.layout_input.text.text();
        let direction_offset = if current_selection.handles_crossed {
            current_offset.min(previous_char(text, text.len()))
        } else {
            previous_char(text, current_offset)
        };
        let new_end_anchor = AnchorInfo::new(
            get_text_direction_for_offset(&layout, direction_offset),
            current_offset,
            current_selectable_id,
        );
        let handles_crossed = if current_selection.start.selectable_id == current_selectable_id {
            current_selection.start.offset > current_offset
        } else {
            selectables
                .iter()
                .position(|s| s.selectable_id() == current_selection.start.selectable_id)
                .is_some_and(|start_index| start_index > current_index)
        };
        let new_selection = Selection::new(current_selection.start, new_end_anchor, handles_crossed);
        self.update_sub_selections(&new_selection);
        self.call_on_selection_change(Some(new_selection));
        *self.previous_selection_layout.borrow_mut() = None;
        self.focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
        self.set_show_toolbar(true);
    }

    /// Whether the selection's anchors are the same (Compose's `isTriviallyCollapsedSelection`; reads the
    /// selection's state). Across several selectables it may select nothing with different anchors: see
    /// [`is_non_empty_selection`](Self::is_non_empty_selection).
    pub fn is_trivially_collapsed_selection(&self) -> bool {
        self.selection.get().is_none_or(|s| s.start == s.end)
    }

    /// Whether the selection selects any character (Compose's `isNonEmptySelection`).
    pub fn is_non_empty_selection(&self) -> bool {
        let Some(selection) = self.selection.peek() else { return false };
        if selection.start == selection.end {
            return false;
        }
        if selection.start.selectable_id == selection.end.selectable_id {
            return true;
        }
        let Some(container) = self.require_container_coordinates() else { return false };
        let subselections = self.registrar.peek_subselections();
        self.registrar
            .sort(&container)
            .iter()
            .any(|s| subselections.get(&s.selectable_id()).is_some_and(|sub| sub.start.offset != sub.end.offset))
    }

    /// The selected text, the parts in each selectable joined by line feeds (Compose's `getSelectedText`).
    pub fn get_selected_text(&self) -> Option<AnnotatedString> {
        if self.selection.peek().is_none() || self.registrar.peek_subselections().is_empty() {
            return None;
        }
        let mut builder = AnnotatedStringBuilder::new();
        self.for_each_selectable_with_selection(|_, text, selection, is_last| {
            builder.append_range(text, selection.min(), selection.max());
            if !is_last {
                builder.append('\n');
            }
            true
        });
        Some(builder.to_annotated_string())
    }

    /// The selected part of each selectable (Compose's `getSelectedTexts`).
    pub fn get_selected_texts(&self) -> Vec<AnnotatedString> {
        if self.selection.peek().is_none() || self.registrar.peek_subselections().is_empty() {
            return Vec::new();
        }
        let mut texts = Vec::new();
        self.for_each_selectable_with_selection(|_, text, selection, _| {
            texts.push(text.sub_sequence(selection.min(), selection.max()));
            true
        });
        texts
    }

    /// The text of each selectable, in order (Compose's `getSelectableTexts`).
    pub fn get_selectable_texts(&self) -> Vec<AnnotatedString> {
        if self.registrar.selectables().is_empty() {
            return Vec::new();
        }
        let Some(container) = self.require_container_coordinates() else { return Vec::new() };
        self.registrar.sort(&container).iter().map(|s| s.get_text()).collect()
    }

    /// The selected text with the rest of its selectables around it, and where the selection is in that
    /// (Compose's `getContextTextAndSelection`): one selectable's whole text, or the first's text up to its part,
    /// the parts, and the last's text from its part on, joined by line feeds.
    pub fn get_context_text_and_selection(&self) -> Option<(AnnotatedString, TextRange)> {
        if self.selection.peek().is_none() || self.registrar.selectables().is_empty() {
            return None;
        }
        let mut start: Option<usize> = None;
        let mut end: Option<usize> = None;
        let mut builder = AnnotatedStringBuilder::new();
        self.for_each_selectable_with_selection(|_, text, selection, is_last| {
            if start.is_none() {
                start = Some(selection.min());
                builder.append_range(text, 0, selection.min());
            }
            builder.append_range(text, selection.min(), selection.max());
            if !is_last {
                builder.append('\n');
            } else {
                end = Some(builder.len());
                builder.append_range(text, selection.max(), text.text().len());
            }
            true
        });
        Some((builder.to_annotated_string(), TextRange::new(start?, end?)))
    }

    /// Runs `block` on each selectable with a part of the selection, in order, with its id, its text, its part
    /// and whether it is the last with a non-empty part, while `block` says to go on (Compose's
    /// `forEachSelectableWithSelection`).
    pub fn for_each_selectable_with_selection(
        &self,
        mut block: impl FnMut(u64, &AnnotatedString, TextRange, bool) -> bool,
    ) {
        let Some(container) = self.require_container_coordinates() else { return };
        let sorted = self.registrar.sort(&container);
        let subselections = self.registrar.peek_subselections();
        let Some(last_selectable_index) = sorted
            .iter()
            .rposition(|s| subselections.get(&s.selectable_id()).is_some_and(|sub| sub.start.offset != sub.end.offset))
        else {
            return;
        };
        for (index, selectable) in sorted.iter().enumerate() {
            if let Some(sub) = subselections.get(&selectable.selectable_id()) {
                let text = selectable.get_text();
                let range = TextRange::new(sub.start.offset, sub.end.offset);
                if !block(selectable.selectable_id(), &text, range, index >= last_selectable_index) {
                    return;
                }
            }
        }
    }

    /// Copies the selected text, if any (Compose's `copy`).
    pub fn copy(&self) {
        let Some(text) = self.get_selected_text().filter(|t| !t.text().is_empty()) else { return };
        let handler = self.on_copy_handler.borrow().clone();
        if let Some(h) = handler {
            h(text);
        }
    }

    fn copy_from_semantics(&self) -> bool {
        if !self.can_copy() {
            return false;
        }
        self.copy();
        true
    }

    /// Whether there is something to copy and a way to (Compose's `canCopy`).
    pub fn can_copy(&self) -> bool {
        self.on_copy_handler.borrow().is_some() && self.is_non_empty_selection()
    }

    fn toolbar_copy(&self) {
        self.copy();
        self.on_release();
    }

    /// Shows or hides the toolbar as things are (Compose's `updateSelectionToolbar`, its text toolbar path).
    fn update_selection_toolbar(&self) {
        if !self.has_focus.peek() {
            return;
        }
        let Some(toolbar) = self.text_toolbar.borrow().clone() else { return };
        if self.show_toolbar.get() && self.is_in_touch_mode.peek() {
            let Some(rect) = self.get_content_rect() else { return };
            let this = self.this.clone();
            let copy: Option<Rc<dyn Fn()>> = self.can_copy().then(|| {
                Rc::new(move || {
                    if let Some(m) = this.upgrade() {
                        m.toolbar_copy();
                    }
                }) as Rc<dyn Fn()>
            });
            let this = self.this.clone();
            let select_all: Option<Rc<dyn Fn()>> = (!self.is_entire_container_selected()).then(|| {
                Rc::new(move || {
                    if let Some(m) = this.upgrade() {
                        m.select_all();
                    }
                }) as Rc<dyn Fn()>
            });
            toolbar.show_menu(rect, copy, None, None, select_all, None);
        } else if toolbar.status() == TextToolbarStatus::Shown {
            toolbar.hide();
        }
    }

    /// The smallest rect around the selection, cut to what is visible of the container, in the root, made taller
    /// by four handle heights (Compose's `getContentRect`); none when nothing of it is visible.
    fn get_content_rect(&self) -> Option<Rect> {
        // Moves when a selected text does.
        self.position_change_state.get();
        self.selection.peek()?;
        let container = self.require_container_coordinates()?;
        let subselections = self.registrar.peek_subselections();
        let pairs: Vec<(Rc<dyn Selectable>, Selection)> = self
            .registrar
            .sort(&container)
            .into_iter()
            .filter_map(|s| subselections.get(&s.selectable_id()).map(|sub| (s, *sub)))
            .collect();
        let pairs = match pairs.len() {
            0 | 1 => pairs,
            n => vec![pairs[0].clone(), pairs[n - 1].clone()],
        };
        if pairs.is_empty() {
            return None;
        }
        let selected = get_selected_region_rect(&pairs, &container);
        if selected == INVERTED_INFINITE_RECT {
            return None;
        }
        let visible = visible_bounds(&container).intersect(selected);
        // Not even touching at the edges: no toolbar.
        if visible.width() < 0.0 || visible.height() < 0.0 {
            return None;
        }
        let root = visible.translate(container.position_in_root());
        Some(Rect::new(root.left, root.top, root.right, root.bottom + HandleHeight.0 * 4.0))
    }

    /// Clears the selection (Compose's `onRelease`).
    pub fn on_release(&self) {
        self.registrar.set_subselections(HashMap::new());
        self.set_show_toolbar(false);
        *self.previous_selection_layout.borrow_mut() = None;
        if self.selection.peek().is_some() {
            self.call_on_selection_change(None);
            if self.is_in_touch_mode.peek() {
                self.perform_haptic_feedback(HapticFeedbackType::TextHandleMove);
            }
        }
    }

    /// The observer of a handle's drag (Compose's `handleDragObserver`).
    pub fn handle_drag_observer(&self, is_start_handle: bool) -> Rc<dyn TextDragObserver> {
        Rc::new(HandleDragObserver {
            manager: self.this.clone(),
            is_start_handle,
            drag_begin_position: Cell::new(Offset::Unspecified),
            drag_total_distance: Cell::new(Offset::Zero),
        })
    }

    fn convert_to_container_coordinates(&self, layout_coordinates: &LayoutCoordinates, offset: Offset) -> Offset {
        match self.require_container_coordinates() {
            Some(c) => c.local_position_of(layout_coordinates, offset),
            None => Offset::Unspecified,
        }
    }

    /// A new selection at `position` (a long press, a double or triple click): both ends there, `is_start_handle`
    /// telling which one the adjustment moves (Compose's `startSelection`).
    fn start_selection(&self, position: Offset, is_start_handle: bool, adjustment: SelectionAdjustment) {
        *self.previous_selection_layout.borrow_mut() = None;
        self.update_selection(position, Offset::Unspecified, is_start_handle, adjustment);
    }

    /// The selection after an end moved to `position` from `previous_handle_position` (unspecified: a new
    /// selection) (Compose's `updateSelection`): whether the move was taken (the selection worked out again).
    pub fn update_selection(
        &self,
        position: Offset,
        previous_handle_position: Offset,
        is_start_handle: bool,
        adjustment: SelectionAdjustment,
    ) -> bool {
        self.dragging_handle.set(Some(if is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd }));
        self.current_drag_position.set(Some(position));
        let Some(selection_layout) = self.get_selection_layout(position, previous_handle_position, is_start_handle)
        else {
            return false;
        };
        let previous_layout = self.previous_selection_layout.borrow().clone();
        let need_to_recompute = selection_layout.should_recompute_selection(previous_layout.as_deref());
        if need_to_recompute {
            let new_selection = adjustment.adjust(&selection_layout);
            if Some(new_selection) != self.selection.peek() {
                if self.should_perform_haptics() {
                    self.perform_haptic_feedback(HapticFeedbackType::TextHandleMove);
                }
                self.registrar.set_subselections(selection_layout.create_sub_selections(&new_selection));
                self.call_on_selection_change(Some(new_selection));
            }
            *self.previous_selection_layout.borrow_mut() = Some(Rc::new(selection_layout));
        }
        self.bring_dragged_position_into_view();
        need_to_recompute
    }

    /// Brings where an end is dragged into view (Compose's `bringDraggedPositionIntoView`): the drag's point, the
    /// edge of the selection there with a finger, and the nearest point of the selectable.
    fn bring_dragged_position_into_view(&self) {
        let Some(layout) = self.previous_selection_layout.borrow().clone() else { return };
        let info = layout.current_info();
        let Some(selectable) = self.registrar.selectable(info.selectable_id) else { return };
        let Some(selectable_coordinates) = selectable.get_layout_coordinates() else { return };
        let Some(requester) = selectable.bring_into_view_requester() else { return };
        let Some(drag_position) = self.current_drag_position.peek() else { return };
        let Some(container) = self.require_container_coordinates() else { return };
        let position_in_selectable = selectable_coordinates.local_position_of(&container, drag_position);
        let mut rect = Rect::from_offset_size(position_in_selectable, Size::Zero);
        if self.is_in_touch_mode.peek() {
            let offset =
                if layout.is_start_handle() { info.raw_start_handle_offset } else { info.raw_end_handle_offset };
            rect = rect.union(info.text_layout_result.get_cursor_rect(offset));
        }
        // The selectable's nearest edge too: a lazy layout isn't scrolled past what it hasn't composed yet.
        let size = selectable_coordinates.size();
        let nearest = Offset::new(
            position_in_selectable.x.clamp(0.0, size.width as f32),
            position_in_selectable.y.clamp(0.0, size.height as f32),
        );
        rect = Rect::new(
            rect.left.min(nearest.x),
            rect.top.min(nearest.y),
            rect.right.max(nearest.x),
            rect.bottom.max(nearest.y),
        );
        let scope = self.coroutine_scope.borrow().clone();
        if let Some(scope) = scope {
            let request = requester.bring_into_view_rect(Some(rect));
            named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, coroutine = request));
        }
    }

    fn get_selection_layout(
        &self,
        position: Offset,
        previous_handle_position: Offset,
        is_start_handle: bool,
    ) -> Option<SelectionLayout> {
        let container = self.require_container_coordinates()?;
        let sorted = self.registrar.sort(&container);
        let order: HashMap<u64, usize> = sorted.iter().enumerate().map(|(i, s)| (s.selectable_id(), i)).collect();
        // No previous position: a new selection.
        let new_selection = previous_handle_position.is_unspecified();
        let previous_selection = if new_selection { None } else { self.selection.peek() };
        let previous_layout = if new_selection { None } else { self.previous_selection_layout.borrow().clone() };
        let mut builder = SelectionLayoutBuilder::new(
            position,
            previous_handle_position,
            container,
            is_start_handle,
            previous_selection,
            previous_layout,
            Rc::new(order),
            !self.is_in_touch_mode.peek(),
        );
        for selectable in &sorted {
            selectable.append_selectable_info_to_builder(&mut builder);
        }
        builder.build()
    }

    /// Whether a finger's selection change is felt: with a finger, over some text (Compose's
    /// `shouldPerformHaptics`).
    pub fn should_perform_haptics(&self) -> bool {
        self.is_in_touch_mode.peek() && self.registrar.selectables().iter().any(|s| !s.get_text().text().is_empty())
    }

    /// Selects the word at `position` unless it is selected already (Compose's
    /// `selectWordAtPositionIfNotAlreadySelected`: macOS's right click).
    pub fn select_word_at_position_if_not_already_selected(&self, position: Offset) {
        let Some(container) = self.require_container_coordinates() else { return };
        let subselections = self.registrar.peek_subselections();
        let inside = self.registrar.selectables().iter().any(|s| {
            let Some(selection) = subselections.get(&s.selectable_id()) else { return false };
            let Some(coordinates) = s.get_layout_coordinates() else { return false };
            let Some(layout) = s.text_layout_result() else { return false };
            let in_selectable = coordinates.local_position_of(&container, position);
            is_position_inside_selection(&layout, in_selectable, Some(selection.to_text_range()))
        });
        if !inside {
            self.start_selection(position, true, SelectionAdjustment::Word);
        }
    }

    /// The items of its context menu (Compose's `contextMenuBuilder`): copy when something is selected, select
    /// all unless all is.
    pub fn context_menu_data(&self) -> TextContextMenuData {
        let copy = self.this.clone();
        let select_all = self.this.clone();
        TextContextMenuData::new(vec![
            TextContextMenuComponent::item(TextContextMenuItems::Copy, self.is_non_empty_selection(), move || {
                if let Some(m) = copy.upgrade() {
                    m.copy();
                    if m.is_in_touch_mode.peek() {
                        m.on_release();
                    }
                }
            }),
            TextContextMenuComponent::item(
                TextContextMenuItems::SelectAll,
                !self.is_entire_container_selected(),
                move || {
                    if let Some(m) = select_all.upgrade() {
                        m.select_all();
                    }
                },
            ),
        ])
    }

    /// Releases the pins of every selectable (Compose's `unpinAllSelectables`: the manager forgotten).
    pub fn unpin_all_selectables(&self) {
        let pinned: Vec<PinnedHandle> =
            self.pinned_handle_by_selectable_id.borrow_mut().drain().map(|(_, h)| h).collect();
        for handle in pinned {
            handle.release();
        }
    }

    /// Pins the selectables with a non-empty part of the selection, unpins the others (Compose's
    /// `updatePinnedSelectables`).
    fn update_pinned_selectables(&self) {
        let subselections = self.registrar.peek_subselections();
        let released: Vec<PinnedHandle> = {
            let mut pinned = self.pinned_handle_by_selectable_id.borrow_mut();
            let gone: Vec<u64> = pinned.keys().filter(|id| !subselections.contains_key(id)).copied().collect();
            gone.into_iter().filter_map(|id| pinned.remove(&id)).collect()
        };
        for handle in released {
            handle.release();
        }
        for (id, sub) in subselections.iter() {
            if sub.start.offset == sub.end.offset || self.pinned_handle_by_selectable_id.borrow().contains_key(id) {
                continue;
            }
            let Some(selectable) = self.registrar.selectable(*id) else { continue };
            let Some(container) = selectable.pinnable_container() else { continue };
            let handle = container.pin();
            self.pinned_handle_by_selectable_id.borrow_mut().insert(*id, handle);
        }
    }
}

/// Clears the selection when a primary press ends without a drag while no handle is dragged (Compose's
/// `onClearSelectionRequested`): seen in the Initial pass, before a drag's end clears the dragging handle.
async fn on_clear_selection_requested(scope: PointerInputScope, manager: Weak<SelectionManager>) {
    scope
        .await_each_gesture(|s| {
            let manager = manager.clone();
            async move {
                // The primary button only: a secondary click opens a context menu.
                let primary_first_down = s.await_first_down_with(false, PointerEventPass::Main).await;
                let pointer_slop_reached =
                    await_all_pointers_up_with_slop_detection(&s, &primary_first_down, PointerEventPass::Initial).await;
                if let Some(m) = manager.upgrade()
                    && !pointer_slop_reached
                    && m.dragging_handle.peek().is_none()
                {
                    m.on_release();
                }
            }
        })
        .await
}

/// A handle's drag (Compose's `handleDragObserver`): its end follows the finger from where the handle points, by
/// character, accelerated by word.
struct HandleDragObserver {
    manager: Weak<SelectionManager>,
    is_start_handle: bool,
    /// Where the moves not taken yet began.
    drag_begin_position: Cell<Offset>,
    /// How far they went.
    drag_total_distance: Cell<Offset>,
}

impl HandleDragObserver {
    /// Where the handle's end begins a drag, in the container: its handle position, a pixel up into its line.
    fn begin_position(&self, m: &SelectionManager) -> Option<Offset> {
        let selection = m.selection.peek()?;
        let anchor = if self.is_start_handle { selection.start } else { selection.end };
        let selectable = m.get_anchor_selectable(&anchor)?;
        let begin_coordinates = selectable.get_layout_coordinates()?;
        let handle_position = selectable.get_handle_position(&selection, self.is_start_handle);
        if handle_position.is_unspecified() {
            return None;
        }
        let container = m.require_container_coordinates()?;
        Some(container.local_position_of(&begin_coordinates, super::handles::get_adjusted_coordinates(handle_position)))
    }

    fn done(&self) {
        let Some(m) = self.manager.upgrade() else { return };
        m.set_show_toolbar(true);
        m.dragging_handle.set(None);
        m.current_drag_position.set(None);
    }
}

impl TextDragObserver for HandleDragObserver {
    fn on_down(&self, _point: Offset) {
        let Some(m) = self.manager.upgrade() else { return };
        // An invisible handle takes no gesture.
        let shown = if self.is_start_handle { m.start_handle_position.peek() } else { m.end_handle_position.peek() };
        if shown.is_none() {
            return;
        }
        let Some(begin) = self.begin_position(&m) else { return };
        m.current_drag_position.set(Some(begin));
        m.dragging_handle.set(Some(if self.is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd }));
        m.set_show_toolbar(false);
    }

    fn on_up(&self) {
        self.done();
    }

    fn on_start(&self, _start_point: Offset, _selection_adjustment: SelectionAdjustment) {
        let Some(m) = self.manager.upgrade() else { return };
        if m.dragging_handle.peek().is_none() {
            return;
        }
        let Some(begin) = self.begin_position(&m) else { return };
        self.drag_begin_position.set(begin);
        self.drag_total_distance.set(Offset::Zero);
    }

    fn on_drag(&self, delta: Offset) {
        let Some(m) = self.manager.upgrade() else { return };
        if m.dragging_handle.peek().is_none() {
            return;
        }
        self.drag_total_distance.set(self.drag_total_distance.get() + delta);
        let end_position = self.drag_begin_position.get() + self.drag_total_distance.get();
        let consumed = m.update_selection(
            end_position,
            self.drag_begin_position.get(),
            self.is_start_handle,
            SelectionAdjustment::CharacterWithWordAccelerate,
        );
        if consumed {
            self.drag_begin_position.set(end_position);
            self.drag_total_distance.set(Offset::Zero);
        }
    }

    fn on_stop(&self) {
        self.done();
    }

    fn on_cancel(&self) {
        self.done();
    }
}

/// Whether the key event copies (Compose's `isCopyKeyEvent`, the default key mapping's COPY): Ctrl+C,
/// Ctrl+Insert, or the Copy key.
pub fn is_copy_key_event(event: &KeyEvent) -> bool {
    match event.modifiers {
        KeyModifiers::Ctrl => matches!(event.key, Key::C | Key::Insert | Key::NumPadInsert),
        KeyModifiers::None => event.key == Key::Copy,
        _ => false,
    }
}

/// The anchor at an offset into the container's texts taken end to end (Compose's `findAnchorForGlobalOffset`).
fn find_anchor_for_global_offset(selectables: &[Rc<dyn Selectable>], global_offset: usize) -> Option<AnchorInfo> {
    let mut running_length = 0;
    for selectable in selectables {
        let text_length = selectable.get_text().text().len();
        if global_offset >= running_length && global_offset <= running_length + text_length {
            let local_offset = global_offset - running_length;
            let layout = selectable.text_layout_result()?;
            return Some(AnchorInfo::new(
                get_text_direction_for_offset(&layout, local_offset),
                local_offset,
                selectable.selectable_id(),
            ));
        }
        running_length += text_length;
    }
    None
}

/// A rect with nothing in it, that anything widens (Compose's invertedInfiniteRect).
const INVERTED_INFINITE_RECT: Rect =
    Rect { left: f32::INFINITY, top: f32::INFINITY, right: f32::NEG_INFINITY, bottom: f32::NEG_INFINITY };

/// The smallest rect around the first and last selected characters of each selectable's part, in the container
/// (Compose's `getSelectedRegionRect`); a rect inverted to infinity (each side past the other) if there is none.
pub fn get_selected_region_rect(pairs: &[(Rc<dyn Selectable>, Selection)], container: &LayoutCoordinates) -> Rect {
    let mut r = INVERTED_INFINITE_RECT;
    for (selectable, sub) in pairs {
        let (start_offset, end_offset) = (sub.start.offset, sub.end.offset);
        if start_offset == end_offset {
            continue;
        }
        let Some(local_coordinates) = selectable.get_layout_coordinates() else { continue };
        let min_offset = start_offset.min(end_offset);
        let max_offset = start_offset.max(end_offset);
        let text = selectable.get_text();
        let last = previous_char(text.text(), max_offset);
        let offsets: &[usize] = if min_offset == last { &[min_offset] } else { &[min_offset, last] };
        let mut local = INVERTED_INFINITE_RECT;
        for &i in offsets {
            let b = selectable.get_bounding_box(i);
            local = Rect::new(
                local.left.min(b.left),
                local.top.min(b.top),
                local.right.max(b.right),
                local.bottom.max(b.bottom),
            );
        }
        let top_left = container.local_position_of(&local_coordinates, Offset::new(local.left, local.top));
        let bottom_right = container.local_position_of(&local_coordinates, Offset::new(local.right, local.bottom));
        r = Rect::new(
            r.left.min(top_left.x),
            r.top.min(top_left.y),
            r.right.max(bottom_right.x),
            r.bottom.max(bottom_right.y),
        );
    }
    r
}

/// Where the magnifier of a dragged handle centers, in the container (Compose's
/// `calculateSelectionMagnifierCenterAndroid`): at the drag's x kept within its line, at the line's middle;
/// unspecified when no handle is dragged, its offset isn't visible, or the drag went further from the line than
/// half the magnifier's width (`magnifier_size`, zero before it is first shown).
pub fn calculate_selection_magnifier_center(manager: &SelectionManager, magnifier_size: IntSize) -> Offset {
    let Some(selection) = manager.selection() else { return Offset::Unspecified };
    match manager.dragging_handle() {
        None => Offset::Unspecified,
        Some(Handle::SelectionStart) => magnifier_center(manager, magnifier_size, &selection.start),
        Some(Handle::SelectionEnd) => magnifier_center(manager, magnifier_size, &selection.end),
        Some(Handle::Cursor) => panic!("SelectionContainer does not support cursor"),
    }
}

fn magnifier_center(manager: &SelectionManager, magnifier_size: IntSize, anchor: &AnchorInfo) -> Offset {
    let Some(selectable) = manager.get_anchor_selectable(anchor) else { return Offset::Unspecified };
    let Some(container) = manager.container_layout_coordinates() else { return Offset::Unspecified };
    let Some(selectable_coordinates) = selectable.get_layout_coordinates() else { return Offset::Unspecified };
    let Some(drag_position) = manager.current_drag_position() else { return Offset::Unspecified };
    let offset = anchor.offset;
    if offset > selectable.get_last_visible_offset() {
        return Offset::Unspecified;
    }
    // The x follows the drag, not the carets…
    let drag_x = selectable_coordinates.local_position_of(&container, drag_position).x;
    // …within the line's sides.
    let line_range = selectable.get_range_of_line_containing(offset);
    let text_constrained_x = if line_range.collapsed() {
        // (no text: its line's sides are the same)
        selectable.get_line_left(offset)
    } else {
        let text = selectable.get_text();
        let line_start_x = selectable.get_line_left(line_range.start);
        let line_end_x = selectable.get_line_right(previous_char(text.text(), line_range.end));
        // (right to left, the start may be the larger)
        drag_x.clamp(line_start_x.min(line_end_x), line_start_x.max(line_end_x))
    };
    if text_constrained_x == -1.0 {
        return Offset::Unspecified;
    }
    // Dragged further than the magnifier is wide from the line: hidden (Android's Editor).
    if magnifier_size != IntSize::Zero && (drag_x - text_constrained_x).abs() > (magnifier_size.width / 2) as f32 {
        return Offset::Unspecified;
    }
    let line_center_y = selectable.get_center_y_for_offset(offset);
    if line_center_y == -1.0 {
        return Offset::Unspecified;
    }
    container.local_position_of(&selectable_coordinates, Offset::new(text_constrained_x, line_center_y))
}

/// What is visible of a layout, in its coordinates (Compose's `visibleBounds`): its bounds in the window, cut by
/// its parents, brought back.
pub fn visible_bounds(coordinates: &LayoutCoordinates) -> Rect {
    let b = coordinates.bounds_in_window();
    Rect::from_corners(coordinates.window_to_local(b.top_left()), coordinates.window_to_local(b.bottom_right()))
}
