#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn SnapshotStateViewModel() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({})
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub(crate) struct SnapshotStateViewModelItem {
    pub id: UUID,
    pub value: ::std::string::String,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn SnapshotStateViewModelItem(
    id: UUID,
    value: impl ::core::convert::Into<::std::string::String>,
) -> SnapshotStateViewModelItem {
    let value: ::std::string::String = ::core::convert::Into::into(value);
    SnapshotStateViewModelItem::__kz_new(id, value)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl SnapshotStateViewModelItem {
    #[doc(hidden)]
    pub(crate) fn __kz_new(id: UUID, value: ::std::string::String) -> SnapshotStateViewModelItem {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(SnapshotStateViewModelItem(rr)), {
            SnapshotStateViewModelItem { id, value }
        })
    }
    pub fn id(&self) -> UUID {
        self.id
    }
    pub fn value(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.value)
    }
    pub fn component1(&self) -> UUID {
        self.id()
    }
    pub fn component2(&self) -> ::std::string::String {
        self.value()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl SnapshotStateViewModelItem {
    pub(crate) fn copy(
        &self,
        # [default (self . id)] id: UUID,
        # [default (:: core :: clone :: Clone :: clone (& self . value))] value: ::std::string::String,
    ) -> SnapshotStateViewModelItem {
        SnapshotStateViewModelItem::__kz_new(id, value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for SnapshotStateViewModelItem {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Item(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push_str(", value=");
        (&::kompoz::runtime::kt::KtProbe(&self.value)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for SnapshotStateViewModelItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for SnapshotStateViewModelItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzSnapshotStateViewModelSnapshotStateViewModel {
    #[doc(hidden)]
    pub __kz_super: ::kompoz::runtime::kt::KtVal<ViewModelImpl>,
    items: ::kompoz::runtime::kt::KtVal<SnapshotStateList<SnapshotStateViewModelItem>>,
    selected_item_ids: ::kompoz::runtime::kt::KtVal<SnapshotStateMap<UUID, ()>>,
    are_selections_enabled:
        ::kompoz::runtime::kt::KtVal<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtMutableDelegate<Value = bool>>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct SnapshotStateViewModelSnapshotStateViewModel {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzSnapshotStateViewModelSnapshotStateViewModel>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn SnapshotStateViewModelSnapshotStateViewModel(
    handle: SavedStateHandle,
) -> SnapshotStateViewModelSnapshotStateViewModel {
    SnapshotStateViewModelSnapshotStateViewModel::__kz_new(handle)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl SnapshotStateViewModelSnapshotStateViewModel {
    #[doc(hidden)]
    pub(crate) fn __kz_new(handle: SavedStateHandle) -> SnapshotStateViewModelSnapshotStateViewModel {
        let __kz_this = SnapshotStateViewModelSnapshotStateViewModel {
            __kz_this: ::std::rc::Rc::new(__KzSnapshotStateViewModelSnapshotStateViewModel {
                __kz_super: ::kompoz::runtime::kt::KtVal::new(),
                items: ::kompoz::runtime::kt::KtVal::new(),
                selected_item_ids: ::kompoz::runtime::kt::KtVal::new(),
                are_selections_enabled: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(handle);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, handle: SavedStateHandle) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.__kz_super.init(ViewModelImpl::new());
        ::kompoz::runtime::named_args!(types(SnapshotStateViewModelItem(rr)), {
            self.__kz_this.items.init(handle.saveable_with(
                "items",
                list_saver(
                    move |it: &SnapshotStateList<SnapshotStateViewModelItem>| {
                        let it = ::core::clone::Clone::clone(it);
                        ::kompoz::runtime::kt::KtList::map(&it, |item| {
                            ::kompoz::runtime::kt::list_of([::kompoz::runtime::kt::to_string(&item.id()), item.value()])
                        })
                    },
                    move |it| {
                        ::core::option::Option::Some(::kompoz::runtime::kt::KtList::to_mutable_state_list(
                            &::kompoz::runtime::kt::KtList::map(&it, |saved| {
                                SnapshotStateViewModelItem(
                                    UUID::from_string(&::kompoz::runtime::kt::KtList::get(&saved, 0)),
                                    ::kompoz::runtime::kt::KtList::get(&saved, 1),
                                )
                            }),
                        ))
                    },
                ),
                || mutable_state_list_of([]),
            ));
            self.__kz_this.selected_item_ids.init(handle.saveable_with(
                "selectedItemIds",
                list_saver(
                    move |it: &SnapshotStateMap<UUID, ()>| {
                        let it = ::core::clone::Clone::clone(it);
                        ::kompoz::runtime::kt::KtList::map(
                            &::kompoz::runtime::kt::KtMapExt::keys(&it),
                            move |__kz_r: UUID| ::kompoz::runtime::kt::to_string(&__kz_r),
                        )
                    },
                    move |it| {
                        ::core::option::Option::Some(::kompoz::runtime::kt::KtList::to_mutable_state_map(
                            &::kompoz::runtime::kt::KtList::map(
                                &::kompoz::runtime::kt::KtList::map(&it, move |__kz_p0| {
                                    UUID::from_string(&__kz_p0.clone())
                                }),
                                |id| ::kompoz::runtime::kt::to(id.clone(), ()),
                            ),
                        ))
                    },
                ),
                || mutable_state_map_of([]),
            ));
            self.__kz_this.are_selections_enabled.init(::std::rc::Rc::new(
                ::kompoz::runtime::kt::delegate_of::<bool, _>(
                    handle.saveable("areSelectionsEnabled", || ::kompoz::runtime::mutable_state_of!(true)),
                ),
            ));
        })
    }
    fn items(&self) -> SnapshotStateList<SnapshotStateViewModelItem> {
        self.__kz_this.items.get()
    }
    fn selected_item_ids(&self) -> SnapshotStateMap<UUID, ()> {
        self.__kz_this.selected_item_ids.get()
    }
    pub fn are_selections_enabled(&self) -> bool {
        self.__kz_this.are_selections_enabled.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
    }
    pub fn set_are_selections_enabled(&self, value: bool) {
        self.__kz_this
            .are_selections_enabled
            .with(|__kz_d| ::kompoz::runtime::kt::KtMutableDelegate::set_value(__kz_d, value))
    }
    pub fn selected_items(&self) -> ::std::vec::Vec<(SnapshotStateViewModelItem, bool)> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::KtList::map(&self.items(), |it| {
                ::kompoz::runtime::kt::to(
                    it.clone(),
                    (::kompoz::runtime::kt::is_in(&it.id(), &self.selected_item_ids())),
                )
            })
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl SnapshotStateViewModelSnapshotStateViewModel {
    pub fn select_item(&self, id: UUID, selected: bool) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            if selected {
                self.selected_item_ids().put(id, ());
            } else {
                self.selected_item_ids().remove(&id);
            }
        })
    }
    pub fn add_item(&self, value: impl ::core::convert::Into<::std::string::String>) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let value: ::std::string::String = ::core::convert::Into::into(value);
        ::kompoz::runtime::named_args!(types(SnapshotStateViewModelItem(rr)), {
            self.items().add(SnapshotStateViewModelItem(UUID::random_uuid(), value.clone()));
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for SnapshotStateViewModelSnapshotStateViewModel {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for SnapshotStateViewModelSnapshotStateViewModel {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for SnapshotStateViewModelSnapshotStateViewModel {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for SnapshotStateViewModelSnapshotStateViewModel {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "SnapshotStateViewModel",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for SnapshotStateViewModelSnapshotStateViewModel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for SnapshotStateViewModelSnapshotStateViewModel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ViewModel for SnapshotStateViewModelSnapshotStateViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        (&self.__kz_this.__kz_super).get_ref()
    }
    fn new_instance(extras: &CreationExtras) -> ::core::option::Option<Self> {
        let _ = extras;
        ::core::option::Option::Some(SnapshotStateViewModelSnapshotStateViewModel::__kz_new(
            extras.create_saved_state_handle(),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ViewModelRef for SnapshotStateViewModelSnapshotStateViewModel {
    type Model = SnapshotStateViewModelSnapshotStateViewModel;
    fn from_rc(
        model: ::std::rc::Rc<SnapshotStateViewModelSnapshotStateViewModel>,
    ) -> SnapshotStateViewModelSnapshotStateViewModel {
        ::core::clone::Clone::clone(&*model)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn SnapshotStateViewModelWithDelegates() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({})
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub(crate) struct SnapshotStateViewModelWithDelegatesItem {
    pub id: UUID,
    pub value: ::std::string::String,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn SnapshotStateViewModelWithDelegatesItem(
    id: UUID,
    value: impl ::core::convert::Into<::std::string::String>,
) -> SnapshotStateViewModelWithDelegatesItem {
    let value: ::std::string::String = ::core::convert::Into::into(value);
    SnapshotStateViewModelWithDelegatesItem::__kz_new(id, value)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl SnapshotStateViewModelWithDelegatesItem {
    #[doc(hidden)]
    pub(crate) fn __kz_new(id: UUID, value: ::std::string::String) -> SnapshotStateViewModelWithDelegatesItem {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(SnapshotStateViewModelWithDelegatesItem(rr)), {
            SnapshotStateViewModelWithDelegatesItem { id, value }
        })
    }
    pub fn id(&self) -> UUID {
        self.id
    }
    pub fn value(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.value)
    }
    pub fn component1(&self) -> UUID {
        self.id()
    }
    pub fn component2(&self) -> ::std::string::String {
        self.value()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl SnapshotStateViewModelWithDelegatesItem {
    pub(crate) fn copy(
        &self,
        # [default (self . id)] id: UUID,
        # [default (:: core :: clone :: Clone :: clone (& self . value))] value: ::std::string::String,
    ) -> SnapshotStateViewModelWithDelegatesItem {
        SnapshotStateViewModelWithDelegatesItem::__kz_new(id, value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for SnapshotStateViewModelWithDelegatesItem {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Item(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push_str(", value=");
        (&::kompoz::runtime::kt::KtProbe(&self.value)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for SnapshotStateViewModelWithDelegatesItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for SnapshotStateViewModelWithDelegatesItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzSnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    #[doc(hidden)]
    pub __kz_super: ::kompoz::runtime::kt::KtVal<ViewModelImpl>,
    items: ::kompoz::runtime::kt::KtVal<
        ::std::rc::Rc<
            dyn ::kompoz::runtime::kt::KtDelegate<Value = SnapshotStateList<SnapshotStateViewModelWithDelegatesItem>>,
        >,
    >,
    selected_item_ids: ::kompoz::runtime::kt::KtVal<
        ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtDelegate<Value = SnapshotStateMap<UUID, ()>>>,
    >,
    are_selections_enabled:
        ::kompoz::runtime::kt::KtVal<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtMutableDelegate<Value = bool>>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzSnapshotStateViewModelWithDelegatesSnapshotStateViewModel>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn SnapshotStateViewModelWithDelegatesSnapshotStateViewModel(
    handle: SavedStateHandle,
) -> SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    SnapshotStateViewModelWithDelegatesSnapshotStateViewModel::__kz_new(handle)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    #[doc(hidden)]
    pub(crate) fn __kz_new(handle: SavedStateHandle) -> SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
        let __kz_this = SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
            __kz_this: ::std::rc::Rc::new(__KzSnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
                __kz_super: ::kompoz::runtime::kt::KtVal::new(),
                items: ::kompoz::runtime::kt::KtVal::new(),
                selected_item_ids: ::kompoz::runtime::kt::KtVal::new(),
                are_selections_enabled: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(handle);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, handle: SavedStateHandle) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.__kz_super.init(ViewModelImpl::new());
        ::kompoz::runtime::named_args!(types(SnapshotStateViewModelWithDelegatesItem(rr)), {
            self.__kz_this.items.init(::std::rc::Rc::new(::kompoz::runtime::kt::delegate_of::<
                SnapshotStateList<SnapshotStateViewModelWithDelegatesItem>,
                _,
            >(::kompoz::runtime::kt::KtValue(
                handle.saveable_with(
                    "SnapshotStateViewModel.items",
                    list_saver(
                        move |it: &SnapshotStateList<SnapshotStateViewModelWithDelegatesItem>| {
                            let it = ::core::clone::Clone::clone(it);
                            ::kompoz::runtime::kt::KtList::map(&it, |item| {
                                ::kompoz::runtime::kt::list_of([
                                    ::kompoz::runtime::kt::to_string(&item.id()),
                                    item.value(),
                                ])
                            })
                        },
                        move |it| {
                            ::core::option::Option::Some(::kompoz::runtime::kt::KtList::to_mutable_state_list(
                                &::kompoz::runtime::kt::KtList::map(&it, |saved| {
                                    SnapshotStateViewModelWithDelegatesItem(
                                        UUID::from_string(&::kompoz::runtime::kt::KtList::get(&saved, 0)),
                                        ::kompoz::runtime::kt::KtList::get(&saved, 1),
                                    )
                                }),
                            ))
                        },
                    ),
                    || mutable_state_list_of([]),
                ),
            ))));
            self.__kz_this.selected_item_ids.init(::std::rc::Rc::new(::kompoz::runtime::kt::delegate_of::<
                SnapshotStateMap<UUID, ()>,
                _,
            >(::kompoz::runtime::kt::KtValue(
                handle.saveable_with(
                    "SnapshotStateViewModel.selectedItemIds",
                    list_saver(
                        move |it: &SnapshotStateMap<UUID, ()>| {
                            let it = ::core::clone::Clone::clone(it);
                            ::kompoz::runtime::kt::KtList::map(
                                &::kompoz::runtime::kt::KtMapExt::keys(&it),
                                move |__kz_r: UUID| ::kompoz::runtime::kt::to_string(&__kz_r),
                            )
                        },
                        move |it| {
                            ::core::option::Option::Some(::kompoz::runtime::kt::KtList::to_mutable_state_map(
                                &::kompoz::runtime::kt::KtList::map(
                                    &::kompoz::runtime::kt::KtList::map(&it, move |__kz_p0| {
                                        UUID::from_string(&__kz_p0.clone())
                                    }),
                                    |id| ::kompoz::runtime::kt::to(id.clone(), ()),
                                ),
                            ))
                        },
                    ),
                    || mutable_state_map_of([]),
                ),
            ))));
            self.__kz_this.are_selections_enabled.init(::std::rc::Rc::new(
                ::kompoz::runtime::kt::delegate_of::<bool, _>(
                    handle.saveable("SnapshotStateViewModel.areSelectionsEnabled", || {
                        ::kompoz::runtime::mutable_state_of!(true)
                    }),
                ),
            ));
        })
    }
    fn items(&self) -> SnapshotStateList<SnapshotStateViewModelWithDelegatesItem> {
        self.__kz_this.items.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
    }
    fn selected_item_ids(&self) -> SnapshotStateMap<UUID, ()> {
        self.__kz_this.selected_item_ids.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
    }
    pub fn are_selections_enabled(&self) -> bool {
        self.__kz_this.are_selections_enabled.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
    }
    pub fn set_are_selections_enabled(&self, value: bool) {
        self.__kz_this
            .are_selections_enabled
            .with(|__kz_d| ::kompoz::runtime::kt::KtMutableDelegate::set_value(__kz_d, value))
    }
    pub fn selected_items(&self) -> ::std::vec::Vec<(SnapshotStateViewModelWithDelegatesItem, bool)> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::KtList::map(&self.items(), |it| {
                ::kompoz::runtime::kt::to(
                    it.clone(),
                    (::kompoz::runtime::kt::is_in(&it.id(), &self.selected_item_ids())),
                )
            })
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    pub fn select_item(&self, id: UUID, selected: bool) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            if selected {
                self.selected_item_ids().put(id, ());
            } else {
                self.selected_item_ids().remove(&id);
            }
        })
    }
    pub fn add_item(&self, value: impl ::core::convert::Into<::std::string::String>) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let value: ::std::string::String = ::core::convert::Into::into(value);
        ::kompoz::runtime::named_args!(types(SnapshotStateViewModelWithDelegatesItem(rr)), {
            self.items().add(SnapshotStateViewModelWithDelegatesItem(UUID::random_uuid(), value.clone()));
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "SnapshotStateViewModel",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ViewModel for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        (&self.__kz_this.__kz_super).get_ref()
    }
    fn new_instance(extras: &CreationExtras) -> ::core::option::Option<Self> {
        let _ = extras;
        ::core::option::Option::Some(SnapshotStateViewModelWithDelegatesSnapshotStateViewModel::__kz_new(
            extras.create_saved_state_handle(),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ViewModelRef for SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
    type Model = SnapshotStateViewModelWithDelegatesSnapshotStateViewModel;
    fn from_rc(
        model: ::std::rc::Rc<SnapshotStateViewModelWithDelegatesSnapshotStateViewModel>,
    ) -> SnapshotStateViewModelWithDelegatesSnapshotStateViewModel {
        ::core::clone::Clone::clone(&*model)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn encode_with_explicit_serializer() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(EncodeWithExplicitSerializerUUIDSerializer()), {
        encode_to_saved_state_with(EncodeWithExplicitSerializerUUIDSerializer(), &UUID::random_uuid());
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzEncodeWithExplicitSerializerUUIDSerializer {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct EncodeWithExplicitSerializerUUIDSerializer {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzEncodeWithExplicitSerializerUUIDSerializer>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn EncodeWithExplicitSerializerUUIDSerializer() -> EncodeWithExplicitSerializerUUIDSerializer {
    EncodeWithExplicitSerializerUUIDSerializer::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl EncodeWithExplicitSerializerUUIDSerializer {
    #[doc(hidden)]
    pub(crate) fn __kz_new() -> EncodeWithExplicitSerializerUUIDSerializer {
        let __kz_this = EncodeWithExplicitSerializerUUIDSerializer {
            __kz_this: ::std::rc::Rc::new(__KzEncodeWithExplicitSerializerUUIDSerializer {}),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    pub fn descriptor(&self) -> SerialDescriptor {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ PrimitiveSerialDescriptor("UUIDSerializer", PrimitiveKind::STRING) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl EncodeWithExplicitSerializerUUIDSerializer {
    pub fn deserialize(&self, decoder: Decoder) -> UUID {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&decoder,);
        ::kompoz::runtime::named_args!({
            return UUID::from_string(&decoder.decode_string());
        })
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
impl ::core::cmp::PartialEq for EncodeWithExplicitSerializerUUIDSerializer {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for EncodeWithExplicitSerializerUUIDSerializer {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for EncodeWithExplicitSerializerUUIDSerializer {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for EncodeWithExplicitSerializerUUIDSerializer {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "UUIDSerializer",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for EncodeWithExplicitSerializerUUIDSerializer {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for EncodeWithExplicitSerializerUUIDSerializer {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<EncodeWithExplicitSerializerUUIDSerializer> for KSerializer<UUID> {
    fn from(o: EncodeWithExplicitSerializerUUIDSerializer) -> KSerializer<UUID> {
        KSerializer::new(o)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl KSerializerImpl for EncodeWithExplicitSerializerUUIDSerializer {
    type Value = UUID;
    fn descriptor(&self) -> SerialDescriptor {
        EncodeWithExplicitSerializerUUIDSerializer::descriptor(self)
    }
    fn deserialize(&self, decoder: &Decoder) -> UUID {
        EncodeWithExplicitSerializerUUIDSerializer::deserialize(self, ::core::clone::Clone::clone(decoder))
    }
    fn serialize(&self, encoder: &Encoder, value: &UUID) {
        EncodeWithExplicitSerializerUUIDSerializer::serialize(
            self,
            ::core::clone::Clone::clone(encoder),
            ::core::clone::Clone::clone(value),
        )
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn decode_with_explicit_serializer() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(DecodeWithExplicitSerializerUUIDSerializer()), {
        let uuid = decode_from_saved_state_with(
            DecodeWithExplicitSerializerUUIDSerializer(),
            &uuid_saved_state.with(::core::clone::Clone::clone),
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzDecodeWithExplicitSerializerUUIDSerializer {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct DecodeWithExplicitSerializerUUIDSerializer {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzDecodeWithExplicitSerializerUUIDSerializer>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn DecodeWithExplicitSerializerUUIDSerializer() -> DecodeWithExplicitSerializerUUIDSerializer {
    DecodeWithExplicitSerializerUUIDSerializer::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DecodeWithExplicitSerializerUUIDSerializer {
    #[doc(hidden)]
    pub(crate) fn __kz_new() -> DecodeWithExplicitSerializerUUIDSerializer {
        let __kz_this = DecodeWithExplicitSerializerUUIDSerializer {
            __kz_this: ::std::rc::Rc::new(__KzDecodeWithExplicitSerializerUUIDSerializer {}),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    pub fn descriptor(&self) -> SerialDescriptor {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ PrimitiveSerialDescriptor("UUIDSerializer", PrimitiveKind::STRING) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl DecodeWithExplicitSerializerUUIDSerializer {
    pub fn deserialize(&self, decoder: Decoder) -> UUID {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&decoder,);
        ::kompoz::runtime::named_args!({
            return UUID::from_string(&decoder.decode_string());
        })
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
impl ::core::cmp::PartialEq for DecodeWithExplicitSerializerUUIDSerializer {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for DecodeWithExplicitSerializerUUIDSerializer {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for DecodeWithExplicitSerializerUUIDSerializer {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for DecodeWithExplicitSerializerUUIDSerializer {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "UUIDSerializer",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for DecodeWithExplicitSerializerUUIDSerializer {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for DecodeWithExplicitSerializerUUIDSerializer {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<DecodeWithExplicitSerializerUUIDSerializer> for KSerializer<UUID> {
    fn from(o: DecodeWithExplicitSerializerUUIDSerializer) -> KSerializer<UUID> {
        KSerializer::new(o)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl KSerializerImpl for DecodeWithExplicitSerializerUUIDSerializer {
    type Value = UUID;
    fn descriptor(&self) -> SerialDescriptor {
        DecodeWithExplicitSerializerUUIDSerializer::descriptor(self)
    }
    fn deserialize(&self, decoder: &Decoder) -> UUID {
        DecodeWithExplicitSerializerUUIDSerializer::deserialize(self, ::core::clone::Clone::clone(decoder))
    }
    fn serialize(&self, encoder: &Encoder, value: &UUID) {
        DecodeWithExplicitSerializerUUIDSerializer::serialize(
            self,
            ::core::clone::Clone::clone(encoder),
            ::core::clone::Clone::clone(value),
        )
    }
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static uuid_saved_state : SavedState = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (saved_state (| this | { this . put_string ("" , :: kompoz :: runtime :: kt :: to_string (& UUID :: random_uuid ())) ; })) } ; }
