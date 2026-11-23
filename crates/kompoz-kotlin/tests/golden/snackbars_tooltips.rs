#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Snackbars() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let snackbar_host_state = remember(|| SnackbarHostState());
    let tooltip_state = remember_tooltip_state();
    let scope = remember_coroutine_scope();
    let count = remember(|| ::kompoz::runtime::mutable_state_of!(0));
    LaunchedEffect((), async {
        snapshot_flow({
            let count = ::kompoz::runtime::__private::capture(&count);
            move || ::kompoz::runtime::kt::KtDelegate::get_value(&count)
        })
        .collect_async({
            let snackbar_host_state = ::kompoz::runtime::__private::capture(&snackbar_host_state);
            let count = ::kompoz::runtime::__private::capture(&count);
            move |n| {
                let snackbar_host_state = ::kompoz::runtime::__private::capture(&snackbar_host_state);
                let count = ::kompoz::runtime::__private::capture(&count);
                async move {
                    if snackbar_host_state
                        .show_snackbar(
                            ::kompoz::runtime::kt::template!("Snackbar # ", n),
                            action_label = ::core::option::Option::Some(::std::string::String::from("Undo")),
                        )
                        .await
                        == SnackbarResult::ActionPerformed
                    {
                        ::kompoz::runtime::kt::KtMutableDelegate::set_value(&count, 0);
                    }
                }
            }
        })
        .await;
    });
    Column(|| {
        TooltipBox(
            {
                let __kz_f1: ::std::rc::Rc<dyn PopupPositionProvider> =
                    TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above);
                __kz_f1
            },
            |this| {
                this.PlainTooltip(
                    caret_shape = ::core::option::Option::Some({
                        let __kz_f2: ::std::sync::Arc<dyn Shape> = TooltipDefaults::caret_shape();
                        __kz_f2
                    }),
                    content = || {
                        Text("Add");
                    },
                );
            },
            {
                let __kz_f3: ::std::rc::Rc<dyn TooltipState> = tooltip_state.clone();
                __kz_f3
            },
            || {
                IconButton(
                    || {
                        ::kompoz::runtime::kt::post_inc(&count);
                    },
                    || {
                        VectorIcon(
                            Icons::Filled::Add(),
                            ::core::option::Option::Some(::std::string::String::from("Add")),
                        );
                    },
                );
            },
        );
        Button(
            || {
                scope.launch({
                    let tooltip_state = ::kompoz::runtime::__private::capture(&tooltip_state);
                    async move {
                        tooltip_state.show(MutatePriority::Default).await;
                    }
                });
            },
            || {
                Text("Show");
            },
        );
        Button(
            || {
                scope.launch({
                    let snackbar_host_state = ::kompoz::runtime::__private::capture(&snackbar_host_state);
                    async move {
                        snackbar_host_state.show_snackbar(::std::string::String::from("Shown")).await;
                    }
                });
            },
            || {
                Text("Snackbar");
            },
        );
        SnackbarHost(snackbar_host_state.clone());
    });
}
