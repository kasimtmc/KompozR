#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Picker(
    options: ::std::vec::Vec<::std::string::String>,
    supporting: ::std::vec::Vec<::core::option::Option<::std::string::String>>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let expanded = remember(|| ::kompoz::runtime::mutable_state_of!(false));
    let picked = ::kompoz::runtime::kt::delegate_of::<::core::option::Option<i32>, _>(remember(|| {
        ::kompoz::runtime::mutable_state_of!(::core::convert::identity::<::core::option::Option<i32>>(
            ::core::option::Option::None
        ))
    }));
    let mut icons = ::kompoz::runtime::kt::list_of([
        ::kompoz::runtime::kt::list_of([
            ::core::option::Option::Some(Icons::Filled::Edit()),
            ::core::option::Option::Some(Icons::Filled::Home()),
        ]),
        ::kompoz::runtime::kt::list_of([
            ::core::option::Option::None,
            ::core::option::Option::Some(Icons::Filled::Info()),
        ]),
    ]);
    let mut declared: ::std::vec::Vec<::core::option::Option<ImageVector>> =
        ::kompoz::runtime::kt::list_of([::core::option::Option::None, ::core::option::Option::None]);
    ExposedDropdownMenuBox(
        ::kompoz::runtime::kt::KtDelegate::get_value(&expanded),
        |it| {
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(&expanded, it);
        },
        |this| {
            StringTextField(
                modifier =
                    this.menu_anchor(Modifier, ExposedDropdownMenuAnchorType::PrimaryNotEditable).fill_max_width(),
                value = ::kompoz::runtime::kt::KtList::get(&options, 0),
                on_value_change = |_| {},
                read_only = true,
                label = ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Seç");
                    });
                    __kz_f1
                }),
                trailing_icon = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                        let expanded = ::kompoz::runtime::__private::capture(&expanded);
                        move || {
                            ExposedDropdownMenuDefaults
                                .TrailingIcon(::kompoz::runtime::kt::KtDelegate::get_value(&expanded));
                        }
                    });
                    __kz_f2
                }),
                colors = ExposedDropdownMenuDefaults.text_field_colors(),
            );
            this.ExposedDropdownMenu(
                expanded = ::kompoz::runtime::kt::KtDelegate::get_value(&expanded),
                on_dismiss_request = || {
                    ::kompoz::runtime::kt::KtMutableDelegate::set_value(&expanded, false);
                },
                scroll_state = ::core::option::Option::Some(remember_scroll_state()),
                shape = {
                    let __kz_f3: ::std::sync::Arc<dyn Shape> = MenuDefaults::shape();
                    __kz_f3
                },
                content = || {
                    ::kompoz::runtime::kt::KtList::for_each_indexed(&options, |index, option| {
                        SelectableDropdownMenuItem(
                            text = || {
                                Text(option.clone());
                            },
                            selected = ::core::option::Option::Some(index)
                                == ::kompoz::runtime::kt::KtDelegate::get_value(&picked),
                            on_click = || {
                                ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                                    &picked,
                                    ::core::option::Option::Some(index),
                                );
                            },
                            shapes = MenuDefaults::item_shape(index, ::kompoz::runtime::kt::KtList::size(&options)),
                            supporting_text = ::core::option::Option::map(
                                ::kompoz::runtime::kt::KtList::get(&supporting, index),
                                |s| {
                                    let __kz_f4: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                                        let s = ::kompoz::runtime::__private::capture(&s);
                                        move || {
                                            Text(s.clone());
                                        }
                                    });
                                    __kz_f4
                                },
                            ),
                            leading_icon = ::core::option::Option::map(
                                ::kompoz::runtime::kt::KtList::get(
                                    &::kompoz::runtime::kt::KtList::get(&icons, 0),
                                    index,
                                ),
                                |it| {
                                    let __kz_f5: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                                        let it = ::kompoz::runtime::__private::capture(&it);
                                        move || {
                                            VectorIcon(
                                                it.clone(),
                                                content_description = ::core::option::Option::None,
                                                modifier = Modifier.size(MenuDefaults::TrailingIconSize()),
                                            );
                                        }
                                    });
                                    __kz_f5
                                },
                            ),
                            content_padding = ExposedDropdownMenuDefaults::ItemContentPadding(),
                        );
                    });
                    CheckableDropdownMenuItem(
                        ::kompoz::runtime::kt::KtDelegate::get_value(&expanded),
                        |it| {
                            ::kompoz::runtime::kt::KtMutableDelegate::set_value(&expanded, it);
                        },
                        || {
                            Text("İşaretli");
                        },
                        MenuDefaults::item_shape(0, 1),
                    );
                    ShapedDropdownMenuItem(
                        || {},
                        || {
                            Text("Biçimli");
                        },
                        {
                            let __kz_f6: ::std::sync::Arc<dyn Shape> = MenuDefaults::standalone_item_shape();
                            __kz_f6
                        },
                    );
                    DropdownMenuItem(
                        || {
                            Text("Konumla");
                        },
                        on_click = || {
                            ::kompoz::runtime::kt::KtMutableDelegate::set_value(&expanded, false);
                        },
                    );
                },
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Old() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ExposedDropdownMenuBox(
        false,
        |_| {},
        |this| {
            TextField(
                state = remember_text_field_state(),
                modifier = this.menu_anchor(Modifier, ::core::convert::Into::into(MenuAnchorType::PrimaryEditable)),
            );
        },
    );
}
