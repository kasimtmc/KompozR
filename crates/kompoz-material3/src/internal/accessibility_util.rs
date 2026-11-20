//! Larger bounds for accessibility, as Compose's internal AccessibilityUtil.kt: a small component's semantics node
//! made 10 dp bigger each way across (or down), so that a screen reader's box around it is big enough, while it
//! keeps its own size and place ("negative padding").

use kompoz_foundation::layout::PaddingModifiers;
use kompoz_graphics::Dp;
use kompoz_ui::Modifier;

/// What the semantics bounds grow by at the start and at the end (Compose's `HorizontalSemanticsBoundsPadding`).
pub(crate) const HorizontalSemanticsBoundsPadding: Dp = Dp(10.0);

/// What the semantics bounds grow by at the top and at the bottom (Compose's `VerticalSemanticsBoundsPadding`).
pub(crate) const VerticalSemanticsBoundsPadding: Dp = Dp(10.0);

/// The semantics bounds [`HorizontalSemanticsBoundsPadding`] wider at each side, the layout's size and place as
/// they were (Compose's `IncreaseHorizontalSemanticsBounds`: one modifier, the same each time).
pub(crate) fn IncreaseHorizontalSemanticsBounds() -> Modifier {
    thread_local! {
        static MODIFIER: Modifier = Modifier
            .layout(|scope, measurable, constraints| {
                let padding_px = scope.round_to_px(HorizontalSemanticsBoundsPadding);
                // We need to add horizontal padding to the semantics bounds in order to meet screenreader green
                // box minimum size, but we also want to preserve a visual appearance and layout size below that
                // minimum in order to maintain backwards compatibility. This custom layout effectively implements
                // "negative padding".
                let new_constraint = constraints.offset(padding_px * 2, 0);
                let placeable = measurable.measure(new_constraint);
                // But when actually placing the placeable, create the layout without additional space. Place the
                // placeable where it would've been without any extra padding.
                let height = placeable.height();
                let width = placeable.width() - padding_px * 2;
                scope.layout(width, height, move |p| p.place(&placeable, -padding_px, 0))
            })
            .semantics_with(true, |_| {})
            .padding_hv_with(HorizontalSemanticsBoundsPadding, Dp(0.0));
    }
    MODIFIER.with(Modifier::clone)
}

/// The semantics bounds [`VerticalSemanticsBoundsPadding`] taller at the top and the bottom, the layout's size and
/// place as they were (Compose's `IncreaseVerticalSemanticsBounds`: one modifier, the same each time).
pub(crate) fn IncreaseVerticalSemanticsBounds() -> Modifier {
    thread_local! {
        static MODIFIER: Modifier = Modifier
            .layout(|scope, measurable, constraints| {
                let padding_px = scope.round_to_px(VerticalSemanticsBoundsPadding);
                // We need to add vertical padding to the semantics bounds in order to meet screenreader green box
                // minimum size, but we also want to preserve a visual appearance and layout size below that
                // minimum in order to maintain backwards compatibility. This custom layout effectively implements
                // "negative padding".
                let new_constraint = constraints.offset(0, padding_px * 2);
                let placeable = measurable.measure(new_constraint);
                // But when actually placing the placeable, create the layout without additional space. Place the
                // placeable where it would've been without any extra padding.
                let height = placeable.height() - padding_px * 2;
                let width = placeable.width();
                scope.layout(width, height, move |p| p.place(&placeable, 0, -padding_px))
            })
            .semantics_with(true, |_| {})
            .padding_hv_with(Dp(0.0), VerticalSemanticsBoundsPadding);
    }
    MODIFIER.with(Modifier::clone)
}
