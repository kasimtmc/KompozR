//! KompozR's foundation, as Jetpack Compose's `androidx.compose.foundation` and `foundation.layout`: the layouts
//! (`Box`, `Row`, `Column`, `Spacer`), arrangements, the layout modifiers (padding, size, offset, aspect ratio,
//! intrinsic sizes, alignment line padding), window insets, the gesture detectors, the clickables (`clickable`,
//! `combined_clickable`, `selectable`, `toggleable`, `tri_state_toggleable`), `focusable`, `hoverable`,
//! interactions and their sources, indications, `magnifier`, `background` and `border`, `Canvas` and `Image`,
//! focus groups, progress and selectable group semantics, bring-into-view requesters and responders, scrolling
//! ([`gestures`]: `draggable`, `scrollable` with its flings, wheel, nested scroll and bringing into view;
//! [`scroll`]: `ScrollState`, `vertical_scroll`, `horizontal_scroll`; [`scrollable_area`]; snapping:
//! [`gestures::snapping`]), overscroll (Android's stretch: [`overscroll`]), lazy lists and grids ([`lazy`]: `LazyColumn`,
//! `LazyRow`, `LazyListState`; [`lazy::grid`]: `LazyVerticalGrid`, `LazyHorizontalGrid`; [`lazy::staggeredgrid`];
//! the `LazyLayout` core: [`lazy::layout`]), pagers ([`pager`]: `HorizontalPager`, `VerticalPager`), and text ([`text`]:
//! `BasicText` with its links, inline content and auto size; `text::selection`: `SelectionContainer`,
//! `DisableSelection`; `text::input`: the state-based text field's model — `TextFieldState`, its buffer, undo and
//! transformations —, keyboard options and actions, key mappings), and Compose for desktop's extras: scrollbars
//! ([`scrollbar`]: `VerticalScrollbar`, `HorizontalScrollbar`; their adapters [`v2`]), context menus
//! ([`context_menu`]: `ContextMenuArea`; texts' own: [`text::context_menu_area`]), `TooltipArea` ([`tooltip_area`]),
//! `Modifier.on_click` ([`on_click`]) and `Modifier.on_drag` of a [`pointer_matcher::PointerMatcher`]. Its modifiers
//! extend `Modifier` through traits: `use kompoz_foundation::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

mod background;
pub mod basic_marquee;
pub mod basic_tooltip;
mod border;
mod canvas;
pub mod clickable;
pub mod clip_scrollable_container;
pub mod content;
pub mod context_menu;
pub mod draganddrop;
mod edge_effect;
pub mod focusable;
pub mod focused_bounds;
pub mod gesture_node;
pub mod gestures;
pub mod hoverable;
mod image;
pub mod indication;
pub mod interaction;
pub mod layout;
pub mod lazy;
pub mod magnifier;
pub mod on_click;
pub mod overscroll;
pub mod overscroll_android;
pub mod pager;
pub mod pointer_matcher;
pub mod progress_semantics;
pub mod relocation;
pub mod scroll;
pub mod scrollable_area;
pub mod scrollbar;
pub mod selection;
pub mod text;
pub mod tooltip_area;
pub mod v2;

pub use background::BackgroundModifiers;
pub use border::{BorderModifierNode, BorderModifiers};
pub use canvas::Canvas;
pub use image::{BitmapImage, Image, VectorImage};
/// Compose's `androidx.compose.foundation.MutatorMutex` (kompoz-animation's, which Animatable uses too).
pub use kompoz_animation::{MutatePriority, MutatorMutex};

/// What an app using foundation imports: the layouts, the composables, the modifier extension traits, the gesture
/// detectors, interactions and indications.
pub mod prelude {
    pub use crate::basic_marquee::{MarqueeAnimationMode, MarqueeDefaults, MarqueeModifiers, MarqueeSpacing};
    pub use crate::basic_tooltip::{
        BasicTooltipBox, BasicTooltipDefaults, BasicTooltipState, remember_basic_tooltip_state,
    };
    pub use crate::clickable::{Callback, ClickableModifiers};
    pub use crate::clip_scrollable_container::{ClipScrollableContainerModifiers, MaxSupportedElevation};
    pub use crate::content::{
        ContentReceiverModifiers, MediaType, PlatformTransferableContent, ReceiveContentListener, TransferableContent,
        TransferableContentSource,
    };
    pub use crate::context_menu::{
        ContextMenuArea, ContextMenuData, ContextMenuDataProvider, ContextMenuItem, ContextMenuModifiers,
        ContextMenuRepresentation, ContextMenuState, ContextMenuStatus, DarkDefaultContextMenuRepresentation,
        DefaultContextMenuRepresentation, LightDefaultContextMenuRepresentation, LocalContextMenuData,
        LocalContextMenuRepresentation,
    };
    pub use crate::draganddrop::DragAndDropModifiers;
    pub use crate::focusable::FocusableModifiers;
    pub use crate::focused_bounds::FocusedBoundsModifiers;
    pub use crate::gestures::snapping::{
        SnapLayoutInfoProvider, SnapPosition, lazy_grid_snap_layout_info_provider, lazy_list_snap_layout_info_provider,
        remember_snap_fling_behavior, remember_snap_fling_behavior_for_grid, remember_snap_fling_behavior_for_list,
        snap_fling_behavior,
    };
    pub use crate::gestures::*;
    pub use crate::hoverable::HoverableModifiers;
    pub use crate::indication::{
        DefaultDebugIndication, Indication, IndicationModifiers, IndicationNodeFactory, LocalIndication,
    };
    pub use crate::interaction::{
        DragInteraction, FocusInteraction, HoverInteraction, Interaction, InteractionSource, MutableInteractionSource,
        PressInteraction,
    };
    pub use crate::layout::*;
    pub use crate::lazy::grid::{
        GridCells, GridItemSpan, LazyGridItemInfo, LazyGridItemSpanScope, LazyGridLayoutInfo, LazyGridScope,
        LazyGridState, LazyHorizontalGrid, LazyVerticalGrid, remember_lazy_grid_state,
    };
    pub use crate::lazy::layout::LazyKey;
    pub use crate::lazy::staggeredgrid::{
        LazyHorizontalStaggeredGrid, LazyStaggeredGridItemInfo, LazyStaggeredGridLayoutInfo, LazyStaggeredGridScope,
        LazyStaggeredGridState, LazyVerticalStaggeredGrid, StaggeredGridCells, StaggeredGridItemSpan,
        remember_lazy_staggered_grid_state,
    };
    pub use kompoz_ui::draganddrop::{
        DragAndDropEvent, DragAndDropTarget, DragAndDropTransferAction, DragAndDropTransferData,
        drag_and_drop_transfer_data,
    };
    // (a lazy item's content type: Kotlin's `Any`, a subcomposition's slot id)
    pub use crate::lazy::{
        LazyColumn, LazyItemScopeModifiers, LazyListItemInfo, LazyListLayoutInfo, LazyListScope, LazyListState,
        LazyRow, remember_lazy_list_state,
    };
    pub use crate::magnifier::MagnifierModifiers;
    pub use crate::on_click::OnClickModifiers;
    pub use crate::overscroll::{
        LocalOverscrollFactory, OverscrollEffect, OverscrollFactory, OverscrollModifiers, PerformFling,
        remember_overscroll_effect, without_event_handling, without_visual_effect,
    };
    pub use crate::overscroll_android::{
        LocalOverscrollConfiguration, OverscrollConfiguration, remember_platform_overscroll_factory,
    };
    pub use crate::pager::{
        HorizontalPager, PageInfo, PageSize, PagerDefaults, PagerLayoutInfo, PagerSnapDistance, PagerState,
        VerticalPager, remember_pager_state,
    };
    pub use crate::pointer_matcher::PointerMatcher;
    pub use crate::progress_semantics::ProgressSemanticsModifiers;
    pub use crate::relocation::{
        BringIntoViewRequester, BringIntoViewResponder, BringIntoViewResponderModifiers, RelocationModifiers,
    };
    pub use crate::scroll::{ScrollModifiers, ScrollState, remember_scroll_state};
    pub use crate::scrollable_area::ScrollableAreaModifiers;
    pub use crate::scrollbar::{
        HorizontalScrollbar, LocalScrollbarStyle, ScrollbarAdapter, ScrollbarStyle, VerticalScrollbar,
        default_scrollbar_style, remember_scrollbar_adapter,
    };
    pub use crate::selection::{SelectableModifiers, SelectionModifiers, ToggleableModifiers};
    pub use crate::text::input::{
        ChangeList, ExpandPolicy, InputTransformation, KeyboardActionHandler, OutputTransformation, TextFieldBuffer,
        TextFieldLineLimits, TextFieldState, TextFieldTextStyles, TextObfuscationMode, TrackedRange, TransformInput,
        UndoState, output_transformation, remember_text_field_state,
    };
    pub use crate::text::selection::{
        DisableSelection, LocalTextSelectionColors, SelectionContainer, SelectionContainerWithSelection,
        SelectionContainerWithState, SelectionState, TextSelectionColors, remember_selection_state,
    };
    pub use crate::text::{
        BasicSecureTextField, BasicText, BasicTextField, BasicTextFieldString, BasicTextFieldValue, BasicTextValue,
        DecorationBox, InlineContentMap, InlineTextContent, KeyboardActionScope, KeyboardActions, KeyboardOptions,
        LocalTextContextMenu, OnTextLayout, StepBased, TextAutoSize, TextAutoSizeDefaults, TextContextMenu,
        TextContextMenuArea, TextFieldDecorator, TextFieldScrollState, remember_text_field_horizontal_scroll_state,
        remember_text_field_scroll_state, remember_text_field_vertical_scroll_state,
    };
    pub use crate::tooltip_area::{BoxWithTooltip, TooltipArea, TooltipPlacement};
    pub use crate::{BackgroundModifiers, BitmapImage, BorderModifiers, Canvas, Image, VectorImage};
    pub use crate::{MutatePriority, MutatorMutex};
    pub use kompoz_ui::layout::SlotKey;
}
