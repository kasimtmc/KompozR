#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Grow(done: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let size = remember(|| Animatable(0_f32));
    LaunchedEffect((), async {
        size.animate_to(
            100_f32,
            animation_spec = ::std::sync::Arc::new(tween(300, easing = ::std::sync::Arc::new(LinearEasing))),
        )
        .await;
        size.animate_to(
            40_f32,
            animation_spec = ::std::sync::Arc::new(spring(
                damping_ratio = Spring::DampingRatioNoBouncy,
                stiffness = Spring::StiffnessMedium,
            )),
        )
        .await;
        done();
    });
    Box(Modifier.test_tag("grow").size(size.value().dp()));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Slide(offset: Animatable<Offset, AnimationVector2D>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect(offset.clone(), async {
        offset
            .animate_to(
                Offset(100_f32, 50_f32),
                animation_spec = ::std::sync::Arc::new(tween(duration_millis = 160, delay_millis = 0)),
            )
            .await;
        offset.snap_to(Offset::Zero).await;
    });
    Box(Modifier.test_tag("slide").offset(offset.value().x().dp(), y = offset.value().y().dp()).size(10.dp()));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Pulse(log: impl ::core::ops::Fn(f32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let scale = remember(|| Animatable(1_f32));
    let scope = remember_coroutine_scope();
    Box(Modifier.test_tag("pulse").size(20.dp()).clickable({
        let scope = ::kompoz::runtime::__private::capture(&scope);
        let scale = ::kompoz::runtime::__private::capture(&scale);
        let log = ::kompoz::runtime::__private::capture(&log);
        move || {
            scope.launch({
                let scale = ::kompoz::runtime::__private::capture(&scale);
                let log = ::kompoz::runtime::__private::capture(&log);
                async move {
                    scale
                        .animate_to(
                            1_f32,
                            ::std::sync::Arc::new(keyframes(|this: &mut KeyframesSpecConfig<_>| {
                                this.set_duration_millis(400);
                                this.at(1.3_f32, 100).using(FastOutSlowInEasing);
                                this.at(0.9_f32, 250);
                            })),
                            block = ::core::option::Option::Some({
                                let __kz_f1: ::std::boxed::Box<dyn ::core::ops::FnMut(&'_ Animatable<_, _>)> =
                                    ::std::boxed::Box::new({
                                        let log = ::kompoz::runtime::__private::capture(&log);
                                        move |this: &'_ Animatable<_, _>| {
                                            log(this.value());
                                        }
                                    });
                                __kz_f1
                            }),
                        )
                        .await;
                }
            });
        }
    }));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Fling(target: impl ::core::ops::Fn(f32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let position = remember(|| Animatable((0_f32, Offset::VisibilityThreshold.x())));
    LaunchedEffect((), async {
        position.update_bounds(upper_bound = ::core::option::Option::Some(150_f32));
        let result = position.animate_decay(1000_f32, exponential_decay()).await;
        target(position.value());
        if result.end_reason() == AnimationEndReason::BoundReached {
            position.stop().await;
        }
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Ticker(seen: impl ::core::ops::Fn(f32, f32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect((), async {
        animate(
            0_f32,
            10_f32,
            animation_spec = ::std::sync::Arc::new(tween(100, easing = ::std::sync::Arc::new(LinearEasing))),
            block = |value, velocity| {
                seen(value, velocity);
            },
        )
        .await;
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Forever(ticks: impl ::core::ops::Fn(f32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let angle = remember(|| Animatable(0_f32));
    LaunchedEffect((), async {
        angle
            .animate_to(
                360_f32,
                ::std::sync::Arc::new(infinite_repeatable(
                    tween(1000, easing = ::std::sync::Arc::new(LinearEasing)),
                    repeat_mode = RepeatMode::Restart,
                )),
                block = ::core::option::Option::Some({
                    let __kz_f2: ::std::boxed::Box<dyn ::core::ops::FnMut(&'_ Animatable<_, _>)> =
                        ::std::boxed::Box::new({
                            let ticks = ::kompoz::runtime::__private::capture(&ticks);
                            move |this: &'_ Animatable<_, _>| {
                                ticks(this.value());
                            }
                        });
                    __kz_f2
                }),
            )
            .await;
    });
}
