//! Material's modal bottom sheet, as Compose's ModalBottomSheet.kt with Compose for desktop's actual
//! (ModalBottomSheet.skiko.kt): a [`BottomSheet`] in a dialog over the whole window, over a [`Scrim`] dismissing it,
//! opening as it is composed; its properties ([`ModalBottomSheetProperties`], [`ModalBottomSheetDefaults`]) and
//! its state ([`remember_modal_bottom_sheet_state`], deprecated by material3 1.5.0-alpha20 for
//! [`remember_bottom_sheet_state`](crate::remember_bottom_sheet_state)).

// (the deprecated rememberModalBottomSheetState is defined here)
#![allow(deprecated)]

use crate::color_scheme::content_color_for;
use crate::internal::strings::{Strings, get_string};
use crate::scrim::Scrim;
use crate::sheet_defaults::{
    BottomSheet, BottomSheetDefaults, SheetState, SheetValue, default_drag_handle, launch_invoking_on_completion,
    remember_bottom_sheet_state, remember_sheet_state,
};
use crate::tokens::MotionSchemeKeyTokens;
use kompoz_animation::{AnimationSpec, animate_float_as_state};
use kompoz_coroutines::{LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Density, Dp, LayoutDirection, Shape};
use kompoz_runtime::kt::set_of;
use kompoz_runtime::{Memoized, derived_state_of, kompozable, named, named_args, remember, remember_keyed};
use kompoz_ui::window::{Dialog, DialogProperties};
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// A modal bottom sheet (Compose's `ModalBottomSheet`): an alternative to inline menus or simple dialogs, a
/// [`BottomSheet`] (of `modifier`, `sheet_state` — None: a remembered
/// [`remember_bottom_sheet_state`] hidden to start with, material3 1.5.0-alpha20's default —, `sheet_max_width`, `sheet_gestures_enabled`, `shape`,
/// `container_color`, `content_color` — None: the container's content color —, `tonal_elevation`, `drag_handle`,
/// `content_window_insets` and `content`) in a dialog over the whole window, at its top center, over a [`Scrim`]
/// of `scrim_color` faded in while the sheet goes anywhere but hidden. It shows itself once it has an expanded
/// anchor; a tap on the scrim (`properties`' dismissing on a click outside) hides it, back (their dismissing on
/// back) takes an expanded sheet with a half way stop there and hides it otherwise — `on_dismiss_request` called
/// once hidden (by back: whenever the hiding ends). Its top stays clear of the window insets its content pads by
/// only as far as the sheet reaches them.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ModalBottomSheet(
    on_dismiss_request: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] sheet_state: Option<SheetState>,
    #[default(BottomSheetDefaults::SheetMaxWidth)] sheet_max_width: Dp,
    #[default(true)] sheet_gestures_enabled: bool,
    #[default(BottomSheetDefaults::ExpandedShape())] shape: Arc<dyn Shape>,
    #[default(BottomSheetDefaults::ContainerColor())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(Dp(0.0))] tonal_elevation: Dp,
    #[default(BottomSheetDefaults::ScrimColor())] scrim_color: Color,
    #[default(default_drag_handle())] drag_handle: Option<Rc<dyn Fn()>>,
    #[default(|| BottomSheetDefaults::modal_window_insets())] content_window_insets: impl Fn() -> WindowInsets + 'static,
    #[default(ModalBottomSheetProperties::default())] properties: ModalBottomSheetProperties,
    content: impl Fn() + 'static,
) {
    let sheet_state = match sheet_state {
        Some(sheet_state) => sheet_state,
        None => remember_bottom_sheet_state(initial_value = SheetValue::Hidden),
    };
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let scope = remember_coroutine_scope();
    let on_dismiss_request: Rc<dyn Fn()> = Rc::new(on_dismiss_request);
    let animate_to_dismiss: Rc<dyn Fn()> = {
        let (sheet_state, scope, on_dismiss_request) = (sheet_state.clone(), scope.clone(), on_dismiss_request.clone());
        Rc::new(move || {
            if sheet_state.confirm_value_change(SheetValue::Hidden) {
                let (s, on_dismiss_request) = (sheet_state.clone(), on_dismiss_request.clone());
                launch_invoking_on_completion(&scope, sheet_state.hide(), move || {
                    if !s.is_visible() {
                        on_dismiss_request();
                    }
                });
            }
        })
    };

    let settle_to_dismiss: Rc<dyn Fn()> = {
        let (sheet_state, scope, on_dismiss_request) = (sheet_state.clone(), scope.clone(), on_dismiss_request.clone());
        Rc::new(move || {
            if sheet_state.current_value() == SheetValue::Expanded && sheet_state.has_partially_expanded_state() {
                // Smoothly animate away predictive back transformations since we are not fully dismissing. We
                // don't need to do this in the else below because we want to preserve the predictive back
                // transformations (scale) during the hide animation.
                let s = sheet_state.clone();
                named_args!(scope.launch(async move { s.partial_expand().await }));
            } else {
                // Is expanded without collapsed state or is collapsed.
                let on_dismiss_request = on_dismiss_request.clone();
                launch_invoking_on_completion(&scope, sheet_state.hide(), move || on_dismiss_request());
            }
        })
    };

    let dialog_state = sheet_state.clone();
    ModalBottomSheetDialog(
        on_dismiss_request = move || settle_to_dismiss(),
        content_color = Some(content_color),
        properties = properties,
        content = move || {
            let sheet_state = dialog_state.clone();
            Box(
                modifier = Modifier.fill_max_size().ime_padding().semantics_with(false, |c| {
                    c.set_is_traversal_group(true);
                }),
                content = || {
                    let insets_state = sheet_state.clone();
                    let sheet_window_insets = remember_keyed(sheet_state.clone(), move || {
                        WindowInsets::custom(SheetWindowInsets(insets_state))
                    });
                    let scrim_state = sheet_state.clone();
                    let is_scrim_visible =
                        remember(move || derived_state_of(move || scrim_state.target_value() != SheetValue::Hidden));
                    let scrim_alpha = animate_float_as_state(
                        target_value = if is_scrim_visible.get() { 1.0 } else { 0.0 },
                        animation_spec =
                            MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>,
                        label = "ScrimAlphaAnimation".to_string(),
                    );
                    Scrim(
                        content_description = Some(get_string(Strings::CloseSheet)),
                        on_click = if properties.should_dismiss_on_click_outside {
                            Some(animate_to_dismiss.clone())
                        } else {
                            None
                        },
                        alpha = Rc::new(move || scrim_alpha.get()) as Rc<dyn Fn() -> f32>,
                        color = scrim_color,
                    );
                    let on_dismiss_request = on_dismiss_request.clone();
                    BottomSheet(
                        modifier =
                            modifier.clone().align(Alignment::TopCenter).consume_window_insets(sheet_window_insets),
                        state = Some(sheet_state.clone()),
                        on_dismiss_request = move || on_dismiss_request(),
                        max_width = sheet_max_width,
                        gestures_enabled = sheet_gestures_enabled,
                        back_handler_enabled = properties.should_dismiss_on_back_press,
                        shape = shape.clone(),
                        container_color = container_color,
                        content_color = Some(content_color),
                        tonal_elevation = tonal_elevation,
                        drag_handle = drag_handle.clone(),
                        content_window_insets = || content_window_insets(),
                        content = || content(),
                    );
                },
            );
        },
    );
    if sheet_state.has_expanded_state() {
        let s = sheet_state.clone();
        LaunchedEffect(sheet_state, async move { s.show().await });
    }
}

/// What a modal bottom sheet does (Compose's `ModalBottomSheetProperties`, Compose for desktop's actual): whether
/// back dismisses it (`should_dismiss_on_back_press`) and a click outside it, on its scrim
/// (`should_dismiss_on_click_outside`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ModalBottomSheetProperties {
    pub should_dismiss_on_back_press: bool,
    pub should_dismiss_on_click_outside: bool,
}

impl Default for ModalBottomSheetProperties {
    fn default() -> Self {
        ModalBottomSheetProperties { should_dismiss_on_back_press: true, should_dismiss_on_click_outside: true }
    }
}

/// Compose's `ModalBottomSheetProperties(shouldDismissOnBackPress = true, shouldDismissOnClickOutside = true)`.
#[named(type)]
pub fn ModalBottomSheetProperties(
    #[default(true)] should_dismiss_on_back_press: bool,
    #[default(true)] should_dismiss_on_click_outside: bool,
) -> ModalBottomSheetProperties {
    ModalBottomSheetProperties { should_dismiss_on_back_press, should_dismiss_on_click_outside }
}

/// The defaults of [`ModalBottomSheet`] (Compose's `object ModalBottomSheetDefaults`).
pub struct ModalBottomSheetDefaults;

impl ModalBottomSheetDefaults {
    /// The properties a modal bottom sheet has by default (Compose's `properties`): dismissed by back and by a
    /// click outside.
    pub fn properties() -> ModalBottomSheetProperties {
        ModalBottomSheetProperties::default()
    }
}

/// A modal bottom sheet's state, hidden to start with, saved and restored (Compose's
/// `rememberModalBottomSheetState`, deprecated by material3 1.5.0-alpha20): `skip_partially_expanded`, it never
/// stops half way; `confirm_value_change` may veto a change (a new closure, by what it captures, makes a new state,
/// as a new key does in Compose). [`remember_bottom_sheet_state`] of `Hidden` and the values it doesn't skip.
#[deprecated(note = "Use rememberBottomSheetState with Hidden initial value")]
#[kompozable]
pub fn remember_modal_bottom_sheet_state(
    #[default(false)] skip_partially_expanded: bool,
    #[default(|_| true)]
    #[memoized]
    confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
) -> SheetState {
    use SheetValue::{Expanded, Hidden, PartiallyExpanded};
    let enabled_values = if skip_partially_expanded {
        set_of([Hidden, Expanded])
    } else {
        set_of([Hidden, PartiallyExpanded, Expanded])
    };
    // (Compose: `rememberBottomSheetState(…)`, rememberSheetState of the same arguments: here of the closure's
    // identity as this function's memoized parameter carries it)
    let id = confirm_value_change.id();
    let confirm: Rc<dyn Fn(SheetValue) -> bool> = Rc::new(confirm_value_change.into_value());
    remember_sheet_state(
        enabled_values = enabled_values,
        confirm_value_change = Memoized::new(confirm, id),
        initial_value = Hidden,
    )
}

/// The insets a modal sheet consumes at its top: as far down as the sheet is (Compose's internal
/// `SheetWindowInsets`), so its content's top insets pad it only where the sheet reaches into them.
pub(crate) struct SheetWindowInsets(pub(crate) SheetState);

impl CustomWindowInsets for SheetWindowInsets {
    fn get_left(&self, _density: &Density, _layout_direction: LayoutDirection) -> i32 {
        0
    }

    fn get_top(&self, _density: &Density) -> i32 {
        let offset = self.0.anchored_draggable_state().offset();
        if offset.is_nan() { 0 } else { (offset as i32).max(0) }
    }

    fn get_right(&self, _density: &Density, _layout_direction: LayoutDirection) -> i32 {
        0
    }

    fn get_bottom(&self, _density: &Density) -> i32 {
        0
    }
}

/// The dialog a modal bottom sheet is shown in (Compose's internal `ModalBottomSheetDialog`, Compose for desktop's
/// actual): ui's [`Dialog`] of `properties`' dismissals, not limited to the platform's default width or insets
/// (nor to the software keyboard's), of no scrim and no transition — KompozR's [`Dialog`] has no scrim,
/// transition or keyboard inset of its own, and limits its content to the system's insets only where
/// `decor_fits_system_windows` (Compose for desktop's `usePlatformInsets`): false here. `content_color` isn't used
/// (as Compose for desktop's actual, CMP-7147).
#[kompozable]
pub(crate) fn ModalBottomSheetDialog(
    #[default(|| {})] on_dismiss_request: impl Fn() + 'static,
    #[default(None)] content_color: Option<Color>,
    #[default(ModalBottomSheetProperties::default())] properties: ModalBottomSheetProperties,
    content: impl Fn() + 'static,
) {
    // (Kotlin's default `contentColorFor(BottomSheetDefaults.ContainerColor)`, unused by the desktop's actual)
    let _content_color = content_color.unwrap_or_else(|| content_color_for(BottomSheetDefaults::ContainerColor()));
    Dialog(
        on_dismiss_request = on_dismiss_request,
        properties = DialogProperties {
            dismiss_on_back_press: properties.should_dismiss_on_back_press,
            dismiss_on_click_outside: properties.should_dismiss_on_click_outside,
            use_platform_default_width: false,
            // (Compose for desktop's `usePlatformInsets = false`)
            decor_fits_system_windows: false,
        },
        content = content,
    );
}
