#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq, Route, :: kompoz :: runtime :: Saveable)]
pub struct Detail {
    pub id: i32,
    # [route (default = :: core :: option :: Option :: None)]
    pub tab: ::core::option::Option<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Detail {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Detail(
    id: i32,
    #[default(::core::option::Option::None)] tab: ::core::option::Option<::std::string::String>,
) -> Detail {
    Detail::__kz_new(id, tab)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Detail {
    #[doc(hidden)]
    pub fn __kz_new(id: i32, tab: ::core::option::Option<::std::string::String>) -> Detail {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Detail(rd)), { Detail { id, tab } })
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn tab(&self) -> ::core::option::Option<::std::string::String> {
        ::core::clone::Clone::clone(&self.tab)
    }
    pub fn component1(&self) -> i32 {
        self.id()
    }
    pub fn component2(&self) -> ::core::option::Option<::std::string::String> {
        self.tab()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Detail {
    pub fn copy(
        &self,
        # [default (self . id)] id: i32,
        # [default (:: core :: clone :: Clone :: clone (& self . tab))] tab: ::core::option::Option<
            ::std::string::String,
        >,
    ) -> Detail {
        Detail::__kz_new(id, tab)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Detail {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Detail(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push_str(", tab=");
        (&::kompoz::runtime::kt::KtProbe(&self.tab)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Detail {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Detail {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzStart {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct Start {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Start {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Start {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzStart {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzStart > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzStart {}, |_| Start {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Start {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Start {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Start",
            Self::__kz_state() as *const __KzStart as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Start {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Start {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Detail(rd)))]
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
            start_destination = Start {},
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
                                nav.navigate_with(Start {}, |this2| {
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
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzShell {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct Shell {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Shell {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Shell {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzShell {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzShell > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzShell {}, |_| Shell {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Shell {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Shell {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Shell",
            Self::__kz_state() as *const __KzShell as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Shell {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Shell {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzAccountGraph {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct AccountGraph {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AccountGraph {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AccountGraph {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzAccountGraph {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzAccountGraph > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzAccountGraph {}, |_| AccountGraph {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl AccountGraph {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for AccountGraph {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "AccountGraph",
            Self::__kz_state() as *const __KzAccountGraph as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for AccountGraph {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for AccountGraph {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq, Route, :: kompoz :: runtime :: Saveable)]
pub struct Member {
    pub n: i32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Member {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Member(n: i32) -> Member {
    Member::__kz_new(n)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Member {
    #[doc(hidden)]
    pub fn __kz_new(n: i32) -> Member {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Member(r)), { Member { n } })
    }
    pub fn n(&self) -> i32 {
        self.n
    }
    pub fn component1(&self) -> i32 {
        self.n()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Member {
    pub fn copy(&self, # [default (self . n)] n: i32) -> Member {
        Member::__kz_new(n)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Member {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Member(");
        __kz_out.push_str("n=");
        (&::kompoz::runtime::kt::KtProbe(&self.n)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Member {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Member {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Member(r)))]
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
                            Shell {},
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
                            AccountGraph {},
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
                NavHost_of(nav.clone(), Shell {}, |this| {
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
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzProfile {
    resource_id: ::kompoz::runtime::kt::KtVal<StringResource>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct Profile {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Profile {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Profile {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzProfile {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzProfile > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(|| __KzProfile { resource_id: ::kompoz::runtime::kt::KtVal::new() }, |_| Profile {}.__kz_init())
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().resource_id.init(Res::string::profile());
        })
    }
    pub fn resource_id(&self) -> StringResource {
        Self::__kz_state().resource_id.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Profile {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Profile {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Profile",
            Self::__kz_state() as *const __KzProfile as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Profile {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Profile {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzScrollable {
    resource_id: ::kompoz::runtime::kt::KtVal<StringResource>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct Scrollable {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Scrollable {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Scrollable {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzScrollable {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzScrollable > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(|| __KzScrollable { resource_id: ::kompoz::runtime::kt::KtVal::new() }, |_| Scrollable {}.__kz_init())
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().resource_id.init(Res::string::scrollable());
        })
    }
    pub fn resource_id(&self) -> StringResource {
        Self::__kz_state().resource_id.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Scrollable {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Scrollable {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Scrollable",
            Self::__kz_state() as *const __KzScrollable as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Scrollable {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Scrollable {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzDialog {
    resource_id: ::kompoz::runtime::kt::KtVal<StringResource>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct Dialog {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Dialog {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Dialog {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzDialog {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzDialog > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(|| __KzDialog { resource_id: ::kompoz::runtime::kt::KtVal::new() }, |_| Dialog {}.__kz_init())
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().resource_id.init(Res::string::dialog());
        })
    }
    pub fn resource_id(&self) -> StringResource {
        Self::__kz_state().resource_id.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Dialog {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Dialog {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Dialog",
            Self::__kz_state() as *const __KzDialog as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Dialog {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Dialog {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq, Route, :: kompoz :: runtime :: Saveable)]
pub struct Dashboard {
    # [route (default = :: core :: option :: Option :: Some (:: std :: string :: String :: from ("no value given")))]
    #[saveable(name = "userId")]
    pub user_id: ::core::option::Option<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Dashboard {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Dashboard(
    #[default(::core::option::Option::Some(::std::string::String::from("no value given")))]
    user_id: ::core::option::Option<::std::string::String>,
) -> Dashboard {
    Dashboard::__kz_new(user_id)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Dashboard {
    #[doc(hidden)]
    pub fn __kz_new(user_id: ::core::option::Option<::std::string::String>) -> Dashboard {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Dashboard(d)), { Dashboard { user_id } })
    }
    pub fn user_id(&self) -> ::core::option::Option<::std::string::String> {
        ::core::clone::Clone::clone(&self.user_id)
    }
    pub fn component1(&self) -> ::core::option::Option<::std::string::String> {
        self.user_id()
    }
    #[allow(non_upper_case_globals)]
    pub const Companion: DashboardCompanion = DashboardCompanion {};
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Dashboard {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . user_id))] user_id: ::core::option::Option<
            ::std::string::String,
        >,
    ) -> Dashboard {
        Dashboard::__kz_new(user_id)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Dashboard {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Dashboard(");
        __kz_out.push_str("userId=");
        (&::kompoz::runtime::kt::KtProbe(&self.user_id)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Dashboard {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Dashboard {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzDashboardCompanion {
    resource_id: ::kompoz::runtime::kt::KtVal<StringResource>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
)]
pub struct DashboardCompanion {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DashboardCompanion {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzDashboardCompanion {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzDashboardCompanion > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(
                || __KzDashboardCompanion { resource_id: ::kompoz::runtime::kt::KtVal::new() },
                |_| DashboardCompanion {}.__kz_init(),
            )
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().resource_id.init(Res::string::dashboard());
        })
    }
    pub fn resource_id(&self) -> StringResource {
        Self::__kz_state().resource_id.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl DashboardCompanion {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for DashboardCompanion {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Dashboard$Companion",
            Self::__kz_state() as *const __KzDashboardCompanion as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for DashboardCompanion {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for DashboardCompanion {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzNested {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
    Route,
    :: kompoz :: runtime :: Saveable,
)]
pub struct Nested {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Nested {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Nested {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzNested {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzNested > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzNested {}, |_| Nested {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Nested {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Nested {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Nested",
            Self::__kz_state() as *const __KzNested as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Nested {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Nested {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq, Route, :: kompoz :: runtime :: Saveable)]
pub struct NestedWithArg {
    # [route (default = :: core :: option :: Option :: Some (:: std :: string :: String :: from ("default nested arg")))]
    #[saveable(name = "userId")]
    pub user_id: ::core::option::Option<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl NestedWithArg {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn NestedWithArg(
    #[default(::core::option::Option::Some(::std::string::String::from("default nested arg")))]
    user_id: ::core::option::Option<::std::string::String>,
) -> NestedWithArg {
    NestedWithArg::__kz_new(user_id)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl NestedWithArg {
    #[doc(hidden)]
    pub fn __kz_new(user_id: ::core::option::Option<::std::string::String>) -> NestedWithArg {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(NestedWithArg(d)), { NestedWithArg { user_id } })
    }
    pub fn user_id(&self) -> ::core::option::Option<::std::string::String> {
        ::core::clone::Clone::clone(&self.user_id)
    }
    pub fn component1(&self) -> ::core::option::Option<::std::string::String> {
        self.user_id()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl NestedWithArg {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . user_id))] user_id: ::core::option::Option<
            ::std::string::String,
        >,
    ) -> NestedWithArg {
        NestedWithArg::__kz_new(user_id)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for NestedWithArg {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("NestedWithArg(");
        __kz_out.push_str("userId=");
        (&::kompoz::runtime::kt::KtProbe(&self.user_id)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for NestedWithArg {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for NestedWithArg {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn BasicNav() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), Profile {}, |this| {
        this.composable_of::<Profile>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                Profile(::core::convert::Into::into(nav_controller.clone()));
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
                Dashboard_nav_controller_string_or_null(nav_controller.clone());
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
                Scrollable(nav_controller.clone());
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
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn NestedNavStartDestination() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), Nested {}, |this| {
        this.navigation_of::<Nested>(Profile {}, |this1| {
            this1.composable_of::<Profile>({
                let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                move |this2, _| {
                    let _ = &this2;
                    Profile(::core::convert::Into::into(nav_controller.clone()));
                }
            });
        });
        this.composable_of::<Dashboard>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                Dashboard_nav_controller_string_or_null(nav_controller.clone());
            }
        });
        this.composable_of::<Scrollable>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                Scrollable(nav_controller.clone());
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
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn NestedNavInGraph() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), Profile {}, |this| {
        this.composable_of::<Profile>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                Profile(::core::convert::Into::into(nav_controller.clone()));
            }
        });
        this.navigation_of::<Dashboard>(Nested {}, |this1| {
            this1.composable_of::<Nested>({
                let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                move |this2, _| {
                    let _ = &this2;
                    Dashboard_nav_controller_string_or_null(nav_controller.clone());
                }
            });
        });
        this.composable_of::<Scrollable>({
            let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
            move |this1, _| {
                let _ = &this1;
                Scrollable(nav_controller.clone());
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
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn NavScaffold() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    Scaffold(|inner_padding| {
        NavHost_of(
            nav_controller.clone(),
            Profile {},
            Modifier.padding_values(inner_padding.clone()),
            builder = |this| {
                this.composable_of::<Profile>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |this1, _| {
                        let _ = &this1;
                        Profile(::core::convert::Into::into(nav_controller.clone()));
                    }
                });
                this.composable_of::<Dashboard>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |this1, _| {
                        let _ = &this1;
                        Dashboard_nav_controller_string_or_null(nav_controller.clone());
                    }
                });
                this.composable_of::<Scrollable>({
                    let nav_controller = ::kompoz::runtime::__private::capture(&nav_controller);
                    move |this1, _| {
                        let _ = &this1;
                        Scrollable(nav_controller.clone());
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
#[::kompoz::runtime::kompozable(types(Dashboard(d), NestedWithArg(d)))]
pub fn NavWithArgsInNestedGraph() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), Profile {}, |this| {
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
                    let user_id = it.to_route::<NestedWithArg>().user_id();
                    Dashboard_nav_controller_string_or_null(nav_controller.clone(), user_id.clone());
                }
            });
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn Profile(nav_controller: NavHostController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            Text(string_resource(Profile {}.resource_id()));
            NavigateButton(string_resource(DashboardCompanion {}.resource_id()), || {
                nav_controller.navigate(Dashboard());
            });
            HorizontalDivider(color = Color::Black);
            NavigateButton(string_resource(Scrollable {}.resource_id()), || {
                nav_controller.navigate(Scrollable {});
            });
            HorizontalDivider(color = Color::Black);
            NavigateButton(string_resource(Dialog {}.resource_id()), || {
                nav_controller.navigate(Dialog {});
            });
            Spacer(Modifier.weight(1_f32));
            NavigateBackButton(nav_controller.clone());
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn ProfileWithArgs(nav_controller: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            Text(string_resource(Profile {}.resource_id()));
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
pub fn Dashboard_nav_controller_string_or_null(
    nav_controller: NavController,
    #[default(::core::option::Option::None)] title: ::core::option::Option<::std::string::String>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            Text(::kompoz::runtime::kt::elvis(title.clone(), || string_resource(DashboardCompanion {}.resource_id())));
            Spacer(Modifier.weight(1_f32));
            NavigateBackButton(nav_controller.clone());
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Dashboard(d)))]
pub fn Scrollable(nav_controller: NavController) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(
        Modifier.fill_max_size().then(Modifier.padding(8.dp())),
        content = || {
            NavigateButton(string_resource(DashboardCompanion {}.resource_id()), || {
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
#[::kompoz::runtime::kompozable(types(User(r), WrappedUUID(r)))]
pub fn ThirdPartySerializableType() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let nav_controller = remember_nav_controller();
    NavHost_of(nav_controller.clone(), User(WrappedUUID(UUID::random_uuid())), |this| {
        this.composable_of::<User>(
            type_map = ::kompoz::runtime::kt::map_of([::kompoz::runtime::kt::to(
                ::kompoz::runtime::kt::type_of::<WrappedUUID>(),
                <NavType as ::core::convert::From<WrappedUUIDNavType>>::from(WrappedUUIDNavType {}),
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
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzWrappedUUIDNavType {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
)]
pub struct WrappedUUIDNavType {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrappedUUIDNavType {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzWrappedUUIDNavType {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzWrappedUUIDNavType > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzWrappedUUIDNavType {}, |_| WrappedUUIDNavType {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl WrappedUUIDNavType {
    pub fn put(&self, bundle: Bundle, key: impl ::core::convert::Into<::std::string::String>, value: WrappedUUID) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let key: ::std::string::String = ::core::convert::Into::into(key);
        let _ = (&bundle, &key, &value);
        ::kompoz::runtime::named_args!({
            bundle.put_string(
                &key.clone(),
                ::core::option::Option::Some(::kompoz::runtime::kt::to_string(&value.uuid())),
            );
        })
    }
    pub fn get(
        &self,
        bundle: Bundle,
        key: impl ::core::convert::Into<::std::string::String>,
    ) -> ::core::option::Option<WrappedUUID> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let key: ::std::string::String = ::core::convert::Into::into(key);
        let _ = (&bundle, &key);
        ::kompoz::runtime::named_args!(types(WrappedUUID(r)), {
            ::core::option::Option::Some(WrappedUUID(UUID::from_string(&::kompoz::runtime::kt::not_null(
                bundle.get_string(&key.clone()),
            ))))
        })
    }
    pub fn parse_value(&self, value: impl ::core::convert::Into<::std::string::String>) -> WrappedUUID {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let value: ::std::string::String = ::core::convert::Into::into(value);
        let _ = (&value,);
        ::kompoz::runtime::named_args!(types(WrappedUUID(r)), { WrappedUUID(UUID::from_string(&value.clone())) })
    }
    pub fn serialize_as_value(&self, value: WrappedUUID) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&value,);
        ::kompoz::runtime::named_args!({
            Uri::encode(&::kompoz::runtime::kt::to_string(&value.uuid()), ::core::option::Option::None)
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for WrappedUUIDNavType {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "WrappedUUIDNavType",
            Self::__kz_state() as *const __KzWrappedUUIDNavType as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for WrappedUUIDNavType {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for WrappedUUIDNavType {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<WrappedUUIDNavType> for NavType {
    fn from(o: WrappedUUIDNavType) -> NavType {
        NavType::new(o)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl NavTypeImpl for WrappedUUIDNavType {
    type Value = WrappedUUID;
    fn is_nullable_allowed(&self) -> bool {
        false
    }
    fn put(&self, bundle: &SavedState, key: &str, value: ::core::option::Option<&WrappedUUID>) {
        WrappedUUIDNavType::put(
            self,
            ::core::clone::Clone::clone(bundle),
            ::std::string::String::from(key),
            ::kompoz::runtime::kt::not_null(::core::option::Option::map(value, ::core::clone::Clone::clone)),
        )
    }
    fn get(&self, bundle: &SavedState, key: &str) -> ::core::option::Option<WrappedUUID> {
        WrappedUUIDNavType::get(self, ::core::clone::Clone::clone(bundle), ::std::string::String::from(key))
    }
    fn parse_value(&self, value: &str) -> ::core::result::Result<::core::option::Option<WrappedUUID>, NavTypeError> {
        ::core::result::Result::Ok(::core::option::Option::Some(WrappedUUIDNavType::parse_value(
            self,
            ::std::string::String::from(value),
        )))
    }
    fn serialize_as_value(&self, value: ::core::option::Option<&WrappedUUID>) -> ::std::string::String {
        WrappedUUIDNavType::serialize_as_value(
            self,
            ::kompoz::runtime::kt::not_null(::core::option::Option::map(value, ::core::clone::Clone::clone)),
        )
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzUser {
    id: ::kompoz::runtime::kt::KtVal<WrappedUUID>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, Route, RouteValue)]
# [route (class (id : WrappedUUID))]
pub struct User {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzUser>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn User(id: WrappedUUID) -> User {
    User::__kz_new(id)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl User {
    #[doc(hidden)]
    pub fn __kz_new(id: WrappedUUID) -> User {
        let __kz_this = User { __kz_this: ::std::rc::Rc::new(__KzUser { id: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(id);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, id: WrappedUUID) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.id.init(::core::clone::Clone::clone(&id));
        ::kompoz::runtime::named_args!({})
    }
    pub fn id(&self) -> WrappedUUID {
        self.__kz_this.id.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl User {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for User {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for User {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for User {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "User",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for User {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for User {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::Saveable for User {
    fn descriptor() -> ::kompoz::runtime::SerialDescriptor {
        ::kompoz::runtime::SerialDescriptor::class(
            ::core::any::type_name::<Self>(),
            ::std::vec![::kompoz::runtime::SerialElement::of::<WrappedUUID>("id", false)],
        )
    }
    fn save(&self) -> ::kompoz::runtime::SavedValue {
        ::kompoz::runtime::SavedValue::map::<&str>([("id", ::kompoz::runtime::Saveable::save(&self.id()))])
    }
    #[allow(unused_variables, clippy::question_mark)]
    fn restore(__kz_saved: &::kompoz::runtime::SavedValue) -> ::core::option::Option<Self> {
        let __kz_map = __kz_saved.as_map()?;
        ::core::option::Option::Some(User(<WrappedUUID as ::kompoz::runtime::Saveable>::restore(__kz_map.get("id")?)?))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl User {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzWrappedUUID {
    uuid: ::kompoz::runtime::kt::KtVal<UUID>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, RouteValue)]
pub struct WrappedUUID {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzWrappedUUID>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn WrappedUUID(uuid: UUID) -> WrappedUUID {
    WrappedUUID::__kz_new(uuid)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrappedUUID {
    #[doc(hidden)]
    pub fn __kz_new(uuid: UUID) -> WrappedUUID {
        let __kz_this = WrappedUUID {
            __kz_this: ::std::rc::Rc::new(__KzWrappedUUID { uuid: ::kompoz::runtime::kt::KtVal::new() }),
        };
        __kz_this.__kz_init(uuid);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, uuid: UUID) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.uuid.init(uuid);
        ::kompoz::runtime::named_args!({})
    }
    pub fn uuid(&self) -> UUID {
        self.__kz_this.uuid.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl WrappedUUID {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for WrappedUUID {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for WrappedUUID {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for WrappedUUID {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for WrappedUUID {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "WrappedUUID",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for WrappedUUID {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for WrappedUUID {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::Saveable for WrappedUUID {
    fn descriptor() -> ::kompoz::runtime::SerialDescriptor {
        ::kompoz::runtime::SerialDescriptor::class(
            ::core::any::type_name::<Self>(),
            ::std::vec![::kompoz::runtime::SerialElement::of::<UUID>("uuid", false)],
        )
    }
    fn save(&self) -> ::kompoz::runtime::SavedValue {
        ::kompoz::runtime::SavedValue::map::<&str>([(
            "uuid",
            ::kompoz::runtime::IntoSaver::into_saver(<KSerializer<_> as ::core::convert::From<UUIDSerializer>>::from(
                UUIDSerializer {},
            ))
            .save(&self.uuid()),
        )])
    }
    #[allow(unused_variables, clippy::question_mark)]
    fn restore(__kz_saved: &::kompoz::runtime::SavedValue) -> ::core::option::Option<Self> {
        let __kz_map = __kz_saved.as_map()?;
        ::core::option::Option::Some(WrappedUUID(
            ::kompoz::runtime::IntoSaver::<UUID>::into_saver(<KSerializer<_> as ::core::convert::From<
                UUIDSerializer,
            >>::from(UUIDSerializer {}))
            .restore(__kz_map.get("uuid")?)?,
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrappedUUID {
    #[allow(dead_code)]
    pub fn serializer() -> ::kompoz::runtime::Saver<Self> {
        ::kompoz::runtime::serializer::<Self>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzUUIDSerializer {
    descriptor: ::kompoz::runtime::kt::KtVal<SerialDescriptor>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
)]
pub struct UUIDSerializer {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl UUIDSerializer {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzUUIDSerializer {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzUUIDSerializer > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(
                || __KzUUIDSerializer { descriptor: ::kompoz::runtime::kt::KtVal::new() },
                |_| UUIDSerializer {}.__kz_init(),
            )
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().descriptor.init(PrimitiveSerialDescriptor("UUID", PrimitiveKind::STRING));
        })
    }
    pub fn descriptor(&self) -> SerialDescriptor {
        Self::__kz_state().descriptor.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl UUIDSerializer {
    pub fn deserialize(&self, decoder: Decoder) -> UUID {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&decoder,);
        ::kompoz::runtime::named_args!({ UUID::from_string(&decoder.decode_string()) })
    }
    pub fn serialize(&self, encoder: Encoder, value: UUID) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&encoder, &value);
        ::kompoz::runtime::named_args!({
            encoder.encode_string(::kompoz::runtime::kt::to_string(&value));
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for UUIDSerializer {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "UUIDSerializer",
            Self::__kz_state() as *const __KzUUIDSerializer as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for UUIDSerializer {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for UUIDSerializer {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<UUIDSerializer> for KSerializer<UUID> {
    fn from(o: UUIDSerializer) -> KSerializer<UUID> {
        KSerializer::new(o)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl KSerializerImpl for UUIDSerializer {
    type Value = UUID;
    fn descriptor(&self) -> SerialDescriptor {
        UUIDSerializer::descriptor(self)
    }
    fn deserialize(&self, decoder: &Decoder) -> UUID {
        UUIDSerializer::deserialize(self, ::core::clone::Clone::clone(decoder))
    }
    fn serialize(&self, encoder: &Encoder, value: &UUID) {
        UUIDSerializer::serialize(self, ::core::clone::Clone::clone(encoder), ::core::clone::Clone::clone(value))
    }
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] static phrases : :: std :: vec :: Vec < :: std :: string :: String > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (:: kompoz :: runtime :: kt :: list_of ([:: std :: string :: String :: from ("Easy As Pie") , :: std :: string :: String :: from ("Wouldn't Harm a Fly") , :: std :: string :: String :: from ("No-Brainer") , :: std :: string :: String :: from ("Keep On Truckin'") , :: std :: string :: String :: from ("An Arm and a Leg") , :: std :: string :: String :: from ("Down To Earth") , :: std :: string :: String :: from ("Under the Weather") , :: std :: string :: String :: from ("Up In Arms") , :: std :: string :: String :: from ("Cup Of Joe") , :: std :: string :: String :: from ("Not the Sharpest Tool in the Shed") , :: std :: string :: String :: from ("Ring Any Bells?") , :: std :: string :: String :: from ("Son of a Gun") , :: std :: string :: String :: from ("Hard Pill to Swallow") , :: std :: string :: String :: from ("Close But No Cigar") , :: std :: string :: String :: from ("Beating a Dead Horse") , :: std :: string :: String :: from ("If You Can't Stand the Heat, Get Out of the Kitchen") , :: std :: string :: String :: from ("Cut To The Chase") , :: std :: string :: String :: from ("Heads Up") , :: std :: string :: String :: from ("Goody Two-Shoes") , :: std :: string :: String :: from ("Fish Out Of Water") , :: std :: string :: String :: from ("Cry Over Spilt Milk") , :: std :: string :: String :: from ("Elephant in the Room") , :: std :: string :: String :: from ("There's No I in Team") , :: std :: string :: String :: from ("Poke Fun At") , :: std :: string :: String :: from ("Talk the Talk") , :: std :: string :: String :: from ("Know the Ropes") , :: std :: string :: String :: from ("Fool's Gold") , :: std :: string :: String :: from ("It's Not Brain Surgery") , :: std :: string :: String :: from ("Fight Fire With Fire") , :: std :: string :: String :: from ("Go For Broke")])) } ; }
