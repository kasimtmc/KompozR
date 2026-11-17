//! foundation.layout, as Compose's `androidx.compose.foundation.layout`: the layouts (`Box`, `Row`, `Column`,
//! `Spacer`, `BoxWithConstraints`), arrangements, the padding, size, offset, aspect ratio, intrinsic size,
//! alignment line padding and scope modifiers, window insets, and fitting inside or outside rulers.
//!
//! Compose's modifiers are extension functions of `Modifier`; here they are methods of extension traits
//! implemented for it ([`PaddingModifiers`], [`SizeModifiers`], [`LayoutScopeModifiers`], …), with Compose's
//! named and defaulted parameters in a kompozable body: `Modifier.padding_sides(start = 8.dp())`. Rust has no
//! overloading, so Compose's overloads have names of their own (each says which it is), and plain code, which
//! can pass only the parameters without a default, has for each modifier with defaults a `…_with` method taking
//! all of them: `Modifier.fill_max_width_with(0.5)`.
//!
//! Compose's `BoxScope`, `RowScope` and `ColumnScope` are receivers of the content lambda; a Rust closure has
//! none, so the content is a plain closure and the scope modifiers (`align`, `weight`, `align_by`, …) are
//! methods of one trait, [`LayoutScopeModifiers`], usable anywhere: one only means something to the parent it is
//! meant for (`align(Alignment)` to a Box, `align(VerticalAlignment)` to a Row, …) and others ignore it, as
//! Compose's parent data does.

mod alignment_line;
mod arrangement;
mod aspect_ratio;
mod box_;
mod box_with_constraints;
mod intrinsic;
mod offset;
mod padding;
mod row_column;
mod ruler_alignment;
mod scope;
mod size;
mod spacer;
mod window_insets;
mod window_insets_connection;
mod window_insets_padding;
mod window_insets_size;

pub use alignment_line::AlignmentLinePaddingModifiers;
pub use arrangement::{
    AbsoluteArrangement, Arrangement, ArrangementAlignment, ArrangementAxis, Horizontal, HorizontalArrangement,
    SpacingAlignment, Vertical, VerticalArrangement,
};
pub use aspect_ratio::AspectRatioModifiers;
pub use box_::Box;
pub use box_with_constraints::{BoxWithConstraints, BoxWithConstraintsScope};
pub use intrinsic::{IntrinsicSize, IntrinsicSizeModifiers};
pub use offset::{OffsetFn, OffsetModifiers};
pub use padding::{CustomPaddingValues, PaddingModifiers, PaddingValues};
pub use row_column::{Column, Row};
pub use ruler_alignment::RulerAlignmentModifiers;
pub use scope::{AlignBy, LayoutScopeModifiers, ScopeAlignment};
pub use size::SizeModifiers;
pub use spacer::Spacer;
pub use window_insets::{CustomWindowInsets, MutableWindowInsets, WindowInsets, WindowInsetsSides};
pub use window_insets_connection::ImeNestedScrollModifiers;
pub use window_insets_padding::WindowInsetsPaddingModifiers;
pub use window_insets_size::WindowInsetsSizeModifiers;

use kompoz_graphics::Dp;

/// Kotlin's `roundToInt` on a float (Math.round: a half up), as kompoz-ui's alignments round.
pub(crate) fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// Compose's `fastCoerceIn`: at least `min`, then at most `max` (never panics, unlike `clamp`).
pub(crate) fn coerce_in(v: i32, min: i32, max: i32) -> i32 {
    v.max(min).min(max)
}

/// Whether two lengths are the same, `Dp::Unspecified` the same as itself (as Compose's `Dp` equality).
pub(crate) fn same_dp(a: Dp, b: Dp) -> bool {
    a == b || a.0.to_bits() == b.0.to_bits()
}
