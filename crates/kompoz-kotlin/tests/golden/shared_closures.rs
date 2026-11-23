#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn SharedClosures() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let threshold = SwipeToDismissBoxDefaults::positional_threshold();
    let state = remember(|| SwipeToDismissBoxState(SwipeToDismissBoxValue::EndToStart, threshold.clone()));
    let own = remember_swipe_to_dismiss_box_state(positional_threshold = threshold.clone());
    let typed = SwipeToDismissBoxDefaults::positional_threshold();
    let third = remember_swipe_to_dismiss_box_state(positional_threshold = typed.clone());
    let fourth = remember_swipe_to_dismiss_box_state(positional_threshold = typed.clone());
    Text(::kompoz::runtime::kt::template!(
        threshold(400_f32),
        " ",
        state.current_value() == own.current_value(),
        " ",
        third.current_value() == fourth.current_value()
    ));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn LambdaValues() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let count = remember(|| ::kompoz::runtime::mutable_state_of!(0));
    let on_refresh = {
        let count = ::kompoz::runtime::__private::capture(&count);
        move || {
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                &count,
                ::kompoz::runtime::kt::KtDelegate::get_value(&count) + 1,
            );
        }
    };
    let described = {
        let count = ::kompoz::runtime::__private::capture(&count);
        move || ::kompoz::runtime::kt::template!("sayı ", ::kompoz::runtime::kt::KtDelegate::get_value(&count))
    };
    Column(|| {
        Button(on_refresh.clone(), || {
            Text(described());
        });
        PullToRefreshBox(false, on_refresh.clone(), || {
            Text(::kompoz::runtime::kt::template!(::kompoz::runtime::kt::KtDelegate::get_value(&count)));
        });
    });
}
