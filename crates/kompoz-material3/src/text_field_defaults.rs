//! Material's text fields' defaults, as Compose's TextFieldDefaults.kt: [`TextFieldDefaults`] (the filled field's
//! sizes, shape, colors, paddings, its decorator, decoration box and container), [`OutlinedTextFieldDefaults`] (the
//! outlined field's), [`TextFieldColors`] (a field's colors in each of its states), [`TextFieldLabelPosition`] (where
//! the label goes) and [`TextFieldLabelScope`] (what a label is told as it moves).
//!
//! Compose's composable members of the `object`s are methods of the unit values ([`TextFieldDefaults`]`.Container(…)`,
//! `.decorator(…)`, `.colors(…)`), their composable getters associated functions of no parameter
//! ([`TextFieldDefaults::shape`]); a default naming another parameter (a content padding by the label, a container
//! of the colors and shape given) is an `Option` here, `None` for it. The composables among them delegate to
//! restartable composables of their own (Compose's restart and skip as members of a stable object).

// (Compose's deprecated members, kept public as Compose has them, name their own replacements)
#![allow(deprecated)]

use crate::color_scheme::ColorScheme;
use crate::internal::text_field_impl::{
    CommonDecorationBox, SupportingTopPadding, TextFieldPadding, TextFieldType, ValueDecorationBox,
};
use crate::material_theme::MaterialTheme;
use crate::text_field::{IndicatorLineModifiers, TextFieldWithLabelVerticalPadding};
use crate::tokens::{FilledTextFieldTokens, MotionSchemeKeyTokens, OutlinedTextFieldTokens};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Shape};
use kompoz_runtime::{KotlinProperties, kompozable, kompozable_block, named, named_args, remember_keyed};
use kompoz_text::input::VisualTransformation;
use kompoz_ui::Alignment;
use kompoz_ui::{HorizontalAlignment, Modifier};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// A field's label, given how far it is minimized (Compose's `@Composable TextFieldLabelScope.() -> Unit`).
pub type TextFieldLabel = Rc<dyn Fn(&dyn TextFieldLabelScope)>;

/// A slot of a field (Compose's `@Composable () -> Unit`): an icon, a placeholder, a prefix…
pub type TextFieldSlot = Rc<dyn Fn()>;

/// The filled text field's defaults (Compose's `object TextFieldDefaults`).
pub struct TextFieldDefaults;

#[named]
impl TextFieldDefaults {
    /// The least height of a field (Compose's `MinHeight`): 56 dp.
    pub const MinHeight: Dp = Dp(56.0);

    /// The least width of a field (Compose's `MinWidth`): 280 dp.
    pub const MinWidth: Dp = Dp(280.0);

    /// The indicator line's thickness unfocused (Compose's `UnfocusedIndicatorThickness`): 1 dp.
    pub const UnfocusedIndicatorThickness: Dp = Dp(1.0);

    /// The indicator line's thickness focused (Compose's `FocusedIndicatorThickness`): 2 dp.
    pub const FocusedIndicatorThickness: Dp = Dp(2.0);

    /// Compose's deprecated `UnfocusedBorderThickness`, split into [`TextFieldDefaults::UnfocusedIndicatorThickness`]
    /// and [`OutlinedTextFieldDefaults::UnfocusedBorderThickness`].
    #[deprecated(
        note = "Split into `TextFieldDefaults.UnfocusedIndicatorThickness` and `OutlinedTextFieldDefaults.UnfocusedBorderThickness`. Please update as appropriate."
    )]
    pub const UnfocusedBorderThickness: Dp = TextFieldDefaults::UnfocusedIndicatorThickness;

    /// Compose's deprecated `FocusedBorderThickness`, split into [`TextFieldDefaults::FocusedIndicatorThickness`]
    /// and [`OutlinedTextFieldDefaults::FocusedBorderThickness`].
    #[deprecated(
        note = "Split into `TextFieldDefaults.FocusedIndicatorThickness` and `OutlinedTextFieldDefaults.FocusedBorderThickness`. Please update as appropriate."
    )]
    pub const FocusedBorderThickness: Dp = TextFieldDefaults::FocusedIndicatorThickness;

    /// The filled field's shape (Compose's composable `TextFieldDefaults.shape`): the theme's extra small one, its
    /// top corners rounded.
    pub fn shape() -> Arc<dyn Shape> {
        FilledTextFieldTokens::ContainerShape.value()
    }

    /// Compose's deprecated `outlinedShape`, renamed [`OutlinedTextFieldDefaults::shape`].
    #[deprecated(note = "Renamed to `OutlinedTextFieldDefaults.shape`")]
    pub fn outlined_shape() -> Arc<dyn Shape> {
        OutlinedTextFieldDefaults::shape()
    }

    /// Compose's deprecated `filledShape`, renamed [`TextFieldDefaults::shape`].
    #[deprecated(note = "Renamed to `TextFieldDefaults.shape`")]
    pub fn filled_shape() -> Arc<dyn Shape> {
        TextFieldDefaults::shape()
    }

    /// The decorator of a state-based filled field (Compose's composable `TextFieldDefaults.decorator`): the field
    /// (`state`'s text, through `output_transformation` when given, is what tells empty) with its label (where
    /// `label_position` says), placeholder, icons, prefix and suffix, supporting text, laid out in `content_padding`
    /// (`None`: [`content_padding_without_label`](Self::content_padding_without_label) without a label or with one
    /// above, else [`content_padding_with_label`](Self::content_padding_with_label)) on `container` (`None`: the
    /// [`Container`](Self::Container) of `enabled`, `is_error`, `interaction_source`, `colors` and the
    /// [`shape`](Self::shape)), in `colors`.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn decorator(
        &self,
        state: TextFieldState,
        enabled: bool,
        line_limits: TextFieldLineLimits,
        output_transformation: Option<Rc<dyn OutputTransformation>>,
        interaction_source: InteractionSource,
        #[default(TextFieldLabelPosition::default())] label_position: TextFieldLabelPosition,
        #[default(None)] label: Option<TextFieldLabel>,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(None)] prefix: Option<TextFieldSlot>,
        #[default(None)] suffix: Option<TextFieldSlot>,
        #[default(None)] supporting_text: Option<TextFieldSlot>,
        #[default(false)] is_error: bool,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(None)] content_padding: Option<PaddingValues>,
        #[default(None)] container: Option<TextFieldSlot>,
    ) -> Rc<dyn TextFieldDecorator> {
        kompozable_block!({
            let content_padding = content_padding.unwrap_or_else(|| {
                if label.is_none() || label_position.is_above() {
                    TextFieldDefaults.content_padding_without_label()
                } else {
                    TextFieldDefaults.content_padding_with_label()
                }
            });
            let source = interaction_source.clone();
            let container = container.unwrap_or_else(|| {
                Rc::new(move || {
                    FilledContainer(
                        enabled,
                        is_error,
                        source.clone(),
                        Modifier,
                        colors,
                        TextFieldDefaults::shape(),
                        TextFieldDefaults::FocusedIndicatorThickness,
                        TextFieldDefaults::UnfocusedIndicatorThickness,
                    )
                })
            });
            state_decorator(
                TextFieldType::Filled,
                state,
                enabled,
                line_limits,
                output_transformation,
                interaction_source,
                label_position,
                label,
                [placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text],
                is_error,
                colors,
                content_padding,
                container,
            )
        })
    }

    /// The filled field's container (Compose's composable `TextFieldDefaults.Container`): `colors`' container color
    /// in `shape` (the focused one animated in as `interaction_source` says it is focused, by the theme's fast
    /// effects motion), with the [indicator line](IndicatorLineModifiers::indicator_line) along its bottom.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn Container(
        &self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        #[default(Modifier)] modifier: Modifier,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(TextFieldDefaults::FocusedIndicatorThickness)] focused_indicator_line_thickness: Dp,
        #[default(TextFieldDefaults::UnfocusedIndicatorThickness)] unfocused_indicator_line_thickness: Dp,
    ) {
        kompozable_block!({
            FilledContainer(
                enabled,
                is_error,
                interaction_source,
                modifier,
                colors,
                shape,
                focused_indicator_line_thickness,
                unfocused_indicator_line_thickness,
            );
        })
    }

    /// The decoration box of a value-based filled field (Compose's composable `TextFieldDefaults.DecorationBox`):
    /// what [`decorator`](Self::decorator) makes, around `inner_text_field` (what a BasicTextField's decoration box
    /// is given), `value` through `visual_transformation` telling empty, the label attached; `shape` the default
    /// container's.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn DecorationBox(
        &self,
        value: &str,
        inner_text_field: &dyn Fn(),
        enabled: bool,
        single_line: bool,
        visual_transformation: VisualTransformation,
        interaction_source: InteractionSource,
        #[default(false)] is_error: bool,
        #[default(None)] label: Option<TextFieldSlot>,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(None)] prefix: Option<TextFieldSlot>,
        #[default(None)] suffix: Option<TextFieldSlot>,
        #[default(None)] supporting_text: Option<TextFieldSlot>,
        #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(None)] content_padding: Option<PaddingValues>,
        #[default(None)] container: Option<TextFieldSlot>,
    ) {
        kompozable_block!({
            let content_padding = content_padding.unwrap_or_else(|| {
                if label.is_none() {
                    TextFieldDefaults.content_padding_without_label()
                } else {
                    TextFieldDefaults.content_padding_with_label()
                }
            });
            let source = interaction_source.clone();
            let container = container.unwrap_or_else(|| {
                Rc::new(move || {
                    FilledContainer(
                        enabled,
                        is_error,
                        source.clone(),
                        Modifier,
                        colors,
                        shape.clone(),
                        TextFieldDefaults::FocusedIndicatorThickness,
                        TextFieldDefaults::UnfocusedIndicatorThickness,
                    )
                })
            });
            ValueDecorationBox(
                TextFieldType::Filled,
                value.to_string(),
                inner_text_field,
                enabled,
                single_line,
                visual_transformation,
                interaction_source,
                is_error,
                label,
                [placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text],
                colors,
                content_padding,
                container,
            );
        })
    }

    /// The padding of a filled field's content with a label (Compose's `contentPaddingWithLabel`): 16 dp at the
    /// sides, 8 dp above and below.
    #[allow(clippy::too_many_arguments)]
    pub fn content_padding_with_label(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(TextFieldWithLabelVerticalPadding)] top: Dp,
        #[default(TextFieldWithLabelVerticalPadding)] bottom: Dp,
    ) -> PaddingValues {
        PaddingValues::new(start, top, end, bottom)
    }

    /// The padding of a filled field's content without a label (Compose's `contentPaddingWithoutLabel`): 16 dp all
    /// round.
    pub fn content_padding_without_label(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(TextFieldPadding)] top: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(TextFieldPadding)] bottom: Dp,
    ) -> PaddingValues {
        PaddingValues::new(start, top, end, bottom)
    }

    /// The padding of a field's supporting text (Compose's internal `supportingTextPadding`): 16 dp at the sides,
    /// 4 dp above.
    pub(crate) fn supporting_text_padding(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(SupportingTopPadding)] top: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(Dp(0.0))] bottom: Dp,
    ) -> PaddingValues {
        PaddingValues::new(start, top, end, bottom)
    }

    /// The filled field's colors (Compose's composable `TextFieldDefaults.colors()` and `colors(…)`: one method,
    /// what isn't given — unspecified, `None` — the theme's): the text, container, cursor, selection, indicator,
    /// icons, label, placeholder, supporting text, prefix and suffix colors, each focused, unfocused, disabled and
    /// in error.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] focused_text_color: Color,
        #[default(Color::Unspecified)] unfocused_text_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] error_text_color: Color,
        #[default(Color::Unspecified)] focused_container_color: Color,
        #[default(Color::Unspecified)] unfocused_container_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] error_container_color: Color,
        #[default(Color::Unspecified)] cursor_color: Color,
        #[default(Color::Unspecified)] error_cursor_color: Color,
        #[default(None)] selection_colors: Option<TextSelectionColors>,
        #[default(Color::Unspecified)] focused_indicator_color: Color,
        #[default(Color::Unspecified)] unfocused_indicator_color: Color,
        #[default(Color::Unspecified)] disabled_indicator_color: Color,
        #[default(Color::Unspecified)] error_indicator_color: Color,
        #[default(Color::Unspecified)] focused_leading_icon_color: Color,
        #[default(Color::Unspecified)] unfocused_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] error_leading_icon_color: Color,
        #[default(Color::Unspecified)] focused_trailing_icon_color: Color,
        #[default(Color::Unspecified)] unfocused_trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] error_trailing_icon_color: Color,
        #[default(Color::Unspecified)] focused_label_color: Color,
        #[default(Color::Unspecified)] unfocused_label_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] error_label_color: Color,
        #[default(Color::Unspecified)] focused_placeholder_color: Color,
        #[default(Color::Unspecified)] unfocused_placeholder_color: Color,
        #[default(Color::Unspecified)] disabled_placeholder_color: Color,
        #[default(Color::Unspecified)] error_placeholder_color: Color,
        #[default(Color::Unspecified)] focused_supporting_text_color: Color,
        #[default(Color::Unspecified)] unfocused_supporting_text_color: Color,
        #[default(Color::Unspecified)] disabled_supporting_text_color: Color,
        #[default(Color::Unspecified)] error_supporting_text_color: Color,
        #[default(Color::Unspecified)] focused_prefix_color: Color,
        #[default(Color::Unspecified)] unfocused_prefix_color: Color,
        #[default(Color::Unspecified)] disabled_prefix_color: Color,
        #[default(Color::Unspecified)] error_prefix_color: Color,
        #[default(Color::Unspecified)] focused_suffix_color: Color,
        #[default(Color::Unspecified)] unfocused_suffix_color: Color,
        #[default(Color::Unspecified)] disabled_suffix_color: Color,
        #[default(Color::Unspecified)] error_suffix_color: Color,
    ) -> TextFieldColors {
        default_text_field_colors(&MaterialTheme::color_scheme(), LocalTextSelectionColors.current()).copy_with(
            focused_text_color,
            unfocused_text_color,
            disabled_text_color,
            error_text_color,
            focused_container_color,
            unfocused_container_color,
            disabled_container_color,
            error_container_color,
            cursor_color,
            error_cursor_color,
            selection_colors,
            focused_indicator_color,
            unfocused_indicator_color,
            disabled_indicator_color,
            error_indicator_color,
            focused_leading_icon_color,
            unfocused_leading_icon_color,
            disabled_leading_icon_color,
            error_leading_icon_color,
            focused_trailing_icon_color,
            unfocused_trailing_icon_color,
            disabled_trailing_icon_color,
            error_trailing_icon_color,
            focused_label_color,
            unfocused_label_color,
            disabled_label_color,
            error_label_color,
            focused_placeholder_color,
            unfocused_placeholder_color,
            disabled_placeholder_color,
            error_placeholder_color,
            focused_supporting_text_color,
            unfocused_supporting_text_color,
            disabled_supporting_text_color,
            error_supporting_text_color,
            focused_prefix_color,
            unfocused_prefix_color,
            disabled_prefix_color,
            error_prefix_color,
            focused_suffix_color,
            unfocused_suffix_color,
            disabled_suffix_color,
            error_suffix_color,
        )
    }

    /// Compose's deprecated `ContainerBox`, renamed [`Container`](Self::Container).
    #[deprecated(note = "Renamed to TextFieldDefaults.Container")]
    #[track_caller]
    pub fn ContainerBox(
        &self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        colors: TextFieldColors,
        #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    ) {
        kompozable_block!({
            FilledContainer(
                enabled,
                is_error,
                interaction_source,
                Modifier,
                colors,
                shape,
                TextFieldDefaults::FocusedIndicatorThickness,
                TextFieldDefaults::UnfocusedIndicatorThickness,
            );
        })
    }

    /// Compose's deprecated `textFieldWithLabelPadding`, renamed
    /// [`content_padding_with_label`](Self::content_padding_with_label).
    #[deprecated(note = "Renamed to `TextFieldDefaults.contentPaddingWithLabel`")]
    pub fn text_field_with_label_padding(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(TextFieldWithLabelVerticalPadding)] top: Dp,
        #[default(TextFieldWithLabelVerticalPadding)] bottom: Dp,
    ) -> PaddingValues {
        named_args!(TextFieldDefaults.content_padding_with_label(start = start, top = top, end = end, bottom = bottom))
    }

    /// Compose's deprecated `textFieldWithoutLabelPadding`, renamed
    /// [`content_padding_without_label`](Self::content_padding_without_label).
    #[deprecated(note = "Renamed to `TextFieldDefaults.contentPaddingWithoutLabel`")]
    pub fn text_field_without_label_padding(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(TextFieldPadding)] top: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(TextFieldPadding)] bottom: Dp,
    ) -> PaddingValues {
        named_args!(TextFieldDefaults.content_padding_without_label(
            start = start,
            top = top,
            end = end,
            bottom = bottom
        ))
    }

    /// Compose's deprecated `outlinedTextFieldPadding`, renamed [`OutlinedTextFieldDefaults`]'
    /// [`content_padding`](OutlinedTextFieldDefaults::content_padding).
    #[deprecated(note = "Renamed to `OutlinedTextFieldDefaults.contentPadding`")]
    pub fn outlined_text_field_padding(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(TextFieldPadding)] top: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(TextFieldPadding)] bottom: Dp,
    ) -> PaddingValues {
        named_args!(OutlinedTextFieldDefaults.content_padding(start = start, top = top, end = end, bottom = bottom))
    }
}

/// The outlined text field's defaults (Compose's `object OutlinedTextFieldDefaults`).
pub struct OutlinedTextFieldDefaults;

#[named]
impl OutlinedTextFieldDefaults {
    /// The least height of a field (Compose's `MinHeight`): 56 dp.
    pub const MinHeight: Dp = Dp(56.0);

    /// The least width of a field (Compose's `MinWidth`): 280 dp.
    pub const MinWidth: Dp = Dp(280.0);

    /// The border's thickness unfocused (Compose's `UnfocusedBorderThickness`): 1 dp.
    pub const UnfocusedBorderThickness: Dp = Dp(1.0);

    /// The border's thickness focused (Compose's `FocusedBorderThickness`): 2 dp.
    pub const FocusedBorderThickness: Dp = Dp(2.0);

    /// The outlined field's shape (Compose's composable `OutlinedTextFieldDefaults.shape`): the theme's extra small
    /// one.
    pub fn shape() -> Arc<dyn Shape> {
        OutlinedTextFieldTokens::ContainerShape.value()
    }

    /// The decorator of a state-based outlined field (Compose's composable `OutlinedTextFieldDefaults.decorator`):
    /// as [`TextFieldDefaults`]' [`decorator`](TextFieldDefaults::decorator), the label in the outline when it is
    /// minimized; `content_padding` 16 dp all round unless given, `container` the outlined
    /// [`Container`](Self::Container) unless given.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn decorator(
        &self,
        state: TextFieldState,
        enabled: bool,
        line_limits: TextFieldLineLimits,
        output_transformation: Option<Rc<dyn OutputTransformation>>,
        interaction_source: InteractionSource,
        #[default(TextFieldLabelPosition::default())] label_position: TextFieldLabelPosition,
        #[default(None)] label: Option<TextFieldLabel>,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(None)] prefix: Option<TextFieldSlot>,
        #[default(None)] suffix: Option<TextFieldSlot>,
        #[default(None)] supporting_text: Option<TextFieldSlot>,
        #[default(false)] is_error: bool,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(OutlinedTextFieldDefaults.content_padding())] content_padding: PaddingValues,
        #[default(None)] container: Option<TextFieldSlot>,
    ) -> Rc<dyn TextFieldDecorator> {
        kompozable_block!({
            let source = interaction_source.clone();
            let container = container.unwrap_or_else(|| {
                Rc::new(move || {
                    OutlinedContainer(
                        enabled,
                        is_error,
                        source.clone(),
                        Modifier,
                        colors,
                        OutlinedTextFieldDefaults::shape(),
                        OutlinedTextFieldDefaults::FocusedBorderThickness,
                        OutlinedTextFieldDefaults::UnfocusedBorderThickness,
                    )
                })
            });
            state_decorator(
                TextFieldType::Outlined,
                state,
                enabled,
                line_limits,
                output_transformation,
                interaction_source,
                label_position,
                label,
                [placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text],
                is_error,
                colors,
                content_padding,
                container,
            )
        })
    }

    /// The outlined field's container (Compose's composable `OutlinedTextFieldDefaults.Container`): `colors`'
    /// container color in `shape` with a border of its indicator color, `unfocused_border_thickness` thick (both
    /// the focused ones, `focused_border_thickness` thick, animated in as `interaction_source` says it is focused,
    /// by the theme's fast effects motion).
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn Container(
        &self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        #[default(Modifier)] modifier: Modifier,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(OutlinedTextFieldDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(OutlinedTextFieldDefaults::FocusedBorderThickness)] focused_border_thickness: Dp,
        #[default(OutlinedTextFieldDefaults::UnfocusedBorderThickness)] unfocused_border_thickness: Dp,
    ) {
        kompozable_block!({
            OutlinedContainer(
                enabled,
                is_error,
                interaction_source,
                modifier,
                colors,
                shape,
                focused_border_thickness,
                unfocused_border_thickness,
            );
        })
    }

    /// The decoration box of a value-based outlined field (Compose's composable
    /// `OutlinedTextFieldDefaults.DecorationBox`): as [`TextFieldDefaults`]'
    /// [`DecorationBox`](TextFieldDefaults::DecorationBox), the outlined [`Container`](Self::Container) unless given.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn DecorationBox(
        &self,
        value: &str,
        inner_text_field: &dyn Fn(),
        enabled: bool,
        single_line: bool,
        visual_transformation: VisualTransformation,
        interaction_source: InteractionSource,
        #[default(false)] is_error: bool,
        #[default(None)] label: Option<TextFieldSlot>,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(None)] prefix: Option<TextFieldSlot>,
        #[default(None)] suffix: Option<TextFieldSlot>,
        #[default(None)] supporting_text: Option<TextFieldSlot>,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(OutlinedTextFieldDefaults.content_padding())] content_padding: PaddingValues,
        #[default(None)] container: Option<TextFieldSlot>,
    ) {
        kompozable_block!({
            let source = interaction_source.clone();
            let container = container.unwrap_or_else(|| {
                Rc::new(move || {
                    OutlinedContainer(
                        enabled,
                        is_error,
                        source.clone(),
                        Modifier,
                        colors,
                        OutlinedTextFieldDefaults::shape(),
                        OutlinedTextFieldDefaults::FocusedBorderThickness,
                        OutlinedTextFieldDefaults::UnfocusedBorderThickness,
                    )
                })
            });
            ValueDecorationBox(
                TextFieldType::Outlined,
                value.to_string(),
                inner_text_field,
                enabled,
                single_line,
                visual_transformation,
                interaction_source,
                is_error,
                label,
                [placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text],
                colors,
                content_padding,
                container,
            );
        })
    }

    /// The padding of an outlined field's content (Compose's `contentPadding`): 16 dp all round.
    pub fn content_padding(
        &self,
        #[default(TextFieldPadding)] start: Dp,
        #[default(TextFieldPadding)] top: Dp,
        #[default(TextFieldPadding)] end: Dp,
        #[default(TextFieldPadding)] bottom: Dp,
    ) -> PaddingValues {
        PaddingValues::new(start, top, end, bottom)
    }

    /// The outlined field's colors (Compose's composable `OutlinedTextFieldDefaults.colors()` and `colors(…)`: one
    /// method, what isn't given the theme's): as [`TextFieldDefaults`]' [`colors`](TextFieldDefaults::colors), the
    /// border's for the indicator's, no container.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] focused_text_color: Color,
        #[default(Color::Unspecified)] unfocused_text_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] error_text_color: Color,
        #[default(Color::Unspecified)] focused_container_color: Color,
        #[default(Color::Unspecified)] unfocused_container_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] error_container_color: Color,
        #[default(Color::Unspecified)] cursor_color: Color,
        #[default(Color::Unspecified)] error_cursor_color: Color,
        #[default(None)] selection_colors: Option<TextSelectionColors>,
        #[default(Color::Unspecified)] focused_border_color: Color,
        #[default(Color::Unspecified)] unfocused_border_color: Color,
        #[default(Color::Unspecified)] disabled_border_color: Color,
        #[default(Color::Unspecified)] error_border_color: Color,
        #[default(Color::Unspecified)] focused_leading_icon_color: Color,
        #[default(Color::Unspecified)] unfocused_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] error_leading_icon_color: Color,
        #[default(Color::Unspecified)] focused_trailing_icon_color: Color,
        #[default(Color::Unspecified)] unfocused_trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] error_trailing_icon_color: Color,
        #[default(Color::Unspecified)] focused_label_color: Color,
        #[default(Color::Unspecified)] unfocused_label_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] error_label_color: Color,
        #[default(Color::Unspecified)] focused_placeholder_color: Color,
        #[default(Color::Unspecified)] unfocused_placeholder_color: Color,
        #[default(Color::Unspecified)] disabled_placeholder_color: Color,
        #[default(Color::Unspecified)] error_placeholder_color: Color,
        #[default(Color::Unspecified)] focused_supporting_text_color: Color,
        #[default(Color::Unspecified)] unfocused_supporting_text_color: Color,
        #[default(Color::Unspecified)] disabled_supporting_text_color: Color,
        #[default(Color::Unspecified)] error_supporting_text_color: Color,
        #[default(Color::Unspecified)] focused_prefix_color: Color,
        #[default(Color::Unspecified)] unfocused_prefix_color: Color,
        #[default(Color::Unspecified)] disabled_prefix_color: Color,
        #[default(Color::Unspecified)] error_prefix_color: Color,
        #[default(Color::Unspecified)] focused_suffix_color: Color,
        #[default(Color::Unspecified)] unfocused_suffix_color: Color,
        #[default(Color::Unspecified)] disabled_suffix_color: Color,
        #[default(Color::Unspecified)] error_suffix_color: Color,
    ) -> TextFieldColors {
        default_outlined_text_field_colors(&MaterialTheme::color_scheme(), LocalTextSelectionColors.current())
            .copy_with(
                focused_text_color,
                unfocused_text_color,
                disabled_text_color,
                error_text_color,
                focused_container_color,
                unfocused_container_color,
                disabled_container_color,
                error_container_color,
                cursor_color,
                error_cursor_color,
                selection_colors,
                focused_border_color,
                unfocused_border_color,
                disabled_border_color,
                error_border_color,
                focused_leading_icon_color,
                unfocused_leading_icon_color,
                disabled_leading_icon_color,
                error_leading_icon_color,
                focused_trailing_icon_color,
                unfocused_trailing_icon_color,
                disabled_trailing_icon_color,
                error_trailing_icon_color,
                focused_label_color,
                unfocused_label_color,
                disabled_label_color,
                error_label_color,
                focused_placeholder_color,
                unfocused_placeholder_color,
                disabled_placeholder_color,
                error_placeholder_color,
                focused_supporting_text_color,
                unfocused_supporting_text_color,
                disabled_supporting_text_color,
                error_supporting_text_color,
                focused_prefix_color,
                unfocused_prefix_color,
                disabled_prefix_color,
                error_prefix_color,
                focused_suffix_color,
                unfocused_suffix_color,
                disabled_suffix_color,
                error_suffix_color,
            )
    }

    /// Compose's deprecated `ContainerBox`, renamed [`Container`](Self::Container).
    #[deprecated(note = "Renamed to OutlinedTextFieldDefaults.Container")]
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn ContainerBox(
        &self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        #[default(self.colors())] colors: TextFieldColors,
        #[default(OutlinedTextFieldDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(OutlinedTextFieldDefaults::FocusedBorderThickness)] focused_border_thickness: Dp,
        #[default(OutlinedTextFieldDefaults::UnfocusedBorderThickness)] unfocused_border_thickness: Dp,
    ) {
        kompozable_block!({
            OutlinedContainer(
                enabled,
                is_error,
                interaction_source,
                Modifier,
                colors,
                shape,
                focused_border_thickness,
                unfocused_border_thickness,
            );
        })
    }
}

/// The decorator both state-based kinds make (the bodies of Compose's `decorator`s' `TextFieldDecorator`s): the
/// field's visual text (`state`'s, through `output_transformation`) read as it composes, then the common decoration
/// box. The slots in Compose's order: placeholder, leading and trailing icons, prefix, suffix, supporting text.
#[allow(clippy::too_many_arguments)]
fn state_decorator(
    type_: TextFieldType,
    state: TextFieldState,
    enabled: bool,
    line_limits: TextFieldLineLimits,
    output_transformation: Option<Rc<dyn OutputTransformation>>,
    interaction_source: InteractionSource,
    label_position: TextFieldLabelPosition,
    label: Option<TextFieldLabel>,
    slots: [Option<TextFieldSlot>; 6],
    is_error: bool,
    colors: TextFieldColors,
    content_padding: PaddingValues,
    container: TextFieldSlot,
) -> Rc<dyn TextFieldDecorator> {
    Rc::new(move |inner_text_field: &dyn Fn()| {
        kompozable_block!({
            let visual_text = match &output_transformation {
                None => state.text(),
                Some(output_transformation) => {
                    let mut buffer = state.to_text_field_buffer();
                    output_transformation.transform_output(&mut buffer);
                    buffer.as_char_sequence().to_string()
                }
            };
            let [placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text] = slots.clone();
            CommonDecorationBox(
                type_,
                visual_text,
                inner_text_field,
                label_position.clone(),
                label.clone(),
                placeholder,
                leading_icon,
                trailing_icon,
                prefix,
                suffix,
                supporting_text,
                line_limits == TextFieldLineLimits::SingleLine,
                enabled,
                is_error,
                interaction_source.clone(),
                content_padding.clone(),
                colors,
                container.clone(),
            );
        })
    })
}

/// The filled container (the body of Compose's `TextFieldDefaults.Container`): a style of the container color in
/// the shape, the focused color animated in by the style state of `interaction_source` (remembered for it), under
/// the indicator line.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn FilledContainer(
    enabled: bool,
    is_error: bool,
    interaction_source: InteractionSource,
    modifier: Modifier,
    colors: TextFieldColors,
    shape: Arc<dyn Shape>,
    focused_indicator_line_thickness: Dp,
    unfocused_indicator_line_thickness: Dp,
) {
    let source = interaction_source.clone();
    let style_state = remember_keyed(interaction_source.clone(), move || MutableStyleState::new(Some(source)));
    let animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
    let style_shape = shape.clone();
    let style = move |s: &dyn StyleScope| {
        s.shape(style_shape.clone());
        s.background(colors.container_color(enabled, is_error, false));
        s.focused(|| {
            s.animate_spec(animation_spec.clone(), || s.background(colors.container_color(enabled, is_error, true)));
        });
    };
    Box(modifier = modifier.styleable_with(Some(style_state.into()), style).indicator_line_with(
        enabled,
        is_error,
        interaction_source,
        Some(colors),
        Some(shape),
        focused_indicator_line_thickness,
        unfocused_indicator_line_thickness,
    ));
}

/// The outlined container (the body of Compose's `OutlinedTextFieldDefaults.Container`): a style of the container
/// color in the shape with a border of the indicator color, both focused ones animated in by the style state of
/// `interaction_source` (remembered for it).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn OutlinedContainer(
    enabled: bool,
    is_error: bool,
    interaction_source: InteractionSource,
    modifier: Modifier,
    colors: TextFieldColors,
    shape: Arc<dyn Shape>,
    focused_border_thickness: Dp,
    unfocused_border_thickness: Dp,
) {
    let source = interaction_source.clone();
    let style_state = remember_keyed(interaction_source, move || MutableStyleState::new(Some(source)));
    let animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
    let style = move |s: &dyn StyleScope| {
        s.shape(shape.clone());
        s.background(colors.container_color(enabled, is_error, false));
        s.border(unfocused_border_thickness, colors.indicator_color(enabled, is_error, false));
        s.focused(|| {
            s.animate_spec(animation_spec.clone(), || {
                s.background(colors.container_color(enabled, is_error, true));
                s.border(focused_border_thickness, colors.indicator_color(enabled, is_error, true));
            });
        });
    };
    Box(modifier = modifier.styleable_with(Some(style_state.into()), style));
}

/// A color of a scheme's at an alpha (Compose's `fromToken(…).copy(alpha = …)`).
fn with_alpha(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

/// The filled field's colors of `scheme` (Compose's internal `ColorScheme.defaultTextFieldColors`), selecting text
/// in `local_text_selection_colors`. (Compose caches them on the scheme: here they are made as asked; see the
/// crate's notes on [`ColorScheme`].)
pub(crate) fn default_text_field_colors(
    scheme: &ColorScheme,
    local_text_selection_colors: TextSelectionColors,
) -> TextFieldColors {
    TextFieldColors {
        focused_text_color: scheme.from_token(FilledTextFieldTokens::FocusInputColor),
        unfocused_text_color: scheme.from_token(FilledTextFieldTokens::InputColor),
        disabled_text_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledInputColor),
            FilledTextFieldTokens::DisabledInputOpacity,
        ),
        error_text_color: scheme.from_token(FilledTextFieldTokens::ErrorInputColor),
        focused_container_color: scheme.from_token(FilledTextFieldTokens::ContainerColor),
        unfocused_container_color: scheme.from_token(FilledTextFieldTokens::ContainerColor),
        disabled_container_color: scheme.from_token(FilledTextFieldTokens::ContainerColor),
        error_container_color: scheme.from_token(FilledTextFieldTokens::ContainerColor),
        cursor_color: scheme.from_token(FilledTextFieldTokens::CaretColor),
        error_cursor_color: scheme.from_token(FilledTextFieldTokens::ErrorFocusCaretColor),
        text_selection_colors: local_text_selection_colors,
        focused_indicator_color: scheme.from_token(FilledTextFieldTokens::FocusActiveIndicatorColor),
        unfocused_indicator_color: scheme.from_token(FilledTextFieldTokens::ActiveIndicatorColor),
        disabled_indicator_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledActiveIndicatorColor),
            FilledTextFieldTokens::DisabledActiveIndicatorOpacity,
        ),
        error_indicator_color: scheme.from_token(FilledTextFieldTokens::ErrorActiveIndicatorColor),
        focused_leading_icon_color: scheme.from_token(FilledTextFieldTokens::FocusLeadingIconColor),
        unfocused_leading_icon_color: scheme.from_token(FilledTextFieldTokens::LeadingIconColor),
        disabled_leading_icon_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledLeadingIconColor),
            FilledTextFieldTokens::DisabledLeadingIconOpacity,
        ),
        error_leading_icon_color: scheme.from_token(FilledTextFieldTokens::ErrorLeadingIconColor),
        focused_trailing_icon_color: scheme.from_token(FilledTextFieldTokens::FocusTrailingIconColor),
        unfocused_trailing_icon_color: scheme.from_token(FilledTextFieldTokens::TrailingIconColor),
        disabled_trailing_icon_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledTrailingIconColor),
            FilledTextFieldTokens::DisabledTrailingIconOpacity,
        ),
        error_trailing_icon_color: scheme.from_token(FilledTextFieldTokens::ErrorTrailingIconColor),
        focused_label_color: scheme.from_token(FilledTextFieldTokens::FocusLabelColor),
        unfocused_label_color: scheme.from_token(FilledTextFieldTokens::LabelColor),
        disabled_label_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledLabelColor),
            FilledTextFieldTokens::DisabledLabelOpacity,
        ),
        error_label_color: scheme.from_token(FilledTextFieldTokens::ErrorLabelColor),
        focused_placeholder_color: scheme.from_token(FilledTextFieldTokens::InputPlaceholderColor),
        unfocused_placeholder_color: scheme.from_token(FilledTextFieldTokens::InputPlaceholderColor),
        disabled_placeholder_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledInputColor),
            FilledTextFieldTokens::DisabledInputOpacity,
        ),
        error_placeholder_color: scheme.from_token(FilledTextFieldTokens::InputPlaceholderColor),
        focused_supporting_text_color: scheme.from_token(FilledTextFieldTokens::FocusSupportingColor),
        unfocused_supporting_text_color: scheme.from_token(FilledTextFieldTokens::SupportingColor),
        disabled_supporting_text_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::DisabledSupportingColor),
            FilledTextFieldTokens::DisabledSupportingOpacity,
        ),
        error_supporting_text_color: scheme.from_token(FilledTextFieldTokens::ErrorSupportingColor),
        focused_prefix_color: scheme.from_token(FilledTextFieldTokens::InputPrefixColor),
        unfocused_prefix_color: scheme.from_token(FilledTextFieldTokens::InputPrefixColor),
        disabled_prefix_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::InputPrefixColor),
            FilledTextFieldTokens::DisabledInputOpacity,
        ),
        error_prefix_color: scheme.from_token(FilledTextFieldTokens::InputPrefixColor),
        focused_suffix_color: scheme.from_token(FilledTextFieldTokens::InputSuffixColor),
        unfocused_suffix_color: scheme.from_token(FilledTextFieldTokens::InputSuffixColor),
        disabled_suffix_color: with_alpha(
            scheme.from_token(FilledTextFieldTokens::InputSuffixColor),
            FilledTextFieldTokens::DisabledInputOpacity,
        ),
        error_suffix_color: scheme.from_token(FilledTextFieldTokens::InputSuffixColor),
    }
}

/// The outlined field's colors of `scheme` (Compose's internal `ColorScheme.defaultOutlinedTextFieldColors`),
/// selecting text in `local_text_selection_colors` (the one around, which Compose's getter reads).
pub(crate) fn default_outlined_text_field_colors(
    scheme: &ColorScheme,
    local_text_selection_colors: TextSelectionColors,
) -> TextFieldColors {
    TextFieldColors {
        focused_text_color: scheme.from_token(OutlinedTextFieldTokens::FocusInputColor),
        unfocused_text_color: scheme.from_token(OutlinedTextFieldTokens::InputColor),
        disabled_text_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledInputColor),
            OutlinedTextFieldTokens::DisabledInputOpacity,
        ),
        error_text_color: scheme.from_token(OutlinedTextFieldTokens::ErrorInputColor),
        focused_container_color: Color::Transparent,
        unfocused_container_color: Color::Transparent,
        disabled_container_color: Color::Transparent,
        error_container_color: Color::Transparent,
        cursor_color: scheme.from_token(OutlinedTextFieldTokens::CaretColor),
        error_cursor_color: scheme.from_token(OutlinedTextFieldTokens::ErrorFocusCaretColor),
        text_selection_colors: local_text_selection_colors,
        focused_indicator_color: scheme.from_token(OutlinedTextFieldTokens::FocusOutlineColor),
        unfocused_indicator_color: scheme.from_token(OutlinedTextFieldTokens::OutlineColor),
        disabled_indicator_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledOutlineColor),
            OutlinedTextFieldTokens::DisabledOutlineOpacity,
        ),
        error_indicator_color: scheme.from_token(OutlinedTextFieldTokens::ErrorOutlineColor),
        focused_leading_icon_color: scheme.from_token(OutlinedTextFieldTokens::FocusLeadingIconColor),
        unfocused_leading_icon_color: scheme.from_token(OutlinedTextFieldTokens::LeadingIconColor),
        disabled_leading_icon_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledLeadingIconColor),
            OutlinedTextFieldTokens::DisabledLeadingIconOpacity,
        ),
        error_leading_icon_color: scheme.from_token(OutlinedTextFieldTokens::ErrorLeadingIconColor),
        focused_trailing_icon_color: scheme.from_token(OutlinedTextFieldTokens::FocusTrailingIconColor),
        unfocused_trailing_icon_color: scheme.from_token(OutlinedTextFieldTokens::TrailingIconColor),
        disabled_trailing_icon_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledTrailingIconColor),
            OutlinedTextFieldTokens::DisabledTrailingIconOpacity,
        ),
        error_trailing_icon_color: scheme.from_token(OutlinedTextFieldTokens::ErrorTrailingIconColor),
        focused_label_color: scheme.from_token(OutlinedTextFieldTokens::FocusLabelColor),
        unfocused_label_color: scheme.from_token(OutlinedTextFieldTokens::LabelColor),
        disabled_label_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledLabelColor),
            OutlinedTextFieldTokens::DisabledLabelOpacity,
        ),
        error_label_color: scheme.from_token(OutlinedTextFieldTokens::ErrorLabelColor),
        focused_placeholder_color: scheme.from_token(OutlinedTextFieldTokens::InputPlaceholderColor),
        unfocused_placeholder_color: scheme.from_token(OutlinedTextFieldTokens::InputPlaceholderColor),
        disabled_placeholder_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledInputColor),
            OutlinedTextFieldTokens::DisabledInputOpacity,
        ),
        error_placeholder_color: scheme.from_token(OutlinedTextFieldTokens::InputPlaceholderColor),
        focused_supporting_text_color: scheme.from_token(OutlinedTextFieldTokens::FocusSupportingColor),
        unfocused_supporting_text_color: scheme.from_token(OutlinedTextFieldTokens::SupportingColor),
        disabled_supporting_text_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::DisabledSupportingColor),
            OutlinedTextFieldTokens::DisabledSupportingOpacity,
        ),
        error_supporting_text_color: scheme.from_token(OutlinedTextFieldTokens::ErrorSupportingColor),
        focused_prefix_color: scheme.from_token(OutlinedTextFieldTokens::InputPrefixColor),
        unfocused_prefix_color: scheme.from_token(OutlinedTextFieldTokens::InputPrefixColor),
        disabled_prefix_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::InputPrefixColor),
            OutlinedTextFieldTokens::DisabledInputOpacity,
        ),
        error_prefix_color: scheme.from_token(OutlinedTextFieldTokens::InputPrefixColor),
        focused_suffix_color: scheme.from_token(OutlinedTextFieldTokens::InputSuffixColor),
        unfocused_suffix_color: scheme.from_token(OutlinedTextFieldTokens::InputSuffixColor),
        disabled_suffix_color: with_alpha(
            scheme.from_token(OutlinedTextFieldTokens::InputSuffixColor),
            OutlinedTextFieldTokens::DisabledInputOpacity,
        ),
        error_suffix_color: scheme.from_token(OutlinedTextFieldTokens::InputSuffixColor),
    }
}

/// A field's colors in each of its states (Compose's TextFieldColors): its text, container, cursor, text
/// selection, indicator (the filled field's line, the outlined field's border), icons, label, placeholder,
/// supporting text, prefix and suffix colors, each focused, unfocused, disabled and in error. Make them with
/// [`TextFieldDefaults`]' or [`OutlinedTextFieldDefaults`]' `colors(…)`, [`TextFieldColors()`](fn@TextFieldColors)
/// or [`copy`](TextFieldColors::copy).
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct TextFieldColors {
    pub focused_text_color: Color,
    pub unfocused_text_color: Color,
    pub disabled_text_color: Color,
    pub error_text_color: Color,
    pub focused_container_color: Color,
    pub unfocused_container_color: Color,
    pub disabled_container_color: Color,
    pub error_container_color: Color,
    pub cursor_color: Color,
    pub error_cursor_color: Color,
    pub text_selection_colors: TextSelectionColors,
    pub focused_indicator_color: Color,
    pub unfocused_indicator_color: Color,
    pub disabled_indicator_color: Color,
    pub error_indicator_color: Color,
    pub focused_leading_icon_color: Color,
    pub unfocused_leading_icon_color: Color,
    pub disabled_leading_icon_color: Color,
    pub error_leading_icon_color: Color,
    pub focused_trailing_icon_color: Color,
    pub unfocused_trailing_icon_color: Color,
    pub disabled_trailing_icon_color: Color,
    pub error_trailing_icon_color: Color,
    pub focused_label_color: Color,
    pub unfocused_label_color: Color,
    pub disabled_label_color: Color,
    pub error_label_color: Color,
    pub focused_placeholder_color: Color,
    pub unfocused_placeholder_color: Color,
    pub disabled_placeholder_color: Color,
    pub error_placeholder_color: Color,
    pub focused_supporting_text_color: Color,
    pub unfocused_supporting_text_color: Color,
    pub disabled_supporting_text_color: Color,
    pub error_supporting_text_color: Color,
    pub focused_prefix_color: Color,
    pub unfocused_prefix_color: Color,
    pub disabled_prefix_color: Color,
    pub error_prefix_color: Color,
    pub focused_suffix_color: Color,
    pub unfocused_suffix_color: Color,
    pub disabled_suffix_color: Color,
    pub error_suffix_color: Color,
}

/// Colors of every one given (Compose's `TextFieldColors(…)` constructor).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn TextFieldColors(
    focused_text_color: Color,
    unfocused_text_color: Color,
    disabled_text_color: Color,
    error_text_color: Color,
    focused_container_color: Color,
    unfocused_container_color: Color,
    disabled_container_color: Color,
    error_container_color: Color,
    cursor_color: Color,
    error_cursor_color: Color,
    text_selection_colors: TextSelectionColors,
    focused_indicator_color: Color,
    unfocused_indicator_color: Color,
    disabled_indicator_color: Color,
    error_indicator_color: Color,
    focused_leading_icon_color: Color,
    unfocused_leading_icon_color: Color,
    disabled_leading_icon_color: Color,
    error_leading_icon_color: Color,
    focused_trailing_icon_color: Color,
    unfocused_trailing_icon_color: Color,
    disabled_trailing_icon_color: Color,
    error_trailing_icon_color: Color,
    focused_label_color: Color,
    unfocused_label_color: Color,
    disabled_label_color: Color,
    error_label_color: Color,
    focused_placeholder_color: Color,
    unfocused_placeholder_color: Color,
    disabled_placeholder_color: Color,
    error_placeholder_color: Color,
    focused_supporting_text_color: Color,
    unfocused_supporting_text_color: Color,
    disabled_supporting_text_color: Color,
    error_supporting_text_color: Color,
    focused_prefix_color: Color,
    unfocused_prefix_color: Color,
    disabled_prefix_color: Color,
    error_prefix_color: Color,
    focused_suffix_color: Color,
    unfocused_suffix_color: Color,
    disabled_suffix_color: Color,
    error_suffix_color: Color,
) -> TextFieldColors {
    TextFieldColors {
        focused_text_color,
        unfocused_text_color,
        disabled_text_color,
        error_text_color,
        focused_container_color,
        unfocused_container_color,
        disabled_container_color,
        error_container_color,
        cursor_color,
        error_cursor_color,
        text_selection_colors,
        focused_indicator_color,
        unfocused_indicator_color,
        disabled_indicator_color,
        error_indicator_color,
        focused_leading_icon_color,
        unfocused_leading_icon_color,
        disabled_leading_icon_color,
        error_leading_icon_color,
        focused_trailing_icon_color,
        unfocused_trailing_icon_color,
        disabled_trailing_icon_color,
        error_trailing_icon_color,
        focused_label_color,
        unfocused_label_color,
        disabled_label_color,
        error_label_color,
        focused_placeholder_color,
        unfocused_placeholder_color,
        disabled_placeholder_color,
        error_placeholder_color,
        focused_supporting_text_color,
        unfocused_supporting_text_color,
        disabled_supporting_text_color,
        error_supporting_text_color,
        focused_prefix_color,
        unfocused_prefix_color,
        disabled_prefix_color,
        error_prefix_color,
        focused_suffix_color,
        unfocused_suffix_color,
        disabled_suffix_color,
        error_suffix_color,
    }
}

#[named]
impl TextFieldColors {
    /// The same with what is given replaced (Compose's `copy`): an unspecified color, a `None` selection, keeps
    /// its own.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(Color::Unspecified)] focused_text_color: Color,
        #[default(Color::Unspecified)] unfocused_text_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] error_text_color: Color,
        #[default(Color::Unspecified)] focused_container_color: Color,
        #[default(Color::Unspecified)] unfocused_container_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] error_container_color: Color,
        #[default(Color::Unspecified)] cursor_color: Color,
        #[default(Color::Unspecified)] error_cursor_color: Color,
        #[default(None)] text_selection_colors: Option<TextSelectionColors>,
        #[default(Color::Unspecified)] focused_indicator_color: Color,
        #[default(Color::Unspecified)] unfocused_indicator_color: Color,
        #[default(Color::Unspecified)] disabled_indicator_color: Color,
        #[default(Color::Unspecified)] error_indicator_color: Color,
        #[default(Color::Unspecified)] focused_leading_icon_color: Color,
        #[default(Color::Unspecified)] unfocused_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] error_leading_icon_color: Color,
        #[default(Color::Unspecified)] focused_trailing_icon_color: Color,
        #[default(Color::Unspecified)] unfocused_trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] error_trailing_icon_color: Color,
        #[default(Color::Unspecified)] focused_label_color: Color,
        #[default(Color::Unspecified)] unfocused_label_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] error_label_color: Color,
        #[default(Color::Unspecified)] focused_placeholder_color: Color,
        #[default(Color::Unspecified)] unfocused_placeholder_color: Color,
        #[default(Color::Unspecified)] disabled_placeholder_color: Color,
        #[default(Color::Unspecified)] error_placeholder_color: Color,
        #[default(Color::Unspecified)] focused_supporting_text_color: Color,
        #[default(Color::Unspecified)] unfocused_supporting_text_color: Color,
        #[default(Color::Unspecified)] disabled_supporting_text_color: Color,
        #[default(Color::Unspecified)] error_supporting_text_color: Color,
        #[default(Color::Unspecified)] focused_prefix_color: Color,
        #[default(Color::Unspecified)] unfocused_prefix_color: Color,
        #[default(Color::Unspecified)] disabled_prefix_color: Color,
        #[default(Color::Unspecified)] error_prefix_color: Color,
        #[default(Color::Unspecified)] focused_suffix_color: Color,
        #[default(Color::Unspecified)] unfocused_suffix_color: Color,
        #[default(Color::Unspecified)] disabled_suffix_color: Color,
        #[default(Color::Unspecified)] error_suffix_color: Color,
    ) -> TextFieldColors {
        self.copy_with(
            focused_text_color,
            unfocused_text_color,
            disabled_text_color,
            error_text_color,
            focused_container_color,
            unfocused_container_color,
            disabled_container_color,
            error_container_color,
            cursor_color,
            error_cursor_color,
            text_selection_colors,
            focused_indicator_color,
            unfocused_indicator_color,
            disabled_indicator_color,
            error_indicator_color,
            focused_leading_icon_color,
            unfocused_leading_icon_color,
            disabled_leading_icon_color,
            error_leading_icon_color,
            focused_trailing_icon_color,
            unfocused_trailing_icon_color,
            disabled_trailing_icon_color,
            error_trailing_icon_color,
            focused_label_color,
            unfocused_label_color,
            disabled_label_color,
            error_label_color,
            focused_placeholder_color,
            unfocused_placeholder_color,
            disabled_placeholder_color,
            error_placeholder_color,
            focused_supporting_text_color,
            unfocused_supporting_text_color,
            disabled_supporting_text_color,
            error_supporting_text_color,
            focused_prefix_color,
            unfocused_prefix_color,
            disabled_prefix_color,
            error_prefix_color,
            focused_suffix_color,
            unfocused_suffix_color,
            disabled_suffix_color,
            error_suffix_color,
        )
    }

    /// [`copy`](Self::copy) of every parameter (what `colors(…)` take the theme's colors over with).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn copy_with(
        &self,
        focused_text_color: Color,
        unfocused_text_color: Color,
        disabled_text_color: Color,
        error_text_color: Color,
        focused_container_color: Color,
        unfocused_container_color: Color,
        disabled_container_color: Color,
        error_container_color: Color,
        cursor_color: Color,
        error_cursor_color: Color,
        text_selection_colors: Option<TextSelectionColors>,
        focused_indicator_color: Color,
        unfocused_indicator_color: Color,
        disabled_indicator_color: Color,
        error_indicator_color: Color,
        focused_leading_icon_color: Color,
        unfocused_leading_icon_color: Color,
        disabled_leading_icon_color: Color,
        error_leading_icon_color: Color,
        focused_trailing_icon_color: Color,
        unfocused_trailing_icon_color: Color,
        disabled_trailing_icon_color: Color,
        error_trailing_icon_color: Color,
        focused_label_color: Color,
        unfocused_label_color: Color,
        disabled_label_color: Color,
        error_label_color: Color,
        focused_placeholder_color: Color,
        unfocused_placeholder_color: Color,
        disabled_placeholder_color: Color,
        error_placeholder_color: Color,
        focused_supporting_text_color: Color,
        unfocused_supporting_text_color: Color,
        disabled_supporting_text_color: Color,
        error_supporting_text_color: Color,
        focused_prefix_color: Color,
        unfocused_prefix_color: Color,
        disabled_prefix_color: Color,
        error_prefix_color: Color,
        focused_suffix_color: Color,
        unfocused_suffix_color: Color,
        disabled_suffix_color: Color,
        error_suffix_color: Color,
    ) -> TextFieldColors {
        TextFieldColors {
            focused_text_color: focused_text_color.take_or_else(|| self.focused_text_color),
            unfocused_text_color: unfocused_text_color.take_or_else(|| self.unfocused_text_color),
            disabled_text_color: disabled_text_color.take_or_else(|| self.disabled_text_color),
            error_text_color: error_text_color.take_or_else(|| self.error_text_color),
            focused_container_color: focused_container_color.take_or_else(|| self.focused_container_color),
            unfocused_container_color: unfocused_container_color.take_or_else(|| self.unfocused_container_color),
            disabled_container_color: disabled_container_color.take_or_else(|| self.disabled_container_color),
            error_container_color: error_container_color.take_or_else(|| self.error_container_color),
            cursor_color: cursor_color.take_or_else(|| self.cursor_color),
            error_cursor_color: error_cursor_color.take_or_else(|| self.error_cursor_color),
            text_selection_colors: text_selection_colors.unwrap_or(self.text_selection_colors),
            focused_indicator_color: focused_indicator_color.take_or_else(|| self.focused_indicator_color),
            unfocused_indicator_color: unfocused_indicator_color.take_or_else(|| self.unfocused_indicator_color),
            disabled_indicator_color: disabled_indicator_color.take_or_else(|| self.disabled_indicator_color),
            error_indicator_color: error_indicator_color.take_or_else(|| self.error_indicator_color),
            focused_leading_icon_color: focused_leading_icon_color.take_or_else(|| self.focused_leading_icon_color),
            unfocused_leading_icon_color: unfocused_leading_icon_color
                .take_or_else(|| self.unfocused_leading_icon_color),
            disabled_leading_icon_color: disabled_leading_icon_color.take_or_else(|| self.disabled_leading_icon_color),
            error_leading_icon_color: error_leading_icon_color.take_or_else(|| self.error_leading_icon_color),
            focused_trailing_icon_color: focused_trailing_icon_color.take_or_else(|| self.focused_trailing_icon_color),
            unfocused_trailing_icon_color: unfocused_trailing_icon_color
                .take_or_else(|| self.unfocused_trailing_icon_color),
            disabled_trailing_icon_color: disabled_trailing_icon_color
                .take_or_else(|| self.disabled_trailing_icon_color),
            error_trailing_icon_color: error_trailing_icon_color.take_or_else(|| self.error_trailing_icon_color),
            focused_label_color: focused_label_color.take_or_else(|| self.focused_label_color),
            unfocused_label_color: unfocused_label_color.take_or_else(|| self.unfocused_label_color),
            disabled_label_color: disabled_label_color.take_or_else(|| self.disabled_label_color),
            error_label_color: error_label_color.take_or_else(|| self.error_label_color),
            focused_placeholder_color: focused_placeholder_color.take_or_else(|| self.focused_placeholder_color),
            unfocused_placeholder_color: unfocused_placeholder_color.take_or_else(|| self.unfocused_placeholder_color),
            disabled_placeholder_color: disabled_placeholder_color.take_or_else(|| self.disabled_placeholder_color),
            error_placeholder_color: error_placeholder_color.take_or_else(|| self.error_placeholder_color),
            focused_supporting_text_color: focused_supporting_text_color
                .take_or_else(|| self.focused_supporting_text_color),
            unfocused_supporting_text_color: unfocused_supporting_text_color
                .take_or_else(|| self.unfocused_supporting_text_color),
            disabled_supporting_text_color: disabled_supporting_text_color
                .take_or_else(|| self.disabled_supporting_text_color),
            error_supporting_text_color: error_supporting_text_color.take_or_else(|| self.error_supporting_text_color),
            focused_prefix_color: focused_prefix_color.take_or_else(|| self.focused_prefix_color),
            unfocused_prefix_color: unfocused_prefix_color.take_or_else(|| self.unfocused_prefix_color),
            disabled_prefix_color: disabled_prefix_color.take_or_else(|| self.disabled_prefix_color),
            error_prefix_color: error_prefix_color.take_or_else(|| self.error_prefix_color),
            focused_suffix_color: focused_suffix_color.take_or_else(|| self.focused_suffix_color),
            unfocused_suffix_color: unfocused_suffix_color.take_or_else(|| self.unfocused_suffix_color),
            disabled_suffix_color: disabled_suffix_color.take_or_else(|| self.disabled_suffix_color),
            error_suffix_color: error_suffix_color.take_or_else(|| self.error_suffix_color),
        }
    }

    /// The leading icon's color (Compose's `leadingIconColor`): disabled, else in error, else focused, else
    /// unfocused.
    pub fn leading_icon_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_leading_icon_color,
                self.error_leading_icon_color,
                self.focused_leading_icon_color,
                self.unfocused_leading_icon_color,
            ],
        )
    }

    /// The trailing icon's color (Compose's `trailingIconColor`).
    pub fn trailing_icon_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_trailing_icon_color,
                self.error_trailing_icon_color,
                self.focused_trailing_icon_color,
                self.unfocused_trailing_icon_color,
            ],
        )
    }

    /// The indicator's color: the filled field's line, the outlined field's border (Compose's `indicatorColor`).
    pub fn indicator_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_indicator_color,
                self.error_indicator_color,
                self.focused_indicator_color,
                self.unfocused_indicator_color,
            ],
        )
    }

    /// The container's color (Compose's `containerColor`).
    pub fn container_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_container_color,
                self.error_container_color,
                self.focused_container_color,
                self.unfocused_container_color,
            ],
        )
    }

    /// The placeholder's color (Compose's `placeholderColor`).
    pub fn placeholder_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_placeholder_color,
                self.error_placeholder_color,
                self.focused_placeholder_color,
                self.unfocused_placeholder_color,
            ],
        )
    }

    /// The label's color (Compose's `labelColor`).
    pub fn label_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [self.disabled_label_color, self.error_label_color, self.focused_label_color, self.unfocused_label_color],
        )
    }

    /// The text's color (Compose's `textColor`).
    pub fn text_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [self.disabled_text_color, self.error_text_color, self.focused_text_color, self.unfocused_text_color],
        )
    }

    /// The supporting text's color (Compose's `supportingTextColor`).
    pub fn supporting_text_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_supporting_text_color,
                self.error_supporting_text_color,
                self.focused_supporting_text_color,
                self.unfocused_supporting_text_color,
            ],
        )
    }

    /// The prefix's color (Compose's `prefixColor`).
    pub fn prefix_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_prefix_color,
                self.error_prefix_color,
                self.focused_prefix_color,
                self.unfocused_prefix_color,
            ],
        )
    }

    /// The suffix's color (Compose's `suffixColor`).
    pub fn suffix_color(&self, enabled: bool, is_error: bool, focused: bool) -> Color {
        pick(
            enabled,
            is_error,
            focused,
            [
                self.disabled_suffix_color,
                self.error_suffix_color,
                self.focused_suffix_color,
                self.unfocused_suffix_color,
            ],
        )
    }

    /// The cursor's color (Compose's `cursorColor(isError)`; named apart from the `cursor_color` property, as Rust
    /// has no overloads).
    ///
    /// Kotlin: `cursorColor(isError: Boolean): Color`
    pub fn cursor_color_of(&self, is_error: bool) -> Color {
        if is_error { self.error_cursor_color } else { self.cursor_color }
    }
}

/// Compose's `when { !enabled -> disabled; isError -> error; focused -> focused; else -> unfocused }`, of the
/// colors in that order.
fn pick(
    enabled: bool,
    is_error: bool,
    focused: bool,
    [disabled, error, focused_color, unfocused]: [Color; 4],
) -> Color {
    if !enabled {
        disabled
    } else if is_error {
        error
    } else if focused {
        focused_color
    } else {
        unfocused
    }
}

/// Where a field's label goes (Compose's TextFieldLabelPosition): attached to the field — inside it, at the text's
/// place while the field is empty and unfocused (unless `always_minimize`), minimized above the text (the outlined
/// field's in its outline) otherwise, placed across by `expanded_alignment` and `minimized_alignment` — or above
/// it, aligned by `alignment`, always minimized.
#[derive(Clone, PartialEq)]
pub enum TextFieldLabelPosition {
    /// Inside the field (Compose's `TextFieldLabelPosition.Attached`; make it with
    /// [`TextFieldLabelPosition::attached`]).
    Attached {
        always_minimize: bool,
        minimized_alignment: HorizontalAlignment,
        expanded_alignment: HorizontalAlignment,
    },
    /// Above the field (Compose's `TextFieldLabelPosition.Above`; make it with [`TextFieldLabelPosition::above`]).
    Above { alignment: HorizontalAlignment },
}

impl TextFieldLabelPosition {
    /// The label attached to the field (Compose's `TextFieldLabelPosition.Attached(…)`).
    ///
    /// Kotlin: `TextFieldLabelPosition.Attached(alwaysMinimize: Boolean = false, minimizedAlignment: Alignment.Horizontal = Alignment.Start, expandedAlignment: Alignment.Horizontal = Alignment.Start)`
    pub fn attached(
        always_minimize: bool,
        minimized_alignment: HorizontalAlignment,
        expanded_alignment: HorizontalAlignment,
    ) -> TextFieldLabelPosition {
        TextFieldLabelPosition::Attached { always_minimize, minimized_alignment, expanded_alignment }
    }

    /// The label above the field (Compose's `TextFieldLabelPosition.Above(…)`).
    ///
    /// Kotlin: `TextFieldLabelPosition.Above(alignment: Alignment.Horizontal = Alignment.Start)`
    pub fn above(alignment: HorizontalAlignment) -> TextFieldLabelPosition {
        TextFieldLabelPosition::Above { alignment }
    }

    /// Whether it is above the field (Kotlin's `is TextFieldLabelPosition.Above`).
    pub fn is_above(&self) -> bool {
        matches!(self, TextFieldLabelPosition::Above { .. })
    }
}

impl Default for TextFieldLabelPosition {
    /// Compose's default: `TextFieldLabelPosition.Attached()`, start aligned, not always minimized.
    fn default() -> TextFieldLabelPosition {
        TextFieldLabelPosition::attached(false, Alignment::Start, Alignment::Start)
    }
}

/// Compose's toString: `Attached(alwaysMinimize=…, minimizedAlignment=…, expandedAlignment=…)`, `Above(alignment=…)`.
impl fmt::Debug for TextFieldLabelPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TextFieldLabelPosition::Attached { always_minimize, minimized_alignment, expanded_alignment } => write!(
                f,
                "Attached(alwaysMinimize={always_minimize}, minimizedAlignment={minimized_alignment:?}, expandedAlignment={expanded_alignment:?})"
            ),
            TextFieldLabelPosition::Above { alignment } => write!(f, "Above(alignment={alignment:?})"),
        }
    }
}

/// What a field's label is told (Compose's TextFieldLabelScope): how far it is minimized, from 0 (expanded, at the
/// text's place) to 1 (minimized), as it animates — read where it is used (as it lays out or draws) to follow it
/// without composing again.
pub trait TextFieldLabelScope {
    /// From 0 (expanded) to 1 (minimized) (Compose's `labelMinimizedProgress`).
    fn label_minimized_progress(&self) -> f32;
}
