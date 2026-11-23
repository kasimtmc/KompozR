//! The menus' platform parts, as Compose for desktop's actuals (SkikoMenu.skiko.kt, Menu.skiko.kt): the dropdown
//! menu ([`DropdownMenu`]) and the Expressive menu's popup ([`DropdownMenuPopup`]) — focusable popups under their
//! anchor, the arrow keys moving the focus between their items —, and the standard item ([`DropdownMenuItem`]).
//!
//! Compose for desktop's deprecated hidden overloads (`DropdownMenu(…, focusable, …)` and the ones without a scroll
//! state) can't be called from Kotlin and aren't ported.

use crate::internal::menu_position::DropdownMenuPositionProvider;
use crate::menu::{ClosedAlphaTarget, ClosedScaleTarget, MenuHorizontalMargin};
use crate::menu::{
    DefaultMenuProperties, DropdownMenuContent, DropdownMenuItemContent, ExpandedAlphaTarget, ExpandedScaleTarget,
    MenuItemColors, MenuVerticalMargin, calculate_transform_origin, menu_graphics_layer,
};
use crate::menu_defaults::MenuDefaults;
use crate::tokens::MotionSchemeKeyTokens;
use kompoz_animation::{MutableTransitionState, update_transition_of_state};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Dp, DpOffset, Shape};
use kompoz_runtime::{MutableState, kompozable, mutable_confined_state_of, mutable_state_of, remember, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::focus::{FocusDirection, FocusManager};
use kompoz_ui::input::{Key, KeyEvent, KeyEventType, KeyHandler};
use kompoz_ui::layout::TransformOrigin;
use kompoz_ui::platform::{
    InputMode, InputModeManager, LocalDensity, LocalFocusManager, LocalInputModeManager, LocalInspectionMode,
};
use kompoz_ui::window::{Popup, PopupPositionProvider, PopupProperties};
use std::rc::Rc;
use std::sync::Arc;

/// What the popup's key handler reads: the popup's focus manager and input mode manager, set as its content is
/// composed (Compose for desktop's two `var … by mutableStateOf(null)`; confined states: the managers aren't
/// `Send`).
#[derive(Clone)]
struct PopupManagers {
    focus_manager: MutableState<Option<FocusManager>>,
    input_mode_manager: MutableState<Option<InputModeManager>>,
}

impl PopupManagers {
    fn new() -> PopupManagers {
        PopupManagers {
            focus_manager: mutable_confined_state_of(None),
            input_mode_manager: mutable_confined_state_of(None),
        }
    }

    /// The managers as the popup's content was last composed with (Compose's reads as keys come).
    fn handle_key_event(&self, event: &KeyEvent) -> bool {
        let (focus_manager, input_mode_manager) = (self.focus_manager.get(), self.input_mode_manager.get());
        handle_dropdown_on_key_event(event, focus_manager.as_ref(), input_mode_manager.as_ref())
    }

    /// Set as the popup's content is composed.
    fn set_current(&self) {
        self.focus_manager.set(Some(LocalFocusManager.current()));
        self.input_mode_manager.set(Some(LocalInputModeManager.current()));
    }
}

/// A dropdown menu (Compose's `DropdownMenu`, Compose for desktop's actual): while `expanded` (and while it scales
/// and fades out after), a focusable popup — `properties`' — under its anchor (the layout it is composed in;
/// `offset` from there, mirrored right to left), else above it, else against the window's edges, 48 dp from the
/// window's top and bottom; on it, a surface of `shape` and `container_color` with its elevations and `border`,
/// scaling and fading in from where it meets its anchor, `content` (typically [`DropdownMenuItem`]s) in a column
/// as wide as its widest item, scrolled by `scroll_state`. `on_dismiss_request` runs on a press outside and on
/// back. Down and Up move the focus to the next and previous item.
///
/// Kotlin: `DropdownMenu(expanded: Boolean, onDismissRequest: () -> Unit, modifier: Modifier = Modifier, offset: DpOffset = DpOffset(0.dp, 0.dp), scrollState: ScrollState = rememberScrollState(), properties: PopupProperties = DefaultMenuProperties, shape: Shape = MenuDefaults.shape, containerColor: Color = MenuDefaults.containerColor, tonalElevation: Dp = MenuDefaults.TonalElevation, shadowElevation: Dp = MenuDefaults.ShadowElevation, border: BorderStroke? = null, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn DropdownMenu(
    expanded: bool,
    on_dismiss_request: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(DpOffset::new(Dp(0.0), Dp(0.0)))] offset: DpOffset,
    #[default(remember_scroll_state())] scroll_state: ScrollState,
    #[default(DefaultMenuProperties)] properties: PopupProperties,
    #[default(MenuDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(MenuDefaults::container_color())] container_color: Color,
    #[default(MenuDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(MenuDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(None)] border: Option<BorderStroke>,
    content: impl Fn() + 'static,
) {
    let expanded_state = remember(|| MutableTransitionState::new(false));
    expanded_state.set_target_state(expanded);

    if expanded_state.current_state() || expanded_state.target_state() {
        let transform_origin_state = remember(|| mutable_state_of(TransformOrigin::Center));
        let density = LocalDensity.current();
        let origin = transform_origin_state.clone();
        let popup_position_provider = remember_keyed((offset, density), move || {
            let origin = origin.clone();
            Rc::new(DropdownMenuPositionProvider::new(
                offset,
                density,
                density.round_to_px(MenuVerticalMargin),
                0,
                Rc::new(move |parent_bounds, menu_bounds| {
                    origin.set(calculate_transform_origin(parent_bounds, menu_bounds));
                }),
            )) as Rc<dyn PopupPositionProvider>
        });

        let managers = remember(PopupManagers::new);
        let keys = managers.clone();
        let content = Rc::new(content);
        Popup(
            on_dismiss_request = on_dismiss_request,
            popup_position_provider = Some(popup_position_provider),
            properties = properties,
            on_key_event = Some(Rc::new(move |event: &KeyEvent| keys.handle_key_event(event)) as KeyHandler),
            content = move || {
                managers.set_current();

                let content = content.clone();
                DropdownMenuContent(
                    modifier.clone(),
                    expanded_state.clone(),
                    transform_origin_state.clone(),
                    scroll_state.clone(),
                    shape.clone(),
                    container_color,
                    tonal_elevation,
                    shadow_elevation,
                    border.clone(),
                    move || content(),
                );
            },
        );
    }
}

/// The popup of an Expressive menu, for a menu of one's own (Compose's Expressive `DropdownMenuPopup`, Compose for
/// desktop's actual): as [`DropdownMenu`]'s popup (8 dp from the window's sides too), scaling and fading in and out
/// the same, but `content` (typically [`DropdownMenuGroup`](crate::DropdownMenuGroup)s) is in a column as wide as
/// its widest child, with `modifier`, on no surface.
///
/// Kotlin: `DropdownMenuPopup(expanded: Boolean, onDismissRequest: () -> Unit, modifier: Modifier = Modifier, offset: DpOffset = DpOffset(0.dp, 0.dp), properties: PopupProperties = DefaultMenuProperties, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
pub fn DropdownMenuPopup(
    expanded: bool,
    on_dismiss_request: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(DpOffset::new(Dp(0.0), Dp(0.0)))] offset: DpOffset,
    #[default(DefaultMenuProperties)] properties: PopupProperties,
    content: impl Fn() + 'static,
) {
    let expanded_state = remember(|| MutableTransitionState::new(false));
    expanded_state.set_target_state(expanded);

    if expanded_state.current_state() || expanded_state.target_state() {
        let transform_origin_state = remember(|| mutable_state_of(TransformOrigin::Center));
        let density = LocalDensity.current();
        let origin = transform_origin_state.clone();
        let popup_position_provider = remember_keyed((offset, density), move || {
            let origin = origin.clone();
            Rc::new(DropdownMenuPositionProvider::new(
                offset,
                density,
                density.round_to_px(MenuVerticalMargin),
                density.round_to_px(MenuHorizontalMargin),
                Rc::new(move |parent_bounds, menu_bounds| {
                    origin.set(calculate_transform_origin(parent_bounds, menu_bounds));
                }),
            )) as Rc<dyn PopupPositionProvider>
        });

        // Menu open/close animation.
        let transition = update_transition_of_state(expanded_state.clone(), Some("DropDownMenu".to_string()));
        let scale_animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
        let alpha_animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
        let scale = transition.animate_float(
            move |_| scale_animation_spec.clone(),
            "FloatAnimation".to_string(),
            |expanded: &bool| if *expanded { ExpandedScaleTarget } else { ClosedScaleTarget },
        );
        let alpha = transition.animate_float(
            move |_| alpha_animation_spec.clone(),
            "FloatAnimation".to_string(),
            |expanded: &bool| if *expanded { ExpandedAlphaTarget } else { ClosedAlphaTarget },
        );

        let is_inspecting = LocalInspectionMode.current();
        let managers = remember(PopupManagers::new);
        let keys = managers.clone();
        let content = Rc::new(content);
        Popup(
            on_dismiss_request = on_dismiss_request,
            popup_position_provider = Some(popup_position_provider),
            properties = properties,
            on_key_event = Some(Rc::new(move |event: &KeyEvent| keys.handle_key_event(event)) as KeyHandler),
            content = move || {
                managers.set_current();

                let layer = menu_graphics_layer(
                    is_inspecting,
                    expanded_state.clone(),
                    scale.clone(),
                    alpha.clone(),
                    transform_origin_state.clone(),
                );
                let content = content.clone();
                Column(
                    modifier = modifier.clone().width_intrinsic(IntrinsicSize::Max).graphics_layer_block(layer),
                    content = move || content(),
                );
            },
        );
    }
}

/// A standard menu item (Compose's `DropdownMenuItem(text, onClick, …)`, Compose for desktop's actual: the row
/// one): `text` between the icons (a trailing [`Text`](crate::Text) can show a keyboard shortcut), in `colors` for
/// its enabled state, padded by `content_padding`, as wide as the menu and at least 48 dp tall. `on_click` runs on a
/// click unless not `enabled`; its interactions go to `interaction_source` (one of its own where none).
///
/// Kotlin: `DropdownMenuItem(text: @Composable () -> Unit, onClick: () -> Unit, modifier: Modifier = Modifier, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, enabled: Boolean = true, colors: MenuItemColors = MenuDefaults.itemColors(), contentPadding: PaddingValues = MenuDefaults.DropdownMenuItemContentPadding, interactionSource: MutableInteractionSource? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn DropdownMenuItem(
    text: impl Fn() + 'static,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(true)] enabled: bool,
    #[default(MenuDefaults.item_colors())] colors: MenuItemColors,
    #[default(MenuDefaults::DropdownMenuItemContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    DropdownMenuItemContent(
        Rc::new(text),
        Rc::new(on_click),
        modifier,
        leading_icon,
        trailing_icon,
        enabled,
        colors,
        content_padding,
        interaction_source,
    );
}

/// A menu's keys (Compose for desktop's internal `handleDropdownOnKeyEvent`): Down and Up going down move the focus
/// to the next and previous item, the input mode turned to the keyboard's; taken. Others aren't.
pub(crate) fn handle_dropdown_on_key_event(
    key_event: &KeyEvent,
    focus_manager: Option<&FocusManager>,
    input_mode_manager: Option<&InputModeManager>,
) -> bool {
    if key_event.r#type == KeyEventType::KeyDown {
        match key_event.key {
            Key::DirectionDown => {
                if let Some(input_mode_manager) = input_mode_manager {
                    input_mode_manager.request_input_mode(InputMode::Keyboard);
                }
                if let Some(focus_manager) = focus_manager {
                    focus_manager.move_focus(FocusDirection::Next);
                }
                true
            }
            Key::DirectionUp => {
                if let Some(input_mode_manager) = input_mode_manager {
                    input_mode_manager.request_input_mode(InputMode::Keyboard);
                }
                if let Some(focus_manager) = focus_manager {
                    focus_manager.move_focus(FocusDirection::Previous);
                }
                true
            }
            _ => false,
        }
    } else {
        false
    }
}
