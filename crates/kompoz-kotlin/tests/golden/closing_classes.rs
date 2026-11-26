#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct AppFaultBase {
    #[doc(hidden)]
    pub __kz_exception: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::Throwable>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppFaultBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        AppFaultBase { __kz_exception: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait AppFaultOpen: ::kompoz::runtime::kt::KtObject {
    #[doc = r" Kotlin's `message`."]
    fn message(&self) -> ::core::option::Option<::std::string::String> {
        self.__kz_app_fault().__kz_exception.with(|e| e.message())
    }
    #[doc = r" Kotlin's `cause`."]
    fn cause(&self) -> ::core::option::Option<::kompoz::runtime::kt::Throwable> {
        self.__kz_app_fault().__kz_exception.with(|e| e.cause())
    }
    #[doc(hidden)]
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::exception(
            "IllegalStateException",
            AppFaultOpen::message(self),
            AppFaultOpen::cause(self),
        )
    }
    #[doc(hidden)]
    fn __kz_app_fault(&self) -> &AppFaultBase;
    #[doc(hidden)]
    fn __kz_app_fault_init(&self, message: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_app_fault().__kz_exception.init(::kompoz::runtime::kt::exception(
                "IllegalStateException",
                ::core::option::Option::map(::core::option::Option::Some(message.clone()), ::std::string::String::from),
                ::core::option::Option::None,
            ));
        })
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtThrowableObject for dyn AppFaultOpen {
    fn kt_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        AppFaultOpen::__kz_throwable(self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn AppFaultOpen {
    fn eq(&self, other: &Self) -> bool {
        AppFaultOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn AppFaultOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        AppFaultOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn AppFaultOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn AppFaultOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn AppFaultOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            AppFaultOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for AppFault {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::Throwable::of_class_object(
            "AppFault",
            &[],
            "IllegalStateException",
            <Self as AppFaultOpen>::message(self),
            <Self as AppFaultOpen>::cause(self),
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
            |o, to| <AppFault as AppFaultOpen>::__kz_cast(o.downcast_ref::<AppFault>()?, to),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<AppFault> for ::kompoz::runtime::kt::Throwable {
    fn from(e: AppFault) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<AppFault> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<AppFault> {
        self.downcast::<AppFault>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzAppFault {
    #[doc(hidden)]
    pub __kz_super: AppFaultBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct AppFault {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzAppFault>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn AppFault(message: impl ::core::convert::Into<::std::string::String>) -> AppFault {
    let message: ::std::string::String = ::core::convert::Into::into(message);
    AppFault::__kz_new(message)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppFault {
    #[doc(hidden)]
    pub fn __kz_new(message: ::std::string::String) -> AppFault {
        let __kz_this =
            AppFault { __kz_this: ::std::rc::Rc::new(__KzAppFault { __kz_super: AppFaultBase::__kz_empty() }) };
        __kz_this.__kz_init(message);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, message: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        AppFaultOpen::__kz_app_fault_init(self, ::std::string::String::from(message.clone()));
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl AppFault {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for AppFault {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for AppFault {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for AppFault {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for AppFault {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for AppFault {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for AppFault {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppFaultOpen for AppFault {
    fn __kz_app_fault(&self) -> &AppFaultBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn AppFaultOpen>>() {
            let o: ::std::rc::Rc<dyn AppFaultOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<AppFault> for ::std::rc::Rc<dyn AppFaultOpen> {
    fn eq(&self, other: &AppFault) -> bool {
        AppFaultOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn AppFaultOpen>> for AppFault {
    fn eq(&self, other: &::std::rc::Rc<dyn AppFaultOpen>) -> bool {
        AppFaultOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn AppFaultOpen>> for AppFault {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn AppFaultOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for DiskFault {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::Throwable::of_class_object(
            "DiskFault",
            &["AppFault"],
            "IllegalStateException",
            <Self as AppFaultOpen>::message(self),
            <Self as AppFaultOpen>::cause(self),
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
            |o, to| <DiskFault as AppFaultOpen>::__kz_cast(o.downcast_ref::<DiskFault>()?, to),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<DiskFault> for ::kompoz::runtime::kt::Throwable {
    fn from(e: DiskFault) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<DiskFault> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<DiskFault> {
        self.downcast::<DiskFault>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzDiskFault {
    #[doc(hidden)]
    pub __kz_super: AppFaultBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct DiskFault {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzDiskFault>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn DiskFault() -> DiskFault {
    DiskFault::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DiskFault {
    #[doc(hidden)]
    pub fn __kz_new() -> DiskFault {
        let __kz_this =
            DiskFault { __kz_this: ::std::rc::Rc::new(__KzDiskFault { __kz_super: AppFaultBase::__kz_empty() }) };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        AppFaultOpen::__kz_app_fault_init(self, ::std::string::String::from(::std::string::String::from("disk")));
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl DiskFault {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for DiskFault {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for DiskFault {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for DiskFault {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for DiskFault {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for DiskFault {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for DiskFault {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppFaultOpen for DiskFault {
    fn __kz_app_fault(&self) -> &AppFaultBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn AppFaultOpen>>() {
            let o: ::std::rc::Rc<dyn AppFaultOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<DiskFault> for ::std::rc::Rc<dyn AppFaultOpen> {
    fn eq(&self, other: &DiskFault) -> bool {
        AppFaultOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn AppFaultOpen>> for DiskFault {
    fn eq(&self, other: &::std::rc::Rc<dyn AppFaultOpen>) -> bool {
        AppFaultOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn AppFaultOpen>> for DiskFault {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn AppFaultOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn fault_kind(e: ::std::rc::Rc<dyn AppFaultOpen>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        if let ::core::option::Option::Some(e) = {
            let __kz_t1 = ::kompoz::runtime::kt::KtThrowable::to_throwable(&e.clone());
            if __kz_t1.is_a("IllegalStateException") {
                ::core::option::Option::Some(__kz_t1)
            } else {
                ::core::option::Option::None
            }
        } {
            ::kompoz::runtime::kt::template!("ise ", e.message())
        } else {
            ::std::string::String::from("başka")
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn disk_kind() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(DiskFault()), { fault_kind(::std::rc::Rc::new(DiskFault())) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct TroubleBase {
    #[doc(hidden)]
    pub __kz_exception: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::Throwable>,
    code: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TroubleBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        TroubleBase { __kz_exception: ::kompoz::runtime::kt::KtVal::new(), code: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait TroubleOpen: ::kompoz::runtime::kt::KtObject {
    #[doc = r" Kotlin's `message`."]
    fn message(&self) -> ::core::option::Option<::std::string::String> {
        self.__kz_trouble().__kz_exception.with(|e| e.message())
    }
    #[doc = r" Kotlin's `cause`."]
    fn cause(&self) -> ::core::option::Option<::kompoz::runtime::kt::Throwable> {
        self.__kz_trouble().__kz_exception.with(|e| e.cause())
    }
    #[doc(hidden)]
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::exception("IllegalStateException", TroubleOpen::message(self), TroubleOpen::cause(self))
    }
    #[doc(hidden)]
    fn __kz_trouble(&self) -> &TroubleBase;
    fn code(&self) -> i32 {
        self.__kz_trouble().code.get()
    }
    #[doc(hidden)]
    fn __kz_trouble_init(&self, code: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_trouble().__kz_exception.init(::kompoz::runtime::kt::exception(
                "IllegalStateException",
                ::core::option::Option::map(
                    ::core::option::Option::Some(::kompoz::runtime::kt::template!("kod ", code)),
                    ::std::string::String::from,
                ),
                ::core::option::Option::None,
            ));
            self.__kz_trouble().code.init(code);
        })
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub enum Trouble {
    NotFound(TroubleNotFound),
    Snapped(TroubleSnapped),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<TroubleNotFound> for Trouble {
    fn from(value: TroubleNotFound) -> Trouble {
        Trouble::NotFound(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<TroubleNotFound> for Trouble {
    fn kt_as_safe(&self) -> ::core::option::Option<TroubleNotFound> {
        match self {
            Trouble::NotFound(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<TroubleNotFound> for Trouble {
    fn eq(&self, other: &TroubleNotFound) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Trouble::NotFound(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Trouble> for TroubleNotFound {
    fn eq(&self, other: &Trouble) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<TroubleSnapped> for Trouble {
    fn from(value: TroubleSnapped) -> Trouble {
        Trouble::Snapped(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<TroubleSnapped> for Trouble {
    fn kt_as_safe(&self) -> ::core::option::Option<TroubleSnapped> {
        match self {
            Trouble::Snapped(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<TroubleSnapped> for Trouble {
    fn eq(&self, other: &TroubleSnapped) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Trouble::Snapped(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Trouble> for TroubleSnapped {
    fn eq(&self, other: &Trouble) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtThrowable for Trouble {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        TroubleOpen::__kz_throwable(self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<Trouble> for ::kompoz::runtime::kt::Throwable {
    fn from(e: Trouble) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<Trouble> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<Trouble> {
        if let ::core::option::Option::Some(v) = self.downcast::<TroubleNotFound>() {
            return ::core::option::Option::Some(Trouble::NotFound(v));
        }
        if let ::core::option::Option::Some(v) = self.downcast::<TroubleSnapped>() {
            return ::core::option::Option::Some(Trouble::Snapped(v));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Trouble {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Trouble::NotFound(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Trouble::Snapped(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Trouble {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Trouble {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TroubleOpen for Trouble {
    fn __kz_trouble(&self) -> &TroubleBase {
        match self {
            Trouble::NotFound(v) => TroubleOpen::__kz_trouble(v),
            Trouble::Snapped(v) => TroubleOpen::__kz_trouble(v),
        }
    }
    fn code(&self) -> i32 {
        match self {
            Trouble::NotFound(v) => TroubleOpen::code(v),
            Trouble::Snapped(v) => TroubleOpen::code(v),
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Trouble::NotFound(v) => TroubleOpen::__kz_eq(v, other),
                Trouble::Snapped(v) => TroubleOpen::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = &to;
        match self {
            Trouble::NotFound(v) => TroubleOpen::__kz_cast(v, to),
            Trouble::Snapped(v) => TroubleOpen::__kz_cast(v, to),
        }
    }
    fn __kz_trouble_init(&self, _: i32) {}
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        #[allow(unreachable_patterns)]
        match self {
            Trouble::NotFound(v) => ::kompoz::runtime::kt::KtThrowable::to_throwable(v),
            Trouble::Snapped(v) => ::kompoz::runtime::kt::KtThrowable::to_throwable(v),
            _ => ::core::unreachable!("a sealed class's value is one of its cases"),
        }
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for TroubleNotFound {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::Throwable::of_class_object(
            "NotFound",
            &["Trouble"],
            "IllegalStateException",
            <Self as TroubleOpen>::message(self),
            <Self as TroubleOpen>::cause(self),
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
            |o, to| <TroubleNotFound as TroubleOpen>::__kz_cast(o.downcast_ref::<TroubleNotFound>()?, to),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<TroubleNotFound> for ::kompoz::runtime::kt::Throwable {
    fn from(e: TroubleNotFound) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<TroubleNotFound> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<TroubleNotFound> {
        self.downcast::<TroubleNotFound>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzTroubleNotFound {
    #[doc(hidden)]
    pub __kz_super: TroubleBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct TroubleNotFound {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzTroubleNotFound>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn TroubleNotFound() -> TroubleNotFound {
    TroubleNotFound::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TroubleNotFound {
    #[doc(hidden)]
    pub fn __kz_new() -> TroubleNotFound {
        let __kz_this = TroubleNotFound {
            __kz_this: ::std::rc::Rc::new(__KzTroubleNotFound { __kz_super: TroubleBase::__kz_empty() }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        TroubleOpen::__kz_trouble_init(self, 404);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl TroubleNotFound {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for TroubleNotFound {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for TroubleNotFound {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for TroubleNotFound {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for TroubleNotFound {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for TroubleNotFound {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for TroubleNotFound {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TroubleOpen for TroubleNotFound {
    fn __kz_trouble(&self) -> &TroubleBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Trouble>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Trouble as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for TroubleSnapped {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::Throwable::of_class_object(
            "Snapped",
            &["Trouble"],
            "IllegalStateException",
            <Self as TroubleOpen>::message(self),
            <Self as TroubleOpen>::cause(self),
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
            |o, to| <TroubleSnapped as TroubleOpen>::__kz_cast(o.downcast_ref::<TroubleSnapped>()?, to),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<TroubleSnapped> for ::kompoz::runtime::kt::Throwable {
    fn from(e: TroubleSnapped) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<TroubleSnapped> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<TroubleSnapped> {
        self.downcast::<TroubleSnapped>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzTroubleSnapped {
    #[doc(hidden)]
    pub __kz_super: TroubleBase,
    why: ::kompoz::runtime::kt::KtVal<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct TroubleSnapped {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzTroubleSnapped>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn TroubleSnapped(why: impl ::core::convert::Into<::std::string::String>) -> TroubleSnapped {
    let why: ::std::string::String = ::core::convert::Into::into(why);
    TroubleSnapped::__kz_new(why)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TroubleSnapped {
    #[doc(hidden)]
    pub fn __kz_new(why: ::std::string::String) -> TroubleSnapped {
        let __kz_this = TroubleSnapped {
            __kz_this: ::std::rc::Rc::new(__KzTroubleSnapped {
                __kz_super: TroubleBase::__kz_empty(),
                why: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(why);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, why: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        TroubleOpen::__kz_trouble_init(self, 500);
        self.__kz_this.why.init(::core::clone::Clone::clone(&why));
        ::kompoz::runtime::named_args!({})
    }
    pub fn why(&self) -> ::std::string::String {
        self.__kz_this.why.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl TroubleSnapped {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for TroubleSnapped {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for TroubleSnapped {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for TroubleSnapped {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for TroubleSnapped {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for TroubleSnapped {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for TroubleSnapped {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TroubleOpen for TroubleSnapped {
    fn __kz_trouble(&self) -> &TroubleBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Trouble>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Trouble as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct MishapBase {
    #[doc(hidden)]
    pub __kz_exception: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::Throwable>,
    code: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl MishapBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        MishapBase { __kz_exception: ::kompoz::runtime::kt::KtVal::new(), code: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait MishapOpen: ::kompoz::runtime::kt::KtObject {
    #[doc = r" Kotlin's `message`."]
    fn message(&self) -> ::core::option::Option<::std::string::String> {
        self.__kz_mishap().__kz_exception.with(|e| e.message())
    }
    #[doc = r" Kotlin's `cause`."]
    fn cause(&self) -> ::core::option::Option<::kompoz::runtime::kt::Throwable> {
        self.__kz_mishap().__kz_exception.with(|e| e.cause())
    }
    #[doc(hidden)]
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::exception("IllegalStateException", MishapOpen::message(self), MishapOpen::cause(self))
    }
    #[doc(hidden)]
    fn __kz_mishap(&self) -> &MishapBase;
    fn code(&self) -> i32 {
        self.__kz_mishap().code.get()
    }
    #[doc(hidden)]
    fn __kz_mishap_init(&self, code: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_mishap().__kz_exception.init(::kompoz::runtime::kt::exception(
                "IllegalStateException",
                ::core::option::Option::map(
                    ::core::option::Option::Some(::kompoz::runtime::kt::template!("kod ", code)),
                    ::std::string::String::from,
                ),
                ::core::option::Option::None,
            ));
            self.__kz_mishap().code.init(code);
        })
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub enum Mishap {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtThrowable for Mishap {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        MishapOpen::__kz_throwable(self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<Mishap> for ::kompoz::runtime::kt::Throwable {
    fn from(e: Mishap) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<Mishap> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<Mishap> {
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Mishap {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match *self {
            _ => {}
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Mishap {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Mishap {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl MishapOpen for Mishap {
    fn __kz_mishap(&self) -> &MishapBase {
        match *self {}
    }
    fn code(&self) -> i32 {
        match *self {}
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self) || match *self {}
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = &to;
        match *self {}
    }
    fn __kz_mishap_init(&self, _: i32) {}
    fn __kz_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        #[allow(unreachable_patterns)]
        match *self {
            _ => ::core::unreachable!("a sealed class's value is one of its cases"),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn risky(n: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(TroubleNotFound(), TroubleSnapped(r)), {
        if n == 1 {
            ::kompoz::runtime::kt::throw(TroubleNotFound());
        }
        if n == 2 {
            ::kompoz::runtime::kt::throw(TroubleSnapped("bozuk"));
        }
        return n;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn try_risky(n: i32) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(TroubleSnapped(r)), {
        match ::kompoz::runtime::kt::catching(|| ::kompoz::runtime::kt::template!("ok ", risky(n))) {
            ::core::result::Result::Ok(__kz_v) => __kz_v,
            ::core::result::Result::Err(__kz_e) => {
                if __kz_e.is_object::<TroubleSnapped>() {
                    #[allow(unused_variables)]
                    let e = ::core::option::Option::unwrap(__kz_e.downcast::<TroubleSnapped>());
                    ::kompoz::runtime::kt::template!("broken ", e.why(), " ", e.code(), " ", e.message())
                } else {
                    if __kz_e.is_a("Trouble") {
                        #[allow(unused_variables)]
                        let e =
                            ::core::option::Option::unwrap(::kompoz::runtime::kt::KtAs::<Trouble>::kt_as_safe(&__kz_e));
                        ::kompoz::runtime::kt::template!(
                            "trouble ",
                            e.code(),
                            " ",
                            (::kompoz::runtime::kt::KtThrowable::to_throwable(&e).is_a("IllegalStateException")),
                            " ",
                            e
                        )
                    } else {
                        ::kompoz::runtime::kt::rethrow(__kz_e)
                    }
                }
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Coin {
    pub amount: i32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Coin(amount: i32) -> Coin {
    Coin::__kz_new(amount)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Coin {
    #[doc(hidden)]
    pub fn __kz_new(amount: i32) -> Coin {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Coin(r)), { Coin { amount } })
    }
    pub fn amount(&self) -> i32 {
        self.amount
    }
    pub fn component1(&self) -> i32 {
        self.amount()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Coin {
    pub fn copy(&self, # [default (self . amount)] amount: i32) -> Coin {
        Coin::__kz_new(amount)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Coin {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.amount, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Coin {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Coin {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Coin(");
        __kz_out.push_str("amount=");
        (&::kompoz::runtime::kt::KtProbe(&self.amount)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Coin {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Coin {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn same_coin(
    m: impl ::core::convert::Into<::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>>>,
) -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let m: ::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>> = ::core::convert::Into::into(m);
    ::kompoz::runtime::named_args!(types(Coin(r)), {
        ::kompoz::runtime::kt::eq_object(::core::option::Option::Some(&Coin(5)), ::core::option::Option::as_deref(&m))
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn other_coin(m: ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>) -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Coin(r)), {
        !::kompoz::runtime::kt::eq_object(::core::option::Option::Some(&Coin(5)), ::core::option::Option::Some(&*m))
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Speckle {
    pub x: i32,
    pub y: f32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Speckle(x: i32, y: f32) -> Speckle {
    Speckle::__kz_new(x, y)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Speckle {
    #[doc(hidden)]
    pub fn __kz_new(x: i32, y: f32) -> Speckle {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Speckle(rr)), { Speckle { x, y } })
    }
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn y(&self) -> f32 {
        self.y
    }
    pub fn component1(&self) -> i32 {
        self.x()
    }
    pub fn component2(&self) -> f32 {
        self.y()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Speckle {
    pub fn copy(&self, # [default (self . x)] x: i32, # [default (self . y)] y: f32) -> Speckle {
        Speckle::__kz_new(x, y)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Speckle {
    fn eq(&self, other: &Self) -> bool {
        true && self.x == other.x
            && ::kompoz::runtime::kt::KtBits::kt_bits(&self.y) == ::kompoz::runtime::kt::KtBits::kt_bits(&other.y)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Speckle {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.x, state);
        ::core::hash::Hash::hash(&::kompoz::runtime::kt::KtBits::kt_bits(&self.y), state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Speckle {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Speckle {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Speckle(");
        __kz_out.push_str("x=");
        (&::kompoz::runtime::kt::KtProbe(&self.x)).kt_write_probe(__kz_out);
        __kz_out.push_str(", y=");
        (&::kompoz::runtime::kt::KtProbe(&self.y)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Speckle {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Speckle {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn speckles() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Speckle(rr)), {
        ::kompoz::runtime::kt::KtList::size(&::kompoz::runtime::kt::set_of([
            Speckle(1, 2_f32),
            Speckle(1, 2_f32),
            Speckle(1, -0_f32),
            Speckle(1, 0_f32),
        ]))
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn speckle_hashes() -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Speckle(rr)), {
        ::kompoz::runtime::kt::hash_code_of(&Speckle(1, 2_f32))
            == ::kompoz::runtime::kt::hash_code_of(&Speckle(1, 2_f32))
            && ::kompoz::runtime::kt::KtHashCode::kt_hash_code(&::std::string::String::from("ab")) == 3105
            && ::kompoz::runtime::kt::KtHashCode::kt_hash_code(&::kompoz::runtime::kt::list_of([1, 2])) == 994
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzKtVec {
    x: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct KtVec {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzKtVec>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn KtVec(x: i32) -> KtVec {
    KtVec::__kz_new(x)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl KtVec {
    #[doc(hidden)]
    pub fn __kz_new(x: i32) -> KtVec {
        let __kz_this = KtVec { __kz_this: ::std::rc::Rc::new(__KzKtVec { x: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(x);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, x: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.x.init(x);
        ::kompoz::runtime::named_args!({})
    }
    pub fn x(&self) -> i32 {
        self.__kz_this.x.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl KtVec {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for KtVec {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for KtVec {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for KtVec {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for KtVec {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Vec",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for KtVec {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for KtVec {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn vec() -> KtVec {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(KtVec(r)), { KtVec(1) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn vecs() -> ::std::vec::Vec<KtVec> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(KtVec(r)), { ::kompoz::runtime::kt::list_of([KtVec(2), KtVec(3)]) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct CreatureBase {
    name: ::kompoz::runtime::kt::KtVal<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl CreatureBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        CreatureBase { name: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait CreatureOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_creature(&self) -> &CreatureBase;
    fn name(&self) -> ::std::string::String {
        self.__kz_creature().name.get()
    }
    #[doc(hidden)]
    fn __kz_creature_init(&self, name: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_creature().name.init(::core::clone::Clone::clone(&name));
        })
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn CreatureOpen {
    fn eq(&self, other: &Self) -> bool {
        CreatureOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn CreatureOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        CreatureOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn CreatureOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn CreatureOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn CreatureOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            CreatureOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzCreature {
    #[doc(hidden)]
    pub __kz_super: CreatureBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Creature {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzCreature>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Creature(name: impl ::core::convert::Into<::std::string::String>) -> Creature {
    let name: ::std::string::String = ::core::convert::Into::into(name);
    Creature::__kz_new(name)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Creature {
    #[doc(hidden)]
    pub fn __kz_new(name: ::std::string::String) -> Creature {
        let __kz_this =
            Creature { __kz_this: ::std::rc::Rc::new(__KzCreature { __kz_super: CreatureBase::__kz_empty() }) };
        __kz_this.__kz_init(name);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, name: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        CreatureOpen::__kz_creature_init(self, ::std::string::String::from(name.clone()));
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Creature {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Creature {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Creature {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Creature {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Creature {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Creature",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Creature {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Creature {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl CreatureOpen for Creature {
    fn __kz_creature(&self) -> &CreatureBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn CreatureOpen>>() {
            let o: ::std::rc::Rc<dyn CreatureOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Creature> for ::std::rc::Rc<dyn CreatureOpen> {
    fn eq(&self, other: &Creature) -> bool {
        CreatureOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn CreatureOpen>> for Creature {
    fn eq(&self, other: &::std::rc::Rc<dyn CreatureOpen>) -> bool {
        CreatureOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn CreatureOpen>> for Creature {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn CreatureOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzHound {
    #[doc(hidden)]
    pub __kz_super: CreatureBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Hound {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzHound>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Hound(name: impl ::core::convert::Into<::std::string::String>) -> Hound {
    let name: ::std::string::String = ::core::convert::Into::into(name);
    Hound::__kz_new(name)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Hound {
    #[doc(hidden)]
    pub fn __kz_new(name: ::std::string::String) -> Hound {
        let __kz_this = Hound { __kz_this: ::std::rc::Rc::new(__KzHound { __kz_super: CreatureBase::__kz_empty() }) };
        __kz_this.__kz_init(name);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, name: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        CreatureOpen::__kz_creature_init(self, ::std::string::String::from(name.clone()));
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Hound {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Hound {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Hound {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Hound {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Hound {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Hound",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Hound {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Hound {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl CreatureOpen for Hound {
    fn __kz_creature(&self) -> &CreatureBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn CreatureOpen>>() {
            let o: ::std::rc::Rc<dyn CreatureOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Hound> for ::std::rc::Rc<dyn CreatureOpen> {
    fn eq(&self, other: &Hound) -> bool {
        CreatureOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn CreatureOpen>> for Hound {
    fn eq(&self, other: &::std::rc::Rc<dyn CreatureOpen>) -> bool {
        CreatureOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn CreatureOpen>> for Hound {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn CreatureOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct LookupBase {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl LookupBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        LookupBase {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait LookupOpen<T>: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_lookup(&self) -> &LookupBase;
    #[doc(hidden)]
    fn __kz_lookup_init(&self)
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub enum Lookup<T> {
    Found(LookupFound<T>),
    Pairs(LookupPairs<T>),
    Missing(LookupMissing),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for Lookup<T>
{
    fn eq(&self, other: &Self) -> bool {
        #[allow(unreachable_patterns)]
        match (self, other) {
            (Lookup::Found(a), Lookup::Found(b)) => a == b,
            (Lookup::Pairs(a), Lookup::Pairs(b)) => a == b,
            (Lookup::Missing(a), Lookup::Missing(b)) => a == b,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<LookupFound<T>> for Lookup<T>
{
    fn from(value: LookupFound<T>) -> Lookup<T> {
        Lookup::Found(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<LookupFound<T>> for Lookup<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<LookupFound<T>> {
        match self {
            Lookup::Found(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<LookupFound<T>> for Lookup<T>
{
    fn eq(&self, other: &LookupFound<T>) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Lookup::Found(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Lookup<T>> for LookupFound<T>
{
    fn eq(&self, other: &Lookup<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<LookupPairs<T>> for Lookup<T>
{
    fn from(value: LookupPairs<T>) -> Lookup<T> {
        Lookup::Pairs(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<LookupPairs<T>> for Lookup<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<LookupPairs<T>> {
        match self {
            Lookup::Pairs(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<LookupPairs<T>> for Lookup<T>
{
    fn eq(&self, other: &LookupPairs<T>) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Lookup::Pairs(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Lookup<T>> for LookupPairs<T>
{
    fn eq(&self, other: &Lookup<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<LookupMissing> for Lookup<T>
{
    fn from(value: LookupMissing) -> Lookup<T> {
        Lookup::Missing(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<LookupMissing> for Lookup<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<LookupMissing> {
        match self {
            Lookup::Missing(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<LookupMissing> for Lookup<T>
{
    fn eq(&self, other: &LookupMissing) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Lookup::Missing(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Lookup<T>> for LookupMissing
{
    fn eq(&self, other: &Lookup<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> Lookup<T> {
    #[doc(hidden)]
    pub fn __kz_out<
        __KzU0: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    >(
        self,
        __kz_f0: &dyn ::core::ops::Fn(T) -> __KzU0,
    ) -> Lookup<__KzU0> {
        match self {
            Lookup::Found(c) => Lookup::Found(c.__kz_out(__kz_f0)),
            Lookup::Pairs(c) => Lookup::Pairs(c.__kz_out(__kz_f0)),
            Lookup::Missing(c) => Lookup::Missing(c),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for Lookup<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Lookup::Found(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Lookup::Pairs(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Lookup::Missing(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for Lookup<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for Lookup<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupOpen<T>
    for Lookup<T>
{
    fn __kz_lookup(&self) -> &LookupBase {
        match self {
            Lookup::Found(v) => LookupOpen::<T>::__kz_lookup(v),
            Lookup::Pairs(v) => LookupOpen::<T>::__kz_lookup(v),
            Lookup::Missing(v) => LookupOpen::<T>::__kz_lookup(v),
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Lookup::Found(v) => LookupOpen::<T>::__kz_eq(v, other),
                Lookup::Pairs(v) => LookupOpen::<T>::__kz_eq(v, other),
                Lookup::Missing(v) => LookupOpen::<T>::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = &to;
        match self {
            Lookup::Found(v) => LookupOpen::<T>::__kz_cast(v, to),
            Lookup::Pairs(v) => LookupOpen::<T>::__kz_cast(v, to),
            Lookup::Missing(v) => LookupOpen::<T>::__kz_cast(v, to),
        }
    }
    fn __kz_lookup_init(&self) {}
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct LookupFound<T> {
    pub value: T,
    pub all: ::std::vec::Vec<T>,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<LookupBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupFound<T> {
    #[doc(hidden)]
    pub fn __kz_out<
        __KzU0: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    >(
        self,
        __kz_f0: &dyn ::core::ops::Fn(T) -> __KzU0,
    ) -> LookupFound<__KzU0> {
        LookupFound {
            value: (__kz_f0)(self.value),
            all: ::core::iter::Iterator::collect::<::std::vec::Vec<_>>(::core::iter::Iterator::map(
                ::core::iter::IntoIterator::into_iter(self.all),
                |__kz_x| (__kz_f0)(__kz_x),
            )),
            __kz_super: self.__kz_super,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn LookupFound<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>(
    value: T,
    all: ::std::vec::Vec<T>,
) -> LookupFound<T> {
    LookupFound::__kz_new(value, all)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupFound<T> {
    #[doc(hidden)]
    pub fn __kz_new(value: T, all: ::std::vec::Vec<T>) -> LookupFound<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(LookupFound(rr)), {
            let __kz_v = LookupFound {
                value: ::core::clone::Clone::clone(&value),
                all: ::core::clone::Clone::clone(&all),
                __kz_super: ::std::rc::Rc::new(LookupBase::__kz_empty()),
            };
            LookupOpen::<T>::__kz_lookup_init(&__kz_v);
            __kz_v
        })
    }
    pub fn value(&self) -> T {
        ::core::clone::Clone::clone(&self.value)
    }
    pub fn all(&self) -> ::std::vec::Vec<T> {
        ::core::clone::Clone::clone(&self.all)
    }
    pub fn component1(&self) -> T {
        self.value()
    }
    pub fn component2(&self) -> ::std::vec::Vec<T> {
        self.all()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupFound<T> {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . value))] value: T,
        # [default (:: core :: clone :: Clone :: clone (& self . all))] all: ::std::vec::Vec<T>,
    ) -> LookupFound<T> {
        LookupFound::__kz_new(value, all)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for LookupFound<T>
{
    fn eq(&self, other: &Self) -> bool {
        true && self.value == other.value && self.all == other.all
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::hash::Hash
    for LookupFound<T>
where
    T: ::core::hash::Hash,
{
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.value, state);
        ::core::hash::Hash::hash(&self.all, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::cmp::Eq
    for LookupFound<T>
where
    T: ::core::cmp::Eq,
{
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for LookupFound<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Found(");
        __kz_out.push_str("value=");
        (&::kompoz::runtime::kt::KtProbe(&self.value)).kt_write_probe(__kz_out);
        __kz_out.push_str(", all=");
        (&::kompoz::runtime::kt::KtProbe(&self.all)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for LookupFound<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for LookupFound<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupOpen<T>
    for LookupFound<T>
{
    fn __kz_lookup(&self) -> &LookupBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Lookup<T>>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Lookup<T> as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct LookupPairs<T> {
    pub p: ::core::option::Option<(T, i32)>,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<LookupBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupPairs<T> {
    #[doc(hidden)]
    pub fn __kz_out<
        __KzU0: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    >(
        self,
        __kz_f0: &dyn ::core::ops::Fn(T) -> __KzU0,
    ) -> LookupPairs<__KzU0> {
        LookupPairs {
            p: ::core::option::Option::map(self.p, |__kz_x| {
                let (__kz_a, __kz_b) = __kz_x;
                ((__kz_f0)(__kz_a), __kz_b)
            }),
            __kz_super: self.__kz_super,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn LookupPairs<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>(
    p: impl ::core::convert::Into<::core::option::Option<(T, i32)>>,
) -> LookupPairs<T> {
    let p: ::core::option::Option<(T, i32)> = ::core::convert::Into::into(p);
    LookupPairs::__kz_new(p)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupPairs<T> {
    #[doc(hidden)]
    pub fn __kz_new(p: ::core::option::Option<(T, i32)>) -> LookupPairs<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(LookupPairs(r)), {
            let __kz_v = LookupPairs {
                p: ::core::clone::Clone::clone(&p),
                __kz_super: ::std::rc::Rc::new(LookupBase::__kz_empty()),
            };
            LookupOpen::<T>::__kz_lookup_init(&__kz_v);
            __kz_v
        })
    }
    pub fn p(&self) -> ::core::option::Option<(T, i32)> {
        ::core::clone::Clone::clone(&self.p)
    }
    pub fn component1(&self) -> ::core::option::Option<(T, i32)> {
        self.p()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupPairs<T> {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . p))] p: ::core::option::Option<(T, i32)>,
    ) -> LookupPairs<T> {
        LookupPairs::__kz_new(p)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for LookupPairs<T>
{
    fn eq(&self, other: &Self) -> bool {
        true && self.p == other.p
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::hash::Hash
    for LookupPairs<T>
where
    T: ::core::hash::Hash,
{
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.p, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::cmp::Eq
    for LookupPairs<T>
where
    T: ::core::cmp::Eq,
{
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for LookupPairs<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Pairs(");
        __kz_out.push_str("p=");
        (&::kompoz::runtime::kt::KtProbe(&self.p)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for LookupPairs<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for LookupPairs<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> LookupOpen<T>
    for LookupPairs<T>
{
    fn __kz_lookup(&self) -> &LookupBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Lookup<T>>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Lookup<T> as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzLookupMissing {
    #[doc(hidden)]
    pub __kz_super: LookupBase,
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
pub struct LookupMissing {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl LookupMissing {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzLookupMissing {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzLookupMissing > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(|| __KzLookupMissing { __kz_super: LookupBase::__kz_empty() }, |_| LookupMissing {}.__kz_init())
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            LookupOpen::<::kompoz::runtime::kt::KtNothing>::__kz_lookup_init(self);
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl LookupMissing {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for LookupMissing {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Lookup.Missing",
            Self::__kz_state() as *const __KzLookupMissing as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for LookupMissing {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for LookupMissing {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzN0: ::core::clone::Clone + 'static> LookupOpen<__KzN0> for LookupMissing {
    fn __kz_lookup(&self) -> &LookupBase {
        &Self::__kz_state().__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Satchel<T> {
    pub item: T,
    pub more: ::core::option::Option<::std::vec::Vec<T>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> Satchel<T> {
    #[doc(hidden)]
    pub fn __kz_out<
        __KzU0: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    >(
        self,
        __kz_f0: &dyn ::core::ops::Fn(T) -> __KzU0,
    ) -> Satchel<__KzU0> {
        Satchel {
            item: (__kz_f0)(self.item),
            more: ::core::option::Option::map(self.more, |__kz_x| {
                ::core::iter::Iterator::collect::<::std::vec::Vec<_>>(::core::iter::Iterator::map(
                    ::core::iter::IntoIterator::into_iter(__kz_x),
                    |__kz_x| (__kz_f0)(__kz_x),
                ))
            }),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Satchel<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>(
    item: T,
    more: impl ::core::convert::Into<::core::option::Option<::std::vec::Vec<T>>>,
) -> Satchel<T> {
    let more: ::core::option::Option<::std::vec::Vec<T>> = ::core::convert::Into::into(more);
    Satchel::__kz_new(item, more)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> Satchel<T> {
    #[doc(hidden)]
    pub fn __kz_new(item: T, more: ::core::option::Option<::std::vec::Vec<T>>) -> Satchel<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Satchel(rr)), { Satchel { item, more } })
    }
    pub fn item(&self) -> T {
        ::core::clone::Clone::clone(&self.item)
    }
    pub fn more(&self) -> ::core::option::Option<::std::vec::Vec<T>> {
        ::core::clone::Clone::clone(&self.more)
    }
    pub fn component1(&self) -> T {
        self.item()
    }
    pub fn component2(&self) -> ::core::option::Option<::std::vec::Vec<T>> {
        self.more()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> Satchel<T> {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . item))] item: T,
        # [default (:: core :: clone :: Clone :: clone (& self . more))] more: ::core::option::Option<
            ::std::vec::Vec<T>,
        >,
    ) -> Satchel<T> {
        Satchel::__kz_new(item, more)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::hash::Hash
    for Satchel<T>
where
    T: ::core::hash::Hash,
{
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.item, state);
        ::core::hash::Hash::hash(&self.more, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::cmp::Eq
    for Satchel<T>
where
    T: ::core::cmp::Eq,
{
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for Satchel<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Satchel(");
        __kz_out.push_str("item=");
        (&::kompoz::runtime::kt::KtProbe(&self.item)).kt_write_probe(__kz_out);
        __kz_out.push_str(", more=");
        (&::kompoz::runtime::kt::KtProbe(&self.more)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for Satchel<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for Satchel<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn hounds() -> Lookup<Hound> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Hound(r), LookupFound(rr)), {
        ::core::convert::Into::<Lookup<Hound>>::into(LookupFound(
            Hound("karabaş"),
            ::kompoz::runtime::kt::list_of([Hound("a")]),
        ))
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn creatures() -> Lookup<::std::rc::Rc<dyn CreatureOpen>> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Hound(r)), {
        (hounds()).__kz_out(&|__kz_x: Hound| -> ::std::rc::Rc<dyn CreatureOpen> {
            {
                let __kz_shared2: ::std::rc::Rc<dyn CreatureOpen> = ::std::rc::Rc::new(__kz_x);
                __kz_shared2
            }
        })
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn satchel() -> Satchel<::std::rc::Rc<dyn CreatureOpen>> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Hound(r), Satchel(rr)), {
        (Satchel(Hound("x"), ::core::option::Option::None)).__kz_out(
            &|__kz_x: Hound| -> ::std::rc::Rc<dyn CreatureOpen> {
                {
                    let __kz_shared3: ::std::rc::Rc<dyn CreatureOpen> = ::std::rc::Rc::new(__kz_x);
                    __kz_shared3
                }
            },
        )
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn describe_lookup(o: Lookup<::std::rc::Rc<dyn CreatureOpen>>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_w4 = o.clone();
            match &__kz_w4 {
                Lookup::Found(__kz_case5) => {
                    #[allow(unused_variables)]
                    let o = ::core::clone::Clone::clone(__kz_case5);
                    { ::kompoz::runtime::kt::template!("bulundu ", o.value().name(), " ", o.all().size()) }
                }
                Lookup::Pairs(__kz_case6) => {
                    #[allow(unused_variables)]
                    let o = ::core::clone::Clone::clone(__kz_case6);
                    { ::std::string::String::from("çift") }
                }
                Lookup::Missing(_) => ::std::string::String::from("yok"),
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn lookups() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Hound(r)), {
        ::kompoz::runtime::kt::template!(
            describe_lookup((hounds()).__kz_out(&|__kz_x: Hound| -> ::std::rc::Rc<dyn CreatureOpen> {
                {
                    let __kz_shared8: ::std::rc::Rc<dyn CreatureOpen> = ::std::rc::Rc::new(__kz_x);
                    __kz_shared8
                }
            })),
            " ",
            describe_lookup(::core::convert::Into::<Lookup<::std::rc::Rc<dyn CreatureOpen>>>::into(LookupMissing {})),
            " ",
            satchel().item().name()
        )
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct KeepingBase<T> {
    first: ::kompoz::runtime::kt::KtVal<T>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T> KeepingBase<T> {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        KeepingBase { first: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait KeepingOpen<T>: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_keeping(&self) -> &KeepingBase<T>;
    fn first(&self) -> T
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        self.__kz_keeping().first.get()
    }
    #[doc(hidden)]
    fn __kz_keeping_init(&self, first: T)
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_keeping().first.init(::core::clone::Clone::clone(&first));
        })
    }
    fn __kz_keeping_open_first_or(&self) -> T
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ self.first() })
    }
    fn first_or(&self) -> T
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        self.__kz_keeping_open_first_or()
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub enum Keeping<T> {
    Value(KeepingValue<T>),
    Both(KeepingBoth<T>),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for Keeping<T>
{
    fn eq(&self, other: &Self) -> bool {
        #[allow(unreachable_patterns)]
        match (self, other) {
            (Keeping::Value(a), Keeping::Value(b)) => a == b,
            (Keeping::Both(a), Keeping::Both(b)) => a == b,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<KeepingValue<T>> for Keeping<T>
{
    fn from(value: KeepingValue<T>) -> Keeping<T> {
        Keeping::Value(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<KeepingValue<T>> for Keeping<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<KeepingValue<T>> {
        match self {
            Keeping::Value(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<KeepingValue<T>> for Keeping<T>
{
    fn eq(&self, other: &KeepingValue<T>) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Keeping::Value(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Keeping<T>> for KeepingValue<T>
{
    fn eq(&self, other: &Keeping<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<KeepingBoth<T>> for Keeping<T>
{
    fn from(value: KeepingBoth<T>) -> Keeping<T> {
        Keeping::Both(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<KeepingBoth<T>> for Keeping<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<KeepingBoth<T>> {
        match self {
            Keeping::Both(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<KeepingBoth<T>> for Keeping<T>
{
    fn eq(&self, other: &KeepingBoth<T>) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Keeping::Both(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Keeping<T>> for KeepingBoth<T>
{
    fn eq(&self, other: &Keeping<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for Keeping<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Keeping::Value(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Keeping::Both(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for Keeping<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for Keeping<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingOpen<T>
    for Keeping<T>
{
    fn __kz_keeping(&self) -> &KeepingBase<T> {
        match self {
            Keeping::Value(v) => KeepingOpen::<T>::__kz_keeping(v),
            Keeping::Both(v) => KeepingOpen::<T>::__kz_keeping(v),
        }
    }
    fn first(&self) -> T {
        match self {
            Keeping::Value(v) => KeepingOpen::<T>::first(v),
            Keeping::Both(v) => KeepingOpen::<T>::first(v),
        }
    }
    fn __kz_keeping_open_first_or(&self) -> T {
        match self {
            Keeping::Value(v) => KeepingOpen::<T>::__kz_keeping_open_first_or(v),
            Keeping::Both(v) => KeepingOpen::<T>::__kz_keeping_open_first_or(v),
        }
    }
    fn first_or(&self) -> T {
        match self {
            Keeping::Value(v) => KeepingOpen::<T>::first_or(v),
            Keeping::Both(v) => KeepingOpen::<T>::first_or(v),
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Keeping::Value(v) => KeepingOpen::<T>::__kz_eq(v, other),
                Keeping::Both(v) => KeepingOpen::<T>::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = &to;
        match self {
            Keeping::Value(v) => KeepingOpen::<T>::__kz_cast(v, to),
            Keeping::Both(v) => KeepingOpen::<T>::__kz_cast(v, to),
        }
    }
    fn __kz_keeping_init(&self, _: T) {}
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct KeepingValue<T> {
    pub value: T,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<KeepingBase<T>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn KeepingValue<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>(
    value: T,
) -> KeepingValue<T> {
    KeepingValue::__kz_new(value)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingValue<T> {
    #[doc(hidden)]
    pub fn __kz_new(value: T) -> KeepingValue<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(KeepingValue(r)), {
            let __kz_v = KeepingValue {
                value: ::core::clone::Clone::clone(&value),
                __kz_super: ::std::rc::Rc::new(KeepingBase::__kz_empty()),
            };
            KeepingOpen::<T>::__kz_keeping_init(&__kz_v, value.clone());
            __kz_v
        })
    }
    pub fn value(&self) -> T {
        ::core::clone::Clone::clone(&self.value)
    }
    pub fn component1(&self) -> T {
        self.value()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingValue<T> {
    pub fn copy(&self, # [default (:: core :: clone :: Clone :: clone (& self . value))] value: T) -> KeepingValue<T> {
        KeepingValue::__kz_new(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for KeepingValue<T>
{
    fn eq(&self, other: &Self) -> bool {
        true && self.value == other.value
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::hash::Hash
    for KeepingValue<T>
where
    T: ::core::hash::Hash,
{
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.value, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::cmp::Eq
    for KeepingValue<T>
where
    T: ::core::cmp::Eq,
{
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for KeepingValue<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Value(");
        __kz_out.push_str("value=");
        (&::kompoz::runtime::kt::KtProbe(&self.value)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for KeepingValue<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for KeepingValue<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingOpen<T>
    for KeepingValue<T>
{
    fn __kz_keeping(&self) -> &KeepingBase<T> {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Keeping<T>>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Keeping<T> as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzKeepingBoth<T> {
    #[doc(hidden)]
    pub __kz_super: KeepingBase<T>,
    a: ::kompoz::runtime::kt::KtVal<T>,
    b: ::kompoz::runtime::kt::KtVal<T>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct KeepingBoth<T> {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzKeepingBoth<T>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn KeepingBoth<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>(
    a: T,
    b: T,
) -> KeepingBoth<T> {
    KeepingBoth::__kz_new(a, b)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingBoth<T> {
    #[doc(hidden)]
    pub fn __kz_new(a: T, b: T) -> KeepingBoth<T> {
        let __kz_this = KeepingBoth {
            __kz_this: ::std::rc::Rc::new(__KzKeepingBoth {
                __kz_super: KeepingBase::__kz_empty(),
                a: ::kompoz::runtime::kt::KtVal::new(),
                b: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(a, b);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, a: T, b: T) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        KeepingOpen::<T>::__kz_keeping_init(self, a.clone());
        self.__kz_this.a.init(::core::clone::Clone::clone(&a));
        self.__kz_this.b.init(::core::clone::Clone::clone(&b));
        ::kompoz::runtime::named_args!({})
    }
    pub fn a(&self) -> T {
        self.__kz_this.a.get()
    }
    pub fn b(&self) -> T {
        self.__kz_this.b.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingBoth<T> {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for KeepingBoth<T>
{
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::cmp::Eq
    for KeepingBoth<T>
{
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::hash::Hash
    for KeepingBoth<T>
{
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for KeepingBoth<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Both",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for KeepingBoth<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for KeepingBoth<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> KeepingOpen<T>
    for KeepingBoth<T>
{
    fn __kz_keeping(&self) -> &KeepingBase<T> {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Keeping<T>>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Keeping<T> as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn keepings() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(KeepingValue(r), KeepingBoth(rr)), {
        let k: Keeping<i32> = ::core::convert::Into::<Keeping<i32>>::into(KeepingValue(3));
        let p: Keeping<::std::string::String> = ::core::convert::Into::<Keeping<::std::string::String>>::into(
            KeepingBoth(::std::string::String::from("a"), ::std::string::String::from("b")),
        );
        return ::kompoz::runtime::kt::template!(
            "",
            k.first(),
            " ",
            k.first_or(),
            " ",
            p.first(),
            " ",
            k,
            " ",
            (k == KeepingValue(3))
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct PolygonBase {
    name: ::kompoz::runtime::kt::KtVal<::std::string::String>,
    note: ::kompoz::runtime::kt::KtVar<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        PolygonBase { name: ::kompoz::runtime::kt::KtVal::new(), note: ::kompoz::runtime::kt::KtVar::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait PolygonOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_polygon(&self) -> &PolygonBase;
    fn name(&self) -> ::std::string::String {
        self.__kz_polygon().name.get()
    }
    fn note(&self) -> ::std::string::String {
        self.__kz_polygon().note.get()
    }
    fn set_note(&self, value: ::std::string::String) {
        self.__kz_polygon().note.set(value)
    }
    #[doc(hidden)]
    fn __kz_polygon_init(&self, name: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_polygon().name.init(::core::clone::Clone::clone(&name));
            self.__kz_polygon().note.set(::std::string::String::from("birincil"));
        })
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub enum Polygon {
    Regular(PolygonRegular),
    Named(PolygonNamed),
    Plain(PolygonPlain),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<PolygonRegular> for Polygon {
    fn from(value: PolygonRegular) -> Polygon {
        Polygon::Regular(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<PolygonRegular> for Polygon {
    fn kt_as_safe(&self) -> ::core::option::Option<PolygonRegular> {
        match self {
            Polygon::Regular(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<PolygonRegular> for Polygon {
    fn eq(&self, other: &PolygonRegular) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Polygon::Regular(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Polygon> for PolygonRegular {
    fn eq(&self, other: &Polygon) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<PolygonNamed> for Polygon {
    fn from(value: PolygonNamed) -> Polygon {
        Polygon::Named(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<PolygonNamed> for Polygon {
    fn kt_as_safe(&self) -> ::core::option::Option<PolygonNamed> {
        match self {
            Polygon::Named(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<PolygonNamed> for Polygon {
    fn eq(&self, other: &PolygonNamed) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Polygon::Named(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Polygon> for PolygonNamed {
    fn eq(&self, other: &Polygon) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<PolygonPlain> for Polygon {
    fn from(value: PolygonPlain) -> Polygon {
        Polygon::Plain(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<PolygonPlain> for Polygon {
    fn kt_as_safe(&self) -> ::core::option::Option<PolygonPlain> {
        match self {
            Polygon::Plain(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<PolygonPlain> for Polygon {
    fn eq(&self, other: &PolygonPlain) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Polygon::Plain(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Polygon> for PolygonPlain {
    fn eq(&self, other: &Polygon) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Polygon {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Polygon::Regular(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Polygon::Named(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Polygon::Plain(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Polygon {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Polygon {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonOpen for Polygon {
    fn __kz_polygon(&self) -> &PolygonBase {
        match self {
            Polygon::Regular(v) => PolygonOpen::__kz_polygon(v),
            Polygon::Named(v) => PolygonOpen::__kz_polygon(v),
            Polygon::Plain(v) => PolygonOpen::__kz_polygon(v),
        }
    }
    fn name(&self) -> ::std::string::String {
        match self {
            Polygon::Regular(v) => PolygonOpen::name(v),
            Polygon::Named(v) => PolygonOpen::name(v),
            Polygon::Plain(v) => PolygonOpen::name(v),
        }
    }
    fn note(&self) -> ::std::string::String {
        match self {
            Polygon::Regular(v) => PolygonOpen::note(v),
            Polygon::Named(v) => PolygonOpen::note(v),
            Polygon::Plain(v) => PolygonOpen::note(v),
        }
    }
    fn set_note(&self, value: ::std::string::String) {
        match self {
            Polygon::Regular(v) => PolygonOpen::set_note(v, value),
            Polygon::Named(v) => PolygonOpen::set_note(v, value),
            Polygon::Plain(v) => PolygonOpen::set_note(v, value),
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Polygon::Regular(v) => PolygonOpen::__kz_eq(v, other),
                Polygon::Named(v) => PolygonOpen::__kz_eq(v, other),
                Polygon::Plain(v) => PolygonOpen::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = &to;
        match self {
            Polygon::Regular(v) => PolygonOpen::__kz_cast(v, to),
            Polygon::Named(v) => PolygonOpen::__kz_cast(v, to),
            Polygon::Plain(v) => PolygonOpen::__kz_cast(v, to),
        }
    }
    fn __kz_polygon_init(&self, _: ::std::string::String) {}
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct PolygonRegular {
    pub n: i32,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<PolygonBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn PolygonRegular(n: i32) -> PolygonRegular {
    PolygonRegular::__kz_new(n)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonRegular {
    #[doc(hidden)]
    pub fn __kz_new(n: i32) -> PolygonRegular {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(PolygonRegular(r)), {
            let __kz_v = PolygonRegular {
                n: ::core::clone::Clone::clone(&n),
                __kz_super: ::std::rc::Rc::new(PolygonBase::__kz_empty()),
            };
            {
                let sides: i32 = n;
                PolygonOpen::__kz_polygon_init(
                    &__kz_v,
                    ::std::string::String::from(::kompoz::runtime::kt::template!("kenar ", sides)),
                );
                __kz_v.set_note(::kompoz::runtime::kt::template!("çokgen ", sides));
            }
            __kz_v
        })
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
impl PolygonRegular {
    pub fn copy(&self, # [default (self . n)] n: i32) -> PolygonRegular {
        PolygonRegular::__kz_new(n)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for PolygonRegular {
    fn eq(&self, other: &Self) -> bool {
        true && self.n == other.n
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for PolygonRegular {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.n, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for PolygonRegular {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for PolygonRegular {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Regular(");
        __kz_out.push_str("n=");
        (&::kompoz::runtime::kt::KtProbe(&self.n)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for PolygonRegular {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for PolygonRegular {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonOpen for PolygonRegular {
    fn __kz_polygon(&self) -> &PolygonBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Polygon>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Polygon as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct PolygonNamed {
    pub label: ::std::string::String,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<PolygonBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn PolygonNamed(label: impl ::core::convert::Into<::std::string::String>) -> PolygonNamed {
    let label: ::std::string::String = ::core::convert::Into::into(label);
    PolygonNamed::__kz_new(label)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonNamed {
    #[doc(hidden)]
    pub fn __kz_new(label: ::std::string::String) -> PolygonNamed {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(PolygonNamed(r)), {
            let __kz_v = PolygonNamed {
                label: ::core::clone::Clone::clone(&label),
                __kz_super: ::std::rc::Rc::new(PolygonBase::__kz_empty()),
            };
            PolygonOpen::__kz_polygon_init(&__kz_v, ::std::string::String::from(label.clone()));
            __kz_v
        })
    }
    pub fn label(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.label)
    }
    pub fn component1(&self) -> ::std::string::String {
        self.label()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl PolygonNamed {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . label))] label: ::std::string::String,
    ) -> PolygonNamed {
        PolygonNamed::__kz_new(label)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for PolygonNamed {
    fn eq(&self, other: &Self) -> bool {
        true && self.label == other.label
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for PolygonNamed {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.label, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for PolygonNamed {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for PolygonNamed {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Named(");
        __kz_out.push_str("label=");
        (&::kompoz::runtime::kt::KtProbe(&self.label)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for PolygonNamed {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for PolygonNamed {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonOpen for PolygonNamed {
    fn __kz_polygon(&self) -> &PolygonBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Polygon>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Polygon as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzPolygonPlain {
    #[doc(hidden)]
    pub __kz_super: PolygonBase,
    k: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct PolygonPlain {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzPolygonPlain>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn PolygonPlain(k: i32) -> PolygonPlain {
    PolygonPlain::__kz_new(k)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonPlain {
    #[doc(hidden)]
    pub fn __kz_new(k: i32) -> PolygonPlain {
        let __kz_this = PolygonPlain {
            __kz_this: ::std::rc::Rc::new(__KzPolygonPlain {
                __kz_super: PolygonBase::__kz_empty(),
                k: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(k);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, k: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        {
            let sides: i32 = k;
            PolygonOpen::__kz_polygon_init(
                self,
                ::std::string::String::from(::kompoz::runtime::kt::template!("kenar ", sides)),
            );
            self.set_note(::kompoz::runtime::kt::template!("çokgen ", sides));
        }
        self.__kz_this.k.init(k);
        ::kompoz::runtime::named_args!({})
    }
    pub fn k(&self) -> i32 {
        self.__kz_this.k.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl PolygonPlain {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for PolygonPlain {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for PolygonPlain {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for PolygonPlain {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for PolygonPlain {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Plain",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for PolygonPlain {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for PolygonPlain {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PolygonOpen for PolygonPlain {
    fn __kz_polygon(&self) -> &PolygonBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Polygon>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Polygon as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn polygons() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(PolygonRegular(r), PolygonNamed(r), PolygonPlain(r)), {
        ::kompoz::runtime::kt::template!(
            PolygonRegular(3).note(),
            " ",
            PolygonRegular(4).name(),
            " ",
            PolygonNamed("x").note(),
            " ",
            PolygonPlain(5).note()
        )
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct QuietBase {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl QuietBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        QuietBase {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait QuietOpen<T>: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_quiet(&self) -> &QuietBase;
    #[doc(hidden)]
    fn __kz_quiet_init(&self)
    where
        T: ::core::clone::Clone + 'static,
    {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub enum Quiet<T> {
    Off(QuietOff),
    Note(QuietNote),
    #[doc(hidden)]
    __KzOf(::core::marker::PhantomData<(T,)>, ::core::convert::Infallible),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::cmp::PartialEq for Quiet<T> {
    fn eq(&self, other: &Self) -> bool {
        #[allow(unreachable_patterns)]
        match (self, other) {
            (Quiet::Off(a), Quiet::Off(b)) => a == b,
            (Quiet::Note(a), Quiet::Note(b)) => a == b,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::convert::From<QuietOff> for Quiet<T> {
    fn from(value: QuietOff) -> Quiet<T> {
        Quiet::Off(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::kompoz::runtime::kt::KtAs<QuietOff> for Quiet<T> {
    fn kt_as_safe(&self) -> ::core::option::Option<QuietOff> {
        match self {
            Quiet::Off(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::cmp::PartialEq<QuietOff> for Quiet<T> {
    fn eq(&self, other: &QuietOff) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Quiet::Off(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::cmp::PartialEq<Quiet<T>> for QuietOff {
    fn eq(&self, other: &Quiet<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::convert::From<QuietNote> for Quiet<T> {
    fn from(value: QuietNote) -> Quiet<T> {
        Quiet::Note(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::kompoz::runtime::kt::KtAs<QuietNote> for Quiet<T> {
    fn kt_as_safe(&self) -> ::core::option::Option<QuietNote> {
        match self {
            Quiet::Note(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::cmp::PartialEq<QuietNote> for Quiet<T> {
    fn eq(&self, other: &QuietNote) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Quiet::Note(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::cmp::PartialEq<Quiet<T>> for QuietNote {
    fn eq(&self, other: &Quiet<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> Quiet<T> {
    #[doc(hidden)]
    pub fn __kz_out<__KzU0: ::core::clone::Clone + 'static>(
        self,
        __kz_f0: &dyn ::core::ops::Fn(T) -> __KzU0,
    ) -> Quiet<__KzU0> {
        match self {
            Quiet::Off(c) => Quiet::Off(c),
            Quiet::Note(c) => Quiet::Note(c),
            Quiet::__KzOf(_, never) => match never {},
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::kompoz::runtime::kt::KtToString for Quiet<T> {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Quiet::Off(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Quiet::Note(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Quiet::__KzOf(_, never) => match *never {},
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::fmt::Display for Quiet<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::fmt::Debug for Quiet<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> QuietOpen<T> for Quiet<T> {
    fn __kz_quiet(&self) -> &QuietBase {
        match self {
            Quiet::Off(v) => QuietOpen::<T>::__kz_quiet(v),
            Quiet::Note(v) => QuietOpen::<T>::__kz_quiet(v),
            Quiet::__KzOf(_, never) => match *never {},
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Quiet::Off(v) => QuietOpen::<T>::__kz_eq(v, other),
                Quiet::Note(v) => QuietOpen::<T>::__kz_eq(v, other),
                Quiet::__KzOf(_, never) => match *never {},
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = &to;
        match self {
            Quiet::Off(v) => QuietOpen::<T>::__kz_cast(v, to),
            Quiet::Note(v) => QuietOpen::<T>::__kz_cast(v, to),
            Quiet::__KzOf(_, never) => match *never {},
        }
    }
    fn __kz_quiet_init(&self) {}
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzQuietOff {
    #[doc(hidden)]
    pub __kz_super: QuietBase,
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
pub struct QuietOff {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl QuietOff {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzQuietOff {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzQuietOff > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzQuietOff { __kz_super: QuietBase::__kz_empty() }, |_| QuietOff {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            QuietOpen::<::kompoz::runtime::kt::KtNothing>::__kz_quiet_init(self);
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl QuietOff {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for QuietOff {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Quiet.Off",
            Self::__kz_state() as *const __KzQuietOff as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for QuietOff {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for QuietOff {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzN0: ::core::clone::Clone + 'static> QuietOpen<__KzN0> for QuietOff {
    fn __kz_quiet(&self) -> &QuietBase {
        &Self::__kz_state().__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct QuietNote {
    pub text: ::std::string::String,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<QuietBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn QuietNote(text: impl ::core::convert::Into<::std::string::String>) -> QuietNote {
    let text: ::std::string::String = ::core::convert::Into::into(text);
    QuietNote::__kz_new(text)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl QuietNote {
    #[doc(hidden)]
    pub fn __kz_new(text: ::std::string::String) -> QuietNote {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(QuietNote(r)), {
            let __kz_v = QuietNote {
                text: ::core::clone::Clone::clone(&text),
                __kz_super: ::std::rc::Rc::new(QuietBase::__kz_empty()),
            };
            QuietOpen::<::kompoz::runtime::kt::KtNothing>::__kz_quiet_init(&__kz_v);
            __kz_v
        })
    }
    pub fn text(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.text)
    }
    pub fn component1(&self) -> ::std::string::String {
        self.text()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl QuietNote {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . text))] text: ::std::string::String,
    ) -> QuietNote {
        QuietNote::__kz_new(text)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for QuietNote {
    fn eq(&self, other: &Self) -> bool {
        true && self.text == other.text
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for QuietNote {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.text, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for QuietNote {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for QuietNote {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Note(");
        __kz_out.push_str("text=");
        (&::kompoz::runtime::kt::KtProbe(&self.text)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for QuietNote {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for QuietNote {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzN0: ::core::clone::Clone + 'static> QuietOpen<__KzN0> for QuietNote {
    fn __kz_quiet(&self) -> &QuietBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn quiets(q: Quiet<i32>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_w9 = q.clone();
            match &__kz_w9 {
                Quiet::Off(_) => ::std::string::String::from("kapalı"),
                Quiet::Note(__kz_case11) => {
                    #[allow(unused_variables)]
                    let q = ::core::clone::Clone::clone(__kz_case11);
                    { ::kompoz::runtime::kt::template!("not ", q.text()) }
                }
                _ => ::core::unreachable!("no `when` branch matched"),
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn quiet_line() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(QuietNote(r)), {
        let mut all: ::std::vec::Vec<Quiet<i32>> = ::kompoz::runtime::kt::list_of([
            ::core::convert::Into::<Quiet<i32>>::into(QuietOff {}),
            ::core::convert::Into::<Quiet<i32>>::into(QuietNote("a")),
        ]);
        return ::kompoz::runtime::kt::template!(
            ::kompoz::runtime::kt::to_string(&::kompoz::runtime::kt::KtList::map(&all, |it| { quiets(it.clone()) })),
            " ",
            (::kompoz::runtime::kt::KtList::get(&all, 1) == QuietNote("a")),
            " ",
            ::kompoz::runtime::kt::KtList::get(&all, 1)
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct TockerBase {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TockerBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        TockerBase {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait TockerOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_tocker(&self) -> &TockerBase;
    #[doc(hidden)]
    fn __kz_tocker_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    fn next(&self) -> i32;
    fn __kz_tocker_open_twice(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(self.next(), " ", self.next()) })
    }
    fn twice(&self) -> ::std::string::String {
        self.__kz_tocker_open_twice()
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn TockerOpen {
    fn eq(&self, other: &Self) -> bool {
        TockerOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn TockerOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        TockerOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn TockerOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn TockerOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn TockerOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            TockerOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn tocker(start: i32) -> ::std::rc::Rc<dyn TockerOpen> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let n = ::kompoz::runtime::kt::Ref::new(start);
            let bump: ::std::rc::Rc<dyn ::core::ops::Fn() -> i32> = ::std::rc::Rc::new({
                let n = ::kompoz::runtime::__private::capture(&n);
                move || -> i32 {
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::post_inc(&n) })
                }
            });
            {
                #[allow(non_camel_case_types, clippy::type_complexity)]
                struct __KzObject {
                    #[doc(hidden)]
                    pub __kz_super: TockerBase,
                    m0: ::std::rc::Rc<dyn ::core::ops::Fn(&__KzObject) -> i32>,
                }
                #[allow(non_snake_case, clippy::all)]
                impl TockerOpen for __KzObject {
                    fn __kz_tocker(&self) -> &TockerBase {
                        &self.__kz_super
                    }
                    fn next(&self) -> i32 {
                        (self.m0)(self)
                    }
                }
                let __kz_o = __KzObject {
                    __kz_super: TockerBase::__kz_empty(),
                    m0: ::std::rc::Rc::new({
                        let bump = ::core::clone::Clone::clone(&bump);
                        move |__kz_this: &__KzObject| -> i32 {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = ();
                            ::kompoz::runtime::named_args!({ bump() })
                        }
                    }),
                };
                TockerOpen::__kz_tocker_init(&__kz_o);
                let __kz_object12: ::std::rc::Rc<dyn TockerOpen> = ::std::rc::Rc::new(__kz_o);
                __kz_object12
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Drip: ::kompoz::runtime::kt::KtObject {
    fn next(&self) -> i32;
    fn both(&self) -> ::std::string::String;
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn Drip {
    fn eq(&self, other: &Self) -> bool {
        Drip::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Drip {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Drip::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Drip {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Drip {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Drip {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Drip::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn drip(start: i32) -> ::std::rc::Rc<dyn Drip> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let last: ::kompoz::runtime::kt::KtLateinit<::std::string::String> =
                ::kompoz::runtime::kt::KtLateinit::new("last");
            let n = ::kompoz::runtime::kt::Ref::new(start);
            let next: ::std::rc::Rc<dyn ::core::ops::Fn() -> i32> = ::std::rc::Rc::new({
                let n = ::kompoz::runtime::__private::capture(&n);
                let last = ::kompoz::runtime::__private::capture(&last);
                move || -> i32 {
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    ::kompoz::runtime::named_args!({
                        ::kompoz::runtime::kt::post_inc(&n);
                        ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                            &last,
                            ::kompoz::runtime::kt::template!("n", ::kompoz::runtime::kt::KtDelegate::get_value(&n)),
                        );
                        return ::kompoz::runtime::kt::KtDelegate::get_value(&n);
                    })
                }
            });
            {
                #[allow(non_camel_case_types)]
                struct __KzObject<F0, F1> {
                    m0: F0,
                    m1: F1,
                }
                impl<F0: ::core::ops::Fn() -> i32 + 'static, F1: ::core::ops::Fn() -> ::std::string::String + 'static>
                    Drip for __KzObject<F0, F1>
                {
                    fn next(&self) -> i32 {
                        (self.m0)()
                    }
                    fn both(&self) -> ::std::string::String {
                        (self.m1)()
                    }
                }
                impl<F0, F1> ::core::cmp::PartialEq for __KzObject<F0, F1> {
                    fn eq(&self, other: &Self) -> bool {
                        ::core::ptr::eq(self, other)
                    }
                }
                impl<F0, F1> ::core::fmt::Debug for __KzObject<F0, F1> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        ::core::write!(f, "<object>@{:p}", self)
                    }
                }
                let __kz_object13: ::std::rc::Rc<dyn Drip> = ::std::rc::Rc::new(__KzObject {
                    m0: {
                        let next = ::core::clone::Clone::clone(&next);
                        move || -> i32 {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = ();
                            ::kompoz::runtime::named_args!({ next() })
                        }
                    },
                    m1: {
                        let next = ::core::clone::Clone::clone(&next);
                        let last = ::core::clone::Clone::clone(&last);
                        move || -> ::std::string::String {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = ();
                            ::kompoz::runtime::named_args!({
                                ::kompoz::runtime::kt::template!(
                                    next(),
                                    " ",
                                    ::kompoz::runtime::kt::KtDelegate::get_value(&last)
                                )
                            })
                        }
                    },
                });
                __kz_object13
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn objects_line() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::template!(tocker(5).twice(), " ", drip(1).both(), " ", drip(7).both())
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Spot2 {
    pub x: i32,
    pub y: i32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Spot2(x: i32, y: i32) -> Spot2 {
    Spot2::__kz_new(x, y)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Spot2 {
    #[doc(hidden)]
    pub fn __kz_new(x: i32, y: i32) -> Spot2 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Spot2(rr)), { Spot2 { x, y } })
    }
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn y(&self) -> i32 {
        self.y
    }
    pub fn component1(&self) -> i32 {
        self.x()
    }
    pub fn component2(&self) -> i32 {
        self.y()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Spot2 {
    pub fn copy(&self, # [default (self . x)] x: i32, # [default (self . y)] y: i32) -> Spot2 {
        Spot2::__kz_new(x, y)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Spot2 {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.x, state);
        ::core::hash::Hash::hash(&self.y, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Spot2 {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Spot2 {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Spot2(");
        __kz_out.push_str("x=");
        (&::kompoz::runtime::kt::KtProbe(&self.x)).kt_write_probe(__kz_out);
        __kz_out.push_str(", y=");
        (&::kompoz::runtime::kt::KtProbe(&self.y)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Spot2 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Spot2 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn spot_type(sep: impl ::core::convert::Into<::std::string::String>) -> NavType {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let sep: ::std::string::String = ::core::convert::Into::into(sep);
    ::kompoz::runtime::named_args!(types(Spot2(rr)), {
        {
            let parse_value: ::std::rc::Rc<dyn ::core::ops::Fn(::std::string::String) -> Spot2> = ::std::rc::Rc::new({
                let sep = ::kompoz::runtime::__private::capture(&sep);
                move |value: ::std::string::String| -> Spot2 {
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    ::kompoz::runtime::named_args!(types(Spot2(rr)), {
                        Spot2(
                            ::kompoz::runtime::kt::KtStr::to_int(&::kompoz::runtime::kt::KtStr::substring_before(
                                &value,
                                sep.clone(),
                                ::core::option::Option::None,
                            )),
                            ::kompoz::runtime::kt::KtStr::to_int(&::kompoz::runtime::kt::KtStr::substring_after(
                                &value,
                                sep.clone(),
                                ::core::option::Option::None,
                            )),
                        )
                    })
                }
            });
            let serialize_as_value: ::std::rc::Rc<dyn ::core::ops::Fn(Spot2) -> ::std::string::String> =
                ::std::rc::Rc::new({
                    let sep = ::kompoz::runtime::__private::capture(&sep);
                    move |value: Spot2| -> ::std::string::String {
                        #[allow(unused_imports)]
                        use ::kompoz::runtime::kt::ext::*;
                        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(value.x(), sep, value.y()) })
                    }
                });
            {
                #[allow(non_camel_case_types, clippy::type_complexity)]
                struct __KzObject {
                    c0: bool,
                    m1: ::std::rc::Rc<dyn ::core::ops::Fn(SavedState, ::std::string::String, Spot2)>,
                    m2: ::std::rc::Rc<
                        dyn ::core::ops::Fn(SavedState, ::std::string::String) -> ::core::option::Option<Spot2>,
                    >,
                    m3: ::std::rc::Rc<dyn ::core::ops::Fn(::std::string::String) -> Spot2>,
                    m4: ::std::rc::Rc<dyn ::core::ops::Fn(Spot2) -> ::std::string::String>,
                }
                #[allow(non_snake_case, clippy::all)]
                impl NavTypeImpl for __KzObject {
                    fn is_nullable_allowed(&self) -> bool {
                        ::core::clone::Clone::clone(&self.c0)
                    }
                    type Value = Spot2;
                    fn put(&self, bundle: &SavedState, key: &str, value: ::core::option::Option<&Spot2>) {
                        (self.m1)(
                            ::core::clone::Clone::clone(bundle),
                            ::std::string::String::from(key),
                            ::kompoz::runtime::kt::not_null(::core::option::Option::map(
                                value,
                                ::core::clone::Clone::clone,
                            )),
                        )
                    }
                    fn get(&self, bundle: &SavedState, key: &str) -> ::core::option::Option<Spot2> {
                        (self.m2)(::core::clone::Clone::clone(bundle), ::std::string::String::from(key))
                    }
                    fn parse_value(
                        &self,
                        value: &str,
                    ) -> ::core::result::Result<::core::option::Option<Spot2>, NavTypeError> {
                        ::core::result::Result::Ok(::core::option::Option::Some((self.m3)(
                            ::std::string::String::from(value),
                        )))
                    }
                    fn serialize_as_value(&self, value: ::core::option::Option<&Spot2>) -> ::std::string::String {
                        (self.m4)(::kompoz::runtime::kt::not_null(::core::option::Option::map(
                            value,
                            ::core::clone::Clone::clone,
                        )))
                    }
                }
                NavType::new(__KzObject {
                    c0: false,
                    m1: ::std::rc::Rc::new({
                        let serialize_as_value = ::core::clone::Clone::clone(&serialize_as_value);
                        move |bundle: SavedState, key: ::std::string::String, value: Spot2| {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = (&bundle, &key, &value);
                            ::kompoz::runtime::named_args!({
                                bundle.write(|this| this.put_string(&key.clone(), serialize_as_value(value.clone())));
                            })
                        }
                    }),
                    m2: ::std::rc::Rc::new({
                        let parse_value = ::core::clone::Clone::clone(&parse_value);
                        move |bundle: SavedState, key: ::std::string::String| -> ::core::option::Option<Spot2> {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = (&bundle, &key);
                            ::kompoz::runtime::named_args!({
                                ::core::option::Option::Some(parse_value(
                                    bundle.read(|this| this.get_string(&key.clone())),
                                ))
                            })
                        }
                    }),
                    m3: ::std::rc::Rc::new({
                        let parse_value = ::core::clone::Clone::clone(&parse_value);
                        move |value: ::std::string::String| -> Spot2 {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = (&value,);
                            ::kompoz::runtime::named_args!({ parse_value(value.clone()) })
                        }
                    }),
                    m4: ::std::rc::Rc::new({
                        let serialize_as_value = ::core::clone::Clone::clone(&serialize_as_value);
                        move |value: Spot2| -> ::std::string::String {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = (&value,);
                            ::kompoz::runtime::named_args!({ serialize_as_value(value.clone()) })
                        }
                    }),
                })
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzBackLog {
    log: ::kompoz::runtime::kt::KtVar<::std::string::String>,
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
pub struct BackLog {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl BackLog {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzBackLog {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzBackLog > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE
            .with(|s| s.get(|| __KzBackLog { log: ::kompoz::runtime::kt::KtVar::new() }, |_| BackLog {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().log.set(::std::string::String::from(""));
        })
    }
    pub fn log(&self) -> ::std::string::String {
        Self::__kz_state().log.get()
    }
    pub fn set_log(&self, value: ::std::string::String) {
        Self::__kz_state().log.set(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl BackLog {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for BackLog {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "BackLog",
            Self::__kz_state() as *const __KzBackLog as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for BackLog {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for BackLog {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<BackLog> for NavigationEventHandler<NavigationEventInfoNone> {
    fn from(o: BackLog) -> NavigationEventHandler<NavigationEventInfoNone> {
        ::std::thread_local! { static __KZ_ONE : :: std :: cell :: OnceCell < NavigationEventHandler < NavigationEventInfoNone > > = const { :: std :: cell :: OnceCell :: new () } ; }
        __KZ_ONE.with(|one| {
            ::core::clone::Clone::clone(
                one.get_or_init(|| NavigationEventHandler::new(NavigationEventInfoNone, true, o)),
            )
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl NavigationEventHandlerCallbacks<NavigationEventInfoNone> for BackLog {
    fn on_back_started(&self, handler: &NavigationEventHandler<NavigationEventInfoNone>, event: NavigationEvent) {
        ({
            move |__kz_this: &NavigationEventHandler<NavigationEventInfoNone>, event: NavigationEvent| {
                #[allow(unused_imports)]
                use ::kompoz::runtime::kt::ext::*;
                let _ = (&event,);
                ::kompoz::runtime::named_args!({
                    self.set_log(::kompoz::runtime::kt::template!(self.log(), "başla "));
                })
            }
        })(handler, event)
    }
    fn on_back_completed(&self, handler: &NavigationEventHandler<NavigationEventInfoNone>) {
        ({
            move |__kz_this: &NavigationEventHandler<NavigationEventInfoNone>| {
                #[allow(unused_imports)]
                use ::kompoz::runtime::kt::ext::*;
                let _ = ();
                ::kompoz::runtime::named_args!({
                    self.set_log(::kompoz::runtime::kt::template!(
                        self.log(),
                        ::kompoz::runtime::kt::template!(
                            "bitti ",
                            __kz_this.is_back_enabled(),
                            " ",
                            (__kz_this.transition_state() == NavigationEventTransitionState::Idle)
                        )
                    ));
                    __kz_this.set_is_back_enabled(false);
                })
            }
        })(handler)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn back_log() -> NavigationEventHandler<NavigationEventInfoNone> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        <NavigationEventHandler<NavigationEventInfoNone> as ::core::convert::From<BackLog>>::from(BackLog {})
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn back_log_text() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ BackLog {}.log() })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn same_handler() -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ back_log() == back_log() })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct WrapperBase {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrapperBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        WrapperBase {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait WrapperOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_wrapper(&self) -> &WrapperBase;
    #[doc(hidden)]
    fn __kz_wrapper_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    fn __kz_wrapper_open_both(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::template!("", self.id(1), self.wrap(::std::string::String::from("a")))
        })
    }
    fn both(&self) -> ::std::string::String {
        self.__kz_wrapper_open_both()
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait WrapperOpenExt: WrapperOpen {
    fn id<T: ::core::clone::Clone + 'static>(&self, x: T) -> T {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ x.clone() })
    }
    fn wrap<T: ::core::clone::Clone + 'static>(&self, x: T) -> ::std::vec::Vec<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        if let ::core::option::Option::Some(__kz_o) =
            ::kompoz::runtime::kt::KtObject::as_any(self).downcast_ref::<TwinWrapper>()
        {
            return TwinWrapper::wrap::<T>(__kz_o, x);
        }
        if let ::core::option::Option::Some(__kz_o) =
            ::kompoz::runtime::kt::KtObject::as_any(self).downcast_ref::<WrapperObject>()
        {
            return WrapperObject::wrap::<T>(__kz_o, x);
        }
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::list_of([x.clone()]) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzS: WrapperOpen + ?Sized> WrapperOpenExt for __KzS {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn WrapperOpen {
    fn eq(&self, other: &Self) -> bool {
        WrapperOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn WrapperOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        WrapperOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn WrapperOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn WrapperOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn WrapperOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            WrapperOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzWrapper {
    #[doc(hidden)]
    pub __kz_super: WrapperBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Wrapper {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzWrapper>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Wrapper() -> Wrapper {
    Wrapper::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Wrapper {
    #[doc(hidden)]
    pub fn __kz_new() -> Wrapper {
        let __kz_this =
            Wrapper { __kz_this: ::std::rc::Rc::new(__KzWrapper { __kz_super: WrapperBase::__kz_empty() }) };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        WrapperOpen::__kz_wrapper_init(self);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Wrapper {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Wrapper {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Wrapper {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Wrapper {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Wrapper {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Wrapper",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Wrapper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Wrapper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrapperOpen for Wrapper {
    fn __kz_wrapper(&self) -> &WrapperBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn WrapperOpen>>() {
            let o: ::std::rc::Rc<dyn WrapperOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Wrapper> for ::std::rc::Rc<dyn WrapperOpen> {
    fn eq(&self, other: &Wrapper) -> bool {
        WrapperOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn WrapperOpen>> for Wrapper {
    fn eq(&self, other: &::std::rc::Rc<dyn WrapperOpen>) -> bool {
        WrapperOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn WrapperOpen>> for Wrapper {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn WrapperOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzTwinWrapper {
    #[doc(hidden)]
    pub __kz_super: WrapperBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct TwinWrapper {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzTwinWrapper>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn TwinWrapper() -> TwinWrapper {
    TwinWrapper::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl TwinWrapper {
    #[doc(hidden)]
    pub fn __kz_new() -> TwinWrapper {
        let __kz_this =
            TwinWrapper { __kz_this: ::std::rc::Rc::new(__KzTwinWrapper { __kz_super: WrapperBase::__kz_empty() }) };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        WrapperOpen::__kz_wrapper_init(self);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl TwinWrapper {
    pub fn wrap<T: ::core::clone::Clone + 'static>(&self, x: T) -> ::std::vec::Vec<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&x,);
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::list_of([x.clone(), x.clone()]) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for TwinWrapper {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for TwinWrapper {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for TwinWrapper {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for TwinWrapper {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "TwinWrapper",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for TwinWrapper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for TwinWrapper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrapperOpen for TwinWrapper {
    fn __kz_wrapper(&self) -> &WrapperBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn WrapperOpen>>() {
            let o: ::std::rc::Rc<dyn WrapperOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<TwinWrapper> for ::std::rc::Rc<dyn WrapperOpen> {
    fn eq(&self, other: &TwinWrapper) -> bool {
        WrapperOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn WrapperOpen>> for TwinWrapper {
    fn eq(&self, other: &::std::rc::Rc<dyn WrapperOpen>) -> bool {
        WrapperOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn WrapperOpen>> for TwinWrapper {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn WrapperOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn wrapper() -> ::std::rc::Rc<dyn WrapperOpen> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(WrapperObject()), {
        {
            let __kz_shared14: ::std::rc::Rc<dyn WrapperOpen> = ::std::rc::Rc::new(WrapperObject());
            __kz_shared14
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzWrapperObject {
    #[doc(hidden)]
    pub __kz_super: WrapperBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct WrapperObject {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzWrapperObject>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn WrapperObject() -> WrapperObject {
    WrapperObject::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrapperObject {
    #[doc(hidden)]
    pub(crate) fn __kz_new() -> WrapperObject {
        let __kz_this = WrapperObject {
            __kz_this: ::std::rc::Rc::new(__KzWrapperObject { __kz_super: WrapperBase::__kz_empty() }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        WrapperOpen::__kz_wrapper_init(self);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl WrapperObject {
    pub fn wrap<T: ::core::clone::Clone + 'static>(&self, x: T) -> ::std::vec::Vec<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&x,);
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::list_of([x.clone(), x.clone(), x.clone()]) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for WrapperObject {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for WrapperObject {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for WrapperObject {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for WrapperObject {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Object",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for WrapperObject {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for WrapperObject {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WrapperOpen for WrapperObject {
    fn __kz_wrapper(&self) -> &WrapperBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn WrapperOpen>>() {
            let o: ::std::rc::Rc<dyn WrapperOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<WrapperObject> for ::std::rc::Rc<dyn WrapperOpen> {
    fn eq(&self, other: &WrapperObject) -> bool {
        WrapperOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn WrapperOpen>> for WrapperObject {
    fn eq(&self, other: &::std::rc::Rc<dyn WrapperOpen>) -> bool {
        WrapperOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn WrapperOpen>> for WrapperObject {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn WrapperOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct DoublerBase {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DoublerBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        DoublerBase {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait DoublerOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_doubler(&self) -> &DoublerBase;
    #[doc(hidden)]
    fn __kz_doubler_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    fn __kz_doubler_open_twice(&self) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtList::size(&self.wrap(2)) })
    }
    fn twice(&self) -> i32 {
        self.__kz_doubler_open_twice()
    }
    #[doc(hidden)]
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
    }
    #[doc(hidden)]
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        let name = ::std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
    }
    #[doc(hidden)]
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        let _ = to;
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait DoublerOpenExt: DoublerOpen {
    fn wrap<T: ::core::clone::Clone + 'static>(&self, x: T) -> ::std::vec::Vec<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        if let ::core::option::Option::Some(__kz_o) =
            ::kompoz::runtime::kt::KtObject::as_any(self).downcast_ref::<DoublerObject>()
        {
            return DoublerObject::wrap::<T>(__kz_o, x);
        }
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::TODO(::core::option::Option::Some(&::std::string::String::from(
                "abstract function wrap called on an object that doesn't override it",
            )))
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzS: DoublerOpen + ?Sized> DoublerOpenExt for __KzS {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn DoublerOpen {
    fn eq(&self, other: &Self) -> bool {
        DoublerOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn DoublerOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        DoublerOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn DoublerOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn DoublerOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn DoublerOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            DoublerOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn doubler() -> ::std::rc::Rc<dyn DoublerOpen> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(DoublerObject()), {
        {
            let __kz_shared15: ::std::rc::Rc<dyn DoublerOpen> = ::std::rc::Rc::new(DoublerObject());
            __kz_shared15
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzDoublerObject {
    #[doc(hidden)]
    pub __kz_super: DoublerBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct DoublerObject {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzDoublerObject>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn DoublerObject() -> DoublerObject {
    DoublerObject::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DoublerObject {
    #[doc(hidden)]
    pub(crate) fn __kz_new() -> DoublerObject {
        let __kz_this = DoublerObject {
            __kz_this: ::std::rc::Rc::new(__KzDoublerObject { __kz_super: DoublerBase::__kz_empty() }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        DoublerOpen::__kz_doubler_init(self);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl DoublerObject {
    pub fn wrap<T: ::core::clone::Clone + 'static>(&self, x: T) -> ::std::vec::Vec<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&x,);
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::list_of([x.clone(), x.clone()]) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for DoublerObject {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for DoublerObject {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for DoublerObject {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for DoublerObject {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Object",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for DoublerObject {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for DoublerObject {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DoublerOpen for DoublerObject {
    fn __kz_doubler(&self) -> &DoublerBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn DoublerOpen>>() {
            let o: ::std::rc::Rc<dyn DoublerOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<DoublerObject> for ::std::rc::Rc<dyn DoublerOpen> {
    fn eq(&self, other: &DoublerObject) -> bool {
        DoublerOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn DoublerOpen>> for DoublerObject {
    fn eq(&self, other: &::std::rc::Rc<dyn DoublerOpen>) -> bool {
        DoublerOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn DoublerOpen>> for DoublerObject {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn DoublerOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn generics_line() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Wrapper(), TwinWrapper()), {
        ::kompoz::runtime::kt::template!(
            Wrapper().both(),
            " ",
            TwinWrapper().both(),
            " ",
            wrapper().both(),
            " ",
            doubler().twice(),
            " ",
            wrapper().wrap(true)
        )
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzBrick {
    side: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Brick {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzBrick>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Brick(side: i32) -> Brick {
    Brick::__kz_new(side)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Brick {
    #[doc(hidden)]
    pub fn __kz_new(side: i32) -> Brick {
        let __kz_this =
            Brick { __kz_this: ::std::rc::Rc::new(__KzBrick { side: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(side);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, side: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.side.init(side);
        ::kompoz::runtime::named_args!({})
    }
    pub fn side(&self) -> i32 {
        self.__kz_this.side.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Brick {
    pub fn equals_kt(
        &self,
        other: impl ::core::convert::Into<::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>>>,
    ) -> bool {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let other: ::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>> =
            ::core::convert::Into::into(other);
        let _ = (&other,);
        ::kompoz::runtime::named_args!(types(Brick(r)), {
            if ::kompoz::runtime::kt::kt_same_as(
                &self.clone(),
                ::core::option::Option::as_deref(&other.clone()),
                |x: &Brick| ::std::rc::Rc::as_ptr(&x.__kz_this) as *const (),
            ) {
                return true;
            }
            let other = match ::core::option::Option::and_then(::core::option::Option::as_ref(&other), |__kz_o16| {
                ::kompoz::runtime::kt::KtAs::<Brick>::kt_as_safe(&**__kz_o16)
            }) {
                ::core::option::Option::Some(__kz_cast) => __kz_cast,
                ::core::option::Option::None => {
                    return false;
                }
            };
            return self.side() == other.side();
        })
    }
    pub fn hash_code_kt(&self) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            (((self.side()).wrapping_mul(1000003)).wrapping_mul(1000003)).wrapping_add(
                ::kompoz::runtime::kt::KtHashCode::kt_hash_code(&::kompoz::runtime::kt::list_of([self.side()])),
            )
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Brick {
    fn eq(&self, other: &Self) -> bool {
        self.equals_kt(::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(other))
            as ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Brick {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Brick {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code_kt())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Brick {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Brick",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Brick {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Brick {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn squares() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Brick(r)), {
        ::kompoz::runtime::kt::template!(
            "",
            (Brick(2) == Brick(2)),
            " ",
            (Brick(2) == Brick(3)),
            " ",
            (Brick(7).hash_code_kt() == Brick(7).hash_code_kt())
        )
    })
}
