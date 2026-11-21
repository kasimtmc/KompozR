#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Fields(error: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_text_field_state();
    let text = remember(|| mutable_state_of(::std::string::String::from("")));
    let value = remember(|| mutable_state_of(TextFieldValue(::std::string::String::from("ab"))));
    let keyboard_controller = LocalSoftwareKeyboardController.current();
    Column(|| {
        TextField(
            state = state.clone(),
            label = ::core::option::Option::Some({
                let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn(&dyn TextFieldLabelScope)> =
                    ::std::rc::Rc::new(move |this: &dyn TextFieldLabelScope| {
                        let _ = &this;
                        Text("Ad");
                    });
                __kz_f1
            }),
            label_position = TextFieldLabelPosition::above(Alignment::Start),
            prefix = ::core::option::Option::Some({
                let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                    Text("₺");
                });
                __kz_f2
            }),
            supporting_text = if error {
                ::core::option::Option::Some({
                    let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Hata");
                    });
                    __kz_f3
                })
            } else {
                ::core::option::Option::None
            },
            shape = {
                let __kz_f4: ::std::sync::Arc<dyn Shape> = TextFieldDefaults::shape();
                __kz_f4
            },
            input_transformation = ::core::option::Option::Some(InputTransformation.max_length((4 as usize)).then(
                input_transformation(move |this: &mut TextFieldBuffer| {
                    if !::kompoz::runtime::kt::KtStr::is_digits_only(&this.as_char_sequence()) {
                        this.revert_all_changes();
                    }
                }),
            )),
            on_keyboard_action = ::core::option::Option::Some({
                let __kz_f6: ::std::rc::Rc<dyn KeyboardActionHandler> = ::std::rc::Rc::new({
                    let keyboard_controller = ::kompoz::runtime::__private::capture(&keyboard_controller);
                    move |_: &dyn ::core::ops::Fn()| {
                        ::kompoz::runtime::kt::safe!(
                            ::core::option::Option::Some(keyboard_controller.clone()),
                            |__kz_it5| __kz_it5.hide()
                        );
                    }
                });
                __kz_f6
            }),
        );
        StringTextField(
            value = ::kompoz::runtime::kt::KtDelegate::get_value(&text),
            on_value_change = |it| {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(&text, it.clone());
            },
            trailing_icon = ::core::option::Option::Some({
                let __kz_f7: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                    let text = ::kompoz::runtime::__private::capture(&text);
                    move || {
                        IconButton(
                            || {
                                ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                                    &text,
                                    ::std::string::String::from(""),
                                );
                            },
                            || {
                                VectorIcon(
                                    Icons::Filled::Clear(),
                                    ::core::option::Option::Some(::std::string::String::from("Sil")),
                                );
                            },
                        );
                    }
                });
                __kz_f7
            }),
        );
        ValueOutlinedTextField(
            value = ::kompoz::runtime::kt::KtDelegate::get_value(&value),
            on_value_change = |it| {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(&value, it.clone());
            },
            colors = OutlinedTextFieldDefaults.colors(unfocused_border_color = Color::Red),
        );
        BasicTextFieldString(
            value = ::kompoz::runtime::kt::KtDelegate::get_value(&text),
            on_value_change = |it| {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(&text, it.clone());
            },
            decoration_box = ::core::option::Option::Some({
                let __kz_f9: ::std::rc::Rc<dyn ::core::ops::Fn(&dyn ::core::ops::Fn())> = ::std::rc::Rc::new({
                    let text = ::kompoz::runtime::__private::capture(&text);
                    move |inner: &dyn ::core::ops::Fn()| {
                        TextFieldDefaults.DecorationBox(
                            value = &::kompoz::runtime::kt::KtDelegate::get_value(&text),
                            inner_text_field = &inner,
                            enabled = true,
                            single_line = true,
                            visual_transformation = VisualTransformation::None,
                            interaction_source =
                                ::core::convert::Into::into(remember(|| MutableInteractionSource::new())),
                            container = ::core::option::Option::Some({
                                let __kz_f8: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                                    TextFieldDefaults.Container(
                                        enabled = true,
                                        is_error = false,
                                        interaction_source =
                                            ::core::convert::Into::into(remember(|| MutableInteractionSource::new())),
                                        colors = TextFieldDefaults.colors(),
                                    );
                                });
                                __kz_f8
                            }),
                        );
                    }
                });
                __kz_f9
            }),
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub (self) trait TextFieldStateAddCommaExt)]
impl TextFieldState {
    fn add_comma(self, at: i32, text: impl ::core::convert::Into<::std::string::String>) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let text: ::std::string::String = ::core::convert::Into::into(text);
        ::kompoz::runtime::named_args!({
            self.edit(|this1| {
                if at == (this1.length() as i32) || this1.char_at((at as usize)) != ',' {
                    this1.insert((at as usize), ", ");
                }
                this1.replace((0 as usize), (0 as usize), text.clone());
            });
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
fn first_digit(text: impl ::core::convert::Into<::std::string::String>) -> ::core::option::Option<char> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let text: ::std::string::String = ::core::convert::Into::into(text);
    ::kompoz::runtime::named_args!({
        for c in ::core::primitive::str::chars(&text).collect::<::std::vec::Vec<char>>() {
            if ::kompoz::runtime::kt::KtChar::is_digit(c) {
                return ::core::option::Option::Some(c);
            }
        }
        return ::core::option::Option::None;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub (self) trait ListLongestExt)]
impl ::std::vec::Vec<::std::string::String> {
    fn longest(self) -> ::core::option::Option<::std::string::String> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            if ::kompoz::runtime::kt::KtList::is_empty(&self) {
                ::core::option::Option::None
            } else {
                ::core::option::Option::Some(::kompoz::runtime::kt::KtList::max_by(&self, |it| {
                    ::kompoz::runtime::kt::KtStr::length(&it)
                }))
            }
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
fn cursor_of(state: TextFieldState) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ (state.selection().start() as i32) - 1 })
}
