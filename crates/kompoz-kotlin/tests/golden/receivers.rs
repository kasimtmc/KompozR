#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub static LocalBase: KompozitionLocal<i32> = {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    kompozition_local_of(|| 1)
};
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub static LocalTwice: KompozitionLocal<i32> = {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    kompozition_local_of(|| 0)
};
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub static LocalHalf: KompozitionLocal<i32> = {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    kompozition_local_with_computed_default_of(|this| this.current_value(&LocalBase) / 2)
};
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Animated(seekable: SeekableTransitionState<i32>, deferred: DeferredTransitionState<i32>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let written: Animatable<f32, AnimationVector1D> = made_elsewhere();
    let remembered = remember(|| Animatable(0_f32));
    let built = remember(|| {
        let a = Animatable(0_f32);
        a.clone()
    });
    let delegated = remember(|| mutable_state_of(Animatable(0_f32)));
    let unknown = made_elsewhere();
    LaunchedEffect((), async {
        written.animate_to(1_f32).await;
        remembered.animate_to(1_f32).await;
        built.animate_to(1_f32).await;
        ::kompoz::runtime::kt::KtDelegate::get_value(&delegated).animate_to(1_f32).await;
        seekable.animate_to(target_state = 2).await;
        deferred.animate_to(3);
        ::kompoz::runtime::kt::with(remembered.clone(), |this: &mut _| {
            this.update_bounds(upper_bound = ::core::option::Option::Some(2_f32))
        });
        ::kompoz::runtime::kt::KtScope::r#let(remembered.clone(), |it| it.clone()).animate_to(3_f32).await;
        ::kompoz::runtime::kt::KtList::first(&::kompoz::runtime::kt::list_of([remembered.clone()]))
            .animate_to(4_f32)
            .await;
        remember(|| Animatable(0_f32)).animate_to(4_f32).await;
        unknown.update_bounds(upper_bound = ::core::option::Option::Some(1_f32));
        let stop = move |it: Animatable<f32, AnimationVector1D>| async move {
            it.stop().await;
        };
        stop(remembered.clone()).await;
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Computed(tint: State<i32>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    KompozitionLocalProvider(
        &[
            LocalTwice.provides_computed(|this| this.current_value(&LocalBase) * 2 + tint.value()),
            LocalBase.provides(4),
        ],
        content = || Text(::kompoz::runtime::kt::template!(LocalTwice.current(), " ", LocalHalf.current())),
    );
}
// error 36:28: write `unknown`'s type: KompozR can't tell which `animateTo` it calls (`AnchoredDraggableState`, `Animatable`, `AnimationState`, `DeferredTransitionState` and `SeekableTransitionState` have one) (`val unknown: Type = …`)
