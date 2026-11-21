#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Bars(selected: i32, on_select: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        NavigationBar(|| {
            NavigationBarItem(
                selected = selected == 0,
                on_click = || {
                    on_select(0);
                },
                icon = || {
                    VectorIcon(Icons::Filled::Home(), ::core::option::Option::None);
                },
                label = ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Bir");
                    });
                    __kz_f1
                }),
                colors = NavigationBarItemDefaults.colors(indicator_color = Color::Red),
            );
        });
        ShortNavigationBar(
            arrangement = ShortNavigationBarArrangement::Centered,
            content = || {
                ShortNavigationBarItem(
                    selected = selected == 1,
                    on_click = || {
                        on_select(1);
                    },
                    icon = || {
                        VectorIcon(Icons::Filled::Star(), ::core::option::Option::None);
                    },
                    label = ::core::option::Option::Some({
                        let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                            Text("İki");
                        });
                        __kz_f2
                    }),
                    icon_position = NavigationItemIconPosition::Start,
                    colors = ShortNavigationBarItemDefaults.colors(),
                );
            },
        );
    });
}
