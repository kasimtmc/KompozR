#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NavForms(nav: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let destination = nav.current_destination();
    let in_account = ::kompoz::runtime::kt::safe!(
        ::kompoz::runtime::kt::safe!(destination.clone(), |__kz_it1| __kz_it1.hierarchy()),
        |__kz_it2| ::kompoz::runtime::kt::KtList::any_that(&__kz_it2, |it| { it.has_route::<AccountGraph>() })
    ) == ::core::option::Option::Some(true);
    let tabs = nav_options(|this: &mut NavOptionsBuilder| {
        this.pop_up_to_of::<Shell>(|this1| {
            this1.set_save_state(true);
        });
        this.set_launch_single_top(true);
        this.set_restore_state(true);
    });
    let link = nav_deep_link(|this: &mut NavDeepLinkDslBuilder| {
        this.set_uri_pattern(::core::option::Option::Some(::std::string::String::from("kompoz://a/{n}")));
    });
    let member = nav_deep_link_of::<Member>("kompoz://member");
    let n = ::kompoz::runtime::kt::elvis(
        ::kompoz::runtime::kt::safe!(
            ::kompoz::runtime::kt::safe!(nav.current_back_stack_entry(), |__kz_it3| __kz_it3.arguments()),
            |__kz_it4| __kz_it4.get_int("n")
        ),
        || 0,
    );
    let entry = nav.current_back_stack_entry();
    let detail = if ::kompoz::runtime::kt::safe!(
        ::kompoz::runtime::kt::safe!(entry.clone(), |__kz_it5| __kz_it5.destination()),
        |__kz_it6| __kz_it6.has_route::<Detail>()
    ) == ::core::option::Option::Some(true)
    {
        ::kompoz::runtime::kt::safe!(entry.clone(), |__kz_it7| __kz_it7.to_route::<Detail>())
    } else {
        ::core::option::Option::None
    };
    Column(|| {
        Text(::kompoz::runtime::kt::template!(
            in_account,
            " ",
            n,
            " ",
            ::kompoz::runtime::kt::elvis(::kompoz::runtime::kt::safe!(detail.clone(), |__kz_it8| __kz_it8.id()), || -1),
            " ",
            link.uri_pattern(),
            " ",
            member.uri_pattern()
        ));
        Button(
            || {
                nav.navigate_options(
                    ::kompoz::runtime::kt::top_level!(Shell),
                    ::core::option::Option::Some(tabs.clone()),
                    ::core::option::Option::None,
                );
            },
            || {
                Text("kabuk");
            },
        );
        Button(
            || {
                nav.navigate_with(Detail(id = 7, tab = ::std::string::String::from("b")), |this| {
                    this.pop_up_to_of::<Start>(|this| {
                        let _ = &this;
                    });
                    this.set_launch_single_top(true);
                });
            },
            || {
                Text("ayrıntı");
            },
        );
        Button(
            || {
                nav.pop_back_stack_of::<AccountGraph>(true, false);
            },
            || {
                Text("geri");
            },
        );
        Button(
            || {
                nav.navigate_with(::std::string::String::from("a/3"), |this| {
                    this.pop_up_to("grup", |this1| {
                        this1.set_inclusive(true);
                    });
                });
            },
            || {
                Text("a");
            },
        );
    });
}
