#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Values(open: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let scale = animate_float_as_state(
        if open { 1.2_f32 } else { 1_f32 },
        ::std::sync::Arc::new(spring(damping_ratio = Spring::DampingRatioMediumBouncy)),
        label = ::std::string::String::from("scale"),
    );
    let transition = update_transition(
        target_state = open,
        label = ::core::option::Option::Some(::std::string::String::from("panel")),
    );
    let size = transition.animate_dp(
        transition_spec = move |this: &Segment<_>| -> ::std::sync::Arc<dyn FiniteAnimationSpec<Dp>> {
            if this.is_transitioning_to(false, true) {
                ::std::sync::Arc::new(tween(duration_millis = 160))
            } else {
                ::std::sync::Arc::new(snap())
            }
        },
        target_value_by_state = |it| {
            let it = ::core::clone::Clone::clone(it);
            if it { 200.dp() } else { 80.dp() }
        },
    );
    let count = transition.animate_int(|it| {
        let it = ::core::clone::Clone::clone(it);
        if it { 2 } else { 1 }
    });
    Text(
        ::kompoz::runtime::kt::template!(
            "ölçek ",
            ::kompoz::runtime::kt::KtFloat::round_to_int((::kompoz::runtime::kt::KtDelegate::get_value(&scale) * 10.0)),
            " ",
            ::kompoz::runtime::kt::KtDelegate::get_value(&count)
        ),
        modifier = Modifier.size(::kompoz::runtime::kt::KtDelegate::get_value(&size)),
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Steps(step: MutableTransitionState<i32>, seekable: SeekableTransitionState<i32>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let steps =
        remember_transition(step.clone(), label = ::core::option::Option::Some(::std::string::String::from("adım")));
    let x = steps.animate_float(
        transition_spec = move |this: &Segment<_>| -> ::std::sync::Arc<dyn FiniteAnimationSpec<f32>> {
            let _ = &this;
            ::std::sync::Arc::new(tween(duration_millis = 160))
        },
        target_value_by_state = |it| {
            let it = ::core::clone::Clone::clone(it);
            (it as f32) * 10_f32
        },
    );
    let seeking = remember_transition(seekable.clone());
    let child = seeking.create_child_transition(|it| {
        let it = ::core::clone::Clone::clone(it);
        it > 1
    });
    let y = child.animate_float(|it| if it.clone() { 1_f32 } else { 0_f32 });
    Text(::kompoz::runtime::kt::template!(
        ::kompoz::runtime::kt::KtDelegate::get_value(&x),
        " ",
        ::kompoz::runtime::kt::KtDelegate::get_value(&y)
    ));
}
