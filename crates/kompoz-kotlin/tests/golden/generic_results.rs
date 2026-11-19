#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Scoped(anim: Animatable<f32, AnimationVector1D>, log: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect((), async {
        let got = ::kompoz::runtime::kt::KtScope::let_async(anim.clone(), async |it| {
            it.animate_to(5_f32).await;
            it.value()
        })
        .await;
        log(::kompoz::runtime::kt::KtNumber::to_int(got));
        let ran = ::kompoz::runtime::kt::KtScope::run(anim.clone(), |this| this.target_value());
        log(::kompoz::runtime::kt::KtNumber::to_int(ran));
        let with = ::kompoz::runtime::kt::with(anim.clone(), |this: &mut _| this.value() * 2.0);
        log(::kompoz::runtime::kt::KtNumber::to_int(with));
        let same = ::kompoz::runtime::kt::KtScope::also_async(anim.clone(), async |it| {
            it.snap_to(1_f32).await;
        })
        .await
        .value();
        log(::kompoz::runtime::kt::KtNumber::to_int(same));
        let kept = ::kompoz::runtime::kt::KtScope::take_if(anim.value(), |it| it > 0_f32);
        log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::not_null(kept)));
        let bound = anim.lower_bound();
        log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::not_null(bound)));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Remembered(log: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let anim = remember(|| Animatable(0_f32));
    let color = remember(|| Animatable(Color::Red));
    LaunchedEffect((), async {
        let got = ::kompoz::runtime::kt::KtScope::let_async(anim.clone(), async |it| {
            it.animate_to(5_f32).await;
            it.value()
        })
        .await;
        log(::kompoz::runtime::kt::KtNumber::to_int(got));
        let red = color.value().red();
        log(::kompoz::runtime::kt::KtNumber::to_int(red));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Collections(log: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let mut floats = ::kompoz::runtime::kt::KtList::map(&::kompoz::runtime::kt::list_of([1, 2]), |it| {
        ::kompoz::runtime::kt::KtNumber::to_float(it)
    });
    log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::KtList::get(&floats, 0)));
    let mut some = ::kompoz::runtime::kt::KtList::map_not_null(&::kompoz::runtime::kt::list_of([1, 2]), |it| {
        if it > 1 {
            ::core::option::Option::Some(::kompoz::runtime::kt::KtNumber::to_float(it))
        } else {
            ::core::option::Option::None
        }
    });
    log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::KtList::first(&some)));
    let mut flat = ::kompoz::runtime::kt::KtList::flat_map(&::kompoz::runtime::kt::list_of([1, 2]), |it| {
        ::kompoz::runtime::kt::list_of([::kompoz::runtime::kt::KtNumber::to_float(it)])
    });
    log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::KtList::last(&flat)));
    let sum =
        ::kompoz::runtime::kt::KtList::fold(&::kompoz::runtime::kt::list_of([1, 2]), 0_f32, |acc, x| acc.clone() + x);
    log(::kompoz::runtime::kt::KtNumber::to_int(sum));
    let mut by_key = ::kompoz::runtime::kt::KtList::associate_with(&::kompoz::runtime::kt::list_of([1, 2]), |it| {
        ::kompoz::runtime::kt::KtNumber::to_double(it)
    });
    log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::not_null(
        ::kompoz::runtime::kt::KtMapExt::get(&by_key, 1),
    )));
    let most = ::kompoz::runtime::kt::KtList::max_of(&::kompoz::runtime::kt::list_of([1, 2]), |it| {
        ::kompoz::runtime::kt::KtNumber::to_float(it)
    });
    log(::kompoz::runtime::kt::KtNumber::to_int(most));
    let mut made = ::kompoz::runtime::kt::List(2, |it| ::kompoz::runtime::kt::KtNumber::to_float(it));
    log(::kompoz::runtime::kt::KtNumber::to_int(::kompoz::runtime::kt::KtList::get(&made, 1)));
    let first = ::kompoz::runtime::kt::KtScope::r#let(::kompoz::runtime::kt::list_of([1.5_f32]), |it| {
        ::kompoz::runtime::kt::KtList::first(&it)
    });
    log(::kompoz::runtime::kt::KtNumber::to_int(first));
}
