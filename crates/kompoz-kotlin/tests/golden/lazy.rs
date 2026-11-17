#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Todos(group: impl ::core::convert::Into<::core::option::Option<::std::string::String>>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let todos = remember(|| {
        mutable_state_list_of([
            Todo(1, ::std::string::String::from("Yaz"), false),
            Todo(2, ::std::string::String::from("Oku"), false),
        ])
    });
    let state = remember_lazy_list_state();
    LazyColumn(
        state = ::core::option::Option::Some(state.clone()),
        fling_behavior = ::core::option::Option::Some({
            let __kz_f1: ::std::rc::Rc<dyn FlingBehavior> = remember_snap_fling_behavior_for_list(&state.clone());
            __kz_f1
        }),
        overscroll_effect = ::core::option::Option::Some(::core::option::Option::None),
        content = |this| {
            this.sticky_header(
                key = ::core::option::Option::map(group.clone(), ::kompoz::foundation::lazy::layout::LazyKey::new),
                content = |_| {
                    Text("başlık");
                },
            );
            this.item(
                key = ::core::option::Option::Some(::kompoz::foundation::lazy::layout::LazyKey::new(7)),
                content_type = ::core::option::Option::Some(::kompoz::ui::layout::SlotKey::new(
                    ::std::string::String::from("tek"),
                )),
                content = || {
                    Text("bir");
                },
            );
            this.items(
                3,
                key = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn(i32) -> LazyKey> =
                        ::std::rc::Rc::new(move |it: i32| {
                            ::kompoz::foundation::lazy::layout::LazyKey::new(::kompoz::runtime::kt::template!(
                                "satır ", it
                            ))
                        });
                    __kz_f2
                }),
                content_type = ::core::option::Option::Some({
                    let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn(i32) -> ::core::option::Option<SlotKey>> =
                        ::std::rc::Rc::new(move |it: i32| {
                            if it == 0 {
                                ::core::option::Option::None
                            } else {
                                ::core::option::Option::Some(::kompoz::ui::layout::SlotKey::new(
                                    ::std::string::String::from("satır"),
                                ))
                            }
                        });
                    __kz_f3
                }),
                item_content = |i| {
                    Text(::kompoz::runtime::kt::template!(i));
                },
            );
            this.items_list(
                todos.clone(),
                key = ::core::option::Option::Some({
                    let __kz_f4: ::std::rc::Rc<dyn ::core::ops::Fn(&'_ Todo) -> LazyKey> =
                        ::std::rc::Rc::new(move |it: &'_ Todo| {
                            let it = ::core::clone::Clone::clone(it);
                            if it.done() {
                                return ::kompoz::foundation::lazy::layout::LazyKey::new(it.title());
                            }
                            ::kompoz::foundation::lazy::layout::LazyKey::new(it.id())
                        });
                    __kz_f4
                }),
                item_content = |t| {
                    let t = ::core::clone::Clone::clone(t);
                    Text(
                        t.title(),
                        modifier = Modifier
                            .animate_item(
                                placement_spec = ::core::option::Option::Some({
                                    let __kz_f5: ::std::sync::Arc<dyn FiniteAnimationSpec<IntOffset>> =
                                        ::std::sync::Arc::new(tween(duration_millis = 200));
                                    __kz_f5
                                }),
                            )
                            .clickable({
                                let todos = ::kompoz::runtime::__private::capture(&todos);
                                let t = ::kompoz::runtime::__private::capture(&t);
                                move || {
                                    let at =
                                        ::kompoz::runtime::kt::KtList::index_of_first(&todos, |it| it.id() == t.id());
                                    todos.set((at) as usize, Todo(t.id(), t.title(), !t.done()));
                                }
                            }),
                    );
                },
            );
            this.items_indexed(todos.clone(), move |index, t| {
                let t = ::core::clone::Clone::clone(t);
                Text(::kompoz::runtime::kt::template!(index, " ", t.title()));
            });
        },
    );
}
