#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Chips(selected: bool, wide: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let tasks = remember(|| mutable_state_list_of([1, 2, 3]));
    Column(|| {
        FilterChip(
            selected = selected,
            on_click = || {
                tasks.shuffle(&::kompoz::runtime::kt::Random(7));
            },
            label = || {
                Text("Filtre");
            },
            leading_icon = if selected {
                ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        VectorIcon(Icons::Filled::Done(), ::core::option::Option::None);
                    });
                    __kz_f1
                })
            } else {
                ::core::option::Option::None
            },
        );
        FilterChip(
            selected = selected,
            on_click = || {},
            label = || {
                Text("İkisi");
            },
            leading_icon = if selected {
                ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        VectorIcon(Icons::Filled::Done(), ::core::option::Option::None);
                    });
                    __kz_f2
                })
            } else {
                if wide {
                    ::core::option::Option::Some({
                        let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                            VectorIcon(Icons::Filled::Home(), ::core::option::Option::None);
                        });
                        __kz_f3
                    })
                } else {
                    ::core::option::Option::None
                }
            },
        );
        InputChip(
            selected,
            || {},
            label = || {
                Text("Giriş");
            },
        );
        IconButton(
            on_click = || {},
            modifier =
                Modifier.size_dp(IconButtonDefaults.medium_container_size(width_option = IconButtonWidthOption::Wide)),
            content = || {
                Icon(
                    ::std::rc::Rc::new(remember_vector_painter(Icons::Filled::Lock())),
                    content_description = ::core::option::Option::None,
                    tint = Color::Red,
                );
            },
        );
    });
}
