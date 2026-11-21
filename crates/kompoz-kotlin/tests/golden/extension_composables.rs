#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
#[doc(hidden)]
pub fn __kz_ext_UnderlineIndicator(__kz_this: TabIndicatorScope, index: i32, #[default(false)] thick: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(__kz_this.tab_indicator_offset(Modifier, index).height(if thick { 6.dp() } else { 2.dp() }));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait TabIndicatorScopeUnderlineIndicatorExt)]
impl TabIndicatorScope {
    #[track_caller]
    fn UnderlineIndicator(&self, index: i32, #[default(false)] thick: bool) {
        let __kz_receiver = ::core::clone::Clone::clone(self);
        ::kompoz::runtime::kompozable_block!({
            __kz_ext_UnderlineIndicator(__kz_receiver, index = index, thick = thick)
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Tabs() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let tab = remember(|| mutable_state_of(0));
    SecondaryTabRow(
        selected_tab_index = ::kompoz::runtime::kt::KtDelegate::get_value(&tab),
        indicator = ::core::option::Option::Some({
            let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn(&TabIndicatorScope)> = ::std::rc::Rc::new({
                let tab = ::kompoz::runtime::__private::capture(&tab);
                move |this: &TabIndicatorScope| {
                    this.UnderlineIndicator(::kompoz::runtime::kt::KtDelegate::get_value(&tab), thick = true);
                }
            });
            __kz_f1
        }),
        tabs = || {
            TextIconTab(
                selected = ::kompoz::runtime::kt::KtDelegate::get_value(&tab) == 0,
                on_click = || {
                    ::kompoz::runtime::kt::KtMutableDelegate::set_value(&tab, 0);
                },
                text = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(move || {
                        Text("bir");
                    });
                    __kz_f2
                }),
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Held() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let held = remember(|| {
        ::core::convert::identity::<MutableState<::core::option::Option<::std::string::String>>>(mutable_state_of(
            ::core::option::Option::None,
        ))
    });
    let shown = ::kompoz::runtime::kt::elvis(::kompoz::runtime::kt::KtDelegate::get_value(&held), || {
        ::std::string::String::from("yok")
    });
    Text(shown.clone());
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Framed() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(Modifier.size(10.dp()).draw_with_content(move |this| {
        this.draw_round_rect(brush = Color::Red, corner_radius = CornerRadius(2_f32));
    }));
}
