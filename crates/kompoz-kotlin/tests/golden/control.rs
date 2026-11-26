#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for Refused {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        let (__kz_m, __kz_c) = self.__kz_this.__kz_exception.with(|e| (e.message(), e.cause()));
        ::kompoz::runtime::kt::Throwable::of_object(
            "Refused",
            "IllegalStateException",
            __kz_m,
            __kz_c,
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<Refused> for ::kompoz::runtime::kt::Throwable {
    fn from(e: Refused) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<Refused> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<Refused> {
        self.downcast::<Refused>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzRefused {
    code: ::kompoz::runtime::kt::KtVal<i32>,
    #[doc(hidden)]
    pub __kz_exception: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::Throwable>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Refused {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzRefused>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Refused(code: i32) -> Refused {
    Refused::__kz_new(code)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Refused {
    #[doc(hidden)]
    pub fn __kz_new(code: i32) -> Refused {
        let __kz_this = Refused {
            __kz_this: ::std::rc::Rc::new(__KzRefused {
                code: ::kompoz::runtime::kt::KtVal::new(),
                __kz_exception: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(code);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, code: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.__kz_exception.init(::kompoz::runtime::kt::exception(
            "IllegalStateException",
            ::core::option::Option::map(
                ::core::option::Option::Some(::kompoz::runtime::kt::template!("kod ", code)),
                ::std::string::String::from,
            ),
            ::core::option::Option::None,
        ));
        self.__kz_this.code.init(code);
        ::kompoz::runtime::named_args!({})
    }
    pub fn code(&self) -> i32 {
        self.__kz_this.code.get()
    }
    #[doc = r" Kotlin's `message`."]
    pub fn message(&self) -> ::core::option::Option<::std::string::String> {
        self.__kz_this.__kz_exception.with(|e| e.message())
    }
    #[doc = r" Kotlin's `cause`."]
    pub fn cause(&self) -> ::core::option::Option<::kompoz::runtime::kt::Throwable> {
        self.__kz_this.__kz_exception.with(|e| e.cause())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Refused {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Refused {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Refused {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Refused {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Refused {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Refused {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Refused {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn parsed_or(text: impl ::core::convert::Into<::std::string::String>, fallback: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let text: ::std::string::String = ::core::convert::Into::into(text);
    ::kompoz::runtime::named_args!({
        {
            let __kz_r = match ::kompoz::runtime::kt::catching(|| ::kompoz::runtime::kt::KtStr::to_int(&text)) {
                ::core::result::Result::Ok(__kz_v) => ::core::result::Result::Ok(__kz_v),
                ::core::result::Result::Err(__kz_e) => ::kompoz::runtime::kt::catching(|| {
                    if __kz_e.is_a("NumberFormatException") {
                        #[allow(unused_variables)]
                        let e = __kz_e;
                        fallback
                    } else {
                        ::kompoz::runtime::kt::rethrow(__kz_e)
                    }
                }),
            };
            {
                ::kompoz::runtime::kt::println("parsed");
            };
            match __kz_r {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_e) => ::kompoz::runtime::kt::rethrow(__kz_e),
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn first_label(items: ::std::vec::Vec<::std::string::String>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Refused(r)), {
        for item in ::kompoz::runtime::kt::KtIterable::kt_iter(&items) {
            match ::kompoz::runtime::kt::catching(|| {
                ::kompoz::runtime::kt::Jump::<_, _>::Value({
                    if ::kompoz::runtime::kt::KtStr::is_empty(&item) {
                        return ::kompoz::runtime::kt::Jump::Loop(0u32);
                    }
                    if item == "stop" {
                        return ::kompoz::runtime::kt::Jump::Loop(1u32);
                    }
                    return ::kompoz::runtime::kt::Jump::Return(item.clone());
                })
            }) {
                ::core::result::Result::Ok(__kz_v) => match __kz_v {
                    ::kompoz::runtime::kt::Jump::Value(__kz_x) => __kz_x,
                    ::kompoz::runtime::kt::Jump::Return(__kz_x) => return __kz_x,
                    ::kompoz::runtime::kt::Jump::Loop(0u32) => continue,
                    ::kompoz::runtime::kt::Jump::Loop(1u32) => break,
                    ::kompoz::runtime::kt::Jump::Loop(_) => ::core::unreachable!(),
                },
                ::core::result::Result::Err(__kz_e) => {
                    if __kz_e.is_object::<Refused>() {
                        #[allow(unused_variables)]
                        let e = ::core::option::Option::unwrap(__kz_e.downcast::<Refused>());
                        return ::kompoz::runtime::kt::template!("kod ", e.code());
                    } else {
                        ::kompoz::runtime::kt::rethrow(__kz_e)
                    }
                }
            };
        }
        ::kompoz::runtime::kt::throw(Refused(1));
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn describe_int(n: i32) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!("sayı ", n) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::named]
pub fn describe_string_int(
    s: impl ::core::convert::Into<::std::string::String>,
    #[default(2)] times: i32,
) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let s: ::std::string::String = ::core::convert::Into::into(s);
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtStr::repeat(&s, times) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Vec2(x: i32, y: i32) -> Vec2 {
    Vec2::__kz_new(x, y)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Vec2 {
    #[doc(hidden)]
    pub fn __kz_new(x: i32, y: i32) -> Vec2 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Vec2(rr)), { Vec2 { x, y } })
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
impl Vec2 {
    pub fn copy(&self, # [default (self . x)] x: i32, # [default (self . y)] y: i32) -> Vec2 {
        Vec2::__kz_new(x, y)
    }
    pub fn plus(&self, o: Vec2) -> Vec2 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Vec2(rr)), { Vec2(self.x() + o.x(), self.y() + o.y()) })
    }
    pub fn get(&self, i: i32) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ if i == 0 { self.x() } else { self.y() } })
    }
    pub fn dot(&self, o: Vec2) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ self.x() * o.x() + self.y() * o.y() })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Vec2 {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.x, state);
        ::core::hash::Hash::hash(&self.y, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Vec2 {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Vec2 {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Vec2(");
        __kz_out.push_str("x=");
        (&::kompoz::runtime::kt::KtProbe(&self.x)).kt_write_probe(__kz_out);
        __kz_out.push_str(", y=");
        (&::kompoz::runtime::kt::KtProbe(&self.y)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Vec2 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Vec2 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait Vec2TimesExt)]
impl Vec2 {
    fn times(self, k: i32) -> Vec2 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Vec2(rr)), { Vec2(self.x() * k, self.y() * k) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(non_camel_case_types)]
pub trait Vec2Length2Ext {
    fn length2(&self) -> i32;
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
impl Vec2Length2Ext for Vec2 {
    fn length2(&self) -> i32 {
        #[allow(unused_variables)]
        let __kz_recv = ::core::clone::Clone::clone(self);
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ __kz_recv.x() * __kz_recv.x() + __kz_recv.y() * __kz_recv.y() })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn total(xs: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtList::sum(&xs) })
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static visits : :: kompoz :: runtime :: kt :: Ref < i32 > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (:: kompoz :: runtime :: kt :: Ref :: new (0)) } ; }
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static greeting : :: std :: rc :: Rc < dyn :: kompoz :: runtime :: kt :: KtDelegate < Value = :: std :: string :: String >> = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (:: std :: rc :: Rc :: new (:: kompoz :: runtime :: kt :: delegate_of :: < :: std :: string :: String , _ > (:: kompoz :: runtime :: kt :: lazy (move | | { :: std :: string :: String :: from ("merhaba") })))) } ; }
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn now() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ visits.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d)) + 1 })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn level() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ visits.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d)) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn set_level(value: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        visits.with(|__kz_d| {
            let __kz_v = value;
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(__kz_d, __kz_v)
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzCounter {
    count: ::kompoz::runtime::kt::KtVal<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtMutableDelegate<Value = i32>>>,
    step: ::kompoz::runtime::kt::KtVar<i32>,
    name: ::kompoz::runtime::kt::KtLateinit<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Counter {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzCounter>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Counter() -> Counter {
    Counter::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Counter {
    #[doc(hidden)]
    pub fn __kz_new() -> Counter {
        let __kz_this = Counter {
            __kz_this: ::std::rc::Rc::new(__KzCounter {
                count: ::kompoz::runtime::kt::KtVal::new(),
                step: ::kompoz::runtime::kt::KtVar::new(),
                name: ::kompoz::runtime::kt::KtLateinit::new("name"),
            }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_this.count.init(::std::rc::Rc::new(::kompoz::runtime::kt::delegate_of::<i32, _>(
                ::kompoz::runtime::mutable_state_of!(0),
            )));
            self.__kz_this.step.set(1);
        })
    }
    pub fn count(&self) -> i32 {
        self.__kz_this.count.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
    }
    fn set_count(&self, value: i32) {
        self.__kz_this.count.with(|__kz_d| ::kompoz::runtime::kt::KtMutableDelegate::set_value(__kz_d, value))
    }
    pub fn step(&self) -> i32 {
        self.__kz_this.step.get()
    }
    pub fn set_step(&self, value: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_mut, unused_assignments)]
        let mut field: i32 = self.__kz_this.step.get();
        ::kompoz::runtime::named_args!({
            field = if value > 0 { value } else { 1 };
        });
        self.__kz_this.step.set(field)
    }
    pub fn name(&self) -> ::std::string::String {
        ::kompoz::runtime::kt::KtDelegate::get_value(&self.__kz_this.name)
    }
    pub fn set_name(&self, value: ::std::string::String) {
        ::kompoz::runtime::kt::KtMutableDelegate::set_value(&self.__kz_this.name, value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Counter {
    pub fn bump(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.set_count(self.count() + self.step());
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Counter {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Counter {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Counter {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Counter {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Counter",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Counter {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Counter {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzProfile {
    handle: ::kompoz::runtime::kt::KtVal<SavedStateHandle>,
    user: ::kompoz::runtime::kt::KtVal<
        ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtDelegate<Value = ::std::string::String>>,
    >,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Profile {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzProfile>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Profile(handle: SavedStateHandle) -> Profile {
    Profile::__kz_new(handle)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Profile {
    #[doc(hidden)]
    pub fn __kz_new(handle: SavedStateHandle) -> Profile {
        let __kz_this = Profile {
            __kz_this: ::std::rc::Rc::new(__KzProfile {
                handle: ::kompoz::runtime::kt::KtVal::new(),
                user: ::kompoz::runtime::kt::KtVal::new(),
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
        self.__kz_this.handle.init(::core::clone::Clone::clone(&handle));
        ::kompoz::runtime::named_args!({
            self.__kz_this.user.init(::std::rc::Rc::new(
                ::kompoz::runtime::kt::delegate_of::<::std::string::String, _>(
                    handle.saved("Profile.user", move || ::std::string::String::from("ada")),
                ),
            ));
        })
    }
    pub fn handle(&self) -> SavedStateHandle {
        self.__kz_this.handle.get()
    }
    pub fn user(&self) -> ::std::string::String {
        self.__kz_this.user.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Profile {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Profile {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Profile {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Profile {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Profile {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Profile",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
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
pub fn twice(block: impl ::core::ops::Fn()) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        block();
        block();
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[doc(hidden)]
pub async fn twice_async(block: impl ::core::ops::AsyncFn()) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        block().await;
        block().await;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn forms(
    more: ::std::vec::Vec<i32>,
    on_done: ::core::option::Option<::std::rc::Rc<dyn ::core::ops::Fn()>>,
    load: impl ::core::ops::AsyncFn() -> i32 + 'static,
) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Vec2(rr), Counter()), {
        let clicks = ::kompoz::runtime::kt::Ref::new(0);
        let click = {
            let clicks = ::kompoz::runtime::__private::capture(&clicks);
            move || ::kompoz::runtime::kt::post_inc(&clicks)
        };
        click();
        let fact: ::std::rc::Rc<dyn ::core::ops::Fn(i32) -> i32> = {
            let __kz_slot: ::std::rc::Rc<::std::cell::OnceCell<::std::rc::Weak<dyn ::core::ops::Fn(i32) -> i32>>> =
                ::std::rc::Rc::new(::std::cell::OnceCell::new());
            let __kz_f: ::std::rc::Rc<dyn ::core::ops::Fn(i32) -> i32> = ::std::rc::Rc::new({
                let __kz_slot = ::std::rc::Rc::clone(&__kz_slot);
                move |n: i32| -> i32 {
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    #[allow(unused_variables)]
                    let fact = __kz_slot.get().and_then(::std::rc::Weak::upgrade).expect("a local function, made");
                    ::kompoz::runtime::named_args!({ if n <= 1 { 1 } else { n * fact(n - 1) } })
                }
            });
            let _ = __kz_slot.set(::std::rc::Rc::downgrade(&__kz_f));
            __kz_f
        };
        let double = {
            move |x: i32| -> i32 {
                #[allow(unused_imports)]
                use ::kompoz::runtime::kt::ext::*;
                ::kompoz::runtime::named_args!({
                    return x * 2;
                })
            }
        };
        let r#ref = move |__kz_p0: ::std::string::String, __kz_p1: i32| parsed_or(__kz_p0.clone(), __kz_p1);
        let a = Vec2(1, 2).plus(Vec2(3, 4).times(2));
        for it in ::kompoz::runtime::kt::KtIterable::kt_iter(&more) {
            if it < 0 {
                return -1;
            }
        }
        ::kompoz::runtime::kt::safe!(on_done.clone(), |__kz_it2| (__kz_it2)());
        {
            visits.with(|__kz_d| ::kompoz::runtime::kt::post_inc(__kz_d));
        };
        set_level(level() + now());
        let c = Counter();
        c.set_step(-3);
        let before = {
            let __kz_old = c.step();
            let __kz_step = ::kompoz::runtime::kt::KtInc::inc(::core::clone::Clone::clone(&__kz_old));
            c.set_step(__kz_step);
            __kz_old
        };
        return fact(4)
            + double(::kompoz::runtime::kt::KtDelegate::get_value(&clicks))
            + r#ref(::std::string::String::from("1"), 0)
            + a.get(0)
            + (a.dot(a.clone()))
            + a.clone().length2()
            + total({
                let mut __kz_va3 = ::std::vec::Vec::new();
                __kz_va3.push(1);
                ::core::iter::Extend::extend(&mut __kz_va3, more.clone());
                __kz_va3.push(3);
                __kz_va3
            })
            + ::kompoz::runtime::kt::KtStr::length(&describe_string_int("x"))
            + before
            + visits.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d))
            + ::kompoz::runtime::kt::KtStr::length(
                &greeting.with(|__kz_d| ::kompoz::runtime::kt::KtDelegate::get_value(__kz_d)),
            );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Coroutines() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect((), async {
        twice_async(async || {
            delay(1).await;
        })
        .await;
        {
            let __kz_cancelled = ::kompoz::runtime::kt::OnCancel::new(|| {
                #[allow(unused_variables)]
                let e = ::kompoz::runtime::kt::exception(
                    "CancellationException",
                    ::core::option::Option::None,
                    ::core::option::Option::None,
                );
                ::kompoz::runtime::kt::println("cancelled");
            });
            let __kz_t = match ::kompoz::runtime::kt::catching_async(async {
                {
                    delay(1).await;
                }
            })
            .await
            {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_e) => {
                    if __kz_e.is_a("CancellationException") {
                        #[allow(unused_variables)]
                        let e = __kz_e;
                        ::kompoz::runtime::kt::println("cancelled");
                    } else {
                        ::kompoz::runtime::kt::rethrow(__kz_e)
                    }
                }
            };
            __kz_cancelled.disarm();
            __kz_t
        };
    });
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] # [doc (hidden)] static __kz_field_stored : :: kompoz :: runtime :: kt :: Ref < i32 > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! ({ :: kompoz :: runtime :: kt :: Ref :: new (1) }) } ; }
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn stored() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    #[allow(unused_mut, unused_variables, unused_assignments)]
    let mut field: i32 = __kz_field_stored.with(|c| c.get());
    ::kompoz::runtime::named_args!({ field * 10 })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn set_stored(value: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    #[allow(unused_mut, unused_variables, unused_assignments)]
    let mut field: i32 = __kz_field_stored.with(|c| c.get());
    ::kompoz::runtime::named_args!({
        field = value + 1;
    });
    __kz_field_stored.with(|c| c.set(field));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzCents {
    cents: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Cents {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzCents>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Cents(cents: i32) -> Cents {
    Cents::__kz_new(cents)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Cents {
    #[doc(hidden)]
    pub fn __kz_new(cents: i32) -> Cents {
        let __kz_this =
            Cents { __kz_this: ::std::rc::Rc::new(__KzCents { cents: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(cents);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, cents: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.cents.init(cents);
        ::kompoz::runtime::named_args!({})
    }
    pub fn cents(&self) -> i32 {
        self.__kz_this.cents.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Cents {
    pub fn equals_kt(
        &self,
        other: impl ::core::convert::Into<::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>>>,
    ) -> bool {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let other: ::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>> =
            ::core::convert::Into::into(other);
        let _ = (&other,);
        ::kompoz::runtime::named_args!(types(Cents(r)), {
            ::kompoz::runtime::kt::safe!(
                (::core::option::Option::and_then(::core::option::Option::as_ref(&other), |__kz_o4| {
                    ::kompoz::runtime::kt::KtAs::<Cents>::kt_as_safe(&**__kz_o4)
                })),
                |__kz_it5| __kz_it5.cents()
            ) == ::core::option::Option::Some(self.cents())
        })
    }
    pub fn hash_code_kt(&self) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ self.cents() })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Cents {
    fn eq(&self, other: &Self) -> bool {
        self.equals_kt(::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(other))
            as ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Cents {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Cents {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code_kt())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Cents {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Cents",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Cents {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Cents {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn pick_one_int(n: i32) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!("i", n) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn pick_one_string(s: impl ::core::convert::Into<::std::string::String>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let s: ::std::string::String = ::core::convert::Into::into(s);
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!("s", s) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn more_locals(base: i32) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Cents(r)), {
        let plus_base: ::std::rc::Rc<dyn ::core::ops::Fn(i32) -> i32> = ::std::rc::Rc::new({
            let base = ::kompoz::runtime::__private::capture(&base);
            move |__kz_recv: i32| -> i32 {
                #[allow(unused_imports)]
                use ::kompoz::runtime::kt::ext::*;
                ::kompoz::runtime::named_args!({ __kz_recv + base })
            }
        });
        let greet: ::std::rc::Rc<
            dyn ::core::ops::Fn(::std::string::String, ::std::string::String) -> ::std::string::String,
        > = ::std::rc::Rc::new({
            move |name: ::std::string::String, punct: ::std::string::String| -> ::std::string::String {
                #[allow(unused_imports)]
                use ::kompoz::runtime::kt::ext::*;
                ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(name, punct) })
            }
        });
        #[allow(
            non_snake_case,
            non_upper_case_globals,
            unused_parens,
            unused_braces,
            unused_mut,
            while_true,
            clippy::all
        )]
        pub fn twice_of<T: ::core::clone::Clone + 'static>(x: T) -> ::std::vec::Vec<T> {
            #[allow(unused_imports)]
            use ::kompoz::runtime::kt::ext::*;
            ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::list_of([x.clone(), x.clone()]) })
        }
        let f = move |__kz_p0: ::std::string::String| pick_one_string(__kz_p0.clone());
        set_stored(5);
        return ::kompoz::runtime::kt::template!(
            greet(::std::string::String::from("a"), ::std::string::String::from("!")),
            greet(::std::string::String::from("b"), ::std::string::String::from("?")),
            plus_base((3 as i32)),
            twice_of(1),
            f(::std::string::String::from("x")),
            stored(),
            (Cents(1) == Cents(1))
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn waits() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let later: ::std::rc::Rc<
            dyn ::core::ops::Fn(i32) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = i32>>>,
        > = ::std::rc::Rc::new({
            move |n: i32| -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = i32>>> {
                ::std::boxed::Box::pin(async move {
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    ::kompoz::runtime::named_args!({
                        delay(1).await;
                        return n * 2;
                    })
                })
            }
        });
        return later(2).await + later(3).await;
    })
}
