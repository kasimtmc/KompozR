//! The scopes a style is run on (Compose's StyleScope.kt): [`StyleScope`] is every property a style can set, made
//! of narrower scopes — padding, size, position, border, background, shape, shadows, the layer's properties, the
//! text's — that a [`CustomStyle`](super::CustomStyle)'s scope can pick from; each scope's Kotlin extension
//! functions (`size(…)`, `border(…)`, `contentPadding(…)`, `pressed { … }`, `animate { … }`…) are its `…Ext`
//! trait's methods, there for any scope.
//!
//! A scope is used as `&dyn StyleScope`: its methods take `&self` (a block given to `pressed`, `animate` or
//! `state` sets properties on the same scope, as Kotlin's lambdas do through their outer receiver).

use super::state::{StyleState, StyleStateKey};
use super::styles::CustomStyle;
use crate::layout::PaddingValues;
use kompoz_animation::AnimationSpec;
use kompoz_graphics::{Brush, Color, ColorFilter, Density, Dp, DpSize, Offset, Shadow, Shape, TextUnit};
use kompoz_runtime::KompozitionLocal;
use kompoz_text::{
    BaselineShift, FontFamily, FontStyle, FontSynthesis, FontWeight, Hyphens, LineBreak, TextAlign, TextDecoration,
    TextDirection, TextIndent, TextMotion, TextStyle,
};
use kompoz_ui::NodeHandle;
use kompoz_ui::layout::TransformOrigin;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::state::ToggleableState;
use std::ops::Deref;
use std::sync::Arc;

// ---- The base scopes ------------------------------------------------------------------------------------------

/// Where a style reads composition locals (what Compose's CompositionLocalAccessorScope is to a style scope): the
/// locals of the node the style is applied to, read as its node reads them (Compose's `currentValueOf`), or their
/// defaults outside of one.
#[derive(Clone, Default)]
pub struct StyleLocals {
    node: Option<NodeHandle>,
}

impl StyleLocals {
    /// The locals of `node`.
    pub(crate) fn of(node: Option<NodeHandle>) -> StyleLocals {
        StyleLocals { node }
    }

    /// `local`'s value where the style is applied (Compose's `CompositionLocal.currentValue` in a style).
    pub fn current_value<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T {
        match &self.node {
            Some(node) => node.current_value_of(local),
            None => local.default_value(),
        }
    }
}

/// What every style scope is (Compose's `CustomStyleScope : Density, CompositionLocalAccessorScope`): a density
/// (it derefs to the node's: `16.dp().round_to_px()`) and where composition locals are read
/// ([`current_value`](CustomStyleScopeExt::current_value)). A custom style's scope is a trait extending it, or
/// [`StyleScope`].
pub trait CustomStyleScope: Deref<Target = Density> {
    /// Where its composition locals are read.
    #[doc(hidden)]
    fn style_locals(&self) -> StyleLocals;
}

/// Compose's extensions of a [`CustomStyleScope`].
pub trait CustomStyleScopeExt: CustomStyleScope {
    /// `local`'s value where the style is applied (Compose's `CompositionLocal.currentValue`, an extension
    /// property in the scope: Kotlin's `LocalX.currentValue`); a state it holds read as the style's other reads
    /// are.
    fn current_value<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T {
        self.style_locals().current_value(local)
    }

    /// Applies `style` to this scope (Compose's `ScopeT.apply(style)`): a custom style on its scope, a
    /// [`Style`](super::Style) (as `&dyn Style`) on a style scope.
    ///
    /// Kotlin: `apply(style: StyleT)`
    fn apply_custom_style<St: CustomStyle<Self> + ?Sized>(&self, style: &St) {
        style.apply_style(self);
    }
}

impl<S: CustomStyleScope + ?Sized> CustomStyleScopeExt for S {}

/// What gives a style its component's state (Compose's StyleStateScope): [`state`](StyleStateScope::state), and
/// blocks run only in a state (its `…Ext` trait: `pressed`, `hovered`, `focused`, `selected`, `checked`, `disabled`,
/// the tri-state toggles, and [`state_of`](StyleStateScopeExt::state_of) for custom states).
pub trait StyleStateScope {
    /// The state of the component the style is applied to (Compose's `state`): pressed, hovered, a custom state's
    /// value…
    fn state(&self) -> StyleState;

    /// Runs `block` when `active` says the state is active (Compose's `state(key, block, active)`, the key given to
    /// `active` by [`state_of`](StyleStateScopeExt::state_of)).
    #[doc(hidden)]
    fn state_dyn(&self, block: &mut dyn FnMut(), active: &dyn Fn(&StyleState) -> bool);
}

/// Compose's extensions of a [`StyleStateScope`]: the blocks of a state.
pub trait StyleStateScopeExt: StyleStateScope {
    /// Runs `block` when `active` says the state of `key` is active (Compose's `state(key, block, active)`): what a
    /// custom state's own function (`playing { … }`) is written with.
    ///
    /// Kotlin: `state(key: StyleStateKey<T>, block: () -> Unit, active: (key: StyleStateKey<T>, state: StyleState) -> Boolean)`
    fn state_of<T: Clone + PartialEq + 'static>(
        &self,
        key: &StyleStateKey<T>,
        block: impl FnOnce(),
        active: impl Fn(&StyleStateKey<T>, &StyleState) -> bool,
    ) {
        let mut block = Some(block);
        self.state_dyn(
            &mut || {
                if let Some(b) = block.take() {
                    b()
                }
            },
            &|state| active(key, state),
        );
    }

    /// Runs `block` while the boolean state of `key` is true (Compose's `state(key, block)`).
    ///
    /// Kotlin: `state(key: StyleStateKey<Boolean>, block: () -> Unit)`
    fn state_key(&self, key: &StyleStateKey<bool>, block: impl FnOnce()) {
        self.state_of(key, block, |key, state| state.get(key));
    }

    /// Runs `block` while the component is checked (Compose's `checked`): its toggle on.
    fn checked(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Toggle, block, |_, state| state.is_checked());
    }

    /// Runs `block` while the component is disabled (Compose's `disabled`).
    fn disabled(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Enabled, block, |_, state| !state.is_enabled());
    }

    /// Runs `block` while the component is focused (Compose's `focused`).
    fn focused(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Focused, block, |_, state| state.is_focused());
    }

    /// Runs `block` while the component is hovered (Compose's `hovered`).
    fn hovered(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Hovered, block, |_, state| state.is_hovered());
    }

    /// Runs `block` while the component is pressed (Compose's `pressed`).
    fn pressed(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Pressed, block, |_, state| state.is_pressed());
    }

    /// Runs `block` while the component is selected (Compose's `selected`).
    fn selected(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Selected, block, |_, state| state.is_selected());
    }

    /// Runs `block` while the component's tri-state toggle is on (Compose's `triStateToggleOn`).
    fn tri_state_toggle_on(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Toggle, block, |_, state| state.tri_state_toggle() == ToggleableState::On);
    }

    /// Runs `block` while the component's tri-state toggle is off (Compose's `triStateToggleOff`).
    fn tri_state_toggle_off(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Toggle, block, |_, state| state.tri_state_toggle() == ToggleableState::Off);
    }

    /// Runs `block` while the component's tri-state toggle is indeterminate (Compose's
    /// `triStateToggleIndeterminate`).
    fn tri_state_toggle_indeterminate(&self, block: impl FnOnce()) {
        self.state_of(&StyleStateKey::Toggle, block, |_, state| {
            state.tri_state_toggle() == ToggleableState::Indeterminate
        });
    }
}

impl<S: StyleStateScope + ?Sized> StyleStateScopeExt for S {}

/// The padding inside the component's border (Compose's ContentPaddingScope): part of its size, not inherited.
pub trait ContentPaddingScope {
    fn content_padding_start(&self, value: Dp);
    fn content_padding_end(&self, value: Dp);
    fn content_padding_top(&self, value: Dp);
    fn content_padding_bottom(&self, value: Dp);
}

/// Compose's extensions of a [`ContentPaddingScope`].
pub trait ContentPaddingScopeExt: ContentPaddingScope {
    /// The start and end padding.
    fn content_padding_horizontal(&self, value: Dp) {
        self.content_padding_start(value);
        self.content_padding_end(value);
    }
    /// The top and bottom padding.
    fn content_padding_vertical(&self, value: Dp) {
        self.content_padding_top(value);
        self.content_padding_bottom(value);
    }
    /// Every side's padding.
    fn content_padding(&self, value: Dp) {
        self.content_padding_start(value);
        self.content_padding_end(value);
        self.content_padding_top(value);
        self.content_padding_bottom(value);
    }
    /// Kotlin: `contentPadding(start: Dp, top: Dp, end: Dp, bottom: Dp)`
    fn content_padding_sides(&self, start: Dp, top: Dp, end: Dp, bottom: Dp) {
        self.content_padding_start(start);
        self.content_padding_top(top);
        self.content_padding_end(end);
        self.content_padding_bottom(bottom);
    }
    /// Kotlin: `contentPadding(horizontal: Dp, vertical: Dp)`
    fn content_padding_hv(&self, horizontal: Dp, vertical: Dp) {
        self.content_padding_horizontal(horizontal);
        self.content_padding_vertical(vertical);
    }
    /// The paddings of `padding_values`, its start and end in the layout's direction (Compose's
    /// `contentPadding(paddingValues)`, of a scope that reads locals).
    ///
    /// Kotlin: `contentPadding(paddingValues: PaddingValues)`
    fn content_padding_values(&self, padding_values: PaddingValues)
    where
        Self: CustomStyleScope,
    {
        let direction = self.current_value(&LocalLayoutDirection);
        self.content_padding_sides(
            padding_values.calculate_start_padding(direction),
            padding_values.calculate_top_padding(),
            padding_values.calculate_end_padding(direction),
            padding_values.calculate_bottom_padding(),
        );
    }
}

impl<S: ContentPaddingScope + ?Sized> ContentPaddingScopeExt for S {}

/// The padding outside the component (Compose's ExternalPaddingScope): around its size, not inherited.
pub trait ExternalPaddingScope {
    fn external_padding_start(&self, value: Dp);
    fn external_padding_end(&self, value: Dp);
    fn external_padding_top(&self, value: Dp);
    fn external_padding_bottom(&self, value: Dp);
}

/// Compose's extensions of an [`ExternalPaddingScope`].
pub trait ExternalPaddingScopeExt: ExternalPaddingScope {
    fn external_padding_horizontal(&self, value: Dp) {
        self.external_padding_start(value);
        self.external_padding_end(value);
    }
    fn external_padding_vertical(&self, value: Dp) {
        self.external_padding_top(value);
        self.external_padding_bottom(value);
    }
    fn external_padding(&self, value: Dp) {
        self.external_padding_start(value);
        self.external_padding_end(value);
        self.external_padding_top(value);
        self.external_padding_bottom(value);
    }
    /// Kotlin: `externalPadding(start: Dp, top: Dp, end: Dp, bottom: Dp)`
    fn external_padding_sides(&self, start: Dp, top: Dp, end: Dp, bottom: Dp) {
        self.external_padding_start(start);
        self.external_padding_top(top);
        self.external_padding_end(end);
        self.external_padding_bottom(bottom);
    }
    /// Kotlin: `externalPadding(horizontal: Dp, vertical: Dp)`
    fn external_padding_hv(&self, horizontal: Dp, vertical: Dp) {
        self.external_padding_horizontal(horizontal);
        self.external_padding_vertical(vertical);
    }
    /// Kotlin: `externalPadding(paddingValues: PaddingValues)`
    fn external_padding_values(&self, padding_values: PaddingValues)
    where
        Self: CustomStyleScope,
    {
        let direction = self.current_value(&LocalLayoutDirection);
        self.external_padding_sides(
            padding_values.calculate_start_padding(direction),
            padding_values.calculate_top_padding(),
            padding_values.calculate_end_padding(direction),
            padding_values.calculate_bottom_padding(),
        );
    }
}

impl<S: ExternalPaddingScope + ?Sized> ExternalPaddingScopeExt for S {}

/// The border (Compose's BorderScope): its width (inside the component, padding its content) and its color or
/// brush.
pub trait BorderScope {
    fn border_width(&self, value: Dp);
    fn border_color(&self, value: Color);
    fn border_brush(&self, value: Brush);
}

/// Compose's extensions of a [`BorderScope`].
pub trait BorderScopeExt: BorderScope {
    /// A border `width` wide in `color`.
    fn border(&self, width: Dp, color: Color) {
        self.border_width(width);
        self.border_color(color);
    }
    /// A border `width` wide painted by `brush`.
    ///
    /// Kotlin: `border(width: Dp, brush: Brush)`
    fn border_with_brush(&self, width: Dp, brush: Brush) {
        self.border_width(width);
        self.border_brush(brush);
    }
}

impl<S: BorderScope + ?Sized> BorderScopeExt for S {}

/// The size (Compose's SizeScope): fixed, or a fraction of the room.
pub trait SizeScope {
    fn width(&self, value: Dp);
    fn height(&self, value: Dp);
    /// Kotlin: `width(fraction: Float)`
    fn width_fraction(&self, fraction: f32);
    /// Kotlin: `height(fraction: Float)`
    fn height_fraction(&self, fraction: f32);
}

/// Compose's extensions of a [`SizeScope`].
pub trait SizeScopeExt: SizeScope {
    /// Kotlin: `size(width: Dp, height: Dp)`
    fn size_wh(&self, width: Dp, height: Dp) {
        self.width(width);
        self.height(height);
    }
    /// All the room's height.
    fn fill_height(&self) {
        self.height_fraction(1.0);
    }
    /// All the room.
    fn fill_size(&self) {
        self.width_fraction(1.0);
        self.height_fraction(1.0);
    }
    /// A square.
    fn size(&self, value: Dp) {
        self.width(value);
        self.height(value);
    }
    /// Kotlin: `size(value: DpSize)`
    fn size_dp(&self, value: DpSize) {
        self.width(value.width);
        self.height(value.height);
    }
    /// All the room's width.
    fn fill_width(&self) {
        self.width_fraction(1.0);
    }
}

impl<S: SizeScope + ?Sized> SizeScopeExt for S {}

/// Where it is in the room (Compose's PositionScope): from the left, top, right and bottom.
pub trait PositionScope {
    fn left(&self, value: Dp);
    fn top(&self, value: Dp);
    fn right(&self, value: Dp);
    fn bottom(&self, value: Dp);
}

/// The least size (Compose's MinSizeScope).
pub trait MinSizeScope {
    fn min_width(&self, value: Dp);
    fn min_height(&self, value: Dp);
}

/// Compose's extensions of a [`MinSizeScope`].
pub trait MinSizeScopeExt: MinSizeScope {
    /// Kotlin: `minSize(size: DpSize)`
    fn min_size(&self, size: DpSize) {
        self.min_width(size.width);
        self.min_height(size.height);
    }
    /// Kotlin: `minSize(width: Dp, height: Dp)`
    fn min_size_wh(&self, width: Dp, height: Dp) {
        self.min_width(width);
        self.min_height(height);
    }
}

impl<S: MinSizeScope + ?Sized> MinSizeScopeExt for S {}

/// The greatest size (Compose's MaxSizeScope).
pub trait MaxSizeScope {
    fn max_width(&self, value: Dp);
    fn max_height(&self, value: Dp);
}

/// Compose's extensions of a [`MaxSizeScope`].
pub trait MaxSizeScopeExt: MaxSizeScope {
    /// Kotlin: `maxSize(size: DpSize)`
    fn max_size(&self, size: DpSize) {
        self.max_width(size.width);
        self.max_height(size.height);
    }
    /// Kotlin: `maxSize(width: Dp, height: Dp)`
    fn max_size_wh(&self, width: Dp, height: Dp) {
        self.max_width(width);
        self.max_height(height);
    }
}

impl<S: MaxSizeScope + ?Sized> MaxSizeScopeExt for S {}

/// The layer's alpha (Compose's AlphaScope).
pub trait AlphaScope {
    fn alpha(&self, value: f32);
}

/// The layer's scale (Compose's ScaleScope).
pub trait ScaleScope {
    fn scale_x(&self, value: f32);
    fn scale_y(&self, value: f32);
}

/// Compose's extensions of a [`ScaleScope`].
pub trait ScaleScopeExt: ScaleScope {
    /// Both ways.
    fn scale(&self, value: f32) {
        self.scale_x(value);
        self.scale_y(value);
    }
    /// Kotlin: `scale(x: Float, y: Float)`
    fn scale_xy(&self, x: f32, y: f32) {
        self.scale_x(x);
        self.scale_y(y);
    }
}

impl<S: ScaleScope + ?Sized> ScaleScopeExt for S {}

/// The layer's translation, px (Compose's TranslationScope).
pub trait TranslationScope {
    fn translation_x(&self, value: f32);
    fn translation_y(&self, value: f32);
}

/// Compose's extensions of a [`TranslationScope`].
pub trait TranslationScopeExt: TranslationScope {
    fn translation(&self, x: f32, y: f32) {
        self.translation_x(x);
        self.translation_y(y);
    }
    /// Kotlin: `translation(offset: Offset)`
    fn translation_offset(&self, offset: Offset) {
        self.translation_x(offset.x);
        self.translation_y(offset.y);
    }
}

impl<S: TranslationScope + ?Sized> TranslationScopeExt for S {}

/// The layer's rotation, degrees (Compose's RotationScope).
pub trait RotationScope {
    fn rotation_x(&self, value: f32);
    fn rotation_y(&self, value: f32);
    fn rotation_z(&self, value: f32);
}

/// Compose's extensions of a [`RotationScope`].
pub trait RotationScopeExt: RotationScope {
    fn rotation(&self, x: f32, y: f32, z: f32) {
        self.rotation_x(x);
        self.rotation_y(y);
        self.rotation_z(z);
    }
}

impl<S: RotationScope + ?Sized> RotationScopeExt for S {}

/// The layer's color filter (Compose's ColorFilterScope).
pub trait ColorFilterScope {
    fn color_filter(&self, value: Option<ColorFilter>);
}

/// The layer's transform origin, fractions of its size (Compose's TransformOriginScope).
pub trait TransformOriginScope {
    fn transform_origin_x(&self, value: f32);
    fn transform_origin_y(&self, value: f32);
}

/// Compose's extensions of a [`TransformOriginScope`].
pub trait TransformOriginScopeExt: TransformOriginScope {
    fn transform_origin(&self, value: TransformOrigin) {
        self.transform_origin_x(value.pivot_fraction_x);
        self.transform_origin_y(value.pivot_fraction_y);
    }
}

impl<S: TransformOriginScope + ?Sized> TransformOriginScopeExt for S {}

/// Whether the layer clips to the shape (Compose's ClipScope).
pub trait ClipScope {
    /// Kotlin: `clip(value: Boolean = true)`
    fn clip(&self, value: bool);
}

/// The layer's z index (Compose's ZIndexScope).
pub trait ZIndexScope {
    fn z_index(&self, value: f32);
}

/// What is drawn behind the content, in the shape (Compose's BackgroundScope).
pub trait BackgroundScope {
    fn background(&self, color: Color);
    /// Kotlin: `background(value: Brush)`
    fn background_brush(&self, value: Brush);
}

/// What is drawn over the content, in the shape (Compose's ForegroundScope).
pub trait ForegroundScope {
    fn foreground(&self, value: Color);
    /// Kotlin: `foreground(value: Brush)`
    fn foreground_brush(&self, value: Brush);
}

/// The shape of the background, foreground, border, shadows and clip (Compose's ShapeScope).
pub trait ShapeScope {
    fn shape(&self, value: Arc<dyn Shape>);
}

/// The shadows under and inside the shape (Compose's foundation.style ShadowScope; ui's drop shadow block scope is
/// `kompoz_ui::draw::ShadowScope`).
pub trait ShadowScope {
    fn drop_shadow(&self, value: Shadow);
    /// Kotlin: `dropShadow(vararg value: Shadow)`
    fn drop_shadows(&self, value: &[Shadow]);
    fn inner_shadow(&self, value: Shadow);
    /// Kotlin: `innerShadow(vararg value: Shadow)`
    fn inner_shadows(&self, value: &[Shadow]);
}

/// Animations of the properties set in a block (Compose's AnimateStyleScope).
pub trait AnimateStyleScope {
    /// Runs `block`, the properties it sets animated: to their new values by `to_spec`, back from them by
    /// `from_spec` (Compose's `animate(toSpec, fromSpec, block)`).
    #[doc(hidden)]
    fn animate_dyn(
        &self,
        to_spec: Arc<dyn AnimationSpec<f32>>,
        from_spec: Arc<dyn AnimationSpec<f32>>,
        block: &mut dyn FnMut(),
    );
}

/// Compose's extensions of an [`AnimateStyleScope`].
pub trait AnimateStyleScopeExt: AnimateStyleScope {
    /// The properties `block` sets animated, to their values by `to_spec` and back by `from_spec` (Compose's
    /// `animate(toSpec, fromSpec, block)`): a change of one of them, in a state or of a state read, animates
    /// rather than jumps.
    ///
    /// Kotlin: `animate(toSpec: AnimationSpec<Float>, fromSpec: AnimationSpec<Float>, block: () -> Unit)`
    fn animate_with(
        &self,
        to_spec: Arc<dyn AnimationSpec<f32>>,
        from_spec: Arc<dyn AnimationSpec<f32>>,
        block: impl FnOnce(),
    ) {
        let mut block = Some(block);
        self.animate_dyn(to_spec, from_spec, &mut || {
            if let Some(b) = block.take() {
                b()
            }
        });
    }
    /// The same by Compose's default spring both ways.
    fn animate(&self, block: impl FnOnce()) {
        self.animate_with(super::animations::default_spring_spec(), super::animations::default_spring_spec(), block);
    }
    /// The same by `spec` both ways.
    ///
    /// Kotlin: `animate(spec: AnimationSpec<Float>, block: () -> Unit)`
    fn animate_spec(&self, spec: Arc<dyn AnimationSpec<f32>>, block: impl FnOnce()) {
        self.animate_with(spec.clone(), spec, block);
    }
}

impl<S: AnimateStyleScope + ?Sized> AnimateStyleScopeExt for S {}

/// A whole text style, its properties set one by one (Compose's TextStyleScope).
pub trait TextStyleScope {
    fn text_style(&self, value: TextStyle);
}
/// The text's decoration (Compose's TextDecorationScope).
pub trait TextDecorationScope {
    fn text_decoration(&self, value: TextDecoration);
}
/// The text's font family (Compose's FontFamilyScope).
pub trait FontFamilyScope {
    fn font_family(&self, value: FontFamily);
}
/// The text's indent (Compose's TextIndentScope).
pub trait TextIndentScope {
    fn text_indent(&self, value: TextIndent);
}
/// The text's size (Compose's FontSizeScope).
pub trait FontSizeScope {
    fn font_size(&self, value: TextUnit);
}
/// The text's line height (Compose's LineHeightScope).
pub trait LineHeightScope {
    fn line_height(&self, value: TextUnit);
}
/// The text's letter spacing (Compose's LetterSpacingScope).
pub trait LetterSpacingScope {
    fn letter_spacing(&self, value: TextUnit);
}
/// The text's baseline shift (Compose's BaselineShiftScope).
pub trait BaselineShiftScope {
    fn baseline_shift(&self, value: BaselineShift);
}
/// The text's weight (Compose's FontWeightScope).
pub trait FontWeightScope {
    fn font_weight(&self, value: FontWeight);
}
/// Upright or italic text (Compose's FontStyleScope).
pub trait FontStyleScope {
    fn font_style(&self, value: FontStyle);
}
/// The text's direction (Compose's TextDirectionScope).
pub trait TextDirectionScope {
    fn text_direction(&self, value: TextDirection);
}
/// The text's alignment (Compose's TextAlignScope).
pub trait TextAlignScope {
    fn text_align(&self, value: TextAlign);
}
/// How the text's lines break (Compose's LineBreakScope).
pub trait LineBreakScope {
    fn line_break(&self, value: LineBreak);
}
/// The text's hyphenation (Compose's HyphensScope).
pub trait HyphensScope {
    fn hyphens(&self, value: Hyphens);
}
/// What the text's font may fake (Compose's FontSynthesisScope).
pub trait FontSynthesisScope {
    fn font_synthesis(&self, value: FontSynthesis);
}
/// How the text is rendered as it moves (Compose's TextMotionScope).
pub trait TextMotionScope {
    fn text_motion(&self, value: TextMotion);
}

/// The text's color or brush (Compose's ContentColorScope): inherited by the text inside.
pub trait ContentColorScope {
    fn content_color(&self, value: Color);
    fn content_brush(&self, value: Brush);
}

// ---- The scopes made of them ----------------------------------------------------------------------------------

/// Layout's properties (Compose's LayoutStyleScope).
pub trait LayoutStyleScope:
    ContentPaddingScope + ExternalPaddingScope + SizeScope + PositionScope + MinSizeScope + MaxSizeScope
{
}

impl<S> LayoutStyleScope for S where
    S: ContentPaddingScope + ExternalPaddingScope + SizeScope + PositionScope + MinSizeScope + MaxSizeScope + ?Sized
{
}

/// The layer's properties (Compose's LayerStyleScope).
pub trait LayerStyleScope:
    AlphaScope
    + ScaleScope
    + TranslationScope
    + RotationScope
    + TransformOriginScope
    + ColorFilterScope
    + ClipScope
    + ZIndexScope
{
}

impl<S> LayerStyleScope for S where
    S: AlphaScope
        + ScaleScope
        + TranslationScope
        + RotationScope
        + TransformOriginScope
        + ColorFilterScope
        + ClipScope
        + ZIndexScope
        + ?Sized
{
}

/// Drawing's properties (Compose's DrawStyleScope).
pub trait DrawStyleScope: BorderScope + BackgroundScope + ForegroundScope + ShapeScope + ShadowScope {}

impl<S> DrawStyleScope for S where S: BorderScope + BackgroundScope + ForegroundScope + ShapeScope + ShadowScope + ?Sized
{}

/// Text's properties, inherited by the text inside (Compose's TextStyleStyleScope).
pub trait TextStyleStyleScope:
    TextStyleScope
    + ContentColorScope
    + TextDecorationScope
    + FontFamilyScope
    + TextIndentScope
    + FontSizeScope
    + LineHeightScope
    + LetterSpacingScope
    + BaselineShiftScope
    + FontWeightScope
    + FontStyleScope
    + TextDirectionScope
    + TextAlignScope
    + LineBreakScope
    + HyphensScope
    + TextMotionScope
    + FontSynthesisScope
{
}

impl<S> TextStyleStyleScope for S where
    S: TextStyleScope
        + ContentColorScope
        + TextDecorationScope
        + FontFamilyScope
        + TextIndentScope
        + FontSizeScope
        + LineHeightScope
        + LetterSpacingScope
        + BaselineShiftScope
        + FontWeightScope
        + FontStyleScope
        + TextDirectionScope
        + TextAlignScope
        + LineBreakScope
        + HyphensScope
        + TextMotionScope
        + FontSynthesisScope
        + ?Sized
{
}

/// Every property a [`Style`](super::Style) can set, and the state it is set in (Compose's StyleScope): what a
/// style's block is given, as `&dyn StyleScope`.
pub trait StyleScope:
    CustomStyleScope
    + StyleStateScope
    + AnimateStyleScope
    + LayoutStyleScope
    + LayerStyleScope
    + DrawStyleScope
    + TextStyleStyleScope
{
}

impl<S> StyleScope for S where
    S: CustomStyleScope
        + StyleStateScope
        + AnimateStyleScope
        + LayoutStyleScope
        + LayerStyleScope
        + DrawStyleScope
        + TextStyleStyleScope
        + ?Sized
{
}

// ---- A scope by another --------------------------------------------------------------------------------------

/// A style scope delegating to another (Compose's `object : StyleScope by this, MyScope {}`): what a custom style's
/// scope is made of. Implement the custom scope's trait for it (a trait extending [`StyleScope`] with nothing of
/// its own: `impl MyScope for StyleScopeRef<'_> {}`) and run the custom style on `&StyleScopeRef(scope)`.
#[derive(Clone, Copy)]
pub struct StyleScopeRef<'a>(pub &'a dyn StyleScope);

impl Deref for StyleScopeRef<'_> {
    type Target = Density;
    fn deref(&self) -> &Density {
        self.0
    }
}

macro_rules! delegate {
    ($($trait:ident { $(fn $m:ident(&self $(, $a:ident: $t:ty)*) $(-> $r:ty)?;)* })*) => {
        $(impl $trait for StyleScopeRef<'_> {
            $(fn $m(&self $(, $a: $t)*) $(-> $r)? { self.0.$m($($a),*) })*
        })*
    };
}

delegate! {
    CustomStyleScope { fn style_locals(&self) -> StyleLocals; }
    StyleStateScope {
        fn state(&self) -> StyleState;
        fn state_dyn(&self, block: &mut dyn FnMut(), active: &dyn Fn(&StyleState) -> bool);
    }
    AnimateStyleScope {
        fn animate_dyn(&self, to_spec: Arc<dyn AnimationSpec<f32>>, from_spec: Arc<dyn AnimationSpec<f32>>, block: &mut dyn FnMut());
    }
    ContentPaddingScope {
        fn content_padding_start(&self, value: Dp);
        fn content_padding_end(&self, value: Dp);
        fn content_padding_top(&self, value: Dp);
        fn content_padding_bottom(&self, value: Dp);
    }
    ExternalPaddingScope {
        fn external_padding_start(&self, value: Dp);
        fn external_padding_end(&self, value: Dp);
        fn external_padding_top(&self, value: Dp);
        fn external_padding_bottom(&self, value: Dp);
    }
    SizeScope {
        fn width(&self, value: Dp);
        fn height(&self, value: Dp);
        fn width_fraction(&self, fraction: f32);
        fn height_fraction(&self, fraction: f32);
    }
    PositionScope {
        fn left(&self, value: Dp);
        fn top(&self, value: Dp);
        fn right(&self, value: Dp);
        fn bottom(&self, value: Dp);
    }
    MinSizeScope { fn min_width(&self, value: Dp); fn min_height(&self, value: Dp); }
    MaxSizeScope { fn max_width(&self, value: Dp); fn max_height(&self, value: Dp); }
    AlphaScope { fn alpha(&self, value: f32); }
    ScaleScope { fn scale_x(&self, value: f32); fn scale_y(&self, value: f32); }
    TranslationScope { fn translation_x(&self, value: f32); fn translation_y(&self, value: f32); }
    RotationScope {
        fn rotation_x(&self, value: f32);
        fn rotation_y(&self, value: f32);
        fn rotation_z(&self, value: f32);
    }
    ColorFilterScope { fn color_filter(&self, value: Option<ColorFilter>); }
    TransformOriginScope { fn transform_origin_x(&self, value: f32); fn transform_origin_y(&self, value: f32); }
    ClipScope { fn clip(&self, value: bool); }
    ZIndexScope { fn z_index(&self, value: f32); }
    BorderScope {
        fn border_width(&self, value: Dp);
        fn border_color(&self, value: Color);
        fn border_brush(&self, value: Brush);
    }
    BackgroundScope { fn background(&self, color: Color); fn background_brush(&self, value: Brush); }
    ForegroundScope { fn foreground(&self, value: Color); fn foreground_brush(&self, value: Brush); }
    ShapeScope { fn shape(&self, value: Arc<dyn Shape>); }
    ShadowScope {
        fn drop_shadow(&self, value: Shadow);
        fn drop_shadows(&self, value: &[Shadow]);
        fn inner_shadow(&self, value: Shadow);
        fn inner_shadows(&self, value: &[Shadow]);
    }
    TextStyleScope { fn text_style(&self, value: TextStyle); }
    ContentColorScope { fn content_color(&self, value: Color); fn content_brush(&self, value: Brush); }
    TextDecorationScope { fn text_decoration(&self, value: TextDecoration); }
    FontFamilyScope { fn font_family(&self, value: FontFamily); }
    TextIndentScope { fn text_indent(&self, value: TextIndent); }
    FontSizeScope { fn font_size(&self, value: TextUnit); }
    LineHeightScope { fn line_height(&self, value: TextUnit); }
    LetterSpacingScope { fn letter_spacing(&self, value: TextUnit); }
    BaselineShiftScope { fn baseline_shift(&self, value: BaselineShift); }
    FontWeightScope { fn font_weight(&self, value: FontWeight); }
    FontStyleScope { fn font_style(&self, value: FontStyle); }
    TextDirectionScope { fn text_direction(&self, value: TextDirection); }
    TextAlignScope { fn text_align(&self, value: TextAlign); }
    LineBreakScope { fn line_break(&self, value: LineBreak); }
    HyphensScope { fn hyphens(&self, value: Hyphens); }
    FontSynthesisScope { fn font_synthesis(&self, value: FontSynthesis); }
    TextMotionScope { fn text_motion(&self, value: TextMotion); }
}
