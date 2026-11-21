//! Material's filled text field, as Compose's TextField.kt: a field on a filled container with an indicator line
//! along its bottom, its label inside it (at the text's place while empty and unfocused, minimized above the text
//! otherwise) or above it, a placeholder, icons, a prefix and suffix, a supporting text below — the layout that
//! places them ([`TextFieldLayout`] and its measure policy, intrinsics too) and the indicator line
//! ([`IndicatorLineModifiers::indicator_line`], Compose's `Modifier.indicatorLine` and its node).
//!
//! Compose's overloads by their engine are composables of their own here: [`TextField`] (Kotlin's
//! `TextField(state: TextFieldState, …)`, on foundation's state-based `BasicTextField`), [`StringTextField`]
//! (`TextField(value: String, onValueChange, …)`) and [`ValueTextField`] (`TextField(value: TextFieldValue,
//! onValueChange, …)`), on foundation's `BasicTextFieldString` and `BasicTextFieldValue` — named before the field
//! as Material's other overloads are ([`ClickableSurface`](crate::ClickableSurface)), `TextFieldValue` being the
//! value's own type.

// (the indicator line's trait, made by `#[named]`, declares its methods with all their parameters)
#![allow(clippy::too_many_arguments)]

use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::internal::float_producer::FloatProducer;
use crate::internal::layout_util::{PlaceableOrZero, SubtractConstraintSafely};
use crate::internal::strings::{Strings, get_string};
use crate::internal::text_field_impl::{
    AboveLabelBottomPadding, AboveLabelHorizontalPadding, ContainerId, LabelId, LeadingId, MinSupportingTextLineHeight,
    MinTextLineHeight, PlaceholderId, PrefixId, PrefixSuffixTextPadding, SuffixId, SupportingId, TextFieldId,
    TextFieldImplModifiers, TrailingId, label_min_height, lerp_int, minimized_label_half_height,
    text_field_horizontal_icon_padding,
};
use crate::material_theme::MaterialTheme;
use crate::text::LocalTextStyle;
use crate::text_field_defaults::{
    TextFieldColors, TextFieldDefaults, TextFieldLabel, TextFieldLabelPosition, TextFieldSlot,
    default_text_field_colors,
};
use crate::tokens::{FilledTextFieldTokens, MotionSchemeKeyTokens, MotionTokens};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, AnimationVector4D, ColorVectorConverter, Easing, VectorConverter,
    snap,
};
use kompoz_coroutines::Job;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::input::internal::OnTextFieldLayout;
use kompoz_graphics::LayoutDirection;
use kompoz_graphics::{Color, Dp, Path, PathDirection, PathOperation, Rect, Shape, SolidColor};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, named, named_args, remember, remember_keyed};
use kompoz_text::TextStyle;
use kompoz_text::input::{TextFieldValue, VisualTransformation};
use kompoz_ui::draw::{CacheDrawModifierNode, CacheDrawScope, DrawResult};
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable,
    PlacementScope, layout_into,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{
    Alignment, Constraints, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// A filled text field editing `state` (Compose's `TextField(state: TextFieldState, …)`, on foundation's
/// state-based `BasicTextField`): the text in `text_style` (its color, where it has none, `colors`' text color of
/// the field's state), the cursor in `colors`' cursor color, the selection in its selection colors; at least
/// [`TextFieldDefaults::MinWidth`] by [`TextFieldDefaults::MinHeight`]; decorated by
/// [`TextFieldDefaults`]`.decorator(…)` on its [`Container`](TextFieldDefaults::Container) in `shape`. `label`
/// goes where `label_position` says; `content_padding` `None`: by the label (Compose's default), `scroll_state`
/// `None`: one remembered (Compose's `rememberScrollState()`). In error (`is_error`) it tells accessibility the
/// default error message. The other parameters as `BasicTextField`'s.
///
/// Kotlin: `TextField(state: TextFieldState, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextField(
    state: TextFieldState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] read_only: bool,
    #[default(LocalTextStyle.current())] text_style: TextStyle,
    #[default(TextFieldLabelPosition::default())] label_position: TextFieldLabelPosition,
    #[default(None)] label: Option<TextFieldLabel>,
    #[default(None)] placeholder: Option<TextFieldSlot>,
    #[default(None)] leading_icon: Option<TextFieldSlot>,
    #[default(None)] trailing_icon: Option<TextFieldSlot>,
    #[default(None)] prefix: Option<TextFieldSlot>,
    #[default(None)] suffix: Option<TextFieldSlot>,
    #[default(None)] supporting_text: Option<TextFieldSlot>,
    #[default(false)] is_error: bool,
    #[default(None)] input_transformation: Option<InputTransformation>,
    #[default(None)] output_transformation: Option<Rc<dyn OutputTransformation>>,
    #[default(KeyboardOptions::Default)] keyboard_options: KeyboardOptions,
    #[default(None)] on_keyboard_action: Option<Rc<dyn KeyboardActionHandler>>,
    #[default(TextFieldLineLimits::Default)] line_limits: TextFieldLineLimits,
    #[default(None)] on_text_layout: Option<OnTextFieldLayout>,
    #[default(None)] scroll_state: Option<ScrollState>,
    #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(TextFieldDefaults.colors())] colors: TextFieldColors,
    #[default(None)] content_padding: Option<PaddingValues>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let source = interaction_source.as_interaction_source();
        let container_source = source.clone();
        let container: TextFieldSlot = Rc::new(move || {
            TextFieldDefaults.Container(
                enabled,
                is_error,
                container_source.clone(),
                colors = colors,
                shape = shape.clone(),
            );
        });
        let decorator = TextFieldDefaults.decorator(
            state.clone(),
            enabled,
            line_limits,
            output_transformation.clone(),
            source,
            label_position = label_position.clone(),
            label = label.clone(),
            placeholder = placeholder.clone(),
            leading_icon = leading_icon.clone(),
            trailing_icon = trailing_icon.clone(),
            prefix = prefix.clone(),
            suffix = suffix.clone(),
            supporting_text = supporting_text.clone(),
            is_error = is_error,
            colors = colors,
            content_padding = content_padding.clone(),
            container = Some(container),
        );
        BasicTextField(
            state.clone(),
            modifier = modifier
                .clone()
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(min_width = TextFieldDefaults::MinWidth, min_height = TextFieldDefaults::MinHeight),
            enabled = enabled,
            read_only = read_only,
            text_style = merged_text_style.clone(),
            cursor_brush = SolidColor(colors.cursor_color_of(is_error)),
            keyboard_options = keyboard_options.clone(),
            on_keyboard_action = on_keyboard_action.clone(),
            line_limits = line_limits,
            on_text_layout = on_text_layout.clone(),
            interaction_source = Some(interaction_source.clone()),
            input_transformation = input_transformation.clone(),
            output_transformation = output_transformation.clone(),
            scroll_state = scroll_state.clone(),
            decorator = Some(decorator),
        );
    });
}

/// A filled text field of the text `value` (Compose's `TextField(value: String, onValueChange, …)`, on
/// foundation's `BasicTextFieldString`): each text the user makes given to `on_value_change`; decorated by
/// [`TextFieldDefaults`]`.DecorationBox(…)` (its label attached), its container in `shape`. `max_lines`: one
/// when `single_line`, whatever it says (Compose's default `if (singleLine) 1 else Int.MAX_VALUE`). The other
/// parameters as [`TextField`]'s and `BasicTextFieldString`'s.
///
/// Kotlin: `TextField(value: String, onValueChange: (String) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StringTextField(
    value: impl Into<String>,
    on_value_change: impl Fn(String) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] read_only: bool,
    #[default(LocalTextStyle.current())] text_style: TextStyle,
    #[default(None)] label: Option<TextFieldSlot>,
    #[default(None)] placeholder: Option<TextFieldSlot>,
    #[default(None)] leading_icon: Option<TextFieldSlot>,
    #[default(None)] trailing_icon: Option<TextFieldSlot>,
    #[default(None)] prefix: Option<TextFieldSlot>,
    #[default(None)] suffix: Option<TextFieldSlot>,
    #[default(None)] supporting_text: Option<TextFieldSlot>,
    #[default(false)] is_error: bool,
    #[default(VisualTransformation::None)] visual_transformation: VisualTransformation,
    #[default(KeyboardOptions::Default)] keyboard_options: KeyboardOptions,
    #[default(KeyboardActions::Default)] keyboard_actions: KeyboardActions,
    #[default(false)] single_line: bool,
    #[default(i32::MAX)] max_lines: i32,
    #[default(1)] min_lines: i32,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(TextFieldDefaults.colors())] colors: TextFieldColors,
) {
    let value: String = value;
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    let on_value_change = Rc::new(on_value_change);
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let source = interaction_source.as_interaction_source();
        let (text, transformation) = (value.clone(), visual_transformation.clone());
        let slots = [
            label.clone(),
            placeholder.clone(),
            leading_icon.clone(),
            trailing_icon.clone(),
            prefix.clone(),
            suffix.clone(),
            supporting_text.clone(),
        ];
        let shape = shape.clone();
        let decoration_box: DecorationBox = Rc::new(move |inner_text_field: &dyn Fn()| {
            let [label, placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text] = slots.clone();
            named_args!(TextFieldDefaults.DecorationBox(
                &text,
                inner_text_field,
                enabled,
                single_line,
                transformation.clone(),
                source.clone(),
                placeholder = placeholder,
                label = label,
                leading_icon = leading_icon,
                trailing_icon = trailing_icon,
                prefix = prefix,
                suffix = suffix,
                supporting_text = supporting_text,
                shape = shape.clone(),
                is_error = is_error,
                colors = colors,
            ));
        });
        let on_value_change = on_value_change.clone();
        BasicTextFieldString(
            value.clone(),
            move |it: String| on_value_change(it),
            modifier = modifier
                .clone()
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(min_width = TextFieldDefaults::MinWidth, min_height = TextFieldDefaults::MinHeight),
            enabled = enabled,
            read_only = read_only,
            text_style = merged_text_style.clone(),
            cursor_brush = SolidColor(colors.cursor_color_of(is_error)),
            visual_transformation = visual_transformation.clone(),
            keyboard_options = keyboard_options.clone(),
            keyboard_actions = keyboard_actions.clone(),
            interaction_source = Some(interaction_source.clone()),
            single_line = single_line,
            max_lines = max_lines,
            min_lines = min_lines,
            decoration_box = Some(decoration_box),
        );
    });
}

/// A filled text field of `value` — its text, selection and composition (Compose's `TextField(value:
/// TextFieldValue, onValueChange, …)`, on foundation's `BasicTextFieldValue`): each value the user makes given to
/// `on_value_change`; as [`StringTextField`] otherwise.
///
/// Kotlin: `TextField(value: TextFieldValue, onValueChange: (TextFieldValue) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ValueTextField(
    value: TextFieldValue,
    on_value_change: impl Fn(TextFieldValue) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] read_only: bool,
    #[default(LocalTextStyle.current())] text_style: TextStyle,
    #[default(None)] label: Option<TextFieldSlot>,
    #[default(None)] placeholder: Option<TextFieldSlot>,
    #[default(None)] leading_icon: Option<TextFieldSlot>,
    #[default(None)] trailing_icon: Option<TextFieldSlot>,
    #[default(None)] prefix: Option<TextFieldSlot>,
    #[default(None)] suffix: Option<TextFieldSlot>,
    #[default(None)] supporting_text: Option<TextFieldSlot>,
    #[default(false)] is_error: bool,
    #[default(VisualTransformation::None)] visual_transformation: VisualTransformation,
    #[default(KeyboardOptions::Default)] keyboard_options: KeyboardOptions,
    #[default(KeyboardActions::Default)] keyboard_actions: KeyboardActions,
    #[default(false)] single_line: bool,
    #[default(i32::MAX)] max_lines: i32,
    #[default(1)] min_lines: i32,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(TextFieldDefaults.colors())] colors: TextFieldColors,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    let on_value_change = Rc::new(on_value_change);
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let source = interaction_source.as_interaction_source();
        let (text, transformation) = (value.text().to_string(), visual_transformation.clone());
        let slots = [
            label.clone(),
            placeholder.clone(),
            leading_icon.clone(),
            trailing_icon.clone(),
            prefix.clone(),
            suffix.clone(),
            supporting_text.clone(),
        ];
        let shape = shape.clone();
        let decoration_box: DecorationBox = Rc::new(move |inner_text_field: &dyn Fn()| {
            let [label, placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text] = slots.clone();
            named_args!(TextFieldDefaults.DecorationBox(
                &text,
                inner_text_field,
                enabled,
                single_line,
                transformation.clone(),
                source.clone(),
                placeholder = placeholder,
                label = label,
                leading_icon = leading_icon,
                trailing_icon = trailing_icon,
                prefix = prefix,
                suffix = suffix,
                supporting_text = supporting_text,
                shape = shape.clone(),
                is_error = is_error,
                colors = colors,
            ));
        });
        let on_value_change = on_value_change.clone();
        BasicTextFieldValue(
            value.clone(),
            move |it: TextFieldValue| on_value_change(it),
            modifier = modifier
                .clone()
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(min_width = TextFieldDefaults::MinWidth, min_height = TextFieldDefaults::MinHeight),
            enabled = enabled,
            read_only = read_only,
            text_style = merged_text_style.clone(),
            cursor_brush = SolidColor(colors.cursor_color_of(is_error)),
            visual_transformation = visual_transformation.clone(),
            keyboard_options = keyboard_options.clone(),
            keyboard_actions = keyboard_actions.clone(),
            interaction_source = Some(interaction_source.clone()),
            single_line = single_line,
            max_lines = max_lines,
            min_lines = min_lines,
            decoration_box = Some(decoration_box),
        );
    });
}

/// A field's text style (each field's `textStyle.merge(TextStyle(color = textColor))`): its own color, else
/// `colors`' text color of the field's state (focused as `interaction_source` says, read only then).
#[track_caller]
pub(crate) fn merged_text_style(
    text_style: &TextStyle,
    interaction_source: &MutableInteractionSource,
    colors: &TextFieldColors,
    enabled: bool,
    is_error: bool,
) -> TextStyle {
    let text_color = text_style.color.take_or_else(|| {
        let focused = interaction_source.collect_is_focused_as_state().get();
        colors.text_color(enabled, is_error, focused)
    });
    text_style.merge(&TextStyle { color: text_color, ..TextStyle::Default })
}

// ---- The layout -----------------------------------------------------------------------------------------------

/// The filled field's layout (Compose's internal `TextFieldLayout`): `container` behind; the icons in boxes of the
/// minimum interactive size, centred down the field; the prefix, the text (and the placeholder over it) and the
/// suffix between them, padded by `padding_values` (less the icons' room where there are icons), the label above
/// the text — or above the field —, the supporting text below. The measure policy remembered for what it is made
/// of.
///
/// Inline (Compose's is restartable): the inner field and the container are borrowed for the call.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn TextFieldLayout(
    modifier: Modifier,
    text_field: &dyn Fn(),
    label: Option<TextFieldSlot>,
    placeholder: Option<Rc<dyn Fn(Modifier)>>,
    leading: Option<TextFieldSlot>,
    trailing: Option<TextFieldSlot>,
    prefix: Option<TextFieldSlot>,
    suffix: Option<TextFieldSlot>,
    single_line: bool,
    label_position: TextFieldLabelPosition,
    label_progress: FloatProducer,
    placeholder_alpha: FloatProducer,
    affix_alpha: FloatProducer,
    container: &dyn Fn(),
    supporting: Option<TextFieldSlot>,
    padding_values: PaddingValues,
) {
    let minimized_label_half_height = minimized_label_half_height();
    let keys = (
        single_line,
        label_position.clone(),
        label_progress.clone(),
        placeholder_alpha.clone(),
        affix_alpha.clone(),
        padding_values.clone(),
        minimized_label_half_height,
    );
    let policy = TextFieldMeasurePolicy {
        single_line,
        label_position: label_position.clone(),
        label_progress: label_progress.clone(),
        placeholder_alpha,
        affix_alpha,
        padding_values: padding_values.clone(),
        minimized_label_half_height,
    };
    let measure_policy = remember_keyed(keys, move || Rc::new(policy) as Rc<dyn MeasurePolicy>);
    let layout_direction = LocalLayoutDirection.current();
    layout_into(modifier, measure_policy, false, || {
        container();
        LeadingTrailingBoxes(leading.clone(), trailing.clone());
        let start_text_field_padding = padding_values.calculate_start_padding(layout_direction);
        let end_text_field_padding = padding_values.calculate_end_padding(layout_direction);
        let horizontal_icon_padding = text_field_horizontal_icon_padding();
        let start_padding = if leading.is_some() {
            coerce_at_least_zero(start_text_field_padding - horizontal_icon_padding)
        } else {
            start_text_field_padding
        };
        let end_padding = if trailing.is_some() {
            coerce_at_least_zero(end_text_field_padding - horizontal_icon_padding)
        } else {
            end_text_field_padding
        };
        AffixBoxes(prefix.clone(), suffix.clone(), start_padding, end_padding);
        let label_padding = if label_position.is_above() {
            Modifier.padding_sides(
                start = AboveLabelHorizontalPadding,
                end = AboveLabelHorizontalPadding,
                bottom = AboveLabelBottomPadding,
            )
        } else {
            Modifier.padding_sides(start = start_padding, end = end_padding)
        };
        if let Some(label) = label.clone() {
            let progress = label_progress.clone();
            Box(
                modifier = Modifier
                    .layout_id(LabelId)
                    .text_field_label_min_height(move || label_min_height(progress.invoke()))
                    .wrap_content_height()
                    .then(label_padding),
                content = move || label(),
            );
        }
        let text_padding = text_padding(prefix.is_none(), suffix.is_none(), start_padding, end_padding);
        if let Some(placeholder) = &placeholder {
            placeholder(Modifier.layout_id(PlaceholderId).then(text_padding.clone()));
        }
        Box(
            modifier = Modifier.layout_id(TextFieldId).then(text_padding),
            propagate_min_constraints = true,
            content = || text_field(),
        );
        SupportingBox(supporting.clone());
    });
}

/// The icons' boxes of both layouts: the minimum interactive size, the icon centred.
#[kompozable(inline)]
pub(crate) fn LeadingTrailingBoxes(leading: Option<TextFieldSlot>, trailing: Option<TextFieldSlot>) {
    if let Some(leading) = leading {
        Box(
            modifier = Modifier.layout_id(LeadingId).minimum_interactive_component_size(),
            content_alignment = Alignment::Center,
            content = move || leading(),
        );
    }
    if let Some(trailing) = trailing {
        Box(
            modifier = Modifier.layout_id(TrailingId).minimum_interactive_component_size(),
            content_alignment = Alignment::Center,
            content = move || trailing(),
        );
    }
}

/// The prefix's and suffix's boxes of both layouts: at least a text line tall, 2 dp from the text.
#[kompozable(inline)]
pub(crate) fn AffixBoxes(
    prefix: Option<TextFieldSlot>,
    suffix: Option<TextFieldSlot>,
    start_padding: Dp,
    end_padding: Dp,
) {
    if let Some(prefix) = prefix {
        Box(
            modifier = Modifier
                .layout_id(PrefixId)
                .height_in(min = MinTextLineHeight)
                .wrap_content_height()
                .padding_sides(start = start_padding, end = PrefixSuffixTextPadding),
            content = move || prefix(),
        );
    }
    if let Some(suffix) = suffix {
        Box(
            modifier = Modifier
                .layout_id(SuffixId)
                .height_in(min = MinTextLineHeight)
                .wrap_content_height()
                .padding_sides(start = PrefixSuffixTextPadding, end = end_padding),
            content = move || suffix(),
        );
    }
}

/// The supporting text's box of both layouts: at least its line tall, in [`TextFieldDefaults`]' supporting text
/// padding.
#[kompozable(inline)]
pub(crate) fn SupportingBox(supporting: Option<TextFieldSlot>) {
    if let Some(supporting) = supporting {
        Box(
            modifier = Modifier
                .layout_id(SupportingId)
                .height_in(min = MinSupportingTextLineHeight)
                .wrap_content_height()
                .padding_values(TextFieldDefaults.supporting_text_padding()),
            content = move || supporting(),
        );
    }
}

/// The text's and the placeholder's padding of both layouts: at least a text line tall, the side padding where no
/// prefix or suffix takes it.
pub(crate) fn text_padding(no_prefix: bool, no_suffix: bool, start_padding: Dp, end_padding: Dp) -> Modifier {
    named_args!(Modifier.height_in(min = MinTextLineHeight).wrap_content_height().padding_sides(
        start = if no_prefix { start_padding } else { Dp(0.0) },
        end = if no_suffix { end_padding } else { Dp(0.0) }
    ))
}

/// Kotlin's `Dp.coerceAtLeast(0.dp)`.
pub(crate) fn coerce_at_least_zero(dp: Dp) -> Dp {
    Dp(dp.0.max(0.0))
}

/// The child of `id` among `measurables` (Compose's `fastFirstOrNull { it.layoutId == id }`).
pub(crate) fn find<'a, M: IntrinsicMeasurable + ?Sized>(measurables: &[&'a M], id: &'static str) -> Option<&'a M> {
    measurables.iter().copied().find(|m| m.layout_id().is_some_and(|it| it.is(&id)))
}

/// The child of `id`, which is there (Compose's `fastFirst { it.layoutId == id }`).
pub(crate) fn find_first<'a, M: IntrinsicMeasurable + ?Sized>(measurables: &[&'a M], id: &'static str) -> &'a M {
    find(measurables, id).unwrap_or_else(|| panic!("Collection contains no element matching the predicate ({id})"))
}

/// The filled field's measure policy (Compose's private `TextFieldMeasurePolicy`).
struct TextFieldMeasurePolicy {
    single_line: bool,
    label_position: TextFieldLabelPosition,
    label_progress: FloatProducer,
    placeholder_alpha: FloatProducer,
    affix_alpha: FloatProducer,
    padding_values: PaddingValues,
    minimized_label_half_height: Dp,
}

impl MeasurePolicy for TextFieldMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let label_progress = self.label_progress.invoke();
        let top_padding_value = scope.round_to_px(self.padding_values.calculate_top_padding());
        let bottom_padding_value = scope.round_to_px(self.padding_values.calculate_bottom_padding());

        let mut occupied_space_horizontally = 0;
        let mut occupied_space_vertically = 0;

        let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));

        // measure leading icon
        let leading_placeable = find(measurables, LeadingId).map(|it| it.measure(loose_constraints));
        occupied_space_horizontally += leading_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(leading_placeable.height_or_zero());

        // measure trailing icon
        let trailing_placeable = find(measurables, TrailingId)
            .map(|it| it.measure(loose_constraints.offset(-occupied_space_horizontally, 0)));
        occupied_space_horizontally += trailing_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(trailing_placeable.height_or_zero());

        // measure prefix
        let prefix_placeable =
            find(measurables, PrefixId).map(|it| it.measure(loose_constraints.offset(-occupied_space_horizontally, 0)));
        occupied_space_horizontally += prefix_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(prefix_placeable.height_or_zero());

        // measure suffix
        let suffix_placeable =
            find(measurables, SuffixId).map(|it| it.measure(loose_constraints.offset(-occupied_space_horizontally, 0)));
        occupied_space_horizontally += suffix_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(suffix_placeable.height_or_zero());

        // measure label
        let is_label_above = self.label_position.is_above();
        let label_measurable = find(measurables, LabelId);
        let mut label_placeable: Option<Placeable> = None;
        let label_intrinsic_height = if !is_label_above {
            let label_constraints = loose_constraints.offset(-occupied_space_horizontally, -bottom_padding_value);
            label_placeable = label_measurable.map(|it| it.measure(label_constraints));
            0
        } else {
            // only intrinsic height can be measured here, the label is measured after the width is known
            label_measurable.map_or(0, |it| it.min_intrinsic_height(constraints.min_width))
        };

        // supporting text intrinsic height measurement is required to set the text field's constraints
        let supporting_measurable = find(measurables, SupportingId);
        let supporting_intrinsic_height =
            supporting_measurable.map_or(0, |it| it.min_intrinsic_height(constraints.min_width));

        // measure input field
        let label_height_or_intrinsic = label_placeable.height_or_zero() + label_intrinsic_height;
        let effective_top_offset = top_padding_value + label_height_or_intrinsic;
        let text_field_constraints = named_args!(constraints.copy(min_height = 0)).offset(
            -occupied_space_horizontally,
            -effective_top_offset - bottom_padding_value - supporting_intrinsic_height,
        );
        let text_field_placeable = find_first(measurables, TextFieldId).measure(text_field_constraints);

        // measure placeholder
        let placeholder_constraints = named_args!(text_field_constraints.copy(min_width = 0));
        let placeholder_placeable = find(measurables, PlaceholderId).map(|it| it.measure(placeholder_constraints));

        occupied_space_vertically = occupied_space_vertically.max(
            text_field_placeable.height().max(placeholder_placeable.height_or_zero())
                + effective_top_offset
                + bottom_padding_value,
        );
        let width = calculate_width(
            leading_placeable.width_or_zero(),
            trailing_placeable.width_or_zero(),
            prefix_placeable.width_or_zero(),
            suffix_placeable.width_or_zero(),
            text_field_placeable.width(),
            label_placeable.width_or_zero(),
            placeholder_placeable.width_or_zero(),
            constraints,
        );

        if is_label_above {
            // now that we know the width, measure the label
            let label_constraints =
                named_args!(loose_constraints.copy(max_height = label_intrinsic_height, max_width = width));
            label_placeable = label_measurable.map(|it| it.measure(label_constraints));
        }

        // measure supporting text
        let supporting_constraints = named_args!(
            Constraints::offset(&loose_constraints, 0, -occupied_space_vertically)
                .copy(min_height = 0, max_width = width)
        );
        let supporting_placeable = supporting_measurable.map(|it| it.measure(supporting_constraints));
        let supporting_height = supporting_placeable.height_or_zero();

        let total_height = self.calculate_height(
            scope,
            text_field_placeable.height(),
            label_placeable.height_or_zero(),
            leading_placeable.height_or_zero(),
            trailing_placeable.height_or_zero(),
            prefix_placeable.height_or_zero(),
            suffix_placeable.height_or_zero(),
            placeholder_placeable.height_or_zero(),
            supporting_placeable.height_or_zero(),
            constraints,
            is_label_above,
            label_progress,
        );
        let height =
            total_height - supporting_height - if is_label_above { label_placeable.height_or_zero() } else { 0 };

        let container_placeable = find_first(measurables, ContainerId).measure(Constraints::new(
            if width != Constraints::Infinity { width } else { 0 },
            width,
            if height != Constraints::Infinity { height } else { 0 },
            height,
        ));

        let placed = Placed {
            text_field: text_field_placeable,
            label: label_placeable,
            placeholder: placeholder_placeable,
            leading: leading_placeable,
            trailing: trailing_placeable,
            prefix: prefix_placeable,
            suffix: suffix_placeable,
            container: container_placeable,
            supporting: supporting_placeable,
        };
        let single_line = self.single_line;
        let label_position = self.label_position.clone();
        let (placeholder_alpha, affix_alpha) = (self.placeholder_alpha.clone(), self.affix_alpha.clone());
        let minimized_label_half_height = scope.round_to_px(self.minimized_label_half_height);
        let layout_direction = scope.layout_direction();
        let density = scope.density().density;
        let top_padding = self.padding_values.calculate_top_padding();
        scope.layout(width, total_height, move |p| {
            if let Some(label_placeable) = &placed.label {
                let label_start_y = if is_label_above {
                    0
                } else if single_line {
                    Alignment::CenterVertically.align(label_placeable.height(), height)
                } else {
                    top_padding_value + minimized_label_half_height
                };
                let label_end_y = if is_label_above { 0 } else { top_padding_value };
                place_with_label(
                    p,
                    &placed,
                    label_placeable,
                    &label_position,
                    width,
                    total_height,
                    label_start_y,
                    label_end_y,
                    is_label_above,
                    label_progress,
                    &placeholder_alpha,
                    &affix_alpha,
                    top_padding_value + if is_label_above { 0 } else { label_placeable.height() },
                    layout_direction,
                );
            } else {
                place_without_label(
                    p,
                    &placed,
                    single_line,
                    top_padding,
                    width,
                    total_height,
                    &placeholder_alpha,
                    &affix_alpha,
                    density,
                );
            }
        })
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        self.intrinsic_height(scope, measurables, width, |it, w| it.max_intrinsic_height(w))
    }

    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        self.intrinsic_height(scope, measurables, width, |it, w| it.min_intrinsic_height(w))
    }

    fn max_intrinsic_width(
        &self,
        _: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        intrinsic_width(measurables, height, |it, h| it.max_intrinsic_width(h))
    }

    fn min_intrinsic_width(
        &self,
        _: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        intrinsic_width(measurables, height, |it, h| it.min_intrinsic_width(h))
    }
}

/// What the filled layout measured, to place.
struct Placed {
    text_field: Placeable,
    label: Option<Placeable>,
    placeholder: Option<Placeable>,
    leading: Option<Placeable>,
    trailing: Option<Placeable>,
    prefix: Option<Placeable>,
    suffix: Option<Placeable>,
    container: Placeable,
    supporting: Option<Placeable>,
}

/// The filled layout's intrinsic width (Compose's private `intrinsicWidth`).
fn intrinsic_width(
    measurables: &[&dyn IntrinsicMeasurable],
    height: i32,
    intrinsic_measurer: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
) -> i32 {
    let text_field_width = intrinsic_measurer(find_first(measurables, TextFieldId), height);
    let measure = |id| find(measurables, id).map_or(0, |it| intrinsic_measurer(it, height));
    let label_width = measure(LabelId);
    let trailing_width = measure(TrailingId);
    let prefix_width = measure(PrefixId);
    let suffix_width = measure(SuffixId);
    let leading_width = measure(LeadingId);
    let placeholder_width = measure(PlaceholderId);
    calculate_width(
        leading_width,
        trailing_width,
        prefix_width,
        suffix_width,
        text_field_width,
        label_width,
        placeholder_width,
        Constraints::new(0, Constraints::Infinity, 0, Constraints::Infinity),
    )
}

/// The filled layout's width (Compose's private `calculateWidth`).
#[allow(clippy::too_many_arguments)]
fn calculate_width(
    leading_width: i32,
    trailing_width: i32,
    prefix_width: i32,
    suffix_width: i32,
    text_field_width: i32,
    label_width: i32,
    placeholder_width: i32,
    constraints: Constraints,
) -> i32 {
    let affix_total_width = prefix_width + suffix_width;
    let middle_section =
        (text_field_width + affix_total_width).max(placeholder_width + affix_total_width).max(label_width);
    let wrapped_width = leading_width + middle_section + trailing_width;
    constraints.constrain_width(wrapped_width)
}

impl TextFieldMeasurePolicy {
    /// The filled layout's intrinsic height (Compose's private `intrinsicHeight`).
    fn intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
        intrinsic_measurer: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
    ) -> i32 {
        let mut remaining_width = width;
        let leading_height = find(measurables, LeadingId).map_or(0, |it| {
            remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
            intrinsic_measurer(it, width)
        });
        let trailing_height = find(measurables, TrailingId).map_or(0, |it| {
            remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
            intrinsic_measurer(it, width)
        });
        let label_height = find(measurables, LabelId).map_or(0, |it| intrinsic_measurer(it, remaining_width));
        let prefix_height = find(measurables, PrefixId).map_or(0, |it| {
            let height = intrinsic_measurer(it, remaining_width);
            remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
            height
        });
        let suffix_height = find(measurables, SuffixId).map_or(0, |it| {
            let height = intrinsic_measurer(it, remaining_width);
            remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
            height
        });
        let text_field_height = intrinsic_measurer(find_first(measurables, TextFieldId), remaining_width);
        let placeholder_height =
            find(measurables, PlaceholderId).map_or(0, |it| intrinsic_measurer(it, remaining_width));
        let supporting_height = find(measurables, SupportingId).map_or(0, |it| intrinsic_measurer(it, width));
        self.calculate_height(
            scope,
            text_field_height,
            label_height,
            leading_height,
            trailing_height,
            prefix_height,
            suffix_height,
            placeholder_height,
            supporting_height,
            Constraints::new(0, Constraints::Infinity, 0, Constraints::Infinity),
            self.label_position.is_above(),
            self.label_progress.invoke(),
        )
    }

    /// The filled layout's height (Compose's private `Density.calculateHeight`): the padding, the label's part
    /// not over the text (at least the minimized label's height; the expanded label eased out as it minimizes),
    /// the tallest of the text, placeholder, prefix, suffix and expanded label — at least the icons' —, the label
    /// above and the supporting text.
    #[allow(clippy::too_many_arguments)]
    fn calculate_height(
        &self,
        scope: &MeasureScope,
        text_field_height: i32,
        label_height: i32,
        leading_height: i32,
        trailing_height: i32,
        prefix_height: i32,
        suffix_height: i32,
        placeholder_height: i32,
        supporting_height: i32,
        constraints: Constraints,
        is_label_above: bool,
        label_progress: f32,
    ) -> i32 {
        let vertical_padding = scope
            .round_to_px(self.padding_values.calculate_top_padding() + self.padding_values.calculate_bottom_padding());
        let input_field_height = text_field_height
            .max(placeholder_height)
            .max(prefix_height)
            .max(suffix_height)
            .max(if is_label_above { 0 } else { lerp_int(label_height, 0, label_progress) });
        let has_label = label_height > 0;
        let non_overlapped_label_height = if has_label && !is_label_above {
            // The label's height when it's minimized, or a part of its expanded height eased out as it minimizes
            scope.round_to_px(self.minimized_label_half_height * 2.0).max(lerp_int(
                0,
                label_height,
                MotionTokens::EasingEmphasizedAccelerateCubicBezier.transform(label_progress),
            ))
        } else {
            0
        };
        let middle_section_height = vertical_padding + non_overlapped_label_height + input_field_height;
        constraints.constrain_height(
            if is_label_above { label_height } else { 0 }
                + leading_height.max(trailing_height).max(middle_section_height)
                + supporting_height,
        )
    }
}

/// Places the filled layout's parts with a label (Compose's private `placeWithLabel`): the label lerped from
/// where it starts expanded to where it ends minimized (above: at the top, aligned across the width).
#[allow(clippy::too_many_arguments)]
fn place_with_label(
    p: &PlacementScope,
    placed: &Placed,
    label_placeable: &Placeable,
    label_position: &TextFieldLabelPosition,
    width: i32,
    total_height: i32,
    label_start_y: i32,
    label_end_y: i32,
    is_label_above: bool,
    label_progress: f32,
    placeholder_alpha: &FloatProducer,
    affix_alpha: &FloatProducer,
    text_position: i32,
    layout_direction: LayoutDirection,
) {
    let y_offset = if is_label_above { label_placeable.height() } else { 0 };

    p.place(&placed.container, 0, y_offset);
    let height =
        total_height - placed.supporting.height_or_zero() - if is_label_above { label_placeable.height() } else { 0 };

    if let Some(leading) = &placed.leading {
        p.place_relative(leading, 0, y_offset + Alignment::CenterVertically.align(leading.height(), height));
    }

    let label_y = lerp_int(label_start_y, label_end_y, label_progress);
    if is_label_above {
        let label_x = label_position.minimized_alignment().align(label_placeable.width(), width, layout_direction);
        // Not placeRelative because alignment already handles RTL
        p.place(label_placeable, label_x, label_y);
    } else {
        let left_icon_width = if layout_direction == LayoutDirection::Ltr {
            placed.leading.width_or_zero()
        } else {
            placed.trailing.width_or_zero()
        };
        let space = width - placed.leading.width_or_zero() - placed.trailing.width_or_zero();
        let label_start_x = label_position.expanded_alignment().align(label_placeable.width(), space, layout_direction)
            + left_icon_width;
        let label_end_x = label_position.minimized_alignment().align(label_placeable.width(), space, layout_direction)
            + left_icon_width;
        let label_x = lerp_int(label_start_x, label_end_x, label_progress);
        // Not placeRelative because alignment already handles RTL
        p.place(label_placeable, label_x, label_y);
    }

    if let Some(prefix) = &placed.prefix {
        let alpha = affix_alpha.clone();
        p.place_relative_with_layer(prefix, placed.leading.width_or_zero(), y_offset + text_position, move |l| {
            l.alpha = alpha.invoke()
        });
    }

    let text_horizontal_position = placed.leading.width_or_zero() + placed.prefix.width_or_zero();
    p.place_relative(&placed.text_field, text_horizontal_position, y_offset + text_position);
    if let Some(placeholder) = &placed.placeholder {
        let alpha = placeholder_alpha.clone();
        p.place_relative_with_layer(placeholder, text_horizontal_position, y_offset + text_position, move |l| {
            l.alpha = alpha.invoke()
        });
    }

    if let Some(suffix) = &placed.suffix {
        let alpha = affix_alpha.clone();
        p.place_relative_with_layer(
            suffix,
            width - placed.trailing.width_or_zero() - suffix.width(),
            y_offset + text_position,
            move |l| l.alpha = alpha.invoke(),
        );
    }

    if let Some(trailing) = &placed.trailing {
        p.place_relative(
            trailing,
            width - trailing.width(),
            y_offset + Alignment::CenterVertically.align(trailing.height(), height),
        );
    }

    if let Some(supporting) = &placed.supporting {
        p.place_relative(supporting, 0, y_offset + height);
    }
}

/// Places the filled layout's parts without a label (Compose's private `placeWithoutLabel`): one line centred
/// down the field, several from the top padding.
#[allow(clippy::too_many_arguments)]
fn place_without_label(
    p: &PlacementScope,
    placed: &Placed,
    single_line: bool,
    top_padding: Dp,
    width: i32,
    total_height: i32,
    placeholder_alpha: &FloatProducer,
    affix_alpha: &FloatProducer,
    density: f32,
) {
    p.place(&placed.container, 0, 0);

    let height = total_height - placed.supporting.height_or_zero();
    let top_padding = (top_padding.0 * density + 0.5).floor() as i32;

    if let Some(leading) = &placed.leading {
        p.place_relative(leading, 0, Alignment::CenterVertically.align(leading.height(), height));
    }

    // Single line text field without label places its input center vertically. Multiline text
    // field without label places its input at the top with padding
    let calculate_vertical_position = |placeable: &Placeable| {
        if single_line { Alignment::CenterVertically.align(placeable.height(), height) } else { top_padding }
    };

    if let Some(prefix) = &placed.prefix {
        let alpha = affix_alpha.clone();
        p.place_relative_with_layer(
            prefix,
            placed.leading.width_or_zero(),
            calculate_vertical_position(prefix),
            move |l| l.alpha = alpha.invoke(),
        );
    }

    let text_horizontal_position = placed.leading.width_or_zero() + placed.prefix.width_or_zero();

    p.place_relative(&placed.text_field, text_horizontal_position, calculate_vertical_position(&placed.text_field));

    if let Some(placeholder) = &placed.placeholder {
        let alpha = placeholder_alpha.clone();
        p.place_relative_with_layer(
            placeholder,
            text_horizontal_position,
            calculate_vertical_position(placeholder),
            move |l| l.alpha = alpha.invoke(),
        );
    }

    if let Some(suffix) = &placed.suffix {
        let alpha = affix_alpha.clone();
        p.place_relative_with_layer(
            suffix,
            width - placed.trailing.width_or_zero() - suffix.width(),
            calculate_vertical_position(suffix),
            move |l| l.alpha = alpha.invoke(),
        );
    }

    if let Some(trailing) = &placed.trailing {
        p.place_relative(
            trailing,
            width - trailing.width(),
            Alignment::CenterVertically.align(trailing.height(), height),
        );
    }

    if let Some(supporting) = &placed.supporting {
        p.place_relative(supporting, 0, height);
    }
}

// ---- The indicator line ---------------------------------------------------------------------------------------

/// The filled field's indicator line (Compose's `TextFieldDefaults`' member extension `Modifier.indicatorLine`).
#[named(trait IndicatorLineModifiers)]
impl Modifier {
    /// A line along the bottom of the layout, cut by `text_field_shape` (the filled field's shape where `None`),
    /// in `colors`' indicator color of the field's state (the theme's filled field colors where `None`; focused as
    /// `interaction_source` says), `focused_indicator_line_thickness` thick focused and enabled,
    /// `unfocused_indicator_line_thickness` otherwise; its color and thickness animated by the theme's fast
    /// effects and fast spatial motions (snapped while disabled). Drawn over the content.
    ///
    /// Kotlin: `indicatorLine(enabled: Boolean, isError: Boolean, interactionSource: InteractionSource, colors: TextFieldColors? = null, textFieldShape: Shape? = null, focusedIndicatorLineThickness: Dp = FocusedIndicatorThickness, unfocusedIndicatorLineThickness: Dp = UnfocusedIndicatorThickness)`
    #[allow(clippy::too_many_arguments)]
    fn indicator_line(
        self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        #[default(None)] colors: Option<TextFieldColors>,
        #[default(None)] text_field_shape: Option<Arc<dyn Shape>>,
        #[default(TextFieldDefaults::FocusedIndicatorThickness)] focused_indicator_line_thickness: Dp,
        #[default(TextFieldDefaults::UnfocusedIndicatorThickness)] unfocused_indicator_line_thickness: Dp,
    ) -> Modifier {
        self.indicator_line_with(
            enabled,
            is_error,
            interaction_source,
            colors,
            text_field_shape,
            focused_indicator_line_thickness,
            unfocused_indicator_line_thickness,
        )
    }

    /// [`indicator_line`](IndicatorLineModifiers::indicator_line) with every parameter, for plain code.
    #[allow(clippy::too_many_arguments)]
    fn indicator_line_with(
        self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        colors: Option<TextFieldColors>,
        text_field_shape: Option<Arc<dyn Shape>>,
        focused_indicator_line_thickness: Dp,
        unfocused_indicator_line_thickness: Dp,
    ) -> Modifier {
        self.then_element(IndicatorLineElement {
            enabled,
            is_error,
            interaction_source,
            colors,
            text_field_shape,
            focused_indicator_line_thickness,
            unfocused_indicator_line_thickness,
        })
    }
}

/// Compose's internal `IndicatorLineElement` (a data class: equal by its values).
#[derive(PartialEq)]
struct IndicatorLineElement {
    enabled: bool,
    is_error: bool,
    interaction_source: InteractionSource,
    colors: Option<TextFieldColors>,
    text_field_shape: Option<Arc<dyn Shape>>,
    focused_indicator_line_thickness: Dp,
    unfocused_indicator_line_thickness: Dp,
}

impl ModifierNodeElement for IndicatorLineElement {
    type Node = IndicatorLineNode;

    fn create(&self) -> IndicatorLineNode {
        IndicatorLineNode {
            line: Rc::new(IndicatorLine {
                enabled: Cell::new(self.enabled),
                is_error: Cell::new(self.is_error),
                interaction_source: RefCell::new(self.interaction_source.clone()),
                colors: Cell::new(self.colors),
                shape: RefCell::new(self.text_field_shape.clone()),
                focused_indicator_width: Cell::new(self.focused_indicator_line_thickness),
                unfocused_indicator_width: Cell::new(self.unfocused_indicator_line_thickness),
                focused: Cell::new(false),
                color_animatable: RefCell::new(None),
                // (made with the node: not focused yet)
                width_animatable: Animatable((self.unfocused_indicator_line_thickness, Dp::VectorConverter)),
                handle: RefCell::new(None),
            }),
            draw_with_cache_modifier_node: None,
            track_focus_state_job: None,
        }
    }

    fn update(&self, node: &mut IndicatorLineNode) {
        node.update(
            self.enabled,
            self.is_error,
            self.interaction_source.clone(),
            self.colors,
            self.text_field_shape.clone(),
            self.focused_indicator_line_thickness,
            self.unfocused_indicator_line_thickness,
        );
    }

    fn name(&self) -> &'static str {
        "indicatorLine"
    }
}

/// What the indicator line's node, its coroutines and its drawing share.
struct IndicatorLine {
    enabled: Cell<bool>,
    is_error: Cell<bool>,
    interaction_source: RefCell<InteractionSource>,
    colors: Cell<Option<TextFieldColors>>,
    shape: RefCell<Option<Arc<dyn Shape>>>,
    focused_indicator_width: Cell<Dp>,
    unfocused_indicator_width: Cell<Dp>,
    focused: Cell<bool>,
    color_animatable: RefCell<Option<Animatable<Color, AnimationVector4D>>>,
    width_animatable: Animatable<Dp, AnimationVector1D>,
    handle: RefCell<Option<NodeHandle>>,
}

impl IndicatorLine {
    fn cx(&self) -> Option<NodeCx> {
        self.handle.borrow().as_ref().and_then(NodeHandle::cx)
    }

    /// The colors given, else the theme's filled field colors (as the node reads its composition locals).
    fn colors(&self) -> TextFieldColors {
        if let Some(colors) = self.colors.get() {
            return colors;
        }
        let cx = self.cx().expect("the indicator line reads the theme while attached");
        let theme = cx.current_value_of(MaterialTheme::LocalMaterialTheme());
        default_text_field_colors(&theme.color_scheme, cx.current_value_of(&LocalTextSelectionColors))
    }

    /// The shape given, else the theme's filled field shape.
    fn shape(&self) -> Arc<dyn Shape> {
        if let Some(shape) = self.shape.borrow().clone() {
            return shape;
        }
        let cx = self.cx().expect("the indicator line reads the theme while attached");
        cx.current_value_of(MaterialTheme::LocalMaterialTheme())
            .shapes
            .from_token(FilledTextFieldTokens::ContainerShape)
    }

    /// Animates the color and the thickness to the state's (Compose's private `invalidateIndicator`): what they
    /// go to and how read as the coroutines run.
    fn invalidate_indicator(self: &Rc<IndicatorLine>) {
        let Some(scope) = self.cx().map(|cx| cx.coroutine_scope()) else { return };
        let line = self.clone();
        scope.launch(async move {
            let Some(animatable) = line.color_animatable.borrow().clone() else { return };
            let enabled = line.enabled.get();
            let target_value = line.colors().indicator_color(enabled, line.is_error.get(), line.focused.get());
            let animation_spec: Arc<dyn AnimationSpec<Color>> = if enabled {
                line.motion_spec::<Color>(MotionSchemeKeyTokens::FastEffects)
            } else {
                Arc::new(snap::<Color>())
            };
            named_args!(animatable.animate_to(target_value, animation_spec = animation_spec)).await;
        });
        let line = self.clone();
        scope.launch(async move {
            let enabled = line.enabled.get();
            let target_value = if line.focused.get() && enabled {
                line.focused_indicator_width.get()
            } else {
                line.unfocused_indicator_width.get()
            };
            let animation_spec: Arc<dyn AnimationSpec<Dp>> = if enabled {
                line.motion_spec::<Dp>(MotionSchemeKeyTokens::FastSpatial)
            } else {
                Arc::new(snap::<Dp>())
            };
            let animatable = line.width_animatable.clone();
            named_args!(animatable.animate_to(target_value, animation_spec = animation_spec)).await;
        });
    }

    /// The theme's motion scheme's spec of `token` (Compose's `currentValueOf(LocalMaterialTheme).motionScheme
    /// .fromToken(…)`).
    fn motion_spec<T: 'static>(&self, token: MotionSchemeKeyTokens) -> Arc<dyn AnimationSpec<T>> {
        let cx = self.cx().expect("the indicator line reads the theme while attached");
        cx.current_value_of(MaterialTheme::LocalMaterialTheme()).motion_scheme.from_token::<T>(token)
    }

    /// Follows the focus of the interaction source (Compose's private `trackFocusState`): focused while a focus
    /// hasn't been taken back, the indicator animated as that changes.
    async fn track_focus_state(self: Rc<IndicatorLine>) {
        self.focused.set(false);
        let interactions = self.interaction_source.borrow().interactions();
        let mut focus_interactions: Vec<FocusInteraction::Focus> = Vec::new();
        let line = self.clone();
        interactions
            .collect(move |interaction: Arc<dyn Interaction>| {
                if let Some(focus) = interaction.downcast_ref::<FocusInteraction::Focus>() {
                    focus_interactions.push(*focus);
                } else if let Some(unfocus) = interaction.downcast_ref::<FocusInteraction::Unfocus>() {
                    focus_interactions.retain(|it| *it != unfocus.focus);
                }
                let is_focused = !focus_interactions.is_empty();
                if is_focused != line.focused.get() {
                    line.focused.set(is_focused);
                    line.invalidate_indicator();
                }
            })
            .await;
    }

    /// Builds the line's drawing (the body of Compose's delegated `CacheDrawModifierNode`): the bottom
    /// stroke-width tall rect cut by the shape's outline, drawn over the content in the animated color.
    fn build(self: &Rc<IndicatorLine>, scope: &mut CacheDrawScope) -> DrawResult {
        let size = scope.size();
        let stroke_width = scope.density().to_px(self.width_animatable.value());
        let mut text_field_shape_path = Path::new();
        text_field_shape_path.add_outline(&self.shape().create_outline(
            size,
            scope.layout_direction(),
            &scope.density(),
        ));
        let mut line_path = Path::new();
        line_path.add_rect(
            Rect::new(0.0, size.height - stroke_width, size.width, size.height),
            PathDirection::CounterClockwise,
        );
        let clipped_line = Path::combine(PathOperation::Intersect, &line_path, &text_field_shape_path);
        let line = self.clone();
        scope.on_draw_with_content(move |d| {
            d.draw_content();
            let color = line.color_animatable.borrow().as_ref().map_or(Color::Unspecified, Animatable::value);
            d.draw_path(&clipped_line, SolidColor(color));
        })
    }
}

/// The indicator line's node (Compose's internal `IndicatorLineNode`): it delegates its drawing to a cache draw
/// node, following the focus and animating the line in coroutines of its own.
struct IndicatorLineNode {
    line: Rc<IndicatorLine>,
    draw_with_cache_modifier_node: Option<DelegatedNode<CacheDrawModifierNode>>,
    track_focus_state_job: Option<Job>,
}

impl IndicatorLineNode {
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        enabled: bool,
        is_error: bool,
        interaction_source: InteractionSource,
        colors: Option<TextFieldColors>,
        text_field_shape: Option<Arc<dyn Shape>>,
        focused_indicator_width: Dp,
        unfocused_indicator_width: Dp,
    ) {
        let line = &self.line;
        let mut should_invalidate = false;
        if line.enabled.get() != enabled {
            line.enabled.set(enabled);
            should_invalidate = true;
        }
        if line.is_error.get() != is_error {
            line.is_error.set(is_error);
            should_invalidate = true;
        }
        if *line.interaction_source.borrow() != interaction_source {
            *line.interaction_source.borrow_mut() = interaction_source;
            if let Some(job) = self.track_focus_state_job.take() {
                job.cancel();
            }
            if let Some(scope) = line.cx().map(|cx| cx.coroutine_scope()) {
                self.track_focus_state_job = Some(scope.launch(line.clone().track_focus_state()));
            }
        }
        if line.colors.get() != colors {
            line.colors.set(colors);
            should_invalidate = true;
        }
        if *line.shape.borrow() != text_field_shape {
            *line.shape.borrow_mut() = text_field_shape;
            // (Compose's `_shape` setter)
            if let Some(node) = &self.draw_with_cache_modifier_node {
                node.borrow_mut().invalidate_draw_cache();
            }
            should_invalidate = true;
        }
        if line.focused_indicator_width.get() != focused_indicator_width {
            line.focused_indicator_width.set(focused_indicator_width);
            should_invalidate = true;
        }
        if line.unfocused_indicator_width.get() != unfocused_indicator_width {
            line.unfocused_indicator_width.set(unfocused_indicator_width);
            should_invalidate = true;
        }
        if should_invalidate {
            line.invalidate_indicator();
        }
    }
}

impl ModifierNode for IndicatorLineNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.line.handle.borrow_mut() = Some(cx.handle());
        // (Compose delegates to it as it is made; a node has its context once attached)
        if self.draw_with_cache_modifier_node.is_none() {
            let line = self.line.clone();
            self.draw_with_cache_modifier_node =
                Some(cx.delegate(CacheDrawModifierNode(move |scope: &mut CacheDrawScope| line.build(scope))));
        }
        self.track_focus_state_job = Some(cx.coroutine_scope().launch(self.line.clone().track_focus_state()));
        if self.line.color_animatable.borrow().is_none() {
            let line = &self.line;
            let initial_color =
                line.colors().indicator_color(line.enabled.get(), line.is_error.get(), line.focused.get());
            *line.color_animatable.borrow_mut() =
                Some(Animatable((initial_color, Color::VectorConverter(initial_color.color_space()))));
        }
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.track_focus_state_job = None;
    }

    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

/// The vertical padding of a filled field's content with a label (Compose's internal
/// `TextFieldWithLabelVerticalPadding`): 8 dp.
pub(crate) const TextFieldWithLabelVerticalPadding: Dp = Dp(8.0);
