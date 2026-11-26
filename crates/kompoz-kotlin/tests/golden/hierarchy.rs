#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Titled: ::kompoz::runtime::kt::KtObject {
    fn name(&self) -> ::std::string::String;
    fn __kz_titled_title(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!("[", self.name(), "]") })
    }
    fn title(&self) -> ::std::string::String {
        self.__kz_titled_title()
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
impl ::core::cmp::PartialEq for dyn Titled {
    fn eq(&self, other: &Self) -> bool {
        Titled::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Titled {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Titled::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Titled {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Titled {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Titled {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Titled::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Shelf: Titled {
    fn picks(&self) -> i32;
    fn set_picks(&self, value: i32);
    fn count(&self) -> i32;
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn Shelf {
    fn eq(&self, other: &Self) -> bool {
        Titled::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Shelf {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Titled::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Shelf {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Shelf {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Shelf {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Titled::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Tag {
    pub name: ::std::string::String,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Tag(name: impl ::core::convert::Into<::std::string::String>) -> Tag {
    let name: ::std::string::String = ::core::convert::Into::into(name);
    Tag::__kz_new(name)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Tag {
    #[doc(hidden)]
    pub fn __kz_new(name: ::std::string::String) -> Tag {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Tag(r)), { Tag { name } })
    }
    pub fn name(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.name)
    }
    pub fn component1(&self) -> ::std::string::String {
        self.name()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Tag {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . name))] name: ::std::string::String,
    ) -> Tag {
        Tag::__kz_new(name)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Tag {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Tag(");
        __kz_out.push_str("name=");
        (&::kompoz::runtime::kt::KtProbe(&self.name)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Tag {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Tag {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Titled for Tag {
    fn name(&self) -> ::std::string::String {
        Tag::name(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn Titled>>() {
            let o: ::std::rc::Rc<dyn Titled> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Tag> for ::std::rc::Rc<dyn Titled> {
    fn eq(&self, other: &Tag) -> bool {
        Titled::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn Titled>> for Tag {
    fn eq(&self, other: &::std::rc::Rc<dyn Titled>) -> bool {
        Titled::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn Titled>> for Tag {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn Titled>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct PlateBase {
    label: ::kompoz::runtime::kt::KtVal<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PlateBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        PlateBase { label: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait PlateOpen: Titled {
    #[doc(hidden)]
    fn __kz_plate(&self) -> &PlateBase;
    fn label(&self) -> ::std::string::String {
        self.__kz_plate().label.get()
    }
    #[doc(hidden)]
    fn __kz_plate_init(&self, label: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_plate().label.init(::core::clone::Clone::clone(&label));
        })
    }
    fn area(&self) -> f32;
    fn __kz_plate_open_describe(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(self.label(), " ", self.area()) })
    }
    fn describe(&self) -> ::std::string::String {
        self.__kz_plate_open_describe()
    }
    fn __kz_plate_open_name(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ self.label() })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn PlateOpen {
    fn eq(&self, other: &Self) -> bool {
        Titled::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn PlateOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Titled::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn PlateOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn PlateOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn PlateOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Titled::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzRound {
    #[doc(hidden)]
    pub __kz_super: PlateBase,
    r: ::kompoz::runtime::kt::KtVal<f32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Round {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzRound>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Round(r: f32) -> Round {
    Round::__kz_new(r)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Round {
    #[doc(hidden)]
    pub fn __kz_new(r: f32) -> Round {
        let __kz_this = Round {
            __kz_this: ::std::rc::Rc::new(__KzRound {
                __kz_super: PlateBase::__kz_empty(),
                r: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(r);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, r: f32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        PlateOpen::__kz_plate_init(self, ::std::string::String::from(::std::string::String::from("daire")));
        self.__kz_this.r.init(r);
        ::kompoz::runtime::named_args!({})
    }
    pub fn r(&self) -> f32 {
        self.__kz_this.r.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Round {
    pub fn area(&self) -> f32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ 3_f32 * self.r() * self.r() })
    }
    pub fn describe(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::template!("yuvarlak ", PlateOpen::__kz_plate_open_describe(self,))
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Round {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Round {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Round {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Round {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Round",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Round {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Round {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PlateOpen for Round {
    fn __kz_plate(&self) -> &PlateBase {
        &self.__kz_this.__kz_super
    }
    fn area(&self) -> f32 {
        Round::area(self)
    }
    fn describe(&self) -> ::std::string::String {
        Round::describe(self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Titled for Round {
    fn name(&self) -> ::std::string::String {
        PlateOpen::__kz_plate_open_name(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn PlateOpen>>() {
            let o: ::std::rc::Rc<dyn PlateOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn Titled>>() {
            let o: ::std::rc::Rc<dyn Titled> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Round> for ::std::rc::Rc<dyn PlateOpen> {
    fn eq(&self, other: &Round) -> bool {
        Titled::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn PlateOpen>> for Round {
    fn eq(&self, other: &::std::rc::Rc<dyn PlateOpen>) -> bool {
        Titled::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn PlateOpen>> for Round {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn PlateOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Round> for ::std::rc::Rc<dyn Titled> {
    fn eq(&self, other: &Round) -> bool {
        Titled::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn Titled>> for Round {
    fn eq(&self, other: &::std::rc::Rc<dyn Titled>) -> bool {
        Titled::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn Titled>> for Round {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn Titled>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct StepperBase {
    total: ::kompoz::runtime::kt::KtVar<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl StepperBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        StepperBase { total: ::kompoz::runtime::kt::KtVar::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait StepperOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_stepper(&self) -> &StepperBase;
    fn total(&self) -> i32 {
        self.__kz_stepper().total.get()
    }
    fn set_total(&self, value: i32) {
        self.__kz_stepper().total.set(value)
    }
    #[doc(hidden)]
    fn __kz_stepper_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_stepper().total.set(0);
        })
    }
    fn __kz_stepper_open_step(&self) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ 1 })
    }
    fn step(&self) -> i32 {
        self.__kz_stepper_open_step()
    }
    fn __kz_stepper_open_tick(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.set_total(self.total() + self.step());
        })
    }
    fn tick(&self) {
        self.__kz_stepper_open_tick()
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
impl ::core::cmp::PartialEq for dyn StepperOpen {
    fn eq(&self, other: &Self) -> bool {
        StepperOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn StepperOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        StepperOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn StepperOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn StepperOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn StepperOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            StepperOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzStepper {
    #[doc(hidden)]
    pub __kz_super: StepperBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Stepper {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzStepper>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Stepper() -> Stepper {
    Stepper::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Stepper {
    #[doc(hidden)]
    pub fn __kz_new() -> Stepper {
        let __kz_this =
            Stepper { __kz_this: ::std::rc::Rc::new(__KzStepper { __kz_super: StepperBase::__kz_empty() }) };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        StepperOpen::__kz_stepper_init(self);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Stepper {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Stepper {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Stepper {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Stepper {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Stepper {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Stepper",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Stepper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Stepper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl StepperOpen for Stepper {
    fn __kz_stepper(&self) -> &StepperBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn StepperOpen>>() {
            let o: ::std::rc::Rc<dyn StepperOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Stepper> for ::std::rc::Rc<dyn StepperOpen> {
    fn eq(&self, other: &Stepper) -> bool {
        StepperOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn StepperOpen>> for Stepper {
    fn eq(&self, other: &::std::rc::Rc<dyn StepperOpen>) -> bool {
        StepperOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn StepperOpen>> for Stepper {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn StepperOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzDoubleStepper {
    #[doc(hidden)]
    pub __kz_super: StepperBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct DoubleStepper {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzDoubleStepper>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn DoubleStepper() -> DoubleStepper {
    DoubleStepper::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl DoubleStepper {
    #[doc(hidden)]
    pub fn __kz_new() -> DoubleStepper {
        let __kz_this = DoubleStepper {
            __kz_this: ::std::rc::Rc::new(__KzDoubleStepper { __kz_super: StepperBase::__kz_empty() }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        StepperOpen::__kz_stepper_init(self);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl DoubleStepper {
    pub fn step(&self) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ 2 })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for DoubleStepper {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for DoubleStepper {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for DoubleStepper {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for DoubleStepper {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "DoubleStepper",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for DoubleStepper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for DoubleStepper {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl StepperOpen for DoubleStepper {
    fn __kz_stepper(&self) -> &StepperBase {
        &self.__kz_this.__kz_super
    }
    fn step(&self) -> i32 {
        DoubleStepper::step(self)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn StepperOpen>>() {
            let o: ::std::rc::Rc<dyn StepperOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<DoubleStepper> for ::std::rc::Rc<dyn StepperOpen> {
    fn eq(&self, other: &DoubleStepper) -> bool {
        StepperOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn StepperOpen>> for DoubleStepper {
    fn eq(&self, other: &::std::rc::Rc<dyn StepperOpen>) -> bool {
        StepperOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn StepperOpen>> for DoubleStepper {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn StepperOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzPageViewModel {
    #[doc(hidden)]
    pub __kz_super: ::kompoz::runtime::kt::KtVal<ViewModelImpl>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct PageViewModel {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzPageViewModel>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn PageViewModel() -> PageViewModel {
    PageViewModel::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl PageViewModel {
    #[doc(hidden)]
    pub fn __kz_new() -> PageViewModel {
        let __kz_this = PageViewModel {
            __kz_this: ::std::rc::Rc::new(__KzPageViewModel { __kz_super: ::kompoz::runtime::kt::KtVal::new() }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    pub fn __kz_new_default() -> PageViewModel {
        PageViewModel::__kz_new()
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.__kz_super.init(ViewModelImpl::new());
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl PageViewModel {
    pub fn on_cleared(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for PageViewModel {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for PageViewModel {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for PageViewModel {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for PageViewModel {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "PageViewModel",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for PageViewModel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for PageViewModel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ViewModel for PageViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        (&self.__kz_this.__kz_super).get_ref()
    }
    fn on_cleared(&self) {
        PageViewModel::on_cleared(self)
    }
    fn new_instance(extras: &CreationExtras) -> ::core::option::Option<Self> {
        let _ = extras;
        ::core::option::Option::Some(PageViewModel::__kz_new_default())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ViewModelRef for PageViewModel {
    type Model = PageViewModel;
    fn from_rc(model: ::std::rc::Rc<PageViewModel>) -> PageViewModel {
        ::core::clone::Clone::clone(&*model)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct ScreenBase {
    route: ::kompoz::runtime::kt::KtVal<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScreenBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        ScreenBase { route: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait ScreenOpen: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_screen(&self) -> &ScreenBase;
    fn route(&self) -> ::std::string::String {
        self.__kz_screen().route.get()
    }
    #[doc(hidden)]
    fn __kz_screen_init(&self, route: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_screen().route.init(::core::clone::Clone::clone(&route));
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
pub enum Screen {
    Home(ScreenHome),
    Detail(ScreenDetail),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<ScreenHome> for Screen {
    fn from(value: ScreenHome) -> Screen {
        Screen::Home(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<ScreenHome> for Screen {
    fn kt_as_safe(&self) -> ::core::option::Option<ScreenHome> {
        match self {
            Screen::Home(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<ScreenHome> for Screen {
    fn eq(&self, other: &ScreenHome) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Screen::Home(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Screen> for ScreenHome {
    fn eq(&self, other: &Screen) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<ScreenDetail> for Screen {
    fn from(value: ScreenDetail) -> Screen {
        Screen::Detail(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<ScreenDetail> for Screen {
    fn kt_as_safe(&self) -> ::core::option::Option<ScreenDetail> {
        match self {
            Screen::Detail(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<ScreenDetail> for Screen {
    fn eq(&self, other: &ScreenDetail) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Screen::Detail(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Screen> for ScreenDetail {
    fn eq(&self, other: &Screen) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Screen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Screen::Home(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Screen::Detail(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Screen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Screen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScreenOpen for Screen {
    fn __kz_screen(&self) -> &ScreenBase {
        match self {
            Screen::Home(v) => ScreenOpen::__kz_screen(v),
            Screen::Detail(v) => ScreenOpen::__kz_screen(v),
        }
    }
    fn route(&self) -> ::std::string::String {
        match self {
            Screen::Home(v) => ScreenOpen::route(v),
            Screen::Detail(v) => ScreenOpen::route(v),
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Screen::Home(v) => ScreenOpen::__kz_eq(v, other),
                Screen::Detail(v) => ScreenOpen::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        match self {
            Screen::Home(v) => ScreenOpen::__kz_cast(v, to),
            Screen::Detail(v) => ScreenOpen::__kz_cast(v, to),
        }
    }
    fn __kz_screen_init(&self, _: ::std::string::String) {}
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzScreenHome {
    #[doc(hidden)]
    pub __kz_super: ScreenBase,
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
pub struct ScreenHome {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScreenHome {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzScreenHome {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzScreenHome > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE
            .with(|s| s.get(|| __KzScreenHome { __kz_super: ScreenBase::__kz_empty() }, |_| ScreenHome {}.__kz_init()))
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ScreenOpen::__kz_screen_init(self, ::std::string::String::from(::std::string::String::from("home")));
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl ScreenHome {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for ScreenHome {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str("Home");
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for ScreenHome {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for ScreenHome {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScreenOpen for ScreenHome {
    fn __kz_screen(&self) -> &ScreenBase {
        &Self::__kz_state().__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Screen>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Screen as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct ScreenDetail {
    pub id: i32,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<ScreenBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn ScreenDetail(id: i32) -> ScreenDetail {
    ScreenDetail::__kz_new(id)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScreenDetail {
    #[doc(hidden)]
    pub fn __kz_new(id: i32) -> ScreenDetail {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(ScreenDetail(r)), {
            let __kz_v = ScreenDetail {
                id: ::core::clone::Clone::clone(&id),
                __kz_super: ::std::rc::Rc::new(ScreenBase::__kz_empty()),
            };
            ScreenOpen::__kz_screen_init(
                &__kz_v,
                ::std::string::String::from(::kompoz::runtime::kt::template!("detail/", id)),
            );
            __kz_v
        })
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn component1(&self) -> i32 {
        self.id()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl ScreenDetail {
    pub fn copy(&self, # [default (self . id)] id: i32) -> ScreenDetail {
        ScreenDetail::__kz_new(id)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for ScreenDetail {
    fn eq(&self, other: &Self) -> bool {
        true && self.id == other.id
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for ScreenDetail {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Detail(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for ScreenDetail {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for ScreenDetail {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScreenOpen for ScreenDetail {
    fn __kz_screen(&self) -> &ScreenBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Screen>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Screen as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait FigureOpen: ::kompoz::runtime::kt::KtObject {
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
pub enum Figure {
    Disc(Disc),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<Disc> for Figure {
    fn from(value: Disc) -> Figure {
        Figure::Disc(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<Disc> for Figure {
    fn kt_as_safe(&self) -> ::core::option::Option<Disc> {
        match self {
            Figure::Disc(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Disc> for Figure {
    fn eq(&self, other: &Disc) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Figure::Disc(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Figure> for Disc {
    fn eq(&self, other: &Figure) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Figure {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Figure::Disc(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Figure {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Figure {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl FigureOpen for Figure {
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Figure::Disc(v) => FigureOpen::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        match self {
            Figure::Disc(v) => FigureOpen::__kz_cast(v, to),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Disc {
    pub r: f32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Disc(r: f32) -> Disc {
    Disc::__kz_new(r)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Disc {
    #[doc(hidden)]
    pub fn __kz_new(r: f32) -> Disc {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Disc(r)), { Disc { r } })
    }
    pub fn r(&self) -> f32 {
        self.r
    }
    pub fn component1(&self) -> f32 {
        self.r()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Disc {
    pub fn copy(&self, # [default (self . r)] r: f32) -> Disc {
        Disc::__kz_new(r)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Disc {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Disc(");
        __kz_out.push_str("r=");
        (&::kompoz::runtime::kt::KtProbe(&self.r)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Disc {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Disc {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl FigureOpen for Disc {
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Figure>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Figure as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn describe(s: Screen) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_w1 = s.clone();
            match &__kz_w1 {
                Screen::Home(__kz_case2) => {
                    #[allow(unused_variables)]
                    let s = ::core::clone::Clone::clone(__kz_case2);
                    { ::std::string::String::from("ana") }
                }
                Screen::Detail(__kz_case3) => {
                    #[allow(unused_variables)]
                    let s = ::core::clone::Clone::clone(__kz_case3);
                    { ::kompoz::runtime::kt::template!("ayrıntı ", s.id()) }
                }
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzCrate<T> {
    value: ::kompoz::runtime::kt::KtVal<T>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Crate<T> {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzCrate<T>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Crate<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString>(value: T) -> Crate<T> {
    Crate::__kz_new(value)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> Crate<T> {
    #[doc(hidden)]
    pub fn __kz_new(value: T) -> Crate<T> {
        let __kz_this =
            Crate { __kz_this: ::std::rc::Rc::new(__KzCrate { value: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(value);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, value: T) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.value.init(::core::clone::Clone::clone(&value));
        ::kompoz::runtime::named_args!({})
    }
    pub fn value(&self) -> T {
        self.__kz_this.value.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> Crate<T> {
    pub fn describe(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!("Crate(", self.value(), ")") })
    }
    pub fn convert<R: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString>(
        &self,
        f: impl ::core::ops::Fn(T) -> R + 'static,
    ) -> Crate<R> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Crate(r)), { Crate(f(self.value())) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> ::core::cmp::PartialEq for Crate<T> {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> ::core::cmp::Eq for Crate<T> {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> ::core::hash::Hash for Crate<T> {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> ::kompoz::runtime::kt::KtToString
    for Crate<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Crate",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> ::core::fmt::Display for Crate<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString> ::core::fmt::Debug for Crate<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Duo<A, B> {
    pub first: A,
    pub second: B,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Duo<
    A: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    B: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
>(
    first: A,
    second: B,
) -> Duo<A, B> {
    Duo::__kz_new(first, second)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<
    A: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    B: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
> Duo<A, B>
{
    #[doc(hidden)]
    pub fn __kz_new(first: A, second: B) -> Duo<A, B> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Duo(rr)), { Duo { first, second } })
    }
    pub fn first(&self) -> A {
        ::core::clone::Clone::clone(&self.first)
    }
    pub fn second(&self) -> B {
        ::core::clone::Clone::clone(&self.second)
    }
    pub fn component1(&self) -> A {
        self.first()
    }
    pub fn component2(&self) -> B {
        self.second()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl<
    A: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    B: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
> Duo<A, B>
{
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . first))] first: A,
        # [default (:: core :: clone :: Clone :: clone (& self . second))] second: B,
    ) -> Duo<A, B> {
        Duo::__kz_new(first, second)
    }
    pub fn swap(&self) -> Duo<B, A> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Duo(rr)), { Duo(self.second(), self.first()) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<
    A: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    B: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
> ::kompoz::runtime::kt::KtToString for Duo<A, B>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Duo(");
        __kz_out.push_str("first=");
        (&::kompoz::runtime::kt::KtProbe(&self.first)).kt_write_probe(__kz_out);
        __kz_out.push_str(", second=");
        (&::kompoz::runtime::kt::KtProbe(&self.second)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<
    A: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    B: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
> ::core::fmt::Display for Duo<A, B>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<
    A: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    B: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
> ::core::fmt::Debug for Duo<A, B>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn first_or<T: ::core::clone::Clone + 'static>(items: ::std::vec::Vec<T>, fallback: T) -> T {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        if ::kompoz::runtime::kt::KtList::is_empty(&items) {
            fallback.clone()
        } else {
            ::kompoz::runtime::kt::KtList::first(&items)
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait ListSecondExt)]
impl<T: ::core::clone::Clone + 'static> ::std::vec::Vec<T> {
    fn second(self) -> T {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtList::get(&self, 1) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: cmp :: PartialOrd,
    :: core :: cmp :: Ord,
    :: core :: fmt :: Debug,
)]
pub enum Calc {
    PLUS,
    TIMES,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Calc {
    #[doc = r" The entry's name (Kotlin's `name`)."]
    pub fn name(self) -> ::std::string::String {
        ::std::string::String::from(match self {
            Calc::PLUS => "PLUS",
            Calc::TIMES => "TIMES",
        })
    }
    #[doc = r" Its place among the entries (Kotlin's `ordinal`)."]
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    #[doc = r" The entries in order (Kotlin's `entries`)."]
    pub fn entries() -> ::std::vec::Vec<Calc> {
        ::std::vec![Calc::PLUS, Calc::TIMES]
    }
    #[doc = r" The entries in order (Kotlin's `values()`)."]
    pub fn values() -> ::std::vec::Vec<Calc> {
        ::std::vec![Calc::PLUS, Calc::TIMES]
    }
    #[doc = r" The entry of a name (Kotlin's `valueOf`: an IllegalArgumentException for another name)."]
    #[track_caller]
    pub fn value_of(value: impl ::core::convert::Into<::std::string::String>) -> Calc {
        let value: ::std::string::String = value.into();
        match value.as_str() {
            "PLUS" => Calc::PLUS,
            "TIMES" => Calc::TIMES,
            _ => ::kompoz::runtime::kt::panic_kt(
                "IllegalArgumentException",
                &::std::format!("{}{}", "No enum constant Calc.", value),
            ),
        }
    }
    pub fn symbol(self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(match self {
            Calc::PLUS => ::std::string::String::from(::std::string::String::from("+")),
            Calc::TIMES => ::std::string::String::from(::std::string::String::from("*")),
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Calc {
    pub fn apply(self, a: i32, b: i32) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            #[allow(unreachable_patterns)]
            match self {
                Calc::PLUS => {
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    ::kompoz::runtime::named_args!({ a + b })
                }
                Calc::TIMES => {
                    let x = a;
                    let y = b;
                    #[allow(unused_imports)]
                    use ::kompoz::runtime::kt::ext::*;
                    ::kompoz::runtime::named_args!({ x * y })
                }
                _ => ::core::unreachable!(),
            }
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Calc {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unreachable_patterns)]
        match self {
            Calc::TIMES => {
                #[allow(unused_imports)]
                use ::kompoz::runtime::kt::ext::*;
                let s: ::std::string::String = ::kompoz::runtime::named_args!({ ::std::string::String::from("kez") });
                __kz_out.push_str(&s)
            }
            _ => __kz_out.push_str(&self.name()),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Calc {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzScoreboard {
    name: ::kompoz::runtime::kt::KtVal<::std::string::String>,
    count: ::kompoz::runtime::kt::KtVar<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Scoreboard {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzScoreboard>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Scoreboard(name: impl ::core::convert::Into<::std::string::String>) -> Scoreboard {
    let name: ::std::string::String = ::core::convert::Into::into(name);
    Scoreboard::__kz_new(name)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Scoreboard {
    #[doc(hidden)]
    pub fn __kz_new(name: ::std::string::String) -> Scoreboard {
        let __kz_this = Scoreboard {
            __kz_this: ::std::rc::Rc::new(__KzScoreboard {
                name: ::kompoz::runtime::kt::KtVal::new(),
                count: ::kompoz::runtime::kt::KtVar::new(),
            }),
        };
        __kz_this.__kz_init(name);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, name: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.name.init(::core::clone::Clone::clone(&name));
        ::kompoz::runtime::named_args!({
            self.__kz_this.count.set(0);
        })
    }
    pub fn name(&self) -> ::std::string::String {
        self.__kz_this.name.get()
    }
    pub fn count(&self) -> i32 {
        self.__kz_this.count.get()
    }
    pub fn set_count(&self, value: i32) {
        self.__kz_this.count.set(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Scoreboard {
    #[allow(non_snake_case)]
    pub fn Counter(&self, step: i32) -> ScoreboardCounter {
        ScoreboardCounter::__kz_new(::core::clone::Clone::clone(self), step)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Scoreboard {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Scoreboard {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Scoreboard {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Scoreboard {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Scoreboard",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Scoreboard {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Scoreboard {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzScoreboardCounter {
    #[doc(hidden)]
    pub __kz_outer: Scoreboard,
    step: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct ScoreboardCounter {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzScoreboardCounter>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ScoreboardCounter {
    #[doc(hidden)]
    pub fn __kz_new(__kz_outer: Scoreboard, step: i32) -> ScoreboardCounter {
        let __kz_this = ScoreboardCounter {
            __kz_this: ::std::rc::Rc::new(__KzScoreboardCounter {
                __kz_outer,
                step: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(step);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, step: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_variables)]
        let __kz_outer = self.__kz_outer();
        self.__kz_this.step.init(step);
        ::kompoz::runtime::named_args!({})
    }
    #[doc(hidden)]
    pub fn __kz_outer(&self) -> Scoreboard {
        ::core::clone::Clone::clone(&self.__kz_this.__kz_outer)
    }
    pub fn step(&self) -> i32 {
        self.__kz_this.step.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl ScoreboardCounter {
    pub fn bump(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_variables)]
        let __kz_outer = self.__kz_outer();
        ::kompoz::runtime::named_args!({
            __kz_outer.set_count(__kz_outer.count() + self.step());
        })
    }
    pub fn owner(&self) -> Scoreboard {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_variables)]
        let __kz_outer = self.__kz_outer();
        ::kompoz::runtime::named_args!({ __kz_outer })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for ScoreboardCounter {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for ScoreboardCounter {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for ScoreboardCounter {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for ScoreboardCounter {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Counter",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for ScoreboardCounter {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for ScoreboardCounter {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Check: ::kompoz::runtime::kt::KtObject {
    fn test(&self, text: ::std::string::String) -> bool;
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
impl<__KzF: ::core::ops::Fn(::std::string::String) -> bool + 'static> Check for __KzF {
    fn test(&self, text: ::std::string::String) -> bool {
        self(text)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn Check {
    fn eq(&self, other: &Self) -> bool {
        Check::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Check {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Check::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Check {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Check {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Check {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Check::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Fetcher: ::kompoz::runtime::kt::KtObject {
    fn load(&self, key: ::std::string::String) -> ::kompoz::runtime::kt::KtFuture<'_, ::std::string::String>;
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
impl<__KzF, __KzFut> Fetcher for __KzF
where
    __KzF: ::core::ops::Fn(::std::string::String) -> __KzFut + 'static,
    __KzFut: ::core::future::Future<Output = ::std::string::String> + 'static,
{
    fn load(&self, key: ::std::string::String) -> ::kompoz::runtime::kt::KtFuture<'_, ::std::string::String> {
        ::std::boxed::Box::pin(self(key))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for dyn Fetcher {
    fn eq(&self, other: &Self) -> bool {
        Fetcher::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Fetcher {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Fetcher::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Fetcher {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Fetcher {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Fetcher {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Fetcher::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Maker<T>: ::kompoz::runtime::kt::KtObject {
    fn make(&self, n: i32) -> T;
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
impl<T: ::core::clone::Clone + 'static> ::core::cmp::PartialEq for dyn Maker<T> {
    fn eq(&self, other: &Self) -> bool {
        Maker::<T>::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::kompoz::runtime::kt::KtToString for dyn Maker<T> {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Maker::<T>::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::fmt::Display for dyn Maker<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static> ::core::fmt::Debug for dyn Maker<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static, __KzT: ::core::any::Any + ::core::clone::Clone>
    ::kompoz::runtime::kt::KtAs<__KzT> for dyn Maker<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Maker::<T>::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzWidget {
    size: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Widget {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzWidget>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Widget(size: i32) -> Widget {
    Widget::__kz_new(size)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Widget {
    #[doc(hidden)]
    pub fn __kz_new(size: i32) -> Widget {
        let __kz_this =
            Widget { __kz_this: ::std::rc::Rc::new(__KzWidget { size: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(size);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, size: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_variables)]
        let __kz_companion = WidgetCompanion {};
        self.__kz_this.size.init(size);
        ::kompoz::runtime::named_args!({})
    }
    pub fn size(&self) -> i32 {
        self.__kz_this.size.get()
    }
    #[allow(non_upper_case_globals)]
    pub const Companion: WidgetCompanion = WidgetCompanion {};
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Widget {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Widget {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Widget {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Widget {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Widget {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Widget",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Widget {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Widget {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzWidgetCompanion {}
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
pub struct WidgetCompanion {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl WidgetCompanion {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzWidgetCompanion {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzWidgetCompanion > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzWidgetCompanion {}, |_| WidgetCompanion {}.__kz_init()))
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
impl WidgetCompanion {
    pub fn make(&self, n: i32) -> Widget {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&n,);
        ::kompoz::runtime::named_args!(types(Widget(r)), { Widget(n * 10) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for WidgetCompanion {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Widget$Companion",
            Self::__kz_state() as *const __KzWidgetCompanion as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for WidgetCompanion {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for WidgetCompanion {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Maker<Widget> for WidgetCompanion {
    fn make(&self, n: i32) -> Widget {
        WidgetCompanion::make(self, n)
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn Maker<Widget>>>() {
            let o: ::std::rc::Rc<dyn Maker<Widget>> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<WidgetCompanion> for ::std::rc::Rc<dyn Maker<Widget>> {
    fn eq(&self, other: &WidgetCompanion) -> bool {
        Maker::<Widget>::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn Maker<Widget>>> for WidgetCompanion {
    fn eq(&self, other: &::std::rc::Rc<dyn Maker<Widget>>) -> bool {
        Maker::<Widget>::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn Maker<Widget>>> for WidgetCompanion {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn Maker<Widget>>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn forms() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Tag(r), Round(r), DoubleStepper(), ScreenDetail(r)), {
        let mut plates: ::std::vec::Vec<::std::rc::Rc<dyn PlateOpen>> = ::kompoz::runtime::kt::list_of([{
            let __kz_shared4: ::std::rc::Rc<dyn PlateOpen> = ::std::rc::Rc::new(Round(1_f32));
            __kz_shared4
        }]);
        let stepper: ::std::rc::Rc<dyn StepperOpen> = ::std::rc::Rc::new(DoubleStepper());
        stepper.tick();
        let check = {
            let __kz_f5: ::std::rc::Rc<dyn Check> =
                ::std::rc::Rc::new(move |it: ::std::string::String| ::kompoz::runtime::kt::KtStr::is_not_empty(&it));
            __kz_f5
        };
        let anonymous = {
            #[allow(non_camel_case_types)]
            struct __KzObject {
                p0: ::std::string::String,
            }
            impl Titled for __KzObject {
                fn name(&self) -> ::std::string::String {
                    ::core::clone::Clone::clone(&self.p0)
                }
            }
            let __kz_object6: ::std::rc::Rc<dyn Titled> =
                ::std::rc::Rc::new(__KzObject { p0: ::std::string::String::from("adsız") });
            __kz_object6
        };
        let mut all = ::core::convert::identity::<::std::vec::Vec<Screen>>(::kompoz::runtime::kt::list_of([
            ::core::convert::Into::<Screen>::into(ScreenHome {}),
            ::core::convert::Into::<Screen>::into(ScreenDetail(7)),
        ]));
        let t: ::std::rc::Rc<dyn Titled> = ::std::rc::Rc::new(Tag("x"));
        return ::kompoz::runtime::kt::template!(
            ::kompoz::runtime::kt::KtList::get(&plates, 0).describe(),
            stepper.total(),
            check.test(::std::string::String::from("x")),
            anonymous.title(),
            ::kompoz::runtime::kt::KtList::size(&all),
            (::kompoz::runtime::kt::kt_is::<Tag, _>(&(*t))),
            ::kompoz::runtime::kt::safe!(
                (::kompoz::runtime::kt::KtAs::<::std::rc::Rc<dyn Shelf>>::kt_as_safe(&(*t))),
                |__kz_it7| __kz_it7.count()
            ),
            first_or(::kompoz::runtime::kt::list_of([1]), 0),
            ::kompoz::runtime::kt::list_of([::std::string::String::from("a"), ::std::string::String::from("b")])
                .second(),
            Calc::TIMES.apply(2, 3)
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn checks(x: ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>) -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::kt_is::<::std::string::String, _>(&(*x)) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn count<__KzStar0: ::core::clone::Clone + 'static>(xs: ::std::vec::Vec<__KzStar0>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtList::size(&xs) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct OutcomeBase {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl OutcomeBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        OutcomeBase {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait OutcomeOpen<T>: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_outcome(&self) -> &OutcomeBase;
    #[doc(hidden)]
    fn __kz_outcome_init(&self)
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    fn __kz_outcome_open_value_or_null(&self) -> ::core::option::Option<T>
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(OutcomeDone(r)), {
            if let ::core::option::Option::Some(__kz_smart8) =
                ::kompoz::runtime::kt::kt_as_this::<OutcomeDone<T>, _>(self)
            {
                ::core::option::Option::Some(__kz_smart8.value())
            } else {
                ::core::option::Option::None
            }
        })
    }
    fn value_or_null(&self) -> ::core::option::Option<T>
    where
        T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq,
    {
        self.__kz_outcome_open_value_or_null()
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
pub enum Outcome<T> {
    Done(OutcomeDone<T>),
    Failed(OutcomeFailed),
    Loading(OutcomeLoading),
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for Outcome<T>
{
    fn eq(&self, other: &Self) -> bool {
        #[allow(unreachable_patterns)]
        match (self, other) {
            (Outcome::Done(a), Outcome::Done(b)) => a == b,
            (Outcome::Failed(a), Outcome::Failed(b)) => a == b,
            (Outcome::Loading(a), Outcome::Loading(b)) => a == b,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<OutcomeDone<T>> for Outcome<T>
{
    fn from(value: OutcomeDone<T>) -> Outcome<T> {
        Outcome::Done(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<OutcomeDone<T>> for Outcome<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<OutcomeDone<T>> {
        match self {
            Outcome::Done(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<OutcomeDone<T>> for Outcome<T>
{
    fn eq(&self, other: &OutcomeDone<T>) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Outcome::Done(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Outcome<T>> for OutcomeDone<T>
{
    fn eq(&self, other: &Outcome<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<OutcomeFailed> for Outcome<T>
{
    fn from(value: OutcomeFailed) -> Outcome<T> {
        Outcome::Failed(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<OutcomeFailed> for Outcome<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<OutcomeFailed> {
        match self {
            Outcome::Failed(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<OutcomeFailed> for Outcome<T>
{
    fn eq(&self, other: &OutcomeFailed) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Outcome::Failed(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Outcome<T>> for OutcomeFailed
{
    fn eq(&self, other: &Outcome<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::convert::From<OutcomeLoading> for Outcome<T>
{
    fn from(value: OutcomeLoading) -> Outcome<T> {
        Outcome::Loading(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtAs<OutcomeLoading> for Outcome<T>
{
    fn kt_as_safe(&self) -> ::core::option::Option<OutcomeLoading> {
        match self {
            Outcome::Loading(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
            #[allow(unreachable_patterns)]
            _ => ::core::option::Option::None,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<OutcomeLoading> for Outcome<T>
{
    fn eq(&self, other: &OutcomeLoading) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            Outcome::Loading(v) => v == other,
            _ => false,
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq<Outcome<T>> for OutcomeLoading
{
    fn eq(&self, other: &Outcome<T>) -> bool {
        other == self
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for Outcome<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        match self {
            Outcome::Done(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Outcome::Failed(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
            Outcome::Loading(v) => ::kompoz::runtime::kt::KtToString::kt_write(v, __kz_out),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for Outcome<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for Outcome<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> OutcomeOpen<T>
    for Outcome<T>
{
    fn __kz_outcome(&self) -> &OutcomeBase {
        match self {
            Outcome::Done(v) => OutcomeOpen::<T>::__kz_outcome(v),
            Outcome::Failed(v) => OutcomeOpen::<T>::__kz_outcome(v),
            Outcome::Loading(v) => OutcomeOpen::<T>::__kz_outcome(v),
        }
    }
    fn __kz_outcome_open_value_or_null(&self) -> ::core::option::Option<T> {
        match self {
            Outcome::Done(v) => OutcomeOpen::<T>::__kz_outcome_open_value_or_null(v),
            Outcome::Failed(v) => OutcomeOpen::<T>::__kz_outcome_open_value_or_null(v),
            Outcome::Loading(v) => OutcomeOpen::<T>::__kz_outcome_open_value_or_null(v),
        }
    }
    fn value_or_null(&self) -> ::core::option::Option<T> {
        match self {
            Outcome::Done(v) => OutcomeOpen::<T>::value_or_null(v),
            Outcome::Failed(v) => OutcomeOpen::<T>::value_or_null(v),
            Outcome::Loading(v) => OutcomeOpen::<T>::value_or_null(v),
        }
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
            || match self {
                Outcome::Done(v) => OutcomeOpen::<T>::__kz_eq(v, other),
                Outcome::Failed(v) => OutcomeOpen::<T>::__kz_eq(v, other),
                Outcome::Loading(v) => OutcomeOpen::<T>::__kz_eq(v, other),
            }
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        match self {
            Outcome::Done(v) => OutcomeOpen::<T>::__kz_cast(v, to),
            Outcome::Failed(v) => OutcomeOpen::<T>::__kz_cast(v, to),
            Outcome::Loading(v) => OutcomeOpen::<T>::__kz_cast(v, to),
        }
    }
    fn __kz_outcome_init(&self) {}
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct OutcomeDone<T> {
    pub value: T,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<OutcomeBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn OutcomeDone<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>(
    value: T,
) -> OutcomeDone<T> {
    OutcomeDone::__kz_new(value)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> OutcomeDone<T> {
    #[doc(hidden)]
    pub fn __kz_new(value: T) -> OutcomeDone<T> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(OutcomeDone(r)), {
            let __kz_v = OutcomeDone {
                value: ::core::clone::Clone::clone(&value),
                __kz_super: ::std::rc::Rc::new(OutcomeBase::__kz_empty()),
            };
            OutcomeOpen::<T>::__kz_outcome_init(&__kz_v);
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
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> OutcomeDone<T> {
    pub fn copy(&self, # [default (:: core :: clone :: Clone :: clone (& self . value))] value: T) -> OutcomeDone<T> {
        OutcomeDone::__kz_new(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::cmp::PartialEq for OutcomeDone<T>
{
    fn eq(&self, other: &Self) -> bool {
        true && self.value == other.value
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::kompoz::runtime::kt::KtToString for OutcomeDone<T>
{
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Done(");
        __kz_out.push_str("value=");
        (&::kompoz::runtime::kt::KtProbe(&self.value)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq>
    ::core::fmt::Display for OutcomeDone<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> ::core::fmt::Debug
    for OutcomeDone<T>
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<T: ::core::clone::Clone + 'static + ::kompoz::runtime::kt::KtToString + ::core::cmp::PartialEq> OutcomeOpen<T>
    for OutcomeDone<T>
{
    fn __kz_outcome(&self) -> &OutcomeBase {
        &*self.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<Outcome<T>>() {
            return ::core::option::Option::Some(::std::boxed::Box::new(
                <Outcome<T> as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self)),
            ));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct OutcomeFailed {
    pub reason: ::std::string::String,
    #[doc(hidden)]
    pub __kz_super: ::std::rc::Rc<OutcomeBase>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn OutcomeFailed(reason: impl ::core::convert::Into<::std::string::String>) -> OutcomeFailed {
    let reason: ::std::string::String = ::core::convert::Into::into(reason);
    OutcomeFailed::__kz_new(reason)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl OutcomeFailed {
    #[doc(hidden)]
    pub fn __kz_new(reason: ::std::string::String) -> OutcomeFailed {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(OutcomeFailed(r)), {
            let __kz_v = OutcomeFailed {
                reason: ::core::clone::Clone::clone(&reason),
                __kz_super: ::std::rc::Rc::new(OutcomeBase::__kz_empty()),
            };
            OutcomeOpen::<::kompoz::runtime::kt::KtNothing>::__kz_outcome_init(&__kz_v);
            __kz_v
        })
    }
    pub fn reason(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.reason)
    }
    pub fn component1(&self) -> ::std::string::String {
        self.reason()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl OutcomeFailed {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . reason))] reason: ::std::string::String,
    ) -> OutcomeFailed {
        OutcomeFailed::__kz_new(reason)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for OutcomeFailed {
    fn eq(&self, other: &Self) -> bool {
        true && self.reason == other.reason
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for OutcomeFailed {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Failed(");
        __kz_out.push_str("reason=");
        (&::kompoz::runtime::kt::KtProbe(&self.reason)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for OutcomeFailed {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for OutcomeFailed {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzN0: ::core::clone::Clone + 'static> OutcomeOpen<__KzN0> for OutcomeFailed {
    fn __kz_outcome(&self) -> &OutcomeBase {
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
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzOutcomeLoading {
    #[doc(hidden)]
    pub __kz_super: OutcomeBase,
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
pub struct OutcomeLoading {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl OutcomeLoading {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzOutcomeLoading {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzOutcomeLoading > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(|| __KzOutcomeLoading { __kz_super: OutcomeBase::__kz_empty() }, |_| OutcomeLoading {}.__kz_init())
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            OutcomeOpen::<::kompoz::runtime::kt::KtNothing>::__kz_outcome_init(self);
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl OutcomeLoading {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for OutcomeLoading {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Outcome.Loading",
            Self::__kz_state() as *const __KzOutcomeLoading as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for OutcomeLoading {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for OutcomeLoading {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzN0: ::core::clone::Clone + 'static> OutcomeOpen<__KzN0> for OutcomeLoading {
    fn __kz_outcome(&self) -> &OutcomeBase {
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
pub fn parse(text: impl ::core::convert::Into<::std::string::String>) -> Outcome<i32> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let text: ::std::string::String = ::core::convert::Into::into(text);
    ::kompoz::runtime::named_args!(types(OutcomeDone(r), OutcomeFailed(r)), {
        if ::kompoz::runtime::kt::KtStr::is_empty(&text) {
            ::core::convert::Into::<Outcome<i32>>::into(OutcomeFailed("boş"))
        } else {
            ::core::convert::Into::<Outcome<i32>>::into(OutcomeDone(::kompoz::runtime::kt::KtStr::length(&text)))
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn shown(o: Outcome<i32>) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            let __kz_w9 = o.clone();
            match &__kz_w9 {
                Outcome::Done(__kz_case10) => {
                    #[allow(unused_variables)]
                    let o = ::core::clone::Clone::clone(__kz_case10);
                    { ::kompoz::runtime::kt::template!("#", o.value()) }
                }
                Outcome::Failed(_) | Outcome::Loading(_) => ::std::string::String::from("-"),
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn handler() -> NavigationEventHandler<NavigationEventInfoNone> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            #[allow(non_camel_case_types, clippy::type_complexity)]
            struct __KzObject {
                m0: ::std::rc::Rc<dyn ::core::ops::Fn(&NavigationEventHandler<NavigationEventInfoNone>)>,
            }
            #[allow(non_snake_case, clippy::all)]
            impl NavigationEventHandlerCallbacks<NavigationEventInfoNone> for __KzObject {
                fn on_back_completed(&self, handler: &NavigationEventHandler<NavigationEventInfoNone>) {
                    (self.m0)(handler)
                }
            }
            NavigationEventHandler::new(
                NavigationEventInfoNone,
                true,
                __KzObject {
                    m0: ::std::rc::Rc::new({
                        move |__kz_this: &NavigationEventHandler<NavigationEventInfoNone>| {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = ();
                            ::kompoz::runtime::named_args!({
                                ::kompoz::runtime::kt::println(__kz_this.transition_state());
                            })
                        }
                    }),
                },
            )
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzIntPairType {}
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
pub struct IntPairType {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl IntPairType {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzIntPairType {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzIntPairType > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| s.get(|| __KzIntPairType {}, |_| IntPairType {}.__kz_init()))
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
impl IntPairType {
    pub fn put(&self, bundle: SavedState, key: impl ::core::convert::Into<::std::string::String>, value: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let key: ::std::string::String = ::core::convert::Into::into(key);
        let _ = (&bundle, &key, &value);
        ::kompoz::runtime::named_args!({})
    }
    pub fn get(
        &self,
        bundle: SavedState,
        key: impl ::core::convert::Into<::std::string::String>,
    ) -> ::core::option::Option<i32> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let key: ::std::string::String = ::core::convert::Into::into(key);
        let _ = (&bundle, &key);
        ::kompoz::runtime::named_args!({ ::core::option::Option::None })
    }
    pub fn parse_value(&self, value: impl ::core::convert::Into<::std::string::String>) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let value: ::std::string::String = ::core::convert::Into::into(value);
        let _ = (&value,);
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtStr::to_int(&value) })
    }
    pub fn serialize_as_value(&self, value: i32) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let _ = (&value,);
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(value) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for IntPairType {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "IntPairType",
            Self::__kz_state() as *const __KzIntPairType as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for IntPairType {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for IntPairType {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::convert::From<IntPairType> for NavType {
    fn from(o: IntPairType) -> NavType {
        NavType::new(o)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl NavTypeImpl for IntPairType {
    type Value = i32;
    fn is_nullable_allowed(&self) -> bool {
        false
    }
    fn put(&self, bundle: &SavedState, key: &str, value: ::core::option::Option<&i32>) {
        IntPairType::put(
            self,
            ::core::clone::Clone::clone(bundle),
            ::std::string::String::from(key),
            ::kompoz::runtime::kt::not_null(::core::option::Option::map(value, ::core::clone::Clone::clone)),
        )
    }
    fn get(&self, bundle: &SavedState, key: &str) -> ::core::option::Option<i32> {
        IntPairType::get(self, ::core::clone::Clone::clone(bundle), ::std::string::String::from(key))
    }
    fn parse_value(&self, value: &str) -> ::core::result::Result<::core::option::Option<i32>, NavTypeError> {
        ::core::result::Result::Ok(::core::option::Option::Some(IntPairType::parse_value(
            self,
            ::std::string::String::from(value),
        )))
    }
    fn serialize_as_value(&self, value: ::core::option::Option<&i32>) -> ::std::string::String {
        IntPairType::serialize_as_value(
            self,
            ::kompoz::runtime::kt::not_null(::core::option::Option::map(value, ::core::clone::Clone::clone)),
        )
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct Fetcher2Base {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Fetcher2Base {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        Fetcher2Base {}
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Fetcher2Open: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_fetcher2(&self) -> &Fetcher2Base;
    #[doc(hidden)]
    fn __kz_fetcher2_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({})
    }
    fn calls(&self) -> i32;
    fn set_calls(&self, value: i32);
    fn fetch(&self, key: ::std::string::String) -> ::kompoz::runtime::kt::KtFuture<'_, ::std::string::String>;
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
impl ::core::cmp::PartialEq for dyn Fetcher2Open {
    fn eq(&self, other: &Self) -> bool {
        Fetcher2Open::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Fetcher2Open {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Fetcher2Open::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Fetcher2Open {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Fetcher2Open {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Fetcher2Open {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Fetcher2Open::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn counting() -> ::std::rc::Rc<dyn Fetcher2Open> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        {
            #[allow(non_camel_case_types, clippy::type_complexity)]
            struct __KzObject {
                #[doc(hidden)]
                pub __kz_super: Fetcher2Base,
                p0: ::std::cell::RefCell<i32>,
                m1: ::std::rc::Rc<
                    dyn ::core::ops::Fn(
                            &__KzObject,
                            ::std::string::String,
                        )
                            -> ::kompoz::runtime::kt::KtFuture<'_, ::std::string::String>,
                >,
            }
            fn __kz_hr2<
                F: for<'a> ::core::ops::Fn(
                        &'a __KzObject,
                        ::std::string::String,
                    )
                        -> ::kompoz::runtime::kt::KtFuture<'a, ::std::string::String>,
            >(
                f: F,
            ) -> F {
                f
            }
            #[allow(non_snake_case, clippy::all)]
            impl Fetcher2Open for __KzObject {
                fn __kz_fetcher2(&self) -> &Fetcher2Base {
                    &self.__kz_super
                }
                fn calls(&self) -> i32 {
                    ::core::clone::Clone::clone(&*self.p0.borrow())
                }
                fn set_calls(&self, value: i32) {
                    *self.p0.borrow_mut() = value;
                }
                fn fetch(
                    &self,
                    key: ::std::string::String,
                ) -> ::kompoz::runtime::kt::KtFuture<'_, ::std::string::String> {
                    (self.m1)(self, key)
                }
            }
            let __kz_o = __KzObject {
                __kz_super: Fetcher2Base::__kz_empty(),
                p0: ::std::cell::RefCell::new(0),
                m1: ::std::rc::Rc::new(__kz_hr2({
                    move |__kz_this: &__KzObject, key: ::std::string::String| {
                        ::std::boxed::Box::pin(async move {
                            #[allow(unused_imports)]
                            use ::kompoz::runtime::kt::ext::*;
                            let _ = (&key,);
                            ::kompoz::runtime::named_args!({
                                __kz_this.set_calls(::kompoz::runtime::kt::KtInc::inc(__kz_this.calls()));
                                return key.clone();
                            })
                        })
                    }
                })),
            };
            Fetcher2Open::__kz_fetcher2_init(&__kz_o);
            let __kz_object12: ::std::rc::Rc<dyn Fetcher2Open> = ::std::rc::Rc::new(__kz_o);
            __kz_object12
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct Panel2Base {
    w: ::kompoz::runtime::kt::KtVal<i32>,
    h: ::kompoz::runtime::kt::KtVal<i32>,
    note: ::kompoz::runtime::kt::KtVar<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Panel2Base {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        Panel2Base {
            w: ::kompoz::runtime::kt::KtVal::new(),
            h: ::kompoz::runtime::kt::KtVal::new(),
            note: ::kompoz::runtime::kt::KtVar::new(),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait Panel2Open: ::kompoz::runtime::kt::KtObject {
    #[doc(hidden)]
    fn __kz_panel2(&self) -> &Panel2Base;
    fn w(&self) -> i32 {
        self.__kz_panel2().w.get()
    }
    fn h(&self) -> i32 {
        self.__kz_panel2().h.get()
    }
    fn note(&self) -> ::std::string::String {
        self.__kz_panel2().note.get()
    }
    fn set_note(&self, value: ::std::string::String) {
        self.__kz_panel2().note.set(value)
    }
    #[doc(hidden)]
    fn __kz_panel2_init(&self, w: i32, h: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_panel2().w.init(w);
            self.__kz_panel2().h.init(h);
            self.__kz_panel2().note.set(::std::string::String::from(""));
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
impl ::core::cmp::PartialEq for dyn Panel2Open {
    fn eq(&self, other: &Self) -> bool {
        Panel2Open::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn Panel2Open {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        Panel2Open::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn Panel2Open {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn Panel2Open {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn Panel2Open {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            Panel2Open::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzPanel2 {
    #[doc(hidden)]
    pub __kz_super: Panel2Base,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Panel2 {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzPanel2>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Panel2(w: i32, h: i32) -> Panel2 {
    Panel2::__kz_new(w, h)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Panel2 {
    #[doc(hidden)]
    pub fn __kz_new(w: i32, h: i32) -> Panel2 {
        let __kz_this = Panel2 { __kz_this: ::std::rc::Rc::new(__KzPanel2 { __kz_super: Panel2Base::__kz_empty() }) };
        __kz_this.__kz_init(w, h);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, w: i32, h: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        Panel2Open::__kz_panel2_init(self, w, h);
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Panel2 {
    #[doc(hidden)]
    pub fn __kz_ctor1(side: i32) -> Panel2 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let __kz_this: Panel2 = ::kompoz::runtime::named_args!(types(Panel2(rr)), Panel2(side, side));
        __kz_this.__kz_ctor_body1(side);
        __kz_this
    }
    #[doc(hidden)]
    fn __kz_ctor_body1(&self, side: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.set_note(::std::string::String::from("kare"));
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Panel2 {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Panel2 {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Panel2 {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Panel2 {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Panel2",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Panel2 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Panel2 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Panel2Open for Panel2 {
    fn __kz_panel2(&self) -> &Panel2Base {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn Panel2Open>>() {
            let o: ::std::rc::Rc<dyn Panel2Open> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<Panel2> for ::std::rc::Rc<dyn Panel2Open> {
    fn eq(&self, other: &Panel2) -> bool {
        Panel2Open::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn Panel2Open>> for Panel2 {
    fn eq(&self, other: &::std::rc::Rc<dyn Panel2Open>) -> bool {
        Panel2Open::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn Panel2Open>> for Panel2 {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn Panel2Open>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzSquarePanel {
    #[doc(hidden)]
    pub __kz_super: Panel2Base,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct SquarePanel {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzSquarePanel>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn SquarePanel(s: i32) -> SquarePanel {
    SquarePanel::__kz_new(s)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl SquarePanel {
    #[doc(hidden)]
    pub fn __kz_new(s: i32) -> SquarePanel {
        let __kz_this =
            SquarePanel { __kz_this: ::std::rc::Rc::new(__KzSquarePanel { __kz_super: Panel2Base::__kz_empty() }) };
        __kz_this.__kz_init(s);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, s: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        {
            let side: i32 = s;
            Panel2Open::__kz_panel2_init(self, side, side);
            self.set_note(::std::string::String::from("kare"));
        }
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl SquarePanel {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for SquarePanel {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for SquarePanel {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for SquarePanel {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for SquarePanel {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "SquarePanel",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for SquarePanel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for SquarePanel {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Panel2Open for SquarePanel {
    fn __kz_panel2(&self) -> &Panel2Base {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn Panel2Open>>() {
            let o: ::std::rc::Rc<dyn Panel2Open> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<SquarePanel> for ::std::rc::Rc<dyn Panel2Open> {
    fn eq(&self, other: &SquarePanel) -> bool {
        Panel2Open::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn Panel2Open>> for SquarePanel {
    fn eq(&self, other: &::std::rc::Rc<dyn Panel2Open>) -> bool {
        Panel2Open::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn Panel2Open>> for SquarePanel {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn Panel2Open>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct AppExceptionBase {
    #[doc(hidden)]
    pub __kz_exception: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::Throwable>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppExceptionBase {
    #[doc(hidden)]
    pub fn __kz_empty() -> Self {
        AppExceptionBase { __kz_exception: ::kompoz::runtime::kt::KtVal::new() }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub trait AppExceptionOpen: ::kompoz::runtime::kt::KtObject {
    #[doc = r" Kotlin's `message`."]
    fn message(&self) -> ::core::option::Option<::std::string::String> {
        self.__kz_app_exception().__kz_exception.with(|e| e.message())
    }
    #[doc = r" Kotlin's `cause`."]
    fn cause(&self) -> ::core::option::Option<::kompoz::runtime::kt::Throwable> {
        self.__kz_app_exception().__kz_exception.with(|e| e.cause())
    }
    #[doc(hidden)]
    fn __kz_app_exception(&self) -> &AppExceptionBase;
    #[doc(hidden)]
    fn __kz_app_exception_init(&self, message: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_app_exception().__kz_exception.init(::kompoz::runtime::kt::exception(
                "Exception",
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
impl ::core::cmp::PartialEq for dyn AppExceptionOpen {
    fn eq(&self, other: &Self) -> bool {
        AppExceptionOpen::__kz_eq(self, ::kompoz::runtime::kt::KtObject::as_any(other))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for dyn AppExceptionOpen {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        AppExceptionOpen::__kz_write(self, __kz_out)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for dyn AppExceptionOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for dyn AppExceptionOpen {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl<__KzT: ::core::any::Any + ::core::clone::Clone> ::kompoz::runtime::kt::KtAs<__KzT> for dyn AppExceptionOpen {
    fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
        ::kompoz::runtime::kt::kt_dyn_as::<__KzT>(::kompoz::runtime::kt::KtObject::as_any(self), || {
            AppExceptionOpen::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
        })
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for AppException {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::Throwable::of_class_object(
            "AppException",
            &[],
            "Exception",
            <Self as AppExceptionOpen>::message(self),
            <Self as AppExceptionOpen>::cause(self),
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
            |o, to| <AppException as AppExceptionOpen>::__kz_cast(o.downcast_ref::<AppException>()?, to),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<AppException> for ::kompoz::runtime::kt::Throwable {
    fn from(e: AppException) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<AppException> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<AppException> {
        self.downcast::<AppException>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzAppException {
    #[doc(hidden)]
    pub __kz_super: AppExceptionBase,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct AppException {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzAppException>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn AppException(message: impl ::core::convert::Into<::std::string::String>) -> AppException {
    let message: ::std::string::String = ::core::convert::Into::into(message);
    AppException::__kz_new(message)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppException {
    #[doc(hidden)]
    pub fn __kz_new(message: ::std::string::String) -> AppException {
        let __kz_this = AppException {
            __kz_this: ::std::rc::Rc::new(__KzAppException { __kz_super: AppExceptionBase::__kz_empty() }),
        };
        __kz_this.__kz_init(message);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, message: ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        AppExceptionOpen::__kz_app_exception_init(self, ::std::string::String::from(message.clone()));
        ::kompoz::runtime::named_args!({})
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl AppException {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for AppException {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for AppException {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for AppException {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for AppException {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for AppException {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for AppException {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppExceptionOpen for AppException {
    fn __kz_app_exception(&self) -> &AppExceptionBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn AppExceptionOpen>>() {
            let o: ::std::rc::Rc<dyn AppExceptionOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<AppException> for ::std::rc::Rc<dyn AppExceptionOpen> {
    fn eq(&self, other: &AppException) -> bool {
        AppExceptionOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn AppExceptionOpen>> for AppException {
    fn eq(&self, other: &::std::rc::Rc<dyn AppExceptionOpen>) -> bool {
        AppExceptionOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn AppExceptionOpen>> for AppException {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn AppExceptionOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtThrowable for NetworkException {
    fn to_throwable(&self) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::Throwable::of_class_object(
            "NetworkException",
            &["AppException"],
            "Exception",
            <Self as AppExceptionOpen>::message(self),
            <Self as AppExceptionOpen>::cause(self),
            ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
            |o, to| <NetworkException as AppExceptionOpen>::__kz_cast(o.downcast_ref::<NetworkException>()?, to),
        )
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::core::convert::From<NetworkException> for ::kompoz::runtime::kt::Throwable {
    fn from(e: NetworkException) -> ::kompoz::runtime::kt::Throwable {
        ::kompoz::runtime::kt::KtThrowable::to_throwable(&e)
    }
}
#[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
impl ::kompoz::runtime::kt::KtAs<NetworkException> for ::kompoz::runtime::kt::Throwable {
    fn kt_as_safe(&self) -> ::core::option::Option<NetworkException> {
        self.downcast::<NetworkException>()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzNetworkException {
    #[doc(hidden)]
    pub __kz_super: AppExceptionBase,
    code: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct NetworkException {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzNetworkException>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn NetworkException(code: i32) -> NetworkException {
    NetworkException::__kz_new(code)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl NetworkException {
    #[doc(hidden)]
    pub fn __kz_new(code: i32) -> NetworkException {
        let __kz_this = NetworkException {
            __kz_this: ::std::rc::Rc::new(__KzNetworkException {
                __kz_super: AppExceptionBase::__kz_empty(),
                code: ::kompoz::runtime::kt::KtVal::new(),
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
        AppExceptionOpen::__kz_app_exception_init(
            self,
            ::std::string::String::from(::kompoz::runtime::kt::template!("ağ ", code)),
        );
        self.__kz_this.code.init(code);
        ::kompoz::runtime::named_args!({})
    }
    pub fn code(&self) -> i32 {
        self.__kz_this.code.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl NetworkException {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for NetworkException {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for NetworkException {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for NetworkException {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for NetworkException {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::KtToString::kt_to_string(
            &::kompoz::runtime::kt::KtThrowable::to_throwable(self),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for NetworkException {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for NetworkException {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl AppExceptionOpen for NetworkException {
    fn __kz_app_exception(&self) -> &AppExceptionBase {
        &self.__kz_this.__kz_super
    }
    fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }
    fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
        ::kompoz::runtime::kt::KtToString::kt_write(self, __kz_out)
    }
    fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
        if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn AppExceptionOpen>>() {
            let o: ::std::rc::Rc<dyn AppExceptionOpen> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
            return ::core::option::Option::Some(::std::boxed::Box::new(o));
        }
        ::core::option::Option::None
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<NetworkException> for ::std::rc::Rc<dyn AppExceptionOpen> {
    fn eq(&self, other: &NetworkException) -> bool {
        AppExceptionOpen::__kz_eq(&**self, other)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq<::std::rc::Rc<dyn AppExceptionOpen>> for NetworkException {
    fn eq(&self, other: &::std::rc::Rc<dyn AppExceptionOpen>) -> bool {
        AppExceptionOpen::__kz_eq(&**other, self)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtAs<::std::rc::Rc<dyn AppExceptionOpen>> for NetworkException {
    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn AppExceptionOpen>> {
        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn handled() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(NetworkException(r)), {
        match ::kompoz::runtime::kt::catching(|| {
            ::kompoz::runtime::kt::throw(NetworkException(1));
        }) {
            ::core::result::Result::Ok(__kz_v) => __kz_v,
            ::core::result::Result::Err(__kz_e) => {
                if __kz_e.is_a("AppException") {
                    #[allow(unused_variables)]
                    let e = ::core::option::Option::unwrap(__kz_e.cast_object::<::std::rc::Rc<dyn AppExceptionOpen>>());
                    ::kompoz::runtime::kt::elvis(e.message(), || ::std::string::String::from(""))
                } else {
                    ::kompoz::runtime::kt::rethrow(__kz_e)
                }
            }
        }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn describe_any(
    x: impl ::core::convert::Into<::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>>>,
) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let x: ::core::option::Option<::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>> = ::core::convert::Into::into(x);
    ::kompoz::runtime::named_args!({
        if let ::core::option::Option::Some(x) =
            ::core::option::Option::and_then(::core::option::Option::as_ref(&x), |__kz_o13| {
                ::kompoz::runtime::kt::KtAs::<::std::string::String>::kt_as_safe(&**__kz_o13)
            })
            && ::kompoz::runtime::kt::KtStr::length(&x) > 2
        {
            x.clone()
        } else if let ::core::option::Option::Some(x) =
            ::core::option::Option::and_then(::core::option::Option::as_ref(&x), |__kz_o14| {
                ::kompoz::runtime::kt::KtAs::<i32>::kt_as_safe(&**__kz_o14)
            })
            && !(x < 0)
        {
            ::kompoz::runtime::kt::template!(x + 1)
        } else {
            ::std::string::String::from("-")
        }
    })
}
