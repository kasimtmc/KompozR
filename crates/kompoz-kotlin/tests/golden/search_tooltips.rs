#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub const tip_text: &str = "Configure permissions. Visit go/permissions for details";
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
fn Results(
    on_result_click: impl ::core::ops::Fn(::std::string::String) + 'static,
    #[default(Modifier)] modifier: Modifier,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        modifier.clone(),
        content = || {
            Text(
                "Suggestion",
                Modifier.clickable({
                    let on_result_click = ::kompoz::runtime::__private::capture(&on_result_click);
                    move || {
                        on_result_click(::std::string::String::from("Suggestion"));
                    }
                }),
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(deprecated)]
#[::kompoz::runtime::kompozable]
pub fn Searching(pinned: bool, on_menu: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let search_bar_state = remember_search_bar_state();
    let text_field_state = remember_text_field_state();
    let scroll_behavior = SearchBarDefaults.enter_always_search_bar_scroll_behavior();
    let field = {
        let text_field_state = ::kompoz::runtime::__private::capture(&text_field_state);
        let search_bar_state = ::kompoz::runtime::__private::capture(&search_bar_state);
        move |tag: ::std::string::String| {
            SearchBarDefaults.InputField(
                text_field_state = text_field_state.clone(),
                search_bar_state = search_bar_state.clone(),
                on_search = |_| {},
                modifier = Modifier.test_tag(tag.clone()),
            )
        }
    };
    Column(
        Modifier.nested_scroll({
            let __kz_f1: ::std::rc::Rc<dyn NestedScrollConnection> = scroll_behavior.nested_scroll_connection();
            __kz_f1
        }),
        content = || {
            AppBarWithSearch(
                state = search_bar_state.clone(),
                input_field = || {
                    field(::std::string::String::from("bar"));
                },
                navigation_icon = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                        let on_menu = ::kompoz::runtime::__private::capture(&on_menu);
                        move || {
                            IconButton(on_menu.clone(), || {
                                VectorIcon(
                                    Icons::Filled::Menu(),
                                    ::core::option::Option::Some(::std::string::String::from("Menu")),
                                );
                            });
                        }
                    });
                    __kz_f2
                }),
                actions = ::core::option::Option::Some({
                    let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                        let search_bar_state = ::kompoz::runtime::__private::capture(&search_bar_state);
                        move || {
                            RowAnimatedVisibility(
                                search_bar_state.current_value() == SearchBarValue::Collapsed,
                                |this| {
                                    let _ = &this;
                                    VectorIcon(
                                        Icons::Filled::Person(),
                                        ::core::option::Option::Some(::std::string::String::from("Account")),
                                    );
                                },
                            );
                        }
                    });
                    __kz_f3
                }),
                scroll_behavior = if pinned {
                    ::core::option::Option::None
                } else {
                    ::core::option::Option::Some(scroll_behavior.clone())
                },
            );
            TopSearchBar(
                search_bar_state.clone(),
                input_field = || {
                    field(::std::string::String::from("top"));
                },
            );
            ColumnAnimatedVisibility(
                visible = search_bar_state.target_value() == SearchBarValue::Collapsed,
                enter = slide_in(
                    animation_spec = {
                        let __kz_f4: ::std::sync::Arc<dyn FiniteAnimationSpec<IntOffset>> =
                            MaterialTheme::motion_scheme().fast_spatial_spec();
                        __kz_f4
                    },
                    initial_offset = |it| IntOffset(-it.width(), 0),
                ),
                content = |this| {
                    let _ = &this;
                    Text(tip_text);
                },
            );
        },
    );
    ExpandedFullScreenSearchBar(
        search_bar_state.clone(),
        || {
            field(::std::string::String::from("view"));
        },
        || {
            ColumnAnimatedVisibility(true, |this| {
                let _ = &this;
                Text("Results");
            });
            Results(
                on_result_click = |result| {
                    text_field_state.set_text_and_place_cursor_at_end(&result.clone());
                },
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn RichTip() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_tooltip_state(is_persistent = true);
    TooltipBox(
        position_provider = {
            let __kz_f5: ::std::rc::Rc<dyn PopupPositionProvider> =
                TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above);
            __kz_f5
        },
        tooltip = |this| {
            this.RichTooltip(
                title = ::core::option::Option::Some({
                    let __kz_f6: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("Title");
                    });
                    __kz_f6
                }),
                action = ::core::option::Option::Some({
                    let __kz_f7: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                        let state = ::kompoz::runtime::__private::capture(&state);
                        move || {
                            TextButton(
                                || {
                                    state.r#dismiss();
                                },
                                || {
                                    Text("OK");
                                },
                            );
                        }
                    });
                    __kz_f7
                }),
                caret_shape = ::core::option::Option::Some({
                    let __kz_f8: ::std::sync::Arc<dyn Shape> = TooltipDefaults::caret_shape();
                    __kz_f8
                }),
                colors = TooltipDefaults.rich_tooltip_colors(container_color = Color::Red),
                text = || {
                    Text(tip_text);
                },
            );
        },
        state = {
            let __kz_f9: ::std::rc::Rc<dyn TooltipState> = state.clone();
            __kz_f9
        },
        has_action = true,
        content = || {
            VectorIcon(Icons::Filled::Info(), ::core::option::Option::Some(::std::string::String::from("Info")));
        },
    );
}
