#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Groups(source: MutableInteractionSource, bold: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let last = remember(|| mutable_state_of(::std::string::String::from("")));
    ButtonGroupWithoutOverflowIndicator(|this| {
        Button(
            on_click = || {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(&last, ::std::string::String::from("oynat"));
            },
            modifier = this.animate_width(Modifier, source.clone()).weight(1_f32),
            interaction_source = ::core::option::Option::Some(source.clone()),
            content = || {
                Text("Oynat");
            },
        );
    });
    ButtonGroup(
        overflow_indicator = |menu_state| {
            let __kz_menu_state = ::core::clone::Clone::clone(menu_state);
            IconButton(
                || {
                    if __kz_menu_state.is_showing() {
                        __kz_menu_state.dismiss();
                    } else {
                        __kz_menu_state.show();
                    }
                },
                || {
                    VectorIcon(
                        Icons::Filled::MoreVert(),
                        ::core::option::Option::Some(::std::string::String::from("Daha")),
                    );
                },
            );
        },
        vertical_alignment = Alignment::CenterVertically,
        content = |this| {
            let __kz_this = ::core::clone::Clone::clone(this);
            this.clickable_item(
                {
                    let last = ::kompoz::runtime::__private::capture(&last);
                    move || {
                        ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                            &last,
                            ::std::string::String::from("kopyala"),
                        );
                    }
                },
                "Kopyala",
            );
            this.toggleable_item(checked = bold, on_checked_change = |_| {}, label = "Kalın", weight = 2_f32);
            this.custom_item(
                {
                    let __kz_this = ::kompoz::runtime::__private::capture(&__kz_this);
                    move || {
                        let held = remember(|| MutableInteractionSource::new());
                        Button(
                            on_click = || {},
                            modifier = __kz_this.animate_width(Modifier, held.clone()).align(Alignment::Bottom),
                            interaction_source = ::core::option::Option::Some(held.clone()),
                            content = || {
                                Text("Özel");
                            },
                        );
                    }
                },
                move |state| {
                    let __kz_state = ::core::clone::Clone::clone(state);
                    DropdownMenuItem(
                        || {
                            Text("Özel");
                        },
                        on_click = || {
                            __kz_state.dismiss();
                        },
                    );
                },
            );
        },
    );
    ButtonGroup(
        |it| {
            ButtonGroupDefaults.OverflowIndicator(&it.clone());
        },
        |this| {
            this.clickable_item(
                on_click = || {},
                label = "Bir",
                icon = ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        VectorIcon(Icons::Filled::Add(), ::core::option::Option::None);
                    });
                    __kz_f1
                }),
            );
        },
    );
    Text(
        ::kompoz::runtime::kt::KtDelegate::get_value(&last),
        font_weight = if bold { ::core::option::Option::Some(FontWeight::Bold) } else { ::core::option::Option::None },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Casts() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let shape = ButtonGroupDefaults.connected_middle_button_shapes().shape();
    let top = (::kompoz::runtime::kt::KtAs::<RoundedCornerShape>::kt_as(&shape))
        .copy(top_start = CornerSize(100), top_end = CornerSize(100));
    let maybe = ::kompoz::runtime::kt::KtAs::<CutCornerShape>::kt_as_safe(&shape);
    Box(Modifier.size(10.dp()).background(
        Color::Red,
        shape = {
            let __kz_f3: ::std::sync::Arc<dyn Shape> = {
                let __kz_shared2: ::std::sync::Arc<dyn Shape> =
                    if maybe.is_some() { shape.clone() } else { ::std::sync::Arc::new(top.clone()) };
                __kz_shared2
            };
            __kz_f3
        },
    ));
}
