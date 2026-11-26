#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Underlined(content: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    SubcomposeLayout(
        modifier = Modifier.test_tag("underlined"),
        measure_policy = |this, constraints| {
            let __kz_this = ::core::clone::Clone::clone(this);
            let mut main = ::kompoz::runtime::kt::KtList::map(
                &this.subcompose(::std::string::String::from("main"), content.clone()),
                |it| it.r#measure(constraints.copy(min_width = 0, min_height = 0)),
            );
            let width = ::kompoz::runtime::kt::KtList::max_of(&main, |it| it.width());
            let height = ::kompoz::runtime::kt::KtList::sum_of(&main, |it| it.height());
            let mut line = ::kompoz::runtime::kt::KtList::map(
                &this.subcompose(::std::string::String::from("line"), {
                    let width = ::kompoz::runtime::__private::capture(&width);
                    let __kz_this = ::kompoz::runtime::__private::capture(&__kz_this);
                    move || {
                        Box(modifier = Modifier.size_wh(__kz_this.to_dp((width as f32)), 2.dp()));
                    }
                }),
                |it| it.r#measure(constraints),
            );
            this.layout(width, height + this.round_to_px(2.dp()), {
                let main = ::kompoz::runtime::__private::capture(&main);
                let line = ::kompoz::runtime::__private::capture(&line);
                let height = ::kompoz::runtime::__private::capture(&height);
                move |this1| {
                    ::kompoz::runtime::kt::KtList::for_each(&main, |it| {
                        this1.place_relative(&it, 0, 0);
                    });
                    ::kompoz::runtime::kt::KtList::for_each(&line, |it| {
                        this1.place_relative(&it, 0, height);
                    });
                }
            })
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Adaptive() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    BoxWithConstraints(
        modifier = Modifier.fill_max_size(),
        content = |this| {
            Text(if this.max_width() < 600.dp() {
                ::kompoz::runtime::kt::template!("compact ", this.constraints().max_width())
            } else {
                ::kompoz::runtime::kt::template!("expanded ", this.min_height())
            });
        },
    );
}
