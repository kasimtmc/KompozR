//! `Dialog` (Compose's Dialog.kt and AndroidDialog's DialogProperties): content over the whole window, centered,
//! taking the focus, keys and back while on top; the scrim is Material's.

use super::popup::emit_popup_root;
use super::{PopupKind, PopupSpec};
use crate::back::BackScope;
use crate::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope};
use crate::modifier::Modifier;
use crate::platform::{Insets, LocalDensity, LocalLayoutDirection, LocalSystemInsets};
use crate::unit::Constraints;
use kompoz_graphics::Dp;
use kompoz_runtime::{kompozable, remember, remember_keyed};
use std::rc::Rc;

/// What a dialog does (Compose's DialogProperties): whether back or a press outside its content dismisses it,
/// whether its content is kept the platform's way from the window's edges (48 dp at the sides, 24 at the top and
/// bottom, as C++'s), whether it keeps clear of the system bars (Android's `decorFitsSystemWindows`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DialogProperties {
    pub dismiss_on_back_press: bool,
    pub dismiss_on_click_outside: bool,
    pub use_platform_default_width: bool,
    pub decor_fits_system_windows: bool,
}

impl Default for DialogProperties {
    fn default() -> Self {
        DialogProperties {
            dismiss_on_back_press: true,
            dismiss_on_click_outside: true,
            use_platform_default_width: true,
            decor_fits_system_windows: true,
        }
    }
}

/// The platform's margins of a dialog's content (C++'s): at the sides, at the top and bottom.
const PLATFORM_MARGIN_X: Dp = Dp(48.0);
const PLATFORM_MARGIN_Y: Dp = Dp(24.0);

/// Content over the whole window, centered (Compose's `Dialog`): `on_dismiss_request` runs on back and on a press
/// outside the content, as `properties` say. Composed where it is called; its content has the IsDialog semantics
/// and its back handlers belong to it.
#[kompozable]
pub fn Dialog(
    on_dismiss_request: impl Fn() + 'static,
    #[default(DialogProperties::default())] properties: DialogProperties,
    content: impl Fn() + 'static,
) {
    let back_scope = remember(BackScope::new);
    let density = LocalDensity.current();
    let insets = LocalSystemInsets.current();
    let spec = Rc::new(PopupSpec {
        kind: PopupKind::Dialog { properties },
        on_dismiss_request: Rc::new(on_dismiss_request),
        back_scope: back_scope.clone(),
        layout_direction: LocalLayoutDirection.current(),
    });
    let mut margins = Insets::Zero;
    if properties.use_platform_default_width {
        let (x, y) = (density.round_to_px(PLATFORM_MARGIN_X), density.round_to_px(PLATFORM_MARGIN_Y));
        margins = Insets::new(x, y, x, y);
    }
    if properties.decor_fits_system_windows {
        for i in [insets.status_bars, insets.navigation_bars, insets.display_cutout] {
            margins.left = margins.left.max(i.left);
            margins.top = margins.top.max(i.top);
            margins.right = margins.right.max(i.right);
            margins.bottom = margins.bottom.max(i.bottom);
        }
    }
    let policy: Rc<dyn MeasurePolicy> =
        remember_keyed(margins, move || Rc::new(DialogRootPolicy { margins }) as Rc<dyn MeasurePolicy>);
    emit_popup_root(spec, Some(back_scope), policy, Modifier.semantics(|c| c.dialog()), content);
}

/// A dialog root's policy: the window's size, its content measured within the margins and centered in them.
pub(crate) struct DialogRootPolicy {
    pub(crate) margins: Insets,
}

impl MeasurePolicy for DialogRootPolicy {
    fn measure(&self, scope: &MeasureScope, measurables: &[&dyn Measurable], c: Constraints) -> MeasureResult {
        let m = self.margins;
        let width = (c.max_width - m.left - m.right).max(0);
        let height = (c.max_height - m.top - m.bottom).max(0);
        let inner = Constraints::new(0, width, 0, height);
        let placeables: Vec<_> = measurables.iter().map(|x| x.measure(inner)).collect();
        scope.layout(c.max_width, c.max_height, move |p| {
            for placeable in &placeables {
                let x = m.left + (width - placeable.width()) / 2;
                let y = m.top + (height - placeable.height()) / 2;
                p.place_relative(placeable, x, y);
            }
        })
    }
}
