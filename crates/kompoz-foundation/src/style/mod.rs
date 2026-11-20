//! Styles, as Compose's experimental `androidx.compose.foundation.style` (1.12; Compose's
//! `@ExperimentalFoundationStyleApi`): a [`Style`] describes how a node looks and is laid out — sizes, paddings,
//! position, background, border, shape, shadows, the layer's properties, the text's properties the text inside
//! inherits — as a block run on a [`StyleScope`]; it applies only in a state (`pressed`, `hovered`, `focused`,
//! `selected`, `checked`, `disabled`, custom [`StyleStateKey`]s) where it says so, animates where it says so
//! (`animate`), and is applied to a layout by [`Modifier.styleable`](StyleModifiers::styleable) with the
//! component's [`StyleState`] (a [`MutableStyleState`] following an interaction source's presses, hovers and
//! focuses, its other states set by the component).
//!
//! A style runs in its own observation scope: what it reads changing resolves it again, and only the phases whose
//! properties changed are invalidated — a background redrawn, a padding remeasured —, nothing composed again.
//!
//! ```ignore
//! Box(modifier = Modifier.styleable(
//!     Some(state),
//!     |s: &dyn StyleScope| {
//!         s.background(Color::Blue);
//!         s.size(150.dp());
//!         s.pressed(|| s.animate(|| s.background(Color::Red)));
//!     },
//! ));
//! ```
//!
//! Text properties (`font_size`, `content_color`…) are inherited by `BasicText` inside where
//! [`ComposeFoundationFlags::is_inherited_text_style_enabled`](crate::ComposeFoundationFlags) is on (off by
//! default, as Compose's).

mod animations;
mod modifier;
pub(crate) mod properties;
mod resolved;
mod scope;
mod state;
mod styles;

pub(crate) use modifier::LocalStyleOuter;
pub use modifier::StyleModifiers;
pub use scope::{
    AlphaScope, AnimateStyleScope, AnimateStyleScopeExt, BackgroundScope, BaselineShiftScope, BorderScope,
    BorderScopeExt, ClipScope, ColorFilterScope, ContentColorScope, ContentPaddingScope, ContentPaddingScopeExt,
    CustomStyleScope, CustomStyleScopeExt, DrawStyleScope, ExternalPaddingScope, ExternalPaddingScopeExt,
    FontFamilyScope, FontSizeScope, FontStyleScope, FontSynthesisScope, FontWeightScope, ForegroundScope, HyphensScope,
    LayerStyleScope, LayoutStyleScope, LetterSpacingScope, LineBreakScope, LineHeightScope, MaxSizeScope,
    MaxSizeScopeExt, MinSizeScope, MinSizeScopeExt, PositionScope, RotationScope, RotationScopeExt, ScaleScope,
    ScaleScopeExt, ShadowScope, ShapeScope, SizeScope, SizeScopeExt, StyleLocals, StyleScope, StyleScopeRef,
    StyleStateScope, StyleStateScopeExt, TextAlignScope, TextDecorationScope, TextDirectionScope, TextIndentScope,
    TextMotionScope, TextStyleScope, TextStyleStyleScope, TransformOriginScope, TransformOriginScopeExt,
    TranslationScope, TranslationScopeExt, ZIndexScope,
};
pub use state::{
    MutableStyleState, StyleState, StyleStateInteractionProcessor, StyleStateKey, remember_updated_style_state,
};
pub use styles::{CustomStyle, EmptyStyle, Style, combined_style, combined_style3, combined_styles};

#[cfg(test)]
mod tests;
