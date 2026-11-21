//! Material's outlined text field, as Compose's OutlinedTextField.kt: a field in an outline, its label inside it
//! (at the text's place while empty and unfocused) or minimized in a cut of the outline — or above the field —, a
//! placeholder, icons, a prefix and suffix, a supporting text below; the layout that places them
//! ([`OutlinedTextFieldLayout`] and its measure policy, intrinsics too) and the outline's cut for the label.
//!
//! As the filled field's: [`OutlinedTextField`] (Kotlin's `OutlinedTextField(state: TextFieldState, …)`),
//! [`StringOutlinedTextField`] (`OutlinedTextField(value: String, onValueChange, …)`) and
//! [`ValueOutlinedTextField`] (`OutlinedTextField(value: TextFieldValue, onValueChange, …)`).

use crate::internal::float_producer::FloatProducer;
use crate::internal::layout_util::{PlaceableOrZero, SubtractConstraintSafely};
use crate::internal::strings::{Strings, get_string};
use crate::internal::text_field_impl::{
    AboveLabelBottomPadding, AboveLabelHorizontalPadding, ContainerId, LabelId, LeadingId, PlaceholderId, PrefixId,
    SuffixId, SupportingId, TextFieldId, TextFieldImplModifiers, TrailingId, label_min_height, lerp_int,
    minimized_label_half_height, text_field_horizontal_icon_padding,
};
use crate::text::LocalTextStyle;
use crate::text_field::{
    AffixBoxes, LeadingTrailingBoxes, SupportingBox, coerce_at_least_zero, find, find_first, merged_text_style,
    text_padding,
};
use crate::text_field_defaults::{
    OutlinedTextFieldDefaults, TextFieldColors, TextFieldLabel, TextFieldLabelPosition, TextFieldSlot,
};
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::input::internal::OnTextFieldLayout;
use kompoz_graphics::{Dp, LayoutDirection, Shape, Size, SolidColor, lerp};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, named_args, remember, remember_keyed};
use kompoz_text::TextStyle;
use kompoz_text::input::{TextFieldValue, VisualTransformation};
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable,
    PlacementScope, layout_into,
};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{Alignment, Constraints, HorizontalAlignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// The room left at each side of the outline's cut around the label (Compose's private
/// `OutlinedTextFieldInnerPadding`): 4 dp.
const OutlinedTextFieldInnerPadding: Dp = Dp(4.0);

/// What the outlined fields add before their least size: with a label attached, their semantics merged (Compose's
/// `semantics(mergeDescendants = true) {}`) and half the minimized label's height above, the room of the label in
/// the outline.
#[track_caller]
fn outlined_label_room(attached_label: bool) -> Modifier {
    if attached_label {
        named_args!(Modifier.semantics(merge_descendants = true, properties = |_| {})).padding_sides_with(
            Dp(0.0),
            minimized_label_half_height(),
            Dp(0.0),
            Dp(0.0),
        )
    } else {
        Modifier
    }
}

/// An outlined text field editing `state` (Compose's `OutlinedTextField(state: TextFieldState, …)`, on
/// foundation's state-based `BasicTextField`): as [`TextField`](crate::TextField), decorated by
/// [`OutlinedTextFieldDefaults`]`.decorator(…)` on its [`Container`](OutlinedTextFieldDefaults::Container) in
/// `shape`; with a label attached, half the minimized label's height above it (where the label sits in the
/// outline) and its semantics merged.
///
/// Kotlin: `OutlinedTextField(state: TextFieldState, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedTextField(
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
    #[default(OutlinedTextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(OutlinedTextFieldDefaults.colors())] colors: TextFieldColors,
    #[default(OutlinedTextFieldDefaults.content_padding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    let label_room = outlined_label_room(label.is_some() && !label_position.is_above());
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let source = interaction_source.as_interaction_source();
        let container_source = source.clone();
        let container: TextFieldSlot = Rc::new(move || {
            OutlinedTextFieldDefaults.Container(
                enabled,
                is_error,
                container_source.clone(),
                colors = colors,
                shape = shape.clone(),
            );
        });
        let decorator = OutlinedTextFieldDefaults.decorator(
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
                .then(label_room.clone())
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(
                    min_width = OutlinedTextFieldDefaults::MinWidth,
                    min_height = OutlinedTextFieldDefaults::MinHeight,
                ),
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

/// An outlined text field of the text `value` (Compose's `OutlinedTextField(value: String, onValueChange, …)`,
/// on foundation's `BasicTextFieldString`): as [`StringTextField`](crate::StringTextField), decorated by
/// [`OutlinedTextFieldDefaults`]`.DecorationBox(…)` on its container in `shape`.
///
/// Kotlin: `OutlinedTextField(value: String, onValueChange: (String) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StringOutlinedTextField(
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
    #[default(OutlinedTextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(OutlinedTextFieldDefaults.colors())] colors: TextFieldColors,
) {
    let value: String = value;
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    let label_room = outlined_label_room(label.is_some());
    let on_value_change = Rc::new(on_value_change);
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let decoration_box = outlined_value_decoration_box(
            value.clone(),
            enabled,
            single_line,
            visual_transformation.clone(),
            interaction_source.as_interaction_source(),
            is_error,
            [
                label.clone(),
                placeholder.clone(),
                leading_icon.clone(),
                trailing_icon.clone(),
                prefix.clone(),
                suffix.clone(),
                supporting_text.clone(),
            ],
            colors,
            shape.clone(),
        );
        let on_value_change = on_value_change.clone();
        BasicTextFieldString(
            value.clone(),
            move |it: String| on_value_change(it),
            modifier = modifier
                .clone()
                .then(label_room.clone())
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(
                    min_width = OutlinedTextFieldDefaults::MinWidth,
                    min_height = OutlinedTextFieldDefaults::MinHeight,
                ),
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

/// An outlined text field of `value` — its text, selection and composition (Compose's `OutlinedTextField(value:
/// TextFieldValue, onValueChange, …)`, on foundation's `BasicTextFieldValue`): as [`StringOutlinedTextField`],
/// each value the user makes given to `on_value_change`.
///
/// Kotlin: `OutlinedTextField(value: TextFieldValue, onValueChange: (TextFieldValue) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ValueOutlinedTextField(
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
    #[default(OutlinedTextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(OutlinedTextFieldDefaults.colors())] colors: TextFieldColors,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    let label_room = outlined_label_room(label.is_some());
    let on_value_change = Rc::new(on_value_change);
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let decoration_box = outlined_value_decoration_box(
            value.text().to_string(),
            enabled,
            single_line,
            visual_transformation.clone(),
            interaction_source.as_interaction_source(),
            is_error,
            [
                label.clone(),
                placeholder.clone(),
                leading_icon.clone(),
                trailing_icon.clone(),
                prefix.clone(),
                suffix.clone(),
                supporting_text.clone(),
            ],
            colors,
            shape.clone(),
        );
        let on_value_change = on_value_change.clone();
        BasicTextFieldValue(
            value.clone(),
            move |it: TextFieldValue| on_value_change(it),
            modifier = modifier
                .clone()
                .then(label_room.clone())
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(
                    min_width = OutlinedTextFieldDefaults::MinWidth,
                    min_height = OutlinedTextFieldDefaults::MinHeight,
                ),
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

/// The decoration box both value-based outlined fields give their BasicTextField (Compose's `decorationBox = {
/// innerTextField -> OutlinedTextFieldDefaults.DecorationBox(…) }`): the outlined container in `shape`. The slots:
/// label, placeholder, leading and trailing icons, prefix, suffix, supporting text.
#[allow(clippy::too_many_arguments)]
fn outlined_value_decoration_box(
    text: String,
    enabled: bool,
    single_line: bool,
    visual_transformation: VisualTransformation,
    source: InteractionSource,
    is_error: bool,
    slots: [Option<TextFieldSlot>; 7],
    colors: TextFieldColors,
    shape: Arc<dyn Shape>,
) -> DecorationBox {
    Rc::new(move |inner_text_field: &dyn Fn()| {
        let [label, placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text] = slots.clone();
        let (container_source, shape) = (source.clone(), shape.clone());
        let container: TextFieldSlot = Rc::new(move || {
            named_args!(OutlinedTextFieldDefaults.Container(
                enabled,
                is_error,
                container_source.clone(),
                colors = colors,
                shape = shape.clone()
            ));
        });
        named_args!(OutlinedTextFieldDefaults.DecorationBox(
            &text,
            inner_text_field,
            enabled,
            single_line,
            visual_transformation.clone(),
            source.clone(),
            placeholder = placeholder,
            label = label,
            leading_icon = leading_icon,
            trailing_icon = trailing_icon,
            prefix = prefix,
            suffix = suffix,
            supporting_text = supporting_text,
            is_error = is_error,
            colors = colors,
            container = Some(container),
        ));
    })
}

// ---- The layout -----------------------------------------------------------------------------------------------

/// The outlined field's layout (Compose's internal `OutlinedTextFieldLayout`): as the filled one's
/// ([`TextFieldLayout`](crate::text_field::TextFieldLayout)), the label minimizing into the outline (its size told
/// to `on_label_measured` as it is measured, for the outline's cut), the text kept below half its height.
///
/// Inline (Compose's is restartable): the inner field and the container are borrowed for the call.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn OutlinedTextFieldLayout(
    modifier: Modifier,
    text_field: &dyn Fn(),
    placeholder: Option<Rc<dyn Fn(Modifier)>>,
    label: Option<TextFieldSlot>,
    leading: Option<TextFieldSlot>,
    trailing: Option<TextFieldSlot>,
    prefix: Option<TextFieldSlot>,
    suffix: Option<TextFieldSlot>,
    single_line: bool,
    label_position: TextFieldLabelPosition,
    label_progress: FloatProducer,
    placeholder_alpha: FloatProducer,
    affix_alpha: FloatProducer,
    on_label_measured: Rc<dyn Fn(Size)>,
    container: &dyn Fn(),
    supporting: Option<TextFieldSlot>,
    padding_values: PaddingValues,
) {
    let horizontal_icon_padding = text_field_horizontal_icon_padding();
    let keys = (
        Rc::as_ptr(&on_label_measured) as *const () as usize,
        single_line,
        label_position.clone(),
        label_progress.clone(),
        placeholder_alpha.clone(),
        affix_alpha.clone(),
        padding_values.clone(),
        horizontal_icon_padding,
    );
    let policy = OutlinedTextFieldMeasurePolicy {
        on_label_measured,
        single_line,
        label_position: label_position.clone(),
        label_progress: label_progress.clone(),
        placeholder_alpha,
        affix_alpha,
        padding_values: padding_values.clone(),
        horizontal_icon_padding,
    };
    let measure_policy = remember_keyed(keys, move || Rc::new(policy) as Rc<dyn MeasurePolicy>);
    let layout_direction = LocalLayoutDirection.current();
    layout_into(modifier, measure_policy, false, || {
        container();
        LeadingTrailingBoxes(leading.clone(), trailing.clone());
        let start_text_field_padding = padding_values.calculate_start_padding(layout_direction);
        let end_text_field_padding = padding_values.calculate_end_padding(layout_direction);
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
        let text_padding = text_padding(prefix.is_none(), suffix.is_none(), start_padding, end_padding);
        if let Some(placeholder) = &placeholder {
            placeholder(Modifier.layout_id(PlaceholderId).then(text_padding.clone()));
        }
        Box(
            modifier = Modifier.layout_id(TextFieldId).then(text_padding),
            propagate_min_constraints = true,
            content = || text_field(),
        );
        let label_padding = if label_position.is_above() {
            Modifier.padding_sides(
                start = AboveLabelHorizontalPadding,
                end = AboveLabelHorizontalPadding,
                bottom = AboveLabelBottomPadding,
            )
        } else {
            Modifier
        };
        if let Some(label) = label.clone() {
            let progress = label_progress.clone();
            Box(
                modifier = Modifier
                    .text_field_label_min_height(move || label_min_height(progress.invoke()))
                    .wrap_content_height()
                    .layout_id(LabelId)
                    .then(label_padding),
                content = move || label(),
            );
        }
        SupportingBox(supporting.clone());
    });
}

/// The outlined field's measure policy (Compose's private `OutlinedTextFieldMeasurePolicy`).
struct OutlinedTextFieldMeasurePolicy {
    on_label_measured: Rc<dyn Fn(Size)>,
    single_line: bool,
    label_position: TextFieldLabelPosition,
    label_progress: FloatProducer,
    placeholder_alpha: FloatProducer,
    affix_alpha: FloatProducer,
    padding_values: PaddingValues,
    horizontal_icon_padding: Dp,
}

/// What the outlined layout measured, to place.
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

fn size_of(placeable: Option<&Placeable>) -> Size {
    placeable.map_or(Size::Zero, |it| Size::new(it.width() as f32, it.height() as f32))
}

impl MeasurePolicy for OutlinedTextFieldMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let label_progress = self.label_progress.invoke();
        let layout_direction = scope.layout_direction();
        let mut occupied_space_horizontally = 0;
        let mut occupied_space_vertically = 0;
        let bottom_padding = scope.round_to_px(self.padding_values.calculate_bottom_padding());

        let relaxed_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));

        // measure leading icon
        let leading_placeable = find(measurables, LeadingId).map(|it| it.measure(relaxed_constraints));
        occupied_space_horizontally += leading_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(leading_placeable.height_or_zero());

        // measure trailing icon
        let trailing_placeable = find(measurables, TrailingId)
            .map(|it| it.measure(relaxed_constraints.offset(-occupied_space_horizontally, 0)));
        occupied_space_horizontally += trailing_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(trailing_placeable.height_or_zero());

        // measure prefix
        let prefix_placeable = find(measurables, PrefixId)
            .map(|it| it.measure(relaxed_constraints.offset(-occupied_space_horizontally, 0)));
        occupied_space_horizontally += prefix_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(prefix_placeable.height_or_zero());

        // measure suffix
        let suffix_placeable = find(measurables, SuffixId)
            .map(|it| it.measure(relaxed_constraints.offset(-occupied_space_horizontally, 0)));
        occupied_space_horizontally += suffix_placeable.width_or_zero();
        occupied_space_vertically = occupied_space_vertically.max(suffix_placeable.height_or_zero());

        // measure label
        let is_label_above = self.label_position.is_above();
        let label_measurable = find(measurables, LabelId);
        let mut label_placeable: Option<Placeable> = None;
        let label_intrinsic_height = if !is_label_above {
            let total_horizontal_padding = scope
                .round_to_px(self.padding_values.calculate_left_padding(layout_direction))
                + scope.round_to_px(self.padding_values.calculate_right_padding(layout_direction));
            let label_horizontal_constraint_offset = lerp_int(
                occupied_space_horizontally + total_horizontal_padding, // label in middle
                total_horizontal_padding,                               // label in outline
                label_progress,
            );
            let label_constraints = relaxed_constraints.offset(-label_horizontal_constraint_offset, -bottom_padding);
            label_placeable = label_measurable.map(|it| it.measure(label_constraints));
            (self.on_label_measured)(size_of(label_placeable.as_ref()));
            0
        } else {
            // only intrinsic height can be measured here, the label is measured after the width is known
            label_measurable.map_or(0, |it| it.min_intrinsic_height(constraints.min_width))
        };

        // supporting text intrinsic height measurement is required to set the text field's constraints
        let supporting_measurable = find(measurables, SupportingId);
        let supporting_intrinsic_height =
            supporting_measurable.map_or(0, |it| it.min_intrinsic_height(constraints.min_width));

        // measure text field
        let top_padding = if is_label_above {
            scope.round_to_px(self.padding_values.calculate_top_padding())
        } else {
            (label_placeable.height_or_zero() / 2).max(scope.round_to_px(self.padding_values.calculate_top_padding()))
        };
        let text_constraints = named_args!(
            Constraints::offset(
                &constraints,
                -occupied_space_horizontally,
                -bottom_padding - top_padding - label_intrinsic_height - supporting_intrinsic_height
            )
            .copy(min_height = 0)
        );
        let text_field_placeable = find_first(measurables, TextFieldId).measure(text_constraints);

        // measure placeholder
        let placeholder_constraints = named_args!(text_constraints.copy(min_width = 0));
        let placeholder_placeable = find(measurables, PlaceholderId).map(|it| it.measure(placeholder_constraints));

        occupied_space_vertically = occupied_space_vertically.max(
            text_field_placeable.height().max(placeholder_placeable.height_or_zero()) + top_padding + bottom_padding,
        );

        let width = self.calculate_width(
            scope,
            leading_placeable.width_or_zero(),
            trailing_placeable.width_or_zero(),
            prefix_placeable.width_or_zero(),
            suffix_placeable.width_or_zero(),
            text_field_placeable.width(),
            label_placeable.width_or_zero(),
            placeholder_placeable.width_or_zero(),
            constraints,
            label_progress,
        );

        if is_label_above {
            // now that we know the width, measure the label
            let label_constraints =
                named_args!(relaxed_constraints.copy(max_height = label_intrinsic_height, max_width = width));
            label_placeable = label_measurable.map(|it| it.measure(label_constraints));
            (self.on_label_measured)(size_of(label_placeable.as_ref()));
        }

        // measure supporting text
        let supporting_constraints = named_args!(
            Constraints::offset(&relaxed_constraints, 0, -occupied_space_vertically)
                .copy(min_height = 0, max_width = width)
        );
        let supporting_placeable = supporting_measurable.map(|it| it.measure(supporting_constraints));
        let supporting_height = supporting_placeable.height_or_zero();

        let total_height = self.calculate_height(
            scope,
            leading_placeable.height_or_zero(),
            trailing_placeable.height_or_zero(),
            prefix_placeable.height_or_zero(),
            suffix_placeable.height_or_zero(),
            text_field_placeable.height(),
            label_placeable.height_or_zero(),
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
        let place = Placement {
            single_line: self.single_line,
            label_position: self.label_position.clone(),
            padding_values: self.padding_values.clone(),
            placeholder_alpha: self.placeholder_alpha.clone(),
            affix_alpha: self.affix_alpha.clone(),
            density: scope.density().density,
            layout_direction,
            is_label_above,
            label_progress,
            icon_padding: scope.to_px(self.horizontal_icon_padding),
        };
        scope.layout(width, total_height, move |p| place.place(p, &placed, total_height, width))
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
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        self.intrinsic_width(scope, measurables, height, |it, h| it.max_intrinsic_width(h))
    }

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        self.intrinsic_width(scope, measurables, height, |it, h| it.min_intrinsic_width(h))
    }
}

impl OutlinedTextFieldMeasurePolicy {
    /// The outlined layout's intrinsic width (Compose's private `intrinsicWidth`).
    fn intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
        intrinsic_measurer: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
    ) -> i32 {
        let text_field_width = intrinsic_measurer(find_first(measurables, TextFieldId), height);
        let measure = |id| find(measurables, id).map_or(0, |it| intrinsic_measurer(it, height));
        let label_width = measure(LabelId);
        let trailing_width = measure(TrailingId);
        let leading_width = measure(LeadingId);
        let prefix_width = measure(PrefixId);
        let suffix_width = measure(SuffixId);
        let placeholder_width = measure(PlaceholderId);
        self.calculate_width(
            scope,
            leading_width,
            trailing_width,
            prefix_width,
            suffix_width,
            text_field_width,
            label_width,
            placeholder_width,
            Constraints::new(0, Constraints::Infinity, 0, Constraints::Infinity),
            self.label_progress.invoke(),
        )
    }

    /// The outlined layout's intrinsic height (Compose's private `intrinsicHeight`).
    fn intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
        intrinsic_measurer: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
    ) -> i32 {
        let label_progress = self.label_progress.invoke();
        let mut remaining_width = width;
        let leading_height = find(measurables, LeadingId).map_or(0, |it| {
            remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
            intrinsic_measurer(it, width)
        });
        let trailing_height = find(measurables, TrailingId).map_or(0, |it| {
            remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
            intrinsic_measurer(it, width)
        });
        let label_height = find(measurables, LabelId)
            .map_or(0, |it| intrinsic_measurer(it, lerp_int(remaining_width, width, label_progress)));
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
            leading_height,
            trailing_height,
            prefix_height,
            suffix_height,
            text_field_height,
            label_height,
            placeholder_height,
            supporting_height,
            Constraints::new(0, Constraints::Infinity, 0, Constraints::Infinity),
            self.label_position.is_above(),
            label_progress,
        )
    }

    /// The outlined layout's width (Compose's private `Density.calculateWidth`): at least the minimized label's
    /// with the side padding, as it minimizes.
    #[allow(clippy::too_many_arguments)]
    fn calculate_width(
        &self,
        scope: &MeasureScope,
        leading_placeable_width: i32,
        trailing_placeable_width: i32,
        prefix_placeable_width: i32,
        suffix_placeable_width: i32,
        text_field_placeable_width: i32,
        label_placeable_width: i32,
        placeholder_placeable_width: i32,
        constraints: Constraints,
        label_progress: f32,
    ) -> i32 {
        let affix_total_width = prefix_placeable_width + suffix_placeable_width;
        let middle_section = (text_field_placeable_width + affix_total_width)
            .max(placeholder_placeable_width + affix_total_width)
            .max(lerp_int(label_placeable_width, 0, label_progress));
        let wrapped_width = leading_placeable_width + middle_section + trailing_placeable_width;

        // Actual LayoutDirection doesn't matter; we only need the sum
        let label_horizontal_padding = scope.to_px(
            self.padding_values.calculate_left_padding(LayoutDirection::Ltr)
                + self.padding_values.calculate_right_padding(LayoutDirection::Ltr),
        );
        let focused_label_width =
            ((label_placeable_width as f32 + label_horizontal_padding) * label_progress + 0.5).floor() as i32;
        constraints.constrain_width(wrapped_width.max(focused_label_width))
    }

    /// The outlined layout's height (Compose's private `Density.calculateHeight`): the top padding at least half
    /// the label's height as it minimizes, the tallest of the text, placeholder, prefix, suffix and expanded
    /// label, the bottom padding — at least the icons' —, the label above and the supporting text.
    #[allow(clippy::too_many_arguments)]
    fn calculate_height(
        &self,
        scope: &MeasureScope,
        leading_height: i32,
        trailing_height: i32,
        prefix_height: i32,
        suffix_height: i32,
        text_field_height: i32,
        label_height: i32,
        placeholder_height: i32,
        supporting_height: i32,
        constraints: Constraints,
        is_label_above: bool,
        label_progress: f32,
    ) -> i32 {
        let input_field_height = text_field_height
            .max(placeholder_height)
            .max(prefix_height)
            .max(suffix_height)
            .max(if is_label_above { 0 } else { lerp_int(label_height, 0, label_progress) });
        let top_padding = scope.to_px(self.padding_values.calculate_top_padding());
        let actual_top_padding = if is_label_above {
            top_padding
        } else {
            lerp(top_padding, top_padding.max(label_height as f32 / 2.0), label_progress)
        };
        let bottom_padding = scope.to_px(self.padding_values.calculate_bottom_padding());
        let middle_section_height = actual_top_padding + input_field_height as f32 + bottom_padding;
        constraints.constrain_height(
            if is_label_above { label_height } else { 0 }
                + leading_height.max(trailing_height).max((middle_section_height + 0.5).floor() as i32)
                + supporting_height,
        )
    }
}

/// What the outlined layout's placement reads (the policy's fields and the measure's values Compose's private
/// `place` is given).
struct Placement {
    single_line: bool,
    label_position: TextFieldLabelPosition,
    padding_values: PaddingValues,
    placeholder_alpha: FloatProducer,
    affix_alpha: FloatProducer,
    density: f32,
    layout_direction: LayoutDirection,
    is_label_above: bool,
    label_progress: f32,
    icon_padding: f32,
}

/// Kotlin's `Float.roundToInt()`: half up.
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

impl Placement {
    /// Places the outlined layout's parts (Compose's private `Placeable.PlacementScope.place`): the label lerped
    /// from the text's place to the outline (half its height above the top), between the side paddings as it
    /// minimizes; the text at least half the label's height down.
    fn place(&self, p: &PlacementScope, placed: &Placed, total_height: i32, width: i32) {
        let (layout_direction, density, label_progress) = (self.layout_direction, self.density, self.label_progress);
        let y_offset = if self.is_label_above { placed.label.height_or_zero() } else { 0 };

        p.place(&placed.container, 0, y_offset);
        let height = total_height
            - placed.supporting.height_or_zero()
            - if self.is_label_above { placed.label.height_or_zero() } else { 0 };

        let top_padding = round(self.padding_values.calculate_top_padding().0 * density);

        if let Some(leading) = &placed.leading {
            p.place_relative(leading, 0, y_offset + Alignment::CenterVertically.align(leading.height(), height));
        }

        if let Some(label) = &placed.label {
            let start_y = if self.is_label_above {
                0
            } else if self.single_line {
                Alignment::CenterVertically.align(label.height(), height)
            } else {
                top_padding
            };
            let end_y = if self.is_label_above { 0 } else { -(label.height() / 2) };
            let position_y = lerp_int(start_y, end_y, label_progress);

            if self.is_label_above {
                let position_x =
                    self.label_position.minimized_alignment().align(label.width(), width, layout_direction);
                // Not placeRelative because alignment already handles RTL
                p.place(label, position_x, position_y);
            } else {
                let start_padding = self.padding_values.calculate_start_padding(layout_direction).0 * density;
                let end_padding = self.padding_values.calculate_end_padding(layout_direction).0 * density;
                let leading_plus_padding = match &placed.leading {
                    None => start_padding,
                    Some(leading) => leading.width() as f32 + (start_padding - self.icon_padding).max(0.0),
                };
                let trailing_plus_padding = match &placed.trailing {
                    None => end_padding,
                    Some(trailing) => trailing.width() as f32 + (end_padding - self.icon_padding).max(0.0),
                };
                let left_padding = if layout_direction == LayoutDirection::Ltr { start_padding } else { end_padding };
                let left_icon_plus_padding =
                    if layout_direction == LayoutDirection::Ltr { leading_plus_padding } else { trailing_plus_padding };
                let start_x = self.label_position.expanded_alignment().align(
                    label.width(),
                    width - round(leading_plus_padding + trailing_plus_padding),
                    layout_direction,
                ) as f32
                    + left_icon_plus_padding;
                let end_x = self.label_position.minimized_alignment().align(
                    label.width(),
                    width - round(start_padding + end_padding),
                    layout_direction,
                ) as f32
                    + left_padding;
                let position_x = round(lerp(start_x, end_x, label_progress));
                // Not placeRelative because alignment already handles RTL
                p.place(label, position_x, position_y);
            }
        }

        let calculate_vertical_position = |placeable: &Placeable| {
            let default_position = y_offset
                + if self.single_line {
                    Alignment::CenterVertically.align(placeable.height(), height)
                } else {
                    top_padding
                };
            if self.label_position.is_above() {
                default_position
            } else {
                // Ensure components are placed below label when it's in the border
                default_position.max(placed.label.height_or_zero() / 2)
            }
        };

        if let Some(prefix) = &placed.prefix {
            let alpha = self.affix_alpha.clone();
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
            let alpha = self.placeholder_alpha.clone();
            p.place_relative_with_layer(
                placeholder,
                text_horizontal_position,
                calculate_vertical_position(placeholder),
                move |l| l.alpha = alpha.invoke(),
            );
        }

        if let Some(suffix) = &placed.suffix {
            let alpha = self.affix_alpha.clone();
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
                y_offset + Alignment::CenterVertically.align(trailing.height(), height),
            );
        }

        if let Some(supporting) = &placed.supporting {
            p.place_relative(supporting, 0, y_offset + height);
        }
    }
}

/// The outline's cut for the minimized label (Compose's internal `Modifier.outlineCutout`).
pub(crate) trait OutlineCutoutModifiers {
    /// What is drawn below, but where the label of `label_size` (read as it draws; none while its width is 0)
    /// sits across the top edge, `alignment` placing it between `padding_values`' sides, 4 dp more each side.
    fn outline_cutout(
        self,
        label_size: impl Fn() -> Size + 'static,
        alignment: HorizontalAlignment,
        padding_values: PaddingValues,
    ) -> Modifier;
}

impl OutlineCutoutModifiers for Modifier {
    fn outline_cutout(
        self,
        label_size: impl Fn() -> Size + 'static,
        alignment: HorizontalAlignment,
        padding_values: PaddingValues,
    ) -> Modifier {
        self.draw_with_content(move |d| {
            let label_size_value = label_size();
            let label_width = label_size_value.width;
            if label_width > 0.0 {
                let inner_padding = d.to_px(OutlinedTextFieldInnerPadding);
                let layout_direction = d.layout_direction();
                let size = d.size();
                let left_padding = d.to_px(padding_values.calculate_left_padding(layout_direction));
                let right_padding = d.to_px(padding_values.calculate_right_padding(layout_direction));
                let label_center = alignment.align(
                    round(label_width),
                    round(size.width - left_padding - right_padding),
                    layout_direction,
                ) as f32
                    + left_padding
                    + (label_width / 2.0);
                let left = (label_center - (label_width / 2.0) - inner_padding).max(0.0);
                let right = (label_center + (label_width / 2.0) + inner_padding).min(size.width);
                let label_height = label_size_value.height;
                d.clip_rect_with(left, -label_height / 2.0, right, label_height / 2.0, ClipOp::Difference, |d| {
                    d.draw_content()
                });
            } else {
                d.draw_content();
            }
        })
    }
}
