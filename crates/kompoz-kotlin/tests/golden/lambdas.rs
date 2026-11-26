#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Drawing(color: Color) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Canvas(Modifier.size(100.dp()), |this| {
        {
            let __kz_a1 = color;
            let __kz_a2 = this.size().min_dimension() / 4.0;
            this.draw_circle(__kz_a1, radius = __kz_a2)
        };
        this.draw_rect(Color::Blue, top_left = Offset(10_f32, 10_f32), size = Size(20_f32, 20_f32));
        {
            let __kz_a3 = color;
            let __kz_a4 = Offset::Zero;
            let __kz_a5 = this.center();
            let __kz_a6 = 2_f32;
            this.draw_line(__kz_a3, start = __kz_a4, end = __kz_a5, stroke_width = __kz_a6)
        };
    });
    Box(Modifier
        .graphics_layer_block(move |this| {
            this.set_scale_x(2_f32);
            this.set_alpha(0.5_f32);
        })
        .semantics(move |this| {
            this.set_content_description(::std::string::String::from("kutu"));
        }));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Effects(key: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let ticks = remember(|| ::kompoz::runtime::mutable_state_of!(0));
    let scope = remember_coroutine_scope();
    LaunchedEffect(key, async {
        while ::kompoz::runtime::kt::KtDelegate::get_value(&ticks) < 3 {
            delay(100).await;
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                &ticks,
                ::kompoz::runtime::kt::KtDelegate::get_value(&ticks) + 1,
            );
        }
    });
    LaunchedEffect((key, ::kompoz::runtime::kt::KtDelegate::get_value(&ticks)), async {
        ::kompoz::runtime::kt::println(::kompoz::runtime::kt::KtDelegate::get_value(&ticks));
    });
    DisposableEffect((), || {
        ::kompoz::runtime::kt::println("başladı");
        on_dispose(move || {
            ::kompoz::runtime::kt::println("bitti");
        })
    });
    SideEffect(|| {
        ::kompoz::runtime::kt::println(::kompoz::runtime::kt::KtDelegate::get_value(&ticks));
    });
    Box(Modifier
        .clickable({
            let scope = ::kompoz::runtime::__private::capture(&scope);
            let ticks = ::kompoz::runtime::__private::capture(&ticks);
            move || {
                scope.launch({
                    let ticks = ::kompoz::runtime::__private::capture(&ticks);
                    async move {
                        delay(10).await;
                        ::kompoz::runtime::kt::KtMutableDelegate::set_value(&ticks, 0);
                    }
                });
            }
        })
        .pointer_input((), {
            let ticks = ::kompoz::runtime::__private::capture(&ticks);
            move |this| {
                let ticks = ::kompoz::runtime::__private::capture(&ticks);
                async move {
                    this.detect_tap_gestures()
                        .on_tap({
                            let ticks = ::kompoz::runtime::__private::capture(&ticks);
                            move |_| {
                                ::kompoz::runtime::kt::post_inc(&ticks);
                            }
                        })
                        .await;
                }
            }
        }));
    KompozitionLocalProvider(&[LocalDensity.provides(Density(2_f32))], || {
        Text(::kompoz::runtime::kt::template!(LocalDensity.current().density()))
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Stacked(content: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Layout(
        content = content,
        modifier = Modifier.test_tag("stack"),
        measure_policy = ::kompoz::ui::layout::measure_policy(move |this, measurables, constraints| {
            let mut placeables = ::kompoz::runtime::kt::KtList::map(&measurables, |it| it.r#measure(constraints));
            let height = ::kompoz::runtime::kt::KtList::sum_of(&placeables, |it| it.height());
            this.layout(constraints.max_width(), height, {
                let placeables = ::kompoz::runtime::__private::capture(&placeables);
                move |this1| {
                    let mut y = 0;
                    ::kompoz::runtime::kt::KtList::for_each(&placeables, |it| {
                        this1.place(&it, 0, y);
                        y += it.height();
                    });
                }
            })
        }),
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn lambda_values() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let add = move |a: i32, b: i32| a + b;
        let mut pairs = ::kompoz::runtime::kt::list_of([
            ::kompoz::runtime::kt::to(1, ::std::string::String::from("a")),
            ::kompoz::runtime::kt::to(2, ::std::string::String::from("b")),
        ]);
        let mut names = ::kompoz::runtime::kt::KtList::map(&pairs, |__kz_d7| {
            let n = __kz_d7.component1();
            let s = __kz_d7.component2();
            ::kompoz::runtime::kt::template!(s, n)
        });
        let mut ignored = ::kompoz::runtime::kt::KtList::map(&pairs, |__kz_d8| {
            let s = __kz_d8.component2();
            s.clone()
        });
        let mut firsts = ::kompoz::runtime::kt::KtList::map(&pairs, |p| ::kompoz::runtime::kt::KtPair::first(&p));
        let annotated = add(1, 2);
        return add(::kompoz::runtime::kt::KtList::size(&names), ::kompoz::runtime::kt::KtList::size(&firsts))
            + ::kompoz::runtime::kt::KtList::size(&ignored)
            + annotated.clone();
    })
}
