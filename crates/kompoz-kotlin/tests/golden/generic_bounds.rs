#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn reflow_adapt_strategy_sample<
    T: ::core::clone::Clone + 'static + ::core::cmp::PartialEq + NavigatorContentKey,
>() -> ::std::rc::Rc<dyn ThreePaneScaffoldNavigator<T>> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::core::convert::identity::<::std::rc::Rc<dyn ThreePaneScaffoldNavigator<T>>>(
        remember_list_detail_pane_scaffold_navigator(
            adapt_strategies = ListDetailPaneScaffoldDefaults.adapt_strategies(
                extra_pane_adapt_strategy = AdaptStrategy::reflow(ListDetailPaneScaffoldRole::Detail),
            ),
        ),
    )
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(inline)]
pub fn levitate_as_dialog_sample<T: ::core::clone::Clone + 'static + ::core::cmp::PartialEq + NavigatorContentKey>()
-> ::std::rc::Rc<dyn ThreePaneScaffoldNavigator<T>> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let coroutine_scope = remember_coroutine_scope();
    let scaffold_directive = calculate_pane_scaffold_directive(&current_window_adaptive_info());
    let navigator = ::kompoz::runtime::kt::Ref::<
        ::core::option::Option<::std::rc::Rc<dyn ThreePaneScaffoldNavigator<T>>>,
    >::new(::core::option::Option::None);
    let on_click = {
        let coroutine_scope = ::kompoz::runtime::__private::capture(&coroutine_scope);
        let navigator = ::kompoz::runtime::__private::capture(&navigator);
        move || {
            coroutine_scope.launch({
                let navigator = ::kompoz::runtime::__private::capture(&navigator);
                async move {
                    ::kompoz::runtime::kt::flatten!(
                        ::kompoz::runtime::kt::safe_async(
                            ::kompoz::runtime::kt::KtDelegate::get_value(&navigator),
                            async |__kz_it1| __kz_it1.navigate_back().await
                        )
                        .await
                    );
                }
            });
        }
    };
    ::kompoz::runtime::kt::KtMutableDelegate::set_value(
        &navigator,
        ::core::option::Option::Some(::core::convert::identity::<::std::rc::Rc<dyn ThreePaneScaffoldNavigator<T>>>(
            remember_list_detail_pane_scaffold_navigator(
                scaffold_directive = scaffold_directive.clone(),
                adapt_strategies = SupportingPaneScaffoldDefaults.adapt_strategies(
                    extra_pane_adapt_strategy = AdaptStrategy::levitate(
                        Alignment::Center,
                        ::core::option::Option::Some({
                            let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                                let on_click = ::kompoz::runtime::__private::capture(&on_click);
                                move || {
                                    LevitatedPaneScrim(
                                        Modifier.semantics({
                                            let on_click = ::kompoz::runtime::__private::capture(&on_click);
                                            move |this| {
                                                this.set_content_description(::std::string::String::from("Scrim"));
                                                this.on_click(
                                                    ::core::option::Option::Some(::std::string::String::from(
                                                        "Dismiss the extra pane",
                                                    )),
                                                    action = || {
                                                        on_click();
                                                        true
                                                    },
                                                );
                                            }
                                        }),
                                        on_click = {
                                            let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> =
                                                ::std::rc::Rc::new(on_click.clone());
                                            __kz_f2
                                        },
                                    );
                                }
                            });
                            __kz_f3
                        }),
                    )
                    .only_if_single_pane(&scaffold_directive.clone()),
                ),
            ),
        )),
    );
    return ::kompoz::runtime::kt::not_null(::kompoz::runtime::kt::KtDelegate::get_value(&navigator));
}
