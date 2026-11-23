#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Suite(tab: i32, on_tab: impl ::core::ops::Fn(i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    NavigationSuiteScaffoldWithLayoutType(
        navigation_suite_items = |this| {
            this.item(
                selected = tab == 0,
                on_click = || {
                    on_tab(0);
                },
                icon = || {
                    VectorIcon(Icons::Filled::Home(), ::core::option::Option::None);
                },
                label = ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Home");
                    });
                    __kz_f1
                }),
            );
            this.item(
                tab == 1,
                {
                    let on_tab = ::kompoz::runtime::__private::capture(&on_tab);
                    move || {
                        on_tab(1);
                    }
                },
                move || {
                    VectorIcon(Icons::Filled::Settings(), ::core::option::Option::None);
                },
            );
        },
        layout_type = NavigationSuiteType::NavigationBar,
        content = || {
            Text(::kompoz::runtime::kt::template!("Tab ", tab));
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Items(r#type: NavigationSuiteType, on_click: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    NavigationSuite(
        r#type.clone(),
        Modifier.test_tag("suite"),
        vertical_arrangement = Arrangement::Bottom,
        content = || {
            NavigationSuiteItem(
                true,
                on_click.clone(),
                icon = || {
                    VectorIcon(Icons::Filled::Home(), ::core::option::Option::Some(::std::string::String::from("")));
                },
                label = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Ev");
                    });
                    __kz_f2
                }),
                navigation_suite_type = r#type.clone(),
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Bodies(which: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let body = move || Box(Modifier.size_wh(60.dp(), 40.dp()));
    let bar = move || Box(Modifier.size_wh(80.dp(), 80.dp()));
    {
        let __kz_w3 = which;
        if __kz_w3 == 2 {
            NavigationSuiteScaffoldLayout(bar.clone(), NavigationSuiteType::NavigationBar, content = body.clone());
        } else {
            NavigationSuiteScaffoldLayoutWithLayoutType(
                navigation_suite = bar.clone(),
                layout_type = NavigationSuiteType::None,
                content = body.clone(),
            );
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn Suited() -> NavigationSuiteType {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::kt::with(current_window_adaptive_info_v2(), |this: &mut _| {
        if this.window_posture().is_tabletop() || this.window_size_class().min_width_dp() < 600 {
            NavigationSuiteType::ShortNavigationBarMedium
        } else {
            NavigationSuiteScaffoldDefaults::navigation_suite_type(current_window_adaptive_info_v2())
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn OwnItems(
    tab: i32,
    on_tab: impl ::core::ops::Fn(i32) + 'static,
    state: ::std::rc::Rc<dyn NavigationSuiteScaffoldState>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    NavigationSuiteScaffold(
        navigation_items = || {
            NavigationSuiteItem(
                tab == 0,
                || {
                    on_tab(0);
                },
                || {
                    VectorIcon(Icons::Filled::Home(), ::core::option::Option::Some(::std::string::String::from("")));
                },
                label = ::core::option::Option::Some({
                    let __kz_f4: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Ev");
                    });
                    __kz_f4
                }),
            );
            NavigationSuiteItem(
                tab == 1,
                on_click = || {
                    on_tab(1);
                },
                icon = || {
                    VectorIcon(
                        Icons::Filled::Settings(),
                        ::core::option::Option::Some(::std::string::String::from("")),
                    );
                },
                label = ::core::option::Option::Some({
                    let __kz_f5: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Ayarlar");
                    });
                    __kz_f5
                }),
                badge = ::core::option::Option::Some({
                    let __kz_f7: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Badge(
                            content = ::core::option::Option::Some({
                                let __kz_f6: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                                    Text("3");
                                });
                                __kz_f6
                            }),
                        );
                    });
                    __kz_f7
                }),
            );
        },
        state = state.clone(),
        primary_action_content = || {
            Box(Modifier.size(56.dp()));
        },
        content = || {
            Box(Modifier.fill_max_size());
        },
    );
}
