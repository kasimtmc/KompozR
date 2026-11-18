//! Context menus, as Compose for desktop's ContextMenuProvider.desktop.kt and BasicContextMenuRepresentation: a
//! [`ContextMenuArea`] opens its menu on a right click (a mouse's secondary press), with its items and those of the
//! areas and [`ContextMenuDataProvider`]s around it; [`LocalContextMenuRepresentation`] shows it — by default
//! [`LightDefaultContextMenuRepresentation`]: a focusable popup at the click, a column of rows (Material's menu
//! sizes: 112 to 280 dp wide, at least 32 dp high), hovered rows tinted, the arrows moving the focus, a click on an
//! item running it and closing the menu.

use crate::interaction::MutableInteractionSource;
use crate::prelude::*;
use crate::text::context_menu::{
    TextContextMenuComponent, TextContextMenuItem, TextContextMenuKey, TextContextMenuSession,
};
use kompoz_graphics::{Color, Dp, Offset, Rect};
use kompoz_runtime::{
    DerivedState, KompozitionLocal, KompozitionLocalProvider, MutableState, SideEffect, derived_state_of_with_policy,
    kompozable, kompozable_block, mutable_state_of, named, named_args, never_equal_policy, remember, remember_keyed,
};
use kompoz_text::TextStyle;
use kompoz_ui::Modifier;
use kompoz_ui::alignment::Alignment;
use kompoz_ui::focus::FocusDirection;
use kompoz_ui::input::{Key, KeyEvent, KeyEventType, PointerInputScope};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::{InputMode, LocalFocusManager, LocalInputModeManager};
use kompoz_ui::window::{Popup, PopupPositionProvider, PopupProperties, remember_popup_position_provider_at_position};
use std::fmt;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Items, data, state

/// An item of a context menu (Compose for desktop's `ContextMenuItem`): its label, whether it can be chosen, and
/// what choosing it does. Two are equal when their labels and enabled states are, and their actions the same.
#[derive(Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub enabled: bool,
    pub on_click: Rc<dyn Fn()>,
}

impl ContextMenuItem {
    /// An enabled item (Compose's `ContextMenuItem(label, onClick)`).
    pub fn new(label: impl Into<String>, on_click: impl Fn() + 'static) -> ContextMenuItem {
        ContextMenuItem { label: label.into(), enabled: true, on_click: Rc::new(on_click) }
    }

    /// An item, enabled or not (Compose's `ContextMenuItem(label, enabled, onClick)`).
    pub fn with_enabled(label: impl Into<String>, enabled: bool, on_click: impl Fn() + 'static) -> ContextMenuItem {
        ContextMenuItem { label: label.into(), enabled, on_click: Rc::new(on_click) }
    }
}

/// Compose for desktop's `ContextMenuItem(label, enabled = true, onClick)`, named arguments in a kompozable body;
/// plain code calls [`ContextMenuItem::new`] and [`ContextMenuItem::with_enabled`].
#[named(type)]
pub fn ContextMenuItem(
    label: impl Into<String>,
    #[default(true)] enabled: bool,
    on_click: impl Fn() + 'static,
) -> ContextMenuItem {
    ContextMenuItem::with_enabled(label, enabled, on_click)
}

impl PartialEq for ContextMenuItem {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && self.enabled == other.enabled && Rc::ptr_eq(&self.on_click, &other.on_click)
    }
}

impl fmt::Debug for ContextMenuItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ContextMenuItem(label='{}', enabled={})", self.label, self.enabled)
    }
}

/// What gives a menu's items when it opens (Compose's `items: () -> List<ContextMenuItem>`).
pub type ContextMenuItems = Rc<dyn Fn() -> Vec<ContextMenuItem>>;

/// The items of an area and of the areas and providers around it (Compose for desktop's `ContextMenuData`): its
/// own first, then those of `next` (the one around it).
#[derive(Clone)]
pub struct ContextMenuData {
    pub items: ContextMenuItems,
    pub next: Option<Rc<ContextMenuData>>,
}

impl ContextMenuData {
    pub fn new(items: ContextMenuItems, next: Option<Rc<ContextMenuData>>) -> ContextMenuData {
        ContextMenuData { items, next }
    }

    /// All the items: this one's, then those around it (Compose's internal `allItems`).
    pub fn all_items(&self) -> Vec<ContextMenuItem> {
        let mut all = (self.items)();
        let mut current = self.next.clone();
        while let Some(data) = current {
            all.extend((data.items)());
            current = data.next.clone();
        }
        all
    }
}

impl PartialEq for ContextMenuData {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.items, &other.items)
            && match (&self.next, &other.next) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            }
    }
}

/// The items around (Compose for desktop's `LocalContextMenuData`): what [`ContextMenuArea`]s and
/// [`ContextMenuDataProvider`]s add theirs to.
pub static LocalContextMenuData: KompozitionLocal<Option<Rc<ContextMenuData>>> = KompozitionLocal::new_static(|| None);

/// Whether a context menu is open, and where (Compose for desktop's `ContextMenuState.Status`): open at a rectangle
/// (a click's point, local to its area), or closed.
/// Kotlin: `ContextMenuState.Status`
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ContextMenuStatus {
    Open(Rect),
    Closed,
}

/// The state of a context menu (Compose for desktop's `ContextMenuState`): its status, a state (reading it
/// subscribes). Clones are the same state.
#[derive(Clone, PartialEq)]
pub struct ContextMenuState {
    status: MutableState<ContextMenuStatus>,
}

impl ContextMenuState {
    /// Closed (Compose's `ContextMenuState()`).
    pub fn new() -> ContextMenuState {
        ContextMenuState { status: mutable_state_of(ContextMenuStatus::Closed) }
    }

    pub fn status(&self) -> ContextMenuStatus {
        self.status.get()
    }

    pub fn set_status(&self, status: ContextMenuStatus) {
        self.status.set(status);
    }
}

impl Default for ContextMenuState {
    fn default() -> Self {
        ContextMenuState::new()
    }
}

impl fmt::Debug for ContextMenuState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ContextMenuState({:?})", self.status.peek())
    }
}

/// What shows context menus (Compose for desktop's `ContextMenuRepresentation`): given an area's state and its
/// items, composed in the area.
pub trait ContextMenuRepresentation {
    /// Shows the menu while `state` is open, with what `items` gives (read only when needed). Called in the
    /// composition (Compose's composable `Representation`).
    fn representation(&self, state: &ContextMenuState, items: ContextMenuItems);

    /// The same with the items as they are (Compose's deprecated overload).
    fn representation_of_list(&self, state: &ContextMenuState, items: Vec<ContextMenuItem>) {
        self.representation(state, Rc::new(move || items.clone()));
    }
}

/// The context menus' representation (Compose for desktop's `LocalContextMenuRepresentation`): light by default.
pub static LocalContextMenuRepresentation: KompozitionLocal<Rc<dyn ContextMenuRepresentation>> =
    KompozitionLocal::new_static(|| Rc::new(LightDefaultContextMenuRepresentation));

// ---------------------------------------------------------------------------------------------------------------
// The area, the providers, the open detector

/// Opens a context menu on a right click over `content` (Compose for desktop's `ContextMenuArea`): its `items`
/// first, then those of the areas and providers around it, shown by [`LocalContextMenuRepresentation`]; `state`
/// is its status. Not `enabled`: nothing opens it.
#[kompozable]
pub fn ContextMenuArea(
    items: impl Fn() -> Vec<ContextMenuItem> + 'static,
    #[default(remember(ContextMenuState::new))] state: ContextMenuState,
    #[default(true)] enabled: bool,
    content: impl Fn() + 'static,
) {
    let detector = context_menu_open_detector_of(Modifier, &state, enabled);
    context_menu_area_with_modifier(Rc::new(items), state, detector, Rc::new(content));
}

/// The open detector of an area's own menu (Compose's private `contextMenuOpenDetector(state, enabled)`): enabled
/// while the menu is closed; opens it at the click.
pub(crate) fn context_menu_open_detector_of(modifier: Modifier, state: &ContextMenuState, enabled: bool) -> Modifier {
    let open = state.clone();
    let closed = matches!(state.status(), ContextMenuStatus::Closed);
    modifier.context_menu_open_detector_with(state.clone(), enabled && closed, move |pointer_position| {
        open.set_status(ContextMenuStatus::Open(Rect::from_center(pointer_position, 0.0)));
    })
}

/// A context menu's area whose opening is `modifier`'s (Compose's internal `ContextMenuArea(items, state,
/// modifier, content)`): the items with those around, the content and the representation in a box.
#[track_caller]
pub(crate) fn context_menu_area_with_modifier(
    items: ContextMenuItems,
    state: ContextMenuState,
    modifier: Modifier,
    content: Rc<dyn Fn()>,
) {
    kompozable_block!({
        let data = Rc::new(ContextMenuData::new(items, LocalContextMenuData.current()));
        Box(
            modifier = modifier,
            propagate_min_constraints = true,
            content = move || {
                content();
                let data = data.clone();
                LocalContextMenuRepresentation.current().representation(&state, Rc::new(move || data.all_items()));
            },
        );
    })
}

/// Adds `items` to the context menus of the areas in `content` (Compose for desktop's `ContextMenuDataProvider`):
/// after theirs, before those around.
#[kompozable]
pub fn ContextMenuDataProvider(items: impl Fn() -> Vec<ContextMenuItem> + 'static, content: impl Fn() + 'static) {
    let data = Rc::new(ContextMenuData::new(Rc::new(items), LocalContextMenuData.current()));
    context_menu_data_provider_of(data, Rc::new(content));
}

/// Compose's internal `ContextMenuDataProvider(data, content)`.
#[track_caller]
pub(crate) fn context_menu_data_provider_of(data: Rc<ContextMenuData>, content: Rc<dyn Fn()>) {
    kompozable_block!({
        KompozitionLocalProvider(&[LocalContextMenuData.provides(Some(data))], move || content());
    })
}

/// The context menu modifiers of foundation.
#[named(trait ContextMenuModifiers)]
impl Modifier {
    /// Calls `on_open` with where a right click (a press with the mouse's secondary button) went down, the press
    /// consumed (Compose for desktop's `Modifier.contextMenuOpenDetector`); its pointer input starts again when
    /// `key` changes (Kotlin's `Any?`, `Unit` by default: none here). Not `enabled`: nothing.
    fn context_menu_open_detector(
        self,
        #[default(None)] key: Option<SlotKey>,
        #[default(true)] enabled: bool,
        on_open: impl Fn(Offset) + 'static,
    ) -> Modifier {
        self.context_menu_open_detector_with(key, enabled, on_open)
    }

    /// [`context_menu_open_detector`](ContextMenuModifiers::context_menu_open_detector) with every parameter, for
    /// plain code.
    fn context_menu_open_detector_with(
        self,
        key: impl PartialEq + 'static,
        enabled: bool,
        on_open: impl Fn(Offset) + 'static,
    ) -> Modifier {
        if !enabled {
            return self;
        }
        let on_open = Rc::new(on_open);
        self.pointer_input(key, move |scope: PointerInputScope| {
            let on_open = on_open.clone();
            async move {
                scope
                    .await_each_gesture(move |s| {
                        let on_open = on_open.clone();
                        async move {
                            // (Compose's `awaitEventFirstDown`: an event of every pointer going down)
                            let event = loop {
                                let event = s.await_pointer_event().await;
                                if event.changes.iter().all(|c| c.changed_to_down()) {
                                    break event;
                                }
                            };
                            if event.buttons.is_secondary_pressed() {
                                event.changes.iter().for_each(|c| c.consume());
                                on_open(event.changes[0].position);
                            }
                        }
                    })
                    .await
            }
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The default representation

/// The default representation of context menus with its colors (Compose for desktop's
/// `DefaultContextMenuRepresentation`): its background, its items' text, a hovered item's tint, a disabled item's
/// text (by default the text's at 0.38 alpha).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DefaultContextMenuRepresentation {
    pub background_color: Color,
    pub text_color: Color,
    pub item_hover_color: Color,
    pub disabled_text_color: Color,
}

impl DefaultContextMenuRepresentation {
    /// Kotlin: `DefaultContextMenuRepresentation(backgroundColor: Color, textColor: Color, itemHoverColor: Color, disabledTextColor: Color = textColor.copy(alpha = 0.38f))`
    pub fn new(
        background_color: Color,
        text_color: Color,
        item_hover_color: Color,
        disabled_text_color: Option<Color>,
    ) -> DefaultContextMenuRepresentation {
        DefaultContextMenuRepresentation {
            background_color,
            text_color,
            item_hover_color,
            disabled_text_color: disabled_text_color
                .unwrap_or_else(|| named_args!(text_color.copy(alpha = DISABLED_ALPHA))),
        }
    }
}

const DISABLED_ALPHA: f32 = 0.38;

/// The default representation for light themes (Compose for desktop's `LightDefaultContextMenuRepresentation`):
/// white, black text, items hovered at 4 % black.
pub const LightDefaultContextMenuRepresentation: DefaultContextMenuRepresentation = DefaultContextMenuRepresentation {
    background_color: Color::White,
    text_color: Color::Black,
    item_hover_color: Color::new(0.0, 0.0, 0.0, 0.04),
    disabled_text_color: Color::new(0.0, 0.0, 0.0, DISABLED_ALPHA),
};

/// The default representation for dark themes (Compose for desktop's `DarkDefaultContextMenuRepresentation`): the
/// dark surface (0xFF121212), white text, items hovered at 4 % white.
pub const DarkDefaultContextMenuRepresentation: DefaultContextMenuRepresentation = DefaultContextMenuRepresentation {
    background_color: Color::from_argb(0xFF121212),
    text_color: Color::White,
    item_hover_color: Color::new(1.0, 1.0, 1.0, 0.04),
    disabled_text_color: Color::new(1.0, 1.0, 1.0, DISABLED_ALPHA),
};

/// Closes the menu of a state (the session the items' clicks get).
struct StateSession(ContextMenuState);

impl TextContextMenuSession for StateSession {
    fn close(&self) {
        self.0.set_status(ContextMenuStatus::Closed);
    }
}

impl ContextMenuRepresentation for DefaultContextMenuRepresentation {
    #[track_caller]
    fn representation(&self, state: &ContextMenuState, items: ContextMenuItems) {
        let me = *self;
        let state = state.clone();
        kompozable_block!({
            let status = state.status();
            if let ContextMenuStatus::Open(rect) = status {
                let session = remember_keyed(state.clone(), || Rc::new(StateSession(state.clone())));
                // (Compose's `remember { derivedStateOf { … } }`: the items of the first composition's lambda)
                let components: DerivedState<Rc<Vec<TextContextMenuComponent>>> = remember(|| {
                    let session = session.clone();
                    // (Compose's derivedStateOf without a policy: its readers told at each new value)
                    derived_state_of_with_policy(never_equal_policy(), move || {
                        let session = session.clone();
                        Rc::new(
                            items()
                                .into_iter()
                                .map(|item| {
                                    let session = session.clone();
                                    let on_click = item.on_click.clone();
                                    TextContextMenuComponent::Item(TextContextMenuItem {
                                        key: TextContextMenuKey::Other(item.label.as_str().into()),
                                        label: item.label,
                                        enabled: item.enabled,
                                        leading_icon: None,
                                        on_click: Rc::new(move |_: &dyn TextContextMenuSession| {
                                            session.close();
                                            on_click();
                                        }),
                                    })
                                })
                                .collect(),
                        )
                    })
                });
                let components = components.get();
                if components.is_empty() {
                    let session = session.clone();
                    SideEffect(move || session.close());
                } else {
                    let colors = ContextMenuColors {
                        background_color: me.background_color,
                        text_color: me.text_color,
                        icon_color: me.text_color,
                        disabled_text_color: me.disabled_text_color,
                        disabled_icon_color: me.disabled_text_color,
                        hover_color: me.item_hover_color,
                    };
                    let provider = remember_popup_position_provider_at_position(position_px = rect.center());
                    DefaultOpenContextMenu(session as Rc<dyn TextContextMenuSession>, components, provider, colors);
                }
            }
        })
    }
}

/// The colors of a context menu (Compose for desktop's internal ContextMenuColors).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ContextMenuColors {
    pub(crate) background_color: Color,
    pub(crate) text_color: Color,
    pub(crate) icon_color: Color,
    pub(crate) disabled_text_color: Color,
    pub(crate) disabled_icon_color: Color,
    pub(crate) hover_color: Color,
}

impl ContextMenuColors {
    fn resolve_text_color(&self, enabled: bool) -> Color {
        if enabled { self.text_color } else { self.disabled_text_color }
    }
    fn resolve_icon_color(&self, enabled: bool) -> Color {
        if enabled { self.icon_color } else { self.disabled_icon_color }
    }
}

/// An open menu (Compose for desktop's internal `DefaultOpenContextMenu`, Material's menu design): a focusable
/// popup where `popup_position_provider` says, closing the session when dismissed, the arrows moving the focus
/// among its items (keyboard mode), its components in a scrolling column as wide as the widest.
#[kompozable]
pub(crate) fn DefaultOpenContextMenu(
    session: Rc<dyn TextContextMenuSession>,
    components: Rc<Vec<TextContextMenuComponent>>,
    popup_position_provider: Rc<dyn PopupPositionProvider>,
    colors: ContextMenuColors,
) {
    let focus_manager = LocalFocusManager.current();
    let input_mode_manager = LocalInputModeManager.current();
    let dismissed = session.clone();
    let on_key_event = move |event: &KeyEvent| {
        if event.r#type != KeyEventType::KeyDown {
            return false;
        }
        // (Compose's pairs: Down and the number pad's Up go forward, Up and the number pad's Down back)
        if event.key == Key::DirectionDown || event.key == Key::NumPadDirectionUp {
            input_mode_manager.request_input_mode(InputMode::Keyboard);
            focus_manager.move_focus(FocusDirection::Next);
            true
        } else if event.key == Key::DirectionUp || event.key == Key::NumPadDirectionDown {
            input_mode_manager.request_input_mode(InputMode::Keyboard);
            focus_manager.move_focus(FocusDirection::Previous);
            true
        } else {
            false
        }
    };
    Popup(
        popup_position_provider = Some(popup_position_provider),
        on_dismiss_request = move || dismissed.close(),
        properties = PopupProperties { focusable: true, ..PopupProperties::default() },
        on_key_event = Some(Rc::new(on_key_event) as kompoz_ui::input::KeyHandler),
        content = move || {
            let (session, components) = (session.clone(), components.clone());
            Column(
                modifier = Modifier
                    .shadow(Dp(8.0))
                    .background(colors.background_color)
                    .padding_hv(vertical = Dp(4.0))
                    .width_intrinsic(IntrinsicSize::Max)
                    .vertical_scroll(&remember_scroll_state()),
                content = move || {
                    for component in components.iter() {
                        match component {
                            TextContextMenuComponent::Separator => MenuSeparator(colors.text_color),
                            TextContextMenuComponent::Item(item) => MenuItem(session.clone(), colors, item.clone()),
                        }
                    }
                },
            );
        },
    );
}

#[kompozable]
fn MenuSeparator(color: Color) {
    Box(modifier = Modifier.padding_hv(vertical = Dp(8.0)).fill_max_width().height(Dp(1.0)).background(color));
}

#[kompozable]
fn MenuItem(session: Rc<dyn TextContextMenuSession>, colors: ContextMenuColors, component: TextContextMenuItem) {
    let enabled = component.enabled;
    let on_click = component.on_click.clone();
    let (icon, label) = (component.leading_icon.clone(), component.label.clone());
    MenuItemContent(
        colors,
        move || on_click(&*session),
        enabled,
        move || {
            if let Some(icon) = &icon {
                icon(colors.resolve_icon_color(enabled));
            }
            BasicText(
                label.clone(),
                style = TextStyle { color: colors.resolve_text_color(enabled), ..TextStyle::Default },
            );
        },
    );
}

/// A menu row (Compose's private `MenuItemContent`): clickable, its texts merged, tinted when hovered, as wide as
/// the menu, 112 to 280 dp wide and at least 32 high (Material's), 16 dp of padding across.
#[kompozable]
fn MenuItemContent(
    colors: ContextMenuColors,
    on_click: impl Fn() + 'static,
    enabled: bool,
    content: impl Fn() + 'static,
) {
    let interaction_source = remember(MutableInteractionSource::new);
    let hover = if enabled {
        let hovered = interaction_source.collect_is_hovered_as_state().get();
        Modifier.background(if hovered { colors.hover_color } else { Color::Transparent })
    } else {
        Modifier
    };
    Row(
        modifier = Modifier
            .clickable(enabled = enabled, interaction_source = Some(interaction_source), on_click = on_click)
            .semantics(merge_descendants = true, properties = |_| {})
            .then(hover)
            .fill_max_width()
            .size_in(min_width = Dp(112.0), max_width = Dp(280.0), min_height = Dp(32.0))
            .padding_values(PaddingValues::hv(Dp(16.0), Dp(0.0))),
        horizontal_arrangement = Arrangement::Start,
        vertical_alignment = Alignment::CenterVertically,
        content = content,
    );
}
