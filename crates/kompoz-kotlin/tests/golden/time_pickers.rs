#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn TimePickers(height: Dp) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_time_picker_state(initial_hour = 9, is_24_hour = true);
    Column(|| {
        if height > TimePickerDialogDefaults::MinHeightForTimePicker {
            TimePicker(
                state = {
                    let __kz_f1: ::std::rc::Rc<dyn TimePickerState> = state.clone();
                    __kz_f1
                },
                layout_type = TimePickerLayoutType::Vertical,
            );
        } else {
            TimeInput({
                let __kz_f2: ::std::rc::Rc<dyn TimePickerState> = state.clone();
                __kz_f2
            });
        }
        Text(::kompoz::runtime::kt::template!(
            state.hour(),
            " ",
            state.is_pm(),
            " ",
            state.is_input_valid(),
            " ",
            state.selection()
        ));
        Button(
            || {
                state.set_selection(TimePickerSelectionMode::Minute);
                state.set_hour(12);
            },
            || {
                Text("Minutes");
            },
        );
    });
}
