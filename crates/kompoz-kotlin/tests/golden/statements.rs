#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn States() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let count = remember(|| mutable_state_of(0));
    let doubled = remember(|| {
        derived_state_of({
            let count = ::kompoz::runtime::__private::capture(&count);
            move || ::kompoz::runtime::kt::KtDelegate::get_value(&count) * 2
        })
    });
    let name = remember_saveable(|| mutable_state_of(::std::string::String::from("")));
    let keyed = remember_keyed(
        (::kompoz::runtime::kt::KtDelegate::get_value(&count), ::kompoz::runtime::kt::KtDelegate::get_value(&name)),
        || ::kompoz::runtime::kt::KtDelegate::get_value(&count) + 1,
    );
    ::kompoz::runtime::kt::KtMutableDelegate::set_value(&count, 5);
    ::kompoz::runtime::kt::KtMutableDelegate::set_value(
        &count,
        ::kompoz::runtime::kt::KtDelegate::get_value(&count) + 2,
    );
    ::kompoz::runtime::kt::post_dec(&count);
    ::kompoz::runtime::kt::post_inc(&count);
    ::kompoz::runtime::kt::KtMutableDelegate::set_value(
        &name,
        ::kompoz::runtime::kt::template!(::kompoz::runtime::kt::KtDelegate::get_value(&name), "x"),
    );
    Text(::kompoz::runtime::kt::template!(
        ::kompoz::runtime::kt::KtDelegate::get_value(&count),
        " ",
        ::kompoz::runtime::kt::KtDelegate::get_value(&doubled),
        " ",
        ::kompoz::runtime::kt::KtDelegate::get_value(&name),
        " ",
        keyed
    ));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn counting(n: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut i = 0;
        let mut total = 0;
        while i < n {
            i += 1;
            if i % 2 == 0 {
                continue;
            }
            total += i;
        }
        loop {
            total -= 1;
            if !(total > 100) {
                break;
            }
        }
        'outer: for a in ::kompoz::runtime::kt::until(0, n) {
            for b in ::kompoz::runtime::kt::range_to(0, a) {
                if b == 3 {
                    break 'outer;
                }
                if b == 1 {
                    continue 'outer;
                }
                total += a * b;
            }
        }
        for k in ::kompoz::runtime::kt::step(::kompoz::runtime::kt::down_to(10, 0), 2) {
            total += k;
        }
        let x = {
            i = ::kompoz::runtime::kt::KtInc::inc(i);
            i
        };
        let y = {
            let __kz_v2 = i;
            i = ::kompoz::runtime::kt::KtInc::dec(i);
            __kz_v2
        };
        return total + x + y;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn destructure(pairs: ::std::vec::Vec<(::std::string::String, i32)>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut out = ::std::string::String::from("");
        let __kz_d3 = ::kompoz::runtime::kt::to(::std::string::String::from("a"), 1);
        let first = __kz_d3.component1();
        let second = __kz_d3.component2();
        for __kz_d4 in ::kompoz::runtime::kt::KtIterable::kt_iter(&pairs) {
            let key = __kz_d4.component1();
            let value = __kz_d4.component2();
            out = ::kompoz::runtime::kt::template!(out, ::kompoz::runtime::kt::template!(key, "=", value, ";"));
        }
        return ::kompoz::runtime::kt::template!(out, first, second);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn branches(n: i32) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let word = {
            let __kz_w5 = n;
            if __kz_w5 == 0 {
                ::std::string::String::from("sıfır")
            } else if __kz_w5 == 1 || __kz_w5 == 2 {
                ::std::string::String::from("biraz")
            } else if ::kompoz::runtime::kt::is_in(&__kz_w5, &::kompoz::runtime::kt::range_to(3, 5)) {
                ::std::string::String::from("orta")
            } else if !::kompoz::runtime::kt::is_in(&__kz_w5, &::kompoz::runtime::kt::range_to(6, 9)) {
                ::std::string::String::from("dışarıda")
            } else {
                ::std::string::String::from("bol")
            }
        };
        if n < 0 {
            ::kompoz::runtime::kt::println("eksi");
        } else if n == 0 {
            ::kompoz::runtime::kt::println("sıfır");
        }
        let sign = if n > 0 {
            1
        } else if n < 0 {
            -1
        } else {
            0
        };
        return ::kompoz::runtime::kt::template!(word, " ", sign);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn loopy(xs: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut found = -1;
        ::kompoz::runtime::kt::KtList::for_each(&xs, |it| {
            if it < 0 {
                return;
            }
            found = it;
        });
        let mut i = 0;
        loop {
            '__kz_body6: {
                i += 1;
                if i == 2 {
                    break '__kz_body6;
                }
            }
            if !(i < 5) {
                break;
            }
        }
        return found;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Counts(i: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let counts = remember(|| mutable_state_list_of([0, 0, 0]));
    let mut local = ::kompoz::runtime::kt::mutable_list_of([1.5_f32, 2_f32]);
    {
        let __kz_i7 = i + 1;
        let __kz_v8 = ::kompoz::runtime::kt::KtInc::inc(::kompoz::runtime::kt::KtList::get(&counts, __kz_i7));
        counts.set((__kz_i7) as usize, __kz_v8);
    };
    {
        let __kz_i9 = 0;
        let __kz_v10 = ::kompoz::runtime::kt::KtInc::dec(::kompoz::runtime::kt::KtList::get(&local, __kz_i9));
        ::kompoz::runtime::kt::KtMutableList::set(&mut local, __kz_i9, __kz_v10);
    };
    Text(::kompoz::runtime::kt::template!(
        ::kompoz::runtime::kt::KtList::get(&counts, 1),
        " ",
        ::kompoz::runtime::kt::KtList::get(&local, 0)
    ));
}
