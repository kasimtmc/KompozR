#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn AnimatedVectorSample(xml: impl ::core::convert::Into<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let image = remember_keyed(xml.clone(), || {
        ::kompoz::runtime::kt::KtResult::get_or_throw(AnimatedImageVector::from_animated_vector_xml(&xml.clone()))
    });
    let at_end = remember(|| ::kompoz::runtime::mutable_state_of!(false));
    Image(
        painter = ::std::rc::Rc::new(remember_animated_vector_painter(
            image.clone(),
            ::kompoz::runtime::kt::KtDelegate::get_value(&at_end),
        )),
        content_description = ::core::option::Option::Some(::std::string::String::from("Your content description")),
        modifier = Modifier.size(64.dp()).clickable({
            let at_end = ::kompoz::runtime::__private::capture(&at_end);
            move || {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                    &at_end,
                    !::kompoz::runtime::kt::KtDelegate::get_value(&at_end),
                );
            }
        }),
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn ReadOrNot(
    xml: impl ::core::convert::Into<::std::string::String>,
    seen: impl ::core::ops::Fn(bool, i32) + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let read = AnimatedImageVector::from_animated_vector_xml(&::kompoz::runtime::kt::template!(
        "<animated-vector>",
        xml,
        "</animated-vector>"
    ));
    seen(
        ::kompoz::runtime::kt::KtResult::is_failure(&read),
        ::kompoz::runtime::kt::elvis(
            ::kompoz::runtime::kt::safe!(::kompoz::runtime::kt::KtResult::get_or_null(read.clone()), |__kz_it1| {
                __kz_it1.total_duration()
            }),
            || 0,
        ),
    );
}
