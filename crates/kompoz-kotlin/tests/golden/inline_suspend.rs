#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn load(n: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        delay(1).await;
        return n * 2;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Scoped(
    anim: Animatable<f32, AnimationVector1D>,
    maybe: impl ::core::convert::Into<::core::option::Option<Animatable<f32, AnimationVector1D>>>,
    log: impl ::core::ops::Fn(i32) + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect((), async {
        ::kompoz::runtime::kt::with_async(anim.clone(), async |this: &mut _| this.animate_to(1_f32).await).await;
        let bound = ::kompoz::runtime::kt::with_async(anim.clone(), async |this: &mut _| {
            this.update_bounds(upper_bound = ::core::option::Option::Some(4_f32));
            this.animate_to(2_f32).await;
            if this.value() > 1_f32 {
                return 7;
            }
            3
        })
        .await;
        ::kompoz::runtime::kt::KtScope::run_async(anim.clone(), async |this| this.animate_to(3_f32).await).await;
        ::kompoz::runtime::kt::KtScope::also_async(
            ::kompoz::runtime::kt::KtScope::apply_async(anim.clone(), async |this| {
                this.snap_to(0_f32).await;
            })
            .await,
            async |it| {
                it.animate_to(4_f32).await;
            },
        )
        .await;
        let got: f32 = ::kompoz::runtime::kt::KtScope::let_async(anim.clone(), async |it| {
            it.animate_to(5_f32).await;
            it.value()
        })
        .await;
        let kept = ::kompoz::runtime::kt::KtScope::take_if_async(anim.clone(), async |it| {
            it.animate_to(6_f32).await;
            true
        })
        .await;
        ::kompoz::runtime::kt::KtScope::take_unless_async(anim.clone(), async |_| load(1).await > 1).await;
        let ran = ::kompoz::runtime::kt::run_async(async || {
            delay(10).await;
            load(3).await
        })
        .await;
        ::kompoz::runtime::kt::with(anim.clone(), |this: &mut _| {
            this.update_bounds(upper_bound = ::core::option::Option::Some(9_f32))
        });
        log(bound + ::kompoz::runtime::kt::KtNumber::to_int(got) + ran);
        ::kompoz::runtime::kt::flatten!(
            ::kompoz::runtime::kt::safe_async(maybe.clone(), async |__kz_it1| __kz_it1.animate_to(2_f32).await).await
        );
        ::kompoz::runtime::kt::flatten!(
            ::kompoz::runtime::kt::safe_async(maybe.clone(), async |__kz_it2| {
                ::kompoz::runtime::kt::KtScope::let_async(__kz_it2, async |it| it.animate_to(3_f32).await).await
            })
            .await
        );
        let n: ::core::option::Option<i32> = ::core::option::Option::None;
        let m = ::kompoz::runtime::kt::elvis_async(n, async || load(4).await).await;
        log(m);
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Collections(anim: Animatable<f32, AnimationVector1D>, log: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let scope = remember_coroutine_scope();
    LaunchedEffect((), async {
        let mut total = 0;
        ::kompoz::runtime::kt::repeat_async(3, async |it| {
            anim.animate_to(::kompoz::runtime::kt::KtNumber::to_float(it)).await;
        })
        .await;
        ::kompoz::runtime::kt::KtList::for_each_async(&::kompoz::runtime::kt::list_of([1, 2, 3]), async |it| {
            delay(10).await;
            total += it;
        })
        .await;
        ::kompoz::runtime::kt::KtList::for_each_indexed_async(&::kompoz::runtime::kt::list_of([4, 5]), async |i, x| {
            log(load(i).await + x);
        })
        .await;
        ::kompoz::runtime::kt::KtList::for_each_async(&::kompoz::runtime::kt::list_of([1, 2, 3, 4]), async |it| {
            if it % 2 == 0 {
                return;
            }
            delay(1).await;
            log(it);
        })
        .await;
        ::kompoz::runtime::kt::KtList::for_each_async(&::kompoz::runtime::kt::list_of([6, 7]), async |it| {
            if it == 6 {
                return;
            }
            log(load(it).await);
        })
        .await;
        let mut doubled =
            ::kompoz::runtime::kt::KtList::map_async(&::kompoz::runtime::kt::list_of([1, 2]), async |it| {
                load(it).await
            })
            .await;
        let mut odd =
            ::kompoz::runtime::kt::KtList::filter_async(&::kompoz::runtime::kt::list_of([1, 2, 3]), async |it| {
                delay(1).await;
                it % 2 == 1
            })
            .await;
        let any = ::kompoz::runtime::kt::KtList::any_that_async(&::kompoz::runtime::kt::list_of([1, 2]), async |it| {
            load(it).await > 3
        })
        .await;
        let sum =
            ::kompoz::runtime::kt::KtList::fold_async(&::kompoz::runtime::kt::list_of([1, 2]), 0, async |acc, x| {
                acc.clone() + load(x).await
            })
            .await;
        let mut chars = ::kompoz::runtime::kt::KtStr::map_async(&"ab", async |it| {
            delay(1).await;
            ::kompoz::runtime::kt::KtChar::uppercase_char(it)
        })
        .await;
        ::kompoz::runtime::kt::KtMapExt::for_each_async(
            &::kompoz::runtime::kt::map_of([::kompoz::runtime::kt::to(::std::string::String::from("a"), 1)]),
            async |__kz_d3| {
                let k = __kz_d3.component1();
                let v = __kz_d3.component2();
                delay(1).await;
                log(k.length() + v.clone());
            },
        )
        .await;
        let mut cache = ::core::convert::identity::<::kompoz::runtime::kt::KtMap<::std::string::String, i32>>(
            ::kompoz::runtime::kt::mutable_map_of([]),
        );
        let cached = cache.get_or_put_async(::std::string::String::from("a"), async || load(5).await).await;
        let mut made = ::kompoz::runtime::kt::List_async(2, async |it| load(it).await).await;
        ::kompoz::runtime::kt::require_async(
            total > 0,
            ::core::option::Option::Some(&async || ::kompoz::runtime::kt::template!("total ", load(total).await)),
        )
        .await;
        ::kompoz::runtime::kt::KtList::for_each_async(&::kompoz::runtime::kt::list_of([1, 2]), async |x| {
            ::kompoz::runtime::kt::repeat_async(2, async |_| {
                delay(::kompoz::runtime::kt::KtNumber::to_long(x)).await;
            })
            .await;
        })
        .await;
        ::kompoz::runtime::kt::repeat_async(2, async |_| {
            for i in ::kompoz::runtime::kt::until(0, 5) {
                if i == 2 {
                    break;
                }
                delay(1).await;
            }
        })
        .await;
        ::kompoz::runtime::kt::KtList::for_each(&::kompoz::runtime::kt::list_of([1, 2]), |it| {
            log(it);
        });
        ::kompoz::runtime::kt::KtList::for_each(&::kompoz::runtime::kt::list_of([1, 2]), |x| {
            scope.launch({
                let x = ::kompoz::runtime::__private::capture(&x);
                async move {
                    delay(::kompoz::runtime::kt::KtNumber::to_long(x)).await;
                }
            });
        });
        ::kompoz::runtime::kt::KtList::for_each_indexed_async(&made, async |i, _| {
            delay(::kompoz::runtime::kt::KtNumber::to_long(i)).await;
        })
        .await;
        log(total
            + ::kompoz::runtime::kt::KtList::size(&doubled)
            + ::kompoz::runtime::kt::KtList::size(&odd)
            + sum
            + ::kompoz::runtime::kt::KtList::size(&chars)
            + cached.clone()
            + ::kompoz::runtime::kt::KtList::size(&made)
            + if any { 1 } else { 0 });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn count_down(from: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut left = 0;
        ::kompoz::runtime::kt::KtList::for_each_async(&(::kompoz::runtime::kt::range_to(1, from)), async |it| {
            delay(1).await;
            left += it;
        })
        .await;
        return left;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn twice(block: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        block();
        block();
    })
}
// error 93:46: `load` suspends, in a lambda `sortedBy` doesn't run in the coroutine: Kotlin suspends in a coroutine and in the lambdas an inline function runs in place (`with`, `let`, `apply`, `forEach`, `map`, `repeat`, …; not a `crossinline` one: `sortedBy`'s)
// error 94:25: `delay` suspends, in a lambda `madeElsewhere` doesn't run in the coroutine: Kotlin suspends in a coroutine and in the lambdas an inline function runs in place (`with`, `let`, `apply`, `forEach`, `map`, `repeat`, …; not a `crossinline` one: `sortedBy`'s)
// error 105:30: `break`/`continue` outside a loop (or out of a lambda) isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 108:42: `return@LaunchedEffect` out of more than the lambda it is in isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
// error 119:36: a suspending call in a lambda given to the file's own `inline fun` isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)
