//! The exposed dropdown menu's platform parts, as Compose for desktop's actuals (ExposedDropdownMenu.skiko.kt): the
//! window's bounds (its container's size), following them as they change, and whether the menu's popup takes the
//! focus by its anchor's type.

use crate::exposed_dropdown_menu::ExposedDropdownMenuAnchorType;
use crate::internal::accessibility_service_state_provider::remember_accessibility_service_state;
use kompoz_coroutines::{LaunchedEffect, snapshot_flow};
use kompoz_graphics::{IntOffset, IntRect};
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::platform::{LocalWindowInfo, WindowInfo};
use kompoz_ui::window::PopupProperties;

/// The window's bounds (Compose's internal `expect class WindowBoundsCalculator`, Compose for desktop's actual: its
/// window info's container size).
#[derive(Clone)]
pub(crate) struct WindowBoundsCalculator {
    window_info: WindowInfo,
}

impl WindowBoundsCalculator {
    /// The window's visible bounds (Compose's `getVisibleWindowBounds`: its container's size from the origin).
    pub(crate) fn get_visible_window_bounds(&self) -> IntRect {
        IntRect::from_offset_size(IntOffset::Zero, self.window_info.container_size())
    }
}

/// The window's bounds calculator (Compose's internal `platformWindowBoundsCalculator`, Compose for desktop's
/// actual: of [`LocalWindowInfo`], remembered while it stays the same).
#[kompozable]
pub(crate) fn platform_window_bounds_calculator() -> WindowBoundsCalculator {
    let window_info = LocalWindowInfo.current();
    remember_keyed(window_info.clone(), move || WindowBoundsCalculator { window_info })
}

/// Runs `block` as the window's bounds change (Compose's internal `OnPlatformWindowBoundsChange`, Compose for
/// desktop's actual: a flow of the window's container size collected while the window info stays the same — the
/// first size too; the block first given kept, as Compose's effect keeps its lambda).
#[kompozable]
pub(crate) fn OnPlatformWindowBoundsChange(block: impl Fn() + 'static) {
    let window_info = LocalWindowInfo.current();
    let info = window_info.clone();
    LaunchedEffect(window_info, async move {
        snapshot_flow(move || info.container_size()).collect(move |_| block()).await;
    });
}

/// The menu's popup properties for its anchor's type (Compose's internal `popupPropertiesForAnchorType`, Compose for
/// desktop's actual): not focusable — so that typing stays with the input method — where typing is needed (an
/// editable primary anchor, or a secondary one with no accessibility service on), unless `always_focusable`.
#[kompozable]
pub(crate) fn popup_properties_for_anchor_type(
    anchor_type: ExposedDropdownMenuAnchorType,
    always_focusable: bool,
) -> PopupProperties {
    let a11y_services_enabled = remember_accessibility_service_state(true, true, true).get();

    // If typing on the IME is required, the menu should not be focusable
    // in order to prevent stealing focus from the input method.
    let ime_required = anchor_type == ExposedDropdownMenuAnchorType::PrimaryEditable
        || (anchor_type == ExposedDropdownMenuAnchorType::SecondaryEditable && !a11y_services_enabled);
    PopupProperties { focusable: !ime_required || always_focusable, ..PopupProperties::default() }
}
