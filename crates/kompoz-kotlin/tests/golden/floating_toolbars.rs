#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Toolbars(shown: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(content = || {
        HorizontalFloatingToolbar(
            expanded = true,
            leading_content = ::core::option::Option::Some({
                let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                    let shown = ::kompoz::runtime::__private::capture(&shown);
                    move || {
                        RowAnimatedVisibility(shown, |this| {
                            let _ = &this;
                            Text("baş");
                        });
                    }
                });
                __kz_f1
            }),
            trailing_content = ::core::option::Option::Some({
                let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                    let shown = ::kompoz::runtime::__private::capture(&shown);
                    move || {
                        RowAnimatedVisibility(shown, |this| {
                            let _ = &this;
                            Text("son");
                        });
                    }
                });
                __kz_f2
            }),
            content = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("orta");
                });
            },
        );
        VerticalFloatingToolbarWithFab(
            true,
            || {
                AnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("fab");
                });
            },
            || {
                ColumnAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("sütun");
                });
            },
        );
    });
}
