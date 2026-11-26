#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Rows(xs: ::std::vec::Vec<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    {
        let __kz_leave8_5 = &::kompoz::runtime::kt::Leave::new();
        match ::kompoz::runtime::kt::leaving(__kz_leave8_5, || {
            Column(|| {
                if ::kompoz::runtime::kt::KtList::is_empty(&xs) {
                    ::kompoz::runtime::kt::leave(&__kz_leave8_5, ::kompoz::runtime::kt::Jump::Return(()));
                }
                Text(::kompoz::runtime::kt::KtList::first(&xs));
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
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Cut(xs: ::std::vec::Vec<::std::string::String>, limit: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        for __kz_d1 in ::kompoz::runtime::kt::KtIterable::kt_iter(&::kompoz::runtime::kt::KtList::with_index(&xs)) {
            let i = __kz_d1.component1();
            let x = __kz_d1.component2();
            {
                let __kz_leave18_13 = &::kompoz::runtime::kt::Leave::<()>::new();
                match ::kompoz::runtime::kt::leaving(__kz_leave18_13, || {
                    Row(|| {
                        if i >= limit {
                            ::kompoz::runtime::kt::leave(&__kz_leave18_13, ::kompoz::runtime::kt::Jump::Loop(0u32));
                        }
                        if x == "iki" {
                            ::kompoz::runtime::kt::leave(&__kz_leave18_13, ::kompoz::runtime::kt::Jump::Loop(1u32));
                        }
                        Text(::kompoz::runtime::kt::template!("satır ", x));
                    })
                }) {
                    ::core::result::Result::Ok(__kz_v) => __kz_v,
                    ::core::result::Result::Err(__kz_j) => match __kz_j {
                        ::kompoz::runtime::kt::Jump::Loop(0u32) => break,
                        ::kompoz::runtime::kt::Jump::Loop(1u32) => continue,
                        #[allow(unreachable_patterns)]
                        _ => ::core::unreachable!(),
                    },
                }
            };
        }
        Text("kesildi");
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Labeled(xs: ::std::vec::Vec<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        {
            let __kz_leave31_9 = &::kompoz::runtime::kt::Leave::new();
            match ::kompoz::runtime::kt::leaving(__kz_leave31_9, || {
                Row(|| {
                    if ::kompoz::runtime::kt::KtList::size(&xs) > 1 {
                        ::kompoz::runtime::kt::leave(&__kz_leave31_9, ::kompoz::runtime::kt::Jump::Return(()));
                    }
                    Text("tek");
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
        Text("çok değil");
    });
    Text("etiket sonu");
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn Framed(content: impl ::core::ops::Fn()) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let content = &content;
    Box(content = || {
        content();
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn Keyed(xs: ::std::vec::Vec<::std::string::String>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    for x in ::kompoz::runtime::kt::KtIterable::kt_iter(&xs) {
        {
            let __kz_leave48_9 = &::kompoz::runtime::kt::Leave::new();
            match ::kompoz::runtime::kt::leaving(__kz_leave48_9, || {
                key(&x.clone(), || {
                    Framed(|| {
                        if ::kompoz::runtime::kt::KtStr::is_empty(&x) {
                            ::kompoz::runtime::kt::leave(&__kz_leave48_9, ::kompoz::runtime::kt::Jump::Return(-1));
                        }
                        Text(x.clone());
                    })
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
    }
    return ::kompoz::runtime::kt::KtList::size(&xs);
}
