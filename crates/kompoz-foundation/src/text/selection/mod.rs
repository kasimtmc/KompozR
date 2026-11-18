//! Text selection, as Compose's `androidx.compose.foundation.text.selection`: [`SelectionContainer`] (its texts
//! selected as one, by touch, mouse and keys) and [`DisableSelection`], and what they are made of — the selection
//! ([`Selection`]), what selects ([`Selectable`], [`MultiWidgetSelectionDelegate`]) and what it subscribes to
//! ([`SelectionRegistrar`]), how gestures map onto texts ([`SelectionLayout`]) and snap ([`SelectionAdjustment`]:
//! characters, ICU's words, paragraphs), the [`SelectionManager`], the gestures, the handles, the magnifier's
//! position, the colors ([`LocalTextSelectionColors`]) and the app's [`SelectionState`].
//!
//! Offsets are UTF-8 byte offsets into a selectable's text, on character boundaries (where Compose steps a UTF-16
//! unit, a character is stepped).

mod adjustment;
mod colors;
pub(crate) mod container;
mod delegate;
mod gestures;
mod handles;
mod helpers;
mod layout;
mod manager;
mod mode;
mod registrar;
mod selectable;
#[allow(clippy::module_inception)]
mod selection;
mod state;

pub use adjustment::{SelectionAdjustment, ensure_at_least_one_char};
pub use colors::{LocalTextSelectionColors, TextSelectionColors};
pub use container::{
    DisableSelection, SelectionContainer, SelectionContainerWithSelection, SelectionContainerWithState, SimpleLayout,
};
pub use delegate::{MultiWidgetSelectionDelegate, append_selectable_info};
pub use gestures::{
    ClicksCounter, FirstLongPressSelectionAdjustment, MouseSelectionObserver, await_selection_gestures,
    is_mouse_or_touch_pad, mouse_selection, touch_selection_first_press, update_selection_touch_mode,
};
pub use handles::{
    HandleHeight, HandlePopup, HandlePositionProvider, HandleWidth, OffsetProvider, SelectionHandle,
    SelectionHandleAnchor, SelectionHandleIcon, SelectionHandleInfo, SelectionHandleInfoKey, draw_selection_handle,
    get_adjusted_coordinates, is_handle_ltr_direction, is_left_selection_handle,
};
pub use helpers::{
    get_line_height, get_selection_handle_coordinates, get_text_direction_for_offset, is_position_inside_selection,
};
pub use layout::{
    CrossStatus, Direction, SelectableInfo, SelectionLayout, SelectionLayoutBuilder, UNASSIGNED_SLOT,
    create_sub_selections, get_text_field_selection_layout, is_collapsed, resolve_2d_direction,
};
pub use manager::{
    SelectionManager, calculate_selection_magnifier_center, get_selected_region_rect, is_copy_key_event, visible_bounds,
};
pub use mode::SelectionMode;
pub use registrar::{
    InvalidSelectableId, LocalSelectionRegistrar, SelectionRegistrar, SelectionRegistrarImpl, SelectionRegistrarRef,
    Subselections, has_selection, in_a_row,
};
pub use selectable::Selectable;
pub use selection::{AnchorInfo, Selection, merge};
pub use state::{SelectionState, remember_selection_state};
