//! The date pickers' input mode, as Compose's DateInput.kt (material3 1.5.0-alpha17): an outlined text field the
//! date is typed in by the locale's short pattern (its digits only, the delimiters shown as they're typed), checked
//! as typed — the pattern, the year range, the selectable dates, a range's order — the error under it.

use std::cell::Cell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use kompoz_coroutines::{LaunchedEffect, delay_ms};
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::KeyboardOptions;
use kompoz_graphics::Dp;
use kompoz_runtime::{
    MutableState, kompozable, mutable_state_of, named_args, remember_keyed, remember_saveable, remember_saveable_keyed,
};
use kompoz_text::{AnnotatedString, OffsetMapping, TextFieldValue, TextRange, TransformedText, VisualTransformation};
use kompoz_ui::Modifier;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::platform::{ImeAction, KeyboardType};

use crate::calendar_locale::{CalendarLocale, to_local_string_min};
use crate::date_picker::{DatePickerColors, DatePickerFormatter, SelectableDates};
use crate::internal::calendar_model::{CalendarDate, CalendarModel, DateInputFormat};
use crate::internal::strings::{Strings, format_string, get_string};
use crate::outlined_text_field::ValueOutlinedTextField;
use crate::text::Text;
use crate::text_field_defaults::{TextFieldDefaults, TextFieldSlot};
use crate::tokens::MotionTokens;

/// The single date's input (Compose's internal `DateInputContent`): the field labelled "Date" (read out with the
/// pattern), the pattern as its placeholder, checked by a [`DateInputValidator`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DateInputContent(
    selected_date_millis: Option<i64>,
    on_date_selection_change: Rc<dyn Fn(Option<i64>)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
    focus_requester: Option<FocusRequester>,
) {
    // Obtain the DateInputFormat for the default Locale.
    let date_input_format = {
        let model = calendar_model.clone();
        remember_keyed(calendar_model.locale().clone(), move || model.get_date_input_format(model.locale()))
    };
    let error_date_pattern = get_string(Strings::DateInputInvalidForPattern);
    let error_date_out_of_year_range = get_string(Strings::DateInputInvalidYearRange);
    let error_invalid_not_allowed = get_string(Strings::DateInputInvalidNotAllowed);
    let date_input_validator = {
        let (format, formatter) = (date_input_format.clone(), date_formatter.clone());
        remember_keyed((date_input_format.clone(), FormatterKey(date_formatter.clone())), move || {
            Rc::new(DateInputValidator::new(
                year_range,
                selectable_dates,
                format,
                formatter,
                error_date_pattern,
                error_date_out_of_year_range,
                error_invalid_not_allowed,
                // Not used for a single date input.
                String::new(),
            ))
        })
    };
    // Only need to apply the start date, as this is for a single date input.
    date_input_validator.current_start_date_millis.set(selected_date_millis);
    let pattern = date_input_format.pattern_with_delimiters.to_uppercase();
    let label_text = get_string(Strings::DateInputLabel);
    let description = format!("{label_text}, {pattern}");
    DateInputTextField(
        named_args!(Modifier.fill_max_width().padding_values(InputTextFieldPadding())),
        selected_date_millis,
        on_date_selection_change,
        calendar_model.clone(),
        Some(Rc::new(move || {
            let description = description.clone();
            Text(
                label_text.clone(),
                modifier = Modifier.semantics(move |c| c.set_content_description(description.clone())),
            )
        })),
        Some(Rc::new(move || Text(pattern.clone(), modifier = Modifier.clear_and_set_semantics(|_| {})))),
        InputIdentifier::SingleDateInput,
        date_input_validator,
        date_input_format,
        calendar_model.locale().clone(),
        colors,
        focus_requester,
    );
}

/// A [`DatePickerFormatter`] as a remember key: the same formatter, or equal default ones.
#[derive(Clone)]
pub(crate) struct FormatterKey(pub(crate) Rc<dyn DatePickerFormatter>);

impl PartialEq for FormatterKey {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

/// A date's text field (Compose's internal `DateInputTextField`): digits only, at most the pattern's, the
/// delimiters shown by a visual transformation; a full date is parsed and validated, picked if valid (none while
/// short or wrong), the error shown as its supporting text; the field shows `initial_date_millis` as it changes,
/// takes the focus a moment after it shows if given a requester.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DateInputTextField(
    modifier: Modifier,
    initial_date_millis: Option<i64>,
    on_date_selection_change: Rc<dyn Fn(Option<i64>)>,
    calendar_model: Rc<dyn CalendarModel>,
    label: Option<TextFieldSlot>,
    placeholder: Option<TextFieldSlot>,
    input_identifier: InputIdentifier,
    date_input_validator: Rc<DateInputValidator>,
    date_input_format: DateInputFormat,
    locale: CalendarLocale,
    colors: DatePickerColors,
    focus_requester: Option<FocusRequester>,
) {
    let text: MutableState<TextFieldValue> = remember_saveable(|| mutable_state_of(TextFieldValue::default()));
    let error_text: MutableState<String> = {
        let (model, validator, format, locale) =
            (calendar_model.clone(), date_input_validator.clone(), date_input_format.clone(), locale.clone());
        let current = text.get();
        remember_saveable_keyed(current.clone(), move || {
            // Run an initial validation if the text is not empty.
            let mut initial_error = String::new();
            if !current.text().is_empty() {
                initial_error = validator.validate(
                    model.parse(current.text(), &format.pattern_without_delimiters, &locale),
                    input_identifier,
                    &locale,
                );
            }
            mutable_state_of(initial_error)
        })
    };

    // Calculate how much bottom padding should be added. In case there is an error text, which is added as a
    // supportingText, take into account the default supportingText padding to ensure the padding does not trigger a
    // component height change.
    let bottom_padding = if error_text.get().trim().is_empty() {
        InputTextNonErroneousBottomPadding
    } else {
        let text_field_padding = TextFieldDefaults.supporting_text_padding();
        InputTextNonErroneousBottomPadding
            - (text_field_padding.calculate_bottom_padding() + text_field_padding.calculate_top_padding())
    };
    let error_for_semantics = error_text.clone();
    let error_for_supporting = error_text.clone();
    let is_error = !error_text.get().trim().is_empty();
    let on_change = {
        let (text, error_text, model, validator, format, locale) = (
            text.clone(),
            error_text.clone(),
            calendar_model.clone(),
            date_input_validator.clone(),
            date_input_format.clone(),
            locale.clone(),
        );
        let on_date_selection_change = on_date_selection_change.clone();
        move |input: TextFieldValue| {
            let pattern_length = format.pattern_without_delimiters.chars().count();
            if input.text().chars().count() <= pattern_length && input.text().chars().all(|c| c.is_ascii_digit()) {
                text.set(input.clone());
                let trimmed_text = input.text().trim();
                if trimmed_text.is_empty() || trimmed_text.chars().count() < pattern_length {
                    error_text.set(String::new());
                    on_date_selection_change(None);
                } else {
                    let parsed_date = model.parse(trimmed_text, &format.pattern_without_delimiters, &locale);
                    error_text.set(validator.validate(parsed_date, input_identifier, &locale));
                    // Set the parsed date only if the error validation returned an empty string. Otherwise, set it
                    // to null, as the validation failed.
                    on_date_selection_change(if error_text.peek().is_empty() {
                        parsed_date.map(|date| date.utc_time_millis)
                    } else {
                        None
                    });
                }
            }
        }
    };
    ValueOutlinedTextField(
        value = text.get(),
        on_value_change = on_change,
        modifier = named_args!(modifier.padding_sides(bottom = bottom_padding))
            .semantics(move |c| {
                let error = error_for_semantics.get();
                if !error.trim().is_empty() {
                    c.error(error);
                }
            })
            .then(match &focus_requester {
                Some(focus_requester) => Modifier.focus_requester(focus_requester),
                None => Modifier,
            }),
        label = label,
        placeholder = placeholder,
        supporting_text = Some(Rc::new(move || {
            let error = error_for_supporting.get();
            if !error.trim().is_empty() {
                Text(error);
            }
        }) as TextFieldSlot),
        is_error = is_error,
        visual_transformation = date_visual_transformation(&date_input_format),
        keyboard_options = KeyboardOptions {
            auto_correct_enabled: Some(false),
            keyboard_type: KeyboardType::Number,
            ime_action: ImeAction::Done,
            ..KeyboardOptions::Default
        },
        single_line = true,
        colors = colors.date_text_field_colors,
    );

    {
        let focus_requester = focus_requester.clone();
        LaunchedEffect((), async move {
            // In case a focus is to be requested, delay the request to allow a smooth transition in case the
            // DateInput is in a dialog.
            if let Some(focus_requester) = focus_requester {
                delay_ms(MotionTokens::DurationMedium2 as u64).await;
                focus_requester.request_focus();
            }
        });
    }

    LaunchedEffect(initial_date_millis, async move {
        if let Some(millis) = initial_date_millis {
            let initial_text =
                calendar_model.format_with_pattern(millis, &date_input_format.pattern_without_delimiters, &locale);
            let length = initial_text.chars().count();
            text.set(TextFieldValue::from_text(
                &initial_text,
                // Ensures that the initial cursor position is at the end of the text.
                if initial_text.is_empty() { TextRange::Zero } else { TextRange::new(length, length) },
                None,
            ));
        }
    });
}

/// Checks a typed date (Compose's internal `DateInputValidator`): its pattern, its year, whether it may be picked,
/// and for a range's start or end, that the start isn't after the end. Each check's error text, or an empty one.
pub(crate) struct DateInputValidator {
    year_range: RangeInclusive<i32>,
    selectable_dates: Rc<dyn SelectableDates>,
    date_input_format: DateInputFormat,
    date_formatter: Rc<dyn DatePickerFormatter>,
    error_date_pattern: String,
    error_date_out_of_year_range: String,
    error_invalid_not_allowed: String,
    error_invalid_range_input: String,
    /// The current start date in UTC milliseconds (Compose's `currentStartDateMillis`): what the end may not be
    /// before.
    pub(crate) current_start_date_millis: Cell<Option<i64>>,
    /// The current end date in UTC milliseconds (Compose's `currentEndDateMillis`): what the start may not be after.
    pub(crate) current_end_date_millis: Cell<Option<i64>>,
}

impl DateInputValidator {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        year_range: RangeInclusive<i32>,
        selectable_dates: Rc<dyn SelectableDates>,
        date_input_format: DateInputFormat,
        date_formatter: Rc<dyn DatePickerFormatter>,
        error_date_pattern: String,
        error_date_out_of_year_range: String,
        error_invalid_not_allowed: String,
        error_invalid_range_input: String,
    ) -> DateInputValidator {
        DateInputValidator {
            year_range,
            selectable_dates,
            date_input_format,
            date_formatter,
            error_date_pattern,
            error_date_out_of_year_range,
            error_invalid_not_allowed,
            error_invalid_range_input,
            current_start_date_millis: Cell::new(None),
            current_end_date_millis: Cell::new(None),
        }
    }

    /// The error of a parsed date (`None`: it didn't parse), empty if it is valid (Compose's `validate`).
    pub(crate) fn validate(
        &self,
        date_to_validate: Option<CalendarDate>,
        input_identifier: InputIdentifier,
        locale: &CalendarLocale,
    ) -> String {
        let Some(date_to_validate) = date_to_validate else {
            return format_string(
                &self.error_date_pattern,
                &[&self.date_input_format.pattern_with_delimiters.to_uppercase()],
            );
        };
        // Check that the date is within the valid range of years.
        if !self.year_range.contains(&date_to_validate.year) {
            return format_string(
                &self.error_date_out_of_year_range,
                &[
                    &to_local_string_min(*self.year_range.start(), 1, locale),
                    &to_local_string_min(*self.year_range.end(), 1, locale),
                ],
            );
        }
        // Check that the provided SelectableDates allows this date to be selected.
        if !self.selectable_dates.is_selectable_year(date_to_validate.year)
            || !self.selectable_dates.is_selectable_date(date_to_validate.utc_time_millis)
        {
            return format_string(
                &self.error_invalid_not_allowed,
                // Kotlin interpolates a null as "null".
                &[&self
                    .date_formatter
                    .format_date(Some(date_to_validate.utc_time_millis), locale, false)
                    .unwrap_or_else(|| "null".to_string())],
            );
        }

        // Additional validation when the InputIdentifier is for start of end dates in a range input
        if (input_identifier == InputIdentifier::StartDateInput
            && date_to_validate.utc_time_millis > self.current_end_date_millis.get().unwrap_or(i64::MAX))
            || (input_identifier == InputIdentifier::EndDateInput
                && date_to_validate.utc_time_millis < self.current_start_date_millis.get().unwrap_or(i64::MIN))
        {
            // The input start date is after the end date, or the end date is before the start date.
            return self.error_invalid_range_input.clone();
        }

        String::new()
    }
}

/// Which of the inputs a field is (Compose's internal value class `InputIdentifier`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct InputIdentifier(i32);

impl InputIdentifier {
    pub(crate) const SingleDateInput: InputIdentifier = InputIdentifier(0);
    pub(crate) const StartDateInput: InputIdentifier = InputIdentifier(1);
    pub(crate) const EndDateInput: InputIdentifier = InputIdentifier(2);
}

/// Compose's toString: the identifier's name.
impl std::fmt::Display for InputIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match *self {
            InputIdentifier::SingleDateInput => "SingleDateInput",
            InputIdentifier::StartDateInput => "StartDateInput",
            InputIdentifier::EndDateInput => "EndDateInput",
            _ => "Unknown",
        })
    }
}

/// The delimiters shown in the typed digits (Compose's private `DateVisualTransformation`): after the first group
/// and the second, the cursor mapped across them.
fn date_visual_transformation(date_input_format: &DateInputFormat) -> VisualTransformation {
    let pattern: Vec<char> = date_input_format.pattern_with_delimiters.chars().collect();
    let delimiter = date_input_format.delimiter;
    let first_delimiter_offset = pattern.iter().position(|&c| c == delimiter).map_or(-1, |i| i as isize);
    let second_delimiter_offset = pattern.iter().rposition(|&c| c == delimiter).map_or(-1, |i| i as isize);
    let date_format_length = date_input_format.pattern_without_delimiters.chars().count() as isize;
    let offset_mapping = OffsetMapping::new(
        move |offset| {
            let offset = offset as isize;
            let transformed = if offset < first_delimiter_offset {
                offset
            } else if offset < second_delimiter_offset {
                offset + 1
            } else if offset <= date_format_length {
                offset + 2
            } else {
                date_format_length + 2 // 10
            };
            transformed.max(0) as usize
        },
        move |offset| {
            let offset = offset as isize;
            // (Compose's `offset <= firstDelimiterOffset - 1`, `offset <= secondDelimiterOffset - 1`)
            let original = if offset < first_delimiter_offset {
                offset
            } else if offset < second_delimiter_offset {
                offset - 1
            } else if offset <= date_format_length + 1 {
                offset - 2
            } else {
                date_format_length // 8
            };
            original.max(0) as usize
        },
    );
    VisualTransformation::new(move |text: &AnnotatedString| {
        let trimmed_text: Vec<char> = text.text().chars().take(date_format_length as usize).collect();
        let mut transformed_text = String::new();
        for (index, char) in trimmed_text.iter().enumerate() {
            transformed_text.push(*char);
            let index = index as isize;
            if index + 1 == first_delimiter_offset || index + 2 == second_delimiter_offset {
                transformed_text.push(delimiter);
            }
        }
        TransformedText::new(AnnotatedString::new(transformed_text), offset_mapping.clone())
    })
}

/// Compose's internal `InputTextFieldPadding`: 24 dp at the sides, 10 dp at the top.
pub(crate) fn InputTextFieldPadding() -> PaddingValues {
    PaddingValues::new(Dp(24.0), Dp(10.0), Dp(24.0), Dp(0.0))
}

// An optional padding that will only be added to the bottom of the date input text field when it's not showing an
// error message.
const InputTextNonErroneousBottomPadding: Dp = Dp(16.0);
