#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn SharedClosures() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let threshold = SwipeToDismissBoxDefaults::positional_threshold();
    let state = remember(|| SwipeToDismissBoxState(SwipeToDismissBoxValue::EndToStart, threshold.clone()));
    let own = remember_swipe_to_dismiss_box_state(positional_threshold = threshold.clone());
    Text(::kompoz::runtime::kt::template!(threshold(400_f32), " ", state.current_value() == own.current_value()));
}
