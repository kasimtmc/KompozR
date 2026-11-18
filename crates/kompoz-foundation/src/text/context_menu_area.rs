//! The desktop's text context menus (Compose for desktop's ContextMenu.desktop.kt): a selection container's and a
//! text field's own [`ContextMenuArea`](crate::context_menu::ContextMenuArea) — cut, copy, paste and select all,
//! labelled in the UI's language ([`LocalStrings`]), shown by [`LocalTextContextMenu`] (by default with the disabled
//! items greyed; [`TextContextMenu::HideDisabledMenuItems`] leaves them out). A right click opens it where it was
//! clicked (on macOS selecting the word there first, unless it is in the selection).
//!
//! (Compose 1.12's new text context menu, `ComposeFoundationFlags.isNewContextMenuEnabled`, is off on the desktop:
//! not ported, as the state field and SelectionContainer.)

use super::input::internal::selection::TextFieldSelectionState;
use super::key_mapping::DesktopPlatform;
use super::selection::SelectionManager;
use super::selection::text_field_selection_manager::TextFieldSelectionManager;
use crate::context_menu::{
    ContextMenuItem, ContextMenuModifiers, ContextMenuState, ContextMenuStatus, context_menu_area_with_modifier,
};
use kompoz_graphics::{Offset, Rect};
use kompoz_runtime::{KompozitionLocal, kompozable, kompozable_block, remember, remember_keyed};
use kompoz_text::AnnotatedString;
use kompoz_ui::Modifier;
use kompoz_ui::strings::LocalStrings;
use std::rc::Rc;

/// An action of a text context menu (Compose for desktop's `TextContextMenu.Action`): whether it can be done now,
/// and doing it.
#[derive(Clone)]
pub struct TextContextMenuAction {
    pub enabled: bool,
    pub execute: Rc<dyn Fn()>,
}

impl TextContextMenuAction {
    pub fn new(enabled: bool, execute: impl Fn() + 'static) -> TextContextMenuAction {
        TextContextMenuAction { enabled, execute: Rc::new(execute) }
    }
}

/// What a text context menu acts on (Compose for desktop's `TextContextMenu.TextManager`): the selected text, the
/// four actions (none: not offered — a selection container can't cut, say), and the macOS right click's word
/// selection.
pub trait TextManager {
    /// The selected text (Compose's `selectedText`).
    fn selected_text(&self) -> AnnotatedString;
    /// Cutting the selected text to the clipboard (Compose's `cut`).
    fn cut(&self) -> Option<TextContextMenuAction>;
    /// Copying it (Compose's `copy`).
    fn copy(&self) -> Option<TextContextMenuAction>;
    /// Pasting the clipboard's text (Compose's `paste`).
    fn paste(&self) -> Option<TextContextMenuAction>;
    /// Selecting all (Compose's `selectAll`).
    fn select_all(&self) -> Option<TextContextMenuAction>;
    /// Selects the word at `offset` unless the selection already holds that position (Compose's
    /// `selectWordAtPositionIfNotAlreadySelected`).
    fn select_word_at_position_if_not_already_selected(&self, offset: Offset);
}

/// How texts' context menus show (Compose for desktop's `interface TextContextMenu`): an area over the content,
/// opening the menu of `text_manager`'s actions (usually through [`TextContextMenuArea`]).
pub trait TextContextMenu {
    /// The area (Compose's composable `Area`).
    fn area(&self, text_manager: Rc<dyn TextManager>, state: ContextMenuState, content: Rc<dyn Fn()>);
}

impl dyn TextContextMenu {
    /// The default (Compose's `TextContextMenu.Default`): the disabled items shown, greyed.
    pub const Default: BasicTextContextMenu = BasicTextContextMenu { show_disabled_items: true };
    /// One leaving out the items that can't be chosen (Compose's `TextContextMenu.HideDisabledMenuItems`).
    pub const HideDisabledMenuItems: BasicTextContextMenu = BasicTextContextMenu { show_disabled_items: false };
}

/// The text context menu of texts (Compose for desktop's `LocalTextContextMenu`): the default one unless provided.
pub static LocalTextContextMenu: KompozitionLocal<Rc<dyn TextContextMenu>> =
    KompozitionLocal::new_static(|| Rc::new(<dyn TextContextMenu>::Default));

/// Compose for desktop's BasicTextContextMenu: cut, copy, paste and select all (those the text manager offers),
/// the disabled ones shown unless `show_disabled_items` is false.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BasicTextContextMenu {
    pub show_disabled_items: bool,
}

impl TextContextMenu for BasicTextContextMenu {
    #[track_caller]
    fn area(&self, text_manager: Rc<dyn TextManager>, state: ContextMenuState, content: Rc<dyn Fn()>) {
        let show_disabled_items = self.show_disabled_items;
        kompozable_block!({
            let strings = LocalStrings.current();
            let manager = text_manager.clone();
            let items = move || {
                [
                    (strings.cut.clone(), manager.cut()),
                    (strings.copy.clone(), manager.copy()),
                    (strings.paste.clone(), manager.paste()),
                    (strings.select_all.clone(), manager.select_all()),
                ]
                .into_iter()
                .filter_map(|(label, action)| {
                    let action = action?;
                    if !show_disabled_items && !action.enabled {
                        return None;
                    }
                    let execute = action.execute.clone();
                    Some(ContextMenuItem::with_enabled(label, action.enabled, move || execute()))
                })
                .collect::<Vec<_>>()
            };
            TextContextMenuArea(text_manager, items, state, content);
        })
    }
}

/// The context menu area of a text (Compose for desktop's `TextContextMenuArea`): `items` with those around, opened
/// by a right click (on macOS the word clicked selected first).
#[kompozable]
pub fn TextContextMenuArea(
    text_manager: Rc<dyn TextManager>,
    items: impl Fn() -> Vec<ContextMenuItem> + 'static,
    state: ContextMenuState,
    content: Rc<dyn Fn()>,
) {
    let (manager, open) = (text_manager.clone(), state.clone());
    let key = (Rc::as_ptr(&text_manager) as *const () as usize, state.clone());
    let detector = Modifier.context_menu_open_detector_with(key, true, move |pointer_position| {
        if DesktopPlatform::current() == DesktopPlatform::MacOS {
            manager.select_word_at_position_if_not_already_selected(pointer_position);
        }
        open.set_status(ContextMenuStatus::Open(Rect::from_center(pointer_position, 0.0)));
    });
    context_menu_area_with_modifier(Rc::new(items), state, detector, content);
}

/// `menu`'s area (its trait method, called outside the composable call rewriting).
#[track_caller]
fn show_area(
    menu: &dyn TextContextMenu,
    text_manager: Rc<dyn TextManager>,
    state: ContextMenuState,
    content: Rc<dyn Fn()>,
) {
    menu.area(text_manager, state, content);
}

// ---- The text managers ----

/// A value-based field's (Compose desktop's `TextFieldSelectionManager.textManager`): each action asks the field
/// for the focus back once done.
struct LegacyFieldTextManager(Rc<TextFieldSelectionManager>);

impl TextManager for LegacyFieldTextManager {
    fn selected_text(&self) -> AnnotatedString {
        let value = self.0.value();
        let selection = value.selection();
        AnnotatedString::new(value.text()[selection.min()..selection.max()].to_string())
    }
    fn cut(&self) -> Option<TextContextMenuAction> {
        let m = self.0.clone();
        Some(TextContextMenuAction::new(self.0.is_cut_allowed(), move || {
            m.cut();
            m.request_focus();
        }))
    }
    fn copy(&self) -> Option<TextContextMenuAction> {
        let m = self.0.clone();
        Some(TextContextMenuAction::new(self.0.is_copy_allowed(), move || {
            m.copy(false);
            m.request_focus();
        }))
    }
    fn paste(&self) -> Option<TextContextMenuAction> {
        // (Compose desktop's `editable && clipboard?.nativeClipboardHasText()`)
        self.0.update_clipboard_entry();
        let m = self.0.clone();
        Some(TextContextMenuAction::new(self.0.can_show_paste_menu_item(), move || {
            m.paste();
            m.request_focus();
        }))
    }
    fn select_all(&self) -> Option<TextContextMenuAction> {
        let m = self.0.clone();
        Some(TextContextMenuAction::new(self.0.can_show_select_all_menu_item(), move || {
            m.select_all();
            m.request_focus();
        }))
    }
    fn select_word_at_position_if_not_already_selected(&self, offset: Offset) {
        self.0.select_word_at_position_if_not_already_selected(offset);
    }
}

/// A state-based text field's (Compose desktop's `TextFieldSelectionState.textManager`): its clipboard calls are
/// KompozR's synchronous ones (Compose launches them undispatched).
struct FieldTextManager(Rc<TextFieldSelectionState>);

impl TextManager for FieldTextManager {
    fn selected_text(&self) -> AnnotatedString {
        AnnotatedString::new(self.0.text_field_state().visual_text().get_selected_text().to_string())
    }
    fn cut(&self) -> Option<TextContextMenuAction> {
        let s = self.0.clone();
        Some(TextContextMenuAction::new(self.0.can_show_cut_menu_item(), move || s.cut()))
    }
    fn copy(&self) -> Option<TextContextMenuAction> {
        let s = self.0.clone();
        Some(TextContextMenuAction::new(self.0.can_show_copy_menu_item(), move || s.copy(true)))
    }
    fn paste(&self) -> Option<TextContextMenuAction> {
        self.0.update_clipboard_entry();
        let s = self.0.clone();
        Some(TextContextMenuAction::new(self.0.can_show_paste_menu_item(), move || s.paste(false)))
    }
    fn select_all(&self) -> Option<TextContextMenuAction> {
        let s = self.0.clone();
        Some(TextContextMenuAction::new(self.0.can_show_select_all_menu_item(), move || s.select_all()))
    }
    fn select_word_at_position_if_not_already_selected(&self, offset: Offset) {
        self.0.select_word_at_position_if_not_already_selected(offset);
    }
}

/// A selection container's (Compose desktop's `SelectionManager.textManager`): copy only.
struct SelectionTextManager(Rc<SelectionManager>);

impl TextManager for SelectionTextManager {
    fn selected_text(&self) -> AnnotatedString {
        self.0.get_selected_text().unwrap_or_else(|| AnnotatedString::new(String::new()))
    }
    fn cut(&self) -> Option<TextContextMenuAction> {
        None
    }
    fn copy(&self) -> Option<TextContextMenuAction> {
        let m = self.0.clone();
        Some(TextContextMenuAction::new(self.0.is_non_empty_selection(), move || m.copy()))
    }
    fn paste(&self) -> Option<TextContextMenuAction> {
        None
    }
    fn select_all(&self) -> Option<TextContextMenuAction> {
        None
    }
    fn select_word_at_position_if_not_already_selected(&self, offset: Offset) {
        self.0.select_word_at_position_if_not_already_selected(offset);
    }
}

/// A value-based field's context menu area (Compose desktop's `ContextMenuArea(manager: TextFieldSelectionManager)`).
#[track_caller]
pub(crate) fn legacy_text_field_context_menu_area(manager: &Rc<TextFieldSelectionManager>, content: Rc<dyn Fn()>) {
    let manager = manager.clone();
    kompozable_block!({
        let state = remember(ContextMenuState::new);
        let key = Rc::as_ptr(&manager) as usize;
        let text_manager =
            remember_keyed(key, || Rc::new(LegacyFieldTextManager(manager.clone())) as Rc<dyn TextManager>);
        show_area(&*LocalTextContextMenu.current(), text_manager, state, content);
    })
}

/// A state-based text field's context menu area (Compose desktop's `ContextMenuArea(selectionState, enabled)`):
/// none when not `enabled`.
#[track_caller]
pub(crate) fn text_field_context_menu_area(
    selection_state: &Rc<TextFieldSelectionState>,
    enabled: bool,
    content: Rc<dyn Fn()>,
) {
    let selection_state = selection_state.clone();
    kompozable_block!({
        if !enabled {
            content();
        } else {
            let state = remember(ContextMenuState::new);
            let key = Rc::as_ptr(&selection_state) as usize;
            let text_manager =
                remember_keyed(key, || Rc::new(FieldTextManager(selection_state.clone())) as Rc<dyn TextManager>);
            show_area(&*LocalTextContextMenu.current(), text_manager, state, content);
        }
    })
}

/// A selection container's context menu area (Compose desktop's `ContextMenuArea(manager)`).
#[track_caller]
pub(crate) fn selection_context_menu_area(manager: &Rc<SelectionManager>, content: Rc<dyn Fn()>) {
    let manager = manager.clone();
    kompozable_block!({
        let state = remember(ContextMenuState::new);
        let key = Rc::as_ptr(&manager) as usize;
        let text_manager =
            remember_keyed(key, || Rc::new(SelectionTextManager(manager.clone())) as Rc<dyn TextManager>);
        show_area(&*LocalTextContextMenu.current(), text_manager, state, content);
    })
}
