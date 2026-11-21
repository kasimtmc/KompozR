//! KompozR's Material 3, as Jetpack Compose's `androidx.compose.material3` (1.5.0-alpha17, Material Expressive's
//! too; Compose for desktop's actuals where the platforms differ): the theme — [`MaterialTheme`] and
//! [`MaterialExpressiveTheme`], color schemes ([`ColorScheme`]: Material's baseline, the system's dynamic colors,
//! the wallpaper's), the type scale ([`Typography`]), shapes ([`Shapes`]), motion ([`MotionScheme`]) —, [`Text`],
//! [`Surface`], [`Icon`], the ripple ([`ripple`]), dividers, the scrim, the minimum interactive size, and the
//! design tokens and texts (in Compose for desktop's 80 or so languages) its components are made of.
//!
//! Its modifiers extend `Modifier` through traits: `use kompoz_material3::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

// (WP5: dialogs)
mod alert_dialog;
mod badge;
mod button;
mod card;
mod chip;
mod color_scheme;
mod content_color;
mod divider;
mod dynamic_color;
mod exposed_dropdown_menu;
mod exposed_dropdown_menu_desktop;
mod flags;
mod floating_action_button;
mod horizontal_center_optically;
mod icon;
mod icon_button;
mod icon_button_defaults;
mod interactive_component_size;
// (WP5: sheets)
mod bottom_sheet_scaffold;
mod drag_handle;
pub(crate) mod internal;
mod label;
mod modal_bottom_sheet;
mod sheet_defaults;
// (WP5: lists)
mod list_item;
mod list_item_defaults;
mod material_theme;
mod menu;
mod menu_defaults;
mod menu_desktop;
mod motion_scheme;
mod outlined_text_field;
// (WP4b: bar)
mod navigation_bar;
mod navigation_item;
mod short_navigation_bar;
// (WP4b: rail)
mod navigation_rail;
mod wide_navigation_rail;
mod wide_navigation_rail_state;
// (WP4b: drawer)
mod navigation_drawer;
mod precision_pointer;
// (WP5: pull to refresh — Compose's package androidx.compose.material3.pulltorefresh)
pub mod pulltorefresh;
mod ripples;
mod scrim;
mod secure_text_field;
mod shapes;
// (WP5: snackbars)
mod snackbar;
mod snackbar_host;
mod surface;
// (WP5: swipe to dismiss)
mod swipe_to_dismiss_box;
// (WP5: tabs)
mod tab;
mod tab_row;
mod text;
mod text_field;
mod text_field_defaults;
pub(crate) mod tokens;
mod tonal_palette;
mod tooltip;
mod typography;

// (WP1a: buttons — Button, FloatingActionButton)
pub use button::{
    Button, ButtonColors, ButtonDefaults, ButtonElevation, ButtonShapes, ButtonWithShapes, ElevatedButton,
    ElevatedButtonWithShapes, FilledTonalButton, FilledTonalButtonWithShapes, OutlinedButton, OutlinedButtonWithShapes,
    TextButton, TextButtonWithShapes,
};
pub use floating_action_button::{
    ExtendedFloatingActionButton, FloatingActionButton, FloatingActionButtonDefaults, FloatingActionButtonElevation,
    FloatingActionButtonModifiers, LargeExtendedFloatingActionButton, LargeFloatingActionButton,
    MediumExtendedFloatingActionButton, MediumFloatingActionButton, SmallExtendedFloatingActionButton,
    SmallFloatingActionButton, TextIconExtendedFloatingActionButton, TextIconLargeExtendedFloatingActionButton,
    TextIconMediumExtendedFloatingActionButton, TextIconSmallExtendedFloatingActionButton,
};
// (WP1a: icon buttons — IconButton, IconButtonDefaults)
pub use icon_button::{
    FilledIconButton, FilledIconButtonWithShapes, FilledIconToggleButton, FilledIconToggleButtonWithShapes,
    FilledTonalIconButton, FilledTonalIconButtonWithShapes, FilledTonalIconToggleButton,
    FilledTonalIconToggleButtonWithShapes, IconButton, IconButtonColors, IconButtonShapes, IconButtonWithShapes,
    IconToggleButton, IconToggleButtonColors, IconToggleButtonShapes, IconToggleButtonWithShapes, OutlinedIconButton,
    OutlinedIconButtonWithShapes, OutlinedIconToggleButton, OutlinedIconToggleButtonWithShapes,
};
#[allow(deprecated)]
pub use icon_button::{IconButtonWithoutShape, IconToggleButtonWithoutShape};
pub use icon_button_defaults::{IconButtonDefaults, IconButtonWidthOption};
// (WP5: dialogs)
#[allow(deprecated)]
pub use alert_dialog::AlertDialogWithContent;
pub use alert_dialog::{
    AlertDialog, AlertDialogDefaults, BasicAlertDialog, BasicAlertDialogOverride, BasicAlertDialogOverrideScope,
    DefaultBasicAlertDialogOverride, LocalBasicAlertDialogOverride,
};
// (WP1a: containers — Card, Chip, Badge)
pub use badge::{Badge, BadgeDefaults, BadgedBox};
pub use card::{
    Card, CardColors, CardDefaults, CardElevation, ClickableCard, ClickableElevatedCard, ClickableOutlinedCard,
    ElevatedCard, OutlinedCard,
};
pub use chip::{
    AssistChip, AssistChipDefaults, ChipColors, ChipElevation, ElevatedAssistChip, ElevatedFilterChip,
    ElevatedSuggestionChip, FilterChip, FilterChipDefaults, InputChip, InputChipDefaults, SelectableChipColors,
    SelectableChipElevation, SuggestionChip, SuggestionChipDefaults,
};
#[allow(deprecated)]
pub use chip::{
    AssistChipWithChipBorder, AssistChipWithoutArrangement, ChipBorder, ElevatedAssistChipWithChipBorder,
    ElevatedAssistChipWithoutArrangement, ElevatedFilterChipWithoutArrangement, ElevatedSuggestionChipWithChipBorder,
    ElevatedSuggestionChipWithoutArrangement, FilterChipWithoutArrangement, InputChipWithoutArrangement,
    SuggestionChipWithChipBorder, SuggestionChipWithoutArrangement,
};
pub use color_scheme::{
    ColorScheme, LocalTonalElevationEnabled, content_color_for, dark_color_scheme, expressive_light_color_scheme,
    light_color_scheme,
};
#[allow(deprecated)]
pub use color_scheme::{color_scheme_without_fixed_roles, color_scheme_without_surface_containers};
pub use content_color::LocalContentColor;
#[allow(deprecated)]
pub use divider::Divider;
pub use divider::{DividerDefaults, HorizontalDivider, VerticalDivider};
pub use dynamic_color::{
    dynamic_dark_color_scheme, dynamic_dark_color_scheme_of, dynamic_light_color_scheme, dynamic_light_color_scheme_of,
    is_dynamic_color_available, is_wallpaper_color_available, wallpaper_dark_color_scheme,
    wallpaper_light_color_scheme,
};
#[allow(deprecated)]
pub use exposed_dropdown_menu::MenuAnchorType;
pub use exposed_dropdown_menu::{
    ExposedDropdownMenuAnchorType, ExposedDropdownMenuBox, ExposedDropdownMenuBoxScope, ExposedDropdownMenuDefaults,
};
pub use flags::ComposeMaterial3Flags;
pub use icon::{BitmapIcon, Icon, TintProducerIcon, VectorIcon};
#[allow(deprecated)]
pub use interactive_component_size::LocalMinimumInteractiveComponentEnforcement;
pub use interactive_component_size::{
    InteractiveComponentSizeModifiers, LocalMinimumInteractiveComponentSize, MinimumInteractiveLeftAlignmentLine,
    MinimumInteractiveTopAlignmentLine,
};
pub use label::Label;
pub use material_theme::{LocalUsingExpressiveTheme, MaterialExpressiveTheme, MaterialTheme, MaterialThemeValues};
pub use menu::{
    CheckableDropdownMenuItem, DropdownMenuGroup, MenuGroupShapes, MenuItemColors, MenuItemShapes,
    SelectableDropdownMenuItem, ShapedDropdownMenuItem,
};
pub use menu_defaults::MenuDefaults;
pub use menu_desktop::{DropdownMenu, DropdownMenuItem, DropdownMenuPopup};
pub use motion_scheme::{AnyTypeAnimationSpec, AnyTypeSpec, MotionScheme, MotionSchemeSpecs};
pub use outlined_text_field::{OutlinedTextField, StringOutlinedTextField, ValueOutlinedTextField};
// (WP4b: bar — NavigationBar, NavigationItem, ShortNavigationBar)
pub use navigation_bar::{
    DefaultNavigationBarOverride, LocalNavigationBarOverride, NavigationBar, NavigationBarDefaults, NavigationBarItem,
    NavigationBarItemColors, NavigationBarItemDefaults, NavigationBarOverride, NavigationBarOverrideScope,
};
pub use navigation_item::{NavigationItemColors, NavigationItemIconPosition};
pub use short_navigation_bar::{
    DefaultShortNavigationBarOverride, LocalShortNavigationBarOverride, ShortNavigationBar,
    ShortNavigationBarArrangement, ShortNavigationBarDefaults, ShortNavigationBarItem, ShortNavigationBarItemDefaults,
    ShortNavigationBarOverride, ShortNavigationBarOverrideScope,
};
// (WP4b: rail — NavigationRail)
pub use navigation_rail::{
    DefaultNavigationRailOverride, LocalNavigationRailOverride, NavigationRail, NavigationRailDefaults,
    NavigationRailItem, NavigationRailItemColors, NavigationRailItemDefaults, NavigationRailOverride,
    NavigationRailOverrideScope,
};
// (WP4b: rail — WideNavigationRailState)
pub use wide_navigation_rail_state::{
    WideNavigationRailState, WideNavigationRailValue, remember_wide_navigation_rail_state,
};
// (WP4b: rail — WideNavigationRail, ModalWideNavigationRail)
#[allow(deprecated)]
pub use wide_navigation_rail::ModalWideNavigationRailDefaults;
pub use wide_navigation_rail::{
    DefaultModalWideNavigationRailOverride, DefaultWideNavigationRailOverride, LocalModalWideNavigationRailOverride,
    LocalWideNavigationRailOverride, ModalWideNavigationRail, ModalWideNavigationRailOverride,
    ModalWideNavigationRailOverrideScope, ModalWideNavigationRailProperties, WideNavigationRail,
    WideNavigationRailColors, WideNavigationRailDefaults, WideNavigationRailItem, WideNavigationRailItemDefaults,
    WideNavigationRailOverride, WideNavigationRailOverrideScope,
};
// (WP4b: drawer — NavigationDrawer)
pub use navigation_drawer::{
    DismissibleDrawerSheet, DismissibleDrawerSheetWithDrawerState, DismissibleNavigationDrawer, DrawerDefaults,
    DrawerState, DrawerValue, ModalDrawerSheet, ModalDrawerSheetWithDrawerState, ModalNavigationDrawer,
    NavigationDrawerItem, NavigationDrawerItemColors, NavigationDrawerItemDefaults, PermanentDrawerSheet,
    PermanentNavigationDrawer, remember_drawer_state,
};
pub use precision_pointer::should_use_precision_pointer_component_sizing;
pub use ripples::{LocalRippleConfiguration, RippleConfiguration, RippleDefaults, ripple, ripple_color_producer};
pub use scrim::{Scrim, ScrimDefaults};
pub use secure_text_field::{OutlinedSecureTextField, SecureTextField};
pub use shapes::{ShapeDefaults, Shapes};
pub use surface::{ClickableSurface, LocalAbsoluteTonalElevation, SelectableSurface, Surface, ToggleableSurface};
// (WP5: swipe to dismiss)
pub use swipe_to_dismiss_box::{
    SwipeToDismissBox, SwipeToDismissBoxDefaults, SwipeToDismissBoxState, SwipeToDismissBoxValue,
    remember_swipe_to_dismiss_box_state,
};
#[allow(deprecated)]
pub use swipe_to_dismiss_box::{remember_swipe_to_dismiss_box_state_confirming, swipe_to_dismiss_box_state_confirming};
// (WP5: lists)
pub use list_item::{
    ClickableListItem, ListItem, SegmentedListItem, SelectableListItem, SelectableSegmentedListItem,
    ToggleableListItem, ToggleableSegmentedListItem,
};
pub use list_item_defaults::{ListItemColors, ListItemDefaults, ListItemElevation, ListItemShapes};
// (WP5: tabs)
pub use tab::{LeadingIconTab, Tab, TextIconTab};
pub use tab_row::{
    PrimaryScrollableTabRow, PrimaryTabRow, SecondaryScrollableTabRow, SecondaryTabRow, TabIndicatorScope, TabPosition,
    TabRowDefaults,
};
#[allow(deprecated)]
pub use tab_row::{ScrollableTabRow, TabRow};
// (WP5: sheets)
pub use bottom_sheet_scaffold::{
    BottomSheetScaffold, BottomSheetScaffoldState, remember_bottom_sheet_scaffold_state,
    remember_standard_bottom_sheet_state,
};
pub use drag_handle::{
    DragHandleColors, DragHandleShapes, DragHandleSizes, VerticalDragHandle, VerticalDragHandleDefaults,
};
pub use modal_bottom_sheet::{
    ModalBottomSheet, ModalBottomSheetDefaults, ModalBottomSheetProperties, remember_modal_bottom_sheet_state,
};
pub use sheet_defaults::{BottomSheet, BottomSheetDefaults, SheetState, SheetValue};
pub use text::{LocalTextStyle, ProvideTextStyle, Text, TextColorProducer};
pub use text_field::{IndicatorLineModifiers, StringTextField, TextField, ValueTextField};
pub use text_field_defaults::{
    OutlinedTextFieldDefaults, TextFieldColors, TextFieldDefaults, TextFieldLabel, TextFieldLabelPosition,
    TextFieldLabelScope, TextFieldSlot,
};
pub use tooltip::{TooltipAnchorPosition, TooltipDefaults, TooltipScope, TooltipState};
// (WP5: snackbars — Snackbar, SnackbarHost, SnackbarHostState)
pub use snackbar::{DataSnackbar, Snackbar, SnackbarDefaults};
pub use snackbar_host::{
    SnackbarData, SnackbarDuration, SnackbarHost, SnackbarHostState, SnackbarResult, SnackbarVisuals,
};
// (WP5: tooltips — TooltipBox, PlainTooltip, rememberTooltipState)
pub use tooltip::{
    DefaultTooltipCaretShape, DynTooltipScope, TooltipBox, TooltipScopeComposables, remember_tooltip_state,
};
pub use typography::{Typography, typography_with_font_family};

/// What an app using Material 3 imports: the theme, the components, the modifier extension traits.
pub mod prelude {
    pub use crate::*;
    // (WP5: pull to refresh — what a Compose app imports from androidx.compose.material3.pulltorefresh)
    pub use crate::pulltorefresh::{
        PullToRefreshDefaults, PullToRefreshModifiers, PullToRefreshState, remember_pull_to_refresh_state,
    };
}
