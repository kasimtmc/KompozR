#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Bars(shown: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        BottomAppBar(
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("eylem");
                });
            },
            floating_action_button = ::core::option::Option::Some({
                let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                    let shown = ::kompoz::runtime::__private::capture(&shown);
                    move || {
                        ColumnAnimatedVisibility(shown, |this| {
                            let _ = &this;
                            FloatingActionButton(
                                on_click = || {},
                                container_color = BottomAppBarDefaults.bottom_app_bar_fab_color(),
                                content = || {
                                    Text("fab");
                                },
                            );
                        });
                    }
                });
                __kz_f1
            }),
            container_color = BottomAppBarDefaults.container_color(),
            content_padding = BottomAppBarDefaults::ContentPadding(),
        );
        BottomAppBarWithContent(|| {
            RowAnimatedVisibility(shown, |this| {
                let _ = &this;
                Text("içerik");
            });
        });
        FlexibleBottomAppBar(|| {
            RowAnimatedVisibility(shown, |this| {
                let _ = &this;
                Text("esnek");
            });
        });
    });
}
