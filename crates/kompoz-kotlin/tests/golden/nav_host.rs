#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Navigator(nav: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let entry = nav.current_back_stack_entry_as_state();
    let route = ::kompoz::runtime::kt::safe!(
        ::kompoz::runtime::kt::safe!(::kompoz::runtime::kt::KtDelegate::get_value(&entry), |__kz_it1| __kz_it1
            .destination()),
        |__kz_it2| __kz_it2.route()
    );
    let selecting = remember(|| ::kompoz::runtime::mutable_state_of!(false));
    BackHandler(
        enabled = ::kompoz::runtime::kt::KtDelegate::get_value(&selecting),
        on_back = || {
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(&selecting, false);
        },
    );
    Column(|| {
        Text(
            ::kompoz::runtime::kt::template!(
                "rota ",
                ::kompoz::runtime::kt::elvis(route.clone(), || ::std::string::String::from("yok"))
            ),
            modifier = Modifier.combined_clickable(
                on_click = || {},
                on_long_click = ::core::option::Option::Some({
                    let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                        let selecting = ::kompoz::runtime::__private::capture(&selecting);
                        move || {
                            ::kompoz::runtime::kt::KtMutableDelegate::set_value(&selecting, true);
                        }
                    });
                    __kz_f3
                }),
            ),
        );
        NavHost_of(
            nav_controller = nav.clone(),
            start_destination = ::kompoz::runtime::kt::top_level!(Start),
            modifier = Modifier.size(200.dp()),
            enter_transition = |this| {
                let _ = &this;
                slide_in_horizontally(initial_offset_x = |it| it)
            },
            exit_transition = |this| {
                let _ = &this;
                slide_out_horizontally(target_offset_x = |it| -it)
            },
            builder = |this| {
                this.kompozable_of::<Start>({
                    let nav = ::kompoz::runtime::__private::capture(&nav);
                    move |this1, _| {
                        let _ = &this1;
                        Button(
                            || {
                                nav.navigate_with(Detail(id = 7, tab = ::std::string::String::from("b")), |this2| {
                                    this2.set_launch_single_top(true);
                                });
                            },
                            || {
                                Text("Aç");
                            },
                        );
                    }
                });
                this.kompozable_of::<Detail>({
                    let nav = ::kompoz::runtime::__private::capture(&nav);
                    move |this1, back_stack_entry| {
                        let _ = &this1;
                        let detail = back_stack_entry.to_route::<Detail>();
                        Text(::kompoz::runtime::kt::template!(
                            "ayrıntı ",
                            detail.id(),
                            " ",
                            ::kompoz::runtime::kt::elvis(detail.tab(), || ::std::string::String::from("-"))
                        ));
                        Button(
                            || {
                                nav.navigate_with(::kompoz::runtime::kt::top_level!(Start), |this2| {
                                    this2.pop_up_to_of::<Start>(|this| {
                                        let _ = &this;
                                    });
                                    this2.set_launch_single_top(true);
                                });
                            },
                            || {
                                Text("Geri");
                            },
                        );
                    }
                });
                this.navigation("a/{n}", "grup", |this1| {
                    this1.kompozable(
                        "a/{n}",
                        arguments =
                            ::kompoz::runtime::kt::list_of([nav_argument("n", |this2: &mut NavArgumentBuilder| {
                                this2.set_type(NavType::IntType);
                            })]),
                        deep_links =
                            ::kompoz::runtime::kt::list_of([nav_deep_link(|this2: &mut NavDeepLinkDslBuilder| {
                                this2.set_uri_pattern(::core::option::Option::Some(::std::string::String::from(
                                    "kompoz://a/{n}",
                                )));
                            })]),
                        content = |this2, e| {
                            let _ = &this2;
                            Text(::kompoz::runtime::kt::template!(
                                "a ",
                                ::kompoz::runtime::kt::elvis(
                                    ::kompoz::runtime::kt::safe!(e.arguments(), |__kz_it4| __kz_it4.get_int("n")),
                                    || 0
                                )
                            ));
                        },
                    );
                });
            },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavigatorApp(on_controller: impl ::core::ops::Fn(NavHostController) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav = remember_nav_controller();
    on_controller(nav.clone());
    Navigator(nav.clone());
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn AppShellForms(nav: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let entry = nav.current_back_stack_entry_as_state();
    let destination =
        ::kompoz::runtime::kt::safe!(::kompoz::runtime::kt::KtDelegate::get_value(&entry), |__kz_it5| __kz_it5
            .destination());
    let in_account = ::kompoz::runtime::kt::safe!(
        ::kompoz::runtime::kt::safe!(destination.clone(), |__kz_it6| __kz_it6.hierarchy()),
        |__kz_it7| ::kompoz::runtime::kt::KtList::any_that(&__kz_it7, |it| { it.has_route::<AccountGraph>() })
    ) == ::core::option::Option::Some(true);
    let tabs = nav_options(|this: &mut NavOptionsBuilder| {
        this.pop_up_to_of::<Shell>(|this1| {
            this1.set_save_state(true);
        });
        this.set_launch_single_top(true);
        this.set_restore_state(true);
    });
    let snackbars = remember(|| SnackbarHostState());
    LaunchedEffect((), async {
        snackbars
            .show_snackbar(
                ::std::string::String::from("m"),
                action_label = ::core::option::Option::Some(::std::string::String::from("Geri al")),
                duration = ::core::option::Option::Some(SnackbarDuration::Indefinite),
            )
            .await;
    });
    let data = snackbars.current_snackbar_data();
    Column(|| {
        Text(::kompoz::runtime::kt::template!(
            "eylem ",
            ::kompoz::runtime::kt::elvis(
                ::kompoz::runtime::kt::safe!(
                    ::kompoz::runtime::kt::safe!(
                        ::kompoz::runtime::kt::safe!(data.clone(), |__kz_it8| __kz_it8.visuals()),
                        |__kz_it9| __kz_it9.action_label()
                    ),
                    |__kz_it10| ::kompoz::runtime::kt::KtStr::length(&__kz_it10)
                ),
                || 0
            )
        ));
        NavigationSuiteScaffold(
            navigation_items = || {
                NavigationSuiteItem(
                    !in_account,
                    || {
                        nav.navigate_options(
                            ::kompoz::runtime::kt::top_level!(Shell),
                            ::core::option::Option::Some(tabs.clone()),
                            ::core::option::Option::None,
                        );
                    },
                    || {
                        VectorIcon(Icons::Filled::Home(), ::core::option::Option::None);
                    },
                    label = ::core::option::Option::Some({
                        let __kz_f11: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                            Text("Kabuk");
                        });
                        __kz_f11
                    }),
                );
                NavigationSuiteItem(
                    in_account,
                    || {
                        nav.navigate_options(
                            ::kompoz::runtime::kt::top_level!(AccountGraph),
                            ::core::option::Option::Some(tabs.clone()),
                            ::core::option::Option::None,
                        );
                    },
                    || {
                        VectorIcon(Icons::Filled::Settings(), ::core::option::Option::None);
                    },
                    label = ::core::option::Option::Some({
                        let __kz_f12: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                            Text("Hesap");
                        });
                        __kz_f12
                    }),
                );
            },
            navigation_suite_type = NavigationSuiteType::NavigationRail,
            modifier = Modifier.weight(1_f32),
            content = || {
                NavHost_of(nav.clone(), ::kompoz::runtime::kt::top_level!(Shell), |this| {
                    this.kompozable_of::<Shell>({
                        let destination = ::kompoz::runtime::__private::capture(&destination);
                        move |this1, _| {
                            let _ = &this1;
                            Text(
                                if ::kompoz::runtime::kt::safe!(destination.clone(), |__kz_it13| __kz_it13
                                    .has_route::<Shell>())
                                    == ::core::option::Option::Some(true)
                                {
                                    ::std::string::String::from("kabuk ekranı")
                                } else {
                                    ::std::string::String::from("yolda")
                                },
                            );
                        }
                    });
                    this.navigation_of::<AccountGraph>(Member(0), |this1| {
                        this1.kompozable_of::<Member>(
                            deep_links =
                                ::kompoz::runtime::kt::list_of([nav_deep_link_of::<Member>("kompoz://member")]),
                            content = |this2, e| {
                                let _ = &this2;
                                Text(::kompoz::runtime::kt::template!("hesap ", e.to_route::<Member>().n()));
                            },
                        );
                    });
                });
            },
        );
        Text(if in_account {
            ::std::string::String::from("sekme hesap")
        } else {
            ::std::string::String::from("sekme kabuk")
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn AppShellApp(on_controller: impl ::core::ops::Fn(NavHostController) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav = remember_nav_controller();
    on_controller(nav.clone());
    AppShellForms(nav.clone());
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn BasicNav() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), ::kompoz::runtime::kt::top_level!(Profile), |this| {
        this.composable_of::<Profile>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                ProfileScreen(::core::convert::Into::into(nav_controller.clone()));
            }
        });
        this.composable_of::<Dashboard>(
            enter_transition = ::core::option::Option::Some({
                let __kz_f14: ::std::rc::Rc<
                    dyn ::core::ops::Fn(
                            &AnimatedContentTransitionScope<NavBackStackEntry>,
                        ) -> ::core::option::Option<EnterTransition>,
                > = ::std::rc::Rc::new(move |this1: &AnimatedContentTransitionScope<NavBackStackEntry>| {
                    if this1.initial_state().destination().has_route::<Scrollable>() {
                        ::core::option::Option::Some(this1.slide_into_container(SlideDirection::Start))
                    } else {
                        ::core::option::Option::None
                    }
                });
                __kz_f14
            }),
            pop_exit_transition = ::core::option::Option::Some({
                let __kz_f15: ::std::rc::Rc<
                    dyn ::core::ops::Fn(
                            &AnimatedContentTransitionScope<NavBackStackEntry>,
                        ) -> ::core::option::Option<ExitTransition>,
                > = ::std::rc::Rc::new(move |this1: &AnimatedContentTransitionScope<NavBackStackEntry>| {
                    if this1.target_state().destination().has_route::<Scrollable>() {
                        ::core::option::Option::Some(this1.slide_out_of_container(SlideDirection::End))
                    } else {
                        ::core::option::Option::None
                    }
                });
                __kz_f15
            }),
            content = |this1, _| {
                let _ = &this1;
                DashboardScreen(nav_controller.clone());
            },
        );
        this.composable_of::<Scrollable>(
            exit_transition = ::core::option::Option::Some({
                let __kz_f16: ::std::rc::Rc<
                    dyn ::core::ops::Fn(
                            &AnimatedContentTransitionScope<NavBackStackEntry>,
                        ) -> ::core::option::Option<ExitTransition>,
                > = ::std::rc::Rc::new(move |this1: &AnimatedContentTransitionScope<NavBackStackEntry>| {
                    if this1.target_state().destination().has_route::<Dashboard>() {
                        ::core::option::Option::Some(this1.slide_out_of_container(SlideDirection::Start))
                    } else {
                        ::core::option::Option::None
                    }
                });
                __kz_f16
            }),
            pop_enter_transition = ::core::option::Option::Some({
                let __kz_f17: ::std::rc::Rc<
                    dyn ::core::ops::Fn(
                            &AnimatedContentTransitionScope<NavBackStackEntry>,
                        ) -> ::core::option::Option<EnterTransition>,
                > = ::std::rc::Rc::new(move |this1: &AnimatedContentTransitionScope<NavBackStackEntry>| {
                    if this1.initial_state().destination().has_route::<Dashboard>() {
                        ::core::option::Option::Some(this1.slide_into_container(SlideDirection::End))
                    } else {
                        ::core::option::Option::None
                    }
                });
                __kz_f17
            }),
            content = |this1, _| {
                let _ = &this1;
                ScrollableScreen(nav_controller.clone());
            },
        );
        this.dialog_of::<Dialog>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |_| {
                DialogContent(nav_controller.clone());
            }
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NestedNavStartDestination() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), ::kompoz::runtime::kt::top_level!(Nested), |this| {
        this.navigation_of::<Nested>(::kompoz::runtime::kt::top_level!(Profile), |this1| {
            this1.composable_of::<Profile>({
                let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                move |this2, _| {
                    let _ = &this2;
                    ProfileScreen(::core::convert::Into::into(nav_controller.clone()));
                }
            });
        });
        this.composable_of::<Dashboard>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                DashboardScreen(nav_controller.clone());
            }
        });
        this.composable_of::<Scrollable>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                ScrollableScreen(nav_controller.clone());
            }
        });
        this.dialog_of::<Dialog>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |_| {
                DialogContent(nav_controller.clone());
            }
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NestedNavInGraph() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), ::kompoz::runtime::kt::top_level!(Profile), |this| {
        this.composable_of::<Profile>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                ProfileScreen(::core::convert::Into::into(nav_controller.clone()));
            }
        });
        this.navigation_of::<Dashboard>(::kompoz::runtime::kt::top_level!(Nested), |this1| {
            this1.composable_of::<Nested>({
                let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                move |this2, _| {
                    let _ = &this2;
                    DashboardScreen(nav_controller.clone());
                }
            });
        });
        this.composable_of::<Scrollable>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                ScrollableScreen(nav_controller.clone());
            }
        });
        this.dialog_of::<Dialog>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |_| {
                DialogContent(nav_controller.clone());
            }
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavScaffold() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    Scaffold(|inner_padding| {
        NavHost_of(
            nav_controller.clone(),
            ::kompoz::runtime::kt::top_level!(Profile),
            Modifier.padding_values(inner_padding.clone()),
            builder = |this| {
                this.composable_of::<Profile>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |this1, _| {
                        let _ = &this1;
                        ProfileScreen(::core::convert::Into::into(nav_controller.clone()));
                    }
                });
                this.composable_of::<Dashboard>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |this1, _| {
                        let _ = &this1;
                        DashboardScreen(nav_controller.clone());
                    }
                });
                this.composable_of::<Scrollable>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |this1, _| {
                        let _ = &this1;
                        ScrollableScreen(nav_controller.clone());
                    }
                });
                this.dialog_of::<Dialog>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |_| {
                        DialogContent(nav_controller.clone());
                    }
                });
            },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavWithArgsInNestedGraph() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), ::kompoz::runtime::kt::top_level!(Profile), |this| {
        this.composable_of::<Profile>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                ProfileWithArgs(nav_controller.clone());
            }
        });
        this.navigation_of::<Dashboard>(RouteClass::of::<NestedWithArg>(), |this1| {
            this1.composable_of::<NestedWithArg>({
                let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                move |this2, it| {
                    let _ = &this2;
                    let user_id: ::core::option::Option<::std::string::String> =
                        it.to_route::<NestedWithArg>().user_id();
                    DashboardScreen(nav_controller.clone(), user_id.clone());
                }
            });
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn ProfileScreen(nav_controller: NavHostController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            Text(string_resource(Res::string::profile()));
            NavigateButton(string_resource(Res::string::dashboard()), || {
                nav_controller.navigate(Dashboard());
            });
            HorizontalDivider(color = Color::Black);
            NavigateButton(string_resource(Res::string::scrollable()), || {
                nav_controller.navigate(::kompoz::runtime::kt::top_level!(Scrollable));
            });
            HorizontalDivider(color = Color::Black);
            NavigateButton(string_resource(Res::string::dialog()), || {
                nav_controller.navigate(::kompoz::runtime::kt::top_level!(Dialog));
            });
            Spacer(Modifier.weight(1_f32));
            NavigateBackButton(nav_controller.clone());
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn ProfileWithArgs(nav_controller: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            Text(string_resource(Res::string::profile()));
            HorizontalDivider(color = Color::Black);
            let state = remember_saveable(|| ::kompoz::runtime::mutable_state_of!(::std::string::String::from("")));
            Box(content = || {
                StringTextField(
                    value = state.value(),
                    on_value_change = |it| {
                        state.set_value(it.clone());
                    },
                    placeholder = ::core::option::Option::Some({
                        let __kz_f18: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                            Text("Enter userId here");
                        });
                        __kz_f18
                    }),
                );
            });
            HorizontalDivider(color = Color::Black);
            NavigateButton("Dashboard with userId", || {
                nav_controller.navigate(Dashboard(state.value()));
            });
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn DashboardScreen(
    nav_controller: NavController,
    #[default(::core::option::Option::None)] title: ::core::option::Option<::std::string::String>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            Text(::kompoz::runtime::kt::elvis(title.clone(), || string_resource(Res::string::dashboard())));
            Spacer(Modifier.weight(1_f32));
            NavigateBackButton(nav_controller.clone());
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn ScrollableScreen(nav_controller: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            NavigateButton(string_resource(Res::string::dashboard()), || {
                nav_controller.navigate(Dashboard());
            });
            LazyColumn(
                modifier = Modifier.weight(1_f32),
                content = |this| {
                    this.items_list(phrases.with(::core::clone::Clone::clone), move |phrase| {
                        let phrase = ::core::clone::Clone::clone(phrase);
                        Text(phrase.clone(), font_size = 30.sp());
                    });
                },
            );
            NavigateBackButton(nav_controller.clone());
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn DialogContent(nav_controller: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let dialog_width = 300.dp();
    let dialog_height = 300.dp();
    Column(
        Modifier.size_wh(dialog_width, dialog_height).background(Color::White).padding(8.dp()),
        content = || {
            NavigateBackButton(nav_controller.clone());
            LazyColumn(
                modifier = Modifier.weight(1_f32),
                content = |this| {
                    this.items_list(phrases.with(::core::clone::Clone::clone), move |phrase| {
                        let phrase = ::core::clone::Clone::clone(phrase);
                        Text(phrase.clone(), font_size = 16.sp());
                    });
                },
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavigateButton(
    text: impl ::core::convert::Into<::std::string::String>,
    # [default (move | | { })] listener: impl ::core::ops::Fn() + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Button(
        on_click = listener,
        colors = ButtonDefaults.button_colors(container_color = Color::LightGray),
        modifier = Modifier.fill_max_width(),
        content = || {
            Text(::kompoz::runtime::kt::template!("Navigate to ", text));
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavigateBackButton(nav_controller: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    if ::kompoz::runtime::kt::eq_nullable(&nav_controller.current_back_stack_entry(), &LocalLifecycleOwner.current())
        && nav_controller.previous_back_stack_entry().is_some()
    {
        Button(
            on_click = || {
                nav_controller.pop_back_stack();
            },
            colors = ButtonDefaults.button_colors(container_color = Color::LightGray),
            modifier = Modifier.fill_max_width(),
            content = || {
                Text("Go to Previous screen");
            },
        );
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavHostPreview() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    KompozitionLocalProvider(&[LocalInspectionMode.provides(true)], || {
        Box(
            Modifier.fill_max_size().background(Color::Red),
            content = || {
                NavHost(remember_nav_controller(), "home", |this| {
                    this.composable("home", move |this1, _| {
                        let _ = &this1;
                        Box(
                            Modifier.fill_max_size().background(Color::Blue),
                            content = || {
                                Text(text = "test", modifier = Modifier.test_tag("text"));
                            },
                        );
                    });
                });
            },
        )
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn ThirdPartySerializableType() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), User(WrappedUUID(UUID::random_uuid())), |this| {
        this.composable_of::<User>(
            type_map = ::kompoz::runtime::kt::map_of([::kompoz::runtime::kt::to(
                ::kompoz::runtime::kt::type_of::<WrappedUUID>(),
                ::kompoz::runtime::kt::top_level!(WrappedUUIDNavType),
            )]),
            content = |this1, entry| {
                let _ = &this1;
                let uuid = entry.to_route::<User>().id().uuid();
                Box(
                    Modifier.fill_max_size().background(Color::Blue),
                    content = || {
                        Text(
                            text = ::kompoz::runtime::kt::template!("uuid: ", uuid),
                            modifier = Modifier.test_tag("text"),
                        );
                    },
                );
            },
        );
    });
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static phrases : :: std :: vec :: Vec < :: std :: string :: String > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (:: kompoz :: runtime :: kt :: list_of ([:: std :: string :: String :: from ("Easy As Pie") , :: std :: string :: String :: from ("Wouldn't Harm a Fly") , :: std :: string :: String :: from ("No-Brainer") , :: std :: string :: String :: from ("Keep On Truckin'") , :: std :: string :: String :: from ("An Arm and a Leg") , :: std :: string :: String :: from ("Down To Earth") , :: std :: string :: String :: from ("Under the Weather") , :: std :: string :: String :: from ("Up In Arms") , :: std :: string :: String :: from ("Cup Of Joe") , :: std :: string :: String :: from ("Not the Sharpest Tool in the Shed") , :: std :: string :: String :: from ("Ring Any Bells?") , :: std :: string :: String :: from ("Son of a Gun") , :: std :: string :: String :: from ("Hard Pill to Swallow") , :: std :: string :: String :: from ("Close But No Cigar") , :: std :: string :: String :: from ("Beating a Dead Horse") , :: std :: string :: String :: from ("If You Can't Stand the Heat, Get Out of the Kitchen") , :: std :: string :: String :: from ("Cut To The Chase") , :: std :: string :: String :: from ("Heads Up") , :: std :: string :: String :: from ("Goody Two-Shoes") , :: std :: string :: String :: from ("Fish Out Of Water") , :: std :: string :: String :: from ("Cry Over Spilt Milk") , :: std :: string :: String :: from ("Elephant in the Room") , :: std :: string :: String :: from ("There's No I in Team") , :: std :: string :: String :: from ("Poke Fun At") , :: std :: string :: String :: from ("Talk the Talk") , :: std :: string :: String :: from ("Know the Ropes") , :: std :: string :: String :: from ("Fool's Gold") , :: std :: string :: String :: from ("It's Not Brain Surgery") , :: std :: string :: String :: from ("Fight Fire With Fire") , :: std :: string :: String :: from ("Go For Broke")])) } ; }
