#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Launched(xs: ::std::vec::Vec<i32>, log: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let scope = remember_coroutine_scope();
    LaunchedEffect((), async {
        ::kompoz::runtime::kt::KtList::for_each(&xs, |it| {
            scope.launch({
                let it = ::kompoz::runtime::__private::capture(&it);
                async move {
                    delay(::kompoz::runtime::kt::KtNumber::to_long(it)).await;
                }
            });
        });
        ::kompoz::runtime::kt::KtList::for_each(&xs, |x| {
            scope.launch({
                let log = ::kompoz::runtime::__private::capture(&log);
                let x = ::kompoz::runtime::__private::capture(&x);
                async move {
                    log(x);
                }
            });
        });
        ::kompoz::runtime::kt::KtList::for_each(&xs, |it| {
            ::kompoz::runtime::kt::run(|| log(it + 1));
        });
        ::kompoz::runtime::kt::KtList::for_each(&xs, |it| {
            ::kompoz::runtime::kt::with(scope.clone(), |this: &mut _| {
                this.launch({
                    let log = ::kompoz::runtime::__private::capture(&log);
                    let it = ::kompoz::runtime::__private::capture(&it);
                    async move {
                        log(it * 2);
                    }
                })
            });
        });
        ::kompoz::runtime::kt::KtList::for_each(&::kompoz::runtime::kt::KtList::map(&xs, |it| it + 1), |it| {
            ::kompoz::runtime::kt::KtList::for_each(&::kompoz::runtime::kt::list_of([it]), |it| {
                log(it);
            });
        });
        ::kompoz::runtime::kt::KtList::for_each(&xs, |_| {
            scope.launch({
                let log = ::kompoz::runtime::__private::capture(&log);
                async move {
                    ::kompoz::runtime::kt::KtList::for_each(&::kompoz::runtime::kt::list_of([1]), |it| {
                        log(it);
                    });
                }
            });
        });
        ::kompoz::runtime::kt::KtList::for_each(&xs, |_| {
            scope.launch({
                let log = ::kompoz::runtime::__private::capture(&log);
                async move {
                    log(0);
                }
            });
        });
        ::kompoz::runtime::kt::repeat(2, |it| {
            scope.launch({
                let log = ::kompoz::runtime::__private::capture(&log);
                let it = ::kompoz::runtime::__private::capture(&it);
                async move {
                    delay(10).await;
                    log(it);
                }
            });
        });
    });
    Box(Modifier.pointer_input((), {
        let xs = ::kompoz::runtime::__private::capture(&xs);
        let log = ::kompoz::runtime::__private::capture(&log);
        move |this| {
            let xs = ::kompoz::runtime::__private::capture(&xs);
            let log = ::kompoz::runtime::__private::capture(&log);
            async move {
                ::kompoz::runtime::kt::KtList::for_each_async(&xs, async |it| {
                    this.await_pointer_event_scope({
                        let log = ::kompoz::runtime::__private::capture(&log);
                        let it = ::kompoz::runtime::__private::capture(&it);
                        move |this1| {
                            let log = ::kompoz::runtime::__private::capture(&log);
                            let it = ::kompoz::runtime::__private::capture(&it);
                            async move {
                                let _ = &this1;
                                log(it)
                            }
                        }
                    })
                    .await;
                })
                .await;
            }
        }
    }));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Unknown(log: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    made_elsewhere(|| made_elsewhere(|it| log(it.clone())));
    made_elsewhere(|it| {
        made_elsewhere2(|| log(0));
        log(it.clone())
    });
}
