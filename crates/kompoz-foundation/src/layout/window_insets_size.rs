//! Window insets as a size, as Compose's `Modifier.windowInsetsTopHeight` family: a side of the insets that the
//! layouts around haven't consumed, as a width or a height (a spacer under a bar).

use super::window_insets::WindowInsets;
use super::window_insets_padding::{InsetSide, InsetsConsumingNode, Kind};
use kompoz_runtime::named;
use kompoz_ui::{Modifier, ModifierNodeElement};

/// The window insets size modifiers (Compose's `windowInsetsStartWidth`, `windowInsetsEndWidth`,
/// `windowInsetsTopHeight` and `windowInsetsBottomHeight`): the side's insets less what the layouts around
/// consumed is the size that way (0 by 0 when that is none), what it wraps measured to it, the other way as the
/// constraints say.
#[named(trait WindowInsetsSizeModifiers)]
impl Modifier {
    /// As wide as `insets` at the start: the left left to right, the right right to left (Compose's
    /// `windowInsetsStartWidth`).
    fn window_insets_start_width(self, insets: WindowInsets) -> Modifier {
        self.then_element(DerivedSizeElement { insets, side: InsetSide::Start })
    }

    /// As wide as `insets` at the end (Compose's `windowInsetsEndWidth`).
    fn window_insets_end_width(self, insets: WindowInsets) -> Modifier {
        self.then_element(DerivedSizeElement { insets, side: InsetSide::End })
    }

    /// As high as `insets` at the top (Compose's `windowInsetsTopHeight`).
    fn window_insets_top_height(self, insets: WindowInsets) -> Modifier {
        self.then_element(DerivedSizeElement { insets, side: InsetSide::Top })
    }

    /// As high as `insets` at the bottom (Compose's `windowInsetsBottomHeight`).
    fn window_insets_bottom_height(self, insets: WindowInsets) -> Modifier {
        self.then_element(DerivedSizeElement { insets, side: InsetSide::Bottom })
    }
}

/// Compose's DerivedWidthModifierElement and DerivedHeightModifierElement: an insets node of the size kind.
#[derive(PartialEq)]
struct DerivedSizeElement {
    insets: WindowInsets,
    side: InsetSide,
}

impl ModifierNodeElement for DerivedSizeElement {
    type Node = InsetsConsumingNode;
    fn create(&self) -> InsetsConsumingNode {
        InsetsConsumingNode::new(Kind::Size { insets: self.insets.clone(), side: self.side, left: WindowInsets::Zero })
    }
    fn update(&self, node: &mut InsetsConsumingNode) {
        if let Kind::Size { insets, side, .. } = &node.kind
            && (*insets != self.insets || *side != self.side)
        {
            node.kind = Kind::Size { insets: self.insets.clone(), side: self.side, left: WindowInsets::Zero };
            node.insets_invalidated_now();
        }
    }
    fn name(&self) -> &'static str {
        match self.side {
            InsetSide::Start => "insetsStartWidth",
            InsetSide::End => "insetsEndWidth",
            InsetSide::Top => "insetsTopHeight",
            InsetSide::Bottom => "insetsBottomHeight",
        }
    }
}
