//! Material's exposed dropdown menu, as Compose's ExposedDropdownMenu.kt (its common code; Compose for desktop's
//! actuals are in `exposed_dropdown_menu_desktop.rs`): a text field showing the choice of a menu anchored to it —
//! read only (a "spinner") or edited (an autocomplete) — in [`ExposedDropdownMenuBox`], whose scope
//! ([`ExposedDropdownMenuBoxScope`]) makes the field the menu's anchor
//! ([`menu_anchor`](ExposedDropdownMenuBoxScope::menu_anchor), of an [`ExposedDropdownMenuAnchorType`]) and shows
//! the menu under or above it ([`ExposedDropdownMenu`](ExposedDropdownMenuBoxScope::ExposedDropdownMenu)) as wide
//! as the field and no taller than the window leaves; [`ExposedDropdownMenuDefaults`]' trailing icon, field colors
//! and item padding.
//!
//! Compose's overloads kept hidden for binary compatibility can't be called from Kotlin and aren't ported.

// (the deprecated items' own generated items name them)
#![allow(deprecated)]

use crate::exposed_dropdown_menu_desktop::{
    OnPlatformWindowBoundsChange, platform_window_bounds_calculator, popup_properties_for_anchor_type,
};
use crate::icon::VectorIcon;
use crate::internal::back_handler::BackHandler;
use crate::internal::icons::Icons;
use crate::internal::menu_position::{MenuPosition, MenuPositionHorizontal, MenuPositionVertical};
use crate::internal::strings::{Strings, get_string};
use crate::menu::{DropdownMenuContent, MenuVerticalMargin, calculate_transform_origin};
use crate::menu_defaults::MenuDefaults;
use crate::text_field_defaults::{OutlinedTextFieldDefaults, TextFieldColors, TextFieldDefaults};
use crate::tokens::{FilledAutocompleteTokens, OutlinedAutocompleteTokens};
use kompoz_animation::MutableTransitionState;
use kompoz_foundation::gestures::{AwaitPointerEventScopeGestures, PointerInputScopeGestures};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Density, Dp, IntOffset, IntRect, IntSize, LayoutDirection, Rect, Shape};
use kompoz_runtime::{
    MutableState, SideEffect, State, kompozable, kompozable_block, mutable_state_of, mutable_state_of_with_policy,
    named, named_args, never_equal_policy, remember, remember_keyed,
};
use kompoz_ui::focus::{FocusDirection, FocusRequester};
use kompoz_ui::input::{Key, KeyEvent, KeyEventType, PointerEventPass, PointerInputScope};
use kompoz_ui::layout::{LayoutCoordinates, TransformOrigin};
use kompoz_ui::platform::{LocalDensity, LocalSoftwareKeyboardController, SoftwareKeyboardController};
use kompoz_ui::semantics::Role;
use kompoz_ui::window::{Popup, PopupPositionProvider};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// An exposed dropdown menu's box (Compose's `ExposedDropdownMenuBox`): `content` — typically a text field given
/// [`menu_anchor`](ExposedDropdownMenuBoxScope::menu_anchor) and an
/// [`ExposedDropdownMenu`](ExposedDropdownMenuBoxScope::ExposedDropdownMenu) — in a box with `modifier`, in the
/// box's scope. A click on the anchor calls `on_expanded_change` with the other state; while `expanded` the anchor
/// is focused, back (where the menu doesn't take it) collapses it, and the menu's height is kept to what the
/// window leaves above or below the box (48 dp from its edges), followed as the window's size changes.
///
/// Kotlin: `ExposedDropdownMenuBox(expanded: Boolean, onExpandedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, content: @Composable ExposedDropdownMenuBoxScope.() -> Unit)`
#[kompozable]
pub fn ExposedDropdownMenuBox(
    expanded: bool,
    on_expanded_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn(&ExposedDropdownMenuBoxScope) + 'static,
) {
    let window_bounds_calculator = platform_window_bounds_calculator();
    let density = LocalDensity.current();

    let vertical_margin = density.round_to_px(MenuVerticalMargin);

    // (Compose's `mutableStateOf<LayoutCoordinates?>`: read only by the callbacks, so a cell)
    let anchor_coordinates: Rc<RefCell<Option<LayoutCoordinates>>> = remember(|| Rc::new(RefCell::new(None)));
    let anchor_width = remember(|| mutable_state_of(0));
    let menu_max_height = remember(|| mutable_state_of(0));

    let focus_requester = remember(FocusRequester::new);
    let keyboard_controller = LocalSoftwareKeyboardController.current();
    let expanded_description = get_string(Strings::MenuExpanded);
    let collapsed_description = get_string(Strings::MenuCollapsed);
    let toggle_description = get_string(Strings::ToggleDropdownMenu);
    let anchor_type_state = remember(|| mutable_state_of(ExposedDropdownMenuAnchorType::PrimaryNotEditable));
    let always_focusable = remember(|| mutable_state_of(false));

    let on_expanded_change: Rc<dyn Fn(bool)> = Rc::new(on_expanded_change);
    let scope = ExposedDropdownMenuBoxScope {
        inner: Rc::new(ScopeImpl {
            expanded,
            on_expanded_change: on_expanded_change.clone(),
            focus_requester: focus_requester.clone(),
            anchor_type_state,
            always_focusable,
            anchor_width: anchor_width.clone(),
            menu_max_height: menu_max_height.clone(),
            expanded_description,
            collapsed_description,
            toggle_description,
            keyboard_controller,
        }),
    };

    let (coordinates, width, max_height, calculator) =
        (anchor_coordinates.clone(), anchor_width.clone(), menu_max_height.clone(), window_bounds_calculator.clone());
    Box(
        modifier = modifier.on_globally_positioned(move |it: &LayoutCoordinates| {
            *coordinates.borrow_mut() = Some(it.clone());
            width.set(it.size().width);
            max_height.set(calculate_max_height(
                calculator.get_visible_window_bounds(),
                Some(get_anchor_bounds(coordinates.borrow().as_ref())),
                vertical_margin,
            ));
        }),
        content = move || content(&scope),
    );

    if expanded {
        let (coordinates, max_height) = (anchor_coordinates.clone(), menu_max_height.clone());
        OnPlatformWindowBoundsChange(move || {
            max_height.set(calculate_max_height(
                window_bounds_calculator.get_visible_window_bounds(),
                Some(get_anchor_bounds(coordinates.borrow().as_ref())),
                vertical_margin,
            ));
        });
    }

    SideEffect(move || {
        if expanded {
            focus_requester.request_focus_with(FocusDirection::Enter);
        }
    });

    // Back events are handled in the Popup layer if the menu is focusable.
    // If it's not focusable, we handle them here.
    BackHandler(expanded, move || on_expanded_change(false));
}

/// What a box's scope reads (Compose's anonymous `ExposedDropdownMenuBoxScopeImpl`).
struct ScopeImpl {
    expanded: bool,
    on_expanded_change: Rc<dyn Fn(bool)>,
    focus_requester: FocusRequester,
    anchor_type_state: MutableState<ExposedDropdownMenuAnchorType>,
    always_focusable: MutableState<bool>,
    anchor_width: MutableState<i32>,
    menu_max_height: MutableState<i32>,
    expanded_description: String,
    collapsed_description: String,
    toggle_description: String,
    keyboard_controller: SoftwareKeyboardController,
}

/// The scope of an [`ExposedDropdownMenuBox`]'s content (Compose's sealed class `ExposedDropdownMenuBoxScope`): the
/// anchor's modifier, the menu's size, the menu. Made by the box as it is composed (Compose remembers it while
/// what it closes over stays the same; here a new one, of the same states, each time).
#[derive(Clone)]
pub struct ExposedDropdownMenuBoxScope {
    inner: Rc<ScopeImpl>,
}

#[named]
impl ExposedDropdownMenuBoxScope {
    /// `modifier` the menu's anchor, typically a text field or an icon in it (Compose's `Modifier.menuAnchor(type,
    /// enabled)`): it is focused while the menu is expanded, and, if `enabled`, a click on it (a press seen before
    /// the field takes it; Enter, or Space unless an editable primary anchor, from the keyboard) expands or
    /// collapses the menu, and it tells accessibility it is a dropdown list (a secondary editable anchor: a button
    /// toggling the menu, with its state). Its `type` decides whether the menu takes the focus (see
    /// [`ExposedDropdownMenuAnchorType`]). `enabled` is only about the menu: not the field's own `enabled`.
    ///
    /// Kotlin: `menuAnchor(type: ExposedDropdownMenuAnchorType, enabled: Boolean = true): Modifier`
    pub fn menu_anchor(
        &self,
        modifier: Modifier,
        r#type: ExposedDropdownMenuAnchorType,
        #[default(true)] enabled: bool,
    ) -> Modifier {
        let inner = &self.inner;
        let anchor_type_state = inner.anchor_type_state.clone();
        let on_attach_state = anchor_type_state.clone();
        let anchored =
            modifier.focus_requester(&inner.focus_requester).then_element(ExposedDropdownMenuAnchorElement {
                update_state_on_attach: Rc::new(move || {
                    if r#type.has_greater_or_equal_priority_than(on_attach_state.peek()) {
                        on_attach_state.set(r#type);
                    }
                }),
            });
        if !enabled {
            return anchored;
        }
        let (expanded, on_expanded_change) = (inner.expanded, inner.on_expanded_change.clone());
        expandable(
            anchored,
            inner.expanded,
            Rc::new(move || {
                anchor_type_state.set(r#type);
                on_expanded_change(!expanded);
            }),
            r#type,
            inner.always_focusable.clone(),
            inner.expanded_description.clone(),
            inner.collapsed_description.clone(),
            inner.toggle_description.clone(),
            Some(inner.keyboard_controller.clone()),
        )
    }

    /// Compose's deprecated `Modifier.menuAnchor()`: [`menu_anchor`](Self::menu_anchor) of a primary anchor that
    /// isn't edited.
    ///
    /// Kotlin: `menuAnchor(): Modifier`
    #[deprecated(note = "Use overload that takes ExposedDropdownMenuAnchorType and enabled parameters")]
    pub fn menu_anchor_untyped(&self, modifier: Modifier) -> Modifier {
        self.menu_anchor(modifier, ExposedDropdownMenuAnchorType::PrimaryNotEditable)
    }

    /// `modifier` sized as the box's menu (Compose's `Modifier.exposedDropdownSize(matchAnchorWidth)`): no taller
    /// than the window leaves above or below the anchor (so it covers neither the field nor the keyboard), as wide
    /// as the anchor if `match_anchor_width`. The [`ExposedDropdownMenu`](Self::ExposedDropdownMenu) has it: for a
    /// menu of one's own.
    ///
    /// Kotlin: `exposedDropdownSize(matchAnchorWidth: Boolean = true): Modifier`
    pub fn exposed_dropdown_size(&self, modifier: Modifier, #[default(true)] match_anchor_width: bool) -> Modifier {
        let (anchor_width, menu_max_height) = (self.inner.anchor_width.clone(), self.inner.menu_max_height.clone());
        modifier.layout(move |scope, measurable, constraints| {
            let menu_width = constraints.constrain_width(anchor_width.get());
            let menu_constraints = named_args!(constraints.copy(
                max_height = constraints.constrain_height(menu_max_height.get()),
                min_width = if match_anchor_width { menu_width } else { constraints.min_width },
                max_width = if match_anchor_width { menu_width } else { constraints.max_width },
            ));
            let placeable = measurable.measure(menu_constraints);
            scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
        })
    }

    /// The anchor's type of the menu (Compose's internal `anchorType`: the latest anchor clicked, else the one of
    /// the highest priority attached; a state read).
    pub(crate) fn anchor_type(&self) -> ExposedDropdownMenuAnchorType {
        self.inner.anchor_type_state.get()
    }

    /// Whether the menu takes the focus whatever the anchor (Compose's internal `alwaysFocusable`: the keyboard
    /// went for it from an editable anchor; a state read).
    pub(crate) fn always_focusable(&self) -> bool {
        self.inner.always_focusable.get()
    }

    /// The box's menu (Compose's composable `ExposedDropdownMenu`): while `expanded` (and while it scales and fades
    /// out after), a popup under the box (else above it, else against the window's edges, 48 dp from its top and
    /// bottom) — taking the focus unless the anchor is edited (the keyboard stays with the field) — on which a
    /// surface of `shape` and `container_color`, with its elevations and `border`, holds `content` (typically
    /// [`DropdownMenuItem`](crate::DropdownMenuItem)s) in a column scrolled by `scroll_state` (`None`: one
    /// remembered, Compose's `rememberScrollState()`), sized by
    /// [`exposed_dropdown_size`](Self::exposed_dropdown_size)`(match_anchor_width)`, with `modifier`.
    /// `on_dismiss_request` runs on a press outside and on back. Composable.
    ///
    /// Kotlin: `ExposedDropdownMenu(expanded: Boolean, onDismissRequest: () -> Unit, modifier: Modifier = Modifier, scrollState: ScrollState = rememberScrollState(), matchAnchorWidth: Boolean = true, shape: Shape = MenuDefaults.shape, containerColor: Color = MenuDefaults.containerColor, tonalElevation: Dp = MenuDefaults.TonalElevation, shadowElevation: Dp = MenuDefaults.ShadowElevation, border: BorderStroke? = null, content: @Composable ColumnScope.() -> Unit)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn ExposedDropdownMenu(
        &self,
        expanded: bool,
        on_dismiss_request: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(None)] scroll_state: Option<ScrollState>,
        #[default(true)] match_anchor_width: bool,
        #[default(MenuDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(MenuDefaults::container_color())] container_color: Color,
        #[default(MenuDefaults::TonalElevation)] tonal_elevation: Dp,
        #[default(MenuDefaults::ShadowElevation)] shadow_elevation: Dp,
        #[default(None)] border: Option<BorderStroke>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            ExposedDropdownMenuImpl(
                self.clone(),
                expanded,
                Rc::new(on_dismiss_request),
                modifier,
                scroll_state,
                match_anchor_width,
                shape,
                container_color,
                tonal_elevation,
                shadow_elevation,
                border,
                Rc::new(content),
            );
        });
    }

    /// Compose's deprecated `ExposedDropdownMenu(…, focusable, matchTextFieldWidth, …)`: `focusable` is unused (the
    /// anchor's type decides), `match_text_field_width` is
    /// [`ExposedDropdownMenu`](Self::ExposedDropdownMenu)'s `match_anchor_width`. Composable.
    ///
    /// Kotlin: `ExposedDropdownMenu(expanded: Boolean, onDismissRequest: () -> Unit, modifier: Modifier = Modifier, scrollState: ScrollState = rememberScrollState(), focusable: Boolean = true, matchTextFieldWidth: Boolean = true, shape: Shape = MenuDefaults.shape, containerColor: Color = MenuDefaults.containerColor, tonalElevation: Dp = MenuDefaults.TonalElevation, shadowElevation: Dp = MenuDefaults.ShadowElevation, border: BorderStroke? = null, content: @Composable ColumnScope.() -> Unit)`
    #[deprecated(
        note = "The `focusable` parameter is unused. Pass the proper ExposedDropdownMenuAnchorType to Modifier.menuAnchor instead, which will handle focusability automatically."
    )]
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn FocusableExposedDropdownMenu(
        &self,
        expanded: bool,
        on_dismiss_request: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(None)] scroll_state: Option<ScrollState>,
        #[default(true)] focusable: bool,
        #[default(true)] match_text_field_width: bool,
        #[default(MenuDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(MenuDefaults::container_color())] container_color: Color,
        #[default(MenuDefaults::TonalElevation)] tonal_elevation: Dp,
        #[default(MenuDefaults::ShadowElevation)] shadow_elevation: Dp,
        #[default(None)] border: Option<BorderStroke>,
        content: impl Fn() + 'static,
    ) {
        let _ = focusable;
        kompozable_block!({
            ExposedDropdownMenuImpl(
                self.clone(),
                expanded,
                Rc::new(on_dismiss_request),
                modifier,
                scroll_state,
                match_text_field_width,
                shape,
                container_color,
                tonal_elevation,
                shadow_elevation,
                border,
                Rc::new(content),
            );
        });
    }
}

/// The body of [`ExposedDropdownMenuBoxScope::ExposedDropdownMenu`] (Compose's: a member of the scope there; a
/// composable of its own here, restartable as Compose's).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ExposedDropdownMenuImpl(
    scope: ExposedDropdownMenuBoxScope,
    expanded: bool,
    on_dismiss_request: Rc<dyn Fn()>,
    modifier: Modifier,
    scroll_state: Option<ScrollState>,
    match_anchor_width: bool,
    shape: Arc<dyn Shape>,
    container_color: Color,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    border: Option<BorderStroke>,
    content: Rc<dyn Fn()>,
) {
    let scroll_state = scroll_state.unwrap_or_else(|| remember_scroll_state());
    // Workaround for b/326394521. We create a state that's read in `calculatePosition`.
    // Then trigger a state change in `OnPlatformWindowBoundsChange` to force recalculation.
    let keyboard_signal_state = remember(|| mutable_state_of_with_policy((), never_equal_policy()));
    let density = LocalDensity.current();
    let top_window_insets = WindowInsets::status_bars().get_top(&density);

    if expanded {
        let signal = keyboard_signal_state.clone();
        OnPlatformWindowBoundsChange(move || signal.set(()));
    }

    // (Compose's b/326064777: a DropdownMenu once it takes a position provider of one's own)
    let expanded_state = remember(|| MutableTransitionState::new(false));
    expanded_state.set_target_state(expanded);

    if expanded_state.current_state() || expanded_state.target_state() {
        let transform_origin_state = remember(|| mutable_state_of(TransformOrigin::Center));
        let (origin, signal) = (transform_origin_state.clone(), keyboard_signal_state.clone());
        let popup_position_provider = remember_keyed((density, top_window_insets), move || {
            let origin = origin.clone();
            Rc::new(ExposedDropdownMenuPositionProvider::new(
                density,
                top_window_insets,
                Some(State::from(signal.clone())),
                density.round_to_px(MenuVerticalMargin),
                Rc::new(move |anchor_bounds, menu_bounds| {
                    origin.set(calculate_transform_origin(anchor_bounds, menu_bounds));
                }),
            )) as Rc<dyn PopupPositionProvider>
        });

        let properties = popup_properties_for_anchor_type(scope.anchor_type(), scope.always_focusable());
        Popup(
            on_dismiss_request = move || on_dismiss_request(),
            popup_position_provider = Some(popup_position_provider),
            properties = properties,
            content = move || {
                let content = content.clone();
                DropdownMenuContent(
                    named_args!(scope.exposed_dropdown_size(modifier.clone(), match_anchor_width = match_anchor_width)),
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

/// The type of element that can serve as a dropdown menu's anchor (Compose's value class
/// `ExposedDropdownMenuAnchorType`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExposedDropdownMenuAnchorType(&'static str);

impl ExposedDropdownMenuAnchorType {
    /// A primary anchor that isn't edited, such as a read-only text field: it opens the menu with the focus.
    pub const PrimaryNotEditable: ExposedDropdownMenuAnchorType = ExposedDropdownMenuAnchorType("PrimaryNotEditable");

    /// A primary anchor that is edited, such as a text field taking the user's input: it opens the menu without
    /// the focus, which stays with the soft keyboard (the input method).
    pub const PrimaryEditable: ExposedDropdownMenuAnchorType = ExposedDropdownMenuAnchorType("PrimaryEditable");

    /// A secondary anchor beside an edited primary one, such as an icon in an edited text field: with
    /// accessibility services on, it opens the menu with the focus; else without, the focus staying with the soft
    /// keyboard.
    pub const SecondaryEditable: ExposedDropdownMenuAnchorType = ExposedDropdownMenuAnchorType("SecondaryEditable");

    /// Whether this type may take the menu from `that` (Compose's private `hasGreaterOrEqualPriorityThan`): a
    /// primary one always, a secondary one only from another secondary one.
    fn has_greater_or_equal_priority_than(self, that: ExposedDropdownMenuAnchorType) -> bool {
        match self {
            ExposedDropdownMenuAnchorType::PrimaryNotEditable | ExposedDropdownMenuAnchorType::PrimaryEditable => true,
            ExposedDropdownMenuAnchorType::SecondaryEditable => {
                that == ExposedDropdownMenuAnchorType::SecondaryEditable
            }
            _ => false,
        }
    }
}

/// Compose's toString: the type's name.
impl fmt::Display for ExposedDropdownMenuAnchorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Debug for ExposedDropdownMenuAnchorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Compose's deprecated `MenuAnchorType`, renamed [`ExposedDropdownMenuAnchorType`].
#[deprecated(note = "Renamed to ExposedDropdownMenuAnchorType")]
pub type MenuAnchorType = ExposedDropdownMenuAnchorType;

/// The exposed dropdown menu's defaults (Compose's `object ExposedDropdownMenuDefaults`).
pub struct ExposedDropdownMenuDefaults;

#[named]
impl ExposedDropdownMenuDefaults {
    /// The menu's trailing icon (Compose's composable `TrailingIcon`): the filled drop down arrow, turned up while
    /// `expanded`; no content description.
    ///
    /// Kotlin: `TrailingIcon(expanded: Boolean, modifier: Modifier = Modifier)`
    #[track_caller]
    pub fn TrailingIcon(&self, expanded: bool, #[default(Modifier)] modifier: Modifier) {
        kompozable_block!({
            VectorIcon(Icons::Filled::ArrowDropDown(), None, modifier.rotate(if expanded { 180.0 } else { 0.0 }));
        });
    }

    /// The colors of a [`TextField`](crate::TextField) in an [`ExposedDropdownMenuBox`] (Compose's composable
    /// `textFieldColors(…)`): the filled autocomplete field's, what is given replacing them; through
    /// [`TextFieldDefaults`]`.colors(…)`.
    ///
    /// Kotlin: `textFieldColors(focusedTextColor: Color = FilledAutocompleteTokens.FieldFocusInputTextColor.value): TextFieldColors`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn text_field_colors(
        &self,
        #[default(FilledAutocompleteTokens::FieldFocusInputTextColor.value())] focused_text_color: Color,
        #[default(FilledAutocompleteTokens::FieldInputTextColor.value())] unfocused_text_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::FieldDisabledInputTextColor.value(), FilledAutocompleteTokens::FieldDisabledInputTextOpacity))]
        disabled_text_color: Color,
        #[default(FilledAutocompleteTokens::FieldErrorInputTextColor.value())] error_text_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldContainerColor.value())] focused_container_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldContainerColor.value())] unfocused_container_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldContainerColor.value())] disabled_container_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldContainerColor.value())] error_container_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldCaretColor.value())] cursor_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldErrorFocusCaretColor.value())] error_cursor_color: Color,
        #[default(LocalTextSelectionColors.current())] selection_colors: TextSelectionColors,
        #[default(FilledAutocompleteTokens::TextFieldFocusActiveIndicatorColor.value())] focused_indicator_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldActiveIndicatorColor.value())] unfocused_indicator_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::TextFieldDisabledActiveIndicatorColor.value(), FilledAutocompleteTokens::TextFieldDisabledActiveIndicatorOpacity))]
        disabled_indicator_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldErrorActiveIndicatorColor.value())] error_indicator_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldFocusLeadingIconColor.value())] focused_leading_icon_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldLeadingIconColor.value())] unfocused_leading_icon_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::TextFieldDisabledLeadingIconColor.value(), FilledAutocompleteTokens::TextFieldDisabledLeadingIconOpacity))]
        disabled_leading_icon_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldErrorLeadingIconColor.value())] error_leading_icon_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldFocusTrailingIconColor.value())]
        focused_trailing_icon_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldTrailingIconColor.value())] unfocused_trailing_icon_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::TextFieldDisabledTrailingIconColor.value(), FilledAutocompleteTokens::TextFieldDisabledTrailingIconOpacity))]
        disabled_trailing_icon_color: Color,
        #[default(FilledAutocompleteTokens::TextFieldErrorTrailingIconColor.value())] error_trailing_icon_color: Color,
        #[default(FilledAutocompleteTokens::FieldFocusLabelTextColor.value())] focused_label_color: Color,
        #[default(FilledAutocompleteTokens::FieldLabelTextColor.value())] unfocused_label_color: Color,
        #[default(FilledAutocompleteTokens::FieldDisabledLabelTextColor.value())] disabled_label_color: Color,
        #[default(FilledAutocompleteTokens::FieldErrorLabelTextColor.value())] error_label_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] focused_placeholder_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] unfocused_placeholder_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::FieldDisabledSupportingTextColor.value(), FilledAutocompleteTokens::FieldDisabledSupportingTextOpacity))]
        disabled_placeholder_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] error_placeholder_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] focused_prefix_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] unfocused_prefix_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::FieldDisabledSupportingTextColor.value(), FilledAutocompleteTokens::FieldDisabledSupportingTextOpacity))]
        disabled_prefix_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] error_prefix_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] focused_suffix_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] unfocused_suffix_color: Color,
        #[default(with_alpha(FilledAutocompleteTokens::FieldDisabledSupportingTextColor.value(), FilledAutocompleteTokens::FieldDisabledSupportingTextOpacity))]
        disabled_suffix_color: Color,
        #[default(FilledAutocompleteTokens::FieldSupportingTextColor.value())] error_suffix_color: Color,
    ) -> TextFieldColors {
        named_args!(TextFieldDefaults.colors(
            focused_text_color = focused_text_color,
            unfocused_text_color = unfocused_text_color,
            disabled_text_color = disabled_text_color,
            error_text_color = error_text_color,
            focused_container_color = focused_container_color,
            unfocused_container_color = unfocused_container_color,
            disabled_container_color = disabled_container_color,
            error_container_color = error_container_color,
            cursor_color = cursor_color,
            error_cursor_color = error_cursor_color,
            selection_colors = Some(selection_colors),
            focused_indicator_color = focused_indicator_color,
            unfocused_indicator_color = unfocused_indicator_color,
            disabled_indicator_color = disabled_indicator_color,
            error_indicator_color = error_indicator_color,
            focused_leading_icon_color = focused_leading_icon_color,
            unfocused_leading_icon_color = unfocused_leading_icon_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            error_leading_icon_color = error_leading_icon_color,
            focused_trailing_icon_color = focused_trailing_icon_color,
            unfocused_trailing_icon_color = unfocused_trailing_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            error_trailing_icon_color = error_trailing_icon_color,
            focused_label_color = focused_label_color,
            unfocused_label_color = unfocused_label_color,
            disabled_label_color = disabled_label_color,
            error_label_color = error_label_color,
            focused_placeholder_color = focused_placeholder_color,
            unfocused_placeholder_color = unfocused_placeholder_color,
            disabled_placeholder_color = disabled_placeholder_color,
            error_placeholder_color = error_placeholder_color,
            focused_prefix_color = focused_prefix_color,
            unfocused_prefix_color = unfocused_prefix_color,
            disabled_prefix_color = disabled_prefix_color,
            error_prefix_color = error_prefix_color,
            focused_suffix_color = focused_suffix_color,
            unfocused_suffix_color = unfocused_suffix_color,
            disabled_suffix_color = disabled_suffix_color,
            error_suffix_color = error_suffix_color,
        ))
    }

    /// The colors of an [`OutlinedTextField`](crate::OutlinedTextField) in an [`ExposedDropdownMenuBox`]
    /// (Compose's composable `outlinedTextFieldColors(…)`): the outlined autocomplete field's, no container, what
    /// is given replacing them; through [`OutlinedTextFieldDefaults`]`.colors(…)`.
    ///
    /// Kotlin: `outlinedTextFieldColors(focusedTextColor: Color = OutlinedAutocompleteTokens.FieldFocusInputTextColor.value): TextFieldColors`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn outlined_text_field_colors(
        &self,
        #[default(OutlinedAutocompleteTokens::FieldFocusInputTextColor.value())] focused_text_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldInputTextColor.value())] unfocused_text_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::FieldDisabledInputTextColor.value(), OutlinedAutocompleteTokens::FieldDisabledInputTextOpacity))]
        disabled_text_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldErrorInputTextColor.value())] error_text_color: Color,
        #[default(Color::Transparent)] focused_container_color: Color,
        #[default(Color::Transparent)] unfocused_container_color: Color,
        #[default(Color::Transparent)] disabled_container_color: Color,
        #[default(Color::Transparent)] error_container_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldCaretColor.value())] cursor_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldErrorFocusCaretColor.value())] error_cursor_color: Color,
        #[default(LocalTextSelectionColors.current())] selection_colors: TextSelectionColors,
        #[default(OutlinedAutocompleteTokens::TextFieldFocusOutlineColor.value())] focused_border_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldOutlineColor.value())] unfocused_border_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::TextFieldDisabledOutlineColor.value(), OutlinedAutocompleteTokens::TextFieldDisabledOutlineOpacity))]
        disabled_border_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldErrorOutlineColor.value())] error_border_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldFocusLeadingIconColor.value())]
        focused_leading_icon_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldLeadingIconColor.value())] unfocused_leading_icon_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::TextFieldDisabledLeadingIconColor.value(), OutlinedAutocompleteTokens::TextFieldDisabledLeadingIconOpacity))]
        disabled_leading_icon_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldErrorLeadingIconColor.value())] error_leading_icon_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldFocusTrailingIconColor.value())]
        focused_trailing_icon_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldTrailingIconColor.value())] unfocused_trailing_icon_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::TextFieldDisabledTrailingIconColor.value(), OutlinedAutocompleteTokens::TextFieldDisabledTrailingIconOpacity))]
        disabled_trailing_icon_color: Color,
        #[default(OutlinedAutocompleteTokens::TextFieldErrorTrailingIconColor.value())]
        error_trailing_icon_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldFocusLabelTextColor.value())] focused_label_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldLabelTextColor.value())] unfocused_label_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::FieldDisabledLabelTextColor.value(), OutlinedAutocompleteTokens::FieldDisabledLabelTextOpacity))]
        disabled_label_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldErrorLabelTextColor.value())] error_label_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] focused_placeholder_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] unfocused_placeholder_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::FieldDisabledSupportingTextColor.value(), OutlinedAutocompleteTokens::FieldDisabledSupportingTextOpacity))]
        disabled_placeholder_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] error_placeholder_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] focused_prefix_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] unfocused_prefix_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::FieldDisabledSupportingTextColor.value(), OutlinedAutocompleteTokens::FieldDisabledSupportingTextOpacity))]
        disabled_prefix_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] error_prefix_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] focused_suffix_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] unfocused_suffix_color: Color,
        #[default(with_alpha(OutlinedAutocompleteTokens::FieldDisabledSupportingTextColor.value(), OutlinedAutocompleteTokens::FieldDisabledSupportingTextOpacity))]
        disabled_suffix_color: Color,
        #[default(OutlinedAutocompleteTokens::FieldSupportingTextColor.value())] error_suffix_color: Color,
    ) -> TextFieldColors {
        named_args!(OutlinedTextFieldDefaults.colors(
            focused_text_color = focused_text_color,
            unfocused_text_color = unfocused_text_color,
            disabled_text_color = disabled_text_color,
            error_text_color = error_text_color,
            focused_container_color = focused_container_color,
            unfocused_container_color = unfocused_container_color,
            disabled_container_color = disabled_container_color,
            error_container_color = error_container_color,
            cursor_color = cursor_color,
            error_cursor_color = error_cursor_color,
            selection_colors = Some(selection_colors),
            focused_border_color = focused_border_color,
            unfocused_border_color = unfocused_border_color,
            disabled_border_color = disabled_border_color,
            error_border_color = error_border_color,
            focused_leading_icon_color = focused_leading_icon_color,
            unfocused_leading_icon_color = unfocused_leading_icon_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            error_leading_icon_color = error_leading_icon_color,
            focused_trailing_icon_color = focused_trailing_icon_color,
            unfocused_trailing_icon_color = unfocused_trailing_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            error_trailing_icon_color = error_trailing_icon_color,
            focused_label_color = focused_label_color,
            unfocused_label_color = unfocused_label_color,
            disabled_label_color = disabled_label_color,
            error_label_color = error_label_color,
            focused_placeholder_color = focused_placeholder_color,
            unfocused_placeholder_color = unfocused_placeholder_color,
            disabled_placeholder_color = disabled_placeholder_color,
            error_placeholder_color = error_placeholder_color,
            focused_prefix_color = focused_prefix_color,
            unfocused_prefix_color = unfocused_prefix_color,
            disabled_prefix_color = disabled_prefix_color,
            error_prefix_color = error_prefix_color,
            focused_suffix_color = focused_suffix_color,
            unfocused_suffix_color = unfocused_suffix_color,
            disabled_suffix_color = disabled_suffix_color,
            error_suffix_color = error_suffix_color,
        ))
    }

    /// The padding of the [`DropdownMenuItem`](crate::DropdownMenuItem)s of an
    /// [`ExposedDropdownMenu`](ExposedDropdownMenuBoxScope::ExposedDropdownMenu), lining them up with the field's
    /// text (Compose's `ItemContentPadding`): 16 dp across.
    pub fn ItemContentPadding() -> PaddingValues {
        PaddingValues::hv(ExposedDropdownMenuItemHorizontalPadding, Dp(0.0))
    }
}

/// The color with `alpha` (Kotlin's `copy(alpha = …)`).
fn with_alpha(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

/// Where an exposed dropdown menu goes (Compose's internal `ExposedDropdownMenuPositionProvider`): across, its
/// start at its anchor's start, else its end at the anchor's end, else against the window's side nearer the
/// anchor; down, below the anchor, else above it, else against the window's top or bottom nearer it
/// (`vertical_margin` from them) — the first that keeps it in the window, its height taken with the status bars'
/// `top_window_insets` (Compose's workaround of b/256233441). It reads `keyboard_signal_state` as it works the
/// position out, so a write to it places the menu again (b/326394521); `on_position_calculated` is told the bounds.
#[allow(dead_code)] // (Compose's constructor properties, kept as its `val`s are)
pub(crate) struct ExposedDropdownMenuPositionProvider {
    pub(crate) density: Density,
    pub(crate) top_window_insets: i32,
    pub(crate) keyboard_signal_state: Option<State<()>>,
    pub(crate) vertical_margin: i32,
    pub(crate) on_position_calculated: Rc<dyn Fn(IntRect, IntRect)>,
    // Horizontal position
    start_to_anchor_start: Rc<dyn MenuPositionHorizontal>,
    end_to_anchor_end: Rc<dyn MenuPositionHorizontal>,
    left_to_window_left: Rc<dyn MenuPositionHorizontal>,
    right_to_window_right: Rc<dyn MenuPositionHorizontal>,
    // Vertical position
    top_to_anchor_bottom: Rc<dyn MenuPositionVertical>,
    bottom_to_anchor_top: Rc<dyn MenuPositionVertical>,
    top_to_window_top: Rc<dyn MenuPositionVertical>,
    bottom_to_window_bottom: Rc<dyn MenuPositionVertical>,
}

impl ExposedDropdownMenuPositionProvider {
    /// Compose's constructor, all its parameters given (its defaults: no signal state, [`MenuVerticalMargin`] in
    /// `density`, nothing told).
    pub(crate) fn new(
        density: Density,
        top_window_insets: i32,
        keyboard_signal_state: Option<State<()>>,
        vertical_margin: i32,
        on_position_calculated: Rc<dyn Fn(IntRect, IntRect)>,
    ) -> ExposedDropdownMenuPositionProvider {
        ExposedDropdownMenuPositionProvider {
            density,
            top_window_insets,
            keyboard_signal_state,
            vertical_margin,
            on_position_calculated,
            start_to_anchor_start: MenuPosition::start_to_anchor_start(0),
            end_to_anchor_end: MenuPosition::end_to_anchor_end(0),
            left_to_window_left: MenuPosition::left_to_window_left(0),
            right_to_window_right: MenuPosition::right_to_window_right(0),
            top_to_anchor_bottom: MenuPosition::top_to_anchor_bottom(0),
            bottom_to_anchor_top: MenuPosition::bottom_to_anchor_top(0),
            top_to_window_top: MenuPosition::top_to_window_top(vertical_margin),
            bottom_to_window_bottom: MenuPosition::bottom_to_window_bottom(vertical_margin),
        }
    }
}

impl PopupPositionProvider for ExposedDropdownMenuPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        // Workaround for b/326394521
        // Read the state because we want any changes to the state to trigger recalculation.
        if let Some(signal) = &self.keyboard_signal_state {
            signal.get();
        }

        // Workaround for b/256233441
        // Popup fails to account for window insets so we do it here instead
        let window_size = IntSize::new(window_size.width, window_size.height + self.top_window_insets);

        let x_candidates = [
            &self.start_to_anchor_start,
            &self.end_to_anchor_end,
            if anchor_bounds.center().x < window_size.width / 2 {
                &self.left_to_window_left
            } else {
                &self.right_to_window_right
            },
        ];
        let mut x = 0;
        for (index, candidate) in x_candidates.iter().enumerate() {
            let x_candidate =
                candidate.position(anchor_bounds, window_size, popup_content_size.width, layout_direction);
            if index == x_candidates.len() - 1
                || (x_candidate >= 0 && x_candidate + popup_content_size.width <= window_size.width)
            {
                x = x_candidate;
                break;
            }
        }

        let y_candidates = [
            &self.top_to_anchor_bottom,
            &self.bottom_to_anchor_top,
            if anchor_bounds.center().y < window_size.height / 2 {
                &self.top_to_window_top
            } else {
                &self.bottom_to_window_bottom
            },
        ];
        let mut y = 0;
        for (index, candidate) in y_candidates.iter().enumerate() {
            let y_candidate = candidate.position(anchor_bounds, window_size, popup_content_size.height);
            if index == y_candidates.len() - 1
                || (y_candidate >= 0 && y_candidate + popup_content_size.height <= window_size.height)
            {
                y = y_candidate;
                break;
            }
        }

        let menu_offset = IntOffset::new(x, y);
        (self.on_position_calculated)(anchor_bounds, IntRect::from_offset_size(menu_offset, popup_content_size));
        menu_offset
    }
}

/// Compose's private `ExposedDropdownMenuAnchorElement`: equal by its callback's instance.
struct ExposedDropdownMenuAnchorElement {
    update_state_on_attach: Rc<dyn Fn()>,
}

impl PartialEq for ExposedDropdownMenuAnchorElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.update_state_on_attach, &other.update_state_on_attach)
    }
}

impl ModifierNodeElement for ExposedDropdownMenuAnchorElement {
    type Node = ExposedDropdownMenuAnchorNode;

    fn create(&self) -> ExposedDropdownMenuAnchorNode {
        ExposedDropdownMenuAnchorNode { update_state_on_attach: self.update_state_on_attach.clone() }
    }

    fn update(&self, node: &mut ExposedDropdownMenuAnchorNode) {
        node.update_state_on_attach = self.update_state_on_attach.clone();
    }

    fn name(&self) -> &'static str {
        "exposedDropdownMenuAnchorType"
    }
}

/// Compose's private `ExposedDropdownMenuAnchorNode`: tells the box its anchor's type as it is attached.
struct ExposedDropdownMenuAnchorNode {
    update_state_on_attach: Rc<dyn Fn()>,
}

impl ModifierNode for ExposedDropdownMenuAnchorNode {
    node_capabilities!();

    fn on_attach(&mut self, _cx: &NodeCx) {
        (self.update_state_on_attach)();
    }
}

/// What makes an anchor expand and collapse the menu (Compose's private `Modifier.expandable`): a press released
/// (seen in the initial pass, before the field takes it — a clickable would take the focus from the field; a
/// secondary editable anchor's press consumed), a key click (an editable primary anchor: Enter only), and the
/// semantics' click (an editable primary anchor showing the keyboard too); an editable primary anchor of an
/// expanded menu sends Tab, Down and Up to the menu, made focusable. To accessibility a dropdown list, or (a
/// secondary editable anchor) a button with the menu's state and a toggle description.
#[allow(clippy::too_many_arguments)]
fn expandable(
    modifier: Modifier,
    expanded: bool,
    on_expanded_change: Rc<dyn Fn()>,
    anchor_type: ExposedDropdownMenuAnchorType,
    always_focusable: MutableState<bool>,
    expanded_description: String,
    collapsed_description: String,
    toggle_description: String,
    keyboard_controller: Option<SoftwareKeyboardController>,
) -> Modifier {
    let (on_press, on_key, on_semantics_click) =
        (on_expanded_change.clone(), on_expanded_change.clone(), on_expanded_change.clone());
    modifier
        .pointer_input(CallbackKey(on_expanded_change), move |scope: PointerInputScope| {
            let on_press = on_press.clone();
            async move {
                scope
                    .await_each_gesture(move |s| {
                        let on_press = on_press.clone();
                        async move {
                            // Modifier.clickable makes the ExposedDropdownMenuBox capture focus first instead
                            // of the text field, which would be a confusing user experience, so we use
                            // Modifier.pointerInput in the Initial pass to observe events before the text field
                            // consumes them in the Main pass.
                            let down_event = s.await_first_down_with(true, PointerEventPass::Initial).await;
                            if anchor_type == ExposedDropdownMenuAnchorType::SecondaryEditable {
                                down_event.consume();
                            }
                            let up_event = s.wait_for_up_or_cancellation_with(PointerEventPass::Initial).await;
                            if up_event.is_some() {
                                on_press();
                            }
                        }
                    })
                    .await
            }
        })
        .on_preview_key_event(move |it: &KeyEvent| {
            // Make sure keyboard input works like if Modifier.clickable was set.
            if is_click(it) {
                if anchor_type != ExposedDropdownMenuAnchorType::PrimaryEditable {
                    on_key();
                } else if is_enter_minus_spacebar(it) {
                    // Primary editable shouldn't expand menu via spacebar.
                    on_key();
                    return true;
                }
            }

            if anchor_type == ExposedDropdownMenuAnchorType::PrimaryEditable && expanded {
                // Since we make the popup menu not focusable for PrimaryEditable to not interrupt
                // typing, we need to make sure the menu becomes focusable when the user try to
                // reach the menu via keyboard navigation.
                if it.key == Key::Tab || it.key == Key::DirectionDown || it.key == Key::DirectionUp {
                    always_focusable.set(true);
                    return true;
                }
            }

            always_focusable.set(false);
            false
        })
        .semantics(move |c| {
            if anchor_type == ExposedDropdownMenuAnchorType::SecondaryEditable {
                c.set_role(Role::Button);
                c.set_state_description(if expanded {
                    expanded_description.clone()
                } else {
                    collapsed_description.clone()
                });
                c.set_content_description(toggle_description.clone());
            } else {
                c.set_role(Role::DropdownList);
            }
            let (on_click, keyboard_controller) = (on_semantics_click.clone(), keyboard_controller.clone());
            c.on_click(move || {
                on_click();
                if anchor_type == ExposedDropdownMenuAnchorType::PrimaryEditable
                    && let Some(keyboard_controller) = &keyboard_controller
                {
                    keyboard_controller.show();
                }
                true
            });
        })
}

/// A pointer input key of a callback, equal only to itself (Compose keys `pointerInput(onExpandedChange)` by the
/// lambda, a new one each time the anchor's modifier is made).
struct CallbackKey(Rc<dyn Fn()>);

impl PartialEq for CallbackKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Whether a key went up as a click does (Compose's private `KeyEvent.isClick`): Enter (or the D-pad's center, the
/// keypad's Enter) or Space.
fn is_click(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyUp && (is_enter_minus_spacebar(event) || event.key == Key::Spacebar)
}

/// Whether it is Enter, the D-pad's center or the keypad's Enter (Compose's private `KeyEvent.isEnterMinusSpacebar`).
fn is_enter_minus_spacebar(event: &KeyEvent) -> bool {
    matches!(event.key, Key::DirectionCenter | Key::Enter | Key::NumPadEnter)
}

/// The menu's greatest height (Compose's private `calculateMaxHeight`): what the window leaves above or below the
/// anchor, the more of the two, `vertical_margin` from the window's edges (the whole margined window for an anchor
/// out of it); none without an anchor.
pub(crate) fn calculate_max_height(window_bounds: IntRect, anchor_bounds: Option<Rect>, vertical_margin: i32) -> i32 {
    let Some(anchor_bounds) = anchor_bounds else { return 0 };

    let margined_window_top = window_bounds.top + vertical_margin;
    let margined_window_bottom = window_bounds.bottom - vertical_margin;
    let available_height =
        if anchor_bounds.top > window_bounds.bottom as f32 || anchor_bounds.bottom < window_bounds.top as f32 {
            margined_window_bottom - margined_window_top
        } else {
            let height_above = anchor_bounds.top - margined_window_top as f32;
            let height_below = margined_window_bottom as f32 - anchor_bounds.bottom;
            height_above.max(height_below).round() as i32
        };

    available_height.max(0)
}

/// The anchor's bounds in the window (Compose's private `LayoutCoordinates?.getAnchorBounds`): from its position
/// in the window and its size (not `boundsInWindow()`, which can be empty while the window animates or resizes);
/// empty for none, or one no longer attached.
fn get_anchor_bounds(coordinates: Option<&LayoutCoordinates>) -> Rect {
    match coordinates {
        Some(coordinates) if coordinates.is_attached() => {
            let size = coordinates.size();
            Rect::from_offset_size(
                coordinates.position_in_window(),
                kompoz_graphics::Size::new(size.width as f32, size.height as f32),
            )
        }
        _ => Rect::Zero,
    }
}

const ExposedDropdownMenuItemHorizontalPadding: Dp = Dp(16.0);

#[cfg(test)]
mod tests {
    use super::*;

    type Told = Rc<RefCell<Vec<(IntRect, IntRect)>>>;

    fn provider(top_window_insets: i32, told: Told) -> ExposedDropdownMenuPositionProvider {
        ExposedDropdownMenuPositionProvider::new(
            Density { density: 1.0, font_scale: 1.0 },
            top_window_insets,
            None,
            48,
            Rc::new(move |anchor, menu| told.borrow_mut().push((anchor, menu))),
        )
    }

    #[test]
    fn the_menus_height_is_the_more_room_beside_its_anchor() {
        let window = IntRect::new(0, 0, 400, 800);
        assert_eq!(calculate_max_height(window, Some(Rect::new(0.0, 20.0, 400.0, 76.0)), 48), 676);
        assert_eq!(calculate_max_height(window, Some(Rect::new(0.0, 650.0, 400.0, 706.0)), 48), 602);
        // (an anchor out of the window: the whole margined window)
        assert_eq!(calculate_max_height(window, Some(Rect::new(0.0, 900.0, 400.0, 950.0)), 48), 704);
        assert_eq!(calculate_max_height(window, None, 48), 0);
        // (no room: none)
        let small = IntRect::new(0, 0, 400, 80);
        assert_eq!(calculate_max_height(small, Some(Rect::new(0.0, 20.0, 400.0, 60.0)), 48), 0);
        assert_eq!(calculate_max_height(window, Some(Rect::new(0.0, 20.4, 400.0, 75.6)), 48), 676);
    }

    #[test]
    fn exposed_menus_go_under_else_above_else_against_the_window() {
        let told: Told = Rc::new(RefCell::new(Vec::new()));
        let p = provider(0, told.clone());
        let window = IntSize::new(400, 800);
        let anchor = IntRect::new(20, 20, 380, 76);
        let ltr = LayoutDirection::Ltr;
        assert_eq!(p.calculate_position(anchor, window, ltr, IntSize::new(360, 200)), IntOffset::new(20, 76));
        assert_eq!(told.borrow().last(), Some(&(anchor, IntRect::new(20, 76, 380, 276))));
        // Right to left: its start (right) at the anchor's.
        let rtl = LayoutDirection::Rtl;
        assert_eq!(p.calculate_position(anchor, window, rtl, IntSize::new(112, 200)), IntOffset::new(268, 76));
        // No room below: above.
        let low = IntRect::new(20, 650, 380, 706);
        assert_eq!(p.calculate_position(low, window, ltr, IntSize::new(360, 300)), IntOffset::new(20, 350));
        // Neither: against the window's bottom (the anchor in its lower half); too tall for the margins: centered.
        assert_eq!(p.calculate_position(low, window, ltr, IntSize::new(360, 680)), IntOffset::new(20, 72));
        assert_eq!(p.calculate_position(low, window, ltr, IntSize::new(360, 720)), IntOffset::new(20, 40));
        // Too wide for its anchor's start or end: against the window's side nearer the anchor.
        let left = IntRect::new(10, 20, 60, 76);
        assert_eq!(p.calculate_position(left, window, ltr, IntSize::new(395, 100)), IntOffset::new(0, 76));
        // The status bars' insets count in the window's height (b/256233441): what fits below with them.
        let p = provider(30, told);
        assert_eq!(p.calculate_position(low, window, ltr, IntSize::new(360, 120)), IntOffset::new(20, 706));
    }

    #[test]
    fn a_secondary_anchor_takes_the_menu_only_from_another() {
        use ExposedDropdownMenuAnchorType as T;
        assert!(T::PrimaryNotEditable.has_greater_or_equal_priority_than(T::SecondaryEditable));
        assert!(T::PrimaryEditable.has_greater_or_equal_priority_than(T::PrimaryNotEditable));
        assert!(T::SecondaryEditable.has_greater_or_equal_priority_than(T::SecondaryEditable));
        assert!(!T::SecondaryEditable.has_greater_or_equal_priority_than(T::PrimaryEditable));
        assert!(!T::SecondaryEditable.has_greater_or_equal_priority_than(T::PrimaryNotEditable));
    }
}
