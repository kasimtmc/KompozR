//! The date range picker's input mode, as Compose's DateRangeInput.kt (material3 1.5.0-alpha17): the start's and
//! the end's text fields side by side, each checked against the other.

use std::ops::RangeInclusive;
use std::rc::Rc;

use kompoz_foundation::prelude::*;
use kompoz_graphics::Dp;
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::focus::FocusRequester;

use crate::date_input::{DateInputTextField, DateInputValidator, FormatterKey, InputIdentifier, InputTextFieldPadding};
use crate::date_picker::{DatePickerColors, DatePickerFormatter, SelectableDates};
use crate::internal::calendar_model::CalendarModel;
use crate::internal::strings::{Strings, get_string};
use crate::text::Text;

/// The range's inputs (Compose's internal `DateRangeInputContent`): "Start date" and "End date" fields, half the
/// width each, 8 dp apart; typing one keeps the other's date, the validator telling an end before the start.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DateRangeInputContent(
    selected_start_date_millis: Option<i64>,
    selected_end_date_millis: Option<i64>,
    on_dates_selection_change: Rc<dyn Fn(Option<i64>, Option<i64>)>,
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
    let error_invalid_range = get_string(Strings::DateRangeInputInvalidRangeInput);
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
                error_invalid_range,
            ))
        })
    };
    // Apply both start and end dates for proper validation.
    date_input_validator.current_start_date_millis.set(selected_start_date_millis);
    date_input_validator.current_end_date_millis.set(selected_end_date_millis);
    Row(
        modifier = Modifier.padding_values(InputTextFieldPadding()),
        horizontal_arrangement = Arrangement::spaced_by(TextFieldSpacing),
        content = move || {
            let pattern = date_input_format.pattern_with_delimiters.to_uppercase();
            let start_range_text = get_string(Strings::DateRangePickerStartHeadline);
            let start_description = format!("{start_range_text}, {pattern}");
            let start_pattern = pattern.clone();
            let on_start = on_dates_selection_change.clone();
            DateInputTextField(
                Modifier.weight_with(0.5, true),
                selected_start_date_millis,
                Rc::new(move |start_date_millis| {
                    // Delegate to the onDatesSelectionChange and change just the start date.
                    on_start(start_date_millis, selected_end_date_millis)
                }),
                calendar_model.clone(),
                Some(Rc::new(move || {
                    let description = start_description.clone();
                    Text(
                        start_range_text.clone(),
                        modifier = Modifier.semantics(move |c| c.set_content_description(description.clone())),
                    )
                })),
                Some(Rc::new(move || Text(start_pattern.clone(), modifier = Modifier.clear_and_set_semantics(|_| {})))),
                InputIdentifier::StartDateInput,
                date_input_validator.clone(),
                date_input_format.clone(),
                calendar_model.locale().clone(),
                colors,
                focus_requester.clone(),
            );
            let end_range_text = get_string(Strings::DateRangePickerEndHeadline);
            let end_description = format!("{end_range_text}, {pattern}");
            let on_end = on_dates_selection_change.clone();
            DateInputTextField(
                Modifier.weight_with(0.5, true),
                selected_end_date_millis,
                Rc::new(move |end_date_millis| {
                    // Delegate to the onDatesSelectionChange and change just the end date.
                    on_end(selected_start_date_millis, end_date_millis)
                }),
                calendar_model.clone(),
                Some(Rc::new(move || {
                    let description = end_description.clone();
                    Text(
                        end_range_text.clone(),
                        modifier = Modifier.semantics(move |c| c.set_content_description(description.clone())),
                    )
                })),
                Some(Rc::new(move || Text(pattern.clone(), modifier = Modifier.clear_and_set_semantics(|_| {})))),
                InputIdentifier::EndDateInput,
                date_input_validator.clone(),
                date_input_format.clone(),
                calendar_model.locale().clone(),
                colors,
                // Setting null to the second field, as only one input field should request focus by default and a
                // possible non-null requester was passed to the first field.
                None,
            );
        },
    );
}

const TextFieldSpacing: Dp = Dp(8.0);
