#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn underline_subsequence(
    needle: impl ::core::convert::Into<::std::string::String>,
    haystack: impl ::core::convert::Into<::std::string::String>,
) -> ::core::option::Option<AnnotatedString> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let needle: ::std::string::String = ::core::convert::Into::into(needle);
    let haystack: ::std::string::String = ::core::convert::Into::into(haystack);
    ::kompoz::runtime::named_args!({
        return ::core::option::Option::Some({
            let __kz_leave7_12 = ::kompoz::runtime::kt::Leave::new();
            match ::kompoz::runtime::kt::leaving(&__kz_leave7_12, || {
                build_annotated_string(|this: &mut AnnotatedStringBuilder| {
                    let mut i = 0;
                    for char in ::core::primitive::str::chars(&needle).collect::<::std::vec::Vec<char>>() {
                        let start = i;
                        ::kompoz::runtime::kt::KtScope::r#let(
                            ::kompoz::runtime::kt::KtStr::index_of(&haystack, char, i, true),
                            |it| {
                                if it < 0 {
                                    ::kompoz::runtime::kt::leave(
                                        &__kz_leave7_12,
                                        ::kompoz::runtime::kt::Jump::Return(::core::option::Option::None),
                                    );
                                } else {
                                    i = it;
                                }
                            },
                        );
                        this.append(::kompoz::runtime::kt::KtStr::substring(
                            &haystack,
                            start,
                            ::core::option::Option::Some(i),
                        ));
                        this.with_style(
                            SpanStyle(text_decoration = ::core::option::Option::Some(TextDecoration::Underline)),
                            |this1| this1.append(::kompoz::runtime::kt::KtStr::get(&haystack, i)),
                        );
                        i += 1;
                    }
                    this.append(::kompoz::runtime::kt::KtStr::substring(
                        &haystack,
                        i,
                        ::core::option::Option::Some(::kompoz::runtime::kt::KtStr::length(&haystack)),
                    ));
                })
            }) {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_j) => match __kz_j {
                    ::kompoz::runtime::kt::Jump::Return(__kz_x) => return __kz_x,
                    #[allow(unreachable_patterns)]
                    _ => ::core::unreachable!(),
                },
            }
        });
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn loops(xs: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut n = 0;
        for x in ::kompoz::runtime::kt::KtIterable::kt_iter(&xs) {
            {
                let __kz_leave27_9 = ::kompoz::runtime::kt::Leave::<()>::new();
                match ::kompoz::runtime::kt::leaving(&__kz_leave27_9, || {
                    ::kompoz::runtime::kt::KtScope::r#let(x, |it| {
                        if it == 2 {
                            ::kompoz::runtime::kt::leave(&__kz_leave27_9, ::kompoz::runtime::kt::Jump::Loop(0u32));
                        }
                        if it == 5 {
                            ::kompoz::runtime::kt::leave(&__kz_leave27_9, ::kompoz::runtime::kt::Jump::Loop(1u32));
                        }
                    })
                }) {
                    ::core::result::Result::Ok(__kz_v) => __kz_v,
                    ::core::result::Result::Err(__kz_j) => match __kz_j {
                        ::kompoz::runtime::kt::Jump::Loop(0u32) => continue,
                        ::kompoz::runtime::kt::Jump::Loop(1u32) => break,
                        #[allow(unreachable_patterns)]
                        _ => ::core::unreachable!(),
                    },
                }
            };
            n += x;
        }
        return n;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn outer_label(xs: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::run(|| {
            {
                let __kz_leave34_5 = ::kompoz::runtime::kt::Leave::new();
                match ::kompoz::runtime::kt::leaving(&__kz_leave34_5, || {
                    ::kompoz::runtime::kt::KtList::map(&xs, |it| {
                        if it == 2 {
                            ::kompoz::runtime::kt::leave(&__kz_leave34_5, ::kompoz::runtime::kt::Jump::Return(it * 10));
                        }
                        it
                    })
                }) {
                    ::core::result::Result::Ok(__kz_v) => __kz_v,
                    ::core::result::Result::Err(__kz_j) => match __kz_j {
                        ::kompoz::runtime::kt::Jump::Return(__kz_x) => return __kz_x,
                        #[allow(unreachable_patterns)]
                        _ => ::core::unreachable!(),
                    },
                }
            };
            0
        })
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn guarded(log: SnapshotStateList<::std::string::String>, block: impl ::core::ops::Fn()) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_r = match ::kompoz::runtime::kt::catching(|| {
                block();
                log.add(::std::string::String::from("sonra"));
            }) {
                ::core::result::Result::Ok(__kz_v) => ::core::result::Result::Ok(__kz_v),
                ::core::result::Result::Err(__kz_e) => ::kompoz::runtime::kt::catching(|| {
                    if __kz_e.is_a("Throwable") {
                        #[allow(unused_variables)]
                        let e = __kz_e;
                        log.add(::std::string::String::from("yakalandı"));
                    } else {
                        ::kompoz::runtime::kt::rethrow(__kz_e)
                    }
                }),
            };
            {
                log.add(::std::string::String::from("finally"));
            };
            match __kz_r {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_e) => ::kompoz::runtime::kt::rethrow(__kz_e),
            }
        };
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[doc(hidden)]
pub async fn guarded_async(log: SnapshotStateList<::std::string::String>, block: impl ::core::ops::AsyncFn()) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_r = match ::kompoz::runtime::kt::catching_async(async {
                {
                    block().await;
                    log.add(::std::string::String::from("sonra"));
                }
            })
            .await
            {
                ::core::result::Result::Ok(__kz_v) => ::core::result::Result::Ok(__kz_v),
                ::core::result::Result::Err(__kz_e) => ::kompoz::runtime::kt::catching(|| {
                    if __kz_e.is_a("Throwable") {
                        #[allow(unused_variables)]
                        let e = __kz_e;
                        log.add(::std::string::String::from("yakalandı"));
                    } else {
                        ::kompoz::runtime::kt::rethrow(__kz_e)
                    }
                }),
            };
            {
                log.add(::std::string::String::from("finally"));
            };
            match __kz_r {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_e) => ::kompoz::runtime::kt::rethrow(__kz_e),
            }
        };
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn through_own(log: SnapshotStateList<::std::string::String>, stop: bool) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_leave50_5 = ::kompoz::runtime::kt::Leave::new();
            match ::kompoz::runtime::kt::leaving(&__kz_leave50_5, || {
                guarded(log.clone(), || {
                    if stop {
                        ::kompoz::runtime::kt::leave(
                            &__kz_leave50_5,
                            ::kompoz::runtime::kt::Jump::Return(::std::string::String::from("erken")),
                        );
                    }
                })
            }) {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_j) => match __kz_j {
                    ::kompoz::runtime::kt::Jump::Return(__kz_x) => return __kz_x,
                    #[allow(unreachable_patterns)]
                    _ => ::core::unreachable!(),
                },
            }
        };
        return ::std::string::String::from("sonuna");
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn waits_then_leaves(xs: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_leave57_5 = ::kompoz::runtime::kt::Leave::new();
            match ::kompoz::runtime::kt::leaving_async(&__kz_leave57_5, async {
                ::kompoz::runtime::kt::KtScope::let_async(xs.clone(), async |it| {
                    delay(1).await;
                    if ::kompoz::runtime::kt::KtList::size(&it) > 1 {
                        ::kompoz::runtime::kt::leave(
                            &__kz_leave57_5,
                            ::kompoz::runtime::kt::Jump::Return(::kompoz::runtime::kt::KtList::first(&it) + 100),
                        );
                    }
                })
                .await
            })
            .await
            {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_j) => match __kz_j {
                    ::kompoz::runtime::kt::Jump::Return(__kz_x) => return __kz_x,
                    #[allow(unreachable_patterns)]
                    _ => ::core::unreachable!(),
                },
            }
        };
        return -1;
    })
}
