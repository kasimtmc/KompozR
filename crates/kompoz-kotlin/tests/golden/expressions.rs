#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn literals() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let i = 16;
        let big = 3000000000_i64;
        let l = 10_i64;
        let f = 1.5_f32;
        let g = 2_f32;
        let d = 2.5;
        let e = 1e3;
        let hex = 0xFF00FF00_i64;
        let bin = 0b1010;
        let u = 7_u32;
        let c = 'ç';
        let b = true && !false;
        let s = ::std::string::String::from("tab\t\"quoted\" ğüş");
        let raw = ::kompoz::runtime::kt::KtStr::trim_indent(&"\n        bir\n          iki\n    ");
        ::kompoz::runtime::kt::println(::kompoz::runtime::kt::template!(
            i, " ", big, " ", l, " ", f, " ", g, " ", d, " ", e, " ", hex, " ", bin, " ", u, " ", c, " ", b, " ", s,
            " ", raw
        ));
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn arithmetic(width: f32, n: i32) -> f32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let half = width / 2.0;
        let third = 3.0 * width;
        let mixed = n * 2 + 1;
        let neg = -n % 3;
        return half + third + (mixed as f32) + (neg as f32) - 1.0;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn templates(
    name: impl ::core::convert::Into<::std::string::String>,
    ratio: f32,
    items: ::std::vec::Vec<::std::string::String>,
) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let name: ::std::string::String = ::core::convert::Into::into(name);
    ::kompoz::runtime::named_args!({
        let a = ::kompoz::runtime::kt::template!(
            "Ad: ",
            name,
            ", uzunluk: ",
            ::kompoz::runtime::kt::KtStr::length(&name),
            ", oran: ",
            ratio,
            ", iki kat: ",
            ratio * 2.0
        );
        let b = ::kompoz::runtime::kt::template!(
            "liste: ",
            items,
            ", boş mu: ",
            ::kompoz::runtime::kt::KtList::is_empty(&items),
            ", ",
            if ratio > 1_f32 {
                ::std::string::String::from("büyük")
            } else {
                ::std::string::String::from("küçük")
            }
        );
        let c = ::kompoz::runtime::kt::template!(::kompoz::runtime::kt::template!("dolar: ", '$', "5, "), name, 1, 'c');
        return ::kompoz::runtime::kt::template!(a, b, c);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn nulls(
    maybe: impl ::core::convert::Into<::core::option::Option<::std::string::String>>,
    other: impl ::core::convert::Into<::core::option::Option<::std::string::String>>,
) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let maybe: ::core::option::Option<::std::string::String> = ::core::convert::Into::into(maybe);
    let other: ::core::option::Option<::std::string::String> = ::core::convert::Into::into(other);
    ::kompoz::runtime::named_args!({
        let length = ::kompoz::runtime::kt::elvis(
            ::kompoz::runtime::kt::safe!(maybe.clone(), |__kz_it1| ::kompoz::runtime::kt::KtStr::length(&__kz_it1)),
            || 0,
        );
        let text = ::kompoz::runtime::kt::elvis(
            ::kompoz::runtime::kt::safe!(
                ::kompoz::runtime::kt::safe!(maybe.clone(), |__kz_it2| ::kompoz::runtime::kt::KtStr::uppercase(
                    &__kz_it2
                )),
                |__kz_it3| ::kompoz::runtime::kt::KtStr::trim(&__kz_it3)
            ),
            || ::std::string::String::from("yok"),
        );
        let forced = ::kompoz::runtime::kt::KtStr::length(&::kompoz::runtime::kt::not_null(other.clone()));
        if let ::core::option::Option::Some(maybe) = maybe.clone()
            && ::kompoz::runtime::kt::KtStr::length(&maybe) > 2
        {
            ::kompoz::runtime::kt::println(maybe.clone());
        }
        if let ::core::option::Option::Some(other) = other.clone() {
            ::kompoz::runtime::kt::println(::kompoz::runtime::kt::KtStr::length(&other));
        } else {
            ::kompoz::runtime::kt::println("null");
        }
        let n: ::core::option::Option<i32> = ::core::option::Option::None;
        let some: ::core::option::Option<::std::string::String> =
            ::core::option::Option::Some(::std::string::String::from("dört"));
        let first = match maybe.clone() {
            ::core::option::Option::Some(__kz_v4) => __kz_v4,
            ::core::option::Option::None => return -1,
        };
        return length
            + forced
            + ::kompoz::runtime::kt::KtStr::length(&text)
            + ::kompoz::runtime::kt::KtStr::length(&first)
            + (::kompoz::runtime::kt::elvis(n, || 0))
            + (::kompoz::runtime::kt::elvis(
                ::kompoz::runtime::kt::safe!(some.clone(), |__kz_it5| ::kompoz::runtime::kt::KtStr::length(&__kz_it5)),
                || 0,
            ));
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn ranges(x: i32) -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let r = ::kompoz::runtime::kt::range_to(1, 10);
        let open = ::kompoz::runtime::kt::until(0, 5);
        let down = ::kompoz::runtime::kt::step(::kompoz::runtime::kt::down_to(10, 1), 3);
        let half = ::kompoz::runtime::kt::until(0, 4);
        return ::kompoz::runtime::kt::is_in(&x, &r)
            && !::kompoz::runtime::kt::is_in(&x, &open)
            && ::kompoz::runtime::kt::is_in(&x, &down)
            || ::kompoz::runtime::kt::is_in(&x, &half);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn qualified() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let red = Color::Red;
        let top = Alignment::TopStart;
        let max = i32::MAX;
        let pi = ::kompoz::runtime::kt::PI;
        let spaced = Arrangement::spaced_by(8.dp());
        let m = Modifier.fill_max_size();
        return ::kompoz::runtime::kt::template!(red, " ", top, " ", max, " ", pi, " ", spaced, " ", m);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn comparisons(
    a: impl ::core::convert::Into<::std::string::String>,
    b: impl ::core::convert::Into<::std::string::String>,
    n: i64,
) -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let a: ::std::string::String = ::core::convert::Into::into(a);
    let b: ::std::string::String = ::core::convert::Into::into(b);
    ::kompoz::runtime::named_args!({
        return a == b || a != "x" || n >= 3 || n < 2_i64 || (n > 1 && a < b);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn bits(x: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ (((x & 0xFF) | (x << 2)) ^ (x >> 1)) })
}
