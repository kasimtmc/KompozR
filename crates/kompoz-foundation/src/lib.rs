//! KompozR's foundation, as Jetpack Compose's `androidx.compose.foundation` and `foundation.layout`: the layouts
//! (`Box`, `Row`, `Column`, `Spacer`), arrangements, the layout modifiers (padding, size, offset, aspect ratio,
//! intrinsic sizes, alignment line padding), window insets, the gesture detectors, the clickables (`clickable`,
//! `combined_clickable`, `selectable`, `toggleable`, `tri_state_toggleable`), `focusable`, `hoverable`,
//! interactions and their sources, indications, `magnifier`, `background` and `border`, `Canvas` and `Image`,
//! focus groups, progress and selectable group semantics, bring-into-view requesters, and text ([`text`]:
//! `BasicText` with its links, inline content and auto size; `text::selection`: `SelectionContainer`,
//! `DisableSelection`). Its modifiers extend `Modifier` through traits: `use kompoz_foundation::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

mod background;
mod border;
mod canvas;
pub mod clickable;
pub mod focusable;
pub mod focused_bounds;
pub mod gesture_node;
pub mod gestures;
pub mod hoverable;
mod image;
pub mod indication;
pub mod interaction;
pub mod layout;
pub mod magnifier;
pub mod progress_semantics;
pub mod relocation;
pub mod selection;
pub mod text;

pub use background::BackgroundModifiers;
pub use border::{BorderModifierNode, BorderModifiers};
pub use canvas::Canvas;
pub use image::{BitmapImage, Image, VectorImage};

/// What an app using foundation imports: the layouts, the composables, the modifier extension traits, the gesture
/// detectors, interactions and indications.
pub mod prelude {
    pub use crate::clickable::{Callback, ClickableModifiers};
    pub use crate::focusable::FocusableModifiers;
    pub use crate::focused_bounds::FocusedBoundsModifiers;
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
    pub use crate::magnifier::MagnifierModifiers;
    pub use crate::progress_semantics::ProgressSemanticsModifiers;
    pub use crate::relocation::{BringIntoViewRequester, RelocationModifiers};
    pub use crate::selection::{SelectableModifiers, SelectionModifiers, ToggleableModifiers};
    pub use crate::text::selection::{
        DisableSelection, LocalTextSelectionColors, SelectionContainer, SelectionContainerWithSelection,
        SelectionContainerWithState, SelectionState, TextSelectionColors, remember_selection_state,
    };
    pub use crate::text::{
        BasicText, BasicTextValue, InlineContentMap, InlineTextContent, OnTextLayout, StepBased, TextAutoSize,
        TextAutoSizeDefaults,
    };
    pub use crate::{BackgroundModifiers, BitmapImage, BorderModifiers, Canvas, Image, VectorImage};
}
