#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Counter(#[default(0)] start: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let count = remember(|| mutable_state_of(start));
    Column(|| {
        Text(::kompoz::runtime::kt::template!("Count: ", ::kompoz::runtime::kt::KtDelegate::get_value(&count)));
        Button(
            || {
                ::kompoz::runtime::kt::post_inc(&count);
            },
            || {
                Text("+");
            },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Button(
    on_click: impl ::core::ops::Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    content: impl ::core::ops::Fn() + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(
        modifier.clone().clickable(on_click.clone()).test_tag("button"),
        content = || {
            content();
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub(crate) fn Labeled(
    label: impl ::core::convert::Into<::std::string::String>,
    #[default(7)] value: i32,
    #[default(::std::string::String::from(""))] suffix: ::std::string::String,
    content: impl ::core::ops::Fn() + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Row(|| {
        Text(::kompoz::runtime::kt::template!(label, "=", value, suffix));
        content();
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
fn NamedArguments() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        Labeled(
            suffix = ::std::string::String::from("!"),
            label = "a",
            content = || {
                Text("içerik");
            },
        );
        Labeled("b", 3, content = || {});
        Labeled("c", || {});
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn textSize() -> TextUnit {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    18.sp()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn Twice(content: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    content();
    content();
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Maybe(
    title: impl ::core::convert::Into<::core::option::Option<::std::string::String>>,
    #[default(10_i64)] count: i64,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    if let ::core::option::Option::Some(title) = title.clone() {
        Text(title.clone());
    }
    Text(::kompoz::runtime::kt::template!(count));
}
