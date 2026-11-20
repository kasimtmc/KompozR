//! Inherited text properties (Compose's TextStyleProviderNode.kt): the text style the styles of the layouts a text
//! is in give it (a [`styleable`](crate::style::StyleModifiers::styleable)'s `font_size`, `content_color`…), what
//! its own style specifies overriding them. Compose finds the nearest style node by traversing to it; KompozR finds
//! it through the modifier local the style's outer node provides (the same node, found while it is busy measuring
//! or drawing the text).

use crate::style::LocalStyleOuter;
use crate::style::properties::{InheritedFlags, TextDrawFlag, TextLayoutFlag};
use kompoz_text::TextStyle;
use kompoz_ui::NodeCx;

/// The phase asking for the inherited text style (Compose's StylePhase): its layout's properties, its drawing's,
/// or all of them (what an animation of them reads).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StylePhase {
    Layout,
    Draw,
    #[allow(dead_code)]
    All,
}

impl StylePhase {
    fn to_flags(self) -> u32 {
        match self {
            StylePhase::Layout => TextLayoutFlag,
            StylePhase::Draw => TextDrawFlag,
            StylePhase::All => InheritedFlags,
        }
    }
}

/// The text style inherited at `cx`'s node for `phase`, `fallback`'s specified properties overriding it; `fallback`
/// where no style above sets a text property (Compose's `DelegatableNode.inheritedTextStyle`).
pub(crate) fn inherited_text_style(cx: &NodeCx, phase: StylePhase, fallback: &TextStyle) -> TextStyle {
    match cx.modifier_local_current(&LocalStyleOuter).and_then(|w| w.upgrade()) {
        Some(outer) => outer.compute_inherited_text_style(phase.to_flags(), fallback),
        None => fallback.clone(),
    }
}
