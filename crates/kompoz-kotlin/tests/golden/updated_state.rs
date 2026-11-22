#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Landing(on_timeout: impl ::core::ops::Fn() + 'static, red: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let current_on_timeout = ::kompoz::runtime::remember_updated_state!(on_timeout.clone());
    let color = ::kompoz::runtime::remember_updated_state!(if red { Color::Red } else { Color::Blue });
    LaunchedEffect(true, async {
        delay(300).await;
        (::kompoz::runtime::kt::KtDelegate::get_value(&current_on_timeout))();
    });
    Text(if color.value() == Color::Red {
        ::std::string::String::from("red")
    } else {
        ::std::string::String::from("blue")
    });
}
