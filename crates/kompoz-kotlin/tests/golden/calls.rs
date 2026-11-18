#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Calls(items: ::std::vec::Vec<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        modifier = Modifier.padding(16.dp()).fill_max_width(),
        vertical_arrangement = Arrangement::spaced_by(4.dp()),
        content = || {
            Text("a", font_size = 20.sp());
            Text(text = "b", modifier = Modifier.test_tag("b"));
            Box(
                Modifier.padding_hv(horizontal = 8.dp()).size(20.dp()).background(Color(0xFF6200EE_i64)),
                content = || {},
            );
            Spacer(Modifier.height(4.dp()));
            Text(::kompoz::runtime::kt::KtList::join_to_string(
                &::kompoz::runtime::kt::KtList::map(
                    &::kompoz::runtime::kt::KtList::filter(&items, |it| {
                        ::kompoz::runtime::kt::KtStr::starts_with(&it, "a", false)
                    }),
                    |it| ::kompoz::runtime::kt::KtStr::uppercase(&it),
                ),
                "+",
                "",
                "",
                -1,
                "...",
                ::core::option::Option::None,
            ));
            Text(::kompoz::runtime::kt::KtList::join_to_string(
                &items,
                ", ",
                "[",
                "]",
                -1,
                "...",
                ::core::option::Option::None,
            ));
            ::kompoz::runtime::kt::KtList::for_each_indexed(&items, |i, s| {
                Text(::kompoz::runtime::kt::template!(i, ": ", s));
            });
            Text(::kompoz::runtime::kt::template!(
                ::kompoz::runtime::kt::KtList::size(&items),
                " ",
                ::kompoz::runtime::kt::KtList::first(&items),
                " ",
                ::kompoz::runtime::kt::elvis(::kompoz::runtime::kt::KtList::get_or_null(&items, 9), || {
                    ::std::string::String::from("-")
                })
            ));
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn colors() -> ::std::vec::Vec<Color> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let a = Color((0.5_f32, 0.25_f32, 1_f32));
        let b = Color((255, 0, 0));
        let c = Color::Red.copy(alpha = 0.5_f32);
        return ::kompoz::runtime::kt::list_of([a, b, c]);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn collections() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut xs = ::kompoz::runtime::kt::mutable_list_of([3, 1, 2]);
        ::kompoz::runtime::kt::KtMutableList::add(&mut xs, 4);
        let mut ys = ::kompoz::runtime::kt::KtList::reversed(&::kompoz::runtime::kt::KtList::sorted(&xs));
        let mut m = ::kompoz::runtime::kt::map_of([
            ::kompoz::runtime::kt::to(::std::string::String::from("a"), 1),
            ::kompoz::runtime::kt::to(::std::string::String::from("b"), 2),
        ]);
        let total = ::kompoz::runtime::kt::KtList::sum_of(&ys, |it| it * 2)
            + (::kompoz::runtime::kt::elvis(
                ::kompoz::runtime::kt::KtMapExt::get(&m, ::std::string::String::from("a")),
                || 0,
            ));
        ::kompoz::runtime::kt::repeat(3, |it| {
            ::kompoz::runtime::kt::println(it);
        });
        ::kompoz::runtime::kt::require(
            total > 0,
            ::core::option::Option::Some(&|| ::std::string::String::from("pozitif olmalı")),
        );
        return total.clone() + ::kompoz::runtime::kt::KtList::get(&xs, 0);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn text(s: impl ::core::convert::Into<::std::string::String>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let s: ::std::string::String = ::core::convert::Into::into(s);
    ::kompoz::runtime::named_args!({
        let mut parts = ::kompoz::runtime::kt::KtStr::split(&s, [::std::string::String::from(",")], false, 0);
        let n = ::kompoz::runtime::kt::KtStr::length(&s) + ::kompoz::runtime::kt::KtList::size(&parts);
        let trimmed = ::kompoz::runtime::kt::KtStr::replace(
            &::kompoz::runtime::kt::KtStr::lowercase(&::kompoz::runtime::kt::KtStr::trim(&s)),
            "a",
            "b",
            false,
        );
        return ::kompoz::runtime::kt::template!(
            n,
            " ",
            ::kompoz::runtime::kt::KtStr::substring(&trimmed, 1, ::core::option::Option::None),
            " ",
            ::kompoz::runtime::kt::KtStr::to_int(&s) + 1,
            " ",
            ::kompoz::runtime::kt::to_string(&n)
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn paddings() -> ::std::vec::Vec<PaddingValues> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::list_of([
            PaddingValues::all(16.dp()),
            PaddingValues::hv(12.dp(), 8.dp()),
            PaddingValues::hv(50.dp(), 0.dp()),
            PaddingValues::hv(0.dp(), 3.dp()),
            PaddingValues(start = 1.dp(), top = 2.dp()),
            PaddingValues(top = 5.dp()),
        ])
    })
}
