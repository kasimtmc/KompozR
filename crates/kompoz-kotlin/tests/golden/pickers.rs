#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn YearsAndElvis(month: i64) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_date_picker_state(
        initial_displayed_month_millis = ::core::option::Option::Some(::core::option::Option::Some(month)),
        year_range = ::kompoz::runtime::kt::to_closed_range(::kompoz::runtime::kt::range_to(2000, 2030)),
    );
    DatePicker({
        let __kz_f1: ::std::rc::Rc<dyn DatePickerState> = state.clone();
        __kz_f1
    });
    Text(::kompoz::runtime::kt::template!(
        "picked ",
        ::kompoz::runtime::kt::elvis(state.selected_date_millis(), || ::std::string::String::from("none")),
        " in ",
        state.year_range()
    ));
    TextButton(
        || {
            state.set_display_mode(DisplayMode::Input);
            state.set_selected_date_millis(::core::option::Option::None);
        },
        || {
            Text("type");
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn DateRange() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_date_range_picker_state_of_dates(
        ::core::option::Option::Some(LocalDate::of(2023, 4, 15)),
        ::core::option::Option::Some(LocalDate::of(2023, 4, 20)),
    );
    let start = state.get_selected_start_date();
    if let ::core::option::Option::Some(start) = start.clone() {
        let range = ::kompoz::runtime::kt::range_to(
            start.clone(),
            ::kompoz::runtime::kt::not_null(state.get_selected_end_date()),
        );
        Text(::kompoz::runtime::kt::template!("range ", range, " from ", start.day_of_week()));
    }
    Text(::kompoz::runtime::kt::template!(::kompoz::runtime::kt::elvis(state.get_selected_end_date(), || {
        ::std::string::String::from("open")
    })));
}
